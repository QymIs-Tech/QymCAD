//! A CROSSING OF ANY CURVE IS SNAPPED TO, by one path of the snaps: a spline crossing the X axis and an ellipse crossing a
//! centre line of a circle drawn by hand, and a line started a few pixels off each crossing - its start stands on the
//! crossing, as it would on the crossing of a line or a circle with them.
//!
//! Reported behaviour: the axes and the centre lines of circles were crossed with lines and circles alone, and a line
//! started at the crossing of a spline with the X axis stood where the pointer was.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;

/// THE CROSSING A LINE IS STARTED AT.
#[derive(Clone, Copy, Debug)]
enum Where {
    SplineAndAxis,
    EllipseAndCentreLine,
}

#[test]
fn a_line_started_by_a_crossing_of_any_curve_starts_on_it() {
    let mut failures = Vec::new();
    for at in [Where::SplineAndAxis, Where::EllipseAndCentreLine] {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        match at {
            Where::SplineAndAxis => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(10.0, 8.0).click2d(18.0, -6.0);
                hand.double_click2d(26.0, -10.0);
            }
            Where::EllipseAndCentreLine => {
                // a circle about (40, 30) of radius 10 - its level centre line runs from x 27.5 to 52.5 - and an ellipse
                // about (50, 33) whose rim crosses that line at x 43.07, inside the circle
                hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(40.0, 30.0).click2d(40.0, 40.0).key(egui::Key::Escape).key(egui::Key::Escape);
                hand.sk_tool(qymcad_ui_state::SketchShape::Ellipse).click2d(50.0, 33.0).click2d(58.0, 33.0).click2d(50.0, 39.0);
            }
        }
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        // the crossing, read off the drawing: of the curve drawn last with the X axis, or with the line y = 30
        let s = &hand.app.project.sketches[si];
        let curve = s.entities.iter().filter_map(|e| Curve::of(s, e)).last().expect("setup: a curve");
        let level = match at {
            Where::SplineAndAxis => 0.0,
            Where::EllipseAndCentreLine => 30.0,
        };
        let Some(crossing) = curve.meets_segment([Point2::new(-100.0, level), Point2::new(100.0, level)]).into_iter().find(|q| q.x > 0.0 && q.x < 45.0) else {
            failures.push(format!("{at:?}: setup: the curve {curve:?} does not cross y = {level}"));
            continue;
        };
        // 3 px off it
        let off = 3.0 / f64::from(hand.app.viewing.view.scale);
        hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(crossing.x + off, crossing.y + off).double_click2d(crossing.x + 20.0, crossing.y + 15.0);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si];
        let start = s.entities.iter().filter_map(|e| e.kind.as_line().and_then(|l| s.point(l.a))).map(|q| Point2::new(q.x, q.y)).last();
        if !start.is_some_and(|q| q.dist(crossing) < 1e-6) {
            failures.push(format!("{at:?}: the line started 3 px off the crossing {crossing:?} starts at {start:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
