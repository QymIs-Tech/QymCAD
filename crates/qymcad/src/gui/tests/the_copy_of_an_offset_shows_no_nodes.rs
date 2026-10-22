//! THE COPY OF AN OFFSET SHOWS NO NODES, on the path of a person: a closed spline of seven nodes drawn by hand, offset 2
//! inwards by the offset tool - the frame draws the points of the spline, and none at the nodes of its copy, which the
//! offset lays again from the spline and which are not dragged.
//!
//! Reported behaviour: the copy of a spline of 82 nodes covered the drawing with the 688 points of its nodes.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::model::curve::Curve;

#[test]
fn the_nodes_of_the_copy_of_an_offset_are_not_drawn() {
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
    let s = &hand.app.project.sketches[si];
    let spline = s.entities.iter().find(|e| e.kind.as_spline().is_some()).map(|e| e.id).expect("setup: the spline");
    // half way along its first span, by the knots of its curve
    let on_it = Curve::of(s, s.entity(spline).expect("the spline")).map(|c| c.at(0.5 * c.b_spline().and_then(|b| b.inner_knots().first().copied()).unwrap_or(0.14))).expect("its curve");
    hand.sk_select().click2d(on_it.x, on_it.y);
    assert!(hand.press_hint(&crate::i18n::tr("tb-offset-hint")), "no sketch offset button");
    hand.click2d(0.0, 0.0);
    hand.type_text("2").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let s = &hand.app.project.sketches[si];
    let held = s.held_copy_nodes();
    assert!(!held.is_empty(), "setup: no copy was laid; the status line says {:?}", hand.app.status);
    let at = |id| s.point(id).map(|q| (q.x, q.y));
    let copy_nodes: Vec<(f64, f64)> = held.iter().filter_map(|id| at(*id)).collect();
    let source_nodes: Vec<(f64, f64)> = s.entity(spline).and_then(|e| e.kind.as_spline()).map(|sp| sp.nodes.iter().filter_map(|id| at(*id)).collect()).unwrap_or_default();
    hand.frame(Vec::new());
    let dots = hand.dots_drawn(3.5);
    let drawn_at = |p: (f64, f64)| {
        let q = hand.on_screen2d(p);
        dots.iter().any(|d| d.distance(q) < 1.0)
    };
    let shown_copy = copy_nodes.iter().filter(|p| drawn_at(**p)).count();
    let shown_source = source_nodes.iter().filter(|p| drawn_at(**p)).count();
    assert!(
        shown_copy == 0 && shown_source == source_nodes.len(),
        "{shown_copy} of the {} nodes of the copy are drawn as points, {shown_source} of the {} of the spline",
        copy_nodes.len(),
        source_nodes.len()
    );
}
