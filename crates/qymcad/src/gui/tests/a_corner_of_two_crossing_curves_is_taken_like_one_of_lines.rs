//! A CORNER OF TWO CROSSING CURVES IS TAKEN LIKE ONE OF LINES, on the path of a person: two circles, an arc and a circle,
//! a spline and a circle, two lines - each drawn by its tool. The fillet or the chamfer taken, a click on each curve (no
//! Shift), the pointer led 2.5 mm beside a crossing into the quarter above it, out of the circle: the preview marks where
//! the curves will be cut. A click there and the size of the bar typed with Enter cut that corner - the arc or the cut
//! line stands in the quarter where the preview drew it - and nothing of the drawing ends on the crossing any more: the halves beyond it are gone.
//!
//! A spline crossing the circle twice keeps its tail past the second crossing; a spline ending inside the circle takes the
//! corner as well.
//!
//! Reported behaviour: at the crossing of circles, arcs or splines the fillet showed no box, a strange preview, and the
//! curves could not be chosen; with lines the same steps worked. The fillet at one crossing of a spline and a circle took
//! the half of the spline beyond it with the second crossing on it, and at a spline ending inside a circle it did
//! nothing.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{ChamferLegs, CornerCut, FilletSize, Id};

/// THE TWO CURVES THAT CROSS.
#[derive(Clone, Copy, Debug)]
enum Two {
    Circles,
    ArcAndCircle,
    SplineAndCircle,
    /// A spline ending inside the circle, crossing it once.
    SplineEndingInside,
    Lines,
}

/// WHICH TOOL CUTS THE CORNER.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tool {
    Fillet,
    Chamfer,
}

