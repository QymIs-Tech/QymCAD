//! THE CURVES OF A SKETCH MEET: intersections of lines, circles and ellipses, the points of entities remapped.

use super::*;

/// Whether point `p` lies inside polygon `poly` (ray casting).
pub(crate) fn point_in_poly(poly: &[Point2], p: Point2) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether the contours touch: the inner one reached the boundary of the outer one.
///
/// Such a contour is not a hole. An inner loop that touches the outer loop forms no face, and the whole
/// region containing it silently disappears. The tolerance is relative (a fraction of the outer bounding
/// box): an absolute one reports false touches on a part hundreds of millimetres across and misses real
/// ones on a small part.
pub(crate) fn polys_touch(outer: &[Point2], inner: &[Point2]) -> bool {
    if outer.len() < 3 || inner.len() < 3 {
        return false;
    }
    let (mut lo, mut hi) = (Point2::new(f64::MAX, f64::MAX), Point2::new(f64::MIN, f64::MIN));
    for p in outer {
        lo = Point2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let tol = ((hi.x - lo.x).hypot(hi.y - lo.y) * 1e-6).max(1e-9);
    for a in inner {
        for i in 0..outer.len() {
            let (b, c) = (outer[i], outer[(i + 1) % outer.len()]);
            let (vx, vy) = (c.x - b.x, c.y - b.y);
            let l2 = vx * vx + vy * vy;
            let t = if l2 > 1e-18 { (((a.x - b.x) * vx + (a.y - b.y) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            if (a.x - (b.x + t * vx)).hypot(a.y - (b.y + t * vy)) <= tol {
                return true;
            }
        }
    }
    false
}

pub(crate) fn poly_contains(outer: &[Point2], inner: &[Point2]) -> bool {
    !inner.is_empty() && inner.iter().all(|p| point_in_poly(outer, *p))
}

/// Orient `axis` from `center` towards the material: the side of the plane through `center` perpendicular
/// to `axis` holding more of `verts` points into the body.
///
/// Used by threads, which grow from the rim into the material. Counting vertices is more reliable than a
/// centroid on asymmetric or L-shaped bodies.
pub fn orient_axis_into_mesh(center: [f64; 3], axis: [f64; 3], verts: &[crate::geom::Point3]) -> [f64; 3] {
    let (mut pos, mut neg) = (0i64, 0i64);
    for v in verts {
        let dp = (v.x - center[0]) * axis[0] + (v.y - center[1]) * axis[1] + (v.z - center[2]) * axis[2];
        if dp > 1e-6 {
            pos += 1;
        } else if dp < -1e-6 {
            neg += 1;
        }
    }
    if neg > pos {
        [-axis[0], -axis[1], -axis[2]]
    } else {
        axis
    }
}

/// Value of dimension `key`: the expression from `dims` when it is present and evaluates against `vars`,
/// otherwise `fallback`.
pub(crate) fn eval_dim(dims: Option<&std::collections::HashMap<String, String>>, key: &str, fallback: f64, vars: &std::collections::HashMap<String, f64>) -> f64 {
    dims.and_then(|m| m.get(key)).map(|e| e.trim()).filter(|e| !e.is_empty()).and_then(|e| crate::expr::eval(e, vars).ok()).unwrap_or(fallback)
}

/// A sketch point with a stable id, referenced by entities and dimensions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SketchPoint {
    pub id: Id,
    pub x: f64,
    pub y: f64,
}

/// A typed sketch entity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SketchEntity {
    pub id: Id,
    pub kind: EntityKind,
    /// Construction geometry: never part of a profile, only a support for snapping and constraints. Drawn
    /// dashed.
    #[serde(default)]
    pub construction: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    /// Segment between points `a` and `b`.
    Line { a: Id, b: Id },
    /// Arc around `center` from `a` to `b` (counter-clockwise when `ccw`).
    Arc { center: Id, a: Id, b: Id, ccw: bool },
    /// Circle around `center` with radius `r`.
    Circle { center: Id, r: f64 },
    /// Ellipse: centre `c`, endpoint of the major semi-axis `ma`, endpoint of the minor semi-axis `mi`.
    /// The semi-axes are kept perpendicular by an implicit constraint; the major axis is |c-ma|, the minor
    /// |c-mi|, and the rotation is the direction from `c` to `ma`. A real analytic entity with five degrees
    /// of freedom, not a polygon.
    Ellipse { c: Id, ma: Id, mi: Id },
}

/// Snapshot of sketch geometry for the clipboard (copy, cut and paste between sketches or within one).
///
/// Points keep their old local ids so entity references can be remapped; `ref_x` and `ref_y` are the
/// reference point given at copy time, from which the paste is measured.
#[derive(Clone, Debug, Default)]
pub struct GeomClip {
    /// Points: (old local id, x, y).
    pub points: Vec<(Id, f64, f64)>,
    /// Entities holding the old ids; they are remapped on paste.
    pub entities: Vec<SketchEntity>,
    /// Constraints and dimensions internal to the set (every reference inside the copy, except `Fixed`),
    /// holding the old ids and remapped on paste. This way a copy carries its own shape (horizontals,
    /// verticals, edge dimensions) but not the anchors or the axes.
    pub constraints: Vec<Constraint>,
    /// Reference point, given at copy time.
    pub ref_x: f64,
    pub ref_y: f64,
}

impl GeomClip {
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// Intersection of segments `a-b` and `c-d`. Returns the parameter t in (0,1) along `a-b` when the
/// intersection lies strictly inside `a-b` and within `c-d`.
pub(crate) fn seg_seg_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (rx, ry) = (bx - ax, by - ay);
    let (sx, sy) = (dx - cx, dy - cy);
    let rxs = rx * sy - ry * sx;
    if rxs.abs() < 1e-12 {
        return None; // parallel
    }
    let (qpx, qpy) = (cx - ax, cy - ay);
    let t = (qpx * sy - qpy * sx) / rxs;
    let u = (qpx * ry - qpy * rx) / rxs;
    if t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Intersection of the infinite lines through `a-b` and `c-d`. `None` when they are parallel.
pub(crate) fn line_intersect_inf(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<(f64, f64)> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (r1, r2) = (bx - ax, by - ay);
    let (s1, s2) = (dx - cx, dy - cy);
    let den = r1 * s2 - r2 * s1;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((cx - ax) * s2 - (cy - ay) * s1) / den;
    Some((ax + r1 * t, ay + r2 * t))
}

/// Tangency tolerance, in millimetres.
///
/// The solver drives a tangency to its own precision rather than to an absolute zero, so "does it touch"
/// has to be decided in units of length. The value matches the one used to stitch coincident points
/// (1e-3 mm): if two points that close count as one, a tangency that close counts as a tangency. It is
/// orders of magnitude below any design intent, so it produces no false cuts.
pub(crate) const TOUCH_TOL: f64 = 1e-3;

/// Roots of the intersection of the line `a-b` with a circle, treating tangency honestly.
///
/// Tangency must not be decided by "discriminant is near zero": the scale of the discriminant depends on
/// the segment length and the radius, so a single threshold there either catches too much or misses the
/// real thing. Measured case: a line 1.6e-10 mm from the rim is geometrically tangent, yet the
/// discriminant came out at -2.9e-6 against a 1e-12 threshold, so no cut was made and three lines tangent
/// to a circle formed no closed region to extrude. Distance is measured instead: when the point of the
/// line nearest the centre differs from the radius by less than the tolerance, there is one tangency
/// point.
pub(crate) fn line_circle_roots(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    let (dx, dy) = (bx - ax, by - ay);
    let (fx, fy) = (ax - cx, ay - cy);
    let aa = dx * dx + dy * dy;
    if aa < 1e-12 {
        return Vec::new();
    }
    let bb = 2.0 * (fx * dx + fy * dy);
    let cc = fx * fx + fy * fy - r * r;
    // The point of the line nearest the centre, and its deviation from the rim.
    let t_near = -bb / (2.0 * aa);
    let (px, py) = (ax + dx * t_near, ay + dy * t_near);
    let gap = ((px - cx).hypot(py - cy) - r).abs();
    let tol = TOUCH_TOL * r.abs().max(1.0); // tolerance in units of length (mm), not in units of the discriminant
    if gap <= tol {
        return vec![t_near];
    }
    let disc = bb * bb - 4.0 * aa * cc;
    if disc <= 0.0 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    vec![(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)]
}

/// Parameters t in (0,1) along segment `a-b` where it crosses the circle (cx, cy, r).
pub(crate) fn seg_circle_t(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    // Endpoints are included: an intersection exactly at a vertex (where the first trim already left a
    // point) is a valid cut, otherwise trimming again at the same point finds nothing.
    //
    // The endpoint tolerance is geometric rather than a fixed 1e-9 in the parameter. Measured: a tangent
    // line 2e-7 mm from the rim put the tangency point 5e-7 of the parameter before the start of the
    // segment, the cut was discarded, and three tangent lines formed no closed region to extrude. A
    // parameter threshold unrelated to length is the same mistake as measuring tangency by a zero
    // discriminant: it is stricter than a millimetre on a long segment and looser on a short one.
    let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt().max(1e-12);
    let te = (TOUCH_TOL / len).max(1e-9); // mm converted into a fraction of the segment length
    line_circle_roots(ax, ay, bx, by, cx, cy, r).into_iter().filter(|&t| t > -te && t < 1.0 + te).collect()
}

/// Parameter t along the infinite line `a-b` where it crosses the segment `c-d`.
pub(crate) fn line_seg_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [dx, dy]) = (a, b, c, d);
    let (rx, ry) = (bx - ax, by - ay);
    let (sx, sy) = (dx - cx, dy - cy);
    let den = rx * sy - ry * sx;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((cx - ax) * sy - (cy - ay) * sx) / den;
    let u = ((cx - ax) * ry - (cy - ay) * rx) / den;
    if (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Parameters t along the infinite line `a-b` where it crosses a circle.
pub(crate) fn line_circle_t(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    line_circle_roots(ax, ay, bx, by, cx, cy, r)
}

/// Semi-axes of an ellipse from its centre and axis endpoints. The major axis is `c` to `ma` (length
/// `major`), the minor axis is perpendicular to it (length |c-mi|), matching `ellipse_contour`. Returns
/// the unit vector of the major axis (ux, uy) together with `major` and `minor`.
pub(crate) fn ellipse_axes(cx: f64, cy: f64, max: f64, may: f64, mix: f64, miy: f64) -> (f64, f64, f64, f64) {
    let major = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-9);
    let minor = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-9);
    let (ux, uy) = ((max - cx) / major, (may - cy) / major);
    (ux, uy, major, minor)
}

/// Parameters t along the infinite line `a-b` where it crosses an ellipse with centre (cx, cy), major-axis
/// unit vector (ux, uy) and semi-axes `major` and `minor` (the minor axis is perpendicular to the major
/// one). In local normalised coordinates the ellipse is a unit circle, which makes this a quadratic. Zero
/// to two roots; filtering by t is left to the caller.
pub(crate) fn line_ellipse_roots(a: [f64; 2], b: [f64; 2], c: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<f64> {
    let ([ax, ay], [bx, by], [cx, cy], [ux, uy]) = (a, b, c, u);
    let (vx, vy) = (-uy, ux); // unit vector of the minor axis
    let (dx, dy) = (bx - ax, by - ay);
    let (acx, acy) = (ax - cx, ay - cy);
    let xa = (acx * ux + acy * uy) / major;
    let xd = (dx * ux + dy * uy) / major;
    let ya = (acx * vx + acy * vy) / minor;
    let yd = (dx * vx + dy * vy) / minor;
    let aa = xd * xd + yd * yd;
    let bb = 2.0 * (xa * xd + ya * yd);
    let cc = xa * xa + ya * ya - 1.0;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < 0.0 || aa < 1e-18 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    vec![(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)]
}

/// Parameters t along the segment `a-b`, within [0,1] plus an endpoint tolerance, where it crosses an
/// ellipse.
pub(crate) fn seg_ellipse_t(a: [f64; 2], b: [f64; 2], c: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<f64> {
    line_ellipse_roots(a, b, c, u, major, minor).into_iter().filter(|&t| t > -1e-9 && t < 1.0 + 1e-9).collect()
}

/// Intersection points of a circle (cx, cy, r) with an ellipse (centre ce, major-axis unit vector u,
/// semi-axes `major` and `minor`).
///
/// There is no closed form (the equation is a quartic), so the roots of f(t) = |E(t) - c|^2 - r^2 are
/// found numerically over the ellipse parameter, refining each sign-change interval by bisection. Up to
/// four points.
pub(crate) fn circle_ellipse_pts(c: [f64; 2], r: f64, ce: [f64; 2], u: [f64; 2], major: f64, minor: f64) -> Vec<(f64, f64)> {
    let ([cx, cy], [cex, cey], [ux, uy]) = (c, ce, u);
    use std::f64::consts::TAU;
    let (vx, vy) = (-uy, ux);
    let e = |th: f64| {
        let (ct, st) = (th.cos(), th.sin());
        (cex + major * ct * ux + minor * st * vx, cey + major * ct * uy + minor * st * vy)
    };
    let f = |th: f64| {
        let (ex, ey) = e(th);
        (ex - cx).powi(2) + (ey - cy).powi(2) - r * r
    };
    let n = 256;
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut prev_th = 0.0_f64;
    let mut prev = f(0.0);
    for k in 1..=n {
        let th = TAU * k as f64 / n as f64;
        let cur = f(th);
        if prev == 0.0 {
            out.push(e(prev_th));
        } else if prev * cur < 0.0 {
            let (mut lo, mut hi, mut flo) = (prev_th, th, prev);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                let fm = f(mid);
                if flo * fm <= 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                    flo = fm;
                }
            }
            out.push(e(0.5 * (lo + hi)));
        }
        prev_th = th;
        prev = cur;
    }
    out
}

/// Intersection points of two circles (cx1, cy1, r1) and (cx2, cy2, r2): zero, one or two points.
pub(crate) fn circle_circle_pts(cx1: f64, cy1: f64, r1: f64, cx2: f64, cy2: f64, r2: f64) -> Vec<(f64, f64)> {
    let (dx, dy) = (cx2 - cx1, cy2 - cy1);
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-9 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let (mx, my) = (cx1 + a * dx / d, cy1 + a * dy / d);
    let (ox, oy) = (-dy / d * h, dx / d * h);
    if h < 1e-9 {
        vec![(mx, my)]
    } else {
        vec![(mx + ox, my + oy), (mx - ox, my - oy)]
    }
}

/// Whether angle `ang` lies on the arc from `a0` to `a1` (`ccw` means counter-clockwise).
///
/// The test is relative to the start, through the swept angle, so it is stable across the wrap at 0 and
/// 2*pi: an intersection exactly at the end of the arc, where floating-point error can push the angle
/// past zero, still counts. Boundary tolerance is 1e-6.
pub(crate) fn angle_in_arc(ang: f64, a0: f64, a1: f64, ccw: bool) -> bool {
    let tau = std::f64::consts::TAU;
    let (s, e) = if ccw { (a0, a1) } else { (a1, a0) };
    let sweep = (e - s).rem_euclid(tau); // arc length along the traversal direction, [0, 2*pi)
    let rel = (ang - s).rem_euclid(tau); // position of the angle from the start, [0, 2*pi)
    rel <= sweep + 1e-6 || rel >= tau - 1e-6 // within the swept angle, or right at the start (the wrap)
}

impl Constraint {
    /// Expression of a dimensional constraint, when it has one; used to rebuild selectively by parameter
    /// name.
    pub fn expr(&self) -> Option<&str> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr).filter(|e| !e.is_empty()).map(|e| e.as_str()),
            _ => None,
        }
    }

    /// Mutable access to a dimension expression. Renaming a driver needs it: sketch formulas reference
    /// names just as project parameters and feature parameters do, and skipping them would leave the
    /// document holding references to a name that no longer exists.
    pub fn expr_mut(&mut self) -> Option<&mut String> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr),
            _ => None,
        }
    }
}

