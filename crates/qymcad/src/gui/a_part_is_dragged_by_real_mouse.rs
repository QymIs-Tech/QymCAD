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

    /// HOVER, PRESS `button`, LEAD `steps` x 14 px TO THE RIGHT, RELEASE - with Shift held through it all when `shift`,
    /// exactly as a hand does, one frame for each.
    fn lead(app: &mut App, ctx: &egui::Context, at: egui::Pos2, button: egui::PointerButton, shift: bool, steps: usize) {
        lead_with(app, ctx, at, button, egui::Modifiers { shift, ..Default::default() }, steps);
    }

    fn lead_with(app: &mut App, ctx: &egui::Context, at: egui::Pos2, button: egui::PointerButton, modifiers: egui::Modifiers, steps: usize) {
        let run = |app: &mut App, events: Vec<egui::Event>| {
            let _ = ctx.run_ui(egui::RawInput { modifiers, ..frame(events) }, |c| app.viewport(c));
        };
        run(app, vec![egui::Event::PointerMoved(at)]);
        run(app, vec![egui::Event::PointerButton { pos: at, button, pressed: true, modifiers }]);
        for k in 1..=steps {
            run(app, vec![egui::Event::PointerMoved(at + egui::vec2(14.0 * k as f32, 0.0))]);
        }
        let end = at + egui::vec2(14.0 * steps as f32, 0.0);
        run(app, vec![egui::Event::PointerButton { pos: end, button, pressed: false, modifiers }]);
    }

    fn origin_of(app: &App, comp: Id) -> [f64; 3] {
        qymcad_core::feature::apply12(&app.project.world_transform(comp), [0.0, 0.0, 0.0])
    }

    /// The point on the body the click will land on: the centre of the topmost face.
    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&app.active_path, &app.project));
        let f = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))).expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    /// TWO PARTS, A SLIDER BETWEEN THEM, LOOKED AT FROM THE ROOT. Returns (body of the driven part,
    /// its component).
    fn a_slider_pair(app: &mut App) -> (Id, Id) {
        let driven = a_joined_pair(app, JointKind::Slider, AnchorRef::BasePlane(BasePlane::YZ));
        (driven.body, driven.comp)
    }

    struct Driven {
        body: Id,
        comp: Id,
    }

    fn a_joined_pair(app: &mut App, kind: JointKind, anchor: AnchorRef) -> Driven {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        while qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) != root {
            app.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own");
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner")).collect();
        app.project.set_grounded(comps[0], true);
        let ca = app.project.add_connector(comps[0], anchor.clone());
        let cb = app.project.add_connector(comps[1], anchor);
        app.project.add_joint(ca, cb, kind);
        app.project.solve_joints();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());

        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [30.0, 10.0, 5.0];
        app.workbench = super::super::Workbench::Assembly;
        Driven { body: mine[1], comp: comps[1] }
    }

    /// SHIFT AND THE LEFT BUTTON WERE HELD ON THE PART AND LED — AND THE PART MOVED.
    #[test]
    fn holding_the_button_on_a_part_and_moving_actually_moves_it() {
        let mut app = App::default();
        let (body, comp) = a_slider_pair(&mut app);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));

        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
        let was = origin_of(&app, comp);

        // SHIFT AND THE LEFT BUTTON ON THE PART - the one gesture that takes a part without its gizmo
        lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 6);

        let now = origin_of(&app, comp);
        let moved = ((now[0] - was[0]).powi(2) + (now[1] - was[1]).powi(2) + (now[2] - was[2]).powi(2)).sqrt();
        assert!(moved > 1.0, "the button was held on the part and led — and it moved by {moved:.3} mm ({was:?} -> {now:?})");
        assert!(!qymcad_assembly::joint_drag_active(&app.side.joint, &app.dragged.part_pull), "the button was released — the hand must let go");
    }

    /// WHILE A TOOL PICKS FACES, THE HAND DOES NOT MOVE PARTS. Width, tangent, group, grounding, relation and
    /// connector tools all read clicks on parts; a drag across one turns the view instead, as it does while the
    /// anchors of a mate are picked. Reported behaviour: with the width tool on, a drag meant to turn the view to the
    /// far wall carried the first part off by 25 and 30.
    #[test]
    fn a_tool_that_picks_faces_leaves_the_parts_where_they_stand() {
        for tool in 0..6 {
            let mut app = App::default();
            let (body, comp) = a_slider_pair(&mut app);
            match tool {
                0 => app.start_width_pick(),
                1 => app.start_tangent_pick(),
                2 => app.start_group_pick(),
                3 => app.start_ground_pick(),
                4 => app.start_relation_pick(),
                _ => app.start_conn_pick(),
            }
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
            let basis = app.viewing.cam.basis();
            let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
            let was = origin_of(&app, comp);
            lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 6);
            let now = origin_of(&app, comp);
            assert!(now == was, "tool {tool} picks faces, a drag went across the part and carried it from {was:?} to {now:?}");
        }
    }

    /// A FRAME DRAWN FROM EMPTY SPACE TAKES THE PARTS IT ENCLOSES and turns nothing. Reported behaviour: a frame
    /// drawn round two parts took nothing - the drag turned the view.
    #[test]
    fn a_frame_from_empty_space_takes_the_parts_it_encloses() {
        let mut app = App::default();
        let _ = a_slider_pair(&mut app);
        app.viewing.cam.scale = 3.0; // both parts well inside the canvas
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
        let (yaw, pitch) = (app.viewing.cam.yaw, app.viewing.cam.pitch);
        let canvas = app.viewing.view_rect;
        let (from, to) = (canvas.min + egui::vec2(6.0, 6.0), canvas.max - egui::vec2(6.0, 6.0));
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
        let _ = ctx.run_ui(frame(vec![press(from, true)]), |c| app.viewport(c));
        for k in 1..=8 {
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0))]), |c| app.viewport(c));
        }
        let _ = ctx.run_ui(frame(vec![press(to, false)]), |c| app.viewport(c));
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
        assert!(app.viewing.cam.yaw == yaw && app.viewing.cam.pitch == pitch, "the frame turned the view");
        assert!(app.chosen.tree_sel.multi.len() == 2, "a frame round both parts took {:?}", app.chosen.tree_sel.multi);
        assert!(matches!(app.chosen.sel, qymcad_ui_state::Sel::Component(_)), "the frame chose no part");
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
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        assert_eq!(app.project.joints.iter().find(|j| j.id == jid).and_then(|j| j.driven(1)), Some(12.0), "setup: the value must be set");

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
        let was = origin_of(&app, comp);

        lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 6);

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
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));

        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
        lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 2);

        assert!(!qymcad_assembly::joint_drag_active(&app.side.joint, &app.dragged.part_pull), "after the release the hand stayed busy: the next drag will go to the part instead of the view");
    }

    /// THE MIDDLE AND THE RIGHT BUTTON, AND THE LEFT WITHOUT SHIFT, TURN THE VIEW over a part and leave it where it
    /// stands. Reported behaviour: with the cursor on a part the middle and the right button carried the part, not the
    /// view; the view is to turn as it always did, and a part is taken with Shift and the left button.
    #[test]
    fn the_buttons_without_shift_on_a_part_turn_the_view() {
        for button in [egui::PointerButton::Middle, egui::PointerButton::Secondary, egui::PointerButton::Primary] {
            let mut app = App::default();
            let (body, comp) = a_slider_pair(&mut app);
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
            let basis = app.viewing.cam.basis();
            let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
            let (was, yaw) = (origin_of(&app, comp), app.viewing.cam.yaw);
            lead(&mut app, &ctx, at, button, false, 6);
            let now = origin_of(&app, comp);
            assert!(now == was, "{button:?} led across the part and carried it from {was:?} to {now:?}");
            assert!(app.viewing.cam.yaw != yaw, "{button:?} led across the part and the view did not turn");
        }
    }

    /// EVERY LAYOUT OF THE MOUSE KEEPS ITS VIEW OVER A PART: the turn and the move of each, begun on a part with its
    /// own one button, move the view and leave the part (a chord of two buttons, or a movement with no button, is
    /// passed over). Ours alone takes the part with Shift and the left button; every other layout is the one of its
    /// own program, which has no such gesture, and leaves the part where it stands.
    #[test]
    fn every_layout_keeps_its_view_over_a_part_and_takes_it_with_shift() {
        let mut problems = Vec::new();
        for nav in qymcad_ui_state::MouseNav::ALL {
            for (what, g) in [("turn", nav.rotate()), ("move", nav.pan())] {
                let button = if g.any_button {
                    egui::PointerButton::Middle
                } else if g.buttons.len() == 1 {
                    g.buttons[0]
                } else {
                    continue;
                };
                if g.shift && button == egui::PointerButton::Primary {
                    continue; // the part's own gesture, begun on a part (ours: Shift and any button moves the view)
                }
                let mut app = App::default();
                app.set.mouse_nav = nav;
                let (body, comp) = a_slider_pair(&mut app);
                let ctx = egui::Context::default();
                super::super::install_fonts(&ctx);
                let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
                let basis = app.viewing.cam.basis();
                let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
                let (was, cam) = (origin_of(&app, comp), (app.viewing.cam.yaw, app.viewing.cam.pitch, app.viewing.cam.target));
                let modifiers = egui::Modifiers { shift: g.shift, ctrl: g.ctrl, command: g.ctrl, alt: g.alt, ..Default::default() };
                lead_with(&mut app, &ctx, at, button, modifiers, 6);
                if origin_of(&app, comp) != was {
                    problems.push(format!("{nav:?}: the {what} of the view with {button:?} begun on a part carried the part"));
                }
                if (app.viewing.cam.yaw, app.viewing.cam.pitch, app.viewing.cam.target) == cam {
                    problems.push(format!("{nav:?}: the {what} of the view with {button:?} begun on a part did not move the view"));
                }
            }
            let mut app = App::default();
            app.set.mouse_nav = nav;
            let (body, comp) = a_slider_pair(&mut app);
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
            let basis = app.viewing.cam.basis();
            let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
            let was = origin_of(&app, comp);
            lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 6);
            let moved = origin_of(&app, comp) != was;
            if moved != (nav == qymcad_ui_state::MouseNav::QymCad) {
                problems.push(format!("{nav:?}: Shift and the left button on a part {} it", if moved { "took" } else { "did not take" }));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn a_part_hinged_at_its_origin_is_turned_by_the_hand() {
        let mut app = App::default();
        let Driven { body, comp } = a_joined_pair(&mut app, JointKind::Revolute, AnchorRef::Origin);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
        let basis = app.viewing.cam.basis();
        let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(aim(&app, body)).0;
        let turn_of = |app: &App| {
            let (o, x) = (origin_of(app, comp), qymcad_core::feature::apply12(&app.project.world_transform(comp), [1.0, 0.0, 0.0]));
            (x[1] - o[1]).atan2(x[0] - o[0]).to_degrees()
        };
        let (pivot, was) = (origin_of(&app, comp), turn_of(&app));

        lead(&mut app, &ctx, at, egui::PointerButton::Primary, true, 6);

        let turned = (turn_of(&app) - was).abs();
        assert!(turned > 3.0, "the hand led a part hinged at its origin across the screen and it turned by {turned:.3} deg");
        let now = origin_of(&app, comp);
        let drift = ((now[0] - pivot[0]).powi(2) + (now[1] - pivot[1]).powi(2) + (now[2] - pivot[2]).powi(2)).sqrt();
        assert!(drift < 1e-6, "the hinge came apart under the hand: the pivot moved {drift:.3e} mm");
        assert!(!app.project.mates_conflict, "the drag left the hinge in conflict");
        assert!(!qymcad_assembly::joint_drag_active(&app.side.joint, &app.dragged.part_pull), "the button was released - the hand must let go");
    }
}
