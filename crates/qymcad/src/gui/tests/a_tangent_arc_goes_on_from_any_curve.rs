//! A TANGENT ARC GOES ON FROM ANY CURVE, on the path of a person: a line, an arc and a spline drawn by hand, the arc
//! tool in its tangent mode, a click on the end of each and one beside it - the arc is laid, leaving the end the way the
//! curve arrives there, and held touching it.
//!
//! Reported behaviour: a tangent arc went on from the end of a line or an arc alone; from a spline or an arc of an
//! ellipse the tool asked for the end of a line or an arc again.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;

/// THE CURVE THE ARC GOES ON FROM.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Line,
    Arc,
    Spline,
}

#[test]
fn a_tangent_arc_goes_on_from_the_end_of_a_curve_of_any_kind() {
    let mut failures = Vec::new();
    for kind in [Kind::Line, Kind::Arc, Kind::Spline] {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        // each ending at (30, 20)
        match kind {
            Kind::Line => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 14.0).double_click2d(30.0, 20.0);
            }
            Kind::Arc => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Arc).click2d(20.0, 20.0).click2d(20.0, 30.0).click2d(30.0, 20.0);
            }
            Kind::Spline => {
                hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(10.0, 10.0).click2d(20.0, 24.0);
                hand.double_click2d(30.0, 20.0);
            }
        }
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        let before: Vec<qymcad_core::model::Id> = hand.app.project.sketches[si].entities.iter().map(|e| e.id).collect();
        let base_id = {
            let s = &hand.app.project.sketches[si];
            s.entities.iter().find(|e| Curve::of(s, e).is_some_and(|c| c.ends().iter().any(|q| q.dist(Point2::new(30.0, 20.0)) < 1e-6))).map(|e| e.id).expect("setup: the curve ends at (30, 20)")
        };
        hand.sk_tool(qymcad_ui_state::SketchShape::Arc);
        assert!(hand.press_word(&crate::i18n::tr("opt-tangent"), egui::pos2(400.0, 0.0)), "no tangent mode on the bar of the arc");
        hand.click2d(30.0, 20.0).click2d(40.0, 30.0);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si];
        let Some(arc) = s.entities.iter().filter(|e| !before.contains(&e.id)).find_map(|e| e.kind.as_arc().and_then(|_| Curve::of(s, e))) else {
            failures.push(format!("{kind:?}: no arc laid; the status line says {:?}", hand.app.status));
            continue;
        };
        // the way the curve arrives at the shared end, as the solve left it, against the way the arc leaves it
        let Some(base) = s.entity(base_id).and_then(|e| Curve::of(s, e)) else {
            failures.push(format!("{kind:?}: the curve the arc went on from is gone"));
            continue;
        };
        let joint = arc.ends().into_iter().find(|q| base.ends().iter().any(|b| b.dist(*q) < 1e-6)).unwrap_or(Point2::new(30.0, 20.0));
        let at_end = if base.at(1.0).dist(joint) < 1e-6 { base.tangent(1.0) } else { Point2::new(-base.tangent(0.0).x, -base.tangent(0.0).y) };
        let leaves = if arc.at(0.0).dist(joint) < 1e-6 { arc.tangent(0.0) } else { Point2::new(-arc.tangent(1.0).x, -arc.tangent(1.0).y) };
        let along = at_end.x * leaves.x + at_end.y * leaves.y;
        if (along - 1.0).abs() > 1e-6 {
            failures.push(format!("{kind:?}: the arc leaves the end {:.4} rad off the way the curve arrives", along.clamp(-1.0, 1.0).acos()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
