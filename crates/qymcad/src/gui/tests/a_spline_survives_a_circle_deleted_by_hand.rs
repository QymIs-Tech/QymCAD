//! A SPLINE SURVIVES A CIRCLE DELETED BY HAND, as the report went: a spline and a circle drawn, the circle clicked and
//! deleted with the Delete key - the spline keeps its nodes and is dragged by a node, and Ctrl+Z brings the circle back.
//!
//! Reported behaviour (#166): deleting a circle in a sketch also deleted the control points of a spline in it.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::geom::Point2;

/// Where the nodes of the first spline of sketch `si` stand.
fn nodes(app: &App, si: usize) -> Vec<Point2> {
    let s = &app.project.sketches[si];
    let ids = s.entities.iter().find_map(|e| e.kind.as_spline().map(|sp| sp.nodes.to_vec())).unwrap_or_default();
    ids.iter().filter_map(|id| s.point(*id).map(|q| Point2::new(q.x, q.y))).collect()
}

#[test]
fn a_circle_deleted_leaves_the_spline_beside_it_whole() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(10.0, 30.0).click2d(20.0, 38.0).double_click2d(30.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(60.0, 30.0).click2d(65.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
    let before = nodes(hand.app, si);
    assert_eq!(before.len(), 3, "setup: the spline of three nodes drawn");
    hand.sk_select().click2d(65.0, 30.0).key(egui::Key::Delete);
    let circles = |a: &App| a.project.sketches[si].entities.iter().filter(|e| e.kind.as_circle().is_some()).count();
    let mut failures = Vec::new();
    if circles(hand.app) != 0 {
        failures.push("the circle clicked and deleted is still there".to_string());
    }
    if nodes(hand.app, si) != before {
        failures.push(format!("the spline beside the circle deleted stands on {:?}, not {before:?}", nodes(hand.app, si)));
    }
    // the spline is still a curve of the sketch: a node of it dragged moves
    hand.drag2d((20.0, 38.0), (20.0, 42.0));
    if !nodes(hand.app, si).iter().any(|q| q.dist(Point2::new(20.0, 42.0)) < 0.5) {
        failures.push(format!("a node of the spline dragged to (20, 42) stands on {:?}", nodes(hand.app, si)));
    }
    hand.undo().undo();
    if circles(hand.app) != 1 || nodes(hand.app, si) != before {
        failures.push(format!("Ctrl+Z twice: {} circles, the spline on {:?}", circles(hand.app), nodes(hand.app, si)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
