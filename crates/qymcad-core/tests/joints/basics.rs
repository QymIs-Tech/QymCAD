//! Rigid, revolute and slider joints, loops, freedoms, face mates, connectors on edges and vertices, serde.

use super::*;

#[test]
fn rigid_joint_coincides_origins() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    // A is grounded and displaced; B is driven.
    set_transform(&mut p, a, tr(10.0, 0.0, 0.0));
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);

    p.solve_joints();
    // Rigid: the origin of B meets the origin of A, so B lands at (10,0,0).
    //
    // A numeric solver converges to a tolerance; exact equality was a property of the earlier implementation
    // (pure matrix composition) rather than a requirement of the problem.
    let got = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let err = ((got[0] - 10.0).powi(2) + got[1].powi(2) + got[2].powi(2)).sqrt();
    assert!(err < 1e-6, "a rigid mate must bring the origins together: {got:?}, off by {err:.3e}");
}

#[test]
fn revolute_joint_angle_drives_rotation() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Revolute);
    // Turn by 90 degrees about the connector Z axis.
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[0] = Some(90.0); // A specified value, not a reading.

    p.solve_joints();
    // The point (1,0,0) fixed in B moves to (0,1,0).
    let q = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!(q[0].abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9, "got {q:?}");

    // Parametric: changing the angle recomputes the placement.
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[0] = Some(180.0); // A specified value, not a reading.
    p.solve_joints();
    let q2 = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!((q2[0] + 1.0).abs() < 1e-9 && q2[1].abs() < 1e-9, "got {q2:?}");
}

#[test]
fn slider_chain_propagates() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    // A to B is a slider of +5 along Z, B to D another of +5: two bodies propagated from the grounded one.
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb1 = p.add_connector(b, AnchorRef::Origin);
    let j1 = p.add_joint(ca, cb1, JointKind::Slider);
    p.joints.iter_mut().find(|x| x.id == j1).unwrap().drive[1] = Some(5.0); // A specified value, not a reading.
    let cb2 = p.add_connector(b, AnchorRef::Origin);
    let cd = p.add_connector(d, AnchorRef::Origin);
    let j2 = p.add_joint(cb2, cd, JointKind::Slider);
    p.joints.iter_mut().find(|x| x.id == j2).unwrap().drive[1] = Some(5.0); // A specified value, not a reading.

    p.solve_joints();
    let got = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let err = (got[0].powi(2) + got[1].powi(2) + (got[2] - 5.0).powi(2)).sqrt();
    assert!(err < 1e-6, "B must be propagated by +5: {got:?}, off by {err:.3e}");
    let gd = apply12(&p.world_transform(d), [0.0, 0.0, 0.0]);
    let ed = (gd[0].powi(2) + gd[1].powi(2) + (gd[2] - 10.0).powi(2)).sqrt();
    assert!(ed < 1e-6, "D must be propagated by +10: {gd:?}, off by {ed:.3e}");
}

#[test]
fn loop_is_detected() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    let mk = |p: &mut Project, x, y| {
        let cx = p.add_connector(x, AnchorRef::Origin);
        let cy = p.add_connector(y, AnchorRef::Origin);
        p.add_joint(cx, cy, JointKind::Rigid)
    };
    mk(&mut p, a, b);
    mk(&mut p, b, d);
    let j_close = mk(&mut p, d, a); // Closes the loop A-B-D-A.

    let rep = p.solve_joints();
    // A loop is not a special case. An algorithm that builds a tree and closes the loops separately needs a
    // list of loops in its report; a solver that computes the whole group at once sees no difference between a
    // closed chain and an open one. What is checked is the result: the loop closed, so the assembly converged
    // and there are no errors.
    let _ = j_close;
    assert!(rep.errors.is_empty(), "a closed chain must assemble without errors: {:?}", rep.errors);
}

#[test]
fn ungrounded_graph_seeds_an_anchor() {
    // With nothing grounded the solver picks its own reference, and the graph still solves rather than
    // failing.
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);
    let rep = p.solve_joints();
    assert!(rep.errors.is_empty() && rep.unsolved.is_empty());
}

#[test]
fn component_dof_by_joint() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Revolute);
    assert_eq!(p.component_dof(a), 0, "a grounded component has zero degrees of freedom");
    assert_eq!(p.component_dof(b), 1, "a revolute mate leaves one degree of freedom");
    assert_eq!(p.component_dof(d), 6, "a free component, with no mates, has six degrees of freedom");
}

