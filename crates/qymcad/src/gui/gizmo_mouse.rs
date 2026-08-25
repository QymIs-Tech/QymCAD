//! THE GIZMO UNDER A REAL MOUSE.
//!
//! The audit itself named this as uncovered: what a run does not touch is the mouse — dragging the
//! gizmo, the rubber band, dragging a dimension. A snapshot is useless there, and a different kind of
//! scenario is needed, one with pointer events.
//!
//! The reason the hole held out so long is plain: grabbing a handle and leading a drag live on
//! `egui::Response`, and faking it means checking the fake. So here there is a real frame: the canvas
//! lays out by the same call as in a live window, and a press, a movement and a release are fed into
//! it — exactly what a hand does.
#[cfg(test)]
mod tests {
    use super::super::App;

    const SCREEN: egui::Vec2 = egui::vec2(900.0, 700.0);

    fn frame(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            events,
            ..Default::default()
        }
    }

    fn press(at: egui::Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }

    /// An assembly with one part, selected the way a person selects it — with the gizmo on screen.
    fn assembly_with_selected_part() -> (App, egui::Context, u64) {
        let mut app = App::default();
        let comp = super::super::comp_array_flow::tests::assembly_with_part(&mut app);
        app.mode_3d = true; // the gizmo and the orbit live in the THREE-DIMENSIONAL canvas; by default the window opens in 2D
        // THE WORKBENCH IS DERIVED FROM THE CONTEXT, but it is the application frame that does it,
        // not the canvas. The component gizmo is only given in the Assembly — without this line it
        // will not be there however carefully one aims.
        app.sync_workbench();
        let ci = app.project.components.iter().position(|c| c.id == comp).expect("the component in the list");
        app.sel = super::super::Sel::Component(ci);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(frame(Vec::new()), |c| app.viewport_for_test(c)); // lay out the canvas and learn its rectangle
        (app, ctx, comp)
    }

    /// THE GIZMO HANDLE CAN BE GRABBED WITH THE MOUSE, AND THE PART FOLLOWS IT.
    ///
    /// THE EARLIER FAILURE WAS IN THE SETUP, NOT IN THE PROGRAM, and that is worth saying plainly. The
    /// component gizmo is only given in the Assembly workbench (`gizmo_component`), and the workbench
    /// is derived from the context by the APPLICATION FRAME, which the check did not have: the canvas
    /// does not move it by itself. The marker stood honestly — but what was found was an unfinished
    /// check, not a defect in the program.
    #[test]
    fn dragging_the_component_gizmo_moves_the_part() {
        let (mut app, ctx, comp) = assembly_with_selected_part();
        let before = app.project.component_transform(comp);

        // where the X-axis handle lies on screen: the tip of the arrow from the gizmo origin
        let rect = app.view_rect_for_test();
        assert!(rect.is_positive(), "setup: the canvas did not lay out");
        let basis = app.cam.basis();
        let (o, l) = app.gizmo_geometry_for_test(comp);
        let mut tip = o;
        tip[0] += l;
        let at_origin = app.project3(o, rect, &basis).0;
        let at_tip = app.project3(tip, rect, &basis).0;
        let grab = at_origin + (at_tip - at_origin) * 0.75; // along the arrow, not right at its tip

        // PRESS AND LEAD. The handle is grabbed NOT on the press: `egui` declares a drag only after
        // accumulated movement, and the decision about what was grabbed is taken at exactly that
        // moment.
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(grab)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(grab, true)]), |c| app.viewport_for_test(c));

        let to = grab + (at_tip - at_origin) * 0.3;
        let mut grabbed = None;
        for k in 1..=4 {
            let p = grab + (to - grab) * (k as f32 / 4.0);
            let _ = ctx.run(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport_for_test(c));
            grabbed = grabbed.or(app.comp_giz.axis);
        }
        assert_eq!(
            grabbed,
            Some(0),
            "the mouse was led along the X arrow and the gizmo did not grab it; without that a drag turns into a camera rotation"
        );
        let _ = ctx.run(frame(vec![press(to, false)]), |c| app.viewport_for_test(c));

        let after = app.project.component_transform(comp);
        let moved = [after[3] - before[3], after[7] - before[7], after[11] - before[11]];
        assert!(moved[0].abs() > 1e-6, "the part did not follow the mouse: shift {moved:?}");
        assert!(
            moved[1].abs() < 1e-6 && moved[2].abs() < 1e-6,
            "the drag went along the X axis and the part went sideways: shift {moved:?} — a drag must keep to the grabbed axis"
        );
        assert!(app.comp_giz.axis.is_none() && app.comp_giz.drag.is_none(), "after the release the grab must be let go, otherwise the next click will drag the part");
    }

    /// AWAY FROM THE HANDLE IS THE CAMERA, NOT THE PART. Otherwise any turn of the view would move
    /// the model.
    #[test]
    fn dragging_away_from_the_handle_turns_the_camera_and_leaves_the_part_alone() {
        let (mut app, ctx, comp) = assembly_with_selected_part();
        let before = app.project.component_transform(comp);
        let yaw_before = app.cam.yaw;

        let rect = app.view_rect_for_test();
        let far = egui::pos2(rect.min.x + 12.0, rect.min.y + 12.0); // the corner of the canvas — far from the gizmo
        let _ = ctx.run(frame(vec![egui::Event::PointerMoved(far)]), |c| app.viewport_for_test(c));
        let _ = ctx.run(frame(vec![press(far, true)]), |c| app.viewport_for_test(c));
        assert!(app.comp_giz.axis.is_none(), "a click far from the gizmo grabbed a handle — then the view cannot be turned at all");

        let to = far + egui::vec2(60.0, 0.0);
        for k in 1..=4 {
            let p = far + egui::vec2(15.0 * k as f32, 0.0);
            let _ = ctx.run(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport_for_test(c));
        }
        let _ = ctx.run(frame(vec![press(to, false)]), |c| app.viewport_for_test(c));

        let after = app.project.component_transform(comp);
        assert_eq!(before, after, "a drag away from the gizmo moved the part — a person would be turning the view and silently breaking the assembly");
        assert!((app.cam.yaw - yaw_before).abs() > 1e-6, "a drag away from the gizmo must turn the view, and the view did not stir");
    }
}
