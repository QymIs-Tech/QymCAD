//! PART COMMANDS - starting them, their parameters, applying them.
//!
//! The single frame of a command (the bar on top, the expression popup at the geometry, picking references
//! by click, the preview, Enter/Esc) lives here rather than being smeared over the viewport.

/// THE DESCRIPTOR OF A REFERENCE, IF IT IS A MANUAL PICK.
///
/// The highlight of a selected face in a command can show one particular face. A query of the sort "every
/// wall of this feature" gives it nothing to show - so it returns `None` rather than lying with a
/// highlight.
fn face_desc_of(r: &qymcad_core::refs::Ref) -> Option<u32> {
    match r.query {
        qymcad_core::refs::Query::Id(d) if d != 0 => Some(d),
        _ => None,
    }
}

use super::*;

impl App {
    /// Start RE-PLACING a sketch: turn on picking a new plane by click in the viewport.
    pub(super) fn start_replace_sketch_plane(&mut self, si: usize) {
        self.cancel_all_tools();
        if self.sketch_ses.editing.is_some() {
            self.finish_sketch_edit();
        }
        self.picking = Picking::ReplaceSketch(si);
        self.mode_3d = true; // planes and faces are picked in 3D
        self.sel = Sel::Sketch(si);
        self.status = crate::i18n::tr("msg-move-sketch");
    }


    /// Derive the workbench from the active context (rather than from a tab chosen by hand) and
    /// synchronise the model.
    pub(super) fn sync_workbench(&mut self) {
        use qymcad_core::feature::ComponentKind;
        self.ensure_active_path();
        let prev = self.workbench;
        self.workbench = if self.cam_mode {
            Workbench::Cam
        } else if self.sketch_ses.editing.is_some() {
            Workbench::Sketch
        } else if self.project.component_kind(self.current_ctx_id()) == Some(ComponentKind::Assembly) {
            Workbench::Assembly
        } else {
            Workbench::Part
        };
        // changing the workbench drops the active tool - it must not leak from one workbench to another
        if self.workbench != prev {
            self.cancel_all_tools();
        }
        self.project.set_active_component(Some(self.current_ctx_id())); // new nodes go into the active context
        self.doc_touched_without_undo(); // navigation changes the document but is not an undo step
    }


