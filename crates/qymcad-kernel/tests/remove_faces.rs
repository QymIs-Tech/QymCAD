//! Removing a face and healing the body.
//!
//! This is how a fillet or a chamfer is taken off without unpicking the timeline: the face goes, its neighbours
//! extend, and the body stays closed.

/// Removing a hole is the canonical case: the cylindrical face goes and the material returns.
///
/// The algorithm removes a whole element rather than an arbitrary piece of surface. A hole is exactly that: one
/// cylindrical face forms it, and the neighbours have somewhere to extend to.
#[test]
fn removing_a_hole_restores_the_material() {
    let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube");
    let v0 = cube.volume();
    let pl = [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0, 20.0];
    let drilled = cube.hole_stepped(0, pl, 6.0, 25.0, 0.0, 0.0, &[]).expect("the hole was drilled");
    let v1 = drilled.volume();
    assert!(v1 < v0 - 100.0, "setup: the hole has to remove material ({v0} -> {v1})");

    let bore = (1u32..200).find(|&id| drilled.face_axis(id).is_some()).expect("the cylindrical face of the hole");
    let healed = drilled.remove_faces(&[bore]).expect("the hole was removed and healed");
    let v2 = healed.volume();
    assert!(healed.is_valid(), "the healed body has to stay valid");
    assert!(
        (v2 - v0).abs() < 1.0,
        "with the hole removed the volume has to return to {v0}, and came out as {v2}"
    );
}

/// When a face cannot be removed, the answer is a refusal rather than a quiet "nothing changed".
///
/// The kernel can report success without having removed a single face. Without a check the feature would appear
/// to work: a step appears in the timeline while the part is unchanged — the worst kind of failure, because it
/// looks like work.
#[test]
fn an_impossible_removal_is_refused_not_silently_ignored() {
    let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube");
    let rounded = cube.fillet_all(3.0).expect("the fillet was built");
    let all_round: Vec<u32> = (1u32..200).filter(|&id| rounded.face_axis(id).is_some()).collect();
    // The fillet strips are bounded by spherical corners: they cannot be removed while the spheres remain,
    // since there is nothing for the neighbours to extend through. The algorithm removes a whole element rather
    // than an arbitrary piece of surface.
    assert!(
        rounded.remove_faces(&all_round).is_none(),
        "an impossible removal has to refuse rather than return the same body as a success"
    );
}

/// A face that does not exist gives a refusal rather than a damaged body.
#[test]
fn an_unknown_face_is_refused() {
    let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("the cube");
    assert!(cube.remove_faces(&[999_999]).is_none(), "an unknown face has to give a refusal");
    assert!(cube.remove_faces(&[]).is_none(), "an empty list is a refusal");
}

/// The same case as in the application: a 20×20×10 plate with a Ø5 through hole.
#[test]
fn removing_a_through_hole_in_a_plate() {
    let plate = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 10.0).expect("the plate");
    let v0 = plate.volume();
    let pl = [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0, 10.0];
    let drilled = plate.hole_stepped(0, pl, 5.0, 20.0, 0.0, 0.0, &[]).expect("the through hole");
    let ids: Vec<u32> = drilled
        .tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k())
        .first()
        .map(|(_, fs)| fs.iter().filter(|f| f.normal[2].abs() < 0.3 && (f.centroid.x - 10.0).abs() < 4.0).map(|f| f.id).collect())
        .unwrap_or_default();

    let healed = drilled.remove_faces(&ids).expect("a through hole in a plate has to be removable");
    assert!((healed.volume() - v0).abs() < 1.0, "the material has to return to {v0}");
}