/// Whether a constraint references any of the lines being deleted (by the unordered pair of endpoints).
///
/// Line-dependent constraints (horizontal, vertical, distance, parallel, perpendicular, equal, collinear,
/// tangent, midpoint) lose their meaning without their line and have to go with it.
pub(crate) fn constraint_uses_line(c: &Constraint, lines: &[(Id, Id)]) -> bool {
    let same = |x: Id, y: Id| lines.iter().any(|&(a, b)| (a == x && b == y) || (a == y && b == x));
    match *c {
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Distance { a, b, .. } => same(a, b),
        Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => same(a, b) || same(c, d),
        Constraint::Tangent { a, b, .. } => same(a, b),
        Constraint::Midpoint { a, b, .. } => same(a, b),
        Constraint::PointOnLine { a, b, .. } => same(a, b),
        Constraint::DistancePL { a, b, .. } => same(a, b),
        Constraint::AngleLines { a, b, c, d, .. } => same(a, b) || same(c, d),
        _ => false,
    }
}

/// Every point id a constraint references. The single source of truth for validating and pruning orphaned
/// constraints and for moving references when a fillet or a chamfer is applied.
pub fn constraint_point_ids(c: &Constraint) -> Vec<Id> {
    match *c {
        Constraint::Fixed { p } => vec![p],
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => vec![a, b],
        Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => vec![a, b, c, d],
        Constraint::Angle { a, b, c, .. } => vec![a, b, c],
        Constraint::Midpoint { p, a, b } => vec![p, a, b],
        Constraint::Tangent { a, b, c, .. } => vec![a, b, c],
        Constraint::Symmetric { a, b, la, lb } => vec![a, b, la, lb],
        Constraint::PointOnLine { p, a, b } => vec![p, a, b],
        Constraint::DistancePL { p, a, b, .. } => vec![p, a, b],
        Constraint::EdgeDistance { c1, c2, .. } => vec![c1, c2],
        Constraint::AngleLines { a, b, c, d, .. } => vec![a, b, c, d],
        Constraint::Diameter { c, .. } => vec![c],
        Constraint::EqualRadius { c1, c2 } => vec![c1, c2],
        Constraint::CircleTangent { c1, c2, .. } => vec![c1, c2],
        Constraint::PointOnCircle { p, c } => vec![p, c],
        Constraint::Concentric { c1, c2 } => vec![c1, c2],
        Constraint::ArcLength { c, a, b, .. } => vec![c, a, b],
    }
}

