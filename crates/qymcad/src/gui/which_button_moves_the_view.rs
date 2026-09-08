//! WHICH BUTTON MOVES THE VIEW IS THE PERSON'S CHOICE.
//!
//! Reported behaviour: "add a way to choose how the mouse behaves in the settings: ours, or as in other
//! CADs, where the way it works in the 3D viewport and in sketches is picked from a list."
//!
//! It was written into the viewport: a drag turned the model, full stop. That is quick, and its price is
//! that a drag begun on the model is never a selection - which is exactly what somebody arriving from a
//! CAD where the left button only ever selects trips over on the first minute.
//!
//! A SET, NOT THREE SWITCHES. The combinations that are not sets are nonsense: "the left button turns the
//! view AND draws a selection box" is not a preference, it is a conflict. So the choice is one named set,
//! and the full list lives next to the type - a list written out in the settings window would fall behind
//! on the first set added.
//!
//! ONE ANSWER FOR THE TURN AND THE MOVE. They are the same gesture with a modifier; asking which button
//! navigates twice is how the two would come to disagree, and a view that turns with one button and moves
//! with another is a bug nobody can describe.
#[cfg(test)]
mod tests {
    use qymcad_ui_state::MouseNav;

    use super::super::App;

    const SCREEN: egui::Vec2 = egui::vec2(1200.0, 800.0);

