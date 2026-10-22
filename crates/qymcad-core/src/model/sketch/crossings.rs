//! WHERE THE OTHER CURVES OF A SKETCH MEET A CURVE OF IT, and the trim, the break and the extension of a curve of any kind
//! built on that and on the pieces of a curve (`Curve::piece`, `Project::cut_curve`).

use super::*;
use crate::model::curve::{Curve, End};

/// WHAT TRIM DOES AT A CLICK: the curve, and the run of it that goes - `None` when the whole of it goes.
pub(crate) struct TrimCut {
    pub(crate) curve: Curve,
    pub(crate) run: Option<[f64; 2]>,
}

/// ANOTHER CURVE OF THE SKETCH, by its id.
struct Other {
    id: Id,
    curve: Curve,
}

/// A PIECE OF A CURVE LEAVING A CROSSING, and the curve it is a piece of.
struct Side {
    of: Id,
    half: crate::model::curve::Half,
}

/// A HALF BEYOND A CORNER THAT GOES IN PART: the curve, and the place of the next crossing on it.
struct Beyond {
    eid: Id,
    t: f64,
}

/// ONE CURVE OF A CROSSING TO CUT: the entity, its curve, the curve it crosses, and the crossing.
struct CutThrough<'a> {
    eid: Id,
    me: &'a Curve,
    them: &'a Curve,
    at: Point2,
}

/// A PLACE WHERE ANOTHER CURVE MEETS A CURVE: the place along it (`Curve` places, 0 to 1), the point, and the curve met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub t: f64,
    pub at: Point2,
    pub by: Id,
}

