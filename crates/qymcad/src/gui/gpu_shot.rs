//! THE PICTURE THE CARD ACTUALLY DREW, taken off it in a check.
//!
//! Reported behaviour: "the faces fell apart" on a document of extruded text. The software rasteriser drew
//! that same document correctly, so the mistake lived in the path the checks could not see - the shader, the
//! culling, the look table, the indices. Nothing there crashes when it is wrong; it draws the body wrong, and
//! a check that reads numbers stays green.
//!
//! So the scene is rendered through the REAL pipeline (`viewport_gpu`), into the same offscreen target the
//! window uses, and the pixels are copied back. There is a graphics device on this machine and the check uses
//! it; where there is none, it says so and stands aside rather than failing.
#[cfg(test)]
pub(crate) mod eyes {
    use eframe::egui_wgpu;
    use eframe::wgpu;

    /// Render the scene through the card and bring the pixels back. `None` means there is no device to draw
    /// with - not that the picture is wrong.
    pub(crate) fn shot(pn: &qymcad_ui_state::Painting, rect: egui::Rect) -> Option<egui::ColorImage> {
        shot_within(pn, rect, |adapter| adapter.limits())
    }

    /// The same picture drawn with `samples` per pixel rather than the program's own setting: 1 is antialiasing
    /// switched off. Given to the renderer it makes, so checks running beside it keep theirs.
    pub(crate) fn shot_with_samples(pn: &qymcad_ui_state::Painting, rect: egui::Rect, samples: u32) -> Option<egui::ColorImage> {
        shot_made(pn, rect, |adapter| adapter.limits(), samples)
    }

    /// The same picture on a device that allows no more than `limits` answers for the card at hand: a check stands
    /// in for a weaker device - a virtual machine reached through OpenGL has no storage buffers in the fragment
    /// stage at all - with the device on this machine.
    pub(crate) fn shot_within(pn: &qymcad_ui_state::Painting, rect: egui::Rect, limits: impl Fn(&wgpu::Adapter) -> wgpu::Limits) -> Option<egui::ColorImage> {
        shot_made(pn, rect, limits, crate::viewport_gpu::msaa_samples_for_test())
    }

    fn shot_made(pn: &qymcad_ui_state::Painting, rect: egui::Rect, limits: impl Fn(&wgpu::Adapter) -> wgpu::Limits, samples: u32) -> Option<egui::ColorImage> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qym_eye"),
            required_features: wgpu::Features::empty(),
            required_limits: limits(&adapter),
            ..Default::default()
        }))
        .ok()?;

        // THE SAME CALL THE WINDOW MAKES, with the same arguments: the camera, the scene and the look table
        // come from `draw_3d_gpu`'s own arithmetic, so what is drawn here is what is drawn there.
        let basis = pn.cam.basis();
        let key = qymcad_ui_state::gpu_scene_key(pn);
        let cam = crate::gui::render::frame_uniform(pn, rect, &basis, 1.0);
        let size_px = [rect.width().round().max(1.0) as u32, rect.height().round().max(1.0) as u32];
        let scene = crate::gui::render_scene::gpu_scene(pn);
        let paint = crate::viewport_gpu::MeshPaint::new(cam, size_px, Some(scene.pieces), scene.looks, key);

        let mut resources = egui_wgpu::CallbackResources::default();
        crate::viewport_gpu::install_for_test(&device, &mut resources, samples);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("qym_eye_enc") });
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size_px, pixels_per_point: 1.0 };
        use eframe::egui_wgpu::CallbackTrait;
        let extra = paint.prepare(&device, &queue, &screen, &mut encoder, &mut resources);

        // the drawn texture is copied into a buffer that can be mapped and read
        let tex = crate::viewport_gpu::color_texture_for_test(&resources)?;
        let row = (size_px[0] * 4).div_ceil(256) * 256; // the copy wants rows aligned to 256 bytes
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qym_eye_read"),
            size: (row * size_px[1]) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size_px[1]) } },
            wgpu::Extent3d { width: size_px[0], height: size_px[1], depth_or_array_layers: 1 },
        );
        queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range();
        let mut img = egui::ColorImage::new([size_px[0] as usize, size_px[1] as usize], vec![egui::Color32::TRANSPARENT; (size_px[0] * size_px[1]) as usize]);
        for y in 0..size_px[1] as usize {
            for x in 0..size_px[0] as usize {
                let o = y * row as usize + x * 4;
                img.pixels[y * size_px[0] as usize + x] = egui::Color32::from_rgba_premultiplied(data[o], data[o + 1], data[o + 2], data[o + 3]);
            }
        }
        drop(data);
        buf.unmap();
        Some(img)
    }
}
