//! THE COLOUR SCHEME — the path a person walks and a defence against the old ailments returning.
//!
//! Reported: the light theme does not repaint the viewport and the sketcher, which stay black, and
//! neither does the bar of the active tool. What is checked here is that this is no longer so, and that
//! the dark scheme stayed THE SAME when it moved into data — its look is liked, and "improving it along
//! the way" would be a redesign nobody asked for.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The luminance of a colour — a light background is told from a dark one by it, without being tied
    /// to exact numbers.
    fn luma(c: egui::Color32) -> f32 {
        0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
    }

    /// THE DARK SCHEME IS A TRANSFER, NOT A REDESIGN: the colours match the ones that were in the code.
    ///
    /// It was for the sake of this test that the transfer was done literal for literal. Should anybody
    /// one day "improve" a shade along with a refactoring, the test will call that a change rather than
    /// let it through silently.
    #[test]
    fn the_dark_scheme_is_a_transfer_not_a_redesign() {
        let d = crate::palette::dark();
        assert_eq!(d.viewport_bg, [26, 26, 26], "the background of the viewport was from_gray(26)");
        assert_eq!(d.toolbar_bg, [34, 40, 46], "the tool bar was from_rgb(34, 40, 46)");
        assert!(!d.light, "the dark scheme is not a light one");
    }

    /// THE LIGHT SCHEME IS REALLY LIGHT — exactly the report all this started from.
    #[test]
    fn the_light_scheme_actually_lightens_the_canvas() {
        let l = crate::palette::light();
        assert!(l.light, "the light scheme is marked as light");
        assert!(luma(l.viewport_bg()) > 200.0, "the background of the viewport must be LIGHT, and its luminance is {}", luma(l.viewport_bg()));
        assert!(luma(l.toolbar_bg()) > 180.0, "the tool bar must be light, and its luminance is {}", luma(l.toolbar_bg()));
        // and the dark one dark, otherwise the test means nothing
        let d = crate::palette::dark();
        assert!(luma(d.viewport_bg()) < 60.0, "a dark background must stay dark");
    }

    /// THE LINES STAY LEGIBLE AGAINST THEIR OWN BACKGROUND. A light scheme is not an inversion: the
    /// yellow sketch line of the dark scheme simply vanishes on white, so the light one has darker values
    /// of its own.
    #[test]
    fn lines_stay_legible_against_their_own_background() {
        for p in crate::palette::builtin() {
            let bg = luma(p.viewport_bg());
            for (what, c) in [
                ("the sketch line", p.sketch_line()),
                ("the construction geometry", p.sketch_construction()),
                ("the selection", p.selected()),
                ("the preview", p.preview()),
                ("the error", p.error()),
                ("the dimension", p.dimension()),
            ] {
                let diff = (luma(c) - bg).abs();
                assert!(diff > 45.0, "in the scheme \"{}\" {what} merges into the background (a luminance difference of {diff:.0})", p.name);
            }
        }
    }

    /// SWITCHING THE SCHEME CHANGES THE CANVAS, not only the buttons.
    ///
    /// The theme used to change the look of `egui` and NOT touch the canvas: the background was filled
    /// from a literal, past the theme. That is precisely the report that the viewport stays black.
    #[test]
    fn switching_the_scheme_repaints_the_canvas() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.set.scheme = "dark".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        let dark_bg = app.scheme.pal.viewport_bg();
        assert!(ctx.style_of(ctx.theme()).visuals.dark_mode, "the dark scheme sets the dark look of egui");

        app.set.scheme = "light".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        let light_bg = app.scheme.pal.viewport_bg();
        assert!(!ctx.style_of(ctx.theme()).visuals.dark_mode, "the light scheme sets the light look of egui");
        assert!(luma(light_bg) > luma(dark_bg) + 100.0, "the canvas must lighten: {} -> {}", luma(dark_bg), luma(light_bg));
    }

    /// AN UNKNOWN SCHEME does not leave the program without colours (a settings file from a future
    /// version).
    #[test]
    fn an_unknown_scheme_falls_back_to_dark() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.set.scheme = "neon-from-the-future".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        assert_eq!(app.scheme.pal.id, "dark", "an unknown scheme is brought to the dark one rather than to emptiness");
    }

    /// THE CANVAS AND THE TOOL BAR TAKE THEIR COLOUR FROM THE SCHEME — a guard against a return to
    /// those two places.
    #[test]
    fn the_canvas_and_toolbar_ask_the_scheme() {
        use crate::gui::render_source::dense;
        let gui = dense(include_str!("../gui.rs"));
        assert!(gui.contains(&dense("pal.viewport_bg()")), "the background of the canvas must come from the scheme");
        assert!(!gui.contains(&dense("rect_filled(rect, 0.0, Color32::from_gray(26))")), "there must be no literal background left");
        // IT TAKES THE SCHEME OUTRIGHT NOW, which is what "sees it" was always about. It used to be a
        // method on the application and the guard asked for `&self`; as a free function it names the
        // scheme in its own signature, and that says the same thing louder.
        // THE FRAME MOVED TO THE RECORDS CRATE with the scheme it reads; the rule did not move.
        let state = include_str!("../../../qymcad-ui-state/src/lib.rs");
        assert!(dense(state).contains(&dense("fn tool_bar_frame(scheme: &SchemeUi)")), "the tool bar must SEE the scheme - it takes it by argument");
        // THE NEEDLE IS THE PALETTE CALL, not the path to it. Inside the free function the scheme arrives
        // by argument and is spelled `scheme.pal`; in a method it was `self.scheme.pal`. What the guard is
        // about - the colour comes from the palette rather than from a literal - is the same either way.
        assert!(dense(state).contains(&dense("pal.toolbar_bg()")), "and take its colour from it");
    }

    /// CHANGING THE SCHEME INVALIDATES THE PICTURE CACHES.
    ///
    /// The raster of the viewport and the vertex buffer of the GPU are computed once and live until their
    /// key changes. The colour of the bodies is already baked into those buffers — so without the scheme
    /// in the key, switching the theme would leave the former picture on screen until the next edit of
    /// the geometry. Exactly the report that the theme does not repaint the viewport, from another
    /// side.
    #[test]
    fn switching_the_scheme_invalidates_the_picture_caches() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        app.set.scheme = "dark".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        let (raster_dark, gpu_dark) = (qymcad_ui_state::view_key(&app.painting(), rect, 1.0), qymcad_ui_state::gpu_scene_key(&app.painting()));

        app.set.scheme = "light".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        assert_ne!(raster_dark, qymcad_ui_state::view_key(&app.painting(), rect, 1.0), "the key of the raster must change together with the scheme");
        assert_ne!(gpu_dark, qymcad_ui_state::gpu_scene_key(&app.painting()), "the key of the vertex buffer must change together with the scheme");
    }

    /// THE SHADING OF BODIES GOES THROUGH THE SCHEME — both of its knobs, not one.
    ///
    /// The first attempt to mend "the parts are too dark" raised only the floor, and the answer was
    /// rightly that they had not become any lighter: the ceiling stayed the part's own colour. A guard
    /// against a repeat — both parameters must be read where a part is painted.
    #[test]
    fn body_shading_reads_both_knobs_from_the_scheme() {
        // THE SHADING MOVED TO THE PICKING CRATE with the questions it answers; the rule did not move -
        // both knobs still have to be read where a part is painted. The needle names the PALETTE by type
        // and not by the path it is reached through: the path changed twice already.
        let src = include_str!("../../../qymcad-pick/src/lib.rs");
        assert!(src.contains("pal.shade_floor_body"), "the floor of the shading must come from the scheme");
        assert!(src.contains("pal.body_lighten") && src.contains("pal.body_saturate"), "both the lightness and the saturation must come from the scheme: without saturation a part turns white rather than light");
        assert!(crate::gui::render_source::has(src, "fn shade_tri(pal: &") && src.contains("Palette"), "the shading must SEE the scheme (as an argument, since the function takes no &self)");
    }

    /// A SCHEME OF ONE'S OWN IS CHOSEN AND APPLIED ON EQUAL TERMS WITH THE BUILT-IN ONES.
    ///
    /// The scheme used to be looked for among THE BUILT-IN ones — one's own would not be found and would
    /// silently be brought to the dark one, that is, it could be created and not used.
    #[test]
    fn a_scheme_of_ones_own_is_selectable_like_any_other() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mut mine = crate::palette::light();
        mine.id = "my-test".into();
        mine.name = "My test scheme".into();
        mine.viewport_bg = [11, 22, 33];
        app.scheme.all.push(mine);

        app.set.scheme = "my-test".into();
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        assert_eq!(app.scheme.pal.name, "My test scheme", "a scheme of one's own is selectable");
        assert_eq!(app.scheme.pal.viewport_bg, [11, 22, 33], "and paints the canvas in its own colour");
    }

    /// EDITING A COLOUR IS SEEN AT ONCE AND DOES NOT MAKE THE PROJECT DIRTY.
    ///
    /// Choosing a shade blind (apply, close the window, look) is impossible, so the edit goes live. But it
    /// is an edit of THE LOOK and not of the document: asking "save the project?" after picking a colour
    /// is a lie.
    #[test]
    fn editing_a_colour_repaints_at_once_and_leaves_the_document_clean() {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        crate::gui::apply_theme(&mut app.scheme, &app.set, &ctx);
        let was_dirty = qymcad_ui_state::is_dirty(&mut app.rebuild_ctx());
        let key_before = qymcad_ui_state::view_key(&app.painting(), rect, 1.0);

        assert!(app.scheme.pal.set("sketch_line", [7, 8, 9]), "a colour is edited by name");
        assert_eq!(app.scheme.pal.sketch_line, [7, 8, 9]);
        assert_ne!(key_before, qymcad_ui_state::view_key(&app.painting(), rect, 1.0), "the picture must be recomputed");
        assert_eq!(qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), was_dirty, "picking a colour is not an edit of the document");
    }

    /// THE LIST OF SCHEMES SHOWS WORDS, NOT ICONS ALONE.
    ///
    /// Reported: under View -> Colour scheme only the icons are visible with no text at all. And so it
    /// was: after the split into "a stable identifier" and "a caption", the menu stayed on the `name`
    /// field, and a built-in scheme has NO name of its own — it comes from the language catalogue through
    /// `title()`.
    #[test]
    fn the_scheme_list_shows_words_not_just_icons() {
        let prev = crate::i18n::language();
        for (code, _) in crate::i18n::available() {
            crate::i18n::set_language(&code);
            for p in crate::palette::builtin() {
                assert!(p.name.is_empty(), "a built-in scheme has no name of its own — the caption comes from the catalogue");
                let title = p.title();
                assert!(!title.trim().is_empty() && title != format!("scheme-{}", p.id), "in language {code} the scheme \"{}\" has no caption: {title}", p.id);
            }
        }
        crate::i18n::set_language(&prev);

        // and both places where the list is drawn ask for the caption itself
        let panels = crate::gui::panels_source::PANELS;
        // THE ROWS OF THE LIST ARE COUNTED, NOT EVERY MENTION OF `title()`. The former edition compared
        // the total number of occurrences with two and turned red at an edit in a NEIGHBOURING function
        // (choosing a free caption for a copy) — that is, it counted something other than what it is
        // written about. The list of schemes is built by exactly two identical lines: the View menu and
        // the settings window.
        let rows = panels.matches("(p.id.clone(), p.title(), p.light)").count();
        assert_eq!(rows, 2, "both the View menu and the settings window must take the caption from title(), and there are {rows} such lists");
        assert!(!panels.contains("format!(\"{icon}  {}\", p.name)"), "the caption of a scheme no longer comes from the empty name");
    }

    /// A MENU ITEM NEVER WRAPS.
    ///
    /// Reported: the wrapping in the top menu broke. The width of a drop-down menu is computed from its
    /// contents, and when the list of schemes turned out to be icons alone the menu narrowed — and a
    /// two-word command spread over two lines. A menu item is a command, not a paragraph.
    #[test]
    fn menu_items_never_wrap() {
        let panels = crate::gui::panels_source::PANELS;
        let from = panels.find("fn menu_bar").expect("the menu is in place");
        let to = panels[from..].find("\n    pub(super) fn ").map(|i| from + i).unwrap_or(panels.len());
        let bar = &panels[from..to];
        // EVERY MENU IS CHECKED SEPARATELY RATHER THAN COUNTS COMPARED. The former version counted all
        // the `|ui| {` against all the bans and turned red at any wrapper (`add_enabled_ui`) that is not a
        // drop-down menu at all. An equality of TOTALS is a poor guard in general: it also adds up when
        // one menu is left without its ban and another gets a spare one.
        let mut naked: Vec<String> = Vec::new();
        let mut rest = bar;
        while let Some(i) = rest.find("menu_button(") {
            let after = &rest[i..];
            let head: String = after.chars().take(60).collect();
            // the body of a menu starts right after `|ui| {`; the ban must stand on the very first line
            let body = after.find("|ui| {").map(|b| &after[b..(b + 220).min(after.len())]).unwrap_or("");
            if !body.contains("TextWrapMode::Extend") {
                naked.push(head.replace('\n', " "));
            }
            rest = &after[12..];
        }
        assert!(
            naked.is_empty(),
            "wrapping is not forbidden in a drop-down menu ({}) — a menu item is a command, not a paragraph:\n{}",
            naked.len(),
            naked.join("\n")
        );
    }

    /// NOT ONE COLOUR WRITTEN AS A NUMBER IN THE INTERFACE LAYER.
    ///
    /// This is the main guard of the whole idea. A scheme governs the look exactly as far as the code
    /// asks it; one `from_rgb(250, 200, 90)` typed out of habit in a new tool and the light scheme shows
    /// a dark patch again, exactly as in the original report.
    ///
    /// The tree is read FROM DISK rather than through `include_str!` over a list of files: the list would
    /// have to be topped up by hand, and the very first new file would slip past the check unnoticed.
    ///
    /// WHAT IS ALLOWED and why it is not a loophole: expressions with no numeric colour — `Color32::WHITE`
    /// as a "do not tint" multiplier, `TRANSPARENT` when creating a buffer of pixels, the shading of a
    /// body's own colour (`base[0] as f32 * lit`). None of the three is a choice of colour — the scheme
    /// has nothing to say there.
    #[test]
    fn no_colour_is_written_as_a_number_in_the_ui() {
        // EVERY CRATE. This walked the `src` of the crate it lives in, which was the whole interface until
        // the workbenches moved out; after that it went on reporting a clean interface while looking at a
        // shrinking part of one - the blindness recorded as D20. Widened: nothing numeric was hiding behind
        // it, which is the answer the check exists to give rather than a reason not to ask.
        let root = qymcad_i18n::ratchet::crates_root();
        let mut found: Vec<String> = Vec::new();
        let mut files = 0usize;
        let mut stack = qymcad_i18n::ratchet::every_crate_src();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the source directory reads") {
                let path = e.expect("an entry of the directory").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                // THE SCHEME ITSELF is the one place where a colour is supposed to be a number - the whole
                // crate of it, not only the files that were called `palette` while it lived in the
                // application. Its own tables carry `from_gray(26)` in trailing comments saying where each
                // colour came from, and those would otherwise be reported as the very thing they document.
                if path.starts_with(root.join("qymcad-scheme")) || path.starts_with(root.join("qymcad/src/palette")) || path.ends_with("palette.rs") || path.ends_with("scheme_flow.rs") {
                    continue;
                }
                files += 1;
                let text = std::fs::read_to_string(&path).expect("the source file reads");
                for (i, line) in text.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue; // a comment explaining HOW IT USED TO BE is not code
                    }
                    for ctor in ["from_rgb(", "from_gray(", "from_rgba_unmultiplied(", "from_rgba_premultiplied(", "from_black_alpha(", "from_white_alpha("] {
                        let mut at = 0;
                        while let Some(k) = line[at..].find(ctor) {
                            let arg = &line[at + k + ctor.len()..];
                            if arg.trim_start().starts_with(|c: char| c.is_ascii_digit()) {
                                let rel = path.strip_prefix(&root).unwrap_or(&path).display();
                                found.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                            }
                            at += k + ctor.len();
                        }
                    }
                }
            }
        }
        assert!(files > 5, "the check was supposed to read something, and it read {files} files");
        assert!(found.is_empty(), "a colour is given as a number past the scheme ({} of them):\n{}", found.len(), found.join("\n"));
    }

    /// THE SCHEME SURVIVES A RESTART — it is a setting, not an action.
    #[test]
    fn the_scheme_survives_a_restart() {
        let mut app = App::default();
        app.set.scheme = "light".into();
        let saved = ron::ser::to_string(&app.set).expect("saved");
        let mut fresh = App::default();
        fresh.set = ron::from_str(&saved).expect("loaded");
        assert_eq!(fresh.set.scheme, "light", "the chosen scheme must survive a restart");
    }
}
