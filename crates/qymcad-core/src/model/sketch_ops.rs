//! A SKETCH'S CURVES MIRRORED, COPIED AND PATTERNED, and its corners limited, rounded and cut.

use super::*;

impl Project {
    /// Hand out a new stable id.
    pub fn alloc_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    // --- Contours. Mutations happen only here, so the parallel id array cannot drift out of step. ---

    /// Add a set of contours (a DXF or SVG import, for example).
    pub fn add_contours(&mut self, cs: impl IntoIterator<Item = Contour>) {
        for c in cs {
            self.add_contour(c);
        }
    }

    // --- Typed sketch: points plus entities. ---

    // --- Editing and duplicating sketch entities, by selected entity ids. ---

    /// Mirror the entities (as a copy) about the line (ax,ay)-(bx,by).
    pub fn mirror_entities(&mut self, si: usize, eids: &[Id], ax: f64, ay: f64, bx: f64, by: f64) {
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-12 {
            return;
        }
        self.dup_entities(
            si,
            eids,
            |x, y| {
                let (vx, vy) = (x - ax, y - ay);
                let t = (vx * dx + vy * dy) / len2;
                let (projx, projy) = (dx * t, dy * t);
                let (perpx, perpy) = (vx - projx, vy - projy);
                (x - 2.0 * perpx, y - 2.0 * perpy)
            },
            false,
        );
    }

    /// MIRROR THE ENTITIES (as a copy) ABOUT THE POINT (px, py): each point goes to the far side of it, the same distance
    /// off - a half turn about it, so an arc keeps its way round.
    pub fn mirror_about_point(&mut self, si: usize, eids: &[Id], px: f64, py: f64) {
        self.dup_entities(si, eids, |x, y| (2.0 * px - x, 2.0 * py - y), false);
    }

    /// Duplicate the selected entities with an offset of (dx, dy) and return the ids of the copies. Used by
    /// the interactive copy command, which reads as base point then target.
    pub fn copy_entities(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64) -> Vec<Id> {
        self.dup_entities(si, eids, |x, y| (x + dx, y + dy), true) // a copy carries its internal constraints and dimensions
    }

    /// Linear array: `count` copies spaced by (dx, dy).
    pub fn array_linear(&mut self, si: usize, eids: &[Id], dx: f64, dy: f64, count: u32) {
        for k in 1..count.max(1) {
            let (ox, oy) = (dx * k as f64, dy * k as f64);
            self.dup_entities_unbuilt(si, eids, |x, y| (x + ox, y + oy), false);
        }
        self.regen_sketch(si); // the loops once, for every copy
    }