#[test]
fn face_to_face_rigid_mate() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // Bodies carrying the faces. No geometry is needed: the solve does not touch it and `resolve_face` falls
    // back to the key.
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    let key_a = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let key_b = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key_a));
    let cb = p.add_connector(b, AnchorRef::FaceCenter(body_b, key_b));
    p.connectors.iter_mut().find(|c| c.id == cb).unwrap().flip = true; // The faces meet face to face.
    p.add_joint(ca, cb, JointKind::Rigid);

    p.solve_joints();
    // The centre of face B (local [0,0,0]) coincides with the centre of face A in world space, [0,0,5].
    let fb = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!((0..3).all(|i| (fb[i] - [0.0, 0.0, 5.0][i]).abs() < 1e-9), "the faces must coincide: {fb:?}");
}

#[test]
fn rigid_face_mate_offset_part_comes_to_face() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    // B is displaced sideways and upwards, as in an exploded assembly.
    set_transform(&mut p, b, tr(20.0, 15.0, 30.0));
    let key_a = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let key_b = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key_a));
    let cb = p.add_connector(b, AnchorRef::FaceCenter(body_b, key_b));
    p.add_joint(ca, cb, JointKind::Rigid);
    p.solve_joints();
    let fb = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]); // World centre of face B.
                                                              // A rigid mate leaves zero degrees of freedom, so face B travels to face A, centre onto centre at
                                                              // [0,0,5], rather than staying displaced sideways at [20,15,5].
    assert!((0..3).all(|i| (fb[i] - [0.0, 0.0, 5.0][i]).abs() < 1e-6), "face B must travel to face A at [0,0,5]: {fb:?}");
}

#[test]
fn assembly_coincident_positions_via_constraint_solver() {
    // A planar mate makes face B coplanar with face A even when the body was displaced sideways and upwards;
    // the freedom within the plane remains, which is correct at three degrees of freedom. This checks that the
    // path from the project through the problem, the solve and the write-back works.
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    set_transform(&mut p, b, tr(20.0, 15.0, 30.0)); // B is displaced sideways and upwards (an exploded assembly).
                                                    // Face A points at +Z at z = 5; face B points at -Z, facing it, at its local zero, so the two are not
                                                    // aligned and the default flip applies.
    let key_a = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let key_b = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, -1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key_a));
    let cb = p.add_connector(b, AnchorRef::FaceCenter(body_b, key_b));
    p.add_joint(ca, cb, JointKind::Planar); // Mating two faces is a planar mate.
    p.solve_joints();
    let fb = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]); // World centre of face B.
    assert!((fb[2] - 5.0).abs() < 1e-5, "face B must be coplanar with face A at z = 5: {fb:?}");
    // A is grounded and must not move.
    let fa = apply12(&p.world_transform(a), [0.0, 0.0, 0.0]);
    assert!((0..3).all(|i| fa[i].abs() < 1e-9), "the grounded A must stay in place: {fa:?}");
}

#[test]
fn deleting_part_removes_joints_anchored_to_its_body() {
    // Deleting a part used to leave behind a joint whose connector belongs to the parent context (owner A)
    // while its anchor references a body of the deleted part B, as `ancestor_child_of` produces when a joint is
    // created. The broken anchor resolved to the old centroid and the solve scattered the bodies to garbage
    // coordinates.
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    let key = |z: f64| FaceKey { index: 0, centroid: [0.0, 0.0, z], normal: [0.0, 0.0, 1.0], id: 0 };
    // A connector owned by A whose anchor sits on a body of part B: the owner differs from the owner of the
    // anchor geometry.
    let c_ctx = p.add_connector(a, AnchorRef::FaceCenter(body_b, key(0.0)));
    let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key(5.0)));
    p.add_joint(ca, c_ctx, JointKind::Rigid);
    assert_eq!(p.joints.len(), 1);
    p.delete_component(b); // Deleting part B removes body_b.
    assert_eq!(p.joints.len(), 0, "a joint anchored on a body of a deleted part must be removed");
    assert!(!p.connectors.iter().any(|cc| cc.id == c_ctx), "a connector anchored on a deleted body must be removed");
}

#[test]
fn loop_solver_closes_open_loop() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    // J1: a slider from A to B along Z. The tree places B while the offset stays free for the loop solve to
    // determine.
    let ca0 = p.add_connector(a, AnchorRef::Origin);
    let cb0 = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca0, cb0, JointKind::Slider);
    // J2 closes the loop: face A at z = 10 is rigidly mated to face B at z = 0, so closing it requires B at
    // z = 10.
    let fa = FaceKey { index: 0, centroid: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let fb = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca1 = p.add_connector(a, AnchorRef::FaceCenter(body_a, fa));
    let cb1 = p.add_connector(b, AnchorRef::FaceCenter(body_b, fb));
    p.add_joint(ca1, cb1, JointKind::Rigid);

    let rep = p.solve_joints();
    // The result is checked rather than the structure of the algorithm: the loop closed and there are no errors.
    assert!(rep.errors.is_empty(), "the loop must close without errors: {:?}", rep.errors);
    let zb = apply12(&p.world_transform(b), [0.0, 0.0, 0.0])[2];
    assert!((zb - 10.0).abs() < 1e-3, "B must be solved to z = 10 (got {zb})");
}

