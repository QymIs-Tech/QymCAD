//! The identity of geometry follows a recipe rather than a resemblance.
//!
//! The rule was tightened twice. First, the silent search for a similar face on every rebuild was replaced by a
//! single visible rebinding recorded in the report. Now, for the features moved onto query references, there is
//! no guessing at all: a face is found by recipe, and if it is not found the node goes red with a reason. The
//! remaining features still live by the earlier rule, and the tests for it are here too.
//!
//! An unknown id used to trigger a silent search for a similar face — a co-directed normal and the nearest
//! centroid — repeated afresh on every rebuild: the reference was found, but it could be the wrong one, and
//! nothing said so. Resolution now goes by id, lost references are repaired by a single pass before the build,
//! the id found is written into the key so the guess happens once rather than every time, and the fact itself
//! reaches the report.
use qymcad_core::feature::{FaceKey, FeatureKind, RegenReport};
use qymcad_core::geom::{MeshFace, Point3};
use qymcad_core::model::Project;

fn face(id: u32, c: [f64; 3], n: [f64; 3]) -> MeshFace {
    MeshFace { triangles: vec![], normal: n, centroid: Point3::new(c[0], c[1], c[2]), area: 1.0, id }
}

/// Resolution goes by id: faces identical in geometry are not confused with one another.
#[test]
fn face_is_resolved_by_persistent_id_only() {
    let mut p = Project::default();
    p.new_document();
    let body = 77;
    p.regen_faces.insert(body, vec![face(10, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), face(20, [9.0, 9.0, 5.0], [0.0, 0.0, 1.0])]);

    // a key with id = 20 but the fingerprint of another face: the id has to win
    let key = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 20 };
    assert_eq!(p.resolve_face(body, &key).0, [9.0, 9.0, 5.0], "the face is taken by id rather than by a similar fingerprint");
}

/// A reference with an unknown id is no longer repaired silently on every resolution: the fingerprint is
/// returned as it is.
#[test]
fn unknown_id_does_not_silently_snap_to_a_lookalike() {
    let mut p = Project::default();
    p.new_document();
    let body = 77;
    p.regen_faces.insert(body, vec![face(10, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0])]);
    let key = FaceKey { index: 0, centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], id: 999 };
    assert_eq!(p.resolve_face(body, &key).0, [1.0, 2.0, 3.0], "there is no silent substitution: the fingerprint of the reference itself is returned");
}

/// Rebinding: a lost id is repaired by a single pass, the new id is written into the key, and the fact reaches
/// A hole is no longer rebound by resemblance: it refuses.
///
/// An earlier version of this test pinned down the opposite: a lost hole reference was repaired with a similar
/// face and the fact reached the report. That was better than a silent search on every rebuild, but still a
/// guess — merely a single, recorded one.
///
/// With query references the rule is stricter: a face is found by recipe rather than by resemblance. If it is
/// not found, the node goes red with a reason and the choice of what to attach to is made by hand. There is no
/// guessing at all.
#[test]
fn a_hole_with_a_lost_face_refuses_instead_of_being_rebound() {
    let mut p = Project::default();
    p.new_document();
    let body = p.add_mesh(qymcad_core::geom::Mesh::default());
    p.regen_faces.insert(body, vec![face(42, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0])]);
    let node = p.add_hole(body, FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 777 }, 4.0, 10.0);

    let mut rep = RegenReport::default();
    p.rebind_lost_face_refs_for_test(&mut rep);
    assert!(rep.rebinds.iter().all(|r| r.node != node), "a hole reference is no longer matched by resemblance: {:?}", rep.rebinds);

    // and it refuses honestly, naming what was sought
    let r = p.timeline.iter().find(|n| n.id == node).and_then(|n| match n.kind {
        FeatureKind::Hole { ref face, .. } => Some(face.clone()),
        _ => None,
    }).expect("the hole node is present");
    match p.resolve_face_ref(body, &r, "ref-what-hole-face") {
        Err(e) => assert_eq!(e.key(), "ref-lost", "a refusal has to be named by its kind rather than left empty"),
        Ok(c) => panic!("the reference matched face {} instead of refusing: the silent guess is back", c.desc),
    }
}
