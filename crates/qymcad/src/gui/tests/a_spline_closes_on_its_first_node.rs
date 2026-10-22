//! A SPLINE CLOSES ON ITS FIRST NODE, on the path of a person: the spline tool, four nodes clicked, and a click 6 px off
//! the first node - within the reach of a snap - closes it into a loop through its nodes, at the scale of a new sketch.
//!
//! Reported behaviour: the point on the mouse did not snap to the start of the spline, and it was hard to hit.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};

#[test]
fn a_click_near_the_first_node_closes_the_spline() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(10.0, 10.0).click2d(30.0, 14.0).click2d(34.0, 30.0).click2d(14.0, 32.0);
    // 6 px off the first node, at the scale of the sheet
    let off = 6.0 / f64::from(hand.app.viewing.view.scale);
    hand.hover2d(10.0 + off, 10.0).click2d(10.0 + off, 10.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let s = &hand.app.project.sketches[si];
    let ends: Vec<qymcad_core::feature::Ends> = s.entities.iter().filter_map(|e| e.kind.as_spline()).map(|sp| sp.ends).collect();
    assert!(
        ends == [qymcad_core::feature::Ends::Closed],
        "a click {off:.3} mm off the first node of a spline of four nodes left {ends:?}, not one closed spline; the status line says {:?}",
        hand.app.status
    );
}
