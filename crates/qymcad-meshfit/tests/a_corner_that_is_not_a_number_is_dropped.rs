//! A CORNER THAT IS NOT A NUMBER IS DROPPED with the triangles round it, before anything downstream sees it.
//!
//! An ASCII STL may write a coordinate as `NaN`, and it parses. Kept, the triangles round that corner were flat and
//! closed, so the preparation kept them as slivers no surface could hold: the regions read the owner of an unplaced
//! sliver past their end ("index out of bounds: the len is 6 but the index is 4294967295"). With that read guarded
//! instead, the corner reached the kernel: `BRepBuilderAPI_MakeFace` was still classifying the face after 10 minutes,
//! past a stop asked at 20 s (the stop is not asked inside OCCT), and a stack taken at 45 s stood in
//! `BRepLib_MakeFace::CheckInside`.
//!
//! Measured through the window, without the drop: an ASCII STL with one coordinate written `NaN` came in without a
//! word; the recognition tool's preview stopped counting, Enter said "The operation was interrupted", and every other
//! body of the document lost its live B-rep with the rebuild that failed.

use qymcad_core::geom::Mesh;
use qymcad_meshfit::{prepare, regions, weld_tolerance, Region, Surface, Tolerance};

/// A cube of `side` drawn with a `grid` x `grid` net on every face; the nets share no corner.
fn gridded_cube(grid: usize, side: f64) -> Mesh {
    use qymcad_core::geom::Point3;
    let (mut verts, mut tris) = (Vec::new(), Vec::new());
    for a in 0..3 {
        for high in [false, true] {
            let (u, v) = ((a + 1) % 3, (a + 2) % 3);
            let base = verts.len() as u32;
            for i in 0..=grid {
                for j in 0..=grid {
                    let mut q = [0.0; 3];
                    q[a] = if high { side } else { 0.0 };
                    q[u] = side * i as f64 / grid as f64;
                    q[v] = side * j as f64 / grid as f64;
                    verts.push(Point3::new(q[0], q[1], q[2]));
                }
            }
            let at = |i: usize, j: usize| base + (i * (grid + 1) + j) as u32;
            for i in 0..grid {
                for j in 0..grid {
                    tris.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                    tris.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
                }
            }
        }
    }
    Mesh { verts, tris }
}

/// The corner `i`, `j` of the top net of `gridded_cube(grid, _)`: the top is the sixth net.
fn top(grid: usize, i: usize, j: usize) -> usize {
    5 * (grid + 1) * (grid + 1) + i * (grid + 1) + j
}

/// Every triangle of the prepared mesh lies in exactly one region; what is wrong, if anything.
fn each_triangle_once(n: usize, found: &[Region]) -> Vec<String> {
    let mut seen = vec![0usize; n];
    for r in found {
        for &t in &r.tris {
            seen[t as usize] += 1;
        }
    }
    seen.iter().enumerate().filter(|&(_, &c)| c != 1).map(|(t, c)| format!("triangle {t} in {c} regions")).collect()
}

/// The kept triangles that still have a corner that is not a finite number.
fn not_finite(p: &qymcad_meshfit::Prepared) -> usize {
    p.mesh
        .tris
        .iter()
        .filter(|t| {
            t.iter().any(|&i| {
                let q = p.mesh.verts[i as usize];
                !(q.x.is_finite() && q.y.is_finite() && q.z.is_finite())
            })
        })
        .count()
}

fn planes(found: &[Region]) -> usize {
    found.iter().filter(|r| matches!(r.surface, Some(Surface::Plane { .. }))).count()
}

/// A NaN CORNER INSIDE A FACE: the six triangles round it go; the top is a plane with a hole where they were. Kept,
/// they were slivers beside that plane, and the absorbing read past the end of the regions.
#[test]
fn a_nan_corner_inside_a_face_goes_with_its_triangles() {
    let mut mesh = gridded_cube(8, 20.0);
    mesh.verts[top(8, 3, 3)].x = f64::NAN; // at (7.5, 7.5, 20)
    let p = prepare(&mesh, weld_tolerance(&mesh));
    assert_eq!((p.dropped, not_finite(&p)), (6, 0), "the six triangles round the NaN corner are not dropped; slivers {:?}", p.slivers);
    let found = regions(&p, &Tolerance::for_mesh(&p));
    let wrong = each_triangle_once(p.mesh.tris.len(), &found);
    assert!(wrong.is_empty(), "the regions do not cover the mesh once: {wrong:?}");
    assert_eq!(planes(&found), 6, "the six faces of the cube are not six planes");
}

