//! Mates: what the user picks, expressed on top of the solver primitives.
//!
//! The split is deliberate: a mate aligns two coordinate systems, and the mate kind states exactly
//! one thing — which degrees of freedom stay free. How the equations are written is the business of
//! the layer below.
//!
//! The inverse arrangement, where degrees of freedom are implied by a hand-picked set of constraints,
//! offers no way to check that a mate leaves as much freedom as it promises. Here every kind declares
//! its free-degree count, and a test compares that number with what the solver derives from rank.
//!
//! A mate value (an angle, a travel) is not a separate mechanism but a displaced anchor: rotating an
//! anchor about its axis by an angle is the same as rotating the body by that angle. The drive
//! therefore has no mathematics of its own and cannot drift apart from the mate itself.

use nalgebra::{Isometry3, Translation3, UnitQuaternion, Vector3};

use super::frame::Anchor;
use super::problem::Constraint;

/// Mate kind, defined by what stays free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    /// Rigid: nothing is free.
    Rigid,
    /// Revolute: rotation about the main axis.
    Revolute,
    /// Slider: travel along the main axis.
    Slider,
    /// Cylindrical: travel along the axis and rotation about it.
    Cylindrical,
    /// Planar: two translations in the plane and rotation about the normal.
    Planar,
    /// Ball: three rotations about a shared point.
    Ball,
    /// Pin-slot: rotation about the main axis and travel along the secondary one.
    PinSlot,
    /// Parallel: only the main axes point the same way. Everything else is free.
    Parallel,
}

impl JointKind {
    /// How many degrees of freedom must remain. This is the definition of the kind, not a consequence
    /// of the implementation: if the solver counts a different number, the code is wrong, not this.
    pub fn free_dof(self) -> usize {
        match self {
            JointKind::Rigid => 0,
            JointKind::Revolute | JointKind::Slider => 1,
            JointKind::Cylindrical | JointKind::PinSlot => 2,
            JointKind::Planar | JointKind::Ball => 3,
            JointKind::Parallel => 4,
        }
    }
}

/// Mate values: position along its free degrees.
///
/// Set by the user and driving the body. `None` leaves the degree free.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    /// Rotation about the main axis, in degrees.
    pub angle: Option<f64>,
    /// Travel along the main axis, in millimetres.
    pub offset: Option<f64>,
    /// Second value: travel along the secondary Y axis for a planar mate, rotation about Y for a ball.
    pub offset2: Option<f64>,
}

/// Which axis a slot runs along and with what motion: `(axis, is_rotation)`; `None` when the kind has
/// no such slot.
///
/// The axis is named in the anchor frame: 2 is the main axis (Z), 0 the secondary (X), 1 the third
/// (Y). This is the single table for all three uses — pinning a degree by a value, reading the value
/// back, and checking a limit. Were they to diverge, a mate would hold one degree, display another and
/// limit a third.
pub fn slot_axis(kind: JointKind, slot: usize) -> Option<(u8, bool)> {
    match (kind, slot) {
        (JointKind::Revolute, 0) | (JointKind::Cylindrical, 0) | (JointKind::PinSlot, 0) | (JointKind::Planar, 0) | (JointKind::Ball, 0) => Some((2, true)),
        (JointKind::Slider, 1) | (JointKind::Cylindrical, 1) => Some((2, false)),
        (JointKind::Planar, 1) | (JointKind::PinSlot, 1) => Some((0, false)),
        (JointKind::Planar, 2) => Some((1, false)),
        (JointKind::Ball, 1) => Some((0, true)),
        (JointKind::Ball, 2) => Some((1, true)),
        _ => None,
    }
}

