//! Opening, saving, the live shapes behind them, undo and autosave.

use super::*;

/// Reported behaviour: every opening ran "Preparing B-rep", and afterwards the program asked to save although
/// nothing had been changed. Two faults at once: (1) `refresh_edges` is called EVERY FRAME in 3D and pulled
/// `ensure_brep` unconditionally, turning the lazy B-rep build into an eager one right after opening; (2)
/// building that cache raised `geom_rev`, which is part of the state key, so a clean project became dirty
/// without a single edit.
#[test]
fn opening_and_preparing_brep_keeps_project_clean() {
    let dir = std::env::temp_dir().join("qym_open_clean_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&path);
    let mut app = App::default();
    let cube = build_cube(&mut app);
    app.disk.project_path = Some(path.clone());
    app.save_project();
    app.wait_bg();

    // reopen it, the way the application does
    let loaded = qymcad_io::load_project(&path).expect("the bundle reads");
    let mut app2 = App::default();
    app2.finish_project_load(path.clone(), loaded, Vec::new());
    assert!(!qymcad_ui_state::is_dirty(&mut app2.rebuild_ctx()), "a project just opened is clean");

    // a frame of the 3D viewport: the edges refresh every frame, but the B-rep must NOT be built eagerly
    crate::gui::commands::refresh_edges(&mut app2.part_ctx());
    assert!(!app2.live.ready && app2.live.shapes.is_empty(), "the B-rep stays lazy: brep_ready={}, shapes={}", app2.live.ready, app2.live.shapes.len());
    assert!(!qymcad_ui_state::is_dirty(&mut app2.rebuild_ctx()), "looking at the model does not make the project dirty");

    // and now an operation really asks for the B-rep: it gets built while the project stays CLEAN
    crate::gui::io_jobs::ensure_brep(&mut app2.rebuild_ctx());
    assert!(app2.live.shapes.contains_key(&cube), "the B-rep was built on demand");
    assert!(!qymcad_ui_state::is_dirty(&mut app2.rebuild_ctx()), "rebuilding a cache is not a person's edit; there is nothing to ask to save");

    // and only a real edit makes the project dirty
    let si = sketch_rect(&mut app2, 40.0, 0.0, 60.0, 20.0);
    app2.chosen.sel = Sel::Sketch(si);
    app2.feat.op = 0;
    app2.start_feat_cmd(1);
    app2.apply_feat_cmd();
    assert!(qymcad_ui_state::is_dirty(&mut app2.rebuild_ctx()), "after a real edit the project is dirty");
    let _ = std::fs::remove_file(&path);
}

/// `wait_bg` used to wait on `recv()` WITHOUT a timeout. Closing the window during long background work
/// (restoring a B-rep from an 80 MB STEP takes 36 seconds) gave a dead-frozen window again, this time without
/// even a spinner. The wait now has a ceiling: a B-rep top-up is waited for only briefly (it can be repeated).
#[test]
fn wait_bg_gives_up_on_slow_background_job() {
    let mut app = App::default();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30)); // a "slow" worker
        let _ = tx.send(JobResult::ImportShapes { shapes: Vec::new(), regen: false });
    });
    app.regen.bg.push(Busy { started: std::time::Instant::now(), label: "a long top-up".into(), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
    let t = std::time::Instant::now();
    app.wait_bg();
    let waited = t.elapsed();
    assert!(waited < std::time::Duration::from_secs(3), "quitting must not hang on a background task: waited {waited:?}");
    assert!(app.status.words.contains(&crate::i18n::tr_prefix("io-not-waited", "what")), "it is said honestly that the task stayed in the background: {}", app.status);
    assert!(app.regen.bg.is_empty(), "the queue is cleared and the window will close");
}

/// An undo snapshot must not carry the BYTES of the embedded sources. They never change and weigh tens of
/// megabytes: 40 steps at 89 MB on a real assembly means gigabytes of memory and a ~90 MB memcpy on every
/// committed edit. The undo must nevertheless stay COMPLETE: after a rollback the bytes are there and the file
/// saves whole.
#[test]
fn undo_snapshot_does_not_carry_embedded_source_bytes() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let src_id = app.project.alloc_id();
    let payload = vec![7u8; 3_000_000]; // an "embedded STEP" of 3 MB
    app.project.sources.push(qymcad_core::model::SourceFile { id: src_id, name: "big.step".into(), ext: "step".into(), data: payload.clone() });

    let snap = crate::gui::snapshot(&app.project);
    let in_snap: usize = snap.project.sources.iter().map(|s| s.data.len()).sum();
    assert_eq!(in_snap, 0, "the snapshot holds NO source bytes (that would be a memcpy on every edit)");
    assert_eq!(snap.project.sources.len(), 1, "the source's record itself is in the snapshot (name, extension, id)");

    // and the undo still restores the state COMPLETELY: the bytes come back from the live project
    app.disk.edits.baseline = snap;
    let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand
    app.undo();
    let live: Vec<u8> = app.project.sources.iter().find(|s| s.id == src_id).map(|s| s.data.clone()).unwrap_or_default();
    assert_eq!(live, payload, "after an undo the source's bytes are there - saving will not lose the original");
}

