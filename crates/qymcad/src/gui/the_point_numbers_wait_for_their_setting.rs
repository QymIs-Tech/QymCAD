//! THE NUMBERS OF THE POINTS WAIT FOR THEIR SETTING: a sketch shows its shapes alone, with no number beside a point; the
//! box "Numbers of the points" in Settings -> Sketch, ticked by hand, brings the numbers, and unticked takes them away.
//!
//! Reported behaviour: every point of a sketch carried its number - for a person drawing they meant nothing and cluttered
//! the sheet; they were there for looking into a sketch.
#[cfg(test)]
pub(crate) mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;

    /// THE BOX OF THE NUMBERS OF THE POINTS TICKED OR UNTICKED BY HAND: Windows -> Settings, the Sketch section, a click
    /// on "Numbers of the points", the window closed the way it was opened. Answers whether every place
    /// was found.
    pub(crate) fn point_numbers_ticked(hand: &mut Hand) -> bool {
        let origin = egui::pos2(0.0, 0.0);
        let opened = hand.press_word(&crate::i18n::tr("menu-windows"), origin) && hand.press_word(&crate::i18n::tr("win-settings"), origin);
        let section = opened && hand.press_word(&crate::i18n::tr("settings-sec-sketch"), origin);
        // the words of a checkbox are part of it: a click on them ticks it, as a person clicks
        let ticked = section && hand.press_word(&crate::i18n::tr("settings-point-numbers"), origin);
        let closed = hand.press_word(&crate::i18n::tr("menu-windows"), origin) && hand.press_word(&crate::i18n::tr("win-settings"), origin);
        hand.frame(Vec::new());
        ticked && closed
    }

    /// The words of the last frame that are numbers alone - what the points are numbered with.
    fn numbers(hand: &Hand) -> usize {
        hand.words_drawn().iter().filter(|(t, _)| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).count()
    }

    #[test]
    fn the_numbers_of_the_points_come_with_their_box_and_go_with_it() {
        let (mut app, _ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(40.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(60.0, 0.0).click2d(90.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        hand.frame(Vec::new());
        assert!(!hand.app.set.show_point_numbers, "the numbers of the points are on by default");
        assert_eq!(numbers(&hand), 0, "a sketch shows numbers beside its points with the box unticked");
        assert!(point_numbers_ticked(&mut hand), "the box of the numbers of the points was not reached in Settings -> Sketch");
        assert!(hand.app.set.show_point_numbers, "the click on the box did not tick it");
        hand.frame(Vec::new());
        assert!(numbers(&hand) >= 4, "with the box ticked the points are not numbered");
        assert!(point_numbers_ticked(&mut hand), "the box was not reached the second time");
        hand.frame(Vec::new());
        assert_eq!(numbers(&hand), 0, "with the box unticked again the numbers stay");
    }
}
