//! SMALL GEOMETRY of the window: rays, segments, triangles, circles, polygons.

use super::*;

/// The intersection of a ray (o + t*d) with a plane (point p0, normal n). None means the ray is nearly
/// parallel to the plane.
pub fn ray_plane(o: [f64; 3], d: [f64; 3], p0: [f64; 3], n: [f64; 3]) -> Option<[f64; 3]> {
    let dn = v_dot(d, n);
    if dn.abs() < 1e-9 {
        return None;
    }
    let t = v_dot(v_sub(p0, o), n) / dn;
    Some([o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t])
}

/// The distance from a point to a segment (in screen coordinates).
pub fn screen_dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let (wx, wy) = (p.x - a.x, p.y - a.y);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 1e-6 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.x + t * vx, a.y + t * vy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// The axis segment (origin plus or minus len * dir), for drawing and picking a datum axis.
pub fn axis_segment(origin: [f64; 3], dir: [f64; 3], len: f64) -> ([f64; 3], [f64; 3]) {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let u = if n > 1e-9 { [dir[0] / n, dir[1] / n, dir[2] / n] } else { [0.0, 0.0, 1.0] };
    ([origin[0] - u[0] * len, origin[1] - u[1] * len, origin[2] - u[2] * len], [origin[0] + u[0] * len, origin[1] + u[1] * len, origin[2] + u[2] * len])
}

pub fn dist_to_contour(c: &Contour, p: Point2) -> f64 {
    let pts = &c.points;
    if pts.is_empty() {
        return f64::INFINITY;
    }
    let mut best = f64::INFINITY;
    let n = pts.len();
    let last = if c.closed { n } else { n - 1 };
    for i in 0..last {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        best = best.min(dist_point_seg(p, a, b));
    }
    best
}

