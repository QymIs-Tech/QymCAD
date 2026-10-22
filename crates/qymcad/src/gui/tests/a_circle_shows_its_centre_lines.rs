//! A CIRCLE SHOWS ITS CENTRE LINES, in the window: a dashed horizontal and vertical axis through its centre, a quarter of
//! the radius past the rim, and the pointer snaps to them and to where they cross the rim, as in every CAD.
//!
//! Reported behaviour: a circle had no dashed cross of axes through its centre to lay dimensions, geometry and tangents
//! off.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

/// A sketch open for editing.
fn a_sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    app
}

/// Where the lines of the first sketch start.
fn line_starts(app: &App) -> Vec<[f64; 2]> {
    let s = &app.project.sketches[0];
    let at = |id: u64| s.points.iter().find(|p| p.id == id).map(|p| [p.x, p.y]);
    s.entities.iter().filter_map(|e| if let Some(qymcad_core::model::LineParts { a, .. }) = e.kind.as_line() { at(a) } else { None }).collect()
}

/// A LINE STARTED NEAR THE VERTICAL CENTRE LINE STARTS ON IT: a circle around (50.5, 30.5), off the nodes of the grid,
/// and a click at (50.7, 36) start the line at x = 50.5 - the grid alone would have put it on 51 or left it at 50.7.
#[test]
fn a_line_started_near_a_centre_line_starts_on_it() {
    let mut app = a_sketch();
    let _ = app.project.add_circle_entity(0, 50.5, 30.5, 10.0, qymcad_core::feature::Purpose::Real);
    Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(50.7, 36.0).double_click2d(90.0, 50.0);
    let starts = line_starts(&app);
    assert!(starts.iter().any(|[x, _]| (x - 50.5).abs() < 1e-6), "the line started at {starts:?}, not on the centre line x = 50.5");
}

/// THE CENTRE LINES ARE DRAWN: dashed strokes past the rim of a circle of radius 10 around (50, 30) - at (50, 41.5) on
/// the vertical one and (61.5, 30) on the horizontal one, where nothing else is drawn in their colour.
#[test]
fn the_centre_lines_of_a_circle_are_drawn() {
    let mut app = a_sketch();
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(50.0, 30.0).click2d(60.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    let colour = hand.app.scheme.pal.sketch_construction().gamma_multiply(0.7);
    let (vertical, horizontal) = (hand.strokes_near2d((50.0, 41.5), colour), hand.strokes_near2d((61.5, 30.0), colour));
    assert!(vertical > 0 && horizontal > 0, "the centre lines of the circle are drawn by {vertical} strokes by the vertical one, {horizontal} by the horizontal one");
}
