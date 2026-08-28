//! DRAWING - everything that paints the scene, the gizmos, the highlights and the overlays.

use super::*;

impl App {
    /// Draw the live sweep preview (the path + the carried profile sections + the longitudinal edges).
    pub(super) fn draw_sweep_preview(&self, painter: &egui::Painter, rect: Rect) {
        let Some((path, sections)) = self.sweep_preview() else { return };
        let basis = self.cam.basis();
        let sp = |p: [f64; 3]| self.project3(p, rect, &basis).0;
        // the path is a bright line
        let pcol = self.scheme.pal.active();
        for w in path.windows(2) {
            painter.line_segment([sp(w[0]), sp(w[1])], Stroke::new(2.0, pcol));
        }
        // the profile sections (closed loops) + the longitudinal edges between neighbouring stations
        let scol = crate::palette::a(self.scheme.pal.preview(), 220);
        for sec in &sections {
            let m = sec.len();
            for k in 0..m {
                painter.line_segment([sp(sec[k]), sp(sec[(k + 1) % m])], Stroke::new(1.5, scol));
            }
        }
        for pair in sections.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            for k in 0..a.len().min(b.len()) {
                painter.line_segment([sp(a[k]), sp(b[k])], Stroke::new(1.0, scol));
            }
        }
    }


    /// The live loft preview: the section outlines + the longitudinal edges between neighbouring sections.
    pub(super) fn draw_loft_preview(&self, painter: &egui::Painter, rect: Rect) {
        let sections = self.loft_preview();
        if sections.is_empty() {
            return;
        }
        let basis = self.cam.basis();
        let sp = |p: [f64; 3]| self.project3(p, rect, &basis).0;
        let scol = crate::palette::a(self.scheme.pal.preview(), 220);
        for sec in &sections {
            let m = sec.len();
            for k in 0..m {
                painter.line_segment([sp(sec[k]), sp(sec[(k + 1) % m])], Stroke::new(1.8, scol));
            }
        }
        // the longitudinal edges between neighbouring sections (sampled by loop parameter, so the lengths may differ)
        for pair in sections.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if a.is_empty() || b.is_empty() {
                continue;
            }
            let steps = a.len().max(b.len());
            for s in 0..steps {
                let ia = s * a.len() / steps;
                let ib = s * b.len() / steps;
                painter.line_segment([sp(a[ia]), sp(b[ib])], Stroke::new(1.0, scol));
            }
        }
    }


    /// The thread preview: a ghost of the HELIX (the crest of the turn) along the axis from the rim over
    /// the length, from the current parameters. Light (a polyline), it builds no geometry - the real body
    /// appears on Enter.
    pub(super) fn draw_thread_preview(&self, painter: &egui::Painter, rect: Rect) {
        let Some(src) = self.thread.src else { return };
        if self.thread.edge == 0 {
            return;
        }
        let (center, axis) = self.thread.axis;
        let r = self.thread.radius;
        let pitch = self.cmd_val("pitch").max(0.05);
        let length = self.cmd_val("length").max(0.1);
        let lead = pitch * self.thread.starts.max(1) as f64;
        let scale = |a: [f64; 3], s: f64| [a[0] * s, a[1] * s, a[2] * s];
        let add = |a: [f64; 3], b: [f64; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let ax = v_norm(axis);
        let refx = if ax[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = v_norm(v_sub(refx, scale(ax, v_dot(refx, ax)))); // the radial basis, perpendicular to the axis
        let v = v_cross(ax, u);
        let basis = self.cam.basis();
        let wt = self.project.body_display_transform(src, self.current_ctx_id());
        let turns = length / lead;
        let n = ((turns * 24.0).ceil() as usize).clamp(8, 4000);
        let sign = if self.thread.left { -1.0 } else { 1.0 };
        let col = self.scheme.pal.preview_axis();
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let frac = i as f64 / n as f64;
                let a = sign * frac * turns * std::f64::consts::TAU;
                let radial = add(scale(u, a.cos()), scale(v, a.sin()));
                let p = add(add(center, scale(ax, frac * length)), scale(radial, r));
                self.project3(qymcad_core::feature::apply12(&wt, p), rect, &basis).0
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.4, col)));
        // the thread axis, dashed over the length
        let a0 = self.project3(qymcad_core::feature::apply12(&wt, center), rect, &basis).0;
        let a1 = self.project3(qymcad_core::feature::apply12(&wt, add(center, scale(ax, length))), rect, &basis).0;
        painter.add(egui::Shape::dashed_line(&[a0, a1], Stroke::new(0.8, col), 5.0, 4.0));
    }


    pub(super) fn draw_feat_cmd_preview(&self, painter: &egui::Painter, rect: Rect) {
        if self.cmd.kind == 8 && self.mode_3d {
            self.draw_sweep_preview(painter, rect);
            return;
        }
        if self.cmd.kind == 9 && self.mode_3d {
            self.draw_loft_preview(painter, rect);
            return;
        }
        if self.cmd.kind == 24 && self.mode_3d {
            self.draw_thread_preview(painter, rect);
            return;
        }
        if self.cmd.kind == 0 || self.cmd.kind == 3 || self.gsel.profiles.is_empty() || !self.mode_3d {
            return;
        }
        let Some(si) = self.cmd.sketch else { return };
        let Some(f) = self.project.sketch_frame(si) else { return };
        let basis = self.cam.basis();
        let n = f.normal();
        let h = self.cmd_val("height");
        // the preview extent is EXACTLY the one the rebuild uses: direction/flip/symmetry/two sides. The
        // distances come from the expression fields at the geometry (cmd_val), with no lag, so preview = result.
        let down = if self.cmd.extent.two_sided() { self.cmd_val("down").abs() } else { 0.0 };
        let (start, total) = qymcad_core::feature::extrude_extent(h, down, self.cmd_reach());
        let col = crate::palette::a(self.scheme.pal.preview(), 220);
        for cid in &self.gsel.profiles {
            let Some(xy) = self.project.contour_profile_xy(*cid) else { continue };
            let m = xy.len() / 2;
            if m < 2 {
                continue;
            }
            let liftp = |k: usize, off: f64| -> Pos2 {
                let p = f.lift(Point2::new(xy[2 * k], xy[2 * k + 1]));
                self.project3([p.x + n[0] * off, p.y + n[1] * off, p.z + n[2] * off], rect, &basis).0
            };
            for k in 0..m {
                let k2 = (k + 1) % m;
                painter.line_segment([liftp(k, start), liftp(k2, start)], Stroke::new(1.5, col));
                painter.line_segment([liftp(k, start + total), liftp(k2, start + total)], Stroke::new(1.5, col));
                painter.line_segment([liftp(k, start), liftp(k, start + total)], Stroke::new(1.0, col));
            }
        }
        if let Some((base, dir, hh)) = self.feat_cmd_axis() {
            // the arrow points along the EFFECTIVE direction (flip -> the negated normal), so preview = result
            let dir = if self.feat.flip { [-dir[0], -dir[1], -dir[2]] } else { dir };
            let tip = [base[0] + dir[0] * hh, base[1] + dir[1] * hh, base[2] + dir[2] * hh];
            let s0 = self.project3(base, rect, &basis).0;
            let s1 = self.project3(tip, rect, &basis).0;
            let acol = if self.cmd.drag { self.scheme.pal.active() } else { self.scheme.pal.handle() };
            painter.line_segment([s0, s1], Stroke::new(2.5, acol));
            let d = s1 - s0;
            let len = d.length().max(1.0);
            let u = d / len;
            let perp = egui::vec2(-u.y, u.x);
            painter.add(egui::Shape::convex_polygon(vec![s1, s1 - u * 12.0 + perp * 5.0, s1 - u * 12.0 - perp * 5.0], acol, Stroke::NONE));
            painter.circle_filled(s1, 5.0, acol);
        }
    }


    /// A HANDLE AT A FACE: an arrow along the normal that the mouse drags. It is visible for as long as
    /// the face is selected - even at a zero value. Otherwise there would be nothing to drag: the arrow
    /// would appear only after the number had been typed in, that is, exactly when it is no longer needed.
    ///
    /// One handle for every command of the "pick a face, set a distance along its normal" kind (push, thicken):
    /// what it actually drags is decided by `face_arrow_key`.
    pub(super) fn draw_face_arrow(&self, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let Some((o, tip, _)) = self.face_arrow_geometry() else { return };
        let (a, b) = (self.project3(o, rect, basis).0, self.project3(tip, rect, basis).0);
        let hot = self.face_arrow_drag.is_some();
        let col = if hot { self.scheme.pal.highlight() } else { self.scheme.pal.handle_face() };
        painter.add(egui::Shape::line_segment([a, b], Stroke::new(if hot { 3.5 } else { 2.5 }, col)));
        painter.circle_filled(b, if hot { 6.0 } else { 5.0 }, col);
    }

    /// THE VERTICES OF THE SELECTED EDGES - the points a radius is set at.
    ///
    /// Without them a variable fillet would be a guessing game: a person does not know that a corner can be
    /// clicked. The ones that already have a radius of their own are larger and in the active colour: a
    /// display of state, not just an invitation.
    pub(super) fn draw_fillet_vertices(&self, painter: &egui::Painter, rect: Rect) {
        if self.cmd.kind != 4 || self.gsel.edges.is_empty() {
            return;
        }
        let Some(body) = self.edges.body else { return };
        let picked: Vec<[[f64; 3]; 2]> = self.project.regen_edges.get(&body).map(|es| es.iter().filter(|e| self.gsel.edges.contains(&e.id)).map(|e| [e.a, e.b]).collect()).unwrap_or_default();
        if picked.is_empty() {
            return;
        }
        let basis = self.cam.basis();
        for (pt, ids) in self.project.vertex_spots(body) {
            let on_picked = picked.iter().flatten().any(|p| (p[0] - pt[0]).abs() < 1e-6 && (p[1] - pt[1]).abs() < 1e-6 && (p[2] - pt[2]).abs() < 1e-6);
            if !on_picked {
                continue;
            }
            // THE NAME IS ASKED FOR, NOT CREATED: drawing has no business adding to the document's table.
            let own = self
                .project
                .names
                .vertex_desc(&qymcad_core::names::VertexName::new(ids))
                .is_some_and(|d| self.cmd.params.iter().any(|p| p.key == format!("at{d}")));
            let sc = self.project3(pt, rect, &basis).0;
            let (r, col) = if own { (4.5, self.scheme.pal.active()) } else { (3.0, self.scheme.pal.handle_face()) };
            painter.circle_filled(sc, r, col);
        }
    }

    /// Draw a body's edges (for picking): the selected ones (by id) in orange, the rest in grey. ONLY under
    /// the Chamfer/Fillet command (outside them edges are not pickable, so the view is not littered with a
    /// mesh and orange edges).
    pub(super) fn draw_body_edges(&self, painter: &egui::Painter, rect: Rect) {
        if !matches!(self.cmd.kind, 4 | 5 | 32) || self.edges.polys.is_empty() {
            return;
        }
        let basis = self.cam.basis();
        let wt = self.edges.body.map(|b| self.project.body_display_transform(b, self.current_ctx_id())).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
        };
        for (i, poly) in self.edges.polys.iter().enumerate() {
            let sel = self.edges.ids.get(i).is_some_and(|id| *id != 0 && self.gsel.edges.contains(id));
            let (col, w) = if sel { (self.scheme.pal.selected(), 2.6) } else { (self.scheme.pal.edge_idle(), 1.0) };
            let pts: Vec<Pos2> = poly.iter().map(|p| self.project3(tp(p), rect, &basis).0).collect();
            for k in 0..pts.len().saturating_sub(1) {
                painter.line_segment([pts[k], pts[k + 1]], Stroke::new(w, col));
            }
        }
    }



    /// Draw the section PLANE (a translucent quad across the scene) + the offset ARROW gizmo.
    pub(super) fn draw_section_gizmo(&self, painter: &egui::Painter, rect: Rect) {
        let Some((cp, u, v, half, tip)) = self.section_gizmo_geom() else { return };
        let basis = self.cam.basis();
        let pr = |p: [f64; 3]| self.project3(p, rect, &basis).0;
        let corners = [
            [cp[0] + (u[0] + v[0]) * half, cp[1] + (u[1] + v[1]) * half, cp[2] + (u[2] + v[2]) * half],
            [cp[0] + (u[0] - v[0]) * half, cp[1] + (u[1] - v[1]) * half, cp[2] + (u[2] - v[2]) * half],
            [cp[0] - (u[0] + v[0]) * half, cp[1] - (u[1] + v[1]) * half, cp[2] - (u[2] + v[2]) * half],
            [cp[0] - (u[0] - v[0]) * half, cp[1] - (u[1] - v[1]) * half, cp[2] - (u[2] - v[2]) * half],
        ];
        let pts: Vec<Pos2> = corners.iter().map(|c| pr(*c)).collect();
        let fill = crate::palette::a(self.scheme.pal.plane_fill(), 26);
        let edge = self.scheme.pal.plane_face();
        painter.add(egui::Shape::convex_polygon(pts.clone(), fill, Stroke::new(1.5, edge)));
        // the normal arrow (dragging it = offsetting the section)
        let (a, b) = (pr(cp), pr(tip));
        painter.line_segment([a, b], Stroke::new(2.5, edge));
        let dirv = (b - a).normalized();
        let nn = egui::vec2(-dirv.y, dirv.x);
        painter.add(egui::Shape::convex_polygon(
            vec![b, b - dirv * 12.0 + nn * 5.0, b - dirv * 12.0 - nn * 5.0],
            edge,
            Stroke::NONE,
        ));
        let hot = self.section.drag;
        painter.circle_filled(b, if hot { 7.0 } else { 5.5 }, if hot { self.scheme.pal.active() } else { edge });
    }


    /// A DIMMING + a spinner over the live interface. The model stays on the screen, so it is visible WHAT
    /// is being rebuilt; unlike the start-up splash, the window does not collapse into a black screen.
    /// AN INPUT BARRIER UNDER THE MODAL: a layer over everything that EATS clicks and drags.
    ///
    /// Reported behaviour: the dimming worked but did not block mouse clicks on the interface itself - the
    /// buttons and menus could still be pressed while it was up. The dimming really was only DRAWN, and the
    /// input was muted by a `ctx.input_mut(|i| i.events.clear())` line that came too late: `egui` gathers the
    /// input state at the start of the pass, before that line is reached. Clearing the events at that point
    /// decides nothing any more.
    ///
    /// This only works "from above": an interactive area in the upper layer takes the hit for itself, and the
    /// widgets below never get it.
    fn modal_input_barrier(&self, ctx: &egui::Context, salt: &'static str) {
        let screen = ctx.viewport_rect();
        egui::Area::new(egui::Id::new(salt)).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
            ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
        });
    }

    /// A BARRIER WITH A WINDOW: it mutes input everywhere EXCEPT `hole`.
    ///
    /// A rebuild forbids changing the document, but it need not forbid LOOKING. Orbiting, zooming and
    /// selecting do not touch the document (the `regen_doc_stamp` fingerprint is taken from the model, the
    /// camera is not part of it), so there is no reason to lock them away: a person used to sit in front of
    /// a dimmed screen and wait. The barrier stays where the buttons and panels live - an edit from there
    /// really would land on a stale copy.
    fn modal_input_barrier_except(&self, ctx: &egui::Context, hole: egui::Rect) {
        let s = ctx.viewport_rect();
        if !hole.is_positive() {
            self.modal_input_barrier(ctx, "regen_modal_barrier");
            return;
        }
        let strips = [
            egui::Rect::from_min_max(s.min, egui::pos2(s.max.x, hole.min.y)),                     // above
            egui::Rect::from_min_max(egui::pos2(s.min.x, hole.max.y), s.max),                     // below
            egui::Rect::from_min_max(egui::pos2(s.min.x, hole.min.y), egui::pos2(hole.min.x, hole.max.y)), // left
            egui::Rect::from_min_max(egui::pos2(hole.max.x, hole.min.y), egui::pos2(s.max.x, hole.max.y)), // right
        ];
        for (k, r) in strips.iter().enumerate() {
            if !r.is_positive() {
                continue;
            }
            egui::Area::new(egui::Id::new(("regen_barrier_strip", k))).order(egui::Order::Foreground).fixed_pos(r.min).interactable(true).show(ctx, |ui| {
                ui.allocate_response(r.size(), egui::Sense::click_and_drag());
            });
        }
    }

    /// THE QUIET-REBUILD SPINNER - in the centre of the canvas, with no text and no backing.
    ///
    /// Its only message: the body on show is STALE, a recompute is under way. No window, no barrier - the
    /// view orbits, selection works, edits are not blocked.
    pub(super) fn draw_quiet_spinner(&self, ctx: &egui::Context) {
        let rect = if self.view_rect.is_positive() { self.view_rect } else { ctx.viewport_rect() };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("quiet_spinner")));
        // THE PART GOES SLIGHTLY GREY - that is the main sign, and the spinner only confirms it.
        //
        // What was asked for: an unobtrusive mark that a tool is being applied right now and that the
        // geometry on screen will be rebuilt once the effect fades. The veil is faint and it is PAINT, not a
        // barrier: the view and the selection work straight through it. Its disappearance, on the other
        // hand, reads at once - grey means counting, clear means here is the new geometry.
        painter.rect_filled(rect, 0.0, crate::palette::a(self.scheme.pal.scrim(), 44));
        let t = ctx.input(|i| i.time) as f32;
        let c = rect.center();
        for k in 0..8 {
            let a = std::f32::consts::TAU * k as f32 / 8.0;
            let phase = (t * 2.0 - k as f32 / 8.0).fract();
            let alpha = (40.0 + 215.0 * (1.0 - phase)) as u8;
            painter.circle_filled(c + egui::vec2(a.cos(), a.sin()) * 16.0, 3.0, crate::palette::a(self.scheme.pal.text_strong(), alpha));
        }
        ctx.request_repaint();
    }

    /// The overlay for long work. Returns `true` if the cancel button was pressed.
    ///
    /// Only the tests call it - the program draws the overlay with the counter (`_with`). It is kept because
    /// a test must draw with THE SAME code as the program rather than with a copy of its own.
    #[cfg(test)]
    pub(super) fn draw_dim_overlay(&self, ctx: &egui::Context, label: &str) -> bool {
        self.draw_dim_overlay_with(ctx, label, None, egui::Rect::NOTHING)
    }

    /// The same, but with a COUNTER: `(done, total)` timeline nodes.
    ///
    /// A spinner on its own says one thing - "busy". On an assembly that takes seconds to compute a person
    /// needs something else: how much is left and whether they can change their mind. Both answers come from
    /// one place (the rebuild loop in the core, see `RegenWatch`), which is why they are shown together.
    pub(super) fn draw_dim_overlay_with(&self, ctx: &egui::Context, label: &str, progress: Option<(usize, usize)>, live: egui::Rect) -> bool {
        let screen = ctx.viewport_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("regen_dim")));
        // WHAT IS TO BE SHOWN IS MEASURED FIRST, and the card is then made to fit it. It used to be a
        // fixed 280 px wide with the label painted centred inside: a long line ("Rebuilding 28 nodes, one
        // of them a thread") ran out past both edges of the card and stood on the dimmed viewport.
        let prog_text = progress.map(|(done, total)| crate::i18n::tr2("io-rebuild-progress", "done", &done.to_string(), "total", &total.to_string()));
        let wrap = (screen.width() - 96.0).clamp(200.0, 520.0); // wraps rather than growing off a narrow window
        let title = painter.layout(label.to_owned(), egui::FontId::proportional(15.0), self.scheme.pal.text_strong(), wrap);
        let prog = prog_text.map(|t| painter.layout(t, egui::FontId::proportional(13.0), self.scheme.pal.text_dim(), wrap));
        let size = regen_card_size(title.size(), prog.as_ref().map(|g| g.size()), progress.is_some());

        // THE VIEWPORT STAYS LIVE AND UNDIMMED when it is given (`live`): the model is visible, the view
        // orbits, selection works. What gets dimmed and muted is exactly where the document is changed from.
        let card = if live.is_positive() {
            self.modal_input_barrier_except(ctx, live);
            for r in [
                egui::Rect::from_min_max(screen.min, egui::pos2(screen.max.x, live.min.y)),
                egui::Rect::from_min_max(egui::pos2(screen.min.x, live.max.y), screen.max),
                egui::Rect::from_min_max(egui::pos2(screen.min.x, live.min.y), egui::pos2(live.min.x, live.max.y)),
                egui::Rect::from_min_max(egui::pos2(live.max.x, live.min.y), egui::pos2(screen.max.x, live.max.y)),
            ] {
                if r.is_positive() {
                    painter.rect_filled(r, 0.0, crate::palette::a(self.scheme.pal.scrim(), 120));
                }
            }
            // the card sits AT THE BOTTOM of the viewport rather than in its centre: in the centre it would
            // cover exactly what the live window was kept for - the model itself
            egui::Rect::from_center_size(egui::pos2(live.center().x, live.max.y - size.y / 2.0 - 16.0), size)
        } else {
            self.modal_input_barrier(ctx, "regen_modal_barrier");
            painter.rect_filled(screen, 0.0, crate::palette::a(self.scheme.pal.scrim(), 120));
            egui::Rect::from_center_size(screen.center(), size)
        };
        let places = regen_card_places(card, title.size(), prog.as_ref().map(|g| g.size()), progress.is_some());
        painter.rect_filled(card, 10.0, self.scheme.pal.panel_bg());
        painter.rect_stroke(card, 10.0, egui::Stroke::new(1.0, self.scheme.pal.panel_border()), egui::StrokeKind::Middle);
        painter.galley(places.title.min, title, self.scheme.pal.text_strong());
        let mut cancelled = false;
        // A NODE COUNT rather than a percentage: nodes are the timeline's unit of work, and a fraction of it
        // would lie - a thread takes seconds, a datum is instant.
        if let (Some(g), Some(at)) = (prog, places.progress) {
            painter.galley(at.min, g, self.scheme.pal.text_dim());
        }
        // THE BUTTON IS A REAL WIDGET AND SITS ABOVE THE BARRIER. The barrier mutes input across the whole
        // screen, so the button's area is created AFTER it: otherwise the click would go to the barrier and
        // the button would stay silent.
        if let Some(bt) = places.button {
            egui::Area::new(egui::Id::new("regen_cancel")).order(egui::Order::Tooltip).fixed_pos(bt.min).show(ctx, |ui| {
                ui.set_max_size(bt.size());
                if ui.add_sized(bt.size(), egui::Button::new(format!("{} {}", ph::X, crate::i18n::tr("io-rebuild-cancel")))).clicked() {
                    cancelled = true;
                }
            });
        }
        // the "spinner" is drawn by hand: dots around a circle with a running brightness - no widget, over everything
        let t = ctx.input(|i| i.time) as f32;
        for k in 0..8 {
            let a = std::f32::consts::TAU * k as f32 / 8.0;
            let phase = (t * 2.0 - k as f32 / 8.0).fract();
            let alpha = (40.0 + 215.0 * (1.0 - phase)) as u8;
            painter.circle_filled(places.spinner + egui::vec2(a.cos(), a.sin()) * 13.0, 2.6, crate::palette::a(self.scheme.pal.text_strong(), alpha));
        }
        ctx.request_repaint();
        cancelled
    }


    /// THE SPLASH IS A SMALL WINDOW IN THE CENTRE, NOT THE WHOLE SCREEN.
    ///
    /// What was asked for: a small centred window with an icon, a name, a spinner and a description of what
    /// is happening. A full-screen splash says no more than a card does, yet it hides the whole program, and
    /// on a small project it only manages to blink.
    pub(super) fn draw_splash(&self, ctx: &egui::Context, label: &str) {
        let screen = ctx.viewport_rect();
        self.modal_input_barrier(ctx, "splash_modal_barrier");
        // THE BACKGROUND GOES IN THE LOWER LAYER, THE CARD IN THE UPPER ONE. Once the fill was put into
        // `Foreground` while the card stayed an ordinary window (`Order::Middle`), and the fill painted over
        // the card: what appeared was an empty white rectangle filling the window. The layer decides who is
        // on top of whom, and "drawn later" does not mean "visible" here.
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("splash_dim")))
            .rect_filled(screen, 0.0, self.scheme.pal.splash_bg());
        egui::Area::new(egui::Id::new("splash"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(300.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    if let Some(tex) = &self.logo_tex {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), egui::vec2(64.0, 64.0))));
                    }
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new("QymCAD").color(self.scheme.pal.text_strong()));
                    ui.add_space(10.0);
                    ui.add(egui::Spinner::new().size(22.0));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(label).size(13.0).color(self.scheme.pal.text_dim()));
                    ui.add_space(6.0);
                    });
                });
            });
    }


    /// Draw the component placement gizmo: 3 axis arrows X/Y/Z + 3 rotation rings.
    pub(super) fn draw_component_gizmo(&self, painter: &egui::Painter, rect: Rect) {
        // the JOINT's DOF gizmo (a driven component OR a directly picked glyph, the root's GLOBAL included)
        if let Some(jid) = self.active_dof_joint() {
            self.draw_joint_gizmo(painter, rect, jid);
            return;
        }
        let Some(comp) = self.gizmo_component() else {
            return;
        };
        // driven by a joint -> the DOF gizmo (the joint's freedoms only); grounded -> no gizmo; free -> 6 DOF.
        match self.comp_gizmo_mode(comp) {
            CompGizmoMode::None => {}
            CompGizmoMode::Joint(jid) => self.draw_joint_gizmo(painter, rect, jid),
            CompGizmoMode::Free => {
                let (o, l) = self.gizmo_geometry(comp);
                self.draw_gizmo_at(painter, rect, o, l, self.comp_giz.axis, self.comp_giz.ring);
                // the readout of the translation/rotation at the gizmo during a drag (as for a body)
                if let Some(text) = self.comp_giz_readout(self.comp_giz.snap) {
                    let s = self.project3(o, rect, &self.cam.basis()).0;
                    let suffix = if self.comp_giz.snap { "  snap" } else { "" };
                    painter.text(s + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{text}{suffix}"), egui::FontId::proportional(13.0), self.scheme.pal.gizmo_label());
                }
            }
        }
    }


    /// Draw the DOF gizmo: only the handles of the joint's freedoms (rings/arrows along the motion axes) + a readout.
    pub(super) fn draw_joint_gizmo(&self, painter: &egui::Painter, rect: Rect, jid: Id) {
        let Some((o, hs)) = self.joint_giz_handles(jid) else { return };
        let basis = self.cam.basis();
        let l = 60.0 / self.cam.scale as f64;
        let s0 = self.project3(o, rect, &basis).0;
        let hot = self.joint.giz_handle;
        let col_ring = self.scheme.pal.active(); // yellow means "a joint freedom" (matching the selected joint)
        let col_arr = self.scheme.pal.preview();
        // the rings
        for &(slot, ring, dir) in hs.iter().filter(|(_, r, _)| *r) {
            if !ring {
                continue;
            }
            let is_hot = hot == Some((slot, true));
            let (u, v) = perp_basis(dir);
            let pts: Vec<Pos2> = (0..=48)
                .map(|k| {
                    let a = k as f64 / 48.0 * std::f64::consts::TAU;
                    let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
                    self.project3(p, rect, &basis).0
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(if is_hot { 3.2 } else { 1.8 }, col_ring)));
        }
        // the arrows
        for &(slot, ring, dir) in hs.iter().filter(|(_, r, _)| !*r) {
            if ring {
                continue;
            }
            let is_hot = hot == Some((slot, false));
            let s1 = self.project3([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l], rect, &basis).0;
            painter.line_segment([s0, s1], Stroke::new(if is_hot { 4.0 } else { 2.5 }, col_arr));
            painter.circle_filled(s1, if is_hot { 7.5 } else { 5.0 }, col_arr);
        }
        // THE LIMITS ARE VISIBLE. The range of a degree of freedom is held by the solver and stops the drag,
        // but it used to exist only in the min/max fields: a person dragged the handle and hit an invisible
        // wall with no idea where it came from. Now a limited degree has its range drawn - dashed from the
        // minimum to the maximum - with cross ticks marking the stops at the ends.
        self.draw_joint_limits(painter, rect, jid, o, l, &hs);
        // THE PICKED AXIS IS VISIBLE. A person pointed at an edge and must see WHAT exactly they pointed at.
        // Otherwise "specify the axis" turns into an act of faith: something was picked, the part moved
        // somehow, and there is no way to check whether the one matches the other.
        self.draw_joint_axis_refs(painter, rect, jid, l);
        // the central marker of the joint axis
        painter.circle_stroke(s0, 3.0, Stroke::new(1.5, col_ring));
        // the readout of the current value during a drag
        if let Some((val, ring)) = self.joint_giz_value(self.comp_giz.snap) {
            let txt = if ring {
                format!("{val:+.1}{}", crate::i18n::tr("unit-deg-suffix"))
            } else {
                crate::i18n::tr1("unit-mm-value", "v", &crate::i18n::num_signed(val, 2))
            };
            let suffix = if self.comp_giz.snap { "  snap" } else { "" };
            painter.text(s0 + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{txt}{suffix}"), egui::FontId::proportional(13.0), self.scheme.pal.gizmo_label());
        }
    }


    /// THE RANGE OF A LIMITED DEGREE: dashes from the minimum to the maximum, with ticks at the stops.
    ///
    /// It is drawn exactly where the part will come to rest: the points are taken from THE SAME direction of
    /// the degree the solver computes with (`joint_slot_axis`), and the bounds from the same `limit_min/max`
    /// fields the drag is clamped by. Two pictures of one range would drift apart silently.
    fn draw_joint_limits(&self, painter: &egui::Painter, rect: Rect, jid: Id, o: [f64; 3], l: f64, hs: &[(u8, bool, [f64; 3])]) {
        let Some(j) = self.project.joints.iter().find(|x| x.id == jid) else { return };
        let basis = self.cam.basis();
        let col = self.scheme.pal.hint();
        for &(slot, ring, dir) in hs {
            let (lo, hi) = (j.limit_min[slot as usize], j.limit_max[slot as usize]);
            if lo.is_none() && hi.is_none() {
                continue; // the degree is unlimited - nothing to draw
            }
            if ring {
                // ROTATION: an arc along the ring from the minimum to the maximum, measured from the joint's own zero.
                let Some(zero) = self.project.joint_zero_dir(jid, self.current_ctx_id()) else { continue };
                let (lo, hi) = (lo.unwrap_or(-180.0).to_radians(), hi.unwrap_or(180.0).to_radians());
                let cross = [dir[1] * zero[2] - dir[2] * zero[1], dir[2] * zero[0] - dir[0] * zero[2], dir[0] * zero[1] - dir[1] * zero[0]];
                let at = |a: f64| {
                    let (s, c) = a.sin_cos();
                    [
                        o[0] + l * (zero[0] * c + cross[0] * s),
                        o[1] + l * (zero[1] * c + cross[1] * s),
                        o[2] + l * (zero[2] * c + cross[2] * s),
                    ]
                };
                let n = 24;
                let pts: Vec<Pos2> = (0..=n).map(|k| self.project3(at(lo + (hi - lo) * k as f64 / n as f64), rect, &basis).0).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(3.0, col)));
                for a in [lo, hi] {
                    let p = at(a);
                    let s = self.project3(p, rect, &basis).0;
                    let c = self.project3(o, rect, &basis).0;
                    let v = (s - c).normalized() * 7.0;
                    painter.line_segment([s - v, s + v], Stroke::new(2.5, col));
                }
                continue;
            }
            // TRANSLATION: dashes along the travel axis from the minimum to the maximum, with cross ticks at the ends.
            let (a, b) = (lo.unwrap_or(-l), hi.unwrap_or(l));
            let at = |t: f64| [o[0] + dir[0] * t, o[1] + dir[1] * t, o[2] + dir[2] * t];
            let n = 16;
            for k in 0..n {
                if k % 2 == 1 {
                    continue; // dashed: every other one
                }
                let t0 = a + (b - a) * k as f64 / n as f64;
                let t1 = a + (b - a) * (k + 1) as f64 / n as f64;
                painter.line_segment([self.project3(at(t0), rect, &basis).0, self.project3(at(t1), rect, &basis).0], Stroke::new(2.0, col));
            }
            for t in [a, b] {
                let s = self.project3(at(t), rect, &basis).0;
                let ahead = self.project3(at(t + l * 0.05), rect, &basis).0;
                let v = (ahead - s).normalized().rot90() * 7.0;
                painter.line_segment([s - v, s + v], Stroke::new(2.5, col));
            }
        }
    }

    /// THE GEOMETRY A PERSON POINTED AT AS THE ANCHOR AXIS, highlighted.
    ///
    /// A straight edge is drawn whole: what is seen is exactly what was pointed at. Everything else (a face,
    /// a circular edge, a datum plane) has no direction in the form of a segment - there a line is drawn
    /// THROUGH the anchor origin along the chosen direction, longer than the gizmo handles so that it is not
    /// mistaken for a degree of freedom.
    fn draw_joint_axis_refs(&self, painter: &egui::Painter, rect: Rect, jid: Id, l: f64) {
        use qymcad_core::feature::AnchorRef;
        let Some(j) = self.project.joints.iter().find(|x| x.id == jid) else { return };
        let ctx = self.current_ctx_id();
        let basis = self.cam.basis();
        let col = self.scheme.pal.active();
        for cid in [j.a, j.b] {
            let Some(c) = self.project.connector(cid) else { continue };
            let Some(r) = c.axis_ref.as_ref() else { continue };
            if let AnchorRef::EdgeMid(body, eid) = r {
                if let Some(e) = self.project.regen_edges.get(body).and_then(|es| es.iter().find(|e| e.id == *eid)) {
                    if !e.is_circular() {
                        let wt = self.project.body_display_transform(*body, ctx);
                        let tp = |v: [f64; 3]| self.project3(qymcad_core::feature::apply12(&wt, v), rect, &basis).0;
                        painter.line_segment([tp(e.a), tp(e.b)], Stroke::new(3.5, col));
                        continue;
                    }
                }
            }
            // not a segment - draw the direction through the anchor origin
            let (Some(m), Some(dir)) = (self.project.connector_matrix(cid), self.project.anchor_direction(r)) else { continue };
            let owner = self.project.connector(cid).map(|c| c.owner).unwrap_or(ctx);
            let wt = qymcad_core::feature::mat_mul12(&self.project.relative_transform(owner, ctx), &m);
            let o = [wt[3], wt[7], wt[11]];
            let d = qymcad_core::feature::apply12(&self.project.relative_transform(owner, ctx), dir);
            let z = qymcad_core::feature::apply12(&self.project.relative_transform(owner, ctx), [0.0; 3]);
            let d = [d[0] - z[0], d[1] - z[1], d[2] - z[2]];
            let at = |t: f64| self.project3([o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t], rect, &basis).0;
            painter.line_segment([at(-1.5 * l), at(1.5 * l)], Stroke::new(3.5, col));
        }
    }

    /// Draw the gizmo (3 axis arrows + 3 rings) at origin `o` and scale `l`, with the hot axis/ring lit.
    /// The shared renderer for the COMPONENT gizmo (Assembly) and the BODY gizmo (Part).
    pub(super) fn draw_gizmo_at(&self, painter: &egui::Painter, rect: Rect, o: [f64; 3], l: f64, hot_axis: Option<u8>, hot_ring: Option<u8>) {
        let basis = self.cam.basis();
        let s0 = self.project3(o, rect, &basis).0;
        let cols = [self.scheme.pal.axis(0), self.scheme.pal.axis(1), self.scheme.pal.axis(2)];
        // the rotation rings (under the arrows)
        for ax in 0..3usize {
            let (u, v) = ring_axes(ax as u8);
            let hot = hot_ring == Some(ax as u8);
            let pts: Vec<Pos2> = (0..=48)
                .map(|k| {
                    let a = k as f64 / 48.0 * std::f64::consts::TAU;
                    let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
                    self.project3(p, rect, &basis).0
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(if hot { 2.8 } else { 1.3 }, cols[ax])));
        }
        // the axis arrows (over the rings)
        for ax in 0..3usize {
            let mut tip = o;
            tip[ax] += l;
            let s1 = self.project3(tip, rect, &basis).0;
            let hot = hot_axis == Some(ax as u8);
            painter.line_segment([s0, s1], Stroke::new(if hot { 4.0 } else { 2.5 }, cols[ax]));
            painter.circle_filled(s1, if hot { 7.5 } else { 5.0 }, cols[ax]);
        }
    }


    /// A WIREFRAME preview of the primitive being created, at the origin, from the command's current sizes.
    /// It needs no kernel, so it is cheap and updates on the fly. The primitive placements are the OCCT
    /// defaults (a cube/prism centred in XY from z=0; a cylinder/cone along +Z from z=0; a sphere/torus
    /// centred at the origin).
    pub(super) fn draw_prim_preview(&self, painter: &egui::Painter, rect: Rect) {
        // only while CREATING (there is no body). While editing (feat_edit) the body is already drawn, so the wireframe is not duplicated.
        if !(10..=15).contains(&self.cmd.kind) || self.cmd.edit.is_some() {
            return;
        }
        let basis = self.cam.basis();
        let frame = self.prim.frame; // the preview sits in the oriented placement frame (otherwise at the origin)
        let st = Stroke::new(1.5, self.scheme.pal.preview_prim());
        let v = |k: &str| self.cmd_val(k);
        let pr = |q: [f64; 3]| {
            let w = match frame {
                Some(m) => qymcad_core::feature::apply12(&m, q),
                None => q,
            };
            self.project3(w, rect, &basis).0
        };
        let ring_xy = |r: f64, z: f64, n: usize| -> Vec<Pos2> {
            (0..=n).map(|i| { let a = std::f64::consts::TAU * i as f64 / n as f64; pr([r * a.cos(), r * a.sin(), z]) }).collect()
        };
        let poly = |painter: &egui::Painter, pts: &[Pos2]| {
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], st);
            }
        };
        let seg = |painter: &egui::Painter, a: [f64; 3], b: [f64; 3]| painter.line_segment([pr(a), pr(b)], st);
        match self.cmd.kind {
            10 => {
                let (hx, hy, dz) = (v("dx") / 2.0, v("dy") / 2.0, v("dz"));
                let c = [[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]];
                for &z in &[0.0, dz] {
                    let r: Vec<Pos2> = c.iter().chain(std::iter::once(&c[0])).map(|p| pr([p[0], p[1], z])).collect();
                    poly(painter, &r);
                }
                for p in c {
                    seg(painter, [p[0], p[1], 0.0], [p[0], p[1], dz]);
                }
            }
            11 => {
                let (r, h) = (v("r"), v("h"));
                poly(painter, &ring_xy(r, 0.0, 48));
                poly(painter, &ring_xy(r, h, 48));
                for a in [0.0, 90.0, 180.0, 270.0_f64] {
                    let a = a.to_radians();
                    seg(painter, [r * a.cos(), r * a.sin(), 0.0], [r * a.cos(), r * a.sin(), h]);
                }
            }
            12 => {
                let r = v("r");
                poly(painter, &ring_xy(r, 0.0, 48));
                let ring_v = |swap: bool| -> Vec<Pos2> { (0..=48).map(|i| { let a = std::f64::consts::TAU * i as f64 / 48.0; let (c, s) = (r * a.cos(), r * a.sin()); pr(if swap { [0.0, c, s] } else { [c, 0.0, s] }) }).collect() };
                poly(painter, &ring_v(false));
                poly(painter, &ring_v(true));
            }
            13 => {
                let (r1, r2, h) = (v("r1"), v("r2"), v("h"));
                if r1 > 1e-6 {
                    poly(painter, &ring_xy(r1, 0.0, 48));
                }
                if r2 > 1e-6 {
                    poly(painter, &ring_xy(r2, h, 48));
                }
                for a in [0.0, 90.0, 180.0, 270.0_f64] {
                    let a = a.to_radians();
                    seg(painter, [r1 * a.cos(), r1 * a.sin(), 0.0], [r2 * a.cos(), r2 * a.sin(), h]);
                }
            }
            14 => {
                let (mr, tr) = (v("major"), v("minor"));
                poly(painter, &ring_xy(mr + tr, 0.0, 64));
                poly(painter, &ring_xy((mr - tr).max(0.0), 0.0, 64));
            }
            15 => {
                let (r, h, n) = (v("r"), v("h"), self.prim.n.max(3) as usize);
                poly(painter, &ring_xy(r, 0.0, n));
                poly(painter, &ring_xy(r, h, n));
                for i in 0..n {
                    let a = std::f64::consts::TAU * i as f64 / n as f64;
                    seg(painter, [r * a.cos(), r * a.sin(), 0.0], [r * a.cos(), r * a.sin(), h]);
                }
            }
            _ => {}
        }
    }


    /// THE PATTERN PREVIEW: wireframe ghosts of the copies (the source body's bbox) at the instance
    /// positions, before Enter. Linear: a 3D grid of i*d1 + j*d2 + k*d3. Circular: a rotation about the axis
    /// (world Z or a datum). The original is not drawn.
    /// It is drawn WHILE EDITING too: there the pattern result is hidden (`edit_result_body`) and the source
    /// is visible, so the ghosts are the preview.
    pub(super) fn draw_array_preview(&self, painter: &egui::Painter, rect: Rect) {
        if !matches!(self.cmd.kind, 17 | 18) || !self.mode_3d {
            return;
        }
        let Some(src) = self.selected_body() else { return };
        let Some(mi) = self.project.mesh_index(src) else { return };
        let Some(bb) = self.project.bodies[mi].mesh.bounds() else { return };
        let basis = self.cam.basis();
        let st = Stroke::new(1.2, crate::palette::a(self.scheme.pal.preview_array(), 170));
        let (mn, mx) = (bb.min, bb.max);
        let base: [[f64; 3]; 8] = [
            [mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z],
            [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z],
        ];
        const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
        let draw_box = |wc: [[f64; 3]; 8]| {
            let pts: [Pos2; 8] = std::array::from_fn(|i| self.project3(wc[i], rect, &basis).0);
            for (a, b) in EDGES {
                painter.line_segment([pts[a], pts[b]], st);
            }
        };
        if self.cmd.kind == 17 {
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
            for i in 0..self.arr.count.max(1) {
                for j in 0..c2 {
                    for l in 0..c3 {
                        if i == 0 && j == 0 && l == 0 {
                            continue; // the original is already drawn as the body (or as the source while editing)
                        }
                        let (tx, ty, tz) = (
                            i as f64 * dx + j as f64 * dx2 + l as f64 * dx3,
                            i as f64 * dy + j as f64 * dy2 + l as f64 * dy3,
                            i as f64 * dz + j as f64 * dz2 + l as f64 * dz3,
                        );
                        draw_box(std::array::from_fn(|k| [base[k][0] + tx, base[k][1] + ty, base[k][2] + tz]));
                    }
                }
            }
        } else {
            let c = self.arr.count.max(1);
            let angle = if self.arr.full { 360.0 } else { self.cmd_val("angle") };
            let step = if angle.abs() >= 359.9 { 360.0 / c as f64 } else { angle / c as f64 };
            let (org, dir) = if self.arr.axis != 0 {
                self.project.datum_axes.iter().find(|d| d.id == self.arr.axis).map(|d| (d.origin(), d.dir())).unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]))
            } else {
                ([0.0; 3], [0.0, 0.0, 1.0])
            };
            for i in 1..c {
                let ang = (i as f64 * step).to_radians();
                draw_box(std::array::from_fn(|k| rotate_pt_about_axis(org, dir, ang, base[k])));
            }
        }
    }


    /// THE SPLIT PREVIEW: the cutting plane itself (a square sized by the body's extent) and the section line
    /// along the body's edges. Without a preview the tool would be blind: before Enter neither where the cut
    /// will run nor whether it hits the body at all would be visible.
    pub(super) fn draw_split_preview(&self, painter: &egui::Painter, rect: Rect) {
        if !matches!(self.cmd.kind, 27 | 29) || !self.mode_3d {
            return;
        }
        let Some(sp) = self.split.plane.clone() else { return };
        let Some((o, n)) = self.mirror_plane_world(&sp) else { return };
        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if nl < 1e-9 {
            return;
        }
        let u3 = [n[0] / nl, n[1] / nl, n[2] / nl];
        let d = self.cmd_val("offset");
        let c = [o[0] + u3[0] * d, o[1] + u3[1] * d, o[2] + u3[2] * d];
        let Some(src) = self.op_target_body() else { return };
        let Some(mi) = self.project.mesh_index(src) else { return };
        let Some(bb) = self.project.bodies[mi].mesh.bounds() else { return };
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let wt = self.project.body_display_transform(src, ctx);
        // THE SQUARE'S SIZE comes from the body's extent: a fixed 30 mm would look like a thread across a
        // large part and would cover the whole scene on a small one
        let half = {
            let c0 = qymcad_core::feature::apply12(&wt, [bb.min.x, bb.min.y, bb.min.z]);
            let c1 = qymcad_core::feature::apply12(&wt, [bb.max.x, bb.max.y, bb.max.z]);
            (((c1[0] - c0[0]).powi(2) + (c1[1] - c0[1]).powi(2) + (c1[2] - c0[2]).powi(2)).sqrt() * 0.5).max(1.0)
        };
        let up = if u3[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let normd = |a: [f64; 3]| {
            let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let uu = normd(cross(up, u3));
        let vv = cross(u3, uu);
        let corner = |su: f64, sv: f64| [c[0] + uu[0] * su * half + vv[0] * sv * half, c[1] + uu[1] * su * half + vv[1] * sv * half, c[2] + uu[2] * su * half + vv[2] * sv * half];
        let poly: Vec<Pos2> = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)].iter().map(|p| self.project3(*p, rect, &basis).0).collect();
        let st = Stroke::new(1.6, crate::palette::a(self.scheme.pal.modify(), 220));
        painter.add(egui::Shape::convex_polygon(poly, crate::palette::a(self.scheme.pal.modify(), 40), st));

        // THE CUT LINE ACROSS THE BODY: the mesh edges the plane crosses give the outline of the future
        // seam - it shows at once whether the plane cuts the body or misses it.
        let mesh = &self.project.bodies[mi].mesh;
        let side = |p: [f64; 3]| {
            let w = qymcad_core::feature::apply12(&wt, p);
            (w[0] - c[0]) * u3[0] + (w[1] - c[1]) * u3[1] + (w[2] - c[2]) * u3[2]
        };
        let cut = Stroke::new(2.0, self.scheme.pal.cut_line());
        for ti in 0..mesh.tris.len() {
            let t = mesh.triangle(ti);
            let pts = [[t[0].x, t[0].y, t[0].z], [t[1].x, t[1].y, t[1].z], [t[2].x, t[2].y, t[2].z]];
            let ds = [side(pts[0]), side(pts[1]), side(pts[2])];
            let mut hits: Vec<Pos2> = Vec::new();
            for e in 0..3 {
                let (a, b) = (e, (e + 1) % 3);
                if (ds[a] > 0.0) == (ds[b] > 0.0) {
                    continue;
                }
                let k = ds[a] / (ds[a] - ds[b]);
                let p = [pts[a][0] + (pts[b][0] - pts[a][0]) * k, pts[a][1] + (pts[b][1] - pts[a][1]) * k, pts[a][2] + (pts[b][2] - pts[a][2]) * k];
                hits.push(self.project3(qymcad_core::feature::apply12(&wt, p), rect, &basis).0);
            }
            if hits.len() == 2 {
                painter.line_segment([hits[0], hits[1]], cut);
            }
        }
    }

    /// THE PREVIEW OF THE MIRROR: a ghost wireframe of the body reflected through the chosen plane,
    /// before Enter (both creation AND editing: while editing, the result is hidden by
    /// `edit_result_body` and the source is visible, so the ghost is the preview of the mirror).
    pub(super) fn draw_mirror_preview(&self, painter: &egui::Painter, rect: Rect) {
        if self.cmd.kind != 16 || !self.mode_3d {
            return;
        }
        let Some(sp) = self.mirror.plane.clone() else { return };
        let Some((o, n)) = self.mirror_plane_world(&sp) else { return };
        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if nl < 1e-9 {
            return;
        }
        let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
        let Some(src) = self.selected_body() else { return };
        let Some(mi) = self.project.mesh_index(src) else { return };
        let Some(bb) = self.project.bodies[mi].mesh.bounds() else { return };
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let wt = self.project.body_display_transform(src, ctx);
        let st = Stroke::new(1.3, crate::palette::a(self.scheme.pal.preview_datum(), 190));
        let (mn, mx) = (bb.min, bb.max);
        let base: [[f64; 3]; 8] = [
            [mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z],
            [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z],
        ];
        const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
        // reflecting a point through the plane (o, nn): p' = p - 2*((p-o).nn)*nn (local -> world by the display transform first)
        let reflect = |p: [f64; 3]| -> [f64; 3] {
            let pw = qymcad_core::feature::apply12(&wt, p);
            let d = (pw[0] - o[0]) * nn[0] + (pw[1] - o[1]) * nn[1] + (pw[2] - o[2]) * nn[2];
            [pw[0] - 2.0 * d * nn[0], pw[1] - 2.0 * d * nn[1], pw[2] - 2.0 * d * nn[2]]
        };
        let pts: [Pos2; 8] = std::array::from_fn(|i| self.project3(reflect(base[i]), rect, &basis).0);
        for (a, b) in EDGES {
            painter.line_segment([pts[a], pts[b]], st);
        }
    }


    /// Highlighting the axis candidates while the circular pattern's axis is being clicked: the datum axes +
    /// the straight edges of the source body; the one under the cursor and the selected one are brighter. The
    /// same single click-pick as the mirror plane highlight.
    pub(super) fn draw_axis_picker(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::feature::{apply12, is_identity12};
        // active while picking the pattern axis (18), the Revolve axis (3, 64) OR in the DATUM AXIS command by edge/face (22, mode 0)
        let active = (self.cmd.kind == 18 && self.arr.axis_pick) || (self.cmd.kind == 3 && self.rev.pick_axis) || (self.cmd.kind == 22 && self.datum.axis_mode == 0);
        if !active || !self.mode_3d {
            return;
        }
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let hovered = painter.ctx().pointer_hover_pos().and_then(|p| self.pick_axis_at(rect, p));
        for d in &self.project.datum_axes {
            if let Some(wt) = self.datum_render_transform(d.id) {
                let (s, e) = axis_segment(d.origin(), d.dir(), 45.0);
                let a = self.project3(apply12(&wt, s), rect, &basis).0;
                let b = self.project3(apply12(&wt, e), rect, &basis).0;
                let hot = matches!(hovered, Some(AxisHit::Datum(id)) if id == d.id) || self.arr.axis == d.id;
                let col = if hot { self.scheme.pal.highlight() } else { self.scheme.pal.preview() };
                painter.line_segment([a, b], Stroke::new(if hot { 2.8 } else { 1.4 }, col));
            }
        }
        // ONLY THE EDGE UNDER THE CURSOR, NOT ALL OF THEM AT ONCE.
        //
        // This used to draw EVERY eligible edge of the assembly as a pale line, with the one under the cursor
        // brighter. On two cubes that is a hint; on a real machine it is a solid green wall of thousands of
        // lines, behind which neither the part nor the highlight itself can be seen.
        //
        // What is shown is what a person pointed at - as everywhere else in the highlighting.
        if let Some(AxisHit::Edge(k)) = hovered {
            if let Some((body, _id, poly)) = self.edges.axes.get(k) {
                let wt = self.project.body_display_transform(*body, ctx);
                let pts: Vec<Pos2> = poly.iter().map(|p| self.project3(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64]), rect, &basis).0).collect();
                for i in 0..pts.len().saturating_sub(1) {
                    painter.line_segment([pts[i], pts[i + 1]], Stroke::new(2.8, self.scheme.pal.highlight()));
                }
            }
        }
        // a CYLINDRICAL face under the cursor: a fill + its axis (so that it is visible the click takes the hole's axis)
        if let Some(AxisHit::Face(body, fid)) = hovered {
            if let Some(mi) = self.project.mesh_index(body) {
                if let Some(fi) = self.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.id == fid)) {
                    let mesh = &self.project.bodies[mi].mesh;
                    let fwt = self.project.body_display_transform(body, ctx);
                    let ftp = |v: [f64; 3]| if is_identity12(&fwt) { v } else { apply12(&fwt, v) };
                    let fill = crate::palette::a(self.scheme.pal.highlight(), 80);
                    let mut hm = egui::Mesh::default();
                    for &tri in &self.project.bodies[mi].faces[fi].triangles {
                        let t = mesh.triangle(tri as usize);
                        let b = hm.vertices.len() as u32;
                        for v in &t {
                            hm.colored_vertex(self.project3(ftp([v.x, v.y, v.z]), rect, &basis).0, fill);
                        }
                        hm.add_triangle(b, b + 1, b + 2);
                    }
                    if !hm.is_empty() {
                        painter.add(egui::Shape::mesh(hm));
                    }
                    // the face axis (world origin/dir) as a line
                    if let Some((lo, ld)) = self.live.shapes.get(&body).and_then(|s| s.face_axis(fid)) {
                        let ow = ftp(lo);
                        let z = if is_identity12(&fwt) { [0.0; 3] } else { apply12(&fwt, [0.0, 0.0, 0.0]) };
                        let dw = if is_identity12(&fwt) { ld } else { [apply12(&fwt, ld)[0] - z[0], apply12(&fwt, ld)[1] - z[1], apply12(&fwt, ld)[2] - z[2]] };
                        let (s, e) = axis_segment(ow, dw, 45.0);
                        painter.line_segment([self.project3(s, rect, &basis).0, self.project3(e, rect, &basis).0], Stroke::new(2.8, self.scheme.pal.highlight()));
                    }
                }
            }
        }
    }


    /// THE DATUM COMMAND PREVIEW: 20 a plane as a square (offset from the reference), 21 a point as a cross, 22 an axis as a line.
    pub(super) fn draw_datum_preview(&self, painter: &egui::Painter, rect: Rect) {
        if !matches!(self.cmd.kind, 20 | 21 | 22) || !self.mode_3d {
            return;
        }
        let basis = self.cam.basis();
        let st = Stroke::new(1.6, crate::palette::a(self.scheme.pal.preview_datum(), 205));
        match self.cmd.kind {
            20 => {
                let Some(sp) = self.datum.plane_pick.clone() else { return };
                let Some((o, n)) = self.mirror_plane_world(&sp) else { return };
                let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if nl < 1e-9 {
                    return;
                }
                let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
                let dist = self.cmd_val("dist");
                let c = [o[0] + nn[0] * dist, o[1] + nn[1] * dist, o[2] + nn[2] * dist];
                // an orthonormal frame (u,v) in the plane
                let up = if nn[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
                let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
                let normd = |a: [f64; 3]| { let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9); [a[0] / l, a[1] / l, a[2] / l] };
                let u = normd(cross(up, nn));
                let v = cross(nn, u);
                let h = 30.0;
                let corner = |su: f64, sv: f64| [c[0] + u[0] * su * h + v[0] * sv * h, c[1] + u[1] * su * h + v[1] * sv * h, c[2] + u[2] * su * h + v[2] * sv * h];
                let poly: Vec<Pos2> = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)].iter().map(|p| self.project3(*p, rect, &basis).0).collect();
                painter.add(egui::Shape::convex_polygon(poly, crate::palette::a(self.scheme.pal.preview_datum(), 45), st));
            }
            21 => {
                use qymcad_core::feature::{apply12, is_identity12};
                // the preview point's position: "at a vertex" -> the world position of the picked vertex; "coordinates" -> the X/Y/Z fields
                let p = if self.datum.pt_mode == 1 {
                    match self.datum.pt_vert {
                        Some((body, _, _, at)) => {
                            let wt = self.project.body_display_transform(body, self.current_ctx_id());
                            if is_identity12(&wt) { at } else { apply12(&wt, at) }
                        }
                        None => {
                            // nothing picked yet - only the snap highlight of the candidate under the cursor
                            if let Some(w) = painter.ctx().pointer_hover_pos().and_then(|hp| self.pick_vertex_pos(rect, hp)) {
                                painter.circle_stroke(self.project3(w, rect, &basis).0, 5.0, Stroke::new(2.0, self.scheme.pal.highlight()));
                            }
                            return;
                        }
                    }
                } else {
                    [self.cmd_val("x"), self.cmd_val("y"), self.cmd_val("z")]
                };
                let d = 5.0;
                for ax in 0..3 {
                    let (mut a, mut b) = (p, p);
                    a[ax] -= d;
                    b[ax] += d;
                    painter.line_segment([self.project3(a, rect, &basis).0, self.project3(b, rect, &basis).0], st);
                }
                painter.circle_filled(self.project3(p, rect, &basis).0, 3.0, self.scheme.pal.preview_datum());
                // snap: highlight the nearest VERTEX under the cursor (the "coordinates" mode)
                if self.datum.pt_mode == 0 {
                    if let Some(w) = painter.ctx().pointer_hover_pos().and_then(|hp| self.pick_vertex_pos(rect, hp)) {
                        painter.circle_stroke(self.project3(w, rect, &basis).0, 5.0, Stroke::new(2.0, self.scheme.pal.highlight()));
                    }
                }
            }
            22 => {
                let od = if self.datum.axis_mode == 1 {
                    Some(([self.cmd_val("ox"), self.cmd_val("oy"), self.cmd_val("oz")], [self.cmd_val("dx"), self.cmd_val("dy"), self.cmd_val("dz")]))
                } else {
                    self.datum.axis_ref
                };
                if let Some((o, d)) = od {
                    if d[0].abs() + d[1].abs() + d[2].abs() > 1e-9 {
                        let (s, e) = axis_segment(o, d, 45.0);
                        painter.line_segment([self.project3(s, rect, &basis).0, self.project3(e, rect, &basis).0], Stroke::new(2.4, self.scheme.pal.preview_datum()));
                    }
                }
                // the "two points" mode: the points gathered so far + the snap highlight of the candidate (a datum point or a vertex) under the cursor
                if self.datum.axis_mode == 2 {
                    for (_, w) in &self.datum.axis_pts {
                        painter.circle_filled(self.project3(*w, rect, &basis).0, 4.0, self.scheme.pal.preview_datum());
                    }
                    let hov = painter.ctx().pointer_hover_pos().and_then(|hp| self.pick_datum_point_at(rect, hp).map(|(_, w)| w).or_else(|| self.pick_vertex_pos(rect, hp)));
                    if let Some(w) = hov {
                        painter.circle_stroke(self.project3(w, rect, &basis).0, 5.0, Stroke::new(2.0, self.scheme.pal.highlight()));
                    }
                }
            }
            _ => {}
        }
    }


    /// Draw the body gizmo: 3 axes + rings at the selected body + a readout of the value during a drag.
    pub(super) fn draw_body_gizmo(&self, painter: &egui::Painter, rect: Rect) {
        let Some((_, mi)) = self.body_gizmo_target() else {
            return;
        };
        let (o, l) = self.body_gizmo_geometry(mi);
        self.draw_gizmo_at(painter, rect, o, l, self.body_giz.axis, self.body_giz.ring);
        if let Some(text) = self.body_giz_readout(self.body_giz.snap) {
            let s = self.project3(o, rect, &self.cam.basis()).0;
            let suffix = if self.body_giz.snap { "  snap" } else { "" };
            painter.text(s + egui::vec2(14.0, -14.0), egui::Align2::LEFT_BOTTOM, format!("{text}{suffix}"), egui::FontId::proportional(13.0), self.scheme.pal.gizmo_label());
        }
    }


    /// Highlighting faces while a mate is being picked: the selected face A in green, the one under the
    /// cursor in blue. The face transform is the display one (the active context's frame, as for render and pick).
    pub(super) fn draw_joint_pick_highlight(&self, painter: &egui::Painter, rect: Rect) {
        // EVERY TOOL THAT ASKS FOR GEOMETRY, not only the mate pick.
        //
        // The highlight is needed while a joint is being PICKED (joint_pick_faces) and while an existing
        // joint's ANCHOR is being CHANGED (joint_edit_repick) - otherwise edges and faces do not light up
        // under the cursor and the anchor is chosen blind.
        //
        // The modes used to be listed here by name, and every new tool was forgotten: first the highlight
        // went silent while a secondary axis was being specified (reported behaviour: moving the cursor over
        // the part lit nothing at all), and then the `every_picking_tool_highlights` guard found FOUR more of
        // the same - a separate anchor, a group, a width, a tangency. A person was aiming blind in each of them.
        //
        // ONE LIST IS ASKED (`gui/assembly_tools.rs`) rather than the modes being listed here: a list of its
        // own in every place is exactly the illness that left five tools without a highlight and five that
        // Esc would not release.
        //
        // EDITING A JOINT IS ALSO A REASON TO LIGHT UP. No tool need be in hand at all while a person looks at
        // a joint and edits it: they must see WHERE its anchors sit.
        if !self.assembly_wants_geometry() && self.joint.edit.is_none() {
            return;
        }
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        use qymcad_core::feature::AnchorRef;
        let hl = |body: Id, fi: usize, col: Color32| {
            if let Some(mi) = self.project.mesh_index(body) {
                if let Some(face) = self.project.bodies.get(mi).and_then(|b| b.faces.get(fi)) {
                    let mesh = &self.project.bodies[mi].mesh;
                    let wt = self.project.body_display_transform(body, ctx);
                    let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
                    // one fill mesh - with no "needles" from the radial edges of the fan triangulation (as in the picker)
                    let mut hm = egui::Mesh::default();
                    for &tri in &face.triangles {
                        let t = mesh.triangle(tri as usize);
                        let base = hm.vertices.len() as u32;
                        for v in &t {
                            hm.colored_vertex(self.project3(tp([v.x, v.y, v.z]), rect, &basis).0, col);
                        }
                        hm.add_triangle(base, base + 1, base + 2);
                    }
                    if !hm.is_empty() {
                        painter.add(egui::Shape::mesh(hm));
                    }
                }
            }
        };
        // highlighting an EDGE by its persistent id
        let hl_edge = |body: Id, eid: u32, col: Color32| {
            // THE EDGES COME FROM THE CACHE. There used to be a direct call into the kernel here, while the
            // highlight is drawn EVERY FRAME for as long as a joint is being picked - even with the mouse
            // standing still. On a real part that is a full extraction of the edges from the B-rep three
            // times per frame, and that is where "placed a mate, waited, the CAD stopped answering" came
            // from. The data are the same; the price is now paid once per rebuild.
            if let Some(edges) = self.body_edges_cached(body) {
                let (polys, ids) = (&edges.0, &edges.1);
                let wt = self.project.body_display_transform(body, ctx);
                let tp = |p: &[f32; 3]| -> [f64; 3] {
                    let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                    if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
                };
                for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                    if id == eid {
                        let sp: Vec<Pos2> = poly.iter().map(|p| self.project3(tp(p), rect, &basis).0).collect();
                        painter.add(egui::Shape::line(sp, Stroke::new(3.0, col)));
                    }
                }
                // a CIRCULAR edge (the rim of a hole or a cylinder): the CENTRE AXIS is drawn - a concentric
                // anchor rather than a point on the rim. The centre and the axis come from the regen cache
                // (the true circle from OCCT).
                if let Some(e) = self.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| e.id == eid)) {
                    if e.is_circular() {
                        let (c, ax) = (e.center, e.axis);
                        let l = (e.radius * 2.0).max(6.0);
                        let a = [c[0] - ax[0] * l, c[1] - ax[1] * l, c[2] - ax[2] * l];
                        let b = [c[0] + ax[0] * l, c[1] + ax[1] * l, c[2] + ax[2] * l];
                        let (pa, pb) = (self.project3(tp(&[a[0] as f32, a[1] as f32, a[2] as f32]), rect, &basis).0, self.project3(tp(&[b[0] as f32, b[1] as f32, b[2] as f32]), rect, &basis).0);
                        painter.add(egui::Shape::dashed_line(&[pa, pb], Stroke::new(1.5, col), 6.0, 4.0));
                        painter.circle_filled(self.project3(tp(&[c[0] as f32, c[1] as f32, c[2] as f32]), rect, &basis).0, 3.5, col);
                    }
                }
            }
        };
        // highlighting a VERTEX (an edge end) with a dot
        let hl_vert = |body: Id, eid: u32, end: bool, col: Color32| {
            if let Some(edges) = self.body_edges_cached(body) {
                let (polys, ids) = (&edges.0, &edges.1);
                let wt = self.project.body_display_transform(body, ctx);
                let tp = |p: &[f32; 3]| -> [f64; 3] {
                    let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                    if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
                };
                for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                    if id == eid && poly.len() >= 2 {
                        let vtx = if end { &poly[poly.len() - 1] } else { &poly[0] };
                        painter.circle_filled(self.project3(tp(vtx), rect, &basis).0, 5.0, col);
                    }
                }
            }
        };
        let green = crate::palette::a(self.scheme.pal.joint_pick_a(), 200);
        let blue = crate::palette::a(self.scheme.pal.joint_pick_b(), 200);
        // THE ANCHORS OF THE JOINT BEING EDITED - both of them, each in its own colour: A green, B blue, as when picking.
        if let Some(jid) = self.joint.edit {
            if let Some(j) = self.project.joints.iter().find(|x| x.id == jid) {
                for (cid, col) in [(j.a, green), (j.b, blue)] {
                    let Some(anchor) = self.project.connector(cid).map(|c| c.anchor.clone()) else { continue };
                    match anchor {
                        AnchorRef::FaceCenter(b, k) => hl(b, k.index as usize, col),
                        AnchorRef::EdgeMid(b, eid) => hl_edge(b, eid, col),
                        AnchorRef::Vertex(b, eid, end) => hl_vert(b, eid, end, col),
                        _ => {}
                    }
                }
            }
        }
        // the anchor A that is already fixed
        if let Some((_, anchor)) = &self.joint.pick_first {
            match anchor {
                AnchorRef::FaceCenter(b, k) => hl(*b, k.index as usize, green),
                AnchorRef::EdgeMid(b, eid) => hl_edge(*b, *eid, green),
                AnchorRef::Vertex(b, eid, end) => hl_vert(*b, *eid, *end, green),
                _ => {}
            }
        }
        // under the cursor, ONLY the parts of the active context. The ghost (show_context) bodies of other
        // subassemblies are dimmed for a reason: under a joint tool they are inactive and are not highlighted,
        // so that they do not get in the way. A joint is hung inside the context anyway.
        let in_ctx = |body: Id| self.project.body_owner(body).is_some_and(|o| self.project.component_is_within(o, ctx));
        // WHAT IS LIT IS EXACTLY WHAT THE CLICK WILL TAKE. There used to be a parsing of the anchor mode of
        // its own here, and it diverged from the click's parsing the moment the kind of anchor started being
        // chosen under the cursor: a person saw a face lit up and a corner was taken. There is one door -
        // `infer_mate_anchor`.
        if let Some(pos) = painter.ctx().pointer_hover_pos() {
            if rect.contains(pos) {
                // THE DOOR IS CHOSEN BY THE TOOL: specifying an axis takes an edge, specifying an anchor
                // takes the nearest snap point. Lighting one thing and taking another is a lie.
                let under = if self.joint.axis_pick.is_some() { self.infer_axis_anchor(rect, pos) } else { self.infer_mate_anchor(rect, pos) };
                match under {
                    Some((body, AnchorRef::FaceCenter(_, key))) if in_ctx(body) => hl(body, key.index as usize, blue),
                    Some((body, AnchorRef::EdgeMid(_, eid))) if in_ctx(body) => hl_edge(body, eid, blue),
                    Some((body, AnchorRef::Vertex(_, eid, end))) if in_ctx(body) => hl_vert(body, eid, end, blue),
                    _ => {}
                }
            }
        }
    }


    /// The mate glyphs in the 3D view: a line A <-> B (what is joined to what) + a badge with the kind's icon
    /// IN THE MIDDLE; the selected one (Sel::Joint) and the one under the cursor (hover_joint) are lit, which
    /// ties the view to the list.
    pub(super) fn draw_joints(&self, painter: &egui::Painter, rect: Rect) {
        if !self.mode_3d {
            return;
        }
        let basis = self.cam.basis();
        for j in &self.project.joints {
            if !self.joint_visible(j) {
                continue; // switched off by the checkbox, or a joint of another context (a nested subassembly) - not drawn
            }
            let Some((a, b)) = self.joint_endpoints(j, rect, &basis) else {
                continue;
            };
            let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let sel = self.sel == Sel::Joint(j.id);
            let hot = self.hover.joint == Some(j.id);
            let (r, col) = if sel {
                (10.0, self.scheme.pal.active())
            } else if hot {
                (9.0, self.scheme.pal.joint_hover())
            } else {
                (8.0, self.scheme.pal.joint_idle())
            };
            // the line between connectors A and B (it shows what is joined to what) + the end dots
            if a.distance(b) > 2.0 {
                let lc = crate::palette::a(col, if sel || hot { 210 } else { 95 });
                painter.line_segment([a, b], Stroke::new(if sel || hot { 1.8 } else { 1.0 }, lc));
                painter.circle_filled(a, 2.0, lc);
                painter.circle_filled(b, 2.0, lc);
            }
            painter.circle_filled(mid, r, crate::palette::a(self.scheme.pal.glyph_backing(), 225));
            painter.circle_stroke(mid, r, Stroke::new(if sel || hot { 2.0 } else { 1.4 }, col));
            paint_joint_glyph(painter, mid, r, j.kind, col);
        }
    }




    /// An anchor glyph at the grounded parts of the active assembly (like the mate glyphs - it shows what is
    /// fixed). It is drawn at the component's ORIGIN (its local zero in the context's frame), projected to the screen.
    pub(super) fn draw_grounded_glyphs(&self, painter: &egui::Painter, rect: Rect) {
        if !self.mode_3d || !self.set.show_joints || !matches!(self.workbench, Workbench::Assembly) {
            return;
        }
        let ctx = self.current_ctx_id();
        let basis = self.cam.basis();
        for &c in &self.project.component_children(ctx) {
            if !self.project.is_grounded(c) {
                continue;
            }
            let o = qymcad_core::feature::apply12(&self.project.relative_transform(c, ctx), [0.0, 0.0, 0.0]);
            let at = self.project3(o, rect, &basis).0;
            let col = self.scheme.pal.grounded();
            painter.circle_filled(at, 8.0, crate::palette::a(self.scheme.pal.glyph_backing(), 225));
            painter.circle_stroke(at, 8.0, Stroke::new(1.4, col));
            painter.text(at, egui::Align2::CENTER_CENTER, ph::ANCHOR, egui::FontId::proportional(11.0), col);
        }
    }


    /// Highlighting the plane candidates while a sketch plane is being chosen (translucent squares, brighter under the cursor).
    pub(super) fn draw_sketch_plane_picker(&self, painter: &egui::Painter, rect: Rect) {
        // active while choosing a sketch plane, while placing an import, in the MIRROR (16) / DATUM PLANE (20)
        // / SPLIT (27) command, while picking a MIRRORED PART and while picking a SECTION - one click-pick for all
        if !(self.picking.is_sketch_plane()
            || self.picking.replace_sketch().is_some()
            || self.pending_import.curves.is_some()
            || self.cmd.kind == 16
            || self.cmd.kind == 20
            || self.cmd.kind == 27
            || self.cmd.kind == 29
            || self.mirror.part.is_some()
            || self.section.pick)
            || !self.mode_3d
        {
            return;
        }
        let basis = self.cam.basis();
        // the plane ALREADY CHOSEN is lit PERMANENTLY (in blue), the candidate under the cursor in yellow.
        // That way both what is chosen (a FACE included) and what is under the cursor right now are visible -
        // before, a chosen face could not be seen at all.
        let picked = match self.cmd.kind {
            16 => self.mirror.plane.clone(),
            20 => self.datum.plane_pick.clone(),
            27 | 29 => self.split.plane.clone(),
            _ => None,
        };
        let hovered = painter.ctx().pointer_hover_pos().and_then(|p| self.pick_sketch_plane_at(rect, p));
        // An earlier edition drew ALL the candidate planes PERMANENTLY (blue frames). Reported behaviour:
        // squares that cannot be selected, and planes lighting up that have nothing to do with anything -
        // the wall of a cylinder among them. Both complaints have one cause: the painter overlay is drawn
        // WITHOUT a depth test against the 3D scene (the GPU and software rasters run EARLIER, the overlay is
        // 2D shapes ON TOP with no Z), while pick_sketch_plane_at gives priority to ANY body under the cursor
        // (even when the face itself was not recognised) - so a square COULD be drawn over or through a body
        // that actually intercepts the click (and cannot be selected), and could SHINE THROUGH curved faces
        // (a cylinder) the plane does not touch at all. It was removed: what is shown is ONLY what is really
        // under the cursor now - which is also what will be selected.
        // first the chosen one (blue), then - if it differs - the one under the cursor (yellow) on top
        if let Some(sp) = &picked {
            if hovered.as_ref() != Some(sp) {
                self.draw_pick_plane(painter, rect, &basis, sp, self.scheme.pal.plane_face());
            }
        }
        if let Some(sp) = &hovered {
            self.draw_pick_plane(painter, rect, &basis, sp, self.scheme.pal.highlight());
            // the origin snap marker - only for a NEW sketch (not for a mirror, a datum or an import)
            if self.picking.is_sketch_plane() {
                if let Some(pos) = painter.ctx().pointer_hover_pos() {
                    if let Some((uv, fr)) = self.sketch_origin_snap(rect, pos, sp).zip(self.world_frame_of_plane(sp)) {
                        let w = fr.lift(uv);
                        let s = self.project3([w.x, w.y, w.z], rect, &basis).0;
                        painter.circle_filled(s, 4.0, self.scheme.pal.snap_point());
                        painter.circle_stroke(s, 6.5, egui::Stroke::new(1.5, self.scheme.pal.emphasis()));
                    }
                }
            }
        }
    }


    /// Draw ONE plane or face of the click-pick in the given colour: a world or datum plane as a square frame
    /// (+/-60 in its frame); a part's face as a fill of its triangles (as one mesh, with no "needles" from the
    /// fan triangulation).
    pub(super) fn draw_pick_plane(&self, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), sp: &qymcad_core::feature::SketchPlane, col: Color32) {
        use qymcad_core::feature::SketchPlane;
        if let SketchPlane::Face(body, key) = sp {
            if let Some(mi) = self.project.mesh_index(*body) {
                if let Some(face) = self.project.bodies.get(mi).and_then(|b| b.faces.get(key.index as usize)) {
                    let mesh = &self.project.bodies[mi].mesh;
                    let wt = self.project.body_display_transform(*body, self.current_ctx_id());
                    let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
                    let fill = crate::palette::a(col, 90);
                    let mut hm = egui::Mesh::default();
                    for &tri in &face.triangles {
                        let t = mesh.triangle(tri as usize);
                        let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                        // BACK-FACING triangles: a face can WRAP AROUND (on a cylinder the whole lateral
                        // surface is one B-rep face), so half of its triangles look AWAY from the camera.
                        // Without culling, the fill is drawn there as well (the painter has no Z test against
                        // the scene) and "shines through" the body - reported as the inner faces of cylinders
                        // lighting up. The same backface-cull formula as in rasterize_3d, including the ray
                        // FROM THE EYE in perspective (a shared `fwd` lies towards the edges of the frame, see
                        // `view_dir_at`).
                        let n = v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa)));
                        if v_dot(n, self.view_dir_at(wa, basis.2, self.persp_inv_d_eye(rect.height() * 0.5))) >= 0.0 {
                            continue;
                        }
                        let b = hm.vertices.len() as u32;
                        for w in [wa, wb, wc] {
                            hm.colored_vertex(self.project3(w, rect, basis).0, fill);
                        }
                        hm.add_triangle(b, b + 1, b + 2);
                    }
                    if !hm.is_empty() {
                        painter.add(egui::Shape::mesh(hm));
                    }
                }
            }
            return;
        }
        // a world or datum plane: a square frame from the candidate's frame (scaled by the scene, see pick)
        let h = self.plane_pick_half_size();
        for (cand, fr) in self.sketch_plane_candidates() {
            if &cand != sp {
                continue;
            }
            let corners = [fr.lift(Point2::new(-h, -h)), fr.lift(Point2::new(h, -h)), fr.lift(Point2::new(h, h)), fr.lift(Point2::new(-h, h))];
            let poly: Vec<Pos2> = corners.iter().map(|p| self.project3([p.x, p.y, p.z], rect, basis).0).collect();
            let fill = crate::palette::a(col, 70);
            painter.add(egui::Shape::convex_polygon(poly, fill, Stroke::new(2.0, col)));
        }
    }


    /// The GPU pass over the bodies: it pushes a paint callback into the viewport rect. The vertices are
    /// re-uploaded only when `gpu_scene_key` changes; while orbiting, only the camera uniform is updated.
    pub(super) fn draw_3d_gpu(&self, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        let ppp = painter.ctx().pixels_per_point();
        let key = self.gpu_scene_key();
        let (inv_d, z_near, z_far, _) = self.proj_params(rect, key);
        let cam = crate::viewport_gpu::CamRaw::new(basis, self.cam.scale, self.cam.target, rect.width(), rect.height(), inv_d as f32, z_near as f32, z_far as f32);
        let size_px = [(rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32];
        let (verts, opaque_count) = if self.cache.gpu_scene_key.get() != key {
            self.cache.gpu_scene_key.set(key);
            let (v, oc) = self.gpu_scene();
            (Some(std::sync::Arc::new(v)), oc)
        } else {
            (None, 0)
        };
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(rect, crate::viewport_gpu::MeshPaint::new(cam, size_px, verts, opaque_count, key)));
    }


    /// Software rasterisation of the visible bodies into RGBA with a Z buffer. The buffer holds the
    /// screen-linear `ndc_z` (`depth_ndc`), so the linear interpolation in `raster_band` is exact both in
    /// orthographic and in perspective. The background is transparent.
    pub(super) fn rasterize_3d(&self, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), ppp: f32, quality: f32) -> Option<egui::ColorImage> {
        let w = (rect.width() * ppp * quality).round() as usize;
        let h = (rect.height() * ppp * quality).round() as usize;
        if w == 0 || h == 0 || w.saturating_mul(h) > 16_000_000 {
            return None;
        }
        let light = v_norm([0.35, 0.5, 0.78]);
        let (_, _, fwd) = basis;
        let mut color = vec![Color32::TRANSPARENT; w * h];
        let mut zbuf = vec![f32::INFINITY; w * h];

        let smooth = self.set.smooth_shading;
        if smooth {
            self.ensure_vertex_normals();
        }
        let ncache = self.cache.norm.borrow();
        let items = self.visible_mesh_items();
        // the depth parameters for this frame (in perspective: tight near/far from the scene's extent)
        let (inv_d, z_near, z_far, depth_half) = self.proj_params(rect, self.gpu_scene_key());

        // 1) project the visible front-facing triangles into pixel coordinates. The colour is PER VERTEX
        // (Gouraud): from the smoothed normal, interpolated in the raster; in flat mode the face normal goes
        // into all three.
        let (ox, oy) = (rect.min.x, rect.min.y);
        let pscale = ppp * quality;
        // the opaque ones (first pass, z-write) and the translucent ghosts (second pass, blend, no z-write)
        let mut tris: Vec<([f32; 3], [f32; 3], [f32; 3], [Color32; 3])> = Vec::new();
        let mut ghost_tris: Vec<([f32; 3], [f32; 3], [f32; 3], [Color32; 3])> = Vec::new();
        for (mi, hot, ghost, base, mesh, wt) in items {
            let ident = qymcad_core::feature::is_identity12(&wt);
            let vn = if smooth { ncache.1.get(mi) } else { None };
            for i in 0..mesh.tris.len() {
                let tri_idx = mesh.tris[i];
                let pw = |vi: u32| {
                    let p = mesh.verts[vi as usize];
                    let a = [p.x, p.y, p.z];
                    if ident { a } else { qymcad_core::feature::apply12(&wt, a) }
                };
                let (a, b, c) = (pw(tri_idx[0]), pw(tri_idx[1]), pw(tri_idx[2]));
                let n = v_norm(v_cross(v_sub(b, a), v_sub(c, a)));
                // CULLING GOES BY THE RAY FROM THE EYE (in orthographic that is `fwd`): in perspective a
                // shared `fwd` lies towards the edges of the frame, the more so the wider the field of view
                // (gaps in a body, ribbons instead of a ring).
                if v_dot(n, self.view_dir_at(a, *fwd, inv_d)) >= 0.0 {
                    continue; // the bodies are oriented outwards
                }
                let col_at = |vi: u32| -> Color32 {
                    let nrm = match vn {
                        Some(list) => Self::rotate_normal(&wt, list[vi as usize]),
                        None => n,
                    };
                    Self::shade_tri(&self.scheme.pal, self.set.ghost_alpha, hot, ghost, base, nrm, light)
                };
                let cols = [col_at(tri_idx[0]), col_at(tri_idx[1]), col_at(tri_idx[2])];
                let (pa, da) = self.project3(a, rect, basis);
                let (pb, db) = self.project3(b, rect, basis);
                let (pc, dc) = self.project3(c, rect, basis);
                // the z buffer takes the screen-linear ndc_z (perspective-correct), not the world depth
                let tri = (
                    [(pa.x - ox) * pscale, (pa.y - oy) * pscale, self.depth_ndc(da, inv_d, z_near, z_far, depth_half)],
                    [(pb.x - ox) * pscale, (pb.y - oy) * pscale, self.depth_ndc(db, inv_d, z_near, z_far, depth_half)],
                    [(pc.x - ox) * pscale, (pc.y - oy) * pscale, self.depth_ndc(dc, inv_d, z_near, z_far, depth_half)],
                    cols,
                );
                if ghost { ghost_tris.push(tri) } else { tris.push(tri) }
            }
        }

        // 2) rasterisation with a Z buffer, in parallel over horizontal bands
        let nthreads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
        if nthreads <= 1 || h < 64 {
            raster_band(&mut color, &mut zbuf, w, 0, h, &tris);
            raster_band_blend(&mut color, &zbuf, w, 0, h, &ghost_tris);
        } else {
            let band = h.div_ceil(nthreads);
            std::thread::scope(|s| {
                let (mut crest, mut zrest) = (&mut color[..], &mut zbuf[..]);
                let mut y0 = 0;
                let tref = &tris;
                let gref = &ghost_tris;
                while y0 < h {
                    let rows = band.min(h - y0);
                    let (cb, c2) = crest.split_at_mut(rows * w);
                    let (zb, z2) = zrest.split_at_mut(rows * w);
                    crest = c2;
                    zrest = z2;
                    let y = y0;
                    // within a band: the opaque ones first (they fill z), then the ghosts on top (z test, blend)
                    s.spawn(move || {
                        raster_band(cb, zb, w, y, rows, tref);
                        raster_band_blend(cb, zb, w, y, rows, gref);
                    });
                    y0 += rows;
                }
            });
        }
        Some(egui::ColorImage { size: [w, h], source_size: egui::Vec2::new([w, h][0] as f32, [w, h][1] as f32), pixels: color })
    }


    pub(super) fn draw_3d(&self, painter: &egui::Painter, rect: Rect) {
        let basis = self.cam.basis();
        let p3 = |p: [f64; 3]| self.project3(p, rect, &basis).0;

        // the table at Z=0 is a machining visual: only in the CAM workbench, not in the CAD view
        if self.cam_mode {
            let m = &self.project.machine;
            let tc = [
                [m.work_min[0], m.work_min[1], 0.0],
                [m.work_max[0], m.work_min[1], 0.0],
                [m.work_max[0], m.work_max[1], 0.0],
                [m.work_min[0], m.work_max[1], 0.0],
            ];
            let tbl = Stroke::new(1.0, self.scheme.pal.cam_table_grid());
            for i in 0..4 {
                painter.line_segment([p3(tc[i]), p3(tc[(i + 1) % 4])], tbl);
            }
        }

        // THE FLOOR GRID at Z=0 is the bearing in the 3D view (NOT the machine table: that one sits under
        // cam_mode). A minor line every `step`, a major one every 5; drawn UNDER the geometry (before the
        // raster mesh), plus the coloured global axes. The grid is CENTRED on the look-at point (cam.target)
        // rather than on the world origin, and the number of lines is computed from the screen size. Otherwise
        // on large projects (a machine over 1 m, coordinates far from the origin) the machine ended up at the
        // faded edge of the grid or beyond it, and looked like it was hanging in the air.
        {
            // an adaptive step for the zoom: the on-screen interval of a line is at least ~9px (as in the sketch grid), a major line every 5
            let sc = self.cam.scale as f64;
            let mut step = 10.0_f64;
            while step * sc < 9.0 {
                step *= 5.0;
            }
            // A smooth LOD: near the switching threshold (step x 5) the minor lines become ~9px and crowd
            // together, then vanish abruptly when the step jumps. The minor lines are faded as the interval
            // approaches the threshold (9 -> 18px: 0 -> 1) while the major ones (every 5) stay solid, so the
            // scale changes without a jerk.
            let px = step * sc; // the on-screen interval of a minor line, in [9, 45)
            let minor_lod = (((px - 9.0) / 9.0).clamp(0.0, 1.0)) as f32;
            // the grid centre is the look-at point projected onto the Z=0 floor and snapped to the step (the lines stay put while panning)
            let cx = (self.cam.target[0] / step).round() * step;
            let cy = (self.cam.target[1] / step).round() * step;
            // the number of lines each way: cover half the screen diagonal (orbiting tilts the floor) plus a
            // margin, with a ceiling so that drawing does not blow up when zoomed far out
            let reach_px = (rect.width().hypot(rect.height()) * 0.5) as f64;
            let n = (((reach_px / (step * sc)).ceil() as i32) + 2).clamp(8, 160);
            let lim = step * n as f64;
            for i in -n..=n {
                let tx = cx + i as f64 * step;
                let ty = cy + i as f64 * step;
                // a fade towards the edges of the grid (a soft vignette, so the lines do not break off hard)
                let fade = (1.0 - (i.abs() as f32 / n as f32).powi(2)).clamp(0.0, 1.0);
                // a major line goes by an absolute coordinate that is a multiple of 5 * step (so the fives stay put while panning)
                let major_x = ((tx / step).round() as i64).rem_euclid(5) == 0;
                let major_y = ((ty / step).round() as i64).rem_euclid(5) == 0;
                let stroke = |major: bool| {
                    let (base, a) = if major { (self.scheme.pal.grid(), 230.0) } else { (self.scheme.pal.grid_minor(), 170.0) };
                    // the minor lines fade further by the LOD near the step's switching threshold
                    let lod = if major { 1.0 } else { minor_lod };
                    Stroke::new(1.0, crate::palette::a(base, (a * fade * lod) as u8))
                };
                // a line along Y (at x=tx) and one along X (at y=ty), stretched over the grid's reach around
                // the centre. The grey line at zero is not drawn - a coloured axis runs there instead (green Y
                // at x=0, red X at y=0).
                if tx.abs() > step * 0.5 {
                    painter.line_segment([p3([tx, cy - lim, 0.0]), p3([tx, cy + lim, 0.0])], stroke(major_x));
                }
                if ty.abs() > step * 0.5 {
                    painter.line_segment([p3([cx - lim, ty, 0.0]), p3([cx + lim, ty, 0.0])], stroke(major_y));
                }
            }
            // the coloured global axes: X red, Y green (on the floor), Z blue upwards; stretched across the
            // grid's current reach around the look-at point (visible for as long as the origin is in view).
            painter.line_segment([p3([cx - lim, 0.0, 0.0]), p3([cx + lim, 0.0, 0.0])], Stroke::new(1.6, self.scheme.pal.grid_axis_x()));
            painter.line_segment([p3([0.0, cy - lim, 0.0]), p3([0.0, cy + lim, 0.0])], Stroke::new(1.6, self.scheme.pal.grid_axis_y()));
            painter.line_segment([p3([0.0, 0.0, 0.0]), p3([0.0, 0.0, step * 4.0])], Stroke::new(1.8, self.scheme.pal.grid_axis_z()));
        }

        // the stock is a wireframe box (translucent edges) - a machining item, only in the CAM workbench
        if self.cam_mode {
            if let Some((mn, mx)) = self.effective_stock() {
                let v = |x: f64, y: f64, z: f64| p3([x, y, z]);
                let c = [
                    v(mn[0], mn[1], mn[2]), v(mx[0], mn[1], mn[2]), v(mx[0], mx[1], mn[2]), v(mn[0], mx[1], mn[2]),
                    v(mn[0], mn[1], mx[2]), v(mx[0], mn[1], mx[2]), v(mx[0], mx[1], mx[2]), v(mn[0], mx[1], mx[2]),
                ];
                let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
                let sel = self.sel == Sel::Stock;
                let st = Stroke::new(if sel { 1.8 } else { 1.0 }, if sel { self.scheme.pal.cam_stock() } else { self.scheme.pal.cam_stock_idle() });
                for (a, b) in edges {
                    painter.line_segment([c[a], c[b]], st);
                }
            }
        }

        // the mesh: either the GPU pass (wgpu, a depth buffer) or the CPU fallback (software rasterisation
        // with a Z buffer - an exact per-pixel order, with none of the painter's-algorithm artefacts on
        // coplanar faces).
        if self.gpu_ok && self.set.gpu_viewport {
            // a paint callback into the rect, UNDER the 2D overlays. The CPU cache is not involved.
            self.draw_3d_gpu(painter, rect, &basis);
        } else {
            // The texture is cached by the view key and redrawn only when that changes.
            let ppp = painter.ctx().pixels_per_point();
            let quality = if self.view_dragging { 0.5 } else { 1.0 };
            let key = self.view_key(rect, ppp);
            let cached = self.cache.view.borrow().as_ref().map(|(k, _)| *k) == Some(key);
            if !cached {
                if let Some(img) = self.rasterize_3d(rect, &basis, ppp, quality) {
                    let filter = if quality < 1.0 { egui::TextureOptions::LINEAR } else { egui::TextureOptions::NEAREST };
                    let tex = painter.ctx().load_texture("qym_view3d", img, filter);
                    *self.cache.view.borrow_mut() = Some((key, tex));
                } else {
                    *self.cache.view.borrow_mut() = None;
                }
            }
            if let Some((_, tex)) = self.cache.view.borrow().as_ref() {
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(tex.id(), rect, uv, Color32::WHITE);
            }
        }

        // the sketch outlines go OVER the bodies (an overlay), so that they can be seen at all.
        // In an assembly they can be switched off with a single toggle; that toggle does not hide the sketch
        // being edited or picked as a command's profile - what is invisible cannot be selected.
        let forced_cids = self.active_sketch_contour_ids();
        if forced_cids.is_some() || !self.contours_switched_off() {
            let selected: &[Id] = self
                .active_op()
                .and_then(|i| self.project.operations.get(i))
                .map(|op| op.selection.as_slice())
                .unwrap_or(&[]);
            let obj_sel = if let Sel::Contour(c) = self.sel { Some(c) } else { None };
            let sketch_sel = if let Sel::Sketch(s) = self.sel { self.project.sketches.get(s).map(|sk| sk.contour_ids.clone()) } else { None };
            // hidden sketches (the checkbox is off) are not drawn - the same computation as in the sketcher
            let hidden_cids = self.hidden_contour_ids();
            // the sketches of OTHER components (outside the active context) are not drawn - sketch isolation
            let foreign_cids = self.foreign_contour_ids();
            // the outlines of sketches on NON-world planes are drawn lifted onto their own plane
            let mut cframe: std::collections::HashMap<usize, qymcad_core::feature::PlaneFrame> = std::collections::HashMap::new();
            for si in 0..self.project.sketches.len() {
                if let Some(f) = self.project.sketch_frame(si) {
                    if !f.is_identity() {
                        for &cid in &self.project.sketches[si].contour_ids {
                            if let Some(ci) = self.project.contour_index(cid) {
                                cframe.insert(ci, f);
                            }
                        }
                    }
                }
            }
            for (ci, c) in self.project.contours.iter().enumerate() {
                if c.points.len() < 2 {
                    continue;
                }
                let cid = self.project.contour_id(ci);
                if cid.is_some_and(|id| hidden_cids.contains(&id)) {
                    continue; // the sketch is hidden by its checkbox
                }
                if cid.is_some_and(|id| foreign_cids.contains(&id)) {
                    continue; // a sketch of another component - outside the active context
                }
                if self.contours_switched_off() && !forced_cids.as_ref().is_some_and(|only| cid.is_some_and(|id| only.contains(&id))) {
                    continue; // the assembly toggle is off: only the sketch currently in hand is visible
                }
                // a selection in the tree (a contour or a sketch) is bright green; one in an operation is yellow
                let in_sketch = sketch_sel.as_ref().is_some_and(|ids| cid.is_some_and(|id| ids.contains(&id)));
                let (w, col) = if obj_sel == Some(ci) || in_sketch {
                    (2.5, self.scheme.pal.ok())
                } else if cid.is_some_and(|id| selected.contains(&id)) {
                    (2.0, self.scheme.pal.active())
                } else {
                    (1.5, self.scheme.pal.sketch_edge_3d())
                };
                let st = Stroke::new(w, col);
                let n = c.points.len();
                let last = if c.closed { n } else { n - 1 };
                let lift = |q: Point2| -> [f64; 3] {
                    match cframe.get(&ci) {
                        Some(f) => {
                            let w = f.lift(q);
                            [w.x, w.y, w.z]
                        }
                        None => [q.x, q.y, 0.0],
                    }
                };
                for k in 0..last {
                    let a = c.points[k];
                    let b = c.points[(k + 1) % n];
                    painter.line_segment([p3(lift(a)), p3(lift(b))], st);
                }
            }
        }

        // the edges of the selected body (for picking under a chamfer or a fillet)
        self.draw_body_edges(painter, rect);
        // the live wireframe preview of the active command (extrude, cut, ...) + the length arrow.
        // There is no permanent gizmo at the selected feature - editing goes through a double click in the tree.
        self.draw_feat_cmd_preview(painter, rect);
        self.draw_sketch_plane_picker(painter, rect);
        self.draw_joint_pick_highlight(painter, rect);
        self.draw_component_gizmo(painter, rect);
        self.draw_body_gizmo(painter, rect); // the body gizmo inside a Part
        self.draw_section_gizmo(painter, rect); // the section plane + the offset arrow
        self.draw_joints(painter, rect); // the mate glyphs
        self.draw_grounded_glyphs(painter, rect); // the anchor at the grounded parts

        // face highlighting happens ONLY under the Shell/Hole command (outside them a face is not pickable).
        // Shell (6) lights EVERY face of the multi-selection by persistent id; Hole (7) lights one Sel::Face.
        let fill_face = |painter: &egui::Painter, face: &qymcad_core::geom::MeshFace, mesh: &qymcad_core::geom::Mesh| {
            let mut hm = egui::Mesh::default();
            for &ti in &face.triangles {
                let t = mesh.triangle(ti as usize);
                let base = hm.vertices.len() as u32;
                for v in &t {
                    hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, self.scheme.pal.selected());
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        };
        if self.cmd.kind == 6 {
            for (mi, faces) in self.project.bodies.iter().map(|b| &b.faces).enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue; // only the target body's faces are lit (the ids are local to a body)
                }
                if let Some(mesh) = self.project.bodies.get(mi).map(|b| &b.mesh) {
                    for face in faces.iter().filter(|f| self.gsel.faces.contains(&f.id)) {
                        fill_face(painter, face, mesh);
                    }
                }
            }
        } else if self.cmd.kind == 7 {
            if self.hole.mode == 1 {
                // the "from a sketch" mode: diameter circles at every isolated marker point of the sketch
                if let Some(sid) = self.hole.sketch {
                    let dia = self.cmd_val("diameter").max(0.1);
                    let col = self.scheme.pal.preview();
                    for at in self.project.sketch_isolated_points(sid) {
                        let sc = p3(at);
                        // a cross + a circle of radius diameter/2 (roughly, in screen scale via an offset point)
                        painter.line_segment([sc + egui::vec2(-6.0, 0.0), sc + egui::vec2(6.0, 0.0)], Stroke::new(1.5, col));
                        painter.line_segment([sc + egui::vec2(0.0, -6.0), sc + egui::vec2(0.0, 6.0)], Stroke::new(1.5, col));
                        // the radius on screen: project a point offset along the sketch's X
                        let rp = (self.project3([at[0] + dia * 0.5, at[1], at[2]], rect, &basis).0 - sc).length();
                        painter.circle_stroke(sc, rp.max(2.0), Stroke::new(1.5, col));
                    }
                }
            } else if let Sel::Face(mi, fi) = self.sel {
                if let (Some(face), Some(mesh)) = (self.project.bodies.get(mi).and_then(|b| b.faces.get(fi)), self.project.bodies.get(mi).map(|b| &b.mesh)) {
                    fill_face(painter, face, mesh);
                }
            }
        } else if self.cmd.kind == 28 {
            // THICKEN: the selected face is lit + a PREVIEW of the plate - the face outline shifted by the
            // thickness. The sign is visible at once: whether the material goes outwards or inwards.
            let t = self.cmd_val("thickness");
            for (mi, body) in self.project.bodies.iter().enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue;
                }
                let Some(face) = body.faces.iter().find(|f| self.gsel.faces.contains(&f.id)) else { continue };
                let mesh = &body.mesh;
                let mut hm = egui::Mesh::default();
                for &ti in &face.triangles {
                    let tri = mesh.triangle(ti as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &tri {
                        hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(self.scheme.pal.add(), 110));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
                // THE OFFSET OUTLINE - where the plate's second surface will go
                let n = face.normal;
                let off = |p: [f64; 3]| [p[0] + n[0] * t, p[1] + n[1] * t, p[2] + n[2] * t];
                let st = Stroke::new(1.6, self.scheme.pal.add());
                for &ti in &face.triangles {
                    let tri = mesh.triangle(ti as usize);
                    for k in 0..3 {
                        let (a, b) = (tri[k], tri[(k + 1) % 3]);
                        let (pa, pb) = (off([a.x, a.y, a.z]), off([b.x, b.y, b.z]));
                        painter.line_segment([self.project3(pa, rect, &basis).0, self.project3(pb, rect, &basis).0], st);
                    }
                }
            }
        } else if matches!(self.cmd.kind, 30 | 31) {
            // COPY FACE (30) and REPLACE FACE (31): show the selection TO THE EYE, not only in the model.
            //
            // The first edition gathered faces silently - nothing changed on the screen, and that read, fairly,
            // as "no face gets selected at all". A tool without a preview is blind: a person sees neither what
            // they picked nor what will come of it.
            //
            // The colour carries the MEANING: for a copy it is "will be added" (a surface appears), for a
            // replacement "will go" (a sheet takes these faces' place). The sheet surface itself is lit separately.
            let taken = if self.cmd.kind == 30 { self.scheme.pal.add() } else { self.scheme.pal.remove() };
            for (mi, body) in self.project.bodies.iter().enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue;
                }
                let mesh = &body.mesh;
                let mut hm = egui::Mesh::default();
                for face in body.faces.iter().filter(|f| self.gsel.faces.contains(&f.id)) {
                    for &ti in &face.triangles {
                        let t = mesh.triangle(ti as usize);
                        let base = hm.vertices.len() as u32;
                        for v in &t {
                            hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(taken, 120));
                        }
                        hm.add_triangle(base, base + 1, base + 2);
                    }
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
            // THE SELECTED SURFACE (31) whole, so that it is visible WHAT exactly will return to the body
            if let Some(surf) = self.repl_surface {
                if let Some(mi) = self.project.mesh_index(surf) {
                    let mesh = &self.project.bodies[mi].mesh;
                    let mut hm = egui::Mesh::default();
                    for ti in 0..mesh.tris.len() {
                        let t = mesh.triangle(ti);
                        let base = hm.vertices.len() as u32;
                        for v in &t {
                            hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(self.scheme.pal.add(), 130));
                        }
                        hm.add_triangle(base, base + 1, base + 2);
                    }
                    if !hm.is_empty() {
                        painter.add(egui::Shape::mesh(hm));
                    }
                }
            }
        } else if self.cmd.kind == 34 {
            // TRIM: the sheet being kept in the "stays" colour, the tool in the "goes" colour.
            for (body, add) in [(self.trim.keep.map(|(b, _)| b), true), (self.trim.tool, false)] {
                let Some(mi) = body.and_then(|b| self.project.mesh_index(b)) else { continue };
                let mesh = &self.project.bodies[mi].mesh;
                let mut hm = egui::Mesh::default();
                for ti in 0..mesh.tris.len() {
                    let t = mesh.triangle(ti);
                    let base = hm.vertices.len() as u32;
                    let col = if add { self.scheme.pal.add() } else { self.scheme.pal.remove() };
                    for v in &t {
                        hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(col, 120));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
            // and the SPOT that was clicked: it is what decides which piece stays
            if let Some((_, at)) = self.trim.keep {
                let p = self.project3(at, rect, &basis).0;
                painter.circle_filled(p, 5.0, self.scheme.pal.add());
            }
        } else if self.cmd.kind == 33 {
            // STITCH: the selected sheets filled in the "will be added" colour. A person must see WHAT
            // exactly will become one surface: clicking blind into an invisible set is not acceptable.
            for part in &self.stitch_parts {
                let Some(mi) = self.project.mesh_index(*part) else { continue };
                let mesh = &self.project.bodies[mi].mesh;
                let mut hm = egui::Mesh::default();
                for ti in 0..mesh.tris.len() {
                    let t = mesh.triangle(ti);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(self.scheme.pal.add(), 130));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
        } else if self.cmd.kind == 26 {
            // DELETE FACE: the selected faces filled in red (what will cease to exist).
            for (mi, body) in self.project.bodies.iter().enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue;
                }
                let mesh = &body.mesh;
                let mut hm = egui::Mesh::default();
                for face in body.faces.iter().filter(|f| self.gsel.faces.contains(&f.id)) {
                    for &ti in &face.triangles {
                        let t = mesh.triangle(ti as usize);
                        let base = hm.vertices.len() as u32;
                        for v in &t {
                            hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(self.scheme.pal.remove(), 120));
                        }
                        hm.add_triangle(base, base + 1, base + 2);
                    }
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
        } else if self.cmd.kind == 25 {
            // PUSH FACE: the selected face is lit + a PREVIEW of the result - the face outline shifted by the
            // given offset, with the edges of the future prism between them. Without this the tool was blind:
            // neither what was picked nor where it would move was clear.
            let dist = self.cmd_val("dist");
            for (mi, body) in self.project.bodies.iter().enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue;
                }
                let Some(face) = body.faces.iter().find(|f| self.gsel.faces.contains(&f.id)) else { continue };
                let mesh = &body.mesh;
                // the face itself, filled in orange (as for the shell and the draft)
                let mut hm = egui::Mesh::default();
                for &ti in &face.triangles {
                    let t = mesh.triangle(ti as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, crate::palette::a(self.scheme.pal.modify(), 110));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
                // THE PREVIEW: the same face shifted along the normal by dist, dashed, plus posts at the corners
                if dist.abs() > 1e-9 {
                    let n = face.normal;
                    let off = |v: [f64; 3]| [v[0] + n[0] * dist, v[1] + n[1] * dist, v[2] + n[2] * dist];
                    let col = if dist > 0.0 { self.scheme.pal.ok() } else { self.scheme.pal.offset_in() };
                    for &ti in &face.triangles {
                        let t = mesh.triangle(ti as usize);
                        let p: Vec<Pos2> = t.iter().map(|v| self.project3(off([v.x, v.y, v.z]), rect, &basis).0).collect();
                        for k in 0..3 {
                            painter.add(egui::Shape::dashed_line(&[p[k], p[(k + 1) % 3]], Stroke::new(1.2, col), 5.0, 4.0));
                        }
                    }
                    // the direction: an arrow from the face centre to the shifted centre
                    let c = [face.centroid.x, face.centroid.y, face.centroid.z];
                    let (a, b) = (self.project3(c, rect, &basis).0, self.project3(off(c), rect, &basis).0);
                    painter.add(egui::Shape::line_segment([a, b], Stroke::new(2.0, col)));
                    painter.circle_filled(b, 3.5, col);
                }
            }
        } else if self.cmd.kind == 23 {
            // Draft: the faces being tilted in orange (as for the shell), the neutral face in blue.
            let fill_col = |painter: &egui::Painter, face: &qymcad_core::geom::MeshFace, mesh: &qymcad_core::geom::Mesh, col: Color32| {
                let mut hm = egui::Mesh::default();
                for &ti in &face.triangles {
                    let t = mesh.triangle(ti as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(self.project3([v.x, v.y, v.z], rect, &basis).0, col);
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            };
            for (mi, faces) in self.project.bodies.iter().map(|b| &b.faces).enumerate() {
                if self.project.mesh_id(mi) != self.gsel.faces_body {
                    continue; // the draft and neutral faces come ONLY from the target body (the ids are local to a body)
                }
                if let Some(mesh) = self.project.bodies.get(mi).map(|b| &b.mesh) {
                    for face in faces.iter() {
                        if self.draft.neutral != 0 && face.id == self.draft.neutral {
                            fill_col(painter, face, mesh, self.scheme.pal.reference());
                        } else if self.gsel.faces.contains(&face.id) {
                            fill_col(painter, face, mesh, self.scheme.pal.selected());
                        }
                    }
                }
            }
        }
        // THE COMMAND HANDLE - one for every tool that has a direction and a distance along it (push face,
        // thicken, shell, both splits). It is drawn AFTER the preview branches so that no call has to be
        // added inside each of them: those would drift apart, the way the popups once did.
        self.draw_face_arrow(painter, rect, &basis);
        self.draw_fillet_vertices(painter, rect); // the points a per-vertex radius is set at
        // the wireframe preview of the primitive being created
        self.draw_prim_preview(painter, rect);
        self.draw_array_preview(painter, rect);
        self.draw_comp_array_preview(painter, rect);
        self.draw_measure_3d(painter, rect);
        self.draw_mirror_preview(painter, rect);
        self.draw_split_preview(painter, rect);
        self.draw_axis_picker(painter, rect);
        self.draw_datum_preview(painter, rect);

        // the work planes
        for (pi, pl) in self.project.planes.iter().enumerate() {
            // as for points and axes: a datum plane is drawn ONLY when it is visible in the context and not
            // hidden by its checkbox (datum_render_transform returns None otherwise). The origin and normal
            // live in the owner's frame and are carried into the view.
            let Some(wt) = self.datum_render_transform(pl.id) else { continue };
            let ident = qymcad_core::feature::is_identity12(&wt);
            let o = if ident { pl.origin } else { qymcad_core::feature::apply12(&wt, pl.origin) };
            let n = v_norm(if ident { pl.normal } else { qymcad_core::feature::apply12_dir(&wt, pl.normal) });
            let ax = if n[0].abs() < 0.9 { v_norm(v_cross(n, [1.0, 0.0, 0.0])) } else { v_norm(v_cross(n, [0.0, 1.0, 0.0])) };
            let ay = v_cross(n, ax);
            let s = 25.0;
            let corner = |sx: f64, sy: f64| [o[0] + ax[0] * sx + ay[0] * sy, o[1] + ax[1] * sx + ay[1] * sy, o[2] + ax[2] * sx + ay[2] * sy];
            let c = [corner(-s, -s), corner(s, -s), corner(s, s), corner(-s, s)];
            let sel = self.sel == Sel::Plane(pi);
            let col = if sel { self.scheme.pal.highlight() } else { self.scheme.pal.plane_idle() };
            let st = Stroke::new(if sel { 2.0 } else { 1.0 }, col);
            for k in 0..4 {
                painter.line_segment([p3(c[k]), p3(c[(k + 1) % 4])], st);
            }
            // the normal
            let tip = [o[0] + n[0] * 20.0, o[1] + n[1] * 20.0, o[2] + n[2] * 20.0];
            painter.line_segment([p3(o), p3(tip)], Stroke::new(1.5, self.scheme.pal.plane_normal()));
        }

        // datum POINTS (a cross marker) and datum AXES (a segment) are drawn in 3D, the selected one brighter.
        // Datums travel with their own part in an assembly (the owner's transform), and isolation hides the
        // components of others.
        let sel_col = self.scheme.pal.highlight();
        let dap = |wt: &[f64; 12], v: [f64; 3]| if qymcad_core::feature::is_identity12(wt) { v } else { qymcad_core::feature::apply12(wt, v) };
        for (i, dp) in self.project.datum_points.iter().enumerate() {
            let Some(wt) = self.datum_render_transform(dp.id) else { continue };
            let s = p3(dap(&wt, dp.at));
            let sel = self.sel == Sel::DatumPoint(i);
            let col = if sel { sel_col } else { self.scheme.pal.datum_point() };
            let r = if sel { 6.0 } else { 4.5 };
            painter.line_segment([s + egui::vec2(-r, 0.0), s + egui::vec2(r, 0.0)], Stroke::new(1.6, col));
            painter.line_segment([s + egui::vec2(0.0, -r), s + egui::vec2(0.0, r)], Stroke::new(1.6, col));
            painter.circle_stroke(s, r * 0.7, Stroke::new(1.0, col));
        }
        for (i, da) in self.project.datum_axes.iter().enumerate() {
            let Some(wt) = self.datum_render_transform(da.id) else { continue };
            let o = dap(&wt, da.origin());
            let d = v_norm(if qymcad_core::feature::is_identity12(&wt) { da.dir() } else { qymcad_core::feature::apply12_dir(&wt, da.dir()) });
            let l = 45.0;
            let a = [o[0] - d[0] * l, o[1] - d[1] * l, o[2] - d[2] * l];
            let b = [o[0] + d[0] * l, o[1] + d[1] * l, o[2] + d[2] * l];
            let sel = self.sel == Sel::DatumAxis(i);
            let col = if sel { sel_col } else { self.scheme.pal.datum_axis() };
            painter.line_segment([p3(a), p3(b)], Stroke::new(if sel { 2.6 } else { 1.5 }, col));
        }

        // the toolpath in 3D (coloured per operation, limited by the progress slider)
        if let Some(prog) = &self.cam_job.program {
            let plunge = Stroke::new(1.4, self.scheme.pal.cam_plunge());
            let rapid = Stroke::new(1.0, self.scheme.pal.cam_rapid());
            let limit = self.progress_limit();
            let mut shown = 0usize;
            'outer: for (oi, tp) in prog.toolpaths.iter().enumerate() {
                let cut = Stroke::new(1.5, self.scheme.pal.cam_op(oi));
                let mut cur = [0.0, 0.0, 0.0];
                for mv in &tp.moves {
                    if shown >= limit {
                        break 'outer;
                    }
                    shown += 1;
                    let seg = match mv {
                        Move::Rapid { to } => Some(([to.x, to.y, to.z], rapid, true)),
                        Move::Linear { to, .. } => Some(([to.x, to.y, to.z], cut, false)),
                        Move::Plunge { to, .. } => Some(([to.x, to.y, to.z], plunge, false)),
                        Move::Arc { to, .. } | Move::Helix { to, .. } => Some(([to.x, to.y, to.z], cut, false)),
                        _ => None,
                    };
                    if let Some((to, stroke, is_rapid)) = seg {
                        if !is_rapid || self.set.show_rapids {
                            painter.line_segment([p3(cur), p3(to)], stroke);
                        }
                        cur = to;
                    }
                }
            }
        }
    }


    pub(super) fn draw_mesh(&self, painter: &egui::Painter, rect: Rect) {
        // a top view (or IN THE PLANE of the active sketch when editing on a face or a datum - so that the
        // body lines up with the sketch's coordinates instead of hanging above it). Shading goes by depth
        // along the normal.
        let frame = self.active_2d_sketch().and_then(|si| self.project.sketch_frame(si)).filter(|f| !f.is_identity());
        // while a sketch is being edited the body is drawn as a GHOST (dimmed and translucent) so that the
        // sketch geometry reads on top of it instead of being drowned by a bright fill.
        let ghost = self.sketch_ses.editing.is_some();
        let proj = |v: qymcad_core::geom::Point3| -> (Point2, f64) {
            match &frame {
                Some(f) => (f.project(v), f.depth(v)),
                None => (Point2::new(v.x, v.y), v.z),
            }
        };
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            let (mut dmin, mut dmax) = (f64::MAX, f64::MIN);
            for v in &mesh.verts {
                let d = proj(*v).1;
                dmin = dmin.min(d);
                dmax = dmax.max(d);
            }
            let span = (dmax - dmin).max(1e-6);
            // a body in interference gets a red fill (not in ghost mode)
            let clash = !ghost && self.project.mesh_id(mi).is_some_and(|b| self.body_interferes(b));
            let mut tris: Vec<(f64, [Pos2; 3], Color32)> = Vec::with_capacity(mesh.tris.len());
            for i in 0..mesh.tris.len() {
                let t = mesh.triangle(i);
                // THE SECTION: an honest clip (the CPU path) - 0..2 sub-triangles exactly along the plane
                let clip = self.section_clip_tri([[t[0].x, t[0].y, t[0].z], [t[1].x, t[1].y, t[1].z], [t[2].x, t[2].y, t[2].z]]);
                if !clip.whole && clip.verts.is_empty() {
                    continue;
                }
                let sub: Vec<[qymcad_core::geom::Point3; 3]> = if clip.whole {
                    vec![[t[0], t[1], t[2]]]
                } else {
                    (1..clip.verts.len().saturating_sub(1))
                        .map(|k| {
                            let g = |i: usize| qymcad_core::geom::Point3::new(clip.verts[i].pos[0], clip.verts[i].pos[1], clip.verts[i].pos[2]);
                            [g(0), g(k), g(k + 1)]
                        })
                        .collect()
                };
                for t in sub {
                let (a, da) = proj(t[0]);
                let (b, db) = proj(t[1]);
                let (c, dc) = proj(t[2]);
                let zc = (da + db + dc) / 3.0;
                let shade = ((zc - dmin) / span).clamp(0.0, 1.0) as f32;
                // Lighting: the floor comes from the palette, 1.0 at the nearest face. The palette stores the
                // body colour AS IT IS ON THE BRIGHTEST FACE - deeper down it simply fades proportionally.
                let k = crate::palette::lit(self.scheme.pal.shade_floor_mesh, shade);
                let col = if ghost {
                    // a ghost: dim and translucent, so the sketch on top of it reads
                    crate::palette::a(crate::palette::tint(self.scheme.pal.body_ghost(), k), 72)
                } else if clash {
                    // the "collision" fill, keeping the shading by depth
                    crate::palette::tint(self.scheme.pal.body_clash(), k)
                } else {
                    crate::palette::tint(self.scheme.pal.body_face(), k)
                };
                tris.push((zc, [self.to_screen(rect, a), self.to_screen(rect, b), self.to_screen(rect, c)], col));
                }
            }
            tris.sort_by(|a, b| a.0.total_cmp(&b.0)); // the bottom first, the top over it
            let mut emesh = egui::Mesh::default();
            for (_, pts, col) in &tris {
                let base = emesh.vertices.len() as u32;
                for p in pts {
                    emesh.colored_vertex(*p, *col);
                }
                emesh.add_triangle(base, base + 1, base + 2);
            }
            if !emesh.is_empty() {
                painter.add(egui::Shape::mesh(emesh));
            }
        }
        // THE SECTION CAPS (the CPU path): an amber fill on top (sorted by depth within the caps)
        if self.section.plane.is_some() {
            let caps = self.section_caps_for_frame();
            let mut ctris: Vec<(f64, [Pos2; 3])> = Vec::new();
            let basis = self.cam.basis();
            for mesh in caps.iter() {
                for t in 0..mesh.tris.len() {
                    let tri = mesh.triangle(t);
                    let pr = |p: qymcad_core::geom::Point3| self.project3([p.x, p.y, p.z], rect, &basis);
                    let (a, da) = pr(tri[0]);
                    let (b, db) = pr(tri[1]);
                    let (c, dc) = pr(tri[2]);
                    ctris.push(((da + db + dc) / 3.0, [a, b, c]));
                }
            }
            ctris.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut cm = egui::Mesh::default();
            let col = self.scheme.pal.cam_stock();
            for (_, pts) in &ctris {
                let base = cm.vertices.len() as u32;
                for p in pts {
                    cm.colored_vertex(*p, col);
                }
                cm.add_triangle(base, base + 1, base + 2);
            }
            if !cm.is_empty() {
                painter.add(egui::Shape::mesh(cm));
            }
        }
    }


    pub(super) fn draw_table(&self, painter: &egui::Painter, rect: Rect) {
        let m = &self.project.machine;
        let a = self.to_screen(rect, Point2::new(m.work_min[0], m.work_min[1]));
        let b = self.to_screen(rect, Point2::new(m.work_max[0], m.work_max[1]));
        let table = Rect::from_two_pos(a, b);
        painter.rect_stroke(table, 0.0, Stroke::new(1.0, self.scheme.pal.cam_table()), egui::StrokeKind::Middle);
    }


    /// THE ONLY renderer of the coordinate axes (it used to be duplicated in draw_sketch_grid).
    /// Outside a sketch it is a thin grey cross. Inside a sketch it is the X (red) and Y (green) axes at full
    /// length, the selected axis in orange, plus the origin marker.
    pub(super) fn draw_axes(&self, painter: &egui::Painter, rect: Rect) {
        let o = self.to_screen(rect, Point2::new(0.0, 0.0));
        let editing = matches!(self.sel, Sel::Sketch(si) if self.edit_si() == Some(si));
        if !editing {
            let ax = Stroke::new(1.0, self.scheme.pal.sketch_axis_idle());
            painter.line_segment([Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)], ax);
            painter.line_segment([Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())], ax);
            return;
        }
        // the axes as reference geometry: X red, Y green; the selected one orange and thicker
        let xsel = self.sel_sk.items.contains(&(3, 0));
        let ysel = self.sel_sk.items.contains(&(3, 1));
        let xst = if xsel { Stroke::new(2.2, self.scheme.pal.selected()) } else { Stroke::new(1.2, self.scheme.pal.sketch_axis_x()) };
        let yst = if ysel { Stroke::new(2.2, self.scheme.pal.selected()) } else { Stroke::new(1.2, self.scheme.pal.sketch_axis_y()) };
        painter.line_segment([Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)], xst);
        painter.line_segment([Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())], yst);
        // the origin marker - it lights up on hover
        let hot = self.cursor.map_or(false, |c| self.to_screen(rect, c).distance(o) <= 11.0);
        let oc = self.scheme.pal.active();
        painter.circle_stroke(o, if hot { 5.0 } else { 3.5 }, Stroke::new(1.6, oc));
        if hot {
            painter.circle_filled(o, 2.0, oc);
        }
    }


    /// Draw the associative dimensions and constraints of the selected sketch in the viewport.
    pub(super) fn draw_sketch_dims(&self, painter: &egui::Painter, rect: Rect, si: usize) {
        use qymcad_core::model::{Constraint, EntityKind};
        let Some(s) = self.project.sketches.get(si) else { return };
        let dim_col = self.scheme.pal.dimension();
        let font = egui::FontId::proportional(13.0);
        // construction geometry is dashed
        let aux_col = self.scheme.pal.sketch_construction();
        // the tangent handles of the splines: a line from the knot to the handle + a grabbable point at its end
        let hcol = self.scheme.pal.dim_helper();
        for spi in 0..s.splines.len() {
            for (knot, hend) in self.project.spline_handles(si, spi) {
                let (sk, sh) = (self.to_screen(rect, knot), self.to_screen(rect, hend));
                painter.line_segment([sk, sh], Stroke::new(1.0, hcol));
                painter.circle_filled(sh, 3.5, hcol);
                painter.circle_stroke(sh, 3.5, Stroke::new(1.0, self.scheme.pal.dim_helper_ring()));
            }
        }
        // construction splines are dashed (they do not go into a profile)
        for spi in 0..s.splines.len() {
            if !s.splines[spi].construction {
                continue;
            }
            let poly = self.project.spline_polyline(si, spi);
            if poly.len() >= 2 {
                let pts: Vec<Pos2> = poly.iter().map(|p| self.to_screen(rect, *p)).collect();
                painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
            }
        }
        let pt_of = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
        for e in &s.entities {
            if !e.construction {
                continue;
            }
            match e.kind {
                qymcad_core::model::EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt_of(a), pt_of(b)) {
                        let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                        painter.add(egui::Shape::dashed_line(&[sa, sb], Stroke::new(1.0, aux_col), 6.0, 4.0));
                    }
                }
                qymcad_core::model::EntityKind::Circle { center, r } => {
                    if let Some(c) = pt_of(center) {
                        let sc = self.to_screen(rect, c);
                        let rp = (self.to_screen(rect, Point2::new(c.x + r, c.y)).x - sc.x).abs();
                        // a dashed circle, as segments
                        let n = 48;
                        let pts: Vec<Pos2> = (0..=n).map(|k| { let a = std::f64::consts::TAU * k as f64 / n as f64; sc + egui::vec2((rp as f64 * a.cos()) as f32, (rp as f64 * a.sin()) as f32) }).collect();
                        painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
                    }
                }
                qymcad_core::model::EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt_of(center), pt_of(a), pt_of(b)) {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                        let a1 = (pb.y - c.y).atan2(pb.x - c.x);
                        let arc = qymcad_core::geom::tessellate_arc(c.x, c.y, r, a0, a1, ccw, 0.02);
                        let sp: Vec<Pos2> = arc.iter().map(|p| self.to_screen(rect, Point2::new(p.x, p.y))).collect();
                        if sp.len() >= 2 {
                            painter.add(egui::Shape::dashed_line(&sp, Stroke::new(1.0, aux_col), 6.0, 4.0));
                        }
                    }
                }
                qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => {
                    let pts: Vec<Pos2> = self.ellipse_outline_world(si, c, ma, mi).iter().map(|p| self.to_screen(rect, *p)).collect();
                    if pts.len() >= 2 {
                        painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
                    }
                }
            }
        }
        // CONFLICTING dimensions (the value contradicts the geometry) are red
        let conflicts = self.sketch_diag(si).conflicts;
        // the linear and angular dimensions (the selected one orange, a conflict red, the one under the cursor white)
        let sc = self.view.scale as f32; // the label offset `off` is in WORLD units -> pixels through the zoom scale
        for (ci, c) in s.constraints.iter().enumerate() {
            let dim_col = if Some(ci) == self.gsel.constraint {
                self.scheme.pal.selected()
            } else if conflicts.contains(&ci) {
                self.scheme.pal.error()
            } else if Some(ci) == self.hover.constraint {
                self.scheme.pal.emphasis()
            } else {
                dim_col
            };
            // a reference (driven) dimension is grey and in brackets (it does not drive the geometry)
            let driven = c.is_driven();
            let dim_col = if driven && Some(ci) != self.gsel.constraint && Some(ci) != self.hover.constraint {
                self.scheme.pal.dimension_driven()
            } else {
                dim_col
            };
            match *c {
                Constraint::Distance { a, b, d, off: doff, axis, .. } => {
                    let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) else { continue };
                    let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                    // the ends of the dimension line (la,lb) and the direction of the extension lines (perp), by orientation
                    let (la, lb, dir, perp) = match axis {
                        1 => {
                            // horizontal: the line sits at mid.y+off and measures the difference in x
                            let y = (sa.y + sb.y) / 2.0 + doff as f32 * sc;
                            (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0))
                        }
                        2 => {
                            // vertical: the line sits at mid.x+off and measures the difference in y
                            let x = (sa.x + sb.x) / 2.0 + doff as f32 * sc;
                            (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(0.0, 1.0), egui::vec2(1.0, 0.0))
                        }
                        _ => {
                            let dir = (sb - sa).normalized();
                            let perp = egui::vec2(-dir.y, dir.x);
                            let off = perp * (16.0 + doff as f32 * sc);
                            (sa + off, sb + off, dir, perp)
                        }
                    };
                    // the extension lines (from the points to the ends of the dimension line) + the dimension line
                    painter.line_segment([sa, la], Stroke::new(0.7, dim_col));
                    painter.line_segment([sb, lb], Stroke::new(0.7, dim_col));
                    painter.line_segment([la, lb], Stroke::new(1.2, dim_col));
                    let along = (lb - la).normalized();
                    for end in [(la, along), (lb, -along)] {
                        let t = end.1 * 6.0;
                        painter.line_segment([end.0, end.0 - t + perp * 2.5], Stroke::new(1.0, dim_col));
                        painter.line_segment([end.0, end.0 - t - perp * 2.5], Stroke::new(1.0, dim_col));
                    }
                    let _ = dir;
                    let mid = (la.to_vec2() + lb.to_vec2()) / 2.0 + perp * 8.0;
                    let pfx = match axis { 1 => "H ", 2 => "V ", _ => "" };
                    let txt = if driven { format!("({pfx}{d:.1})") } else { format!("{pfx}{d:.1}") };
                    painter.text(mid.to_pos2(), egui::Align2::CENTER_CENTER, txt, font.clone(), dim_col);
                }
                Constraint::EdgeDistance { c1, c2, d, m1, m2, off: doff, .. } => {
                    // a tangent dimension: the dimension line runs between the RIMS of the circles (centre +/- radius along the line of centres)
                    let (Some(p1), Some(p2)) = (self.sketch_pt(si, c1), self.sketch_pt(si, c2)) else { continue };
                    let r_of = |cid: Id| -> f64 {
                        for e in &s.entities {
                            match e.kind {
                                EntityKind::Circle { center, r } if center == cid => return r,
                                EntityKind::Arc { center, a, .. } if center == cid => {
                                    if let (Some(pc), Some(pa)) = (self.sketch_pt(si, center), self.sketch_pt(si, a)) {
                                        return ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt();
                                    }
                                }
                                _ => {}
                            }
                        }
                        0.0
                    };
                    let (r1, r2) = (r_of(c1), r_of(c2));
                    let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
                    let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
                    let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
                    let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
                    let (sa, sb) = (self.to_screen(rect, e1), self.to_screen(rect, e2));
                    let dir = (sb - sa).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    let off = perp * (16.0 + doff as f32 * sc);
                    let (la, lb) = (sa + off, sb + off);
                    painter.line_segment([sa, la], Stroke::new(0.7, dim_col));
                    painter.line_segment([sb, lb], Stroke::new(0.7, dim_col));
                    painter.line_segment([la, lb], Stroke::new(1.2, dim_col));
                    let along = (lb - la).normalized();
                    for end in [(la, along), (lb, -along)] {
                        let t = end.1 * 6.0;
                        painter.line_segment([end.0, end.0 - t + perp * 2.5], Stroke::new(1.0, dim_col));
                        painter.line_segment([end.0, end.0 - t - perp * 2.5], Stroke::new(1.0, dim_col));
                    }
                    let mid = (la.to_vec2() + lb.to_vec2()) / 2.0 + perp * 8.0;
                    let label = if driven { format!("(T {d:.1})") } else { format!("T {d:.1}") };
                    painter.text(mid.to_pos2(), egui::Align2::CENTER_CENTER, label, font.clone(), dim_col);
                }
                Constraint::DistancePL { p, a, b, d, off: doff, .. } => {
                    // the distance from point p to the line a->b: a perpendicular from p to its foot on the line
                    let (Some(pp), Some(pa)) = (self.sketch_pt(si, p), self.sketch_pt(si, a)) else { continue };
                    let (sp, sa) = (self.to_screen(rect, pp), self.to_screen(rect, pa));
                    let Some(ab) = self.line_screen_dir(si, a, b, rect) else { continue };
                    let t = (sp - sa).dot(ab);
                    let foot = sa + ab * t; // the foot of the perpendicular on the line
                    let perp = (sp - foot).normalized();
                    // the leader is offset ALONG the line (ab), so the dimension line can be raised or lowered
                    // over the geometry. It used to be offset along perp, and then the label could only travel
                    // along the axis being measured.
                    let off = ab * (doff as f32 * sc);
                    let (lp, lf) = (sp + off, foot + off);
                    painter.line_segment([sp, lp], Stroke::new(0.7, dim_col));
                    painter.line_segment([foot, lf], Stroke::new(0.7, dim_col));
                    painter.line_segment([lp, lf], Stroke::new(1.2, dim_col));
                    let along = (lf - lp).normalized();
                    for end in [(lp, -along), (lf, along)] {
                        let tt = end.1 * 6.0;
                        let pv = egui::vec2(-along.y, along.x);
                        painter.line_segment([end.0, end.0 - tt + pv * 2.5], Stroke::new(1.0, dim_col));
                        painter.line_segment([end.0, end.0 - tt - pv * 2.5], Stroke::new(1.0, dim_col));
                    }
                    let mid = (lp.to_vec2() + lf.to_vec2()) / 2.0 + perp * 8.0;
                    let dabs = d.abs(); // d is signed (it carries the side); the magnitude is shown
                    let txt = if driven { format!("({dabs:.1})") } else { format!("{dabs:.1}") };
                    painter.text(mid.to_pos2(), egui::Align2::CENTER_CENTER, txt, font.clone(), dim_col);
                }
                Constraint::Angle { a, b, c: cc, deg, .. } => {
                    let (Some(pa), Some(pb), Some(pc)) = (self.sketch_pt(si, a), self.sketch_pt(si, b), self.sketch_pt(si, cc)) else { continue };
                    let (sa, sb, sc) = (self.to_screen(rect, pa), self.to_screen(rect, pb), self.to_screen(rect, pc));
                    let u = (sa - sb).normalized();
                    let v = (sc - sb).normalized();
                    draw_dim_arc(painter, sb, u, v, 24.0, dim_col); // the arc between the angle's sides at the vertex
                    let bis = (u + v).normalized();
                    let txt = if driven { format!("({deg:.0}°)") } else { format!("{deg:.0}°") };
                    painter.text(sb + bis * 40.0, egui::Align2::CENTER_CENTER, txt, font.clone(), dim_col);
                }
                Constraint::Diameter { c, d, off, diam, .. } => {
                    let Some(cp) = self.sketch_pt(si, c) else { continue };
                    let Some(r) = self.center_radius(si, c) else { continue }; // a circle OR an arc
                    let sc = self.to_screen(rect, cp);
                    // `off` here is the leader's ANGLE (in radians, screen frame); the label travels ALONG THE
                    // CIRCLE (drag and dim_label_pos read off the same way). That allows a diameter or radius to
                    // be spun around the centre, so concentric dimensions spread out instead of merging. A
                    // diameter is a line through the centre (rim to rim), a radius runs from the centre.
                    let r_px = (self.to_screen(rect, Point2::new(cp.x + r, cp.y)) - sc).length();
                    let ang = off as f32;
                    let dir = egui::vec2(ang.cos(), ang.sin());
                    let edge = sc + dir * r_px;
                    let label_at = sc + dir * (r_px + 14.0);
                    let start = if diam { sc - dir * r_px } else { sc };
                    painter.line_segment([start, edge], Stroke::new(1.0, dim_col));
                    painter.line_segment([edge, label_at], Stroke::new(0.7, dim_col));
                    let pfx = if diam { "Ø" } else { "R" };
                    let txt = if driven { format!("({pfx}{d:.1})") } else { format!("{pfx}{d:.1}") };
                    painter.text(label_at, egui::Align2::CENTER_CENTER, txt, font.clone(), dim_col);
                }
                Constraint::ArcLength { c, a, b, off, len, .. } => {
                    // the arc length: a leader from the middle of the arc
                    let (Some(cp), Some(pa), Some(pb)) = (self.sketch_pt(si, c), self.sketch_pt(si, a), self.sketch_pt(si, b)) else { continue };
                    let r = ((pa.x - cp.x).powi(2) + (pa.y - cp.y).powi(2)).sqrt();
                    let mid = Point2::new((pa.x + pb.x) / 2.0 - cp.x, (pa.y + pb.y) / 2.0 - cp.y);
                    let ml = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-9);
                    let edge = Point2::new(cp.x + mid.x / ml * r, cp.y + mid.y / ml * r);
                    let ed = self.to_screen(rect, edge);
                    let txt = if driven { format!("(L{len:.1})") } else { format!("L{len:.1}") };
                    painter.text((ed.to_vec2() + egui::vec2(2.0, -8.0 + off as f32 * sc)).to_pos2(), egui::Align2::LEFT_CENTER, txt, font.clone(), dim_col);
                }
                Constraint::AngleLines { a, b, c: cc, d: dd, deg, .. } => {
                    let (Some(pa), Some(pb), Some(pc), Some(pd)) = (self.sketch_pt(si, a), self.sketch_pt(si, b), self.sketch_pt(si, cc), self.sketch_pt(si, dd)) else { continue };
                    let (sa, sb, sc, sd) = (self.to_screen(rect, pa), self.to_screen(rect, pb), self.to_screen(rect, pc), self.to_screen(rect, pd));
                    if let Some(ix) = lines_intersect(sa, sb, sc, sd) {
                        // the directions point away from the intersection towards the far ends (b and d are
                        // oriented outwards at creation), so the arc spans exactly the angle that is labelled
                        let u = (sb - ix).normalized();
                        let v = (sd - ix).normalized();
                        draw_dim_arc(painter, ix, u, v, 24.0, dim_col);
                        let bis = (u + v).normalized();
                        let txt = if driven { format!("({deg:.0}°)") } else { format!("{deg:.0}°") };
                        painter.text(ix + bis * 40.0, egui::Align2::CENTER_CENTER, txt, font.clone(), dim_col);
                    }
                }
                _ => {}
            }
        }
        // the radii and diameters of circle entities and of arcs and fillets:
        // the selected entity (in sk_sel) or the one being edited is highlighted
        for e in &s.entities {
            let dim_col = if self.sel_sk.items.contains(&(1, e.id)) || self.inline.circle() == Some(e.id) {
                self.scheme.pal.selected()
            } else {
                dim_col
            };
            match e.kind {
                EntityKind::Circle { center, r } => {
                    // if the circle already carries a diameter or radius dimension (a Diameter constraint), that loop draws it
                    let has_dim = s.constraints.iter().any(|x| matches!(x, Constraint::Diameter { c, .. } if *c == center));
                    if !has_dim {
                        if let Some(cp) = self.sketch_pt(si, center) {
                            // drawn in the same style as a Diameter constraint at off=0 (the radius to the right,
                            // the label beyond the rim), so that grabbing the label (which materialises a
                            // reference diameter) causes no jump.
                            let sc = self.to_screen(rect, cp);
                            let r_px = (self.to_screen(rect, Point2::new(cp.x + r, cp.y)) - sc).length();
                            let dir = egui::vec2(1.0, 0.0);
                            let edge = sc + dir * r_px;
                            let label_at = sc + dir * (r_px + 14.0);
                            painter.line_segment([sc, edge], Stroke::new(1.0, dim_col));
                            painter.line_segment([edge, label_at], Stroke::new(0.7, dim_col));
                            painter.text(label_at, egui::Align2::CENTER_CENTER, format!("Ø{:.1}", 2.0 * r), font.clone(), dim_col);
                        }
                    }
                }
                EntityKind::Arc { center, a, b, .. } => {
                    // if the arc already carries a radius or diameter dimension (a Diameter constraint), that loop above draws it
                    let has_dim = s.constraints.iter().any(|x| matches!(x, Constraint::Diameter { c, .. } if *c == center));
                    // a fillet radius: an R leader from the centre to the middle of the arc, the label beyond
                    // the rim (r+14) - the same style and position that passive_radius_label_at grabs, otherwise
                    // the two would not line up.
                    if !has_dim {
                        if let (Some(cp), Some(pa), Some(pb)) = (self.sketch_pt(si, center), self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                            let r = ((pa.x - cp.x).powi(2) + (pa.y - cp.y).powi(2)).sqrt();
                            let sc = self.to_screen(rect, cp);
                            let r_px = (self.to_screen(rect, Point2::new(cp.x + r, cp.y)) - sc).length();
                            let m = self.to_screen(rect, Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0)) - sc;
                            let dir = if m.length() > 1e-3 { m.normalized() } else { egui::vec2(1.0, 0.0) };
                            let edge = sc + dir * r_px;
                            let label_at = sc + dir * (r_px + 14.0);
                            painter.line_segment([sc, edge], Stroke::new(1.0, dim_col));
                            painter.line_segment([edge, label_at], Stroke::new(0.7, dim_col));
                            painter.text(label_at, egui::Align2::CENTER_CENTER, format!("R{r:.1}"), font.clone(), dim_col);
                        }
                    }
                }
                _ => {}
            }
        }
    }


    /// Draw a ghost of entity `eid`, transforming its points with the function `f` (world -> world). Shared by
    /// the move/copy preview (the selected entities shifted by cursor minus base) and by the pattern preview.
    pub(super) fn draw_entity_xform(&self, painter: &egui::Painter, rect: Rect, si: usize, eid: Id, f: &dyn Fn(f64, f64) -> (f64, f64), stroke: Stroke) {
        use qymcad_core::model::EntityKind;
        let Some(s) = self.project.sketches.get(si) else { return };
        let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return };
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| { let (x, y) = f(q.x, q.y); Point2::new(x, y) });
        match kind {
            EntityKind::Line { a, b } => {
                if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                    painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], stroke);
                }
            }
            EntityKind::Circle { center, r } => {
                if let (Some(c), Some(redge)) = (pt(center), s.points.iter().find(|q| q.id == center).map(|q| { let (x, y) = f(q.x + r, q.y); Point2::new(x, y) })) {
                    let sc = self.to_screen(rect, c);
                    painter.circle_stroke(sc, self.to_screen(rect, redge).distance(sc), stroke);
                }
            }
            EntityKind::Arc { center, a, b, ccw } => {
                if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                    let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                    let mut a1 = (pb.y - c.y).atan2(pb.x - c.x);
                    if ccw && a1 < a0 {
                        a1 += std::f64::consts::TAU;
                    } else if !ccw && a1 > a0 {
                        a1 -= std::f64::consts::TAU;
                    }
                    let pts: Vec<Pos2> = (0..=40).map(|k| { let a = a0 + (a1 - a0) * k as f64 / 40.0; self.to_screen(rect, Point2::new(c.x + r * a.cos(), c.y + r * a.sin())) }).collect();
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
            EntityKind::Ellipse { c, ma, mi } => {
                if let (Some(pc), Some(pma), Some(pmi)) = (pt(c), pt(ma), pt(mi)) {
                    let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(1e-6);
                    let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(1e-6);
                    let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major);
                    let (vx, vy) = (-uy, ux);
                    let pts: Vec<Pos2> = (0..=48).map(|k| { let t = std::f64::consts::TAU * k as f64 / 48.0; let (ct, st) = (t.cos(), t.sin()); self.to_screen(rect, Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy)) }).collect();
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
        }
    }


    pub(super) fn draw_move_preview(&self, painter: &egui::Painter, rect: Rect) {
        if self.tool.move_op == 0 {
            return;
        }
        let Sel::Sketch(si) = self.sel else { return };
        let Some(base) = self.tool.move_base else { return };
        // rotation: the ghost is turned by rot_angle about the centre (the angle comes from the popup, not from the cursor)
        if self.tool.move_op == 3 {
            let (sn, cs) = (self.rot.angle.to_radians().sin(), self.rot.angle.to_radians().cos());
            let col = self.scheme.pal.preview_axis();
            let stroke = Stroke::new(1.4, col);
            let f = move |x: f64, y: f64| {
                let (px, py) = (x - base.x, y - base.y);
                (base.x + px * cs - py * sn, base.y + px * sn + py * cs)
            };
            for id in self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id) {
                self.draw_entity_xform(painter, rect, si, id, &f, stroke);
            }
            let c = self.to_screen(rect, base);
            painter.circle_filled(c, 4.0, col); // the centre of rotation
            painter.circle_stroke(c, 9.0, Stroke::new(0.8, col));
            return;
        }
        let Some(cur) = self.cursor else { return };
        let (dx, dy) = (cur.x - base.x, cur.y - base.y);
        let col = if self.tool.move_op == 2 { self.scheme.pal.clip() } else { self.scheme.pal.highlight() };
        let stroke = Stroke::new(1.4, col);
        let f = move |x: f64, y: f64| (x + dx, y + dy);
        for (k, id) in self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).enumerate() {
            let _ = k;
            self.draw_entity_xform(painter, rect, si, id, &f, stroke);
        }
        painter.circle_filled(self.to_screen(rect, base), 3.0, col);
        painter.line_segment([self.to_screen(rect, base), self.to_screen(rect, cur)], Stroke::new(0.8, col));
    }


    /// Waiting for a base point after Ctrl+C/X: the selected geometry is lit green (a hint that this is what
    /// will be copied) and a green crosshair is drawn under the cursor (a hint to click the base point).
    pub(super) fn draw_clip_pending(&self, painter: &egui::Painter, rect: Rect) {
        let Some((eids, _cut)) = self.clip.geom_pending.as_ref() else { return };
        let Some(si) = self.edit_si() else { return };
        let col = self.scheme.pal.clip();
        let stroke = Stroke::new(2.2, col);
        let f = |x: f64, y: f64| (x, y);
        for &id in eids {
            self.draw_entity_xform(painter, rect, si, id, &f, stroke);
        }
        if let Some(cur) = self.cursor {
            let c = self.to_screen(rect, cur);
            let arm = 9.0;
            painter.line_segment([c + egui::vec2(-arm, 0.0), c + egui::vec2(arm, 0.0)], Stroke::new(1.6, col));
            painter.line_segment([c + egui::vec2(0.0, -arm), c + egui::vec2(0.0, arm)], Stroke::new(1.6, col));
            painter.circle_stroke(c, 4.0, Stroke::new(1.2, col));
        }
    }


    /// WHERE THE COPIES WILL LAND - one computation for the ghost and for the check alike.
    ///
    /// Without a preview the command is as blind as push-face once was: the count and the step are set, but
    /// where the row will stand can only be seen by applying it and undoing.
    ///
    /// It is computed by exactly the same means as the finished pattern (`resolve_comp_patterns`): the step is
    /// applied in the source's PARENT frame, not in the view frame. Let those two computations drift apart and
    /// the ghost would again point somewhere other than where the copies later land.
    pub(super) fn comp_array_ghosts(&self, pre: &[f64; 12], base: &[f64; 12], already: usize) -> Vec<[f64; 12]> {
        use qymcad_core::feature::mat_mul12;
        let kind = self.comp_array_kind();
        (1..kind.count())
            .filter(|i| (*i as usize) > already) // the copy already stands in the document - a ghost would be its twin
            .map(|i| mat_mul12(pre, &mat_mul12(&kind.step_transform(i), base)))
            .collect()
    }

    pub(super) fn draw_comp_array_preview(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::feature::apply12;
        if self.carr.mode == 0 || !self.mode_3d {
            return;
        }
        let Some(src_body) = self.project.active_body(self.carr.src) else { return };
        let Some(mi) = self.project.mesh_index(src_body) else { return };
        let Some(bb) = self.project.bodies[mi].mesh.bounds() else { return };
        let ctx = self.current_ctx_id();
        // THE GHOST IS COMPUTED THE WAY THE COPY WILL LATER LAND.
        //
        // This used to be `mat_mul12(step, wt)`, where `wt` is the body's transform IN THE VIEW FRAME. The
        // finished pattern places a copy differently: `mat_mul12(step, base)` in the source's PARENT frame
        // (`resolve_comp_patterns`). The two agreed only while the parent sat at the origin; as soon as the
        // source lay in an assembly with a position of its own, the ghosts drifted off the step.
        let (base, parent) = self
            .project
            .components
            .iter()
            .find(|c| c.id == self.carr.src)
            .map(|c| (c.transform, c.parent.unwrap_or(self.project.root)))
            .unwrap_or((qymcad_core::feature::PLACE_IDENTITY, self.project.root));
        let pre = self.project.relative_transform(parent, ctx);
        // EDITING A FINISHED PATTERN: the copies already stand on the screen. Drawing ghosts on top of them
        // would show twice as many frames as there are bodies.
        let already: usize = match self.project.comp_patterns.iter().find(|p| p.id == self.carr.edit) {
            Some(p) => p.copies.len(),
            None => 0,
        };
        let basis = self.cam.basis();
        let st = Stroke::new(1.3, crate::palette::a(self.scheme.pal.preview_datum(), 190));
        let (mn, mx) = (bb.min, bb.max);
        let corners: [[f64; 3]; 8] = [
            [mn.x, mn.y, mn.z], [mx.x, mn.y, mn.z], [mx.x, mx.y, mn.z], [mn.x, mx.y, mn.z],
            [mn.x, mn.y, mx.z], [mx.x, mn.y, mx.z], [mx.x, mx.y, mx.z], [mn.x, mx.y, mx.z],
        ];
        const EDGES: [(usize, usize); 12] = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
        // GHOSTS ONLY FOR THE COPIES (i from 1): instance zero is the source itself, which is on screen anyway
        for m in self.comp_array_ghosts(&pre, &base, already) {
            let pts: Vec<Pos2> = corners.iter().map(|c| self.project3(apply12(&m, *c), rect, &basis).0).collect();
            for (a, b) in EDGES {
                painter.line_segment([pts[a], pts[b]], st);
            }
        }
    }

    /// MEASURING IN 3D: markers on the picked elements, a leader between them and a plate with the number.
    ///
    /// The number lives both in the status line and here, from ONE source (`measure_text`): two wordings of one
    /// measurement drift apart silently, and then the line says one thing while the geometry says another.
    pub(super) fn draw_measure_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.m3.on || self.m3.picks.is_empty() {
            return;
        }
        let basis = self.cam.basis();
        let col = self.scheme.pal.measure();
        let pts: Vec<Pos2> = self.m3.picks.iter().map(|p| self.project3(p.at, rect, &basis).0).collect();
        for sp in &pts {
            painter.circle_stroke(*sp, 6.0, Stroke::new(2.0, col));
            painter.circle_filled(*sp, 2.5, col);
        }
        if pts.len() == 2 {
            painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.6, col), 7.0, 5.0));
        }
        let text = self.measure_text();
        let at = pts.last().copied().unwrap_or(rect.center()) + egui::vec2(12.0, -18.0);
        let font = egui::FontId::proportional(13.0);
        let galley = painter.layout_no_wrap(text, font, self.scheme.pal.plate_text());
        let pad = egui::vec2(6.0, 4.0);
        let bg = egui::Rect::from_min_size(at, galley.size() + pad * 2.0);
        painter.rect_filled(bg, 4.0, crate::palette::a(self.scheme.pal.measure(), 235));
        painter.galley(at + pad, galley, self.scheme.pal.plate_text());
    }

    /// THE DRIVEN GEOMETRY OF PROJECTIONS — in a colour of its own over the ordinary kind.
    ///
    /// A projection enters a profile on a par with one's own geometry (it is what gets extruded), so it
    /// is drawn as an ordinary outline. But a person must SEE that it is a view of the part rather than
    /// something they drew: otherwise it is not clear why the line does not drag with the mouse. A lost
    /// source is drawn in red.
    pub(super) fn draw_projection_overlay(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::model::EntityKind;
        let Sel::Sketch(si) = self.sel else { return };
        let Some(s) = self.project.sketches.get(si) else { return };
        if s.projections.is_empty() {
            return;
        }
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        for proj in &s.projections {
            let col = if proj.lost { self.scheme.pal.error() } else { self.scheme.pal.sketch_driven() };
            let stroke = Stroke::new(if proj.lost { 2.2 } else { 1.8 }, col);
            for eid in &proj.entities {
                let Some(kind) = s.entities.iter().find(|e| e.id == *eid).map(|e| e.kind) else { continue };
                match kind {
                    EntityKind::Line { a, b } => {
                        if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                            painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], stroke);
                        }
                    }
                    EntityKind::Circle { center, r } => {
                        if let Some(c) = pt(center) {
                            let sc = self.to_screen(rect, c);
                            let rp = (self.to_screen(rect, Point2::new(c.x + r, c.y)).x - sc.x).abs();
                            painter.circle_stroke(sc, rp.max(1.0), stroke);
                        }
                    }
                    EntityKind::Arc { center, a, b, ccw } => {
                        if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                            let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                            let (a0, a1) = ((pa.y - c.y).atan2(pa.x - c.x), (pb.y - c.y).atan2(pb.x - c.x));
                            let arc = qymcad_core::geom::tessellate_arc(c.x, c.y, r, a0, a1, ccw, 0.02);
                            let sp: Vec<Pos2> = arc.iter().map(|q| self.to_screen(rect, Point2::new(q.x, q.y))).collect();
                            if sp.len() >= 2 {
                                painter.add(egui::Shape::line(sp, stroke));
                            }
                        }
                    }
                    EntityKind::Ellipse { .. } => {}
                }
            }
            // THE DRIVEN NODES as small circles: they can be snapped to, but not dragged
            for pid in &proj.points {
                if let Some(q) = pt(*pid) {
                    painter.circle_stroke(self.to_screen(rect, q), 3.0, Stroke::new(1.2, col));
                }
            }
        }
    }

    /// The ghost of an insertion of geometry from the clipboard: green copies of the clipboard
    /// entities following the cursor so that the anchor point coincides with it. Drawn straight from
    /// the clipboard data (the entities are not in the sketch yet).
    pub(super) fn draw_clip_ghost(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::model::EntityKind;
        if !self.clip.geom_place {
            return;
        }
        let Some(clip) = self.clip.geom.as_ref() else { return };
        let Some(cur) = self.cursor else { return };
        let (ox, oy) = (cur.x - clip.ref_x, cur.y - clip.ref_y);
        let col = self.scheme.pal.clip();
        let stroke = Stroke::new(1.4, col);
        let pt = |id: Id| clip.points.iter().find(|(pid, ..)| *pid == id).map(|(_, x, y)| Point2::new(x + ox, y + oy));
        for e in &clip.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], stroke);
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some(c) = pt(center) {
                        let sc = self.to_screen(rect, c);
                        let redge = self.to_screen(rect, Point2::new(c.x + r, c.y));
                        painter.circle_stroke(sc, redge.distance(sc), stroke);
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                        let mut a1 = (pb.y - c.y).atan2(pb.x - c.x);
                        if ccw && a1 < a0 {
                            a1 += std::f64::consts::TAU;
                        } else if !ccw && a1 > a0 {
                            a1 -= std::f64::consts::TAU;
                        }
                        let pts: Vec<Pos2> = (0..=40).map(|k| { let a = a0 + (a1 - a0) * k as f64 / 40.0; self.to_screen(rect, Point2::new(c.x + r * a.cos(), c.y + r * a.sin())) }).collect();
                        painter.add(egui::Shape::line(pts, stroke));
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some(pc), Some(pma), Some(pmi)) = (pt(c), pt(ma), pt(mi)) {
                        let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major);
                        let (vx, vy) = (-uy, ux);
                        let pts: Vec<Pos2> = (0..=48).map(|k| { let t = std::f64::consts::TAU * k as f64 / 48.0; let (ct, st) = (t.cos(), t.sin()); self.to_screen(rect, Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy)) }).collect();
                        painter.add(egui::Shape::line(pts, stroke));
                    }
                }
            }
        }
        painter.circle_filled(self.to_screen(rect, cur), 3.0, col);
    }


    /// The pattern preview: ghosts of the copies from the row's current parameters. The source is either the
    /// selection (for a new pattern) or the source of the pattern being edited.
    pub(super) fn draw_pattern_preview(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::model::PatternKind;
        if self.pat.op == 0 {
            return;
        }
        let Sel::Sketch(si) = self.sel else { return };
        let eids: Vec<Id> = if let Some(pi) = self.pat.edit {
            self.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default()
        } else {
            self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect()
        };
        if eids.is_empty() {
            return;
        }
        let kind = self.current_pattern_kind(si, &eids);
        let stroke = Stroke::new(1.2, self.scheme.pal.highlight());
        // the centre of rotation of a circular pattern as a cross (so that what it is built around is visible)
        if let qymcad_core::model::PatternKind::Circular { cx, cy, .. } = kind {
            let c = self.to_screen(rect, Point2::new(cx, cy));
            let m = self.scheme.pal.pattern_center();
            painter.line_segment([c + egui::vec2(-7.0, 0.0), c + egui::vec2(7.0, 0.0)], Stroke::new(1.5, m));
            painter.line_segment([c + egui::vec2(0.0, -7.0), c + egui::vec2(0.0, 7.0)], Stroke::new(1.5, m));
            painter.circle_stroke(c, 4.0, Stroke::new(1.2, m));
        }
        let transforms: Vec<Box<dyn Fn(f64, f64) -> (f64, f64)>> = match kind {
            PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
                let mut v: Vec<Box<dyn Fn(f64, f64) -> (f64, f64)>> = Vec::new();
                for i in 0..count.max(1) {
                    for j in 0..count2.max(1) {
                        if i == 0 && j == 0 {
                            continue;
                        }
                        let (ox, oy) = (dx * i as f64 + dx2 * j as f64, dy * i as f64 + dy2 * j as f64);
                        v.push(Box::new(move |x, y| (x + ox, y + oy)));
                    }
                }
                v
            }
            PatternKind::Circular { cx, cy, count: c, total_deg } => {
                let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / c as f64 } else { total_deg / c.max(2) as f64 };
                (1..c.max(1))
                    .map(|k| {
                        let ang = (step * k as f64).to_radians();
                        let (s_, c_) = (ang.sin(), ang.cos());
                        Box::new(move |x: f64, y: f64| { let (vx, vy) = (x - cx, y - cy); (cx + vx * c_ - vy * s_, cy + vx * s_ + vy * c_) }) as Box<dyn Fn(f64, f64) -> (f64, f64)>
                    })
                    .collect()
            }
        };
        for xf in &transforms {
            for &eid in &eids {
                self.draw_entity_xform(painter, rect, si, eid, xf.as_ref(), stroke);
            }
        }
    }


    /// The hover preview for trim, extend and break: what will happen if the entity under the cursor is
    /// clicked. Trim lights the span that will be removed in red; break puts a marker at the point; extend
    /// lights in green the end that will be pulled.
    pub(super) fn draw_trim_preview(&self, painter: &egui::Painter, rect: Rect) {
        use qymcad_core::model::EntityKind;
        if self.tool.click_op == 0 {
            return;
        }
        let Some(cur) = self.cursor else { return };
        let Sel::Sketch(si) = self.sel else { return };
        let pos = self.to_screen(rect, cur);
        let Some(eid) = self.nearest_line_eid(rect, pos, si).or_else(|| self.nearest_circle_entity(rect, pos, si)) else { return };
        let Some(s) = self.project.sketches.get(si) else { return };
        let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return };
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        let red = self.scheme.pal.error();
        let green = self.scheme.pal.add();
        let inter = self.project.entity_intersections(si, eid);
        match self.tool.click_op {
            1 => {
                // TRIM: light the span that will be removed in red
                match kind {
                    EntityKind::Line { a, b } => {
                        let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { return };
                        let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                        let len2 = dx * dx + dy * dy;
                        if len2 < 1e-9 {
                            return;
                        }
                        let param = |x: f64, y: f64| ((x - pa.x) * dx + (y - pa.y) * dy) / len2;
                        let mut ts: Vec<f64> = inter.iter().map(|&(x, y)| param(x, y)).filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6).collect();
                        ts.push(0.0);
                        ts.push(1.0);
                        ts.sort_by(|x, y| x.total_cmp(y));
                        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                        let tc = param(cur.x, cur.y).clamp(0.0, 1.0);
                        for w in ts.windows(2) {
                            if tc >= w[0] - 1e-9 && tc <= w[1] + 1e-9 {
                                let p0 = self.to_screen(rect, Point2::new(pa.x + dx * w[0], pa.y + dy * w[0]));
                                let p1 = self.to_screen(rect, Point2::new(pa.x + dx * w[1], pa.y + dy * w[1]));
                                painter.line_segment([p0, p1], Stroke::new(3.0, red));
                                break;
                            }
                        }
                    }
                    EntityKind::Circle { center, r } => {
                        let Some(c) = pt(center) else { return };
                        let mut angs: Vec<f64> = inter.iter().map(|&(x, y)| (y - c.y).atan2(x - c.x)).collect();
                        self.draw_curve_trim_span(painter, rect, c, r, &mut angs, None, cur, red);
                    }
                    EntityKind::Arc { center, a, b, ccw } => {
                        let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) else { return };
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                        let a1 = (pb.y - c.y).atan2(pb.x - c.x);
                        let mut angs: Vec<f64> = inter.iter().map(|&(x, y)| (y - c.y).atan2(x - c.x)).collect();
                        self.draw_curve_trim_span(painter, rect, c, r, &mut angs, Some((a0, a1, ccw)), cur, red);
                    }
                    EntityKind::Ellipse { .. } => {}
                }
            }
            2 => {
                // EXTEND: in green, the end that will be pulled (the one nearest the cursor)
                let ends = match kind {
                    EntityKind::Line { a, b } => vec![pt(a), pt(b)],
                    EntityKind::Arc { a, b, .. } => vec![pt(a), pt(b)],
                    _ => vec![],
                };
                if let Some(end) = ends.into_iter().flatten().min_by(|p, q| {
                    let dp = (p.x - cur.x).powi(2) + (p.y - cur.y).powi(2);
                    let dq = (q.x - cur.x).powi(2) + (q.y - cur.y).powi(2);
                    dp.total_cmp(&dq)
                }) {
                    painter.circle_stroke(self.to_screen(rect, end), 6.0, Stroke::new(2.0, green));
                }
            }
            3 => {
                // BREAK: a marker at the cut point (the cursor projected onto the entity)
                let split = match kind {
                    EntityKind::Line { a, b } => {
                        if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                            let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                            let len2 = (dx * dx + dy * dy).max(1e-9);
                            let t = (((cur.x - pa.x) * dx + (cur.y - pa.y) * dy) / len2).clamp(0.05, 0.95);
                            Some(Point2::new(pa.x + dx * t, pa.y + dy * t))
                        } else {
                            None
                        }
                    }
                    EntityKind::Circle { center, r } => pt(center).map(|c| {
                        let ang = (cur.y - c.y).atan2(cur.x - c.x);
                        Point2::new(c.x + r * ang.cos(), c.y + r * ang.sin())
                    }),
                    EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                        (Some(c), Some(pa)) => {
                            let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                            let ang = (cur.y - c.y).atan2(cur.x - c.x);
                            Some(Point2::new(c.x + r * ang.cos(), c.y + r * ang.sin()))
                        }
                        _ => None,
                    },
                    EntityKind::Ellipse { .. } => None,
                };
                if let Some(p) = split {
                    let sp = self.to_screen(rect, p);
                    painter.circle_filled(sp, 4.0, green);
                    painter.circle_stroke(sp, 7.0, Stroke::new(1.5, green));
                }
            }
            _ => {}
        }
    }


    /// Light the angular span of a circle or arc that will be removed (for draw_trim_preview).
    pub(super) fn draw_curve_trim_span(&self, painter: &egui::Painter, rect: Rect, c: Point2, r: f64, angs: &mut Vec<f64>, span: Option<(f64, f64, bool)>, cur: Point2, col: Color32) {
        use std::f64::consts::TAU;
        let click_ang = (cur.y - c.y).atan2(cur.x - c.x);
        let arc_poly = |painter: &egui::Painter, g0: f64, g1: f64| {
            let n = 24;
            let pts: Vec<Pos2> = (0..=n).map(|k| { let g = g0 + (g1 - g0) * k as f64 / n as f64; self.to_screen(rect, Point2::new(c.x + r * g.cos(), c.y + r * g.sin())) }).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(3.0, col)));
        };
        match span {
            None => {
                // a circle: the span between neighbouring cut angles that contains the click
                let mut a: Vec<f64> = angs.iter().map(|x| x.rem_euclid(TAU)).collect();
                a.sort_by(|x, y| x.total_cmp(y));
                a.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                if a.len() < 2 {
                    return;
                }
                let ca = click_ang.rem_euclid(TAU);
                let n = a.len();
                for i in 0..n {
                    let (a0, a1) = (a[i], if i + 1 < n { a[i + 1] } else { a[0] + TAU });
                    if (ca >= a0 - 1e-9 && ca < a1) || (ca + TAU >= a0 && ca + TAU < a1) {
                        arc_poly(painter, a0, a1);
                        return;
                    }
                }
            }
            Some((a0, a1, ccw)) => {
                let to_param = |ang: f64| if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) };
                let sweep = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
                let mut ps: Vec<f64> = angs.iter().map(|&x| to_param(x)).filter(|&v| v > 1e-6 && v < sweep - 1e-6).collect();
                ps.push(0.0);
                ps.push(sweep);
                ps.sort_by(|x, y| x.total_cmp(y));
                ps.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                let cp = to_param(click_ang);
                for w in ps.windows(2) {
                    if cp > w[0] - 1e-9 && cp < w[1] + 1e-9 {
                        let (g0, g1) = if ccw { (a0 + w[0], a0 + w[1]) } else { (a0 - w[0], a0 - w[1]) };
                        arc_poly(painter, g0, g1);
                        return;
                    }
                }
            }
        }
    }


    pub(super) fn draw_sketch_preview(&self, painter: &egui::Painter, rect: Rect) {
        if self.tool.kind == 0 {
            return;
        }
        let col = if self.tool.construction { self.scheme.pal.sketch_construction() } else { self.scheme.pal.sketch_line() };
        let stroke = Stroke::new(1.3, col);
        for p in &self.tool.pts {
            painter.circle_filled(self.to_screen(rect, *p), 3.0, col);
        }
        let Some(cur) = self.cursor else { return };
        let sc = self.to_screen(rect, cur);
        match self.tool.kind {
            1 => {
                if let Some(&last) = self.tool.pts.last() {
                    painter.line_segment([self.to_screen(rect, last), sc], stroke);
                    // a live preview of the automatic constraints: what will be attached if a point is placed here
                    if let Some(si) = self.edit_si() {
                        let prev = (self.tool.pts.len() >= 2).then(|| self.tool.pts[self.tool.pts.len() - 2]);
                        for (g, at) in self.infer_hints(si, prev, last, cur) {
                            let sat = self.to_screen(rect, at) + egui::vec2(11.0, -11.0);
                            painter.rect_filled(Rect::from_center_size(sat, egui::vec2(14.0, 14.0)), 2.0, self.scheme.pal.constraint_ok());
                            paint_gly(painter, sat, 4.0, g, self.scheme.pal.glyph_text());
                        }
                    }
                }
            }
            2 => match self.tool_prefs.rect_mode {
                1 => {
                    // centre plus corner: the extent is mirrored through the centre
                    if let Some(&c) = self.tool.pts.first() {
                        let opp = self.to_screen(rect, Point2::new(2.0 * c.x - cur.x, 2.0 * c.y - cur.y));
                        painter.rect_stroke(Rect::from_two_pos(opp, sc), 0.0, stroke, egui::StrokeKind::Middle);
                    }
                }
                2 => {
                    // rotated, by three points: the first point to the cursor is one side; after the second, the frame follows the height
                    if self.tool.pts.len() == 1 {
                        painter.line_segment([self.to_screen(rect, self.tool.pts[0]), sc], stroke);
                    } else if self.tool.pts.len() == 2 {
                        let (p1, p2) = (self.tool.pts[0], self.tool.pts[1]);
                        let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
                        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                        let (nx, ny) = (-dy / len, dx / len);
                        let h = (cur.x - p2.x) * nx + (cur.y - p2.y) * ny;
                        let p3 = Point2::new(p2.x + nx * h, p2.y + ny * h);
                        let p4 = Point2::new(p1.x + nx * h, p1.y + ny * h);
                        let poly: Vec<Pos2> = [p1, p2, p3, p4, p1].iter().map(|p| self.to_screen(rect, *p)).collect();
                        painter.add(egui::Shape::line(poly, stroke));
                    }
                }
                _ => {
                    if let Some(&a) = self.tool.pts.first() {
                        painter.rect_stroke(Rect::from_two_pos(self.to_screen(rect, a), sc), 0.0, stroke, egui::StrokeKind::Middle);
                    }
                }
            },
            3 => {
                if self.tool_prefs.circ_mode == 2 {
                    // a tangent circle: once the base is chosen, a circle at the cursor with the tangency radius
                    if let (Some(eref), Some(si)) = (self.tool.circ_tan, self.edit_si()) {
                        let r = self.tangent_radius_to_edge(si, eref, cur);
                        if r > 1e-6 {
                            let scn = self.to_screen(rect, cur);
                            let rp = (self.to_screen(rect, Point2::new(cur.x + r, cur.y)).x - scn.x).abs();
                            painter.circle_stroke(scn, rp, stroke);
                        }
                    }
                } else if let Some(&c) = self.tool.pts.first() {
                    if self.tool_prefs.circ_mode == 1 {
                        // by two points: the diameter runs from c to the cursor
                        let mid = self.to_screen(rect, Point2::new((c.x + cur.x) / 2.0, (c.y + cur.y) / 2.0));
                        painter.circle_stroke(mid, mid.distance(self.to_screen(rect, c)), stroke);
                    } else {
                        let scn = self.to_screen(rect, c);
                        painter.circle_stroke(scn, scn.distance(sc), stroke);
                    }
                }
            }
            4 if self.tool_prefs.arc_mode == 2 => {
                // a tangent arc: once started (at the end of a curve), an arc from s to the cursor, smooth into the base
                if self.tool.pts.len() == 1 {
                    let s = self.tool.pts[0];
                    if let Some((t, _)) = self.edit_si().and_then(|si| self.arc_tangent_ref(si, s)) {
                        if let Some((cx, cy, r, ccw)) = tangent_arc(s, t, cur) {
                            let a0 = (s.y - cy).atan2(s.x - cx);
                            let mut a1 = (cur.y - cy).atan2(cur.x - cx);
                            if ccw && a1 < a0 {
                                a1 += std::f64::consts::TAU;
                            } else if !ccw && a1 > a0 {
                                a1 -= std::f64::consts::TAU;
                            }
                            let pts: Vec<Pos2> = (0..=40).map(|k| { let a = a0 + (a1 - a0) * k as f64 / 40.0; self.to_screen(rect, Point2::new(cx + r * a.cos(), cy + r * a.sin())) }).collect();
                            painter.add(egui::Shape::line(pts, stroke));
                        } else {
                            painter.line_segment([self.to_screen(rect, s), sc], stroke);
                        }
                    } else {
                        painter.line_segment([self.to_screen(rect, s), sc], Stroke::new(0.6, col));
                    }
                }
            }
            4 => {
                if self.tool_prefs.arc_mode == 1 {
                    // by three points: the start, the end, and the cursor as a point on the arc
                    if self.tool.pts.len() == 1 {
                        painter.line_segment([self.to_screen(rect, self.tool.pts[0]), sc], Stroke::new(0.6, col));
                    } else if self.tool.pts.len() == 2 {
                        let (s, e) = (self.tool.pts[0], self.tool.pts[1]);
                        if let Some((cx, cy, _r)) = circumcircle(s, e, cur) {
                            let cen = Point2::new(cx, cy);
                            let a0 = (s.y - cy).atan2(s.x - cx);
                            let mut a1 = (e.y - cy).atan2(e.x - cx);
                            let ccw = (cur.x - s.x) * (e.y - s.y) - (cur.y - s.y) * (e.x - s.x) > 0.0;
                            if ccw && a1 < a0 {
                                a1 += std::f64::consts::TAU;
                            } else if !ccw && a1 > a0 {
                                a1 -= std::f64::consts::TAU;
                            }
                            let r = ((s.x - cx).powi(2) + (s.y - cy).powi(2)).sqrt();
                            let pts: Vec<Pos2> = (0..=40).map(|k| { let t = a0 + (a1 - a0) * k as f64 / 40.0; self.to_screen(rect, Point2::new(cen.x + r * t.cos(), cen.y + r * t.sin())) }).collect();
                            painter.add(egui::Shape::line(pts, stroke));
                        } else {
                            painter.line_segment([self.to_screen(rect, s), sc], stroke);
                        }
                    }
                } else if let Some(&c) = self.tool.pts.first() {
                    let scn = self.to_screen(rect, c);
                    painter.line_segment([scn, sc], Stroke::new(0.6, col));
                    painter.circle_stroke(scn, scn.distance(sc), Stroke::new(0.6, col));
                }
            }
            6 => {
                if let Some(&c) = self.tool.pts.first() {
                    let n = self.tool_prefs.poly_n.max(3);
                    let half = std::f64::consts::PI / n as f64;
                    let a0 = (cur.y - c.y).atan2(cur.x - c.x) as f64;
                    let r_click = ((cur.x - c.x).powi(2) + (cur.y - c.y).powi(2)).sqrt().max(1e-6);
                    let r = match self.tool_prefs.poly_mode {
                        1 => r_click / half.cos(),
                        2 => self.tool_prefs.poly_edge.max(0.01) / (2.0 * half.sin()),
                        _ => r_click,
                    };
                    let pts: Vec<Pos2> = (0..=n)
                        .map(|k| {
                            let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
                            self.to_screen(rect, Point2::new(c.x + r * a.cos(), c.y + r * a.sin()))
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
            7 => {
                if self.tool.pts.len() == 1 {
                    painter.line_segment([self.to_screen(rect, self.tool.pts[0]), sc], stroke);
                } else if self.tool.pts.len() == 2 {
                    let (a, b) = (self.tool.pts[0], self.tool.pts[1]);
                    let (sa, sb) = (self.to_screen(rect, a), self.to_screen(rect, b));
                    painter.line_segment([sa, sb], stroke);
                    let r = (sc - sb).length().min((sc - sa).length());
                    painter.circle_stroke(sa, r, Stroke::new(0.6, col));
                    painter.circle_stroke(sb, r, Stroke::new(0.6, col));
                }
            }
            8 => {
                if let Some(&cc) = self.tool.pts.first() {
                    let (rx, ry) = ((cur.x - cc.x).abs(), (cur.y - cc.y).abs());
                    let pts: Vec<Pos2> = (0..=48).map(|i| { let a = std::f64::consts::TAU * i as f64 / 48.0; self.to_screen(rect, Point2::new(cc.x + rx * a.cos(), cc.y + ry * a.sin())) }).collect();
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
            9 => {
                // a polyline preview of the knots + a tail to the cursor
                let mut sp: Vec<Pos2> = self.tool.pts.iter().map(|p| self.to_screen(rect, *p)).collect();
                sp.push(sc);
                if sp.len() >= 2 {
                    painter.add(egui::Shape::line(sp, stroke));
                }
            }
            10 => {
                if self.tool.pts.len() == 2 {
                    if let Some((cx, cy, r)) = circumcircle(self.tool.pts[0], self.tool.pts[1], cur) {
                        let scn = self.to_screen(rect, Point2::new(cx, cy));
                        let rp = (self.to_screen(rect, Point2::new(cx + r, cy)).x - scn.x).abs();
                        painter.circle_stroke(scn, rp, stroke);
                    }
                }
            }
            _ => {}
        }
        // under ANY tool, show the automatic constraint that the cursor's snap implies (coincident, on-edge):
        // a click snapped to geometry will attach it through the shared point. The badge sits at the snap point.
        if let Some((g, at)) = self.snap_infer_glyph() {
            let sat = self.to_screen(rect, at) + egui::vec2(11.0, -11.0);
            painter.rect_filled(Rect::from_center_size(sat, egui::vec2(14.0, 14.0)), 2.0, self.scheme.pal.constraint_ok());
            paint_gly(painter, sat, 4.0, g, self.scheme.pal.glyph_text());
        }
    }


    /// Highlighting the selected entities + the glyphs of the geometric constraints.
    pub(super) fn draw_sketch_constraints(&self, painter: &egui::Painter, rect: Rect, si: usize) {
        use qymcad_core::model::EntityKind;
        let Some(s) = self.project.sketches.get(si) else { return };
        let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
        // hovering a constraint (its glyph or its row in the list) lights the points and edges it holds
        if let Some(ci) = self.hover.constraint {
            let pts = self.project.sketch_constraint_points(si, ci);
            let hc = self.scheme.pal.active();
            for id in &pts {
                if let Some(p) = pt(*id) {
                    painter.circle_stroke(self.to_screen(rect, p), 6.0, Stroke::new(1.8, hc));
                }
            }
            // edges with both ends in the set are lit as a line
            for e in &s.entities {
                if let EntityKind::Line { a, b } = e.kind {
                    if pts.contains(&a) && pts.contains(&b) {
                        if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                            painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], Stroke::new(2.0, hc));
                        }
                    }
                }
            }
        }
        // highlighting the selected entities (white) and the hovered ones (blue, thinner)
        for e in &s.entities {
            let sel = self.sel_sk.items.contains(&(1, e.id));
            let hov = self.hover.sketch == Some((1, e.id));
            if !sel && !hov {
                continue;
            }
            let hl = if sel { Stroke::new(2.5, self.scheme.pal.emphasis()) } else { Stroke::new(2.0, self.scheme.pal.preview()) };
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], hl);
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some(c) = pt(center) {
                        let sc = self.to_screen(rect, c);
                        let rp = (self.to_screen(rect, Point2::new(c.x + r, c.y)).x - sc.x).abs();
                        painter.circle_stroke(sc, rp, hl);
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                        let a1 = (pb.y - c.y).atan2(pb.x - c.x);
                        let arc = qymcad_core::geom::tessellate_arc(c.x, c.y, r, a0, a1, ccw, 0.02);
                        let sp: Vec<Pos2> = arc.iter().map(|p| self.to_screen(rect, Point2::new(p.x, p.y))).collect();
                        if sp.len() >= 2 {
                            painter.add(egui::Shape::line(sp, hl));
                        }
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some(pc), Some(pma), Some(pmi)) = (pt(c), pt(ma), pt(mi)) {
                        let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major);
                        let (vx, vy) = (-uy, ux);
                        let n = 72;
                        let sp: Vec<Pos2> = (0..=n)
                            .map(|k| {
                                let t = std::f64::consts::TAU * k as f64 / n as f64;
                                let (ct, st) = (t.cos(), t.sin());
                                self.to_screen(rect, Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy))
                            })
                            .collect();
                        painter.add(egui::Shape::line(sp, hl));
                    }
                }
            }
        }
        // REDUNDANT constraints are marked SPECIFICALLY (an orange-yellow badge); one of them can be deleted
        let diag = self.sketch_diag(si);
        // MARKED BY THE SAME RULE AS THE CONSTRAINT LIST. This used to be the raw `diag.redundant`, and the
        // canvas diverged from the list: on a slot (two semicircles and two tangents) the list was clean while
        // the tangency glyphs burned with "redundant constraint". The rank analysis marks tangencies falsely -
        // their Jacobian at the point of tangency is parallel to the arc's intrinsic.
        let redundant = self.flagged_redundant(si);
        // THE ARGUING SET: the geometry held by conflicting constraints, in red. Otherwise a conflict shows
        // only in the panel and on the dimensions, and which part of the sketch does not solve has to be
        // guessed. Constraints ALWAYS argue several at a time, so the whole set is lit rather than one culprit.
        if !diag.conflicts.is_empty() {
            let cc = self.scheme.pal.error();
            let mut hot: std::collections::HashSet<Id> = std::collections::HashSet::new();
            for &ci in &diag.conflicts {
                hot.extend(self.project.sketch_constraint_points(si, ci));
            }
            for e in &s.entities {
                // an edge wholly inside the set becomes a red line; otherwise only the participating points
                let ends: Vec<Id> = match e.kind {
                    EntityKind::Line { a, b } => vec![a, b],
                    _ => Vec::new(),
                };
                if !ends.is_empty() && ends.iter().all(|id| hot.contains(id)) {
                    if let (Some(pa), Some(pb)) = (pt(ends[0]), pt(ends[1])) {
                        painter.line_segment([self.to_screen(rect, pa), self.to_screen(rect, pb)], Stroke::new(2.2, cc));
                    }
                }
            }
            for id in &hot {
                if let Some(p) = pt(*id) {
                    painter.circle_stroke(self.to_screen(rect, p), 5.0, Stroke::new(2.0, cc));
                }
            }
        }
        for (ci, at, g) in self.visible_constraint_glyphs(rect, si) {
            let bg = if Some(ci) == self.gsel.constraint {
                self.scheme.pal.constraint_selected()
            } else if Some(ci) == self.hover.constraint {
                self.scheme.pal.constraint_hover()
            } else if diag.conflicts.contains(&ci) {
                self.scheme.pal.error() // arguing: red (an error), which outranks redundancy
            } else if redundant.contains(&ci) {
                self.scheme.pal.warning() // redundant: orange-yellow
            } else {
                self.scheme.pal.constraint_ok()
            };
            let r = Rect::from_center_size(at, egui::vec2(15.0, 15.0));
            painter.rect_filled(r, 2.0, bg);
            paint_gly(painter, at, 4.5, g, self.scheme.pal.glyph_text());
        }
    }


    /// A translucent grid in the background + the highlighted axis lines through the origin.
    pub(super) fn draw_sketch_grid(&self, painter: &egui::Painter, rect: Rect) {
        let tl = self.to_world(rect, rect.min);
        let br = self.to_world(rect, rect.max);
        let (x0, x1) = (tl.x.min(br.x), tl.x.max(br.x));
        let (y0, y1) = (tl.y.min(br.y), tl.y.max(br.y));
        // a grid step whose on-screen interval is at least ~8 px
        let mut step = self.set.snap.grid.max(0.1);
        let sc = self.view.scale as f64;
        while step * sc < 8.0 {
            step *= 5.0;
        }
        // THE GRID COMES FROM THE PALETTE. It used to be translucent white: on a light background such a grid cannot be seen at all.
        let g = self.scheme.pal.grid();
        let minor = Stroke::new(1.0, crate::palette::a(g, 40));
        let major = Stroke::new(1.0, crate::palette::a(g, 90));
        let (kx0, kx1) = ((x0 / step).floor() as i64, (x1 / step).ceil() as i64);
        let (ky0, ky1) = ((y0 / step).floor() as i64, (y1 / step).ceil() as i64);
        if (kx1 - kx0) < 600 && (ky1 - ky0) < 600 {
            for k in kx0..=kx1 {
                let x = k as f64 * step;
                let st = if k % 5 == 0 { major } else { minor };
                painter.line_segment([self.to_screen(rect, Point2::new(x, y0)), self.to_screen(rect, Point2::new(x, y1))], st);
            }
            for k in ky0..=ky1 {
                let y = k as f64 * step;
                let st = if k % 5 == 0 { major } else { minor };
                painter.line_segment([self.to_screen(rect, Point2::new(x0, y)), self.to_screen(rect, Point2::new(x1, y))], st);
            }
        }
        // the axis lines are drawn by draw_axes ALONE (nothing is duplicated here)
    }


    /// Highlighting the edges of the reference body in the 2D sketcher - the edges only (no fill, no 3D body).
    pub(super) fn draw_sketch_face_edges(&self, painter: &egui::Painter, rect: Rect) {
        let Some(si) = self.edit_si() else { return };
        let col = crate::palette::a(self.scheme.pal.sketch_face_edge(), 140);
        for poly in self.sketch_ref_edges_2d(si) {
            let pts: Vec<Pos2> = poly.iter().map(|p| self.to_screen(rect, *p)).collect();
            for k in 0..pts.len().saturating_sub(1) {
                painter.line_segment([pts[k], pts[k + 1]], Stroke::new(1.0, col));
            }
        }
    }


    /// WHETHER THE SHARED TOGGLE IS HIDING THE OUTLINES RIGHT NOW.
    ///
    /// It exists for the ASSEMBLY only: there the sketches number in the dozens across all the components and
    /// hiding them one by one is impractical. Inside a Part there is no such toggle - every sketch has a
    /// checkbox of its own there, and a second, shared switch would duplicate it (and once already hid what
    /// had just been asked to be shown).
    ///
    /// The outlines of the sketch CURRENTLY IN HAND are the ones being edited in the sketcher or picked as a
    /// Part command's profile. `None` means neither, and the screen simply shows the model. This lives in one
    /// method because it decides not only the "show only this one" filter but also whether the shared toggle
    /// may hide it - and two places computing the same thing each in their own way drift apart sooner or later.
    pub(super) fn contours_switched_off(&self) -> bool {
        matches!(self.workbench, Workbench::Assembly) && !self.set.show_contours
    }

    pub(super) fn hidden_contour_ids(&self) -> std::collections::HashSet<Id> {
        self.project.sketches.iter().filter(|s| self.sketch_hidden.contains(&s.id)).flat_map(|s| s.contour_ids.iter().copied()).collect()
    }

    pub(super) fn active_sketch_contour_ids(&self) -> Option<std::collections::HashSet<Id>> {
        self.sketch_ses
            .editing
            .and_then(|sid| self.project.sketches.iter().find(|s| s.id == sid))
            .map(|s| s.contour_ids.iter().copied().collect())
            .or_else(|| {
                (self.cmd.active())
                    .then(|| self.cmd.sketch.and_then(|si| self.project.sketches.get(si)))
                    .flatten()
                    .map(|s| s.contour_ids.iter().copied().collect())
            })
    }

    pub(super) fn draw_contours(&self, painter: &egui::Painter, rect: Rect) {
        // while a sketch is being edited, ONLY its outlines are shown (the neighbouring sketches and the
        // part's original one do not get in the way); outside editing, the filter goes by owner (sketch
        // isolation, other components hidden). The half-sketcher of a Part command (feat_cmd, not editing)
        // behaves the same way: the outlines of other sketches are hidden - they get in the way of picking a
        // closed contour.
        let edit_only = self.active_sketch_contour_ids();
        // The sketch being edited is ALWAYS visible: that is why it was opened. The rest go by their own
        // checkbox, and in an assembly by the shared toggle as well.
        let edit_only_none = edit_only.is_none();
        if edit_only_none && self.contours_switched_off() {
            return;
        }
        let hidden_cids = self.hidden_contour_ids();
        // the contours picked by the active operation are brighter
        let selected: &[Id] = self
            .active_op()
            .and_then(|i| self.project.operations.get(i))
            .map(|op| op.selection.as_slice())
            .unwrap_or(&[]);
        let hovered = self.hovered_contour();
        let obj_sel = if let Sel::Contour(i) = self.sel { Some(i) } else { None };
        let base = Stroke::new(1.5, self.scheme.pal.contour_idle());
        let hot = Stroke::new(2.5, self.scheme.pal.active());
        let obj = Stroke::new(2.5, self.scheme.pal.ok());
        let hover = Stroke::new(2.0, self.scheme.pal.contour_hover());
        let foreign_cids = self.foreign_contour_ids();
        // the contour of the active sweep or loft slot (lit in the half-sketcher, like a picked profile)
        let pick_cid = self.picking.contour().map(|s| self.slot_current_cid(s));
        for (i, c) in self.project.contours.iter().enumerate() {
            if c.points.len() < 2 {
                continue;
            }
            let cid = self.project.contour_id(i);
            if edit_only.is_none() && cid.is_some_and(|id| hidden_cids.contains(&id)) {
                continue; // the sketch is hidden by its own checkbox in the tree
            }
            match &edit_only {
                Some(only) if !cid.is_some_and(|id| only.contains(&id)) => continue, // not the current sketch
                None if cid.is_some_and(|id| foreign_cids.contains(&id)) => continue, // another component
                _ => {}
            }
            let mut pts: Vec<Pos2> = c.points.iter().map(|p| self.to_screen(rect, *p)).collect();
            if c.closed {
                pts.push(pts[0]);
            }
            // the profile is picked for a command (extrude, cut, revolve). It is NOT filled translucently:
            // where three or four contours overlapped the fills added up (alpha), everything grew pale and the
            // boundaries were lost. Instead there is a BRIGHT THICK outline - a thickness does not accumulate,
            // so every picked contour stays visible on its own.
            let is_profile = self.cmd.active() && self.project.contour_id(i).is_some_and(|id| self.gsel.profiles.contains(&id) || pick_cid == Some(id));
            let stroke = if is_profile {
                Stroke::new(3.0, self.scheme.pal.contour_profile())
            } else if obj_sel == Some(i) {
                obj
            } else if self.project.contour_id(i).is_some_and(|id| selected.contains(&id)) {
                hot
            } else if hovered == Some(i) {
                hover
            } else {
                base
            };
            painter.add(egui::Shape::line(pts, stroke));
        }
    }


    pub(super) fn draw_toolpath(&self, painter: &egui::Painter, rect: Rect) {
        let Some(prog) = &self.cam_job.program else { return };
        let plunge = Stroke::new(1.4, self.scheme.pal.cam_plunge());
        let rapid = Stroke::new(1.0, self.scheme.pal.cam_rapid());
        let limit = self.progress_limit();
        let mut shown = 0usize;

        for (oi, tp) in prog.toolpaths.iter().enumerate() {
            let cut = Stroke::new(1.5, self.scheme.pal.cam_op(oi));
            let (mut x, mut y) = (0.0_f64, 0.0_f64);
            for m in &tp.moves {
                if shown >= limit {
                    return;
                }
                shown += 1;
                let seg = match m {
                    Move::Rapid { to } => Some((to.x, to.y, rapid, true)),
                    Move::Linear { to, .. } => Some((to.x, to.y, cut, false)),
                    Move::Plunge { to, .. } => Some((to.x, to.y, plunge, false)),
                    Move::Arc { to, .. } | Move::Helix { to, .. } => Some((to.x, to.y, cut, false)),
                    Move::DrillCycle { points, .. } => {
                        for p in points {
                            painter.circle_stroke(self.to_screen(rect, Point2::new(p.x, p.y)), 4.0, plunge);
                        }
                        if let Some(l) = points.last() {
                            x = l.x;
                            y = l.y;
                        }
                        None
                    }
                    _ => None,
                };
                if let Some((nx, ny, stroke, is_rapid)) = seg {
                    if !is_rapid || self.set.show_rapids {
                        painter.line_segment(
                            [self.to_screen(rect, Point2::new(x, y)), self.to_screen(rect, Point2::new(nx, ny))],
                            stroke,
                        );
                    }
                    x = nx;
                    y = ny;
                }
            }
        }
    }
}

