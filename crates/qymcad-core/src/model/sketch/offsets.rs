//! CURVES OFFSET FROM A CURVE AND HELD TO IT (`SketchOffset`): the offset of an ellipse or a spline is no curve of its
//! own kind, so it is not held by constraints, as the offset of a circle or of lines is, but laid again from its source
//! after every solve, at the distance of its dimension - as a projection follows its body. Edited, dragged or sized, the
//! source carries its offset with it; the dimension edited moves the offset.
use super::*;
use crate::model::curve::Curve;

/// A PIECE OF AN OFFSET AND A COPY IT MAY BE LAID ON, by their places, and how far apart they lie.
struct Pairing {
    piece: usize,
    copy: usize,
    apart: f64,
}

/// AN END OF A COPY OF A LINE: the line and the point it ends at.
struct LineEnd {
    line: Id,
    point: Id,
}

/// THE END OF A LINE OF A CHAIN AND WHERE IT IS HELD: at the end of the copy it was cut against.
struct HeldEnd {
    point: Id,
    at: Point2,
}

/// THE END `cut` NAMES OF THE PIECES OF AN OFFSET CUT WHERE THE COPY OF ITS LINE CROSSES IT, read along the whole line:
/// the crossing nearest that end. Nothing where the two no longer cross near it.
fn cut_at_the_line(s: &Sketch, pieces: &mut [Curve], cut: &OffsetCut) {
    let Some(line) = s.entity(cut.line).and_then(|e| Curve::of(s, e)).map(|c| c.reaching(10.0)) else { return };
    let k = match cut.end {
        CopyEnd::Start => 0,
        CopyEnd::Finish => pieces.len().saturating_sub(1),
    };
    let Some(piece) = pieces.get(k) else { return };
    // the crossing on the half of the piece at that end, the nearest to it
    let near_end = |t: f64| if cut.end == CopyEnd::Start { t < 0.5 } else { t > 0.5 };
    let Some(t) = piece.meet(&line).into_iter().filter(|m| near_end(m.ta)).min_by(|x, y| (x.ta - 0.5).abs().total_cmp(&(y.ta - 0.5).abs()).reverse()).map(|m| m.ta) else { return };
    pieces[k] = match cut.end {
        CopyEnd::Start => piece.piece([t, 1.0]),
        CopyEnd::Finish => piece.piece([0.0, t]),
    };
}

/// WHAT LAYING THE OFFSETS AGAIN CHANGED: the points it moved, and whether a copy came out with another number of points
/// - new points laid and old ones gone, for which the whole sketch is made again.
#[derive(Default)]
pub(crate) struct Followed {
    pub(crate) moved: Vec<Id>,
    pub(crate) relaid: bool,
}

impl OffsetSide {
    /// The side `d` lies on to the left of a curve: the left for a positive one.
    pub(crate) fn of(d: f64) -> OffsetSide {
        if d >= 0.0 {
            OffsetSide::Left
        } else {
            OffsetSide::Right
        }
    }

    /// The distance `d` to the left of the curve, on this side.
    fn signed(self, d: f64) -> f64 {
        match self {
            OffsetSide::Left => d,
            OffsetSide::Right => -d,
        }
    }
}

impl Sketch {
    /// The place among the constraints of the dimension of offset `o`: the distance from its source to its first copy.
    fn offset_dim(&self, o: &SketchOffset) -> Option<usize> {
        let first = *o.copies.first()?;
        self.constraints.iter().position(|c| matches!(c, Constraint::CurveDistance { to, .. } if to.curve_id() == Some(first)))
    }

    /// WHETHER CONSTRAINT `c` SETS AN OFFSET: the dimension of a `SketchOffset`, which says where the copies are laid. The
    /// solver does not solve it - the copies are laid at it - and holding the source to copies it may not move, it took
    /// the source's freedom.
    pub(crate) fn sets_an_offset(&self, c: &Constraint) -> bool {
        let Constraint::CurveDistance { to, .. } = c else { return false };
        to.curve_id().is_some_and(|id| self.offsets.iter().any(|o| o.copies.first() == Some(&id)))
    }

