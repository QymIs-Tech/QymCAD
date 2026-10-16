//! Contours, parametric dimensions of features, references by face, moves, rollback, arrays, pruning, booleans.

use super::*;

#[test]
fn extrude_picks_chosen_contour() {
    // A sketch with two closed contours: a specific one is extruded (`profile = cid`) rather than the
    // first.
    let mut p = part_project();
    let sid = p.add_line_sketch("two shapes", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let si = p.sketch_index(sid).unwrap();
    // The second square contour is added as entities followed by a regenerate.
    p.add_rect_entity(si, 20.0, 0.0, 30.0, 8.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_profile_xy(*cid).is_some()).collect();
    assert!(closed.len() >= 2, "there must be two closed contours: {}", closed.len());
    let chosen = closed[1];
    let body = p.add_extrude_on(sid, chosen, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // The feature holds the selected profile.
    let prof = p.timeline.iter().find_map(|n| if let qymcad_core::feature::FeatureKind::Extrude { profiles, .. } = &n.kind { profiles.first().copied() } else { None }).unwrap();
    assert_eq!(prof, chosen, "the selected contour must be the one extruded");
    // The feature profile is the XY of the second contour rather than the first.
    let xy = p.feature_profile_xy(sid, chosen).unwrap();
    let xs: Vec<f64> = xy.iter().step_by(2).copied().collect();
    assert!(xs.iter().cloned().fold(0.0_f64, f64::max) >= 20.0, "the second contour must be taken (x up to 30): {xs:?}");
    let _ = body;
}

#[test]
fn extrude_two_sided_and_combine_through_stored() {
    // The extent parameters reach the features: `down` for two sides and `through` for through all.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let sid = p.add_line_sketch("rectangle", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 3.0); // Second side of 3.
    let down = p.timeline.iter().find_map(|n| if let FeatureKind::Extrude { down, .. } = n.kind { Some(down) } else { None }).unwrap();
    assert_eq!(down, 3.0, "the second side must be stored in the feature");
    let cut = p.add_combine_on(base, sid, 0, qymcad_core::model::CombineSpan { height: 5.0, down: 0.0, extent: qymcad_core::feature::Extent { through: true, ..Default::default() }, fill: &[] }, 0); // A through cut.
    let through = p.timeline.iter().find_map(|n| if let FeatureKind::Combine { extent, .. } = n.kind { Some(extent.through) } else { None }).unwrap();
    assert!(through, "through all must be stored in the cut feature");
    let _ = cut;
}

#[test]
fn feature_dim_expression_is_parametric() {
    // A feature dimension can be an expression over a global parameter and rebuilds when that parameter
    // changes.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "h".into(), expr: "20".into(), value: 20.0 });
    let sid = p.add_line_sketch("square", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0); // The plain number 5.
    p.set_feat_dim(body, "height", "h".into()); // height = h
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.contains("h=20")), "height=h → 20: {:?}", k.calls.borrow());
    // Changing the parameter dirties the dependent features and they rebuild with the new value.
    p.parameters[0].value = 30.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c.contains("h=30")), "after the parameter change it must be 30: {:?}", k2.calls.borrow());
}

