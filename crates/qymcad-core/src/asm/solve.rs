//! The solving core: residuals, derivatives, iteration.
//!
//! The unknowns are not the poses themselves but increments to them: 6 per ungrounded body, 3 for
//! translation and 3 for rotation. An increment is applied on the left, with the rotational part
//! going through the exponential map. That leaves no gimbal lock and no discontinuity at a half-turn,
//! and the pose stays an exact rotation at every step — unlike editing a matrix, where orthogonality
//! has to be restored by hand.
//!
//! The derivatives are analytic rather than automatic. `Isometry3` from `nalgebra` works with a
//! concrete `f64`, so pushing dual numbers through it would mean making the whole residual code
//! generic over the scalar type — a large change for something that fits in two rules for these six
//! primitives:
//!
//!   point   p' = p + dt + dw x p   ->   dp/d(dt) = I,   dp/d(dw) = -[p]x
//!   vector  d' = d + dw x d        ->   dd/d(dt) = 0,   dd/d(dw) = -[d]x
//!
//! Hand-derived formulas are risky, so every derivative is checked against a central numerical
//! difference in the tests. A wrong formula then fails a test instead of showing up as a body landing
//! in the wrong place.

use nalgebra::{DMatrix, DVector, Isometry3, Matrix3, Translation3, UnitQuaternion, Vector3};

use super::problem::{Constraint, Problem, SlotMeasure};

/// Skew-symmetric cross-product matrix: `[v]x * w = v x w`.
fn skew(v: Vector3<f64>) -> Matrix3<f64> {
    Matrix3::new(0.0, -v.z, v.y, v.z, 0.0, -v.x, -v.y, v.x, 0.0)
}

/// Apply an increment (translation plus rotation) to a body pose.
///
/// On the left and through the exponential map: `T' = Exp(dw) . T`, then the translation. Multiplying
/// on the right would be wrong here, because the increment is expressed in world axes rather than in
/// the axes of the body.
pub fn retract(pose: &Isometry3<f64>, dt: Vector3<f64>, dw: Vector3<f64>) -> Isometry3<f64> {
    let rot = UnitQuaternion::from_scaled_axis(dw);
    // The rotation is about the origin of the body, not about the world origin.
    //
    // Rotating the translation as well (`rot * pose.translation.vector + dt`) drags a body that
    // stands away from the world origin by `(rot - I) * p`. Translation and rotation then become
    // coupled, and a minimum-norm step prefers "rotate" over "translate" because that is cheaper by
    // norm. Measured: a constraint asking for 17 mm of travel along one axis was answered with a
    // rotation of 0.8 rad, and the body rolled 14 mm sideways off its place.
    //
    // About its own origin, translation and rotation are independent: what is asked for is what
    // happens.
    Isometry3::from_parts(Translation3::from(pose.translation.vector + dt), rot * pose.rotation)
}

/// Layout of the unknowns: the column where the 6 increments of a body start.
/// Grounded bodies have no column — they take no part in the solve at all.
#[derive(Clone, Debug)]
pub struct Layout {
    col: Vec<Option<usize>>,
    pub unknowns: usize,
}

impl Layout {
    pub fn of(problem: &Problem) -> Self {
        let mut col = Vec::with_capacity(problem.bodies.len());
        let mut n = 0;
        for b in &problem.bodies {
            if b.grounded {
                col.push(None);
            } else {
                col.push(Some(n));
                n += 6;
            }
        }
        Self { col, unknowns: n }
    }

    pub fn column_of(&self, body: usize) -> Option<usize> {
        self.col.get(body).copied().flatten()
    }
}

/// Body poses for a given vector of increments.
pub fn poses_at(problem: &Problem, layout: &Layout, x: &DVector<f64>) -> Vec<Isometry3<f64>> {
    problem
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| match layout.column_of(i) {
            None => b.pose,
            Some(c) => retract(&b.pose, Vector3::new(x[c], x[c + 1], x[c + 2]), Vector3::new(x[c + 3], x[c + 4], x[c + 5])),
        })
        .collect()
}

