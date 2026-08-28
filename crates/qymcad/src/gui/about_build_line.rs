//! THE WINDOW MUST SAY WHICH BUILD THIS IS, AND HAND IT OVER READY TO PASTE.
//!
//! The first question asked of any complaint is what it was built from, and with a build a day the
//! version number alone names a whole week of binaries. So the About window carries the commit beside
//! the version, and a button that copies the whole block: nobody retypes a hash by eye, and a hash
//! retyped by eye is worse than none.
//!
//! Measured through a real frame rather than by calling the handler: the button is found among the
//! shapes the window actually painted, and clicked with pointer events. A direct call would skip the
//! frame's own reading of the input and prove nothing about what a person can reach.
#[cfg(test)]
mod tests {
    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    fn raw() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            ..Default::default()
        }
    }

    /// Every text shape of a frame, with where it was painted.
    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

    #[test]
    fn the_about_window_names_the_build() {
        let mut app = crate::gui::screen_keys::tests::populated();
        app.win.about = true;

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(raw(), |c| app.about_dialog(c)); // the first frame lays the window out
        let out = ctx.run_ui(raw(), |c| app.about_dialog(c.ctx()));

        let painted = texts(&out.shapes);
        let line = crate::build_info::line();
        assert!(
            painted.iter().any(|(t, _)| t.contains(&line)),
            "the About window does not show the build {line:?}; it painted: {:?}",
            painted.iter().map(|(t, _)| t.chars().take(40).collect::<String>()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_button_hands_the_details_over() {
        let mut app = crate::gui::screen_keys::tests::populated();
        app.win.about = true;

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(raw(), |c| app.about_dialog(c.ctx()));
        let out = ctx.run_ui(raw(), |c| app.about_dialog(c.ctx()));

        // The copy button carries the icon glyph as its whole label, so it is found by that glyph.
        let icon = egui_phosphor::regular::COPY;
        let spot = texts(&out.shapes)
            .into_iter()
            .find(|(t, _)| t.trim() == icon)
            .map(|(_, r)| r.center())
            .expect("the About window has no copy button");

        let press = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(spot),
                egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            ],
            ..raw()
        };
        let _ = ctx.run_ui(press, |c| app.about_dialog(c.ctx()));
        let release = egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: spot,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            ..raw()
        };
        let out = ctx.run_ui(release, |c| app.about_dialog(c.ctx()));

        // THE CLIPBOARD IS A COMMAND NOW, not a field: since egui 0.30 the frame reports what it did as a
        // list, and copying is one entry in it. Reading the old field would have silently found nothing.
        let copied: String = out
            .platform_output
            .commands
            .iter()
            .find_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
            .unwrap_or_default();
        assert!(!copied.is_empty(), "the click copied nothing");
        assert!(copied.starts_with("QymCAD "), "the copied block does not name the program: {copied:?}");
        assert!(
            copied.contains(crate::build_info::version()),
            "the copied block does not carry the version: {copied:?}"
        );
        assert!(copied.contains("OS: "), "the copied block does not carry the system: {copied:?}");
        // IT GOES INTO A PUBLIC TRACKER. A build path carries the name of whoever built it.
        assert!(!copied.contains("/home/") && !copied.contains("C:\\"), "the copied block carries a personal path: {copied:?}");
    }
}
