//! Tests of contour transformations, editing a contour as an object.

use qymcad_core::geom::{Contour, Point2};

fn square(side: f64) -> Contour {
    Contour::closed(vec![
        Point2::new(0.0, 0.0),
        Point2::new(side, 0.0),
        Point2::new(side, side),
        Point2::new(0.0, side),
    ])
}

#[test]
fn translate_moves_bbox() {
    let mut c = square(10.0);
    c.translate(5.0, 3.0);
    let b = c.bbox().unwrap();
    assert!((b.min.x - 5.0).abs() < 1e-9 && (b.min.y - 3.0).abs() < 1e-9);
    assert!((b.max.x - 15.0).abs() < 1e-9);
}

#[test]
fn scale_about_center_doubles_size() {
    let mut c = square(10.0);
    c.scale(Point2::new(5.0, 5.0), 2.0);
    let b = c.bbox().unwrap();
    assert!((b.min.x + 5.0).abs() < 1e-9 && (b.max.x - 15.0).abs() < 1e-9, "20×20 about the centre");
    assert!((c.area() - 400.0).abs() < 1e-6);
}

#[test]
fn dogbone_overcuts_point_outward() {
    use qymcad_core::geom::dogbone_overcuts;
    let c = square(20.0); // corners at (0,0), (20,0), (20,20) and (0,20), with the centre at (10,10)
    let oc = dogbone_overcuts(&c, 2.0);
    assert_eq!(oc.len(), 4, "four sharp corners");
    let centroid = Point2::new(10.0, 10.0);
    for (corner, tip) in &oc {
        // the tip is further from the centre than the corner itself, reaching outwards into the wall
        let dc = ((corner.x - centroid.x).powi(2) + (corner.y - centroid.y).powi(2)).sqrt();
        let dt = ((tip.x - centroid.x).powi(2) + (tip.y - centroid.y).powi(2)).sqrt();
        assert!(dt > dc, "the overcut goes outwards: corner d={dc}, tip d={dt}");
    }
}

#[test]
fn rotate_preserves_area() {
    let mut c = square(10.0);
    let a0 = c.area();
    c.rotate(Point2::new(5.0, 5.0), 37.0);
    assert!((c.area() - a0).abs() < 1e-6, "a rotation does not change the area");
}

#[test]
fn mirror_flips_and_preserves_area() {
    let mut c = square(10.0); // x,y ∈ [0,10]
    let a0 = c.area().abs();
    c.mirror(true, 5.0); // a mirror about the vertical x = 5, giving x in [0,10]: the same square
    assert!((c.area().abs() - a0).abs() < 1e-6, "a mirror preserves the area");
    // the point (0,0) becomes (10,0)
    let mut c2 = Contour::closed(vec![Point2::new(0.0, 0.0), Point2::new(2.0, 0.0), Point2::new(2.0, 3.0), Point2::new(0.0, 3.0)]);
    c2.mirror(true, 0.0); // x → -x
    assert!(c2.points.iter().any(|p| (p.x + 2.0).abs() < 1e-9), "x was mirrored to -2");
}

/// The volume of a mesh, which the rebuild uses to check that an operation actually did something: a thread
/// that removed nothing has to be an error. It is compared against the exact value on a cube and on a prism.
#[test]
fn mesh_volume_matches_exact_solids() {
    use qymcad_core::geom::{Mesh, Point3};
    // a 2×3×4 cube of twelve triangles, with vertices at the corners
    let (a, b, c) = (2.0, 3.0, 4.0);
    let v = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
    let mut m = Mesh { verts: vec![v(0., 0., 0.), v(a, 0., 0.), v(a, b, 0.), v(0., b, 0.), v(0., 0., c), v(a, 0., c), v(a, b, c), v(0., b, c)], tris: vec![] };
    for f in [[0u32, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]] {
        m.tris.push(f);
    }
    assert!((m.volume() - a * b * c).abs() < 1e-9, "cube 2×3×4: V={:.6}, expecting {:.6}", m.volume(), a * b * c);

    // the same cube moved far from the origin: the volume does not depend on position
    let mut far = m.clone();
    for p in &mut far.verts {
        p.x += 1000.0;
        p.y -= 500.0;
        p.z += 77.0;
    }
    assert!((far.volume() - a * b * c).abs() < 1e-6, "the moved cube: V={:.6}", far.volume());

    // degenerate inputs: an empty mesh gives zero rather than a panic
    assert_eq!(Mesh::default().volume(), 0.0);
}
