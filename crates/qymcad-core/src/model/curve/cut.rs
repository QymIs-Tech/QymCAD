//! A CURVE OF A SKETCH CUT INTO PIECES OF ITS OWN KIND, laid in its stead: a line holds its pieces on its straight and
//! halves what held its ends level or upright, an arc or a circle lays arcs about its own centre, any other curve is cut
//! by `cut_curve`. Trim and break call `keep_pieces` and `break_into_pieces` and do not tell the kinds apart.
use super::*;

/// WHAT A TRIM LEAVES OF A CURVE: the runs that stay, the run that goes, and the click that named it.
pub(crate) struct Trimmed {
    pub(crate) kept: Vec<[f64; 2]>,
    pub(crate) run: [f64; 2],
    pub(crate) click: Point2,
}

impl Project {
    /// PUT THE RUNS OF CURVE `eid` THAT A TRIM KEEPS IN ITS STEAD, as pieces of its own kind, and settle the sketch.
    pub(crate) fn keep_pieces(&mut self, si: usize, eid: Id, curve: &Curve, trimmed: Trimmed) -> bool {
        let Trimmed { kept, run: [from, to], click } = trimmed;
        match curve {
            Curve::Line { .. } => self.keep_line_pieces(si, eid, curve, &kept),
            Curve::Round { .. } => match self.round_pieces(si, eid, curve, &kept) {
                Some(ends) => {
                    // the ends of the arcs are anchored to the crossing curves, so a cut point does not drift along the
                    // angle on the next solve
                    for p in ends {
                        self.anchor_cut_point(si, p);
                    }
                    self.settle_after_cut(si);
                    true
                }
                None => false,
            },
            _ => self.trim_piece(si, eid, curve, [from, to], click),
        }
    }
    /// PUT THE PIECES `kept` OF LINE `eid` IN ITS STEAD, as places along it, and hold them as one line: the inner cut
    /// points on the straight through the outermost points, each cut point on the curve crossing it there.
    fn keep_line_pieces(&mut self, si: usize, eid: Id, line: &Curve, kept: &[[f64; 2]]) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let con = self.sketches[si].entity(eid).is_some_and(|e| e.construction);
        self.sketches[si].entities.retain(|e| e.id != eid);
        let mut order: Vec<Id> = Vec::new();
        for run in kept {
            let [q0, q1] = line.piece(*run).ends();
            let (p0, p1) = (self.sketch_point_at(si, q0.x, q0.y, 1e-9), self.sketch_point_at(si, q1.x, q1.y, 1e-9));
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a: p0, b: p1 }.into(), construction: con });
            if order.last() != Some(&p0) {
                order.push(p0);
            }
            order.push(p1);
        }
        // The pieces of one line stay on one straight line: the interior cut points are held by `PointOnLine`
        // against the line through the outermost points, which the solver cannot collapse (the residual is
        // divided by the length). Otherwise editing a dimension spreads the pieces into a parallelogram and the
        // notch or arc flies away.
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
        for pi in order.into_iter().filter(|&pi| pi != a && pi != b) {
            self.anchor_cut_point(si, pi);
        }
        self.settle_after_cut(si);
        true
    }
    /// PUT THE PIECES `runs` OF CIRCLE OR ARC `eid` IN ITS STEAD, as places along it: arcs about its own centre, each the
    /// way its piece runs. Answers the ends of the arcs; `None` where `eid` is no circle or arc of the sketch.
    fn round_pieces(&mut self, si: usize, eid: Id, round: &Curve, runs: &[[f64; 2]]) -> Option<Vec<Id>> {
        let s = self.sketches.get(si)?;
        let e = s.entity(eid)?;
        let (con, center) = (e.construction, e.kind.circle_by(&|id| s.point(id).map(|q| Point2::new(q.x, q.y)))?.center);
        self.sketches[si].entities.retain(|x| x.id != eid);
        let mut ends = Vec::new();
        for run in runs {
            let piece = round.piece(*run);
            let Curve::Round { sweep, .. } = piece else { continue };
            let [q0, q1] = piece.ends();
            let (pa, pb) = (self.sketch_point_at(si, q0.x, q0.y, 1e-6), self.sketch_point_at(si, q1.x, q1.y, 1e-6));
            let id = self.alloc_id();
            self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::ArcParts { center, a: pa, b: pb, ccw: sweep > 0.0 }.into(), construction: con });
            ends.extend([pa, pb]);
        }
        Some(ends)
    }
    /// BREAK LINE `eid` INTO TWO at place `tc` along it, the halves held as the line was.
    fn break_line_at(&mut self, si: usize, eid: Id, tc: f64) -> bool {
        let Some((a, b)) = self.line_ends(si, eid) else { return false };
        let (Some((pax, pay)), Some((pbx, pby))) = (self.point_xy(si, a), self.point_xy(si, b)) else { return false };
        let dlen2 = (pbx - pax).powi(2) + (pby - pay).powi(2);
        if dlen2 < 1e-12 {
            return false;
        }
        let mid = self.sketch_point_at(si, pax + (pbx - pax) * tc, pay + (pby - pay) * tc, 1e-9);
        let con = self.sketches[si].entities.iter().find(|e| e.id == eid).is_some_and(|e| e.construction);
        // THE FIRST HALF IS THE LINE IT WAS, the second a line added: what was built from the line - the wall swept from
        // it, and every rounding and sketch on that wall - goes on naming it. Both halves new, the wall took the name
        // of a half: measured on the case of the examples, breaking the first line of each of its 32 sketches renamed
        // 59 faces that did not move.
        if let Some(line) = self.sketches[si].entities.iter_mut().find(|e| e.id == eid) {
            line.kind = crate::model::LineParts { a, b: mid }.into();
        }
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind: crate::model::LineParts { a: mid, b }.into(), construction: con });
        // THE HALVES KEEP WHAT THE LINE WAS HELD BY: a horizontal or a vertical of its ends becomes one of each half. Left
        // as it stood, it held the far ends only, and the break point was free to leave the line - two constraints and
        // seven freedoms where there are three and six.
        let halves = |c: &Constraint| -> Option<[Constraint; 2]> {
            match *c {
                Constraint::Horizontal { a: x, b: y } if (x == a && y == b) || (x == b && y == a) => Some([Constraint::Horizontal { a, b: mid }, Constraint::Horizontal { a: mid, b }]),
                Constraint::Vertical { a: x, b: y } if (x == a && y == b) || (x == b && y == a) => Some([Constraint::Vertical { a, b: mid }, Constraint::Vertical { a: mid, b }]),
                _ => None,
            }
        };
        let old = std::mem::take(&mut self.sketches[si].constraints);
        for c in old {
            match halves(&c) {
                Some(two) => self.sketches[si].constraints.extend(two),
                None => self.sketches[si].constraints.push(c),
            }
        }
        self.settle_after_cut(si);
        true
    }
    /// The cut of `break_curve_here`: a line in two halves held as the line was, an arc or a circle into arcs about its
    /// own centre, any other curve into pieces of its own kind.
    pub(crate) fn break_into_pieces(&mut self, si: usize, eid: Id, curve: &Curve, click: Point2) -> bool {
        let mut places = self.break_at(si, eid, click);
        places.sort_by(f64::total_cmp);
        match curve {
            Curve::Line { .. } => places.first().is_some_and(|t| self.break_line_at(si, eid, *t)),
            Curve::Round { .. } => {
                let runs = match places[..] {
                    [t] => vec![[0.0, t], [t, 1.0]],
                    [t0, t1] => vec![[t0, t1], [t1, t0 + 1.0]],
                    _ => return false,
                };
                if self.round_pieces(si, eid, curve, &runs).is_none() {
                    return false;
                }
                self.settle_after_cut(si);
                true
            }
            _ => self.break_piece(si, eid, click),
        }
    }
}
