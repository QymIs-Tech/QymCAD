//! THE DIAGNOSTICS OF A SKETCH WAIT FOR THE RELEASE, through the window: a point dragged with the select tool leaves
//! the degrees of freedom, free points, conflicts and redundant constraints as they were for the sketch's shape until
//! the button is let go; the release computes them once where the point came to rest.
//!
//! Reported behaviour (#95): a sketch of a few hundred lines hangs; every frame of a drag recomputed the rank analysis
//! of the whole sketch, 3.7 s on 3 000 lines.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    #[test]
    fn a_drag_leaves_the_diagnostics_to_the_release() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line);
        hand.click2d(10.0, 10.0).click2d(40.0, 25.0);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        let place = |app: &App| qymcad_ui_state::sketch_place_key(&app.project, si);
        let computed_at = |app: &App| app.cache.sk_status.borrow().as_ref().filter(|st| st.si == si).map(|st| st.place);
        hand.drag2d_begun((40.0, 25.0), (50.0, 35.0));
        let held = computed_at(hand.app);
        assert!(held.is_some(), "the window computed the diagnostics of the sketch");
        assert_ne!(held, Some(place(hand.app)), "a frame in the middle of the drag recomputed the diagnostics where the point stood");
        // the frame after the release draws the sketch with its diagnostics
        hand.release2d((50.0, 35.0)).frame(Vec::new());
        assert_eq!(computed_at(hand.app), Some(place(hand.app)), "the release recomputes the diagnostics where the point came to rest");
    }
}
