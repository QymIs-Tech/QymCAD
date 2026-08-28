//! THE GPU PASS OF THE 3D VIEWPORT.
//!
//! It replaces the software rasterisation of bodies (`rasterize_3d`) with real GPU rendering through
//! `wgpu` under eframe: the vertices of a body are uploaded into video memory and projected in the vertex
//! shader, and visibility is decided by the hardware depth buffer (no CPU sorting or reprojection). It is
//! built into egui as a paint callback UNDER the 2D overlays (dimensions, edges and gizmos stay on the
//! `painter`).
//!
//! The scheme: in `prepare` the scene is drawn into an OFFSCREEN target (colour plus depth) through the
//! egui encoder; in `paint` the offscreen texture is blitted into the rectangle of the viewport with
//! alpha-over (a transparent background lets the floor grid show through from below). The offscreen
//! target is needed because the main pass of egui has NO depth attachment — a z-buffer of our own cannot
//! be hung on it. The matrices agree with `project3` pixel for pixel (orthographic, world-up upwards).
//!
//! Colour: the offscreen target is in an sRGB format, the fragment gives out a LINEAR colour
//! (srgb-to-linear), and the write encodes it back into the same sRGB bytes the CPU path lays down
//! (`Color32`), so the colour matches frame for frame. The light and the shading, and the hot and ghost
//! branches, are computed on the CPU by the same formula as the raster (see `App::shade_tri`) and
//! uploaded ONCE per change of the scene (the light is of the world and does not depend on the camera).
//! Back faces are culled in the fragment by the ray FROM THE EYE (in orthographic that is `fwd`, in
//! perspective a direction of its own at every point), with the normal of the face carried in the
//! vertices.

use eframe::egui_wgpu;
use eframe::wgpu;
use egui::PaintCallbackInfo;

/// The offscreen target lives in GAMMA space (non-sRGB), like the main framebuffer of egui: the sRGB
/// bytes of `Color32` are kept as they are, with no hardware conversion. The blit carries them one for one
/// (or encodes them, if the target format is sRGB).
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Edge antialiasing (MSAA). The bodies are rendered into a multisample target (colour plus depth) and
/// then resolved into a single-sample texture, which the blit samples. 4x is the universally supported
/// level.
/// THE NUMBER OF SAMPLES THIS RENDERER WAS BUILT WITH.
///
/// It is taken ONCE when the pipelines are created: they bake it into themselves. The setting lives in
/// `Settings::msaa` and takes effect on a restart — the settings window says so.
static MSAA_SAMPLES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(4);

#[cfg(test)]
pub fn msaa_samples_for_test() -> u32 {
    msaa_samples()
}

fn msaa_samples() -> u32 {
    MSAA_SAMPLES.load(std::sync::atomic::Ordering::Relaxed)
}

/// WHAT THIS DEVICE CAN REALLY DO (a bit mask over the positions 1/2/4/8/16). 0 means it has not been
/// asked yet.
static MSAA_SUPPORTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The sample counts supported by BOTH formats of the pass (colour and depth).
///
/// Asking is essential: the specification guarantees only 1 and 4, and everything else depends on the
/// luck of the hardware and the driver. 8x was once offered in the settings without asking — and the
/// program CRASHED AT STARTUP, that is, the setting made it unlaunchable: getting back would have meant
/// editing the config by hand.
pub fn supported_msaa() -> Vec<u32> {
    let mask = MSAA_SUPPORTED.load(std::sync::atomic::Ordering::Relaxed);
    if mask == 0 {
        return vec![1, 4]; // the device has not been asked yet, so only what the spec guarantees is promised
    }
    [1u32, 2, 4, 8, 16].into_iter().filter(|n| mask & (1 << n.trailing_zeros()) != 0).collect()
}

fn probe_supported(device: &wgpu::Device, target: wgpu::TextureFormat) {
    let feats = device.features();
    let ok = |fmt: wgpu::TextureFormat, n: u32| fmt.guaranteed_format_features(feats).flags.sample_count_supported(n);
    let mut mask = 0u32;
    for n in [1u32, 2, 4, 8, 16] {
        // the pass draws into the offscreen target, into the depth and (after the blit) into the target
        // format of the window — what suits is what ALL THREE can do: a pipeline is built against each
        if ok(OFFSCREEN_FORMAT, n) && ok(DEPTH_FORMAT, n) && ok(target, n) {
            mask |= 1 << n.trailing_zeros();
        }
    }
    MSAA_SUPPORTED.store(mask.max(1), std::sync::atomic::Ordering::Relaxed);
}

