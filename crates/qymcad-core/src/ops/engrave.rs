//! Engraving along the centreline of a contour, with a V bit or an engraver.
//!
//! It is a contour operation with the side set to `On`, usually taken in one or a few passes by depth.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Contour, Point3};
use crate::ir::{Move, Toolpath};
use crate::tool::Tool;

pub struct EngraveOp {
    pub name: String,
    pub tool: Tool,
    pub contours: Vec<Contour>,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

impl Operation for EngraveOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "engrave", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        for c in &self.contours {
            if c.points.len() < 2 {
                continue;
            }
            let start = c.points[0];
            for z in self.heights.z_levels(self.passes.stepdown) {
                tp.push(Move::Rapid { to: Point3::new(start.x, start.y, self.heights.retract) });
                tp.push(Move::Plunge { to: Point3::at(start, z), feed: self.feeds.plunge });
                for p in c.points.iter().skip(1) {
                    tp.push(Move::Linear { to: Point3::at(*p, z), feed: self.feeds.cut });
                }
                if c.closed {
                    tp.push(Move::Linear { to: Point3::at(start, z), feed: self.feeds.cut });
                }
                tp.push(Move::Rapid {
                    to: Point3::new(c.points.last().unwrap().x, c.points.last().unwrap().y, self.heights.retract),
                });
            }
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
