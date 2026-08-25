//! Offsets of contours through `cavalier_contours`, preserving arcs.
//!
//! Used to compensate for the cutter diameter in a contour operation, and to clear a pocket as a series of
//! nested offsets. The library rounds convex corners with arcs, given as bulge values, and those are
//! tessellated back into a polyline here.

use crate::geom::{Contour, Point2};
use cavalier_contours::polyline::{PlineSource, PlineSourceMut, Polyline};

const ARC_SAG: f64 = 0.01;

/// A polyline vertex carrying an arc: `bulge = tan(θ/4)`, signed by direction, with zero meaning a straight
/// segment to the next vertex. Used by the arc-aware offset of a sketch, where both input and output keep their
/// arcs.
#[derive(Clone, Copy, Debug)]
pub struct BVert {
    pub x: f64,
    pub y: f64,
    pub bulge: f64,
}

/// Offset a closed bulge polyline by a signed distance, keeping the arcs in the output.
///
/// Returns a list of bulge polylines, since a shape may break into several loops.
pub fn offset_bulge(verts: &[BVert], dist: f64) -> Vec<Vec<BVert>> {
    if verts.len() < 2 {
        return Vec::new();
    }
    let mut pl: Polyline<f64> = Polyline::new_closed();
    for v in verts {
        pl.add(v.x, v.y, v.bulge);
    }
    pl.parallel_offset(dist)
        .iter()
        .map(|r| (0..r.vertex_count()).map(|i| { let v = r.at(i); BVert { x: v.x, y: v.y, bulge: v.bulge } }).collect::<Vec<_>>())
        .filter(|v: &Vec<BVert>| v.len() >= 2)
        .collect()
}

/// Offset a contour by a signed distance. May return several loops if the shape breaks apart. Works on closed
/// contours.
pub fn offset(contour: &Contour, dist: f64) -> Vec<Contour> {
    if contour.points.len() < 3 || !contour.closed {
        return Vec::new();
    }
    let mut pl: Polyline<f64> = Polyline::new_closed();
    for p in &contour.points {
        pl.add(p.x, p.y, 0.0);
    }
    pl.parallel_offset(dist)
        .iter()
        .map(pline_to_contour)
        .filter(|c| c.points.len() >= 3)
        .collect()
}

/// Offset outwards, growing the extents, or inwards, shrinking them, regardless of the traversal direction of
/// the original contour: the choice is made by area.
pub fn offset_to_side(contour: &Contour, dist: f64, outward: bool) -> Vec<Contour> {
    let base = contour.area();
    let mut candidates: Vec<Contour> = Vec::new();
    candidates.extend(offset(contour, dist));
    candidates.extend(offset(contour, -dist));

    candidates
        .into_iter()
        .filter(|c| if outward { c.area() > base } else { c.area() < base })
        .collect()
}

/// A boolean operation over two closed contours, used by trimming and region edits.
///
/// `op` selects the operation: 0 subtracts b from a, 1 unions, 2 intersects, 3 is exclusive or. Returns the
/// resulting contours, both outer and inner boundaries.
pub fn boolean_contours(a: &Contour, b: &Contour, op: u8) -> Vec<Contour> {
    use cavalier_contours::polyline::BooleanOp;
    if a.points.len() < 3 || b.points.len() < 3 || !a.closed || !b.closed {
        return Vec::new();
    }
    let to_pline = |c: &Contour| -> Polyline<f64> {
        let mut pl = Polyline::new_closed();
        for p in &c.points {
            pl.add(p.x, p.y, 0.0);
        }
        pl
    };
    let bop = match op {
        1 => BooleanOp::Or,
        2 => BooleanOp::And,
        3 => BooleanOp::Xor,
        _ => BooleanOp::Not,
    };
    let res = to_pline(a).boolean(&to_pline(b), bop);
    res.pos_plines.iter().chain(res.neg_plines.iter()).map(|r| pline_to_contour(&r.pline)).filter(|c| c.points.len() >= 3).collect()
}

