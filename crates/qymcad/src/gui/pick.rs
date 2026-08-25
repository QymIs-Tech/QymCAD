//! PICKING AND HIT TESTING: what is under the cursor.
//!
//! This used to live mixed in with the rendering and the commands in a 27-thousand-line `gui.rs`. Out of
//! that grew the defect with choosing an axis: the click handler ended up in the 2D half of the viewport
//! while the candidates were drawn and hit-tested in 3D — both halves were in one function, and nobody
//! noticed them drifting apart.

use super::*;
use super::grab::Grab;

impl App {
    /// The sketch point of `si` nearest to a screen position (within the threshold) -> its Id.
    pub(super) fn nearest_sketch_point(&self, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, Id)> = None;
        for p in &s.points {
            let d = self.to_screen(rect, Point2::new(p.x, p.y)).distance(screen);
            if d <= self.grab(Grab::Point) && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, p.id));
            }
        }
        best.map(|(_, id)| id)
    }


    /// The line entity nearest to a screen point -> ITS OWN id (not its ends).
    ///
    /// A method of its own precisely because the neighbouring `nearest_line_entity` returns THE ENDS of a
    /// line: while choosing the axis of a revolution the id of a point was compared against a list of
    /// lines, there was never a match, and the half-sketcher opened while not one line could be
    /// selected.
    pub(super) fn nearest_line_id(&self, rect: Rect, pos: Pos2, si: usize, only: &[Id]) -> Option<Id> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, Id)> = None;
        for e in &s.entities {
            if !only.is_empty() && !only.contains(&e.id) {
                continue;
            }
            if let EntityKind::Line { a, b } = e.kind {
                if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    let d = screen_dist_seg(pos, self.to_screen(rect, pa), self.to_screen(rect, pb));
                    if d <= self.grab(Grab::Curve) && best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((d, e.id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }


    /// The line entity nearest to a screen point -> its ends (a, b).
    pub(super) fn nearest_line_entity(&self, rect: Rect, pos: Pos2, si: usize) -> Option<(Id, Id)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        // the hierarchy: an ordinary line outranks a construction one (for dimensions and constraints)
        let mut best: Option<(u8, f32, (Id, Id))> = None;
        for e in &s.entities {
            if let EntityKind::Line { a, b } = e.kind {
                if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    let d = screen_dist_seg(pos, self.to_screen(rect, pa), self.to_screen(rect, pb));
                    if d <= self.grab(Grab::Curve) {
                        let tier = if e.construction { 1u8 } else { 0u8 };
                        if best.map_or(true, |(bt, bd, _)| (tier, d) < (bt, bd)) {
                            best = Some((tier, d, (a, b)));
                        }
                    }
                }
            }
        }
        best.map(|(_, _, ab)| ab)
    }


    /// The vertex (sketch point) nearest to a screen point within the tolerance — for clicking a corner.
    pub(super) fn nearest_vertex(&self, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, Id)> = None;
        for p in &s.points {
            let d = self.to_screen(rect, Point2::new(p.x, p.y)).distance(pos);
            if d <= self.grab(Grab::Point) && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, p.id));
            }
        }
        best.map(|(_, id)| id)
    }


    /// The nearest line entity -> the Id of the entity (for trimming).
    pub(super) fn nearest_line_eid(&self, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, Id)> = None;
        for e in &s.entities {
            if let EntityKind::Line { a, b } = e.kind {
                if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    let d = screen_dist_seg(pos, self.to_screen(rect, pa), self.to_screen(rect, pb));
                    if d <= self.grab(Grab::Curve) && best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((d, e.id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }


    /// The circle or arc entity nearest to a screen point -> its Id. It catches both the outline and the
    /// diameter or radius label (the position of the label is where `draw_sketch_dims` draws it).
    pub(super) fn nearest_circle_entity(&self, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, Id)> = None;
        for e in &s.entities {
            let (center, r, label) = match e.kind {
                EntityKind::Circle { center, r } => {
                    let Some(c) = self.sketch_pt(si, center) else { continue };
                    let sc = self.to_screen(rect, c);
                    let ed = self.to_screen(rect, Point2::new(c.x + r, c.y));
                    // the diameter label sits above the middle of the leader from the centre to the rim
                    (c, r, ((sc.to_vec2() + ed.to_vec2()) / 2.0 + egui::vec2(0.0, -8.0)).to_pos2())
                }
                EntityKind::Arc { center, a, b, .. } => {
                    let (Some(c), Some(pa), Some(pb)) = (self.sketch_pt(si, center), self.sketch_pt(si, a), self.sketch_pt(si, b)) else { continue };
                    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                    // the radius label sits at the rim of the arc towards its middle (as in
                    // `draw_sketch_dims`)
                    let mid = Point2::new((pa.x + pb.x) / 2.0 - c.x, (pa.y + pb.y) / 2.0 - c.y);
                    let ml = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-9);
                    let edge = Point2::new(c.x + mid.x / ml * r, c.y + mid.y / ml * r);
                    (c, r, (self.to_screen(rect, edge).to_vec2() + egui::vec2(10.0, -8.0)).to_pos2())
                }
                _ => continue,
            };
            let sc = self.to_screen(rect, center);
            let rp = (self.to_screen(rect, Point2::new(center.x + r, center.y)).x - sc.x).abs();
            let d_out = (sc.distance(pos) - rp).abs(); // by the outline of the circle
            let d_label = label.distance(pos); // by the diameter or radius label
            let d = d_out.min(d_label);
            if d <= self.grab(Grab::Label) && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, e.id));
            }
        }
        best.map(|(_, id)| id)
    }


    /// Toggle the selection of the edge under the cursor (in 3D) by its PERSISTENT id. `true` means an
    /// edge was hit.
    /// WHICH EDGE IS UNDER THE CURSOR is a question in its own right, apart from any selection.
    ///
    /// Two things ask it: a click (select the edge) and the right button (offer menu items for that edge).
    /// One answer for both — otherwise the menu would offer a chain for one edge while the command took
    /// another.
    pub(super) fn edge_at(&self, rect: Rect, screen: Pos2) -> Option<u32> {
        if self.edges.polys.is_empty() {
            return None;
        }
        let basis = self.cam.basis();
        let wt = self.edges.body.map(|b| self.project.body_display_transform(b, self.current_ctx_id())).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
        let tp = |p: &[f32; 3]| -> [f64; 3] {
            let v = [p[0] as f64, p[1] as f64, p[2] as f64];
            if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
        };
        // ON A COLLINEAR OVERLAP (a long seam edge containing a short one) both project into the same
        // screen line. The first one met used to win (usually the long one), so the short one could not be
        // selected by a click. Now, at an ALMOST equal distance (within 2 px), the SHORT edge is preferred
        // as the more specific one: over the overlapping stretch the short one is chosen, and the long one
        // is available where it is ALONE. Both are reachable.
        let mut best: Option<(f32, f32, usize)> = None; // (the least distance, the screen length, the index)
        for (i, poly) in self.edges.polys.iter().enumerate() {
            let pts: Vec<Pos2> = poly.iter().map(|p| self.project3(tp(p), rect, &basis).0).collect();
            let (mut d, mut slen) = (f32::MAX, 0.0f32);
            for k in 0..pts.len().saturating_sub(1) {
                d = d.min(screen_dist_seg(screen, pts[k], pts[k + 1]));
                slen += pts[k].distance(pts[k + 1]);
            }
            let better = match best {
                None => true,
                Some((bd, bl, _)) => d < bd - 2.0 || (d < bd + 2.0 && slen < bl),
            };
            if better {
                best = Some((d, slen, i));
            }
        }
        let (d, _, i) = best?;
        if d > self.grab(Grab::Curve) {
            return None;
        }
        self.edges.ids.get(i).copied().filter(|id| *id != 0)
    }

    /// THE VERTEX UNDER THE CURSOR among the ends of the SELECTED edges: its descriptor and its point.
    ///
    /// Among the selected ones only — a variable radius makes sense on the set being rounded, and offering
    /// the vertices of the whole part would clutter the aim with what will affect nothing anyway.
    pub(super) fn fillet_vertex_at(&mut self, rect: Rect, screen: Pos2) -> Option<(u32, [f64; 3])> {
        if self.cmd.kind != 4 || self.gsel.edges.is_empty() {
            return None;
        }
        let body = self.edges.body?;
        let picked: Vec<[[f64; 3]; 2]> = self.project.regen_edges.get(&body)?.iter().filter(|e| self.gsel.edges.contains(&e.id)).map(|e| [e.a, e.b]).collect();
        let basis = self.cam.basis();
        let grab = self.grab(Grab::Point);
        let mut best: Option<(f32, u32, [f64; 3])> = None;
        for c in self.project.vertex_pool(body) {
            let on_picked = picked.iter().flatten().any(|p| {
                (p[0] - c.centroid[0]).abs() < 1e-6 && (p[1] - c.centroid[1]).abs() < 1e-6 && (p[2] - c.centroid[2]).abs() < 1e-6
            });
            if !on_picked {
                continue;
            }
            let d = self.project3(c.centroid, rect, &basis).0.distance(screen);
            if d <= grab && best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                best = Some((d, c.desc, c.centroid));
            }
        }
        best.map(|(_, desc, p)| (desc, p))
    }

    /// A CLICK ON A VERTEX IN THE FILLET: create or remove its radius field. `true` means a hit.
    pub(super) fn pick_fillet_vertex(&mut self, rect: Rect, screen: Pos2) -> bool {
        let Some((desc, p)) = self.fillet_vertex_at(rect, screen) else { return false };
        let key = format!("at{desc}");
        if let Some(i) = self.cmd.params.iter().position(|p| p.key == key) {
            self.cmd.params.remove(i);
            self.status = crate::i18n::tr("pk-vertex-radius-off");
        } else {
            let base = self.cmd_val("radius");
            self.cmd.params.push(crate::gui::CmdParam::new("f-radius-at-vertex", &key, base, 0.0, 1000.0).at(p));
            self.status = crate::i18n::tr("pk-vertex-radius-on");
        }
        true
    }

    pub(super) fn pick_edge_3d(&mut self, rect: Rect, screen: Pos2) -> bool {
        // A VERTEX OUTRANKS AN EDGE WHEN IT IS UNDER THE CURSOR: a corner is aimed at in order to set a
        // radius there, and hitting an edge instead would clear the selection rather than fine-tune it.
        if self.pick_fillet_vertex(rect, screen) {
            return true;
        }
        let Some(id) = self.edge_at(rect, screen) else { return false };
        if !self.gsel.edges.insert(id) {
            self.gsel.edges.remove(&id);
        }
        // A SINGLE EDGE BREAKS THE DESCRIPTION: "every edge of the face except this one" is not something
        // we express, and leaving the description as it stands would be a lie.
        self.gsel.described = None;
        self.status = crate::i18n::tr1("pk-edges-selected", "n", &self.gsel.edges.len().to_string());
        true
    }


    /// A click on a FACE of a body in the chamfer or the fillet -> select or clear ALL the edges of that
    /// face (a click on a face of a cube gives its 4 edges). It toggles: if every edge of the face is
    /// already selected they are cleared, otherwise they are added. `true` means a face was hit.
    pub(super) fn pick_face_edges_fillet(&mut self, rect: Rect, pos: Pos2) -> bool {
        let Some(body) = self.edges.body else { return false };
        let Some(fid) = self.pick_face_persist_id(rect, pos).filter(|&f| f != 0) else { return false };
        // THE SECOND SIDE OF A JUNCTION. The junction item of the menu put the command into waiting for a
        // second pick — and here it is. The reference is assembled only now: a junction has two sides and
        // is not described by one.
        if let Some(first) = self.gsel.between_first.take() {
            if fid != first {
                let q = qymcad_core::refs::Query::Between(
                    Box::new(qymcad_core::refs::Query::Id(first)),
                    Box::new(qymcad_core::refs::Query::Id(fid)),
                );
                self.apply_expansion("expand-between-done", q);
                return true;
            }
            // the same face was clicked — there is no junction with itself, so the wait goes on
            self.gsel.between_first = Some(first);
            self.status = crate::i18n::tr("expand-between-pick-second");
            return true;
        }
        let eids = match self.live.shapes.get(&body) {
            Some(shape) => shape.face_edge_ids(fid),
            None => return false,
        };
        // only the edges that really exist in the body are taken (ids from `edge_ids`), so a face of
        // another body will not be caught
        let live: std::collections::HashSet<u32> = self.edges.ids.iter().copied().collect();
        let eids: Vec<u32> = eids.into_iter().filter(|id| live.contains(id)).collect();
        if eids.is_empty() {
            return false;
        }
        let all_sel = eids.iter().all(|id| self.gsel.edges.contains(id));
        for id in &eids {
            if all_sel {
                self.gsel.edges.remove(id);
            } else {
                self.gsel.edges.insert(*id);
            }
        }
        // THE FACE ITSELF IS REMEMBERED rather than only its edges of today: the intention is "this whole
        // rim", and after an edit that added edges it must stay true.
        if all_sel {
            self.gsel.described = None; // the face was cleared, so there is no description any more
            self.gsel.last_face = None;
        } else {
            self.gsel.describe_edges_of_face(fid);
            self.gsel.last_face = Some((fid, body)); // the expand-the-selection menu will ask about it
            self.gsel.last_edge = None; // a face was asked about, not an edge
        }
        self.status = crate::i18n::trn("pk-face-edges", &[("n", &eids.len().to_string()), ("what", &if all_sel { crate::i18n::tr("pk-removed") } else { crate::i18n::tr("pk-added") }), ("total", &self.gsel.edges.len().to_string())]);
        true
    }


    /// Find the INDEX of a face `(mi, fi)` of `body` by the key `key`: first by the PERSISTENT id,
    /// otherwise by a fallback match — an aligned normal plus the nearest centre (as `Project::resolve_face`
    /// does, but returning an index for `Sel::Face`). Needed to restore the selection of a face when the
    /// shell or the hole is reopened.
    pub(super) fn resolve_face_sel(&self, body: Id, key: &qymcad_core::feature::FaceKey) -> Option<(usize, usize)> {
        let mi = self.project.mesh_index(body)?;
        let faces = &self.project.bodies.get(mi)?.faces;
        if key.id != 0 {
            if let Some(fi) = faces.iter().position(|f| f.id == key.id) {
                return Some((mi, fi));
            }
        }
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let d2 = |c: &qymcad_core::geom::Point3| (c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2);
        let fi = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| dot(f.normal, key.normal) > 0.9)
            .min_by(|(_, a), (_, b)| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)?;
        Some((mi, fi))
    }


    /// The circle or arc that the segment p1-p2 is ALMOST TANGENT to (the distance from the centre to the
    /// line is about the radius, and the point of tangency lies inside the segment). Returns (the Id of the
    /// centre, the radius). For the automatic tangency.
    pub(super) fn nearest_tangent_circle(&self, si: usize, p1: Point2, p2: Point2) -> Option<(Id, f64)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return None;
        }
        let mut best: Option<(f64, (Id, f64))> = None;
        for e in &s.entities {
            let (center, r) = match e.kind {
                EntityKind::Circle { center, r } => (center, r),
                EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                    (Some((cx, cy)), Some((ax, ay))) => (center, ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                    _ => continue,
                },
                _ => continue,
            };
            let Some((cx, cy)) = pt(center) else { continue };
            // the signed perpendicular distance from the centre to the line, plus where the projection
            // falls on the segment
            let dist = ((dx * (cy - p1.y) - dy * (cx - p1.x)) / len).abs();
            let t = ((cx - p1.x) * dx + (cy - p1.y) * dy) / (len * len);
            let err = (dist - r).abs();
            if err < 0.06 * r && t > -0.1 && t < 1.1 && best.map_or(true, |(be, _)| err < be) {
                best = Some((err, (center, r)));
            }
        }
        best.map(|(_, v)| v)
    }


    /// The ends of an ADJACENT line (sharing an end with p1 or p2) whose length is about the length of the
    /// segment p1-p2 — for the automatic equal-length constraint. The requirement of adjacency plus a hard
    /// tolerance of 2% guard against false matches of length with distant unrelated geometry. The line's
    /// own ends (ea, eb) are excluded.
    pub(super) fn nearest_equal_line(&self, si: usize, p1: Point2, p2: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ln = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
        if ln < 1e-3 {
            return None;
        }
        let near = |x: f64, y: f64, p: Point2| (x - p.x).abs() < 1e-4 && (y - p.y).abs() < 1e-4;
        let mut best: Option<(f64, (Id, Id))> = None;
        for e in &s.entities {
            let EntityKind::Line { a, b } = e.kind else { continue };
            if (a == ea && b == eb) || (a == eb && b == ea) {
                continue;
            }
            let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) else { continue };
            // adjacency: an end shared with the new segment (by position, which works in the preview too)
            let adj = near(ax, ay, p1) || near(ax, ay, p2) || near(bx, by, p1) || near(bx, by, p2);
            if !adj {
                continue;
            }
            let le = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            if le < 1e-3 {
                continue;
            }
            let rel = (ln - le).abs() / ln.max(le);
            if rel < 0.02 && best.map_or(true, |(br, _)| rel < br) {
                best = Some((rel, (a, b)));
            }
        }
        best.map(|(_, v)| v)
    }


    /// The ends of the nearest NON-axis line almost parallel to the segment p1-p2 (for the automatic
    /// parallel constraint).
    pub(super) fn nearest_parallel_line(&self, si: usize, p1: Point2, p2: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
        let lv = (vx * vx + vy * vy).sqrt();
        if lv < 1e-6 {
            return None;
        }
        let mut best: Option<(f64, (Id, Id))> = None;
        for e in &s.entities {
            let EntityKind::Line { a, b } = e.kind else { continue };
            if (a == ea && b == eb) || (a == eb && b == ea) {
                continue;
            }
            let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { continue };
            let (ux, uy) = (pb.x - pa.x, pb.y - pa.y);
            let lu = (ux * ux + uy * uy).sqrt();
            if lu < 1e-6 {
                continue;
            }
            // non-axis (otherwise the horizontal or vertical constraint fires) and almost parallel
            let axis = (ux.abs() <= uy.abs() * 0.06) || (uy.abs() <= ux.abs() * 0.06);
            let cross = (ux * vy - uy * vx).abs() / (lu * lv);
            if !axis && cross < 0.06 && best.map_or(true, |(bc, _)| cross < bc) {
                best = Some((cross, (a, b)));
            }
        }
        best.map(|(_, e)| e)
    }


    /// Choose a font of one's own (TTF or OTF) — the bytes go into the cache.
    pub(super) fn pick_font(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter(&crate::i18n::tr("pk-font"), &["ttf", "otf", "TTF", "OTF"]).pick_file() {
            match std::fs::read(&p) {
                Ok(b) => {
                    self.font_cache = Some(b);
                    self.status = crate::i18n::tr1("pk-font-is", "name", &p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
                }
                Err(e) => self.status = crate::i18n::tr1("pk-font-error", "error", &e.to_string()),
            }
        }
    }


    pub(super) fn pick_dxf(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("DXF", &["dxf"]).pick_file() {
            self.open_dxf(p.to_string_lossy().into_owned());
        }
    }

    pub(super) fn pick_stl(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("STL", &["stl"]).pick_file() {
            self.open_stl(p.to_string_lossy().into_owned());
        }
    }

    pub(super) fn pick_step(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("STEP", &["step", "stp"]).pick_file() {
            self.open_step(p.to_string_lossy().into_owned());
        }
    }

    pub(super) fn pick_svg(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).pick_file() {
            let path = p.to_string_lossy().into_owned();
            match import_svg(&path) {
                Ok(sk) => self.arm_sketch_import(sk.curves, &path),
                Err(e) => self.status = crate::i18n::tr1("pk-svg-error", "error", &e.to_string()),
            }
        }
    }


    /// The vertex (an end of an edge) under the cursor among ALL the visible bodies -> (the body, the id
    /// of the edge, which end).
    pub(super) fn pick_vertex_any(&self, rect: Rect, pos: Pos2) -> Option<(Id, u32, bool)> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, Id, u32, bool)> = None;
        for (_mi, body) in self.shown_bodies() {
            // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
            // worth walking
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let (polys, ids) = (&edges.0, &edges.1);
            let wt = self.project.body_display_transform(body, ctx);
            let tp = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
            };
            for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                if id == 0 || poly.len() < 2 {
                    continue;
                }
                for (end, vert) in [(false, &poly[0]), (true, &poly[poly.len() - 1])] {
                    let d = self.project3(tp(vert), rect, &basis).0.distance(pos);
                    if best.map_or(true, |(bd, _, _, _)| d < bd) {
                        best = Some((d, body, id, end));
                    }
                }
            }
        }
        best.filter(|(d, _, _, _)| *d <= self.grab(Grab::Point)).map(|(_, b, id, e)| (b, id, e))
    }


    /// The WORLD COORDINATE of the nearest vertex (an end of an edge) of a visible body under the cursor —
    /// for snapping a datum point.
    pub(super) fn pick_vertex_pos(&self, rect: Rect, pos: Pos2) -> Option<[f64; 3]> {
        use qymcad_core::feature::{apply12, is_identity12};
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, [f64; 3])> = None;
        for (_mi, body) in self.shown_bodies() {
            // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
            // worth walking
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let (polys, ids) = (&edges.0, &edges.1);
            let wt = self.project.body_display_transform(body, ctx);
            let tp = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if is_identity12(&wt) { v } else { apply12(&wt, v) }
            };
            for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                if id == 0 || poly.len() < 2 {
                    continue;
                }
                for vert in [&poly[0], &poly[poly.len() - 1]] {
                    let w = tp(vert);
                    let d = self.project3(w, rect, &basis).0.distance(pos);
                    if best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((d, w));
                    }
                }
            }
        }
        best.filter(|(d, _)| *d <= self.grab(Grab::Point)).map(|(_, w)| w)
    }


    /// The DATUM POINT under the cursor -> (its Id, its world position). For a two-point axis (kept
    /// parametric through `TwoPoints`).
    pub(super) fn pick_datum_point_at(&self, rect: Rect, pos: Pos2) -> Option<(Id, [f64; 3])> {
        use qymcad_core::feature::apply12;
        let basis = self.cam.basis();
        let mut best: Option<(f32, Id, [f64; 3])> = None;
        for d in &self.project.datum_points {
            let Some(wt) = self.datum_render_transform(d.id) else { continue };
            let w = apply12(&wt, d.at);
            let dist = self.project3(w, rect, &basis).0.distance(pos);
            if best.map_or(true, |(bd, _, _)| dist < bd) {
                best = Some((dist, d.id, w));
            }
        }
        best.filter(|(dist, _, _)| *dist <= self.grab(Grab::Point)).map(|(_, id, w)| (id, w))
    }


    /// The edge under the cursor among ALL the visible bodies (for picking the axis of a connector) -> (the
    /// body, the persistent id of the edge).
    pub(super) fn pick_edge_any(&self, rect: Rect, pos: Pos2) -> Option<(Id, u32)> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, Id, u32)> = None;
        for (_mi, body) in self.shown_bodies() {
            // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
            // worth walking
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let (polys, ids) = (&edges.0, &edges.1);
            let wt = self.project.body_display_transform(body, ctx);
            let tp = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) }
            };
            for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                if id == 0 {
                    continue;
                }
                let sp: Vec<Pos2> = poly.iter().map(|p| self.project3(tp(p), rect, &basis).0).collect();
                for k in 0..sp.len().saturating_sub(1) {
                    let d = screen_dist_seg(pos, sp[k], sp[k + 1]);
                    if best.map_or(true, |(bd, _, _)| d < bd) {
                        best = Some((d, body, id));
                    }
                }
            }
        }
        best.filter(|(d, _, _)| *d <= self.grab(Grab::Curve)).map(|(_, b, id)| (b, id))
    }


    /// Picking the axis of a circular array or of a datum axis by a click: the nearest DATUM AXIS (a line)
    /// OR a STRAIGHT edge of ANY visible body (`axis_edges`) within the threshold, otherwise the axis of the
    /// cylindrical face under the cursor.
    pub(super) fn pick_axis_at(&self, rect: Rect, screen: Pos2) -> Option<AxisHit> {
        use qymcad_core::feature::apply12;
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, AxisHit)> = None;
        // 1) the datum axes that exist
        for d in &self.project.datum_axes {
            if let Some(wt) = self.datum_render_transform(d.id) {
                let (s, e) = axis_segment(d.origin(), d.dir(), 45.0);
                let a = self.project3(apply12(&wt, s), rect, &basis).0;
                let b = self.project3(apply12(&wt, e), rect, &basis).0;
                let dist = screen_dist_seg(screen, a, b);
                if dist <= self.grab(Grab::Curve) && best.map_or(true, |(bd, _)| dist < bd) {
                    best = Some((dist, AxisHit::Datum(d.id)));
                }
            }
        }
        // 2) the straight edges of every visible body (`axis_edges`, each with its own display transform)
        for (i, (body, _id, poly)) in self.edges.axes.iter().enumerate() {
            let wt = self.project.body_display_transform(*body, ctx);
            let pts: Vec<Pos2> = poly.iter().map(|p| self.project3(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64]), rect, &basis).0).collect();
            for k in 0..pts.len().saturating_sub(1) {
                let dist = screen_dist_seg(screen, pts[k], pts[k + 1]);
                if dist <= self.grab(Grab::Curve) && best.map_or(true, |(bd, _)| dist < bd) {
                    best = Some((dist, AxisHit::Edge(i)));
                }
            }
        }
        // LINES (a datum or an edge) are more precise — if one was hit it is taken; otherwise the
        // CYLINDRICAL face under the cursor is tried
        if let Some((_, h)) = best {
            return Some(h);
        }
        self.pick_cyl_face_axis_at(rect, screen)
    }


    /// The axis of the CYLINDRICAL or conical face under the cursor (for picking the axis of a circular
    /// array by a click): the nearest visible face by depth for which OCCT gives an axis
    /// (`Shape::face_axis`). Planar and spline faces are passed over.
    pub(super) fn pick_cyl_face_axis_at(&self, rect: Rect, screen: Pos2) -> Option<AxisHit> {
        use qymcad_core::feature::{apply12, is_identity12};
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let consumed = self.consumed_bodies();
        let edit_hide = self.edit_hidden_bodies();
        let edit_src = if !edit_hide.is_empty() { self.edit_src_body() } else { None };
        let mut best: Option<(f64, Id, u32)> = None; // (the depth, the body, the id of the face)
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            let bid = self.project.mesh_id(mi);
            if bid.is_some_and(|b| edit_hide.contains(&b)) || bid.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
                continue;
            }
            let wt = bid.map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if is_identity12(&wt) { v } else { apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.map_or(true, |(bd, _, _)| depth < bd) {
                        if let (Some(b), Some(fi)) = (bid, self.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32))))) {
                            let fid = self.project.bodies[mi].faces[fi].id;
                            // only a cylinder or a cone has an axis in OCCT; otherwise the face is passed
                            // over as a candidate
                            if fid != 0 && self.live.shapes.get(&b).and_then(|s| s.face_axis(fid)).is_some() {
                                best = Some((depth, b, fid));
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, b, fid)| AxisHit::Face(b, fid))
    }


    /// The full LOCAL-TO-WORLD 3x4 frame for placing a primitive under the cursor: first an exact SNAP (the
    /// vertex of an edge or a datum point) — a translation only, with no rotation (the axes of the world);
    /// otherwise the intersection of the cursor ray with the CHOSEN plane (a face of a body, a base
    /// XY/XZ/YZ plane or a datum): the origin is the point ON the plane under the cursor, and the local +Z
    /// is oriented along the normal (the base of the primitive sits ON the surface). `None` means a miss.
    pub(super) fn pick_place_frame_at(&self, rect: Rect, screen: Pos2) -> Option<[f64; 12]> {
        use qymcad_core::feature::{apply12, apply12_dir, SketchPlane};
        let translate = |w: [f64; 3]| [1.0, 0.0, 0.0, w[0], 0.0, 1.0, 0.0, w[1], 0.0, 0.0, 1.0, w[2]];
        if let Some(w) = self.pick_vertex_pos(rect, screen) {
            return Some(translate(w)); // a snap to the vertex of an edge has the highest priority
        }
        if let Some((_, w)) = self.pick_datum_point_at(rect, screen) {
            return Some(translate(w)); // a snap to a datum point
        }
        // the plane under the cursor -> its world point and normal -> the intersection with the cursor ray
        let (p0, n) = match self.pick_sketch_plane_at(rect, screen)? {
            SketchPlane::World(bp) => {
                let f = bp.frame();
                (f.origin, f.normal())
            }
            SketchPlane::Datum(id) => {
                let p = self.project.planes.iter().find(|p| p.id == id)?;
                (p.origin, p.normal)
            }
            SketchPlane::Face(body, key) => {
                let wt = self.project.body_display_transform(body, self.current_ctx_id());
                (apply12(&wt, key.centroid), apply12_dir(&wt, key.normal))
            }
        };
        let (o, d) = self.screen_ray(rect, screen);
        let origin = ray_plane(o, d, p0, n)?;
        // the orthonormal basis of the plane: the columns of the transform are the images of the local
        // X (u), Y (v) and Z (n, the normal)
        let nn = v_norm(n);
        let a = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = v_norm(v_cross(a, nn));
        let vv = v_cross(nn, u);
        Some([u[0], vv[0], nn[0], origin[0], u[1], vv[1], nn[1], origin[1], u[2], vv[2], nn[2], origin[2]])
    }


    /// Picking the target of a thread — a cylindrical face gives the source body plus the circular rim edge
    /// (the axis and the radius by fact), plus a heuristic for inner or outer (by the direction of the
    /// normal relative to the axis).
    pub(super) fn pick_thread_target(&mut self, rect: Rect, pos: Pos2) {
        let Some(AxisHit::Face(body, fid)) = self.pick_axis_at(rect, pos) else {
            self.status = crate::i18n::tr("pk-miss-cylinder");
            return;
        };
        // the edges of this face -> the circular rims among the `regen_edges` of the body. A cylinder has
        // TWO rims (top and bottom) — the one NEAREST to the point of the click is taken (a defect: `.find`
        // used to take an arbitrary one, so a thread began at a random end). That way the SIDE OF ENTRY is
        // chosen by the click: click near the end you want and the thread runs from there.
        let eids = self.live.shapes.get(&body).map(|s| s.face_edge_ids(fid)).unwrap_or_default();
        let basis = self.cam.basis();
        let wt = self.project.body_display_transform(body, self.current_ctx_id());
        // THE RADIUS IS TAKEN FROM THE FACE ITSELF rather than from the nearest rim. If a chamfer has been
        // cut at the end, the edges of a cylindrical face include rims of DIFFERENT radii, and "the nearest
        // to the click" gives the wrong one — the thread is built on the wrong surface, and the report was
        // that a thread cannot be drawn where there is a chamfer. This also catches a click on the chamfer
        // itself: on a cone the radius changes along the axis and the spread is large.
        let face_tris = self
            .project
            .mesh_index(body)
            .and_then(|mi| self.project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.id == fid)).map(|f| (mi, f.triangles.clone())));
        let candidates: Vec<_> = self
            .project
            .regen_edges
            .get(&body)
            .map(|es| es.iter().filter(|e| e.is_circular() && eids.contains(&e.id)).cloned().collect())
            .unwrap_or_else(Vec::new);
        let face_r = face_tris.as_ref().and_then(|(mi, tris)| {
            candidates.first().and_then(|e| qymcad_core::geom::cyl_face_radius(&self.project.bodies[*mi].mesh, tris, e.center, e.axis))
        });
        if let Some((_, spread)) = face_r {
            if spread > 0.08 {
                self.status = crate::i18n::tr("pk-not-a-cylinder");
                return;
            }
        }
        let circ = candidates
            .iter()
            .filter(|e| face_r.map(|(m, _)| (e.radius - m).abs() < 0.05 * m.max(1e-6)).unwrap_or(true))
            .min_by(|a, b| {
                let da = self.project3(qymcad_core::feature::apply12(&wt, a.center), rect, &basis).0.distance(pos);
                let db = self.project3(qymcad_core::feature::apply12(&wt, b.center), rect, &basis).0.distance(pos);
                da.total_cmp(&db)
            })
            .map(|e| (e.id, e.center, e.axis, e.radius));
        let Some((eid, center, mut axis, radius)) = circ else {
            self.status = crate::i18n::tr("pk-no-round-rim");
            return;
        };
        // The axis is turned ALONG THE CHOSEN FACE: a thread runs where the cylinder itself lies. Computing
        // it over the whole mesh ("where there are more vertices") will not do — with a chamfer at the end
        // the rim ends up at its base, and on a part such as a boss the thread ran INTO THE AIR, towards the
        // end. If the face is unavailable, the former way over the mesh remains.
        match face_tris.as_ref() {
            Some((mi, tris)) => axis = qymcad_core::geom::axis_along_face(&self.project.bodies[*mi].mesh, tris, center, axis),
            None => {
                if let Some(mi) = self.project.mesh_index(body) {
                    axis = qymcad_core::model::orient_axis_into_mesh(center, axis, &self.project.bodies[mi].mesh.verts);
                }
            }
        }
        self.thread.src = Some(body);
        self.thread.edge = eid;
        self.thread.axis = (center, axis);
        self.thread.radius = radius;
        self.thread.internal = self.cyl_face_is_internal(body, fid, center, axis);
        // the size comes FROM THE GEOMETRY — all that is left is choosing a standard (the nominal is
        // already filled in)
        if self.cmd.edit.is_none() {
            self.set_thread_params();
        }
        self.status = crate::i18n::trn(
            "pk-thread-target",
            &[
                ("what", &if self.thread.internal { crate::i18n::tr("pk-hole") } else { crate::i18n::tr("pk-cylinder") }),
                ("d", &crate::i18n::num(radius * 2.0, 1)),
                ("next", &if self.thread.auger { crate::i18n::tr("pk-set-flight") } else { crate::i18n::tr("pk-pick-standard") }),
            ],
        );
    }


    /// What is under the cursor while a sketch plane is being chosen (the nearest by depth): a world or
    /// datum plane, or a FACE of a part. Returns a `SketchPlane`.
    /// THE FACE OF A VISIBLE PART UNDER THE CURSOR and its depth — the part shared by two resolutions.
    ///
    /// It became a door of its own for this reason. When a sketch plane is being chosen, a face COMPETES
    /// with a base plane, and there a comparison by depth is right. The assembly tools have nothing to
    /// compete with: they want a part and only a part. While there was one door, collecting an anchor got
    /// a base plane and declared a miss — see `a_mate_takes_the_face_you_click.rs`.
    fn face_under_cursor(&self, rect: Rect, screen: Pos2) -> Option<(f64, qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
        use qymcad_core::feature::FaceKey;
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        // ONLY WHAT IS DRAWN gets picked: the consumed bodies are skipped (except the source of an edit)
        // along with the result of the feature being edited — otherwise stale geometry is caught (the faces
        // of a hidden body hanging in empty space where it stood before the move).
        let consumed = self.consumed_bodies();
        let edit_hide = self.edit_hidden_bodies();
        let edit_src = if !edit_hide.is_empty() { self.edit_src_body() } else { None };
        let mut face_best: Option<(f64, qymcad_core::model::Id, FaceKey)> = None;
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            // GHOSTS (the neighbouring components shown translucently in context, for reference) are NOT
            // picked ONLY in the mirror and section modes: the plane of a mirror or a section must not be
            // taken by accident from a part those tools logically cannot reach.
            // For an ORDINARY pick of a sketch plane a ghost stays pickable — working in context exists
            // precisely for that ("a sketch on the face of a neighbour gives an external reference"). The
            // filter was once applied unconditionally and broke the top-down associative sketch on a
            // neighbouring part.
            if (self.mirror.part.is_some() || self.section.pick) && self.body_is_ghost(mi) {
                continue;
            }
            let bid = self.project.mesh_id(mi);
            if bid.is_some_and(|b| edit_hide.contains(&b)) || bid.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
                continue; // a hidden body (consumed, or the result of an edit) — its faces are not picked
            }
            // a face is hit-tested where it is drawn; the centroid and normal in a `FaceKey` are LOCAL (for
            // `resolve_face`)
            let wt = self.project.mesh_id(mi).map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                // A BACK-FACING triangle, as in `rasterize_3d`: a wrapping face (a cylinder) gives half
                // its triangles facing away from the camera, and those must not be picked (they are
                // invisible, and picking them punches a hole in the picture of the body)
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue;
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if face_best.as_ref().map_or(true, |(bd, _, _)| depth < *bd) {
                        if let Some(fi) = self.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32)))) {
                            let face = &self.project.bodies[mi].faces[fi];
                            let body = self.project.mesh_id(mi).unwrap_or(0);
                            let key = FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };
                            face_best = Some((depth, body, key));
                        }
                    }
                }
            }
        }
        face_best
    }

    /// THE FACE OF A PART UNDER THE CURSOR — a base plane does NOT intercept it.
    ///
    /// The assembly tools go by this resolution: collecting an anchor, editing an anchor, the secondary
    /// axis, the tangency, the width. All of them want a face of a part, and a miss must mean emptiness
    /// under the cursor rather than an invisible square a third of the scene across turning out to be
    /// nearer.
    pub(super) fn pick_part_face_at(&self, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
        self.face_under_cursor(rect, screen).map(|(_, b, k)| (b, k))
    }

    pub(super) fn pick_sketch_plane_at(&self, rect: Rect, screen: Pos2) -> Option<qymcad_core::feature::SketchPlane> {
        use qymcad_core::feature::SketchPlane;
        let basis = self.cam.basis();
        let face_best: Option<(f64, SketchPlane)> = self.face_under_cursor(rect, screen).map(|(d, b, k)| (d, SketchPlane::Face(b, k)));
        // A base plane (XY/XZ/YZ or a datum) is compared with a face BY DEPTH rather than "a face always
        // wins". A plane (a fixed 60 mm square) used to be blocked SOLIDLY by any body under the cursor
        // (`body_hit`, even when the hit did not resolve into a valid face) — and at the enlarged,
        // scale-dependent size that would have blocked the plane almost everywhere a body appears on
        // screen. An honest comparison of depth also removes the flicker at the silhouette of a body — it
        // was reported as planes flashing yellow for a second and vanishing, with a shift of one pixel
        // curing it: `body_hit` used to jump between true and false at the boundary, while depth changes
        // continuously.
        let h = self.plane_pick_half_size();
        let mut plane_best: Option<(f64, SketchPlane)> = None;
        for (sp, fr) in self.sketch_plane_candidates() {
            let corners = [fr.lift(Point2::new(-h, -h)), fr.lift(Point2::new(h, -h)), fr.lift(Point2::new(h, h)), fr.lift(Point2::new(-h, h))];
            let pr: Vec<(Pos2, f64)> = corners.iter().map(|p| self.project3([p.x, p.y, p.z], rect, &basis)).collect();
            if point_in_tri(screen, pr[0].0, pr[1].0, pr[2].0) || point_in_tri(screen, pr[0].0, pr[2].0, pr[3].0) {
                let depth = pr.iter().map(|(_, d)| *d).sum::<f64>() / 4.0;
                if plane_best.map_or(true, |(bd, _)| depth < bd) {
                    plane_best = Some((depth, sp));
                }
            }
        }
        match (face_best, plane_best) {
            (Some((fd, fsp)), Some((pd, psp))) => Some(if fd <= pd { fsp } else { psp }),
            (Some((_, fsp)), None) => Some(fsp),
            (None, Some((_, psp))) => Some(psp),
            (None, None) => None,
        }
    }


    /// The nearest WORLD point on an edge of a visible body to the cursor (within the grab). It
    /// complements `pick_vertex_pos` (the vertices) — the origin of a sketch snaps not only to corners but
    /// to any point on an edge.
    pub(super) fn pick_edge_point(&self, rect: Rect, pos: Pos2) -> Option<[f64; 3]> {
        use qymcad_core::feature::{apply12, is_identity12};
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, [f64; 3])> = None;
        for (_mi, body) in self.shown_bodies() {
            // a cull by the bounding box: a body whose screen rectangle does not cover the cursor is not
            // worth walking
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let (polys, ids) = (&edges.0, &edges.1);
            let wt = self.project.body_display_transform(body, ctx);
            let tp = |p: &[f32; 3]| -> [f64; 3] {
                let v = [p[0] as f64, p[1] as f64, p[2] as f64];
                if is_identity12(&wt) { v } else { apply12(&wt, v) }
            };
            for (poly, id) in polys.iter().zip(ids.iter().copied()) {
                if id == 0 || poly.len() < 2 {
                    continue;
                }
                for w in poly.windows(2) {
                    let (a3, b3) = (tp(&w[0]), tp(&w[1]));
                    let pa = self.project3(a3, rect, &basis).0;
                    let pb = self.project3(b3, rect, &basis).0;
                    let ab = pb - pa;
                    let len2 = ab.length_sq();
                    let t = if len2 > 1e-6 { ((pos - pa).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let d = (pa + ab * t).distance(pos);
                    if best.map_or(true, |(bd, _)| d < bd) {
                        let td = t as f64;
                        best = Some((d, [a3[0] + (b3[0] - a3[0]) * td, a3[1] + (b3[1] - a3[1]) * td, a3[2] + (b3[2] - a3[2]) * td]));
                    }
                }
            }
        }
        best.filter(|(d, _)| *d <= self.grab(Grab::Point)).map(|(_, w)| w)
    }


    /// The mesh index of the VISIBLE body under the cursor (the topmost by depth) — for picking an operand
    /// of the boolean of bodies. Only what is drawn is picked (not the consumed or hidden ones), as in
    /// `pick_face_3d`.
    pub(super) fn pick_body_at(&self, rect: Rect, screen: Pos2) -> Option<usize> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let consumed = self.consumed_bodies();
        let mut best: Option<(f64, usize)> = None;
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            let b = self.project.mesh_id(mi);
            if b.is_some_and(|b| consumed.contains(&b)) {
                continue;
            }
            let wt = b.map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                // A BACK-FACING triangle, as in `rasterize_3d`
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue;
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.map_or(true, |(bd, _)| depth < bd) {
                        best = Some((depth, mi));
                    }
                }
            }
        }
        best.map(|(_, mi)| mi)
    }


    /// The PERSISTENT id of the face under the cursor among the bodies THAT ARE DRAWN (the same logic of
    /// visibility as `pick_face_3d`). For choosing the reference face of a chamfer by hand. `None` means a
    /// miss, or that the triangle has no face.
    pub(super) fn pick_face_persist_id(&self, rect: Rect, screen: Pos2) -> Option<u32> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let consumed = self.consumed_bodies();
        let edit_hide = self.edit_hidden_bodies();
        let edit_src = if !edit_hide.is_empty() { self.edit_src_body() } else { None };
        let mut best: Option<(f64, usize, usize)> = None;
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            let b = self.project.mesh_id(mi);
            if b.is_some_and(|b| edit_hide.contains(&b)) || b.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
                continue;
            }
            let wt = self.project.mesh_id(mi).map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                // A BACK-FACING triangle, as in `rasterize_3d`
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue;
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.map_or(true, |(bd, _, _)| depth < bd) {
                        best = Some((depth, mi, ti));
                    }
                }
            }
        }
        let (_, mi, ti) = best?;
        self.project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.triangles.contains(&(ti as u32)))).map(|f| f.id)
    }


    /// A RAY INTO A FACE WITH NO SIDE EFFECTS: (the body, the persistent id of the face, the point of the
    /// hit in the world).
    ///
    /// Besides searching, `pick_face_3d` also CHANGES the selection, the set of faces of a command and the
    /// highlight — the measuring tool needs none of that and is harmed by it: measure a gap and lose the
    /// selection of the part. The search is the same, only clean.
    pub(super) fn pick_face_ray(&self, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, u32, [f64; 3])> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let consumed = self.consumed_bodies();
        let mut best: Option<(f64, usize, usize, [f64; 3])> = None;
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            let b = self.project.mesh_id(mi);
            if b.is_some_and(|b| consumed.contains(&b)) {
                continue;
            }
            let wt = b.map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue;
                }
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue; // a back-facing face is not the one under the cursor
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.is_none_or(|(bd, _, _, _)| depth < bd) {
                        let hit = [(wa[0] + wb[0] + wc[0]) / 3.0, (wa[1] + wb[1] + wc[1]) / 3.0, (wa[2] + wb[2] + wc[2]) / 3.0];
                        best = Some((depth, mi, ti, hit));
                    }
                }
            }
        }
        let (_, mi, ti, hit) = best?;
        let body = self.project.mesh_id(mi)?;
        let fid = self.project.bodies.get(mi)?.faces.iter().find(|f| f.triangles.contains(&(ti as u32)))?.id;
        Some((body, fid, hit))
    }

    pub(super) fn pick_face_3d(&mut self, rect: Rect, screen: Pos2) {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        // ONLY WHAT IS DRAWN gets picked: bodies consumed by modifiers (the hidden old extrusion under the
        // final one) and the result of the feature being edited are NOT picked, otherwise a click lands in
        // a hidden body — the defect where one place gave three different selections. The criterion is the
        // same as in the rendering.
        let consumed = self.consumed_bodies();
        let edit_hide = self.edit_hidden_bodies();
        let edit_src = if !edit_hide.is_empty() { self.edit_src_body() } else { None };
        // A TOOL FOR BODIES DOES NOT CATCH ON A SURFACE. A copy of a face lies EXACTLY ON the face of the
        // part, and a click there landed on the sheet: the shell got a face of a surface and honestly
        // refused (`OpFailed(Shell)`), while what was seen was "I clicked the face of the part and it does
        // not work". The tools for which a sheet is lawful (thicken, replace a face, stitch, trim, patch,
        // copy a face) are not included here — a surface is what they want.
        let solid_only = matches!(self.cmd.kind, 4 | 5 | 6 | 7 | 23 | 25 | 26 | 27 | 29);
        let mut best: Option<(f64, usize, usize)> = None; // (the depth, the mesh, the triangle)
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !self.body_shown(mi) {
                continue;
            }
            if solid_only && self.project.bodies[mi].sheet {
                continue;
            }
            // ...AND NOT ON A NEIGHBOURING PART EITHER. While working in context the bodies of the
            // neighbours are shown as ghosts, so that their geometry can be REFERRED to (a sketch on the
            // face of a neighbour, top-down). But a tool editing THE CURRENT part cannot take them as a
            // target: isolation rejects such a reference, the node rolls back, and what is seen is "I
            // clicked and nothing happened" — with no reason and no red. The worst kind of breakage: no
            // trace is left at all.
            if solid_only {
                let mine = self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b)).is_some_and(|o| o == ctx || self.project.component_is_within(o, ctx));
                if !mine {
                    continue;
                }
            }
            let b = self.project.mesh_id(mi);
            if b.is_some_and(|b| edit_hide.contains(&b)) || b.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
                continue; // a hidden body is not picked
            }
            // a body is picked where it is drawn — in the frame of the active context
            let wt = self.project.mesh_id(mi).map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if self.section_tri_hidden(wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                // A BACK-FACING triangle, as in `rasterize_3d`. Reported behaviour: a click on a wheel
                // selected a part ten centimetres away from it — a wrapping face gives a back-facing
                // triangle a wrong depth
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue;
                }
                let (pa, da) = self.project3(wa, rect, &basis);
                let (pb, db) = self.project3(wb, rect, &basis);
                let (pc, dc) = self.project3(wc, rect, &basis);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.map_or(true, |(bd, _, _)| depth < bd) {
                        best = Some((depth, mi, ti));
                    }
                }
            }
        }
        if let Some((_, mi, ti)) = best {
            // In an Assembly a click on a body selects the COMPONENT in the tree (and switches on the
            // placement gizmo); in a Part it selects a FACE (for features and for anchoring a sketch). The
            // behaviour follows the workbench, with no manual switching.
            if matches!(self.workbench, Workbench::Assembly) {
                // The isolation of the selection: a body of a leaf part inside a subassembly selects THE
                // SUBASSEMBLY (the direct child of the active context) rather than the leaf.
                // `highlight_mesh_set(Component)` will highlight its whole subtree. A click on a body
                // directly in the context selects that component itself.
                let ctx = self.current_ctx_id();
                let comp = self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b)).and_then(|owner| self.project.ancestor_child_of(ctx, owner));
                if let Some(idx) = comp.and_then(|cid| self.project.components.iter().position(|c| c.id == cid)) {
                    self.sel = Sel::Component(idx);
                } else {
                    self.sel = Sel::Mesh(mi);
                }
            } else {
                // A face is selected ONLY under a command that asks for one (the shell, the hole); an
                // ordinary click in a Part selects THE BODY and switches on the move gizmo.
                let want_face = matches!(self.cmd.kind, 6 | 7 | 23 | 25 | 26 | 28 | 30 | 31);
                let fi = if want_face { self.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32)))) } else { None };
                self.sel = match fi {
                    Some(fi) => Sel::Face(mi, fi),
                    None => Sel::Mesh(mi),
                };
                // The shell and the draft: multi-selection of faces strictly within ONE body — the ids of
                // faces are local to a body (OCCT numbers them from zero in each). A click on a face of
                // ANOTHER body starts the selection afresh on it, otherwise the ids of neighbouring bodies
                // get confused and the highlight leaks across.
                if matches!(self.cmd.kind, 6 | 23 | 25 | 26 | 28 | 30) && fi.is_some() {
                    // command 31 is NOT included here: there a click on ANOTHER body means "here is the
                    // surface" rather than "start the selection afresh" (see below).
                    let clicked_body = self.project.mesh_id(mi);
                    if self.gsel.faces_body != clicked_body {
                        self.gsel.faces.clear();
                        self.draft.neutral = 0;
                        self.gsel.faces_body = clicked_body;
                    }
                }
                // The shell: a click on a face ADDS or REMOVES its id from the multi-selection (by the
                // persistent id)
                if self.cmd.kind == 6 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if !self.gsel.faces.remove(&id) {
                            self.gsel.faces.insert(id);
                        }
                    }
                }
                // PUSH FACE: the face is EXACTLY ONE — a click replaces the previous one rather than
                // accumulating a set. Pushing several faces by one offset means a different result on each
                // of them (their normals differ), and that cannot be predicted.
                // THICKEN follows the same logic: the face is EXACTLY ONE, and one plate comes out of it.
                if self.cmd.kind == 25 || self.cmd.kind == 28 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        self.gsel.faces.clear();
                        self.gsel.faces.insert(id);
                    }
                }
                // REMOVE FACE: a multi-selection — a feature may consist of several faces (a stepped hole,
                // a boss with a chamfer). A click adds or removes.
                // REPLACE FACE: a click on a SHEET chooses the surface, a click on a body collects the
                // faces to be replaced. The document itself tells them apart (`sheet`) rather than a mode
                // switch: an extra mode here would force one to remember which step one is on.
                // TRIM: the first click on a SHEET says both WHAT is being cut and WHICH part is kept — the
                // point of the hit is the answer about the side. The second click, on a body, gives the
                // tool.
                if self.cmd.kind == 34 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    if is_sheet && self.trim.keep.is_none() {
                        if let (Some(id), Some((_, _, at))) = (clicked, self.pick_face_ray(rect, screen)) {
                            self.trim.keep = Some((id, at));
                            self.status = crate::i18n::tr("msg-trim-pick-tool");
                        }
                    } else if let Some(id) = clicked {
                        if self.trim.keep.map(|(b, _)| b) == Some(id) {
                            self.trim.keep = None; // a repeated click on the same sheet starts afresh
                            self.status = crate::i18n::tr("msg-trim");
                        } else {
                            self.trim.tool = if self.trim.tool == Some(id) { None } else { Some(id) };
                            self.status = crate::i18n::tr(if self.trim.tool.is_some() { "msg-trim-tool-picked" } else { "msg-trim-pick-tool" });
                        }
                    }
                }
                // STITCH: a click on a SHEET adds it to the set or removes it. Clicking a body is
                // pointless — surfaces are what get stitched, and that must be said at once rather than
                // after Enter.
                if self.cmd.kind == 33 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    match (is_sheet, clicked) {
                        (true, Some(id)) => {
                            if let Some(at) = self.stitch_parts.iter().position(|x| *x == id) {
                                self.stitch_parts.remove(at);
                            } else {
                                self.stitch_parts.push(id);
                            }
                            self.status = crate::i18n::tr1("msg-stitch-picked", "n", &self.stitch_parts.len().to_string());
                        }
                        _ => self.status = crate::i18n::tr("msg-stitch-only-sheets"),
                    }
                }
                if self.cmd.kind == 31 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    if is_sheet {
                        self.repl_surface = if self.repl_surface == clicked { None } else { clicked };
                        self.status = crate::i18n::tr(if self.repl_surface.is_some() { "msg-surface-picked" } else { "msg-surface-unpicked" });
                    } else if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if self.gsel.faces_body != clicked {
                            self.gsel.faces.clear();
                            self.gsel.faces_body = clicked;
                        }
                        if !self.gsel.faces.remove(&id) {
                            self.gsel.faces.insert(id);
                        }
                    }
                }
                if matches!(self.cmd.kind, 26 | 30) {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if !self.gsel.faces.remove(&id) {
                            self.gsel.faces.insert(id);
                        }
                    }
                }
                // The draft: in the neutral-face mode a click sets the neutral face (by the persistent
                // id), otherwise a click ADDS or REMOVES a face from the set of faces to be drafted.
                if self.cmd.kind == 23 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if self.draft.pick_neutral {
                            self.draft.neutral = if self.draft.neutral == id { 0 } else { id };
                            self.draft.pick_neutral = false;
                            self.gsel.faces.remove(&id); // the neutral face cannot also be a drafted one
                        } else if !self.gsel.faces.remove(&id) {
                            self.gsel.faces.insert(id);
                        }
                    }
                }
            }
        } else if matches!(self.sel, Sel::Face(..) | Sel::Mesh(..) | Sel::Component(..)) {
            // a click into emptiness clears the selection of a face, a body or a component
            self.sel = Sel::None;
        }
    }


    pub(super) fn pick_contour(&mut self, rect: Rect, screen: Pos2) {
        let world = self.to_world(rect, screen);
        let mut best: Option<(usize, f64)> = None;
        for (i, c) in self.project.contours.iter().enumerate() {
            let d = dist_to_contour(c, world);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let empty = match best {
            Some((_, d)) if d * self.view.scale as f64 <= self.grab(Grab::Curve) as f64 => false,
            _ => true,
        };
        if empty {
            // a click into emptiness clears the selection of an object (if a contour is selected)
            if matches!(self.sel, Sel::Contour(..)) {
                self.sel = Sel::None;
            }
            return;
        }
        let (i, _) = best.unwrap();
        // while an operation is being edited a click assigns geometry (selecting or clearing) by Id;
        // otherwise a contour is selected as an object to edit
        if let Some(op_i) = self.active_op() {
            let id = self.project.contour_id(i).unwrap_or(0);
            let sel = &mut self.project.operations[op_i].selection;
            if let Some(pos) = sel.iter().position(|&x| x == id) {
                sel.remove(pos);
            } else {
                sel.push(id);
            }
        } else {
            self.sel = Sel::Contour(i);
        }
    }
}

