//! Basic geometry, in f64 for CAD accuracy.
//!
//! A contour is a polyline; arcs are tessellated on import. Arc segments (bulge) for an arc-aware offset can be
//! added later if the need arises.

use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn dist(self, o: Point2) -> f64 {
        (self - o).len()
    }
}

// THE OPERATORS THEMSELVES, not methods named after them. `a.sub(b)` looked like a call to
// `std::ops::Sub::sub` and was not one: a reader had to check which of the two was being called, and the
// two could have drifted apart without a word.
impl std::ops::Sub for Point2 {
    type Output = Point2;
    fn sub(self, o: Point2) -> Point2 {
        Point2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Add for Point2 {
    type Output = Point2;
    fn add(self, o: Point2) -> Point2 {
        Point2::new(self.x + o.x, self.y + o.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// A 2D point raised to height `z`.
    pub fn at(p: Point2, z: f64) -> Self {
        Self { x: p.x, y: p.y, z }
    }
}

/// An axis-aligned bounding rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub min: Point2,
    pub max: Point2,
}

impl Bbox {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
}

/// An axis-aligned bounding box in 3D.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bbox3 {
    pub min: Point3,
    pub max: Point3,
}

/// A UNIT DIRECTION IN FOUR BYTES: the octahedral form, 16 bits a coordinate, from 1 up so that 0 is free to mean "no
/// direction". Off by 0.003 deg at most; three signed bytes (1/127 an axis) were off by up to 0.5 deg, which a smooth
/// gradient of light shows. Zero or not finite packs to 0.
pub fn pack_direction(n: [f64; 3]) -> u32 {
    let s = n[0].abs() + n[1].abs() + n[2].abs();
    if !s.is_finite() || s < 1e-12 {
        return 0;
    }
    let (mut x, mut y) = (n[0] / s, n[1] / s);
    if n[2] < 0.0 {
        let (ax, ay) = (x.abs(), y.abs());
        x = (1.0 - ay) * x.signum();
        y = (1.0 - ax) * y.signum();
    }
    let q = |v: f64| 1 + ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65534.0).round() as u32;
    q(x) | (q(y) << 16)
}

/// The direction `pack_direction` packed, unit length; `None` for 0.
pub fn unpack_direction(p: u32) -> Option<[f64; 3]> {
    if p == 0 {
        return None;
    }
    let d = |q: u32| ((q as f64 - 1.0) / 65534.0) * 2.0 - 1.0;
    let (ex, ey) = (d(p & 0xffff), d(p >> 16));
    let z = 1.0 - ex.abs() - ey.abs();
    let (x, y) = if z < 0.0 { ((1.0 - ey.abs()) * ex.signum(), (1.0 - ex.abs()) * ey.signum()) } else { (ex, ey) };
    let len = (x * x + y * y + z * z).sqrt();
    Some([x / len, y / len, z / len])
}

#[cfg(test)]
mod normal_tests {
    use super::*;

    // THE CORNERS OF A SHARED EDGE: the roof of `vertex_normals_shared_edge_averages_bisector` turns by 90 degrees and
    // stays sharp - a corner of each face is lit by its face - while a fold of 10 degrees is smoothed into the bisector.
    /// A DIRECTION PACKED INTO FOUR BYTES COMES BACK within 0.01 deg, over the whole sphere of directions and on every
    /// axis and octant edge; and a real direction never packs to 0, which means "none".
    #[test]
    fn a_packed_direction_comes_back_within_a_hundredth_of_a_degree() {
        let mut worst = 0.0f64;
        let mut check = |d: [f64; 3]| {
            let p = pack_direction(d);
            assert_ne!(p, 0, "the direction {d:?} packed to 0");
            let u = unpack_direction(p).expect("a packed direction unpacks");
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let c = (u[0] * d[0] + u[1] * d[1] + u[2] * d[2]) / len;
            worst = worst.max(c.clamp(-1.0, 1.0).acos().to_degrees());
        };
        for i in 0..200 {
            for j in 0..400 {
                let (t, f) = (std::f64::consts::PI * i as f64 / 199.0, std::f64::consts::TAU * j as f64 / 400.0);
                check([t.sin() * f.cos(), t.sin() * f.sin(), t.cos()]);
            }
        }
        for d in [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [1.0, 1.0, 0.0], [-1.0, 0.0, -1.0]] {
            check(d);
        }
        assert!(worst <= 0.01, "a packed direction comes back {worst:.4} deg off");
        assert_eq!(pack_direction([0.0; 3]), 0, "no direction packs to 0");
        assert_eq!(unpack_direction(0), None);
    }

