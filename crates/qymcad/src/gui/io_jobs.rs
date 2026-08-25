//! FILES AND BACKGROUND JOBS - opening, saving, importing, exporting, and taking their results on the
//! UI thread.

use super::*;

impl App {
    /// Integrating a finished STL mesh (from the worker) into the project - on the UI thread, and fast.
    pub(super) fn finish_stl_import(&mut self, path: String, mesh: qymcad_core::geom::Mesh, faces: Vec<qymcad_core::geom::MeshFace>) {
        self.begin_edit(&crate::i18n::tr("io-import-stl")); // this is an EDIT of the document: a body is added to the current one
        let tris = mesh.tris.len();
        self.add_bodies(vec![(mesh, faces)]);
        self.embed_source(&path);
        self.dxf_path = Some(path);
        self.status = crate::i18n::tr1("io-stl-added", "n", &tris.to_string());
            self.commit_edit();
    }


    /// Open a project asynchronously: the heavy part (RON parsing of the timeline plus reparsing the
    /// embedded STEP into live B-rep shapes) goes to a worker thread while the UI shows a splash with a
    /// spinner. The result arrives as `JobResult::ProjectLoaded` -> `finish_project_load`, already on the
    /// UI thread.
    pub(super) fn spawn_project_load(&mut self, path: String) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let res = match qymcad_io::load_project_with_brep(&path) {
                Ok((mut project, breps)) => {
                    project.ensure_document(); // normalisation: a root assembly plus reparenting of floating nodes
                    // The embedded STEP is NO LONGER parsed here (36 s on a real assembly) - the geometry
                    // comes from the bundle, and the B-rep of imports is fetched in the background once the
                    // model is already on screen.
                    // LIVE BODIES ARE PARSED RIGHT HERE, IN THE THREAD: on the UI thread this would be a
                    // freeze of exactly the kind we are getting away from.
                    let shapes = {
                        let _gate = qymcad_kernel::kernel_gate();
                        breps.into_iter().filter_map(|(id, b)| qymcad_kernel::Shape::from_brep_bytes(&b).map(|s| (id, s))).collect()
                    };
                    JobResult::ProjectLoaded { path, project: Box::new(project), shapes }
                }
                Err(e) => JobResult::Failed(crate::i18n::tr1("io-open-error", "error", &crate::i18n::name(&e.to_string()))),
            };
            let _ = tx.send(res);
        });
        self.regen.busy = Some(Busy { label: crate::i18n::tr("io-loading"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
    }


    /// WAIT for background work (writing the project, fetching B-rep) and apply its result.
    /// Needed where there will be no "later": leaving the program with a save still in flight. In tests it
    /// is a synchronisation point instead of waiting for UI frames.
    pub(super) fn wait_bg(&mut self) {
        // there can be several jobs (a write plus a B-rep fetch) - EACH is waited for, otherwise leaving
        // would cut off the one we never reached.
        // But THE WAIT HAS A CEILING. This used to be `recv()` with no timeout: closing the window during a
        // 36-second B-rep restore gave a dead frozen window again, this time without even a spinner. A data
        // write is waited for at length (those are edits, and losing them is not allowed); a B-rep fetch
        // only briefly - it can always be repeated and the geometry does not suffer.
        for bg in std::mem::take(&mut self.regen.bg) {
            let budget = match bg.kind {
                BgKind::Save => std::time::Duration::from_secs(120),
                BgKind::ImportShapes => std::time::Duration::from_millis(300),
                // A rebuild is THE GEOMETRY OF THE EDITS: it is waited for, otherwise we close with an
                // unfinished model and lose the result of a heavy operation.
                BgKind::Regen => std::time::Duration::from_secs(120),
            };
            match bg.rx.recv_timeout(budget) {
                Ok(res) => self.apply_job_result(res),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("io-not-waited", "what", &bg.label));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("io-bg-broken", "what", &bg.label));
                }
            }
        }
        // the applied result may have queued the next job (a deferred write)
        if !self.regen.bg.is_empty() {
            self.wait_bg();
        }
    }


    /// Restore the B-rep of imported solids from the embedded STEP in a separate thread.
    /// `regen = true` means a file with no stored geometry: show a modal spinner and rebuild the timeline
    /// once the restore is done. Otherwise it is a QUIET background fetch: the model is already on screen
    /// and only operations on the imports have to wait (the status line says a restore is in progress).
    pub(super) fn spawn_import_shapes(&mut self, regen: bool) {
        let project = self.project.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let shapes = restore_import_shapes_for(&project);
            let _ = tx.send(JobResult::ImportShapes { shapes, regen });
        });
        if regen {
            self.regen.busy = Some(Busy { label: crate::i18n::tr("io-brep-restore"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
        } else {
            self.regen.bg.push(Busy { label: crate::i18n::tr("io-brep-restoring"), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
        }
    }


    /// Integrating a loaded project into the application, on the UI thread: rebuilding the timeline and the
    /// faces is comparatively light here (the heavy STEP reparse was already done in the worker).
    pub(super) fn finish_project_load(&mut self, path: String, project: Project, shapes: Vec<(Id, qymcad_kernel::Shape)>) {
        // OPENING A FILE IS NOT AN EDIT BUT A REPLACEMENT OF THE DOCUMENT. No undo step is created here;
        // the STACK IS CLEARED instead: otherwise Undo right after opening would bring back pieces of the
        // PREVIOUS document on top of the new one - a state that never existed.
        self.edits.undo.clear();
        self.edits.redo.clear();
        self.edits.open = None;
        self.edits.depth = 0;
        self.project = project;
        // PARAMETER VALUES FROM THE FILE ARE ALREADY APPLIED: the geometry in the bundle was built from
        // exactly these. Without this mark the snapshot is empty and the very first frame declares EVERY
        // parameter changed - opening a file used to schedule a full parametric rebuild for itself, and
        // without a live B-rep at that.
        self.settle_params_seen();
        // LIVE BODIES FROM THE FILE GO INTO THE CACHE rather than `clear()`. This used to be an
        // unconditional wipe: there was no live B-rep in the bundle at all, so there was nothing to wipe.
        // Now there is - and the first operation stopped paying with a full rebuild of the timeline. If it
        // is empty (a file written without bodies) everything is as before: `ensure_brep` fills the cache
        // on demand.
        self.live.shapes = shapes.into_iter().collect();
        // the faces came INSIDE the bodies (Body.faces) - there is nothing left to spread over a parallel
        // list.
        // GEOMETRY FROM THE BUNDLE rather than a rebuild from scratch. A full forced regen on opening cost
        // 31 s of tessellation (1170 imported solids) right on the UI thread - the window went "not
        // responding". The B-rep faces go back into `regen_faces` (associativity: sketches and features
        // resolve faces by id), and only what has no geometry in the file IS REBUILT.
        self.live.faces.clear();
        for (i, f) in self.project.bodies.iter().map(|b| &b.faces).enumerate() {
            if let (Some(body), false) = (self.project.mesh_id(i), f.is_empty()) {
                self.project.regen_faces.insert(body, f.clone());
                self.live.faces.insert(body, f.clone()); // a cache keyed by body Id survives edits to the topology
            }
        }
        let missing: Vec<Id> = self
            .project
            .timeline
            .iter()
            .filter_map(|n| n.kind.body())
            .filter(|b| self.project.mesh_index(*b).is_none())
            .collect();
        for n in &mut self.project.timeline {
            if n.kind.body().is_some_and(|b| missing.contains(&b)) {
                n.dirty = true;
            }
        }
        // to rebuild, imported solids need the live B-rep from the embedded STEP: with no geometry in the
        // file, restore first (a modal spinner); otherwise fetch quietly in the background after the show
        let needs_import_shapes = self.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Import { .. }));
        if missing.is_empty() {
            // the geometry comes entirely from the file: the live B-rep is built lazily, when really needed
            self.live.ready = false;
            self.regenerate_all(); // nothing is dirty - in effect this only synchronises the caches
            if needs_import_shapes {
                self.spawn_import_shapes(false);
            }
        } else if needs_import_shapes {
            self.spawn_import_shapes(true); // restores the B-rep and rebuilds what is missing
        } else {
            self.regenerate_all();
        }
        self.detect_missing_faces(); // mesh detection ONLY for raw meshes with no B-rep (an imported STL)
        self.sel = if self.project.operations.is_empty() { Sel::None } else { Sel::Op(0) };
        self.set_project_path(path);
        self.invalidate();
        self.view.initialized = false;
        self.cam.init = false;
        self.edits.saved_key = self.edit_key(); // straight off the disk - there are no edits
        // an autosave newer than the file means the previous session broke off after edits - offer to recover
        if let Some(p) = self.project_path.clone() {
            // ONE RULE FOR THE NAME, not a copy of it. This place used to build the autosave name itself, and
            // the moment `autosave_path` started taking the stem instead of the whole name the two would have
            // parted: the recovery check would look for a file the autosave never writes.
            let auto = self.autosave_path();
            let p = &p;
            let newer = (|| -> Option<bool> {
                let ma = std::fs::metadata(&auto).ok()?.modified().ok()?;
                let mf = std::fs::metadata(p).ok()?.modified().ok()?;
                Some(ma > mf)
            })()
            .unwrap_or(false);
            if newer {
                self.status = format!("{} {}", ph::WARNING, crate::i18n::tr1("io-autosave-found", "path", &auto));
            }
        }
        self.status = crate::i18n::tr("io-project-loaded");
    }


    /// Writing the project in a BACKGROUND thread. On a real assembly a save took seconds (compressing the
    /// embedded STEP plus serialising the meshes) and was done right on the UI thread - the window froze on
    /// every Save and on every autosave. Now the snapshot goes off to a thread and the window stays alive.
    ///
    /// IS A FILE BEING WRITTEN RIGHT NOW. Both the waiting card and the decision whether to wait for a
    /// transition are driven by this: the FACT must be asked for rather than remembered in a separate flag,
    /// because a flag will drift.
    pub(super) fn saving_now(&self) -> bool {
        self.regen.bg.iter().any(|b| b.kind == BgKind::Save)
    }

    pub(super) fn spawn_save(&mut self, path: String, autosave: bool) {
        // two writes of one file at once are a tmp+rename race. While the first is running, the LAST
        // request is remembered and started when the first reports back (see the JobResult::Saved handler).
        if self.regen.bg.iter().any(|b| b.kind == BgKind::Save) {
            self.io.save_request = Some((path, autosave));
            return;
        }
        // "WHEN IT WAS STARTED" IS A FACT, NOT A PROPERTY OF THE LAST WRITE: set once, on the first save,
        // and never touched again. An autosave does not start a document - it is a snapshot.
        if !autosave && self.project.meta.created.is_empty() {
            self.project.meta.created = crate::gui::now_iso8601();
        }
        let mut proj = self.project.clone();
        proj.regen_faces.clear(); // derived from the faces of the bundle - not duplicated
        proj.regen_edges.clear();
        // LIVE BODIES GO INTO THE FILE. Without them opening shows the model instantly, but the very first
        // operation rebuilds the whole timeline: there is nowhere to get a live B-rep from.
        self.live.blobs.retain(|id, _| self.live.shapes.contains_key(id)); // the body is gone -> the blob is not needed either
        let missing: Vec<qymcad_core::model::Id> = self.live.shapes.keys().filter(|id| !self.live.blobs.contains_key(id)).copied().collect();
        for id in missing {
            if let Some(b) = self.live.shapes.get(&id).and_then(|sh| sh.to_brep_bytes()) {
                self.live.blobs.insert(id, b);
            }
        }
        let breps: Vec<(qymcad_core::model::Id, Vec<u8>)> = self.live.blobs.iter().map(|(id, b)| (*id, b.clone())).collect();
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.clone();
        std::thread::spawn(move || {
            // THROUGH THE GUARDED WRITE: an empty document over a non-empty file is a refusal, not a loss.
            let res = qymcad_io::save_project_guarded_with_brep(&proj, &p, &breps);
            let _ = tx.send(JobResult::Saved { path: p, autosave, error: res.err() });
        });
        self.regen.bg.push(Busy { label: if autosave { crate::i18n::tr("io-autosaving") } else { crate::i18n::tr("io-saving") }, rx, kind: BgKind::Save, pulse: None, quiet: false });
        self.status = if autosave { crate::i18n::tr("io-autosaving") } else { crate::i18n::tr1("io-saving-path", "path", &path) };
    }


    /// Save (Ctrl+S): if the project has been saved before, write there silently; otherwise Save As.
    pub(super) fn save_project(&mut self) {
        match self.project_path.clone() {
            Some(path) => {
                // the write is in the background; the key is taken from the SNAPSHOT - edits made during the
                // write leave the project dirty. But it is APPLIED only after a successful write, otherwise a
                // failed save silently marked the project clean and leaving never asked about unsaved work.
                self.io.saved_key = Some(self.edit_key());
                self.spawn_save(path, false);
            }
            None => self.save_project_as(),
        }
    }


    /// Save As (Ctrl+Shift+S): always ask for the path and the name.
    pub(super) fn save_project_as(&mut self) {
        let start = self.project_path.clone().unwrap_or_else(|| "project.qcad".into());
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(std::path::Path::new(&start).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
            .add_filter("QymCAD project", &["qcad", "ron"])
            .save_file()
        {
            let path = path.to_string_lossy().into_owned();
            self.set_project_path(path.clone());
            self.io.saved_key = Some(self.edit_key()); // confirmed once the write actually succeeds
            self.spawn_save(path, false); // the write goes to the background
        }
    }


    /// Finish adding dimension `ci`: solve and CLASSIFY it. Redundant-but-CONSISTENT becomes a driven
    /// (reference) dimension. A CONFLICT with other constraints is NOT muffled - such a dimension is NOT
    /// made driven (otherwise it would "measure" averaged geometry and hide the conflict; making it driven
    /// is available by hand, with the button in the list of constraints). Returns (redundant, conflicting).
    ///
    /// The rank analysis is called DIRECTLY here, past the diagnostics cache: this is a one-off action
    /// right after an edit, and the answer is needed for the fresh state rather than for an imprint of the
    /// previous frame.
    pub(super) fn finish_dim(&mut self, si: usize, ci: usize) -> (bool, bool) {
        self.project.solve_sketch(si);
        let redundant = self.project.dim_redundant(si, ci);
        let conflict = self.project.sketch_conflicts(si).contains(&ci);
        if redundant && !conflict {
            self.project.auto_driven(si, ci);
            self.project.solve_sketch(si);
        }
        self.invalidate();
        (redundant, conflict)
    }


    /// Finish editing a sketch.
    pub(super) fn finish_sketch_edit(&mut self) {
        let edited = self.sketch_ses.editing;
        self.sketch_ses.editing = None;
        self.cmd.ref_body = None; // highlighting the faces of a neighbour belongs to the creation session only
        self.sel_sk.clear(); // the selection and whatever was waiting for it
        self.exit_draw_tools(); // leaving a sketch drops all of its modes in one transition
        // associativity: editing a sketch rebuilds the bodies built on it
        if let Some(sid) = edited {
            // ...and there is NOTHING TO REBUILD THEM ON while no live B-rep exists. A project from a
            // bundle shows the geometry FROM THE FILE and has no `Shape` (the lazy B-rep): one sketch node
            // gets marked dirty, the feature on it asks for its source body - and is told the source body
            // was not built. This was hit on the very first opening; "Rebuild all" cured it and it never
            // came back. `ensure_brep` builds EXACTLY the missing nodes rather than the whole project.
            self.ensure_brep();
            self.project.mark_sketch_dirty(sid);
            self.regenerate_all();
        }
        // restore the viewpoint and mode from before entering the sketch (pop one level off the stack)
        if let Some((cam, view, mode_3d)) = self.nav_stash.pop() {
            self.cam = cam;
            self.cam.init = true; // do not refit - keep the same framing
            self.view = view;
            self.mode_3d = mode_3d;
        }
        self.sync_workbench(); // the workbench follows the active context (part or assembly)
        self.status = crate::i18n::tr("io-sketch-done");
    }


    pub(super) fn finish_drawing(&mut self, closed: bool) {
        if let Some(pts) = self.pending_import.draw_pts.take() {
            let min = if closed { 3 } else { 2 };
            if pts.len() >= min {
                // while editing, fill the active sketch; otherwise start a new one
                if let Some(si) = self.edit_si() {
                    if self.project.fill_sketch_polyline(si, pts, closed) {
                        self.sel = Sel::Sketch(si);
                    } else {
                        self.status = crate::i18n::tr("io-sketch-has-profile");
                    }
                } else {
                    self.project.add_line_sketch(&crate::i18n::tr("io-sketch"), pts, closed);
                    self.sel = Sel::Sketch(self.project.sketches.len() - 1);
                }
                self.invalidate();
                self.view.initialized = false;
            }
        }
    }


    /// Exporting the G-code of a single operation.
    pub(super) fn export_op(&mut self, i: usize) {
        let Some(op) = self.project.operations.get(i) else { return };
        let prog_name = format!("{}_{}", self.program_name(), op.name.replace(' ', "_"));
        let program = self.project.build_program_for(&prog_name, &[i]);
        if program.toolpaths.is_empty() {
            self.status = crate::i18n::tr("io-op-no-toolpath");
            return;
        }
        let gcode = post_for(&program, self.project.machine.post, &self.post_options());
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{prog_name}.tap"))
            .add_filter("G-code", &["tap", "nc", "ngc"])
            .save_file()
        {
            match std::fs::write(&path, &gcode) {
                Ok(()) => self.status = crate::i18n::tr1("io-op-written", "path", &path.display().to_string()),
                Err(e) => self.status = crate::i18n::tr1("io-write-error", "error", &crate::i18n::name(&e.to_string())),
            }
        }
    }


    /// SORTING the target bodies by [`ExportKind`] - ONE routine for both STEP and STL, so that two exports
    /// of one project do not drift apart silently (STEP used to skip a body with no B-rep while STL quietly
    /// wrote out its last mesh).
    pub(super) fn export_plan(&self, target: ExportTarget) -> ExportPlan {
        let mut plan = ExportPlan::default();
        for b in self.visible_export_bodies(target) {
            match self.project.export_kind(b, self.live.shapes.contains_key(&b)) {
                qymcad_core::model::ExportKind::Brep => plan.brep.push(b),
                qymcad_core::model::ExportKind::MeshOnly => plan.mesh_only.push(b),
                qymcad_core::model::ExportKind::Stale => plan.stale.push(b),
            }
        }
        plan
    }


    /// The suggested file name for an export target (the component name or the project name).
    pub(super) fn export_base_name(&self, target: ExportTarget) -> String {
        match target {
            ExportTarget::Project => std::path::Path::new(self.project_path.as_deref().unwrap_or("project"))
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".into()),
            ExportTarget::Component(cid) => self.project.components.iter().find(|c| c.id == cid).map(|c| crate::i18n::name(&c.name)).unwrap_or_else(|| crate::i18n::tr("io-part-lower")),
        }
    }


    /// Exporting a target to STEP (exact B-rep, live `Shape`s from the core, every body in the world frame
    /// of the assembly).
    pub(super) fn export_step(&mut self, target: ExportTarget) {
        self.ensure_brep(); // without a live B-rep the sort would count every body as B-rep-less and the file would come out empty
        // the list of bodies and the world transforms are computed on the UI thread (self is needed)
        let plan = self.export_plan(target); // the same sort STL uses
        let note = plan.note(true);
        let items: Vec<(Id, [f64; 12])> = plan.brep.iter().map(|&b| (b, self.project.body_world_transform(b))).collect();
        if items.is_empty() {
            self.status = if plan.mesh_only.len() + plan.stale.len() > 0 {
                format!("{} {}{}", ph::WARNING, crate::i18n::tr("io-step-no-brep"), note)
            } else {
                crate::i18n::tr("io-step-no-bodies")
            };
            return;
        }
        let default = format!("{}.step", self.export_base_name(target));
        let Some(path) = rfd::FileDialog::new().set_file_name(default).add_filter("STEP", &["step", "stp"]).save_file() else { return };
        let p = path.to_string_lossy().into_owned();
        // writing the STEP goes to a worker. The `Shape`s (which are not Clone) are moved into the thread
        // TEMPORARILY and returned to the cache when it finishes. While the export runs the overlay is modal
        // (no edits are possible), so losing the cache is ruled out.
        let mut moved: Vec<(Id, qymcad_kernel::Shape, [f64; 12])> = Vec::with_capacity(items.len());
        for (id, m) in items {
            if let Some(s) = self.live.shapes.remove(&id) {
                moved.push((id, s, m));
            }
        }
        let n = moved.len();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let pairs: Vec<(&qymcad_kernel::Shape, [f64; 12])> = moved.iter().map(|(_, s, m)| (s, *m)).collect();
            // honest about what was skipped: a body with no live B-rep (an imported STL, a failed regen)
            // does not get into the STEP - such a file used to come out short of parts SILENTLY, and that
            // was discovered only by whoever received it.
            let status = match qymcad_kernel::write_step(&pairs, &p) {
                Ok(()) if !note.is_empty() => format!("(!) {}{}", crate::i18n::tr2("io-step-done", "n", &n.to_string(), "path", &p), note),
                Ok(()) => crate::i18n::tr2("io-step-done", "n", &n.to_string(), "path", &p),
                Err(e) => crate::i18n::name(&e),
            };
            drop(pairs);
            let shapes_back = moved.into_iter().map(|(id, s, _)| (id, s)).collect();
            let _ = tx.send(JobResult::Exported { status, shapes_back });
        });
        self.regen.busy = Some(Busy { label: crate::i18n::tr("io-export-step"), rx, kind: BgKind::Save, pulse: None, quiet: false });
    }


    /// Exporting a target to STL at a given detail (deflection in mm). A live `Shape` is re-tessellated to
    /// that quality; imported bodies with no shape use the stored mesh. Every mesh is placed into the world
    /// frame of the assembly.
    pub(super) fn export_stl(&mut self, target: ExportTarget, deflection: f64) {
        self.ensure_brep(); // STL quality comes from re-tessellating the live B-rep - bring the cache up
        // split into bodies with a live shape (tessellated in the worker) and raw meshes (a data clone is Send)
        let plan = self.export_plan(target); // the same sort STEP uses
        let note = plan.note(false);
        let mut moved: Vec<(Id, qymcad_kernel::Shape, [f64; 12])> = Vec::new();
        let mut raw: Vec<(qymcad_core::geom::Mesh, [f64; 12])> = Vec::new();
        for b in plan.stl_bodies() {
            let m = self.project.body_world_transform(b);
            if self.live.shapes.contains_key(&b) {
                if let Some(s) = self.live.shapes.remove(&b) {
                    moved.push((b, s, m));
                }
            } else if let Some(i) = self.project.mesh_index(b) {
                raw.push((self.project.bodies[i].mesh.clone(), m));
            }
        }
        if moved.is_empty() && raw.is_empty() {
            self.status = crate::i18n::tr("io-stl-no-bodies");
            return;
        }
        let default = format!("{}.stl", self.export_base_name(target));
        let Some(path) = rfd::FileDialog::new().set_file_name(default).add_filter("STL", &["stl"]).save_file() else {
            // the dialog was cancelled - put the moved shapes back into the cache
            for (id, s, _) in moved {
                self.live.shapes.insert(id, s);
            }
            return;
        };
        let p = path.to_string_lossy().into_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut meshes: Vec<qymcad_core::geom::Mesh> = Vec::new();
            let mut failed = 0usize; // a body whose tessellation failed is NOT dropped silently but reported
            for (_, s, m) in &moved {
                if let Some((mut mesh, _)) = s.tessellate_merged(deflection) {
                    mesh.transform(m);
                    meshes.push(mesh);
                } else {
                    failed += 1;
                }
            }
            for (mut mesh, m) in raw {
                mesh.transform(&m);
                meshes.push(mesh);
            }
            // STL writes EVERYTHING that is on screen (B-rep plus meshes), but it must say that some of the
            // bodies have no B-rep: the same sort STEP uses, so that the contents of the two files do not
            // drift apart SILENTLY.
            let status = match qymcad_io::export_stl(&meshes, &p) {
                Ok(()) if failed > 0 => format!("(!) {}{}", crate::i18n::trn("io-stl-partial", &[("n", &meshes.len().to_string()), ("path", &p), ("failed", &failed.to_string())]), note),
                Ok(()) if !note.is_empty() => format!("(!) {}{}", crate::i18n::tr2("io-stl-done", "n", &meshes.len().to_string(), "path", &p), note),
                Ok(()) => crate::i18n::tr2("io-stl-done", "n", &meshes.len().to_string(), "path", &p),
                Err(e) => crate::i18n::name(&e),
            };
            let shapes_back = moved.into_iter().map(|(id, s, _)| (id, s)).collect();
            let _ = tx.send(JobResult::Exported { status, shapes_back });
        });
        self.regen.busy = Some(Busy { label: crate::i18n::tr("io-export-stl"), rx, kind: BgKind::Save, pulse: None, quiet: false });
    }


    /// Exporting a sketch to SVG or DXF (`dxf=true` -> DXF). Exact primitives, not a tessellation.
    pub(super) fn export_sketch(&mut self, si: usize, dxf: bool) {
        let edges = self.sketch_export_edges(si);
        if edges.is_empty() {
            self.status = crate::i18n::tr("io-export-empty-sketch");
            return;
        }
        let name = self.project.sketches.get(si).map(|s| crate::i18n::name(&s.name)).unwrap_or_else(|| crate::i18n::tr("io-sketch-lower"));
        let (ext, filter) = if dxf { ("dxf", "DXF") } else { ("svg", "SVG") };
        if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{name}.{ext}")).add_filter(filter, &[ext]).save_file() {
            let p = path.to_string_lossy();
            let res = if dxf { qymcad_io::export_dxf(&edges, &p) } else { qymcad_io::export_svg(&edges, &p) };
            match res {
                Ok(()) => self.status = format!("{filter} -> {}", path.display()),
                Err(e) => self.status = crate::i18n::name(&e),
            }
        }
    }


    /// The "Save as part" dialog window (metadata, category, preview). The write itself is `commit_save_part`.
    pub(super) fn save_part_window(&mut self, ctx: &egui::Context) {
        if self.parts.save.is_none() {
            return;
        }
        let mut open = true;
        let (mut do_save, mut cancel) = (false, false);
        egui::Window::new(format!("{} {}", ph::PACKAGE, crate::i18n::tr("io-save-as-part")))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let Some(d) = self.parts.save.as_mut() else { return }; // the window may have closed
                // the preview texture is loaded lazily
                if d.tex.is_none() {
                    if let Some(img) = &d.preview {
                        d.tex = Some(ctx.load_texture("save_part_thumb", img.clone(), egui::TextureOptions::LINEAR));
                    }
                }
                ui.horizontal(|ui| {
                    // the preview on the left
                    if let Some(t) = &d.tex {
                        ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(t.id(), egui::vec2(150.0, 150.0))).rounding(4.0));
                    } else {
                        ui.add_sized([150.0, 150.0], egui::Label::new(egui::RichText::new(format!("{}\n{}", ph::CUBE, crate::i18n::tr("io-no-preview"))).weak()));
                    }
                    ui.vertical(|ui| {
                        egui::Grid::new("save_part_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                            ui.label(&crate::i18n::tr("io-name"));
                            ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(240.0));
                            ui.end_row();
                            ui.label(&crate::i18n::tr("io-description"));
                            ui.add(egui::TextEdit::singleline(&mut d.description).desired_width(240.0).hint_text(&crate::i18n::tr("io-description-example")));
                            ui.end_row();
                            ui.label(&crate::i18n::tr("io-tags"));
                            ui.add(egui::TextEdit::singleline(&mut d.tags).desired_width(240.0).hint_text(&crate::i18n::tr("io-comma-separated")));
                            ui.end_row();
                            ui.label(&crate::i18n::tr("io-category"));
                            ui.add(egui::TextEdit::singleline(&mut d.category).desired_width(240.0).hint_text(&crate::i18n::tr("io-category-example")));
                            ui.end_row();
                        });
                    });
                });
                // a quick pick of the categories that already exist (folders of the user's library)
                if !d.known_cats.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(&crate::i18n::tr("io-existing")).weak().small());
                        let mut pick: Option<String> = None;
                        for c in &d.known_cats {
                            if ui.small_button(c.as_str()).clicked() {
                                pick = Some(c.clone());
                            }
                        }
                        if let Some(c) = pick {
                            d.category = c;
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let can_save = !d.name.trim().is_empty();
                    if ui.add_enabled(can_save, egui::Button::new(format!("{} {}", ph::FLOPPY_DISK, crate::i18n::tr("io-save")))).clicked() {
                        do_save = true;
                    }
                    if ui.button(&crate::i18n::tr("io-cancel")).clicked() {
                        cancel = true;
                    }
                    ui.label(egui::RichText::new(&crate::i18n::tr("io-to-my-parts")).weak().small());
                });
            });
        if do_save {
            match self.commit_save_part() {
                Ok(p) => self.status = crate::i18n::tr1("io-part-saved", "path", &p),
                Err(e) => {
                    self.status = crate::i18n::tr1("io-part-save-failed", "error", &e.to_string());
                    return; // the dialog stays open so it can be corrected
                }
            }
        }
        if do_save || cancel || !open {
            // closing the dialog - the preview texture is dropped NOT now (it was drawn in this frame) but
            // through the graveyard
            if let Some(d) = self.parts.save.take() {
                self.tex_graveyard.extend(d.tex);
            }
        }
    }


    pub(super) fn export_setup_sheet(&mut self) {
        let Some(verify) = &self.cam_job.verify else { return };
        let html = self.setup_sheet_html(verify);
        let default = format!("{}_setup.html", self.program_name());
        if let Some(path) = rfd::FileDialog::new().set_file_name(default).add_filter("HTML", &["html"]).save_file() {
            match std::fs::write(&path, html) {
                Ok(()) => self.status = crate::i18n::tr1("io-setup-sheet", "path", &path.display().to_string()),
                Err(e) => self.status = crate::i18n::tr1("io-write-error", "error", &crate::i18n::name(&e.to_string())),
            }
        }
    }


    /// Integrating finished STEP solids (from the worker) into the project - on the UI thread, and fast.
    /// Every solid becomes the base body of its own part (several of them become a subassembly). The live
    /// shape goes into the cache.
    pub(super) fn finish_step_import(&mut self, path: String, bodies: Vec<qymcad_kernel::Body>, shapes: Vec<qymcad_kernel::Shape>) {
        let mut shapes = shapes.into_iter();
        let nbodies = bodies.len();
        let tris: usize = bodies.iter().map(|(m, _)| m.tris.len()).sum();
        let source = self.embed_source(&path).unwrap_or(0);
        let base = Self::file_name(&path);
        let stem = std::path::Path::new(&base).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| base.clone());

        // add the mesh, the faces and the shape of every solid; collect (body, name, source, index)
        let mut solids: Vec<(Id, String, Id, u32)> = Vec::with_capacity(nbodies);
        for (k, (mesh, fs)) in bodies.into_iter().enumerate() {
            let bid = self.project.add_mesh(mesh);
            self.live.faces.insert(bid, fs.clone()); // a face cache keyed by body Id, for quick access
            if let Some(b) = self.project.bodies.last_mut() {
                b.faces = fs;
            }
            if let Some(s) = shapes.next() {
                self.live.shapes.insert(bid, s);
            }
            let name = if nbodies == 1 { stem.clone() } else { format!("{stem} {}", k + 1) };
            solids.push((bid, name, source, k as u32));
        }
        // create the parts or the subassembly in the active context (one tested topology operation of the core)
        let created = self.project.import_bodies_as_parts(solids, &stem);
        self.regenerate_all(); // re-tessellate the import nodes, and take faces and edges from the B-rep
        if let Some(cid) = created {
            if let Some(ci) = self.project.components.iter().position(|c| c.id == cid) {
                self.sel = Sel::Component(ci);
            }
        }
        self.dxf_path = Some(path);
        self.invalidate();
        self.view.initialized = false;
        self.cam.init = false;
        let what = if nbodies == 1 { crate::i18n::tr("io-part") } else { crate::i18n::tr1("io-subassembly-of", "n", &nbodies.to_string()) };
        self.status = crate::i18n::tr2("io-step-imported", "what", &what, "tris", &tris.to_string());
    }


    /// Move the rebuild of the timeline into a worker thread. It is a modal job: while it runs the window
    /// draws a spinner and input is blocked - otherwise edits would land on a stale copy of the project.
    /// The bytes of the embedded sources do NOT travel to the thread (tens of megabytes); they are put back
    /// in place.
    pub(super) fn spawn_regen(&mut self) {
        self.prune_dangling_features();
        let stamp = self.regen_doc_stamp();
        // THE SAME SELECTION THE SYNCHRONOUS BRANCH MAKES. This used to mark ALL of the parametrics without
        // discrimination - "the project has a named dimension, so anything could have moved". In a live
        // window a rebuild always goes through this branch, which meant a parametric project was rebuilt
        // whole on every round and immediately asked for the next one.
        self.mark_changed_params_dirty();
        let plan = self.project.regen_plan();
        let mut proj = self.project.clone_without_source_data();
        let shapes = std::mem::take(&mut self.live.shapes);
        // precision is a property of THE DOCUMENT, taken here: in the thread there is no project to ask
        let quality_k = self.project.geom_quality.deflection_k();
        let (tx, rx) = std::sync::mpsc::channel();
        let pulse = std::sync::Arc::new(crate::gui::RegenPulse::default());
        let watch = pulse.clone();
        std::thread::spawn(move || {
            let _gate = qymcad_kernel::kernel_gate();
            let kernel = OcctKernel { shapes: std::cell::RefCell::new(shapes), quality_k };
            let report = proj.regenerate_watched(&kernel, watch.as_ref());
            let shapes = kernel.shapes.into_inner().into_iter().collect::<Vec<_>>();
            let _ = tx.send(JobResult::Regenerated { stamp, project: Box::new(proj), shapes, built: report.built, errors: report.errors, cancelled: report.cancelled });
        });
        // WHAT EXACTLY IS BEING REBUILT IS ASKED IN ADVANCE, and how to announce it depends on the answer.
        //
        // Reported: a cut in a single part pops up a modal window and makes you wait. A modal window over a
        // pinpoint edit is precisely the trouble: the work runs in a thread, the document on screen is
        // intact, and the person is held. The rule is simple:
        //   * a thread inside the scope of the edit means the window IS REQUIRED: it takes seconds to cut,
        //     and without the window a person concludes the thread simply failed to appear;
        //   * the whole timeline being rebuilt also gets the window: it is long, and it must be named;
        //   * everything else is quiet, one status line.
        let quiet = !plan.heavy && plan.nodes.len() * 2 < plan.total.max(1);
        let label = if plan.heavy {
            // THE CAPTION ANSWERS THE PERSON'S QUESTION, NOT ITS OWN.
            //
            // Reported: a draft angle was changed, and a window came up saying the program was rebuilding a
            // thread - which reads as odd. Formally it is right: the thread sits further down the chain and
            // is rebuilt next. But the edit was to the draft, and "cutting a thread" answers something else.
            // Both facts are stated: how many nodes are being rebuilt, and that a slow thread is among them.
            crate::i18n::tr1("io-rebuilding-heavy-n", "n", &plan.nodes.len().to_string())
        } else if quiet {
            crate::i18n::tr1("io-rebuilding-quiet", "n", &plan.nodes.len().to_string())
        } else {
            crate::i18n::tr("io-rebuilding")
        };
        if quiet {
            self.status = label.clone();
        }
        self.regen.busy = Some(Busy { label, rx, kind: BgKind::Regen, pulse: Some(pulse), quiet });
    }


    /// AN IMPRINT OF WHAT THE REBUILD WAS COMPUTED FROM - to check whether its result has gone stale.
    ///
    /// THE FULL DOCUMENT KEY (`state_key`) USED TO BE HERE, and it was THE SAME mistake the planner made: it
    /// includes THE PLACEMENT - where the parts stand. Drag a part, and the imprint changes on every frame,
    /// the arriving result is declared stale and thrown away, and another rebuild is requested right after.
    /// The circle closes and does not open until the hand stops: the window blinks twenty times a second
    /// while the parts lag behind, as if on rubber bands.
    ///
    /// What must be asked for is exactly WHAT THE RESULT WAS COMPUTED FROM: recipes, sketches, parameters.
    /// Dragging a part is none of those, and there is no reason to discard finished work over it - the live
    /// placement is carried across by [`Project::take_placement_from`].
    pub(super) fn regen_doc_stamp(&self) -> u64 {
        self.project.rebuild_key()
    }

    /// Take the result of a background rebuild - IF it is still current.
    ///
    /// A rebuild is computed on a COPY of the document and replaces the document WHOLE. While it runs the
    /// frame stops before drawing and before input, so there seems to be nowhere for an edit to come from -
    /// but that protection rests on the order of calls in `update`, not on the copy-then-replace pairing
    /// itself. Let one edit path appear that goes around the lock, and a person's work disappears without a
    /// trace and without an error. So currency is checked explicitly: if the document has moved on, the
    /// result is stale, and it is dropped and rebuilt again.
    pub(super) fn finish_regen_checked(&mut self, stamp: u64, project: Project, shapes: Vec<(Id, qymcad_kernel::Shape)>, built: Vec<(Id, Vec<MeshFace>)>, errors: Vec<(Id, qymcad_core::errors::CoreError)>, cancelled: bool) {
        // STOPPED BY THE PERSON - THE RESULT IS DROPPED WHOLE.
        //
        // The report is incomplete by construction: half the timeline was simply never reached, and "this
        // feature failed to build" cannot be read out of it. The document stays what it was - it never
        // changed: the work was done on a copy. The live B-rep is taken back, otherwise the next operation
        // would be left without it.
        //
        // AND NOTHING IS STARTED AGAIN. The dirty marks on the nodes are still there, and the planner looks
        // at exactly those - without this flag the very next frame would launch the same rebuild that was
        // just stopped, and Cancel would turn into a blinking button.
        if cancelled {
            self.adopt_shapes(shapes);
            self.regen.paused = true;
            self.status = format!("{} {}", ph::WARNING, crate::i18n::tr("io-rebuild-cancelled"));
            return;
        }
        if self.regen_doc_stamp() != stamp {
            self.adopt_shapes(shapes); // the live B-rep had travelled to the thread - take it back
            self.status = crate::i18n::tr("io-doc-changed");
            self.mark_dirty_for_rebuild();
            return;
        }
        self.finish_regen(project, shapes, built, errors);
    }

    /// TAKE THE LIVE SHAPES BACK FROM THE THREAD - ADDING TO THE CACHE, NOT REPLACING IT.
    ///
    /// One door serves all three returns (the result accepted, cancelled, or rejected as stale): in all
    /// three the thread hands back the cache taken at DISPATCH, and in all three other geometry may have
    /// come up meanwhile - restoring imports from the embedded STEP runs on its own background path.
    /// Replacing the cache wholesale wiped it; see `finish_regen`, where the price of that mistake is
    /// written down.
    fn adopt_shapes(&mut self, shapes: Vec<(Id, qymcad_kernel::Shape)>) {
        for (body, shape) in shapes {
            self.live.shapes.insert(body, shape);
        }
    }

    /// Take the model rebuilt in the thread. The caches are laid out exactly as in `regenerate_now`.
    pub(super) fn finish_regen(&mut self, mut project: Project, shapes: Vec<(Id, qymcad_kernel::Shape)>, built: Vec<(Id, Vec<MeshFace>)>, errors: Vec<(Id, qymcad_core::errors::CoreError)>) {
        project.take_source_data_from(&mut self.project); // the source bytes stayed on the UI thread
        // THE PLACEMENT IS THE LIVE ONE, NOT THE ONE FROM THE SNAPSHOT. The rebuild took a copy of the
        // document into the thread and brings it back WHOLE; the geometry in it is fresh, but "where the
        // parts stand" is what it was at dispatch. While a part is being dragged those are different things,
        // and taking the other placement means undoing that motion. The clash used to be resolved by
        // dropping the whole result - hence both the endless blinking of the window and the rubber-band
        // joints.
        project.take_placement_from(&self.project);
        self.project = project;
        self.project.solve_joints(); // live placement, new geometry - reconcile them at once, in this same frame
        self.settle_params_seen(); // the rebuild ARRIVED - the values from IT are the ones now "seen"
        // LIVE SHAPES ARE ADDED, THEY DO NOT REPLACE THE CACHE WHOLE.
        //
        // THIS USED TO BE `self.live.shapes = shapes...`, AND IT WAS THE LATCH OF AN ENDLESS CIRCLE. A rebuild
        // TAKES the cache with it into the thread (`mem::take` at dispatch) and returns its own copy. If the
        // cache was empty at dispatch - and right after "Rebuild all" it is exactly empty - then the copy is
        // empty, and the return WIPED everything that had come up while the thread was computing: the
        // imports restored from the embedded STEP. The preparation then saw zero shapes again and asked for
        // another rebuild.
        //
        // A MEASUREMENT IN A LIVE WINDOW caught it word for word: "B-rep preparation started: 136 live
        // shapes" -> "rebuild result accepted" -> "B-rep preparation started: 0 live shapes". Without end.
        //
        // The same class as the placement just above: the copy from the thread is stale for EVERYTHING THE
        // THREAD DID NOT COMPUTE. It brings back its own and lays it on top; it does not touch anyone
        // else's.
        self.adopt_shapes(shapes);
        let imports: std::collections::HashSet<Id> = self
            .project
            .timeline
            .iter()
            .filter_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
                _ => None,
            })
            .collect();
        self.live.shapes.retain(|body, _| self.project.mesh_index(*body).is_some() || imports.contains(body));
        for (body, faces) in built {
            self.set_body_faces(body, faces);
        }
        self.status = match errors.first() {
            Some((_, e)) => crate::i18n::tr1("io-rebuild-error", "error", &crate::i18n::error_text(e)),
            None => format!("{} {}", ph::CHECK, crate::i18n::tr("io-ready")),
        };
        self.invalidate();
        // if this rebuild was the preparation of a live B-rep, then it HAS NOW HAPPENED, and the outcome is
        // drawn from its result (after invalidate: that is what moves the geometry revision).
        if let Some(was_clean) = self.live.wait.take() {
            self.settle_brep_wait(was_clean);
        }
    }
}

