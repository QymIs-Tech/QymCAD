//! THE SKETCHER - the geometry of a sketch, its dimensions and constraints, editing them and diagnosing
//! them.

#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_ui_state::{sketch_closed_contours, sketch_diag};
#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_pick::{constraint_glyphs, sketch_ref_edges_2d};
pub(crate) use qymcad_sketch::*;
use super::*;



impl App {
    /// The sketch key layout: drawing (S/L/R/C/A/P/G/E/O/N/T), editing (F corner fillet, M mirror, K trim,
    /// X construction), dimensions (D). The remaining tools (angles, arrays, constraints) go through buttons -
    /// there are not enough letters.
    pub(super) fn sketch_hotkey(&mut self, key: egui::Key) {
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "sketch", key) else { return };
        match action {
            "sketch.select" => {
                self.tools.armed = qymcad_ui_state::Armed::None;
                self.tools.tool.pts.clear();
                self.status = crate::i18n::tr("sk-select");
            }
            "sketch.line" => self.set_sk_tool(1),
            "sketch.rect" => self.set_sk_tool(2),
            "sketch.circle" => self.set_sk_tool(3),
            "sketch.arc" => self.set_sk_tool(4),
            "sketch.point" => self.set_sk_tool(5),
            "sketch.polygon" => self.set_sk_tool(6),
            "sketch.slot" => self.set_sk_tool(7),
            "sketch.ellipse" => self.set_sk_tool(8),
            "sketch.spline" => self.set_sk_tool(9),
            "sketch.text" => self.set_sk_tool(11),
            "sketch.dim" => qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, &self.project, self.chosen.sel, self.sketch_ses, &mut self.status, 1),
            "sketch.corner-fillet" => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, 4),
            "sketch.trim" => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, 1),
            "sketch.mirror" => qymcad_ui_state::modify_button(qymcad_ui_state::editing_of!(self), &mut qymcad_ui_state::tools_of!(self), self.sk_pat, &self.tool_prefs, 1),
            "sketch.construction" => self.tools.tool.construction = !self.tools.tool.construction,
            _ => {}
        }
    }




























}

