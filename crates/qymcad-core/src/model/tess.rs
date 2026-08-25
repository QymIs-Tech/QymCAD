//! 2D geometry of a sketch: profiles, tessellation, decomposition into regions, splines.
//!
//! Free functions, unrelated to `Project`; they lived in its file for historical reasons only. What is here:
//! profiles of primitives, translation and rotation matrices, arcs from bulge values, traversal of sketch
//! contours, decomposition of intersecting curves into regions, and Hermite tessellation of splines.

use super::*;

/// The profile of a rectangle centred at the origin, for a box primitive; flat in XY.
pub(super) fn rect_profile(dx: f64, dy: f64) -> Vec<f64> {
    let (hx, hy) = (dx.abs() / 2.0, dy.abs() / 2.0);
    vec![-hx, -hy, hx, -hy, hx, hy, -hx, hy]
}

/// A 3×4 row-major translation matrix, used by a linear pattern.
pub(super) fn translate_mat(dx: f64, dy: f64, dz: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, dx, 0.0, 1.0, 0.0, dy, 0.0, 0.0, 1.0, dz]
}

/// A 3×4 row-major matrix rotating about the Z axis by `deg` degrees, used by a circular pattern.
pub(super) fn rotate_z_mat(deg: f64) -> [f64; 12] {
    let (s, c) = deg.to_radians().sin_cos();
    [c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
}

/// A rotation by `deg` about an arbitrary axis, passing through `origin` in the direction `dir`, as a 3×4
/// affine matrix built by the Rodrigues formula. Used by a circular pattern about a datum axis. A degenerate
/// `dir` falls back to a rotation about Z.
pub(super) fn rot_about_axis(origin: [f64; 3], dir: [f64; 3], deg: f64) -> [f64; 12] {
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if l < 1e-9 {
        return rotate_z_mat(deg);
    }
    let k = [dir[0] / l, dir[1] / l, dir[2] / l];
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    // the linear part R, a 3×3 Rodrigues matrix, row-major
    let r = [
        c + k[0] * k[0] * t,
        k[0] * k[1] * t - k[2] * s,
        k[0] * k[2] * t + k[1] * s,
        k[1] * k[0] * t + k[2] * s,
        c + k[1] * k[1] * t,
        k[1] * k[2] * t - k[0] * s,
        k[2] * k[0] * t - k[1] * s,
        k[2] * k[1] * t + k[0] * s,
        c + k[2] * k[2] * t,
    ];
    // translation = origin − R·origin, which keeps the axis fixed
    let ro = [
        r[0] * origin[0] + r[1] * origin[1] + r[2] * origin[2],
        r[3] * origin[0] + r[4] * origin[1] + r[5] * origin[2],
        r[6] * origin[0] + r[7] * origin[1] + r[8] * origin[2],
    ];
    [r[0], r[1], r[2], origin[0] - ro[0], r[3], r[4], r[5], origin[1] - ro[1], r[6], r[7], r[8], origin[2] - ro[2]]
}



/// A 3×4 row-major matrix rotating about the world axis `axis` (0 = X, 1 = Y, 2 = Z) by `deg` degrees.
pub(super) fn rot_axis_mat(axis: u8, deg: f64) -> [f64; 12] {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [1.0, 0.0, 0.0, 0.0, 0.0, c, -s, 0.0, 0.0, s, c, 0.0],
        1 => [c, 0.0, s, 0.0, 0.0, 1.0, 0.0, 0.0, -s, 0.0, c, 0.0],
        _ => [c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    }
}

/// The profile of a regular n-gon with circumscribed radius r, for a prism; flat in XY.
pub(super) fn polygon_profile(r: f64, n: u32) -> Vec<f64> {
    let n = n.max(3);
    (0..n)
        .flat_map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [r * a.cos(), r * a.sin()]
        })
        .collect()
}

/// The centre and the direction of an arc from its endpoints and a bulge value of `tan(θ/4)`; returns
/// (cx, cy, ccw).
pub(super) fn arc_center_from_bulge(p0x: f64, p0y: f64, p1x: f64, p1y: f64, bulge: f64) -> (f64, f64, bool) {
    let theta = 4.0 * bulge.atan();
    let chord = ((p1x - p0x).powi(2) + (p1y - p0y).powi(2)).sqrt().max(1e-9);
    let half = theta / 2.0;
    let radius = chord / 2.0 / half.sin();
    let (ux, uy) = ((p1x - p0x) / chord, (p1y - p0y) / chord);
    let (nx, ny) = (-uy, ux); // the left normal
    let apo = radius * half.cos();
    let (mx, my) = ((p0x + p1x) / 2.0, (p0y + p1y) / 2.0);
    (mx + nx * apo, my + ny * apo, bulge > 0.0)
}

