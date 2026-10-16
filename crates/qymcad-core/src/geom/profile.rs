//! PROFILES: segments stitched, dogbones, arcs tessellated, loops encoded.

use super::*;

/// Stitch segments into polylines by their coincident endpoints.
pub fn stitch_segments(segs: Vec<(Point2, Point2)>, tol: f64) -> Vec<Contour> {
    let mut chains: Vec<Vec<Point2>> = segs.into_iter().map(|(a, b)| vec![a, b]).collect();
    let mut out: Vec<Contour> = Vec::new();
    while let Some(mut chain) = chains.pop() {
        // A chain is non-empty by construction, since each one is born from a segment of two points, but that
        // invariant must not be enforced by a panic: a CAD core has no business dying on data. If it is
        // violated, leave the chain rather than take the application down along with unsaved work.
        while let (Some(&head), Some(&tail)) = (chain.first(), chain.last()) {
            let mut matched = None;
            for (i, s) in chains.iter().enumerate() {
                let (Some(&a), Some(&b)) = (s.first(), s.last()) else { continue };
                if b.dist(head) <= tol {
                    matched = Some((i, true, false));
                    break;
                } else if a.dist(head) <= tol {
                    matched = Some((i, true, true));
                    break;
                } else if a.dist(tail) <= tol {
                    matched = Some((i, false, false));
                    break;
                } else if b.dist(tail) <= tol {
                    matched = Some((i, false, true));
                    break;
                }
            }
            let Some((i, at_head, rev)) = matched else { break };
            let mut s = chains.remove(i);
            if rev {
                s.reverse();
            }
            if at_head {
                s.pop();
                s.extend(chain);
                chain = s;
            } else {
                chain.extend(s.into_iter().skip(1));
            }
        }
        let mut pts: Vec<Point2> = Vec::with_capacity(chain.len());
        for p in chain {
            if pts.last().is_none_or(|l: &Point2| l.dist(p) > tol) {
                pts.push(p);
            }
        }
        if pts.len() < 2 {
            continue;
        }
        // `pts` is non-empty by the check above, but it is read safely: an invariant is no reason to panic.
        let ends = pts.first().copied().zip(pts.last().copied());
        let gap = ends.map(|(f, l)| f.dist(l)).unwrap_or(f64::INFINITY);
        let closed = pts.len() >= 3 && gap <= tol * 4.0;
        if closed {
            if gap <= tol {
                pts.pop();
            }
            out.push(Contour::closed(pts));
        } else {
            out.push(Contour::open(pts));
        }
    }
    out
}

/// Dogbone corner relief points: for every sharp corner of a closed contour, a pair of (vertex, tip of the overcut)
/// running outwards along the bisector. It points away from the centre of the contour, into the wall, so that the
/// resulting socket accepts a square corner.
pub fn dogbone_overcuts(contour: &Contour, length: f64) -> Vec<(Point2, Point2)> {
    let p = &contour.points;
    let n = p.len();
    if !contour.closed || n < 3 {
        return Vec::new();
    }
    let centroid = contour.centroid();
    let norm = |v: Point2| {
        let l = v.len().max(1e-9);
        Point2::new(v.x / l, v.y / l)
    };
    let mut out = Vec::new();
    for i in 0..n {
        let prev = p[(i + n - 1) % n];
        let v = p[i];
        let next = p[(i + 1) % n];
        let t1 = norm(v - prev);
        let t2 = norm(next - v);
        // Sharp corners only, meaning a turn of more than about 60 degrees.
        if t1.x * t2.x + t1.y * t2.y > 0.5 {
            continue;
        }
        let mut bis = norm(t2 - t1);
        let outward = v - centroid;
        if bis.x * outward.x + bis.y * outward.y < 0.0 {
            bis = Point2::new(-bis.x, -bis.y);
        }
        out.push((v, Point2::new(v.x + bis.x * length, v.y + bis.y * length)));
    }
    out
}

