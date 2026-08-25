//! SPLIT BODY AS A TIMELINE FEATURE — parametric, not a one-off cut.
//!
//! The cutting plane is a REFERENCE (datum or world) plus an offset along the normal, as with a
//! mirror: the split must follow the geometry rather than cut at forgotten coordinates.
//!
//! A split is the only operation with SEVERAL OUTPUTS. The whole timeline had assumed one body per
//! node (`kind.body()`), so every piece but the first would disappear from visibility, export,
//! deletion and rollback. Hence this checks not only "the volume adds up" but that the timeline sees
//! ALL the pieces.
use qymcad_core::model::Project;

fn part_with_cube() -> (Project, u64) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    (p, body)
}

fn volume_of(p: &Project, b: u64) -> f64 {
    p.bodies.iter().find(|x| x.id == b).map(|x| x.mesh.volume()).unwrap_or(0.0)
}

/// A split down the middle gives two bodies of half the volume, and both live in the timeline.
#[test]
fn the_feature_builds_two_bodies_of_half_volume_each() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    assert_eq!(parts.len(), 2, "two pieces were asked for — ids for both must come back");
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the split must build: {:?}", rep.errors);

    for (i, &b) in parts.iter().enumerate() {
        let v = volume_of(&p, b);
        assert!((v - 4000.0).abs() < 1.0, "piece {i} must be half the box (4000), and it came out {v}");
    }
    // THE SOURCE BODY IS CONSUMED: otherwise the material would be on screen twice — the whole box
    // and both halves.
    assert!(p.consumed_bodies().contains(&body), "the source body must count as consumed by the split");
}

/// The pieces run BOTTOM TO TOP along the normal — the order must not depend on how OCCT walked the
/// shape.
#[test]
fn pieces_are_ordered_along_the_normal() {
    let (mut p, body) = part_with_cube();
    // cut OFF centre: the pieces differ in volume, so the volume tells which one is at the bottom
    let parts = p.add_split_body(body, 0, 0, 5.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);
    let (lo, hi) = (volume_of(&p, parts[0]), volume_of(&p, parts[1]));
    assert!((lo - 2000.0).abs() < 1.0, "the first piece is the BOTTOM one (20*20*5 = 2000), and it came out {lo}");
    assert!((hi - 6000.0).abs() < 1.0, "the second piece is the top one (20*20*15 = 6000), and it came out {hi}");
}

/// The plane is PARAMETRIC: edit the expression and the split moves, the pieces are recomputed.
#[test]
fn the_cutting_plane_is_parametric() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!((volume_of(&p, parts[0]) - 4000.0).abs() < 1.0, "setup: the split is down the middle");

    p.parameters.push(qymcad_core::model::Param { name: "z".into(), expr: "15".into(), value: 15.0 });
    p.set_feat_dim(parts[0], "offset", "z".into());
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == parts[0]) {
        n.dirty = true;
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild driven by the expression must pass: {:?}", rep.errors);
    let lo = volume_of(&p, parts[0]);
    assert!((lo - 6000.0).abs() < 1.0, "with \"z\"=15 the bottom piece must become 20*20*15=6000, and it came out {lo}");
}

/// The plane moved CLEAR OF the body — an honest node error, not pieces that quietly vanish.
#[test]
fn moving_the_plane_off_the_body_reports_an_error() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);

    p.set_feat_dim(parts[0], "offset", "100".into());
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == parts[0]) {
        n.dirty = true;
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(!rep.errors.is_empty(), "a plane clear of the body must raise an error, not a silent success");
    assert!(p.regen_errors.contains_key(&parts[0]), "the split node must be flagged with an error in the tree");
}

/// Deleting ONE piece removes the whole split: the pieces are the output of one operation, and a half
/// without a recipe would be a ghost body.
#[test]
fn deleting_one_piece_removes_the_whole_split() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);

    let gone = p.delete_body_cascade(parts[1]);
    assert!(gone.contains(&parts[0]) && gone.contains(&parts[1]), "both pieces must go together, and what went was {gone:?}");
    assert!(!p.timeline.iter().any(|n| n.id == parts[0]), "the split node must leave the timeline");
    // the source body itself stays — the split did not touch its node
    assert!(p.timeline.iter().any(|n| n.kind.bodies().contains(&body)), "the source body must stay in the timeline");
}

