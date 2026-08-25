//! A BROKEN BODY DOES NOT BECOME A PART.
//!
//! The symptom: a hole where a wall should be, with the inside of the part visible through it.
//! Numbers from the file it came from: pushing a face produced a body that fails OCCT validation,
//! with a volume of 160767 instead of the expected 20019 — and the timeline said nothing. The next
//! operation then settled on that carcass (a thicken caught on an unnamed face of the corpse) and
//! everything fell apart.
//!
//! The cause was the order of the boolean operands: `Fuse(body, prism)` returned a solid with
//! inconsistent face orientations while `Fuse(prism, body)` on the same inputs returned a correct
//! one. That is fixed (see `qym_shape_push_face`) and the operation itself now works.
//!
//! The barrier stayed, because the case was not unique by nature: any kernel operation can return an
//! unusable body, and accepting one silently is not allowed. The barrier sits in the single funnel
//! `OcctKernel::finish`, through which the result of each of the 41 operations passes.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A 40x30x20 box with a 2 mm shell open at both ends; returns (project, body).
fn through_shell() -> (Project, u64) {
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
    let b = p.add_extrude_multi(sid, closed, 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&b].iter().filter(|f| f.normal[2].abs() > 0.9).map(|f| f.id).collect();
    let shell = p.add_shell_mode(b, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    (p, shell)
}


/// AND A SOUND OPERATION GOES THROUGH AS BEFORE — the barrier does not choke working cases.
#[test]
fn a_sound_operation_still_goes_through() {
    let (mut p, shell) = through_shell();
    let wall = p.regen_faces[&shell].iter().filter(|f| f.normal[0] > 0.9).max_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x)).expect("the outer wall").clone();
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [wall.centroid.x, wall.centroid.y, wall.centroid.z], normal: wall.normal, id: wall.id };
    let node = p.add_push_face(shell, key, 3.0);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a working face push must build: {:?}", rep.errors);
    let s = shapes.get(&node).expect("the body was built");
    assert!((s.volume() - 7080.0).abs() < 1.0, "volume {:.1} instead of 7080 — the wall went the wrong way", s.volume());
}

/// A RIM EATEN BY A FILLET DOWN TO TWO STRIPS CAN BE PUSHED — AND IF NOT, THEN BY REFUSAL.
///
/// A fillet of R1 takes both the outer and the inner long edges of the rim: a millimetre from each
/// side eats a 2 mm wall right through, and the rim falls apart into two strips at the edges. That
/// used to be one face with one name lying in two places — a push lifted both halves and returned a
/// corpse (`face_is_one_island` covers the same thing against the live kernel). Now the strips are
/// distinct and exactly one is pushed.
///
/// The requirement is unchanged for the case where the kernel refuses after all: the node fails and
/// the part stays untouched. Accepting a corpse silently is not allowed under any outcome.
#[test]
fn when_the_push_cannot_be_done_the_part_is_left_alone() {
    let (mut p, shell) = through_shell();
    let top = p.regen_edges[&shell].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
    let len = |e: &qymcad_core::geom::MeshEdge| {
        let d = [e.b[0] - e.a[0], e.b[1] - e.a[1], e.b[2] - e.a[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    let outer: Vec<u32> = p.regen_edges[&shell].iter().filter(|e| (e.a[2] - top).abs() < 1e-6 && (e.b[2] - top).abs() < 1e-6 && len(e) > 35.0).map(|e| e.id).collect();
    assert!(!outer.is_empty(), "the long rim edges were found");
    let cha = p.add_fillet_ref(shell, 1.0, qymcad_core::refs::Ref::picks(&outer));
    let (r1, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(r1.errors.is_empty(), "the setup must pass: {:?}", r1.errors);
    let before = shapes.get(&cha).expect("the body before the push").volume();

    // push the top rim
    let tz = p.regen_faces[&cha].iter().filter(|f| f.normal[2] > 0.9).map(|f| f.centroid.z).fold(f64::MIN, f64::max);
    let rim = p.regen_faces[&cha].iter().find(|f| f.normal[2] > 0.9 && (f.centroid.z - tz).abs() < 1e-6).expect("the rim").clone();
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [rim.centroid.x, rim.centroid.y, rim.centroid.z], normal: rim.normal, id: rim.id };
    let node = p.add_push_face(cha, key, 5.0);
    let (r2, shapes) = qymcad_testkit::regenerate(&mut p);

    // THE TIMELINE DECIDES, NOT THE BODY CACHE: the kernel keeps the PREVIOUS shape under the same
    // id, so "there is a body" does not yet mean "the operation succeeded". Ask the error list.
    if r2.errors.iter().any(|(id, _)| *id == node) {
        // AN HONEST REFUSAL — the part must stay untouched
        let src = shapes.get(&cha).expect("the source is intact");
        assert!(src.is_valid(), "the source must stay usable");
        assert!((src.volume() - before).abs() < 1e-6, "the source must stay untouched: was {before:.1}, now {:.1}", src.volume());
    } else {
        // IT REALLY WORKED: the body is usable and the gain is commensurate with the rim
        let s = shapes.get(&node).expect("the body was built");
        assert!(s.is_valid(), "the body must pass kernel validation");
        let grown = s.volume() - before;
        assert!(grown > 0.0 && grown < 3000.0, "a gain of {grown:.1} mm^3 — with broken orientations this used to come out in the hundreds of thousands");
    }
}
