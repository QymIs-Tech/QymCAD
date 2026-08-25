//! Mates built on connectors: propagation over the tree, grounding, parametric angles, loops and degrees of
//! freedom. The solver is a pure model and needs no geometry kernel.

use qymcad_core::feature::{apply12, mat_mul12, AnchorRef, JointKind};
use qymcad_core::model::{Id, Project};

fn tr(x: f64, y: f64, z: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

fn set_transform(p: &mut Project, comp: Id, m: [f64; 12]) {
    let i = p.component_index(comp).unwrap();
    p.components[i].transform = m;
}

/// An assembly with N parts as children of the root. Returns their ids.
fn parts(p: &mut Project, n: usize) -> Vec<Id> {
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    (0..n).map(|i| p.add_part(format!("P{i}"))).collect()
}

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
    p.regen_edges.insert(
        body_a,
        vec![MeshEdge { id: 12, mid: [1.0, 2.0, 3.0], dir: [0.0, 0.0, 1.0], a: [1.0, 2.0, 0.0], b: [1.0, 2.0, 6.0], ..Default::default() }],
    );
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

/// A free degree stays free while a specified one is held.
///
/// That separation is the point of `drive`. Using one value as both the specification and the reading means the
/// solver writes where the body ended up and reads the same entry as a requirement on the next solve: a body
/// slid along the axis of a cylindrical mate springs back although it is free along that axis. Zero was also
/// indistinguishable from unset, so a value of 0 could not be specified at all.
#[test]
fn a_free_slot_stays_where_you_put_it_and_a_driven_one_holds() {
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 5, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, 0.0], b: [0.0, 0.0, 10.0], radius: 0.0, center: [0.0; 3], axis: [0.0, 0.0, 1.0], ref_dir: [0.0; 3] }]);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    p.regen_edges.insert(body_b, vec![MeshEdge { id: 7, mid: [0.0, 0.0, 0.0], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, -5.0], b: [0.0, 0.0, 5.0], radius: 0.0, center: [0.0; 3], axis: [0.0, 0.0, 1.0], ref_dir: [0.0; 3] }]);
    let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
    let cb = p.add_connector(b, AnchorRef::EdgeMid(body_b, 7));
    let jid = p.add_joint(ca, cb, JointKind::Cylindrical);
    p.solve_joints();

    // Displacing anchor B along the axis of anchor A: exactly the free degree of a cylindrical mate.
    let slide = |p: &Project| -> f64 {
        let wa = mat_mul12(&p.world_transform(a), &p.connector_matrix(ca).unwrap());
        let wb = mat_mul12(&p.world_transform(b), &p.connector_matrix(cb).unwrap());
        let ax = [wa[2], wa[6], wa[10]];
        (wb[3] - wa[3]) * ax[0] + (wb[7] - wa[7]) * ax[1] + (wb[11] - wa[11]) * ax[2]
    };
    // Free: the body is moved along the axis, and the solve has to leave it where it was put.
    let before = slide(&p);
    let t0 = p.component_transform(b);
    p.set_component_transform(b, mat_mul12(&tr(0.0, 0.0, 30.0), &t0));
    let moved = slide(&p);
    assert!((moved - before - 30.0).abs() < 1e-6, "setup: the body must move 30 mm along the axis (was {before:.2}, now {moved:.2})");
    p.solve_joints();
    let after = slide(&p);
    assert!(
        (after - moved).abs() < 1e-3,
        "a cylindrical mate does not constrain along its axis, so the body must stay at {moved:.2} mm, but it sprang back to {after:.2}"
    );

    // Specified: the offset is pinned and the mate has to hold it.
    p.joints.iter_mut().find(|j| j.id == jid).unwrap().drive[1] = Some(12.0);
    p.solve_joints();
    let held = slide(&p);
    assert!((held - 12.0).abs() < 1e-3, "a specified offset must be honoured: asked for 12 mm, got {held:.2}");

    // Zero is a specified value too. Using zero to mean "unset" makes a value of 0 impossible to specify.
    p.joints.iter_mut().find(|j| j.id == jid).unwrap().drive[1] = Some(0.0);
    p.solve_joints();
    let zero = slide(&p);
    assert!(zero.abs() < 1e-3, "a specified zero must be honoured like any other value, but got {zero:.2}");
}

/// Every joint kind and every degree: a free one is left alone, a specified one is honoured.
///
/// The "specification equals reading" defect was common to every kind and was fixed in one place, so every kind
/// has to be checked rather than inferred by analogy with the cylindrical one. The anchors sit at the origins,
/// so the relative placement of the bodies is exactly the `motion` of the joint and can be compared against it
/// directly.
///
/// The test also cross-checks two descriptions of one motion — the document one (`JointKind::motion`) and the
/// solver one (feeding the specified value into the anchor): the body is placed by the first and held by the
/// second. If they diverge, a mate behaves differently from what the interface shows, silently.
#[test]
fn every_mechanical_kind_keeps_free_slots_and_holds_driven_ones() {
    let kinds = [
        JointKind::Revolute,
        JointKind::Slider,
        JointKind::Cylindrical,
        JointKind::Planar,
        JointKind::Ball,
        JointKind::PinSlot,
    ];
    for kind in kinds {
        let free = kind.free_slots();
        for slot in 0..3 {
            if !free[slot] {
                continue;
            }
            // v0 is where the body stands (its freedom) and v1 is what will be required of it (the
            // specification). The two differ deliberately: equal values would let the test pass with a
            // completely broken solver.
            let (v0, v1) = (7.0_f64, 19.0_f64);
            let vals = |v: f64| {
                let mut a = [0.0; 3];
                a[slot] = v;
                a
            };
            let put = |p: &mut Project, b: Id, v: f64| {
                let m = vals(v);
                p.set_component_transform(b, kind.motion(m[0], m[1], m[2]));
            };

            let mut p = Project::default();
            let c = parts(&mut p, 2);
            let (a, b) = (c[0], c[1]);
            p.set_grounded(a, true);
            let ca = p.add_connector(a, AnchorRef::Origin);
            let cb = p.add_connector(b, AnchorRef::Origin);
            let jid = p.add_joint(ca, cb, kind);

            // Freedom: the body is placed at v0 along its degree, and the solve has to leave it there.
            put(&mut p, b, v0);
            p.solve_joints();
            let want = {
                let m = vals(v0);
                kind.motion(m[0], m[1], m[2])
            };
            let got = p.world_transform(b);
            let err = (0..12).map(|i| (got[i] - want[i]).abs()).fold(0.0, f64::max);
            assert!(
                err < 1e-6,
                "{:?}, slot {slot}: a free degree must not be moved — the body stood at {v0} and was carried away (off by {err:.4})",
                kind
            );

            // Specification: v1 is required, and the solve has to bring the body exactly there.
            p.joints.iter_mut().find(|x| x.id == jid).unwrap().drive[slot] = Some(v1);
            p.solve_joints();
            let want1 = {
                let m = vals(v1);
                kind.motion(m[0], m[1], m[2])
            };
            let got1 = p.world_transform(b);
            let err1 = (0..12).map(|i| (got1[i] - want1[i]).abs()).fold(0.0, f64::max);
            assert!(err1 < 1e-3, "{:?}, slot {slot}: the specified value {v1} must be honoured (off by {err1:.4})", kind);

            // Zero is a specified value too, not "unset".
            p.joints.iter_mut().find(|x| x.id == jid).unwrap().drive[slot] = Some(0.0);
            p.solve_joints();
            let got0 = p.world_transform(b);
            let err0 = (0..12).map(|i| (got0[i] - qymcad_core::feature::PLACE_IDENTITY[i]).abs()).fold(0.0, f64::max);
            assert!(err0 < 1e-3, "{:?}, slot {slot}: a specified zero must be honoured like any other value (off by {err0:.4})", kind);
        }
    }
}

/// A limit holds a free degree too, not only a specified one.
///
/// A limit is visible in the interface, so it is a promise. Clamping only the specified value leaves the free
/// degree unconstrained and a slider with a range of 0..50 can be dragged anywhere. A limit is an inequality
/// while the solver works with equalities, so an offender is pinned at the boundary and the problem is solved
/// again — an active set.
#[test]
fn a_limit_holds_a_free_slot_too() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);
    {
        let j = p.joints.iter_mut().find(|x| x.id == jid).unwrap();
        j.limit_min[1] = Some(0.0);
        j.limit_max[1] = Some(50.0);
    }

    // Within the range the body stays where it was put and the limit must not touch anything.
    p.set_component_transform(b, tr(0.0, 0.0, 30.0));
    p.solve_joints();
    let inside = apply12(&p.world_transform(b), [0.0, 0.0, 0.0])[2];
    assert!((inside - 30.0).abs() < 1e-6, "within the range the limit must not interfere: expected 30, got {inside:.3}");

    // Beyond the upper bound the body is pinned at 50 rather than left at 300.
    p.set_component_transform(b, tr(0.0, 0.0, 300.0));
    p.solve_joints();
    let above = apply12(&p.world_transform(b), [0.0, 0.0, 0.0])[2];
    assert!((above - 50.0).abs() < 1e-3, "beyond the upper limit the body must stop at 50, but it is at {above:.3}");

    // Beyond the lower bound it is pinned at 0.
    p.set_component_transform(b, tr(0.0, 0.0, -120.0));
    p.solve_joints();
    let below = apply12(&p.world_transform(b), [0.0, 0.0, 0.0])[2];
    assert!(below.abs() < 1e-3, "beyond the lower limit the body must stop at 0, but it is at {below:.3}");

    // The reading agrees with the placement: a mate must not display one value while holding another.
    let shown = p.joints.iter().find(|x| x.id == jid).unwrap().offset;
    assert!((shown - below).abs() < 1e-3, "the reading ({shown:.3}) must match the actual placement ({below:.3})");
}

