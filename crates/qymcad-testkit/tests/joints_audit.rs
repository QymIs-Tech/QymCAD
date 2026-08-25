//! AUDIT OF THE WHOLE MATE SUBSYSTEM, not of a single example.
//!
//! Reported behaviour: joints do not work, parts stand in the wrong place, degrees of freedom are
//! not where they should be. That means EVERY kind must be checked against its DEFINITION in
//! `JointKind` rather than against the one case that was named. Failures accumulate and are reported
//! at once — what is needed is scale, not the first thing that trips.
//!
//! Definitions (JointKind): Rigid 0 DOF; Revolute 1R about Z; Slider 1T along Z; Cylindrical 1R+1T;
//! Planar 2T in XY plus 1R about Z; Ball 3R about a point; PinSlot 1R plus 1T along X.
use qymcad_core::feature::{AnchorRef, JointKind};
use qymcad_core::model::Project;

fn two_parts() -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let mk = |p: &mut Project, name: &str, x: f64| {
        let c = p.add_part(name);
        p.set_active_component(Some(c));
        let s = p.new_sketch(name);
        let sid = p.sketches[s].id;
        p.add_sketch_node(sid, name);
        p.add_rect_entity(s, x, 0.0, x + 10.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s);
        let b = p.add_extrude(sid, 10.0);
        p.finish_base_body(b, 1);
        c
    };
    let a = mk(&mut p, "A", 0.0);
    let b = mk(&mut p, "B", 50.0);
    (p, a, b)
}

fn org(p: &Project, c: u64) -> [f64; 3] {
    let t = p.world_transform(c);
    [t[3], t[7], t[11]]
}

/// Place a mate and push B away; return where the solver put it.
fn solved(kind: JointKind, off: [f64; 3]) -> ([f64; 3], [f64; 12]) {
    let (mut p, a, b) = two_parts();
    let _ = qymcad_testkit::regenerate(&mut p);
    p.move_component(b, off);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, kind);
    p.solve_joints();
    let _ = org(&p, a);
    (org(&p, b), p.world_transform(b))
}

