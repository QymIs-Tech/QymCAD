//! Measurement in 3D: pure mathematics over already resolved geometry.
//!
//! Measuring used to be possible only inside a sketch, between two points on a plane. Distance between faces,
//! a gap between parts, an angle of convergence, the diameter of a hole — none of that could be measured in 3D
//! at all, although it is a tool that sits on a hotkey in every professional CAD.
//!
//! The mathematics lives here and knows nothing about picking or about the screen: the application resolves a
//! click into a [`MeasureItem`], and the numbers are computed here and checked by tests against known geometry.
//! Code like this used to compute its numbers straight inside the click handler, where the only way to check
//! them was by eye.

/// What is being measured: the geometric primitive a pick resolved into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureItem {
    /// A vertex.
    Point([f64; 3]),
    /// A straight edge: a point on it, a direction that need not be a unit vector, and a length.
    Line { origin: [f64; 3], dir: [f64; 3], len: f64 },
    /// A circular edge, such as the rim of a hole or a fillet.
    Circle { center: [f64; 3], axis: [f64; 3], r: f64 },
    /// A planar face: origin, normal, and optional surface area and loop perimeter.
    Plane { origin: [f64; 3], normal: [f64; 3], area: Option<f64>, perimeter: Option<f64> },
    /// A cylindrical face: the wall of a hole or of a shaft, optional surface area and length.
    Cylinder { origin: [f64; 3], axis: [f64; 3], r: f64, area: Option<f64>, length: Option<f64> },
}

/// The result of a measurement. There are several fields because different pairs make different things
/// meaningful: two points have a distance and its projections onto the axes, two planes have either a distance,
/// when parallel, or an angle. An empty field means the quantity is meaningless for this pair, not that it is
/// zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasureResult {
    /// The shortest distance, in mm.
    pub distance: Option<f64>,
    /// Center-to-center distance for cylindrical or circular elements, in mm.
    pub center_to_center: Option<f64>,
    /// The angle between directions or normals, in degrees.
    pub angle_deg: Option<f64>,
    /// The distance broken down by axis (Delta X, Delta Y, Delta Z).
    pub delta: Option<[f64; 3]>,
    /// Edge length, radius or diameter: whatever is meaningful for a single selected element.
    pub value: Option<(&'static str, f64)>,
    /// Surface area in mm^2 (for planar or cylindrical faces).
    pub area: Option<f64>,
    /// Loop perimeter in mm (for planar faces).
    pub perimeter: Option<f64>,
    /// Length / depth in mm (for cylinders or lines).
    pub length: Option<f64>,
    /// The two closest points in 3D between which distance and deltas are measured.
    pub closest_points: Option<([f64; 3], [f64; 3])>,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = len(a);
    if l < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// The angle between directions in degrees, folded into [0, 90]: the sign of a direction is arbitrary for lines
/// and normals, since an edge could have been built the other way round, and reporting 179 deg instead of 1 deg is an
/// artefact of how the part was assembled rather than information.
fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (u, v) = (norm(a), norm(b));
    let c = dot(u, v).abs().min(1.0);
    c.acos().to_degrees()
}

/// A representative point on an element, for the cases where the shortest distance degenerates.
fn anchor(i: &MeasureItem) -> [f64; 3] {
    match *i {
        MeasureItem::Point(p) => p,
        MeasureItem::Line { origin, .. } => origin,
        MeasureItem::Circle { center, .. } => center,
        MeasureItem::Plane { origin, .. } => origin,
        MeasureItem::Cylinder { origin, .. } => origin,
    }
}

/// The axis or direction of an element, where it has one.
fn direction(i: &MeasureItem) -> Option<[f64; 3]> {
    match *i {
        MeasureItem::Line { dir, .. } => Some(norm(dir)),
        MeasureItem::Circle { axis, .. } | MeasureItem::Cylinder { axis, .. } => Some(norm(axis)),
        MeasureItem::Plane { normal, .. } => Some(norm(normal)),
        MeasureItem::Point(_) => None,
    }
}

/// The distance from a point to a line given by an origin and a direction.
fn point_line(p: [f64; 3], o: [f64; 3], d: [f64; 3]) -> f64 {
    let u = norm(d);
    let w = sub(p, o);
    len(cross(w, u))
}