// THE SKETCH VIEWPORT moved here from `gui.rs` whole, along with its phases: the start of a drag, the drag
// itself, the click, the drawing. It used to lie in the root as one body of 959 lines, and moving it was
// impossible: the phases had neither names nor boundaries.
impl App {
    /// THE FLAT SKETCH VIEWPORT: panning and zooming, the drawing tools, picking entities, drawing.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn viewport_2d(&mut self, ctx: &egui::Context, resp: &egui::Response, painter: &egui::Painter, rect: Rect, has_geom: bool, scroll: f32) {
                if !self.viewing.view.initialized && has_geom {
                    qymcad_ui_state::fit(&self.project, &mut self.viewing.view, rect);
                }
                let ctrl = ctx.input(|i| i.modifiers.ctrl);
                let handle = qymcad_ui_state::selected_centroid(&self.project, &self.chosen.sel);
                // the priority of starting a drag: a sketch point, then the gizmo (on a handle), then the
                // selection box (Ctrl), then panning.
                // GRABBING GEOMETRY happens with the LEFT button only. The middle button pans the canvas;
                // a middle drag over a point or a dimension used to start dragging the geometry and broke the
                // drawing.
                // WHAT EXACTLY WAS GRABBED: the priority chain that reads the start of a drag
                sketch_drag_start(&mut self.sketch_ctx(), ctx, resp, rect, ctrl, handle);
                // WHILE DRAGGING: every link carries ITS OWN object until the release
                sketch_drag_update(&mut self.sketch_ctx(), ctx, resp, rect);
                power_trim_drag(&mut self.sketch_ctx(), resp, rect); // trimming by dragging (the trim tool)
                // panning with the middle button (in a sketch the left button on empty space is a selection box)
                if ctx.input(|i| i.pointer.middle_down()) {
                    let d = ctx.input(|i| i.pointer.delta());
                    self.viewing.view.center.x -= d.x / self.viewing.view.scale;
                    self.viewing.view.center.y += d.y / self.viewing.view.scale;
                }
                if scroll != 0.0 && resp.hovered() {
                    self.viewing.view.scale = (self.viewing.view.scale * (scroll * 0.002).exp()).clamp(0.02, 800.0);
                }
                // A CLICK IN A SKETCH: a drawing tool, placing a dimension, or picking geometry
                sketch_click(&mut self.sketch_ctx(), ctx, resp, rect);
                // the right button opens the context menu of the sketch
                if let Sel::Sketch(si) = self.chosen.sel {
                    if qymcad_ui_state::edit_si(&self.project, &self.sketch_ses) == Some(si) {
                        // if something unselected was clicked, pick it before the menu opens
                        if resp.secondary_clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                if let Some(h) = sketch_hit(&self.pick_ctx(), rect, pos, si) {
                                    if !self.tools.sel_sk.items.contains(&h) {
                                        self.tools.sel_sk.items = vec![h];
                                    }
                                }
                            }
                        }
                        resp.context_menu(|ui| {
                            let has_sel = !self.tools.sel_sk.items.is_empty();
                            let has_ent = self.tools.sel_sk.items.iter().any(|(k, _)| *k == 1);
                            if ui.add_enabled(has_sel, egui::Button::new(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete")))).clicked() {
                                qymcad_ui_state::delete_sketch_sel(&mut self.project, &mut self.regen, &mut self.tools.sel_sk, &mut self.status, si);
                                ui.close();
                            }
                            if ui.add_enabled(has_ent, egui::Button::new(crate::i18n::tr("sk-construction-toggle"))).clicked() {
                                let eids: Vec<Id> = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                                self.project.toggle_construction(si, &eids);
                                self.project.solve_sketch(si);
                                qymcad_ui_state::invalidate(&mut self.regen);
                                ui.close();
                            }
                            // an arc length dimension, for the picked arc
                            let arc_eid = self.tools.sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, id)| {
                                self.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == *id)).and_then(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. }).then_some(*id))
                            });
                            if let Some(aid) = arc_eid {
                                if ui.button(crate::i18n::tr("sk-arc-length-dim")).clicked() {
                                    if let Some(ci) = self.project.ensure_arc_length(si, aid) {
                                        crate::gui::io_jobs::finish_dim(&mut self.project, &mut self.regen, si, ci);
                                        self.tools.place.dim = Some(ci);
                                    }
                                    ui.close();
                                }
                            }
                            // a tangent (edge-to-edge) dimension: two references are picked, at least one of
                            // them a circle or an arc
                            let edge_refs: Vec<(Id, i8)> = self.project.sketches.get(si).map(|s| {
                                self.tools.sel_sk.items.iter().filter_map(|&(k, id)| {
                                    if k == 1 {
                                        s.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                                            qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some((center, -1i8)),
                                            _ => None,
                                        })
                                    } else if k == 0 {
                                        Some((id, 0i8))
                                    } else {
                                        None
                                    }
                                }).collect()
                            }).unwrap_or_default();
                            let can_edge = edge_refs.len() == 2 && edge_refs.iter().any(|(_, m)| *m != 0);
                            if ui.add_enabled(can_edge, egui::Button::new(crate::i18n::tr("sk-tangent-dim"))).on_hover_text(crate::i18n::tr("sk-gap-hint")).clicked() {
                                let ((c1, m1), (c2, m2)) = (edge_refs[0], edge_refs[1]);
                                let d = self.project.measure_edge_distance(si, c1, m1, c2, m2);
                                let ci = self.project.sketches[si].constraints.len();
                                self.project.sketches[si].constraints.push(qymcad_core::model::Constraint::EdgeDistance { c1, c2, d, m1, m2, off: 0.0, expr: String::new(), driven: false });
                                crate::gui::io_jobs::finish_dim(&mut self.project, &mut self.regen, si, ci);
                                self.tools.gsel.constraint = Some(ci);
                                qymcad_ui_state::invalidate(&mut self.regen);
                                ui.close();
                            }
                            ui.separator();
                            ui.menu_button(crate::i18n::tr("sk-constraint"), |ui| {
                                for (label, code) in [(&crate::i18n::tr("sk-coincident"), 0u8), (&crate::i18n::tr("sk-horizontal"), 1), (&crate::i18n::tr("sk-vertical"), 2), (&crate::i18n::tr("sk-parallel"), 3), (&crate::i18n::tr("sk-perpendicular"), 4), (&crate::i18n::tr("sk-equal"), 5), (&crate::i18n::tr("sk-tangency"), 9), (&crate::i18n::tr("sk-midpoint"), 11), (&crate::i18n::tr("sk-fix"), 6)] {
                                    if ui.button(label).clicked() {
                                        constraint_button(&mut self.sketch_ctx(), code);
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            if ui.button(crate::i18n::tr("sk-select-all")).clicked() {
                                qymcad_ui_state::select_all_sketch(&mut self.tools.annot, &mut self.tools.gsel, &self.project, &mut self.tools.sel_sk, &mut self.status, si);
                                ui.close();
                            }
                        });
                    }
                }
                // a double click finishes a chain of lines; otherwise it edits a dimension
                if resp.double_clicked() && self.workbench == Workbench::Sketch {
                    if self.tools.armed.draw_kind() == 1 {
                        self.tools.tool.pts.clear(); // finish the current chain; the tool stays armed
                    } else if self.tools.armed.draw_kind() == 9 {
                        finish_spline(&mut self.project, &mut self.regen, self.chosen.sel, &mut self.tools.tool, &mut self.viewing.view);
                    } else if let (Sel::Sketch(si), Some(pos)) = (self.chosen.sel, resp.interact_pointer_pos()) {
                        // a copy of an array opens its parameters; a text object opens for editing; so do a
                        // note, a dimension and a circle
                        if let Some(pi) = sketch_hit(&self.pick_ctx(), rect, pos, si).filter(|(k, _)| *k == 1).and_then(|(_, eid)| self.project.pattern_of_entity(si, eid)) {
                            // a double click on a copy of an array edits the count and the step, with a preview
                            use qymcad_core::model::PatternKind;
                            // re-opening an array TAKES the pattern tool, so the previous one goes first
                            qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_of!(self));
                            self.tools.pat.edit = Some(pi);
                            match self.project.sketches[si].patterns[pi].kind {
                                PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
                                    self.tools.armed = qymcad_ui_state::Armed::Pattern(1);
                                    self.sk_pat.dx = dx;
                                    self.sk_pat.dy = dy;
                                    self.sk_pat.count = count;
                                    self.sk_pat.dx2 = dx2;
                                    self.sk_pat.dy2 = dy2;
                                    self.sk_pat.count2 = count2.max(1);
                                    self.tools.pat.center = None;
                                }
                                PatternKind::Circular { cx, cy, count, total_deg } => {
                                    self.tools.armed = qymcad_ui_state::Armed::Pattern(2);
                                    self.sk_pat.count = count;
                                    self.sk_pat.angle = total_deg;
                                    self.tools.pat.center = Some(qymcad_core::geom::Point2::new(cx, cy)); // the centre from the record (it can be re-picked)
                                }
                            }
                            self.status = crate::i18n::tr("sk-array-edit-hint");
                        } else if let Some(ti) = qymcad_ui_state::text_at(&self.project, &self.viewing.view, rect, pos, si) {
                            self.tools.inline = InlineEdit::Text(ti);
                            self.tools.annot.text = Some(ti);
                            let t = &self.project.sketches[si].texts[ti];
                            self.tools.annot.text_buf = t.text.clone();
                            self.tools.annot.text_h = t.height;
                        } else if let Some(ni) = qymcad_ui_state::note_at(&self.project, &self.viewing.view, rect, pos, si) {
                            self.tools.inline = InlineEdit::Note(ni);
                            self.tools.annot.note_buf = self.project.sketches[si].notes.get(ni).map(|n| n.text.clone()).unwrap_or_default();
                        } else if let Some(ci) = dim_at(&mut self.sketch_ctx(), rect, pos, si) {
                            // the radius of the circumscribed circle of A POLYGON opens the polygon popup (the
                            // radius plus the rotation angle) rather than the ordinary dimension editor
                            let poly_center = match self.project.sketches[si].constraints.get(ci) {
                                Some(qymcad_core::model::Constraint::Diameter { c, .. }) => {
                                    let c = *c;
                                    self.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)).then_some(c)
                                }
                                _ => None,
                            };
                            if let Some(c) = poly_center {
                                self.tools.place.set(PlacingShape::Poly(c));
                                self.tools.place.focus = true;
                            } else {
                                self.tools.inline = InlineEdit::Dim(ci);
                                self.tools.dim.focus = true;
                            }
                        } else if let Some(eid) = qymcad_pick::nearest_circle_entity(&self.pick_ctx(), rect, pos, si) {
                            // a circle gets a diameter dimension; an arc has its radius edited
                            let center = self.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
                                qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
                                _ => None,
                            });
                            // the circumscribed circle of a polygon (the vertices hang on it) opens the polygon
                            // popup (the radius plus the angle), while an ordinary circle gets a diameter
                            let is_poly_rim = center.is_some_and(|c| self.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)));
                            if let (true, Some(c)) = (is_poly_rim, center) {
                                self.tools.place.set(PlacingShape::Poly(c));
                                self.tools.place.focus = true;
                            } else if let Some(c) = center {
                                if let Some(ci) = self.project.ensure_diameter(si, c, true) {
                                    self.tools.inline = InlineEdit::Dim(ci);
                                    self.tools.dim.focus = true;
                                }
                            } else {
                                self.tools.inline = InlineEdit::Circle(eid);
                                self.tools.dim.focus = true;
                            }
                        } else if let Some(cid) = qymcad_ui_state::polygon_under(&self.project, &self.viewing.view, rect, pos, si) {
                            self.tools.place.set(PlacingShape::Poly(cid)); // editing the radius of the construction circle
                            self.tools.place.focus = true;
                        }
                    }
                }
                // the cursor with snapping (it refreshes the snap hint for the marker)
                self.cursor = resp.hover_pos().map(|p| snap_world(&mut self.sketch_ctx(), rect, p));
                // the pre-select highlight: what is under the cursor - only while the sketch is in selection mode
                self.chosen.hover.sketch = None;
                if let (Sel::Sketch(si), Some(hp)) = (self.chosen.sel, resp.hover_pos()) {
                    if qymcad_ui_state::edit_si(&self.project, &self.sketch_ses) == Some(si) && self.tools.armed.draw_kind() == 0 && self.tools.armed.dim_kind() == 0 && !resp.dragged() && self.tools.drag.pt().is_none() && self.tools.drag.mov().is_none() {
                        self.chosen.hover.sketch = sketch_hit(&self.pick_ctx(), rect, hp, si);
                        // the constraint glyph under the cursor is highlighted, without wiping the hover coming
                        // from the list of constraints
                        if self.chosen.hover.sketch.is_none() {
                            if let Some(gc) = constraint_glyph_at(&mut self.sketch_ctx(), rect, hp, si) {
                                self.chosen.hover.constraint = Some(gc);
                            }
                        }
                    }
                }
                update_placing_dim(&mut self.sketch_ctx(), rect); // the dimension follows the cursor until it is placed
                if resp.hover_pos().is_none() {
                    self.snap_hint = None;
                }
                if self.sketch_ses.editing.is_some() {
                    qymcad_render::draw_sketch_grid(&self.scheme, &self.set, self.viewing.view, painter, rect); // the grid and the axes while a sketch is being edited
                }
                // DRAWING THE SKETCH VIEWPORT is the last phase of the frame: the input has been read by now
                self.draw_sketch_viewport(ctx, resp, painter, rect, handle);
    }










    /// DRAWING THE SKETCH VIEWPORT: the table, the axes, the bodies, the contours, the points and dimensions,
    /// the tool previews, the measurement, the snap marker, the selection box, the in-place editors.
    ///
    /// It is the last phase of the frame, and that matters: by the time drawing happens the input HAS BEEN
    /// read (`sketch_drag_start` -> `sketch_drag_update` -> `sketch_click`), so drawing decides nothing and
    /// picks nothing - it only shows what was decided. While everything lay in one body, drawing and reading
    /// the input were mixed together, and "showing" easily turned into "deciding".
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_sketch_viewport(&mut self, ctx: &egui::Context, resp: &egui::Response, painter: &egui::Painter, rect: Rect, handle: Option<qymcad_core::geom::Point2>) {
        let sh = qymcad_ui_state::Sheet { view: self.viewing.view, rect: rect };
                        qymcad_render::draw_axes(&self.painting(), painter, rect);
                self.draw_mesh(painter, rect);
                self.draw_sketch_face_edges(painter, rect); // the edges of the host face (an outside one too) as a reference
                self.draw_contours(painter, rect);
                // the points of the picked sketch plus its dimensions and constraints (visible, associative)
                if let Sel::Sketch(si) = self.chosen.sel {
                    if self.project.sketches.get(si).is_some() {
                        // in the half-sketcher of a part command (extrude, cut, revolve) the dimensions, the
                        // constraint glyphs, the construction geometry and the point numbers are hidden - a clean
                        // profile, like a drawing.
                        let profile_pick = self.tools.armed.commanding() && self.sketch_ses.editing.is_none();
                      if !profile_pick {
                        // the colour of the points follows how defined they are, as in any CAD: green means
                        // defined, YELLOW still free (showing what is under-defined), red means trouble.
                        // Points go RED only on a CONFLICT of dimensions (inconsistent values, a real error).
                        // Harmless redundancy (consistent reference dimensions) does NOT redden them - that
                        // would be a false alarm of "a heap of errors" on a perfectly good sketch.
                        let has_conflict = !sketch_diag(&self.cache, &self.project, si).conflicts.is_empty();
                        let (_, free) = sketch_status(&mut self.sketch_ctx(), si); // cached: the Jacobian is not computed every frame
                        // the reference points (the origin, the guides of the axes) are drawn by the axis marker
                        // rather than as numbered geometry, so they are hidden from the common list.
                        let refset: std::collections::HashSet<Id> = self.project.sketches[si].system_ids().into_iter().collect();
                        for (pi, p) in self.project.sketches[si].points.iter().enumerate() {
                            if refset.contains(&p.id) {
                                continue;
                            }
                            let movable = free.get(pi).copied().unwrap_or(true);
                            let base_col = if has_conflict {
                                self.scheme.pal.error_mild() // a conflict of dimensions - the points do not satisfy the constraints
                            } else if movable {
                                self.scheme.pal.underdefined() // still free
                            } else {
                                self.scheme.pal.ok() // defined
                            };
                            let sp = sh.at(qymcad_core::geom::Point2::new(p.x, p.y));
                            let picked = self.tools.dim.pick.contains(&p.id);
                            let selected = self.tools.sel_sk.items.contains(&(0, p.id));
                            let hovered = self.chosen.hover.sketch == Some((0, p.id));
                            let (col, r) = if selected {
                                (self.scheme.pal.emphasis(), 5.0)
                            } else if hovered {
                                (self.scheme.pal.preview(), 5.0) // the pre-select highlight
                            } else if picked {
                                (self.scheme.pal.sketch_point(), 4.5)
                            } else {
                                (base_col, 3.5)
                            };
                            painter.circle_filled(sp, r, col);
                            painter.text(sp + egui::vec2(5.0, -5.0), egui::Align2::LEFT_BOTTOM, format!("{}", pi + 1), egui::FontId::monospace(10.0), self.scheme.pal.text_faint());
                        }
                        self.draw_sketch_dims(painter, rect, si);
                        self.draw_sketch_constraints(painter, rect, si);
                      }
                        // text AS GEOMETRY (parametric captions): the glyphs go over the contours, and the
                        // selected one is orange with a bounding box. It is drawn explicitly, so it shows
                        // whether or not the contours are displayed.
                        for (ti, t) in self.project.sketches[si].texts.iter().enumerate() {
                            let sel = self.tools.annot.text == Some(ti);
                            let col = if sel { self.scheme.pal.selected() } else if t.construction { self.scheme.pal.sketch_construction() } else { self.scheme.pal.annotation() };
                            for loop_ in &t.glyphs {
                                if loop_.len() >= 2 {
                                    let mut pts: Vec<Pos2> = loop_.iter().map(|p| sh.at(*p)).collect();
                                    pts.push(pts[0]);
                                    painter.add(egui::Shape::line(pts, Stroke::new(if sel { 2.0 } else { 1.7 }, col)));
                                }
                            }
                            if sel {
                                if let Some((minx, miny, maxx, maxy)) = self.project.sketch_text_bbox(si, ti) {
                                    let bb = Rect::from_two_pos(sh.at(qymcad_core::geom::Point2::new(minx, miny)), sh.at(qymcad_core::geom::Point2::new(maxx, maxy))).expand(3.0);
                                    painter.rect_stroke(bb, 0.0, Stroke::new(1.0, self.scheme.pal.selected()), egui::StrokeKind::Middle);
                                }
                            }
                        }
                        // text notes (remarks and to-dos); the selected one is orange
                        for (ni, note) in self.project.sketches[si].notes.iter().enumerate() {
                            let sp = sh.at(qymcad_core::geom::Point2::new(note.x, note.y));
                            let nc = if self.tools.annot.note == Some(ni) { self.scheme.pal.selected() } else { self.scheme.pal.note() };
                            painter.text(sp, egui::Align2::LEFT_BOTTOM, &note.text, egui::FontId::proportional(14.0), nc);
                        }
                    }
                }
                qymcad_render::draw_projection_overlay(&self.project, &self.scheme, self.chosen.sel, self.viewing.view, painter, rect); // driven projected geometry, in its own colour
                self.draw_sketch_preview(painter, rect);
                self.draw_trim_preview(painter, rect); // the hover preview of a trim, extend or break
                self.draw_move_preview(painter, rect); // the ghost of a move or a copy
                self.draw_pattern_preview(painter, rect); // the ghost of an array
                self.draw_clip_pending(painter, rect); // the highlight of the selection plus the crosshair awaiting the anchor point
                qymcad_render::draw_clip_ghost(&self.side.clip, self.cursor, &self.scheme, self.viewing.view, painter, rect); // the ghost of geometry being pasted from the buffer
                // the measurement: the points, the line and the distance caption
                if self.tools.armed.measuring() && !self.tools.measure.pts.is_empty() {
                    let col = self.scheme.pal.measure();
                    for p in &self.tools.measure.pts {
                        painter.circle_filled(sh.at(*p), 3.5, col);
                    }
                    if self.tools.measure.pts.len() == 2 {
                        let (a, b) = (self.tools.measure.pts[0], self.tools.measure.pts[1]);
                        let (sa, sb) = (sh.at(a), sh.at(b));
                        painter.line_segment([sa, sb], Stroke::new(1.5, col));
                        let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                        painter.text(((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2(), egui::Align2::CENTER_BOTTOM, format!("{d:.2}"), egui::FontId::proportional(13.0), col);
                    }
                }
                // the snap marker of the cursor
                if let Some((p, kind)) = self.snap_hint {
                    let sp = sh.at(p);
                    let yellow = self.scheme.pal.snap_marker();
                    match kind {
                        0 => {
                            // a vertex is a yellow square
                            let r = 5.0;
                            painter.rect_stroke(Rect::from_center_size(sp, egui::vec2(r * 2.0, r * 2.0)), 0.0, Stroke::new(1.5, yellow), egui::StrokeKind::Middle);
                        }
                        3 => {
                            // a midpoint is a triangle
                            let r = 6.0;
                            let pts = vec![sp + egui::vec2(0.0, -r), sp + egui::vec2(r * 0.87, r * 0.5), sp + egui::vec2(-r * 0.87, r * 0.5)];
                            painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, yellow)));
                        }
                        4 => {
                            // a centre is a ring
                            painter.circle_stroke(sp, 5.5, Stroke::new(1.5, yellow));
                            painter.circle_filled(sp, 1.3, yellow);
                        }
                        5 => {
                            // an intersection is a diagonal cross
                            let r = 6.0;
                            let c = self.scheme.pal.snap_intersection();
                            painter.line_segment([sp + egui::vec2(-r, -r), sp + egui::vec2(r, r)], Stroke::new(1.6, c));
                            painter.line_segment([sp + egui::vec2(-r, r), sp + egui::vec2(r, -r)], Stroke::new(1.6, c));
                        }
                        6 => {
                            // a point on an edge is a diamond
                            let r = 5.0;
                            let c = self.scheme.pal.snap_edge();
                            let pts = vec![sp + egui::vec2(0.0, -r), sp + egui::vec2(r, 0.0), sp + egui::vec2(0.0, r), sp + egui::vec2(-r, 0.0)];
                            painter.add(egui::Shape::closed_line(pts, Stroke::new(1.4, c)));
                        }
                        2 => {
                            // an axis is a larger purple cross
                            let r = 7.0;
                            let c = self.scheme.pal.snap_axis();
                            painter.line_segment([sp - egui::vec2(r, 0.0), sp + egui::vec2(r, 0.0)], Stroke::new(1.3, c));
                            painter.line_segment([sp - egui::vec2(0.0, r), sp + egui::vec2(0.0, r)], Stroke::new(1.3, c));
                        }
                        _ => {
                            // a grid node is a grey cross
                            let r = 4.0;
                            let c = self.scheme.pal.snap_grid();
                            painter.line_segment([sp - egui::vec2(r, 0.0), sp + egui::vec2(r, 0.0)], Stroke::new(1.0, c));
                            painter.line_segment([sp - egui::vec2(0.0, r), sp + egui::vec2(0.0, r)], Stroke::new(1.0, c));
                        }
                    }
                }
                        // the sketch while it is being drawn
                if let Some(pts) = &self.tools.pending_import.draw_pts {
                    let st = Stroke::new(2.0, self.scheme.pal.sketch_line());
                    let screen: Vec<Pos2> = pts.iter().map(|p| sh.at(*p)).collect();
                    if screen.len() >= 2 {
                        painter.add(egui::Shape::line(screen.clone(), st));
                    }
                    for s in &screen {
                        painter.circle_filled(*s, 3.0, self.scheme.pal.sketch_line());
                    }
                    if let (Some(last), Some(cur)) = (screen.last(), self.cursor) {
                        painter.line_segment([*last, sh.at(cur)], Stroke::new(1.0, self.scheme.pal.rubber_band()));
                    }
                }
                // the move gizmo of the selected object
                if let Some(hw) = handle {
                    let hs = sh.at(hw);
                    let col = if self.dragged.body_giz.dragging { self.scheme.pal.active() } else { self.scheme.pal.ok() };
                    painter.rect_filled(Rect::from_center_size(hs, egui::vec2(10.0, 10.0)), 2.0, col);
                    painter.circle_stroke(hs, 14.0, Stroke::new(1.0, col));
                }
                // the selection box on top: left to right is A WINDOW (solid blue, only what is wholly
                // inside), right to left is A CROSSING (dashed green, anything touched)
                if let (Some(a), Some(b)) = (self.chosen.tree_sel.box_start, resp.interact_pointer_pos()) {
                    let r = Rect::from_two_pos(a, b);
                    if b.x < a.x {
                        let st = Stroke::new(1.2, self.scheme.pal.select_cross());
                        for seg in [[r.left_top(), r.right_top()], [r.right_top(), r.right_bottom()], [r.right_bottom(), r.left_bottom()], [r.left_bottom(), r.left_top()]] {
                            painter.add(egui::Shape::dashed_line(&seg, st, 5.0, 4.0));
                        }
                        painter.rect_filled(r, 0.0, crate::palette::a(self.scheme.pal.select_cross(), 20));
                    } else {
                        painter.rect_stroke(r, 0.0, Stroke::new(1.2, self.scheme.pal.select_window()), egui::StrokeKind::Middle);
                        painter.rect_filled(r, 0.0, crate::palette::a(self.scheme.pal.select_window(), 20));
                    }
                }
                // the in-place dimension editor (a double click on a dimension)
                dim_editor(&mut self.sketch_ctx(), ctx, rect);
                qymcad_ui_state::note_editor(&mut self.tools.annot, &mut self.tools.inline, &mut self.project, self.chosen.sel, self.viewing.view, ctx, rect); // editing the text of a note
                qymcad_ui_state::text_obj_editor(qymcad_ui_state::editing_of!(self), &mut self.tools.annot, &mut self.font_cache, &mut self.tools.inline, ctx, rect); // editing a text object (the string and the height)
                place_input_popup(qymcad_ui_state::editing_of!(self), &mut self.tools.corner, &mut self.tools.place, &mut self.tools.sel_sk, &mut self.tool_prefs, ctx, rect); // typing the sizes right after a shape is built
                sketch_rotate_popup(qymcad_ui_state::editing_of!(self), &mut self.tools.armed, &mut self.side.rot, &self.tools.sel_sk, &mut self.tools.tool, ctx, rect); // the rotation angle at the centre
    }
}

// SKETCH INPUT: the size popup that follows building a shape, snapping the cursor to the world, and a
// dimension between two objects. This is work on the sketch, not the frame of the frame.
impl App {



}

// SKETCHES: the dimension that follows the cursor until it is placed, and inferring that a point belongs
// to a segment.
impl App {

}

impl App {
}

impl App {
}
