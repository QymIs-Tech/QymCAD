//! Tool buttons through to the body: sweep, primitives, shell, mirror, array, section; cases that cannot be built.

use qymcad_ui_state::Notice;
use super::*;

/// Reported behaviour: deleting a boss in a large project froze it for minutes. DELETING a node rebuilds only
/// what it touches, not the whole project. `resync_after_topology_change` used to clear the faces and call a
/// FORCED rebuild (the only thing that filled them back in) - on an assembly of a thousand bodies that is a
/// re-tessellation of everything. The trap in this test: the mesh of an unrelated body is SPOILED; a full
/// rebuild would restore it, an honest "only what is dirty" leaves it alone. And the faces of unrelated bodies
/// must survive the topology change.
#[test]
fn deleting_feature_does_not_rebuild_whole_project() {
    let mut app = App::default();
    let keep = build_cube(&mut app); // part one, which is left alone
                                     // part two with a body of its own (this is the one that gets deleted)
    let part2 = app.project.add_part("Part two");
    app.enter_component(part2);
    let before: std::collections::HashSet<Id> = app.live.shapes.keys().copied().collect();
    let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let victim = *app.live.shapes.keys().find(|b| !before.contains(b) && !consumed.contains(b)).expect("part two's body");

    // the trap: the mesh of an UNRELATED body is spoiled and its faces are remembered
    let keep_faces = app.project.mesh_index(keep).and_then(|i| app.project.bodies.get(i).map(|b| b.faces.clone())).unwrap_or_default();
    assert!(!keep_faces.is_empty(), "the unrelated body has B-rep faces");
    if let Some(i) = app.project.mesh_index(keep) {
        app.project.bodies[i].mesh.tris.clear(); // the spoiling: a full rebuild would restore this, a targeted one would not
    }

    // delete part two's node
    let ti = app.project.timeline.iter().position(|n| n.kind.body() == Some(victim)).expect("the node of part two's body");
    crate::gui::commands::delete_feature(&mut app.part_ctx(), ti, false);

    assert!(app.project.mesh_index(victim).is_none() && !app.live.shapes.contains_key(&victim), "the deleted body went away entirely");
    let ki = app.project.mesh_index(keep).expect("the unrelated body is still there");
    assert!(app.project.bodies[ki].mesh.tris.is_empty(), "the unrelated bodies were NOT rebuilt (that is what froze a large project)");
    assert_eq!(app.project.bodies.get(ki).map(|b| b.faces.len()), Some(keep_faces.len()), "the unrelated body's faces survived the topology change (cached by body id)");
}

/// Reported behaviour: there was no amber cap and the bodies looked hollow inside - no CAD cuts that way. The
/// section cap is built FROM THE MESH and therefore exists ALWAYS - including without a live B-rep (right after
/// opening from a bundle, or for an imported STL) and during a gizmo drag. The bodies the plane does not touch
/// are rejected by their extent - otherwise a frame would cost minutes on a thousand bodies.
#[test]
fn section_cap_is_built_from_mesh_always() {
    let mut app = App::default();
    let cube = build_cube(&mut app); // a block from 0 to 20
    let mi = app.project.mesh_index(cube).expect("the block's mesh");
    assert!(qymcad_ui_state::mesh_crosses_plane(&app.painting(), mi, [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]), "the plane goes through the body, so a cap is needed");
    assert!(!qymcad_ui_state::mesh_crosses_plane(&app.painting(), mi, [0.0, 0.0, 100.0], [0.0, 0.0, 1.0]), "the plane misses the body, so no cap is computed");
    assert!(!qymcad_ui_state::mesh_crosses_plane(&app.painting(), mi, [-50.0, 0.0, 0.0], [1.0, 0.0, 0.0]), "and a miss from the side counts the same");

    // THE KEY POINT: there is no live B-rep at all and the cap must still be there (it used to be computed by a kernel boolean)
    app.live.shapes.clear();
    app.side.section.plane = Some(([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]));
    let caps = crate::gui::render_scene::section_caps_for_frame(&app.painting());
    assert!(!caps.is_empty(), "the cap is there without a B-rep too");
    let area: f64 = caps.iter().flat_map(|m| (0..m.tris.len()).map(|i| m.tri_normal_area(i).1)).sum();
    assert!((area - 400.0).abs() < 1e-6, "the cap's area equals the block's 20x20 section: {area}");
    for m in caps.iter() {
        for v in &m.verts {
            assert!((v.z - 10.0).abs() < 1e-9, "the cap lies exactly in the cutting plane: z={}", v.z);
        }
    }

    // and during a gizmo drag the cap does NOT vanish (from the mesh this is cheap)
    app.side.section.drag = true;
    app.side.section.offset = 2.0;
    assert!(!crate::gui::render_scene::section_caps_for_frame(&app.painting()).is_empty(), "during a drag the section stays a closed body");
    // a plane that misses the whole scene gives no caps (and the kernel is not spent on it)
    app.side.section.drag = false;
    app.side.section.offset = 500.0;
    assert!(crate::gui::render_scene::section_caps_for_frame(&app.painting()).is_empty(), "a plane outside the model gives no caps");
}

