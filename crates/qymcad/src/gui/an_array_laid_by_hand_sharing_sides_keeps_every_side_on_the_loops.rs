//! AN ARRAY LAID BY HAND SHARING SIDES KEEPS EVERY SIDE ON THE LOOPS, through the window: a square
//! drawn in the sketcher, taken by a click on its side, arrayed in copies lying against it (the step
//! equal to its side), Enter - and every side of every square is named on some closed loop of the
//! sketch, with a region under each square to take a profile by.
//!
//! Reported behaviour: after such an array the shared sides stood on no loop - lit only while picked,
//! and no loop to take a boss by.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// The drawn curves of the sketch no closed loop names, read the way the rebuild round a change
    /// reads a loop: by the entities of the loop and by the sources of its edges.
    fn orphan_curves(app: &App, si: usize) -> Vec<u64> {
        let s = &app.project.sketches[si];
        s.entities
            .iter()
            .filter(|e| !e.construction)
            .filter(|e| {
                !s.contour_ids.iter().any(|&cid| {
                    app.project.contour_index(cid).is_some_and(|ci| app.project.contours[ci].edge_src.contains(&e.id)) || app.project.contours.ents_of(cid).is_some_and(|ents| ents.contains(&e.id))
                })
            })
            .map(|e| e.id)
            .collect()
    }

    #[test]
    fn a_square_arrayed_in_touching_copies_keeps_its_loops_by_hand() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        // a square 20 x 20, taken by a click on its side
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(20.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
        assert!(orphan_curves(hand.app, si).is_empty(), "setup: the square before the array has curves on no loop");
        hand.sk_select().click2d(10.0, 0.0);
        assert!(hand.press_hint(&crate::i18n::tr("tb-lin-array-hint")), "the linear pattern");
        assert!(hand.type_after_word(&crate::i18n::tr("cmd-copies"), "3"), "the count");
        assert!(hand.type_after_word(&crate::i18n::tr("opt-step-x"), "20"), "the step x");
        assert!(hand.type_after_word(&crate::i18n::tr("opt-step-y"), "0"), "the step y");
        hand.key(egui::Key::Enter);

        let orphans = orphan_curves(hand.app, si);
        assert!(orphans.is_empty(), "after the array curves {orphans:?} are on no loop");
        // a region lies under the centre of each square, for a boss to take
        hand.look2d((30.0, 10.0)).frame(Vec::new());
        let rect = hand.app.viewing.view_rect;
        let sheet = qymcad_ui_state::Sheet { view: hand.app.viewing.view, rect };
        for x in [10.0, 30.0, 50.0] {
            let at = sheet.at(qymcad_core::geom::Point2::new(x, 10.0));
            let got = qymcad_ui_state::contour_under_2d(&hand.app.project, &hand.app.viewing.view, rect, at, si);
            assert!(got.is_some(), "no region under the centre of the square at x={x}");
        }
    }
}