    /// Circular array: `count` copies around (cx, cy) spanning `total_deg` in total.
    pub fn array_circular(&mut self, si: usize, eids: &[Id], cx: f64, cy: f64, count: u32, total_deg: f64) {
        let count = count.max(1);
        // A full circle (360 degrees) spreads `count` copies around it; a sector holds `count` copies within it.
        let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / count as f64 } else { total_deg / count.max(2) as f64 };
        for k in 1..count {
            let ang = (step * k as f64).to_radians();
            let (s_, c_) = (ang.sin(), ang.cos());
            self.dup_entities_unbuilt(
                si,
                eids,
                move |x, y| {
                    let (vx, vy) = (x - cx, y - cy);
                    (cx + vx * c_ - vy * s_, cy + vx * s_ + vy * c_)
                },
                false,
            );
        }
        self.regen_sketch(si); // the loops once, for every copy
    }

    /// Create the pattern instances without recording the pattern. Returns the ids of the new entities.
    pub(crate) fn pattern_instances(&mut self, si: usize, source: &[Id], kind: PatternKind) -> Vec<Id> {
        let mut out = Vec::new();
        match kind {
            PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
                // Grid: i along (dx,dy), j along (dx2,dy2). (0,0) is the original and is not duplicated.
                for i in 0..count.max(1) {
                    for j in 0..count2.max(1) {
                        if i == 0 && j == 0 {
                            continue;
                        }
                        let (ox, oy) = (dx * i as f64 + dx2 * j as f64, dy * i as f64 + dy2 * j as f64);
                        out.extend(self.dup_entities_unbuilt(si, source, |x, y| (x + ox, y + oy), false));
                    }
                }
            }
            PatternKind::Circular { cx, cy, count, total_deg } => {
                let count = count.max(1);
                let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / count as f64 } else { total_deg / count.max(2) as f64 };
                for k in 1..count {
                    let ang = (step * k as f64).to_radians();
                    let (s_, c_) = (ang.sin(), ang.cos());
                    out.extend(self.dup_entities_unbuilt(
                        si,
                        source,
                        move |x, y| {
                            let (vx, vy) = (x - cx, y - cy);
                            (cx + vx * c_ - vy * s_, cy + vx * s_ + vy * c_)
                        },
                        false,
                    ));
                }
            }
        }
        out
    }

    /// Fillet the corner at vertex `pid`, when exactly two edges meet there. The inner side is chosen by
    /// the bisector of the chords. Used both by click-on-corner and by the chain command. Returns whether
    /// it succeeded.
    /// THE LARGEST VALUE A CORNER OF TWO LINES TAKES for `tool`: a fillet's point of touching lies r / tan(theta / 2) from
    /// the corner (theta the angle between the lines), a chamfer's leg along the line, and the cut of a symmetric chamfer
    /// is 2 sin(theta / 2) times its leg - none may reach the far end of the shorter line. `None` for a corner not made of two lines. The field of the tool refuses a value past it
    /// in words, where the corner used to take it and do nothing, or cut it down without a word.
    pub fn corner_limit(&self, si: usize, pid: Id, tool: CornerTool) -> Option<f64> {
        // Where more than two edges meet, there is a corner for every pair, and the field must refuse a value that
        // is too big for AT LEAST ONE of them: the tightest corner is the one that bounds it.
        self.vertex_pairs(si, pid).iter().filter_map(|pair| self.corner_limit_of_pair(si, pid, *pair, tool)).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
    }
    /// The bound of the one corner the point `(x, y)` stands in, where more than two edges meet at `pid`.
    pub fn corner_limit_near(&self, si: usize, pid: Id, tool: CornerTool, x: f64, y: f64) -> Option<f64> {
        self.vertex_pair(si, pid, Some((x, y))).and_then(|pair| self.corner_limit_of_pair(si, pid, pair, tool))
    }
    /// The bound of one named pair of edges at `pid`.
    pub fn corner_limit_of_pair(&self, si: usize, pid: Id, (e1, e2): (Id, Id), tool: CornerTool) -> Option<f64> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let mut dirs = Vec::new();
        for e in [e1, e2] {
            let (a, b) = self.line_ends(si, e)?;
            let other = if a == pid { b } else { a };
            let (ox, oy) = self.point_xy(si, other)?;
            let l = (ox - pcx).hypot(oy - pcy);
            if l < 1e-9 {
                return None;
            }
            dirs.push(((ox - pcx) / l, (oy - pcy) / l, l));
        }
        if dirs.len() != 2 {
            return None;
        }
        let shorter = dirs[0].2.min(dirs[1].2);
        let theta = (dirs[0].0 * dirs[1].0 + dirs[0].1 * dirs[1].1).clamp(-1.0, 1.0).acos();
        Some(match tool {
            CornerTool::Fillet => shorter * (theta / 2.0).tan(),
            CornerTool::Chamfer(crate::feature::ChamferMode::Symmetric) => shorter * 2.0 * (theta / 2.0).sin(),
            CornerTool::Chamfer(_) => shorter,
        })
    }

    /// THE LARGEST VALUE `tool` TAKES on the corners of a set of lines at once (every line of the sketch with `None`): a
    /// line between two corners being cut spends both on itself - for a fillet r / tan(a / 2) + r / tan(b / 2) of its
    /// length - and a line with one corner cut spends one. `None` when there is no corner of two lines to cut. A
    /// rectangle 40 x 30 takes a radius up to 15, half its short side, where one corner alone takes 30.
    pub fn all_corners_limit(&self, si: usize, only: Option<&std::collections::HashSet<Id>>, tool: CornerTool) -> Option<f64> {
        let s = self.sketches.get(si)?;
        let lines: Vec<(Id, Id, Id)> = s
            .entities
            .iter()
            .filter(|e| only.is_none_or(|o| o.contains(&e.id)))
            .filter_map(|e| match e.kind {
                EntityKind::Line { a, b } => Some((e.id, a, b)),
                _ => None,
            })
            .collect();
        // what one unit of the value spends along each line at every corner of two lines - the cotangent of half the angle for
        // a fillet - the corners being the points two lines of the set meet at
        let mut cot: std::collections::HashMap<Id, f64> = std::collections::HashMap::new();
        for &(_, a, b) in &lines {
            for p in [a, b] {
                if cot.contains_key(&p) || lines.iter().filter(|(_, x, y)| *x == p || *y == p).count() != 2 || self.vertex_edges(si, p).len() != 2 {
                    continue;
                }
                if let Some(l) = self.corner_limit(si, p, tool) {
                    let (pcx, pcy) = self.point_xy(si, p)?;
                    let ends: Vec<f64> = self
                        .vertex_edges(si, p)
                        .iter()
                        .filter_map(|e| self.line_ends(si, *e))
                        .filter_map(|(x, y)| self.point_xy(si, if x == p { y } else { x }))
                        .map(|(ox, oy)| (ox - pcx).hypot(oy - pcy))
                        .collect();
                    let shorter = ends.iter().copied().fold(f64::INFINITY, f64::min);
                    cot.insert(p, shorter / l); // corner_limit = shorter / (what one unit spends)
                }
            }
        }
        lines
            .iter()
            .filter_map(|&(_, a, b)| {
                let spent = cot.get(&a).copied().unwrap_or(0.0) + cot.get(&b).copied().unwrap_or(0.0);
                let (ax, ay) = self.point_xy(si, a)?;
                let (bx, by) = self.point_xy(si, b)?;
                (spent > 1e-12).then(|| (bx - ax).hypot(by - ay) / spent)
            })
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
    }

    pub fn fillet_at_vertex(&mut self, si: usize, pid: Id, r: f64) -> bool {
        self.fillet_at_vertex_by(si, pid, FilletSize::radius(r))
    }

    /// HOW FAR THE ARC OF A FILLET TURNS on the corner at `pid`: pi minus the angle between its two edges, each taken
    /// from the corner to its far end (exact for lines, the chord of an arc for an arc). `None` without a corner of two
    /// edges.
    pub fn corner_sweep(&self, si: usize, pid: Id) -> Option<f64> {
        let edges = self.vertex_edges(si, pid);
        if edges.len() != 2 {
            return None;
        }
        self.corner_sweep_of_pair(si, pid, (edges[0], edges[1]))
    }

    /// The same for the corner the two named edges make at `pid`, where more than two edges may meet.
    pub fn corner_sweep_of_pair(&self, si: usize, pid: Id, (e1, e2): (Id, Id)) -> Option<f64> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        let mut dirs = Vec::new();
        for e in [e1, e2] {
            let (a, b) = self.edge_end_ids(si, e)?;
            let (ox, oy) = self.point_xy(si, if a == pid { b } else { a })?;
            let l = (ox - pcx).hypot(oy - pcy);
            if l < 1e-9 {
                return None;
            }
            dirs.push(((ox - pcx) / l, (oy - pcy) / l));
        }
        let corner = (dirs[0].0 * dirs[1].0 + dirs[0].1 * dirs[1].1).clamp(-1.0, 1.0).acos();
        Some(std::f64::consts::PI - corner)
    }

    /// FILLET THE CORNER AT `pid` WITH A SIZE GIVEN BY ITS RADIUS, ITS CHORD OR ITS ARC LENGTH. The radius the size
    /// makes on this corner is laid, and the fillet keeps the size as it was given: a chord as the distance between
    /// the two points of touching, an arc length as the length of the arc - so a dimension changed afterwards moves the
    /// fillet. A chord or an arc the corner cannot take is refused, where a radius is pressed to the corner as before.
    pub fn fillet_at_vertex_by(&mut self, si: usize, pid: Id, size: FilletSize) -> bool {
        let r = match size.by {
            FilletBy::Radius => size.value,
            _ => {
                let Some(r) = self.corner_sweep(si, pid).and_then(|sweep| size.radius_on(sweep)) else { return false };
                if self.corner_limit(si, pid, CornerTool::Fillet).is_some_and(|l| r >= l * (1.0 - 1e-9)) {
                    return false;
                }
                r
            }
        };
        if !self.fillet_round(si, pid, r) {
            return false;
        }
        if size.by != FilletBy::Radius {
            self.give_fillet_its_size(si, size);
        }
        true
    }

    /// CUT EVERY CORNER OF A SET WITH ONE ANSWER, OR NONE OF THEM: a fillet of the size on every corner - a chord or an
    /// arc length makes its own radius on each and is kept as it was given - or a chamfer of the legs, its first line
    /// the one nearer `toward`. A corner the cut does not fit leaves the sketch as it was and answers 0: cut on the
    /// corners that took it and left whole on the one that did not, the set was a half-made drawing nobody asked for.
    /// Answers how many corners were cut.
    pub fn cut_corner_set(&mut self, si: usize, corners: &[CornerAt], cut: CornerCut, toward: Option<crate::geom::Point2>) -> usize {
        // every corner laid, the sketch rebuilt once (`batch`)
        self.batch(|p| p.cut_corner_set_unbuilt(si, corners, cut, toward))
    }

    pub(crate) fn cut_corner_set_unbuilt(&mut self, si: usize, corners: &[CornerAt], cut: CornerCut, toward: Option<crate::geom::Point2>) -> usize {
        let Some(before) = self.sketches.get(si).cloned() else { return 0 };
        for corner in corners {
            let done = match cut {
                CornerCut::Chamfer(legs) => self.chamfer_lines_of_pair(si, corner.pair, legs, toward),
                CornerCut::Fillet(size) if size.by == FilletBy::Radius => self.fillet_at_pair(si, corner.pair, size.value),
                CornerCut::Fillet(size) => self.fillet_pair_by(si, *corner, size),
            };
            if !done {
                self.sketches[si] = before;
                self.regen_sketch(si);
                return 0;
            }
        }
        corners.len()
    }

    /// A FILLET OF THE CORNER OF A SET BY ITS CHORD OR ITS ARC LENGTH: the radius the size makes on this corner, as the
    /// preview of the corner draws it, and the size kept as it was given.
    pub(crate) fn fillet_pair_by(&mut self, si: usize, corner: CornerAt, size: FilletSize) -> bool {
        let Some((_, r)) = self.corner_blend(si, corner.point, corner.pair, CornerCut::Fillet(size)).and_then(|b| b.arc) else { return false };
        if !self.fillet_at_pair(si, corner.pair, r) {
            return false;
        }
        self.give_fillet_its_size(si, size);
        true
    }

    /// THE LAST FILLET LAID KEEPS ITS SIZE AS IT WAS GIVEN: its radius dimension is put in place of a distance between
    /// the two points of touching (a chord) or of an arc length dimension, and the sketch is solved.
    pub(crate) fn give_fillet_its_size(&mut self, si: usize, size: FilletSize) {
        let Some(s) = self.sketches.get(si) else { return };
        let Some(last) = s.entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).max_by_key(|e| e.id).copied() else { return };
        let EntityKind::Arc { center: centre, a, b, ccw } = last.kind else { return };
        let Some(ci) = self.fillet_radius_constraint(si, last.id) else { return };
        let end = |id: Id| self.point_xy(si, id).map(|(x, y)| sketch::DimEnd { id, at: crate::geom::Point2::new(x, y) });
        let (Some(end_a), Some(end_b), Some((cx, cy))) = (end(a), end(b), self.point_xy(si, centre)) else { return };
        // THE CHORD STANDS OUTSIDE THE SHAPE, on the side of the arc away from its centre: written as the arc ran, it
        // stood inside the shape on one corner out of two
        let [ca, cb] = sketch::ends_facing([end_a, end_b], sketch::Side::AwayFrom(crate::geom::Point2::new(cx, cy)));
        self.sketches[si].constraints[ci] = match size.by {
            FilletBy::Chord => Constraint::Distance { a: ca, b: cb, d: size.value, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None },
            _ => Constraint::ArcLength { c: centre, a, b, ccw, len: size.value, off: 0.0, expr: String::new(), driven: false },
        };
        self.solve_sketch(si);
    }

    pub(crate) fn fillet_round(&mut self, si: usize, pid: Id, r: f64) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, None) else { return false };
        self.fillet_at_pair(si, pair, r)
    }

    /// Fillet the corner at `pid` that the point `(x, y)` stands in, where more than two edges meet there.
    pub fn fillet_at_vertex_near(&mut self, si: usize, pid: Id, r: f64, x: f64, y: f64) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, Some((x, y))) else { return false };
        self.fillet_at_pair(si, pair, r)
    }

    /// Fillet the corner the two named edges make at a shared vertex. The arc goes on the side of their bisector,
    /// which is what the fillet at a vertex has always done.
    pub fn fillet_at_pair(&mut self, si: usize, (e1, e2): (Id, Id), r: f64) -> bool {
        let Some(shared) = self.corner_of_pair(si, e1, e2) else { return false };
        let Some((pcx, pcy)) = self.point_xy(si, shared) else { return false };
        let dir = |me: &Self, eid: Id| -> Option<(f64, f64)> {
            let (a, b) = me.edge_end_ids(si, eid)?;
            let other = if a == shared { b } else { a };
            let (ox, oy) = me.point_xy(si, other)?;
            let (dx, dy) = (ox - pcx, oy - pcy);
            let l = (dx * dx + dy * dy).sqrt();
            (l > 1e-9).then(|| (dx / l, dy / l))
        };
        let (Some(d1), Some(d2)) = (dir(self, e1), dir(self, e2)) else { return false };
        let (bx, by) = (d1.0 + d2.0, d1.1 + d2.1);
        let bl = (bx * bx + by * by).sqrt();
        let near = if bl < 1e-6 { (pcx, pcy) } else { (pcx + bx / bl * r * 0.3, pcy + by / bl * r * 0.3) };
        self.fillet_curves(si, e1, e2, r, near.0, near.1)
    }

    /// Fillet every corner (each vertex with exactly two edges) with radius `r`, returning how many
    /// corners were filleted. The vertex list is snapshotted first, because filleting removes the old
    /// vertex.
    pub fn fillet_all_corners(&mut self, si: usize, r: f64) -> usize {
        self.fillet_all_corners_of(si, r, None)
    }
}
