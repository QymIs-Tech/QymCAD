//! Solver diagnostics: which constraints exactly are in conflict.
//!
//! The solver used to report only a single residual for the whole sketch, leaving the culprit to be found by
//! switching constraints off one at a time. There are two answers here: the residual of each constraint
//! separately, and the set of constraints that are contradictory together — the system cannot be satisfied
//! until one of them is removed or made driven.
use qymcad_core::model::{Constraint, Project};

fn sketch(p: &mut Project) -> usize {
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    si
}

/// Two contradictory dimensions on one segment: the solver has to name both rather than report a residual.
#[test]
fn two_contradicting_dimensions_are_both_named() {
    let mut p = Project::default();
    p.new_document();
    let si = sketch(&mut p);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    s.constraints.push(Constraint::Distance { a, b, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 0 }); // contradicts the previous one
    p.solve_sketch(si);

    let bad = p.sketch_conflicts(si);
    assert!(bad.contains(&2) && bad.contains(&3), "both conflicting dimensions are named, got {bad:?}");
    assert!(!bad.contains(&1), "the horizontal does not contradict them and stays out of the list: {bad:?}");
}

/// A consistent sketch: no conflicts, and every constraint residual is zero.
#[test]
fn a_consistent_sketch_reports_no_conflicts() {
    let mut p = Project::default();
    p.new_document();
    let si = sketch(&mut p);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 42.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);

    assert!(p.sketch_conflicts(si).is_empty(), "there are no conflicts: {:?}", p.sketch_conflicts(si));
    let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
    assert!(worst < 1e-7, "every constraint is satisfied, worst residual {worst:.3e}");
}

/// A driven dimension constrains nothing and never enters a conflict, even when its value disagrees with the
/// geometry: it merely measures it.
#[test]
fn a_driven_dimension_never_conflicts() {
    let mut p = Project::default();
    p.new_document();
    let si = sketch(&mut p);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    s.constraints.push(Constraint::Distance { a, b, d: 999.0, off: 0.0, expr: String::new(), driven: true, axis: 0 });
    p.solve_sketch(si);
    assert!(p.sketch_conflicts(si).is_empty(), "a driven dimension does not conflict: {:?}", p.sketch_conflicts(si));
}

/// A residual per constraint, so which one is unsatisfied is visible rather than a single figure for the
/// sketch.
#[test]
fn per_constraint_residuals_point_at_the_broken_one() {
    let mut p = Project::default();
    p.new_document();
    let si = sketch(&mut p);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    s.constraints.push(Constraint::Distance { a, b, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);

    let r = p.sketch_residuals(si);
    assert!(r[1] < 1e-6, "the horizontal is satisfied: {:.3e}", r[1]);
    assert!(r[2] > 1e-3 || r[3] > 1e-3, "at least one of the conflicting dimensions is unsatisfied: {:?}", &r[2..4]);
}

/// Geometric constraints conflict too, not only dimensions: a horizontal together with a vertical collapses a
/// segment to a point while its length is fixed. The old heuristic of a dimension disagreeing with the geometry
/// did not see such a case at all.
///
/// What is checked is the contract of the set rather than its exact membership: it contains a geometric
/// constraint, and removing any of its members clears the conflict — which is exactly what the hint in the panel
/// promises.
#[test]
fn geometric_constraints_conflict_too() {
    let build = || {
        let mut p = Project::default();
        p.new_document();
        let si = sketch(&mut p);
        let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
        let s = &mut p.sketches[si];
        s.constraints.push(Constraint::Fixed { p: a });
        s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        s.constraints.push(Constraint::Horizontal { a, b });
        s.constraints.push(Constraint::Vertical { a, b });
        p.solve_sketch(si);
        (p, si)
    };
    let (p, si) = build();
    let bad = p.sketch_conflicts(si);
    assert!(!bad.is_empty(), "a conflict was found");
    let is_dim = |p: &Project, ci: usize| matches!(p.sketches[si].constraints[ci], Constraint::Distance { .. });
    assert!(bad.iter().any(|&ci| !is_dim(&p, ci)), "the set contains a geometric constraint and not only a dimension: {bad:?}");
    // the contract: removing any of the named constraints makes the sketch solvable
    for &ci in &bad {
        let (mut q, si) = build();
        q.delete_sketch_constraint(si, ci);
        q.solve_sketch(si);
        assert!(q.sketch_conflicts(si).is_empty(), "removing constraint {ci} from the set clears the conflict, leaving {:?}", q.sketch_conflicts(si));
    }
}

/// The analysis runs over the same system the solver solves. An arc carries implicit constraints, its endpoints
/// lying on the circle of its own radius; without them the analysis ran over a different system from the solve,
/// and a conflict involving an arc fell on an innocent constraint or went unfound. Here the radius of an arc is
/// given twice, inconsistently.
#[test]
fn arc_intrinsics_are_part_of_the_analyzed_system() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    let (c, a) = match p.sketches[si].entities[0].kind {
        qymcad_core::model::EntityKind::Arc { center, a, .. } => (center, a),
        _ => unreachable!("an arc"),
    };
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: c });
    // the radius of an arc is the distance from centre to endpoint; it is given twice, incompatibly
    s.constraints.push(Constraint::Distance { a: c, b: a, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    s.constraints.push(Constraint::Distance { a: c, b: a, d: 25.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);

    let bad = p.sketch_conflicts(si);
    assert!(bad.contains(&1) && bad.contains(&2), "both conflicting radius dimensions of the arc are named: {bad:?}");
    assert!(bad.iter().all(|&i| i < p.sketches[si].constraints.len()), "the indices of intrinsic constraints do not leak outwards: {bad:?}");
}

/// A conflicting dimension can be made driven, which clears the conflict: the ordinary way out, instead of
/// deleting it.
#[test]
fn making_a_conflicting_dimension_driven_resolves_the_conflict() {
    let mut p = Project::default();
    p.new_document();
    let si = sketch(&mut p);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Fixed { p: a });
    s.constraints.push(Constraint::Horizontal { a, b });
    s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    s.constraints.push(Constraint::Distance { a, b, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    assert!(!p.sketch_conflicts(si).is_empty(), "there is a conflict before the edit");

    assert!(p.auto_driven(si, 3), "the conflicting dimension becomes driven");
    p.solve_sketch(si);
    assert!(p.sketch_conflicts(si).is_empty(), "after that there are no conflicts: {:?}", p.sketch_conflicts(si));
    // and the geometry settled on the remaining driving dimension
    let (pa, pb) = (p.sketches[si].points[0], p.sketches[si].points[1]);
    let len = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
    assert!((len - 30.0).abs() < 1e-6, "the length follows the driving dimension at 30 rather than a compromise: {len}");
}