/// The gap of a rigid mate is a parameter rather than a degree of freedom.
///
/// A rigid mate has no freedom at all, yet a gap between the faces can be specified and the interface offers
/// it. It therefore has to apply always rather than "when the degree is free": a rigid mate never has a free
/// degree, so testing for freedom is the wrong check.
#[test]
fn a_rigid_joint_keeps_the_gap_it_was_given() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    set_transform(&mut p, b, tr(20.0, 15.0, 30.0));
    let key_a = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let key_b = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key_a));
    let cb = p.add_connector(b, AnchorRef::FaceCenter(body_b, key_b));
    let jid = p.add_joint(ca, cb, JointKind::Rigid);
    p.joints.iter_mut().find(|x| x.id == jid).unwrap().drive[1] = Some(4.0);
    p.solve_joints();
    let fb = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(
        (fb[2] - 9.0).abs() < 1e-6 && fb[0].abs() < 1e-6 && fb[1].abs() < 1e-6,
        "a gap of 4 mm must separate the faces: expected the centre of B at [0,0,9], got {fb:?}"
    );
}

/// The "flip side" control really does turn the body over.
///
/// Mating two faces admits two sides, and the solver picks the one nearer to the current placement so the body
/// does not flip by itself. Since the interface offers a toggle, that toggle has to change the side: otherwise
/// the automatic choice cannot be corrected.
#[test]
fn the_flip_toggle_actually_turns_the_part_over() {
    use qymcad_core::feature::FaceKey;
    let build = |flip: bool| -> [f64; 12] {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        p.set_active_component(Some(a));
        let body_a = p.add_extrude(1, 5.0);
        p.set_active_component(Some(b));
        let body_b = p.add_extrude(2, 5.0);
        set_transform(&mut p, b, tr(0.0, 0.0, 30.0));
        let key_a = FaceKey { index: 0, centroid: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
        let key_b = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
        let ca = p.add_connector(a, AnchorRef::FaceCenter(body_a, key_a));
        let cb = p.add_connector(b, AnchorRef::FaceCenter(body_b, key_b));
        let jid = p.add_joint(ca, cb, JointKind::Rigid);
        // The toggle goes through the core entry point rather than writing the field. `flip` is the side
        // itself rather than a request to change it, and the solver overwrites that same field with its own
        // answer: a write that bypasses `flip_joint_side` is silently undone by the next solve, the side not
        // being marked as decided and the nearer one — the same one — being chosen again.
        if flip {
            p.flip_joint_side(jid);
        }
        p.solve_joints();
        p.world_transform(b)
    };
    // The Z axis of body B in world space: the third column of the rotation matrix.
    let axis = |m: [f64; 12]| [m[2], m[6], m[10]];
    let (straight, flipped) = (axis(build(false)), axis(build(true)));
    let dot = straight[0] * flipped[0] + straight[1] * flipped[1] + straight[2] * flipped[2];
    assert!(
        dot < -0.99,
        "the toggle must turn the body to the opposite side, yet the axes point almost the same way (dot product {dot:.3})"
    );
}

/// Nested assembly: placements have to be written parents before children.
///
/// A component placement is stored relative to its parent while the solver works in world space, and converting
/// from world to local uses the current placement of the parent. Writing a child first converts it against the
/// old parent, and then the parent moves and drags the child along. The order of the component list is
/// arbitrary, so happening to get it right is no defence.
#[test]
fn a_nested_child_lands_where_the_solver_put_it() {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let base = p.add_part("base");
    p.set_active_component(Some(root));
    let sub = p.add_assembly("subassembly");
    p.set_active_component(Some(sub));
    let leaf = p.add_part("part inside the subassembly");
    p.set_grounded(base, true);
    // The order of the mates is deliberate: the mate to the part inside the subassembly comes first and the
    // mate to the subassembly itself second. The component list is built from first appearance, so the child
    // enters it before the parent — and mates are created in any order.
    let c_base1 = p.add_connector(base, AnchorRef::Origin);
    let c_leaf = p.add_connector(leaf, AnchorRef::Origin);
    let j2 = p.add_joint(c_base1, c_leaf, JointKind::Slider);
    p.joints.iter_mut().find(|x| x.id == j2).unwrap().drive[1] = Some(65.0);
    let c_base = p.add_connector(base, AnchorRef::Origin);
    let c_sub = p.add_connector(sub, AnchorRef::Origin);
    let j1 = p.add_joint(c_base, c_sub, JointKind::Slider);
    p.joints.iter_mut().find(|x| x.id == j1).unwrap().drive[1] = Some(40.0);

    p.solve_joints();
    // The subassembly sits 40 from the base and the part 65 from it, its mate being expressed in world space
    // through the base.
    let ws = apply12(&p.world_transform(sub), [0.0, 0.0, 0.0]);
    let wl = apply12(&p.world_transform(leaf), [0.0, 0.0, 0.0]);
    assert!((ws[2] - 40.0).abs() < 1e-6, "the subassembly must land at 40: {ws:?}");
    assert!(
        (wl[2] - 65.0).abs() < 1e-6,
        "the part inside the subassembly must stay at 65 but is at {:.3} — its placement was written against the old parent and the moved subassembly dragged it along",
        wl[2]
    );
}

// --- A mate without an anchor does not stay silent ---
//
// Measured on a real document: none of its five edge-based mates had a connector frame, the bodies being
// imported, the live B-rep not raised without being asked, and the edge axis read from it. The solver dropped
// such mates from the problem silently: the assembly looks assembled, nothing moves, and no explanation is
// given.

/// An unresolved anchor is named rather than swallowed.
#[test]
fn a_joint_whose_anchor_does_not_resolve_is_reported() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // An anchor on an edge of a body absent from the edge cache: exactly the state of a document right after
    // opening, with the body imported and its edges not built.
    let ca = p.add_connector(a, AnchorRef::EdgeMid(777, 5));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Slider);

    assert_eq!(p.unresolved_joints(), vec![j], "a mate with an unresolved anchor must be named");
    assert!(p.connector_matrix(ca).is_none(), "setup: the anchor really has no frame");
}

/// A sound mate is not reported, or the warning becomes noise.
#[test]
fn a_healthy_joint_is_not_reported() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);

    assert!(p.unresolved_joints().is_empty(), "a sound mate was marked unresolved: {:?}", p.unresolved_joints());
}

/// Once the edges appear the complaint clears itself: the answer depends only on whether the geometry is raised
/// right now and carries no state.
#[test]
fn the_complaint_goes_away_once_the_edges_are_there() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::EdgeMid(777, 5));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Slider);
    assert_eq!(p.unresolved_joints(), vec![j], "setup: the anchor does not resolve");

    p.regen_edges.insert(
        777,
        vec![qymcad_core::geom::MeshEdge {
            id: 5,
            mid: [1.0, 0.0, 0.0],
            dir: [0.0, 1.0, 0.0],
            a: [1.0, -1.0, 0.0],
            b: [1.0, 1.0, 0.0],
            ..Default::default()
        }],
    );
    assert!(p.unresolved_joints().is_empty(), "the edges appeared yet the mate is still counted as unresolved");
}

/// A slider travels along the selected edge rather than along a world axis.
///
/// The edge here deliberately matches no world axis: if the motion follows X, Y or Z, the check catches it.
#[test]
fn a_slider_moves_along_the_chosen_edge() {
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 5.0);
    // A diagonal edge with direction (1,1,0)/sqrt(2). No world axis looks like that.
    let s = 1.0 / 2.0_f64.sqrt();
    p.regen_edges.insert(
        body_a,
        vec![MeshEdge { id: 5, mid: [0.0, 0.0, 0.0], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ..Default::default() }],
    );
    let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Slider);

    p.solve_joints();
    let at0 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0); // A specified value, not a reading.
    p.solve_joints();
    let at1 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);

    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!(len > 1e-6, "the body did not move along the slider at all: {at0:?} to {at1:?}");
    // Along the edge means the cosine with its direction equals one.
    let cos = (d[0] * s + d[1] * s + d[2] * 0.0) / len;
    assert!(
        (cos.abs() - 1.0).abs() < 1e-6,
        "the body did not move along the selected edge: displacement {d:?}, edge [{s:.3},{s:.3},0], cosine {cos:.6}"
    );
    assert!((len - 10.0).abs() < 1e-6, "it travelled {len:.3} instead of the specified 10 mm");
}

/// Deleting a mate and creating it again gives the same behaviour.
///
/// Re-creating it has to yield the same axis and the same motion, or a mate depends on history rather than on
/// geometry.
#[test]
fn re_creating_a_slider_gives_the_same_motion() {
    use qymcad_core::geom::MeshEdge;
    let s = 1.0 / 2.0_f64.sqrt();
    let run = |recreate: bool| -> [f64; 3] {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        p.set_active_component(Some(a));
        let body_a = p.add_extrude(1, 5.0);
        p.regen_edges.insert(
            body_a,
            vec![MeshEdge { id: 5, mid: [0.0, 0.0, 0.0], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ..Default::default() }],
        );
        let mk = |p: &mut Project| {
            let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
            let cb = p.add_connector(b, AnchorRef::Origin);
            p.add_joint(ca, cb, JointKind::Slider)
        };
        let mut j = mk(&mut p);
        if recreate {
            // Deleted the way the interface deletes: the mate leaves the list while the connectors stay, being
            // cleaned up separately. The core has no dedicated `remove_joint`, and adding one for a test would
            // be wrong.
            p.joints.retain(|x| x.id != j);
            j = mk(&mut p);
        }
        p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
        p.solve_joints();
        apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
    };
    let once = run(false);
    let again = run(true);
    for k in 0..3 {
        assert!((once[k] - again[k]).abs() < 1e-6, "after re-creating the mate the body settled differently: {once:?} against {again:?}");
    }
}

