//! What a rebuild plans and touches, mirrors and moves that follow their source, renames and tilted planes.

use super::*;

// A multi-contour operation is exactly one node holding every contour, not a chain of extrude plus boolean.
#[test]
fn multi_op_is_single_node_with_all_profiles() {
    let mut p = Project::default();
    p.new_document();
    let s = square(&mut p, "s");
    let before = p.timeline.len();
    // Three contours in a single operation; src = 0 makes a new body.
    let body = p.add_combine_multi_op(0, s, vec![101, 102, 103], qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 1);
    assert_eq!(p.timeline.len(), before + 1, "N contours must give one node, not a chain");
    let node = p.timeline.iter().find(|n| n.kind.body() == Some(body)).unwrap();
    match &node.kind {
        qymcad_core::feature::FeatureKind::Combine { profiles, src, .. } => {
            assert_eq!(profiles, &vec![101, 102, 103], "every contour must live in the one node");
            assert_eq!(*src, 0, "src = 0 makes a new body");
        }
        _ => panic!("expected a Combine node"),
    }
    // `add_extrude_multi` also produces one node holding every contour.
    let before2 = p.timeline.len();
    let e = p.add_extrude_multi(s, vec![201, 202], 3.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    assert_eq!(p.timeline.len(), before2 + 1, "extruding N contours must give one node");
    match &p.timeline.iter().find(|n| n.kind.body() == Some(e)).unwrap().kind {
        qymcad_core::feature::FeatureKind::Extrude { profiles, .. } => assert_eq!(profiles, &vec![201, 202]),
        _ => panic!("expected an Extrude node"),
    }
}

// Deleting a single-node multi-contour operation is clean: the part body survives and no orphan is left.
#[test]
fn delete_one_node_op_clean() {
    let mut p = Project::default();
    p.new_document();
    let s = square(&mut p, "s");
    // The base part: a Combine with src = 0, which makes a new body.
    let base = p.add_combine_multi_op(0, s, vec![101], qymcad_core::model::CombineSpan { height: 10.0, down: 0.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 1);
    // A boss of three contours added to the part as a single node.
    let op = p.add_combine_multi_op(base, s, vec![201, 202, 203], qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 1);
    let before = p.timeline.iter().filter(|n| n.kind.body().is_some()).count();
    // Delete the operation.
    let removed = p.delete_feature_op(op);
    assert!(removed.contains(&op), "the operation node itself must be removed");
    assert!(!removed.contains(&base), "the base part must not be removed");
    let after: Vec<Id> = p.timeline.iter().filter_map(|n| n.kind.body()).collect();
    assert!(after.contains(&base), "the part must return to its state before the operation");
    assert_eq!(after.len(), before - 1, "exactly one node must be removed, without splitting into bodies");
    assert!(!after.contains(&op), "no orphan of the operation must remain");
}

// ─── Estimating the size of a rebuild (`regen_plan`) ──────────────────────────────────────────
//
// Cutting a hole in one part reported the whole project as rebuilt, 28 of 28. The counter in the
// window showed the position in the timeline rather than the work done, so "walked past" could not
// be told from "recomputed". The plan answers that question up front and without geometry, which
// makes it an over-approximation by contract: everything a rebuild actually touches must appear in
// it.

/// A project of two independent parts, each with its own sketch and its own body.
fn two_independent_parts() -> (Project, Id, Id, Id, Id) {
    let mut p = part_project();
    let d1 = p.add_component("Part 1");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    let d2 = p.add_component("Part 2");
    p.set_active_component(Some(d2));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 5.0);
    (p, d1, d2, b1, b2)
}

/// An edit in one part does not pull in the other, which is what a reader of the plan expects.
#[test]
fn a_plan_for_an_edit_in_one_part_leaves_the_other_alone() {
    let (mut p, _d1, _d2, b1, b2) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k); // A cold build, so every node is clean afterwards.

    let victim = p.timeline.iter().find(|n| n.kind.body() == Some(b1)).map(|n| n.id).expect("the node of body 1");
    p.mark_node_dirty(victim);
    let plan = p.regen_plan();
    assert!(plan.nodes.contains(&victim), "the edited node must appear in the plan");
    let touches_other = p.timeline.iter().any(|n| plan.nodes.contains(&n.id) && n.kind.body() == Some(b2));
    assert!(!touches_other, "the plan pulls in the other part: an edit in one part must not rebuild the second");
    assert!(plan.nodes.len() < plan.total, "the plan named the whole timeline where one node was touched: {plan:?}");
}

