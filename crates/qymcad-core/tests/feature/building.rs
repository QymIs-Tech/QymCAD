//! The active body, deleting what others stand on, combining, primitives, chamfers, shells, mirrors and holes.

use super::*;

#[test]
fn active_body_scoped_to_context_not_global() {
    // `active_body(ctx)` has to take the body of the current part rather than the globally last one; otherwise
    // a cut in part 1 attaches to the body of part 2 and part 2 appears to vanish.
    let mut p = part_project();
    let d1 = p.add_component("Part 1");
    p.set_active_component(Some(d1));
    let sid1 = square(&mut p, "s1");
    p.add_sketch_node(sid1, "s1");
    let b1 = p.add_extrude(sid1, 5.0);
    // The second part, whose body comes later in the timeline.
    let d2 = p.add_component("Part 2");
    p.set_active_component(Some(d2));
    let sid2 = square(&mut p, "s2");
    p.add_sketch_node(sid2, "s2");
    let b2 = p.add_extrude(sid2, 5.0);
    assert_eq!(p.active_body(d1), Some(b1), "part 1 must give its own body b1, not the globally last b2");
    assert_eq!(p.active_body(d2), Some(b2), "part 2 must give its own body b2");
}

#[test]
fn active_body_respects_rollback() {
    // Ignoring the rollback bar makes a rolled-back body (which has no mesh) count as active, so a cut or a
    // shell targets a body that does not exist.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let b1 = p.add_extrude(sid, 5.0);
    let b2 = p.add_extrude(sid, 7.0);
    assert_eq!(p.active_body(d), Some(b2), "without a rollback bar the last body is active");
    // Rolling back to before node b2 leaves b2 unbuilt, so the previous body b1 is active.
    let idx = p.timeline_index(b2).unwrap();
    p.set_rollback(Some(idx));
    assert_eq!(p.active_body(d), Some(b1), "with the rollback bar b2 is rolled back, so b1 is active");
}

#[test]
fn delete_plane_cascades_sketch_on_datum() {
    // Deleting a datum plane used to leave a sketch hanging on it: without a frame the feature failed
    // silently. The sketch and its body now go in a cascade.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let pl = p.add_offset_plane(BasePlane::XY, 20.0);
    let si = p.new_sketch("s");
    p.sketches[si].plane = SketchPlane::Datum(pl);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    assert!(p.delete_plane_with_dependents(pl));
    assert!(p.sketch_index(sid).is_none(), "a sketch on a deleted plane must be removed in the cascade");
    assert!(p.timeline_index(body).is_none(), "the body of the sketch must be removed in the cascade");
    assert!(!p.planes.iter().any(|x| x.id == pl), "the plane must be deleted");
}

#[test]
fn delete_datum_axis_degrades_circular_array() {
    // Deleting an axis used to degrade the pattern inside regenerate without marking it dirty, leaving stale
    // geometry.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let ax = p.add_axis_manual([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let arr = p.add_circular_array_axis(body, 4, 360.0, ax);
    let axis_of = |p: &Project, id: Id| {
        p.timeline.iter().find(|n| n.id == id).and_then(|n| match n.kind {
            FeatureKind::CircularArray { axis, .. } => Some(axis),
            _ => None,
        })
    };
    assert_eq!(axis_of(&p, arr), Some(ax), "the pattern must reference the axis");
    assert!(p.delete_datum_axis(ax));
    assert_eq!(axis_of(&p, arr), Some(0), "the pattern must degrade to world Z (axis 0)");
    assert!(p.timeline.iter().find(|n| n.id == arr).unwrap().dirty, "the consumer must be marked dirty so it rebuilds");
}

#[test]
fn reorder_respects_datum_dependency() {
    // Checking only body dependencies lets an offset plane be moved above its source, so it resolves against
    // stale data. Datum dependencies gate the reorder as well.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let base = p.add_offset_plane(BasePlane::XY, 10.0);
    let child = p.add_offset_from_plane(base, 5.0);
    let bi = p.timeline_index(base).unwrap();
    let ci = p.timeline_index(child).unwrap();
    assert!(ci > bi, "the child plane must be created after its source");
    assert!(!p.can_reorder_feature(ci, bi), "an offset plane must not be movable above its source");
}