    #[test]
    fn corner_normals_keep_a_sharp_edge_sharp_and_smooth_a_shallow_fold() {
        let roof = |z: f64| Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0), Point3::new(2.0, 0.0, z), Point3::new(2.0, 1.0, z)],
            tris: vec![[0, 1, 2], [1, 3, 4], [1, 4, 2]],
            ..Default::default()
        };
        // the roof's second face turned straight up: its corners at the shared vertex 1 keep their faces' normals
        let sharp = Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0), Point3::new(1.0, 0.0, 1.0), Point3::new(1.0, 1.0, 1.0)],
            tris: vec![[0, 1, 2], [1, 2, 4], [1, 4, 3]],
            ..Default::default()
        };
        let n = sharp.corner_normals(30.0);
        assert!((n[1][2] - 1.0).abs() < 1e-9, "the flat face's corner at the edge is {:?}, not +Z", n[1]);
        assert!((n[3][0] - 1.0).abs() < 1e-9, "the upright face's corner at the edge is {:?}, not +X", n[3]);
        // a fold of 10 degrees: the corners at the fold are smoothed, the far ones keep their faces
        let fold = roof((10.0f64).to_radians().tan()).corner_normals(30.0);
        assert!(fold[1][0] < -1e-3 && fold[1][2] > 0.99, "the flat face's corner at the fold is not smoothed: {:?}", fold[1]);
        assert!(fold[0][0].abs() < 1e-9, "the far corner of the flat face is smoothed: {:?}", fold[0]);
    }

    // A flat quad of two triangles with shared vertices: the vertex normal equals the face normal, +Z.
    #[test]
    fn vertex_normals_flat_quad_shares_face_normal() {
        let m = Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            tris: vec![[0, 1, 2], [0, 2, 3]],
            ..Default::default()
        };
        for n in m.vertex_normals() {
            assert!((n[0]).abs() < 1e-9 && (n[1]).abs() < 1e-9 && (n[2] - 1.0).abs() < 1e-9, "a flat quad must give +Z, got {n:?}");
        }
    }

    // A roof: two faces of equal area meeting at 90 degrees share an edge (vertices 1 and 2). The shared vertices
    // are averaged into the bisector with equal weights, while the outer ones keep the normal of their own face and
    // stay sharp. A smooth surface is thus smoothed, while a break in the topology — separate vertices — keeps the
    // edge sharp.
    #[test]
    fn vertex_normals_shared_edge_averages_bisector() {
        // Face A lies in the z plane (normal +Z), face B is vertical (normal +X), and they share the edge x = 1.
        let m = Mesh {
            verts: vec![
                Point3::new(0.0, 0.0, 0.0), // 0, on A only.
                Point3::new(1.0, 0.0, 0.0), // 1, shared.
                Point3::new(1.0, 1.0, 0.0), // 2, shared.
                Point3::new(1.0, 0.0, 1.0), // 3, on B only.
                Point3::new(1.0, 1.0, 1.0), // 4, on B only.
            ],
            tris: vec![[0, 1, 2], [1, 2, 4], [1, 4, 3]],
            ..Default::default()
        };
        let n = m.vertex_normals();
        // Vertex 0 belongs to face A only, so it is +Z.
        assert!((n[0][2] - 1.0).abs() < 1e-9, "vertex0 = +Z, got {:?}", n[0]);
        // Vertex 3 belongs to face B only, so it is +X.
        assert!((n[3][0] - 1.0).abs() < 1e-9, "vertex3 = +X, got {:?}", n[3]);
        // Shared vertex 1 mixes +Z and +X: components x and z positive, y near zero, and unit length.
        let v = n[1];
        assert!(v[0] > 0.1 && v[2] > 0.1 && v[1].abs() < 1e-9, "vertex 1 must be averaged, got {v:?}");
        assert!(((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() - 1.0).abs() < 1e-9, "the normal must be unit length");
    }
    /// The section cap from a mesh: a closed body on the cutting plane with no B-rep involved, so that a section
    /// does not leave bodies hollow inside.
    mod section_cap_tests {
        use super::super::{mesh_section_cap, triangulate_with_holes, Mesh, Point2, Point3};

        /// A box [x0..x1] x [y0..y1] x [z0..z1] as 12 triangles; the normals do not matter for a section.
        fn box_mesh(x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> Mesh {
            let v = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
            let verts = vec![v(x0, y0, z0), v(x1, y0, z0), v(x1, y1, z0), v(x0, y1, z0), v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)];
            let tris = vec![
                [0, 2, 1],
                [0, 3, 2], // Bottom.
                [4, 5, 6],
                [4, 6, 7], // Top.
                [0, 1, 5],
                [0, 5, 4], // y0
                [1, 2, 6],
                [1, 6, 5], // x1
                [2, 3, 7],
                [2, 7, 6], // y1
                [3, 0, 4],
                [3, 4, 7], // x0
            ];
            Mesh { verts, tris, ..Default::default() }
        }

        fn cap_area(tris: &[[Point3; 3]], n: [f64; 3]) -> f64 {
            tris.iter()
                .map(|t| {
                    let (a, b, c) = (t[0], t[1], t[2]);
                    let u = [b.x - a.x, b.y - a.y, b.z - a.z];
                    let w = [c.x - a.x, c.y - a.y, c.z - a.z];
                    let cr = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                    ((cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2]).abs()) * 0.5
                })
                .sum()
        }

        /// NaN coordinates in a mesh, which a malformed STL routinely carries, brought the application down on
        /// `partial_cmp(..).unwrap()` while sorting faces. The expected result is an honest one without degenerate
        /// faces and, above all, without a panic.
        #[test]
        fn detect_faces_survives_nan_mesh() {
            let mut m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let good = m.detect_faces(8.0).len();
            assert!(good > 0, "faces must be found on a sound cube");
            // Corrupt one vertex, as a broken import would.
            m.verts[0] = Point3::new(f64::NAN, f64::NAN, f64::NAN);
            let faces = m.detect_faces(8.0);
            assert!(faces.iter().all(|f| f.area.is_finite() && f.normal.iter().all(|v| v.is_finite())), "the result must contain no NaN faces");
            assert!(faces.len() < good + 1, "some faces are dropped and the rest are still computed: {} of {good}", faces.len());
            // A mesh entirely of NaN must not bring anything down either.
            for v in &mut m.verts {
                *v = Point3::new(f64::NAN, 0.0, 0.0);
            }
            let _ = m.detect_faces(8.0);
        }

        #[test]
        fn cap_of_cube_is_full_cross_section() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let tris = mesh_section_cap(&m, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
            assert!(!tris.is_empty(), "the cap must be built");
            let a = cap_area(&tris, [0.0, 0.0, 1.0]);
            assert!((a - 100.0).abs() < 1e-6, "the area of the cap is the 10x10 cross section of the cube: {a}");
            for t in &tris {
                for p in t {
                    assert!((p.z - 5.0).abs() < 1e-9, "the cap must lie exactly in the plane: z={}", p.z);
                }
            }
        }

        #[test]
        fn cap_has_hole_for_hollow_body() {
            // A tube: an outer box of 20x20 and an inner cavity of 10x10, a separate shell within the same mesh.
            let mut m = box_mesh(0.0, 20.0, 0.0, 20.0, 0.0, 20.0);
            let inner = box_mesh(5.0, 15.0, 5.0, 15.0, -1.0, 21.0);
            let off = m.verts.len() as u32;
            m.verts.extend(inner.verts);
            m.tris.extend(inner.tris.into_iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
            let tris = mesh_section_cap(&m, [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]);
            let a = cap_area(&tris, [0.0, 0.0, 1.0]);
            assert!((a - 300.0).abs() < 1e-6, "the cap of a hollow body is 400 - 100: {a}, so the hole is not filled");
        }

        /// The centroid of a concave, C-shaped loop lies outside the loop itself, so classifying nesting by the
        /// centroid gave the wrong answer and parts of the cap were filled incorrectly. Checked on a C-shaped
        /// profile: the area of the cap must be the area of the C itself, not of its convex hull.
        #[test]
        fn cap_of_concave_c_shaped_body() {
            // A C-shaped profile in the XZ plane, extruded along Y: 30x30 overall with a 20x10 notch on the right.
            //  ┌──────┐
            //  │  ┌───┘   (the notch opens to the right)
            //  │  └───┐
            //  └──────┘
            let prof: [(f64, f64); 8] = [(0.0, 0.0), (30.0, 0.0), (30.0, 10.0), (10.0, 10.0), (10.0, 20.0), (30.0, 20.0), (30.0, 30.0), (0.0, 30.0)];
            let mut m = Mesh::default();
            // Two caps (y = 0 and y = 10) plus the side walls; only the walls matter for a section, but the body
            // is assembled in full.
            let n = prof.len();
            for y in [0.0, 10.0] {
                for p in prof {
                    m.verts.push(Point3::new(p.0, y, p.1));
                }
            }
            for i in 0..n {
                let j = (i + 1) % n;
                let (a0, b0, a1, b1) = (i as u32, j as u32, (i + n) as u32, (j + n) as u32);
                m.tris.push([a0, b0, b1]);
                m.tris.push([a0, b1, a1]);
            }
            // A section by the plane y = 5, across the extrusion, so the cross section is the C profile itself.
            let tris = mesh_section_cap(&m, [0.0, 5.0, 0.0], [0.0, 1.0, 0.0]);
            let area = cap_area(&tris, [0.0, 1.0, 0.0]);
            let exp = 30.0 * 30.0 - 20.0 * 10.0; // 900 - 200 = 700, with the notch left unfilled.
            assert!((area - exp).abs() < 1e-6, "the cap of a concave profile: {area}, expected {exp}");
        }

        #[test]
        fn plane_beside_body_gives_no_cap() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            assert!(mesh_section_cap(&m, [0.0, 0.0, 50.0], [0.0, 0.0, 1.0]).is_empty(), "a plane clear of the body gives no cap");
        }

        #[test]
        fn cap_works_on_tilted_plane() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let n = [0.0, 1.0f64 / 2f64.sqrt(), 1.0 / 2f64.sqrt()]; // At 45 degrees to Z.
            let tris = mesh_section_cap(&m, [5.0, 5.0, 5.0], n);
            let a = cap_area(&tris, n);
            let exp = 10.0 * 10.0 * 2f64.sqrt(); // A rectangle of 10 by 10 * sqrt(2).
            assert!((a - exp).abs() / exp < 1e-6, "a tilted section: {a}, expected {exp}");
        }

        #[test]
        fn triangulation_of_square_with_square_hole() {
            let outer = vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)];
            let hole = vec![Point2::new(4.0, 4.0), Point2::new(6.0, 4.0), Point2::new(6.0, 6.0), Point2::new(4.0, 6.0)];
            let tris = triangulate_with_holes(&outer, &[hole]);
            let area: f64 = tris.iter().map(|t| ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x)).abs() * 0.5).sum();
            assert!((area - 96.0).abs() < 1e-6, "the triangulated area is 100 - 4: {area}");
        }

        #[test]
        fn degenerate_input_is_empty_not_garbage() {
            assert!(triangulate_with_holes(&[Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)], &[]).is_empty(), "two points are not a polygon");
            assert!(mesh_section_cap(&Mesh::default(), [0.0; 3], [0.0, 0.0, 1.0]).is_empty(), "an empty mesh");
            assert!(mesh_section_cap(&box_mesh(0.0, 1.0, 0.0, 1.0, 0.0, 1.0), [0.0; 3], [0.0; 3]).is_empty(), "a zero normal");
        }
    }
}

mod contour;
pub use contour::*;
mod mesh;
pub use mesh::*;
mod profile;
pub use profile::*;
mod cylinders;
pub use cylinders::*;
mod caps;
pub use caps::*;