    /// WHERE THE DIMENSION OF AN OFFSET IS DRAWN: from the first node of its first copy, a point of the true offset, to
    /// the place of the source nearest it - the distance itself. Read as the least distance between the two curves,
    /// a search along both, it cost a solve of a spline and its copy 15 s and every frame its drawing. `None` for a
    /// constraint that is no dimension of an offset.
    pub(crate) fn offset_dim_ends(&self, c: &Constraint) -> Option<[Point2; 2]> {
        let Constraint::CurveDistance { to, .. } = c else { return None };
        let copy = to.curve_id().filter(|id| self.offsets.iter().any(|o| o.copies.first() == Some(id)))?;
        let node = self.entity(copy).map(entity_points)?.first().and_then(|id| self.point(*id)).map(|q| Point2::new(q.x, q.y))?;
        let source = self.offsets.iter().find(|o| o.copies.first() == Some(&copy)).and_then(|o| self.entity(o.source)).and_then(|e| Curve::of(self, e))?;
        Some([source.at(source.nearest(node).t), node])
    }

    /// WHETHER THE SOLVER SOLVES CONSTRAINT `c`: not a reference dimension, which measures, and not the dimension of an
    /// offset, which lays its copies.
    pub(crate) fn solves(&self, c: &Constraint) -> bool {
        !c.is_driven() && !self.sets_an_offset(c)
    }

    /// The points of the copies of every offset: given to the solver, as the points of a projection are.
    pub(crate) fn offset_points(&self) -> Vec<Id> {
        let mut out: Vec<Id> = self.offsets.iter().flat_map(|o| o.copies.iter()).filter_map(|id| self.entity(*id)).flat_map(entity_points).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// DROP THE OFFSETS OF WHAT IS GONE: a source or a copy no longer in the sketch. What is left of it stays as plain
    /// curves.
    pub(crate) fn drop_offsets_of_what_is_gone(&mut self) {
        let curves: std::collections::HashSet<Id> = self.entities.iter().map(|e| e.id).collect();
        self.let_offsets_go(|o| !curves.contains(&o.source) || o.copies.iter().any(|c| !curves.contains(c)) || o.cuts.iter().any(|c| !curves.contains(&c.line)));
    }

    /// LET GO THE OFFSET CURVE `eid` IS A COPY OF, before an edit changes the copy: what the edit leaves is a plain
    /// curve, and laid again from the source after the solve it would be undone.
    pub(crate) fn let_go_the_copy(&mut self, eid: Id) {
        self.let_offsets_go(|o| o.copies.contains(&eid));
    }

    /// LET GO THE OFFSETS CURVE `eid` IS THE SOURCE OR A COPY OF, before the curve is cut into pieces.
    pub(crate) fn let_go_the_offsets_of(&mut self, eid: Id) {
        self.let_offsets_go(|o| o.source == eid || o.copies.contains(&eid));
    }

    /// LET GO THE OFFSETS `gone` PICKS, each with its dimension: the dimension belongs to the record, and left behind as a
    /// least distance from the source to a copy, a spline to a spline, it was solved by a search along both at every step
    /// - a crossing of a copy cut took 15 s against 0.06 s without it.
    fn let_offsets_go(&mut self, gone: impl Fn(&SketchOffset) -> bool) {
        let dims: Vec<usize> = self.offsets.iter().filter(|o| gone(o)).filter_map(|o| self.offset_dim(o)).collect();
        if dims.is_empty() && !self.offsets.iter().any(&gone) {
            return;
        }
        let mut k = 0;
        self.constraints.retain(|_| {
            k += 1;
            !dims.contains(&(k - 1))
        });
        self.offsets.retain(|o| !gone(o));
    }
}

impl Project {
    /// OFFSET CURVE `eid` OF SKETCH `si` BY `d` TO ITS LEFT (to its right for a negative one) AND HOLD THE OFFSET TO IT:
    /// the copies laid, the record kept and the distance laid as its dimension. Answers whether anything was laid -
    /// nothing where the offset folds away whole.
    pub(crate) fn lay_held_offset(&mut self, si: usize, eid: Id, d: f64) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let Some(e) = s.entity(eid) else { return false };
        let Some(source) = Curve::of(s, e) else { return false };
        let from = crate::model::curve_ref_drawn(e);
        let pieces = source.offset(d);
        if pieces.is_empty() {
            return false;
        }
        let copies: Vec<Id> = pieces.iter().map(|c| self.lay_curve(si, c, LayAs::default())).collect();
        let Some(to) = self.sketches[si].entity(copies[0]).map(crate::model::curve_ref_of) else { return false };
        let id = self.alloc_id();
        let s = &mut self.sketches[si];
        s.constraints.push(Constraint::CurveDistance { from, to, d: d.abs(), off: 0.0, expr: String::new(), driven: false, at: None });
        s.offsets.push(SketchOffset { id, source: eid, side: OffsetSide::of(d), copies, cuts: Vec::new() });
        true
    }