/// The measurement axis and a perpendicular to it, both in the anchor frame, selected by axis index.
///
/// The perpendicular is needed only by angles: it acts as the pointer whose rotation is measured. The
/// pairing matches `bridge::measured_slot` — X is measured against Y, Y against Z, Z against X.
fn measure_dirs(axis: u8, q: &UnitQuaternion<f64>) -> (Vector3<f64>, Vector3<f64>) {
    match axis {
        0 => (q * Vector3::x(), q * Vector3::y()),
        1 => (q * Vector3::y(), q * Vector3::z()),
        _ => (q * Vector3::z(), q * Vector3::x()),
    }
}

/// Value of a measurement: travel in millimetres along an axis, or an angle in radians about it.
///
/// Radians rather than degrees, even though degrees are displayed. A relation compares two
/// measurements in a single equation: in degrees its Jacobian row would be 57 times larger than its
/// neighbours, and the solver would prefer rotation over translation because of a unit choice rather
/// than the meaning of the problem. Converting to degrees belongs to the layer that shows the number.
pub fn measure(m: &SlotMeasure, poses: &[Isometry3<f64>]) -> f64 {
    let (pa, pb) = (poses[m.a.body], poses[m.b.body]);
    let (wa, wb) = (m.a.world(&pa), m.b.world(&pb));
    let (n, perp_a) = measure_dirs(m.axis, &wa.rotation);
    if !m.rotation {
        return n.dot(&(wb.translation.vector - wa.translation.vector));
    }
    let (_, perp_b) = measure_dirs(m.axis, &wb.rotation);
    let s = n.dot(&perp_a.cross(&perp_b));
    let c = perp_a.dot(&perp_b);
    s.atan2(c)
}

/// Derivatives of a measurement with respect to the increments of its bodies: `(for body a, for body
/// b)`, each 1x6.
///
/// Hand-derived like the rest of this module and checked against a central difference in the tests.
/// For an angle:
///
///   θ = atan2(s, c),  s = n·(pₐ×p_b),  c = pₐ·p_b
///   ∂s/∂ω_a = (n·p_b)·pₐ − c·n     ∂c/∂ω_a = pₐ×p_b
///   ∂s/∂ω_b = c·n − (n·p_b)·pₐ     ∂c/∂ω_b = p_b×pₐ
///   ∂θ = (c·∂s − s·∂c)/(s² + c²)
fn measure_grad(m: &SlotMeasure, poses: &[Isometry3<f64>]) -> (nalgebra::Matrix1x6<f64>, nalgebra::Matrix1x6<f64>) {
    let (pa, pb) = (poses[m.a.body], poses[m.b.body]);
    let (wa, wb) = (m.a.world(&pa), m.b.world(&pb));
    let (n, perp_a) = measure_dirs(m.axis, &wa.rotation);
    if !m.rotation {
        // m = n . (o_b - o_a): the same two rows as `OnPlane`, except the axis need not be the main one.
        let d = wb.translation.vector - wa.translation.vector;
        let dn = block(n, false);
        let ga = -(n.transpose() * block(wa.translation.vector - pa.translation.vector, true)) + d.transpose() * dn;
        let gb = n.transpose() * block(wb.translation.vector - pb.translation.vector, true);
        return (ga, gb);
    }
    let (_, perp_b) = measure_dirs(m.axis, &wb.rotation);
    let s = n.dot(&perp_a.cross(&perp_b));
    let c = perp_a.dot(&perp_b);
    let den = s * s + c * c;
    let mut ga = nalgebra::Matrix1x6::zeros();
    let mut gb = nalgebra::Matrix1x6::zeros();
    if den > 1e-18 {
        let npb = n.dot(&perp_b);
        let ds_a = perp_a * npb - n * c;
        let dc_a = perp_a.cross(&perp_b);
        let ds_b = n * c - perp_a * npb;
        let dc_b = perp_b.cross(&perp_a);
        // Derivative with respect to rotation: an angle does not depend on translation at all, so the first three are zero.
        let put = |v: Vector3<f64>| nalgebra::Matrix1x6::new(0.0, 0.0, 0.0, v.x, v.y, v.z);
        ga = put((ds_a * c - dc_a * s) / den);
        gb = put((ds_b * c - dc_b * s) / den);
    }
    (ga, gb)
}

/// Reduce a residual modulo its period: `r -> r - period * round(r / period)`.
///
/// A period of zero means the quantity is not circular, and no reduction is done.
fn wrap(r: f64, period: f64) -> f64 {
    if period <= 0.0 {
        return r;
    }
    r - period * (r / period).round()
}

