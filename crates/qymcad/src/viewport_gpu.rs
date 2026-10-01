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
//! be hung on it. The matrices agree with `Screen::at` pixel for pixel (orthographic, world-up upwards).
//!
//! Colour: the offscreen target is in an sRGB format, the fragment gives out a LINEAR colour
//! (srgb-to-linear), and the write encodes it back into the same sRGB bytes the CPU path lays down
//! (`Color32`), so the colour matches frame for frame. The light and the shading, and the hot and ghost
//! branches, are computed on the CPU by the same formula as the raster (see `App::shade_tri`) and
//! uploaded ONCE per change of the scene (the light is of the world and does not depend on the camera).
//! Back faces are culled in the fragment by the ray FROM THE EYE (in orthographic that is `fwd`, in
//! perspective a direction of its own at every point), with the normal of the face carried in the
//! vertices.

use qymcad_ui_state::GpuVert;
use eframe::egui_wgpu;
use eframe::wgpu;
use egui::PaintCallbackInfo;

/// The offscreen target lives in GAMMA space (non-sRGB), like the main framebuffer of egui: the sRGB
/// bytes of `Color32` are kept as they are, with no hardware conversion. The blit carries them one for one
/// (or encodes them, if the target format is sRGB).
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Texels in one row of the look table: the texture width, 2048 being the smallest limit a device may have
/// (the shader's `look_of` divides by the same number).
const LOOK_ROW: u32 = 2048;
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

/// HOW THE OFFSCREEN COLOUR ATTACHMENT IS WIRED for the current sample count.
///
/// Returns whether a separate multisample target is resolved into the colour texture, and how that
/// colour attachment stores its own samples. MSAA off (`samples == 1`) draws STRAIGHT into the colour
/// target: a resolve from 1 sample into 1 sample is a wgpu validation error, and that was exactly what
/// crashed the program at startup when a person chose "Off" in the settings.
pub(crate) fn color_attachment_plan(samples: u32) -> (bool, wgpu::StoreOp) {
    if samples > 1 {
        (true, wgpu::StoreOp::Discard)
    } else {
        (false, wgpu::StoreOp::Store)
    }
}


/// The camera uniform (orthographic). `right`/`up`/`fwd` are an orthonormal basis; the projection
/// repeats `Screen::at`.
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
    /// THE LIGHT AND THE SHADING, moved here from the vertices.
    ///
    /// `light` is the direction the scene is lit from; `shade` is [floor, ghost_alpha 0..1, 0, 0], and
    /// `ghost` is the colour a ghost is led towards. All of it used to be applied on the processor and baked
    /// into every vertex - which is why a change of highlight rewrote the whole scene.
    light: [f32; 4],
    shade: [f32; 4],
    ghost: [f32; 4],
}

/// WHAT THE SHADING NEEDS, gathered from the scheme and the settings: the light, the floor the shading
/// cannot go below, how solid a ghost is, and the colour a ghost is led towards.
#[derive(Clone, Copy)]
pub struct ShadeRaw {
    pub light: [f32; 3],
    pub floor: f32,
    pub ghost_alpha: f32,
    pub ghost_target: [f32; 3],
}

/// THE EYE-SPACE DEPTH BOUNDS the GPU clips against. Set by the caller from the same formula as
/// `proj_params`, so that the CPU and the GPU agree about what is in front of what.
#[derive(Clone, Copy)]
pub struct ZRange {
    pub near: f32,
    pub far: f32,
}

impl CamRaw {
    /// Build from the basis of the camera (`Cam3::basis`), the scale and the rectangle of the viewport
    /// (in points). `persp_inv_d_eye` is 1/d_eye (0 for orthographic), `z_near` and `z_far` are the
    /// eye-space bounds (for perspective); all of it is set by the caller from the same formula as
    /// `proj_params`, so the CPU and the GPU agree.
    pub fn new(basis: &([f64; 3], [f64; 3], [f64; 3]), scale: f32, target: [f64; 3], size: egui::Vec2, persp_inv_d_eye: f32, z: ZRange, look: ShadeRaw) -> Self {
        let (rect_w, rect_h, z_near, z_far) = (size.x, size.y, z.near, z.far);
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
            light: [look.light[0], look.light[1], look.light[2], 0.0],
            shade: [look.floor, look.ghost_alpha, 0.0, 0.0],
            ghost: [look.ghost_target[0], look.ghost_target[1], look.ghost_target[2], 0.0],
        }
    }
}