/// Stitch lines and arcs into closed loops carrying bulge values, for an arc-aware offset. Each vertex is the
/// start of a segment; the bulge is 0 for a line and `tan(θ/4)`, signed by the direction, for an arc.
pub(super) fn entity_bulge_loops(pts: &[SketchPoint], ents: &[SketchEntity]) -> Vec<Vec<crate::offset::BVert>> {
    use std::f64::consts::TAU;
    let pt = |id: Id| pts.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let segs: Vec<(usize, Id, Id)> = ents
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.construction)
        .filter_map(|(i, e)| match e.kind {
            EntityKind::Line { a, b } => Some((i, a, b)),
            EntityKind::Arc { a, b, .. } => Some((i, a, b)),
            _ => None,
        })
        .collect();
    let mut adj: std::collections::HashMap<Id, Vec<usize>> = std::collections::HashMap::new();
    for (k, &(_, a, b)) in segs.iter().enumerate() {
        adj.entry(a).or_default().push(k);
        adj.entry(b).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    let mut out: Vec<Vec<crate::offset::BVert>> = Vec::new();
    for start_k in 0..segs.len() {
        if used[start_k] {
            continue;
        }
        let (_, sa, _) = segs[start_k];
        let mut verts: Vec<crate::offset::BVert> = Vec::new();
        let (mut cur_pt, mut cur_k) = (sa, start_k);
        let mut closed = false;
        loop {
            used[cur_k] = true;
            let (ei, a, b) = segs[cur_k];
            let to = if cur_pt == a { b } else { a };
            let (Some((fx, fy)), Some((_tx, _ty))) = (pt(cur_pt), pt(to)) else { break };
            let bulge = match ents[ei].kind {
                EntityKind::Arc { center, a: aa, b: _bb, ccw } => {
                    if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (pt(center), pt(cur_pt), pt(to)) {
                        let (sa_, ea_) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
                        let dir_ccw = if cur_pt == aa { ccw } else { !ccw };
                        let sweep = if dir_ccw { (ea_ - sa_).rem_euclid(TAU) } else { -((sa_ - ea_).rem_euclid(TAU)) };
                        (sweep / 4.0).tan()
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            verts.push(crate::offset::BVert { x: fx, y: fy, bulge });
            cur_pt = to;
            if cur_pt == sa {
                closed = true;
                break;
            }
            match adj.get(&cur_pt).and_then(|v| v.iter().copied().find(|&k| !used[k])) {
                Some(nk) => cur_k = nk,
                None => break,
            }
        }
        if closed && verts.len() >= 2 {
            out.push(verts);
        }
    }
    out
}

/// A closed contour for an ellipse, from its centre and the endpoints of its semi-axes. The major semi-axis is
/// c→ma and the minor one is perpendicular to it with length |c−mi|. The tessellation adapts to the larger
/// semi-axis.
pub(super) fn ellipse_contour(pc: Point2, pma: Point2, pmi: Point2) -> Contour {
    let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(0.001);
    let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(0.001);
    let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major); // unit vector of the major axis
    let (vx, vy) = (-uy, ux); // unit vector of the minor axis, perpendicular to it
    let maxax = major.max(minor);
    let sag = (maxax * 0.0016).clamp(0.003, 0.05);
    let dtheta = 2.0 * (1.0 - sag / maxax).clamp(-1.0, 1.0).acos();
    let n = (std::f64::consts::TAU / dtheta.max(1e-3)).ceil().clamp(24.0, 512.0) as usize;
    let pts: Vec<Point2> = (0..n)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / n as f64;
            let (ct, st) = (t.cos(), t.sin());
            Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy)
        })
        .collect();
    Contour { points: pts, closed: true, edges: Vec::new(), edge_src: Vec::new() }
}