/// THE PADDING INSIDE THE REBUILD CARD, and the room its parts take. Named once, because the size of the
/// card and the places of its pieces have to agree - when they did not, the text stood outside the card.
const CARD_PAD: f32 = 16.0;
const CARD_SPINNER: f32 = 34.0; // the ring of dots: radius 13 plus the dots themselves
const CARD_GAP: f32 = 8.0;
const CARD_BUTTON: egui::Vec2 = egui::vec2(150.0, 28.0);
const CARD_MIN_W: f32 = 280.0;

/// Where each piece of the rebuild card goes, once the card itself has been placed.
pub(super) struct RegenCard {
    /// the centre of the ring of dots
    pub spinner: egui::Pos2,
    pub title: egui::Rect,
    pub progress: Option<egui::Rect>,
    pub button: Option<egui::Rect>,
}

/// How big the card has to be to hold what is going into it.
///
/// It used to be a fixed 280x96 (150 with a counter) with the label painted centred inside, which held
/// only as long as the label was short. Reported behaviour: the text runs out past the edges of the
/// rebuild window - the line naming the node count and the thread is a good half wider than 280 px.
pub(super) fn regen_card_size(title: egui::Vec2, progress: Option<egui::Vec2>, button: bool) -> egui::Vec2 {
    let mut inner = egui::vec2(title.x, CARD_SPINNER + CARD_GAP + title.y);
    if let Some(p) = progress {
        inner.x = inner.x.max(p.x);
        inner.y += CARD_GAP * 0.75 + p.y;
    }
    if button {
        inner.x = inner.x.max(CARD_BUTTON.x);
        inner.y += CARD_GAP + CARD_BUTTON.y;
    }
    egui::vec2((inner.x + CARD_PAD * 2.0).max(CARD_MIN_W), inner.y + CARD_PAD * 2.0)
}

/// The pieces laid out top-down inside `card`, each centred across it. Reads the same measurements
/// [`regen_card_size`] reserved room for, so nothing can land outside.
pub(super) fn regen_card_places(card: egui::Rect, title: egui::Vec2, progress: Option<egui::Vec2>, button: bool) -> RegenCard {
    let mid = card.center().x;
    let mut y = card.min.y + CARD_PAD;
    let spinner = egui::pos2(mid, y + CARD_SPINNER / 2.0);
    y += CARD_SPINNER + CARD_GAP;
    let title_rect = egui::Rect::from_min_size(egui::pos2(mid - title.x / 2.0, y), title);
    y = title_rect.max.y;
    let progress = progress.map(|p| {
        y += CARD_GAP * 0.75;
        let r = egui::Rect::from_min_size(egui::pos2(mid - p.x / 2.0, y), p);
        y = r.max.y;
        r
    });
    let button = button.then(|| {
        y += CARD_GAP;
        egui::Rect::from_min_size(egui::pos2(mid - CARD_BUTTON.x / 2.0, y), CARD_BUTTON)
    });
    RegenCard { spinner, title: title_rect, progress, button }
}
