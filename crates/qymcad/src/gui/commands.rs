//! PART COMMANDS - starting them, their parameters, applying them.
//!
//! The single frame of a command (the bar on top, the expression popup at the geometry, picking references
//! by click, the preview, Enter/Esc) lives here rather than being smeared over the viewport.

#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_ui_state::{comp_array_kind};
#[allow(unused_imports)] // the checks reach it through this module, which is the address they know
pub(crate) use qymcad_ui_state::try_modify;
pub(crate) use qymcad_part::*;
use super::*;

impl App {
    /// A test facade: the checks apply a command the way the frame does, but they have no context to hand.
    #[cfg(test)]
    pub(crate) fn apply_feat_cmd(&mut self) {
        apply_feat_cmd(&mut self.part_ctx());
    }

    /// A test facade: the axis candidates, refreshed the way the command bar refreshes them.
    #[cfg(test)]
    pub(crate) fn refresh_axis_edges(&mut self) {
        refresh_axis_edges(&mut self.part_ctx());
    }

    /// A test facade: open the half-sketcher for picking a contour.
    #[cfg(test)]
    pub(crate) fn begin_contour_pick(&mut self, slot: ContourSlot, sid: Id) {
        begin_contour_pick(&mut self.part_ctx(), slot, sid);
    }








    /// An array as A COMMAND: pick a body, then the top bar (the count, the direction, the axis), then the
    /// STEP or ANGLE as an expression field AT THE GEOMETRY, then ghost previews of the copies, then Enter.
    /// 17 is linear (a grid), 18 is circular.
    pub(super) fn start_array_cmd(&mut self, cmd: u8) {
        self.cancel_all_tools(); // exclusivity: the array drops the previous tool and its picks
        start_array_cmd(&mut self.part_ctx(), cmd)
    }




    /// A primitive as A COMMAND: 10 box, 11 cylinder, 12 sphere, 13 cone, 14 torus, 15 prism. The sizes are
    /// expression fields at the geometry (the popup) plus a wireframe PREVIEW; Enter creates, Esc cancels.
    /// By default it sits at the origin; a click on a vertex, a datum point, a plane or a face PLACES it
    /// (orienting the base along the normal of the plane).
    pub(super) fn start_prim_cmd(&mut self, code: u8) {
        self.cancel_all_tools(); // exclusivity: a new tool drops ANY previous one, picks and modes alike
        start_prim_cmd(&mut self.part_ctx(), code)
    }




































    /// Start RE-PLACING a sketch: turn on picking a new plane by click in the viewport.
    pub(super) fn start_replace_sketch_plane(&mut self, si: usize) {
        self.cancel_all_tools();
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.tools.picking = Picking::ReplaceSketch(si);
        self.viewing.mode_3d = true; // planes and faces are picked in 3D
        self.chosen.sel = Sel::Sketch(si);
        self.status = crate::i18n::tr("msg-move-sketch");
    }


    /// Derive the workbench from the active context (rather than from a tab chosen by hand) and
    /// synchronise the model.
    pub(super) fn sync_workbench(&mut self) {
        use qymcad_core::feature::ComponentKind;
        qymcad_ui_state::ensure_active_path(&mut self.active_path, &mut self.project);
        let prev = self.workbench;
        self.workbench = if self.sketch_ses.editing.is_some() {
            Workbench::Sketch
        } else if self.project.component_kind(qymcad_ui_state::current_ctx_id(&self.active_path, &self.project)) == Some(ComponentKind::Assembly) {
            Workbench::Assembly
        } else {
            Workbench::Part
        };
        // changing the workbench drops the active tool - it must not leak from one workbench to another
        if self.workbench != prev {
            self.cancel_all_tools();
        }
        self.project.set_active_component(Some(qymcad_ui_state::current_ctx_id(&self.active_path, &self.project))); // new nodes go into the active context
        qymcad_ui_state::doc_touched_without_undo(&mut self.disk.edits, &self.project); // navigation changes the document but is not an undo step
    }
















