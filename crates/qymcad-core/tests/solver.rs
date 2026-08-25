//! Tests of the sketch constraint solver.
use qymcad_core::model::{Constraint, Project};
use qymcad_core::geom::Point2;

#[test]
fn solve_horizontal_and_distance() {
    let mut p = Project::default();
    // an open polyline: A(0,0) - B(10,2) - C(3,8)
    let sid = p.add_line_sketch("L", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 2.0), Point2::new(3.0, 8.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);

    // anchor A, make AB horizontal and 20 long
    p.sketches[si].constraints = vec![
        Constraint::Fixed { p: a },
        Constraint::Horizontal { a, b },
        Constraint::Distance { a, b, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 },
    ];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-3, "the constraints are solved, residual={resid}");

    let pa = p.sketches[si].points[0];
    let pb = p.sketches[si].points[1];
    assert!((pa.x).abs() < 1e-3 && (pa.y).abs() < 1e-3, "A stays anchored at (0,0): ({},{})", pa.x, pa.y);
    assert!((pa.y - pb.y).abs() < 1e-3, "AB is horizontal");
    let len = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt();
    assert!((len - 20.0).abs() < 1e-2, "|AB| = 20, got {len}");
}

#[test]
fn solve_angle_90() {
    let mut p = Project::default();
    // three points with B at the vertex; the angle ABC is set to 90°
    let sid = p.add_line_sketch("ang", vec![Point2::new(10.0, 0.0), Point2::new(0.0, 0.0), Point2::new(8.0, 3.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let (a, b, c) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id, p.sketches[si].points[2].id);
    p.sketches[si].constraints = vec![Constraint::Fixed { p: b }, Constraint::Fixed { p: a }, Constraint::Angle { a, b, c, deg: 90.0, expr: String::new(), driven: false }];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "the angle is solved, residual={resid}");
    // the dot product BA·BC has to be about zero
    let (pa, pb, pc) = (p.sketches[si].points[0], p.sketches[si].points[1], p.sketches[si].points[2]);
    let dot = (pa.x - pb.x) * (pc.x - pb.x) + (pa.y - pb.y) * (pc.y - pb.y);
    assert!(dot.abs() < 1e-1, "BA·BC ≈ 0, i.e. perpendicular, dot={dot}");
}

#[test]
fn solve_collinear() {
    let mut p = Project::default();
    let sid = p.add_line_sketch("L", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(5.0, 4.0), Point2::new(8.0, 4.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let ids: Vec<_> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);
    p.sketches[si].constraints = vec![
        Constraint::Fixed { p: a },
        Constraint::Fixed { p: b },
        Constraint::Collinear { a, b, c, d },
    ];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-3, "residual={resid}");
    let pc = p.sketches[si].points.iter().find(|q| q.id == c).unwrap();
    let pd = p.sketches[si].points.iter().find(|q| q.id == d).unwrap();
    assert!(pc.y.abs() < 1e-2 && pd.y.abs() < 1e-2, "c and d land on the line a-b, y ≈ 0: {} {}", pc.y, pd.y);
}

#[test]
fn solve_midpoint() {
    let mut p = Project::default();
    let sid = p.add_line_sketch("L", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(3.0, 5.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let ids: Vec<_> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b, mid) = (ids[0], ids[1], ids[2]);
    p.sketches[si].constraints = vec![
        Constraint::Fixed { p: a },
        Constraint::Fixed { p: b },
        Constraint::Midpoint { p: mid, a, b },
    ];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-3, "residual={resid}");
    let pm = p.sketches[si].points.iter().find(|q| q.id == mid).unwrap();
    assert!((pm.x - 5.0).abs() < 1e-2 && pm.y.abs() < 1e-2, "the midpoint is at about (5,0): ({},{})", pm.x, pm.y);
}

#[test]
fn solve_tangent() {
    let mut p = Project::default();
    // the line a-b and the centre point c
    let sid = p.add_line_sketch("L", vec![Point2::new(8.0, 5.0), Point2::new(8.0, -5.0), Point2::new(0.0, 0.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let ids: Vec<_> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    p.sketches[si].constraints = vec![
        Constraint::Fixed { p: c },
        Constraint::Tangent { a, b, c, r: 5.0 },
    ];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "residual={resid}");
    let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
    let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
    let len = (dx * dx + dy * dy).sqrt();
    let dist = ((dx * (0.0 - pa.y) - dy * (0.0 - pa.x)) / len).abs();
    assert!((dist - 5.0).abs() < 1e-2, "distance to the line is about 5: {dist}");
}

#[test]
fn solve_symmetric() {
    let mut p = Project::default();
    // the points a and b, and the axis la-lb at x = 5
    let sid = p.add_line_sketch("S", vec![Point2::new(3.0, 2.0), Point2::new(8.0, 4.0), Point2::new(5.0, -10.0), Point2::new(5.0, 10.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let ids: Vec<_> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b, la, lb) = (ids[0], ids[1], ids[2], ids[3]);
    p.sketches[si].constraints = vec![
        Constraint::Fixed { p: a },
        Constraint::Fixed { p: la },
        Constraint::Fixed { p: lb },
        Constraint::Symmetric { a, b, la, lb },
    ];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "residual={resid}");
    let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    assert!((pb.x - 7.0).abs() < 1e-2 && (pb.y - 2.0).abs() < 1e-2, "b mirrors a about x = 5: ({},{})", pb.x, pb.y);
}

#[test]
fn dof_indication() {
    let mut p = Project::default();
    let sid = p.add_line_sketch("d", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false);
    let si = p.sketch_index(sid).unwrap();
    assert_eq!(p.sketch_dof(si), (4, 0), "two points without constraints give 4 degrees of freedom and no redundancy");
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    p.sketches[si].constraints = vec![Constraint::Fixed { p: a }, Constraint::Fixed { p: b }];
    assert_eq!(p.sketch_dof(si), (0, 0), "both points anchored give 0 degrees of freedom and no redundancy");
    // the horizontal already follows from the anchors, so it is redundant: the Jacobian rank does not grow
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    let (dof, redun) = p.sketch_dof(si);
    assert_eq!(dof, 0, "still 0 degrees of freedom");
    assert!(redun >= 1, "the horizontal is redundant: redun={redun}");
}

#[test]
fn free_points_marks_movable() {
    let mut p = Project::default();
    let sid = p.add_line_sketch("f", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let (a, _b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    // without constraints both points are free
    assert_eq!(p.sketch_free_points(si), vec![true, true]);
    // anchor the first one: it becomes fully constrained while the second stays free
    p.sketches[si].constraints = vec![Constraint::Fixed { p: a }];
    let free = p.sketch_free_points(si);
    assert!(!free[0], "an anchored point is fully constrained");
    assert!(free[1], "the second point is still free");
}

#[test]
fn closed_square_stays_grounded_under_constraints() {
    // A closed square with shared corners under horizontal, vertical and equal constraints must neither fly
    // apart nor rotate: the soft regularisation holds the free degrees of freedom in place.
    let mut p = Project::default();
    let sid = p.add_line_sketch("sq", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.3), Point2::new(9.7, 10.0), Point2::new(0.2, 9.8)], true);
    let si = p.sketch_index(sid).unwrap();
    let pid: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let before: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect();
    // bottom edge horizontal, left edge vertical, sides equal
    p.sketches[si].constraints.push(Constraint::Horizontal { a: pid[0], b: pid[1] });
    p.sketches[si].constraints.push(Constraint::Vertical { a: pid[0], b: pid[3] });
    p.sketches[si].constraints.push(Constraint::Equal { a: pid[0], b: pid[1], c: pid[0], d: pid[3] });
    p.solve_sketch(si);
    // no point has travelled far from where it started, so nothing exploded or rotated away
    for (i, q) in p.sketches[si].points.iter().enumerate() {
        let (bx, by) = before[i];
        let d = ((q.x - bx).powi(2) + (q.y - by).powi(2)).sqrt();
        assert!(d < 2.0, "point {i} moved by {d:.2} (from {bx:.1},{by:.1} to {:.1},{:.1})", q.x, q.y);
    }
    // the first corner stays near the origin, grounded by the regularisation
    assert!(p.sketches[si].points[0].x.abs() < 1.0 && p.sketches[si].points[0].y.abs() < 1.0);
}

#[test]
fn ensure_origin_is_fixed_at_zero() {
    let mut p = Project::default();
    let si = p.new_sketch("o");
    let o = p.ensure_origin(si);
    assert_ne!(o, 0, "the origin is materialised");
    // a repeated call returns the same point instead of adding another
    assert_eq!(p.ensure_origin(si), o, "the origin is reused");
    let op = p.sketches[si].points.iter().find(|q| q.id == o).unwrap();
    assert!(op.x.abs() < 1e-9 && op.y.abs() < 1e-9, "the origin sits at (0,0)");
    // the origin belongs to no entity, so it never reaches the profile
    assert!(p.sketches[si].entities.is_empty());
}

#[test]
fn dimension_from_origin_to_midpoint() {
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("d");
    // the line (0,4)-(10,4), whose midpoint has to land at (5,4)
    p.add_line_entity(si, 0.0, 4.0, 10.0, 4.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    // materialise the midpoint, the way `materialize_ref` does it in the interface
    let mid = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: mid, x: 5.0, y: 4.0 });
    p.sketches[si].constraints.push(Constraint::Midpoint { p: mid, a, b });
    // a dimension of 8 from the origin to the midpoint moves the line up until the midpoint is 8 away
    let o = p.ensure_origin(si);
    p.sketches[si].constraints.push(Constraint::Distance { a: o, b: mid, d: 8.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "solved, residual={resid}");
    let pm = p.sketches[si].points.iter().find(|q| q.id == mid).unwrap();
    let dist = (pm.x * pm.x + pm.y * pm.y).sqrt();
    assert!((dist - 8.0).abs() < 1e-2, "the midpoint sits 8 away from the origin: {dist}");
    // and it really is the middle of the segment
    let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
    let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    assert!((pm.x - (pa.x + pb.x) / 2.0).abs() < 1e-2 && (pm.y - (pa.y + pb.y) / 2.0).abs() < 1e-2, "the point is still the midpoint");
}

#[test]
fn parametric_dimension_follows_parameter() {
    use qymcad_core::model::{Constraint, Param};
    let mut p = Project::default();
    let si = p.new_sketch("p");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    // the dimension is the expression w/2, and with the parameter w = 50 the length has to be 25
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 0.0, off: 0.0, expr: "w/2".into(), driven: false, axis: 0 });
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 0.0 });
    p.solve_sketch(si);
    let len = {
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
        ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt()
    };
    assert!((len - 25.0).abs() < 1e-2, "length = w/2 = 25, got {len}");
    // changing the parameter recomputes the dimension
    p.parameters[0].expr = "80".into();
    p.solve_sketch(si);
    let len2 = {
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
        ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt()
    };
    assert!((len2 - 40.0).abs() < 1e-2, "with w = 80 the length is 40, got {len2}");
    assert!((p.parameters[0].value - 80.0).abs() < 1e-9, "the value of the parameter is updated");
}

