//! Suppression, colours, reordering, sketches on faces of other components, joints and skeleton dimensions.

use super::*;

#[test]
fn body_boolean_survives_serde() {
    // A body boolean node survives the RON round trip.
    let mut p = part_project();
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let a = p.add_extrude_on(s1, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b = p.add_extrude_on(s2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let res = p.add_body_boolean(a, b, 2); // Intersection.
    let ron = qymcad_core::model::to_ron(&p).expect("serialisation");
    let p2 = qymcad_core::model::from_ron(&ron).expect("deserialisation");
    let node = p2.timeline.iter().find(|n| n.id == res).expect("the boolean node must exist after loading");
    assert!(matches!(node.kind, qymcad_core::feature::FeatureKind::BodyBoolean { op: 2, .. }), "the intersection operation must be preserved");
}

#[test]
fn suppress_feature_cascades_to_dependents() {
    // Suppressing a single feature leaves its body unbuilt and cascades to its consumers, such as a cut on
    // that body.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let cut = p.add_combine_on(
        base,
        sid,
        0,
        qymcad_core::model::CombineSpan { height: 4.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, fill: &[] },
        0,
    ); // Depends on the base.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(base).is_some() && p.mesh_index(cut).is_some(), "both bodies must be built");
    // Suppressing the extrude must, by cascade, leave the cut unbuilt as well.
    let ti = p.timeline_index(base).unwrap();
    assert!(p.set_feature_suppressed(ti, true), "suppression must be switched on");
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(base).is_none(), "a suppressed feature must not be built");
    assert!(p.mesh_index(cut).is_none(), "a consumer of a suppressed feature must not be built either");
    // Switching it back on restores both bodies.
    p.set_feature_suppressed(ti, false);
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(p.mesh_index(base).is_some(), "after switching back on the base must be built");
    assert!(p.mesh_index(cut).is_some(), "after switching back on the consumer must be restored by the dirty cascade");
}

#[test]
fn suppress_modifier_passes_through_keeping_chain() {
    // Suppressing a modifier in the middle of a chain is a pass-through: its effect is removed while its
    // output is a copy of its source, so the consumers below it keep building. The chain does not collapse, as
    // it did with a cascade that switched off everything below a suppressed middle feature.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let ext = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(ext, 1.0, Vec::<u32>::new());
    let cha = p.add_chamfer(fil, 0.5, Vec::<u32>::new());
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(ext).is_some() && p.mesh_index(fil).is_some() && p.mesh_index(cha).is_some(), "all of them must be built");
    let ti = p.timeline_index(fil).unwrap();
    p.set_feature_suppressed(ti, true); // Suppress the middle modifier, the fillet.
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(fil).is_some(), "a suppressed modifier is a pass-through whose output is a copy of its source");
    assert!(p.mesh_index(cha).is_some(), "the consumer must keep building: the chain does not collapse");
    // Switching it back on makes the fillet real again and the chain stays intact.
    p.set_feature_suppressed(ti, false);
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(p.mesh_index(cha).is_some(), "after switching back on the chain must be intact");
}

#[test]
fn suppressed_flag_survives_serde() {
    // The suppressed flag of a timeline node survives saving and loading.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ti = p.timeline_index(body).unwrap();
    p.set_feature_suppressed(ti, true);
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert!(back.timeline.iter().find(|n| n.id == body).unwrap().suppressed, "the suppressed flag must survive serialisation");
}

#[test]
fn body_color_stable_across_lineage() {
    // A colour is keyed by the lineage root of a part, so it does not change with operations; different parts
    // get different colours; a manual colour paints the whole chain and clearing it returns the palette
    // entry.
    use qymcad_core::model::default_part_color;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let ext = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(ext, 1.0, Vec::<u32>::new()); // Consumes the extrude: the same part.
    let k = MockKernel::default();
    p.regenerate(&k);
    let (ie, iff) = (p.mesh_index(ext).unwrap(), p.mesh_index(fil).unwrap());
    assert_eq!(p.mesh_color(iff), default_part_color(0), "the colour comes from the lineage root (part 0)");
    assert_eq!(p.mesh_color(ie), p.mesh_color(iff), "the colour must not jump with an operation on the same part");
    // A second part gets a different, stable colour.
    let sid2 = p.add_line_sketch("square 2", vec![Point2::new(20.0, 0.0), Point2::new(30.0, 0.0), Point2::new(30.0, 10.0), Point2::new(20.0, 10.0)], true);
    p.add_sketch_node(sid2, "square 2");
    let ext2 = p.add_extrude_on(sid2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    p.regenerate(&k);
    let ie2 = p.mesh_index(ext2).unwrap();
    assert_eq!(p.mesh_color(ie2), default_part_color(1), "the second part must take colour 1");
    assert_ne!(p.mesh_color(ie2), p.mesh_color(iff), "different parts must get different colours");
    // A manual colour on the filleted body paints the whole part through its root; clearing it returns the
    // palette.
    p.set_mesh_color(iff, [10, 20, 30]);
    assert_eq!(p.mesh_color(iff), [10, 20, 30], "the manual colour must be applied");
    assert_eq!(p.mesh_color(ie), [10, 20, 30], "the manual colour must cover the whole chain sharing that root");
    p.reset_mesh_color(ie);
    assert_eq!(p.mesh_color(iff), default_part_color(0), "clearing it must return the palette entry for the root");
}

#[test]
fn reorder_respects_body_dependencies() {
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(body, 1.0, Vec::<u32>::new());
    let ei = p.timeline_index(body).unwrap();
    let fi = p.timeline_index(fil).unwrap();
    assert!(!p.reorder_feature(fi, ei), "moving a chamfer above the extrude that produces its input body must be refused");
}

// --- Component isolation ---

#[test]
fn body_tool_in_assembly_lands_in_a_part() {
    // Invariant: a body never belongs to an assembly, so `body_parent` creates a part.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root)); // Active in the root assembly.
    let body = p.add_box(10.0, 10.0, 10.0);
    let owner = p.body_owner(body).expect("the body has an owner");
    assert!(p.component_is_part(owner), "the body must land in a part rather than in an assembly");
    assert_ne!(owner, root, "the owner must not be the root assembly");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "the build must produce no isolation errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the body must be built");
}