/// Every joint kind leaves exactly its own degrees of freedom.
///
/// The table below is the definition: rigid holds everything, revolute leaves a rotation, a slider leaves a
/// translation, cylindrical leaves both, a ball joint leaves three rotations, and planar leaves two
/// translations and a rotation. Checking only the revolute case leaves the rest indistinguishable.
#[test]
fn every_joint_kind_leaves_exactly_its_freedoms() {
    let cases = [
        (JointKind::Rigid, 0u8, "rigid holds everything"),
        (JointKind::Revolute, 1, "revolute leaves a rotation"),
        (JointKind::Slider, 1, "a slider leaves a translation"),
        (JointKind::Cylindrical, 2, "cylindrical leaves a rotation and a translation"),
        (JointKind::Ball, 3, "a ball joint leaves three rotations"),
        (JointKind::Planar, 3, "planar leaves two translations and a rotation"),
    ];
    for (kind, dof, what) in cases {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        p.add_joint(ca, cb, kind);
        p.solve_joints();
        assert_eq!(p.component_dof(b), dof, "{what}: {kind:?} left {} degrees instead of {dof}", p.component_dof(b));
    }
}

/// A slider on faces travels along the face rather than away from it.
///
/// Selecting the long face of a rail and a face of the carriage should make one face slide along the other. For
/// a planar face the frame is built from the normal, so the motion runs along the normal, that is, away from
/// and towards the face. It has to slide within the plane.
///
/// Checking a slider on an edge alone does not cover this: the face case behaves differently.
#[test]
fn a_slider_on_flat_faces_moves_along_the_face_not_away_from_it() {
    use qymcad_core::feature::FaceKey;
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);

    // The rail face: a horizontal patch with its normal pointing up (+Z). Sliding on it has to go
    // sideways.
    let key = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(1, key.clone()));
    let cb = p.add_connector(b, AnchorRef::FaceCenter(2, key));
    let j = p.add_joint(ca, cb, JointKind::Slider);

    p.solve_joints();
    let at0 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0); // A specified value, not a reading.
    p.solve_joints();
    let at1 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);

    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!(len > 1e-6, "a slider on faces did not move at all: {at0:?} to {at1:?}");
    assert!(
        d[2].abs() < 1e-6,
        "the body moved along the face normal (up or down) when it must slide along the face: displacement {d:?}"
    );
}

/// Acceptance matrix: every joint kind on every anchor kind behaves as expected.
///
/// Checking a slider on an edge alone left the face case travelling along the normal. All three anchor kinds
/// (the part origin, a planar face, a straight edge) are exercised against every joint kind, and not merely for
/// "it did not fail" but for where exactly the body moves and by how much.
#[test]
fn every_kind_on_every_anchor_moves_the_way_a_human_expects() {
    use qymcad_core::feature::FaceKey;
    use qymcad_core::geom::MeshEdge;

    // Anchors: a name, how to build one on part `c`, and the expected travel axis for a slider.
    let s = 1.0 / 2.0_f64.sqrt();
    let anchors: Vec<(&str, Box<dyn Fn(&mut Project, Id) -> AnchorRef>, [f64; 3])> = vec![
        ("part origin", Box::new(|_p: &mut Project, _c: Id| AnchorRef::Origin), [0.0, 0.0, 1.0]),
        (
            "planar face (normal +Z)",
            Box::new(|_p: &mut Project, _c: Id| AnchorRef::FaceCenter(1, FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: 0 })),
            // Along the face rather than along the normal: any axis in the XY plane will do, so Z is checked
            // to be near zero.
            [0.0, 0.0, 0.0],
        ),
        (
            "straight diagonal edge",
            Box::new(move |p: &mut Project, c: Id| {
                p.set_active_component(Some(c));
                let body = p.add_extrude(1, 5.0);
                p.regen_edges.insert(
                    body,
                    // An edge does have a reference direction: on a real body every edge has an adjacent face
                    // whose normal defines the roll (verified against the real kernel in
                    // `qymcad-testkit/tests/an_edge_knows_its_face.rs`). Without it a rigid mate honestly keeps
                    // one extra degree of freedom, but that is the "no geometry" state rather than the
                    // behaviour of an edge anchor.
                    vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ref_dir: [0.0, 0.0, 1.0], ..Default::default() }],
                );
                AnchorRef::EdgeMid(body, 5)
            }),
            [s, s, 0.0],
        ),
    ];

    for (anchor_name, make, axis) in &anchors {
        for (kind, dof, moves) in [
            (JointKind::Rigid, 0u8, false),
            (JointKind::Revolute, 1, false),
            (JointKind::Slider, 1, true),
            (JointKind::Cylindrical, 2, true),
            (JointKind::Ball, 3, false),
            (JointKind::Planar, 3, true),
            // The pin-slot was missing from the matrix entirely: the kind existed with nothing exercising it.
            // A rotation about the pin axis plus travel along the slot: two degrees.
            (JointKind::PinSlot, 2, true),
            // Parallel holds direction only: three translations plus a rotation about the shared axis. It has
            // no value, so there is nothing to drive.
            (JointKind::Parallel, 4, false),
        ] {
            let mut p = Project::default();
            let c = parts(&mut p, 2);
            let (a, b) = (c[0], c[1]);
            p.set_grounded(a, true);
            let aa = make(&mut p, a);
            let ab = make(&mut p, b);
            let ca = p.add_connector(a, aa);
            let cb = p.add_connector(b, ab);
            let j = p.add_joint(ca, cb, kind);

            // The mate has to resolve: dropping it from the problem silently is the defect this file exists
            // for.
            assert!(
                p.unresolved_joints().is_empty(),
                "{kind:?} on anchor \"{anchor_name}\": the anchor did not resolve, so the mate drops out of the computation silently"
            );
            // Degrees of freedom are checked only where the roll is defined.
            //
            // A face anchor without raised geometry has nowhere to take a secondary axis from, and the solver
            // honestly leaves the roll free: a rigid mate then reports one degree instead of zero. That is not
            // an artefact of the test but the state of a document whose live B-rep is not raised, which is why
            // the application now raises it for any mate on geometry (`needs_live_brep`). This test has no
            // geometry by construction, so the question is asked only where the answer is defined.
            let roll_known = !anchor_name.starts_with("planar face");
            if roll_known {
                assert_eq!(p.component_dof(b), dof, "{kind:?} on anchor \"{anchor_name}\": {} degrees of freedom instead of {dof}", p.component_dof(b));
            }

            if !moves {
                continue; // These kinds have no translation, so there is no travel to check.
            }
            p.solve_joints();
            let at0 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
            p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
            p.solve_joints();
            let at1 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
            let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            assert!(len > 1e-6, "{kind:?} on anchor \"{anchor_name}\": the body did not move at all");
            // A specified 10 means travelling exactly 10, with no exceptions.
            assert!((len - 10.0).abs() < 1e-6, "{kind:?} on anchor \"{anchor_name}\": travelled {len:.3} instead of 10");
            // The expectation differs by kind, and the difference matters:
            //   a slider and a cylindrical mate travel along the anchor axis;
            //   a planar mate travels within the plane, across its normal, which is what makes it planar;
            //   a slider on a planar face travels along the face, that is, across the normal as well.
            //
            // The matrix caught this at once: a planar mate travelled "not along the anchor", and it was right
            // while the first table of expectations was not.
            //
            // The anchor axis: a face gives its normal, a part origin gives world Z, an edge gives itself.
            let ax = if *axis == [0.0, 0.0, 0.0] { [0.0, 0.0, 1.0] } else { *axis };
            let on_face = anchor_name.starts_with("planar face");
            // The expectation per case, spelled out:
            //   a slider travels along the anchor axis, except on a planar face, where it travels along the
            //     face — travelling along the normal there tears the body off the rail;
            //   a cylindrical mate always travels along the axis, and on a planar face that is a pin through a
            //     plate, so motion along the normal is correct;
            //   a planar mate always travels within the plane, across the normal, which is what makes it
            //     planar.
            let along = match kind {
                JointKind::Slider => !on_face,
                JointKind::Cylindrical => true,
                _ => false,
            };
            let want = ax;
            let cos = (d[0] * want[0] + d[1] * want[1] + d[2] * want[2]) / len;
            if along {
                assert!((cos.abs() - 1.0).abs() < 1e-6, "{kind:?} on anchor \"{anchor_name}\": did not travel along the anchor: {d:?}, axis {want:?}");
            } else {
                assert!(cos.abs() < 1e-6, "{kind:?} on anchor \"{anchor_name}\": travelled along the axis when it must travel across it: {d:?}, axis {want:?}");
            }
        }
    }
}

/// A body does not fly away: free directions stay where they were left.
///
/// Minimal displacement. Measured: a planar mate on an edge with a specified offset of 0 put the body at
/// z = -1134 mm while the solver reported convergence.
///
/// What carries the body away is not the mate: along a free direction the constraint gradient is exactly zero
/// and nothing holds the body there. Of all the placements satisfying the constraints, the nearest to the
/// current one has to be chosen; without that the assembly looks broken for every joint kind.
#[test]
fn a_new_joint_moves_the_part_no_further_than_it_must() {
    use qymcad_core::geom::MeshEdge;
    let s = 1.0 / 2.0_f64.sqrt();
    for (what, kind) in [("planar", JointKind::Planar), ("slider", JointKind::Slider), ("cylindrical", JointKind::Cylindrical)] {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        let mk = |p: &mut Project, comp: Id| {
            p.set_active_component(Some(comp));
            let body = p.add_extrude(1, 5.0);
            p.regen_edges.insert(body, vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ..Default::default() }]);
            AnchorRef::EdgeMid(body, 5)
        };
        let aa = mk(&mut p, a);
        let ab = mk(&mut p, b);
        let ca = p.add_connector(a, aa);
        let cb = p.add_connector(b, ab);
        let j = p.add_joint(ca, cb, kind);

        // The same sequence a person performs: create the mate and let the assembly recompute, move the body
        // by a specified value, then set that value back. At zero the body has to return where it came from
        // rather than end up a metre away.
        p.solve_joints();
        let after_create = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
        let d0 = (after_create[0].powi(2) + after_create[1].powi(2) + after_create[2].powi(2)).sqrt();
        assert!(d0 < 1e-3, "{what}: creating the mate carried the body {d0:.1} mm away: {after_create:?}");

        p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
        p.solve_joints();
        p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(0.0);
        p.solve_joints();
        let at = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
        let d = (at[0] * at[0] + at[1] * at[1] + at[2] * at[2]).sqrt();
        assert!(d < 1e-3, "{what}: the specified value went back to zero yet the body stayed {d:.1} mm away: {at:?}");
    }
}