    /// Apply the result of a background operation, on the UI thread.
    pub(super) fn apply_job_result(&mut self, res: JobResult) {
        match res {
            JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => self.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
            JobResult::StepImported { path, bodies, shapes } => self.finish_step_import(path, bodies, shapes),
            JobResult::StlImported { path, mesh, faces } => self.finish_stl_import(path, mesh, faces),
            JobResult::ProjectLoaded { path, project, shapes } => self.finish_project_load(path, *project, shapes),
            JobResult::Saved { path, autosave, error } => {
                // "clean" is set ONLY once the write has actually succeeded; if it failed, the project stays
                // dirty and closing the window honestly asks about the unsaved work.
                let (pending_save, pending_auto) = (self.disk.io.saved_key.take(), self.disk.io.autosave_key.take());
                let failed = error.is_some(); // the answer is needed below, while `error` goes into the message
                self.status = match (error, autosave) {
                    (Some(e), true) => format!("{} {}", ph::WARNING, crate::i18n::tr1("io-autosave-failed", "error", &e)),
                    (Some(e), false) => crate::i18n::tr1("io-save-error", "error", &e),
                    (None, true) => {
                        if let Some(k) = pending_auto {
                            self.disk.edits.autosave_key = k;
                        }
                        format!("{} {}", ph::CHECK, crate::i18n::tr1("io-autosaved", "time", &clock_hh_mm()))
                    }
                    (None, false) => {
                        if let Some(k) = pending_save {
                            self.disk.edits.saved_key = k;
                        }
                        let _ = std::fs::remove_file(qymcad_ui_state::autosave_path(&self.disk.project_path)); // the autosave is no longer needed
                        crate::i18n::tr1("io-project-saved", "path", &path)
                    }
                };
                // THE NAVIGATION THAT WAS WAITING FOR THE WRITE HAPPENS HERE. The answer was "save", and
                // then off to open another document: the write went through, so we go on. It failed - the
                // navigation is cancelled and the document stays where it is, rather than the edits being
                // carried away.
                if self.deferred.nav_after_save && !autosave {
                    self.deferred.nav_after_save = false;
                    if !failed {
                        self.disk.pending_nav = self.deferred.nav.take();
                    } else {
                        self.deferred.nav = None;
                    }
                }
                // another write may have been asked for while this one ran - start the deferred request
                if let Some((p, auto)) = self.disk.io.save_request.take() {
                    if auto {
                        self.disk.io.autosave_key = Some(qymcad_ui_state::edit_key(&self.draw_ctx()));
                    } else {
                        self.disk.io.saved_key = Some(qymcad_ui_state::edit_key(&self.draw_ctx()));
                    }
                    crate::gui::io_jobs::spawn_save(&mut self.disk.io, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, p, auto);
                }
            }
            JobResult::ImportShapes { shapes, regen } => {
                let was_clean = !qymcad_ui_state::is_dirty(&mut self.rebuild_ctx()); // fetching the B-rep is derived work, not an edit
                let n = shapes.len();
                for (body, s) in shapes {
                    self.live.shapes.insert(body, s);
                }
                if regen {
                    qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the planner does the counting - a file with no geometry now has something to rebuild from
                    self.status = crate::i18n::tr1("io-loaded-brep-rebuilt", "n", &n.to_string());
                } else {
                    self.status = crate::i18n::tr1("io-brep-restored-n", "n", &n.to_string());
                }
                qymcad_ui_state::invalidate(&mut self.regen);
                if was_clean {
                    self.disk.edits.saved_key = qymcad_ui_state::edit_key(&self.draw_ctx()); // the project was clean and stays that way
                }
            }
            JobResult::Exported { status, shapes_back } => {
                for (id, s) in shapes_back {
                    self.live.shapes.insert(id, s); // put the shapes moved into the worker back into the cache
                }
                self.status = status;
            }
            JobResult::Failed(e) => self.status = e,
        }
    }











