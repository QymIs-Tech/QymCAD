//! THE ROWS OF THE CONSTRAINTS ON CURVES OF ANY KIND: a point on a curve and two curves touching, where the curve is a
//! spline or an ellipse with no equation a pair of points satisfies. Each is one row - how far the point stands off the
//! curve, how far apart the curves stand where they run the same way - read off the curve as it stands at the variables
//! of the solve, and differentiated by central differences over the points the curves stand on: the rows are found by
//! a search along the curves, and their derivatives written out would be those of the search.
use super::*;
use crate::geom::Point2;
use crate::model::curve::Curve;
use crate::model::CurveRef;

/// How far a line is looked along past its ends, in lengths of it, for the place a curve touches it: a line is touched
/// on the straight it lies on, as every sketcher reads it.
const LINE_REACH: f64 = 10.0;

/// How many places of each curve the search for where two curves come nearest starts from.
const LOOKS: usize = 64;

/// THE VARIABLES OF A SOLVE, read as points and radii.
pub(crate) struct Now<'a> {
    pub(crate) x: &'a [f64],
    pub(crate) idx: &'a HashMap<Id, usize>,
    pub(crate) ridx: &'a HashMap<Id, usize>,
}

impl Now<'_> {
    fn point(&self, id: Id) -> Option<Point2> {
        self.idx.get(&id).map(|&i| Point2::new(self.x[2 * i], self.x[2 * i + 1]))
    }

    /// THE CURVE OF `on` where its points stand now: a line between its ends, or on the straight it lies on where `reach`
    /// says so, a circle or an arc whole on its circle, an ellipse whole, a spline as drawn.
    fn curve(&self, on: &CurveRef, reach: Reach) -> Option<Curve> {
        let c = Curve::of_ref(on, &|id| self.point(id), &|c| self.ridx.get(&c).map(|&i| self.x[i]))?;
        Some(if reach == Reach::Straight { c.reaching(LINE_REACH) } else { c })
    }
}

/// HOW FAR A LINE REACHES: between its ends, or along the whole straight it lies on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Ends,
    Straight,
}

/// The left normal of the curve at place `t`.
fn normal(c: &Curve, t: f64) -> Point2 {
    let u = c.tangent(t);
    Point2::new(-u.y, u.x)
}

/// HOW FAR POINT `p` STANDS OFF CURVE `on`, to its left positive.
pub(crate) fn point_off(now: &Now, p: Id, on: &CurveRef) -> f64 {
    foot(now, p, on).map_or(0.0, |f| point_off_at(now, p, on, f))
}

/// WHERE A POINT IS MEASURED TO A CURVE: the place nearest it, and the way the distance is read there - the normal of the
/// curve, or, where the place is an end of an open curve, the way from that end to the point. Read along the normal at an
/// end, a point 11 mm off the end of a spline along its tangent stood nought off it.
#[derive(Clone, Copy)]
struct Foot {
    t: f64,
    way: Point2,
}

fn foot(now: &Now, p: Id, on: &CurveRef) -> Option<Foot> {
    let c = now.curve(on, Reach::Straight)?;
    let q = now.point(p)?;
    let at = c.nearest(q);
    let at_end = !c.closed() && (at.t < 1e-9 || at.t > 1.0 - 1e-9) && at.off > 1e-12;
    let way = if at_end { Point2::new((q.x - at.point.x) / at.off, (q.y - at.point.y) / at.off) } else { normal(&c, at.t) };
    Some(Foot { t: at.t, way })
}

/// How far point `p` stands off curve `on` at its foot: the distance itself where the foot is the nearest place, and its
/// derivatives there those of the distance - the place moves along the curve, square to the way it is read.
fn point_off_at(now: &Now, p: Id, on: &CurveRef, f: Foot) -> f64 {
    let (Some(q), Some(c)) = (now.point(p), now.curve(on, Reach::Straight)) else { return 0.0 };
    let at = c.at(f.t);
    (q.x - at.x) * f.way.x + (q.y - at.y) * f.way.y
}

