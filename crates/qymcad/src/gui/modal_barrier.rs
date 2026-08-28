//! A MODAL WINDOW MUST BE MODAL. Written from a report.
//!
//! Reported behaviour: the dimming behind the rebuild window works, but it does not block mouse clicks
//! on the CAD's own interface — buttons and menus can still be pressed at that moment, which is
//! wrong.
//!
//! The dimming really was only DRAWN. The input was muted by a line of
//! `ctx.input_mut(|i| i.events.clear())` in the handler of the background task — and that line was
//! late by construction: `egui` gathers the input state at the start of a pass, while the events were
//! being cleared inside it. By that moment the presses have been parsed.
//!
//! The cost of the mistake is not cosmetic: while a rebuild runs, the document lives in the WORKING
//! COPY of the thread, and an edit that slipped through the dimming would land on a stale model.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A frame with a button under the overlay; returns whether the click reached the button.
    ///
    /// The click is spread over frames, as live input is: bring the cursor over, press, release. `egui`
    /// counts a press on the frame of the RELEASE, and gluing it all into one frame means checking the
    /// wrong thing (the first version did exactly that — and the control test honestly showed that the
    /// click did not arrive at all).
    fn button_clicked_under(overlay: Option<&str>) -> bool {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let app = App::default();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        let rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let clicked = std::cell::Cell::new(false);
        let draw = |ui: &mut egui::Ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = ui.button("a CAD button");
                rect.set(r.rect);
                if r.clicked() {
                    clicked.set(true);
                }
            });
            if let Some(label) = overlay {
                app.draw_dim_overlay_for_test(ui.ctx(), label);
            }
        };
        let frame = |events: Vec<egui::Event>| egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };

        let _ = ctx.run_ui(frame(vec![]), &draw); // lay out and learn where the button is
        let at = rect.get().center();
        assert!(rect.get().is_positive(), "setup: the button did not lay out");

        let _ = ctx.run_ui(frame(vec![egui::Event::PointerMoved(at)]), &draw);
        let press = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() };
        let _ = ctx.run_ui(frame(vec![press]), &draw);
        let release = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() };
        let _ = ctx.run_ui(frame(vec![release]), &draw);
        clicked.get()
    }

    /// WITHOUT THE OVERLAY A CLICK ARRIVES — otherwise the test below is green for a broken scene
    /// as well.
    #[test]
    fn without_the_overlay_a_click_reaches_the_button() {
        assert!(button_clicked_under(None), "setup: without the dimming a click must reach the button");
    }

    /// UNDER THE OVERLAY A CLICK IS SWALLOWED. Exactly the reported complaint.
    #[test]
    fn under_the_overlay_a_click_is_swallowed() {
        assert!(
            !button_clicked_under(Some("Rebuilding...")),
            "a click went THROUGH the dimming — during a rebuild an edit would land on a stale copy of the project"
        );
    }

    /// THE SPLASH IS A SMALL CARD IN THE CENTRE, NOT THE WHOLE SCREEN.
    ///
    /// Reported: what was expected was a small window in the centre with an icon, a name, a spinner and
    /// a description; instead, on a small project something flashes across the whole window, and on a
    /// large one the loading takes the whole screen.
    #[test]
    fn the_splash_is_a_small_card_not_the_whole_screen() {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 1000.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let app = App::default();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        for _ in 0..3 {
            let _ = ctx.run_ui(input.clone(), |c| app.draw_splash_for_test(c, "Opening the project..."));
        }

        let area = egui::AreaState::load(&ctx, egui::Id::new("splash")).expect("the splash must be on the screen");
        let size = area.size.expect("the splash must have a size");
        assert!(size.x > 100.0 && size.y > 80.0, "the splash came out degenerate: {size:?}");
        assert!(
            size.x <= screen.width() * 0.45 && size.y <= screen.height() * 0.45,
            "the splash took {size:?} on a screen of {:?} — that is not a card in the centre but the whole screen",
            screen.size()
        );
        // and in the centre rather than in a corner
        let c = area.pivot_pos.expect("the position of the splash");
        assert!((c.x - screen.center().x).abs() < screen.width() * 0.25, "the splash must be in the centre, and it stands at {c:?}");
    }

    /// THE SPLASH GOES AWAY RATHER THAN HANGING FOR EVER. Written from a report.
    ///
    /// Reported: it does not start, just a white window and nothing else. The early return for the
    /// splash had been put BEFORE the startup loading was launched — the loading never began at all,
    /// the "still loading" condition held for ever, and the program stayed a white rectangle
    /// permanently.
    ///
    /// What is checked is not the layout but THE MAIN PROPERTY: the frame stops being a splash and the
    /// interface appears. There is no need to wait five seconds — the time is set through a test
    /// facade.
    #[test]
    fn the_splash_goes_away_and_the_ui_appears() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };

        app.set_splash_for_test(std::time::Duration::from_millis(0));
        let mut eaten = true;
        for _ in 0..5 {
            let _ = ctx.run_ui(input.clone(), |c| eaten = app.tick_async_for_test(c.ctx()));
            if !eaten {
                break;
            }
        }
        assert!(!eaten, "the splash does not release the frame — the program stays a white window");
    }

    /// THE CARD IS VISIBLE ABOVE THE BACKDROP rather than painted over by it.
    ///
    /// The backdrop fill was put into the top layer while the card was left an ordinary window — and
    /// the fill painted over it: what was seen was an empty white rectangle. "Drawn later" does not
    /// mean "visible": the layer decides.
    #[test]
    fn the_splash_card_is_visible_above_the_backdrop() {
        let app = App::default();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let mut out = ctx.run_ui(input.clone(), |c| app.draw_splash_for_test(c, "Starting"));
        for _ in 0..2 {
            out = ctx.run_ui(input.clone(), |c| app.draw_splash_for_test(c, "Starting"));
        }

        // the order in the list of shapes is the order of drawing: later means on top
        let mut backdrop_at: Option<usize> = None;
        let mut card_text_at: Option<usize> = None;
        for (i, cs) in out.shapes.iter().enumerate() {
            if backdrop_at.is_none() {
                if let egui::epaint::Shape::Rect(r) = &cs.shape {
                    if r.rect.width() >= screen.width() - 1.0 && r.rect.height() >= screen.height() - 1.0 {
                        backdrop_at = Some(i);
                    }
                }
            }
            let mut texts = Vec::new();
            collect_text(&cs.shape, &mut texts);
            if card_text_at.is_none() && texts.iter().any(|t| t.contains("QymCAD")) {
                card_text_at = Some(i);
            }
        }
        let b = backdrop_at.expect("the backdrop fill must be in the frame");
        let c = card_text_at.expect("the name on the card must be in the frame");
        assert!(c > b, "the card ({c}) is drawn EARLIER than the backdrop ({b}) — the backdrop will paint over it and a person will see an empty rectangle");
    }

    /// All the text of a shape of the frame.
    fn collect_text(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect_text(x, out)),
            _ => {}
        }
    }

    /// A PROJECT OPENED AT STARTUP ACTUALLY FINISHES LOADING. Written from a report.
    ///
    /// Reported: opened a project, closed the application, opened it again — and everything hung on
    /// "loading the project", the spinner turning endlessly.
    ///
    /// The cause was the SECOND of its kind in a row: the early return from the frame stood BEFORE the
    /// branch that polls the channel of the background task. The splash was drawn, the frame ended,
    /// nobody asked the task anything — and it never finished.
    ///
    /// So what is checked is not the picture but THE MOVEMENT: within a reasonable number of frames the
    /// startup loading must finish and release the interface.
    #[test]
    fn a_project_opened_at_startup_actually_finishes_loading() {
        use qymcad_core::feature::SketchPlane;
        let dir = std::env::temp_dir().join("qym_startup_load_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("startup.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        // a real project on the disk
        let mut src = App::default();
        let si = src.create_sketch_on(SketchPlane::default());
        src.project.add_rect_entity(si, -10.0, -10.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        src.project.regen_sketch(si);
        src.finish_sketch_edit();
        src.set_project_path(path.clone());
        src.save_for_test(path.clone());

        // ...and a launch of the program WITH IT, the way `launch` does it
        let mut app = App::default();
        app.set_startup_for_test(&path);
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };

        let mut released = false;
        for _ in 0..400 {
            let mut eaten = true;
            let _ = ctx.run_ui(input.clone(), |c| eaten = app.tick_async_for_test(c.ctx()));
            if !eaten {
                released = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(released, "the startup loading did not finish — the spinner turns for ever and the program cannot be used");
        assert!(!app.project.timeline.is_empty(), "the project must turn out loaded, and the timeline is empty");
        let _ = std::fs::remove_file(&path);
    }
}