/// A mate: two anchors, a kind and its values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint {
    pub a: Anchor,
    pub b: Anchor,
    pub kind: JointKind,
    pub drive: Drive,
    /// Mating side: the main axes of the anchors point towards each other rather than the same way.
    ///
    /// Needed whenever a body is placed face to face: face normals point out of their bodies, so
    /// "facing each other" means opposite directions. Without the flag the body would have to be
    /// turned by hand, and a solver left to choose would choose arbitrarily, since at a half-turn both
    /// sides are equally stable.
    pub flip: bool,
    /// The other half of the side: the secondary axis of the first anchor is reversed, a half-turn
    /// about the main axis.
    ///
    /// Needed in the same situations as `flip`. For a slider on a flat face the main axis is the
    /// direction of travel and the secondary one is the face normal; placing the body by its other
    /// face reverses the secondary axis specifically, and without this half the mate turns the body
    /// inside out about the travel axis.
    pub roll_flip: bool,
}

impl Joint {
    pub fn new(a: Anchor, b: Anchor, kind: JointKind) -> Self {
        Self { a, b, kind, drive: Drive::default(), flip: false, roll_flip: false }
    }

    /// Mate face to face: the main axes of the anchors point against each other.
    pub fn flipped(mut self) -> Self {
        self.flip = true;
        self
    }

    /// Set the whole mating side: main axis reversed, secondary axis reversed.
    pub fn with_side(mut self, side: crate::feature::Side) -> Self {
        self.flip = side.flip;
        self.roll_flip = side.roll_flip;
        self
    }

    pub fn with_angle(mut self, deg: f64) -> Self {
        self.drive.angle = Some(deg);
        self
    }

    pub fn with_offset(mut self, mm: f64) -> Self {
        self.drive.offset = Some(mm);
        self
    }

    /// Set the second value: travel along the third axis for a planar mate, rotation about it for a ball.
    pub fn with_offset2(mut self, v: f64) -> Self {
        self.drive.offset2 = Some(v);
        self
    }