#[test]
fn delete_sketch_cascades_dependent_bodies() {
    // Deleting a sketch removes the bodies built on it together with their timeline nodes.
    let mut p = part_project();
    let sid = p.add_line_sketch("square", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(body, 1.0, Vec::<u32>::new()); // Depends on the body, so it cascades too.
    let before = p.timeline.len();
    assert!(before >= 3, "the nodes are a sketch, an extrude and a fillet");
    let removed = p.delete_sketch_with_dependents(sid);
    assert!(removed.contains(&body) && removed.contains(&fil), "both bodies must be removed by the cascade: {removed:?}");
    assert!(p.sketch_index(sid).is_none(), "the sketch must be deleted");
    assert!(p.timeline.iter().all(|n| n.kind.body() != Some(body) && n.kind.body() != Some(fil)), "the body nodes must be deleted");
    assert!(p.mesh_index(body).is_none() && p.mesh_index(fil).is_none(), "the meshes must be deleted");
    // The rebuild caches go too. Dropping the mesh while leaving `regen_faces` behind leaves a body that no
    // timeline node produces counted as alive: measured on a loft, a two-node timeline still reported the body
    // as live with its previous three faces. Everything that enumerates live bodies counts such a ghost, from
    // the document tree to the machine output.
    assert!(!p.regen_faces.contains_key(&body) && !p.regen_faces.contains_key(&fil), "the faces of deleted bodies must leave the rebuild cache: {:?}", p.regen_faces.keys().collect::<Vec<_>>());
    assert!(!p.regen_edges.contains_key(&body) && !p.regen_edges.contains_key(&fil), "the edges of deleted bodies must leave the cache");
}

#[test]
fn face_ref_resolves_by_persistent_id() {
    // A face reference holds on to the persistent id, so after a rebuild the sketch travels with the face even
    // when the key still carries a stale centre.
    let mut p = part_project();
    let si = p.new_sketch("s");
    let body = 777u64;
    p.regen_faces.insert(
        body,
        vec![
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(1.0, 1.0, 5.0), area: 1.0, id: 10 },
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(9.0, 9.0, 5.0), area: 1.0, id: 20 },
        ],
    );
    // A sketch on face id 20, whose key holds a stale centre from before the rebuild.
    let key = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 20 };
    // Resolving by the persistent id 20 gives the current face centre (9,9,5) rather than the stale key.
    assert_eq!(p.resolve_face(body, &key).0, [9.0, 9.0, 5.0], "the face must resolve by id 20 to its current centre");
    p.sketches[si].plane = SketchPlane::Face(body, key);
    // The 2D zero of a sketch on a face is the projection of the part origin onto the face plane at z = 5,
    // giving [0,0,5].
    let f = p.sketch_frame(si).unwrap();
    assert_eq!(f.origin, [0.0, 0.0, 5.0], "the origin of a sketch on a face is the projection of the part zero onto the plane");
}

#[test]
fn move_feature_transforms_brep() {
    // Moving a body is a parametric feature: it moves the B-rep through the kernel.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let _moved = p.add_move(body, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "move"), "`transform_body` must be called: {:?}", k.calls.borrow());
}

#[test]
fn rollback_suppresses_later_features() {
    // The rollback bar suppresses the last nodes, so their bodies are neither built nor shown.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(body).is_some(), "the body must be built before the rollback");
    let n = p.timeline.len();
    p.set_rollback(Some(n - 1)); // Do not build the last node, the extrude.
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(body).is_none(), "after the rollback the body must be suppressed");
}

#[test]
fn mirror_by_datum_plane_uses_plane_origin_normal() {
    // A mirror about a datum plane calls `mirror_plane` with the origin and normal of that plane rather than
    // of a world one.
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let pl = p.add_plane(WorkPlane { id: 0, name: "d".into(), origin: [10.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], rot_deg: 0.0, tilt: Default::default(), def: PlaneDef::Manual });
    let mir = p.add_mirror(base, 0, false, 0);
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == mir) {
        if let qymcad_core::feature::FeatureKind::Mirror { datum, .. } = &mut n.kind {
            *datum = pl;
        }
        n.dirty = true;
    }
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("mirror_plane") && c.contains("[10.0, 0.0, 0.0]")), "the mirror must use the origin of the datum plane: {:?}", k.calls.borrow());
    assert!(p.mesh_index(mir).is_some(), "the mirrored body must be built");
}

#[test]
fn linear_array_two_directions_grid() {
    // A linear pattern with two directions gives a grid of count by count2 copies.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array(base, 25.0, 0.0, 0.0, 3); // Three along X; by default there is no second direction.
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == la) {
        if let qymcad_core::feature::FeatureKind::LinearArray { dy2, count2, .. } = &mut n.kind {
            *dy2 = 25.0;
            *count2 = 2; // Plus two along Y, giving a 3 by 2 grid of 6.
        }
        n.dirty = true;
    }
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")), "a 3 by 2 grid must give 6 copies: {:?}", k.calls.borrow());
    assert!(p.mesh_index(la).is_some(), "the pattern body must be built");
}

#[test]
fn linear_array_grid_helper_builds_two_directions() {
    // `add_linear_array_grid` is the single core method the interface calls: a count by count2 grid in one
    // call.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array_grid(base, qymcad_core::model::ArrayAxis { d: [20.0, 0.0, 0.0], count: 3 }, qymcad_core::model::ArrayAxis { d: [0.0, 15.0, 0.0], count: 2 }); // 3×2 = 6
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")), "3×2=6: {:?}", k.calls.borrow());
    assert!(p.mesh_index(la).is_some());
}

