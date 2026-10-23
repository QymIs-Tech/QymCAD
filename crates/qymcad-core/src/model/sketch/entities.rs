//! THE CURVES OF A SKETCH ADDED AND EDITED: lines, circles, rectangles, polygons, splines, ellipses, texts.

use super::*;

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
        self.sketches.push(Sketch {
            id,
            name: "name-sketch".into(),
            contour_ids: Vec::new(),
            source: None,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            figures: Vec::new(),
            projections: Vec::new(),
            offsets: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
        self.sketches.len() - 1
    }
    /// Add an ellipse as a real entity: a centre plus the endpoints of the major and minor semi-axes.
    ///
    /// `rot` is the rotation of the major axis in radians. Returns the id of the centre, which serves as the
    /// handle. The semi-axes are kept perpendicular by an implicit constraint, so the ellipse is parametric
    /// with five degrees of freedom rather than a polygon.
    pub fn add_ellipse_entity(&mut self, si: usize, c: crate::geom::Point2, rx: f64, ry: f64, rot: f64, purpose: crate::feature::Purpose) -> Id {
        let (cx, cy) = (c.x, c.y);
        let construction = purpose == crate::feature::Purpose::Construction;
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        let (ux, uy) = (rot.cos(), rot.sin());
        let c = self.sketch_point_at(si, cx, cy, 1e-6);
        let ma = self.sketch_point_at(si, cx + rx * ux, cy + rx * uy, 1e-6); // Endpoint of the major axis.
        let mi = self.sketch_point_at(si, cx - ry * uy, cy + ry * ux, 1e-6); // Endpoint of the minor axis, perpendicular to it.
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::EllipseParts { c, ma, mi, span: None }.into(), construction });
        self.regen_sketch(si);
        c
    }
    /// ADD AN ARC OF AN ELLIPSE: the ellipse of `arc.centre`, semi-axes `arc.radii` (major, minor) turned by `arc.rot`, from
    /// the eccentric angle `arc.angles[0]` to `arc.angles[1]` the way `arc.winding` goes. Its ends are points of the
    /// sketch held on the ellipse. Answers its id.
    pub fn add_ellipse_arc_entity(&mut self, si: usize, arc: EllipseArcSpec, purpose: crate::feature::Purpose) -> Id {
        let (rx, ry) = (arc.radii.x.max(0.01), arc.radii.y.max(0.01));
        let (u, v) = (crate::geom::Point2::new(rx * arc.rot.cos(), rx * arc.rot.sin()), crate::geom::Point2::new(-ry * arc.rot.sin(), ry * arc.rot.cos()));
        let at = |t: f64| crate::geom::Point2::new(arc.centre.x + u.x * t.cos() + v.x * t.sin(), arc.centre.y + u.y * t.cos() + v.y * t.sin());
        let c = self.sketch_point_at(si, arc.centre.x, arc.centre.y, 1e-6);
        let ma = self.sketch_point_at(si, arc.centre.x + u.x, arc.centre.y + u.y, 1e-6);
        let mi = self.sketch_point_at(si, arc.centre.x + v.x, arc.centre.y + v.y, 1e-6);
        // the ends are points of their own, not glued to an end of an axis they may stand on
        let (from, to) = (self.alloc_id(), self.alloc_id());
        let (p0, p1) = (at(arc.angles[0]), at(arc.angles[1]));
        self.sketches[si].points.push(SketchPoint { id: from, x: p0.x, y: p0.y });
        self.sketches[si].points.push(SketchPoint { id: to, x: p1.x, y: p1.y });
        let id = self.alloc_id();
        let span = Some(EllipseSpan { from, to, winding: arc.winding });
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::EllipseParts { c, ma, mi, span }.into(), construction: purpose == crate::feature::Purpose::Construction });
        self.regen_sketch(si);
        id
    }
    /// The major and minor semi-axes of an ellipse entity, by the id of its centre.
    pub fn ellipse_axes(&self, si: usize, center: Id) -> Option<(f64, f64)> {
        let s = self.sketches.get(si)?;
        let (ma, mi) = s.entities.iter().find_map(|e| match e.kind.as_ellipse() {
            Some(crate::model::EllipseParts { c, ma, mi, .. }) if c == center => Some((ma, mi)),
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
        if !self.ellipse_axes_points(si, center, [rx, ry]) {
            return false;
        }
        self.solve_sketch(si);
        true
    }
    /// The points of the axes of an ellipse set to semi-axes `rx`, `ry` along their directions, with no solve: the
    /// evaluation of the parameters sets them before the solve that follows it.
    pub(crate) fn ellipse_axes_points(&mut self, si: usize, center: Id, semi: [f64; 2]) -> bool {
        let (rx, ry) = (semi[0].max(0.01), semi[1].max(0.01));
        let Some(s) = self.sketches.get(si) else { return false };
        let Some((ma, mi)) = s.entities.iter().find_map(|e| match e.kind.as_ellipse() {
            Some(crate::model::EllipseParts { c, ma, mi, .. }) if c == center => Some((ma, mi)),
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
        true
    }
    /// Vertices of a parametric polygon: the points bound by `PointOnCircle` to the centre of its circumscribed
    /// circle, in creation order. The handle of the polygon is the id of that centre.
    pub(crate) fn polygon_vertices(&self, si: usize, center: Id) -> Vec<Id> {
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
                if e.kind.as_circle().is_some_and(|c| c.center == center) {
                    e.kind.set_radius(r);
                    found = true;
                }
            }
        }
        if found {
            self.solve_sketch(si);
        }
        found
    }
    /// THE DIMENSION ON THE EDGE OF A POLYGON - the length between its first two corners - by its place among the
    /// constraints of the sketch; `None` when the polygon is held by the circle of its corners or by nothing.
    pub fn polygon_side_dim(&self, si: usize, center: Id) -> Option<usize> {
        let [a, b] = self.polygon_vertices(si, center).get(..2)?.try_into().ok()?;
        self.sketches.get(si)?.constraints.iter().position(|c| matches!(c, Constraint::Distance { a: x, b: y, .. } if (*x == a && *y == b) || (*x == b && *y == a)))
    }
    /// THE DIMENSION ON THE EDGE OF A POLYGON, laid when it has none, at the length the edge has: a polygon sized by
    /// its edge is held by it, and a dimension of the circle of its corners, which would hold the same size twice, goes.
    /// Answers its place among the constraints of the sketch.
    pub fn ensure_polygon_side(&mut self, si: usize, center: Id) -> Option<usize> {
        let s = self.sketches.get_mut(si)?;
        s.constraints.retain(|c| !matches!(c, Constraint::Diameter { c: cc, .. } if *cc == center));
        if let Some(i) = self.polygon_side_dim(si, center) {
            return Some(i);
        }
        let [a, b] = self.polygon_vertices(si, center).get(..2)?.try_into().ok()?;
        let s = self.sketches.get_mut(si)?;
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let ((ax, ay), (bx, by)) = (at(a)?, at(b)?);
        let d = (bx - ax).hypot(by - ay);
        s.constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
        Some(s.constraints.len() - 1)
    }
    /// Set the length of the edge of a polygon: through the dimension on its edge when it holds one, otherwise through
    /// the radius of the circle of its corners that gives that edge. Returns `false` when the entity is not a polygon.
    pub fn set_polygon_side(&mut self, si: usize, center: Id, len: f64) -> bool {
        let len = len.max(0.01);
        if let Some(i) = self.polygon_side_dim(si, center) {
            if let Some(Constraint::Distance { d, .. }) = self.sketches[si].constraints.get_mut(i) {
                *d = len;
            }
            self.solve_sketch(si);
            return true;
        }
        let n = self.polygon_vertices(si, center).len().max(3) as f64;
        self.set_polygon_radius(si, center, len / (2.0 * (std::f64::consts::PI / n).sin()))
    }
    /// Centre x, centre y and radius of the circumscribed circle of a polygon, by the id of its centre.
    pub fn polygon_circle(&self, si: usize, center: Id) -> Option<(f64, f64, f64)> {
        let s = self.sketches.get(si)?;
        let (cx, cy) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y))?;
        let r = s.entities.iter().find_map(|e| match e.kind.as_circle() {
            Some(crate::model::CircleParts { center: cc, r }) if cc == center => Some(r),
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
            entities.push(SketchEntity { id, kind: crate::model::LineParts { a, b }.into(), construction: false });
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
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            figures: Vec::new(),
            projections: Vec::new(),
            offsets: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
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
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            figures: Vec::new(),
            projections: Vec::new(),
            offsets: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
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
            entities.push(SketchEntity { id, kind: crate::model::LineParts { a: points[k].id, b: points[(k + 1) % n].id }.into(), construction: false });
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
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).collect();
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
        if !matches!(
            s.constraints.get(ci),
            Some(Constraint::Distance { .. })
                | Some(Constraint::Angle { .. })
                | Some(Constraint::DistancePL { .. })
                | Some(Constraint::AngleLines { .. })
                | Some(Constraint::ArcLength { .. })
                | Some(Constraint::Diameter { .. })
                | Some(Constraint::EdgeDistance { .. })
                | Some(Constraint::CurveDistance { .. })
        ) {
            return false;
        }
        let intr = self.entity_intrinsics(si);
        let mut without: Vec<Constraint> = s.constraints.iter().enumerate().filter(|(i, c)| *i != ci && s.solves(c)).map(|(_, c)| s.as_solved(c)).collect();
        without.extend(intr.iter().cloned());
        let mut with = without;
        with.push(s.as_solved(&s.constraints[ci]));
        // The rank did not grow, so the dimension constrains nothing.
        crate::solver::freedom_taken(&s.points, &self.entity_radii(si), &with, with.len() - 1) == 0
    }
    /// Redundant constraints: non-reference constraints whose removal frees no degree of freedom (the rank of
    /// the Jacobian does not drop), so they can be removed without losing determinacy.
    ///
    /// It names the specific redundant constraints rather than only counting them. Computed only when there is
    /// an excess to explain.
    pub fn sketch_redundant_constraints(&self, si: usize) -> Vec<usize> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        // the sketch's own constraints that constrain (not the reference ones), by their place in the sketch, then the
        // entities' own
        let at: Vec<usize> = (0..s.constraints.len()).filter(|&ci| s.solves(&s.constraints[ci])).collect();
        let mut active: Vec<Constraint> = at.iter().map(|&ci| s.as_solved(&s.constraints[ci])).collect();
        active.extend(self.entity_intrinsics(si));
        crate::solver::redundant(&s.points, &self.entity_radii(si), &active, at.len()).into_iter().map(|k| at[k]).collect()
    }
    /// Add a constraint only when it is independent, that is, when it reduces the degrees of freedom by raising
    /// the rank of the Jacobian. Redundant automatic constraints (inferred while drawing) are dropped, so the
    /// sketch is not over-constrained. Returns whether the constraint was added.
    pub fn add_constraint_if_independent(&mut self, si: usize, c: Constraint) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let intr = self.entity_intrinsics(si);
        let radii = self.entity_radii(si);
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).collect();
        active.extend(intr.iter().cloned());
        let mut with = active;
        with.push(s.as_solved(&c));
        // JUDGED WHERE THE CONSTRAINTS HOLD, on a copy solved with the new one: at the geometry as clicked, nearly but not
        // quite satisfying what is laid, constraints that follow from each other read as independent. A U drawn with
        // Line, its corners squared, got a Parallel between its legs on top of the two Perpendiculars that imply it, and
        // once solved all of them stood redundant. Only the part the new constraint lies in is solved and counted.
        if crate::solver::freedom_taken_where_solved(&s.points, &radii, &with, with.len() - 1) > 0 {
            self.sketches[si].constraints.push(c);
            true
        } else {
            false
        }
    }
    /// THE NEW CONSTRAINTS THAT CONSTRAIN SOMETHING, in their order: each is kept only when it raises the rank of the
    /// Jacobian over what the sketch holds and the ones kept before it. A second Vertical on a line that has one, or
    /// Vertical on a line Vertical by other constraints, constrains nothing, and laid beside the first it turned both
    /// redundant (yellow). Equal radii pass untouched: the caller makes a radius a reference for them first.
    pub fn independent_of(&self, si: usize, new: Vec<Constraint>) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let radii = self.entity_radii(si);
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).collect();
        active.extend(self.entity_intrinsics(si));
        let mut kept = Vec::new();
        for c in new {
            if matches!(c, Constraint::EqualRadius { .. }) {
                kept.push(c);
                continue;
            }
            active.push(s.as_solved(&c));
            if crate::solver::freedom_taken(&s.points, &radii, &active, active.len() - 1) > 0 {
                kept.push(c);
            } else {
                active.pop();
            }
        }
        kept
    }
    /// LIFT THE RELATIONS A NEW ONE MADE REDUNDANT: among the constraints standing before index `had`, a relation
    /// between lines (horizontal, vertical, parallel, perpendicular, equal, collinear) whose points all lie in `among`
    /// and whose removal frees no degree of freedom is removed. Collinear on three vertical lines makes the Vertical of
    /// the second and third follow from the first; kept, they stood yellow. Returns how many went.
    pub fn drop_implied_relations(&mut self, si: usize, had: usize, among: &std::collections::HashSet<Id>) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        if self.sketch_dof(si).1 <= 0 {
            return 0;
        }
        let radii = self.entity_radii(si);
        let intr = self.entity_intrinsics(si);
        let relation = |c: &Constraint| {
            matches!(
                c,
                Constraint::Horizontal { .. } | Constraint::Vertical { .. } | Constraint::Parallel { .. } | Constraint::Perpendicular { .. } | Constraint::Equal { .. } | Constraint::Collinear { .. }
            )
        };
        let mut cs = s.constraints.clone();
        let active = |cs: &[Constraint]| -> Vec<Constraint> { cs.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).chain(intr.iter().cloned()).collect() };
        let mut gone = 0;
        for ci in (0..had.min(cs.len())).rev() {
            if !relation(&cs[ci]) || !constraint_point_ids(&cs[ci]).iter().all(|p| among.contains(p)) {
                continue;
            }
            // the place of the relation among the active constraints: the reference ones before it are not there
            let at = cs[..ci].iter().filter(|c| s.solves(c)).count();
            if crate::solver::freedom_taken(&s.points, &radii, &active(&cs), at) == 0 {
                cs.remove(ci);
                gone += 1;
            }
        }
        self.sketches[si].constraints = cs;
        gone
    }
    /// If dimension `ci` is redundant, turn it into a reference (driven) dimension. Returns whether it did.
    pub fn auto_driven(&mut self, si: usize, ci: usize) -> bool {
        if !self.dim_redundant(si, ci) {
            return false;
        }
        if let Some(
            Constraint::AngleLines { driven, .. }
            | Constraint::Distance { driven, .. }
            | Constraint::Angle { driven, .. }
            | Constraint::DistancePL { driven, .. }
            | Constraint::Diameter { driven, .. }
            | Constraint::ArcLength { driven, .. }
            | Constraint::EdgeDistance { driven, .. }
            | Constraint::CurveDistance { driven, .. },
        ) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci))
        {
            *driven = true;
            return true;
        }
        false
    }
    /// Toggle dimension `ci` between driving and reference. Returns the new state.
    pub fn toggle_driven(&mut self, si: usize, ci: usize) -> bool {
        if let Some(
            Constraint::AngleLines { driven, .. }
            | Constraint::Distance { driven, .. }
            | Constraint::Angle { driven, .. }
            | Constraint::DistancePL { driven, .. }
            | Constraint::Diameter { driven, .. }
            | Constraint::ArcLength { driven, .. }
            | Constraint::CurveDistance { driven, .. },
        ) = self.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci))
        {
            *driven = !*driven;
            return *driven;
        }
        false
    }
    /// Mobility mask of the sketch points (`true` means the point can still move), one entry per point.
    /// THE CHECKS OF A SKETCH COUNTED TOGETHER (`solver::checks`): its degrees of freedom, its free points and its
    /// redundant constraints by their place, from one elimination a part - the answers of `sketch_dof`,
    /// `sketch_free_points` and `sketch_redundant_constraints`.
    pub fn sketch_checks(&self, si: usize) -> crate::solver::Checks {
        let Some(s) = self.sketches.get(si) else { return crate::solver::Checks { dof: (0, 0), free: Vec::new(), redundant: Vec::new() } };
        let at: Vec<usize> = (0..s.constraints.len()).filter(|&ci| s.solves(&s.constraints[ci])).collect();
        let mut active: Vec<Constraint> = at.iter().map(|&ci| s.as_solved(&s.constraints[ci])).collect();
        active.extend(self.entity_intrinsics(si));
        let radii = self.entity_radii(si);
        let mut checks = s.checked.with(|memo| crate::solver::checks_remembered(&s.points, &radii, &active, at.len(), memo));
        checks.redundant = checks.redundant.into_iter().map(|k| at[k]).collect();
        checks
    }
    pub fn sketch_free_points(&self, si: usize) -> Vec<bool> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| s.solves(c)).map(|c| s.as_solved(c)).collect();
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
    pub fn add_sketch_text(&mut self, si: usize, t: TextSpec, purpose: crate::feature::Purpose) -> Id {
        let TextSpec { at, height, angle, text, glyphs, font } = t;
        let (x, y) = (at.x, at.y);
        let construction = purpose == crate::feature::Purpose::Construction;
        let id = self.alloc_id();
        if let Some(s) = self.sketches.get_mut(si) {
            s.texts.push(SketchText { id, x, y, height, angle, text, construction, glyphs, font });
        }
        self.regen_sketch(si);
        id
    }
    /// Update the parameters of a text and its baked glyphs, after the application re-baked them for a new
    /// font or string.
    pub fn set_sketch_text(&mut self, si: usize, ti: usize, t: TextSpec) {
        let TextSpec { at, height, angle, text, glyphs, font } = t;
        let (x, y) = (at.x, at.y);
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            t.x = x;
            t.y = y;
            t.height = height;
            t.angle = angle;
            t.text = text;
            t.glyphs = glyphs;
            t.font = font;
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
    /// TURN TEXT `ti` by `deg` about (cx, cy): its place goes round the centre, its angle grows by `deg`, and its baked
    /// glyphs turn with it - no font is needed, as for a move. Reported behaviour (issue #32): the Rotate tool did not
    /// take a text, and a text stood level always.
    pub fn rotate_sketch_text(&mut self, si: usize, ti: usize, cx: f64, cy: f64, deg: f64) {
        let (sn, cs) = deg.to_radians().sin_cos();
        let turn = |x: f64, y: f64| (cx + (x - cx) * cs - (y - cy) * sn, cy + (x - cx) * sn + (y - cy) * cs);
        if let Some(t) = self.sketches.get_mut(si).and_then(|s| s.texts.get_mut(ti)) {
            (t.x, t.y) = turn(t.x, t.y);
            t.angle += deg;
            for p in t.glyphs.iter_mut().flatten() {
                (p.x, p.y) = turn(p.x, p.y);
            }
        }
        self.regen_sketch(si);
    }
    /// COPY TEXT `ti` shifted by (dx, dy): the same string, height, angle and font, the glyphs shifted with it. Returns
    /// the id of the copy.
    pub fn copy_sketch_text(&mut self, si: usize, ti: usize, dx: f64, dy: f64) -> Option<Id> {
        let mut copy = self.sketches.get(si)?.texts.get(ti)?.clone();
        copy.id = self.alloc_id();
        copy.x += dx;
        copy.y += dy;
        for p in copy.glyphs.iter_mut().flatten() {
            p.x += dx;
            p.y += dy;
        }
        let id = copy.id;
        self.sketches[si].texts.push(copy);
        self.regen_sketch(si);
        Some(id)
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
    /// ADD A SPLINE through `pts`, an entity like any curve: its nodes are points of the sketch, its tangents automatic.
    /// Answers its id, or `None` for fewer than two points or no such sketch.
    pub fn add_spline(&mut self, si: usize, pts: Vec<Point2>, ends: crate::feature::Ends, purpose: crate::feature::Purpose) -> Option<Id> {
        if pts.len() < 2 || si >= self.sketches.len() {
            return None;
        }
        // A NODE CLICKED ON A POINT STANDING THERE IS THAT POINT, as the end of a line is: a spline drawn on from the end
        // of a line shares the point with it and makes a corner with it. Each node a point of its own, the end and the
        // line were two points held together, and the corner tools found no corner there.
        let mut ids: Vec<Id> = Vec::with_capacity(pts.len());
        for p in &pts {
            let id = self.sketch_point_at(si, p.x, p.y, 1e-6);
            let id = if ids.contains(&id) {
                let fresh = self.alloc_id();
                self.sketches[si].points.push(SketchPoint { id: fresh, x: p.x, y: p.y });
                fresh
            } else {
                id
            };
            ids.push(id);
        }
        let eid = self.alloc_id();
        let s = self.sketches.get_mut(si)?;
        let handles = vec![Handle::default(); ids.len()];
        s.entities.push(SketchEntity { id: eid, kind: EntityKind::spline(ids, handles, ends), construction: purpose == crate::feature::Purpose::Construction });
        self.regen_sketch(si);
        Some(eid)
    }
    /// THE NODES, HANDLES AND ENDS OF SPLINE `eid`, with its nodes where they stand; `None` when it is no spline of the
    /// sketch or has lost a node.
    fn spline_of(&self, si: usize, eid: Id) -> Option<SplineNow> {
        let s = self.sketches.get(si)?;
        let sp = s.entity(eid)?.kind.as_spline()?;
        let at: Vec<Point2> = sp.nodes.iter().map(|id| s.point(*id).map(|p| crate::geom::Point2::new(p.x, p.y))).collect::<Option<_>>()?;
        Some(SplineNow { at, handles: sp.handles.to_vec(), ends: sp.ends })
    }
    /// Polyline of spline `eid` in world coordinates, for drawing - including the dashed
    /// rendering of a construction spline, which never reaches the contours.
    pub fn spline_polyline(&self, si: usize, eid: Id) -> Vec<Point2> {
        match self.spline_of(si, eid) {
            Some(sp) if sp.at.len() >= 2 => crate::model::spline_line(&sp.at, &sp.handles, sp.ends),
            _ => Vec::new(),
        }
    }
    /// Tangent handles of spline `eid`: for each node, its position and the end of its handle - the node plus the tangent
    /// it leaves by, explicit or automatic - in world coordinates. Used for drawing and dragging.
    pub fn spline_handles(&self, si: usize, eid: Id) -> Vec<SplineHandle> {
        let Some(sp) = self.spline_of(si, eid).filter(|sp| sp.at.len() >= 2) else { return Vec::new() };
        let tangents = crate::geom::bspline::node_tangents(&sp.at, &sp.handles, sp.ends);
        (0..sp.at.len())
            .map(|i| {
                let m = tangents[i].leave;
                SplineHandle { node: sp.at[i], end: crate::geom::Point2::new(sp.at[i].x + m.x, sp.at[i].y + m.y) }
            })
            .collect()
    }
    /// Set the tangent of node `ki` of spline `eid` from a dragged handle whose end is at `to`, in world coordinates. This
    /// makes the tangent explicit - the same both ways, so the node stays smooth - pinning the shape at that node, and
    /// rebuilds the contour. Returns `false` when there is no such node.
    pub fn set_spline_handle(&mut self, si: usize, eid: Id, ki: usize, to: Point2) -> bool {
        let Some(sp) = self.spline_of(si, eid) else { return false };
        let Some(&node) = sp.at.get(ki) else { return false };
        let v = [to.x - node.x, to.y - node.y];
        if !self.with_handles(si, eid, |h| h[ki] = Handle { arrive: Some(v), leave: Some(v) }) {
            return false;
        }
        self.regen_sketch(si);
        true
    }
    /// Reset the tangent of a node to automatic, so the handle follows the nodes again.
    pub fn reset_spline_handle(&mut self, si: usize, eid: Id, ki: usize) -> bool {
        let Some(sp) = self.spline_of(si, eid) else { return false };
        if ki >= sp.at.len() || !self.with_handles(si, eid, |h| h[ki] = Handle::default()) {
            return false;
        }
        self.regen_sketch(si);
        true
    }
    /// The handles of spline `eid` given to `change`, one per node however many were kept. Answers whether it is a spline.
    fn with_handles(&mut self, si: usize, eid: Id, change: impl FnOnce(&mut Vec<Handle>)) -> bool {
        let Some(e) = self.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)) else { return false };
        let Some(sp) = e.kind.as_spline_mut() else { return false };
        sp.handles.resize(sp.nodes.len(), Handle::default());
        change(sp.handles);
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
                s.entities.push(SketchEntity { id: eid, kind: crate::model::LineParts { a: pids[k], b: pids[(k + 1) % n] }.into(), construction: true });
            }
        }
    }
    /// Unique points referenced by the given entities, centres included. An id naming a point stands for the point
    /// itself: a point drawn alone is a shape of its own, which Move, Rotate, Copy, Mirror and the patterns take.
    pub fn entity_point_ids(&self, si: usize, eids: &[Id]) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out: Vec<Id> = Vec::new();
        for e in &s.entities {
            if !eids.contains(&e.id) {
                continue;
            }
            for id in entity_points(e) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        for p in &s.points {
            if eids.contains(&p.id) && !out.contains(&p.id) {
                out.push(p.id);
            }
        }
        out
    }
    /// Centroid of the selected entities (the mean of their points).
    pub fn entities_centroid(&self, si: usize, eids: &[Id]) -> (f64, f64) {
        let pts = self.movable_of(si, eids);
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
        self.break_figures_of(si, &[]); // a figure whose curve a trim or a break replaced is a figure no more
        let Some(s) = self.sketches.get_mut(si) else { return };
        let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(entity_points).collect();
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
        s.drop_constraints_on_what_is_gone();
    }
    /// Delete the selected entities, together with the points they orphan, the constraints on those points,
    /// and the constraints that hung on the deleted lines by their pair of endpoints — even when those
    /// endpoints are shared and still alive.
    pub fn delete_entities(&mut self, si: usize, eids: &[Id]) {
        // A RECTANGLE LOSING A SIDE IS BROKEN: its diagonals go with the side, its centre with them, and the constraints
        // it held itself by - four plain lines are left, with nothing of the rectangle on them.
        let mut eids = eids.to_vec();
        let before: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects()).unwrap_or_default();
        if let Some(s) = self.sketches.get_mut(si) {
            let broken: Vec<crate::model::SketchRect> = before.iter().filter(|r| r.sides.iter().chain(r.diagonals.iter().flatten()).any(|e| eids.contains(e))).cloned().collect();
            s.drop_rects(|r| broken.contains(r));
            for r in &broken {
                // the centre is a point of the rectangle no curve stands on: named, it goes, as a point drawn alone does not
                eids.extend(r.diagonals.into_iter().flatten().chain([r.centre]));
                let [c0, c1, c2, c3] = r.corners;
                let own = rect_own_constraints(c0, c1, c2, c3, r.centre, 0.0);
                s.constraints.retain(|c| !own.iter().any(|o| same_rect_constraint(o, c)));
            }
        }
        // A FIGURE LOSING A CURVE IS BROKEN the same way: the record and its own constraints go, plain curves are left.
        self.break_figures_of(si, &eids);
        let eids = &eids[..];
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            // Endpoint pairs of the lines being deleted: constraints referencing those lines go as well.
            let dead_lines: Vec<(Id, Id)> = s
                .entities
                .iter()
                .filter(|e| eids.contains(&e.id))
                .filter_map(|e| match e.kind.as_line() {
                    Some(crate::model::LineParts { a, b }) => Some((a, b)),
                    _ => None,
                })
                .collect();
            // A POINT DRAWN ALONE STAYS unless it is named among what goes: it is no end that a deleted curve orphans.
            // Swept with the orphans, a point drawn by the point tool went whenever any curve of the sketch was deleted.
            // The middle of a line is a place on it, not a point drawn alone.
            let lone_before: std::collections::HashSet<Id> = {
                let middles = s.constraints.iter().filter_map(|c| if let Constraint::Midpoint { p, .. } = c { Some(*p) } else { None });
                let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(entity_points).chain(middles).collect();
                s.points.iter().map(|p| p.id).filter(|id| !used.contains(id) && !eids.contains(id)).collect()
            };
            // Drop the constraints attached to a deleted line (horizontal, vertical, parallel, perpendicular,
            // equal, collinear, tangent, midpoint — those referencing its pair of endpoints), or dangling
            // glyphs remain.
            if !dead_lines.is_empty() {
                s.constraints.retain(|c| !constraint_uses_line(c, &dead_lines));
            }
            s.entities.retain(|e| !eids.contains(&e.id));
            let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(entity_points).chain(lone_before).collect();
            // Protected free points: the system ones (origin and axes) and materialised midpoints. They must
            // not be dropped as orphaned endpoints, or the axes and the dimensions measured from them are
            // lost.
            let protected: std::collections::HashSet<Id> =
                s.system_ids().into_iter().chain(s.constraints.iter().filter_map(|c| if let Constraint::Midpoint { p, .. } = c { Some(*p) } else { None })).collect();
            s.points.retain(|p| used.contains(&p.id) || protected.contains(&p.id));
            s.drop_constraints_on_what_is_gone();
        }
        self.regen_sketch(si);
        // the sides a broken rectangle leaves keep what they plainly have, as they do in a sketch of plain lines
        self.relate_lines_left(si, &before);
    }
    /// Toggle the selected entities between ordinary and construction geometry. Construction geometry never
    /// reaches a profile. Returns the new state.
    pub fn toggle_construction(&mut self, si: usize, eids: &[Id]) -> bool {
        let mut now_construction = false;
        if let Some(s) = self.sketches.get_mut(si) {
            // ONE STATE FOR THE WHOLE SET: construction while any of it is ordinary, ordinary once all of it is construction.
            // Taken as the inverse of the first one chosen, a polygon - six sides and its construction circle - turned
            // ordinary or construction by the order it was picked in.
            let target = s.entities.iter().filter(|e| eids.contains(&e.id)).any(|e| !e.construction);
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
    /// orphan. The other points of those entities go with them where nothing else stands on them; every other point,
    /// the origin included, is kept.
    pub fn delete_points(&mut self, si: usize, ids: &[Id]) {
        {
            let Some(s) = self.sketches.get_mut(si) else { return };
            let mut set: std::collections::HashSet<Id> = ids.iter().copied().collect();
            // Entities incident to the deleted points go as well.
            let incident = |e: &SketchEntity| entity_points(e).iter().any(|p| set.contains(p));
            let going: Vec<SketchEntity> = s.entities.iter().filter(|e| incident(e)).cloned().collect();
            s.entities.retain(|e| !incident(e));
            // THE FAR END OF A CURVE TAKEN AWAY GOES WITH IT: kept, it stood as a point of its own that nobody drew, and
            // the end of a line deleted left the other end behind as a dot.
            let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(entity_points).collect();
            let protected: std::collections::HashSet<Id> =
                s.system_ids().into_iter().chain(s.constraints.iter().filter_map(|c| if let Constraint::Midpoint { p, .. } = c { Some(*p) } else { None })).collect();
            set.extend(going.iter().flat_map(entity_points).filter(|p| !used.contains(p) && !protected.contains(p)));
            s.points.retain(|p| !set.contains(&p.id));
            s.drop_constraints_on_what_is_gone();
            if set.contains(&s.origin) {
                s.origin = 0;
            }
        }
        self.regen_sketch(si);
    }
    /// THE FIGURES OF SKETCH `si` THAT HAVE LOST A CURVE, or are about to lose one of `going`, broken: the record and the
    /// constraints the figure held itself by go, its other curves stay as plain curves.
    pub(crate) fn break_figures_of(&mut self, si: usize, going: &[Id]) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let alive: std::collections::HashSet<Id> = s.entities.iter().map(|e| e.id).filter(|id| !going.contains(id)).collect();
        // a rectangle is broken with its centre and its diagonals where a side is deleted (`delete_entities`)
        let broken: Vec<SketchFigure> = s.shapes_not_rects().filter(|f| f.curves.iter().any(|c| !alive.contains(c))).cloned().collect();
        if broken.is_empty() {
            return;
        }
        s.figures.retain(|f| !broken.iter().any(|b| b.id == f.id));
        s.constraints.retain(|c| !broken.iter().any(|f| f.holds(c)));
    }
    /// Find a sketch point near (x, y) within `eps`, or create one. Returns its id.
    pub fn sketch_point_at(&mut self, si: usize, x: f64, y: f64, eps: f64) -> Id {
        // THE FRAME OF REFERENCE IS NEVER ADOPTED AS SOMEBODY'S GEOMETRY.
        //
        // Reported behaviour: "I cannot move the circle away from the origin", and before that "a
        // dimension to the X or Y axis comes out crooked". Both came from here. A click at zero found
        // the origin already standing there and handed it over as the new circle's centre - so the circle
        // WAS the origin. Drag it and the reference frame travels with it, tilting both axes; pin the
        // frame and the circle can no longer be moved. The drawing and the frame it is measured from
        // cannot be the same point.
        //
        // Geometry landing on the origin gets a point of its own, in the same place. A coincidence with
        // the origin is a CONSTRAINT, made by clicking it with the constraint tool - not by silently
        // sharing the id.
        let sys = self.sketches[si].system_ids();
        // the distance told by its square first, the frame of reference only for a point that near: a root and a look
        // at the frame for every point of the sketch was 2 s of filleting every corner of 2 000 rectangles
        let near = eps * eps;
        if let Some(p) = self.sketches[si].points.iter().find(|p| (p.x - x).powi(2) + (p.y - y).powi(2) <= near && !sys.contains(&p.id)) {
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
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a, b }.into(), construction });
        self.regen_sketch(si);
        id
    }
    /// Whether point `pid` is already used as the centre of a circle or an arc, and so carries a solver radius
    /// variable.
    ///
    /// A radius variable is keyed by centre id (see `solver::RadiusVar`), so two curves cannot share one centre
    /// node without their radii collapsing. Concentricity is a constraint, not a shared node.
    pub(crate) fn is_radius_center(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.kind.round_centre() == Some(pid)))
    }
    /// Centre node of a radius curve (circle, arc, polygon, slot) at (x, y).
    ///
    /// Ordinary deduplication, except that landing on another curve's centre (drawing concentrically from one
    /// point) allocates a separate node: the solver radius variable is keyed by centre, and a shared node would
    /// collapse the radii. Concentricity is expressed as a constraint instead.
    pub(crate) fn radius_center_at(&mut self, si: usize, x: f64, y: f64) -> Id {
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
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::CircleParts { center: c, r: r.max(0.01) }.into(), construction });
        self.regen_sketch(si);
        id
    }
    /// A rectangle by two opposite corners, kept as one shape (see `SketchRect`); it grows from the corner at
    /// (`x0`, `y0`). Returns the ids of its four sides.
    pub fn add_rect_entity(&mut self, si: usize, x0: f64, y0: f64, x1: f64, y1: f64, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        let corners = [Point2::new(xa, ya), Point2::new(xb, ya), Point2::new(xb, yb), Point2::new(xa, yb)];
        let first = corners.iter().position(|c| c.x == x0 && c.y == y0).unwrap_or(0);
        self.lay_rect(si, corners, DrawnFrom::Corner(first), purpose)
    }
    /// A rectangle drawn from its centre and a corner, kept as one shape; it grows about the centre. Returns the ids
    /// of its four sides.
    pub fn add_rect_from_centre(&mut self, si: usize, centre: Point2, corner: Point2, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (dx, dy) = ((corner.x - centre.x).abs(), (corner.y - centre.y).abs());
        let corners = [
            Point2::new(centre.x - dx, centre.y - dy),
            Point2::new(centre.x + dx, centre.y - dy),
            Point2::new(centre.x + dx, centre.y + dy),
            Point2::new(centre.x - dx, centre.y + dy),
        ];
        self.lay_rect(si, corners, DrawnFrom::Centre, purpose)
    }
    /// LAY A RECTANGLE OF FOUR CORNERS in order round it: the sides, and - for real geometry - the shape kept as one:
    /// opposite sides parallel, the first two square, the turn of the first side held (`Orientation`), a centre point
    /// on the middle of the diagonal, the two construction diagonals for a rectangle drawn from its centre, and the
    /// record. Construction
    /// geometry gets the four sides alone. Returns the ids of the sides.
    pub(crate) fn lay_rect(&mut self, si: usize, corners: [Point2; 4], from: DrawnFrom, purpose: crate::feature::Purpose) -> Vec<Id> {
        let construction = purpose == crate::feature::Purpose::Construction;
        let pids: Vec<Id> = corners.iter().map(|c| self.sketch_point_at(si, c.x, c.y, 1e-6)).collect();
        let mut sides = [0; 4];
        for (k, side) in sides.iter_mut().enumerate() {
            *side = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id: *side, kind: crate::model::LineParts { a: pids[k], b: pids[(k + 1) % 4] }.into(), construction });
        }
        // A RECTANGLE DRAWN AS CONSTRUCTION IS A RECTANGLE all the same - its own constraints and its record - so its size
        // is typed and it is dragged by its corners whole. Reported behaviour: "a construction rectangle is just four
        // lines: its size cannot be changed and it cannot be dragged by its corners".
        let [c0, c1, c2, c3] = [pids[0], pids[1], pids[2], pids[3]];
        let mid = Point2::new((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0);
        let centre = self.sketch_point_at(si, mid.x, mid.y, 1e-6);
        let diagonals = matches!(from, DrawnFrom::Centre).then(|| {
            let mut d = [0; 2];
            for (id, (a, b)) in d.iter_mut().zip([(c0, c2), (c1, c3)]) {
                *id = self.alloc_id();
                self.sketches[si].entities.push(SketchEntity { id: *id, kind: crate::model::LineParts { a, b }.into(), construction: true });
            }
            d
        });
        let deg = (corners[1].y - corners[0].y).atan2(corners[1].x - corners[0].x).to_degrees();
        for c in rect_own_constraints(c0, c1, c2, c3, centre, deg) {
            self.add_constraint_if_independent(si, c);
        }
        let anchor = match from {
            DrawnFrom::Corner(k) => crate::model::RectAnchor::Corner(pids[k]),
            DrawnFrom::Centre => crate::model::RectAnchor::Centre,
        };
        let id = self.alloc_id();
        self.sketches[si].add_rect(&crate::model::SketchRect { id, corners: [c0, c1, c2, c3], sides, centre, diagonals, anchor });
        self.regen_sketch(si);
        sides.to_vec()
    }
    /// Like `add_polygon_entity`, but returns the id of the circumscribed circle centre together with the side
    /// ids.
    pub fn add_polygon_param(&mut self, si: usize, c: crate::geom::Point2, v: crate::geom::Point2, n: u32, purpose: crate::feature::Purpose) -> (Id, Vec<Id>) {
        let ((cx, cy), (vx, vy)) = ((c.x, c.y), (v.x, v.y));
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
        self.sketches[si].entities.push(SketchEntity { id: circ, kind: crate::model::CircleParts { center, r }.into(), construction: true });
        let mut eids = Vec::with_capacity(n);
        for k in 0..n {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a: pids[k], b: pids[(k + 1) % n] }.into(), construction });
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
        let own: Vec<Constraint> = s.constraints[s.constraints.len() - (2 * n - 1)..].to_vec();
        s.constraints.push(Constraint::Diameter { c: center, d: r, off: -a0 + std::f64::consts::PI / n as f64, expr: String::new(), driven: false, diam: false, at: None });
        let id = self.alloc_id();
        let curves = eids.iter().copied().chain([circ]).collect();
        self.sketches[si].figures.push(SketchFigure { id, shape: FigureShape::Polygon, curves, own });
        self.regen_sketch(si);
        (center, eids)
    }
    /// Add a slot between two centres with radius `r`: two lines plus two end arcs.
    pub fn add_slot_entity(&mut self, si: usize, c1: crate::geom::Point2, c2: crate::geom::Point2, r: f64, purpose: crate::feature::Purpose) {
        let ((c1x, c1y), (c2x, c2y)) = ((c1.x, c1.y), (c2.x, c2.y));
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
        let mut curves = Vec::with_capacity(4);
        let mut add = |kind| {
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind, construction });
            curves.push(id);
        };
        add(LineParts { a: i1, b: i2 }.into());
        add(ArcParts { center: c2, a: i3, b: i2, ccw: true }.into());
        add(LineParts { a: i3, b: i4 }.into());
        add(ArcParts { center: c1, a: i1, b: i4, ccw: true }.into());
        // A parametric slot, or dragging and dimension edits spread its corners and arcs apart. The arc
        // endpoints already sit on the radius circles of their centres (the arc intrinsics). Added on top:
        // equal end radii plus tangency of both side lines to both ends (i1 to i4 are the tangency points).
        // That keeps the sides parallel to the centreline and tangentially smooth, so the slot holds its shape
        // under the solver.
        let mut own = vec![Constraint::EqualRadius { c1, c2 }];
        // each side touches each end, by the touching their kinds take (`tangency`)
        for pair in [[curves[0], curves[3]], [curves[0], curves[1]], [curves[2], curves[3]], [curves[2], curves[1]]] {
            own.extend(self.tangency(si, pair));
        }
        self.sketches[si].constraints.extend(own.iter().cloned());
        let id = self.alloc_id();
        self.sketches[si].figures.push(SketchFigure { id, shape: FigureShape::Slot, curves, own });
        self.regen_sketch(si);
    }
    /// A rotated rectangle from three points: p1 to p2 gives one side and its direction, p3 gives the height as
    /// a projection onto the normal. Returns the ids of the four sides. Opposite sides are held parallel and
    /// adjacent ones perpendicular, so it stays a rectangle under the solver.
    pub fn add_rect3_entity(&mut self, si: usize, p1: crate::geom::Point2, p2: crate::geom::Point2, p3: crate::geom::Point2, purpose: crate::feature::Purpose) -> Vec<Id> {
        let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return Vec::new();
        }
        let (nx, ny) = (-dy / len, dx / len); // Unit normal to the side.
        let h = (p3.x - p2.x) * nx + (p3.y - p2.y) * ny; // Signed height: the projection of p3 onto the normal.
        let corners = [p1, p2, Point2::new(p2.x + nx * h, p2.y + ny * h), Point2::new(p1.x + nx * h, p1.y + ny * h)];
        self.lay_rect(si, corners, DrawnFrom::Corner(0), purpose)
    }
    /// Add an arc entity (centre, start, end, direction).
    pub fn add_arc_entity(&mut self, si: usize, c: crate::geom::Point2, a: crate::geom::Point2, b: crate::geom::Point2, winding: crate::feature::Winding, purpose: crate::feature::Purpose) {
        let ((cx, cy), (ax, ay), (bx, by)) = ((c.x, c.y), (a.x, a.y), (b.x, b.y));
        // THE END LANDS ON THE ARC: the point given for it says which way the arc runs to, and the end stands on the
        // circle of the start's radius in that direction. Laid where it was clicked, the end of an arc drawn by its centre
        // stood inside or outside the arc, away from its end (a third click at (0, 50) of an arc from (100, 0) about the
        // origin left it at (0, 50), where the arc ends at (0, 100)).
        let (r, (dx, dy)) = ((ax - cx).hypot(ay - cy), (bx - cx, by - cy));
        let d = dx.hypot(dy);
        let (bx, by) = if r > 1e-12 && d > 1e-12 { (cx + dx / d * r, cy + dy / d * r) } else { (bx, by) };
        let construction = purpose == crate::feature::Purpose::Construction;
        let ccw = winding == crate::feature::Winding::Ccw;
        let center = self.radius_center_at(si, cx, cy); // Its own centre node (see `radius_center_at`).
        let a = self.sketch_point_at(si, ax, ay, 1e-6);
        let b = self.sketch_point_at(si, bx, by, 1e-6);
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::ArcParts { center, a, b, ccw }.into(), construction });
        self.regen_sketch(si);
    }
    /// Create a sketch from a set of contours (a DXF or SVG import) and return its id.
    pub fn add_sketch(&mut self, name: impl Into<String>, contours: Vec<Contour>, source: Option<Id>) -> Id {
        let contour_ids: Vec<Id> = contours.into_iter().map(|c| self.add_contour(c)).collect();
        let id = self.alloc_id();
        self.sketches.push(Sketch {
            id,
            name: name.into(),
            contour_ids,
            source,
            points: Vec::new(),
            entities: Vec::new(),
            closed: false,
            constraints: Vec::new(),
            notes: Vec::new(),
            texts: Vec::new(),
            patterns: Vec::new(),
            figures: Vec::new(),
            projections: Vec::new(),
            offsets: Vec::new(),
            plane: crate::feature::SketchPlane::default(),
            origin: 0,
            axis_pts: [0, 0],
            frame: 0,
            origin_uv: None,
            left_unsolved: 0,
            drag_session: None,
            laid: None,
            checked: Default::default(),
            point_at: Default::default(),
            entity_at: Default::default(),
        });
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
        let mut cache = ImportPoints { welded: super::tess::Welded::new(IMPORT_WELD), ids: Vec::new() };
        let mut k = 0;
        while k < curves.len() {
            let e = &curves[k];
            k += 1;
            match *e {
                ProfEdge::Line { a, b } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    if ai != bi {
                        let id = self.alloc_id();
                        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a: ai, b: bi }.into(), construction: false });
                    }
                }
                ProfEdge::Arc { a, b, center, ccw } => {
                    let (ai, bi) = (self.import_intern_pt(si, &mut cache, a.x, a.y), self.import_intern_pt(si, &mut cache, b.x, b.y));
                    // The arc centre is its own point: it is not shared with the endpoints and is moved
                    // independently.
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::ArcParts { center: ci, a: ai, b: bi, ccw }.into(), construction: false });
                }
                ProfEdge::Circle { center, r } => {
                    let ci = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id: ci, x: center.x, y: center.y });
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::CircleParts { center: ci, r }.into(), construction: false });
                }
                ProfEdge::Oval { .. } => {
                    self.lay_curve(si, &crate::model::curve::Curve::of_edge(e), crate::model::LayAs::default());
                }
                // the pieces of a spline going on smoothly one into the next are one spline; a corner between two ends it
                ProfEdge::Spline(_) => {
                    let smooth = |a: &ProfEdge, b: &ProfEdge| matches!(b, ProfEdge::Spline(_)) && crate::model::curve::Curve::of_edge(a).goes_on_smoothly(&crate::model::curve::Curve::of_edge(b));
                    let joined = curves[k - 1..].windows(2).take_while(|w| smooth(&w[0], &w[1])).count();
                    let pieces = &curves[k - 1..k + joined];
                    k += joined;
                    if let Some(spline) = crate::model::curve::Curve::of_splines(pieces) {
                        self.lay_curve(si, &spline, crate::model::LayAs::default());
                    }
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
    /// creating one. Nearby: within `IMPORT_WELD` along x and along y; the first point made that is, as a look along
    /// all of them in order finds it - through a grid as wide (`Welded`): the 140 000 ends of a drawing of 70 000
    /// segments looked along every point before them took 5.3 s.
    pub(crate) fn import_intern_pt(&mut self, si: usize, cache: &mut ImportPoints, x: f64, y: f64) -> Id {
        let node = cache.welded.weld((x, y), |(cx, cy), (x, y)| (cx - x).abs() < IMPORT_WELD && (cy - y).abs() < IMPORT_WELD);
        if let Some(&id) = cache.ids.get(node) {
            return id;
        }
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x, y });
        cache.ids.push(id);
        id
    }
}
