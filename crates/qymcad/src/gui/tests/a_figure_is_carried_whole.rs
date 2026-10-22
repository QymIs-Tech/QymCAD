//! A FIGURE IS CARRIED WHOLE: one side of a polygon chosen, Copy lays the whole polygon - its sides and its circle - and
//! the copy is a polygon of its own; one side of a rectangle chosen, Mirror lays the whole rectangle, and the image is a
//! rectangle of its own.
//!
//! Reported behaviour: the tools carried the side clicked alone out of the figure, and what they laid were plain curves.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

#[test]
fn one_side_of_a_polygon_copied_copies_the_polygon() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    // a hexagon about (0, 0) through (14, 0); its first side is clicked 0.3 along from (14, 0) to (7, 12.12)
    hand.sk_tool(qymcad_ui_state::SketchShape::Polygon).click2d(0.0, 0.0).click2d(14.0, 0.0);
    hand.key(egui::Key::Escape);
    hand.sk_select().click2d(11.9, 3.64);
    hand.sk_move_tool(qymcad_ui_state::MoveKind::Copy).click2d(40.0, -30.0).click2d(70.0, -30.0);
    let s = &hand.app.project.sketches[si];
    let circles = s.entities.iter().filter(|e| e.kind.as_circle().is_some()).count();
    let status = hand.app.status.clone();
    assert_eq!(s.entities.len(), 14, "the sketch holds {} curves, not two hexagons with their circles; the status line says {status:?}", s.entities.len());
    assert_eq!(circles, 2, "{circles} circles, not one for each hexagon");
    assert_eq!(s.figures.len(), 2, "{} figures: the copy is no polygon", s.figures.len());
}

#[test]
fn one_side_of_a_rectangle_mirrored_mirrors_the_rectangle() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(5.0, 0.0).click2d(25.0, 10.0);
    hand.key(egui::Key::Escape);
    hand.sk_select().click2d(11.0, 0.0);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-mirror-sketch-hint")), "no mirror button");
    hand.close_window();
    hand.click2d(0.0, 30.0); // the Y axis
    let s = &hand.app.project.sketches[si];
    let status = hand.app.status.clone();
    let left = s.points.iter().filter(|q| q.x < -4.9 && q.x > -25.1).count();
    assert_eq!(s.rects().len(), 2, "{} rectangles: the image is no rectangle; the status line says {status:?}", s.rects().len());
    assert!(left >= 4, "{left} points left of the Y axis, not the four corners of the image");
}