    /// THE COPY EACH PIECE OF AN OFFSET IS LAID ON AGAIN: the copy nearest it - its ends and its middle to the piece's -
    /// each copy taken once, the copy carrying the dimension kept while a piece is laid; `None` for a piece more than the
    /// copies there are. Taken in order, the pieces of an offset cut where its loops were came round in another order once
    /// the source was dragged, and each copy was laid on the other's piece - one ran 40 mm across the drawing.
    fn pieces_onto_copies(&self, si: usize, copies: &[Id], pieces: &[Curve]) -> Vec<Option<Id>> {
        let s = &self.sketches[si];
        let was: Vec<Option<Curve>> = copies.iter().map(|id| s.entity(*id).and_then(|e| Curve::of(s, e))).collect();
        let apart = |a: &Curve, b: &Curve| [0.0, 0.5, 1.0].iter().map(|&t| a.at(t).dist(b.at(t))).sum::<f64>();
        let mut onto: Vec<Option<Id>> = vec![None; pieces.len()];
        let mut taken = vec![false; copies.len()];
        // the nearest pairs first, as long as both are free
        let mut pairs: Vec<Pairing> =
            pieces.iter().enumerate().flat_map(|(n, p)| was.iter().enumerate().filter_map(move |(c, w)| w.as_ref().map(|w| Pairing { piece: n, copy: c, apart: apart(p, w) }))).collect();
        pairs.sort_by(|a, b| a.apart.total_cmp(&b.apart));
        for Pairing { piece, copy, .. } in pairs {
            if onto[piece].is_none() && !taken[copy] {
                onto[piece] = Some(copies[copy]);
                taken[copy] = true;
            }
        }
        // the copy carrying the dimension goes on the nearest piece left without a copy, where it was not taken
        if let (Some(false), Some(free)) = (taken.first().copied(), onto.iter().position(Option::is_none)) {
            onto[free] = copies.first().copied();
        }
        onto
    }