pub fn point_in_tri(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let sign = |p: Pos2, a: Pos2, b: Pos2| (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// The surface's depth AT THE CLICKED POINT `p` (a barycentric interpolation of the projected triangle's
/// vertex depths). CRITICAL for picking: the hit's depth used to be the AVERAGE of the triangle's vertex
/// depths — on a thin wall (2 mm) the large triangle of the outer face had its centroid DEEPER than the
/// small triangle of the inner one, so the wrong face won and a sketch landed on the inner wall seemingly
/// at random.
pub fn tri_depth_at(p: Pos2, a: Pos2, da: f64, b: Pos2, db: f64, c: Pos2, dc: f64) -> f64 {
    let (wa, wb, wc) = tri_weights(p, a, b, c);
    da * wa + db * wb + dc * wc
}

/// A CORNER OF A TRIANGLE AS THE VIEW DRAWS IT: where it lies on the screen, where it is in space, and the perspective
/// factor it is drawn with - `Screen::at`'s `1 / (1 + depth / d_eye)`, 1 in an orthographic view.
#[derive(Clone, Copy, Debug)]
pub struct TriCorner {
    pub screen: Pos2,
    pub world: [f64; 3],
    pub persp: f64,
}

/// THE POINT OF THE TRIANGLE `corners` UNDER SCREEN `p`. The centre of the triangle stood here before, and it is
/// not the point pointed at: a click near the corner of a large facet looked at, measured from and trimmed at the
/// middle of the facet - measured, 50 points away on screen on a 60 x 40 block.
///
/// Weights taken on the screen are not weights in space once the view has perspective: the nearer corner is drawn
/// larger, and a point interpolated by the screen weights alone lay 12 points off the pointer on the top of a block
/// seen at three quarters. Each corner's weight is scaled by its perspective factor, as a perspective interpolation is.
pub fn tri_world_at(p: Pos2, corners: [TriCorner; 3]) -> [f64; 3] {
    let [a, b, c] = corners;
    let (la, lb, lc) = tri_weights(p, a.screen, b.screen, c.screen);
    let (ka, kb, kc) = (la * a.persp, lb * b.persp, lc * c.persp);
    let sum = ka + kb + kc;
    let (ka, kb, kc) = if sum.abs() > 1e-12 { (ka / sum, kb / sum, kc / sum) } else { (la, lb, lc) };
    std::array::from_fn(|i| a.world[i] * ka + b.world[i] * kb + c.world[i] * kc)
}

/// The weights of the corners of the screen triangle `a, b, c` at `p`; a third each when it is seen edge-on.
pub(crate) fn tri_weights(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> (f64, f64, f64) {
    let (v0x, v0y) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (v1x, v1y) = ((c.x - a.x) as f64, (c.y - a.y) as f64);
    let (v2x, v2y) = ((p.x - a.x) as f64, (p.y - a.y) as f64);
    let den = v0x * v1y - v1x * v0y;
    if den.abs() < 1e-12 {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0); // degenerate (edge-on to the camera): fall back to the average
    }
    let u = (v2x * v1y - v1x * v2y) / den;
    let v = (v0x * v2y - v2x * v0y) / den;
    (1.0 - u - v, u, v)
}

pub fn dist_point_seg(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2;
    let t = t.clamp(0.0, 1.0);
    p.dist(Point2::new(a.x + ab.x * t, a.y + ab.y * t))
}

pub fn lines_intersect(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<Pos2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Pos2::new(a.x + t * rx, a.y + t * ry))
}

/// An ellipse from its three clicks: the centre `c`, the end `a` of the major semi-axis, and a point `e` whose distance
/// from the major axis is the minor semi-axis. Gives (major semi-axis, rotation in radians, minor semi-axis).
pub fn ellipse_from_clicks(c: Point2, a: Point2, e: Point2) -> (f64, f64, f64) {
    let (dx, dy) = (a.x - c.x, a.y - c.y);
    let major = dx.hypot(dy);
    if major < 1e-9 {
        return (0.0, 0.0, 0.0);
    }
    let minor = ((e.x - c.x) * (-dy) + (e.y - c.y) * dx).abs() / major;
    (major, dy.atan2(dx), minor)
}

/// A constraint's label for the list.
/// The circle through three points, giving (centre x, centre y, radius). None means they are collinear.
pub fn circumcircle(a: Point2, b: Point2, c: Point2) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (a.x * a.x + a.y * a.y, b.x * b.x + b.y * b.y, c.x * c.x + c.y * c.y);
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let r = ((ux - a.x).powi(2) + (uy - a.y).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// The arc tangent to the unit direction `t` at the point `s` and passing through `e`.
/// Returns (cx, cy, r, ccw). None when the end lies on the tangent line, which would make the radius infinite.
pub fn tangent_arc(s: Point2, t: (f64, f64), e: Point2) -> Option<(f64, f64, f64, bool)> {
    let (tx, ty) = t;
    let (nx, ny) = (-ty, tx); // the normal to the tangent: the centre lies on it
    let (sex, sey) = (s.x - e.x, s.y - e.y);
    let denom = 2.0 * (nx * sex + ny * sey);
    if denom.abs() < 1e-9 {
        return None;
    }
    let d = -(sex * sex + sey * sey) / denom; // the signed offset of the centre along the normal
    let (cx, cy) = (s.x + nx * d, s.y + ny * d);
    let r = d.abs();
    if r < 1e-9 {
        return None;
    }
    let (rx, ry) = ((s.x - cx) / r, (s.y - cy) / r); // the radial direction at s
                                                     // the counter-clockwise tangent at s is the radial turned by +90 deg: (-ry, rx)
    let ccw = tx * (-ry) + ty * rx > 0.0;
    Some((cx, cy, r, ccw))
}

/// The two unit world axes that define the plane of a gizmo's rotation ring for axis `ax`.
pub fn ring_axes(ax: u8) -> ([f64; 3], [f64; 3]) {
    match ax {
        0 => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), // the ring about X lies in the YZ plane
        1 => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // about Y, in XZ
        _ => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), // about Z, in XY
    }
}

