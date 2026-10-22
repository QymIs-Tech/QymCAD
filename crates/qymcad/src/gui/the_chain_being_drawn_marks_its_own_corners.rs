//! THE CHAIN BEING DRAWN MARKS ITS OWN CORNERS, through the window: while a chain of lines is drawn, the dots the
//! preview puts at its corners stand on the corners as they are - after an automatic constraint has moved them - and
//! not where the clicks were.
//!
//! Reported behaviour: drawing lines, a second dot stood beside every corner a few pixels off it, and went only with Esc;
//! a Perpendicular laid by auto-constraints had moved the corner, and the tool kept the place of the click.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    #[test]
    fn the_dots_of_a_chain_stand_on_its_corners() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line);
        // the second segment runs at 88.5 deg to the first: auto-constraints square it, and the solve moves a corner
        for (x, y) in [(-40.0, 40.0), (0.0, 0.0), (40.0, 38.0)] {
            hand.click2d(x, y);
        }
        hand.hover2d(20.0, 60.0);
        assert!(hand.app.project.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Perpendicular { .. })), "setup: the corner was squared by auto-constraints");
        let rings = hand.rings_drawn(3.0);
        let sheet = qymcad_ui_state::Sheet { view: hand.app.viewing.view, rect: hand.app.viewing.view_rect };
        let corners: Vec<egui::Pos2> = hand.app.project.sketches[si].points.iter().map(|q| sheet.at(qymcad_core::geom::Point2::new(q.x, q.y))).collect();
        let off: Vec<egui::Pos2> = rings.iter().copied().filter(|r| !corners.iter().any(|c| c.distance(*r) < 0.5)).collect();
        assert!(!rings.is_empty(), "setup: the chain draws its dots");
        assert!(off.is_empty(), "dots of the chain stand off its corners: {off:?}, the corners at {corners:?}");
    }
}
