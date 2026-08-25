//! A thread once landed on a shaft as though it were a hole and removed almost nothing.
//!
//! The kind of a cylindrical face, shaft or hole, decides which way the groove goes, so an error here means a
//! thread cut into thin air. The earlier check took the averaged normal of the whole face: on a full cylinder
//! the normals cancel to zero around the circle and the centroid lands on the axis, so the sign came out at
//! random. These tests hold the classification on closed cylinders, which is exactly where the old method
//! lied.
use qymcad_core::geom::{cyl_face_is_internal, Mesh, Point3};

const TAU: f64 = std::f64::consts::TAU;

/// The side surface of a cylinder of radius `r` and height `h` along Z, with normals pointing outwards for a
/// shaft or inwards for the wall of a hole. In a mesh that is the only difference between the two.
fn cylinder_side(r: f64, h: f64, seg: usize, outward: bool) -> (Mesh, Vec<u32>) {
    let mut m = Mesh::default();
    for i in 0..=1 {
        for j in 0..seg {
            let a = j as f64 / seg as f64 * TAU;
            m.verts.push(Point3::new(r * a.cos(), r * a.sin(), i as f64 * h));
        }
    }
    for j in 0..seg {
        let (a, b) = (j as u32, ((j + 1) % seg) as u32);
        let (c, d) = (a + seg as u32, b + seg as u32);
        if outward {
            m.tris.push([a, b, c]);
            m.tris.push([b, d, c]);
        } else {
            m.tris.push([a, c, b]);
            m.tris.push([b, c, d]);
        }
    }
    let tris = (0..m.tris.len() as u32).collect();
    (m, tris)
}