#[test]
fn parameters_resolve_dependencies() {
    use qymcad_core::model::Param;
    let mut p = Project::default();
    p.parameters.push(Param { name: "d".into(), expr: "w/2 + 5".into(), value: 0.0 });
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 0.0 });
    let errs = p.eval_parameters();
    assert!(errs.is_empty(), "no errors: {errs:?}");
    let d = p.parameters.iter().find(|x| x.name == "d").unwrap().value;
    assert!((d - 30.0).abs() < 1e-9, "d = w/2 + 5 = 30, got {d}");
}

#[test]
fn redundant_dimension_becomes_driven() {
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("r");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    // fully constrain the line by anchoring both endpoints
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Fixed { p: b });
    // adding a dimension now is redundant, since the geometry is already fully constrained
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    let ci = p.sketches[si].constraints.len() - 1;
    assert!(p.dim_redundant(si, ci), "a dimension between two anchored points is redundant");
    assert!(p.auto_driven(si, ci), "it becomes a driven dimension");
    assert!(p.sketches[si].constraints[ci].is_driven());
    // a driven dimension does not affect the degrees of freedom and adds no redundancy
    let (dof, redun) = p.sketch_dof(si);
    assert_eq!((dof, redun), (0, 0), "once driven: fully constrained, without redundancy");
}