/// A SKETCH MARKED NAMES WHAT STANDS ON IT: the rebuild carries the sketch on to the extrusion over it, and so must the
/// plan. Reported behaviour: a parameter added on the filter of the examples marked two sketches with formulas, the
/// plan named those two of 23 nodes - "a quiet rebuild of 2 nodes" - and the cuts and the rounding of 36 edges on them
/// were rebuilt for 7.8 s.
#[test]
fn a_marked_sketch_takes_what_stands_on_it_into_the_plan() {
    let (mut p, _d1, _d2, b1, b2) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k);
    let s1 = p.timeline.iter().find_map(|n| match n.kind {
        qymcad_core::feature::FeatureKind::Sketch { sketch } if p.timeline.iter().any(|m| m.kind.body() == Some(b1) && m.kind.inputs().contains(&sketch)) => Some(sketch),
        _ => None,
    });
    p.mark_sketch_dirty(s1.expect("the sketch of body 1"));
    let plan = p.regen_plan();
    let named = |b: Id| p.timeline.iter().any(|n| plan.nodes.contains(&n.id) && n.kind.body() == Some(b));
    assert!(named(b1), "the sketch under body 1 is marked and the plan does not name the extrusion on it: {plan:?}");
    assert!(!named(b2), "the sketch under body 1 is marked and the plan names the other part: {plan:?}");
    let planned: HashSet<Id> = plan.nodes.into_iter().collect();
    let rep = p.regenerate(&k);
    for (body, _) in rep.built.iter() {
        let node = p.timeline.iter().find(|n| n.kind.bodies().contains(body)).map(|n| n.id);
        assert!(node.is_some_and(|id| planned.contains(&id)), "body {body} was rebuilt from the marked sketch while the plan said nothing about it");
    }
}

/// The plan is an over-approximation: whatever the rebuild built, the plan had to name.
#[test]
fn everything_rebuilt_was_named_by_the_plan() {
    let (mut p, _d1, _d2, b1, _b2) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k);

    let victim = p.timeline.iter().find(|n| n.kind.body() == Some(b1)).map(|n| n.id).expect("the node of body 1");
    p.mark_node_dirty(victim);
    let planned: HashSet<Id> = p.regen_plan().nodes.into_iter().collect();
    let rep = p.regenerate(&k);
    for (body, _) in rep.built.iter() {
        let node = p.timeline.iter().find(|n| n.kind.bodies().contains(body)).map(|n| n.id);
        assert!(
            node.is_some_and(|id| planned.contains(&id)),
            "body {body} was rebuilt while the plan said nothing about it: the counter and the choice between a modal and a status line would both lie"
        );
    }
}

/// A clean document plans nothing; otherwise the rebuild window would pop up for no reason.
#[test]
fn a_clean_document_plans_nothing() {
    let (mut p, ..) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k);
    let plan = p.regen_plan();
    assert!(plan.nodes.is_empty(), "after a completed rebuild there is nothing left to rebuild, yet the plan named {:?}", plan.nodes.len());
}

// ─── Mirrored parts and the active body of the source ─────────────────────────────────────────
//
// Found by a scenario run: a full rebuild of an unchanged timeline dropped the mirrored part with
// `SourcePartHasNoBody`, turning 131 bodies into 130, even though the mirror built fine while the
// document was being edited. `MirrorPart` takes the body of its source through `active_body`, which
// is structural: the last body of the component that has not been consumed. So the structure was
// what changed, and the question is whether the mirror learns that its source has a new active
// body.

