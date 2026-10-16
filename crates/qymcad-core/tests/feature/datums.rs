//! Directions of extrudes and combines, datum planes, axes and points, revolves and holes.

use super::*;

#[test]
fn extrude_extent_direction() {
    // Extrude and cut direction: one-sided, reversed (flip against the normal), symmetric, and two-sided.
    use qymcad_core::feature::extrude_extent;
    assert_eq!(extrude_extent(5.0, 0.0, qymcad_core::feature::Reach::Forward), (0.0, 5.0), "one-sided: [0, +h]");
    assert_eq!(extrude_extent(5.0, 0.0, qymcad_core::feature::Reach::Backward), (-5.0, 5.0), "flipped: [-h, 0] against the normal");
    assert_eq!(extrude_extent(6.0, 0.0, qymcad_core::feature::Reach::BothWays), (-3.0, 6.0), "symmetric: [-h/2, +h/2]");
    assert_eq!(extrude_extent(5.0, 3.0, qymcad_core::feature::Reach::Forward), (-3.0, 8.0), "two-sided: [-down, +h]");
    assert_eq!(extrude_extent(5.0, 3.0, qymcad_core::feature::Reach::Backward), (-5.0, 8.0), "two-sided plus flip: up and down swap");
}

#[test]
fn regen_applies_direction_to_placement() {
    // Regenerate really does apply the direction and the flip to the placement handed to the kernel, not only
    // to `extrude_extent` in isolation. The mock encodes `start` in the translation columns; with the sketch on
    // world XY (X = (1,0,0), N = (0,0,1)) the vertex becomes (total, 0, start), where z is the offset of the
    // start along the normal.
    let mut p = part_project();
    let sid = square(&mut p, "square"); // On world XY.
    p.add_sketch_node(sid, "square");
    let up = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0); // Outwards: [0, +5].
    let dn = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Backward, 0.0); // Flipped: [-5, 0].
    let sym = p.add_extrude_on(sid, 0, 6.0, qymcad_core::feature::Reach::BothWays, 0.0); // Symmetric: [-3, +6].
    let two = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 3.0); // Two-sided: [-3, +8].
    let k = MockKernel::default();
    p.regenerate(&k);
    let z = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].z;
    let x = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].x;
    assert_eq!(z(up), 0.0, "outwards: the profile sits on the plane (start 0)");
    assert_eq!(x(up), 5.0, "outwards: a height of 5 (total)");
    assert_eq!(z(dn), -5.0, "flipped: the profile moved against the normal (start -5)");
    assert_eq!(z(sym), -3.0, "symmetric: start is -h/2");
    assert_eq!(x(sym), 6.0, "symmetric: total is h");
    assert_eq!(z(two), -3.0, "two-sided: start is -down");
    assert_eq!(x(two), 8.0, "two-sided: total is h plus down");
}

#[test]
fn regen_combine_direction_matches_extrude() {
    // A cut or a boss is directional in the same way as an extrude (flip, symmetry, two sides) rather than
    // only "through all". The tool placement is checked through the mock kernel, where z is the start along the
    // normal.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // One-sided plus flip starts the tool against the normal, at -h.
    let cut_flip = p.add_combine_on(
        base,
        sid,
        0,
        qymcad_core::model::CombineSpan { height: 4.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, fill: &[] },
        0,
    );
    // Two-sided with down = 2 starts at -down.
    let cut_two = p.add_combine_on(base, sid, 0, qymcad_core::model::CombineSpan { height: 4.0, down: 2.0, extent: qymcad_core::feature::Extent::default(), fill: &[] }, 0);
    // Symmetric starts at -h/2.
    let cut_sym = p.add_combine_on(
        base,
        sid,
        0,
        qymcad_core::model::CombineSpan { height: 4.0, down: 0.0, extent: qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::BothWays, ..Default::default() }, fill: &[] },
        0,
    );
    let k = MockKernel::default();
    p.regenerate(&k);
    let z = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].z;
    assert_eq!(z(cut_flip), -4.0, "a flipped cut puts the tool against the normal (start -h)");
    assert_eq!(z(cut_two), -2.0, "two-sided: start is -down");
    assert_eq!(z(cut_sym), -2.0, "symmetric: start is -h/2");
}

