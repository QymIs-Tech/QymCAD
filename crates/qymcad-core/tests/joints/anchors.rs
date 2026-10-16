//! Every kind on every anchor, the least move, rigid on edges, pin slots, connectors moved and turned, groups.

use super::*;

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
    let anchors: Vec<Anchor> = vec![
        anchor("part origin", Box::new(|_p: &mut Project, _c: Id| AnchorRef::Origin), [0.0, 0.0, 1.0]),
        anchor(
            "planar face (normal +Z)",
            Box::new(|_p: &mut Project, _c: Id| AnchorRef::FaceCenter(1, FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: 0 })),
            // Along the face rather than along the normal: any axis in the XY plane will do, so Z is checked
            // to be near zero.
            [0.0, 0.0, 0.0],
        ),
        anchor(
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

    for Anchor { name: anchor_name, make, axis } in &anchors {
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
            assert!(p.unresolved_joints().is_empty(), "{kind:?} on anchor \"{anchor_name}\": the anchor did not resolve, so the mate drops out of the computation silently");
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

    let edge = |ref_dir: [f64; 3]| MeshEdge { id: 5, mid: [0.0, 0.0, 0.0], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, -5.0], b: [0.0, 0.0, 5.0], ref_dir, ..Default::default() };
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
    assert!(dot > 0.999, "a rigid mate on edges placed the body by world axes rather than by its face: face B points at {got:?}, expected +Y");
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
    assert!(rep.errors.iter().any(|(id, _)| *id == j), "a mate without a connector dropped out of the computation silently: report {rep:?}, while on screen the assembly looks assembled");
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
    assert!(moved < 1e-9, "there is no solution yet the body moved {moved:.3} mm to {at:?}: the solver placed it at a compromise nobody asked for");
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
    assert!((at[1] - 10.0).abs() < 1e-6 && at[0].abs() < 1e-6 && at[2].abs() < 1e-6, "the body travelled along the pin rather than along the slot: it is at {at:?}, expected (0, 10, 0)");
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
    assert!((x[0] - s30).abs() < 1e-9 && (x[1] - c30).abs() < 1e-9 && x[2].abs() < 1e-9, "the secondary axis was not turned by 30 degrees: frame X is {x:?}");
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
    p.regen_edges.insert(body, vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ref_dir: [0.0, 0.0, 1.0], ..Default::default() }]);

    let cid = p.add_connector(a, AnchorRef::Origin);
    let before = p.connector_matrix(cid).expect("the connector frame");
    assert!((before[0] - 1.0).abs() < 1e-9, "setup: without a pick the frame X axis is world X");

    // An edge is picked, so the axis runs along it.
    p.connectors.iter_mut().find(|x| x.id == cid).expect("the connector").axis_ref = Some(AnchorRef::EdgeMid(body, 5));
    let m = p.connector_matrix(cid).expect("the connector frame");
    let x = [m[0], m[4], m[8]];
    assert!((x[0] - s).abs() < 1e-9 && (x[1] - s).abs() < 1e-9 && x[2].abs() < 1e-9, "the connector axis was not taken from the picked edge: frame X is {x:?}, expected ({s:.4}, {s:.4}, 0)");
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
        p.regen_edges.insert(body, vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ref_dir: [0.0, 0.0, 1.0], ..Default::default() }]);
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
    assert!((d[0] / len - s).abs() < 1e-6 && (d[1] / len - s).abs() < 1e-6, "the body did not travel along the picked edge: {:?}", [d[0] / len, d[1] / len, d[2] / len]);
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

/// A PART OF A GROUP MOVED BY HAND CARRIES THE GROUP WITH IT. The group reads where its members stand when the
/// problem is assembled, so a hand that puts one member somewhere else by its placement alone breaks the group
/// for good: the next solve takes the broken placement as the one to hold. Reported behaviour: the second part
/// went 18 along X by the arm of its gizmo and the third, grouped with it, stayed.
#[test]
fn a_part_of_a_group_moved_by_hand_carries_the_group() {
    let mut p = Project::default();
    let c = parts(&mut p, 4);
    let (a, b, d, e) = (c[0], c[1], c[2], c[3]);
    p.set_grounded(a, true);
    set_transform(&mut p, b, tr(60.0, 0.0, 0.0));
    set_transform(&mut p, d, tr(120.0, 0.0, 0.0));
    set_transform(&mut p, e, tr(180.0, 0.0, 0.0));
    p.add_group(&[b, d]);
    // turned a quarter about Z and moved: the other member turns about the same point and goes the same way
    let turned = [0.0, -1.0, 0.0, 78.0, 1.0, 0.0, 0.0, 5.0, 0.0, 0.0, 1.0, 0.0];
    assert!(p.move_component_by_hand(b, turned), "a member of a group nobody grounded could not be moved");
    p.solve_joints();
    let at = |p: &Project, c: Id| apply12(&p.world_transform(c), [0.0; 3]);
    let (bb, dd, ee) = (at(&p, b), at(&p, d), at(&p, e));
    assert!((bb[0] - 78.0).abs() < 1e-6 && (bb[1] - 5.0).abs() < 1e-6, "the part moved by hand stands at {bb:?}, not at [78, 5, 0]");
    assert!((dd[0] - 78.0).abs() < 1e-6 && (dd[1] - 65.0).abs() < 1e-6, "the group did not carry its other member: it stands at {dd:?}, [78, 65, 0] was expected");
    assert!((ee[0] - 180.0).abs() < 1e-6, "a part outside the group moved: it stands at {ee:?}");
    // a group holding a grounded part does not move by hand
    p.add_group(&[a, e]);
    assert!(!p.move_component_by_hand(e, tr(0.0, 0.0, 50.0)), "a part grouped with the grounded one was moved");
    assert!((at(&p, e)[2]).abs() < 1e-9, "a part grouped with the grounded one was moved");
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
