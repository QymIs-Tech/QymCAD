//! Parallel and width, a deleted body, as-built joints, the animated range, connectors that cannot be deleted, names.

use super::*;

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

/// A TAB FAR FROM THE SLOT COMES TO IT, IT DOES NOT TURN. The tab's plane lies in the mid-plane of the walls, so it
/// looks the way they do; with the point alone a tab 120 away met the mid-plane by turning about its own origin.
#[test]
fn a_tab_far_from_the_slot_comes_to_it_rather_than_turning() {
    // the measured case: three parts standing at zero with their bodies built 60 apart, the walls on the first two
    // (both grounded), 20 and 80 along X, the tab on the third at 140
    let mut p = Project::default();
    let c = parts(&mut p, 3);
    p.set_grounded(c[0], true);
    p.set_grounded(c[1], true);
    let plane_at = |p: &mut Project, owner: Id, x: f64| {
        let id = p.add_connector(owner, AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
        p.connectors.iter_mut().find(|k| k.id == id).expect("the connector").offset_xyz = [10.0, 5.0, x];
        id
    };
    let (w1, w2, tab) = (plane_at(&mut p, c[0], 20.0), plane_at(&mut p, c[1], 80.0), plane_at(&mut p, c[2], 140.0));
    p.add_width(&[w1, w2], tab);
    p.solve_joints();
    let m = p.world_transform(c[2]);
    let turned = (1.0 - m[0]).abs() + (1.0 - m[5]).abs() + (1.0 - m[10]).abs();
    assert!(turned < 1e-6, "the tab turned to meet the mid-plane instead of coming to it: {m:?}");
    assert!((m[3] + 90.0).abs() < 1e-6, "the tab's plane did not come to 50, halfway between 20 and 80: its part stands at x {}", m[3]);
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
    p.regen_edges.insert(body_a, vec![MeshEdge { id: 5, mid: [0.0; 3], dir: [0.0, 0.0, 1.0], a: [0.0, 0.0, -5.0], b: [0.0, 0.0, 5.0], ref_dir: [0.0, 1.0, 0.0], ..Default::default() }]);
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
    assert!((pulled[0] - 10.0).abs() < 1e-3, "setup: an ordinary rigid mate must bring the origins together (x = 10), but the body is at {pulled:?}");

    // Now the current placement is declared as the one the mate holds.
    set_transform(&mut p, b, tr(50.0, 7.0, -3.0));
    assert!(p.set_joint_as_built(jid), "the mate must accept the hold-as-built declaration");
    p.solve_joints();
    let held = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    for (i, want) in [50.0, 7.0, -3.0].into_iter().enumerate() {
        assert!((held[i] - want).abs() < 1e-3, "the body must stay where it stood ({want} along axis {i}), but it is at {held:?}");
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
    assert!((twice[0] - 50.0).abs() < 1e-3 && (twice[1] - 7.0).abs() < 1e-3 && (twice[2] + 3.0).abs() < 1e-3, "the second hold-as-built moved the body: was [50, 7, -3], now {twice:?}");
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