/// Remember the chosen number of samples BEFORE the renderer is created (called at startup).
///
/// An unsupported value does NOT bring the program down but is lowered to the nearest smaller one the
/// device can do: somebody asked for a prettier picture, and a program that does not work is not what
/// they should get for it.
pub fn set_msaa(n: u32) {
    let ok = supported_msaa();
    let take = ok.iter().rev().find(|&&s| s <= n).copied().unwrap_or(1);
    MSAA_SAMPLES.store(take, std::sync::atomic::Ordering::Relaxed);
}

/// A vertex of a body for the GPU. The colour and the normal do NOT depend on the camera (the light is
/// of the world), so the buffer is re-uploaded only when the scene changes and not when it is rotated.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVert {
    /// The position in world coordinates (with the transform of the owning component already applied).
    pub pos: [f32; 3],
    /// The normal of the face (the same for all 3 vertices of a triangle) — for culling back faces in
    /// the fragment.
    pub nrm: [f32; 3],
    /// The shaded colour as rgba8 (sRGB bytes, as in `Color32`): the low byte is r, the high one is a.
    pub color: u32,
    pub _pad: u32,
}

/// The camera uniform (orthographic). `right`/`up`/`fwd` are an orthonormal basis; the projection
/// repeats `project3`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CamRaw {
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    target: [f32; 4],
    /// [scale (points), half_w (points), half_h (points), depth_half (world)].
    params: [f32; 4],
    /// [inv_d_eye (1/d_eye for perspective, 0 for orthographic), z_near, z_far, 0] — the eye-space near
    /// and far come from the bounding box of the scene (tight ones mean precision in the z-buffer). The
    /// formula is the same as in `App::proj_params` and `depth_ndc`.
    persp: [f32; 4],
}

impl CamRaw {
    /// Build from the basis of the camera (`Cam3::basis`), the scale and the rectangle of the viewport
    /// (in points). `persp_inv_d_eye` is 1/d_eye (0 for orthographic), `z_near` and `z_far` are the
    /// eye-space bounds (for perspective); all of it is set by the caller from the same formula as
    /// `proj_params`, so the CPU and the GPU agree.
    pub fn new(basis: &([f64; 3], [f64; 3], [f64; 3]), scale: f32, target: [f64; 3], rect_w: f32, rect_h: f32, persp_inv_d_eye: f32, z_near: f32, z_far: f32) -> Self {
        let (r, u, f) = basis;
        let cv = |v: &[f64; 3]| [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
        // The depth range of the orthographic clip: it grows as one zooms out (the world half-extent is
        // half_w/scale) with a generous margin along the axis of view, so that deep bodies are not clipped
        // by the near and far planes.
        let half_w = rect_w * 0.5;
        let half_h = rect_h * 0.5;
        let depth_half = (half_w.max(half_h) / scale.max(1e-4)) * 50.0 + 1000.0;
        Self {
            right: cv(r),
            up: cv(u),
            fwd: cv(f),
            target: [target[0] as f32, target[1] as f32, target[2] as f32, 0.0],
            params: [scale, half_w, half_h, depth_half],
            persp: [persp_inv_d_eye, z_near, z_far, 0.0],
        }
    }
}

const SHADER: &str = r#"
struct Cam {
    right: vec4<f32>,
    up: vec4<f32>,
    fwd: vec4<f32>,
    tgt: vec4<f32>,
    params: vec4<f32>, // scale, half_w, half_h, depth_half
    persp: vec4<f32>,  // inv_d_eye (0 for orthographic), z_near, z_far, _
};
@group(0) @binding(0) var<uniform> cam: Cam;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) nrm: vec3<f32>,
    @location(1) color: vec4<f32>,
    // THE WORLD POINT — for culling back faces BY THE RAY FROM THE EYE. In perspective the direction of
    // view is its own at every point of the frame, and a shared `fwd` will not do for it (see
    // `fs_mesh`).
    @location(2) wpos: vec3<f32>,
};