// BACKGROUND JOBS AND REBUILDING: polling the workers, restoring the live B-rep, and the rebuild of the
// timeline itself. All of this is about the life cycle of the document, not about the interface.
impl App {
    /// One tick of the asynchronous subsystem. Returns true when the frame has been consumed by the splash
    /// screen (`update` must return early - no ordinary UI is built and no input is handled while loading).
    pub(super) fn tick_async(&mut self, ctx: &egui::Context) -> bool {
        self.ensure_logo(ctx);
        self.regen.ui_running = true; // the window is alive -> a heavy rebuild goes to a thread
        // a rebuild was asked for - start it in a thread. While it runs the `busy` branch below draws a
        // spinner and refuses input: edits would land on a stale copy of the project.
        if self.regen.wanted && self.regen.busy.is_none() {
            self.regen.wanted = false;
            self.spawn_regen();
        }
        // THE SECOND AND LAST POINT OF THE PLANNER: whatever the system marked (a background job arriving,
        // a B-rep fetch, a change of context) is rebuilt here. The first point is closing a command.
        if self.regen.busy.is_none() && self.edits.open.is_none() {
            self.rebuild_if_dirty();
        }
        // 1) Loading the project at startup: it goes to a worker (the heavy STEP reparse), and from there
        //    the `busy` branch below carries it - the splash spinner turns and the window does not hang.
        if let Some(path) = self.io.startup.take() {
            self.spawn_project_load(path);
        }
        // THE SPLASH AT STARTUP IS UNCONDITIONAL, at least `SPLASH_MIN`. It must stand AFTER the startup
        // load has been launched: an early return BEFORE it kept the load from starting at all - `io.startup`
        // stayed non-empty, the "still loading" condition held forever, and the result was a white window
        // for good. The order here is not a matter of style but of working at all.
        if let Some(until) = self.waiting.splash_until {
            let waited = std::time::Instant::now() >= until;
            if self.regen.busy.is_some() {
                // BACKGROUND WORK IS RUNNING - RETURNING HERE IS NOT ALLOWED. Below there is a branch that
                // polls the job channel and draws the splash itself; an early return from here kept it from
                // polling the channel at all - the load NEVER finished and the spinner turned forever. This
                // is the second mistake of one kind: leaving a frame before the thing that moves the work.
            } else if waited {
                self.waiting.splash_until = None;
            } else {
                // there is nothing to load, so the greeting is simply held for its due time
                self.draw_splash(ctx, &crate::i18n::tr("io-starting"));
                ctx.request_repaint();
                return true;
            }
        }
        // 1b) BACKGROUND work with no overlay (fetching the B-rep of imports, saving) - the model is already
        //     on screen and fully interactive; we simply wait for the result and keep the status alive.
        if !self.regen.bg.is_empty() {
            let mut done: Vec<JobResult> = Vec::new();
            let mut lost = false;
            self.regen.bg.retain(|bg| match bg.rx.try_recv() {
                Ok(res) => {
                    done.push(res);
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    lost = true;
                    false
                }
            });
            for res in done {
                self.apply_job_result(res);
            }
            if lost {
                self.status = crate::i18n::tr("io-bg-interrupted");
            }
            if !self.regen.bg.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        // 2) A background operation (import or export): turn the spinner and poll the result channel.
        if let Some(busy) = &self.regen.busy {
            match busy.rx.try_recv() {
                Ok(res) => {
                    self.regen.busy = None;
                    self.apply_job_result(res);
                    return false;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    let label = busy.label.clone();
                    // A REBUILD does not hide the interface. Collapsing the window to a black screen with a
                    // spinner - the way startup does - is frightening: the model is gone and it is unclear
                    // what is happening. Everything is drawn as usual, a dimming and a spinner go on top, and
                    // input is muted: edits made during a rebuild would land on a stale copy of the project.
                    if busy.kind == BgKind::Regen && busy.quiet {
                        self.dim.spinner = true; // the only sign: the body on screen is out of date
                        // A PINPOINT EDIT GETS NO WINDOW. The thread is computing, the document on screen is
                        // intact, and work can go on: an edit made while it computes will not be lost - the
                        // stale result is rejected by the imprint check (`finish_regen_checked`) and the
                        // rebuild repeats.
                        ctx.request_repaint();
                        return false;
                    }
                    if busy.kind == BgKind::Regen {
                        // INPUT IS MUTED BY THE BARRIER IN THE OVERLAY ITSELF rather than by clearing events
                        // here: that was too late - egui collects the input state at the start of a pass.
                        self.dim.overlay_progress = busy.pulse.as_ref().map(|p| p.progress());
                        self.dim.overlay = Some(label);
                        ctx.request_repaint();
                        return false;
                    }
                    self.draw_splash(ctx, &label);
                    ctx.request_repaint();
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // the worker died without sending a result - drop `busy` rather than hang in the overlay
                    self.regen.busy = None;
                    // the B-rep preparation was waiting for exactly this result. Leave the wait in place and
                    // it sticks forever: until a restart, operations that need a live B-rep would silently
                    // answer that the body was not built. The wait is cleared - a new attempt is allowed.
                    self.live.wait = None;
                    self.status = crate::i18n::tr("io-op-interrupted");
                    return false;
                }
            }
        }
        false
    }

    /// BRING UP the cache of live B-rep when the project was opened from a bundle WITHOUT a rebuild (the
    /// geometry is shown at once, but the `Shape`s of feature bodies do not exist yet). The first operation
    /// that needs a real B-rep pays for the rebuild once - the same idea as resolving a lightweight
    /// document. Until a project has been opened the flag is set and this is a pure no-op.
    ///
    /// EDGES FOR ANCHORS GO INTO THE MODEL as soon as the live B-rep is up.
    ///
    /// The first fix here was incomplete: joints on edges worked in the session where they were made and
    /// died on the next OPENING - "anchor lost", travel 0.000 mm. The two sources of edges were reconciled
    /// only at the moment an anchor is PLACED (`ensure_model_edges` while picking) and not reconciled when
    /// the document is opened: `regen_edges` is filled by the post-pass of a rebuild, and imported bodies
    /// are not rebuilt at all - they have no timeline nodes.
    ///
    /// The core is asked only about the bodies that EDGE anchors and VERTEX anchors refer to: there are a
    /// handful of those, and going through all hundred-odd would be pointless.
    pub(super) fn fill_model_edges_for_anchors(&mut self) {
        use qymcad_core::feature::AnchorRef;
        let want: Vec<Id> = self
            .project
            .connectors
            .iter()
            .filter_map(|c| match c.anchor {
                AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => Some(b),
                _ => None,
            })
            .filter(|b| !self.project.regen_edges.contains_key(b) && self.live.shapes.contains_key(b))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for b in want {
            self.ensure_model_edges(b);
        }
    }

    pub(super) fn ensure_brep(&mut self) {
        if self.live.ready || self.live.wait.is_some() {
            return; // done, or an attempt is ALREADY running - a second launch would make the overlay blink
        }
        // THE CACHE OF LIVE SHAPES HAS GONE TO THE THREAD - THERE IS NOTHING TO JUDGE BY.
        //
        // A rebuild TAKES the cache for the duration of the computation (`mem::take` in `spawn_regen`):
        // otherwise the thread has nothing to work with. All that time the application has ZERO live shapes
        // - not because there are none, but because the thread holds them. The preparation, asked at that
        // moment, honestly answered that not a single body was up and ordered a rebuild on top of the
        // running one; the answer depended on whether a computation was in flight, and the "we have already
        // tried this" guard missed every other time.
        //
        // A MEASUREMENT IN A LIVE WINDOW, a document with 138 imports: "136 live shapes" and "0 live shapes"
        // alternated endlessly and the rebuild window blinked without stopping. Judging by what you do not
        // hold is not allowed - while the computation runs, the preparation stays silent.
        if matches!(&self.regen.busy, Some(b) if b.kind == BgKind::Regen) {
            return;
        }
        // building the B-rep is DERIVED work: the model does not change from it, the caches do. If the
        // project was clean (just opened) it must stay clean, otherwise closing asks whether to save after
        // nothing at all has been touched.
        let was_clean = !self.is_dirty();
        // retrying on EVERY call is pointless - if nothing changed since the last attempt, the result will
        // be the same. But declaring the cache ready is not allowed either: that would be a lie, and because
        // of it operations on imports silently failed with "body not built" right up to a restart.
        if self.live.tried_rev == Some(self.brep_input_key()) {
            return;
        }
        self.live.tried_rev = Some(self.brep_input_key()); // guards against re-entering from inside a rebuild
        // ONLY the nodes whose bodies have no live B-rep are rebuilt (their sources - also without shapes -
        // land on the same list, so the chain comes whole). A forced regen of the entire project here would
        // cost a full re-tessellation of 1170 imports for nothing.
        let missing: Vec<Id> = self
            .project
            .timeline
            .iter()
            .filter_map(|n| n.kind.body().map(|b| (n.id, b)))
            .filter(|(_, b)| !self.live.shapes.contains_key(b))
            .map(|(id, _)| id)
            .collect();
        if missing.is_empty() {
            self.live.ready = true;
            self.fill_model_edges_for_anchors(); // see above: otherwise edge anchors are dead after opening
            if was_clean {
                self.edits.saved_key = self.edit_key();
            }
            return;
        }
        self.status = crate::i18n::tr1("io-brep-prepare", "n", &missing.len().to_string());
        for n in &mut self.project.timeline {
            if missing.contains(&n.id) {
                n.dirty = true;
            }
        }
        let deferred = self.regen.ui_running; // in a live window the rebuild goes to a thread and comes back later
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        if deferred {
            // Reported: pressing "create sketch" made the rebuild window blink back and forth. The rebuild
            // HAS NOT RUN YET - it is queued. Marking "tried at revision N" now is not allowed: the rebuild,
            // once it arrives, moves the revision, the mark stops matching, and the next frame (and picking
            // a sketch plane calls us on EVERY frame) starts it all over - an endless loop. The outcome is
            // drawn by `settle_brep_wait` when the result gets here.
            self.live.wait = Some(was_clean);
            return;
        }
        self.settle_brep_wait(was_clean);
    }

    /// LEND THE KERNEL a live B-rep cache. The `shapes` cache is the only owner of the `Shape`s, so
    /// operations that need geometry (projecting an edge into a sketch) take it from here rather than
    /// building a second cache alongside: copies drift apart silently, and the projection then comes from
    /// the wrong part.
    pub(super) fn with_kernel<R>(&mut self, f: impl FnOnce(&mut Self, &dyn qymcad_core::feature::Kernel) -> R) -> R {
        let _gate = qymcad_kernel::kernel_gate();
        let kernel = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.live.shapes)), quality_k: self.project.geom_quality.deflection_k() };
        let out = f(self, &kernel);
        self.live.shapes = kernel.shapes.into_inner();
        out
    }

    pub(super) fn regenerate_now(&mut self) {
        self.prune_dangling_features(); // anti-ghost: on EVERY regen, orphan meshes and dangling features go
        // THE REBUILD GRAPH: a parameter may have changed (including a named sketch dimension - those are in
        // `param_map`) -> ONLY the features that refer to it are rebuilt. This used to mark ALL features
        // carrying expressions, and it fired on EVERY rebuild: any trifle dragged the whole parametrics of
        // the project through a recount.
        self.mark_changed_params_dirty();
        let _gate = qymcad_kernel::kernel_gate();
        let kernel = OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut self.live.shapes)), quality_k: self.project.geom_quality.deflection_k() };
        // HOW OFTEN THE GEOMETRIC FALLBACK FIRED is counted AROUND the rebuild. This is an event of a
        // different kind from rebinding a reference: there a name was found and moved, here no name was
        // found at all and the element was identified BY PLACE, by resemblance. Staying silent about it is
        // not allowed - that is exactly how a reference lands on a neighbouring face, and it is discovered
        // three operations later.
        let snaps_before = self.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let report = self.project.regenerate(&kernel);
        let snaps = self.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed).saturating_sub(snaps_before);
        self.live.shapes = kernel.shapes.into_inner();
        self.settle_params_seen(); // the rebuild HAPPENED - only now are the values "seen"
        // the cache of live B-rep must match what the project actually holds. A regen REMOVES the mesh of a
        // body that stopped building (a rollback, a suppression, a cascade of an error) - while its former
        // shape stayed in the cache as a ghost: a foreign volume in the counts, wasted memory, and the risk
        // of handing dead geometry outside. It is cleaned by whether a mesh exists in the project.
        // The exception is IMPORTED bodies. Their B-rep cannot be rebuilt from a recipe - only by parsing
        // the embedded STEP again (tens of seconds), so their shape is kept even while the body temporarily
        // does not build (a rollback or a suppression): move the rollback bar back, and the import is on
        // screen instantly.
        let imports: std::collections::HashSet<Id> = self
            .project
            .timeline
            .iter()
            .filter_map(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
                _ => None,
            })
            .collect();
        self.live.shapes.retain(|body, _| self.project.mesh_index(*body).is_some() || imports.contains(body));
        for (body, faces) in report.built {
            self.set_body_faces(body, faces); // both into the index-parallel `faces` and into the cache by body Id
        }
        // REBINDS OF GEOMETRY REFERENCES ARE VISIBLE. A lost reference used to latch onto a "similar" face
        // again on every rebuild, silently - now it is an event that gets announced.
        self.regen.rebinds = report.rebinds.clone();
        if let Some((_, e)) = report.errors.first() {
            // the error text is in the person's language: the core returned a CODE, the words are found here
            self.status = format!("{} {}", crate::i18n::tr("status-rebuild"), crate::i18n::error_text(e));
        } else if !report.rebinds.is_empty() {
            let first = &report.rebinds[0];
            self.status = if report.rebinds.len() == 1 {
                format!("{} {}", ph::LINK_BREAK, crate::i18n::tr1("io-rebound-one", "what", &first.what))
            } else {
                format!("{} {}", ph::LINK_BREAK, crate::i18n::tr2("io-rebound-many", "n", &report.rebinds.len().to_string(), "first", &first.what))
            };
        } else if snaps > 0 {
            self.status = format!("{} {}", ph::LINK_BREAK, crate::i18n::tr1("io-rebound-by-place", "n", &snaps.to_string()));
        }
        self.invalidate();
    }
}