/// There can be SEVERAL background tasks. There used to be a single slot: Ctrl+S during the restoration of the
/// imports' B-rep overwrote its receiver - the thread finished computing and sent the shapes into a closed
/// channel, and the import's B-rep did not appear until a restart (while the ready flag was already set).
#[test]
fn save_during_import_restore_does_not_lose_shapes() {
    use qymcad_core::feature::FeatureKind;
    let dir = std::env::temp_dir().join("qym_bg_jobs_test");
    std::fs::create_dir_all(&dir).unwrap();
    let step = dir.join("src.step");
    let proj_path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&proj_path);

    // a real embedded source: a block, then STEP, then the bytes into the project plus an import node on them
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("the block");
    qymcad_kernel::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], step.to_string_lossy().as_ref()).expect("the step was written");
    let bytes = std::fs::read(&step).expect("the step's bytes");
    let mut app = App::default();
    let src_id = app.project.alloc_id();
    app.project.sources.push(qymcad_core::model::SourceFile { id: src_id, name: "src.step".into(), ext: "step".into(), data: bytes });
    let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
    app.project.timeline.push(qymcad_core::feature::FeatureNode {
        id: body,
        name: "Import".into(),
        kind: FeatureKind::Import { body, source: src_id, solid: 0, scale: 1.0 },
        parent: Some(app.project.root),
        dirty: false,
        suppressed: false,
    });
    app.disk.project_path = Some(proj_path.clone());

    // BOTH tasks at once: topping up the import's B-rep and writing the project
    app.spawn_import_shapes(false);
    app.save_project();
    assert_eq!(app.regen.bg.len(), 2, "two background tasks live at once (the second used to overwrite the first)");
    app.wait_bg();

    assert!(app.live.shapes.contains_key(&body), "the import's B-rep was restored rather than lost in a closed channel");
    assert!(std::path::Path::new(&proj_path).exists(), "the project was written: {}", app.status);
    assert!(app.regen.bg.is_empty(), "the background queue is empty");
    let _ = std::fs::remove_file(&proj_path);
    let _ = std::fs::remove_file(&step);
}

/// Two writes of one file at once are a race between the temporary file and the rename. The second request
/// must WAIT rather than start in parallel; and it is not lost - it runs immediately after.
#[test]
fn second_save_is_queued_not_parallel() {
    let dir = std::env::temp_dir().join("qym_bg_queue_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&path);
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    app.disk.project_path = Some(path.clone());

    app.save_project();
    app.save_project(); // the second request, while the first is still in flight
    assert_eq!(app.regen.bg.iter().filter(|b| b.kind == BgKind::Save).count(), 1, "there are no parallel writes");
    assert!(app.disk.io.save_request.is_some(), "the second request is deferred rather than thrown away");
    app.wait_bg();
    assert!(app.disk.io.save_request.is_none(), "the deferred write was carried out");
    assert!(std::path::Path::new(&path).exists() && !qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), "the file is written and the project is clean: {}", app.status);
    let _ = std::fs::remove_file(&path);
}