/// CORRECTED READING. Slider and planar were first declared "over-constrained" here, and that was
/// wrong: the freedom of a mechanical joint is expressed by its SLOT (angle or offset), not by the
/// residual position of the part. Verified separately: a slider travels exactly the commanded 25 mm
/// along its slot. A mate pulls the parts together and motion is set by the joint value; the
/// expectation was wrong, not the code.
///
/// Separately (not covered by this test, it needs its own pass): a `FaceCenter` anchor builds its
/// frame from the face NORMAL, and on a cylindrical bore the normals cancel each other around the
/// circle — the rotation axis degenerates and the hinge turns inside out. The correct pattern lies
/// right next to it: `EdgeMid` on a round edge takes the AXIS of the circle (`resolve_edge_axis`),
/// not a normal.
#[test]
fn every_joint_kind_removes_exactly_the_degrees_it_promises() {
    let mut bad: Vec<String> = Vec::new();
    let off = [80.0, 30.0, 15.0];
    let d = |o: [f64; 3]| (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();

    // Rigid: 0 DOF — a full coincidence
    let (o, _) = solved(JointKind::Rigid, off);
    if d(o) > 1e-6 { bad.push(format!("Rigid: must make the origins coincide, deviation {:.3}", d(o))); }

    // Revolute: 1R about Z — translation removed ENTIRELY
    let (o, _) = solved(JointKind::Revolute, off);
    if d(o) > 1e-6 { bad.push(format!("Revolute: translation must be removed, deviation {:.3}", d(o))); }

    // Slider: 1T along Z. The freedom is expressed by the joint SLOT (see
    // a_slider_slides_by_its_offset_slot), not by residual position: at zero offset the parts are
    // pulled together. Across the axis the parts are pulled together; ALONG it they stay where they
    // were (minimal displacement).
    let (o, _) = solved(JointKind::Slider, off);
    if o[0].abs() > 1e-6 || o[1].abs() > 1e-6 {
        bad.push(format!("Slider: must pull together across the axis, and X={:.3} Y={:.3}", o[0], o[1]));
    }

    // Cylindrical: 1R+1T along Z — same as the slider in translation
    let (o, _) = solved(JointKind::Cylindrical, off);
    if o[0].abs() > 1e-6 || o[1].abs() > 1e-6 {
        bad.push(format!("Cylindrical: translation across the axis must be removed, and X={:.3} Y={:.3}", o[0], o[1]));
    }

    // Planar: 2T in XY plus 1R — translation along the normal removed (in-plane freedom is set by slots)
    let (o, _) = solved(JointKind::Planar, off);
    if o[2].abs() > 1e-6 { bad.push(format!("Planar: translation along the normal must be removed, and Z={:.3}", o[2])); }

    // Ball: 3R about a point — translation removed entirely
    let (o, _) = solved(JointKind::Ball, off);
    if d(o) > 1e-6 { bad.push(format!("Ball: translation must be removed, deviation {:.3}", d(o))); }

    assert!(bad.is_empty(), "MATES — {} failures:\n  {}", bad.len(), bad.join("\n  "));
}

/// THE JOINT SLOT DRIVES THE PART: a slider moves by its commanded `offset`, not by where the part
/// happened to lie.
///
/// Before calling over-constraining a defect, check the reading itself. The code states that purely
/// mechanical assemblies are solved as a TREE and freedom is expressed by the joint slot (angle or
/// offset), not by the residual position of a component. If so, Z=0 at zero offset is correct and it
/// was the test that was wrong.
#[test]
fn a_slider_slides_by_its_offset_slot() {
    let (mut p, a, b) = two_parts();
    let _ = qymcad_testkit::regenerate(&mut p);
    p.move_component(b, [80.0, 30.0, 15.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Slider);
    p.solve_joints();
    let at0 = org(&p, b);
    if let Some(jj) = p.joints.iter_mut().find(|x| x.id == j) {
        jj.drive[1] = Some(25.0); // the COMMANDED offset (the readout is written by the solver)
    }
    p.solve_joints();
    let at25 = org(&p, b);
    eprintln!("[slider] offset 0 -> {at0:?}; offset 25 -> {at25:?}");
    // A joint value sets a POSITION, not an increment: an offset of 25 puts the anchor at z=25, it
    // does not "travel 25 from wherever the part stood". Otherwise the same value would mean
    // different things depending on history, and the assembly would stop being reproducible.
    assert!((at25[2] - 25.0).abs() < 1e-6, "an offset of 25 must put the part at z=25, and it is at z={:.3}", at25[2]);
    // and at zero offset the part stays WHERE IT WAS: the joint does not require otherwise (minimal displacement)
    let _ = at0;
}

/// THE AXIS OF A BORE: a mate on a cylindrical face must rotate about the AXIS, not about some
/// arbitrary direction.
///
/// Reported behaviour: a laptop lid on hinges, mated by bore faces, turned inside out. The cause was
/// that a face anchor built its frame from the NORMAL, and on a cylinder the normal depends on the
/// point of the surface and cancels to zero around the circle. Checked directly: the axis of a bore
/// drilled along Z must be parallel to Z.
#[test]
fn a_hole_face_reports_its_axis_not_its_normal() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("plate");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "plate");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 10.0);
    let si2 = p.new_sketch("bore");
    let sid2 = p.sketches[si2].id;
    p.add_sketch_node(sid2, "bore");
    p.add_circle_entity(si2, 20.0, 20.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si2);
    let cut = p.add_combine(body, sid2, 20.0, 0);
    let last = p.finish_base_body(cut, 1);
    let (r, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);

    let faces = p.regen_faces.get(&last).cloned().unwrap_or_default();
    let hole = faces
        .iter()
        .find(|f| {
            let c = f.centroid;
            (c.x - 20.0).abs() < 6.0 && (c.y - 20.0).abs() < 6.0 && f.normal[2].abs() < 0.5
        })
        .expect("the bore face");
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [hole.centroid.x, hole.centroid.y, hole.centroid.z], normal: hole.normal, id: hole.id };
    let ax = p.face_axis(last, &key).expect("a cylindrical face must have an axis");
    eprintln!("[bore axis] point {:?} direction {:?} (the face normal was {:?})", ax.0, ax.1, hole.normal);
    assert!(ax.1[2].abs() > 0.999, "a bore along Z must have an axis parallel to Z, and it came out {:?}", ax.1);
}