#[test]
fn combine_direction_survives_serde() {
    // The reach and the second side of a combine survive saving and loading.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let cut = p.add_combine_on(
        base,
        sid,
        0,
        qymcad_core::model::CombineSpan { height: 4.0, down: 2.5, extent: qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::BothWays, ..Default::default() }, fill: &[] },
        0,
    );
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    let (reach, down) = back.timeline.iter().find_map(|n| if let FeatureKind::Combine { extent, down, .. } = n.kind { Some((extent.reach, down)) } else { None }).unwrap();
    assert_eq!(reach, qymcad_core::feature::Reach::BothWays, "the direction of the tool must be saved");
    assert_eq!(down, 2.5, "the second side must be saved");
    let _ = cut;
}

#[test]
fn offset_plane_resolves_from_base() {
    // A parametric plane is an offset from a base plane, and its origin and normal are resolved during
    // regenerate.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let wp = WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::XY, dist: 20.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.normal, [0.0, 0.0, 1.0], "the normal of the base XY plane is +Z");
    assert_eq!(pl.origin, [0.0, 0.0, 20.0], "the origin must be offset by 20 along Z");
}

#[test]
fn offset_plane_dist_is_parametric() {
    // The distance of an offset plane is an expression over a global parameter (the `dist` feature dimension).
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{Param, PlaneDef, WorkPlane};
    let mut p = part_project();
    p.parameters.push(Param { name: "h".into(), expr: "30".into(), value: 30.0 });
    let wp = WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::XZ, dist: 5.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.set_feat_dim(pid, "dist", "h".into()); // dist = h = 30; the XZ plane has normal -Y.
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.origin[1].abs(), 30.0, "the distance from the expression h = 30 must apply along the XZ normal");
}

#[test]
fn datum_point_coords_are_parametric() {
    // The coordinates of a datum point are expressions (the x, y and z feature dimensions), so a global
    // parameter moves the point.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "g".into(), expr: "7".into(), value: 7.0 });
    let id = p.add_point_at([1.0, 2.0, 3.0]);
    p.set_feat_dim(id, "z", "g*2".into()); // z = 14
    p.regenerate(&MockKernel::default());
    let pt = p.datum_points.iter().find(|d| d.id == id).unwrap();
    assert_eq!(pt.at, [1.0, 2.0, 14.0], "z comes from the expression g*2 = 14 while x and y are stored numbers");
    // Changing the parameter moves the point without editing the feature.
    p.parameters[0].value = 10.0;
    p.mark_param_dependents_dirty();
    p.regenerate(&MockKernel::default());
    assert_eq!(p.datum_points.iter().find(|d| d.id == id).unwrap().at[2], 20.0, "g=10 → z=20");
}

#[test]
fn offset_plane_from_datum_plane_resolves() {
    // A plane offset from another datum plane is parametric and follows its source.
    use qymcad_core::feature::BasePlane;
    let mut p = part_project();
    let base = p.add_offset_plane(BasePlane::XY, 10.0); // origin z=10, normal +Z
    let child = p.add_offset_from_plane(base, 5.0); // Another +5 along the source normal, giving z = 15.
    p.regenerate(&MockKernel::default());
    let c = p.planes.iter().find(|x| x.id == child).unwrap();
    assert_eq!(c.normal, [0.0, 0.0, 1.0]);
    assert!((c.origin[2] - 15.0).abs() < 1e-9, "z must be 15 (10 from the base plus 5): {:?}", c.origin);
    // Moving the source (dist = 30) moves the child to z = 35. Datums are resolved on every regenerate rather
    // than only when dirty.
    p.set_feat_dim(base, "dist", "30".into());
    p.regenerate(&MockKernel::default());
    assert!((p.planes.iter().find(|x| x.id == child).unwrap().origin[2] - 35.0).abs() < 1e-9, "the child must follow its source");
}