#[test]
fn driven_dimension_measures_geometry() {
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("m");
    p.add_line_entity(si, 0.0, 0.0, 7.0, 0.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    // a driven dimension with a deliberately wrong d has to report the actual value of 7
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 999.0, off: 0.0, expr: String::new(), driven: true, axis: 0 });
    p.solve_sketch(si);
    if let Constraint::Distance { d, .. } = &p.sketches[si].constraints[0] {
        assert!((d - 7.0).abs() < 1e-6, "the driven dimension measured a length of 7, got {d}");
    } else {
        panic!("expected a Distance constraint");
    }
}

#[test]
fn point_on_line_pulls_point_onto_line() {
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("pol");
    // a slanted line (0,0)-(10,10) and a point off to the side at (8,2)
    p.add_line_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    let cp = p.alloc_id();
    p.sketches[si].points.push(qymcad_core::model::SketchPoint { id: cp, x: 8.0, y: 2.0 });
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Fixed { p: b });
    p.sketches[si].constraints.push(Constraint::PointOnLine { p: cp, a, b });
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "solved, residual={resid}");
    // the point has to land on the line y = x, so |x − y| ≈ 0
    let q = p.sketches[si].points.iter().find(|q| q.id == cp).unwrap();
    assert!((q.x - q.y).abs() < 1e-2, "the point lies on y = x: ({},{})", q.x, q.y);
}

#[test]
fn delete_points_removes_incident_geometry() {
    let mut p = Project::default();
    let si = p.new_sketch("d");
    // two segments sharing a corner at (10,0)
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.merge_close_points(si, 0.1); // the shared corner
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 10.0).abs() < 0.1 && q.y.abs() < 0.1).unwrap().id;
    let before = p.sketches[si].entities.len();
    p.delete_points(si, &[corner]);
    // both lines that shared the corner are removed
    assert!(p.sketches[si].entities.len() < before, "the incident lines are removed");
    assert!(!p.sketches[si].points.iter().any(|q| q.id == corner), "the point is removed");
}

#[test]
fn point_on_axis_via_ensure_axis() {
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("ax");
    // a point off to the side of the X axis
    let q = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: q, x: 5.0, y: 7.0 });
    let (o, d) = p.ensure_axis(si, 0); // the X axis: (0,0)-(1,0)
    p.sketches[si].constraints.push(Constraint::PointOnLine { p: q, a: o, b: d });
    p.solve_sketch(si);
    let qp = p.sketches[si].points.iter().find(|x| x.id == q).unwrap();
    assert!(qp.y.abs() < 1e-2, "the point lands on the X axis, y ≈ 0: y={}", qp.y);
    // calling `ensure_axis` again returns the same points
    let (o2, d2) = p.ensure_axis(si, 0);
    assert_eq!((o, d), (o2, d2), "the axis is reused");
}