/// Replace a reference to point `from` with `to` in every point field of a constraint: the mutating
/// counterpart of [`constraint_point_ids`].
///
/// Needed by fillets and chamfers, where the corner point disappears and constraints that referenced it
/// move to the tangency point. Non-point fields (radii, expressions) are left alone.
/// The points an entity stands on.
pub fn entity_points(e: &SketchEntity) -> Vec<Id> {
    match e.kind {
        EntityKind::Line { a, b } => vec![a, b],
        EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
        EntityKind::Circle { center, .. } => vec![center],
        EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
    }
}

/// Re-point one entity from one point to another.
pub fn remap_entity_point(e: &mut SketchEntity, from: Id, to: Id) {
    let fix = |id: &mut Id| {
        if *id == from {
            *id = to;
        }
    };
    match &mut e.kind {
        EntityKind::Line { a, b } => {
            fix(a);
            fix(b);
        }
        EntityKind::Arc { center, a, b, .. } => {
            fix(center);
            fix(a);
            fix(b);
        }
        EntityKind::Circle { center, .. } => fix(center),
        EntityKind::Ellipse { c, ma, mi } => {
            fix(c);
            fix(ma);
            fix(mi);
        }
    }
}

/// The same as `remap_constraint_point`, reachable from the sketch module.
pub fn remap_constraint_point_pub(c: &mut Constraint, from: Id, to: Id) {
    remap_constraint_point(c, from, to);
}

