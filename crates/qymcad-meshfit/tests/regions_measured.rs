//! How the angle between neighbouring triangles runs on a real CAD mesh, face by face. Ignored: it measures and
//! does not judge - the answer decides how regions are grown.

use qymcad_meshfit::{prepare, weld_tolerance};
use qymcad_kernel::specimen::Specimen as _;

#[test]
#[ignore = "a measurement"]
fn angles_across_and_inside_the_faces_of_a_filleted_bar() {
    for defl in [0.1, 0.01] {
        // a shape of its own for each: the kernel keeps the first tessellation of a shape and hands it back
        let bar = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted");
        let bodies = qymcad_kernel::specimen::mesh(&bar, defl);
        let qymcad_core::geom::Built { mesh, faces } = &bodies[0];
        let mut face_of = vec![u32::MAX; mesh.tris.len()];
        for (k, f) in faces.iter().enumerate() {
            for &t in &f.triangles {
                face_of[t as usize] = k as u32;
            }
        }
        let p = prepare(mesh, weld_tolerance(mesh));
        let nf = faces.len();
        let (mut inside, mut lo, mut hi) = (vec![0.0f64; nf], vec![f64::MAX; nf], vec![0.0f64; nf]);
        for t in 0..p.mesh.tris.len() {
            let ft = face_of[p.origin[t] as usize] as usize;
            for k in 0..3 {
                let Some(a) = p.dihedral_deg(t, k) else { continue };
                let fu = face_of[p.origin[p.neighbours[t][k] as usize] as usize] as usize;
                if fu == ft {
                    inside[ft] = inside[ft].max(a);
                } else {
                    lo[ft] = lo[ft].min(a);
                    hi[ft] = hi[ft].max(a);
                }
            }
        }
        println!("deflection {defl}: {nf} faces, {} triangles, holes {}, slivers {}", p.mesh.tris.len(), p.holes.len(), p.slivers.len());
        for f in 0..nf {
            println!("  face {f:2}: {:5} tris, inside up to {:6.2} deg, across {:6.2} .. {:6.2} deg", faces[f].triangles.len(), inside[f], lo[f], hi[f]);
        }
    }
}

/// Every region of the rounded bar: its surface, its size, and the faces its triangles lie in.
#[test]
#[ignore = "a measurement"]
fn regions_of_a_filleted_bar_against_its_faces() {
    for defl in [0.1, 0.01] {
        let bar = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted");
        let qymcad_core::geom::Built { mesh, faces } = qymcad_kernel::specimen::mesh(&bar, defl).into_iter().next().expect("a body");
        let mut face_of = vec![u32::MAX; mesh.tris.len()];
        for (k, f) in faces.iter().enumerate() {
            for &t in &f.triangles {
                face_of[t as usize] = k as u32;
            }
        }
        let p = prepare(&mesh, weld_tolerance(&mesh));
        let tol = qymcad_meshfit::Tolerance::for_mesh(&p);
        let found = qymcad_meshfit::regions(&p, &tol);
        println!("deflection {defl}: {} regions for {} faces, tolerance {tol:?}", found.len(), faces.len());
        let mut homes: Vec<Vec<usize>> = vec![Vec::new(); p.mesh.tris.len()];
        for (r, region) in found.iter().enumerate() {
            for &t in &region.tris {
                homes[t as usize].push(r);
            }
        }
        let none = homes.iter().filter(|h| h.is_empty()).count();
        let twice: Vec<(usize, &Vec<usize>)> = homes.iter().enumerate().filter(|(_, h)| h.len() > 1).collect();
        println!("  in no region: {none}, in two or more: {} (of {}); slivers {}", twice.len(), p.mesh.tris.len(), p.slivers.len());
        for (t, h) in twice.iter().take(6) {
            println!("    triangle {t} of face {} lies in regions {h:?}", face_of[p.origin[*t] as usize]);
        }
        for (t, h) in homes.iter().enumerate().filter(|(_, h)| h.is_empty()).take(6) {
            println!("    triangle {t} of face {} lies in no region {h:?}", face_of[p.origin[t] as usize]);
        }
        for (r, region) in found.iter().enumerate() {
            let mut by_face = std::collections::BTreeMap::new();
            for &t in &region.tris {
                *by_face.entry(face_of[p.origin[t as usize] as usize]).or_insert(0) += 1;
            }
            let kind = match &region.surface {
                Some(qymcad_meshfit::Surface::Plane { .. }) => "plane".to_string(),
                Some(qymcad_meshfit::Surface::Cylinder { radius, .. }) => format!("cylinder r {radius:.4}"),
                Some(qymcad_meshfit::Surface::Sphere { radius, .. }) => format!("sphere r {radius:.4}"),
                Some(other) => format!("{other:?}"),
                None => "none".to_string(),
            };
            if by_face.len() > 1 || region.tris.len() < 2 {
                // how far the corners of the triangles of the other faces lie from this region's surface
                let main = by_face.iter().max_by_key(|(_, n)| **n).map(|(f, _)| *f).unwrap_or(u32::MAX);
                let off = region
                    .tris
                    .iter()
                    .filter(|&&t| face_of[p.origin[t as usize] as usize] != main)
                    .flat_map(|&t| p.mesh.tris[t as usize])
                    .map(|i| {
                        let v = p.mesh.verts[i as usize];
                        let v = [v.x, v.y, v.z];
                        let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                        match &region.surface {
                            Some(qymcad_meshfit::Surface::Plane { point, normal }) => dot(d(v, *point), *normal).abs(),
                            Some(qymcad_meshfit::Surface::Cylinder { point, axis, radius }) => {
                                let w = d(v, *point);
                                let along = dot(w, *axis);
                                let r = d(w, [axis[0] * along, axis[1] * along, axis[2] * along]);
                                (dot(r, r).sqrt() - radius).abs()
                            }
                            Some(qymcad_meshfit::Surface::Sphere { center, radius }) => (dot(d(v, *center), d(v, *center)).sqrt() - radius).abs(),
                            Some(other) => other.distance(v),
                            None => 0.0,
                        }
                    })
                    .fold(0.0f64, f64::max);
                println!("  region {r:2}: {kind}, {} tris, faces {by_face:?}, other faces' corners off it up to {off:.6}", region.tris.len());
            }
        }
    }
}
