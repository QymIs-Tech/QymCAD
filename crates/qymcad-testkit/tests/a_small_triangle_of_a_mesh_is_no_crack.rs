//! A SMALL TRIANGLE OF A MESH IS NO CRACK. The common funnel of every operation takes out cracks - faces narrower than
//! the precision a limiting rounding leaves - and it knew them by their area alone. A body made of a mesh has thousands
//! of faces under that area that are not narrow at all, and each operation on such a body copied it whole and ran the
//! repair of small faces over it for nothing: measured on a 178 032-triangle mesh, 50 s of every operation, with not one
//! face taken out.
use qymcad_core::geom::{Mesh, Point3};

/// A closed ball of radius `r` as a mesh of `n` x `m` bands.
fn ball(r: f64, n: usize, m: usize) -> Mesh {
    let mut verts = vec![Point3::new(0.0, 0.0, -r)];
    for j in 1..m {
        let t = std::f64::consts::PI * j as f64 / m as f64;
        for i in 0..n {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            verts.push(Point3::new(r * t.sin() * a.cos(), r * t.sin() * a.sin(), -r * t.cos()));
        }
    }
    verts.push(Point3::new(0.0, 0.0, r));
    let top = (verts.len() - 1) as u32;
    let at = |j: usize, i: usize| (1 + (j - 1) * n + i % n) as u32;
    let mut tris = Vec::new();
    for i in 0..n {
        tris.push([0, at(1, i), at(1, i + 1)]);
        tris.push([top, at(m - 1, i + 1), at(m - 1, i)]);
    }
    for j in 1..m - 1 {
        for i in 0..n {
            tris.push([at(j, i), at(j + 1, i), at(j, i + 1)]);
            tris.push([at(j, i + 1), at(j + 1, i), at(j + 1, i + 1)]);
        }
    }
    Mesh { verts, tris, ..Default::default() }
}

#[test]
fn the_faces_of_a_fine_mesh_are_not_taken_for_cracks() {
    // a ball of 0.2 mm in 40 x 20 bands: faces of 2e-4 mm^2 and more, under the crack's area, none of them narrow
    let body = qymcad_kernel::specimen::from_mesh(&ball(0.2, 40, 20)).expect("a body of the mesh");
    assert!(!body.is_sheet(), "setup: the ball did not close into a solid");
    let cracks = body.sliver_faces();
    assert_eq!(cracks, 0, "{cracks} small triangles of a mesh were taken for cracks - each operation on such a body would copy it and repair it for nothing");
}

/// A BODY MADE OF A MESH IS SHOWN BY ITS OWN TRIANGLES: a flat face bounded by straight edges meshes the same at any
/// deflection, so the faces keep the triangles the mesh gave them and the tessellation meshes none of them again.
/// Measured on a 178 032-triangle mesh: 10 s of every operation over the body went on meshing its 144 381 flat faces
/// anew. And a flat box meshed once is not meshed again at another deflection.
#[test]
fn a_body_of_a_mesh_keeps_its_own_triangles() {
    let body = qymcad_kernel::specimen::from_mesh(&ball(0.2, 40, 20)).expect("a body of the mesh");
    let meshed = qymcad_kernel::remeshed_faces(&body, 0.001);
    assert_eq!(meshed, 0, "the tessellation meshed {meshed} flat faces of a body made of a mesh anew");
    let block = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 5.0, 0.0, 5.0], 3.0).expect("a block");
    let first = qymcad_kernel::remeshed_faces(&block, 0.1);
    let again = qymcad_kernel::remeshed_faces(&block, 0.01);
    assert_eq!((first, again), (6, 0), "a flat box is meshed once and keeps its triangles at another deflection");
}

/// A BODY MADE OF A MESH IS SOUND AS IT IS MADE: the check of the kernel passes it, so no operation over it has to
/// repair it. Measured on a 178 032-triangle mesh: the check refused the body (21 s) and the repair took 94 s, on the
/// body and again on every operation after it.
#[test]
fn a_body_of_a_mesh_is_sound_as_it_is_made() {
    let out = ball(0.2, 40, 20);
    // the same ball wound the other way round: a mesh may come in wound inwards, all of it
    let inward = Mesh { verts: out.verts.clone(), tris: out.tris.iter().map(|t| [t[0], t[2], t[1]]).collect(), ..Default::default() };
    for (name, mesh) in [("wound outwards", out), ("wound inwards", inward)] {
        let body = qymcad_kernel::specimen::from_mesh(&mesh).expect("a body of the mesh");
        assert!(body.volume() > 0.0, "{name}: the body holds {} - it is inside out", body.volume());
        assert!(body.is_valid(), "{name}: the kernel's check refuses a body made of a closed clean mesh - every operation over it would repair it");
    }
}

/// A LARGE BODY OF A MESH WITH A FACE FLAT ONLY TO THE PRECISION OF ITS FILE is sound too. Triangles are joined into one
/// flat face while their corners lie on its plane to a millionth of a millionth of the size of the part - on a part of
/// 100 mm that is 1.1e-7 mm, more than the tolerance an edge was built with (1e-7), and the check refused every edge that
/// stood off its face's plane by more (798 edges of a 178 032-triangle mesh). A cube of 1000 mm with its top of four
/// triangles, the middle corner lifted by 5e-7 mm, is that case.
#[test]
fn a_large_body_of_a_mesh_with_a_face_flat_to_its_precision_is_sound() {
    let v = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
    let s = 1000.0;
    let verts = vec![v(0.0, 0.0, 0.0), v(s, 0.0, 0.0), v(s, s, 0.0), v(0.0, s, 0.0), v(0.0, 0.0, s), v(s, 0.0, s), v(s, s, s), v(0.0, s, s), v(s / 2.0, s / 2.0, s + 5e-7)];
    let tris = vec![
        [0, 2, 1],
        [0, 3, 2], // bottom, facing down
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7], // sides
        [4, 5, 8],
        [5, 6, 8],
        [6, 7, 8],
        [7, 4, 8], // the top, around its lifted middle
    ];
    let body = qymcad_kernel::specimen::from_mesh(&Mesh { verts, tris, ..Default::default() }).expect("a body of the mesh");
    assert!(body.volume() > 0.0, "setup: the cube is inside out");
    let top = body.face_kinds().map(|k| k.iter().sum::<u32>()).unwrap_or(0);
    assert_eq!(top, 6, "setup: the top of four triangles within the precision is to be one flat face");
    assert!(body.is_valid(), "the kernel's check refuses a large body whose flat face holds its corners to the precision of the file");
}
