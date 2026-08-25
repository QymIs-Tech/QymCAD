//! NOT ONE INTERNAL NAME REACHES THE SCREEN. Written from a reported screenshot.
//!
//! In the thread popup the captions read `f-nominal-d`, `f-pitch-std`, `f-length` — catalogue keys
//! instead of words. Not one of the localisation tests caught that, and none could: the keys WERE in
//! the catalogue and were translated correctly. The field was drawn past the translation —
//! `ui.label(p.label)` instead of `p.label()`, a difference of two brackets, and the compiler stayed
//! silent because `ui.label` accepts a `&str`.
//!
//! Hence the only way to check: BUILD A FRAME and look at what ended up in it. The same lesson had
//! already been recorded over the view cube — a widget that draws must have a test that DRAWS it;
//! checking the shape and checking the source are no substitute.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use super::super::{App, Sel};
    use crate::i18n;

    /// Every key of the catalogue — a leak is recognised by them. A string on screen that is LITERALLY
    /// equal to a key is an internal name that reached a person.
    pub(in crate::gui) fn catalogue_keys() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("i18n/en");
        let mut keys = Vec::new();
        for e in std::fs::read_dir(&dir).expect("the en directory reads").flatten() {
            if e.path().extension().is_none_or(|x| x != "ftl") {
                continue;
            }
            for line in std::fs::read_to_string(e.path()).expect("the catalogue file reads").lines() {
                if let Some((k, _)) = line.split_once(" = ") {
                    if k.starts_with(|c: char| c.is_ascii_lowercase()) && !k.contains(' ') {
                        keys.push(k.to_string());
                    }
                }
            }
        }
        keys
    }

    /// All the text of a shape of the frame, including what is nested inside composite ones.
    pub(in crate::gui) fn collect_text(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect_text(x, out)),
            _ => {}
        }
    }

    /// WHERE IN THE FRAME THE TEXT `needle` IS DRAWN — the middle of its caption.
    ///
    /// A means for checks from a person's side: a person looks for a button by eye, by its caption or
    /// its glyph, and clicks where it is drawn. Aiming "roughly there" has already been tried — the
    /// click went into empty space and the check declared broken what actually works.
    ///
    /// The first match, top to bottom in the order of the shapes of the frame.
    pub(in crate::gui) fn text_pos(s: &egui::epaint::Shape, needle: &str, out: &mut Option<egui::Pos2>) {
        match s {
            egui::epaint::Shape::Text(t) if out.is_none() && t.galley.text().contains(needle) => {
                *out = Some(t.pos + t.galley.size() * 0.5);
            }
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| text_pos(x, needle, out)),
            _ => {}
        }
    }

    /// A PLATE OF 40x30x10 by the same path a person takes: sketch, then extrude. The command popup is
    /// anchored to the geometry and does not open at all without a body — there would be nothing to
    /// check.
    pub(in crate::gui) fn plate() -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app
    }

    #[test]
    fn no_catalogue_key_ever_reaches_the_screen() {
        let keys = catalogue_keys();
        assert!(keys.len() > 500, "suspiciously few catalogue keys were collected: {}", keys.len());

        let mut app = plate();
        assert!(app.project.timeline.iter().any(|n| n.kind.body().is_some()), "the plate did not build — there is nothing to check: {}", app.status);

        let prev = i18n::language();
        let mut leaks: Vec<String> = Vec::new();
        let mut drawn = 0usize;
        for code in ["ru", "en"] {
            i18n::set_language(code);
            // the Part tools with fields at the geometry: fillet, chamfer, shell, hole, patterns
            for cmd in [4u8, 5, 6, 7, 17, 18] {
                app.start_feat_cmd(cmd);
                if app.cmd.params.is_empty() {
                    continue; // the tool did not open on this scene — it will have one of its own
                }
                let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
                let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
                let ctx = egui::Context::default();
                super::super::install_fonts(&ctx);
                // TWO PASSES: an `Area` learns its size only after the first layout, and on the first
                // frame the popup is not yet in place. The second one is what is looked at — the one a
                // person sees.
                let _ = ctx.run(input.clone(), |ctx| app.feat_cmd_popup(ctx, screen));
                let out = ctx.run(input, |ctx| app.feat_cmd_popup(ctx, screen));
                let mut texts = Vec::new();
                for cs in &out.shapes {
                    collect_text(&cs.shape, &mut texts);
                }
                drawn += texts.len();
                for t in texts {
                    if keys.iter().any(|k| *k == t) {
                        leaks.push(format!("{code}: tool {cmd} drew the key \"{t}\""));
                    }
                }
            }
        }
        i18n::set_language(&prev);
        assert!(drawn > 10, "the frame came out empty ({drawn} captions) — the test checked nothing");
        assert!(leaks.is_empty(), "an internal name reached the screen instead of words ({}):\n{}", leaks.len(), leaks.join("\n"));
    }

    /// A SCENE THE PANELS HAVE SOMETHING TO SHOW ON: a part with features, a sketch, a datum, a second
    /// part.
    ///
    /// An empty document is a poor check: on it half the panels draw a single line saying "select
    /// something in the tree", and a leak in a branch that was never reached goes unnoticed.
    pub(in crate::gui) fn populated() -> App {
        let mut app = plate();
        // a fillet, so that the timeline holds a modifier feature with a reference to edges
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().take(2).map(|e| e.id).collect()).unwrap_or_default();
        if !edges.is_empty() {
            app.project.add_fillet(body, 1.0, edges);
            app.rebuild_if_dirty();
        }
        // a datum plane and a second sketch — the "plane" and "sketch" branches of the properties
        app.project.add_offset_plane(qymcad_core::feature::BasePlane::XY, 12.0);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_circle_entity(si, 0.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.rebuild_if_dirty();
        app
    }

    /// A frame of a surface: built TWICE (the first pass lays out, the second is what a person sees)
    /// and all the text is collected.
    pub(in crate::gui) fn frame_text(app: &mut App, draw: impl Fn(&mut App, &egui::Context)) -> Vec<String> {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(input.clone(), |c| draw(app, c));
        let out = ctx.run(input, |c| draw(app, c));
        let mut texts = Vec::new();
        for cs in &out.shapes {
            collect_text(&cs.shape, &mut texts);
        }
        texts
    }

    /// THE WHOLE INTERFACE, NOT ONE POPUP: the tree, the properties, the settings, the bars, the menu,
    /// the windows.
    ///
    /// Extended on request: the guard covered only the command popup, and was asked to cover the rest
    /// of the windows so that leaks are found by a test rather than by a person. The list of surfaces
    /// is not "as many as there was time for" but everything that draws text and opens without a click
    /// on geometry.
    #[test]
    fn no_catalogue_key_reaches_the_screen_anywhere() {
        let keys = catalogue_keys();
        assert!(keys.len() > 500, "suspiciously few catalogue keys were collected: {}", keys.len());

        type Surface = (&'static str, fn(&mut App, &egui::Context));
        let surfaces: &[Surface] = &[
            ("tree", |a, c| a.tree_panel(c)),
            ("properties", |a, c| a.properties_panel(c)),
            ("menu", |a, c| a.menu_bar(c)),
            ("tool bar", |a, c| a.tool_options_bar(c)),
            ("command bar", |a, c| a.feat_command_bar(c)),
            ("section bar", |a, c| a.section_bar(c)),
            ("component pattern bar", |a, c| a.comp_array_bar(c)),
            ("settings", |a, c| a.settings_window(c)),
            ("parameters", |a, c| a.params_window(c)),
            ("parts library", |a, c| a.parts_library_window(c)),
            ("hotkeys", |a, c| a.hotkeys_window(c)),
            ("about", |a, c| a.about_dialog(c)),
            ("tools (CAM)", |a, c| a.tools_window(c)),
        ];

        let prev = i18n::language();
        let mut leaks: Vec<String> = Vec::new();
        let mut drawn = 0usize;
        for code in ["ru", "en"] {
            i18n::set_language(code);
            for (name, draw) in surfaces {
                let mut app = populated();
                // windows are drawn only when open, and CAM only with the module switched on
                app.win.settings = true;
                app.win.params = true;
                app.win.parts_library = true;
                app.win.hotkeys = true;
                app.win.about = true;
                app.win.tools = true;
                app.set.cam_tab_enabled = true;
                // the right panel must show EVERY kind of selection, not only "nothing is selected"
                let sels: Vec<(&str, Sel)> = vec![
                    ("nothing", Sel::None),
                    ("body", Sel::Mesh(0)),
                    ("face", Sel::Face(0, 0)),
                    ("sketch", Sel::Sketch(0)),
                    ("plane", Sel::Plane(0)),
                    ("component", Sel::Component(0)),
                    ("feature 0", Sel::Feature(0)),
                    ("feature 1", Sel::Feature(1)),
                ];
                for (sel_name, sel) in sels {
                    app.sel = sel;
                    for t in frame_text(&mut app, *draw) {
                        drawn += 1;
                        if keys.iter().any(|k| *k == t) {
                            let msg = format!("{code}: \"{name}\" ({sel_name}) drew the key \"{t}\"");
                            if !leaks.contains(&msg) {
                                leaks.push(msg);
                            }
                        }
                    }
                }
            }
        }
        i18n::set_language(&prev);
        assert!(drawn > 200, "the frames came out empty ({drawn} captions) — the test checked nothing");
        assert!(leaks.is_empty(), "an internal name reached the screen instead of words ({}):\n{}", leaks.len(), leaks.join("\n"));
    }
}
