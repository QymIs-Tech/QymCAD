//! A FLAT FACE IS NO SPHERE. An assembly tangency classifies a face picked by its centre through `face_sphere`, a fit of
//! a sphere to the vertices of the face. Every circle lies on endless spheres, so a flat face with a round rim fits one
//! tightly. Reported behaviour: the flat floor of a 260 x 162 pocket with R11 corners and an R4 rounding along its
//! bottom came back as a sphere of radius about 534; a flat disk of radius 100 with 256 vertices on its rim comes back
//! as a sphere of radius about 368. A spherical cap is still a sphere.
use qymcad_core::feature::FaceKey;
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::{Id, Project};

/// The face of the probe, on a body of its own.
const BODY: Id = 1000;
const FACE: u32 = 1001;

/// A document holding one body, the mesh `mesh` with all its triangles one face.
fn one_face(mesh: Mesh) -> Project {
    let mut p = Project::default();
    p.new_document();
    let mut face = mesh.meshface_from_triangles((0..mesh.tris.len() as u32).collect());
    face.id = FACE;
    p.set_body_mesh(BODY, mesh);
    p.regen_faces.insert(BODY, vec![face]);
    p
}

/// A flat disk of radius 100 at one height off the origin, 256 vertices on its rim, fanned from the first.
fn flat_disk() -> Mesh {
    let z = f64::from(1.1234568_f32);
    let n = 256_u32;
    let verts: Vec<Point3> = (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            Point3::new(-38.60926322568048 + 100.0 * a.cos(), 4.437894717610419 + 100.0 * a.sin(), z)
        })
        .collect();
    let tris: Vec<[u32; 3]> = (1..n - 1).map(|i| [0, i, i + 1]).collect();
    Mesh { verts, tris, norms: Vec::new() }
}

/// The cap of a sphere of radius 50 about (3, -4, 5), 60 deg from its pole, its triangles facing out.
fn spherical_cap() -> Mesh {
    let (rings, around) = (12_u32, 48_u32);
    let at = |i: u32, j: u32| {
        let (t, f) = (60f64.to_radians() * f64::from(i) / f64::from(rings), std::f64::consts::TAU * f64::from(j) / f64::from(around));
        Point3::new(3.0 + 50.0 * t.sin() * f.cos(), -4.0 + 50.0 * t.sin() * f.sin(), 5.0 + 50.0 * t.cos())
    };
    let mut verts = vec![at(0, 0)];
    for i in 1..=rings {
        verts.extend((0..around).map(|j| at(i, j)));
    }
    let index = |i: u32, j: u32| if i == 0 { 0 } else { 1 + (i - 1) * around + j % around };
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..around {
        tris.push([0, index(1, j), index(1, j + 1)]);
    }
    for i in 1..rings {
        for j in 0..around {
            tris.push([index(i, j), index(i + 1, j), index(i, j + 1)]);
            tris.push([index(i + 1, j), index(i + 1, j + 1), index(i, j + 1)]);
        }
    }
    Mesh { verts, tris, norms: Vec::new() }
}

#[test]
fn a_flat_disk_is_no_sphere() {
    let p = one_face(flat_disk());
    let sphere = p.face_sphere(BODY, &FaceKey { id: FACE, ..Default::default() });
    assert!(sphere.is_none(), "every vertex lies on one plane, yet face_sphere gives {sphere:?}");
}

#[test]
fn a_spherical_cap_is_a_sphere() {
    let p = one_face(spherical_cap());
    let sphere = p.face_sphere(BODY, &FaceKey { id: FACE, ..Default::default() });
    let Some(([x, y, z], r)) = sphere else { panic!("a cap of a sphere of radius 50 is not taken for a sphere") };
    assert!((x - 3.0).abs() < 1e-6 && (y + 4.0).abs() < 1e-6 && (z - 5.0).abs() < 1e-6 && (r - 50.0).abs() < 1e-6, "the cap gives the sphere ({x}, {y}, {z}) r {r}, not (3, -4, 5) r 50");
}