/// Residual of one constraint at the given poses. Its length matches `Constraint::rows`.
pub fn residual_of(c: &Constraint, poses: &[Isometry3<f64>]) -> Vec<f64> {
    if let Constraint::SlotRatio { m1, m2, ratio, offset, period } = *c {
        return vec![wrap(measure(&m1, poses) - ratio * measure(&m2, poses) - offset, period)];
    }
    // The drag target contributes no equations (`rows()` is 0): it is a goal, not a constraint, and
    // moves the body through a separate null-space step. This branch has to come before the pairwise
    // handling, because the goal has no pair and `pair()` below would panic.
    if matches!(*c, Constraint::Pull { .. }) {
        return Vec::new();
    }
    let (a, b) = c.pair().expect("pairwise primitive: relations are handled above");
    let (pa, pb) = (poses[a.body], poses[b.body]);
    match *c {
        // The drag target is handled above, before the pairwise branch: it has no pair.
        Constraint::Pull { .. } => Vec::new(),
        Constraint::PointCoincident { .. } => {
            let d = b.world_origin(&pb) - a.world_origin(&pa);
            vec![d.x, d.y, d.z]
        }
        Constraint::AxisAligned { .. } => {
            let d = b.world_z(&pb) - a.world_z(&pa);
            vec![d.x, d.y, d.z]
        }
        Constraint::RollAligned { .. } => {
            let d = b.world_x(&pb) - a.world_x(&pa);
            vec![d.x, d.y, d.z]
        }
        Constraint::OnAxis { .. } => {
            let z = a.world_z(&pa);
            let d = b.world_origin(&pb) - a.world_origin(&pa);
            let perp = d - z * z.dot(&d);
            vec![perp.x, perp.y, perp.z]
        }
        Constraint::OnPlane { offset, .. } => {
            let z = a.world_z(&pa);
            let d = b.world_origin(&pb) - a.world_origin(&pa);
            vec![z.dot(&d) - offset]
        }
        Constraint::Angle { deg, .. } => {
            let (za, zb) = (a.world_z(&pa), b.world_z(&pb));
            vec![za.dot(&zb) - deg.to_radians().cos()]
        }
        Constraint::PointDistance { dist, .. } => {
            let d = b.world_origin(&pb) - a.world_origin(&pa);
            vec![d.norm() - dist]
        }
        Constraint::AxisDistance { dist, .. } => {
            let z = a.world_z(&pa);
            let d = b.world_origin(&pb) - a.world_origin(&pa);
            vec![(d - z * z.dot(&d)).norm() - dist]
        }
        Constraint::SlotRatio { .. } => unreachable!("handled at the top of this function"),
    }
}

/// Full residual vector of the problem.
pub fn residuals(problem: &Problem, poses: &[Isometry3<f64>]) -> DVector<f64> {
    let mut out = Vec::with_capacity(problem.rows());
    for c in &problem.constraints {
        out.extend(residual_of(c, poses));
    }
    DVector::from_vec(out)
}

/// Derivative block of a residual with respect to the increments of one body: `[dr/d(dt) |
/// dr/d(dw)]`, 3x6.
///
/// `point` is the world point the residual holds on to. For directions it is the vector itself, and
/// translation then has no effect on it.
fn block(point: Vector3<f64>, translates: bool) -> nalgebra::Matrix3x6<f64> {
    // For translational residuals `point` is the lever arm from the body origin, since `retract`
    // rotates about that origin. For directions it is the vector itself and the origin is irrelevant.
    let mut m = nalgebra::Matrix3x6::zeros();
    if translates {
        m.fixed_view_mut::<3, 3>(0, 0).copy_from(&Matrix3::identity());
    }
    m.fixed_view_mut::<3, 3>(0, 3).copy_from(&(-skew(point)));
    m
}

/// Derivatives of the transverse component `perp = d - z(z . d)` with respect to both bodies:
/// `(a, b)`, 3x6.
///
/// Shared by two constraints: "point on axis" holds this vector whole, while "distance to axis" holds
/// only its length. Computing them apart would mean two copies of one formula to repair separately.
///
///   ∂perp = (I − zzᵀ)·∂d − [ z·(∂z·d)ᵀ + (∂z)·(z·d) ]
fn perp_blocks(oa: Vector3<f64>, ob: Vector3<f64>, za: Vector3<f64>, pa: &Isometry3<f64>, pb: &Isometry3<f64>) -> (nalgebra::Matrix3x6<f64>, nalgebra::Matrix3x6<f64>) {
    let d = ob - oa;
    let proj = Matrix3::identity() - za * za.transpose();
    // For body b: d depends only on o_b.
    let gb = proj * block(ob - pb.translation.vector, true);
    // For body a: dd/da = -d(o_a); dz/da = -[z]x for rotation.
    let dz = block(za, false);
    let zd = za.dot(&d);
    let ga = -(proj * block(oa - pa.translation.vector, true)) - (za * (dz.transpose() * d).transpose() + dz * zd);
    (ga, gb)
}