pub(super) fn tessellate_sketch_multi(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<Contour> {
    let pt = |id: Id| points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
    let mut out: Vec<Contour> = Vec::new();
    // circles are closed contours of their own
    for e in entities {
        if e.construction {
            continue;
        }
        if let EntityKind::Circle { center, r } = e.kind {
            if let Some(c) = pt(center) {
                let rr = r.max(0.01);
                // a chord sag of about r/600 keeps the circle visually smooth at any radius
                let sag = (rr * 0.0016).clamp(0.003, 0.05);
                out.push(crate::geom::circle_contour_from(c.x, c.y, rr, sag, e.id));
            }
        }
        // an ellipse is a closed contour of its own: an analytic entity with adaptive tessellation
        if let EntityKind::Ellipse { c, ma, mi } = e.kind {
            if let (Some(pc), Some(pma), Some(pmi)) = (pt(c), pt(ma), pt(mi)) {
                out.push(ellipse_contour(pc, pma, pmi));
            }
        }
    }
    // lines and arcs are gathered into loops by the connectivity of their endpoints
    let segs: Vec<(usize, Id, Id)> = entities
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.construction)
        .filter_map(|(i, e)| match e.kind {
            EntityKind::Line { a, b } => Some((i, a, b)),
            EntityKind::Arc { a, b, .. } => Some((i, a, b)),
            _ => None,
        })
        .collect();
    // Positional welding of a loop: endpoints within `WELD_TOL` count as one vertex. The profile then closes
    // even when adjacent lines end at different, unwelded nodes at the same position — a common case at a
    // corner sitting on the origin, which the "merge coincident points" command leaves alone as a system point.
    // Otherwise the loop tears and a visually closed figure refuses to extrude. Adjacency and comparisons go
    // through the cluster representative `cl`, while the points of the loop are taken from actual coordinates.
    const WELD_TOL2: f64 = 1e-3 * 1e-3;
    let mut rep: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
    {
        let mut anchors: Vec<(Id, Point2)> = Vec::new(); // (representative, position)
        let mut ids: Vec<Id> = segs.iter().flat_map(|&(_, a, b)| [a, b]).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let Some(p) = pt(id) else {
                rep.insert(id, id);
                continue;
            };
            match anchors.iter().find(|(_, ap)| (ap.x - p.x).powi(2) + (ap.y - p.y).powi(2) < WELD_TOL2) {
                Some(&(r, _)) => {
                    rep.insert(id, r);
                }
                None => {
                    anchors.push((id, p));
                    rep.insert(id, id);
                }
            }
        }
    }
    let cl = |id: Id| rep.get(&id).copied().unwrap_or(id);
    let mut adj: std::collections::HashMap<Id, Vec<usize>> = std::collections::HashMap::new();
    for (k, &(_, a, b)) in segs.iter().enumerate() {
        adj.entry(cl(a)).or_default().push(k);
        adj.entry(cl(b)).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    for start_k in 0..segs.len() {
        if used[start_k] {
            continue;
        }
        let (_, sa, _) = segs[start_k];
        let mut loop_pts: Vec<Point2> = Vec::new();
        let mut loop_edges: Vec<crate::geom::ProfEdge> = Vec::new(); // exact edges, lines and arcs, for the B-rep
        let mut loop_src: Vec<Id> = Vec::new(); // provenance of each edge: the id of the sketch entity
        if let Some(p) = pt(sa) {
            loop_pts.push(p);
        }
        let mut cur_pt = sa;
        let mut cur_k = start_k;
        let mut closed = false;
        loop {
            used[cur_k] = true;
            let (ei, a, b) = segs[cur_k];
            let to = if cl(cur_pt) == cl(a) { b } else { a };
            match entities[ei].kind {
                EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(aa), pt(bb)) {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let (from_pt, to_pt, dir) = if cl(cur_pt) == cl(aa) { (pa, pb, ccw) } else { (pb, pa, !ccw) };
                        let a0 = (from_pt.y - c.y).atan2(from_pt.x - c.x);
                        let a1 = (to_pt.y - c.y).atan2(to_pt.x - c.x);
                        let arc = crate::geom::tessellate_arc(c.x, c.y, r, a0, a1, dir, 0.05);
                        loop_pts.extend(arc.into_iter().skip(1));
                        loop_edges.push(crate::geom::ProfEdge::Arc { a: from_pt, b: to_pt, center: c, ccw: dir });
                        loop_src.push(entities[ei].id);
                    }
                }
                _ => {
                    if let (Some(fp), Some(tp)) = (pt(cur_pt), pt(to)) {
                        loop_pts.push(tp);
                        loop_edges.push(crate::geom::ProfEdge::Line { a: fp, b: tp });
                        loop_src.push(entities[ei].id);
                    }
                }
            }
            let from_v = cur_pt;
            cur_pt = to;
            if cl(cur_pt) == cl(sa) {
                closed = true;
                break;
            }
            // Choosing the next edge at a branch point: above degree 2 a consistent turn is taken, the first
            // edge clockwise from the incoming direction, which traces the minimal face rather than an
            // arbitrary first edge — with an arbitrary choice, loops at shared corners and T-junctions were
            // assembled haphazardly and refused to extrude. At degree 2 or below, which covers every ordinary
            // figure, there is a single candidate and the behaviour is unchanged.
            let cands: Vec<usize> = adj.get(&cl(cur_pt)).map(|v| v.iter().copied().filter(|&k| !used[k]).collect()).unwrap_or_default();
            let next = if cands.len() <= 1 {
                cands.first().copied()
            } else if let (Some(pc), Some(pp)) = (pt(cur_pt), pt(from_v)) {
                let back = (pp.y - pc.y).atan2(pp.x - pc.x); // the direction back, where we came from
                cands.iter().copied().min_by(|&k1, &k2| {
                    let cw = |k: usize| -> f64 {
                        let (_, a, b) = segs[k];
                        let other = if cl(a) == cl(cur_pt) { b } else { a };
                        let po = pt(other).unwrap_or(pc);
                        let ang = (po.y - pc.y).atan2(po.x - pc.x);
                        let mut d = (back - ang).rem_euclid(std::f64::consts::TAU); // clockwise angle from the back direction
                        if d < 1e-6 { d += std::f64::consts::TAU; } // the edge we came along goes last in the queue
                        d
                    };
                    cw(k1).partial_cmp(&cw(k2)).unwrap_or(std::cmp::Ordering::Equal)
                })
            } else {
                cands.first().copied()
            };
            match next {
                Some(k) => cur_k = k,
                None => break,
            }
        }
        // The closing point coincided with the first one, within the welding tolerance: drop the duplicate and
        // pull the end of the last exact edge onto the first point, closing the micro-gap at the node. Otherwise
        // the wire handed to the B-rep kernel stays open and a visually closed figure refuses to extrude. The
        // threshold 1e-3 is √WELD_TOL2.
        if closed && loop_pts.len() >= 2 {
            if let (Some(f), Some(l)) = (loop_pts.first().copied(), loop_pts.last().copied()) {
                if (f.x - l.x).abs() < 1e-3 && (f.y - l.y).abs() < 1e-3 {
                    loop_pts.pop();
                    match loop_edges.last_mut() {
                        Some(crate::geom::ProfEdge::Line { b, .. }) | Some(crate::geom::ProfEdge::Arc { b, .. }) => *b = f,
                        _ => {} // a circle is whole and has no gap
                    }
                }
            }
        }
        if loop_pts.len() >= 2 {
            let mut c = if closed { Contour::closed(loop_pts) } else { Contour::open(loop_pts) };
            c.edges = loop_edges; // exact edges: profiles of bodies when closed, sweep paths when open
            c.edge_src = loop_src; // provenance of the edges, which is what faces are named from; one rule for every contour
            out.push(c);
        }
    }
    out
}

