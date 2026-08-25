//! Extruding an annular profile — an outer circle with an inner one as a hole — gives a tube.
use qymcad_kernel::Shape;

use qymcad_core::geom::{encode_loops, Point2, ProfEdge};

// The profile is assembled by the production encoder rather than by a copy of the format inside the test.
fn annulus_profile(r_out: f64, r_in: f64) -> Vec<f64> {
    let c = |r: f64| ProfEdge::Circle { center: Point2::new(0.0, 0.0), r };
    encode_loops(&[&[c(r_out)], &[c(r_in)]]) // the outer circle with the inner one as a hole
}

#[test]
fn annulus_extrudes_to_tube() {
    let data = annulus_profile(10.0, 5.0);
    let s = Shape::extrude_profile(&data, 8.0).expect("extruding the annulus");
    let bodies = s.tessellate(0.2);
    eprintln!("bodies: {}", bodies.len());
    assert_eq!(bodies.len(), 1, "one body");
    let (mesh, faces) = &bodies[0];
    eprintln!("vertices: {}, triangles: {}, faces: {}", mesh.verts.len(), mesh.tris.len(), faces.len());
    for (i, f) in faces.iter().enumerate() {
        eprintln!("  face {i}: id={} triangles={}", f.id, f.triangles.len());
    }
    // A tube has four faces: the outer cylinder, the inner cylinder, and the top and bottom annuli.
    assert_eq!(faces.len(), 4, "a tube has four faces: two cylinders and two annuli");
    assert!(faces.iter().all(|f| !f.triangles.is_empty()), "every face has triangles, so the annuli are not empty");
}