/// Jacobian of the whole problem: rows are constraint equations, columns are increments of ungrounded bodies.
pub fn jacobian(problem: &Problem, layout: &Layout, poses: &[Isometry3<f64>]) -> DMatrix<f64> {
    let mut j = DMatrix::zeros(problem.rows(), layout.unknowns);
    let mut row = 0;
    for c in &problem.constraints {
        // A relation is a single row over up to four bodies, and the contributions add up: in a gear
        // pair on a shared housing the same body appears in both measurements.
        if let Constraint::SlotRatio { m1, m2, ratio, .. } = *c {
            let (g1a, g1b) = measure_grad(&m1, poses);
            let (g2a, g2b) = measure_grad(&m2, poses);
            for (anchor, g) in [(m1.a, g1a), (m1.b, g1b), (m2.a, -ratio * g2a), (m2.b, -ratio * g2b)] {
                if let Some(col) = layout.column_of(anchor.body) {
                    let mut dst = j.view_mut((row, col), (1, 6));
                    dst += g;
                }
            }
            row += 1;
            continue;
        }
        // The drag target occupies no rows (`rows()` is 0) and has no derivative in this matrix. The
        // branch comes before the pairwise handling, because the goal has no pair.
        if matches!(*c, Constraint::Pull { .. }) {
            continue;
        }
        let (a, b) = c.pair().expect("pairwise primitive: relations are handled above");
        let (pa, pb) = (poses[a.body], poses[b.body]);
        let (ca, cb) = (layout.column_of(a.body), layout.column_of(b.body));
        let (oa, ob) = (a.world_origin(&pa), b.world_origin(&pb));
        let (za, zb) = (a.world_z(&pa), b.world_z(&pb));

        // Place a 3x6 block into rows [row..row+3) for the body starting at column `col`, with a sign.
        let put3 = |j: &mut DMatrix<f64>, col: Option<usize>, blk: nalgebra::Matrix3x6<f64>, sign: f64| {
            if let Some(col) = col {
                let mut dst = j.view_mut((row, col), (3, 6));
                dst += blk * sign;
            }
        };
        // Place a 1x6 row.
        let put1 = |j: &mut DMatrix<f64>, col: Option<usize>, r: nalgebra::Matrix1x6<f64>| {
            if let Some(col) = col {
                let mut dst = j.view_mut((row, col), (1, 6));
                dst += r;
            }
        };

        match *c {
            // The drag target is handled above, before the pairwise branch: it has no pair.
            Constraint::Pull { .. } => {}
            Constraint::PointCoincident { .. } => {
                put3(&mut j, cb, block(ob - pb.translation.vector, true), 1.0);
                put3(&mut j, ca, block(oa - pa.translation.vector, true), -1.0);
            }
            Constraint::AxisAligned { .. } => {
                put3(&mut j, cb, block(zb, false), 1.0);
                put3(&mut j, ca, block(za, false), -1.0);
            }
            Constraint::RollAligned { .. } => {
                let (xa, xb) = (a.world_x(&pa), b.world_x(&pb));
                put3(&mut j, cb, block(xb, false), 1.0);
                put3(&mut j, ca, block(xa, false), -1.0);
            }
            Constraint::OnAxis { .. } => {
                let (ga, gb) = perp_blocks(oa, ob, za, &pa, &pb);
                put3(&mut j, cb, gb, 1.0);
                put3(&mut j, ca, ga, 1.0);
            }
            Constraint::PointDistance { .. } => {
                // r = |o_b - o_a| - dist. Derivative of a length: d|v| = (v/|v|)^T . dv.
                //
                // At coincident points (|d| close to 0) there is nothing to repair: the direction
                // "away from each other" is undefined there, so any choice would be invented. The row
                // stays zero, damping produces the step, and the residual reports the miss.
                let d = ob - oa;
                let n = d.norm();
                if n > 1e-12 {
                    let u = d / n;
                    put1(&mut j, cb, u.transpose() * block(ob - pb.translation.vector, true));
                    put1(&mut j, ca, -(u.transpose() * block(oa - pa.translation.vector, true)));
                }
            }
            Constraint::AxisDistance { .. } => {
                // r = |perp| - dist. Derivative of a length: d|v| = (v/|v|)^T . dv, and d(perp) is already known.
                //
                // On the axis itself (|perp| close to 0) there is nothing to repair: the direction
                // "away from the axis" is undefined there. The row stays zero, damping produces the
                // step, and the residual reports the miss.
                let d = ob - oa;
                let perp = d - za * za.dot(&d);
                let n = perp.norm();
                if n > 1e-12 {
                    let u = perp / n;
                    let (ga, gb) = perp_blocks(oa, ob, za, &pa, &pb);
                    put1(&mut j, cb, u.transpose() * gb);
                    put1(&mut j, ca, u.transpose() * ga);
                }
            }
            Constraint::OnPlane { .. } => {
                // r = z·(o_b − o_a) − offset
                let d = ob - oa;
                if let Some(col) = cb {
                    let r = za.transpose() * block(ob - pb.translation.vector, true);
                    put1(&mut j, Some(col), r);
                }
                if let Some(col) = ca {
                    let dz = block(za, false);
                    let r = -(za.transpose() * block(oa - pa.translation.vector, true)) + d.transpose() * dz;
                    put1(&mut j, Some(col), r);
                }
            }
            Constraint::Angle { .. } => {
                // r = z_a·z_b − cos θ
                if let Some(col) = cb {
                    let r = za.transpose() * block(zb, false);
                    put1(&mut j, Some(col), r);
                }
                if let Some(col) = ca {
                    let r = zb.transpose() * block(za, false);
                    put1(&mut j, Some(col), r);
                }
            }
            Constraint::SlotRatio { .. } => unreachable!("handled at the top of the loop"),
        }
        row += c.rows();
    }
    j
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::frame::Anchor;
    use crate::asm::problem::Body;

    fn pose(x: f64, y: f64, z: f64, ax: Vector3<f64>, ang: f64) -> Isometry3<f64> {
        Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(ax), ang))
    }

    /// A problem covering every case: two free bodies, one grounded, anchors with offset and rotation.
    fn sample() -> Problem {
        let mut p = Problem::new(vec![
            Body::grounded(pose(0.0, 0.0, 0.0, Vector3::z(), 0.0)),
            Body::new(pose(7.0, -3.0, 2.0, Vector3::new(1.0, 2.0, 3.0), 0.7)),
            Body::new(pose(-4.0, 5.0, 1.0, Vector3::new(-2.0, 1.0, 0.5), -1.1)),
        ]);
        let a0 = Anchor::from_axes(0, Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0), Vector3::x()).unwrap();
        let a1 = Anchor::from_axes(1, Vector3::new(0.0, 2.0, 0.0), Vector3::new(1.0, 1.0, 0.0), Vector3::z()).unwrap();
        let a2 = Anchor::from_axes(2, Vector3::new(0.5, 0.5, -1.0), Vector3::new(0.0, 1.0, 1.0), Vector3::x()).unwrap();
        p.add(Constraint::PointCoincident { a: a0, b: a1 });
        p.add(Constraint::AxisAligned { a: a1, b: a2 });
        p.add(Constraint::OnAxis { a: a0, b: a2 });
        // The axis must sit on a free body. For "point on axis" and "distance to axis" the
        // derivative with respect to the body that owns the axis is the longest formula of all, and it
        // was the one left unchecked: with the axis on a grounded body there are no columns for it, so
        // a corrupted formula passed unnoticed. Found by deliberately corrupting a coefficient and
        // seeing the comparison stay green.
        p.add(Constraint::OnAxis { a: a1, b: a2 });
        p.add(Constraint::OnPlane { a: a1, b: a2, offset: 1.5 });
        p.add(Constraint::RollAligned { a: a0, b: a1 });
        p.add(Constraint::Angle { a: a1, b: a2, deg: 35.0 });
        p.add(Constraint::AxisDistance { a: a1, b: a2, dist: 4.5 });
        p.add(Constraint::PointDistance { a: a1, b: a2, dist: 6.5 });
        // Relations with both kinds of measurement and both axes, so the numerical comparison covers
        // all four hand-derived derivatives.
        p.add(Constraint::slot_ratio(SlotMeasure::around(a0, a1, 2), SlotMeasure::around(a1, a2, 2), 2.0, 0.3));
        p.add(Constraint::slot_ratio(SlotMeasure::along(a0, a2, 2), SlotMeasure::along(a1, a2, 0), -1.5, 4.0));
        p.add(Constraint::slot_ratio(SlotMeasure::along(a0, a1, 1), SlotMeasure::around(a2, a0, 1), 3.0, 0.0));
        p
    }

    /// The central check of this module: analytic derivatives against numerical ones.
    ///
    /// A hand-derived derivative is the one place where a mistake stays silent: the solver converges,
    /// but to the wrong place, and that is indistinguishable from bad geometry. Here it becomes a
    /// failing test. Central differences are compared for every unknown and every equation.
    #[test]
    fn analytic_jacobian_matches_numerical_differentiation() {
        let p = sample();
        let layout = Layout::of(&p);
        let x0 = DVector::zeros(layout.unknowns);
        let poses = poses_at(&p, &layout, &x0);
        let j = jacobian(&p, &layout, &poses);

        let h = 1e-6;
        let mut worst = 0.0f64;
        let mut worst_at = (0usize, 0usize);
        for k in 0..layout.unknowns {
            let (mut xp, mut xm) = (x0.clone(), x0.clone());
            xp[k] += h;
            xm[k] -= h;
            let rp = residuals(&p, &poses_at(&p, &layout, &xp));
            let rm = residuals(&p, &poses_at(&p, &layout, &xm));
            for r in 0..p.rows() {
                let num = (rp[r] - rm[r]) / (2.0 * h);
                let err = (num - j[(r, k)]).abs();
                if err > worst {
                    worst = err;
                    worst_at = (r, k);
                }
            }
        }
        assert!(
            worst < 1e-6,
            "analytic derivative differs from the numerical one by {worst:.3e} (equation {}, unknown {}): the formula is wrong",
            worst_at.0,
            worst_at.1
        );
    }

    #[test]
    fn grounded_bodies_have_no_unknowns() {
        let p = sample();
        let l = Layout::of(&p);
        assert_eq!(l.unknowns, 12, "two free bodies mean twelve unknowns");
        assert!(l.column_of(0).is_none(), "a grounded body must have no columns");
        assert!(l.column_of(1).is_some() && l.column_of(2).is_some());
    }

    #[test]
    fn retraction_keeps_rotation_exact() {
        // Accumulate many small rotations: a matrix representation builds up non-orthogonality here,
        // a quaternion does not.
        let mut t = Isometry3::identity();
        for _ in 0..10_000 {
            t = retract(&t, Vector3::new(0.001, 0.0, 0.0), Vector3::new(0.0, 0.001, 0.0));
        }
        let r = t.rotation.to_rotation_matrix();
        let err = (r.matrix() * r.matrix().transpose() - Matrix3::identity()).norm();
        // The threshold is not machine epsilon but "no drift": over 10 000 steps only f64 rounding
        // accumulates (about 1e-12). A matrix representation reaches 1e-6 and worse here and has to be
        // repaired by re-orthogonalisation, moving the body slightly every time.
        assert!(err < 1e-9, "rotation drifts away from orthogonality: {err:.3e}");
    }

    #[test]
    fn residual_is_zero_exactly_when_the_constraint_holds() {
        // Coincident points: two anchors at the same world point.
        let a = Anchor::new(0, Isometry3::translation(3.0, 0.0, 0.0));
        let b = Anchor::new(1, Isometry3::translation(0.0, 0.0, 0.0));
        let p = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(Isometry3::translation(3.0, 0.0, 0.0))]);
        let poses: Vec<_> = p.bodies.iter().map(|b| b.pose).collect();
        let r = residual_of(&Constraint::PointCoincident { a, b }, &poses);
        assert!(r.iter().all(|v| v.abs() < 1e-12), "the constraint holds, so the residual must be zero: {r:?}");

        // And not zero when it does not hold.
        let p2 = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(Isometry3::translation(5.0, 0.0, 0.0))]);
        let poses2: Vec<_> = p2.bodies.iter().map(|b| b.pose).collect();
        let r2 = residual_of(&Constraint::PointCoincident { a, b }, &poses2);
        assert!((r2[0] - 2.0).abs() < 1e-12, "the residual must equal the miss (2 mm), not merely be close to something: {r2:?}");
    }

    /// An angle is measured as an angle: the residual is zero exactly where the constraint says.
    ///
    /// Added after a mutation pass: swapping cosine for sine in the residual made nothing in the core
    /// fail. The meaning of the primitive was held only by end-to-end checks on live geometry, so its
    /// formula could be debugged only through the kernel and a real shaft.
    #[test]
    fn an_angle_residual_is_zero_exactly_at_the_angle_it_names() {
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let turn = |deg: f64| Isometry3::from_parts(Translation3::identity(), UnitQuaternion::from_axis_angle(&Vector3::x_axis(), deg.to_radians()));
        for deg in [0.0, 30.0, 90.0, 150.0] {
            let poses = vec![Isometry3::identity(), turn(deg)];
            let r = residual_of(&Constraint::Angle { a, b, deg }, &poses)[0];
            assert!(r.abs() < 1e-12, "axes are {deg} degrees apart and the constraint asks for {deg}: the residual must be zero, but it is {r:.3e}");
        }
        // And not zero at a different angle: the miss is a difference of cosines, nothing else.
        let poses = vec![Isometry3::identity(), turn(30.0)];
        let r = residual_of(&Constraint::Angle { a, b, deg: 60.0 }, &poses)[0];
        let want = 30.0f64.to_radians().cos() - 60.0f64.to_radians().cos();
        assert!((r - want).abs() < 1e-12, "an angular miss must equal the difference of cosines ({want:.6}), but it is {r:.6}");
    }

    /// Distance to an axis is measured across it; travel along the axis does not affect it.
    ///
    /// Added for the same reason: flipping a sign made nothing in the core fail.
    #[test]
    fn a_distance_to_an_axis_ignores_how_far_along_that_axis_you_are() {
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        for along in [0.0, 20.0, -75.0] {
            let poses = vec![Isometry3::identity(), Isometry3::translation(7.0, 0.0, along)];
            let r = residual_of(&Constraint::AxisDistance { a, b, dist: 7.0 }, &poses)[0];
            assert!(r.abs() < 1e-12, "a point 7 mm from the axis and {along} along it: the residual must be zero, but it is {r:.3e}");
        }
        // The miss is the signed shortfall against the requested distance.
        let poses = vec![Isometry3::identity(), Isometry3::translation(7.0, 0.0, 0.0)];
        let r = residual_of(&Constraint::AxisDistance { a, b, dist: 20.0 }, &poses)[0];
        assert!((r + 13.0).abs() < 1e-12, "13 mm short of the requested 20, so the residual must be -13, but it is {r:.6}");
    }

    /// And it takes effect: the body is moved away from the axis by exactly the requested distance.
    #[test]
    fn a_distance_to_an_axis_pushes_the_part_out_to_exactly_that_distance() {
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let mut p = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(Isometry3::translation(30.0, 0.0, 5.0))]);
        p.add(Constraint::AxisDistance { a, b, dist: 12.0 });
        let (poses, rep) = crate::asm::iterate::solve(&p);
        assert!(rep.converged, "must converge: {:.3e}", rep.residual);
        let o = poses[1].translation.vector;
        let d = (o.x * o.x + o.y * o.y).sqrt();
        assert!((d - 12.0).abs() < 1e-6, "the body must settle 12 mm from the axis, but it sits at {d:.6}");
        assert!((o.z - 5.0).abs() < 1e-6, "the constraint asks nothing along the axis, so the body must not travel in Z: {:.6}", o.z);
    }

    #[test]
    fn axis_alignment_is_not_fooled_by_a_180_degree_flip() {
        // Why three equations and not two: at a half-turn the projections of the axis difference onto
        // the perpendiculars would vanish, and the solver would accept a flipped body as a solution.
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let flipped = Isometry3::from_parts(Translation3::identity(), UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::PI));
        let poses = vec![Isometry3::identity(), flipped];
        let r = residual_of(&Constraint::AxisAligned { a, b }, &poses);
        let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        assert!((n - 2.0).abs() < 1e-9, "a flipped axis must give a residual of 2, not zero: {n}");
    }
}