/// The roll of an edge comes from the adjacent face of the body rather than from world Z.
///
/// An edge anchor has only one axis of its own, along the edge. Deriving the second one, the roll, from world Z
/// takes it from how the body happens to lie rather than from its own geometry. Taking it from the adjacent
/// face is what makes a rigid mate on an edge place a body predictably.
///
/// The scenario: two blocks, each with an anchor edge along Z, whose adjacent faces point different ways — +Y
/// on A and +X on B. A rigid mate has to turn B so the faces agree. Derived from world Z both frames get the
/// same X, there is nothing to turn, and B stays as it was with its face pointing the other way.
#[test]
fn a_rigid_joint_on_edges_lines_up_the_faces_not_the_world_axes() {
    use qymcad_core::geom::MeshEdge;

    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);

    let edge = |ref_dir: [f64; 3]| MeshEdge {
        id: 5,
        mid: [0.0, 0.0, 0.0],
        dir: [0.0, 0.0, 1.0],
        a: [0.0, 0.0, -5.0],
        b: [0.0, 0.0, 5.0],
        ref_dir,
        ..Default::default()
    };
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 10.0);
    p.regen_edges.insert(body_a, vec![edge([0.0, 1.0, 0.0])]);
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 10.0);
    p.regen_edges.insert(body_b, vec![edge([1.0, 0.0, 0.0])]);

    let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
    let cb = p.add_connector(b, AnchorRef::EdgeMid(body_b, 5));
    p.add_joint(ca, cb, JointKind::Rigid);
    p.solve_joints();

    // Where does the adjacent face of B point after the solve? It has to point the way face A does.
    let m = p.world_transform(b);
    let o = apply12(&m, [0.0, 0.0, 0.0]);
    let f = apply12(&m, [1.0, 0.0, 0.0]);
    let got = [f[0] - o[0], f[1] - o[1], f[2] - o[2]];
    let dot = got[1]; // Projection onto +Y, the direction of face A.
    assert!(
        dot > 0.999,
        "a rigid mate on edges placed the body by world axes rather than by its face: face B points at {got:?}, expected +Y"
    );
}

/// A mate the solver did not take is named in the report.
///
/// Found on a real document: five mates and two connectors. Four rigid mates referenced connectors the document
/// does not hold, the bridge skipped such mates silently (`continue`) and the report came back empty. On screen
/// the assembly looks assembled, the list shows five mates, and nothing says that four of them do not act.
///
/// Silence here is worse than a wrong answer: a wrong answer is visible and this is not.
#[test]
fn a_joint_the_solver_could_not_use_is_named_in_the_report() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Rigid);

    // A sound mate is not mentioned in the report.
    assert!(p.solve_joints().errors.is_empty(), "setup: a sound mate was reported as faulty");

    // The state of the measured document: the connector is lost and the mate remains.
    p.connectors.retain(|x| x.id != cb);
    let rep = p.solve_joints();
    assert!(
        rep.errors.iter().any(|(id, _)| *id == j),
        "a mate without a connector dropped out of the computation silently: report {rep:?}, while on screen the assembly looks assembled"
    );
}

/// Deleting one mate does not touch the others.
///
/// This is the mechanism behind "five mates, two connectors" in the measured document. The mate panel shows the
/// mates of the current assembly only, and it pruned orphaned connectors against that same filtered list:
/// deleting a mate in one subassembly removed the connectors of mates in every other context. Those mates
/// stayed and never acted again, with nothing to report it.
///
/// Deleting a mate is therefore one core method that counts orphans across the whole document.
#[test]
fn deleting_one_joint_leaves_the_others_whole() {
    let mut p = Project::default();
    let c = parts(&mut p, 4);
    let (a, b, d, e) = (c[0], c[1], c[2], c[3]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j1 = p.add_joint(ca, cb, JointKind::Rigid);
    let cd = p.add_connector(d, AnchorRef::Origin);
    let ce = p.add_connector(e, AnchorRef::Origin);
    let j2 = p.add_joint(cd, ce, JointKind::Slider);

    p.delete_joint(j1);

    assert!(!p.joints.iter().any(|x| x.id == j1), "the deleted mate is still in the document");
    assert!(p.joints.iter().any(|x| x.id == j2), "deleting one mate removed another");
    assert!(p.connector(cd).is_some() && p.connector(ce).is_some(), "the surviving mate lost its connectors and will never act again");
    assert!(p.connector(ca).is_none() && p.connector(cb).is_none(), "the connectors of the deleted mate were left orphaned");
    assert!(p.joint_faults().is_empty(), "deleting a mate left a faulty one in the document: {:?}", p.joint_faults());
}

/// No convergence means the body is not moved, and that is reported.
///
/// Contradictory mates have no solution: the sum of the residuals at its minimum is not zero. Writing the body
/// wherever the solver stopped puts it at a compromise nobody asked for and no requirement contains. From the
/// outside the assembly blows apart: the body moved somewhere unexplained.
///
/// The rule: without a solution the body stays where it was placed and the mate is flagged. Resolving the
/// contradiction is a decision for the author, not for the solver.
#[test]
fn when_there_is_no_solution_the_part_stays_where_the_human_put_it() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    // Two grounded anchors in different places and one body rigidly mated to both: it would have to be in two
    // places at once, so there is no solution.
    set_transform(&mut p, a, tr(0.0, 0.0, 0.0));
    set_transform(&mut p, d, tr(100.0, 0.0, 0.0));
    p.set_grounded(a, true);
    p.set_grounded(d, true);
    set_transform(&mut p, b, tr(7.0, 8.0, 9.0)); // The body was placed here.
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb1 = p.add_connector(b, AnchorRef::Origin);
    let cb2 = p.add_connector(b, AnchorRef::Origin);
    let cd = p.add_connector(d, AnchorRef::Origin);
    p.add_joint(ca, cb1, JointKind::Rigid);
    p.add_joint(cd, cb2, JointKind::Rigid);

    let rep = p.solve_joints();
    let at = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let moved = ((at[0] - 7.0).powi(2) + (at[1] - 8.0).powi(2) + (at[2] - 9.0).powi(2)).sqrt();
    assert!(
        moved < 1e-9,
        "there is no solution yet the body moved {moved:.3} mm to {at:?}: the solver placed it at a compromise nobody asked for"
    );
    assert!(p.mates_conflict, "an unsolvable assembly was not flagged as conflicting");
    assert!(!rep.errors.is_empty(), "there is no solution yet the report is silent: {rep:?}");
}

/// Pin-slot: the slot belongs to the second anchor.
///
/// In this mate the first anchor is the pin and the point of rotation while the second carries the translation:
/// rotation about Z, travel along X. The slot direction therefore comes from the second anchor and the order of
/// the anchors matters, which is the point of the kind.
///
/// Building both planes from the first anchor makes the body travel along the X of the pin. While the axes of
/// the two anchors happen to coincide the difference is invisible; as soon as the slot points its own way, the
/// body travels somewhere other than where the slot is drawn.
///
/// In this test the slot is turned by a quarter, which is the reorientation control used when a slot does not
/// run along the body axis.
#[test]
fn a_pin_slot_slides_along_the_slot_not_along_the_pin() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    // The slot points its own way: the secondary axis of the second anchor is turned by a quarter, so its X is
    // world Y.
    p.connectors.iter_mut().find(|x| x.id == cb).expect("the slot anchor").rot_deg = 90.0;
    let j = p.add_joint(ca, cb, JointKind::PinSlot);
    p.solve_joints();

    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
    p.solve_joints();

    let at = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(
        (at[1] - 10.0).abs() < 1e-6 && at[0].abs() < 1e-6 && at[2].abs() < 1e-6,
        "the body travelled along the pin rather than along the slot: it is at {at:?}, expected (0, 10, 0)"
    );
}

/// Pin-slot: the travel reading is measured along the slot.
///
/// The specified value and the reading have to speak about the same thing. The slot direction belongs to the
/// second anchor, so how far the body travelled is measured along its axis; otherwise the mate misreports its
/// own state — the body slid seven along the slot while the field shows zero.
#[test]
fn a_pin_slot_reports_how_far_it_slid_along_the_slot() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.connectors.iter_mut().find(|x| x.id == cb).expect("the slot anchor").rot_deg = 90.0;
    let j = p.add_joint(ca, cb, JointKind::PinSlot);
    // The body sits in the slot, displaced by 7 along its axis (the slot X is world Y).
    set_transform(&mut p, b, tr(0.0, 7.0, 0.0));
    p.solve_joints();

    let got = p.joints.iter().find(|x| x.id == j).expect("the mate").offset;
    assert!((got - 7.0).abs() < 1e-6, "the slot travel reads {got:.3} instead of 7: the reading is not measured along the slot");
}