#[test]
fn add_constraint_if_independent_skips_redundant() {
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("i");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    // an independent constraint: a horizontal on a free segment is accepted
    assert!(p.add_constraint_if_independent(si, Constraint::Horizontal { a, b }), "the horizontal is independent and is added");
    let n = p.sketches[si].constraints.len();
    // a second horizontal is redundant and is rejected
    assert!(!p.add_constraint_if_independent(si, Constraint::Horizontal { a, b }), "the repeated horizontal is redundant and is rejected");
    assert_eq!(p.sketches[si].constraints.len(), n, "the constraint count did not grow");
}

#[test]
fn distance_point_to_line() {
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("dpl");
    // a horizontal line along y = 0 and a point above it
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let ids: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let (a, b) = (ids[0], ids[1]);
    let q = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: q, x: 5.0, y: 3.0 });
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Fixed { p: b });
    // a point-to-line distance of 8 moves the point to y = 8
    p.sketches[si].constraints.push(Constraint::DistancePL { p: q, a, b, d: 8.0, off: 0.0, expr: String::new(), driven: false });
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-2, "solved, residual={resid}");
    let qp = p.sketches[si].points.iter().find(|x| x.id == q).unwrap();
    assert!((qp.y.abs() - 8.0).abs() < 1e-2, "the point is 8 away from the line: y={}", qp.y);
}

#[test]
fn horizontal_vertical_distance_variants() {
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("hv");
    // two points on a diagonal
    let a = p.alloc_id();
    let b = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: a, x: 0.0, y: 0.0 });
    p.sketches[si].points.push(SketchPoint { id: b, x: 6.0, y: 8.0 });
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    // a horizontal dimension, axis = 1: |Δx| = 20 moves b.x to 20 while b.y stays free
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 1 });
    p.solve_sketch(si);
    let bp = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    assert!((bp.x.abs() - 20.0).abs() < 1e-2, "horizontal: |Δx| = 20, got {}", bp.x);
    // switch to a vertical one, axis = 2: |Δy| = 15
    p.sketches[si].constraints.pop();
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 15.0, off: 0.0, expr: String::new(), driven: false, axis: 2 });
    p.solve_sketch(si);
    let bp = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    assert!((bp.y.abs() - 15.0).abs() < 1e-2, "vertical: |Δy| = 15, got {}", bp.y);
}

#[test]
fn diameter_dimension_drives_circle() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("circ");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real); // a circle of r = 5
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).unwrap();
    // a diameter dimension of 30 makes r become 15
    let ci = p.ensure_diameter(si, center, true).unwrap();
    if let Constraint::Diameter { d, .. } = &mut p.sketches[si].constraints[ci] { *d = 30.0; }
    p.solve_sketch(si);
    let r = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { r, .. } => Some(r), _ => None }).unwrap();
    assert!((r - 15.0).abs() < 1e-3, "Ø30 gives r = 15, got {r}");
    // a driven diameter reports the measurement: r = 15 means Ø30
    if let Constraint::Diameter { driven, d, .. } = &mut p.sketches[si].constraints[ci] { *driven = true; *d = 0.0; }
    p.solve_sketch(si);
    if let Constraint::Diameter { d, .. } = &p.sketches[si].constraints[ci] {
        assert!((d - 30.0).abs() < 1e-2, "the driven diameter measured 30, got {d}");
    }
}

#[test]
fn angle_between_two_lines() {
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("al");
    // line 1 runs along X from (0,0) to (10,0); line 2 is slanted, from (0,0) to (10,2)
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, 0.0, 10.0, 2.0, qymcad_core::feature::Purpose::Real);
    // line1 = ids[0]->ids[1], line2 = ids[2]->ids[3]; after deduplication the shared start may merge
    p.merge_close_points(si, 0.01);
    // find the endpoints by coordinates
    let at = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x-x).abs()<0.2 && (q.y-y).abs()<0.2).unwrap().id;
    let (a, b) = (at(&p, 0.0, 0.0), at(&p, 10.0, 0.0));
    let (c, d) = (at(&p, 0.0, 0.0), at(&p, 10.0, 2.0));
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Fixed { p: b });
    p.sketches[si].constraints.push(Constraint::Fixed { p: c });
    // an angle of 45° between the lines rotates the endpoint d
    p.sketches[si].constraints.push(Constraint::AngleLines { a, b, c, d, deg: 45.0, expr: String::new(), driven: false });
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-1, "solved, residual={resid}");
    let pd = p.sketches[si].points.iter().find(|q| q.id == d).unwrap();
    let ang = (pd.y - 0.0).atan2(pd.x - 0.0).to_degrees();
    assert!((ang.abs() - 45.0).abs() < 1.0, "the second line sits at about 45°, got {ang}");
}