/// A part with a body, a mirror of that part, and an operation that consumes the body of the source.
#[test]
fn a_mirror_follows_the_source_when_its_active_body_is_replaced() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);

    let mirror = p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let k = MockKernel::default();
    p.regenerate(&k);
    let mirror_body = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MirrorPart { body, .. } => Some(body),
            _ => None,
        })
        .expect("the mirror node");
    assert!(p.mesh_index(mirror_body).is_some(), "setup: the mirror must build on the first pass");
    let _ = mirror;

    // The body of the source is consumed and replaced by a new one, exactly what a body boolean or a split does.
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);
    assert_ne!(merged, b1, "setup: a boolean must produce a new body");
    assert_eq!(p.active_body(d1), Some(merged), "setup: the new body must become the active body of the source");

    // The mirror must learn of this at once, not ten steps later.
    let mirror_dirty = p.timeline.iter().find(|n| n.kind.bodies().contains(&mirror_body)).map(|n| n.dirty);
    p.regenerate(&k);
    assert!(
        !p.regen_errors.contains_key(&mirror_body),
        "the mirror failed after the active body of the source was replaced: {:?} (its dirty flag was {mirror_dirty:?})",
        p.regen_errors.get(&mirror_body)
    );
    assert!(p.mesh_index(mirror_body).is_some(), "the mirror was left without a body after the active body of the source was replaced (its dirty flag was {mirror_dirty:?})");
}

/// The same document gives the same result, however it was reached.
///
/// The set of bodies used to depend on the history of rebuilds: one document gave three different answers depending
/// on whether it was rebuilt along the way. The cause was not full versus incremental rebuilding but a forward
/// reference: the mirror took the body of its source through `active_body`, which scanned the entire timeline. A
/// boolean consumed the body of the source and produced a new one, and the mirror saw that new body even though the
/// node producing it stands below the mirror itself. Rebuild along the way and the mirror latches onto the old body;
/// rebuild once at the end and it fails.
#[test]
fn rebuilding_the_same_timeline_twice_gives_the_same_bodies() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);

    let k = MockKernel::default();
    let _ = merged;
    p.regenerate(&k);
    let first: HashSet<Id> = p.bodies.iter().map(|b| b.id).collect();

    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    p.regenerate(&k);
    let second: HashSet<Id> = p.bodies.iter().map(|b| b.id).collect();
    assert_eq!(
        first,
        second,
        "a full rebuild of the same timeline gave a different set of bodies: {:?} vanished, {:?} appeared, so the model is not reproducible",
        first.difference(&second).collect::<Vec<_>>(),
        second.difference(&first).collect::<Vec<_>>()
    );
}

/// Where the mirror node lands in the timeline, measured rather than assumed.
///
/// A mirror copies the body of its source, so in the history it must stand after that body. Standing above it, the
/// document would describe something impossible: a copy of what does not exist yet at that point.
#[test]
fn a_mirror_node_stands_below_the_body_it_copies() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    let mirror_comp = p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let _ = mirror_comp;

    let pos = |p: &Project, b: Id| p.timeline.iter().position(|n| n.kind.bodies().contains(&b));
    let mirror_body = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MirrorPart { body, .. } => Some(body),
            _ => None,
        })
        .expect("the mirror node");
    assert!(pos(&p, mirror_body) > pos(&p, b1), "the mirror node stands above the body it copies: mirror at {:?}, source body at {:?}", pos(&p, mirror_body), pos(&p, b1));

    // ...and it still holds once the source has gained a new active body.
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);
    assert_eq!(p.active_body(d1), Some(merged), "setup: the merged body must become the active one");
    let seen = p.active_body_before(d1, pos(&p, mirror_body).expect("the position of the mirror"));
    assert!(seen.is_some(), "the mirror sees no body of the source above itself, so it copies what does not exist at its point in the timeline");
}

