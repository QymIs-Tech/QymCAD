//! "NO TIES" SHOWS NO TIE: with it ticked on the bar of the line tool, the live preview of the line drawn from its first
//! point carries no badge of a tie it is not going to lay - a line led level from its first point shows no "level"
//! badge. Without it the badge is there.
//!
//! Reported behaviour: "with No ties ticked the badges of the ties show from the first click while the pointer is led
//! to the second point".
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// The badges of ties the preview of a line shows, its first point at (0, 0), the pointer at (20, 0.2), "No ties"
    /// ticked as `free` says, construction as `construction` says.
    fn badges(free: bool, construction: bool) -> usize {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1);
        for (on, key) in [(construction, "opt-construction-short"), (free, "opt-no-ties-short")] {
            if on {
                assert!(hand.press_word(&crate::i18n::tr(key), egui::pos2(0.0, 0.0)), "no {key} on the bar of the line tool");
            }
        }
        hand.click2d(0.0, 0.0).hover2d(20.0, 0.2);
        // the sketch holds nothing else: every badge of a tie on the sheet is one the preview shows
        let colour = hand.app.scheme.pal.constraint_ok();
        hand.badges_in(colour).len()
    }

    #[test]
    fn no_ties_shows_no_badge_in_the_preview_of_a_line() {
        assert!(badges(false, false) >= 1, "GUARD: a line led level from its first point shows its level badge");
        assert!(badges(false, true) >= 1, "a construction line led level shows no badge, and it is tied level");
        let shown = [badges(true, false), badges(true, true)];
        assert!(shown == [0, 0], "\"No ties\" ticked: the preview of the line shows {} badge(s), of a construction line {}", shown[0], shown[1]);
    }
}