#[test]
fn a_one_sided_cut_is_handed_with_ends_to_meet_the_body() {
    // A cut from a sketch on a face used to leave a cap at the entry, where the tool end face coincided with
    // the body face, so the hole was not a through one. The model hands a one-sided cut as such, from the sketch
    // plane for its depth, and the bridge meets its ends with the body (`ops::profile::meet_ends`); a boss is
    // handed with its ends kept.
    use qymcad_core::feature::{ToolEnds, ToolSpan};
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    p.add_combine_on(base, sid, 0, qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 0);
    p.add_combine_on(base, sid, 0, qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 1);
    let k = MockKernel::default();
    p.regenerate(&k);
    let spans = k.spans.borrow().clone();
    assert!(spans.contains(&ToolSpan { start: 0.0, length: 5.0, ends: ToolEnds::OneSidedCut }), "the cut is not handed as a one-sided cut of 5 from the plane: {spans:?}");
    assert!(spans.contains(&ToolSpan { start: 0.0, length: 5.0, ends: ToolEnds::Kept }), "the boss is not handed with its ends kept: {spans:?}");
}

#[test]
fn active_component_nests_new_nodes() {
    let mut p = part_project();
    // Create a component and make it active.
    let comp = p.add_component("Part 1");
    p.set_active_component(Some(comp));
    // New nodes land in the active component.
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    assert_eq!(p.node_component(sid), Some(comp), "the sketch must nest inside the active component");
    assert_eq!(p.node_component(body), Some(comp), "the body must nest inside the active component");
    // Outside an active component nodes land in the root assembly, an explicit root rather than `None`.
    p.set_active_component(None);
    let sid2 = square(&mut p, "s2");
    p.add_sketch_node(sid2, "s2");
    assert_eq!(p.node_component(sid2), Some(p.root), "outside a component nodes land in the root assembly");
}

#[test]
fn components_survive_serde() {
    let mut p: Project = Default::default();
    let comp = p.add_component("Node");
    p.set_active_component(Some(comp));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.components.len(), 2, "the root assembly plus the created part");
    assert_eq!(back.active_component, Some(comp));
    assert_eq!(back.node_component(sid), Some(comp));
}

#[test]
fn primitives_build_via_kernel() {
    let mut p = part_project();
    let bx = p.add_box(10.0, 20.0, 5.0);
    let cy = p.add_cylinder(4.0, 8.0);
    let sp = p.add_sphere(3.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    // Three primitives were built by the mock kernel and their bodies created.
    assert!(p.mesh_index(bx).is_some(), "box");
    assert!(p.mesh_index(cy).is_some(), "cylinder");
    assert!(p.mesh_index(sp).is_some(), "sphere");
    assert_eq!(k.count(), 3);
    // Primitives have no sketch inputs.
    let bn = p.timeline.iter().find(|n| n.id == bx).unwrap();
    assert!(bn.kind.inputs().is_empty());
    assert_eq!(bn.kind.body(), Some(bx));
}

#[test]
fn combine_cut_needs_built_src_then_rebuilds() {
    let mut p = part_project();
    // The base body: a box.
    let base = p.add_box(20.0, 20.0, 10.0);
    // The tool sketch of the cut.
    let cut = square(&mut p, "cut");
    p.add_sketch_node(cut, "cut");
    let body = p.add_combine(base, cut, 10.0, 0); // A cut.
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    // The source box was built first, so the combine found its shape and produced a body.
    assert!(rep.built.iter().any(|(b, _)| *b == body), "the cut must be built");
    assert!(p.mesh_index(body).is_some());
    // It depends on both the source body and the sketch.
    let n = p.timeline.iter().find(|n| n.id == body).unwrap();
    assert_eq!(n.kind.inputs(), vec![base, cut]);
    assert_eq!(n.kind.consumed_body(), Some(base), "the box must be consumed by the cut and hidden");
}

#[test]
fn fillet_chamfer_consume_source_body() {
    let mut p = part_project();
    let cyl = p.add_cylinder(10.0, 20.0);
    let fil = p.add_fillet(cyl, 2.0, vec![]);
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.built.iter().any(|(b, _)| *b == fil), "the fillet must be built");
    let n = p.timeline.iter().find(|n| n.id == fil).unwrap();
    assert_eq!(n.kind.inputs(), vec![cyl]);
    assert_eq!(n.kind.consumed_body(), Some(cyl));
    // Editing the radius of the source cylinder rebuilds the fillet downstream.
    let before = k.count();
    p.mark_node_dirty(cyl);
    p.regenerate(&k);
    assert!(k.count() > before, "the source changed, so the fillet must be rebuilt");
}