impl App {
    /// THE EDGES OF A BODY FOR PICKING — from a cache rather than from the kernel every frame.
    ///
    /// Extracting the edges of a live B-rep is expensive, and under the cursor it happens on every frame
    /// and for every body. On a real assembly (1182 components) that literally hung the application while
    /// an edge or vertex anchor was being chosen. The cache lives until the next rebuild of the
    /// geometry.
    pub(super) fn body_edges_cached(&self, body: qymcad_core::model::Id) -> Option<std::rc::Rc<(Vec<Vec<[f32; 3]>>, Vec<u32>)>> {
        {
            let c = self.cache.pick_edges.borrow();
            if c.0 == self.view_rev() {
                if let Some(v) = c.1.get(&body) {
                    return Some(v.clone()); // an `Rc`: a shared reference, not a copy of tens of thousands
                                            // of points
                }
            }
        }
        let shape = self.live.shapes.get(&body)?;
        let v = std::rc::Rc::new(shape.edges_with_ids());
        let mut c = self.cache.pick_edges.borrow_mut();
        if c.0 != self.view_rev() {
            c.0 = self.view_rev();
            c.1.clear();
        }
        c.1.insert(body, v.clone());
        Some(v)
    }
}

impl App {
    /// DOES THE SCREEN BOUNDING BOX OF A BODY CONTAIN THE CURSOR? A cheap cull before the expensive walk
    /// over the edges.
    ///
    /// At any moment there are one or two bodies under the cursor, and all of them were being walked. What
    /// matters is not the cull itself but its PRICE: approaching a body (finding the mesh, the display
    /// transform) cost 0.56 ms, and because of that the cull did not pay for itself. The corners of the
    /// box in world coordinates are cached per rebuild, and eight projections are all that is left.
    pub(super) fn body_bbox_hit(&self, body: qymcad_core::model::Id, rect: Rect, pos: Pos2, basis: &([f64; 3], [f64; 3], [f64; 3]), margin: f32) -> bool {
        let corners = {
            let mut c = self.cache.bbox_world.borrow_mut();
            if c.0 != self.view_rev() {
                c.0 = self.view_rev();
                c.1.clear();
            }
            match c.1.get(&body) {
                Some(v) => *v,
                None => {
                    let Some(mi) = self.project.mesh_index(body) else { return true };
                    let Some(bb) = self.project.bodies[mi].mesh.bounds() else { return true };
                    let wt = self.project.body_display_transform(body, self.current_ctx_id());
                    let mut out = [[0.0f64; 3]; 8];
                    for (k, p) in [
                        [bb.min.x, bb.min.y, bb.min.z], [bb.max.x, bb.min.y, bb.min.z],
                        [bb.min.x, bb.max.y, bb.min.z], [bb.max.x, bb.max.y, bb.min.z],
                        [bb.min.x, bb.min.y, bb.max.z], [bb.max.x, bb.min.y, bb.max.z],
                        [bb.min.x, bb.max.y, bb.max.z], [bb.max.x, bb.max.y, bb.max.z],
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        out[k] = if qymcad_core::feature::is_identity12(&wt) { p } else { qymcad_core::feature::apply12(&wt, p) };
                    }
                    c.1.insert(body, out);
                    out
                }
            }
        };
        let (mut lo, mut hi) = (Pos2::new(f32::MAX, f32::MAX), Pos2::new(f32::MIN, f32::MIN));
        for w in corners {
            let p = self.project3(w, rect, basis).0;
            lo = Pos2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Pos2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        pos.x >= lo.x - margin && pos.x <= hi.x + margin && pos.y >= lo.y - margin && pos.y <= hi.y + margin
    }
}

impl App {
    /// THE VISIBLE BODIES — as a list from a cache rather than recomputed for every body every frame.
    ///
    /// Deciding whether a body is visible requires finding its owner (a linear walk over the timeline) and
    /// following the chain of tick boxes in the tree. That is not expensive in itself, but inside the
    /// picking loop it repeats for EVERY body on EVERY frame — and it is exactly that which hung the
    /// application while an edge or vertex anchor was being chosen.
    pub(super) fn shown_bodies(&self) -> Vec<(usize, qymcad_core::model::Id)> {
        let ctx = self.current_ctx_id();
        {
            let c = self.cache.shown_bodies.borrow();
            if c.0 == self.view_rev() && c.1 == ctx {
                return c.2.clone();
            }
        }
        let list: Vec<(usize, qymcad_core::model::Id)> = (0..self.project.bodies.len())
            .filter(|&mi| self.body_shown(mi))
            .filter_map(|mi| self.project.mesh_id(mi).map(|b| (mi, b)))
            .collect();
        *self.cache.shown_bodies.borrow_mut() = (self.view_rev(), ctx, list.clone());
        list
    }
}