/// The circle about (20, 20) of radius 10, drawn by its tool.
fn circle(hand: &mut Hand) {
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(20.0, 20.0).click2d(30.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
}

/// The two curves drawn by hand in a new sketch.
fn drawn(two: Two) -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    match two {
        Two::Circles => {
            circle(&mut hand);
            hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(32.0, 20.0).click2d(42.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
        }
        Two::ArcAndCircle => {
            circle(&mut hand);
            hand.sk_tool(qymcad_ui_state::SketchShape::Arc).click2d(32.0, 20.0).click2d(30.26, 29.85).click2d(30.26, 10.15).key(egui::Key::Escape).key(egui::Key::Escape);
        }
        Two::SplineAndCircle => {
            circle(&mut hand);
            hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(8.0, 36.0).click2d(20.0, 24.0);
            hand.double_click2d(34.0, 36.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
        }
        Two::SplineEndingInside => {
            circle(&mut hand);
            hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(4.0, 36.0).click2d(10.0, 29.0);
            hand.double_click2d(16.0, 22.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
        }
        Two::Lines => {
            hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 28.0).double_click2d(40.0, 28.0);
            hand.click2d(20.0, 14.0).double_click2d(32.0, 42.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
        }
    }
    hand.sk_select();
    app
}

/// A CURVE OF THE SKETCH and its id.
#[derive(Clone)]
struct Drawn {
    id: Id,
    curve: Curve,
}

/// The curves of the sketch.
fn curves(app: &App) -> Vec<Drawn> {
    let s = &app.project.sketches[0];
    s.entities.iter().filter_map(|e| Curve::of(s, e).map(|curve| Drawn { id: e.id, curve })).collect()
}

/// A place on `curve`, off its ends, at least 6 mm from every other curve - out of the zone of a crossing, 30 px at 6 px/mm: where a click picks
/// it and nothing else.
fn clear_of(curve: &Curve, others: &[Curve]) -> Option<Point2> {
    (20..=180).map(|k| curve.at(k as f64 / 200.0)).find(|q| others.iter().all(|o| o.nearest(*q).off > 6.0))
}

/// The two curves of the corner taken by a click on each, the pointer led into the quarter above the crossing, the preview read, a click there and the size typed. Answers what went wrong.
fn cut(two: Two, tool: Tool) -> Result<(), String> {
    let mut app = drawn(two);
    let drawn_curves = curves(&app);
    let [one, other] = drawn_curves.as_slice() else { return Err(format!("{} curves drawn, want 2", drawn_curves.len())) };
    let (Drawn { id: a, curve: ca }, Drawn { id: b, curve: cb }) = (one.clone(), other.clone());
    let crossing = app.project.crossing_of(0, [a, b], Point2::new(26.0, 30.0)).ok_or("the two drawn do not cross")?.at;
    // up from the crossing, out of the circle and between the outer halves; up and left between two lines
    let out = match two {
        Two::Lines => Point2::new(-std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2),
        _ => Point2::new(0.0, 1.0),
    };
    let toward = Point2::new(crossing.x + 2.5 * out.x, crossing.y + 2.5 * out.y);
    let crossing_of = app.project.crossing_of(0, [a, b], crossing).ok_or("no crossing to read")?;
    let (pick_a, pick_b) = (clear_of(&ca, std::slice::from_ref(&cb)).ok_or("no place to pick the first")?, clear_of(&cb, std::slice::from_ref(&ca)).ok_or("no place to pick the second")?);
    let mut hand = Hand::new(&mut app);
    let hint = match tool {
        Tool::Fillet => "tb-fillet-sketch-hint",
        Tool::Chamfer => "tb-chamfer-sketch-hint",
    };
    if !hand.press_hint(&crate::i18n::tr(hint)) {
        return Err("no button of the tool".into());
    }
    hand.close_window();
    hand.frame(Vec::new());
    // the size the bar carries, which the preview is drawn with and which is typed
    let v = hand.app.tool_prefs.fillet;
    let size = match tool {
        Tool::Fillet => CornerCut::Fillet(FilletSize::radius(v)),
        Tool::Chamfer => CornerCut::Chamfer(ChamferLegs::equal(v)),
    };
    let shown = hand.app.project.crossing_blend(0, crossing_of, toward, size).ok_or(format!("the corner of the quarter takes no cut of {v}"))?;
    hand.click2d(pick_a.x, pick_a.y).click2d(pick_b.x, pick_b.y);
    let chosen: Vec<Id> = hand.app.tools.sel_sk.curves().collect();
    if !(chosen.contains(&a) && chosen.contains(&b)) {
        return Err(format!("a click on each curve chose {chosen:?}, not both of {a} and {b}"));
    }
    // the preview marks where the two will be cut, before the click
    hand.hover2d(toward.x, toward.y);
    let rings = hand.rings_drawn(3.0);
    let marked = |p: [f64; 2]| rings.iter().any(|r| r.distance(hand.on_screen2d((p[0], p[1]))) < 1.5);
    if !(marked(shown.ends[0]) && marked(shown.ends[1])) {
        return Err(format!("the pointer in the quarter marks no cut at {:?}: rings {rings:?}", shown.ends));
    }
    hand.click2d(toward.x, toward.y);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text(&v.to_string()).key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let status = hand.app.status.clone();
    let after = curves(hand.app);
    let laid = after.iter().map(|d| &d.curve).any(|c| {
        let [p, q] = c.ends();
        let mid = c.at(0.5);
        let in_quarter = (mid.x - crossing.x) * out.x + (mid.y - crossing.y) * out.y > 0.0;
        let ends_as_shown = |e: [f64; 2]| [p, q].iter().any(|x| x.dist(Point2::new(e[0], e[1])) < 1e-6);
        let kind = match tool {
            Tool::Fillet => matches!(c, Curve::Round { r, .. } if (r - v).abs() < 1e-6),
            Tool::Chamfer => matches!(c, Curve::Line { .. }),
        };
        kind && in_quarter && ends_as_shown(shown.ends[0]) && ends_as_shown(shown.ends[1])
    });
    if !laid {
        return Err(format!("no {tool:?} of {v} in the quarter where the preview stood; the status line says {status:?}"));
    }
    // a circle crossed once keeps its rest round the long way, ending on the crossing; nothing else ends there
    let rest_of_the_circle = |c: &Curve| matches!(two, Two::SplineEndingInside) && matches!(c, Curve::Round { r, .. } if (r - 10.0).abs() < 1e-3);
    let on_crossing = after.iter().map(|d| &d.curve).filter(|c| !c.closed() && !rest_of_the_circle(c) && c.ends().iter().any(|p| p.dist(crossing) < 1e-6)).count();
    if matches!(two, Two::SplineEndingInside) && after.iter().any(|d| d.curve.ends().iter().any(|q| q.dist(Point2::new(16.0, 22.0)) < 1e-6)) {
        return Err("the piece of the spline inside the circle stayed".into());
    }
    if on_crossing != 0 {
        return Err(format!("{on_crossing} curves still end on the crossing: the halves beyond the corner stayed"));
    }
    // the half of the spline beyond the corner goes as far as its second crossing: its tail past that stays
    if matches!(two, Two::SplineAndCircle) && !after.iter().any(|d| d.curve.ends().iter().any(|q| q.dist(Point2::new(34.0, 36.0)) < 1e-6)) {
        return Err("the tail of the spline past its second crossing went with the half beyond the corner".into());
    }
    Ok(())
}

#[test]
fn a_corner_of_two_crossing_curves_is_cut_by_a_click_on_each_and_one_in_its_quarter() {
    let mut failures = Vec::new();
    for two in [Two::Circles, Two::ArcAndCircle, Two::SplineAndCircle, Two::SplineEndingInside, Two::Lines] {
        for tool in [Tool::Fillet, Tool::Chamfer] {
            if let Err(why) = cut(two, tool) {
                failures.push(format!("{two:?} {tool:?}: {why}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