/// A curve for the planar arrangement, in its internal representation.
#[derive(Clone, Copy)]
pub(super) enum ArrCurve {
    Line { a: (f64, f64), b: (f64, f64) },
    Circle { c: (f64, f64), r: f64 },
    Arc { c: (f64, f64), r: f64, a0: f64, a1: f64, ccw: bool, pa: (f64, f64), pb: (f64, f64) },
}

/// The world-space intersection points of two arrangement curves that lie on both of them, inside the segment
/// or the span of the arc.
pub(super) fn arr_intersect(x: ArrCurve, y: ArrCurve) -> Vec<(f64, f64)> {
    use ArrCurve::*;
    let on_seg = |a: (f64, f64), b: (f64, f64), t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let arc_ok = |c: (f64, f64), a0: f64, a1: f64, ccw: bool, p: (f64, f64)| angle_in_arc((p.1 - c.1).atan2(p.0 - c.0), a0, a1, ccw);
    match (x, y) {
        (Line { a, b }, Line { a: c, b: d }) => match seg_seg_t(a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1) {
            Some(t) => vec![on_seg(a, b, t)],
            None => {
                // Collinear overlap, as with two rectangles sharing a side: there is no point intersection,
                // but the segments lie on top of each other, so both are cut at the ends of the overlap.
                // Otherwise the arrangement loses regions. Coincident sub-edges are deduplicated below, when
                // the edges are assembled.
                let (ux, uy) = (b.0 - a.0, b.1 - a.1);
                let len2 = ux * ux + uy * uy;
                if len2 < 1e-18 {
                    return Vec::new();
                }
                let cross1 = ux * (c.1 - a.1) - uy * (c.0 - a.0);
                let cross2 = ux * (d.1 - a.1) - uy * (d.0 - a.0);
                let tol = 1e-6 * len2.sqrt();
                if cross1.abs() > tol || cross2.abs() > tol {
                    return Vec::new(); // parallel, but not on the same line
                }
                let mut out = Vec::new();
                let mut push_in = |px: f64, py: f64, sa: (f64, f64), sb: (f64, f64)| {
                    let (vx, vy) = (sb.0 - sa.0, sb.1 - sa.1);
                    let l2 = vx * vx + vy * vy;
                    if l2 < 1e-18 {
                        return;
                    }
                    let t = ((px - sa.0) * vx + (py - sa.1) * vy) / l2;
                    if t > 1e-9 && t < 1.0 - 1e-9 {
                        out.push((px, py));
                    }
                };
                push_in(c.0, c.1, a, b);
                push_in(d.0, d.1, a, b);
                push_in(a.0, a.1, c, d);
                push_in(b.0, b.1, c, d);
                out
            }
        },
        // The tolerance lives in one place, inside `seg_circle_t`. A second filter at ±1e-9 used to stand here
        // and discarded a tangency that had already been found: the parameter of the tangency point came out as
        // −3.3e-6, which is 5e-5 mm before the start of the segment. Two thresholds answering one question is
        // the same ailment as one piece of knowledge kept in two places: relax one and the other keeps
        // cutting.
        (Line { a, b }, Circle { c, r }) | (Circle { c, r }, Line { a, b }) => seg_circle_t(a.0, a.1, b.0, b.1, c.0, c.1, r).into_iter().map(|t| on_seg(a, b, t)).collect(),
        (Line { a, b }, Arc { c, r, a0, a1, ccw, .. }) | (Arc { c, r, a0, a1, ccw, .. }, Line { a, b }) => seg_circle_t(a.0, a.1, b.0, b.1, c.0, c.1, r).into_iter().map(|t| on_seg(a, b, t)).filter(|&p| arc_ok(c, a0, a1, ccw, p)).collect(),
        (Circle { c: c1, r: r1 }, Circle { c: c2, r: r2 }) => circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2),
        (Circle { c: c1, r: r1 }, Arc { c: c2, r: r2, a0, a1, ccw, .. }) | (Arc { c: c2, r: r2, a0, a1, ccw, .. }, Circle { c: c1, r: r1 }) => circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2).into_iter().filter(|&p| arc_ok(c2, a0, a1, ccw, p)).collect(),
        (Arc { c: c1, r: r1, a0: s1, a1: e1, ccw: w1, .. }, Arc { c: c2, r: r2, a0: s2, a1: e2, ccw: w2, .. }) => circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2).into_iter().filter(|&p| arc_ok(c1, s1, e1, w1, p) && arc_ok(c2, s2, e2, w2, p)).collect(),
    }
}

