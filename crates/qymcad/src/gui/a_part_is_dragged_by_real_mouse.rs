//! A PART IS DRAGGED WITH A REAL MOUSE — THROUGH THE FRAME PASS, NOT AROUND IT.
//!
//! Reported behaviour, and the reason this exists: motion along the mates does not work anywhere; hold
//! the mouse button down on a part and try to move it and nothing happens, only the gizmo handles
//! work.
//!
//! And that was right while the checks were green. Because the checks called `joint_grab_part_at` and
//! `joint_giz_drag_to` DIRECTLY, while the live application first asks the frame pass: what was
//! grabbed? The pass knew exactly one answer — the handle of the DOF gizmo (`joint.giz_drag`) — and a
//! pull on a part never reached the dragging at all: the drag went to the camera. The grab itself
//! honestly worked, so from the side of the kernel everything looked sound.
//!
//! HENCE THE RULE: a mouse path is checked BY THE SAME PATH a person walks it — a real canvas and a
//! real press-move-release. Going round the frame pass proves only that code called by hand can do
//! arithmetic.
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

    fn origin_of(app: &App, comp: Id) -> [f64; 3] {
        qymcad_core::feature::apply12(&app.project.world_transform(comp), [0.0, 0.0, 0.0])
    }

    /// The point on the body the click will land on: the centre of the topmost face.
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

    /// TWO PARTS, A SLIDER BETWEEN THEM, LOOKED AT FROM THE ROOT. Returns (body of the driven part,
    /// its component).
    fn a_slider_pair(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        while app.current_ctx_id_for_test() != root {
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own");
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner")).collect();
        app.project.set_grounded(comps[0], true);
        let ca = app.project.add_connector(comps[0], AnchorRef::BasePlane(BasePlane::YZ)); // normal X
        let cb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::YZ));
        app.project.add_joint(ca, cb, JointKind::Slider);
        app.project.solve_joints();
        app.rebuild_if_dirty();
        app.refresh_edges();

        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];
        app.workbench = super::super::Workbench::Assembly;
        (mine[1], comps[1])
    }

    /// THE BUTTON WAS HELD ON THE PART AND LED — AND THE PART MOVED.
    #[test]
    fn holding_the_button_on_a_part_and_moving_actually_moves_it() {
        let mut app = App::default();
        let (body, comp) = a_slider_pair(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c));

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let was = origin_of(&app, comp);

        // HOVER, PRESS, LEAD OVER SEVERAL FRAMES, RELEASE — exactly like a hand.
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(at, true)]), |c| app.viewport_for_test(c));
        for k in 1..=6 {
            let p = at + egui::vec2(14.0 * k as f32, 0.0);
            let _ = ctx.run(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport_for_test(c));
        }
        let _ = ctx.run(frame(vec![press(at + egui::vec2(84.0, 0.0), false)]), |c| app.viewport_for_test(c));

        let now = origin_of(&app, comp);
        let moved = ((now[0] - was[0]).powi(2) + (now[1] - was[1]).powi(2) + (now[2] - was[2]).powi(2)).sqrt();
        assert!(moved > 1.0, "the button was held on the part and led — and it moved by {moved:.3} mm ({was:?} -> {now:?})");
        assert!(!app.joint_drag_active(), "the button was released — the hand must let go");
    }

    /// THE JOINT HAS A VALUE SET — THE PART IS STILL DRAGGED, AND THE NUMBER FOLLOWS IT.
    ///
    /// That was the reported trouble, and measurement found it on the reported document: three
    /// sliders, all with a value set (-280, 7, 100) — the problem has ZERO FREEDOMS, and the pull
    /// along the null space honestly moved nothing. The gizmo handles worked meanwhile, because they
    /// edit the number itself. From the outside: motion along the mates does not work anywhere, only
    /// the gizmo handles do.
    ///
    /// The rule: the hand does not argue with the number — it leads the part, and the number is
    /// written to what was reached.
    #[test]
    fn a_specified_mate_value_does_not_block_the_hand() {
        let mut app = App::default();
        let (body, comp) = a_slider_pair(&mut app);
        // the travel of the slider was set as a number, as in the mates panel
        let jid = app.project.joints.last().map(|j| j.id).expect("the joint");
        app.project.joints.iter_mut().find(|j| j.id == jid).expect("the joint").drive[1] = Some(12.0);
        app.project.solve_joints();
        app.rebuild_if_dirty();
        assert_eq!(app.project.joints.iter().find(|j| j.id == jid).and_then(|j| j.driven(1)), Some(12.0), "setup: the value must be set");

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c));
        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let was = origin_of(&app, comp);

        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(at, true)]), |c| app.viewport_for_test(c));
        for k in 1..=6 {
            let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at + egui::vec2(14.0 * k as f32, 0.0))]), |c| app.viewport_for_test(c));
        }
        let _ = ctx.run(frame(vec![press(at + egui::vec2(84.0, 0.0), false)]), |c| app.viewport_for_test(c));

        let now = origin_of(&app, comp);
        let moved = ((now[0] - was[0]).powi(2) + (now[1] - was[1]).powi(2) + (now[2] - was[2]).powi(2)).sqrt();
        assert!(moved > 1.0, "the joint has a value set and the part stood dead still: it travelled {moved:.3} mm");
        // THE NUMBER MUST FOLLOW THE PART, otherwise the next solve drags it back to the old drive
        let after = app.project.joints.iter().find(|j| j.id == jid).and_then(|j| j.driven(1)).expect("the drive is there");
        assert!((after - 12.0).abs() > 0.5, "the part moved and the drive stayed at {after} — a solve without the hand will bring it back");
        // and it really stays where it was led: a solve WITHOUT the hand does not drag it back
        app.project.solve_joints();
        let settled = origin_of(&app, comp);
        let back = ((settled[0] - now[0]).powi(2) + (settled[1] - now[1]).powi(2) + (settled[2] - now[2]).powi(2)).sqrt();
        assert!(back < 0.01, "after the release the solve dragged the part {back:.3} mm from where the hand left it");
    }

    /// RELEASE AND THE VIEW TURNS AGAIN. The pull must not stay held down after a drag.
    #[test]
    fn after_the_drag_the_view_is_free_again() {
        let mut app = App::default();
        let (body, _comp) = a_slider_pair(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c));

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(at, true)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(at + egui::vec2(30.0, 0.0))]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(at + egui::vec2(30.0, 0.0), false)]), |c| app.viewport_for_test(c));

        assert!(!app.joint_drag_active(), "after the release the hand stayed busy: the next drag will go to the part instead of the view");
    }
}