#[test]
fn angle_change_preserves_line_length() {
    // An angular dimension must not change the length of a line. B is the anchored vertex, A is anchored too,
    // and C is free with nothing fixing its distance to B. Setting ABC to 90° has to rotate the side BC about B
    // while its length is preserved.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let sid = p.add_line_sketch("ang_len", vec![Point2::new(10.0, 0.0), Point2::new(0.0, 0.0), Point2::new(8.0, 3.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let (a, b, c) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id, p.sketches[si].points[2].id);
    let bc = |p: &Project| {
        let (pb, pc) = (p.sketches[si].points[1], p.sketches[si].points[2]);
        ((pc.x - pb.x).powi(2) + (pc.y - pb.y).powi(2)).sqrt()
    };
    let len0 = bc(&p);
    p.sketches[si].constraints = vec![Constraint::Fixed { p: a }, Constraint::Fixed { p: b }, Constraint::Angle { a, b, c, deg: 90.0, expr: String::new(), driven: false }];
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-3, "the 90° angle is solved, residual={resid}");
    let (pb, pc) = (p.sketches[si].points[1], p.sketches[si].points[2]);
    let dot = (10.0 - pb.x) * (pc.x - pb.x) + (0.0 - pb.y) * (pc.y - pb.y);
    assert!(dot.abs() < 1e-1, "the 90° angle is reached, BA·BC ≈ 0, dot={dot}");
    assert!((bc(&p) - len0).abs() < 0.05, "the length of BC is preserved and not broken by the angle: was {len0:.3}, got {:.3}", bc(&p));
}

#[test]
fn tangent_solves_circle_radius() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("t");
    // a circle centred at (0,0) with an arbitrary r = 5, and a horizontal line along y = 8
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).unwrap();
    p.add_line_entity(si, -10.0, 8.0, 10.0, 8.0, qymcad_core::feature::Purpose::Real);
    let la = p.sketch_point_at(si, -10.0, 8.0, 1e-6);
    let lb = p.sketch_point_at(si, 10.0, 8.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    p.sketches[si].constraints.push(Constraint::Fixed { p: la });
    p.sketches[si].constraints.push(Constraint::Fixed { p: lb });
    // tangency forces the radius to become the distance from the centre to the line, i.e. 8
    p.sketches[si].constraints.push(Constraint::Tangent { a: la, b: lb, c: center, r: 5.0 });
    p.solve_sketch(si);
    let r = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { r, .. } => Some(r), _ => None }).unwrap();
    assert!((r - 8.0).abs() < 1e-2, "tangency drove the radius to 8, got {r}");
}

#[test]
fn equal_radius_links_circles() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("eq");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 30.0, 0.0, 12.0, qymcad_core::feature::Purpose::Real);
    let centers: Vec<u64> = p.sketches[si].entities.iter().filter_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).collect();
    let (c1, c2) = (centers[0], centers[1]);
    // fix the radius of the first circle at 5 and tie the radii together
    p.sketches[si].constraints.push(Constraint::Diameter { c: c1, d: 5.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.sketches[si].constraints.push(Constraint::EqualRadius { c1, c2 });
    p.solve_sketch(si);
    let radii: Vec<f64> = p.sketches[si].entities.iter().filter_map(|e| match e.kind { EntityKind::Circle { center, r } => Some((center, r)), _ => None }).map(|(_, r)| r).collect();
    assert!((radii[0] - radii[1]).abs() < 1e-2, "the radii are equal: {radii:?}");
    assert!((radii[0] - 5.0).abs() < 1e-2, "both are 5: {radii:?}");
}

#[test]
fn circle_radius_counts_in_dof() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("d");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).unwrap();
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    // the centre is anchored, removing two degrees of freedom, and the free radius leaves one
    let (dof, _) = p.sketch_dof(si);
    assert_eq!(dof, 1, "a free radius is one degree of freedom");
    // adding a diameter dimension brings it to zero
    p.sketches[si].constraints.push(Constraint::Diameter { c: center, d: 10.0, off: 0.0, expr: String::new(), driven: false, diam: true });
    let (dof2, _) = p.sketch_dof(si);
    assert_eq!(dof2, 0, "with the radius dimensioned the sketch is fully constrained");
}

#[test]
fn arc_endpoints_stay_on_one_circle() {
    // An arc is a real entity: its endpoints are held intrinsically on a circle of radius R. Here the
    // endpoints start at different radii, 10 and 5; anchoring the centre and the endpoint a at r = 10 has to
    // pull the endpoint b out to radius 10 rather than leave it at 5, keeping its angle at about 90°.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("arc");
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 5.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    let (center, a, b) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Arc { center, a, b, .. } => Some((center, a, b)), _ => None }).unwrap();
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.solve_sketch(si);
    let pos = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap();
    let (cx, cy) = pos(center);
    let (ax, ay) = pos(a);
    let (bx, by) = pos(b);
    let ra = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
    let rb = ((bx - cx).powi(2) + (by - cy).powi(2)).sqrt();
    assert!((ra - rb).abs() < 1e-2, "both endpoints share one radius: ra={ra}, rb={rb}");
    assert!((rb - 10.0).abs() < 1e-2, "the endpoint b moved out to radius 10, got {rb}");
}

#[test]
fn free_arc_has_five_dof() {
    // A free arc: centre (2) + a (2) + b (2) + radius (1) minus the two intrinsic constraints leaves five
    // degrees of freedom and no redundancy.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("arc");
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    assert!(matches!(p.sketches[si].entities[0].kind, EntityKind::Arc { .. }));
    assert_eq!(p.sketch_dof(si), (5, 0), "a free arc has five degrees of freedom");
}

