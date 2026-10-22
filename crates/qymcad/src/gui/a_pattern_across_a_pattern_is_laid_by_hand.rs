//! A PATTERN ACROSS A PATTERN IS LAID BY HAND, through the window: a line and another across it, each patterned by the
//! linear pattern tool - the count and the steps typed in its bar, Enter - make a grid of cells, and the frames over it
//! stay quick.
//!
//! Reported behaviour (#95): one line horizontal and one vertical, each patterned 200 times 20 mm apart, and every
//! operation of the sketch waited 30 s or more. Each copy of a pattern rebuilt the loops of the whole growing grid.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::a_big_drawing_stays_live::tests::{apart, numbers_apart, points_drawn, points_in_sight};
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use std::time::Duration;

    #[test]
    fn a_grid_of_two_patterns_is_laid_and_worked() {
        // A RELEASE BUILD TAKES THE GRID OF THE REPORT, 200 by 200; a test build 40 by 40
        let release = !cfg!(debug_assertions);
        let n: u32 = if release { 200 } else { 40 };
        let len = 20.0 * n as f64;
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, -10.0).click2d(len, -10.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(-10.0, 0.0).click2d(-10.0, len).key(egui::Key::Escape).key(egui::Key::Escape);
        let mut laid = Vec::new();
        for (middle, step) in [((len / 2.0, -10.0), ["0", "20"]), ((-10.0, len / 2.0), ["20", "0"])] {
            hand.sk_select();
            hand.click2d(middle.0, middle.1);
            assert!(hand.press_hint(&crate::i18n::tr("tb-lin-array-hint")), "the linear pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-count"), &n.to_string()), "the count of the pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-x"), step[0]), "the step X");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-y"), step[1]), "the step Y");
            // the work of the pattern, not the share of the machine it was given (`time_budget::Cost`): the solve of the
            // sketch runs on threads of its own
            laid.push(
                qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Process, || {
                    hand.key(egui::Key::Enter);
                })
                .cost,
            );
            hand.key(egui::Key::Escape);
        }
        let s = &hand.app.project.sketches[si];
        assert_eq!(s.entities.len(), 2 * n as usize, "the two patterns laid their copies");
        let cells = ((n - 1) * (n - 1)) as usize;
        assert!(s.contour_ids.len() >= cells, "the grid has {} loops, fewer than its {cells} cells", s.contour_ids.len());
        // the numbers of the points are behind their box in Settings -> Sketch: ticked here, to see they keep apart
        assert!(crate::gui::the_point_numbers_wait_for_their_setting::tests::point_numbers_ticked(&mut hand), "the box of the numbers of the points was not reached");
        // TAKEN AWAY until the whole grid is in sight and its ends, 20 mm apart, stand closer than a point is wide: they are
        // not drawn as bars - no two points nearer than a point is wide, no number over another
        let total = hand.app.project.sketches[si].points.iter().filter(|q| !hand.app.project.sketches[si].unseen_points().contains(&q.id)).count();
        hand.look2d((len / 2.0, len / 2.0)).frame(Vec::new()); // the wheel turns over the middle of the grid, in the view
        for _ in 0..40 {
            if points_in_sight(&hand, si) == total && 20.0 * hand.app.viewing.view.scale < qymcad_render::POINT_ROOM {
                break;
            }
            hand.wheel2d((len / 2.0, len / 2.0), false);
        }
        assert_eq!(points_in_sight(&hand, si), total, "the wheel did not take the whole grid into sight");
        assert!(20.0 * hand.app.viewing.view.scale < qymcad_render::POINT_ROOM, "the wheel did not take the ends of the grid nearer than a point is wide");
        let drawn = points_drawn(&hand);
        assert!(apart(&drawn), "two ends of the grid are drawn nearer than a point is wide");
        assert!(drawn.len() < total, "all {total} ends of the grid are drawn at a whole view, one over another");
        assert!(numbers_apart(&hand), "two numbers of the ends are written one over another");
        // brought near a crossing, the ends there apart again: every one in sight is drawn
        hand.look2d((0.0, 0.0)).frame(Vec::new());
        assert_eq!(points_drawn(&hand).len(), points_in_sight(&hand, si), "near the corner of the grid not every end in sight is drawn");
        let hover = qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Thread, || {
            hand.hover2d(15.0, 15.0);
        })
        .cost;
        eprintln!("a grid of {n} by {n}: the patterns laid in {laid:?}, two frames with the pointer over it {hover:?}");
        // measured in a release build on 200 by 200: the second pattern 29 s rebuilt after each copy, 0.3 s rebuilt once
        let budget = Duration::from_secs(2);
        let over: Vec<String> = laid.iter().filter(|c| c.work >= budget).map(|c| qymcad_core::time_budget::over("a pattern of the grid", *c, budget)).collect();
        assert!(over.is_empty(), "{}", over.join("; "));
    }
}
