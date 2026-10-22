//! THE 3D VIEWPORT: what the mouse does, in phases of its own - what was grabbed -> drive what was grabbed
//! -> a click by mode.
//!
//! Split out of `render.rs`: drawing and reading the mouse are different duties with different checks, and a
//! frame test for a click had to be written past three thousand lines of painting. The same skeleton as the
//! sketch viewport in `sketching.rs`, so one shape reads the same way everywhere.

use qymcad_ui_state::Notice;
use super::*;

// THE 3D VIEWPORT moved here from `gui.rs`, with phases of its own: what was grabbed -> drive what was
// grabbed -> a click by mode -> drawing. The same skeleton as the sketch viewport in `sketching.rs`: one shape
// reads the same way everywhere and does not have to be reconstructed on the spot.
/// HOW FAR THE POINTER MOVED FOR A NAVIGATION GESTURE.
///
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
            self.side.joint.giz_handle = match (qymcad_pick::active_dof_joint(&self.painting()), resp.hover_pos().filter(|p| rect.contains(*p))) {
                (Some(jid), Some(p)) => self.joint_handle_hit(jid, rect, &basis3, p),
                _ => None,
            };
        }
        // grabbing the extrude arrow's gizmo handle (before the camera orbit)
        // A FRAME FROM EMPTY SPACE; else what was grabbed (command arrow -> section -> gizmo -> orbit) and its drag
        let framed = crate::gui::frame_select::frame(&self.painting(), ctx, resp, painter, rect);
        if crate::gui::frame_select::none_or_take(framed, &mut self.chosen, &self.project, self.workbench) {
            self.viewport_3d_drag_start(resp, rect, &basis3);
            self.viewport_3d_drag_update(ctx, resp, rect, &basis3);
        }
        if scroll != 0.0 && resp.contains_pointer() {
            let part = crate::gui::commands::cmd_spot(&mut self.part_ctx(), rect); // where the open command's fields stand
            self.viewing.cam = crate::gui::look_at_point::wheeled(&self.painting(), rect, qymcad_ui_state::zoom_anchor(&self.set, rect, resp.hover_pos(), part), scroll);
        }
        if let Some(p) = crate::gui::look_at_point::look_at(&self.painting(), self.set.mouse_nav, resp, rect) {
            crate::gui::animate_look_to(self.viewing.cam, &mut self.viewing.view_anim, p);
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
                let edge = self
                    .tools
                    .armed
                    .part_tool()
                    .is_some_and(qymcad_ui_state::PartTool::blends_edges)
                    .then(|| crate::gui::pick::edge_at(&self.active_path, self.viewing.cam, &self.edges, &self.project, &self.set, rect, pos))
                    .flatten();
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
        if let Some(cut) = crate::gui::frame_select::menu(resp, &self.chosen.tree_sel.multi) {
            self.clipboard_copy(cut);
        } // several parts taken by a frame
        if let Some((body, name)) = crate::gui::piece_part::menu(&self.painting(), resp, rect) {
            qymcad_part::piece_to_part(&mut self.part_ctx(), body, name);
        } // a piece a cut or a split left
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
        self.viewing.view_dragging = resp.dragged() || (scroll != 0.0 && resp.contains_pointer());
        if self.viewing.view_dragging {
            ctx.request_repaint();
        }
        crate::gui::refresh_interference(qymcad_ui_state::body_view_of!(self), qymcad_ui_state::scene_drag_of!(self), &mut self.interference, &self.live, &self.set, self.workbench); // recompute the intersections on the 3D path, where the assembly lives
        self.draw_3d(painter, rect);
        crate::gui::viewcube::draw(&self.cube_ctx(), painter, rect);
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
            if let Some(pp) = resp.interact_pointer_pos() {
                let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 };
                self.dragged.handle_drag = qymcad_ui_state::grab_handle(&resp.ctx, &self.painting(), &scr, pp).or(self.dragged.handle_drag);
            }
            // priority goes to the ACTIVE command's arrow (extrude, cut, ...) when a profile is picked
            if self.tools.armed.commanding() && !self.tools.gsel.profiles.is_empty() {
                if let Some(pp) = resp.interact_pointer_pos() {
                    qymcad_ui_state::grab_cmd_arrow(
                        &resp.ctx,
                        &mut self.tools.cmd,
                        &self.tools.gsel,
                        &self.project,
                        self.feat.flip,
                        &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 },
                        pp,
                    );
                }
            }
            // THE SECTION: grabbing the plane's offset arrow remembers the offset and the cursor AT THE
            // MOMENT of the grab (an anchor), so that the drag counts as a delta rather than an absolute
            // reprojection from o0 (which used to jump, adding the gizmo arrow's length to the offset at
            // the moment of the grab)
            if self.tools.cmd.drag.is_none() && self.side.section.plane.is_some() {
                if let (Some(qymcad_ui_state::SectionGizmo { tip, .. }), Some(pp)) = (self.section_gizmo_geom(), resp.interact_pointer_pos()) {
                    if (qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }).at(tip).0.distance(pp) <= 14.0 {
                        self.side.section.drag = true;
                        self.side.section.drag_anchor = Some((self.side.section.offset, pp));
                    }
                }
            }
            // grabbing a joint FREEDOM handle (a driven component OR a directly picked glyph - the root's GLOBAL)
            if self.tools.cmd.drag.is_none() && !self.side.section.drag {
                if let (Some(jid), Some(pp)) = (qymcad_pick::active_dof_joint(&self.painting()), resp.interact_pointer_pos()) {
                    if let Some((slot, ring)) = self.joint_handle_hit(jid, rect, basis3, pp) {
                        self.side.joint.giz_handle = Some((slot, ring));
                        qymcad_assembly::joint_giz_begin(&mut self.joint_ctx(), jid, slot, ring);
                    }
                }
            }
            // the placement gizmo of a FREE component: grabbing an axis or a rotation ring
            if self.tools.cmd.drag.is_none() && !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull) {
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
            // GRABBING THE PART ITSELF, anywhere, once no handle of a gizmo drawn over it took the press (the arm
            // of a free part's gizmo lies over the part, and the part used to take the drag meant for it).
            if self.tools.cmd.drag.is_none()
                && !self.side.section.drag
                && self.dragged.comp_giz.drag.is_none()
                && !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull)
                && matches!(self.workbench, Workbench::Assembly)
                && !qymcad_assembly::joint_picking(&self.side.joint)
                && self.side.joint.edit_repick.is_none()
                && self.set.mouse_nav.take_a_part().active(&resp.ctx, resp)
            {
                if let Some(pp) = resp.interact_pointer_pos() {
                    self.joint_grab_part_at(rect, pp, resp.drag_delta(), basis3);
                }
            }
            // the BODY gizmo inside a Part: grabbing a translation axis or a rotation ring of the selected body
            if self.tools.cmd.drag.is_none()
                && self.dragged.comp_giz.axis.is_none()
                && self.dragged.comp_giz.ring.is_none()
                && !qymcad_assembly::joint_drag_active(&self.side.joint, &self.dragged.part_pull)
            {
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
                    let s0 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }.at(base).0;
                    let s1 = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }.at([base[0] + n_eff[0], base[1] + n_eff[1], base[2] + n_eff[2]]).0;
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
        } else if self.tools.cmd.drag.is_some() {
            // drag the arrow of the active command taken at the press
            if let (true, Some(cur)) = (resp.dragged(), resp.interact_pointer_pos()) {
                qymcad_ui_state::drag_cmd_arrow(
                    &mut self.tools.cmd,
                    &self.tools.gsel,
                    &self.project,
                    &mut self.feat,
                    &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 },
                    cur,
                );
            }
            if resp.drag_stopped() {
                self.tools.cmd.drag = None;
            }
        } else if self.dragged.handle_drag.is_some() {
            // DRAGGING A HANDLE OF THE COMMAND: the field and the arrow or the ring show one value
            if let (true, Some(at)) = (resp.dragged(), resp.interact_pointer_pos()) {
                let hs = qymcad_ui_state::handles(&self.painting());
                let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 };
                qymcad_ui_state::drag_handle(&mut self.tools.cmd, &mut self.regen, &hs, self.dragged.handle_drag, &scr, qymcad_ui_state::PointerStep { at, by: resp.drag_delta() });
            }
            if resp.drag_stopped() {
                self.dragged.handle_drag = None;
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
                qymcad_ui_state::after_placement_change(&mut self.rebuild_ctx());
                // the source moved, so the consumers are rebuilt
            }
        } else if self.dragged.comp_giz.ring.is_some() {
            // rotating a component by the grabbed gizmo ring (unified with a body)
            self.dragged.comp_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
            if resp.dragged() {
                if let (Some(comp), Some(ax), Some(cur)) =
                    (qymcad_ui_state::gizmo_component(&self.active_path, &self.project, self.chosen.sel, self.workbench), self.dragged.comp_giz.ring, resp.interact_pointer_pos())
                {
                    self.drag_component_ring(comp, ax, cur, resp.drag_delta(), rect, basis3);
                }
            }
            if resp.drag_stopped() {
                self.dragged.comp_giz.ring = None;
                self.dragged.comp_giz.drag = None;
                qymcad_ui_state::commit_edit(&mut self.rebuild_ctx()); // ONE undo step for the whole drag (opened when the gizmo was grabbed)
                qymcad_ui_state::after_placement_change(&mut self.rebuild_ctx());
                // the source rotated, so the consumers are rebuilt
            }
        } else if self.dragged.body_giz.axis.is_some() && self.dragged.body_giz.drag.is_some() {
            // moving a BODY along the grabbed gizmo axis: it accumulates and is committed as a Move feature on release
            self.dragged.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
            if resp.dragged() {
                crate::gui::body_gizmo_axis_drag(
                    &mut self.dragged.body_giz,
                    &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 },
                    &self.project,
                    &mut self.regen,
                    resp.drag_delta(),
                );
            }
            if resp.drag_stopped() {
                self.commit_body_gizmo(self.dragged.body_giz.snap);
            }
        } else if self.dragged.body_giz.ring.is_some() && self.dragged.body_giz.drag.is_some() {
            // rotating a BODY by the grabbed gizmo ring
            self.dragged.body_giz.snap = self.set.snap.on || ctx.input(|i| i.modifiers.ctrl || i.modifiers.command); // the Snap panel or Ctrl
            if resp.dragged() {
                if let Some(cur) = resp.interact_pointer_pos() {
                    crate::gui::body_gizmo_ring_drag(
                        &mut self.dragged.body_giz,
                        &qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 },
                        &self.project,
                        &mut self.regen,
                        cur,
                        resp.drag_delta(),
                    );
                }
            }
            if resp.drag_stopped() {
                self.commit_body_gizmo(self.dragged.body_giz.snap);
            }
        } else if let Some(pan) = qymcad_ui_state::pan_now(self.set.mouse_nav, ctx, resp) {
            self.viewing.cam = crate::gui::orbit_about::panned(&self.painting(), rect, ctx, resp, pan);
        } else {
            qymcad_ui_state::turn_view(crate::gui::orbit_about::pivot(&self.painting(), rect, ctx, resp), &mut self.viewing.cam, self.set.mouse_nav, ctx, resp);
            // the layout's turns and tilts, about the centre the setting names
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
        // WHAT WAS PICKED BEFORE THE TOOL IS HANDED TO IT, as clicks of its own where those places stand on screen now
        let held = crate::gui::panel_places::tool_in_hand(self);
        let before: Vec<egui::Pos2> =
            self.tools.gsel.before.hand_over(held).into_iter().map(|w| qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: basis3 }.at(w).0).collect();
        for at in before {
            self.viewport_3d_click_at(at, rect, basis3);
        }
        if !resp.clicked() || !self.set.mouse_nav.click_takes(&resp.ctx) {
            return;
        }
        let Some(pos) = resp.interact_pointer_pos() else { return };
        // the cube first, clear of the bars over the view: a control over the view turns it whatever is in hand
        if self.viewcube_click(qymcad_shell::clear_of_overs(&resp.ctx, rect), pos) {
            return;
        }
        let was = self.chosen.sel;
        let adding = if resp.ctx.input(|i| i.modifiers.shift) { qymcad_ui_state::Adding::Yes } else { qymcad_ui_state::Adding::No };
        // SHIFT WITH NOTHING IN HAND ADDS WHAT IS UNDER THE POINTER to what is picked - the face under the gizmo of the
        // part picked too, which a plain click there opens the numeric move of
        match (held, adding) {
            (qymcad_ui_state::InHand::Nothing, qymcad_ui_state::Adding::Yes) => self.pick_face_3d(rect, pos),
            _ => self.viewport_3d_click_at(pos, rect, basis3),
        }
        // a click that opened the numeric entry of a gizmo is a click on the gizmo, not on the model under it
        if held == qymcad_ui_state::InHand::Nothing && self.dragged.body_giz.num.is_none() {
            self.tools.gsel.before.note(qymcad_pick::place_under(&self.painting(), rect, pos), adding);
            let under = crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi));
            qymcad_ui_state::hold_bodies(&self.project, was, under, &mut self.chosen, adding);
        }
        qymcad_part::whole_body_on_double_click(&mut self.part_ctx(), resp.double_clicked());
        // the second click of two takes the body
    }

    /// THE ACTION AT A POINT - the same thing the mouse does, but without an `egui` event.
    ///
    /// The split exists for the tests that act "by a person's hand": faking a `Response` would mean checking
    /// one's own fake, and calling the pick functions directly would bypass the whole parsing that lives here
    /// (what outranks what, which tool is open, what was hit). A test must touch the program in exactly the
    /// same way a person does, or it is not the program it checks.
    pub(super) fn viewport_3d_click_at(&mut self, pos: egui::Pos2, rect: Rect, basis3: &([f64; 3], [f64; 3], [f64; 3])) {
        let aim = qymcad_part::aim(&self.part_ctx()); // what the tool of a part in hand aims at
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
                // an extrusion or a revolution in hand: the click picks a contour - see `profile_click_3d`
                else if self.tools.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::picks_profiles) && self.tools.cmd.sketch.is_some() {
                    qymcad_part::profile_click_3d(&mut self.part_ctx(), rect, pos);
                }
                // A WAITING TOOL: the click names the sketch. High in the chain - see `name_the_sketch`.
                else if self.tools.picking.sketch_for().is_some() {
                    let hit = crate::gui::pick::sketch_at_3d(&self.painting(), rect, pos);
                    qymcad_part::name_the_sketch(&mut self.part_ctx(), hit);
                } else if self.side.joint.axis_pick.is_some() {
                    // THE SECOND PICK: a person points at what the anchor's secondary axis runs along.
                    // ONE door is asked, and it takes only what is under the cursor.
                    match self.infer_axis_anchor(rect, pos) {
                        Some((_, anchor)) => qymcad_assembly::joint_axis_pick_apply(&mut self.joint_ctx(), anchor),
                        None => self.status = Notice::refusal(crate::i18n::tr("j-axis-miss")),
                    }
                } else if self.side.joint.edit_repick.is_some() {
                    // Editing a mate: the NEW anchor A or B is inferred under the cursor, as when creating one
                    self.joint_repick_inferred_click(rect, pos);
                } else if self.side.joint.tangent_pick.is_some() {
                    // The Tangent tool: TWO SURFACES are pointed at, and there are no connectors.
                    match self.pick_part_face_at(rect, pos) {
                        Some((body, key)) => qymcad_assembly::tangent_pick_click(&mut self.joint_ctx(), body, key),
                        None => self.status = Notice::refusal(crate::i18n::tr("vp-miss-face")),
                    }
                } else if self.side.joint.width_pick.is_some() {
                    // The Width tool: FACES are pointed at - two walls and the part between them.
                    match self.pick_part_face_at(rect, pos) {
                        Some((body, key)) => qymcad_assembly::width_pick_click(&self.active_path, &mut self.side.joint, &mut self.project, &mut self.status, body, key),
                        None => self.status = Notice::refusal(crate::i18n::tr("vp-miss-face")),
                    }
                } else if self.side.joint.group_pick.is_some() {
                    // The Group tool: a click on a part adds it to the set or removes it.
                    match crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi)) {
                        Some(body) => qymcad_assembly::group_pick_click(&self.active_path, &mut self.side.joint, &mut self.project, &mut self.status, body),
                        None => self.status = Notice::refusal(crate::i18n::tr("j-body-miss")),
                    }
                } else if self.side.joint.relation_pick.is_some() {
                    self.status = Notice::info(crate::i18n::tr("j-relation-pick-in-list"));
                // a relation ties mates: they are picked in the list, not on the canvas
                } else if self.side.joint.ground_pick {
                    let hit = crate::gui::pick::pick_body_at(&self.painting(), rect, pos).and_then(|mi| self.project.mesh_id(mi));
                    qymcad_assembly::ground_click(&mut self.joint_ctx(), hit); // the Ground tool: a click on a part fixes it or releases it
                } else if self.side.joint.pick_faces || self.side.joint.conn_pick {
                    // picking an anchor: either for a mate connector (A, then B on another part) or for a
                    // STANDALONE anchor - the parsing is the same, only what happens on the click differs,
                    // and `joint_pick_anchor_click` decides that.
                    // THE KIND OF ANCHOR IS INFERRED UNDER THE CURSOR rather than declared in advance.
                    self.joint_pick_inferred_click(rect, pos);
                } else if self.params.mirror.waiting && self.params.mirror.part.is_none() {
                    let hit = crate::gui::pick::pick_body_at(&self.painting(), rect, pos); // the part is taken by its body
                    qymcad_part::mirror_part_take(&mut self.part_ctx(), hit);
                } else if self.params.mirror.part.is_some() {
                    // a click on ANY plane - a base plane, a datum or a FACE - is taken as the plane of the
                    // mirrored copy, shown before Enter makes it (`mirror_part_apply`)
                    let plane = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos).and_then(|sp| qymcad_ui_state::plane_in_context(&self.painting(), sp));
                    self.params.mirror.at = plane.or(self.params.mirror.at); // a miss keeps the plane taken before
                    self.status = if plane.is_some() { Notice::info(crate::i18n::tr("mp-plane-taken")) } else { Notice::refusal(crate::i18n::tr("vp-miss-mirror-plane")) };
                } else if self.side.section.pick {
                    // THE SECTION: a click on a plane, a datum or a face sets the cutting plane
                    let plane = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos).and_then(|sp| qymcad_ui_state::plane_in_context(&self.painting(), sp));
                    qymcad_part::section_pick_click(&mut self.side.section, &mut self.regen, &mut self.status, plane);
                } else if let Some(target) = self.tools.picking.plane_face() {
                    // a datum plane offset from a face: a click on a part's face
                    if let Some(qymcad_core::feature::SketchPlane::Face(body, key)) = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos) {
                        self.make_offset_plane_from_face(target, body, key);
                        self.tools.picking.set_plane_face(None);
                    } else {
                        self.status = Notice::refusal(crate::i18n::tr("vp-miss-face-only"));
                    }
                } else if self.tools.pending_import.curves.is_some() {
                    // placing an imported DXF or SVG: the click sets the sketch plane
                    if let Some(sp) = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos) {
                        self.place_pending_import(sp);
                    } else {
                        self.status = Notice::refusal(crate::i18n::tr("vp-miss-place-import"));
                    }
                } else if let Some(si) = self.tools.picking.replace_sketch() {
                    // RE-placing a sketch: the click sets a new plane and the bodies are rebuilt
                    if let Some(sp) = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos) {
                        crate::gui::sketching::set_sketch_plane(&mut self.sketch_ctx(), si, sp);
                    } else {
                        self.status = Notice::refusal(crate::i18n::tr("vp-miss-plane"));
                    }
                } else if self.tools.picking.is_sketch_plane() {
                    // choosing a plane or a face for a new sketch by a click in the viewport
                    if let Some(sp) = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos) {
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
                            self.status = Notice::info(crate::i18n::tr("vp-sketch-origin-bound"));
                        }
                    } else {
                        self.status = Notice::refusal(crate::i18n::tr("vp-miss-plane"));
                    }
                } else if aim == Some(qymcad_part::Aim::Plane) {
                    // the mirror plane, the cutting plane, the base of a datum plane: a plane, datum or face
                    let plane = crate::gui::pick::pick_sketch_plane_at(&self.painting(), rect, pos);
                    qymcad_part::take_plane(&mut self.part_ctx(), plane);
                } else if aim == Some(qymcad_part::Aim::VertexOfThePart) {
                    let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
                    let vertex = self.pick_vertex_any(rect, pos).filter(|(b, _, _)| self.project.body_owner(*b) == Some(ctx)).map(|(body, edge, end)| qymcad_part::VertexAt {
                        body,
                        edge,
                        end,
                        at: crate::gui::vertex_local_pos(&self.live, body, edge, end).unwrap_or([0.0; 3]),
                    });
                    qymcad_part::take_vertex(&mut self.part_ctx(), vertex);
                } else if aim == Some(qymcad_part::Aim::PointSnap) {
                    let hit = crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos);
                    qymcad_part::datum_point_snap(&mut self.part_ctx(), hit);
                } else if aim == Some(qymcad_part::Aim::Axis) {
                    let axis = crate::gui::pick::pick_axis_at(&self.painting(), rect, pos).map(|hit| qymcad_part::AxisAt {
                        hit,
                        axis: crate::gui::axis_ref_world(&self.active_path, &self.edges, &self.live, &self.project, hit).map(|(origin, dir)| qymcad_part::Line { origin, dir }),
                    });
                    qymcad_part::take_axis(&mut self.part_ctx(), axis);
                } else if aim == Some(qymcad_part::Aim::TwoPoints) {
                    let hit = crate::gui::pick::pick_datum_point_at(&self.painting(), rect, pos).or_else(|| crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos).map(|w| (0, w)));
                    qymcad_part::take_axis_point(&mut self.part_ctx(), hit.map(|(datum, at)| qymcad_part::PointAt { datum, at }));
                } else if aim == Some(qymcad_part::Aim::Placement) {
                    let frame = crate::gui::pick::pick_place_frame_at(&self.painting(), rect, pos);
                    qymcad_part::take_placement(&mut self.part_ctx(), frame);
                } else if aim == Some(qymcad_part::Aim::PatternAxis) || (self.side.carr.is_circular() && self.params.arr.axis_pick) {
                    // A CIRCULAR PATTERN, of bodies or of parts: the click picks the AXIS (`gui/array_axis.rs`)
                    let at = crate::gui::array_axis::aim(&self.painting(), rect, pos);
                    crate::gui::array_axis::take(&mut self.part_ctx(), at);
                } else if aim == Some(qymcad_part::Aim::RevolveAxis) {
                    // the REVOLVE axis by a click in 3D - in exactly the same place as the circular pattern's axis.
                    self.rev_axis_pick_click(rect, pos);
                } else if let Some(jid) = qymcad_assembly::joint_glyph_at(&mut self.joint_ctx(), rect, pos) {
                    self.chosen.sel = Sel::Joint(jid); // a click on a mate glyph selects it
                } else if aim == Some(qymcad_part::Aim::ReferenceFace) {
                    let face = crate::gui::pick::pick_face_persist_id(&self.painting(), rect, pos);
                    qymcad_part::take_reference_face(&mut self.part_ctx(), face);
                } else if self.side.m3.on {
                    // MEASURING IN 3D: the click picks a vertex, an edge or a face; the second one computes
                    self.measure_3d_click(rect, pos);
                } else if aim == Some(qymcad_part::Aim::Edges) && (self.pick_edge_3d(rect, pos) || self.pick_face_edges_fillet(rect, pos)) {
                    // a click on an edge OR on a FACE (a face takes all of its edges) - ONLY under
                    // Chamfer/Fillet (otherwise an ordinary click would pick an edge or a face instead of a
                    // body, and leave a stray orange selection)
                } else if aim == Some(qymcad_part::Aim::ThreadRim) {
                    // THREAD: a click on a cylindrical face gives the body and the rim (axis and radius) + inner/outer
                    self.pick_thread_target(rect, pos);
                } else if let Some((on, ax, rot)) = crate::gui::gizmo_click_hit(&self.painting(), rect, pos, basis3) {
                    // A CLICK (with no drag) on the body gizmo's arrow or ring opens precise numeric entry at the geometry
                    self.dragged.body_giz.num = Some((on, ax, rot));
                    self.dragged.body_giz.num_field.clear();
                    self.dragged.body_giz.num_focus = true;
                } else {
                    self.pick_face_3d(rect, pos);
                }
            }
        }
    }
}
