//! THE WHEEL ZOOMS WHERE THE CURSOR POINTS.
//!
//! Reported behaviour: "zooming pans from the origin rather than from the coordinates of the cursor, both
//! in sketches and in the 3D viewport (two variants are needed, put them in the settings). And with a
//! tool's popup open, panning should go from the centre of the part (put that in the settings too)."
//!
//! WHERE IT WENT WRONG. Both viewports changed the SCALE and nothing else, which holds the middle of the
//! viewport still. Whatever a person is looking at slides away from the cursor as the view grows, so
//! reaching a corner means zooming and then panning back - every time.
//!
//! MEASURED BY WHERE A WORLD POINT LANDS, not by the camera's numbers: the whole question is whether the
//! thing under the cursor stays under the cursor, and that is its screen position.
#[cfg(test)]
mod tests {
    use crate::gui::App;
    use qymcad_ui_state::{ZoomAt, ZoomWhileEditing};

    const RECT: egui::Rect = egui::Rect { min: egui::Pos2 { x: 0.0, y: 0.0 }, max: egui::Pos2 { x: 900.0, y: 700.0 } };

    /// Where a world point lands in the 3D viewport right now.
    fn at3(app: &App, p: [f64; 3]) -> egui::Pos2 {
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: RECT, basis: &basis }.at(p).0
    }

    /// A part with a body, seen in 3D, with the camera fitted.
    fn a_part_in_view() -> App {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let cid = app.project.add_part("P");
        app.enter_component(cid);
        let c = app.project.add_cylinder(10.0, 20.0);
        app.project.finish_base_body(c, 1);
        app.exit_context();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app.viewing.mode_3d = true;
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, RECT);
        app
    }

    /// THE POINT UNDER THE CURSOR STAYS UNDER THE CURSOR when the 3D view is zoomed.
    #[test]
    fn zooming_in_3d_holds_the_point_under_the_cursor() {
        let mut app = a_part_in_view();
        app.set.zoom_at = ZoomAt::Cursor;
        let cursor = egui::pos2(700.0, 200.0); // well away from the middle, or the two rules agree
        // the world point that is under the cursor right now, taken in the plane the camera turns about
        let basis = app.viewing.cam.basis();
        let c = RECT.center();
        let (du, dv) = (((cursor.x - c.x) / app.viewing.cam.scale) as f64, (-(cursor.y - c.y) / app.viewing.cam.scale) as f64);
        let p = [
            app.viewing.cam.target[0] + basis.0[0] * du + basis.1[0] * dv,
            app.viewing.cam.target[1] + basis.0[1] * du + basis.1[1] * dv,
            app.viewing.cam.target[2] + basis.0[2] * du + basis.1[2] * dv,
        ];
        assert!((at3(&app, p) - cursor).length() < 0.01, "GUARD: that point must start under the cursor");

        qymcad_ui_state::zoom_cam_3d(&mut app.viewing.cam, RECT, qymcad_ui_state::zoom_anchor(&app.set, RECT, Some(cursor), None), 1.4, 0.05, 400.0);

        let moved = (at3(&app, p) - cursor).length();
        assert!(moved < 0.5, "the point under the cursor travelled {moved:.1} points away while zooming: the view grew from its middle, not from where the hand is pointing");
    }

    /// AND WITH THE OTHER SETTING IT IS THE MIDDLE OF THE VIEW THAT HOLDS.
    ///
    /// Two variants were asked for, so the second is checked too - otherwise the setting could be a
    /// dropdown that changes nothing.
    #[test]
    fn the_other_setting_holds_the_middle_of_the_view() {
        let mut app = a_part_in_view();
        app.set.zoom_at = ZoomAt::ViewCentre;
        let target = app.viewing.cam.target;
        let cursor = egui::pos2(700.0, 200.0);

        qymcad_ui_state::zoom_cam_3d(&mut app.viewing.cam, RECT, qymcad_ui_state::zoom_anchor(&app.set, RECT, Some(cursor), None), 1.4, 0.05, 400.0);

        assert_eq!(app.viewing.cam.target, target, "with \"from the middle of the view\" chosen the camera must not travel at all");
    }

    /// WHILE A COMMAND IS OPEN IT IS THE PART THAT HOLDS STILL.
    #[test]
    fn a_command_holds_its_own_geometry_still() {
        let mut app = a_part_in_view();
        app.set.zoom_at = ZoomAt::Cursor;
        app.set.zoom_editing = ZoomWhileEditing::PartCentre;
        let popup = egui::pos2(300.0, 500.0); // where the command's fields stand
        let cursor = egui::pos2(800.0, 100.0); // and where the hand happens to be

        let at = qymcad_ui_state::zoom_anchor(&app.set, RECT, Some(cursor), Some(popup));
        assert_eq!(at, popup, "with a command open the view must hold the part, not the cursor");

        app.set.zoom_editing = ZoomWhileEditing::AsUsual;
        let at = qymcad_ui_state::zoom_anchor(&app.set, RECT, Some(cursor), Some(popup));
        assert_eq!(at, cursor, "and with the exception switched off the ordinary rule applies");
    }

    /// THE FLAT VIEW OF A SKETCH DOES THE SAME.
    #[test]
    fn zooming_a_sketch_holds_the_point_under_the_cursor() {
        let mut view = qymcad_ui_state::View2d { center: egui::vec2(0.0, 0.0), scale: 4.0, initialized: true };
        let cursor = egui::pos2(700.0, 200.0);
        let before = qymcad_ui_state::to_world(&view, RECT, cursor);

        qymcad_ui_state::zoom_view_2d(&mut view, RECT, cursor, 1.4, 0.02, 800.0);

        let after = qymcad_ui_state::to_world(&view, RECT, cursor);
        let moved = ((before.x - after.x).powi(2) + (before.y - after.y).powi(2)).sqrt();
        assert!(moved < 1e-6, "the world point under the cursor changed by {moved:.4} mm while zooming the sketch");
    }

    /// AT THE LIMIT OF THE ZOOM NOTHING CREEPS.
    ///
    /// The scale is clamped, and a compensation computed from a change that did not happen would pan the
    /// view a little on every notch of the wheel - a view that slides away while the zoom stands still.
    #[test]
    fn at_the_limit_the_view_does_not_creep() {
        let mut view = qymcad_ui_state::View2d { center: egui::vec2(3.0, -2.0), scale: 800.0, initialized: true };
        let before = view.center;
        for _ in 0..5 {
            qymcad_ui_state::zoom_view_2d(&mut view, RECT, egui::pos2(700.0, 200.0), 1.4, 0.02, 800.0);
        }
        assert_eq!(view.center, before, "the view crept while the zoom stood at its limit");
    }

    /// AND THE WHEEL IN A REAL FRAME REALLY DOES IT.
    ///
    /// The checks above prove the arithmetic; this one proves the viewport asks for it. A rule computed in
    /// a function nobody calls is the same as no rule - that lesson has its own scar in this repository.
    #[test]
    fn the_wheel_over_the_viewport_zooms_towards_the_cursor() {
        let mut moved = Vec::new();
        for (at, expect_move) in [(ZoomAt::Cursor, true), (ZoomAt::ViewCentre, false)] {
            let mut app = a_part_in_view();
            app.set.zoom_at = at;
            let ctx = egui::Context::default();
            crate::gui::install_fonts(&ctx);
            app.waiting.splash_until = None;
            let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
            let frame = |app: &mut App, events: Vec<egui::Event>| {
                let raw = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
                let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
            };
            for _ in 0..3 {
                frame(&mut app, Vec::new()); // the panels settle and the viewport learns its rectangle
            }
            let before = app.viewing.cam.target;
            // the cursor sits well off the middle of the window, then the wheel turns under it
            let spot = app.viewing.view_rect.center() + egui::vec2(app.viewing.view_rect.width() * 0.3, -app.viewing.view_rect.height() * 0.3);
            frame(&mut app, vec![egui::Event::PointerMoved(spot)]);
            frame(&mut app, vec![egui::Event::PointerMoved(spot), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, 120.0), phase: egui::TouchPhase::Move, modifiers: Default::default() }]);
            let travelled = (0..3).map(|a| (app.viewing.cam.target[a] - before[a]).abs()).fold(0.0f64, f64::max);
            if (travelled > 1e-6) != expect_move {
                moved.push(format!("{at:?}: the camera travelled {travelled:.4} mm, expected {}", if expect_move { "it to follow the cursor" } else { "it to stay" }));
            }
        }
        assert!(moved.is_empty(), "the wheel over the viewport does not honour the setting:\n{}", moved.join("\n"));
    }
}
