//! PART COMMANDS - starting them, their parameters, applying them.
//!
//! The single frame of a command (the bar on top, the expression popup at the geometry, picking references
//! by click, the preview, Enter/Esc) lives here rather than being smeared over the viewport.

#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_ui_state::{comp_array_kind};
#[allow(unused_imports)] // the checks reach it through this module, which is the address they know
pub(crate) use qymcad_ui_state::try_modify;
pub(crate) use qymcad_part::*;
#[cfg(test)]
use qymcad_ui_state::NotApplied;
use qymcad_ui_state::PatternKind;
use qymcad_ui_state::Notice;
use super::*;

impl App {
    /// A test facade: the checks apply a command the way the frame does, but they have no context to hand.
    #[cfg(test)]
    pub(crate) fn apply_feat_cmd(&mut self) -> Option<NotApplied> {
        apply_feat_cmd(&mut self.part_ctx()).err()
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

    /// Start RE-PLACING a sketch: turn on picking a new plane by click in the viewport.
    pub(super) fn start_replace_sketch_plane(&mut self, si: usize) {
        self.cancel_all_tools();
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.tools.picking = Picking::ReplaceSketch(si);
        self.viewing.mode_3d = true; // planes and faces are picked in 3D
        self.chosen.sel = Sel::Sketch(si);
        self.status = Notice::info(crate::i18n::tr("msg-move-sketch"));
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
        qymcad_ui_state::doc_touched_without_undo(&mut self.disk.edits, &self.project);
        // navigation changes the document but is not an undo step
    }

    /// Apply the result of a background operation, on the UI thread.
    pub(super) fn apply_job_result(&mut self, res: JobResult) {
        match res {
            JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => self.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
            JobResult::RegenFailed { stamp, shapes, why } => {
                if let Some(path) = crate::gui::io_jobs::finish_regen_failed(&mut self.rebuild_ctx(), stamp, shapes, why) {
                    self.active_path = path;
                }
            }
            JobResult::ExactImported { path, format, bodies, shapes, nodes } => crate::gui::import_scale::land_exact(&mut self.win_ctx(&mut Vec::new()), path, format, bodies, shapes, nodes),
            JobResult::DrawingRead { path, curves, note } => {
                self.arm_sketch_import(curves, &path);
                if !note.is_empty() {
                    self.status = Notice::warning(format!("{} {note}", self.status));
                }
            }
            JobResult::MeshImported { path, format, pieces } => crate::gui::import_scale::land_mesh(&mut self.win_ctx(&mut Vec::new()), path, format, pieces),
            JobResult::ProjectLoaded { path, project, shapes } => self.finish_project_load(path, *project, shapes),
            JobResult::Saved { path, autosave, error } => {
                // "clean" is set ONLY once the write has actually succeeded; if it failed, the project stays
                // dirty and closing the window honestly asks about the unsaved work.
                let (pending_save, pending_auto) = (self.disk.io.saved_key.take(), self.disk.io.autosave_key.take());
                let failed = error.is_some(); // the answer is needed below, while `error` goes into the message
                self.status = match (error, autosave) {
                    (Some(e), true) => Notice::error(crate::i18n::tr1("io-autosave-failed", "error", &e)),
                    (Some(e), false) => Notice::error(crate::i18n::tr1("io-save-error", "error", &e)),
                    (None, true) => {
                        if let Some(k) = pending_auto {
                            self.disk.edits.autosave_key = k;
                        }
                        Notice::info(format!("{} {}", ph::CHECK, crate::i18n::tr1("io-autosaved", "time", &clock_hh_mm())))
                    }
                    (None, false) => {
                        if let Some(k) = pending_save {
                            self.disk.edits.saved_key = k;
                        }
                        let _ = std::fs::remove_file(qymcad_ui_state::autosave_path(&self.disk.project_path)); // the autosave is no longer needed
                        Notice::info(crate::i18n::tr1("io-project-saved", "path", &path))
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
                    crate::gui::io_jobs::spawn_save(&mut self.disk, &mut self.live, &mut self.project, &mut self.regen, &mut self.status, p, auto);
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
                    self.status = Notice::info(crate::i18n::tr1("io-loaded-brep-rebuilt", "n", &n.to_string()));
                } else if n > 0 {
                    self.status = Notice::info(crate::i18n::tr1("io-brep-restored-n", "n", &n.to_string()));
                    // none raised: nothing to tell
                }
                qymcad_ui_state::invalidate(&mut self.regen);
                if was_clean {
                    self.disk.edits.saved_key = qymcad_ui_state::edit_key(&self.draw_ctx());
                    // the project was clean and stays that way
                }
            }
            JobResult::Exported { status, shapes_back } => {
                for (id, s) in shapes_back {
                    self.live.shapes.insert(id, s); // put the shapes moved into the worker back into the cache
                }
                self.status = status;
            }
            JobResult::Failed(e) => self.status = Notice::error(e),
        }
    }

    /// THE SINGLE entry point of a tool of a part: the tool in hand and its picks put down, then the tool taken.
    pub(super) fn start_feat_cmd(&mut self, tool: qymcad_ui_state::PartTool) {
        self.cancel_all_tools(); // a new part or datum command CANCELS the previous tool or pick - never two at once
        qymcad_part::start_tool(&mut self.part_ctx(), tool);
    }

    /// A COMPONENT ARRAY (in an assembly): pick a part, set the count and the direction on top, the step or
    /// the angle in a field at the geometry, then Enter. `mode`: linear or circular.
    ///
    /// A copy here is AN INSTANCE rather than the part inserted again: its body associatively repeats the
    /// active body of the source, and the array drives the placement. Edit the part and every copy follows.
    pub(super) fn start_comp_array(&mut self, mode: PatternKind) {
        let src = qymcad_part::comp_array_source(&mut self.part_ctx()).ok();
        self.cancel_all_tools(); // exclusivity: the array drops the previous tool
        qymcad_part::take_comp_array(&mut self.part_ctx(), mode, src);
    }

    /// Reopen an EXISTING component array for editing (a double click in the tree).
    pub(super) fn start_comp_array_edit(&mut self, pid: Id) {
        self.cancel_all_tools();
        crate::gui::commands::open_comp_array_edit(&mut self.part_ctx(), pid);
    }
}

// THE FEATURE COMMAND: the frame-by-frame drag (`update_feat`), the length field at the arrow, the sweep
// preview. This is where they belong - next to opening, applying and cancelling a command.
impl App {}

// COMMANDS: editing a datum, the boolean bar, numeric input at the body gizmo, modifying a sketch,
// deleting the selection. Their place is next to the rest of the command life cycle.
impl App {
    /// Carry out a confirmed deletion of a tree node. One set of cascading core methods, plus a resync.
    pub(super) fn execute_delete(&mut self, sel: Sel, dependents: bool) {
        match sel {
            Sel::Feature(ti) => crate::gui::commands::delete_feature(&mut self.part_ctx(), ti, dependents),
            Sel::Mesh(mi) => crate::gui::commands::delete_body_mesh(&mut self.part_ctx(), mi, dependents),
            Sel::Contour(i) => qymcad_ui_state::delete_contour(&mut self.project, &mut self.regen, &mut self.chosen.sel, &mut self.viewing.view, i),
            Sel::Sketch(si) => {
                if let Some(s) = self.project.sketches.get(si) {
                    let sid = s.id;
                    crate::gui::commands::delete_sketch_full(&mut self.part_ctx(), sid, dependents);
                }
            }
            // DELETING A DATUM IS THE SAME KIND OF OPERATION as deleting a feature or a sketch, and must go
            // through the same boundary. Without it the edit went past `App::edit`: the guard reported the
            // document changed outside `App::edit`, and the undo step came out as a nameless "edit" picked up
            // after the fact by a snapshot. It was hit on a cut - the cutting plane and the feature deleted.
            Sel::Plane(i) => delete_one(&mut self.part_ctx(), "status-plane-delete", |p| {
                p.planes.get(i).map(|x| x.id).is_some_and(|id| if dependents { p.delete_plane_with_dependents(id) } else { p.delete_plane(id) })
            }),
            Sel::DatumPoint(i) => delete_one(&mut self.part_ctx(), "status-delete-point", |p| p.datum_points.get(i).map(|x| x.id).is_some_and(|id| p.delete_datum_point(id))),
            Sel::DatumAxis(i) => delete_one(&mut self.part_ctx(), "status-axis-delete", |p| p.datum_axes.get(i).map(|x| x.id).is_some_and(|id| p.delete_datum_axis(id))),
            // the joint and any orphaned connectors, through the same core method the cross in the list uses
            Sel::Joint(jid) => delete_one(&mut self.part_ctx(), "status-delete-joint", |p| {
                p.delete_joint(jid);
                true
            }),
            Sel::Component(ci) => {
                // a part or a subassembly WHOLE: one core method clears the bodies, sketches, datums,
                // connectors and joints of the subtree. Deleting the active context returns to the root.
                if let Some(c) = self.project.components.get(ci) {
                    let cid = c.id;
                    qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-delete-component"));
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
                            self.status = Notice::info(if is_src { crate::i18n::tr("msg-part-and-array-deleted") } else { crate::i18n::tr("msg-comp-array-deleted") });
                        }
                        None => {
                            self.project.delete_component(cid);
                        }
                    }
                    self.chosen.sel = Sel::None;
                    crate::gui::commands::resync_after_topology_change(&mut self.part_ctx());
                    qymcad_ui_state::commit_edit(&mut self.rebuild_ctx());
                }
            }
            _ => {}
        }
        self.status = Notice::info(crate::i18n::tr("msg-deleted"));
    }
}

