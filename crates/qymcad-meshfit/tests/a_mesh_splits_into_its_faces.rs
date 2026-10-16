//! A mesh split into regions, each one face of the solid it was made from. The meshes come from our kernel, so the
//! true face of every triangle is known and each region is checked against it - not only counted.

use qymcad_kernel::Shape;
use qymcad_meshfit::{prepare, regions, weld_tolerance, Surface, Tolerance};
use qymcad_kernel::specimen::Specimen as _;

/// Splits the tessellation of `shape` and checks it against the faces: as many regions as faces, each within one
/// face. Returns how many planes, cylinders and spheres were found.
fn split_as_faces(name: &str, shape: &Shape, deflection: f64) -> [usize; 3] {
    let bodies = qymcad_kernel::specimen::mesh(shape, deflection);
    let qymcad_core::geom::Built { mesh, faces } = bodies.into_iter().next().expect("a body");
    let mut face_of = vec![u32::MAX; mesh.tris.len()];
    for (k, f) in faces.iter().enumerate() {
        for &t in &f.triangles {
            face_of[t as usize] = k as u32;
        }
    }
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let mut seen = vec![0usize; p.mesh.tris.len()];
    let mut mixed = Vec::new();
    // A TRIANGLE OF ANOTHER FACE MAY SIT IN A REGION ONLY ALONG ITS BORDER AND ON ITS SURFACE. On the finer bar two
    // triangles of each corner sphere hug the rounding's end within 0.00006 mm of its cylinder: on both surfaces at
    // once, closer than any tolerance a text STL survives. Where exactly the border runs is settled by the surfaces
    // meeting, a step later; a triangle off the surface is a region gone wrong.
    for (r, region) in found.iter().enumerate() {
        for &t in &region.tris {
            seen[t as usize] += 1;
        }
        let mut count = std::collections::BTreeMap::new();
        for &t in &region.tris {
            *count.entry(face_of[p.origin[t as usize] as usize]).or_insert(0usize) += 1;
        }
        let main = count.iter().max_by_key(|(_, n)| **n).map(|(f, _)| *f).expect("a region has triangles");
        let surface = region.surface.as_ref();
        let stray = region.tris.iter().filter(|&&t| face_of[p.origin[t as usize] as usize] != main).any(|&t| {
            p.mesh.tris[t as usize].iter().any(|&i| {
                let v = p.mesh.verts[i as usize];
                surface.is_none_or(|s| s.distance([v.x, v.y, v.z]) > tol.distance)
            })
        });
        if stray {
            mixed.push(r);
        }
    }
    assert!(seen.iter().all(|&n| n == 1), "{name}: a triangle is in no region or in two");
    assert!(mixed.is_empty(), "{name}: {} regions cross from one face into another: {mixed:?}", mixed.len());
    assert_eq!(found.len(), faces.len(), "{name}: {} regions for {} faces", found.len(), faces.len());
    let mut kinds = [0usize; 3];
    for r in &found {
        match r.surface {
            Some(Surface::Plane { .. }) => kinds[0] += 1,
            Some(Surface::Cylinder { .. }) => kinds[1] += 1,
            Some(Surface::Sphere { .. }) => kinds[2] += 1,
            _ => {}
        }
    }
    kinds
}

fn bar() -> Shape {
    qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted")
}

/// A BAR WITH EVERY EDGE ROUNDED: six planes, twelve cylinders along the edges, eight spheres at the corners - and
/// every one of them tangent to its neighbours, so no angle between triangles tells them apart.
#[test]
fn a_rounded_bar_splits_into_its_twenty_six_faces() {
    for deflection in [0.1, 0.01] {
        let kinds = split_as_faces(&format!("bar at {deflection}"), &bar(), deflection);
        assert_eq!(kinds, [6, 12, 8], "bar at {deflection}: planes, cylinders, spheres");
    }
}

