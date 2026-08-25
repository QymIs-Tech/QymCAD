//! Mates do what they promise.
//!
//! What was reported: every mate was broken and did whatever it liked. That is checked by number rather than by
//! eye — two parts are built, a mate is placed, and where the solver puts the second one is measured. Each kind
//! is checked against its own definition in `JointKind`, not against whether the result looks right.
use qymcad_core::feature::{AnchorRef, JointKind};
use qymcad_core::model::Project;

/// Two parts, each a cube of side 10, the second moved 50 along X.
fn two_parts() -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = p.new_sketch("a");
    let sid_a = p.sketches[sa].id;
    p.add_sketch_node(sid_a, "a");
    p.add_rect_entity(sa, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(sa);
    let ba = p.add_extrude(sid_a, 10.0);
    p.finish_base_body(ba, 1);

    let b = p.add_part("B");
    p.set_active_component(Some(b));
    let sb = p.new_sketch("b");
    let sid_b = p.sketches[sb].id;
    p.add_sketch_node(sid_b, "b");
    p.add_rect_entity(sb, 50.0, 0.0, 60.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(sb);
    let bb = p.add_extrude(sid_b, 10.0);
    p.finish_base_body(bb, 1);
    (p, a, b)
}

fn origin_of(p: &Project, comp: u64) -> [f64; 3] {
    let t = p.world_transform(comp);
    [t[3], t[7], t[11]]
}

#[test]
fn a_rigid_joint_actually_brings_the_parts_together() {
    let (mut p, a, b) = two_parts();
    let (r0, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r0.errors.is_empty(), "the parts did not build: {:?}", r0.errors);
    let before = origin_of(&p, b);

    // The components are moved apart, and the offset has to live in the placement rather than in the sketch:
    // otherwise both sit at the origin, the mate has nothing to bring together, and the test passes having
    // checked nothing.
    p.move_component(b, [80.0, 30.0, 15.0]);
    let moved = origin_of(&p, b);
    assert!(moved[0] > 1.0, "the component did not move, so there is nothing to check: {moved:?}");

    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);
    p.solve_joints();

    let after = origin_of(&p, b);
    let oa = origin_of(&p, a);
    let d = ((after[0] - oa[0]).powi(2) + (after[1] - oa[1]).powi(2) + (after[2] - oa[2]).powi(2)).sqrt();
    let _ = before;
    eprintln!("rigid: moved to {moved:?}, came back to {after:?}, A at {oa:?}, distance {d:.3}");
    assert!(d < 1e-6, "a rigid mate has to bring the origins together, yet the distance is {d:.3}");
}

/// A revolute mate: one rotational degree about the Z of the connector. The origins therefore have to coincide,
/// the translation being taken away entirely, and the only freedom left is the rotation about that axis.
#[test]
fn a_revolute_joint_removes_translation_but_keeps_rotation() {
    let (mut p, a, b) = two_parts();
    let _ = qymcad_testkit::regenerate(&mut p);
    p.move_component(b, [80.0, 30.0, 15.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Revolute);
    p.solve_joints();
    let after = origin_of(&p, b);
    let d = (after[0] * after[0] + after[1] * after[1] + after[2] * after[2]).sqrt();
    eprintln!("revolute: the origin of B after solving is {after:?}, deviation {d:.3}");
    assert!(d < 1e-6, "a revolute mate has to remove all the translation, yet the origin of B is off by {d:.3}");

    // the angle is the free degree: it is driven, and the part has to turn while the origin stays put
    if let Some(jj) = p.joints.iter_mut().find(|x| x.id == j) {
        jj.drive[0] = Some(90.0); // the angle is driven; the reading is written by the solver
    }
    p.solve_joints();
    let t = p.world_transform(b);
    let after2 = [t[3], t[7], t[11]];
    let d2 = (after2[0] * after2[0] + after2[1] * after2[1] + after2[2] * after2[2]).sqrt();
    eprintln!("revolute at 90°: origin {after2:?}, deviation {d2:.3}, matrix xx={:.3} xy={:.3}", t[0], t[1]);
    assert!(d2 < 1e-6, "a rotation about the axis must not carry the origin away, yet it moved by {d2:.3}");
    assert!((t[0] - 1.0).abs() > 1e-3, "an angle of 90° was driven and there is no rotation: the matrix stayed the identity");
}