/// A moved sketch does not cost a fillet its edges.
///
/// Taken from a real document: editing the coordinates of a rectangle in a sketch left a fillet, picked by hand over
/// edges further down the timeline, unable to find them. The cutouts stayed sharp and the edges had to be picked
/// again. Measured on that file, healing returned 28 of the 36 picked edges.
///
/// The cause was the measure: a replacement was accepted only while the edge stayed within 0.5 mm of its former
/// place. An edit moves geometry by an amount comparable to the part, not to half a millimetre. The tolerance is now
/// derived from the bounding box of the body, and the same file returns 34 of 36.
#[test]
fn a_moved_sketch_does_not_cost_the_fillet_its_edges() {
    let mut p = Project::default();
    // A snapshot of an edge: id, midpoint and direction, which is what the reference is healed by.
    let edge = |id: u32, mid: [f64; 3]| qymcad_core::geom::MeshEdge {
        id,
        a: [mid[0] - 5.0, mid[1], mid[2]],
        b: [mid[0] + 5.0, mid[1], mid[2]],
        mid,
        dir: [1.0, 0.0, 0.0],
        center: mid,
        axis: [0.0, 0.0, 1.0],
        radius: 0.0,
        ref_dir: [0.0, 0.0, 1.0],
        ends: Default::default(),
    };
    let fid: Id = 100;
    // A part roughly 40 mm across; the edges used to sit here.
    p.edge_refs.insert(fid, vec![ElemSnapshot { id: 7, at: [0.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0] }, ElemSnapshot { id: 8, at: [40.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0] }]);
    // After the sketch edit the names changed and the edges themselves moved by one and a half millimetres.
    let now = vec![edge(70, [1.5, 0.0, 0.0]), edge(80, [41.5, 0.0, 0.0])];
    let healed = p.resolve_edge_ids_in(&now, fid, &[7, 8]);
    assert_eq!(healed.len(), 2, "healing lost the edges that moved with the edit: it returned {healed:?}, leaving unfilleted cutouts and no error to explain them");

    // ...and it does not grab an unrelated edge. An edge at the other end of the part is no replacement for one that vanished.
    let far = vec![edge(90, [200.0, 0.0, 0.0])];
    let wrong = p.resolve_edge_ids_in(&far, fid, &[7]);
    assert!(wrong.is_empty(), "healing grabbed the distant edge {wrong:?}: a fillet landing in the wrong place is worse than a lost one");
}

/// A SECOND MOVE MULTIPLIES ONTO THE FIRST, AND IN THE RIGHT ORDER.
///
/// The interface used to walk the tree, reach inside the node's kind and compose the matrices there. Two
/// ways to get it wrong were open: replace instead of compose (the first drag is thrown away), and compose
/// the other way round (the second drag goes along the OLD axes, which on a turned part is visibly wrong
/// and hard to describe). Here both are nailed down by numbers.
#[test]
fn a_second_move_composes_onto_the_first_and_not_the_other_way() {
    let mut p = Project::default();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);

    // A QUARTER TURN ABOUT Z, then a shift of 10 along X. Turned first, the shift must come out along the
    // WORLD X - the new move acts on the result of the old one.
    let mut turn = qymcad_core::feature::PLACE_IDENTITY;
    turn[0] = 0.0;
    turn[1] = -1.0;
    turn[4] = 1.0;
    turn[5] = 0.0;
    let mv = p.add_move(base, turn);

    let mut shift = qymcad_core::feature::PLACE_IDENTITY;
    shift[3] = 10.0;
    assert!(p.accumulate_move(mv, &shift), "the body carries a Move node, so it accumulates");

    let node = p.timeline.iter().find(|n| n.id == mv).expect("the move node");
    let qymcad_core::feature::FeatureKind::Move { mat, .. } = node.kind else { panic!("a Move node was expected") };
    // compose12(shift, turn): the rotation stays, the translation is the shift itself.
    assert!((mat[3] - 10.0).abs() < 1e-9, "the shift must be along the WORLD X, got {}", mat[3]);
    assert!((mat[7] - 0.0).abs() < 1e-9, "and nothing along Y, got {} - the order of multiplication is reversed", mat[7]);
    assert!((mat[1] + 1.0).abs() < 1e-9, "the quarter turn must survive: composing must not replace");
    assert!(node.dirty, "an accumulated move asks for a rebuild");

    // A BODY WITHOUT A MOVE NODE SAYS SO rather than silently doing nothing: the caller then has a
    // different job - to make one.
    assert!(!p.accumulate_move(base, &shift), "the base carries no Move node");
    assert!(!p.accumulate_move(9999, &shift), "and there is no such body at all");
}

/// RENAMING A NODE IS ONE NAMED OPERATION, and a miss is told apart from a change.
#[test]
fn a_node_is_renamed_or_says_there_is_no_such_node() {
    let mut p = Project::default();
    let sid = square(&mut p, "square");
    let node = p.add_sketch_node(sid, "square");
    assert!(p.rename_node(node, "base outline"), "the node is there");
    assert_eq!(p.timeline.iter().find(|n| n.id == node).unwrap().name, "base outline");
    assert!(!p.rename_node(9999, "nowhere"), "a miss must be told apart from a change");
}

