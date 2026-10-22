//! TWO CURVES OF A SKETCH MADE TO TOUCH, the constraint chosen by their kinds in one place: a line and a round curve by
//! the line touching the circle (`Tangent`), two round curves by their centres (`CircleTangent`), any other pair by
//! the curves touching where they run the same way (`TangentCurves`). The slot, the tool of touching, the automatic
//! tangency, the tangent arc and the fillet all take what `tangency` gives.
use super::*;

/// How far a place on a line may stand past its end and still be read as on it.
const ON_THE_LINE: f64 = 1e-6;

/// How near a curve comes to touching a line drawn by it, to be made to touch: a share of the radius of its bend where it
/// runs along the line - 6 %, as a line was taken to touch a circle.
const NEARLY: f64 = 0.06;

/// How far past the ends of a line drawn by a curve the place it runs along it may stand, in lengths of the line.
const PAST_THE_LINE: f64 = 0.1;

/// How many even places a curve is looked along for where it runs along a line.
const ALONG_LOOKS: usize = 64;

impl Curve {
    /// HOW FAR THE CURVE STANDS FROM TOUCHING THE STRAIGHT THROUGH `line`, as a share of the radius of its bend, where it
    /// runs along that straight within the line - a tenth of it past either end allowed; `None` where it runs along it
    /// nowhere there. A circle a line passes at its radius gives nought.
    pub fn off_touching(&self, line: [Point2; 2]) -> Option<f64> {
        let [a, b] = line;
        let run = b - a;
        let len = run.x.hypot(run.y);
        if len < 1e-9 || self.straight() {
            return None;
        }
        let n = Point2::new(-run.y / len, run.x / len);
        let off = |t: f64| {
            let q = self.at(t);
            (q.x - a.x) * n.x + (q.y - a.y) * n.y
        };
        let places = (self.spans().max(1) * ALONG_LOOKS / 8).max(ALONG_LOOKS);
        let at = |k: usize| k as f64 / places as f64;
        let mut best: Option<f64> = None;
        for k in 1..places {
            let (l, m, r) = (off(at(k - 1)), off(at(k)), off(at(k + 1)));
            if (m - l) * (r - m) > 0.0 {
                continue;
            }
            // the place the distance turns, by golden sections between the places either side
            let sign = if m >= l { -1.0 } else { 1.0 };
            let (mut lo, mut hi) = (at(k - 1), at(k + 1));
            let g = 0.618_033_988_749_895;
            for _ in 0..40 {
                let (x1, x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
                if sign * off(x1) < sign * off(x2) {
                    hi = x2;
                } else {
                    lo = x1;
                }
            }
            let t = 0.5 * (lo + hi);
            let q = self.at(t);
            let along = ((q.x - a.x) * run.x + (q.y - a.y) * run.y) / (len * len);
            let bend = self.bend_radius(t);
            if !(-PAST_THE_LINE..=1.0 + PAST_THE_LINE).contains(&along) || !bend.is_finite() || bend < 1e-9 {
                continue;
            }
            let share = off(t).abs() / bend;
            if best.is_none_or(|b| share < b) {
                best = Some(share);
            }
        }
        best
    }

    /// THE RADIUS OF THE BEND OF THE CURVE AT PLACE `t`: its radius for a round curve, from its first and second
    /// derivatives for any other; infinite on a straight.
    fn bend_radius(&self, t: f64) -> f64 {
        let h = 1e-5;
        let held = |x: f64| if self.closed() { x.rem_euclid(1.0) } else { x.clamp(0.0, 1.0) };
        let (v0, v1, v2) = (self.velocity(held(t - h)), self.velocity(held(t)), self.velocity(held(t + h)));
        let acc = Point2::new((v2.x - v0.x) / (2.0 * h), (v2.y - v0.y) / (2.0 * h));
        let speed = v1.x.hypot(v1.y);
        let cross = (v1.x * acc.y - v1.y * acc.x).abs();
        if cross < 1e-12 {
            return f64::INFINITY;
        }
        speed.powi(3) / cross
    }
}

/// THE WAY A CURVE GOES ON PAST ITS END: the curve, and the unit way along its tangent there, out of the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WayOn {
    pub eid: Id,
    pub way: Point2,
}

impl Project {
    /// THE CURVE OF SKETCH `si` ENDING AT `p` AND THE WAY IT GOES ON PAST THAT END, along its tangent - a line, an arc, an
    /// arc of an ellipse, a spline alike: what a tangent arc started there goes on from. `None` where no open curve ends
    /// at `p`.
    pub fn way_on_from_end(&self, si: usize, p: Point2) -> Option<WayOn> {
        let s = self.sketches.get(si)?;
        s.entities.iter().filter(|e| !e.construction).find_map(|e| {
            let c = Curve::of(s, e).filter(|c| !c.closed())?;
            let [a, b] = c.ends();
            let way = if b.dist(p) < 1e-6 {
                c.tangent(1.0)
            } else if a.dist(p) < 1e-6 {
                mul(c.tangent(0.0), -1.0)
            } else {
                return None;
            };
            Some(WayOn { eid: e.id, way })
        })
    }

