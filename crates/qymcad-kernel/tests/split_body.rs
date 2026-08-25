//! Splitting a body by a plane.
//!
//! The inverse of assembly: one part falls apart into independent bodies. The application had nothing of the
//! kind — a body could only be grown.

use qymcad_kernel::Shape;

/// A 20 mm cube, its height along Z.
fn cube() -> Shape {
    Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube was built")
}

/// A cut through the middle gives exactly two halves, and no material is lost.
#[test]
fn cutting_through_the_middle_gives_two_halves_and_loses_nothing() {
    let c = cube();
    let v0 = c.volume();
    let parts = c.split_by_plane([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], 0).expect("the body was split");
    assert_eq!(parts.len(), 2, "a plane across the cube gives two halves, and the count came out as {}", parts.len());
    let sum: f64 = parts.iter().map(|p| p.volume()).sum();
    assert!((sum - v0).abs() < 1e-3, "the pieces have to sum to the original volume: it was {v0} and became {sum}");
    for (i, p) in parts.iter().enumerate() {
        assert!((p.volume() - v0 / 2.0).abs() < 1e-3, "half {i} has to be exactly a half, and its volume is {}", p.volume());
        assert!(p.is_valid(), "piece {i} has to be a valid body");
    }
}

/// A plane that misses the body gives an honest refusal rather than a success that changed nothing.
///
/// Without this check the feature would enter the timeline, run to no effect, and leave the reason the body
/// stayed whole to be guessed at.
#[test]
fn a_plane_that_misses_the_body_is_refused() {
    assert!(cube().split_by_plane([0.0, 0.0, 100.0], [0.0, 0.0, 1.0], 0).is_none(), "above the body there is nothing to cut, so a refusal is required");
    assert!(cube().split_by_plane([0.0, 0.0, -5.0], [0.0, 0.0, 1.0], 0).is_none(), "below the body there is nothing to cut, so a refusal is required");
    // touching a face is not a cut either: one piece remains
    assert!(cube().split_by_plane([0.0, 0.0, 20.0], [0.0, 0.0, 1.0], 0).is_none(), "a plane on the face itself splits nothing");
}

/// The pieces keep the face names of the original body, or the references of fillets and chamfers would move
/// after a split.
#[test]
fn each_piece_keeps_the_original_face_names() {
    let c = cube();
    let before = face_ids(&c);
    assert_eq!(before.len(), 6, "setup: a cube has six faces");
    let parts = c.split_by_plane([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], 0).expect("the body was split");

    // The bottom face of the cube has to stay the bottom face of the lower half, under the same id.
    let bottom = face_by_normal(&c, [0.0, 0.0, -1.0]).expect("the bottom of the cube was found");
    let keeps_bottom = parts.iter().any(|p| face_ids(p).contains(&bottom));
    assert!(keeps_bottom, "the id of the bottom face {bottom} has to survive the split");

    let top = face_by_normal(&c, [0.0, 0.0, 1.0]).expect("the top of the cube was found");
    let keeps_top = parts.iter().any(|p| face_ids(p).contains(&top));
    assert!(keeps_top, "the id of the top face {top} has to survive the split");

    // No piece lost its side walls: half a cube has exactly six faces — four walls, a bottom and the cut.
    for (i, p) in parts.iter().enumerate() {
        assert_eq!(face_ids(p).len(), 6, "half {i} has to have six faces");
    }
}

/// An inclined plane cuts the same way: the operation is not tied to the axes.
#[test]
fn a_slanted_plane_cuts_too() {
    let c = cube();
    let v0 = c.volume();
    let parts = c.split_by_plane([10.0, 10.0, 10.0], [1.0, 0.0, 1.0], 0).expect("the inclined plane cuts");
    assert_eq!(parts.len(), 2, "an inclined plane through the middle gives two pieces");
    let sum: f64 = parts.iter().map(|p| p.volume()).sum();
    assert!((sum - v0).abs() < 1e-3, "no material is lost on an incline either: it was {v0} and became {sum}");
}

/// The persistent ids of every face of a body, obtained through tessellation: names have no other way out.
fn face_ids(s: &Shape) -> Vec<u32> {
    let bodies = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let mut ids: Vec<u32> = bodies.first().map(|(_, f)| f.iter().map(|f| f.id).collect()).unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// The id of the face whose normal matches the one given.
fn face_by_normal(s: &Shape, n: [f64; 3]) -> Option<u32> {
    let bodies = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let (_, faces) = bodies.first()?;
    faces.iter().find(|f| f.normal[0] * n[0] + f.normal[1] * n[1] + f.normal[2] * n[2] > 0.9).map(|f| f.id)
}
