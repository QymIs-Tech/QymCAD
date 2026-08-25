//! The anchor as an object: where exactly it sits on a body and where its axes point.
//!
//! While an anchor is a reference to a face, with its frame derived on the fly from that face and the
//! world axes, the question "where will the body land" has no definite answer and the body lands
//! arbitrarily. An anchor has to be a full coordinate system that is visible and adjustable, with a
//! mate merely aligning two such systems.
//!
//! Three things follow from that:
//!
//! 1. **Attach point.** A cylindrical face has three: the middle and both ends. This settles the case
//!    of a revolute pair through two holes of different length, 10 mm and 140 mm: taking only the
//!    middles moves the body by the difference, because the middles are apart. Which end to use is the
//!    user's choice.
//! 2. **Secondary axis from geometry**, not from the world Z, otherwise rotation about the main axis is
//!    arbitrary relative to the shape of the body.
//! 3. **An explicit "unknown".** When there is nothing to derive the secondary axis from, the anchor
//!    records that, and the mate then leaves roll unpinned instead of inventing it.

use serde::{Deserialize, Serialize};


/// Attach point on the selected geometry.
///
/// Pointing at a face offers a centre, a vertex, an edge midpoint, or — on a cylinder — a point on the
/// axis at the middle or at either end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachPoint {
    /// Middle: the face centroid, the edge midpoint, or the mid-length of a cylinder.
    #[default]
    Middle,
    /// Start end: for a cylinder or an edge, the end nearer the origin of the axis.
    Start,
    /// Far end.
    End,
}

impl AttachPoint {
    /// Offset of the point along the axis, given the extent `[lo, hi]` of the geometry along it.
    ///
    /// Returns the coordinate along the axis relative to the centre: zero for the middle, half the
    /// extent with a sign for the ends. This is the quantity that pushes bodies apart when only the
    /// middle is available.
    pub fn along(self, lo: f64, hi: f64) -> f64 {
        match self {
            AttachPoint::Middle => 0.0,
            AttachPoint::Start => lo - 0.5 * (lo + hi),
            AttachPoint::End => hi - 0.5 * (lo + hi),
        }
    }

    /// Human-readable name for the interface.
    ///
    /// A catalogue key rather than a word: the core knows no language.
    pub fn label(self) -> &'static str {
        match self {
            AttachPoint::Middle => "conn-middle",
            AttachPoint::Start => "conn-start",
            AttachPoint::End => "conn-end",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_point_places_the_origin_at_the_chosen_end() {
        // A hole from z=0 to z=140: the middle is at 70, the ends at 0 and 140.
        assert_eq!(AttachPoint::Middle.along(0.0, 140.0), 0.0, "the middle is zero relative to the centre");
        assert_eq!(AttachPoint::Start.along(0.0, 140.0), -70.0, "the start end is half the extent back");
        assert_eq!(AttachPoint::End.along(0.0, 140.0), 70.0, "the far end is half the extent forward");
    }

    /// This is what pushes bodies apart: the middles of holes of different length do not coincide.
    #[test]
    fn middles_of_holes_of_different_length_are_apart_but_ends_are_not() {
        // A short hole 0..10 and a long one 0..140, both bodies placed by their ends at z=0.
        let short_mid: f64 = 0.5 * (0.0 + 10.0);
        let long_mid: f64 = 0.5 * (0.0 + 140.0);
        assert!((short_mid - long_mid).abs() > 60.0, "the middles are tens of millimetres apart, which is where the displacement comes from");
        // The ends do coincide: both attachments give the same world point.
        let short_start = short_mid + AttachPoint::Start.along(0.0, 10.0);
        let long_start = long_mid + AttachPoint::Start.along(0.0, 140.0);
        assert!((short_start - long_start).abs() < 1e-12, "by their ends the bodies meet exactly, which is what the ends are for");
    }
}
