//! THE SKETCHER - the geometry of a sketch, its dimensions and constraints, editing them and diagnosing
//! them.

use super::*;
use super::grab::Grab;

/// The sign of a cross product, as the word an arc is stored with: the sketcher works out the turn from the
/// geometry under the pointer, and the model keeps it as a word rather than as a bare flag.
fn winding(ccw: bool) -> qymcad_core::feature::Winding {
    if ccw {
        qymcad_core::feature::Winding::Ccw
    } else {
        qymcad_core::feature::Winding::Cw
    }
}

impl App {
    pub(super) fn sketch_pt(&self, si: usize, id: Id) -> Option<Point2> {
        self.project.sketches.get(si)?.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y))
    }


    /// A click with the dimension tool. Returns true when the click was handled.
    pub(super) fn dim_click(&mut self, rect: Rect, pos: Pos2) -> bool {
        // THE BOUNDARY OF AN OPERATION: placing a dimension is a deliberate act and makes one undo step.
        self.begin_edit(&crate::i18n::tr("sk-dim"));
        let r = self.dim_click_inner(rect, pos);
        self.commit_edit();
        r
    }

    fn dim_click_inner(&mut self, rect: Rect, pos: Pos2) -> bool {
        use qymcad_core::model::Constraint;
        let Sel::Sketch(si) = self.sel else {
            self.status = crate::i18n::tr("sk-pick-sketch-first");
            return true;
        };
        // the dimension follows the cursor: a click places it, OR (when it is a provisional length of a line
        // and a SECOND element was hit) switches it to a distance between the two.
        if let Some(ci) = self.place.dim {
            if let Some(DimRef::Line(la, lb)) = self.dim.first {
                // It switches to a distance ONLY on a click exactly on A VERTEX (not an end of this line) or
                // on AN AXIS. A click on the body of another line does NOT switch it - otherwise placing the
                // dimension line near the edge of a shape (a square, say) broke.
                let r2 = self.nearest_sketch_point(rect, pos, si).filter(|p| *p != la && *p != lb).map(DimRef::Point).or_else(|| {
                    let o = self.to_screen(rect, Point2::new(0.0, 0.0));
                    if (pos.y - o.y).abs() <= self.grab(Grab::Guide) {
                        let (a, b) = self.project.ensure_axis(si, 0);
                        Some(DimRef::Line(a, b))
                    } else if (pos.x - o.x).abs() <= self.grab(Grab::Guide) {
                        let (a, b) = self.project.ensure_axis(si, 1);
                        Some(DimRef::Line(a, b))
                    } else {
                        None
                    }
                }).or_else(|| {
                    // a click on ANOTHER roughly parallel line gives the distance between the two lines (the
                    // body of a line used to be ignored, and the gap between two parallels could not be
                    // measured). Parallel ones only - a perpendicular edge of a square next to the leader does
                    // NOT switch it, and a length is placed instead.
                    self.nearest_line_entity(rect, pos, si)
                        .filter(|&(a2, b2)| (a2, b2) != (la, lb) && (a2, b2) != (lb, la))
                        .filter(|&(a2, b2)| self.lines_parallel(si, la, lb, a2, b2))
                        .map(|(a2, b2)| DimRef::Line(a2, b2))
                });
                if let Some(r2) = r2 {
                    self.project.sketches[si].constraints.remove(ci); // remove the provisional length
                    self.place.dim = None;
                    self.dim.first = None;
                    self.make_between_dim(si, DimRef::Line(la, lb), r2);
                    return true;
                }
            }
            self.place.dim = None;
            self.dim.first = None;
            self.inline = InlineEdit::Dim(ci); // place it and go straight into typing the value
            self.dim.focus = true;
            self.status = crate::i18n::tr("sk-dim-placed");
            return true;
        }
        // a radius or a diameter: a circle entity (a diameter or radius dimension) or a circle primitive
        if self.dim.kind == 3 {
            if let Some(eid) = self.nearest_circle_entity(rect, pos, si) {
                // a circle gets a parametric diameter dimension (a Diameter constraint); an arc has its radius
                // edited
                let center = self.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
                    qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
                    _ => None,
                });
                if let Some(c) = center {
                    if let Some(ci) = self.project.ensure_diameter(si, c, true) {
                        self.inline = InlineEdit::Dim(ci);
                        self.dim.focus = true;
                        self.status = crate::i18n::tr("sk-enter-diameter");
                    }
                } else {
                    self.inline = InlineEdit::Circle(eid); // an arc or a fillet
                    self.dim.focus = true;
                    self.status = crate::i18n::tr("sk-enter-arc-radius");
                }
                return true;
            }
            // the circumscribed circle of a polygon has its radius edited, through its centre handle
            if let Some(cid) = self.polygon_under(rect, pos, si) {
                self.place.set(PlacingShape::Poly(cid));
                self.place.focus = true;
                self.status = crate::i18n::tr("sk-enter-polygon-radius");
                return true;
            }
            self.status = crate::i18n::tr("sk-click-nearer-circle");
            return true;
        }
        // A LINEAR dimension takes TWO references (a point, a line or an axis). The first line gives a length
        // that follows the cursor, but a click on a second element switches it to a distance between them
        // (point to line, line to a parallel line, or to an axis). A single line and nothing else is a
        // length.
        if self.dim.kind == 1 {
            let Some(r) = self.resolve_dim_ref(rect, pos, si) else {
                self.status = if self.dim.first.is_some() { crate::i18n::tr("sk-need-second") } else { crate::i18n::tr("sk-dim-pick-hint") };
                return true;
            };
            match self.dim.first.take() {
                None => match r {
                    // a coordinate axis as the first reference creates NO provisional length (an axis has no
                    // length; this used to breed a service dimension of 1.0 on the axes). It simply waits for
                    // the second element.
                    DimRef::Line(a, b) if self.project.sketches[si].axis_pts.contains(&a) || self.project.sketches[si].axis_pts.contains(&b) => {
                        self.dim.first = Some(r);
                        self.status = crate::i18n::tr("sk-axis-picked");
                    }
                    DimRef::Line(a, b) => {
                        // the provisional LENGTH of the line, following the cursor; a second click may switch it
                        // to a distance
                        if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                            let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt();
                            self.project.sketches[si].constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0 });
                            let ci = self.project.sketches[si].constraints.len() - 1;
                            let (_, conflict) = self.finish_dim(si, ci);
                            self.place.dim = Some(ci);
                            self.dim.first = Some(r);
                            self.status = if conflict {
                                format!("{} {}", ph::WARNING, crate::i18n::tr("sk-length-conflict"))
                            } else {
                                crate::i18n::tr("sk-length-hint")
                            };
                        }
                    }
                    DimRef::Point(_) => {
                        self.dim.first = Some(r);
                        self.status = crate::i18n::tr("sk-point-picked");
                    }
                },
                Some(r1) => self.make_between_dim(si, r1, r),
            }
            return true;
        }
        // AN ANGULAR dimension takes 2 LINES (the angle between them, placed at their real or implied
        // intersection) OR 3 points (A, the vertex, then C). A LINE under the cursor takes priority: click two
        // lines and an angle appears.
        if let Some(lr) = self.resolve_dim_ref(rect, pos, si).filter(|r| matches!(r, DimRef::Line(..))) {
            match self.dim.first.take() {
                Some(prev @ DimRef::Line(a1, b1)) => {
                    if matches!(lr, DimRef::Line(la, lb) if (la, lb) != (a1, b1)) {
                        self.dim.pick.clear();
                        self.make_between_dim(si, prev, lr); // two non-parallel lines give an AngleLines
                    } else {
                        self.dim.first = Some(prev); // the same line - wait for ANOTHER one
                    }
                }
                _ => {
                    self.dim.first = Some(lr);
                    self.dim.pick.clear();
                    self.status = crate::i18n::tr("sk-line1-picked");
                }
            }
            return true;
        }
        // otherwise it is the three-point mode (A, the vertex B, then C)
        let id = if let Some(r) = self.resolve_sketch_ref(rect, pos, si) {
            self.materialize_ref(si, r)
        } else {
            self.status = crate::i18n::tr("sk-angle-hint");
            return true;
        };
        if !self.dim.pick.contains(&id) {
            self.dim.pick.push(id);
        }
        if self.dim.pick.len() >= 3 {
            let pick = std::mem::take(&mut self.dim.pick);
            let (a, b, c) = (pick[0], pick[1], pick[2]);
            if let (Some(pa), Some(pb), Some(pc)) = (self.sketch_pt(si, a), self.sketch_pt(si, b), self.sketch_pt(si, c)) {
                let (ux, uy) = (pa.x - pb.x, pa.y - pb.y);
                let (vx, vy) = (pc.x - pb.x, pc.y - pb.y);
                let deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
                self.project.sketches[si].constraints.push(Constraint::Angle { a, b, c, deg, expr: String::new(), driven: false });
                let ci = self.project.sketches[si].constraints.len() - 1;
                let (redundant, conflict) = self.finish_dim(si, ci);
                self.status = if conflict {
                    format!("{} {}", ph::WARNING, crate::i18n::tr("sk-angle-conflict"))
                } else if redundant {
                    crate::i18n::tr("sk-driven-angle")
                } else {
                    crate::i18n::tr1("sk-angle-added", "a", &crate::i18n::num(deg, 1))
                };
            }
        }
        true
    }


    /// What is under the cursor in a sketch: (kind, Id). 0 is a point, 1 an entity (a line, an arc, a
    /// circle), 2 a primitive (a contour). Ordinary geometry takes priority over construction geometry.
    pub(super) fn sketch_hit(&self, rect: Rect, pos: Pos2, si: usize) -> Option<(u8, Id)> {
        use qymcad_core::model::EntityKind;
        let s = self.project.sketches.get(si)?;
        let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
        // the ends of THE AXES are not picked as ordinary points (they mark infinite lines); the origin
        // deliberately STAYS pickable (it is needed for a coincidence with the origin) - hence `axis_pts`
        // rather than `system_ids`
        let is_axis_ref = |id: Id| s.axis_pts.contains(&id);
        // 1) the nearest point
        let mut best_pt: Option<(f32, Id)> = None;
        for p in &s.points {
            if is_axis_ref(p.id) {
                continue;
            }
            let d = self.to_screen(rect, Point2::new(p.x, p.y)).distance(pos);
            if d <= self.grab(Grab::Point) && best_pt.map_or(true, |(bd, _)| d < bd) {
                best_pt = Some((d, p.id));
            }
        }
        if let Some((_, id)) = best_pt {
            return Some((0u8, id));
        }
        // 2) the nearest entity (an ordinary one takes priority over construction geometry)
        let mut best_e: Option<(u8, f32, Id)> = None;
        for e in &s.entities {
            let d = match e.kind {
                EntityKind::Line { a, b } => match (pt(a), pt(b)) {
                    (Some(pa), Some(pb)) => screen_dist_seg(pos, self.to_screen(rect, pa), self.to_screen(rect, pb)),
                    _ => f32::INFINITY,
                },
                EntityKind::Circle { center, r } => match pt(center) {
                    Some(c) => {
                        let sc = self.to_screen(rect, c);
                        let rp = (self.to_screen(rect, Point2::new(c.x + r, c.y)).x - sc.x).abs();
                        (sc.distance(pos) - rp).abs()
                    }
                    None => f32::INFINITY,
                },
                EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                    (Some(c), Some(pa)) => {
                        let sc = self.to_screen(rect, c);
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let rp = (self.to_screen(rect, Point2::new(c.x + r, c.y)).x - sc.x).abs();
                        (sc.distance(pos) - rp).abs()
                    }
                    _ => f32::INFINITY,
                },
                EntityKind::Ellipse { c, ma, mi } => match (pt(c), pt(ma), pt(mi)) {
                    (Some(pc), Some(pma), Some(pmi)) => {
                        // the distance to the outline of the ellipse, measured over samples
                        let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(1e-6);
                        let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major);
                        let (vx, vy) = (-uy, ux);
                        let (mut best, mut prev) = (f32::INFINITY, None::<Pos2>);
                        for k in 0..=48 {
                            let t = std::f64::consts::TAU * k as f64 / 48.0;
                            let (ct, st) = (t.cos(), t.sin());
                            let wp = Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy);
                            let sp = self.to_screen(rect, wp);
                            if let Some(pp) = prev {
                                best = best.min(screen_dist_seg(pos, pp, sp));
                            }
                            prev = Some(sp);
                        }
                        best
                    }
                    _ => f32::INFINITY,
                },
            };
            if d <= self.grab(Grab::Curve) {
                let tier = if e.construction { 1u8 } else { 0u8 };
                if best_e.map_or(true, |(bt, bd, _)| (tier, d) < (bt, bd)) {
                    best_e = Some((tier, d, e.id));
                }
            }
        }
        best_e.map(|(_, _, id)| (1u8, id))
    }


    /// Every point belonging to the current sketch selection: the points themselves plus the ends and
    /// centres of the picked entities. Used to move geometry as a whole.
    pub(super) fn sketch_sel_points(&self, si: usize) -> Vec<Id> {
        use qymcad_core::model::EntityKind;
        let Some(s) = self.project.sketches.get(si) else { return vec![] };
        let mut ids: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for (k, id) in &self.sel_sk.items {
            match k {
                0 => {
                    ids.insert(*id);
                }
                1 => {
                    if let Some(e) = s.entities.iter().find(|e| e.id == *id) {
                        match e.kind {
                            EntityKind::Line { a, b } => {
                                ids.insert(a);
                                ids.insert(b);
                            }
                            EntityKind::Circle { center, .. } => {
                                ids.insert(center);
                            }
                            EntityKind::Arc { center, a, b, .. } => {
                                ids.insert(center);
                                ids.insert(a);
                                ids.insert(b);
                            }
                            EntityKind::Ellipse { c, ma, mi } => {
                                ids.insert(c);
                                ids.insert(ma);
                                ids.insert(mi);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // DRIVEN POINTS ARE SUBTRACTED FROM A GROUP MOVE: selecting half a sketch by a box together with a
        // projection would move the projection too - and at the very first rebuild it would snap back into
        // place, leaving its own geometry adrift. What gets moved is what really belongs to the person.
        let driven = s.projected_points();
        ids.retain(|id| !driven.contains(id));
        ids.into_iter().collect()
    }


    /// A click in the sketch workbench: pick the nearest point or entity (Shift adds to the selection).
    pub(super) fn sketch_select_click(&mut self, rect: Rect, pos: Pos2, additive: bool) {
        let Sel::Sketch(si) = self.sel else { return };
        // a click on a text object picks it (Del removes it, a double click edits the string or the height,
        // dragging moves it)
        if let Some(ti) = self.text_at(rect, pos, si) {
            self.annot.text = Some(ti);
            self.annot.note = None;
            self.gsel.constraint = None;
            self.sel_sk.clear(); // the selection and whatever was waiting for it
            self.status = crate::i18n::tr("sk-text-selected");
            return;
        }
        self.annot.text = None;
        // a click on a note picks it (Del removes it, a double click edits it, dragging moves it)
        if let Some(ni) = self.note_at(rect, pos, si) {
            self.annot.note = Some(ni);
            self.gsel.constraint = None;
            self.sel_sk.clear(); // the selection and whatever was waiting for it
            self.status = crate::i18n::tr("sk-note-selected");
            return;
        }
        self.annot.note = None;
        // a click on a constraint glyph or a dimension caption PICKS it (Del removes it) rather than
        // deleting it at once
        if let Some(ci) = self.constraint_glyph_at(rect, pos, si).or_else(|| self.dim_at(rect, pos, si)) {
            self.gsel.constraint = Some(ci);
            self.sel_sk.clear(); // the selection and whatever was waiting for it
            self.status = crate::i18n::tr("sk-constraint-selected");
            return;
        }
        self.gsel.constraint = None;
        // while a constraint or modify tool is active, the selection accumulates WITHOUT Shift: elements are
        // clicked one at a time, and the constraint applies as soon as there are enough of them.
        let additive = additive || self.sel_sk.constraint.is_some() || self.sel_sk.modify.is_some();
        let mut hit = self.sketch_hit(rect, pos, si);
        // a click on the origin materialises the reference point and picks it (for constraints and dimensions)
        if hit.is_none() && self.to_screen(rect, Point2::new(0.0, 0.0)).distance(pos) <= self.grab(Grab::Point) {
            let o = self.project.ensure_origin(si);
            hit = Some((0u8, o));
        }
        // a click on the diameter or radius caption of a circle or an arc (outside the contour) picks it as an
        // entity
        if hit.is_none() {
            if let Some(eid) = self.nearest_circle_entity(rect, pos, si) {
                hit = Some((1u8, eid));
            }
        }
        // a click on the X or Y axis (when no other geometry is near) picks the axis as a line (kind 3, id 0
        // or 1)
        if hit.is_none() {
            let o = self.to_screen(rect, Point2::new(0.0, 0.0));
            let near_x = (pos.y - o.y).abs() <= self.grab(Grab::Guide); // the horizontal X axis (y = 0)
            let near_y = (pos.x - o.x).abs() <= self.grab(Grab::Guide); // the vertical Y axis (x = 0)
            if near_x && (!near_y || (pos.y - o.y).abs() <= (pos.x - o.x).abs()) {
                hit = Some((3u8, 0));
            } else if near_y {
                hit = Some((3u8, 1));
            }
        }
        match hit {
            Some(refr) => {
                if !additive {
                    self.sel_sk.clear(); // the selection and whatever was waiting for it
                }
                if let Some(p) = self.sel_sk.items.iter().position(|r| *r == refr) {
                    self.sel_sk.items.remove(p); // a second click deselects it
                } else {
                    self.sel_sk.items.push(refr);
                }
            }
            None => {
                if !additive {
                    self.sel_sk.clear(); // the selection and whatever was waiting for it
                }
            }
        }
        // a deferred constraint or operation: apply it as soon as the selection is enough
        if let Some(code) = self.sel_sk.constraint {
            if self.try_constraint(code) {
                self.sel_sk.constraint = None;
            }
        }
        if let Some(op) = self.sel_sk.modify {
            if self.try_modify(op) {
                self.sel_sk.modify = None;
            }
        }
    }


    /// The constraint button: if the selection is enough, apply it; otherwise go into waiting mode (the
    /// button lights up, the elements are picked, Esc cancels).
    pub(super) fn constraint_button(&mut self, code: u8) {
        self.exit_draw_tools();
        self.tool.modify = 0;
        self.sel_sk.constraint = None;
        self.sel_sk.modify = None;
        if self.try_constraint(code) {
            self.sel_sk.constraint = None;
        } else {
            // it could not be applied to the current selection, so the picking starts AFRESH, without any
            // elements left stuck: otherwise they would go into the constraint as `pts[0]` or `lines[0]` and
            // tie the wrong things together.
            self.sel_sk.clear(); // the selection and whatever was waiting for it
            self.sel_sk.constraint = Some(code);
            self.status = crate::i18n::tr("sk-pick-for-constraint");
        }
    }


    /// Apply a geometric constraint to the sketch selection. Returns true when it was applied.
    /// `code`: 0 coincident, 1 horizontal, 2 vertical, 3 parallel, 4 perpendicular, 5 equal, 6 fixed,
    /// 7 collinear, 8 concentric.
    pub(super) fn try_constraint(&mut self, code: u8) -> bool {
        // THE BOUNDARY OF AN OPERATION: placing a constraint is a deliberate act and makes one undo step.
        self.begin_edit(&crate::i18n::tr("sk-constraint"));
        let ok = self.try_constraint_inner(code);
        if ok {
            self.commit_edit();
        } else {
            self.abort_edit(); // the constraint did not take, so it leaves no trace
        }
        ok
    }

    fn try_constraint_inner(&mut self, code: u8) -> bool {
        use qymcad_core::model::Constraint;
        let Sel::Sketch(si) = self.sel else { return false };
        let pts = self.sel_point_ids();
        let mut lines = self.sel_line_pts(si);
        // coordinate axes picked as lines (kind 3): their straight lines are materialised and used as lines
        let axes: Vec<u64> = self.sel_sk.items.iter().filter(|(k, _)| *k == 3).map(|(_, id)| *id).collect();
        for w in axes {
            let (o, d) = self.project.ensure_axis(si, w as usize);
            lines.push((o, d));
        }
        let mut new: Vec<Constraint> = Vec::new();
        match code {
            0 if pts.len() >= 2 => new.push(Constraint::Coincident { a: pts[0], b: pts[1] }),
            // one point coincident with an entity: a circle or an arc gives point-on-circle (which takes
            // priority), otherwise a line or an axis gives point-on-line
            0 if pts.len() == 1 => {
                let cs = self.sel_circle_centers(si);
                if !cs.is_empty() {
                    new.push(Constraint::PointOnCircle { p: pts[0], c: cs[0] });
                } else if !lines.is_empty() {
                    new.push(Constraint::PointOnLine { p: pts[0], a: lines[0].0, b: lines[0].1 });
                }
            }
            1 if pts.len() >= 2 => new.push(Constraint::Horizontal { a: pts[0], b: pts[1] }),
            1 if !lines.is_empty() => new.push(Constraint::Horizontal { a: lines[0].0, b: lines[0].1 }),
            2 if pts.len() >= 2 => new.push(Constraint::Vertical { a: pts[0], b: pts[1] }),
            2 if !lines.is_empty() => new.push(Constraint::Vertical { a: lines[0].0, b: lines[0].1 }),
            3 if lines.len() >= 2 => new.push(Constraint::Parallel { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
            4 if lines.len() >= 2 => new.push(Constraint::Perpendicular { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
            5 if lines.len() >= 2 => new.push(Constraint::Equal { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
            5 => {
                // equal radii of two circles
                let cs = self.sel_circle_centers(si);
                if cs.len() >= 2 {
                    new.push(Constraint::EqualRadius { c1: cs[0], c2: cs[1] });
                }
            }
            6 if !pts.is_empty() => new.extend(pts.iter().map(|p| Constraint::Fixed { p: *p })),
            7 if lines.len() >= 2 => new.push(Constraint::Collinear { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
            8 => {
                // concentricity is A REAL kind with a glyph of its own, not a fake made of coincident centres
                let centers = self.sel_circle_centers(si);
                if centers.len() >= 2 {
                    new.push(Constraint::Concentric { c1: centers[0], c2: centers[1] });
                }
            }
            9 => {
                // tangency: a line plus a circle
                if let (Some((la, lb)), Some((cc, rr))) = (lines.first().copied(), self.sel_circle_cr(si)) {
                    new.push(Constraint::Tangent { a: la, b: lb, c: cc, r: rr });
                }
            }
            10 => {
                // symmetry: two points about an axis (the picked line)
                if pts.len() >= 2 {
                    if let Some((la, lb)) = lines.first().copied() {
                        new.push(Constraint::Symmetric { a: pts[0], b: pts[1], la, lb });
                    }
                }
            }
            11 if pts.len() == 1 && !lines.is_empty() => {
                // midpoint: the point at the middle of the picked line
                new.push(Constraint::Midpoint { p: pts[0], a: lines[0].0, b: lines[0].1 });
            }
            _ => {}
        }
        if new.is_empty() {
            return false;
        }
        self.project.sketches[si].constraints.extend(new);
        let resid = self.project.solve_sketch(si);
        self.sel_sk.clear(); // the selection and whatever was waiting for it
        self.invalidate();
        self.status = if resid < 1e-3 { crate::i18n::tr("sk-constraint-added") } else { crate::i18n::tr1("sk-constraint-added-resid", "r", &crate::i18n::num(resid, 2)) };
        true
    }


    /// The screen position of a dimension caption (for editing in place and for hit testing).
    pub(super) fn dim_label_pos(&self, rect: Rect, si: usize, ci: usize) -> Option<Pos2> {
        use qymcad_core::model::Constraint;
        let s = self.project.sketches.get(si)?;
        // the caption offset `off` is stored in WORLD units and converted to pixels through the scale
        // (otherwise the caption does not scale with the geometry on zoom and drifts off screen). Angles (the
        // diameter) are left alone.
        let sc = self.view.scale as f32;
        match *s.constraints.get(ci)? {
            Constraint::Distance { a, b, off, axis, .. } => {
                let (pa, pb) = (self.sketch_pt(si, a)?, self.sketch_pt(si, b)?);
                let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                let (la, lb, perp) = match axis {
                    1 => {
                        let y = (sa.y + sb.y) / 2.0 + off as f32 * sc;
                        (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(0.0, 1.0))
                    }
                    2 => {
                        let x = (sa.x + sb.x) / 2.0 + off as f32 * sc;
                        (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(1.0, 0.0))
                    }
                    _ => {
                        let dir = (sb - sa).normalized();
                        let perp = egui::vec2(-dir.y, dir.x);
                        let o = 16.0 + off as f32 * sc;
                        (sa + perp * o, sb + perp * o, perp)
                    }
                };
                Some(((la.to_vec2() + lb.to_vec2()) / 2.0 + perp * 8.0).to_pos2())
            }
            Constraint::DistancePL { p, a, b, off, .. } => {
                let (pp, pa, _pb) = (self.sketch_pt(si, p)?, self.sketch_pt(si, a)?, self.sketch_pt(si, b)?);
                let (sp, sa) = (self.to_screen(rect, pp), self.to_screen(rect, pa));
                let ab = self.line_screen_dir(si, a, b, rect)?;
                let foot = sa + ab * (sp - sa).dot(ab);
                let perp = (sp - foot).normalized();
                let o = off as f32 * sc;
                let (lp, lf) = (sp + ab * o, foot + ab * o); // the leader runs along the line (see the drawing and the hit test)
                Some(((lp.to_vec2() + lf.to_vec2()) / 2.0 + perp * 8.0).to_pos2())
            }
            Constraint::EdgeDistance { c1, c2, m1, m2, off, .. } => {
                let (p1, p2) = (self.sketch_pt(si, c1)?, self.sketch_pt(si, c2)?);
                let r_of = |cid: Id| -> f64 {
                    s.entities.iter().find_map(|e| match e.kind {
                        qymcad_core::model::EntityKind::Circle { center, r } if center == cid => Some(r),
                        qymcad_core::model::EntityKind::Arc { center, a, .. } if center == cid => self.sketch_pt(si, a).zip(self.sketch_pt(si, center)).map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
                        _ => None,
                    }).unwrap_or(0.0)
                };
                let (r1, r2) = (r_of(c1), r_of(c2));
                let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
                let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
                let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
                let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
                let (sa, sb) = (self.to_screen(rect, e1), self.to_screen(rect, e2));
                let dir = (sb - sa).normalized();
                let perp = egui::vec2(-dir.y, dir.x);
                let o = 16.0 + off as f32 * sc;
                Some((((sa + perp * o).to_vec2() + (sb + perp * o).to_vec2()) / 2.0 + perp * 8.0).to_pos2())
            }
            Constraint::Angle { a, b, c, .. } => {
                let (pa, pb, pc) = (self.sketch_pt(si, a)?, self.sketch_pt(si, b)?, self.sketch_pt(si, c)?);
                let (sa, sb, sc) = (self.to_screen(rect, pa), self.to_screen(rect, pb), self.to_screen(rect, pc));
                let bis = ((sa - sb).normalized() + (sc - sb).normalized()).normalized();
                Some(sb + bis * 40.0)
            }
            Constraint::Diameter { c, off, .. } => {
                let cp = self.sketch_pt(si, c)?;
                let r = self.center_radius(si, c)?; // a circle OR an arc
                let sc = self.to_screen(rect, cp);
                // the diameter or radius label is placed AROUND THE CIRCLE - `off` holds the angle of the
                // leader in radians, in screen coordinates. By default (off = 0) it goes horizontally to the
                // right, but it can be dragged to any angle about the centre.
                let r_px = (self.to_screen(rect, Point2::new(cp.x + r, cp.y)) - sc).length();
                let ang = off as f32;
                Some(sc + egui::vec2(ang.cos(), ang.sin()) * (r_px + 14.0))
            }
            Constraint::AngleLines { a, b, c, d, .. } => {
                let (pa, pb, pc, pd) = (self.sketch_pt(si, a)?, self.sketch_pt(si, b)?, self.sketch_pt(si, c)?, self.sketch_pt(si, d)?);
                let (sa, sb, sc, sd) = (self.to_screen(rect, pa), self.to_screen(rect, pb), self.to_screen(rect, pc), self.to_screen(rect, pd));
                let ix = lines_intersect(sa, sb, sc, sd).unwrap_or(((sb.to_vec2() + sd.to_vec2()) / 2.0).to_pos2());
                let bis = ((sb - ix).normalized() + (sd - ix).normalized()).normalized();
                Some(ix + bis * 40.0)
            }
            // an arc length: the caption sits at the middle of the arc, matching how it is drawn - otherwise
            // `dim_at` does not find it, and the dimension can be neither picked, nor edited, nor dragged.
            Constraint::ArcLength { c, a, b, off, .. } => {
                let (cp, pa, pb) = (self.sketch_pt(si, c)?, self.sketch_pt(si, a)?, self.sketch_pt(si, b)?);
                let r = ((pa.x - cp.x).powi(2) + (pa.y - cp.y).powi(2)).sqrt();
                let mid = Point2::new((pa.x + pb.x) / 2.0 - cp.x, (pa.y + pb.y) / 2.0 - cp.y);
                let ml = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-9);
                let ed = self.to_screen(rect, Point2::new(cp.x + mid.x / ml * r, cp.y + mid.y / ml * r));
                Some((ed.to_vec2() + egui::vec2(10.0, -8.0 + off as f32 * sc)).to_pos2())
            }
            _ => None,
        }
    }


    /// The index of the dimension (a distance or an angle) whose caption is nearest to a screen point.
    pub(super) fn dim_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
        use qymcad_core::model::Constraint;
        let s = self.project.sketches.get(si)?;
        let mut best: Option<(f32, usize)> = None;
        for ci in 0..s.constraints.len() {
            // the distance to the caption
            let mut d = self.dim_label_pos(rect, si, ci).map(|p| p.distance(pos)).unwrap_or(f32::INFINITY);
            // ...and to the dimension line itself (a click on the line picks it too)
            if let Some(Constraint::Distance { a, b, off, .. }) = s.constraints.get(ci).cloned() {
                if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                    let dir = (sb - sa).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    let o = perp * (16.0 + off as f32 * self.view.scale as f32);
                    d = d.min(screen_dist_seg(pos, sa + o, sb + o));
                }
            }
            if d <= self.grab(Grab::Label) && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, ci));
            }
        }
        best.map(|(_, ci)| ci)
    }


    /// The in-place editors of dimension values in the viewport (a double click, or the radius tool).
    pub(super) fn dim_editor(&mut self, ctx: &egui::Context, rect: Rect) {
        use qymcad_core::model::{Constraint, EntityKind};
        let Sel::Sketch(si) = self.sel else {
            self.inline.clear();
            self.inline.clear();
            return;
        };
        // the radius of a circle or an arc entity
        if let Some(eid) = self.inline.circle() {
            let info = self.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
                EntityKind::Circle { center, r } => Some((center, r, false)),
                EntityKind::Arc { center, a, .. } => self.sketch_pt(si, a).zip(self.sketch_pt(si, center)).map(|(pa, c)| (center, ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), true)),
                _ => None,
            });
            if let (Some((_center, r, is_arc)), Some(cp)) = (info, info.and_then(|(c, ..)| self.sketch_pt(si, c))) {
                let at = self.to_screen(rect, Point2::new(cp.x + r, cp.y));
                let mut rr = r;
                let (mut chg, mut close) = (false, false);
                let want_focus = self.dim.focus;
                let mut got_focus = false;
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                // the text buffer of the radius, with auto-focus and Enter, as with a linear dimension
                let mut buf = std::mem::take(&mut self.dim.buf);
                if want_focus {
                    // THROUGH THE COMMON DOOR: no longer than four digits, with no tail. A raw `format!("{}")`
                    // printed the whole truth about an f64 - "12.750000000000002" in the radius field.
                    buf = qymcad_core::expr::fmt_num(if is_arc { r } else { 2.0 * r }); // a radius for an arc, a diameter for a circle
                }
                let mut buf_changed = false;
                egui::Area::new(egui::Id::new(("circedit", si, eid))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(if is_arc { crate::i18n::tr("sk-radius") } else { crate::i18n::tr("sk-diameter") });
                            let rsp = Self::focus_edit(ui, &mut buf, 80.0, &crate::i18n::tr("sk-mm"), want_focus);
                            if rsp.changed() {
                                buf_changed = true;
                            }
                            got_focus = rsp.has_focus();
                            if rsp.lost_focus() && enter {
                                close = true;
                            }
                            if ui.button(ph::CHECK).clicked() {
                                close = true;
                            }
                        });
                    });
                });
                if got_focus {
                    self.dim.focus = false;
                }
                self.dim.buf = buf.clone();
                if buf_changed {
                    // the radius or diameter of an arc: the value is evaluated when an expression is typed. It
                    // is stored as a number (an arc has no parametric dimension constraint), just like any
                    // value set by dragging - but "w/2" can be typed now.
                    if let Some(v) = self.parse_num(&buf) {
                        rr = (if is_arc { v } else { v * 0.5 }).max(0.01); // Ø -> r
                        chg = true;
                    }
                }
                if chg {
                    if is_arc {
                        // a fillet arc has its radius edited through the dimension constraint (parametrically);
                        // failing that, the fillet is recomputed geometrically; failing that, it is a plain arc
                        if !self.project.set_fillet_radius_dim(si, eid, rr.max(0.01)) && !self.project.set_fillet_radius(si, eid, rr.max(0.01)) {
                            self.project.set_arc_radius(si, eid, rr.max(0.01));
                        }
                    } else {
                        if let Some(e) = self.project.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)) {
                            if let EntityKind::Circle { r, .. } = &mut e.kind {
                                *r = rr.max(0.01);
                            }
                        }
                        self.project.regen_sketch(si);
                    }
                    self.invalidate();
                }
                if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.inline.clear();
                    self.dim.buf.clear();
                    self.dim.focus = false;
                }
            } else {
                self.inline.clear();
            }
        }
        let Some(ci) = self.inline.dim() else { return };
        let Some(at) = self.dim_label_pos(rect, si, ci) else {
            self.inline.clear();
            return;
        };
        let mut changed = false;
        let mut close = false;
        let want_focus = self.dim.focus; // focus is requested until the field actually takes it
        let mut got_focus = false;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        // the current state of the dimension: the value, the expression, whether it is driven, whether it is
        // an angle
        let (cur_val, cur_expr, is_driven, is_angle) = match self.project.sketches[si].constraints.get(ci) {
            // `DistancePL` stores a SIGNED d (which side); the magnitude is what gets shown and edited
            Some(Constraint::DistancePL { d, expr, driven, .. }) => (d.abs(), expr.clone(), *driven, false),
            Some(Constraint::Distance { d, expr, driven, .. }) | Some(Constraint::Diameter { d, expr, driven, .. }) | Some(Constraint::EdgeDistance { d, expr, driven, .. }) => (*d, expr.clone(), *driven, false),
            Some(Constraint::ArcLength { len, expr, driven, .. }) => (*len, expr.clone(), *driven, false),
            Some(Constraint::Angle { deg, expr, driven, .. }) | Some(Constraint::AngleLines { deg, expr, driven, .. }) => (*deg, expr.clone(), *driven, true),
            _ => {
                self.inline.clear();
                return;
            }
        };
        // diameter or radius: which of the two the field shows
        let diam_mode = matches!(self.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { diam: true, .. }));
        let is_diameter = matches!(self.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { .. }));
        // the buffer of the field: on opening it holds the current expression, or the number
        let mut buf = std::mem::take(&mut self.dim.buf);
        if want_focus {
            buf = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
        }
        let eval_res = if buf.trim().is_empty() { None } else { Some(self.project.eval_expr(&buf)) };
        let (mut toggle_driven, mut toggle_diam) = (false, false);
        let mut name_taken = false;
        let is_edge = matches!(self.project.sketches[si].constraints.get(ci), Some(Constraint::EdgeDistance { .. }));
        let mut toggle_edge = false;
        let label = if is_angle { crate::i18n::tr("sk-angle") } else if is_diameter { if diam_mode { "Ø" } else { "R" }.to_string() } else if is_edge { crate::i18n::tr("sk-tangent-short") } else { crate::i18n::tr("sk-dim") };
        // A DRIVER BELONGS TO ANY DIMENSION, NOT ONLY TO A LINEAR ONE.
        //
        // This used to read "only `Constraint::Distance`", and the name field appeared solely on a distance
        // between two points. The question came up plainly: why does a drive field exist in some places and
        // not in others? There was no explainable logic behind it: the driver name WAS STORED as a pair of
        // points, so an angle, a diameter, a distance to a line, an arc length or a tangent gap simply had
        // nothing to be named by. Now a driver is identified by a set of entities, and any dimension can be
        // named.
        let dim_refs: Option<Vec<Id>> = self.project.sketches[si].constraints.get(ci).and_then(Project::dim_refs);
        let sid = self.project.sketches[si].id;
        let cur_name: String = dim_refs
            .as_ref()
            .map(|r| {
                let t = qymcad_core::model::DimTarget::Sketch { sketch: sid, refs: Project::dim_key_pub(r) };
                crate::i18n::name(&self.project.name_of_target(&t))
            })
            .unwrap_or_default();
        // THE TEXT LIVES IN THE BUFFER OF THE FIELD ITSELF WHILE IT IS BEING TYPED. The model is touched
        // once, on Enter.
        let key_refs = dim_refs.clone().unwrap_or_default();
        // WHAT WAS ASKED FOR: a new driver name. It is applied AFTER the drawing - during it the document is
        // lent to the field (which reads the project), and there is no reason to change the model mid-frame.
        let mut name_commit: Option<String> = None;
        let mut name_owner: Option<String> = None;
        egui::Area::new(egui::Id::new(("dimedit", si, ci))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    if is_diameter && ui.button(if diam_mode { "->R" } else { "->Ø" }).on_hover_text(&crate::i18n::tr("sk-toggle-rad-dia")).clicked() {
                        toggle_diam = true;
                    }
                    if is_edge && ui.button(&crate::i18n::tr("sk-edge-toggle")).on_hover_text(&crate::i18n::tr("sk-near-far-edge")).clicked() {
                        toggle_edge = true;
                    }
                    if is_driven {
                        ui.label(egui::RichText::new(crate::i18n::tr1("sk-driven-value", "v", &crate::i18n::num(cur_val, 3))).color(self.scheme.pal.text_dim()));
                    } else {
                        // ONE field for the whole project: a number or an expression (50, w/2, len+5), the list
                        // of drivers, auto-focus with the previous text selected, and Enter to apply.
                        let fid = egui::Id::new(("dimval", si, ci));
                        let o = super::expr_field::expr_field_autofocus(ui, &self.project, fid, &buf, 100.0, &crate::i18n::tr("sk-expr-example"), want_focus);
                        buf = o.text;
                        got_focus = o.resp.has_focus();
                        if o.committed && enter {
                            close = true;
                        }
                    }
                    if ui.selectable_label(is_driven, &crate::i18n::tr("sk-ref-short")).on_hover_text(&crate::i18n::tr("sk-driven-hint")).clicked() {
                        toggle_driven = true;
                    }
                    if ui.button(ph::CHECK).clicked() {
                        close = true;
                    }
                });
                if !is_driven {
                    match &eval_res {
                        Some(Ok(v)) => {
                            ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                        }
                        Some(Err(e)) => {
                            // THROUGH THE COMMON DOOR: the `Display` of the error is in one language only, and
                            // an interface in another showed a raw "unexpected token: /".
                            ui.label(egui::RichText::new(crate::i18n::expr_error_text(e)).color(self.scheme.pal.error_mild()).small());
                        }
                        None => {}
                    }
                    ui.label(egui::RichText::new(&crate::i18n::tr("sk-enter-apply-expr")).weak().small());
                }
                if dim_refs.is_some() {
                    // WHETHER THE NAME WILL DO. An empty one is legitimate: the dimension simply stops being a
                    // driver.
                    let ok = |nm: &str| {
                        nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !self.project.driver_name_taken(nm, sid, &key_refs))
                    };
                    ui.horizontal(|ui| {
                        // THE CAPTION IS TRANSLATABLE. A raw "driver:" used to stand here - the only string of
                        // the popup that went around the language catalogue.
                        ui.label(&crate::i18n::tr("sk-driver-label")).on_hover_text(&crate::i18n::tr("sk-driver-name-hint"));
                        let id = egui::Id::new(("dimdrv", si, ci));
                        let r = super::expr_field::name_field(ui, &self.project, id, &cur_name, 110.0, &crate::i18n::tr("sk-name-placeholder"), &ok);
                        let nm = r.text.trim().to_string();
                        if r.committed && nm != cur_name.trim() {
                            name_commit = Some(nm.clone());
                        }
                        // A TAKEN NAME GIVES A YELLOW LINE BELOW AND BLOCKS APPLYING. Not a single letter is
                        // lost while this happens: type `len`, `lena` or `length` - until the name is free, the
                        // popup simply refuses to apply and says why.
                        name_taken = !nm.is_empty() && !ok(&nm);
                        if name_taken {
                            name_owner = self.project.name_owner(&nm).map(|o| if o.path.is_empty() { crate::i18n::tr("par-owner-project") } else { o.path });
                        }
                    });
                    if name_taken {
                        // WHOSE NAME IT IS, not merely "taken": otherwise the namesake has to be hunted for
                        // across the whole project by hand. A name with no owner is simply unusable in itself.
                        let msg = match &name_owner {
                            Some(w) => crate::i18n::tr2("par-name-taken", "name", "", "where", w),
                            None => crate::i18n::tr("sk-driver-name-bad"),
                        };
                        ui.label(egui::RichText::new(&msg).color(self.scheme.pal.warning()).small()).on_hover_text(&crate::i18n::tr("sk-driver-name-taken-hint"));
                    }
                }
            });
        });
        if got_focus || is_driven {
            self.dim.focus = false; // focus was taken (or there is nothing to focus) - stop asking
        }
        self.dim.buf = buf.clone();
        // a snapshot of the dimension AND of the point positions BEFORE the edit, so it can be ROLLED BACK if
        // the new value CONFLICTS (over-defines the sketch). Otherwise the solver silently tilted a vertical
        // or moved a fixed point - the soft least-squares compromise. An inconsistent dimension is rejected
        // rather than allowed to break the sketch.
        let old_con = self.project.sketches[si].constraints.get(ci).cloned();
        // THE TEXT DIFFERS FROM WHAT IS STORED, so there is something to apply on commit. TEXTS are compared
        // rather than the number parsed again: parsing a value in the sketcher must go through one door
        // (`parse_num`), and a second `parse::<f64>()` is caught at once by the ratchet in `expr_fields.rs`.
        let shown_before = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
        let value_differs = buf.trim() != shown_before.trim();
        let apply_value = close && !is_driven && value_differs;
        // apply the changes to the constraint
        let mut new_buf: Option<String> = None; // refresh the field after a value edit (a diameter/radius swap, say)
        if let Some(c) = self.project.sketches[si].constraints.get_mut(ci) {
            if toggle_driven {
                match c {
                    Constraint::Distance { driven, .. } | Constraint::DistancePL { driven, .. } | Constraint::Angle { driven, .. } | Constraint::Diameter { driven, .. } | Constraint::AngleLines { driven, .. } | Constraint::ArcLength { driven, .. } | Constraint::EdgeDistance { driven, .. } => *driven = !*driven,
                    _ => {}
                }
                changed = true;
            }
            if toggle_diam {
                if let Constraint::Diameter { d, diam, expr, .. } = c {
                    *d = if *diam { *d * 0.5 } else { *d * 2.0 }; // diameter to radius: the value is recomputed
                    *diam = !*diam;
                    if expr.trim().is_empty() {
                        new_buf = Some(qymcad_core::expr::fmt_num(*d)); // refresh the field
                    }
                }
                changed = true;
            }
            // THE VALUE IS APPLIED ON COMMIT, NOT ON EVERY LETTER.
            //
            // Every keystroke used to edit the constraint, solve the sketch again and mark the document for a
            // rebuild: typing "125" meant three rebuilds, and on the intermediate "1" and "12" the sketch
            // honestly rebuilt itself to a different size. The answer is visible anyway - the line "= 42.500"
            // lives under the field, and it costs the document nothing.
            if apply_value {
                let t = buf.trim();
                let num = t.parse::<f64>().ok();
                match c {
                    // `DistancePL`: d is signed (it says which side); the magnitude is typed and the sign is kept
                    Constraint::DistancePL { d, expr, .. } => {
                        if let Some(v) = num {
                            *d = if *d < 0.0 { -v.abs() } else { v.abs() };
                            expr.clear();
                        } else {
                            *expr = buf.clone();
                        }
                    }
                    Constraint::Distance { d, expr, .. } | Constraint::Diameter { d, expr, .. } | Constraint::EdgeDistance { d, expr, .. } => {
                        if let Some(v) = num {
                            *d = v;
                            expr.clear();
                        } else {
                            *expr = buf.clone();
                        }
                    }
                    Constraint::ArcLength { len, expr, .. } => {
                        if let Some(v) = num {
                            *len = v;
                            expr.clear();
                        } else {
                            *expr = buf.clone();
                        }
                    }
                    Constraint::Angle { deg, expr, .. } | Constraint::AngleLines { deg, expr, .. } => {
                        if let Some(v) = num {
                            *deg = v;
                            expr.clear();
                        } else {
                            *expr = buf.clone();
                        }
                    }
                    _ => {}
                }
                changed = true;
            }
        }
        if let Some(nb) = new_buf {
            self.dim.buf = nb; // the field shows the new value (after a diameter/radius swap)
        }
        if toggle_edge {
            // near edge against far edge: the non-zero m values are inverted and d is recomputed from the
            // geometry, so nothing jumps
            let edge = match self.project.sketches[si].constraints.get(ci) {
                Some(Constraint::EdgeDistance { c1, c2, m1, m2, .. }) => Some((*c1, *c2, *m1, *m2)),
                _ => None,
            };
            if let Some((c1, c2, m1, m2)) = edge {
                let (nm1, nm2) = (if m1 != 0 { -m1 } else { 0 }, if m2 != 0 { -m2 } else { 0 });
                let nd = self.project.measure_edge_distance(si, c1, nm1, c2, nm2);
                if let Some(Constraint::EdgeDistance { m1, m2, d, expr, .. }) = self.project.sketches[si].constraints.get_mut(ci) {
                    *m1 = nm1;
                    *m2 = nm2;
                    *d = nd;
                    expr.clear();
                }
                changed = true;
            }
        }
        // THE DRIVER NAME GOES IN AS ONE OPERATION, ON ENTER.
        //
        // This was the main trouble that got reported: the name was written into the model on EVERY letter,
        // and `mark_param_dependents_dirty()` was called right after, marking EVERY node carrying dimensions
        // dirty - that is, the whole project was rebuilt per letter. Now the edit arrives once.
        if let (Some(refs), Some(nm)) = (dim_refs.clone(), name_commit) {
            let ed = self.edit(crate::i18n::tr("sk-driver-step"));
            let old = cur_name.trim().to_string();
            if !old.is_empty() && !nm.is_empty() {
                // RENAMING CARRIES THE FORMULAS WITH IT - otherwise the expressions stay pointed at a name
                // that no longer exists, and the model breaks silently.
                let _ = ed.app.project.rename_driver(&old, &nm);
            } else {
                ed.app.project.add_named_dim(nm.clone(), sid, refs);
            }
            drop(ed);
            // ONLY WHAT DEPENDS ON THIS NAME is recomputed, not the whole timeline of every body.
            if !nm.is_empty() {
                self.project.mark_param_dependents_dirty_for(&nm);
            }
            self.mark_dirty_for_rebuild();
        }
        if changed {
            // a snapshot of the positions BEFORE solving (the sketch is consistent right now)
            let old_pts: Vec<(f64, f64)> = self.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
            let resid = self.project.solve_sketch(si);
            // a large residual means the system is INCONSISTENT (the new dimension conflicts with a vertical,
            // a fixed point or another constraint). The solver would converge to about 1e-6; a threshold of
            // 1e-2 mm is above the noise and below any real conflict. The dimension and the positions are
            // rolled back and solved again, so the sketch returns to a consistent state instead of breaking
            // silently.
            if resid > 1e-2 {
                if let (Some(oc), Some(c)) = (old_con.clone(), self.project.sketches[si].constraints.get_mut(ci)) {
                    *c = oc;
                }
                for (p, (x, y)) in self.project.sketches[si].points.iter_mut().zip(old_pts) {
                    p.x = x;
                    p.y = y;
                }
                self.project.solve_sketch(si);
                self.status = crate::i18n::tr1("sk-dim-incompatible", "r", &crate::i18n::num(resid, 2));
            }
            self.invalidate();
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.inline.clear();
            self.dim.buf.clear();
            self.dim.focus = false;
            if dim_refs.is_some() && !self.project.named_dims.is_empty() {
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting - the driver reaches the bodies that consume it
            }
        }
    }


    /// Whether a sketch (by id) is shown in the visibility context.
    pub(super) fn sketch_in_ctx(&self, sketch_id: Id) -> bool {
        // NESTED ONES BELONG. The rule used to be strict equality, and in the root of an assembly EVERY
        // sketch counted as someone else's: they belong to the parts, not to the root. Because of that the
        // contours tick box in an assembly showed nothing at all - reported as there being no contours in 3D
        // however much they were clicked.
        //
        // Bodies count differently (`component_is_within`, see `body_shown`), which is why the bodies of
        // nested parts are visible in an assembly. Sketches must follow the same rule: one's own means the
        // context AND its descendants. Neighbours (another branch of the tree) stay foreign, and the
        // isolation does not suffer from this.
        let ctx = self.viz_ctx_id();
        match self.project.sketch_owner(sketch_id) {
            Some(owner) => owner == ctx || self.project.component_is_within(owner, ctx),
            None => false,
        }
    }


    /// The imprint of a sketch, for the status cache: the coordinates of the points plus the number of
    /// entities, constraints and splines. O(n) per frame is pennies against the Jacobian. Any edit or drag
    /// changes the imprint, which forces a recount.
    pub(super) fn sketch_fingerprint(&self, si: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(s) = self.project.sketches.get(si) {
            for p in &s.points {
                p.id.hash(&mut h);
                p.x.to_bits().hash(&mut h);
                p.y.to_bits().hash(&mut h);
            }
            s.entities.len().hash(&mut h);
            s.constraints.len().hash(&mut h);
            s.splines.len().hash(&mut h);
            // the values of the dimensions (a number edited without the points moving - before solving)
            for c in &s.constraints {
                if let Some(d) = c.dim_value() {
                    d.to_bits().hash(&mut h);
                }
            }
        }
        h.finish()
    }


    /// ALL of the sketch diagnostics in one pass and THROUGH A CACHE (recomputed only when the imprint
    /// changes).
    ///
    /// Every piece of it is a full Jacobian plus a Gaussian elimination (O(m*nv^2)). Only the degrees of
    /// freedom and the free points used to be cached, while the conflicting set and the redundant
    /// constraints were computed DIRECTLY and several times per frame: the panel, the list of constraints,
    /// the dimension overlay, the glyphs and the model tree - five independent runs over the same sketch.
    /// Now there is one source.
    pub(super) fn sketch_diag(&self, si: usize) -> SketchDiag {
        let fp = self.sketch_fingerprint(si);
        if let Some((csi, cfp, d)) = &*self.cache.sk_status.borrow() {
            if *csi == si && *cfp == fp {
                return d.clone();
            }
        }
        let d = SketchDiag {
            dof: self.project.sketch_dof(si),
            free: self.project.sketch_free_points(si),
            conflicts: self.project.sketch_conflicts(si).into_iter().collect(),
            redundant: self.project.sketch_redundant_constraints(si).into_iter().collect(),
        };
        *self.cache.sk_status.borrow_mut() = Some((si, fp, d.clone()));
        d
    }

    /// THE NAME OF A SKETCH ENTITY FOR A PERSON: "Line 3", "Circle 1".
    ///
    /// The number is the ordinal among entities OF THE SAME kind rather than a running id: "Line 3" can be
    /// found by eye, while "Line 47" says nothing.
    pub(crate) fn sketch_entity_name(&self, si: usize, eid: Id) -> String {
        use qymcad_core::model::EntityKind as EK;
        let Some(s) = self.project.sketches.get(si) else { return String::new() };
        let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return String::new() };
        let key = match e.kind {
            EK::Line { .. } => "ent-line",
            EK::Arc { .. } => "ent-arc",
            EK::Circle { .. } => "ent-circle",
            EK::Ellipse { .. } => "ent-ellipse",
        };
        let same = |k: &EK| std::mem::discriminant(k) == std::mem::discriminant(&e.kind);
        let n = s.entities.iter().filter(|x| same(&x.kind)).position(|x| x.id == eid).unwrap_or(0) + 1;
        crate::i18n::tr2("ent-named", "what", &crate::i18n::tr(key), "n", &n.to_string())
    }

    /// THE ENTITIES A CONSTRAINT TOUCHES, derived from its points.
    ///
    /// The list of constraints used to name only THE KIND: horizontal, horizontal, vertical, vertical - four
    /// identical rows. On a sketch with thirty constraints such a list is useless: it shows that constraints
    /// exist but gives no way to find the one wanted. Highlighting on hover already existed, but it answers
    /// "where is this one" rather than "which one do I need".
    pub(crate) fn constraint_parts(&self, si: usize, c: &qymcad_core::model::Constraint) -> Vec<String> {
        use qymcad_core::model::EntityKind as EK;
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        let owner = |pid: Id| -> Option<Id> {
            s.entities
                .iter()
                .find(|e| match e.kind {
                    EK::Line { a, b } => a == pid || b == pid,
                    EK::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
                    EK::Circle { center, .. } => center == pid,
                    EK::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
                })
                .map(|e| e.id)
        };
        let mut out: Vec<String> = Vec::new();
        for pid in c.points() {
            if let Some(eid) = owner(pid) {
                let name = self.sketch_entity_name(si, eid);
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }

    /// WHICH REDUNDANT CONSTRAINTS TO MARK - one place for the list of constraints AND for the glyphs on the
    /// canvas.
    ///
    /// The rule used to be written down only in the list, while the glyphs on the canvas were painted from a
    /// raw `diag.redundant`. The divergence looked like this: the slot is clean in the list, while orange
    /// "redundant constraint" glyphs burn on its tangencies in the sketch. It was caught on a picture for the
    /// help - on screen it had been visible for years to anyone who drew a slot.
    ///
    /// WHAT IS EXCLUDED AND WHY:
    /// * **dimensions** - consistent redundancy among dimensions is harmless (a reference dimension is not an
    ///   error);
    /// * **tangencies** (`Tangent`/`CircleTangent`) - their Jacobian at the point of contact is PARALLEL to
    ///   the intrinsic of the arc, both measure the radius in one direction, and the rank analysis marks them
    ///   falsely. A tangency is the structural constraint of a fillet, a slot, a rounded rectangle;
    ///   professional CAD does not mark them red;
    /// * **a virtual corner** - a `PointOnLine` on a point that belongs to no entity (the sharp corner under
    ///   a fillet, kept for the dimensions): the same lie of the rank;
    /// * **everything geometric while the sketch contains tangencies** - the rank lies not only about the
    ///   tangencies themselves but about the constraints entangled with them (the horizontals and verticals
    ///   of a rectangle).
    ///
    /// This hides no real contradictions: those are caught by `sketch_conflicts`, which counts by the
    /// geometry.
    pub(super) fn flagged_redundant(&self, si: usize) -> std::collections::HashSet<usize> {
        use qymcad_core::model::Constraint;
        let diag = self.sketch_diag(si);
        let Some(s) = self.project.sketches.get(si) else { return Default::default() };
        let tangency = |c: &Constraint| matches!(c, Constraint::Tangent { .. } | Constraint::CircleTangent { .. });
        if s.constraints.iter().any(tangency) {
            return Default::default();
        }
        let entity_pts: std::collections::HashSet<Id> = s
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                qymcad_core::model::EntityKind::Line { a, b } => vec![a, b],
                qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                qymcad_core::model::EntityKind::Circle { center, .. } => vec![center],
                qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            })
            .collect();
        diag.redundant
            .iter()
            .copied()
            .filter(|ci| {
                let Some(c) = s.constraints.get(*ci) else { return false };
                let is_dim = matches!(
                    c,
                    Constraint::Distance { .. }
                        | Constraint::Angle { .. }
                        | Constraint::DistancePL { .. }
                        | Constraint::AngleLines { .. }
                        | Constraint::ArcLength { .. }
                        | Constraint::Diameter { .. }
                        | Constraint::EdgeDistance { .. }
                );
                let virtual_corner = matches!(c, Constraint::PointOnLine { p, .. } if !entity_pts.contains(p));
                !is_dim && !tangency(c) && !virtual_corner
            })
            .collect()
    }

    /// Degrees of freedom, redundancy and free points (a wrapper over `sketch_diag` for the older call
    /// sites).
    pub(super) fn sketch_status(&self, si: usize) -> ((i32, i32), Vec<bool>) {
        let d = self.sketch_diag(si);
        (d.dof, d.free)
    }


    /// Selection mode (the arrow): every tool is switched off and geometry is picked by click.
    pub(super) fn sketch_select_mode(&mut self) {
        self.exit_draw_tools(); // the single transition from a mode back to selection
        self.status = crate::i18n::tr("sk-select-hint");
    }


    /// A click with a drawing tool: it adds entities to the active sketch.
    pub(super) fn sketch_tool_click(&mut self, rect: Rect, pos: Pos2) {
        // THE BOUNDARY OF AN OPERATION: a click with a drawing tool is a deliberate act, so it makes one undo
        // step with a name of its own. The step used to be "noticed" by the frame, so what got undone in a
        // sketch was not an action but whatever had accumulated between observations.
        let name = match self.tool.kind {
            1 => &crate::i18n::tr("sk-line"),
            2 => &crate::i18n::tr("sk-rect"),
            3 => &crate::i18n::tr("sk-circle"),
            4 => &crate::i18n::tr("sk-arc"),
            5 => &crate::i18n::tr("sk-point"),
            6 => &crate::i18n::tr("sk-polygon"),
            7 => &crate::i18n::tr("sk-spline"),
            8 => &crate::i18n::tr("sk-ellipse"),
            9 => &crate::i18n::tr("sk-text"),
            11 => &crate::i18n::tr("sk-slot"),
            _ => &crate::i18n::tr("sk-drawing"),
        };
        self.begin_edit(name);
        self.sketch_tool_click_inner(rect, pos);
        self.commit_edit();
    }

    fn sketch_tool_click_inner(&mut self, rect: Rect, pos: Pos2) {
        let w = self.snap_world(rect, pos);
        let Some(si) = self.edit_si() else { return };
        // a new click finishes the previous dimension entry
        self.place.clear(); // everything unfinished in the drawing, all at once
        let con = self.tool.construction;
        match self.tool.kind {
            1 => {
                // a chain of lines: every further click adds a segment
                if let Some(&last) = self.tool.pts.last() {
                    let prev = (self.tool.pts.len() >= 2).then(|| self.tool.pts[self.tool.pts.len() - 2]);
                    self.project.add_line_entity(si, last.x, last.y, w.x, w.y, qymcad_core::feature::Purpose::of(con));
                    if !con && self.set.auto_constrain {
                        // automatic constraints: horizontal or vertical, perpendicular to the previous segment,
                        // point-on-edge
                        self.infer_on_segment(si, prev, last, w);
                    }
                    // stitch the ends to nearby existing vertices, so the corners do not fall apart. The
                    // tolerance is PERCEPTUAL, measured on screen: it used to be clamped to 2.0 mm, so at a
                    // distant zoom it welded ANY points within 2 mm, collapsing small geometry and neighbouring
                    // shapes (parts under 2 mm could not be built at all). Now it is about 8 px with a low
                    // ceiling of 0.4 mm: at high zoom it is nearly nothing (small detail survives), and at any
                    // zoom it merges only what the eye cannot tell apart anyway. Snapping already returns the
                    // exact coordinates of a vertex on a hit.
                    let tol = (8.0 / self.view.scale as f64).clamp(1e-4, 0.4);
                    self.project.merge_close_points(si, tol);
                    self.invalidate();
                }
                self.tool.pts.push(w);
            }
            2 => {
                self.tool.pts.push(w);
                let need = if self.tool_prefs.rect_mode == 2 { 3 } else { 2 };
                if self.tool.pts.len() == need {
                    // (the ids of the sides, corner a and corner b for the width-by-height editor, whether it is
                    // axis-aligned)
                    let (ids, ra, rb, axis_aligned) = match self.tool_prefs.rect_mode {
                        1 => {
                            // centre plus a corner: the opposite corner is its mirror through the centre
                            let (c, cr) = (self.tool.pts[0], self.tool.pts[1]);
                            let a = Point2::new(2.0 * c.x - cr.x, 2.0 * c.y - cr.y);
                            (self.project.add_rect_entity(si, a.x, a.y, cr.x, cr.y, qymcad_core::feature::Purpose::of(con)), a, cr, true)
                        }
                        2 => {
                            // three points give a rotated rectangle
                            let (p1, p2, p3) = (self.tool.pts[0], self.tool.pts[1], self.tool.pts[2]);
                            (self.project.add_rect3_entity(si, p1.x, p1.y, p2.x, p2.y, p3.x, p3.y, qymcad_core::feature::Purpose::of(con)), p1, p2, false)
                        }
                        _ => {
                            let (a, b) = (self.tool.pts[0], self.tool.pts[1]);
                            (self.project.add_rect_entity(si, a.x, a.y, b.x, b.y, qymcad_core::feature::Purpose::of(con)), a, b, true)
                        }
                    };
                    self.tool.pts.clear();
                    self.invalidate();
                    if !con && axis_aligned {
                        self.place.set(PlacingShape::Rect(ra, rb, ids)); // typing the width and height (axis-aligned)
                        self.place.focus = true;
                    }
                }
            }
            3 if self.tool_prefs.circ_mode == 2 => {
                // A TANGENT circle: the first click picks the base edge, the second the centre (the radius
                // follows from the tangency)
                if self.tool.circ_tan.is_none() {
                    if let Some(eref) = self.ref_edge_at(rect, pos, si) {
                        self.tool.circ_tan = Some(eref);
                        self.status = crate::i18n::tr("sk-tangent-base-picked");
                    } else {
                        self.status = crate::i18n::tr("sk-tangent-pick-base");
                    }
                } else if let Some(eref) = self.tool.circ_tan.take() {
                    let r = self.tangent_radius_to_edge(si, eref, w);
                    if r > 1e-6 {
                        let eid = self.project.add_circle_entity(si, w.x, w.y, r, qymcad_core::feature::Purpose::of(con));
                        let cen = self.project.sketch_point_at(si, w.x, w.y, 1e-6);
                        self.add_tangent_to_edge(si, eref, cen, w.x, w.y, r);
                        self.project.solve_sketch(si);
                        self.invalidate();
                        if !con {
                            self.inline = InlineEdit::Circle(eid);
                            self.dim.focus = true;
                        }
                    } else {
                        self.status = crate::i18n::tr("sk-centre-on-edge");
                    }
                }
            }
            3 => {
                self.tool.pts.push(w);
                if self.tool.pts.len() == 2 {
                    let (p0, p1) = (self.tool.pts[0], self.tool.pts[1]);
                    let (cx, cy, r) = if self.tool_prefs.circ_mode == 1 {
                        // by two points: the ends of a diameter
                        let d = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt();
                        (0.5 * (p0.x + p1.x), 0.5 * (p0.y + p1.y), 0.5 * d)
                    } else {
                        // centre plus radius
                        (p0.x, p0.y, ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt())
                    };
                    let eid = self.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                    self.tool.pts.clear();
                    self.invalidate();
                    if !con {
                        self.inline = InlineEdit::Circle(eid); // straight into typing the radius or diameter
                        self.dim.focus = true;
                    }
                }
            }
            4 if self.tool_prefs.arc_mode == 2 => {
                // A TANGENT arc: the start (the end of a line or an arc) plus the end; a smooth continuation
                self.tool.pts.push(w);
                if self.tool.pts.len() == 2 {
                    let (s, e) = (self.tool.pts[0], self.tool.pts[1]);
                    if let Some((t, eref)) = self.arc_tangent_ref(si, s) {
                        if let Some((cx, cy, r, ccw)) = tangent_arc(s, t, e) {
                            self.project.add_arc_entity(si, cx, cy, s.x, s.y, e.x, e.y, winding(ccw), qymcad_core::feature::Purpose::of(con));
                            let cen = self.project.sketch_point_at(si, cx, cy, 1e-6);
                            self.add_tangent_to_edge(si, eref, cen, cx, cy, r);
                            self.project.solve_sketch(si);
                            self.invalidate();
                        } else {
                            self.status = crate::i18n::tr("sk-arc-end-on-tangent");
                        }
                    } else {
                        self.status = crate::i18n::tr("sk-tangent-arc-hint");
                    }
                    self.tool.pts.clear();
                }
            }
            4 => {
                self.tool.pts.push(w);
                if self.tool.pts.len() == 3 {
                    let (p0, p1, p2) = (self.tool.pts[0], self.tool.pts[1], self.tool.pts[2]);
                    if self.tool_prefs.arc_mode == 1 {
                        // by three points: the start, the end and a point on the arc give the circumscribed circle
                        let (s, e, m) = (p0, p1, p2);
                        if let Some((cx, cy, _r)) = circumcircle(s, e, m) {
                            // the orientation start-mid-end: counter-clockwise when the triple turns that way
                            let ccw = (m.x - s.x) * (e.y - s.y) - (m.y - s.y) * (e.x - s.x) > 0.0;
                            self.project.add_arc_entity(si, cx, cy, s.x, s.y, e.x, e.y, winding(ccw), qymcad_core::feature::Purpose::of(con));
                        } else {
                            self.status = crate::i18n::tr("sk-points-collinear-arc");
                        }
                    } else {
                        // the centre, the start and the end
                        let (c, a, b) = (p0, p1, p2);
                        let ccw = (a.x - c.x) * (b.y - c.y) - (a.y - c.y) * (b.x - c.x) > 0.0;
                        self.project.add_arc_entity(si, c.x, c.y, a.x, a.y, b.x, b.y, winding(ccw), qymcad_core::feature::Purpose::of(con));
                    }
                    self.tool.pts.clear();
                    self.invalidate();
                }
            }
            5 => {
                // a point is a single node
                self.project.sketch_point_at(si, w.x, w.y, 1e-6);
                self.invalidate();
            }
            6 => {
                // a polygon: the centre plus a vertex (the number of sides and the kind come from the options bar)
                self.tool.pts.push(w);
                if self.tool.pts.len() == 2 {
                    let (c, vtx) = (self.tool.pts[0], self.tool.pts[1]);
                    let n = self.tool_prefs.poly_n.max(3);
                    let half = std::f64::consts::PI / n as f64;
                    // the radius of the circumscribed circle (through the vertices), by the click mode
                    let r_click = ((vtx.x - c.x).powi(2) + (vtx.y - c.y).powi(2)).sqrt().max(1e-6);
                    let rr = match self.tool_prefs.poly_mode {
                        1 => r_click * half.cos(),                              // inscribed, touching the edges
                        2 => self.tool_prefs.poly_edge.max(0.01) / (2.0 * half.sin()),  // by the length of an edge
                        _ => r_click,                                           // circumscribed, through the vertices
                    };
                    // the vertex direction, normalised to the required radius rr (the angle of the click is kept)
                    let (ux, uy) = ((vtx.x - c.x) / r_click, (vtx.y - c.y) / r_click);
                    // A PARAMETRIC regular polygon: the circumscribed circle plus constraints
                    let (center, _sides) = self.project.add_polygon_param(si, c.x, c.y, c.x + ux * rr, c.y + uy * rr, n, qymcad_core::feature::Purpose::of(con));
                    self.tool.pts.clear();
                    self.invalidate();
                    self.place.set(PlacingShape::Poly(center)); // typing the radius and angle through the centre handle
                    self.place.focus = true;
                }
            }
            7 => {
                // a slot: two centres plus a point that sets the width
                self.tool.pts.push(w);
                if self.tool.pts.len() == 3 {
                    let (a, b, e) = (self.tool.pts[0], self.tool.pts[1], self.tool.pts[2]);
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                    let r = ((e.x - a.x) * (-dy) + (e.y - a.y) * dx).abs() / len;
                    self.project.add_slot_entity(si, a.x, a.y, b.x, b.y, r.max(0.5), qymcad_core::feature::Purpose::of(con));
                    self.tool.pts.clear();
                    self.invalidate();
                }
            }
            8 => {
                // an ellipse: the centre plus a corner of the bounding rectangle, then the width and height are typed
                self.tool.pts.push(w);
                if self.tool.pts.len() == 2 {
                    let (cc, corner) = (self.tool.pts[0], self.tool.pts[1]);
                    let (rx, ry) = ((corner.x - cc.x).abs(), (corner.y - cc.y).abs());
                    // A PARAMETRIC ellipse entity (the axes are perpendicular and the semi-axes can carry
                    // dimensions); the rotation is 0, so the axes follow X and Y
                    let center = self.project.add_ellipse_entity(si, cc.x, cc.y, rx.max(0.5), ry.max(0.5), 0.0, qymcad_core::feature::Purpose::of(con));
                    self.tool.pts.clear();
                    self.invalidate();
                    self.place.set(PlacingShape::Ellipse(center, cc)); // typing the semi-axes through the centre handle
                    self.place.focus = true;
                }
            }
            9 => {
                // a spline: a set of nodes (a double click or Esc finishes it)
                self.tool.pts.push(w);
            }
            10 => {
                // a circle through three points
                self.tool.pts.push(w);
                if self.tool.pts.len() == 3 {
                    if let Some((cx, cy, r)) = circumcircle(self.tool.pts[0], self.tool.pts[1], self.tool.pts[2]) {
                        let eid = self.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                        self.invalidate();
                        if !con {
                            self.inline = InlineEdit::Circle(eid); // straight into typing the diameter, as with an ordinary circle
                            self.dim.focus = true;
                        }
                    } else {
                        self.status = crate::i18n::tr("sk-points-collinear-circle");
                    }
                    self.tool.pts.clear();
                }
            }
            11 if self.tool_prefs.text_note => {
                // a text note, which is not geometry
                if !self.tool_prefs.text.trim().is_empty() {
                    self.project.add_note(si, w.x, w.y, self.tool_prefs.text.clone());
                    self.status = crate::i18n::tr("sk-note-added");
                }
            }
            11 if self.tool_prefs.text.trim().is_empty() => {
                // AN EMPTY STRING IS NO REASON TO STAY SILENT. The tool was armed, the click went through, and
                // nothing happened: there was no telling whether the program was broken or something had been
                // left undone.
                self.status = crate::i18n::tr("sk-text-needs-string");
            }
            11 => {
                // text AS GEOMETRY: a parametric object (the outlines of the glyphs) that can be selected,
                // moved, scaled and retyped - not the loose contours it used to be
                let glyphs = self.bake_text_glyphs(w.x, w.y, self.tool_prefs.text_h, &self.tool_prefs.text.clone());
                let n = glyphs.len();
                if n > 0 {
                    let id = self.project.add_sketch_text(si, w.x, w.y, self.tool_prefs.text_h, 0.0, self.tool_prefs.text.clone(), qymcad_core::feature::Purpose::of(self.tool.construction), glyphs);
                    self.annot.text = self.project.sketches[si].texts.iter().position(|t| t.id == id);
                    self.invalidate();
                    self.view.initialized = false;
                    self.status = crate::i18n::tr1("sk-text-placed", "n", &n.to_string());
                } else if self.default_font().is_none() {
                    self.status = crate::i18n::tr("sk-font-not-found");
                } else {
                    self.status = crate::i18n::tr("sk-text-empty");
                }
            }
            _ => {}
        }
    }


    /// The exact curves of a sketch for a vector export: the edges of the contours (a segment, an arc, a
    /// circle); where no exact edges exist, the polyline of points is used as segments.
    pub(super) fn sketch_export_edges(&self, si: usize) -> Vec<qymcad_core::geom::ProfEdge> {
        use qymcad_core::geom::ProfEdge;
        let mut out = Vec::new();
        let Some(sk) = self.project.sketches.get(si) else { return out };
        for cid in &sk.contour_ids {
            let Some(ci) = self.project.contour_index(*cid) else { continue };
            let c = &self.project.contours[ci];
            if !c.edges.is_empty() {
                out.extend(c.edges.iter().copied());
            } else {
                let n = c.points.len();
                let last = if c.closed { n } else { n.saturating_sub(1) };
                for i in 0..last {
                    out.push(ProfEdge::Line { a: c.points[i], b: c.points[(i + 1) % n] });
                }
            }
        }
        out
    }


    /// The closed contours of sketch `si` (by Id) that will do as the profile of a feature.
    pub(super) fn sketch_closed_contours(&self, si: usize) -> Vec<Id> {
        self.project.sketches.get(si).map(|s| s.contour_ids.iter().copied().filter(|cid| self.project.contour_profile_xy(*cid).is_some()).collect()).unwrap_or_default()
    }


    /// The sketch key layout: drawing (S/L/R/C/A/P/G/E/O/N/T), editing (F corner fillet, M mirror, K trim,
    /// X construction), dimensions (D). The remaining tools (angles, arrays, constraints) go through buttons -
    /// there are not enough letters.
    pub(super) fn sketch_hotkey(&mut self, key: egui::Key) {
        let Some(action) = self.hotkey_action("sketch", key) else { return };
        match action {
            "sketch.select" => {
                self.tool.kind = 0;
                self.dim.kind = 0;
                self.tool.pts.clear();
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
            "sketch.dim" => self.set_dim_tool(1),
            "sketch.corner-fillet" => self.set_click_op(4),
            "sketch.trim" => self.set_click_op(1),
            "sketch.mirror" => self.modify_button(1),
            "sketch.construction" => self.tool.construction = !self.tool.construction,
            _ => {}
        }
    }


    /// The field of the parametric EXPRESSION for feature dimension `key` (just like a sketch dimension): a
    /// number OR something like `w/2+3` over the global parameters. Empty means the stored number is used
    /// (driven by the drag value or the arrow). It commits on losing focus, calling `set_feat_dim` and a
    /// rebuild, and shows either the evaluated value or a cross.
    pub(super) fn dim_expr_field(&mut self, ui: &mut egui::Ui, id: Id, key: &str) {
        self.dim_expr_field_in(ui, id, key, "");
    }


    /// THE SAME FIELD, BUT WITH THE NAME OF THE PLACE it is drawn in.
    ///
    /// One and the same number appears on screen TWICE - in the joint popup and in a row of the right-hand
    /// panel - while the widget name was taken absolutely ("featdim", the joint, the slot). egui complained
    /// in red right in the frame: "First use of widget ID... Second use of widget ID D898...", and one of the
    /// two fields stopped responding (it was caught on a screenshot). The place tells them apart, while the
    /// familiar name stays with the main place - an empty string gives exactly the former key.
    ///
    /// A test facade: the same field the panel draws.
    #[cfg(test)]
    pub(crate) fn dim_expr_field_for_test(&mut self, ui: &mut egui::Ui, id: Id, key: &str) {
        self.dim_expr_field(ui, id, key);
    }

    pub(super) fn dim_expr_field_in(&mut self, ui: &mut egui::Ui, id: Id, key: &str, place: &str) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("ƒ=").weak());
            // THE SAME FIELD AS ON A SKETCH DIMENSION AND IN THE PARAMETER TABLE. It was asked for plainly:
            // features must have all of this too. Different behaviour in two similar places is a defect, even
            // when both of them work.
            let model = self.project.feat_dim(id, key).unwrap_or("").to_string();
            // THE WIDGET NAME COMES FROM THE PLACE IT IS DRAWN IN. An absolute key ("featdim", the joint, the
            // slot) coincided between the joint popup and the row of the right-hand panel, and the two can be
            // on screen AT THE SAME TIME: egui honestly complained in red right in the frame - "First use of
            // widget ID... Second use of widget ID D898..." - and one of the two fields stopped responding.
            // The name of the parent is appended to the key, and the two places stop arguing.
            let fid = if place.is_empty() { egui::Id::new(("featdim", id, key)) } else { egui::Id::new(("featdim", id, key, place)) };
            let o = super::expr_field::expr_field(ui, &self.project, fid, &model, 120.0, &crate::i18n::tr("sk-expr-placeholder"));
            if o.committed && o.text != model {
                // ONE EDIT, ONE UNDO STEP. The edit used to go into the model on losing focus, past
                // `App::edit`, and Ctrl+Z never saw it.
                let ed = self.edit(crate::i18n::tr("par-edit-step"));
                ed.app.project.set_feat_dim(id, key, o.text.clone());
                drop(ed);
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
            }
            // WHAT CAME OUT OF WHAT WAS TYPED, IN WORDS, RIGHT HERE.
            //
            // The field can REFUSE: a bad expression does not reach the model, the letters survive and the
            // caret stays where it was (`ExprOut::refused`). But there was nobody to say WHAT was wrong, and
            // a refusal looked like the program not listening: Enter is pressed, nothing happens, no
            // explanation follows. Exactly the silent refusal that counts as the worst answer in this tree.
            //
            // The speaker (`expr_value_label`) had been written and NEVER ONCE CALLED - a compiler warning
            // said the method was never used, and it had drowned among 175 others. This is where it gets
            // wired up: for feature dimensions and joint values it is the only place an answer is visible.
            self.expr_value_label(ui, &o.text);
            // A DRIVER NAME BELONGS TO A FEATURE PARAMETER TOO. It was asked for plainly: features must have
            // all of this, not sketches alone. The field is the same, in the same place, with the same
            // behaviour: a buffer, a refusal that loses no letters, and the owner named.
            let target = qymcad_core::model::DimTarget::Feature { node: id, key: key.to_string() };
            let cur_name = self.project.name_of_target(&target);
            // THE NAME CHECK IS COMPUTED BEFORE THE EDIT. The closure holds `&self.project` while applying
            // asks for `&mut self`: both cannot be borrowed at once, so the verdict is computed up front and
            // the ready answer is what gets used further on.
            ui.label(&crate::i18n::tr("sk-driver-label")).on_hover_text(&crate::i18n::tr("sk-driver-name-hint"));
            let nid = if place.is_empty() { egui::Id::new(("featdrv", id, key)) } else { egui::Id::new(("featdrv", id, key, place)) };
            let rn = {
                let ok = |nm: &str| nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !self.project.driver_name_taken_by(nm, &target));
                super::expr_field::name_field(ui, &self.project, nid, &cur_name, 100.0, &crate::i18n::tr("sk-name-placeholder"), &ok)
            };
            let nm = rn.text.trim().to_string();
            let name_ok = nm.is_empty() || (qymcad_core::drivers::check_ident(&nm).is_ok() && !self.project.driver_name_taken_by(&nm, &target));
            if rn.committed && nm != cur_name.trim() {
                let ed = self.edit(crate::i18n::tr("sk-driver-step"));
                let old = cur_name.trim().to_string();
                if !old.is_empty() && !nm.is_empty() {
                    let _ = ed.app.project.rename_driver(&old, &nm);
                } else {
                    ed.app.project.name_dim(nm.clone(), target.clone());
                }
                drop(ed);
                if !nm.is_empty() {
                    self.project.mark_param_dependents_dirty_for(&nm);
                }
                self.mark_dirty_for_rebuild();
            }
            if !nm.is_empty() && !name_ok {
                let who = self.project.name_owner(&nm).map(|o| if o.path.is_empty() { crate::i18n::tr("par-owner-project") } else { o.path });
                let msg = match who {
                    Some(w) => crate::i18n::tr2("par-name-taken", "name", &nm, "where", &w),
                    None => crate::i18n::tr("sk-driver-name-bad"),
                };
                ui.label(egui::RichText::new(&msg).color(self.scheme.pal.warning()).small()).on_hover_text(&crate::i18n::tr("sk-driver-name-taken-hint"));
            }
            let shown = o.text;
            if !shown.trim().is_empty() {
                match qymcad_core::expr::eval(&shown, &self.project.param_map()) {
                    Ok(v) => {
                        ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                    }
                    Err(_) => {
                        ui.colored_label(self.scheme.pal.error_mild(), ph::X);
                    }
                }
            }
        });
    }


    pub(super) fn sketch_props(&mut self, ui: &mut egui::Ui, si: usize) {
        let lin = self.lineage_of(Some(self.project.sketches[si].id));
        if let Some(n) = props_header(ui, ph::POLYGON, "sk-props", NameSlot::Editable(self.project.sketches[si].name.clone()), &lin) {
            self.project.sketches[si].name = n;
        }
        // the plane the sketch is placed on
        {
            use qymcad_core::feature::{BasePlane, SketchPlane};
            let pl = match self.project.sketches[si].plane {
                SketchPlane::World(BasePlane::XY) => crate::i18n::tr("sk-plane-xy"),
                SketchPlane::World(BasePlane::XZ) => crate::i18n::tr("sk-plane-xz"),
                SketchPlane::World(BasePlane::YZ) => crate::i18n::tr("sk-plane-yz"),
                SketchPlane::Datum(id) => crate::i18n::tr1("sk-on-work-plane", "id", &id.to_string()),
                SketchPlane::Face(body, _) => crate::i18n::tr1("sk-on-body-face", "b", &body.to_string()),
            };
            ui.label(egui::RichText::new(crate::i18n::tr1("sk-on", "what", &pl)).weak().small());
        }
        let cids = self.project.sketches[si].contour_ids.clone();
        let src = self.project.sketches[si].source;
        ui.label(crate::i18n::tr1("sk-contours-n", "n", &cids.len().to_string()));
        if let Some(srcid) = src {
            if let Some(sf) = self.project.sources.iter().find(|x| x.id == srcid) {
                ui.label(egui::RichText::new(crate::i18n::tr2("sk-source", "name", &sf.name, "kb", &crate::i18n::num(sf.data.len() as f64 / 1024.0, 1))).weak().small());
            }
        }

        // --- while editing: a clean panel, with no clutter ---
        if self.edit_si() == Some(si) {
            use qymcad_core::model::Constraint;
            // system points (the origin and the axes) and their `Fixed` constraints are hidden from the list
            // and the counter, and are never deleted
            let sys_pts: std::collections::HashSet<Id> = self.project.sketches[si].system_ids().into_iter().collect();
            let is_sys = |c: &Constraint| matches!(c, Constraint::Fixed { p } if sys_pts.contains(p));
            let np = self.project.sketches[si].points.iter().filter(|p| !sys_pts.contains(&p.id)).count();
            let nc = self.project.sketches[si].constraints.iter().filter(|c| !is_sys(c)).count();
            ui.label(egui::RichText::new(crate::i18n::tr2("sk-counts", "np", &np.to_string(), "nc", &nc.to_string())).weak().small());
            // the degrees-of-freedom readout (the rank of the Jacobian, THROUGH THE CACHE: it is not computed
            // every frame)
            let diag = self.sketch_diag(si);
            let (dof, redun) = diag.dof;
            let conflicts = diag.conflicts.len();
            let (dline, dcol) = self.sketch_dof_line(si);
            ui.label(egui::RichText::new(dline).color(dcol).small());
            if conflicts > 0 {
                // AN HONEST WORDING: it is A SET of constraints that conflicts, and no single one in it is
                // "the culprit" - any of them can be removed. It used to say that the dimensions contradicted
                // the geometry, although geometric constraints can conflict too, and the geometry has nothing
                // to do with it: it stands where the compromise between incompatible constraints put it.
                ui.label(
                    egui::RichText::new(crate::i18n::tr1("sk-conflicts-n", "n", &conflicts.to_string()))
                        .color(self.scheme.pal.error())
                        .small(),
                );
                ui.label(egui::RichText::new(crate::i18n::tr1("sk-conflict-advice", "icon", ph::RULER)).weak().small());
            } else if redun > 0 {
                ui.label(egui::RichText::new(crate::i18n::tr1("sk-redundant-n", "n", &redun.to_string())).color(self.scheme.pal.note()).small());
            }
            if dof > 0 {
                ui.label(egui::RichText::new(crate::i18n::tr1("sk-dof-n", "n", &dof.to_string())).weak().small());
            }
            ui.separator();
            ui.checkbox(&mut self.win.constraints, &crate::i18n::tr("sk-show-constraints")).on_hover_text(&crate::i18n::tr("sk-show-constraints-hint"));
            ui.separator();
            // the parameters of the project (the named dimensions and formulas)
            if ui.button(format!("{} {}", ph::FUNCTION, crate::i18n::tr("sk-params-btn"))).on_hover_text(&crate::i18n::tr("sk-params-hint")).clicked() {
                self.win.params = true;
            }
            // stitching coincident points cures a corner that has fallen apart on shapes already drawn
            if ui.button(format!("{} {}", ph::LINK, crate::i18n::tr("sk-stitch-btn"))).on_hover_text(&crate::i18n::tr("sk-merge-ends-hint")).clicked() {
                let tol = (10.0 / self.view.scale as f64).clamp(1e-4, 1.0);
                let n = self.project.merge_close_points(si, tol);
                self.project.solve_sketch(si);
                self.invalidate();
                self.status = if n > 0 { crate::i18n::tr1("sk-stitched-n", "n", &n.to_string()) } else { crate::i18n::tr("sk-no-coincident-points") };
            }
            ui.label(egui::RichText::new(&crate::i18n::tr("sk-welcome")).weak().small());
            ui.separator();
            ui.label(egui::RichText::new(&crate::i18n::tr("sk-constraints-and-dims")).strong());
            ui.label(egui::RichText::new(crate::i18n::tr1("sk-list-hint", "icon", ph::TRASH)).weak().small());
            if nc == 0 {
                ui.label(egui::RichText::new(&crate::i18n::tr("sk-not-yet")).weak().small());
            }
            self.hover.constraint = None;
            let mut cons = self.project.sketches[si].constraints.clone();
            // REDUNDANT constraints (which ones exactly over-define the sketch) are marked in the list as
            // candidates for removal. BUT the harmless redundancy of CONSISTENT dimensions (reference ones,
            // not an error) is NOT painted red - that would be a false alarm. The warning appears only on
            // (a) a redundant GEOMETRIC constraint, which does need removing, and (b) a conflicting
            // dimension. They form an interdependent group: removing any marked one lifts the over-definition.
            let conflict_set = diag.conflicts.clone();
            let cur_sel = self.gsel.constraint;
            let (mut rm, mut changed, mut sel_click, mut hov) = (None, false, None, None);
            let mut to_driven: Option<usize> = None;
            // Does the sketch contain fillets (structural tangencies)? Their Jacobian at the point of contact
            // is degenerate (parallel to the intrinsic of the arc), so the rank analysis falsely marks as
            // redundant not only the tangencies themselves but the constraints ENTANGLED with them (the
            // horizontals and verticals of a rectangle, the `PointOnLine` of a virtual corner). While fillets
            // are present, rank redundancy of GEOMETRIC constraints is unreliable, so none of them is painted
            // red (real contradictions of values are still caught by `sketch_conflicts`, which works on the
            // geometry and is reliable).
            let flagged = self.flagged_redundant(si); // ONE rule for the list and for the canvas
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (ci, c) in cons.iter_mut().enumerate() {
                    if is_sys(c) {
                        continue; // a system `Fixed` on the origin or an axis is not shown
                    }
                    let is_sel = cur_sel == Some(ci);
                    // is it a dimension? (consistent redundancy among dimensions is harmless and gets no warning)
                    let is_dim = matches!(c, Constraint::Distance { .. } | Constraint::Angle { .. } | Constraint::DistancePL { .. } | Constraint::AngleLines { .. } | Constraint::ArcLength { .. } | Constraint::Diameter { .. } | Constraint::EdgeDistance { .. });
                    // which redundant ones to mark is decided by ONE rule, shared with the canvas glyphs
                    let redun_geom = flagged.contains(&ci);
                    // A CONFLICTING constraint gets a separate, sharper mark: it is an error (the sketch does
                    // not solve), whereas redundancy only means "this one may be removed". Both used to give
                    // the same glyph.
                    let in_conflict = conflict_set.contains(&ci);
                    let is_driven = c.is_driven();
                    let flag = redun_geom || in_conflict;
                    let row = ui.horizontal(|ui| {
                        if in_conflict {
                            ui.colored_label(self.scheme.pal.error(), ph::WARNING_OCTAGON)
                                .on_hover_text(&crate::i18n::tr("sk-conflict-hint"));
                        } else if flag {
                            ui.colored_label(self.scheme.pal.error_mild(), ph::WARNING).on_hover_text(&crate::i18n::tr("sk-overdefined-hint"));
                        }
                        match c {
                            Constraint::Distance { d, .. } => {
                                if ui.selectable_label(is_sel, &crate::i18n::tr("sk-dim")).clicked() {
                                    sel_click = Some(ci);
                                }
                                changed |= ui.add(egui::DragValue::new(d).speed(0.2).range(0.01..=100000.0).suffix(crate::i18n::tr("unit-mm-suffix"))).changed();
                            }
                            Constraint::Angle { deg, .. } => {
                                if ui.selectable_label(is_sel, &crate::i18n::tr("sk-angle")).clicked() {
                                    sel_click = Some(ci);
                                }
                                changed |= ui.add(egui::DragValue::new(deg).speed(0.5).range(0.1..=359.9).suffix("°")).changed();
                            }
                            other => {
                                // THE PARTICIPANTS IN THE ROW: "Horizontal: Line 3". Without them a list of
                                // thirty constraints shows that constraints exist but gives no way to find the
                                // one wanted - four rows of "Horizontal" in a row are indistinguishable.
                                let parts = self.constraint_parts(si, other);
                                let text = if parts.is_empty() { constraint_label(other) } else { format!("{}: {}", constraint_label(other), parts.join(", ")) };
                                if ui.selectable_label(is_sel, text).clicked() {
                                    sel_click = Some(ci);
                                }
                            }
                        }
                        // RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes a driven one - it
                        // stops driving the geometry but stays on the drawing and shows the actual value. That
                        // is the standard way out in a professional CAD; without it the only option left is to
                        // delete the dimension.
                        if in_conflict && is_dim && !is_driven && ui.small_button(ph::RULER).on_hover_text(&crate::i18n::tr("sk-make-driven-hint")).clicked() {
                            to_driven = Some(ci);
                        }
                        if ui.small_button(ph::TRASH).on_hover_text(&crate::i18n::tr("sk-delete")).clicked() {
                            rm = Some(ci);
                        }
                    })
                    .response;
                    if ui.rect_contains_pointer(row.rect) {
                        hov = Some(ci);
                    }
                }
            });
            self.hover.constraint = hov;
            if let Some(ci) = sel_click {
                self.gsel.constraint = Some(ci);
            }
            if changed {
                self.project.sketches[si].constraints = cons;
                self.project.solve_sketch(si);
                self.invalidate();
            }
            if let Some(ci) = to_driven {
                self.make_dim_driven(si, ci);
            }
            if let Some(ci) = rm {
                self.project.delete_sketch_constraint(si, ci);
                self.gsel.constraint = None;
                self.invalidate();
            }
            return;
        }

        // NOT in edit mode: only what is relevant. A compact summary plus a way into editing. Geometry,
        // dimensions and constraints are edited INSIDE the sketch (through Edit or a double click). A body is
        // made from a sketch by the Extrude or Revolve command on the toolbar - a parametric feature in the
        // timeline - rather than by one-off buttons here.
        let (dline, dcol) = self.sketch_dof_line(si);
        ui.separator();
        ui.label(egui::RichText::new(dline).color(dcol).small());
        ui.add_space(2.0);
        if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, crate::i18n::tr("sk-edit-btn"))).clicked() {
            self.enter_sketch_edit(si);
        }
        ui.label(egui::RichText::new(&crate::i18n::tr("sk-body-from-sketch")).weak().small());

        ui.separator();
        if ui.button(format!("{} {}", ph::TRASH, crate::i18n::tr("act-delete-sketch"))).clicked() {
            self.ask_delete(Sel::Sketch(si)); // the same path the tree takes - the question and the deletion
        }
    }


    /// The ANGLE popup for the rotate tool (`move_op == 3`), placed at the picked centre. Enter or the tick
    /// applies `rotate_entities`; editing the value gives a live preview (the ghost in `draw_move_preview`).
    pub(super) fn sketch_rotate_popup(&mut self, ctx: &egui::Context, rect: Rect) {
        if self.tool.move_op != 3 {
            return;
        }
        let Sel::Sketch(si) = self.sel else { return };
        let Some(base) = self.tool.move_base else { return };
        let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        if eids.is_empty() {
            return;
        }
        let at = self.to_screen(rect, base);
        let want_focus = std::mem::take(&mut self.rot.focus);
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let (mut chg, mut apply, mut got_focus) = (false, false, false);
        let mut buf = std::mem::take(&mut self.rot.buf);
        egui::Area::new(egui::Id::new(("rotinput", si))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{} {}", ph::ARROWS_CLOCKWISE, crate::i18n::tr("sk-angle-deg")));
                    let r = Self::focus_edit(ui, &mut buf, 64.0, &crate::i18n::tr("sk-angle-placeholder"), want_focus);
                    chg |= r.changed();
                    got_focus |= r.has_focus();
                    if r.lost_focus() && enter {
                        apply = true;
                    }
                    if ui.button(ph::CHECK).on_hover_text(&crate::i18n::tr("sk-apply-enter")).clicked() {
                        apply = true;
                    }
                });
            });
        });
        self.rot.buf = buf;
        if got_focus {
            self.rot.focus = false;
        }
        if chg || want_focus {
            self.rot.angle = self.parse_num(&self.rot.buf.clone()).unwrap_or(0.0); // the live preview
        }
        if apply {
            if let Some(v) = self.parse_num(&self.rot.buf.clone()) {
                self.project.rotate_entities(si, &eids, base.x, base.y, v);
                self.status = crate::i18n::tr1("sk-rotated-by", "a", &v.to_string());
            }
            self.tool.move_op = 0;
            self.tool.move_base = None;
            self.invalidate();
        }
    }


    /// The candidate planes for a sketch: the world XY, XZ and YZ plus the datum planes, with their frames.
    pub(super) fn sketch_plane_candidates(&self) -> Vec<(qymcad_core::feature::SketchPlane, qymcad_core::feature::PlaneFrame)> {
        use qymcad_core::feature::{BasePlane, PlaneFrame, SketchPlane};
        let mut v = vec![
            (SketchPlane::World(BasePlane::XY), BasePlane::XY.frame()),
            (SketchPlane::World(BasePlane::XZ), BasePlane::XZ.frame()),
            (SketchPlane::World(BasePlane::YZ), BasePlane::YZ.frame()),
        ];
        for p in &self.project.planes {
            // only the datums visible in the current context (not hidden by a tick box, not belonging to
            // another component) - otherwise an invisible datum of a part could be picked as the plane of a
            // sketch from inside an assembly. The same holds for picking a mirror or a section plane: what
            // gets drawn are the datums of THE assembly currently open, with no exceptions for direct
            // children - only the datums OF the current context (plus those of its ancestors while working in
            // context, as usual). The frame is carried by the transform of the owner: a datum travels with
            // its part.
            if let Some(wt) = self.datum_render_transform(p.id) {
                let fr = PlaneFrame::from_origin_normal(p.origin, p.normal, p.rot_deg);
                let fr = if qymcad_core::feature::is_identity12(&wt) { fr } else { fr.transformed(&wt) };
                v.push((SketchPlane::Datum(p.id), fr));
            }
        }
        v
    }


    /// The anchor point for THE ORIGIN of a new sketch, as (u, v) in the axes of plane `sp`.
    /// The priority: a vertex of a body (a firmer anchor) over a point on an edge. `None` means there is no
    /// geometry nearby.
    pub(super) fn sketch_origin_snap(&self, rect: Rect, pos: Pos2, sp: &qymcad_core::feature::SketchPlane) -> Option<Point2> {
        let w = self.pick_vertex_pos(rect, pos).or_else(|| self.pick_edge_point(rect, pos))?;
        let fr = self.world_frame_of_plane(sp)?;
        Some(fr.project(qymcad_core::geom::Point3::new(w[0], w[1], w[2])))
    }


    /// RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes A DRIVEN one - it stops driving the
    /// geometry but stays on the drawing and shows the actual value. That is the standard way out in a
    /// professional CAD; without it the only option left is to delete the dimension and lose it from the
    /// drawing.
    ///
    /// It lives here rather than in the body of the panel: it is an operation on the document, and a test
    /// drives it.
    pub(super) fn make_dim_driven(&mut self, si: usize, ci: usize) -> bool {
        if self.project.auto_driven(si, ci) {
            self.project.solve_sketch(si);
            self.invalidate();
            self.status = crate::i18n::tr("sk-dim-now-driven");
            true
        } else {
            self.status = crate::i18n::tr("sk-cannot-be-driven");
            false
        }
    }

    /// The glyphs actually shown in the viewport. The "show constraints" toggle hides ORDINARY constraints
    /// but NOT the conflicting ones: an error cannot be switched off by a tick box - otherwise the conflict
    /// exists while the viewport is empty.
    pub(super) fn visible_constraint_glyphs(&self, rect: Rect, si: usize) -> Vec<(usize, Pos2, Gly)> {
        if self.win.constraints {
            return self.constraint_glyphs(rect, si);
        }
        let conflicts = self.sketch_diag(si).conflicts;
        if conflicts.is_empty() {
            return Vec::new();
        }
        self.constraint_glyphs(rect, si).into_iter().filter(|(ci, _, _)| conflicts.contains(ci)).collect()
    }

    /// The positions of the constraint glyphs: (the index of the constraint, the screen point, the symbol).
    /// One source for both drawing and hit testing (deleting by click).
    pub(super) fn constraint_glyphs(&self, rect: Rect, si: usize) -> Vec<(usize, Pos2, Gly)> {
        use qymcad_core::model::Constraint;
        let Some(s) = self.project.sketches.get(si) else { return Vec::new() };
        let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
        let mid = |a: Id, b: Id| -> Option<Pos2> {
            let (pa, pb) = (pt(a)?, pt(b)?);
            Some(((self.to_screen(rect, pa).to_vec2() + self.to_screen(rect, pb).to_vec2()) / 2.0).to_pos2())
        };
        let mut out: Vec<(usize, Pos2, Gly)> = Vec::new();
        // THE SAME GLYPH AT THE SAME PLACE IS DRAWN ONCE.
        //
        // A regular hexagon is held by FIVE equality constraints, and all five share the same first side.
        // That gave five glyphs at one point, and spreading them out carried them 17 pixels to the right
        // each - on a shot of a sketch run they lined up in a row IN EMPTY SPACE, attached to nothing. There
        // is no sense in four copies: "this side equals the others" is said once. The remaining constraints
        // do not go anywhere - they are in the list of constraints, and they are picked from there.
        let mut seen: std::collections::HashSet<(u8, i32, i32)> = std::collections::HashSet::new();
        // THE LAYOUT SIZES OF THE GLYPHS ARE NAMED, not written as numbers on the spot. This is THE DISTANCE
        // BETWEEN badges, not a grab radius (that lives in `grab.rs` and obeys the pointing-precision
        // setting). The guard over the radii told these two meanings apart by an accident of formatting - by
        // the word `guard` on the same line; splitting the lines was enough to make it go red on code that
        // had not changed in substance.
        const BADGE_GAP: f32 = 16.0;
        const BADGE_STEP: f32 = 17.0;
        // The badge is placed above the point or the midpoint and spread out NEAR IT rather than in a row to
        // the right: a glyph a hundred pixels away from its geometry lies worse than an overlapped one.
        let place = |out: &mut Vec<(usize, Pos2, Gly)>, seen: &mut std::collections::HashSet<(u8, i32, i32)>, ci: usize, p: Option<Pos2>, g: Gly| {
            if let Some(p) = p {
                if !seen.insert((g as u8, (p.x / 4.0).round() as i32, (p.y / 4.0).round() as i32)) {
                    return;
                }
                let base = p + egui::vec2(0.0, -15.0);
                let mut at = base;
                for k in 1..9 {
                    if !out.iter().any(|(_, q, _)| q.distance(at) < BADGE_GAP) {
                        break;
                    }
                    // in a snake: two steps to the right, then a row lower - a compact block by the geometry
                    at = base + egui::vec2(BADGE_STEP * (k % 3) as f32, -BADGE_STEP * (k / 3) as f32);
                }
                out.push((ci, at, g));
            }
        };
        let mut push = |out: &mut Vec<(usize, Pos2, Gly)>, ci: usize, p: Option<Pos2>, g: Gly| place(out, &mut seen, ci, p, g);
        for (ci, c) in s.constraints.iter().enumerate() {
            match *c {
                Constraint::Horizontal { a, b } => push(&mut out, ci, mid(a, b), Gly::Horiz),
                Constraint::Vertical { a, b } => push(&mut out, ci, mid(a, b), Gly::Vert),
                Constraint::Coincident { a, .. } => push(&mut out, ci, pt(a).map(|p| self.to_screen(rect, p) + egui::vec2(8.0, 8.0)), Gly::Coincident),
                Constraint::Parallel { a, b, c: cc, d } => {
                    push(&mut out, ci, mid(a, b), Gly::Parallel);
                    push(&mut out, ci, mid(cc, d), Gly::Parallel);
                }
                Constraint::Perpendicular { a, b, c: cc, d } => {
                    push(&mut out, ci, mid(a, b), Gly::Perp);
                    push(&mut out, ci, mid(cc, d), Gly::Perp);
                }
                Constraint::Equal { a, b, c: cc, d } => {
                    push(&mut out, ci, mid(a, b), Gly::Equal);
                    push(&mut out, ci, mid(cc, d), Gly::Equal);
                }
                Constraint::Collinear { a, b, c: cc, d } => {
                    push(&mut out, ci, mid(a, b), Gly::Collinear);
                    push(&mut out, ci, mid(cc, d), Gly::Collinear);
                }
                Constraint::Tangent { a, b, .. } => push(&mut out, ci, mid(a, b), Gly::Tangent),
                Constraint::CircleTangent { c1, c2, .. } => push(&mut out, ci, mid(c1, c2), Gly::Tangent),
                Constraint::Symmetric { a, b, .. } => push(&mut out, ci, mid(a, b), Gly::Symmetric),
                // a `Fixed` on SYSTEM points (the origin, the ends of the axes) is not a constraint placed by
                // hand but a service anchor. No glyph is drawn for it (otherwise a dimension to an axis looks
                // as if it were breeding extra constraints).
                Constraint::Fixed { p } if !s.system_ids().contains(&p) => push(&mut out, ci, pt(p).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::Fix),
                Constraint::PointOnLine { p, .. } => push(&mut out, ci, pt(p).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::PointOnLine),
                Constraint::Midpoint { p, .. } => push(&mut out, ci, pt(p).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::Midpoint),
                Constraint::EqualRadius { c1, c2 } => {
                    push(&mut out, ci, pt(c1).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::Equal);
                    push(&mut out, ci, pt(c2).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::Equal);
                }
                Constraint::PointOnCircle { p, .. } => push(&mut out, ci, pt(p).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::PointOnCircle),
                Constraint::Concentric { c1, .. } => push(&mut out, ci, pt(c1).map(|q| self.to_screen(rect, q) + egui::vec2(8.0, 8.0)), Gly::Concentric),
                _ => {}
            }
        }
        out
    }


    /// The index of the constraint whose glyph is nearest to a screen point (for deleting by click).
    pub(super) fn constraint_glyph_at(&self, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
        self.constraint_glyphs(rect, si).into_iter().find(|(_, at, _)| at.distance(pos) <= self.grab(Grab::Label)).map(|(ci, _, _)| ci)
    }


    /// The edges of the REFERENCE body, projected into the 2D frame of sketch `si` (polylines in sketch
    /// coordinates). The body is a face of ITS OWN part (`SketchPlane::Face`) always, or a NEIGHBOUR (an
    /// in-context datum snapshot) only during the creation session (`sketch_ref_body`). They come from the
    /// `shape` and land exactly. Empty when there is no reference or no frame.
    ///
    /// The same as [`App::sketch_ref_edges_2d`], but WITH THE NAMES of the edges and the source body - for
    /// PICKING: to project an edge it is not enough to draw it, one has to know which edge was clicked.
    pub(super) fn sketch_ref_edges_2d_ids(&self, si: usize) -> (Id, Vec<(u32, Vec<Point2>)>) {
        use qymcad_core::feature::SketchPlane;
        let Some(s) = self.project.sketches.get(si) else { return (0, Vec::new()) };
        let body = match s.plane {
            SketchPlane::Face(b, _) => Some(self.project.live_body(b)),
            _ => self.cmd.ref_body,
        };
        let (Some(body), Some(frame)) = (body, self.project.sketch_frame(si)) else { return (0, Vec::new()) };
        let Some(shape) = self.live.shapes.get(&body) else { return (0, Vec::new()) };
        let rel = match (self.project.sketch_owner(s.id), self.project.body_owner(body)) {
            (Some(so), Some(bo)) if so != bo => self.project.relative_transform(bo, so),
            _ => qymcad_core::feature::PLACE_IDENTITY,
        };
        let ident = qymcad_core::feature::is_identity12(&rel);
        let Some(edges) = self.body_edges_cached(body) else { return (0, Vec::new()) };
        let (polys, ids) = (&edges.0, &edges.1);
        let face_edges: Option<std::collections::HashSet<u32>> = match &s.plane {
            SketchPlane::Face(_, fid) if fid.id != 0 => Some(shape.face_edge_ids(fid.id).into_iter().collect()),
            _ => None,
        };
        let out = polys
            .iter()
            .zip(ids.iter().copied())
            .filter(|(_, id)| *id != 0 && face_edges.as_ref().is_none_or(|set| set.contains(id)))
            .map(|(poly, id)| {
                let pts = poly
                    .iter()
                    .map(|p| {
                        let mut l = [p[0] as f64, p[1] as f64, p[2] as f64];
                        if !ident {
                            l = qymcad_core::feature::apply12(&rel, l);
                        }
                        frame.project(qymcad_core::geom::Point3::new(l[0], l[1], l[2]))
                    })
                    .collect();
                (id, pts)
            })
            .collect();
        (body, out)
    }

    /// The underlay edge under the cursor (the name of the edge plus the body), by distance in SCREEN pixels,
    /// as with every other sketch pick: in world millimetres the threshold would depend on the zoom.
    pub(super) fn nearest_ref_edge(&self, si: usize, rect: Rect, pos: Pos2) -> Option<(Id, u32)> {
        let (body, edges) = self.sketch_ref_edges_2d_ids(si);
        if body == 0 {
            return None;
        }
        let mut best: Option<(f32, u32)> = None;
        for (id, poly) in &edges {
            for w in poly.windows(2) {
                let (a, b) = (self.to_screen(rect, w[0]), self.to_screen(rect, w[1]));
                // the distance to the segment in screen pixels (a projection onto it, clamped to the ends)
                let (vx, vy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
                let (wx, wy) = ((pos.x - a.x) as f64, (pos.y - a.y) as f64);
                let len2 = vx * vx + vy * vy;
                let t = if len2 > 1e-12 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                let d = ((wx - vx * t).powi(2) + (wy - vy * t).powi(2)).sqrt() as f32;
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, *id));
                }
            }
        }
        best.filter(|(d, _)| *d <= self.grab(Grab::Curve)).map(|(_, id)| (body, id))
    }

    /// Project into the sketch whatever was clicked: an edge of the underlay, or - in "face contour" mode -
    /// the whole contour of the face the sketch stands on.
    pub(super) fn project_clicked_edge(&mut self, si: usize, rect: Rect, pos: Pos2) {
        use qymcad_core::feature::SketchPlane;
        use qymcad_core::model::ProjSource;
        // "FACE CONTOUR" MODE works only for a sketch seated ON A FACE: a sketch on a world plane has no host
        // face, and projecting its contour would mean guessing.
        let face_mode = self.tool.proj_face;
        let src = match (face_mode, self.project.sketches.get(si).map(|s| s.plane.clone())) {
            (true, Some(SketchPlane::Face(b, key))) if key.id != 0 => Some((self.project.live_body(b), ProjSource::Face(key.id))),
            (true, _) => {
                self.status = crate::i18n::tr("sk-face-outline-only");
                return;
            }
            _ => self.nearest_ref_edge(si, rect, pos).map(|(b, e)| (b, ProjSource::Edge(e))),
        };
        let Some((body, src)) = src else {
            self.status = crate::i18n::tr("sk-miss-click-edge");
            return;
        };
        self.begin_edit(&crate::i18n::tr("sk-project")); // THE BOUNDARY OF AN OPERATION, as with other sketch edits
        let id = self.with_kernel(|app, k| app.project.add_sketch_projection(si, body, src, k));
        self.status = if id == 0 {
            crate::i18n::tr("sk-not-projectable")
        } else {
            crate::i18n::tr("sk-projected-hint")
        };
        self.project.solve_sketch(si);
        self.commit_edit();
        self.invalidate();
    }

    pub(super) fn sketch_ref_edges_2d(&self, si: usize) -> Vec<Vec<Point2>> {
        use qymcad_core::feature::SketchPlane;
        let s = match self.project.sketches.get(si) { Some(s) => s, None => return Vec::new() };
        // THE BODY IS TAKEN LIVE, not the one recorded when the sketch was created.
        //
        // `SketchPlane::Face(b, _)` holds the id of the body AT THE MOMENT of creation, while every subsequent
        // operation creates a new body and consumes the previous one. It looked like this: extrude a square,
        // make a hole, cut the chamfers - and the sketcher shows an overlay of both the original square and
        // the chamfered body. There was no overlay: exactly one contour was drawn, but of the CONSUMED body,
        // that is, geometry out of the past on top of the current model.
        let body = match s.plane {
            SketchPlane::Face(b, _) => Some(self.project.live_body(b)),
            _ => self.cmd.ref_body,
        };
        let (Some(body), Some(frame)) = (body, self.project.sketch_frame(si)) else { return Vec::new() };
        let Some(shape) = self.live.shapes.get(&body) else { return Vec::new() };
        let rel = match (self.project.sketch_owner(s.id), self.project.body_owner(body)) {
            (Some(so), Some(bo)) if so != bo => self.project.relative_transform(bo, so),
            _ => qymcad_core::feature::PLACE_IDENTITY,
        };
        let ident = qymcad_core::feature::is_identity12(&rel);
        // a sketch on A FACE of a part projects the edges of THAT FACE ONLY (its contour: the outer one plus
        // the holes) rather than EVERY edge of the body. Otherwise the edges of the bottom and the sides were
        // flattened onto the plane of the sketch over the contour of the face, and both the square (the
        // unfilleted edges below and to the side) and the fillets showed at once. A non-zero `face_id` means
        // filtering by that face.
        // From the cache: this projection is drawn on every frame of a sketch edit, while pulling the edges
        // out of the B-rep costs as much as a rebuild - on a large part, drawing on a face itself became
        // ragged.
        let Some(edges) = self.body_edges_cached(body) else { return Vec::new() };
        let (polys, ids) = (&edges.0, &edges.1);
        let face_edges: Option<std::collections::HashSet<u32>> = match &s.plane {
            SketchPlane::Face(_, fid) if fid.id != 0 => Some(shape.face_edge_ids(fid.id).into_iter().collect()),
            _ => None,
        };
        polys
            .iter()
            .zip(ids.iter().copied())
            .filter(|(_, id)| face_edges.as_ref().map_or(true, |set| set.contains(id)))
            .map(|(poly, _)| poly)
            .map(|poly| {
                poly.iter()
                    .map(|p| {
                        let mut l = [p[0] as f64, p[1] as f64, p[2] as f64];
                        if !ident {
                            l = qymcad_core::feature::apply12(&rel, l);
                        }
                        frame.project(qymcad_core::geom::Point3::new(l[0], l[1], l[2]))
                    })
                    .collect()
            })
            .collect()
    }
}