impl Project {
    /// THE PLACES ON `curve` WHERE THE CURVES OF SKETCH `si` MEET IT - construction geometry included, a boundary as
    /// well - but entity `skip`, along it in order.
    pub(crate) fn meetings(&self, si: usize, curve: &Curve, skip: Id) -> Vec<Crossing> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let mut out: Vec<Crossing> = s
            .entities
            .iter()
            .filter(|e| e.id != skip)
            .filter_map(|e| Curve::of(s, e).map(|c| (e.id, c)))
            .flat_map(|(id, other)| curve.meet(&other).into_iter().map(move |m| Crossing { t: m.ta, at: m.at, by: id }))
            .collect();
        out.sort_by(|a, b| a.t.total_cmp(&b.t));
        out
    }

    /// THE CROSSING OF TWO CURVES NEAREST `at`, within `reach` of it: two curves of the drawing (not construction) going
    /// through each other where neither has a point the other ends at - or one ending on the middle of the other.
    /// Where both end there, they already share a corner and there is no crossing to find. Only the curves passing
    /// within `reach` of `at` are asked where they meet.
    pub fn crossing_near(&self, si: usize, at: Point2, reach: f64) -> Option<LinesCrossing> {
        let s = self.sketches.get(si)?;
        let near: Vec<Id> = s.entities.iter().filter(|e| !e.construction).filter(|e| Curve::of(s, e).is_some_and(|c| c.nearest(at).off <= reach)).map(|e| e.id).collect();
        near.iter()
            .enumerate()
            .flat_map(|(i, &one)| near[i + 1..].iter().filter_map(move |&other| self.crossing_of(si, [one, other], at)))
            .filter(|c| c.at.dist(at) <= reach)
            .min_by(|x, y| x.at.dist(at).total_cmp(&y.at.dist(at)))
    }

    /// WHERE CURVES `one` AND `other` CROSS, the crossing nearest `near` of those they have: through each other, or one
    /// ending on the middle of the other. Curves that both end there make a corner, not a crossing.
    pub fn crossing_of(&self, si: usize, [one, other]: [Id; 2], near: Point2) -> Option<LinesCrossing> {
        let s = self.sketches.get(si)?;
        let curve = |id: Id| s.entities.iter().find(|e| e.id == id).and_then(|e| Curve::of(s, e));
        let (a, b) = (curve(one)?, curve(other)?);
        let inside = |c: &Curve, t: f64| c.closed() || (t > 1e-9 && t < 1.0 - 1e-9);
        a.meet(&b).into_iter().filter(|m| inside(&a, m.ta) || inside(&b, m.tb)).min_by(|x, y| x.at.dist(near).total_cmp(&y.at.dist(near))).map(|m| LinesCrossing { one, other, at: m.at })
    }

    /// THE TWO CURVES OF `crossing` CUT WHERE THEY CROSS, each that goes on past it: a line in two (`break_line`), any
    /// other open curve into its two pieces there, a closed one at every place it meets the other curve - cut at the one
    /// place alone it would be a single piece both of whose ends stand there. The halves share one point at the crossing:
    /// the corners of a crossing are then corners of the drawing, rounded and cut as any are. Answers that point.
    pub fn cut_at_crossing(&mut self, si: usize, crossing: LinesCrossing) -> Option<Id> {
        let LinesCrossing { one, other, at } = crossing;
        let s = self.sketches.get(si)?;
        let curve = |id: Id| s.entities.iter().find(|e| e.id == id).and_then(|e| Curve::of(s, e));
        let (a, b) = (curve(one)?, curve(other)?);
        if !(self.cut_through(si, CutThrough { eid: one, me: &a, them: &b, at }) && self.cut_through(si, CutThrough { eid: other, me: &b, them: &a, at })) {
            return None;
        }
        self.merge_close_points(si, 1e-9);
        let s = self.sketches.get(si)?;
        // the point the halves end at: a point of the origin may stand on the same place, and is not stitched to them
        let half_end = |p: Id| s.entities.iter().any(|e| !e.construction && entity_ends(e).is_some_and(|ends| ends.contains(&p)));
        s.points.iter().filter(|q| (q.x - at.x).hypot(q.y - at.y) < 1e-6).map(|q| q.id).find(|&p| half_end(p))
    }

    /// WHAT THE CORNER OF A CROSSING WOULD BE, changing nothing: the two curves of `crossing` as `cut_at_crossing` would
    /// leave them, and the cut of the corner of the quarter `toward` stands in - the two pieces either side of it, of the
    /// two curves. What the preview of a corner at a crossing draws before the crossing is cut.
    pub fn crossing_blend(&self, si: usize, crossing: LinesCrossing, toward: Point2, cut: CornerCut) -> Option<CornerBlend> {
        let LinesCrossing { one, other, at } = crossing;
        let s = self.sketches.get(si)?;
        let curve = |id: Id| s.entities.iter().find(|e| e.id == id).and_then(|e| Curve::of(s, e));
        let (a, b) = (curve(one)?, curve(other)?);
        let mut sides: Vec<Side> = a.halves_at(&b, at).into_iter().map(|half| Side { of: one, half }).chain(b.halves_at(&a, at).into_iter().map(|half| Side { of: other, half })).collect();
        let angle = |u: Point2| u.y.atan2(u.x);
        sides.sort_by(|x, y| angle(x.half.arm().leaving()).total_cmp(&angle(y.half.arm().leaving())));
        let n = sides.len();
        if n < 2 {
            return None;
        }
        // the piece before the pointer and the piece after it, round the turn
        let k = sides.partition_point(|x| angle(x.half.arm().leaving()) <= angle(toward - at)) % n;
        let before = (k + n - 1) % n;
        let (first, second) = (&sides[before], &sides[k]);
        if first.of == second.of {
            return None;
        }
        blend_of_arms([first.half.arm(), second.half.arm()], cut)
    }

    /// THE HALVES OF A CROSSING BEYOND ITS CORNER GO, once `corner` is cut, as far as the next crossing: the halves still
    /// ending at the point the crossing was cut at are those of the other quarters - the two sides of the corner end at
    /// its arc or its cut line now. As in the other CAD of the trade a corner of a crossing leaves its two sides alone;
    /// kept, the halves stood joined at the corner point, held to it, and a drag of the corner pulled them round.
    ///
    /// A HALF GOES AS FAR AS IT MEETS ANOTHER CURVE: a spline crossing a circle twice loses the run to the second crossing
    /// and keeps its tail beyond it; a half ending at a crossing or at a free end goes whole. A half whose far end only
    /// joins a side of the corner is the rest of a closed curve crossed once - a circle a spline ends inside - and stays.
    /// Reported behaviour: a fillet at one crossing of a spline and a circle took the half of the spline beyond it with
    /// the second crossing on it. Answers how many halves went, whole or in part.
    pub fn drop_halves_beyond_corner(&mut self, si: usize, corner: CornerAt) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let Some(p) = s.point(corner.point).map(|q| Point2::new(q.x, q.y)) else { return 0 };
        let halves: Vec<Id> = s.entities.iter().filter(|e| !e.construction && entity_ends(e).is_some_and(|ends| ends.contains(&corner.point))).map(|e| e.id).collect();
        let mut whole: Vec<Id> = Vec::new();
        let mut parts: Vec<Beyond> = Vec::new();
        for eid in halves {
            let s = &self.sketches[si];
            let Some(c) = s.entity(eid).and_then(|e| Curve::of(s, e)) else { continue };
            let from_start = c.at(0.0).dist(p) < 1e-6;
            let inside: Vec<f64> = self.meetings(si, &c, eid).into_iter().map(|m| m.t).filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6).collect();
            let next = if from_start { inside.iter().copied().reduce(f64::min) } else { inside.iter().copied().reduce(f64::max) };
            match next {
                Some(t) => parts.push(Beyond { eid, t }),
                None if self.joins_a_side_alone(si, eid, if from_start { c.at(1.0) } else { c.at(0.0) }, corner) => {}
                None => whole.push(eid),
            }
        }
        let went = whole.len() + parts.len();
        for Beyond { eid, t } in parts {
            let pieces = self.cut_curve(si, eid, &[t]);
            let s = &self.sketches[si];
            let near: Vec<Id> = pieces.into_iter().filter(|id| s.entity(*id).and_then(|e| Curve::of(s, e)).is_some_and(|c| c.ends().iter().any(|q| q.dist(p) < 1e-6))).collect();
            whole.extend(near);
        }
        if went > 0 {
            self.delete_entities(si, &whole);
            self.solve_sketch(si);
        }
        went
    }

    /// WHETHER THE FAR END `far` OF HALF `eid` ONLY JOINS A SIDE OF `corner`: one other curve stands there, ending there, and
    /// it is a side - the half is the rest of the closed curve that side is a piece of.
    fn joins_a_side_alone(&self, si: usize, eid: Id, far: Point2, corner: CornerAt) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let there: Vec<&SketchEntity> = s.entities.iter().filter(|e| e.id != eid && !e.construction).filter(|e| Curve::of(s, e).is_some_and(|c| c.nearest(far).off < 1e-6)).collect();
        let ends_there = |e: &SketchEntity| Curve::of(s, e).is_some_and(|c| !c.closed() && c.ends().iter().any(|q| q.dist(far) < 1e-6));
        let [one] = there.as_slice() else { return false };
        ends_there(one) && (one.id == corner.pair.0 || one.id == corner.pair.1)
    }

    /// ONE CURVE OF A CROSSING CUT THERE, where it goes on past it: a line in two, any other open curve into its two
    /// pieces, a closed one at every place it meets the other. Answers false where a closed curve meets the other at one
    /// place alone.
    fn cut_through(&mut self, si: usize, cut: CutThrough) -> bool {
        let CutThrough { eid, me, them, at } = cut;
        if !me.closed() && me.ends().iter().any(|p| p.dist(at) < 1e-9) {
            return true;
        }
        if !me.closed() {
            return self.break_curve(si, eid, at.x, at.y);
        }
        // a closed curve met at one place alone is cut there and opposite, so the two halves at the crossing are curves with
        // ends of their own
        let mut places: Vec<f64> = me.meet(them).iter().map(|m| m.ta).collect();
        if let [t] = places[..] {
            places.push((t + 0.5).rem_euclid(1.0));
        }
        if places.len() < 2 {
            return false;
        }
        self.cut_curve(si, eid, &places);
        true
    }

    /// THE PLACES ON CURVE `eid` OF SKETCH `si` WHERE ITS OTHER CURVES MEET IT, in order along it.
    pub fn crossings_on(&self, si: usize, eid: Id) -> Vec<Crossing> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(c) = s.entities.iter().find(|e| e.id == eid).and_then(|e| Curve::of(s, e)) else { return Vec::new() };
        self.meetings(si, &c, eid)
    }

    /// WHAT TRIM DOES AT A CLICK ON CURVE `eid`: the curve, and the run of it under the click between the two crossings on
    /// either side of it - or an end - which goes; `None` for the run when the whole curve goes, crossed nowhere, or a
    /// closed one crossed once. The run of a closed curve may pass its start: its end is then past 1.
    pub(crate) fn trim_cut(&self, si: usize, eid: Id, click: Point2) -> Option<TrimCut> {
        let s = self.sketches.get(si)?;
        let curve = s.entities.iter().find(|e| e.id == eid).and_then(|e| Curve::of(s, e))?;
        let mut cuts: Vec<f64> = self.crossings_on(si, eid).into_iter().map(|x| x.t).collect();
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        let tc = curve.nearest(click).t;
        let run = if curve.closed() {
            (cuts.len() >= 2).then(|| {
                // the crossings either side of the click, round the start where the click lies past the last or before the
                // first
                let after = cuts.iter().copied().find(|t| *t > tc).unwrap_or(cuts[0] + 1.0);
                let before = cuts.iter().copied().rev().find(|t| *t <= tc).unwrap_or(cuts[cuts.len() - 1] - 1.0);
                let from = before.rem_euclid(1.0);
                [from, from + (after - before)]
            })
        } else {
            cuts.retain(|t| *t > 1e-9 && *t < 1.0 - 1e-9);
            (!cuts.is_empty()).then(|| [cuts.iter().copied().rev().find(|t| *t <= tc).unwrap_or(0.0), cuts.iter().copied().find(|t| *t > tc).unwrap_or(1.0)])
        };
        Some(TrimCut { curve, run })
    }

    /// THE PIECE OF CURVE `eid` A TRIM AT THE CLICK TAKES AWAY - the whole curve when it goes whole. What the preview of
    /// the trim draws, from the places the trim cuts at.
    pub fn trim_span(&self, si: usize, eid: Id, click: Point2) -> Option<Curve> {
        let TrimCut { curve, run } = self.trim_cut(si, eid, click)?;
        Some(match run {
            Some(run) => curve.piece(run),
            None => curve,
        })
    }

    /// TRIM CURVE `eid` OF A KIND THAT HOLDS NOTHING OF ITS OWN - an ellipse, an arc of one, a spline - taking away its
    /// run `[from, to]` under the click: it is cut at both ends of the run and the piece holding the click goes.
    pub(crate) fn trim_piece(&mut self, si: usize, eid: Id, curve: &Curve, [from, to]: [f64; 2], click: Point2) -> bool {
        // an open curve is cut where the run starts and ends inside it; a closed one at both, round its start
        let places: Vec<f64> = if curve.closed() { vec![from, to.rem_euclid(1.0)] } else { [from, to].into_iter().filter(|t| *t > 1e-9 && *t < 1.0 - 1e-9).collect() };
        let pieces = self.cut_curve(si, eid, &places);
        // the piece holding the click is the one that goes
        if let Some(g) = pieces.iter().copied().find(|id| self.piece_holds(si, *id, click)) {
            self.delete_entities(si, &[g]);
        }
        self.settle_after_cut(si);
        true
    }

    /// Whether piece `eid` is the one of its curve nearest to `click` - the piece a click there trims.
    fn piece_holds(&self, si: usize, eid: Id, click: Point2) -> bool {
        let Some(s) = self.sketches.get(si) else { return false };
        let off = |e: &SketchEntity| Curve::of(s, e).map_or(f64::MAX, |c| c.nearest(click).off);
        let Some(me) = s.entities.iter().find(|e| e.id == eid) else { return false };
        let mine = off(me);
        // among the curves of the sketch, none nearer to the click
        s.entities.iter().all(|e| e.id == eid || off(e) >= mine)
    }

    /// WHERE A BREAK AT THE CLICK CUTS CURVE `eid`: an open curve at the place nearest the click, no nearer its ends than a
    /// twentieth of it, a closed one there and opposite it, into two halves. What the preview of the break draws, and
    /// where the break cuts.
    pub fn break_at(&self, si: usize, eid: Id, click: Point2) -> Vec<f64> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(c) = s.entities.iter().find(|e| e.id == eid).and_then(|e| Curve::of(s, e)) else { return Vec::new() };
        let t = c.nearest(click).t;
        if c.closed() {
            vec![t, (t + 0.5).rem_euclid(1.0)]
        } else {
            vec![t.clamp(0.05, 0.95)]
        }
    }

    /// BREAK CURVE `eid` OF ANY KIND AT THE CLICK, where `break_at` says.
    pub(crate) fn break_piece(&mut self, si: usize, eid: Id, click: Point2) -> bool {
        let places = self.break_at(si, eid, click);
        if places.is_empty() || self.cut_curve(si, eid, &places).is_empty() {
            return false;
        }
        self.settle_after_cut(si);
        true
    }

    /// HOW CURVE `eid` GOES AS THE EXTEND TOOL ASKS: each end asked taken to the curve the pointer is over when its way
    /// meets that curve, else to the nearest curve on its way - a line along its straight, an arc round its circle, an arc
    /// of an ellipse round its ellipse, a spline by a straight span along the tangent of its end; an end whose way meets
    /// nothing stays. One end - the one whose way meets the curve under the pointer, else the one nearer the pointer - or
    /// both. `None` for a closed curve or no curve of the sketch.
    ///
    /// AN END JOINED TO OTHER GEOMETRY STAYS (`end_held`): moved, it pulled what it is joined to, and the solve pulled it
    /// back. Reported behaviour: Extend on the side two rectangles share, its end the end of a fillet, moved the drawing
    /// (the ends of the fillet by 5e-6 mm, enough to lose the face of the rectangle), extended nothing and said it was
    /// done. A spline grows by a node past its end, which does not move, and is never kept.
    pub fn curve_extension(&self, si: usize, eid: Id, ask: &ExtendAsk) -> Option<CurveExtension> {
        let s = self.sketches.get(si)?;
        let c = Curve::of(s, s.entity(eid)?)?;
        if c.closed() {
            return None;
        }
        let others: Vec<Other> = s.entities.iter().filter(|o| o.id != eid).filter_map(|o| Curve::of(s, o).map(|curve| Other { id: o.id, curve })).collect();
        // the curve each end goes to: the one under the pointer when its way meets it, else the nearest on its way
        let target = |end: End| {
            let met: Vec<Other> = others.iter().filter_map(|o| c.stretched(end, &o.curve).map(|longer| Other { id: o.id, curve: longer })).collect();
            let under = met.iter().find(|m| Some(m.id) == ask.over);
            under.or(met.iter().min_by(|x, y| x.curve.length().total_cmp(&y.curve.length()))).map(|m| m.id)
        };
        let [start, finish] = c.ends();
        let asked: Vec<End> = match ask.sides {
            ExtendSides::Both => vec![End::Start, End::Finish],
            ExtendSides::Nearer => {
                let meets_under = |end: End| ask.over.is_some() && target(end) == ask.over;
                let nearer = if ask.pointer.dist(start) < ask.pointer.dist(finish) { End::Start } else { End::Finish };
                vec![if meets_under(End::Finish) {
                    End::Finish
                } else if meets_under(End::Start) {
                    End::Start
                } else {
                    nearer
                }]
            }
        };
        let moves = s.entity(eid)?.kind.as_spline().is_none();
        let mut out = CurveExtension { curve: c.clone(), goes: Vec::new(), kept: EndKept::Free };
        for end in asked {
            let Some(to) = target(end).and_then(|id| others.iter().find(|o| o.id == id)) else { continue };
            if moves && self.end_point_of(si, eid, end).is_some_and(|p| self.end_held(si, eid, p)) {
                out.kept = EndKept::Joined;
                continue;
            }
            if let Some(longer) = out.curve.stretched(end, &to.curve) {
                out.curve = longer;
                out.goes.push(end);
            }
        }
        Some(out)
    }

    /// CURVE `eid` AS AN EXTENSION AT THE CLICK LEAVES IT, stretched from its end nearer the click to the nearest curve on
    /// its way; `None` when nothing goes. What the preview of the extension under the pointer draws.
    pub fn extended(&self, si: usize, eid: Id, click: Point2) -> Option<Curve> {
        self.curve_extension(si, eid, &ExtendAsk { pointer: click, over: None, sides: ExtendSides::Nearer }).filter(|x| !x.goes.is_empty()).map(|x| x.curve)
    }

    /// EXTEND CURVE `eid` AS `ext` SAYS: the ends that go move to where the curve now ends, free of what placed them
    /// where they stood; a spline gains a node past each end that goes. Answers whether an end went.
    pub fn extend_curve_by(&mut self, si: usize, eid: Id, ext: &CurveExtension) -> bool {
        self.cut_projection(si, eid, |p| p.extend_curve_by_here(si, eid, ext))
    }
    fn extend_curve_by_here(&mut self, si: usize, eid: Id, ext: &CurveExtension) -> bool {
        if ext.goes.is_empty() {
            return false;
        }
        if let Some(s) = self.sketches.get_mut(si) {
            s.let_go_the_copy(eid);
        }
        let Some(e) = self.sketches.get(si).and_then(|s| s.entity(eid)) else { return false };
        let reached = |end: End| if end == End::Start { ext.curve.at(0.0) } else { ext.curve.at(1.0) };
        let spline = e.kind.as_spline().map(|sp| (sp.nodes.to_vec(), sp.ends));
        match spline {
            Some((mut nodes, ends)) => {
                let Some(handles) = ext.curve.handles() else { return false };
                for &end in &ext.goes {
                    let at = reached(end);
                    let id = self.alloc_id();
                    self.sketches[si].points.push(SketchPoint { id, x: at.x, y: at.y });
                    if end == End::Start {
                        nodes.insert(0, id);
                    } else {
                        nodes.push(id);
                    }
                }
                if let Some(x) = self.sketches[si].entities.iter_mut().find(|x| x.id == eid) {
                    x.kind = EntityKind::spline(nodes, handles, ends);
                }
            }
            None => {
                for &end in &ext.goes {
                    let Some(id) = self.end_point_of(si, eid, end) else { continue };
                    let at = reached(end);
                    self.lift_the_place_of(si, id);
                    if let Some(q) = self.sketches[si].points.iter_mut().find(|q| q.id == id) {
                        q.x = at.x;
                        q.y = at.y;
                    }
                }
            }
        }
        self.solve_sketch(si);
        true
    }

    /// THE POINT AT `end` OF CURVE `eid`: its first end along it, or its last (`entity_ends`). Looked up by the place of
    /// the end, an arc of an ellipse from 0 to 90 deg took the end of its minor axis standing on its last end, and Extend
    /// moved the axis.
    fn end_point_of(&self, si: usize, eid: Id, end: End) -> Option<Id> {
        let [first, last] = entity_ends(self.sketches.get(si)?.entity(eid)?)?;
        Some(if end == End::Start { first } else { last })
    }
}