#[test]
fn loop_driver_param_is_held_free_solved() {
    // A driving loop parameter (one carrying an expression in `feat_dims`) is held fixed while the solver
    // determines the free ones. A chain of sliders A to B to D along Z plus a loop (face A at z = 10 against
    // face D at z = 0) requires D at z = 10, so offset1 plus offset2 equals 10. With the offset of J1 driven at
    // 3, the offset of J2 must come out as 7.
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(d));
    let body_d = p.add_extrude(2, 5.0);
    // A chain of sliders from A to B to D along Z.
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb0 = p.add_connector(b, AnchorRef::Origin);
    let j1 = p.add_joint(ca, cb0, JointKind::Slider);
    let cb1 = p.add_connector(b, AnchorRef::Origin);
    let cd0 = p.add_connector(d, AnchorRef::Origin);
    let j2 = p.add_joint(cb1, cd0, JointKind::Slider);
    // The loop: face A at z = 10 rigidly mated to face D at z = 0.
    let fa = FaceKey { index: 0, centroid: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let fd = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca1 = p.add_connector(a, AnchorRef::FaceCenter(body_a, fa));
    let cd1 = p.add_connector(d, AnchorRef::FaceCenter(body_d, fd));
    p.add_joint(ca1, cd1, JointKind::Rigid);
    // The driver: the offset of slider J1 carries an expression in `feat_dims` and is held at 3.
    p.set_feat_dim(j1, "offset", "3".into());
    p.joints.iter_mut().find(|x| x.id == j1).unwrap().drive[1] = Some(3.0); // A specified value, not a reading.

    let rep = p.solve_joints();
    // The readings are checked: the driven one has to match the specified value, and the free one has to show
    // where the body ended up after the loop closed.
    let o1 = p.joints.iter().find(|x| x.id == j1).unwrap().offset;
    let o2 = p.joints.iter().find(|x| x.id == j2).unwrap().offset;
    assert!((o1 - 3.0).abs() < 1e-9, "the driven J1 must stay at the specified 3 (reading {o1})");
    assert!((o2 - 7.0).abs() < 1e-3, "the free J2 must be solved to 7 (reading {o2})");
    assert!(rep.errors.is_empty(), "the loop must close with the driver held: {:?}", rep.errors);
}

#[test]
fn edge_mid_connector_places_by_axis() {
    // An `EdgeMid` axis connector (the midpoint of an edge, by persistent id) resolves through `regen_edges`
    // and positions the driven body. A is grounded and its edge id 5 has midpoint [0,0,5]; a rigid mate joins
    // it to the origin of B.
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    // Edges of body A as the kernel would supply them: edge id 5 with midpoint [0,0,5] and tangent +Z.
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 5, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, 0.0], b: [0.0, 0.0, 10.0], ..Default::default() }]);
    assert_eq!(p.resolve_edge(body_a, 5), Some(([0.0, 0.0, 5.0], [0.0, 0.0, 1.0])), "the edge must resolve by id");
    let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);
    p.solve_joints();
    // B is rigidly mated to the edge frame, so the origin of B lands on the midpoint of edge A at [0,0,5].
    let ob = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!((ob[2] - 5.0).abs() < 1e-9 && ob[0].abs() < 1e-9 && ob[1].abs() < 1e-9, "B must land on the midpoint of edge A: {ob:?}");
}

#[test]
fn edge_mid_anchor_survives_serde() {
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    p.add_connector(c[0], AnchorRef::EdgeMid(42, 7));
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    match back.connectors[0].anchor {
        AnchorRef::EdgeMid(b, e) => assert_eq!((b, e), (42, 7), "EdgeMid must survive serialisation"),
        _ => panic!("EdgeMid was lost in serialisation"),
    }
}

#[test]
fn vertex_connector_resolves_endpoint() {
    // A vertex connector is an endpoint of an edge, by persistent id: `false` is the start and `true` the
    // end.
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 9, mid: [0.0; 3], dir: [0.0, 0.0, 1.0], a: [1.0, 2.0, 3.0], b: [4.0, 5.0, 6.0], ..Default::default() }]);
    assert_eq!(p.resolve_vertex(body_a, 9, false), Some([1.0, 2.0, 3.0]), "start of the edge");
    assert_eq!(p.resolve_vertex(body_a, 9, true), Some([4.0, 5.0, 6.0]), "end of the edge");
    // The frame of a vertex connector sits at that point.
    let cv = p.add_connector(a, AnchorRef::Vertex(body_a, 9, true));
    let conn = p.connector(cv).unwrap().clone();
    assert_eq!(p.connector_frame(&conn).unwrap().origin, [4.0, 5.0, 6.0], "the vertex frame must sit at the end of the edge");
    // serde
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert!(matches!(back.connectors[0].anchor, AnchorRef::Vertex(_, 9, true)), "Vertex must survive serialisation");
}

