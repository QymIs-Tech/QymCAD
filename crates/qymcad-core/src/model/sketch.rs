//! Sketches: planar geometry, contours and their solving.
//!
//! The split is mechanical: a method belongs here because it touches the fields of this subsystem alone, not
//! because of what it is called.

use super::*;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

impl Project {
    /// Add a contour and return its stable id.
    pub fn add_contour(&mut self, c: Contour) -> Id {
        let id = self.alloc_id();
        self.contours.push(id, c);
        id
    }
    /// Replace the contour geometry entirely (a fresh import), handing out new ids.
    pub fn set_contours(&mut self, cs: Vec<Contour>) {
        self.contours.clear();
        self.add_contours(cs);
    }
    /// Delete a contour by index. Its id goes with it, and references to it in operations simply stop
    /// resolving, so no re-indexing is needed.
    pub fn remove_contour(&mut self, index: usize) {
        if let Some(cid) = self.contours.remove_at(index) {
            // Detach it from the sketches.
            for s in &mut self.sketches {
                s.contour_ids.retain(|x| *x != cid);
            }
        }
    }
    /// The sketch to draw into: the first non-imported one, or a new one.
    pub fn drawing_sketch(&mut self) -> usize {
        if let Some(i) = self.sketches.iter().position(|s| s.source.is_none()) {
            return i;
        }
        let id = self.alloc_id();
        self.sketches.push(Sketch { id, name: "name-sketch".into(), contour_ids: Vec::new(), source: None, points: Vec::new(), entities: Vec::new(), closed: false, constraints: Vec::new(), splines: Vec::new(), notes: Vec::new(), texts: Vec::new(), patterns: Vec::new(), projections: Vec::new(), plane: crate::feature::SketchPlane::default(), origin: 0, axis_pts: [0, 0], origin_uv: None });
        self.sketches.len() - 1
    }
    /// Add an ellipse as a real entity: a centre plus the endpoints of the major and minor semi-axes.
    ///
    /// `rot` is the rotation of the major axis in radians. Returns the id of the centre, which serves as the
    /// handle. The semi-axes are kept perpendicular by an implicit constraint, so the ellipse is parametric
    /// with five degrees of freedom rather than a polygon.
    pub fn add_ellipse_entity(&mut self, si: usize, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, purpose: crate::feature::Purpose) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        let (ux, uy) = (rot.cos(), rot.sin());
        let c = self.sketch_point_at(si, cx, cy, 1e-6);
        let ma = self.sketch_point_at(si, cx + rx * ux, cy + rx * uy, 1e-6); // Endpoint of the major axis.
        let mi = self.sketch_point_at(si, cx - ry * uy, cy + ry * ux, 1e-6); // Endpoint of the minor axis, perpendicular to it.
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Ellipse { c, ma, mi }, construction });
        self.regen_sketch(si);
        c
    }
    /// The major and minor semi-axes of an ellipse entity, by the id of its centre.
    pub fn ellipse_axes(&self, si: usize, center: Id) -> Option<(f64, f64)> {
        let s = self.sketches.get(si)?;
        let (ma, mi) = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == center => Some((ma, mi)),
            _ => None,
        })?;
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ((cx, cy), (max, may), (mix, miy)) = (p(center)?, p(ma)?, p(mi)?);
        Some((((max - cx).powi(2) + (may - cy).powi(2)).sqrt(), ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt()))
    }
    /// Set the semi-axes of an ellipse entity (`rx` major, `ry` minor) by the id of its centre. The axis
    /// endpoints move along their current directions, so the rotation is preserved, and the solver keeps them
    /// perpendicular.
    pub fn set_ellipse_axes(&mut self, si: usize, center: Id, rx: f64, ry: f64) -> bool {
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        let Some(s) = self.sketches.get(si) else { return false };
        let Some((ma, mi)) = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == center => Some((ma, mi)),
            _ => None,
        }) else {
            return false;
        };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (Some((cx, cy)), Some((max, may)), Some((mix, miy))) = (p(center), p(ma), p(mi)) else { return false };
        // Unit vectors of the current axes; the rotation is preserved.
        let dma = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-9);
        let dmi = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-9);
        let (uax, uay) = ((max - cx) / dma, (may - cy) / dma);
        let (ubx, uby) = ((mix - cx) / dmi, (miy - cy) / dmi);
        for (pid, (x, y)) in [(ma, (cx + rx * uax, cy + rx * uay)), (mi, (cx + ry * ubx, cy + ry * uby))] {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                q.x = x;
                q.y = y;
            }
        }
        self.solve_sketch(si);
        true
    }
    /// Vertices of a parametric polygon: the points bound by `PointOnCircle` to the centre of its circumscribed
    /// circle, in creation order. The handle of the polygon is the id of that centre.
    pub(super) fn polygon_vertices(&self, si: usize, center: Id) -> Vec<Id> {
        self.sketches
            .get(si)
            .map(|s| {
                s.constraints
                    .iter()
                    .filter_map(|c| match *c {
                        Constraint::PointOnCircle { p, c: cc } if cc == center => Some(p),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Set the circumscribed radius of a polygon through its driving dimension, letting the solver rescale it.
    /// `center` is the id of the circle centre. Returns `false` when the entity is not a polygon.
    pub fn set_polygon_radius(&mut self, si: usize, center: Id, r: f64) -> bool {
        let r = r.max(0.01);
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let mut found = false;
        for c in s.constraints.iter_mut() {
            if let Constraint::Diameter { c: cc, d, diam, .. } = c {
                if *cc == center {
                    *d = if *diam { 2.0 * r } else { r };
                    found = true;
                }
            }
        }
        if !found {
            // Without a dimension, the circle radius is edited directly and the solver keeps the vertices on
            // it.
            for e in s.entities.iter_mut() {
                if let EntityKind::Circle { center: cc, r: er } = &mut e.kind {
                    if *cc == center {
                        *er = r;
                        found = true;
                    }
                }
            }
        }
        if found {
            self.solve_sketch(si);
        }
        found
    }
    /// Centre x, centre y and radius of the circumscribed circle of a polygon, by the id of its centre.
    pub fn polygon_circle(&self, si: usize, center: Id) -> Option<(f64, f64, f64)> {
        let s = self.sketches.get(si)?;
        let (cx, cy) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y))?;
        let r = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { center: cc, r } if cc == center => Some(r),
            _ => None,
        })?;
        Some((cx, cy, r))
    }
    /// Rotation angle of a polygon in radians: the direction from the centre to the first vertex.
    pub fn polygon_angle(&self, si: usize, center: Id) -> Option<f64> {
        let s = self.sketches.get(si)?;
        let (cx, cy) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y))?;
        let v0 = *self.polygon_vertices(si, center).first()?;
        let (vx, vy) = s.points.iter().find(|q| q.id == v0).map(|q| (q.x, q.y))?;
        Some((vy - cy).atan2(vx - cx))
    }
    /// Rotate a polygon to an absolute angle `a` in radians: the vertices move around the centre at a constant
    /// radius and the solver keeps the shape regular. The rotation is a free degree of freedom, not a
    /// dimension.
    pub fn set_polygon_angle(&mut self, si: usize, center: Id, a: f64) -> bool {
        let verts = self.polygon_vertices(si, center);
        let Some((cx, cy, r)) = self.polygon_circle(si, center) else { return false };
        if verts.is_empty() {
            return false;
        }
        let cur = {
            let Some(q) = self.sketches[si].points.iter().find(|q| q.id == verts[0]) else { return false };
            (q.y - cy).atan2(q.x - cx)
        };
        let delta = a - cur;
        for &vid in &verts {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == vid) {
                let ang = (q.y - cy).atan2(q.x - cx) + delta;
                q.x = cx + r * ang.cos();
                q.y = cy + r * ang.sin();
            }
        }
        self.solve_sketch(si);
        true
    }
    /// Create a typed sketch from a polyline (a chain of lines through points). The tessellation goes into a
    /// contour with a stable id, which the toolpath side references.
    pub fn add_line_sketch(&mut self, name: impl Into<String>, pts: Vec<Point2>, closed: bool) -> Id {
        let points: Vec<SketchPoint> = pts.iter().map(|p| SketchPoint { id: self.alloc_id(), x: p.x, y: p.y }).collect();
        let mut entities = Vec::new();
        let n = points.len();
        let last = if closed { n } else { n.saturating_sub(1) };
        for k in 0..last {
            let a = points[k].id;
            let b = points[(k + 1) % n].id;
            let id = self.alloc_id();
            entities.push(SketchEntity { id, kind: EntityKind::Line { a, b }, construction: false });
        }
        let sid = self.alloc_id();
        let si = self.sketches.len();
        self.sketches.push(Sketch {
            id: sid,
            name: name.into(),
            contour_ids: Vec::new(),
            source: None,
            points,
            entities,
            closed,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(), projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            origin_uv: None,
        });
        self.regen_sketch(si);
        sid
    }
    /// Create an empty sketch (to enter edit mode) and return its index.
    pub fn new_sketch(&mut self, name: impl Into<String>) -> usize {
        let id = self.alloc_id();
        self.sketches.push(Sketch {
            id,
            name: name.into(),
            contour_ids: Vec::new(),
            source: None,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            splines: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(), projections: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            origin_uv: None,
        });
        self.sketches.len() - 1
    }
    /// Fill the profile of an existing sketch from a polyline (points, lines and a contour). Returns `false`
    /// when the sketch already has profile entities.
    pub fn fill_sketch_polyline(&mut self, si: usize, pts: Vec<Point2>, closed: bool) -> bool {
        if pts.len() < 2 || si >= self.sketches.len() {
            return false;
        }
        if self.sketches[si].entities.iter().any(|e| !e.construction) {
            return false; // A profile already exists and is not overwritten.
        }
        let points: Vec<SketchPoint> = pts.iter().map(|p| SketchPoint { id: self.alloc_id(), x: p.x, y: p.y }).collect();
        let mut entities = Vec::new();
        let n = points.len();
        let last = if closed { n } else { n - 1 };
        for k in 0..last {
            let id = self.alloc_id();
            entities.push(SketchEntity { id, kind: EntityKind::Line { a: points[k].id, b: points[(k + 1) % n].id }, construction: false });
        }
        let s = &mut self.sketches[si];
        s.points.extend(points);
        s.entities.extend(entities);
        s.closed = closed;
        self.regen_sketch(si);
        true
    }
    /// Degrees of freedom of a sketch from the rank of the constraint Jacobian, which accounts for redundancy.
    ///
    /// Returns the number of degrees of freedom and the number of redundant constraints. Zero and zero means
    /// fully constrained; a non-zero second value means there are redundant constraints.
    pub fn sketch_dof(&self, si: usize) -> (i32, i32) {
        let Some(s) = self.sketches.get(si) else { return (0, 0) };
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(self.entity_intrinsics(si)); // Arcs keep their endpoints on the circle: five degrees of
                                                   // freedom for a free arc.
        crate::solver::dof(&s.points, &self.entity_radii(si), &active)
    }
    /// Point ids referenced by constraint `ci` of a sketch, used to highlight geometry on hover.
    pub fn sketch_constraint_points(&self, si: usize, ci: usize) -> Vec<Id> {
        self.sketches.get(si).and_then(|s| s.constraints.get(ci)).map(constraint_point_ids).unwrap_or_default()
    }
    /// Whether dimension `ci` is redundant: adding it does not raise the rank of the Jacobian, so the geometry
    /// is already determined by other constraints. Distance and angle dimensions only.
    pub fn dim_redundant(&self, si: usize, ci: usize) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        if !matches!(s.constraints.get(ci), Some(Constraint::Distance { .. }) | Some(Constraint::Angle { .. }) | Some(Constraint::DistancePL { .. }) | Some(Constraint::AngleLines { .. }) | Some(Constraint::ArcLength { .. }) | Some(Constraint::Diameter { .. }) | Some(Constraint::EdgeDistance { .. })) {
            return false;
        }
        let intr = self.entity_intrinsics(si);
        let mut without: Vec<Constraint> = s.constraints.iter().enumerate().filter(|(i, c)| *i != ci && !c.is_driven()).map(|(_, c)| c.clone()).collect();
        without.extend(intr.iter().cloned());
        let radii = self.entity_radii(si);
        let (dof_without, _) = crate::solver::dof(&s.points, &radii, &without);
        let mut with = without;
        with.push(s.constraints[ci].clone());
        let (dof_with, _) = crate::solver::dof(&s.points, &radii, &with);
        dof_with == dof_without // The rank did not grow, so the dimension constrains nothing.
    }
    /// Redundant constraints: non-reference constraints whose removal frees no degree of freedom (the rank of
    /// the Jacobian does not drop), so they can be removed without losing determinacy.
    ///
    /// It names the specific redundant constraints rather than only counting them. Computed only when there is
    /// an excess to explain.
    pub fn sketch_redundant_constraints(&self, si: usize) -> Vec<usize> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let (_, redun) = self.sketch_dof(si);
        if redun <= 0 {
            return Vec::new();
        }
        let radii = self.entity_radii(si);
        let intr = self.entity_intrinsics(si);
        let active_all: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        let mut base = active_all.clone();
        base.extend(intr.iter().cloned());
        let (dof_all, _) = crate::solver::dof(&s.points, &radii, &base);
        let mut out = Vec::new();
        for (ci, c) in s.constraints.iter().enumerate() {
            if c.is_driven() {
                continue;
            }
            let mut without: Vec<Constraint> = s.constraints.iter().enumerate().filter(|(i, cc)| *i != ci && !cc.is_driven()).map(|(_, cc)| cc.clone()).collect();
            without.extend(intr.iter().cloned());
            let (dof_without, _) = crate::solver::dof(&s.points, &radii, &without);
            if dof_without == dof_all {
                out.push(ci); // Removing it did not raise the degrees of freedom, so the constraint is
                              // redundant (one of an interdependent set).
            }
        }
        out
    }
    /// Add a constraint only when it is independent, that is, when it reduces the degrees of freedom by raising
    /// the rank of the Jacobian. Redundant automatic constraints (inferred while drawing) are dropped, so the
    /// sketch is not over-constrained. Returns whether the constraint was added.
    pub fn add_constraint_if_independent(&mut self, si: usize, c: Constraint) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let intr = self.entity_intrinsics(si);
        let radii = self.entity_radii(si);
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|x| !x.is_driven()).cloned().collect();
        active.extend(intr.iter().cloned());
        let (dof_before, _) = crate::solver::dof(&s.points, &radii, &active);
        let mut with = active;
        with.push(c.clone());
        let (dof_after, _) = crate::solver::dof(&s.points, &radii, &with);
        if dof_after < dof_before {
            self.sketches[si].constraints.push(c);
            true
        } else {
            false
        }
    }
    /// If dimension `ci` is redundant, turn it into a reference (driven) dimension. Returns whether it did.
    pub fn auto_driven(&mut self, si: usize, ci: usize) -> bool {
        if !self.dim_redundant(si, ci) {
            return false;
        }
        if let Some(c) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci)) {
            match c {
                Constraint::AngleLines { driven, .. } | Constraint::Distance { driven, .. } | Constraint::Angle { driven, .. } | Constraint::DistancePL { driven, .. } | Constraint::Diameter { driven, .. } | Constraint::ArcLength { driven, .. } | Constraint::EdgeDistance { driven, .. } => {
                    *driven = true;
                    return true;
                }
                _ => {}
            }
        }
        false
    }
    /// Toggle dimension `ci` between driving and reference. Returns the new state.
    pub fn toggle_driven(&mut self, si: usize, ci: usize) -> bool {
        if let Some(c) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci)) {
            match c {
                Constraint::AngleLines { driven, .. } | Constraint::Distance { driven, .. } | Constraint::Angle { driven, .. } | Constraint::DistancePL { driven, .. } | Constraint::Diameter { driven, .. } | Constraint::ArcLength { driven, .. } => {
                    *driven = !*driven;
                    return *driven;
                }
                _ => {}
            }
        }
        false
    }
    /// Mobility mask of the sketch points (`true` means the point can still move), one entry per point.
    pub fn sketch_free_points(&self, si: usize) -> Vec<bool> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(self.entity_intrinsics(si));
        crate::solver::free_points(&s.points, &self.entity_radii(si), &active)
    }
    /// Add a text note to a sketch.
    pub fn add_note(&mut self, si: usize, x: f64, y: f64, text: String) {
        if let Some(s) = self.sketches.get_mut(si) {
            s.notes.push(Note { x, y, text });
        }
    }
    /// Add parametric text geometry. `glyphs` are the glyph polylines baked by the application (in world
    /// coordinates, since the font lives there). Returns the id of the text; the contours are updated.
    pub fn add_sketch_text(&mut self, si: usize, x: f64, y: f64, height: f64, angle: f64, text: String, purpose: crate::feature::Purpose, glyphs: Vec<Vec<Point2>>) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let id = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            s.texts.push(SketchText { id, x, y, height, angle, text, construction, glyphs });
        }
        self.regen_sketch(si);
        id
    }
    /// Update the parameters of a text and its baked glyphs, after the application re-baked them for a new
    /// font or string.
    pub fn set_sketch_text(&mut self, si: usize, ti: usize, x: f64, y: f64, height: f64, angle: f64, text: String, glyphs: Vec<Vec<Point2>>) {
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            t.x = x;
            t.y = y;
            t.height = height;
            t.angle = angle;
            t.text = text;
            t.glyphs = glyphs;
        }
        self.regen_sketch(si);
    }
    /// Move a text by (dx, dy), shifting both its parameters and its baked glyphs, so no font is needed.
    pub fn move_sketch_text(&mut self, si: usize, ti: usize, dx: f64, dy: f64) {
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            t.x += dx;
            t.y += dy;
            for loop_ in t.glyphs.iter_mut() {
                for p in loop_.iter_mut() {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self.regen_sketch(si);
    }
    /// Delete text `ti` from a sketch.
    pub fn delete_sketch_text(&mut self, si: usize, ti: usize) {
        if let Some(s) = self.sketches.get_mut(si) {
            if ti < s.texts.len() {
                s.texts.remove(ti);
            }
        }
        self.regen_sketch(si);
    }
    /// Bounding box of a text (min_x, min_y, max_x, max_y) from its baked glyphs.
    pub fn sketch_text_bbox(&self, si: usize, ti: usize) -> Option<(f64, f64, f64, f64)> {
        let t = self.sketches.get(si)?.texts.get(ti)?;
        let mut bb: Option<(f64, f64, f64, f64)> = None;
        for loop_ in &t.glyphs {
            for p in loop_ {
                bb = Some(match bb {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
                });
            }
        }
        // Fallback bounding box when the glyphs are empty (a space, or a non-printing character).
        bb.or(Some((t.x, t.y, t.x + t.height * 0.5, t.y + t.height)))
    }
    /// Add a spline (a smooth Catmull-Rom curve) through control points.
    pub fn add_spline(&mut self, si: usize, pts: Vec<Point2>, ends: crate::feature::Ends, purpose: crate::feature::Purpose) {
        let construction = purpose == crate::feature::Purpose::Construction;
        let closed = ends == crate::feature::Ends::Closed;
        if pts.len() < 2 || si >= self.sketches.len() {
            return;
        }
        let ids: Vec<Id> = pts.iter().map(|_| self.alloc_id()).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            for (id, p) in ids.iter().zip(&pts) {
                s.points.push(SketchPoint { id: *id, x: p.x, y: p.y });
            }
            let nt = ids.len();
            s.splines.push(Spline { points: ids, tangents: vec![None; nt], closed, construction });
        }
        self.regen_sketch(si);
    }
    /// Polyline of a spline (a cubic Hermite tessellation) in world coordinates, for drawing — including the
    /// dashed rendering of a construction spline, which never reaches the contours.
    pub fn spline_polyline(&self, si: usize, spi: usize) -> Vec<Point2> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(sp) = s.splines.get(spi) else { return Vec::new() };
        let cps: Vec<Point2> = sp.points.iter().filter_map(|id| s.points.iter().find(|p| p.id == *id).map(|p| Point2::new(p.x, p.y))).collect();
        if cps.len() < 2 {
            return Vec::new();
        }
        tessellate_spline_hermite(&cps, &sp.tangents, sp.closed).points
    }
    /// Tangent handles of spline `spi`: for each node, its position and the end of its handle, in world
    /// coordinates. A handle is the node plus its tangent, explicit or automatic. Used for drawing and
    /// dragging.
    pub fn spline_handles(&self, si: usize, spi: usize) -> Vec<(Point2, Point2)> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(sp) = s.splines.get(spi) else { return Vec::new() };
        let pos = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
        let cps: Vec<Point2> = sp.points.iter().filter_map(|id| pos(*id)).collect();
        let n = cps.len();
        if n < 2 {
            return Vec::new();
        }
        (0..n)
            .map(|i| {
                let m = spline_tangent_at(&cps, &sp.tangents, i, sp.closed);
                (cps[i], Point2::new(cps[i].x + m.x, cps[i].y + m.y))
            })
            .collect()
    }
    /// Set the tangent of node `ki` of spline `spi` from a dragged handle in world coordinates. This makes the
    /// tangent explicit, pinning the shape at that node, and rebuilds the contour. Returns `false` when there is
    /// no such node.
    pub fn set_spline_handle(&mut self, si: usize, spi: usize, ki: usize, hx: f64, hy: f64) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(sp) = s.splines.get(spi) else { return false };
        let Some(&kid) = sp.points.get(ki) else { return false };
        let Some((kx, ky)) = s.points.iter().find(|p| p.id == kid).map(|p| (p.x, p.y)) else { return false };
        let sp = &mut s.splines[spi];
        if sp.tangents.len() != sp.points.len() {
            sp.tangents = vec![None; sp.points.len()];
        }
        sp.tangents[ki] = Some([hx - kx, hy - ky]);
        self.regen_sketch(si);
        true
    }
    /// Reset the tangent of a node to automatic (Catmull-Rom), so the handle follows the nodes again.
    pub fn reset_spline_handle(&mut self, si: usize, spi: usize, ki: usize) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(sp) = s.splines.get_mut(spi) else { return false };
        if ki >= sp.points.len() {
            return false;
        }
        if sp.tangents.len() != sp.points.len() {
            sp.tangents = vec![None; sp.points.len()];
        }
        sp.tangents[ki] = None;
        self.regen_sketch(si);
        true
    }
    /// Add a construction polyline (a projection of part geometry into a sketch). Every segment is construction
    /// geometry: a support for snapping and construction, never a profile.
    pub fn add_construction_polyline(&mut self, si: usize, pts: &[Point2], closed: bool) {
        if pts.len() < 2 {
            return;
        }
        let pids: Vec<Id> = pts.iter().map(|_| self.alloc_id()).collect();
        let n = pids.len();
        let last = if closed { n } else { n - 1 };
        let eids: Vec<Id> = (0..last).map(|_| self.alloc_id()).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            for (id, p) in pids.iter().zip(pts) {
                s.points.push(SketchPoint { id: *id, x: p.x, y: p.y });
            }
            for (k, eid) in eids.into_iter().enumerate() {
                s.entities.push(SketchEntity { id: eid, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % n] }, construction: true });
            }
        }
    }
    /// Unique points referenced by the given entities, centres included.
    pub fn entity_point_ids(&self, si: usize, eids: &[Id]) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out: Vec<Id> = Vec::new();
        for e in &s.entities {
            if !eids.contains(&e.id) {
                continue;
            }
            let ids: Vec<Id> = match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            };
            for id in ids {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }
    /// Centroid of the selected entities (the mean of their points).
    pub fn entities_centroid(&self, si: usize, eids: &[Id]) -> (f64, f64) {
        let pts = self.entity_point_ids(si, eids);
        let Some(s) = self.sketches.get(si) else { return (0.0, 0.0) };
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for p in &s.points {
            if pts.contains(&p.id) {
                sx += p.x;
                sy += p.y;
                n += 1.0;
            }
        }
        if n > 0.0 {
            (sx / n, sy / n)
        } else {
            (0.0, 0.0)
        }
    }
    /// Prune orphaned sketch points (used by no entity and no spline) together with the constraints on them.
    ///
    /// Without this, trimming leaves dangling points behind that have to be deleted by hand. Protected: the
    /// system points (origin and axes), materialised midpoints, virtual fillet corners (points held by
    /// `PointOnLine` or `PointOnCircle`, which dimensions are measured to) and any point carrying a
    /// dimension.
    pub fn prune_orphan_sketch_points(&mut self, si: usize) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let mut used: std::collections::HashSet<Id> = s
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            })
            .collect();
        for sp in &s.splines {
            used.extend(sp.points.iter().copied());
        }
        let mut protected: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        for c in &s.constraints {
            let keep = match c {
                Constraint::Midpoint { p, .. } => Some(vec![*p]),
                // A virtual fillet corner is held by its supports and dimensions are measured to it.
                Constraint::PointOnLine { p, .. } | Constraint::PointOnCircle { p, .. } => Some(vec![*p]),
                _ if c.dim_value().is_some() => Some(constraint_point_ids(c)),
                _ => None,
            };
            if let Some(ids) = keep {
                protected.extend(ids);
            }
        }
        s.points.retain(|p| used.contains(&p.id) || protected.contains(&p.id));
        let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
        s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
    }
    /// Delete the selected entities, together with the points they orphan, the constraints on those points,
    /// and the constraints that hung on the deleted lines by their pair of endpoints — even when those
    /// endpoints are shared and still alive.
    pub fn delete_entities(&mut self, si: usize, eids: &[Id]) {
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            // Endpoint pairs of the lines being deleted: constraints referencing those lines go as well.
            let dead_lines: Vec<(Id, Id)> = s
                .entities
                .iter()
                .filter(|e| eids.contains(&e.id))
                .filter_map(|e| match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    _ => None,
                })
                .collect();
            s.entities.retain(|e| !eids.contains(&e.id));
            // Drop the constraints attached to a deleted line (horizontal, vertical, parallel, perpendicular,
            // equal, collinear, tangent, midpoint — those referencing its pair of endpoints), or dangling
            // glyphs remain.
            if !dead_lines.is_empty() {
                s.constraints.retain(|c| !constraint_uses_line(c, &dead_lines));
            }
            let used: std::collections::HashSet<Id> = s
                .entities
                .iter()
                .flat_map(|e| match e.kind {
                    EntityKind::Line { a, b } => vec![a, b],
                    EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                    EntityKind::Circle { center, .. } => vec![center],
                    EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
                })
                .collect();
            // Protected free points: the system ones (origin and axes) and materialised midpoints. They must
            // not be dropped as orphaned endpoints, or the axes and the dimensions measured from them are
            // lost.
            let protected: std::collections::HashSet<Id> = s
                .system_ids()
                .into_iter()
                .chain(s.constraints.iter().filter_map(|c| if let Constraint::Midpoint { p, .. } = c { Some(*p) } else { None }))
                .collect();
            s.points.retain(|p| used.contains(&p.id) || protected.contains(&p.id));
            let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
            s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
        }
        self.regen_sketch(si);
    }
    /// Toggle the selected entities between ordinary and construction geometry. Construction geometry never
    /// reaches a profile. Returns the new state.
    pub fn toggle_construction(&mut self, si: usize, eids: &[Id]) -> bool {
        let mut now_construction = false;
        if let Some(s) = self.sketches.get_mut(si) {
            // The target state is the inverse of the first selected entity.
            let target = !s.entities.iter().find(|e| eids.contains(&e.id)).map_or(false, |e| e.construction);
            for e in s.entities.iter_mut() {
                if eids.contains(&e.id) {
                    e.construction = target;
                }
            }
            now_construction = target;
        }
        self.regen_sketch(si);
        now_construction
    }
    /// Delete the selected sketch points together with the entities incident to them and the constraints they
    /// orphan. Every other point, the origin included, is kept.
    pub fn delete_points(&mut self, si: usize, ids: &[Id]) {
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            let set: std::collections::HashSet<Id> = ids.iter().copied().collect();
            // Entities incident to the deleted points go as well.
            s.entities.retain(|e| {
                let inc = match e.kind {
                    EntityKind::Line { a, b } => set.contains(&a) || set.contains(&b),
                    EntityKind::Arc { center, a, b, .. } => set.contains(&center) || set.contains(&a) || set.contains(&b),
                    EntityKind::Circle { center, .. } => set.contains(&center),
                    EntityKind::Ellipse { c, ma, mi } => set.contains(&c) || set.contains(&ma) || set.contains(&mi),
                };
                !inc
            });
            s.points.retain(|p| !set.contains(&p.id));
            let alive: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
            s.constraints.retain(|c| constraint_point_ids(c).iter().all(|id| alive.contains(id)));
            s.splines.retain(|sp| sp.points.iter().all(|id| alive.contains(id)));
            if set.contains(&s.origin) {
                s.origin = 0;
            }
        }
        self.regen_sketch(si);
    }
    /// Delete constraint `ci` of a sketch. For a midpoint constraint the orphaned midpoint is pruned as well
    /// (nothing else uses it and it is not a system point), so no debris is left behind. The sketch is then
    /// re-solved.
    pub fn delete_sketch_constraint(&mut self, si: usize, ci: usize) -> bool {
        {
            let Some(s) = self.sketches.get_mut(si) else { return false };
            if ci >= s.constraints.len() {
                return false;
            }
            let removed = s.constraints.remove(ci);
            if let Constraint::Midpoint { p, .. } = removed {
                let used_ent = s.entities.iter().any(|e| match e.kind {
                    EntityKind::Line { a, b } => a == p || b == p,
                    EntityKind::Arc { center, a, b, .. } => center == p || a == p || b == p,
                    EntityKind::Circle { center, .. } => center == p,
                    EntityKind::Ellipse { c, ma, mi } => c == p || ma == p || mi == p,
                });
                let used_con = s.constraints.iter().any(|c| constraint_point_ids(c).contains(&p));
                let used_sp = s.splines.iter().any(|sp| sp.points.contains(&p));
                if !used_ent && !used_con && !used_sp && !s.system_ids().contains(&p) {
                    s.points.retain(|q| q.id != p); // The orphaned midpoint is removed.
                }
            }
        }
        self.solve_sketch(si);
        true
    }
    /// Move the selected entities by a vector.
    pub fn move_entities(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64) {
        let pts = self.entity_point_ids(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self.regen_sketch(si);
    }
    /// Rotate the selected entities about (cx, cy) by an angle in degrees.
    pub fn rotate_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, deg: f64) {
        let (sn, cs) = (deg.to_radians().sin(), deg.to_radians().cos());
        let pts = self.entity_point_ids(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    let (x, y) = (p.x - cx, p.y - cy);
                    p.x = cx + x * cs - y * sn;
                    p.y = cy + x * sn + y * cs;
                }
            }
        }
        self.regen_sketch(si);
    }
    /// Scale the selected entities about (cx, cy).
    pub fn scale_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, f: f64) {
        let f = if f.abs() < 1e-6 { 1.0 } else { f };
        let pts = self.entity_point_ids(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x = cx + (p.x - cx) * f;
                    p.y = cy + (p.y - cy) * f;
                }
            }
            for e in s.entities.iter_mut() {
                if eids.contains(&e.id) {
                    if let EntityKind::Circle { r, .. } = &mut e.kind {
                        *r *= f.abs();
                    }
                }
            }
        }
        self.regen_sketch(si);
    }
    /// Constraints internal to the point set `inside`: every reference that stays within the set, except
    /// `Fixed` — an absolute-position anchor is not copied, since the copy is placed elsewhere. References to
    /// the axes, the origin or outside points are filtered out automatically, their points not being in the
    /// set. The clones carry the old ids.
    pub(super) fn internal_constraints(&self, si: usize, inside: &std::collections::HashSet<Id>) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.constraints
            .iter()
            .filter(|c| {
                if matches!(c, Constraint::Fixed { .. }) {
                    return false;
                }
                let ps = constraint_point_ids(c);
                !ps.is_empty() && ps.iter().all(|p| inside.contains(p))
            })
            .cloned()
            .collect()
    }
    /// Duplicate entities with point transform `f`. `with_constraints` selects whether the constraints and
    /// dimensions internal to the set are carried over (horizontals, verticals, edge dimensions and so on, but
    /// no `Fixed` and no references to the axes). Returns the new ids.
    pub(super) fn dup_entities<F: Fn(f64, f64) -> (f64, f64)>(&mut self, si: usize, eids: &[Id], f: F, with_constraints: bool) -> Vec<Id> {
        let pids = self.entity_point_ids(si, eids);
        let coords: Vec<(Id, f64, f64)> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect()
        };
        let ents: Vec<SketchEntity> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect()
        };
        let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        for (old, x, y) in coords {
            let (nx, ny) = f(x, y);
            let nid = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id: nid, x: nx, y: ny });
            map.insert(old, nid);
        }
        let m = |id: Id| map.get(&id).copied().unwrap_or(id);
        let mut new_ids = Vec::new();
        for e in ents {
            let nk = match e.kind {
                EntityKind::Line { a, b } => EntityKind::Line { a: m(a), b: m(b) },
                EntityKind::Arc { center, a, b, ccw } => EntityKind::Arc { center: m(center), a: m(a), b: m(b), ccw },
                EntityKind::Circle { center, r } => EntityKind::Circle { center: m(center), r },
                EntityKind::Ellipse { c, ma, mi } => EntityKind::Ellipse { c: m(c), ma: m(ma), mi: m(mi) },
            };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            new_ids.push(id);
        }
        if with_constraints {
            // The internal constraints of the set are re-pointed at the new points, so the copy keeps its
            // shape but not its anchor or its links to the axes.
            let inside: std::collections::HashSet<Id> = map.keys().copied().collect();
            let cons: Vec<Constraint> = self.internal_constraints(si, &inside).iter().map(|c| remap_constraint_via(c, &map)).collect();
            self.sketches[si].constraints.extend(cons);
        }
        self.regen_sketch(si);
        new_ids
    }
    /// Capture the selected geometry of sketch `si` into the clipboard, for pasting into this sketch or
    /// another one. `eids` are the selected entities; the snapshot holds them and all their points (centres,
    /// endpoints). `(rx, ry)` is the reference point given at copy time. Does not mutate.
    pub fn copy_sketch_geometry(&self, si: usize, eids: &[Id], rx: f64, ry: f64) -> GeomClip {
        let pids = self.entity_point_ids(si, eids);
        let Some(s) = self.sketches.get(si) else { return GeomClip::default() };
        let points: Vec<(Id, f64, f64)> = s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect();
        let entities: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect();
        let inside: std::collections::HashSet<Id> = pids.iter().copied().collect();
        let constraints = self.internal_constraints(si, &inside); // Internal constraints and dimensions, without
                                                                  // `Fixed` or axis references.
        GeomClip { points, entities, constraints, ref_x: rx, ref_y: ry }
    }
    /// Paste snapshot `clip` into sketch `si` so that its reference point lands at `(tx, ty)`.
    ///
    /// Points and entities receive new ids and the references are remapped. Returns the ids of the pasted
    /// entities, for highlighting and selection. The constraints and dimensions internal to the copy are
    /// carried over, so the shape is preserved, while the anchor and any dimensions to the axes are not, their
    /// points not being in the set.
    pub fn paste_sketch_geometry(&mut self, si: usize, clip: &GeomClip, tx: f64, ty: f64) -> Vec<Id> {
        if si >= self.sketches.len() || clip.is_empty() {
            return Vec::new();
        }
        let (ox, oy) = (tx - clip.ref_x, ty - clip.ref_y);
        let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        for (old, x, y) in &clip.points {
            let nid = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id: nid, x: x + ox, y: y + oy });
            map.insert(*old, nid);
        }
        let m = |id: Id| map.get(&id).copied().unwrap_or(id);
        let mut new_ids = Vec::new();
        for e in &clip.entities {
            let nk = match e.kind {
                EntityKind::Line { a, b } => EntityKind::Line { a: m(a), b: m(b) },
                EntityKind::Arc { center, a, b, ccw } => EntityKind::Arc { center: m(center), a: m(a), b: m(b), ccw },
                EntityKind::Circle { center, r } => EntityKind::Circle { center: m(center), r },
                EntityKind::Ellipse { c, ma, mi } => EntityKind::Ellipse { c: m(c), ma: m(ma), mi: m(mi) },
            };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            new_ids.push(id);
        }
        // The internal constraints and dimensions of the copy are re-pointed at the new points; every
        // reference stays within the set and is remapped through the map.
        for c in &clip.constraints {
            let nc = remap_constraint_via(c, &map);
            self.sketches[si].constraints.push(nc);
        }
        self.regen_sketch(si);
        new_ids
    }
    /// An editable pattern: duplicates the source according to its parameters and records the pattern itself.
    /// Returns the pattern id, so it can be edited again. The instances are real entities and reach profiles and
    /// toolpaths.
    pub fn add_pattern(&mut self, si: usize, source: &[Id], kind: PatternKind) -> Id {
        let instances = self.pattern_instances(si, source, kind);
        let id = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            s.patterns.push(SketchPattern { id, source: source.to_vec(), kind, instances });
        }
        self.regen_sketch(si);
        id
    }
    /// Change the parameters of pattern `pi`: remove the old instances and recreate them with the new ones.
    pub fn update_pattern(&mut self, si: usize, pi: usize, kind: PatternKind) {
        let (source, old) = match self.sketches.get(si).and_then(|s| s.patterns.get(pi)) {
            Some(p) => (p.source.clone(), p.instances.clone()),
            None => return,
        };
        self.delete_entities(si, &old); // Remove the old copies and the points they orphan.
        let instances = self.pattern_instances(si, &source, kind);
        if let Some(p) = self.sketches.get_mut(si).and_then(|s| s.patterns.get_mut(pi)) {
            p.kind = kind;
            p.instances = instances;
        }
        self.regen_sketch(si);
    }
    /// Delete pattern `pi` together with its copies; the source stays.
    pub fn delete_pattern(&mut self, si: usize, pi: usize) {
        let old = match self.sketches.get(si).and_then(|s| s.patterns.get(pi)) {
            Some(p) => p.instances.clone(),
            None => return,
        };
        self.delete_entities(si, &old);
        if let Some(s) = self.sketches.get_mut(si) {
            if pi < s.patterns.len() {
                s.patterns.remove(pi);
            }
        }
        self.regen_sketch(si);
    }
    /// Index of the pattern that instance entity `eid` belongs to, so a click can reopen it for editing.
    pub fn pattern_of_entity(&self, si: usize, eid: Id) -> Option<usize> {
        self.sketches.get(si)?.patterns.iter().position(|p| p.instances.contains(&eid))
    }
    /// Attach cut point `pid` to the entity it actually lies on — excluding the ones already holding it as an
    /// endpoint or a centre: a line gives `PointOnLine`, a circle or arc gives `PointOnCircle`.
    ///
    /// Without it the cut point is free (along the angle on an arc, along the crossing entity on a line) and
    /// drifts off the intersection on the next solve, so the trimmed geometry wanders and twists and the
    /// dimensions and radii move with it. Attaching it to the crossing entity, together with the intrinsic
    /// constraint that keeps it on its own curve, pins the cut point to the intersection.
    pub(super) fn anchor_cut_point(&mut self, si: usize, pid: Id) {
        const TOL: f64 = 5e-3;
        let Some((px, py)) = self.point_xy(si, pid) else { return };
        let Some(s) = self.sketches.get(si) else { return };
        let owns = |k: &EntityKind| match *k {
            EntityKind::Line { a, b } => a == pid || b == pid,
            EntityKind::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
            EntityKind::Circle { center, .. } => center == pid,
            EntityKind::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
        };
        let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut anchor: Option<Constraint> = None;
        for e in &s.entities {
            if owns(&e.kind) {
                continue; // The point already belongs to this entity as an endpoint or a centre, so it is not
                          // anchored to it again.
            }
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) {
                        let (dx, dy) = (bx - ax, by - ay);
                        let len2 = dx * dx + dy * dy;
                        if len2 < 1e-12 {
                            continue;
                        }
                        let t = ((px - ax) * dx + (py - ay) * dy) / len2; // Parameter along the segment.
                        let perp = (dx * (py - ay) - dy * (px - ax)).abs() / len2.sqrt();
                        if perp < TOL && (-1e-6..=1.0 + 1e-6).contains(&t) {
                            anchor = Some(Constraint::PointOnLine { p: pid, a, b });
                            break;
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = pt(center) {
                        if (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r).abs() < TOL {
                            anchor = Some(Constraint::PointOnCircle { p: pid, c: center });
                            break;
                        }
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (pt(center), pt(a), pt(b)) {
                        let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                        let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                        let ang = (py - cy).atan2(px - cx);
                        if (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r).abs() < TOL && angle_in_arc(ang, a0, a1, ccw) {
                            anchor = Some(Constraint::PointOnCircle { p: pid, c: center });
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(c) = anchor {
            let dup = self.sketches[si].constraints.iter().any(|x| match (x, &c) {
                (Constraint::PointOnLine { p: p1, a: a1, b: b1 }, Constraint::PointOnLine { p: p2, a: a2, b: b2 }) => p1 == p2 && ((a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2)),
                (Constraint::PointOnCircle { p: p1, c: c1 }, Constraint::PointOnCircle { p: p2, c: c2 }) => p1 == p2 && c1 == c2,
                _ => false,
            });
            if !dup {
                self.sketches[si].constraints.push(c);
            }
        }
    }
    /// Trim segment `eid` at the clicked point: the segment is divided by its intersections with other
    /// entities and the piece containing the click is removed.
    pub fn trim_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        // Cut points from the intersections, as the parameter t along a to b.
        let ents = self.sketches[si].entities.clone();
        let mut cuts: Vec<f64> = Vec::new();
        for e in &ents {
            if e.id == eid {
                continue; // Construction geometry is a valid trim boundary.
            }
            match e.kind {
                EntityKind::Line { a: c, b: d } => {
                    if let (Some((cx, cy)), Some((dx, dy))) = (self.point_xy(si, c), self.point_xy(si, d)) {
                        if let Some(t) = seg_seg_t(pax, pay, pbx, pby, cx, cy, dx, dy) {
                            cuts.push(t);
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = self.point_xy(si, center) {
                        cuts.extend(seg_circle_t(pax, pay, pbx, pby, cx, cy, r));
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (self.point_xy(si, c), self.point_xy(si, ma), self.point_xy(si, mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        cuts.extend(seg_ellipse_t(pax, pay, pbx, pby, ex, ey, ux, uy, major, minor));
                    }
                }
                EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                    if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (self.point_xy(si, center), self.point_xy(si, aa), self.point_xy(si, bb)) {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        let a0 = (sy - cy).atan2(sx - cx);
                        let a1 = (ey - cy).atan2(ex - cx);
                        for t in seg_circle_t(pax, pay, pbx, pby, cx, cy, r) {
                            let (ix, iy) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
                            if angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                cuts.push(t);
                            }
                        }
                    }
                }
            }
        }
        if cuts.is_empty() {
            return false;
        }
        cuts.push(0.0);
        cuts.push(1.0);
        cuts.sort_by(|x, y| x.total_cmp(y));
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
        // Parameter of the click.
        let tc = ((clickx - pax) * (pbx - pax) + (clicky - pay) * (pby - pay)) / dlen2;
        let mut keep: Vec<(f64, f64)> = Vec::new();
        let mut removed = false;
        for w in cuts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if !removed && tc >= t0 - 1e-9 && tc <= t1 + 1e-9 {
                removed = true;
                continue;
            }
            keep.push((t0, t1));
        }
        if !removed {
            return false;
        }
        // Rebuild: delete the original segment and add the remaining pieces.
        self.sketches[si].entities.retain(|e| e.id != eid);
        let mut seg_pts: Vec<(Id, Id)> = Vec::new();
        for (t0, t1) in keep {
            let p0 = if t0 <= 1e-9 { a } else { self.sketch_point_at(si, pax + (pbx - pax) * t0, pay + (pby - pay) * t0, 1e-9) };
            let p1 = if t1 >= 1.0 - 1e-9 { b } else { self.sketch_point_at(si, pax + (pbx - pax) * t1, pay + (pby - pay) * t1, 1e-9) };
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: p0, b: p1 }, construction: false });
            seg_pts.push((p0, p1));
        }
        // The pieces of one line stay on one straight line: the interior cut points are held by `PointOnLine`
        // against the line through the outermost points, which the solver cannot collapse (the residual is
        // divided by the length). Otherwise editing a dimension spreads the pieces into a parallelogram and the
        // notch or arc flies away.
        let mut order: Vec<Id> = Vec::new();
        for &(p0, p1) in &seg_pts {
            if order.last() != Some(&p0) {
                order.push(p0);
            }
            order.push(p1);
        }
        order.dedup();
        if order.len() >= 3 {
            let (r0, r1) = (order[0], order[order.len() - 1]);
            for &pi in &order[1..order.len() - 1] {
                self.sketches[si].constraints.push(Constraint::PointOnLine { p: pi, a: r0, b: r1 });
            }
        }
        // The cut points (not the original endpoints) are anchored to the crossing curve — a circle, an arc or
        // another line — so a point stays at the intersection instead of sliding along it and the pieces do not
        // spread when a dimension is edited.
        let cut_pts: Vec<Id> = order.iter().copied().filter(|&pi| pi != a && pi != b).collect();
        for pi in cut_pts {
            self.anchor_cut_point(si, pi);
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.regen_sketch(si);
        true
    }
    /// Extend: the segment endpoint nearer to the click is stretched to the nearest intersection with another
    /// entity. Returns whether it was extended.
    pub fn extend_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        let tc = ((clickx - pax) * (pbx - pax) + (clicky - pay) * (pby - pay)) / dlen2;
        let extend_b = tc >= 0.5; // Stretch the endpoint further from the centre.
        let ents = self.sketches[si].entities.clone();
        let mut cand: Vec<f64> = Vec::new();
        for e in &ents {
            if e.id == eid {
                continue; // Construction geometry is a valid extension boundary.
            }
            match e.kind {
                EntityKind::Line { a: c, b: d } => {
                    if let (Some((cx, cy)), Some((dx, dy))) = (self.point_xy(si, c), self.point_xy(si, d)) {
                        if let Some(t) = line_seg_t(pax, pay, pbx, pby, cx, cy, dx, dy) {
                            cand.push(t);
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some((cx, cy)) = self.point_xy(si, center) {
                        cand.extend(line_circle_t(pax, pay, pbx, pby, cx, cy, r));
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (self.point_xy(si, c), self.point_xy(si, ma), self.point_xy(si, mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        cand.extend(line_ellipse_roots(pax, pay, pbx, pby, ex, ey, ux, uy, major, minor));
                    }
                }
                EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                    if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (self.point_xy(si, center), self.point_xy(si, aa), self.point_xy(si, bb)) {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        let a0 = (sy - cy).atan2(sx - cx);
                        let a1 = (ey - cy).atan2(ex - cx);
                        for t in line_circle_t(pax, pay, pbx, pby, cx, cy, r) {
                            let (ix, iy) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
                            if angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                cand.push(t);
                            }
                        }
                    }
                }
            }
        }
        // Pick the nearest intersection beyond the chosen endpoint (t > 1 for b, t < 0 for a).
        let target = if extend_b {
            cand.into_iter().filter(|&t| t > 1.0 + 1e-6).min_by(|x, y| x.total_cmp(y))
        } else {
            cand.into_iter().filter(|&t| t < -1e-6).max_by(|x, y| x.total_cmp(y))
        };
        let Some(t) = target else { return false };
        let (nx, ny) = (pax + (pbx - pax) * t, pay + (pby - pay) * t);
        let pid = if extend_b { b } else { a };
        if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
            p.x = nx;
            p.y = ny;
        }
        self.regen_sketch(si);
        true
    }
    /// Extend an arc: the endpoint nearer to the click is stretched along its own circle to the nearest
    /// intersection with another entity, within the gap outside the current span of the arc. Returns whether it
    /// was extended.
    pub fn extend_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        use std::f64::consts::TAU;
        // Arc geometry; the coordinates are copied to release the borrow before calling
        // `curve_cut_angles`.
        let (center, a, b, ccw, cx, cy, ax, ay, bx, by) = {
            let Some(s) = self.sketches.get(si) else { return false };
            let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
            let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
            let EntityKind::Arc { center, a, b, ccw } = e.kind else { return false };
            let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) else { return false };
            (center, a, b, ccw, cx, cy, ax, ay, bx, by)
        };
        let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        let a0 = (ay - cy).atan2(ax - cx);
        let a1 = (by - cy).atan2(bx - cx);
        let sweep = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
        // Which endpoint is stretched: the one nearer to the click.
        let extend_b = ((clickx - bx).powi(2) + (clicky - by).powi(2)) < ((clickx - ax).powi(2) + (clicky - ay).powi(2));
        // Angles where the full circle meets the other entities, as a parameter along the direction of the
        // arc.
        let cuts = self.curve_cut_angles(si, eid, cx, cy, r);
        let to_param = |ang: f64| if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) };
        let gap: Vec<f64> = cuts.iter().map(|&ang| to_param(ang)).filter(|&pp| pp > sweep + 1e-6 && pp < TAU - 1e-6).collect();
        // Extending b takes the first cut in the gap just past endpoint b; extending a takes the last one,
        // nearer to a from the other side.
        let target = if extend_b {
            gap.into_iter().min_by(|x, y| x.total_cmp(y))
        } else {
            gap.into_iter().max_by(|x, y| x.total_cmp(y))
        };
        let Some(pp) = target else { return false };
        let new_ang = if ccw { a0 + pp } else { a0 - pp };
        let (nx, ny) = (cx + r * new_ang.cos(), cy + r * new_ang.sin());
        let pid = if extend_b { b } else { a };
        let _ = center;
        if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
            p.x = nx;
            p.y = ny;
        }
        self.regen_sketch(si);
        true
    }
    /// Break: split a segment into two at the clicked point.
    pub fn break_line(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        let tc = (((clickx - pax) * (pbx - pax) + (clicky - pay) * (pby - pay)) / dlen2).clamp(0.05, 0.95);
        let mid = self.sketch_point_at(si, pax + (pbx - pax) * tc, pay + (pby - pay) * tc, 1e-9);
        self.sketches[si].entities.retain(|e| e.id != eid);
        for (p, q) in [(a, mid), (mid, b)] {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: p, b: q }, construction: false });
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.regen_sketch(si);
        true
    }
    pub(super) fn line_ends(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        })
    }
    /// Angles, about centre (cx, cy) with radius r, of the intersections with the other non-construction
    /// entities of the sketch, excluding `eid_self`. These are the cut points of a circle or an arc.
    pub(super) fn curve_cut_angles(&self, si: usize, eid_self: Id, cx: f64, cy: f64, r: f64) -> Vec<f64> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for e in &s.entities {
            if e.id == eid_self {
                continue; // Construction geometry is a valid cut boundary for a curve.
            }
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some((ax, ay)), Some((bx, by))) = (p(a), p(b)) {
                        for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                            pts.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                        }
                    }
                }
                EntityKind::Circle { center, r: r2 } => {
                    if let Some((c2x, c2y)) = p(center) {
                        pts.extend(circle_circle_pts(cx, cy, r, c2x, c2y, r2));
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some((c2x, c2y)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) {
                        let r2 = ((ax - c2x).powi(2) + (ay - c2y).powi(2)).sqrt();
                        let (a0, a1) = ((ay - c2y).atan2(ax - c2x), (by - c2y).atan2(bx - c2x));
                        for (ix, iy) in circle_circle_pts(cx, cy, r, c2x, c2y, r2) {
                            if angle_in_arc((iy - c2y).atan2(ix - c2x), a0, a1, ccw) {
                                pts.push((ix, iy));
                            }
                        }
                    }
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (p(c), p(ma), p(mi)) {
                        let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                        pts.extend(circle_ellipse_pts(cx, cy, r, ex, ey, ux, uy, major, minor));
                    }
                }
            }
        }
        let mut angs: Vec<f64> = pts.iter().map(|(x, y)| (y - cy).atan2(x - cx)).collect();
        angs.sort_by(|a, b| a.total_cmp(b));
        angs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        angs
    }
    /// Intersections of entity `eid` with the other entities (construction included) that lie on `eid`, in
    /// world coordinates. Used for the hover preview of trim and break.
    pub fn entity_intersections(&self, si: usize, eid: Id) -> Vec<(f64, f64)> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return Vec::new() };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let mut out: Vec<(f64, f64)> = Vec::new();
        match kind {
            EntityKind::Line { a, b } => {
                let (Some((ax, ay)), Some((bx, by))) = (p(a), p(b)) else { return out };
                for o in &s.entities {
                    if o.id == eid {
                        continue;
                    }
                    match o.kind {
                        EntityKind::Line { a: c, b: d } => {
                            if let (Some((cx, cy)), Some((dx, dy))) = (p(c), p(d)) {
                                if let Some(t) = seg_seg_t(ax, ay, bx, by, cx, cy, dx, dy) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                        EntityKind::Circle { center, r } => {
                            if let Some((cx, cy)) = p(center) {
                                for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                        EntityKind::Arc { center, a: aa, b: bb, ccw } => {
                            if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (p(center), p(aa), p(bb)) {
                                let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                                let (a0, a1) = ((sy - cy).atan2(sx - cx), (ey - cy).atan2(ex - cx));
                                for t in seg_circle_t(ax, ay, bx, by, cx, cy, r) {
                                    let (ix, iy) = (ax + (bx - ax) * t, ay + (by - ay) * t);
                                    if t > 1e-6 && t < 1.0 - 1e-6 && angle_in_arc((iy - cy).atan2(ix - cx), a0, a1, ccw) {
                                        out.push((ix, iy));
                                    }
                                }
                            }
                        }
                        EntityKind::Ellipse { c, ma, mi } => {
                            if let (Some((ex, ey)), Some((mx, my)), Some((nx, ny))) = (p(c), p(ma), p(mi)) {
                                let (ux, uy, major, minor) = ellipse_axes(ex, ey, mx, my, nx, ny);
                                for t in seg_ellipse_t(ax, ay, bx, by, ex, ey, ux, uy, major, minor) {
                                    if t > 1e-6 && t < 1.0 - 1e-6 {
                                        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            EntityKind::Circle { center, r } => {
                if let Some((cx, cy)) = p(center) {
                    for ang in self.curve_cut_angles(si, eid, cx, cy, r) {
                        out.push((cx + r * ang.cos(), cy + r * ang.sin()));
                    }
                }
            }
            EntityKind::Arc { center, a, b, ccw } => {
                if let (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) = (p(center), p(a), p(b)) {
                    let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                    let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                    for ang in self.curve_cut_angles(si, eid, cx, cy, r) {
                        if angle_in_arc(ang, a0, a1, ccw) {
                            out.push((cx + r * ang.cos(), cy + r * ang.sin()));
                        }
                    }
                }
            }
            EntityKind::Ellipse { .. } => {}
        }
        out
    }
    /// Trim a circle or an arc: it is cut at the intersections, the angular span under the click is removed and
    /// the remaining spans become arcs. Lines are handled by `trim_line`. Returns whether it succeeded.
    pub fn trim_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        use std::f64::consts::TAU;
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
        let con = e.construction;
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        // Centre, radius and, for an arc, the angular range.
        let (center_id, cx, cy, r, span): (Id, f64, f64, f64, Option<(f64, f64, bool)>) = match e.kind {
            EntityKind::Circle { center, r } => match p(center) {
                Some((cx, cy)) => (center, cx, cy, r, None),
                None => return false,
            },
            EntityKind::Arc { center, a, b, ccw } => match (p(center), p(a), p(b)) {
                (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) => {
                    let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                    (center, cx, cy, r, Some(((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx), ccw)))
                }
                _ => return false,
            },
            _ => return false,
        };
        let cuts = self.curve_cut_angles(si, eid, cx, cy, r);
        if cuts.is_empty() {
            return false;
        }
        let click_ang = (clicky - cy).atan2(clickx - cx);
        // One click removes only the span under the cursor, between the two nearest cut points, and the rest
        // stays whole: a circle leaves one remaining arc and an arc leaves up to two pieces, before and after
        // the removed window. Cutting at every intersection at once shatters a circle into a pile of arcs, which
        // breaks selection and radius editing and makes a pie slice impossible to cut out. Each remaining arc is
        // a tuple of (angle0, angle1, ccw).
        let mut kept: Vec<(f64, f64, bool)> = Vec::new();
        match span {
            None => {
                // A circle: one span under the click is removed and the remainder is a single arc, running
                // around the circle past the other cuts.
                let mut a: Vec<f64> = cuts.iter().map(|x| x.rem_euclid(TAU)).collect();
                a.sort_by(|x, y| x.total_cmp(y));
                a.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                if a.len() < 2 {
                    return false;
                }
                let ca = click_ang.rem_euclid(TAU);
                let n = a.len();
                for i in 0..n {
                    let (lo, hi) = (a[i], if i + 1 < n { a[i + 1] } else { a[0] + TAU });
                    if (ca >= lo - 1e-9 && ca < hi) || (ca + TAU >= lo && ca + TAU < hi) {
                        kept.push((hi, lo + TAU, true)); // The remainder is one arc, from hi around to lo.
                        break;
                    }
                }
            }
            Some((a0, a1, ccw)) => {
                // An arc: the parameter runs along its direction over [0, sweep]; the window under the click is
                // removed and the pieces before and after it stay whole.
                let to_param = |ang: f64| if ccw { (ang - a0).rem_euclid(TAU) } else { (a0 - ang).rem_euclid(TAU) };
                let sweep = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
                let mut ps: Vec<f64> = cuts.iter().map(|&x| to_param(x)).filter(|&v| v > 1e-6 && v < sweep - 1e-6).collect();
                ps.push(0.0);
                ps.push(sweep);
                ps.sort_by(|x, y| x.total_cmp(y));
                ps.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                let cp = to_param(click_ang);
                let ang = |param: f64| if ccw { a0 + param } else { a0 - param };
                for w in ps.windows(2) {
                    if cp > w[0] + 1e-9 && cp < w[1] - 1e-9 {
                        // Remove the window [w0, w1] under the click and keep the pieces before and after it,
                        // each as a single arc.
                        if w[0] > 1e-9 {
                            kept.push((ang(0.0), ang(w[0]), ccw));
                        }
                        if w[1] < sweep - 1e-9 {
                            kept.push((ang(w[1]), ang(sweep), ccw));
                        }
                        break;
                    }
                }
            }
        }
        if kept.is_empty() {
            return false;
        }
        // Replace the original entity with the remaining arcs.
        self.sketches[si].entities.retain(|x| x.id != eid);
        for (g0, g1, accw) in kept {
            let pa = self.sketch_point_at(si, cx + r * g0.cos(), cy + r * g0.sin(), 1e-6);
            let pb = self.sketch_point_at(si, cx + r * g1.cos(), cy + r * g1.sin(), 1e-6);
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: center_id, a: pa, b: pb, ccw: accw }, construction: con });
            // The arc endpoints are anchored to the crossing entities, so a cut point does not drift along the
            // angle on the next solve.
            self.anchor_cut_point(si, pa);
            self.anchor_cut_point(si, pb);
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.regen_sketch(si);
        true
    }
    /// Break a circle or an arc at the clicked point: an arc becomes two arcs, a circle becomes two half arcs
    /// (split at the click and at the opposite point). Lines are handled by `break_line`. Returns whether it
    /// succeeded.
    pub fn break_curve(&mut self, si: usize, eid: Id, clickx: f64, clicky: f64) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return false };
        let con = e.construction;
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let (center_id, cx, cy, r, span): (Id, f64, f64, f64, Option<(f64, f64, bool)>) = match e.kind {
            EntityKind::Circle { center, r } => match p(center) {
                Some((cx, cy)) => (center, cx, cy, r, None),
                None => return false,
            },
            EntityKind::Arc { center, a, b, ccw } => match (p(center), p(a), p(b)) {
                (Some((cx, cy)), Some((ax, ay)), Some((bx, by))) => {
                    let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                    (center, cx, cy, r, Some(((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx), ccw)))
                }
                _ => return false,
            },
            _ => return false,
        };
        let ca = (clicky - cy).atan2(clickx - cx);
        let mk = |me: &mut Self, g0: f64, g1: f64, ccw: bool| {
            let pa = me.sketch_point_at(si, cx + r * g0.cos(), cy + r * g0.sin(), 1e-6);
            let pb = me.sketch_point_at(si, cx + r * g1.cos(), cy + r * g1.sin(), 1e-6);
            let id = me.alloc_id();
            me.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: center_id, a: pa, b: pb, ccw }, construction: con });
        };
        self.sketches[si].entities.retain(|x| x.id != eid);
        match span {
            None => {
                // A circle becomes two arcs: [click, click + pi] and [click + pi, click].
                let opp = ca + std::f64::consts::PI;
                mk(self, ca, opp, true);
                mk(self, opp, ca, true);
            }
            Some((a0, a1, ccw)) => {
                // An arc becomes two arcs at the clicked point.
                mk(self, a0, ca, ccw);
                mk(self, ca, a1, ccw);
            }
        }
        self.merge_close_points(si, 1e-3); // Stitch coincident cut points so the pieces stay connected.
        self.prune_orphan_sketch_points(si); // Leave no dangling points behind a trim.
        self.regen_sketch(si);
        true
    }
    pub(super) fn point_xy(&self, si: usize, id: Id) -> Option<(f64, f64)> {
        let s = self.sketches.get(si)?;
        s.points.iter().find(|p| p.id == id).map(|p| (p.x, p.y))
    }
    /// Set the radius of an arc entity, moving its endpoints to radius `rr` while preserving their angles.
    pub fn set_arc_radius(&mut self, si: usize, eid: Id, rr: f64) {
        let (center, a, b) = {
            let Some(s) = self.sketches.get(si) else { return };
            let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return };
            match e.kind {
                EntityKind::Arc { center, a, b, .. } => (center, a, b),
                _ => return,
            }
        };
        let Some((cx, cy)) = self.point_xy(si, center) else { return };
        let rr = rr.max(0.01);
        for pid in [a, b] {
            if let Some((px, py)) = self.point_xy(si, pid) {
                let ang = (py - cy).atan2(px - cx);
                if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                    p.x = cx + rr * ang.cos();
                    p.y = cy + rr * ang.sin();
                }
            }
        }
        self.regen_sketch(si);
    }
    /// The other endpoint of a segment (other than `exclude`) that uses point `pid`.
    pub(super) fn line_other_end(&self, si: usize, pid: Id, exclude: Id) -> Option<Id> {
        let s = self.sketches.get(si)?;
        s.entities.iter().find_map(|e| {
            if e.id == exclude {
                return None;
            }
            if let EntityKind::Line { a, b } = e.kind {
                if a == pid {
                    return Some(b);
                }
                if b == pid {
                    return Some(a);
                }
            }
            None
        })
    }
    /// Index of the radius dimension of an arc (a `Diameter` constraint in radius mode on the arc centre).
    pub fn fillet_radius_constraint(&self, si: usize, arc_eid: Id) -> Option<usize> {
        let s = self.sketches.get(si)?;
        let center = match s.entities.iter().find(|e| e.id == arc_eid)?.kind {
            EntityKind::Arc { center, .. } => center,
            _ => return None,
        };
        s.constraints.iter().position(|c| matches!(*c, Constraint::Diameter { c: cc, .. } if cc == center))
    }
    /// Change the radius of a fillet through its dimension, letting the solver move the geometry while keeping
    /// the tangency. Returns whether the arc has a radius dimension.
    pub fn set_fillet_radius_dim(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let Some(ci) = self.fillet_radius_constraint(si, arc_eid) else { return false };
        if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
            *d = if *diam { 2.0 * new_r.max(0.01) } else { new_r.max(0.01) };
        }
        // The tangency points and the centre are repositioned geometrically for the new radius, giving the
        // solver a consistent starting point so the geometry does not spread.
        self.set_fillet_radius(si, arc_eid, new_r);
        self.solve_sketch(si);
        true
    }
    /// Change the radius of a fillet arc: the corner is reconstructed from the two adjacent lines and the
    /// fillet is rebuilt, moving the tangency points and the centre. Returns `false` when it is not a
    /// fillet.
    pub fn set_fillet_radius(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let (center, t1, t2) = {
            let Some(s) = self.sketches.get(si) else { return false };
            let Some(e) = s.entities.iter().find(|e| e.id == arc_eid) else { return false };
            match e.kind {
                EntityKind::Arc { center, a, b, .. } => (center, a, b),
                _ => return false,
            }
        };
        let (Some(o1), Some(o2)) = (self.line_other_end(si, t1, arc_eid), self.line_other_end(si, t2, arc_eid)) else { return false };
        let (Some((o1x, o1y)), Some((t1x, t1y)), Some((o2x, o2y)), Some((t2x, t2y))) = (self.point_xy(si, o1), self.point_xy(si, t1), self.point_xy(si, o2), self.point_xy(si, t2)) else { return false };
        // The corner is the intersection of the lines o1 to t1 and o2 to t2.
        let Some((px, py)) = line_intersect_inf(o1x, o1y, t1x, t1y, o2x, o2y, t2x, t2y) else { return false };
        let l1 = ((o1x - px).powi(2) + (o1y - py).powi(2)).sqrt();
        let l2 = ((o2x - px).powi(2) + (o2y - py).powi(2)).sqrt();
        if l1 < 1e-9 || l2 < 1e-9 {
            return false;
        }
        let (ux, uy) = ((o1x - px) / l1, (o1y - py) / l1);
        let (vx, vy) = ((o2x - px) / l2, (o2y - py) / l2);
        let cosang = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let theta = cosang.acos();
        if theta < 1e-3 || theta > std::f64::consts::PI - 1e-3 {
            return false;
        }
        let half = theta / 2.0;
        let t = (new_r / half.tan()).min(l1 * 0.98).min(l2 * 0.98);
        let r = t * half.tan();
        let (nt1x, nt1y) = (px + ux * t, py + uy * t);
        let (nt2x, nt2y) = (px + vx * t, py + vy * t);
        let bl = ((ux + vx).powi(2) + (uy + vy).powi(2)).sqrt();
        if bl < 1e-9 {
            return false;
        }
        let (bisx, bisy) = ((ux + vx) / bl, (uy + vy) / bl);
        let (ncx, ncy) = (px + bisx * (r / half.sin()), py + bisy * (r / half.sin()));
        let cross = (nt1x - ncx) * (nt2y - ncy) - (nt1y - ncy) * (nt2x - ncx);
        let ccw = cross > 0.0;
        // Move the existing points; the lines follow t1 and t2.
        for (pid, x, y) in [(t1, nt1x, nt1y), (t2, nt2x, nt2y), (center, ncx, ncy)] {
            if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                p.x = x;
                p.y = y;
            }
        }
        if let Some(e) = self.sketches[si].entities.iter_mut().find(|e| e.id == arc_eid) {
            if let EntityKind::Arc { ccw: c, .. } = &mut e.kind {
                *c = ccw;
            }
        }
        // Keep the radius dimension in step with the geometry, when the fillet has a parametric one.
        if let Some(ci) = self.fillet_radius_constraint(si, arc_eid) {
            if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
                *d = if *diam { 2.0 * r.max(0.01) } else { r.max(0.01) };
            }
        }
        self.regen_sketch(si);
        true
    }
    /// Fillet the corner between two segments sharing a vertex, with radius `r`. The lines are shortened to the
    /// tangency points and an arc is inserted between them. Returns whether it succeeded.
    pub fn fillet_lines(&mut self, si: usize, e1: Id, e2: Id, r: f64) -> bool {
        let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
        let pc = if a1 == a2 || a1 == b2 {
            a1
        } else if b1 == a2 || b1 == b2 {
            b1
        } else {
            return false; // No shared vertex.
        };
        let o1 = if a1 == pc { b1 } else { a1 };
        let o2 = if a2 == pc { b2 } else { a2 };
        let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
        let la = ((ax - px).powi(2) + (ay - py).powi(2)).sqrt();
        let lb = ((bx - px).powi(2) + (by - py).powi(2)).sqrt();
        if la < 1e-9 || lb < 1e-9 {
            return false;
        }
        let (ux, uy) = ((ax - px) / la, (ay - py) / la);
        let (vx, vy) = ((bx - px) / lb, (by - py) / lb);
        let cos = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let theta = cos.acos();
        if theta < 1e-3 || theta > std::f64::consts::PI - 1e-3 {
            return false; // Collinear: there is no corner to fillet.
        }
        let half = theta / 2.0;
        let mut t = r / half.tan();
        let maxt = la.min(lb) * 0.95;
        let r = if t > maxt {
            t = maxt;
            t * half.tan()
        } else {
            r
        };
        let dd = r / half.sin();
        let (t1x, t1y) = (px + ux * t, py + uy * t);
        let (t2x, t2y) = (px + vx * t, py + vy * t);
        let bl = ((ux + vx).powi(2) + (uy + vy).powi(2)).sqrt();
        if bl < 1e-9 {
            return false;
        }
        let (bisx, bisy) = ((ux + vx) / bl, (uy + vy) / bl);
        let (cxx, cyy) = (px + bisx * dd, py + bisy * dd);
        let cross = (t1x - cxx) * (t2y - cyy) - (t1y - cyy) * (t2x - cxx);
        let ccw = cross > 0.0;
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let cen = self.sketch_point_at(si, cxx, cyy, 1e-9);
        let arc = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            for e in s.entities.iter_mut() {
                if e.id == e1 {
                    e.kind = EntityKind::Line { a: o1, b: t1 };
                } else if e.id == e2 {
                    e.kind = EntityKind::Line { a: o2, b: t2 };
                }
            }
            s.entities.push(SketchEntity { id: arc, kind: EntityKind::Arc { center: cen, a: t1, b: t2, ccw }, construction: false });
            // A parametric fillet: the arc is held by tangency to both lines plus a radius dimension (a
            // `Diameter` constraint in radius mode on the arc centre — one R dimension, with no extra linear
            // witness line).
            // The arc is a real entity now (its endpoints sit intrinsically on the circle of a radius
            // variable), so the constraints are stable: moving the walls carries the fillet along and it stays
            // tangent.
            s.constraints.push(Constraint::Tangent { a: o1, b: t1, c: cen, r });
            s.constraints.push(Constraint::Tangent { a: o2, b: t2, c: cen, r });
            let off = fillet_label_angle(&s.points, cen, t1, t2);
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false });
        }
        // Virtual corner (described in detail in `fillet_curves`): vertex `pc` is kept and the dimensions on it
        // are left alone. It becomes the sharp corner on the extensions of both shortened lines, held by
        // `PointOnLine`. An edge dimension is measured to the virtual corner and holds at any radius, while the
        // contour stays closed and selectable.
        self.keep_virtual_corner_lines(si, pc, o1, t1, o2, t2);
        self.regen_sketch(si);
        true
    }
    /// Virtual corner for a fillet or a chamfer between two lines: the vanished vertex `pc` is held on the
    /// extensions of both shortened edges (o1 to t1, o2 to t2) by `PointOnLine`, provided `pc` no longer belongs
    /// to any entity.
    ///
    /// This keeps the dimensions and constraints on the corner valid while `pc` never reaches the contour. A
    /// real vertex, still needed by a third edge, is left alone.
    pub(super) fn keep_virtual_corner_lines(&mut self, si: usize, pc: Id, o1: Id, t1: Id, o2: Id, t2: Id) {
        let pc_still_used = self.sketches.get(si).map_or(false, |s| {
            s.entities.iter().any(|e| match e.kind {
                EntityKind::Line { a, b } => a == pc || b == pc,
                EntityKind::Arc { center, a, b, .. } => center == pc || a == pc || b == pc,
                EntityKind::Circle { center, .. } => center == pc,
                EntityKind::Ellipse { c, ma, mi } => c == pc || ma == pc || mi == pc,
            })
        });
        if !pc_still_used {
            if let Some(s) = self.sketches.get_mut(si) {
                s.constraints.push(Constraint::PointOnLine { p: pc, a: o1, b: t1 });
                s.constraints.push(Constraint::PointOnLine { p: pc, a: o2, b: t2 });
            }
        }
    }
    /// Chamfer between two segments sharing a vertex, with setback `d`.
    pub fn chamfer_lines(&mut self, si: usize, e1: Id, e2: Id, d: f64) -> bool {
        let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
        let pc = if a1 == a2 || a1 == b2 {
            a1
        } else if b1 == a2 || b1 == b2 {
            b1
        } else {
            return false;
        };
        let o1 = if a1 == pc { b1 } else { a1 };
        let o2 = if a2 == pc { b2 } else { a2 };
        let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
        let la = ((ax - px).powi(2) + (ay - py).powi(2)).sqrt();
        let lb = ((bx - px).powi(2) + (by - py).powi(2)).sqrt();
        if la < 1e-9 || lb < 1e-9 {
            return false;
        }
        let d = d.min(la * 0.95).min(lb * 0.95);
        let (t1x, t1y) = (px + (ax - px) / la * d, py + (ay - py) / la * d);
        let (t2x, t2y) = (px + (bx - px) / lb * d, py + (by - py) / lb * d);
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let seg = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            for e in s.entities.iter_mut() {
                if e.id == e1 {
                    e.kind = EntityKind::Line { a: o1, b: t1 };
                } else if e.id == e2 {
                    e.kind = EntityKind::Line { a: o2, b: t2 };
                }
            }
            s.entities.push(SketchEntity { id: seg, kind: EntityKind::Line { a: t1, b: t2 }, construction: false });
        }
        // Virtual corner: the vertex is held on the extensions of both lines, so dimensions to the corner stay
        // valid and the contour stays whole.
        self.keep_virtual_corner_lines(si, pc, o1, t1, o2, t2);
        self.regen_sketch(si);
        true
    }
    /// Endpoints of an edge entity (a line or an arc), used to find the shared vertex when filleting.
    pub(super) fn edge_end_ids(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            EntityKind::Arc { a, b, .. } => Some((a, b)),
            _ => None,
        })
    }
    /// General fillet: an arc of radius `r` tangent to two adjacent edges (lines or arcs) sharing a vertex.
    ///
    /// The offset method: the fillet centre is the intersection of the offset curves of both edges at distance
    /// `r`; of the four sign combinations, the one nearest the click (inside the corner) with tangency points
    /// within the edges is taken. The edges are trimmed to the tangency points and a parametric arc is added
    /// (tangency to each edge plus a radius dimension). Works for line to line, line to arc and arc to arc.
    /// Returns whether it succeeded.
    pub fn fillet_curves(&mut self, si: usize, e1: Id, e2: Id, r: f64, nearx: f64, neary: f64) -> bool {
        if r <= 1e-9 {
            return false;
        }
        let Some((e1a, e1b)) = self.edge_end_ids(si, e1) else { return false };
        let Some((e2a, e2b)) = self.edge_end_ids(si, e2) else { return false };
        // The shared vertex.
        let pc = if e1a == e2a || e1a == e2b {
            e1a
        } else if e1b == e2a || e1b == e2b {
            e1b
        } else {
            return false;
        };
        let o1 = if e1a == pc { e1b } else { e1a };
        let o2 = if e2a == pc { e2b } else { e2a };
        let Some((pcx, pcy)) = self.point_xy(si, pc) else { return false };
        // Support of an edge at the vertex: its kind and parameters, plus the sign of the side facing the
        // interior of the corner.
        #[derive(Clone, Copy)]
        enum Sup {
            Line { px: f64, py: f64, ux: f64, uy: f64, len: f64 }, // Vertex point, unit direction towards o,
                                                                   // and length.
            Circle { cx: f64, cy: f64, rad: f64, center: Id },
        }
        let support = |me: &Self, eid: Id, other: Id| -> Option<Sup> {
            let kind = me.sketches.get(si)?.entities.iter().find(|e| e.id == eid).map(|e| e.kind)?;
            match kind {
                EntityKind::Line { .. } => {
                    let (ox, oy) = me.point_xy(si, other)?;
                    let (dx, dy) = (ox - pcx, oy - pcy);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-9 {
                        return None;
                    }
                    Some(Sup::Line { px: pcx, py: pcy, ux: dx / len, uy: dy / len, len })
                }
                EntityKind::Arc { center, .. } => {
                    let (cx, cy) = me.point_xy(si, center)?;
                    let rad = ((pcx - cx).powi(2) + (pcy - cy).powi(2)).sqrt();
                    Some(Sup::Circle { cx, cy, rad, center })
                }
                _ => None,
            }
        };
        let (Some(s1), Some(s2)) = (support(self, e1, o1), support(self, e2, o2)) else { return false };
        // Candidate centres at distance r from both supports, over the sign combinations of the offsets.
        let mut centers: Vec<(f64, f64)> = Vec::new();
        let line_off = |px: f64, py: f64, ux: f64, uy: f64, sgn: f64| (px - uy * r * sgn, py + ux * r * sgn, ux, uy); // Offset line: a point plus a direction.
        let line_line = |l1: (f64, f64, f64, f64), l2: (f64, f64, f64, f64)| -> Option<(f64, f64)> {
            let (p1x, p1y, d1x, d1y) = l1;
            let (p2x, p2y, d2x, d2y) = l2;
            let den = d1x * d2y - d1y * d2x;
            if den.abs() < 1e-12 {
                return None;
            }
            let t = ((p2x - p1x) * d2y - (p2y - p1y) * d2x) / den;
            Some((p1x + d1x * t, p1y + d1y * t))
        };
        // Line (point ox, oy plus direction dx, dy) against circle (cx, cy, rr), giving points.
        let line_circle_pts = |ox: f64, oy: f64, dx: f64, dy: f64, cx: f64, cy: f64, rr: f64| -> Vec<(f64, f64)> {
            line_circle_t(ox, oy, ox + dx, oy + dy, cx, cy, rr).into_iter().map(|t| (ox + dx * t, oy + dy * t)).collect()
        };
        for s1g in [-1.0_f64, 1.0] {
            for s2g in [-1.0_f64, 1.0] {
                match (s1, s2) {
                    (Sup::Line { px: ax, py: ay, ux: aux, uy: auy, .. }, Sup::Line { px: bx, py: by, ux: bux, uy: buy, .. }) => {
                        if let Some(c) = line_line(line_off(ax, ay, aux, auy, s1g), line_off(bx, by, bux, buy, s2g)) {
                            centers.push(c);
                        }
                    }
                    (Sup::Line { px, py, ux, uy, .. }, Sup::Circle { cx, cy, rad, .. }) => {
                        let (ox, oy, dx, dy) = line_off(px, py, ux, uy, s1g);
                        centers.extend(line_circle_pts(ox, oy, dx, dy, cx, cy, (rad + r * s2g).abs()));
                    }
                    (Sup::Circle { cx, cy, rad, .. }, Sup::Line { px, py, ux, uy, .. }) => {
                        let (ox, oy, dx, dy) = line_off(px, py, ux, uy, s2g);
                        centers.extend(line_circle_pts(ox, oy, dx, dy, cx, cy, (rad + r * s1g).abs()));
                    }
                    (Sup::Circle { cx: c1x, cy: c1y, rad: r1, .. }, Sup::Circle { cx: c2x, cy: c2y, rad: r2, .. }) => {
                        centers.extend(circle_circle_pts(c1x, c1y, (r1 + r * s1g).abs(), c2x, c2y, (r2 + r * s2g).abs()));
                    }
                }
            }
        }
        // Tangency point of a support, from centre C.
        let tangent_pt = |sup: Sup, cx: f64, cy: f64| -> Option<(f64, f64)> {
            match sup {
                Sup::Line { px, py, ux, uy, len } => {
                    let proj = (cx - px) * ux + (cy - py) * uy;
                    if proj < -1e-6 || proj > len + 1e-6 {
                        return None; // The tangency point lies outside the edge.
                    }
                    Some((px + ux * proj, py + uy * proj))
                }
                Sup::Circle { cx: ccx, cy: ccy, rad, .. } => {
                    let d = ((cx - ccx).powi(2) + (cy - ccy).powi(2)).sqrt();
                    if d < 1e-9 {
                        return None;
                    }
                    Some((ccx + (cx - ccx) / d * rad, ccy + (cy - ccy) / d * rad))
                }
            }
        };
        // Choose the centre: tangency valid on both edges, and nearest to the click.
        let mut best: Option<(f64, (f64, f64), (f64, f64), (f64, f64))> = None; // (score, C, t1, t2)
        for (cx, cy) in centers {
            let (Some(t1), Some(t2)) = (tangent_pt(s1, cx, cy), tangent_pt(s2, cx, cy)) else { continue };
            // Both tangency points have to lie on the vertex side, between `pc` and `o`; `tangent_pt` checks
            // that for lines, while for an arc the radial projection is trusted. The score is the distance from
            // the centre to the click.
            let score = (cx - nearx).powi(2) + (cy - neary).powi(2);
            if best.map_or(true, |(bs, ..)| score < bs) {
                best = Some((score, (cx, cy), t1, t2));
            }
        }
        let Some((_, (cxx, cyy), (t1x, t1y), (t2x, t2y))) = best else { return false };
        // Build it.
        let t1 = self.sketch_point_at(si, t1x, t1y, 1e-9);
        let t2 = self.sketch_point_at(si, t2x, t2y, 1e-9);
        let cen = self.sketch_point_at(si, cxx, cyy, 1e-9);
        // The short arc from t1 to t2, bulging towards the vertex.
        let a1 = (t1y - cyy).atan2(t1x - cxx);
        let a2 = (t2y - cyy).atan2(t2x - cxx);
        let sweep_ccw = (a2 - a1).rem_euclid(std::f64::consts::TAU);
        let ccw = sweep_ccw <= std::f64::consts::PI;
        let arc = self.alloc_id();
        // Trim the edges: replace the endpoint at the vertex with the tangency point.
        let replace_end = |me: &mut Self, eid: Id, newp: Id| {
            if let Some(e) = me.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)) {
                match &mut e.kind {
                    EntityKind::Line { a, b } => {
                        if *a == pc {
                            *a = newp;
                        } else if *b == pc {
                            *b = newp;
                        }
                    }
                    EntityKind::Arc { a, b, .. } => {
                        if *a == pc {
                            *a = newp;
                        } else if *b == pc {
                            *b = newp;
                        }
                    }
                    _ => {}
                }
            }
        };
        replace_end(self, e1, t1);
        replace_end(self, e2, t2);
        // Degenerate case: a radius equal to half the side collapses the trimmed edge to zero length, the
        // tangency point having met the far end left by the previous fillet. A zero-length line must not be
        // kept: its tangency constraint is degenerate (a 0/0 direction), the solver then conflicts and deflates
        // the radii of the neighbouring arcs — a 4 by 4 square at r = 2 gave about 1.33 instead of a circle. The
        // zero edge is removed, distinct points in the same place are merged with `Coincident`, and no tangency
        // is added for the removed edge.
        let mut dropped: Vec<Id> = Vec::new();
        for eid in [e1, e2] {
            let Some((a, b)) = self.edge_end_ids(si, eid) else { continue };
            let gone = if a == b {
                true
            } else {
                match (self.point_xy(si, a), self.point_xy(si, b)) {
                    (Some((ax, ay)), Some((bx, by))) => ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt() < 1e-6,
                    _ => false,
                }
            };
            if gone {
                if let Some(s) = self.sketches.get_mut(si) {
                    s.entities.retain(|e| e.id != eid);
                    if a != b {
                        s.constraints.push(Constraint::Coincident { a, b });
                    }
                }
                dropped.push(eid);
            }
        }
        // The fillet arc, plus parametric tangency constraints and a radius dimension.
        let (s1c, s2c) = (s1, s2);
        if let Some(s) = self.sketches.get_mut(si) {
            s.entities.push(SketchEntity { id: arc, kind: EntityKind::Arc { center: cen, a: t1, b: t2, ccw }, construction: false });
            // External or internal tangency between circles, chosen by which the actual centre distance is
            // nearer to.
            let ext = |cx: f64, cy: f64, rad: f64| {
                let d = ((cxx - cx).powi(2) + (cyy - cy).powi(2)).sqrt();
                ((rad + r) - d).abs() <= ((rad - r).abs() - d).abs()
            };
            if !dropped.contains(&e1) {
                match s1c {
                    Sup::Line { .. } => s.constraints.push(Constraint::Tangent { a: o1, b: t1, c: cen, r }),
                    Sup::Circle { cx, cy, rad, center } => s.constraints.push(Constraint::CircleTangent { c1: center, c2: cen, external: ext(cx, cy, rad) }),
                }
            }
            if !dropped.contains(&e2) {
                match s2c {
                    Sup::Line { .. } => s.constraints.push(Constraint::Tangent { a: o2, b: t2, c: cen, r }),
                    Sup::Circle { cx, cy, rad, center } => s.constraints.push(Constraint::CircleTangent { c1: center, c2: cen, external: ext(cx, cy, rad) }),
                }
            }
            let off = fillet_label_angle(&s.points, cen, t1, t2);
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false });
        }
        // Virtual corner: vertex `pc` is kept and the constraints and dimensions on it are left alone, so it
        // stays the sharp corner on the extensions of both shortened edges.
        //
        // Deleting the corner and moving the dimensions onto the tangency points recomputes them against the
        // shortened segment: any edit — the radius, a neighbouring dimension — then makes the geometry wander,
        // the dimensions conflict and go red, and the broken contour cannot be selected. Measuring an edge
        // dimension to the virtual corner holds at any radius, while `pc` belongs to no entity and therefore
        // never reaches a contour or a profile.
        //
        // `pc` is held against every support: a line by `PointOnLine` on its extension, an arc or circle by
        // `PointOnCircle`. When `pc` is still needed by a third edge (three or more edges met at the corner) it
        // is a real vertex already and is left alone.
        let pc_still_used = self.sketches.get(si).map_or(false, |s| {
            s.entities.iter().any(|e| match e.kind {
                EntityKind::Line { a, b } => a == pc || b == pc,
                EntityKind::Arc { center, a, b, .. } => center == pc || a == pc || b == pc,
                EntityKind::Circle { center, .. } => center == pc,
                EntityKind::Ellipse { c, ma, mi } => c == pc || ma == pc || mi == pc,
            })
        });
        if !pc_still_used {
            if let Some(s) = self.sketches.get_mut(si) {
                match s1c {
                    Sup::Line { .. } => s.constraints.push(Constraint::PointOnLine { p: pc, a: o1, b: t1 }),
                    Sup::Circle { center, .. } => s.constraints.push(Constraint::PointOnCircle { p: pc, c: center }),
                }
                match s2c {
                    Sup::Line { .. } => s.constraints.push(Constraint::PointOnLine { p: pc, a: o2, b: t2 }),
                    Sup::Circle { center, .. } => s.constraints.push(Constraint::PointOnCircle { p: pc, c: center }),
                }
            }
        }
        self.regen_sketch(si);
        true
    }
    /// Current value of a tangent (edge-to-edge) dimension: the centre distance plus m1*r1 + m2*r2. The radii
    /// come from the circle or arc with the matching centre, and are zero when the centre is an ordinary
    /// point.
    pub fn measure_edge_distance(&self, si: usize, c1: Id, m1: i8, c2: Id, m2: i8) -> f64 {
        let Some(s) = self.sketches.get(si) else { return 0.0 };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let r_of = |cid: Id| -> f64 {
            s.entities.iter().find_map(|e| match e.kind {
                EntityKind::Circle { center, r } if center == cid => Some(r),
                EntityKind::Arc { center, a, .. } if center == cid => match (p(center), p(a)) {
                    (Some((cx, cy)), Some((ax, ay))) => Some(((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                    _ => None,
                },
                _ => None,
            }).unwrap_or(0.0)
        };
        let (Some((x1, y1)), Some((x2, y2))) = (p(c1), p(c2)) else { return 0.0 };
        let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        dist + m1 as f64 * r_of(c1) + m2 as f64 * r_of(c2)
    }
    /// Edges (lines or arcs) for which point `pid` is an endpoint. Used by click-on-corner fillets and
    /// chamfers.
    pub fn vertex_edges(&self, si: usize, pid: Id) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { a, b } if a == pid || b == pid) || matches!(e.kind, EntityKind::Arc { a, b, .. } if a == pid || b == pid)).map(|e| e.id).collect()
    }
    /// Chamfer the corner at vertex `pid`, where exactly two lines meet. Returns whether it succeeded.
    pub fn chamfer_at_vertex(&mut self, si: usize, pid: Id, d: f64) -> bool {
        let edges = self.vertex_edges(si, pid);
        if edges.len() != 2 {
            return false;
        }
        // A chamfer is a straight cut between two lines.
        let both_lines = edges.iter().all(|&eid| matches!(self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).map(|e| e.kind), Some(EntityKind::Line { .. })));
        if !both_lines {
            return false;
        }
        self.chamfer_lines(si, edges[0], edges[1], d)
    }
    /// Connected shape: every entity reachable from `eid` through shared endpoints — a rectangle from one of
    /// its sides, a chain of lines and arcs. A circle or an ellipse stands alone. Used by "fillet all" followed
    /// by a click on a shape.
    pub fn connected_entities(&self, si: usize, eid: Id) -> std::collections::HashSet<Id> {
        let mut out: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let Some(s) = self.sketches.get(si) else { return out };
        let ends = |e: &SketchEntity| -> Vec<Id> {
            match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { a, b, .. } => vec![a, b],
                _ => Vec::new(),
            }
        };
        let mut queue = vec![eid];
        while let Some(cur) = queue.pop() {
            if !out.insert(cur) {
                continue;
            }
            let Some(ce) = s.entities.iter().find(|e| e.id == cur) else { continue };
            let cends = ends(ce);
            if cends.is_empty() {
                continue; // A circle or an ellipse stands alone.
            }
            for e in &s.entities {
                if !out.contains(&e.id) && ends(e).iter().any(|id| cends.contains(id)) {
                    queue.push(e.id);
                }
            }
        }
        out
    }
    /// Fillet the corners of the selected geometry: a corner is taken only when both of its edges are in
    /// `only` (`None` means the whole sketch). "Fillet all" with a shape selected fillets that shape alone.
    pub fn fillet_all_corners_of(&mut self, si: usize, r: f64, only: Option<&std::collections::HashSet<Id>>) -> usize {
        let corners: Vec<Id> = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let mut count: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
            for e in &s.entities {
                if only.is_some_and(|f| !f.contains(&e.id)) {
                    continue;
                }
                let ends = match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    EntityKind::Arc { a, b, .. } => Some((a, b)),
                    _ => None,
                };
                if let Some((a, b)) = ends {
                    *count.entry(a).or_default() += 1;
                    *count.entry(b).or_default() += 1;
                }
            }
            count.into_iter().filter(|&(_, c)| c == 2).map(|(id, _)| id).collect()
        };
        let mut done = 0;
        for pid in corners {
            // Is the vertex still intact, with two edges still meeting there?
            if self.sketches.get(si).map_or(false, |s| s.points.iter().any(|q| q.id == pid)) && self.vertex_edges(si, pid).len() == 2 && self.fillet_at_vertex(si, pid, r) {
                done += 1;
            }
        }
        done
    }
    /// Offset the selected entities: their closed loops are moved by `dist`, inwards or outwards, and added as
    /// new entities.
    pub fn offset_entities(&mut self, si: usize, eids: &[Id], dist: f64) -> usize {
        let (pts, ents) = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).copied().collect();
            (s.points.clone(), ents)
        };
        let mut count = 0;
        // A circle entity gives a concentric circle (r plus or minus dist) rather than a polygon.
        for e in &ents {
            if let EntityKind::Circle { center, r } = e.kind {
                if let Some(c) = pts.iter().find(|p| p.id == center) {
                    let nr = r + dist;
                    if nr > 0.05 {
                        self.add_circle_entity(si, c.x, c.y, nr, crate::feature::Purpose::of(e.construction));
                        count += 1;
                    }
                }
            }
        }
        // Lines and arcs are collected into bulge loops, offset with the arcs preserved, and reassembled into
        // lines and arcs.
        for loop_v in entity_bulge_loops(&pts, &ents) {
            for oloop in crate::offset::offset_bulge(&loop_v, dist) {
                let n = oloop.len();
                if n < 2 {
                    continue;
                }
                let ids: Vec<Id> = oloop.iter().map(|v| self.sketch_point_at(si, v.x, v.y, 1e-6)).collect();
                for k in 0..n {
                    let v = oloop[k];
                    let (a, b) = (ids[k], ids[(k + 1) % n]);
                    let id = self.alloc_id();
                    let kind = if v.bulge.abs() < 1e-9 {
                        EntityKind::Line { a, b }
                    } else {
                        let (cx, cy, ccw) = arc_center_from_bulge(oloop[k].x, oloop[k].y, oloop[(k + 1) % n].x, oloop[(k + 1) % n].y, v.bulge);
                        let center = self.sketch_point_at(si, cx, cy, 1e-6);
                        EntityKind::Arc { center, a, b, ccw }
                    };
                    self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
                }
                count += 1;
            }
        }
        if count > 0 {
            self.regen_sketch(si);
        }
        count
    }
    /// Value of a named driving dimension: the `Distance` constraint of the sketch is looked up by its points
    /// (a and b, in either order) and its `d` is read. `None` when the sketch or the constraint was not
    /// found.
    pub fn named_dim_value(&self, nd: &NamedDim) -> Option<f64> {
        self.dim_target_value(&nd.target)
    }

    /// Value of whatever was named: a sketch dimension or a feature parameter.
    ///
    /// For a feature parameter the value is taken the way the rebuild takes it: an expression in `feat_dims` is
    /// evaluated, otherwise the stored number from the node itself is used. Evaluation goes against the global
    /// parameters rather than the whole scope, or a driver evaluating itself would recurse forever through
    /// `param_map`.
    pub fn dim_target_value(&self, target: &crate::model::DimTarget) -> Option<f64> {
        match target {
            crate::model::DimTarget::Sketch { sketch, refs } => {
                let si = self.sketch_index(*sketch)?;
                let want: std::collections::BTreeSet<crate::model::Id> = refs.iter().copied().collect();
                self.sketches[si].constraints.iter().find_map(|c| {
                    let r: std::collections::BTreeSet<crate::model::Id> = Project::dim_refs(c)?.into_iter().collect();
                    (r == want).then(|| Project::dim_value_of(c)).flatten()
                })
            }
            crate::model::DimTarget::Feature { node, key } => {
                if let Some(e) = self.feat_dim(*node, key) {
                    if !e.trim().is_empty() {
                        let vars: std::collections::HashMap<String, f64> =
                            self.parameters.iter().filter(|p| !p.name.is_empty()).map(|p| (p.name.to_lowercase(), p.value)).collect();
                        return crate::expr::eval(e, &vars).ok();
                    }
                }
                self.timeline.iter().find(|n| n.id == *node)?.kind.dim(key)
            }
        }
    }

    /// Returns the remaining residual; zero means the constraints are satisfied.
    /// Sketch diagnostics: the residual of each constraint, in `constraints` order, so the interface can show
    /// which dimension is unsatisfied rather than one number for the whole sketch.
    pub fn sketch_residuals(&self, si: usize) -> Vec<f64> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let active: Vec<Constraint> = s.constraints.clone();
        crate::solver::residual_per_constraint(&s.points, &radii, &active)
    }
    /// Conflicting constraints of a sketch: the indices of constraints that contradict each other, so the
    /// system cannot be satisfied until one of them is removed or made a reference dimension.
    ///
    /// This is a set of disagreeing constraints rather than "the dimensions that are currently unsatisfied":
    /// geometric constraints can disagree too (a horizontal against an angle), and no single constraint in the
    /// set is at fault — any one of them may be removed.
    ///
    /// A heuristic cannot answer this. Comparing each dimension against the measured geometry and reddening it
    /// when the value misses by more than a scale-derived tolerance fails on two contradictory dimensions: the
    /// solver puts the geometry between them, so either both miss or, if the compromise lands within tolerance,
    /// neither does. The answer comes from rank analysis (`solver::conflicts`) instead: a row whose coefficients
    /// on the variables cancelled out while the residual remained is a linear combination of constraints with an
    /// inconsistent right-hand side, and the constraints entering it are the disagreeing set.
    ///
    /// What is guaranteed: the set is a resolution set. Removing any of its members resolves the conflict
    /// (asserted by the test `geometric_constraints_conflict_too`), so the advice "remove any of the red ones"
    /// is honest.
    ///
    /// What is not: the analysis is linear and local to the current geometry, so a constraint that holds this
    /// position from afar — removing it would help, but only after a global rearrangement such as rotating a
    /// segment by 90 degrees — will not appear in the set. That is a limitation of any local method; the same
    /// situation is otherwise handled by refusing to add a constraint that over-constrains the sketch, which is
    /// `add_constraint_if_independent` while drawing.
    ///
    /// The system analysed is exactly the one the solver solves: the user constraints plus the arc intrinsics
    /// (endpoints on the circle of their own radius). Without the intrinsics the analysis runs against a
    /// different system than the solution: arcs are missing some of their equations, and a conflict involving
    /// them is either not found or blamed on an innocent constraint. Only user indices are returned, an
    /// intrinsic being part of the geometry itself and impossible to delete.
    pub fn sketch_conflicts(&self, si: usize) -> Vec<usize> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let nuser = s.constraints.len();
        let mut all: Vec<Constraint> = s.constraints.clone();
        all.extend(self.entity_intrinsics(si));
        let mut out = crate::solver::conflicts(&s.points, &radii, &all);
        out.retain(|&ci| ci < nuser);
        // Unevaluable constraints go red too. The solver rejects them (otherwise they silently hold nothing),
        // but a silent rejection is no better than a silent no-op: the point still looks constrained. A
        // constraint with nothing to measure against therefore enters the diagnostics.
        let centers: std::collections::HashSet<Id> = radii.iter().map(|r| r.center).collect();
        for (ci, c) in s.constraints.iter().enumerate() {
            if let Constraint::PointOnCircle { c: cc, .. } = c {
                if !centers.contains(cc) && !out.contains(&ci) {
                    out.push(ci);
                }
            }
        }
        out
    }
    pub(super) fn solve_sketch_inner(&mut self, si: usize, drag: Option<(Id, f64, f64)>, max_iter: usize) -> f64 {
        // The radius variables and the implicit arc constraints are computed before the mutable borrow.
        let mut radii = self.entity_radii(si);
        let intrinsics = self.entity_intrinsics(si);
        let Some(s) = self.sketches.get_mut(si) else { return 0.0 };
        // Reference (driven) dimensions do not constrain the geometry and are excluded from the solver, while
        // the arc intrinsics (endpoints on the circle of radius R) are always active.
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(intrinsics);
        let resid = crate::solver::solve_full_iter(&mut s.points, &mut radii, &active, drag, max_iter);
        // The solved radii go back into the circles; an arc derives its radius from its points and stores
        // none.
        for rv in &radii {
            for e in s.entities.iter_mut() {
                if let EntityKind::Circle { center, r } = &mut e.kind {
                    if *center == rv.center {
                        *r = rv.value;
                    }
                }
            }
        }
        self.update_driven_dims(si);
        self.regen_sketch(si);
        resid
    }
    /// Radius variables of the solver: circles (which store `r`) and arcs (whose radius is the distance from
    /// the centre to endpoint `a`), keyed by centre id. This makes the radius a real variable for degrees of
    /// freedom, tangency and equal-radius constraints.
    pub(super) fn entity_radii(&self, si: usize) -> Vec<crate::solver::RadiusVar> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        let mut out = Vec::new();
        for e in &s.entities {
            match e.kind {
                EntityKind::Circle { center, r } => out.push(crate::solver::RadiusVar { center, value: r }),
                EntityKind::Arc { center, a, .. } => {
                    if let (Some(&(cx, cy)), Some(&(ax, ay))) = (pos.get(&center), pos.get(&a)) {
                        let rr = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt().max(0.001);
                        out.push(crate::solver::RadiusVar { center, value: rr });
                    }
                }
                _ => {}
            }
        }
        out
    }
    /// Implicit constraints of curve entities, neither stored nor deletable: an arc keeps both endpoints on the
    /// circle of its centre (with radius R as a variable), which makes the arc a real entity; an ellipse keeps
    /// its semi-axes perpendicular (c to ma against c to mi), which makes the ellipse a real entity with five
    /// degrees of freedom.
    pub(super) fn entity_intrinsics(&self, si: usize) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out = Vec::new();
        for e in &s.entities {
            match e.kind {
                EntityKind::Arc { center, a, b, .. } => {
                    out.push(Constraint::PointOnCircle { p: a, c: center });
                    out.push(Constraint::PointOnCircle { p: b, c: center });
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    out.push(Constraint::Perpendicular { a: c, b: ma, c, d: mi });
                }
                _ => {}
            }
        }
        out
    }
    /// Find or create a diameter or radius dimension for the circle entity with centre `c`. Returns the
    /// constraint index; `diam` selects whether it is displayed as a diameter by default.
    pub fn ensure_diameter(&mut self, si: usize, c: Id, diam: bool) -> Option<usize> {
        let s = self.sketches.get(si)?;
        if let Some(i) = s.constraints.iter().position(|x| matches!(x, Constraint::Diameter { c: cc, .. } if *cc == c)) {
            return Some(i);
        }
        // The initial value is the current radius of the circle.
        let r = s.entities.iter().find_map(|e| match e.kind {
            EntityKind::Circle { center, r } if center == c => Some(r),
            _ => None,
        })?;
        let d = if diam { 2.0 * r } else { r };
        let s = self.sketches.get_mut(si)?;
        s.constraints.push(Constraint::Diameter { c, d, off: 0.0, expr: String::new(), driven: false, diam });
        Some(s.constraints.len() - 1)
    }
    /// Find or create a driving arc-length dimension for arc entity `arc_eid`. Returns its index.
    pub fn ensure_arc_length(&mut self, si: usize, arc_eid: Id) -> Option<usize> {
        let s = self.sketches.get(si)?;
        let (c, a, b, ccw) = s.entities.iter().find(|e| e.id == arc_eid).and_then(|e| match e.kind {
            EntityKind::Arc { center, a, b, ccw } => Some((center, a, b, ccw)),
            _ => None,
        })?;
        if let Some(i) = s.constraints.iter().position(|x| matches!(x, Constraint::ArcLength { c: cc, a: ca, b: cb, .. } if *cc == c && *ca == a && *cb == b)) {
            return Some(i);
        }
        let pos = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ((cx, cy), (ax, ay), (bx, by)) = (pos(c)?, pos(a)?, pos(b)?);
        let rad = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
        let theta = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
        let len = rad * theta;
        let s = self.sketches.get_mut(si)?;
        s.constraints.push(Constraint::ArcLength { c, a, b, ccw, len, off: 0.0, expr: String::new(), driven: false });
        Some(s.constraints.len() - 1)
    }
    /// Update the values of the reference (driven) dimensions from the current geometry: they measure rather
    /// than constrain.
    pub(super) fn update_driven_dims(&mut self, si: usize) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        // Circle radii by centre, for reference diameter and radius dimensions.
        let crad: std::collections::HashMap<Id, f64> = s.entities.iter().filter_map(|e| match e.kind {
            EntityKind::Circle { center, r } => Some((center, r)),
            _ => None,
        }).collect();
        for c in &mut s.constraints {
            match c {
                Constraint::Distance { a, b, d, driven: true, axis, .. } => {
                    if let (Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(a), pos.get(b)) {
                        *d = match axis {
                            1 => (ax - bx).abs(),
                            2 => (ay - by).abs(),
                            _ => ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt(),
                        };
                    }
                }
                Constraint::Angle { a, b, c: cc, deg, driven: true, .. } => {
                    if let (Some(&(ax, ay)), Some(&(bx, by)), Some(&(cx, cy))) = (pos.get(a), pos.get(b), pos.get(cc)) {
                        let (ux, uy) = (ax - bx, ay - by);
                        let (vx, vy) = (cx - bx, cy - by);
                        *deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
                    }
                }
                Constraint::DistancePL { p, a, b, d, driven: true, .. } => {
                    if let (Some(&(px, py)), Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(p), pos.get(a), pos.get(b)) {
                        let (dx, dy) = (bx - ax, by - ay);
                        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                        *d = (dx * (py - ay) - dy * (px - ax)) / len; // Signed, so the side is preserved.
                    }
                }
                Constraint::AngleLines { a, b, c, d, deg, driven: true, .. } => {
                    if let (Some(&(ax, ay)), Some(&(bx, by)), Some(&(cx, cy)), Some(&(dx2, dy2))) = (pos.get(a), pos.get(b), pos.get(c), pos.get(d)) {
                        let (ux, uy) = (bx - ax, by - ay);
                        let (vx, vy) = (dx2 - cx, dy2 - cy);
                        *deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
                    }
                }
                Constraint::Diameter { c, d, driven: true, diam, .. } => {
                    if let Some(&r) = crad.get(c) {
                        *d = if *diam { 2.0 * r } else { r };
                    }
                }
                Constraint::ArcLength { c, a, b, ccw, len, driven: true, .. } => {
                    if let (Some(&(cx, cy)), Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(c), pos.get(a), pos.get(b)) {
                        let rad = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                        let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                        let theta = if *ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
                        *len = rad * theta;
                    }
                }
                _ => {}
            }
        }
    }
    /// Whether a sketch is typed (built from points and entities).
    pub fn is_typed_sketch(&self, si: usize) -> bool {
        self.sketches.get(si).is_some_and(|s| !s.entities.is_empty())
    }
    /// Rebuild the contours of a sketch from its entities, as a multi-loop tessellation. The contour ids are
    /// preserved where possible (see the matching below).
    pub fn regen_sketch(&mut self, si: usize) {
        let Some(s) = self.sketches.get(si) else { return };
        // Every contour of a sketch comes from entities now; the ids are reused by position.
        let entity_cids: Vec<Id> = s.contour_ids.clone();
        // Construction geometry never reaches a profile or a contour; it is drawn separately, dashed. Only
        // ordinary entities enter the contours.
        let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| !e.construction).cloned().collect();
        let (pts, splines) = (s.points.clone(), s.splines.clone());
        // Closed contours are the region faces of the planar arrangement (intersections give minimal faces), so
        // half-rings and other areas that are not whole loops become selectable. Open chains (sweep and loft
        // paths) come from the ordinary tessellation, the arrangement producing closed faces only.
        let mut pairs: Vec<(Contour, Vec<Id>)> = arrangement_regions_prov(&pts, &ents);
        pairs.extend(tessellate_sketch_multi(&pts, &ents).into_iter().filter(|c| !c.closed).map(|c| (c, Vec::new())));
        // Spline contours (cubic Hermite with tangent handles, automatic Catmull-Rom by default).
        for sp in &splines {
            if sp.construction {
                continue; // A construction spline never reaches a profile; it is drawn dashed.
            }
            let cps: Vec<Point2> = sp.points.iter().filter_map(|id| pts.iter().find(|p| p.id == *id).map(|p| Point2::new(p.x, p.y))).collect();
            if cps.len() >= 2 {
                pairs.push((tessellate_spline_hermite(&cps, &sp.tangents, sp.closed), Vec::new()));
            }
        }
        // Contours of parametric text (its baked glyphs); ordinary, non-construction text reaches the
        // profile.
        for t in &s.texts {
            if t.construction {
                continue;
            }
            for loop_ in &t.glyphs {
                if loop_.len() >= 3 {
                    pairs.push((Contour::closed(loop_.clone()), Vec::new()));
                }
            }
        }
        // Stable contour ids: the new loops are matched against the old ones by a geometric signature
        // (closedness, centroid, area) rather than by position in the list. Positional reuse breaks
        // associativity: adding or removing a loop shifts a contour id onto a different physical loop, and an
        // extrude or an operation on the selected contour attaches to the wrong one. The signature survives
        // reordering and a changed number of loops; when the loop itself was edited, the nearest one of the same
        // kind by centroid is taken, as the best available "this is the same loop" heuristic.
        let (new_contours, new_prov): (Vec<Contour>, Vec<Vec<Id>>) = pairs.into_iter().unzip();
        let sig = |pts: &[Point2]| -> (Point2, f64) {
            let n = pts.len().max(1) as f64;
            let (sx, sy) = pts.iter().fold((0.0, 0.0), |(ax, ay), p| (ax + p.x, ay + p.y));
            let mut area = 0.0;
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                area += a.x * b.y - b.x * a.y;
            }
            (Point2::new(sx / n, sy / n), (0.5 * area).abs())
        };
        let old: Vec<(Id, Point2, f64, bool)> = entity_cids
            .iter()
            .filter_map(|&cid| {
                let ci = self.contour_index(cid)?;
                let c = &self.contours[ci];
                let (ctr, ar) = sig(&c.points);
                Some((cid, ctr, ar, c.closed))
            })
            .collect();
        let n_new = new_contours.len();
        let new_sig: Vec<(Point2, f64, bool)> = new_contours.iter().map(|c| { let (ctr, ar) = sig(&c.points); (ctr, ar, c.closed) }).collect();
        let mut assign: Vec<Option<Id>> = vec![None; n_new];
        let mut used_old = vec![false; old.len()];
        // Phase one: match by provenance, that is, by the set of boundary entities. A loop is its entities
        // wherever it ends up after an edit, so loops that swapped places do not swap ids and a feature follows
        // its own geometry. Several regions sharing one set (a circle cut by a line) are separated within the
        // group by geometric proximity.
        {
            let old_prov: Vec<Vec<Id>> = old.iter().map(|o| self.contours.ents_of(o.0).cloned().unwrap_or_default()).collect();
            let mut groups: std::collections::HashMap<&Vec<Id>, (Vec<usize>, Vec<usize>)> = std::collections::HashMap::new();
            for (ni, pv) in new_prov.iter().enumerate() {
                if !pv.is_empty() {
                    groups.entry(pv).or_default().0.push(ni);
                }
            }
            for (oi, pv) in old_prov.iter().enumerate() {
                if !pv.is_empty() {
                    if let Some(g) = groups.get_mut(pv) {
                        g.1.push(oi);
                    }
                }
            }
            for (nis, ois) in groups.into_values() {
                let mut inner: Vec<(f64, usize, usize)> = Vec::new();
                for &ni in &nis {
                    for &oi in &ois {
                        if old[oi].3 == new_sig[ni].2 {
                            inner.push((new_sig[ni].0.dist(old[oi].1) + (new_sig[ni].1 - old[oi].2).abs().sqrt(), ni, oi));
                        }
                    }
                }
                inner.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                for (_, ni, oi) in inner {
                    if assign[ni].is_none() && !used_old[oi] {
                        assign[ni] = Some(old[oi].0);
                        used_old[oi] = true;
                    }
                }
            }
        }
        // Phase two: the rest are matched greedily by geometry (centroid and area), best matches first, or the
        // first loop would take an id belonging to another.
        let mut cands: Vec<(usize, usize, f64)> = Vec::new();
        for (ni, (ctr, ar, cl)) in new_sig.iter().enumerate() {
            for (oi, o) in old.iter().enumerate() {
                if o.3 == *cl {
                    cands.push((ni, oi, ctr.dist(o.1) + (ar - o.2).abs().sqrt()));
                }
            }
        }
        cands.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        for (ni, oi, _) in cands {
            if assign[ni].is_none() && !used_old[oi] {
                assign[ni] = Some(old[oi].0);
                used_old[oi] = true;
            }
        }
        let mut new_entity_cids: Vec<Id> = Vec::with_capacity(n_new);
        for (ni, mut c) in new_contours.into_iter().enumerate() {
            c.canonicalize(); // The exact edges are the single source; the polyline and the traversal come from
                              // them.
            match assign[ni] {
                Some(cid) => {
                    if let Some(ci) = self.contour_index(cid) {
                        if let Some(slot) = self.contours.get_mut(ci) {
                            *slot = c;
                        }
                    }
                    new_entity_cids.push(cid);
                }
                None => new_entity_cids.push(self.add_contour(c)),
            }
        }
        self.rebuild_contour_nesting(&new_entity_cids);
        // Old contours not reused by any new loop are deleted.
        for (k, o) in old.iter().enumerate() {
            if !used_old[k] {
                if let Some(ci) = self.contour_index(o.0) {
                    self.contours.remove_at(ci); // The id, the provenance and the nesting go with it.
                }
            }
        }
        // Provenance is persisted: contour id to the entities of its boundary, for matching on the next
        // edit.
        for (ni, cid) in new_entity_cids.iter().enumerate() {
            if new_prov[ni].is_empty() {
                self.contours.clear_ents(*cid);
            } else {
                self.contours.set_ents(*cid, new_prov[ni].clone());
            }
        }
        self.sketches[si].contour_ids = new_entity_cids;
    }
    /// The origin point (0,0, fixed) of a sketch, created lazily. Returns its id. It belongs to no entity and
    /// therefore never reaches a profile or a contour.
    pub fn ensure_origin(&mut self, si: usize) -> Id {
        if let Some(s) = self.sketches.get(si) {
            if s.origin != 0 && s.points.iter().any(|p| p.id == s.origin) {
                return s.origin;
            }
        } else {
            return 0;
        }
        let id = self.alloc_id();
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id, x: 0.0, y: 0.0 });
        s.constraints.push(Constraint::Fixed { p: id });
        s.origin = id;
        id
    }
    /// The two points defining the infinite line of a coordinate axis (`which` is 0 for X, 1 for Y): the origin
    /// (0,0) and a fixed guide point at (1,0) or (0,1). Created lazily, for constraints against the axes.
    pub fn ensure_axis(&mut self, si: usize, which: usize) -> (Id, Id) {
        let o = self.ensure_origin(si);
        let w = which.min(1);
        let existing = self.sketches.get(si).map(|s| s.axis_pts[w]).unwrap_or(0);
        if existing != 0 && self.sketches.get(si).map_or(false, |s| s.points.iter().any(|p| p.id == existing)) {
            return (o, existing);
        }
        let id = self.alloc_id();
        let (x, y) = if w == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
        let s = &mut self.sketches[si];
        s.points.push(SketchPoint { id, x, y });
        s.constraints.push(Constraint::Fixed { p: id });
        s.axis_pts[w] = id;
        (o, id)
    }
    /// Stitch nearby sketch points (closer than `tol`): duplicates are merged into one and every entity,
    /// constraint and spline is re-pointed at it.
    ///
    /// This cures a corner that falls apart — line endpoints that never merged into a shared point, so a
    /// dimension or a constraint tears the shape open. Returns how many points were merged.
    pub fn merge_close_points(&mut self, si: usize, tol: f64) -> usize {
        let Some(s) = self.sketches.get_mut(si) else { return 0 };
        // Is the point the centre of a circle or an arc, and therefore the carrier of a solver radius variable?
        // Two such centres must not be stitched into one node, or their radii collapse and concentric circles
        // drawn from one coordinate break. The check runs against the current state of the entities, so it
        // accounts for the remappings already done in this loop.
        let is_center = |ents: &[SketchEntity], pid: Id| ents.iter().any(|e| matches!(e.kind, EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } if center == pid));
        // System points (the origin and the axis endpoints) must not be merged: they lie between 1 and sqrt(2)
        // apart while the tolerance of the line tool reaches 2.0, so merging would consume the axes and tear the
        // dimensions measured to them.
        let sys: Vec<Id> = s.system_ids();
        let mut merged = 0usize;
        // First, non-system points coinciding with the origin are glued onto it, keeping its id so the
        // reference frame stays intact. The ordinary stitching below leaves system points alone, so a profile
        // corner at the origin falls apart into two nodes in one position, the contour does not close and the
        // shape cannot be extruded. Only the origin is glued to; the axis endpoints are arbitrary.
        if let Some((oid, ox, oy)) = sys.iter().filter_map(|&sid| s.points.iter().find(|p| p.id == sid).map(|p| (sid, p.x, p.y))).find(|&(_, x, y)| x.abs() < 1e-9 && y.abs() < 1e-9) {
            let mut j = 0;
            while j < s.points.len() {
                let (jid, jx, jy) = (s.points[j].id, s.points[j].x, s.points[j].y);
                if sys.contains(&jid) || is_center(&s.entities, jid) {
                    j += 1; // System points and radius-curve centres are not glued.
                    continue;
                }
                if (ox - jx).powi(2) + (oy - jy).powi(2) < tol * tol {
                    remap_point_id(s, jid, oid); // A non-system point moves onto the origin.
                    s.points.remove(j);
                    merged += 1;
                } else {
                    j += 1;
                }
            }
        }
        let mut i = 0;
        while i < s.points.len() {
            let (ix, iy, keep) = (s.points[i].x, s.points[i].y, s.points[i].id);
            let mut j = i + 1;
            while j < s.points.len() {
                let dup = s.points[j].id;
                let d2 = (ix - s.points[j].x).powi(2) + (iy - s.points[j].y).powi(2);
                // Not merged: two radius-curve centres (concentricity is a constraint, not a shared node) and
                // any system point (the reference frame of the origin and the axes is left alone).
                if dup != keep && d2 < tol * tol && !(is_center(&s.entities, keep) && is_center(&s.entities, dup)) && !sys.contains(&keep) && !sys.contains(&dup) {
                    remap_point_id(s, dup, keep);
                    s.points.remove(j);
                    merged += 1;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if merged > 0 {
            // Drop degenerate zero-length lines and the constraints they orphan.
            s.entities.retain(|e| !matches!(e.kind, EntityKind::Line { a, b } if a == b));
            self.regen_sketch(si);
        }
        merged
    }
    /// Find a sketch point near (x, y) within `eps`, or create one. Returns its id.
    pub fn sketch_point_at(&mut self, si: usize, x: f64, y: f64, eps: f64) -> Id {
        if let Some(p) = self.sketches[si].points.iter().find(|p| ((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt() <= eps) {
            return p.id;
        }
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x, y });
        id
    }
    /// Add a segment entity; its endpoints are deduplicated against the existing points.
    pub fn add_line_entity(&mut self, si: usize, ax: f64, ay: f64, bx: f64, by: f64, purpose: crate::feature::Purpose) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let a = self.sketch_point_at(si, ax, ay, 1e-6);
        let b = self.sketch_point_at(si, bx, by, 1e-6);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a, b }, construction });
        self.regen_sketch(si);
        id
    }
    /// Whether point `pid` is already used as the centre of a circle or an arc, and so carries a solver radius
    /// variable.
    ///
    /// A radius variable is keyed by centre id (see `solver::RadiusVar`), so two curves cannot share one centre
    /// node without their radii collapsing. Concentricity is a constraint, not a shared node.
    pub(super) fn is_radius_center(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| {
            s.entities.iter().any(|e| matches!(e.kind, EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } if center == pid))
        })
    }
    /// Centre node of a radius curve (circle, arc, polygon, slot) at (x, y).
    ///
    /// Ordinary deduplication, except that landing on another curve's centre (drawing concentrically from one
    /// point) allocates a separate node: the solver radius variable is keyed by centre, and a shared node would
    /// collapse the radii. Concentricity is expressed as a constraint instead.
    pub(super) fn radius_center_at(&mut self, si: usize, x: f64, y: f64) -> Id {
        let c = self.sketch_point_at(si, x, y, 1e-6);
        if self.is_radius_center(si, c) {
            let id = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id, x, y });
            id
        } else {
            c
        }
    }
    /// Add a circle entity (a centre point plus a radius). Returns the entity id.
    pub fn add_circle_entity(&mut self, si: usize, cx: f64, cy: f64, r: f64, purpose: crate::feature::Purpose) -> Id {
        let construction = purpose == crate::feature::Purpose::Construction;
        let c = self.radius_center_at(si, cx, cy);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Circle { center: c, r: r.max(0.01) }, construction });
        self.regen_sketch(si);
        id
    }
    /// Add a rectangle as four segment entities, from two opposite corners. Returns the entity ids.
    pub fn add_rect_entity(&mut self, si: usize, x0: f64, y0: f64, x1: f64, y1: f64, purpose: crate::feature::Purpose) -> Vec<Id> {
        let construction = purpose == crate::feature::Purpose::Construction;
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        let corners = [(xa, ya), (xb, ya), (xb, yb), (xa, yb)];
        let pids: Vec<Id> = corners.iter().map(|&(x, y)| self.sketch_point_at(si, x, y, 1e-6)).collect();
        let mut eids = Vec::with_capacity(4);
        for k in 0..4 {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % 4] }, construction });
            eids.push(id);
        }
        // Automatic rectangle constraints: the bottom and top sides horizontal, the sides vertical. Only the
        // independent ones are added, so the sketch is not over-constrained, and none for construction
        // geometry.
        if !construction {
            self.add_constraint_if_independent(si, Constraint::Horizontal { a: pids[0], b: pids[1] });
            self.add_constraint_if_independent(si, Constraint::Horizontal { a: pids[3], b: pids[2] });
            self.add_constraint_if_independent(si, Constraint::Vertical { a: pids[1], b: pids[2] });
            self.add_constraint_if_independent(si, Constraint::Vertical { a: pids[0], b: pids[3] });
        }
        self.regen_sketch(si);
        eids
    }
    /// Like `add_polygon_entity`, but returns the id of the circumscribed circle centre together with the side
    /// ids.
    pub fn add_polygon_param(&mut self, si: usize, cx: f64, cy: f64, vx: f64, vy: f64, n: u32, purpose: crate::feature::Purpose) -> (Id, Vec<Id>) {
        let construction = purpose == crate::feature::Purpose::Construction;
        let n = n.max(3) as usize;
        let r = ((vx - cx).powi(2) + (vy - cy).powi(2)).sqrt().max(0.01);
        let a0 = (vy - cy).atan2(vx - cx);
        let center = self.radius_center_at(si, cx, cy); // The circumscribed circle of a polygon gets its own
                                                        // centre node.
        let pids: Vec<Id> = (0..n)
            .map(|k| {
                let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
                self.sketch_point_at(si, cx + r * a.cos(), cy + r * a.sin(), 1e-6)
            })
            .collect();
        // The construction circumscribed circle is the parametric rim of the polygon.
        let circ = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id: circ, kind: EntityKind::Circle { center, r }, construction: true });
        let mut eids = Vec::with_capacity(n);
        for k in 0..n {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: pids[k], b: pids[(k + 1) % n] }, construction });
            eids.push(id);
        }
        // Parametric shape: the vertices sit on the circumscribed circle and every side is equal, which makes
        // the polygon regular, while a driving radius dimension holds its size. What stays free is the position
        // of the centre and the rotation.
        let s = &mut self.sketches[si];
        for &p in &pids {
            s.constraints.push(Constraint::PointOnCircle { p, c: center });
        }
        for k in 1..n {
            s.constraints.push(Constraint::Equal { a: pids[0], b: pids[1], c: pids[k], d: pids[(k + 1) % n] });
        }
        // The radius witness line goes into the gap between two vertices rather than onto a vertex.
        //
        // The `off` field of a `Diameter` constraint is the on-screen angle of the witness line, and zero means
        // to the right. The first vertex of a polygon lies where it was dragged, so drawing left to right in the
        // usual way puts the "R20" label exactly on that vertex and the value becomes unreadable. Half a step
        // around the circle moves it into the gap and keeps it there for any number of sides and any rotation.
        // The sign is in screen space, where the canvas Y axis points down.
        s.constraints.push(Constraint::Diameter { c: center, d: r, off: -a0 + std::f64::consts::PI / n as f64, expr: String::new(), driven: false, diam: false });
        self.regen_sketch(si);
        (center, eids)
    }
    /// Add a slot between two centres with radius `r`: two lines plus two end arcs.
    pub fn add_slot_entity(&mut self, si: usize, c1x: f64, c1y: f64, c2x: f64, c2y: f64, r: f64, purpose: crate::feature::Purpose) {
        let construction = purpose == crate::feature::Purpose::Construction;
        let r = r.max(0.01);
        let (dx, dy) = (c2x - c1x, c2y - c1y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return;
        }
        let (px, py) = (-dy / len, dx / len); // Perpendicular.
        let c1 = self.radius_center_at(si, c1x, c1y); // The slot ends get their own centre nodes (see
                                                      // `radius_center_at`).
        let c2 = self.radius_center_at(si, c2x, c2y);
        let i1 = self.sketch_point_at(si, c1x + px * r, c1y + py * r, 1e-6);
        let i2 = self.sketch_point_at(si, c2x + px * r, c2y + py * r, 1e-6);
        let i3 = self.sketch_point_at(si, c2x - px * r, c2y - py * r, 1e-6);
        let i4 = self.sketch_point_at(si, c1x - px * r, c1y - py * r, 1e-6);
        let mut add = |kind| {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind, construction });
        };
        add(EntityKind::Line { a: i1, b: i2 });
        add(EntityKind::Arc { center: c2, a: i3, b: i2, ccw: true });
        add(EntityKind::Line { a: i3, b: i4 });
        add(EntityKind::Arc { center: c1, a: i1, b: i4, ccw: true });
        // A parametric slot, or dragging and dimension edits spread its corners and arcs apart. The arc
        // endpoints already sit on the radius circles of their centres (the arc intrinsics). Added on top:
        // equal end radii plus tangency of both side lines to both ends (i1 to i4 are the tangency points).
        // That keeps the sides parallel to the centreline and tangentially smooth, so the slot holds its shape
        // under the solver.
        let cs = &mut self.sketches[si].constraints;
        cs.push(Constraint::EqualRadius { c1, c2 });
        cs.push(Constraint::Tangent { a: i1, b: i2, c: c1, r });
        cs.push(Constraint::Tangent { a: i1, b: i2, c: c2, r });
        cs.push(Constraint::Tangent { a: i4, b: i3, c: c1, r });
        cs.push(Constraint::Tangent { a: i4, b: i3, c: c2, r });
        self.regen_sketch(si);
    }
    /// A rotated rectangle from three points: p1 to p2 gives one side and its direction, p3 gives the height as
    /// a projection onto the normal. Returns the ids of the four sides. Opposite sides are held parallel and
    /// adjacent ones perpendicular, so it stays a rectangle under the solver.
    pub fn add_rect3_entity(&mut self, si: usize, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, purpose: crate::feature::Purpose) -> Vec<Id> {
        let construction = purpose == crate::feature::Purpose::Construction;
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return Vec::new();
        }
        let (nx, ny) = (-dy / len, dx / len); // Unit normal to the side.
        let h = (x3 - x2) * nx + (y3 - y2) * ny; // Signed height: the projection of p3 onto the normal.
        let p1 = self.sketch_point_at(si, x1, y1, 1e-6);
        let p2 = self.sketch_point_at(si, x2, y2, 1e-6);
        let p3 = self.sketch_point_at(si, x2 + nx * h, y2 + ny * h, 1e-6);
        let p4 = self.sketch_point_at(si, x1 + nx * h, y1 + ny * h, 1e-6);
        let segs = [(p1, p2), (p2, p3), (p3, p4), (p4, p1)];
        let mut eids = Vec::with_capacity(4);
        for (a, b) in segs {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a, b }, construction });
            eids.push(id);
        }
        if !construction {
            self.add_constraint_if_independent(si, Constraint::Parallel { a: p1, b: p2, c: p4, d: p3 });
            self.add_constraint_if_independent(si, Constraint::Parallel { a: p1, b: p4, c: p2, d: p3 });
            self.add_constraint_if_independent(si, Constraint::Perpendicular { a: p1, b: p2, c: p1, d: p4 });
        }
        self.regen_sketch(si);
        eids
    }
    /// Add an arc entity (centre, start, end, direction).
    pub fn add_arc_entity(&mut self, si: usize, cx: f64, cy: f64, ax: f64, ay: f64, bx: f64, by: f64, winding: crate::feature::Winding, purpose: crate::feature::Purpose) {
        let construction = purpose == crate::feature::Purpose::Construction;
        let ccw = winding == crate::feature::Winding::Ccw;
        let center = self.radius_center_at(si, cx, cy); // Its own centre node (see `radius_center_at`).
        let a = self.sketch_point_at(si, ax, ay, 1e-6);
        let b = self.sketch_point_at(si, bx, by, 1e-6);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center, a, b, ccw }, construction });
        self.regen_sketch(si);
    }
    /// Create a sketch from a set of contours (a DXF or SVG import) and return its id.
    pub fn add_sketch(&mut self, name: impl Into<String>, contours: Vec<Contour>, source: Option<Id>) -> Id {
        let contour_ids: Vec<Id> = contours.into_iter().map(|c| self.add_contour(c)).collect();
        let id = self.alloc_id();
        self.sketches.push(Sketch { id, name: name.into(), contour_ids, source, points: Vec::new(), entities: Vec::new(), closed: false, constraints: Vec::new(), splines: Vec::new(), notes: Vec::new(), texts: Vec::new(), patterns: Vec::new(), projections: Vec::new(), plane: crate::feature::SketchPlane::default(), origin: 0, axis_pts: [0, 0], origin_uv: None });
        id
    }
    /// Import a DXF or SVG as an editable sketch: the exact curves (`ProfEdge`) become typed sketcher entities,
    /// so a circle stays a circle, an arc or a fillet stays an arc and a segment stays a segment, rather than
    /// being tessellated into thousands of segments. Shared curve endpoints are stitched by point deduplication
    /// into a connected, editable chain.
    ///
    /// The sketch is placed on `plane` and gets a timeline node in the active context. Unlike `add_sketch`,
    /// which only fills the contour pool and leaves an orphan sketch invisible under component isolation, this
    /// gives the sketch an owner (`add_sketch_node`) and editable geometry that can later carry dimensions and
    /// constraints. Returns the sketch index.
    pub fn import_sketch(&mut self, name: impl Into<String>, curves: Vec<crate::geom::ProfEdge>, source: Option<Id>, plane: crate::feature::SketchPlane) -> usize {
        use crate::geom::ProfEdge;
        let si = self.new_sketch(name);
        self.sketches[si].source = source;
        self.sketches[si].plane = plane;
        // Endpoint deduplication cache: nearby endpoints of adjacent curves become one sketch point, which is
        // what makes the chain connected and closable.
        let mut cache: Vec<(f64, f64, Id)> = Vec::new();
        for e in &curves {
            match *e {
                ProfEdge::Line { a, b } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    if ai != bi {
                        let id = self.alloc_id();
                        self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Line { a: ai, b: bi }, construction: false });
                    }
                }
                ProfEdge::Arc { a, b, center, ccw } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    // The arc centre is its own point: it is not shared with the endpoints and is moved
                    // independently.
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Arc { center: ci, a: ai, b: bi, ccw }, construction: false });
                }
                ProfEdge::Circle { center, r } => {
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: EntityKind::Circle { center: ci, r }, construction: false });
                }
            }
        }
        let (sid, nm) = (self.sketches[si].id, self.sketches[si].name.clone());
        self.add_sketch_node(sid, nm); // The sketch becomes a timeline node owned by the active context.
        self.regen_sketch(si); // Rebuild the contours from the entities: the profile for extrudes and
                               // toolpaths.
        si
    }
    /// Return the id of a sketch point at (x, y), reusing a nearby one (endpoint deduplication for imports) or
    /// creating one.
    pub(super) fn import_intern_pt(&mut self, si: usize, cache: &mut Vec<(f64, f64, Id)>, x: f64, y: f64) -> Id {
        const TOL: f64 = 1e-4;
        if let Some((_, _, id)) = cache.iter().find(|(cx, cy, _)| (cx - x).abs() < TOL && (cy - y).abs() < TOL) {
            return *id;
        }
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x, y });
        cache.push((x, y, id));
        id
    }
    pub fn sketch_index(&self, id: Id) -> Option<usize> {
        self.sketches.iter().position(|s| s.id == id)
    }
    /// The sketch a contour belongs to, by contour id.
    pub fn sketch_of_contour(&self, cid: Id) -> Option<Id> {
        self.sketches.iter().find(|s| s.contour_ids.contains(&cid)).map(|s| s.id)
    }
    /// Ids of contours that belong to no sketch (drawn or free-standing).
    pub fn loose_contour_ids(&self) -> Vec<Id> {
        self.contours.ids().iter().copied().filter(|cid| self.sketch_of_contour(*cid).is_none()).collect()
    }
    pub fn contour_id(&self, index: usize) -> Option<Id> {
        self.contours.id_at(index)
    }
    pub fn contour_index(&self, id: Id) -> Option<usize> {
        self.contours.index_of(id)
    }
    /// Frame of the plane sketch `si` is placed on: a world plane, a datum or a face, resolved into an
    /// orthonormal frame.
    pub fn sketch_frame(&self, si: usize) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{PlaneFrame, SketchPlane};
        // Shift of the origin within the plane (snapped to an edge or a vertex). It is applied to the base
        // frame, in its own axes, before any outer transform: u*X + v*Y is invariant to the placement of the
        // body.
        let uv = self.sketches.get(si)?.origin_uv;
        let shift = |mut f: PlaneFrame| -> PlaneFrame {
            if let Some(uv) = uv {
                let o = f.lift(uv);
                f.origin = [o.x, o.y, o.z];
            }
            f
        };
        match self.sketches.get(si)?.plane {
            SketchPlane::World(b) => Some(shift(b.frame())),
            // The sketch origin is the projection of the world origin onto the plane, so the sketch XY axes
            // coincide with the world ones and what is drawn in 2D lands in the same place in 3D.
            SketchPlane::Datum(id) => self.planes.iter().find(|p| p.id == id).map(|p| shift(PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg))),
            // A face is resolved by persistent id against the faces of the current rebuild of the body, so the
            // sketch keeps holding the same face and travels with it; the centroid and normal fingerprint is the
            // fallback match.
            SketchPlane::Face(body, key) => {
                let sid = self.sketches.get(si)?.id;
                // The face resolves in the local space of the source body (`regen_faces` is local), and the
                // frame is built in that space: the 2D zero is the projection of the source origin onto the face
                // plane, as for a world plane or a datum, and the axes are stable, with no mirroring.
                let (c, n) = self.resolve_face(body, &key);
                let frame = shift(PlaneFrame::world_aligned(c, n, 0.0));
                // Isolation: a sketch on a face of another component's body is allowed only with an explicit
                // external reference. The frame is then carried whole into the local space of the consumer
                // through `relative_transform`, so the sketch is anchored to the source face and follows it
                // along every axis rather than only along the normal, which is all the projection of the
                // consumer origin would give. Without the reference there is no frame, and regenerate blocks the
                // node.
                if let (Some(so), Some(bo)) = (self.sketch_owner(sid), self.body_owner(body)) {
                    if so != bo {
                        if !self.external_authorized(so, body) {
                            return None;
                        }
                        return Some(frame.transformed(&self.relative_transform(bo, so)));
                    }
                }
                Some(frame)
            }
        }
    }
    /// Isolated points of sketch `sid`: those that are neither endpoints of entities or splines nor system
    /// points (the origin and the axes). These are the marks placed for drilling. Returns their world
    /// coordinates, through the sketch frame, in sketch point order.
    pub fn sketch_isolated_points(&self, sid: Id) -> Vec<[f64; 3]> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let Some(frame) = self.sketch_frame(si) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        // Endpoints of the entities.
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => { used.insert(a); used.insert(b); }
                EntityKind::Arc { center, a, b, .. } => { used.insert(center); used.insert(a); used.insert(b); }
                EntityKind::Circle { center, .. } => { used.insert(center); }
                EntityKind::Ellipse { c, ma, mi } => { used.insert(c); used.insert(ma); used.insert(mi); }
            }
        }
        // Spline nodes.
        for sp in &s.splines {
            for &p in &sp.points { used.insert(p); }
        }
        s.points
            .iter()
            .filter(|p| !used.contains(&p.id))
            .map(|p| { let w = frame.lift(Point2 { x: p.x, y: p.y }); [w.x, w.y, w.z] })
            .collect()
    }
    /// Ids of the same isolated points, in the same order as `sketch_isolated_points`. The hole walls are named
    /// after them, so adding a point to the sketch must not rename the existing holes.
    pub fn sketch_hole_point_ids(&self, sid: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Arc { center, a, b, .. } => {
                    used.insert(center);
                    used.insert(a);
                    used.insert(b);
                }
                EntityKind::Circle { center, .. } => {
                    used.insert(center);
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    used.insert(c);
                    used.insert(ma);
                    used.insert(mi);
                }
            }
        }
        for sp in &s.splines {
            for &p in &sp.points {
                used.insert(p);
            }
        }
        s.points.iter().filter(|p| !used.contains(&p.id)).map(|p| p.id).collect()
    }
    /// Profile of sketch `si` for an extrude or a revolve: its first closed contour, flattened to XY.
    pub fn sketch_profile_xy(&self, si: usize) -> Option<(Id, Vec<f64>)> {
        let s = self.sketches.get(si)?;
        for &cid in &s.contour_ids {
            if let Some(ci) = self.contour_index(cid) {
                let c = &self.contours[ci];
                if c.closed && c.points.len() >= 3 {
                    let xy: Vec<f64> = c.points.iter().flat_map(|p| [p.x, p.y]).collect();
                    return Some((cid, xy));
                }
            }
        }
        None
    }
    /// XY of a specific closed contour by id, used to choose which shape an extrude, cut or revolve takes.
    pub fn contour_profile_xy(&self, cid: Id) -> Option<Vec<f64>> {
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.closed && c.points.len() >= 3 {
            Some(c.points.iter().flat_map(|p| [p.x, p.y]).collect())
        } else {
            None
        }
    }
    /// Planar regions of a sketch: the minimal closed faces of the arrangement of every entity (intersections,
    /// then splitting, then faces). This makes areas selectable that are not whole loops — a strip between two
    /// circles cut by a line, for instance. Used to pick an extrude profile.
    pub fn sketch_regions(&self, si: usize) -> Vec<crate::geom::Contour> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        arrangement_regions(&s.points, &s.entities)
    }
    /// Like `feature_profile_encoded`, but the contours listed in `fill` are not subtracted as holes, having
    /// been explicitly selected as filled. Two concentric circles with both selected put the inner one in
    /// `fill` and give a solid cylinder rather than a tube.
    ///
    /// Note that this is one face: an outer contour plus its holes. Nesting deeper than two levels also
    /// produces islands, which only [`Project::profile_faces`] and [`Project::encode_profiles_fill`] return.
    pub fn feature_profile_encoded_fill(&self, sketch: Id, profile: Id, fill: &[Id]) -> Option<Vec<f64>> {
        let (outer_cid, hole_ids) = self.profile_faces(sketch, &[profile], fill).into_iter().next()?;
        let outer = self.contours.get(self.contour_index(outer_cid)?)?;
        let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
        Some(crate::geom::encode_profile(outer, &holes))
    }
    /// Profile of a feature with names: each side face is named "`role` of feature F from sketch entity E".
    /// `feature` is the node that builds the body, and the role depends on the operation (revolve, sweep,
    /// loft).
    pub fn feature_profile_encoded_named(&mut self, feature: Id, sketch: Id, profile: Id, fill: &[Id], role: crate::names::Role) -> Option<Vec<f64>> {
        let (outer_cid, hole_ids) = self.profile_faces(sketch, &[profile], fill).into_iter().next()?;
        let mut srcs: Vec<Id> = Vec::new();
        for cid in std::iter::once(&outer_cid).chain(hole_ids.iter()) {
            if let Some(i) = self.contour_index(*cid) {
                srcs.extend(self.contours[i].edge_src.iter().copied().filter(|s| *s != 0));
            }
        }
        srcs.sort_unstable();
        srcs.dedup();
        let map: std::collections::HashMap<Id, u32> = srcs.into_iter().map(|src| (src, self.intern_name(feature, role, src))).collect();
        let outer = self.contours.get(self.contour_index(outer_cid)?)?;
        let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
        Some(crate::geom::encode_profile_named(outer, &holes, &|src| map.get(&src).copied().unwrap_or(0)))
    }
    /// Resolve a selection into faces: `(outer contour, its holes)`, correct at any depth of nesting.
    ///
    /// The model is simple and predictable: every selected contour is its own region, holding the material
    /// between it and its direct children, which become its holes. The regions are then fused in the kernel, so
    /// selecting adjacent nesting levels composes by itself — selecting the outer and the middle of three
    /// nested rectangles gives `(outer, [middle])` together with `(middle, [inner])`, that is, a plate with a
    /// hole the size of the inner rectangle. This works for any number of nesting levels.
    ///
    /// Marking a nested selected contour as filled on its parent instead, without giving it a region of its
    /// own, ends the descent there and loses everything deeper: three nested rectangles extrude solid. `fill` is
    /// therefore read simply as one more selected contour, which repairs older files without editing the
    /// feature.
    ///
    /// An empty `profiles`, or a zero, means the first closed contour of the sketch.
    pub fn profile_faces(&self, sketch: Id, profiles: &[Id], fill: &[Id]) -> Vec<(Id, Vec<Id>)> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Closed contours of the sketch usable as a profile.
        let ids: Vec<Id> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| {
                self.contour_index(*cid).is_some_and(|i| self.contours[i].closed && self.contours[i].points.len() >= 3) && self.contour_profile_xy(*cid).is_some()
            })
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        // The selection is `profiles` plus `fill`; the older "filled" mark reads as one more selected
        // contour.
        let mut sel: Vec<Id> = profiles.iter().chain(fill).copied().map(|p| if p == 0 { ids[0] } else { p }).filter(|p| ids.contains(p)).collect();
        if sel.is_empty() {
            sel.push(ids[0]);
        }
        sel.sort_unstable();
        sel.dedup();
        sel.into_iter().map(|c| (c, self.feature_holes(sketch, c))).collect()
    }
    /// Encode every contour in `profiles`, each as its own tool; empty takes the first contour. Returns `None`
    /// when any contour fails to encode. The list feeds `Kernel::combine_region_multi`.
    ///
    /// Nesting of any depth also produces island faces, which enter the same list.
    ///
    /// `feature` is the node building these profiles: its id is part of the name of every side face ("wall of
    /// feature F from sketch entity E"), so two different features over one sketch do not share face names and
    /// the boolean does not confuse them.
    pub fn encode_profiles_fill(&mut self, feature: Id, sketch: Id, profiles: &[Id], fill: &[Id]) -> Option<Vec<Vec<f64>>> {
        self.encode_profiles_role(feature, sketch, profiles, fill, crate::names::Role::Wall)
    }

    /// The same, with the role of the side faces given explicitly. The role is part of a face name, and a
    /// revolve has its own (`Revolved`) as does a sweep (`Swept`): the wall of an extrude and a surface of
    /// revolution are different things, and naming them alike would collide the names of two unlike faces.
    pub fn encode_profiles_role(&mut self, feature: Id, sketch: Id, profiles: &[Id], fill: &[Id], role: crate::names::Role) -> Option<Vec<Vec<f64>>> {
        let faces = self.profile_faces(sketch, profiles, fill);
        if faces.is_empty() {
            return None;
        }
        // The wall names are prepared in advance, in one pass: interning mutates the name table and needs a
        // mutable borrow, while the encoder only needs the finished substitution table.
        let mut srcs: Vec<Id> = Vec::new();
        for (outer_cid, hole_ids) in &faces {
            for cid in std::iter::once(outer_cid).chain(hole_ids.iter()) {
                if let Some(i) = self.contour_index(*cid) {
                    srcs.extend(self.contours[i].edge_src.iter().copied().filter(|s| *s != 0));
                }
            }
        }
        srcs.sort_unstable();
        srcs.dedup();
        let wall: std::collections::HashMap<Id, u32> = srcs.into_iter().map(|src| (src, self.intern_name(feature, role, src))).collect();
        let name_of = |src: Id| -> u32 { wall.get(&src).copied().unwrap_or(0) };
        faces
            .into_iter()
            .map(|(outer_cid, hole_ids)| {
                let outer = self.contours.get(self.contour_index(outer_cid)?)?;
                let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
                Some(crate::geom::encode_profile_named(outer, &holes, &name_of))
            })
            .collect()
    }
    /// Encode the path of a sweep: one sketch contour, open or closed, as `[1.0, loop_block]` with exact lines
    /// and arcs (`build_exact_wire`). A `path` of zero takes the first contour with at least two points,
    /// preferring an open one, which is the typical path. Sketch contours always carry exact edges.
    pub fn sweep_path_encoded(&self, sketch: Id, path: Id) -> Option<Vec<f64>> {
        let has_pts = |cid: &&Id| self.contour_index(**cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let cid = if path != 0 {
            path
        } else {
            let si = self.sketch_index(sketch)?;
            let ids = &self.sketches[si].contour_ids;
            let open = |cid: &&Id| self.contour_index(**cid).map(|i| !self.contours[i].closed).unwrap_or(false) && has_pts(cid);
            *ids.iter().find(open).or_else(|| ids.iter().find(has_pts))?
        };
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.points.len() < 2 {
            return None;
        }
        let mut v = vec![1.0];
        v.extend(c.loop_block());
        Some(v)
    }
    /// Candidate profile contours for a sweep (closed, at least three points), in sketch order. The first is
    /// the automatic choice for a `profile` of zero. Used to switch contours in the interface and to preview
    /// them; it matches `feature_profile_encoded`.
    pub fn sweep_profile_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| self.contour_index(*cid).map(|i| self.contours[i].closed && self.contours[i].points.len() >= 3).unwrap_or(false) && self.contour_profile_xy(*cid).is_some())
            .collect()
    }
    /// Candidate path contours (at least two points), open ones first as the typical path, then closed ones.
    /// The first is the automatic choice for a `path` of zero, matching the order used by
    /// `sweep_path_encoded`.
    pub fn sweep_path_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        let has_pts = |cid: Id| self.contour_index(cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let is_open = |cid: Id| self.contour_index(cid).map(|i| !self.contours[i].closed).unwrap_or(false);
        let ids: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|c| has_pts(*c)).collect();
        let mut open: Vec<Id> = ids.iter().copied().filter(|c| is_open(*c)).collect();
        let closed: Vec<Id> = ids.iter().copied().filter(|c| !is_open(*c)).collect();
        open.extend(closed);
        open
    }
    /// Loft contour for section sketch `sketch`, honouring the selection `cid` (zero takes the first closed
    /// profile). Returns the id of a contour usable as a section: closed, with at least three points.
    pub fn loft_section_contour(&self, sketch: Id, cid: Id) -> Option<Id> {
        if cid != 0 {
            let ci = self.contour_index(cid)?;
            return (self.contours[ci].closed && self.contours[ci].points.len() >= 3).then_some(cid);
        }
        self.sweep_profile_contours(sketch).first().copied()
    }
    /// Like `loft_encoded`, but the edges of the first section carry the names of the future loft faces: a face
    /// between sections is named after the edge that swept it, there being no other source in the recipe.
    pub fn loft_encoded_named(&mut self, feature: Id, sketches: &[Id], contours: &[Id]) -> Option<(Vec<f64>, Vec<usize>, Vec<f64>)> {
        if sketches.len() < 2 {
            return None;
        }
        // The names are prepared in advance (interning mutates the table) and only for the first section.
        let first_cid = self.loft_section_contour(sketches[0], contours.first().copied().unwrap_or(0))?;
        let mut srcs: Vec<Id> = self.contour_index(first_cid).map(|i| self.contours[i].edge_src.iter().copied().filter(|s| *s != 0).collect()).unwrap_or_default();
        srcs.sort_unstable();
        srcs.dedup();
        let map: std::collections::HashMap<Id, u32> = srcs.into_iter().map(|src| (src, self.intern_name(feature, crate::names::Role::Lofted, src))).collect();
        self.loft_encoded_with(sketches, contours, &map)
    }
    pub(super) fn loft_encoded_with(&self, sketches: &[Id], contours: &[Id], names: &std::collections::HashMap<Id, u32>) -> Option<(Vec<f64>, Vec<usize>, Vec<f64>)> {
        if sketches.len() < 2 {
            return None;
        }
        let mut data = Vec::new();
        let mut offsets = vec![0usize];
        let mut places = Vec::new();
        for (i, &sid) in sketches.iter().enumerate() {
            let cid = self.loft_section_contour(sid, contours.get(i).copied().unwrap_or(0))?;
            let ci = self.contour_index(cid)?;
            let c = &self.contours[ci];
            if !c.closed || c.points.len() < 3 {
                return None;
            }
            if i == 0 {
                data.extend(c.loop_block_named(&|src| names.get(&src).copied().unwrap_or(0)));
            } else {
                data.extend(c.loop_block());
            }
            offsets.push(data.len());
            let pl = self.sketch_frame_by_id(sid).map(|f| f.matrix12()).unwrap_or(crate::feature::PLACE_IDENTITY);
            places.extend_from_slice(&pl);
        }
        Some((data, offsets, places))
    }
    /// Points of a contour, by id.
    pub(super) fn contour_pts(&self, cid: Id) -> Vec<Point2> {
        self.contour_index(cid).map(|ci| self.contours[ci].points.clone()).unwrap_or_default()
    }
    /// Recompute the nesting of the sketch contours and record it in `contour_parent`. Computed once when a
    /// sketch is regenerated rather than by every consumer on every build.
    ///
    /// The parent is the smallest strictly enclosing contour. "Strictly" is the key word: a contour that touches
    /// its enclosing one is not a child — it is part of the boundary rather than a hole. Measured failure
    /// without it: a pad flush against a wall was counted as a hole, the hole touched the outer loop, the face
    /// failed to build, and the whole region silently disappeared from the body.
    pub(super) fn rebuild_contour_nesting(&mut self, cids: &[Id]) {
        let data: Vec<(Id, Vec<Point2>, f64)> = cids
            .iter()
            .filter_map(|&cid| {
                let c = self.contours.get(self.contour_index(cid)?)?;
                (c.closed && c.points.len() >= 3).then(|| (cid, c.points.clone(), c.signed_area().abs()))
            })
            .collect();
        for &cid in cids {
            self.contours.clear_parent(cid);
        }
        for (cid, pts, _) in &data {
            let parent = data
                .iter()
                .filter(|(oid, opts, _)| oid != cid && poly_contains(opts, pts) && !polys_touch(opts, pts))
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(oid, _, _)| *oid);
            // A root is marked explicitly (parent 0) rather than by a missing entry: otherwise "the graph is
            // built" and "there is no nesting" are indistinguishable, and a sketch without nesting would be
            // recomputed on every build.
            self.contours.set_parent(*cid, parent.unwrap_or(0));
        }
    }
    pub fn feature_holes(&self, sketch: Id, outer: Id) -> Vec<Id> {
        if outer == 0 {
            return Vec::new();
        }
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Nesting is a stored relation (`contour_parent`), computed once when the sketch is regenerated. The
        // direct children are the contours whose parent is this one — no point-in-polygon recomputation on
        // every build and no per-consumer tolerances.
        let by_graph: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|cid| self.contours.parent_of(*cid).as_ref() == Some(&outer)).collect();
        if !by_graph.is_empty() || self.sketches[si].contour_ids.iter().any(|c| self.contours.parent_of(*c).is_some()) {
            return by_graph;
        }
        // The graph is not built yet (the sketch was not regenerated in this session): it is computed here, and
        // nothing beyond it is guessed.
        let outer_pts = self.contour_pts(outer);
        if outer_pts.len() < 3 {
            return Vec::new();
        }
        let cands: Vec<(Id, Vec<Point2>)> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| *cid != outer && self.contour_profile_xy(*cid).is_some())
            .map(|cid| (cid, self.contour_pts(cid)))
            .filter(|(_, p)| p.len() >= 3 && poly_contains(&outer_pts, p) && !polys_touch(&outer_pts, p))
            .collect();
        // A direct child of `outer` is one that lies inside no other candidate.
        cands.iter().filter(|(id, p)| !cands.iter().any(|(id2, p2)| id2 != id && poly_contains(p2, p))).map(|(id, _)| *id).collect()
    }
    /// Local revolve axis (origin and direction in sketch 2D space, z = 0) from centreline `line`. A `line` of
    /// zero, or one that is not a line or is degenerate, gives `None` and the caller falls back to a datum or the
    /// sketch X or Y axis. `dir` is normalised. The 2D coordinates of the line are already in sketch plane space,
    /// so no inverse placement is needed, unlike a datum axis given in world space.
    pub(super) fn revolve_axis_from_line(&self, sketch: Id, line: Id) -> Option<([f64; 3], [f64; 3])> {
        if line == 0 {
            return None;
        }
        let si = self.sketch_index(sketch)?;
        let (a, b) = self.line_ends(si, line)?;
        let s = &self.sketches[si];
        let pa = s.points.iter().find(|q| q.id == a).map(|q| (q.x, q.y))?;
        let pb = s.points.iter().find(|q| q.id == b).map(|q| (q.x, q.y))?;
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt();
        (len > 1e-9).then(|| ([pa.0, pa.1, 0.0], [dx / len, dy / len, 0.0]))
    }
    /// Add a work plane with a stable id (a timeline node). Returns its id.
    pub fn add_plane(&mut self, mut pl: WorkPlane) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if pl.id == 0 {
            pl.id = self.alloc_id();
        }
        let id = pl.id;
        let name = pl.name.clone();
        let parent = Some(self.active_ctx());
        self.planes.push(pl);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::Plane { plane: id }, parent, dirty: false, suppressed: false });
        id
    }
    /// Resolve a parametric datum plane: recompute its origin and normal from `def` and write them into the
    /// `WorkPlane`, so the rest of the code (the sketch frame, the renderer) reads finished values. `dist` is
    /// parametric through the `dist` feature dimension of the plane node. `Manual` is left alone.
    pub(super) fn resolve_plane_into(&mut self, plane_id: Id, vars: &std::collections::HashMap<String, f64>, dim: Option<&std::collections::HashMap<String, String>>) {
        let Some(pi) = self.planes.iter().position(|p| p.id == plane_id) else { return };
        match self.planes[pi].def {
            PlaneDef::OffsetBase { base, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let f = base.frame();
                let n = f.normal();
                self.planes[pi].normal = n;
                self.planes[pi].origin = [f.origin[0] + n[0] * dist, f.origin[1] + n[1] * dist, f.origin[2] + n[2] * dist];
            }
            PlaneDef::OffsetFace { body, face, dist } => {
                let dist = eval_dim(dim, "dist", dist, vars);
                let (c, n) = self.resolve_face(body, &face); // Persistent face resolution: centre and
                                                             // normal.
                self.planes[pi].normal = n;
                self.planes[pi].origin = [c[0] + n[0] * dist, c[1] + n[1] * dist, c[2] + n[2] * dist];
            }
            PlaneDef::OffsetPlane { plane: src, dist } => {
                // The source plane is resolved earlier in the timeline, having been created earlier, so its
                // origin and normal are already available.
                let dist = eval_dim(dim, "dist", dist, vars);
                if let Some((o, n)) = self.planes.iter().find(|p| p.id == src).map(|p| (p.origin, p.normal)) {
                    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                    let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
                    self.planes[pi].normal = n;
                    self.planes[pi].origin = [o[0] + nn[0] * dist, o[1] + nn[1] * dist, o[2] + nn[2] * dist];
                }
            }
            PlaneDef::Manual => {}
        }
    }
    /// Deep clone of sketch `sid` into a new node under component `target_parent`.
    ///
    /// The internal ids of points, entities and constraints are local and are reused as they are, which is safe
    /// since none of them reference anything outside. Only `Sketch::id` is new, and the timeline node references
    /// it. With a `remap` map given, `source` and `plane` are re-pointed at the cloned objects (for a clone made
    /// as part of a component); otherwise they are kept. Returns the id of the new sketch, which equals the id of
    /// its timeline node.
    pub(super) fn clone_sketch_impl(&mut self, sid: Id, target_parent: Id, remap: &std::collections::HashMap<Id, Id>) -> Option<Id> {
        let si = self.sketch_index(sid)?;
        let mut sk = self.sketches[si].clone();
        let new_id = self.alloc_id();
        sk.id = new_id;
        sk.contour_ids.clear(); // Regenerated afresh, with their own contour ids.
        if let Some(src) = sk.source {
            if let Some(&n) = remap.get(&src) {
                sk.source = Some(n);
            }
        }
        // Placement: a datum or a face of another body is re-pointed at the clone, when it is in the map.
        sk.plane = match sk.plane {
            crate::feature::SketchPlane::Datum(p) => crate::feature::SketchPlane::Datum(remap.get(&p).copied().unwrap_or(p)),
            crate::feature::SketchPlane::Face(b, f) => crate::feature::SketchPlane::Face(remap.get(&b).copied().unwrap_or(b), f),
            other => other,
        };
        // The anchor may still point at a face or a datum of another component (with the tree clipboard the
        // remap is empty and the source body is not carried over). Such a cross-component reference does not
        // resolve (`sketch_frame` is `None`) and the sketch would hang: no gizmo, no contour selection, nothing
        // to extrude. It is detached onto the XY plane of the target part, which preserves the flat profile in
        // local 2D and makes the sketch self-contained.
        use crate::feature::{BasePlane, SketchPlane};
        let carried = |slf: &Self, owner: Option<Id>| owner.map_or(true, |o| slf.component_is_within(o, target_parent));
        let attached = match sk.plane {
            SketchPlane::Face(body, _) => carried(self, self.body_owner(body)),
            SketchPlane::Datum(p) => carried(self, self.plane_owner(p)),
            SketchPlane::World(_) => true,
        };
        if !attached {
            sk.plane = SketchPlane::World(BasePlane::XY);
        }
        let name = sk.name.clone();
        self.sketches.push(sk);
        let ni = self.sketches.len() - 1;
        self.regen_sketch(ni);
        // The timeline node under the target component.
        use crate::feature::{FeatureKind, FeatureNode};
        self.push_timeline(FeatureNode { id: new_id, name, kind: FeatureKind::Sketch { sketch: new_id }, parent: Some(target_parent), dirty: false, suppressed: false });
        Some(new_id)
    }
    /// The body whose face a sketch is placed on, when its plane is a `Face`; otherwise `None`. Used by the
    /// isolation check: a sketch must not rest on a face of another component's body.
    pub fn sketch_plane_body(&self, sketch: Id) -> Option<Id> {
        let si = self.sketch_index(sketch)?;
        match self.sketches[si].plane {
            crate::feature::SketchPlane::Face(body, _) => Some(body),
            _ => None,
        }
    }
    /// Settle the sketches before building: a body must not be built from geometry that does not yet satisfy
    /// its own constraints.
    ///
    /// Measured case: a slot 5 wide sat collinear with a construction line dimensioned at exactly 130 from the
    /// axis, while the file held x = -129.99900 and -129.99930. The constraints were not grossly violated, yet
    /// they were not satisfied either — the endpoints of a vertical line differed by 0.3 micrometres. The cut
    /// was built against exactly that geometry and left a film a fraction of a micrometre thick: a wall where
    /// an opening was expected, and after an edit, a wall on the other side. One full solve moves the points to
    /// -129.999999908 (0.1 nanometres from the dimension) and the film disappears.
    ///
    /// An interactive drag solves the sketch on a reduced budget for responsiveness, and the interface runs a
    /// full solve on release, but catching every interface path is pointless: the invariant is cheaper to hold
    /// here. For an already solved sketch this is a single residual evaluation, the solver exiting on the first
    /// iteration, so there is no measurable cost.
    pub(super) fn settle_sketches(&mut self) {
        // Contours loaded from a file may have been written by an older build with a clockwise traversal, or
        // with a polyline that drifted apart from the exact edges. Opening a document recomputes nothing, so
        // such a contour reaches the kernel and produces a face with an inside-out hole. It is canonicalised
        // here; the operation is idempotent and changes nothing for freshly built contours.
        for c in self.contours.iter_mut() {
            c.canonicalize();
        }
        // The nesting graph is not in the file either, and waiting for a sketch edit to create it would mean
        // two behaviours for one model. It is computed for every sketch that lacks one; after that it lives with
        // the sketch regeneration.
        let need: Vec<Vec<Id>> = self
            .sketches
            .iter()
            .map(|s| s.contour_ids.clone())
            .filter(|ids| !ids.is_empty() && !ids.iter().any(|c| self.contours.parent_of(*c).is_some()))
            .collect();
        for ids in need {
            self.rebuild_contour_nesting(&ids);
        }
        // No `eval_parameters` here: the dimension values are taken as they are. Re-evaluating the expressions
        // is a separate step (editing a parameter or a sketch), and doing it from here would overwrite values
        // set directly, changing a feature height during an ordinary build.
        for si in 0..self.sketches.len() {
            let before: Vec<(Id, f64, f64)> = self.sketches[si].points.iter().map(|p| (p.id, p.x, p.y)).collect();
            self.solve_sketch_inner(si, None, 120);
            let moved = self.sketches[si]
                .points
                .iter()
                .zip(before.iter())
                .any(|(p, (id, x, y))| p.id != *id || (p.x - x).abs() > 1e-9 || (p.y - y).abs() > 1e-9);
            if moved {
                self.regen_sketch(si); // The points moved, so the contours are rebuilt; otherwise the profile
                                       // stays as it was.
            }
        }
    }
    /// Bounding box of all the geometry.
    pub fn bounds(&self) -> Option<Bbox> {
        let mut acc: Option<Bbox> = None;
        for c in self.contours.iter() {
            if let Some(b) = c.bbox() {
                acc = Some(match acc {
                    None => b,
                    Some(a) => Bbox {
                        min: Point2::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
                        max: Point2::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
                    },
                });
            }
        }
        acc
    }
    /// Contours of an operation (selected by id, or all of them when the selection is empty), with their
    /// indices.
    pub(super) fn resolve_selection(&self, op: &OperationDef) -> Vec<(usize, &Contour)> {
        if op.selection.is_empty() {
            self.contours.iter().enumerate().collect()
        } else {
            op.selection.iter().filter_map(|&id| self.contour_index(id).map(|i| (i, &self.contours[i]))).collect()
        }
    }
    /// The selected closed contours as a machining boundary; empty means the whole part.
    pub(super) fn boundary_contours(&self, op: &OperationDef) -> Vec<Contour> {
        op.selection
            .iter()
            .filter_map(|&id| self.contour_index(id).map(|i| &self.contours[i]))
            .filter(|c| c.closed && c.points.len() >= 3)
            .cloned()
            .collect()
    }
    /// Resolve the side: `Auto` decides from the nesting depth of contour `idx`.
    pub(super) fn resolve_side(&self, mode: SideMode, idx: usize) -> Side {
        match mode {
            SideMode::Outside => Side::Outside,
            SideMode::Inside => Side::Inside,
            SideMode::On => Side::On,
            SideMode::Auto => {
                if nesting_depth(&self.contours, idx) % 2 == 0 {
                    Side::Outside
                } else {
                    Side::Inside
                }
            }
        }
    }
}