    /// THE CURVE OF SKETCH `si` A LINE DRAWN FROM `line[0]` TO `line[1]` NEARLY TOUCHES, to be made to touch it by the
    /// automatic tangency: of the curves of any kind but straights and `skip`, the one the line passes nearest to touching,
    /// within `NEARLY` of the radius of its bend. `None` where it nearly touches none.
    pub fn nearly_touching(&self, si: usize, line: [Point2; 2], skip: Id) -> Option<Id> {
        let s = self.sketches.get(si)?;
        s.entities
            .iter()
            .filter(|e| e.id != skip)
            .filter_map(|e| Curve::of(s, e).and_then(|c| c.off_touching(line)).map(|share| (e.id, share)))
            .filter(|(_, share)| *share < NEARLY)
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|(id, _)| id)
    }

    /// THE CONSTRAINTS THAT MAKE CURVES `one` AND `other` OF SKETCH `si` TOUCH: the touching itself, chosen by the kinds of
    /// the two, with the place it stands at now, and an end of either put on the other where the place is past that end
    /// - not where the end is already a point of the other, as at a fillet. Empty where either is no curve of the sketch.
    pub fn tangency(&self, si: usize, [one, other]: [Id; 2]) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let (Some(e1), Some(e2)) = (s.entity(one), s.entity(other)) else { return Vec::new() };
        let (r1, r2) = (curve_ref_of(e1), curve_ref_of(e2));
        let at = |id: Id| s.point(id).map(|q| Point2::new(q.x, q.y));
        let radius = |e: &SketchEntity| e.kind.circle_by(&at).map(|c| c.r);
        match (&r1, &r2) {
            (CurveRef::Line { a, b }, CurveRef::Round { c }) | (CurveRef::Round { c }, CurveRef::Line { a, b }) => {
                let round = if matches!(r1, CurveRef::Line { .. }) { e2 } else { e1 };
                let (Some(r), Some(pa), Some(pb), Some(pc)) = (radius(round), at(*a), at(*b), at(*c)) else { return Vec::new() };
                let mut out = vec![Constraint::Tangent { a: *a, b: *b, c: *c, r }];
                // the place the line touches the circle: the foot of the centre on it; past an end, that end goes on the
                // circle, unless it is a point of the round curve already
                let run = pb - pa;
                let t = ((pc.x - pa.x) * run.x + (pc.y - pa.y) * run.y) / (run.x * run.x + run.y * run.y).max(1e-30);
                let own = entity_points(round);
                let end = if t < -ON_THE_LINE {
                    Some(*a)
                } else if t > 1.0 + ON_THE_LINE {
                    Some(*b)
                } else {
                    None
                };
                out.extend(end.filter(|p| !own.contains(p)).map(|p| Constraint::PointOnCircle { p, c: *c }));
                out
            }
            // two straights do not touch: they cross, or run along one another
            (CurveRef::Line { .. }, CurveRef::Line { .. }) => Vec::new(),
            (CurveRef::Round { c: c1 }, CurveRef::Round { c: c2 }) => {
                let (Some(ra), Some(rb), Some(p1), Some(p2)) = (radius(e1), radius(e2), at(*c1), at(*c2)) else { return Vec::new() };
                let d = p1.dist(p2);
                vec![Constraint::CircleTangent { c1: *c1, c2: *c2, external: ((ra + rb) - d).abs() <= ((ra - rb).abs() - d).abs() }]
            }
            _ => self.touching_curves(si, [e1, e2]),
        }
    }

    /// TWO CURVES OF ANY KIND MADE TO TOUCH where they run the same way: the touching with the place it stands at now, and
    /// an end of either put on the other where the place is past that end.
    fn touching_curves(&self, si: usize, [e1, e2]: [&SketchEntity; 2]) -> Vec<Constraint> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let (r1, r2) = (curve_ref_of(e1), curve_ref_of(e2));
        // TWO CURVES SHARING AN END TOUCH THERE, the ways of the two held alike at it: a tangent arc gone on from a spline,
        // the arc of a fillet. Sought along themselves, the touching of an arc with the spline it went on from was found
        // at 0.93 of the spline, and the joint stood 0.075 rad apart.
        // The row of a touching at an end reads whose end it is and the way the curve goes there, so both are taken as
        // drawn, between their ends (`curve_ref_drawn`): an arc of an ellipse taken whole had no ends and leaned 0.8 off
        // touching the arc of its fillet.
        let shared = entity_ends(e1).and_then(|a| entity_ends(e2).and_then(|b| a.into_iter().find(|p| b.contains(p))));
        if let (Some(p), Some(c1), Some(c2)) = (shared, Curve::of(s, e1), Curve::of(s, e2)) {
            let q = s.point(p).map(|q| Point2::new(q.x, q.y));
            let near = q.map(|q| [c1.nearest(q).t, c2.nearest(q).t]);
            return vec![Constraint::TangentCurves { one: curve_ref_drawn(e1), other: curve_ref_drawn(e2), near, end: Some(p) }];
        }
        let near = crate::solver::touching_places(&s.points, &self.entity_radii(si), [&s.resolved(&r1), &s.resolved(&r2)], None);
        // the end the touching comes past, of the first of the two it comes past: one end held on the other curve is the
        // whole of a touching at an end, and both held were one more than the two can meet
        let past = |e: &SketchEntity, place: f64| {
            let [start, finish] = entity_ends(e)?;
            if place < 0.0 {
                Some(start)
            } else if place > 1.0 {
                Some(finish)
            } else {
                None
            }
        };
        let end = near.and_then(|[t1, t2]| past(e1, t1).map(|p| (p, r2.clone(), e2)).or_else(|| past(e2, t2).map(|p| (p, r1.clone(), e1))));
        let end_id = end.as_ref().map(|(p, _, _)| *p);
        // the end held on the other curve - not where it is a point of the other already, as at the joint of a tangent arc
        let held = end.filter(|(p, _, other)| !entity_points(other).contains(p)).map(|(p, on, _)| Constraint::PointOnCurve { p, on });
        std::iter::once(Constraint::TangentCurves { one: r1, other: r2, near, end: end_id }).chain(held).collect()
    }
}