#[test]
fn cross_component_sketch_on_face_is_blocked() {
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    // Part A with a body.
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    let body_a = p.add_extrude(sa, 5.0);
    // Part B, whose sketch sits on a face of the body of A: a cross-component reference.
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_active_component(Some(b));
    let sb = square(&mut p, "sb");
    let si = p.sketch_index(sb).unwrap();
    p.sketches[si].plane = SketchPlane::Face(body_a, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 });
    p.add_sketch_node(sb, "sb");
    let body_b = p.add_extrude(sb, 5.0);

    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(p.mesh_index(body_a).is_some(), "the body of its own part A must be built");
    assert!(rep.errors.iter().any(|(id, _)| *id == body_b), "the cross-component reference of body B must be blocked: {:?}", rep.errors);
    assert!(p.mesh_index(body_b).is_none(), "body B must not be built");
    assert_eq!(k.count(), 1, "the kernel must be called only for the valid body A");
}

#[test]
fn external_ref_unblocks_cross_component_face_sketch() {
    // Controlled top-down design: a sketch of part B sits on a face of the body of part A. Without an external
    // reference isolation blocks it (see `cross_component_sketch_on_face_is_blocked`). With an explicit
    // `ExternalRef` the cross-component reference is authorised and the source face is carried into the local
    // space of the consumer through `world_transform`.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    // Part A with a body, providing the source face.
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    let body_a = p.add_extrude(sa, 5.0);
    // Part B with a sketch on a face of the body of A: a cross-component reference.
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_active_component(Some(b));
    let sb = square(&mut p, "sb");
    let si = p.sketch_index(sb).unwrap();
    let key = FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    p.sketches[si].plane = SketchPlane::Face(body_a, key);
    p.add_sketch_node(sb, "sb");
    let body_b = p.add_extrude(sb, 5.0);

    // Without an external reference it is blocked and the frame of the foreign face does not resolve.
    let k = MockKernel { no_faces: true, ..Default::default() };
    let rep = p.regenerate(&k);
    assert!(rep.errors.iter().any(|(id, _)| *id == body_b), "without an external reference body B must be blocked");
    assert!(p.mesh_index(body_b).is_none(), "without an external reference body B must not be built");
    assert!(p.sketch_frame_by_id(sb).is_none(), "without an external reference the frame of a foreign face must be `None`");

    // Register an external reference from B to a face of body A, then move A by +10 along X to exercise the
    // conversion between local spaces.
    p.set_component_transform(a, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let ext = p.add_external_face_ref(b, body_a, key);
    assert!(p.external_authorized(b, body_a), "the external reference must be authorised");

    let k2 = MockKernel { no_faces: true, ..Default::default() };
    let rep2 = p.regenerate(&k2);
    assert!(!rep2.errors.iter().any(|(id, _)| *id == body_b), "with an external reference body B must not be in the errors: {:?}", rep2.errors);
    assert!(p.mesh_index(body_b).is_some(), "with an external reference body B must be built");

    // The frame of sketch B is anchored to the face of A and carried whole into the local space of B. Its 2D
    // zero is the projection of the origin of source A onto the face ([0,0,5] in the space of A), and after
    // moving A by +10 along X it becomes [10,0,5] in the space of B.
    // Moving A along any axis carries the origin with it, not only along the face normal.
    let f = p.sketch_frame_by_id(sb).expect("the frame must resolve through the external reference");
    assert!(
        (f.origin[0] - 10.0).abs() < 1e-9 && (f.origin[1]).abs() < 1e-9 && (f.origin[2] - 5.0).abs() < 1e-9,
        "the 2D zero must be anchored to the source face and follow it along every axis: {:?}",
        f.origin
    );

    // Top-down associativity: the source body A changed, so the consumer body B must rebuild.
    p.mark_node_dirty(body_a);
    let k3 = MockKernel { no_faces: true, ..Default::default() };
    p.regenerate(&k3);
    assert_eq!(k3.count(), 2, "editing source body A must rebuild the dependent body B as well");

    // Registration is idempotent.
    assert_eq!(p.add_external_face_ref(b, body_a, key), ext, "registering again must return the same id");
    assert_eq!(p.external_refs.len(), 1, "no duplicate external reference must be created");
}

#[test]
fn rigid_joint_offset_translates_along_normal() {
    // A rigid mate honours the gap along the connector normal, leaving the faces a given distance apart.
    use qymcad_core::feature::{JointKind, PLACE_IDENTITY};
    assert_eq!(JointKind::Rigid.motion(0.0, 0.0, 0.0), PLACE_IDENTITY, "a zero offset means flush, that is, the identity");
    let m = JointKind::Rigid.motion(0.0, 7.0, 0.0);
    assert_eq!(m[11], 7.0, "an offset of 7 must translate by +7 along the connector Z axis");
    assert_eq!((m[3], m[7]), (0.0, 0.0), "there must be no shift along X or Y");
}

#[test]
fn joint_angle_is_parametric_via_global() {
    // The angle of a mate can be an expression over a global parameter, like a feature or sketch dimension. It
    // is stored in `feat_dims` under the joint id and evaluated during regenerate before the mates are solved,
    // so editing the parameter recomputes it.
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Param;
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Revolute);
    // The global parameter `ang` is 90 and the joint angle is the expression `ang`.
    p.parameters.push(Param { name: "ang".into(), expr: "90".into(), value: 90.0 });
    p.set_feat_dim(j, "angle", "ang".into());

    let k = MockKernel { no_faces: true, ..Default::default() };
    p.regenerate(&k);
    assert!((p.joints.iter().find(|x| x.id == j).unwrap().drive[0].unwrap_or(f64::NAN) - 90.0).abs() < 1e-9, "the angle must be taken from the global parameter");
    // The point (1,0,0) fixed in B is turned by 90 degrees into (0,1,0).
    let q = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!(q[0].abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9, "the rotation must use the angle from the parameter (90 degrees): {q:?}");

    // Changing the global parameter recomputes both the angle and the placement.
    p.parameters[0].value = 180.0;
    p.regenerate(&k);
    // The tolerance follows the contract of the solver rather than luck. A tolerance of 1e-9 degrees is
    // tighter than anything the solver promises, its own tolerance being a residual below 1e-7. The check
    // passed only because the solver silently took a hundred and ninety extra steps: its exit threshold was
    // compared against a cost that included the pull towards the original place and was never reached. Once the
    // exit started comparing against the constraint residual (7 steps instead of 200) the angle came out as
    // 179.99999999, that is 1.2e-8 degrees or 2e-10 radians. Demanding more would test rounding rather than the
    // model.
    let got = p.joints.iter().find(|x| x.id == j).unwrap().angle;
    assert!((got - 180.0).abs() < 1e-6, "the angle must be recomputed: {got}");
    let q2 = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!((q2[0] + 1.0).abs() < 1e-9 && q2[1].abs() < 1e-9, "a rotation of 180 degrees: {q2:?}");
}

