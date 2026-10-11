//! A SCENE SENT IN A DISCARDED PASS IS SENT AGAIN.
//!
//! Reported behaviour: on a Radeon RX 480 under DX12 the cube had no walls - its edges drawn, its faces not - until
//! drawing was switched to the processor and back. The window sends the vertices to the card only in the frame
//! where the scene changed and marks them sent at once. egui runs a second pass when one asks it to
//! (`request_discard`), and keeps the shapes of the last pass only: the paint callback carrying the vertices went
//! with the discarded pass, the next pass sent none, and the card kept nothing to draw. Switching to the processor
//! and back cleared the mark, and that was all the switch mended.
//!
//! The frame is run as the live window runs it - two passes, the first discarded - and what is left of it goes
//! through the real `egui_wgpu::Renderer`, which calls the callback's `prepare`; the picture is taken off the card.
#[cfg(test)]
mod tests {
    use crate::gui::App;
    use eframe::egui_wgpu;
    use eframe::wgpu;

    const SIZE: [u32; 2] = [320, 240];

    /// The first cube of a new document, the camera on it.
    fn the_cube() -> App {
        let mut app = App::default();
        app.project.cube_sample();
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        assert!(!app.project.bodies.is_empty(), "setup: the cube was not built");
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for (i, b) in app.project.bodies.iter().enumerate() {
            let Some(id) = app.project.mesh_id(i) else { continue };
            let wt = app.project.body_world_transform(id);
            for v in &b.mesh.verts {
                let q = qymcad_core::feature::apply12(&wt, [v.x, v.y, v.z]);
                for k in 0..3 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
            }
        }
        let across = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f64, f64::max).max(1e-6);
        app.viewing.cam.target = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        app.viewing.cam.scale = (SIZE[1] as f64 / 2.0 / across) as f32;
        app.viewing.cam.init = true;
        app.viewing.mode_3d = true;
        app
    }

    #[test]
    fn the_cube_keeps_its_faces_when_the_pass_that_sent_them_is_discarded() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            eprintln!("PASSED OVER: no graphics device to draw with");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
            eprintln!("PASSED OVER: the graphics device would not open");
            return;
        };
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, egui_wgpu::RendererOptions::default());
        crate::viewport_gpu::install_for_test(&device, &mut renderer.callback_resources, 1);

        let app = the_cube();
        let pn = app.painting();
        let basis = pn.cam.basis();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE[0] as f32, SIZE[1] as f32));
        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(rect), ..Default::default() };
        // THE FRAME OF THE LIVE WINDOW: the viewport drawn in every pass, and the first pass asked to be run again
        // - as a window opening, a table measuring its columns or a text laid out for the first time ask
        let out = ctx.run_ui(raw, |ui| {
            crate::gui::render::draw_3d_gpu(&pn, ui.painter(), rect, &basis);
            if ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard("a widget measured itself");
            }
        });
        assert_eq!(out.platform_output.num_completed_passes, 2, "setup: the frame did not run a second pass");

        let prims = ctx.tessellate(out.shapes, 1.0);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: SIZE, pixels_per_point: 1.0 };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("qym_discard_probe") });
        let extra = renderer.update_buffers(&device, &queue, &mut encoder, &prims, &screen);
        let Some(tex) = crate::viewport_gpu::color_texture_for_test(&renderer.callback_resources) else {
            panic!("the viewport's callback never reached the card: no picture was drawn");
        };
        let row = (SIZE[0] * 4).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qym_discard_read"),
            size: (row * SIZE[1]) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(SIZE[1]) } },
            wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 },
        );
        queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).expect("the card answers");
        let data = slice.get_mapped_range();
        let covered = (0..SIZE[1] as usize).flat_map(|y| (0..SIZE[0] as usize).map(move |x| y * row as usize + x * 4 + 3)).filter(|&o| data[o] > 128).count();
        // the cube fills a good part of the picture: a third of its height across, seen from three quarters
        assert!(covered > 2_000, "the cube's faces are not on the card after a discarded pass: {covered} pixels covered of {}", SIZE[0] * SIZE[1]);
    }
}
