//! The hierarchy of components: composition of transforms (`mat_mul12` and `apply12`), `world_transform` down
//! the tree, tree queries, local frames of a component, and a new document.

use qymcad_core::feature::{apply12, mat_inv12, mat_mul12, BasePlane, ComponentKind, PLACE_IDENTITY};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

/// A 3×4 transform of pure translation.
fn tr(x: f64, y: f64, z: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

/// A 90° rotation about Z, taking x to y and y to −x.
fn rot_z90() -> [f64; 12] {
    [0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
}

/// A square sketch plus a timeline node in the active context; returns the id of the sketch.
fn square(p: &mut Project, name: &str) -> Id {
    let sid = p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, name);
    sid
}

fn set_transform(p: &mut Project, comp: Id, m: [f64; 12]) {
    let i = p.component_index(comp).unwrap();
    p.components[i].transform = m;
}

#[test]
fn mat_mul12_composes_and_identity_is_neutral() {
    let a = tr(1.0, 2.0, 3.0);
    let b = tr(10.0, 20.0, 30.0);
    // c applies b first and a second, so the origin moves to (11,22,33)
    let c = mat_mul12(&a, &b);
    assert_eq!(apply12(&c, [0.0, 0.0, 0.0]), [11.0, 22.0, 33.0]);
    // the identity is neutral on both sides
    assert_eq!(mat_mul12(&PLACE_IDENTITY, &b), b);
    assert_eq!(mat_mul12(&a, &PLACE_IDENTITY), a);
}

#[test]
fn mat_mul12_respects_order_rotation_then_translation() {
    // world = rot ∘ translate: the point (0,0,0) is translated to (1,0,0) and then rotated 90° about Z to
    // (0,1,0)
    let m = mat_mul12(&rot_z90(), &tr(1.0, 0.0, 0.0));
    let q = apply12(&m, [0.0, 0.0, 0.0]);
    assert!((q[0] - 0.0).abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9 && q[2].abs() < 1e-9, "got {q:?}");
}

#[test]
fn world_transform_composes_down_the_tree() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let asm = p.add_assembly("subassembly");
    set_transform(&mut p, asm, tr(5.0, 0.0, 0.0));
    p.set_active_component(Some(asm));
    let part = p.add_part("part");
    set_transform(&mut p, part, tr(0.0, 7.0, 0.0));

    // world(part) = world(root = I) ∘ asm(+5x) ∘ part(+7y), putting the origin of the part at (5,7,0)
    let w = p.world_transform(part);
    assert_eq!(apply12(&w, [0.0, 0.0, 0.0]), [5.0, 7.0, 0.0]);
    // the root is the identity
    assert_eq!(p.world_transform(root), PLACE_IDENTITY);
}

#[test]
fn tree_queries_children_descendants_bodies_sketches() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let asm = p.add_assembly("subassembly");
    p.set_active_component(Some(asm));
    let part = p.add_part("part");

    assert!(p.component_children(root).contains(&asm));
    assert_eq!(p.component_children(asm), vec![part]);
    let desc = p.descendants(root);
    assert!(desc.contains(&asm) && desc.contains(&part));

    // a sketch and a body inside the part
    p.set_active_component(Some(part));
    let sid = square(&mut p, "s");
    let body = p.add_extrude(sid, 5.0);
    assert!(p.component_bodies(part).contains(&body));
    assert!(p.sketches_of_component(part).contains(&sid));
    // neither the subassembly nor the root holds the bodies or sketches of this part
    assert!(p.component_bodies(asm).is_empty());
    assert!(p.sketches_of_component(root).is_empty());
    // the owner of the body resolves to the part
    assert_eq!(p.body_owner(body), Some(part));
    assert_eq!(p.body_world_transform(body), PLACE_IDENTITY);
}

#[test]
fn new_document_makes_root_assembly_with_active_part() {
    let mut p = Project::default();
    let part = p.new_document();
    // the active context is the part
    assert_eq!(p.current_ctx(), part);
    assert_eq!(p.component_kind(part), Some(ComponentKind::Part));
    // the root is an assembly and the part is nested inside it
    let root = p.root;
    assert_eq!(p.component_kind(root), Some(ComponentKind::Assembly));
    let pc = p.components.iter().find(|c| c.id == part).unwrap();
    assert_eq!(pc.parent, Some(root));
    // a part may hold bodies, the root assembly may not
    assert!(p.ctx_holds_bodies(part));
    assert!(!p.ctx_holds_bodies(root));
}

