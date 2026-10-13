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
    /// A planar face. `size` is filled when the face record is at hand and stays empty when it is not:
    /// no record means no number rather than a zero.
    Plane { origin: [f64; 3], normal: [f64; 3], size: Option<FaceSize> },
    /// A cylindrical face: the wall of a hole or of a shaft. `size` likewise.
    Cylinder { origin: [f64; 3], axis: [f64; 3], r: f64, size: Option<CylSize> },
}

/// The measured size of a planar face: its area and the length of its boundary. One struct rather than
/// two `f64` neighbours, which swap and still compile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceSize {
    /// In mm^2.
    pub area: f64,
    /// In mm.
    pub perimeter: f64,
}

/// The measured size of a cylindrical face: its length along the axis and its side area. One struct for
/// the same reason as [`FaceSize`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CylSize {
    /// Along the axis, in mm: the depth of a hole, the length of a shaft.
    pub len: f64,
    /// The side area in mm^2.
    pub area: f64,
}

/// The result of a measurement. There are several fields because different pairs make different things
/// meaningful: two points have a distance and its projections onto the axes, two planes have either a distance,
/// when parallel, or an angle. An empty field means the quantity is meaningless for this pair, not that it is
/// zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasureResult {
    /// The shortest distance, in mm.
    pub distance: Option<f64>,
    /// The angle between directions or normals, in degrees.
    pub angle_deg: Option<f64>,
    /// The distance broken down by axis. For a pair of points, and for two parallel cylinders where it
    /// is the offset of the second axis point from the first (the pitch components of two holes).
    /// For any other pair it misleads.
    pub delta: Option<[f64; 3]>,
    /// Sizes of a single selected element, each with the catalogue label it is shown under (`m3-length`,
    /// `m3-area`, `m3-perimeter`, or the sign `Ø`). A list rather than one slot: a face answers both its
    /// area and its perimeter.
    pub values: Vec<(&'static str, f64)>,
    /// Centre-to-centre distance of two parallel cylinders (holes): the dimension a drawing gives, beside
    /// the wall-to-wall clearance in `distance`. `None` for any other pair.
    pub centre_distance: Option<f64>,
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
/// and normals, since an edge could have been built the other way round, and reporting 179° instead of 1° is an
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

/// The shortest distance between two lines: the distance between them when parallel, and along the common
/// normal when skew.
fn line_line(o1: [f64; 3], d1: [f64; 3], o2: [f64; 3], d2: [f64; 3]) -> f64 {
    let (u, v) = (norm(d1), norm(d2));
    let n = cross(u, v);
    if len(n) < 1e-9 {
        return point_line(o2, o1, u); // parallel
    }
    (dot(sub(o2, o1), norm(n))).abs()
}

/// Measure a single element: the length of an edge, the diameter of a circle or a cylinder with its
/// length and side area beside it, the area and perimeter of a planar face.
///
/// A planar face and a vertex with no size record have no size of their own, so the result for them is
/// empty. That is more honest than showing zero, which reads as a measurement that came out at zero.
pub fn measure_one(a: &MeasureItem) -> MeasureResult {
    let mut r = MeasureResult::default();
    match *a {
        MeasureItem::Line { len: l, .. } => r.values.push(("m3-length", l)),
        MeasureItem::Circle { r: rad, .. } => r.values.push(("Ø", 2.0 * rad)),
        MeasureItem::Cylinder { r: rad, size, .. } => {
            r.values.push(("Ø", 2.0 * rad));
            if let Some(s) = size {
                r.values.push(("m3-length", s.len));
                r.values.push(("m3-area", s.area));
            }
        }
        MeasureItem::Plane { size: Some(s), .. } => {
            r.values.push(("m3-area", s.area));
            r.values.push(("m3-perimeter", s.perimeter));
        }
        _ => {}
    }
    r
}

/// The area of mesh triangles `tris` (indexes into `mesh`), in mm^2. The side area of a cylindrical face
/// and the area of a planar one alike: both are sums over the face's own triangles.
pub fn tris_area(mesh: &crate::geom::Mesh, tris: &[u32]) -> f64 {
    tris.iter()
        .map(|&t| {
            let p = mesh.triangle(t as usize);
            let (ux, uy, uz) = (p[1].x - p[0].x, p[1].y - p[0].y, p[1].z - p[0].z);
            let (vx, vy, vz) = (p[2].x - p[0].x, p[2].y - p[0].y, p[2].z - p[0].z);
            let (cx, cy, cz) = (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx);
            0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
        })
        .sum()
}

/// The length of the boundary of mesh triangles `tris`: every mesh edge used by exactly one of them.
/// An interior edge is shared by two triangles and cancels out, leaving the outer loops and the holes.
pub fn boundary_len(mesh: &crate::geom::Mesh, tris: &[u32]) -> f64 {
    use std::collections::HashMap;
    let key = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let mut uses: HashMap<(u32, u32), usize> = HashMap::new();
    for &t in tris {
        let tri = mesh.tris[t as usize];
        for e in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            *uses.entry(key(e.0, e.1)).or_default() += 1;
        }
    }
    let at = |i: u32| mesh.verts[i as usize];
    uses.into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|((a, b), _)| {
            let (p, q) = (at(a), at(b));
            ((q.x - p.x).powi(2) + (q.y - p.y).powi(2) + (q.z - p.z).powi(2)).sqrt()
        })
        .sum()
}

