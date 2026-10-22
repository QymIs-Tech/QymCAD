//! A POINT DRAWN ON A CURVE IS HELD ON IT, whatever the curve: a line started on an ellipse has its first end held on the
//! ellipse, a line started on a spline on the spline - as a line started on a circle is held on the circle.
//!
//! Reported behaviour: a point snapped onto an ellipse or a spline was held by nothing, and left the curve at the first
//! drag.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::feature::SketchPlane;
use qymcad_core::model::Constraint;

fn sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.enter_sketch_edit(si);
    app
}

/// The constraints of the first sketch, by their names.
fn ties(app: &App) -> Vec<&'static str> {
    app.project.sketches[0]
        .constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::PointOnEllipse { .. } => Some("on ellipse"),
            Constraint::PointOnCurve { .. } => Some("on curve"),
            Constraint::PointOnCircle { .. } => Some("on circle"),
            _ => None,
        })
        .collect()
}

#[test]
fn a_line_started_on_an_ellipse_is_held_on_it() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // an ellipse 8 by 4 about (40, 30); its point 45 deg round is (45.657, 32.828)
    hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(40.0, 30.0).click2d(48.0, 30.0).click2d(40.0, 34.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(45.657, 32.828).double_click2d(60.0, 45.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let got = ties(hand.app);
    eprintln!(
        "PROBE points {:?} cons {:?} status {:?}",
        hand.app.project.sketches[0].points.iter().map(|q| (q.id, q.x, q.y)).collect::<Vec<_>>(),
        hand.app.project.sketches[0].constraints,
        hand.app.status
    );
    assert!(got.contains(&"on ellipse"), "the line started on the ellipse is held by {got:?}, not on the ellipse");
}

#[test]
fn a_line_started_on_a_spline_is_held_on_it() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // a spline through (20, 30), (30, 36), (40, 30): a line started 0.3 of the way along it
    hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(20.0, 30.0).click2d(30.0, 36.0).double_click2d(40.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let on = {
        let s = &hand.app.project.sketches[0];
        let e = s.entities.last().expect("the spline");
        qymcad_core::model::curve::Curve::of(s, e).expect("its curve").at(0.3)
    };
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(on.x, on.y).double_click2d(30.0, 50.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let got = ties(hand.app);
    assert!(got.contains(&"on curve"), "the line started on the spline at ({:.3}, {:.3}) is held by {got:?}, not on the spline", on.x, on.y);
}
