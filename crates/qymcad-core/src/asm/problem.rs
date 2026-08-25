//! The placement problem: bodies, constraints between their anchors, and what the solver reports.
//!
//! A constraint is described here purely as meaning — what must hold. No derivatives and no numerics:
//! derivatives come from automatic differentiation over the same code that computes the residual, and
//! the iteration is driven by `levenberg-marquardt`. The split is deliberate, because domain logic and
//! numerics fail in different ways and mixing them makes both harder to repair.

use super::frame::{Anchor, Pose};

pub type BodyId = usize;

/// A body of the assembly: its current pose and whether it is grounded.
///
/// A grounded body does not move at all: it contributes no unknowns. This is not a mate to the world
/// and not an infinitely stiff spring — the body simply has no degrees of freedom and never enters
/// the matrix.
#[derive(Clone, Debug)]
pub struct Body {
    pub pose: Pose,
    pub grounded: bool,
}

impl Body {
    pub fn new(pose: Pose) -> Self {
        Self { pose, grounded: false }
    }

    pub fn grounded(pose: Pose) -> Self {
        Self { pose, grounded: true }
    }
}

/// A measurement of one degree of freedom: how far anchor `b` has moved from `a` along it.
///
/// It requires nothing by itself and only produces a number — travel in millimetres along an axis, or
/// an angle about it. `Constraint::SlotRatio` turns two such measurements into a requirement.
///
/// The axis is named in the frame of anchor `a` (0 for X, 1 for Y, 2 for the main Z) using the same
/// table that hands out joint slots (`joint::slot_axis`). A different numbering would let a relation
/// hold one degree while the joint field displays another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotMeasure {
    /// Anchor that defines the axis and the origin of the measurement.
    pub a: Anchor,
    /// Anchor whose position is measured.
    pub b: Anchor,
    /// Axis index in the frame of anchor `a`.
    pub axis: u8,
    /// Rotation about the axis; otherwise travel along it.
    pub rotation: bool,
}

impl SlotMeasure {
    pub fn along(a: Anchor, b: Anchor, axis: u8) -> Self {
        Self { a, b, axis, rotation: false }
    }

    pub fn around(a: Anchor, b: Anchor, axis: u8) -> Self {
        Self { a, b, axis, rotation: true }
    }
}