#[test]
fn datum_point_at_vertex_is_associative() {
    // An `AtVertex` datum point resolves through the kernel, as an endpoint of a persistent edge, and travels
    // with that vertex.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let pa = p.add_point_at_vertex([0.0; 3], body, 7, false); // The start of edge 7.
    let pb = p.add_point_at_vertex([0.0; 3], body, 7, true); // The end of edge 7.
    let k = MockKernel::default();
    k.edges.borrow_mut().push(qymcad_core::geom::MeshEdge { id: 7, mid: [0.0; 3], dir: [0.0, 0.0, 1.0], a: [1.0, 2.0, 3.0], b: [4.0, 5.0, 6.0], ..Default::default() });
    p.regenerate(&k);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pa).unwrap().at, [1.0, 2.0, 3.0], "`AtVertex` with end = false is the start of the edge");
    assert_eq!(p.datum_points.iter().find(|d| d.id == pb).unwrap().at, [4.0, 5.0, 6.0], "`AtVertex` with end = true is the end of the edge");
    // The vertex moved (the source was rebuilt) and the point follows.
    k.edges.borrow_mut()[0].a = [9.0, 9.0, 9.0];
    p.regenerate(&k);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pa).unwrap().at, [9.0, 9.0, 9.0], "an `AtVertex` point must follow its vertex");
}

#[test]
fn datum_axis_from_edge_and_face_are_associative() {
    // A `FromEdge` or `FromFace` axis resolves through the kernel (the midpoint and tangent of an edge, or the
    // axis of a face) and travels with its source.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let axe = p.add_axis_from_edge(body, 7);
    let axf = p.add_axis_from_face(body, 3);
    let k = MockKernel::default();
    k.edges.borrow_mut().push(qymcad_core::geom::MeshEdge { id: 7, mid: [1.0, 2.0, 3.0], dir: [0.0, 0.0, 1.0], a: [0.0; 3], b: [0.0; 3], ..Default::default() });
    *k.face_axis.borrow_mut() = Some(([4.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    p.regenerate(&k);
    let e = p.datum_axes.iter().find(|a| a.id == axe).unwrap();
    assert_eq!((e.origin(), e.dir()), ([1.0, 2.0, 3.0], [0.0, 0.0, 1.0]), "`FromEdge` gives the midpoint and tangent of the edge");
    let f = p.datum_axes.iter().find(|a| a.id == axf).unwrap();
    assert_eq!((f.origin(), f.dir()), ([4.0, 0.0, 0.0], [1.0, 0.0, 0.0]), "`FromFace` gives the axis of the face");
    // The edge moved and the axis follows: associativity.
    k.edges.borrow_mut()[0].mid = [9.0, 9.0, 9.0];
    p.regenerate(&k);
    assert_eq!(p.datum_axes.iter().find(|a| a.id == axe).unwrap().origin(), [9.0, 9.0, 9.0], "a `FromEdge` axis must follow its edge");
}

#[test]
fn revolve_around_datum_axis_uses_axis_kernel() {
    // A non-zero `axis_datum` on a revolve makes regenerate convert the datum axis from world into local space
    // and call `revolve_region_axis`.
    use qymcad_core::model::DatumAxis;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    // A datum axis along Y through (-3,0,0) in world space; the sketch is on XY with an identity placement, so
    // the local axis is the same. The axis is deliberately placed beyond the profile: a profile crossing the
    // axis is now rejected honestly, and checking axis resolution on such a profile would test the wrong
    // thing.
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [-3.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let rev = p.add_revolve_axis(sid, Vec::new(), 0, 270.0, ax, 0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c.starts_with("revolve_axis o=[-3.0, 0.0, 0.0] d=[0.0, 1.0, 0.0] a=270")), "it must revolve about the datum axis: {calls:?}");
    drop(calls);
    assert!(p.mesh_index(rev).is_some());
    // With the axis deleted it falls back to an ordinary revolve about the sketch X or Y axis.
    p.delete_datum_axis(ax);
    p.mark_param_dependents_dirty();
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == rev) {
        n.dirty = true;
    }
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c.starts_with("revolve a=270")), "it must fall back to the sketch axes: {:?}", k2.calls.borrow());
}

