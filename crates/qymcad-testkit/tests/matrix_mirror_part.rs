//! MIRRORED COPY of a part or a subassembly across a WORLD plane (a click on XY/XZ/YZ, on a datum or
//! on a FACE — the interface turns all of them into a world origin/normal). The two-armed robot is
//! the canonical case.
//!
//! Semantics: the SHAPE is associative (editing the source rebuilds the mirror) while the PLACEMENT
//! is free — the copy is dragged by its gizmo, and a rebuild neither touches nor resets that
//! transform.

use qymcad_core::model::Project;

fn probe_box(s: &qymcad_kernel::Shape, wt: [f64; 12], x0: f64, x1: f64) -> f64 {
    let outer = qymcad_core::geom::Contour::closed(vec![
        qymcad_core::geom::Point2::new(x0, -5.0),
        qymcad_core::geom::Point2::new(x1, -5.0),
        qymcad_core::geom::Point2::new(x1, 25.0),
        qymcad_core::geom::Point2::new(x0, 25.0),
    ]);
    let prof = qymcad_core::geom::encode_profile(&outer, &[]);
    let bx = qymcad_kernel::Shape::extrude_profile(&prof, 40.0).unwrap();
    let mut pl = qymcad_core::feature::PLACE_IDENTITY;
    pl[11] = -5.0;
    let bx = bx.transformed(&pl).unwrap();
    // the body is in LOCAL space — bring it into the world for the probe
    let sw = s.transformed(&wt).unwrap();
    sw.boolean(&bx, 2).map(|c| c.volume()).unwrap_or(0.0)
}

fn mk_arm(p: &mut Project) -> (u64, u64) {
    // the "arm": a 20^3 box, with the part shifted +40 along X
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
    let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let arm = p.finish_base_body(e, 1);
    let arm_comp = p.body_owner(arm).expect("owner of the arm");
    let mut t = qymcad_core::feature::PLACE_IDENTITY;
    t[3] = 40.0;
    p.set_component_transform(arm_comp, t);
    (arm, arm_comp)
}

#[test]
fn mirror_part_reflects_follows_shape_and_drags_free() {
    let mut p = Project::default();
    p.new_document();
    let (_arm, arm_comp) = mk_arm(&mut p);
    // mirror across world YZ (x -> -x)
    let mbody = p.add_mirror_part(arm_comp, [0.0; 3], [1.0, 0.0, 0.0]);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    let ms = shapes.get(&mbody).expect("the mirror body");
    assert!((ms.volume() - 8000.0).abs() < 80.0, "the mirror volume equals the box");
    // world position: source x in [40..60] -> mirror x in [-60..-40]
    let mcomp = p.body_owner(mbody).expect("owner of the mirror");
    let mwt = p.world_transform(mcomp);
    let v_neg = probe_box(ms, mwt, -62.0, -38.0);
    let v_pos = probe_box(ms, mwt, 38.0, 62.0);
    assert!((v_neg - 8000.0).abs() < 80.0, "the mirror sits at x in [-60..-40]: {v_neg:.0}");
    assert!(v_pos < 1.0, "there is no mirror on the source side: {v_pos:.0}");
    // 1) THE SHAPE is associative: source height 20 -> 30 rebuilds the mirror
    if let Some(n) = p.timeline.iter_mut().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })) {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 30.0;
        }
        n.dirty = true;
    }
    let (r2, s2) = qymcad_testkit::regenerate(&mut p);
    assert!(r2.errors.is_empty(), "{:?}", r2.errors);
    let v2 = s2.get(&mbody).map(|s| s.volume()).unwrap_or(0.0);
    assert!((v2 - 12000.0).abs() < 120.0, "the mirror followed the shape edit: V={v2:.0}");
    // 2) THE PLACEMENT is free: the mirror is dragged by its gizmo to x=+100 and a rebuild does not reset it
    let mut drag = qymcad_core::feature::PLACE_IDENTITY;
    drag[3] = 100.0;
    p.set_component_transform(mcomp, drag);
    // moving the SOURCE does not touch the mirror (the plane is fixed in the source's local space)
    let mut t2 = qymcad_core::feature::PLACE_IDENTITY;
    t2[3] = 80.0;
    p.set_component_transform(arm_comp, t2);
    // editing the source shape: height 30 -> 25 rebuilds the body while the mirror keeps the dragged transform
    if let Some(n) = p.timeline.iter_mut().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })) {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 25.0;
        }
        n.dirty = true;
    }
    let (r3, s3) = qymcad_testkit::regenerate(&mut p);
    assert!(r3.errors.is_empty(), "{:?}", r3.errors);
    let mwt3 = p.world_transform(mcomp);
    assert!((mwt3[3] - 100.0).abs() < 1e-9, "the rebuild did NOT reset the dragged transform of the mirror: x={}", mwt3[3]);
    let ms3 = s3.get(&mbody).unwrap();
    // the local geometry of the mirror goes through the LOCAL ORIGIN of the copy (not through the
    // shifted point), so the source's local x in [0..20] reflects into x in [-20..0]; plus the drag
    // (+100) that is world [80..100]; and the shape is the new one (V=10000)
    let v_dragged = probe_box(ms3, mwt3, 78.0, 102.0);
    assert!((v_dragged - 10000.0).abs() < 100.0, "the copy is where it was dragged, with the NEW shape: {v_dragged:.0}");
}

