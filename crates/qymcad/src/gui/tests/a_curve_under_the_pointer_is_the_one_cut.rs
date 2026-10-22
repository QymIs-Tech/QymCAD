//! THE CURVE UNDER THE POINTER IS THE ONE CUT, RUN TO AND MEASURED: Trim takes the curve nearest the pointer, of whatever
//! kind - a spline under the pointer is trimmed, though a line runs within the grab of a curve beside it; Extend runs a
//! line to the ellipse under the pointer, past a nearer circle on its way; a dimension from a line to a circle under the
//! pointer measures to the circle, though a line parallel to the first runs within the grab beside it.
//!
//! Reported behaviour: a line within the grab radius was taken before a spline under the pointer, and the click cut the
//! line nobody pointed at; an ellipse under the pointer was no target of Extend, and the line stopped at the nearer
//! circle; a dimension took the parallel line beside a circle clicked.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;

#[test]
fn trim_takes_the_spline_under_the_pointer_and_not_the_line_beside_it() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    // a spline over (30, 31), and a line along y = 30 a millimetre - 6 px - below it, within the grab of a curve (8 px),
    // drawn with no ties, so it is not made to touch the spline it runs along
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(20.0, 32.0).click2d(30.0, 31.0).double_click2d(40.0, 32.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line);
    assert!(hand.press_word(&qymcad_i18n::tr("opt-no-ties-short"), egui::pos2(400.0, 0.0)), "no box of no ties on the bar of the line");
    hand.click2d(10.0, 30.0).double_click2d(50.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-trim-hint")), "no trim button");
    hand.close_window();
    hand.click2d(30.0, 31.0);
    let kinds: Vec<&str> = hand.app.project.sketches[si]
        .entities
        .iter()
        .map(|e| match (e.kind.as_line(), e.kind.as_spline()) {
            (Some(_), _) => "line",
            (_, Some(_)) => "spline",
            _ => "other",
        })
        .collect();
    let status = hand.app.status.clone();
    assert_eq!(kinds, ["line"], "the sketch holds {kinds:?} after the trim under the pointer on the spline; the status line says {status:?}");
}

/// The x of the ends of line `line` of sketch `si`.
fn ends_x(hand: &Hand, si: usize, line: u64) -> [f64; 2] {
    let s = &hand.app.project.sketches[si];
    let ends = s.entity(line).and_then(|e| e.kind.as_line()).expect("the line");
    let x = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| q.x).expect("an end");
    [x(ends.a), x(ends.b)]
}

#[test]
fn extend_runs_to_the_ellipse_under_the_pointer_past_a_nearer_circle() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    // the line (0, 0) - (10, 0); on its axis a circle about (20, 0) of radius 2 first, then an ellipse about (40, 0)
    // 5 by 3: the pointer over the top of the ellipse, 3 mm off the axis, names the ellipse
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(10.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
    let line = hand.app.project.sketches[si].entities[0].id;
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(20.0, 0.0).click2d(22.0, 0.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(40.0, 0.0).click2d(45.0, 0.0).click2d(40.0, 3.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-extend-hint")), "no Extend button");
    hand.close_window();
    hand.click2d(5.0, 0.0).click2d(40.0, 3.0);
    let [a, b] = ends_x(&hand, si, line);
    let status = hand.app.status.clone();
    assert!((a - 0.0).abs() < 1e-6 && (b - 35.0).abs() < 1e-3, "the line runs from {a:.4} to {b:.4}, not from 0 to the ellipse at 35; the status line says {status:?}");
}

#[test]
fn a_dimension_to_a_circle_beside_a_line_measures_to_the_circle() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    // a line along x = 0, a circle about (20, 20) of radius 5, and a line along x = 14 a millimetre - 6 px - left of its
    // rim, parallel to the first and within the grab of a curve (8 px) of the place clicked on the rim; all of it clear
    // of the axes, which take a click of their own
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 10.0).click2d(0.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(20.0, 20.0).click2d(25.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(14.0, 10.0).click2d(14.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select();
    assert!(hand.press_hint(&qymcad_i18n::tr("tb-dim-hint")), "the dimension tool");
    hand.click2d(0.0, 25.0).click2d(15.0, 20.0).click2d(8.0, 40.0).key(egui::Key::Escape);
    use qymcad_core::model::Constraint;
    let dimension = |c: &&Constraint| matches!(c, Constraint::Distance { .. } | Constraint::DistancePL { .. } | Constraint::CurveDistance { .. } | Constraint::Diameter { .. });
    let laid: Vec<String> = hand.app.project.sketches[si].constraints.iter().filter(dimension).map(|c| format!("{c:?}")).collect();
    let to_circle = laid.iter().any(|c| c.starts_with("CurveDistance"));
    assert!(to_circle && laid.len() == 1, "the dimensions laid: {laid:?} - not one least distance from the line to the circle under the pointer");
}