/// Save used to mark the project CLEAN before the background write had finished. If the write failed (no
/// permission, a full disk, a broken path), `is_dirty()` was already false, the "save?" dialogue would not
/// appear on closing and the edits would go silently. The snapshot's key must be applied ONLY on success.
#[test]
fn failed_save_keeps_project_dirty() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    assert!(qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), "after building there are unsaved edits");
    // a deliberately impossible path: a directory instead of a file, with non-existent parents
    app.disk.project_path = Some("/proc/qymcad-no-such/path/proj.qcad".into());
    app.save_project();
    app.wait_bg();
    assert!(app.status.words.contains(&crate::i18n::tr_prefix("io-save-error", "error")), "the error is honestly in the status line: {}", app.status);
    assert!(qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), "the project STAYS dirty - otherwise quitting without a warning would lose the work");
}

/// A successful write, conversely, must clear the dirt - and by THE SNAPSHOT: an edit made while the write was
/// running makes the project dirty again.
#[test]
fn successful_save_clears_dirty_but_edits_during_write_do_not() {
    let dir = std::env::temp_dir().join("qym_save_dirty_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&path);
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    app.disk.project_path = Some(path.clone());
    app.save_project();
    app.wait_bg();
    assert!(!qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), "after a successful write the project is clean: {}", app.status);
    assert!(std::path::Path::new(&path).exists(), "the file is on disk");
    // an edit after the snapshot makes it dirty again (a full edit through the command layer)
    let si = sketch_rect(&mut app, 40.0, 40.0, 50.0, 50.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    assert!(qymcad_ui_state::is_dirty(&mut app.rebuild_ctx()), "a new edit makes the project dirty again");
    let _ = std::fs::remove_file(&path);
}

/// `ensure_brep` used to raise the "cache is ready" flag UNCONDITIONALLY. When there was nothing to rebuild
/// from (an import waiting for its B-rep to be restored from an embedded STEP), operations silently fell into
/// "the body was not built" and there was never a second attempt. The flag must reflect THE FACT.
#[test]
fn ensure_brep_reports_readiness_by_fact() {
    use qymcad_core::feature::FeatureKind;
    let mut app = App::default();
    let cube = build_cube(&mut app);
    // emulate opening from a bundle: the geometry is there and there is no live B-rep
    app.live.shapes.clear();
    app.live.ready = false;
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(app.live.ready && app.live.shapes.contains_key(&cube), "an ordinary feature rebuilds: {}", app.status);

    // and now a body there is NOTHING to rebuild from: an import node with no source in the kernel's cache
    let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
    app.project.timeline.push(qymcad_core::feature::FeatureNode {
        id: body,
        name: "Import".into(),
        kind: FeatureKind::Import { body, source: 0, solid: 0, scale: 1.0 },
        parent: Some(app.project.root),
        dirty: false,
        suppressed: false,
    });
    app.live.ready = false;
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(!app.live.ready, "the import's B-rep is not restored - the cache is NOT ready and the attempt must be repeated");
}

/// Reported behaviour: pressing "create a sketch" made the rebuild window flicker back and forth. Preparing the
/// B-rep is called EVERY FRAME (the sketch plane pick), and when there is nothing to rebuild a body from, the
/// attempt must happen ONCE: the "already tried" mark was set BEFORE the rebuild, which moves geom_rev, so the
/// mark never matched and the rebuild went into an endless loop, flickering the overlay.
#[test]
fn failed_brep_attempt_is_not_repeated_every_frame() {
    use qymcad_core::feature::FeatureKind;
    let mut app = App::default();
    build_cube(&mut app);
    // a body there is NOTHING to rebuild from (an import with no source) - the cache will never become ready
    let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
    app.project.timeline.push(qymcad_core::feature::FeatureNode {
        id: body,
        name: "Import".into(),
        kind: FeatureKind::Import { body, source: 0, solid: 0, scale: 1.0 },
        parent: Some(app.project.root),
        dirty: false,
        suppressed: false,
    });
    app.live.shapes.clear();
    app.live.ready = false;
    app.live.tried_rev = None;

    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(!app.live.ready, "the cache is honestly NOT ready - there is nothing to rebuild the import from");
    let rev = app.regen.geom_rev;

    // the next frames of the plane pick: not one new rebuild while the geometry has not changed
    for _ in 0..5 {
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    }
    assert_eq!(app.regen.geom_rev, rev, "there are no repeated rebuilds - the overlay does not flicker");
}

/// AN UNDO must roll back the live B-rep as well, not only the meshes. `restore()` returned `project`, `faces`
/// and `mesh_visible` but NOT `App.shapes` - after Ctrl+Z the mesh showed the old geometry while the kernel's
/// cache held the NEW one, and the next operation was built on the undone shape. Silently: after a rollback the
/// nodes are not dirty, so this does not heal by itself.
#[test]
fn undo_restores_brep_cache_not_only_meshes() {
    use qymcad_core::feature::FeatureKind;
    let mut app = App::default();
    let cube = build_cube(&mut app);
    let v0 = app.live.shapes.get(&cube).map(|s| s.volume()).unwrap_or(0.0);
    assert!((v0 - 8000.0).abs() < 1.0, "a 20-cubed block was built: {v0}");
    app.disk.edits.baseline = crate::gui::snapshot(&app.project); // as in the real cycle: the state before the edit is fixed

    // editing the extrude's HEIGHT from 20 to 30 (the same body, the Id does not change - the trickiest case)
    let fid = app.project.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Extrude { .. })).map(|n| n.id).expect("the extrude's node");
    crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 30.0;
        p.txt = "30".into();
    }
    app.apply_feat_cmd();
    let body = app.project.timeline.iter().find_map(|n| if n.id == fid { n.kind.body() } else { None }).expect("the feature's body");
    let v1 = app.live.shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
    assert!((v1 - 12000.0).abs() < 120.0, "after editing 20 -> 30: V={v1:.0}");
    // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand

    app.undo();

    // the mesh came back - that already worked
    let tris = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.tris.len()).unwrap_or(0);
    assert!(tris > 0, "the body is there after the undo");
    let mz = app.project.mesh_index(body).and_then(|i| app.project.bodies[i].mesh.bounds()).map(|b| b.max.z).unwrap_or(0.0);
    assert!((mz - 20.0).abs() < 0.1, "the mesh rolled back to a height of 20: max z = {mz}");
    // and the B-rep is where the defect lived: the cache must match what is on the screen
    let vu = app.live.shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
    assert!((vu - 8000.0).abs() < 80.0, "after an undo the live B-rep must be the pre-edit one: V={vu:.0} (expected 8000; 12000 means the cache stayed from the undone edit)");
}