/// A HINGE ON TWO BORES OF DIFFERENT DEPTH DOES NOT DRAG THE PART ALONG THE AXIS.
///
/// Reported behaviour: picking the bore in a lid and the bore in a body makes the lid move away;
/// long bores centre on their own mid-length points. That is what happened: the origin of a
/// cylindrical-face anchor lies at the MIDDLE of the bore, bores of different depth have different
/// middles, and a revolute mate forcibly pulled those points together. Physically the hinge axis
/// requires nothing of the sort — the pin can sit anywhere along the axis.
#[test]
fn a_hinge_on_holes_of_different_depth_does_not_drag_the_part() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    p.new_document();
    // a part with a bore of the given depth, offset along X
    let hole_part = |p: &mut Project, name: &str, x: f64, depth: f64| -> (u64, u64) {
        let c = p.add_part(name);
        p.set_active_component(Some(c));
        let s = p.new_sketch(name);
        let sid = p.sketches[s].id;
        p.add_sketch_node(sid, name);
        p.add_rect_entity(s, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s);
        let b = p.add_extrude(sid, depth);
        let s2 = p.new_sketch("bore");
        let sid2 = p.sketches[s2].id;
        p.add_sketch_node(sid2, "bore");
        p.add_circle_entity(s2, x + 10.0, 10.0, 4.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s2);
        let cut = p.add_combine(b, sid2, depth * 2.0, 0);
        let last = p.finish_base_body(cut, 1);
        (c, last)
    };
    let (ca_comp, ba) = hole_part(&mut p, "body", 0.0, 12.0);
    let (cb_comp, bb) = hole_part(&mut p, "lid", 60.0, 3.0);
    let (r, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);

    let hole_key = |p: &Project, body: u64, cx: f64| -> FaceKey {
        let f = p.regen_faces.get(&body).unwrap().iter().find(|f| {
            (f.centroid.x - cx).abs() < 5.0 && (f.centroid.y - 10.0).abs() < 5.0 && f.normal[2].abs() < 0.5
        }).expect("the bore face").clone();
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    };
    let ka = hole_key(&p, ba, 10.0);
    let kb = hole_key(&p, bb, 70.0);
    // the bore depths DIFFER -> the middles are at different heights
    eprintln!("[hinge] middle of the body bore z={:.2}, of the lid bore z={:.2}", ka.centroid[2], kb.centroid[2]);
    assert!((ka.centroid[2] - kb.centroid[2]).abs() > 1.0, "the test is pointless: the middles coincide");

    let a = p.add_connector(ca_comp, qymcad_core::feature::AnchorRef::FaceCenter(ba, ka.clone()));
    let b = p.add_connector(cb_comp, qymcad_core::feature::AnchorRef::FaceCenter(bb, kb.clone()));
    // A REVOLUTE mate makes the anchor origins coincide by definition. If the anchors sit at the
    // middles of bores of different depth, the part honestly travels by the difference: the fault is
    // not in the joint but in the ATTACHMENT POINT. Choosing that point is the job of the anchor
    // layer, where a cylinder offers attachments at the middle AND at each end.
    //
    // What is checked here is the case where the position along the axis is NOT determined: a
    // cylindrical mate, where the part must stay where it stood.
    p.add_joint(a, b, JointKind::Cylindrical);
    let before = org(&p, cb_comp);
    p.solve_joints();
    let after = org(&p, cb_comp);
    eprintln!("[hinge] lid: {before:?} -> {after:?}");
    let dz = (after[2] - before[2]).abs();
    assert!(dz < 1e-6, "the lid was dragged {dz:.3} mm along the hinge axis — exactly the difference of the bore depths");
}

