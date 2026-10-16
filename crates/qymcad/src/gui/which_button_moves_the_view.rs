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
        // a BODY: a drag that starts on the model turns the view, one that starts on empty space draws a frame
        app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        app.chosen.sel = qymcad_ui_state::Sel::None; // no gizmo stands over the body to catch the drag
        app.viewing.mode_3d = true;
        // IN THE PART: in an assembly a left drag on a part carries the part, and one from empty space draws a frame
        if let Some(owner) = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)) {
            app.enter_component(owner);
        }
        app.sync_workbench();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c)); // lay out the canvas
        (app, ctx)
    }

    /// Where the body is seen: the middle of its top, as the view stands now.
    fn on_the_body(app: &App) -> egui::Pos2 {
        let basis = app.viewing.cam.basis();
        let mi = (0..app.project.bodies.len()).find(|&mi| app.project.mesh_id(mi).is_some()).expect("the body");
        let f = app.project.bodies[mi].faces.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("a face");
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([f.centroid.x, f.centroid.y, f.centroid.z]).0
    }

    /// THE POINT OF THE MODEL SEEN IN THE MIDDLE OF THE VIEW, as the view stands now.
    fn seen_in_the_middle(app: &App) -> [f64; 3] {
        let rect = app.viewing.view_rect;
        crate::gui::look_at_point::point_under(&app.painting(), rect, rect.center())
    }

    /// WHETHER THE VIEW MOVED SIDEWAYS: the point seen in the middle before has left the middle. The centre of the view
    /// may slide along the line of sight to what is seen there, and nothing on screen shows that.
    fn middle_left(app: &App, seen: [f64; 3]) -> bool {
        let rect = app.viewing.view_rect;
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis: &basis }.at(seen).0.distance(rect.center()) > 0.5
    }

    /// Drag across the canvas with `button`, through real frames, and answer how far the view turned.
    fn yaw_after_a_drag(nav: MouseNav, button: egui::PointerButton) -> f64 {
        let (mut app, ctx) = a_part_in_view(nav);
        let before = app.viewing.cam.yaw;
        let from = on_the_body(&app);
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
        let _ = ctx.run_ui(frame(vec![press(from, button, true)]), |c| app.viewport(c));
        for k in 1..=6 {
            let p = egui::pos2(from.x + k as f32 * 12.0, from.y);
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(p)]), |c| app.viewport(c));
        }
        let _ = ctx.run_ui(frame(vec![press(egui::pos2(from.x + 72.0, from.y), button, false)]), |c| app.viewport(c));
        (app.viewing.cam.yaw - before).abs()
    }

    /// THE TOUCHPAD SCALES WITH CTRL, SHIFT AND A MOVEMENT: it has no wheel, and its layout puts the zoom on that
    /// chord. Reported behaviour: the scale stood at 323.44 before and after - nothing asked the layout for its zoom.
    /// A movement up brings the model nearer, as the wheel turned forward does.
    #[test]
    fn the_touchpad_scales_with_its_own_gesture() {
        let (mut app, ctx) = a_part_in_view(MouseNav::Touchpad);
        let was = app.viewing.cam.scale;
        let held = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
        let at = |y: f32| egui::pos2(700.0, y);
        let moved = |p: egui::Pos2| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            events: vec![egui::Event::PointerMoved(p)],
            modifiers: held,
            ..Default::default()
        };
        for k in 0..=6 {
            let _ = ctx.run_ui(moved(at(400.0 - k as f32 * 20.0)), |c| app.viewport(c));
        }
        let now = app.viewing.cam.scale;
        assert!(now > was * 1.1, "Ctrl, Shift and a movement up under the touchpad layout left the scale at {now} (it was {was})");
    }

    /// A CHORD OF TWO BUTTONS TURNS THE MODEL where the layout says so, though one of its buttons alone moves the
    /// view sideways. Reported behaviour: under CAD (middle + left) and OpenCascade (middle + right) the chord moved
    /// the view by exactly the drag and never turned it - the pan was asked first and its one button was held.
    /// The middle button goes down first, as a hand does it, then the other, then the drag.
    #[test]
    fn a_chord_turns_the_model_rather_than_moving_it() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            let chord = nav.rotate().buttons;
            if chord.len() < 2 {
                continue;
            }
            let (mut app, ctx) = a_part_in_view(nav);
            let (yaw, seen) = (app.viewing.cam.yaw, seen_in_the_middle(&app));
            let from = on_the_body(&app);
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
            for b in chord {
                let _ = ctx.run_ui(frame(vec![press(from, *b, true)]), |c| app.viewport(c));
            }
            for k in 1..=6 {
                let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(egui::pos2(from.x + k as f32 * 12.0, from.y))]), |c| app.viewport(c));
            }
            for b in chord.iter().rev() {
                let _ = ctx.run_ui(frame(vec![press(egui::pos2(from.x + 72.0, from.y), *b, false)]), |c| app.viewport(c));
            }
            let turned = (app.viewing.cam.yaw - yaw).abs();
            if turned < 1e-6 || middle_left(&app, seen) {
                wrong.push(format!("{nav:?}: turned {turned:.4}, the middle of the view moved {}", middle_left(&app, seen)));
            }
        }
        assert!(wrong.is_empty(), "the chord of rotation did not turn the model (or moved it too): {wrong:?}");
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
            egui::Window::new("probe").id(egui::Id::new("probe")).default_pos(egui::pos2(400.0, 200.0)).show(&ui.ctx().clone(), |ui| {
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

        assert!((app.viewing.cam.yaw - before).abs() < 1e-9, "the window was dragged by its title bar and the camera turned with it, by {}", (app.viewing.cam.yaw - before).abs());
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
            src.contains("qymcad_ui_state::pan_now(self.set.mouse_nav, ctx, resp)")
                && src.contains("qymcad_ui_state::turn_view(crate::gui::orbit_about::pivot(&self.painting(), rect, ctx, resp), &mut self.viewing.cam, self.set.mouse_nav, ctx, resp)"),
            "the 3D viewport decides for itself which button moves the view, so the setting is a dead control"
        );
    }

    /// MAKE `g` AS A HAND DOES: its modifiers held through every frame, its buttons pressed one after another, a
    /// movement of `by` in six steps, the buttons let go in the reverse order. A gesture of no button is the movement
    /// with the modifiers held. `any` is the middle button.
    fn make(app: &mut App, ctx: &egui::Context, from: egui::Pos2, g: &qymcad_ui_state::Gesture, by: egui::Vec2) {
        let modifiers = egui::Modifiers { shift: g.shift, ctrl: g.ctrl, command: g.ctrl, alt: g.alt, ..Default::default() };
        let buttons: Vec<egui::PointerButton> = if g.any_button { vec![egui::PointerButton::Middle] } else { g.buttons.to_vec() };
        let run = |app: &mut App, events: Vec<egui::Event>| {
            let _ = ctx.run_ui(egui::RawInput { modifiers, ..frame(events) }, |c| app.viewport(c));
        };
        run(app, vec![egui::Event::PointerMoved(from)]);
        for b in &buttons {
            run(app, vec![egui::Event::PointerButton { pos: from, button: *b, pressed: true, modifiers }]);
        }
        for k in 1..=6 {
            run(app, vec![egui::Event::PointerMoved(from + by * (k as f32 / 6.0))]);
        }
        for b in buttons.iter().rev() {
            run(app, vec![egui::Event::PointerButton { pos: from + by, button: *b, pressed: false, modifiers }]);
        }
    }

    /// EVERY GESTURE OF EVERY LAYOUT DOES WHAT ITS PROGRAM DOES WITH IT, in the 3D view: each way of turning turns the
    /// model and moves nothing else, each way of moving moves the view and turns nothing, each way of zooming by a
    /// movement scales it. Begun on the model, as a hand begins. Reported behaviour: the layouts named after other
    /// programs did one gesture of each and not the rest, and OpenCascade turned on the middle and right buttons where
    /// its program turns on Ctrl and the right one.
    #[test]
    fn every_gesture_of_every_layout_moves_the_view_as_its_program_does() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            let all = [("turn", nav.rotates()), ("move", nav.pans()), ("zoom", nav.zooms())];
            for (what, gestures) in all {
                for g in gestures {
                    let (mut app, ctx) = a_part_in_view(nav);
                    let from = on_the_body(&app);
                    let (yaw, seen, scale) = (app.viewing.cam.yaw, seen_in_the_middle(&app), app.viewing.cam.scale);
                    let by = if what == "zoom" { egui::vec2(0.0, -72.0) } else { egui::vec2(72.0, 0.0) };
                    make(&mut app, &ctx, from, g, by);
                    let (turned, moved, scaled) = (app.viewing.cam.yaw != yaw, middle_left(&app, seen), app.viewing.cam.scale != scale);
                    let right = match what {
                        "turn" => turned && !moved && !scaled,
                        "move" => moved && !turned && !scaled,
                        _ => scaled && !turned,
                    };
                    if !right {
                        wrong.push(format!("{nav:?} {what} {g:?}: turned {turned}, moved {moved}, scaled {scaled}"));
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "a gesture of a layout does not do what its program does ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// A SKETCH OPEN, its sheet in view, under `nav`.
    fn a_sketch_in_view(nav: MouseNav) -> (App, egui::Context) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        app.set.mouse_nav = nav;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
        (app, ctx)
    }

    /// THE SHEET OF A SKETCH MOVES WITH THE LAYOUT'S OWN GESTURES OF MOVING THE VIEW, and with no other: a flat sheet
    /// has nothing to turn, and the bare middle button moves it only where the layout moves the view with it (ours
    /// too). A gesture that holds the left button is passed over: in a sketch that button draws and grabs.
    #[test]
    fn the_sheet_of_a_sketch_moves_with_the_layouts_own_gestures() {
        let mut wrong = Vec::new();
        let middle = qymcad_ui_state::Gesture { buttons: &[egui::PointerButton::Middle], any_button: false, shift: false, ctrl: false, alt: false };
        for nav in MouseNav::ALL {
            let pans: Vec<qymcad_ui_state::Gesture> = nav.pans().iter().copied().filter(|g| g.sheet_may_take()).collect();
            let middle_pans = nav == MouseNav::QymCad || pans.contains(&middle);
            let mut cases: Vec<(qymcad_ui_state::Gesture, bool)> = pans.into_iter().map(|g| (g, true)).collect();
            cases.push((middle, middle_pans));
            for g in nav.rotates().iter().filter(|g| !g.buttons.contains(&egui::PointerButton::Primary) && !g.takes_a_bare_left_drag()) {
                cases.push((*g, false));
            }
            for (g, should) in cases {
                let (mut app, ctx) = a_sketch_in_view(nav);
                let from = app.viewing.view_rect.center() + egui::vec2(150.0, 120.0); // off the rectangle
                let was = app.viewing.view.center;
                make(&mut app, &ctx, from, &g, egui::vec2(72.0, 0.0));
                let moved = app.viewing.view.center != was;
                if moved != should {
                    wrong.push(format!("{nav:?} {g:?}: the sheet {}", if moved { "moved" } else { "stood" }));
                }
            }
        }
        // and the layout's zoom by a movement scales the sheet, a movement up bringing it nearer
        for nav in MouseNav::ALL {
            for g in nav.zooms() {
                let (mut app, ctx) = a_sketch_in_view(nav);
                let from = app.viewing.view_rect.center() + egui::vec2(150.0, 120.0);
                let was = app.viewing.view.scale;
                make(&mut app, &ctx, from, g, egui::vec2(0.0, -72.0));
                // a chord zooms the sheet; the left button alone, with Ctrl or Shift, is the sketch's own
                let nearer = app.viewing.view.scale > was;
                if nearer != g.sheet_may_take() {
                    wrong.push(format!("{nav:?} {g:?}: the sheet {} ({was} -> {})", if nearer { "came nearer" } else { "stood" }, app.viewing.view.scale));
                }
            }
            // A CHORD BEGUN ON A CORNER OF THE RECTANGLE moves the sheet and leaves the corner where it is
            for g in nav.pans().iter().chain(nav.zooms()).filter(|g| g.buttons.len() >= 2) {
                let (mut app, ctx) = a_sketch_in_view(nav);
                let si = app.project.sketches.len() - 1;
                let corner = {
                    let p = app.project.sketches[si].points.iter().find(|p| p.x.abs() < 1e-9 && p.y.abs() < 1e-9).expect("the corner at the origin");
                    qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect }.at(qymcad_core::geom::Point2 { x: p.x, y: p.y })
                };
                let before: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
                make(&mut app, &ctx, corner, g, egui::vec2(40.0, -30.0));
                let after: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
                if after != before {
                    wrong.push(format!("{nav:?} {g:?}: a chord begun on the corner moved the geometry"));
                }
            }
        }
        assert!(wrong.is_empty(), "the sheet of a sketch does not move as the layout moves the view ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// A CLICK TAKES WHAT IS UNDER IT AS THE LAYOUT'S PROGRAM TAKES IT: under the layout whose bare left button turns
    /// the model only with Ctrl or Shift; under every other layout a bare click.
    #[test]
    fn a_click_takes_the_body_as_the_layouts_program_does() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            for modifiers in [egui::Modifiers::NONE, egui::Modifiers::CTRL, egui::Modifiers::SHIFT] {
                let (mut app, ctx) = a_part_in_view(nav);
                let at = on_the_body(&app);
                let run = |app: &mut App, events: Vec<egui::Event>| {
                    let _ = ctx.run_ui(egui::RawInput { modifiers, ..frame(events) }, |c| app.viewport(c));
                };
                run(&mut app, vec![egui::Event::PointerMoved(at)]);
                run(&mut app, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers }]);
                run(&mut app, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers }]);
                run(&mut app, Vec::new());
                let took = !matches!(app.chosen.sel, qymcad_ui_state::Sel::None);
                let should = nav != MouseNav::OpenInventor || modifiers != egui::Modifiers::NONE;
                if took != should {
                    wrong.push(format!("{nav:?} with {modifiers:?}: the click {}", if took { "took the body" } else { "took nothing" }));
                }
            }
        }
        assert!(wrong.is_empty(), "a click does not take as the layout's program does ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// A turn by a drag of the left button under the QymCad layout, from `from` along `path` (steps of the pointer),
    /// frame by frame; `each` looks at the program after every step.
    fn turn_along(app: &mut App, ctx: &egui::Context, from: egui::Pos2, path: &[egui::Vec2], mut each: impl FnMut(&App)) {
        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
        let _ = ctx.run_ui(frame(vec![press(from, egui::PointerButton::Primary, true)]), |c| app.viewport(c));
        let mut at = from;
        for step in path {
            at += *step;
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport(c));
            each(app);
        }
        let _ = ctx.run_ui(frame(vec![press(at, egui::PointerButton::Primary, false)]), |c| app.viewport(c));
    }

    /// THE VIEW TURNS ABOUT THE POINT UNDER THE POINTER when the setting says so: the point stays on its spot of the
    /// screen through the whole turn, and a turn there and back, five times over, leaves the view where it was.
    /// Reported in a pull request with a video: the first way of doing it slid the centre in the plane of the screen,
    /// and after a few turns at a corner of a long block the model swung round a point far out in space.
    #[test]
    fn the_view_turns_about_the_point_under_the_pointer_and_comes_back() {
        let mut wrong = Vec::new();
        for projection in [qymcad_ui_state::Projection::Ortho, qymcad_ui_state::Projection::Perspective] {
            let (mut app, ctx) = a_part_in_view(MouseNav::QymCad);
            app.set.projection = projection;
            app.set.orbit_about = qymcad_ui_state::OrbitAbout::Pointer;
            let from = on_the_body(&app) + egui::vec2(-45.0, 6.0); // far from the middle of the view
            let rect = app.viewing.view_rect;
            let pivot = crate::gui::look_at_point::point_under(&app.painting(), rect, from);
            let spot = |app: &App| {
                let basis = app.viewing.cam.basis();
                qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis: &basis }.at(pivot).0
            };
            let (spot0, target0, yaw0, pitch0) = (spot(&app), app.viewing.cam.target, app.viewing.cam.yaw, app.viewing.cam.pitch);
            let mut path = Vec::new();
            for _ in 0..5 {
                path.extend(std::iter::repeat_n(egui::vec2(10.0, 4.0), 8));
                path.extend(std::iter::repeat_n(egui::vec2(-10.0, -4.0), 8));
            }
            let mut worst = 0.0f32;
            turn_along(&mut app, &ctx, from, &path, |app| worst = worst.max(spot(app).distance(spot0)));
            if worst > 0.5 {
                wrong.push(format!("{projection:?}: the point turned about left its spot by {worst:.2} points"));
            }
            let drift = (0..3).map(|k| (app.viewing.cam.target[k] - target0[k]).abs()).fold(0.0, f64::max);
            if drift > 1e-6 || (app.viewing.cam.yaw - yaw0).abs() > 1e-9 || (app.viewing.cam.pitch - pitch0).abs() > 1e-9 {
                wrong.push(format!(
                    "{projection:?}: five turns there and back moved the centre by {drift:.3e} and left yaw {} / pitch {} (were {yaw0} / {pitch0})",
                    app.viewing.cam.yaw, app.viewing.cam.pitch
                ));
            }
            // UP TO THE STOP OF THE TILT AND ON: the tilt stands still there, and the point must not go anywhere
            let mut worst = 0.0f32;
            turn_along(&mut app, &ctx, from, &[egui::vec2(0.0, -40.0); 10], |app| worst = worst.max(spot(app).distance(spot0)));
            if worst > 0.5 {
                wrong.push(format!("{projection:?}: at the stop of the tilt the point left its spot by {worst:.2} points"));
            }
        }
        assert!(wrong.is_empty(), "the view does not turn about the point under the pointer:\n{}", wrong.join("\n"));
    }

    /// THE MIDDLE OF THE VIEW STAYS THE CENTRE OF A TURN BY DEFAULT: the program's own way, whatever is under the pointer.
    /// What holds is the point of the model seen in the middle - the centre of the view may slide along the line of sight
    /// to it, which changes nothing on screen.
    #[test]
    fn by_default_the_view_turns_about_its_middle() {
        let (mut app, ctx) = a_part_in_view(MouseNav::QymCad);
        assert_eq!(app.set.orbit_about, qymcad_ui_state::OrbitAbout::ViewCentre, "the factory setting turns about the middle of the view");
        let yaw0 = app.viewing.cam.yaw;
        let rect = app.viewing.view_rect;
        let middle = crate::gui::look_at_point::point_under(&app.painting(), rect, rect.center());
        let spot = |app: &App| {
            let basis = app.viewing.cam.basis();
            qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis: &basis }.at(middle).0
        };
        let from = on_the_body(&app) + egui::vec2(-45.0, 6.0);
        let mut moved = 0.0f32;
        turn_along(&mut app, &ctx, from, &[egui::vec2(10.0, 4.0); 8], |app| moved = moved.max(spot(app).distance(rect.center())));
        assert!((app.viewing.cam.yaw - yaw0).abs() > 0.1, "setup: the drag did not turn the view");
        assert!(moved < 0.5, "turning about the middle of the view moved the point seen there by {moved} points");
    }

    /// THE POINT OF THE MODEL UNDER THE POINTER IS WHERE THE POINTER IS: put back on the screen, it lands on the spot
    /// that was pointed at, wherever on a face that is. The centre of the triangle hit is not that point - a click
    /// near the corner of a large facet looked at, measured from and trimmed at the middle of the facet.
    #[test]
    fn the_point_under_the_pointer_lands_back_under_it() {
        let mut wrong = Vec::new();
        let mut taken = 0;
        for projection in [qymcad_ui_state::Projection::Ortho, qymcad_ui_state::Projection::Perspective] {
            let (mut app, _) = a_part_in_view(MouseNav::QymCad);
            app.set.projection = projection;
            let basis = app.viewing.cam.basis();
            let screen = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis };
            let mid = on_the_body(&app);
            // the top and, below it, the faces of the sides the view looks at, slanted to the line of sight
            for dy in [-15.0, 0.0, 12.0, 30.0, 45.0, 60.0] {
                for dx in [-60.0, -30.0, 0.0, 25.0, 50.0] {
                    let at = mid + egui::vec2(dx, dy);
                    let Some((_, _, hit)) = qymcad_pick::pick_face_ray(&app.painting(), app.viewing.view_rect, at) else { continue };
                    taken += 1;
                    let back = screen.at(hit).0;
                    if back.distance(at) > 0.5 {
                        wrong.push(format!("{projection:?}: pointed at {at:?}, the point {hit:?} lies at {back:?} on screen"));
                    }
                }
            }
        }
        assert!(taken > 20, "setup: the pointer found the body {taken} times");
        assert!(wrong.is_empty(), "the point taken under the pointer is not under it:\n{}", wrong.join("\n"));
    }

    /// A SHORT CLICK OF THE MIDDLE BUTTON LOOKS AT THE POINT OF THE MODEL UNDER IT where the layout's program does so
    /// - the view's centre, the point it turns about, goes there - and moves nothing under the other layouts.
    #[test]
    fn a_middle_click_looks_at_the_point_where_the_layouts_program_does() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            let (mut app, ctx) = a_part_in_view(nav);
            let at = on_the_body(&app) + egui::vec2(-12.0, 6.0);
            let hit = qymcad_pick::pick_face_ray(&app.painting(), app.viewing.view_rect, at).map(|(_, _, w)| w).expect("the body is under the pointer");
            let was = app.viewing.cam.target;
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(at)]), |c| app.viewport(c));
            let _ = ctx.run_ui(frame(vec![press(at, egui::PointerButton::Middle, true)]), |c| app.viewport(c));
            let _ = ctx.run_ui(frame(vec![press(at, egui::PointerButton::Middle, false)]), |c| app.viewport(c));
            // the view moves there smoothly, as the programs move it: the move is let run to its end
            let halfway = app.viewing.view_anim.is_some();
            std::thread::sleep(std::time::Duration::from_millis(260));
            crate::gui::tick_view_anim(&mut app.viewing.cam, &mut app.viewing.view_anim, &ctx);
            if nav.middle_click_looks() && !halfway {
                wrong.push(format!("{nav:?}: the view jumped to the point instead of moving there"));
            }
            let now = app.viewing.cam.target;
            let looked = (0..3).all(|k| (now[k] - hit[k]).abs() < 1e-6);
            if nav.middle_click_looks() != looked {
                wrong.push(format!("{nav:?}: the centre went from {was:?} to {now:?}, the point clicked is {hit:?}"));
            }
        }
        assert!(wrong.is_empty(), "a middle click does not look as the layout's program does ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// THE MIDDLE BUTTON HELD, A SHORT CLICK OF THE LEFT OR RIGHT ONE, AND THE MOVEMENT ZOOMS, under CAD as its program
    /// has it - in the 3D view and on the sheet of a sketch; without the click the same movement moves the view.
    #[test]
    fn under_cad_a_click_while_the_middle_button_is_held_latches_the_zoom() {
        let lead = |app: &mut App, ctx: &egui::Context, from: egui::Pos2, click: Option<egui::PointerButton>| {
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
            let _ = ctx.run_ui(frame(vec![press(from, egui::PointerButton::Middle, true)]), |c| app.viewport(c));
            if let Some(b) = click {
                let _ = ctx.run_ui(frame(vec![press(from, b, true)]), |c| app.viewport(c));
                let _ = ctx.run_ui(frame(vec![press(from, b, false)]), |c| app.viewport(c));
            }
            for k in 1..=6 {
                let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from + egui::vec2(0.0, -12.0 * k as f32))]), |c| app.viewport(c));
            }
            let _ = ctx.run_ui(frame(vec![press(from + egui::vec2(0.0, -72.0), egui::PointerButton::Middle, false)]), |c| app.viewport(c));
        };
        let mut wrong = Vec::new();
        for click in [None, Some(egui::PointerButton::Primary), Some(egui::PointerButton::Secondary)] {
            let (mut app, ctx) = a_part_in_view(MouseNav::Cad);
            let from = on_the_body(&app);
            let (yaw, scale, target) = (app.viewing.cam.yaw, app.viewing.cam.scale, app.viewing.cam.target);
            lead(&mut app, &ctx, from, click);
            let (turned, scaled, moved) = (app.viewing.cam.yaw != yaw, app.viewing.cam.scale != scale, app.viewing.cam.target != target);
            let right = if click.is_some() { scaled && !turned } else { moved && !scaled && !turned };
            if !right {
                wrong.push(format!("3D, click {click:?}: turned {turned}, scaled {scaled}, moved {moved}"));
            }
            let (mut app, ctx) = a_sketch_in_view(MouseNav::Cad);
            let from = app.viewing.view_rect.center() + egui::vec2(150.0, 120.0);
            let (scale, centre) = (app.viewing.view.scale, app.viewing.view.center);
            lead(&mut app, &ctx, from, click);
            let (scaled, moved) = (app.viewing.view.scale != scale, app.viewing.view.center != centre);
            let right = if click.is_some() { scaled } else { moved && !scaled };
            if !right {
                wrong.push(format!("sketch, click {click:?}: scaled {scaled}, moved {moved}"));
            }
        }
        assert!(wrong.is_empty(), "the zoom latched by the middle button does not work as the CAD layout's program has it:\n{}", wrong.join("\n"));
    }

    /// THE FRAME OF SELECTION IS DRAWN WITH THE LAYOUT'S OWN GESTURE, round the body from empty space, and takes the body
    /// without turning the view; under the layouts whose programs draw none, the bare left drag from empty space turns
    /// the view and takes nothing.
    #[test]
    fn the_frame_is_drawn_as_the_layouts_program_draws_it() {
        let mut wrong = Vec::new();
        for nav in MouseNav::ALL {
            let (mut app, ctx) = a_part_in_view(nav);
            app.viewing.cam.scale *= 0.5; // the body well inside the canvas, empty space round it
            let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
            let canvas = app.viewing.view_rect;
            let (from, to) = (canvas.min + egui::vec2(8.0, 8.0), canvas.max - egui::vec2(8.0, 8.0));
            let (yaw, target) = (app.viewing.cam.yaw, app.viewing.cam.target);
            let gesture = nav.frames().map_or(qymcad_ui_state::Gesture { buttons: &[egui::PointerButton::Primary], any_button: false, shift: false, ctrl: false, alt: false }, |(g, _)| g);
            make(&mut app, &ctx, from, &gesture, to - from);
            let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c));
            let took = !matches!(app.chosen.sel, qymcad_ui_state::Sel::None) || !app.chosen.tree_sel.multi.is_empty();
            let moved = app.viewing.cam.yaw != yaw || app.viewing.cam.target != target;
            match nav.frames() {
                Some(_) if !took || moved => wrong.push(format!("{nav:?} {gesture:?}: the frame took {took}, the view moved {moved}")),
                None if took || !moved => wrong.push(format!("{nav:?}: a bare left drag from empty space took {took}, the view moved {moved}")),
                _ => {}
            }
        }
        assert!(wrong.is_empty(), "the frame is not drawn as the layout's program draws it ({}):\n{}", wrong.len(), wrong.join("\n"));
    }

    /// THE LEFT AND RIGHT BUTTONS TOGETHER TILT THE VIEW under Gesture, as its program has it, and no other layout tilts;
    /// a standard view of the cube takes the tilt away.
    #[test]
    fn under_gesture_both_buttons_tilt_the_view_and_the_cube_untilts_it() {
        let mut wrong = Vec::new();
        let chord = qymcad_ui_state::Gesture { buttons: &[egui::PointerButton::Primary, egui::PointerButton::Secondary], any_button: false, shift: false, ctrl: false, alt: false };
        for nav in MouseNav::ALL {
            let (mut app, ctx) = a_part_in_view(nav);
            let (yaw, pitch) = (app.viewing.cam.yaw, app.viewing.cam.pitch);
            let from = on_the_body(&app);
            make(&mut app, &ctx, from, &chord, egui::vec2(72.0, 0.0));
            let tilted = app.viewing.cam.roll != 0.0;
            if tilted != (nav == MouseNav::Gesture) || (tilted && (app.viewing.cam.yaw != yaw || app.viewing.cam.pitch != pitch)) {
                wrong.push(format!("{nav:?}: the tilt is {}, the turn {} / {}", app.viewing.cam.roll, app.viewing.cam.yaw - yaw, app.viewing.cam.pitch - pitch));
            }
        }
        let (mut app, ctx) = a_part_in_view(MouseNav::Gesture);
        app.viewing.cam.roll = 0.5;
        crate::gui::animate_view_to(app.viewing.cam, &mut app.viewing.mode_3d, &mut app.viewing.view_anim, 0.0, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(260));
        crate::gui::tick_view_anim(&mut app.viewing.cam, &mut app.viewing.view_anim, &ctx);
        if app.viewing.cam.roll != 0.0 {
            wrong.push(format!("a standard view left the tilt at {}", app.viewing.cam.roll));
        }
        assert!(wrong.is_empty(), "the tilt is not as the Gesture layout's program has it:\n{}", wrong.join("\n"));
    }

    /// UNDER GESTURE THE LEFT BUTTON HELD STILL AND THEN LED MOVES THE VIEW, as its program pans after a long press; led
    /// at once it turns the view as ever.
    #[test]
    fn under_gesture_a_long_press_then_a_movement_moves_the_view() {
        let mut wrong = Vec::new();
        for hold in [false, true] {
            let (mut app, ctx) = a_part_in_view(MouseNav::Gesture);
            let from = on_the_body(&app);
            let (yaw, seen) = (app.viewing.cam.yaw, seen_in_the_middle(&app));
            let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from)]), |c| app.viewport(c));
            let _ = ctx.run_ui(frame(vec![press(from, egui::PointerButton::Primary, true)]), |c| app.viewport(c));
            if hold {
                for _ in 0..40 {
                    let _ = ctx.run_ui(frame(Vec::new()), |c| app.viewport(c)); // a frame is 1/60 s: 0.67 s held still
                }
            }
            for k in 1..=6 {
                let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(from + egui::vec2(12.0 * k as f32, 0.0))]), |c| app.viewport(c));
            }
            let _ = ctx.run_ui(frame(vec![press(from + egui::vec2(72.0, 0.0), egui::PointerButton::Primary, false)]), |c| app.viewport(c));
            let (turned, moved) = (app.viewing.cam.yaw != yaw, middle_left(&app, seen));
            let right = if hold { moved && !turned } else { turned && !moved };
            if !right {
                wrong.push(format!("held {hold}: turned {turned}, moved {moved}"));
            }
        }
        assert!(wrong.is_empty(), "the long press is not as the Gesture layout's program has it:\n{}", wrong.join("\n"));
    }

    /// THE PEN'S LAYOUT ZOOMS WITH THE MIDDLE AND LEFT BUTTONS TOGETHER: right or up brings the view nearer, left or down
    /// takes it away - in the 3D view and on the sheet of a sketch alike. Reported behaviour asked for: "middle and left
    /// held, the pen moved right (or up) zooms in, the other way zooms out".
    #[test]
    fn the_pens_layout_zooms_right_and_up_in_left_and_down_out() {
        let chord = MouseNav::Den.zooms()[0];
        let mut wrong = Vec::new();
        // the scale a movement up gives is what "nearer" means for each view
        let (mut app, ctx) = a_part_in_view(MouseNav::Den);
        let (from, was) = (on_the_body(&app), app.viewing.cam.scale);
        make(&mut app, &ctx, from, &chord, egui::vec2(0.0, -60.0));
        let up_3d = app.viewing.cam.scale - was;
        for (name, by, near) in [("right", egui::vec2(60.0, 0.0), true), ("left", egui::vec2(-60.0, 0.0), false), ("down", egui::vec2(0.0, 60.0), false)] {
            let (mut app, ctx) = a_part_in_view(MouseNav::Den);
            let (from, was) = (on_the_body(&app), app.viewing.cam.scale);
            make(&mut app, &ctx, from, &chord, by);
            let d = app.viewing.cam.scale - was;
            if d == 0.0 || (d.signum() == up_3d.signum()) != near {
                wrong.push(format!("3D, {name}: the scale went {was} -> {}", app.viewing.cam.scale));
            }
            let (mut app, ctx) = a_sketch_in_view(MouseNav::Den);
            let from = app.viewing.view_rect.center() + egui::vec2(150.0, 120.0);
            let was = app.viewing.view.scale;
            make(&mut app, &ctx, from, &chord, by);
            if (app.viewing.view.scale > was) != near || app.viewing.view.scale == was {
                wrong.push(format!("sketch, {name}: the scale went {was} -> {}", app.viewing.view.scale));
            }
        }
        assert!(up_3d != 0.0 && wrong.is_empty(), "the pen's layout does not zoom right and up in, left and down out (up changed the 3D scale by {up_3d}):\n{}", wrong.join("\n"));
    }
}