#[test]
fn revolve_around_sketch_centerline_uses_line_local() {
    // A non-zero `axis_line` on a revolve makes regenerate take the 2D endpoints of the sketch centreline as
    // the local axis — no inverse placement is needed, those already being sketch plane coordinates — and call
    // `revolve_region_axis`.
    let mut p = part_project();
    let si = p.new_sketch("sphere");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "sphere");
    // The profile does not cross the axis, the circle sitting to its right, which is how a body of revolution
    // is built anywhere. The crossing case is checked separately, where it now produces an honest error with a
    // hint.
    p.add_circle_entity(si, 6.0, 5.0, 3.0, qymcad_core::feature::Purpose::Real);
    let axis = p.add_line_entity(si, 0.0, -10.0, 0.0, 10.0, qymcad_core::feature::Purpose::Construction); // The centreline at x = 0.
    let rev = p.add_revolve_axis(sid, Vec::new(), 0, 360.0, 0, axis);
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    // In local space the line endpoints are taken as they are (the placement is the identity): the origin is
    // the first endpoint and the direction is the unit vector along the line.
    assert!(calls.iter().any(|c| c.starts_with("revolve_axis o=[0.0, -10.0, 0.0] d=[0.0, 1.0, 0.0] a=360")), "it must revolve about the sketch centreline: {calls:?}");
    drop(calls);
    assert!(p.mesh_index(rev).is_some(), "the body of revolution must be built");

    // A centreline outranks a datum: with a non-zero `axis_line` the line is used even when `axis_datum` is
    // set.
    use qymcad_core::model::DatumAxis;
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [50.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    if let Some(qymcad_core::feature::FeatureKind::Revolve { axis_datum, .. }) = p.timeline.iter_mut().find(|n| n.id == rev).map(|n| &mut n.kind) {
        *axis_datum = ax;
    }
    p.mark_param_dependents_dirty();
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == rev) {
        n.dirty = true;
    }
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(k3.calls.borrow().iter().any(|c| c.starts_with("revolve_axis o=[0.0, -10.0, 0.0]")), "the centreline must outrank the datum: {:?}", k3.calls.borrow());
}

#[test]
fn hole_typed_routes_kind_and_recess_to_kernel() {
    // The hole type (counterbore or countersink) together with the diameter and depth of the recess reach the
    // kernel through `kernel.hole`.
    use qymcad_core::feature::FaceKey;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    let key = FaceKey { index: 0, centroid: [5.0, 5.0, 10.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let h = p.add_hole_typed(base, key, qymcad_core::model::HoleTool { kind: 1, diameter: 4.0, depth: 8.0, dia2: 9.0, depth2: 3.0 }); // A counterbore.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "hole k=1 dia=4 depth=8 dia2=9 depth2=3"), "the type and the recess must reach the kernel: {:?}", k.calls.borrow());
    assert!(p.mesh_index(h).is_some());
    // A plain hole gives kind 0 with a zero recess.
    let h2 = p.add_hole(base, key, 3.0, 5.0);
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "hole k=0 dia=3 depth=5 dia2=0 depth2=0"), "a plain hole: {:?}", k.calls.borrow());
    let _ = h2;
}

