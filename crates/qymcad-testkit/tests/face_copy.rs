//! A SURFACE IS A LEGITIMATE BODY OF THE DOCUMENT, AND "COPY FACE" CREATES ONE.
//!
//! Before this step the design layer was closed at the entrance: the kernel funnel required a
//! volume, and a surface has none by nature — any sheet was rejected as "the part disappeared". The
//! check now asks WHAT the shape is: a solid is measured by volume, a sheet by whether it exists at
//! all. A degenerate solid with zero volume is still dead and still does not enter the document.
//!
//! "Copy face" is the first design tool and the bridge from parametrics into design: a face of a body
//! becomes a surface in its own right while the body stays put. "Replace face" is the far end of the
//! same bridge: the surface GOES BACK into the body and the part is whole again.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::{Query, Ref};

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

fn top_face(p: &Project, body: u64) -> u32 {
    p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("top face").id
}

/// A SHEET PASSES THE KERNEL FUNNEL AND BECOMES A BODY OF THE DOCUMENT — MARKED AS A SHEET.
#[test]
fn a_surface_is_a_body_the_document_knows_about() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let surf = p.add_face_copy(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "copying a face must build — otherwise the whole design layer is closed at the entrance: {:?}", rep.errors);

    let sheet = p.bodies.iter().find(|b| b.id == surf).expect("the surface became a body of the document");
    assert!(!sheet.mesh.tris.is_empty(), "a surface must have geometry");
    // A SHEET'S VOLUME IS NOT ZERO BUT MEANINGLESS: the integral over an OPEN surface says nothing
    // (for this copy it comes out as 9600 — the "volume" of the prism under the face). That is
    // exactly why the document carries the `sheet` flag: deducing from geometry whether the shape is
    // closed means getting it wrong one day.
    assert!(sheet.sheet, "the sheet flag is the only honest answer to \"is there a volume here\"");

    // the area of the copy is exactly the area of the source face
    let want = p.regen_faces[&body].iter().find(|f| f.id == top).map(|f| f.area).expect("the source face");
    let got: f64 = p.regen_faces[&surf].iter().map(|f| f.area).sum();
    assert!((got - want).abs() < 1e-3, "the copy must reproduce the face: area {got:.3} against {want:.3}");
}

/// THE SOURCE BODY STAYS PUT: a copy is a copy, not "the face was taken off the part".
#[test]
fn the_source_body_is_not_consumed() {
    let (mut p, body) = plate();
    let v0 = p.bodies.iter().find(|b| b.id == body).expect("the body").mesh.volume();
    let top = top_face(&p, body);
    p.add_face_copy(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()));
    qymcad_testkit::regenerate(&mut p);

    let src = p.bodies.iter().find(|b| b.id == body).expect("the source must remain");
    assert!((src.mesh.volume() - v0).abs() < 1e-6, "the source must remain untouched: was {v0:.2}, is now {:.2}", src.mesh.volume());
    assert!(!p.consumed_bodies().contains(&body), "a copy does NOT consume its source — otherwise \"take a face into design\" would mean \"lose the part\"");
    assert!(!src.sheet, "and the source stays a BODY, not a sheet");
}

/// FACES ARE TAKEN BY QUERY — the copy follows its base like everything else.
#[test]
fn the_copy_follows_the_base_it_was_taken_from() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let surf = p.add_face_copy(body, Ref::many(Query::Adjacent(Box::new(Query::Id(top)))).clone());
    // "every face parallel to the top one" — a description, not a snapshot
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == surf) {
        if let qymcad_core::feature::FeatureKind::FaceCopy { faces, .. } = &mut n.kind {
            *faces = Ref::many(Query::Oriented { dir: [0.0, 0.0, 1.0], tol_deg: 5.0 });
        }
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a copy by description must build: {:?}", rep.errors);
    let area_before: f64 = p.regen_faces[&surf].iter().map(|f| f.area).sum();
    assert!((area_before - 2400.0).abs() < 1.0, "the top face of a 60x40 plate is 2400 mm^2, and it came out {area_before:.1}");

    // EDITING THE BASE: the plate grew longer — the copy must follow it
    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if (pt.x - 60.0).abs() < 1e-9 {
            pt.x = 80.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the base edit the copy must rebuild: {:?}", rep.errors);
    let area_after: f64 = p.regen_faces[&surf].iter().map(|f| f.area).sum();
    assert!((area_after - 3200.0).abs() < 1.0, "the base grew to 80x40 — the copy must become 3200 mm^2, and it became {area_after:.1}");
}

/// AND A DEGENERATE SOLID IS STILL NOT A PART: the relaxation was made for SHEETS, not for emptiness.
///
/// This check stands right here on purpose: while softening a barrier it is easy to soften it
/// entirely and bring back the very trouble it was raised against (a body with zero volume silently
/// becoming a part).
#[test]
fn a_degenerate_solid_is_still_refused() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    // push the top face INWARD by the full thickness — the solid collapses into nothing
    let f = p.regen_faces[&body].iter().find(|f| f.id == top).cloned().expect("top face");
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let node = p.add_push_face(body, key, -12.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.iter().any(|(id, _)| *id == node), "a collapsed solid must fail rather than become a part: {:?}", rep.errors);
}