/// How far an open curve is read on past each of its ends, along its tangent there, in lengths of the curve.
const GONE_ON: f64 = 3.0;

/// A CURVE GONE ON PAST ITS ENDS along its tangents there, `GONE_ON` lengths of it each way: an open curve read for where
/// it would touch another, the places past its ends running below 0 and above 1. A closed curve is itself.
struct Gone {
    c: Curve,
    len: f64,
    /// how far it is gone on past each end, in lengths of it: `GONE_ON` read for a touching, nought for a distance
    on: f64,
}

impl Gone {
    fn new(c: Curve, on: f64) -> Gone {
        let len = c.length().max(1e-9);
        Gone { c, len, on }
    }

    /// The places the curve is read over.
    fn span(&self) -> [f64; 2] {
        if self.c.closed() {
            [0.0, 1.0]
        } else {
            [-self.on, 1.0 + self.on]
        }
    }

    fn at(&self, t: f64) -> Point2 {
        let c = &self.c;
        if c.closed() {
            return c.at(t.rem_euclid(1.0));
        }
        let past = |end: f64, by: f64| {
            let (q, u) = (c.at(end), c.tangent(end));
            Point2::new(q.x + u.x * by * self.len, q.y + u.y * by * self.len)
        };
        if t < 0.0 {
            past(0.0, t)
        } else if t > 1.0 {
            past(1.0, t - 1.0)
        } else {
            c.at(t)
        }
    }

    fn tangent(&self, t: f64) -> Point2 {
        if self.c.closed() {
            self.c.tangent(t.rem_euclid(1.0))
        } else {
            self.c.tangent(t.clamp(0.0, 1.0))
        }
    }

    /// The left normal at place `t`.
    fn normal(&self, t: f64) -> Point2 {
        let u = self.tangent(t);
        Point2::new(-u.y, u.x)
    }

    /// The place nearest `q`, on the curve or on its tangent past an end.
    fn nearest(&self, q: Point2) -> f64 {
        let c = &self.c;
        let at = c.nearest(q);
        if c.closed() {
            return at.t;
        }
        let along = |end: f64| {
            let (e, u) = (c.at(end), c.tangent(end));
            ((q.x - e.x) * u.x + (q.y - e.y) * u.y) / self.len
        };
        if self.on > 0.0 && at.t < 1e-9 && along(0.0) < 0.0 {
            return along(0.0).max(-self.on);
        }
        if self.on > 0.0 && at.t > 1.0 - 1e-9 && along(1.0) > 0.0 {
            return 1.0 + along(1.0).min(self.on);
        }
        at.t
    }
}

/// WHERE TWO CURVES ARE READ AS TOUCHING: the curve read to, the curve read along, and the place on each.
struct Touching<'a> {
    to: &'a CurveRef,
    along: &'a CurveRef,
    places: [f64; 2],
    /// how far the curves are read gone on past their ends (`Gone::on`)
    on: f64,
    /// whether the curve read to is the second of the constraint
    swapped: bool,
}

/// HOW FAR APART TWO CURVES STAND WHERE THEY RUN THE SAME WAY: the distance from the second to the first, signed by the
/// side of the first, taken where it stops growing or shrinking along the second - there the second runs the way the
/// first does. Of those places, the one standing nearest. Touching, it is nought; apart, the gap; through each other,
/// the depth they cross by - not nought, as the distance at a crossing is.
///
/// An open curve is read gone on past its ends: two splines whose nearest places are their ends run the same way nowhere
/// on themselves, and read as they are they met end to end and stood still, the ends 52 deg apart. Where the touching
/// comes past an end, the tool puts that end on the other curve, as it does the end of a line.
///
/// The simpler of the two is read along - a line, a circle, an ellipse, a spline.
pub(crate) fn gap(now: &Now, pair: [&CurveRef; 2], near: Option<[f64; 2]>) -> f64 {
    touching(now, pair, near, GONE_ON).map_or(0.0, |t| gap_at(now, &t))
}