    /// THE JOINTS OF AN OFFSET OF A CHAIN OF LINES AND A CURVE, laid once the copies are: where an end of the source of an
    /// offset (`SketchOffset`) is an end of a line of the same chain, the copy of the line and the copy of the curve are
    /// made to meet. Where they cross, the copy of the curve is cut at the crossing - again at every laying - and the end of
    /// the line held on it; where they part, an arc about the joint closes the gap, as the offset of a chain of lines
    /// rounds a corner; where they meet already, as at a smooth joint, nothing is laid. `before` - the curves of the sketch
    /// before the offset: the copies are the rest. Answers whether a joint was laid.
    pub(crate) fn join_offset_chain(&mut self, si: usize, before: &std::collections::HashSet<Id>, d: f64) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let at = |id: Id| s.point(id).map(|q| Point2::new(q.x, q.y));
        // the copies of lines laid by this offset, by the ends they have
        let lines: Vec<Id> = s.entities.iter().filter(|e| !before.contains(&e.id) && e.kind.as_line().is_some()).map(|e| e.id).collect();
        struct Joint {
            k: usize,
            end: CopyEnd,
            line: Id,
            point: Id,
            source_end: Id,
            copy_end: Id,
        }
        let mut joints: Vec<Joint> = Vec::new();
        for (k, o) in s.offsets.iter().enumerate().filter(|(_, o)| o.copies.iter().all(|c| !before.contains(c))) {
            let Some(source) = s.entity(o.source) else { continue };
            let Some(ends) = entity_ends(source) else { continue };
            for (end, j) in [CopyEnd::Start, CopyEnd::Finish].into_iter().zip(ends) {
                let Some(jp) = at(j) else { continue };
                let copy = match end {
                    CopyEnd::Start => o.copies.first(),
                    CopyEnd::Finish => o.copies.last(),
                };
                let Some(copy_end) = copy.and_then(|c| s.entity(*c)).and_then(entity_ends).map(|e| if end == CopyEnd::Start { e[0] } else { e[1] }) else { continue };
                // the end of a copy of a line standing the offset away from the joint
                let found = lines.iter().find_map(|l| {
                    let e = s.entity(*l)?.kind.as_line()?;
                    [e.a, e.b].into_iter().find(|p| at(*p).is_some_and(|q| (q.dist(jp) - d.abs()).abs() < 1e-6 * (1.0 + d.abs()))).map(|point| LineEnd { line: *l, point })
                });
                let Some(LineEnd { line, point }) = found else { continue };
                joints.push(Joint { k, end, line, point, source_end: j, copy_end });
            }
        }
        let mut laid = false;
        for Joint { k, end, line, point, source_end, copy_end } in joints {
            let s = &self.sketches[si];
            let at = |id: Id| s.point(id).map(|q| Point2::new(q.x, q.y));
            let (Some(lp), Some(cp), Some(jp)) = (at(point), at(copy_end), at(source_end)) else { continue };
            if lp.dist(cp) < 1e-6 {
                continue; // they meet already: a smooth joint
            }
            let cut = OffsetCut { end, line, point };
            let mut pieces: Vec<Curve> = s.offsets[k].copies.iter().filter_map(|c| s.entity(*c).and_then(|e| Curve::of(s, e))).collect();
            let was = pieces.clone();
            cut_at_the_line(s, &mut pieces, &cut);
            let crossed = pieces.iter().zip(&was).any(|(a, b)| a.ends() != b.ends());
            if crossed {
                // the end of the line no longer square to the end of its source: on the copy, where the two cross
                let on = s.entity(match end {
                    CopyEnd::Start => s.offsets[k].copies[0],
                    CopyEnd::Finish => *s.offsets[k].copies.last().unwrap_or(&0),
                });
                let Some(on) = on.map(crate::model::curve_ref_of) else { continue };
                let sc = &mut self.sketches[si];
                sc.constraints.retain(|c| !matches!(*c, Constraint::Perpendicular { a, b, .. } if a == source_end && b == point));
                sc.constraints.push(Constraint::PointOnCurve { p: point, on });
                sc.offsets[k].cuts.push(cut);
                laid = true;
            } else {
                // they part: an arc about the joint from the end of the line to the end of the copy, the short way
                let (u, v) = (lp - jp, cp - jp);
                let ccw = u.x * v.y - u.y * v.x > 0.0;
                let id = self.alloc_id();
                self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::ArcParts { center: source_end, a: point, b: copy_end, ccw }.into(), construction: false });
                laid = true;
            }
        }
        laid
    }

    /// LAY EVERY OFFSET OF SKETCH `si` AGAIN from its source as it stands, at the distance of its dimension: each copy on
    /// its own points in their order (`relay_curve`), a copy more where the offset now comes in more pieces. An offset
    /// whose source cannot be offset at that distance stays as it was.
    pub(crate) fn offsets_follow(&mut self, si: usize) -> Followed {
        let mut out = Followed::default();
        let Some(s) = self.sketches.get(si) else { return out };
        // what each offset is laid as now: its place among the offsets and the pieces of the true offset
        struct Laying {
            k: usize,
            pieces: Vec<Curve>,
        }
        let layings: Vec<Laying> = s
            .offsets
            .iter()
            .enumerate()
            .filter_map(|(k, o)| {
                let d = s.offset_dim(o).and_then(|ci| s.constraints[ci].dim_value())?;
                let source = s.entity(o.source).and_then(|e| Curve::of(s, e))?;
                let mut pieces = source.offset(o.side.signed(d));
                for cut in &o.cuts {
                    cut_at_the_line(s, &mut pieces, cut);
                }
                (!pieces.is_empty()).then_some(Laying { k, pieces })
            })
            .collect();
        for Laying { k, pieces } in layings {
            let copies = self.sketches[si].offsets[k].copies.clone();
            let onto = self.pieces_onto_copies(si, &copies, &pieces);
            let mut kept = Vec::with_capacity(pieces.len());
            for (piece, copy) in pieces.iter().zip(&onto) {
                match copy {
                    Some(copy) => {
                        let again = self.relay_curve(si, *copy, piece);
                        out.moved.extend(again.points);
                        out.relaid |= again.renumbered;
                        kept.push(*copy);
                    }
                    None => {
                        kept.push(self.lay_curve(si, piece, LayAs::default()));
                        out.relaid = true;
                    }
                }
            }
            // A COPY OFF ITS PIECE: the copies share their ends where the pieces meet, and once the pieces change how they
            // meet - a piece of the true offset parting in two at a fold a drag brought in - a copy laid on a new piece moved
            // a shared end the copy beside it put back, and ran across the drawing to it (12 mm off the distance on a closed
            // spline of 12 nodes offset 3 inwards). The copies of the offset are laid anew then, the dimension going to the
            // first of them.
            let s = &self.sketches[si];
            let off_its_piece =
                pieces.iter().zip(&kept).any(|(piece, copy)| s.entity(*copy).and_then(|e| Curve::of(s, e)).is_none_or(|c| c.ends().iter().zip(piece.ends()).any(|(a, b)| a.dist(b) > 1e-6)));
            if off_its_piece {
                let old: Vec<Id> = copies.iter().chain(&kept).copied().collect();
                self.sketches[si].entities.retain(|e| !old.contains(&e.id));
                kept = pieces.iter().map(|c| self.lay_curve(si, c, LayAs::default())).collect();
                let to = self.sketches[si].entity(kept[0]).map(crate::model::curve_ref_of);
                if let (Some(first), Some(to)) = (copies.first(), to) {
                    for c in self.sketches[si].constraints.iter_mut() {
                        if let Constraint::CurveDistance { to: on, .. } = c {
                            if on.curve_id() == Some(*first) {
                                *on = to.clone();
                            }
                        }
                    }
                }
                out.relaid = true;
            }
            // the copy carrying the dimension stays first; the copies no piece came onto go
            kept.sort_by_key(|id| copies.iter().position(|c| c == id).unwrap_or(usize::MAX));
            let gone: Vec<Id> = copies.iter().filter(|c| !kept.contains(c)).copied().collect();
            if !gone.is_empty() {
                self.sketches[si].entities.retain(|e| !gone.contains(&e.id));
                out.relaid = true;
            }
            self.sketches[si].offsets[k].copies = kept;
        }
        // the end of each line of a chain cut against a copy, at the end of the copy: where the two cross
        let me = &*self;
        let held: Vec<HeldEnd> = me.sketches[si].offsets.iter().flat_map(|o| o.cuts.iter().filter_map(move |c| me.copy_end_at(si, o, c))).collect();
        for HeldEnd { point, at } in held {
            if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == point) {
                (q.x, q.y) = (at.x, at.y);
                out.moved.push(point);
            }
        }
        if out.relaid {
            self.prune_orphan_sketch_points(si);
        }
        out
    }

    /// Where the end `cut` names of the copy of offset `o` stands, for the end of the line held at it.
    fn copy_end_at(&self, si: usize, o: &SketchOffset, cut: &OffsetCut) -> Option<HeldEnd> {
        let s = self.sketches.get(si)?;
        let copy = match cut.end {
            CopyEnd::Start => o.copies.first(),
            CopyEnd::Finish => o.copies.last(),
        }?;
        let c = Curve::of(s, s.entity(*copy)?)?;
        let [a, b] = c.ends();
        Some(HeldEnd { point: cut.point, at: if cut.end == CopyEnd::Start { a } else { b } })
    }
}
