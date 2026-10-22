//! TWO CURVES OF A SKETCH MADE TO TOUCH: the constraint of touching, and where the touching would come past an end of an
//! open curve - a line, a spline - that end put on the other curve, so the two touch on themselves and not on the
//! straight one of them goes on along.
use super::*;

/// A CIRCLE OF A RADIUS TOUCHING TWO CURVES, asked for near a point: the curves, the radius, and the point the centre is
/// sought nearest to.
#[derive(Clone, Copy, Debug)]
pub struct Touching {
    pub curves: [Id; 2],
    pub r: f64,
    pub near: Point2,
}

impl Project {
    /// WHERE THE CENTRE OF THE CIRCLE OF `ask` STANDS: of the circles of its radius touching both its curves, the one
    /// whose centre is nearest its point. `None` where no circle of the radius touches both.
    pub fn circle_touching(&self, si: usize, ask: Touching) -> Option<Point2> {
        let s = self.sketches.get(si)?;
        let curve = |id: Id| s.entity(id).and_then(|e| crate::model::curve::Curve::of(s, e));
        let [a, b] = ask.curves;
        let (ca, cb) = (curve(a)?, curve(b)?);
        crate::model::curve::centres_touching([&ca, &cb], ask.r).into_iter().min_by(|x, y| x.dist(ask.near).total_cmp(&y.dist(ask.near)))
    }

    /// THE CIRCLE OF `ask` LAID and held touching both its curves (`tangency`); answers its id, `None` where no circle of
    /// the radius touches both.
    pub fn add_circle_touching(&mut self, si: usize, ask: Touching, purpose: crate::feature::Purpose) -> Option<Id> {
        let c = self.circle_touching(si, ask)?;
        let eid = self.add_circle_entity(si, c.x, c.y, ask.r, purpose);
        for other in ask.curves {
            let touch = self.tangency(si, [eid, other]);
            self.sketches[si].constraints.extend(touch);
        }
        self.solve_sketch(si);
        Some(eid)
    }

    /// WHERE DIMENSION `c` OF SKETCH `si`, A DISTANCE TO A CURVE, IS MEASURED: its two ends, the first on what it is
    /// measured from. `None` for any other constraint.
    pub fn curve_distance_ends(&self, si: usize, c: &Constraint) -> Option<[Point2; 2]> {
        let s = self.sketches.get(si)?;
        let Constraint::CurveDistance { from, to, .. } = c else { return None };
        if let Some(ends) = s.offset_dim_ends(c) {
            return Some(ends);
        }
        crate::solver::distance_ends(&s.points, &self.entity_radii(si), [&s.resolved(from), &s.resolved(to)])
    }

    /// EVERY TOUCHING OF SKETCH `si` THAT HAS COME PAST AN END of a line or a spline, held at that end: the end put on the
    /// other curve and the touching read at it. Answers whether any was. A solve moves the curves and the place they touch
    /// with them, and a line touching a spline at 0.83 of its length was carried 1.1 mm past its end.
    pub(crate) fn touchings_held_at_ends(&mut self, si: usize) -> bool {
        let radii = self.entity_radii(si);
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let solved: Vec<Constraint> = s.constraints.iter().map(|c| s.as_solved(c)).collect();
        let mut held: Vec<Constraint> = Vec::new();
        for (c, now) in s.constraints.iter_mut().zip(&solved) {
            let (Constraint::TangentCurves { one, other, near, end: end @ None }, Constraint::TangentCurves { one: one_now, other: other_now, .. }) = (c, now) else { continue };
            let Some([t1, t2]) = crate::solver::touching_places(&s.points, &radii, [one_now, other_now], *near) else { continue };
            let past = |on: &CurveRef, t: f64| {
                let [start, finish] = on.open_ends()?;
                if t < -1e-6 {
                    Some(start)
                } else if t > 1.0 + 1e-6 {
                    Some(finish)
                } else {
                    None
                }
            };
            if let Some(p) = past(one_now, t1) {
                held.push(Constraint::PointOnCurve { p, on: other.clone() });
                *end = Some(p);
            } else if let Some(p) = past(other_now, t2) {
                held.push(Constraint::PointOnCurve { p, on: one.clone() });
                *end = Some(p);
            }
        }
        let any = !held.is_empty();
        s.constraints.extend(held);
        any
    }
}