/// The shortest distance between two lines and the closest points on each line.
fn line_line_points(o1: [f64; 3], d1: [f64; 3], o2: [f64; 3], d2: [f64; 3]) -> (f64, [f64; 3], [f64; 3]) {
    let (u, v) = (norm(d1), norm(d2));
    let w0 = sub(o1, o2);
    let a = dot(u, u);
    let b = dot(u, v);
    let c = dot(v, v);
    let d = dot(u, w0);
    let e = dot(v, w0);
    let denom = a * c - b * b;
    if denom < 1e-9 {
        let t = dot(sub(o1, o2), v);
        let p2 = [o2[0] + v[0] * t, o2[1] + v[1] * t, o2[2] + v[2] * t];
        return (len(sub(p2, o1)), o1, p2);
    }
    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;
    let p1 = [o1[0] + u[0] * s, o1[1] + u[1] * s, o1[2] + u[2] * s];
    let p2 = [o2[0] + v[0] * t, o2[1] + v[1] * t, o2[2] + v[2] * t];
    (len(sub(p2, p1)), p1, p2)
}

/// Measure a single element: the length of an edge, radius, diameter, area or loop perimeter.
pub fn measure_one(a: &MeasureItem) -> MeasureResult {
    let mut r = MeasureResult::default();
    match *a {
        MeasureItem::Point(p) => {
            r.closest_points = Some((p, p));
        }
        MeasureItem::Line { origin, dir, len: l } => {
            r.value = Some(("m3-length", l));
            r.length = Some(l);
            let u = norm(dir);
            let end = [origin[0] + u[0] * l, origin[1] + u[1] * l, origin[2] + u[2] * l];
            r.closest_points = Some((origin, end));
        }
        MeasureItem::Circle { center, r: rad, .. } => {
            r.value = Some(("Ø", 2.0 * rad));
            r.perimeter = Some(2.0 * std::f64::consts::PI * rad);
            r.closest_points = Some((center, center));
        }
        MeasureItem::Cylinder { origin, r: rad, area, length, .. } => {
            r.value = Some(("Ø", 2.0 * rad));
            r.area = area;
            r.length = length;
            r.closest_points = Some((origin, origin));
        }
        MeasureItem::Plane { origin, area, perimeter, .. } => {
            r.area = area;
            r.perimeter = perimeter;
            r.closest_points = Some((origin, origin));
        }
    }
    r
}