/// THE ROW OF TWO CURVES TOUCHING: the gap where they touch along themselves; where they touch at an end, how far the way
/// of the curve at that end turns off the way of the other where the end stands - the end itself is held on the other by
/// a constraint of its own. Read by the gap there, the end lying on the other curve leaves the gap growing with the square
/// of the turn, and a solve stopped with the ways 3.5 deg apart.
pub(crate) fn touch_row(now: &Now, pair: [&CurveRef; 2], near: Option<[f64; 2]>, end: Option<Id>) -> f64 {
    match end {
        Some(p) => end_turn(now, pair, p).unwrap_or(0.0),
        None => gap(now, pair, near),
    }
}

/// The sine of the turn between the way of the curve of the pair that ends at `p`, there, and the way of the other at the
/// place nearest `p`.
fn end_turn(now: &Now, [one, other]: [&CurveRef; 2], p: Id) -> Option<f64> {
    let ends_at = |on: &CurveRef| on.open_ends().is_some_and(|e| e.contains(&p));
    let (owner, other) = if ends_at(one) { (one, other) } else { (other, one) };
    let (c, o, q) = (now.curve(owner, Reach::Ends)?, now.curve(other, Reach::Straight)?, now.point(p)?);
    let t = if c.at(0.0).dist(q) <= c.at(1.0).dist(q) { 0.0 } else { 1.0 };
    let (u, w) = (c.tangent(t), o.tangent(o.nearest(q).t));
    Some(u.x * w.y - u.y * w.x)
}

/// WHERE TWO CURVES TOUCH, OR WOULD: a place along each of the pair, past an end of an open one below 0 or above 1 - the
/// place of touching nearest `near` where there is more than one.
pub fn touching_places(points: &[crate::model::SketchPoint], radii: &[RadiusVar], pair: [&CurveRef; 2], near: Option<[f64; 2]>) -> Option<[f64; 2]> {
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(i, r)| (r.center, points.len() * 2 + i)).collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|r| r.value));
    let now = Now { x: &x, idx: &idx, ridx: &ridx };
    let t = touching(&now, pair, near, GONE_ON)?;
    Some(if t.swapped { [t.places[1], t.places[0]] } else { t.places })
}

/// A curve read gone on `on` past its ends; read for a distance (`on` nought), a line is its whole straight, as a distance
/// to a line is measured.
fn gone(now: &Now, curve: &CurveRef, on: f64) -> Option<Gone> {
    let reach = if on == 0.0 { Reach::Straight } else { Reach::Ends };
    Some(Gone::new(now.curve(curve, reach)?, on))
}

fn touching<'a>(now: &Now, [one, other]: [&'a CurveRef; 2], near: Option<[f64; 2]>, on: f64) -> Option<Touching<'a>> {
    // the simpler is read along; `swapped` when the curve read to is the second of the pair
    let first_along = one.order() < other.order();
    let read = |swapped: bool| -> Option<Touching<'a>> {
        let (to, along) = if swapped { (other, one) } else { (one, other) };
        // the place last touched at, as [on the curve read to, on the curve read along]
        let hint = near.map(|[a, b]| if swapped { [b, a] } else { [a, b] });
        let places = turning_place([&gone(now, to, on)?, &gone(now, along, on)?], hint)?;
        Some(Touching { to, along, places, on, swapped })
    };
    read(first_along).or_else(|| read(!first_along))
}

/// The gap at the places of `t`, along the normal of the curve read to: the gap itself where the places are those
/// `touching` found, and its derivatives there those of the gap - both places stand where the gap is flat in them.
fn gap_at(now: &Now, t: &Touching) -> f64 {
    let (Some(a), Some(b)) = (gone(now, t.to, t.on), gone(now, t.along, t.on)) else { return 0.0 };
    let (qa, qb, n) = (a.at(t.places[0]), b.at(t.places[1]), a.normal(t.places[0]));
    (qb.x - qa.x) * n.x + (qb.y - qa.y) * n.y
}

