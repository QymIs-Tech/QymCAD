//! A CRASH FILE NOBODY IS TOLD ABOUT IS THE SAME AS NO CRASH FILE.
//!
//! The reports are written into the data directory of the program — a place no person has heard of and
//! none will look in. So the next start says the last run ended in an error and hands the path over.
//!
//! Shown ONCE: after it is closed the file is renamed rather than deleted, because the person may still
//! want to attach it, and a window that returns every start is a window people learn to dismiss without
//! reading.
#[cfg(test)]
mod tests {
    const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

    fn raw() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
            ..Default::default()
        }
    }

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
    fn the_next_start_says_the_last_run_crashed() {
        let dir = std::env::temp_dir().join(format!("qymcad-notice-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the directory is made");
        let report = dir.join("crash_1756150000.txt");
        std::fs::write(&report, "QymCAD 0.0.0\nPanic: a wall fell over\n").expect("the report is written");

        let mut app = crate::gui::screen_keys::tests::populated();
        app.crash_report = Some(report.clone());

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run(raw(), |c| app.crash_notice(c));
        let out = ctx.run(raw(), |c| app.crash_notice(c));

        let painted = texts(&out.shapes);
        let title = crate::i18n::tr("crash-title");
        assert!(painted.iter().any(|(t, _)| t.contains(&title)), "the window does not say the last run crashed");
        assert!(
            painted.iter().any(|(t, _)| t.contains("crash_1756150000.txt")),
            "the window does not say where the file is; it painted: {:?}",
            painted.iter().map(|(t, _)| t.chars().take(50).collect::<String>()).collect::<Vec<_>>()
        );

        // CLOSING IT MARKS IT SEEN. Clicked through a real frame: the button has to be reachable, not
        // merely present in the code.
        let close = crate::i18n::tr("close");
        let spot = painted
            .iter()
            .find(|(t, _)| t.trim() == close.trim())
            .map(|(_, r)| r.center())
            .expect("the window has no close button");
        let press = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(spot),
                egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            ],
            ..raw()
        };
        let _ = ctx.run(press, |c| app.crash_notice(c));
        let release = egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: spot,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            ..raw()
        };
        let _ = ctx.run(release, |c| app.crash_notice(c));

        assert!(app.crash_report.is_none(), "the window stayed open after it was closed");
        assert!(!report.exists(), "the report was not marked seen");
        assert!(dir.join("crash_1756150000.seen.txt").exists(), "marking it seen deleted the report instead of renaming it");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
