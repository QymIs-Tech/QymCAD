//! A SYMMETRY IS MET BY THE PAIR, NOT BY SHRINKING ITS AXIS: an upright line held on the Y axis by one end, and the two
//! ends of another line, at (10, 5) and (30, 25), made symmetric about it. The pair is mirrored about x = 0 and the axis
//! keeps its 40 mm. Reported behaviour: the symmetry was refused as shrinking a line to a point - the solve had brought
//! the axis down to a point at (0, 0), where the residuals, times the length of the axis, were nought.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Id, Project};

#[test]
fn a_symmetry_about_a_line_held_on_an_axis_mirrors_the_pair_and_keeps_the_line() {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("symmetry", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).expect("the sketch is there");
    let pair = p.add_line_entity(si, 10.0, 5.0, 30.0, 25.0, Purpose::Real);
    let axis = p.add_line_entity(si, 0.0, -20.0, 0.0, 20.0, Purpose::Real);
    let ends = |p: &Project, id: Id| match p.sketches[si].entities.iter().find(|e| e.id == id).map(|e| e.kind) {
        Some(EntityKind::Line { a, b }) => (a, b),
        other => panic!("the line {id} is {other:?}"),
    };
    let ((a, b), (la, lb)) = (ends(&p, pair), ends(&p, axis));
    let (o, guide) = p.ensure_axis(si, 1);
    p.sketches[si].constraints.push(Constraint::Vertical { a: la, b: lb });
    p.sketches[si].constraints.push(Constraint::PointOnLine { p: la, a: o, b: guide });
    p.solve_sketch(si);
    p.sketches[si].constraints.push(Constraint::Symmetric { a, b, la, lb });
    let resid = p.solve_sketch(si);
    let at = |id: Id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point is there");
    let ((ax, ay), (bx, by), (lax, lay), (lbx, lby)) = (at(a), at(b), at(la), at(lb));
    let length = (lbx - lax).hypot(lby - lay);
    assert!(resid < 1e-6, "the symmetry is not met: residual {resid}");
    assert!((length - 40.0).abs() < 1e-3, "the axis is {length} mm long after the solve, not the 40 it was drawn");
    assert!((ax + bx).abs() < 1e-4 && (ay - by).abs() < 1e-4, "the pair ({ax}, {ay}) and ({bx}, {by}) is not mirrored about x = 0");
}