// THE SKETCH VIEWPORT moved here from `gui.rs` whole, along with its phases: the start of a drag, the drag
// itself, the click, the drawing. It used to lie in the root as one body of 959 lines, and moving it was
// impossible: the phases had neither names nor boundaries.
impl App {
    /// THE FLAT SKETCH VIEWPORT: panning and zooming, the drawing tools, picking entities, drawing.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn viewport_2d(&mut self, ctx: &egui::Context, resp: &egui::Response, painter: &egui::Painter, rect: Rect, has_geom: bool, scroll: f32) {
                if !self.view.initialized && has_geom {
                    self.fit(rect);
                }
                let ctrl = ctx.input(|i| i.modifiers.ctrl);
                let handle = self.selected_centroid();
                // the priority of starting a drag: a sketch point, then the gizmo (on a handle), then the
                // selection box (Ctrl), then panning.
                // GRABBING GEOMETRY happens with the LEFT button only. The middle button pans the canvas;
                // a middle drag over a point or a dimension used to start dragging the geometry and broke the
                // drawing.
                // WHAT EXACTLY WAS GRABBED: the priority chain that reads the start of a drag
                self.sketch_drag_start(ctx, resp, rect, ctrl, handle);
                // WHILE DRAGGING: every link carries ITS OWN object until the release
                self.sketch_drag_update(ctx, resp, rect);
                self.power_trim_drag(resp, rect); // trimming by dragging (the trim tool)
                // panning with the middle button (in a sketch the left button on empty space is a selection box)
                if ctx.input(|i| i.pointer.middle_down()) {
                    let d = ctx.input(|i| i.pointer.delta());
                    self.view.center.x -= d.x / self.view.scale;
                    self.view.center.y += d.y / self.view.scale;
                }
                if scroll != 0.0 && resp.hovered() {
                    self.view.scale = (self.view.scale * (scroll * 0.002).exp()).clamp(0.02, 800.0);
                }
                // A CLICK IN A SKETCH: a drawing tool, placing a dimension, or picking geometry
                self.sketch_click(ctx, resp, rect);
                // the right button opens the context menu of the sketch
                if let Sel::Sketch(si) = self.sel {
                    if self.edit_si() == Some(si) {
                        // if something unselected was clicked, pick it before the menu opens
                        if resp.secondary_clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                if let Some(h) = self.sketch_hit(rect, pos, si) {
                                    if !self.sel_sk.items.contains(&h) {
                                        self.sel_sk.items = vec![h];
                                    }
                                }
                            }
                        }
                        resp.context_menu(|ui| {
                            let has_sel = !self.sel_sk.items.is_empty();
                            let has_ent = self.sel_sk.items.iter().any(|(k, _)| *k == 1);
                            if ui.add_enabled(has_sel, egui::Button::new(format!("{} {}", ph::TRASH, crate::i18n::tr("props-delete")))).clicked() {
                                self.delete_sketch_sel(si);
                                ui.close_menu();
                            }
                            if ui.add_enabled(has_ent, egui::Button::new(&crate::i18n::tr("sk-construction-toggle"))).clicked() {
                                let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                                self.project.toggle_construction(si, &eids);
                                self.project.solve_sketch(si);
                                self.invalidate();
                                ui.close_menu();
                            }
                            // an arc length dimension, for the picked arc
                            let arc_eid = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, id)| {
                                self.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == *id)).and_then(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. }).then_some(*id))
                            });
                            if let Some(aid) = arc_eid {
                                if ui.button(&crate::i18n::tr("sk-arc-length-dim")).clicked() {
                                    if let Some(ci) = self.project.ensure_arc_length(si, aid) {
                                        self.finish_dim(si, ci);
                                        self.place.dim = Some(ci);
                                    }
                                    ui.close_menu();
                                }
                            }
                            // a tangent (edge-to-edge) dimension: two references are picked, at least one of
                            // them a circle or an arc
                            let edge_refs: Vec<(Id, i8)> = self.project.sketches.get(si).map(|s| {
                                self.sel_sk.items.iter().filter_map(|&(k, id)| {
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
                            if ui.add_enabled(can_edge, egui::Button::new(&crate::i18n::tr("sk-tangent-dim"))).on_hover_text(&crate::i18n::tr("sk-gap-hint")).clicked() {
                                let ((c1, m1), (c2, m2)) = (edge_refs[0], edge_refs[1]);
                                let d = self.project.measure_edge_distance(si, c1, m1, c2, m2);
                                let ci = self.project.sketches[si].constraints.len();
                                self.project.sketches[si].constraints.push(qymcad_core::model::Constraint::EdgeDistance { c1, c2, d, m1, m2, off: 0.0, expr: String::new(), driven: false });
                                self.finish_dim(si, ci);
                                self.gsel.constraint = Some(ci);
                                self.invalidate();
                                ui.close_menu();
                            }
                            ui.separator();
                            ui.menu_button(&crate::i18n::tr("sk-constraint"), |ui| {
                                for (label, code) in [(&crate::i18n::tr("sk-coincident"), 0u8), (&crate::i18n::tr("sk-horizontal"), 1), (&crate::i18n::tr("sk-vertical"), 2), (&crate::i18n::tr("sk-parallel"), 3), (&crate::i18n::tr("sk-perpendicular"), 4), (&crate::i18n::tr("sk-equal"), 5), (&crate::i18n::tr("sk-tangency"), 9), (&crate::i18n::tr("sk-midpoint"), 11), (&crate::i18n::tr("sk-fix"), 6)] {
                                    if ui.button(label).clicked() {
                                        self.constraint_button(code);
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.separator();
                            if ui.button(&crate::i18n::tr("sk-select-all")).clicked() {
                                self.select_all_sketch(si);
                                ui.close_menu();
                            }
                        });
                    }
                }
                // a double click finishes a chain of lines; otherwise it edits a dimension
                if resp.double_clicked() && self.workbench == Workbench::Sketch {
                    if self.tool.kind == 1 {
                        self.tool.pts.clear(); // finish the current chain; the tool stays armed
                    } else if self.tool.kind == 9 {
                        self.finish_spline();
                    } else if let (Sel::Sketch(si), Some(pos)) = (self.sel, resp.interact_pointer_pos()) {
                        // a copy of an array opens its parameters; a text object opens for editing; so do a
                        // note, a dimension and a circle
                        if let Some(pi) = self.sketch_hit(rect, pos, si).filter(|(k, _)| *k == 1).and_then(|(_, eid)| self.project.pattern_of_entity(si, eid)) {
                            // a double click on a copy of an array edits the count and the step, with a preview
                            use qymcad_core::model::PatternKind;
                            self.pat.edit = Some(pi);
                            match self.project.sketches[si].patterns[pi].kind {
                                PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
                                    self.pat.op = 1;
                                    self.sk_pat.dx = dx;
                                    self.sk_pat.dy = dy;
                                    self.sk_pat.count = count;
                                    self.sk_pat.dx2 = dx2;
                                    self.sk_pat.dy2 = dy2;
                                    self.sk_pat.count2 = count2.max(1);
                                    self.pat.center = None;
                                }
                                PatternKind::Circular { cx, cy, count, total_deg } => {
                                    self.pat.op = 2;
                                    self.sk_pat.count = count;
                                    self.sk_pat.angle = total_deg;
                                    self.pat.center = Some(Point2::new(cx, cy)); // the centre from the record (it can be re-picked)
                                }
                            }
                            self.status = crate::i18n::tr("sk-array-edit-hint");
                        } else if let Some(ti) = self.text_at(rect, pos, si) {
                            self.inline = InlineEdit::Text(ti);
                            self.annot.text = Some(ti);
                            let t = &self.project.sketches[si].texts[ti];
                            self.annot.text_buf = t.text.clone();
                            self.annot.text_h = t.height;
                        } else if let Some(ni) = self.note_at(rect, pos, si) {
                            self.inline = InlineEdit::Note(ni);
                            self.annot.note_buf = self.project.sketches[si].notes.get(ni).map(|n| n.text.clone()).unwrap_or_default();
                        } else if let Some(ci) = self.dim_at(rect, pos, si) {
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
                                self.place.set(PlacingShape::Poly(c));
                                self.place.focus = true;
                            } else {
                                self.inline = InlineEdit::Dim(ci);
                                self.dim.focus = true;
                            }
                        } else if let Some(eid) = self.nearest_circle_entity(rect, pos, si) {
                            // a circle gets a diameter dimension; an arc has its radius edited
                            let center = self.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
                                qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
                                _ => None,
                            });
                            // the circumscribed circle of a polygon (the vertices hang on it) opens the polygon
                            // popup (the radius plus the angle), while an ordinary circle gets a diameter
                            let is_poly_rim = center.is_some_and(|c| self.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)));
                            if let (true, Some(c)) = (is_poly_rim, center) {
                                self.place.set(PlacingShape::Poly(c));
                                self.place.focus = true;
                            } else if let Some(c) = center {
                                if let Some(ci) = self.project.ensure_diameter(si, c, true) {
                                    self.inline = InlineEdit::Dim(ci);
                                    self.dim.focus = true;
                                }
                            } else {
                                self.inline = InlineEdit::Circle(eid);
                                self.dim.focus = true;
                            }
                        } else if let Some(cid) = self.polygon_under(rect, pos, si) {
                            self.place.set(PlacingShape::Poly(cid)); // editing the radius of the construction circle
                            self.place.focus = true;
                        }
                    }
                }
                // the cursor with snapping (it refreshes the snap hint for the marker)
                self.cursor = resp.hover_pos().map(|p| self.snap_world(rect, p));
                // the pre-select highlight: what is under the cursor - only while the sketch is in selection mode
                self.hover.sketch = None;
                if let (Sel::Sketch(si), Some(hp)) = (self.sel, resp.hover_pos()) {
                    if self.edit_si() == Some(si) && self.tool.kind == 0 && self.dim.kind == 0 && !resp.dragged() && self.drag.pt().is_none() && self.drag.mov().is_none() {
                        self.hover.sketch = self.sketch_hit(rect, hp, si);
                        // the constraint glyph under the cursor is highlighted, without wiping the hover coming
                        // from the list of constraints
                        if self.hover.sketch.is_none() {
                            if let Some(gc) = self.constraint_glyph_at(rect, hp, si) {
                                self.hover.constraint = Some(gc);
                            }
                        }
                    }
                }
                self.update_placing_dim(rect); // the dimension follows the cursor until it is placed
                if resp.hover_pos().is_none() {
                    self.snap_hint = None;
                }
                if self.sketch_ses.editing.is_some() {
                    self.draw_sketch_grid(&painter, rect); // the grid and the axes while a sketch is being edited
                }
                // DRAWING THE SKETCH VIEWPORT is the last phase of the frame: the input has been read by now
                self.draw_sketch_viewport(ctx, resp, &painter, rect, handle);
    }

    /// THE START OF A DRAG IN A SKETCH: what exactly was grabbed - a text, a note, a dimension caption, a
    /// point, a spline handle, the move gizmo or a selection box.
    ///
    /// This is A PRIORITY CHAIN: the links are tried in order, and the first one that fires takes the drag
    /// for itself. While it lay inside the viewport the order of the links was invisible, and each link
    /// checked "is it taken" with ITS OWN set of conditions, which had drifted apart: dragging a text still
    /// fired the dimension branch. Now the chain has a name, and the question
    /// "is something already being dragged?" is asked in exactly one place - `Dragging::active`.
    pub(super) fn sketch_drag_start(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, ctrl: bool, handle: Option<qymcad_core::geom::Point2>) {
                if resp.drag_started() && ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                    let pp = resp.interact_pointer_pos();
                    // moving a text object (the highest priority among the captions)
                    if !self.drag.active() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if let Some(ti) = self.text_at(rect, pp, si) {
                                self.drag = Dragging::Text(ti);
                                self.annot.text = Some(ti);
                            }
                        }
                    }
                    // moving a note (it takes priority)
                    if !self.drag.active() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if let Some(ni) = self.note_at(rect, pp, si) {
                                self.drag = Dragging::Note(ni);
                            }
                        }
                    }
                    // offsetting a dimension line: the caption of a linear dimension is dragged
                    if !self.drag.active() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if let Some(ci) = self.dim_at(rect, pp, si) {
                                // a diameter or radius, an arc length and a tangent distance are draggable too -
                                // their labels used to be impossible to move.
                                if matches!(self.project.sketches[si].constraints.get(ci), Some(qymcad_core::model::Constraint::Distance { .. }) | Some(qymcad_core::model::Constraint::DistancePL { .. }) | Some(qymcad_core::model::Constraint::Diameter { .. }) | Some(qymcad_core::model::Constraint::ArcLength { .. }) | Some(qymcad_core::model::Constraint::EdgeDistance { .. })) {
                                    self.drag = Dragging::Dim(ci);
                                    self.gsel.constraint = Some(ci); // grabbing a dimension selects it, even on a tiny drag
                                    self.annot.note = None;
                                }
                            } else if self.edit_si() == Some(si) {
                                if let Some((center, diam)) = self.passive_radius_label_at(rect, pp, si) {
                                    // on A CIRCLE or AN ARC with no dimension the label is automatic, so a
                                    // DRIVEN dimension is materialised (it changes no degrees of freedom) to let
                                    // the label be turned about the centre instead of dragging the whole
                                    // geometry - grabbing the radius used to fall through into moving the entire
                                    // arc. A circle gets a diameter, an arc a radius.
                                    use qymcad_core::model::Constraint;
                                    let r = self.center_radius(si, center).unwrap_or(0.0);
                                    let ang = self.sketch_pt(si, center).map(|cp| { let sc = self.to_screen(rect, cp); (pp.y - sc.y).atan2(pp.x - sc.x) as f64 }).unwrap_or(0.0);
                                    let d = if diam { 2.0 * r } else { r };
                                    self.project.sketches[si].constraints.push(Constraint::Diameter { c: center, d, off: ang, expr: String::new(), driven: true, diam });
                                    let ci = self.project.sketches[si].constraints.len() - 1;
                                    self.drag = Dragging::Dim(ci);
                                    self.gsel.constraint = Some(ci);
                                    self.annot.note = None;
                                }
                            }
                        }
                    }
                    // a tangent handle of a spline takes the highest priority - it is grabbed before a point
                    if !self.drag.active() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if self.edit_si() == Some(si) {
                                let mut best: Option<(f32, usize, usize)> = None;
                                for spi in 0..self.project.sketches[si].splines.len() {
                                    for (ki, (_knot, hend)) in self.project.spline_handles(si, spi).into_iter().enumerate() {
                                        let d = self.to_screen(rect, hend).distance(pp);
                                        if d <= self.grab(Grab::Point) && best.map_or(true, |(bd, _, _)| d < bd) {
                                            best = Some((d, spi, ki));
                                        }
                                    }
                                }
                                if let Some((_, spi, ki)) = best {
                                    self.drag = Dragging::Handle(si, spi, ki);
                                }
                            }
                        }
                    }
                    // a point of the selected typed sketch is dragged when the cursor is near it
                    if !self.drag.active() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if self.project.is_typed_sketch(si) {
                                // the points of arcs (the centre, the tangencies of fillets) are not dragged, or
                                // the fillet breaks
                                let mut arc_pts: std::collections::HashSet<Id> = self.project.sketches[si].entities.iter().flat_map(|e| match e.kind {
                                    qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                                    _ => vec![],
                                }).collect();
                                // reference points (the origin, the axes, materialised midpoints) are not dragged
                                arc_pts.extend(self.project.sketches[si].system_ids());
                                for c in &self.project.sketches[si].constraints {
                                    match c {
                                        qymcad_core::model::Constraint::Midpoint { p, .. } => {
                                            arc_pts.insert(*p);
                                        }
                                        // a pinned point is not dragged - it is fixed
                                        qymcad_core::model::Constraint::Fixed { p } => {
                                            arc_pts.insert(*p);
                                        }
                                        _ => {}
                                    }
                                }
                                // DRIVEN POINTS (projections of the geometry of a body) are not dragged: their
                                // position is set by the part, and a projection dragged by hand would snap back
                                // at the very first rebuild, silently undoing the work.
                                self.begin_point_drag(si, rect, pp, &arc_pts);
                            }
                        }
                    }
                    // moving the whole selected geometry: the cursor is on A SELECTED entity, not on a point
                    if !self.drag.active() && self.drag.mov().is_none() && !ctrl {
                        if let (Sel::Sketch(si), Some(pp)) = (self.sel, pp) {
                            if self.project.is_typed_sketch(si) && !self.sel_sk.items.is_empty() {
                                if let Some(h) = self.sketch_hit(rect, pp, si) {
                                    if self.sel_sk.items.contains(&h) {
                                        let ids = self.sketch_sel_points(si);
                                        if !ids.is_empty() {
                                            self.drag = Dragging::Move(si, ids);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // a selection box works with the primary button ONLY, and with no other drag mode active
                    // (not while a dimension, a note or a point is being dragged, and not while the middle
                    // button pans)
                    let no_other = !self.drag.active() && self.drag.mov().is_none() && self.drag.handle().is_none();
                    let primary = resp.drag_started_by(egui::PointerButton::Primary);
                    if no_other && primary {
                        let on_empty = matches!(self.sel, Sel::Sketch(si) if self.edit_si() == Some(si))
                            && pp.map_or(false, |p| if let Sel::Sketch(si) = self.sel { self.sketch_hit(rect, p, si).is_none() } else { false });
                        if ctrl || on_empty {
                            self.tree_sel.box_start = resp.interact_pointer_pos();
                        } else if let (Some(hw), Some(pp)) = (handle, resp.interact_pointer_pos()) {
                            if self.to_screen(rect, hw).distance(pp) <= self.grab(Grab::Point) {
                                self.body_giz.dragging = true;
                            }
                        }
                    }
                }
    }

    /// CONTINUING A DRAG IN A SKETCH: whatever was grabbed is carried until the button is released.
    ///
    /// The paired phase to `sketch_drag_start`: that one decides WHAT was grabbed, this one what to do with
    /// it every frame. While both lay in one piece there was no telling "picking an object" from "carrying
    /// it", and those are different things: the first fires once, the second on every frame.
    ///
    /// POWER TRIM: trimming BY DRAGGING - the cursor passes through several segments, and every one it
    /// crossed gets trimmed.
    ///
    /// Trimming by click existed and remains; dragging is for where a lot has to be cut in a row - taking
    /// apart a grid of construction lines one click at a time means dozens of careful hits.
    ///
    /// A TRAIL is followed rather than a single point: between frames the cursor jumps by tens of pixels,
    /// and checking only the final position would miss the segments the mouse flew across. Every segment is
    /// cut once per drag - otherwise the same piece would keep being trimmed until its neighbours
    /// disappeared.
    pub(super) fn power_trim_drag(&mut self, resp: &egui::Response, rect: Rect) {
        if self.tool.click_op != 1 {
            return;
        }
        if resp.drag_started() {
            self.trim.path.clear();
            self.trim.done.clear();
        }
        if !resp.dragged() {
            if resp.drag_stopped() {
                self.trim.path.clear();
                self.trim.done.clear();
            }
            return;
        }
        let Some(now) = resp.interact_pointer_pos() else { return };
        let prev = self.trim.path.last().copied().unwrap_or(now);
        self.trim.path.push(now);
        self.power_trim_sweep(rect, prev, now);
    }

    /// ONE pass of the trail from `prev` to `now`: it cuts everything it went through. Returns the number of
    /// spans trimmed.
    ///
    /// Split out of the drag handler so that a test can drive it: a real `egui::Response` cannot be built in
    /// a headless test, while cutting along a trail is exactly the logic the tool exists for.
    pub(super) fn power_trim_sweep(&mut self, rect: Rect, prev: Pos2, now: Pos2) -> usize {
        let Sel::Sketch(si) = self.sel else { return 0 };
        // SAMPLING ALONG THE TRAIL: between frames the cursor covers a noticeable distance, so the segment
        // from `prev` to `now` is walked in steps of a couple of pixels rather than checked only at its end.
        let steps = ((prev.distance(now) / 3.0).ceil() as usize).clamp(1, 64);
        let mut cut = 0;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let at = egui::pos2(prev.x + (now.x - prev.x) * t, prev.y + (now.y - prev.y) * t);
            let w = self.to_world(rect, at);
            let hit = self.nearest_line_eid(rect, at, si).map(|e| (e, true)).or_else(|| self.nearest_circle_entity(rect, at, si).map(|e| (e, false)));
            let Some((eid, is_line)) = hit else { continue };
            // ONE SPAN, ONE CUT PER DRAG: the key is the entity plus the span that was hit. Without it a
            // trembling cursor standing still would go on trimming the neighbouring pieces.
            let span = self.trim_span_key(si, eid, w.x, w.y);
            if !self.trim.done.insert((eid, span)) {
                continue;
            }
            let ok = if is_line { self.project.trim_line(si, eid, w.x, w.y) } else { self.project.trim_curve(si, eid, w.x, w.y) };
            if ok {
                cut += 1;
            }
        }
        if cut > 0 {
            self.sel_sk.clear();
            self.invalidate();
            self.status = crate::i18n::tr1("sk-trimmed-n", "n", &self.trim.done.len().to_string());
        }
        cut
    }

    /// A test facade: START a drag and walk the trail from a to b.
    #[cfg(test)]
    pub(crate) fn power_trim_path_test(&mut self, rect: Rect, a: Pos2, b: Pos2) -> usize {
        self.trim.path.clear();
        self.trim.done.clear();
        self.power_trim_sweep(rect, a, b)
    }

    /// A test facade: CONTINUE the same drag (the memory of the spans already trimmed is kept).
    #[cfg(test)]
    pub(crate) fn power_trim_path_test_continue(&mut self, rect: Rect, a: Pos2, b: Pos2) -> usize {
        self.power_trim_sweep(rect, a, b)
    }

    /// THE NUMBER OF THE SPAN of a curve a point fell into (a span is the stretch between two neighbouring
    /// intersections). It tells "the same segment we already cut" from "the next one" - without it, dragging
    /// along one line would trim only its first piece.
    fn trim_span_key(&self, si: usize, eid: Id, x: f64, y: f64) -> u32 {
        let inter = self.project.entity_intersections(si, eid);
        let Some(s) = self.project.sketches.get(si) else { return 0 };
        let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return 0 };
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        match kind {
            qymcad_core::model::EntityKind::Line { a, b } => {
                let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) else { return 0 };
                let (dx, dy) = (bx - ax, by - ay);
                let len2 = dx * dx + dy * dy;
                if len2 < 1e-12 {
                    return 0;
                }
                let param = |px: f64, py: f64| ((px - ax) * dx + (py - ay) * dy) / len2;
                let tc = param(x, y);
                inter.iter().filter(|(ix, iy)| param(*ix, *iy) < tc).count() as u32
            }
            qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => {
                let Some((cx, cy)) = pt(center) else { return 0 };
                let ang = |px: f64, py: f64| (py - cy).atan2(px - cx).rem_euclid(std::f64::consts::TAU);
                let ac = ang(x, y);
                inter.iter().filter(|(ix, iy)| ang(*ix, *iy) < ac).count() as u32
            }
            _ => 0,
        }
    }

    pub(super) fn sketch_drag_update(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
                if let Some(ti) = self.drag.text() {
                    // moving a text object (it shifts the parameters, the baked glyphs and the contours)
                    if resp.dragged() {
                        if let Sel::Sketch(si) = self.sel {
                            let d = resp.drag_delta();
                            let (wx, wy) = (d.x as f64 / self.view.scale as f64, -d.y as f64 / self.view.scale as f64);
                            self.project.move_sketch_text(si, ti, wx, wy);
                            self.invalidate();
                        }
                    }
                    if resp.drag_stopped() {
                        self.drag.clear();
                    }
                } else if let Some(ni) = self.drag.note() {
                    // moving a note
                    if resp.dragged() {
                        if let Sel::Sketch(si) = self.sel {
                            let d = resp.drag_delta();
                            let (wx, wy) = (d.x as f64 / self.view.scale as f64, -d.y as f64 / self.view.scale as f64);
                            if let Some(n) = self.project.sketches.get_mut(si).and_then(|s| s.notes.get_mut(ni)) {
                                n.x += wx;
                                n.y += wy;
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.drag.clear();
                    }
                } else if let Some(ci) = self.drag.dim() {
                    // offsetting a dimension line (editing `off`, with no recomputation of the geometry), with
                    // THE AXIS taken into account: a horizontal dimension offsets along screen Y, a vertical one
                    // along X, an aligned one or a point-to-line along the perpendicular.
                    if resp.dragged() {
                        if let Sel::Sketch(si) = self.sel {
                            use qymcad_core::model::Constraint;
                            // the diameter or radius label travels AROUND THE CIRCLE - `off` is the absolute
                            // angle of the leader (in radians) from the centre to the cursor. It is set directly
                            // rather than by a delta, so the label sticks to the pointer.
                            if let Some(Constraint::Diameter { c, .. }) = self.project.sketches[si].constraints.get(ci).cloned() {
                                if let (Some(cp), Some(pp)) = (self.sketch_pt(si, c), resp.interact_pointer_pos()) {
                                    let sc = self.to_screen(rect, cp);
                                    let ang = (pp.y - sc.y).atan2(pp.x - sc.x) as f64;
                                    if let Some(Constraint::Diameter { off, .. }) = self.project.sketches[si].constraints.get_mut(ci) {
                                        *off = ang;
                                    }
                                }
                                self.invalidate();
                            } else {
                            let dl = resp.drag_delta();
                            let dadd = match self.project.sketches[si].constraints.get(ci).cloned() {
                                Some(Constraint::Distance { a, b, axis, .. }) => match axis {
                                    1 => dl.y as f64,
                                    2 => dl.x as f64,
                                    _ => {
                                        if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                                            let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                                            let dir = (sb - sa).normalized();
                                            let perp = egui::vec2(-dir.y, dir.x);
                                            (dl.x * perp.x + dl.y * perp.y) as f64
                                        } else {
                                            0.0
                                        }
                                    }
                                },
                                Some(Constraint::DistancePL { a, b, .. }) => {
                                    // the dimension line slides ALONG the line (ab) rather than across it - that
                                    // is what lets it be raised or lowered over the geometry instead of merely
                                    // moved nearer or further.
                                    if let Some(ab) = self.line_screen_dir(si, a, b, rect) {
                                        (dl.x * ab.x + dl.y * ab.y) as f64
                                    } else {
                                        0.0
                                    }
                                }
                                // an arc length: `off` is the vertical screen shift of the caption (see the drawing)
                                Some(Constraint::ArcLength { .. }) => dl.y as f64,
                                // a tangent distance: `off` runs along the perpendicular to the line of centres c1-c2
                                Some(Constraint::EdgeDistance { c1, c2, .. }) => {
                                    if let (Some(p1), Some(p2)) = (self.sketch_pt(si, c1), self.sketch_pt(si, c2)) {
                                        let dir = (self.to_screen(rect, p2) - self.to_screen(rect, p1)).normalized();
                                        let perp = egui::vec2(-dir.y, dir.x);
                                        (dl.x * perp.x + dl.y * perp.y) as f64
                                    } else {
                                        0.0
                                    }
                                }
                                _ => 0.0,
                            };
                            let dadd = dadd / self.view.scale as f64; // a screen delta becomes WORLD units, so the offset scales with the zoom
                            if let Some(Constraint::Distance { off, .. }) | Some(Constraint::DistancePL { off, .. }) | Some(Constraint::ArcLength { off, .. }) | Some(Constraint::EdgeDistance { off, .. }) = self.project.sketches[si].constraints.get_mut(ci) {
                                *off += dadd;
                            }
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.drag.clear();
                    }
                } else if let Some((si, spi, ki)) = self.drag.handle() {
                    // dragging a tangent handle of a spline changes its shape (the tangent becomes explicit)
                    if resp.dragged() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            let w = self.to_world(rect, pp);
                            self.project.set_spline_handle(si, spi, ki, w.x, w.y);
                            self.invalidate();
                        }
                    }
                    if resp.drag_stopped() {
                        self.drag.clear();
                    }
                } else if let Some((si, pi)) = self.drag.pt() {
                    // A DRAG IS ONE OPERATION: it opens on the first frame of the drag and closes on the
                    // release. The intermediate frames do not enter the step - an undo returns the point to
                    // where it was BEFORE the drag rather than to the previous frame.
                    if resp.drag_started() {
                        self.begin_edit(&crate::i18n::tr("status-move-point"));
                    }
                    if resp.dragged() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            self.drag_point_to(si, pi, rect, pp);
                        }
                    }
                    if resp.drag_stopped() {
                        self.finish_point_drag();
                    }
                } else if let Some((si, ids)) = self.drag.mov() {
                    // moving the selected geometry as a whole: all of its points are shifted, then the
                    // constraints are solved
                    if resp.dragged() {
                        let d = resp.drag_delta();
                        let (dx, dy) = (d.x as f64 / self.view.scale as f64, -d.y as f64 / self.view.scale as f64);
                        if let Some(s) = self.project.sketches.get_mut(si) {
                            for p in s.points.iter_mut() {
                                if ids.contains(&p.id) {
                                    p.x += dx;
                                    p.y += dy;
                                }
                            }
                        }
                        self.project.solve_sketch_drag_fast(si, None); // a frame of the move takes the fast path
                        self.invalidate();
                    }
                    if resp.drag_stopped() {
                        self.drag.clear();
                        self.project.solve_sketch(si); // the final full solve, including evaluating the parameters
                        self.invalidate();
                    }
                } else if self.body_giz.dragging {
                    if resp.dragged() {
                        let d = resp.drag_delta();
                        self.translate_selected(d.x as f64 / self.view.scale as f64, -d.y as f64 / self.view.scale as f64);
                    }
                    if resp.drag_stopped() {
                        self.body_giz.dragging = false;
                    }
                } else if self.tree_sel.box_start.is_some() {
                    if resp.drag_stopped() {
                        if let (Some(a), Some(b)) = (self.tree_sel.box_start, resp.interact_pointer_pos()) {
                            // without Shift the box REPLACES the selection; with Shift it adds to it
                            if !ctx.input(|i| i.modifiers.shift) && matches!(self.sel, Sel::Sketch(_)) {
                                self.sel_sk.clear(); // the selection and whatever was waiting for it
                            }
                            self.box_select(rect, a, b);
                        }
                        self.tree_sel.box_start = None;
                    }
                } else if resp.dragged_by(egui::PointerButton::Primary) {
                    // the fallback pan is for the left button only - a middle drag is served by the EXPLICIT
                    // handler below (otherwise the middle button would pan twice, at double speed)
                    let d = resp.drag_delta();
                    self.view.center.x -= d.x / self.view.scale;
                    self.view.center.y += d.y / self.view.scale;
                }
    }

    /// A CLICK IN A SKETCH: what it is right now - a step of a drawing tool, the placing of a dimension, or
    /// a pick of geometry.
    ///
    /// What a click means depends on the active mode, and that is precisely why the modes must be mutually
    /// exclusive: while they were cleared by hand one field at a time, a click could mean two things at
    /// once. Here that reading is gathered in one place and visible whole.
    ///
    /// TAKE THE POINT UNDER THE CURSOR (the start of a drag). `skip` lists the points that are not dragged.
    ///
    /// Split out of the event handling so that a test can repeat a drag through THE SAME code. While this sat
    /// inside `if resp.drag_started()`, a test could not reach the logic without faking the event - that is,
    /// it would have been checking its own fake.
    pub(super) fn begin_point_drag(&mut self, si: usize, rect: Rect, pp: Pos2, skip: &std::collections::HashSet<Id>) -> bool {
        let driven = self.project.sketches[si].projected_points();
        let mut best: Option<(f32, usize)> = None;
        for pi in 0..self.project.sketches[si].points.len() {
            let p = self.project.sketches[si].points[pi];
            if skip.contains(&p.id) || driven.contains(&p.id) {
                continue;
            }
            let sp = self.to_screen(rect, Point2::new(p.x, p.y));
            let d = sp.distance(pp);
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, pi));
            }
        }
        match best {
            Some((d, pi)) if d <= self.grab(Grab::Curve) => {
                self.drag = Dragging::Point(si, pi);
                true
            }
            _ => false,
        }
    }

    /// DRAG THE TAKEN POINT to where the cursor is - one frame of a drag.
    pub(super) fn drag_point_to(&mut self, si: usize, pi: usize, rect: Rect, pp: Pos2) {
        let w = self.snap_world(rect, pp);
        // the point is pinned to the cursor: it follows the mouse while defined geometry resists (the solver
        // runs with a strong drag residual).
        let pid = self.project.sketches.get(si).and_then(|s| s.points.get(pi)).map(|p| p.id);
        if let Some(id) = pid {
            // a drag frame takes the fast path (no re-evaluation of the parameters, a reduced iteration
            // budget). The full solve happens on release.
            self.project.solve_sketch_drag_fast(si, Some((id, w.x, w.y)));
        }
        self.invalidate();
    }

    /// RELEASE: a full solve with no drag residual, and the undo step is closed.
    pub(super) fn finish_point_drag(&mut self) {
        self.drag.clear();
        if let Sel::Sketch(si) = self.sel {
            self.project.solve_sketch(si);
            self.invalidate();
        }
        self.commit_edit();
    }

    /// FINISH THE SPLINE - what a double click does.
    ///
    /// Split out of the event handling for the same reason as everything else: a test must finish the shape
    /// through THE SAME code rather than a copy of its own. Esc cancels a spline - that is a different
    /// action, and substituting it for finishing would mean checking the wrong thing.
    pub(super) fn finish_spline(&mut self) {
        if let Sel::Sketch(si) = self.sel {
            if self.tool.pts.len() >= 2 {
                let pts = std::mem::take(&mut self.tool.pts);
                self.project.add_spline(si, pts, qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::of(self.tool.construction));
                self.invalidate();
                self.view.initialized = false;
            }
        }
        self.tool.pts.clear();
    }

    /// AN EVENT BECOMES A POINT (the same split as for a click in 3D).
    pub(super) fn sketch_click(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
        if !resp.clicked() {
            return;
        }
        let Some(pos) = resp.interact_pointer_pos() else { return };
        self.sketch_click_at(ctx, pos, rect);
    }

    /// AN ACTION AT A POINT on the sketch canvas - the same thing the mouse does, but with no `egui` event.
    pub(super) fn sketch_click_at(&mut self, ctx: &egui::Context, pos: egui::Pos2, rect: Rect) {
        {
            {
                        if self.picking.fillet_all() {
                            // a click on a shape follows the connected chain and opens the radius popup
                            if let Sel::Sketch(si) = self.sel {
                                if let Some(eid) = self.entity_near(rect, pos, si) {
                                    let comp = self.project.connected_entities(si, eid);
                                    self.corner.at = Some((si, 0, false));
                                    self.corner.only = Some(comp);
                                    self.corner.pos = Some(pos);
                                    self.corner.buf = qymcad_core::expr::fmt_num(self.tool_prefs.fillet);
                                    self.corner.focus = true;
                                    self.picking.clear();
                                } else {
                                    self.status = crate::i18n::tr("sk-click-shape-line").into();
                                }
                            }
                        } else if self.tool.kind != 0 {
                            self.sketch_tool_click(rect, pos);
                        } else if self.dim.kind != 0 {
                            self.dim_click(rect, pos);
                        } else if self.measure.on {
                            let w = self.snap_world(rect, pos);
                            if self.measure.pts.len() >= 2 {
                                self.measure.pts.clear(); // a new measurement
                            }
                            self.measure.pts.push(w);
                            if self.measure.pts.len() == 2 {
                                let (a, b) = (self.measure.pts[0], self.measure.pts[1]);
                                let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                                self.status = crate::i18n::trn("sk-distance-dxdy", &[("d", &crate::i18n::num(d, 3)), ("dx", &crate::i18n::num(b.x - a.x, 3)), ("dy", &crate::i18n::num(b.y - a.y, 3))]);
                            }
                        } else if self.pending_import.draw_pts.is_some() {
                            let w = self.snap_world(rect, pos);
                            if let Some(pts) = self.pending_import.draw_pts.as_mut() {
                                pts.push(w);
                            }
                        } else if self.tool.click_op == 6 {
                            // PROJECT THE GEOMETRY OF A BODY: a click on an edge of the underlay takes it into
                            // the sketch as a driven entity. The underlay was drawn before as well - but only as
                            // a picture: it could be snapped to and not taken as geometry.
                            if let Sel::Sketch(si) = self.sel {
                                self.project_clicked_edge(si, rect, pos);
                            }
                        } else if self.tool.click_op == 4 || self.tool.click_op == 5 {
                            // a click on a corner opens the RADIUS or LEG popup, and it applies only on Enter or
                            // the tick (a default of 3 mm used to be applied silently, and on a small part that
                            // failed)
                            if let Sel::Sketch(si) = self.sel {
                                if let Some(pid) = self.nearest_vertex(rect, pos, si) {
                                    self.corner.at = Some((si, pid, self.tool.click_op == 5));
                                    self.corner.pos = Some(pos);
                                    self.corner.buf = qymcad_core::expr::fmt_num(self.tool_prefs.fillet);
                                    self.corner.focus = true;
                                } else {
                                    self.status = crate::i18n::tr("sk-click-corner").into();
                                }
                            }
                        } else if self.tool.click_op != 0 {
                            // trimming, extending or breaking by click: a line first, then a circle or an arc
                            if let Sel::Sketch(si) = self.sel {
                                let w = self.to_world(rect, pos);
                                let line_eid = self.nearest_line_eid(rect, pos, si);
                                let ok = if let Some(eid) = line_eid {
                                    match self.tool.click_op {
                                        1 => self.project.trim_line(si, eid, w.x, w.y),
                                        2 => self.project.extend_line(si, eid, w.x, w.y),
                                        3 => self.project.break_line(si, eid, w.x, w.y),
                                        _ => false,
                                    }
                                } else {
                                    false
                                };
                                // not a line (or the line did not work) - try a circle or an arc
                                let ok = ok
                                    || self.nearest_circle_entity(rect, pos, si).is_some_and(|eid| match self.tool.click_op {
                                        1 => self.project.trim_curve(si, eid, w.x, w.y),
                                        2 => self.project.extend_curve(si, eid, w.x, w.y),
                                        3 => self.project.break_curve(si, eid, w.x, w.y),
                                        _ => false,
                                    });
                                if ok {
                                    self.sel_sk.clear(); // the selection and whatever was waiting for it
                                    self.invalidate();
                                    self.status = crate::i18n::tr("sk-done").into();
                                } else if line_eid.is_none() && self.nearest_circle_entity(rect, pos, si).is_none() {
                                    self.status = crate::i18n::tr("sk-click-curve").into();
                                } else {
                                    self.status = crate::i18n::tr("sk-op-failed-no-intersection").into();
                                }
                            }
                        } else if self.clip.geom_pending.is_some() && matches!(self.sel, Sel::Sketch(_)) {
                            // a click on THE ANCHOR point takes the geometry into the buffer (on a cut, the
                            // source is removed)
                            let Sel::Sketch(si) = self.sel else { return }; // the selection may have changed between frames - do not crash
                            let w = self.snap_world(rect, pos);
                            // it should not be empty, but the program must not crash over that
                            let Some((eids, cut)) = self.clip.geom_pending.take() else { return };
                            let clip = self.project.copy_sketch_geometry(si, &eids, w.x, w.y);
                            if cut {
                                self.project.delete_entities(si, &eids);
                                self.project.solve_sketch(si);
                                self.invalidate();
                            }
                            // the anchor point has been clicked, so the selection is cleared - visually the copy
                            // is finished
                            self.sel_sk.clear(); // the selection and whatever was waiting for it
                            let n = clip.entities.len();
                            self.clip.geom = Some(clip);
                            self.status = crate::i18n::tr2("sk-clipboard", "what", &if cut { crate::i18n::tr("sk-cut-done") } else { crate::i18n::tr("sk-copied") }, "n", &n.to_string());
                        } else if self.clip.geom_place && matches!(self.sel, Sel::Sketch(_)) {
                            // a placement click pastes the buffer so that the anchor lands on the clicked point
                            let Sel::Sketch(si) = self.sel else { return }; // the selection may have changed between frames - do not crash
                            let w = self.snap_world(rect, pos);
                            if let Some(clip) = self.clip.geom.clone() {
                                let ids = self.project.paste_sketch_geometry(si, &clip, w.x, w.y);
                                self.project.solve_sketch(si);
                                self.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect();
                                self.invalidate();
                                self.status = crate::i18n::tr("sk-pasted").into();
                            }
                            self.clip.geom_place = false;
                        } else if self.pat.op != 0 && matches!(self.sel, Sel::Sketch(_)) {
                            // an array: pick the entities, then (for a circular one) click THE CENTRE of
                            // rotation, then Enter
                            let shift = ctx.input(|i| i.modifiers.shift);
                            let has_sel = self.sel_sk.items.iter().any(|(k, _)| *k == 1);
                            if !has_sel {
                                self.sketch_select_click(rect, pos, shift);
                                self.status = if self.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                                    if self.pat.op == 2 { crate::i18n::tr("sk-click-rot-centre").into() } else { crate::i18n::tr("sk-params-above").into() }
                                } else {
                                    crate::i18n::tr("sk-click-for-array").into()
                                };
                            } else if shift {
                                // Shift continues picking the source
                                self.sketch_select_click(rect, pos, true);
                            } else if self.pat.op == 2 {
                                // circular: a click sets or moves the centre, snapping to an intersection or a vertex
                                self.pat.center = Some(self.snap_world(rect, pos));
                                self.status = crate::i18n::tr("sk-centre-set-params").into();
                            }
                        } else if self.tool.move_op != 0 && matches!(self.sel, Sel::Sketch(_)) {
                            // an interactive move or copy: the selection, then the base point, then the target
                            let Sel::Sketch(si) = self.sel else { return }; // the selection may have changed between frames - do not crash
                            let w = self.snap_world(rect, pos);
                            let eids: Vec<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                            if eids.is_empty() {
                                let shift = ctx.input(|i| i.modifiers.shift);
                                self.sketch_select_click(rect, pos, shift);
                                if !self.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                                    self.status = crate::i18n::tr("sk-click-for-move").into();
                                } else {
                                    self.status = crate::i18n::tr("sk-click-base-point").into();
                                }
                            } else if self.tool.move_base.is_none() {
                                self.tool.move_base = Some(w);
                                if self.tool.move_op == 3 {
                                    // the centre is set, so the angle is typed in the popup - no target click is
                                    // awaited
                                    self.rot.angle = 0.0;
                                    self.rot.buf = "0".into();
                                    self.rot.focus = true;
                                    self.status = crate::i18n::tr("sk-centre-set-angle").into();
                                } else {
                                    self.status = crate::i18n::tr("sk-click-target-point").into();
                                }
                            } else if self.tool.move_op != 3 {
                                let Some(base) = self.tool.move_base.take() else { return };
                                let (dx, dy) = (w.x - base.x, w.y - base.y);
                                if self.tool.move_op == 2 {
                                    let ids = self.project.copy_entities(si, &eids, dx, dy);
                                    self.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect(); // select the copies
                                    self.status = crate::i18n::tr("sk-copied").into();
                                } else {
                                    self.project.move_entities(si, &eids, dx, dy);
                                    self.status = crate::i18n::tr("sk-moved").into();
                                }
                                self.tool.move_op = 0;
                                self.invalidate();
                            }
                        } else if self.cmd.kind == 3 && self.rev.pick_line {
                            // the axis of revolution is picked BY CLICKING a line of the sketch. It used to be
                            // chosen from a list of "Line 1 / Line 2 / Line 3", where a number tells nothing
                            // about which one is wanted.
                            let si = self.cmd.sketch.unwrap_or(0);
                            let cands = self.profile_axis_lines(si);
                            match self.nearest_line_id(rect, pos, si, &cands) {
                                Some(eid) => {
                                    self.rev.axis_line = eid;
                                    self.rev.axis_datum = 0;
                                    self.rev.pick_line = false;
                                    self.mode_3d = true;
                                    self.view.initialized = false;
                                    let n = cands.iter().position(|l| *l == eid).map(|i| i + 1).unwrap_or(1);
                                    self.status = format!("{} {}", ph::CHECK, crate::i18n::tr1("g-rev-axis", "what", &self.axis_line_label(si, eid, n)));
                                }
                                None => self.status = crate::i18n::tr("sk-miss-line").into(),
                            }
                        } else if let Some(slot) = self.picking.contour() {
                            // picking the contour for a sweep or loft slot through the half-sketcher: a click on
                            // a contour fills the slot and returns to 3D (as in Extrude, but a single pick into a
                            // particular slot).
                            let cands = self.slot_candidates(slot);
                            if let Some(cid) = self.slot_contour_under_2d(rect, pos, &cands) {
                                self.set_contour_slot(slot, cid);
                                self.picking.clear();
                                self.mode_3d = true;
                                self.view.initialized = false;
                                self.status = crate::i18n::tr("sk-contour-picked").into();
                            } else {
                                self.status = crate::i18n::tr("sk-miss-contour").into();
                            }
                        } else if self.cmd.active() {
                            // a click on a contour ALWAYS adds to the selection (as Ctrl used to): a single click
                            // does NOT leave the profile-picking mode, and only Enter moves on to the dimension.
                            // A miss (a click past every contour) does NOT clear the set - otherwise an accidental
                            // near-miss wiped every profile gathered so far; clearing happens only on Esc or on a
                            // repeated click.
                            if let Some(si) = self.cmd.sketch {
                                if let Some(cid) = self.contour_under_2d(rect, pos, si) {
                                    if !self.gsel.profiles.remove(&cid) {
                                        self.gsel.profiles.insert(cid); // a repeated click on a contour deselects it
                                    }
                                    self.status = crate::i18n::tr1("sk-profiles-n", "n", &self.gsel.profiles.len().to_string());
                                }
                            }
                        } else if self.workbench == Workbench::Sketch && matches!(self.sel, Sel::Sketch(_)) {
                            // the sketch workbench: a click picks a point or an entity (Shift adds to the selection)
                            let shift = ctx.input(|i| i.modifiers.shift);
                            self.sketch_select_click(rect, pos, shift);
                        } else {
                            // in 2D a click on a contour already adds it while an operation is active
                            self.pick_contour(rect, pos);
                            self.op_pick = None;
                        }
                    }
                }
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
                self.draw_table(&painter, rect);
                self.draw_axes(&painter, rect);
                self.draw_mesh(&painter, rect);
                self.draw_sketch_face_edges(&painter, rect); // the edges of the host face (an outside one too) as a reference
                self.draw_contours(&painter, rect);
                // the points of the picked sketch plus its dimensions and constraints (visible, associative)
                if let Sel::Sketch(si) = self.sel {
                    if self.project.sketches.get(si).is_some() {
                        // in the half-sketcher of a part command (extrude, cut, revolve) the dimensions, the
                        // constraint glyphs, the construction geometry and the point numbers are hidden - a clean
                        // profile, like a drawing.
                        let profile_pick = self.cmd.active() && self.sketch_ses.editing.is_none();
                      if !profile_pick {
                        // the colour of the points follows how defined they are, as in any CAD: green means
                        // defined, YELLOW still free (showing what is under-defined), red means trouble.
                        // Points go RED only on a CONFLICT of dimensions (inconsistent values, a real error).
                        // Harmless redundancy (consistent reference dimensions) does NOT redden them - that
                        // would be a false alarm of "a heap of errors" on a perfectly good sketch.
                        let has_conflict = !self.sketch_diag(si).conflicts.is_empty();
                        let (_, free) = self.sketch_status(si); // cached: the Jacobian is not computed every frame
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
                            let sp = self.to_screen(rect, Point2::new(p.x, p.y));
                            let picked = self.dim.pick.contains(&p.id);
                            let selected = self.sel_sk.items.contains(&(0, p.id));
                            let hovered = self.hover.sketch == Some((0, p.id));
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
                        self.draw_sketch_dims(&painter, rect, si);
                        self.draw_sketch_constraints(&painter, rect, si);
                      }
                        // text AS GEOMETRY (parametric captions): the glyphs go over the contours, and the
                        // selected one is orange with a bounding box. It is drawn explicitly, so it shows
                        // whether or not the contours are displayed.
                        for (ti, t) in self.project.sketches[si].texts.iter().enumerate() {
                            let sel = self.annot.text == Some(ti);
                            let col = if sel { self.scheme.pal.selected() } else if t.construction { self.scheme.pal.sketch_construction() } else { self.scheme.pal.annotation() };
                            for loop_ in &t.glyphs {
                                if loop_.len() >= 2 {
                                    let mut pts: Vec<Pos2> = loop_.iter().map(|p| self.to_screen(rect, *p)).collect();
                                    pts.push(pts[0]);
                                    painter.add(egui::Shape::line(pts, Stroke::new(if sel { 2.0 } else { 1.7 }, col)));
                                }
                            }
                            if sel {
                                if let Some((minx, miny, maxx, maxy)) = self.project.sketch_text_bbox(si, ti) {
                                    let bb = Rect::from_two_pos(self.to_screen(rect, Point2::new(minx, miny)), self.to_screen(rect, Point2::new(maxx, maxy))).expand(3.0);
                                    painter.rect_stroke(bb, 0.0, Stroke::new(1.0, self.scheme.pal.selected()));
                                }
                            }
                        }
                        // text notes (remarks and to-dos); the selected one is orange
                        for (ni, note) in self.project.sketches[si].notes.iter().enumerate() {
                            let sp = self.to_screen(rect, Point2::new(note.x, note.y));
                            let nc = if self.annot.note == Some(ni) { self.scheme.pal.selected() } else { self.scheme.pal.note() };
                            painter.text(sp, egui::Align2::LEFT_BOTTOM, &note.text, egui::FontId::proportional(14.0), nc);
                        }
                    }
                }
                self.draw_projection_overlay(&painter, rect); // driven projected geometry, in its own colour
                self.draw_sketch_preview(&painter, rect);
                self.draw_trim_preview(&painter, rect); // the hover preview of a trim, extend or break
                self.draw_move_preview(&painter, rect); // the ghost of a move or a copy
                self.draw_pattern_preview(&painter, rect); // the ghost of an array
                self.draw_clip_pending(&painter, rect); // the highlight of the selection plus the crosshair awaiting the anchor point
                self.draw_clip_ghost(&painter, rect); // the ghost of geometry being pasted from the buffer
                // the measurement: the points, the line and the distance caption
                if self.measure.on && !self.measure.pts.is_empty() {
                    let col = self.scheme.pal.measure();
                    for p in &self.measure.pts {
                        painter.circle_filled(self.to_screen(rect, *p), 3.5, col);
                    }
                    if self.measure.pts.len() == 2 {
                        let (a, b) = (self.measure.pts[0], self.measure.pts[1]);
                        let (sa, sb) = (self.to_screen(rect, a), self.to_screen(rect, b));
                        painter.line_segment([sa, sb], Stroke::new(1.5, col));
                        let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                        painter.text(((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2(), egui::Align2::CENTER_BOTTOM, format!("{d:.2}"), egui::FontId::proportional(13.0), col);
                    }
                }
                // the snap marker of the cursor
                if let Some((p, kind)) = self.snap_hint {
                    let sp = self.to_screen(rect, p);
                    let yellow = self.scheme.pal.snap_marker();
                    match kind {
                        0 => {
                            // a vertex is a yellow square
                            let r = 5.0;
                            painter.rect_stroke(Rect::from_center_size(sp, egui::vec2(r * 2.0, r * 2.0)), 0.0, Stroke::new(1.5, yellow));
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
                self.draw_toolpath(&painter, rect);
                // the sketch while it is being drawn
                if let Some(pts) = &self.pending_import.draw_pts {
                    let st = Stroke::new(2.0, self.scheme.pal.sketch_line());
                    let screen: Vec<Pos2> = pts.iter().map(|p| self.to_screen(rect, *p)).collect();
                    if screen.len() >= 2 {
                        painter.add(egui::Shape::line(screen.clone(), st));
                    }
                    for s in &screen {
                        painter.circle_filled(*s, 3.0, self.scheme.pal.sketch_line());
                    }
                    if let (Some(last), Some(cur)) = (screen.last(), self.cursor) {
                        painter.line_segment([*last, self.to_screen(rect, cur)], Stroke::new(1.0, self.scheme.pal.rubber_band()));
                    }
                }
                // the move gizmo of the selected object
                if let Some(hw) = handle {
                    let hs = self.to_screen(rect, hw);
                    let col = if self.body_giz.dragging { self.scheme.pal.active() } else { self.scheme.pal.ok() };
                    painter.rect_filled(Rect::from_center_size(hs, egui::vec2(10.0, 10.0)), 2.0, col);
                    painter.circle_stroke(hs, 14.0, Stroke::new(1.0, col));
                }
                // the selection box on top: left to right is A WINDOW (solid blue, only what is wholly
                // inside), right to left is A CROSSING (dashed green, anything touched)
                if let (Some(a), Some(b)) = (self.tree_sel.box_start, resp.interact_pointer_pos()) {
                    let r = Rect::from_two_pos(a, b);
                    if b.x < a.x {
                        let st = Stroke::new(1.2, self.scheme.pal.select_cross());
                        for seg in [[r.left_top(), r.right_top()], [r.right_top(), r.right_bottom()], [r.right_bottom(), r.left_bottom()], [r.left_bottom(), r.left_top()]] {
                            painter.add(egui::Shape::dashed_line(&seg, st, 5.0, 4.0));
                        }
                        painter.rect_filled(r, 0.0, crate::palette::a(self.scheme.pal.select_cross(), 20));
                    } else {
                        painter.rect_stroke(r, 0.0, Stroke::new(1.2, self.scheme.pal.select_window()));
                        painter.rect_filled(r, 0.0, crate::palette::a(self.scheme.pal.select_window(), 20));
                    }
                }
                // the in-place dimension editor (a double click on a dimension)
                self.dim_editor(ctx, rect);
                self.note_editor(ctx, rect); // editing the text of a note
                self.text_obj_editor(ctx, rect); // editing a text object (the string and the height)
                self.place_input_popup(ctx, rect); // typing the sizes right after a shape is built
                self.sketch_rotate_popup(ctx, rect); // the rotation angle at the centre
    }
}

// SKETCH INPUT: the size popup that follows building a shape, snapping the cursor to the world, and a
// dimension between two objects. This is work on the sketch, not the frame of the frame.
impl App {
    /// The pop-up entry of sizes right after a rectangle or a polygon is built.
    pub(super) fn place_input_popup(&mut self, ctx: &egui::Context, rect: Rect) {
        // the popup for a corner fillet radius or a chamfer leg (Enter applies, Esc cancels)
        if let Some((si, pid, chamfer)) = self.corner.at {
            let at = self.corner.pos.unwrap_or_else(|| rect.center());
            let want_focus = std::mem::take(&mut self.corner.focus);
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut apply, mut cancel) = (false, false);
            let mut buf = std::mem::take(&mut self.corner.buf);
            egui::Area::new(egui::Id::new(("cornerinput", si, pid))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if chamfer { crate::i18n::tr("cmd-leg") } else if pid == 0 { crate::i18n::tr("sk-r-all-corners") } else { crate::i18n::tr("sk-radius") });
                        let r0 = Self::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                        if r0.lost_focus() && enter {
                            apply = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            apply = true;
                        }
                        if ui.button(ph::X).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
            self.corner.buf = buf;
            if apply {
                let r = self.parse_num(&self.corner.buf.clone()).unwrap_or(0.0);
                if r > 1e-6 {
                    self.tool_prefs.fillet = r; // sticky: the next corner offers the same value
                    let ok_n = if pid == 0 {
                        // the set from clicking a shape; failing that the selection; failing that the whole sketch
                        let only = self.corner.only.take().or_else(|| {
                            let sel: std::collections::HashSet<Id> = self.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                            (!sel.is_empty()).then_some(sel)
                        });
                        self.project.fillet_all_corners_of(si, r, only.as_ref())
                    } else if chamfer {
                        self.project.chamfer_at_vertex(si, pid, r) as usize
                    } else {
                        self.project.fillet_at_vertex(si, pid, r) as usize
                    };
                    if ok_n > 0 {
                        self.sel_sk.clear(); // the selection and whatever was waiting for it
                        self.invalidate();
                        self.status = if pid == 0 { crate::i18n::tr1("sk-filleted-n", "n", &ok_n.to_string()) } else { crate::i18n::tr("sk-done").into() };
                    } else {
                        self.status = format!("{} {}", ph::WARNING, crate::i18n::tr("sk-fillet-too-big"));
                    }
                }
                self.corner.clear(); // ALL of the input: `only` (a restricted set of corners) otherwise travelled
                // into the next call, and "round every corner" silently worked on the old set
            }
            if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.corner.clear();
            }
        }

        let Sel::Sketch(si) = self.sel else {
            self.place.clear(); // everything unfinished in the drawing at once (otherwise one of the three is forgotten)
            return;
        };
        // an ellipse: width by height (the full axes are twice the major and twice the minor semi-axis) - the
        // entity is found through its centre handle
        if let Some((handle, click)) = self.place.ellipse() {
            let cur = self.project.ellipse_axes(si, handle);
            if let Some((rx, ry)) = cur {
                let (w0, h0) = (2.0 * rx, 2.0 * ry);
                let at = self.to_screen(rect, Point2::new(click.x + rx, click.y + ry));
                let want_focus = std::mem::take(&mut self.place.focus);
                if want_focus {
                    self.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
                    self.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
                }
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                let (mut chg, mut close, mut got_focus) = (false, false, false);
                let mut buf = [std::mem::take(&mut self.place.buf[0]), std::mem::take(&mut self.place.buf[1])];
                egui::Area::new(egui::Id::new(("ellinput", si))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&crate::i18n::tr("sk-width-short"));
                            let r0 = Self::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                            chg |= r0.changed();
                            got_focus |= r0.has_focus();
                            ui.label(&crate::i18n::tr("sk-height-short"));
                            let r1 = Self::focus_edit(ui, &mut buf[1], 60.0, "", false);
                            chg |= r1.changed();
                            got_focus |= r1.has_focus();
                            if (r0.lost_focus() || r1.lost_focus()) && enter {
                                close = true;
                            }
                            if ui.button(ph::CHECK).clicked() {
                                close = true;
                            }
                        });
                    });
                });
                self.place.buf = buf.clone();
                if got_focus {
                    self.place.focus = false;
                }
                if chg {
                    let nw = self.parse_num(&buf[0]).unwrap_or(w0).max(0.02);
                    let nh = self.parse_num(&buf[1]).unwrap_or(h0).max(0.02);
                    self.project.set_ellipse_axes(si, handle, nw / 2.0, nh / 2.0);
                    self.invalidate();
                }
                if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.place.clear();
                    self.place.focus = false;
                }
            } else {
                self.place.clear();
            }
        }
        // a rectangle: width by height (text fields, with auto-focus, Tab and Enter)
        if let Some((a, b, ids)) = self.place.rect() {
            let (w0, h0) = ((b.x - a.x).abs(), (b.y - a.y).abs());
            let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
            let at = self.to_screen(rect, Point2::new(a.x.max(b.x), a.y.max(b.y)));
            let want_focus = std::mem::take(&mut self.place.focus);
            if want_focus {
                self.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
                self.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
            }
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut chg, mut close, mut got_focus) = (false, false, false);
            let mut buf = [std::mem::take(&mut self.place.buf[0]), std::mem::take(&mut self.place.buf[1])];
            egui::Area::new(egui::Id::new(("rectinput", si))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("sk-width-short"));
                        let r0 = Self::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(&crate::i18n::tr("sk-height-short"));
                        let r1 = Self::focus_edit(ui, &mut buf[1], 60.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            self.place.buf = buf.clone();
            if got_focus {
                self.place.focus = false;
            }
            if chg {
                let nw = self.parse_num(&buf[0]).unwrap_or(w0).max(0.01);
                let nh = self.parse_num(&buf[1]).unwrap_or(h0).max(0.01);
                let nb = Point2::new(a.x + if sx < 0.0 { -nw } else { nw }, a.y + if sy < 0.0 { -nh } else { nh });
                self.project.delete_entities(si, &ids);
                let nids = self.project.add_rect_entity(si, a.x, a.y, nb.x, nb.y, qymcad_core::feature::Purpose::Real);
                self.place.set(PlacingShape::Rect(a, nb, nids));
                self.invalidate();
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.place.clear();
                self.place.focus = false;
            }
        }
        // a polygon: the radius of the construction circle plus the rotation angle (it is rebuilt)
        if let Some(cid) = self.place.poly() {
            if let (Some((cx, cy, r)), Some(ang)) = (self.project.polygon_circle(si, cid), self.project.polygon_angle(si, cid)) {
                let at = self.to_screen(rect, Point2::new(cx + r, cy));
                let want_focus = std::mem::take(&mut self.place.focus);
                if want_focus {
                    self.place.buf[0] = format!("{}", (r * 1000.0).round() / 1000.0);
                    self.place.buf[1] = format!("{}", (ang.to_degrees() * 100.0).round() / 100.0);
                }
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                let (mut chg, mut close, mut got_focus) = (false, false, false);
                let mut buf = [std::mem::take(&mut self.place.buf[0]), std::mem::take(&mut self.place.buf[1])];
                egui::Area::new(egui::Id::new(("polyinput", si))).fixed_pos(self.clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&crate::i18n::tr("sk-radius"));
                            let r0 = Self::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                            chg |= r0.changed();
                            got_focus |= r0.has_focus();
                            ui.label(&crate::i18n::tr("sk-angle-deg"));
                            let r1 = Self::focus_edit(ui, &mut buf[1], 50.0, "", false);
                            chg |= r1.changed();
                            got_focus |= r1.has_focus();
                            if (r0.lost_focus() || r1.lost_focus()) && enter {
                                close = true;
                            }
                            if ui.button(ph::CHECK).clicked() {
                                close = true;
                            }
                        });
                    });
                });
                self.place.buf = buf.clone();
                if got_focus {
                    self.place.focus = false;
                }
                if chg {
                    let nr = self.parse_num(&buf[0]).unwrap_or(r).max(0.01);
                    let na = self.parse_num(&buf[1]).map(|d| d.to_radians()).unwrap_or(ang);
                    self.project.set_polygon_radius(si, cid, nr);
                    self.project.set_polygon_angle(si, cid, na);
                    self.invalidate();
                }
                if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.place.clear();
                    self.place.focus = false;
                }
            } else {
                self.place.clear();
            }
        }
    }

    /// Snapping the cursor: a vertex or a centre (always), then a midpoint, an intersection, a point on an
    /// edge, an axis, a grid node. It sets `snap_hint` (the type for the glyph) and returns a world point.
    /// The type codes: 0 vertex, 1 grid, 2 axis, 3 midpoint, 4 centre, 5 intersection, 6 on an edge.
    pub(super) fn snap_world(&mut self, rect: Rect, screen: Pos2) -> Point2 {
        let w = self.to_world(rect, screen);
        let sd = |p: Point2, this: &Self| this.to_screen(rect, p).distance(screen);
        // the centres of the circles and arcs of the active sketch, so a centre can be told from a plain vertex
        let centers: std::collections::HashSet<u64> = self
            .edit_si()
            .and_then(|si| self.project.sketches.get(si))
            .map(|s| {
                s.entities
                    .iter()
                    .filter_map(|e| match e.kind {
                        qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some(center),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // THE VERTICES of existing geometry snap ALWAYS - that is topology. Type 4 marks the centres.
        // THE ACTIVE SKETCH ONLY: every sketch and every contour of the project used to be walked, so the
        // cursor stuck to the vertices of other people's sketches (on different planes the 2D coordinates
        // coincide) and to the tessellation of every circle in the project - random snaps to the wrong place
        // plus a cost of O(the whole model) per frame. The reference geometry of neighbours and of the host
        // face arrives separately, below, through `sketch_ref_edges_2d`, already projected into the sketch.
        let mut best: Option<(f32, Point2, u8)> = None;
        if let Some(asi) = self.edit_si() {
            if let Some(s) = self.project.sketches.get(asi) {
                for sp in &s.points {
                    let p = Point2::new(sp.x, sp.y);
                    let d = sd(p, self);
                    let ty = if centers.contains(&sp.id) { 4 } else { 0 };
                    if best.map_or(true, |(bd, _, _)| d < bd) {
                        best = Some((d, p, ty));
                    }
                }
                for cid in &s.contour_ids {
                    let Some(ci) = self.project.contour_index(*cid) else { continue };
                    for p in &self.project.contours[ci].points {
                        let d = sd(*p, self);
                        if best.map_or(true, |(bd, _, _)| d < bd) {
                            best = Some((d, *p, 0));
                        }
                    }
                }
            }
        }
        // the edges of the REFERENCE body (a face of its own part, or a neighbour during the creation
        // session), projected into the sketch. THE ENDS of those edges (the corners of the outline) are
        // VERTICES (type 0, taking priority). The nearest point ON an edge and the INTERSECTIONS with it come
        // below, in the inference: a point on an edge used to stand as a vertex and SHORT-CIRCUITED the rest,
        // making it impossible to snap to the intersection of a construction line with the outline of a part.
        let ref_edges = self.edit_si().map(|si| self.sketch_ref_edges_2d(si)).unwrap_or_default();
        for poly in &ref_edges {
            for end in [poly.first(), poly.last()].into_iter().flatten() {
                let d = sd(*end, self);
                if best.map_or(true, |(bd, _, _)| d < bd) {
                    best = Some((d, *end, 0));
                }
            }
        }
        if let Some((d, p, ty)) = best {
            if d <= self.grab(Grab::Snap) {
                self.snap_hint = Some((p, ty));
                return p;
            }
        }
        if !self.set.snap.on {
            self.snap_hint = None;
            return w;
        }
        // the inference snaps of the active sketch: a midpoint, then an intersection, then a point on an edge.
        // `cand` holds (the screen distance, the point, the type); a smaller (type, distance) wins.
        let mut cand: Option<(f32, Point2, u8)> = None;
        if let Some(si) = self.edit_si() {
            let (lines, circs) = self.active_edges(si);
            // the segments of the projected outlines of the reference body, used for INTERSECTIONS with the
            // sketch lines and for points on an edge. That is how the intersection of a construction line with
            // a face or the outline of a part becomes snappable.
            let ref_segs: Vec<(Point2, Point2)> = ref_edges.iter().flat_map(|poly| poly.windows(2).map(|s| (s[0], s[1]))).collect();
            // the priority: a midpoint (3) over an intersection (5) over a point on an edge (6)
            // 1) the midpoints of segments (SKETCH lines only - the midpoints of a tessellated outline are noise)
            for (a, b) in &lines {
                let m = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                let d = sd(m, self);
                if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (3u8, d) < (bty, bd)) {
                    cand = Some((d, m, 3));
                }
            }
            // 2) intersections: sketch with sketch, sketch with a circle, AND sketch with THE OUTLINE OF A PART
            // (a construction line against a face)
            for i in 0..lines.len() {
                for j in (i + 1)..lines.len() {
                    if let Some(p) = seg_seg_intersect(lines[i].0, lines[i].1, lines[j].0, lines[j].1) {
                        let d = sd(p, self);
                        if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (5u8, d) < (bty, bd)) {
                            cand = Some((d, p, 5));
                        }
                    }
                }
                for (c, r) in &circs {
                    for p in seg_circle_intersect(lines[i].0, lines[i].1, *c, *r) {
                        let d = sd(p, self);
                        if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (5u8, d) < (bty, bd)) {
                            cand = Some((d, p, 5));
                        }
                    }
                }
                for (ra, rb) in &ref_segs {
                    if let Some(p) = seg_seg_intersect(lines[i].0, lines[i].1, *ra, *rb) {
                        let d = sd(p, self);
                        if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (5u8, d) < (bty, bd)) {
                            cand = Some((d, p, 5));
                        }
                    }
                }
            }
            // 3) a point on an edge (the cursor projected onto a sketch segment, THE OUTLINE OF A PART or a
            // circle) plus A TIE TO THE GRID ALONG the face (face against grid, type 5, which outranks a plain
            // point on an edge): across the face it sticks to the face, along it to the nearest grid line, so
            // the intersection of a face and the grid can be hit exactly.
            let gsz = self.set.snap.grid.max(0.1);
            for (a, b) in lines.iter().chain(ref_segs.iter()) {
                if let Some(p) = project_on_seg(w, *a, *b) {
                    if let Some(gpt) = grid_cross_on_seg(p, *a, *b, gsz) {
                        let d = sd(gpt, self);
                        if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (5u8, d) < (bty, bd)) {
                            cand = Some((d, gpt, 5));
                        }
                    }
                    let d = sd(p, self);
                    if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (6u8, d) < (bty, bd)) {
                        cand = Some((d, p, 6));
                    }
                }
            }
            for (c, r) in &circs {
                let dir = Point2::new(w.x - c.x, w.y - c.y);
                let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                if len > 1e-9 {
                    let p = Point2::new(c.x + dir.x / len * r, c.y + dir.y / len * r);
                    let d = sd(p, self);
                    if d <= self.grab(Grab::Snap) && cand.map_or(true, |(bd, _, bty)| (6u8, d) < (bty, bd)) {
                        cand = Some((d, p, 6));
                    }
                }
            }
        }
        if let Some((_, p, ty)) = cand {
            self.snap_hint = Some((p, ty));
            return p;
        }
        // snapping to the X = 0 and Y = 0 axes and to the origin. The free coordinate ALONG the axis is still
        // pulled towards the grid nodes (otherwise snapping by cells disappeared along an axis).
        let g = self.set.snap.grid.max(0.1);
        let snap_to_grid = |v: f64| (v / g).round() * g;
        let near_x0 = (self.to_screen(rect, Point2::new(0.0, w.y)).x - screen.x).abs() <= self.grab(Grab::Snap);
        let near_y0 = (self.to_screen(rect, Point2::new(w.x, 0.0)).y - screen.y).abs() <= self.grab(Grab::Snap);
        if near_x0 || near_y0 {
            // the coordinate ACROSS the axis becomes 0; along the axis it goes to the grid, if a node is near
            // on screen
            let gx = snap_to_grid(w.x);
            let gy = snap_to_grid(w.y);
            let ax = if near_x0 { 0.0 } else if (self.to_screen(rect, Point2::new(gx, w.y)).x - screen.x).abs() <= self.grab(Grab::Snap) { gx } else { w.x };
            let ay = if near_y0 { 0.0 } else if (self.to_screen(rect, Point2::new(w.x, gy)).y - screen.y).abs() <= self.grab(Grab::Snap) { gy } else { w.y };
            let ap = Point2::new(ax, ay);
            self.snap_hint = Some((ap, 2)); // an axis
            return ap;
        }
        // a grid node
        let gp = Point2::new(snap_to_grid(w.x), snap_to_grid(w.y));
        if self.to_screen(rect, gp).distance(screen) <= self.grab(Grab::Snap) {
            self.snap_hint = Some((gp, 1)); // the grid
            return gp;
        }
        self.snap_hint = None;
        w
    }

    /// Create a dimension BETWEEN two references (a point or a line) and let it follow the cursor until it is
    /// placed.
    pub(super) fn make_between_dim(&mut self, si: usize, r1: DimRef, r2: DimRef) {
        use qymcad_core::model::Constraint;
        let pt = |this: &Self, id: Id| this.sketch_pt(si, id);
        // the perpendicular distance from point p to the line a-b
        // a SIGNED perpendicular distance (the side matters: the point must not mirror to the other side
        // during the solve)
        let perp = |this: &Self, p: Id, a: Id, b: Id| -> f64 {
            match (pt(this, p), pt(this, a), pt(this, b)) {
                (Some(pp), Some(pa), Some(pb)) => {
                    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                    (dx * (pp.y - pa.y) - dy * (pp.x - pa.x)) / len
                }
                _ => 0.0,
            }
        };
        let c: Option<Constraint> = match (r1, r2) {
            (DimRef::Point(p1), DimRef::Point(p2)) => {
                let d = match (pt(self, p1), pt(self, p2)) {
                    (Some(a), Some(b)) => ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt(),
                    _ => 0.0,
                };
                Some(Constraint::Distance { a: p1, b: p2, d, off: 0.0, expr: String::new(), driven: false, axis: 0 })
            }
            (DimRef::Point(p), DimRef::Line(a, b)) | (DimRef::Line(a, b), DimRef::Point(p)) => {
                let d = perp(self, p, a, b);
                Some(Constraint::DistancePL { p, a, b, d, off: 0.0, expr: String::new(), driven: false })
            }
            (DimRef::Line(a1, b1), DimRef::Line(a2, b2)) => {
                let (ax1, ax2) = (self.is_axis_line(si, a1, b1), self.is_axis_line(si, a2, b2));
                if ax1 ^ ax2 {
                    // when ONE of the lines is a coordinate axis, the result is ALWAYS a distance (the position
                    // of the line or its end relative to the axis) and NOT an angle. A line perpendicular to an
                    // axis used to give 90 degrees or a driven 0.0. The end of the geometry FURTHER from the
                    // axis is taken, so the value is not a degenerate zero.
                    let (ga, gb, axa, axb) = if ax2 { (a1, b1, a2, b2) } else { (a2, b2, a1, b1) };
                    let (da, db) = (perp(self, ga, axa, axb), perp(self, gb, axa, axb));
                    let (p, d) = if db.abs() > da.abs() { (gb, db) } else { (ga, da) };
                    Some(Constraint::DistancePL { p, a: axa, b: axb, d, off: 0.0, expr: String::new(), driven: false })
                } else {
                    // parallel? then it is the distance between the lines (a perpendicular from the end of one
                    // to the other)
                    let (d1, d2) = match (pt(self, a1), pt(self, b1), pt(self, a2), pt(self, b2)) {
                        (Some(pa1), Some(pb1), Some(pa2), Some(pb2)) => ((pb1.x - pa1.x, pb1.y - pa1.y), (pb2.x - pa2.x, pb2.y - pa2.y)),
                        _ => ((1.0, 0.0), (0.0, 1.0)),
                    };
                    let cross = d1.0 * d2.1 - d1.1 * d2.0;
                    let (l1, l2) = ((d1.0 * d1.0 + d1.1 * d1.1).sqrt(), (d2.0 * d2.0 + d2.1 * d2.1).sqrt());
                    if cross.abs() / (l1 * l2).max(1e-9) < 0.05 {
                        // parallel: the distance from the end a1 to the line (a2, b2)
                        let d = perp(self, a1, a2, b2);
                        Some(Constraint::DistancePL { p: a1, a: a2, b: b2, d, off: 0.0, expr: String::new(), driven: false })
                    } else if let (Some(pa1), Some(pb1), Some(pa2), Some(pb2)) = (pt(self, a1), pt(self, b1), pt(self, a2), pt(self, b2)) {
                        // THE ANGLE between the lines. The directions of both lines are ORIENTED OUTWARDS FROM
                        // their (virtual) INTERSECTION, so the angle equals the visual opening between the
                        // segments. Otherwise the direction (b - a) follows the order of the points of the line
                        // and may point INTO the vertex, showing THE SUPPLEMENT (30 degrees where the eye sees
                        // 150).
                        let ix = line_line_ix(pa1, pb1, pa2, pb2).unwrap_or(pb1);
                        let far = |p: Point2, q: Point2| (q.x - ix.x).powi(2) + (q.y - ix.y).powi(2) > (p.x - ix.x).powi(2) + (p.y - ix.y).powi(2);
                        // (start is the end nearer to ix, end the further one), so the direction end - start
                        // points outwards
                        let (na1, nb1, ea, eb) = if far(pa1, pb1) { (a1, b1, pa1, pb1) } else { (b1, a1, pb1, pa1) };
                        let (na2, nb2, ec, ed) = if far(pa2, pb2) { (a2, b2, pa2, pb2) } else { (b2, a2, pb2, pa2) };
                        let (u, v) = ((eb.x - ea.x, eb.y - ea.y), (ed.x - ec.x, ed.y - ec.y));
                        let deg = (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1).abs().to_degrees();
                        Some(Constraint::AngleLines { a: na1, b: nb1, c: na2, d: nb2, deg, expr: String::new(), driven: false })
                    } else {
                        None
                    }
                }
            }
        };
        if let Some(c) = c {
            self.project.sketches[si].constraints.push(c);
            let ci = self.project.sketches[si].constraints.len() - 1;
            let (redundant, conflict) = self.finish_dim(si, ci);
            self.place.dim = Some(ci);
            self.status = if conflict {
                format!("{} {}", ph::WARNING, crate::i18n::tr("sk-dim-conflict"))
            } else if redundant {
                crate::i18n::tr("sk-driven-dim-hint").into()
            } else {
                crate::i18n::tr("sk-drag-dim-hint").into()
            };
        }
    }
}

