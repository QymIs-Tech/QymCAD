//! A FIGURE OPENS ITS SIZE: a slot drawn takes its length in the field that comes up at it, and lays it as a dimension
//! only when it is typed; a double click on a straight side of a slot opens that field again; a double click on the middle of a side of a polygon opens its radius - as a side of a
//! rectangle opens its width and height.
//!
//! Reported behaviour: a slot drawn had no field of its size at all, and a polygon opened its size only within 8 px of
//! its construction circle - nowhere near the middle of a side of a large one.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;
use qymcad_core::geom::Point2;

fn sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    app
}

/// The distance between the centres of the ends of the slot of the first sketch.
fn slot_length(app: &App) -> f64 {
    let s = &app.project.sketches[0];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    let centres: Vec<Point2> = s.entities.iter().filter_map(|e| if let Some(qymcad_core::model::ArcParts { center, .. }) = e.kind.as_arc() { at(center) } else { None }).collect();
    match centres[..] {
        [a, b] => a.dist(b),
        _ => f64::NAN,
    }
}

#[test]
fn a_slot_drawn_takes_its_length_in_its_field() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Slot).click2d(0.0, 0.0).click2d(20.0, 0.0).click2d(0.0, 3.0);
    hand.type_text("30").key(egui::Key::Enter);
    let status = hand.app.status.clone();
    let length = slot_length(hand.app);
    assert!((length - 30.0).abs() < 1e-6, "the slot is {length} long, not 30; the status line says {status:?}");
}

#[test]
fn a_slot_drawn_and_left_untyped_holds_no_dimension() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Slot).click2d(0.0, 0.0).click2d(20.0, 0.0).click2d(0.0, 3.0);
    hand.key(egui::Key::Escape);
    let dims = hand.app.project.sketches[0].constraints.iter().filter(|c| matches!(c, qymcad_core::model::Constraint::Distance { .. })).count();
    assert_eq!(dims, 0, "a slot closed with Esc holds {dims} dimensions nobody typed");
}

#[test]
fn a_double_click_on_a_side_of_a_slot_opens_its_length() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Slot).click2d(0.0, 0.0).click2d(20.0, 0.0).click2d(0.0, 3.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    hand.double_click2d(7.0, 3.0);
    hand.type_text("25").key(egui::Key::Enter);
    let status = hand.app.status.clone();
    let length = slot_length(hand.app);
    assert!((length - 25.0).abs() < 1e-6, "the slot is {length} long, not 25; the status line says {status:?}");
}

#[test]
fn a_double_click_on_the_middle_of_a_side_of_a_polygon_opens_its_radius() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // a hexagon R50; its first side runs from (50, 0) to (25, 43.3), and its middle stands 6.7 mm inside the circle
    hand.sk_tool(qymcad_ui_state::SketchShape::Polygon).click2d(0.0, 0.0).click2d(50.0, 0.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    hand.double_click2d(40.0, 17.32);
    hand.type_text("40").key(egui::Key::Enter);
    let s = &hand.app.project.sketches[0];
    let radii: Vec<f64> = s.entities.iter().filter_map(|e| if let Some(qymcad_core::model::CircleParts { r, .. }) = e.kind.as_circle() { Some(r) } else { None }).collect();
    let status = hand.app.status.clone();
    assert!(radii.len() == 1 && (radii[0] - 40.0).abs() < 1e-6, "the circle of the polygon is of radius {radii:?}, not 40; the status line says {status:?}");
}
