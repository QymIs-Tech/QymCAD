//! SAVING DOES NOT STAY SILENT.
//!
//! Reported behaviour: opening another project, closing the current one or closing the application
//! brings up a "Save / Save as" window, and there should be a popup with a spinner so that a person
//! does not think the program went quiet — it is saving the current project in order to open another.
//!
//! It really did stay silent, and worse than reported: on answering "Save" the interface went into a
//! BLOCKING wait for the write (`wait_bg`) and no frame was painted at all. To a person that is
//! indistinguishable from a frozen program.
#[cfg(test)]
mod tests {
    use super::super::{App, Nav, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A document with a body, saved to a file. Returns (application, path).
    fn project_with_a_body(name: &str) -> (App, String) {
        let dir = std::env::temp_dir().join("qym_saving_not_silent");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join(name).to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app.set_project_path(path.clone());
        (app, path)
    }

    /// One frame of the "save?" dialogue: what is painted.
    fn frame(app: &mut App, ctx: &egui::Context) -> Vec<String> {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |c| {
            app.nav_dialog_for_test(c);
        });
        let mut texts = Vec::new();
        for cs in &out.shapes {
            super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
        }
        texts
    }

    /// WHILE THE WRITE IS RUNNING the frame shows a waiting card, and the navigation waits its turn.
    #[test]
    fn while_saving_the_window_says_what_it_is_doing() {
        let (mut app, _path) = project_with_a_body("wait.qcad");
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        // A person asked to open another document and answered "Save".
        app.request_nav_for_test(Nav::New);
        assert!(app.deferred_nav_is_set_for_test(), "setup: unsaved edits held the navigation back");
        app.answer_save_for_test();

        // TWO FRAMES: egui windows and areas settle into place on the second pass — popup checks
        // have already stumbled on this.
        let _ = frame(&mut app, &ctx);
        // A REAL WRITE OUTLASTS THE GRACE. The card is deliberately silent on a write shorter than
        // `SAVE_WAIT_GRACE`, so the clock is pushed back rather than slept through.
        app.age_save_wait_for_test(std::time::Duration::from_secs(1));
        // THE CARD IS DRAWN ON THE FRAME AFTER THE GRACE PASSES, and an egui area settles on the pass after
        // that: two frames, as everywhere else here.
        let _ = frame(&mut app, &ctx);
        let texts = frame(&mut app, &ctx);
        let saying = crate::i18n::tr("io-saving");
        assert!(
            texts.iter().any(|t| t.contains(&saying)),
            "while the write runs the window stays silent: expected \"{saying}\", writing: {}, in the frame {texts:?}",
            app.saving_now_for_test()
        );

        // THE NAVIGATION IS NOT LOST: it happens once the write reaches the disk and what was put on
        // screen has been readable long enough (the floor holds the navigation too, or the card would blink
        // out under it).
        app.drain_bg_for_test();
        app.age_save_wait_for_test(std::time::Duration::from_secs(1));
        let _ = frame(&mut app, &ctx);
        assert!(!app.deferred_nav_is_set_for_test(), "the navigation did not happen after the write: the command was lost");
    }

