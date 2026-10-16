//! EXTEND IS WORKED BY HAND AS EVERY TOOL: a line selected before it is the line it holds, or the first click picks one;
//! the extension is drawn dashed up to what the pointer is over, or the nearest on that side; Enter or a click makes
//! it, Esc lets the line go. "Both sides" in the bar extends both ends, each to its own; a side meeting nothing stays.
//!
//! Reported behaviour: Extend took no selection, showed no preview of where the line goes, stretched the end nearer the
//! click to the nearest crossing at once, and could not extend both ends.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;

    /// The line (0, 0) - (10, 0) drawn by hand and vertical lines across its axis at each of `across`, from y -5 to 5.
    /// Answers the application and the id of the line.
    fn lines(across: &[f64]) -> (App, egui::Context, u64) {
        let (mut app, ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        app.enter_sketch_edit(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1).click2d(0.0, 0.0).click2d(10.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
        for &x in across {
            hand.sk_tool(1).click2d(x, -5.0).click2d(x, 5.0).key(egui::Key::Escape).key(egui::Key::Escape);
        }
        hand.sk_tool(0);
        let line = hand.app.project.sketches[0].entities[0].id;
        (app, ctx, line)
    }

    fn take_extend(hand: &mut Hand) {
        assert!(hand.press_hint(&crate::i18n::tr("tb-extend-hint")), "no Extend button");
    }

    /// Where the ends of the line stand: x of its start and of its end.
    fn ends_x(hand: &Hand, line: u64) -> (f64, f64) {
        let sk = &hand.app.project.sketches[0];
        let e = sk.entities.iter().find(|e| e.id == line).expect("the line");
        let qymcad_core::model::EntityKind::Line { a, b } = e.kind else { panic!("not a line") };
        let x = |id| sk.points.iter().find(|q| q.id == id).map(|q| q.x).expect("the point");
        (x(a), x(b))
    }

    /// THE DASHED EXTENSION DRAWN IN THE LAST FRAME runs on the axis of the line up to `to` - its last dash within a
    /// dash and a gap of it (10 px), none past it - with the ring where the end lands on `to`.
    fn preview_reaches(hand: &Hand, to: f64) -> bool {
        let green = hand.app.scheme.pal.add();
        let (target, from) = (hand.on_screen2d((to, 0.0)), hand.on_screen2d((0.0, 0.0)));
        let past = |p: &egui::Pos2| (p.x - target.x) * (target.x - from.x).signum() > 1.0;
        let dashes = hand.segments_in(green);
        let reaches = dashes.iter().flatten().any(|p| p.distance(target) <= 10.0);
        let ring = hand.rings_drawn(qymcad_render::EXTEND_RING).iter().any(|p| p.distance(target) < 1.5);
        reaches && ring && !dashes.iter().flatten().any(past)
    }

    #[test]
    fn a_line_selected_first_is_extended_to_the_nearest_with_its_preview() {
        let (mut app, _ctx, line) = lines(&[20.0, 30.0]);
        let mut hand = Hand::new(&mut app);
        assert!(hand.select2d(&[(1, line)]), "the line could not be picked");
        take_extend(&mut hand);
        assert_eq!(hand.app.tools.tool.extend, Some(line), "Extend did not take the line selected before it");
        hand.hover2d(12.0, 1.0);
        assert!(preview_reaches(&hand, 20.0), "no dashed extension up to the nearest line at x 20");
        hand.key(egui::Key::Enter);
        assert_eq!(ends_x(&hand, line), (0.0, 20.0), "Enter extended the line elsewhere");
    }

    #[test]
    fn a_line_picked_after_the_tool_is_extended_to_the_line_under_the_pointer() {
        let (mut app, _ctx, line) = lines(&[20.0, 30.0]);
        let mut hand = Hand::new(&mut app);
        take_extend(&mut hand);
        assert!(hand.app.tools.tool.extend.is_none(), "GUARD: nothing held before a pick");
        hand.click2d(5.0, 0.0);
        assert_eq!(hand.app.tools.tool.extend, Some(line), "the click did not take the line");
        assert_eq!(ends_x(&hand, line), (0.0, 10.0), "taking the line extended it already");
        hand.hover2d(30.0, 3.0);
        assert!(preview_reaches(&hand, 30.0), "the pointer over the farther line - no dashed extension up to it");
        hand.click2d(30.0, 3.0);
        assert_eq!(ends_x(&hand, line), (0.0, 30.0), "the click extended the line elsewhere");
    }

    #[test]
    fn both_sides_extend_each_end_and_a_side_meeting_nothing_stays() {
        let mut failures = Vec::new();
        for (across, want) in [(&[-15.0, 20.0][..], (-15.0, 20.0)), (&[20.0][..], (0.0, 20.0))] {
            let (mut app, _ctx, line) = lines(across);
            let mut hand = Hand::new(&mut app);
            assert!(hand.select2d(&[(1, line)]), "the line could not be picked");
            take_extend(&mut hand);
            assert!(hand.press_word(&crate::i18n::tr("opt-extend-both"), egui::pos2(0.0, 0.0)), "no Both sides in the bar");
            assert!(hand.app.tool_prefs.extend_both, "GUARD: Both sides ticked");
            hand.hover2d(5.0, 1.0);
            hand.key(egui::Key::Enter);
            if ends_x(&hand, line) != want {
                failures.push(format!("lines across at {across:?}: the line runs {:?}, wanted {want:?}; the constraints {:?}", ends_x(&hand, line), hand.app.project.sketches[0].constraints));
            }
        }
        assert!(failures.is_empty(), "both sides:\n{}", failures.join("\n"));
    }

    #[test]
    fn esc_lets_the_line_go_and_a_second_puts_the_tool_down() {
        let (mut app, _ctx, line) = lines(&[20.0]);
        let mut hand = Hand::new(&mut app);
        assert!(hand.select2d(&[(1, line)]), "the line could not be picked");
        take_extend(&mut hand);
        hand.key(egui::Key::Escape);
        assert!(hand.app.tools.tool.extend.is_none() && hand.app.tools.armed.click_op() == 2, "Esc did not let the line go, keeping the tool");
        hand.key(egui::Key::Escape);
        assert_eq!(hand.app.tools.armed.click_op(), 0, "a second Esc did not put Extend down");
        assert_eq!(ends_x(&hand, line), (0.0, 10.0), "Esc changed the line");
    }
}
