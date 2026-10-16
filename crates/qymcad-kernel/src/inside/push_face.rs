//! Pushing and pulling a face: direct modelling.
//!
//! Until this existed, a body could only be built from a sketch or a primitive: the feature vocabulary held no
//! operation on a face at all. That was the largest gap against a professional CAD, where grabbing a face and
//! pulling it is an ordinary way to work.

/// A 20 mm cube whose top face moves up by 5: the volume grows by exactly 20·20·5.
#[test]
fn pulling_a_face_outwards_adds_exactly_that_much_material() {
    let cube = crate::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube was built");
    let v0 = cube.volume();
    assert!((v0 - 8000.0).abs() < 1e-6, "setup: the volume of the cube is 8000 and came out as {v0}");

    // the top face is the one whose axis points up; its persistent id is taken
    let top = top_face_id(&cube).expect("the top face was found");
    let pulled = cube.push_face(top, 5.0).expect("the face pulled outwards");
    let v1 = pulled.volume();
    assert!((v1 - (v0 + 20.0 * 20.0 * 5.0)).abs() < 1e-3, "pulling the face by 5 has to grow the volume by 2000: it was {v0} and became {v1}");
    assert!(pulled.is_valid(), "the body has to stay valid");
}

/// The same face pushed in by 5: the volume falls by exactly as much.
#[test]
fn pushing_a_face_inwards_removes_exactly_that_much_material() {
    let cube = crate::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube was built");
    let v0 = cube.volume();
    let top = top_face_id(&cube).expect("the top face was found");
    let pushed = cube.push_face(top, -5.0).expect("the face pushed in");
    let v1 = pushed.volume();
    assert!((v1 - (v0 - 20.0 * 20.0 * 5.0)).abs() < 1e-3, "pushing the face in by 5 has to reduce the volume by 2000: it was {v0} and became {v1}");
    assert!(pushed.is_valid(), "the body has to stay valid");
}

/// The side of a cylinder between its flat ends moves as an offset of itself: 2 out makes the radius 12, the volume
/// pi x 12^2 x 20. A prism along one direction would be no offset of a cylinder, so it is never used for one.
#[test]
fn the_side_of_a_cylinder_is_offset() {
    let cyl = crate::Shape::cylinder(10.0, 20.0).expect("the cylinder was built");
    // the side face of a cylinder: the one that has an axis, since `face_axis` answers only for cylindrical faces
    let side = (1u32..64).find(|&id| cyl.face_axis(id).is_some()).expect("the side of the cylinder");
    let pushed = cyl.push_face(side, 2.0).expect("the side of the cylinder pushed out");
    let want = std::f64::consts::PI * 144.0 * 20.0;
    assert!((pushed.volume() - want).abs() < 0.5, "the side pushed 2 out: {:.1}, a cylinder of radius 12 is {want:.1}", pushed.volume());
    assert!(pushed.is_valid(), "the body has to stay valid");
}

/// The persistent id of the top face: the planar one whose centre has the greatest Z.
fn top_face_id(s: &crate::Shape) -> Option<u32> {
    let bodies = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let qymcad_core::geom::Built { faces, .. } = bodies.first()?;
    let mut best: Option<(f64, u32)> = None;
    for f in faces {
        if f.normal[2] > 0.9 {
            let z = f.centroid.z;
            if best.is_none_or(|(bz, _)| z > bz) {
                best = Some((z, f.id));
            }
        }
    }
    best.map(|(_, id)| id)
}
