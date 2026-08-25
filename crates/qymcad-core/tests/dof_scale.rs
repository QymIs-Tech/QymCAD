//! Degrees of freedom and redundancy have to be invariant to the scale of a sketch.
//!
//! The rank is taken from the Jacobian against an absolute threshold of 1e-7. Without normalising the rows, a
//! large or small scale changes the magnitudes of the derivatives — `Parallel` scales with L², `Fixed` carries
//! ×50, tangencies sit near 1 — and the rank drifts, giving false redundancy or under-constraint depending on
//! the size of the part.

use qymcad_core::model::{Constraint, Project};

/// A w by h rectangle with the full set of constraints the tool creates, plus a diagonal dimension.
fn rect_sketch(scale: f64) -> (Project, usize) {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_rect_entity(si, 0.0, 0.0, 30.0 * scale, 20.0 * scale, qymcad_core::feature::Purpose::Real);
    // a corner anchor plus overall dimensions: a typical fully constrained rectangle
    let (a, b) = {
        let s = &p.sketches[si];
        let e = s.entities.iter().find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).unwrap();
        match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => (a, b),
            _ => unreachable!(),
        }
    };
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 30.0 * scale, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    (p, si)
}

#[test]
fn dof_invariant_under_scale() {
    let (p_mm, si_mm) = rect_sketch(1.0);
    let (p_m, si_m) = rect_sketch(1000.0);
    let d_mm = p_mm.sketch_dof(si_mm);
    let d_m = p_m.sketch_dof(si_m);
    assert_eq!(d_mm, d_m, "degrees of freedom and redundancy depend on the scale of the sketch: mm={d_mm:?}, m={d_m:?}");
}

/// The hard case: a parallelogram held by `Parallel` and `Equal`, whose derivatives scale with length, plus a
/// circle tangency.
fn para_sketch(scale: f64) -> (Project, usize) {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let s = scale;
    let l1 = p.add_line_entity(si, 0.0, 0.0, 30.0 * s, 0.0, qymcad_core::feature::Purpose::Real);
    let l2 = p.add_line_entity(si, 5.0 * s, 15.0 * s, 36.0 * s, 15.0 * s, qymcad_core::feature::Purpose::Real);
    let ids = |p: &Project, eid: u64| -> (u64, u64) {
        match p.sketches[si].entities.iter().find(|e| e.id == eid).unwrap().kind {
            qymcad_core::model::EntityKind::Line { a, b } => (a, b),
            _ => unreachable!(),
        }
    };
    let (a, b) = ids(&p, l1);
    let (c, d) = ids(&p, l2);
    let e1 = p.add_circle_entity(si, 15.0 * s, 7.0 * s, 7.0 * s, qymcad_core::feature::Purpose::Real);
    let cc = match p.sketches[si].entities.iter().find(|e| e.id == e1).unwrap().kind {
        qymcad_core::model::EntityKind::Circle { center, .. } => center,
        _ => unreachable!(),
    };
    let k = &mut p.sketches[si].constraints;
    k.push(Constraint::Fixed { p: a });
    k.push(Constraint::Fixed { p: b });
    k.push(Constraint::Parallel { a, b, c, d });
    k.push(Constraint::Equal { a, b, c, d });
    k.push(Constraint::Tangent { a, b, c: cc, r: 7.0 * s });
    p.solve_sketch(si);
    (p, si)
}

#[test]
fn dof_invariant_parallel_tangent_scales() {
    let base = para_sketch(1.0);
    let d0 = base.0.sketch_dof(base.1);
    for scale in [1000.0, 0.001] {
        let (p, si) = para_sketch(scale);
        let d = p.sketch_dof(si);
        assert_eq!(d0, d, "the degrees of freedom of a parallelogram with a tangency depend on the scale ×{scale}: {d0:?} vs {d:?}");
    }
}

#[test]
fn dof_invariant_small_scale() {
    let (p_mm, si_mm) = rect_sketch(1.0);
    let (p_um, si_um) = rect_sketch(0.001); // a tiny part
    let d_mm = p_mm.sketch_dof(si_mm);
    let d_um = p_um.sketch_dof(si_um);
    assert_eq!(d_mm, d_um, "the degrees of freedom depend on scale: mm={d_mm:?}, micron scale={d_um:?}");
}
