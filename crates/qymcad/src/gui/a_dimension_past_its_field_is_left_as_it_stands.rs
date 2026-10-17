//! A DIMENSION PAST ITS FIELD IS LEFT AS IT STANDS: the list of the constraints of a sketch open for editing shows each
//! dimension in a field held to 0.01 - 100 000 mm; a dimension standing past that - 0.005 mm - is shown and left alone,
//! frame after frame, and the document does not change under the window.
//!
//! Reported behaviour: a long chain of steps stopped at a save and an open - the program never came to rest, a rebuild
//! asked for again on every frame. The field put a value standing past its range back into the range on every frame and
//! said so as an edit; the constraint was written, the sketch solved, and the next frame did it again.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{Constraint, EntityKind};

    #[test]
    fn a_dimension_past_its_field_is_left_as_it_stands() {
        let (mut app, _ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        hand.sk_tool(1).click2d(0.0, 0.0).click2d(20.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
        let line = hand.app.project.sketches[si].entities.iter().find_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        });
        let (a, b) = line.expect("the line drawn");
        // a dimension of 0.005 mm on the line, the sketch solved to it: past the low end of the field
        let ci = hand.app.project.sketches[si].constraints.len();
        hand.app.project.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a, b, d: 0.005, off: 2.0, expr: String::new(), driven: false, axis: 0, at: None });
        hand.app.project.solve_sketch(si);
        hand.frame(Vec::new());
        let key = hand.app.project.rebuild_key();
        for _ in 0..5 {
            hand.frame(Vec::new());
        }
        let d = match &hand.app.project.sketches[si].constraints[ci] {
            Constraint::Distance { d, .. } => *d,
            _ => f64::NAN,
        };
        assert_eq!(d, 0.005, "the field of the list put the dimension of 0.005 mm into its range");
        assert_eq!(hand.app.project.rebuild_key(), key, "the document changed under the window with nothing done");
    }
}
