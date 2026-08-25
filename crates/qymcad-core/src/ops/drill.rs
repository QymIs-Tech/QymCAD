//! Drilling at a list of points. Emits a high-level `DrillCycle`; the post-processor decides whether to output
//! G81, G82 or G83 or to expand it into G0 and G1 moves.

use super::params::{intro, new_toolpath, Feeds, Heights};
use super::Operation;
use crate::geom::{Point2, Point3};
use crate::ir::{DrillKind, Move, Toolpath};
use crate::tool::Tool;

pub struct DrillOp {
    pub name: String,
    pub tool: Tool,
    /// The centres of the holes, in XY.
    pub points: Vec<Point2>,
    pub kind: DrillKind,
    pub heights: Heights,
    /// The peck depth, Q, for G83.
    pub peck: Option<f64>,
    /// The dwell at the bottom, P in seconds, for G82.
    pub dwell: Option<f64>,
    pub feeds: Feeds,
}

impl DrillOp {
    /// Sort the points greedily by proximity, which shortens the rapids.
    fn sorted_points(&self) -> Vec<Point2> {
        let mut remaining = self.points.clone();
        let mut ordered = Vec::with_capacity(remaining.len());
        if remaining.is_empty() {
            return ordered;
        }
        let mut cur = remaining.remove(0);
        ordered.push(cur);
        while !remaining.is_empty() {
            let (i, _) = remaining
                .iter()
                .enumerate()
                .min_by(|a, b| cur.dist(*a.1).partial_cmp(&cur.dist(*b.1)).unwrap())
                .unwrap();
            cur = remaining.remove(i);
            ordered.push(cur);
        }
        ordered
    }
}

impl Operation for DrillOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "drill", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let pts: Vec<Point3> = self
            .sorted_points()
            .iter()
            .map(|p| Point3::at(*p, self.heights.bottom))
            .collect();

        tp.push(Move::DrillCycle {
            kind: self.kind,
            points: pts,
            retract: self.heights.retract,
            feed: self.feeds.plunge,
            peck: self.peck,
            dwell: self.dwell,
        });

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