pub(crate) fn remap_constraint_point(c: &mut Constraint, from: Id, to: Id) {
    let fix = |id: &mut Id| {
        if *id == from {
            *id = to;
        }
    };
    match c {
        Constraint::Fixed { p } => fix(p),
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => {
            fix(a);
            fix(b);
        }
        Constraint::Parallel { a, b, c, d }
        | Constraint::Perpendicular { a, b, c, d }
        | Constraint::Equal { a, b, c, d }
        | Constraint::Collinear { a, b, c, d }
        | Constraint::AngleLines { a, b, c, d, .. } => {
            fix(a);
            fix(b);
            fix(c);
            fix(d);
        }
        Constraint::Angle { a, b, c, .. } | Constraint::Tangent { a, b, c, .. } => {
            fix(a);
            fix(b);
            fix(c);
        }
        Constraint::Midpoint { p, a, b } | Constraint::PointOnLine { p, a, b } | Constraint::DistancePL { p, a, b, .. } => {
            fix(p);
            fix(a);
            fix(b);
        }
        Constraint::Symmetric { a, b, la, lb } => {
            fix(a);
            fix(b);
            fix(la);
            fix(lb);
        }
        Constraint::EdgeDistance { c1, c2, .. } | Constraint::EqualRadius { c1, c2 } | Constraint::CircleTangent { c1, c2, .. } | Constraint::Concentric { c1, c2 } => {
            fix(c1);
            fix(c2);
        }
        Constraint::Diameter { c, .. } => fix(c),
        Constraint::PointOnCircle { p, c } => {
            fix(p);
            fix(c);
        }
        Constraint::ArcLength { c, a, b, .. } => {
            fix(c);
            fix(a);
            fix(b);
        }
    }
}

