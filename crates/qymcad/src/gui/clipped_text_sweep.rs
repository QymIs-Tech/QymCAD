//! HOW MUCH OF THE INTERFACE IS PAINTED PAST ITS OWN EDGE.
//!
//! The parameter window said why an expression failed and painted the sentence 152 points past the edge it
//! was clipped to, so a person read half of it. That was found by eye, on a picture. The same class of defect
//! - text laid out whole and cut when painted - can hide on any surface, and no eye goes over all of them.
//!
//! This is the measurement, not a rule: it prints what sticks out and by how much, over every surface the
//! catalogue sweep already knows about. Some of it is legitimate (a name too long for a tree row is cut on
//! purpose, and the whole name is in the tooltip); some of it is a reason nobody can read. Telling one from
//! the other is a judgement, so the numbers come first.
//!
//! WHAT IT SAID WHEN IT WAS WRITTEN (25.08.2026), after the parameter window was fixed: ONE place over eleven
//! surfaces and six kinds of selection - the CAM tool library, where a tool's name sticks out 48 points. That
//! one is legitimate: the name lives in a TEXT FIELD, and a field scrolls its own content under the caret.
//! So the interface has no unreadable text left of the kind that was found by eye, and this file is the way
//! to check that again without going over the pictures by hand.
//!
//! It stays a measurement rather than becoming a rule, because a text field cannot be told from a label by
//! the shapes alone: a guard here would go red on the tool library and be silenced within the week.
#[cfg(test)]
mod tests {
    use crate::gui::WinKind;
    use crate::gui::{App, Sel};

    /// Text shapes of one frame, each with the rectangle it is clipped to.
    fn painted(app: &mut App, draw: impl Fn(&mut App, &mut egui::Ui)) -> Vec<(String, egui::Rect, egui::Rect)> {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let _ = ctx.run_ui(input.clone(), |c| draw(app, c));
        let out = ctx.run_ui(input, |c| draw(app, c));
        let mut found = Vec::new();
        for cs in &out.shapes {
            collect(&cs.shape, cs.clip_rect, &mut found);
        }
        found
    }

    fn collect(shape: &egui::epaint::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()), clip)),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, clip, out)),
            _ => {}
        }
    }

    #[test]
    #[ignore = "a measurement: prints the text painted past its own edge, surface by surface"]
    fn what_is_painted_past_its_own_edge() {
        type Surface = (&'static str, fn(&mut App, &mut egui::Ui));
        let surfaces: &[Surface] = &[
            ("tree", |a, c| a.tree_panel(c)),
            ("properties", |a, c| a.properties_panel(c)),
            ("menu", |a, c| { let mut asks = Vec::new(); crate::gui::panels_bars::menu_bar(&mut a.bar_ctx(&mut asks), c); let c = c.ctx().clone(); a.do_bar_asks(asks, &c); }),
            ("tool bar", |a, c| { let mut asks = Vec::new(); crate::gui::panels_bars::tool_options_bar(&mut a.bar_ctx(&mut asks), c); let c = c.ctx().clone(); a.do_bar_asks(asks, &c); }),
            ("command bar", |a, c| a.feat_command_bar(c)),
            ("settings", |a, c| { let mut asks = Vec::new(); crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), c); a.do_win_asks(asks, c); }),
            ("parameters", |a, c| { let mut asks = Vec::new(); crate::gui::panels_windows::params_window(&mut a.win_ctx(&mut asks), c); a.do_win_asks(asks, c); }),
            ("parts library", |a, c| { let mut asks = Vec::new(); crate::gui::panels_windows::parts_library_window(&mut a.win_ctx(&mut asks), c); a.do_win_asks(asks, c); }),
            ("hotkeys", |a, c| a.hotkeys_window(c)),
            ("about", |a, c| crate::gui::panels_windows::about_dialog(&mut a.win, &a.scheme, c)),
        ];
        let mut worst: Vec<(f32, String)> = Vec::new();
        for (name, draw) in surfaces {
            let mut app = crate::gui::screen_keys::tests::populated();
            app.win.open(WinKind::Settings);
            app.win.open(WinKind::Params);
            app.win.open(WinKind::PartsLibrary);
            app.win.open(WinKind::Hotkeys);
            app.win.open(WinKind::About);
            app.project.parameters.push(qymcad_core::model::Param { name: "bad".into(), expr: "w/".into(), value: 0.0 });
            app.project.eval_parameters();
            for sel in [Sel::None, Sel::Mesh(0), Sel::Face(0, 0), Sel::Sketch(0), Sel::Component(0), Sel::Feature(0)] {
                app.chosen.sel = sel;
                for (text, rect, clip) in painted(&mut app, *draw) {
                    let over = rect.max.x - clip.max.x;
                    // A point or two is the rounding of the layout, not a cut word.
                    if over > 2.0 && !text.trim().is_empty() {
                        worst.push((over, format!("{name}: {over:.0} points past the edge: {:?}", text.chars().take(60).collect::<String>())));
                    }
                }
            }
        }
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        worst.dedup_by(|a, b| a.1 == b.1);
        eprintln!("PAINTED PAST THE EDGE: {} places", worst.len());
        for (_, line) in worst.iter().take(40) {
            eprintln!("   {line}");
        }
    }
}