/// The planar arrangement of a sketch: every entity — lines, arcs and circles — is intersected with every
/// other, the curves are cut at the intersection points, and the minimal faces, that is the closed regions, are
/// assembled. This is what turns the strip between two circles, cut by a line, into a selectable closed region.
/// Returns the faces as contours, carrying exact `ProfEdge` edges for the B-rep. Narrow in scope: lines, arcs
/// and circles, but not ellipses.
pub(super) fn arrangement_regions(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<Contour> {
    arrangement_regions_prov(points, entities).into_iter().map(|(c, _)| c).collect()
}

/// As `arrangement_regions`, but each region carries its provenance: the sorted set of entity ids its boundary
/// is stitched from. This is what keeps the identity of a contour stable across edits, so that loops which
/// swapped places do not take over each other's ids.
pub(super) fn arrangement_regions_prov(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<(Contour, Vec<Id>)> {
    use std::f64::consts::{PI, TAU};
    let pt = |id: Id| points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let mut curves: Vec<ArrCurve> = Vec::new();
    let mut curve_eid: Vec<Id> = Vec::new(); // provenance of a curve, its entity id, parallel to `curves`
    let mut out_ellipse: Vec<(Contour, Vec<Id>)> = Vec::new(); // ellipses are regions of their own, outside the arrangement graph
    for e in entities {
        if e.construction {
            continue;
        }
        match e.kind {
            EntityKind::Line { a, b } => {
                if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                    if (pa.0 - pb.0).hypot(pa.1 - pb.1) > 1e-9 {
                        curves.push(ArrCurve::Line { a: pa, b: pb });
                        curve_eid.push(e.id);
                    }
                }
            }
            EntityKind::Circle { center, r } => {
                if let Some(c) = pt(center) {
                    curves.push(ArrCurve::Circle { c, r: r.max(1e-6) });
                    curve_eid.push(e.id);
                }
            }
            EntityKind::Arc { center, a, b, ccw } => {
                if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                    let r = (pa.0 - c.0).hypot(pa.1 - c.1);
                    if r > 1e-9 {
                        curves.push(ArrCurve::Arc { c, r, a0: (pa.1 - c.1).atan2(pa.0 - c.0), a1: (pb.1 - c.1).atan2(pb.0 - c.0), ccw, pa, pb });
                        curve_eid.push(e.id);
                    }
                }
            }
            EntityKind::Ellipse { c, ma, mi } => {
                // an ellipse is a closed region of its own; its intersections are not part of the arrangement yet
                if let (Some(pc), Some(pma), Some(pmi)) = (pt(c), pt(ma), pt(mi)) {
                    out_ellipse.push((ellipse_contour(Point2::new(pc.0, pc.1), Point2::new(pma.0, pma.1), Point2::new(pmi.0, pmi.1)), vec![e.id]));
                }
            }
        }
    }
    let nc = curves.len();
    let mut out: Vec<(Contour, Vec<Id>)> = std::mem::take(&mut out_ellipse);
    if nc == 0 {
        return out;
    }
    let mut cuts: Vec<Vec<(f64, f64)>> = vec![Vec::new(); nc];
    for i in 0..nc {
        for j in (i + 1)..nc {
            for p in arr_intersect(curves[i], curves[j]) {
                cuts[i].push(p);
                cuts[j].push(p);
            }
        }
    }
    let mut nodes: Vec<(f64, f64)> = Vec::new();
    fn weld(nodes: &mut Vec<(f64, f64)>, p: (f64, f64)) -> usize {
        for (k, q) in nodes.iter().enumerate() {
            if (q.0 - p.0).hypot(q.1 - p.1) < 1e-4 {
                return k;
            }
        }
        nodes.push(p);
        nodes.len() - 1
    }
    // a directed edge of the arrangement: a line or an arc, where cx, cy, r and ccw describe the direction
    // from `from` to `to`
    struct DE { from: usize, to: usize, line: bool, cx: f64, cy: f64, r: f64, ccw: bool, ci: usize }
    let mut edges: Vec<DE> = Vec::new();
    let mut seen_line: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new(); // B28
    for (ci, cu) in curves.iter().enumerate() {
        match *cu {
            ArrCurve::Line { a, b } => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len2 = dx * dx + dy * dy;
                let mut ts: Vec<f64> = vec![0.0, 1.0];
                for &p in &cuts[ci] {
                    ts.push(((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2);
                }
                ts.retain(|t| *t > -1e-9 && *t < 1.0 + 1e-9);
                ts.sort_by(|x, y| x.total_cmp(y));
                ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                for w in ts.windows(2) {
                    let p0 = (a.0 + dx * w[0], a.1 + dy * w[0]);
                    let p1 = (a.0 + dx * w[1], a.1 + dy * w[1]);
                    let (n0, n1) = (weld(&mut nodes, p0), weld(&mut nodes, p1));
                    // Collinearly overlapping pieces of two lines collapse into one and the same segment
                    // between the same nodes, and a duplicate breaks the twin pairing of the traversal. Only
                    // one is kept.
                    if n0 != n1 && seen_line.insert((n0.min(n1), n0.max(n1))) {
                        edges.push(DE { from: n0, to: n1, line: true, cx: 0.0, cy: 0.0, r: 0.0, ccw: true, ci });
                        edges.push(DE { from: n1, to: n0, line: true, cx: 0.0, cy: 0.0, r: 0.0, ccw: true, ci });
                    }
                }
            }
            ArrCurve::Circle { c, r } => {
                if cuts[ci].is_empty() {
                    out.push((crate::geom::circle_contour_from(c.0, c.1, r, (r * 0.0016).clamp(0.003, 0.05), curve_eid[ci]), vec![curve_eid[ci]])); // a whole circle is a region in itself
                    continue;
                }
                // The actual cut points, rather than points reconstructed from an angle: for a line and a
                // circle this is one and the same intersection point, so the nodes weld exactly.
                // Reconstructing from R left a discrepancy on the order of the residual.
                let mut cw: Vec<(f64, (f64, f64))> = cuts[ci].iter().map(|&p| ((p.1 - c.1).atan2(p.0 - c.0).rem_euclid(TAU), p)).collect();
                cw.sort_by(|x, y| x.0.total_cmp(&y.0));
                cw.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
                // A single cut, that is a lone tangency, does not divide a circle: a span from a node to
                // itself was discarded below and the circle vanished from the regions. A tangency is not a
                // cut, and the circle stays whole.
                if cw.len() < 2 {
                    out.push((crate::geom::circle_contour_from(c.0, c.1, r, (r * 0.0016).clamp(0.003, 0.05), curve_eid[ci]), vec![curve_eid[ci]]));
                    continue;
                }
                let m = cw.len();
                for k in 0..m {
                    let (_, p0) = cw[k];
                    let (_, p1) = cw[(k + 1) % m];
                    let (n0, n1) = (weld(&mut nodes, p0), weld(&mut nodes, p1));
                    if n0 != n1 {
                        edges.push(DE { from: n0, to: n1, line: false, cx: c.0, cy: c.1, r, ccw: true, ci });
                        edges.push(DE { from: n1, to: n0, line: false, cx: c.0, cy: c.1, r, ccw: false, ci });
                    }
                }
            }
            ArrCurve::Arc { c, r, a0, ccw, pa, pb, .. } => {
                let sweep = {
                    let a1 = (pb.1 - c.1).atan2(pb.0 - c.0);
                    if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) }
                };
                let to_param = |p: (f64, f64)| {
                    let ang = (p.1 - c.1).atan2(p.0 - c.0);
                    if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) }
                };
                // (parameter along the arc, the actual point): the ends of the arc plus the cuts inside the span
                let mut aw: Vec<(f64, (f64, f64))> = vec![(0.0, pa), (sweep, pb)];
                for &p in &cuts[ci] {
                    let pr = to_param(p);
                    if pr > 1e-9 && pr < sweep - 1e-9 {
                        aw.push((pr, p));
                    }
                }
                aw.sort_by(|x, y| x.0.total_cmp(&y.0));
                aw.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
                for w in aw.windows(2) {
                    let (n0, n1) = (weld(&mut nodes, w[0].1), weld(&mut nodes, w[1].1));
                    if n0 != n1 {
                        edges.push(DE { from: n0, to: n1, line: false, cx: c.0, cy: c.1, r, ccw, ci });
                        edges.push(DE { from: n1, to: n0, line: false, cx: c.0, cy: c.1, r, ccw: !ccw, ci });
                    }
                }
            }
        }
    }
    if edges.is_empty() {
        return out;
    }
    if std::env::var("QYM_ARR_DEBUG").is_ok() {
        eprintln!("[arr] curves={nc} nodes={} edges={}", nodes.len(), edges.len());
        for (ci, c) in cuts.iter().enumerate() {
            eprintln!("[arr] cuts[{ci}]={:?}", c.iter().map(|p| (p.0 * 100.0).round() / 100.0).collect::<Vec<_>>());
        }
    }
    // tangent angle, either leaving `from` when `at_from` is true, or entering `to`; in the direction of travel
    let tang = |e: &DE, at_from: bool| -> f64 {
        if e.line {
            let (o0, o1) = (nodes[e.from], nodes[e.to]);
            (o1.1 - o0.1).atan2(o1.0 - o0.0)
        } else {
            let node = if at_from { nodes[e.from] } else { nodes[e.to] };
            let theta = (node.1 - e.cy).atan2(node.0 - e.cx);
            theta + if e.ccw { PI / 2.0 } else { -PI / 2.0 }
        }
    };
    let mut out_adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (ei, e) in edges.iter().enumerate() {
        out_adj[e.from].push(ei);
    }
    // Signed curvature of a half-edge: 0 for a line and ±1/r for an arc, positive when counter-clockwise.
    // It breaks ties at tangencies: where the tangents coincide, as with a line tangent to an arc or two
    // tangent arcs, the first-order angle is the same and the direction is decided at second order by the
    // curvature — a larger κ leans slightly left and therefore comes earlier clockwise from the back
    // direction.
    let curv = |e: &DE| -> f64 {
        if e.line {
            0.0
        } else if e.ccw {
            1.0 / e.r
        } else {
            -1.0 / e.r
        }
    };
    // Tracing the faces: at a node the next half-edge is the one with the smallest clockwise turn from the
    // back direction, which keeps the face on the left, so interior faces come out counter-clockwise with a
    // positive area while the outer face comes out clockwise and is discarded.
    //
    // The reversal is found by the twin index — edges are stored in pairs, so the twin is `ei ^ 1` — rather
    // than by an angle below 1e-9: at a tangency a different edge carries the same angle and used to be
    // swallowed as if it were the reversal, which cost the arrangement whole faces. A dumbbell of three
    // mutually tangent circles produced two regions instead of five.
    let mut used = vec![false; edges.len()];
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut face: Vec<usize> = Vec::new();
        let mut cur = start;
        let mut ok = false;
        for _ in 0..edges.len() + 2 {
            used[cur] = true;
            face.push(cur);
            let v = edges[cur].to;
            let back = tang(&edges[cur], false) + PI;
            let twin = cur ^ 1;
            // The correct half-edge step: the next edge of a face is the first one after the twin in the
            // clockwise circular order around the node. The angle used is effective, s_eff = s − κ·δ, where the
            // second-order term is the curvature; at a tangency the first-order terms coincide and only κ tells
            // the sides apart.
            //
            // Taking the minimum s with the twin excluded put the choice on the wrong edge at tangencies, where
            // a line and an arc share a tangent and s is near zero, and the arrangement lost faces — the
            // dumbbell case dropped to no regions at all.
            const CURV_DELTA: f64 = 1e-7;
            let s_eff = |oe: usize| -> f64 {
                let s = (back - tang(&edges[oe], true)).rem_euclid(TAU);
                (s - curv(&edges[oe]) * CURV_DELTA).rem_euclid(TAU)
            };
            let st = s_eff(twin);
            let mut best: Option<(f64, usize)> = None; // (clockwise cyclic distance from the twin, edge)
            for &oe in &out_adj[v] {
                if oe == twin {
                    continue; // the reversal is taken only when there is nowhere else to go
                }
                let d = (s_eff(oe) - st).rem_euclid(TAU);
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, oe));
                }
            }
            let Some(nx) = best.map(|(_, e)| e).or(Some(twin).filter(|&t| t < edges.len() && edges[t].from == v)) else { break };
            if nx == start {
                ok = true;
                break;
            }
            if used[nx] {
                break;
            }
            cur = nx;
        }
        if std::env::var("QYM_ARR_DEBUG").is_ok() {
            let path: Vec<String> = face.iter().map(|&ei| { let e = &edges[ei]; format!("{}->{}{}", e.from, e.to, if e.line { "L" } else if e.ccw { "+" } else { "-" }) }).collect();
            eprintln!("[arr] walk start={start} ok={ok} len={} path={}", face.len(), path.join(" "));
        }
        if !ok || face.len() < 2 {
            continue;
        }
        // face to contour: points plus exact edges
        let mut cpts: Vec<Point2> = Vec::new();
        let mut cedges: Vec<crate::geom::ProfEdge> = Vec::new();
        let mut csrc: Vec<Id> = Vec::new(); // provenance of each edge: the id of the sketch entity
        for (k, &ei) in face.iter().enumerate() {
            let e = &edges[ei];
            let (a, b) = (nodes[e.from], nodes[e.to]);
            let (pa, pb) = (Point2::new(a.0, a.1), Point2::new(b.0, b.1));
            if e.line {
                if k == 0 {
                    cpts.push(pa);
                }
                cpts.push(pb);
                cedges.push(crate::geom::ProfEdge::Line { a: pa, b: pb });
                csrc.push(curve_eid[e.ci]);
            } else {
                let (g0, g1) = ((a.1 - e.cy).atan2(a.0 - e.cx), (b.1 - e.cy).atan2(b.0 - e.cx));
                let arc = crate::geom::tessellate_arc(e.cx, e.cy, e.r, g0, g1, e.ccw, 0.03);
                if k == 0 {
                    if let Some(f) = arc.first() {
                        cpts.push(*f);
                    }
                }
                cpts.extend(arc.iter().skip(1).cloned());
                cedges.push(crate::geom::ProfEdge::Arc { a: pa, b: pb, center: Point2::new(e.cx, e.cy), ccw: e.ccw });
                csrc.push(curve_eid[e.ci]);
            }
        }
        if let (Some(f), Some(l)) = (cpts.first().copied(), cpts.last().copied()) {
            if (f.x - l.x).abs() < 1e-6 && (f.y - l.y).abs() < 1e-6 {
                cpts.pop();
            }
        }
        let mut cont = Contour::closed(cpts);
        cont.edges = cedges;
        cont.edge_src = csrc;
        if cont.signed_area() > 1e-3 {
            // provenance of a face: the set of entities whose pieces form its boundary, sorted and without
            // repeats
            let prov: std::collections::BTreeSet<Id> = face.iter().map(|&ei| curve_eid[edges[ei].ci]).collect();
            out.push((cont, prov.into_iter().collect())); // an interior face, counter-clockwise; the outer one and degenerate results were dropped
        }
    }
    out
}