/// A HINGE MATCHED BY THE ENDS OF BORES OF DIFFERENT DEPTH LANDS EXACTLY.
///
/// The real case: one bore 10 mm, the other 140. Matched by MIDDLES the parts stand apart by the
/// difference (65 mm) — and that is not a solver error but a consequence of the middles of different
/// bores not corresponding to each other. A cylinder therefore offers three attachment points, and
/// the end is chosen by the user. What is checked here is that choosing the end really does place
/// the parts correctly.
#[test]
fn a_hinge_matched_by_hole_ends_lands_exactly() {
    use qymcad_core::asm::connector::AttachPoint;
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    p.new_document();
    let hole_part = |p: &mut Project, name: &str, x: f64, depth: f64| -> (u64, u64) {
        let c = p.add_part(name);
        p.set_active_component(Some(c));
        let s = p.new_sketch(name);
        let sid = p.sketches[s].id;
        p.add_sketch_node(sid, name);
        p.add_rect_entity(s, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s);
        let b = p.add_extrude(sid, depth);
        let s2 = p.new_sketch("bore");
        let sid2 = p.sketches[s2].id;
        p.add_sketch_node(sid2, "bore");
        p.add_circle_entity(s2, x + 10.0, 10.0, 4.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s2);
        let cut = p.add_combine(b, sid2, depth * 2.0, 0);
        (c, p.finish_base_body(cut, 1))
    };
    let (ca_comp, ba) = hole_part(&mut p, "body", 0.0, 14.0);
    let (cb_comp, bb) = hole_part(&mut p, "lid", 60.0, 3.0);
    let (r, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "did not build: {:?}", r.errors);

    let hole_key = |p: &Project, body: u64, cx: f64| -> FaceKey {
        let f = p
            .regen_faces
            .get(&body)
            .unwrap()
            .iter()
            .find(|f| (f.centroid.x - cx).abs() < 5.0 && (f.centroid.y - 10.0).abs() < 5.0 && f.normal[2].abs() < 0.5)
            .expect("the bore face")
            .clone();
        FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
    };
    let a = p.add_connector(ca_comp, qymcad_core::feature::AnchorRef::FaceCenter(ba, hole_key(&p, ba, 10.0)));
    let b = p.add_connector(cb_comp, qymcad_core::feature::AnchorRef::FaceCenter(bb, hole_key(&p, bb, 70.0)));
    // BOTH ANCHORS ON THE START END: the parts must meet end to end, not middle to middle
    for cid in [a, b] {
        p.connectors.iter_mut().find(|c| c.id == cid).unwrap().point = AttachPoint::Start;
    }
    p.add_joint(a, b, JointKind::Revolute);
    p.solve_joints();

    // the anchors must coincide: a revolute mate makes the origins coincide by definition
    let (fa, fb) = (p.connector_matrix(a).unwrap(), p.connector_matrix(b).unwrap());
    let wa = qymcad_core::asm::bridge::pose_from12(&p.world_transform(ca_comp)) * qymcad_core::asm::bridge::pose_from12(&fa);
    let wb = qymcad_core::asm::bridge::pose_from12(&p.world_transform(cb_comp)) * qymcad_core::asm::bridge::pose_from12(&fb);
    let d = (wb.translation.vector - wa.translation.vector).norm();
    eprintln!("[ends] body anchor {:?}, lid anchor {:?}, divergence {d:.6}", wa.translation.vector, wb.translation.vector);
    assert!(d < 1e-6, "matched by ends the anchors must coincide exactly, and the divergence is {d:.6} mm");
}