/// The nesting depth of contour `i` within `all`: how many other closed contours contain it. An even depth (0, 2,
/// ...) marks an outer boundary, an odd one marks a hole.
pub fn nesting_depth(all: &[Contour], i: usize) -> usize {
    let me = &all[i];
    if me.points.is_empty() {
        return 0;
    }
    let probe = me.centroid();
    all.iter().enumerate().filter(|(k, c)| *k != i && c.closed && c.points.len() >= 3 && c.contains(probe)).count()
}

/// Tessellate an arc into points. The angles are in radians and `ccw` gives the direction. `max_sag` is the largest
/// permitted deviation of a chord from the arc, that is the accuracy of the linearisation.
pub fn tessellate_arc(cx: f64, cy: f64, radius: f64, start: f64, end: f64, ccw: bool, max_sag: f64) -> Vec<Point2> {
    // Normalise the swept angle to the requested direction.
    let mut sweep = end - start;
    if ccw {
        while sweep <= 0.0 {
            sweep += TAU;
        }
    } else {
        while sweep >= 0.0 {
            sweep -= TAU;
        }
    }

    // The segment count follows from the chord sagitta tolerance: max_sag = r(1 - cos(dtheta/2)).
    let sag = max_sag.max(1e-6).min(radius);
    let dtheta_max = 2.0 * (1.0 - sag / radius).clamp(-1.0, 1.0).acos();
    let segs = (sweep.abs() / dtheta_max.max(1e-3)).ceil().max(1.0) as usize;

    let mut pts = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let t = start + sweep * (i as f64 / segs as f64);
        pts.push(Point2::new(cx + radius * t.cos(), cy + radius * t.sin()));
    }
    pts
}

/// A circle as a closed contour.
pub fn circle_contour(cx: f64, cy: f64, radius: f64, max_sag: f64) -> Contour {
    let mut pts = tessellate_arc(cx, cy, radius, 0.0, TAU, true, max_sag);
    pts.pop(); // On a full circle the last point coincides with the first.
    let mut c = Contour::closed(pts);
    // The exact circular edge for the B-rep, which yields a real cylinder when extruded.
    c.edges = vec![ProfEdge::Circle { center: Point2::new(cx, cy), r: radius }];
    c
}

/// A circular contour carrying provenance: one exact edge produced by the sketch entity `src`.
///
/// This is a separate function rather than a field every call site must remember to fill in, because that is exactly
/// how provenance was once lost: the cylindrical wall of a hole was named positionally, although it is the most
/// frequent target of references (a fillet on the rim of a hole, a sketch on its wall).
pub fn circle_contour_from(cx: f64, cy: f64, radius: f64, max_sag: f64, src: crate::model::Id) -> Contour {
    let mut c = circle_contour(cx, cy, radius, max_sag);
    c.edge_src = vec![src];
    c
}

/// How many numbers one edge occupies in the profile encoding.
pub const EDGE_FIELDS: usize = 9;

/// The single source of truth for the edge format of a kernel profile:
/// `[kind, ax, ay, bx, by, cx, cy, ccw, name]`, where kind 0 is a segment, 1 an arc and 2 a circle.
///
/// The ninth number is the name descriptor of the face this edge will produce (0 means no name, and the kernel
/// falls back to a positional index). The name is derived from the recipe as "the wall of feature F from sketch
/// entity E", so inserting an edge in the middle of a contour does not shift the names of its neighbours.
///
/// The format used to be written out in six places (the contour, threads and four sets of tests), and widening the
/// record from 8 numbers to 9 left them out of step: the kernel read the profile with a shift and removed 74 mm³
/// instead of 7515. The record is therefore assembled here alone, both by the product and by the tests.
pub fn encode_edge(e: ProfEdge, name: u32) -> [f64; EDGE_FIELDS] {
    let s = name as f64;
    match e {
        ProfEdge::Line { a, b } => [0.0, a.x, a.y, b.x, b.y, 0.0, 0.0, 0.0, s],
        ProfEdge::Arc { a, b, center, ccw } => [1.0, a.x, a.y, b.x, b.y, center.x, center.y, if ccw { 1.0 } else { 0.0 }, s],
        ProfEdge::Circle { center, r } => [2.0, r, 0.0, 0.0, 0.0, center.x, center.y, 0.0, s],
    }
}