/// An orthonormal pair (u, v) spanning the plane perpendicular to an arbitrary axis `n` — for the ring of a
/// degree-of-freedom gizmo about a joint's axis, which, unlike `ring_axes`, does not line up with the world
/// X, Y and Z.
pub fn perp_basis(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let ln = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    let a = [n[0] / ln, n[1] / ln, n[2] / ln];
    // take the world axis least collinear with `a` as the seed
    let seed = if a[0].abs() <= a[1].abs() && a[0].abs() <= a[2].abs() {
        [1.0, 0.0, 0.0]
    } else if a[1].abs() <= a[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let mut u = [a[1] * seed[2] - a[2] * seed[1], a[2] * seed[0] - a[0] * seed[2], a[0] * seed[1] - a[1] * seed[0]];
    let lu = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt().max(1e-9);
    u = [u[0] / lu, u[1] / lu, u[2] / lu];
    let v = [a[1] * u[2] - a[2] * u[1], a[2] * u[0] - a[0] * u[2], a[0] * u[1] - a[1] * u[0]];
    (u, v)
}

/// Rotate the point `p` about an axis (origin `o`, direction `dir`) by `ang` radians (Rodrigues) — for the
/// preview of a circular pattern. It matches the core's `rot_about_axis` (a right-handed frame).
pub fn rotate_pt_about_axis(o: [f64; 3], dir: [f64; 3], ang: f64, p: [f64; 3]) -> [f64; 3] {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let k = if n > 1e-12 { [dir[0] / n, dir[1] / n, dir[2] / n] } else { [0.0, 0.0, 1.0] };
    let v = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let (c, s) = (ang.cos(), ang.sin());
    let kdv = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let kxv = [k[1] * v[2] - k[2] * v[1], k[2] * v[0] - k[0] * v[2], k[0] * v[1] - k[1] * v[0]];
    [
        v[0] * c + kxv[0] * s + k[0] * kdv * (1.0 - c) + o[0],
        v[1] * c + kxv[1] * s + k[1] * kdv * (1.0 - c) + o[1],
        v[2] * c + kxv[2] * s + k[2] * kdv * (1.0 - c) + o[2],
    ]
}

/// The signed area (the edge function) used for barycentric coordinates and rasterisation.
pub fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// The projection of point `p` onto the SEGMENT a -> b (clamped to its ends). None means a degenerate segment.
pub fn project_on_seg(p: Point2, a: Point2, b: Point2) -> Option<Point2> {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = vx * vx + vy * vy;
    if len2 < 1e-12 {
        return None;
    }
    let t = (((p.x - a.x) * vx + (p.y - a.y) * vy) / len2).clamp(0.0, 1.0);
    Some(Point2::new(a.x + t * vx, a.y + t * vy))
}

pub fn bounds(contours: &[Contour]) -> Option<(Point2, Point2)> {
    let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in contours {
        for p in &c.points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
    }
    min.x.is_finite().then_some((min, max))
}

/// The Id of a body by its mesh index, IF it is a B-rep body (a live shape exists) — otherwise None (a raw import).
pub fn brep_at(live: &LiveGeom, project: &Project, mi: usize) -> Option<Id> {
    project.mesh_id(mi).filter(|id| live.shapes.contains_key(id))
}

/// Select the feature that produced the body (so its parameters show up at once) and fit the view.
pub fn select_body(project: &mut Project, sel: &mut Sel, view: &mut View2d, body: Id) {
    if let Some(ti) = project.timeline.iter().position(|n| n.kind.body() == Some(body)) {
        *sel = Sel::Feature(ti);
    } else if let Some(mi) = project.mesh_index(body) {
        *sel = Sel::Mesh(mi);
    }
    view.initialized = false;
}

/// The sketch anchor under the cursor - for dimensions and constraints between ANY geometry.
#[derive(Clone, Copy)]
pub enum SketchRef {
    /// An existing point (a circle's or an arc's centre included).
    Point(Id),
    /// The midpoint of segment (a,b) - materialised as a point with a Midpoint constraint.
    Midpoint(Id, Id),
    /// The origin (0,0) - materialised as a Fixed reference point.
    Origin,
}

/// THE NAME OF THE OBJECT in the header: editable, shown only, or absent.
///
/// The edit is returned outwards rather than written here: for a body the name is set by
/// `set_mesh_name`, for a sketch by a field of the structure, and a feature has no name in that sense
/// at all. The write paths differ in substance, and folding them into one would be a lie for the sake
/// of symmetry.
pub enum NameSlot {
    /// The stored value (may be an auto-name key — shown translated, written only if touched).
    Editable(String),
    Fixed(String),
    None,
}

/// THE LINEAGE OF AN OBJECT, ready to be shown: what created it and what depends on it.
#[derive(Default)]
pub struct Lineage {
    pub built_on: Vec<String>,
    pub dependents: Vec<String>,
}

/// The point on the segment a -> b nearest to `p` that also lies on a GRID LINE (x = n*g or y = n*g) — the
/// intersection of an edge with the grid. For a vertical edge it ties Y to the grid (the edge holds X), for
/// a horizontal one it ties X, and for a slanted one it takes the nearest crossing with the grid. None when
/// there is no crossing within the segment.
pub fn grid_cross_on_seg(p: Point2, a: Point2, b: Point2, g: f64) -> Option<Point2> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx * dx + dy * dy < 1e-12 || g <= 0.0 {
        return None;
    }
    let mut best: Option<(f64, Point2)> = None;
    let mut consider = |q: Point2| {
        let dd = (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
        if best.is_none_or(|(bd, _)| dd < bd) {
            best = Some((dd, q));
        }
    };
    if dx.abs() > 1e-9 {
        let gx = (p.x / g).round() * g; // the nearest vertical grid line
        let t = (gx - a.x) / dx;
        if (-1e-6..=1.0 + 1e-6).contains(&t) {
            consider(Point2::new(gx, a.y + dy * t));
        }
    }
    if dy.abs() > 1e-9 {
        let gy = (p.y / g).round() * g; // the nearest horizontal grid line
        let t = (gy - a.y) / dy;
        if (-1e-6..=1.0 + 1e-6).contains(&t) {
            consider(Point2::new(a.x + dx * t, gy));
        }
    }
    best.map(|(_, q)| q)
}

/// The intersection of two SEGMENTS a1 -> a2 and b1 -> b2, within both. None means there is none, or they are parallel.
pub fn seg_seg_intersect(a1: Point2, a2: Point2, b1: Point2, b2: Point2) -> Option<Point2> {
    let (rx, ry) = (a2.x - a1.x, a2.y - a1.y);
    let (sx, sy) = (b2.x - b1.x, b2.y - b1.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-12 {
        return None;
    }
    let (qpx, qpy) = (b1.x - a1.x, b1.y - a1.y);
    let t = (qpx * sy - qpy * sx) / denom;
    let u = (qpx * ry - qpy * rx) / denom;
    if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some(Point2::new(a1.x + t * rx, a1.y + t * ry))
    } else {
        None
    }
}

/// The intersection of INFINITE lines (through the segments ab and cd) in the sketch's world coordinates.
/// None means they are parallel. Used to orient an angular dimension's directions outwards from the
/// intersection, so the angle shown is the visible opening rather than its supplement.
pub fn line_line_ix(a: Point2, b: Point2, c: Point2, d: Point2) -> Option<Point2> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denom;
    Some(Point2::new(a.x + t * rx, a.y + t * ry))
}

