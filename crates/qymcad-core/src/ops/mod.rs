//! Machining operations. Each turns geometry, a tool and parameters into a `Toolpath`, the neutral intermediate
//! representation. They are pure functions and are tested without an interface.

pub mod adaptive;
pub mod bore;
pub mod contour;
pub mod cut;
pub mod drill;
pub mod engrave;
pub mod face;
pub mod params;
pub mod pocket;
pub mod surface;

pub use adaptive::AdaptiveOp;
pub use bore::BoreOp;
pub use contour::ContourOp;
pub use drill::DrillOp;
pub use engrave::EngraveOp;
pub use face::FaceOp;
pub use params::{Feeds, Heights, Passes, Ramp, Side, Tabs};
pub use pocket::PocketOp;
pub use surface::{FlatOp, ProjectOp, Rough3DOp, SurfaceOp, WaterlineOp};

use crate::ir::Toolpath;

/// The contract shared by every operation.
pub trait Operation {
    fn name(&self) -> &str;
    /// Generate the toolpath.
    fn generate(&self) -> Toolpath;
}