#[test]
fn tangent_line_to_arc_drives_arc_radius() {
    // Tangency of a line to an arc uses the live radius of the arc, exactly as for a circle, so the arc
    // adjusts its radius.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("ta");
    p.add_arc_entity(si, 0.0, 0.0, 5.0, 0.0, 0.0, 5.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real); // centre (0,0), r = 5
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Arc { center, .. } => Some(center), _ => None }).unwrap();
    p.add_line_entity(si, -10.0, 8.0, 10.0, 8.0, qymcad_core::feature::Purpose::Real);
    let la = p.sketch_point_at(si, -10.0, 8.0, 1e-6);
    let lb = p.sketch_point_at(si, 10.0, 8.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    p.sketches[si].constraints.push(Constraint::Fixed { p: la });
    p.sketches[si].constraints.push(Constraint::Fixed { p: lb });
    p.sketches[si].constraints.push(Constraint::Tangent { a: la, b: lb, c: center, r: 5.0 });
    p.solve_sketch(si);
    let pos = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap();
    let aend = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Arc { a, .. } => Some(a), _ => None }).unwrap();
    let (cx, cy) = pos(center);
    let (ax, ay) = pos(aend);
    let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
    assert!((r - 8.0).abs() < 1e-2, "tangency to the arc drove the radius to 8, got {r}");
}

#[test]
fn point_on_circle_constraint() {
    use qymcad_core::model::{Constraint, EntityKind, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("poc");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let center = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).unwrap();
    let pid = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: pid, x: 20.0, y: 0.0 }); // outside the circle
    p.sketches[si].constraints.push(Constraint::Fixed { p: center });
    p.sketches[si].constraints.push(Constraint::Diameter { c: center, d: 10.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.sketches[si].constraints.push(Constraint::PointOnCircle { p: pid, c: center });
    p.solve_sketch(si);
    let pt = p.sketches[si].points.iter().find(|q| q.id == pid).unwrap();
    let r = (pt.x.powi(2) + pt.y.powi(2)).sqrt();
    assert!((r - 10.0).abs() < 1e-2, "the point landed on the circle of r = 10, got {r}");
}

#[test]
fn concentric_aligns_centers() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("conc");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 30.0, 5.0, 4.0, qymcad_core::feature::Purpose::Real);
    let cs: Vec<u64> = p.sketches[si].entities.iter().filter_map(|e| match e.kind { EntityKind::Circle { center, .. } => Some(center), _ => None }).collect();
    p.sketches[si].constraints.push(Constraint::Fixed { p: cs[0] });
    p.sketches[si].constraints.push(Constraint::Concentric { c1: cs[0], c2: cs[1] });
    p.solve_sketch(si);
    let pos = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap();
    let (ax, ay) = pos(cs[0]);
    let (bx, by) = pos(cs[1]);
    assert!((ax - bx).abs() < 1e-2 && (ay - by).abs() < 1e-2, "the centres coincide: ({ax},{ay}) vs ({bx},{by})");
}

#[test]
fn parametric_polygon_stays_regular() {
    // A regular polygon is a parametric group: vertices on a circumscribed circle plus equal sides. After a
    // perturbation and a solve it stays regular — all sides equal, all vertices at one radius — and the
    // circumscribed circle carries the dimension.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("hex");
    let (center, sides) = p.add_polygon_param(si, 0.0, 0.0, 10.0, 0.0, 6, qymcad_core::feature::Purpose::Real);
    assert_eq!(sides.len(), 6, "six sides");
    // move the centre, which is a free degree of freedom, and solve: the polygon has to stay regular
    if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.id == center) {
        q.x += 7.0; q.y -= 3.0;
    }
    p.solve_sketch(si);
    // the lengths of all sides
    let pt = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    let mut lens = Vec::new();
    for &e in &sides {
        if let EntityKind::Line { a, b } = p.sketches[si].entities.iter().find(|x| x.id == e).unwrap().kind {
            let ((ax, ay), (bx, by)) = (pt(a), pt(b));
            lens.push(((ax - bx).powi(2) + (ay - by).powi(2)).sqrt());
        }
    }
    let l0 = lens[0];
    for l in &lens {
        assert!((l - l0).abs() < 0.05, "all sides are equal: {lens:?}");
    }
    // every vertex sits at radius R from the centre, held by the circumscribed circle
    let (cx, cy) = pt(center);
    let r = p.sketches[si].entities.iter().find_map(|e| match e.kind { EntityKind::Circle { center: cc, r } if cc == center => Some(r), _ => None }).unwrap();
    for &e in &sides {
        if let EntityKind::Line { a, .. } = p.sketches[si].entities.iter().find(|x| x.id == e).unwrap().kind {
            let (ax, ay) = pt(a);
            let dr = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
            assert!((dr - r).abs() < 0.05, "the vertex is on the circumscribed circle R={r}, dr={dr}");
        }
    }
    assert!((r - 10.0).abs() < 0.1, "the radius is held by the dimension, R={r}");
}

