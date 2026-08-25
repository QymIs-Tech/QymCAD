//! MOVING A PART DOES NOT REBUILD ITS CHUNK OF THE SCENE BUFFER.
//!
//! Reported behaviour: a joint moves as if on a slack rubber band; the joint glyphs and the thin blue
//! lines can be seen stretching. The glyph is drawn from live numbers while the body arrives a frame
//! late — so what costs is not the solver (0.3 ms) but assembling the scene buffer.
//!
//! WHY THIS IS MEASURED BY A COUNT OF BLOCKS AND NOT BY A STOPWATCH. A stopwatch on a debug build
//! lies by whole multiples and depends on the machine; a check by time would be green one run and red
//! the next without a single edit. But WHAT EXACTLY the program did — rebuilt a chunk or shifted a
//! ready one — is an exact number and does not depend on the machine. Measurement on a real assembly
//! confirms the link between that number and the cost: 63 rebuilds = 81-100 ms, 63 shifts =
//! 13-14 ms.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, BasePlane, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    fn frame(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { screen_rect: Some(viewport()), events, ..Default::default() }
    }

    fn press(at: egui::Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }

    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    /// Three parts, a slider between the first and the second: lead it and one moves while the rest
    /// stand still.
    fn a_slider_scene(app: &mut App) -> Id {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for k in 0..3 {
            super::super::joint_flow::tests::add_part_at(app, k as f64 * 60.0);
        }
        let root = app.project.root;
        while app.current_ctx_id_for_test() != root {
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner")).collect();
        app.project.set_grounded(comps[0], true);
        // THE BASE STANDS AT AN ANGLE — AND THAT IS NOT SCENE DECORATION.
        //
        // In an axis-aligned assembly the rotation of the driven part comes out of the solver bit for
        // bit identical, and the check would go green with ANY comparison threshold — that is, it
        // would check nothing. At an angle the solver derives the rotation afresh every frame, and it
        // honestly breathes by 1e-12 to 1e-9 — exactly as on the reported assembly, where that
        // breathing meant the fast path was never taken once.
        let (c0, s0) = (20.0f64.to_radians().cos(), 20.0f64.to_radians().sin());
        app.project.set_component_transform(comps[0], [c0, -s0, 0.0, 0.0, s0, c0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let ca = app.project.add_connector(comps[0], AnchorRef::BasePlane(BasePlane::YZ));
        let cb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::YZ));
        app.project.add_joint(ca, cb, JointKind::Slider);
        // A SECOND LINK ON ANOTHER AXIS: the solver has something to compute every frame, and its
        // output honestly breathes.
        let kb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::XY));
        let kc = app.project.add_connector(comps[2], AnchorRef::BasePlane(BasePlane::XY));
        app.project.add_joint(kb, kc, JointKind::Slider);
        app.project.solve_joints();
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 5.0;
        app.cam.target = [60.0, 10.0, 5.0];
        app.workbench = super::super::Workbench::Assembly;
        mine[2]
    }

    /// LEAD A PART AND THE BLOCKS ARE SHIFTED RATHER THAN REBUILT.
    #[test]
    fn dragging_a_part_shifts_its_block_instead_of_rebuilding_it() {
        let mut app = App::default();
        let body = a_slider_scene(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c));
        let _ = app.gpu_scene_for_test(); // the first pass: there are no blocks yet, everything is built — that is legitimate

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(at, true)]), |c| app.viewport_for_test(c));

        // THE FIRST STEP IS NOT JUDGED: on it the part becomes selected, the highlight changes the
        // APPEARANCE of the body, and rebuilding its block is legitimate. What is judged is the
        // settled dragging — there only the place changes.
        let mut rebuilt = 0u32;
        let mut shifted = 0u32;
        for k in 1..=6 {
            let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at + egui::vec2(12.0 * k as f32, 0.0))]), |c| app.viewport_for_test(c));
            let _ = app.gpu_scene_for_test();
            if k >= 3 {
                let st = app.scene_stats_for_test();
                rebuilt += st[0];
                shifted += st[1];
            }
        }
        let _ = ctx.run(frame(vec![press(at + egui::vec2(72.0, 0.0), false)]), |c| app.viewport_for_test(c));

        assert!(shifted > 0, "not one shift: the part was led and the buffer never learned of it — so the wrong thing is being measured");
        assert_eq!(rebuilt, 0, "moving the part rebuilt {rebuilt} chunks of the buffer: on a real assembly that is 63 chunks and 80 ms per frame instead of 13");
    }

    /// THE "IS THIS THE SAME ROTATION" THRESHOLD — SEPARATELY AND EXACTLY.
    ///
    /// A synthetic scene cannot check it: in an axis-aligned assembly the solver derives the rotation
    /// bit for bit identical, and the dragging check would go green with ANY threshold. On a real
    /// assembly it breathes by 1e-12 to 9e-10 (measured while dragging), and with a threshold of
    /// 1e-12 the fast path was NEVER taken: the fix was written and did not work. So the contract is
    /// pinned down here, in numbers.
    #[test]
    fn the_rotation_threshold_ignores_solver_noise_but_not_a_real_turn() {
        let id = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut moved = id;
        moved[3] = 137.5; // moved 137.5 mm — a translation is of no concern to the threshold
        moved[11] = -12.0;
        assert!(super::super::same_rotation12(&id, &moved), "a translation is not a turn: the block must be shifted rather than rebuilt");

        let mut noisy = moved;
        noisy[0] -= 9e-10; // exactly the solver noise measured on the reported assembly
        noisy[4] += 4e-12;
        assert!(super::super::same_rotation12(&moved, &noisy), "solver noise was taken for a turn — the fast path will never be taken");

        let (c, s) = (0.1f64.cos(), 0.1f64.sin()); // 5.7 deg — a real drag of a hinge
        let turned = [c, -s, 0.0, moved[3], s, c, 0.0, 0.0, 0.0, 0.0, 1.0, moved[11]];
        assert!(!super::super::same_rotation12(&moved, &turned), "a real turn was taken for noise — the part will be shown with somebody else's shading");
    }

    /// AND A TURN IS REBUILT HONESTLY. The world normals became different, and both the vertex
    /// colour and the back-face culling are computed from them: there is nothing to shift here, and
    /// pretending otherwise means showing the part with somebody else's shading.
    #[test]
    fn a_turned_part_is_rebuilt_honestly() {
        let mut app = App::default();
        let body = a_slider_scene(&mut app);
        let comp = app.project.body_owner(body).expect("the owner");
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c));
        let _ = app.gpu_scene_for_test();
        let _ = app.gpu_scene_for_test();
        assert_eq!(app.scene_stats_for_test()[0], 0, "setup: with no motion there is nothing to rebuild");

        // turn the part 30 deg about Z — by the same means the document turns it
        let m = app.project.component_transform(comp);
        let (c, s) = (30.0f64.to_radians().cos(), 30.0f64.to_radians().sin());
        let r = [c, -s, 0.0, m[3], s, c, 0.0, m[7], 0.0, 0.0, 1.0, m[11]];
        app.project.set_component_transform(comp, r);
        app.rebuild_if_dirty();
        let _ = app.gpu_scene_for_test();
        assert!(app.scene_stats_for_test()[0] > 0, "the part was turned and the block was not rebuilt: the shading is left over from the previous turn");
    }
}