    /// EVERY DOOR OUT OF THE DOCUMENT SAYS THE SAME THING.
    ///
    /// The report named three: opening another project, closing the current one, and closing the
    /// application. Only one of them used to be checked, and "it is the same code" is precisely the
    /// reasoning that leaves the other two silent — they go through the same deferred navigation, but
    /// nothing said so out loud.
    #[test]
    fn every_way_out_of_the_document_shows_the_wait() {
        let saying = crate::i18n::tr("io-saving");
        let mut bad: Vec<String> = Vec::new();
        for (what, nav) in [
            ("a new document", Nav::New),
            ("a new assembly", Nav::NewAssembly),
            ("opening another project", Nav::OpenPath(String::new())),
            ("leaving the program", Nav::Exit),
        ] {
            let (mut app, path) = project_with_a_body(&format!("way-{}.qcad", what.replace(' ', "-")));
            // OPENING NEEDS SOMETHING TO OPEN: the document is written once, and that same file is what
            // the navigation will go to.
            let nav = match nav {
                Nav::OpenPath(_) => {
                    app.answer_save_for_test();
                    app.drain_bg_for_test();
                    app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "5".into(), value: 5.0 });
                    Nav::OpenPath(path.clone())
                }
                other => other,
            };
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);

            app.request_nav_for_test(nav);
            if !app.deferred_nav_is_set_for_test() {
                bad.push(format!("  {what}: the unsaved edits did not hold the navigation back — there was nothing to wait for"));
                continue;
            }
            app.answer_save_for_test();
            let _ = frame(&mut app, &ctx);
            app.age_save_wait_for_test(std::time::Duration::from_secs(1));
            let _ = frame(&mut app, &ctx);
            let texts = frame(&mut app, &ctx);
            if !texts.iter().any(|t| t.contains(&saying)) {
                bad.push(format!("  {what}: the frame stays silent while the write runs, drawn: {texts:?}"));
            }

            app.drain_bg_for_test();
            app.age_save_wait_for_test(std::time::Duration::from_secs(1));
            let _ = frame(&mut app, &ctx);
            let after = frame(&mut app, &ctx);
            if after.iter().any(|t| t.contains(&saying)) {
                bad.push(format!("  {what}: the waiting card stuck after the write"));
            }
            if app.deferred_nav_is_set_for_test() {
                bad.push(format!("  {what}: the navigation did not happen after the write — the command was lost"));
            }
        }
        assert!(bad.is_empty(), "not every way out of the document says what it is doing:\n{}", bad.join("\n"));
    }

    /// ONCE THE WRITE ENDS THE CARD GOES AWAY BY ITSELF (no flicker, no sticking).
    #[test]
    fn the_card_goes_away_by_itself() {
        let (mut app, _path) = project_with_a_body("gone.qcad");
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        app.request_nav_for_test(Nav::New);
        app.answer_save_for_test();
        app.drain_bg_for_test();
        app.age_save_wait_for_test(std::time::Duration::from_secs(1));
        let _ = frame(&mut app, &ctx); // the frame in which the navigation actually happens

        let texts = frame(&mut app, &ctx);
        let saying = crate::i18n::tr("io-saving");
        assert!(!texts.iter().any(|t| t.contains(&saying)), "the waiting card stuck after the write: {texts:?}");
    }

    /// AN INSTANT WRITE SHOWS NOTHING AT ALL.
    ///
    /// Reported requirement: the window appears ONLY if the saving really takes time — an instant one must
    /// not blink. A card up for a single frame is not an answer, it reads as a glitch.
    #[test]
    fn an_instant_write_does_not_blink() {
        let (mut app, _path) = project_with_a_body("blink.qcad");
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        app.request_nav_for_test(Nav::New);
        app.answer_save_for_test();
        // THE WRITE IS OVER BEFORE THE FIRST FRAME — that is what "instant" means here.
        app.drain_bg_for_test();

        let saying = crate::i18n::tr("io-saving");
        for _ in 0..4 {
            let texts = frame(&mut app, &ctx);
            assert!(!texts.iter().any(|t| t.contains(&saying)), "an instant write flashed the waiting card: {texts:?}");
        }
        assert!(!app.save_wait_shown_for_test(), "the card was counted as shown though nothing was drawn");
    }

    /// A CARD THAT WAS SHOWN STAYS LONG ENOUGH TO BE READ.
    ///
    /// The other half of the same rule: without a floor the card still blinks when the write ends a moment
    /// after the grace has passed.
    #[test]
    fn a_shown_card_is_not_snatched_away() {
        let (mut app, _path) = project_with_a_body("floor.qcad");
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let saying = crate::i18n::tr("io-saving");

        app.request_nav_for_test(Nav::New);
        app.answer_save_for_test();
        let _ = frame(&mut app, &ctx);
        app.age_save_wait_for_test(std::time::Duration::from_millis(200)); // past the grace, not past the floor
        let _ = frame(&mut app, &ctx);
        let texts = frame(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.contains(&saying)), "the card did not come up on a slow write: {texts:?}");

        // THE WRITE ENDS RIGHT AFTER IT CAME UP.
        app.drain_bg_for_test();
        let texts = frame(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.contains(&saying)), "the card was snatched away the moment the write ended: {texts:?}");

        // ONCE THE FLOOR HAS PASSED IT GOES BY ITSELF.
        app.age_save_wait_for_test(std::time::Duration::from_secs(1));
        let _ = frame(&mut app, &ctx);
        let texts = frame(&mut app, &ctx);
        assert!(!texts.iter().any(|t| t.contains(&saying)), "the card stuck after the floor had passed: {texts:?}");
    }

    /// One frame that reports WHERE the text landed, so a button can be pressed rather than merely found.
    fn frame_with_places(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |c| {
            app.nav_dialog_for_test(c);
        });
        let mut places = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut places);
        }
        places
    }

    /// "SAVE AND MOVE ON" ON A DOCUMENT THAT HAS NO NAME YET.
    ///
    /// Save As no longer holds the frame thread: it puts the chooser up and returns, and the write does
    /// not begin until a name is given. So at the moment the "save?" dialogue looks at the world there is
    /// no write running and the document is still dirty - which used to mean exactly one thing to it:
    /// throw the navigation away. The person would name the file, watch it save, and never get the
    /// document they asked to open.
    ///
    /// Pressed through a real frame, because it is the BUTTON's branch that decided to drop it. The
    /// chooser is armed by hand first: a test cannot answer a system window, and with the slot already
    /// taken the real Save As opens none - leaving exactly the state it would have left.
    #[test]
    fn a_document_with_no_name_still_gets_where_it_was_going() {
        let (mut app, _path) = project_with_a_body("unnamed.qcad");
        app.forget_project_path_for_test(); // never saved: Save turns into Save As
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);

        app.request_nav_for_test(Nav::New);
        assert!(app.deferred_nav_is_set_for_test(), "setup: unsaved edits held the navigation back");
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, |_app, _p| {});

        // the window settles on the second pass, as everywhere else here
        let _ = frame_with_places(&mut app, &ctx, Vec::new());
        let places = frame_with_places(&mut app, &ctx, Vec::new());
        // the button wears an icon in front of its name - matched by the name, with the icon dropped
        let bare = |t: &str| t.chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect::<String>().trim().to_string();
        let save = crate::i18n::tr("io-save");
        let spot = places
            .iter()
            .find(|(t, _)| bare(t) == save.trim())
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("the question has no Save button; it painted: {:?}", places.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()));
        let click = vec![
            egui::Event::PointerMoved(spot),
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ];
        let _ = frame_with_places(&mut app, &ctx, click);

        assert!(
            app.asking_for_a_file() && !app.saving_now_for_test(),
            "setup: pressing Save on an unnamed document must leave a chooser open and no write running"
        );
        let _ = frame_with_places(&mut app, &ctx, Vec::new());
        assert!(
            app.deferred_nav_is_set_for_test(),
            "the navigation was thrown away while the person was still choosing a name - they save the file and never get the document they asked for"
        );

        // WALKING AWAY FROM THE CHOOSER LEAVES THEM WHERE THEY WERE. No write happened, so the navigation
        // is cancelled - and the question is not put a second time, which would read as a loop.
        tx.send(None).expect("the chooser is listening");
        assert!(!app.poll_file_ask());
        let _ = frame_with_places(&mut app, &ctx, Vec::new());
        assert!(!app.deferred_nav_is_set_for_test(), "cancelling the chooser must leave the person where they were");
    }
}
