//! A TRIANGLE MESH BECOMES A BODY - one that can be cut, drilled and sketched on, as an imported STEP part can.
use qymcad_core::geom::{Mesh, Point3};
use crate::Shape;

fn cube(open_top: bool) -> Mesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
    let verts = vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0), v(10.0, 0.0, 10.0), v(10.0, 10.0, 10.0), v(0.0, 10.0, 10.0)];
    let mut tris = vec![[0, 2, 1], [0, 3, 2], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
    if !open_top {
        tris.extend([[4, 5, 6], [4, 6, 7]]);
    }
    Mesh { verts, tris, ..Default::default() }
}

/// A CLOSED MESH IS A SOLID: twelve triangles make six faces - the two of each side merged - and the volume.
#[test]
fn a_closed_mesh_is_a_solid_with_its_sides_merged() {
    let s = Shape::from_mesh(&cube(false)).expect("the cube becomes a body");
    assert_eq!(s.solid_count(), 1, "a closed mesh must give one solid");
    assert!(s.is_valid(), "the body is broken");
    assert!((s.volume() - 1000.0).abs() < 1e-6, "the volume came out as {}", s.volume());
    let faces = s.edges_info().len();
    assert_eq!(faces, 12, "a cube's twelve edges - the diagonals of its sides must be gone once each side is one face: {faces}");
}

/// AN OPEN MESH IS A SURFACE, not a solid and not refused.
#[test]
fn an_open_mesh_is_a_surface() {
    let s = Shape::from_mesh(&cube(true)).expect("the open box becomes a body");
    assert_eq!(s.solid_count(), 0, "an open box cannot be a solid");
    assert!(s.bbox().is_some());
}

/// A TRIANGLE WITH NO AREA does not spoil the rest.
#[test]
fn a_triangle_with_no_area_does_not_spoil_the_rest() {
    let mut m = cube(false);
    m.verts.push(Point3::new(5.0, 5.0, 5.0));
    m.tris.push([0, 0, 1]); // two corners meet
    m.tris.push([0, 1, 8]); // off the surface, and a spike: a face with an area, so it is kept - and the solid is then not closed
    m.tris.pop();
    let s = Shape::from_mesh(&m).expect("becomes a body");
    assert_eq!(s.solid_count(), 1, "a zero-area triangle broke the solid");
    assert!(Shape::from_mesh(&Mesh { verts: vec![Point3::new(0.0, 0.0, 0.0); 3], tris: vec![[0, 1, 2]], ..Default::default() }).is_none(), "a mesh of nothing became a body");
}

/// HOW LONG A MESH TAKES TO BECOME A POLYHEDRON: a sphere of our kernel tessellated finer and finer, each mesh turned
/// into a body - its time, and its volume against the mesh's own. Ignored: a measure, not a check.
#[test]
#[ignore = "a measure"]
fn a_big_mesh_becomes_a_body_in_time() {
    let sphere = Shape::sphere(20.0).expect("a sphere");
    for defl in [0.05, 0.005, 0.001] {
        let qymcad_core::geom::Built { mesh, .. } = sphere.tessellate(defl).into_iter().next().expect("a mesh");
        let held: f64 = mesh
            .tris
            .iter()
            .map(|t| {
                let (a, b, c) = (mesh.verts[t[0] as usize], mesh.verts[t[1] as usize], mesh.verts[t[2] as usize]);
                (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
            })
            .sum();
        let t = std::time::Instant::now();
        let body = Shape::from_mesh(&mesh);
        let took = t.elapsed().as_secs_f64();
        let (v, solids, valid) = body.as_ref().map_or((0.0, 0, false), |b| (b.volume(), b.solid_count(), b.is_valid()));
        println!("MESH TO BODY: {} triangles, {} corners - {took:.2} s, volume {v:.3} against the mesh's {held:.3}, solids {solids}, valid {valid}", mesh.tris.len(), mesh.verts.len());
    }
}

/// The faces of `s`, as its tessellation counts them.
fn faces(s: &Shape) -> usize {
    s.tessellate(0.1).into_iter().next().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0)
}

/// A PLATE WITH A SQUARE HOLE THROUGH IT comes back the same plate: its top and its bottom each one face with the hole in
/// it - a border with a loop inside - and four sides out, four in; valid, and holding the volume.
#[test]
fn a_plate_with_a_hole_keeps_its_faces() {
    let plate = Shape::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 40.0, 0.0, 40.0], 5.0).expect("a plate");
    let hole = Shape::extrude(&[10.0, 10.0, 30.0, 10.0, 30.0, 30.0, 10.0, 30.0], 5.0).expect("a hole");
    let holed = plate.boolean(&hole, 0).expect("the plate with its hole");
    let qymcad_core::geom::Built { mesh, .. } = holed.tessellate(0.1).into_iter().next().expect("a mesh");
    let s = Shape::from_mesh(&mesh).expect("the mesh becomes a body");
    assert!(s.is_valid(), "the body is broken");
    assert_eq!(s.solid_count(), 1, "one solid");
    assert!((s.volume() - holed.volume()).abs() < 1e-6, "the volume {} against {}", s.volume(), holed.volume());
    assert_eq!(faces(&s), 10, "top and bottom with their hole, four sides out and four in");
}

/// A CYLINDER'S MESH comes back with each cap one face - a fan of triangles on one plane - and its wall of flat strips;
/// valid, and holding the mesh's volume.
#[test]
fn a_cylinder_mesh_keeps_its_caps_whole() {
    let qymcad_core::geom::Built { mesh, .. } = Shape::cylinder(10.0, 20.0).expect("a cylinder").tessellate(0.05).into_iter().next().expect("a mesh");
    let s = Shape::from_mesh(&mesh).expect("the mesh becomes a body");
    assert!(s.is_valid(), "the body is broken");
    assert_eq!(s.solid_count(), 1, "one solid");
    let held: f64 = mesh
        .tris
        .iter()
        .map(|t| {
            let (a, b, c) = (mesh.verts[t[0] as usize], mesh.verts[t[1] as usize], mesh.verts[t[2] as usize]);
            (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
        })
        .sum();
    assert!((s.volume() - held).abs() < 1e-6 * held, "the volume {} against the mesh's {held}", s.volume());
    let caps = mesh.tris.iter().filter(|t| t.iter().all(|&v| mesh.verts[v as usize].z.abs() < 1e-9) || t.iter().all(|&v| (mesh.verts[v as usize].z - 20.0).abs() < 1e-9)).count();
    let walls = mesh.tris.len() - caps;
    assert_eq!(faces(&s), walls / 2 + 2, "the caps are not one face each, or the wall's strips are not merged: {} triangles", mesh.tris.len());
}
