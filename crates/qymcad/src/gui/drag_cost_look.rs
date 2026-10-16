//! WHAT A FRAME COSTS WHILE SOMETHING IS BEING DRAGGED - and what it does NOT do.
//!
//! A coarser draft accuracy during a drag pays only if meshing takes a noticeable share of a step - a fifth or
//! more; below that there is nothing to gain from it.
//!
//! THE ANSWER, measured here: a drag builds no geometry at all. A corner of a sketch under the hand solves
//! the sketch and redraws; the body is rebuilt when the hand lets go. The arrow of a feature's size writes a
//! number and draws an outline; the body is built on Enter. Twenty steps of each, and the geometry changed on
//! none of them.
//!
//! So the two checks below guard that finding rather than measuring a share: the day a drag starts rebuilding
//! the document, meshing appears under the hand and a draft accuracy becomes worth having - and that day this
//! goes red.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))
    }

    /// A part of middling weight: a plate with a pocket and a round boss, built from two sketches.
    fn a_part_of_middling_weight() -> (App, usize) {
        let mut app = App::default();
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part("Part");
        app.enter_component(part);
        // THE BASE SKETCH IS DRAWN BY HAND, with the rectangle tool: the points then behave as a person's do
        // (a rectangle built by the model layer comes out with the corner constrained, and the drag below
        // would measure an idle frame).
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(2).click2d(0.0, 0.0).click2d(40.0, 30.0);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let sid = app.project.sketches[si].id;
        let profiles: Vec<qymcad_core::model::Id> = app.project.sketches[si].contour_ids.clone();
        let body = app.project.add_extrude_multi(sid, profiles, 20.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());

        // a pocket and a boss, so the timeline is not one node long
        let si2 = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si2, 6.0, 6.0, 34.0, 24.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si2);
        app.finish_sketch_edit();
        let sid2 = app.project.sketches[si2].id;
        let cut = app.project.add_combine(body, sid2, 10.0, 0);
        let si3 = app.create_sketch_on(SketchPlane::default());
        app.project.add_circle_entity(si3, 20.0, 15.0, 6.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si3);
        app.finish_sketch_edit();
        let sid3 = app.project.sketches[si3].id;
        let boss = app.project.add_combine(cut, sid3, 30.0, 1);
        app.project.finish_base_body(boss, 1);
        qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
        (app, si)
    }

    /// The milliseconds a step of the drag costs, split into what it was spent on.
    #[derive(Default)]
    struct Step {
        solve: f64,
        rebuild: f64,
        mesh: f64,
        scene: f64,
        draw: f64,
    }

    /// THE OTHER CONTINUOUS ACTION: the arrow of a feature's size, pulled in 3D.
    ///
    /// The command is open, its height changes with the cursor, and the question is the same - does the frame
    /// build geometry through the kernel, or does it draw an outline and build on Enter.
    #[test]
    fn pulling_a_size_builds_no_geometry_until_enter() {
        let (mut app, _si) = a_part_of_middling_weight();
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 5.0;
        app.viewing.cam.target = [20.0, 15.0, 10.0];
        // a new extrude is taken up on the base sketch, as the tool bar does it
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, 4.0, 4.0, 20.0, 16.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);

        let mut changed = 0usize;
        let mut per_step = 0.0;
        for k in 0..20 {
            let before: Vec<(usize, i64)> = app.project.bodies.iter().map(|b| (b.mesh.tris.len(), (b.mesh.volume() * 1e3) as i64)).collect();
            let t = std::time::Instant::now();
            // exactly what the arrow does with the cursor: it writes the height into the command
            if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 5.0 + k as f64 * 0.5;
                p.txt = format!("{:.2}", p.val);
            }
            qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
            let scene = crate::gui::render_scene::gpu_scene(&app.painting());
            let basis = app.viewing.cam.basis();
            let _ = qymcad_render::rasterize_3d(&app.painting(), rect(), &basis, 1.0, 1.0);
            per_step += t.elapsed().as_secs_f64() * 1000.0;
            assert!(!scene.pieces.is_empty(), "setup: the scene came out empty");
            let after: Vec<(usize, i64)> = app.project.bodies.iter().map(|b| (b.mesh.tris.len(), (b.mesh.volume() * 1e3) as i64)).collect();
            if before != after {
                changed += 1;
            }
        }
        eprintln!("MEASURED twenty steps of pulling a size\n  the step {:.1} ms, and the geometry really changed on {changed} steps out of 20", per_step / 20.0);
        assert_eq!(changed, 0, "pulling a size rebuilt the geometry {changed} times out of 20 - then a draft accuracy for the duration of the pull is worth having");
    }

    #[test]
    fn dragging_a_corner_builds_no_geometry_until_the_hand_lets_go() {
        let (mut app, si) = a_part_of_middling_weight();
        assert!(!app.project.bodies.is_empty(), "setup: the part was not built");
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 5.0;
        app.viewing.cam.target = [20.0, 15.0, 10.0];
        // THE SKETCH IS OPENED BY ITS OWN DOOR, as a person opens it to drag a corner. Writing the session in by
        // hand left the rectangle tool of the drawing in hand, and in the window a drag with a tool in hand moves
        // the view, not the corner: measured, the view went to (-1.3, -1.3) and the corner stayed.
        app.enter_sketch_edit(si);

        // THE DRAG HAS TO REALLY MOVE THE CORNER, or the measurement is of an idle frame. The corner of the
        // rectangle is at (40, 30); the canvas of the hand shows the origin at scale 6, so it is in the frame.
        // SNAPPING OFF: with the grid on, a step smaller than the grid snaps back and the drag measures
        // nothing. A person dragging freely has it off too.
        app.set.snap.on = false;
        let corner_before = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
        Hand::canvas(&mut app).drag2d((40.0, 30.0), (42.0, 32.0));
        let corner_after = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
        assert_ne!(corner_before, corner_after, "setup: the drag moved nothing - the corner was not under the cursor");
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());

        let mut steps: Vec<Step> = Vec::new();
        let mut rebuilt_steps = 0usize;
        for k in 0..20 {
            let from = (42.0 + k as f64 * 0.5, 32.0 + k as f64 * 0.5);
            let to = (42.0 + (k + 1) as f64 * 0.5, 32.0 + (k + 1) as f64 * 0.5);
            let mut s = Step::default();

            let t = std::time::Instant::now();
            Hand::canvas(&mut app).drag2d(from, to);
            s.solve = t.elapsed().as_secs_f64() * 1000.0;

            // WAS THE BODY REBUILT AT ALL on this step, or only the sketch solved? The whole question of the
            // plan hangs on it: there is nothing to mesh more coarsely if nothing is meshed.
            let before: Vec<(usize, i64)> = app.project.bodies.iter().map(|b| (b.mesh.tris.len(), (b.mesh.volume() * 1e3) as i64)).collect();
            let t = std::time::Instant::now();
            qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
            s.rebuild = t.elapsed().as_secs_f64() * 1000.0;
            let after: Vec<(usize, i64)> = app.project.bodies.iter().map(|b| (b.mesh.tris.len(), (b.mesh.volume() * 1e3) as i64)).collect();
            if before != after {
                rebuilt_steps += 1;
            }

            // THE SHARE OF THE MESHING: the kernel builds the shape and meshes it in one call, so the meshing
            // is measured by asking it for the same mesh again at the same accuracy.
            let t = std::time::Instant::now();
            let k_ = app.project.geom_quality.deflection_k();
            let mut meshed = 0usize;
            for shape in app.live.shapes.values() {
                meshed += qymcad_kernel::specimen::mesh_auto(shape, k_).len();
            }
            s.mesh = t.elapsed().as_secs_f64() * 1000.0;
            assert!(meshed > 0, "setup: there is no live B-rep to mesh, so the share cannot be measured");

            let t = std::time::Instant::now();
            let scene = crate::gui::render_scene::gpu_scene(&app.painting());
            s.scene = t.elapsed().as_secs_f64() * 1000.0;
            assert!(!scene.pieces.is_empty(), "setup: the scene came out empty");

            let basis = app.viewing.cam.basis();
            let t = std::time::Instant::now();
            let _ = qymcad_render::rasterize_3d(&app.painting(), rect(), &basis, 1.0, 1.0);
            s.draw = t.elapsed().as_secs_f64() * 1000.0;

            steps.push(s);
        }

        let avg = |f: &dyn Fn(&Step) -> f64| steps.iter().map(f).sum::<f64>() / steps.len() as f64;
        let (solve, rebuild, mesh, scene, draw) = (avg(&|s| s.solve), avg(&|s| s.rebuild), avg(&|s| s.mesh), avg(&|s| s.scene), avg(&|s| s.draw));
        let whole = solve + rebuild + scene + draw;
        eprintln!(
            "MEASURED a drag of twenty steps on a part of three features\n  \
             per step: the drag and the solver {solve:.1} ms, the rebuild {rebuild:.1} ms, the scene {scene:.1} ms, the drawing {draw:.1} ms; the step {whole:.1} ms\n  \
             of the rebuild, the meshing is {mesh:.1} ms - that is {:.0} % of the step\n  \
             bodies {}, triangles {}; the body really changed on {rebuilt_steps} steps out of 20",
            mesh / whole.max(0.001) * 100.0,
            app.project.bodies.len(),
            app.project.bodies.iter().map(|b| b.mesh.tris.len()).sum::<usize>(),
        );
        assert_eq!(rebuilt_steps, 0, "a drag rebuilt the body {rebuilt_steps} times out of 20 - then meshing happens under the hand, and a draft accuracy is worth having");
    }
}