#[test]
fn holes_from_sketch_points_route_to_kernel() {
    // Holes at the isolated points of a sketch make one `kernel.holes` call with N placements.
    use qymcad_core::model::Param;
    let mut p = part_project();
    let base_sk = square(&mut p, "square");
    p.add_sketch_node(base_sk, "square");
    let base = p.add_extrude_on(base_sk, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    // A separate sketch with three drill marks (isolated points) plus a segment, whose endpoints do not
    // count.
    let hsi = p.new_sketch("drill marks");
    let holes_sk = p.sketches[hsi].id;
    p.add_sketch_node(holes_sk, "drill marks");
    p.sketch_point_at(hsi, 2.0, 2.0, 1e-6);
    p.sketch_point_at(hsi, 8.0, 2.0, 1e-6);
    p.sketch_point_at(hsi, 5.0, 8.0, 1e-6);
    p.add_line_entity(hsi, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real); // The endpoints (0,0) and (10,0) are not drill marks.
    assert_eq!(p.sketch_isolated_points(holes_sk).len(), 3, "there must be three isolated points");
    let h = p.add_hole_from_sketch(base, holes_sk, qymcad_core::model::HoleTool { kind: 0, diameter: 4.0, depth: 8.0, dia2: 0.0, depth2: 0.0 }, false);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "holes k=0 n=3 dia=4 depth=8 dia2=0 depth2=0"), "three holes must reach the kernel in one call: {:?}", k.calls.borrow());
    assert!(p.mesh_index(h).is_some());
    // A parametric diameter driven by a global parameter.
    p.parameters.push(Param { name: "hd".into(), expr: "6".into(), value: 6.0 });
    p.set_feat_dim(h, "diameter", "hd".into());
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c == "holes k=0 n=3 dia=6 depth=8 dia2=0 depth2=0"), "the diameter parameter must apply: {:?}", k2.calls.borrow());
}

#[test]
fn feature_inserted_at_rollback_line() {
    // With the rollback bar active a new feature is inserted at the bar, becoming the last active node, and
    // the bar moves past it.
    let mut p = part_project();
    let _a = p.add_box(10.0, 10.0, 10.0);
    let c = p.add_box(5.0, 5.0, 5.0);
    let ti = |p: &qymcad_core::model::Project, id: u64| p.timeline.iter().position(|n| n.id == id).unwrap();
    let ci = ti(&p, c);
    p.set_rollback(Some(ci)); // The bar sits before c, so only the first box is active.
    let d = p.add_box(2.0, 2.0, 2.0); // It must land at the bar, before c.
    assert_eq!(ti(&p, d), ci, "the new feature must sit at the position of the bar");
    assert!(ti(&p, d) < ti(&p, c), "d must be above c");
    assert_eq!(p.rollback, Some(ci + 1), "the bar must move past the new feature");
    // Without a rollback bar it goes to the end.
    p.set_rollback(None);
    let e = p.add_box(1.0, 1.0, 1.0);
    assert_eq!(ti(&p, e), p.timeline.len() - 1, "without a rollback bar it must go to the end of the timeline");
}

