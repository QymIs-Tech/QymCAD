//! Pocketing: clearing an area with contour-parallel passes.
//!
//! A series of nested offsets from the wall inwards, spaced by `stepover`. It is machined from the centre
//! outwards, from the smallest loop to the wall, over several Z passes.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Contour, Point3};
use crate::ir::{Move, Toolpath};
use crate::offset::offset;
use crate::tool::Tool;

pub struct PocketOp {
    pub name: String,
    pub tool: Tool,
    /// The boundaries of the pocket, as closed contours.
    pub boundary: Vec<Contour>,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    /// The length of the corner relief, in mm; zero disables it.
    pub dogbone: f64,
}

impl PocketOp {
    /// Concentric rings from the wall inwards.
    fn rings(&self) -> Vec<Contour> {
        let r = self.tool.radius() + self.passes.stock_to_leave;
        let step = self.passes.stepover.max(0.1);
        let mut rings: Vec<Contour> = Vec::new();

        for b in &self.boundary {
            // the first ring lies one cutter radius inside the wall
            let mut current = inward(b, r);
            let mut dist = r;
            while !current.is_empty() {
                rings.extend(current.clone());
                dist += step;
                current = inward(b, dist);
            }
        }
        // from the smaller area to the larger one, that is from the centre to the wall
        rings.sort_by(|a, c| a.area().partial_cmp(&c.area()).unwrap());
        rings
    }
}

/// Offset inwards, shrinking the area, by the given amount.
fn inward(contour: &Contour, dist: f64) -> Vec<Contour> {
    let base = contour.area();
    let mut res: Vec<Contour> = Vec::new();
    for cand in offset(contour, dist) {
        if cand.area() < base {
            res.push(cand);
        }
    }
    for cand in offset(contour, -dist) {
        if cand.area() < base {
            res.push(cand);
        }
    }
    res
}

impl Operation for PocketOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "pocket", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let rings = self.rings();
        for z in self.heights.z_levels(self.passes.stepdown) {
            // a continuous pass from the centre to the wall, the rings linked without retracting
            let mut first = true;
            for ring in &rings {
                if ring.points.len() < 2 {
                    continue;
                }
                let start = ring.points[0];
                if first {
                    tp.push(Move::Rapid { to: Point3::new(start.x, start.y, self.heights.retract) });
                    tp.push(Move::Plunge { to: Point3::at(start, z), feed: self.feeds.plunge });
                    first = false;
                } else {
                    // step to the next ring at the cutting feed rate, without retracting
                    tp.push(Move::Linear { to: Point3::at(start, z), feed: self.feeds.cut });
                }
                for p in ring.points.iter().skip(1) {
                    tp.push(Move::Linear { to: Point3::at(*p, z), feed: self.feeds.cut });
                }
                if ring.closed {
                    tp.push(Move::Linear { to: Point3::at(start, z), feed: self.feeds.cut });
                }
            }
            // retract only between Z levels
            if !first {
                tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.retract) });
            }
        }

        // corner relief at the floor
        if self.dogbone > 0.0 {
            let z = self.heights.bottom;
            for c in &self.boundary {
                for (corner, tip) in crate::geom::dogbone_overcuts(c, self.dogbone) {
                    tp.push(Move::Rapid { to: Point3::new(corner.x, corner.y, self.heights.retract) });
                    tp.push(Move::Plunge { to: Point3::at(corner, z), feed: self.feeds.plunge });
                    tp.push(Move::Linear { to: Point3::at(tip, z), feed: self.feeds.cut });
                    tp.push(Move::Linear { to: Point3::at(corner, z), feed: self.feeds.cut });
                    tp.push(Move::Rapid { to: Point3::new(corner.x, corner.y, self.heights.retract) });
                }
            }
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
