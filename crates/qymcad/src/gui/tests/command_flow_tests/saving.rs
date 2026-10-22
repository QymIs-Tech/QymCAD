//! Opening, saving, the live shapes behind them, undo and autosave.

use super::*;
use crate::gui::hand::Hand;

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
///
/// Both are waited for to land, as the frames of the window take them. Waited for the way the program quits
/// (`wait_bg`), the restoration was given 300 ms - a quit does not wait for what can be fetched again - and it took
/// longer in a run of the whole crate under a load of 45 on 20 threads: red there, green alone.
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
    app.drain_bg_for_test();

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

/// A scratch folder of one check, emptied first: a file left from an earlier run would answer for this one.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("qym_saving_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    dir
}

/// Ctrl+S into `path` through a whole frame of the window, as a person saves, and the write waited for.
fn press_save(app: &mut App, path: &str) {
    // THE PATH IS REMEMBERED as Save As remembers it: without one Ctrl+S opens a file dialogue a check does not have
    crate::gui::set_project_path(&mut app.disk.project_path, &mut app.set, path.to_string());
    Hand::new(app).ctrl(egui::Key::S);
    app.wait_bg();
}

/// The preview of the file at `path`, decoded.
fn preview_of(path: &str) -> Option<image::RgbaImage> {
    qymcad_io::load_project_thumb(path).map(|png| image::load_from_memory(&png).expect("the preview is a PNG that decodes").to_rgba8())
}

/// Two previews of one size to tell apart: the one taken first and the one after a change.
struct Previews<'a> {
    before: &'a image::RgbaImage,
    after: &'a image::RgbaImage,
}

/// How many pixels of the two previews differ.
fn pixels_apart(two: Previews) -> usize {
    two.before.pixels().zip(two.after.pixels()).filter(|(p, q)| p != q).count()
}

/// CTRL+S PUTS THE PREVIEW INTO THE FILE, THE AUTOSAVE DOES NOT, AND A FILE WITH NOTHING TO DRAW HAS NONE.
///
/// The preview is what a file manager shows for the project: the model on a transparent ground, with the badge
/// of the program in the lower right corner (64x64, 8 px from the edges).
#[test]
fn save_project_embeds_preview_thumbnail() {
    let dir = scratch_dir("embeds");
    let path = dir.join("model.qcad").to_string_lossy().into_owned();
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    press_save(&mut app, &path);

    let img = preview_of(&path).unwrap_or_else(|| panic!("the saved project holds no preview: {}", app.status));
    assert_eq!(img.dimensions(), (256, 256), "the preview is 256x256");
    assert_eq!(img.get_pixel(0, 0)[3], 0, "the ground of the preview is transparent");
    assert_eq!(img.get_pixel(128, 128)[3], 255, "the cube in the middle of the frame is opaque");
    assert_eq!(img.get_pixel(216, 216)[3], 255, "the badge stands in the lower right corner");

    // the autosave, by its own trigger: an edit and the timer
    let _second = sketch_rect(&mut app, 40.0, 0.0, 50.0, 10.0);
    app.maybe_autosave(true);
    app.wait_bg();
    let auto = qymcad_ui_state::autosave_path(&app.disk.project_path);
    assert!(std::path::Path::new(&auto).exists(), "the autosave was written: {}", app.status);
    assert!(qymcad_io::load_project_thumb(&auto).is_none(), "the autosave carries a preview");

    // a document with no body: the file is shown with the icon of its type
    let empty = dir.join("empty.qcad").to_string_lossy().into_owned();
    let mut blank = App::default();
    let _sketch = sketch_rect(&mut blank, 0.0, 0.0, 20.0, 20.0);
    press_save(&mut blank, &empty);
    assert!(std::path::Path::new(&empty).exists(), "the sketch was saved: {}", blank.status);
    assert!(qymcad_io::load_project_thumb(&empty).is_none(), "a document of one sketch carries a preview");

    // THE PREVIEW OF THE PREVIOUS VERSION IS NOT CARRIED OVER: the cube undone, the file holds a sketch alone
    let mut back = App::default();
    let _cube = build_cube(&mut back);
    press_save(&mut back, &empty);
    assert!(qymcad_io::load_project_thumb(&empty).is_some(), "setup: the cube saved over the sketch holds a preview");
    Hand::new(&mut back).undo();
    press_save(&mut back, &empty);
    assert!(qymcad_io::load_project_thumb(&empty).is_none(), "the cube was undone and the file still shows its preview");
    let _ = std::fs::remove_dir_all(&dir);
}