/// MIRRORING A SUBASSEMBLY. The arm is a SUBASSEMBLY of two parts (shoulder plus a shifted hand), and
/// the assembly itself is shifted. A mirror across YZ gives a mirrored SUBASSEMBLY with both parts in
/// their correct reflected places; dragging the mirrored subassembly is free and a rebuild does not
/// reset it.
#[test]
fn mirror_subassembly_reflects_all_parts() {
    let mut p = Project::default();
    p.new_document();
    let root_saved = p.active_component;
    // the "Arm" subassembly
    let arm_asm = p.add_assembly("Arm");
    p.set_active_component(Some(arm_asm));
    let mk_cube = |p: &mut Project, name: &str| -> u64 {
        let si = p.new_sketch(name);
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, name);
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        p.finish_base_body(e, 1)
    };
    // each part is created as a separate Part inside the assembly
    let shoulder_part = p.add_part("Shoulder");
    p.set_active_component(Some(shoulder_part));
    let shoulder = mk_cube(&mut p, "s1");
    p.set_active_component(Some(arm_asm));
    let hand_part = p.add_part("Hand");
    p.set_active_component(Some(hand_part));
    let hand = mk_cube(&mut p, "s2");
    // the hand is locally shifted within the assembly to x=30
    let mut th = qymcad_core::feature::PLACE_IDENTITY;
    th[3] = 30.0;
    p.set_component_transform(hand_part, th);
    // the arm itself is shifted to x=50
    let mut ta = qymcad_core::feature::PLACE_IDENTITY;
    ta[3] = 50.0;
    p.set_component_transform(arm_asm, ta);
    p.active_component = root_saved;
    // mirror the SUBASSEMBLY across world YZ
    let mirrors = p.add_mirror_component(arm_asm, [0.0; 3], [1.0, 0.0, 0.0]);
    assert_eq!(mirrors.len(), 2, "one mirror per part of the subtree: {mirrors:?}");
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    // world positions: shoulder x in [50..70] -> mirror [-70..-50]; hand x in [80..100] -> [-100..-80]
    let expect = [(-72.0, -48.0), (-102.0, -78.0)];
    let mut found = [false; 2];
    for &mb in &mirrors {
        let ms = shapes.get(&mb).expect("the mirror body");
        let mcomp = p.body_owner(mb).unwrap();
        let wt = p.world_transform(mcomp);
        for (k, &(x0, x1)) in expect.iter().enumerate() {
            if (probe_box(ms, wt, x0, x1) - 8000.0).abs() < 80.0 {
                found[k] = true;
            }
        }
    }
    assert!(found[0] && found[1], "both parts are reflected into their own places: {found:?}");
    let _ = (shoulder, hand);
    // dragging the mirrored SUBASSEMBLY (the parent of the mirrored parts) by +10 along X is free,
    // and a rebuild after a source shape edit does not reset the transform
    let masm = p.components.iter().find(|c| c.id == p.body_owner(mirrors[0]).unwrap()).and_then(|c| c.parent).expect("the mirrored subassembly");
    let mut td = p.component_transform(masm);
    td[3] += 10.0;
    p.set_component_transform(masm, td);
    if let Some(n) = p.timeline.iter_mut().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. }) && n.parent == Some(shoulder_part)) {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 30.0;
        }
        n.dirty = true;
    }
    let (r2, s2) = qymcad_testkit::regenerate(&mut p);
    assert!(r2.errors.is_empty(), "{:?}", r2.errors);
    // the mirrored shoulder was at x in [-70..-50]; after dragging the subassembly it is [-60..-40], shape V=12000
    let mut dragged = false;
    for &mb in &mirrors {
        let ms = s2.get(&mb).unwrap();
        let wt = p.world_transform(p.body_owner(mb).unwrap());
        if (probe_box(ms, wt, -62.0, -38.0) - 12000.0).abs() < 120.0 {
            dragged = true;
        }
    }
    assert!(dragged, "the mirrored subassembly stayed where it was dragged and was NOT reset by the rebuild (shoulder -> x in [-60..-40], V=12000)");
}

