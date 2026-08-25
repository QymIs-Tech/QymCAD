//! NEITHER A BODY NOR A REFERENCE TO IT OUTLIVES ITS NODE.
//!
//! The invariant is simple: every body counted as live in `regen_faces` must be produced by some
//! timeline node. A body without a node is a ghost: nothing builds it, yet everything that walks the
//! live bodies counts it — from the document tree to machine output. The kernel states the symptom
//! plainly: bodies appear in the tree that nobody made.
//!
//! The measurement covered EVERY kind of deletion. Plane, body cascade and feature deletion came out
//! clean; two places left a ghost behind:
//!
//! ```text
//! deleting a SKETCH:    timeline 4 -> 2 nodes, the loft node None, yet the body live with its 3 faces
//! deleting a COMPONENT: body 14 stayed in the regen cache with no node at all
//! ```
//!
//! Both are fixed by the same helper, `drop_orphan_bodies`, written for exactly this trouble — it
//! simply was not called. The check here stands on all four paths at once, so that whoever adds the
//! next kind of deletion hears about it from a test rather than from a bug report.
use qymcad_core::feature::{AnchorRef, FaceKey, JointKind, SketchPlane};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project, WorkPlane};

/// Bodies listed in the regen cache that no timeline node produces.
fn ghosts(p: &Project) -> Vec<Id> {
    let produced: std::collections::HashSet<Id> = p.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
    let mut v: Vec<Id> = p.regen_faces.keys().copied().filter(|b| !produced.contains(b)).collect();
    v.sort_unstable();
    v
}

fn brick(p: &mut Project, name: &str, w: f64, h: f64, up: f64) -> Id {
    let sid = p.add_line_sketch(
        name,
        vec![Point2::new(0.0, 0.0), Point2::new(w, 0.0), Point2::new(w, h), Point2::new(0.0, h)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, name);
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, up, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(p);
    b
}

#[test]
fn deleting_a_sketch_leaves_no_ghost() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "sq",
        vec![Point2::new(0.0, 0.0), Point2::new(12.0, 0.0), Point2::new(12.0, 9.0), Point2::new(0.0, 9.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "sq");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 5.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.contains_key(&body), "the body was built");
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts before the deletion");
    p.delete_sketch(sid);
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts after deleting the sketch");
}

#[test]
fn deleting_a_component_leaves_no_ghost() {
    let mut p = Project::default();
    p.new_document();
    let comp = p.add_component("Part");
    p.set_active_component(Some(comp));
    let body = brick(&mut p, "Base", 16.0, 12.0, 6.0);
    assert!(p.regen_faces.contains_key(&body), "the body was built");
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts before the deletion");
    p.delete_component(comp);
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts after deleting the component");
}