/// The distance from `b` at place `t` to `a`, to the left of `a` positive, and the place on `a` nearest it.
fn off_a([a, b]: [&Gone; 2], t: f64) -> [f64; 2] {
    let q = b.at(t);
    let ta = a.nearest(q);
    let (at, n) = (a.at(ta), a.normal(ta));
    let across = (q.x - at.x) * n.x + (q.y - at.y) * n.y;
    // past an end of an open curve read as it is drawn, the whole distance to the end, on the side it stands: its part
    // across the normal there put a spline 10.2 from a circle at 4
    if !a.c.closed() && a.on == 0.0 && (ta < 1e-9 || ta > 1.0 - 1e-9) {
        return [q.dist(at).copysign(across), ta];
    }
    [across, ta]
}

/// The places on `a` and on `b` where the signed distance from `b` to `a` stops growing or shrinking along `b`: those
/// nearest `near` where it is given, else those standing nearest nought; `None` where there are none.
fn turning_place(pair: [&Gone; 2], near: Option<[f64; 2]>) -> Option<[f64; 2]> {
    let [a, b] = pair;
    let closed = b.c.closed();
    let [lo_t, hi_t] = b.span();
    // the curve itself read finely, the straights past its ends coarsely: a straight turns nowhere
    let mut places: Vec<f64> = Vec::new();
    if !closed {
        places.extend((0..8).map(|k| lo_t + (0.0 - lo_t) * k as f64 / 8.0));
    }
    let inner = if closed { LOOKS } else { LOOKS - 1 };
    places.extend((0..LOOKS).map(|k| k as f64 / inner as f64));
    if !closed {
        places.extend((1..=8).map(|k| 1.0 + (hi_t - 1.0) * k as f64 / 8.0));
    }
    let count = places.len() as isize;
    let f: Vec<f64> = places.iter().map(|t| off_a(pair, *t)[0]).collect();
    let wrap = |k: isize| k.rem_euclid(count) as usize;
    let range = if closed { 0..count } else { 1..count - 1 };
    // the place found, by how far it stands from what it is chosen by
    struct Found {
        score: f64,
        places: [f64; 2],
    }
    let mut best: Option<Found> = None;
    for k in range {
        let (l, m, r) = (f[wrap(k - 1)], f[wrap(k)], f[wrap(k + 1)]);
        let sign = if m <= l && m <= r {
            1.0
        } else if m >= l && m >= r {
            -1.0
        } else {
            continue;
        };
        // the extremum between the places either side, by golden sections: the distance is flat there in the place, so
        // 40 sections leave it exact
        let (mut lo, mut hi) = (places[wrap(k - 1)], places[wrap(k + 1)]);
        if closed && hi < lo {
            hi += 1.0;
        }
        let value = |t: f64| sign * off_a(pair, t)[0];
        let g = 0.618_033_988_749_895;
        let (mut x1, mut x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
        let (mut f1, mut f2) = (value(x1), value(x2));
        for _ in 0..40 {
            if f1 < f2 {
                hi = x2;
                (x2, f2) = (x1, f1);
                x1 = hi - g * (hi - lo);
                f1 = value(x1);
            } else {
                lo = x1;
                (x1, f1) = (x2, f2);
                x2 = lo + g * (hi - lo);
                f2 = value(x2);
            }
        }
        let tb = if f1 < f2 { x1 } else { x2 };
        let tb = if closed { tb.rem_euclid(1.0) } else { tb };
        let [v, ta] = off_a(pair, tb);
        // a turning of the distance where the place nearest on `a` leaps from one branch of it to another is a corner of
        // the distance, not a place the two run the same way: the ways there stood 0.33 deg apart, and the gap read
        // there moved off its row by 0.5 %
        let (ub, na) = (b.tangent(tb), a.normal(ta));
        if (ub.x * na.x + ub.y * na.y).abs() > 1e-6 {
            continue;
        }
        let ta = if a.c.closed() { ta.rem_euclid(1.0) } else { ta };
        // how far a place stands from the last one, round a closed curve the shorter way
        let apart = |x: f64, h: f64, round: bool| if round { ((x - h).rem_euclid(1.0)).min((h - x).rem_euclid(1.0)) } else { (x - h).abs() };
        let score = near.map_or(v.abs(), |[ha, hb]| apart(ta, ha, a.c.closed()) + apart(tb, hb, closed));
        if best.as_ref().is_none_or(|b| score < b.score) {
            best = Some(Found { score, places: [ta, tb] });
        }
    }
    best.map(|b| b.places)
}

/// THE ROW OF A DISTANCE BETWEEN A POINT OR A CURVE AND A CURVE: the least distance between them less the value.
pub(crate) fn distance_row(now: &Now, pair: [&CurveRef; 2], d: f64) -> f64 {
    least(now, pair).map_or(0.0, |m| m.value.abs() - d)
}

/// WHERE THE LEAST DISTANCE BETWEEN TWO THINGS STANDS: between a place of each where they run the same way, or from an
/// end of an open curve - or a point - to the place of the other nearest it.
enum Nearest<'a> {
    Pair(Touching<'a>),
    End { p: Id, on: &'a CurveRef, foot: Foot },
}

/// The least distance, signed by the side, and where it stands.
struct Least<'a> {
    value: f64,
    at: Nearest<'a>,
}

/// THE LEAST DISTANCE BETWEEN A POINT OR A CURVE AND A CURVE, as drawn - no curve gone on past its ends: of the places
/// two curves run the same way and of the ends of an open one, the nearest. Read past its end, the distance from a circle
/// to a spline was taken to the straight the spline goes on along, 10.2 where the spline stood 4 off.
fn least<'a>(now: &Now, [from, to]: [&'a CurveRef; 2]) -> Option<Least<'a>> {
    let end = |p: Id, on: &'a CurveRef| foot(now, p, on).map(|f| Least { value: point_off_at(now, p, on, f), at: Nearest::End { p, on, foot: f } });
    let mut found: Vec<Least<'a>> = Vec::new();
    // a point measured to a curve, either way round
    let point = from.as_point().map(|p| (p, to)).or_else(|| to.as_point().map(|p| (p, from)));
    match point {
        Some((p, on)) => found.extend(end(p, on)),
        None => {
            found.extend(touching(now, [from, to], None, 0.0).map(|t| Least { value: gap_at(now, &t), at: Nearest::Pair(t) }));
            for [curve, other] in [[from, to], [to, from]] {
                // the ends of an open curve, not a line's, read as its whole straight
                for p in curve.open_ends().filter(|_| !curve.is_straight()).into_iter().flatten() {
                    found.extend(end(p, other));
                }
            }
        }
    }
    found.into_iter().min_by(|x, y| x.value.abs().total_cmp(&y.value.abs()))
}