#[test]
fn linear_array_grid3_builds_full_3d_grid() {
    // Three independent directions along X, Y and Z give a full 3D grid of count by count2 by count3. Both the
    // number of copies and the position of the far instance at the corner of the box (i, j, k at their maxima)
    // are checked.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array_grid3(base, [ArrayAxis { d: [10.0, 0.0, 0.0], count: 2 }, ArrayAxis { d: [0.0, 20.0, 0.0], count: 3 }, ArrayAxis { d: [0.0, 0.0, 30.0], count: 2 }]); // 2×3×2 = 12
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    let pat = calls.iter().find(|c| c.starts_with("pattern n=12")).expect("2 by 3 by 2 gives 12 copies").clone();
    // The far corner: i = 1 (10 along X), j = 2 (40 along Y), k = 1 (30 along Z).
    assert!(pat.contains("(10.0,40.0,30.0)"), "corner of the 3D grid: {pat}");
    drop(calls);
    assert!(p.mesh_index(la).is_some());
}

#[test]
fn linear_array_step_is_parametric_via_feat_dim() {
    // The step lives as an expression on a vector component (`dx`), so a global parameter moves the pattern:
    // regenerate reads `feat_dims` rather than the stored number. The actual instance translations are
    // checked.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "gap".into(), expr: "30".into(), value: 30.0 });
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array(base, 10.0, 0.0, 0.0, 3); // The stored dx is 10...
    p.set_feat_dim(la, "dx", "gap".into()); // ...but the expression dx = gap = 30 overrides it.
    let k = MockKernel::default();
    p.regenerate(&k);
    // The instances land at 0, 30 and 60 along X rather than 0, 10 and 20: parametric.
    let calls = k.calls.borrow();
    let pat = calls.iter().find(|c| c.starts_with("pattern n=3")).expect("the pattern must be built").clone();
    drop(calls);
    assert!(pat.contains("(0.0,0.0,0.0)") && pat.contains("(30.0,0.0,0.0)") && pat.contains("(60.0,0.0,0.0)"), "the step must be parametric (gap = 30): {pat}");
    // Changing the parameter moves the pattern without editing the feature.
    p.parameters[0].value = 50.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    let calls2 = k2.calls.borrow();
    let pat2 = calls2.iter().find(|c| c.starts_with("pattern n=3")).expect("it must be rebuilt");
    assert!(pat2.contains("(100.0,0.0,0.0)"), "with gap = 50 the far instance must sit at 100: {pat2}");
}

#[test]
fn circular_array_angle_is_parametric_via_feat_dim() {
    // The angle of a circular pattern is an expression in `feat_dims`. A rotation does not change the
    // translation, so what is checked is that regenerate reads the parameter (building the right number of
    // copies) and does not fail when the global parameter changes.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "ang".into(), expr: "180".into(), value: 180.0 });
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ca = p.add_circular_array_axis(base, 4, 90.0, 0); // The stored angle is 90...
    p.set_feat_dim(ca, "angle", "ang".into()); // ...and the expression ang = 180 overrides it.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=4")), "four copies from the angle parameter: {:?}", k.calls.borrow());
    assert!(p.mesh_index(ca).is_some());
    p.parameters[0].value = 270.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(ca).is_some(), "it must rebuild when the angle parameter changes");
}

#[test]
fn delete_datums_remove_from_vec_and_timeline() {
    // Datum deletions (plane, point, axis) are single project methods: they remove the entry from its vector
    // and the timeline node together.
    use qymcad_core::model::{DatumAxis, DatumPoint, WorkPlane};
    let mut p = part_project();
    let pl = p.add_plane(WorkPlane::default());
    assert!(p.planes.iter().any(|x| x.id == pl) && p.timeline.iter().any(|n| n.id == pl), "the plane must exist in both the vector and the timeline");
    assert!(p.delete_plane(pl));
    assert!(!p.planes.iter().any(|x| x.id == pl) && !p.timeline.iter().any(|n| n.id == pl), "the plane must be removed from both the vector and the timeline");
    let dp = p.add_datum_point(DatumPoint::default());
    assert!(p.delete_datum_point(dp));
    assert!(!p.datum_points.iter().any(|x| x.id == dp) && !p.timeline.iter().any(|n| n.id == dp), "the point must be removed from both the vector and the timeline");
    let ax = p.add_datum_axis(DatumAxis::default());
    assert!(p.delete_datum_axis(ax));
    assert!(!p.datum_axes.iter().any(|x| x.id == ax) && !p.timeline.iter().any(|n| n.id == ax), "the axis must be removed from both the vector and the timeline");
    // Deleting again returns false: there is no such entry.
    assert!(!p.delete_plane(pl) && !p.delete_datum_point(dp) && !p.delete_datum_axis(ax));
}

