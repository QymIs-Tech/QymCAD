//! 2D geometry of a sketch: profiles, tessellation, decomposition into regions, splines.
//!
//! Free functions, unrelated to `Project`; they lived in its file for historical reasons only. What is here:
//! profiles of primitives, translation and rotation matrices, arcs from bulge values, traversal of sketch
//! contours, decomposition of intersecting curves into regions, and Hermite tessellation of splines.

use super::*;
use crate::geom::ProfEdge;
use crate::model::curve::Curve;

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

/// A LINE OR AN ARC OF A CHAIN: its place among the entities, and the points at its two ends.
#[derive(Clone, Copy)]
struct Link {
    entity: usize,
    ends: [Id; 2],
}

/// The lines and arcs of `ents` drawn (not construction), each by its place and its ends: what loops and chains are
/// stitched from.
fn links(ents: &[SketchEntity]) -> Vec<Link> {
    ents.iter().enumerate().filter(|(_, e)| !e.construction).filter_map(|(entity, e)| e.kind.straight_or_round_ends().map(|ends| Link { entity, ends })).collect()
}

/// Stitch lines and arcs into closed loops carrying bulge values, for an arc-aware offset. Each vertex is the
/// start of a segment; the bulge is 0 for a line and `tan(θ/4)`, signed by the direction, for an arc.
pub(super) fn entity_bulge_loops(pts: &[SketchPoint], ents: &[SketchEntity]) -> Vec<Vec<crate::offset::BVert>> {
    use std::f64::consts::TAU;
    let pt = |id: Id| pts.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let segs = links(ents);
    let mut adj: std::collections::HashMap<Id, Vec<usize>> = std::collections::HashMap::new();
    for (k, &Link { ends: [a, b], .. }) in segs.iter().enumerate() {
        adj.entry(a).or_default().push(k);
        adj.entry(b).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    let mut out: Vec<Vec<crate::offset::BVert>> = Vec::new();
    for start_k in 0..segs.len() {
        if used[start_k] {
            continue;
        }
        let [sa, _] = segs[start_k].ends;
        let mut verts: Vec<crate::offset::BVert> = Vec::new();
        let (mut cur_pt, mut cur_k) = (sa, start_k);
        let mut closed = false;
        loop {
            used[cur_k] = true;
            let Link { entity: ei, ends: [a, b] } = segs[cur_k];
            let to = if cur_pt == a { b } else { a };
            let (Some((fx, fy)), Some((_tx, _ty))) = (pt(cur_pt), pt(to)) else { break };
            let bulge = match ents[ei].kind.as_arc() {
                Some(crate::model::ArcParts { center, a: aa, b: _bb, ccw }) => {
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

/// THE OPEN CHAINS OF LINES AND ARCS AMONG `ents`, each as a bulge polyline from one free end to the other - its last
/// vertex the far end, with no bulge. A chain is walked from an end no other of the curves meets; a closed loop has
/// none and is left to `entity_bulge_loops`.
pub(super) fn entity_bulge_chains(pts: &[SketchPoint], ents: &[SketchEntity]) -> Vec<Vec<crate::offset::BVert>> {
    use std::f64::consts::TAU;
    let pt = |id: Id| pts.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let segs = links(ents);
    let mut adj: std::collections::HashMap<Id, Vec<usize>> = std::collections::HashMap::new();
    for (k, &Link { ends: [a, b], .. }) in segs.iter().enumerate() {
        adj.entry(a).or_default().push(k);
        adj.entry(b).or_default().push(k);
    }
    // the free ends, in the order the curves were drawn: a chain starts where its first curve does
    let mut ends: Vec<Id> = Vec::new();
    for &Link { ends: [a, b], .. } in &segs {
        for p in [a, b] {
            if adj.get(&p).is_some_and(|v| v.len() == 1) && !ends.contains(&p) {
                ends.push(p);
            }
        }
    }
    let mut used = vec![false; segs.len()];
    let mut out: Vec<Vec<crate::offset::BVert>> = Vec::new();
    for start in ends {
        let Some(&first) = adj.get(&start).and_then(|v| v.first()) else { continue };
        if used[first] {
            continue;
        }
        let mut verts: Vec<crate::offset::BVert> = Vec::new();
        let (mut cur_pt, mut cur_k) = (start, first);
        loop {
            used[cur_k] = true;
            let Link { entity: ei, ends: [a, b] } = segs[cur_k];
            let to = if cur_pt == a { b } else { a };
            let Some((fx, fy)) = pt(cur_pt) else { break };
            let bulge = match ents[ei].kind.as_arc() {
                Some(crate::model::ArcParts { center, a: aa, ccw, .. }) => match (pt(center), pt(cur_pt), pt(to)) {
                    (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) => {
                        let (sa_, ea_) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
                        let dir_ccw = if cur_pt == aa { ccw } else { !ccw };
                        let sweep = if dir_ccw { (ea_ - sa_).rem_euclid(TAU) } else { -((sa_ - ea_).rem_euclid(TAU)) };
                        (sweep / 4.0).tan()
                    }
                    _ => 0.0,
                },
                _ => 0.0,
            };
            verts.push(crate::offset::BVert { x: fx, y: fy, bulge });
            cur_pt = to;
            match adj.get(&cur_pt).and_then(|v| v.iter().copied().find(|&k| !used[k])) {
                Some(nk) => cur_k = nk,
                None => break,
            }
        }
        if let Some((x, y)) = pt(cur_pt) {
            verts.push(crate::offset::BVert { x, y, bulge: 0.0 });
        }
        if verts.len() >= 2 {
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
            crate::geom::Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy)
        })
        .collect();
    Contour { points: pts, closed: true, edges: Vec::new(), edge_src: Vec::new() }
}

/// THE ECCENTRIC ANGLE OF POINT `p` on the ellipse of centre `pc`, major end `pma` and minor end `pmi`: the `t` of the
/// point `c + a cos t u + b sin t v` the line from the centre through `p` meets.
pub(crate) fn ellipse_angle(axes: [Point2; 3], p: Point2) -> f64 {
    let [pc, pma, pmi] = axes;
    let (a, b) = (pc.dist(pma).max(1e-9), pc.dist(pmi).max(1e-9));
    let (u, v) = (pma - pc, pmi - pc);
    let d = p - pc;
    ((d.x * v.x + d.y * v.y) / (b * b)).atan2((d.x * u.x + d.y * u.y) / (a * a))
}

/// THE POINTS AN ARC OF AN ELLIPSE IS LAID OUT BY, from the eccentric angle of `from` to that of `to` the way `winding`
/// goes, at the sag of the whole ellipse (`ellipse_contour`). The minor end need not stand square to the major one in
/// the middle of a solve: the points follow the two ends as they stand.
pub(crate) fn ellipse_arc_points(axes: [Point2; 3], ends: [Point2; 2], winding: crate::feature::Winding) -> Vec<Point2> {
    let [pc, pma, pmi] = axes;
    let (t0, t1) = (ellipse_angle(axes, ends[0]), ellipse_angle(axes, ends[1]));
    let tau = std::f64::consts::TAU;
    let sweep = match winding {
        crate::feature::Winding::Ccw => (t1 - t0).rem_euclid(tau),
        crate::feature::Winding::Cw => -(t0 - t1).rem_euclid(tau),
    };
    let whole = ellipse_contour(pc, pma, pmi).points.len().max(24);
    let n = ((sweep.abs() / tau) * whole as f64).ceil().max(2.0) as usize;
    let (u, v) = (pma - pc, pmi - pc);
    (0..=n)
        .map(|k| {
            let t = t0 + sweep * k as f64 / n as f64;
            crate::geom::Point2::new(pc.x + u.x * t.cos() + v.x * t.sin(), pc.y + u.y * t.cos() + v.y * t.sin())
        })
        .collect()
}

pub(super) fn tessellate_sketch_multi(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<Contour> {
    let at: std::collections::HashMap<Id, Point2> = points.iter().map(|p| (p.id, crate::geom::Point2::new(p.x, p.y))).collect();
    let pt = |id: Id| at.get(&id).copied();
    let mut out: Vec<Contour> = Vec::new();
    // circles are closed contours of their own
    for e in entities {
        if e.construction {
            continue;
        }
        if let Some(crate::model::CircleParts { center, r }) = e.kind.as_circle() {
            if let Some(c) = pt(center) {
                let rr = r.max(0.01);
                // a chord sag of about r/600 keeps the circle visually smooth at any radius
                let sag = (rr * 0.0016).clamp(0.003, 0.05);
                out.push(crate::geom::circle_contour_from(c.x, c.y, rr, sag, e.id));
            }
        }
        // an ellipse is a closed contour of its own: an analytic entity with adaptive tessellation; an arc of one is an
        // open chain of its own
        if let Some(crate::model::EllipseParts { c, ma, mi, span }) = e.kind.as_ellipse() {
            match (pt(c), pt(ma), pt(mi), span) {
                (Some(pc), Some(pma), Some(pmi), None) => out.push(with_exact_edges(ellipse_contour(pc, pma, pmi), &pt, e)),
                (Some(pc), Some(pma), Some(pmi), Some(s)) => {
                    if let (Some(from), Some(to)) = (pt(s.from), pt(s.to)) {
                        out.push(with_exact_edges(Contour::open(ellipse_arc_points([pc, pma, pmi], [from, to], s.winding)), &pt, e));
                    }
                }
                _ => {}
            }
        }
    }
    // lines and arcs are gathered into loops by the connectivity of their endpoints
    let segs = links(entities);
    // Positional welding of a loop: endpoints within `WELD_TOL` count as one vertex. The profile then closes
    // even when adjacent lines end at different, unwelded nodes at the same position — a common case at a
    // corner sitting on the origin, which the "merge coincident points" command leaves alone as a system point.
    // Otherwise the loop tears and a visually closed figure refuses to extrude. Adjacency and comparisons go
    // through the cluster representative `cl`, while the points of the loop are taken from actual coordinates.
    const WELD_TOL2: f64 = 1e-3 * 1e-3;
    let mut rep: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
    {
        // the anchors welded by a grid as wide as the tolerance (`Welded`), each node standing for the first id at it
        let mut anchors = Welded::new(1e-3);
        let mut representative: Vec<Id> = Vec::new();
        let mut ids: Vec<Id> = segs.iter().flat_map(|l| l.ends).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let Some(p) = pt(id) else {
                rep.insert(id, id);
                continue;
            };
            let node = anchors.weld((p.x, p.y), |a, p| (a.0 - p.0).powi(2) + (a.1 - p.1).powi(2) < WELD_TOL2);
            if node == representative.len() {
                representative.push(id);
            }
            rep.insert(id, representative[node]);
        }
    }
    let cl = |id: Id| rep.get(&id).copied().unwrap_or(id);
    let mut adj: std::collections::HashMap<Id, Vec<usize>> = std::collections::HashMap::new();
    for (k, &Link { ends: [a, b], .. }) in segs.iter().enumerate() {
        adj.entry(cl(a)).or_default().push(k);
        adj.entry(cl(b)).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    for start_k in 0..segs.len() {
        if used[start_k] {
            continue;
        }
        let [sa, _] = segs[start_k].ends;
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
            let Link { entity: ei, ends: [a, b] } = segs[cur_k];
            let to = if cl(cur_pt) == cl(a) { b } else { a };
            match entities[ei].kind.as_arc() {
                Some(crate::model::ArcParts { center, a: aa, b: bb, ccw }) => {
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
                        let [a, b] = segs[k].ends;
                        let other = if cl(a) == cl(cur_pt) { b } else { a };
                        let po = pt(other).unwrap_or(pc);
                        let ang = (po.y - pc.y).atan2(po.x - pc.x);
                        let mut d = (back - ang).rem_euclid(std::f64::consts::TAU); // clockwise angle from the back direction
                        if d < 1e-6 {
                            d += std::f64::consts::TAU;
                        } // the edge we came along goes last in the queue
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
        // a point inside either segment, on the other or at its end: an end of one segment lying on the other is where
        // that one is cut, whichever of the two comes first - a line drawn before a rectangle, from side to side of it,
        // or a line through a point of the segments of an ellipse drawn before it, cut nothing
        (Line { a, b }, Line { a: c, b: d }) => {
            match seg_seg_t([a.0, a.1], [b.0, b.1], [c.0, c.1], [d.0, d.1]).map(|t| on_seg(a, b, t)).or_else(|| seg_seg_t([c.0, c.1], [d.0, d.1], [a.0, a.1], [b.0, b.1]).map(|u| on_seg(c, d, u))) {
                Some(q) => vec![q],
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
            }
        }
        // The tolerance lives in one place, inside `seg_circle_t`. A second filter at ±1e-9 used to stand here
        // and discarded a tangency that had already been found: the parameter of the tangency point came out as
        // −3.3e-6, which is 5e-5 mm before the start of the segment. Two thresholds answering one question is
        // the same ailment as one piece of knowledge kept in two places: relax one and the other keeps
        // cutting.
        (Line { a, b }, Circle { c, r }) | (Circle { c, r }, Line { a, b }) => seg_circle_t(a.0, a.1, b.0, b.1, c.0, c.1, r).into_iter().map(|t| on_seg(a, b, t)).collect(),
        (Line { a, b }, Arc { c, r, a0, a1, ccw, .. }) | (Arc { c, r, a0, a1, ccw, .. }, Line { a, b }) => {
            seg_circle_t(a.0, a.1, b.0, b.1, c.0, c.1, r).into_iter().map(|t| on_seg(a, b, t)).filter(|&p| arc_ok(c, a0, a1, ccw, p)).collect()
        }
        (Circle { c: c1, r: r1 }, Circle { c: c2, r: r2 }) => circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2),
        (Circle { c: c1, r: r1 }, Arc { c: c2, r: r2, a0, a1, ccw, .. }) | (Arc { c: c2, r: r2, a0, a1, ccw, .. }, Circle { c: c1, r: r1 }) => {
            circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2).into_iter().filter(|&p| arc_ok(c2, a0, a1, ccw, p)).collect()
        }
        (Arc { c: c1, r: r1, a0: s1, a1: e1, ccw: w1, .. }, Arc { c: c2, r: r2, a0: s2, a1: e2, ccw: w2, .. }) => {
            circle_circle_pts(c1.0, c1.1, r1, c2.0, c2.1, r2).into_iter().filter(|&p| arc_ok(c1, s1, e1, w1, p) && arc_ok(c2, s2, e2, w2, p)).collect()
        }
    }
}

/// POINTS WELDED INTO NODES: a point is one with the first node, by the order the nodes came, that `near` says it is
/// near, as a look through all the nodes in that order finds it. The nodes are looked up in the point's own and the
/// neighbouring cells of a grid `reach` wide, so `near` must say no past `reach`: 10 000 lines are 40 000 points,
/// and a look through all nodes for each of them was 8e8 comparisons.
pub(super) struct Welded {
    reach: f64,
    at: Vec<(f64, f64)>,
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

/// Points of an arrangement closer than this are one node, mm.
const WELD: f64 = 1e-4;

impl Welded {
    pub(super) fn new(reach: f64) -> Self {
        Welded { reach, at: Vec::new(), cells: std::collections::HashMap::new() }
    }

    fn cell(&self, p: (f64, f64)) -> (i64, i64) {
        ((p.0 / self.reach).floor() as i64, (p.1 / self.reach).floor() as i64)
    }

    pub(super) fn weld(&mut self, p: (f64, f64), near: impl Fn((f64, f64), (f64, f64)) -> bool) -> usize {
        let (cx, cy) = self.cell(p);
        let found = (cx - 1..=cx + 1).flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y))).filter_map(|c| self.cells.get(&c)).flatten().copied().filter(|&k| near(self.at[k], p)).min();
        if let Some(k) = found {
            return k;
        }
        self.at.push(p);
        self.cells.entry((cx, cy)).or_default().push(self.at.len() - 1);
        self.at.len() - 1
    }
}

/// Two points of an arrangement are one node closer than `WELD`.
fn within_weld(q: (f64, f64), p: (f64, f64)) -> bool {
    (q.0 - p.0).hypot(q.1 - p.1) < WELD
}

/// The box of a curve, widened past what an intersection may stand off it (a tangency within 1e-3 mm counts, see
/// `seg_circle_t`) and by a millionth of the curve, as a collinear overlap is judged.
fn curve_box(c: ArrCurve) -> [f64; 4] {
    let [x0, y0, x1, y1] = match c {
        ArrCurve::Line { a, b } => [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)],
        ArrCurve::Circle { c, r } | ArrCurve::Arc { c, r, .. } => [c.0 - r, c.1 - r, c.0 + r, c.1 + r],
    };
    let m = 1e-2 + 1e-6 * (x1 - x0).max(y1 - y0);
    [x0 - m, y0 - m, x1 + m, y1 + m]
}

/// Two boxes by their places, `first < second`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BoxPair {
    pub first: usize,
    pub second: usize,
}

/// THE PAIRS OF CURVES WHOSE BOXES MEET, see `meeting_boxes`.
fn meeting_pairs(curves: &[ArrCurve]) -> Vec<BoxPair> {
    meeting_boxes(&curves.iter().map(|&c| curve_box(c)).collect::<Vec<_>>())
}

/// THE PAIRS OF BOXES THAT MEET, each box `[x0, y0, x1, y1]`, in the order of `first` then `second` - the order a pass
/// of every box with every other takes them. Swept along x: a box meets only those whose x-span it enters before
/// they end, so boxes laid apart cost the sort, not every pair.
pub(super) fn meeting_boxes(boxes: &[[f64; 4]]) -> Vec<BoxPair> {
    let mut by_x: Vec<usize> = (0..boxes.len()).collect();
    by_x.sort_by(|&a, &b| boxes[a][0].total_cmp(&boxes[b][0]));
    let mut open: Vec<usize> = Vec::new();
    let mut pairs = Vec::new();
    for &i in &by_x {
        open.retain(|&k| boxes[k][2] >= boxes[i][0]);
        for &k in &open {
            if boxes[k][1] <= boxes[i][3] && boxes[i][1] <= boxes[k][3] {
                pairs.push(BoxPair { first: k.min(i), second: k.max(i) });
            }
        }
        open.push(i);
    }
    pairs.sort_unstable();
    pairs
}

/// THE CONTOUR OF ONE CURVE WITH ITS EXACT EDGES, from entity `e` with its points where `at` puts them: an ellipse one
/// edge, an arc of one one edge, the kernel building the ellipse it is rather than the segments of its points.
fn with_exact_edges(mut c: Contour, at: &dyn Fn(Id) -> Option<Point2>, e: &SketchEntity) -> Contour {
    if let Some(curve) = Curve::of_placed(at, e) {
        c.edges = curve.to_edges();
        c.edge_src = vec![e.id; c.edges.len()];
    }
    c
}

/// THE PIECES OF A SPLINE OR AN ARC OF AN ELLIPSE IN A LOOP AS THE CURVE THEY ARE. The arrangement cuts such a curve as
/// the segments of its points, and the kernel built a face of a body for every segment; a run of them along one curve
/// now goes as the piece of that curve between the ends of the run - an arc of an ellipse one edge, a spline a cubic a
/// span. A piece starts and ends where the run does, on the edges beside it: a crossing with a line lies on a segment of
/// the points, up to 0.005 mm off the curve. An ellipse keeps its ends, held by its centre and axes, and the edges beside
/// it are brought to them.
/// A PLACE A SEGMENT OF THE ARRANGEMENT IS CUT AT: its share along the segment, and the point.
#[derive(Clone, Copy)]
struct Cut {
    t: f64,
    at: (f64, f64),
}

/// A SEGMENT OF THE ARRANGEMENT AS A PIECE OF A CURVE: the curve it lies along, and the piece of its layout it is.
struct OnCurve {
    curve: Curve,
    piece: crate::model::curve::LaidPiece,
}

/// A SEGMENT OF THE LAYOUT OF A SPLINE OR AN ELLIPSE: the curve, and the piece of its layout it is.
#[derive(Clone, Copy)]
struct SegmentOf {
    curve: Id,
    piece: crate::model::curve::LaidPiece,
}

/// A RUN OF EDGES OF A REGION ALONG ONE CURVE: the entity, and the edges as the arrangement gave them.
struct RunOf {
    id: Id,
    edges: Vec<ProfEdge>,
}

/// A POINT A CURVE IS LAID OUT AT, by the curve and the bits of the point.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Spot {
    curve: Id,
    x: u64,
    y: u64,
}

impl Spot {
    fn of(curve: Id, p: Point2) -> Spot {
        Spot { curve, x: p.x.to_bits(), y: p.y.to_bits() }
    }
}

fn exact_runs(c: &mut Contour, exact: &std::collections::HashMap<Id, Curve>, spots: &std::collections::HashMap<Spot, f64>) {
    let n = c.edges.len();
    if n == 0 || c.edge_src.len() != n || !c.edge_src.iter().any(|id| exact.contains_key(id)) {
        return;
    }
    // WHETHER EDGE k GOES ON FROM THE EDGE BEFORE IT along one curve: of the same curve, and at the same place of it - the
    // point they share found from the middle of each. Where a curve crosses itself the two stand on two branches of it,
    // and a lobe of a figure of eight, all of one spline, is a run from the crossing round to it.
    // the place of the middle of every edge along its curve, found once: asked for each edge by itself and by the one
    // after it, the search ran four times an edge
    // EACH FROM THE PLACE OF THE EDGE BEFORE IT, where that is of the same curve: the edges of a run follow one another
    // along it, and the middle of a segment stands within 0.005 mm of the curve. Sought along the whole curve for each,
    // a loop of a spline of 276 nodes, 5 437 segments, took 1.2 s to rebuild, every frame of a drag.
    // A POINT THE CURVE WAS LAID OUT AT IS READ OFF ITS PLACE (`Spot`); only a point it was cut at is sought.
    let place = |id: Id, p: Point2| spots.get(&Spot::of(id, p)).copied();
    let mut middles: Vec<Option<f64>> = Vec::with_capacity(n);
    for k in 0..n {
        let found = c.edges[k].ends().zip(exact.get(&c.edge_src[k])).map(|([a, b], curve)| {
            if let (Some(ta), Some(tb)) = (place(c.edge_src[k], a), place(c.edge_src[k], b)) {
                // a segment over the seam of a loop runs from near 1 to 0
                return if curve.closed() && (ta - tb).abs() > 0.5 { ((ta + tb + 1.0) * 0.5).rem_euclid(1.0) } else { (ta + tb) * 0.5 };
            }
            let m = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let before = k.checked_sub(1).filter(|p| c.edge_src[*p] == c.edge_src[k]).and_then(|p| middles[p]);
            before.map(|t| curve.polish(m, t)).filter(|t| curve.at(*t).dist(m) < 0.02).unwrap_or_else(|| curve.nearest(m).t)
        });
        middles.push(found);
    }
    let joined = |k: usize| -> bool {
        let p = (k + n - 1) % n;
        if c.edge_src[p] != c.edge_src[k] {
            return false;
        }
        let (Some(curve), Some([pa, pb]), Some([qa, qb]), Some(mp), Some(mk)) = (exact.get(&c.edge_src[k]), c.edges[p].ends(), c.edges[k].ends(), middles[p], middles[k]) else { return true };
        // the walk turning back on the segment it came along, at the tip of a curve hanging into a region: the run out and
        // the run back are two pieces, taken out of the region as a pair (`without_spikes`); joined, they were one piece
        // from a point to itself
        if pa.dist(qb) < 1e-12 && pb.dist(qa) < 1e-12 {
            return false;
        }
        let id = c.edge_src[k];
        let gap = match (place(id, pb), place(id, qa)) {
            (Some(x), Some(y)) => (x - y).abs(),
            _ => (curve.polish(pb, mp) - curve.polish(qa, mk)).abs(),
        };
        gap < 1e-6 || (curve.closed() && (gap - 1.0).abs() < 1e-6)
    };
    // the loop read from the start of a run, or from its first edge when it is one curve all round
    let first = (0..n).find(|&k| !joined(k)).unwrap_or(0);
    let at = |k: usize| (first + k) % n;
    let mut runs: Vec<RunOf> = Vec::new();
    let mut k = 0;
    while k < n {
        let id = c.edge_src[at(k)];
        let len = 1 + (k + 1..n).take_while(|&j| joined(at(j))).count();
        runs.push(RunOf { id, edges: (k..k + len).map(|j| c.edges[at(j)].clone()).collect() });
        k += len;
    }
    // WHERE TWO RUNS OF TWO CURVES MEET IS WHERE THE CURVES MEET: the corner the segments crossed at, settled onto both
    // curves at once by Newton's steps (`Curve::settle`). Laid from the crossing of their segments, each run ended at the
    // foot of that crossing on its own curve - up to the sag off the true crossing, farther along a shallow one - and the
    // two runs passed each other there: the wall of a region of a drawing of 386 splines was refused by the kernel as a
    // wire crossing itself.
    let curve_of = |r: &RunOf| exact.get(&r.id).cloned().or_else(|| (r.edges.len() == 1).then(|| Curve::of_edge(&r.edges[0])));
    let found = |curve: &Curve, id: Id, p: Point2| place(id, p).unwrap_or_else(|| curve.nearest(p).t);
    let m = runs.len();
    let joints: Vec<Option<Point2>> = (0..m)
        .map(|r| {
            let (a, b) = (&runs[r], &runs[(r + 1) % m]);
            let raw = a.edges.last()?.ends()?[1];
            if m < 2 || a.id == b.id {
                return Some(raw);
            }
            let (Some(ca), Some(cb)) = (curve_of(a), curve_of(b)) else { return Some(raw) };
            let met = ca.settle(&cb, Point2::new(found(&ca, a.id, raw), found(&cb, b.id, raw))).filter(|met| met.at.dist(raw) < 0.1);
            Some(met.map_or(raw, |met| met.at))
        })
        .collect();
    let mut edges: Vec<ProfEdge> = Vec::with_capacity(n);
    let mut src: Vec<Id> = Vec::with_capacity(n);
    for (r, run) in runs.iter().enumerate() {
        let (Some(first), Some(last)) = (run.edges.first().and_then(ProfEdge::ends), run.edges.last().and_then(ProfEdge::ends)) else { continue };
        let ends = [joints[(r + m - 1) % m].unwrap_or(first[0]), joints[r].unwrap_or(last[1])];
        let piece = exact.get(&run.id).and_then(|curve| run_as_curve(curve, &run.edges, &|p: Point2| place(run.id, p), ends));
        let mut laid = piece.unwrap_or_else(|| run.edges.clone());
        if let Some(e) = laid.first_mut() {
            *e = e.clone().with_start(ends[0]);
        }
        if let Some(e) = laid.last_mut() {
            *e = e.clone().with_end(ends[1]);
        }
        src.extend(std::iter::repeat_n(run.id, laid.len()));
        edges.extend(laid);
    }
    // the edges beside an ellipse brought to its ends
    let m = edges.len();
    for k in 0..m {
        if let (ProfEdge::Oval { .. }, Some([s, e])) = (&edges[k], edges[k].ends()) {
            let (before, after) = ((k + m - 1) % m, (k + 1) % m);
            edges[before] = edges[before].clone().with_end(s);
            edges[after] = edges[after].clone().with_start(e);
        }
    }
    c.edges = edges;
    c.edge_src = src;
}

/// THE RUN OF SEGMENTS ALONG `curve` AS THE PIECE OF IT between the ends of the run, in the way the run goes. `None`
/// when the run holds an edge that is no segment.
///
/// The place of each end is found from the place of the point next to it in the run: where a curve crosses itself, its
/// two ends stand at one point on two branches of it - a lobe of a figure of eight was taken for the whole curve, and its
/// area came out 0.008 mm^2 against the other lobe's 0.
fn run_as_curve(curve: &Curve, run: &[ProfEdge], laid_at: &dyn Fn(Point2) -> Option<f64>, [start, end]: [Point2; 2]) -> Option<Vec<ProfEdge>> {
    let [_, second] = run.first()?.ends()?;
    let [last_but_one, _] = run.last()?.ends()?;
    if !run.iter().all(|e| matches!(e, ProfEdge::Line { .. })) {
        return None;
    }
    // a point the curve was laid out at by its place, any other sought
    let place = |p: Point2| laid_at(p).unwrap_or_else(|| curve.nearest(p).t);
    let (t0, t1) = (curve.polish(start, place(second)), curve.polish(end, place(last_but_one)));
    // the way the run goes along the curve, told by its first segment; a closed curve run round from where it started is
    // the whole of it
    let v = curve.velocity(t0);
    let forward = (second.x - start.x) * v.x + (second.y - start.y) * v.y >= 0.0;
    let whole = curve.closed() && start.dist(end) < 1e-9 && ((t1 - t0).rem_euclid(1.0) < 1e-6 || (t0 - t1).rem_euclid(1.0) < 1e-6);
    let span = |from: f64, to: f64| if curve.closed() { from + if whole { 1.0 } else { (to - from).rem_euclid(1.0) } } else { to };
    let mut edges = if forward { curve.piece([t0, span(t0, t1)]).to_edges() } else { curve.piece([t1, span(t1, t0)]).to_edges() };
    if !forward {
        edges.reverse();
        edges = edges.into_iter().map(ProfEdge::reversed).collect();
    }
    let last = edges.len().checked_sub(1)?;
    edges[0] = edges[0].clone().with_start(start);
    edges[last] = edges[last].clone().with_end(end);
    Some(edges)
}

/// The planar arrangement of a sketch: every entity — lines, arcs, circles and splines — is intersected with every
/// other, the curves are cut at the intersection points, and the minimal faces, that is the closed regions, are
/// assembled. This is what turns the strip between two circles, cut by a line, into a selectable closed region.
/// Returns the faces as contours, carrying exact `ProfEdge` edges for the B-rep: a spline, an ellipse and an arc of one
/// are cut as the segments of their points and laid back as the curves they are.
pub(super) fn arrangement_regions(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<Contour> {
    arrangement_regions_prov(points, entities).into_iter().map(|(c, _)| c).collect()
}

/// As `arrangement_regions`, but each region carries its provenance: the sorted set of entity ids its boundary
/// is stitched from. This is what keeps the identity of a contour stable across edits, so that loops which
/// swapped places do not take over each other's ids.
pub(super) fn arrangement_regions_prov(points: &[SketchPoint], entities: &[SketchEntity]) -> Vec<(Contour, Vec<Id>)> {
    let Faces { inner, laid, .. } = arrangement_faces(points, entities);
    inner
        .into_iter()
        .map(|(mut c, mut prov)| {
            laid.finish(&mut c, &mut prov);
            (c, prov)
        })
        .collect()
}

/// A REGION WITHOUT THE LINES HANGING INTO IT: the walk round a face goes out along a line hanging into it and back, an
/// edge and its twin side by side, and the region came with a spike of no width the kernel would not make a face of
/// (measured on the case example: a chain of six lines hanging into the region of a base after an edit, the base refused
/// to extrude). Such pairs are taken out, the inner first, until none is left; a bridge to an island stays, its two
/// halves standing apart in the walk. Taken out of the region as it goes to the sketch, not out of the walks of the
/// arrangement: the sketch, solved round a change, reads those to tell whether a region runs past its window, and with
/// the hanging pieces gone it lost the region a last line closed (a hexagon side by side). The provenance is the
/// entities left on the boundary. A spline and an arc of an ellipse hang in as their pieces do, out and back.
pub(super) fn without_spikes(c: &mut Contour, prov: &mut Vec<Id>) {
    use crate::geom::ProfEdge;
    let n = c.edges.len();
    let same = |p: Point2, q: Point2| (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9;
    let back = |p: &ProfEdge, q: &ProfEdge| match (p, q) {
        (ProfEdge::Line { a, b }, ProfEdge::Line { a: qa, b: qb }) => same(*a, *qb) && same(*b, *qa),
        (ProfEdge::Arc { a, b, center, ccw }, ProfEdge::Arc { a: qa, b: qb, center: qc, ccw: qw }) => same(*a, *qb) && same(*b, *qa) && same(*center, *qc) && ccw != qw,
        // a piece of an ellipse or of a spline and the same piece run back: the ends swapped, the middle one
        (ProfEdge::Oval { .. }, ProfEdge::Oval { .. }) | (ProfEdge::Spline(_), ProfEdge::Spline(_)) => match (p.ends(), q.ends(), p.curve_at(0.5), q.curve_at(0.5)) {
            (Some([a, b]), Some([qa, qb]), Some(m), Some(qm)) => same(a, qb) && same(b, qa) && same(m, qm),
            _ => false,
        },
        _ => false,
    };
    let mut kept: Vec<usize> = Vec::with_capacity(n);
    for k in 0..n {
        if kept.last().is_some_and(|&l| back(&c.edges[l], &c.edges[k])) {
            kept.pop();
        } else {
            kept.push(k);
        }
    }
    while kept.len() >= 2 && back(&c.edges[kept[kept.len() - 1]], &c.edges[kept[0]]) {
        kept.pop();
        kept.remove(0);
    }
    if kept.len() == n || kept.len() < 2 {
        return;
    }
    c.edges = kept.iter().map(|&k| c.edges[k].clone()).collect();
    if c.edge_src.len() == n {
        c.edge_src = kept.iter().map(|&k| c.edge_src[k]).collect();
    }
    c.points = c.points_from_edges();
    let kept_src: std::collections::BTreeSet<Id> = c.edge_src.iter().copied().collect();
    *prov = kept_src.into_iter().collect();
}

/// THE FACES OF AN ARRANGEMENT AND THE WALKS AROUND ITS PIECES: the regions of `arrangement_regions_prov`, and the walks
/// left out of them - the outside of each piece of the drawing, clockwise, and the walk along a piece that bounds no
/// area, out along it and back. The loops round a change (`round`) look at those: a moved curve on the outside of a
/// piece seen in its window may close a region past it.
pub(super) struct Faces {
    pub inner: Vec<(Contour, Vec<Id>)>,
    pub outer: Vec<Contour>,
    pub laid: LaidCurves,
}

/// THE CURVES THE SEGMENTS OF A SPLINE OR AN ELLIPSE STAND FOR in an arrangement, and the place along its curve of every
/// point such a curve was laid out at (`Spot`): what a region is laid back as curves by when it goes to the sketch.
pub(super) struct LaidCurves {
    exact: std::collections::HashMap<Id, Curve>,
    spots: std::collections::HashMap<Spot, f64>,
}

impl LaidCurves {
    /// A REGION OF THE ARRANGEMENT AS IT GOES TO THE SKETCH: the walks out along a piece and back taken out of it
    /// (`without_spikes`) while it is still segments - out and back the same segments - and then its runs along one
    /// curve laid as that curve (`exact_runs`). Laid first, the way out ran on past the turn and the way back was a piece
    /// of it, and the wall of the region crossed itself; taken out of the walk itself, the sketch solved round a change
    /// lost the walk it reads a region running past its window by. The walks of the arrangement are left as they are.
    pub(super) fn finish(&self, c: &mut Contour, prov: &mut Vec<Id>) {
        without_spikes(c, prov);
        exact_runs(c, &self.exact, &self.spots);
    }
}

pub(super) fn arrangement_faces(points: &[SketchPoint], entities: &[SketchEntity]) -> Faces {
    use std::f64::consts::{PI, TAU};
    let at: std::collections::HashMap<Id, (f64, f64)> = points.iter().map(|p| (p.id, (p.x, p.y))).collect();
    let pt = |id: Id| at.get(&id).copied();
    let mut curves: Vec<ArrCurve> = Vec::new();
    let mut curve_eid: Vec<Id> = Vec::new(); // provenance of a curve, its entity id, parallel to `curves`
                                             // the curves that enter the arrangement as the segments of their points, as the curves they are, to give the kernel
    let exact: std::collections::HashMap<Id, Curve> = entities
        .iter()
        .filter(|e| !e.construction && (e.kind.as_spline().is_some() || e.kind.as_ellipse().is_some()))
        .filter_map(|e| Curve::of_placed(&|id: Id| pt(id).map(|(x, y)| Point2::new(x, y)), e).map(|c| (e.id, c)))
        .collect();
    // THE PLACE ALONG ITS CURVE OF EVERY POINT A SPLINE OR AN ELLIPSE IS LAID OUT AT, by the bits of the point: what a run
    // of segments goes back to its curve by (`exact_runs`). Sought again by Newton's steps for every segment, a sketch of
    // two splines of 82 and 688 nodes took 0.27 s to rebuild in a release build, every step of a drag.
    let mut spots: std::collections::HashMap<Spot, f64> = std::collections::HashMap::new();
    // the curve and the places of it each segment of a spline or an ellipse stands between, parallel to `curves`
    let mut laid_on: Vec<Option<SegmentOf>> = Vec::new();
    for e in entities {
        if e.construction {
            continue;
        }
        match exact.get(&e.id) {
            Some(curve) => {
                let places = curve.places();
                let laid: Vec<Point2> = places.iter().map(|t| curve.at(*t)).collect();
                for (t, q) in places.iter().zip(&laid) {
                    spots.entry(Spot::of(e.id, *q)).or_insert(*t);
                }
                let xy = |p: Point2| (p.x, p.y);
                for k in 0..laid.len().saturating_sub(1) {
                    if laid[k].dist(laid[k + 1]) > 1e-9 {
                        curves.push(ArrCurve::Line { a: xy(laid[k]), b: xy(laid[k + 1]) });
                        curve_eid.push(e.id);
                        laid_on.push(Some(SegmentOf { curve: e.id, piece: crate::model::curve::LaidPiece { ends: [laid[k], laid[k + 1]], run: [places[k], places[k + 1]] } }));
                    }
                }
            }
            None => {
                let placed = |id: Id| pt(id).map(|(x, y)| Point2::new(x, y));
                let Some(curve) = Curve::of_placed(&placed, e) else { continue };
                let pieces = curve.arranged();
                curve_eid.extend(std::iter::repeat_n(e.id, pieces.len()));
                laid_on.extend(std::iter::repeat_n(None, pieces.len()));
                curves.extend(pieces);
            }
        }
    }
    let nc = curves.len();
    let mut out: Vec<(Contour, Vec<Id>)> = Vec::new();
    let mut outer: Vec<Contour> = Vec::new();
    if nc == 0 {
        return Faces { inner: out, outer, laid: LaidCurves { exact, spots } };
    }
    // Only the curves whose boxes meet are intersected (`meeting_pairs`), in the order of the pairs of every curve with
    // every other, so the cuts come in the order they always did. All pairs of 10 000 lines are 5e7 intersections.
    let mut cuts: Vec<Vec<(f64, f64)>> = vec![Vec::new(); nc];
    // A SEGMENT OF A SPLINE OR AN ELLIPSE MEETS ANOTHER WHERE THE CURVES MEET (`Curve::meets_near`): a segment of a line
    // stands for the line itself. Two curves nearly touching cross twice within a few hundredths of a millimetre, and
    // the second crossing fell between the segments of their layouts: the region past the first ran over the second,
    // and the kernel refused its wall as a wire crossing itself.
    let as_curve = |k: usize| -> Option<OnCurve> {
        match (&laid_on[k], curves[k]) {
            (Some(s), _) => exact.get(&s.curve).map(|c| OnCurve { curve: c.clone(), piece: s.piece }),
            (None, ArrCurve::Line { a, b }) => {
                let (a, b) = (Point2::new(a.0, a.1), Point2::new(b.0, b.1));
                Some(OnCurve { curve: Curve::of_segment([a, b]), piece: crate::model::curve::LaidPiece { ends: [a, b], run: [0.0, 1.0] } })
            }
            _ => None,
        }
    };
    for BoxPair { first, second } in meeting_pairs(&curves) {
        let by_curves = laid_on[first].is_some() || laid_on[second].is_some();
        let met = match (by_curves, as_curve(first), as_curve(second)) {
            (true, Some(OnCurve { curve: ca, piece: pa }), Some(OnCurve { curve: cb, piece: pb })) => {
                // two segments of one curve next to each other share a point and meet nowhere else
                let neighbours =
                    laid_on[first].as_ref().zip(laid_on[second].as_ref()).is_some_and(|(x, y)| x.curve == y.curve && (x.piece.run[1] == y.piece.run[0] || y.piece.run[1] == x.piece.run[0]));
                if neighbours {
                    Vec::new()
                } else {
                    ca.meets_near(&cb, [pa, pb]).into_iter().map(|m| (m.at.x, m.at.y)).collect()
                }
            }
            _ => arr_intersect(curves[first], curves[second]),
        };
        for p in met {
            cuts[first].push(p);
            cuts[second].push(p);
        }
    }
    let mut nodes = Welded::new(WELD);
    let weld = |nodes: &mut Welded, p: (f64, f64)| nodes.weld(p, within_weld);
    // a directed edge of the arrangement: a line or an arc, where cx, cy, r and ccw describe the direction
    // from `from` to `to`
    struct DE {
        from: usize,
        to: usize,
        line: bool,
        cx: f64,
        cy: f64,
        r: f64,
        ccw: bool,
        ci: usize,
    }
    let mut edges: Vec<DE> = Vec::new();
    let mut seen_line: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new(); // B28
    for (ci, cu) in curves.iter().enumerate() {
        match *cu {
            ArrCurve::Line { a, b } => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len2 = dx * dx + dy * dy;
                // A CUT IS A NODE WHERE IT STANDS, ordered along the segment by its foot: the meeting of two curves stands
                // on the curves, up to the sag off either segment of their layouts, and laid at its foot on each it was two
                // nodes 0.005 mm apart that tied nothing together
                let mut ts: Vec<Cut> = vec![Cut { t: 0.0, at: a }, Cut { t: 1.0, at: b }];
                for &p in &cuts[ci] {
                    ts.push(Cut { t: ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2, at: p });
                }
                ts.retain(|c| c.t > -1e-9 && c.t < 1.0 + 1e-9);
                ts.sort_by(|x, y| x.t.total_cmp(&y.t));
                ts.dedup_by(|x, y| (x.t - y.t).abs() < 1e-6);
                for w in ts.windows(2) {
                    let (p0, p1) = (w[0].at, w[1].at);
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
                    if ccw {
                        (a1 - a0).rem_euclid(TAU)
                    } else {
                        (a0 - a1).rem_euclid(TAU)
                    }
                };
                let to_param = |p: (f64, f64)| {
                    let ang = (p.1 - c.1).atan2(p.0 - c.0);
                    if ccw {
                        (ang - a0).rem_euclid(TAU)
                    } else {
                        (a0 - ang).rem_euclid(TAU)
                    }
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
    let nodes = nodes.at;
    if edges.is_empty() {
        return Faces { inner: out, outer, laid: LaidCurves { exact, spots } };
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
            //
            // A TANGENCY IS A TIE WITHIN WHAT WELDING MOVES AN ANGLE BY: a node stands up to `WELD` off the true point,
            // which turns the tangent of an arc by up to WELD / r and a line by WELD / its length. Two edges whose
            // first-order angles differ by less than that leave as one tangent, and the curvature decides. Measured:
            // a sketch solved after an edit left the end of a fillet of R10 4.7e-6 mm off, the tangent of the arc
            // 4.7e-7 rad off the line touching it there - past the second-order term of 1e-8 - and the walk took
            // the line for the way back along the arc: the face of the rounded rectangle was lost, and its lines
            // were no longer drawn.
            const CURV_DELTA: f64 = 1e-7;
            let slack = |e: &DE| -> f64 {
                let span = if e.line { (nodes[e.to].0 - nodes[e.from].0).hypot(nodes[e.to].1 - nodes[e.from].1) } else { e.r };
                WELD / span.max(WELD)
            };
            let raw = |oe: usize| -> f64 { (back - tang(&edges[oe], true)).rem_euclid(TAU) };
            let st_raw = raw(twin);
            let mut taken: Vec<(f64, f64)> = vec![(st_raw, slack(&edges[twin]))]; // (first-order angle, its slack)
            let mut s_eff = |oe: usize| -> f64 {
                let mut s = raw(oe);
                let own = slack(&edges[oe]);
                if let Some(&(t, _)) = taken.iter().find(|(t, sl)| {
                    let d = (s - t).rem_euclid(TAU);
                    d.min(TAU - d) <= own + sl
                }) {
                    s = t;
                } else {
                    taken.push((s, own));
                }
                (s - curv(&edges[oe]) * CURV_DELTA).rem_euclid(TAU)
            };
            let st = s_eff(twin);
            let mut best: Option<(f64, usize)> = None; // (clockwise cyclic distance from the twin, edge)
            for &oe in &out_adj[v] {
                if oe == twin {
                    continue; // the reversal is taken only when there is nowhere else to go
                }
                let d = (s_eff(oe) - st).rem_euclid(TAU);
                if best.is_none_or(|(bd, _)| d < bd) {
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
            let path: Vec<String> = face
                .iter()
                .map(|&ei| {
                    let e = &edges[ei];
                    format!(
                        "{}->{}{}",
                        e.from,
                        e.to,
                        if e.line {
                            "L"
                        } else if e.ccw {
                            "+"
                        } else {
                            "-"
                        }
                    )
                })
                .collect();
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
            let (pa, pb) = (crate::geom::Point2::new(a.0, a.1), crate::geom::Point2::new(b.0, b.1));
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
                cedges.push(crate::geom::ProfEdge::Arc { a: pa, b: pb, center: crate::geom::Point2::new(e.cx, e.cy), ccw: e.ccw });
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
            // the walk as it is, its segments and its spikes: a region is laid as curves as it goes to the sketch
            // (`LaidCurves::finish`)
            // provenance of a face: the set of entities whose pieces form its boundary, sorted and without
            // repeats
            let prov: std::collections::BTreeSet<Id> = face.iter().map(|&ei| curve_eid[edges[ei].ci]).collect();
            out.push((cont, prov.into_iter().collect())); // an interior face, counter-clockwise; the outer one and degenerate results were dropped
        } else {
            outer.push(cont);
        }
    }
    Faces { inner: out, outer, laid: LaidCurves { exact, spots } }
}

#[cfg(test)]
mod tests {
    /// A LINE TOUCHING A FILLET AT ITS END, the ends of the fillet off by what a solve leaves: three rectangles drawn side by
    /// side, the top right corner of the left one rounded 10, the sketch solved after an edit - the ends of the arc
    /// 4.7e-6 and 5.7e-6 mm off its circle - and a line drawn along the top from the end of the arc to the corner. The
    /// rounded rectangle stays a face, and the chip between the arc, the line and the side is one too. Reported behaviour:
    /// the lines of the left rectangle vanished from the sheet and its points stayed.
    #[test]
    fn a_line_touching_a_fillet_off_by_a_solve_keeps_the_face_of_the_rectangle() {
        use crate::model::{SketchEntity, SketchPoint};
        let pts: Vec<SketchPoint> = [
            (5, 0.0, 0.0),
            (6, -125.0, 24.99999999357503),
            (7, -75.0, 24.99999999357503),
            (8, -75.0, 125.0),
            (9, -125.0, 125.0),
            (14, -100.0, 74.99999999678752),
            (16, 0.0, 0.0),
            (18, -39.999999999999275, 24.99999999357503),
            (19, -40.0, 125.0),
            (24, -57.5, 74.99999999678752),
            (27, -40.0, 160.0),
            (28, -75.0, 160.0),
            (33, -57.5, 142.5),
            (36, -75.0, 135.0),
            (37, -65.0, 125.0),
            (38, -65.0, 135.0),
            (40, -75.0, 150.0),
            (41, -65.0, 160.0),
            (42, -65.0, 150.0),
            (44, -50.0, 125.0),
            (45, -40.0, 135.0),
            (46, -50.0, 135.0),
            (48, -50.0, 160.0),
            (49, -40.0, 150.0),
            (50, -50.0, 150.0),
            (52, -85.00000474124738, 125.0),
            (53, -75.0, 115.0000056834509),
            (54, -85.0, 115.0),
            (56, -125.0, 114.99999986304107),
            (57, -114.99999989346223, 125.0),
            (58, -115.0, 115.0),
        ]
        .iter()
        .map(|&(id, x, y)| SketchPoint { id, x, y })
        .collect();
        let l = |id, a, b| SketchEntity { id, kind: crate::model::LineParts { a, b }.into(), construction: false };
        let arc = |id, center, a, b, ccw| SketchEntity { id, kind: crate::model::ArcParts { center, a, b, ccw }.into(), construction: false };
        let all: Vec<SketchEntity> = vec![
            l(10, 6, 7),
            l(11, 7, 53),
            l(12, 52, 57),
            l(13, 56, 6),
            l(20, 7, 18),
            l(21, 18, 19),
            l(22, 44, 37),
            l(23, 8, 7),
            l(29, 8, 19),
            l(30, 45, 49),
            l(31, 48, 41),
            l(32, 40, 36),
            arc(39, 38, 36, 37, true),
            arc(43, 42, 40, 41, false),
            arc(47, 46, 44, 45, true),
            arc(51, 50, 48, 49, false),
            arc(55, 54, 52, 53, false),
            arc(59, 58, 56, 57, false),
            l(138, 52, 8),
        ];
        let before: Vec<SketchEntity> = all.iter().filter(|e| e.id != 138).cloned().collect();
        let faces = |ents: &[SketchEntity]| -> Vec<Vec<u64>> {
            super::arrangement_regions_prov(&pts, ents)
                .into_iter()
                .map(|r| {
                    let mut ids = r.1;
                    ids.sort_unstable();
                    ids.dedup();
                    ids
                })
                .collect()
        };
        let rounded = vec![10, 11, 12, 13, 55, 59];
        assert!(faces(&before).contains(&rounded), "GUARD: the rounded rectangle is a face before the line: {:?}", faces(&before));
        let after = faces(&all);
        assert!(after.contains(&rounded) && after.contains(&vec![23, 55, 138]), "the line along the top, touching the fillet at its end, lost the faces: {after:?}");
    }

    /// A LINE THAT ENDS INSIDE A REGION IS NO PART OF ITS EDGE. The walk round a face went out along a line hanging into
    /// it and back, and the region came with a spike of no width that the kernel would not make a face of. Measured on
    /// the case example: an angle of the outline of a base edited to 231 left a chain of six lines hanging into the
    /// region of the base, walked out and back, and the base refused to extrude. A square 10 x 10 and a line from the
    /// middle of its bottom 4 up into it: the square is one region of area 100, the hanging line no edge of it.
    #[test]
    fn a_line_hanging_into_a_region_is_no_edge_of_it() {
        use crate::model::{SketchEntity, SketchPoint};
        let pts = vec![
            SketchPoint { id: 1, x: 0.0, y: 0.0 },
            SketchPoint { id: 2, x: 10.0, y: 0.0 },
            SketchPoint { id: 3, x: 10.0, y: 10.0 },
            SketchPoint { id: 4, x: 0.0, y: 10.0 },
            SketchPoint { id: 5, x: 5.0, y: 0.0 },
            SketchPoint { id: 6, x: 5.0, y: 4.0 },
        ];
        let l = |id, a, b| SketchEntity { id, kind: crate::model::LineParts { a, b }.into(), construction: false };
        let ents = vec![l(10, 1, 2), l(11, 2, 3), l(12, 3, 4), l(13, 4, 1), l(14, 5, 6)];
        let regions = super::arrangement_regions_prov(&pts, &ents);
        assert_eq!(regions.len(), 1, "the square is not one region: {}", regions.len());
        let (contour, ids) = &regions[0];
        assert!(!ids.contains(&14), "the line hanging into the square is an edge of its region: {ids:?}");
        assert!((contour.area().abs() - 100.0).abs() < 1e-9, "the region of the square holds {}", contour.area());
    }

    use super::{arr_intersect, meeting_pairs, within_weld, ArrCurve, BoxPair, Welded, WELD};

    /// A fixed pseudo-random sequence in 0..1.
    struct Seq(u64);

    impl Seq {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Curves crowded into a 100 mm square: lines, circles and arcs, lines laid over each other, and lines tangent to a
    /// circle within the 1e-3 mm a tangency is judged by.
    fn crowd(n: usize) -> Vec<ArrCurve> {
        let mut s = Seq(7);
        let mut out = Vec::new();
        for k in 0..n {
            let (x, y) = (s.next() * 100.0, s.next() * 100.0);
            let curve = match k % 5 {
                0 => ArrCurve::Line { a: (x, y), b: (x + s.next() * 30.0 - 15.0, y + s.next() * 30.0 - 15.0) },
                1 => ArrCurve::Circle { c: (x, y), r: 1.0 + s.next() * 10.0 },
                2 => {
                    let (r, a0) = (1.0 + s.next() * 10.0, s.next() * 6.0);
                    let a1 = a0 + 0.5 + s.next() * 4.0;
                    ArrCurve::Arc { c: (x, y), r, a0, a1, ccw: true, pa: (x + r * a0.cos(), y + r * a0.sin()), pb: (x + r * a1.cos(), y + r * a1.sin()) }
                }
                // a line over the line before it, shifted along itself
                3 => match out.last() {
                    Some(&ArrCurve::Line { a, b }) => ArrCurve::Line { a: ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), b: (b.0 + (b.0 - a.0), b.1 + (b.1 - a.1)) },
                    _ => ArrCurve::Line { a: (x, y), b: (x + 5.0, y) },
                },
                // a line tangent to a circle of radius 4 at (x, y), standing off it by up to 9e-4 mm
                _ => {
                    out.push(ArrCurve::Circle { c: (x, y), r: 4.0 });
                    let off = 4.0 + (s.next() - 0.5) * 1.8e-3;
                    ArrCurve::Line { a: (x - 6.0, y + off), b: (x + 6.0, y + off) }
                }
            };
            out.push(curve);
        }
        out
    }

    #[test]
    fn every_pair_of_curves_that_meet_is_among_the_pairs_whose_boxes_meet() {
        let curves = crowd(600);
        let pairs = meeting_pairs(&curves);
        assert!(pairs.windows(2).all(|w| w[0] < w[1]), "the pairs come in the order of a pass of every curve with every other");
        let mut missed = Vec::new();
        let mut meeting = 0;
        for first in 0..curves.len() {
            for second in first + 1..curves.len() {
                if !arr_intersect(curves[first], curves[second]).is_empty() {
                    meeting += 1;
                    if pairs.binary_search(&BoxPair { first, second }).is_err() {
                        missed.push(format!("{first} x {second}"));
                    }
                }
            }
        }
        assert!(meeting > 500, "the crowd meets too little to say anything: {meeting} pairs");
        assert!(missed.is_empty(), "pairs that meet but were not looked at: {missed:?}");
    }

    #[test]
    fn a_point_is_welded_to_the_node_a_look_through_all_of_them_finds() {
        let mut s = Seq(11);
        let mut welded = Welded::new(WELD);
        let mut all: Vec<(f64, f64)> = Vec::new();
        for _ in 0..5_000 {
            // points strewn over a square 3e-3 mm wide, 30 welds across: many within the weld of one another, many not,
            // and as many across the edges of the cells as inside them
            let p = (s.next() * 3e-3, s.next() * 3e-3);
            let by_look = all.iter().position(|q| (q.0 - p.0).hypot(q.1 - p.1) < WELD).unwrap_or_else(|| {
                all.push(p);
                all.len() - 1
            });
            assert_eq!(welded.weld(p, within_weld), by_look, "the point {p:?}");
        }
        assert!(all.len() > 100 && all.len() < 4_000, "the lattice welds some points and not others: {} nodes", all.len());
    }
}
