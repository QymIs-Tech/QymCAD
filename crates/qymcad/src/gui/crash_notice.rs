//! A CRASH FILE NOBODY IS TOLD ABOUT IS THE SAME AS NO CRASH FILE.
//!
//! The reports are written into the data directory of the program — a place no person has heard of and
//! none will look in. So the next start says the last run ended in an error and hands the path over.
//!
//! Shown ONCE: after it is closed the file is renamed rather than deleted, because the person may still
//! want to attach it, and a window that returns every start is a window people learn to dismiss without
//! reading.
//!
//! ONCE MEANS FOR ALL OF THEM. The start used to pick up a single report - the newest unseen one - so
//! two crashes meant two starts and two windows, and closing the first only brought the second. Measured
//! on the reporter's own directory: two files, two launches, and both renamed to `.seen.txt` one launch
//! apart, which is exactly what "it comes up every time" looks like from the outside.
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
        app.disk.crash_report = vec![report.clone()];

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(raw(), |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));
        let out = ctx.run_ui(raw(), |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));

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
        let _ = ctx.run_ui(press, |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));
        let release = egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: spot,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            ..raw()
        };
        let _ = ctx.run_ui(release, |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));

        assert!(app.disk.crash_report.is_empty(), "the window stayed open after it was closed");
        assert!(!report.exists(), "the report was not marked seen");
        assert!(dir.join("crash_1756150000.seen.txt").exists(), "marking it seen deleted the report instead of renaming it");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CLOSING IT ONCE CLOSES IT FOR ALL OF THEM.
    ///
    /// Reported behaviour: "however many times you close the window at startup, it comes up again every
    /// time saying there was a crash".
    ///
    /// The start picked up ONE report - the newest of the unseen ones - so three reports meant three
    /// starts and three windows, each closed and each followed by the next. Measured on the reporter's
    /// own directory: two files, two launches, two windows, and both ended up renamed to `.seen.txt` one
    /// launch apart. That is the very thing the note at the top of this module warns against: a window
    /// that returns every start is a window people learn to dismiss without reading.
    #[test]
    fn closing_it_once_answers_for_every_report_waiting() {
        let dir = std::env::temp_dir().join(format!("qymcad-notice-many-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the directory is made");
        let reports: Vec<std::path::PathBuf> = ["crash_1756150001.txt", "crash_1756150002.txt", "crash_1756150003.txt"]
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, "QymCAD 0.0.0\nPanic: a wall fell over\n").expect("the report is written");
                p
            })
            .collect();

        let mut app = crate::gui::screen_keys::tests::populated();
        app.disk.crash_report = reports.clone();

        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(raw(), |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));
        let out = ctx.run_ui(raw(), |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));

        let painted = texts(&out.shapes);
        let close = crate::i18n::tr("close");
        let spot = painted
            .iter()
            .find(|(t, _)| t.trim() == close.trim())
            .map(|(_, r)| r.center())
            .expect("the window has no close button");
        for pressed in [true, false] {
            let ev = egui::RawInput {
                events: vec![egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }],
                ..raw()
            };
            let _ = ctx.run_ui(ev, |c| crate::gui::panels_windows::crash_notice(&mut app.disk.crash_report, c.ctx()));
        }

        let still_waiting: Vec<&std::path::PathBuf> = reports.iter().filter(|p| p.exists()).collect();
        assert!(
            still_waiting.is_empty(),
            "the window was closed once and {} report(s) are still unseen, so the next start shows it again: {still_waiting:?}",
            still_waiting.len()
        );
        for p in &reports {
            assert!(p.with_extension("seen.txt").exists(), "marking it seen deleted the report instead of renaming it: {}", p.display());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}