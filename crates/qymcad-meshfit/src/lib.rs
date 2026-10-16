//! Turning a triangle mesh into exact surfaces. The chain: prepare the mesh (weld, adjacency, holes,
//! orientation), split it into regions of smoothly joined triangles, fit a primitive to each region, then build
//! faces on the fitted surfaces; the kernel builds the body out of them (`qymcad_kernel::recognise`).
//!
//! No interface lives here: the recognition tool calls into this crate and shows what it found.

pub mod boundary;
pub mod bspline;
pub mod curve;
pub mod prep;
pub mod regions;

pub use boundary::{boundaries, Boundaries, Edge, OPEN};
pub use curve::{curves, Curve};
pub use prep::{chord_deflection, prepare, weld_tolerance, Prepared, NO_NEIGHBOUR};
pub use regions::{free_form_of, regions, regions_until, Region, Surface, Tolerance};

/// THE FACES OF A RAW MESH - a piece of an STL, OBJ or PLY with no B-rep: its flat groups of triangles, turned no more than
/// 8 deg from one another. The one rule for them: a piece is given its faces here when it comes in or has none, and a
/// move or a scale of it in the document carries them along (`MeshFace::transformed`, `MeshFace::scaled`). A face of a
/// raw mesh has no name (`id` 0): a reference to it does not outlive an edit.
pub fn raw_faces(mesh: &qymcad_core::geom::Mesh) -> Vec<qymcad_core::geom::MeshFace> {
    mesh.detect_faces(8.0)
}

/// WHAT A MESH IS FOUND TO BE, triangle by triangle, for a person to see before a body is built: the kind of surface the
/// region of each triangle lies on - 0 plane, 1 cylinder, 2 cone, 3 sphere, 4 torus, 5 none, 6 a free form, 7 a helix or a coil - and how many regions of
/// each kind there are. `tol` multiplies the distance tolerance the mesh itself gives; `sharp_deg` is the sharp edge.
pub fn classify(mesh: &qymcad_core::geom::Mesh, tol: f64, sharp_deg: f64) -> (Vec<u8>, [usize; 8]) {
    let p = prepare(mesh, weld_tolerance(mesh));
    let mut t = Tolerance::for_mesh(&p);
    t.distance *= if tol > 0.0 { tol } else { 1.0 };
    t.sharp_deg = sharp_deg;
    let found = regions(&p, &t);
    let mut kinds = vec![5u8; mesh.tris.len()];
    let mut counts = [0usize; 8];
    for r in &found {
        let k = match r.surface {
            Some(Surface::Plane { .. }) => 0u8,
            Some(Surface::Cylinder { .. }) => 1,
            Some(Surface::Cone { .. }) => 2,
            Some(Surface::Sphere { .. }) => 3,
            Some(Surface::Torus { .. }) => 4,
            None => 5,
            Some(Surface::Free { .. } | Surface::Spline(_)) => 6,
            Some(Surface::Helix { .. } | Surface::RoundHelix { .. } | Surface::Coil { .. }) => 7,
        };
        counts[k as usize] += 1;
        for &tri in &r.tris {
            if let Some(slot) = p.origin.get(tri as usize).and_then(|&o| kinds.get_mut(o as usize)) {
                *slot = k;
            }
        }
    }
    (kinds, counts)
}