/// One loop of exact edges: `[nedges, edges...]`. Names are absent (0) because the profile does not come from a
/// sketch.
pub fn encode_loop(edges: &[ProfEdge]) -> Vec<f64> {
    let mut v = vec![edges.len() as f64];
    for e in edges {
        v.extend_from_slice(&encode_edge(*e, 0));
    }
    v
}

/// A profile built straight from loops of exact edges: `[nloops, then nedges and the edges of each loop]`. The
/// first loop is the outer one and the rest are holes.
pub fn encode_loops(loops: &[&[ProfEdge]]) -> Vec<f64> {
    let mut v = vec![loops.len() as f64];
    for l in loops {
        v.extend(encode_loop(l));
    }
    v
}

/// The profile encoding for the kernel: an outer contour plus holes flattened into `[L, loop blocks...]` (see
/// `qym_shape_extrude_profile`). Exact edges yield exact faces. `name_of` maps a sketch entity to the name
/// descriptor of the side face it will produce.
pub fn encode_profile_named(outer: &Contour, holes: &[&Contour], name_of: &dyn Fn(crate::model::Id, &ProfEdge) -> u32) -> Vec<f64> {
    let mut v = vec![(1 + holes.len()) as f64];
    v.extend(outer.loop_block_named(name_of));
    for h in holes {
        v.extend(h.loop_block_named(name_of));
    }
    v
}

/// WHICH PIECE OF ITS ENTITY AN EDGE IS, and where along the entity it lies.
///
/// The piece is told by its midpoint, to a micrometre: the same segment met from the two regions on either side of it
/// is one piece. The place along the entity orders the pieces of one entity - a line along its own direction taken
/// one way round, an arc by the angle of its midpoint about the centre - so the numbering does not follow the order
/// in which contours happen to be walked.
pub fn piece_of(e: &ProfEdge) -> ([i64; 2], f64) {
    let key = |x: f64, y: f64| [(x * 1e6).round() as i64, (y * 1e6).round() as i64];
    match *e {
        ProfEdge::Line { a, b } => {
            let (mx, my) = ((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let (mut dx, mut dy) = (b.x - a.x, b.y - a.y);
            let len = (dx * dx + dy * dy).sqrt().max(1e-12);
            if dx < 0.0 || (dx == 0.0 && dy < 0.0) {
                (dx, dy) = (-dx, -dy);
            }
            (key(mx, my), (mx * dx + my * dy) / len)
        }
        ProfEdge::Arc { a, b, center, ccw } => {
            let tau = std::f64::consts::TAU;
            let (aa, ab) = ((a.y - center.y).atan2(a.x - center.x), (b.y - center.y).atan2(b.x - center.x));
            let sweep = if ccw { (ab - aa).rem_euclid(tau) } else { -(aa - ab).rem_euclid(tau) };
            let mid = aa + sweep * 0.5;
            let r = ((a.x - center.x).powi(2) + (a.y - center.y).powi(2)).sqrt();
            (key(center.x + r * mid.cos(), center.y + r * mid.sin()), mid.rem_euclid(tau))
        }
        ProfEdge::Circle { center, r } => (key(center.x + r, center.y), 0.0),
    }
}

/// The same without names, for profiles that do not come from a sketch.
pub fn encode_profile(outer: &Contour, holes: &[&Contour]) -> Vec<f64> {
    encode_profile_named(outer, holes, &|_, _| 0)
}
