//! THE 3D VIEWPORT: what the mouse does, in phases of its own - what was grabbed -> drive what was grabbed
//! -> a click by mode.
//!
//! Split out of `render.rs`: drawing and reading the mouse are different duties with different checks, and a
//! frame test for a click had to be written past three thousand lines of painting. The same skeleton as the
//! sketch viewport in `sketching.rs`, so one shape reads the same way everywhere.

use super::*;

// THE 3D VIEWPORT moved here from `gui.rs`, with phases of its own: what was grabbed -> drive what was
// grabbed -> a click by mode -> drawing. The same skeleton as the sketch viewport in `sketching.rs`: one shape
// reads the same way everywhere and does not have to be reconstructed on the spot.
impl App {
    /// THE 3D VIEWPORT: camera orbiting, grabbing the gizmo handles, picking, drawing the bodies.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn viewport_3d(&mut self, ctx: &egui::Context, resp: &egui::Response, painter: &egui::Painter, rect: Rect, has_geom: bool, scroll: f32) {
                if !self.cam.init && has_geom {
                    self.fit3d(rect);
                }
                self.refresh_edges(); // the selected body's edges (for a chamfer or a fillet)
                // hovering a mate lights the glyph in 3D and its row in the list on the right, both ways. It is
                // written ONLY while the cursor is over the viewport - otherwise it would wipe the hover set by
                // the list row.
                if let Some(p) = resp.hover_pos().filter(|p| rect.contains(*p)) {
                    self.hover.joint = self.joint_glyph_at(rect, p);
                }
                let basis3 = self.cam.basis();
                // hover highlighting of a DOF gizmo handle (while nothing is being dragged) - it shows what to grab
                if !self.joint_drag_active() {
                    self.joint.giz_handle = match (self.active_dof_joint(), resp.hover_pos().filter(|p| rect.contains(*p))) {
                        (Some(jid), Some(p)) => self.joint_handle_hit(jid, rect, &basis3, p),
                        _ => None,
                    };
                }
                // grabbing the extrude arrow's gizmo handle (before the camera orbit)
                // WHAT WAS GRABBED IN 3D: the command arrow's handle -> the section -> the body gizmo -> the orbit
                self.viewport_3d_drag_start(resp, rect, &basis3);
                // WHILE DRAGGING IN 3D: the section, the gizmo, the camera orbit or pan
                self.viewport_3d_drag_update(ctx, resp, rect, &basis3);
                if scroll != 0.0 && resp.hovered() {
                    self.cam.scale = (self.cam.scale * (scroll * 0.002).exp()).clamp(0.05, 400.0);
                }
                // "EXPAND THE SELECTION" - THE RIGHT BUTTON ON WHAT IS PICKED.
                //
                // Camera orbiting is untouched: `context_menu` opens on a right-button CLICK, and a click in
                // egui is a press and a release with no drag in between. A drag is still an orbit, and no menu
                // appears.
                //
                // The menu is built from the shared `expand_selection::EXPANSIONS` table: an item and the query
                // it sets must live in one place, otherwise they diverge on the very first new row - a price
                // this project has already paid.
                //
                // THE MENU TARGETS WHAT IS UNDER THE CURSOR, not what has been gathered. Reported behaviour:
                // the menu worked only once every face had already been selected; hovering a face and clicking
                // the right button did nothing. What was expected is what any CAD does: the right button acts
                // on the element under the pointer, with nothing selected beforehand. The first edition looked
                // at the STORED selection, and that is where the silence came from.
                //
                // AND ONLY UNDER A COMMAND THAT WILL ACCEPT A DESCRIPTION. A description is a way of telling a
                // command what to take; outside a command (and outside a Part) there is nowhere to write it,
                // and the right button must neither open the menu nor touch the selection. `expansion_accepts`
                // is one answer to both questions, so that the click and the menu cannot diverge.
                if resp.secondary_clicked() && self.expansion_accepts().is_some() {
                    if let Some(pos) = resp.interact_pointer_pos().filter(|p| rect.contains(*p)) {
                        // AN EDGE UNDER THE CURSOR OUTRANKS A FACE: hover an edge and the menu asks about that
                        // edge ("the whole tangent chain"). Otherwise it would offer items about a face while a
                        // person was pointing at an edge.
                        let edge = matches!(self.cmd.kind, 4 | 5).then(|| self.edge_at(rect, pos)).flatten();
                        match (edge, self.edges.body) {
                            (Some(e), Some(b)) => {
                                self.gsel.last_edge = Some((e, b));
                                self.gsel.last_face = None;
                            }
                            _ => {
                                self.gsel.last_edge = None;
                                if let Some(fid) = self.pick_face_persist_id(rect, pos).filter(|&f| f != 0) {
                                    let body = self.edges.body.or(self.gsel.faces_body).or_else(|| self.body_of_face(fid));
                                    if let Some(b) = body {
                                        self.gsel.last_face = Some((fid, b));
                                    }
                                }
                            }
                        }
                    }
                }
                let items = self.expansion_menu_items();
                if !items.is_empty() {
                    resp.context_menu(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::tr("expand-title")).strong());
                        // THE CHOSEN ITEM IS MARKED. Reported behaviour: the item currently in effect was not
                        // highlighted. A menu that does not show its current state makes a person remember their
                        // own clicks, and they are under no obligation to.
                        let cur = self.gsel.described.clone();
                        for (key, q) in items {
                            let on = cur.as_ref() == Some(&q);
                            if ui.selectable_label(on, crate::i18n::tr(key)).clicked() {
                                self.apply_expansion(key, q);
                                ui.close_menu();
                            }
                        }
                    });
                }
                // a double click on a mate glyph enters EDIT mode (the parameter bar + the popup for anchors A/B).
                if resp.double_clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        if let Some(jid) = self.joint_glyph_at(rect, pos) {
                            self.enter_joint_edit(jid);
                        }
                    }
                }
                // A CLICK IN 3D: picking a plane, a face or an edge, selecting a body, assigning a command reference
                self.viewport_3d_click(resp, rect, &basis3);
                // while orbiting or zooming, draw at a reduced resolution (for smoothness);
                // at rest, full resolution (one more frame is requested to sharpen it up).
                self.view_dragging = resp.dragged() || (scroll != 0.0 && resp.hovered());
                if self.view_dragging {
                    ctx.request_repaint();
                }
                self.refresh_interference(); // recompute the intersections on the 3D path, where the assembly lives
                self.draw_3d(&painter, rect);
                self.draw_viewcube(&painter, rect);
                // the on-screen length field of the active command at the arrow tip (like the sketch dimensions)
                self.feat_cmd_popup(ctx, rect);
                // the angle or offset of the SELECTED mate as an expression popup AT THE GEOMETRY (by its glyph), not only in the panel
                self.joint_popup(ctx, rect);
                // precise numeric entry at the body gizmo: a click on an axis or a ring opens a field at the geometry
                self.body_num_popup(ctx, rect, &basis3);
    }

    /// THE START OF A DRAG IN 3D: a priority chain of grabs - the active command's arrow, the section's offset
    /// arrow, the body or component gizmo; if nothing is grabbed, the drag goes to the camera orbit.
    ///
    /// The paired phase to `viewport_3d_click`, as in the sketch: a click selects, a drag drives. The priority
    /// follows the same principle - a gizmo handle is tested BEFORE the orbit, otherwise the camera would pull
    /// the rotation out from under the finger at the moment someone was aiming at the arrow.
    pub(super) fn viewport_3d_drag_start(&mut self, resp: &egui::Response, rect: Rect, basis3: &([f64; 3], [f64; 3], [f64; 3])) {
                if resp.drag_started() {
                    // THE HANDLE AT A FACE (push, thicken): grabbing the arrow outranks the orbit, otherwise a
                    // face could not be dragged with the mouse and only typing a number would be left.
                    if let Some(key) = self.face_arrow_key() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if self.face_arrow_hit(rect, pp, &basis3) {
                                self.face_arrow_drag = Some(self.cmd_val(key));
                            }
                        }
                    }
                    // priority goes to the ACTIVE command's arrow (extrude, cut, ...) when a profile is picked
                    if self.cmd.active() && !self.gsel.profiles.is_empty() {
                        if let (Some((base, dir, h)), Some(pp)) = (self.feat_cmd_axis(), resp.interact_pointer_pos()) {
                            let tip = [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h];
                            if self.project3(tip, rect, &basis3).0.distance(pp) <= 14.0 {
                                self.cmd.drag = true;
                            }
                        }
                    }
                    // THE SECTION: grabbing the plane's offset arrow remembers the offset and the cursor AT THE
                    // MOMENT of the grab (an anchor), so that the drag counts as a delta rather than an absolute
                    // reprojection from o0 (which used to jump, adding the gizmo arrow's length to the offset at
                    // the moment of the grab)
                    if !self.cmd.drag && self.section.plane.is_some() {
                        if let (Some((_, _, _, _, tip)), Some(pp)) = (self.section_gizmo_geom(), resp.interact_pointer_pos()) {
                            if self.project3(tip, rect, &basis3).0.distance(pp) <= 14.0 {
                                self.section.drag = true;
                                self.section.drag_anchor = Some((self.section.offset, pp));
                            }
                        }
                    }
                    // grabbing a joint FREEDOM handle (a driven component OR a directly picked glyph - the root's GLOBAL)
                    if !self.cmd.drag && !self.section.drag {
                        if let (Some(jid), Some(pp)) = (self.active_dof_joint(), resp.interact_pointer_pos()) {
                            if let Some((slot, ring)) = self.joint_handle_hit(jid, rect, &basis3, pp) {
                                self.joint.giz_handle = Some((slot, ring));
                                self.joint_giz_begin(jid, slot, ring);
                            }
                        }
                        // GRABBING THE PART ITSELF: miss the thin arrow and the mechanism would not budge; a
                        // part should be grabbable anywhere.
                        if !self.joint_drag_active() && matches!(self.workbench, Workbench::Assembly) && !self.joint.pick_faces && self.joint.edit_repick.is_none() {
                            if let Some(pp) = resp.interact_pointer_pos() {
                                self.joint_grab_part_at(rect, pp, resp.drag_delta(), &basis3);
                            }
                        }
                    }
                    // the placement gizmo of a FREE component: grabbing an axis or a rotation ring
                    if !self.cmd.drag && !self.joint_drag_active() {
                        if let (Some(comp), Some(pp)) = (self.gizmo_component(), resp.interact_pointer_pos()) {
                            match self.comp_gizmo_mode(comp) {
                                CompGizmoMode::Joint(_) => {} // the joint handle was already handled above
                                CompGizmoMode::Free => {
                                    self.comp_giz.axis = self.gizmo_axis_hit(comp, rect, &basis3, pp);
                                    if self.comp_giz.axis.is_none() {
                                        self.comp_giz.ring = self.gizmo_ring_hit(comp, rect, &basis3, pp);
                                    }
                                    // pin the START transform and the gizmo origin for the duration of the drag (as
                                    // for a body), so the drag does not drift and there is a readout
                                    if self.comp_giz.axis.is_some() || self.comp_giz.ring.is_some() {
                                        let (o, _) = self.gizmo_geometry(comp);
                                        self.comp_giz.drag = Some((comp, self.project.component_transform(comp), o, 0.0));
                                        // THE EDIT BOUNDARY SPANS THE WHOLE DRAG, NOT EVERY FRAME.
                                        //
                                        // `begin_edit` takes a FULL COPY of the document. By opening and closing
                                        // the boundary inside every frame, `apply_comp_giz` used to take one on
                                        // every mouse movement: on a real assembly (138 bodies with meshes) that
                                        // is both the part following reluctantly and an undo stack filled with
                                        // one step per pixel. What is expected: grab a part, move it, and the
                                        // undo remembers nothing until it is dropped - an undo step per
                                        // coordinate change makes undo unusable.
                                        self.begin_edit(&crate::i18n::tr("status-move-component"));
                                    }
                                }
                                CompGizmoMode::None => {}
                            }
                        }
                    }
                    // the BODY gizmo inside a Part: grabbing a translation axis or a rotation ring of the selected body
                    if !self.cmd.drag && self.comp_giz.axis.is_none() && self.comp_giz.ring.is_none() && !self.joint_drag_active() {
                        if let (Some((_, mi)), Some(pp)) = (self.body_gizmo_target(), resp.interact_pointer_pos()) {
                            let (o, l) = self.body_gizmo_geometry(mi);
                            self.body_giz.axis = self.gizmo_axis_hit_at(o, l, rect, &basis3, pp);
                            self.body_giz.ring = if self.body_giz.axis.is_none() { self.gizmo_ring_hit_at(o, l, rect, &basis3, pp) } else { None };
                            self.body_giz.drag = (self.body_giz.axis.is_some() || self.body_giz.ring.is_some()).then_some((mi, o, 0.0));
                        }
                    }
                }
    }

    /// THE CONTINUATION OF A DRAG IN 3D: drive whatever was grabbed - the section plane, a gizmo handle, the
    /// command arrow - and if nothing was grabbed, orbit and pan the camera.
    ///
    /// The paired phase to `viewport_3d_drag_start`, exactly as in the sketch: that one decides WHAT was
    /// grabbed (once), this one drives it every frame. Different rates, different mistakes.
    pub(super) fn viewport_3d_drag_update(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, basis3: &([f64; 3], [f64; 3], [f64; 3])) {
                if self.section.drag {
                    // drag the SECTION PLANE along the normal as a DELTA from the anchor (the offset and cursor
                    // at the moment of the grab), rather than an absolute reprojection from o0 (which knew
                    // nothing of the offset already accumulated nor of the gizmo arrow's length, and so jumped
                    // on every fresh grab).
                    if resp.dragged() {
                        if let (Some((off0, p0)), Some((o0, _)), Some((_, n_eff)), Some(cur)) =
                            (self.section.drag_anchor, self.section.plane, self.section_eff(), resp.interact_pointer_pos())
                        {
                            let base = [o0[0], o0[1], o0[2]];
                            let s0 = self.project3(base, rect, &basis3).0;
                            let s1 = self.project3([base[0] + n_eff[0], base[1] + n_eff[1], base[2] + n_eff[2]], rect, &basis3).0;
                            if let Some(new_off) = section_drag_delta_offset(off0, p0, s0, s1, cur) {
                                self.section.offset = new_off;
                                self.invalidate();
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.section.drag = false;
                        self.section.drag_anchor = None;
                    }
                } else if self.cmd.drag {
                    // drag the active command's length along the normal (the cursor projected onto the screen axis)
                    if resp.dragged() {
                        if let (Some((base, dir, _)), Some(cur)) = (self.feat_cmd_axis(), resp.interact_pointer_pos()) {
                            let s0 = self.project3(base, rect, &basis3).0;
                            let s1 = self.project3([base[0] + dir[0], base[1] + dir[1], base[2] + dir[2]], rect, &basis3).0;
                            let pd = s1 - s0;
                            let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
                            if denom > 1e-6 {
                                let t = ((cur.x - s0.x) * pd.x + (cur.y - s0.y) * pd.y) as f64 / denom;
                                // A TWO-WAY drag: backwards (t<0, towards the negated normal) reverses the
                                // direction (flip); the magnitude is |t|. The preview, the arrow and the rebuild
                                // all take flip, so it pulls BOTH ways.
                                self.feat.set_flip(t < 0.0); // the direction is set by the gizmo drag
                                if let Some(p) = self.cmd.params.iter_mut().find(|p| p.key == "height") {
                                    p.val = t.abs().max(0.1);
                                    p.txt = format!("{:.2}", p.val);
                                }
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.cmd.drag = false;
                    }
                } else if self.face_arrow_drag.is_some() {
                    // DRAGGING A FACE WITH THE MOUSE: the offset grows along the normal, and the field and the arrow show one value
                    if resp.dragged() {
                        self.face_arrow_drag_to(resp.drag_delta(), rect, &basis3);
                    }
                    if resp.drag_stopped() {
                        self.face_arrow_drag = None;
                    }
                } else if self.joint_drag_active() {
                    // dragging a joint FREEDOM pulls the parameter (angle/offset/offset2), and solve_joints lays out the rest
                    self.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let Some(cur) = resp.interact_pointer_pos() {
                            self.joint_giz_drag_to(cur, resp.drag_delta(), rect, &basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.joint_giz_end(); // the end of a continuing operation: an undo step + a rebuild of the consumers
                    }
                } else if self.comp_giz.axis.is_some() {
                    // moving a component along the grabbed gizmo axis (unified with a body: a pinned origin + snap)
                    self.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let (Some(comp), Some(ax)) = (self.gizmo_component(), self.comp_giz.axis) {
                            self.drag_component_axis(comp, ax, resp.drag_delta(), rect, &basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.comp_giz.axis = None;
                        self.comp_giz.drag = None;
                        self.commit_edit(); // ONE undo step for the whole drag (opened when the gizmo was grabbed)
                        self.after_placement_change(); // the source moved, so the consumers are rebuilt
                    }
                } else if self.comp_giz.ring.is_some() {
                    // rotating a component by the grabbed gizmo ring (unified with a body)
                    self.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let (Some(comp), Some(ax), Some(cur)) = (self.gizmo_component(), self.comp_giz.ring, resp.interact_pointer_pos()) {
                            self.drag_component_ring(comp, ax, cur, resp.drag_delta(), rect, &basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.comp_giz.ring = None;
                        self.comp_giz.drag = None;
                        self.commit_edit(); // ONE undo step for the whole drag (opened when the gizmo was grabbed)
                        self.after_placement_change(); // the source rotated, so the consumers are rebuilt
                    }
                } else if self.body_giz.axis.is_some() && self.body_giz.drag.is_some() {
                    // moving a BODY along the grabbed gizmo axis: it accumulates and is committed as a Move feature on release
                    self.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
                    if resp.dragged() {
                        if let (Some((mi, _, _)), Some(ax)) = (self.body_giz.drag, self.body_giz.axis) {
                            self.body_gizmo_axis_drag(mi, ax, resp.drag_delta(), rect, &basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.commit_body_gizmo(self.body_giz.snap);
                    }
                } else if self.body_giz.ring.is_some() && self.body_giz.drag.is_some() {
                    // rotating a BODY by the grabbed gizmo ring
                    self.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
                    if resp.dragged() {
                        if let (Some((mi, _, _)), Some(ax), Some(cur)) = (self.body_giz.drag, self.body_giz.ring, resp.interact_pointer_pos()) {
                            self.body_gizmo_ring_drag(mi, ax, cur, resp.drag_delta(), rect, &basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.commit_body_gizmo(self.body_giz.snap);
                    }
                } else if resp.dragged() {
                    let d = resp.drag_delta();
                    if ctx.input(|i| i.modifiers.shift) {
                        let (right, up, _) = self.cam.basis();
                        let k = 1.0 / self.cam.scale as f64;
                        for a in 0..3 {
                            self.cam.target[a] -= d.x as f64 * right[a] * k;
                            self.cam.target[a] += d.y as f64 * up[a] * k;
                        }
                    } else {
                        self.cam.yaw -= d.x as f64 * 0.01;
                        self.cam.pitch = (self.cam.pitch + d.y as f64 * 0.01).clamp(-1.5, 1.5);
                    }
                }
    }

    /// AN EVENT INTO A POINT. All this layer does is take the click position out of the event.
    ///
    /// A CLICK IN THE 3D VIEWPORT: picking a plane or a face for a sketch, selecting a body and a component,
    /// assigning the active command's references (edges, faces, an axis, a profile), clearing the selection.
    ///
    /// As in the sketch, the meaning of a click depends on the active mode - which is exactly why the modes
    /// must all be dropped by ONE transition. That parsing is gathered here in one place: a long chain of
    /// `else if`, where each link answers "is this click about me?". The chain is legitimate (the links are
    /// mutually exclusive), but while it had no name the order of its links could neither be seen nor checked.
    pub(super) fn viewport_3d_click(&mut self, resp: &egui::Response, rect: Rect, basis3: &([f64; 3], [f64; 3], [f64; 3])) {
        if !resp.clicked() {
            return;
        }
        let Some(pos) = resp.interact_pointer_pos() else { return };
        self.viewport_3d_click_at(pos, rect, basis3);
    }

    /// THE ACTION AT A POINT - the same thing the mouse does, but without an `egui` event.
    ///
    /// The split exists for the tests that act "by a person's hand": faking a `Response` would mean checking
    /// one's own fake, and calling the pick functions directly would bypass the whole parsing that lives here
    /// (what outranks what, which tool is open, what was hit). A test must touch the program in exactly the
    /// same way a person does, or it is not the program it checks.
    pub(super) fn viewport_3d_click_at(&mut self, pos: egui::Pos2, rect: Rect, basis3: &([f64; 3], [f64; 3], [f64; 3])) {
        {
            {
                        // the numeric entry at the gizmo is open, so a click OUTSIDE the popup closes it (the
                        // commit is done by body_num_popup on lost_focus) and the selection and pick are untouched
                        if self.body_giz.num.is_some() {
                            // no-op: the popup commits itself
                        }
                        // a body-to-body boolean: waiting for a click on body B to create BodyBoolean(A, B, op)
                        else if let Some((a, op)) = self.boolean.pick {
                            match self.pick_body_at(rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                                Some(b) if b != a => {
                                    let res = self.project.add_body_boolean(a, b, op);
                                    self.boolean.pick = None;
                                    self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
                                    self.select_body(res);
                                    self.status = crate::i18n::tr("vp-bool-created");
                                }
                                Some(_) => self.status = crate::i18n::tr("vp-this-is-a"),
                                None => self.status = crate::i18n::tr("vp-miss-body-b"),
                            }
                        }
                        // a click on the ViewCube snaps the view
                        // the ViewCube has 26 zones (faces, edges, corners) + a home button; the turn is smooth
                        else if self.viewcube_click(rect, pos) {
                        } else if self.joint.axis_pick.is_some() {
                            // THE SECOND PICK: a person points at what the anchor's secondary axis runs along.
                            // ONE door is asked, and it takes only what is under the cursor.
                            match self.infer_axis_anchor(rect, pos) {
                                Some((_, anchor)) => self.joint_axis_pick_apply(anchor),
                                None => self.status = crate::i18n::tr("j-axis-miss"),
                            }
                        } else if self.joint.edit_repick.is_some() {
                            // Editing a mate: the NEW anchor A or B is inferred under the cursor, as when creating one
                            self.joint_repick_inferred_click(rect, pos);
                        } else if self.joint.tangent_pick.is_some() {
                            // The Tangent tool: TWO SURFACES are pointed at, and there are no connectors.
                            match self.pick_part_face_at(rect, pos) {
                                Some((body, key)) => self.tangent_pick_click(body, key),
                                None => self.status = crate::i18n::tr("vp-miss-face"),
                            }
                        } else if self.joint.width_pick.is_some() {
                            // The Width tool: FACES are pointed at - two walls and the part between them.
                            match self.pick_part_face_at(rect, pos) {
                                Some((body, key)) => self.width_pick_click(body, key),
                                None => self.status = crate::i18n::tr("vp-miss-face"),
                            }
                        } else if self.joint.group_pick.is_some() {
                            // The Group tool: a click on a part adds it to the set or removes it.
                            match self.pick_body_at(rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                                Some(body) => self.group_pick_click(body),
                                None => self.status = crate::i18n::tr("j-body-miss"),
                            }
                        } else if self.joint.ground_pick {
                            // the Ground tool: a click on a part fixes it or releases it
                            match self.pick_body_at(rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                                Some(body) => self.joint_pick_ground_click(body),
                                None => self.status = crate::i18n::tr("vp-miss-part-ground"),
                            }
                        } else if self.joint.pick_faces || self.joint.conn_pick {
                            // picking an anchor: either for a mate connector (A, then B on another part) or for a
                            // STANDALONE anchor - the parsing is the same, only what happens on the click differs,
                            // and `joint_pick_anchor_click` decides that.
                            // THE KIND OF ANCHOR IS INFERRED UNDER THE CURSOR rather than declared in advance.
                            self.joint_pick_inferred_click(rect, pos);
                        } else if let Some(src_comp) = self.mirror.part {
                            // a click on ANY plane - a base plane, a datum or a FACE - creates a mirrored copy;
                            // after that it can be dragged freely with the gizmo
                            let plane = self.pick_sketch_plane_at(rect, pos).and_then(|sp| match sp {
                                qymcad_core::feature::SketchPlane::World(bp) => {
                                    let f = bp.frame();
                                    Some((f.origin, f.normal()))
                                }
                                qymcad_core::feature::SketchPlane::Datum(id) => self.project.planes.iter().find(|p| p.id == id).map(|p| {
                                    let wt = self.datum_render_transform(id).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
                                    (qymcad_core::feature::apply12(&wt, p.origin), qymcad_core::feature::apply12_dir(&wt, p.normal))
                                }),
                                qymcad_core::feature::SketchPlane::Face(body, key) => {
                                    let wt = self.project.body_display_transform(body, self.current_ctx_id());
                                    Some((qymcad_core::feature::apply12(&wt, key.centroid), qymcad_core::feature::apply12_dir(&wt, key.normal)))
                                }
                            });
                            match plane {
                                Some((o, n)) => {
                                    // the click coordinates are in the current context's frame -> into the WORLD (the root)
                                    let cwt = self.project.world_transform(self.current_ctx_id());
                                    let (wo, wn) = (qymcad_core::feature::apply12(&cwt, o), qymcad_core::feature::apply12_dir(&cwt, n));
                                    let cnt = self.project.add_mirror_component(src_comp, wo, wn).len();
                                    self.mirror.part = None;
                                    self.mark_dirty_for_rebuild(); // the document is marked; the scheduler does the computing
                                    self.status = crate::i18n::tr1("vp-mirror-created", "n", &cnt.to_string());
                                }
                                None => {
                                    self.status = crate::i18n::tr("vp-miss-mirror-plane");
                                }
                            }
                        } else if self.section.pick {
                            // THE SECTION: a click on a plane, a datum or a face sets the cutting plane
                            let plane = self.pick_sketch_plane_at(rect, pos).and_then(|sp| match sp {
                                qymcad_core::feature::SketchPlane::World(bp) => {
                                    let f = bp.frame();
                                    Some((f.origin, f.normal()))
                                }
                                qymcad_core::feature::SketchPlane::Datum(id) => self.project.planes.iter().find(|p| p.id == id).map(|p| {
                                    // a datum is stored in its owner's LOCAL frame - its transform carries it into the context
                                    let wt = self.datum_render_transform(id).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
                                    (qymcad_core::feature::apply12(&wt, p.origin), qymcad_core::feature::apply12_dir(&wt, p.normal))
                                }),
                                qymcad_core::feature::SketchPlane::Face(body, key) => {
                                    let wt = self.project.body_display_transform(body, self.current_ctx_id());
                                    Some((qymcad_core::feature::apply12(&wt, key.centroid), qymcad_core::feature::apply12_dir(&wt, key.normal)))
                                }
                            });
                            match plane {
                                Some((o, n)) => {
                                    self.section.plane = Some((o, n));
                                    self.section.offset = 0.0;
                                    self.section.rot = [0.0, 0.0];
                                    self.section.pick = false;
                                    self.invalidate();
                                    self.status = crate::i18n::tr("vp-section-on");
                                }
                                None => {
                                    self.status = crate::i18n::tr("vp-miss-plane-datum-face");
                                }
                            }
                        } else if let Some(target) = self.picking.plane_face() {
                            // a datum plane offset from a face: a click on a part's face
                            if let Some(qymcad_core::feature::SketchPlane::Face(body, key)) = self.pick_sketch_plane_at(rect, pos) {
                                self.make_offset_plane_from_face(target, body, key);
                                self.picking.set_plane_face(None);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-face-only");
                            }
                        } else if self.pending_import.curves.is_some() {
                            // placing an imported DXF or SVG: the click sets the sketch plane
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                self.place_pending_import(sp);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-place-import");
                            }
                        } else if let Some(si) = self.picking.replace_sketch() {
                            // RE-placing a sketch: the click sets a new plane and the bodies are rebuilt
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                self.set_sketch_plane(si, sp);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-plane");
                            }
                        } else if self.picking.is_sketch_plane() {
                            // choosing a plane or a face for a new sketch by a click in the viewport
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                // the face corner (for binding the origin) is picked BEFORE entering the sketch -
                                // otherwise body_shown hides the neighbour's body and pick_vertex_pos would not find
                                // it, which would shift the origin.
                                let corner_w = self.pick_vertex_pos(rect, pos).or_else(|| self.pick_edge_point(rect, pos));
                                self.picking.clear();
                                let si = self.create_sketch_on(sp);
                                // binding the origin to an edge or a vertex. The snap is computed from the RESOLVED
                                // plane (create_sketch_on may have replaced a neighbour's face with a datum copy).
                                let resolved = self.project.sketches[si].plane.clone();
                                if let (Some(w), Some(fr)) = (corner_w, self.world_frame_of_plane(&resolved)) {
                                    self.project.sketches[si].origin_uv = Some(fr.project(qymcad_core::geom::Point3::new(w[0], w[1], w[2])));
                                    self.status = crate::i18n::tr("vp-sketch-origin-bound");
                                }
                            } else {
                                self.status = crate::i18n::tr("vp-miss-plane");
                            }
                        } else if self.cmd.kind == 16 {
                            // MIRROR: the click picks the mirror PLANE, datum or face (Enter applies it)
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.mirror.plane = Some(sp);
                                    self.status = crate::i18n::tr("vp-mirror-plane-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.cmd.kind == 27 || self.cmd.kind == 29 {
                            // SPLIT BODY: the click picks the cutting PLANE, datum or face (Enter applies it)
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.split.plane = Some(sp);
                                    let src = self.op_target_body().unwrap_or(0);
                                    self.status = match self.split_piece_count(src) {
                                        // how many pieces will come out is visible AT ONCE: otherwise "it cuts
                                        // nothing" would only be learnt on Enter, after a click and an offset
                                        // had already been spent
                                        Some(n) if n >= 2 => crate::i18n::tr1("vp-cut-plane-picked", "n", &n.to_string()),
                                        _ => crate::i18n::tr("vp-plane-cuts-nothing"),
                                    };
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.cmd.kind == 20 {
                            // DATUM PLANE: the click picks the base plane, datum or face the offset is measured from
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.datum.plane_pick = Some(sp);
                                    self.status = crate::i18n::tr("vp-ref-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.cmd.kind == 21 && self.datum.pt_mode == 1 {
                            // DATUM POINT, "at a vertex": a click on a vertex of the current part makes an ASSOCIATIVE reference
                            let ctx = self.current_ctx_id();
                            match self.pick_vertex_any(rect, pos).filter(|(b, _, _)| self.project.body_owner(*b) == Some(ctx)) {
                                Some((body, edge, end)) => {
                                    let at = self.vertex_local_pos(body, edge, end).unwrap_or([0.0; 3]);
                                    self.datum.pt_vert = Some((body, edge, end, at));
                                    self.status = crate::i18n::tr("vp-vertex-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-vertex-current"),
                            }
                        } else if self.cmd.kind == 21 {
                            // DATUM POINT, "coordinates": a click on a vertex snaps X/Y/Z to its coordinates (once, not associatively)
                            match self.pick_vertex_pos(rect, pos) {
                                Some(w) => {
                                    for (k, key) in ["x", "y", "z"].iter().enumerate() {
                                        if let Some(p) = self.cmd.params.iter_mut().find(|p| &p.key == key) {
                                            p.txt = format!("{:.3}", w[k]);
                                            p.val = w[k];
                                        }
                                    }
                                    self.status = crate::i18n::tr("vp-point-bound");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-vertex-or-xyz"),
                            }
                        } else if self.cmd.kind == 22 && self.datum.axis_mode == 0 {
                            // DATUM AXIS: a click on a straight edge or a cylindrical face makes a reference (Enter creates it ASSOCIATIVELY)
                            match self.pick_axis_at(rect, pos) {
                                Some(h) => match self.axis_ref_world(h) {
                                    Some(od) => {
                                        self.datum.axis_hit = Some(h);
                                        self.datum.axis_ref = Some(od);
                                        self.status = crate::i18n::tr("vp-axis-picked");
                                    }
                                    None => self.status = crate::i18n::tr("vp-ref-gives-no-axis"),
                                },
                                None => self.status = crate::i18n::tr("vp-miss-edge-or-cyl"),
                            }
                        } else if self.cmd.kind == 22 && self.datum.axis_mode == 2 {
                            // DATUM AXIS BY TWO POINTS: two datum points or vertices are gathered; datum points make it parametric
                            let hit = self.pick_datum_point_at(rect, pos).or_else(|| self.pick_vertex_pos(rect, pos).map(|w| (0, w)));
                            match hit {
                                Some(pt) => {
                                    if self.datum.axis_pts.len() >= 2 {
                                        self.datum.axis_pts.clear();
                                    }
                                    self.datum.axis_pts.push(pt);
                                    if self.datum.axis_pts.len() == 2 {
                                        let (a, b) = (self.datum.axis_pts[0].1, self.datum.axis_pts[1].1);
                                        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                                        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                                        self.datum.axis_ref = (l > 1e-9).then(|| (a, [d[0] / l, d[1] / l, d[2] / l]));
                                        self.status = crate::i18n::tr("vp-two-points-set");
                                    } else {
                                        self.status = crate::i18n::tr("vp-point1-set");
                                    }
                                }
                                None => self.status = crate::i18n::tr("vp-miss-datum-point"),
                            }
                        } else if (10..=15).contains(&self.cmd.kind) && self.cmd.edit.is_none() {
                            // A PRIMITIVE: a click on a vertex, a datum point, a plane or a face places it (with an orientation)
                            match self.pick_place_frame_at(rect, pos) {
                                Some(m) => {
                                    self.prim.frame = Some(m);
                                    self.prim.place = Some([m[3], m[7], m[11]]);
                                    self.status = crate::i18n::tr("vp-placement-set");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-placement"),
                            }
                        } else if self.cmd.kind == 18 && self.arr.axis_pick {
                            // A CIRCULAR PATTERN: the click picks the AXIS of rotation - a datum axis or a straight edge of the body
                            match self.pick_axis_at(rect, pos) {
                                Some(AxisHit::Datum(id)) => {
                                    self.arr.axis = id;
                                    self.arr.axis_pick = false;
                                    self.status = crate::i18n::tr("vp-array-axis-datum");
                                }
                                Some(AxisHit::Edge(i)) => match self.axis_from_edge(i) {
                                    Some(id) => {
                                        self.arr.axis = id;
                                        self.arr.axis_pick = false;
                                        self.status = crate::i18n::tr("vp-array-axis-edge");
                                    }
                                    None => self.status = crate::i18n::tr("vp-edge-not-axis"),
                                },
                                Some(AxisHit::Face(body, fid)) => match self.axis_from_face(body, fid) {
                                    Some(id) => {
                                        self.arr.axis = id;
                                        self.arr.axis_pick = false;
                                        self.status = crate::i18n::tr("vp-array-axis-cyl");
                                    }
                                    None => self.status = crate::i18n::tr("vp-face-has-no-axis"),
                                },
                                None => self.status = crate::i18n::tr("vp-miss-axis"),
                            }
                        } else if self.cmd.kind == 3 && self.rev.pick_axis {
                            // the REVOLVE axis by a click in 3D - in exactly the same place as the circular pattern's axis.
                            self.rev_axis_pick_click(rect, pos);
                        } else if self.op_pick.is_some() {
                            // the mode for gathering geometry into an operation
                            self.op_pick_at(rect, pos);
                        } else if let Some(jid) = self.joint_glyph_at(rect, pos) {
                            self.sel = Sel::Joint(jid); // a click on a mate glyph selects it
                        } else if self.cmd.kind == 5 && self.chamfer.pick_ref && self.chamfer.mode != qymcad_core::feature::ChamferMode::Symmetric {
                            // picking the chamfer's REFERENCE FACE: a click on a face sets chamfer_ref_face, and a
                            // second click on the same face clears it. A miss changes nothing.
                            match self.pick_face_persist_id(rect, pos) {
                                Some(fid) => {
                                    self.chamfer.ref_face = if self.chamfer.ref_face == fid { 0 } else { fid };
                                    self.chamfer.pick_ref = false;
                                    self.status = if self.chamfer.ref_face != 0 { crate::i18n::tr("vp-chamfer-ref-set") } else { crate::i18n::tr("vp-chamfer-ref-cleared") };
                                }
                                None => self.status = crate::i18n::tr("vp-miss-body-face"),
                            }
                        } else if self.m3.on {
                            // MEASURING IN 3D: the click picks a vertex, an edge or a face; the second one computes
                            self.measure_3d_click(rect, pos);
                        } else if matches!(self.cmd.kind, 4 | 5 | 32) && (self.pick_edge_3d(rect, pos) || self.pick_face_edges_fillet(rect, pos)) {
                            // a click on an edge OR on a FACE (a face takes all of its edges) - ONLY under
                            // Chamfer/Fillet (otherwise an ordinary click would pick an edge or a face instead of a
                            // body, and leave a stray orange selection)
                        } else if self.cmd.kind == 24 {
                            // THREAD: a click on a cylindrical face gives the body and the rim (axis and radius) + inner/outer
                            self.pick_thread_target(rect, pos);
                        } else if let Some((mi, ax, rot)) = self.body_gizmo_click_hit(rect, pos, &basis3) {
                            // A CLICK (with no drag) on the body gizmo's arrow or ring opens precise numeric entry at the geometry
                            self.body_giz.num = Some((mi, ax, rot));
                            self.body_giz.num_buf.clear();
                            self.body_giz.num_focus = true;
                        } else {
                            self.pick_face_3d(rect, pos);
                        }
                    }
                }
    }
}
