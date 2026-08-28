//! THE PROGRAM DOES NOT ANSWER WHILE THE SYSTEM IS ASKING FOR A FILE.
//!
//! The file chooser stopped holding the frame thread, and that is what it was for - but it left the window
//! behind it not merely drawing but LISTENING. What was asked for: while a system dialogue is up, the
//! interface must be inactive until it is closed. Otherwise "new project" can be started under an open
//! "open project", the help can be opened over it, and the answer to the chooser arrives in a document
//! that is no longer the one it was opened from.
//!
//! Checked both ways round in one go: the same key, first with no chooser (it must work) and then with one
//! (it must not). A check that only ever presses the key while the chooser is up would pass just as well
//! against a program where that key never did anything.
#[cfg(test)]
mod tests {
    use crate::gui::App;

    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    /// One whole frame of the program, with `events` delivered into it. Returns what it painted, with the
    /// icons stripped off the labels.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => {
                    // the phosphor icons live in the private-use block and are not part of a label
                    let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                    out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
                }
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            events,
            ..Default::default()
        };
        let out = ctx.run_ui(raw, |ui| app.draw_frame(ui));
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    }

    /// Click at `spot`, as a pointer does.
    fn click(spot: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(spot),
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ]
    }

    /// Where a label was painted, if it was.
    fn spot_of(texts: &[(String, egui::Rect)], label: &str) -> Option<egui::Pos2> {
        texts.iter().find(|(t, _)| t == label).map(|(_, r)| r.center())
    }

    fn press(key: egui::Key) -> Vec<egui::Event> {
        vec![
            egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() },
            egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() },
        ]
    }

    /// A program past its start-up splash, with frames already running.
    fn running() -> (App, egui::Context) {
        let mut app = App::default();
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        app.clear_splash_for_test();
        for _ in 0..3 {
            frame(&mut app, &ctx, Vec::new()); // the splash and the areas settle
        }
        (app, ctx)
    }

    /// F1 IS THE HELP, AND UNDER AN OPEN CHOOSER IT IS NOTHING.
    #[test]
    fn a_key_press_behind_the_chooser_does_nothing() {
        let (mut app, ctx) = running();

        // FIRST WITHOUT ONE: the key has to do its job, or what follows proves nothing.
        frame(&mut app, &ctx, press(egui::Key::F1));
        assert!(app.help_is_open_for_test(), "setup: F1 must open the help when nothing is in the way");
        app.close_help_for_test();
        frame(&mut app, &ctx, Vec::new());
        assert!(!app.help_is_open_for_test(), "setup: and close again");

        // NOW WITH ONE.
        let (_tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, |_app, _p| {});
        frame(&mut app, &ctx, press(egui::Key::F1));
        assert!(
            !app.help_is_open_for_test(),
            "the help opened over an open file chooser - the interface is meant to be deaf until the system window is answered"
        );
    }

    /// A CLICK BEHIND THE CHOOSER OPENS NO MENU. The keys are read straight off the context and are held
    /// back by hand; the widgets are held back by the barrier and by the frame being disabled, which is a
    /// different mechanism and needs an answer of its own.
    ///
    /// The menu is the case that was named: "new project" must not be reachable under an open "open
    /// project". So the File menu is opened for real first, to be sure the click lands where it is aimed.
    #[test]
    fn a_click_behind_the_chooser_opens_no_menu() {
        let (mut app, ctx) = running();
        let file = crate::i18n::tr("menu-file");
        let new_project = crate::i18n::tr("file-new");

        // FIRST WITHOUT A CHOOSER: the click has to open the menu, or what follows proves nothing.
        let texts = frame(&mut app, &ctx, Vec::new());
        let at = spot_of(&texts, &file).unwrap_or_else(|| panic!("the menu bar has no \"{file}\"; painted: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()));
        let texts = frame(&mut app, &ctx, click(at));
        let texts = if spot_of(&texts, &new_project).is_some() { texts } else { frame(&mut app, &ctx, Vec::new()) };
        assert!(spot_of(&texts, &new_project).is_some(), "setup: clicking \"{file}\" must open the menu; painted: {:?}", texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>());

        // close it again and check it really closed
        let _ = frame(&mut app, &ctx, click(egui::pos2(SCREEN.x - 40.0, SCREEN.y - 40.0)));
        let texts = frame(&mut app, &ctx, Vec::new());
        assert!(spot_of(&texts, &new_project).is_none(), "setup: the menu would not close");

        // NOW WITH ONE.
        let (_tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, |_app, _p| {});
        let _ = frame(&mut app, &ctx, click(at));
        let texts = frame(&mut app, &ctx, Vec::new());
        assert!(
            spot_of(&texts, &new_project).is_none(),
            "the File menu opened under an open file chooser - \"{new_project}\" is one click away from replacing the document the chooser was opened from"
        );
        assert!(app.asking_for_a_file(), "and the chooser is still the one being answered");
    }

    /// THE FRAME COMES BACK TO LIFE once the chooser is answered. A barrier that outlives what it was put up
    /// for is a program that has quietly stopped working.
    #[test]
    fn answering_the_chooser_gives_the_program_back() {
        let (mut app, ctx) = running();
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, |_app, _p| {});
        frame(&mut app, &ctx, press(egui::Key::F1));
        assert!(!app.help_is_open_for_test(), "setup: deaf while the chooser is up");

        tx.send(None).expect("the chooser is listening"); // walked away from it
        frame(&mut app, &ctx, Vec::new());
        assert!(!app.asking_for_a_file(), "setup: the chooser is done with");

        frame(&mut app, &ctx, press(egui::Key::F1));
        assert!(app.help_is_open_for_test(), "the interface stayed deaf after the chooser was gone");
    }
}