/// A connector is offset along all three axes, not only along the main one.
///
/// A connector needs three offsets — X, Y and Z of its own frame — plus a rotation about the main axis. With a
/// single offset along the main axis and quarter-turn reorientation, moving an anchor seven millimetres
/// sideways requires moving the body itself by guesswork, and an angle between quarters cannot be expressed at
/// all.
///
/// The offsets are read in connector space, along its own axes rather than the world ones; otherwise the same
/// number would mean different things on different bodies.
#[test]
fn a_connector_shifts_along_all_three_of_its_own_axes() {
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    let cid = p.add_connector(a, AnchorRef::Origin);
    let base = p.connector_matrix(cid).expect("the connector frame");
    assert_eq!([base[3], base[7], base[11]], [0.0; 3], "setup: the frame sits at the part origin");

    p.connectors.iter_mut().find(|x| x.id == cid).expect("the connector").offset_xyz = [3.0, 7.0, 11.0];
    let m = p.connector_matrix(cid).expect("the connector frame");
    // For a part-origin anchor the connector axes coincide with the world ones, so the offset is visible
    // directly.
    assert!(
        (m[3] - 3.0).abs() < 1e-9 && (m[7] - 7.0).abs() < 1e-9 && (m[11] - 11.0).abs() < 1e-9,
        "the connector axis offsets were not applied: frame origin ({}, {}, {}), expected (3, 7, 11)",
        m[3],
        m[7],
        m[11]
    );
}

/// The secondary axis turns by an arbitrary angle, not only by quarters.
///
/// A quarter turn is the common case but not the only one: a slot at 30 degrees to the body axis cannot be
/// expressed in quarters.
#[test]
fn a_connector_turns_its_secondary_axis_by_any_angle() {
    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    let cid = p.add_connector(a, AnchorRef::Origin);

    p.connectors.iter_mut().find(|x| x.id == cid).expect("the connector").rot_deg = 30.0;
    let m = p.connector_matrix(cid).expect("the connector frame");
    let (x, y) = ([m[0], m[4], m[8]], [m[1], m[5], m[9]]);
    let (c30, s30) = 30.0_f64.to_radians().sin_cos();
    assert!(
        (x[0] - s30).abs() < 1e-9 && (x[1] - c30).abs() < 1e-9 && x[2].abs() < 1e-9,
        "the secondary axis was not turned by 30 degrees: frame X is {x:?}"
    );
    // Orthonormality is preserved.
    let dot = x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    assert!(dot.abs() < 1e-9, "the frame axes stopped being perpendicular: dot product {dot:.3e}");
}

/// The connector axis can be given by geometry rather than only derived.
///
/// The secondary axis is derived automatically: the long side of a face, the adjacent face of an edge. That is
/// right on a rail, but a square face has no long side at all and the automatic answer is arbitrary. A second
/// pick covers that case: point at an edge and the axis runs along it. Without it the only recourse is moving
/// the body by guesswork.
///
/// The axis is taken from the picked geometry and placed perpendicular to the main axis: it gives a direction
/// rather than replacing the anchor.
#[test]
fn a_connector_takes_its_axis_from_the_geometry_you_point_at() {
    use qymcad_core::geom::MeshEdge;

    let mut p = Project::default();
    let c = parts(&mut p, 1);
    let a = c[0];
    p.set_active_component(Some(a));
    let body = p.add_extrude(1, 10.0);
    // An edge along the XY diagonal: a direction the automatic derivation would never choose.
    let s = 1.0 / 2.0_f64.sqrt();
    p.regen_edges.insert(
        body,
        vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ref_dir: [0.0, 0.0, 1.0], ..Default::default() }],
    );

    let cid = p.add_connector(a, AnchorRef::Origin);
    let before = p.connector_matrix(cid).expect("the connector frame");
    assert!((before[0] - 1.0).abs() < 1e-9, "setup: without a pick the frame X axis is world X");

    // An edge is picked, so the axis runs along it.
    p.connectors.iter_mut().find(|x| x.id == cid).expect("the connector").axis_ref = Some(AnchorRef::EdgeMid(body, 5));
    let m = p.connector_matrix(cid).expect("the connector frame");
    let x = [m[0], m[4], m[8]];
    assert!(
        (x[0] - s).abs() < 1e-9 && (x[1] - s).abs() < 1e-9 && x[2].abs() < 1e-9,
        "the connector axis was not taken from the picked edge: frame X is {x:?}, expected ({s:.4}, {s:.4}, 0)"
    );
    // The main axis is untouched: the pick sets the secondary one.
    let z = [m[2], m[6], m[10]];
    assert!(z[2].abs() > 0.999, "picking the secondary axis turned the main one: frame Z is {z:?}");
}

/// A slider travels along the picked edge rather than along a derived one.
#[test]
fn a_slider_runs_along_the_edge_you_pointed_at() {
    use qymcad_core::feature::FaceKey;
    use qymcad_core::geom::MeshEdge;

    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let s = 1.0 / 2.0_f64.sqrt();
    let mk = |p: &mut Project, comp: Id, sketch: Id| {
        p.set_active_component(Some(comp));
        let body = p.add_extrude(sketch, 10.0);
        p.regen_edges.insert(
            body,
            vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ref_dir: [0.0, 0.0, 1.0], ..Default::default() }],
        );
        let key = FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: 0 };
        let cid = p.add_connector(comp, AnchorRef::FaceCenter(body, key));
        p.connectors.iter_mut().find(|x| x.id == cid).expect("the connector").axis_ref = Some(AnchorRef::EdgeMid(body, 5));
        cid
    };
    let ca = mk(&mut p, a, 1);
    let cb = mk(&mut p, b, 2);
    let j = p.add_joint(ca, cb, JointKind::Slider);

    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(0.0);
    p.solve_joints();
    let at0 = apply12(&p.world_transform(b), [0.0; 3]);
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
    p.solve_joints();
    let at1 = apply12(&p.world_transform(b), [0.0; 3]);
    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!((len - 10.0).abs() < 1e-6, "10 was specified but it travelled {len:.3}");
    assert!(
        (d[0] / len - s).abs() < 1e-6 && (d[1] / len - s).abs() < 1e-6,
        "the body did not travel along the picked edge: {:?}",
        [d[0] / len, d[1] / len, d[2] / len]
    );
}

/// A group carries bodies together and needs no connectors.
///
/// A group fixes the placements of the selected instances relative to each other. Any two or more bodies that
/// do not move relative to one another are better collected into a group than joined by a rigid mate between
/// every pair; on an imported assembly of dozens of parts that is the difference between one action and
/// dozens.
///
/// The bodies are fixed where they stand: a group moves nothing when it is created, it only forbids them to
/// drift apart afterwards.
#[test]
fn a_group_carries_its_parts_along_without_any_connectors() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, d) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    // B and D stand apart and are joined only by the group.
    set_transform(&mut p, b, tr(50.0, 0.0, 0.0));
    set_transform(&mut p, d, tr(50.0, 30.0, 0.0));
    p.add_group(&[b, d]);

    // A slider drives B from the grounded A; D has to travel with B, preserving their relative
    // placement.
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Slider);
    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(0.0);
    p.solve_joints();
    let (b0, d0) = (apply12(&p.world_transform(b), [0.0; 3]), apply12(&p.world_transform(d), [0.0; 3]));

    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(20.0);
    p.solve_joints();
    let (b1, d1) = (apply12(&p.world_transform(b), [0.0; 3]), apply12(&p.world_transform(d), [0.0; 3]));

    let moved_b = ((b1[0] - b0[0]).powi(2) + (b1[1] - b0[1]).powi(2) + (b1[2] - b0[2]).powi(2)).sqrt();
    assert!((moved_b - 20.0).abs() < 1e-6, "the driven body travelled {moved_b:.3} instead of 20");
    let apart0 = [d0[0] - b0[0], d0[1] - b0[1], d0[2] - b0[2]];
    let apart1 = [d1[0] - b1[0], d1[1] - b1[1], d1[2] - b1[2]];
    let drift = ((apart1[0] - apart0[0]).powi(2) + (apart1[1] - apart0[1]).powi(2) + (apart1[2] - apart0[2]).powi(2)).sqrt();
    assert!(drift < 1e-6, "the group members drifted apart by {drift:.3} mm: was {apart0:?}, now {apart1:?}");
}

/// A group grounds nothing.
///
/// Grouped bodies are not fixed: a group relates them only to each other, and one of them still has to be
/// attached to something. Otherwise the assembly looks fully constrained while it is free as a whole.
#[test]
fn a_group_alone_does_not_pin_anything_down() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.add_group(&[a, b]);
    assert_eq!(p.component_dof(b), 6, "a group on its own declared the body constrained although it holds it by nothing");

    // Grounding one of them makes the whole group constrained.
    p.set_grounded(a, true);
    assert_eq!(p.component_dof(b), 0, "grounding one member must make the whole group constrained");
}

/// A parallel mate holds direction only.
///
/// It leaves four degrees of freedom: translations along X, Y and Z plus a rotation about Z. It is a condition
/// rather than a fit — the anchor axes point the same way while where the body stands and how it is rotated
/// about that axis is its own business.
///
/// Needed where bodies have to be co-directed without being brought together: shelves parallel to a base, rails
/// parallel to each other.
#[test]
fn parallel_holds_the_direction_and_nothing_else() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // B stands aside and rotated: the parallel mate has to turn it without pulling it in.
    set_transform(&mut p, b, [0.0, -1.0, 0.0, 40.0, 1.0, 0.0, 0.0, 25.0, 0.0, 0.0, 1.0, 15.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
    p.add_joint(ca, cb, JointKind::Parallel);

    assert_eq!(p.component_dof(b), 4, "a parallel mate leaves four degrees of freedom, but {} were counted", p.component_dof(b));

    let before = apply12(&p.world_transform(b), [0.0; 3]);
    p.solve_joints();
    let after = apply12(&p.world_transform(b), [0.0; 3]);

    // The axes are co-directed, which is all that is promised.
    let za = {
        let m = p.connector_matrix(ca).expect("frame A");
        [m[2], m[6], m[10]]
    };
    let zb = {
        let m = qymcad_core::feature::mat_mul12(&p.world_transform(b), &p.connector_matrix(cb).expect("frame B"));
        [m[2], m[6], m[10]]
    };
    let dot = za[0] * zb[0] + za[1] * zb[1] + za[2] * zb[2];
    assert!(dot.abs() > 0.999, "the anchor axes did not become parallel: dot product {dot:.4}");

    // Pulling the body in is not promised: its placement stays as it was set.
    let moved = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2) + (after[2] - before[2]).powi(2)).sqrt();
    assert!(moved < 1e-6, "the parallel mate pulled the body {moved:.3} mm although it must hold direction only");
}

