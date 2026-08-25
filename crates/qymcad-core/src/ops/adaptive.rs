//! Adaptive, or trochoidal, 2D clearing: high-speed machining that keeps the load on the cutter small and
//! constant. The area is given by closed contours and is filled with a serpentine of trochoidal loops.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Contour, Point2, Point3};
use crate::ir::{Move, Toolpath};
use crate::offset::offset_to_side;
use crate::tool::Tool;
use std::f64::consts::TAU;

pub struct AdaptiveOp {
    pub name: String,
    pub tool: Tool,
    /// The closed contours bounding the area to clear.
    pub boundary: Vec<Contour>,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

fn inside(x: f64, y: f64, region: &[Contour]) -> bool {
    region.iter().any(|c| c.closed && c.contains(Point2::new(x, y)))
}

impl Operation for AdaptiveOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "adaptive", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        // the area is the boundary shrunk by the cutter radius, keeping the centre inside
        let r = self.tool.radius() + self.passes.stock_to_leave;
        let mut region: Vec<Contour> = Vec::new();
        for c in &self.boundary {
            if c.closed && c.points.len() >= 3 {
                region.extend(offset_to_side(c, r, false));
            }
        }
        if region.is_empty() {
            region = self.boundary.clone();
        }
        let (mut mn, mut mx) = (Point2::new(f64::INFINITY, f64::INFINITY), Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for c in &region {
            if let Some(b) = c.bbox() {
                mn.x = mn.x.min(b.min.x);
                mn.y = mn.y.min(b.min.y);
                mx.x = mx.x.max(b.max.x);
                mx.y = mx.y.max(b.max.y);
            }
        }
        if !mn.x.is_finite() {
            return tp;
        }

        let step = self.passes.stepover.max(0.3);
        let loop_r = step * 0.7;
        let pitch = step * 0.6;
        let dtheta = TAU / 24.0;

        for z in self.heights.z_levels(self.passes.stepdown) {
            let mut yy = mn.y + loop_r;
            while yy <= mx.y - loop_r + 1e-6 {
                let length = mx.x - mn.x;
                let total = length / pitch * TAU;
                let mut theta = 0.0;
                let mut engaged = false;
                let mut last = (mn.x, yy);
                while theta <= total {
                    let advance = pitch * theta / TAU;
                    let cx = mn.x + advance + loop_r * theta.cos();
                    let cy = yy + loop_r * theta.sin();
                    if inside(cx, cy, &region) {
                        if !engaged {
                            tp.push(Move::Rapid { to: Point3::new(cx, cy, self.heights.retract) });
                            tp.push(Move::Plunge { to: Point3::new(cx, cy, z), feed: self.feeds.plunge });
                            engaged = true;
                        } else {
                            tp.push(Move::Linear { to: Point3::new(cx, cy, z), feed: self.feeds.cut });
                        }
                        last = (cx, cy);
                    } else if engaged {
                        tp.push(Move::Rapid { to: Point3::new(last.0, last.1, self.heights.retract) });
                        engaged = false;
                    }
                    theta += dtheta;
                }
                if engaged {
                    tp.push(Move::Rapid { to: Point3::new(last.0, last.1, self.heights.retract) });
                }
                yy += step;
            }
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