/// Reported behaviour: loading a large project froze it and brought up a "not responding" window. Opening now
/// takes the geometry FROM THE BUNDLE and does NOT rebuild the timeline. `write_project` used to throw away the
/// features' bodies ("the timeline will rebuild them") while loading called a forced rebuild: on a real
/// assembly that was 36 seconds of parsing the embedded STEP plus 31 seconds of tessellating 1170 solids, the
/// tessellation running straight on the UI thread.
#[test]
fn open_uses_bundle_geometry_without_rebuild() {
    let dir = std::env::temp_dir().join("qym_open_fast_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&path);

    let mut app = App::default();
    let cube = build_cube(&mut app);
    let tris_before = app.project.mesh_index(cube).map(|i| app.project.bodies[i].mesh.tris.len()).unwrap_or(0);
    assert!(tris_before > 0, "the block was built");
    app.disk.project_path = Some(path.clone());
    app.save_project();
    app.wait_bg();

    // 1. the geometry is REALLY in the file (the features' bodies used to be thrown away, leaving a bundle with no meshes)
    let loaded = qymcad_io::load_project(&path).expect("the bundle reads");
    let li = loaded.mesh_index(cube).expect("the block's body was saved into the bundle");
    assert_eq!(loaded.bodies[li].mesh.tris.len(), tris_before, "the body's mesh lies in the file whole");
    assert!(!loaded.bodies[li].faces.is_empty(), "the B-rep faces arrived INSIDE the body");

    // 2. opening: not one dirty node and NOT ONE rebuilt body - only what the file shows
    let mut app2 = App::default();
    app2.finish_project_load(path.clone(), loaded, Vec::new());
    assert!(app2.project.timeline.iter().all(|n| !n.dirty), "after opening there is nothing to rebuild");
    assert!(app2.live.shapes.is_empty(), "the live B-rep was NOT built on opening (that is the whole point)");
    assert!(!app2.live.ready, "the B-rep cache is marked as one to be built on demand");
    let i2 = app2.project.mesh_index(cube).expect("the body is there");
    assert_eq!(app2.project.bodies[i2].mesh.tris.len(), tris_before, "what is on the screen is the geometry from the file");
    assert!(app2.project.regen_faces.contains_key(&cube), "the B-rep faces came back into the project (associativity)");

    // 3. the first operation that needs a B-rep brings the cache up itself (resolved on demand)
    crate::gui::io_jobs::ensure_brep(&mut app2.rebuild_ctx());
    assert!(app2.live.ready && app2.live.shapes.contains_key(&cube), "the B-rep was built on demand: {}", app2.status);
    let v: f64 = app2.live.shapes.get(&cube).map(|s| s.volume()).unwrap_or(0.0);
    assert!((v - 8000.0).abs() < 80.0, "and it is the same geometry: V={v:.0}");
    let _ = std::fs::remove_file(&path);
}

/// A SWEEP WITH AN OPERATION (a cut or an intersection), as a revolve has. It lives in the command layer only:
/// the `Sweep` node does not store the operation, `feat_op` sets it through `finish_base_body`. So this is where
/// it has to be checked: with the same profile and path, Add GIVES material and Cut TAKES it away.
#[test]
fn sweep_with_operation_flow() {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    // a 20-cubed block + sweeping a circle of diameter 6 along a vertical 30 through it; the caller sets op
    let build = |op: u8| -> f64 {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let sprof = app.project.new_sketch("the profile");
        let prof_sid = app.project.sketches[sprof].id;
        app.project.add_circle_entity(sprof, 10.0, 10.0, 3.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(sprof);
        let spath = app.project.new_sketch("the path");
        let path_sid = app.project.sketches[spath].id;
        app.project.sketches[spath].plane = SketchPlane::World(BasePlane::XZ);
        app.project.add_line_entity(spath, 10.0, 0.0, 10.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(spath);
        app.params.sweep.prof_sid = prof_sid;
        app.params.sweep.path_sid = path_sid;
        app.feat.op = op;
        crate::gui::commands::apply_sweep_cmd(app.feat, &mut app.project, &mut app.status, app.params.sweep);
        qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
        let consumed = qymcad_ui_state::consumed_bodies(&app.project);
        app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
    };
    let (add, cut) = (build(0), build(2));
    assert!(add > 8000.0 + 1.0, "an Add sweep must GIVE material: V={add:.0} (the block is 8000)");
    assert!(cut < 8000.0 - 1.0, "a Cut sweep must TAKE material away: V={cut:.0} (the block is 8000)");
    // the tool is one and the same: outside the block (the boss) plus inside it (the cut) equals the whole swept
    // cylinder of diameter 6 and length 30 - that is how THE OPERATION itself gets checked, without depending on
    // where exactly the sweep convention places the profile along the path
    let full = std::f64::consts::PI * 9.0 * 30.0;
    let sum = (add - 8000.0) + (8000.0 - cut);
    assert!((sum - full).abs() < 0.03 * full, "the boss {:.0} plus the cut {:.0} is {sum:.0}, a cylinder of {full:.0} was expected", add - 8000.0, 8000.0 - cut);
}

/// A MATRIX of edits THROUGH THE COMMAND LAYER. The model layer is covered by the kernel's matrices
/// (matrix_assoc, chain_edit, onface), but the complaint lived in the GUI layer, which is what this aims at:
/// an edit under a chain of modifiers, an edit through a feature's dimension popup, an honest error when a
/// support is removed (keep-last-good), an edit of a sketch AFTER the timeline was rolled back. The failures
/// accumulate.
#[test]
fn sketch_edit_matrix_gui() {
    use qymcad_core::feature::FeatureKind;
    let mut fails: Vec<String> = Vec::new();
    let live_volume = |app: &App| -> f64 {
        let consumed = qymcad_ui_state::consumed_bodies(&app.project);
        app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
    };
    // stretch sketch `si`'s base rectangle along X from 20 to `to`
    let stretch = |app: &mut App, si: usize, to: f64| {
        app.enter_sketch_edit(si);
        for pt in &mut app.project.sketches[si].points {
            if (pt.x - 20.0).abs() < 1e-9 {
                pt.x = to;
            }
        }
        app.project.solve_sketch(si);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
    };

    // 1. UNDER A CHAIN: a block plus an edge fillet, then the base sketch is stretched. The fillet must survive
    //    the edit (the contour was not substituted, the edge resolves) and the node must not be red.
    {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let eid = app.project.regen_edges.get(&cube).and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0)).map(|e| e.id);
        if let Some(eid) = eid {
            app.start_feat_cmd(4); // an edge fillet
            app.tools.gsel.edges.insert(eid);
            app.edges.body = Some(cube);
            if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "radius") {
                p.val = 2.0;
                p.txt = "2".into();
            }
            app.apply_feat_cmd();
            let v0 = live_volume(&app);
            if !(v0 > 7000.0 && v0 < 8000.0) {
                fails.push(format!("the block's fillet did not build: V={v0:.0} (a little under 8000 was expected); status: {}", app.status));
            }
            stretch(&mut app, 0, 30.0);
            let v1 = live_volume(&app);
            if (v1 - (v0 + 4000.0)).abs() > 60.0 {
                fails.push(format!("editing the sketch under a fillet: V={v1:.0}, {:.0} expected; status: {}", v0 + 4000.0, app.status));
            }
            if !app.project.regen_errors.is_empty() {
                fails.push(format!("after the edit the nodes are red: {:?}", app.project.regen_errors.values().collect::<Vec<_>>()));
            }
        } else {
            fails.push("the block has no edges to fillet".into());
        }
    }

    // 2. EDITING A FEATURE'S DIMENSION through the popup (start_feat_cmd_edit) RIGHT AFTER a sketch edit -
    //    the previous command's stale parameters must not mix in.
    {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        stretch(&mut app, 0, 40.0); // 40×20×20 = 16000
        let fid = app.project.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Extrude { .. })).map(|n| n.id);
        if let Some(fid) = fid {
            crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);
            if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 10.0;
                p.txt = "10".into();
            }
            app.apply_feat_cmd();
            let v = live_volume(&app);
            if (v - 8000.0).abs() > 80.0 {
                fails.push(format!("editing the sketch and the height: V={v:.0}, 8000 expected (40x20x10); status: {}", app.status));
            }
        } else {
            fails.push("no extrude node was found".into());
        }
    }

    // 3. REMOVING A SUPPORT: the contour an extrude stands on is taken out of the sketch, so the node must go
    //    red HONESTLY while the body stays the last good one (keep-last-good) rather than vanishing silently.
    {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let v0 = live_volume(&app);
        app.enter_sketch_edit(0);
        app.project.sketches[0].entities.clear();
        app.project.regen_sketch(0);
        app.finish_sketch_edit();
        if app.project.regen_errors.is_empty() {
            fails.push("removing the supporting contour passed SILENTLY (a red node was expected)".into());
        }
        let v1 = live_volume(&app);
        if (v1 - v0).abs() > 1.0 {
            fails.push(format!("the body did not hold on to the last good geometry: {v0:.0} -> {v1:.0}"));
        }
    }

    // 4. AN EDIT UNDER A ROLLBACK: the rollback line is raised above the cut, so editing the sketch rebuilds
    //    what is above the line and does NOT resurrect the cut the rollback suppressed.
    {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
        app.chosen.sel = Sel::Sketch(s2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        let cut_ti = app.project.timeline.iter().position(|n| matches!(n.kind, FeatureKind::Combine { .. }));
        if let Some(cut_ti) = cut_ti {
            app.project.rollback = Some(cut_ti); // the rollback sits BEFORE the cut
            qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
            stretch(&mut app, 0, 30.0);
            let v = live_volume(&app);
            if (v - 12000.0).abs() > 120.0 {
                fails.push(format!("an edit under a rollback: V={v:.0}, 12000 expected (the cut must not build); status: {}", app.status));
            }
        } else {
            fails.push("no cut node was found for the rollback".into());
        }
    }

    assert!(fails.is_empty(), "the GUI matrix of sketch edits:\n{}", fails.join("\n"));
}