/// The points where the SEGMENT a -> b crosses a circle (centre c, radius r), within the segment.
pub fn seg_circle_intersect(a: Point2, b: Point2, c: Point2, r: f64) -> Vec<Point2> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (fx, fy) = (a.x - c.x, a.y - c.y);
    let aa = dx * dx + dy * dy;
    let bb = 2.0 * (fx * dx + fy * dy);
    let cc = fx * fx + fy * fy - r * r;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < 0.0 || aa < 1e-12 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    let mut out = Vec::new();
    for t in [(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)] {
        if (-1e-9..=1.0 + 1e-9).contains(&t) {
            out.push(Point2::new(a.x + t * dx, a.y + t * dy));
        }
    }
    out
}

/// A 3x4 row-major translation matrix (for the Move feature and for a body's transform).
pub fn mat_translate(dx: f64, dy: f64, dz: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, dx, 0.0, 1.0, 0.0, dy, 0.0, 0.0, 1.0, dz]
}

/// Is the line (a, b) a coordinate axis of the sketch (one of its axis reference points)?
pub fn is_axis_line(project: &qymcad_core::model::Project, si: usize, a: Id, b: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.axis_pts.contains(&a) || s.axis_pts.contains(&b))
}

/// The LINES of the profile sketch that are candidates for the axis of revolution (a centreline). They
/// are chosen by a button or a combo box on the bar, NOT by a click in 3D: sketch geometry is not
/// hit-tested in the command's 3D view.
pub fn profile_axis_lines(project: &qymcad_core::model::Project, si: usize) -> Vec<Id> {
    // ANY straight line of the sketch can be the axis, not only a construction one — the usual CAD
    // behaviour. The core never minded; the restriction sat right here, in the list of candidates, and a
    // plain line drawn to serve as an axis simply did not appear on the command's bar. Reported
    // behaviour: only X or Y could be chosen, an axis of one's own could not.
    // Construction lines come FIRST: they are drawn precisely to serve as an axis, and that is the most
    // frequent choice.
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let lines = |constr: bool| s.entities.iter().filter(move |e| e.construction == constr && matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).map(|e| e.id);
    lines(true).chain(lines(false)).collect()
}

/// Whether a point lies inside a polygon (by ray casting). The contour is closed implicitly.
pub fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
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

pub fn poly_area(poly: &[Pos2]) -> f64 {
    let n = poly.len();
    let mut s = 0.0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        s += (a.x as f64) * (b.y as f64) - (b.x as f64) * (a.y as f64);
    }
    (s * 0.5).abs()
}