/// Clone of a constraint with every point reference translated through `map` (old id to new id).
///
/// Points missing from `map` are left as they are; for constraints internal to a copy there are none.
/// Fresh ids from `map` cannot collide with the old ones, so remapping in sequence is safe.
pub(crate) fn remap_constraint_via(c: &Constraint, map: &std::collections::HashMap<Id, Id>) -> Constraint {
    let mut nc = c.clone();
    for old in constraint_point_ids(c) {
        if let Some(&new) = map.get(&old) {
            remap_constraint_point(&mut nc, old, new);
        }
    }
    nc
}

/// THE PALETTE OF A BODY'S COLOURS, given one per triangle, and the place of each triangle's colour in it. The places are
/// bytes, so a palette holds 255. Up to that the distinct colours stand in the order they come. Past it - a scan, whose
/// colours run to thousands - 255 are cut out of them by median, the colours weighed by how many triangles have them,
/// and every colour takes the nearest; the first 255 met would leave a colour met later with nothing like it.
pub fn palette_of(colours: &[[u8; 3]]) -> (Vec<[u8; 3]>, Vec<u8>) {
    let mut place: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    let mut distinct: Vec<([u8; 3], u64)> = Vec::new();
    for c in colours {
        let k = *place.entry(*c).or_insert_with(|| {
            distinct.push((*c, 0));
            distinct.len() - 1
        });
        distinct[k].1 += 1;
    }
    if distinct.len() <= 255 {
        return (distinct.iter().map(|(c, _)| *c).collect(), colours.iter().map(|c| place[c] as u8).collect());
    }
    // split the box whose colours spread widest along a channel, at the median of the triangles, until there are 255
    let spread = |b: &[([u8; 3], u64)], ch: usize| b.iter().map(|(c, _)| c[ch]).max().unwrap_or(0) - b.iter().map(|(c, _)| c[ch]).min().unwrap_or(0);
    let mut boxes = vec![distinct];
    while boxes.len() < 255 {
        let Some((bi, ch, _)) = boxes.iter().enumerate().filter(|(_, b)| b.len() > 1).flat_map(|(i, b)| (0..3).map(move |ch| (i, ch, spread(b, ch)))).max_by_key(|x| x.2) else { break };
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|(c, _)| c[ch]);
        let total: u64 = b.iter().map(|(_, n)| n).sum();
        let mut acc = 0;
        let cut = b
            .iter()
            .position(|(_, n)| {
                acc += n;
                acc * 2 >= total
            })
            .map_or(1, |k| k + 1)
            .clamp(1, b.len() - 1);
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let palette: Vec<[u8; 3]> = boxes
        .iter()
        .map(|b| {
            let total: u64 = b.iter().map(|(_, n)| n).sum();
            std::array::from_fn(|ch| ((b.iter().map(|(c, n)| u64::from(c[ch]) * n).sum::<u64>() + total / 2) / total) as u8)
        })
        .collect();
    let near = |c: &[u8; 3]| (0..palette.len()).min_by_key(|&k| (0..3).map(|i| u32::from(palette[k][i].abs_diff(c[i])).pow(2)).sum::<u32>()).unwrap_or(0) as u8;
    let nearest: std::collections::HashMap<[u8; 3], u8> = place.keys().map(|c| (*c, near(c))).collect();
    let places = colours.iter().map(|c| nearest[c]).collect();
    (palette, places)
}

/// Default part colour from the palette, by index, so an assembly of many bodies is not monochrome. Soft,
/// distinguishable tones; the palette wraps around.
pub fn default_part_color(index: usize) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [
        [150, 170, 190], // steel
        [200, 160, 110], // brass
        [140, 180, 150], // green
        [180, 150, 175], // lilac
        [170, 175, 120], // olive
        [130, 165, 195], // light blue
        [195, 145, 135], // terracotta
        [160, 160, 170], // grey
    ];
    PALETTE[index % PALETTE.len()]
}