// 0-1 linear out of 0-1 sRGB gamma (needed only if the target framebuffer is sRGB-aware)
fn s2l(c: vec3<f32>) -> vec3<f32> {
    let lower = c / 12.92;
    let higher = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(higher, lower, c <= vec3<f32>(0.04045));
}

@vertex
fn vs_mesh(@location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) color: u32) -> VsOut {
    var out: VsOut;
    let rel = pos - cam.tgt.xyz;
    let sx = dot(rel, cam.right.xyz);
    let sy = dot(rel, cam.up.xyz);
    let depth = dot(rel, cam.fwd.xyz);
    let scale = cam.params.x;
    let inv_d = cam.persp.x;                 // 1/d_eye (0 for orthographic)
    if (inv_d > 0.0) {
        // PERSPECTIVE: the real clip-w is the eye distance, the hardware divides by it and the depth
        // comes out perspective-correct (otherwise large triangles pierce each other when the depth is
        // interpolated across the screen).
        let d_eye = 1.0 / inv_d;
        let zc = depth + d_eye;              // the distance along the view from the eye (>0 in front of it)
        let z_near = cam.persp.y;            // tight near/far from the scene box (z-buffer precision)
        let z_far = cam.persp.z;
        let a = z_far / (z_far - z_near);
        let b = -z_near * z_far / (z_far - z_near); // ndc_z=clip_z/w=a+b/zc: near→0, far→1
        // ndc.xy = clip.xy/w: clip.xy = sx·scale/half·d_eye, /zc → sx·scale/half·(d_eye/zc)=…·f
        out.pos = vec4<f32>(sx * scale / cam.params.y * d_eye, sy * scale / cam.params.z * d_eye, a * zc + b, zc);
    } else {
        // ORTHOGRAPHIC (as it was): w=1, the depth is linear in the world (in orthographic that is the
        // same as linear on screen)
        let ndc_x = sx * scale / cam.params.y;
        let ndc_y = sy * scale / cam.params.z;     // world-up becomes ndc +y (upwards), as in the raster
        let ndc_z = 0.5 + depth / (2.0 * cam.params.w);
        out.pos = vec4<f32>(ndc_x, ndc_y, ndc_z, 1.0);
    }
    out.nrm = nrm;
    out.wpos = pos;
    let r = f32(color & 0xffu) / 255.0;
    let g = f32((color >> 8u) & 0xffu) / 255.0;
    let b = f32((color >> 16u) & 0xffu) / 255.0;
    let a = f32((color >> 24u) & 0xffu) / 255.0;
    out.color = vec4<f32>(r, g, b, a); // the sRGB bytes as they are, into the gamma offscreen unconverted
    return out;
}

@fragment
fn fs_mesh(in: VsOut) -> @location(0) vec4<f32> {
    // CULLING BACK FACES BY THE RAY FROM THE EYE, not by a shared direction of view.
    //
    // In ORTHOGRAPHIC the ray is one for the whole frame and `fwd` is exactly it. In PERSPECTIVE it is its
    // own at every point, and the wider the field of view the further it diverges from `fwd` at the edges:
    // some VISIBLE faces were being discarded (slits appeared in a body) and some invisible ones stayed (a
    // ring fell apart into ribbons). That arrived as a screenshot the moment the field of view was allowed
    // to become a setting.
    let inv_d = cam.persp.x;
    var view = cam.fwd.xyz;
    if (inv_d > 0.0) {
        let eye = cam.tgt.xyz - cam.fwd.xyz * (1.0 / inv_d);
        view = normalize(in.wpos - eye);
    }
    if (dot(in.nrm, view) >= 0.0) { discard; } // bodies are oriented outwards
    return in.color;
}

// ---- the blit of the offscreen target into the rectangle of the viewport (a fullscreen triangle) ----
struct BlitOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) vi: u32) -> BlitOut {
    var out: BlitOut;
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0; // -1, 3, -1
    let y = f32(vi & 2u) * 2.0 - 1.0;         // -1, -1, 3
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5); // ndc +y (up) becomes uv.y 0 (the top row)
    return out;
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_smp: sampler;

// A gamma target framebuffer (non-sRGB, the ordinary eframe case): the sRGB bytes are carried one for one.
@fragment
fn fs_blit_gamma(in: BlitOut) -> @location(0) vec4<f32> {
    return textureSample(src_tex, src_smp, in.uv);
}