#[test]
fn parametric_ellipse_axes_perpendicular_and_sized() {
    // An ellipse is a real entity: a centre plus the endpoints of its semi-axes, which an implicit constraint
    // keeps perpendicular. A free ellipse has five degrees of freedom, and editing the semi-axes preserves both
    // the perpendicularity and the rotation.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("ell");
    // an ellipse rotated by 30°, with semi-axes of 10 and 4
    let center = p.add_ellipse_entity(si, 0.0, 0.0, 10.0, 4.0, 30f64.to_radians(), qymcad_core::feature::Purpose::Real);
    // a free ellipse has five degrees of freedom: centre (2), major, minor and rotation
    assert_eq!(p.sketch_dof(si), (5, 0), "a free ellipse has five degrees of freedom");
    let (ma, mi) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Ellipse { c, ma, mi } if c == center => Some((ma, mi)), _ => None }).unwrap();
    // the axes have to stay perpendicular after the solve
    p.solve_sketch(si);
    let pt = |p: &Project, id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    let (cx, cy) = pt(&p, center);
    let (max, may) = pt(&p, ma);
    let (mix, miy) = pt(&p, mi);
    let dot = (max - cx) * (mix - cx) + (may - cy) * (miy - cy);
    assert!(dot.abs() < 1e-2, "the semi-axes are perpendicular: dot={dot}");
    let rot_before = (may - cy).atan2(max - cx);
    // the semi-axes are editable and the rotation survives
    assert!(p.set_ellipse_axes(si, center, 20.0, 8.0));
    let (a, b) = p.ellipse_axes(si, center).unwrap();
    assert!((a - 20.0).abs() < 0.05 && (b - 8.0).abs() < 0.05, "the semi-axes became 20 and 8: {a},{b}");
    let (max2, may2) = pt(&p, ma);
    let rot_after = (may2 - cy).atan2(max2 - cx);
    assert!((rot_before - rot_after).abs() < 0.02, "the rotation of the major axis is preserved");
}

#[test]
fn point_on_line_distance_keeps_side() {
    // A signed `DistancePL`: a solve must not mirror the point onto the other side of the line. The line runs
    // along X at y = 0 and the point starts above it at y = +5 with a dimension of 5. Solving from a perturbed
    // start has to leave the point above, at y > 0, rather than drop it to y = -5.
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("pl");
    p.add_line_entity(si, -10.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real); // the line y = 0
    let la = p.sketch_point_at(si, -10.0, 0.0, 1e-6);
    let lb = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    let pid = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: pid, x: 3.0, y: 5.0 }); // above the line
    p.sketches[si].constraints.push(Constraint::Fixed { p: la });
    p.sketches[si].constraints.push(Constraint::Fixed { p: lb });
    // d is signed: for a point above the line the perpendicular is +5, which depends on the direction la->lb
    let dx = 20.0_f64; // (lb-la).x
    let perp = (dx * (5.0 - 0.0)) / 20.0; // = +5
    p.sketches[si].constraints.push(Constraint::DistancePL { p: pid, a: la, b: lb, d: perp, off: 0.0, expr: String::new(), driven: false });
    // perturb the point downwards and solve: it has to return to its own side, y > 0, not to -5
    if let Some(q) = p.sketches[si].points.iter_mut().find(|q| q.id == pid) { q.y = 1.0; }
    p.solve_sketch(si);
    let q = p.sketches[si].points.iter().find(|q| q.id == pid).unwrap();
    assert!(q.y > 0.0, "the point stayed on its own side, above the line, y={}", q.y);
    assert!((q.y.abs() - 5.0).abs() < 0.1, "the distance to the line is 5, y={}", q.y);
}

#[test]
fn conflict_vs_redundant_distinguished() {
    // Two different length dimensions on one edge are a conflict: the solver averages them and both end up
    // wrong. An extra dimension that agrees with the first is not a conflict — redundant, but consistent.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("c");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let a = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let b = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    // an extra dimension that agrees, also 10, is not a conflict
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    assert!(p.sketch_conflicts(si).is_empty(), "an agreeing extra dimension is not a conflict");
    // a contradictory dimension, 20 on the same edge, is a conflict
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    let conf = p.sketch_conflicts(si);
    assert!(!conf.is_empty(), "contradictory dimensions, 10 and 20, produce a conflict: {conf:?}");
}

#[test]
fn arc_length_dimension_drives_arc() {
    // An arc length dimension. A quarter arc of r = 10 is about 15.7 long. Anchoring the centre and the
    // endpoint a, which sets R = 10 and the start angle, and asking for a length of 10 gives a swept angle of
    // θ = L/R = 1 rad, and the endpoint b moves accordingly.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("al");
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real); // centre (0,0), a (10,0), b (0,10), ccw
    let (c, a, b) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Arc { center, a, b, .. } => Some((center, a, b)), _ => None }).unwrap();
    p.sketches[si].constraints.push(Constraint::Fixed { p: c });
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    let arc_eid = p.sketches[si].entities[0].id;
    let ci = p.ensure_arc_length(si, arc_eid).unwrap();
    if let Constraint::ArcLength { len, .. } = &mut p.sketches[si].constraints[ci] { *len = 10.0; }
    p.solve_sketch(si);
    let pt = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    let (cx, cy) = pt(c); let (bx, by) = pt(b);
    let rad = ((bx - cx).powi(2) + (by - cy).powi(2)).sqrt();
    let theta = (by - cy).atan2(bx - cx).rem_euclid(std::f64::consts::TAU); // a starts at angle 0
    assert!((rad - 10.0).abs() < 0.1, "the endpoint stayed at radius 10: {rad}");
    assert!((rad * theta - 10.0).abs() < 0.2, "the arc length is 10, with θ ≈ 1: L={}", rad * theta);
}