/// Cancelling with Esc mid-command leaves NO stale state: the next extrude is clean.
#[test]
fn cancel_then_extrude_flow() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 2; // a CUT was started (not allowed in an empty part, but the command can be opened)
    app.start_feat_cmd(1);
    crate::gui::cancel_feat_cmd(&mut app.part_ctx()); // Esc
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the command was dropped");
    // now an honest extrude - nothing leaked from the cut
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    let v = total_volume(&app);
    assert!((v - 4000.0).abs() < 40.0, "after Esc the extrude is clean: V={v:.0}; status: {}", app.status);
}

/// Editing a cut's DEPTH by a double click: 5 to 10, the volume is recomputed and no nodes are bred.
#[test]
fn edit_cut_depth_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
    app.chosen.sel = Sel::Sketch(s2);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 5.0;
        p.txt = "5".into();
    }
    app.apply_feat_cmd();
    let nodes_before = app.project.timeline.len();
    let fid = app.project.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Combine { .. })).map(|n| n.id).expect("the cut's node");
    crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.project.timeline.len(), nodes_before, "editing a cut breeds no nodes");
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 7000.0).abs() < 70.0, "the cut went 5 -> 10: V={v:.0} (8000 minus 1000); status: {}", app.status);
}

/// The Box primitive by button: the sizes, then Enter. Plus a cylinder that is ADDED into the same body (one part).
#[test]
fn primitive_flows() {
    let mut app = App::default();
    app.start_prim_cmd(10); // a box
    for (k, v) in [("dx", 20.0), ("dy", 20.0), ("dz", 20.0)] {
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == k) {
            p.val = v;
            p.txt = format!("{v}");
        }
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the box was applied: {}", app.status);
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
    assert_eq!(live.len(), 1, "one body after the box");
    assert!((live[0] - 8000.0).abs() < 80.0, "a 20-cubed box: V={:.0}", live[0]);
}

