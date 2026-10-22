//! AN ARC DRAWN BY ITS CENTRE ENDS ON ITSELF, through the window: the third click gives the direction the arc runs to,
//! and its end point lands on the circle of the start's radius in that direction, wherever the click was.
//!
//! Reported behaviour (#62): the arc was drawn with the radius of the start, and its end point stayed where the third
//! click was - inside or outside the arc, away from its end.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// An arc drawn by its centre (0, 0), its start (40, 0) and a third click at `end`: where its end point stands, and
    /// where it stands after the sketch is solved once more.
    fn the_end_of_an_arc_to(end: (f64, f64)) -> ((f64, f64), (f64, f64)) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Arc);
        assert!(hand.press_word(&crate::i18n::tr("opt-arc-cse"), egui::Pos2::ZERO), "the centre mode of the arc on the bar");
        hand.click2d(0.0, 0.0).click2d(40.0, 0.0).click2d(end.0, end.1).key(egui::Key::Escape);
        let at = |app: &App| {
            let s = &app.project.sketches[si];
            let b = s.entities.iter().find_map(|e| match e.kind.as_arc() {
                Some(qymcad_core::model::ArcParts { b, .. }) => Some(b),
                _ => None,
            });
            let b = b.expect("an arc was drawn");
            s.points.iter().find(|q| q.id == b).map(|q| (q.x, q.y)).expect("the end point")
        };
        let drawn = at(&app);
        app.project.solve_sketch(si);
        (drawn, at(&app))
    }

    #[test]
    fn the_end_of_an_arc_by_its_centre_lands_on_the_arc() {
        for click in [(0.0, 20.0), (0.0, 60.0)] {
            let (drawn, solved) = the_end_of_an_arc_to(click);
            assert!(drawn.0.abs() < 1e-6 && (drawn.1 - 40.0).abs() < 1e-6, "a third click at {click:?}: the end point stands at {drawn:?}, not at (0, 40)");
            assert!((solved.0 - drawn.0).hypot(solved.1 - drawn.1) < 1e-6, "a third click at {click:?}: solving again moved the end from {drawn:?} to {solved:?}");
        }
    }
}