/// The basic undo and redo cycle through the command layer: edit a sketch, undo, redo.
/// There were NO tests of undo at all, although it is one of the most used functions.
#[test]
fn undo_redo_cycle_on_sketch_edit() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let volume = |a: &App| -> f64 {
        let consumed = qymcad_ui_state::consumed_bodies(&a.project);
        a.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
    };
    app.disk.edits.baseline = crate::gui::snapshot(&app.project);
    let v0 = volume(&app);

    // stretch the base rectangle from 20 to 30 through the GUI cycle. In a live window this is a point DRAG,
    // which opens an edit of its own (see `drag_pt`): the same thing is done here by hand - an edit boundary.
    app.enter_sketch_edit(0);
    qymcad_ui_state::begin_edit(&mut app.disk.edits, &app.project, "Moving a point");
    for pt in &mut app.project.sketches[0].points {
        if (pt.x - 20.0).abs() < 1e-9 {
            pt.x = 30.0;
        }
    }
    app.project.solve_sketch(0);
    app.project.regen_sketch(0);
    app.finish_sketch_edit();
    qymcad_ui_state::commit_edit(&mut app.rebuild_ctx());
    let v1 = volume(&app);
    assert!((v1 - 12000.0).abs() < 120.0, "the sketch edit was applied: {v1:.0}");
    // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand

    app.undo();
    assert!((volume(&app) - v0).abs() < 80.0, "the undo restored a volume of {v0:.0}, got {:.0}", volume(&app));
    app.redo();
    assert!((volume(&app) - v1).abs() < 80.0, "the redo restored a volume of {v1:.0}, got {:.0}", volume(&app));
}

