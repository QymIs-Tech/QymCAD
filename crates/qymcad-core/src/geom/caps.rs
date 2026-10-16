//! THE CAPS OF A SECTION and polygons with holes triangulated.

use super::*;

/// A section cap computed from the mesh: a closed slice of a body by a plane, with no B-rep involved.
///
/// A section that leaves bodies hollow inside is not how a section is expected to read; a plane must draw a closed
/// body. The cap used to be computed as a kernel boolean (the common part of a face and a body), which made it
/// unavailable wherever no live B-rep exists: an imported STL, a body loaded from a bundle before the B-rep is
/// built, and over a thousand bodies a boolean is too expensive anyway. A mesh is always present, so the cap is
/// computed from it.
///
/// The triangles crossed by the plane give the segments of the slice; the segments are stitched into loops
/// (`stitch_segments`), the loops are sorted into outer ones and holes by nesting and triangulated
/// (`triangulate_with_holes`). Returns the triangles of the cap in world coordinates.
pub fn mesh_section_cap(mesh: &Mesh, origin: [f64; 3], normal: [f64; 3]) -> Vec<[Point3; 3]> {
    let nl = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if nl < 1e-12 || mesh.tris.is_empty() {
        return Vec::new();
    }
    let n = [normal[0] / nl, normal[1] / nl, normal[2] / nl];
    // The basis of the plane.
    let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let cross = |p: [f64; 3], q: [f64; 3]| [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]];
    let norm = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let u = norm(cross(a, n));
    let v = cross(n, u);
    let dist = |p: &Point3| (p.x - origin[0]) * n[0] + (p.y - origin[1]) * n[1] + (p.z - origin[2]) * n[2];
    let to2 = |p: Point3| {
        let d = [p.x - origin[0], p.y - origin[1], p.z - origin[2]];
        Point2::new(d[0] * u[0] + d[1] * u[1] + d[2] * u[2], d[0] * v[0] + d[1] * v[1] + d[2] * v[2])
    };
    // The stitching tolerance follows the scale of the scene: a small part and a metre-sized frame must not share
    // one constant.
    let tol = mesh.bounds().map(|b| ((b.max.x - b.min.x).abs() + (b.max.y - b.min.y).abs() + (b.max.z - b.min.z).abs()) * 1e-6).unwrap_or(1e-6).max(1e-9);

    let mut segs: Vec<(Point2, Point2)> = Vec::new();
    for ti in 0..mesh.tris.len() {
        let t = mesh.triangle(ti);
        let d = [dist(&t[0]), dist(&t[1]), dist(&t[2])];
        let (pos, neg, zero) = (d.iter().filter(|x| **x > tol).count(), d.iter().filter(|x| **x < -tol).count(), d.iter().filter(|x| x.abs() <= tol).count());
        if pos > 0 && neg > 0 {
            // A genuine intersection: points on the edges that change sign, plus vertices exactly on the plane.
            let mut pts: Vec<Point2> = Vec::new();
            for k in 0..3 {
                let (p0, p1) = (t[k], t[(k + 1) % 3]);
                let (d0, d1) = (d[k], d[(k + 1) % 3]);
                if d0.abs() <= tol {
                    pts.push(to2(p0));
                }
                if (d0 > tol && d1 < -tol) || (d0 < -tol && d1 > tol) {
                    let s = d0 / (d0 - d1);
                    pts.push(to2(Point3::new(p0.x + (p1.x - p0.x) * s, p0.y + (p1.y - p0.y) * s, p0.z + (p1.z - p0.z) * s)));
                }
            }
            pts.dedup_by(|a, b| a.dist(*b) <= tol);
            if pts.len() >= 2 && pts[0].dist(pts[1]) > tol {
                segs.push((pts[0], pts[1]));
            }
        } else if zero == 2 && (pos + neg) == 1 {
            // An edge lying in the plane, which is what a section taken exactly along a datum produces: no
            // triangle "crosses" the plane at all, and without this branch a piece of the contour was lost, the
            // loop did not close and there was no cap whatsoever. Two triangles share such an edge, so the
            // duplicates are removed below.
            let k = (0..3).find(|&k| d[k].abs() <= tol && d[(k + 1) % 3].abs() <= tol).unwrap_or(0);
            let (a, b) = (to2(t[k]), to2(t[(k + 1) % 3]));
            if a.dist(b) > tol {
                segs.push((a, b));
            }
        }
    }
    // Remove duplicates: an edge in the plane arrives from both adjacent triangles, and stitching would branch.
    segs.dedup_by(|a, b| (a.0.dist(b.0) <= tol && a.1.dist(b.1) <= tol) || (a.0.dist(b.1) <= tol && a.1.dist(b.0) <= tol));
    {
        let mut uniq: Vec<(Point2, Point2)> = Vec::with_capacity(segs.len());
        for s in segs.drain(..) {
            if !uniq.iter().any(|u| (u.0.dist(s.0) <= tol && u.1.dist(s.1) <= tol) || (u.0.dist(s.1) <= tol && u.1.dist(s.0) <= tol)) {
                uniq.push(s);
            }
        }
        segs = uniq;
    }
    if segs.is_empty() {
        return Vec::new();
    }
    let loops: Vec<Contour> = stitch_segments(segs, tol.max(1e-7) * 10.0).into_iter().filter(|c| c.closed && c.points.len() >= 3 && c.area() > 0.0).collect();
    if loops.is_empty() {
        return Vec::new();
    }
    // Nesting: a loop is a hole when it lies inside an odd number of others, by the parity rule.
    //
    // The probe point is deliberately not the centroid: on a concave, C-shaped loop the centroid lies outside the
    // loop itself, so nesting came out wrong and parts of the cap were filled incorrectly. The probe is a point
    // known to be inside the loop: the midpoint of a diagonal running inwards from a vertex, verified by testing it
    // against the loop. If none is found, which means a degenerate loop, it falls back to the centroid.
    let probe = |c: &Contour| -> Point2 {
        let n = c.points.len();
        for i in 0..n {
            let (prev, cur, next) = (c.points[(i + n - 1) % n], c.points[i], c.points[(i + 1) % n]);
            // A point just inside the vertex, along the bisector of the adjacent edges.
            let mid = Point2::new((prev.x + next.x) * 0.5, (prev.y + next.y) * 0.5);
            let p = Point2::new(cur.x + (mid.x - cur.x) * 0.5, cur.y + (mid.y - cur.y) * 0.5);
            if c.contains(p) {
                return p;
            }
        }
        c.centroid()
    };
    let probes: Vec<Point2> = loops.iter().map(&probe).collect();
    let inside = |ip: Point2, ia: f64, outer: &Contour| outer.contains(ip) && outer.area() > ia;
    let depth: Vec<usize> = loops.iter().enumerate().map(|(i, c)| loops.iter().enumerate().filter(|(j, o)| *j != i && inside(probes[i], c.area(), o)).count()).collect();
    let mut tris: Vec<[Point3; 3]> = Vec::new();
    for (i, c) in loops.iter().enumerate() {
        if !depth[i].is_multiple_of(2) {
            continue; // A hole; the material around it comes from its parent loop.
        }
        // The direct children of this loop are its holes.
        let holes: Vec<Vec<Point2>> = loops.iter().enumerate().filter(|(j, h)| depth[*j] == depth[i] + 1 && inside(probes[*j], h.area(), c)).map(|(_, h)| h.points.clone()).collect();
        for t in triangulate_with_holes(&c.points, &holes) {
            let lift = |p: Point2| Point3::new(origin[0] + u[0] * p.x + v[0] * p.y, origin[1] + u[1] * p.x + v[1] * p.y, origin[2] + u[2] * p.x + v[2] * p.y);
            tris.push([lift(t[0]), lift(t[1]), lift(t[2])]);
        }
    }
    tris
}

