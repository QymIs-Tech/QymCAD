//! A SIDE OF A RECTANGLE DELETED LEAVES THE OTHER LINES HELD, through the window: the top side picked and Delete
//! pressed, the three lines left are held Horizontal and Vertical, the two uprights Equal, with nothing over-defined -
//! as plain lines a person drew would be, and as a rectangle of lines keeps them in other systems.
//!
//! Reported behaviour (#72): the lines left were plain, held by nothing, though they still stood level and square.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    #[test]
    fn a_side_deleted_by_hand_leaves_the_other_lines_held() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        Hand::new(&mut app).sk_select().click2d(20.0, 30.0).key(egui::Key::Delete);
        let s = &app.project.sketches[si];
        assert!(s.rects().is_empty(), "setup: the top side deleted breaks the rectangle: {} lines left", s.entities.len());
        let count = |pred: fn(&Constraint) -> bool| s.constraints.iter().filter(|c| pred(c)).count();
        let (h, v, eq) = (count(|c| matches!(c, Constraint::Horizontal { .. })), count(|c| matches!(c, Constraint::Vertical { .. })), count(|c| matches!(c, Constraint::Equal { .. })));
        assert!(h == 1 && v == 2 && eq == 1, "the three lines left: {h} Horizontal, {v} Vertical, {eq} Equal: {:?}", s.constraints);
        assert_eq!(app.project.sketch_dof(si).1, 0, "the lines left are over-defined");
    }
}