#[test]
fn delete_midpoint_cleans_orphan_point() {
    // Deleting a `Midpoint` constraint removes the orphaned midpoint with it, leaving no debris behind.
    use qymcad_core::model::{Constraint, SketchPoint};
    let mut p = Project::default();
    let si = p.new_sketch("m");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let a = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let b = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    let mid = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: mid, x: 5.0, y: 0.0 });
    let mci = p.sketches[si].constraints.len();
    p.sketches[si].constraints.push(Constraint::Midpoint { p: mid, a, b });
    let before = p.sketches[si].points.len();
    assert!(p.delete_sketch_constraint(si, mci), "the constraint is deleted");
    assert_eq!(p.sketches[si].points.len(), before - 1, "the midpoint is removed as an orphan");
    assert!(!p.sketches[si].points.iter().any(|q| q.id == mid), "it is the midpoint that was removed");
}

#[test]
fn redundant_constraints_are_flagged() {
    // A redundant constraint is flagged individually, by index, while a consistent fully constrained sketch
    // is not.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("r");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let a = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let b = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Fixed { p: b });
    // without redundancy the list is empty
    assert!(p.sketch_redundant_constraints(si).is_empty(), "no redundancy yet");
    // an extra horizontal, already implied by the two anchors, is redundant
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    let red = p.sketch_redundant_constraints(si);
    assert!(!red.is_empty(), "the redundant constraint is flagged: {red:?}");
}

#[test]
fn circle_tangent_external_pulls_to_sum_of_radii() {
    // External circle-to-circle tangency pulls the distance between the centres to r1 + r2.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("ct");
    p.add_circle_entity(si, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 10.0, 0.0, 2.0, qymcad_core::feature::Purpose::Real);
    let c1 = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let c2 = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: c1 });
    p.sketches[si].constraints.push(Constraint::Diameter { c: c1, d: 3.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.sketches[si].constraints.push(Constraint::Diameter { c: c2, d: 2.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.sketches[si].constraints.push(Constraint::CircleTangent { c1, c2, external: true });
    p.solve_sketch(si);
    let g = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    let (a, b) = (g(c1), g(c2));
    let d = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    assert!((d - 5.0).abs() < 1e-2, "external tangency: the centre distance is R1 + R2 = 5, got {d}");
}

#[test]
fn tangent_arc_to_line_holds_under_radius_change() {
    // An arc tangent to a line at a shared endpoint is parametric: changing the radius of the arc keeps the
    // tangency, with the centre staying above the shared point at distance R from the line.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("ta");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let la = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let lb = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    // an arc tangent at the line endpoint (10,0): with t = (1,0) and the far end at (10,4) the centre is
    // (10,2) and R = 2
    p.add_arc_entity(si, 10.0, 2.0, 10.0, 0.0, 10.0, 4.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    let cen = p.sketch_point_at(si, 10.0, 2.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Tangent { a: la, b: lb, c: cen, r: 2.0 });
    p.sketches[si].constraints.push(Constraint::Fixed { p: la });
    p.sketches[si].constraints.push(Constraint::Fixed { p: lb });
    // change the radius of the arc to 3
    p.sketches[si].constraints.push(Constraint::Diameter { c: cen, d: 3.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.solve_sketch(si);
    let c = p.sketches[si].points.iter().find(|q| q.id == cen).unwrap();
    // the centre stays above the shared point at cx = 10, at distance R = 3 from the line y = 0
    assert!((c.x - 10.0).abs() < 2e-2, "the centre is above the tangency point, cx ≈ 10: {}", c.x);
    assert!((c.y.abs() - 3.0).abs() < 2e-2, "the centre-to-line distance is R = 3, so the tangency holds: {}", c.y);
}

#[test]
fn edge_distance_between_circle_edges() {
    // An edge distance dimension measures the gap between the rims of two circles: centres − r1 − r2 = d.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("ed");
    p.add_circle_entity(si, 0.0, 0.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 20.0, 0.0, 2.0, qymcad_core::feature::Purpose::Real);
    let c1 = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    let c2 = p.sketch_point_at(si, 20.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: c1 });
    p.sketches[si].constraints.push(Constraint::Diameter { c: c1, d: 3.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.sketches[si].constraints.push(Constraint::Diameter { c: c2, d: 2.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    // a gap of 2 between the nearest rims means a centre distance of 3 + 2 + 2 = 7
    p.sketches[si].constraints.push(Constraint::EdgeDistance { c1, c2, d: 2.0, m1: -1, m2: -1, off: 0.0, expr: String::new(), driven: false });
    p.solve_sketch(si);
    let g = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    let (a, b) = (g(c1), g(c2));
    let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    assert!((dist - 7.0).abs() < 2e-2, "rim gap of 2: the centre distance is R1 + R2 + gap = 7, got {dist}");
}
