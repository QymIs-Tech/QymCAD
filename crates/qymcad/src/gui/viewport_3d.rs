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
/// HOW FAR THE POINTER MOVED FOR A NAVIGATION GESTURE.
///
/// THE RAW POINTER DELTA IS FOR THE BUTTONLESS GESTURE ALONE. A touchpad layout moves the view with no
/// button held, so egui reports no drag and there is nothing else to ask. Reaching for it whenever
/// `drag_delta` came back zero was wrong and showed itself at once: dragging an open window by its title
/// bar moved the pointer, and the camera turned along with the window.
fn nav_delta(ctx: &egui::Context, resp: &egui::Response, g: &qymcad_ui_state::Gesture) -> egui::Vec2 {
    if g.buttons.is_empty() && !g.any_button { ctx.input(|i| i.pointer.delta()) } else { resp.drag_delta() }
}

impl App {
    /// THE 3D VIEWPORT: camera orbiting, grabbing the gizmo handles, picking, drawing the bodies.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn viewport_3d(&mut self, ctx: &egui::Context, resp: &egui::Response, painter: &egui::Painter, rect: Rect, has_geom: bool, scroll: f32) {
                if !self.viewing.cam.init && has_geom {
                    crate::gui::fit3d(&mut self.viewing.cam, &self.project, rect);
                }
                crate::gui::commands::refresh_edges(&mut self.part_ctx()); // the selected body's edges (for a chamfer or a fillet)
                // hovering a mate lights the glyph in 3D and its row in the list on the right, both ways. It is
                // written ONLY while the cursor is over the viewport - otherwise it would wipe the hover set by
                // the list row.
                if let Some(p) = resp.hover_pos().filter(|p| rect.contains(*p)) {
                    self.chosen.hover.joint = qymcad_assembly::joint_glyph_at(&mut self.joint_ctx(), rect, p);
                }
                // the sketch under the cursor lights up while a tool waits for one - see `sketch_hover_3d`
                self.chosen.hover.sketch_3d = crate::gui::pick::sketch_hover_3d(&self.painting(), rect, resp.hover_pos(), self.tools.picking.sketch_for());
                let basis3 = self.viewing.cam.basis();
                // hover highlighting of a DOF gizmo handle (while nothing is being dragged) - it shows what to grab
                if !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) {
                    self.side.joint.giz_handle = match (self.active_dof_joint(), resp.hover_pos().filter(|p| rect.contains(*p))) {
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
                    let part = crate::gui::commands::cmd_anchor_screen(&mut self.part_ctx(), rect); // where the open command's fields stand
                    qymcad_ui_state::wheel_zoom_3d(&mut self.viewing.cam, &self.set, rect, resp.hover_pos(), part, scroll);
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
                        let edge = matches!(self.tools.armed.cmd_kind(), 4 | 5).then(|| crate::gui::pick::edge_at(&self.active_path, self.viewing.cam, &self.edges, &self.project, &self.set, rect, pos)).flatten();
                        match (edge, self.edges.body) {
                            (Some(e), Some(b)) => {
                                self.tools.gsel.last_edge = Some((e, b));
                                self.tools.gsel.last_face = None;
                            }
                            _ => {
                                self.tools.gsel.last_edge = None;
                                if let Some(fid) = crate::gui::pick::pick_face_persist_id(&self.painting(), rect, pos).filter(|&f| f != 0) {
                                    let body = self.edges.body.or(self.tools.gsel.faces_body).or_else(|| self.body_of_face(fid));
                                    if let Some(b) = body {
                                        self.tools.gsel.last_face = Some((fid, b));
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
                        let cur = self.tools.gsel.described.clone();
                        for (key, q) in items {
                            let on = cur.as_ref() == Some(&q);
                            if ui.selectable_label(on, crate::i18n::tr(key)).clicked() {
                                self.apply_expansion(key, q);
                                ui.close();
                            }
                        }
                    });
                }
                // a double click on a mate glyph enters EDIT mode (the parameter bar + the popup for anchors A/B).
                if resp.double_clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        if let Some(jid) = qymcad_assembly::joint_glyph_at(&mut self.joint_ctx(), rect, pos) {
                            crate::gui::enter_joint_edit(&mut self.side.joint, &mut self.chosen.sel, &mut self.status, jid);
                        }
                    }
                }
                // A CLICK IN 3D: picking a plane, a face or an edge, selecting a body, assigning a command reference
                self.viewport_3d_click(resp, rect, &basis3);
                // while orbiting or zooming, draw at a reduced resolution (for smoothness);
                // at rest, full resolution (one more frame is requested to sharpen it up).
                self.viewing.view_dragging = resp.dragged() || (scroll != 0.0 && resp.hovered());
                if self.viewing.view_dragging {
                    ctx.request_repaint();
                }
                crate::gui::refresh_interference(qymcad_ui_state::body_view_of!(self), qymcad_ui_state::scene_drag_of!(self), &mut self.interference, &self.live, &self.set, self.workbench); // recompute the intersections on the 3D path, where the assembly lives
                self.draw_3d(painter, rect);
                self.draw_viewcube(painter, rect);
                // the on-screen length field of the active command at the arrow tip (like the sketch dimensions)
                crate::gui::commands::feat_cmd_popup(&mut self.part_ctx(), ctx, rect);
                // the angle or offset of the SELECTED mate as an expression popup AT THE GEOMETRY (by its glyph), not only in the panel
                qymcad_assembly::joint_popup(&mut self.joint_ctx(), ctx, rect);
                // precise numeric entry at the body gizmo: a click on an axis or a ring opens a field at the geometry
                crate::gui::commands::body_num_popup(&mut self.part_ctx(), ctx, rect, &basis3);
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
                    if let Some(key) = qymcad_ui_state::face_arrow_key(&self.tools.armed) {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if self.face_arrow_hit(rect, pp, basis3) {
                                self.dragged.face_arrow_drag = Some(qymcad_ui_state::cmd_val(&self.tools.cmd, key));
                            }
                        }
                    }
                    // priority goes to the ACTIVE command's arrow (extrude, cut, ...) when a profile is picked
                    if self.tools.armed.commanding() && !self.tools.gsel.profiles.is_empty() {
                        if let (Some((base, dir, h)), Some(pp)) = (qymcad_ui_state::feat_cmd_axis(&self.tools.cmd, &self.tools.gsel, &self.project), resp.interact_pointer_pos()) {
                            let tip = [base[0] + dir[0] * h, base[1] + dir[1] * h, base[2] + dir[2] * h];
                            if (qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }).at(tip).0.distance(pp) <= 14.0 {
                                self.tools.cmd.drag = true;
                            }
                        }
                    }
                    // THE SECTION: grabbing the plane's offset arrow remembers the offset and the cursor AT THE
                    // MOMENT of the grab (an anchor), so that the drag counts as a delta rather than an absolute
                    // reprojection from o0 (which used to jump, adding the gizmo arrow's length to the offset at
                    // the moment of the grab)
                    if !self.tools.cmd.drag && self.side.section.plane.is_some() {
                        if let (Some(qymcad_ui_state::SectionGizmo { tip, .. }), Some(pp)) = (self.section_gizmo_geom(), resp.interact_pointer_pos()) {
                            if (qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }).at(tip).0.distance(pp) <= 14.0 {
                                self.side.section.drag = true;
                                self.side.section.drag_anchor = Some((self.side.section.offset, pp));
                            }
                        }
                    }
                    // grabbing a joint FREEDOM handle (a driven component OR a directly picked glyph - the root's GLOBAL)
                    if !self.tools.cmd.drag && !self.side.section.drag {
                        if let (Some(jid), Some(pp)) = (self.active_dof_joint(), resp.interact_pointer_pos()) {
                            if let Some((slot, ring)) = self.joint_handle_hit(jid, rect, basis3, pp) {
                                self.side.joint.giz_handle = Some((slot, ring));
                                qymcad_assembly::joint_giz_begin(&mut self.joint_ctx(), jid, slot, ring);
                            }
                        }
                        // GRABBING THE PART ITSELF: miss the thin arrow and the mechanism would not budge; a
                        // part should be grabbable anywhere.
                        if !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) && matches!(self.workbench, Workbench::Assembly) && !self.side.joint.pick_faces && self.side.joint.edit_repick.is_none() {
                            if let Some(pp) = resp.interact_pointer_pos() {
                                self.joint_grab_part_at(rect, pp, resp.drag_delta(), basis3);
                            }
                        }
                    }
                    // the placement gizmo of a FREE component: grabbing an axis or a rotation ring
                    if !self.tools.cmd.drag && !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) {
                        if let (Some(comp), Some(pp)) = (qymcad_ui_state::gizmo_component(&self.active_path, &self.project, self.chosen.sel, self.workbench), resp.interact_pointer_pos()) {
                            match self.comp_gizmo_mode(comp) {
                                CompGizmoMode::Joint(_) => {} // the joint handle was already handled above
                                CompGizmoMode::Free => {
                                    self.dragged.comp_giz.axis = self.gizmo_axis_hit(comp, rect, basis3, pp);
                                    if self.dragged.comp_giz.axis.is_none() {
                                        self.dragged.comp_giz.ring = self.gizmo_ring_hit(comp, rect, basis3, pp);
                                    }
                                    // pin the START transform and the gizmo origin for the duration of the drag (as
                                    // for a body), so the drag does not drift and there is a readout
                                    if self.dragged.comp_giz.axis.is_some() || self.dragged.comp_giz.ring.is_some() {
                                        let (o, _) = qymcad_ui_state::gizmo_geometry(self.viewing.cam, self.dragged.comp_giz, &self.project, comp);
                                        self.dragged.comp_giz.drag = Some((comp, self.project.component_transform(comp), o, 0.0));
                                        // THE EDIT BOUNDARY SPANS THE WHOLE DRAG, NOT EVERY FRAME.
                                        //
                                        // `begin_edit` takes a FULL COPY of the document. By opening and closing
                                        // the boundary inside every frame, `apply_comp_giz` used to take one on
                                        // every mouse movement: on a real assembly (138 bodies with meshes) that
                                        // is both the part following reluctantly and an undo stack filled with
                                        // one step per pixel. What is expected: grab a part, move it, and the
                                        // undo remembers nothing until it is dropped - an undo step per
                                        // coordinate change makes undo unusable.
                                        qymcad_ui_state::begin_edit(&mut self.disk.edits, &self.project, crate::i18n::tr("status-move-component"));
                                    }
                                }
                                CompGizmoMode::None => {}
                            }
                        }
                    }
                    // the BODY gizmo inside a Part: grabbing a translation axis or a rotation ring of the selected body
                    if !self.tools.cmd.drag && self.dragged.comp_giz.axis.is_none() && self.dragged.comp_giz.ring.is_none() && !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) {
                        if let (Some((_, mi)), Some(pp)) = (qymcad_ui_state::body_gizmo_target(qymcad_ui_state::body_view_of!(self), self.chosen.sel), resp.interact_pointer_pos()) {
                            let (o, l) = qymcad_ui_state::body_gizmo_geometry(&self.dragged.body_giz, self.viewing.cam, &self.project, &self.set, mi);
                            self.dragged.body_giz.axis = crate::gui::gizmo_axis_hit_at(&self.draw_ctx(), o, l, rect, basis3, pp);
                            self.dragged.body_giz.ring = if self.dragged.body_giz.axis.is_none() { crate::gui::gizmo_ring_hit_at(&self.draw_ctx(), o, l, rect, basis3, pp) } else { None };
                            self.dragged.body_giz.drag = (self.dragged.body_giz.axis.is_some() || self.dragged.body_giz.ring.is_some()).then_some((mi, o, 0.0));
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
                if self.side.section.drag {
                    // drag the SECTION PLANE along the normal as a DELTA from the anchor (the offset and cursor
                    // at the moment of the grab), rather than an absolute reprojection from o0 (which knew
                    // nothing of the offset already accumulated nor of the gizmo arrow's length, and so jumped
                    // on every fresh grab).
                    if resp.dragged() {
                        if let (Some((off0, p0)), Some((o0, _)), Some((_, n_eff)), Some(cur)) =
                            (self.side.section.drag_anchor, self.side.section.plane, qymcad_ui_state::section_eff(&self.side.section), resp.interact_pointer_pos())
                        {
                            let base = [o0[0], o0[1], o0[2]];
                            let s0 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }.at(base).0;
                            let s1 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }.at([base[0] + n_eff[0], base[1] + n_eff[1], base[2] + n_eff[2]]).0;
                            if let Some(new_off) = section_drag_delta_offset(off0, p0, s0, s1, cur) {
                                self.side.section.offset = new_off;
                                qymcad_ui_state::invalidate(&mut self.regen);
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.side.section.drag = false;
                        self.side.section.drag_anchor = None;
                    }
                } else if self.tools.cmd.drag {
                    // drag the active command's length along the normal (the cursor projected onto the screen axis)
                    if resp.dragged() {
                        if let (Some((base, dir, _)), Some(cur)) = (qymcad_ui_state::feat_cmd_axis(&self.tools.cmd, &self.tools.gsel, &self.project), resp.interact_pointer_pos()) {
                            let s0 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }.at(base).0;
                            let s1 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: basis3 }.at([base[0] + dir[0], base[1] + dir[1], base[2] + dir[2]]).0;
                            let pd = s1 - s0;
                            let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
                            if denom > 1e-6 {
                                let t = ((cur.x - s0.x) * pd.x + (cur.y - s0.y) * pd.y) as f64 / denom;
                                // A TWO-WAY drag: backwards (t<0, towards the negated normal) reverses the
                                // direction (flip); the magnitude is |t|. The preview, the arrow and the rebuild
                                // all take flip, so it pulls BOTH ways.
                                self.feat.set_flip(t < 0.0); // the direction is set by the gizmo drag
                                if let Some(p) = self.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
                                    p.val = t.abs().max(0.1);
                                    p.txt = format!("{:.2}", p.val);
                                }
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.tools.cmd.drag = false;
                    }
                } else if self.dragged.face_arrow_drag.is_some() {
                    // DRAGGING A FACE WITH THE MOUSE: the offset grows along the normal, and the field and the arrow show one value
                    if resp.dragged() {
                        self.face_arrow_drag_to(resp.drag_delta(), rect, basis3);
                    }
                    if resp.drag_stopped() {
                        self.dragged.face_arrow_drag = None;
                    }
                } else if qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) {
                    // dragging a joint FREEDOM pulls the parameter (angle/offset/offset2), and solve_joints lays out the rest
                    self.dragged.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let Some(cur) = resp.interact_pointer_pos() {
                            qymcad_assembly::joint_giz_drag_to(&mut self.joint_ctx(), cur, resp.drag_delta(), rect, basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        qymcad_assembly::joint_giz_end(&mut self.joint_ctx()); // the end of a continuing operation: an undo step + a rebuild of the consumers
                    }
                } else if self.dragged.comp_giz.axis.is_some() {
                    // moving a component along the grabbed gizmo axis (unified with a body: a pinned origin + snap)
                    self.dragged.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let (Some(comp), Some(ax)) = (qymcad_ui_state::gizmo_component(&self.active_path, &self.project, self.chosen.sel, self.workbench), self.dragged.comp_giz.axis) {
                            self.drag_component_axis(comp, ax, resp.drag_delta(), rect, basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.dragged.comp_giz.axis = None;
                        self.dragged.comp_giz.drag = None;
                        qymcad_ui_state::commit_edit(&mut self.rebuild_ctx()); // ONE undo step for the whole drag (opened when the gizmo was grabbed)
                        qymcad_ui_state::after_placement_change(&mut self.rebuild_ctx()); // the source moved, so the consumers are rebuilt
                    }
                } else if self.dragged.comp_giz.ring.is_some() {
                    // rotating a component by the grabbed gizmo ring (unified with a body)
                    self.dragged.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if resp.dragged() {
                        if let (Some(comp), Some(ax), Some(cur)) = (qymcad_ui_state::gizmo_component(&self.active_path, &self.project, self.chosen.sel, self.workbench), self.dragged.comp_giz.ring, resp.interact_pointer_pos()) {
                            self.drag_component_ring(comp, ax, cur, resp.drag_delta(), rect, basis3);
                        }
                    }
                    if resp.drag_stopped() {
                        self.dragged.comp_giz.ring = None;
                        self.dragged.comp_giz.drag = None;
                        qymcad_ui_state::commit_edit(&mut self.rebuild_ctx()); // ONE undo step for the whole drag (opened when the gizmo was grabbed)
                        qymcad_ui_state::after_placement_change(&mut self.rebuild_ctx()); // the source rotated, so the consumers are rebuilt
                    }
                } else if self.dragged.body_giz.axis.is_some() && self.dragged.body_giz.drag.is_some() {
                    // moving a BODY along the grabbed gizmo axis: it accumulates and is committed as a Move feature on release
                    self.dragged.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
                    if resp.dragged() {
                        crate::gui::body_gizmo_axis_drag(&mut self.dragged.body_giz, &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }, &self.project, &mut self.regen, resp.drag_delta());
                    }
                    if resp.drag_stopped() {
                        self.commit_body_gizmo(self.dragged.body_giz.snap);
                    }
                } else if self.dragged.body_giz.ring.is_some() && self.dragged.body_giz.drag.is_some() {
                    // rotating a BODY by the grabbed gizmo ring
                    self.dragged.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
                    if resp.dragged() {
                        if let Some(cur) = resp.interact_pointer_pos() {
                            crate::gui::body_gizmo_ring_drag(&mut self.dragged.body_giz, &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }, &self.project, &mut self.regen, cur, resp.drag_delta());
                        }
                    }
                    if resp.drag_stopped() {
                        self.commit_body_gizmo(self.dragged.body_giz.snap);
                    }
                } else if self.set.mouse_nav.pan().active(ctx, resp) {
                    let d = nav_delta(ctx, resp, &self.set.mouse_nav.pan());
                    let (right, up, _) = self.viewing.cam.basis();
                    let k = 1.0 / self.viewing.cam.scale as f64;
                    for a in 0..3 {
                        self.viewing.cam.target[a] -= d.x as f64 * right[a] * k;
                        self.viewing.cam.target[a] += d.y as f64 * up[a] * k;
                    }
                } else if self.set.mouse_nav.rotate().active(ctx, resp) {
                    let d = nav_delta(ctx, resp, &self.set.mouse_nav.rotate());
                    self.viewing.cam.yaw -= d.x as f64 * 0.01;
                    self.viewing.cam.pitch = (self.viewing.cam.pitch + d.y as f64 * 0.01).clamp(-1.5, 1.5);
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
                        if self.dragged.body_giz.num.is_some() {
                            // no-op: the popup commits itself
                        }
                        // a body-to-body boolean: the click names body B - see `take_boolean_pick`
                        else if self.params.boolean.pick.is_some() {
                            let hit = crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi));
                            qymcad_part::take_boolean_pick(&mut self.part_ctx(), hit);
                        }
                        // A WAITING TOOL: the click names the sketch. High in the chain - see `name_the_sketch`.
                        else if self.tools.picking.sketch_for().is_some() {
                            let hit = crate::gui::pick::sketch_at_3d(&self.painting(), rect, pos);
                            qymcad_part::name_the_sketch(&mut self.part_ctx(), hit);
                        }
                        // a click on the ViewCube snaps the view
                        // the ViewCube has 26 zones (faces, edges, corners) + a home button; the turn is smooth
                        else if self.viewcube_click(rect, pos) {
                        } else if self.side.joint.axis_pick.is_some() {
                            // THE SECOND PICK: a person points at what the anchor's secondary axis runs along.
                            // ONE door is asked, and it takes only what is under the cursor.
                            match self.infer_axis_anchor(rect, pos) {
                                Some((_, anchor)) => qymcad_assembly::joint_axis_pick_apply(&mut self.joint_ctx(), anchor),
                                None => self.status = crate::i18n::tr("j-axis-miss"),
                            }
                        } else if self.side.joint.edit_repick.is_some() {
                            // Editing a mate: the NEW anchor A or B is inferred under the cursor, as when creating one
                            self.joint_repick_inferred_click(rect, pos);
                        } else if self.side.joint.tangent_pick.is_some() {
                            // The Tangent tool: TWO SURFACES are pointed at, and there are no connectors.
                            match self.pick_part_face_at(rect, pos) {
                                Some((body, key)) => qymcad_assembly::tangent_pick_click(&mut self.joint_ctx(), body, key),
                                None => self.status = crate::i18n::tr("vp-miss-face"),
                            }
                        } else if self.side.joint.width_pick.is_some() {
                            // The Width tool: FACES are pointed at - two walls and the part between them.
                            match self.pick_part_face_at(rect, pos) {
                                Some((body, key)) => qymcad_assembly::width_pick_click(&self.active_path, &mut self.side.joint, &mut self.project, &mut self.status, body, key),
                                None => self.status = crate::i18n::tr("vp-miss-face"),
                            }
                        } else if self.side.joint.group_pick.is_some() {
                            // The Group tool: a click on a part adds it to the set or removes it.
                            match crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                                Some(body) => qymcad_assembly::group_pick_click(&self.active_path, &mut self.side.joint, &mut self.project, &mut self.status, body),
                                None => self.status = crate::i18n::tr("j-body-miss"),
                            }
                        } else if self.side.joint.ground_pick {
                            // the Ground tool: a click on a part fixes it or releases it
                            match crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                                Some(body) => qymcad_assembly::joint_pick_ground_click(&mut self.joint_ctx(), body),
                                None => self.status = crate::i18n::tr("vp-miss-part-ground"),
                            }
                        } else if self.side.joint.pick_faces || self.side.joint.conn_pick {
                            // picking an anchor: either for a mate connector (A, then B on another part) or for a
                            // STANDALONE anchor - the parsing is the same, only what happens on the click differs,
                            // and `joint_pick_anchor_click` decides that.
                            // THE KIND OF ANCHOR IS INFERRED UNDER THE CURSOR rather than declared in advance.
                            self.joint_pick_inferred_click(rect, pos);
                        } else if let Some(src_comp) = self.params.mirror.part {
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
                                    let wt = self.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&self.active_path, &self.project));
                                    Some((qymcad_core::feature::apply12(&wt, key.centroid), qymcad_core::feature::apply12_dir(&wt, key.normal)))
                                }
                            });
                            match plane {
                                Some((o, n)) => {
                                    // the click coordinates are in the current context's frame -> into the WORLD (the root)
                                    let cwt = self.project.world_transform(qymcad_ui_state::current_ctx_id(&self.active_path, &self.project));
                                    let (wo, wn) = (qymcad_core::feature::apply12(&cwt, o), qymcad_core::feature::apply12_dir(&cwt, n));
                                    let cnt = self.project.add_mirror_component(src_comp, wo, wn).len();
                                    self.params.mirror.part = None;
                                    qymcad_ui_state::mark_dirty_for_rebuild(&mut self.rebuild_ctx()); // the document is marked; the scheduler does the computing
                                    self.status = crate::i18n::tr1("vp-mirror-created", "n", &cnt.to_string());
                                }
                                None => {
                                    self.status = crate::i18n::tr("vp-miss-mirror-plane");
                                }
                            }
                        } else if self.side.section.pick {
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
                                    let wt = self.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&self.active_path, &self.project));
                                    Some((qymcad_core::feature::apply12(&wt, key.centroid), qymcad_core::feature::apply12_dir(&wt, key.normal)))
                                }
                            });
                            match plane {
                                Some((o, n)) => {
                                    self.side.section.plane = Some((o, n));
                                    self.side.section.offset = 0.0;
                                    self.side.section.rot = [0.0, 0.0];
                                    self.side.section.pick = false;
                                    qymcad_ui_state::invalidate(&mut self.regen);
                                    self.status = crate::i18n::tr("vp-section-on");
                                }
                                None => {
                                    self.status = crate::i18n::tr("vp-miss-plane-datum-face");
                                }
                            }
                        } else if let Some(target) = self.tools.picking.plane_face() {
                            // a datum plane offset from a face: a click on a part's face
                            if let Some(qymcad_core::feature::SketchPlane::Face(body, key)) = self.pick_sketch_plane_at(rect, pos) {
                                self.make_offset_plane_from_face(target, body, key);
                                self.tools.picking.set_plane_face(None);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-face-only");
                            }
                        } else if self.tools.pending_import.curves.is_some() {
                            // placing an imported DXF or SVG: the click sets the sketch plane
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                self.place_pending_import(sp);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-place-import");
                            }
                        } else if let Some(si) = self.tools.picking.replace_sketch() {
                            // RE-placing a sketch: the click sets a new plane and the bodies are rebuilt
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                self.set_sketch_plane(si, sp);
                            } else {
                                self.status = crate::i18n::tr("vp-miss-plane");
                            }
                        } else if self.tools.picking.is_sketch_plane() {
                            // choosing a plane or a face for a new sketch by a click in the viewport
                            if let Some(sp) = self.pick_sketch_plane_at(rect, pos) {
                                // the face corner (for binding the origin) is picked BEFORE entering the sketch -
                                // otherwise body_shown hides the neighbour's body and pick_vertex_pos would not find
                                // it, which would shift the origin.
                                let corner_w = crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos).or_else(|| crate::gui::pick::pick_edge_point(&self.painting(), rect, pos));
                                self.tools.picking.clear();
                                let si = self.create_sketch_on(sp);
                                // binding the origin to an edge or a vertex. The snap is computed from the RESOLVED
                                // plane (create_sketch_on may have replaced a neighbour's face with a datum copy).
                                let resolved = self.project.sketches[si].plane;
                                if let (Some(w), Some(fr)) = (corner_w, qymcad_ui_state::world_frame_of_plane(&self.draw_ctx(), &resolved)) {
                                    self.project.sketches[si].origin_uv = Some(fr.project(qymcad_core::geom::Point3::new(w[0], w[1], w[2])));
                                    self.status = crate::i18n::tr("vp-sketch-origin-bound");
                                }
                            } else {
                                self.status = crate::i18n::tr("vp-miss-plane");
                            }
                        } else if self.tools.armed.cmd_kind() == 16 {
                            // MIRROR: the click picks the mirror PLANE, datum or face (Enter applies it)
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.params.mirror.plane = Some(sp);
                                    self.status = crate::i18n::tr("vp-mirror-plane-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.tools.armed.cmd_kind() == 27 || self.tools.armed.cmd_kind() == 29 {
                            // SPLIT BODY: the click picks the cutting PLANE, datum or face (Enter applies it)
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.params.split.plane = Some(sp);
                                    let src = qymcad_ui_state::op_target_body(&self.draw_ctx(), self.chosen.sel).unwrap_or(0);
                                    self.status = match crate::gui::commands::split_piece_count(&mut self.part_ctx(), src) {
                                        // how many pieces will come out is visible AT ONCE: otherwise "it cuts
                                        // nothing" would only be learnt on Enter, after a click and an offset
                                        // had already been spent
                                        Some(n) if n >= 2 => crate::i18n::tr1("vp-cut-plane-picked", "n", &n.to_string()),
                                        _ => crate::i18n::tr("vp-plane-cuts-nothing"),
                                    };
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.tools.armed.cmd_kind() == 20 {
                            // DATUM PLANE: the click picks the base plane, datum or face the offset is measured from
                            match self.pick_sketch_plane_at(rect, pos) {
                                Some(sp) => {
                                    self.side.datum.plane_pick = Some(sp);
                                    self.status = crate::i18n::tr("vp-ref-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-plane-short"),
                            }
                        } else if self.tools.armed.cmd_kind() == 21 && self.side.datum.pt_mode == 1 {
                            // DATUM POINT, "at a vertex": a click on a vertex of the current part makes an ASSOCIATIVE reference
                            let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
                            match self.pick_vertex_any(rect, pos).filter(|(b, _, _)| self.project.body_owner(*b) == Some(ctx)) {
                                Some((body, edge, end)) => {
                                    let at = crate::gui::vertex_local_pos(&self.live, body, edge, end).unwrap_or([0.0; 3]);
                                    self.side.datum.pt_vert = Some((body, edge, end, at));
                                    self.status = crate::i18n::tr("vp-vertex-picked");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-vertex-current"),
                            }
                        } else if self.tools.armed.cmd_kind() == 21 {
                            // DATUM POINT, "coordinates": a click on a vertex snaps X/Y/Z to its coordinates (once, not associatively)
                            match crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos) {
                                Some(w) => {
                                    for (k, key) in ["x", "y", "z"].iter().enumerate() {
                                        if let Some(p) = self.tools.cmd.params.iter_mut().find(|p| &p.key == key) {
                                            p.txt = format!("{:.3}", w[k]);
                                            p.val = w[k];
                                        }
                                    }
                                    self.status = crate::i18n::tr("vp-point-bound");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-vertex-or-xyz"),
                            }
                        } else if self.tools.armed.cmd_kind() == 22 && self.side.datum.axis_mode == 0 {
                            // DATUM AXIS: a click on a straight edge or a cylindrical face makes a reference (Enter creates it ASSOCIATIVELY)
                            match crate::gui::pick::pick_axis_at(&self.painting(), rect, pos) {
                                Some(h) => match crate::gui::axis_ref_world(&self.active_path, &self.edges, &self.live, &self.project, h) {
                                    Some(od) => {
                                        self.side.datum.axis_hit = Some(h);
                                        self.side.datum.axis_ref = Some(od);
                                        self.status = crate::i18n::tr("vp-axis-picked");
                                    }
                                    None => self.status = crate::i18n::tr("vp-ref-gives-no-axis"),
                                },
                                None => self.status = crate::i18n::tr("vp-miss-edge-or-cyl"),
                            }
                        } else if self.tools.armed.cmd_kind() == 22 && self.side.datum.axis_mode == 2 {
                            // DATUM AXIS BY TWO POINTS: two datum points or vertices are gathered; datum points make it parametric
                            let hit = self.pick_datum_point_at(rect, pos).or_else(|| crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos).map(|w| (0, w)));
                            match hit {
                                Some(pt) => {
                                    if self.side.datum.axis_pts.len() >= 2 {
                                        self.side.datum.axis_pts.clear();
                                    }
                                    self.side.datum.axis_pts.push(pt);
                                    if self.side.datum.axis_pts.len() == 2 {
                                        let (a, b) = (self.side.datum.axis_pts[0].1, self.side.datum.axis_pts[1].1);
                                        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                                        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                                        self.side.datum.axis_ref = (l > 1e-9).then(|| (a, [d[0] / l, d[1] / l, d[2] / l]));
                                        self.status = crate::i18n::tr("vp-two-points-set");
                                    } else {
                                        self.status = crate::i18n::tr("vp-point1-set");
                                    }
                                }
                                None => self.status = crate::i18n::tr("vp-miss-datum-point"),
                            }
                        } else if (10..=15).contains(&self.tools.armed.cmd_kind()) && self.tools.cmd.edit.is_none() {
                            // A PRIMITIVE: a click on a vertex, a datum point, a plane or a face places it (with an orientation)
                            match crate::gui::pick::pick_place_frame_at(&self.painting(), rect, pos) {
                                Some(m) => {
                                    self.params.prim.frame = Some(m);
                                    self.params.prim.place = Some([m[3], m[7], m[11]]);
                                    self.status = crate::i18n::tr("vp-placement-set");
                                }
                                None => self.status = crate::i18n::tr("vp-miss-placement"),
                            }
                        } else if self.tools.armed.cmd_kind() == 18 && self.params.arr.axis_pick {
                            // A CIRCULAR PATTERN: the click picks the AXIS of rotation - a datum axis or a straight edge of the body
                            match crate::gui::pick::pick_axis_at(&self.painting(), rect, pos) {
                                Some(AxisHit::Datum(id)) => {
                                    self.params.arr.axis = id;
                                    self.params.arr.axis_pick = false;
                                    self.status = crate::i18n::tr("vp-array-axis-datum");
                                }
                                Some(AxisHit::Edge(i)) => match crate::gui::axis_from_edge(&self.active_path, &self.edges, &self.live, &mut self.project, i) {
                                    Some(id) => {
                                        self.params.arr.axis = id;
                                        self.params.arr.axis_pick = false;
                                        self.status = crate::i18n::tr("vp-array-axis-edge");
                                    }
                                    None => self.status = crate::i18n::tr("vp-edge-not-axis"),
                                },
                                Some(AxisHit::Face(body, fid)) => match crate::gui::axis_from_face(&self.active_path, &self.edges, &self.live, &mut self.project, body, fid) {
                                    Some(id) => {
                                        self.params.arr.axis = id;
                                        self.params.arr.axis_pick = false;
                                        self.status = crate::i18n::tr("vp-array-axis-cyl");
                                    }
                                    None => self.status = crate::i18n::tr("vp-face-has-no-axis"),
                                },
                                None => self.status = crate::i18n::tr("vp-miss-axis"),
                            }
                        } else if self.tools.armed.cmd_kind() == 3 && self.params.rev.pick_axis {
                            // the REVOLVE axis by a click in 3D - in exactly the same place as the circular pattern's axis.
                            self.rev_axis_pick_click(rect, pos);
                        } else if let Some(jid) = qymcad_assembly::joint_glyph_at(&mut self.joint_ctx(), rect, pos) {
                            self.chosen.sel = Sel::Joint(jid); // a click on a mate glyph selects it
                        } else if self.tools.armed.cmd_kind() == 5 && self.params.chamfer.pick_ref && self.params.chamfer.mode != qymcad_core::feature::ChamferMode::Symmetric {
                            // picking the chamfer's REFERENCE FACE: a click on a face sets chamfer_ref_face, and a
                            // second click on the same face clears it. A miss changes nothing.
                            match crate::gui::pick::pick_face_persist_id(&self.painting(), rect, pos) {
                                Some(fid) => {
                                    self.params.chamfer.ref_face = if self.params.chamfer.ref_face == fid { 0 } else { fid };
                                    self.params.chamfer.pick_ref = false;
                                    self.status = if self.params.chamfer.ref_face != 0 { crate::i18n::tr("vp-chamfer-ref-set") } else { crate::i18n::tr("vp-chamfer-ref-cleared") };
                                }
                                None => self.status = crate::i18n::tr("vp-miss-body-face"),
                            }
                        } else if self.side.m3.on {
                            // MEASURING IN 3D: the click picks a vertex, an edge or a face; the second one computes
                            self.measure_3d_click(rect, pos);
                        } else if matches!(self.tools.armed.cmd_kind(), 4 | 5 | 32) && (self.pick_edge_3d(rect, pos) || self.pick_face_edges_fillet(rect, pos)) {
                            // a click on an edge OR on a FACE (a face takes all of its edges) - ONLY under
                            // Chamfer/Fillet (otherwise an ordinary click would pick an edge or a face instead of a
                            // body, and leave a stray orange selection)
                        } else if self.tools.armed.cmd_kind() == 24 {
                            // THREAD: a click on a cylindrical face gives the body and the rim (axis and radius) + inner/outer
                            self.pick_thread_target(rect, pos);
                        } else if let Some((mi, ax, rot)) = self.body_gizmo_click_hit(rect, pos, basis3) {
                            // A CLICK (with no drag) on the body gizmo's arrow or ring opens precise numeric entry at the geometry
                            self.dragged.body_giz.num = Some((mi, ax, rot));
                            self.dragged.body_giz.num_buf.clear();
                            self.dragged.body_giz.num_focus = true;
                        } else {
                            self.pick_face_3d(rect, pos);
                        }
                    }
                }
    }
}