/// A constraint: what must hold between two anchors.
///
/// These are primitives, not user-facing mates. A mate (revolute, slider) belongs to the layer above:
/// it states which degrees of freedom stay free and decomposes into a set of primitives. The split
/// matters because a primitive can be checked against a formula, while a mate can only be checked by
/// behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    /// The anchor origins coincide: 3 equations, removing 3 translations.
    PointCoincident { a: Anchor, b: Anchor },
    /// The main axes point the same way (not merely parallel): 3 equations of rank 2, removing 2
    /// rotations.
    ///
    /// Same direction rather than parallel: parallelism admits a half-turn, and the solver would be
    /// free to pick either, so the body appears to flip on its own. Where a flip is wanted, the mate
    /// layer states it with an explicit flag instead of relying on an ambiguous primitive.
    AxisAligned { a: Anchor, b: Anchor },
    /// The origin of anchor `b` lies on the axis of anchor `a`: 3 equations of rank 2, removing the
    /// 2 transverse translations and leaving travel along the axis free. Coaxial alignment is this
    /// plus `AxisAligned`.
    OnAxis { a: Anchor, b: Anchor },
    /// The origin of anchor `b` lies in the plane of anchor `a`, at `offset` along its axis:
    /// 1 equation, removing 1 translation along the normal.
    OnPlane { a: Anchor, b: Anchor, offset: f64 },
    /// The secondary axes point the same way: 3 equations of rank 1, removing rotation about the
    /// main axis. Required where a mate has to pin roll, as a rigid or slider mate does.
    RollAligned { a: Anchor, b: Anchor },
    /// The angle between the main axes equals the given value in degrees: 1 equation.
    Angle { a: Anchor, b: Anchor, deg: f64 },
    /// The distance between the anchor origins equals the given value: 1 equation.
    ///
    /// Required by sphere-to-sphere tangency: two spheres touch exactly when the distance between
    /// their centres equals the sum of the radii (outside) or their difference (one inside the
    /// other). Unlike distance-to-axis, a sphere has no axis at all, so what is held here is the
    /// length of the segment between two points, not a transverse component.
    PointDistance { a: Anchor, b: Anchor, dist: f64 },
    /// Drag target: a point of a body is led towards a point in world space. Contributes no
    /// equations at all — `rows()` is 0.
    ///
    /// This is not a document constraint but a goal that lives only while the pointer drags. The
    /// mechanism has to follow as a chain, through every free degree of every mate at once.
    ///
    /// A soft weighted goal was measured and rejected. Inside the shared least-squares problem the
    /// goal bargains with the constraints: the residual settles at a non-zero equilibrium, so
    /// `converged` is false even though the mates look satisfied, and the rule "no solution, no
    /// movement" then freezes the whole mechanism — zero motion on every drag step. No weight fixes
    /// that, because the bargaining happens inside the step rather than in the verdict.
    ///
    /// The goal therefore rides in the constraint list only as cargo: `bodies()` returns one body, so
    /// splitting the problem (`decompose`) puts the goal in the same part as the mates of the dragged
    /// body. The work is done by a separate null-space step (`pull_towards_cursor` in `iterate.rs`),
    /// the mirror of minimal displacement: where the constraints are indifferent, the body moves
    /// towards the cursor, so the constraints cannot be violated in principle.
    ///
    /// It is the only entry in the list without a pair of anchors: `pair()` answers `None` and
    /// `bodies()` returns a single body. Code that assumed every constraint is a pair used to trip
    /// over this.
    Pull { a: Anchor, to: nalgebra::Vector3<f64> },
    /// The distance from the origin of anchor `b` to the axis of anchor `a` equals the given value:
    /// 1 equation.
    ///
    /// Required by cylinder-to-cylinder tangency: two cylinders touch exactly when the distance
    /// between their axes equals the sum of the radii (outside) or their difference (one inside the
    /// other). `OnAxis` cannot express it: that one demands zero distance and holds the whole vector,
    /// whereas only the length is held here, leaving the cylinder free to roll around its neighbour.
    AxisDistance { a: Anchor, b: Anchor, dist: f64 },
    /// A relation between two degrees of freedom: `m1 = ratio * m2 + offset`, one equation.
    ///
    /// This constrains not bodies but two already measured degrees of two other mates: gear, rack and
    /// pinion, screw, linear. All four are the same equation with a different meaning for the
    /// coefficient — a ratio of angles for a gear, travel per revolution for a rack or a screw, a
    /// ratio of travels for a linear relation.
    ///
    /// Up to four bodies take part rather than two, since each measurement has its own pair of
    /// anchors. That is why the relation does not fit `pair()` and needs a separate branch wherever
    /// constraints are enumerated.
    ///
    /// `period` is the period of the residual; zero means aperiodic. An angle is circular: a gear
    /// turned by a full revolution stands where it stood. Without a period, a ratio of 2 would fall
    /// apart as soon as the driving wheel passed half a turn — the driven one would need 340 degrees
    /// while the measurement returned -20, and the solver would chase a solution that does not
    /// exist.
    SlotRatio { m1: SlotMeasure, m2: SlotMeasure, ratio: f64, offset: f64, period: f64 },
}

/// Denominator of `x` as an irreducible fraction `p/q`, if one is found below `max_q`.
///
/// A continued-fraction expansion rather than a search: for a ratio of 17/53 a search over
/// denominators finds the answer on step 53 while the expansion finds it on step 3, and the expansion
/// honestly reports "no fraction" for an irrational number instead of fitting a close one.
fn denominator_of(x: f64, max_q: u64, eps: f64) -> Option<u64> {
    let x = x.abs();
    if !x.is_finite() {
        return None;
    }
    let (mut h0, mut h1, mut k0, mut k1) = (0i128, 1i128, 1i128, 0i128);
    let mut v = x;
    for _ in 0..40 {
        let a = v.floor();
        if !a.is_finite() || a > 1e18 {
            break;
        }
        let (h2, k2) = (a as i128 * h1 + h0, a as i128 * k1 + k0);
        if k2 <= 0 || k2 as u64 > max_q {
            break;
        }
        (h0, h1, k0, k1) = (h1, h2, k1, k2);
        if (x - h1 as f64 / k1 as f64).abs() <= eps {
            return Some(k1 as u64);
        }
        let frac = v - a;
        if frac.abs() < 1e-15 {
            break;
        }
        v = 1.0 / frac;
    }
    None
}