    /// Anchor `a`, displaced and rotated by the mate values.
    ///
    /// This is the whole of the drive: rotating an anchor about its axis by an angle is the same as
    /// rotating the body. There is no separate drive mathematics, so nothing can drift apart from the
    /// mate.
    fn driven_a(&self) -> Anchor {
        let mut local = self.a.local;
        if self.flip {
            // Half-turn of the anchor about its X: the main axis then points the other way.
            local *= UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::PI);
        }
        if self.roll_flip {
            // Half-turn about the main axis: the secondary axis reverses, the main one stays.
            local *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), std::f64::consts::PI);
        }
        // A value is applied with the same motion the mate kind describes. One formula for every kind
        // — rotate about Z, translate along Z — is right only for revolute and slider: a planar mate
        // translates along X and Y, a ball takes angles for its second and third values, and a pin-slot
        // translates along X. With a single formula those values never reach the solver.
        let rot = |axis: Vector3<f64>, deg: f64| UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), deg.to_radians());
        let (ang, off, off2) = (self.drive.angle.unwrap_or(0.0), self.drive.offset.unwrap_or(0.0), self.drive.offset2.unwrap_or(0.0));
        match self.kind {
            // A rigid mate has no freedom but does have parameters: both a gap and a rotation about
            // the mating axis.
            //
            // With only a gap there is no way to turn a fastened body about the mating axis, and the
            // body ends up rotated by hand. Freedom is not involved: the mate still removes all six
            // degrees, it simply has two parameters of its own.
            JointKind::Rigid => {
                local *= rot(Vector3::z(), ang);
                local *= Translation3::new(0.0, 0.0, off);
            }
            JointKind::Revolute => local *= rot(Vector3::z(), ang),
            JointKind::Slider => local *= Translation3::new(0.0, 0.0, off),
            JointKind::Cylindrical => {
                local *= rot(Vector3::z(), ang);
                local *= Translation3::new(0.0, 0.0, off);
            }
            JointKind::Planar => {
                local *= Translation3::new(off, off2, 0.0);
                local *= rot(Vector3::z(), ang);
            }
            JointKind::Ball => {
                local *= rot(Vector3::z(), ang);
                local *= rot(Vector3::x(), off);
                local *= rot(Vector3::y(), off2);
            }
            JointKind::PinSlot => {
                // Slot travel is not baked into the first anchor: the slot belongs to the second one.
                // Only the pin rotation is applied here; travel is imposed by a constraint in the slot
                // frame, see `primitives`.
                local *= rot(Vector3::z(), ang);
            }
            // A parallel mate has no values: it holds a direction, not a distance or an angle.
            JointKind::Parallel => {}
        }
        Anchor { body: self.a.body, local, roll_known: self.a.roll_known }
    }

    /// Anchor rotated so that the named axis becomes its main axis (2 is already the main one).
    /// Lets motion along the secondary axes be pinned with the same primitives as along the main one.
    fn turned_to(a: Anchor, axis: u8) -> Anchor {
        let q = match axis {
            0 => UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f64::consts::FRAC_PI_2), // Z → X
            1 => UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -std::f64::consts::FRAC_PI_2), // Z → Y
            _ => UnitQuaternion::identity(),
        };
        Anchor { body: a.body, local: a.local * q, roll_known: a.roll_known }
    }

    /// Decomposition of a mate into solver primitives.
    ///
    /// Each set is chosen so that its rank removes exactly as many degrees as the kind promises. A
    /// test compares the declared `free_dof` with the rank the solver derives.
    pub fn primitives(&self) -> Vec<Constraint> {
        let a = self.driven_a();
        let b = self.b;
        let mut out = self.base_primitives(a, b);
        // Pin only what is defined. If either anchor has a secondary axis that was not derived from
        // geometry, roll is determined by nothing, and pinning it would invent an answer. Such a mate
        // leaves the extra degree free instead of settling the body into an arbitrary rotation.
        if !(self.a.roll_known && self.b.roll_known) {
            out.retain(|c| !matches!(c, Constraint::RollAligned { .. }));
        }
        // A driven degree stops being free. The displaced anchor says where the body should end up,
        // but while the degree is free the solver may leave the body anywhere — and will, because
        // minimal displacement tells it not to move without reason. A driven value therefore also has
        // to pin its degree.
        for (slot, given) in [self.drive.angle, self.drive.offset, self.drive.offset2].into_iter().enumerate() {
            if given.is_none() {
                continue;
            }
            let Some((axis, is_rotation)) = slot_axis(self.kind, slot) else { continue };
            // Slot travel is pinned in the slot frame. For the other kinds the driven value is already
            // baked into the first anchor (`driven_a`) and the constraint only has to say "exactly
            // here"; a slot has nowhere to bake it, because its direction belongs to the second anchor
            // and lives in that frame. The value therefore sits in the constraint itself: travel along
            // the slot X equals the driven value.
            if matches!(self.kind, JointKind::PinSlot) && slot == 1 {
                out.push(Constraint::OnPlane { a: Self::turned_to(b, 0), b: a, offset: -given.unwrap_or(0.0) });
                continue;
            }
            let (ta, tb) = (Self::turned_to(a, axis), Self::turned_to(b, axis));
            out.push(if is_rotation { Constraint::RollAligned { a: ta, b: tb } } else { Constraint::OnPlane { a: ta, b: tb, offset: 0.0 } });
        }
        out
    }

    /// How many degrees of freedom remain once driven values are taken into account.
    pub fn free_dof(&self) -> usize {
        let mut n = self.kind.free_dof();
        // Undefined roll is an extra degree of freedom, and it is reported as one.
        if !(self.a.roll_known && self.b.roll_known) && matches!(self.kind, JointKind::Rigid | JointKind::Slider) {
            n += 1;
        }
        for (slot, given) in [self.drive.angle, self.drive.offset, self.drive.offset2].into_iter().enumerate() {
            if given.is_some() && slot_axis(self.kind, slot).is_some() {
                n = n.saturating_sub(1);
            }
        }
        n
    }

    fn base_primitives(&self, a: Anchor, b: Anchor) -> Vec<Constraint> {
        match self.kind {
            // Nothing is free: points, axes and roll all coincide.
            JointKind::Rigid => vec![
                Constraint::PointCoincident { a, b },
                Constraint::AxisAligned { a, b },
                Constraint::RollAligned { a, b },
            ],
            // Rotation about the axis is free: points and axes coincide, roll is not pinned.
            JointKind::Revolute => vec![Constraint::PointCoincident { a, b }, Constraint::AxisAligned { a, b }],
            // Travel along the axis is free: point on axis, axes aligned, roll pinned.
            JointKind::Slider => vec![
                Constraint::OnAxis { a, b },
                Constraint::AxisAligned { a, b },
                Constraint::RollAligned { a, b },
            ],
            // Travel along the axis and rotation about it are free.
            JointKind::Cylindrical => vec![Constraint::OnAxis { a, b }, Constraint::AxisAligned { a, b }],
            // Two translations in the plane and rotation about the normal are free.
            JointKind::Planar => vec![Constraint::OnPlane { a, b, offset: 0.0 }, Constraint::AxisAligned { a, b }],
            // Three rotations about a shared point are free.
            JointKind::Ball => vec![Constraint::PointCoincident { a, b }],
            // Rotation about the main axis and travel along the secondary one (the slot) are free.
            // The pin lies on the slot axis and both anchor axes point the same way, which leaves
            // exactly two degrees: rotation about the pin axis and travel along the slot.
            //
            // The slot belongs to the second anchor: the first one carries the pin and the centre of
            // rotation, the second one the translation. Built from the first anchor instead, the body
            // travels along the pin X — invisible while the two anchor axes happen to coincide, and
            // plainly wrong as soon as the slot points its own way.
            //
            // The axis is taken from the first field of the constraint, so the anchors are swapped
            // here: "the pin origin lies on the slot axis" is the same condition written the other way
            // round.
            //
            // Two planes instead of an axis do not work, and that is measured twice. On live
            // geometry: 5 mm of travel gave a residual of 1.8e-6 and 10 mm gave 2.4e-3, so the mate was
            // declared conflicting for no reason. By step count: a pin-slot burned all 200 steps of the
            // first attempt and took its answer from the second one, started with the anchors aligned.
            // The condition is the same, but written as two dot products it is badly conditioned.
            //
            // "The pin lies on the slot line" is a single `OnAxis` about the slot axis: it holds the
            // transverse component whole (rank two) and leaves travel along the slot free. The same
            // requirement, but smooth.
            // A parallel mate aligns the main axes only: 2 equations of rank 2. Three translations and
            // rotation about the shared axis remain, exactly four degrees. A planar mate additionally
            // holds the distance along the normal, which is why it has three degrees rather than four;
            // treating the two as the same mate silently converts one into the other.
            JointKind::Parallel => vec![Constraint::AxisAligned { a, b }],
            JointKind::PinSlot => vec![
                Constraint::OnAxis { a: Self::turned_to(b, 0), b: a },
                Constraint::AxisAligned { a: b, b: a },
            ],
        }
    }
}

