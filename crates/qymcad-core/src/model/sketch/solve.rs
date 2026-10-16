//! THE SOLVE OF A SKETCH: the values of its dimensions, its residuals and conflicts, its rebuild.

use super::*;

impl Project {
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
                        let vars: std::collections::HashMap<String, f64> = self.parameters.iter().filter(|p| !p.name.is_empty()).map(|p| (p.name.clone(), p.value)).collect();
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
        let mut out = s.checked.with(|memo| crate::solver::conflicts_remembered(&s.points, &radii, &all, memo));
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
    pub(crate) fn solve_sketch_inner(&mut self, si: usize, drag: Option<(Id, f64, f64)>, budget: crate::solver::Budget, rebuild: Rebuild) -> f64 {
        // THE TIME OF A SOLVE COUNTS FROM HERE: the work before the solver is inside it, and each call of the solver gets
        // what is left - a held rectangle's second call no longer starts a clock of its own
        let started = std::time::Instant::now();
        let left = |b: crate::solver::Budget| crate::solver::Budget { time: b.time.map(|t| t.saturating_sub(started.elapsed())), ..b };
        // The radius variables and the implicit arc constraints are computed before the mutable borrow.
        let mut radii = self.entity_radii(si);
        let intrinsics = self.entity_intrinsics(si);
        let Some(s) = self.sketches.get_mut(si) else { return 0.0 };
        // where the points stood before anything of this solve moved them, for what it rebuilds after it
        let at_start: Vec<(f64, f64)> = s.points.iter().map(|p| (p.x, p.y)).collect();
        // THE CENTRE OF A RECTANGLE DRAWN BY ITS CORNERS FOLLOWS THE CORNERS: it is put on the middle of the diagonal
        // before the solve. Left where it stood, it held the corners back - a side moved from 20 to 30 came out at 28.9,
        // the solver sharing the move between the corners and the centre. A centre a rectangle was drawn from is its
        // anchor and stays.
        struct Mid {
            centre: Id,
            at: Point2,
        }
        // how many constraints name each centre, and where each point stands, counted once: asked of every constraint for
        // every rectangle and looked along the points, 10 000 rectangles drawn were 6e8 steps a solve
        let corner_drawn: Vec<&crate::model::SketchRect> = s.rects.iter().filter(|r| matches!(r.anchor, crate::model::RectAnchor::Corner(_))).collect();
        let mut naming: std::collections::HashMap<Id, usize> = corner_drawn.iter().map(|r| (r.centre, 0)).collect();
        if !naming.is_empty() {
            for c in &s.constraints {
                for p in c.points() {
                    if let Some(n) = naming.get_mut(&p) {
                        *n += 1;
                    }
                }
            }
        }
        let standing: std::collections::HashMap<Id, Point2> = if corner_drawn.is_empty() { Default::default() } else { s.points.iter().map(|q| (q.id, Point2::new(q.x, q.y))).collect() };
        let mids: Vec<Mid> = corner_drawn
            .into_iter()
            // only a centre nothing else holds: one that carries a constraint of its own is solved with it, and put back on
            // the middle before every solve of a contradicting sketch it was a new compromise each time - a point drifted
            // by 1 mm from one solve to the next
            .filter(|r| naming.get(&r.centre) == Some(&1))
            .filter_map(|r| {
                let at = |id: Id| standing.get(&id).copied();
                let (a, c) = (at(r.corners[0])?, at(r.corners[2])?);
                Some(Mid { centre: r.centre, at: Point2::new((a.x + c.x) / 2.0, (a.y + c.y) / 2.0) })
            })
            .collect();
        if !mids.is_empty() {
            let place: std::collections::HashMap<Id, usize> = s.points.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
            for m in mids {
                if let Some(&i) = place.get(&m.centre) {
                    (s.points[i].x, s.points[i].y) = (m.at.x, m.at.y);
                }
            }
        }
        // A SHAPE DRAGGED BY ITS CENTRE GOES WITH IT AS A WHOLE, as a circle goes with its centre: what the centre
        // carries (`Sketch::carried_with` - the corners of a rectangle, the ends of an arc, a slot, the axis ends of an
        // ellipse) is moved by the move of the centre before the solve. Left where they stood, the drag pulled the
        // centre alone: the middle of a rectangle's diagonal pulled it back by the two corners on it, and an ellipse
        // turned and changed shape about its axis ends.
        if let Some((d, tx, ty)) = drag {
            let carried: Vec<Id> = s.carried_with(d);
            if let Some((cx, cy)) = s.points.iter().find(|q| q.id == d).map(|q| (q.x, q.y)) {
                if !carried.is_empty() {
                    let held = s.held_points();
                    let (dx, dy) = (tx - cx, ty - cy);
                    for q in s.points.iter_mut().filter(|q| (carried.contains(&q.id) || q.id == d) && !held.contains(&q.id)) {
                        q.x += dx;
                        q.y += dy;
                    }
                }
            }
        }
        // Reference (driven) dimensions do not constrain the geometry and are excluded from the solver, while
        // the arc intrinsics (endpoints on the circle of radius R) are always active.
        let mut active: Vec<Constraint> = s.constraints.iter().filter(|c| !c.is_driven()).cloned().collect();
        active.extend(intrinsics);
        let (was, radii_was): (Vec<(f64, f64)>, Vec<f64>) = (s.points.iter().map(|p| (p.x, p.y)).collect(), radii.iter().map(|r| r.value).collect());
        // A RECTANGLE CHANGES ITS SIZE FROM WHERE IT WAS DRAWN: from the centre a rectangle drawn from the centre, from the
        // first corner one drawn by its corners. The solver alone spreads a change over every point it may move, so a
        // dragged corner or a width typed grew a rectangle about wherever the least travel lay. What is held for this
        // solve (`held_for_size`) is let go when the sketch does not solve with it - a dimension that moves the
        // rectangle as a whole.
        // A CORNER DRAGGED OF A RECTANGLE DRAWN FROM A CORNER asks whether its centre is held - counted only then, as a
        // drag frame of a sketch the drag session does not take is solved here
        let dragged = drag.map(|(d, _, _)| d);
        let corner_dragged = dragged.is_some_and(|d| s.rects.iter().any(|r| matches!(r.anchor, crate::model::RectAnchor::Corner(_)) && r.corners.contains(&d)));
        let free: std::collections::HashMap<Id, bool> = if corner_dragged {
            s.points.iter().map(|q| q.id).zip(crate::solver::free_points(&s.points, &radii, &active)).collect()
        } else {
            std::collections::HashMap::new()
        };
        let centre_of = |r: &crate::model::SketchRect| if free.get(&r.centre) == Some(&false) { Centre::Held } else { Centre::Free };
        let anchors: Vec<Constraint> =
            s.rects.iter().filter_map(|r| held_for_size(r, dragged, centre_of(r))).filter(|p| s.points.iter().any(|q| q.id == *p)).map(|p| Constraint::Fixed { p }).collect();
        let outcome = if anchors.is_empty() {
            crate::solver::solve_within(&mut s.points, &mut radii, &active, drag, left(budget))
        } else {
            let (mut held_points, mut held_radii) = (s.points.clone(), radii.clone());
            let held: Vec<Constraint> = active.iter().cloned().chain(anchors).collect();
            let with_hold = crate::solver::solve_within(&mut held_points, &mut held_radii, &held, drag, left(budget));
            // SOLVED IS TOLD BY THE SKETCH'S OWN CONSTRAINTS, not by the hold, a soft pull of which a little is always left
            // under a corner dragged away from it. Solved means to the precision the polish reaches (1e-12): a side held
            // 0.0009 short of a collinear line by its anchor left 1e-7 and passed at 1e-6. A DRAG FRAME KEEPS THE HOLD
            // whatever it leaves: a frame is a compromise with the pointer until the release solves it, and the
            // compromise grows with how far the pointer is from where the corner can go - a corner pulled 8 mm up off
            // its horizontal side left 2e-2, and a frame let go of the hold carried the corner drawn from 7 mm with it.
            // The release is solved in full, and lets the hold go if the sketch does not solve with it.
            let r = crate::solver::residual_per_constraint(&held_points, &held_radii, &active).iter().map(|v| v * v).sum::<f64>().sqrt();
            let solved = if drag.is_some() { f64::INFINITY } else { 1e-9 };
            if r <= solved {
                (s.points, radii) = (held_points, held_radii);
                crate::solver::Outcome { residual: r, left: with_hold.left }
            } else {
                crate::solver::solve_within(&mut s.points, &mut radii, &active, drag, left(budget))
            }
        };
        // NOT SOLVED TO THE END: the parts the time ran out before, said beside the degrees of freedom of the sketch
        s.left_unsolved = outcome.left;
        let resid = outcome.residual;
        // A SOLVED SKETCH SOLVED AGAIN STAYS AS IT WAS: a move below rounding is not written. Each solve of a solved
        // polygon shifted its points by about 2e-18 mm, the document key read every frame as an edit, and a rebuild
        // in the background always came back stale and was started again - the program never came to rest.
        let same = |new: f64, old: f64| (new - old).abs() <= 1e-12 * old.abs().max(1.0);
        for (p, &(x, y)) in s.points.iter_mut().zip(&was) {
            if same(p.x, x) && same(p.y, y) {
                (p.x, p.y) = (x, y);
            }
        }
        for (r, &old) in radii.iter_mut().zip(&radii_was) {
            if same(r.value, old) {
                r.value = old;
            }
        }
        // The solved radii go back into the circles; an arc derives its radius from its points and stores
        // none. By the centre, from a table: every radius against every entity was 4.9e9 steps a solve on 70 000
        // circles.
        let solved: std::collections::HashMap<Id, f64> = radii.iter().map(|rv| (rv.center, rv.value)).collect();
        for e in s.entities.iter_mut() {
            if let EntityKind::Circle { center, r } = &mut e.kind {
                if let Some(&v) = solved.get(center) {
                    *r = v;
                }
            }
        }
        // what the solve moved: the points and the centres of the circles whose radius changed
        let moved: std::collections::HashSet<Id> = s
            .points
            .iter()
            .zip(&at_start)
            .filter(|(p, &(x, y))| p.x.to_bits() != x.to_bits() || p.y.to_bits() != y.to_bits())
            .map(|(p, _)| p.id)
            .chain(radii.iter().zip(&radii_was).filter(|(r, &old)| r.value.to_bits() != old.to_bits()).map(|(r, _)| r.center))
            .collect();
        self.update_driven_dims(si);
        match rebuild {
            Rebuild::Whole => self.regen_sketch(si),
            Rebuild::Moved => self.regen_sketch_moved(si, &moved),
        }
        resid
    }
    /// Radius variables of the solver: circles (which store `r`) and arcs (whose radius is the distance from
    /// the centre to endpoint `a`), keyed by centre id. This makes the radius a real variable for degrees of
    /// freedom, tangency and equal-radius constraints.
    pub(crate) fn entity_radii(&self, si: usize) -> Vec<crate::solver::RadiusVar> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        let mut out = Vec::new();
        for e in &s.entities {
            match e.kind {
                EntityKind::Circle { center, r } => out.push(crate::solver::RadiusVar { center, value: r }),
                EntityKind::Arc { center, a, .. } => {
                    if let (Some(&(cx, cy)), Some(&(ax, ay))) = (pos.get(&center), pos.get(&a)) {
                        // `hypot`, as the drag session reads it (`drag_session`): the root of the squares rounds the last
                        // bit apart from it, and a frame kept by the drag and the frame solved alone came out different
                        let rr = (ax - cx).hypot(ay - cy).max(0.001);
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
    pub(crate) fn entity_intrinsics(&self, si: usize) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out = Vec::new();
        let projected = s.projected_entities();
        // THE END OF A FILLET WHERE ITS LINE ENDS: the line touches the arc (`Tangent`) and ends at the arc's end. On the
        // circle, the row of that end measures the radius along the normal of the line, as the row of the tangency
        // does, and the rank took one of the two for redundant: every rounded corner showed one redundant constraint
        // and one freedom too many - 12 on a drawing of three rectangles with six fillets. The end is held instead by
        // the radius to it standing square to the line: the centre at the distance r from the line, its foot on the line
        // at the end - the end is on the circle all the same, and the two rows look across and along the line.
        let touched_at = |centre: Id, end: Id| -> Option<(Id, Id)> {
            s.constraints.iter().find_map(|c| match *c {
                Constraint::Tangent { a, b, c, .. } if c == centre && (a == end || b == end) => Some((a, b)),
                _ => None,
            })
        };
        let on_the_arc = |centre: Id, end: Id| match touched_at(centre, end) {
            Some((a, b)) => Constraint::Perpendicular { a: centre, b: end, c: a, d: b },
            None => Constraint::PointOnCircle { p: end, c: centre },
        };
        for e in &s.entities {
            match e.kind {
                // an arc of a projection has its centre and both ends pinned by the body: one end on the circle sets the
                // radius, the other would say it again - a fillet of the body projected showed one redundant
                EntityKind::Arc { center, a, .. } if projected.contains(&e.id) => {
                    out.push(Constraint::PointOnCircle { p: a, c: center });
                }
                EntityKind::Arc { center, a, b, .. } => {
                    out.push(on_the_arc(center, a));
                    out.push(on_the_arc(center, b));
                }
                EntityKind::Ellipse { c, ma, mi } => {
                    out.push(Constraint::Perpendicular { a: c, b: ma, c, d: mi });
                }
                _ => {}
            }
        }
        // Projected geometry is driven by the body: its points and circle radii are given, not unknowns.
        // Without this a projected edge counted 4 degrees of freedom and a box face outline 8, and a solve
        // was free to drag the projection off the part.
        // A shared corner occupies several slots of `points`; it is pinned once, or the duplicates would
        // count as redundant constraints (8 on a box face).
        let mut pinned = std::collections::HashSet::new();
        for pr in &s.projections {
            out.extend(pr.points.iter().filter(|&&p| pinned.insert(p)).map(|&p| Constraint::Fixed { p }));
            for e in s.entities.iter().filter(|e| pr.entities.contains(&e.id)) {
                if let EntityKind::Circle { center, r } = e.kind {
                    out.push(Constraint::Diameter { c: center, d: r, off: 0.0, expr: String::new(), driven: false, diam: false, at: None });
                }
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
        s.constraints.push(Constraint::Diameter { c, d, off: 0.0, expr: String::new(), driven: false, diam, at: None });
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
    pub(crate) fn update_driven_dims(&mut self, si: usize) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let pos: std::collections::HashMap<Id, (f64, f64)> = s.points.iter().map(|p| (p.id, (p.x, p.y))).collect();
        // Circle radii by centre, for reference diameter and radius dimensions.
        let crad: std::collections::HashMap<Id, f64> = s
            .entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Circle { center, r } => Some((center, r)),
                _ => None,
            })
            .collect();
        for c in &mut s.constraints {
            measure_driven(c, &pos, &crad);
        }
    }
    /// Whether a sketch is typed: built from points the sketcher edits - its entities, or its splines, which are
    /// kept apart from the entities. A sketch of splines alone is typed too; read from the entities only, its nodes
    /// could not be dragged.
    pub fn is_typed_sketch(&self, si: usize) -> bool {
        self.sketches.get(si).is_some_and(|s| !s.entities.is_empty() || !s.splines.is_empty())
    }
    /// THE LOOPS OF A SKETCH MADE FROM ALL ITS CURVES, whatever changed since they were made: what a rebuild round a
    /// change (`round`) is held to.
    pub fn regen_sketch_whole(&mut self, si: usize) {
        if let Some(s) = self.sketches.get_mut(si) {
            s.laid = None;
        }
        self.regen_sketch(si);
    }

    /// Rebuild the contours of a sketch from its entities, as a multi-loop tessellation. The contour ids are
    /// preserved where possible (see the matching below).
    pub fn regen_sketch(&mut self, si: usize) {
        // within a batch the rebuild waits for its end, once for every change (`batch`)
        if self.held_rebuilds.depth > 0 {
            self.held_rebuilds.sketches.insert(si);
            return;
        }
        // the loops are made anew: what a drag kept of them goes
        if let Some(s) = self.sketches.get_mut(si) {
            s.drag_session = None;
        }
        // THE FRAME OF REFERENCE FIRST, before anything is derived from it. The origin and the axis
        // guides are not geometry: a contour, a dimension to an axis and every profile taken from this
        // sketch stand on them. See `pin_frame` for what moving them costs.
        if self.sketches.get(si).is_some_and(|s| s.origin != 0 || s.axis_pts.iter().any(|g| *g != 0)) {
            self.ensure_frame(si); // a document from before the anchor gets one, so the repair below has a target
        }
        self.detach_geometry_from_origin(si);
        if let Some(s) = self.sketches.get_mut(si) {
            s.pin_frame();
        }
        let Some(s) = self.sketches.get(si) else { return };
        // only the loops round what changed since they were made (`round::Laid`), where few curves changed; splines and
        // text are made whole
        let plain = s.splines.is_empty() && s.texts.is_empty();
        if let Some(changed) = self.changed_since_laid(si).filter(|_| plain) {
            let drawn = s.entities.iter().filter(|e| !e.construction).count();
            if changed.drawn.is_empty() && changed.gone.is_empty() {
                return; // nothing drawn changed: every loop stands
            }
            if 4 * (changed.drawn.len() + changed.gone.len()) <= drawn {
                return self.loops_round(si, &changed.drawn, &changed.gone);
            }
        }
        let s = &self.sketches[si];
        // Every contour of a sketch comes from entities now; the ids are reused by position.
        let entity_cids: Vec<Id> = s.contour_ids.clone();
        // Construction geometry never reaches a profile or a contour; it is drawn separately, dashed. Only
        // ordinary entities enter the contours.
        let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| !e.construction).cloned().collect();
        let (pts, splines) = (s.points.clone(), s.splines.clone());
        // Closed contours are the region faces of the planar arrangement (intersections give minimal faces), so
        // half-rings and other areas that are not whole loops become selectable. Open chains (sweep and loft
        // paths) come from the ordinary tessellation, the arrangement producing closed faces only.
        // Splines are cut into the arrangement with the rest (`arrangement_faces`), so their closed regions are among these.
        let mut pairs: Vec<(Contour, Vec<Id>)> = arrangement_regions_prov(&pts, &ents, &splines);
        pairs.extend(tessellate_sketch_multi(&pts, &ents).into_iter().filter(|c| !c.closed).map(|c| (c, Vec::new())));
        // An open spline is an open chain too (cubic Hermite with tangent handles, automatic Catmull-Rom by default): a
        // path for a sweep or a loft. A construction spline never reaches a profile; it is drawn dashed.
        for sp in splines.iter().filter(|sp| !sp.closed && !sp.construction) {
            let cps: Vec<Point2> = sp.points.iter().filter_map(|id| pts.iter().find(|p| p.id == *id).map(|p| crate::geom::Point2::new(p.x, p.y))).collect();
            if cps.len() >= 2 {
                pairs.push((tessellate_spline_hermite(&cps, &sp.tangents, false), Vec::new()));
            }
        }
        // Contours of parametric text (its baked glyphs); ordinary, non-construction text reaches the
        // profile.
        for t in &s.texts {
            if t.construction {
                continue;
            }
            for loop_ in &t.glyphs {
                // THE SEAM IS CLEANED HERE TOO, not only where the glyphs are baked. A glyf outline (TTF)
                // closes by returning to its starting point, so the loop carries two coincident points in a
                // row - a segment of zero length, which OCCT will not make a wire from. Documents whose text
                // was baked before this was fixed already exist and keep their glyphs as they were baked:
                // without this they would go on failing to extrude however often they are rebuilt.
                let clean = seam_cleaned(loop_);
                if clean.len() >= 3 {
                    pairs.push((Contour::closed(clean), Vec::new()));
                }
            }
        }
        let new_entity_cids = self.replace_contours(entity_cids, pairs, &[]);
        self.sketches[si].contour_ids = new_entity_cids;
        let laid = if plain { self.laid_now(si).map(Box::new) } else { None };
        self.sketches[si].laid = laid;
    }
    /// A BATCH OF CHANGES TO SKETCHES: within `work` a rebuild of a sketch is held back, and at its end every sketch
    /// asked for is rebuilt once. A tool that lays many elements, each rebuilding the sketch, rebuilt it as many times:
    /// every corner of 2 000 rectangles filleted took 1 673 s in a test build. Batches nest; the outermost rebuilds.
    pub fn batch<R>(&mut self, work: impl FnOnce(&mut Self) -> R) -> R {
        self.held_rebuilds.depth += 1;
        let out = work(self);
        self.held_rebuilds.depth -= 1;
        if self.held_rebuilds.depth == 0 {
            // the solves held back first, each rebuilding its sketch; then the rebuilds of the rest
            let solves = std::mem::take(&mut self.held_rebuilds.solves);
            for &si in &solves {
                self.solve_sketch(si);
            }
            for si in std::mem::take(&mut self.held_rebuilds.sketches) {
                if !solves.contains(&si) {
                    self.regen_sketch(si);
                }
            }
        }
        out
    }

    /// SOLVE A SKETCH, or within a batch at its end, once for every change (`batch`) - for a tool that solves after
    /// laying an element and reads nothing of the solve. A chamfer at each of 20 corners solved the whole sketch 20
    /// times.
    pub fn solve_sketch_held(&mut self, si: usize) {
        if self.held_rebuilds.depth > 0 {
            self.held_rebuilds.solves.insert(si);
            return;
        }
        self.solve_sketch(si);
    }

    /// THE LOOPS OF A SKETCH REBUILT WHERE IT MOVED, for a frame of a drag: `moved` - the points moved by the frame and
    /// the centres of the circles whose radius changed. The loops are the faces of the arrangement of the curves and
    /// the open chains, and neither runs between curves whose boxes, widened past every weld, do not meet: the curves
    /// of a group whose boxes meet one another make their loops alone. So only the groups holding a curve that moved,
    /// or a curve that shared a loop with one, are made again; the loops of the rest stand. Rebuilt whole, a frame
    /// of a drag among 70 000 lines took 0.43 s, among 70 000 rectangles 1.6 s, in a release build. A sketch with
    /// splines or text, or one whose frame of reference had to be put back, is rebuilt whole.
    pub(crate) fn regen_sketch_moved(&mut self, si: usize, moved: &std::collections::HashSet<Id>) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        s.drag_session = None; // its loops are made anew: what a drag kept of them goes
        let s = &self.sketches[si];
        // a frame of reference to be made first (`ensure_frame` in `regen_sketch`), splines, text: rebuilt whole
        if !s.splines.is_empty() || !s.texts.is_empty() || (s.origin != 0 || s.axis_pts.iter().any(|g| *g != 0)) && s.frame == 0 {
            return self.regen_sketch(si);
        }
        if self.detach_geometry_from_origin(si) || self.sketches[si].pin_frame() {
            return self.regen_sketch(si);
        }
        let s = &self.sketches[si];
        let changed: std::collections::HashSet<Id> = s.entities.iter().filter(|e| !e.construction && entity_points(e).iter().any(|p| moved.contains(p))).map(|e| e.id).collect();
        if changed.is_empty() {
            return; // nothing drawn moved: every loop stands
        }
        self.loops_round(si, &changed, &std::collections::HashSet::new());
    }

    /// THE LOOPS ROUND A CHANGE MADE AGAIN (`round`): the curves `changed` (drawn now) and `gone` since the loops were
    /// made, the regions and chains round them made again, the rest standing.
    pub(crate) fn loops_round(&mut self, si: usize, changed: &std::collections::HashSet<Id>, gone: &std::collections::HashSet<Id>) {
        let laid = self.laid_held(si);
        let s = &self.sketches[si];
        let at: std::collections::HashMap<Id, SketchPoint> = s.points.iter().map(|p| (p.id, *p)).collect();
        let drawn: Vec<SketchEntity> = s.entities.iter().filter(|e| !e.construction).copied().collect();
        let shifted: Vec<usize> = drawn.iter().enumerate().filter(|(_, e)| changed.contains(&e.id)).map(|(k, _)| k).collect();
        let touched = |e: &Id| changed.contains(e) || gone.contains(e);
        // each loop of the sketch: its place, closed or open, its box, and whether a curve changed is in it - looked at
        // once, with no list made for a loop: made for each of the 40 000 cells of a grid, the lists were 14 ms of an edit
        struct Before {
            id: Id,
            ci: usize,
            closed: bool,
            span: [f64; 4],
            touched: bool,
        }
        let of = |id: Id, ci: usize| self.contours.ents_of(id).into_iter().flatten().chain(&self.contours[ci].edge_src);
        let before: Vec<Before> = s
            .contour_ids
            .iter()
            .filter_map(|&id| {
                let ci = self.contour_index(id)?;
                Some(Before { id, ci, closed: self.contours[ci].closed, span: round::contour_box(&self.contours[ci]), touched: of(id, ci).any(touched) })
            })
            .collect();
        // where the moved curves stood is known by the loops they were in, and where they stand by their boxes now
        let boxes: Vec<[f64; 4]> = drawn.iter().map(|e| entity_box(e, &|id| at.get(&id).map(|p| (p.x, p.y)))).collect();
        let swept: Vec<[f64; 4]> = before.iter().filter(|b| b.touched).map(|b| b.span).chain(shifted.iter().map(|&k| boxes[k])).collect();
        // the regions meeting a swept box, looked up on a grid of their boxes: each region against each of the 400 boxes
        // a line of a grid of 200 by 200 sweeps was 16 million looks, 60 ms of an edit
        let mut old: std::collections::HashSet<Id> = before.iter().filter(|b| b.touched).map(|b| b.id).collect();
        let spans: Vec<[f64; 4]> = before.iter().map(|b| b.span).collect();
        let regions = round::BoxGrid::new(&spans);
        for w in &swept {
            old.extend(regions.near(w, spans.len()).into_iter().filter(|&i| before[i].closed && round::meet(&spans[i], w)).map(|i| before[i].id));
        }
        let held = before.iter().filter(|b| b.closed && old.contains(&b.id)).map(|b| b.span).reduce(round::union);
        // the chains are made from the changed curves, every curve of an open loop one of them was in, and every curve of
        // any loop a curve gone was in: two sides of a rectangle deleted leave the other two an open chain, and with
        // nothing changed left to reach it from it was lost. A curve still there reaches its chain along its ends; seeded
        // from every loop it was in, a line of a grid of 200 by 200 took in the whole grid
        let place: std::collections::HashMap<Id, usize> = drawn.iter().enumerate().map(|(k, e)| (e.id, k)).collect();
        let from: Vec<usize> = shifted
            .iter()
            .copied()
            .chain(before.iter().filter(|b| b.touched && (!b.closed || of(b.id, b.ci).any(|e| gone.contains(e)))).flat_map(|b| of(b.id, b.ci).filter_map(|e| place.get(e).copied())))
            .collect();
        let grid = round::BoxGrid::new(&boxes);
        let curve = |k: usize| drawn[k];
        let point = |id: Id| at.get(&id).copied();
        let near = |b: &[f64; 4]| grid.near(b, boxes.len());
        let change = round::Change { curve: &curve, boxes: &boxes, at: &point, near: &near, moved: &shifted, swept: &swept, held };
        let mut pairs = round::regions_round(&change);
        let chains = round::chains_round(&change, &from);
        pairs.extend(chains.made);
        old.extend(before.iter().filter(|b| !b.closed && of(b.id, b.ci).any(|e| chains.through.contains(e))).map(|b| b.id));
        let (old, kept): (Vec<Id>, Vec<Id>) = s.contour_ids.iter().partition(|cid| old.contains(cid));
        // the loops whose nesting may change: those whose box meets the box of the loops going and coming
        let going: std::collections::HashSet<Id> = old.iter().copied().collect();
        let over = before.iter().filter(|b| going.contains(&b.id)).map(|b| b.span).chain(pairs.iter().map(|(c, _)| round::contour_box(c))).reduce(round::union);
        let near: Vec<Id> = before.iter().filter(|b| !going.contains(&b.id) && over.is_some_and(|o| round::meet(&b.span, &o))).map(|b| b.id).collect();
        let made = self.replace_contours(old, pairs, &near);
        self.sketches[si].contour_ids = kept.into_iter().chain(made).collect();
        self.laid_follows(si, laid, changed, gone);
    }
}
