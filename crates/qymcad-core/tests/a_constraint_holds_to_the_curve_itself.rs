//! A CONSTRAINT ON A CURVE HOLDS TO THE CURVE ITSELF, not to a copy of its shape taken when it was laid: the end of a
//! line put on a spline stays on what is left of the spline when its far end is trimmed away, and the constraint saved
//! names the curve and carries nothing of its handles.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{curve_ref_of, Constraint, Id, Project};

/// A DRAWING: the project, its sketch, and the end of the line put on the spline.
struct Drawn {
    p: Project,
    si: usize,
    end: Id,
}

/// The spline (0, 0) -> (10, 6) -> (20, 0) -> (30, 6), a line from (5, -10) whose end is put on it at a fifth of it,
/// and an upright line across it at x = 25.
fn drawn() -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let nodes = [[0.0, 0.0], [10.0, 6.0], [20.0, 0.0], [30.0, 6.0]].map(|[x, y]| Point2::new(x, y));
    let spline = p.add_spline(si, nodes.to_vec(), Ends::Open, Purpose::Real).expect("the spline");
    let s = &p.sketches[si];
    let on = Curve::of(s, s.entity(spline).expect("the spline")).expect("its curve").at(0.2);
    p.add_line_entity(si, 5.0, -10.0, on.x, on.y, Purpose::Real);
    p.add_line_entity(si, 25.0, -10.0, 25.0, 20.0, Purpose::Real);
    let s = &p.sketches[si];
    let end = s.points.iter().find(|q| (q.x - on.x).abs() < 1e-9 && (q.y - on.y).abs() < 1e-9).map(|q| q.id).expect("the end of the line");
    let held = curve_ref_of(s.entity(spline).expect("the spline"));
    p.sketches[si].constraints.push(Constraint::PointOnCurve { p: end, on: held });
    p.solve_sketch(si);
    Drawn { p, si, end }
}

/// How far point `id` stands off the nearest spline of the sketch.
fn off_the_spline(p: &Project, si: usize, id: Id) -> f64 {
    let s = &p.sketches[si];
    let q = s.point(id).map(|q| Point2::new(q.x, q.y)).expect("the point");
    s.entities.iter().filter(|e| e.kind.as_spline().is_some()).filter_map(|e| Curve::of(s, e)).map(|c| c.nearest(q).off).fold(f64::MAX, f64::min)
}

#[test]
fn a_point_on_a_spline_stays_on_what_is_left_of_it_after_a_trim() {
    let Drawn { mut p, si, end } = drawn();
    let spline = p.sketches[si].entities.iter().find(|e| e.kind.as_spline().is_some()).map(|e| e.id).expect("the spline");
    assert!(p.trim_curve(si, spline, 29.0, 5.0), "the far end of the spline is not trimmed");
    let held = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::PointOnCurve { p, .. } if *p == end));
    assert!(held, "the end of the line lost its place on the spline when the far end of the spline was trimmed away");
    // the first node of what is left moved: the end of the line goes with the curve
    let first = p.sketches[si].entities.iter().find_map(|e| e.kind.as_spline().map(|sp| sp.nodes[0])).expect("what is left of the spline");
    if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.id == first) {
        q.y -= 3.0;
    }
    p.solve_sketch(si);
    let off = off_the_spline(&p, si, end);
    assert!(off < 1e-6, "the end of the line stands {off:.6} off the spline after its first node moved");
}

#[test]
fn a_constraint_saved_names_the_curve_and_carries_none_of_its_shape() {
    let Drawn { p, si, end } = drawn();
    let c = p.sketches[si].constraints.iter().find(|c| matches!(c, Constraint::PointOnCurve { p, .. } if *p == end)).expect("the point on the spline");
    let saved = ron::to_string(c).expect("saved");
    assert!(!saved.contains("handles") && !saved.contains("nodes"), "the constraint carries a copy of the spline: {saved}");
}

#[test]
fn a_constraint_on_a_curve_goes_with_the_curve_and_comes_with_its_copy() {
    // the line and the spline copied and put 40 to the right: the end of the copied line on the copied spline
    let Drawn { mut p, si, end } = drawn();
    let s = &p.sketches[si];
    let spline = s.entities.iter().find(|e| e.kind.as_spline().is_some()).map(|e| e.id).expect("the spline");
    let line = s.entities.iter().find(|e| e.kind.as_line().is_some_and(|l| l.b == end)).map(|e| e.id).expect("the line");
    let clip = p.copy_sketch_geometry(si, &[line, spline], 0.0, 0.0);
    let pasted = p.paste_sketch_geometry(si, &clip, 40.0, 0.0);
    let s = &p.sketches[si];
    let copied = pasted.iter().copied().find(|id| s.entity(*id).is_some_and(|e| e.kind.as_spline().is_some())).expect("the copied spline");
    let on_copy = s.constraints.iter().any(|c| matches!(c, Constraint::PointOnCurve { on, .. } if on.curve_id() == Some(copied)));
    assert!(on_copy, "the copy of the line does not stand on the copy of the spline: {:?}", s.constraints);

    // the spline deleted: nothing holds to it
    let Drawn { mut p, si, end } = drawn();
    let spline = p.sketches[si].entities.iter().find(|e| e.kind.as_spline().is_some()).map(|e| e.id).expect("the spline");
    p.delete_entities(si, &[spline]);
    let left = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::PointOnCurve { p, .. } if *p == end));
    assert!(!left, "the end of the line is held on a spline that is gone");
}
