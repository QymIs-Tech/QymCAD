//! 3D machining over a Z map: raster finishing, waterline finishing by levels, and layered roughing.
//!
//! All three support a boundary: where closed contours are given, only what lies inside them is machined;
//! otherwise the whole part is.

use super::params::{intro, new_toolpath, Feeds, Heights, Passes};
use super::Operation;
use crate::geom::{Contour, Mesh, Point2, Point3};
use crate::heightmap::Heightmap;
use crate::ir::{Move, Toolpath};
use crate::offset::offset_to_side;
use crate::tool::{Tool, ToolType};

/// Whether a point lies inside the machining area; an empty boundary always answers yes.
fn in_boundary(x: f64, y: f64, b: &[Contour]) -> bool {
    if b.is_empty() {
        return true;
    }
    b.iter().any(|c| c.closed && c.contains(Point2::new(x, y)))
}

/// Raster emission over the grid ys by [x0..x1] with a step of dx, honouring the area and the shape of the
/// surface, where `tip(x, y)` is the height of the cutter tip. A zigzag, plunging and retracting at the
/// boundaries of the area.
fn emit_raster(
    tp: &mut Toolpath,
    boundary: &[Contour],
    (x0, x1): (f64, f64),
    ys: &[f64],
    dx: f64,
    tip: &dyn Fn(f64, f64) -> f64,
    h: &Heights,
    f: &Feeds,
) {
    let mut l2r = true;
    for &yy in ys {
        let mut engaged = false;
        let mut last = (x0, yy);
        let mut x = if l2r { x0 } else { x1 };
        let step = if l2r { dx } else { -dx };
        loop {
            let done = if l2r { x > x1 + 1e-6 } else { x < x0 - 1e-6 };
            if done {
                break;
            }
            if in_boundary(x, yy, boundary) {
                let z = tip(x, yy);
                if !engaged {
                    tp.push(Move::Rapid { to: Point3::new(x, yy, h.retract) });
                    tp.push(Move::Plunge { to: Point3::new(x, yy, z), feed: f.plunge });
                    engaged = true;
                } else {
                    tp.push(Move::Linear { to: Point3::new(x, yy, z), feed: f.cut });
                }
                last = (x, yy);
            } else if engaged {
                tp.push(Move::Rapid { to: Point3::new(last.0, last.1, h.retract) });
                engaged = false;
            }
            x += step;
        }
        if engaged {
            tp.push(Move::Rapid { to: Point3::new(last.0, last.1, h.retract) });
        }
        l2r = !l2r;
    }
}

fn ys_range(b: &(Point2, Point2), step: f64) -> Vec<f64> {
    let mut ys = Vec::new();
    let mut y = b.0.y;
    while y <= b.1.y + 1e-6 {
        ys.push(y);
        y += step.max(0.1);
    }
    ys
}

/// Resample a contour into points spaced about `step` apart, for projecting onto a surface.
fn resample(c: &Contour, step: f64) -> Vec<Point2> {
    let mut pts = c.points.clone();
    if c.closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    if pts.len() < 2 {
        return pts;
    }
    let step = step.max(0.05);
    let mut out = vec![pts[0]];
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        let n = (len / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            out.push(Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        }
    }
    out
}

/// Projecting a contour onto the surface of a part: engraving by drop-cutter.
pub struct ProjectOp {
    pub name: String,
    pub tool: Tool,
    pub mesh: Mesh,
    pub contours: Vec<Contour>,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

impl Operation for ProjectOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "project", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let cell = 0.5;
        let Some(hm) = Heightmap::from_mesh(&self.mesh, cell) else {
            return tp;
        };
        let r = self.tool.radius();
        let ball = matches!(self.tool.kind, ToolType::BallNose);
        let engrave = (self.heights.top - self.heights.bottom).max(0.05);
        let sd = self.passes.stepdown.max(0.05);
        let mut depths = Vec::new();
        let mut d = sd.min(engrave);
        loop {
            depths.push(d);
            if d >= engrave - 1e-6 {
                break;
            }
            d = (d + sd).min(engrave);
        }

