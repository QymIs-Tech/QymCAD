//! A 2D contour, or profile: following a contour with compensation for the cutter diameter.
//!
//! The side — inside, outside or on the centreline — is realised by offsetting the contour by the cutter radius
//! plus the stock allowance. Several passes down Z, with optional tabs. The compensation is computed here rather
//! than emitted as G41 or G42.

use super::cut::{contour_paths, emit_profile};
use super::params::{intro, new_toolpath, Feeds, Heights, Passes, Ramp, Side, Tabs};
use super::Operation;
use crate::geom::{Contour, Point3};
use crate::ir::{Move, Toolpath};
use crate::tool::Tool;

pub struct ContourOp {
    pub name: String,
    pub tool: Tool,
    pub contours: Vec<Contour>,
    pub side: Side,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    pub tabs: Tabs,
    pub ramp: Ramp,
}

impl Operation for ContourOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "contour", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let r = self.tool.radius() + self.passes.stock_to_leave;
        let paths = contour_paths(&self.contours, self.side, r);
        emit_profile(&mut tp, &paths, &self.heights, &self.passes, &self.feeds, Some(self.tabs), self.ramp);

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