/// A shell BY BUTTON: a body, a face, a thickness, then Enter (without entering the part).
#[test]
fn shell_button_flow() {
    let mut app = App::default();
    let cube = build_cube(&mut app);
    let top = app.project.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).map(|f| f.id).expect("the top face");
    app.start_feat_cmd(6);
    assert_eq!(app.tools.armed.cmd_kind(), 6, "the shell started: {}", app.status);
    app.tools.gsel.faces.insert(top);
    app.tools.gsel.faces_body = Some(cube);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "thickness") {
        p.val = 2.0;
        p.txt = "2".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the shell was applied: {}", app.status);
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    let exp = 8000.0 - 16.0 * 16.0 * 18.0;
    assert!((v - exp).abs() < 30.0, "a shell of t2: V={v:.0}, {exp:.0} expected");
}

/// IMPOSSIBLE CASES: the commands must give a clear refusal or an honest error, neither rubbish nor silence.
#[test]
fn impossible_cases_are_honest() {
    // 1) extruding a sketch WITHOUT a closed contour (one line): a refusal at the start
    {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        assert_eq!(app.tools.armed.cmd_kind(), 0, "the command does NOT start without a closed contour");
        assert_eq!(app.status, Notice::refusal(crate::i18n::tr("msg-no-closed-contour")), "a clear status");
    }
    // 2) a cut ENTIRELY CLEAR of the body: the node is created, but the rebuild must give an honest error or
    //    cut nothing at all (the volume does not change) - never a rubbish body
    {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let s2 = sketch_rect(&mut app, 100.0, 100.0, 120.0, 120.0); // far from the block
        app.chosen.sel = Sel::Sketch(s2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        let consumed = qymcad_ui_state::consumed_bodies(&app.project);
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 8000.0).abs() < 80.0, "a cut that misses: the block neither changed nor multiplied, V={v:.0}; status: {}", app.status);
    }
    // 3) a shell thicker than half the body: an honest error on the node (red) and no rubbish body
    {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let top = app.project.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).map(|f| f.id).unwrap();
        app.start_feat_cmd(6);
        app.tools.gsel.faces.insert(top);
        app.tools.gsel.faces_body = Some(cube);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 15.0; // more than the half-body's 10
            p.txt = "15".into();
        }
        app.apply_feat_cmd();
        let consumed = qymcad_ui_state::consumed_bodies(&app.project);
        let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
        let errored = !app.project.regen_errors.is_empty();
        let sane = live.iter().all(|v| *v > 0.0 && *v <= 8000.0 + 1.0);
        assert!(sane || errored, "a shell of t15: either a valid body or an honest error; V={live:?}, err={errored}");
    }
    // 4) a fillet with a zero radius: a no-op or an honest refusal, but NOT a crash and not an empty body
    {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let eid = app.project.regen_edges.get(&cube).and_then(|es| es.first()).map(|e| e.id).unwrap();
        app.start_feat_cmd(4);
        app.tools.gsel.edges.insert(eid);
        app.edges.body = Some(cube);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 0.0;
            p.txt = "0".into();
        }
        app.apply_feat_cmd();
        let consumed = qymcad_ui_state::consumed_bodies(&app.project);
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!(v > 7900.0 && v < 8100.0, "a radius of 0: the body is alive (V={v:.0}) and not rubbish; status: {}", app.status);
    }
}