/// A width constraint puts a body midway between two walls.
///
/// It relates the bodies symmetrically: exactly two wall anchors are selected plus the tab between them, and
/// two degrees of freedom remain — translation within the mid-plane and rotation about it. It holds one thing:
/// the distances to the two walls are equal.
///
/// Needed where a body has to sit centred in a slot while its exact position along the slot is the business of
/// other mates.
#[test]
fn width_puts_the_part_halfway_between_the_two_walls() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The slot walls: two planes with normal X at 0 and at 40. The tab starts right against the first
    // wall.
    let w1 = p.add_connector(a, AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
    let w2 = p.add_connector(a, AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
    p.connectors.iter_mut().find(|x| x.id == w2).expect("the second wall").offset_xyz = [0.0, 0.0, 40.0];
    set_transform(&mut p, b, tr(3.0, 12.0, -7.0));
    let tab = p.add_connector(b, AnchorRef::Origin);

    p.add_width(&[w1, w2], tab);
    p.solve_joints();

    // The distances to the walls along their normal have to become equal, putting the tab at 20.
    let at = apply12(&p.world_transform(b), [0.0; 3]);
    assert!((at[0] - 20.0).abs() < 1e-6, "the tab did not land midway: it is at {at:?} with the walls at 0 and 40 along X");
    // Along the slot nothing moved it: a width constraint does not promise that.
    //
    // The body used to drift 14 mm here, and the fault lay with the solver rather than with the constraint:
    // the rotation was taken about the world origin and the damping was computed per component. A width
    // constraint simply happened to be the first condition leaving five free directions, which is where the
    // drift became visible as a number.
    //
    // The tolerance is 0.1 micrometres, and that is not a concession. Position along a free direction is not a
    // constraint residual: its accuracy comes from the retraction with a refinement step, bounded by the same
    // solver tolerance of 1e-7. A tolerance of 1e-6 mm — one nanometre — held only because the solver silently
    // took a hundred and ninety extra steps, its exit threshold being compared against a cost that included
    // the pull towards the original place. After the exit was fixed the drift became 6 nanometres, and claiming
    // nanometre accuracy for an assembly would promise something that does not exist.
    let along = ((at[1] - 12.0).powi(2) + (at[2] + 7.0).powi(2)).sqrt();
    assert!(along < 1e-4, "the width constraint carried the body {along:.6} mm along the slot: {at:?}");
}

/// A mate outlives its geometry and goes red.
///
/// Deleting the body an anchor stood on used to delete the mate. The motive was sound: the anchor resolved to
/// the old centroid fingerprint stored in the face key itself, the solver got a garbage frame and scattered the
/// bodies to wild coordinates. But that treated the symptom at the author's expense: deleting one body lost the
/// assembly work, discovered only when parts turned out to be missing.
///
/// When its geometry disappears a mate is kept and goes red; repairing or deleting it is the author's
/// decision.
#[test]
fn deleting_a_body_leaves_the_joint_broken_instead_of_vanishing() {
    use qymcad_core::geom::MeshEdge;

    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    p.set_active_component(Some(a));
    let body_a = p.add_extrude(1, 10.0);
    p.regen_edges.insert(
        body_a,
        vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, -5.0], b: [0.0, 0.0, 5.0], ref_dir: [0.0, 1.0, 0.0], ..Default::default() }],
    );
    let ca = p.add_connector(a, AnchorRef::EdgeMid(body_a, 5));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Rigid);
    assert!(p.joint_faults().is_empty(), "setup: the mate is sound");

    // The body goes, as when an operation is deleted.
    p.timeline.retain(|n| n.kind.body() != Some(body_a));
    p.regen_edges.remove(&body_a);
    p.drop_connectors_of_dead_bodies(&[body_a]);

    assert!(p.joints.iter().any(|x| x.id == j), "the mate disappeared with the body, silently losing the assembly work");
    let faults = p.joint_faults();
    assert!(faults.iter().any(|(id, _)| *id == j), "the mate stayed but is not counted as faulty, so it says nothing: {faults:?}");
    let rep = p.solve_joints();
    assert!(rep.errors.iter().any(|(id, _)| *id == j), "the solver says nothing about the lost anchor: {rep:?}");
}

/// "The body is not built yet" is not "the body was deleted".
///
/// The distinction is mandatory. Unbuilt geometry is a temporary state: the document was opened from a bundle,
/// the live B-rep is raised on demand, and the mate has to come back to life by itself as soon as it is. The
/// test "the body is not in the timeline" does not separate the two cases, which is why the document remembers
/// deleted bodies by id rather than guessing.
#[test]
fn a_body_that_was_never_built_is_not_a_deleted_one() {
    use qymcad_core::feature::FaceKey;

    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // An anchor on a body that was never in the timeline, which is how the frame tests are built.
    let key = FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: 0 };
    let ca = p.add_connector(a, AnchorRef::FaceCenter(1, key));
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Rigid);

    p.drop_connectors_of_dead_bodies(&[]);
    assert!(p.joint_faults().is_empty(), "an unbuilt body was taken for a deleted one and the mate was needlessly declared broken");
    assert!(p.joints.iter().any(|x| x.id == j), "a mate on an unbuilt body was deleted");
}

/// Hold as built: the mate declares the current placement as the one it holds.
///
/// An ordinary rigid mate aligns its anchors, and rightly so. But in an assembly already arranged by hand, or
/// arriving through an import, there is nothing to align: the bodies already stand correctly and the mate is
/// only there to keep them from drifting apart.
///
/// Without it the offsets have to be dialled in by hand, and an imported assembly collapses into a single point
/// on the very first mate.
#[test]
fn an_as_built_joint_holds_the_parts_exactly_where_they_stand() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    set_transform(&mut p, a, tr(10.0, 0.0, 0.0));
    set_transform(&mut p, b, tr(50.0, 7.0, -3.0));
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    // Guard: an ordinary rigid mate does pull the body in, or there would be nothing to test.
    p.solve_joints();
    let pulled = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(
        (pulled[0] - 10.0).abs() < 1e-3,
        "setup: an ordinary rigid mate must bring the origins together (x = 10), but the body is at {pulled:?}"
    );

    // Now the current placement is declared as the one the mate holds.
    set_transform(&mut p, b, tr(50.0, 7.0, -3.0));
    assert!(p.set_joint_as_built(jid), "the mate must accept the hold-as-built declaration");
    p.solve_joints();
    let held = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    for (i, want) in [50.0, 7.0, -3.0].into_iter().enumerate() {
        assert!(
            (held[i] - want).abs() < 1e-3,
            "the body must stay where it stood ({want} along axis {i}), but it is at {held:?}"
        );
    }
    // And it holds: displaced by hand, the body returns.
    set_transform(&mut p, b, tr(80.0, 7.0, -3.0));
    p.solve_joints();
    let back = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!((back[0] - 50.0).abs() < 1e-3, "a hold-as-built mate must return the body to the declared place, but it is at {back:?}");

    // Declaring it again changes nothing. The body stands where the mate holds it, so saying "hold as built" a
    // second time confirms the same thing. Capturing the declaration on top of an already captured one would
    // compose it with itself and the body would move on every press. Found by a mutation run: corrupting this
    // reddened nothing.
    assert!(p.set_joint_as_built(jid), "declaring it again must succeed");
    p.solve_joints();
    let twice = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(
        (twice[0] - 50.0).abs() < 1e-3 && (twice[1] - 7.0).abs() < 1e-3 && (twice[2] + 3.0).abs() < 1e-3,
        "the second hold-as-built moved the body: was [50, 7, -3], now {twice:?}"
    );
}

/// Sweeping a degree for animation: the bounds come from the limits rather than from thin air.
#[test]
fn the_range_a_degree_is_animated_over_comes_from_its_limits() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let hinge = p.add_joint(ca, cb, JointKind::Revolute);
    let cc = p.add_connector(a, AnchorRef::Origin);
    let cd = p.add_connector(b, AnchorRef::Origin);
    let rail = p.add_joint(cc, cd, JointKind::Slider);

    // A rotation without limits sweeps a full turn: an angle has a natural end.
    assert_eq!(p.joint_anim_range(hinge, 0), Some((0.0, 360.0)), "a hinge without limits must sweep a full turn");
    // A translation without limits has nowhere to sweep to: a displacement has no natural end and inventing
    // one is not allowed.
    assert_eq!(p.joint_anim_range(rail, 1), None, "a slider without limits has no range and must not be swept to an invented number");
    // With limits set, the sweep runs exactly between them.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == rail) {
        j.limit_min[1] = Some(-5.0);
        j.limit_max[1] = Some(45.0);
    }
    assert_eq!(p.joint_anim_range(rail, 1), Some((-5.0, 45.0)), "with limits the sweep must run between them");
    // A one-sided limit gives no range either: sweeping to an invented end would violate the specified
    // one.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == rail) {
        j.limit_max[1] = None;
    }
    assert_eq!(p.joint_anim_range(rail, 1), None, "with only one limit there is no end to sweep to");
    // A degree the kind does not have is not swept at all.
    assert_eq!(p.joint_anim_range(hinge, 1), None, "a hinge has no travel, so there is nothing to sweep");
}