#[test]
fn deleting_a_plane_leaves_no_ghost() {
    let mut p = Project::default();
    p.new_document();
    let pl = p.add_plane(WorkPlane {
        id: 0,
        name: "z5".into(),
        origin: [0.0, 0.0, 5.0],
        normal: [0.0, 0.0, 1.0],
        rot_deg: 0.0,
        def: Default::default(),
    });
    let si = p.new_sketch("on the plane");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(sid, "on the plane");
    p.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("contour");
    let body = p.add_extrude_multi(sid, vec![c], 6.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    assert!(p.regen_faces.contains_key(&body), "the body was built");
    p.delete_plane(pl);
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts after deleting the plane");
}

#[test]
fn deleting_a_body_and_a_feature_leaves_no_ghost() {
    let mut p = Project::default();
    p.new_document();
    let body = brick(&mut p, "Base", 20.0, 14.0, 8.0);
    let fil = p.add_fillet(body, 1.0, Vec::<u32>::new());
    qymcad_testkit::regenerate(&mut p);
    let node = p.timeline.iter().find(|n| n.kind.bodies().contains(&fil)).map(|n| n.id).expect("the fillet node");
    p.delete_feature_op(node);
    assert_eq!(ghosts(&p), Vec::<Id>::new(), "no ghosts after deleting the feature");

    let mut q = Project::default();
    q.new_document();
    let b2 = brick(&mut q, "Base", 20.0, 14.0, 8.0);
    q.add_fillet(b2, 1.0, Vec::<u32>::new());
    qymcad_testkit::regenerate(&mut q);
    q.delete_body_cascade(b2);
    assert_eq!(ghosts(&q), Vec::<Id>::new(), "no ghosts after the cascading body deletion");
}

/// Connectors whose anchor points at a body no node produces any more.
fn stale_connectors(p: &Project) -> Vec<Id> {
    let live: std::collections::HashSet<Id> = p.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
    let mut v: Vec<Id> = p
        .connectors
        .iter()
        .filter(|c| match &c.anchor {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => !live.contains(b),
            _ => false,
        })
        .map(|c| c.id)
        .collect();
    v.sort_unstable();
    v
}

fn key_of(f: &qymcad_core::geom::MeshFace) -> FaceKey {
    FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
}

/// A CONNECTOR DOES NOT OUTLIVE ITS BODY.
///
/// The anchor points at a face of a specific body. Once the body is gone the anchor resolves against
/// the OLD centroid imprint and the solver places parts at garbage coordinates — the kernel describes
/// the result as the assembly flying apart. Component deletion had disposed of such connectors for a
/// long time; the body cascade and sketch deletion had not: the measurement showed a dangling
/// connector and a SURVIVING joint on both paths (2 connectors and 1 joint left after the body was
/// removed).
#[test]
fn a_connector_does_not_outlive_its_body_after_a_cascade() {
    let mut p = Project::default();
    p.new_document();
    let comp = p.add_component("Part");
    p.set_active_component(Some(comp));
    let a = brick(&mut p, "First", 20.0, 14.0, 8.0);
    let ka = p.regen_faces[&a].iter().next().map(key_of).expect("a face of the first body");
    let ca = p.add_connector(comp, AnchorRef::FaceCenter(a, ka));
    let b = brick(&mut p, "Second", 10.0, 10.0, 5.0);
    let kb = p.regen_faces[&b].iter().next().map(key_of).expect("a face of the second body");
    let cb = p.add_connector(comp, AnchorRef::FaceCenter(b, kb));
    p.add_joint(ca, cb, JointKind::Rigid);
    assert_eq!(p.connectors.len(), 2, "two connectors");
    assert_eq!(p.joints.len(), 1, "one joint");
    assert_eq!(stale_connectors(&p), Vec::<Id>::new(), "nothing dangling before the deletion");

    p.delete_body_cascade(a);
    // THE CONTRACT CHANGED, AND THAT IS NOT A RELAXATION.
    //
    // The rule here used to be "a joint with a broken anchor must go along with the connector":
    // deleting a single body silently threw away assembly work. The motive was honest — a garbage
    // frame from the old centroid imprint — but it treated the symptom. Now the document REMEMBERS
    // the deleted body, an anchor on it honestly fails to resolve, and the joint stays VISIBLE and
    // FLAGGED: repairing or deleting it is a decision for the person.
    //
    // The requirement about the garbage frame is unchanged and is checked right here: the frame does
    // not resolve.
    assert_eq!(p.joints.len(), 1, "the joint vanished with the body — assembly work was lost silently");
    assert_eq!(p.connectors.len(), 2, "the connector was deleted — then there is nothing left to repair the joint with");
    assert!(p.connector_matrix(ca).is_none(), "an anchor frame on a deleted body resolves — that is the garbage frame");
    assert!(p.connector_matrix(cb).is_some(), "the anchor of a live body stopped resolving");
    let faults = p.joint_faults();
    assert_eq!(faults.len(), 1, "the joint on the deleted body is not reported as faulty: {faults:?}");
}

/// THE SAME, WHEN THE BODY IS REMOVED BY A CASCADE FROM THE SKETCH.
#[test]
fn a_connector_does_not_outlive_its_body_after_a_sketch_delete() {
    let mut p = Project::default();
    p.new_document();
    let comp = p.add_component("Part");
    p.set_active_component(Some(comp));
    let sid = p.add_line_sketch(
        "sq",
        vec![Point2::new(0.0, 0.0), Point2::new(12.0, 0.0), Point2::new(12.0, 9.0), Point2::new(0.0, 9.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    p.add_sketch_node(sid, "sq");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 5.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    let k = p.regen_faces[&body].iter().next().map(key_of).expect("a face of the body");
    p.add_connector(comp, AnchorRef::FaceCenter(body, k));
    assert_eq!(p.connectors.len(), 1, "the connector was placed");

    p.delete_sketch(sid);
    // Same new contract: the connector stays, but a frame on the deleted body does not resolve —
    // there is nowhere for a garbage frame to come from, and the person's work is intact.
    assert_eq!(p.connectors.len(), 1, "the connector was removed — a joint on it would have nothing left to repair");
    let c = p.connectors[0].id;
    assert!(p.connector_matrix(c).is_none(), "an anchor frame on a deleted body resolves — that is a garbage frame");
}

/// A SKETCH ON A DELETED FACE FREEZES IN PLACE INSTEAD OF HANGING ON THE `Face` OF A BODY THAT IS GONE.
///
/// Measured before the fix: a sketch placed on a face of its OWN body survived the removal of that
/// body and stayed `SketchPlane::Face(dead body, ...)`. The geometry did not move — the frame
/// resolved from the old imprint — which is what makes the trouble quiet: the sketch looks bound to
/// a face that does not exist, and every later rebuild silently takes a stale imprint.
///
/// For the neighbouring case — a sketch on a face of ANOTHER part — the freeze was already done via
/// `break_external_ref`. The same move applies here, only there is no external-reference record.
#[test]
fn a_sketch_on_a_deleted_face_freezes_in_place() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "sq",
        vec![Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 14.0), Point2::new(0.0, 14.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "sq");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 8.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    let top = p.regen_faces[&body]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .cloned()
        .expect("top face");
    let s2 = p.new_sketch("on the face");
    let sid2 = p.sketches[s2].id;
    p.sketches[s2].plane = SketchPlane::Face(body, key_of(&top));
    p.add_sketch_node(sid2, "on the face");
    p.add_circle_entity(s2, 0.0, 0.0, 6.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s2);
    let before = p.sketch_frame(s2).expect("frame of the sketch on the face").origin;

    p.delete_body_cascade(body);

    let i = p.sketch_index(sid2).expect("the sketch survived the body removal — it is not doomed by itself");
    assert!(
        matches!(p.sketches[i].plane, SketchPlane::Datum(_)),
        "the sketch must freeze into a datum imprint instead of hanging on a face of a body that is gone"
    );
    let after = p.sketch_frame(i).expect("frame after the removal").origin;
    for k in 0..3 {
        assert!(
            (after[k] - before[k]).abs() < 1e-9,
            "the freeze moved the sketch: {before:?} -> {after:?}"
        );
    }
}

/// THE FREEZE SURVIVES A SAVE. A fix that lives only until the file is written is no fix: close the
/// document, open it, and the sketch hangs on a face of a body that is gone again. Measured: after a
/// save-and-open round trip the plane stays a datum, the frame is the same, and there are no ghosts.
#[test]
fn the_freeze_survives_a_save_and_reload() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "sq",
        vec![Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 14.0), Point2::new(0.0, 14.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "sq");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 8.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    // A SECOND BODY THAT OUTLIVES THE FIRST. Without it the document holds no body at all after the
    // deletion, and the "no ghosts" check passes by itself while checking nothing: an empty cache is
    // empty with or without any cleanup. The neighbour makes the question real.
    let neighbour = brick(&mut p, "neighbour", 9.0, 7.0, 4.0);
    let top = p.regen_faces[&body]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .cloned()
        .expect("top face");
    let s2 = p.new_sketch("on the top");
    let sid2 = p.sketches[s2].id;
    p.sketches[s2].plane = SketchPlane::Face(body, key_of(&top));
    p.add_sketch_node(sid2, "on the top");
    p.add_circle_entity(s2, 0.0, 0.0, 4.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s2);
    p.delete_body_cascade(body);
    let before = p.sketch_frame(p.sketch_index(sid2).expect("the sketch is alive")).expect("frame after the freeze").origin;

    let ron = qymcad_core::model::to_ron(&p).expect("the document was written");
    let back = qymcad_core::model::from_ron(&ron).expect("the document was read back");

    let j = back.sketch_index(sid2).expect("the sketch survived the save");
    assert!(matches!(back.sketches[j].plane, SketchPlane::Datum(_)), "after reopening the sketch must stay frozen");
    let after = back.sketch_frame(j).expect("frame after reopening").origin;
    for k in 0..3 {
        assert!((after[k] - before[k]).abs() < 1e-9, "the save moved the frozen sketch: {before:?} -> {after:?}");
    }
    // ASK ABOUT GHOSTS AFTER A REBUILD, NOT IMMEDIATELY. `regen_faces` is derived data and is not
    // written to RON: a freshly read document has an EMPTY cache, and the "no ghosts" check would
    // pass by itself without checking anything. Rebuild first, then ask for real.
    let mut back = back;
    for n in back.timeline.iter_mut() {
        n.dirty = true;
    }
    qymcad_testkit::regenerate(&mut back);
    let produced: std::collections::HashSet<Id> = back.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
    let after_ghosts: Vec<Id> = back.regen_faces.keys().copied().filter(|b| !produced.contains(b)).collect();
    assert_eq!(after_ghosts, Vec::<Id>::new(), "no ghosts after reopening and rebuilding");
    assert!(
        back.regen_faces.contains_key(&neighbour),
        "the neighbour must survive both the removal of the first body and the save — otherwise the ghost check is vacuous"
    );
}