#[test]
fn delete_base_cascades_to_move() {
    // Deleting a base body (a primitive) cascades into the move that consumes it, leaving no ghosts.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 50.0; // Offset along X.
    let mv = p.add_move(base, mat);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(base).is_some() && p.mesh_index(mv).is_some(), "both bodies must be built");
    let removed = p.delete_body_cascade(base);
    assert!(removed.contains(&base) && removed.contains(&mv), "the cascade must remove both the base and the move: {removed:?}");
    assert!(p.timeline.iter().all(|n| n.kind.body() != Some(mv)), "the move node must be removed by the cascade");
    assert!(p.mesh_index(mv).is_none() && p.mesh_index(base).is_none(), "both meshes must be deleted");
}

#[test]
fn prune_removes_the_orphan_mesh_and_leaves_its_consumer_red() {
    // The base node is deleted alone, leaving an orphaned mesh and a move standing on it. The mesh goes; the move
    // stays in the timeline, red with the reason, for the person to repair or delete.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 50.0;
    let mv = p.add_move(base, mat);
    let k = MockKernel::default();
    p.regenerate(&k);
    p.timeline.retain(|n| n.kind.body() != Some(base)); // Remove only the base node; the mesh is left orphaned.
    assert!(p.mesh_index(base).is_some(), "the base mesh must be left orphaned");
    let removed = p.prune_dangling();
    assert_eq!(removed, std::collections::HashSet::from([base]), "pruning removes the orphaned base and nothing else");
    assert!(p.timeline.iter().any(|n| n.kind.body() == Some(mv)), "the move went with its base");
    p.regenerate(&k);
    assert_eq!(p.regen_errors.get(&mv), Some(&qymcad_core::errors::CoreError::SourceBodyDeleted), "the move stands on nothing and says nothing");
}

#[test]
fn prune_keeps_imported_body() {
    // An imported body (no node, but listed in `imported_bodies`) is not an orphan: pruning leaves it and the
    // feature built on it alone.
    let mut p = part_project();
    let imp = p.add_mesh(qymcad_core::geom::Mesh::default());
    p.imported_bodies.insert(imp);
    let _fil = p.add_fillet(imp, 1.0, Vec::<u32>::new());
    let removed = p.prune_dangling();
    assert!(removed.is_empty(), "the import and the feature on it must be left alone: {removed:?}");
    assert!(p.mesh_index(imp).is_some(), "the imported mesh must stay");
}

#[test]
fn body_boolean_builds_and_consumes_both_operands() {
    // A body-to-body boolean builds the result and consumes both operands, hiding a and b from view.
    let mut p = part_project();
    let s1 = square(&mut p, "square 1");
    p.add_sketch_node(s1, "square 1");
    let a = p.add_extrude_on(s1, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let s2 = square(&mut p, "square 2");
    p.add_sketch_node(s2, "square 2");
    let b = p.add_extrude_on(s2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let res = p.add_body_boolean(a, b, 0); // The cut a minus b.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(a).is_some() && p.mesh_index(b).is_some() && p.mesh_index(res).is_some(), "both operands and the result must be built by the kernel");
    assert!(k.calls.borrow().iter().any(|c| c == "body_boolean"), "the kernel must call `body_boolean`: {:?}", k.calls.borrow());
    // Both operands are consumed and only the result stays visible: `consumed()` of the boolean node is
    // [a, b].
    let node = p.timeline.iter().find(|n| n.id == res).unwrap();
    let consumed = node.kind.consumed();
    assert!(consumed.contains(&a) && consumed.contains(&b), "`consumed()` of the boolean must be [a, b]: {consumed:?}");
}
