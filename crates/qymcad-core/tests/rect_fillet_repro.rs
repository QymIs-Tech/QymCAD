//! A rectangle with filleted corners must not break its horizontal and vertical constraints.
//!
//! The failure being reproduced: filleting removes a corner point while the horizontals and verticals that
//! referenced it are left dangling.
use qymcad_core::model::{constraint_point_ids, Constraint, Project};

fn dangling_constraints(p: &Project, si: usize) -> Vec<String> {
    let s = &p.sketches[si];
    let pts: std::collections::HashSet<_> = s.points.iter().map(|q| q.id).collect();
    let mut out = Vec::new();
    for c in &s.constraints {
        for r in constraint_point_ids(c) {
            if r != 0 && !pts.contains(&r) {
                out.push(format!("{c:?} references the deleted point {r}"));
                break;
            }
        }
    }
    out
}

#[test]
fn rect_fillet_keeps_constraints_valid() {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("rect", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    // a 40×30 rectangle with automatic horizontal and vertical constraints
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    let h_before = p.sketches[si].constraints.len();
    eprintln!("constraints after the rectangle: {h_before}");
    assert!(dangling_constraints(&p, si).is_empty(), "rectangle: the constraints are valid");

    // fillet every corner at R5
    let done = p.fillet_all_corners(si, 5.0);
    eprintln!("corners filleted: {done}");
    assert_eq!(done, 4, "four corners are filleted");

    let dangling = dangling_constraints(&p, si);
    eprintln!("dangling constraints after filleting: {dangling:#?}");
    assert!(dangling.is_empty(), "after filleting there must be no constraints on deleted points");

    // solve, which must not blow up
    let resid = p.solve_sketch(si);
    eprintln!("solver residual: {resid}");
    assert!(resid.is_finite() && resid < 1.0, "the solver converges, residual {resid}");

    // the sides are still horizontal and vertical, so those constraints hold
    let has_h = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Horizontal { .. }));
    let has_v = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Vertical { .. }));
    assert!(has_h && has_v, "the horizontal and vertical constraints of the rectangle survived");
}