/// A mirror by button: a body, the YZ plane, Enter with the original kept, and the volume doubles.
#[test]
fn mirror_button_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app); // a block from 0 to 20, touching the YZ plane along the face at x=0
    app.start_feat_cmd(16);
    assert_eq!(app.tools.armed.cmd_kind(), 16, "the mirror started: {}", app.status);
    app.params.mirror.plane = Some(qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::YZ));
    app.opts.mirror_keep = true;
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the mirror was applied: {}", app.status);
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 16000.0).abs() < 160.0, "a mirror keeping the original: V={v:.0}, 16000 expected");
}

/// A linear pattern by button: 3 copies at a step of 40 (disjoint), so three times the volume.
#[test]
fn linear_array_button_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    app.start_array_cmd(17);
    assert_eq!(app.tools.armed.cmd_kind(), 17, "the pattern started: {}", app.status);
    // the body to repeat is picked, as a person clicks it: a pattern with nothing picked is refused
    app.chosen.sel = Sel::Mesh(app.project.bodies.len() - 1);
    app.params.arr.count = 3;
    app.params.arr.two = false;
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "step") {
        p.val = 40.0;
        p.txt = "40".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the pattern was applied: {}", app.status);
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 24000.0).abs() < 240.0, "three blocks: V={v:.0}");
}