/// Triangulate a polygon with holes by ear clipping with bridges to the holes.
///
/// Each hole is cut into the outer contour by a bridge from its rightmost point to a visible vertex of the contour,
/// the classical technique that leaves one simple polygon and ordinary ear clipping. Degenerate input (a loop of
/// fewer than three points, or zero area) yields nothing rather than garbage.
pub fn triangulate_with_holes(outer: &[Point2], holes: &[Vec<Point2>]) -> Vec<[Point2; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let area2 = |p: &[Point2]| -> f64 {
        let n = p.len();
        (0..n).map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y).sum::<f64>()
    };
    // The outer contour runs counter-clockwise and the holes clockwise, so a bridge does not turn the polygon
    // inside out.
    let mut poly: Vec<Point2> = outer.to_vec();
    if area2(&poly) < 0.0 {
        poly.reverse();
    }
    let mut hs: Vec<Vec<Point2>> = holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|h| {
            let mut h = h.clone();
            if area2(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .collect();
    // Holes sorted by the x of their rightmost point, descending, so they are cut in from right to left.
    hs.sort_by(|a, b| {
        let mx = |h: &Vec<Point2>| h.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        mx(b).partial_cmp(&mx(a)).unwrap_or(std::cmp::Ordering::Equal)
    });
    for h in hs {
        let (hi, _) = h.iter().enumerate().fold((0usize, f64::MIN), |(bi, bx), (i, p)| if p.x > bx { (i, p.x) } else { (bi, bx) });
        // The nearest vertex of the outer contour to the right of the cut-in point, a simple and robust bridge.
        let m = h[hi];
        let Some(oi) = (0..poly.len())
            .filter(|&i| poly[i].x >= m.x - 1e-9)
            .min_by(|&i, &j| poly[i].dist(m).partial_cmp(&poly[j].dist(m)).unwrap_or(std::cmp::Ordering::Equal))
            .or_else(|| (0..poly.len()).min_by(|&i, &j| poly[i].dist(m).partial_cmp(&poly[j].dist(m)).unwrap_or(std::cmp::Ordering::Equal)))
        else {
            continue;
        };
        let mut merged: Vec<Point2> = Vec::with_capacity(poly.len() + h.len() + 2);
        merged.extend_from_slice(&poly[..=oi]);
        for k in 0..h.len() {
            merged.push(h[(hi + k) % h.len()]);
        }
        merged.push(m);
        merged.push(poly[oi]);
        merged.extend_from_slice(&poly[oi + 1..]);
        poly = merged;
    }
    // ── ear clipping ─────────────────────────────────────────────────────────────────────────────
    // The tolerance is relative. An absolute one (1e-12) worked on nothing real: the coordinates of a part run to
    // hundreds of millimetres, cross products have the dimension of area, and on the shallow flanks of a thread
    // almost any vertex counted as "inside the ear". No ear was found, the loop exited through `None => break` and
    // silently returned a stub: the section of a threaded part was filled only across its smooth portion — of 424
    // triangles in the cap, 5 fell in the thread.
    let cross = |o: Point2, a: Point2, b: Point2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let (mut lo, mut hi) = (poly[0], poly[0]);
    for p in &poly {
        lo = Point2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let scale = (hi.x - lo.x).hypot(hi.y - lo.y).max(1e-9);
    let eps = 1e-10 * scale * scale; // A threshold on area, not on length.
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    let mut out: Vec<[Point2; 3]> = Vec::new();
    let mut guard = poly.len() * poly.len() + 16;
    while idx.len() > 2 && guard > 0 {
        guard -= 1;
        let n = idx.len();
        let mut cut = None;
        let mut best_convex: Option<(usize, f64)> = None; // The fallback when no ear is found.
        for k in 0..n {
            let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            let conv = cross(a, b, c);
            if conv <= eps {
                continue; // Not a convex corner, or a degenerate one.
            }
            if best_convex.map(|(_, v)| conv > v).unwrap_or(true) {
                best_convex = Some((k, conv));
            }
            // An ear is a convex corner with no reflex vertex inside it. Testing only the reflex vertices is the
            // classical formulation and the only correct one: a convex vertex may legitimately lie on an edge of
            // the ear — after holes are bridged in, such coincidences always occur — and an inclusive test counted
            // that as "inside", so no ear was found. The containment test is therefore strict: a point on an edge
            // does not block the ear.
            //
            // The earlier variant, inclusive and over all vertices, produced overlapping triangles: on one section
            // the cap covered 103.5% of the area of the contour. An area larger than the figure itself can only
            // come from overlaps, and on screen it reads as a torn fill.
            let inside_ear = (0..n).any(|jk| {
                let j = idx[jk];
                if j == ia || j == ib || j == ic {
                    return false;
                }
                // Is vertex j reflex within the current contour?
                let (pj0, pj1, pj2) = (poly[idx[(jk + n - 1) % n]], poly[j], poly[idx[(jk + 1) % n]]);
                if cross(pj0, pj1, pj2) >= -eps {
                    return false; // Convex or nearly straight, so it cannot block the ear.
                }
                // A reflex vertex blocks the ear even when it lies on its boundary: otherwise the ear covers the
                // notch and the area of the cap exceeds the figure itself (725 instead of 700 on a C-shaped
                // profile).
                let p = poly[j];
                cross(a, b, p) >= -eps && cross(b, c, p) >= -eps && cross(c, a, p) >= -eps
            });
            if !inside_ear {
                out.push([a, b, c]);
                cut = Some(k);
                break;
            }
        }
        match cut.or_else(|| best_convex.map(|(k, _)| k)) {
            // The fallback: with no ear found, clip the most convex corner and carry on. One questionable
            // triangle at a degenerate spot is far better than losing the fill of a whole region, which is what
            // the loop used to do by exiting here.
            Some(k) => {
                if cut.is_none() {
                    let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
                    out.push([poly[ia], poly[ib], poly[ic]]);
                }
                idx.remove(k);
            }
            None => break, // No convex corners are left at all: the contour has degenerated.
        }
    }
    out
}