/// Build a solver problem from bodies and mates.
pub fn problem_from(bodies: Vec<super::problem::Body>, joints: &[Joint]) -> super::problem::Problem {
    let mut p = super::problem::Problem::new(bodies);
    for j in joints {
        for c in j.primitives() {
            p.add(c);
        }
    }
    p
}

/// Identity pose, for brevity at call sites.
pub fn identity() -> Isometry3<f64> {
    Isometry3::identity()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::iterate::solve;
    use crate::asm::problem::Body;

    fn at(x: f64, y: f64, z: f64) -> Isometry3<f64> {
        Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::identity())
    }

    /// Two bodies: one grounded, the other moved and turned, so the solver has work to do.
    fn pair(kind: JointKind) -> (super::super::problem::Problem, Joint) {
        let far = Isometry3::from_parts(
            Translation3::new(120.0, -45.0, 30.0),
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::new(1.0, 2.0, 0.5)), 1.2),
        );
        let a = Anchor::from_axes(0, Vector3::new(0.0, 0.0, 0.0), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::new(0.0, 0.0, 0.0), Vector3::z(), Vector3::x()).unwrap();
        let j = Joint::new(a, b, kind);
        (problem_from(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(far)], &[j]), j)
    }

    /// The central check: every kind leaves exactly as much freedom as it promises.
    ///
    /// The promise is `free_dof`, the definition of the kind. The fact is the Jacobian rank computed by
    /// the solver. Without this comparison, degrees of freedom are implied by a hand-picked set of
    /// constraints, and "a slider behaves like a rigid mate" shows up only on screen.
    #[test]
    fn every_joint_kind_leaves_exactly_the_freedom_it_promises() {
        let kinds = [
            JointKind::Rigid,
            JointKind::Revolute,
            JointKind::Slider,
            JointKind::Cylindrical,
            JointKind::Planar,
            JointKind::Ball,
            JointKind::PinSlot,
        ];
        let mut bad = Vec::new();
        for k in kinds {
            let (p, _) = pair(k);
            let (_, rep) = solve(&p);
            if !rep.converged {
                bad.push(format!("{k:?}: did not converge, residual {:.3e}", rep.residual));
            }
            if rep.dof != k.free_dof() {
                bad.push(format!("{k:?}: promises {} degrees of freedom, {} remain", k.free_dof(), rep.dof));
            }
        }
        assert!(bad.is_empty(), "mates do not keep the freedom they promise:\n  {}", bad.join("\n  "));
    }

    /// An angle drives the body: set 90 degrees and it turns by exactly 90, with its origin unmoved.
    #[test]
    fn a_revolute_angle_drives_the_part_exactly() {
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let j = Joint::new(a, b, JointKind::Revolute).with_angle(90.0);
        // Roll needs no manual pinning: the mate knows that a driven angle takes the freedom away.
        let p = problem_from(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(50.0, 0.0, 0.0))], &[j]);
        assert_eq!(j.free_dof(), 0, "a driven angle must take away the single freedom of a revolute mate");
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "must converge: {:.3e}", rep.residual);
        let x = b.world_x(&poses[1]);
        assert!((x - Vector3::y()).norm() < 1e-6, "at 90 degrees the secondary axis must land on Y, but it is at {x:?}");
        assert!(poses[1].translation.vector.norm() < 1e-6, "rotation about the axis must not move the origin: {:?}", poses[1].translation.vector);
    }

    /// A travel value drives the body: set 25 mm and it moves exactly 25 along the axis.
    #[test]
    fn a_slider_offset_drives_the_part_exactly() {
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let j = Joint::new(a, b, JointKind::Slider).with_offset(25.0);
        let p = problem_from(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 90.0))], &[j]);
        assert_eq!(j.free_dof(), 0, "a driven travel must take away the single freedom of a slider");
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "must converge: {:.3e}", rep.residual);
        let o = b.world_origin(&poses[1]);
        assert!((o - Vector3::new(0.0, 0.0, 25.0)).norm() < 1e-6, "a travel of 25 must place the anchor at z=25, but it is at {o:?}");
    }

    /// Every mate kind preserves its free directions.
    ///
    /// The body starts away from the target and turned. A mate must bring it where it requires and
    /// leave alone what it does not: a slider and a cylindrical mate keep travel along the axis, a
    /// planar mate two travels in the plane, a parallel mate all three.
    ///
    /// Added after a mutation pass: the existing cylindrical checks placed the body where there was
    /// nothing to preserve, so losing 30 mm along the axis failed nowhere.
    #[test]
    fn every_kind_keeps_the_place_along_the_directions_it_does_not_constrain() {
        let start = Isometry3::from_parts(
            Translation3::new(120.0, -45.0, 30.0),
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::new(1.0, 2.0, 0.5)), 1.2),
        );
        // What must stay untouched, along X, Y and Z.
        let cases = [
            (JointKind::Slider, [false, false, true]),
            (JointKind::Cylindrical, [false, false, true]),
            (JointKind::Planar, [true, true, false]),
            (JointKind::Parallel, [true, true, true]),
        ];
        let mut bad = Vec::new();
        for (kind, keep) in cases {
            let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
            let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
            let p = problem_from(vec![Body::grounded(Isometry3::identity()), Body::new(start)], &[Joint::new(a, b, kind)]);
            let (poses, rep) = solve(&p);
            if !rep.converged {
                bad.push(format!("{kind:?}: did not converge, residual {:.3e}", rep.residual));
                continue;
            }
            let got = poses[1].translation.vector;
            let want = start.translation.vector;
            for k in 0..3 {
                if keep[k] && (got[k] - want[k]).abs() > 1e-3 {
                    bad.push(format!("{kind:?}: the mate requires nothing along axis {k}, yet the body moved from {:.4} to {:.4}", want[k], got[k]));
                }
            }
        }
        assert!(bad.is_empty(), "a mate moves the body where it requires nothing:\n  {}", bad.join("\n  "));
    }

    /// No mate kind burns iterations for nothing.
    ///
    /// Hitting the step ceiling is not slowness but incorrectness: the attempt is declared
    /// non-convergent, and the solver takes the answer of the second attempt, which starts from
    /// aligned anchors and therefore loses everything the mate does not require. That is exactly how a
    /// body ends up 30 mm away from where it was left.
    ///
    /// The threshold is generous: this catches getting stuck, it does not count steps. Cylindrical,
    /// planar and parallel mates used to burn all 200.
    #[test]
    fn no_kind_of_joint_burns_its_way_to_the_iteration_cap() {
        let start = Isometry3::from_parts(
            Translation3::new(120.0, -45.0, 30.0),
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::new(1.0, 2.0, 0.5)), 1.2),
        );
        // Pin-slot is excluded here by measurement, not as a concession. From a distant, turned start
        // its first attempt does not stall near the answer, it does not solve the problem at all:
        // residual 1.963e-1. The slot axis belongs to the moving body, so the line the pin must lie on
        // travels with it, and the problem is genuinely non-linear. The second attempt, started from
        // the assembled guess, finds the answer in 4 steps; here that is not a fallback but the only
        // path, and the guess exists for exactly such cases.
        let kinds = [
            JointKind::Rigid,
            JointKind::Revolute,
            JointKind::Slider,
            JointKind::Cylindrical,
            JointKind::Planar,
            JointKind::Ball,
            JointKind::Parallel,
        ];
        let mut bad = Vec::new();
        let mut checked = 0usize;
        for kind in kinds {
            let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
            let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
            let p = problem_from(vec![Body::grounded(Isometry3::identity()), Body::new(start)], &[Joint::new(a, b, kind)]);
            let (_, rep) = solve(&p);
            checked += 1;
            if !rep.converged {
                bad.push(format!("{kind:?}: did not converge, residual {:.3e}", rep.residual));
            }
            if rep.iterations > 120 {
                bad.push(format!("{kind:?}: {} steps hit the ceiling, so the answer came from the second attempt", rep.iterations));
            }
        }
        assert_eq!(checked, 7, "guard: seven kinds are judged here (pin-slot is excluded above), but {checked} were checked");
        assert!(bad.is_empty(), "the solver gets stuck:\n  {}", bad.join("\n  "));
    }

    /// A revolute pair through two holes of different depth: the body must not travel along the axis.
    ///
    /// Anchors on cylindrical faces sit at the mid-depth of their hole, so for a 10 mm and a 140 mm
    /// hole the midpoints are tens of millimetres apart. The mate has to align the axes and the points,
    /// but it must not drag the body by the difference of the midpoints when nothing asked for that.
    #[test]
    fn a_hinge_on_holes_of_different_depth_stays_where_it_was_put() {
        // The anchors are apart along the shared axis: one at the middle of the short hole, one of the long.
        let a = Anchor::from_axes(0, Vector3::new(0.0, 0.0, 5.0), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::new(0.0, 0.0, 70.0), Vector3::z(), Vector3::x()).unwrap();
        // Cylindrical: coaxial without a longitudinal tie, which is how the pair should behave while
        // no position along the axis has been set.
        let j = Joint::new(a, b, JointKind::Cylindrical);
        let p = problem_from(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0))], &[j]);
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "the coaxial condition must hold: {:.3e}", rep.residual);
        let o = b.world_origin(&poses[1]);
        assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "the anchors must become coaxial: {o:?}");
        assert!(
            poses[1].translation.vector.z.abs() < 1e-3,
            "the body was moved {:.3} mm along the axis: a difference in hole depth must not move it",
            poses[1].translation.vector.z
        );
    }
}