/// "EVERYTHING IS STALE" MARKS EVERY NODE, and that is all it does - who rebuilds is the planner's.
#[test]
fn marking_everything_stale_touches_every_node() {
    let mut p = Project::default();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // THE STARTING POINT IS SET BY HAND rather than assumed. A rebuild does NOT leave every node clean -
    // that was the first guess and the check said otherwise - so the test makes its own "before".
    for n in p.timeline.iter_mut() {
        n.dirty = false;
    }
    assert!(p.timeline.iter().all(|n| !n.dirty), "the starting point: nothing is stale");
    assert!(p.timeline.len() >= 2, "there is something to mark: the sketch node and the extrude");
    p.mark_all_dirty();
    assert!(p.timeline.iter().all(|n| n.dirty), "every node must be stale again");
    let _ = base;
}

/// A PLANE OFFSET FROM A BASE PLANE GOES TO THE SIDE ITS PREVIEW SHOWS: along the positive axis across it - +Z from XY,
/// +Y from XZ, +X from YZ - the side `BasePlane::offset_direction` gives the preview as well. Reported behaviour: an offset
/// of 100 from XZ was previewed on one side, and Enter put the plane on the other.
#[test]
fn a_plane_offset_from_a_base_plane_goes_along_its_positive_axis() {
    use qymcad_core::feature::BasePlane;
    let mut failed = Vec::new();
    for (base, axis) in [(BasePlane::XY, 2usize), (BasePlane::XZ, 1), (BasePlane::YZ, 0)] {
        let mut p = qymcad_core::model::Project::default();
        let id = p.add_offset_plane(base, 100.0);
        p.regenerate(&MockKernel::default());
        let o = p.planes.iter().find(|pl| pl.id == id).map(|pl| pl.origin).expect("the plane is made");
        if (o[axis] - 100.0).abs() > 1e-9 || base.offset_direction()[axis] != 1.0 {
            failed.push(format!("{base:?}: the plane stands at {o:?}, the preview's direction is {:?}", base.offset_direction()));
        }
    }
    assert!(failed.is_empty(), "planes offset by 100 not on the positive side:\n{}", failed.join("\n"));
}

/// A DATUM PLANE TILTED BY ITS ANGLES: about the X of the plane it is offset from, about its Y, the turn about its normal,
/// each parametric through its feature dimension as the offset is - and the second rebuild does not tilt it again.
#[test]
fn a_datum_plane_is_tilted_by_its_angles_through_the_point_it_stands_at() {
    use qymcad_core::feature::BasePlane;
    let mut failed = Vec::new();
    let mut p = qymcad_core::model::Project::default();
    let id = p.add_offset_plane(BasePlane::XY, 10.0);
    p.set_feat_dim(id, "tilt_x", "90".into());
    for pass in 0..2 {
        p.regenerate(&MockKernel::default());
        let pl = p.planes.iter().find(|pl| pl.id == id).expect("the plane is made");
        // XY's X is world X: turned 90 deg about it, the normal +Z goes to -Y, the plane stands where the offset put it
        if (pl.normal[1] + 1.0).abs() > 1e-9 || pl.normal[2].abs() > 1e-9 || (pl.origin[2] - 10.0).abs() > 1e-9 {
            failed.push(format!("rebuild {pass}: tilt_x 90 gives the normal {:?} at {:?}", pl.normal, pl.origin));
        }
    }
    p.set_feat_dim(id, "tilt_x", String::new());
    p.set_feat_dim(id, "tilt_y", "90".into());
    p.set_feat_dim(id, "turn", "30".into());
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|pl| pl.id == id).expect("the plane is made");
    // about XY's Y (world Y) the normal +Z goes to +X
    if (pl.normal[0] - 1.0).abs() > 1e-9 || (pl.rot_deg - 30.0).abs() > 1e-9 {
        failed.push(format!("tilt_y 90, turn 30 give the normal {:?}, the turn {}", pl.normal, pl.rot_deg));
    }
    assert!(failed.is_empty(), "the tilt of a datum plane:\n{}", failed.join("\n"));
}