/// THE COORDINATE SYSTEMS INSIDE A MIRRORED SUBASSEMBLY ARE NOT MANGLED. The world positions of the
/// parts are one thing (already covered by mirror_subassembly_reflects_all_parts), but each part used
/// to be reflected INDEPENDENTLY from its ABSOLUTE position while the mirrored subassembly itself
/// never got a placement of its own — it stayed at identity. Because of that the LOCAL transform of a
/// part relative to the new mirrored subassembly turned into a large unrelated number (for an arm
/// shifted to x=50 the hand offset flew off to -80 instead of a meaningful -30). The subassembly is
/// shifted here ON PURPOSE (x=50, as in mirror_subassembly_reflects_all_parts) — otherwise the defect
/// would produce the same numbers as the fix and the test would tell nothing apart. What is checked is
/// the STRUCTURE: the shoulder (offset 0 in the assembly) keeps a zero local offset in the mirror too,
/// and the hand (offset +30) gets a local offset of EXACTLY -30 (the offset mirrored within the
/// subassembly's LOCAL space — it is still on the other side of the shoulder, only reflected).
#[test]
fn mirror_subassembly_preserves_internal_local_offsets_mirrored() {
    let mut p = Project::default();
    p.new_document();
    let root_saved = p.active_component;
    let arm_asm = p.add_assembly("Arm");
    p.set_active_component(Some(arm_asm));
    let mk_cube = |p: &mut Project, name: &str| -> u64 {
        let si = p.new_sketch(name);
        let sid = p.sketches[si].id;
        p.add_sketch_node(sid, name);
        p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        p.regen_sketch(si);
        let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).unwrap();
        let e = p.add_extrude_multi(sid, vec![cid], 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
        p.finish_base_body(e, 1)
    };
    let shoulder_part = p.add_part("Shoulder");
    p.set_active_component(Some(shoulder_part));
    let _shoulder = mk_cube(&mut p, "s1");
    p.set_active_component(Some(arm_asm));
    let hand_part = p.add_part("Hand");
    p.set_active_component(Some(hand_part));
    let _hand = mk_cube(&mut p, "s2");
    let mut th = qymcad_core::feature::PLACE_IDENTITY;
    th[3] = 30.0; // the hand is locally offset from the shoulder by +30 along X
    p.set_component_transform(hand_part, th);
    // the subassembly itself is shifted ON PURPOSE — otherwise the defect (an assembly with no
    // placement of its own) would coincide with the fix
    let mut ta = qymcad_core::feature::PLACE_IDENTITY;
    ta[3] = 50.0;
    p.set_component_transform(arm_asm, ta);
    p.active_component = root_saved;
    let mirrors = p.add_mirror_component(arm_asm, [0.0; 3], [1.0, 0.0, 0.0]);
    assert_eq!(mirrors.len(), 2);
    let masm = p.components.iter().find(|c| c.id == p.body_owner(mirrors[0]).unwrap()).and_then(|c| c.parent).expect("the mirrored subassembly");
    for &mb in &mirrors {
        let mcomp = p.body_owner(mb).unwrap();
        // the name of a mirrored copy is stored in the document as a CODE with the source name as its
        // argument: `name-mirror-of#Shoulder`. The wording is chosen by the window; what matters here
        // is WHOSE mirror it is.
        let stored = p.components.iter().find(|c| c.id == mcomp).map(|c| c.name.clone()).unwrap_or_default();
        assert!(stored.starts_with("name-mirror-of#"), "a mirror must be named by a code rather than by words of one language: {stored}");
        let owner_name = stored.split_once('#').map(|(_, v)| v.to_string()).unwrap_or(stored);
        let local = p.relative_transform(mcomp, masm); // the local transform of the part RELATIVE TO THE NEW subassembly
        let x = local[3];
        if owner_name.starts_with("Shoulder") {
            assert!(x.abs() < 1e-6, "the local offset of the shoulder inside the mirrored subassembly is 0, as in the original: x={x}");
        } else if owner_name.starts_with("Hand") {
            assert!((x - (-30.0)).abs() < 1e-6, "the local offset of the hand inside the mirrored subassembly is -30 (the mirror of the original +30, NOT an unrelated number): x={x}");
        } else {
            panic!("unexpected owner name for a mirror: {owner_name}");
        }
        // the orientation (rotation) is NOT mirrored — same as in the original
        for k in [0usize, 1, 2, 4, 5, 6, 8, 9, 10] {
            let expect = if k == 0 || k == 5 || k == 10 { 1.0 } else { 0.0 };
            assert!((local[k] - expect).abs() < 1e-9, "the rotation of the part inside the subassembly is untouched (index {k}): {}", local[k]);
        }
    }
}

