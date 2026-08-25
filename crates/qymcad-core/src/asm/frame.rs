//! Pose and anchor: the coordinate system of a body and the frame a mate attaches to.
//!
//! A pose is an `Isometry3` (quaternion plus translation) rather than a 3x4 matrix. A matrix
//! accumulates non-orthogonality over solver iterations and has to be re-orthogonalised, and every
//! such repair displaces the body slightly. A quaternion normalises exactly and cheaply, and a
//! half-turn is not a discontinuity for it.

use nalgebra::{Isometry3, Translation3, UnitQuaternion, Vector3};

/// Pose of a body in world space, or of an anchor in the local space of its body.
pub type Pose = Isometry3<f64>;

/// A mate anchor: a full coordinate system on a body, not a reference to a face.
///
/// A mate aligns two coordinate systems; the joint kind only says which degrees of freedom stay
/// free. Anything less than a full frame leaves the resulting placement undefined — deriving the
/// secondary axis from a world axis makes a body settle into an arbitrary roll relative to its own
/// shape.
///
/// The anchor is plain data: a local pose on a body. Where that pose came from (face, edge, vertex)
/// belongs to the layer above; the solver only needs the pose itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// Body the anchor belongs to.
    pub body: usize,
    /// Pose of the anchor in the local space of that body.
    pub local: Pose,
    /// Whether roll is defined, that is, whether the secondary axis was derived from part geometry.
    ///
    /// When it was not — geometry was insufficient, or the anchor sits on a point — rotation about
    /// the main axis is not determined by anything. Pinning it would be a guess and would settle the
    /// body into an arbitrary roll, so the degree is left free and reported as free instead.
    pub roll_known: bool,
}

impl Anchor {
    pub fn new(body: usize, local: Pose) -> Self {
        Self { body, local, roll_known: true }
    }

    /// Anchor whose secondary axis was not derived from geometry: roll is undefined.
    pub fn without_roll(body: usize, local: Pose) -> Self {
        Self { body, local, roll_known: false }
    }

    /// Anchor at the origin of the body, axes aligned with the world.
    pub fn origin(body: usize) -> Self {
        Self { body, local: Pose::identity(), roll_known: true }
    }

    /// Anchor from an origin and a main axis (Z), with the secondary axis (X) given explicitly.
    ///
    /// The secondary axis must come from part geometry. Derived from world axes it is inconsistent
    /// between two parts, and aligning the frames then rotates a body by an arbitrary angle about
    /// the shared normal.
    ///
    /// `x_hint` is orthogonalised against `z` (its component along `z` is dropped). A hint parallel
    /// to `z`, or a degenerate one, yields `None` rather than a silently chosen perpendicular: the
    /// caller has to know that rotation about the axis is undefined.
    pub fn from_axes(body: usize, origin: Vector3<f64>, z: Vector3<f64>, x_hint: Vector3<f64>) -> Option<Self> {
        let z = z.try_normalize(1e-12)?;
        let x = {
            let proj = x_hint - z * z.dot(&x_hint);
            proj.try_normalize(1e-9)?
        };
        let y = z.cross(&x);
        let rot = UnitQuaternion::from_basis_unchecked(&[x, y, z]);
        Some(Self { body, local: Isometry3::from_parts(Translation3::from(origin), rot), roll_known: true })
    }

    /// Origin of the anchor in world space for a given body pose.
    pub fn world_origin(&self, body_pose: &Pose) -> Vector3<f64> {
        (body_pose * self.local).translation.vector
    }

    /// Main axis (Z) of the anchor in world space.
    pub fn world_z(&self, body_pose: &Pose) -> Vector3<f64> {
        (body_pose * self.local).rotation * Vector3::z()
    }

    /// Secondary axis (X) of the anchor in world space.
    pub fn world_x(&self, body_pose: &Pose) -> Vector3<f64> {
        (body_pose * self.local).rotation * Vector3::x()
    }

    /// Full pose of the anchor in world space.
    pub fn world(&self, body_pose: &Pose) -> Pose {
        body_pose * self.local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_are_orthonormal_and_right_handed() {
        let a = Anchor::from_axes(0, Vector3::new(1.0, 2.0, 3.0), Vector3::new(0.0, 0.0, 2.0), Vector3::new(3.0, 0.0, 0.0)).expect("axes are given");
        let p = Pose::identity();
        let (x, z) = (a.world_x(&p), a.world_z(&p));
        let y = z.cross(&x);
        assert!((x.norm() - 1.0).abs() < 1e-12 && (z.norm() - 1.0).abs() < 1e-12, "axes must be unit length");
        assert!(x.dot(&z).abs() < 1e-12, "axes must be orthogonal");
        assert!((x.cross(&y) - z).norm() < 1e-12, "the basis must be right-handed");
        assert!((a.world_origin(&p) - Vector3::new(1.0, 2.0, 3.0)).norm() < 1e-12, "the origin must not shift");
    }

    #[test]
    fn secondary_axis_is_orthogonalised_not_taken_as_is() {
        // The hint is not perpendicular to the main axis: its longitudinal part must be dropped
        // rather than used to build a skewed basis.
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::new(1.0, 0.0, 5.0)).expect("axes are given");
        let p = Pose::identity();
        assert!((a.world_x(&p) - Vector3::x()).norm() < 1e-12, "X must land in the plane perpendicular to Z");
    }

    #[test]
    fn degenerate_hint_is_reported_not_silently_guessed() {
        // The hint is parallel to the main axis, so rotation about it is undefined. Silently
        // picking "some" perpendicular puts the body into an arbitrary roll.
        assert!(Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::z()).is_none(), "a degenerate hint must be rejected");
        assert!(Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::zeros()).is_none(), "a zero hint must be rejected");
        assert!(Anchor::from_axes(0, Vector3::zeros(), Vector3::zeros(), Vector3::x()).is_none(), "a zero main axis must be rejected");
    }

    #[test]
    fn anchor_follows_its_body() {
        let a = Anchor::from_axes(0, Vector3::new(10.0, 0.0, 0.0), Vector3::z(), Vector3::x()).expect("axes");
        let body = Isometry3::from_parts(
            Translation3::new(0.0, 5.0, 0.0),
            UnitQuaternion::from_axis_angle(&Vector3::z_axis(), std::f64::consts::FRAC_PI_2),
        );
        // Body turned 90 degrees about Z: an anchor at (10,0,0) moves to (0,10,0), plus the (0,5,0) translation.
        let o = a.world_origin(&body);
        assert!((o - Vector3::new(0.0, 15.0, 0.0)).norm() < 1e-12, "the anchor must travel with its body instead of staying in local space: {o:?}");
        assert!((a.world_x(&body) - Vector3::y()).norm() < 1e-12, "anchor axes must rotate with the body");
    }
}
