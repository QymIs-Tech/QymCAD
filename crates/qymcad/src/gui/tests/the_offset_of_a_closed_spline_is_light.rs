//! THE OFFSET OF A CLOSED SPLINE IS LIGHT, on the path of the report: a closed spline of seven nodes drawn by hand, closed
//! by a click on its first node, clicked, Offset taken, a click inside it and 2 typed - the copy is of tens of nodes in
//! all, not hundreds.
//!
//! Reported behaviour: the offset of a closed spline came out a spline of hundreds of points, 1038 degrees of freedom,
//! and the program hung.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};

#[test]
fn the_offset_of_a_closed_spline_drawn_by_hand_is_a_spline_of_few_nodes() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    let blob = [(-22.0, 6.0), (-14.0, 14.0), (-3.0, 7.0), (7.0, 25.0), (21.0, 9.0), (10.0, -9.0), (-17.0, -14.0)];
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline);
    for (x, y) in blob {
        hand.click2d(x, y);
    }
    hand.click2d(blob[0].0, blob[0].1);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let closed = hand.app.project.sketches[si].entities.iter().filter_map(|e| e.kind.as_spline()).any(|s| s.ends == qymcad_core::feature::Ends::Closed);
    assert!(closed, "setup: the spline was not closed by a click on its first node");
    // the spline clicked on its way between two nodes, not on a node
    let on_it = hand.app.project.sketches[si]
        .entities
        .iter()
        .find(|e| e.kind.as_spline().is_some())
        .and_then(|e| qymcad_core::model::curve::Curve::of(&hand.app.project.sketches[si], e))
        // half way along its first span, by the knots of its curve
        .map(|c| c.at(0.5 * c.b_spline().and_then(|b| b.inner_knots().first().copied()).unwrap_or(0.14)))
        .expect("the spline");
    hand.sk_select().click2d(on_it.x, on_it.y);
    assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
    hand.click2d(0.0, 0.0);
    hand.type_text("2").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let s = &hand.app.project.sketches[si];
    let splines: Vec<usize> = s.entities.iter().filter_map(|e| e.kind.as_spline().map(|sp| sp.nodes.len())).collect();
    let status = hand.app.status.clone();
    // inwards past the bend of the notch the copy is in pieces, its loop cut away: tens of nodes in all
    let copy: usize = splines.iter().skip(1).sum();
    assert!(splines.len() >= 2 && copy <= 8 * blob.len(), "the copy: splines of {splines:?} nodes, not the blob and a copy of tens; the status line says {status:?}");
}
