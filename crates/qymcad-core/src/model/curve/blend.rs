//! THE CUT OF A CORNER TWO CURVES OF ANY KIND MAKE: where a fillet of a radius touches them, and where a chamfer of its
//! legs cuts them. A corner is two curves leaving one point, each from one of its ends.
//!
//! The centre of a fillet stands at the radius from both curves: it is where their offsets by the radius meet, on the
//! sides that face each other, narrowed by Newton's steps on the two places it touches - the offset of a spline is itself
//! laid within `OFFSET_TOLERANCE`, which would leave the arc 0.01 mm off the curve. A leg of a chamfer is measured
//! straight from the corner, as on a line.
use super::*;

/// ONE SIDE OF A CORNER: a curve and the end of it standing on the corner.
#[derive(Clone, Copy, Debug)]
pub struct Arm<'a> {
    pub curve: &'a Curve,
    pub end: End,
}

/// A FILLET OF A CORNER: the centre of its arc and the places it touches the two curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rounding {
    pub centre: Point2,
    pub at: [f64; 2],
}

/// A PIECE OF A CURVE LEAVING A CROSSING, as a cut there would leave it: the piece and its end standing on the crossing.
#[derive(Clone, Debug, PartialEq)]
pub struct Half {
    pub curve: Curve,
    pub end: End,
}

impl Half {
    /// The piece as a side of a corner on the crossing.
    pub fn arm(&self) -> Arm<'_> {
        Arm { curve: &self.curve, end: self.end }
    }
}

impl Curve {
    /// THE PIECES OF THE CURVE LEAVING `at`, where `other` crosses it, as a cut of the crossing leaves them: an open curve
    /// in two there - one piece where it ends there - a closed one cut at every place `other` meets it, so the two pieces
    /// at `at` run to the meetings either side. Nothing for a closed curve met at one place alone, which no cut takes.
    pub fn halves_at(&self, other: &Curve, at: Point2) -> Vec<Half> {
        let t = self.nearest(at).t;
        let half = |run: [f64; 2], end: End| Half { curve: self.piece(run), end };
        if !self.closed() {
            let mut out = Vec::new();
            if t < 1.0 - 1e-9 {
                out.push(half([t, 1.0], End::Start));
            }
            if t > 1e-9 {
                out.push(half([0.0, t], End::Finish));
            }
            return out;
        }
        let mut places: Vec<f64> = self.meet(other).iter().map(|m| m.ta).collect();
        places.sort_by(f64::total_cmp);
        places.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        // met at one place alone, the curve is cut there and opposite (`cut_at_crossing`)
        if let [p] = places[..] {
            places.push((p + 0.5).rem_euclid(1.0));
            places.sort_by(f64::total_cmp);
        }
        let (Some(&first), Some(&last)) = (places.first(), places.last()) else { return Vec::new() };
        if places.len() < 2 {
            return Vec::new();
        }
        let next = places.iter().copied().find(|p| *p > t + 1e-9).unwrap_or(first + 1.0);
        let prev = places.iter().copied().rev().find(|p| *p < t - 1e-9).unwrap_or(last - 1.0);
        let back = if prev < 0.0 { [prev + 1.0, t + 1.0] } else { [prev, t] };
        vec![half([t, next], End::Start), half(back, End::Finish)]
    }
}

/// THE RADIUS A FILLET OF `size` TAKES IN THE CORNER of the two arms: a chord or an arc length makes it on the turn between
/// the ways they leave the corner.
pub fn radius_in(arms: [Arm; 2], size: FilletSize) -> Option<f64> {
    if size.by == FilletBy::Radius {
        return Some(size.value);
    }
    let [u, v] = [arms[0].leaving(), arms[1].leaving()];
    size.radius_on(std::f64::consts::PI - (u.x * v.x + u.y * v.y).clamp(-1.0, 1.0).acos())
}

/// THE CENTRES OF THE CIRCLES OF RADIUS `r` TOUCHING BOTH CURVES: where their offsets by the radius meet, on either side
/// of each. A line is taken as the whole straight line it lies on: a circle touching a line touches it anywhere along it.
pub fn centres_touching(curves: [&Curve; 2], r: f64) -> Vec<Point2> {
    if r <= 1e-9 {
        return Vec::new();
    }
    let whole = |c: &Curve| match c {
        Curve::Line { a, b } => {
            let u = c.tangent(0.0);
            Curve::Line { a: *a - mul(u, WHOLE_LINE), b: *b + mul(u, WHOLE_LINE) }
        }
        other => other.clone(),
    };
    let [a, b] = curves.map(whole);
    let mut out = Vec::new();
    for da in [-r, r] {
        for db in [-r, r] {
            for oa in a.offset(da) {
                for ob in b.offset(db) {
                    out.extend(oa.meet(&ob).into_iter().map(|m| standing_off([&a, &b], r, m.at)));
                }
            }
        }
    }
    out
}