    fn frame(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), events, ..Default::default() }
    }

    fn press(at: egui::Pos2, button: egui::PointerButton, down: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button, pressed: down, modifiers: Default::default() }
    }

    /// A part on screen, in the 3D canvas, ready to be dragged over.
    fn a_part_in_view(nav: MouseNav) -> (App, egui::Context) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        app.set.mouse_nav = nav;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.viewing.mode_3d = true;
        app.sync_workbench();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c)); // lay out the canvas
        (app, ctx)
    }

    /// Drag across the canvas with `button`, through real frames, and answer how far the view turned.
    fn yaw_after_a_drag(nav: MouseNav, button: egui::PointerButton) -> f64 {
        let (mut app, ctx) = a_part_in_view(nav);
        let before = app.viewing.cam.yaw;
        let from = egui::pos2(700.0, 400.0);
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
        let _ = ctx.run_ui(frame(vec![press(from, button, true)]), |c| app.viewport(c));
        for k in 1..=6 {
            let p = egui::pos2(from.x + k as f32 * 12.0, from.y);
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport(c));
        }
        let _ = ctx.run_ui(frame(vec![press(egui::pos2(from.x + 72.0, from.y), button, false)]), |c| app.viewport(c));
        (app.viewing.cam.yaw - before).abs()
    }

    /// OURS IS THE FACTORY LAYOUT, and a left drag turns the model under it.
    #[test]
    fn the_factory_layout_is_ours_and_a_left_drag_turns_the_model() {
        assert_eq!(qymcad_ui_state::Settings::default().mouse_nav, MouseNav::QymCad, "the factory layout must be ours: it is what people already have in their hands");
        let turned = yaw_after_a_drag(MouseNav::QymCad, egui::PointerButton::Primary);
        assert!(turned > 1e-6, "under our layout a left drag must turn the model - that is the behaviour people have now");
    }

    /// AND UNDER A LAYOUT WHOSE ROTATE IS NOT THE BARE LEFT BUTTON, a left drag leaves the view alone.
    ///
    /// Walked over `MouseNav::ALL` rather than over two names written here: a layout added to the type
    /// must be measured by this check without anybody remembering to add it (D19).
    #[test]
    fn a_left_drag_moves_the_view_only_where_the_layout_says_so() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            let bare_left = nav.rotate().takes_a_bare_left_drag();
            let turned = yaw_after_a_drag(nav, egui::PointerButton::Primary);
            if bare_left != (turned > 1e-6) {
                wrong.push(format!("{nav:?}: rotate is {:?}, and a bare left drag turned the view by {turned}", nav.rotate()));
            }
        }
        assert!(wrong.is_empty(), "a layout does not do what it declares, so the person picked a habit and got another ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// EVERY LAYOUT DECLARES A ROTATE AND A PAN, and they are not the same gesture.
    ///
    /// A layout whose two movements are one gesture is not a layout - it is a view that pans and turns at
    /// once, which nobody can aim. The transcription from another CAD is exactly where such a slip lands.
    #[test]
    fn no_layout_gives_the_same_gesture_to_turning_and_moving() {
        for nav in MouseNav::ALL {
            assert_ne!(nav.rotate(), qymcad_ui_state::Gesture::NONE, "{nav:?} declares no way to turn the model");
            assert_ne!(nav.pan(), qymcad_ui_state::Gesture::NONE, "{nav:?} declares no way to move the view");
            assert_ne!(nav.rotate(), nav.pan(), "{nav:?} turns and moves on the SAME gesture, so it does both at once and neither can be aimed");
        }
    }

    /// DRAGGING AN OPEN WINDOW DOES NOT DRAG THE CAMERA WITH IT.
    ///
    /// Reported behaviour: "now dragging an open window - Settings or Help - drags the viewport camera
    /// too." Two mistakes of mine met there. The gesture asked only whether a button was DOWN, and a window
    /// held by its title bar holds one; and the movement was taken from the raw pointer delta whenever egui
    /// reported no drag, which is exactly what a window drag looks like from the canvas.
    ///
    /// REPRODUCED THROUGH A REAL WINDOW, by its own door: a window is drawn over the canvas and dragged by
    /// its title bar, as a person drags it. A check that merely held a button somewhere passed even with
    /// the defect back in place - the canvas has to be COVERED for the case to exist at all.
    #[test]
    fn dragging_an_open_window_leaves_the_camera_alone() {
        // A LAYOUT WHOSE ROTATE NAMES A BUTTON: that is where the defect lived. Ours takes "whichever button
        // is dragging", and a window swallows the drag, so ours never showed it - the layouts that name the
        // left button did, because "the button is down" was true while a window was being held.
        let (mut app, ctx) = a_part_in_view(MouseNav::Gesture);
        let title = egui::pos2(500.0, 210.0); // on the window's title bar, over the canvas

        let draw = |app: &mut App, ui: &mut egui::Ui| {
            egui::Window::new("probe").default_pos(egui::pos2(400.0, 200.0)).show(&ui.ctx().clone(), |ui| {
                ui.label("probe");
            });
            app.viewport(ui);
        };
        let _ = ctx.run_ui(frame(Vec::new()), |c| draw(&mut app, c)); // lay the window out
        let before = app.viewing.cam.yaw;
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(title)]), |c| draw(&mut app, c));
        let _ = ctx.run_ui(frame(vec![press(title, egui::PointerButton::Primary, true)]), |c| draw(&mut app, c));
        for k in 1..=6 {
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(egui::pos2(title.x + k as f32 * 20.0, title.y))]), |c| draw(&mut app, c));
        }

        assert!(
            (app.viewing.cam.yaw - before).abs() < 1e-9,
            "the window was dragged by its title bar and the camera turned with it, by {}",
            (app.viewing.cam.yaw - before).abs()
        );
    }

    /// EVERY SET HAS WORDS IN EVERY LANGUAGE - a name and a line saying what it does.
    ///
    /// Walked over `MouseNav::ALL` rather than over a list written here: a set added to the type must show
    /// up in this check by itself, or the check falls behind exactly when it is needed.
    #[test]
    fn every_set_is_named_in_every_language() {
        let prev = crate::i18n::language();
        let mut holes = Vec::new();
        for (code, _) in crate::i18n::available() {
            crate::i18n::set_language(&code);
            for nav in MouseNav::ALL {
                for k in [nav.key(), nav.hint_key()] {
                    let t = crate::i18n::tr(&k);
                    if t == k || t.trim().is_empty() {
                        holes.push(format!("{code}: {k}"));
                    }
                }
            }
        }
        crate::i18n::set_language(&prev);
        assert!(holes.is_empty(), "a mouse set would show up as a key instead of a name ({}):\n{}", holes.len(), holes.join("\n"));
    }

    /// THE VIEWPORT ASKS THE SET INSTEAD OF DECIDING FOR ITSELF.
    ///
    /// The two above prove the sets differ; this one proves the 3D view is the thing that asks. Without it
    /// the sets could be right and unused, which is the shape of a green check over a dead setting.
    #[test]
    fn the_viewport_asks_the_layout() {
        let src = std::fs::read_to_string(qymcad_i18n::ratchet::crates_root().join("qymcad/src/gui/viewport_3d.rs")).expect("the 3D viewport reads");
        assert!(
            src.contains("mouse_nav.pan().active(ctx, resp)") && src.contains("mouse_nav.rotate().active(ctx, resp)"),
            "the 3D viewport decides for itself which button moves the view, so the setting is a dead control"
        );
    }
}