#[test]
fn component_base_frame_world_follows_placement() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    set_transform(&mut p, part, tr(5.0, 0.0, 0.0));

    // the local XY frame of a component sits at zero; the world one is shifted by its placement
    let local = p.component_base_frame(BasePlane::XY);
    assert_eq!(local.origin, [0.0, 0.0, 0.0]);
    let world = p.component_base_frame_world(part, BasePlane::XY);
    assert_eq!(world.origin, [5.0, 0.0, 0.0]);
    assert_eq!(world.x, [1.0, 0.0, 0.0]);
}

fn approx12(a: [f64; 12], b: [f64; 12]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn mat_inv12_inverts_rigid_transform() {
    // a 90° rotation about Z plus a translation of (5,7,0)
    let m = mat_mul12(&tr(5.0, 7.0, 0.0), &rot_z90());
    let inv = mat_inv12(&m);
    assert!(approx12(mat_mul12(&m, &inv), PLACE_IDENTITY), "m·inv = I");
    assert!(approx12(mat_mul12(&inv, &m), PLACE_IDENTITY), "inv·m = I");
    // a point there and back
    let p = [3.0, -2.0, 1.5];
    let back = apply12(&inv, apply12(&m, p));
    assert!((0..3).all(|i| (back[i] - p[i]).abs() < 1e-9), "the point survives the round trip");
}

#[test]
fn relative_and_display_transform() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let asm = p.add_assembly("subassembly");
    set_transform(&mut p, asm, tr(5.0, 0.0, 0.0));
    p.set_active_component(Some(asm));
    let part = p.add_part("part");
    set_transform(&mut p, part, tr(0.0, 7.0, 0.0));
    p.set_active_component(Some(part));
    let sid = square(&mut p, "s");
    let body = p.add_extrude(sid, 5.0);

    // relative to itself it is the identity: a part is edited at its own zero
    assert!(approx12(p.relative_transform(part, part), PLACE_IDENTITY));
    assert!(approx12(p.body_display_transform(body, part), PLACE_IDENTITY), "editing a part: the body sits at zero");
    // relative to the root it is the absolute world, as seen when reviewing the assembly: the part is at
    // (5,7,0)
    assert_eq!(apply12(&p.body_display_transform(body, root), [0.0, 0.0, 0.0]), [5.0, 7.0, 0.0]);
    // relative to the subassembly it is only its own shift of the part, (0,7,0)
    assert_eq!(apply12(&p.body_display_transform(body, asm), [0.0, 0.0, 0.0]), [0.0, 7.0, 0.0]);
}

#[test]
fn component_is_within_walks_up() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let asm = p.add_assembly("subassembly");
    p.set_active_component(Some(asm));
    let part = p.add_part("part");
    assert!(p.component_is_within(part, root), "the part is in the subtree of the root");
    assert!(p.component_is_within(part, asm), "the part is in the subtree of the subassembly");
    assert!(p.component_is_within(part, part), "a component contains itself");
    assert!(!p.component_is_within(asm, part), "the subassembly is not in the subtree of the part");
}

#[test]
fn can_add_body_reflects_context() {
    let mut p = Project::default();
    p.new_document(); // the first part is active
    assert!(p.can_add_body(), "bodies may be created inside a part");
    let root = p.root;
    p.set_active_component(Some(root)); // the root assembly
    assert!(!p.can_add_body(), "bodies may not be created inside an assembly");
    assert!(p.component_is_part(p.current_ctx()) == false);
}

#[test]
fn component_placement_ops() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let part = p.add_part("part");

    p.move_component(part, [3.0, 4.0, 0.0]);
    assert_eq!(apply12(&p.world_transform(part), [0.0, 0.0, 0.0]), [3.0, 4.0, 0.0]);

    p.set_component_transform(part, tr(0.0, 0.0, 5.0));
    assert_eq!(p.component_transform(part), tr(0.0, 0.0, 5.0));
    assert_eq!(apply12(&p.world_transform(part), [0.0, 0.0, 0.0]), [0.0, 0.0, 5.0]);

    // a 90° rotation about Z takes (1,0,0) to (0,1,0)
    p.set_component_transform(part, PLACE_IDENTITY);
    p.rotate_component(part, 2, 90.0);
    let q = apply12(&p.world_transform(part), [1.0, 0.0, 0.0]);
    assert!(q[0].abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9, "got {q:?}");

    assert!(!p.is_grounded(part));
    p.set_grounded(part, true);
    assert!(p.is_grounded(part));
}