/// A smooth Catmull-Rom curve through the control points, as a contour.
///
/// The tangent, that is the Hermite vector dP/dt, at spline node `i`: taken explicitly when given, otherwise
/// the automatic Catmull-Rom value `(p_{i+1} − p_{i-1})/2`, one-sided at the ends of an open spline. It governs
/// the shape of the curve.
pub(super) fn spline_tangent_at(pts: &[Point2], tangents: &[Option<[f64; 2]>], i: usize, closed: bool) -> Point2 {
    if let Some(Some([x, y])) = tangents.get(i) {
        return Point2::new(*x, *y);
    }
    let n = pts.len() as i64;
    let get = |k: i64| -> Point2 {
        if closed {
            pts[((k % n + n) % n) as usize]
        } else {
            pts[k.clamp(0, n - 1) as usize]
        }
    };
    let (a, b) = (get(i as i64 - 1), get(i as i64 + 1));
    Point2::new((b.x - a.x) * 0.5, (b.y - a.y) * 0.5)
}

/// A point on a cubic Hermite segment: nodes p0 and p1 with tangents m0 and m1, at t ∈ [0,1].
pub(super) fn hermite_pt(p0: Point2, m0: Point2, p1: Point2, m1: Point2, t: f64) -> Point2 {
    let (t2, t3) = (t * t, t * t * t);
    let (h00, h10, h01, h11) = (2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2);
    Point2::new(h00 * p0.x + h10 * m0.x + h01 * p1.x + h11 * m1.x, h00 * p0.y + h10 * m0.y + h01 * p1.y + h11 * m1.y)
}