/// A NaN CORNER ON A SPHERE: the triangles round it go. Kept, they were slivers beside a curved region, and the
/// merging read past the end of the regions.
#[test]
fn a_nan_corner_on_a_sphere_goes_with_its_triangles() {
    let ball = qymcad_kernel::Shape::sphere(10.0).expect("a sphere");
    let qymcad_core::geom::Built { mut mesh, .. } = ball.tessellate(0.1).into_iter().next().expect("a mesh");
    // a corner off the seams and the poles: one whose place no other corner of the mesh repeats
    let once = |k: usize| mesh.verts.iter().filter(|q| q.x == mesh.verts[k].x && q.y == mesh.verts[k].y && q.z == mesh.verts[k].z).count() == 1;
    let k = (0..mesh.verts.len()).find(|&k| mesh.verts[k].z.abs() < 5.0 && once(k)).expect("a corner of the band");
    mesh.verts[k].x = f64::NAN;
    let p = prepare(&mesh, weld_tolerance(&mesh));
    assert!(p.dropped > 0 && not_finite(&p) == 0, "the triangles round the NaN corner are not dropped: dropped {}, slivers {:?}", p.dropped, p.slivers);
    let found = regions(&p, &Tolerance::for_mesh(&p));
    let wrong = each_triangle_once(p.mesh.tris.len(), &found);
    assert!(wrong.is_empty(), "the regions do not cover the mesh once: {wrong:?}");
    assert!(found.iter().any(|r| matches!(r.surface, Some(Surface::Sphere { .. }))), "the sphere is not found");
}

/// A closed cylinder of radius `r` and height `h` about Z: its wall `around` x `up` quads, its caps fans about their
/// centres. Every corner is shared; the corner `around * (up / 2)` stands inside the wall.
fn gridded_cylinder(r: f64, h: f64, around: usize, up: usize) -> Mesh {
    use qymcad_core::geom::Point3;
    let mut verts = Vec::new();
    for k in 0..=up {
        for i in 0..around {
            let a = std::f64::consts::TAU * i as f64 / around as f64;
            verts.push(Point3::new(r * a.cos(), r * a.sin(), h * k as f64 / up as f64));
        }
    }
    let at = |k: usize, i: usize| (k * around + i % around) as u32;
    let (bottom, top) = (verts.len() as u32, verts.len() as u32 + 1);
    verts.push(Point3::new(0.0, 0.0, 0.0));
    verts.push(Point3::new(0.0, 0.0, h));
    let mut tris = Vec::new();
    for k in 0..up {
        for i in 0..around {
            tris.push([at(k, i), at(k, i + 1), at(k + 1, i + 1)]);
            tris.push([at(k, i), at(k + 1, i + 1), at(k + 1, i)]);
        }
    }
    for i in 0..around {
        tris.push([bottom, at(0, i + 1), at(0, i)]);
        tris.push([top, at(up, i), at(up, i + 1)]);
    }
    Mesh { verts, tris }
}

/// A NaN CORNER ON THE WALL OF A CYLINDER: the six triangles round it go. Kept, they were slivers beside a region turned
/// about an axis, and the merging read past the end of the regions - and the search about the axis after it.
#[test]
fn a_nan_corner_on_a_cylinder_goes_with_its_triangles() {
    let (around, up) = (48, 8);
    let mut mesh = gridded_cylinder(5.0, 10.0, around, up);
    mesh.verts[around * (up / 2)].x = f64::NAN; // at (5, 0, 5)
    let p = prepare(&mesh, weld_tolerance(&mesh));
    assert_eq!((p.dropped, not_finite(&p)), (6, 0), "the six triangles round the NaN corner are not dropped; slivers {:?}", p.slivers);
    let found = regions(&p, &Tolerance::for_mesh(&p));
    let wrong = each_triangle_once(p.mesh.tris.len(), &found);
    assert!(wrong.is_empty(), "the regions do not cover the mesh once: {wrong:?}");
    assert!(found.iter().any(|r| matches!(r.surface, Some(Surface::Cylinder { .. }))), "the cylinder is not found");
}