#[test]
fn delete_component_removes_whole_subtree() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    // a subassembly with a part inside it, the part holding a sketch and a body; a separate neighbouring part
    // stays behind
    let asm = p.add_assembly("subassembly");
    p.set_active_component(Some(asm));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let sid = square(&mut p, "s");
    let body = p.add_extrude(sid, 5.0);
    p.set_active_component(Some(root));
    let neighbor = p.add_part("neighbour");
    p.set_active_component(Some(neighbor));
    let nsid = square(&mut p, "n");
    let nbody = p.add_extrude(nsid, 3.0);

    let comps_before = p.components.len();
    // deleting the subassembly removes it together with the nested part, its sketch and its body
    let removed = p.delete_component(asm);
    assert!(removed.contains(&body), "the body of the part is in the list of removed items");
    assert!(p.component_index(asm).is_none(), "the subassembly is deleted");
    assert!(p.component_index(part).is_none(), "the nested part is deleted");
    assert!(p.sketch_index(sid).is_none(), "the sketch of the part is removed from the pool");
    assert!(!p.timeline.iter().any(|n| n.kind.body() == Some(body)), "the body node of the part is gone from the timeline");
    assert_eq!(p.components.len(), comps_before - 2, "the subassembly and its part are gone");
    // the neighbour and its contents are untouched
    assert!(p.component_index(neighbor).is_some(), "the neighbour survives");
    assert!(p.sketch_index(nsid).is_some(), "the sketch of the neighbour survives");
    assert!(p.timeline.iter().any(|n| n.kind.body() == Some(nbody)), "the body of the neighbour survives in the timeline");
    // the root cannot be deleted
    assert!(p.delete_component(root).is_empty(), "the root is not deleted");
    assert!(p.component_index(root).is_some(), "the root is still there");
}

#[test]
fn snapshot_face_plane_is_fixed_local_datum_not_live_ref() {
    use qymcad_core::feature::{FaceKey, FeatureKind, SketchPlane};
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    // the source A is shifted in the world; the consumer B has a shift of its own
    let src = p.add_part("A");
    set_transform(&mut p, src, tr(20.0, 0.0, 0.0));
    p.set_active_component(Some(src));
    let ssid = square(&mut p, "sa");
    let sbody = p.add_extrude(ssid, 5.0);
    let consumer = {
        p.set_active_component(Some(root));
        let c = p.add_part("B");
        set_transform(&mut p, c, tr(0.0, 5.0, 0.0));
        c
    };
    p.set_active_component(Some(consumer));

    // the face of A: in the local frame of body A its centre is [0,0,3] with normal +Z, which is [20,0,3] in
    // the world
    let key = FaceKey { index: 0, centroid: [0.0, 0.0, 3.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let pid = p.snapshot_face_plane(sbody, &key);

    // the plane belongs to the consumer B and is defined manually: a fixed snapshot rather than an
    // associative reference
    let owner = p.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Plane { plane } if plane == pid)).and_then(|n| n.parent);
    assert_eq!(owner, Some(consumer), "the snapshot plane belongs to the consumer");
    let pl = p.planes.iter().find(|pl| pl.id == pid).unwrap().clone();
    assert!(matches!(pl.def, qymcad_core::model::PlaneDef::Manual), "the snapshot is manual, with no live reference");
    // no external reference was created
    assert!(p.external_refs.is_empty(), "no live external reference is created, so the part stays free");

    // the world coordinates of the snapshot match those of the source face, so it is placed correctly
    let world_origin = apply12(&p.world_transform(consumer), pl.origin);
    assert!((world_origin[0] - 20.0).abs() < 1e-9 && world_origin[1].abs() < 1e-9 && (world_origin[2] - 3.0).abs() < 1e-9, "the snapshot in the world matches the face of A, got {world_origin:?}");

    // Independence: moving the source A anywhere leaves the local position of the snapshot in B unchanged,
    // since it is not tied to it.
    let before = pl.origin;
    set_transform(&mut p, src, tr(-100.0, 42.0, 7.0));
    let after = p.planes.iter().find(|pl| pl.id == pid).unwrap().origin;
    assert_eq!(before, after, "the snapshot does not follow its source; there is no rubber band");

    // Moving the part: shifting B carries the snapshot with it, since it is local to B.
    p.set_component_transform(consumer, tr(0.0, 0.0, 10.0));
    let moved = apply12(&p.world_transform(consumer), after);
    assert!((moved[2] - 13.0).abs() < 1e-9, "the snapshot moves together with part B, got {moved:?}");
    let _ = SketchPlane::Datum(pid);
}

#[test]
fn component_placement_survives_serde() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_component_transform(part, tr(5.0, 7.0, 9.0));
    p.set_grounded(part, true);

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.component_transform(part), tr(5.0, 7.0, 9.0));
    assert!(back.is_grounded(part), "the grounded flag survived serialisation");
}
