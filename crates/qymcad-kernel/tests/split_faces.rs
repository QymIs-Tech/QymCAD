//! Splitting faces without cutting the body.
//!
//! The difference from a body split is fundamental: the body stays one while the faces break into parts. This is
//! how an area is marked out for painting, for machining, or for a future feature.
use qymcad_kernel::Shape;

fn cube() -> Shape {
    Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube")
}

fn face_count(s: &Shape) -> usize {
    let b = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let mut ids: Vec<u32> = b.first().map(|(_, f)| f.iter().map(|f| f.id).collect()).unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    ids.len()
}

/// The body stays whole and the face count grows: four walls split in half, so six faces become ten.
#[test]
fn splitting_faces_keeps_one_body_and_adds_faces() {
    let c = cube();
    let v0 = c.volume();
    assert_eq!(face_count(&c), 6, "setup: a cube has six faces");

    let split = c.split_faces([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]).expect("the faces were split");
    assert!((split.volume() - v0).abs() < 1e-6, "the body is not cut: the volume has to stay {v0} and became {}", split.volume());
    assert!(split.is_valid(), "the body has to stay valid");
    assert_eq!(face_count(&split), 10, "four walls split in half, giving 6 + 4 = 10, and the result was {}", face_count(&split));
}

/// A plane that misses the body gives an honest refusal rather than a success that split nothing.
#[test]
fn a_plane_that_misses_is_refused() {
    assert!(cube().split_faces([0.0, 0.0, 100.0], [0.0, 0.0, 1.0]).is_none(), "above the body there is nothing to split");
    assert!(cube().split_faces([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]).is_none(), "below the body there is nothing to split");
}

/// Untouched faces keep their names, or marking out an area would move fillet references across the whole
/// part.
#[test]
fn untouched_faces_keep_their_names() {
    let c = cube();
    let bodies = c.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let (_, faces) = bodies.first().expect("the body");
    let bottom = faces.iter().find(|f| f.normal[2] < -0.9).map(|f| f.id).expect("the bottom");
    let top = faces.iter().find(|f| f.normal[2] > 0.9).map(|f| f.id).expect("the top");

    let split = c.split_faces([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]).expect("split");
    let b2 = split.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let (_, f2) = b2.first().expect("the body");
    let ids: Vec<u32> = f2.iter().map(|f| f.id).collect();
    assert!(ids.contains(&bottom), "the plane did not touch the bottom, so its name {bottom} has to remain");
    assert!(ids.contains(&top), "the plane did not touch the top, so its name {top} has to remain");
}
