//! DELETING A NODE DOES NOT BREAK NEIGHBOURS THAT DO NOT TOUCH IT.
//!
//! Reported behaviour: deleting a fillet that stood above a chamfer in the list made the chamfer fail
//! and disappear from the body too, even though the two are on opposite sides and do not touch each
//! other.
//!
//! They do not touch. What broke the chamfer was not the geometry but the NAMES: the consumer of a
//! deleted node is moved onto the source body, while its references keep naming faces and edges of
//! the DELETED body. An edge name is derived from the pair of its faces; the source has different
//! faces, so the name is different too, even though the edge is the same one in the same place.
//! Nobody translated the names during that move.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::Ref;

/// A 60x40x12 plate in a part. Returns (project, body).
fn plate() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 12.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    (p, body)
}

/// The top edge of a body at the given y (the front or the back long edge).
fn top_edge_at_y(p: &Project, body: u64, y: f64) -> u32 {
    p.regen_edges[&body]
        .iter()
        .find(|e| (e.a[2] - 12.0).abs() < 1e-6 && (e.b[2] - 12.0).abs() < 1e-6 && (e.mid[1] - y).abs() < 1e-6)
        .map(|e| e.id)
        .expect("the top edge")
}

/// THE REPORTED CASE: a fillet at the front, a chamfer at the back; delete the fillet and the chamfer
/// must survive.
#[test]
fn deleting_a_fillet_does_not_break_a_chamfer_on_the_other_side() {
    let (mut p, body) = plate();
    let front = top_edge_at_y(&p, body, 0.0);
    let fil = p.add_fillet_ref(body, 2.0, Ref::picks(&[front]));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the fillet: {:?}", rep.errors);

    let back = top_edge_at_y(&p, fil, 40.0);
    let cha = p.add_chamfer_ref(fil, 2.0, Ref::picks(&[back]));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the chamfer: {:?}", rep.errors);
    let v_both = p.bodies.iter().find(|b| b.id == cha).expect("the body").mesh.volume();

    // DELETE THE FILLET — the one ABOVE the chamfer in the timeline
    p.delete_feature_op(fil);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the chamfer on the OPPOSITE side must survive: {:?}", rep.errors);

    // and it really is still a chamfer: exactly its own material is gone, not a fillet plus a chamfer
    let v_after = p.bodies.iter().find(|b| b.id == cha).expect("the chamfer body").mesh.volume();
    let plate_v = 60.0 * 40.0 * 12.0;
    assert!(v_after < plate_v - 1.0, "the chamfer must remove material: {v_after:.2} against the plate {plate_v:.2}");
    assert!(v_after > v_both + 1.0, "the fillet is gone, so there must be MORE material than with both: {v_after:.2} against {v_both:.2}");
}

/// A SKETCH ON A FACE MOVES ACROSS TOGETHER WITH THE FACE NAME.
///
/// The same slip one storey up: a face anchor stores not only the body id but also the FACE NAME.
/// Change one and leave the other and the anchor points at nothing, and a cut driven by such a sketch
/// silently stops cutting.
#[test]
fn a_sketch_on_a_face_survives_deleting_the_feature_under_it() {
    use qymcad_core::feature::{FaceKey, SketchPlane};
    let (mut p, body) = plate();
    let front = top_edge_at_y(&p, body, 0.0);
    let fil = p.add_fillet_ref(body, 2.0, Ref::picks(&[front]));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the fillet: {:?}", rep.errors);

    // a sketch on the TOP face of the filleted body, and a cut driven by it
    let top = p.regen_faces[&fil].iter().find(|f| f.normal[2] > 0.9).cloned().expect("the top face");
    let si = p.new_sketch("pocket");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Face(fil, FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id });
    p.add_sketch_node(sid, "pocket");
    p.add_rect_entity(si, 20.0, 15.0, 40.0, 25.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the pocket contour");
    let cut = p.add_combine_multi_op(fil, sid, vec![cid], 4.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, 0.0, vec![]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — the pocket: {:?}", rep.errors);
    let v_cut = p.bodies.iter().find(|b| b.id == cut).expect("the body with the pocket").mesh.volume();

    p.delete_feature_op(fil);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the pocket must survive the deletion of the fillet under it: {:?}", rep.errors);

    // and it REALLY cuts: without translating the face name the anchor dangles and the cut builds nothing
    let v_after = p.bodies.iter().find(|b| b.id == cut).expect("the pocket body").mesh.volume();
    let plate_v = 60.0 * 40.0 * 12.0;
    assert!(v_after < plate_v - 100.0, "the pocket must stay cut: {v_after:.2} against a whole plate {plate_v:.2}");
    assert!(v_after > v_cut, "the fillet is gone, so there must be more material: {v_after:.2} against {v_cut:.2}");
    match p.sketches.iter().find(|s| s.id == sid).map(|s| &s.plane) {
        Some(SketchPlane::Face(b, k)) => {
            assert_eq!(*b, body, "the sketch must land on the source body");
            assert!(p.regen_faces[&body].iter().any(|f| f.id == k.id), "and name the face by ITS name rather than by the name of the body that is gone ({:#x})", k.id);
        }
        other => panic!("the sketch plane must stay a face, and it became {other:?}"),
    }
}

/// AND THE REVERSE: a deletion must NOT silently move a reference onto something "similar".
///
/// The translation follows a coincidence of PLACE. Geometry the source body does not have (it was
/// created by the deleted fillet) has nothing to be translated into — and such a reference must fail
/// honestly rather than settle on a neighbour.
#[test]
fn a_reference_to_geometry_that_the_deleted_node_created_fails_honestly() {
    let (mut p, body) = plate();
    let front = top_edge_at_y(&p, body, 0.0);
    let fil = p.add_fillet_ref(body, 2.0, Ref::picks(&[front]));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    // THE FILLET SURFACE ITSELF — the one the plate did not have: it is looked up by name rather than
    // by inclination, so the check does not depend on how the kernel placed the highlight.
    let base: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let blend = p.regen_faces[&fil].iter().find(|f| !base.contains(&f.id)).map(|f| f.id).expect("a face born of the fillet");
    let copy = p.add_face_copy(fil, Ref::one(blend, qymcad_core::refs::Fingerprint::default()));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — a copy of the fillet face: {:?}", rep.errors);

    p.delete_feature_op(fil);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(
        rep.errors.iter().any(|(id, _)| *id == copy),
        "the anchor vanished with the fillet — the node must fail rather than settle on a neighbouring face: {:?}",
        rep.errors
    );
}