/// Adaptive tessellation of a Hermite segment, driven by chord sag as with Catmull-Rom.
#[allow(clippy::too_many_arguments)]
pub(super) fn hermite_seg_adaptive(p0: Point2, m0: Point2, p1: Point2, m1: Point2, t0: f64, t1: f64, a: Point2, b: Point2, tol: f64, depth: u8, out: &mut Vec<Point2>) {
    let tm = 0.5 * (t0 + t1);
    let m = hermite_pt(p0, m0, p1, m1, tm);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let dev = if len2 < 1e-18 { m.dist(a) } else { (dx * (m.y - a.y) - dy * (m.x - a.x)).abs() / len2.sqrt() };
    if depth == 0 || dev <= tol {
        out.push(b);
    } else {
        hermite_seg_adaptive(p0, m0, p1, m1, t0, tm, a, m, tol, depth - 1, out);
        hermite_seg_adaptive(p0, m0, p1, m1, tm, t1, m, b, tol, depth - 1, out);
    }
}

/// Tessellation of a spline as a cubic Hermite with tangents: a fit-point spline with handles.
pub(super) fn tessellate_spline_hermite(pts: &[Point2], tangents: &[Option<[f64; 2]>], closed: bool) -> Contour {
    let n = pts.len();
    if n < 2 {
        return if closed { Contour::closed(pts.to_vec()) } else { Contour::open(pts.to_vec()) };
    }
    let span: f64 = (0..n).map(|i| pts[i].dist(pts[(i + 1) % n])).sum::<f64>() / n as f64;
    let tol = (span * 0.004).clamp(0.002, 0.2);
    let seg = if closed { n } else { n - 1 };
    let mut out: Vec<Point2> = vec![pts[0]];
    for s in 0..seg {
        let (p0, p1) = (pts[s], pts[(s + 1) % n]);
        let (m0, m1) = (spline_tangent_at(pts, tangents, s, closed), spline_tangent_at(pts, tangents, (s + 1) % n, closed));
        hermite_seg_adaptive(p0, m0, p1, m1, 0.0, 1.0, p0, p1, tol, 10, &mut out);
    }
    if closed {
        out.pop();
        Contour::closed(out)
    } else {
        Contour::open(out)
    }
}

pub(super) fn bbox_of(contours: &[Contour]) -> Option<Bbox> {
    let mut acc: Option<Bbox> = None;
    for c in contours {
        if let Some(b) = c.bbox() {
            acc = Some(match acc {
                None => b,
                Some(a) => Bbox {
                    min: Point2::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
                    max: Point2::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
                },
            });
        }
    }
    acc
}
