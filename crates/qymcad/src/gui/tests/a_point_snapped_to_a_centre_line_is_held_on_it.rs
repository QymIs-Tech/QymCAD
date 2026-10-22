//! A POINT SNAPPED TO A CENTRE LINE OF A CIRCLE IS HELD ON IT: a circle about (0, 0) of radius 10 drawn by hand, a line
//! from its right quadrant - where the rim crosses the level centre line - out to (30, 0), and a point put on its upright
//! centre line inside it; the centre of the circle dragged 3 right and 4 up - the line still starts at the level of the centre, on the
//! rim, and the point stands on the upright through the centre.
//!
//! Reported behaviour: the centre lines of a circle were drawn and the pointer snapped to them, but what was put there
//! was held by nothing - the start of a line on the quadrant held to the rim alone slid along it.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;

#[test]
fn a_line_from_a_quadrant_and_a_point_on_a_centre_line_go_with_the_centre() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    // off the axes, which hold what stands on them anyway
    let c = (20.0, 20.0);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(c.0, c.1).click2d(c.0 + 10.0, c.1).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(c.0 + 10.0, c.1).click2d(c.0 + 30.0, c.1 - 3.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Point).click2d(c.0, c.1 + 6.0).key(egui::Key::Escape).key(egui::Key::Escape);
    let at = |hand: &Hand, x: f64, y: f64| hand.app.project.sketches[si].points.iter().filter(|q| (q.x - x).hypot(q.y - y) < 0.05).map(|q| q.id).next_back();
    let (start, point) = (at(&hand, c.0 + 10.0, c.1).expect("the start of the line"), at(&hand, c.0, c.1 + 6.0).expect("the point"));
    hand.sk_select().click2d(60.0, -30.0);
    hand.drag2d(c, (c.0 + 3.0, c.1 + 4.0));
    let s = &hand.app.project.sketches[si];
    let place = |id: u64| s.point(id).map(|q| Point2::new(q.x, q.y)).expect("a point");
    let centre = s.entities.iter().find_map(|e| e.kind.as_circle()).map(|k| place(k.center)).expect("the circle");
    let (sp, pp) = (place(start), place(point));
    assert!(
        centre.x - c.0 > 2.0 && centre.y - c.1 > 2.0 && (sp.y - centre.y).abs() < 1e-6 && (pp.x - centre.x).abs() < 1e-6,
        "the centre dragged to {centre:?}: the start of the line at {sp:?} is off its level by {:.4}, the point at {pp:?} off its upright by {:.4}",
        sp.y - centre.y,
        pp.x - centre.x
    );
}