        for c in &self.contours {
            let samples = resample(c, 0.4);
            if samples.len() < 2 {
                continue;
            }
            for &depth in &depths {
                let p0 = samples[0];
                let z0 = hm.drop_tool(p0.x, p0.y, r, ball) - depth;
                tp.push(Move::Rapid { to: Point3::new(p0.x, p0.y, self.heights.retract) });
                tp.push(Move::Plunge { to: Point3::new(p0.x, p0.y, z0), feed: self.feeds.plunge });
                for p in samples.iter().skip(1) {
                    let z = hm.drop_tool(p.x, p.y, r, ball) - depth;
                    tp.push(Move::Linear { to: Point3::new(p.x, p.y, z), feed: self.feeds.cut });
                }
                let last = *samples.last().unwrap();
                tp.push(Move::Rapid { to: Point3::new(last.x, last.y, self.heights.retract) });
            }
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}

/// Machining horizontal flat areas, the upward-facing planar faces, at their own height.
///
/// It uses face recognition: a face whose normal points up gives its outline, which is then cleared by raster
/// at the Z of that face.
pub struct FlatOp {
    pub name: String,
    pub tool: Tool,
    pub mesh: Mesh,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
}

impl Operation for FlatOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "flat", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let r = self.tool.radius();
        let cell = (self.passes.stepover * 0.5).clamp(0.2, 2.0);
        let faces = self.mesh.detect_faces(6.0);
        for f in &faces {
            if f.normal[2] < 0.9 || f.area < r * r {
                continue; // upward-facing horizontal faces only, and not tiny ones
            }
            let z = f.centroid.z + self.passes.stock_to_leave;
            // the boundary of the area, shrunk by the cutter radius so it does not overhang
            let mut inner = Vec::new();
            for o in self.mesh.face_outline_xy(f) {
                if o.closed && o.points.len() >= 3 {
                    inner.extend(offset_to_side(&o, r, false));
                }
            }
            if inner.is_empty() {
                continue;
            }
            let (mut mn, mut mx) = (Point2::new(f64::INFINITY, f64::INFINITY), Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
            for o in &inner {
                if let Some(b) = o.bbox() {
                    mn.x = mn.x.min(b.min.x);
                    mn.y = mn.y.min(b.min.y);
                    mx.x = mx.x.max(b.max.x);
                    mx.y = mx.y.max(b.max.y);
                }
            }
            let ys = ys_range(&(mn, mx), self.passes.stepover);
            let tip = |_x: f64, _y: f64| z;
            emit_raster(&mut tp, &inner, (mn.x, mx.x), &ys, cell, &tip, &self.heights, &self.feeds);
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}

/// Raster finishing by drop-cutter.
pub struct SurfaceOp {
    pub name: String,
    pub tool: Tool,
    pub mesh: Mesh,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    pub boundary: Vec<Contour>,
}

impl Operation for SurfaceOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "surface", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let cell = (self.passes.stepover * 0.5).clamp(0.2, 2.0);
        let Some(hm) = Heightmap::from_mesh(&self.mesh, cell) else {
            return tp;
        };
        let Some(b3) = self.mesh.bounds() else { return tp };
        let bmin = Point2::new(b3.min.x, b3.min.y);
        let bmax = Point2::new(b3.max.x, b3.max.y);

        let r = self.tool.radius();
        let ball = matches!(self.tool.kind, ToolType::BallNose);
        let bottom = self.heights.bottom;
        let leave = self.passes.stock_to_leave;
        let tip = |x: f64, y: f64| (hm.drop_tool(x, y, r, ball) + leave).max(bottom);

        let ys = ys_range(&(bmin, bmax), self.passes.stepover);
        emit_raster(&mut tp, &self.boundary, (bmin.x, bmax.x), &ys, cell, &tip, &self.heights, &self.feeds);

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}

/// Waterline finishing by Z levels, following iso-contours.
pub struct WaterlineOp {
    pub name: String,
    pub tool: Tool,
    pub mesh: Mesh,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    pub boundary: Vec<Contour>,
}

impl Operation for WaterlineOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "waterline", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let cell = (self.passes.stepover * 0.5).clamp(0.2, 2.0);
        let Some(hm) = Heightmap::from_mesh(&self.mesh, cell) else {
            return tp;
        };
        let r = self.tool.radius() + self.passes.stock_to_leave;

        for z in self.heights.z_levels(self.passes.stepdown) {
            for iso in hm.iso_contours(z) {
                // the boundary restriction: the centre of the contour has to lie inside
                if !self.boundary.is_empty() {
                    let c = iso.centroid();
                    if !in_boundary(c.x, c.y, &self.boundary) {
                        continue;
                    }
                }
                let paths = if iso.closed && iso.points.len() >= 3 {
                    let off = offset_to_side(&iso, r, true);
                    if off.is_empty() {
                        vec![iso]
                    } else {
                        off
                    }
                } else {
                    vec![iso]
                };
                for path in &paths {
                    if path.points.len() < 2 {
                        continue;
                    }
                    let start = path.points[0];
                    tp.push(Move::Rapid { to: Point3::new(start.x, start.y, self.heights.retract) });
                    tp.push(Move::Plunge { to: Point3::at(start, z), feed: self.feeds.plunge });
                    for p in path.points.iter().skip(1) {
                        tp.push(Move::Linear { to: Point3::at(*p, z), feed: self.feeds.cut });
                    }
                    if path.closed {
                        tp.push(Move::Linear { to: Point3::at(start, z), feed: self.feeds.cut });
                    }
                    tp.push(Move::Rapid { to: Point3::new(start.x, start.y, self.heights.retract) });
                }
            }
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}

/// Roughing: layered raster clearing of the stock above the part.
pub struct Rough3DOp {
    pub name: String,
    pub tool: Tool,
    pub mesh: Mesh,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    pub boundary: Vec<Contour>,
}

impl Operation for Rough3DOp {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self) -> Toolpath {
        let mut tp = new_toolpath(&self.name, "rough3d", &self.tool);
        intro(&mut tp, &self.name, &self.tool, &self.feeds);
        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });

        let cell = self.passes.stepover.clamp(0.3, 4.0);
        let Some(hm) = Heightmap::from_mesh(&self.mesh, cell) else {
            return tp;
        };
        let Some(b3) = self.mesh.bounds() else { return tp };
        let bmin = Point2::new(b3.min.x, b3.min.y);
        let bmax = Point2::new(b3.max.x, b3.max.y);

        let r = self.tool.radius();
        let ball = matches!(self.tool.kind, ToolType::BallNose);
        let leave = self.passes.stock_to_leave.max(0.0);
        let ys = ys_range(&(bmin, bmax), self.passes.stepover);

        for z in self.heights.z_levels(self.passes.stepdown) {
            let tip = |x: f64, y: f64| (hm.drop_tool(x, y, r, ball) + leave).max(z);
            emit_raster(&mut tp, &self.boundary, (bmin.x, bmax.x), &ys, cell, &tip, &self.heights, &self.feeds);
            tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        }

        tp.push(Move::Rapid { to: Point3::new(0.0, 0.0, self.heights.clearance) });
        tp
    }
}