#[test]
fn chamfer_modes_route_and_are_parametric() {
    use qymcad_core::feature::ChamferMode;
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "k".into(), expr: "3".into(), value: 3.0 });

    // A symmetric chamfer over every edge calls the plain `chamfer` rather than `chamfer_ex`.
    let cyl = p.add_cylinder(10.0, 20.0);
    let sym = p.add_chamfer(cyl, 1.5, vec![]);
    // Two setbacks over selected edges call `chamfer_ex`; d1 and d2 are parametric and `flip` reaches the
    // kernel.
    let two = p.add_chamfer_ex(sym, 2.0, qymcad_core::model::ChamferShape { mode: ChamferMode::TwoDist, d2: 1.0, flip: true, ref_face: 0 }, vec![7]);
    p.set_feat_dim(two, "dist", "k".into()); // d1 = k = 3
    p.set_feat_dim(two, "d2", "k/2".into()); // d2 = 1.5
                                             // Setback plus angle calls `chamfer_ex` in `DistAngle` mode with a manually chosen reference face, so
                                             // `ref_face` reaches the kernel.
    let da = p.add_chamfer_ex(two, 2.5, qymcad_core::model::ChamferShape { mode: ChamferMode::DistAngle, d2: 30.0, flip: false, ref_face: 42 }, vec![9]);

    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c == "chamfer d=1.5 n=0"), "a symmetric chamfer must call `chamfer`: {calls:?}");
    assert!(calls.iter().any(|c| c == "chamfer_ex d1=3 d2=1.5 mode=TwoDist flip=true rf=0 n=1"), "two setbacks must be parametric: {calls:?}");
    assert!(calls.iter().any(|c| c == "chamfer_ex d1=2.5 d2=30 mode=DistAngle flip=false rf=42 n=1"), "setback plus angle with a manual reference face: {calls:?}");
    drop(calls);

    // Changing parameter k rebuilds the two-setback chamfer with new d1 and d2.
    p.parameters[0].value = 4.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c == "chamfer_ex d1=4 d2=2 mode=TwoDist flip=true rf=0 n=1"), "after setting k = 4: {:?}", k2.calls.borrow());
    let _ = da;
}

#[test]
fn combine_fails_if_src_missing() {
    // A combine against a non-existent source body errors and the node stays dirty.
    let mut p = part_project();
    let cut = square(&mut p, "cut");
    p.add_sketch_node(cut, "cut");
    let body = p.add_combine(999, cut, 5.0, 0); // Source 999 does not exist.
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.iter().any(|(n, _)| *n == body), "an error must be reported: no source");
    assert!(rep.built.iter().all(|(b, _)| *b != body));
}

#[test]
fn cone_torus_prism_build() {
    let mut p = part_project();
    let cn = p.add_cone(10.0, 0.0, 20.0);
    let tr = p.add_torus(12.0, 4.0);
    let pr = p.add_prism(10.0, 6, 20.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    for b in [cn, tr, pr] {
        assert!(p.mesh_index(b).is_some(), "the primitive must be built");
        let n = p.timeline.iter().find(|n| n.id == b).unwrap();
        assert!(n.kind.inputs().is_empty(), "a primitive has no inputs");
        assert_eq!(n.kind.body(), Some(b));
    }
    // A cone and a torus are revolved, a prism is extruded.
    assert_eq!(k.count(), 3);
}

#[test]
fn shell_and_arrays_build_and_consume_source() {
    let mut p = part_project();
    let cube = p.add_box(20.0, 20.0, 20.0);
    let sh = p.add_shell(cube, 2.0, vec![1], false);
    let la = p.add_linear_array(sh, 25.0, 0.0, 0.0, 3);
    let ca = p.add_circular_array(la, 6, 360.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    for b in [sh, la, ca] {
        assert!(p.mesh_index(b).is_some());
    }
    // Every modifier consumes its own source body.
    let node = |id: u64| p.timeline.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node(sh).kind.consumed_body(), Some(cube));
    assert_eq!(node(la).kind.consumed_body(), Some(sh));
    assert_eq!(node(ca).kind.consumed_body(), Some(la));
    // A circular pattern of 6 copies calls the kernel with 6 transforms.
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")));
    // Editing the box rebuilds the whole chain below it.
    let before = k.count();
    p.mark_node_dirty(cube);
    p.regenerate(&k);
    assert!(k.count() >= before + 4, "box, shell, linear pattern and circular pattern must all rebuild");
}

#[test]
fn mirror_and_hole_build_consume_source() {
    let mut p = part_project();
    let cube = p.add_box(20.0, 20.0, 20.0);
    let hole = p.add_hole(cube, FaceKey { index: 0, centroid: [10.0, 10.0, 20.0], normal: [0.0, 0.0, 1.0], id: 0 }, 6.0, 15.0);
    let mir = p.add_mirror(hole, 1, true, 0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(hole).is_some() && p.mesh_index(mir).is_some());
    // A hole goes through `kernel.hole` (a cutting tool) and a mirror through `mirror`.
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("hole k=0")), "a hole must be a cut");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("mirror")), "the mirror must be called");
    let node = |id: u64| p.timeline.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node(hole).kind.consumed_body(), Some(cube));
    assert_eq!(node(mir).kind.consumed_body(), Some(hole));
}