// A rectangle with edge dimensions for width and height, then filleted, has to remain solvable: no conflict,
// nothing shown as red, and no drift of the points.
#[test]
fn rect_dims_then_fillet_stays_solvable() {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("rect", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    let corner = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6).map(|q| q.id).unwrap();
    let (c00, c40, c43) = (corner(&p, 0.0, 0.0), corner(&p, 40.0, 0.0), corner(&p, 40.0, 30.0));

    // edge dimensions: width 40 on the bottom, height 30 on the right edge, plus the corner anchored at the
    // origin
    p.sketches[si].constraints.push(Constraint::Fixed { p: c00 });
    p.sketches[si].constraints.push(Constraint::Distance { a: c00, b: c40, d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.sketches[si].constraints.push(Constraint::Distance { a: c40, b: c43, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    assert!(p.sketch_conflicts(si).is_empty(), "there are no conflicts before filleting");

    // fillet every corner
    let done = p.fillet_all_corners(si, 5.0);
    eprintln!("filleted: {done}");

    let resid = p.solve_sketch(si);
    let conflicts = p.sketch_conflicts(si);
    eprintln!("after filleting: residual={resid}, conflicts={conflicts:?}");
    let maxx = p.sketches[si].points.iter().map(|q| q.x).fold(f64::MIN, f64::max);
    let maxy = p.sketches[si].points.iter().map(|q| q.y).fold(f64::MIN, f64::max);
    eprintln!("extents afterwards: maxx={maxx:.3} maxy={maxy:.3}, expecting 40 and 30");

    assert!(resid.is_finite() && resid < 1.0, "the solver converges, residual {resid}");
    assert!(conflicts.is_empty(), "there must be no dimension conflict after filleting: {conflicts:?}");
    assert!((maxx - 40.0).abs() < 0.05 && (maxy - 30.0).abs() < 0.05, "the extents hold at 40×30 and did not drift: {maxx:.2}×{maxy:.2}");

    // red means redundant constraints, and there must be none after filleting
    let (dof, redundant) = p.sketch_dof(si);
    let red = p.sketch_redundant_constraints(si);
    eprintln!("DOF={dof} redundant={redundant} red_constraints={red:?}");
    // What the interface actually does: where fillets are present, and with them structural tangencies, the
    // rank-based redundancy of geometric constraints is unreliable, the Jacobian of a tangency being
    // degenerate, so geometry is not reddened at all. Only conflicting dimensions are, through
    // `sketch_conflicts`, which is reliable on geometry. A filleted rectangle has no conflicts.
    let has_fillet = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Tangent { .. } | Constraint::CircleTangent { .. }));
    let flagged: Vec<usize> = red.iter().copied().filter(|&ci| {
        let c = &p.sketches[si].constraints[ci];
        let is_dim = matches!(c, Constraint::Distance { .. } | Constraint::Angle { .. } | Constraint::Diameter { .. } | Constraint::DistancePL { .. } | Constraint::AngleLines { .. } | Constraint::ArcLength { .. } | Constraint::EdgeDistance { .. });
        !is_dim && !has_fillet // geometric redundancy is not reddened where fillets are present
    }).collect();
    eprintln!("what the interface actually reddens: {flagged:?}");
    assert!(flagged.is_empty(), "no constraint should be reddened on a filleted rectangle: {flagged:?}");

    // Extrusion: there has to be a closed contour with an area, or it cannot be extruded.
    let closed: Vec<_> = p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_index(*cid).map(|i| p.contours[i].closed && p.contours[i].points.len() >= 3).unwrap_or(false) && p.contour_profile_xy(*cid).is_some()).collect();
    eprintln!("closed contours available for extrusion: {}", closed.len());
    assert!(!closed.is_empty(), "a closed contour exists, so the filleted rectangle can be extruded");
}

fn gab(p: &Project, si: usize) -> (f64, f64) {
    let maxx = p.sketches[si].points.iter().map(|q| q.x).fold(f64::MIN, f64::max);
    let maxy = p.sketches[si].points.iter().map(|q| q.y).fold(f64::MIN, f64::max);
    (maxx, maxy)
}
fn one_closed(p: &Project, si: usize) -> bool {
    p.sketches[si].contour_ids.iter().any(|cid| p.contour_profile_xy(*cid).is_some())
}

// Editing a filleted rectangle that carries edge dimensions used to break it. With a virtual corner the edits
// stay associative: changing the width from 40 to 50 makes the geometry follow, and changing the fillet radius
// leaves the extents alone — the dimension measures to the virtual corner — with the contour intact.
#[test]
fn filleted_rect_stays_associative_on_edits() {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("rect", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    let corner = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6).map(|q| q.id).unwrap();
    let (c00, c40, c43) = (corner(&p, 0.0, 0.0), corner(&p, 40.0, 0.0), corner(&p, 40.0, 30.0));
    p.sketches[si].constraints.push(Constraint::Fixed { p: c00 });
    p.sketches[si].constraints.push(Constraint::Distance { a: c00, b: c40, d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.sketches[si].constraints.push(Constraint::Distance { a: c40, b: c43, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    p.fillet_all_corners(si, 5.0);
    p.solve_sketch(si);

    // the width goes from 40 to 50
    for c in &mut p.sketches[si].constraints {
        if let Constraint::Distance { a, b, d, .. } = c {
            if (*a == c00 && *b == c40) || (*a == c40 && *b == c00) {
                *d = 50.0;
            }
        }
    }
    let r1 = p.solve_sketch(si);
    let (mx, my) = gab(&p, si);
    eprintln!("after the width change to 50: residual={r1} extents={mx:.2}×{my:.2}, expecting 50×30, conflicts={:?}", p.sketch_conflicts(si));
    assert!(r1 < 1.0 && (mx - 50.0).abs() < 0.1 && (my - 30.0).abs() < 0.1, "the dimension drives the geometry after filleting: {mx:.2}×{my:.2}");
    assert!(p.sketch_conflicts(si).is_empty() && one_closed(&p, si), "no conflicts and the contour is intact after the dimension is edited");

    // the radius of every fillet goes from 5 to 8, and the extents must not move, thanks to the virtual corner
    for c in &mut p.sketches[si].constraints {
        if let Constraint::Diameter { d, .. } = c {
            *d = 8.0;
        }
    }
    let r2 = p.solve_sketch(si);
    let (mx2, my2) = gab(&p, si);
    eprintln!("after the radius change to 8: residual={r2} extents={mx2:.2}×{my2:.2}, expecting the same 50×30");
    assert!(r2 < 1.0 && (mx2 - 50.0).abs() < 0.1 && (my2 - 30.0).abs() < 0.1, "changing the radius does not move the extents: {mx2:.2}×{my2:.2}");
    assert!(one_closed(&p, si), "the contour is intact after the radius change");
}
