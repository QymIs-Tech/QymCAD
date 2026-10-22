//! A LINE DRAWN BY A CURVE TOUCHES IT, of any kind, on the path of a person: a circle, an ellipse and a spline drawn by
//! hand, and a line drawn level a hair above the top of each - the line is made to touch the curve, as it was the circle
//! alone.
//!
//! Reported behaviour: the automatic tangency took a circle and an arc, and an ellipse or a spline a line was drawn along
//! was left untouched.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::model::Constraint;

/// THE CURVE A LINE IS DRAWN BY.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Circle,
    Ellipse,
    Spline,
}

#[test]
fn a_line_drawn_along_a_curve_of_any_kind_is_made_to_touch_it() {
    let mut failures = Vec::new();
    for kind in [Kind::Circle, Kind::Ellipse, Kind::Spline] {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        // each with its top at (30, 30)
        match kind {
            Kind::Circle => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(30.0, 20.0).click2d(40.0, 20.0);
            }
            Kind::Ellipse => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(30.0, 22.0).click2d(44.0, 22.0).click2d(30.0, 30.0);
            }
            Kind::Spline => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(16.0, 22.0).click2d(30.0, 30.0);
                hand.double_click2d(44.0, 22.0);
            }
        }
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        // a line level a hair above the top, clear of the snaps of the curve
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(18.0, 30.12).double_click2d(42.0, 30.12);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si];
        let touches = s.constraints.iter().any(|c| matches!(c, Constraint::Tangent { .. } | Constraint::TangentCurves { .. }));
        if !touches {
            failures.push(format!("{kind:?}: the line drawn along the top is not made to touch it: {:?}", s.constraints));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
