//! A FILLETED SPLINE DRAGS LIGHTLY, on the path of a person: a circle and a spline through it drawn by hand, the corner
//! at their crossing rounded by the fillet tool, and a middle node of the spline led 6 mm away in steps of 3 px - the
//! drag keeps up with the pointer.
//!
//! Reported behaviour: a spline joined to other geometry or rounded dragged by a node with a large delay, behind the
//! pointer. Measured on this scene: 6.3 s the whole drag, 516 ms its longest frame, where the nearest place on the
//! spline laid it out afresh at every look of the rows of touching; 0.15 s and 55 ms since.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;

#[test]
fn a_node_of_a_spline_rounded_to_a_circle_drags_lightly() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(20.0, 20.0).click2d(32.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(2.0, 40.0).click2d(10.0, 30.0).click2d(18.0, 22.0).click2d(28.0, 18.0).click2d(34.0, 26.0);
    hand.double_click2d(42.0, 36.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    // the corner outside the circle at the crossing near (11, 29), by a click on each curve and one in its quarter
    hand.sk_select();
    assert!(hand.press_hint(&crate::i18n::tr("tb-fillet-sketch-hint")), "no fillet button");
    hand.close_window();
    hand.frame(Vec::new());
    hand.click2d(8.0, 20.0).click2d(3.0, 39.0);
    let x = hand.app.project.crossing_near(si, Point2::new(11.0, 29.0), 6.0).expect("setup: the spline crosses the circle").at;
    hand.hover2d(x.x - 2.5, x.y + 2.5).click2d(x.x - 2.5, x.y + 2.5);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("1.5").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let s = &hand.app.project.sketches[si];
    assert!(s.entities.iter().any(|e| e.kind.as_arc().is_some_and(|a| s.point(a.center).is_some_and(|c| (c.x - 20.0).abs() > 1.0 || (c.y - 20.0).abs() > 1.0))), "setup: no fillet was laid");
    let node = s
        .entities
        .iter()
        .find_map(|e| e.kind.as_spline().map(|sp| sp.nodes.to_vec()))
        .and_then(|n| n.get(n.len() / 2).copied())
        .and_then(|id| s.point(id))
        .map(|q| (q.x, q.y))
        .expect("a node of the spline");
    hand.sk_select();
    let drag = qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Thread, || {
        hand.drag2d(node, (node.0 + 6.0, node.1 - 6.0));
    });
    let budget = std::time::Duration::from_millis(2000);
    assert!(drag.cost.work < budget, "{}", qymcad_core::time_budget::over("the drag of a node of a filleted spline (6.3 s before)", drag.cost, budget));
}