// An sRGB-aware target framebuffer: the hardware write encodes, so a linear colour is what to give out.
@fragment
fn fs_blit_srgb(in: BlitOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, src_smp, in.uv);
    return vec4<f32>(s2l(c.rgb), c.a);
}
"#;

/// The persistent GPU resources of the viewport. They live in `Renderer::callback_resources` of the egui
/// render state.
pub struct GpuRenderer {
    mesh_pipeline: wgpu::RenderPipeline,
    mesh_pipeline_ghost: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    cam_buf: wgpu::Buffer,
    cam_bind: wgpu::BindGroup,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // the resources for the current size of the viewport (recreated on a resize)
    msaa_view: Option<wgpu::TextureView>, // the multisample render target, resolved into color_view
    color_view: Option<wgpu::TextureView>, // the single-sample resolve target, sampled by the blit
    depth_view: Option<wgpu::TextureView>, // the multisample depth
    blit_bind: Option<wgpu::BindGroup>,
    size: [u32; 2],
    // the vertex buffer of the scene (re-uploaded only when scene_key changes)
    vbuf: Option<wgpu::Buffer>,
    vcount: u32,
    /// The number of opaque vertices at the start of the buffer; [0..opaque_count) is the 1st pass
    /// (REPLACE, depth-write), [opaque_count..vcount) is the 2nd pass (ghost, alpha-blend, no
    /// depth-write).
    opaque_count: u32,
    scene_key: u64,
}