/// The least distance read where `at` stands - its row read at fixed places, as the touching's is.
fn least_at(now: &Now, at: &Nearest) -> f64 {
    match at {
        Nearest::Pair(t) => gap_at(now, t),
        Nearest::End { p, on, foot } => point_off_at(now, *p, on, *foot),
    }
}

/// WHERE THE LEAST DISTANCE BETWEEN A POINT OR A CURVE AND A CURVE IS MEASURED: its two ends, the first on `from` - what a
/// dimension of it is drawn between.
pub fn distance_ends(points: &[crate::model::SketchPoint], radii: &[RadiusVar], [from, to]: [&CurveRef; 2]) -> Option<[Point2; 2]> {
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(i, r)| (r.center, points.len() * 2 + i)).collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|r| r.value));
    let now = Now { x: &x, idx: &idx, ridx: &ridx };
    let found = least(&now, [from, to])?;
    match found.at {
        Nearest::End { p, on, foot } => {
            let (q, f) = (now.point(p)?, now.curve(on, Reach::Straight)?.at(foot.t));
            // which of the two the point belongs to: the end of `from` stands first
            let of_from = from.points().contains(&p) && !std::ptr::eq(on, from);
            Some(if of_from { [q, f] } else { [f, q] })
        }
        Nearest::Pair(t) => {
            let (a, b) = (gone(&now, t.to, t.on)?.at(t.places[0]), gone(&now, t.along, t.on)?.at(t.places[1]));
            Some(if t.swapped { [b, a] } else { [a, b] })
        }
    }
}