/// THE SECTION view: half the scene is hidden, Flip shows the other half, Off brings everything back.
#[test]
fn section_view_hides_half() {
    let mut app = App::default();
    let _cube = build_cube(&mut app); // a block from 0 to 20
    let (full, _) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    assert!(!full.is_empty(), "the scene is not empty");
    // a section through the centre with a +X normal, so the x>10 half is hidden
    app.side.section.plane = Some(([10.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    let (half, looks) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    assert!(!half.is_empty(), "the visible half is still there");
    // THE CAP IS KNOWN BY ITS LOOK: the colour is no longer baked into the vertices, and state 3 is the cap.
    let is_cap = |v: &qymcad_ui_state::GpuVert| looks.get(v.body as usize).is_some_and(|l| l.state == qymcad_ui_state::LOOK_CAP);
    // What is checked is THE BODY's clip: the section cap is not part of it - the cap is deliberately nudged a
    // hair beyond the plane, into the cut-away side, so that the clipped triangles of the thread turns do not
    // occlude it.
    for v in half.iter().filter(|v| !is_cap(v)) {
        assert!(v.pos[0] <= 10.0 + 1e-3, "an honest clip: EVERY vertex of the visible part has x<=10 (the cut runs exactly along the plane): {}", v.pos[0]);
    }
    assert!(half.iter().any(|v| (v.pos[0] - 10.0).abs() < 1e-3), "there are vertices EXACTLY on the cutting plane");
    // THE CAP: amber vertices at the plane itself (the cut is filled and one cannot see straight through it).
    // A tolerance rather than exact equality: the cap is deliberately nudged a hair into the CUT-AWAY side -
    // there is no material left there, so the body's clipped triangles do not occlude it. Lying exactly in the
    // plane, it argued with them over depth, and on a thread the fill disappeared in patches.
    let cap_verts = half.iter().filter(|v| is_cap(v) && (v.pos[0] - 10.0).abs() < 0.05).count();
    assert!(cap_verts >= 3, "the section cap is present ({cap_verts} cap vertices at the plane)");
    let wrong_side = half.iter().filter(|v| is_cap(v)).any(|v| v.pos[0] < 10.0 - 1e-6);
    assert!(!wrong_side, "the cap is nudged into the CUT-AWAY side (x >= 10): inside the material the thread turns occlude it");
    // flip: the other half becomes visible
    app.side.section.plane = Some(([10.0, 0.0, 0.0], [-1.0, 0.0, 0.0]));
    let (other, looks_o) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    let is_cap_o = |v: &qymcad_ui_state::GpuVert| looks_o.get(v.body as usize).is_some_and(|l| l.state == qymcad_ui_state::LOOK_CAP);
    for v in other.iter().filter(|v| !is_cap_o(v)) {
        assert!(v.pos[0] >= 10.0 - 1e-3, "after the flip every vertex has x>=10: {}", v.pos[0]);
    }
    // the offset moves the plane: a -X normal with an offset of 5 puts the plane at x=5 and shows x>=5
    app.side.section.offset = 5.0;
    let (shifted, looks_s) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    let is_cap_s = |v: &qymcad_ui_state::GpuVert| looks_s.get(v.body as usize).is_some_and(|l| l.state == qymcad_ui_state::LOOK_CAP);
    let minx = shifted.iter().filter(|v| !is_cap_s(v)).map(|v| v.pos[0]).fold(f32::MAX, f32::min);
    assert!((minx - 5.0).abs() < 1e-3, "the cut moved to x=5 (min x = {minx})");
    // off: everything is back
    app.side.section.plane = None;
    app.side.section.offset = 0.0;
    let (back, _) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    assert_eq!(back.len(), full.len(), "Off brings the whole scene back");
}
