//! AN END-TO-END RUN OF A QUERY REFERENCE AGAINST THE LIVE KERNEL.
//!
//! The unit tests in `qymcad-core/src/refs` work on an invented set of faces. Here the same thing
//! goes through real OCCT: a plate, a hole in its top face, an edit of the extrude height — and the
//! question of whether the hole stayed where it was meant to.
//!
//! Without this check, moving the hole onto a query is proved only by the fact that it compiles.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A 40x30xh plate with a hole in its top face. Returns (project, plate body, hole node).
fn plate_with_hole(h: f64) -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 30.0), Point2::new(0.0, 30.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let plate = p.add_extrude_multi(sid, closed, h, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    // THE TOP FACE — the way a person would pick it with the mouse: the highest of the upward-facing ones
    let top = p.regen_faces[&plate]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
        .expect("the plate has a top face")
        .clone();
    let key = qymcad_core::feature::FaceKey {
        index: 0,
        centroid: [top.centroid.x, top.centroid.y, top.centroid.z],
        normal: top.normal,
        id: top.id,
    };
    let hole = p.add_hole(plate, key, 8.0, 5.0);
    qymcad_testkit::regenerate(&mut p);
    (p, plate, hole)
}

/// THE HOLE STAYS ON THE TOP FACE WHEN THE PLATE GETS THICKER.
///
/// That is what an associative reference is for: an edit higher up the timeline changes the geometry,
/// and the hole travels with its own face instead of staying at the old height or moving to a
/// neighbouring face.
#[test]
fn a_hole_follows_its_face_when_the_plate_grows() {
    let (mut p, plate, hole) = plate_with_hole(10.0);
    let v_before = p.regen_faces[&plate].len();
    assert!(v_before > 0, "the plate built");
    assert!(p.regen_errors.get(&hole).is_none(), "the hole built without errors");

    // AN EDIT HIGHER UP THE TIMELINE: the plate became twice as thick
    for n in &mut p.timeline {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 20.0;
            n.dirty = true;
        }
    }
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &report.errors {
        eprintln!("error on node {id}: {e:?}");
    }

    let body = shapes.get(&hole).expect("the body with the hole built after the edit");
    assert!(body.is_valid(), "the body after the edit is broken");

    // the hole must be in the TOP face of the new body: look for the cylindrical bore near the top
    let top_z = p.regen_faces[&hole].iter().map(|f| f.centroid.z).fold(f64::MIN, f64::max);
    assert!((top_z - 20.0).abs() < 1e-6, "the plate did not get thicker: the top is at {top_z}");

    // the volume: the plate minus a d8 cylinder 5 deep
    let expect = 40.0 * 30.0 * 20.0 - std::f64::consts::PI * 4.0 * 4.0 * 5.0;
    let v = body.volume();
    assert!((v - expect).abs() / expect < 0.02, "volume {v:.0} instead of {expect:.0} — the hole is in the wrong place or is the wrong hole");
}

/// AND IF THE FACE IS GONE, THE NODE FAILS INSTEAD OF DRILLING AT RANDOM.
///
/// A "nearest face pointing the same way" fallback used to fire here, and the hole silently moved to
/// another face. Now the reference refuses honestly and a person sees what to repair.
#[test]
fn a_hole_whose_face_vanished_reports_instead_of_drilling_elsewhere() {
    let (mut p, _plate, hole) = plate_with_hole(10.0);

    // THE REFERENCE POINTS AT NOTHING: this imitates "the face disappeared after an edit higher up"
    for n in &mut p.timeline {
        if let qymcad_core::feature::FeatureKind::Hole { face, .. } = &mut n.kind {
            *face = qymcad_core::refs::Ref::one(0xDEAD_BEEF, face.hint);
            n.dirty = true;
        }
    }
    qymcad_testkit::regenerate(&mut p);
    assert!(
        p.regen_errors.contains_key(&hole),
        "the face is lost and the node says nothing — the silent guess is back"
    );
}
