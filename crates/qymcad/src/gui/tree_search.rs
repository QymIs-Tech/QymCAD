//! THE SEARCH OVER THE BUILD TREE.
//!
//! On a part of fifty-odd features the one wanted cannot be found by eye. The search goes by THE SAME
//! label a person sees in the row: the label is moved out into `feature_row_label` and is one and the
//! same for everybody who shows it or searches by it. Separate them and the search will start failing to
//! find what is shown; that class of divergence has already been caught twice (the localisation keys,
//! the settings table).
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A plate with an extrusion and a fillet — two different features, so the search has something to
    /// tell apart.
    fn part(app: &mut App) {
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
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().take(2).map(|e| e.id).collect()).unwrap_or_default();
        if !edges.is_empty() {
            app.project.add_fillet(body, 2.0, edges);
            app.rebuild_if_dirty();
        }
    }

    /// The indices of the timeline rows that fall under the search.
    fn matching(app: &App) -> Vec<usize> {
        (0..app.project.timeline.len()).filter(|&ti| !app.feature_row_label(ti).is_empty() && app.tree_row_matches(ti)).collect()
    }

    /// AN EMPTY QUERY SHOWS EVERYTHING — the search must not hide anything of its own accord.
    #[test]
    fn an_empty_query_hides_nothing() {
        let mut app = App::default();
        part(&mut app);
        let all = matching(&app);
        assert!(all.len() >= 2, "setup: at least two features were expected, and out came {}", all.len());
        app.set_tree_search_for_test("   ");
        assert_eq!(matching(&app), all, "a query of spaces must show everything rather than hide the tree");
    }

    /// THE SEARCH FINDS A FEATURE BY ITS LABEL — the one visible in the row.
    #[test]
    fn it_finds_a_feature_by_the_label_the_user_sees() {
        let mut app = App::default();
        part(&mut app);
        let rows = matching(&app);
        let sample = rows.iter().copied().find(|&ti| !app.feature_row_label(ti).is_empty()).expect("at least one row");
        let label = app.feature_row_label(sample);
        // a word from the label is taken — that is how people type
        let word = label.split_whitespace().find(|w| w.chars().any(|c| c.is_alphabetic())).expect("a word in the label").to_string();

        app.set_tree_search_for_test(&word);
        let got = matching(&app);
        assert!(got.contains(&sample), "the feature \"{label}\" was not found by the word \"{word}\" from its own label");
    }

    /// CASE DOES NOT MATTER: nobody will type a capital letter for the sake of the search.
    #[test]
    fn the_search_ignores_case() {
        let mut app = App::default();
        part(&mut app);
        let sample = *matching(&app).first().expect("at least one row");
        let label = app.feature_row_label(sample);
        let word = label.split_whitespace().find(|w| w.chars().any(|c| c.is_alphabetic())).expect("a word").to_string();

        app.set_tree_search_for_test(&word.to_uppercase());
        assert!(matching(&app).contains(&sample), "the search for \"{}\" did not find \"{label}\" — case must not get in the way", word.to_uppercase());
    }

    /// THE SEARCH ALSO FINDS BY THE NAME A PERSON GAVE, not only by the automatic label.
    #[test]
    fn it_finds_a_feature_by_the_name_the_user_gave_it() {
        let mut app = App::default();
        part(&mut app);
        let ti = *matching(&app).first().expect("at least one row");
        app.project.timeline[ti].name = "Gearbox cover".into();

        app.set_tree_search_for_test("gearbox");
        let got = matching(&app);
        assert!(got.contains(&ti), "a feature named by a person must be found by their word");
        assert_eq!(got.len(), 1, "that word must find exactly it, and {} were found", got.len());
    }

    /// A QUERY WITH NO MATCHES HIDES EVERYTHING — and that is more honest than showing "something
    /// similar".
    #[test]
    fn a_query_that_matches_nothing_shows_nothing() {
        let mut app = App::default();
        part(&mut app);
        app.set_tree_search_for_test("zzqqxx");
        assert!(matching(&app).is_empty(), "for nonsense the tree must be empty rather than show \"something similar\"");
    }

    /// THE SEARCH AND THE TREE ROW LOOK AT ONE LABEL.
    ///
    /// A guard over the source: let the label be assembled in the row by code of its own and the search
    /// will start failing to find what is shown, and it will be noticed by a person rather than by a
    /// test.
    #[test]
    fn the_row_and_the_search_read_the_same_label() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]\nmod ").next().expect("the working part");
        let row = code.find("fn tree_feature_row").expect("the tree row is in place");
        // the boundary is taken at the NEXT function rather than as a window of N characters: a window
        // drifts into neighbouring code and starts judging it — the first version turned red on somebody
        // else's `match` for exactly that reason.
        let end = code[row + 10..].find("\n    pub(super) fn ").map(|i| row + 10 + i).unwrap_or(code.len());
        let body = &code[row..end];
        assert!(body.contains("self.feature_row_label(ti)"), "the tree row must take its label from `feature_row_label` — the search goes by the same one");
        assert!(!body.contains("match kind {"), "the label is being assembled in the row by its own code again — the search will diverge from the tree");
    }

    /// THE PANEL DOES NOT GROW FROM WHAT IS TYPED INTO THE SEARCH. Written from reported behaviour.
    ///
    /// A query is started in the search line and the panel smoothly drifts to the right, squeezing
    /// everything out of the window. The cause is feedback, classic for immediate mode: the width of the
    /// field was taken as `available_width()`, that is, from the width of the panel of the PREVIOUS
    /// frame. The field demands that width -> the contents of the panel become wider by an icon and a
    /// button -> the panel grows -> on the next frame `available_width()` is larger -> and so on every
    /// frame, "smoothly".
    ///
    /// So what is measured is not the layout but the CONSEQUENCE: a dozen frames are run and the width
    /// of the panel is required not to change. Such a test does not depend on how exactly the field asks
    /// for its place.
    #[test]
    fn typing_in_the_search_never_widens_the_panel() {
        let mut app = App::default();
        part(&mut app);
        let root = app.project.root;
        let part_id = app.project.components.iter().find(|c| c.parent == Some(root)).map(|c| c.id).expect("the part");
        app.enter_ctx_for_test(part_id);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let width = |app: &mut App| -> f32 {
            let _ = ctx.run(input.clone(), |c| app.tree_panel(c));
            egui::panel::PanelState::load(&ctx, egui::Id::new("tree")).map(|p| p.rect.width()).unwrap_or(0.0)
        };

        let base = width(&mut app);
        assert!(base > 50.0, "setup: the tree panel must have a width, and out came {base}");

        // the query is typed letter by letter, as a person does, and a frame is built after each letter
        for ch in "extrusion".chars() {
            let mut q = app.tree_search_for_test();
            q.push(ch);
            app.set_tree_search_for_test(&q);
            let w = width(&mut app);
            assert!(
                w <= base + 1.0,
                "the tree panel has drifted: it was {base}, it became {w} after \"{q}\" — the width of the field must not depend on the width of the panel"
            );
        }
        // and it does not grow from repeated frames alone either
        for _ in 0..10 {
            let w = width(&mut app);
            assert!(w <= base + 1.0, "the panel grows by itself from frame to frame: it was {base}, it became {w}");
        }
    }

    /// THE SEARCH FIELD CAN BE TYPED INTO. Written from a second report.
    ///
    /// Reported behaviour: the field is not active at all any more, nothing can be entered into it, as
    /// if it were not there. While mending the drifting panel the field was asked for a width of zero —
    /// "let it ask for the minimum". And zero is what it became: the line is there and cannot be reached.
    ///
    /// The check guards BOTH boundaries at once: the field must be wide enough to aim at and no wider
    /// than the panel, otherwise the drift comes back. Between them lives the right answer.
    #[test]
    fn the_search_field_is_wide_enough_to_type_in() {
        let mut app = App::default();
        part(&mut app);
        let root = app.project.root;
        let part_id = app.project.components.iter().find(|c| c.parent == Some(root)).map(|c| c.id).expect("the part");
        app.enter_ctx_for_test(part_id);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run(input.clone(), |c| app.tree_panel(c));
        let _ = ctx.run(input, |c| app.tree_panel(c));

        let field = ctx.read_response(egui::Id::new("tree_search_field")).expect("the search field must be in the frame");
        let panel = egui::panel::PanelState::load(&ctx, egui::Id::new("tree")).map(|p| p.rect.width()).unwrap_or(0.0);
        assert!(field.rect.width() >= 80.0, "a search field {} px wide cannot be hit with a cursor — there will be nowhere to type", field.rect.width());
        assert!(field.rect.width() <= panel, "the field is wider than the panel ({} > {panel}) — the panel will start drifting", field.rect.width());
        assert!(field.sense.click, "the search field must accept a click, otherwise there is no getting into it");
    }

    /// THE SEARCH COVERS EVERY SECTION OF THE TREE, NOT ONE. Written from reported behaviour.
    ///
    /// It was reported that the search looks only for features in the Part workbench and does not look
    /// for the parts and subassemblies of the current assembly, so what comes out is not a search at
    /// all. Fair enough: the first version filtered only the bodies-and-build section. A search that
    /// looks in one section out of five is WORSE than none — it looks as if it works and therefore
    /// misleads.
    ///
    /// The check goes BY THE DRAWING and not by the predicate: a section could be filtered and still be
    /// drawn past the filter — which is exactly how the first version lived.
    #[test]
    fn the_search_covers_every_section_of_the_tree() {
        let mut app = App::default();
        // an assembly with two parts: exactly what could not be found
        let root = app.project.root;
        // `add_part` puts the part into the ACTIVE context, and in a new document the part already
        // created is the active one — without this both new ones would end up nested inside it rather
        // than beside it.
        app.project.set_active_component(Some(root));
        let arm = app.project.add_part("Arm Body");
        let wheel = app.project.add_part("Wheel");
        assert!(arm != 0 && wheel != 0, "setup: the parts are created");
        app.enter_ctx_for_test(root);

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let shown = |app: &mut App| -> Vec<String> {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run(input.clone(), |c| app.tree_panel(c));
            let out = ctx.run(input.clone(), |c| app.tree_panel(c));
            let mut t = Vec::new();
            for cs in &out.shapes {
                collect(&cs.shape, &mut t);
            }
            t
        };

        let all = shown(&mut app);
        assert!(all.iter().any(|s| s.contains("Arm Body")), "setup: the part \"Arm Body\" must be in the tree: {all:?}");
        assert!(all.iter().any(|s| s.contains("Wheel")), "setup: the part \"Wheel\" must be in the tree");

        app.set_tree_search_for_test("arm");
        let got = shown(&mut app);
        assert!(got.iter().any(|s| s.contains("Arm Body")), "the search for \"arm\" must KEEP the part \"Arm Body\": {got:?}");
        assert!(!got.iter().any(|s| s.contains("Wheel")), "the search for \"arm\" must REMOVE the part \"Wheel\" — otherwise it does not filter components: {got:?}");
    }

    /// All the text of a shape in the frame.
    fn collect(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }

    /// THE QUERY RESETS WHEN THE CONTEXT CHANGES — in both directions.
    ///
    /// Reported behaviour: the filter does not reset on entering a part or a subassembly, nor on leaving
    /// one. The query was about the PREVIOUS context: inside a part it searches among features for what
    /// was typed about components and finds emptiness — and the tree looks broken.
    #[test]
    fn the_query_resets_when_the_context_changes() {
        let mut app = App::default();
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part_id = app.project.add_part("Arm Body");
        app.enter_ctx_for_test(root);

        app.set_tree_search_for_test("arm");
        app.enter_component_for_test(part_id);
        assert!(app.tree_search_for_test().is_empty(), "entering a part must reset the query — it was about the assembly");

        app.set_tree_search_for_test("extrusion");
        app.exit_context_for_test();
        assert!(app.tree_search_for_test().is_empty(), "leaving a part must reset the query — it was about its features");
    }
}