/// THE PREVIEW SHOWS WHAT THE WINDOW SHOWS: a component or a body whose tick is off is not drawn, and neither is a
/// body a cut consumed.
#[test]
fn preview_thumbnail_filters_consumed_and_hidden_bodies() {
    let dir = scratch_dir("filters");
    let path = dir.join("shown.qcad").to_string_lossy().into_owned();

    // the plain block, the picture to tell a cut from
    let mut plain = App::default();
    let _cube = build_cube(&mut plain);
    press_save(&mut plain, &path);
    let block = preview_of(&path).expect("the block has a preview");

    // A CORNER COLUMN CUT RIGHT THROUGH: the block before the cut stays in the document as a consumed body, and
    // drawn it fills the corner back in - the picture of the plain block again.
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let column = sketch_rect(&mut app, 10.0, 10.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(column);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();
    assert!(!qymcad_ui_state::consumed_bodies(&app.project).is_empty(), "setup: the cut consumed the block: {}", app.status);
    press_save(&mut app, &path);
    let cut = preview_of(&path).expect("the cut block has a preview");
    let apart = pixels_apart(Previews { before: &block, after: &cut });
    assert!(apart > 200, "the preview of the cut block is the plain block but for {apart} pixels: the consumed block is drawn");

    // THE TICK OF THE BODY in the tree, which lists the bodies of a part once the part is entered
    let mi = app.project.bodies.iter().position(|b| !b.mesh.tris.is_empty() && !qymcad_ui_state::consumed_bodies(&app.project).contains(&b.id)).expect("the body of the cut");
    let name = crate::i18n::name(&app.project.mesh_name(mi));
    let part = app.project.body_owner(app.project.bodies[mi].id).and_then(|c| app.project.components.iter().find(|x| x.id == c)).map(|c| crate::i18n::name(&c.name)).expect("the part of the body");
    assert!(Hand::new(&mut app).double_click_word(&part), "the tree shows no part {part:?}");
    assert!(Hand::new(&mut app).press_tick_before(&name, egui::pos2(0.0, 300.0)), "the tree shows no tick before the body {name:?}");
    Hand::new(&mut app).close_window();
    assert!(!app.project.bodies[mi].visible, "setup: the tick before {name:?} did not hide the body");
    press_save(&mut app, &path);
    assert!(qymcad_io::load_project_thumb(&path).is_none(), "the only body is hidden and the file still has a preview");
    assert!(Hand::new(&mut app).press_tick_before(&name, egui::pos2(0.0, 300.0)), "the tree shows no tick before the body {name:?}");
    Hand::new(&mut app).close_window();

    press_save(&mut app, &path);
    let shown = preview_of(&path).expect("the body shown again brings the preview back");
    assert_eq!(pixels_apart(Previews { before: &cut, after: &shown }), 0, "the body hidden and shown again, the preview is not the one it was");

    // THE TICK OF THE COMPONENTS heading of the assembly hides every part
    let heading = qymcad_i18n::tr("tree-components");
    assert!(Hand::new(&mut plain).press_tick_before(&heading, egui::pos2(0.0, 300.0)), "the heading of the components has no tick");
    Hand::new(&mut plain).close_window();
    press_save(&mut plain, &path);
    assert!(qymcad_io::load_project_thumb(&path).is_none(), "every component is hidden and the file still has a preview");
    assert!(Hand::new(&mut plain).press_tick_before(&heading, egui::pos2(0.0, 300.0)), "the heading of the components has no tick");
    Hand::new(&mut plain).close_window();
    press_save(&mut plain, &path);
    let again = preview_of(&path).expect("the components shown again bring the preview back");
    assert_eq!(pixels_apart(Previews { before: &block, after: &again }), 0, "the components hidden and shown again, the preview is not the one it was");
    let _ = std::fs::remove_dir_all(&dir);
}

/// ONE FILE, ONE PREVIEW, WHATEVER THE SCHEME OF WHOEVER SAVES IT. Reported behaviour: the same cube saved from the
/// light scheme and from the dark one gave two different pictures, since the light scheme lifts the brightness
/// of a body.
#[test]
fn preview_thumbnail_does_not_follow_the_colour_scheme() {
    let dir = scratch_dir("scheme");
    let mut previews = Vec::new();
    for (k, pal) in [qymcad_scheme::dark(), qymcad_scheme::light()].into_iter().enumerate() {
        let path = dir.join(format!("scheme{k}.qcad")).to_string_lossy().into_owned();
        let mut app = App::default();
        app.scheme.pal = pal;
        let _cube = build_cube(&mut app);
        press_save(&mut app, &path);
        previews.push(preview_of(&path).expect("the cube has a preview"));
    }
    let apart = pixels_apart(Previews { before: &previews[0], after: &previews[1] });
    assert_eq!(apart, 0, "the cube saved from the dark and from the light scheme differs in {apart} pixels");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A PIXEL KEEPS ITS COLOUR IN THE PNG AT ANY OPACITY. `Color32` holds premultiplied alpha and a PNG straight alpha:
/// written as held, a pixel at half opacity came back at half its brightness - the rim of the badge on the
/// transparent preview was a dark fringe.
#[test]
fn a_translucent_pixel_keeps_its_colour_in_the_png() {
    let held = qymcad_scheme::dark().warning().gamma_multiply(0.5);
    let want = held.to_srgba_unmultiplied();
    let img = egui::ColorImage { size: [1, 1], source_size: egui::Vec2::new(1.0, 1.0), pixels: vec![held] };
    let png = crate::gui::color_image_to_png(&img).expect("encodes");
    let back = image::load_from_memory(&png).expect("decodes").to_rgba8();
    let p = back.get_pixel(0, 0);
    assert_eq!(p.0, want, "the half-opaque warning colour came out of the PNG changed");
}

/// NOTHING DRAWN, NO PICTURE: a body whose mesh holds points but no triangle gives neither a file preview nor a
/// part preview - an empty frame with a badge or a bare background would stand for a model that is not there.
#[test]
fn preview_thumbnail_omitted_when_mesh_has_no_triangles() {
    let mut app = App::default();
    let body = build_cube(&mut app);
    let mi = app.project.mesh_index(body).expect("the block's mesh");
    for b in app.project.bodies.iter_mut() {
        b.mesh.tris.clear();
    }
    assert!(!app.project.bodies[mi].mesh.verts.is_empty(), "setup: the mesh keeps its points");
    assert!(crate::gui::render_scene::render_project_file_preview_png(&app.project).is_none(), "a mesh of points alone gives a file preview");
    let part = app.project.body_owner(body).expect("the block's part");
    assert!(crate::gui::render_scene::render_component_thumbnail(&app.draw_ctx(), part).is_none(), "a mesh of points alone gives a part preview");
}

/// THE PART PREVIEW (Save as part, the library) DRAWS THE PART WHOLE, its tick in the tree off or on: a part put
/// into the library hidden is still the part.
#[test]
fn component_thumbnail_renders_even_when_component_is_hidden() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let heading = qymcad_i18n::tr("tree-components");
    assert!(Hand::new(&mut app).press_tick_before(&heading, egui::pos2(0.0, 300.0)), "the heading of the components has no tick");
    Hand::new(&mut app).close_window();
    assert!(crate::gui::render_scene::render_project_file_preview_png(&app.project).is_none(), "the file preview draws hidden components");
    assert!(crate::gui::render_scene::render_component_thumbnail(&app.draw_ctx(), app.project.root).is_some(), "the part preview leaves a hidden part out");
}

/// THE WARNING ABOUT A NEWER AUTOSAVE REACHES THE STATUS LINE. Reported behaviour: a project was opened while an
/// autosave newer than it lay beside it, and the line said "The project is loaded." - the warning was set and
/// painted over in the same call, so the work kept in the autosave went unmentioned.
#[test]
fn opening_says_an_autosave_is_newer() {
    let dir = scratch_dir("recover");
    let path = dir.join("work.qcad").to_string_lossy().into_owned();
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    press_save(&mut app, &path);
    // the session broke off after an edit: the autosave is newer than the file
    let _edit = sketch_rect(&mut app, 40.0, 0.0, 50.0, 10.0);
    app.maybe_autosave(true);
    app.wait_bg();
    let auto = qymcad_ui_state::autosave_path(&app.disk.project_path);
    assert!(std::path::Path::new(&auto).exists(), "setup: the autosave was written: {}", app.status);
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(hour_ago)).expect("the file is dated an hour back");

    let mut opened = App::default();
    opened.open_for_test(path.clone());
    let warning = crate::i18n::tr1("io-autosave-found", "path", &auto);
    let first_words: String = warning.chars().take(30).collect();
    let pal = opened.scheme.pal.clone();
    let said = Hand::new(&mut opened).status_painted().into_iter().find(|w| w.text.contains(&first_words));
    assert!(said.as_ref().is_some_and(|w| w.colour == pal.warning()), "the status line does not say in yellow that a newer autosave lies beside the file: {:?}", said.map(|w| (w.text, w.colour)));
    let _ = std::fs::remove_dir_all(&dir);
}
