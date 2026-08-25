//! MATRIX: a sketch ON A FACE of a body, and the feature follows that face when the source is
//! rebuilt (top-down within a part). The classic class of breakages where a step rebuilds in the
//! wrong place. The position of the pocket is measured with a BOOLEAN PROBE (an intersection with a
//! control column near the top), because the volume is the same either way, snapshot or not.

use qymcad_core::feature::{FaceKey, SketchPlane};
use qymcad_core::model::Project;

/// A 10x10x5 pocket in the centre of the TOP face of a 20x20xh box. Returns (project, box body, cut
/// body, sid of the pocket sketch).
fn build(h: f64) -> (Project, u64, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("base");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "base");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], h, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let cube = p.finish_base_body(e, 1);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the top face, and a sketch on it
    let top = p.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).cloned().expect("the top face");
    let key = FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id };
    let s2 = p.new_sketch("pocket");
    let sid2 = p.sketches[s2].id;
    p.sketches[s2].plane = SketchPlane::Face(cube, key);
    p.add_sketch_node(sid2, "pocket");
    // the 2D origin of a sketch on an axis-aligned face is the PROJECTION OF THE WORLD ORIGIN (a
    // corner) with world axes (anti-mirror, world_aligned), so the centre of the top face of a
    // 0..20 box is (10,10) in sketch coordinates.
    p.add_rect_entity(s2, 5.0, 5.0, 15.0, 15.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s2);
    let cid2 = p.sketches[s2].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    // a cut 5 deep INTO the body (flip=true: against the face normal, the interface's smart default)
    let cut = p.add_combine_multi_op(cube, sid2, vec![cid2], 5.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, 0.0, vec![]);
    (p, cube, cut, sid2)
}

/// Material in a 10x10 control column at the TOP of the body [ztop-5 .. ztop]: a pocket there gives 0,
/// solid material gives 500.
fn probe_top(shape: &qymcad_kernel::Shape, ztop: f64) -> f64 {
    let outer = qymcad_core::geom::Contour::closed(vec![
        qymcad_core::geom::Point2::new(-5.0, -5.0),
        qymcad_core::geom::Point2::new(5.0, -5.0),
        qymcad_core::geom::Point2::new(5.0, 5.0),
        qymcad_core::geom::Point2::new(-5.0, 5.0),
    ]);
    let prof = qymcad_core::geom::encode_profile(&outer, &[]);
    let probe = qymcad_kernel::Shape::extrude_profile(&prof, 5.0).expect("the probe");
    let mut place = qymcad_core::feature::PLACE_IDENTITY;
    place[3] = 10.0; // the centre of the box (the box is 0..20)
    place[7] = 10.0;
    place[11] = ztop - 5.0;
    let probe = probe.transformed(&place).expect("moving the probe");
    shape.boolean(&probe, 2).map(|s| s.volume()).unwrap_or(-1.0)
}

#[test]
fn pocket_follows_face_on_height_change() {
    let mut fails: Vec<String> = Vec::new();
    let (mut p, _cube, cut, _sid2) = build(20.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &report.errors {
        fails.push(format!("before the edit: ERROR {id}: {e}"));
    }
    let v = shapes.get(&cut).map(|s| s.volume()).unwrap_or(0.0);
    if (v - 7500.0).abs() > 75.0 {
        fails.push(format!("before the edit: V={v:.0}, expected 7500"));
    }
    if let Some(s) = shapes.get(&cut) {
        let m = probe_top(s, 20.0);
        if m.abs() > 5.0 {
            fails.push(format!("before the edit: the control column at the top holds {m:.0} of material (the pocket should be there)"));
        }
    }
    // THE EDIT: the box height 20 -> 30. The top face moves to z=30 — the pocket MUST move with it.
    let enode = p.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })).unwrap();
    if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut p.timeline[enode].kind {
        *height = 30.0;
    }
    p.timeline[enode].dirty = true;
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id, e) in &report.errors {
        fails.push(format!("after the edit: ERROR {id}: {e}"));
    }
    let v = shapes.get(&cut).map(|s| s.volume()).unwrap_or(0.0);
    if (v - 11500.0).abs() > 115.0 {
        fails.push(format!("after the edit: V={v:.0}, expected 11500 (box 12000 minus pocket 500)"));
    }
    if let Some(s) = shapes.get(&cut) {
        let m = probe_top(s, 30.0); // the column at the NEW top: the pocket moved, so it is empty
        if m.abs() > 5.0 {
            fails.push(format!("after the edit: the pocket did NOT follow the face — the new top holds {m:.0} of material (a snapshot sketch?)"));
        }
        let m_old = probe_top(s, 25.0); // where the pocket would be under a snapshot (z 15..20, column 20..25): must be SOLID
        if (m_old - 500.0).abs() > 25.0 {
            fails.push(format!("after the edit: the middle of the body (column 20..25) is {m_old:.0}, expected solid 500 — the pocket is stuck in its old place"));
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