/// THE ROW OF THE CONSTRAINT `c` ON CURVES, by central differences over the coordinates of its points and the radii of
/// its circles, a step of a millionth of a millimetre, at the places on the curves found where the solve stands: the row
/// is flat in them there, so the steps need not look for them again.
pub(crate) fn numeric_row(c: &Constraint, now: &Now) -> Vec<Slope> {
    let mut vars: Vec<usize> = c.points().iter().filter_map(|id| now.idx.get(id)).flat_map(|&i| [2 * i, 2 * i + 1]).collect();
    let rounds = |on: &CurveRef| on.radius_centre().and_then(|c| now.ridx.get(&c).copied());
    match c {
        Constraint::PointOnCurve { on, .. } => vars.extend(rounds(on)),
        Constraint::TangentCurves { one, other, .. } | Constraint::CurveDistance { from: one, to: other, .. } => vars.extend(rounds(one).into_iter().chain(rounds(other))),
        _ => {}
    }
    vars.sort_unstable();
    vars.dedup();
    // the places found once, where the solve stands; each step reads the row at them
    let found_foot = match c {
        Constraint::PointOnCurve { p, on } => foot(now, *p, on),
        _ => None,
    };
    let found_touch = match c {
        Constraint::TangentCurves { one, other, near, end: None } => touching(now, [one, other], *near, GONE_ON),
        _ => None,
    };
    // a distance is read where it is least where the solve stands, on the side its curves stand at there
    let found_least = match c {
        Constraint::CurveDistance { from, to, .. } => least(now, [from, to]),
        _ => None,
    };
    let side = found_least.as_ref().map_or(1.0, |m| if m.value < 0.0 { -1.0 } else { 1.0 });
    let value = |x: &[f64]| {
        let at = Now { x, idx: now.idx, ridx: now.ridx };
        match (c, found_foot, &found_touch) {
            (Constraint::PointOnCurve { p, on }, Some(t), _) => point_off_at(&at, *p, on, t),
            (Constraint::CurveDistance { d, .. }, _, _) => found_least.as_ref().map_or(0.0, |m| side * least_at(&at, &m.at) - d),
            (Constraint::TangentCurves { .. }, _, Some(t)) => gap_at(&at, t),
            // at an end the turn is read afresh at every step: the place on the other curve moves with the end
            (Constraint::TangentCurves { one, other, end: Some(p), .. }, _, _) => end_turn(&at, [one, other], *p).unwrap_or(0.0),
            _ => 0.0,
        }
    };
    let mut x = now.x.to_vec();
    vars.into_iter()
        .map(|v| {
            let h = 1e-6 * now.x[v].abs().max(1.0);
            x[v] = now.x[v] + h;
            let up = value(&x);
            x[v] = now.x[v] - h;
            let down = value(&x);
            x[v] = now.x[v];
            Slope { var: v, by: (up - down) / (2.0 * h) }
        })
        .collect()
}

/// HOW A ROW CHANGES WITH ONE VARIABLE OF THE SOLVE: the variable and the derivative.
pub(crate) struct Slope {
    pub(crate) var: usize,
    pub(crate) by: f64,
}
