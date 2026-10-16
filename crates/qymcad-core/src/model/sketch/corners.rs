//! THE CORNERS OF A SKETCH: rounded and chamfered, one by one or all at once.

use super::*;

impl Project {
    /// Set the radius of an arc entity, moving its endpoints to radius `rr` while preserving their angles.
    /// PUT A RADIUS DIMENSION ON A PLAIN ARC: the radius set, then held by a driving dimension, as the radius of a
    /// circle is - one constraint more and one freedom less. Reported behaviour: the radius tool wrote 15 into the arc
    /// and left no dimension, the arc free to be dragged to any radius.
    pub fn put_arc_radius_dim(&mut self, si: usize, eid: Id, rr: f64) {
        let Some((center, a, b, ccw)) = self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
            EntityKind::Arc { center, a, b, ccw } => Some((center, a, b, ccw)),
            _ => None,
        }) else {
            return;
        };
        let rr = rr.max(0.01);
        self.set_arc_radius(si, eid, rr);
        if let Some(s) = self.sketches.get_mut(si) {
            if !s.constraints.iter().any(|c| matches!(c, Constraint::Diameter { c: cc, .. } if *cc == center)) {
                let off = arc_label_angle(&s.points, ArcEnds { center, a, b, ccw });
                s.constraints.push(Constraint::Diameter { c: center, d: rr, off, expr: String::new(), driven: false, diam: false, at: None });
            }
        }
        self.solve_sketch(si);
    }
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
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
    }
    /// The other endpoint of a segment (other than `exclude`) that uses point `pid`.
    pub(crate) fn line_other_end(&self, si: usize, pid: Id, exclude: Id) -> Option<Id> {
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
        let (Some((o1x, o1y)), Some((t1x, t1y)), Some((o2x, o2y)), Some((t2x, t2y))) = (self.point_xy(si, o1), self.point_xy(si, t1), self.point_xy(si, o2), self.point_xy(si, t2)) else {
            return false;
        };
        // The corner is the intersection of the lines o1 to t1 and o2 to t2.
        let Some((px, py)) = line_intersect_inf([o1x, o1y], [t1x, t1y], [o2x, o2y], [t2x, t2y]) else { return false };
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
    /// The same with a size given by a radius, a chord or an arc length (see `fillet_at_vertex_by`).
    pub fn fillet_lines_by(&mut self, si: usize, e1: Id, e2: Id, size: FilletSize) -> bool {
        let r = match size.by {
            FilletBy::Radius => size.value,
            _ => {
                let (Some((a1, b1)), Some((a2, b2))) = (self.line_ends(si, e1), self.line_ends(si, e2)) else { return false };
                let pc = if a1 == a2 || a1 == b2 {
                    a1
                } else if b1 == a2 || b1 == b2 {
                    b1
                } else {
                    return false;
                };
                let (o1, o2) = (if a1 == pc { b1 } else { a1 }, if a2 == pc { b2 } else { a2 });
                let (Some((px, py)), Some((ax, ay)), Some((bx, by))) = (self.point_xy(si, pc), self.point_xy(si, o1), self.point_xy(si, o2)) else { return false };
                let (la, lb) = ((ax - px).hypot(ay - py), (bx - px).hypot(by - py));
                if la < 1e-9 || lb < 1e-9 {
                    return false;
                }
                let corner = (((ax - px) * (bx - px) + (ay - py) * (by - py)) / (la * lb)).clamp(-1.0, 1.0).acos();
                let Some(r) = size.radius_on(std::f64::consts::PI - corner) else { return false };
                // what the lines take: the points of touching r / tan(corner / 2) from the corner, within the shorter line
                if r / (corner / 2.0).tan() >= la.min(lb) * 0.95 {
                    return false;
                }
                r
            }
        };
        if !self.fillet_lines(si, e1, e2, r) {
            return false;
        }
        if size.by != FilletBy::Radius {
            self.give_fillet_its_size(si, size);
        }
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
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false, at: None });
        }
        // The point of the corner (described in `settle_the_corner_point`): it goes with the corner, and what was
        // stated about it goes with it. What is left on each shortened line is a real point of it.
        self.settle_the_corner_point(si, pc, o1, t1, o2, t2);
        self.regen_sketch(si);
        true
    }
    /// WHAT HOLDS A LINE GOES WITH THE LINE when a fillet or a chamfer shortens it: a horizontal, vertical, parallel,
    /// perpendicular, collinear or equal of an edge's ends (o-pc) is the edge's own, and after the fillet the edge runs
    /// o-t. Left on the vanished vertex, the automatic constraints a person's defaults put on every corner kept that
    /// vertex alive as a point of its own - six points where the filleted corner has five - and a typed radius seemed
    /// to do nothing. A side that is not a line (`None`) keeps what it had.
    pub(crate) fn carry_edge_constraints(&mut self, si: usize, pc: Id, side1: Option<(Id, Id)>, side2: Option<(Id, Id)>) {
        let pair = |x: Id, y: Id| -> (Id, Id) {
            for (o, t) in [side1, side2].into_iter().flatten() {
                // a side the corner ate whole (radius equal to it) has no second point to carry onto
                if o != t && ((x == o && y == pc) || (x == pc && y == o)) {
                    return (o, t);
                }
            }
            (x, y)
        };
        // only a constraint naming the corner has a pair to carry: `pair` changes a pair one of whose points is `pc`. The
        // whole match for every constraint of the sketch was 2.3 s of filleting every corner of 2 000 rectangles.
        let names_corner = |c: &Constraint| match *c {
            Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Tangent { a, b, .. } | Constraint::PointOnLine { a, b, .. } => {
                a == pc || b == pc
            }
            Constraint::Equal { a, b, c, d } | Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Collinear { a, b, c, d } => [a, b, c, d].contains(&pc),
            _ => false,
        };
        if let Some(s) = self.sketches.get_mut(si) {
            for c in s.constraints.iter_mut().filter(|c| names_corner(c)) {
                match c {
                    // a tangency or a point held on a line names the line by two points too: rounding the next corner of
                    // a rectangle shortened a side whose tangency still ran to the old vertex, and that vertex was kept
                    // as a point nothing drew (14 or 15 points on a rectangle rounded all round, not 12)
                    Constraint::Horizontal { a, b }
                    | Constraint::Vertical { a, b }
                    | Constraint::Orientation { a, b, .. }
                    | Constraint::Tangent { a, b, .. }
                    | Constraint::PointOnLine { a, b, .. } => (*a, *b) = pair(*a, *b),
                    // AN EQUALITY IS A LENGTH: it is carried onto the pieces the cut leaves only where they are still
                    // equal - both sides of the corner shortened alike. Carried regardless, it said 55 = 58 for a chamfer
                    // of 3 on one of two equal sides of 58, and the solver bent everything tied to the drawing to satisfy
                    // it. Otherwise it stays on the corner, which then stays as the virtual sharp.
                    Constraint::Equal { a, b, c: cc, d } => {
                        let (ab, cd) = (pair(*a, *b), pair(*cc, *d));
                        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
                        let len = |(u, v): (Id, Id)| Some((at(u)?.0 - at(v)?.0).hypot(at(u)?.1 - at(v)?.1));
                        if let (Some(l1), Some(l2)) = (len(ab), len(cd)) {
                            if (l1 - l2).abs() <= 1e-9 * l1.max(l2).max(1.0) {
                                ((*a, *b), (*cc, *d)) = (ab, cd);
                            }
                        }
                    }
                    // a direction is the same along the whole line, so it is carried as it is
                    Constraint::Parallel { a, b, c: cc, d } | Constraint::Perpendicular { a, b, c: cc, d } | Constraint::Collinear { a, b, c: cc, d } => {
                        (*a, *b) = pair(*a, *b);
                        (*cc, *d) = pair(*cc, *d);
                    }
                    _ => {}
                }
            }
        }
    }
    /// THE POINT OF A CORNER THAT HAS BEEN TAKEN OFF: it stays, as the VIRTUAL SHARP on the extensions of both shortened
    /// lines - drawn and picked like any point, so a dimension or a constraint can be measured to the corner at any time.
    ///
    /// Where four lines met at the point and two of them have just been cut, the other two still end at it and the point
    /// stays their vertex - and the two cut lines are tied to it all the same, as to any sharp.
    pub(crate) fn settle_the_corner_point(&mut self, si: usize, pc: Id, o1: Id, t1: Id, o2: Id, t2: Id) {
        // THE TWO CUT LINES ARE TIED TO THE CORNER EVEN WHERE OTHER LINES STILL END AT IT. Skipped there, the first cut
        // of a cross left its two lines on nothing: their constraints still named the point, the lines no longer did,
        // and dragging the point swung them loose by 4 mm.
        self.carry_edge_constraints(si, pc, Some((o1, t1)), Some((o2, t2)));
        // THE CORNER STAYS, ALWAYS, as the virtual sharp on the extensions of both shortened lines: drawn and picked, so a
        // dimension or a constraint can be measured to it at any time, as in the professional systems. Kept only while
        // something stood on it, a chamfer left its sharp and a fillet did not, and a rectangle's were hidden: three
        // rules for one point.
        if let Some(s) = self.sketches.get_mut(si) {
            s.constraints.push(Constraint::PointOnLine { p: pc, a: o1, b: t1 });
            s.constraints.push(Constraint::PointOnLine { p: pc, a: o2, b: t2 });
        }
    }
    /// Whether any entity of the sketch stands on the point `pid` - an end of a line or an arc, a centre of one.
    pub fn point_used_by_geometry(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| {
            s.entities.iter().any(|e| match e.kind {
                EntityKind::Line { a, b } => a == pid || b == pid,
                EntityKind::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
                EntityKind::Circle { center, .. } => center == pid,
                EntityKind::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
            }) || s.splines.iter().any(|sp| sp.points.contains(&pid))
        })
    }
    /// HOW MANY CONSTRAINTS STATE SOMETHING ABOUT THE POINT `pid` - a dimension measured to a corner above all.
    pub fn constraints_on_point(&self, si: usize, pid: Id) -> usize {
        self.sketches.get(si).map(|s| s.constraints.iter().filter(|c| constraint_point_ids(c).contains(&pid)).count()).unwrap_or(0)
    }
    /// Chamfer between two segments sharing a vertex, with setback `d`.
    pub fn chamfer_lines(&mut self, si: usize, e1: Id, e2: Id, legs: ChamferLegs) -> bool {
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
        // the angle of the corner between the two lines
        let corner = (((ax - px) * (bx - px) + (ay - py) * (by - py)) / (la * lb)).clamp(-1.0, 1.0).acos();
        let Some((d1, d2)) = legs.along(corner) else { return false };
        // A LEG AS LONG AS ITS LINE, OR LONGER, IS REFUSED: pressed silently to 0.95 of the shorter line it was taken - a leg
        // of 300 on lines of 30 made a chamfer of 28.5 with no word, and so did one of 29.9
        if d1 >= la - 1e-9 || d2 >= lb - 1e-9 {
            return false;
        }
        let (t1x, t1y) = (px + (ax - px) / la * d1, py + (ay - py) / la * d1);
        let (t2x, t2y) = (px + (bx - px) / lb * d2, py + (by - py) / lb * d2);
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
            // THE CHAMFER KEEPS ITS SIZE AS DIMENSIONS, so it can be changed afterwards and the cut follows. A symmetric
            // one is sized by the cut itself, between its two ends, and held symmetric by its legs kept equal; two legs,
            // or a leg and the angle from the first line, are measured from the sharp corner. Whatever stands on the
            // corner keeps it as the virtual sharp below.
            // EVERY DIMENSION OF THE CHAMFER STANDS OUTSIDE THE SHAPE: the cut on the side of the sharp, a leg on the side
            // of its line away from the cut. Written as they came, the dimension of a chamfer on one corner of a triangle
            // dipped into the triangle, and one leg of two on a square stood inside it.
            let (sharp, at1, at2) = (Point2::new(px, py), Point2::new(t1x, t1y), Point2::new(t2x, t2y));
            let leg = |ends: [DimEnd; 2], side: Side, d: f64| {
                let [a, b] = ends_facing(ends, side);
                Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None }
            };
            let (end_c, end_1, end_2) = (DimEnd { id: pc, at: sharp }, DimEnd { id: t1, at: at1 }, DimEnd { id: t2, at: at2 });
            match legs.mode {
                crate::feature::ChamferMode::Symmetric => {
                    s.constraints.push(leg([end_1, end_2], Side::Toward(sharp), legs.first));
                    s.constraints.push(Constraint::Equal { a: pc, b: t1, c: pc, d: t2 });
                }
                crate::feature::ChamferMode::TwoDist => {
                    s.constraints.push(leg([end_c, end_1], Side::AwayFrom(at2), d1));
                    s.constraints.push(leg([end_c, end_2], Side::AwayFrom(at1), d2));
                }
                // the angle between the first line, run towards the corner, and the cut: what a drawing of the chamfer gives
                crate::feature::ChamferMode::DistAngle => {
                    s.constraints.push(leg([end_c, end_1], Side::AwayFrom(at2), d1));
                    // THE ARC OF THE ANGLE STANDS PAST THE END OF THE CUT, 1.25 of its length out, both sides carried to it:
                    // at the radius of every angle, 24 px, it was a tick tucked between the cut and the line
                    let off = 1.25 * (t2x - t1x).hypot(t2y - t1y);
                    s.constraints.push(Constraint::AngleLines { a: o1, b: t1, c: t1, d: t2, deg: legs.second, expr: String::new(), driven: false, off, at: None });
                }
            }
        }
        // Virtual corner: the vertex is held on the extensions of both lines, so dimensions to the corner stay
        // valid and the contour stays whole.
        self.settle_the_corner_point(si, pc, o1, t1, o2, t2);
        // SOLVED, not only drawn: the legs and the angle are laid as dimensions above, and a chamfer of two legs or of a
        // leg and an angle stands by them only once the sketch is solved - at the end of a batch of corners, once
        self.solve_sketch_held(si);
        true
    }
    /// Endpoints of an edge entity (a line or an arc), used to find the shared vertex when filleting.
    pub(crate) fn edge_end_ids(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entity(eid).and_then(|e| match e.kind {
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
        // A NAMED RECORD, not a four-place tuple of pairs: `(f64, (f64, f64), (f64, f64), (f64, f64))` needed
        // a comment beside it saying which pair was the centre and which the tangency points, and a comment is
        // not checked by anything.
        #[derive(Clone, Copy)]
        struct Candidate {
            /// How far the centre is from the click - smaller wins.
            score: f64,
            /// The fillet's centre.
            center: (f64, f64),
            /// Where it touches the first edge.
            t1: (f64, f64),
            /// Where it touches the second.
            t2: (f64, f64),
        }
        // Choose the centre: tangency valid on both edges, and nearest to the click.
        let mut best: Option<Candidate> = None;
        for (cx, cy) in centers {
            let (Some(t1), Some(t2)) = (tangent_pt(s1, cx, cy), tangent_pt(s2, cx, cy)) else { continue };
            // Both tangency points have to lie on the vertex side, between `pc` and `o`; `tangent_pt` checks
            // that for lines, while for an arc the radial projection is trusted. The score is the distance from
            // the centre to the click.
            let score = (cx - nearx).powi(2) + (cy - neary).powi(2);
            if best.is_none_or(|b| score < b.score) {
                best = Some(Candidate { score, center: (cx, cy), t1, t2 });
            }
        }
        let Some(Candidate { center: (cxx, cyy), t1: (t1x, t1y), t2: (t2x, t2y), .. }) = best else { return false };
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
            s.constraints.push(Constraint::Diameter { c: cen, d: r, off, expr: String::new(), driven: false, diam: false, at: None });
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
        // `PointOnCircle` - also where a third edge still ends at it and it stays that edge's vertex: the two cut
        // edges no longer end there, and untied they swing loose when the point is dragged.
        let side = |sup: &Sup, o: Id, t: Id| matches!(sup, Sup::Line { .. }).then_some((o, t));
        self.carry_edge_constraints(si, pc, side(&s1c, o1, t1), side(&s2c, o2, t2));
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
            s.entities
                .iter()
                .find_map(|e| match e.kind {
                    EntityKind::Circle { center, r } if center == cid => Some(r),
                    EntityKind::Arc { center, a, .. } if center == cid => match (p(center), p(a)) {
                        (Some((cx, cy)), Some((ax, ay))) => Some(((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or(0.0)
        };
        let (Some((x1, y1)), Some((x2, y2))) = (p(c1), p(c2)) else { return 0.0 };
        let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        dist + m1 as f64 * r_of(c1) + m2 as f64 * r_of(c2)
    }
    /// Edges (lines or arcs) for which point `pid` is an endpoint. Used by click-on-corner fillets and
    /// chamfers.
    pub fn vertex_edges(&self, si: usize, pid: Id) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.entities
            .iter()
            // construction lines are not edges of a corner: the diagonals of a rectangle end at its corners
            .filter(|e| !e.construction)
            .filter(|e| matches!(e.kind, EntityKind::Line { a, b } if a == pid || b == pid) || matches!(e.kind, EntityKind::Arc { a, b, .. } if a == pid || b == pid))
            .map(|e| e.id)
            .collect()
    }
    /// WHETHER AN EDGE STANDS AT A POINT, which is `vertex_edges` asked the other way round.
    ///
    /// Which of the lines of a point were named by a hand and which were let in by the point itself is decided by
    /// this: a line named by the hand takes its place among the lines of the point it arrives at.
    /// THE TWO POINTS AN EDGE STANDS ON, which is what the pairing of the chosen lines walks along.
    pub fn edge_ends(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        self.edge_end_ids(si, eid)
    }

    pub fn edge_stands_at(&self, si: usize, eid: Id, pid: Id) -> bool {
        self.edge_end_ids(si, eid).is_some_and(|(x, y)| x == pid || y == pid)
    }
    /// THE CORNER TWO EDGES SHARE: the point that is an end of both of them, when there is one.
    ///
    /// Two lines crossing without a point in common share no corner. That is the whole question the fillet and the
    /// chamfer ask of a chosen pair, and it is asked here once rather than at each place a pair is chosen.
    pub fn shared_vertex(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let (Some((a1, b1)), Some((a2, b2))) = (self.edge_end_ids(si, e1), self.edge_end_ids(si, e2)) else { return None };
        if a1 == a2 || a1 == b2 {
            Some(a1)
        } else if b1 == a2 || b1 == b2 {
            Some(b1)
        } else {
            None
        }
    }
    /// THE CORNER A NAMED PAIR OF EDGES MAKES: the point they meet at, where they make one.
    ///
    /// A shared point is not enough: two lines lying along ONE straight line through it share a vertex and no angle
    /// at all - there is nothing to round between them and nothing to cut. That pair names no corner, and a pair that
    /// names none is not an error: the search for a corner goes on with the second line as the first of the next pair.
    ///
    /// AN ARC IS TAKEN BY ITS TANGENT at the point, not by the chord to its far end: a line going on from a fillet along
    /// its tangent makes no corner either. Taken by the chord, it did: a click on the top of a rounded square by the end
    /// of its fillet named the joint a corner, and the fillet put on it stood its centre 8 above the square. The two
    /// are one straight within 1e-6 of a turn - what a solve leaves at a tangency.
    pub fn corner_of_pair(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let pid = self.shared_vertex(si, e1, e2)?;
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64)> {
            let kind = me.sketches.get(si)?.entities.iter().find(|e| e.id == eid)?.kind;
            let (dx, dy) = match kind {
                EntityKind::Arc { center, a, ccw, .. } => {
                    // the way along the arc out of the point: from its start the way it turns, from its end back
                    let (cx, cy) = me.point_xy(si, center)?;
                    let (rx, ry) = (pcx - cx, pcy - cy);
                    let left = (a == pid) == ccw;
                    if left {
                        (-ry, rx)
                    } else {
                        (ry, -rx)
                    }
                }
                _ => {
                    let (a, b) = me.edge_end_ids(si, eid)?;
                    let other = if a == pid { b } else { a };
                    let (ox, oy) = me.point_xy(si, other)?;
                    (ox - pcx, oy - pcy)
                }
            };
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then_some((dx / l, dy / l))
        };
        let (Some((x1, y1)), Some((x2, y2))) = (dir(self, e1), dir(self, e2)) else { return None };
        // parallel means one straight line: the angle between them is 180 degrees, and there is no corner in it
        ((x1 * y2 - y1 * x2).abs() > 1e-6).then_some(pid)
    }
    /// THE CORNERS A NAMED SET OF LINES MAKES: every pair of them that meets, two at a time, in the order they
    /// were named.
    ///
    /// **WHERE TWO OF THEM MEET THERE IS ONE CORNER**, and one pair is that corner — three lines of a triangle make
    /// three points and so three corners, which is the whole of a closed contour. A line that meets nothing of the
    /// set makes none: a line named on its own is half of the next corner, waiting for whatever answers it.
    ///
    /// **WHERE MORE THAN TWO OF THEM STAND ON ONE POINT** — a cross of four lines, a T-joint — every pair of them is
    /// a corner of that point, and taking all of them would round one place six times. So they are taken **as they
    /// were named, two at a time**: the first with the second, the third with the fourth, and a line left over alone
    /// stands there until another one is named beside it. That is what makes a third line joining a corner two lines
    /// have already made add nothing, and a fourth beside it make a second corner.
    ///
    /// The order is the rule, so it is the order the lines were named in and not the order a map hands them back:
    /// a set read off a hash would pair them by accident of the hashing, and the same drawing would round different
    /// corners on a second run.
    pub fn corners_of_lines(&self, si: usize, lines: &[Id]) -> Vec<(Id, Id)> {
        // THE LINES AS THEY WERE NAMED, one line named twice being one line: a line standing twice at the same
        // point would be paired with itself.
        let mut set: Vec<Id> = Vec::new();
        // a construction line is no side of a corner - the diagonal of a rectangle ends at its corners - so a picked one
        // takes no part in the pairing
        let construction = |me: &Self, l: Id| me.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == l && e.construction));
        for &l in lines {
            if !set.contains(&l) && !construction(self, l) {
                set.push(l);
            }
        }
        let here = |me: &Self, p: Id| -> Vec<usize> { (0..set.len()).filter(|&m| me.edge_end_ids(si, set[m]).is_some_and(|(x, y)| x == p || y == p)).collect() };
        let mut out: Vec<(Id, Id)> = Vec::new();
        let mut seen: Vec<Id> = Vec::new(); // the points already asked about, so a line asks about each of its ends once
        for &l in &set {
            let Some((x, y)) = self.edge_end_ids(si, l) else { continue };
            for p in [x, y] {
                if seen.contains(&p) {
                    continue;
                }
                seen.push(p);
                for two in here(self, p).chunks(2) {
                    // A PAIR LYING ALONG ONE STRAIGHT LINE SHARES ITS POINT AND MAKES NO ANGLE: it is not a corner,
                    // and naming it as one would offer a cut where there is nothing to cut.
                    if let [m, n] = two {
                        if self.corner_of_pair(si, set[*m], set[*n]).is_some() {
                            out.push((set[*m], set[*n]));
                        }
                    }
                }
            }
        }
        out
    }
    /// WHAT A CORNER WOULD LOOK LIKE IF THE VALUE WERE APPLIED: the point of the corner, where the two named
    /// edges will be cut, and the arc that will stand between them — a chamfer is a straight cut and has none.
    ///
    /// Nothing is changed here. The edges keep their ends and the point of the corner stays where it is: this is
    /// only what a person is shown before answering for it, drawn exactly as the operation will leave it — or not
    /// drawn at all when the value does not fit the corner.
    ///
    /// `None` when the pair is not a corner, when the value is not one, or when it is too big for the corner.
    pub fn corner_blend(&self, si: usize, pid: Id, (e1, e2): (Id, Id), cut: CornerCut) -> Option<CornerBlend> {
        // THE PAIR MUST BE THE CORNER: two lines that meet nowhere, or that lie along one straight line, are not a
        // corner and there is nothing to show where they are not.
        if self.corner_of_pair(si, e1, e2) != Some(pid) {
            return None;
        }
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // the direction away from the corner along each edge, and how long that edge is
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64, f64)> {
            let (a, b) = me.edge_end_ids(si, eid)?;
            let other = if a == pid { b } else { a };
            let (ox, oy) = me.point_xy(si, other)?;
            let (dx, dy) = (ox - pcx, oy - pcy);
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then_some((dx / l, dy / l, l))
        };
        let (Some((d1x, d1y, l1)), Some((d2x, d2y, l2))) = (dir(self, e1), dir(self, e2)) else { return None };
        let shorter = l1.min(l2);
        let theta = (d1x * d2x + d1y * d2y).clamp(-1.0, 1.0).acos();
        let at = |dx: f64, dy: f64, d: f64| [pcx + dx * d, pcy + dy * d];
        // THE MOST THE CORNER TAKES: a chamfer's legs short of the far end of each line, a radius whose touching points
        // stand at r / tan(theta / 2) from the corner for a fillet. Past this there is nothing to show.
        let value = match cut {
            CornerCut::Chamfer(legs) => {
                let (a, b) = legs.along(theta)?;
                if a >= l1 * (1.0 - 1e-9) || b >= l2 * (1.0 - 1e-9) {
                    return None;
                }
                return Some(CornerBlend { vertex: [pcx, pcy], ends: [at(d1x, d1y, a), at(d2x, d2y, b)], arc: None });
            }
            // the radius the size makes on this corner, whose arc turns through pi less the angle of its edges
            CornerCut::Fillet(size) => size.radius_on(std::f64::consts::PI - theta)?,
        };
        let limit = shorter * (theta / 2.0).tan();
        if value <= 1e-6 || !limit.is_finite() || value >= limit * (1.0 - 1e-9) {
            return None;
        }
        let half = (theta / 2.0).max(1e-6);
        let along = value / half.tan(); // where the arc touches each line
        let (bx, by) = (d1x + d2x, d1y + d2y);
        let bl = (bx * bx + by * by).sqrt();
        if bl < 1e-9 {
            return None;
        }
        let out = value / half.sin(); // the centre stands this far from the corner, along the bisector
        Some(CornerBlend { vertex: [pcx, pcy], ends: [at(d1x, d1y, along), at(d2x, d2y, along)], arc: Some(([pcx + bx / bl * out, pcy + by / bl * out], value)) })
    }
    /// THE TWO EDGES OF THE CORNER AT `pid`, out of every edge that meets there.
    ///
    /// Where exactly two edges meet, they are the corner and nothing is asked. Where more of them meet - two squares
    /// sharing a single point, four lines through it - there is a corner for every PAIR, and the pair is the two
    /// edges that bracket the point `near` (the cursor, in the drawing's own coordinates): the angle the cursor is
    /// standing in is the corner being pointed at. Without such a point (`None`) a vertex of more than two edges
    /// names no corner: which of them is meant is not knowable from the geometry alone, and the caller that only
    /// wants a bound asks `corner_limit`, which takes the tightest of them all.
    pub fn vertex_pair(&self, si: usize, pid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // each edge that meets at the vertex, with its direction AWAY from it
        let dirs: Vec<(Id, f64, f64)> = self
            .vertex_edges(si, pid)
            .into_iter()
            .filter_map(|eid| {
                let (a, b) = self.edge_end_ids(si, eid)?;
                let other = if a == pid { b } else { a };
                let (ox, oy) = self.point_xy(si, other)?;
                let (dx, dy) = (ox - pcx, oy - pcy);
                let l = (dx * dx + dy * dy).sqrt();
                (l > 1e-9).then_some((eid, dx / l, dy / l))
            })
            .collect();
        if dirs.len() < 2 {
            return None;
        }
        if dirs.len() == 2 {
            return self.corner_of_pair(si, dirs[0].0, dirs[1].0).map(|_| (dirs[0].0, dirs[1].0));
        }
        let (nx, ny) = near?;
        let (tx, ty) = (nx - pcx, ny - pcy);
        if (tx * tx + ty * ty).sqrt() < 1e-9 {
            return None; // the cursor stands ON the vertex: no side of it to read
        }
        // sorted by the angle of the direction; the pair that brackets the cursor is the one before it and the
        // one after it, round the circle
        let mut v: Vec<(Id, f64)> = dirs.iter().map(|(id, ux, uy)| (*id, uy.atan2(*ux))).collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.partition_point(|(_, a)| *a <= ty.atan2(tx)) % v.len();
        let pair = (v[(at + v.len() - 1) % v.len()].0, v[at].0);
        // the two sides bracketing the cursor are a corner unless they lie along one straight line
        self.corner_of_pair(si, pair.0, pair.1).map(|_| pair)
    }
    /// THE CORNER OF ONE NAMED EDGE AT `pid`, on the side the point `near` stands.
    ///
    /// Where several edges meet at one point every pair of them is a corner, and `vertex_pair` lets the cursor say
    /// which. Where the corner has been named by an edge AND the point together there are fewer corners to say it
    /// about - only the ones this edge takes part in - and the cursor says which of them by the sector it stands in:
    /// the two corners of an edge are the two sectors either side of it.
    ///
    /// Without a point to read (`near` is `None` - the pointer is not over the sheet, which happens while the value
    /// is being typed) the neighbour after it stands. The drawing has to begin somewhere, and a corner is as good a
    /// beginning as any; refusing outright would leave a field open with nothing behind it.
    pub fn vertex_pair_through(&self, si: usize, pid: Id, eid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // every edge at the point with the angle of its direction away from it, in the turn of the circle
        let mut v: Vec<(Id, f64)> = self
            .vertex_edges(si, pid)
            .into_iter()
            .filter_map(|e| {
                let (a, b) = self.edge_end_ids(si, e)?;
                let other = if a == pid { b } else { a };
                let (ox, oy) = self.point_xy(si, other)?;
                let (dx, dy) = (ox - pcx, oy - pcy);
                let l = (dx * dx + dy * dy).sqrt();
                (l > 1e-9).then_some((e, dy.atan2(dx)))
            })
            .collect();
        if v.len() < 2 {
            return None; // one edge at the point is not a corner of anything
        }
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.iter().position(|(id, _)| *id == eid)?;
        let len = v.len();
        // THE TWO SECTORS THIS EDGE BOUNDS, one on either side of it in the turn of the circle, each measured as the
        // turn from the edge named so that neither breaks where the angles run out
        let a0 = v[at].1;
        let up = {
            let x = v[(at + 1) % len].1;
            if x > a0 {
                x - a0
            } else {
                x + std::f64::consts::TAU - a0
            }
        };
        let down = {
            let x = v[(at + len - 1) % len].1;
            if x < a0 {
                a0 - x
            } else {
                a0 + std::f64::consts::TAU - x
            }
        };
        // THE SECTOR THE CURSOR STANDS IN, and where it stands in neither, the one it is nearer the middle of - the
        // pointer may be in a sector that belongs to two other edges altogether. The two are equally far apart when
        // the cursor is opposite the edge named, and the one after it in the turn of the circle stands then.
        let forward = near.is_none_or(|(nx, ny)| {
            let mut d = (ny - pcy).atan2(nx - pcx) - a0;
            if d < 0.0 {
                d += std::f64::consts::TAU;
            }
            (d - up / 2.0).abs() <= (d - (std::f64::consts::TAU - down / 2.0)).abs()
        });
        let other = if forward { v[(at + 1) % len].0 } else { v[(at + len - 1) % len].0 };
        let pair = (eid, other);
        self.corner_of_pair(si, pair.0, pair.1).map(|_| pair)
    }
    /// EVERY PAIR OF EDGES THAT COULD BE THE CORNER at `pid`, for the bounds that do not name a pair.
    pub fn vertex_pairs(&self, si: usize, pid: Id) -> Vec<(Id, Id)> {
        let edges = self.vertex_edges(si, pid);
        let mut out = Vec::new();
        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                out.push((edges[i], edges[j]));
            }
        }
        out
    }
    /// Chamfer the corner at vertex `pid`, where exactly two lines meet. Returns whether it succeeded.
    ///
    /// THE FIRST LINE is the one `toward` stands nearer to - the side of the corner that was clicked: the first leg is
    /// laid along it, and for a leg and an angle the angle is measured from it. With no point given, the line drawn
    /// first.
    pub fn chamfer_at_vertex(&mut self, si: usize, pid: Id, legs: ChamferLegs, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, None) else { return false };
        self.chamfer_lines_of_pair(si, pair, legs, toward)
    }
    /// Chamfer the corner at `pid` that the point `(x, y)` stands in, where more than two edges meet there.
    pub fn chamfer_at_vertex_near(&mut self, si: usize, pid: Id, legs: ChamferLegs, x: f64, y: f64, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, Some((x, y))) else { return false };
        self.chamfer_lines_of_pair(si, pair, legs, toward)
    }
    /// The cut of `legs` off the corner the two named edges make, both of them straight: the chamfer of a sketch is a
    /// straight cut, and an arc in the pair is refused rather than cut as if it were a line.
    pub fn chamfer_lines_of_pair(&mut self, si: usize, (e1, e2): (Id, Id), legs: ChamferLegs, toward: Option<Point2>) -> bool {
        if self.corner_of_pair(si, e1, e2).is_none() {
            return false; // two lines in one straight line: nothing is cut, and the cut would be along them
        }
        let both_lines = [e1, e2].iter().all(|&eid| matches!(self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).map(|e| e.kind), Some(EntityKind::Line { .. })));
        if !both_lines {
            return false;
        }
        // the line whose direction from the corner leans nearer to the click is the first
        let lean = |me: &Self, eid: Id| -> f64 {
            let Some(toward) = toward else { return 0.0 };
            let Some(shared) = me.corner_of_pair(si, e1, e2) else { return f64::MAX };
            let (Some((a, b)), Some((px, py))) = (me.line_ends(si, eid), me.point_xy(si, shared)) else { return f64::MAX };
            let far = if a == shared { b } else { a };
            let Some((fx, fy)) = me.point_xy(si, far) else { return f64::MAX };
            let (ux, uy, vx, vy) = (fx - px, fy - py, toward.x - px, toward.y - py);
            -(ux * vx + uy * vy) / (ux.hypot(uy) * vx.hypot(vy)).max(1e-12)
        };
        let (first, second) = if lean(self, e2) < lean(self, e1) { (e2, e1) } else { (e1, e2) };
        self.chamfer_lines(si, first, second, legs)
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
        self.fillet_all_corners_by(si, FilletSize::radius(r), only)
    }
    /// The same with a size given by a radius, a chord or an arc length: every corner gets the radius the size makes on
    /// it, and keeps the size as it was given (see `fillet_at_vertex_by`).
    pub fn fillet_all_corners_by(&mut self, si: usize, size: FilletSize, only: Option<&std::collections::HashSet<Id>>) -> usize {
        let corners: Vec<Id> = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let mut count: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
            for e in &s.entities {
                if e.construction || only.is_some_and(|f| !f.contains(&e.id)) {
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
        // every corner laid, the sketch rebuilt once (`batch`)
        self.batch(|p| {
            let mut done = 0;
            for pid in corners {
                // Is the vertex still intact, with two edges still meeting there?
                if p.sketches.get(si).is_some_and(|s| s.points.iter().any(|q| q.id == pid)) && p.vertex_edges(si, pid).len() == 2 && p.fillet_at_vertex_by(si, pid, size) {
                    done += 1;
                }
            }
            done
        })
    }
}