    pub(super) fn cmd_anchor_screen(&self, rect: Rect) -> Option<Pos2> {
        let basis = self.cam.basis();
        match self.cmd.kind {
            1 | 3 => {
                let (base, dir, h) = self.feat_cmd_axis()?;
                let dir = if self.cmd.kind == 1 && self.feat.flip { [-dir[0], -dir[1], -dir[2]] } else { dir };
                let p = if self.cmd.kind == 3 { base } else { [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h] };
                Some(self.project3(p, rect, &basis).0)
            }
            // EVERYTHING AIMED BY CLICKING A BODY KEEPS ITS POPUP BESIDE THE BODY.
            //
            // The fillet and the chamfer already did that, while the hole put its field right at the face
            // and the thread at the rim: that is, over what has to be aimed at next. The cuts had no anchor
            // at all - there was nowhere to show the offset field, although the command has that parameter.
            //
            // The shell, the draft, pushing and removing a face, thickening - all belong here. What helps
            // with aiming is not the popup but THE HANDLE: it sits on the geometry and covers nothing.
            4 | 5 | 6 | 7 | 17 | 18 | 23 | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 | 32 | 33 | 34 => {
                let b = self.stitch_parts.first().copied().or(self.trim.keep.map(|(b, _)| b)).or(self.gsel.faces_body).or_else(|| self.op_target_body())?;
                self.body_side_anchor(b, rect, &basis)
            }
            // datum plane (20): at the origin of the picked reference; point (21): at (x,y,z); axis (22): at
            // the origin of the reference or of the hand-typed one
            20 => {
                let p = self.datum.plane_pick.as_ref().and_then(|sp| self.mirror_plane_world(sp)).map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0]);
                Some(self.project3(p, rect, &basis).0)
            }
            21 => Some(self.project3([self.cmd_val("x"), self.cmd_val("y"), self.cmd_val("z")], rect, &basis).0),
            22 => {
                let p = if self.datum.axis_mode == 1 { [self.cmd_val("ox"), self.cmd_val("oy"), self.cmd_val("oz")] } else { self.datum.axis_ref.map(|(o, _)| o).unwrap_or([0.0, 0.0, 0.0]) };
                Some(self.project3(p, rect, &basis).0)
            }
            // primitives: the popup goes BESIDE - from the body while editing (like the chamfer), from the
            // placement point while creating (it used to sit at the centre of the primitive and covered it).
            10..=15 => {
                if self.cmd.edit.is_some() {
                    if let Some(b) = self.selected_body() {
                        if let Some(a) = self.body_side_anchor(b, rect, &basis) {
                            return Some(a);
                        }
                    }
                }
                let p = self.project3(self.prim.place.unwrap_or([0.0, 0.0, 0.0]), rect, &basis).0;
                Some(Pos2::new(p.x + 90.0, p.y - 20.0)) // beside the placement point, not over the primitive
            }
            _ => None,
        }
    }


    /// Whether the command is ready to apply (the required selection exists) - for highlighting the button
    /// and the field.
    pub(super) fn cmd_ready(&self) -> bool {
        if self.cmd.edit.is_some() {
            return true; // editing an existing feature - the input is already valid
        }
        match self.cmd.kind {
            1 | 3 => !self.gsel.profiles.is_empty() || self.cmd.sketch.map(|si| self.sketch_closed_contours(si).len() == 1).unwrap_or(false),
            8 => self.sweep.prof_sid != 0 && self.sweep.path_sid != 0, // sweep: a profile and a path
            9 => self.loft.sids.len() >= 2, // loft: at least two sections
            4 => self.op_target_body().is_some(),
            // chamfer: a body exists (a part has one body); the asymmetric modes need edges TO BE PICKED
            5 => self.op_target_body().is_some() && (self.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric || !self.gsel.edges.is_empty()),
            6 => !self.gsel.faces.is_empty(), // shell: a multiple selection of faces
            23 => !self.gsel.faces.is_empty() && self.draft.neutral != 0, // draft: faces plus the neutral face
            25 | 26 | 28 | 30 => !self.gsel.faces.is_empty(), // push, remove, thicken or copy a face
            // surface replace: BOTH the faces of the base AND the surface are needed - a node without either
            // makes no sense
            31 => !self.gsel.faces.is_empty() && self.repl_surface.is_some(),
            32 => self.gsel.edges.len() >= 2, // patch: a boundary of edges (one cannot define it)
            33 => self.stitch_parts.len() >= 2, // stitch: at least two sheets
            34 => self.trim.keep.is_some() && self.trim.tool.is_some(), // trim: what is cut and what cuts it
            24 => self.thread.edge != 0, // thread: the rim of a cylinder or a hole is picked

            // hole: "by face" needs a face picked; "by sketch" needs a body plus a sketch with isolated points
            7 if self.hole.mode == 1 => self.op_target_body().is_some() && self.hole.sketch.map(|sid| !self.project.sketch_isolated_points(sid).is_empty()).unwrap_or(false),
            7 => matches!(self.sel, Sel::Face(..)),
            10..=15 => true, // primitives: the sizes are in fields, no selection is needed
            16 => self.mirror.plane.is_some(), // mirror: a plane is picked by click
            27 | 29 => self.split.plane.is_some(), // splitting a body or dividing faces: a plane is picked
            17 | 18 => self.op_target_body().is_some(), // array: the part has a body (the options are in the bar)
            20 => self.datum.plane_pick.is_some(),  // datum plane: a reference is picked by click
            21 => self.datum.pt_mode == 0 || self.datum.pt_vert.is_some(), // coordinates in fields, or a vertex picked
            22 => self.datum.axis_mode == 1 || (self.datum.axis_mode == 2 && self.datum.axis_pts.len() == 2) || self.datum.axis_ref.is_some(),
            _ => false,
        }
    }


    /// The "what to pick" hint for the active command, while nothing has been picked yet.
    pub(super) fn cmd_hint(&self) -> String {
        match self.cmd.kind {
            1 | 3 => crate::i18n::tr("hint-closed-contour"),
            8 => crate::i18n::tr("hint-path-sketch"),
            9 => crate::i18n::tr("hint-loft-sections"),
            4 | 5 => crate::i18n::tr("hint-body-edges"),
            6 => crate::i18n::tr("hint-faces-thickness"),
            7 => crate::i18n::tr("hint-body-face"),
            24 => crate::i18n::tr("hint-cyl-or-hole"),
            23 => crate::i18n::tr("hint-draft-faces"),
            25 => crate::i18n::tr("hint-flat-face-offset"),
            30 => crate::i18n::tr("hint-face-copy"),
            32 => crate::i18n::tr("hint-patch"),
            33 => crate::i18n::tr("hint-stitch"),
            34 => crate::i18n::tr(if self.trim.keep.is_none() { "hint-trim" } else { "hint-trim-tool" }),
            31 => crate::i18n::tr(if self.gsel.faces.is_empty() { "hint-surface-replace" } else { "hint-surface-replace-pick" }),
            27 => crate::i18n::tr("hint-cut-plane"),
            29 => crate::i18n::tr("hint-split-face-plane"),
            28 => crate::i18n::tr("hint-face-thickness"),
            26 => crate::i18n::tr("hint-feature-faces"),
            16 => crate::i18n::tr("hint-mirror-plane"),
            17 => crate::i18n::tr("hint-count-dir-step"),
            18 => crate::i18n::tr("hint-count-axis-angle"),
            20 => crate::i18n::tr("hint-plane-offset"),
            22 => crate::i18n::tr("hint-edge-or-manual"),
            _ => String::new(),
        }
    }


    /// Turn on the array feature (op=1 linear, 2 circular): pick, then parameters, then Enter, with a
    /// preview.
    pub(super) fn start_pattern(&mut self, op: u8) {
        let cur = self.pat.op;
        self.exit_draw_tools(); // drops the other modes (move, click-op) and `pat.op`
        self.sel_sk.modify = None;
        self.pat.op = if cur == op { 0 } else { op };
        self.pat.edit = None;
        self.pat.center = None;
        self.status = if self.pat.op == 0 {
            crate::i18n::tr("msg-cancelled")
        } else if self.pat.op == 2 {
            crate::i18n::tr("msg-circ-array-sketch")
        } else {
            crate::i18n::tr("msg-lin-array-sketch")
        };
    }


    /// Turn on interactive moving (op=1), copying (op=2) or rotating (op=3): pick, then the base point or
    /// centre, then the target (for a rotation, the angle in the popup).
    pub(super) fn start_move_tool(&mut self, op: u8) {
        let cur = self.tool.move_op;
        self.exit_draw_tools();
        self.sel_sk.modify = None;
        self.tool.move_op = if cur == op { 0 } else { op };
        self.tool.move_base = None;
        let has_sel = self.sel_sk.items.iter().any(|(k, _)| *k == 1);
        let what = if op == 3 { crate::i18n::tr("f-rotation-centre") } else { crate::i18n::tr("f-base-point") };
        self.status = if self.tool.move_op == 0 {
            crate::i18n::tr("msg-cancelled")
        } else if has_sel {
            crate::i18n::tr1("cmd-click-what", "what", &what)
        } else {
            crate::i18n::tr1("cmd-select-then-click", "what", &what)
        };
    }


    /// THE TEST FACADES FOR THE TOOLS THAT HAVE NO COMMAND NUMBER - through the same handles the bar
    /// buttons use.
    #[cfg(test)]
    pub(crate) fn start_move_tool_for_test(&mut self, op: u8) {
        self.start_move_tool(op);
    }

    #[cfg(test)]
    pub(crate) fn start_pattern_for_test(&mut self, op: u8) {
        self.start_pattern(op);
    }

    /// Take the body boolean by its first body - the same thing the bar button does.
    #[cfg(test)]
    pub(crate) fn arm_boolean_for_test(&mut self) {
        self.boolean.pick = Some((1, 0));
    }

    /// Turn on the 3D measure tool - through the same handle the button uses.
    #[cfg(test)]
    pub(crate) fn arm_measure3d_for_test(&mut self) {
        self.m3.on = true;
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
                let (pending_save, pending_auto) = (self.io.saved_key.take(), self.io.autosave_key.take());
                let failed = error.is_some(); // the answer is needed below, while `error` goes into the message
                self.status = match (error, autosave) {
                    (Some(e), true) => format!("{} {}", ph::WARNING, crate::i18n::tr1("io-autosave-failed", "error", &e)),
                    (Some(e), false) => crate::i18n::tr1("io-save-error", "error", &e),
                    (None, true) => {
                        if let Some(k) = pending_auto {
                            self.edits.autosave_key = k;
                        }
                        format!("{} {}", ph::CHECK, crate::i18n::tr1("io-autosaved", "time", &chrono_free_time()))
                    }
                    (None, false) => {
                        if let Some(k) = pending_save {
                            self.edits.saved_key = k;
                        }
                        let _ = std::fs::remove_file(self.autosave_path()); // the autosave is no longer needed
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
                        self.pending_nav = self.deferred.nav.take();
                    } else {
                        self.deferred.nav = None;
                    }
                }
                // another write may have been asked for while this one ran - start the deferred request
                if let Some((p, auto)) = self.io.save_request.take() {
                    if auto {
                        self.io.autosave_key = Some(self.edit_key());
                    } else {
                        self.io.saved_key = Some(self.edit_key());
                    }
                    self.spawn_save(p, auto);
                }
            }
            JobResult::ImportShapes { shapes, regen } => {
                let was_clean = !self.is_dirty(); // fetching the B-rep is derived work, not an edit
                let n = shapes.len();
                for (body, s) in shapes {
                    self.live.shapes.insert(body, s);
                }
                if regen {
                    self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting - a file with no geometry now has something to rebuild from
                    self.status = crate::i18n::tr1("io-loaded-brep-rebuilt", "n", &n.to_string());
                } else {
                    self.status = crate::i18n::tr1("io-brep-restored-n", "n", &n.to_string());
                }
                self.invalidate();
                if was_clean {
                    self.edits.saved_key = self.edit_key(); // the project was clean and stays that way
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


    /// Start renaming timeline node `id` in place: remember the target, copy the name, ask for auto-focus.
    pub(super) fn start_rename(&mut self, id: Id) {
        let cur = self.project.timeline.iter().find(|n| n.id == id).map(|n| crate::i18n::name(&n.name)).unwrap_or_default();
        self.rename.target = Some(id);
        self.rename.sketch = None; // must not clash with renaming a sketch
        self.rename.buf = cur;
        self.rename.focus = true;
    }


    /// Start renaming a component, a datum or a body in place. The current name goes into the field, with
    /// auto-focus.
    pub(super) fn start_rename_node(&mut self, node: RenameNode, cur: String) {
        self.rename.node = Some(node);
        self.rename.target = None; // must not clash with renaming a feature or a sketch
        self.rename.sketch = None;
        self.rename.buf = cur;
        self.rename.focus = true;
    }


    /// Apply the accumulated drag of the component gizmo: `transform = accumulated * start` ->
    /// `set_component_transform` (cheap, with no rebuild of bodies). The start is pinned, so the motion is
    /// smooth; snapping comes from the panel or from Ctrl.
    pub(super) fn apply_comp_giz(&mut self) {
        self.begin_edit(&crate::i18n::tr("status-move-component")); // THE BOUNDARY OF AN OPERATION
        let Some((comp, _, _, _)) = self.comp_giz.drag else { return };
        if let Some(t) = self.comp_giz_accum(self.comp_giz.snap) {
            self.project.set_component_transform(comp, t);
            self.invalidate_placement(); // THE PLACEMENT moved; the shape of the bodies did not change
        }
            self.commit_edit();
    }


    /// Apply the drag: the joint parameter becomes start plus accumulated -> `solve_joints` (cheap, with no
    /// rebuild of bodies) -> invalidate.
    pub(super) fn apply_joint_giz(&mut self) {
        self.begin_edit(&crate::i18n::tr("status-edit-joint")); // THE BOUNDARY OF AN OPERATION
        let Some(dg) = self.joint.giz_drag else { return };
        let snap = self.comp_giz.snap;
        let Some((val, _)) = self.joint_giz_value(snap) else { return };
        if let Some(j) = self.project.joints.iter_mut().find(|x| x.id == dg.jid) {
            let val = j.clamp_slot(dg.slot as usize, val); // the gizmo stops at the limit
            // DRAGGING A GIZMO IS A DELIBERATE ACT, so it is A DRIVER. It used to write into the reading
            // field, and the part would then "by itself" return to where the previous solve had left it.
            j.drive[(dg.slot as usize).min(2)] = Some(val);
        }
        self.project.solve_joints();
        self.invalidate_placement(); // the joint placed the parts: THE PLACEMENT changed, not the shape
            self.commit_edit();
    }


    /// Apply the world transform `accum` to the body of mesh `mi`: if the body is already a Move feature,
    /// accumulate into its matrix (no chain of Move nodes); otherwise create a new Move for a B-rep, or
    /// shift a raw mesh directly.
    pub(super) fn apply_body_move(&mut self, mi: usize, accum: [f64; 12]) {
        self.begin_edit(&crate::i18n::tr("status-move-body")); // THE BOUNDARY OF AN OPERATION
        use qymcad_core::feature::FeatureKind;
        let Some(body) = self.project.mesh_id(mi) else { return };
        let is_move = self.project.timeline.iter().any(|n| n.id == body && matches!(n.kind, FeatureKind::Move { .. }));
        if is_move {
            if let Some(node) = self.project.timeline.iter_mut().find(|n| n.id == body) {
                if let FeatureKind::Move { mat, .. } = &mut node.kind {
                    *mat = compose12(&accum, mat); // accumulate on top of what is there
                    node.dirty = true;
                }
            }
            self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
            self.select_body(body);
        } else {
            self.move_body_at(mi, accum); // a B-rep gets a new Move feature (and selection); a raw mesh is shifted directly
        }
        self.after_placement_change(); // the source moved -> rebuild whoever consumes it
            self.commit_edit();
    }


    /// The current value of a command parameter, by key.
    pub(super) fn cmd_val(&self, key: &str) -> f64 {
        self.cmd.params.iter().find(|p| p.key == key).map(|p| p.val).unwrap_or(0.0)
    }

    /// WHICH WAY THE TOOL GROWS, in the word the model uses.
    ///
    /// The window holds this as two separate switches - the extent mode of the top bar and the flip of the
    /// gizmo - while the model holds one word of three. The translation lives here alone, so the preview and
    /// the rebuild cannot come to different answers about the same two switches.
    pub(super) fn cmd_reach(&self) -> qymcad_core::feature::Reach {
        use qymcad_core::feature::Reach;
        if self.cmd.extent.symmetric() {
            Reach::BothWays
        } else if self.feat.flip {
            Reach::Backward
        } else {
            Reach::Forward
        }
    }


    /// Whether every expression of the command's parameters is valid. An invalid one (empty or broken)
    /// leaves `p.val` at its OLD value; the command must not be applied then - otherwise a stale value would
    /// be applied silently. This gates Apply (both Enter and the buttons) and marks the field in the popup.
    pub(super) fn cmd_exprs_valid(&self) -> bool {
        let vars = self.project.param_map();
        self.cmd.params.iter().all(|p| qymcad_core::expr::eval(&p.txt, &vars).is_ok())
    }


    /// Synchronise the DIRECTION expression fields at the geometry: the second side (`down`) appears as a
    /// field in the popup at the geometry ONLY in the two-sided mode (`ExtentMode::TwoSided`) of an extrude
    /// or a cut, and is removed otherwise. The value is mirrored into `feat_down`, which the preview, the
    /// apply and the update all read. When a feature is reopened the seed takes the stored expression
    /// (`cmd_param_from`). The height is already a field - so every distance of a command is edited at the
    /// geometry rather than by drag values in the top bar.
    pub(super) fn sync_dir_cmd_params(&mut self) {
        if self.cmd.kind != 1 {
            return;
        }
        let want_down = self.cmd.extent.two_sided();
        let has_down = self.cmd.params.iter().any(|p| p.key == "down");
        if want_down && !has_down {
            let seed = self.cmd.down.max(0.1);
            let prm = match self.cmd.edit {
                Some(fid) => self.cmd_param_from(fid, "f-second-side", "down", seed, 0.1, 10000.0),
                None => CmdParam::new("f-second-side", "down", seed, 0.1, 10000.0),
            };
            self.cmd.params.push(prm); // after the height, so it becomes the second line of the popup
        } else if !want_down && has_down {
            self.cmd.params.retain(|p| p.key != "down");
        }
        if want_down {
            self.cmd.down = self.cmd_val("down"); // mirrored for the preview, the apply and the update
        }
    }


    /// Store the dimensions of a command on feature `body`: an expression stays parametric, a plain number
    /// removes the expression (this matters while editing, when a formula gets replaced by a number).
    pub(super) fn store_cmd_exprs(&mut self, body: Id) {
        for p in self.cmd.params.clone() {
            let t = p.txt.trim().to_string();
            if !t.is_empty() && t.parse::<f64>().is_err() {
                self.project.set_feat_dim(body, &p.key, t);
            } else {
                self.project.set_feat_dim(body, &p.key, String::new());
            }
        }
    }


    /// A command parameter taken from a feature dimension: the text is the expression if there is one, or
    /// the number; the value is evaluated.
    pub(super) fn cmd_param_from(&self, fid: Id, label: &'static str, key: &str, num: f64, lo: f64, hi: f64) -> CmdParam {
        // A NUMBER IN A FIELD GOES THROUGH THE COMMON DOOR. This used to be `{num:.2}`: two decimals in one
        // field, three in another, the whole truth about an f64 in a third. There is one rule for the whole
        // project.
        let txt = self.project.feat_dim(fid, key).map(|s| s.to_string()).unwrap_or_else(|| qymcad_core::expr::fmt_num(num));
        let val = qymcad_core::expr::eval(&txt, &self.project.param_map()).unwrap_or(num);
        let mut p = CmdParam::new(label, key, num, lo, hi);
        p.val = val;
        p.txt = txt;
        p
    }


    /// THE SINGLE entry point of a part command. 1 extrude, 3 revolve (from a sketch); 4 fillet, 5 chamfer
    /// (on edges); 6 shell, 7 hole (on a face).
    pub(super) fn start_feat_cmd(&mut self, cmd: u8) {
        self.cancel_all_tools(); // a new part or datum command CANCELS the previous tool or pick - never two at once
        // A CLEAN SLATE BEFORE THE CHECKS: the state of the previous command must not seep into the new one.
        // The command itself opens BELOW, and only if the checks passed (no contour means no command starts).
        let prev_3d = self.mode_3d;
        self.cmd.close();
        self.clear_feat_picks(); // a clean slate: the picks of the previous command do not travel into the new one
        // AND NEITHER DOES THE GEOMETRY SELECTION. The tools that take neither edges nor faces (extrude, the
        // cuts, the arrays, the datums) never cleared them: the selection from the previous command stayed
        // there and - what matters - stayed HIGHLIGHTED. What looks selected is something the new command
        // will not take.
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.gsel.described = None;
        self.cmd.prev_3d = prev_3d;
        self.boolean.edit = None; // the edit mode of the boolean is not held over into another command
        match cmd {
            1 | 3 => self.start_sketch_cmd(cmd),
            8 => self.start_sweep_cmd(),
            9 => self.start_loft_cmd(),
            4 | 5 | 6 | 7 => self.start_body_cmd(cmd),
            23 => self.start_draft_cmd(),
            25 => self.start_push_face_cmd(),
            26 => self.start_remove_face_cmd(),
            30 => self.start_face_copy_cmd(),
            31 => self.start_surface_replace_cmd(),
            32 => self.start_patch_cmd(),
            33 => self.start_stitch_cmd(),
            34 => self.start_trim_cmd(),
            27 => self.start_split_cmd(),
            28 => self.start_thicken_cmd(),
            29 => self.start_split_face_cmd(),
            24 => self.start_thread_cmd(),
            16 => self.start_mirror_cmd(),
            17 | 18 => self.start_array_cmd(cmd),
            20 | 21 | 22 => self.start_datum_cmd(cmd),
            _ => {}
        }
    }


    /// A command driven by a sketch (extrude or revolve): pick a profile, set the size on the canvas, Enter.
    pub(super) fn start_sketch_cmd(&mut self, cmd: u8) {
        let si = match self.sel {
            Sel::Sketch(si) => Some(si),
            _ => self.cmd.sketch,
        };
        let Some(si) = si.filter(|&si| si < self.project.sketches.len()) else {
            self.status = crate::i18n::tr("msg-pick-sketch-first");
            return;
        };
        let closed = self.sketch_closed_contours(si);
        if closed.is_empty() {
            self.status = crate::i18n::tr("msg-no-closed-contour");
            return;
        }
        let was_3d = self.cmd.prev_3d;
        self.cmd.open(cmd, was_3d); // the checks passed - the command is open
        self.cmd.edit = None; // a new feature, not an edit
        if cmd == 3 {
            self.feat.op = 0; // revolve starts as "add" (for extrude the op is set by the tool button)
        }
        // CLEARING THE STALE STATE OF THE PREVIOUS COMMAND: "through", symmetry and the second side must NOT
        // travel silently into the new one (a through cut followed by an extrude in an empty part built a
        // 2000 mm body or broke silently). A new command means a clean one-sided extent.
        self.cmd.extent = ExtentMode::default();
        // a sensible default for the direction: a CUT on a face of the part goes INTO the body (the negative
        // normal), otherwise it would cut outwards into the void. Everything else goes outwards (the
        // positive normal); dragging the gizmo or pressing flip reverses it, and the preview equals the
        // result. The command opens with the direction still automatic - it is computed on apply.
        self.feat = FeatTarget::opened(self.feat.op);
        self.cmd.sketch = Some(si);
        self.gsel.profiles.clear();
        self.rev.axis_datum = 0; // by default the X or Y of the sketch
        self.rev.axis_line = 0; // no centre line is picked
        self.rev.pick_axis = false; // the axis-picking sub-mode is off
        self.rev.pick_line = false;
        self.cmd.params = match cmd {
            3 => vec![CmdParam::new("f-angle", "angle", self.rev.angle, 1.0, 360.0)],
            _ => vec![CmdParam::new("f-length", "height", self.set.defaults.extrude_h.max(0.1), 0.1, 10000.0)],
        };
        // ALL closed contours are selected - the sketch is extruded WHOLE, in one operation - so the 3D
        // preview and the gizmo appear at once. With two or more contours this used to fall back to a flat
        // pick by click, and extruding a sketch did not work.
        for &c in &closed {
            self.gsel.profiles.insert(c);
        }
        self.borrow_view(); // the view is borrowed for the command and returned on exit - no refitting
        self.mode_3d = true;
        let what = if cmd == 3 { crate::i18n::tr("f-revolve") } else { crate::i18n::tr("f-extrude") };
        self.status = crate::i18n::tr1("cmd-pick-contours-hint", "what", &what);
    }


    /// SWEEP - a profile (the selected sketch with a closed contour) along a path (a second sketch, picked
    /// in the tree). The profile usually lies on a plane at the start of the path, roughly perpendicular
    /// to it.
    pub(super) fn start_sweep_cmd(&mut self) {
        // the profile is the sketch selected in the tree (or the one captured earlier)
        let si = match self.sel {
            Sel::Sketch(si) => Some(si),
            _ => self.cmd.sketch,
        };
        let Some(si) = si.filter(|&si| si < self.project.sketches.len()) else {
            self.status = crate::i18n::tr("msg-sweep-pick-profile");
            return;
        };
        if self.sketch_closed_contours(si).is_empty() {
            self.status = crate::i18n::tr("msg-profile-no-contour");
            return;
        }
        self.cmd.open(8, self.mode_3d); // a clean slate, then open
        
        self.feat.op = 0; // a sweep starts as "add" (the op is chosen in the bar)
        self.cmd.sketch = Some(si);
        self.sweep.prof_sid = self.project.sketches[si].id;
        self.sweep.path_sid = 0;
        self.sweep.pick_path = true; // the path pick is expected straight away
        self.cmd.params.clear();
        self.borrow_view(); // the view is borrowed for the command and returned on exit - no refitting
        self.mode_3d = true;
        self.status = crate::i18n::tr("msg-sweep-pick-path");
    }


    /// LOFT (through sections) - a body through two or more sketch sections. The first section is the
    /// selected sketch; after that, clicking sketches in the tree adds sections in order. Every section must
    /// have a closed contour.
    pub(super) fn start_loft_cmd(&mut self) {
        let si = match self.sel {
            Sel::Sketch(si) => Some(si),
            _ => self.cmd.sketch,
        };
        let Some(si) = si.filter(|&si| si < self.project.sketches.len()) else {
            self.status = crate::i18n::tr("msg-loft-pick-first");
            return;
        };
        if self.sketch_closed_contours(si).is_empty() {
            self.status = crate::i18n::tr("msg-section-no-contour");
            return;
        }
        self.cmd.open(9, self.mode_3d); // a clean slate, then open
        
        self.cmd.sketch = Some(si);
        self.loft.sids = vec![self.project.sketches[si].id];
        self.loft.cids = vec![0];
        self.loft.ruled = false;
        self.loft.pick = true; // the next sections are expected to be added by clicking in the tree
        self.loft.pick_last = Some(self.project.sketches[si].id); // the first section is already in the set - no duplicate
        self.loft.result = 0; // by default a separate new body
        self.cmd.params.clear();
        self.borrow_view(); // the view is borrowed for the command and returned on exit - no refitting
        self.mode_3d = true;
        self.status = crate::i18n::tr("msg-loft-pick");
    }


    /// A primitive as A COMMAND: 10 box, 11 cylinder, 12 sphere, 13 cone, 14 torus, 15 prism. The sizes are
    /// expression fields at the geometry (the popup) plus a wireframe PREVIEW; Enter creates, Esc cancels.
    /// By default it sits at the origin; a click on a vertex, a datum point, a plane or a face PLACES it
    /// (orienting the base along the normal of the plane).
    pub(super) fn start_prim_cmd(&mut self, code: u8) {
        self.cancel_all_tools(); // exclusivity: a new tool drops ANY previous one, picks and modes alike
        self.cmd.focus = true;
        self.boolean.edit = None;
        self.cmd.open(code, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.prim.n = 6;
        self.prim.place = None;
        self.prim.frame = None;
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.gsel.edges.clear();
        // THE KEYS match those of the regen and of the feature properties (r/r1/r2/major/minor/dx/dy/dz/h -
        // RADII), otherwise the expressions would not bind (the regen reads `r`, not `dia`) and reopening
        // would go out of step.
        self.cmd.params = match code {
            10 => vec![CmdParam::new("f-length-x", "dx", 20.0, 0.1, 100000.0), CmdParam::new("f-width-y", "dy", 20.0, 0.1, 100000.0), CmdParam::new("f-height-z", "dz", 20.0, 0.1, 100000.0)],
            11 => vec![CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0), CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
            12 => vec![CmdParam::new("f-radius", "r", 10.0, 0.05, 100000.0)],
            13 => vec![CmdParam::new("f-radius-bottom", "r1", 10.0, 0.0, 100000.0), CmdParam::new("f-radius-top", "r2", 0.0, 0.0, 100000.0), CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
            14 => vec![CmdParam::new("f-ring-r", "major", 12.0, 0.1, 100000.0), CmdParam::new("f-tube-r", "minor", 4.0, 0.1, 100000.0)],
            15 => vec![CmdParam::new("f-radius-circ", "r", 10.0, 0.1, 100000.0), CmdParam::new("f-height", "h", 20.0, 0.1, 100000.0)],
            _ => vec![],
        };
        self.status = crate::i18n::tr("msg-primitive");
    }


    /// Create a primitive from the command's sizes. A diameter becomes a radius by halving. If an anchor
    /// point is set, a Move into it is added. Returns the Id of the visible body.
    pub(super) fn apply_prim_cmd(&mut self) -> Option<Id> {
        let v = |s: &Self, k: &str| s.cmd_val(k);
        let body = match self.cmd.kind {
            10 => self.project.add_box(v(self, "dx"), v(self, "dy"), v(self, "dz")),
            11 => self.project.add_cylinder(v(self, "r"), v(self, "h")),
            12 => self.project.add_sphere(v(self, "r")),
            13 => self.project.add_cone(v(self, "r1"), v(self, "r2"), v(self, "h")),
            14 => self.project.add_torus(v(self, "major"), v(self, "minor")),
            15 => self.project.add_prism(v(self, "r"), self.prim.n.max(3), v(self, "h")),
            _ => return None,
        };
        self.store_cmd_exprs(body);
        // placement: an oriented move of the primitive into the picked frame (the base sits on the surface)
        let placed = match self.prim.frame {
            Some(m) if !is_identity12(&m) => self.project.add_move(body, m),
            _ => body,
        };
        // A PART IS ONE BODY - the primitive is merged into the single body of the part (the first one seeds it)
        Some(self.project.finish_base_body(placed, 1))
    }


    /// Mirror as A COMMAND: pick a body, then the top bar (keep the original or not), then PICK BY CLICK the
    /// mirror plane, datum or face in the viewport (as when picking a sketch plane), then the preview, then
    /// Enter.
    pub(super) fn start_mirror_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(16, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.opts.mirror_keep = true;
        self.mirror.plane = None;
        self.cmd.params.clear();
        self.status = crate::i18n::tr("msg-mirror-pick");
    }


    /// Create the mirror from the plane picked by the command. Associative.
    pub(super) fn apply_mirror_cmd(&mut self) -> Option<Id> {
        let src = self.op_target_body()?;
        let Some(sp) = self.mirror.plane.clone() else {
            self.status = crate::i18n::tr("msg-click-mirror-plane");
            return None;
        };
        let (plane, datum) = self.resolve_mirror_plane(sp);
        Some(self.project.add_mirror(src, plane, self.opts.mirror_keep, datum))
    }


    /// An array as A COMMAND: pick a body, then the top bar (the count, the direction, the axis), then the
    /// STEP or ANGLE as an expression field AT THE GEOMETRY, then ghost previews of the copies, then Enter.
    /// 17 is linear (a grid), 18 is circular.
    pub(super) fn start_array_cmd(&mut self, cmd: u8) {
        self.cancel_all_tools(); // exclusivity: the array drops the previous tool and its picks
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(cmd, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.arr.count = if cmd == 18 { 6 } else { 3 };
        self.arr.dir = 0;
        self.arr.two = false;
        self.arr.count2 = 2;
        self.arr.dir2 = 1;
        self.arr.three = false;
        self.arr.count3 = 2;
        self.arr.dir3 = 2;
        self.arr.axis = 0;
        self.arr.full = true;
        self.arr.axis_pick = false;
        // a full circular array asks no angle (the sync adds the field once "full" is switched off)
        self.cmd.params = if cmd == 18 { vec![] } else { vec![CmdParam::new("f-pitch", "step", 25.0, 0.01, 100000.0)] };
        self.status = if cmd == 18 {
            crate::i18n::tr("msg-circ-array")
        } else {
            crate::i18n::tr("msg-lin-array")
        };
    }


    /// Synchronise the array fields at the geometry with the options of the bar: a second step for a linear
    /// array with a second direction, and an angle for a circular one that is NOT a full circle (the same way
    /// `sync_dir_cmd_params` handles the second side of an extrude).
    pub(super) fn sync_array_params(&mut self) {
        match self.cmd.kind {
            17 => {
                let has2 = self.cmd.params.iter().any(|p| p.key == "step2");
                if self.arr.two && !has2 {
                    let prm = match self.cmd.edit {
                        Some(fid) => self.cmd_param_from(fid, "f-pitch2", "step2", 25.0, 0.01, 100000.0),
                        None => CmdParam::new("f-pitch2", "step2", 25.0, 0.01, 100000.0),
                    };
                    self.cmd.params.push(prm);
                } else if !self.arr.two && has2 {
                    self.cmd.params.retain(|p| p.key != "step2");
                }
                let has3 = self.cmd.params.iter().any(|p| p.key == "step3");
                if self.arr.two && self.arr.three && !has3 {
                    let prm = match self.cmd.edit {
                        Some(fid) => self.cmd_param_from(fid, "f-pitch3", "step3", 25.0, 0.01, 100000.0),
                        None => CmdParam::new("f-pitch3", "step3", 25.0, 0.01, 100000.0),
                    };
                    self.cmd.params.push(prm);
                } else if (!self.arr.three || !self.arr.two) && has3 {
                    self.cmd.params.retain(|p| p.key != "step3");
                }
            }
            18 => {
                let has_ang = self.cmd.params.iter().any(|p| p.key == "angle");
                if !self.arr.full && !has_ang {
                    let prm = match self.cmd.edit {
                        Some(fid) => self.cmd_param_from(fid, "f-angle", "angle", 360.0, 1.0, 360.0),
                        None => CmdParam::new("f-angle", "angle", 360.0, 1.0, 360.0),
                    };
                    self.cmd.params.push(prm);
                } else if self.arr.full && has_ang {
                    self.cmd.params.retain(|p| p.key != "angle");
                }
            }
            _ => {}
        }
    }


    /// The raw text of a command field (an expression or a number), by key.
    pub(super) fn cmd_txt(&self, key: &str) -> String {
        self.cmd.params.iter().find(|p| p.key == key).map(|p| p.txt.clone()).unwrap_or_default()
    }


    /// Create the array from the options picked by the command. Associative; the step and the angle stay
    /// parametric.
    pub(super) fn apply_array_cmd(&mut self) -> Option<Id> {
        let src = self.op_target_body()?;
        if self.cmd.kind == 18 {
            let angle = if self.arr.full { 360.0 } else { self.cmd_val("angle") };
            let body = self.project.add_circular_array_axis(src, self.arr.count.max(1), angle, self.arr.axis);
            if self.arr.full {
                self.project.set_feat_dim(body, "angle", String::new());
            } else {
                self.store_cmd_exprs(body); // the `angle` key stays parametric (the regen reads the feature dims)
            }
            Some(body)
        } else {
            let (dx, dy, dz) = Self::arr_vec(self.arr.dir, self.cmd_val("step"));
            let (dx2, dy2, dz2, c2) = if self.arr.two {
                let (a, b, c) = Self::arr_vec(self.arr.dir2, self.cmd_val("step2"));
                (a, b, c, self.arr.count2.max(1))
            } else {
                (0.0, 0.0, 0.0, 1)
            };
            let (dx3, dy3, dz3, c3) = if self.arr.two && self.arr.three {
                let (a, b, c) = Self::arr_vec(self.arr.dir3, self.cmd_val("step3"));
                (a, b, c, self.arr.count3.max(1))
            } else {
                (0.0, 0.0, 0.0, 1)
            };
            let body = self.project.add_linear_array_grid3(src, dx, dy, dz, self.arr.count.max(1), dx2, dy2, dz2, c2, dx3, dy3, dz3, c3);
            self.store_cmd_exprs(body); // the logical `step`, `step2` and `step3` so that reopening returns the TEXT of the expression
            self.store_arr_component(body, ["dx", "dy", "dz"], self.arr.dir, self.cmd_txt("step"));
            if self.arr.two {
                self.store_arr_component(body, ["dx2", "dy2", "dz2"], self.arr.dir2, self.cmd_txt("step2"));
            }
            if self.arr.two && self.arr.three {
                self.store_arr_component(body, ["dx3", "dy3", "dz3"], self.arr.dir3, self.cmd_txt("step3"));
            }
            Some(body)
        }
    }


    /// A DATUM as A COMMAND: 20 plane, 21 point, 22 axis. The same frame as an extrude - the top bar plus
    /// fields at the geometry (the offset or the coordinates, as expressions) plus picking references by
    /// click plus a preview plus Enter/Esc.
    pub(super) fn start_datum_cmd(&mut self, code: u8) {
        self.cancel_all_tools(); // clears a stuck `pick_sketch_plane` and the like - otherwise, while placing
        // a datum axis, the picker of base planes is on screen and a click starts a sketch instead
        self.cmd.open(code, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.datum.plane_pick = None;
        self.datum.axis_ref = None;
        self.datum.axis_hit = None;
        self.datum.axis_mode = 0;
        self.datum.axis_pts.clear();
        match code {
            20 => {
                self.cmd.params = vec![CmdParam::new("f-offset", "dist", 10.0, -100000.0, 100000.0)];
                self.status = crate::i18n::tr("msg-plane-pick");
            }
            21 => {
                self.datum.pt_mode = 0;
                self.datum.pt_vert = None;
                self.cmd.params = vec![CmdParam::new("X", "x", 0.0, -1e7, 1e7), CmdParam::new("Y", "y", 0.0, -1e7, 1e7), CmdParam::new("Z", "z", 0.0, -1e7, 1e7)];
                self.status = crate::i18n::tr("msg-point-cmd");
            }
            22 => {
                self.refresh_axis_edges(); // the straight edges of ALL visible bodies, for picking an axis
                self.cmd.params.clear();
                self.status = crate::i18n::tr("msg-axis-pick");
            }
            _ => {}
        }
    }


    /// Synchronise the axis fields at the geometry with the mode (a click on an edge or a face against a
    /// hand-typed origin and direction).
    pub(super) fn sync_datum_axis_params(&mut self) {
        if self.cmd.kind != 22 {
            return;
        }
        let want_manual = self.datum.axis_mode == 1;
        let has_manual = self.cmd.params.iter().any(|p| p.key == "ox");
        if want_manual && !has_manual {
            self.cmd.params = vec![
                CmdParam::new("O.x", "ox", 0.0, -1e7, 1e7), CmdParam::new("O.y", "oy", 0.0, -1e7, 1e7), CmdParam::new("O.z", "oz", 0.0, -1e7, 1e7),
                CmdParam::new("Dir.x", "dx", 0.0, -1e7, 1e7), CmdParam::new("Dir.y", "dy", 0.0, -1e7, 1e7), CmdParam::new("Dir.z", "dz", 1.0, -1e7, 1e7),
            ];
        } else if !want_manual && has_manual {
            self.cmd.params.clear();
        }
    }


    /// A datum POINT: the X, Y and Z fields at the geometry in coordinate mode, removed in "at a vertex"
    /// mode.
    pub(super) fn sync_datum_point_params(&mut self) {
        if self.cmd.kind != 21 {
            return;
        }
        let want_coords = self.datum.pt_mode == 0;
        let has_coords = self.cmd.params.iter().any(|p| p.key == "x");
        if want_coords && !has_coords {
            self.cmd.params = vec![CmdParam::new("X", "x", 0.0, -1e7, 1e7), CmdParam::new("Y", "y", 0.0, -1e7, 1e7), CmdParam::new("Z", "z", 0.0, -1e7, 1e7)];
        } else if !want_coords && has_coords {
            self.cmd.params.clear();
        }
    }


    /// The diameter and depth fields of the RECESS (a counterbore or a countersink) at the geometry - they
    /// appear for any hole type other than the plain one.
    pub(super) fn sync_hole_params(&mut self) {
        if self.cmd.kind != 7 {
            return;
        }
        let want = self.hole.kind != 0;
        let has = self.cmd.params.iter().any(|p| p.key == "dia2");
        if want && !has {
            let (d2, dp2) = match self.cmd.edit {
                Some(fid) => (self.cmd_param_from(fid, "f-recess-d", "dia2", 12.0, 0.1, 10000.0), self.cmd_param_from(fid, "f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
                None => (CmdParam::new("f-recess-d", "dia2", 12.0, 0.1, 10000.0), CmdParam::new("f-recess-depth", "depth2", 4.0, 0.1, 10000.0)),
            };
            self.cmd.params.push(d2);
            self.cmd.params.push(dp2);
        } else if !want && has {
            self.cmd.params.retain(|p| p.key != "dia2" && p.key != "depth2");
        }
    }


    /// Apply the active datum command (Enter). Parametric: the offset and the coordinates are expressions
    /// stored as feature dimensions.
    pub(super) fn apply_datum_cmd(&mut self) -> Option<Id> {
        use qymcad_core::feature::SketchPlane;
        match self.cmd.kind {
            20 => {
                let dist = self.cmd_val("dist");
                let id = match self.datum.plane_pick.clone() {
                    Some(SketchPlane::World(bp)) => self.project.add_offset_plane(bp, dist),
                    Some(SketchPlane::Face(body, key)) => self.project.add_plane_from_face(body, key, dist),
                    Some(SketchPlane::Datum(did)) => self.project.add_offset_from_plane(did, dist), // parametric
                    None => {
                        self.status = crate::i18n::tr("msg-click-base-plane");
                        return None;
                    }
                };
                self.store_cmd_exprs(id); // the `dist` offset stays parametric in all three cases
                Some(id)
            }
            21 => {
                // "at a vertex" gives an associative point that travels with the vertex; otherwise the
                // coordinates, which stay parametric
                if self.datum.pt_mode == 1 {
                    let Some((body, edge, end, at)) = self.datum.pt_vert else {
                        self.status = crate::i18n::tr("msg-click-vertex");
                        return None;
                    };
                    return Some(self.project.add_point_at_vertex(at, body, edge, end));
                }
                let at = [self.cmd_val("x"), self.cmd_val("y"), self.cmd_val("z")];
                let id = self.project.add_point_at(at);
                self.store_cmd_exprs(id); // x, y and z stay parametric
                Some(id)
            }
            22 => {
                // mode 2, "two points": if BOTH are datum points the axis is a parametric TwoPoints;
                // otherwise it is a manual one built from the coordinates
                if self.datum.axis_mode == 2 {
                    if self.datum.axis_pts.len() < 2 {
                        self.status = crate::i18n::tr("msg-click-two-points");
                        return None;
                    }
                    let (p0, p1) = (self.datum.axis_pts[0], self.datum.axis_pts[1]);
                    if p0.0 != 0 && p1.0 != 0 {
                        return Some(self.project.add_axis_two_points(p0.0, p1.0)); // associative to the points
                    }
                    let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                    if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                        self.status = crate::i18n::tr("msg-points-coincide");
                        return None;
                    }
                    return Some(self.project.add_axis_manual(p0.1, d));
                }
                // mode 0, "by an edge or a face", gives an ASSOCIATIVE axis that travels with its source
                if self.datum.axis_mode == 0 {
                    return match self.datum.axis_hit {
                        Some(AxisHit::Edge(i)) => self.edges.axes.get(i).map(|&(body, edge, _)| self.project.add_axis_from_edge(body, edge)),
                        Some(AxisHit::Face(body, fid)) => Some(self.project.add_axis_from_face(body, fid)),
                        Some(AxisHit::Datum(_)) | None => match self.datum.axis_ref {
                            Some((o, d)) => Some(self.project.add_axis_manual(o, d)),
                            None => {
                                self.status = crate::i18n::tr("msg-click-edge-axis");
                                None
                            }
                        },
                    };
                }
                let (o, d) = ([self.cmd_val("ox"), self.cmd_val("oy"), self.cmd_val("oz")], [self.cmd_val("dx"), self.cmd_val("dy"), self.cmd_val("dz")]);
                if d[0].abs() + d[1].abs() + d[2].abs() < 1e-9 {
                    self.status = crate::i18n::tr("msg-axis-not-set");
                    return None;
                }
                Some(self.project.add_axis_manual(o, d))
            }
            _ => None,
        }
    }


    /// A command on a finished body (a fillet or a chamfer works on edges; a shell or a hole on a face).
    pub(super) fn start_body_cmd(&mut self, cmd: u8) {
        // a part is one body, so a command on a body works on THE SINGLE body of the part without clicking it
        // first. It starts if there is a body at all (either selected or the active one); it refuses only
        // when there is no body whatsoever.
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(cmd, self.mode_3d); // a clean slate, then open
         // a new feature, not an edit
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear(); // a shell picks faces from scratch
        self.gsel.faces_body = None; // the scope of the multiple face selection
        self.opts.shell_side = qymcad_core::feature::ShellSide::Inward; // a shell goes INWARDS by default
        self.chamfer.mode = qymcad_core::feature::ChamferMode::Symmetric; // a chamfer is symmetric by default
        self.chamfer.flip = false;
        self.chamfer.ref_face = 0; // the reference face is automatic until picked by hand
        self.chamfer.pick_ref = false;
        self.hole.kind = 0; // a hole is plain by default
        self.hole.mode = 0; // by default "on a face"
        self.hole.sketch = None;
        self.hole.flip = false;
        self.cmd.params = match cmd {
            // ONE field: the base radius. A variable fillet is set BY VERTICES: click a vertex of the
            // selection and it gets a field of its own. There is no second "radius 2" field any more: it
            // described a single edge with a direction and was incompatible with a selection in principle.
            4 => vec![CmdParam::new("f-radius", "radius", 2.0, 0.05, 1000.0)],
            // the leg d1 is always there; d2 is either the second leg (two distances) or the angle in
            // degrees (a leg plus an angle), enabled by the mode chosen in the top panel
            5 => vec![CmdParam::new("f-leg", "dist", 1.5, 0.05, 1000.0), CmdParam::new(Self::chamfer_d2_label(qymcad_core::feature::ChamferMode::Symmetric), "d2", 1.5, 0.0, 1000.0)],
            6 => vec![CmdParam::new("f-thickness", "thickness", 2.0, 0.1, 1000.0)],
            _ => vec![CmdParam::new("f-diameter", "diameter", 6.0, 0.1, 10000.0), CmdParam::new("f-depth", "depth", 15.0, 0.1, 10000.0)],
        };
        self.status = match cmd {
            4 => crate::i18n::tr("msg-fillet"),
            5 => crate::i18n::tr("msg-chamfer"),
            6 => crate::i18n::tr("msg-shell"),
            _ => crate::i18n::tr("msg-hole"),
        };
    }


    /// DRAFT: tilt the faces of a body relative to a neutral face. With the body selected, click the faces
    /// to tilt (a multiple selection) and pick the reference with the neutral-face button, then the angle,
    /// then Enter.
    ///
    /// PUSH A FACE - direct modelling: click a flat face, set the offset, Enter.
    ///
    /// The same command experience as everywhere: the bar on top, a field at the geometry (with an
    /// expression), the face picked by click, Enter/Esc. A positive offset adds material, a negative one
    /// cuts it away.
    pub(super) fn start_push_face_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(25, self.mode_3d);
        self.borrow_view(); // the view is borrowed and returned on exit
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear(); // the face is picked afresh: a pre-selection would only confuse here
        self.gsel.faces_body = None;
        self.cmd.params = vec![CmdParam::new("f-offset2", "dist", 5.0, -100000.0, 100000.0)];
        self.status = crate::i18n::tr("msg-push-face");
    }

    /// Apply "push a face": one selected face plus an offset.
    pub(super) fn apply_push_face_cmd(&mut self) -> Option<Id> {
        // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. When
        // the two diverged, the search for the face key failed and the tool SILENTLY did nothing: no node, no
        // error, no message - it reads as "pressed it, and nothing happened". Thicken was mended the same
        // way.
        let src = self.gsel.faces_body.or_else(|| self.op_target_body())?;
        let fid = self.gsel.faces.iter().copied().next();
        let Some(fid) = fid else {
            self.status = crate::i18n::tr("msg-click-face-push");
            return None;
        };
        // the face key is taken from the built body: the offset is later resolved BY NAME, not by index
        let key = self
            .project
            .mesh_index(src)
            .and_then(|mi| self.project.bodies.get(mi))
            .and_then(|b| b.faces.iter().find(|f| f.id == fid))
            .map(|f| qymcad_core::feature::FaceKey {
                index: 0,
                centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
                normal: f.normal,
                id: f.id,
            });
        // AND IF THE FACE WAS NOT FOUND AFTER ALL, SAY SO. A silent return from here was exactly that
        // "pressed it, and nothing": silence is indistinguishable from a broken program.
        let Some(key) = key else {
            self.status = crate::i18n::tr("msg-faces-not-found");
            return None;
        };
        let dist = self.cmd_val("dist");
        if dist.abs() < 1e-9 {
            self.status = crate::i18n::tr("msg-zero-offset");
            return None;
        }
        let body = self.project.add_push_face(src, key, dist);
        self.store_cmd_exprs(body); // the offset stays parametric
        Some(body)
    }

    /// A COMPONENT ARRAY (in an assembly): pick a part, set the count and the direction on top, the step or
    /// the angle in a field at the geometry, then Enter. `mode`: 1 linear, 2 circular.
    ///
    /// A copy here is AN INSTANCE rather than the part inserted again: its body associatively repeats the
    /// active body of the source, and the array drives the placement. Edit the part and every copy follows.
    pub(super) fn start_comp_array(&mut self, mode: u8) {
        let src = match self.sel {
            Sel::Component(ci) => self.project.components.get(ci).map(|c| c.id),
            _ => self.selected_body().and_then(|b| self.project.body_owner(b)),
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
        self.carr = CompArrayCmd { mode, src, dir: 0, axis: 2, edit: 0 };
        self.arr.count = if mode == 2 { 6 } else { 3 };
        self.arr.full = true;
        self.mode_3d = true;
        self.cmd.params = if mode == 2 { vec![] } else { vec![CmdParam::new("f-pitch", "cstep", 30.0, 0.01, 100000.0)] };
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
            CompPatternKind::Linear { dir, step, count } => (1u8, Self::axis_of(dir), 2u8, count, step),
            CompPatternKind::Circular { dir, angle, count, .. } => (2u8, 0u8, Self::axis_of(dir), count, angle),
        };
        self.carr = CompArrayCmd { mode, src: pat.src, dir, axis, edit: pid };
        self.arr.count = count.max(1);
        self.mode_3d = true;
        if mode == 2 {
            self.arr.full = (val - 360.0).abs() < 0.1;
            self.cmd.params = if self.arr.full { vec![] } else { vec![CmdParam::new("f-angle", "cangle", val, -3600.0, 3600.0)] };
        } else {
            self.cmd.params = vec![CmdParam::new("f-pitch", "cstep", val, 0.01, 100000.0)];
        }
        self.status = crate::i18n::tr("msg-edit-comp-array");
    }

    /// The axis (0 for X, 1 for Y, 2 for Z) from a vector - for reopening an array to edit it.
    fn axis_of(d: [f64; 3]) -> u8 {
        let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
        if az >= ax && az >= ay {
            2
        } else if ay >= ax {
            1
        } else {
            0
        }
    }

    /// The layout of the command turned into a description of the array.
    pub(super) fn comp_array_kind(&self) -> qymcad_core::model::CompPatternKind {
        use qymcad_core::model::CompPatternKind;
        let unit = |a: u8| match a {
            1 => [0.0, 1.0, 0.0],
            2 => [0.0, 0.0, 1.0],
            _ => [1.0, 0.0, 0.0],
        };
        let count = self.arr.count.max(1);
        if self.carr.mode == 2 {
            let angle = if self.arr.full { 360.0 } else { self.cmd_val("cangle") };
            // THE AXIS PASSES THROUGH THE ORIGIN OF THE ASSEMBLY: the array has no axis of its own yet, and
            // that is stated honestly - for bolts around a flange the part is placed relative to the origin
            // of the assembly, and the centre sits there too.
            CompPatternKind::Circular { origin: [0.0; 3], dir: unit(self.carr.axis), angle, count }
        } else {
            CompPatternKind::Linear { dir: unit(self.carr.dir), step: self.cmd_val("cstep"), count }
        }
    }

    /// Apply the component array (Enter): create a new one or update the one being edited.
    pub(super) fn apply_comp_array(&mut self) {
        let kind = self.comp_array_kind();
        self.begin_edit(if self.carr.edit != 0 { crate::i18n::tr("status-edit-comp-array") } else { crate::i18n::tr("f-comp-array") });
        let ok = if self.carr.edit != 0 {
            self.project.set_comp_pattern(self.carr.edit, kind)
        } else {
            self.project.add_comp_pattern(self.carr.src, kind) != 0
        };
        self.status = if ok { crate::i18n::tr1("cmd-comp-array-done", "n", &self.arr.count.max(1).to_string()) } else { crate::i18n::tr("msg-array-no-body") };
        self.carr = CompArrayCmd::default();
        self.cmd.params.clear();
        self.mark_dirty_for_rebuild();
        self.commit_edit();
    }

    /// SPLIT FACES: the same reference plane a body cut uses, but the body stays ONE - only the faces are
    /// divided. That is how an area is marked out for painting or machining without breaking the part
    /// apart.
    pub(super) fn start_split_face_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(29, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.split.plane = None;
        self.cmd.params = vec![CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
        self.status = crate::i18n::tr("msg-split-face");
    }

    /// Apply the division of faces.
    pub(super) fn apply_split_face_cmd(&mut self) -> Option<Id> {
        let src = self.op_target_body()?;
        if self.split.plane.is_none() {
            self.status = crate::i18n::tr("msg-click-plane");
            return None;
        }
        // WHAT WILL COME OUT IS CHECKED BEFORE THE FEATURE IS CREATED: a plane that misses the body divides
        // nothing, and a node in the timeline that is certain to go red is of no use to anyone.
        let (o, n) = self.split_plane_local(src)?;
        self.ensure_brep();
        if self.live.shapes.get(&src).and_then(|sh| sh.split_faces(o, n)).is_none() {
            self.status = crate::i18n::tr("msg-plane-splits-nothing");
            return None;
        }
        let sp = self.split.plane.clone()?;
        let (plane, datum) = self.resolve_mirror_plane(sp);
        let offset = self.cmd_val("offset");
        let body = self.project.add_split_face(src, plane, datum, offset);
        self.store_cmd_exprs(body);
        self.status = crate::i18n::tr("msg-faces-split");
        Some(body)
    }

    /// THICKEN: click a face, set the thickness at the geometry, and Enter grows the face into A PLATE.
    /// The plate is glued to the part, because a part is ONE body. As a separate body it was painted in a
    /// different colour, and one part came out on screen as two.
    pub(super) fn start_thicken_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(28, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.cmd.params = vec![CmdParam::new("f-thickness", "thickness", 2.0, -100000.0, 100000.0)];
        self.status = crate::i18n::tr("msg-thicken");
    }

    /// Apply the thickening: one selected face plus a thickness.
    pub(super) fn apply_thicken_cmd(&mut self) -> Option<Id> {
        // THE BODY IS TAKEN FROM THE FACE THAT WAS CLICKED rather than from the selection in the tree. Now
        // that surfaces exist, a sheet lives in the scene alongside the part, and it is the sheet that gets
        // thickened: with the part selected in the tree, the face of the sheet would be looked for on the
        // part and not found.
        let src = self.gsel.faces_body.or_else(|| self.op_target_body())?;
        let Some(fid) = self.gsel.faces.iter().copied().next() else {
            self.status = crate::i18n::tr("msg-click-face-thicken");
            return None;
        };
        let t = self.cmd_val("thickness");
        if t.abs() < 1e-9 {
            self.status = crate::i18n::tr("msg-zero-thickness");
            return None;
        }
        let body = self.project.add_thicken(src, fid, t);
        self.store_cmd_exprs(body); // the thickness stays parametric
        Some(body)
    }

    /// SPLIT A BODY: click a plane, a datum or a face, set the offset at the geometry, and Enter breaks the
    /// body into independent pieces.
    pub(super) fn start_split_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(27, self.mode_3d);
        self.borrow_view(); // the view is borrowed and returned on exit
        self.mode_3d = true;
        self.split.plane = None;
        self.cmd.params = vec![CmdParam::new("f-offset", "offset", 0.0, -100000.0, 100000.0)];
        self.status = crate::i18n::tr("msg-split-body");
    }

    /// How many pieces a cutting plane yields - counted by the LIVE B-rep. It cannot be derived from the
    /// plane itself: one plane cuts a U-shaped part into three, and "always two halves" would lose a piece.
    /// `None` means the plane does not cut the body at all.
    pub(super) fn split_piece_count(&mut self, src: Id) -> Option<usize> {
        let (o, n) = self.split_plane_local(src)?;
        self.ensure_brep(); // there is nothing to cut with while there is no live B-rep
        let sh = self.live.shapes.get(&src)?;
        sh.split_by_plane(o, n, 0).map(|v| v.len())
    }

    /// The cutting plane in THE LOCAL FRAME of the body (where its B-rep lives), with the offset already
    /// applied. The pick gives world coordinates, and without the conversion a cut inside an assembly would
    /// drift by the transform of the component.
    pub(super) fn split_plane_local(&self, src: Id) -> Option<([f64; 3], [f64; 3])> {
        use qymcad_core::feature::{apply12, apply12_dir, mat_inv12};
        let sp = self.split.plane.clone()?;
        let (o, n) = self.mirror_plane_world(&sp)?;
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l < 1e-9 {
            return None;
        }
        let u = [n[0] / l, n[1] / l, n[2] / l];
        let d = self.cmd_val("offset");
        let ow = [o[0] + u[0] * d, o[1] + u[1] * d, o[2] + u[2] * d];
        let inv = mat_inv12(&self.project.body_display_transform(src, self.current_ctx_id()));
        Some((apply12(&inv, ow), apply12_dir(&inv, u)))
    }

    /// Apply the cut: the plane (as a reference - a datum or a world one) plus the offset. The number of
    /// pieces is counted BEFORE the feature is created: the timeline must create exactly as many bodies as
    /// will come out.
    pub(super) fn apply_split_cmd(&mut self) -> Option<Id> {
        let src = self.op_target_body()?;
        if self.split.plane.is_none() {
            self.status = crate::i18n::tr("msg-click-cut-plane");
            return None;
        }
        let Some(pieces) = self.split_piece_count(src).filter(|&n| n >= 2) else {
            self.status = crate::i18n::tr("msg-plane-cuts-nothing");
            return None;
        };
        let sp = self.split.plane.clone()?;
        let (plane, datum) = self.resolve_mirror_plane(sp); // the same reference the mirror uses: a datum or a world plane
        let offset = self.cmd_val("offset");
        let parts = self.project.add_split_body(src, plane, datum, offset, pieces);
        let first = *parts.first()?;
        self.store_cmd_exprs(first); // the offset stays parametric
        self.status = crate::i18n::tr1("cmd-split-done", "n", &pieces.to_string());
        Some(first)
    }

    /// REMOVE A FACE AND HEAL: click the faces of an element (a hole, a boss), and Enter takes them away.
    ///
    /// A FACE COPY: the faces of a body become a separate SURFACE while the body stays where it is. It is
    /// the bridge from the parametric side into the design layer. There are no parameters - the operation is
    /// defined solely by which faces are picked.
    pub(super) fn start_face_copy_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(30, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.cmd.params = vec![];
        self.status = crate::i18n::tr("msg-face-copy");
    }

    /// A PATCH: stretch a surface over the picked edges. The first tool of this layer that creates a shape
    /// the body did not have.
    pub(super) fn start_patch_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(32, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.faces.clear();
        self.gsel.edges.clear();
        self.refresh_edges();
        self.cmd.params = vec![];
        self.status = crate::i18n::tr("msg-patch");
    }

    /// APPLY THE PATCH: the edges go in as a query (a description if there is one), otherwise as a list of
    /// picks.
    pub(super) fn apply_patch_cmd(&mut self) -> Option<Id> {
        let src = self.edges.body.or_else(|| self.op_target_body())?;
        if self.gsel.edges.len() < 2 {
            self.status = crate::i18n::tr("msg-patch-needs-edges");
            return None;
        }
        let picks: Vec<u32> = self.gsel.edges.iter().copied().collect();
        let q = self.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
        Some(self.project.add_patch(src, q, self.opts.patch_tangent))
    }

    /// REPLACE A FACE WITH A SURFACE: the node that sews the design layer to the timeline. Two picks in one
    /// gesture - the faces of the base and the sheet; neither can be mistaken for anything else, so there is
    /// no mode to switch between them.
    pub(super) fn start_surface_replace_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(31, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.repl_surface = None;
        self.cmd.params = vec![];
        self.status = crate::i18n::tr("msg-surface-replace");
    }

    /// APPLY THE FACE REPLACEMENT: the faces go in as a query (a description if there is one), the surface
    /// as a body.
    pub(super) fn apply_surface_replace_cmd(&mut self) -> Option<Id> {
        let src = self.gsel.faces_body.or_else(|| self.op_target_body())?;
        let surface = self.repl_surface?;
        if self.gsel.faces.is_empty() {
            self.status = crate::i18n::tr("msg-faces-not-found");
            return None;
        }
        let picks: Vec<u32> = self.gsel.faces.iter().copied().collect();
        let q = self.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
        Some(self.project.add_surface_replace(src, q, surface))
    }

    /// TRIM A SURFACE: click THE part of the sheet that stays, then the tool body.
    pub(super) fn start_trim_cmd(&mut self) {
        self.cmd.open(34, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.trim.keep = None;
        self.trim.tool = None;
        self.cmd.params = vec![];
        self.status = crate::i18n::tr("msg-trim");
    }

    /// Apply the trim: the sheet with the "keep" point plus the tool.
    pub(super) fn apply_trim_cmd(&mut self) -> Option<Id> {
        let Some((src, keep)) = self.trim.keep else {
            self.status = crate::i18n::tr("msg-trim-pick-sheet");
            return None;
        };
        let Some(tool) = self.trim.tool else {
            self.status = crate::i18n::tr("msg-trim-pick-tool");
            return None;
        };
        Some(self.project.add_trim(src, tool, keep))
    }

    /// STITCH SHEETS: click the surfaces, then Enter. The operation has no number other than the tolerance -
    /// a stitch either meets along the edges or it does not.
    pub(super) fn start_stitch_cmd(&mut self) {
        self.cmd.open(33, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.stitch_parts.clear();
        self.cmd.params = vec![CmdParam::new("f-stitch-tol", "tol", 0.01, 1e-6, 10.0)];
        self.status = crate::i18n::tr("msg-stitch");
    }

    /// Apply the stitch: two or more sheets.
    pub(super) fn apply_stitch_cmd(&mut self) -> Option<Id> {
        if self.stitch_parts.len() < 2 {
            self.status = crate::i18n::tr("msg-stitch-needs-two");
            return None;
        }
        let body = self.project.add_stitch(self.stitch_parts.clone(), self.cmd_val("tol"));
        self.store_cmd_exprs(body); // the tolerance stays parametric, like every number of a feature
        Some(body)
    }

    pub(super) fn start_remove_face_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(26, self.mode_3d);
        self.borrow_view();
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear();
        self.gsel.faces_body = None;
        self.cmd.params = vec![]; // no parameters: the operation is defined solely by which faces are picked
        self.status = crate::i18n::tr("msg-remove-face");
    }

    /// Apply the removal of faces.
    ///
    /// APPLY THE FACE COPY: the faces are recorded AS A QUERY - a description if there is one, otherwise a
    /// list of picks. A copy must travel with its base exactly as everything else does.
    pub(super) fn apply_face_copy_cmd(&mut self) -> Option<Id> {
        let src = self.gsel.faces_body.or_else(|| self.op_target_body())?;
        if self.gsel.faces.is_empty() {
            self.status = crate::i18n::tr("msg-faces-not-found");
            return None;
        }
        let picks: Vec<u32> = self.gsel.faces.iter().copied().collect();
        let q = self.gsel.described.clone().map(qymcad_core::refs::Ref::many).unwrap_or_else(|| qymcad_core::refs::Ref::picks(&picks));
        Some(self.project.add_face_copy(src, q))
    }

    pub(super) fn apply_remove_face_cmd(&mut self) -> Option<Id> {
        let src = self.op_target_body()?;
        if self.gsel.faces.is_empty() {
            self.status = crate::i18n::tr("msg-click-faces-remove");
            return None;
        }
        let keys: Vec<qymcad_core::feature::FaceKey> = self
            .project
            .mesh_index(src)
            .and_then(|mi| self.project.bodies.get(mi))
            .map(|b| {
                b.faces
                    .iter()
                    .filter(|f| self.gsel.faces.contains(&f.id))
                    .map(|f| qymcad_core::feature::FaceKey {
                        index: 0,
                        centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
                        normal: f.normal,
                        id: f.id,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if keys.is_empty() {
            self.status = crate::i18n::tr("msg-faces-not-found");
            return None;
        }
        Some(self.project.add_remove_face(src, keys))
    }

    pub(super) fn start_draft_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-body");
            return;
        }
        self.cmd.open(23, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.gsel.edges.clear();
        self.gsel.faces.clear(); // the faces to tilt are picked from scratch: a draft needs no pre-selection
        self.gsel.faces_body = None;
        self.cmd.params = vec![CmdParam::new("f-angle", "angle", 3.0, -60.0, 60.0)];
        self.status = crate::i18n::tr("msg-draft");
    }


    /// A THREAD: a real thread on a cylinder or inside a hole. With a body present, click a CYLINDRICAL
    /// face; the top bar carries the side, the number of starts and the hand, while the popup at the
    /// geometry carries the pitch, the length, the angle and the depth.
    pub(super) fn start_thread_cmd(&mut self) {
        if self.op_target_body().is_none() {
            self.status = crate::i18n::tr("msg-no-cylinder");
            return;
        }
        self.cmd.open(24, self.mode_3d); // a clean slate, then open
        
        self.mode_3d = true;
        self.thread.src = None;
        self.thread.edge = 0;
        self.thread.internal = false;
        self.thread.starts = 1;
        self.thread.left = false;
        self.thread.form = 0;
        self.thread.auger = false;
        self.set_thread_params();
        self.status = crate::i18n::tr("msg-thread");
    }


    /// A CUSTOM thread opens two fields - the profile angle and the depth of the groove; every other
    /// standard removes them. This is called when THE STANDARD CHANGES: the fields used to be added only in
    /// `set_thread_params`, that is, when a face was picked, so switching to a custom thread showed nothing
    /// and the choice had no effect at all. The values already typed are preserved.
    pub(super) fn sync_custom_params(&mut self) {
        let custom = !self.thread.auger && Self::thread_standard(self.thread.form) == qymcad_core::thread::ThreadStandard::Custom;
        let has = self.cmd.params.iter().any(|p| p.key == "angle");
        if custom && !has {
            let p = self.cmd_val("pitch");
            self.cmd.params.push(CmdParam::new("f-profile-angle", "angle", 60.0, 5.0, 170.0));
            self.cmd.params.push(CmdParam::new("f-thread-depth", "depth", if p > 0.0 { p * 0.6 } else { 0.0 }, 0.0, 1000.0));
        } else if !custom && has {
            self.cmd.params.retain(|p| p.key != "angle" && p.key != "depth");
        }
    }

    /// Apply the thread: `add_thread` on the picked rim, plus storing the parameter expressions.
    pub(super) fn apply_thread_cmd(&mut self) -> Option<Id> {
        let src = self.thread.src?;
        if self.thread.edge == 0 {
            self.status = crate::i18n::tr("msg-pick-cyl-first");
            return None;
        }
        // the input is a standard plus a size, as in a professional CAD; the geometry (the diameters, the
        // depth, the profile) is computed by the core from the formulas of the standard rather than
        // estimated by eye.
        let body = if self.thread.auger {
            let spec = qymcad_core::thread::AugerSpec {
                shaft_d: self.thread.radius * 2.0, // taken from the actual geometry; the regen refines it
                outer_d: self.cmd_val("outer"),
                pitch: self.cmd_val("pitch"),
                thickness: self.cmd_val("thickness"),
                starts: self.thread.starts.max(1),
                left: self.thread.left,
                edge_r: self.cmd_val("edge_r"),
            };
            self.project.add_auger(src, self.thread.edge, spec, self.cmd_val("length"), self.cmd_val("lead_in"), self.cmd_val("lead_out"))
        } else {
            let spec = qymcad_core::thread::ThreadSpec {
                standard: Self::thread_standard(self.thread.form),
                nominal_d: self.cmd_val("nominal"),
                pitch: self.cmd_val("pitch"),
                starts: self.thread.starts.max(1),
                left: self.thread.left,
                internal: self.thread.internal,
                fit: self.cmd_val("fit"),
                crest_r: (self.cmd_val("crest_r") > 1e-9).then(|| self.cmd_val("crest_r")),
                root_r: (self.cmd_val("root_r") > 1e-9).then(|| self.cmd_val("root_r")),
                custom_depth: self.cmd_val("depth"),
                custom_angle: if self.cmd_val("angle") > 1.0 { self.cmd_val("angle") } else { 60.0 },
            };
            self.project.add_thread(src, self.thread.edge, spec, self.cmd_val("length"), self.cmd_val("lead_in"), self.cmd_val("lead_out"))
        };
        self.store_cmd_exprs(body);
        Some(body)
    }


    /// A SENSIBLE DEFAULT FOR THE DIRECTION: a cut from a sketch ON A FACE goes INTO the body (the negative
    /// normal), otherwise it would cut the void. It is a derived value, so it is computed FROM THE CURRENT
    /// parameters of the command rather than remembered when the command opens: the operation is chosen in
    /// the bar, after opening.
    fn smart_flip(cmd: u8, op: u8, plane: &qymcad_core::feature::SketchPlane) -> bool {
        cmd == 1 && op == 2 && matches!(plane, qymcad_core::feature::SketchPlane::Face(..))
    }

    /// The name of the operation for the undo step - the same one shown on the button.
    pub(super) fn feat_cmd_name(&self) -> String {
        match self.cmd.kind {
            1 => match self.feat.op {
                2 => crate::i18n::tr("f-cut"),
                3 => crate::i18n::tr("f-intersection"),
                _ => crate::i18n::tr("f-extrusion"),
            },
            3 => crate::i18n::tr("f-revolution"),
            4 => crate::i18n::tr("f-fillet"),
            5 => crate::i18n::tr("f-chamfer"),
            6 => crate::i18n::tr("f-shell"),
            8 => crate::i18n::tr("f-sweep"),
            9 => crate::i18n::tr("f-loft"),
            16 => crate::i18n::tr("f-mirror"),
            23 => crate::i18n::tr("f-draft"),
            25 => crate::i18n::tr("f-push-face"),
            26 => crate::i18n::tr("f-remove-face"),
            30 => crate::i18n::tr("f-face-copy"),
            31 => crate::i18n::tr("f-surface-replace"),
            32 => crate::i18n::tr("f-patch"),
            24 => crate::i18n::tr("f-thread"),
            _ => crate::i18n::tr("f-operation"),
        }
    }

    pub(super) fn apply_feat_cmd(&mut self) {
        // THE BOUNDARY OF THE OPERATION: everything the command does to the document is one undo step with
        // a name of its own. It opens HERE rather than in `start_feat_cmd`: opening a dialog does not change
        // the document and there is nothing to undo there. It closes at the end: success commits, a refusal
        // rolls back and leaves no trace.
        let name = self.feat_cmd_name();
        let mut tx = self.edit(name);
        tx.app().cmd_failed = false;
        tx.app().apply_feat_cmd_inner();
        // A FACT, NOT A GUESS FROM TEXT. This used to search the status line for a substring meaning "did
        // not succeed" - checking what was written for a person instead of what actually happened. Once the
        // interface was translated the status would no longer match, and a failed operation would land in
        // the undo history as a successful one: Ctrl+Z then rolls back the wrong thing.
        if tx.app().cmd_failed {
            tx.abort();
        } else {
            tx.commit();
        }
    }

    fn apply_feat_cmd_inner(&mut self) {
        // THE DERIVED VALUE IS COMPUTED HERE rather than when the command opens: the operation may have
        // been switched in the bar after opening (see `smart_flip`).
        if self.feat.flip_auto {
            if let Some(si) = self.cmd.sketch.filter(|i| *i < self.project.sketches.len()) {
                self.feat.flip = Self::smart_flip(self.cmd.kind, self.feat.op, &self.project.sketches[si].plane);
            }
        }
        self.ensure_brep(); // after opening from a bundle there are no live shapes yet - bring the cache up once
        // A sketch command (extrude or revolve) takes two steps when SEVERAL contours are picked: while we
        // are in the flat half-sketcher (`mode_3d = false`) and the profiles ARE picked, Apply or Enter first
        // CONFIRMS the selection and moves to 3D (the gizmo plus the size popup) rather than creating the
        // feature silently at the default height. Otherwise the Apply button (which is enabled as soon as the
        // profile selection is non-empty) would apply 10 mm straight away during a Ctrl multi-pick, with no
        // gizmo and no popup, and would breed a chain of nodes. A single contour already moves to 3D at the
        // start.
        if matches!(self.cmd.kind, 1 | 3) && self.cmd.edit.is_none() && !self.mode_3d && !self.gsel.profiles.is_empty() {
            self.borrow_view(); // the view is borrowed for the command and returned on exit
            self.mode_3d = true;
            self.status = crate::i18n::tr("msg-profile-picked");
            return;
        }
        // a hard backstop: DO NOT apply while any dimension carries an invalid expression (a stale value
        // would go through otherwise). The gate is duplicated on the buttons, but Enter can arrive past
        // them.
        if !self.cmd_exprs_valid() {
            self.status = crate::i18n::tr1("cmd-expr-error", "icon", ph::X);
            return;
        }
        // editing an existing feature (a double click) updates it rather than creating a new one
        let last = if let Some(fid) = self.cmd.edit {
            self.update_feat(fid)
        } else {
            match self.cmd.kind {
                1 | 3 => self.apply_sketch_cmd(self.cmd.kind),
                8 => self.apply_sweep_cmd(),
                9 => self.apply_loft_cmd(),
                4 | 5 => self.apply_edge_cmd(self.cmd.kind),
                6 => self.apply_shell_cmd(),
                23 => self.apply_draft_cmd(),
                25 => self.apply_push_face_cmd(),
                26 => self.apply_remove_face_cmd(),
                30 => self.apply_face_copy_cmd(),
                31 => self.apply_surface_replace_cmd(),
                32 => self.apply_patch_cmd(),
                33 => self.apply_stitch_cmd(),
                34 => self.apply_trim_cmd(),
                27 => self.apply_split_cmd(),
                28 => self.apply_thicken_cmd(),
                29 => self.apply_split_face_cmd(),
                24 => self.apply_thread_cmd(),
                7 => self.apply_hole_cmd(),
                10..=15 => self.apply_prim_cmd(), // the primitives
                16 => self.apply_mirror_cmd(),   // the mirror
                17 | 18 => self.apply_array_cmd(), // the array
                20 | 21 | 22 => self.apply_datum_cmd(), // a datum plane, point or axis
                _ => None,
            }
        };
        let Some(_body) = last else { return }; // on failure we stay in the command (the status is already set)
        // A CLEAN finish: the command is dropped and the selection is cleared, so neither the gizmo arrows
        // nor the right-hand panel remain
        self.cmd.open(0, self.mode_3d); // a clean slate, then open
        
        self.clear_feat_picks();
        // back from the flat half-sketcher into 3D: RETURN the view that was borrowed rather than refitting
        // it. Refitting threw away everything that had been set up by hand, and finishing a command looked
        // as if the viewport had flown off.
        self.return_view();
        self.status = crate::i18n::tr("f-done");
        // AFTER the status: if the regen fails, `regenerate_all` writes its own rebuild message - do NOT
        // overwrite it with "done" (otherwise the feature silently fails to build while the screen says
        // "done", which reads as nothing happening at all).
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.sel = Sel::None;
    }


    /// Reopen a command on an EXISTING feature (a double click in the tree). It loads the dimensions, the
    /// expressions and the selection of the feature into the command; Enter then updates it in place
    /// (`update_feat`).
    ///
    /// ONLY LIVE REFERENCES - what is on screen NOW and can be picked.
    ///
    /// Reported: a chamfer on four edges, then the sketch was rebuilt and the chamfer went red; a double
    /// click, four edges picked again, Enter - and the feature then showed eight edges.
    ///
    /// Editing used to raise ALL of the previous descriptors into the selection, including those that no
    /// longer resolve: they cannot be highlighted (there is no geometry) and cannot be removed (there is
    /// nothing to click), and on writing they went into the feature together with the new ones. The program
    /// did something other than what was asked, and said so only through a number in the tree.
    ///
    /// The rule is the same for ALL features with references, which is why it lives in one method: split it
    /// across the branches and it will drift.
    fn live_picks(&self, body: Id, r: &qymcad_core::refs::Ref, faces: bool) -> std::collections::HashSet<u32> {
        let live: std::collections::HashSet<u32> = if faces {
            self.project.regen_faces.get(&body).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default()
        } else {
            self.project.regen_edges.get(&body).map(|es| es.iter().map(|e| e.id).collect()).unwrap_or_default()
        };
        // THERE MAY BE NO LIVE GEOMETRY AT ALL (the cache is not built yet) - then there is nothing to
        // discard and nothing to judge by: it is handed back as it is, otherwise editing would lose honest
        // references for no reason.
        let picked = r.query.picked_descs();
        if live.is_empty() {
            return picked.into_iter().collect();
        }
        picked.into_iter().filter(|d| live.contains(d)).collect()
    }

    pub(super) fn start_feat_cmd_edit(&mut self, fid: Id) {
        use qymcad_core::feature::FeatureKind;
        let Some(node) = self.project.timeline.iter().find(|n| n.id == fid).cloned() else { return };
        // EDITING AN EXISTING FEATURE is the same command, only with its parameters restored. It opens
        // through the same life cycle (`open` on every branch below) rather than by assigning a field:
        // otherwise the remains of the previous command seep into the edit. The view from BEFORE the edit is
        // remembered here - `open` carries it.
        let was_3d = self.mode_3d;
        self.boolean.edit = None; // close the previous boolean edit mode (the arm below turns it on again)
        self.boolean.pick = None;
        self.gsel.profiles.clear();
        self.gsel.edges.clear();
        match node.kind {
            FeatureKind::Extrude { sketch, ref profiles, height, reach, down, ref fill, .. } => {
                self.cmd.open(1, was_3d);
                self.feat.op = 0;
                self.feat.flip = reach == qymcad_core::feature::Reach::Backward; // restore the direction
                self.cmd.sketch = self.project.sketch_index(sketch);
                // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
                // chain. Editing means all of the contours.
                for &p in profiles {
                    if p != 0 {
                        self.gsel.profiles.insert(p);
                    }
                }
                for &f in fill {
                    self.gsel.profiles.insert(f); // the filled inner ones are picked too (otherwise only one circle remains)
                }
                self.cmd.extent = ExtentMode::from_extent(reach, down, false);
                if down.abs() > 1e-9 {
                    self.cmd.down = down;
                }
                self.cmd.params = vec![self.cmd_param_from(fid, "f-length", "height", height, 0.1, 10000.0)];
            }
            FeatureKind::Combine { sketch, ref profiles, height, op, extent, down, ref fill, .. } => {
                self.cmd.open(1, was_3d);
                self.feat.op = match op {
                    1 => 1,
                    2 => 3,
                    _ => 2,
                };
                self.feat.flip = extent.reach == qymcad_core::feature::Reach::Backward; // restore the direction of the tool
                self.cmd.sketch = self.project.sketch_index(sketch);
                // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
                // chain. Editing means all of the contours.
                for &p in profiles {
                    if p != 0 {
                        self.gsel.profiles.insert(p);
                    }
                }
                for &f in fill {
                    self.gsel.profiles.insert(f); // the filled inner ones are picked too
                }
                // restore the extent mode: through, symmetric, two-sided or to a length
                self.cmd.extent = ExtentMode::from_extent(extent.reach, down, extent.through);
                if down.abs() > 1e-9 {
                    self.cmd.down = down;
                }
                // `sync_dir_cmd_params` adds the second-side field at the geometry, with the stored expression
                self.cmd.params = vec![self.cmd_param_from(fid, "f-depth", "height", height.abs(), 0.1, 10000.0)];
            }
            FeatureKind::Revolve { sketch, ref profiles, axis, angle, axis_datum, axis_line, reach, op, .. } => {
                self.cmd.open(3, was_3d);
                use qymcad_core::feature::Reach;
                self.cmd.extent = if reach == Reach::BothWays { ExtentMode::Symmetric } else { ExtentMode::Length };
                self.feat.flip = reach == Reach::Backward;
                self.rev.axis = axis;
                self.rev.axis_datum = axis_datum;
                self.rev.axis_line = axis_line;
                self.rev.pick_axis = false;
                self.cmd.sketch = self.project.sketch_index(sketch);
                // ALL contours of the node go into the selection: editing a multi-contour feature must open it
                // whole, otherwise Apply quietly reduces two areas to one.
                for c in profiles {
                    self.gsel.profiles.insert(*c);
                }
                // the core op (0 = cut, 1 = pad, 2 = intersect) maps to the bar switch (0 = add, 2 = cut,
                // 3 = intersect)
                self.feat.op = match op {
                    0 => 2,
                    2 => 3,
                    _ => 0,
                };
                self.cmd.params = vec![self.cmd_param_from(fid, "f-angle", "angle", angle, 1.0, 360.0)];
            }
            FeatureKind::Sweep { sketch, ref profiles, path_sketch, path, op, .. } => {
                // reopen the sweep: restore the profile, the path and the picked contours, with no picking
                self.cmd.open(8, was_3d);
                self.cmd.sketch = self.project.sketch_index(sketch);
                self.sweep.prof_sid = sketch;
                self.sweep.path_sid = path_sketch;
                self.sweep.prof_cid = profiles.first().copied().unwrap_or(0);
                for c in profiles {
                    self.gsel.profiles.insert(*c);
                }
                self.feat.op = match op {
                    0 => 2,
                    2 => 3,
                    _ => 0,
                };
                self.sweep.path_cid = path;
                self.sweep.pick_path = false;
                self.cmd.params.clear();
            }
            FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
                // reopen the loft: restore the set of sections and the picked contours, with no picking
                self.cmd.open(9, was_3d);
                self.cmd.sketch = sketches.first().and_then(|&s| self.project.sketch_index(s));
                self.loft.sids = sketches.clone();
                self.loft.cids = (0..sketches.len()).map(|i| contours.get(i).copied().unwrap_or(0)).collect();
                self.loft.ruled = ruled;
                self.loft.result = if src == 0 { 0 } else { op + 1 }; // 0 = a new body; otherwise cut, union or intersect
                self.loft.pick = false;
                self.loft.pick_last = None;
                self.cmd.params.clear();
            }
            FeatureKind::Fillet { src, radius, ref edges, ref at_vertices, .. } => {
                self.cmd.open(4, was_3d);
                self.select_body(src);
                self.refresh_edges(); // pull up the edges of the body (this clears the selection), then restore it
                self.gsel.edges = self.live_picks(src, edges, false);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-radius", "radius", radius, 0.05, 1000.0)];
                // THE TABLE OF VERTICES - one field per vertex, each at its own place. The reference is
                // resolved against the live body: the name of a vertex is derived from its edges and survives
                // edits to its neighbours.
                let table: Vec<(qymcad_core::refs::Ref, f64)> = at_vertices.clone();
                for (r, val) in table {
                    let Ok(found) = self.project.resolve_vertex_refs(src, &r, "ref-what-fillet-vertex") else { continue };
                    let Some(desc) = found.first().copied() else { continue };
                    let Some(p) = self.project.vertex_point(src, desc) else { continue };
                    let key = format!("at{desc}");
                    let prm = self.cmd_param_from(fid, "f-radius-at-vertex", &key, val, 0.0, 1000.0);
                    self.cmd.params.push(prm.at(p));
                }
            }
            FeatureKind::Chamfer { src, dist, ref edges, mode, d2, flip, ref_face, .. } => {
                self.cmd.open(5, was_3d);
                self.select_body(src);
                self.refresh_edges();
                self.gsel.edges = self.live_picks(src, edges, false);
                self.chamfer.mode = mode; // restore the mode, the side and the second parameter
                self.chamfer.flip = flip;
                self.chamfer.ref_face = ref_face; // restore the hand-picked reference face
                self.chamfer.pick_ref = false;
                use qymcad_core::feature::ChamferMode;
                let d2_def = if d2 > 0.0 { d2 } else if mode == ChamferMode::DistAngle { 45.0 } else { 1.5 };
                self.cmd.params = vec![
                    self.cmd_param_from(fid, "f-leg", "dist", dist, 0.05, 1000.0),
                    self.cmd_param_from(fid, Self::chamfer_d2_label(mode), "d2", d2_def, 0.0, 1000.0),
                ];
            }
            FeatureKind::Shell { src, thickness, ref faces, side, .. } => {
                self.cmd.open(6, was_3d);
                self.select_body(src);
                // restore the MULTIPLE selection of open faces by their PERSISTENT ids - the highlight follows
                // the face selection
                self.gsel.faces = self.live_picks(src, faces, true);
                self.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
                self.opts.shell_side = side; // which way the wall goes
                self.cmd.params = vec![self.cmd_param_from(fid, "f-thickness", "thickness", thickness, 0.1, 1000.0)];
            }
            FeatureKind::Thicken { face, thickness, .. } => {
                self.cmd.open(28, was_3d);
                self.borrow_view();
                self.mode_3d = true;
                self.gsel.faces.clear();
                self.gsel.faces.insert(face);
                self.gsel.faces_body = node.parent.and_then(|_| self.op_target_body());
                self.cmd.params = vec![self.cmd_param_from(fid, "f-thickness", "thickness", thickness, -100000.0, 100000.0)];
                self.status = crate::i18n::tr("msg-edit-thicken");
            }
            FeatureKind::SplitFace { plane, datum, offset, .. } => {
                self.cmd.open(29, was_3d);
                self.borrow_view();
                self.mode_3d = true;
                self.split.plane = Some(if datum != 0 {
                    qymcad_core::feature::SketchPlane::Datum(datum)
                } else {
                    use qymcad_core::feature::{BasePlane, SketchPlane};
                    SketchPlane::World(match plane {
                        1 => BasePlane::XZ,
                        2 => BasePlane::YZ,
                        _ => BasePlane::XY,
                    })
                });
                self.cmd.params = vec![self.cmd_param_from(fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
                self.status = crate::i18n::tr("msg-edit-split-face");
            }
            FeatureKind::SplitBody { plane, datum, offset, .. } => {
                // EDITING A CUT: restore the reference to the plane and the offset, exactly as at creation, so
                // that pressing Enter again does not recreate the feature from scratch and lose the link to the
                // datum.
                self.cmd.open(27, was_3d);
                self.borrow_view();
                self.mode_3d = true;
                self.split.plane = Some(if datum != 0 {
                    qymcad_core::feature::SketchPlane::Datum(datum)
                } else {
                    use qymcad_core::feature::{BasePlane, SketchPlane};
                    SketchPlane::World(match plane {
                        1 => BasePlane::XZ,
                        2 => BasePlane::YZ,
                        _ => BasePlane::XY,
                    })
                });
                self.cmd.params = vec![self.cmd_param_from(fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
                self.status = crate::i18n::tr("msg-edit-split-body");
            }
            FeatureKind::RemoveFace { src, ref faces, .. } => {
                self.cmd.open(26, was_3d);
                self.select_body(src);
                self.gsel.faces = self.live_picks(src, faces, true);
                self.gsel.faces_body = Some(src);
                self.cmd.params = vec![];
            }
            FeatureKind::PushFace { src, ref face, dist, .. } => {
                // EDITING THE FEATURE: the picked face is restored by its persistent id and the offset as an
                // expression if there was one (`cmd_param_from` takes the text from the feature dims).
                self.cmd.open(25, was_3d);
                self.select_body(src);
                self.gsel.faces = self.live_picks(src, face, true);
                self.gsel.faces_body = Some(src);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-offset2", "dist", dist, -100000.0, 100000.0)];
            }
            FeatureKind::Draft { src, ref faces, neutral, angle, flip, .. } => {
                self.cmd.open(23, was_3d);
                self.select_body(src);
                // restore the set of tilted faces and the neutral one by their persistent ids
                self.gsel.faces = self.live_picks(src, faces, true);
                self.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
                self.draft.neutral = self.live_picks(src, &neutral, true).into_iter().next().unwrap_or(0);
                self.draft.pick_neutral = false;
                self.draft.flip = flip;
                self.cmd.params = vec![self.cmd_param_from(fid, "f-angle", "angle", angle, -60.0, 60.0)];
            }
            FeatureKind::Hole { src, face, diameter, depth, kind, dia2, depth2, sketch, flip, .. } => {
                self.cmd.open(7, was_3d);
                self.select_body(src);
                self.hole.kind = kind; // F1
                self.hole.mode = if sketch != 0 { 1 } else { 0 }; // the placement mode
                self.hole.sketch = if sketch != 0 { Some(sketch) } else { None };
                self.hole.flip = flip;
                if sketch != 0 {
                    // "by sketch": highlight the sketch of marks itself (no face pick is needed)
                    if let Some(si) = self.project.sketch_index(sketch) {
                        self.sel = Sel::Sketch(si);
                    }
                } else if let Some((mi, fi)) = face_desc_of(&face).and_then(|d| self.resolve_face_sel(src, &qymcad_core::feature::FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: d })) {
                    // restore THE FACE SELECTION by its persistent `FaceKey`, then highlight it
                    self.sel = Sel::Face(mi, fi);
                }
                self.cmd.params = vec![self.cmd_param_from(fid, "f-diameter", "diameter", diameter, 0.1, 10000.0), self.cmd_param_from(fid, "f-depth", "depth", depth, 0.1, 10000.0)];
                if kind != 0 {
                    self.cmd.params.push(self.cmd_param_from(fid, "f-recess-d", "dia2", dia2, 0.1, 10000.0));
                    self.cmd.params.push(self.cmd_param_from(fid, "f-recess-depth", "depth2", depth2, 0.1, 10000.0));
                }
            }
            // A THREAD: a double click reopens it with its parameters; the rim, the axis and the radius are
            // restored from `regen_edges`
            FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, .. } => {
                self.cmd.open(24, was_3d);
                self.select_body(src);
                self.thread.src = Some(src);
                self.thread.edge = edge;
                self.thread.auger = false;
                self.thread.internal = spec.internal;
                self.thread.starts = spec.starts.max(1);
                self.thread.left = spec.left;
                self.thread.form = Self::thread_standard_idx(spec.standard);
                self.restore_thread_axis(src, edge);
                // THE SET OF FIELDS IS THE SAME AS AT CREATION. The fillets were missing here entirely: they
                // could be set only when the feature was first built, and on editing they vanished silently.
                self.cmd.params = vec![
                    self.cmd_param_from(fid, "f-nominal-d", "nominal", spec.nominal_d, 0.5, 1000.0),
                    self.cmd_param_from(fid, "f-pitch-std", "pitch", spec.pitch, 0.0, 100.0),
                    self.cmd_param_from(fid, "f-length", "length", length, 0.1, 10000.0),
                    self.cmd_param_from(fid, "f-fit-clearance", "fit", spec.fit, 0.0, 5.0),
                    self.cmd_param_from(fid, "f-lead-in", "lead_in", lead_in, 0.0, 10000.0),
                    self.cmd_param_from(fid, "f-lead-out", "lead_out", lead_out, 0.0, 10000.0),
                    self.cmd_param_from(fid, "f-crest-fillet", "crest_r", spec.crest_r.unwrap_or(0.0), 0.0, 100.0),
                    self.cmd_param_from(fid, "f-root-fillet", "root_r", spec.root_r.unwrap_or(0.0), 0.0, 100.0),
                ];
                if spec.standard == qymcad_core::thread::ThreadStandard::Custom {
                    self.cmd.params.push(self.cmd_param_from(fid, "f-profile-angle", "angle", spec.custom_angle, 5.0, 170.0));
                    self.cmd.params.push(self.cmd_param_from(fid, "f-thread-depth", "depth", spec.custom_depth, 0.0, 1000.0));
                }
            }
            // AN AUGER - the same command, a mode of the timeline
            FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, .. } => {
                self.cmd.open(24, was_3d);
                self.select_body(src);
                self.thread.src = Some(src);
                self.thread.edge = edge;
                self.thread.auger = true;
                self.thread.starts = spec.starts.max(1);
                self.thread.left = spec.left;
                self.restore_thread_axis(src, edge);
                self.cmd.params = vec![
                    self.cmd_param_from(fid, "f-outer-d", "outer", spec.outer_d, 0.5, 2000.0),
                    self.cmd_param_from(fid, "f-pitch", "pitch", spec.pitch, 0.1, 1000.0),
                    self.cmd_param_from(fid, "f-length", "length", length, 0.1, 10000.0),
                    self.cmd_param_from(fid, "f-flight-thickness", "thickness", spec.thickness, 0.1, 100.0),
                    self.cmd_param_from(fid, "f-edge-fillet", "edge_r", spec.edge_r, 0.0, 50.0),
                    self.cmd_param_from(fid, "f-taper-in", "lead_in", lead_in, 0.0, 10000.0),
                    self.cmd_param_from(fid, "f-taper-out", "lead_out", lead_out, 0.0, 10000.0),
                ];
            }
            // A MIRROR: a double click reopens the command; the plane is clicked in the viewport again
            FeatureKind::Mirror { src, plane, keep, datum, .. } => {
                use qymcad_core::feature::{BasePlane, SketchPlane};
                self.cmd.open(16, was_3d);
                self.select_body(src);
                self.opts.mirror_keep = keep;
                self.mirror.plane = Some(if datum != 0 {
                    SketchPlane::Datum(datum)
                } else {
                    SketchPlane::World(match plane {
                        1 => BasePlane::XZ,
                        2 => BasePlane::YZ,
                        _ => BasePlane::XY,
                    })
                });
                self.cmd.params.clear();
            }
            // THE PRIMITIVES: a double click reopens the command with the current sizes (the fields are the
            // keys the regen uses)
            FeatureKind::Box3 { dx, dy, dz, .. } => {
                self.cmd.open(10, was_3d);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-length-x", "dx", dx, 0.1, 100000.0), self.cmd_param_from(fid, "f-width-y", "dy", dy, 0.1, 100000.0), self.cmd_param_from(fid, "f-height-z", "dz", dz, 0.1, 100000.0)];
            }
            FeatureKind::Cylinder { r, h, .. } => {
                self.cmd.open(11, was_3d);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-radius", "r", r, 0.05, 100000.0), self.cmd_param_from(fid, "f-height", "h", h, 0.1, 100000.0)];
            }
            FeatureKind::Sphere { r, .. } => {
                self.cmd.open(12, was_3d);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-radius", "r", r, 0.05, 100000.0)];
            }
            FeatureKind::Cone { r1, r2, h, .. } => {
                self.cmd.open(13, was_3d);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-radius-bottom", "r1", r1, 0.0, 100000.0), self.cmd_param_from(fid, "f-radius-top", "r2", r2, 0.0, 100000.0), self.cmd_param_from(fid, "f-height", "h", h, 0.1, 100000.0)];
            }
            FeatureKind::Torus { major, minor, .. } => {
                self.cmd.open(14, was_3d);
                self.cmd.params = vec![self.cmd_param_from(fid, "f-ring-r", "major", major, 0.1, 100000.0), self.cmd_param_from(fid, "f-tube-r", "minor", minor, 0.1, 100000.0)];
            }
            FeatureKind::Prism { r, n, h, .. } => {
                self.cmd.open(15, was_3d);
                self.prim.n = n;
                self.cmd.params = vec![self.cmd_param_from(fid, "f-radius-circ", "r", r, 0.1, 100000.0), self.cmd_param_from(fid, "f-height", "h", h, 0.1, 100000.0)];
            }
            // AN ARRAY: a double click reopens the command; the count, the direction and the axis live in the
            // bar, the step and the angle at the geometry
            FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
                self.cmd.open(17, was_3d);
                self.select_body(src);
                let (dir, step) = Self::arr_dir_of(dx, dy, dz);
                self.arr.dir = dir;
                self.arr.count = count.max(1);
                let two = count2 > 1 && (dx2.abs() + dy2.abs() + dz2.abs()) > 1e-9;
                self.arr.two = two;
                let (dir2, s2) = Self::arr_dir_of(dx2, dy2, dz2);
                self.arr.dir2 = if two { dir2 } else { 1 };
                self.arr.count2 = count2.max(1);
                let three = two && count3 > 1 && (dx3.abs() + dy3.abs() + dz3.abs()) > 1e-9;
                self.arr.three = three;
                let (dir3, s3) = Self::arr_dir_of(dx3, dy3, dz3);
                self.arr.dir3 = if three { dir3 } else { 2 };
                self.arr.count3 = count3.max(1);
                // the step is the expression text from the logical key (`store_cmd_exprs`); the numeric
                // fallback is the magnitude of the component
                self.cmd.params = vec![self.cmd_param_from(fid, "f-pitch", "step", step, 0.01, 100000.0)];
                // the second and third steps are added AT ONCE with the right magnitude as a fallback (the sync
                // does not know the value)
                if two {
                    self.cmd.params.push(self.cmd_param_from(fid, "f-pitch2", "step2", s2, 0.01, 100000.0));
                }
                if three {
                    self.cmd.params.push(self.cmd_param_from(fid, "f-pitch3", "step3", s3, 0.01, 100000.0));
                }
            }
            FeatureKind::CircularArray { src, count, angle, axis, .. } => {
                self.cmd.open(18, was_3d);
                self.select_body(src);
                self.arr.count = count.max(1);
                self.arr.axis = axis;
                self.arr.full = angle.abs() >= 359.9;
                self.arr.two = false;
                self.cmd.params = if self.arr.full { vec![] } else { vec![self.cmd_param_from(fid, "f-angle", "angle", angle, 1.0, 360.0)] };
            }
            // THE DATUMS: a double click reopens the command - a plane, a point or an axis is edited in place
            FeatureKind::Plane { plane } => {
                use qymcad_core::feature::SketchPlane;
                use qymcad_core::model::PlaneDef;
                let Some(pl) = self.project.planes.iter().find(|p| p.id == plane).cloned() else { return };
                self.datum.axis_ref = None;
                self.datum.axis_mode = 0;
                match pl.def {
                    PlaneDef::OffsetBase { base, dist } => {
                        self.cmd.open(20, was_3d);
                        self.datum.plane_pick = Some(SketchPlane::World(base));
                        self.cmd.params = vec![self.cmd_param_from(fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                    }
                    PlaneDef::OffsetFace { body, face, dist } => {
                        self.cmd.open(20, was_3d);
                        self.datum.plane_pick = Some(SketchPlane::Face(body, face));
                        self.cmd.params = vec![self.cmd_param_from(fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                    }
                    PlaneDef::OffsetPlane { plane: src, dist } => {
                        self.cmd.open(20, was_3d);
                        self.datum.plane_pick = Some(SketchPlane::Datum(src));
                        self.cmd.params = vec![self.cmd_param_from(fid, "f-offset", "dist", dist, -100000.0, 100000.0)];
                    }
                    PlaneDef::Manual => {
                        self.status = crate::i18n::tr("msg-plane-manual");
                        return;
                    }
                }
            }
            FeatureKind::DatumPoint { point } => {
                use qymcad_core::model::PointDef;
                let Some(dp) = self.project.datum_points.iter().find(|p| p.id == point).cloned() else { return };
                self.cmd.open(21, was_3d);
                match dp.def {
                    // an associative point: it reopens in "at a vertex" mode with the current reference as the
                    // preview; clicking a new vertex replaces it, and without a re-pick the definition is kept
                    PointDef::AtVertex { body, edge, end } => {
                        self.datum.pt_mode = 1;
                        self.datum.pt_vert = Some((body, edge, end, dp.at));
                        self.cmd.params.clear();
                    }
                    PointDef::Manual => {
                        self.datum.pt_mode = 0;
                        self.datum.pt_vert = None;
                        self.cmd.params = vec![
                            self.cmd_param_from(fid, "X", "x", dp.at[0], -1e7, 1e7),
                            self.cmd_param_from(fid, "Y", "y", dp.at[1], -1e7, 1e7),
                            self.cmd_param_from(fid, "Z", "z", dp.at[2], -1e7, 1e7),
                        ];
                    }
                }
            }
            FeatureKind::DatumAxis { axis } => {
                use qymcad_core::model::AxisDef;
                let Some(da) = self.project.datum_axes.iter().find(|a| a.id == axis).cloned() else { return };
                match da.def {
                    AxisDef::Manual { .. } => {
                        self.cmd.open(22, was_3d);
                        self.datum.axis_mode = 1;
                        self.datum.axis_ref = None;
                        self.cmd.params = vec![
                            self.cmd_param_from(fid, "O.x", "ox", da.origin()[0], -1e7, 1e7),
                            self.cmd_param_from(fid, "O.y", "oy", da.origin()[1], -1e7, 1e7),
                            self.cmd_param_from(fid, "O.z", "oz", da.origin()[2], -1e7, 1e7),
                            self.cmd_param_from(fid, "Dir.x", "dx", da.dir()[0], -1e7, 1e7),
                            self.cmd_param_from(fid, "Dir.y", "dy", da.dir()[1], -1e7, 1e7),
                            self.cmd_param_from(fid, "Dir.z", "dz", da.dir()[2], -1e7, 1e7),
                        ];
                    }
                    // associative axes and two-point ones reopen in "by an edge or a face" mode: the current
                    // axis is the preview, and without a re-pick the definition is kept; clicking a new
                    // reference replaces it
                    AxisDef::TwoPoints { .. } | AxisDef::FromEdge { .. } | AxisDef::FromFace { .. } => {
                        self.cmd.open(22, was_3d);
                        self.datum.axis_mode = 0;
                        self.datum.axis_hit = None;
                        self.datum.axis_ref = Some((da.origin(), da.dir()));
                        self.refresh_axis_edges();
                        self.cmd.params.clear();
                    }
                }
            }
            FeatureKind::BodyBoolean { .. } => {
                // a body boolean has no dimension popup - its KIND is edited in the top bar (the edit mode of
                // `bool_tool_bar`)
                self.boolean.edit = self.project.timeline.iter().position(|n| n.id == fid);
                if let Some(ti) = self.boolean.edit {
                    self.sel = Sel::Feature(ti);
                }
                self.status = crate::i18n::tr("msg-bool-switch");
                return;
            }
            _ => {
                // arrays, mirrors and moves have no command popup yet; they are edited in THE RIGHT-HAND PANEL
                // (the feature is already selected by the click)
                self.status = crate::i18n::tr("msg-params-on-right");
                return;
            }
        }
        self.cmd.edit = Some(fid);
        // the highlighted row of the tree STAYS on THE FEATURE BEING EDITED: `select_body(src)` in the
        // branches above moved the selection to the SOURCE body, one row higher, and the highlight appeared
        // to jump up a level while editing.
        if let Some(ti) = self.project.timeline.iter().position(|n| n.id == fid) {
            self.sel = Sel::Feature(ti);
        }
        self.mode_3d = true;
        self.status = crate::i18n::tr("msg-edit-feature");
    }


    /// Extrude or revolve along the picked contours. Returns the last body created.
    pub(super) fn apply_sketch_cmd(&mut self, cmd: u8) -> Option<Id> {
        let si = self.cmd.sketch.filter(|&si| si < self.project.sketches.len())?;
        let sid = self.project.sketches[si].id;
        // THE CONTEXT: a feature built from a sketch belongs to the component that OWNS the sketch (the part)
        // rather than to the active context. Extruding from AN ASSEMBLY (or without having entered the part)
        // would make `body_parent()` create a NEW part and put the feature there while the sketch stayed in
        // the old one - a cross-component reference is forbidden, and the sketch would not extrude. The owner
        // of the sketch is made active, so the feature lands in its part and the body builds.
        if let Some(owner) = self.project.sketch_owner(sid) {
            self.project.set_active_component(Some(owner));
        }
        let closed = self.sketch_closed_contours(si);
        let mut targets: Vec<Id> = closed.iter().copied().filter(|c| self.gsel.profiles.contains(c)).collect();
        if targets.is_empty() {
            // no contours are picked explicitly, so THE WHOLE sketch (every closed contour) is extruded in ONE
            // operation. "Extrude the sketch" means the sketch entire; with two or more contours it used to
            // demand a click on every shape, which read as the sketch not extruding at all. Now all the
            // contours make one node, a multi-profile.
            if closed.is_empty() {
                self.status = crate::i18n::tr("msg-no-closed-contour");
                return None;
            }
            targets = closed.clone();
        }
        // NESTING IS NO LONGER FILTERED. Every picked contour is A REGION OF ITS OWN (itself minus its direct
        // children), and the regions are merged by the profile fuse in the core. Picking the outer and the
        // middle of three nested contours gives a plate with a hole the size of the small one; picking two
        // concentric circles gives a solid disc. It works at any depth. A nested picked contour used to be
        // thrown into the `fill` of the outer one, and everything deeper was lost - the part came out solid,
        // which was reported as not being able to extrude with a hole in the middle.
        let fill: Vec<Id> = Vec::new();
        // The target body is looked for IN THE CONTEXT OF THE SKETCH OWNER (where the feature will land)
        // rather than in the navigation context of the interface: without having entered the part by a double
        // click, a cut reported the part as empty and every extrude made a NEW body instead of the single
        // body of the part.
        let owner_body = self.project.sketch_owner(sid).and_then(|o| self.project.active_body(o));
        // A body is REQUIRED only for A CUT or AN INTERSECTION. A pad (op 1) with no body simply creates a new
        // one, as a join does in an empty part, rather than refusing silently with "build a body first".
        let need_target = (cmd == 1 || cmd == 3) && matches!(self.feat.op, 2 | 3);
        let target = if need_target { owner_body.unwrap_or(0) } else { 0 };
        if need_target && target == 0 {
            self.status = format!("{} {}", ph::WARNING, crate::i18n::tr("cmd-cut-needs-body"));
            return None;
        }
        let reach = self.cmd_reach();
        // the second side is an expression field at the geometry (`cmd_val("down")`), not a drag value in the bar
        let down = if self.cmd.extent.two_sided() { self.cmd_val("down").abs() } else { 0.0 };
        let through = self.cmd.extent.through();
        let name = crate::i18n::name(&self.project.sketches[si].name);
        self.project.add_sketch_node(sid, name);
        let mut last = 0;
        // ALL the picked contours go through ONE operation and make ONE node (the core fuses N tools into a
        // single boolean against the body of the part). No chain of Extrude plus BodyBoolean or Combine, and
        // no collapsing.
        if cmd == 1 {
            let part = owner_body.unwrap_or(0); // the body of the part that OWNS the sketch (0 means an empty part, so create one)
            let h = self.cmd_val("height").abs();
            // console diagnostics ON REQUEST (`QYM_EXTRUDE_DEBUG=1`), as with the contour analysis. It used to
            // print unconditionally, and an ordinary session poured a line into stderr on every extrude.
            if std::env::var("QYM_EXTRUDE_DEBUG").is_ok() {
                eprintln!("[extrude] sid={sid} targets={targets:?} fill={fill:?} part={part} h={h} op={} reach={reach:?} down={down} through={through} active={:?}", self.feat.op, self.project.active_component);
            }
            last = if part == 0 {
                // AN EMPTY part gets A NEW body: an Extrude node plus `finish_base_body` - THE SAME path that
                // revolve takes (in an empty part `finish_base_body` simply returns the seed body). That makes
                // an extrude in an empty part behave like a revolve, which works, and like a sketch on an
                // existing body.
                let e = self.project.add_extrude_multi(sid, targets.clone(), h, reach, down, fill.clone());
                if e != 0 {
                    self.project.finish_base_body(e, 1)
                } else {
                    0
                }
            } else {
                // with a body present: add (0) becomes a pad (1); pad (1) stays 1; cut (2) becomes a cut (0);
                // intersect (3) becomes an intersection (2)
                let occt = match self.feat.op {
                    2 => 0,
                    3 => 2,
                    _ => 1,
                };
                self.project.add_combine_multi_op(part, sid, targets.clone(), h, occt, qymcad_core::feature::Extent { through, reach }, down, fill.clone())
            };
            if last != 0 {
                self.store_cmd_exprs(last); // the dimensions (the height) go onto the node of the operation
            } else {
                self.cmd_fail(format!("{} {}", ph::WARNING, crate::i18n::tr("cmd-op-failed")));
            }
        } else {
            // A REVOLVE puts ALL the contours and the boolean into ONE node, exactly as an extrude does.
            //
            // There used to be a loop over the contours here: a `Revolve` per contour (which is a NEW body,
            // that is, an add) plus a `BodyBoolean` per cut. Two contours produced four timeline nodes, which
            // read as the revolve falling apart into two features that add instead of cutting; editing opened
            // only one contour, and deleting either of them took the whole chain below with it.
            let part = owner_body.unwrap_or(0);
            let occt = match self.feat.op {
                2 => 0,
                3 => 2,
                _ => 1,
            };
            // A pad in AN EMPTY part is simply a new body: there is nothing to boolean against.
            let (rev_src, rev_op) = if part != 0 && matches!(self.feat.op, 1 | 2 | 3) { (part, occt) } else { (0, 1) };
            let body = self.project.add_revolve_multi_op(
                sid,
                targets.clone(),
                self.rev.axis,
                self.cmd_val("angle"),
                self.rev.axis_datum,
                self.rev.axis_line,
                self.cmd_reach(),
                rev_src,
                rev_op,
            );
            if body != 0 {
                self.store_cmd_exprs(body);
                // a new body (nothing to boolean) is merged into the single body of the part, as with an extrude
                last = if rev_src == 0 { self.project.finish_base_body(body, 1) } else { body };
            }
        }
        (last != 0).then_some(last)
    }


    /// Apply the SWEEP: the profile (`sweep_prof_sid`) along the path (`sweep_path_sid`). Both sketches are
    /// entered into the timeline if they are not there yet, then the Sweep node follows. The contours come
    /// from the selection in the bar (`sweep_prof_cid` and `sweep_path_cid`; 0 means the first suitable one
    /// is taken automatically).
    pub(super) fn apply_sweep_cmd(&mut self) -> Option<Id> {
        if self.sweep.prof_sid == 0 || self.sweep.path_sid == 0 {
            self.status = crate::i18n::tr("msg-need-profile-path");
            return None;
        }
        if self.sweep.prof_sid == self.sweep.path_sid {
            self.status = crate::i18n::tr("msg-profile-path-differ");
            return None;
        }
        let prof_name = self.project.sketches.iter().find(|s| s.id == self.sweep.prof_sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_default();
        let path_name = self.project.sketches.iter().find(|s| s.id == self.sweep.path_sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_default();
        self.project.add_sketch_node(self.sweep.prof_sid, prof_name);
        self.project.add_sketch_node(self.sweep.path_sid, path_name);
        // the profile contours and the boolean go IN ONE node (see the revolve: the reason is the same).
        let profiles: Vec<Id> = if self.sweep.prof_cid != 0 { vec![self.sweep.prof_cid] } else { Vec::new() };
        // a sweep carries AN OPERATION: add (0) becomes a pad, plus cut (2) and intersect (3)
        let occt = match self.feat.op {
            2 => 0,
            3 => 2,
            _ => 1,
        };
        let part = self.project.sketch_owner(self.sweep.prof_sid).and_then(|o| self.project.active_body(o)).unwrap_or(0);
        let (src, op) = if part != 0 && matches!(self.feat.op, 1 | 2 | 3) { (part, occt) } else { (0, 1) };
        let body = self.project.add_sweep_multi_op(self.sweep.prof_sid, profiles, self.sweep.path_sid, self.sweep.path_cid, src, op);
        (body != 0).then(|| if src == 0 { self.project.finish_base_body(body, 1) } else { body })
    }


    /// Apply the LOFT: a body through a set of sketch sections (`loft_sids`, two or more). Every section is
    /// entered into the timeline if it is not there yet, then the Loft node follows, with the picked contours
    /// (`loft_cids`) and the kind of faces (`loft_ruled`).
    pub(super) fn apply_loft_cmd(&mut self) -> Option<Id> {
        if self.loft.sids.len() < 2 {
            self.status = crate::i18n::tr("msg-loft-needs-two");
            return None;
        }
        // A cut, a pad or an intersection through sections: the target is the active body of the context.
        // "Surface" is a fourth kind of result rather than a separate tool. The question is the same one -
        // what should come out - and splitting it across two buttons would mean asking for a tool to be
        // chosen before that question has been answered.
        let surface = self.loft.result == 4;
        let (mut src, mut op): (Id, u8) = (0, 0);
        if self.loft.result != 0 && !surface {
            match self.current_body() {
                Some(b) => {
                    src = b;
                    op = self.loft.result - 1; // 1 becomes a cut (0), 2 a union (1), 3 an intersection (2)
                }
                None => {
                    self.status = crate::i18n::tr("msg-loft-no-body");
                }
            }
        }
        let (sids, cids) = (self.loft.sids.clone(), self.loft.cids.clone());
        for &sid in &sids {
            let name = self.project.sketches.iter().find(|s| s.id == sid).map(|s| crate::i18n::name(&s.name)).unwrap_or_default();
            self.project.add_sketch_node(sid, name);
        }
        let body = self.project.add_loft(sids, cids, self.loft.ruled, src, op, surface);
        if body == 0 {
            return None;
        }
        // a SOLID loft (src == 0) is a material base and goes into the single body of the part; a cut loft
        // (src != 0) is already a boolean
        Some(if src == 0 { self.project.finish_base_body(body, 1) } else { body })
    }


    /// A fillet (4) or a chamfer (5) on the picked edges of the picked body (an empty pick means every edge).
    pub(super) fn apply_edge_cmd(&mut self, cmd: u8) -> Option<Id> {
        // the target of a fillet or a chamfer is the body whose PERSISTENT edge ids actually sit in the edge
        // selection (`self.edges.body`, kept in step by `refresh_edges`) rather than `selected_body()`:
        // otherwise, if `self.sel` moved to another body between picking the edges and pressing Enter, the
        // chamfer would go to ids belonging to someone else or to none at all.
        // A part is one body, so when neither edges nor a body are picked, THE SINGLE body of the part is
        // taken (`active_body`), and the tool works straight away - a chamfer on EVERY edge, for instance -
        // without a "pick a body" step.
        let ctx = self.current_ctx_id();
        // the fallback through `current_body()` sees the body of the active part even WITHOUT entering it
        let Some(body) = self.edges.body.or_else(|| self.selected_body()).or_else(|| self.current_body()) else {
            self.status = crate::i18n::tr("msg-no-body");
            return None;
        };
        // a hard gate: the body is alive and not consumed by a chain, or it would be a ghost branch...
        if self.project.consumed_bodies().contains(&body) {
            self.status = crate::i18n::tr("msg-body-consumed");
            return None;
        }
        // ...and belongs to the current context (a body of a neighbouring part must not be filleted from
        // inside this one).
        if self.project.body_owner(body).is_some_and(|o| !self.project.component_is_within(o, ctx)) {
            self.status = crate::i18n::tr("msg-edge-other-part");
            return None;
        }
        let edges: Vec<u32> = self.gsel.edges.iter().copied().collect();
        // IT IS RECORDED THE WAY IT WAS PICKED. If there is a description ("every edge of this face", "every
        // edge parallel to this one"), the description goes in: it survives an edit that adds elements.
        // Otherwise the list of picked ids goes in.
        let described: Option<qymcad_core::refs::Ref> = self.gsel.described.clone().map(qymcad_core::refs::Ref::many);
        let last = if cmd == 4 {
            let r = self.cmd_val("radius");
            // THE "VERTEX -> RADIUS" TABLE. It works with a description and with a list alike: the radius is
            // set at a point rather than along an edge, so it needs no direction of an edge.
            let at = self.fillet_vertex_table();
            if !at.is_empty() {
                let q = described.clone().unwrap_or_else(|| qymcad_core::refs::Ref::picks(&edges));
                self.project.add_fillet_at_vertices(body, r, q, at)
            } else if let Some(q) = described {
                self.project.add_fillet_ref(body, r, q)
            } else {
                self.project.add_fillet(body, r, edges)
            }
        } else {
            use qymcad_core::feature::ChamferMode;
            // the asymmetric modes (two distances, or a leg plus an angle) work only on picked edges;
            // otherwise it is symmetric
            if self.chamfer.mode != ChamferMode::Symmetric && !edges.is_empty() {
                self.project.add_chamfer_ex(body, self.cmd_val("dist"), self.cmd_val("d2"), self.chamfer.mode, self.chamfer.flip, self.chamfer.ref_face, edges)
            } else {
                match described {
                    Some(q) => self.project.add_chamfer_ref(body, self.cmd_val("dist"), q),
                    None => self.project.add_chamfer(body, self.cmd_val("dist"), edges),
                }
            }
        };
        self.store_cmd_exprs(last); // radius, radius2, dist and d2 stay parametric
        Some(last)
    }


    /// A shell: the picked faces become open and the body becomes hollow (a multiple selection by id, plus
    /// a direction).
    pub(super) fn apply_shell_cmd(&mut self) -> Option<Id> {
        // the source is the body the picked faces belong to; the faces are the multiple selection, by
        // persistent id. It falls back to the selection when the body of the faces is not pinned yet.
        let src = self.gsel.faces_body.or_else(|| match self.sel {
            Sel::Face(mi, _) | Sel::Mesh(mi) => self.project.mesh_id(mi),
            _ => self.selected_body(),
        }).or_else(|| self.current_body())?; // a part is one body; `current_body` falls back without entering it
        let faces: Vec<u32> = self.gsel.faces.iter().copied().collect();
        if faces.is_empty() {
            self.status = crate::i18n::tr("msg-click-face-open");
            return None;
        }
        let body = self.project.add_shell_mode(src, self.cmd_val("thickness"), faces, self.opts.shell_side);
        self.store_cmd_exprs(body);
        Some(body)
    }


    /// A DRAFT: tilt the picked faces of the body relative to the neutral face (`draft_neutral`) by an
    /// angle.
    pub(super) fn apply_draft_cmd(&mut self) -> Option<Id> {
        let src = self.gsel.faces_body.or_else(|| match self.sel {
            Sel::Face(mi, _) | Sel::Mesh(mi) => self.project.mesh_id(mi),
            _ => self.selected_body(),
        }).or_else(|| self.current_body())?; // a part is one body; `current_body` falls back without entering it
        let faces: Vec<u32> = self.gsel.faces.iter().copied().collect();
        if faces.is_empty() {
            self.status = crate::i18n::tr("msg-click-face-draft");
            return None;
        }
        if self.draft.neutral == 0 {
            self.status = crate::i18n::tr("msg-pick-neutral");
            return None;
        }
        let body = self.project.add_draft(src, faces, self.draft.neutral, self.cmd_val("angle"), self.draft.flip);
        self.store_cmd_exprs(body); // the angle stays parametric
        Some(body)
    }


    /// A hole: a cylinder cut at the centre of the picked face, perpendicular to it.
    pub(super) fn apply_hole_cmd(&mut self) -> Option<Id> {
        // the kind plus the diameter and depth of the recess (for a counterbore or a countersink)
        let (dia2, depth2) = if self.hole.kind != 0 { (self.cmd_val("dia2"), self.cmd_val("depth2")) } else { (0.0, 0.0) };
        // "by sketch" mode drills many holes into the picked body, at the isolated points of a sketch
        if self.hole.mode == 1 {
            let Some(sid) = self.hole.sketch else {
                self.status = crate::i18n::tr("msg-pick-sketch-points");
                return None;
            };
            let Some(src) = self.selected_body() else {
                self.status = crate::i18n::tr("msg-pick-body-drill");
                return None;
            };
            if self.project.sketch_isolated_points(sid).is_empty() {
                self.status = crate::i18n::tr("msg-no-isolated-points");
                return None;
            }
            let body = self.project.add_hole_from_sketch(src, sid, self.cmd_val("diameter"), self.cmd_val("depth"), self.hole.kind, dia2, depth2, self.hole.flip);
            self.store_cmd_exprs(body);
            return Some(body);
        }
        let Sel::Face(mi, fi) = self.sel else {
            self.status = crate::i18n::tr("msg-pick-face-centre");
            return None;
        };
        let src = self.project.mesh_id(mi)?;
        let face = self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))?;
        // the face is referred to by its persistent id, so the hole holds on to it through a rebuild
        let key = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
        let body = self.project.add_hole_typed(src, key, self.cmd_val("diameter"), self.cmd_val("depth"), self.hole.kind, dia2, depth2);
        self.store_cmd_exprs(body);
        Some(body)
    }
}

// THE FEATURE COMMAND: the frame-by-frame drag (`update_feat`), the length field at the arrow, the sweep
// preview. This is where they belong - next to opening, applying and cancelling a command.
impl App {
    /// Update existing feature `fid` with the parameters of the active command (edit mode).
    pub(super) fn update_feat(&mut self, fid: Id) -> Option<Id> {
        use qymcad_core::feature::FeatureKind;
        // THE DATUMS edit `project.planes`, `points` and `axes` plus the feature dims rather than
        // `node.kind`, so they take a separate path
        if matches!(self.cmd.kind, 20 | 21 | 22) {
            return self.update_datum_feat(fid);
        }
        // take everything out of the command state in advance, to avoid borrowing self twice
        let (h, ang, r, dist, th, dia, dep) = (self.cmd_val("height"), self.cmd_val("angle"), self.cmd_val("radius"), self.cmd_val("dist"), self.cmd_val("thickness"), self.cmd_val("diameter"), self.cmd_val("depth"));
        // the "vertex -> radius" table is taken BEFORE the node is edited (otherwise self is borrowed twice)
        let vtable: Vec<(qymcad_core::refs::Ref, f64)> = self
            .fillet_vertex_table()
            .into_iter()
            .map(|(desc, val)| (qymcad_core::refs::Ref::one(desc, qymcad_core::refs::Fingerprint::default()), val))
            .collect();
        let dist_v = self.cmd_val("dist"); // "push a face": the offset, which may be an expression
        let ch_d2 = self.cmd_val("d2"); // the second leg or the angle of the chamfer, in degrees
        let (ch_mode, ch_flip) = (self.chamfer.mode, self.chamfer.flip); // the mode plus the side of the reference face
        let ch_ref_face = self.chamfer.ref_face; // the hand-picked reference face (0 means automatic)
        let profile = self.gsel.profiles.iter().copied().next().unwrap_or(0);
        // ALL the picked contours go into editing a multi-contour node. The edit puts them ALL into the
        // `profiles` of the node rather than a single contour.
        let (edit_contours, edit_fill): (Vec<Id>, Vec<Id>) = match self.cmd.sketch.filter(|&si| si < self.project.sketches.len()) {
            Some(si) => {
                // nesting is not filtered - every picked contour goes as a region of its own (see `apply`)
                (self.sketch_closed_contours(si).into_iter().filter(|c| self.gsel.profiles.contains(c)).collect(), Vec::new())
            }
            None => (Vec::new(), Vec::new()),
        };
        let edges: Vec<u32> = self.gsel.edges.iter().copied().collect();
        let faces_set: Vec<u32> = self.gsel.faces.iter().copied().collect(); // the shell: a multiple selection by id
        let shell_side = self.opts.shell_side; // the shell: which way the wall goes
        let draft_neutral = self.draft.neutral; // the draft: the neutral face, 0 means unset
        let draft_flip = self.draft.flip; // the draft: the direction of the pull
        // the mirror: the picked plane resolves to (plane, datum); a face creates a datum plane
        let mirror_resolved: Option<(u8, Id)> = match self.mirror.plane.clone() {
            Some(sp) => Some(self.resolve_mirror_plane(sp)),
            None => None,
        };
        let mirror_keep = self.opts.mirror_keep;
        // THE CUT: the new plane plus how many pieces it will yield. It is computed BEFORE the timeline is
        // borrowed; if it disagrees with the number of pieces the feature has, the edit is rejected below -
        // the bodies of the cut have already spread through the timeline (they are referred to and they are
        // visible), and their number cannot be changed in place.
        let (split_resolved, split_pieces) = if self.cmd.kind == 27 || self.cmd.kind == 29 {
            let n = (self.cmd.kind == 27).then(|| self.op_target_body().and_then(|src| self.split_piece_count(src))).flatten();
            let r = self.split.plane.clone().map(|sp| self.resolve_mirror_plane(sp));
            (r, n)
        } else {
            (None, None)
        };
        let split_offset = self.cmd_val("offset");
        let thickness_v = self.cmd_val("thickness"); // the thickening
        // the array: gather the vector, the angle and the count BEFORE the timeline is borrowed
        let (a_dir, a_dir2, a_dir3, a_two, a_three, a_axis, a_full) = (self.arr.dir, self.arr.dir2, self.arr.dir3, self.arr.two, self.arr.three, self.arr.axis, self.arr.full);
        let (a_count, a_count2, a_count3) = (self.arr.count.max(1), self.arr.count2.max(1), self.arr.count3.max(1));
        let (a_dx, a_dy, a_dz) = Self::arr_vec(a_dir, self.cmd_val("step"));
        let (a_dx2, a_dy2, a_dz2) = if a_two { Self::arr_vec(a_dir2, self.cmd_val("step2")) } else { (0.0, 0.0, 0.0) };
        let a_c2 = if a_two { a_count2 } else { 1 };
        let (a_dx3, a_dy3, a_dz3) = if a_two && a_three { Self::arr_vec(a_dir3, self.cmd_val("step3")) } else { (0.0, 0.0, 0.0) };
        let a_c3 = if a_two && a_three { a_count3 } else { 1 };
        let a_angle = if a_full { 360.0 } else { self.cmd_val("angle") };
        let (a_step_txt, a_step2_txt, a_step3_txt) = (self.cmd_txt("step"), self.cmd_txt("step2"), self.cmd_txt("step3"));
        // the primitives: values under the same keys the regen uses (radii)
        let (p_dx, p_dy, p_dz) = (self.cmd_val("dx"), self.cmd_val("dy"), self.cmd_val("dz"));
        let (p_r, p_r1, p_r2, p_ph, p_major, p_minor) = (self.cmd_val("r"), self.cmd_val("r1"), self.cmd_val("r2"), self.cmd_val("h"), self.cmd_val("major"), self.cmd_val("minor"));
        let p_n = self.prim.n.max(3);
        // if ANOTHER face is picked while editing, it is updated on the feature. Only for the hole: the face
        // goes in by its persistent `FaceKey` (the shell is now a multiple selection by id - `faces_set`
        // above)
        let hole_face: Option<(qymcad_core::feature::FaceKey, [f64; 3], [f64; 3])> = match self.sel {
            Sel::Face(mi, fi) => self.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|face| {
                let k = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
                (k, k.centroid, k.normal)
            }),
            _ => None,
        };
        let reach = self.cmd_reach();
        let down = if self.cmd.extent.two_sided() { self.cmd_val("down").abs() } else { 0.0 };
        let through = self.cmd.extent.through();
        let occt = match self.feat.op {
            1 => 1,
            3 => 2,
            _ => 0,
        };
        let axis = self.rev.axis;
        let axis_datum_rev = self.rev.axis_datum; // K4
        let axis_line_rev = self.rev.axis_line; // 64
        let hole_kind = self.hole.kind; // F1
        let hole_flip = self.hole.flip; // the direction in "by sketch" mode
        let (hole_dia2, hole_depth2) = if self.hole.kind != 0 { (self.cmd_val("dia2"), self.cmd_val("depth2")) } else { (0.0, 0.0) };
        // THE THREAD: the values of the parameters and options BEFORE the timeline is borrowed (the angle and
        // the depth have already been taken)
        let (t_pitch, t_length) = (self.cmd_val("pitch"), self.cmd_val("length"));
        let (t_internal, t_starts, t_left) = (self.thread.internal, self.thread.starts.max(1), self.thread.left);
        let t_form = self.thread.form;
        let (t_nominal, t_fit) = (self.cmd_val("nominal"), self.cmd_val("fit")); // the size and the fit
        let (t_outer, t_thickness, t_edge_r) = (self.cmd_val("outer"), self.cmd_val("thickness"), self.cmd_val("edge_r")); // the auger
        let (t_lead_in, t_lead_out) = (self.cmd_val("lead_in"), self.cmd_val("lead_out"));
        let (t_crest_r, t_root_r) = (self.cmd_val("crest_r"), self.cmd_val("root_r")); // the fillets of the profile
        let t_angle = self.cmd_val("angle"); // the angle of a custom profile
        let (sw_prof, sw_path, sw_pcid, sw_hcid) = (self.sweep.prof_sid, self.sweep.path_sid, self.sweep.prof_cid, self.sweep.path_cid); // captured before the mutable borrow
        let (lf_sids, lf_cids, lf_ruled) = (self.loft.sids.clone(), self.loft.cids.clone(), self.loft.ruled); // the loft, captured before the mutable borrow
        // the kind of loft result maps to (src, op). For a boolean the target is the active body of the
        // context. When EDITING a loft that is already a boolean, the previous target is kept (`active_body`
        // would skip it as consumed); switching from "new body" to "cut" takes the active body.
        let lf_prev_src = self.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
            FeatureKind::Loft { src, .. } => Some(src),
            _ => None,
        });
        let (lf_src, lf_op): (Id, u8) = if self.loft.result == 0 {
            (0, 0)
        } else {
            let src = match lf_prev_src {
                Some(s) if s != 0 => s, // already a boolean - do not change the target body
                _ => self.current_body().unwrap_or(0),
            };
            (src, self.loft.result - 1)
        };
        // THE CUT rejects the edit if the plane now divides the body into a different number of pieces
        let mut bad_pieces: Option<(usize, usize)> = None;
        let found = if let Some(node) = self.project.timeline.iter_mut().find(|n| n.id == fid) {
            match &mut node.kind {
                FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
                    // update the set of sections and contours, the kind of faces and the kind of result (two
                    // or more sections are required)
                    if lf_sids.len() >= 2 {
                        *sketches = lf_sids.clone();
                        *contours = lf_cids.clone();
                    }
                    *ruled = lf_ruled;
                    *src = lf_src; // 0 means a separate body; otherwise a boolean with the active or previous body
                    *op = lf_op;
                }
                FeatureKind::Sweep { sketch, profiles, path_sketch, path, .. } => {
                    // update the profile, the path and the picked contours; either sketch may be changed
                    if sw_prof != 0 {
                        *sketch = sw_prof;
                    }
                    if sw_path != 0 {
                        *path_sketch = sw_path;
                    }
                    *profiles = if !edit_contours.is_empty() {
                        edit_contours.clone()
                    } else if sw_pcid != 0 {
                        vec![sw_pcid]
                    } else {
                        Vec::new()
                    };
                    *path = sw_hcid;
                }
                FeatureKind::Extrude { profiles: prs, height, reach: rch, down: dn, fill: fl_field, .. } => {
                    // ALL the picked contours go into the node (a multi-contour feature is edited whole)
                    if !edit_contours.is_empty() {
                        *prs = edit_contours.clone();
                        *fl_field = edit_fill.clone();
                    } else if profile != 0 {
                        *prs = vec![profile];
                    }
                    *height = h;
                    *rch = reach;
                    *dn = down;
                }
                FeatureKind::Combine { profiles: prs, height, op, extent: ext, down: dnc, fill: fl_field, .. } => {
                    if !edit_contours.is_empty() {
                        *prs = edit_contours.clone();
                        *fl_field = edit_fill.clone();
                    } else if profile != 0 {
                        *prs = vec![profile];
                    }
                    *height = h.abs(); // a positive MAGNITUDE; the direction comes from flip, symmetry or the two sides
                    *op = occt;
                    *ext = qymcad_core::feature::Extent { through, reach };
                    *dnc = down;
                }
                FeatureKind::Revolve { profiles: prs, axis: ax, angle, axis_datum, axis_line: al, reach: rch, .. } => {
                    // the edit puts ALL the picked contours into the node rather than the first one to hand
                    if !edit_contours.is_empty() {
                        *prs = edit_contours.clone();
                    } else if profile != 0 {
                        *prs = vec![profile];
                    }
                    *ax = axis;
                    *angle = ang;
                    *axis_datum = axis_datum_rev; // K4
                    *al = axis_line_rev; // the centre line of the sketch (0 means a datum or X/Y)
                    *rch = reach;
                }
                FeatureKind::Fillet { radius, edges: e, at_vertices, .. } => {
                    *radius = r;
                    *e = qymcad_core::refs::Ref::picks(&edges); // a hand-picked set of edges is a query built from ids
                    *at_vertices = vtable; // the "vertex -> radius" table
                }
                FeatureKind::Chamfer { dist: d, edges: e, mode, d2, flip, ref_face, .. } => {
                    *d = dist;
                    *e = qymcad_core::refs::Ref::picks(&edges); // a hand-picked set of edges is a query built from ids
                    *mode = ch_mode; // the mode: symmetric, two distances, or a leg plus an angle
                    *d2 = ch_d2;
                    *flip = ch_flip;
                    *ref_face = ch_ref_face; // the hand-picked reference face
                }
                FeatureKind::Shell { thickness, faces, side, .. } => {
                    *thickness = th;
                    if !faces_set.is_empty() {
                        *faces = qymcad_core::refs::Ref::picks(&faces_set); // a multiple face selection; a hand pick is a query of ids
                    }
                    *side = shell_side; // which way the wall goes
                }
                FeatureKind::Thicken { face, thickness, .. } => {
                    *thickness = thickness_v;
                    if let Some(&id) = faces_set.iter().next() {
                        *face = id; // the face can be reassigned by a click, without recreating the feature
                    }
                }
                FeatureKind::SplitFace { plane, datum, offset, .. } => {
                    if let Some((pl, dt)) = split_resolved {
                        *plane = pl;
                        *datum = dt;
                    }
                    *offset = split_offset;
                }
                FeatureKind::SplitBody { plane, datum, offset, bodies, .. } => {
                    // THE NUMBER OF PIECES MUST NOT CHANGE: later features may already refer to the existing
                    // bodies, and creating or removing a body through an edit would break those references
                    // silently. An honest refusal leaves the cut to be recreated by hand.
                    if let Some(n) = split_pieces {
                        if n != bodies.len() {
                            bad_pieces = Some((n, bodies.len()));
                        }
                    }
                    if bad_pieces.is_none() {
                        if let Some((pl, dt)) = split_resolved {
                            *plane = pl;
                            *datum = dt;
                        }
                        *offset = split_offset;
                    }
                }
                FeatureKind::RemoveFace { faces, .. } => {
                    if !faces_set.is_empty() {
                        // the set of faces is reassigned by clicks without recreating the feature; a hand-picked
                        // set is a query of `Id`s and is replaced whole rather than edited one by one
                        *faces = qymcad_core::refs::Ref::picks(&faces_set.iter().copied().collect::<Vec<_>>());
                    }
                }
                FeatureKind::PushFace { face, dist, .. } => {
                    *dist = dist_v;
                    if let Some(&id) = faces_set.iter().next() {
                        *face = qymcad_core::refs::Ref::one(id, face.hint); // the face is reassigned by a click
                    }
                }
                FeatureKind::Draft { faces, neutral, angle, flip, .. } => {
                    // update the set of tilted faces (by id), the neutral face, the angle and the direction
                    if !faces_set.is_empty() {
                        *faces = qymcad_core::refs::Ref::picks(&faces_set);
                    }
                    if draft_neutral != 0 {
                        *neutral = qymcad_core::refs::Ref::one(draft_neutral, neutral.hint);
                    }
                    *angle = ang;
                    *flip = draft_flip;
                }
                FeatureKind::Hole { face, point, normal, diameter, depth, kind, dia2, depth2, flip, .. } => {
                    *diameter = dia;
                    *depth = dep;
                    *kind = hole_kind; // F1
                    *dia2 = hole_dia2;
                    *depth2 = hole_depth2;
                    *flip = hole_flip; // the drilling direction in "by sketch" mode
                    if let Some((k, p, nrm)) = hole_face {
                        // editing the face recomputes the reference to it along with the centre and the normal
                        // of the hole. A hand pick is a query of `Id`: one particular face was pointed at.
                        *face = qymcad_core::refs::Ref::one(k.id, qymcad_core::refs::Fingerprint { centroid: k.centroid, normal: k.normal });
                        *point = p;
                        *normal = nrm;
                    }
                }
                FeatureKind::Thread { spec, length, lead_in, lead_out, .. } => {
                    // editing a thread means the standard and the size (the core computes the geometry)
                    spec.standard = Self::thread_standard(t_form);
                    spec.nominal_d = t_nominal;
                    spec.pitch = t_pitch;
                    spec.starts = t_starts;
                    spec.left = t_left;
                    spec.internal = t_internal;
                    spec.fit = t_fit;
                    spec.crest_r = (t_crest_r > 1e-9).then_some(t_crest_r);
                    spec.root_r = (t_root_r > 1e-9).then_some(t_root_r);
                    spec.custom_depth = dep;
                    spec.custom_angle = if t_angle > 1.0 { t_angle } else { 60.0 };
                    *length = t_length;
                    *lead_in = t_lead_in;
                    *lead_out = t_lead_out;
                }
                FeatureKind::Auger { spec, length, lead_in, lead_out, .. } => {
                    // editing an auger (the shaft is taken from the geometry during the regen)
                    spec.outer_d = t_outer;
                    spec.pitch = t_pitch;
                    spec.thickness = t_thickness;
                    spec.edge_r = t_edge_r;
                    spec.starts = t_starts;
                    spec.left = t_left;
                    *length = t_length;
                    *lead_in = t_lead_in;
                    *lead_out = t_lead_out;
                }
                FeatureKind::Mirror { plane, keep, datum, .. } => {
                    *keep = mirror_keep;
                    if let Some((pl, dt)) = mirror_resolved {
                        *plane = pl;
                        *datum = dt;
                    }
                }
                FeatureKind::LinearArray { dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
                    *dx = a_dx;
                    *dy = a_dy;
                    *dz = a_dz;
                    *count = a_count;
                    *dx2 = a_dx2;
                    *dy2 = a_dy2;
                    *dz2 = a_dz2;
                    *count2 = a_c2;
                    *dx3 = a_dx3;
                    *dy3 = a_dy3;
                    *dz3 = a_dz3;
                    *count3 = a_c3;
                }
                FeatureKind::CircularArray { count, angle, axis, .. } => {
                    *count = a_count;
                    *angle = a_angle;
                    *axis = a_axis;
                }
                // the primitives: the sizes are written back on reopening (the keys are the regen's keys)
                FeatureKind::Box3 { dx, dy, dz, .. } => {
                    *dx = p_dx;
                    *dy = p_dy;
                    *dz = p_dz;
                }
                FeatureKind::Cylinder { r, h, .. } => {
                    *r = p_r;
                    *h = p_ph;
                }
                FeatureKind::Sphere { r, .. } => {
                    *r = p_r;
                }
                FeatureKind::Cone { r1, r2, h, .. } => {
                    *r1 = p_r1;
                    *r2 = p_r2;
                    *h = p_ph;
                }
                FeatureKind::Torus { major, minor, .. } => {
                    *major = p_major;
                    *minor = p_minor;
                }
                FeatureKind::Prism { r, n, h, .. } => {
                    *r = p_r;
                    *n = p_n;
                    *h = p_ph;
                }
                _ => return None,
            }
            node.dirty = true;
            true
        } else {
            false
        };
        if !found {
            return None;
        }
        if let Some((got, had)) = bad_pieces {
            // the feature was left untouched - the edit was not applied AT ALL rather than half applied
            self.status = crate::i18n::tr2("cmd-split-count-changed", "got", &got.to_string(), "had", &had.to_string());
            return None;
        }
        self.store_cmd_exprs(fid); // store or clear the dimension expressions (the logical step, step2 and angle included)
        // the array: the step expressions go onto the components of the vector (the regen reads those), and the
        // angle is cleared for a full circle
        match self.cmd.kind {
            17 => {
                self.store_arr_component(fid, ["dx", "dy", "dz"], a_dir, a_step_txt.clone());
                if a_two {
                    self.store_arr_component(fid, ["dx2", "dy2", "dz2"], a_dir2, a_step2_txt.clone());
                } else {
                    for k in ["dx2", "dy2", "dz2"] {
                        self.project.set_feat_dim(fid, k, String::new());
                    }
                }
                if a_two && a_three {
                    self.store_arr_component(fid, ["dx3", "dy3", "dz3"], a_dir3, a_step3_txt.clone());
                } else {
                    for k in ["dx3", "dy3", "dz3"] {
                        self.project.set_feat_dim(fid, k, String::new());
                    }
                }
            }
            18 => {
                if a_full {
                    self.project.set_feat_dim(fid, "angle", String::new());
                }
            }
            _ => {}
        }
        Some(fid)
    }

    /// THE "VERTEX -> RADIUS" TABLE, built from the fields of the command.
    ///
    /// The fields live by the ordinary dimension mechanism - with expressions, parametrics and Enter/Esc -
    /// while which place a field belongs to is stated in its key: `at{vertex descriptor}`. No second store
    /// had to be created for this.
    pub(super) fn fillet_vertex_table(&self) -> Vec<(u32, f64)> {
        self.cmd
            .params
            .iter()
            .filter_map(|p| p.key.strip_prefix("at").and_then(|d| d.parse::<u32>().ok()).map(|desc| (desc, p.val)))
            .filter(|(_, v)| *v > 1e-9)
            .collect()
    }

    /// The on-screen input field or fields for the dimensions of the active command, at the geometry (just
    /// like sketch dimensions): a number OR an expression such as `w/2+3`. Enter applies, Esc cancels. One
    /// mechanism for every part tool.
    pub(super) fn feat_cmd_popup(&mut self, ctx: &egui::Context, rect: Rect) {
        if self.cmd.kind == 0 || self.cmd.params.is_empty() || !self.mode_3d {
            return;
        }
        let Some(anchor) = self.cmd_anchor_screen(rect) else { return };
        let ready = self.cmd_ready();
        let vars = self.project.param_map();
        // in a symmetric chamfer the d2 field (the second leg or the angle) takes no part, so it is hidden
        let hide_d2 = self.cmd.kind == 5 && self.chamfer.mode == qymcad_core::feature::ChamferMode::Symmetric;
        let mut params = std::mem::take(&mut self.cmd.params);
        let mut apply = false;
        // THE FIELDS AT THE GEOMETRY, EACH AT ITS OWN PLACE. The radius at a vertex is shown AT THAT VERTEX:
        // six identical fields in a common column cannot be told apart, and which of them is which corner is
        // the only thing that matters here.
        let basis = self.cam.basis();
        for (i, p) in params.iter_mut().enumerate() {
            let Some(w) = p.at else { continue };
            let at = self.clamp_popup(self.project3(w, rect, &basis).0, rect);
            egui::Area::new(egui::Id::new(("feat_cmd_at", i))).fixed_pos(at + egui::vec2(8.0, -8.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(p.label());
                        Self::focus_edit(ui, &mut p.txt, 56.0, &crate::i18n::tr("f-number-or-expr"), false);
                        match qymcad_core::expr::eval(&p.txt, &vars) {
                            Ok(v) => p.val = v.clamp(p.lo, p.hi),
                            Err(e) => {
                                ui.colored_label(self.scheme.pal.error_mild(), ph::X).on_hover_text(crate::i18n::expr_error_text(&e));
                            }
                        }
                    });
                });
            });
        }
        let want_focus = std::mem::take(&mut self.cmd.focus); // a one-shot auto-focus of field 0 (Enter plus selection)
        egui::Area::new(egui::Id::new("feat_cmd_popup")).fixed_pos(self.clamp_popup(anchor, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                // WHAT EXACTLY IS WRONG AND IN WHICH FIELD. There used to be only a flag saying whether all
                // was well, and the bottom of the popup printed that a dimension expression could not be
                // evaluated - the same message for a typo, for an unknown name and for a division by zero.
                // With two or three fields there is no telling which one holds the mistake.
                let mut bad: Option<(String, String)> = None;
                let mut enter = false; // Enter was pressed in at least one field
                for (i, p) in params.iter_mut().enumerate() {
                    if hide_d2 && p.key == "d2" {
                        continue; // d2 is hidden in the symmetric mode
                    }
                    if p.at.is_some() {
                        continue; // a field AT THE GEOMETRY is drawn by its own window below
                    }
                    ui.horizontal(|ui| {
                        ui.label(p.label());
                        // THE SAME FIELD AS IN A SKETCH AND IN THE PARAMETER TABLE.
                        //
                        // A private `focus_edit` used to stand here - and the list of drivers did not exist in
                        // the popups of the part tools AT ALL. It was reported plainly: there is no drop-down
                        // with a search of parameters and drivers in the popups of the sketcher and part tools.
                        // The assumption at the time was that "the field in the bars is one for everybody, so
                        // it is wired everywhere". Not everywhere: the popup at the geometry draws fields of
                        // its own, and the check showed that only once it went THROUGH THE FRAME of every
                        // tool.
                        let fid = egui::Id::new(("cmdparam", self.cmd.kind, p.key.clone()));
                        let o = super::expr_field::expr_field_autofocus(ui, &self.project, fid, &p.txt, 74.0, &crate::i18n::tr("f-number-or-expr"), i == 0 && want_focus);
                        p.txt = o.text;
                        let te = o.resp;
                        match qymcad_core::expr::eval(&p.txt, &vars) {
                            Ok(v) => p.val = v.clamp(p.lo, p.hi),
                            Err(e) => {
                                // a broken expression leaves `p.val` alone (the old value stays), marks the field and blocks Apply
                                let msg = crate::i18n::expr_error_text(&e);
                                if bad.is_none() {
                                    bad = Some((p.label(), msg.clone()));
                                }
                                ui.colored_label(self.scheme.pal.error_mild(), ph::X).on_hover_text(&msg);
                            }
                        }
                        if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            enter = true;
                        }
                    });
                }
                // Enter applies ONLY when EVERY field is valid (a stale value would go through otherwise)
                let all_ok = bad.is_none();
                if enter && all_ok {
                    apply = true;
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(ready && all_ok, egui::Button::new(egui::RichText::new(format!("{} {}", ph::CHECK, crate::i18n::tr("cmd-apply-enter"))).strong())).clicked() {
                        apply = true;
                    }
                    if ui.button("Esc").clicked() {
                        // the cancel is handled below, after the params are returned
                    }
                });
                // AN ERROR MATTERS MORE THAN A HINT. The reason used to be shown only once the command was
                // ALREADY ready (a face or a contour picked) - that is, a typo in a field was reported after
                // everything else had been done. A typo concerns what is being typed right now, and there is
                // nothing to wait for.
                if let Some((field, msg)) = &bad {
                    ui.label(egui::RichText::new(format!("{} {field}: {msg}", ph::X)).color(self.scheme.pal.error_mild()).small());
                } else if !ready {
                    ui.label(egui::RichText::new(self.cmd_hint()).weak().small());
                }
            });
        });
        self.cmd.params = params;
        if apply {
            self.apply_feat_cmd();
        }
    }

    /// The live wireframe preview of the active command (the prism of the profile along its extent) plus the
    /// arrow gizmo of the length.
    /// The geometry of the LIVE SWEEP PREVIEW: the contour of the profile carried along the path with a
    /// parallel-transported frame (the same idea as the corrected Frenet frame in the core: the minimal
    /// rotation between tangents, with no false twist). Returns the polyline of the path in world
    /// coordinates and the section loops at the stations along it. It agrees with the core at the start and
    /// the finish (the same frame perpendicular to the tangent); the middle is a stable approximation.
    pub(super) fn sweep_preview(&self) -> Option<(Vec<[f64; 3]>, Vec<Vec<[f64; 3]>>)> {
        let (prof_sid, path_sid) = (self.sweep.prof_sid, self.sweep.path_sid);
        if prof_sid == 0 || path_sid == 0 {
            return None;
        }
        let prof_cid = if self.sweep.prof_cid != 0 { self.sweep.prof_cid } else { *self.project.sweep_profile_contours(prof_sid).first()? };
        let path_cid = if self.sweep.path_cid != 0 { self.sweep.path_cid } else { *self.project.sweep_path_contours(path_sid).first()? };
        let pxy = self.project.contour_profile_xy(prof_cid)?; // the flat contour of the profile, closed
        let np = pxy.len() / 2;
        if np < 3 {
            return None;
        }
        let pf = self.project.sketch_frame_by_id(path_sid)?;
        let pidx = self.project.contour_index(path_cid)?;
        let pts2 = &self.project.contours[pidx].points;
        if pts2.len() < 2 {
            return None;
        }
        let path: Vec<[f64; 3]> = pts2
            .iter()
            .map(|p| {
                let w = pf.lift(*p);
                [w.x, w.y, w.z]
            })
            .collect();
        let n = path.len();
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let add = |a: [f64; 3], b: [f64; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let norm = |v: [f64; 3]| {
            let l = dot(v, v).sqrt();
            if l < 1e-9 {
                [0.0, 0.0, 1.0]
            } else {
                [v[0] / l, v[1] / l, v[2] / l]
            }
        };
        let tangent = |i: usize| -> [f64; 3] {
            if i == 0 {
                norm(sub(path[1], path[0]))
            } else if i == n - 1 {
                norm(sub(path[n - 1], path[n - 2]))
            } else {
                norm(add(norm(sub(path[i], path[i - 1])), norm(sub(path[i + 1], path[i]))))
            }
        };
        // the starting frame (as in the C++ core): refX perpendicular to Z, Y = Z x refX, X = Y x Z
        let z0 = tangent(0);
        let refx = if dot(z0, [1.0, 0.0, 0.0]).abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let mut y = norm(cross(z0, refx));
        let mut x = norm(cross(y, z0));
        let mut zprev = z0;
        // rotating vector v about the unit axis a by an angle given as (sin, cos) - Rodrigues
        let rot = |v: [f64; 3], a: [f64; 3], s: f64, c: f64| {
            let av = cross(a, v);
            let ad = dot(a, v) * (1.0 - c);
            [v[0] * c + av[0] * s + a[0] * ad, v[1] * c + av[1] * s + a[1] * ad, v[2] * c + av[2] * s + a[2] * ad]
        };
        let mut sections = Vec::with_capacity(n);
        for (i, org) in path.iter().enumerate() {
            let zi = tangent(i);
            // parallel transport of the frame from zprev to zi (the minimal rotation)
            let axc = cross(zprev, zi);
            let sn = dot(axc, axc).sqrt();
            if sn > 1e-9 {
                let a = [axc[0] / sn, axc[1] / sn, axc[2] / sn];
                let cs = dot(zprev, zi).clamp(-1.0, 1.0);
                x = norm(rot(x, a, sn, cs));
                y = norm(rot(y, a, sn, cs));
            }
            let sec: Vec<[f64; 3]> = (0..np)
                .map(|k| {
                    let (px, py) = (pxy[2 * k], pxy[2 * k + 1]);
                    [org[0] + px * x[0] + py * y[0], org[1] + px * x[1] + py * y[1], org[2] + px * x[2] + py * y[2]]
                })
                .collect();
            sections.push(sec);
            zprev = zi;
        }
        Some((path, sections))
    }
}

// COMMANDS: editing a datum, the boolean bar, numeric input at the body gizmo, modifying a sketch,
// deleting the selection. Their place is next to the rest of the command life cycle.
impl App {
    /// Update an existing DATUM with the parameters of the command (reopened by a double click): it edits
    /// `project.planes`, `points` and `axes` plus the feature dims (the offset and the coordinates stay
    /// parametric). Resolving happens in `regenerate_all` after the apply.
    pub(super) fn update_datum_feat(&mut self, fid: Id) -> Option<Id> {
        use qymcad_core::feature::SketchPlane;
        use qymcad_core::model::{AxisDef, PlaneDef};
        match self.cmd.kind {
            20 => {
                let dist = self.cmd_val("dist");
                let pi = self.project.planes.iter().position(|p| p.id == fid)?;
                match self.datum.plane_pick.clone() {
                    Some(SketchPlane::World(bp)) => self.project.planes[pi].def = PlaneDef::OffsetBase { base: bp, dist },
                    Some(SketchPlane::Face(body, key)) => self.project.planes[pi].def = PlaneDef::OffsetFace { body, face: key, dist },
                    Some(SketchPlane::Datum(did)) => self.project.planes[pi].def = PlaneDef::OffsetPlane { plane: did, dist },
                    _ => match &mut self.project.planes[pi].def {
                        PlaneDef::OffsetBase { dist: d, .. } | PlaneDef::OffsetFace { dist: d, .. } | PlaneDef::OffsetPlane { dist: d, .. } => *d = dist,
                        PlaneDef::Manual => {}
                    },
                }
                self.store_cmd_exprs(fid); // `dist` stays parametric
                Some(fid)
            }
            21 => {
                use qymcad_core::model::PointDef;
                let pi = self.project.datum_points.iter().position(|p| p.id == fid)?;
                if self.datum.pt_mode == 1 {
                    // "at a vertex" mode: picking another vertex makes a new associative reference; otherwise
                    // the previous one IS KEPT
                    if let Some((body, edge, end, at)) = self.datum.pt_vert {
                        self.project.datum_points[pi].def = PointDef::AtVertex { body, edge, end };
                        self.project.datum_points[pi].at = at;
                    }
                    return Some(fid);
                }
                let at = [self.cmd_val("x"), self.cmd_val("y"), self.cmd_val("z")];
                self.project.datum_points[pi].at = at;
                self.project.datum_points[pi].def = PointDef::Manual; // back to hand-typed coordinates
                self.store_cmd_exprs(fid); // x, y and z stay parametric
                Some(fid)
            }
            22 => {
                let ai = self.project.datum_axes.iter().position(|a| a.id == fid)?;
                match self.datum.axis_mode {
                    1 => {
                        // editing the coordinates by hand IS A CHANGE OF DEFINITION to a manual one rather than
                        // a note written beside the parametric definition (that would create a second truth).
                        let o = [self.cmd_val("ox"), self.cmd_val("oy"), self.cmd_val("oz")];
                        let dv = [self.cmd_val("dx"), self.cmd_val("dy"), self.cmd_val("dz")];
                        self.project.datum_axes[ai].set_manual(o, dv);
                    }
                    2 => {
                        if self.datum.axis_pts.len() == 2 {
                            let (p0, p1) = (self.datum.axis_pts[0], self.datum.axis_pts[1]);
                            if p0.0 != 0 && p1.0 != 0 {
                                self.project.datum_axes[ai].def = AxisDef::TwoPoints { a: p0.0, b: p1.0 };
                            } else {
                                let d = [p1.1[0] - p0.1[0], p1.1[1] - p0.1[1], p1.1[2] - p0.1[2]];
                                self.project.datum_axes[ai].set_manual(p0.1, d);
                            }
                        }
                    }
                    // mode 0: a new reference (a click) replaces the definition; without a re-pick the existing
                    // definition IS KEPT
                    _ => match self.datum.axis_hit {
                        Some(AxisHit::Edge(i)) => {
                            if let Some(&(body, edge, _)) = self.edges.axes.get(i) {
                                self.project.datum_axes[ai].def = AxisDef::FromEdge { body, edge };
                            }
                        }
                        Some(AxisHit::Face(body, fid2)) => self.project.datum_axes[ai].def = AxisDef::FromFace { body, face: fid2 },
                        _ => {} // nothing was re-picked, so the definition stays as it is
                    },
                }
                Some(fid)
            }
            _ => None,
        }
    }

    /// The top bar of a body-to-body boolean, in the style of the sketcher: the kind of operation, the hint
    /// about picking body B, and a cancel. It is shown while body A is picked and the click on B is awaited
    /// (`bool_pick`).
    pub(super) fn bool_tool_bar(&mut self, ctx: &egui::Context) {
        use qymcad_core::feature::FeatureKind;
        // CREATING: body A is picked, the click on B and the choice of kind are awaited
        if let Some((a, mut op)) = self.boolean.pick {
            let a_name = self.project.mesh_index(a).map(|mi| crate::i18n::name(&self.project.mesh_name(mi))).unwrap_or_else(|| "?".into());
            let mut cancel = false;
            egui::TopBottomPanel::top("bool_tool_bar").frame(self.tool_bar_frame()).show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, crate::i18n::tr("bool-bodies-btn"))).strong());
                    ui.separator();
                    ui.label(&crate::i18n::tr("f-kind"));
                    ui.selectable_value(&mut op, 0u8, &crate::i18n::tr("f-cut-ab"));
                    ui.selectable_value(&mut op, 1u8, &crate::i18n::tr("f-union"));
                    ui.selectable_value(&mut op, 2u8, &crate::i18n::tr("f-intersect"));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::tr1("cmd-bool-a-is", "name", &a_name)).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("f-cancel-esc")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
            self.boolean.pick = if cancel { None } else { Some((a, op)) };
            return;
        }
        // EDITING: a double click on the boolean node changes its KIND in the bar (as reopening the command
        // does for an extrude)
        if let Some(ti) = self.boolean.edit {
            let Some(FeatureKind::BodyBoolean { op, .. }) = self.project.timeline.get(ti).map(|n| n.kind.clone()) else {
                self.boolean.edit = None;
                return;
            };
            let mut op = op;
            let (mut done, mut changed) = (false, false);
            egui::TopBottomPanel::top("bool_tool_bar").frame(self.tool_bar_frame()).show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, crate::i18n::tr("bool-bodies-edit"))).strong());
                    ui.separator();
                    ui.label(&crate::i18n::tr("f-kind"));
                    changed |= ui.selectable_value(&mut op, 0u8, &crate::i18n::tr("f-cut-ab")).changed();
                    changed |= ui.selectable_value(&mut op, 1u8, &crate::i18n::tr("f-union")).changed();
                    changed |= ui.selectable_value(&mut op, 2u8, &crate::i18n::tr("f-intersect")).changed();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("f-done")).clicked() {
                            done = true;
                        }
                    });
                });
            });
            if changed {
                if let Some(FeatureKind::BodyBoolean { op: o, .. }) = self.project.timeline.get_mut(ti).map(|n| &mut n.kind) {
                    *o = op;
                }
                self.project.timeline[ti].dirty = true;
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
            }
            if done || self.project.timeline.get(ti).map_or(true, |n| !matches!(n.kind, FeatureKind::BodyBoolean { .. })) {
                self.boolean.edit = None;
            }
        }
    }

    /// Exact numeric input at the body gizmo: an EXPRESSION field at the geometry, as in the sketcher. Type
    /// millimetres (a shift along an axis) or degrees (a rotation about an axis), and Enter applies a
    /// PARAMETRIC Move feature; Esc cancels.
    pub(super) fn body_num_popup(&mut self, ctx: &egui::Context, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((mi, ax, rot)) = self.body_giz.num else { return };
        let (o, _) = self.body_gizmo_geometry(mi);
        let at = self.clamp_popup(self.project3(o, rect, basis).0, rect);
        let axn = ["X", "Y", "Z"][(ax as usize).min(2)];
        let label = if rot { crate::i18n::tr1("cmd-rotation-axis", "axis", axn) } else { crate::i18n::tr1("cmd-offset-axis", "axis", axn) };
        let preview = self.project.eval_expr(&self.body_giz.num_buf); // the preview lags by one frame - the buffer changes inside the field
        let mut apply = false;
        egui::Area::new(egui::Id::new("body_num")).fixed_pos(at + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(label).small());
                    let r = Self::focus_edit(ui, &mut self.body_giz.num_buf, 74.0, &crate::i18n::tr("f-expression"), std::mem::take(&mut self.body_giz.num_focus));
                    if r.lost_focus() {
                        apply = true; // Enter OR a click elsewhere commits, as with renaming in place
                    }
                    if ui.small_button(ph::CHECK).clicked() {
                        apply = true;
                    }
                });
                match &preview {
                    Ok(v) => {
                        ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                    }
                    Err(e) if !self.body_giz.num_buf.is_empty() => {
                        // THE REASON, not "it did not evaluate". A general phrase is the same for a typo, for
                        // an unknown name and for a division by zero - that is, it says nothing.
                        ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::expr_error_text(e))).small().color(self.scheme.pal.error_mild()));
                    }
                    _ => {}
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.body_giz.num = None;
            return;
        }
        if apply {
            if let Ok(v) = self.project.eval_expr(&self.body_giz.num_buf) {
                if v.abs() > 1e-9 {
                    let t = if rot {
                        rot_about_point(ax, v, o)
                    } else {
                        let mut t = qymcad_core::feature::PLACE_IDENTITY;
                        t[[3, 7, 11][(ax as usize).min(2)]] = v;
                        t
                    };
                    self.apply_body_move(mi, t);
                }
            }
            self.body_giz.num = None;
        }
    }

    /// Apply an edit operation to the selection. Returns true when it was applied.
    /// `op`: 0 delete, 1 mirror, 2 linear array, 3 circular array, 4 fillet, 5 chamfer, 6 offset.
    pub(super) fn try_modify(&mut self, op: u8) -> bool {
        let Sel::Sketch(si) = self.sel else { return false };
        let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        if eids.is_empty() {
            return false;
        }
        let ok = match op {
            0 => {
                self.project.delete_entities(si, &eids);
                self.sel_sk.clear(); // the selection and whatever was waiting for it
                true
            }
            1 => {
                let lines = self.sel_line_pts(si);
                if let Some((a, b)) = lines.first().copied() {
                    if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                        self.project.mirror_entities(si, &eids, pa.x, pa.y, pb.x, pb.y);
                    }
                } else {
                    self.project.mirror_entities(si, &eids, 0.0, 0.0, 0.0, 1.0);
                    self.status = crate::i18n::tr("msg-mirror-y");
                }
                true
            }
            2 => {
                self.project.array_linear(si, &eids, self.sk_pat.dx, self.sk_pat.dy, self.sk_pat.count);
                true
            }
            3 => {
                let (cx, cy) = if let Some(p) = self.sel_point_ids().first().and_then(|id| self.sketch_pt(si, *id)) {
                    (p.x, p.y)
                } else {
                    self.project.entities_centroid(si, &eids)
                };
                self.project.array_circular(si, &eids, cx, cy, self.sk_pat.count, self.sk_pat.angle);
                true
            }
            4 => eids.len() >= 2 && self.project.fillet_lines(si, eids[0], eids[1], self.tool_prefs.fillet) && {
                self.sel_sk.clear(); // the selection and whatever was waiting for it
                true
            },
            5 => eids.len() >= 2 && self.project.chamfer_lines(si, eids[0], eids[1], self.tool_prefs.fillet) && {
                self.sel_sk.clear(); // the selection and whatever was waiting for it
                true
            },
            6 => self.project.offset_entities(si, &eids, self.tool_prefs.offset) > 0,
            _ => false,
        };
        if ok {
            self.invalidate();
        }
        ok
    }

    /// Carry out a confirmed deletion of a tree node. One set of cascading core methods, plus a resync.
    pub(super) fn execute_delete(&mut self, sel: Sel) {
        match sel {
            Sel::Feature(ti) => self.delete_feature(ti),
            Sel::Mesh(mi) => self.delete_body_mesh(mi),
            Sel::Contour(i) => self.delete_contour(i),
            Sel::Sketch(si) => {
                if let Some(s) = self.project.sketches.get(si) {
                    let sid = s.id;
                    self.delete_sketch_full(sid);
                }
            }
            // DELETING A DATUM IS THE SAME KIND OF OPERATION as deleting a feature or a sketch, and must go
            // through the same boundary. Without it the edit went past `App::edit`: the guard reported the
            // document changed outside `App::edit`, and the undo step came out as a nameless "edit" picked up
            // after the fact by a snapshot. It was hit on a cut - the cutting plane and the feature deleted.
            Sel::Plane(i) => {
                if let Some(p) = self.project.planes.get(i) {
                    let pid = p.id;
                    self.begin_edit(&crate::i18n::tr("status-plane-delete"));
                    self.project.delete_plane(pid);
                    self.sel = Sel::None;
                    self.resync_after_topology_change();
                    self.commit_edit();
                }
            }
            Sel::DatumPoint(i) => {
                if let Some(d) = self.project.datum_points.get(i) {
                    let did = d.id;
                    self.begin_edit(&crate::i18n::tr("status-delete-point"));
                    self.project.delete_datum_point(did);
                    self.sel = Sel::None;
                    self.resync_after_topology_change();
                    self.commit_edit();
                }
            }
            Sel::DatumAxis(i) => {
                if let Some(d) = self.project.datum_axes.get(i) {
                    let did = d.id;
                    self.begin_edit(&crate::i18n::tr("status-axis-delete"));
                    self.project.delete_datum_axis(did);
                    self.sel = Sel::None;
                    self.resync_after_topology_change();
                    self.commit_edit();
                }
            }
            Sel::Joint(jid) => {
                // delete the joint and any orphaned connectors, through the same core method the cross in the
                // list uses
                self.project.delete_joint(jid);
                self.sel = Sel::None;
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
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
                    self.sel = Sel::None;
                    self.resync_after_topology_change();
                }
            }
            _ => {}
        }
        self.status = crate::i18n::tr("msg-deleted");
    }
}
