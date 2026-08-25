//! Tests of face recognition on a mesh.

use qymcad_core::geom::{Mesh, Point3};

/// A 10×10×10 cube of twelve triangles, with consistent winding across the faces.
fn cube() -> Mesh {
    let v = vec![
        Point3::new(0.0, 0.0, 0.0),   // 0
        Point3::new(10.0, 0.0, 0.0),  // 1
        Point3::new(10.0, 10.0, 0.0), // 2
        Point3::new(0.0, 10.0, 0.0),  // 3
        Point3::new(0.0, 0.0, 10.0),  // 4
        Point3::new(10.0, 0.0, 10.0), // 5
        Point3::new(10.0, 10.0, 10.0),// 6
        Point3::new(0.0, 10.0, 10.0), // 7
    ];
    let tris = vec![
        [0, 2, 1], [0, 3, 2], // bottom
        [4, 5, 6], [4, 6, 7], // top
        [0, 1, 5], [0, 5, 4], // front
        [3, 2, 6], [3, 6, 7], // back
        [0, 4, 7], [0, 7, 3], // left
        [1, 2, 6], [1, 6, 5], // right
    ];
    Mesh { verts: v, tris }
}

#[test]
fn cube_has_six_faces() {
    let faces = cube().detect_faces(5.0);
    assert_eq!(faces.len(), 6, "a cube has six faces, found {}", faces.len());
    // each face has an area of 100, being 10×10 from two triangles
    for f in &faces {
        assert!((f.area - 100.0).abs() < 1e-6, "the area of a face is about 100, got {}", f.area);
        assert_eq!(f.triangles.len(), 2);
    }
    // the normals cover ±X, ±Y and ±Z
    let has = |n: [f64; 3]| faces.iter().any(|f| {
        (f.normal[0] - n[0]).abs() < 0.01 && (f.normal[1] - n[1]).abs() < 0.01 && (f.normal[2] - n[2]).abs() < 0.01
    });
    assert!(has([0.0, 0.0, 1.0]) && has([0.0, 0.0, -1.0]), "top and bottom");
    assert!(has([1.0, 0.0, 0.0]) && has([-1.0, 0.0, 0.0]), "±X");
}

#[test]
fn top_face_outline_is_square() {
    let c = cube();
    let faces = c.detect_faces(5.0);
    // the top face, with a normal along +Z
    let top = faces.iter().find(|f| f.normal[2] > 0.85).expect("the top face");
    let outlines = c.face_outline_xy(top);
    assert_eq!(outlines.len(), 1, "a single closed boundary");
    let o = &outlines[0];
    assert!(o.closed, "the contour is closed");
    assert!((o.area() - 100.0).abs() < 1e-6, "a 10×10 square, area {}", o.area());
}
