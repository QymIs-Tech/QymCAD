//! Facing a rectangular area in a zigzag.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Bbox, Point3};
use crate::ir::{Move, Toolpath};
use crate::tool::Tool;

pub struct FaceOp {
    pub name: String,
    pub tool: Tool,
    /// The area to face.
    pub area: Bbox,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

impl Operation for FaceOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "face", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let r = self.tool.radius();
        let step = self.passes.stepover.max(0.1);
        // the passes overrun the edges by the cutter radius
        let x0 = self.area.min.x - r;
        let x1 = self.area.max.x + r;
        let y0 = self.area.min.y;
        let y1 = self.area.max.y;

        // the positions of the passes along Y
        let mut ys: Vec<f64> = Vec::new();
        let mut y = y0 + r;
        while y < y1 - r + 1e-6 {
            ys.push(y);
            y += step;
        }
        if ys.is_empty() {
            ys.push((y0 + y1) / 2.0);
        } else if *ys.last().unwrap() < y1 - r - 1e-6 {
            ys.push(y1 - r);
        }

        for z in self.heights.z_levels(self.passes.stepdown) {
            let mut left_to_right = true;
            for (i, &yy) in ys.iter().enumerate() {
                let (sx, ex) = if left_to_right { (x0, x1) } else { (x1, x0) };
                if i == 0 {
                    tp.push(Move::Rapid { to: Point3::new(sx, yy, self.heights.retract) });
                    tp.push(Move::Plunge { to: Point3::new(sx, yy, z), feed: self.feeds.plunge });
                } else {
                    // step over to the next pass at the cutting feed rate
                    tp.push(Move::Linear { to: Point3::new(sx, yy, z), feed: self.feeds.cut });
                }
                tp.push(Move::Linear { to: Point3::new(ex, yy, z), feed: self.feeds.cut });
                left_to_right = !left_to_right;
            }
            tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.retract) });
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