fn pline_to_contour(pl: &Polyline<f64>) -> Contour {
    let n = pl.vertex_count();
    let closed = pl.is_closed();
    let mut pts: Vec<Point2> = Vec::with_capacity(n);
    for i in 0..n {
        let v = pl.at(i);
        pts.push(Point2::new(v.x, v.y));
        let next = if i + 1 < n {
            Some(pl.at(i + 1))
        } else if closed {
            Some(pl.at(0))
        } else {
            None
        };
        if let Some(nv) = next {
            if v.bulge.abs() > 1e-9 {
                let arc = tessellate_bulge(
                    Point2::new(v.x, v.y),
                    Point2::new(nv.x, nv.y),
                    v.bulge,
                    ARC_SAG,
                );
                pts.extend(arc);
            }
        }
    }
    Contour { points: pts, closed, edges: Vec::new(), edge_src: Vec::new() }
}

/// Tessellate a bulge arc between p0 and p1, where `bulge = tan(θ/4)`. Returns the intermediate points only,
/// without the endpoints.
fn tessellate_bulge(p0: Point2, p1: Point2, bulge: f64, max_sag: f64) -> Vec<Point2> {
    let theta = 4.0 * bulge.atan(); // the signed swept angle
    if theta.abs() < 1e-9 {
        return Vec::new();
    }
    let chord = p0.dist(p1);
    if chord < 1e-9 {
        return Vec::new();
    }
    let half = theta / 2.0;
    let radius = chord / 2.0 / half.sin();
    let ux = (p1.x - p0.x) / chord;
    let uy = (p1.y - p0.y) / chord;
    // the left normal
    let (nx, ny) = (-uy, ux);
    let apo = radius * half.cos();
    let mx = (p0.x + p1.x) / 2.0;
    let my = (p0.y + p1.y) / 2.0;
    let (cx, cy) = (mx + nx * apo, my + ny * apo);

    let a0 = (p0.y - cy).atan2(p0.x - cx);
    let r = radius.abs();
    let sag = max_sag.max(1e-6).min(r);
    let dtheta_max = 2.0 * (1.0 - sag / r).clamp(-1.0, 1.0).acos();
    let segs = (theta.abs() / dtheta_max.max(1e-3)).ceil().max(1.0) as usize;

    let mut out = Vec::with_capacity(segs.saturating_sub(1));
    for i in 1..segs {
        let t = a0 + theta * (i as f64 / segs as f64);
        out.push(Point2::new(cx + r * t.cos(), cy + r * t.sin()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulge_quarter_circle() {
        // p0=(1,0) -> p1=(0,1) with bulge=tan(22.5°): an arc of the unit circle centred at (0,0)
        let b = (22.5_f64).to_radians().tan();
        let pts = tessellate_bulge(Point2::new(1.0, 0.0), Point2::new(0.0, 1.0), b, 0.001);
        assert!(!pts.is_empty());
        // every intermediate point lies on the unit circle
        for p in &pts {
            assert!((p.len() - 1.0).abs() < 1e-2, "the point is not on the circle: {p:?}");
        }
        // the middle one passes near (0.707, 0.707)
        let mid = pts[pts.len() / 2];
        assert!((mid.x - 0.7071).abs() < 0.05 && (mid.y - 0.7071).abs() < 0.05);
    }

    #[test]
    fn offset_square_outward_inward() {
        // a 10×10 square, counter-clockwise
        let sq = Contour::closed(vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ]);
        let out = offset_to_side(&sq, 2.0, true);
        assert!(!out.is_empty(), "an outward offset has to exist");
        // the area grew: with rounded corners it is about 14×14 minus the corner cuts
        assert!(out[0].area() > 100.0);

        let inn = offset_to_side(&sq, 2.0, false);
        assert!(!inn.is_empty(), "an inward offset has to exist");
        // the inner square is 6×6, giving 36
        assert!((inn[0].area() - 36.0).abs() < 1.0, "expected about 36, got {}", inn[0].area());
    }
}