    /// THE SINGLE entry point of a part command. 1 extrude, 3 revolve (from a sketch); 4 fillet, 5 chamfer
    /// (on edges); 6 shell, 7 hole (on a face).
    pub(super) fn start_feat_cmd(&mut self, cmd: u8) {
        self.cancel_all_tools(); // a new part or datum command CANCELS the previous tool or pick - never two at once
        // A CLEAN SLATE BEFORE THE CHECKS: the state of the previous command must not seep into the new one.
        // The command itself opens BELOW, and only if the checks passed (no contour means no command starts).
        let prev_3d = self.viewing.mode_3d;
        self.tools.cmd.close(&mut self.tools.armed);
        qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_of!(self)); // a clean slate: the picks of the previous command do not travel into the new one
        // AND NEITHER DOES THE GEOMETRY SELECTION. The tools that take neither edges nor faces (extrude, the
        // cuts, the arrays, the datums) never cleared them: the selection from the previous command stayed
        // there and - what matters - stayed HIGHLIGHTED. What looks selected is something the new command
        // will not take.
        self.tools.gsel.edges.clear();
        self.tools.gsel.faces.clear();
        self.tools.gsel.faces_body = None;
        self.tools.gsel.described = None;
        self.tools.cmd.prev_3d = prev_3d;
        self.params.boolean.edit = None; // the edit mode of the boolean is not held over into another command
        match cmd {
            1 | 3 => start_sketch_cmd(&mut self.part_ctx(), cmd),
            8 => start_sweep_cmd(&mut self.part_ctx()),
            9 => start_loft_cmd(&mut self.part_ctx()),
            4..=7 => start_body_cmd(&mut self.part_ctx(), cmd),
            23 => start_draft_cmd(&mut self.part_ctx()),
            25 => start_push_face_cmd(&mut self.part_ctx()),
            26 => start_remove_face_cmd(&mut self.part_ctx()),
            30 => start_face_copy_cmd(&mut self.part_ctx()),
            31 => start_surface_replace_cmd(&mut self.part_ctx()),
            32 => crate::gui::commands::start_patch_cmd(&mut self.part_ctx()),
            33 => start_stitch_cmd(&mut self.part_ctx()),
            34 => start_trim_cmd(&mut self.part_ctx()),
            27 => start_split_cmd(&mut self.part_ctx()),
            28 => start_thicken_cmd(&mut self.part_ctx()),
            29 => start_split_face_cmd(&mut self.part_ctx()),
            24 => start_thread_cmd(&mut self.part_ctx()),
            16 => start_mirror_cmd(&mut self.part_ctx()),
            17 | 18 => self.start_array_cmd(cmd),
            20..=22 => self.start_datum_cmd(cmd),
            _ => {}
        }
    }
























    /// A DATUM as A COMMAND: 20 plane, 21 point, 22 axis. The same frame as an extrude - the top bar plus
    /// fields at the geometry (the offset or the coordinates, as expressions) plus picking references by
    /// click plus a preview plus Enter/Esc.
    pub(super) fn start_datum_cmd(&mut self, code: u8) {
        self.cancel_all_tools(); // clears a stuck `pick_sketch_plane` and the like - otherwise, while placing
        // a datum axis, the picker of base planes is on screen and a click starts a sketch instead
        self.tools.cmd.open(&mut self.tools.armed, code, self.viewing.mode_3d); // a clean slate, then open
        
        self.viewing.mode_3d = true;
        self.side.datum.plane_pick = None;
        self.side.datum.axis_ref = None;
        self.side.datum.axis_hit = None;
        self.side.datum.axis_mode = 0;
        self.side.datum.axis_pts.clear();
        match code {
            20 => {
                self.tools.cmd.params = vec![CmdParam::new("f-offset", "dist", 10.0, -100000.0, 100000.0)];
                self.status = crate::i18n::tr("msg-plane-pick");
            }
            21 => {
                self.side.datum.pt_mode = 0;
                self.side.datum.pt_vert = None;
                self.tools.cmd.params = vec![CmdParam::new("X", "x", 0.0, -1e7, 1e7), CmdParam::new("Y", "y", 0.0, -1e7, 1e7), CmdParam::new("Z", "z", 0.0, -1e7, 1e7)];
                self.status = crate::i18n::tr("msg-point-cmd");
            }
            22 => {
                refresh_axis_edges(&mut self.part_ctx()); // the straight edges of ALL visible bodies, for picking an axis
                self.tools.cmd.params.clear();
                self.status = crate::i18n::tr("msg-axis-pick");
            }
            _ => {}
        }
    }














    /// A COMPONENT ARRAY (in an assembly): pick a part, set the count and the direction on top, the step or
    /// the angle in a field at the geometry, then Enter. `mode`: 1 linear, 2 circular.
    ///
    /// A copy here is AN INSTANCE rather than the part inserted again: its body associatively repeats the
    /// active body of the source, and the array drives the placement. Edit the part and every copy follows.
    pub(super) fn start_comp_array(&mut self, mode: u8) {
        let src = match self.chosen.sel {
            Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id),
            _ => qymcad_ui_state::selected_body(&self.project, &self.chosen.sel).and_then(|b| self.project.body_owner(b)),
        };
        let Some(src) = src.filter(|c| *c != self.project.root) else {
            self.status = crate::i18n::tr("msg-pick-part-first");
            return;
        };
        if self.project.active_body(src).is_none() {
            self.status = crate::i18n::tr("msg-part-no-body");
            return;
        }
        self.cancel_all_tools(); // exclusivity: the array drops the previous tool
        self.side.carr = CompArrayCmd { mode, src, dir: 0, axis: 2, edit: 0 };
        self.params.arr.count = if mode == 2 { 6 } else { 3 };
        self.params.arr.full = true;
        self.viewing.mode_3d = true;
        self.tools.cmd.params = if mode == 2 { vec![] } else { vec![CmdParam::new("f-pitch", "cstep", 30.0, 0.01, 100000.0)] };
        self.status = if mode == 2 {
            crate::i18n::tr("msg-comp-circ-array")
        } else {
            crate::i18n::tr("msg-comp-lin-array")
        };
    }

    /// Reopen an EXISTING component array for editing (a double click in the tree).
    pub(super) fn start_comp_array_edit(&mut self, pid: Id) {
        use qymcad_core::model::CompPatternKind;
        let Some(pat) = self.project.comp_patterns.iter().find(|p| p.id == pid).cloned() else { return };
        self.cancel_all_tools();
        let (mode, dir, axis, count, val) = match pat.kind {
            CompPatternKind::Linear { dir, step, count } => (1u8, axis_of(dir), 2u8, count, step),
            CompPatternKind::Circular { dir, angle, count, .. } => (2u8, 0u8, axis_of(dir), count, angle),
        };
        self.side.carr = CompArrayCmd { mode, src: pat.src, dir, axis, edit: pid };
        self.params.arr.count = count.max(1);
        self.viewing.mode_3d = true;
        if mode == 2 {
            self.params.arr.full = (val - 360.0).abs() < 0.1;
            self.tools.cmd.params = if self.params.arr.full { vec![] } else { vec![CmdParam::new("f-angle", "cangle", val, -3600.0, 3600.0)] };
        } else {
            self.tools.cmd.params = vec![CmdParam::new("f-pitch", "cstep", val, 0.01, 100000.0)];
        }
        self.status = crate::i18n::tr("msg-edit-comp-array");
    }















}