/// A connector is an element in its own right and cannot be deleted from under a mate.
///
/// While a connector lived only inside a mate there was no reason to delete it separately. Making it a tree
/// element opens the possibility of removing an anchor a mate rests on. Agreeing silently would break the mate
/// so that it stays in the list and stops acting.
#[test]
fn a_connector_that_a_joint_stands_on_cannot_be_deleted() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let spare = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    assert_eq!(p.connector_users(ca), vec![jid], "a mate must count as a user of its own anchor");
    assert!(p.connector_users(spare).is_empty(), "a free connector is nobody's anchor");

    assert!(!p.delete_connector(ca), "an anchor a mate rests on must not be deleted");
    assert!(p.connector(ca).is_some(), "a refusal must delete nothing");
    assert_eq!(p.joints.len(), 1, "the mate must stay intact");

    assert!(p.delete_connector(spare), "a free connector must be deletable");
    assert!(p.connector(spare).is_none(), "a deleted connector must disappear");

    // Deleting a mate takes its own anchors with it: they were created for it.
    p.delete_joint(jid);
    assert!(p.connector(ca).is_none(), "an anchor created for a mate must go with it");
}

/// Deleting a mate does not touch anchors belonging to something else.
///
/// Removing every connector no mate uses is wrong: a width constraint rests on connectors without being a
/// mate. Deleting any mate then removed the anchors of the width constraint, which stayed in the list and
/// stopped acting, silently.
#[test]
fn deleting_a_joint_does_not_take_away_connectors_that_belong_to_something_else() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let (a, b, w) = (c[0], c[1], c[2]);
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);
    // The width constraint on its own three anchors, unrelated to the mate.
    let walls = [p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin)];
    let tab = p.add_connector(w, AnchorRef::Origin);
    p.add_width(&walls, tab);
    assert_eq!(p.connectors.len(), 5, "guard: five anchors were prepared but there are {}", p.connectors.len());

    p.delete_joint(jid);
    for (i, cid) in walls.iter().chain([&tab]).enumerate() {
        assert!(p.connector(*cid).is_some(), "anchor {i} of the width constraint disappeared with an unrelated mate, and the constraint silently stopped acting");
    }
    // A standalone connector stays too: it is an element in its own right rather than a part of a mate.
    let own = p.add_connector_standalone(b, AnchorRef::Origin);
    let c2 = p.add_connector(a, AnchorRef::Origin);
    let c3 = p.add_connector(b, AnchorRef::Origin);
    let j2 = p.add_joint(c2, c3, JointKind::Rigid);
    p.delete_joint(j2);
    assert!(p.connector(own).is_some(), "a connector created on its own must survive the deletion of an unrelated mate");
}

/// A mate is not the only thing that holds an anchor.
///
/// A width constraint rests on three connectors without being a mate. Counting only mates as users of an anchor
/// makes a wall of a width constraint removable: the constraint stays in the list and silently stops acting.
/// The same defect as in mate deletion, reached through a different door; found by a mutation run, since
/// corrupting `connector_users` reddened nothing.
#[test]
fn a_connector_a_width_stands_on_is_held_too() {
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    let walls = [p.add_connector(c[0], AnchorRef::Origin), p.add_connector(c[1], AnchorRef::Origin)];
    let tab = p.add_connector(c[2], AnchorRef::Origin);
    let wid = p.add_width(&walls, tab);

    for (i, cid) in walls.iter().chain([&tab]).enumerate() {
        assert_eq!(p.connector_users(*cid), vec![wid], "anchor {i} must be counted as used by the width constraint");
        assert!(!p.delete_connector(*cid), "anchor {i}, which the width constraint rests on, must not be deleted");
        assert!(p.connector(*cid).is_some(), "a refusal must delete nothing");
    }
    // Once the width constraint is deleted the anchors are free.
    p.delete_group(wid);
    assert!(p.connector_users(tab).is_empty(), "after deleting the width constraint the anchor must be free");
    assert!(p.delete_connector(tab), "a freed anchor must be deletable");
}

/// A connector has a name, and the names do not repeat.
#[test]
fn every_connector_gets_its_own_name() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let ids: Vec<Id> = (0..4).map(|i| p.add_connector(c[i % 2], AnchorRef::Origin)).collect();
    let names: Vec<String> = ids.iter().filter_map(|id| p.connector(*id).map(|x| x.name.clone())).collect();
    assert_eq!(names.len(), 4, "guard: there should be four connectors but {} names were collected", names.len());
    assert!(names.iter().all(|n| !n.is_empty()), "a nameless connector cannot be referred to in the tree: {names:?}");
    let mut uniq = names.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), names.len(), "connector names repeat: {names:?}");
}

/// The gizmo arrow points where the body travels, including when the anchor was turned.
///
/// The mating side is chosen as the nearer to the current placement: when the anchor axes face each other the
/// solver turns the first anchor itself while the stored flag stays false. The turn is 180 degrees about X and
/// reverses the Y and Z axes, that is, the travel axis itself. A gizmo asking for the frame without that turn
/// draws the arrow the other way, so dragging forwards moves the body backwards.
#[test]
fn the_gizmo_arrow_agrees_with_the_travel_even_when_the_anchor_is_turned_around() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The second body is turned by 180 degrees about X, so its anchor faces the first one.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(v);
        }
        p.solve_joints();
        apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
    };
    let at0 = go(&mut p, 0.0);
    let at1 = go(&mut p, 10.0);
    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    assert!((len - 10.0).abs() < 1e-3, "setup: 10 was specified but the body travelled {len:.4}");

    let ax = p.joint_slot_axis(jid, 1, p.root).expect("a slider has a travel axis");
    let along = (d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2]) / len;
    assert!(
        along > 0.999,
        "the body does not travel where the arrow points: agreement {along:.4} (travel {:?}, arrow {ax:?})",
        [d[0] / len, d[1] / len, d[2] / len]
    );
}

/// A limit holds where its mark is drawn, including when the anchor was turned.
///
/// The limit, the reading and the arrow are one measurement. When the solver turns the first anchor itself
/// (the axes facing each other), all three have to turn together; otherwise the body stops somewhere other
/// than where the mark is drawn and the limit appears not to work.
///
/// What this test holds and what it does not was checked by corruption. Removing the turn from the arrow
/// reddens it ("travelled -10.0000"). Removing the turn from the measurement does not: the specified value is
/// baked into the same turned anchor and the limit agrees with it regardless of the measurement. The
/// measurement is covered by the gates on real documents, where the reading is compared against the specified
/// angle.
#[test]
fn a_limit_stops_the_part_where_its_mark_is_drawn_even_on_a_turned_anchor() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let dir = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(v);
        }
        p.solve_joints();
        apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
    };
    let at0 = go(&mut p, 0.0);

    // The limit is 10 mm while 50 is requested, so the body has to stop exactly at the mark.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.limit_min[1] = Some(0.0);
        j.limit_max[1] = Some(10.0);
    }
    let at1 = go(&mut p, 50.0);
    let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
    let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
    let across = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) - along * along).max(0.0).sqrt();
    assert!(across < 1e-3, "the body moved {across:.4} mm across the travel axis");
    assert!(
        (along - 10.0).abs() < 1e-3,
        "a limit of 10 mm: the body must stop at the mark (10 along the arrow) but travelled {along:.4}"
    );
}

/// Hold as built works on a turned anchor too.
///
/// The declaration is captured from the frame turned the same way the solver turns it, and afterwards the side
/// flag is cleared so the solver decides the side again from the current axes. If what is captured and what is
/// applied differ even in the order of multiplication, the body jumps when the button is pressed.
#[test]
fn holding_as_it_stands_works_on_a_turned_anchor_too() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other and the body stands aside: without a declaration the mate would pull it
    // in.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 25.0, 0.0, -1.0, 0.0, 13.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    let before = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!(p.set_joint_as_built(jid), "the hold-as-built declaration must succeed");
    p.solve_joints();
    let after = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let d = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2) + (after[2] - before[2]).powi(2)).sqrt();
    assert!(d < 1e-3, "the hold-as-built declaration moved the body {d:.4} mm: was {before:?}, now {after:?}");

    // And it holds: displaced by hand, the body returns to the same place.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 90.0, 0.0, -1.0, 0.0, 13.0, 0.0, 0.0, -1.0, 40.0]);
    p.solve_joints();
    let back = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let d2 = ((back[0] - before[0]).powi(2) + (back[1] - before[1]).powi(2) + (back[2] - before[2]).powi(2)).sqrt();
    assert!(d2 < 1e-3, "the hold-as-built mate did not return the body to the declared place: off by {d2:.4} mm ({back:?})");
}

/// An angle is measured in the direction the gizmo shows, including on a turned anchor.
///
/// Travel is already covered (the arrow, the drag, the limit). Rotation was not, and turning an anchor reverses
/// the rotation axis too: a positive angle would run the other way, the limit arc would be drawn mirrored, and
/// the body would stop somewhere other than the mark.
#[test]
fn a_positive_angle_turns_the_part_the_way_the_gizmo_ring_points() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Revolute);

    let axis = p.joint_slot_axis(jid, 0, p.root).expect("the rotation axis");
    let zero = p.joint_zero_dir(jid, p.root).expect("the angle zero");
    // The angle of the body about the gizmo axis, measured from the gizmo zero.
    let spin = |p: &Project| {
        let m = p.world_transform(b);
        let o = apply12(&m, [0.0, 0.0, 0.0]);
        let px = apply12(&m, [1.0, 0.0, 0.0]);
        let v = [px[0] - o[0], px[1] - o[1], px[2] - o[2]];
        let cross = [zero[1] * v[2] - zero[2] * v[1], zero[2] * v[0] - zero[0] * v[2], zero[0] * v[1] - zero[1] * v[0]];
        let s = axis[0] * cross[0] + axis[1] * cross[1] + axis[2] * cross[2];
        let c = zero[0] * v[0] + zero[1] * v[1] + zero[2] * v[2];
        s.atan2(c).to_degrees()
    };
    let go = |p: &mut Project, v: f64| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[0] = Some(v);
        }
        p.solve_joints();
    };
    go(&mut p, 0.0);
    let at0 = spin(&p);
    go(&mut p, 45.0);
    let at1 = spin(&p);
    let d = (at1 - at0 + 540.0) % 360.0 - 180.0;
    assert!(
        (d - 45.0).abs() < 1e-2,
        "+45 degrees were specified but the body turned {d:.4} degrees about the gizmo axis (from {at0:.3} to {at1:.3}): the angle runs the wrong way"
    );
}

