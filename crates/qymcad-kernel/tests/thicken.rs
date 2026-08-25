//! Thickening a face: turning a face of a body into a plate of a given thickness.
//!
//! This is how a part is made from a curved surface: take a face of a housing, thicken it, and the skin is
//! ready. The application had nothing of the kind before; a body could only be grown from a sketch.

/// A 20 mm cube.
fn cube() -> qymcad_kernel::Shape {
    qymcad_kernel::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube")
}

/// The id of the top face.
fn top_face(s: &qymcad_kernel::Shape) -> Option<u32> {
    let bodies = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let (_, faces) = bodies.first()?;
    faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()).map(|f| f.id)
}

/// A flat 20×20 face thickened by 3 gives a plate of exactly 20·20·3.
#[test]
fn thickening_a_flat_face_gives_a_plate_of_that_exact_volume() {
    let c = cube();
    let top = top_face(&c).expect("the top");
    let plate = c.thicken_face(top, 3.0, &[], &[]).expect("the plate was built");
    let v = plate.volume();
    assert!((v - 20.0 * 20.0 * 3.0).abs() < 1e-3, "a 20×20×3 plate is 1200, and came out as {v}");
    assert!(plate.is_valid(), "the plate has to be a valid body");
}

/// The sign chooses the side while the volume does not depend on it: a thickness is a thickness.
#[test]
fn the_sign_picks_the_side_not_the_amount() {
    let c = cube();
    let top = top_face(&c).expect("the top");
    let up = c.thicken_face(top, 3.0, &[], &[]).expect("outwards");
    let down = c.thicken_face(top, -3.0, &[], &[]).expect("inwards");
    assert!((up.volume() - down.volume()).abs() < 1e-3, "the thickness is the same and the side differs: {} and {}", up.volume(), down.volume());
    let (ub, db) = (up.bbox().expect("the extents"), down.bbox().expect("the extents"));
    assert!(ub[5] > db[5] + 1.0, "outwards the plate sits above the original face and inwards below it: {ub:?} against {db:?}");
}

/// A zero thickness is a refusal rather than a body of zero volume.
#[test]
fn zero_thickness_is_refused() {
    let c = cube();
    let top = top_face(&c).expect("the top");
    assert!(c.thicken_face(top, 0.0, &[], &[]).is_none(), "a plate of zero thickness is not a body");
}

/// A face that does not exist gives a refusal rather than silent nothing.
#[test]
fn a_missing_face_is_refused() {
    assert!(cube().thicken_face(999_999, 2.0, &[], &[]).is_none(), "a reference to a non-existent face has to give a refusal");
}

/// A curved face thickens too, which is what the tool exists for.
#[test]
fn a_curved_face_can_be_thickened_too() {
    let cyl = qymcad_kernel::Shape::cylinder(10.0, 20.0).expect("the cylinder");
    let bodies = cyl.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let (_, faces) = bodies.first().expect("the body");
    // the side wall, whose normal lies in the XY plane
    let side = faces.iter().find(|f| f.normal[2].abs() < 0.1).map(|f| f.id).expect("the side face");
    let shell = cyl.thicken_face(side, 2.0, &[], &[]).expect("the shell was built");
    assert!(shell.is_valid(), "the shell has to be a valid body");
    // the volume of a tube from Ø20 to Ø24 at a height of 20: π(12² − 10²)·20
    let want = std::f64::consts::PI * (12.0f64.powi(2) - 10.0f64.powi(2)) * 20.0;
    assert!((shell.volume() - want).abs() < want * 0.02, "the shell has to be a tube of about {want:.0}, and came out as {}", shell.volume());
}
