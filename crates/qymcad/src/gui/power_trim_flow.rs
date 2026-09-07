//! POWER TRIM: trimming BY DRAGGING.
//!
//! Trimming by click existed; dragging is needed where a lot has to be cut in a row: taking a grid of
//! construction lines apart one click at a time means dozens of aimed hits.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A sketch with a GRID: three verticals crossed by two horizontals. Returns the sketch index.
    fn grid_sketch(app: &mut App) -> usize {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        for x in [10.0, 20.0, 30.0] {
            app.project.add_line_entity(si, x, 0.0, x, 40.0, qymcad_core::feature::Purpose::Real);
        }
        for y in [10.0, 30.0] {
            app.project.add_line_entity(si, 0.0, y, 40.0, y, qymcad_core::feature::Purpose::Real);
        }
        app.project.regen_sketch(si);
        app.chosen.sel = Sel::Sketch(si);
        app.viewing.view.initialized = true;
        app.viewing.view.scale = 8.0;
        si
    }

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// It is the same tool: trimming gained a drag, and no separate button is added.
    #[test]
    fn the_drag_belongs_to_the_existing_trim_tool() {
        // the hint lives in the language catalogue rather than the source, so the TEXT is checked, not a literal in the code
        let prev = crate::i18n::language();
        for (code, _) in crate::i18n::available() {
            crate::i18n::set_language(&code);
            let hint = (crate::i18n::tr("tb-trim-hint") + &crate::i18n::tr("opt-trim-hint")).to_lowercase();
            // ASK ABOUT MEANING, NOT ABOUT SPELLING. This used to hold a match on the word in capital
            // letters, exactly as the catalogue wrote it. A tone pass removed the shouting mid-sentence
            // and the check failed on a perfectly good hint: it guarded not "is dragging mentioned" but
            // "is that word typed exactly so".
            //
            // The Russian stems below are lookup keys into the language catalogue and stay as they are:
            // translating them would match nothing.
            let says_drag = hint.contains("протя") || hint.contains("протащ") || hint.contains("drag");
            assert!(says_drag, "in language {code} the trim hint says nothing about dragging: {hint}");
        }
        crate::i18n::set_language(&prev);
        // THE NEEDLE CARRIES NO PATH: inside its own module the function is called by its bare name, from
        // another one by the full path, and which of the two stands here says nothing about the rule.
        assert!(
            crate::gui::render_source::dense(crate::gui::sketch_source::SKETCH).contains(&crate::gui::render_source::dense("power_trim_drag(&mut self.sketch_ctx(), resp, rect)")),
            "the drag must be invoked from the drag phase"
        );
    }

    /// A DRAG CUTS EVERYTHING IT PASSED THROUGH: one movement across three verticals.
    ///
    /// Checked through real geometry: how many entities there are after the drag. Trimming a span
    /// between intersections either shortens a line or splits it into two — either way the set of
    /// entities MUST change on each of the three, not on one.
    #[test]
    fn dragging_across_several_segments_trims_them_all() {
        let mut app = App::default();
        let si = grid_sketch(&mut app);
        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, 1);
        let before: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();

        // drag the cursor across the three verticals at a height between the horizontals (y = 20)
        let a = (qymcad_ui_state::Sheet { view: app.viewing.view, rect: rect() }).at(qymcad_core::geom::Point2::new(5.0, 20.0));
        let b = (qymcad_ui_state::Sheet { view: app.viewing.view, rect: rect() }).at(qymcad_core::geom::Point2::new(35.0, 20.0));
        let cut = crate::gui::sketching::power_trim_path_test(&mut app.sketch_ctx(), rect(), a, b);

        let after: Vec<(f64, f64)> = app.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
        assert_ne!(before, after, "the drag must trim something; status: {}", app.status);
        // THE MAIN POINT: SEVERAL were cut. One is an ordinary click, and the tool was not reworked for that.
        assert!(cut >= 3, "one movement across THREE verticals must cut three spans, and it cut {cut}");
        assert_eq!(app.status, crate::i18n::tr1("sk-trimmed-n", "n", &app.side.trim.done.len().to_string()), "the person must be told how many were cut");
    }

    /// ONE SPAN IS CUT ONCE: cursor jitter in place does not go on to cut neighbouring pieces.
    #[test]
    fn jitter_on_one_span_cuts_it_only_once() {
        let mut app = App::default();
        let si = grid_sketch(&mut app);
        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, 1);

        // jitter the cursor at one point on one vertical
        let p = (qymcad_ui_state::Sheet { view: app.viewing.view, rect: rect() }).at(qymcad_core::geom::Point2::new(20.0, 20.0));
        crate::gui::sketching::power_trim_path_test(&mut app.sketch_ctx(), rect(), p, p + egui::vec2(1.0, 0.0));
        let after_first = app.project.sketches[si].entities.len();
        crate::gui::sketching::power_trim_path_test_continue(&mut app.sketch_ctx(), rect(), p + egui::vec2(1.0, 0.0), p);
        assert_eq!(app.project.sketches[si].entities.len(), after_first, "a repeat pass over THE SAME span must not cut again");
    }
}