#[test]
fn datum_helpers_build_expected_defs() {
    // The single datum constructors the interface calls: an offset plane, a point, a manual axis and an axis
    // through two points.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{AxisDef, PlaneDef};
    let mut p = part_project();
    let pl = p.add_offset_plane(BasePlane::XY, 12.0);
    assert!(matches!(p.planes.iter().find(|x| x.id == pl).unwrap().def, PlaneDef::OffsetBase { base: BasePlane::XY, dist } if (dist - 12.0).abs() < 1e-9));
    let pt = p.add_point_at([3.0, 4.0, 5.0]);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pt).unwrap().at, [3.0, 4.0, 5.0]);
    let axm = p.add_axis_manual([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let am = p.datum_axes.iter().find(|a| a.id == axm).unwrap();
    assert!(matches!(am.def, AxisDef::Manual { .. }) && am.dir() == [0.0, 1.0, 0.0]);
    let ax2 = p.add_axis_two_points(pt, axm); // Arbitrary ids: only the definition is checked.
    assert!(matches!(p.datum_axes.iter().find(|a| a.id == ax2).unwrap().def, AxisDef::TwoPoints { .. }));
    // Every datum is a timeline node.
    for id in [pl, pt, axm, ax2] {
        assert!(p.timeline.iter().any(|n| n.id == id), "datum {id} must be in the timeline");
    }
}

#[test]
fn axis_through_two_points_resolves() {
    // An axis through two datum points gives origin = A and dir = norm(B - A), and follows the coordinates of
    // those points.
    use qymcad_core::model::{AxisDef, DatumAxis, DatumPoint};
    let mut p = part_project();
    let ida = p.add_datum_point(DatumPoint { at: [1.0, 0.0, 0.0], ..Default::default() });
    let idb = p.add_datum_point(DatumPoint { at: [1.0, 0.0, 10.0], ..Default::default() });
    let ax = DatumAxis::from_def("Axis", AxisDef::TwoPoints { a: ida, b: idb });
    let axid = p.add_datum_axis(ax);
    p.regenerate(&MockKernel::default());
    let res = p.datum_axes.iter().find(|x| x.id == axid).unwrap();
    assert_eq!(res.origin(), [1.0, 0.0, 0.0], "the origin must be point A");
    assert_eq!(res.dir(), [0.0, 0.0, 1.0], "dir = norm(B−A) = +Z");
    // Moving point B reorients the axis.
    let bi = p.datum_points.iter().position(|d| d.id == idb).unwrap();
    p.datum_points[bi].at = [11.0, 0.0, 0.0];
    p.regenerate(&MockKernel::default());
    let res2 = p.datum_axes.iter().find(|x| x.id == axid).unwrap();
    assert_eq!(res2.dir(), [1.0, 0.0, 0.0], "after moving B the axis must run along +X: associativity");
}

#[test]
fn offset_plane_from_face_resolves() {
    // A plane offset from a face of a body, by persistent key. The mock kernel supplies no faces, so resolution
    // falls back to the fingerprint (centroid and normal) and the origin becomes the face centre plus the normal
    // times the distance.
    use qymcad_core::feature::FaceKey;
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let key = FaceKey { index: 0, centroid: [1.0, 2.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let wp = WorkPlane { def: PlaneDef::OffsetFace { body: base, face: key, dist: 3.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.normal, [0.0, 0.0, 1.0], "the face normal");
    assert_eq!(pl.origin, [1.0, 2.0, 8.0], "the origin must be the face centre plus the normal times 3");
    // `OffsetFace` survives serialisation.
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert!(matches!(back.planes.iter().find(|x| x.id == pid).unwrap().def, PlaneDef::OffsetFace { dist, .. } if (dist - 3.0).abs() < 1e-9));
}

#[test]
fn circular_array_around_datum_axis() {
    // A circular pattern about a chosen datum axis rather than only about Z; the axis is stored and survives
    // serialisation.
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::DatumAxis;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [0.0; 3], [1.0, 0.0, 0.0])); // The X axis.
    let arr = p.add_circular_array_axis(base, 4, 360.0, ax);
    p.regenerate(&MockKernel::default());
    assert!(p.mesh_index(arr).is_some(), "the pattern about a datum axis must be built");
    let stored = p.timeline.iter().find_map(|n| if let FeatureKind::CircularArray { axis, .. } = n.kind { Some(axis) } else { None }).unwrap();
    assert_eq!(stored, ax, "the pattern axis must be the datum axis");
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    let stored2 = back.timeline.iter().find_map(|n| if let FeatureKind::CircularArray { axis, .. } = n.kind { Some(axis) } else { None }).unwrap();
    assert_eq!(stored2, ax, "the pattern axis must survive serialisation");
}

#[test]
fn datum_defs_survive_serde() {
    // Parametric datum definitions survive serialisation.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{AxisDef, DatumAxis, DatumPoint, PlaneDef, WorkPlane};
    let mut p = part_project();
    let pid = p.add_plane(WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::YZ, dist: 7.0 }, ..Default::default() });
    let a = p.add_datum_point(DatumPoint::default());
    let b = p.add_datum_point(DatumPoint::default());
    let axid = p.add_datum_axis(DatumAxis::from_def("Axis", AxisDef::TwoPoints { a, b }));
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert_eq!(back.planes.iter().find(|x| x.id == pid).unwrap().def, PlaneDef::OffsetBase { base: BasePlane::YZ, dist: 7.0 });
    assert_eq!(back.datum_axes.iter().find(|x| x.id == axid).unwrap().def, AxisDef::TwoPoints { a, b });
}

#[test]
fn external_ref_survives_serde() {
    let mut p = part_project();
    let id = p.add_external_face_ref(7, 42, FaceKey { index: 1, centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], id: 9 });
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.external_refs.len(), 1, "the external reference must survive serialisation");
    let r = &back.external_refs[0];
    assert_eq!(r.id, id);
    assert_eq!(r.from_component, 7);
    assert_eq!(r.source_body(), Some(42));
}
