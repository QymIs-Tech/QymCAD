//! A SKETCH EDITED AS A WHOLE: its rectangles, its curves deleted, moved, turned, scaled, copied, patterned.

use super::*;

impl Project {
    /// THE RECTANGLE WITH SIDE `side` in sketch `si`; none when the side is no rectangle's. Every question about a
    /// rectangle by one of its sides starts here.
    fn rect_of(&self, si: usize, side: Id) -> Option<crate::model::SketchRect> {
        self.sketches.get(si)?.rects().iter().find(|r| r.sides.contains(&side)).cloned()
    }
    /// THE DIMENSIONS THE RECTANGLE WITH SIDE `side` IS HELD BY, each where it has one: a dimension of a side, or of the
    /// side across from it, is its width or its height.
    fn rect_dims_found(&self, si: usize, side: Id) -> Option<RectDimsFound> {
        let s = self.sketches.get(si)?;
        let [c0, c1, c2, c3] = self.rect_of(si, side)?.corners;
        let find =
            |pairs: [[Id; 2]; 2]| s.constraints.iter().position(|c| matches!(*c, Constraint::Distance { a, b, axis: 0, .. } if pairs.iter().any(|&[x, y]| (a == x && b == y) || (a == y && b == x))));
        Some(RectDimsFound { width: find([[c0, c1], [c3, c2]]), height: find([[c1, c2], [c0, c3]]) })
    }
    /// THE WIDTH AND THE HEIGHT DIMENSIONS OF THE RECTANGLE WITH SIDE `side`, laid when it has none. `None` when the
    /// side is no rectangle's.
    pub fn rect_dims(&mut self, si: usize, side: Id) -> Option<crate::model::RectDims> {
        let found = self.rect_dims_found(si, side)?;
        if let (Some(width), Some(height)) = (found.width, found.height) {
            return Some(crate::model::RectDims { width, height });
        }
        // the one there is goes, and both are laid afresh beside each other
        if let Some(i) = found.width.or(found.height) {
            self.sketches.get_mut(si)?.constraints.remove(i);
        }
        self.dimension_rect(si, side);
        let n = self.sketches[si].constraints.len();
        Some(crate::model::RectDims { width: n - 2, height: n - 1 })
    }
    /// THE WIDTH AND THE HEIGHT DIMENSIONS THE RECTANGLE WITH SIDE `side` HOLDS, both of them; none while it holds fewer -
    /// nothing is laid. A double click on a rectangle opens its sizes by them, and lays them only once a size is typed.
    pub fn rect_dims_held(&self, si: usize, side: Id) -> Option<crate::model::RectDims> {
        let found = self.rect_dims_found(si, side)?;
        Some(crate::model::RectDims { width: found.width?, height: found.height? })
    }
    /// THE CORNERS OF THE RECTANGLE WITH SIDE `side`, in order round it; none when the side is no rectangle's.
    pub fn rect_corners(&self, si: usize, side: Id) -> Option<[Point2; 4]> {
        let s = self.sketches.get(si)?;
        let r = self.rect_of(si, side)?;
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        Some([at(r.corners[0])?, at(r.corners[1])?, at(r.corners[2])?, at(r.corners[3])?])
    }
    /// THE TWO POINTS A SIZE OF THE SLOT WITH CURVE `curve` IS MEASURED BETWEEN: the centres of its ends for its length,
    /// the two points its sides touch its first end at - twice its radius apart - for its width. `None` when the curve
    /// is no slot's.
    fn slot_size_ends(&self, si: usize, curve: Id, which: crate::model::SlotSize) -> Option<[Id; 2]> {
        let s = self.sketches.get(si)?;
        let f = s.figures.iter().find(|f| f.shape == FigureShape::Slot && f.curves.contains(&curve))?;
        let ends: Vec<[Id; 3]> = f.curves.iter().filter_map(|c| s.entity(*c)?.kind.as_arc().map(|arc| [arc.center, arc.a, arc.b])).collect();
        let [[c1, a, b], [c2, ..]] = ends[..] else { return None };
        Some(match which {
            crate::model::SlotSize::Length => [c1, c2],
            crate::model::SlotSize::Width => [a, b],
        })
    }
    /// A SIZE OF THE SLOT WITH CURVE `curve` as it stands.
    pub fn slot_size(&self, si: usize, curve: Id, which: crate::model::SlotSize) -> Option<f64> {
        let [a, b] = self.slot_size_ends(si, curve, which)?;
        let s = self.sketches.get(si)?;
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
        Some(at(a)?.dist(at(b)?))
    }
    /// A SIZE OF THE SLOT WITH CURVE `curve` SET TO `v`: its dimension laid when it has none, set when it has, and the
    /// sketch solved - the slot follows, its sides touching both ends. Answers whether the curve is a slot's.
    pub fn set_slot_size(&mut self, si: usize, curve: Id, which: crate::model::SlotSize, v: f64) -> bool {
        let Some([x, y]) = self.slot_size_ends(si, curve, which) else { return false };
        let held = self.slot_size_dim(si, curve, which);
        let s = &mut self.sketches[si];
        match held.and_then(|ci| s.constraints.get_mut(ci)) {
            Some(Constraint::Distance { d, .. }) => *d = v,
            _ => s.constraints.push(Constraint::Distance { a: x, b: y, d: v, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None }),
        }
        self.solve_sketch(si);
        true
    }
    /// THE PLACE AMONG THE CONSTRAINTS OF THE DIMENSION A SIZE OF THE SLOT WITH CURVE `curve` IS HELD BY: none while the
    /// size has no dimension, or the curve is no slot's. The box of a slot makes it a reference or names it by it.
    pub fn slot_size_dim(&self, si: usize, curve: Id, which: crate::model::SlotSize) -> Option<usize> {
        let [x, y] = self.slot_size_ends(si, curve, which)?;
        self.sketches.get(si)?.constraints.iter().position(|c| matches!(*c, Constraint::Distance { a, b, axis: 0, .. } if (a == x && b == y) || (a == y && b == x)))
    }
    /// THE WIDTH AND THE HEIGHT OF THE RECTANGLE WITH SIDE `side` LAID AS DIMENSIONS, at the lengths its first two sides
    /// stand at, each led out a fifth of the shorter side beyond its side (a positive offset is to the right of a side
    /// run from its first corner, and the corners go round anticlockwise - the dimensions stand outside). Returns
    /// whether the rectangle was found.
    pub fn dimension_rect(&mut self, si: usize, side: Id) -> bool {
        let (Some(r), Some([p0, p1, p2, _])) = (self.rect_of(si, side), self.rect_corners(si, side)) else { return false };
        let (w, h) = (p0.dist(p1), p1.dist(p2));
        let off = 0.2 * w.min(h);
        let dim = |a: Id, b: Id, d: f64| Constraint::Distance { a, b, d, off, expr: String::new(), driven: false, axis: 0, at: None };
        let s = &mut self.sketches[si];
        s.constraints.push(dim(r.corners[0], r.corners[1], w));
        s.constraints.push(dim(r.corners[1], r.corners[2], h));
        true
    }
    /// AN ANGLE DIMENSION ON A SIDE OF A RECTANGLE TAKES ITS TURN: called with the dimension before it is laid. The turns
    /// the rectangle holds itself by go, and its shape is held by parallel and square sides, so the dimension turns it -
    /// both held, the rectangle would be held twice over and the dimension refused.
    pub fn give_rect_turn_to(&mut self, si: usize, dim: &Constraint) {
        let Constraint::AngleLines { a, b, c, d, .. } = *dim else { return };
        let Some(s) = self.sketches.get_mut(si) else { return };
        let side_of = |s: &crate::model::Sketch, r: &crate::model::SketchRect, x: Id, y: Id| {
            r.sides.iter().any(|e| s.entity(*e).and_then(|q| q.kind.as_line()).is_some_and(|l| (l.a == x && l.b == y) || (l.a == y && l.b == x)))
        };
        let turned: Vec<crate::model::SketchRect> = s.rects().iter().filter(|r| side_of(s, r, a, b) || side_of(s, r, c, d)).cloned().collect();
        for r in turned {
            let held_by_turns = s.constraints.iter().any(|k| matches!(k, Constraint::Orientation { a, .. } if r.corners.contains(a)));
            if !held_by_turns {
                continue;
            }
            s.constraints.retain(|k| !(matches!(k, Constraint::Orientation { .. }) && r.holds(k)));
            let [c0, c1, c2, c3] = r.corners;
            s.constraints.extend(rect_free_constraints(c0, c1, c2, c3));
            // the dimensions of its sides go round with the sides the angle is about to turn
            let along = axis_dims_of_whole_sides(s, &|id| r.corners.contains(&id));
            lay_along_their_sides(s, &along, 0.0);
        }
    }
    /// AFTER A CONSTRAINT IS DELETED, the rectangles it was part of: one of a rectangle's own constraints deleted breaks
    /// it - the record, its centre, its diagonals and the rest of its own constraints go, four plain lines are left; an
    /// angle dimension that held a rectangle's turn deleted gives the turn back to the rectangle, at the turn it stands
    /// at, so a dragged corner does not turn it.
    pub(crate) fn settle_rects_after(&mut self, si: usize, removed: &Constraint) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let broken: Vec<crate::model::SketchRect> = s.rects().iter().filter(|r| r.holds(removed)).cloned().collect();
        for r in &broken {
            s.drop_rects(|k| k.id == r.id);
            s.constraints.retain(|k| !r.holds(k));
        }
        let diagonals: Vec<Id> = broken.iter().flat_map(|r| r.diagonals.into_iter().flatten()).collect();
        let free: Vec<crate::model::SketchRect> = s
            .rects()
            .iter()
            .filter(|r| !s.constraints.iter().any(|k| matches!(k, Constraint::Orientation { a, .. } if r.corners.contains(a))))
            .filter(|r| !s.constraints.iter().any(|k| matches!(k, Constraint::AngleLines { a, b, c, d, .. } if [a, b, c, d].iter().filter(|p| r.corners.contains(p)).count() >= 2)))
            .cloned()
            .collect();
        for r in free {
            let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
            let (Some(p0), Some(p1)) = (at(r.corners[0]), at(r.corners[1])) else { continue };
            let deg = (p1.1 - p0.1).atan2(p1.0 - p0.0).to_degrees();
            let [c0, c1, c2, c3] = r.corners;
            s.constraints.retain(|k| !(matches!(k, Constraint::Parallel { .. } | Constraint::Perpendicular { .. }) && r.holds(k)));
            s.constraints.extend(rect_own_constraints(c0, c1, c2, c3, r.centre, deg).into_iter().filter(|k| matches!(k, Constraint::Orientation { .. })));
        }
        let centres: Vec<Id> = broken.iter().map(|r| r.centre).collect();
        if !diagonals.is_empty() {
            self.delete_entities(si, &diagonals);
        }
        if let Some(s) = self.sketches.get_mut(si) {
            let used: std::collections::HashSet<Id> = s.constraints.iter().flat_map(constraint_point_ids).collect();
            s.points.retain(|q| !centres.contains(&q.id) || used.contains(&q.id));
        }
    }
    /// Delete constraint `ci` of a sketch. For a midpoint constraint the orphaned midpoint is pruned as well
    /// (nothing else uses it and it is not a system point), so no debris is left behind. The sketch is then
    /// re-solved.
    /// AN EDIT `op` OF CURVE `eid` THAT CUTS A PROJECTION: the projection parted one per edge (`part_projections_holding`),
    /// the edit done as on any curve, and when it is done the projection of the edge cut is let go - what is left of that
    /// edge is ordinary geometry, and its lines, with the pieces the edit made, get what they plainly have
    /// (`relate_lines`). The other edges stay projected and follow the body. An edit that did nothing leaves the edges
    /// projected. Reported behaviour: a piece of a projection trimmed, broken or deleted - what is left of it must stop
    /// being a projection at once, or the next rebuild of the body brings the piece back over it.
    pub(crate) fn cut_projection(&mut self, si: usize, eid: Id, op: impl FnOnce(&mut Self) -> bool) -> bool {
        if self.sketches.get(si).is_none_or(|s| !s.projections.iter().any(|p| p.entities.contains(&eid))) {
            return op(self);
        }
        let pids = self.part_projections_holding(si, &[eid]);
        let Some(s) = self.sketches.get(si) else { return false };
        let curves: Vec<Id> = s.projections.iter().filter(|p| pids.contains(&p.id)).flat_map(|p| p.entities.clone()).collect();
        let before: std::collections::HashSet<Id> = s.entities.iter().map(|e| e.id).collect();
        if !op(self) {
            return false;
        }
        let Some(s) = self.sketches.get_mut(si) else { return true };
        s.projections.retain(|p| !pids.contains(&p.id));
        let mut left: Vec<Id> = s.entities.iter().map(|e| e.id).filter(|id| curves.contains(id) || !before.contains(id)).collect();
        left.dedup();
        self.relate_lines(si, &left);
        self.solve_sketch(si);
        true
    }

    /// DELETE CURVES `eids` OF THE SKETCH, cutting what projections they belong to: parted one per edge, the projections
    /// of the edges deleted go with their curves, the other edges stay projected, and what is left of an edge cut is
    /// ordinary geometry held by what it plainly is.
    pub fn delete_sketch_curves(&mut self, si: usize, eids: &[Id]) {
        let pids = self.part_projections_holding(si, eids);
        let Some(s) = self.sketches.get(si) else { return };
        let left: Vec<Id> = s.projections.iter().filter(|p| pids.contains(&p.id)).flat_map(|p| p.entities.clone()).filter(|e| !eids.contains(e)).collect();
        if let Some(s) = self.sketches.get_mut(si) {
            s.projections.retain(|p| !pids.contains(&p.id));
        }
        self.delete_entities(si, eids);
        self.relate_lines(si, &left);
    }

    /// THE LINES A BROKEN RECTANGLE LEAVES GET WHAT THEY PLAINLY HAVE, as lines drawn by hand get it with auto-constraints
    /// on: of every rectangle of `before` the sketch no longer holds, the sides still standing are laid Horizontal or
    /// Vertical where they stand so, then Perpendicular and Parallel between them, then Equal between those of one
    /// length - each only where it constrains something (`add_constraint_if_independent`), so nothing is over-defined.
    /// Left as plain lines, a rectangle broken by deleting a side still stood level and square, held by nothing: a corner
    /// dragged pulled it out of shape. Answers how many constraints were laid.
    pub fn relate_lines_left(&mut self, si: usize, before: &[crate::model::SketchRect]) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let standing: std::collections::HashSet<Id> = s.rects().iter().map(|k| k.id).collect();
        let sides: Vec<Id> = before.iter().filter(|r| !standing.contains(&r.id)).flat_map(|r| r.sides).collect();
        self.relate_lines(si, &sides)
    }

    /// THE LINES `eids` GET WHAT THEY PLAINLY HAVE, as lines drawn by hand get it with auto-constraints on: Horizontal or
    /// Vertical where they stand so, then Perpendicular and Parallel between them, then Equal between those of one length -
    /// each only where it constrains something (`add_constraint_if_independent`). For the sides a broken rectangle leaves,
    /// and for what a cut projection leaves. Answers how many constraints were laid.
    pub fn relate_lines(&mut self, si: usize, eids: &[Id]) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let lines: Vec<(Id, Id)> = eids.iter().filter_map(|side| s.entity(*side)?.kind.as_line().map(|l| (l.a, l.b))).collect();
        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let dirs: Vec<Option<(f64, f64, f64)>> = lines
            .iter()
            .map(|&(a, b)| {
                let ((ax, ay), (bx, by)) = (at(a)?, at(b)?);
                let len = (bx - ax).hypot(by - ay);
                (len > 1e-9).then(|| ((bx - ax) / len, (by - ay) / len, len))
            })
            .collect();
        // a relation is read off geometry the rectangle held exactly, so the tolerance is the solver's, not the hand's
        const TOL: f64 = 1e-6;
        let mut wanted: Vec<Constraint> = Vec::new();
        for (&(a, b), d) in lines.iter().zip(&dirs) {
            let Some((ux, uy, _)) = *d else { continue };
            if uy.abs() < TOL {
                wanted.push(Constraint::Horizontal { a, b });
            } else if ux.abs() < TOL {
                wanted.push(Constraint::Vertical { a, b });
            }
        }
        for i in 0..lines.len() {
            for j in i + 1..lines.len() {
                let (Some((ux, uy, lu)), Some((vx, vy, lv))) = (dirs[i], dirs[j]) else { continue };
                let ((a, b), (c, d)) = (lines[i], lines[j]);
                if (ux * vx + uy * vy).abs() < TOL {
                    wanted.push(Constraint::Perpendicular { a, b, c, d });
                } else if (ux * vy - uy * vx).abs() < TOL {
                    wanted.push(Constraint::Parallel { a, b, c, d });
                }
                if (lu - lv).abs() < TOL * lu.max(lv) {
                    wanted.push(Constraint::Equal { a, b, c, d });
                }
            }
        }
        let laid = wanted.into_iter().filter(|c| self.add_constraint_if_independent(si, c.clone())).count();
        if laid > 0 {
            self.solve_sketch(si);
        }
        laid
    }
    pub fn delete_sketch_constraint(&mut self, si: usize, ci: usize) -> bool {
        let before: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects().clone()).unwrap_or_default();
        {
            let Some(s) = self.sketches.get_mut(si) else { return false };
            if ci >= s.constraints.len() {
                return false;
            }
            let removed = s.constraints.remove(ci);
            if let Constraint::Midpoint { p, .. } = &removed {
                let p = *p;
                let used_ent = s.entities.iter().any(|e| entity_points(e).contains(&p));
                let used_con = s.constraints.iter().any(|c| constraint_point_ids(c).contains(&p));
                if !used_ent && !used_con && !s.system_ids().contains(&p) {
                    s.points.retain(|q| q.id != p); // The orphaned midpoint is removed.
                }
            }
            self.settle_rects_after(si, &removed);
        }
        self.solve_sketch(si);
        // a rectangle broken by losing one of its own constraints leaves its sides what they plainly have
        self.relate_lines_left(si, &before);
        true
    }
    /// The points of `eids` an editing tool is allowed to shift: everything except what the sketch holds
    /// (the frame, the driven projections, the pinned points). See `Sketch::held_points`.
    pub(crate) fn movable_of(&self, si: usize, eids: &[Id]) -> Vec<Id> {
        let held = self.sketches.get(si).map(|s| s.held_points()).unwrap_or_default();
        let mut pts = self.entity_point_ids(si, eids);
        // A RECTANGLE TAKEN BY ALL ITS SIDES MOVES AS ONE SHAPE, with what its corners became: rounded or cut, a corner
        // is a virtual sharp no side ends at, and the arc or the cut between two sides is a piece of the same contour.
        // Left behind, the sharps and the arcs held the sides by their tangencies and dimensions, and Rotate of a
        // rectangle 45 x 35 rounded R3 all round was refused as held.
        let taken: Vec<crate::model::SketchRect> = self.sketches.get(si).map(|s| s.rects().iter().filter(|r| r.sides.iter().all(|e| eids.contains(e))).cloned().collect()).unwrap_or_default();
        for r in &taken {
            pts.extend(r.corners);
            let contour: Vec<Id> = self.connected_entities(si, r.sides[0]).into_iter().collect();
            pts.extend(self.entity_point_ids(si, &contour));
        }
        // A RECTANGLE MOVES AS ONE SHAPE: with all its corners its centre goes too. Left behind, the centre held the
        // middle of the diagonal back, and a rectangle moved by 5 came out moved by 4.
        pts.extend(self.whole_rects(si, &pts).iter().map(|r| r.centre));
        pts.sort_unstable();
        pts.dedup();
        pts.into_iter().filter(|id| !held.contains(id)).collect()
    }
    /// The rectangles of sketch `si` whose four corners are all among `pts`.
    pub(crate) fn whole_rects(&self, si: usize, pts: &[Id]) -> Vec<crate::model::SketchRect> {
        self.sketches.get(si).map(|s| s.rects().iter().filter(|r| r.corners.iter().all(|c| pts.contains(c))).cloned().collect()).unwrap_or_default()
    }

    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// THE TIES OF WHAT GOES TO WHAT STAYS ARE LIFTED by Move and Rotate: a relation between a point of `going` - every
    /// point of the shapes chosen, the held ones with them, so a shape keeps its own ties - and one
    /// not among them - the origin, an axis, a shape left where it was - is taken out, and what goes keeps the ties among
    /// its own points and every dimension. A shape drawn on the origin is held on it, and moved by the tool it leaves
    /// it, as it would with the relations not kept; dragged by the mouse it stays held. Reported behaviour: a circle drawn
    /// at zero could not be moved away from the origin.
    pub(crate) fn lift_ties_to_what_stays(&mut self, si: usize, going: &[Id]) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        s.constraints.retain(|c| {
            let tie = matches!(
                c,
                Constraint::Coincident { .. }
                    | Constraint::PointOnLine { .. }
                    | Constraint::Midpoint { .. }
                    | Constraint::PointOnCircle { .. }
                    | Constraint::Concentric { .. }
                    | Constraint::Tangent { .. }
                    | Constraint::CircleTangent { .. }
                    | Constraint::Symmetric { .. }
                    | Constraint::Collinear { .. }
                    | Constraint::Parallel { .. }
                    | Constraint::Perpendicular { .. }
                    | Constraint::Equal { .. }
                    | Constraint::EqualRadius { .. }
                    | Constraint::Horizontal { .. }
                    | Constraint::Vertical { .. }
            );
            let pts = c.points();
            !(tie && pts.iter().any(|p| going.contains(p)) && pts.iter().any(|p| !going.contains(p)))
        });
    }
    /// THE TIES THAT PUT `end` WHERE IT STANDS ARE LIFTED when Extend takes it elsewhere: on a point, on a line, at a
    /// middle, on a circle. Kept, they held the end where it was - a line ending on the Y axis did not reach the line
    /// it was extended to.
    pub(crate) fn lift_the_place_of(&mut self, si: usize, end: Id) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let ends_a_curve: std::collections::HashSet<Id> = s.entities.iter().flat_map(entity_points).collect();
        s.constraints.retain(|c| match *c {
            // a coincidence with the end of another curve is a corner, and stays
            Constraint::Coincident { a, b } => !((a == end && !ends_a_curve.contains(&b)) || (b == end && !ends_a_curve.contains(&a))),
            Constraint::PointOnLine { p, .. } | Constraint::Midpoint { p, .. } | Constraint::PointOnCircle { p, .. } => p != end,
            _ => true,
        });
    }
    /// Move the selected entities by a vector.
    pub fn move_entities(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64) {
        let pts = self.movable_of(si, eids);
        let chosen: Vec<Id> = pts.iter().copied().chain(self.entity_point_ids(si, eids)).collect();
        self.lift_ties_to_what_stays(si, &chosen);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// Rotate the selected entities about (cx, cy) by an angle in degrees.
    pub fn rotate_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, deg: f64) {
        let (sn, cs) = (deg.to_radians().sin(), deg.to_radians().cos());
        let pts = self.movable_of(si, eids);
        let chosen: Vec<Id> = pts.iter().copied().chain(self.entity_point_ids(si, eids)).collect();
        self.lift_ties_to_what_stays(si, &chosen);
        // THE ROTATE TOOL TURNS A RECTANGLE: the turn it holds itself by goes round with it. Kept, it pulled the
        // rectangle back to where it stood. It is the turn of every side whose two points go round - on the corners, or
        // on the points of touching a rounded corner carried it to, where a turn kept at 0 deg pulled a rectangle 45 x 35
        // rounded R3 back to the axes and flipped its arcs outward.
        if let Some(s) = self.sketches.get_mut(si) {
            for c in s.constraints.iter_mut() {
                if let Constraint::Orientation { a, b, deg: held } = c {
                    if pts.contains(a) && pts.contains(b) {
                        *held += deg;
                    }
                }
            }
            // read before the points go round: which dimensions measure a whole side along its axis
            let along = axis_dims_of_whole_sides(s, &|id| pts.contains(&id));
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    let (x, y) = (p.x - cx, p.y - cy);
                    p.x = cx + x * cs - y * sn;
                    p.y = cy + x * sn + y * cs;
                }
            }
            lay_along_their_sides(s, &along, deg);
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// THE EDIT ENDS AT THE SOLVER, not at a rebuild. `regen_sketch` recomputes the contours from whatever
    /// the coordinates now are; it does not check that the constraints still hold. An edit that stops there
    /// leaves a drawing standing where nothing allows it to stand, and the lie holds until the next edit
    /// puts it back.
    ///
    /// Reported behaviour: "I moved the circle far away with the move tool and the coincidence with the
    /// origin stayed - and stayed GREEN. Reopen the project and the circle is back at the centre." That was
    /// repaired at the call site in the interface, which left the method itself lying to the next caller;
    /// the rule lives here now.
    /// Scale the selected entities about (cx, cy).
    pub fn scale_entities(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, f: f64) {
        let f = if f.abs() < 1e-6 { 1.0 } else { f };
        let pts = self.movable_of(si, eids);
        if let Some(s) = self.sketches.get_mut(si) {
            for p in s.points.iter_mut() {
                if pts.contains(&p.id) {
                    p.x = cx + (p.x - cx) * f;
                    p.y = cy + (p.y - cy) * f;
                }
            }
            for e in s.entities.iter_mut() {
                if eids.contains(&e.id) {
                    if let Some(c) = e.kind.as_circle() {
                        e.kind.set_radius(c.r * f.abs());
                    }
                }
            }
        }
        self.solve_sketch(si); // regenerates as well, on the positions the constraints allow
    }
    /// Constraints internal to the point set `inside`: every reference that stays within the set, except
    /// `Fixed` — an absolute-position anchor is not copied, since the copy is placed elsewhere. References to
    /// the axes, the origin or outside points are filtered out automatically, their points not being in the
    /// set. The clones carry the old ids.
    pub(crate) fn internal_constraints(&self, si: usize, inside: &std::collections::HashSet<Id>, curves: &[Id]) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.constraints
            .iter()
            .filter(|c| {
                if matches!(c, Constraint::Fixed { .. }) {
                    return false;
                }
                // on the points of the set and on its curves - a spline is referred to by its id
                let (ps, cs) = (constraint_point_ids(c), c.curve_ids());
                (!ps.is_empty() || !cs.is_empty()) && ps.iter().all(|p| inside.contains(p)) && cs.iter().all(|e| curves.contains(e))
            })
            .cloned()
            .collect()
    }
    /// Duplicate entities with point transform `f`. `with_constraints` selects whether the constraints and
    /// dimensions internal to the set are carried over (horizontals, verticals, edge dimensions and so on, but
    /// no `Fixed` and no references to the axes). Returns the new ids.
    pub(crate) fn dup_entities<F: Fn(f64, f64) -> (f64, f64)>(&mut self, si: usize, eids: &[Id], f: F, with_constraints: bool) -> Vec<Id> {
        let new_ids = self.dup_entities_unbuilt(si, eids, f, with_constraints);
        self.regen_sketch(si);
        new_ids
    }
    /// `dup_entities` without the loops made again after it: for a pattern, which lays every copy and makes the loops
    /// once. Made again after each copy, a pattern of 200 lines across 200 others made a growing grid 199 times - 29 s,
    /// the last grid of 40 000 cells.
    pub(crate) fn dup_entities_unbuilt<F: Fn(f64, f64) -> (f64, f64)>(&mut self, si: usize, eids: &[Id], f: F, with_constraints: bool) -> Vec<Id> {
        // a rectangle, a polygon, a slot taken whole is copied as one, a rectangle with its diagonals and its centre
        let whole = self.wholes_among(si, eids);
        let mut taken: Vec<Id> = eids.to_vec();
        taken.extend(whole.carried().filter(|id| !eids.contains(id)).collect::<Vec<Id>>());
        let eids_taken = &taken[..];
        let pids = self.entity_point_ids(si, eids_taken);
        let coords: Vec<(Id, f64, f64)> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect()
        };
        let ents: Vec<SketchEntity> = {
            let Some(s) = self.sketches.get(si) else { return Vec::new() };
            s.entities.iter().filter(|e| eids_taken.contains(&e.id)).cloned().collect()
        };
        let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        let mut curves: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        let stood: std::collections::HashMap<Id, Point2> = coords.iter().map(|&(id, x, y)| (id, Point2::new(x, y))).collect();
        let was = |id: Id| stood.get(&id).copied();
        for (old, x, y) in coords {
            let (nx, ny) = f(x, y);
            let nid = self.alloc_id();
            self.sketches[si].points.push(SketchPoint { id: nid, x: nx, y: ny });
            map.insert(old, nid);
        }
        let m = |id: Id| map.get(&id).copied().unwrap_or(id);
        // A REFLECTION TURNS THE WAY ROUND OF AN ARC: copied with its winding, a mirrored arc ran round the other side of
        // its circle between the mirrored ends. Read off the map itself - a unit square taken through it comes out
        // turned over - so a move, a turn and a point mirror keep the winding and a line mirror flips it.
        let ((ox, oy), (ex, ey), (fx, fy)) = (f(0.0, 0.0), f(1.0, 0.0), f(0.0, 1.0));
        let flips = (ex - ox) * (fy - oy) - (ey - oy) * (fx - ox) < 0.0;
        let mut new_ids = Vec::new();
        // the tangents of a spline go through the map without its shift: along X to where X went, along Y to where Y went
        let along = |[x, y]: [f64; 2]| [x * (ex - ox) + y * (fx - ox), x * (ey - oy) + y * (fy - oy)];
        let kind_map = KindMap { point: &m, turn: &along, facing: if flips { Facing::Flipped } else { Facing::Kept } };
        for e in ents {
            let nk = e.kind.copied(&kind_map);
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            curves.insert(e.id, id);
            new_ids.push(id);
        }
        // a point named among the set is a shape of its own, and its copy is one of the new shapes
        new_ids.extend(eids.iter().filter_map(|id| map.get(id).copied()));
        if with_constraints {
            // The internal constraints of the set are re-pointed at the new points, so the copy keeps its
            // shape but not its anchor or its links to the axes.
            let inside: std::collections::HashSet<Id> = map.keys().copied().collect();
            let maps = CopyMaps { points: &map, curves: &curves, turn: &along, was: &was };
            let cons: Vec<Constraint> = self.internal_constraints(si, &inside, eids_taken).iter().filter_map(|c| copied_constraint(c, &maps)).collect();
            self.sketches[si].constraints.extend(cons);
        }
        let maps = CopyMaps { points: &map, curves: &curves, turn: &along, was: &was };
        if !with_constraints {
            let own = self.copied_own_constraints(si, &whole, &maps);
            self.sketches[si].constraints.extend(own);
        }
        self.lay_copied_records(si, &whole, &maps);
        new_ids
    }
    /// Capture the selected geometry of sketch `si` into the clipboard, for pasting into this sketch or
    /// another one. `eids` are the selected entities; the snapshot holds them and all their points (centres,
    /// endpoints). `(rx, ry)` is the reference point given at copy time. Does not mutate.
    pub fn copy_sketch_geometry(&self, si: usize, eids: &[Id], rx: f64, ry: f64) -> GeomClip {
        // a rectangle, a polygon, a slot taken whole goes as one, a rectangle with its diagonals and its centre
        let whole = self.wholes_among(si, eids);
        let taken: Vec<Id> = eids.iter().copied().chain(whole.carried().filter(|id| !eids.contains(id))).collect();
        let pids = self.entity_point_ids(si, &taken);
        let Some(s) = self.sketches.get(si) else { return GeomClip::default() };
        let points: Vec<(Id, f64, f64)> = s.points.iter().filter(|p| pids.contains(&p.id)).map(|p| (p.id, p.x, p.y)).collect();
        let entities: Vec<SketchEntity> = s.entities.iter().filter(|e| taken.contains(&e.id)).cloned().collect();
        let inside: std::collections::HashSet<Id> = pids.iter().copied().collect();
        let constraints = self.internal_constraints(si, &inside, &taken); // Internal constraints and dimensions, without
                                                                          // `Fixed` or axis references.
        GeomClip { points, entities, constraints, ref_x: rx, ref_y: ry, whole }
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
        let mut curves: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
        // the clipboard is only shifted: the tangents of a spline stay as they are
        let kind_map = KindMap { point: &m, turn: &|v| v, facing: Facing::Kept };
        for e in &clip.entities {
            let nk = e.kind.copied(&kind_map);
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: nk, construction: e.construction });
            curves.insert(e.id, id);
            new_ids.push(id);
        }
        // The internal constraints and dimensions of the copy are re-pointed at the new points; every
        // reference stays within the set and is remapped through the map.
        let was = |id: Id| clip.points.iter().find(|q| q.0 == id).map(|q| Point2::new(q.1, q.2));
        let maps = CopyMaps { points: &map, curves: &curves, turn: &|v| v, was: &was };
        for c in &clip.constraints {
            if let Some(nc) = copied_constraint(c, &maps) {
                self.sketches[si].constraints.push(nc);
            }
        }
        self.lay_copied_records(si, &clip.whole, &maps);
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
    /// Attach cut point `pid` to the curve it actually lies on, of whatever kind (`point_on`) - excluding the ones already
    /// holding it as an endpoint or a centre.
    ///
    /// Without it the cut point is free (along the angle on an arc, along the crossing entity on a line) and
    /// drifts off the intersection on the next solve, so the trimmed geometry wanders and twists and the
    /// dimensions and radii move with it. Attaching it to the crossing entity, together with the intrinsic
    /// constraint that keeps it on its own curve, pins the cut point to the intersection.
    pub(crate) fn anchor_cut_point(&mut self, si: usize, pid: Id) {
        const TOL: f64 = 5e-3;
        let Some((px, py)) = self.point_xy(si, pid) else { return };
        let Some(s) = self.sketches.get(si) else { return };
        let at = Point2::new(px, py);
        // the first curve it lies on - within the curve as drawn, not on its line or circle past its ends - other than
        // those it is a point of
        let anchor = s
            .entities
            .iter()
            .filter(|e| !entity_points(e).contains(&pid))
            .find(|e| crate::model::curve::Curve::of(s, e).is_some_and(|c| c.nearest(at).off < TOL))
            .map(|e| crate::model::point_on(e, pid));
        if let Some(c) = anchor {
            let same = |x: &Constraint| {
                std::mem::discriminant(x) == std::mem::discriminant(&c) && {
                    let (mut u, mut v) = (constraint_point_ids(x), constraint_point_ids(&c));
                    u.sort_unstable();
                    v.sort_unstable();
                    u == v
                }
            };
            if !self.sketches[si].constraints.iter().any(same) {
                self.sketches[si].constraints.push(c);
            }
        }
    }
}

/// THE DIMENSIONS A RECTANGLE IS HELD BY, each where it has one: of its width, of its height.
struct RectDimsFound {
    width: Option<usize>,
    height: Option<usize>,
}