/// Measure a pair: a distance, an angle, or both, according to what is meaningful for that pair.
pub fn measure_pair(a: &MeasureItem, b: &MeasureItem) -> MeasureResult {
    use MeasureItem::*;
    let mut out = MeasureResult::default();
    if let (Some(da), Some(db)) = (direction(a), direction(b)) {
        out.angle_deg = Some(angle_between(da, db));
    }
    let parallel = out.angle_deg.is_none_or(|ang| ang < 1e-6);
    match (a, b) {
        (Point(p), Point(q)) => {
            let d = sub(*q, *p);
            out.distance = Some(len(d));
            out.delta = Some(d);
            out.closest_points = Some((*p, *q));
        }
        (Point(p), Plane { origin, normal, .. }) => {
            let n = norm(*normal);
            let dist = dot(sub(*p, *origin), n);
            let proj = [p[0] - n[0] * dist, p[1] - n[1] * dist, p[2] - n[2] * dist];
            out.distance = Some(dist.abs());
            out.delta = Some(sub(proj, *p));
            out.closest_points = Some((*p, proj));
        }
        (Plane { origin, normal, .. }, Point(p)) => {
            let n = norm(*normal);
            let dist = dot(sub(*p, *origin), n);
            let proj = [p[0] - n[0] * dist, p[1] - n[1] * dist, p[2] - n[2] * dist];
            out.distance = Some(dist.abs());
            out.delta = Some(sub(*p, proj));
            out.closest_points = Some((proj, *p));
        }
        (Point(p), Line { origin, dir, .. }) => {
            let u = norm(*dir);
            let t = dot(sub(*p, *origin), u);
            let proj = [origin[0] + u[0] * t, origin[1] + u[1] * t, origin[2] + u[2] * t];
            out.distance = Some(len(sub(*p, proj)));
            out.delta = Some(sub(proj, *p));
            out.closest_points = Some((*p, proj));
        }
        (Line { origin, dir, .. }, Point(p)) => {
            let u = norm(*dir);
            let t = dot(sub(*p, *origin), u);
            let proj = [origin[0] + u[0] * t, origin[1] + u[1] * t, origin[2] + u[2] * t];
            out.distance = Some(len(sub(*p, proj)));
            out.delta = Some(sub(*p, proj));
            out.closest_points = Some((proj, *p));
        }
        (Point(p), Cylinder { origin, axis, r, .. }) => {
            let u = norm(*axis);
            let t = dot(sub(*p, *origin), u);
            let p_ax = [origin[0] + u[0] * t, origin[1] + u[1] * t, origin[2] + u[2] * t];
            let rad_vec = sub(*p, p_ax);
            let rad_len = len(rad_vec);
            let surf_dir = if rad_len > 1e-12 { norm(rad_vec) } else { [1.0, 0.0, 0.0] };
            let p_surf = [p_ax[0] + surf_dir[0] * r, p_ax[1] + surf_dir[1] * r, p_ax[2] + surf_dir[2] * r];
            out.distance = Some((rad_len - r).abs());
            out.delta = Some(sub(p_surf, *p));
            out.closest_points = Some((*p, p_surf));
        }
        (Cylinder { origin, axis, r, .. }, Point(p)) => {
            let u = norm(*axis);
            let t = dot(sub(*p, *origin), u);
            let p_ax = [origin[0] + u[0] * t, origin[1] + u[1] * t, origin[2] + u[2] * t];
            let rad_vec = sub(*p, p_ax);
            let rad_len = len(rad_vec);
            let surf_dir = if rad_len > 1e-12 { norm(rad_vec) } else { [1.0, 0.0, 0.0] };
            let p_surf = [p_ax[0] + surf_dir[0] * r, p_ax[1] + surf_dir[1] * r, p_ax[2] + surf_dir[2] * r];
            out.distance = Some((rad_len - r).abs());
            out.delta = Some(sub(*p, p_surf));
            out.closest_points = Some((p_surf, *p));
        }
        (Point(p), Circle { center, axis, r }) => {
            let w = sub(*p, *center);
            let along = dot(w, norm(*axis));
            let ax_pt = [center[0] + axis[0] * along, center[1] + axis[1] * along, center[2] + axis[2] * along];
            let rad_vec = sub(*p, ax_pt);
            let rad_len = len(rad_vec);
            let surf_dir = if rad_len > 1e-12 { norm(rad_vec) } else { [1.0, 0.0, 0.0] };
            let p_rim = [center[0] + surf_dir[0] * r, center[1] + surf_dir[1] * r, center[2] + surf_dir[2] * r];
            out.distance = Some(len(sub(*p, p_rim)));
            out.delta = Some(sub(p_rim, *p));
            out.closest_points = Some((*p, p_rim));
        }
        (Circle { center, axis, r }, Point(p)) => {
            let w = sub(*p, *center);
            let along = dot(w, norm(*axis));
            let ax_pt = [center[0] + axis[0] * along, center[1] + axis[1] * along, center[2] + axis[2] * along];
            let rad_vec = sub(*p, ax_pt);
            let rad_len = len(rad_vec);
            let surf_dir = if rad_len > 1e-12 { norm(rad_vec) } else { [1.0, 0.0, 0.0] };
            let p_rim = [center[0] + surf_dir[0] * r, center[1] + surf_dir[1] * r, center[2] + surf_dir[2] * r];
            out.distance = Some(len(sub(*p, p_rim)));
            out.delta = Some(sub(*p, p_rim));
            out.closest_points = Some((p_rim, *p));
        }
        (Plane { origin: o1, normal: n1, .. }, Plane { origin: o2, .. }) if parallel => {
            let n = norm(*n1);
            let dist = dot(sub(*o2, *o1), n);
            let p2 = [o1[0] + n[0] * dist, o1[1] + n[1] * dist, o1[2] + n[2] * dist];
            out.distance = Some(dist.abs());
            out.delta = Some(sub(p2, *o1));
            out.closest_points = Some((*o1, p2));
        }
        (Plane { origin, normal, .. }, Line { origin: lo, dir, .. }) => {
            if dot(norm(*normal), norm(*dir)).abs() < 1e-9 {
                let n = norm(*normal);
                let dist = dot(sub(*lo, *origin), n);
                let p_plane = [lo[0] - n[0] * dist, lo[1] - n[1] * dist, lo[2] - n[2] * dist];
                out.distance = Some(dist.abs());
                out.delta = Some(sub(*lo, p_plane));
                out.closest_points = Some((p_plane, *lo));
            }
            out.angle_deg = out.angle_deg.map(|a| (90.0 - a).abs());
        }
        (Line { origin: lo, dir, .. }, Plane { origin, normal, .. }) => {
            if dot(norm(*normal), norm(*dir)).abs() < 1e-9 {
                let n = norm(*normal);
                let dist = dot(sub(*lo, *origin), n);
                let p_plane = [lo[0] - n[0] * dist, lo[1] - n[1] * dist, lo[2] - n[2] * dist];
                out.distance = Some(dist.abs());
                out.delta = Some(sub(p_plane, *lo));
                out.closest_points = Some((*lo, p_plane));
            }
            out.angle_deg = out.angle_deg.map(|a| (90.0 - a).abs());
        }
        (Line { origin: o1, dir: d1, .. }, Line { origin: o2, dir: d2, .. }) => {
            let (dist, p1, p2) = line_line_points(*o1, *d1, *o2, *d2);
            out.distance = Some(dist);
            out.delta = Some(sub(p2, p1));
            out.closest_points = Some((p1, p2));
        }
        (Cylinder { origin: o1, axis: a1, r: r1, .. }, Cylinder { origin: o2, axis: a2, r: r2, .. }) if parallel => {
            let (c2c, p_ax1, p_ax2) = line_line_points(*o1, *a1, *o2, *a2);
            let clearance = c2c - r1 - r2;
            out.center_to_center = Some(c2c);
            out.distance = Some(clearance);
            let perp = sub(p_ax2, p_ax1);
            let n_perp = norm(perp);
            let p1 = [p_ax1[0] + n_perp[0] * r1, p_ax1[1] + n_perp[1] * r1, p_ax1[2] + n_perp[2] * r1];
            let p2 = [p_ax2[0] - n_perp[0] * r2, p_ax2[1] - n_perp[1] * r2, p_ax2[2] - n_perp[2] * r2];
            out.delta = Some(sub(p2, p1));
            out.closest_points = Some((p1, p2));
        }
        (Circle { center: c1, axis: a1, .. }, Circle { center: c2, axis: a2, .. }) if parallel => {
            let d = point_line(*c2, *c1, *a1).hypot(dot(sub(*c2, *c1), norm(*a1))).min(len(sub(*c2, *c1)));
            out.distance = Some(d);
            out.center_to_center = Some(len(sub(*c2, *c1)));
            out.delta = Some(sub(*c2, *c1));
            out.closest_points = Some((*c1, *c2));
            let _ = a2;
        }
        _ => {
            if parallel {
                let pa = anchor(a);
                let pb = anchor(b);
                out.distance = Some(len(sub(pb, pa)));
                out.delta = Some(sub(pb, pa));
                out.closest_points = Some((pa, pb));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_points_give_distance_and_axis_deltas() {
        let r = measure_pair(&MeasureItem::Point([0.0, 0.0, 0.0]), &MeasureItem::Point([3.0, 4.0, 0.0]));
        assert!((r.distance.unwrap() - 5.0).abs() < 1e-9, "3-4-5: {r:?}");
        assert_eq!(r.delta.unwrap(), [3.0, 4.0, 0.0], "the per-axis breakdown is meaningful for points");
    }

    #[test]
    fn point_to_plane_is_the_perpendicular_distance() {
        let pl = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], area: None, perimeter: None };
        let r = measure_pair(&MeasureItem::Point([5.0, -7.0, 12.0]), &pl);
        assert!((r.distance.unwrap() - 12.0).abs() < 1e-9, "12 along the normal, got {:?}", r.distance);
        assert_eq!(r.delta.unwrap(), [0.0, 0.0, -12.0]);
    }

    #[test]
    fn parallel_planes_give_the_gap_between_them() {
        let a = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], area: None, perimeter: None };
        let b = MeasureItem::Plane { origin: [100.0, 50.0, 8.0], normal: [0.0, 0.0, -1.0], area: None, perimeter: None };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 8.0).abs() < 1e-9, "a clearance of 8 regardless of which point of the face is taken: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "opposing normals still describe parallel faces, giving 0 deg rather than 180 deg");
    }

    /// Non-parallel faces have an angle and no distance.
    #[test]
    fn planes_at_an_angle_report_the_angle_and_no_distance() {
        let a = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], area: None, perimeter: None };
        let b = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 1.0, 1.0], area: None, perimeter: None };
        let r = measure_pair(&a, &b);
        assert!((r.angle_deg.unwrap() - 45.0).abs() < 1e-9, "45 deg, got {:?}", r.angle_deg);
        assert!(r.distance.is_none(), "the distance between converging planes is not constant, so there must be no number");
    }

    #[test]
    fn skew_lines_give_the_common_normal_distance() {
        let a = MeasureItem::Line { origin: [0.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], len: 10.0 };
        let b = MeasureItem::Line { origin: [0.0, 0.0, 7.0], dir: [0.0, 1.0, 0.0], len: 10.0 };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 7.0).abs() < 1e-9, "skew lines 7 apart: {r:?}");
        assert!((r.angle_deg.unwrap() - 90.0).abs() < 1e-9, "and at 90 deg");
        assert_eq!(r.delta.unwrap(), [0.0, 0.0, 7.0]);
    }

    #[test]
    fn parallel_lines_give_the_distance_between_them() {
        let a = MeasureItem::Line { origin: [0.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], len: 10.0 };
        let b = MeasureItem::Line { origin: [5.0, 3.0, 4.0], dir: [-2.0, 0.0, 0.0], len: 10.0 };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 5.0).abs() < 1e-9, "3-4-5 across: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "opposite directions still describe parallel edges, giving 0 deg");
    }

    #[test]
    fn point_to_cylinder_measures_to_the_wall() {
        let c = MeasureItem::Cylinder { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 4.0, area: None, length: None };
        let r = measure_pair(&MeasureItem::Point([10.0, 0.0, 3.0]), &c);
        assert!((r.distance.unwrap() - 6.0).abs() < 1e-9, "10 from the axis minus a radius of 4 gives 6, got {:?}", r.distance);
        assert_eq!(r.delta.unwrap(), [-6.0, 0.0, 0.0]);
    }

    #[test]
    fn parallel_cylinders_measure_wall_to_wall_and_center_to_center() {
        let a = MeasureItem::Cylinder { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 3.0, area: None, length: None };
        let b = MeasureItem::Cylinder { origin: [20.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 5.0, area: None, length: None };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 12.0).abs() < 1e-9, "20 between the axes minus 3 and 5 gives 12, got {:?}", r.distance);
        assert!((r.center_to_center.unwrap() - 20.0).abs() < 1e-9, "20 between centers");
        assert_eq!(r.delta.unwrap(), [12.0, 0.0, 0.0]);
    }

    #[test]
    fn a_single_edge_reports_its_length_and_a_hole_its_diameter() {
        let l = measure_one(&MeasureItem::Line { origin: [0.0; 3], dir: [1.0, 0.0, 0.0], len: 17.5 });
        assert_eq!(l.value, Some(("m3-length", 17.5)));
        let c = measure_one(&MeasureItem::Cylinder { origin: [0.0; 3], axis: [0.0, 0.0, 1.0], r: 4.0, area: Some(150.0), length: Some(10.0) });
        assert_eq!(c.value, Some(("Ø", 8.0)), "a hole is reported by its diameter");
        assert_eq!(c.area, Some(150.0));
        assert_eq!(c.length, Some(10.0));
    }

    #[test]
    fn a_planar_face_reports_area_and_perimeter() {
        let f = measure_one(&MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0], area: Some(400.0), perimeter: Some(80.0) });
        assert_eq!(f.area, Some(400.0));
        assert_eq!(f.perimeter, Some(80.0));
    }

    #[test]
    fn a_point_or_plane_without_area_has_no_size() {
        assert_eq!(measure_one(&MeasureItem::Point([1.0, 2.0, 3.0])).value, None);
        assert_eq!(measure_one(&MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0], area: None, perimeter: None }).value, None);
    }

    #[test]
    fn a_line_parallel_to_a_plane_has_a_distance() {
        let pl = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], area: None, perimeter: None };
        let ln = MeasureItem::Line { origin: [3.0, 3.0, 9.0], dir: [1.0, 1.0, 0.0], len: 5.0 };
        let r = measure_pair(&pl, &ln);
        assert!((r.distance.unwrap() - 9.0).abs() < 1e-9, "9 above the plane: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "parallel to the plane means 0 deg, not 90 deg to the normal");
        assert_eq!(r.delta.unwrap(), [0.0, 0.0, 9.0]);
    }
}