/// MIRRORING ACROSS AN ARBITRARY WORLD PLANE (a datum of a shifted part, or a face — the interface
/// resolves both into world coordinates). The plane x=40 (a face of the source box): the box at
/// x in [40..60] is reflected "in place" into x in [20..40], not through raw local space (x=0, which
/// would give [-60..-40]).
#[test]
fn mirror_part_across_arbitrary_plane() {
    let mut p = Project::default();
    p.new_document();
    let (_arm, arm_comp) = mk_arm(&mut p);
    // the world plane x=40 — as the interface hands it over after a click on a face or a datum of a shifted part
    let mbody = p.add_mirror_part(arm_comp, [40.0, 10.0, 10.0], [1.0, 0.0, 0.0]);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    let ms = shapes.get(&mbody).expect("the mirror body");
    assert!((ms.volume() - 8000.0).abs() < 80.0, "the mirror volume equals the box: {:.0}", ms.volume());
    let mwt = p.world_transform(p.body_owner(mbody).unwrap());
    let v_at = probe_box(ms, mwt, 18.0, 42.0);
    let v_wrong = probe_box(ms, mwt, -62.0, -38.0); // where RAW local space (world x=0) would have reflected it
    assert!((v_at - 8000.0).abs() < 80.0, "the mirror sits at the world position of the plane, x in [20..40]: {v_at:.0}");
    assert!(v_wrong < 1.0, "the mirror is NOT at the raw local position (x in [-60..-40]): {v_wrong:.0}");
}

/// THE GIZMO OF A COPY SITS AT THE REFLECTED ORIGIN, NOT AT THE SOURCE.
///
/// The gizmo of a copy — the world position of its LOCAL ORIGIN, the point the gizmo lands on — must
/// end up at the reflection of the source's local origin across the plane, rather than staying with
/// the source. The plane x=10 deliberately does NOT pass through the source (x=40), so that "the
/// source" and "the reflected point" are different places: the old defect left the gizmo at exactly
/// x=40, the world origin of the source, with no reflection at all.
#[test]
fn mirror_part_gizmo_sits_at_reflected_origin_not_source() {
    let mut p = Project::default();
    p.new_document();
    let (_arm, arm_comp) = mk_arm(&mut p); // the source: world origin at x=40
    let mbody = p.add_mirror_part(arm_comp, [10.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let mcomp = p.body_owner(mbody).expect("owner of the mirror");
    let mwt = p.world_transform(mcomp);
    // the expected reflected point: reflect(40, plane x=10) = 2*10-40 = -20 (the OLD defect gave 40)
    assert!((mwt[3] - (-20.0)).abs() < 1e-6, "the gizmo of the copy is at the reflected point x=-20 (the defect left it at x=40 with the source): x={}", mwt[3]);
    assert!(mwt[7].abs() < 1e-9 && mwt[11].abs() < 1e-9, "y/z of the copy origin did not slide: y={} z={}", mwt[7], mwt[11]);
    // the orientation of the copy's frame matches the source (it is not mirrored — the local frame stays as the part's)
    let swt = p.world_transform(arm_comp);
    for k in [0usize, 1, 2, 4, 5, 6, 8, 9, 10] {
        assert!((mwt[k] - swt[k]).abs() < 1e-9, "the rotation of the copy frame equals that of the source (index {k}): {} vs {}", mwt[k], swt[k]);
    }
    // and the geometry itself is where it should be (the world position does not depend on that fix)
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    let ms = shapes.get(&mbody).unwrap();
    let v = probe_box(ms, mwt, -42.0, -18.0);
    assert!((v - 8000.0).abs() < 80.0, "the body is at the correct world position, x in [-40..-20]: {v:.0}");
}
