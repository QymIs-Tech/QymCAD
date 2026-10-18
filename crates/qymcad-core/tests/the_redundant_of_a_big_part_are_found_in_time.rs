//! THE REDUNDANT CONSTRAINTS OF A BIG PART ARE FOUND IN TIME: a ladder of 500 rungs, each held by its ends on two rails,
//! is one part of 1 000 points on lines; one of them laid twice is the one redundancy. The count names the two and
//! nothing else, in a fraction of a second.
//!
//! Reported behaviour: a sketch of 973 lines tied by 1 062 points on lines took some 20 s for every point of a line
//! drawn in it. On every change the redundant constraints were counted by taking each constraint of the part out in
//! turn and counting the part again: 9 s a count.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};
use qymcad_core::time_budget::{cost_of, over, Doer};
use std::time::Duration;

#[test]
fn the_redundant_of_a_ladder_of_500_rungs_are_found_in_time() {
    let n = 500;
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let width = 10.0 * n as f64;
    let rails = [p.add_line_entity(si, -5.0, 0.0, width, 0.0, Purpose::Real), p.add_line_entity(si, -5.0, 50.0, width, 50.0, Purpose::Real)];
    let ends = |p: &Project, line: u64| match p.sketches[si].entities.iter().find(|e| e.id == line).map(|e| e.kind) {
        Some(EntityKind::Line { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    let (low, high) = (ends(&p, rails[0]), ends(&p, rails[1]));
    let mut first_foot = None;
    for i in 0..n {
        let rung = p.add_line_entity(si, 10.0 * i as f64, 0.0, 10.0 * i as f64, 50.0, Purpose::Real);
        let (a, b) = ends(&p, rung);
        first_foot.get_or_insert(a);
        let s = &mut p.sketches[si];
        s.constraints.push(Constraint::PointOnLine { p: a, a: low.0, b: low.1 });
        s.constraints.push(Constraint::PointOnLine { p: b, a: high.0, b: high.1 });
    }
    let foot = first_foot.expect("a rung");
    let twice = p.sketches[si].constraints.len();
    p.sketches[si].constraints.push(Constraint::PointOnLine { p: foot, a: low.0, b: low.1 });
    p.solve_sketch(si);
    let counted = cost_of(Doer::Process, || p.sketch_redundant_constraints(si));
    let redundant = counted.out;
    eprintln!("the redundant of a ladder of {n} rungs: {redundant:?} in {}", counted.cost);
    let first = p.sketches[si].constraints.iter().position(|c| matches!(c, Constraint::PointOnLine { p: q, .. } if *q == foot)).expect("the first foot on its rail");
    assert_eq!(redundant, vec![first, twice], "the foot laid twice on its rail is the one redundancy");
    // measured: 12.4 s with every constraint of the part counted without it in turn (25 ms a count), 0.5 s with the two
    // of the dependency alone - most of it the sums of rows followed through the elimination
    let budget = Duration::from_millis(1500);
    assert!(counted.cost.work < budget, "{}", over(&format!("counting the redundant of a ladder of {n} rungs"), counted.cost, budget));
}