// THE FEATURE COMMAND: the frame-by-frame drag (`update_feat`), the length field at the arrow, the sweep
// preview. This is where they belong - next to opening, applying and cancelling a command.
impl App {



}

// COMMANDS: editing a datum, the boolean bar, numeric input at the body gizmo, modifying a sketch,
// deleting the selection. Their place is next to the rest of the command life cycle.
impl App {



    /// Carry out a confirmed deletion of a tree node. One set of cascading core methods, plus a resync.
    pub(super) fn execute_delete(&mut self, sel: Sel) {
        match sel {
            Sel::Feature(ti) => crate::gui::commands::delete_feature(&mut self.part_ctx(), ti),
            Sel::Mesh(mi) => crate::gui::commands::delete_body_mesh(&mut self.part_ctx(), mi),
            Sel::Contour(i) => qymcad_ui_state::delete_contour(&mut self.project, &mut self.regen, &mut self.chosen.sel, &mut self.viewing.view, i),
            Sel::Sketch(si) => {
                if let Some(s) = self.project.sketches.get(si) {
                    let sid = s.id;
                    crate::gui::commands::delete_sketch_full(&mut self.part_ctx(), sid);
                }
            }
            // DELETING A DATUM IS THE SAME KIND OF OPERATION as deleting a feature or a sketch, and must go
            // through the same boundary. Without it the edit went past `App::edit`: the guard reported the
            // document changed outside `App::edit`, and the undo step came out as a nameless "edit" picked up
            // after the fact by a snapshot. It was hit on a cut - the cutting plane and the feature deleted.
            Sel::Plane(i) => {
                if let Some(p) = self.project.planes.get(i) {
                    let pid = p.id;
                    qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-plane-delete"));
                    self.project.delete_plane(pid);
                    self.chosen.sel = Sel::None;
                    crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                    qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
                }
            }
            Sel::DatumPoint(i) => {
                if let Some(d) = self.project.datum_points.get(i) {
                    let did = d.id;
                    qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-delete-point"));
                    self.project.delete_datum_point(did);
                    self.chosen.sel = Sel::None;
                    crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                    qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
                }
            }
            Sel::DatumAxis(i) => {
                if let Some(d) = self.project.datum_axes.get(i) {
                    let did = d.id;
                    qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-axis-delete"));
                    self.project.delete_datum_axis(did);
                    self.chosen.sel = Sel::None;
                    crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                    qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
                }
            }
            Sel::Joint(jid) => {
                // delete the joint and any orphaned connectors, through the same core method the cross in the
                // list uses
                self.project.delete_joint(jid);
                self.chosen.sel = Sel::None;
                qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the planner does the counting
            }
            Sel::Component(ci) => {
                // a part or a subassembly WHOLE: one core method clears the bodies, sketches, datums,
                // connectors and joints of the subtree. Deleting the active context returns to the root.
                if let Some(c) = self.project.components.get(ci) {
                    let cid = c.id;
                    if self.active_path.contains(&cid) {
                        let root = self.project.ensure_root();
                        self.set_context_to(root);
                    }
                    // A COPY OF AN ARRAY IS NOT DELETED ON ITS OWN: its placement and shape are driven by the
                    // array, and a single deletion would grow back at the next rebuild. THE WHOLE array is
                    // deleted; the source, which is a part of one's own, stays.
                    match self.project.comp_pattern_of(cid).map(|p| (p.id, p.src == cid)) {
                        Some((pid, is_src)) => {
                            self.project.delete_comp_pattern(pid);
                            if is_src {
                                self.project.delete_component(cid); // the source was the target, so it goes too
                            }
                            self.status = if is_src { crate::i18n::tr("msg-part-and-array-deleted") } else { crate::i18n::tr("msg-comp-array-deleted") };
                        }
                        None => {
                            self.project.delete_component(cid);
                        }
                    }
                    self.chosen.sel = Sel::None;
                    crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                }
            }
            _ => {}
        }
        self.status = crate::i18n::tr("msg-deleted");
    }
}