/// AUTOSAVE: an edit writes an autosave beside the project; that autosave reads back; Save removes it; and a
/// clean state is not written again.
#[test]
fn autosave_cycle() {
    let dir = std::env::temp_dir().join("qym_autosave_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let auto = std::path::Path::new(&path).with_extension("").to_string_lossy().into_owned() + ".autosave.qcad";
    let _ = std::fs::remove_file(&auto);
    let mut app = App::default();
    app.disk.project_path = Some(path.clone());
    let _cube = build_cube(&mut app); // unsaved edits
    app.maybe_autosave(true);
    app.wait_bg(); // the write runs in the background, so the test synchronises explicitly
    assert!(std::path::Path::new(&auto).exists(), "the autosave was created: {}", app.status);
    let loaded = qymcad_io::load_project(&auto).expect("the autosave reads");
    assert_eq!(loaded.timeline.len(), app.project.timeline.len(), "the autosave holds the whole timeline");
    // forcing it again with no new edits does not rewrite it (the key matched)
    let m1 = std::fs::metadata(&auto).unwrap().modified().unwrap();
    app.maybe_autosave(true);
    app.wait_bg();
    let m2 = std::fs::metadata(&auto).unwrap().modified().unwrap();
    assert_eq!(m1, m2, "the same state is not written twice");
    // Save removes the autosave
    app.save_project();
    app.wait_bg();
    assert!(!std::path::Path::new(&auto).exists(), "after Save the autosave is gone");
    assert!(std::path::Path::new(&path).exists(), "the project was written: {}", app.status);
}

/// THE AUTOSAVE LINE TELLS THE WALL CLOCK. Reported behaviour: at 14:28 in UTC+3 the line read
/// "Auto-saved (11:27)". The minute is read on both sides of the write, so a write across a minute passes.
#[test]
fn the_autosave_line_tells_the_local_time() {
    let dir = std::env::temp_dir().join("qym_autosave_clock_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad").to_string_lossy().into_owned();
    let mut app = App::default();
    app.disk.project_path = Some(path);
    let _cube = build_cube(&mut app);
    let before = chrono::Local::now().format("(%H:%M)").to_string();
    app.maybe_autosave(true);
    app.wait_bg();
    let after = chrono::Local::now().format("(%H:%M)").to_string();
    assert!(app.status.words.contains(&before) || app.status.words.contains(&after), "the autosave line does not tell the local time {before} .. {after}: {:?}", app.status);
}

/// "hh:mm" follows the zone it is asked in: ahead of UTC, behind it, and across midnight both ways.
#[test]
fn the_clock_is_read_in_the_zone_asked() {
    let hour = |h: i32| chrono::FixedOffset::east_opt(h * 3600).expect("an offset within a day");
    let at_11_27 = 1_791_286_020; // 2026-10-06T11:27:00Z
    let at_23_30 = 1_791_329_400; // 2026-10-06T23:30:00Z
    assert_eq!(super::super::hh_mm_in(at_11_27, &chrono::Utc), "11:27");
    assert_eq!(super::super::hh_mm_in(at_11_27, &hour(3)), "14:27");
    assert_eq!(super::super::hh_mm_in(at_11_27, &hour(-5)), "06:27");
    assert_eq!(super::super::hh_mm_in(at_23_30, &hour(3)), "02:30");
    assert_eq!(super::super::hh_mm_in(at_11_27, &hour(-12)), "23:27");
}