/// DELETE ONE THING OF THE DOCUMENT as one named step of undo: `del` finds it and deletes it, answering whether there was
/// anything to delete; the selection goes, and the document is brought in step with its new topology.
fn delete_one(pc: &mut qymcad_ui_state::PartCtx, name: &str, del: impl FnOnce(&mut qymcad_core::model::Project) -> bool) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, crate::i18n::tr(name));
    if del(&mut *pc.project) {
        *pc.sel = Sel::None;
        crate::gui::commands::resync_after_topology_change(pc);
    }
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// CARRY OUT WHAT THE PROPERTIES PANEL ASKED FOR, after it has drawn. It lives here rather than on `App`: the panel
/// hands over a typed request, and what each request does is a command of its own workbench.
pub(crate) fn do_props_asks(app: &mut crate::gui::App, asks: Vec<qymcad_ui_state::PropsAsk>) {
    use qymcad_ui_state::PropsAsk;
    for a in asks {
        match a {
            PropsAsk::EditFeature(id) => crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), id),
            PropsAsk::EditJoint(jid) => crate::gui::enter_joint_edit(&mut app.side.joint, &mut app.chosen.sel, &mut app.status, jid),
            PropsAsk::RescaleImport(id) => crate::gui::import_scale::rescale(&mut app.win_ctx(&mut Vec::new()), id),
            PropsAsk::SketchOnDatum(id) => {
                app.create_sketch_on(qymcad_core::feature::SketchPlane::Datum(id));
            }
            PropsAsk::JointPick => app.start_joint_pick(),
            PropsAsk::ConnPick => app.start_conn_pick(),
            PropsAsk::DeleteConnector(cid) => qymcad_assembly::delete_connector_asked(&mut app.joint_ctx(), cid),
            PropsAsk::RelationPick(j) => qymcad_assembly::relation_pick_click(&mut app.joint_ctx(), j),
            PropsAsk::SketchOnBasePlane(b) => {
                app.create_sketch_on(qymcad_core::feature::SketchPlane::World(b));
            }
            PropsAsk::EnterSketch(si) => app.enter_sketch_edit(si),
            PropsAsk::ExitContext => app.exit_context(),
            PropsAsk::SetContext(cid) => app.set_context_to(cid),
        }
    }
}
