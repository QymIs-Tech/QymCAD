//! Boring: opening holes with an endmill by helical interpolation.
//!
//! For each hole, given a centre and a diameter, the cutter spirals down at a radius of `hole_r − tool_r` and
//! then takes one full finishing circle at the bottom. The helix is linearised, so the output stays independent
//! of the post-processor. A hole smaller than the cutter is skipped.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Point2, Point3};
use crate::ir::{Move, Toolpath};
use crate::tool::Tool;
use std::f64::consts::TAU;

pub struct BoreOp {
    pub name: String,
    pub tool: Tool,
    /// The holes, as a centre and a diameter.
    pub holes: Vec<(Point2, f64)>,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

impl Operation for BoreOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "bore", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        const SEG: usize = 36; // segments per turn
        let depth = (self.heights.top - self.heights.bottom).max(0.0);

        for (c, dia) in &self.holes {
            let path_r = dia / 2.0 - self.tool.radius();
            if path_r < 0.1 {
                continue; // the cutter is no smaller than the hole, so there is nothing to bore
            }
            let pitch = self.passes.stepdown.max(0.2);
            let turns = (depth / pitch).ceil().max(1.0) as usize;
            let total = (turns * SEG).max(SEG);

            let start = Point2::new(c.x + path_r, c.y);
            tp.push(Move::Rapid { to: Point3::new(start.x, start.y, self.heights.retract) });
            tp.push(Move::Plunge { to: Point3::at(start, self.heights.top), feed: self.feeds.plunge });

            // spiral down
            for s in 1..=total {
                let ang = TAU * (s as f64 / SEG as f64);
                let z = self.heights.top - depth * (s as f64 / total as f64);
                let x = c.x + path_r * ang.cos();
                let y = c.y + path_r * ang.sin();
                tp.push(Move::Linear { to: Point3::new(x, y, z), feed: self.feeds.cut });
            }
            // a finishing circle at the bottom
            for s in 0..=SEG {
                let ang = TAU * (s as f64 / SEG as f64);
                let x = c.x + path_r * ang.cos();
                let y = c.y + path_r * ang.sin();
                tp.push(Move::Linear { to: Point3::new(x, y, self.heights.bottom), feed: self.feeds.cut });
            }
            tp.push(Move::Rapid { to: Point3::new(c.x + path_r, c.y, self.heights.retract) });
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