pub(crate) const SHADER: &str = r#"
struct Cam {
    right: vec4<f32>,
    up: vec4<f32>,
    fwd: vec4<f32>,
    tgt: vec4<f32>,
    params: vec4<f32>, // scale, half_w, half_h, depth_half
    persp: vec4<f32>,  // inv_d_eye (0 for orthographic), z_near, z_far, _
    light: vec4<f32>,  // the direction the scene is lit from
    shade: vec4<f32>,  // floor, ghost_alpha, _, _
    ghost: vec4<f32>,  // the colour a ghost is led towards
};
@group(0) @binding(0) var<uniform> cam: Cam;

// WHAT A BODY LOOKS LIKE: its own colour (already brightened) and the flags of its look - bit 1 selected,
// bit 2 a ghost, bit 4 the cap of a section (`BodyLook`). One row per body, rewritten every frame; the
// vertices know only which row is theirs.
struct Look {
    tint: u32,
    state: u32,
};
// A TEXTURE, NOT A STORAGE BUFFER: a device reached through OpenGL has no storage buffers in the fragment stage
// (limit 0), and the program died at start there with exit code 101. A row of `LOOK_ROW` texels, two numbers each.
@group(1) @binding(0) var looks: texture_2d<u32>;
fn look_of(body: u32) -> Look {
    let t = textureLoad(looks, vec2<i32>(i32(body % 2048u), i32(body / 2048u)), 0);
    return Look(t.x, t.y);
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // THE ROW OF THE LOOK TABLE travels flat: it is the same for the whole triangle, and interpolating an
    // index would be meaningless.
    @location(1) @interpolate(flat) body: u32,
    // THE VERTEX NORMAL TRAVELS INTERPOLATED, and that is the whole of smooth shading: unpacked in the vertex
    // stage and blended across the triangle, so a curved surface reads as curved. Carried flat - which is how
    // it was first written - every triangle took one vertex's normal and the smooth mode drew facets. Reported
    // on an imported engine: pipes and fan blades visibly polygonal, while the software raster (which
    // interpolates its colours) drew the same bodies smooth.
    @location(3) nrm: vec3<f32>,
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

// a signed byte out of the low 8 bits
fn sbyte(v: u32) -> f32 {
    let b = f32(v & 0xffu);
    return select(b, b - 256.0, b > 127.0) / 127.0;
}

@vertex
fn vs_mesh(@location(0) pos: vec3<f32>, @location(1) body: u32, @location(2) nrm_packed: u32) -> VsOut {
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
    out.wpos = pos;
    out.body = body;
    // zero means flat shading: then the fragment takes the face normal from the derivatives
    out.nrm = select(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(sbyte(nrm_packed), sbyte(nrm_packed >> 8u), sbyte(nrm_packed >> 16u)), nrm_packed != 0u);
    return out;
}

@fragment
fn fs_mesh(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // THE SIDE TURNED AWAY FROM THE EYE IS NOT DRAWN, and which side that is comes from `front_facing` - the
    // winding of this triangle as it came out ON SCREEN.
    //
    // Asked that way the question is answered per triangle and per point, which is what perspective needs: the
    // ray of sight is its own at every point of the frame, and the wider the field of view the further it
    // diverges from the camera axis at the edges. Culling by a shared direction lost visible faces (slits in a
    // body) and kept invisible ones (a ring broken into ribbons) - two reported screenshots.
    //
    // The derivatives cannot answer it: `cross(dpdx, dpdy)` is built from the screen basis and comes out
    // pointing along the line of sight for EVERY triangle, whichever way it is wound. Taking its sign from
    // `front_facing` and then testing it against the ray discarded exactly the faces that should have stayed -
    // reported as extruded text losing its walls, while the software raster drew the same document correctly.
    if (!front) { discard; }
    // THE FLAT NORMAL COMES FROM THE DERIVATIVES of the world position across the triangle - the same normal
    // that used to be carried in every vertex, 12 bytes lighter per vertex. It faces the eye, and the light
    // takes it by its absolute value, so no sign has to be recovered.
    let n = normalize(cross(dpdx(in.wpos), dpdy(in.wpos)));

    // SMOOTH SHADING BRINGS ITS OWN NORMAL: a vertex normal packed into four bytes. Flat shading leaves it
    // zero and the face normal above is used - which is exactly what flat shading means.
    var shading_n = n;
    if (dot(in.nrm, in.nrm) > 0.25) {
        shading_n = normalize(in.nrm);
    }

    let look = look_of(in.body);
    let tint = vec3<f32>(f32(look.tint & 0xffu), f32((look.tint >> 8u) & 0xffu), f32((look.tint >> 16u) & 0xffu));
    // THE SHADING, moved here from the processor: it can only darken, never brighten past the body's own
    // colour, and never below the floor the scheme sets.
    let floor = cam.shade.x;
    let diff = abs(dot(shading_n, cam.light.xyz));
    let lit = floor + clamp(diff, 0.0, 1.0) * (1.0 - floor);

    if ((look.state & 4u) != 0u) {
        // the cap of a section: a fill, not a surface of the part - it is not shaded
        return vec4<f32>(tint / 255.0, 1.0);
    }
    if ((look.state & 1u) != 0u) {
        // A SELECTED BODY: lifted towards white with a slight cool tint - and OPAQUE even when it is a ghost.
        // The selection replaces the colour whole, exactly as it does in the raster; what stays with the
        // ghost is its PASS, chosen outside the shader by the same bit.
        let v = tint * lit;
        let cool = vec3<f32>(0.0, 8.0, 22.0);
        return vec4<f32>(min(v + (vec3<f32>(255.0) - v) * 0.4 + cool, vec3<f32>(255.0)) / 255.0, 1.0);
    }
    if ((look.state & 2u) != 0u) {
        // a ghost: a quarter of its own colour and three quarters of the colour the scheme leads it towards
        return vec4<f32>(tint * lit * 0.25 / 255.0 + cam.ghost.xyz * 0.75, cam.shade.y);
    }
    return vec4<f32>(tint * lit / 255.0, 1.0);
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
    /// The depth of the ghosts alone, drawn before their colour (see `mesh_pipeline_ghost_depth` where it is made).
    mesh_pipeline_ghost_depth: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    cam_buf: wgpu::Buffer,
    cam_bind: wgpu::BindGroup,
    blit_layout: wgpu::BindGroupLayout,
    look_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // the resources for the current size of the viewport (recreated on a resize)
    /// The multisample render target, resolved into `color_view`. `None` when antialiasing is off
    /// (`samples == 1`): the pass then draws straight into `color_view`.
    msaa_view: Option<wgpu::TextureView>,
    color_tex: Option<wgpu::Texture>, // the same texture, kept so a check can copy the drawn picture out
    color_view: Option<wgpu::TextureView>, // the single-sample colour the blit samples (resolve target, or the draw target when MSAA is off)
    depth_view: Option<wgpu::TextureView>, // depth matching the draw target's sample count
    blit_bind: Option<wgpu::BindGroup>,
    size: [u32; 2],
    // THE BUFFERS OF THE SCENE, in pieces (re-uploaded only when scene_key changes): a vertex buffer and an
    // index buffer for each piece.
    //
    // Reported behaviour on the heaviest reference file, a V8 engine in STEP: the program died with
    // "Buffer size 739358112 is greater than the maximum buffer size (268435456)". A device has a limit on
    // ONE buffer - 256 MB is the ordinary one - and the engine holds 23 million vertices. So the scene is cut
    // into pieces and drawn one piece after another; the blocks are laid into the pieces by
    // `gui::scene_chunks::pack_blocks`, and a block never lies across a seam - an indexed draw reads from one
    // buffer only.
    bufs: Vec<(wgpu::Buffer, wgpu::Buffer)>,
    /// Where each block lies and whose it is: the pass is chosen from the look of that body, so a body
    /// becoming a ghost changes no buffer at all.
    spans: Vec<Span>,
    /// The look table on the card, rewritten every frame - two numbers per body.
    look_buf: Option<wgpu::Texture>,
    look_bind: Option<wgpu::BindGroup>,
    look_len: usize,
    vcount: u32,
    scene_key: u64,
}

/// ONE BLOCK AS THE DRAW SEES IT: which piece it lies in, which of that piece's indices are its own, where
/// its vertices begin (the `base_vertex` of the draw), and whose body it is.
struct Span {
    chunk: usize,
    idx: std::ops::Range<u32>,
    base_vertex: i32,
    body: u32,
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

        // THE LOOK TABLE: an integer texture, one texel per body. It is the whole of what a change of highlight
        // costs now - a few kilobytes rewritten, against the scene re-uploaded. A texture rather than a storage
        // buffer so it reads on every backend, OpenGL included (see `look_of` in the shader).
        let look_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qym_look_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            }],
        });
        // --- the mesh pipeline ---
        let mesh_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qym_mesh_pl"),
            bind_group_layouts: &[Some(&cam_layout), Some(&look_layout)],
            immediate_size: 0,
        });
        let vbl = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVert>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32, offset: 16, shader_location: 2 },
            ],
        };
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_mesh_pipeline"),
            layout: Some(&mesh_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: std::slice::from_ref(&vbl) },
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

        // --- the depth of translucent bodies (ghosts) ---
        // A GHOST IS SEEN AS ITS NEAREST SURFACE, one pane of glass: this pass writes the depth of the ghosts and no
        // colour, and the colour pass after it blends only what lies at that depth. Blended face over face, the walls
        // of a hole through a ghost and the faces behind them heaped up darker than the rest - a mess of layers.
        let mesh_pipeline_ghost_depth = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_mesh_pipeline_ghost_depth"),
            layout: Some(&mesh_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: std::slice::from_ref(&vbl) },
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
                targets: &[Some(wgpu::ColorTargetState { format: OFFSCREEN_FORMAT, blend: None, write_mask: wgpu::ColorWrites::empty() })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- the pipeline of translucent bodies (ghosts) ---
        // The colour of the ghosts comes AFTER their depth: alpha-blended on top of the opaque bodies, at the depth the
        // pass before wrote (LessEqual), so each pixel takes the nearest face of a ghost once. The colour is already
        // premultiplied (`Color32`), hence PREMULTIPLIED_ALPHA_BLENDING.
        let mesh_pipeline_ghost = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qym_mesh_pipeline_ghost"),
            layout: Some(&mesh_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: std::slice::from_ref(&vbl) },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
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
            mesh_pipeline_ghost_depth,
            blit_pipeline,
            cam_buf,
            cam_bind,
            blit_layout,
            look_layout,
            sampler,
            msaa_view: None,
            color_tex: None,
            color_view: None,
            depth_view: None,
            blit_bind: None,
            size: [0, 0],
            bufs: Vec::new(),
            spans: Vec::new(),
            look_buf: None,
            look_bind: None,
            look_len: 0,
            vcount: 0,
            scene_key: u64::MAX,
        }
    }

    /// Recreate the offscreen colour and depth for a new size (in pixels), plus the bind group of the
    /// blit.
    fn ensure_size(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if size == self.size && self.color_view.is_some() {
            return;
        }
        let samples = msaa_samples();
        let (resolve, _) = color_attachment_plan(samples);
        let extent = wgpu::Extent3d { width: size[0].max(1), height: size[1].max(1), depth_or_array_layers: 1 };
        // THE COLOUR THE BLIT SAMPLES — always single-sample. With MSAA it is the resolve target; without,
        // the pass draws into it directly.
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qym_offscreen_color"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            // COPY_SRC IS THERE SO THE PICTURE CAN BE TAKEN OFF THE CARD. Everything below the blit - the
            // culling, the shading, the look table, the indices - is invisible to a check that only looks at
            // the numbers, and a mistake in it does not crash: it draws the body wrong. Reported behaviour:
            // "the faces fell apart" on a document the software rasteriser drew correctly.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qym_offscreen_depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        // A MULTISAMPLE TARGET ONLY WHEN THERE IS SOMETHING TO RESOLVE. Creating one with sample_count 1
        // and still naming it as a resolve source is what wgpu refused — and closed the window on start.
        let msaa_view = if resolve {
            let msaa = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("qym_offscreen_msaa"),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: OFFSCREEN_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            Some(msaa.create_view(&wgpu::TextureViewDescriptor::default()))
        } else {
            None
        };
        let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("qym_blit_bind"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        self.msaa_view = msaa_view;
        self.color_tex = Some(color);
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
    /// The scene in pieces, in drawing order: one per body, plus the caps of a section. `None` means the
    /// geometry has not changed and only the look below has.
    verts: Option<Vec<qymcad_ui_state::ScenePiece>>,
    /// What every body looks like at this moment - rewritten every frame, two numbers per body.
    looks: Vec<qymcad_ui_state::BodyLook>,
    scene_key: u64,
}

impl MeshPaint {
    pub fn new(cam: CamRaw, size_px: [u32; 2], verts: Option<Vec<qymcad_ui_state::ScenePiece>>, looks: Vec<qymcad_ui_state::BodyLook>, scene_key: u64) -> Self {
        Self { cam, size_px, verts, looks, scene_key }
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
            if self.scene_key != gpu.scene_key || gpu.bufs.is_empty() {
                // THE LIMIT IS ASKED OF THE DEVICE, not assumed: it differs between cards and drivers, and a
                // number written here would be right on one machine and fatal on another.
                let (vsize, isize_) = (std::mem::size_of::<GpuVert>() as u64, 4u64);
                let room = crate::gui::scene_chunks::room_in_a_chunk(device.limits().max_buffer_size, vsize, isize_);
                let sizes: Vec<(u32, u32)> = verts.iter().map(|p| (p.verts.len() as u32, p.idx.len() as u32)).collect();
                let placed = crate::gui::scene_chunks::pack_blocks(&sizes, room);
                // how much each piece holds in the end: the last block laid into it says so
                let mut totals: Vec<(u32, u32)> = Vec::new();
                for (p, (v, i)) in placed.iter().zip(&sizes) {
                    let c = p.chunk as usize;
                    if totals.len() <= c {
                        totals.resize(c + 1, (0, 0));
                    }
                    totals[c] = (p.first_vertex + v, p.first_index + i);
                }
                gpu.bufs.clear();
                for (v, i) in &totals {
                    let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("qym_scene_vbuf"),
                        size: (*v as u64 * vsize).max(4),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let ibuf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("qym_scene_ibuf"),
                        size: (*i as u64 * isize_).max(4),
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    gpu.bufs.push((vbuf, ibuf));
                }
                // EVERY BLOCK GOES STRAIGHT FROM THE SCENE into the piece that holds it, whole: its vertices
                // at one offset, its indices at another. The indices stay LOCAL to the block - where its
                // vertices begin is told to the draw as `base_vertex`.
                gpu.spans.clear();
                for (part, at) in verts.iter().zip(&placed) {
                    let Some((vbuf, ibuf)) = gpu.bufs.get(at.chunk as usize) else { continue };
                    queue.write_buffer(vbuf, at.first_vertex as u64 * vsize, bytemuck::cast_slice(&part.verts));
                    queue.write_buffer(ibuf, at.first_index as u64 * isize_, bytemuck::cast_slice(&part.idx));
                    gpu.spans.push(Span {
                        chunk: at.chunk as usize,
                        idx: at.first_index..at.first_index + part.idx.len() as u32,
                        base_vertex: at.first_vertex as i32,
                        body: part.body,
                    });
                }
                gpu.vcount = verts.iter().map(|p| p.idx.len() as u32).sum();
                gpu.scene_key = self.scene_key;
            }
        }

        // THE LOOK TABLE, EVERY FRAME. It is two numbers per body: rewriting it costs kilobytes, while the
        // geometry above is touched only when a shape or a position changed.
        if !self.looks.is_empty() {
            // rows of LOOK_ROW texels; the last row is padded, the texture being written whole
            let rows = self.looks.len().div_ceil(LOOK_ROW as usize) as u32;
            let mut table = self.looks.clone();
            table.resize(rows as usize * LOOK_ROW as usize, qymcad_ui_state::BodyLook::default());
            let bytes: &[u8] = bytemuck::cast_slice(&table);
            if gpu.look_len < table.len() || gpu.look_buf.is_none() {
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("qym_look_buf"),
                    size: wgpu::Extent3d { width: LOOK_ROW, height: rows, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rg32Uint,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.look_bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("qym_look_bind"),
                    layout: &gpu.look_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
                }));
                gpu.look_buf = Some(tex);
                gpu.look_len = table.len();
            }
            if let Some(tex) = gpu.look_buf.as_ref() {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytes,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(LOOK_ROW * 8), rows_per_image: Some(rows) },
                    wgpu::Extent3d { width: LOOK_ROW, height: rows, depth_or_array_layers: 1 },
                );
            }
        }

        // the offscreen pass: clear to transparent, draw the bodies with a depth buffer; with MSAA into
        // the multisample target and resolve into colour, without it straight into colour
        let (Some(cv), Some(dv)) = (gpu.color_view.as_ref(), gpu.depth_view.as_ref()) else { return Vec::new() };
        let (_, store) = color_attachment_plan(msaa_samples());
        let (draw_view, resolve_target) = match gpu.msaa_view.as_ref() {
            Some(mv) => (mv, Some(cv)),
            None => (cv, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qym_offscreen_pass"),
            multiview_mask: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: draw_view,
                resolve_target,
                depth_slice: None,
                // with MSAA the multisample texture is needed only for the resolve, so it is not stored;
                // without MSAA the colour target IS what the blit samples, so it must be kept
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: dv,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if gpu.vcount > 0 && !gpu.bufs.is_empty() {
            if let Some(look_bind) = gpu.look_bind.as_ref() {
                pass.set_bind_group(0, &gpu.cam_bind, &[]);
                pass.set_bind_group(1, look_bind, &[]);
                // WHICH PASS A BODY BELONGS TO IS READ FROM THE LOOK, not from where its vertices lie. A body
                // turning into a ghost changes a row of the table and nothing else; before, its vertices had
                // to move into the other half of the buffer, which meant rebuilding and re-uploading.
                //
                // The order of the passes is kept across the whole scene: every solid body first, so the depth
                // buffer is set, and the ghosts after - otherwise a ghost drawn early hides a solid body drawn
                // later.
                // THE PASS IS CHOSEN BY THE GHOST BIT ALONE, with no regard to the selection: picking a
                // neighbour's body paints it as selected and leaves it in the blended pass, as in the raster.
                let ghostly = |body: u32| self.looks.get(body as usize).is_some_and(|l| l.state & qymcad_ui_state::LOOK_GHOST != 0);
                for (ghost_pass, pipeline) in [(false, &gpu.mesh_pipeline), (true, &gpu.mesh_pipeline_ghost_depth), (true, &gpu.mesh_pipeline_ghost)] {
                    pass.set_pipeline(pipeline);
                    for span in &gpu.spans {
                        if ghostly(span.body) != ghost_pass {
                            continue;
                        }
                        let Some((vbuf, ibuf)) = gpu.bufs.get(span.chunk) else { continue };
                        pass.set_vertex_buffer(0, vbuf.slice(..));
                        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(span.idx.clone(), span.base_vertex, 0..1);
                    }
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

/// THE SAME RESOURCES, INTO A BARE BAG - for a check that renders without a window.
///
/// `install` below needs the render state of eframe, which exists only when a window is open. A check has a
/// device and nothing else, and it must run the SAME pipeline: a copy of the setup here would prove that the
/// copy works.
#[cfg(test)]
pub(crate) fn install_for_test(device: &wgpu::Device, resources: &mut egui_wgpu::CallbackResources) {
    resources.insert(GpuRenderer::new(device, OFFSCREEN_FORMAT));
}

/// The texture the bodies were drawn into, for copying the picture out of a check.
#[cfg(test)]
pub(crate) fn color_texture_for_test(resources: &egui_wgpu::CallbackResources) -> Option<wgpu::Texture> {
    resources.get::<GpuRenderer>().and_then(|g| g.color_tex.clone())
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
    use eframe::wgpu;
    use super::{CamRaw, ZRange};

    /// ANTIALIASING OFF MUST NOT ASK FOR A RESOLVE. Choosing "Off" (1 sample) used to keep the
    /// resolve path: both the source and the destination had 1 sample, wgpu refused that at the first
    /// frame, and the window closed on start with nothing a person could fix without editing the
    /// config by hand.
    #[test]
    fn antialiasing_off_does_not_resolve() {
        let (resolve, store) = super::color_attachment_plan(1);
        assert!(!resolve, "MSAA off still asks for a resolve from 1 sample into 1 sample");
        assert_eq!(store, wgpu::StoreOp::Store, "without a resolve the colour target itself must be kept for the blit");
        for n in [2u32, 4, 8, 16] {
            let (resolve, store) = super::color_attachment_plan(n);
            assert!(resolve, "{n}x MSAA must resolve into the colour target");
            assert_eq!(store, wgpu::StoreOp::Discard, "{n}x: the multisample buffer is only for the resolve");
        }
    }

    /// The camera basis for looking along -Z: right=+X, up=+Y, fwd=-Z.
    fn basis() -> ([f64; 3], [f64; 3], [f64; 3]) {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
    }

    /// Shading plays no part in what these checks measure - the projection does.
    fn plain() -> super::ShadeRaw {
        super::ShadeRaw { light: [0.0, 0.0, 1.0], floor: 0.35, ghost_alpha: 0.5, ghost_target: [0.1, 0.1, 0.12] }
    }

    #[test]
    fn camera_carries_basis_scale_and_viewport() {
        let c = CamRaw::new(&basis(), 2.0, [10.0, 20.0, 30.0], egui::vec2(800.0, 600.0), 0.0, ZRange { near: 0.1, far: 1000.0 }, plain());
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
        let near = CamRaw::new(&basis(), 10.0, [0.0; 3], egui::vec2(800.0, 600.0), 0.0, ZRange { near: 0.1, far: 1000.0 }, plain());
        let far = CamRaw::new(&basis(), 0.1, [0.0; 3], egui::vec2(800.0, 600.0), 0.0, ZRange { near: 0.1, far: 1000.0 }, plain());
        assert!(far.params[3] > near.params[3] * 10.0, "having pulled back, the depth of the clip has grown: {} -> {}", near.params[3], far.params[3]);
        assert!(near.params[3] >= 1000.0, "there is a depth margin even at a strong zoom: {}", near.params[3]);
    }

    /// A degenerate scale (zero or negative) must not give an infinity or a NaN in the uniform —
    /// otherwise the frame is drawn as rubbish rather than as the scene.
    #[test]
    fn degenerate_scale_stays_finite() {
        for scale in [0.0, -1.0, f32::MIN_POSITIVE] {
            let c = CamRaw::new(&basis(), scale, [0.0; 3], egui::vec2(800.0, 600.0), 0.0, ZRange { near: 0.1, far: 1000.0 }, plain());
            assert!(c.params.iter().all(|v| v.is_finite()), "scale {scale}: the parameters are finite, and what came back is {:?}", c.params);
            assert!(c.params[3] > 0.0, "the depth of the clip is positive at scale {scale}");
        }
    }

    /// Perspective: 1/d_eye and the near and far bounds reach the shader as they are (the CPU and the GPU
    /// compute by one formula — let them diverge and the picks stop matching the picture).
    #[test]
    fn perspective_params_pass_through() {
        let c = CamRaw::new(&basis(), 1.0, [0.0; 3], egui::vec2(1024.0, 768.0), 1.0 / 500.0, ZRange { near: 5.0, far: 2000.0 }, plain());
        assert!((c.persp[0] - 0.002).abs() < 1e-9, "1/d_eye");
        assert_eq!((c.persp[1], c.persp[2]), (5.0, 2000.0), "near and far are as they were passed");
    }
}