/// THE CENTRE `r` OFF BOTH CURVES, from `m` near it, by Newton's steps on the two distances: the offset of a spline or an
/// ellipse is laid within `OFFSET_TOLERANCE` of the true one, and the centre where two such offsets meet stood as far
/// off - a circle of 3 touching a spline and a line came out of radius 2.999998.
fn standing_off(curves: [&Curve; 2], r: f64, mut m: Point2) -> Point2 {
    for _ in 0..8 {
        let [fa, fb] = curves.map(|c| c.nearest(m));
        let [ua, ub] = [fa, fb].map(|f| {
            let away = m - f.point;
            mul(away, 1.0 / away.x.hypot(away.y).max(1e-12))
        });
        let (ga, gb) = (fa.off - r, fb.off - r);
        let det = ua.x * ub.y - ua.y * ub.x;
        if det.abs() < 1e-12 {
            break;
        }
        let step = Point2::new((ga * ub.y - gb * ua.y) / det, (ua.x * gb - ub.x * ga) / det);
        m = m - step;
        if step.x.hypot(step.y) < 1e-12 {
            break;
        }
    }
    m
}

/// How far a line is drawn on past each end to stand for the whole straight line it lies on, in mm: past the largest
/// sheet a sketch is drawn on.
const WHOLE_LINE: f64 = 1e4;

/// How many points a curve is looked along from its end for the place a leg reaches.
const LOOKS: usize = 400;

impl Arm<'_> {
    /// The place of the end on the corner.
    pub fn place(&self) -> f64 {
        match self.end {
            End::Start => 0.0,
            End::Finish => 1.0,
        }
    }

    /// The corner point.
    pub fn corner(&self) -> Point2 {
        self.curve.at(self.place())
    }

    /// THE WAY THE CURVE LEAVES THE CORNER, a unit vector into it.
    pub fn leaving(&self) -> Point2 {
        let u = self.curve.tangent(self.place());
        match self.end {
            End::Start => u,
            End::Finish => mul(u, -1.0),
        }
    }

    /// THE PLACE `d` STRAIGHT FROM THE CORNER, the first one looking along the curve from it; `None` where the curve does
    /// not reach that far.
    pub fn reach(&self, d: f64) -> Option<f64> {
        let corner = self.corner();
        let from = self.place();
        let at = |k: usize| from + (1.0 - 2.0 * from) * k as f64 / LOOKS as f64;
        let k = (1..=LOOKS).find(|k| self.curve.at(at(*k)).dist(corner) >= d)?;
        let (mut lo, mut hi) = (at(k - 1), at(k));
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if self.curve.at(mid).dist(corner) >= d {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(hi)
    }

    /// Whether place `t` lies inside the curve and off its corner end.
    fn holds(&self, t: f64) -> bool {
        (-1e-9..=1.0 + 1e-9).contains(&t) && (t - self.place()).abs() > 1e-9
    }
}

/// The left normal of a unit direction.
fn left(u: Point2) -> Point2 {
    Point2::new(-u.y, u.x)
}

/// THE FILLET OF RADIUS `r` IN THE CORNER the two arms make: of the centres at the radius from both, the one touching
/// both curves nearest the corner. `None` where the radius does not fit.
pub fn rounding(arms: [Arm; 2], r: f64) -> Option<Rounding> {
    if r <= 1e-9 {
        return None;
    }
    let corner = arms[0].corner();
    let mut best: Option<Rounding> = None;
    for s0 in [-1.0, 1.0] {
        for s1 in [-1.0, 1.0] {
            let sides = [s0 * r, s1 * r];
            let starts: Vec<Point2> = arms[0].curve.offset(sides[0]).iter().flat_map(|a| arms[1].curve.offset(sides[1]).into_iter().flat_map(move |b| a.meet(&b)).map(|m| m.at)).collect();
            for start in starts {
                let Some(found) = settle_centre(arms, sides, start) else { continue };
                if !(arms[0].holds(found.at[0]) && arms[1].holds(found.at[1])) {
                    continue;
                }
                let nearer = best.is_none_or(|b| found.centre.dist(corner) < b.centre.dist(corner) - 1e-9);
                if nearer {
                    best = Some(found);
                }
            }
        }
    }
    best
}

