//! Free and driven slots, limits, the flip, nested children, reports of broken joints, sliders and freedoms of each kind.

use super::*;

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
    p.regen_edges.insert(
        body_a,
        vec![MeshEdge {
            id: 5,
            mid: [0.0, 0.0, 5.0],
            dir: [0.0, 0.0, 1.0],
            a: [0.0, 0.0, 0.0],
            b: [0.0, 0.0, 10.0],
            radius: 0.0,
            center: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            ref_dir: [0.0; 3],
            ends: Default::default(),
        }],
    );
    p.set_active_component(Some(b));
    let body_b = p.add_extrude(2, 5.0);
    p.regen_edges.insert(
        body_b,
        vec![MeshEdge {
            id: 7,
            mid: [0.0, 0.0, 0.0],
            dir: [0.0, 0.0, 1.0],
            a: [0.0, 0.0, -5.0],
            b: [0.0, 0.0, 5.0],
            radius: 0.0,
            center: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            ref_dir: [0.0; 3],
            ends: Default::default(),
        }],
    );
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
    assert!((after - moved).abs() < 1e-3, "a cylindrical mate does not constrain along its axis, so the body must stay at {moved:.2} mm, but it sprang back to {after:.2}");

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
    let kinds = [JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot];
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
            assert!(err < 1e-6, "{:?}, slot {slot}: a free degree must not be moved — the body stood at {v0} and was carried away (off by {err:.4})", kind);

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
    assert!((fb[2] - 9.0).abs() < 1e-6 && fb[0].abs() < 1e-6 && fb[1].abs() < 1e-6, "a gap of 4 mm must separate the faces: expected the centre of B at [0,0,9], got {fb:?}");
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
    assert!(dot < -0.99, "the toggle must turn the body to the opposite side, yet the axes point almost the same way (dot product {dot:.3})");
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

    p.regen_edges.insert(777, vec![qymcad_core::geom::MeshEdge { id: 5, mid: [1.0, 0.0, 0.0], dir: [0.0, 1.0, 0.0], a: [1.0, -1.0, 0.0], b: [1.0, 1.0, 0.0], ..Default::default() }]);
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
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 5, mid: [0.0, 0.0, 0.0], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ..Default::default() }]);
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
    assert!((cos.abs() - 1.0).abs() < 1e-6, "the body did not move along the selected edge: displacement {d:?}, edge [{s:.3},{s:.3},0], cosine {cos:.6}");
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
        p.regen_edges.insert(body_a, vec![MeshEdge { id: 5, mid: [0.0, 0.0, 0.0], dir: [s, s, 0.0], a: [-s, -s, 0.0], b: [s, s, 0.0], ..Default::default() }]);
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
    let ca = p.add_connector(a, AnchorRef::FaceCenter(1, key));
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
    assert!(d[2].abs() < 1e-6, "the body moved along the face normal (up or down) when it must slide along the face: displacement {d:?}");
}