#[test]
fn a_cylinder_splits_into_two_caps_and_a_wall() {
    let kinds = split_as_faces("cylinder", &qymcad_kernel::specimen::cylinder(10.0, 30.0).expect("a cylinder"), 0.05);
    assert_eq!(kinds, [2, 1, 0], "cylinder: planes, cylinders, spheres");
}

#[test]
fn a_sphere_is_one_region() {
    let kinds = split_as_faces("sphere", &qymcad_kernel::specimen::sphere(15.0).expect("a sphere"), 0.05);
    assert_eq!(kinds, [0, 0, 1], "sphere: planes, cylinders, spheres");
}

/// A THREAD DRAWN AS RINGS SPLITS INTO ITS RINGS' FACES: ten rings of a 0.5 mm pitch on a 1.2 mm core, 1.5 mm at the
/// crests - each ring a cone up, a narrow crest cylinder, a cone down, the rings standing on narrow root cylinders, and
/// a plane at either end: 43 faces, 2 planes, 21 cylinders, 20 cones. Catalogue screws come drawn so (the tensioner
/// screw of the owner's print head is one), and every band of it is narrow - a cone a few triangles wide, which a
/// sphere through the same corners fits as closely.
#[test]
fn a_thread_drawn_as_rings_splits_into_its_faces() {
    let (r0, r1, pitch, root, crest, n) = (1.2, 1.5, 0.5, 0.06, 0.06, 10usize);
    let flank = (pitch - root - crest) / 2.0;
    // the profile in the XY plane, y the radius, turned about X
    let mut xy = vec![0.0, 0.0, 0.0, r0];
    let mut x = 0.0;
    for _ in 0..n {
        x += root;
        xy.extend([x, r0]);
        x += flank;
        xy.extend([x, r1]);
        x += crest;
        xy.extend([x, r1]);
        x += flank;
        xy.extend([x, r0]);
    }
    x += root;
    xy.extend([x, r0, x, 0.0]);
    let thread = qymcad_kernel::specimen::revolve(&xy, 0, 360.0).expect("the thread");
    for deflection in [0.005, 0.001] {
        let kinds = split_as_faces(&format!("thread at {deflection}"), &thread, deflection);
        assert_eq!(kinds, [2, 2 * n + 1, 0], "thread at {deflection}: planes, cylinders, spheres - a flank is a cone, not a sphere");
    }
}

/// A WALL CUT BY A BOOLEAN IS ONE REGION, NOT STRIPS. A cylinder of 10 by 30 with a sphere of 10.5 fused onto its top,
/// the sphere's centre 0.1 mm off the axis: the two meet along a curve the kernel can only approximate, and the wall
/// under it is meshed as long thin strips running up to that curve. The wall is one cylinder of 10.
#[test]
fn a_wall_cut_by_a_boolean_is_one_region() {
    let post = qymcad_kernel::specimen::cylinder(10.0, 30.0).expect("a cylinder");
    let up = [1.0, 0.0, 0.0, 0.1, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 30.0];
    let knob = qymcad_kernel::specimen::placed(&qymcad_kernel::specimen::sphere(10.5).expect("a sphere"), &up).expect("the sphere on the top");
    let both = qymcad_kernel::specimen::fuse(&[&post, &knob]).expect("the knob on the post");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&both, 0.02).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let walls: Vec<(usize, f64)> = found
        .iter()
        .filter_map(|r| match r.surface {
            Some(Surface::Cylinder { radius, .. }) => Some((r.tris.len(), radius)),
            _ => None,
        })
        .collect();
    let off = p.mesh.verts.iter().filter(|v| v.z > 0.01 && v.z < 19.4).map(|v| ((v.x * v.x + v.y * v.y).sqrt() - 10.0).abs()).fold(0.0, f64::max);
    let one = walls.len() == 1 && (walls[0].1 - 10.0).abs() < 0.01;
    assert!(
        one,
        "the wall came in {} cylinders {:?} (of {} regions); its corners stand up to {off:.1e} mm off the true wall, the distance tolerance is {:.1e}",
        walls.len(),
        &walls[..walls.len().min(6)],
        found.len(),
        tol.distance
    );
}
