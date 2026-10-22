//! THE BOX OF "FILLET ALL" STANDS AT THE SIDE CLICKED, the same every time: the lines of the shape go into the set with
//! the one clicked first, so the first corner of the set, which the box stands beside, is a corner of that line.
//!
//! Reported behaviour: the box stood beside whichever corner the set happened to start with - the lines of the shape
//! came out of a hash set - so on the same rectangle and the same click it stood at the bottom one time and at the top
//! of the sheet under the bar the next.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A rectangle of 40 x 30 drawn by the rectangle tool, "fillet all" taken and its bottom side clicked: where the box
    /// of the value stands, and where the middle of the rectangle is, on screen.
    fn the_box_after_a_click_on_the_bottom() -> (egui::Rect, egui::Pos2) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Enter);
        let mut hand = Hand::new(&mut app);
        hand.sk_select();
        hand.hover2d(20.0, 15.0);
        assert!(hand.press_hint(&crate::i18n::tr("tb-fillet-all-hint")), "the button of fillet all");
        hand.click2d(20.0, 0.0);
        hand.frame(Vec::new());
        let middle = hand.on_screen2d((20.0, 15.0));
        (hand.corner_box().expect("the box of the value is open"), middle)
    }

    #[test]
    fn the_box_of_fillet_all_stands_at_the_clicked_side_every_time() {
        let runs: Vec<(egui::Rect, egui::Pos2)> = (0..6).map(|_| the_box_after_a_click_on_the_bottom()).collect();
        let (first, middle) = runs[0];
        for (i, (place, _)) in runs.iter().enumerate() {
            assert!(place.min.distance(first.min) < 1.0, "the box stood elsewhere on run {i}: {place:?} against {first:?}");
        }
        // the bottom side of the drawing is below its middle on screen
        assert!(first.center().y > middle.y, "the box does not stand at the bottom side clicked: {first:?}, the middle of the rectangle at {middle:?}");
    }
}
