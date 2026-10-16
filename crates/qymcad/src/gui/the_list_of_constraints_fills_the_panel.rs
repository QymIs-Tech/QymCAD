//! THE LIST OF CONSTRAINTS AND DIMENSIONS FILLS THE PANEL IT STANDS IN.
//!
//! Reported behaviour: the list was capped at a fixed 360 points of height, and below it the rest of
//! the properties panel stood empty whatever the window's height - the rows scrolled inside their
//! strip while the panel had room to spare.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{Purpose, SketchPlane};

    /// A sketch of many rectangles: every side brings its own constraints, so the list holds far more
    /// rows than any panel shows at once.
    fn many_rows() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        for k in 0..40 {
            let x = 30.0 * f64::from(k);
            app.project.add_rect_entity(si, x, 0.0, x + 20.0, 12.0, Purpose::Real);
        }
        app.project.regen_sketch(si);
        (app, si)
    }

    /// THE LIST ENDS NEAR THE BOTTOM OF THE PANEL. The whole frame the hand draws runs in a window
    /// 900 points tall with the properties panel on its right; a row the list's viewport does not
    /// hold is not painted at all, so the deepest painted row of the list is where the list ends on
    /// screen. The deepest word of the frame belongs to the bottom bar that closes the panel, so its
    /// top edge is where the panel itself ends: the deepest row must reach within 40 points of it.
    /// With the fixed cap of 360 points the list ended 79 points above the bar in this frame and the
    /// panel stood empty below; the list that follows the panel reaches past the bar's top edge, its
    /// last row half-cut.
    #[test]
    fn the_list_ends_near_the_bottom_of_the_panel() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        // the row marker is asked of the catalogue, as a person reads it: a row names its entities
        let line_word = crate::i18n::tr("ent-line");
        let (mut app, si) = many_rows();
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        for _ in 0..3 {
            hand.frame(Vec::new()); // the panels settle and the list learns the height it is given
        }
        let words = hand.words_drawn();
        // the rows of the constraint list alone: a canvas label names an entity, only a row of the
        // list says which constraint holds it - "Midpoint: Line 85, Line 86"
        let bottom = words.iter().filter(|(t, _)| t.contains(&line_word) && t.contains(": ")).map(|(_, r)| r.bottom()).fold(f32::NEG_INFINITY, f32::max);
        // the top of the frame's deepest word is the panel's own bottom
        let panel_bottom = words.iter().max_by(|a, b| a.1.bottom().total_cmp(&b.1.bottom())).expect("the frame holds words").1.top();
        crate::i18n::set_language(&prev);
        assert!(bottom > 0.0, "the frame holds no rows of the constraint list - the check has nothing to look at");
        assert!(bottom > panel_bottom - 40.0, "the list ends {:.0} points above the bottom of the panel - it does not fill the panel it stands in", panel_bottom - bottom);
    }
}