/// Period of a relation residual: the smallest shift after which the bodies stand in the same place.
///
/// An angle is circular and travel is not, so the period follows from which measurements are
/// circular:
///
/// * both are travel — no period (zero): sliders do not come back "on the same turn";
/// * only the first is circular — the period is one full turn;
/// * only the second is circular — the period is `|k|` turns, which is the travel per revolution of
///   a rack or a screw;
/// * both are circular — the period is one turn divided by `q`, the denominator of the ratio. With a
///   ratio of 1:2 the driving wheel turned by half a revolution turns the driven one by a full one,
///   so the configuration repeats twice as often. Taking a whole turn here would break the relation
///   halfway through the first rotation.
///
/// An irrational ratio has no period at all, since the configuration never repeats; the honest
/// answer is then zero and the relation works within one measurable turn.
pub fn ratio_period(m1_rotation: bool, m2_rotation: bool, ratio: f64) -> f64 {
    let turn = std::f64::consts::TAU;
    match (m1_rotation, m2_rotation) {
        (false, false) => 0.0,
        (true, false) => turn,
        (false, true) => turn * ratio.abs(),
        (true, true) => denominator_of(ratio, 10_000, 1e-9).map(|q| turn / q as f64).unwrap_or(0.0),
    }
}

impl Constraint {
    /// A relation between two degrees, with the period computed here.
    ///
    /// A constructor rather than a field filled in by the caller: the period follows unambiguously
    /// from the kind of the measurements and the ratio, and omitting it yields a relation that breaks
    /// half a turn in. See `ratio_period`.
    pub fn slot_ratio(m1: SlotMeasure, m2: SlotMeasure, ratio: f64, offset: f64) -> Self {
        Constraint::SlotRatio { m1, m2, ratio, offset, period: ratio_period(m1.rotation, m2.rotation, ratio) }
    }

    /// How many equations a constraint contributes — not how many degrees of freedom it removes.
    /// Under redundancy there are more equations than removed degrees, and the difference is for the
    /// solver to find by rank, not for the author of the constraint to declare.
    ///
    /// Directions contribute three equations rather than two. Aligning axes removes two degrees, so
    /// two equations look sufficient: project the difference of the axes onto two perpendicular
    /// directions. That fails at a half-turn, where the difference is collinear with the axis itself,
    /// both projections vanish, and the solver sees a false solution with the body flipped. The whole
    /// difference (three components, rank two) has no such zero: at a half-turn its length is two. The
    /// dependent equation is removed by rank inside the solver.
    pub fn rows(&self) -> usize {
        match self {
            // The drag target contributes no equations: it is a goal, not a constraint. See `Constraint::Pull`.
            Constraint::Pull { .. } => 0,
            Constraint::PointCoincident { .. } => 3,
            Constraint::AxisAligned { .. } | Constraint::OnAxis { .. } | Constraint::RollAligned { .. } => 3,
            Constraint::OnPlane { .. } | Constraint::Angle { .. } | Constraint::SlotRatio { .. } | Constraint::AxisDistance { .. } | Constraint::PointDistance { .. } => 1,
        }
    }

    /// Both anchors of a constraint, in the order (a, b). A relation between degrees has no pair —
    /// it has four anchors — and the caller must handle it separately. `None` here replaces a trap:
    /// returning "the first two of four" would produce a constraint that the rest of the code
    /// silently treats as a pair.
    pub fn pair(&self) -> Option<(Anchor, Anchor)> {
        match *self {
            Constraint::PointCoincident { a, b }
            | Constraint::AxisAligned { a, b }
            | Constraint::OnAxis { a, b }
            | Constraint::OnPlane { a, b, .. }
            | Constraint::RollAligned { a, b }
            | Constraint::Angle { a, b, .. }
            | Constraint::AxisDistance { a, b, .. }
            | Constraint::PointDistance { a, b, .. } => Some((a, b)),
            Constraint::SlotRatio { .. } | Constraint::Pull { .. } => None,
        }
    }