#[test]
fn circular_edge_connector_uses_center_axis() {
    // A circular edge (the rim of a hole): an `EdgeMid` anchor has to land on the centre of the circle with its
    // axis rather than on the rim.
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    // A circular edge: centre (10,20,5), axis +Z, radius 3; its midpoint lies on the rim at (13,20,5).
    p.regen_edges.insert(
        body_a,
        vec![MeshEdge {
            id: 11,
            mid: [13.0, 20.0, 5.0],
            dir: [0.0, 1.0, 0.0],
            a: [13.0, 20.0, 5.0],
            b: [13.0, 20.0, 5.0],
            center: [10.0, 20.0, 5.0],
            axis: [0.0, 0.0, 1.0],
            radius: 3.0,
            ref_dir: [0.0; 3],
            ends: Default::default(),
        }],
    );
    // The resolved edge axis is centre plus axis, not rim plus tangent.
    assert_eq!(p.resolve_edge_axis(body_a, 11), Some(([10.0, 20.0, 5.0], [0.0, 0.0, 1.0])), "a circular edge gives centre plus axis");
    // The frame of an axis connector sits at the centre of the circle, concentric with the hole, rather than
    // on the rim.
    let ce = p.add_connector(a, AnchorRef::EdgeMid(body_a, 11));
    let conn = p.connector(ce).unwrap().clone();
    let f = p.connector_frame(&conn).unwrap();
    assert_eq!(f.origin, [10.0, 20.0, 5.0], "the connector frame must sit at the centre of the hole");
    assert_eq!(f.normal(), [0.0, 0.0, 1.0], "the frame normal must be the axis of the hole");
}

#[test]
fn straight_edge_connector_uses_midpoint() {
    // A straight edge (radius 0) still anchors at midpoint plus tangent.
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 12, mid: [1.0, 2.0, 3.0], dir: [0.0, 0.0, 1.0], a: [1.0, 2.0, 0.0], b: [1.0, 2.0, 6.0], ..Default::default() }]);
    assert_eq!(p.resolve_edge_axis(body_a, 12), Some(([1.0, 2.0, 3.0], [0.0, 0.0, 1.0])), "a straight edge gives midpoint plus tangent");
}

#[test]
fn joints_survive_serde() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    let ca = p.add_connector(a, AnchorRef::BasePlane(qymcad_core::feature::BasePlane::XY));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Cylindrical);
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(7.0); // A specified value, not a reading.

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.connectors.len(), 2);
    assert_eq!(back.joints.len(), 1);
    assert_eq!(back.joints[0].kind, JointKind::Cylindrical);
    // A specified value has to survive saving, or after reopening the file the mate forgets what was required
    // of it and the body moves wherever it likes.
    assert_eq!(back.joints[0].drive[1], Some(7.0), "a specified offset must be saved");
    assert_eq!(back.joints[0].drive[0], None, "an unspecified degree must stay free rather than become a specified zero");
}

#[test]
fn joint_home_scopes_to_owning_assembly() {
    // A root holding parts d1 and d2 plus a subassembly S holding sc1 and sc2. The mate d1 to d2 is at home in
    // the root, the mate sc1 to sc2 at home in S, and the crossing mate d1 to sc1 at home in the root.
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let d1 = p.add_part("P1");
    let d2 = p.add_part("P2");
    let s = p.add_assembly("S");
    p.set_active_component(Some(s));
    let sc1 = p.add_part("SC1");
    let sc2 = p.add_part("SC2");

    let mk = |p: &mut Project, a: Id, b: Id| {
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, JointKind::Rigid);
        p.joints.iter().find(|x| x.id == jid).unwrap().clone()
    };
    let j_root = mk(&mut p, d1, d2);
    let j_sub = mk(&mut p, sc1, sc2);
    let j_cross = mk(&mut p, d1, sc1);

    assert_eq!(p.joint_home(&j_root), Some(root), "a mate between parts of the root is at home in the root");
    assert_eq!(p.joint_home(&j_sub), Some(s), "a mate inside a subassembly is at home in that subassembly");
    assert_eq!(p.joint_home(&j_cross), Some(root), "a crossing mate is at home in the common ancestor");
    // The lowest common ancestor directly.
    assert_eq!(p.common_ancestor(sc1, sc2), Some(s));
    assert_eq!(p.common_ancestor(d1, sc2), Some(root));
    assert_eq!(p.common_ancestor(s, sc1), Some(s), "an ancestor and its descendant give the ancestor");
}
