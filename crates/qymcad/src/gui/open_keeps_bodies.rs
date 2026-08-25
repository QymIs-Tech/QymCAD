//! OPEN A PROJECT AND IT IS SIMPLY OPEN. Written from a report: on opening a project the bodies are
//! sometimes not visible until you rebuild.
//!
//! "Sometimes" means a PARAMETRIC project. The snapshot of the parameter values (`params_seen`) was
//! moved only by a SYNCHRONOUS rebuild, while in a live window the rebuild is asynchronous — so the
//! snapshot was NEVER updated. Every parameter counted as changed for ever: the scheduler marked all
//! the parametrics dirty, the frame asked for a rebuild, the rebuild did not move the snapshot — and
//! the circle repeated. An opened project rebuilt WITHOUT END, and for nothing: it has no live B-rep
//! yet (the geometry comes from the bundle), so every such rebuild failed with "the source body is not
//! built".
//!
//! The tests hold BOTH boundaries. "The bodies are on screen" is weak on its own: the circle did not
//! erase them, it kept the window in a rebuild with an error in the status line. "Exactly one rebuild"
//! is weak on its own too: zero rebuilds would pass it, and "nothing was built" would pass along with
//! it.
#[cfg(test)]
mod tests {
    use super::super::{App, BgKind, Busy, JobResult, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::Constraint;

    /// A plate with a FILLET whose radius is an EXPRESSION over a named sketch dimension, on disk.
    ///
    /// Both halves carry weight and both are taken from real work: the named dimension (without it the
    /// project holds no parameter whose "has it changed" started the circle) and the expression on a
    /// MODIFIER feature (the base is built from the sketch and asks for no live B-rep, so the failure
    /// is not visible on it).
    fn saved_parametric_project(name: &str) -> String {
        let dir = std::env::temp_dir().join("qym_open_keeps_bodies_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join(name).to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        // A NAMED DIMENSION: the width of the plate is available as the parameter `len` (a skeleton
        // dimension, top-down).
        let (a, b) = {
            let s = &app.project.sketches[si];
            let left = s.points.iter().min_by(|p, q| p.x.total_cmp(&q.x)).expect("the points of the rectangle").id;
            let right = s.points.iter().max_by(|p, q| p.x.total_cmp(&q.x)).expect("the points of the rectangle").id;
            (left, right)
        };
        app.project.sketches[si].constraints.push(Constraint::Distance { a, b, d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        let sid = app.project.sketches[si].id;
        assert!(app.project.add_named_dim("len".into(), sid, vec![a, b]), "setup: the dimension is named");
        app.project.regen_sketch(si);
        app.finish_sketch_edit();

        // A GLOBAL VARIABLE for the height is the second half of the parametrics: that is the one a
        // person edits by hand.
        app.project.parameters.push(qymcad_core::model::Param { name: "h".into(), expr: "10".into(), value: 10.0 });
        app.project.eval_parameters();

        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "h".into();
        }
        app.apply_feat_cmd();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("setup: the plate is built");

        app.sel = Sel::Mesh(app.project.mesh_index(body).expect("setup: the plate has a mesh"));
        app.start_feat_cmd(4); // fillet
        app.gsel.edges = app.body_edges_cached(body).map(|e| e.1.iter().copied().filter(|&i| i != 0).collect()).unwrap_or_default();
        app.edges.body = Some(body);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 2.0;
            p.txt = "len/20".into(); // THE RADIUS IS PARAMETRIC: this is exactly what was marked dirty every frame
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty_for_test();
        assert!(!app.project.feat_dims.is_empty(), "setup: the expression of the feature dimension is stored");
        assert!(!app.project.named_dims.is_empty(), "setup: the named dimension is stored");

        app.set_project_path(path.clone());
        app.save_project_for_test();
        app.wait_bg_for_test();
        path
    }

    /// Open a file in a LIVE window: a rebuild from there goes into a thread — which is exactly where
    /// all of this happened.
    fn opened_in_a_live_window(path: &str) -> App {
        let mut app = App::default();
        app.regen.ui_running = true;
        app.spawn_project_load(path.to_string());
        app.drain_busy_for_test(); // reading a file goes through the MODAL queue rather than the background one
        app
    }

    /// One frame of the dispatcher of a live window without egui: what `tick_async` does.
    /// Returns true if this frame STARTED a rebuild.
    fn pump_frame(app: &mut App) -> bool {
        let started = app.regen.wanted && app.regen.busy.is_none();
        if started {
            app.regen.wanted = false;
            app.spawn_regen();
        }
        if app.regen.busy.is_none() && app.edits.open.is_none() {
            app.rebuild_if_dirty_for_test();
        }
        if let Some(Busy { rx, kind: BgKind::Regen, .. }) = app.regen.busy.take() {
            match rx.recv_timeout(std::time::Duration::from_secs(120)).expect("the rebuild thread reported back") {
                JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => app.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
                other => app.apply_job_result(other),
            }
        }
        started
    }

    /// THE PART IS ON SCREEN — and stays there however many frames are run.
    #[test]
    fn opening_a_parametric_project_keeps_its_bodies_on_screen() {
        let path = saved_parametric_project("keeps_bodies.qcad");
        let mut app = opened_in_a_live_window(&path);
        assert!(app.visible_mesh_items_for_test() > 0, "right after opening the part must be on screen: the geometry came from the file");

        for i in 0..8 {
            pump_frame(&mut app);
            assert!(app.visible_mesh_items_for_test() > 0, "after frame {} the part vanished from the screen — the reported \"until you rebuild\"; status: {}", i + 1, app.status_for_test());
        }
        let _ = std::fs::remove_file(&path);
    }

    /// THERE IS ONE REBUILD — the one the opening itself scheduled. After that the frames run and
    /// there is no work.
    #[test]
    fn opening_a_parametric_project_rebuilds_once_not_every_frame() {
        let path = saved_parametric_project("rebuild_once.qcad");
        let mut app = opened_in_a_live_window(&path);

        let regens = (0..8).filter(|_| pump_frame(&mut app)).count();
        assert_eq!(regens, 1, "opening schedules EXACTLY one rebuild; there were {regens} over eight frames — the circle does not break");
        let _ = std::fs::remove_file(&path);
    }

    /// AND IT DOES NOT FAIL. The circle was not merely superfluous: every turn of it honestly failed,
    /// because a just-opened project has no live B-rep yet — and that is what people read in the status
    /// line.
    #[test]
    fn the_rebuild_that_opening_schedules_does_not_fail() {
        let path = saved_parametric_project("no_error.qcad");
        let mut app = opened_in_a_live_window(&path);
        for _ in 0..4 {
            pump_frame(&mut app);
        }
        assert!(app.project.regen_errors.is_empty(), "opening must leave no unbuilt features, and {} were left; status: {}", app.project.regen_errors.len(), app.status_for_test());
        let _ = std::fs::remove_file(&path);
    }

    /// OPENING MARKS NOT ONE FEATURE DIRTY. The parameter values in a file are already applied: the
    /// geometry of the bundle was built by exactly those. Without a "seen" mark the very first pass of
    /// the scheduler declared ALL the parameters changed at once, and the opening scheduled a full
    /// rebuild of the parametrics for itself — on a part that is needless work for nothing, on an
    /// assembly it is seconds.
    #[test]
    fn opening_marks_nothing_for_rebuild() {
        let path = saved_parametric_project("no_dirt.qcad");
        let mut app = opened_in_a_live_window(&path);
        app.rebuild_if_dirty_for_test(); // this is where the "which parameters changed" check stands
        let dirty: Vec<&str> = app.project.timeline.iter().filter(|n| n.dirty).map(|n| n.name.as_str()).collect();
        assert!(dirty.is_empty(), "opening marked {} features dirty ({dirty:?}) — the file was opened, not changed", dirty.len());
        let _ = std::fs::remove_file(&path);
    }

    /// AND EDITING A PARAMETER ALSO REBUILDS ONCE — and the part follows the value.
    ///
    /// A test of its own, because the "values seen" mark is set in TWO places and for different
    /// reasons: on opening (the values came from the file already applied) and on a rebuild that
    /// actually arrived from the thread. The first closes the opening, the second the work afterwards;
    /// remove the second while keeping the first and the opening will not notice, but the circle comes
    /// back after the very first edit.
    #[test]
    fn editing_a_global_parameter_rebuilds_once_and_the_body_follows() {
        let path = saved_parametric_project("param_edit.qcad");
        let mut app = opened_in_a_live_window(&path);
        for _ in 0..4 {
            pump_frame(&mut app); // carry the opening through to silence
        }
        let before = app.body_height_for_test();

        app.set_param_for_test("h", "16"); // as an edit in the parameters window
        let regens = (0..8).filter(|_| pump_frame(&mut app)).count();
        assert_eq!(regens, 1, "editing a parameter means EXACTLY one rebuild; there were {regens} over eight frames");
        let after = app.body_height_for_test();
        assert!((after - before - 6.0).abs() < 0.05, "the part must follow the parameter: it was {before:.2}, it became {after:.2}, +6 was expected");
        assert!(app.project.regen_errors.is_empty(), "editing a parameter must not break features; status: {}", app.status_for_test());
        let _ = std::fs::remove_file(&path);
    }
}