#[test]
fn full_shaft_is_external_full_bore_is_internal() {
    let (shaft, st) = cylinder_side(15.0, 100.0, 64, true);
    let (bore, bt) = cylinder_side(15.0, 100.0, 64, false);
    assert!(
        !cyl_face_is_internal(&shaft, &st, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        "a solid Ø30 shaft takes an external thread; this exact error produced a thread cut into thin air"
    );
    assert!(cyl_face_is_internal(&bore, &bt, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "a Ø30 hole takes an internal thread");
}

/// The axis is neither at the origin nor along Z: the classification has to work from the axis it is given
/// rather than from world coordinates, since a thread is placed on a part inside an assembly with a coordinate
/// system of its own.
#[test]
fn works_for_shifted_and_tilted_axis() {
    let (mut m, tris) = cylinder_side(8.0, 40.0, 48, true);
    // rotate 90° about X, so the axis of the cylinder becomes −Y, and translate
    for v in &mut m.verts {
        let (y, z) = (v.y, v.z);
        v.y = -z;
        v.z = y;
        v.x += 100.0;
        v.y += 50.0;
        v.z -= 20.0;
    }
    assert!(!cyl_face_is_internal(&m, &tris, [100.0, 50.0, -20.0], [0.0, -1.0, 0.0]), "a translated and rotated shaft still takes an external thread");
    // and with the axis reversed the answer is the same: which side the material is on does not depend on the
    // direction
    assert!(!cyl_face_is_internal(&m, &tris, [100.0, 50.0, -20.0], [0.0, 1.0, 0.0]), "the direction of the axis does not affect the kind of the face");
}

/// A partial face, a half cylinder, has to be classified too. The old method did work on this case, so the
/// check is that the new one did not break it.
#[test]
fn half_cylinder_still_classifies() {
    let half = |outward: bool| {
        let (m, all) = cylinder_side(10.0, 20.0, 64, outward);
        let half: Vec<u32> = all.into_iter().filter(|&t| m.tri_normal_area(t as usize).2.x > 0.0).collect();
        (m, half)
    };
    let (ms, ts) = half(true);
    let (mb, tb) = half(false);
    assert!(!ts.is_empty() && !tb.is_empty());
    assert!(!cyl_face_is_internal(&ms, &ts, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "half a shaft takes an external thread");
    assert!(cyl_face_is_internal(&mb, &tb, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "half a hole takes an internal thread");
}

/// Rubbish on the input is not a panic: an empty triangle list, a zero axis, indices past the end of the mesh.
#[test]
fn degenerate_input_is_safe() {
    let (m, tris) = cylinder_side(5.0, 10.0, 16, true);
    assert!(!cyl_face_is_internal(&m, &[], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "with no triangles it counts as a shaft");
    assert!(!cyl_face_is_internal(&m, &tris, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]), "a zero axis does not panic");
    assert!(!cyl_face_is_internal(&m, &[9999, 10000], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "indices past the end of the mesh do not panic");
}

/// The radius of a face, and telling a cylinder from a chamfer: along the axis the radius of a cylinder is
/// constant while that of a cone changes. By that test the thread target picks the right rim — a chamfered face
/// has several, of different radii — and refuses honestly when the chamfer itself was clicked.
///
/// Without it, a chamfer applied beforehand made a thread impossible to create.
#[test]
fn cylinder_and_cone_are_told_apart_by_radius_spread() {
    use qymcad_core::geom::cyl_face_radius;
    let (cyl, ct) = cylinder_side(12.0, 40.0, 64, true);
    let (r, spread) = cyl_face_radius(&cyl, &ct, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).expect("radius of the cylinder");
    assert!((r - 12.0).abs() < 0.05, "the radius of the cylinder is {r:.3} instead of 12");
    assert!(spread < 0.01, "the radius spread of a cylinder is zero, got {spread:.4}");

    // a cone, that is a chamfer: the radius changes linearly from 12 to 8
    let mut cone = qymcad_core::geom::Mesh::default();
    let seg = 64;
    for i in 0..=1 {
        for j in 0..seg {
            let a = j as f64 / seg as f64 * TAU;
            let rr = if i == 0 { 12.0 } else { 8.0 };
            cone.verts.push(Point3::new(rr * a.cos(), rr * a.sin(), i as f64 * 4.0));
        }
    }
    for j in 0..seg {
        let (a, b) = (j as u32, ((j + 1) % seg) as u32);
        let (c, d) = (a + seg as u32, b + seg as u32);
        cone.tris.push([a, b, c]);
        cone.tris.push([b, d, c]);
    }
    let ct2: Vec<u32> = (0..cone.tris.len() as u32).collect();
    let (rc, spread_c) = cyl_face_radius(&cone, &ct2, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).expect("radius of the cone");
    assert!((rc - 10.0).abs() < 0.6, "the mean radius of the cone is about 10, got {rc:.3}");
    assert!(spread_c > 0.3, "a cone from 12 to 8 has a radius spread of about 0.4, got {spread_c:.4}, so a chamfer is indistinguishable from a cylinder");

    // and a small chamfer, 1 mm on Ø20, has to differ from a cylinder, or it passes as one
    let mut small = qymcad_core::geom::Mesh::default();
    for i in 0..=1 {
        for j in 0..seg {
            let a = j as f64 / seg as f64 * TAU;
            let rr = if i == 0 { 10.0 } else { 9.0 };
            small.verts.push(Point3::new(rr * a.cos(), rr * a.sin(), i as f64));
        }
    }
    for j in 0..seg {
        let (a, b) = (j as u32, ((j + 1) % seg) as u32);
        let (c, d) = (a + seg as u32, b + seg as u32);
        small.tris.push([a, b, c]);
        small.tris.push([b, d, c]);
    }
    let st: Vec<u32> = (0..small.tris.len() as u32).collect();
    let (_, spread_s) = cyl_face_radius(&small, &st, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).expect("radius of the small chamfer");
    assert!(spread_s > 0.08, "a 1 mm chamfer on Ø20 has a spread of {spread_s:.4}, so it would be taken for a cylinder");

    assert!(cyl_face_radius(&cyl, &[], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "no triangles means no answer");
    assert!(cyl_face_radius(&cyl, &ct, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]).is_none(), "a zero axis means no answer");
}

/// The direction of a thread is taken from the selected face rather than from where most of the vertices of the
/// body are. On a part whose bulk lies towards the end face, such as a boss on a plate, counting over the mesh
/// turned the thread outwards — with a chamfer present it then ran from the end of the chamfer into thin air.
#[test]
fn thread_direction_follows_the_picked_face() {
    use qymcad_core::geom::axis_along_face;
    // a cylinder from z = 0 to z = 40, taking the upper rim at z = 40: the thread has to run downwards along
    // the face
    let (m, tris) = cylinder_side(10.0, 40.0, 48, true);
    let dir = axis_along_face(&m, &tris, [0.0, 0.0, 40.0], [0.0, 0.0, 1.0]);
    assert!(dir[2] < -0.9, "from the upper rim the thread runs downwards along the cylinder, got {dir:?}");
    let dir2 = axis_along_face(&m, &tris, [0.0, 0.0, 40.0], [0.0, 0.0, -1.0]);
    assert!(dir2[2] < -0.9, "the initial direction of the axis does not affect the answer: {dir2:?}");
    // from the lower rim it runs upwards
    let up = axis_along_face(&m, &tris, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    assert!(up[2] > 0.9, "from the lower rim the thread runs upwards, got {up:?}");
    // rubbish does not bring it down
    assert_eq!(axis_along_face(&m, &tris, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
}