// SKETCHES: the dimension that follows the cursor until it is placed, and inferring that a point belongs
// to a segment.
impl App {
    /// While the dimension follows the cursor, its offset (`off`) is updated to match.
    pub(super) fn update_placing_dim(&mut self, rect: Rect) {
        use qymcad_core::model::Constraint;
        let Some(ci) = self.place.dim else { return };
        let Sel::Sketch(si) = self.sel else {
            self.place.dim = None;
            return;
        };
        let Some(cur) = self.cursor else { return };
        let sc = self.to_screen(rect, cur);
        match self.project.sketches[si].constraints.get(ci).cloned() {
            Some(Constraint::Distance { a, b, .. }) => {
                if let (Some(pa), Some(pb)) = (self.sketch_pt(si, a), self.sketch_pt(si, b)) {
                    let (sa, sb) = (self.to_screen(rect, pa), self.to_screen(rect, pb));
                    let mid = ((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2();
                    // THE ORIENTATION follows the cursor: to the side gives a vertical dimension (dy), above or
                    // below a horizontal one (dx), anything else an aligned one.
                    let (cx, cy) = (sc.x - mid.x, sc.y - mid.y);
                    let new_axis = if cx.abs() > cy.abs() * 1.7 { 2u8 } else if cy.abs() > cx.abs() * 1.7 { 1u8 } else { 0u8 };
                    // the offset of the line: along Y for a horizontal dimension, along X for a vertical one,
                    // along the perpendicular for an aligned one.
                    // The offset is in WORLD units: the screen shift divided by the scale (for an aligned one
                    // the base gap of 16 px is subtracted)
                    let vscale = self.view.scale as f64;
                    let off = match new_axis {
                        1 => (sc.y - mid.y) as f64 / vscale,
                        2 => (sc.x - mid.x) as f64 / vscale,
                        _ => {
                            let dir = (sb - sa).normalized();
                            let perp = egui::vec2(-dir.y, dir.x);
                            (((sc - sa).x * perp.x + (sc - sa).y * perp.y) as f64 - 16.0) / vscale
                        }
                    };
                    // the measured value for the chosen axis, in world coordinates
                    let measured = match new_axis {
                        1 => (pa.x - pb.x).abs(),
                        2 => (pa.y - pb.y).abs(),
                        _ => ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt(),
                    };
                    if let Some(Constraint::Distance { off: o, axis, d, .. }) = self.project.sketches[si].constraints.get_mut(ci) {
                        *o = off;
                        *axis = new_axis;
                        *d = measured; // while it flies it measures; the value is typed when it is placed
                    }
                }
            }
            Some(Constraint::DistancePL { p, a, b, .. }) => {
                if let (Some(pp), Some(pa), Some(ab)) = (self.sketch_pt(si, p), self.sketch_pt(si, a), self.line_screen_dir(si, a, b, rect)) {
                    let (sp, sa) = (self.to_screen(rect, pp), self.to_screen(rect, pa));
                    let foot = sa + ab * (sp - sa).dot(ab);
                    // the leader runs ALONG the line (ab), exactly as in the drawing, the hit test and the
                    // caption. The label sticks to the cursor along the direction of the line: the cursor goes
                    // up and the label goes up, not sideways.
                    let off = (sc - foot).dot(ab) as f64 / self.view.scale as f64; // WORLD units, through the scale
                    if let Some(Constraint::DistancePL { off: o, .. }) = self.project.sketches[si].constraints.get_mut(ci) {
                        *o = off;
                    }
                }
            }
            _ => {
                self.place.dim = None;
            }
        }
    }

    /// Automatic constraints while drawing the segment prev-p1-p2: horizontal or vertical, perpendicular to
    /// the previous segment, and a point-on-edge for the new end. Every constraint is added ONLY if it is
    /// independent (does not over-define the sketch), so no redundant ones appear.
    pub(super) fn infer_on_segment(&mut self, si: usize, prev: Option<Point2>, p1: Point2, p2: Point2) {
        use qymcad_core::model::Constraint;
        let a = self.project.sketch_point_at(si, p1.x, p1.y, 1e-6);
        let b = self.project.sketch_point_at(si, p2.x, p2.y, 1e-6);
        let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
        let tol = 0.06; // ~3.5°
        // 1) horizontal or vertical
        let mut axis_aligned = false;
        if dy <= dx * tol && dx > 1e-6 {
            axis_aligned = self.project.add_constraint_if_independent(si, Constraint::Horizontal { a, b });
        } else if dx <= dy * tol && dy > 1e-6 {
            axis_aligned = self.project.add_constraint_if_independent(si, Constraint::Vertical { a, b });
        }
        // 2) perpendicular to the previous segment (only when the new one did not land on an axis, or it
        // would be a duplicate)
        if !axis_aligned {
            if let Some(pv) = prev {
                let pa = self.project.sketch_point_at(si, pv.x, pv.y, 1e-6);
                let (ux, uy) = (p1.x - pv.x, p1.y - pv.y);
                let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
                let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
                if lu > 1e-6 && lv > 1e-6 {
                    let cosang = (ux * vx + uy * vy) / (lu * lv);
                    if cosang.abs() < 0.06 {
                        self.project.add_constraint_if_independent(si, Constraint::Perpendicular { a: pa, b: a, c: a, d: b });
                    }
                }
            }
        }
        // 2b) PARALLEL to the nearest non-axis line (when the new one did not land on an axis itself)
        if !axis_aligned {
            if let Some((la, lb)) = self.nearest_parallel_line(si, p1, p2, a, b) {
                self.project.add_constraint_if_independent(si, Constraint::Parallel { a: la, b: lb, c: a, d: b });
            }
        }
        // 2c) TANGENT to the nearest circle or arc that is almost tangent already
        if let Some((cen, r)) = self.nearest_tangent_circle(si, p1, p2) {
            self.project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
        }
        // 2d) EQUAL LENGTH to the nearest line of the same length
        if let Some((la, lb)) = self.nearest_equal_line(si, p1, p2, a, b) {
            self.project.add_constraint_if_independent(si, Constraint::Equal { a: la, b: lb, c: a, d: b });
        }
        // 3) point on an edge: the new end landed on an existing line (not its own), so it is tied to it
        if let Some((la, lb)) = self.line_under_point(si, p2, a, b) {
            self.project.add_constraint_if_independent(si, Constraint::PointOnLine { p: b, a: la, b: lb });
        }
        // (a coincidence with a vertex happens by itself: `sketch_point_at(1e-6)` already shares the point)
        self.project.solve_sketch(si);
    }
}

impl App {
    /// THE DEFINEDNESS OF A SKETCH in one line - the single definition for the whole application.
    ///
    /// The line was produced in two places by two copies, and they had drifted apart: one said "CONSTRAINT
    /// CONFLICT" and the other "DIMENSION CONFLICT" about the same state. The priority follows the meaning:
    /// A CONFLICT (a real error), then under-defined (there is something left to add), then harmless
    /// redundancy (consistent reference dimensions, NOT an error), then fully defined. Any `redun > 0` used
    /// to paint it red, and consistent reference dimensions looked like a fault.
    pub(super) fn sketch_dof_line(&self, si: usize) -> (String, Color32) {
        let diag = self.sketch_diag(si);
        let (dof, redun) = diag.dof;
        let (lbl, col) = if !diag.conflicts.is_empty() {
            (crate::i18n::tr("dof-conflict"), self.scheme.pal.error())
        } else if dof > 0 {
            (crate::i18n::tr("dof-underdefined"), self.scheme.pal.underdefined())
        } else if redun > 0 {
            (crate::i18n::tr("dof-defined-ref"), self.scheme.pal.ok_soft())
        } else {
            (crate::i18n::tr("dof-fully-defined"), self.scheme.pal.ok())
        };
        (crate::i18n::tr2("dof-line", "n", &dof.to_string(), "state", &lbl), col)
    }
}

impl App {
    /// READ WHAT WAS TYPED INTO A SKETCH FIELD: a number OR an expression over the global variables.
    ///
    /// The size fields that follow drawing (the width and height of a rectangle or an ellipse, the radius and
    /// angle of a polygon, a corner fillet, a rotation) used to read the value through `parse::<f64>()` -
    /// that is, they accepted a bare number only. A formula or a global variable could not be typed, although
    /// those are WORKING dimensions of a part: "width = housing - 2*wall" is an everyday thing.
    ///
    /// An empty or broken expression gives `None`, and the caller keeps the previous value: no rubbish must
    /// travel into the model.
    pub(super) fn parse_num(&self, text: &str) -> Option<f64> {
        let t = text.trim().replace(',', ".");
        if t.is_empty() {
            return None;
        }
        qymcad_core::expr::eval(&t, &self.project.param_map()).ok().filter(|v| v.is_finite())
    }
}