#[test]
fn skeleton_named_dim_drives_parameter() {
    // A skeleton sketch as a driver: a distance dimension named `len` becomes available as a parameter, so a
    // part references it from an expression in top-down fashion. Editing the skeleton dimension moves the
    // parameter with it.
    use qymcad_core::model::Constraint;
    let mut p = part_project();
    let sid = p.add_line_sketch("skeleton", vec![Point2::new(0.0, 0.0), Point2::new(30.0, 0.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|pt| pt.id).collect();
    let (a, b) = (pts[0], pts[1]);
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    assert!(p.add_named_dim("len".into(), sid, vec![a, b]), "the dimension must be named as a driver");
    assert_eq!(p.param_map().get("len"), Some(&30.0), "a named dimension must be visible as a parameter");
    assert_eq!(p.eval_expr("len*2").unwrap(), 60.0, "a part must be able to reference it from an expression");
    // Editing the skeleton dimension moves the parameter with it.
    if let Constraint::Distance { d, .. } = &mut p.sketches[si].constraints[0] {
        *d = 50.0;
    }
    assert_eq!(p.param_map().get("len"), Some(&50.0), "editing the skeleton dimension must give the parameter its new value");
    // serde
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert_eq!(back.named_dims.len(), 1, "the named dimension must survive serialisation");
    assert_eq!(back.param_map().get("len"), Some(&50.0), "it must still resolve after serialisation");
}
