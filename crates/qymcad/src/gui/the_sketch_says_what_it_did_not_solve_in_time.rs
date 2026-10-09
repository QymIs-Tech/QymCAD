//! A SKETCH SAYS WHAT IT DID NOT SOLVE IN TIME, through the window: the panel of a sketch whose last solve ran out of
//! time names the groups of shapes left, and an edit by hand - a point dragged - solves them and the line goes.
//!
//! The state is laid through the project's own door (`Project::solve_sketch_within` with no time): a sketch big enough
//! to run a solve out of its 10 s is not a check that runs in seconds.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::solver::Budget;

    #[test]
    fn the_panel_names_what_a_solve_left_and_an_edit_takes_it_up() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1);
        // a line laid nearly level takes a Horizontal of its own: a part with a constraint
        hand.click2d(10.0, 10.0).click2d(40.0, 10.5);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_tool(0);
        hand.app.project.solve_sketch_within(si, Budget { time: Some(std::time::Duration::ZERO), ..Budget::FULL });
        let left = hand.app.project.sketches[si].left_unsolved;
        assert!(left > 0, "a solve with no time left nothing to say");
        let said = crate::i18n::tr1("sk-unsolved-left", "n", &left.to_string());
        assert!(hand.shows(&said), "the panel of the sketch does not say {said:?}");
        let s = &hand.app.project.sketches[si];
        let b = s.entities.iter().find_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Line { b, .. } => Some(b),
            _ => None,
        });
        let end = s.points.iter().find(|q| Some(q.id) == b).map(|q| (q.x, q.y)).expect("the line has ends");
        hand.drag2d(end, (end.0 + 5.0, end.1));
        assert_eq!(hand.app.project.sketches[si].left_unsolved, 0, "the edit did not solve what was left");
        assert!(!hand.shows(&said), "the panel still says {said:?} after the edit");
    }
}