/// Angle of the witness line of a fillet radius, pointing into the corner, in screen radians.
///
/// Zero means to the right, and the "R6.0" label then lands exactly on a tangency point: on a rounded
/// rectangle the tangency points sit to the left and right of the arc centre. Found on a screenshot of a sketch
/// run — the numbers show nothing, the constraints are intact and the sketch solves.
///
/// Outwards is wrong too: along the bisector on the outside stands the virtual corner, the sharp vertex a
/// fillet leaves in place for the dimensions. The label lands on it and the guard reddens all four corners
/// instead of two.
///
/// Into the corner is the only direction that is empty by construction: that is the open part of the shape. The
/// tangency points lie ninety degrees to the sides and the virtual corner lies behind. The screen Y axis points
/// down, which is why the sign of the angle is inverted.
fn fillet_label_angle(points: &[SketchPoint], cen: Id, t1: Id, t2: Id) -> f64 {
    let get = |id: Id| points.iter().find(|p| p.id == id).map(|p| (p.x, p.y));
    let (Some(c), Some(a), Some(b)) = (get(cen), get(t1), get(t2)) else { return 0.0 };
    let (dx, dy) = ((a.0 - c.0) + (b.0 - c.0), (a.1 - c.1) + (b.1 - c.1));
    if dx.hypot(dy) < 1e-9 {
        return 0.0; // The tangency points are diametrically opposite, so there is no bisector.
    }
    dy.atan2(-dx)
}