impl GpuRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("qym_viewport_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        // --- the camera uniform ---
        let cam_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qymcad_uniform"),
            size: std::mem::size_of::<CamRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cam_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qymcad_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let cam_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("qymcad_bind"),
            layout: &cam_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: cam_buf.as_entire_binding() }],
        });

        // --- the mesh pipeline ---
        let mesh_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qym_mesh_pl"),
            bind_group_layouts: &[Some(&cam_layout)],
            immediate_size: 0,
        });
        let vbl = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVert>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 24, shader_location: 2 },
            ],
        };
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_mesh_pipeline"),
            layout: Some(&mesh_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: &[vbl.clone()] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa_samples(), ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: OFFSCREEN_FORMAT, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- the pipeline of translucent bodies (ghosts) ---
        // The second pass comes AFTER the opaque one: alpha-blended on top, with the depth TESTED (so
        // occlusion by solid bodies works) but NOT written (translucent bodies do not occlude each other
        // by z, which avoids holes caused by ordering). The colour is already premultiplied (`Color32`),
        // hence PREMULTIPLIED_ALPHA_BLENDING.
        let mesh_pipeline_ghost = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_mesh_pipeline_ghost"),
            layout: Some(&mesh_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: &[vbl.clone()] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa_samples(), ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: OFFSCREEN_FORMAT, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- the blit pipeline ---
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qym_blit_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qym_blit_pl"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_blit_pipeline"),
            layout: Some(&blit_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_blit"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                // a gamma framebuffer (ordinary eframe) means a one-for-one carry; an sRGB-aware one
                // means giving out linear for the hardware to encode
                entry_point: Some(if target_format.is_srgb() { "fs_blit_srgb" } else { "fs_blit_gamma" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: target_format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("qym_blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            mesh_pipeline,
            mesh_pipeline_ghost,
            blit_pipeline,
            cam_buf,
            cam_bind,
            blit_layout,
            sampler,
            msaa_view: None,
            color_view: None,
            depth_view: None,
            blit_bind: None,
            size: [0, 0],
            vbuf: None,
            vcount: 0,
            opaque_count: 0,
            scene_key: u64::MAX,
        }
    }

    /// Recreate the offscreen colour and depth for a new size (in pixels), plus the bind group of the
    /// blit.
    fn ensure_size(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if size == self.size && self.color_view.is_some() {
            return;
        }
        let extent = wgpu::Extent3d { width: size[0].max(1), height: size[1].max(1), depth_or_array_layers: 1 };
        // the multisample render target — never sampled, only resolved
        let msaa = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qym_offscreen_msaa"),
            size: extent,
            mip_level_count: 1,
            sample_count: msaa_samples(),
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        // the single-sample resolve target — the blit carries it
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qym_offscreen_color"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qym_offscreen_depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: msaa_samples(),
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_view = msaa.create_view(&wgpu::TextureViewDescriptor::default());
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("qym_blit_bind"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        self.msaa_view = Some(msaa_view);
        self.color_view = Some(color_view);
        self.depth_view = Some(depth_view);
        self.blit_bind = Some(blit_bind);
        self.size = size;
    }
}

/// The per-frame paint callback: it carries the camera, the size of the viewport and (when the scene has
/// changed) new vertices.
pub struct MeshPaint {
    cam: CamRaw,
    size_px: [u32; 2],
    /// `Some` only when the scene has changed (otherwise the uploaded buffer is reused).
    verts: Option<std::sync::Arc<Vec<GpuVert>>>,
    /// The number of opaque vertices (the prefix of the buffer); the rest are translucent (ghosts).
    opaque_count: u32,
    scene_key: u64,
}

impl MeshPaint {
    pub fn new(cam: CamRaw, size_px: [u32; 2], verts: Option<std::sync::Arc<Vec<GpuVert>>>, opaque_count: u32, scene_key: u64) -> Self {
        Self { cam, size_px, verts, opaque_count, scene_key }
    }
}

impl egui_wgpu::CallbackTrait for MeshPaint {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<GpuRenderer>() else { return Vec::new() };
        queue.write_buffer(&gpu.cam_buf, 0, bytemuck::bytes_of(&self.cam));
        gpu.ensure_size(device, self.size_px);

        // re-upload the vertices only when the scene has changed
        if let Some(verts) = &self.verts {
            if self.scene_key != gpu.scene_key || gpu.vbuf.is_none() {
                let bytes: &[u8] = bytemuck::cast_slice(verts);
                let buf = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("qym_scene_vbuf"),
                    size: bytes.len().max(4) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                if !bytes.is_empty() {
                    queue.write_buffer(&buf, 0, bytes);
                }
                gpu.vbuf = Some(buf);
                gpu.vcount = verts.len() as u32;
                gpu.opaque_count = self.opaque_count.min(verts.len() as u32);
                gpu.scene_key = self.scene_key;
            }
        }

        // the offscreen pass: clear to transparent, draw the bodies with a depth buffer into the MSAA
        // target, resolve into the colour target
        let (Some(mv), Some(cv), Some(dv)) = (gpu.msaa_view.as_ref(), gpu.color_view.as_ref(), gpu.depth_view.as_ref()) else { return Vec::new() };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qym_offscreen_pass"),
            multiview_mask: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: mv,
                resolve_target: Some(cv),
                depth_slice: None,
                // the MSAA texture is needed only for the resolve, so it is not stored (Discard); the
                // resolve happens all the same
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: dv,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some(vbuf) = gpu.vbuf.as_ref() {
            if gpu.vcount > 0 {
                pass.set_bind_group(0, &gpu.cam_bind, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                let oc = gpu.opaque_count.min(gpu.vcount);
                // the 1st pass: opaque bodies (REPLACE plus depth-write) — they set the z-buffer
                if oc > 0 {
                    pass.set_pipeline(&gpu.mesh_pipeline);
                    pass.draw(0..oc, 0..1);
                }
                // the 2nd pass: translucent ones (ghosts) on top — alpha-blended, depth tested but not
                // written
                if oc < gpu.vcount {
                    pass.set_pipeline(&gpu.mesh_pipeline_ghost);
                    pass.draw(oc..gpu.vcount, 0..1);
                }
            }
        }
        drop(pass);
        Vec::new()
    }

    fn paint(&self, info: PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        let Some(gpu) = resources.get::<GpuRenderer>() else { return };
        let Some(bind) = gpu.blit_bind.as_ref() else { return };
        let vp = info.viewport_in_pixels();
        if vp.width_px <= 0 || vp.height_px <= 0 {
            return;
        }
        render_pass.set_viewport(vp.left_px as f32, vp.top_px as f32, vp.width_px as f32, vp.height_px as f32, 0.0, 1.0);
        render_pass.set_pipeline(&gpu.blit_pipeline);
        render_pass.set_bind_group(0, bind, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Install the GPU resources of the viewport into the egui render state (called from `launch` if the wgpu
/// backend is active). Returns `false` if there is no render state (the glow fallback) — and then the CPU
/// raster does the work.
pub fn install(render_state: &egui_wgpu::RenderState) -> bool {
    // ASK THE DEVICE BEFORE BUILDING THE PIPELINES and lower the request to what it can do: an
    // unsupported number of samples is a panic from wgpu right at startup, not "a slightly worse
    // picture".
    probe_supported(&render_state.device, render_state.target_format);
    set_msaa(MSAA_SAMPLES.load(std::sync::atomic::Ordering::Relaxed));
    let renderer = GpuRenderer::new(&render_state.device, render_state.target_format);
    render_state.renderer.write().callback_resources.insert(renderer);
    true
}

#[cfg(test)]
mod tests {
    //! The GPU viewport had not a single test, although THE CAMERA in it is pure arithmetic that can be
    //! checked without a window and without a GPU. And it is the camera that must agree with the CPU path
    //! (`proj_params`), otherwise the picture and the picks drift apart.
    use super::CamRaw;

    /// The camera basis for looking along -Z: right=+X, up=+Y, fwd=-Z.
    fn basis() -> ([f64; 3], [f64; 3], [f64; 3]) {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
    }

    #[test]
    fn camera_carries_basis_scale_and_viewport() {
        let c = CamRaw::new(&basis(), 2.0, [10.0, 20.0, 30.0], 800.0, 600.0, 0.0, 0.1, 1000.0);
        assert_eq!(c.right[..3], [1.0, 0.0, 0.0], "the right unit vector is as it was passed");
        assert_eq!(c.up[..3], [0.0, 1.0, 0.0]);
        assert_eq!(c.fwd[..3], [0.0, 0.0, -1.0]);
        assert_eq!(c.target[..3], [10.0, 20.0, 30.0], "the target is as it was passed");
        assert_eq!(c.params[0], 2.0, "the scale");
        assert_eq!((c.params[1], c.params[2]), (400.0, 300.0), "the half-sizes of the viewport in points");
        assert_eq!(c.persp[0], 0.0, "0 means an orthographic projection");
    }

    /// The depth range of the orthographic clip must GROW as one zooms out: otherwise, at a distance, the
    /// bodies start being cut by the near and far planes (the model gets "eaten" as the camera pulls
    /// back).
    #[test]
    fn ortho_depth_range_grows_when_zooming_out() {
        let near = CamRaw::new(&basis(), 10.0, [0.0; 3], 800.0, 600.0, 0.0, 0.1, 1000.0);
        let far = CamRaw::new(&basis(), 0.1, [0.0; 3], 800.0, 600.0, 0.0, 0.1, 1000.0);
        assert!(far.params[3] > near.params[3] * 10.0, "having pulled back, the depth of the clip has grown: {} -> {}", near.params[3], far.params[3]);
        assert!(near.params[3] >= 1000.0, "there is a depth margin even at a strong zoom: {}", near.params[3]);
    }

    /// A degenerate scale (zero or negative) must not give an infinity or a NaN in the uniform —
    /// otherwise the frame is drawn as rubbish rather than as the scene.
    #[test]
    fn degenerate_scale_stays_finite() {
        for scale in [0.0, -1.0, f32::MIN_POSITIVE] {
            let c = CamRaw::new(&basis(), scale, [0.0; 3], 800.0, 600.0, 0.0, 0.1, 1000.0);
            assert!(c.params.iter().all(|v| v.is_finite()), "scale {scale}: the parameters are finite, and what came back is {:?}", c.params);
            assert!(c.params[3] > 0.0, "the depth of the clip is positive at scale {scale}");
        }
    }

    /// Perspective: 1/d_eye and the near and far bounds reach the shader as they are (the CPU and the GPU
    /// compute by one formula — let them diverge and the picks stop matching the picture).
    #[test]
    fn perspective_params_pass_through() {
        let c = CamRaw::new(&basis(), 1.0, [0.0; 3], 1024.0, 768.0, 1.0 / 500.0, 5.0, 2000.0);
        assert!((c.persp[0] - 0.002).abs() < 1e-9, "1/d_eye");
        assert_eq!((c.persp[1], c.persp[2]), (5.0, 2000.0), "near and far are as they were passed");
    }
}