/// The centre at the signed distances `sides` to the left of both curves, by Newton's steps on the two places it touches
/// from the places nearest `start`.
fn settle_centre(arms: [Arm; 2], sides: [f64; 2], start: Point2) -> Option<Rounding> {
    let off = |k: usize, t: f64| {
        let c = arms[k].curve;
        let t = t.clamp(0.0, 1.0);
        let n = left(c.tangent(t));
        c.at(t) + mul(n, sides[k])
    };
    let mut t = [arms[0].curve.nearest(start).t, arms[1].curve.nearest(start).t];
    let h = 1e-7;
    for _ in 0..50 {
        let f = off(0, t[0]) - off(1, t[1]);
        if f.x.hypot(f.y) < 1e-11 {
            break;
        }
        let d0 = mul(off(0, t[0] + h) - off(0, t[0] - h), 0.5 / h);
        let d1 = mul(off(1, t[1] + h) - off(1, t[1] - h), -0.5 / h);
        let det = d0.x * d1.y - d0.y * d1.x;
        if det.abs() < 1e-18 {
            return None;
        }
        t[0] -= (f.x * d1.y - f.y * d1.x) / det;
        t[1] -= (d0.x * f.y - d0.y * f.x) / det;
        if !t.iter().all(|x| x.is_finite()) {
            return None;
        }
    }
    let gap = off(0, t[0]) - off(1, t[1]);
    (gap.x.hypot(gap.y) < 1e-8 && t.iter().all(|x| (-1e-9..=1.0 + 1e-9).contains(x))).then(|| Rounding { centre: off(0, t[0]), at: t.map(|x| x.clamp(0.0, 1.0)) })
}

/// THE PLACES A CHAMFER OF `legs` CUTS THE TWO ARMS AT: a symmetric one as far from the corner on both, its cut as long
/// as asked; two legs each straight from the corner; a leg on the first and the cut at the angle from the first's way to
/// the corner. `None` where a curve does not reach as far.
pub fn chamfer(arms: [Arm; 2], legs: ChamferLegs) -> Option<[f64; 2]> {
    // a leg of no length cuts nothing, and an angle of none or of a half turn cuts along a side
    let leg_of_none = legs.first <= 1e-9 || (legs.mode == crate::feature::ChamferMode::TwoDist && legs.second <= 1e-9);
    let flat = legs.mode == crate::feature::ChamferMode::DistAngle && !(legs.second > 0.0 && legs.second < 180.0);
    if leg_of_none || flat {
        return None;
    }
    let corner = arms[0].corner();
    match legs.mode {
        crate::feature::ChamferMode::TwoDist => Some([arms[0].reach(legs.first)?, arms[1].reach(legs.second)?]),
        crate::feature::ChamferMode::Symmetric => {
            // the cut grows with the legs near the corner: the leg whose cut is as long as asked, by halving
            let cut = |d: f64| -> Option<f64> { Some(arms[0].curve.at(arms[0].reach(d)?).dist(arms[1].curve.at(arms[1].reach(d)?))) };
            let (mut lo, mut hi) = (0.0, legs.first);
            // a leg as long as the cut need not fit: the cut across a corner is longer than its legs, and a cut of 16 on a
            // square corner of a side 12 was refused for its leg of 16, where its legs are 11.3. The longest leg both arms
            // reach is found by halving, and the cut must fit within it
            if cut(hi).is_none() {
                let (mut fits, mut not) = (0.0, hi);
                for _ in 0..60 {
                    let mid = 0.5 * (fits + not);
                    if cut(mid).is_some() {
                        fits = mid;
                    } else {
                        not = mid;
                    }
                }
                if cut(fits)? < legs.first {
                    return None;
                }
                hi = fits;
            }
            while cut(hi)? < legs.first {
                lo = hi;
                hi *= 2.0;
            }
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if cut(mid)? >= legs.first {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            Some([arms[0].reach(hi)?, arms[1].reach(hi)?])
        }
        crate::feature::ChamferMode::DistAngle => {
            let t0 = arms[0].reach(legs.first)?;
            let p0 = arms[0].curve.at(t0);
            let back = corner - p0;
            let want = legs.second.to_radians();
            // the angle at the end on the first curve between its way to the corner and the cut, growing with the leg
            // along the second
            let angle = |d: f64| -> Option<f64> {
                let p1 = arms[1].curve.at(arms[1].reach(d)?) - p0;
                Some(((back.x * p1.x + back.y * p1.y) / (back.x.hypot(back.y) * p1.x.hypot(p1.y)).max(1e-18)).clamp(-1.0, 1.0).acos())
            };
            let (mut lo, mut hi) = (0.0, legs.first);
            while angle(hi)? < want {
                lo = hi;
                hi *= 2.0;
            }
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if angle(mid)? >= want {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            Some([t0, arms[1].reach(hi)?])
        }
    }
}
