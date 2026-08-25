//! ASSOCIATIVITY MATRIX: a sketch edit rebuilds the features that stand on it. Without it the vast
//! majority of edits rebuild into the wrong shape, which is the whole point of a parametric model.
//! Covers dimension/geometry edits, adding and removing loops, suppression, rollback, and deleting
//! an operation. Real OCCT kernel; failures accumulate.

use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

fn check(fails: &mut Vec<String>, label: &str, got: f64, exp: f64, tol: f64) {
    if exp <= 0.0 || ((got - exp) / exp).abs() > tol {
        fails.push(format!("{label}: V={got:.1}, expected {exp:.1}"));
    }
}

fn regen_v(p: &mut Project, body: u64, fails: &mut Vec<String>, label: &str) -> f64 {
    let (report, shapes) = qymcad_testkit::regenerate(p);
    for (id, e) in &report.errors {
        fails.push(format!("{label}: ERROR {id}: {e}"));
    }
    shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0)
}

/// After a sketch edit the body MUST rebuild with the new geometry.
#[test]
fn matrix_sketch_edit_rebuild() {
    let mut fails = Vec::new();
    // 1) rectangle 30x30 -> extrude 10 -> stretch to 40x30 -> V 9000 becomes 12000
    {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let v0 = regen_v(&mut p, body, &mut fails, "stretch (before)");
        check(&mut fails, "stretch (before)", v0, 9000.0, 0.01);
        // the edit: right-hand points x 30 -> 40 (as a drag in the sketcher)
        for pt in &mut p.sketches[si].points {
            if (pt.x - 30.0).abs() < 1e-9 {
                pt.x = 40.0;
            }
        }
        p.solve_sketch(si);
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        let v1 = regen_v(&mut p, body, &mut fails, "stretch (after)");
        check(&mut fails, "stretch (after)", v1, 12000.0, 0.01);
    }
    // 2) ring R20/R10: extrude it, then change the outer radius 20 -> 25, so the volume goes from
    //    pi*(400-100)*10 to pi*(625-100)*10
    {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        let outer = p.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        // the outer region (largest area) — with the hole left by the inner circle
        let cid = {
            let mut cc: Vec<(u64, f64)> = p.sketches[si].contour_ids.iter().copied().filter_map(|c| Some((c, p.contours[p.contour_index(c)?].area()))).collect();
            cc.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            cc[0].0
        };
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let v0 = regen_v(&mut p, body, &mut fails, "ring (before)");
        check(&mut fails, "ring (before)", v0, PI * 300.0 * 10.0, 0.01);
        // edit the outer circle radius through the entity
        if let Some(ent) = p.sketches[si].entities.iter_mut().find(|e| e.id == outer) {
            if let qymcad_core::model::EntityKind::Circle { r, .. } = &mut ent.kind {
                *r = 25.0;
            }
        }
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        let v1 = regen_v(&mut p, body, &mut fails, "ring (after r20->25)");
        check(&mut fails, "ring (after r20->25)", v1, PI * 525.0 * 10.0, 0.01);
    }
    // 3) ADDING a loop to the sketch must not break the extrude of an existing one (contour id is stable)
    {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let v0 = regen_v(&mut p, body, &mut fails, "id stability (before)");
        check(&mut fails, "id stability (before)", v0, 4000.0, 0.01);
        // add a NEW circle off to the side — the former contour must keep its id
        p.add_circle_entity(si, 60.0, 0.0, 8.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        let v1 = regen_v(&mut p, body, &mut fails, "id stability (after adding a loop)");
        check(&mut fails, "id stability (after adding a loop)", v1, 4000.0, 0.01);
    }
    // 4) DELETING the loop an operation references -> an HONEST node error, not silent garbage
    {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        let rect_ids = p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let body = p.finish_base_body(e, 1);
        let _ = regen_v(&mut p, body, &mut fails, "loop deletion (before)");
        p.delete_entities(si, &rect_ids);
        p.regen_sketch(si);
        p.mark_sketch_dirty(sid);
        let (report, _shapes) = qymcad_testkit::regenerate(&mut p);
        if report.errors.is_empty() {
            fails.push("loop deletion: an operation on a deleted contour raised NO error (silent garbage or a stale body)".into());
        }
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Suppressing and re-enabling a modifier: pass-through and back. Rollback. Deleting an operation.
#[test]
fn matrix_suppress_rollback_delete() {
    let mut fails = Vec::new();
    // box + a pocket cut; suppress the cut and the box is whole; re-enable and the cut returns
    {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("s");
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, "s");
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        let cube = p.finish_base_body(e, 1);
        // the pocket: a second 10x10 sketch, cut 5 deep
        let s2 = p.new_sketch("t");
        let sid2 = p.sketches[s2].id;
        p.add_sketch_node(sid2, "t");
        p.add_rect_entity(s2, 5.0, 5.0, 15.0, 15.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(s2);
        let cid2 = p.sketches[s2].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let cut = p.add_combine_multi_op(cube, sid2, vec![cid2], 5.0, 0, qymcad_core::feature::Extent::default(), 0.0, vec![]);
        let v0 = regen_v(&mut p, cut, &mut fails, "cut (before)");
        check(&mut fails, "cut (before)", v0, 8000.0 - 500.0, 0.01);
        // suppress the cut
        let ti = p.timeline.iter().position(|n| n.id == cut).unwrap();
        p.set_feature_suppressed(ti, true);
        let v1 = regen_v(&mut p, cut, &mut fails, "cut suppressed");
        check(&mut fails, "cut suppressed (box passes through)", v1, 8000.0, 0.01);
        // enable it again
        p.set_feature_suppressed(ti, false);
        let v2 = regen_v(&mut p, cut, &mut fails, "cut enabled");
        check(&mut fails, "cut enabled again", v2, 7500.0, 0.01);
        // rollback BEFORE the cut: only the box is built
        p.set_rollback(Some(ti));
        let (report, shapes) = qymcad_testkit::regenerate(&mut p);
        let _ = report;
        let vc = shapes.get(&cube).map(|s| s.volume()).unwrap_or(0.0);
        check(&mut fails, "rollback before the cut: box", vc, 8000.0, 0.01);
        if shapes.contains_key(&cut) && p.mesh_index(cut).is_some() {
            fails.push("rollback: the cut body is still visible (it must be hidden)".into());
        }
        p.set_rollback(None);
        let v3 = regen_v(&mut p, cut, &mut fails, "rollback cleared");
        check(&mut fails, "rollback cleared", v3, 7500.0, 0.01);
        // delete the cut operation entirely: the box remains
        let removed = p.delete_feature_op(cut);
        if !removed.contains(&cut) {
            fails.push(format!("delete_feature_op did not remove the cut (removed={removed:?})"));
        }
        let v4 = regen_v(&mut p, cube, &mut fails, "after deleting the cut");
        check(&mut fails, "after deleting the cut: box", v4, 8000.0, 0.01);
    }
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}

/// Editing a feature PARAMETER (extrude height) rebuilds the body.
#[test]
fn matrix_feature_param_edit() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    let v0 = regen_v(&mut p, body, &mut fails, "parameter (before)");
    check(&mut fails, "parameter (before)", v0, 4000.0, 0.01);
    // edit the height on the node directly (as the edit command does): 10 -> 25
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == e) {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 25.0;
        }
        n.dirty = true;
    }
    let v1 = regen_v(&mut p, body, &mut fails, "height 10->25");
    check(&mut fails, "height 10->25", v1, 10000.0, 0.01);
    assert!(fails.is_empty(), "\nFAILURES ({}):\n{}", fails.len(), fails.join("\n"));
}
