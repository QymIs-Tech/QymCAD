//! THE GRAB RADIUS FOLLOWS THE ROLE OF THE TARGET, NOT THE PLACE IN THE CODE.
//!
//! There were 42 hard-wired thresholds in `pick.rs` and `sketching.rs`. The plan called that
//! "thresholds differing for no reason" — a census showed that half of it is not so, and two different
//! kinds of disorder have to be told apart.
//!
//! **Between roles the difference is LEGITIMATE.** Aiming at a point is harder than aiming at a line:
//! a point occupies a pixel, a line stretches across half the screen. Giving them one radius would
//! either make points uncatchable or make lines steal clicks from their neighbours. Mature CAD systems
//! work the same way.
//!
//! **Within a role the difference is PURE ARBITRARINESS.** The same vertex was caught from 8, 9, 10,
//! 13 and 18 pixels in six different functions; an edge from 7, 8 and 10 in seven. A user cannot
//! explain that, because there is no explanation: the numbers were typed in place and from memory.
//!
//! So there are ROLES here rather than one number. A role names WHAT is being caught, and how many
//! pixels that is comes from one table and one precision multiplier.
use super::App;

/// What exactly the cursor catches. The grab radius depends on the role.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Grab {
    /// A point, a vertex, a centre: a target the size of a pixel, aimed at by eye.
    Point,
    /// A line, an arc, an edge, a contour, an axis: an extended target, caught across.
    Curve,
    /// A label: a dimension, a joint glyph, a diameter or radius mark. Text, and missing it is the most irritating.
    Label,
    /// A guide: a sketch axis, the band of a dimension extension line. Narrow but endless.
    Guide,
    /// A SNAP while drawing is not a selection but an attraction of the cursor. Its radius is its
    /// own: too generous and it steals the freedom to place a point near a node, too mean and the snap
    /// is useless.
    Snap,
}

impl Grab {
    /// The base radius in pixels at the "normal" precision.
    ///
    /// The numbers are the MEDIAN of what stood in place before the consolidation (a point at
    /// 8/9/9/10/11/13/18, an edge at 7/7/8/8/8/8/8/10/10, and so on). The median was taken rather than
    /// the mean or the maximum: that way the behaviour in most places does not change at all, and
    /// changes only where it was an outlier.
    pub(crate) fn base(self) -> f32 {
        match self {
            Grab::Point => 10.0,
            Grab::Curve => 8.0,
            Grab::Label => 12.0,
            Grab::Guide => 6.0,
            Grab::Snap => 9.0,
        }
    }
}

/// The pick precision multiplier: 0 is precise, 1 is normal, 2 is coarse.
///
/// One number for all the roles: a person says "this is too small for me", not "this is too small for
/// me on edges". The coarse mode is about touch screens and about 4K, where everything is twice as
/// small in the viewer's pixels.
pub(crate) fn precision_factor(level: u8) -> f32 {
    match level {
        0 => 0.7,
        2 => 1.5,
        _ => 1.0,
    }
}

impl App {
    /// THE GRAB RADIUS FOR A ROLE, with the chosen pick precision applied. The single source.
    pub(crate) fn grab(&self, what: Grab) -> f32 {
        what.base() * precision_factor(self.set.pick_precision)
    }
}
