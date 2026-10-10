//! THE OFFSET IS AIMED AS A FILLET IS: the curves chosen before the tool are taken, the copy follows the pointer to one
//! side until a click there fixes it - its preview turns the colour of a fixed corner and stays when the pointer goes -
//! and the box of its distance opens beside the copy with the keyboard in it; Enter makes the copy, its corners sharp
//! where the source has sharp corners. The tool stays in hand: the next curves are clicked and offset the same way
//! without pressing it again. Esc in the box frees the side; Esc again puts the tool down. A box dragged over the curves
//! with the tool in hand chooses them for it and leaves the tool in hand.
//!
//! Reported behaviour: "the polyline has no fillets, and the copy is rounded"; "I take the tool, choose something, and
//! nothing happens - the choice works every other time"; "a popup with a field, the same choice as the fillet: a click
//! fixes the side, then the mouse can go to the popup"; "the tool is put down when the geometry is chosen all at once
//! with the left button held".
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::EntityKind;
    use qymcad_ui_state::EditTool;

    /// A zigzag of three lines: up, down, up.
    const ZIGZAG: [(f64, f64); 4] = [(-30.0, 0.0), (-10.0, 15.0), (10.0, -5.0), (30.0, 10.0)];

    fn stands(app: &App, x: f64, y: f64) -> bool {
        app.project.sketches[0].points.iter().any(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6)
    }

    fn arcs(app: &App) -> usize {
        app.project.sketches[0].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count()
    }

    #[test]
    fn a_click_fixes_the_side_the_box_takes_the_distance_and_the_tool_stays() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        let [a, b, c, d] = ZIGZAG;
        hand.sk_tool(1).click2d(a.0, a.1).click2d(b.0, b.1).click2d(c.0, c.1).double_click2d(d.0, d.1);
        hand.key(egui::Key::Escape).sk_tool(0);
        let (following, fixed) = (hand.app.scheme.pal.preview_corner_new(), hand.app.scheme.pal.preview_corner_fixed());
        let mut failures = Vec::new();

        // chosen before the tool: the three lines, then Offset
        hand.click2d(-20.0, 7.5).shift_click2d(0.0, 5.0).shift_click2d(20.0, 2.5);
        assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
        // by the start of the chain, to its right, the copy follows the pointer there (the hand reads strokes by their
        // points, and a copy of lines has its points at its corners and ends); a click on nothing fixes it
        let right = (-27.0, -3.0);
        if hand.strokes_near2d(right, following) == 0 {
            failures.push("the pointer by the start of the chain, to its right: no preview of the copy follows it".to_string());
        }
        hand.click2d(right.0, right.1);
        if hand.app.tools.sel_sk.offset.at.is_none() || !hand.typing() {
            failures.push(format!("a click beside the chain did not fix the side and open the box with the keyboard in it; status: {}", hand.app.status));
        }
        // the pointer goes over the peak, to the left of the chain: no copy there, the one fixed to the right stays
        let left = (-10.0, 19.0);
        if hand.strokes_near2d(left, fixed) != 0 || hand.strokes_near2d(left, following) != 0 || hand.strokes_near2d(right, fixed) == 0 {
            failures.push("the side fixed to the right of the chain and the pointer led to its left: the copy went with the pointer".to_string());
        }
        hand.type_text("4").key(egui::Key::Enter);
        // the right of the way of the first and the last line is down: their copies start 4 off, square to them
        if !stands(hand.app, -27.6, -3.2) || !stands(hand.app, 32.4, 6.8) || arcs(hand.app) != 0 {
            failures.push(format!(
                "the zigzag offset 4 to its right: no copy end at (-27.6, -3.2) or (32.4, 6.8), or {} arcs where its corners are sharp; status: {}",
                arcs(hand.app),
                hand.app.status
            ));
        }
        if hand.app.tools.sel_sk.modify != Some(EditTool::Offset) || hand.app.tools.armed.modify() != Some(EditTool::Offset) || !hand.app.tools.sel_sk.items.is_empty() {
            failures.push(format!("after the copy the tool is not in hand with nothing chosen: modify {:?}, armed {:?}", hand.app.tools.sel_sk.modify, hand.app.tools.armed.modify()));
        }

        // the tool still in hand: the lines clicked one by one, the side above fixed, 2 typed, Enter - a second copy
        hand.click2d(-20.0, 7.5).click2d(0.0, 5.0).click2d(20.0, 2.5);
        hand.click2d(-20.0, 13.0).type_text("2").key(egui::Key::Enter);
        if !stands(hand.app, -31.2, 1.6) || !stands(hand.app, 28.8, 11.6) {
            failures.push(format!("the tool kept in hand, the zigzag offset 2 to its left: no copy end at (-31.2, 1.6) or (28.8, 11.6); status: {}", hand.app.status));
        }

        // Esc in the box frees the side and keeps the tool; Esc again puts it down
        hand.click2d(-20.0, 7.5).click2d(-20.0, -1.0);
        hand.key(egui::Key::Escape);
        if hand.app.tools.sel_sk.offset.at.is_some() || hand.app.tools.sel_sk.modify != Some(EditTool::Offset) {
            failures.push(format!("Esc in the box: the side is still fixed ({:?}) or the tool went ({:?})", hand.app.tools.sel_sk.offset.at, hand.app.tools.sel_sk.modify));
        }
        hand.key(egui::Key::Escape);
        if hand.app.tools.sel_sk.modify.is_some() || hand.app.tools.armed.modify().is_some() {
            failures.push("a second Esc did not put the tool down".to_string());
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn a_box_dragged_with_the_tool_in_hand_chooses_for_it_and_keeps_it() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        let [a, b, c, d] = ZIGZAG;
        hand.sk_tool(1).click2d(a.0, a.1).click2d(b.0, b.1).click2d(c.0, c.1).double_click2d(d.0, d.1);
        hand.key(egui::Key::Escape).sk_tool(0);
        assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
        // the box from above the left end down past the right end, over the whole zigzag, on nothing but paper
        hand.drag2d((-38.0, 22.0), (38.0, -12.0));
        let curves = hand.app.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).count();
        let mut failures = Vec::new();
        if hand.app.tools.sel_sk.modify != Some(EditTool::Offset) || hand.app.tools.armed.modify() != Some(EditTool::Offset) || curves != 3 {
            failures.push(format!(
                "a box over the zigzag with Offset in hand: modify {:?}, armed {:?}, {curves} curves chosen; status: {}",
                hand.app.tools.sel_sk.modify,
                hand.app.tools.armed.modify(),
                hand.app.status
            ));
        }
        if hand.app.status != crate::i18n::tr("sk-offset-side") {
            failures.push(format!("after the box the status does not say what to do next: {}", hand.app.status));
        }
        // the pointer by the start, to the right, Enter: the copy by the distance of the bar, 3, starts square to it there
        hand.hover2d(-27.0, -3.0).key(egui::Key::Enter);
        if !stands(hand.app, -28.2, -2.4) {
            failures.push(format!("Enter after the box made no copy to the right of the zigzag; status: {}", hand.app.status));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
