//! Importing SVG into exact curves. `usvg` resolves the transforms and units and returns flat paths.
//!
//! SVG curves are Beziers — `usvg` does not keep arcs or circles as primitives but lowers them into cubic
//! Beziers — so the curves are linearised into segments while straight segments stay as they are. Y is flipped,
//! SVG running downwards and CAM upwards. The sketch assembles connected chains from the segments.

use qymcad_core::geom::{Point2, ProfEdge};
use usvg::tiny_skia_path::PathSegment;

use crate::ImportedSketch;

pub fn import_svg(path: &str) -> Result<ImportedSketch, String> {
    let data = std::fs::read(path).map_err(|e| format!("SVG open: {e}"))?;
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("SVG parse: {e}"))?;
    let h = tree.size().height() as f64;
    let mut curves = Vec::new();
    collect(tree.root(), h, &mut curves);
    Ok(ImportedSketch { curves })
}

fn collect(group: &usvg::Group, h: f64, out: &mut Vec<ProfEdge>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, h, out),
            usvg::Node::Path(p) => path_to_curves(p, h, out),
            _ => {}
        }
    }
}

/// A segment between two points; degenerate ones are skipped.
fn line(a: Point2, b: Point2, out: &mut Vec<ProfEdge>) {
    if a.dist(b) > 1e-9 {
        out.push(ProfEdge::Line { a, b });
    }
}

fn path_to_curves(p: &usvg::Path, h: f64, out: &mut Vec<ProfEdge>) {
    let t = p.abs_transform();
    let map = |x: f32, y: f32| -> Point2 {
        let mut pt = usvg::tiny_skia_path::Point::from_xy(x, y);
        t.map_point(&mut pt);
        Point2::new(pt.x as f64, h - pt.y as f64)
    };

    let mut start = Point2::new(0.0, 0.0); // the start of the current subpath, used by a close command
    let mut last = Point2::new(0.0, 0.0);

    for seg in p.data().segments() {
        match seg {
            PathSegment::MoveTo(pt) => {
                let q = map(pt.x, pt.y);
                start = q;
                last = q;
            }
            PathSegment::LineTo(pt) => {
                let q = map(pt.x, pt.y);
                line(last, q, out);
                last = q;
            }
            PathSegment::QuadTo(c, pt) => {
                let (c, e) = (map(c.x, c.y), map(pt.x, pt.y));
                let mut prev = last;
                for k in 1..=12 {
                    let tt = k as f64 / 12.0;
                    let q = quad(last, c, e, tt);
                    line(prev, q, out);
                    prev = q;
                }
                last = e;
            }
            PathSegment::CubicTo(c1, c2, pt) => {
                let (c1, c2, e) = (map(c1.x, c1.y), map(c2.x, c2.y), map(pt.x, pt.y));
                let mut prev = last;
                for k in 1..=16 {
                    let tt = k as f64 / 16.0;
                    let q = cubic(last, c1, c2, e, tt);
                    line(prev, q, out);
                    prev = q;
                }
                last = e;
            }
            PathSegment::Close => {
                line(last, start, out);
                last = start;
            }
        }
    }
}

fn lerp(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn quad(p0: Point2, p1: Point2, p2: Point2, t: f64) -> Point2 {
    lerp(lerp(p0, p1, t), lerp(p1, p2, t), t)
}

fn cubic(p0: Point2, p1: Point2, p2: Point2, p3: Point2, t: f64) -> Point2 {
    let a = quad(p0, p1, p2, t);
    let b = quad(p1, p2, p3, t);
    lerp(a, b, t)
}