/// THE CIRCLE CLOSES: take a face out -> put it back -> the same part.
///
/// This checks the whole design-layer scheme rather than one tool. Replacing a face with its own copy
/// must give EXACTLY the previous part: same volume, whole body, timeline without errors. If that
/// circle does not close on the identity case, there is no point checking any surface edit between
/// "took out" and "put back" — it would land on something already broken.
#[test]
fn a_face_taken_out_and_put_back_gives_the_same_part() {
    let (mut p, body) = plate();
    let v0 = p.bodies.iter().find(|b| b.id == body).expect("the body").mesh.volume();
    let top = top_face(&p, body);

    let surf = p.add_face_copy(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup — copying the face: {:?}", rep.errors);

    let out = p.add_surface_replace(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()), surf);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "replacing a face with its own copy must build: {:?}", rep.errors);

    let res = p.bodies.iter().find(|b| b.id == out).expect("the replacement result");
    assert!(!res.sheet, "the result must be a BODY, not a sheet — otherwise the stitch did not close");
    assert!((res.mesh.volume() - v0).abs() < 1.0, "the part must stay as it was: was {v0:.2}, is now {:.2}", res.mesh.volume());

    // BOTH INPUTS ARE CONSUMED: one part on screen, not a part plus a surface on top of it
    let consumed = p.consumed_bodies();
    assert!(consumed.contains(&body) && consumed.contains(&surf), "both the base and the surface must be consumed by the node");
}

/// THE RESULT CAN BE BUILT ON FURTHER — that is why the node stands IN THE TIMELINE rather than as a
/// layer at the end.
#[test]
fn the_timeline_goes_on_after_the_replacement() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let surf = p.add_face_copy(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()));
    qymcad_testkit::regenerate(&mut p);
    let out = p.add_surface_replace(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()), surf);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    // a fillet AFTER the replacement — an ordinary feature on an ordinary body
    let v_before = p.bodies.iter().find(|b| b.id == out).expect("the body").mesh.volume();
    let edges: Vec<u32> = p.regen_edges[&out].iter().filter(|e| (e.a[2] - 12.0).abs() < 1e-6 && (e.b[2] - 12.0).abs() < 1e-6).map(|e| e.id).collect();
    assert!(!edges.is_empty(), "the result must have edges — which means the topology is intact");
    let fil = p.add_fillet_ref(out, 1.0, Ref::picks(&edges));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "anything at all must build below the node: {:?}", rep.errors);
    let v_after = p.bodies.iter().find(|b| b.id == fil).expect("the filleted body").mesh.volume();
    assert!(v_after < v_before, "the fillet must remove material: {v_before:.2} -> {v_after:.2}");
}

/// THE QUERY FOUND NOTHING — A NAMED REFUSAL, NOT A REPLACEMENT OF "SOMETHING SIMILAR".
///
/// Design is expensive handwork. Putting it in the wrong place silently is worse than not putting it
/// anywhere.
#[test]
fn a_lost_target_face_is_a_named_refusal() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let surf = p.add_face_copy(body, Ref::one(top, qymcad_core::refs::Fingerprint::default()));
    qymcad_testkit::regenerate(&mut p);
    // a reference to a face the body does not have
    let out = p.add_surface_replace(body, Ref::one(0xDEAD_BEEF, qymcad_core::refs::Fingerprint::default()), surf);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.iter().any(|(id, _)| *id == out), "a lost target must fail: {:?}", rep.errors);
}
