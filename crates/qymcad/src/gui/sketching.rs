//! THE SKETCHER - the geometry of a sketch, its dimensions and constraints, editing them and diagnosing
//! them.

#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_ui_state::{sketch_closed_contours, sketch_diag};
#[allow(unused_imports)] // the checks reach these through this module, which is the address they know
pub(crate) use qymcad_pick::{constraint_glyphs, sketch_ref_edges_2d};
pub(crate) use qymcad_sketch::*;
use super::*;
use qymcad_ui_state::EditTool;

impl App {
    /// The sketch key layout: drawing (S/L/R/C/A/P/G/E/O/N/T), editing (F corner fillet, M mirror, K trim,
    /// X construction), dimensions (D). The remaining tools (angles, arrays, constraints) go through buttons -
    /// there are not enough letters.
    pub(super) fn sketch_hotkey(&mut self, key: impl Into<qymcad_ui_state::Chord>) {
        let Some(action) = qymcad_ui_state::hotkey_action(&self.set, "sketch", key) else { return };
        if let Some(t) = qymcad_sketch::tool_for_action(action) {
            return self.set_sk_tool(t);
        }
        match action {
            "sketch.select" => {
                self.tools.armed = qymcad_ui_state::Armed::None;
                self.tools.tool.pts.clear();
                self.status = crate::i18n::tr("sk-select");
            }
            "sketch.dim" => qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, &self.project, self.chosen.sel, self.sketch_ses, &mut self.status, 1),
            "sketch.corner-fillet" => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, 4),
            "sketch.trim" => qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_of!(self), &mut self.viewing.mode_3d, 1),
            "sketch.mirror" => qymcad_ui_state::modify_button(qymcad_ui_state::editing_of!(self), &mut qymcad_ui_state::tools_of!(self), self.sk_pat, &self.tool_prefs, EditTool::Mirror),
            "sketch.construction" => qymcad_ui_state::construction_toggle(qymcad_ui_state::editing_of!(self), &self.tools.sel_sk, &self.sketch_ses, &mut self.tools.tool.construction),
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
        qymcad_ui_state::pan_sheet_2d(&mut self.viewing.view, ctx, resp, self.set.mouse_nav);
        if scroll != 0.0 && resp.contains_pointer() {
            qymcad_ui_state::wheel_zoom_2d(&mut self.viewing.view, &self.set, rect, resp.hover_pos(), scroll);
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
                resp.context_menu(|ui| sketch_context_menu(&mut self.sketch_ctx(), ui, si));
            }
        }
        // a double click finishes a chain of lines; otherwise it edits a dimension
        if resp.double_clicked() && self.workbench == Workbench::Sketch {
            if self.tools.armed.draw_kind() == 1 {
                self.tools.tool.pts.clear(); // finish the current chain; the tool stays armed
            } else if self.tools.armed.draw_kind() == 9 {
                finish_spline(&mut self.sketch_ctx());
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
                            self.tools.pat.center = Some(qymcad_core::geom::Point2::new(cx, cy));
                            // the centre from the record (it can be re-picked)
                        }
                    }
                    self.status = crate::i18n::tr("sk-array-edit-hint");
                } else if let Some(ti) = qymcad_ui_state::text_at(&self.project, &self.viewing.view, rect, pos, si) {
                    qymcad_ui_state::begin_text_edit(&self.project, &mut qymcad_ui_state::tools_of!(self), &mut self.tool_prefs, si, ti);
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
                } else {
                    open_shape_size(&mut self.sketch_ctx(), rect, pos, si);
                }
            }
        }
        // the cursor with snapping (it refreshes the snap hint for the marker)
        self.cursor = resp.hover_pos().map(|p| snap_world(&mut self.sketch_ctx(), rect, p));
        let lit = qymcad_sketch::pre_select(&mut self.sketch_ctx(), rect, resp.hover_pos(), resp.dragged());
        (self.chosen.hover.sketch, self.chosen.hover.constraint) = (lit.sketch, lit.constraint.or(self.chosen.hover.constraint));
        qymcad_sketch::follow_pointer(&mut self.sketch_ctx(), rect, resp.hover_pos());
        if self.sketch_ses.editing.is_some() {
            qymcad_render::draw_sketch_grid(&self.scheme, &self.set, self.viewing.view, painter, rect);
            // the grid and the axes while a sketch is being edited
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
        let sh = qymcad_ui_state::Sheet { view: self.viewing.view, rect };
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
                    qymcad_render::draw_sketch_points(&self.painting(), painter, rect, si, &self.tools.dim.pick);
                    self.draw_sketch_dims(painter, rect, si);
                    self.draw_sketch_constraints(painter, rect, si);
                }
                // text AS GEOMETRY (parametric captions): the glyphs go over the contours, and the
                // selected one is orange with a bounding box. It is drawn explicitly, so it shows
                // whether or not the contours are displayed.
                for (ti, t) in self.project.sketches[si].texts.iter().enumerate() {
                    let sel = self.tools.annot.text == Some(ti);
                    let col = if sel {
                        self.scheme.pal.selected()
                    } else if t.construction {
                        self.scheme.pal.sketch_construction()
                    } else {
                        self.scheme.pal.annotation()
                    };
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
        let asks = qymcad_ui_state::text_popups(qymcad_ui_state::editing_of!(self), &mut self.font_cache, &mut qymcad_ui_state::text_ctx_of!(self), ctx, rect); // the label editor and the list of fonts
        self.do_bar_asks(asks, ctx);
        place_input_popup(qymcad_ui_state::editing_of!(self), qymcad_ui_state::popup_tools!(self), qymcad_ui_state::popup_looks!(self), ctx, rect); // typing the sizes right after a shape is built
        sketch_tool_keys(&mut self.sketch_ctx(), ctx, rect); // the angle of a turn, Enter of Extend
    }
}

// SKETCH INPUT: the size popup that follows building a shape, snapping the cursor to the world, and a
// dimension between two objects. This is work on the sketch, not the frame of the frame.
impl App {}

// SKETCHES: the dimension that follows the cursor until it is placed, and inferring that a point belongs
// to a segment.
impl App {}

impl App {}

impl App {}

/// Apply a NEW plane to sketch `si` (the 2D geometry is kept and carried onto it) and rebuild the bodies built on that
/// sketch (just as an ordinary sketch edit does, through `mark_sketch_dirty`).
pub(crate) fn set_sketch_plane(sk: &mut qymcad_ui_state::SketchCtx, si: usize, plane: qymcad_core::feature::SketchPlane) {
    let plane = crate::gui::resolve_placement_plane(&mut *sk.cmd, &mut *sk.project, &mut *sk.status, plane);
    let sid = sk.project.sketches[si].id;
    sk.project.sketches[si].plane = plane;
    sk.project.mark_sketch_dirty(sid); // associativity: the bodies on this sketch will be rebuilt
    qymcad_ui_state::mark_dirty_for_rebuild(&mut sk.rebuild()); // the document is marked; the scheduler does the computing
    sk.picking.clear();
    sk.view.initialized = false;
    *sk.status = crate::i18n::tr("g-sketch-moved");
}