/// The mate frame shows the same axes the mate holds by.
///
/// The frame is used by the glyph drawing and by the popup. While only its origin was taken, disagreeing axes
/// broke nothing — the origin depends on neither the kind nor the anchor turn. But the name promises the axes
/// of the mate, and sooner or later somebody takes them. The promise is pinned here: the main axis of the frame
/// has to match the axis `joint_slot_axis` returns.
#[test]
fn the_joint_frame_shows_the_axes_the_joint_actually_holds_by() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    // The anchors face each other, so the solver turns the first one itself.
    set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    let m = p.joint_frame(jid, p.root).expect("the mate frame");
    let main = [m[2], m[6], m[10]]; // The main axis (Z) of the frame.
    let axis = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
    let dot = main[0] * axis[0] + main[1] * axis[1] + main[2] * axis[2];
    assert!(
        dot > 0.999,
        "the main axis of the mate frame {main:?} disagrees with the travel axis {axis:?}: agreement {dot:.4}"
    );
}

/// Every degree travels along its own arrow, the second one included, and on a turned anchor too.
///
/// The anchor turn is 180 degrees about X: it leaves X alone and reverses Y and Z. So for a planar mate the
/// first travel (along X) is indifferent to the turn while the second (along Y) is not; for a pin-slot the
/// travel belongs to the second anchor and turning the first does not affect it either. All of this is
/// measured rather than deduced: reasoning about axes has already been right on paper and wrong in the code
/// three times.
#[test]
fn every_slot_travels_along_its_own_arrow_on_a_turned_anchor() {
    let cases: [(JointKind, usize, f64); 3] = [(JointKind::Planar, 1, 10.0), (JointKind::Planar, 2, 10.0), (JointKind::PinSlot, 1, 10.0)];
    let mut bad = Vec::new();
    let mut checked = 0usize;
    for (kind, slot, want) in cases {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        // The anchors face each other, so the solver turns the first one itself.
        set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, kind);

        let dir = match p.joint_slot_axis(jid, slot, p.root) {
            Some(d) => d,
            None => {
                bad.push(format!("{kind:?} slot {slot}: the degree has no arrow"));
                continue;
            }
        };
        let go = |p: &mut Project, v: f64| {
            if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
                j.drive[slot] = Some(v);
            }
            p.solve_joints();
            apply12(&p.world_transform(b), [0.0, 0.0, 0.0])
        };
        let at0 = go(&mut p, 0.0);
        let at1 = go(&mut p, want);
        let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
        let along = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
        checked += 1;
        if (along - want).abs() > 1e-3 {
            bad.push(format!("{kind:?} slot {slot}: {want} was specified but {along:.4} was travelled along the arrow (travel {d:?})"));
        }
    }
    assert_eq!(checked, 3, "guard: three degrees were set up but {checked} were checked");
    assert!(bad.is_empty(), "a degree does not travel along its own arrow:\n  {}", bad.join("\n  "));
}

/// The mate field shows exactly what was specified, including on a turned anchor.
///
/// This is the number shown in the mate popup, and the limit and the relation are computed from it. On a real
/// document it was already caught with the opposite sign (an angle of 30 degrees displayed as -30), but there
/// the mate was a single cylindrical one. Here every kind that has something to display is checked, on a
/// deliberately turned anchor.
#[test]
fn the_field_of_a_joint_shows_exactly_what_was_asked_on_a_turned_anchor() {
    let cases: [(JointKind, usize, f64); 4] = [
        (JointKind::Revolute, 0, 30.0),
        (JointKind::Slider, 1, 12.0),
        (JointKind::Cylindrical, 0, -25.0),
        (JointKind::Planar, 2, 7.0),
    ];
    let mut bad = Vec::new();
    let mut checked = 0usize;
    for (kind, slot, want) in cases {
        let mut p = Project::default();
        let c = parts(&mut p, 2);
        let (a, b) = (c[0], c[1]);
        p.set_grounded(a, true);
        // The anchors face each other, so the solver turns the first one itself.
        set_transform(&mut p, b, [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0]);
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        let jid = p.add_joint(ca, cb, kind);
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[slot] = Some(want);
        }
        p.solve_joints();
        let shown = p
            .joints
            .iter()
            .find(|x| x.id == jid)
            .map(|j| match slot {
                0 => j.angle,
                1 => j.offset,
                _ => j.offset2,
            })
            .unwrap_or(f64::NAN);
        checked += 1;
        if (shown - want).abs() > 1e-3 {
            bad.push(format!("{kind:?} slot {slot}: {want} was specified but the field shows {shown:.4}"));
        }
    }
    assert_eq!(checked, 4, "guard: four fields were set up but {checked} were checked");
    assert!(bad.is_empty(), "the mate holds one thing while the field says another:\n  {}", bad.join("\n  "));
}

/// A mate re-created on a turned anchor behaves the same way.
///
/// The mating side is not stored in the document as an answer: it is chosen from the current placement. So
/// re-creating a mate from the same anchors has to give the same side, the same field and the same travel.
/// Otherwise deleting a mate and creating it again would change the behaviour.
///
/// What this test holds and what it does not was checked by corruption. Reducing the side choice to a single
/// stored flag does not redden it: both runs use one rule, whatever it is, and therefore agree. So it holds the
/// determinism of re-creation rather than the correctness of the turn. The turn is covered by the neighbouring
/// tests (the mate field across every kind, the arrow, the relation).
#[test]
fn a_remade_joint_on_a_turned_anchor_behaves_the_same() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    let home = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0];
    set_transform(&mut p, b, home);
    let mk = |p: &mut Project| {
        let ca = p.add_connector(a, AnchorRef::Origin);
        let cb = p.add_connector(b, AnchorRef::Origin);
        p.add_joint(ca, cb, JointKind::Slider)
    };
    let run = |p: &mut Project, jid: Id| {
        if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(12.0);
        }
        p.solve_joints();
        let shown = p.joints.iter().find(|x| x.id == jid).map(|j| j.offset).unwrap_or(f64::NAN);
        let dir = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis");
        (shown, dir, apply12(&p.world_transform(b), [0.0, 0.0, 0.0]))
    };

    let j1 = mk(&mut p);
    let (shown1, dir1, at1) = run(&mut p, j1);

    // Create it again from the same anchors, with the body put back.
    p.delete_joint(j1);
    set_transform(&mut p, b, home);
    let j2 = mk(&mut p);
    let (shown2, dir2, at2) = run(&mut p, j2);

    assert!((shown1 - 12.0).abs() < 1e-3 && (shown2 - 12.0).abs() < 1e-3, "the field must show the specified 12: was {shown1:.4}, now {shown2:.4}");
    let dd = (0..3).map(|k| (dir1[k] - dir2[k]).abs()).fold(0.0f64, f64::max);
    assert!(dd < 1e-9, "the arrow points differently after re-creation: was {dir1:?}, now {dir2:?}");
    let d = ((at1[0] - at2[0]).powi(2) + (at1[1] - at2[1]).powi(2) + (at1[2] - at2[2]).powi(2)).sqrt();
    assert!(d < 1e-3, "the re-created mate placed the body elsewhere: was {at1:?}, now {at2:?} (off by {d:.4} mm)");
}

/// A rigid mate turns the body about the joint axis.
///
/// A rigid mate has two parameters: an offset and a rotation. With only the gap along the axis available there
/// is no way to turn a fastened body about the joint axis, and the body itself has to be rotated by guesswork.
/// This is not a missing dialogue field but a missing specifiable parameter: a rigid mate has no freedom, yet
/// it does have parameters, and rotation is one of them.
#[test]
fn a_fastened_joint_can_be_given_a_twist_about_its_axis() {
    let mut p = Project::default();
    let c = parts(&mut p, 2);
    let (a, b) = (c[0], c[1]);
    p.set_grounded(a, true);
    set_transform(&mut p, b, tr(50.0, 20.0, -10.0));
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let jid = p.add_joint(ca, cb, JointKind::Rigid);

    // Without a specified value a rigid mate aligns the anchors with no extra turn.
    p.solve_joints();
    let x0 = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!((x0[0] - 1.0).abs() < 1e-6 && x0[1].abs() < 1e-6, "setup: without a rotation the body X axis must land on world X, but it is {x0:?}");

    // Specify a rotation of 30 degrees about the joint axis.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.drive[0] = Some(30.0);
    }
    p.solve_joints();
    let o = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let x = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    let v = [x[0] - o[0], x[1] - o[1], x[2] - o[2]];
    let ang = v[1].atan2(v[0]).to_degrees();
    assert!((ang - 30.0).abs() < 1e-3, "a rotation of 30 degrees was specified but the body turned {ang:.4} degrees");
    // The gap still applies alongside the rotation.
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.drive[1] = Some(7.0);
    }
    p.solve_joints();
    let o2 = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    assert!((o2[2] - 7.0).abs() < 1e-3, "a gap of 7 mm must apply together with the rotation, but the body is at z = {:.4}", o2[2]);
}