/// The span of mesh triangles `tris` along `axis`: the length of a cylindrical face, the depth of a
/// hole. The axis need not be a unit vector.
pub fn span_along(mesh: &crate::geom::Mesh, tris: &[u32], axis: [f64; 3]) -> f64 {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if l < 1e-12 {
        return 0.0;
    }
    let (ux, uy, uz) = (axis[0] / l, axis[1] / l, axis[2] / l);
    let mut seen = false;
    let (mut lo, mut hi) = (0.0, 0.0);
    for &t in tris {
        for v in mesh.triangle(t as usize) {
            let s = v.x * ux + v.y * uy + v.z * uz;
            if !seen {
                (lo, hi, seen) = (s, s, true);
            } else {
                lo = lo.min(s);
                hi = hi.max(s);
            }
        }
    }
    hi - lo
}

/// Measure a pair: a distance, an angle, or both, according to what is meaningful for that pair.
///
/// One rule holds throughout: when the elements are parallel, or one of them is a point, the distance is
/// measured; when they converge at an angle, the distance between them is not constant, and the angle is shown
/// instead of a meaningless number. Reporting a distance between non-parallel planes would be a lie, since it
/// depends on where exactly it is measured.
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
            out.delta = Some(d); // the per-axis breakdown is meaningful only for two points
        }
        // A point and a plane: the distance along the normal. The sign is not shown, since which side it
        // falls on depends on where the face normal points, and that is a detail of the topology rather than
        // part of the measurement.
        (Point(p), Plane { origin, normal, .. }) | (Plane { origin, normal, .. }, Point(p)) => {
            out.distance = Some(dot(sub(*p, *origin), norm(*normal)).abs());
        }
        (Point(p), Line { origin, dir, .. }) | (Line { origin, dir, .. }, Point(p)) => {
            out.distance = Some(point_line(*p, *origin, *dir));
        }
        // A point and a cylinder or circle: measured to the surface rather than to the axis. Pointing at the
        // wall of a hole means asking for the clearance to that wall, not the distance to an imaginary axis
        // buried in the material.
        (Point(p), Cylinder { origin, axis, r, .. }) | (Cylinder { origin, axis, r, .. }, Point(p)) => {
            out.distance = Some((point_line(*p, *origin, *axis) - r).abs());
        }
        (Point(p), Circle { center, axis, r }) | (Circle { center, axis, r }, Point(p)) => {
            // the distance to the circle itself rather than to its centre: along the axis plus within the
            // plane
            let w = sub(*p, *center);
            let along = dot(w, norm(*axis));
            let radial = (len(w).powi(2) - along * along).max(0.0).sqrt() - r;
            out.distance = Some((along * along + radial * radial).sqrt());
        }
        (Plane { origin: o1, normal: n1, .. }, Plane { origin: o2, .. }) if parallel => {
            out.distance = Some(dot(sub(*o2, *o1), norm(*n1)).abs());
        }
        (Plane { origin, normal, .. }, Line { origin: lo, dir, .. }) | (Line { origin: lo, dir, .. }, Plane { origin, normal, .. }) => {
            // the line is parallel to the plane, its normal being perpendicular to the direction, so a
            // distance applies; otherwise the line crosses the plane and the distance is zero at the
            // intersection, so the angle is measured instead
            if dot(norm(*normal), norm(*dir)).abs() < 1e-9 {
                out.distance = Some(dot(sub(*lo, *origin), norm(*normal)).abs());
            }
            // the angle between a line and a plane is 90° minus the angle to the normal
            out.angle_deg = out.angle_deg.map(|a| (90.0 - a).abs());
        }
        (Line { origin: o1, dir: d1, .. }, Line { origin: o2, dir: d2, .. }) => {
            out.distance = Some(line_line(*o1, *d1, *o2, *d2));
        }
        (Cylinder { origin: o1, axis: a1, r: r1, .. }, Cylinder { origin: o2, axis: a2, r: r2, .. }) if parallel => {
            // between walls: the distance between the axes minus both radii; a negative value means overlap.
            // The axes' distance is kept beside it: two holes 50 apart read as a centre distance of 50, which
            // is the dimension a drawing gives, not just the 40 of wall clearance. The axis-point offset goes
            // into the per-axis breakdown: the pitch components of the holes along X, Y and Z.
            let between_axes = line_line(*o1, *a1, *o2, *a2);
            out.distance = Some(between_axes - r1 - r2);
            out.centre_distance = Some(between_axes);
            out.delta = Some(sub(*o2, *o1));
        }
        (Circle { center: c1, axis: a1, .. }, Circle { center: c2, axis: a2, .. }) if parallel => {
            out.distance = Some(point_line(*c2, *c1, *a1).hypot(dot(sub(*c2, *c1), norm(*a1))).min(len(sub(*c2, *c1))));
            let _ = a2;
        }
        // Every other pair: the distance between representative points is not computed, since it depends on
        // which points are taken and would be a number about nothing. The angle, where there is one, is
        // already computed above.
        _ => {
            if parallel {
                out.distance = Some(len(sub(anchor(b), anchor(a))));
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
        let pl = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], size: None };
        let r = measure_pair(&MeasureItem::Point([5.0, -7.0, 12.0]), &pl);
        assert!((r.distance.unwrap() - 12.0).abs() < 1e-9, "12 along the normal, got {:?}", r.distance);
    }

    #[test]
    fn parallel_planes_give_the_gap_between_them() {
        let a = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], size: None };
        let b = MeasureItem::Plane { origin: [100.0, 50.0, 8.0], normal: [0.0, 0.0, -1.0], size: None };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 8.0).abs() < 1e-9, "a clearance of 8 regardless of which point of the face is taken: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "opposing normals still describe parallel faces, giving 0° rather than 180°");
    }

    /// Non-parallel faces have an angle and no distance.
    ///
    /// The distance between converging planes depends on where it is measured, so showing a single number
    /// would be a lie.
    #[test]
    fn planes_at_an_angle_report_the_angle_and_no_distance() {
        let a = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], size: None };
        let b = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 1.0, 1.0], size: None };
        let r = measure_pair(&a, &b);
        assert!((r.angle_deg.unwrap() - 45.0).abs() < 1e-9, "45°, got {:?}", r.angle_deg);
        assert!(r.distance.is_none(), "the distance between converging planes is not constant, so there must be no number");
    }

    #[test]
    fn skew_lines_give_the_common_normal_distance() {
        let a = MeasureItem::Line { origin: [0.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], len: 10.0 };
        let b = MeasureItem::Line { origin: [0.0, 0.0, 7.0], dir: [0.0, 1.0, 0.0], len: 10.0 };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 7.0).abs() < 1e-9, "skew lines 7 apart: {r:?}");
        assert!((r.angle_deg.unwrap() - 90.0).abs() < 1e-9, "and at 90°");
    }

    #[test]
    fn parallel_lines_give_the_distance_between_them() {
        let a = MeasureItem::Line { origin: [0.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], len: 10.0 };
        let b = MeasureItem::Line { origin: [5.0, 3.0, 4.0], dir: [-2.0, 0.0, 0.0], len: 10.0 };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 5.0).abs() < 1e-9, "3-4-5 across: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "opposite directions still describe parallel edges, giving 0°");
    }

    /// A point and a cylinder: measured to the wall rather than to the axis. Pointing at a hole means asking
    /// for the clearance to it.
    #[test]
    fn point_to_cylinder_measures_to_the_wall() {
        let c = MeasureItem::Cylinder { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 4.0, size: None };
        let r = measure_pair(&MeasureItem::Point([10.0, 0.0, 3.0]), &c);
        assert!((r.distance.unwrap() - 6.0).abs() < 1e-9, "10 from the axis minus a radius of 4 gives 6, got {:?}", r.distance);
    }

    #[test]
    fn parallel_cylinders_measure_wall_to_wall() {
        let a = MeasureItem::Cylinder { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 3.0, size: None };
        let b = MeasureItem::Cylinder { origin: [20.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 5.0, size: None };
        let r = measure_pair(&a, &b);
        assert!((r.distance.unwrap() - 12.0).abs() < 1e-9, "20 between the axes minus 3 and 5 gives 12, got {:?}", r.distance);
    }

    /// Two holes 50 apart read as a centre distance of 50 beside the wall clearance: the dimension a
    /// drawing gives (#45). A 50 pitch of r=5 holes clears 40 of wall but dimensions 50 centre to centre.
    #[test]
    fn parallel_cylinders_report_the_centre_distance_beside_the_clearance() {
        let a = MeasureItem::Cylinder { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 5.0, size: None };
        let b = MeasureItem::Cylinder { origin: [50.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], r: 5.0, size: None };
        let r = measure_pair(&a, &b);
        assert!((r.centre_distance.unwrap() - 50.0).abs() < 1e-9, "centre to centre is 50: {r:?}");
        assert!((r.distance.unwrap() - 40.0).abs() < 1e-9, "the wall clearance beside it is 40: {r:?}");
        assert_eq!(r.delta.unwrap(), [50.0, 0.0, 0.0], "the pitch components along X, Y and Z: {r:?}");
    }

    #[test]
    fn a_single_edge_reports_its_length_and_a_hole_its_diameter() {
        let l = measure_one(&MeasureItem::Line { origin: [0.0; 3], dir: [1.0, 0.0, 0.0], len: 17.5 });
        assert_eq!(l.values, [("m3-length", 17.5)]);
        let c = measure_one(&MeasureItem::Cylinder { origin: [0.0; 3], axis: [0.0, 0.0, 1.0], r: 4.0, size: None });
        assert_eq!(c.values, [("Ø", 8.0)], "a hole is reported by its diameter, which is what it is measured by");
    }

    /// A vertex and a size-less plane have no size of their own: the result is empty rather than zero.
    #[test]
    fn a_point_or_sizeless_plane_alone_has_no_size() {
        assert!(measure_one(&MeasureItem::Point([1.0, 2.0, 3.0])).values.is_empty());
        assert!(measure_one(&MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0], size: None }).values.is_empty());
    }

    /// A planar face of 40 x 30 reports an area of 1200 mm^2 and a perimeter of 140 mm (#45).
    #[test]
    fn a_face_reports_its_area_and_perimeter() {
        let f = MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0], size: Some(FaceSize { area: 1200.0, perimeter: 140.0 }) };
        assert_eq!(measure_one(&f).values, [("m3-area", 1200.0), ("m3-perimeter", 140.0)]);
    }

    /// A hole of diameter 10 and depth 20 reports diameter 10, length 20 and a side area of 628.32 mm^2 (#45).
    #[test]
    fn a_hole_reports_diameter_depth_and_side_area() {
        let h = MeasureItem::Cylinder { origin: [0.0; 3], axis: [0.0, 0.0, 1.0], r: 5.0, size: Some(CylSize { len: 20.0, area: 628.3185 }) };
        let got = measure_one(&h).values;
        assert_eq!(got.len(), 3, "diameter, depth and side area: {got:?}");
        assert_eq!(got[0], ("Ø", 10.0));
        assert_eq!(got[1], ("m3-length", 20.0));
        assert!((got[2].1 - 628.32).abs() < 0.01, "2·pi·5·20 of wall: {got:?}");
    }

    /// The boundary of two triangles forming a 10 x 10 square is its perimeter of 40: shared edges cancel.
    #[test]
    fn a_square_of_two_triangles_has_a_perimeter_of_40() {
        use crate::geom::{Mesh, Point3};
        let mesh = Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 0.0, 0.0), Point3::new(10.0, 10.0, 0.0), Point3::new(0.0, 10.0, 0.0)],
            tris: vec![[0, 1, 2], [0, 2, 3]],
            ..Default::default()
        };
        assert!((boundary_len(&mesh, &[0, 1]) - 40.0).abs() < 1e-9, "the outer loop, not the diagonal");
        assert!((tris_area(&mesh, &[0, 1]) - 100.0).abs() < 1e-9, "both halves");
        assert!((span_along(&mesh, &[0, 1], [0.0, 0.0, 2.0]) - 0.0).abs() < 1e-9, "flat in Z, whatever the axis length");
        assert!((span_along(&mesh, &[0, 1], [1.0, 0.0, 0.0]) - 10.0).abs() < 1e-9, "10 along X");
    }

    /// A line parallel to a plane has a distance and an angle of 0°.
    #[test]
    fn a_line_parallel_to_a_plane_has_a_distance() {
        let pl = MeasureItem::Plane { origin: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], size: None };
        let ln = MeasureItem::Line { origin: [3.0, 3.0, 9.0], dir: [1.0, 1.0, 0.0], len: 5.0 };
        let r = measure_pair(&pl, &ln);
        assert!((r.distance.unwrap() - 9.0).abs() < 1e-9, "9 above the plane: {r:?}");
        assert!(r.angle_deg.unwrap() < 1e-9, "parallel to the plane means 0°, not 90° to the normal");
    }
}