/// SUPPRESSING the split brings back the whole body instead of leaving the halves beside it.
#[test]
fn suppressing_the_split_brings_the_whole_body_back() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!(p.consumed_bodies().contains(&body), "setup: while the split is alive the source is consumed");

    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == parts[0]) {
        n.suppressed = true;
        n.dirty = true;
    }
    let _ = qymcad_testkit::regenerate(&mut p);
    for (i, &b) in parts.iter().enumerate() {
        assert!(volume_of(&p, b) < 1e-6, "under suppression piece {i} must disappear, and its volume is {}", volume_of(&p, b));
    }
    assert!((volume_of(&p, body) - 8000.0).abs() < 1.0, "a suppressed split must bring back the WHOLE box, 8000, and it came out {}", volume_of(&p, body));
}

/// The pieces are full bodies of the part: they appear in the component's body list and in exports.
#[test]
fn both_pieces_belong_to_the_part_and_are_exportable() {
    let (mut p, body) = part_with_cube();
    let parts = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);
    let owner = p.body_owner(parts[0]).expect("owner of the piece");
    let of_part = p.component_bodies(owner);
    for (i, b) in parts.iter().enumerate() {
        assert!(of_part.contains(b), "piece {i} must count as a body of the part");
        assert_eq!(p.body_owner(*b), Some(owner), "both pieces belong to ONE part");
    }
    // what goes to export is the pieces, not the consumed source
    let consumed = p.consumed_bodies();
    assert!(parts.iter().all(|b| !consumed.contains(b)), "the pieces are not consumed — they are the result");
    assert!(consumed.contains(&body), "the source is consumed");
}

/// ASSOCIATIVITY: a split by a datum-from-face follows the face instead of cutting at forgotten
/// numbers.
///
/// That is why the plane is stored as a reference: with bare origin/normal in the node, any edit of
/// the body higher up the timeline left the split where it was.
#[test]
fn the_cut_follows_the_face_it_was_taken_from() {
    let (mut p, body) = part_with_cube();
    // a datum plane from the BOTTOM face of the box: its normal points DOWN, so "up" is an offset of -6
    let f = p
        .regen_faces
        .get(&body)
        .and_then(|fs| fs.iter().find(|f| f.normal[2] < -0.9))
        .cloned()
        .expect("there is a bottom face");
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let datum = p.add_plane_from_face(body, key, -6.0);
    let _ = qymcad_testkit::regenerate(&mut p);

    let parts = p.add_split_body(body, 0, datum, 0.0, 2);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a split by a datum must build: {:?}", rep.errors);
    // the bottom face normal points DOWN, so "first along it" is the piece that is UPPER in Z (14 mm)
    let a = volume_of(&p, parts[0]) + volume_of(&p, parts[1]);
    assert!((a - 8000.0).abs() < 1.0, "no material disappears: {a}");
    let thin = volume_of(&p, parts[0]).min(volume_of(&p, parts[1]));
    assert!((thin - 2400.0).abs() < 1.0, "an offset of 6 from the bottom gives a piece 20*20*6=2400, and it came out {thin}");

    // MOVE THE DATUM — the split must follow it
    if let Some(pl) = p.planes.iter_mut().find(|x| x.id == datum) {
        if let qymcad_core::model::PlaneDef::OffsetFace { dist, .. } = &mut pl.def {
            *dist = -15.0;
        }
    }
    // the split node is NOT touched by hand: the datum is its input and the rebuild must arrive by itself
    p.mark_node_dirty(datum);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild following the datum must pass: {:?}", rep.errors);
    let thin = volume_of(&p, parts[0]).min(volume_of(&p, parts[1]));
    assert!((thin - 2000.0).abs() < 1.0, "the datum moved to -15, so the thin piece became 20*20*5=2000, and it came out {thin}");
}

/// The cutting plane was DELETED — the split fails loudly instead of quietly cutting elsewhere.
///
/// A mirror in that situation falls back to a world plane, and for a mirror that is reasonable: a
/// mirror about another plane is still a mirror. A split by another plane breaks the part in a
/// different place, and "degrading" there means silently substituting the result.
#[test]
fn deleting_the_cutting_plane_makes_the_split_fail_loudly() {
    let (mut p, body) = part_with_cube();
    let f = p.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[2] < -0.9)).cloned().expect("bottom face");
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let datum = p.add_plane_from_face(body, key, -6.0);
    let parts = p.add_split_body(body, 0, datum, 0.0, 2);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: the split built");

    p.delete_plane(datum);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(!rep.errors.is_empty(), "deleting the cutting plane must be noticed");
    // THE ERROR IS TOLD APART BY ITS CODE, not by a substring. A `msg.contains(...)` check here went
    // silently blind to any rewording, translation included.
    let err = p.regen_errors.get(&parts[0]).cloned();
    assert!(
        matches!(err, Some(qymcad_core::errors::CoreError::CutPlaneDeleted)),
        "the node must say EXACTLY that the cutting plane was deleted, and it said: {err:?}"
    );
}
