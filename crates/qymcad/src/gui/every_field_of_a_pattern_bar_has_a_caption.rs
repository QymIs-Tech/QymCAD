//! EVERY FIELD OF THE BAR OF A SKETCH PATTERN HAS A CAPTION, through the window: the linear pattern taken by its button
//! shows a caption before the count, the step X and Y, the rows and - with more than one row - the two steps of the rows;
//! the circular one before the count and the angle.
//!
//! Reported behaviour (#55): the bar showed `Count [3]`, then `[20]` and `[0]` with no caption and no unit, then
//! `rows [1]`; the circular pattern `Count [3]` and `[360]`.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// A line drawn and selected, and the pattern of the button `hint` taken.
    fn a_pattern_in_hand<'a>(app: &'a mut App, hint: &str) -> Hand<'a> {
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(20.0, 0.0).key(egui::Key::Escape);
        let mut hand = Hand::new(app);
        hand.sk_select();
        hand.click2d(10.0, 0.0);
        assert!(hand.press_hint(&crate::i18n::tr(hint)), "the button of the pattern {hint}");
        hand
    }

    #[test]
    fn every_field_of_the_linear_pattern_has_a_caption() {
        let mut app = App::default();
        let mut hand = a_pattern_in_hand(&mut app, "tb-lin-array-hint");
        let bare = hand.bar_fields_without_caption();
        assert!(bare.is_empty(), "fields of the linear pattern with no caption: {bare:?}");
        let second = crate::i18n::tr1("box-direction-n", "n", "2");
        assert!(hand.press_word(&second, egui::pos2(0.0, 0.0)), "the second direction, opened by its title");
        let bare = hand.bar_fields_without_caption();
        assert!(bare.is_empty(), "fields of the linear pattern of two directions with no caption: {bare:?}");
        let steps = hand.words_drawn().iter().filter(|(t, _)| *t == crate::i18n::tr("opt-step-x")).count();
        assert_eq!(steps, 2, "the second direction opened shows no step of its own");
    }

    #[test]
    fn every_field_of_the_circular_pattern_has_a_caption() {
        let mut app = App::default();
        let mut hand = a_pattern_in_hand(&mut app, "tb-circ-array-hint");
        let bare = hand.bar_fields_without_caption();
        assert!(bare.is_empty(), "fields of the circular pattern with no caption: {bare:?}");
    }
}