    /// Bodies a constraint joins: two for the pairwise primitives, up to four for a relation.
    pub fn bodies(&self) -> Vec<BodyId> {
        match *self {
            Constraint::SlotRatio { m1, m2, .. } => vec![m1.a.body, m1.b.body, m2.a.body, m2.b.body],
            Constraint::Pull { a, .. } => vec![a.body], // the goal names one body and has no pair
            _ => {
                let (a, b) = self.pair().expect("a paired primitive");
                vec![a.body, b.body]
            }
        }
    }
}

/// The problem statement: bodies and the constraints between them.
#[derive(Clone, Debug, Default)]
pub struct Problem {
    pub bodies: Vec<Body>,
    pub constraints: Vec<Constraint>,
}

impl Problem {
    pub fn new(bodies: Vec<Body>) -> Self {
        Self { bodies, constraints: Vec::new() }
    }

    pub fn add(&mut self, c: Constraint) -> &mut Self {
        self.constraints.push(c);
        self
    }

    /// Total number of equations in the problem.
    pub fn rows(&self) -> usize {
        self.constraints.iter().map(|c| c.rows()).sum()
    }

    /// Total number of unknowns: 6 per ungrounded body (3 translations and 3 rotations).
    pub fn unknowns(&self) -> usize {
        self.bodies.iter().filter(|b| !b.grounded).count() * 6
    }

    /// Whether every constraint refers to a body that exists.
    ///
    /// Checked separately and up front: a broken reference inside the solve would become an
    /// out-of-bounds access or, worse, a silently skipped constraint.
    pub fn references_are_valid(&self) -> bool {
        let n = self.bodies.len();
        self.constraints.iter().all(|c| c.bodies().iter().all(|&b| b < n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Translation3, Vector3};

    fn at(x: f64) -> Pose {
        Pose::from_parts(Translation3::new(x, 0.0, 0.0), nalgebra::UnitQuaternion::identity())
    }

    #[test]
    fn unknowns_count_only_free_bodies() {
        let p = Problem::new(vec![Body::grounded(Pose::identity()), Body::new(at(10.0)), Body::new(at(20.0))]);
        assert_eq!(p.unknowns(), 12, "six unknowns per non-grounded body");
    }

    #[test]
    fn rows_match_the_declared_meaning_of_each_constraint() {
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        // The counts are not arbitrary: they follow from the meaning of each constraint.
        assert_eq!(Constraint::PointCoincident { a, b }.rows(), 3, "coincident points: three coordinates");
        assert_eq!(Constraint::AxisAligned { a, b }.rows(), 3, "the full axis difference: rank two, three components, otherwise 180 degrees gives a false zero");
        assert_eq!(Constraint::OnAxis { a, b }.rows(), 3, "the full transverse component: rank two");
        assert_eq!(Constraint::OnPlane { a, b, offset: 0.0 }.rows(), 1, "a point in a plane: one distance");
        assert_eq!(Constraint::RollAligned { a, b }.rows(), 3, "the full secondary axis difference: rank one");
        assert_eq!(Constraint::Angle { a, b, deg: 30.0 }.rows(), 1, "an angle: one equation");
    }

    #[test]
    fn broken_references_are_detected_before_solving() {
        let mut p = Problem::new(vec![Body::grounded(Pose::identity())]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(7) });
        assert!(!p.references_are_valid(), "a reference to a missing body must be caught before solving");
    }

    #[test]
    fn anchor_axes_survive_body_placement() {
        // An anchor lives in the local space of its body; its world axes must follow the body, or an
        // axis-alignment constraint compares things that are not comparable.
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).expect("axes");
        let turned = Pose::from_parts(
            Translation3::new(0.0, 0.0, 0.0),
            nalgebra::UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::FRAC_PI_2),
        );
        assert!((a.world_z(&turned) - (-Vector3::y())).norm() < 1e-12, "the main axis must rotate with the body");
    }
}
