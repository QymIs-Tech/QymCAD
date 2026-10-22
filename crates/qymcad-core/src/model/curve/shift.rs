//! A CURVE MOVED AND A CURVE OFFSET: taken through a map of the sheet - a shift, a turn, a mirror, any of them after
//! another - and laid at a distance to one side of itself.
use super::*;
use crate::feature::Ends;
use crate::geom::bspline::{Approximated, BSpline};

/// A MAP OF THE SHEET that keeps lengths: a point `p` goes to `origin + p.x * x + p.y * y`, `x` and `y` square unit
/// vectors - turned, or turned over for a mirror.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Map {
    pub origin: Point2,
    pub x: Point2,
    pub y: Point2,
}

impl Map {
    /// A shift by `by`.
    pub fn shift(by: Point2) -> Map {
        Map { origin: by, x: Point2::new(1.0, 0.0), y: Point2::new(0.0, 1.0) }
    }

    /// A turn by `angle` radians about `centre`.
    pub fn turn(centre: Point2, angle: f64) -> Map {
        let (s, c) = angle.sin_cos();
        let (x, y) = (Point2::new(c, s), Point2::new(-s, c));
        Map { origin: Point2::new(centre.x - (centre.x * x.x + centre.y * y.x), centre.y - (centre.x * x.y + centre.y * y.y)), x, y }
    }

    /// A mirror about the line through `a` and `b`.
    pub fn mirror(line: [Point2; 2]) -> Map {
        let [a, b] = line;
        let d = b - a;
        let l = d.len().max(1e-300);
        let (ux, uy) = (d.x / l, d.y / l);
        // reflection across a line through the origin of direction u: [[2ux^2-1, 2uxuy], [2uxuy, 2uy^2-1]]
        let (x, y) = (Point2::new(2.0 * ux * ux - 1.0, 2.0 * ux * uy), Point2::new(2.0 * ux * uy, 2.0 * uy * uy - 1.0));
        Map { origin: Point2::new(a.x - (a.x * x.x + a.y * y.x), a.y - (a.x * x.y + a.y * y.y)), x, y }
    }

    /// Where point `p` goes.
    pub fn point(&self, p: Point2) -> Point2 {
        Point2::new(self.origin.x + p.x * self.x.x + p.y * self.y.x, self.origin.y + p.x * self.x.y + p.y * self.y.y)
    }

    /// Where vector `v` goes: turned, the shift left out.
    pub fn vector(&self, v: Point2) -> Point2 {
        Point2::new(v.x * self.x.x + v.y * self.y.x, v.x * self.x.y + v.y * self.y.y)
    }

    /// Whether the map turns the sheet over - a mirror - and every way round goes the other way.
    pub fn turns_over(&self) -> bool {
        self.x.x * self.y.y - self.x.y * self.y.x < 0.0
    }
}

/// The tolerance an offset of an ellipse or a spline is laid within, in millimetres.
pub const OFFSET_TOLERANCE: f64 = 0.01;

impl Curve {
    /// THE CURVE TAKEN THROUGH `map`, the same kind: an arc turned over runs the other way round.
    pub fn mapped(&self, map: &Map) -> Curve {
        match self {
            Curve::Line { a, b } => Curve::Line { a: map.point(*a), b: map.point(*b) },
            Curve::Round { centre, r, from, sweep } => {
                let start = map.vector(Point2::new(from.cos(), from.sin()));
                let sweep = if map.turns_over() { -sweep } else { *sweep };
                Curve::Round { centre: map.point(*centre), r: *r, from: start.y.atan2(start.x), sweep }
            }
            Curve::Oval { centre, major, minor, from, sweep } => Curve::Oval { centre: map.point(*centre), major: map.vector(*major), minor: map.vector(*minor), from: *from, sweep: *sweep },
            Curve::Spline { b, ends } => Curve::Spline { b: b.mapped(&|p| map.point(p)), ends: *ends },
        }
    }

    /// WHETHER THE OFFSET OF THE CURVE IS A CURVE OF ITS OWN KIND - a line's a line, an arc's or a circle's one about the
    /// same centre - which constraints can hold to it; an ellipse's and a spline's are not, and are laid again from it.
    pub fn offsets_exactly(&self) -> bool {
        matches!(self, Curve::Line { .. } | Curve::Round { .. })
    }

    /// THE CURVE LAID `d` TO THE LEFT OF ITSELF - looking along it; a negative `d` to the right: a line and an arc or a
    /// circle exactly, one curve of the same kind; an ellipse and a spline - whose offsets are no ellipse and no spline -
    /// as B-splines fitted to the true offset within `OFFSET_TOLERANCE` (`fitted_offset`), as the kernel of a pro-CAD
    /// approximates an offset curve. Where the curve bends tighter than `d` its true offset folds into a loop; the loop
    /// is cut away at the place the offset crosses itself, and the offset is the splines on either side of that corner, as every sketcher
    /// trims it; near a free end the part standing nearer the curve than `d` is cut away too. Nothing where the offset
    /// folds away whole: an arc offset past its centre.
    pub fn offset(&self, d: f64) -> Vec<Curve> {
        match self {
            Curve::Line { a, b } => {
                let t = self.tangent(0.0);
                let n = Point2::new(-t.y * d, t.x * d);
                vec![Curve::Line { a: *a + n, b: *b + n }]
            }
            Curve::Round { centre, r, from, sweep } => {
                // the left of a way round counter-clockwise is its centre
                let r2 = if *sweep >= 0.0 { r - d } else { r + d };
                if r2 > 1e-9 {
                    vec![Curve::Round { centre: *centre, r: r2, from: *from, sweep: *sweep }]
                } else {
                    Vec::new()
                }
            }
            Curve::Oval { .. } | Curve::Spline { .. } => remembered(self, d),
        }
    }
}

/// THE RUNS OF A POLYLINE LEFT WHEN ITS LOOPS ARE CUT AWAY: where a piece crosses a later one, the points between the
/// two crossings go and the crossing becomes a corner, ending one run and starting the next; a loop inside a loop goes
/// with the outer one. A closed polyline that crosses itself is opened at its first corner.
///
/// The crossings are found among the pieces sharing a cell of a grid as wide as the longest piece, not among all pairs:
/// 2 000 pieces are four million pairs, and a preview lays the offset out every frame.
fn without_loops(pts: Vec<Point2>, ends: Ends) -> Vec<Vec<Point2>> {
    cut_loops(pts, ends, Turned::No)
}

/// WHETHER A LOOP HAS BEEN READ AGAIN FROM ANOTHER POINT already, by `cut_loops`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Turned {
    No,
    Yes,
}

/// `without_loops`, read from where `pts` starts, or once again from a point outside every loop.
fn cut_loops(mut pts: Vec<Point2>, ends: Ends, turned: Turned) -> Vec<Vec<Point2>> {
    let closed = ends == Ends::Closed;
    if closed {
        pts.push(pts[0]);
    }
    let n = pts.len();
    let cell = pts.windows(2).map(|w| w[0].dist(w[1])).fold(1e-9, f64::max);
    let key = |p: Point2| Cell { x: (p.x / cell).floor() as i64, y: (p.y / cell).floor() as i64 };
    let mut grid: std::collections::HashMap<Cell, Vec<usize>> = std::collections::HashMap::new();
    for k in 0..n - 1 {
        let (a, b) = (key(pts[k]), key(pts[k + 1]));
        for x in a.x.min(b.x)..=a.x.max(b.x) {
            for y in a.y.min(b.y)..=a.y.max(b.y) {
                grid.entry(Cell { x, y }).or_default().push(k);
            }
        }
    }
    // every crossing of a piece with a later one not next to it: the two pieces and the point
    let mut cuts: Vec<Cut> = Vec::new();
    for list in grid.values() {
        for (x, &i) in list.iter().enumerate() {
            for &j in &list[x + 1..] {
                let (i, j) = (i.min(j), i.max(j));
                if j <= i + 1 || (closed && i == 0 && j + 2 == n) || cuts.iter().any(|c| c.first == i && c.last == j) {
                    continue;
                }
                if let Some(at) = crossing([[pts[i], pts[i + 1]], [pts[j], pts[j + 1]]]) {
                    cuts.push(Cut { first: i, last: j, at });
                }
            }
        }
    }
    // A LOOP OF A LOOP IS THE SHORTER WAY ROUND between its crossing: a crossing reaching past half the way is a loop
    // standing across the seam, and the polyline is read again from half way round, the loop in its middle. Taken from
    // the seam, the loop at the tip of a closed spline offset 0.3 inwards was the whole of it but 35 points
    if closed && turned == Turned::No && cuts.iter().any(|c| c.last - c.first > (n - 1) / 2) {
        // read again from a point outside every loop - the shorter way round of each crossing - once
        let pieces = n - 1;
        let in_a_loop = |k: usize| cuts.iter().any(|c| if c.last - c.first <= pieces / 2 { k > c.first && k <= c.last } else { k > c.last || k <= c.first });
        if let Some(start) = (0..pieces).find(|&k| !in_a_loop(k)) {
            pts.pop();
            pts.rotate_left(start);
            return cut_loops(pts, ends, Turned::Yes);
        }
    }
    // the outer loops: from the first piece on, the crossing reaching farthest, the next taken past its end
    cuts.sort_by(|a, b| a.first.cmp(&b.first).then(b.last.cmp(&a.last)));
    let mut outer: Vec<Cut> = Vec::new();
    for c in cuts {
        if outer.last().is_none_or(|o| c.first > o.last) {
            outer.push(c);
        }
    }
    if outer.is_empty() {
        if closed {
            pts.pop();
        }
        return vec![pts];
    }
    let mut out = Vec::new();
    let mut run = vec![pts[0]];
    let mut k = 0;
    for c in &outer {
        run.extend_from_slice(&pts[k + 1..=c.first]);
        run.push(c.at);
        out.push(std::mem::replace(&mut run, vec![c.at]));
        k = c.last;
    }
    run.extend_from_slice(&pts[k + 1..]);
    out.push(run);
    if closed && out.len() > 1 {
        // the run past the last corner goes on round into the run before the first
        let tail = out.pop().unwrap_or_default();
        let head = out.remove(0);
        let mut joined = tail;
        joined.extend_from_slice(&head[1..]);
        out.insert(0, joined);
    }
    out.retain(|r| r.len() >= 2);
    out
}

/// A CELL OF THE GRID the pieces of a polyline are sorted into, by its column and row.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Cell {
    x: i64,
    y: i64,
}

/// A CROSSING OF A POLYLINE WITH ITSELF: the earlier piece, the later one, and the point.
struct Cut {
    first: usize,
    last: usize,
    at: Point2,
}

/// Where two straight pieces cross strictly inside both, if they do.
fn crossing([a, b]: [[Point2; 2]; 2]) -> Option<Point2> {
    let (r, s) = (a[1] - a[0], b[1] - b[0]);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-18 {
        return None;
    }
    let q = b[0] - a[0];
    let (t, u) = ((q.x * s.y - q.y * s.x) / den, (q.x * r.y - q.y * r.x) / den);
    (t > 1e-12 && t < 1.0 - 1e-12 && u > 1e-12 && u < 1.0 - 1e-12).then(|| Point2::new(a[0].x + r.x * t, a[0].y + r.y * t))
}

/// HOW SHARPLY THE LAYOUT OF AN OFFSET TURNS where it is laid out finer: 20 deg between two pieces of it.
const SHARP_TURN: f64 = 20.0 * std::f64::consts::PI / 180.0;

/// WHAT AN OFFSET IS LAID ALONG: the stretch of the curve's own parameter it runs over, its ends, and its points laid out
/// densely, which every span of it is held to.
struct OffsetRun<'a> {
    span: [f64; 2],
    ends: Ends,
    /// the ends of the curve offset: a loop's parameter runs on past 1 to its start again
    source: Ends,
    run: &'a [Point2],
    /// how far a point stands off the true offset, its foot on the curve sought from a place of the curve
    off: &'a dyn Fn(Point2, f64) -> f64,
    /// the places of the curve's own parameter where its bends change - the knots of a spline, unrolled round a loop -
    /// which the offset begins its nodes at
    bends: &'a [f64],
}

/// THE OFFSET `at` AS ONE B-SPLINE, approximated within `OFFSET_TOLERANCE` (`BSpline::approximate`) in the curve's own
/// parameter - in which a spline is cubic span by span - its knots begun at the curve's bends, each place of it held to
/// the distance from the curve; an open piece through the two ends of its run. The copy is one smooth curve, one face of
/// a body a piece. `None` where the fit has no answer.
///
/// Laid as a spline through nodes of the true offset, each span cut in two until it held, a piece whose end met the true
/// offset at a cusp never held and was cut up to 4 096 nodes.
fn fitted_offset(at: &dyn Fn(f64) -> Point2, laid: OffsetRun) -> Option<Curve> {
    let OffsetRun { span: [t0, t1], ends, source, run, off, bends } = laid;
    let wraps = source == Ends::Closed;
    let width = (t1 - t0).max(1e-12);
    let place = |u: f64| {
        let t = t0 + u * width;
        at(if wraps { t.rem_euclid(1.0) } else { t.clamp(t0, t1) })
    };
    let knots: Vec<f64> = bends.iter().map(|k| (k - t0) / width).collect();
    let held = |q: Point2, u: f64| off(q, t0 + u * width);
    let through = (ends == Ends::Open).then(|| [run[0], run[run.len() - 1]]);
    let b = BSpline::approximate(&Approximated { at: &place, bends: &knots, ends, off: &held, tolerance: OFFSET_TOLERANCE, through })?;
    Some(Curve::Spline { b, ends })
}

impl Curve {
    /// THE STRETCH OF THE CURVE'S OWN PARAMETER A PIECE OF ITS OFFSET RUNS OVER: the places of the samples next to its ends,
    /// read off the dense layout the piece was cut from (`places`) exactly, and from them the place within a sample whose
    /// offset stands nearest the end itself - a corner where the offset crosses itself, or a free end. The nearest place of
    /// the curve to a point of the offset is two places at a loop, and took a piece round the loop cut away. A loop's
    /// parameter runs on past 1.
    fn span_of_run(&self, at: &dyn Fn(f64) -> Point2, run: &[Point2], laid: &Laid) -> [f64; 2] {
        let n = run.len();
        let step = 1.0 / laid.samples;
        let wrap = |t: f64| if laid.ends == Ends::Closed { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) };
        let place_of = |k: usize| laid.places.get(&key_of(run[k.min(n - 1)])).copied().unwrap_or_else(|| self.nearest(run[k.min(n - 1)]).t);
        // the place within a sample of `guess`, on the side `dir`, whose offset stands nearest `end`
        let nearest_place =
            |guess: f64, end: Point2, dir: f64| (0..=200).map(|k| guess + dir * step * k as f64 / 200.0).min_by(|x, y| at(wrap(*x)).dist(end).total_cmp(&at(wrap(*y)).dist(end))).unwrap_or(guess);
        let start = nearest_place(place_of(1), run[0], -1.0);
        let mut finish = nearest_place(place_of(n.saturating_sub(2)), run[n - 1], 1.0);
        if laid.ends == Ends::Closed {
            if finish <= start {
                finish += 1.0;
            }
            [start, finish]
        } else {
            [start.clamp(0.0, 1.0), finish.clamp(0.0, 1.0)]
        }
    }
}

/// THE DENSE LAYOUT OF AN OFFSET: the place of the curve each sample stands at, by the sample, how many samples a unit of
/// the place holds, and the ends of the curve.
struct Laid {
    places: std::collections::HashMap<SampleKey, f64>,
    samples: f64,
    ends: Ends,
}

/// A SAMPLE OF THE LAYOUT, by the bits of its coordinates: what a piece cut from the layout keeps of it exactly.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SampleKey {
    x: u64,
    y: u64,
}

fn key_of(p: Point2) -> SampleKey {
    SampleKey { x: p.x.to_bits(), y: p.y.to_bits() }
}

impl Curve {
    /// THE OFFSET OF AN ELLIPSE OR A SPLINE, laid afresh (`Curve::offset`).
    fn offset_laid(&self, d: f64) -> Vec<Curve> {
        match self {
            Curve::Oval { .. } | Curve::Spline { .. } => {
                let true_at = |t: f64| {
                    let (p, u) = (self.at(t), self.tangent(t));
                    Point2::new(p.x - u.y * d, p.y + u.x * d)
                };
                let closed = self.closed();
                // the true offset, densely, its loops cut away: what the splines are laid through and held to
                // a loop laid out once round: its place 1 is its place 0 again, which the cutting of loops closes it by
                let last = if closed { 1999 } else { 2000 };
                let even: Vec<f64> = (0..=last).map(|k| k as f64 / 2000.0).collect();
                // LAID OUT FINER WHERE IT TURNS SHARPLY: a loop of the true offset at a bend about as tight as the distance
                // may be smaller than the spacing of the samples, and its crossing was not found - the outward offset 3 of
                // a closed spline of 39 nodes ran 0.165 mm into the curve there. Between samples where the layout turns by
                // more than 20 deg, sixteen more.
                let places: Vec<f64> = {
                    let at: Vec<Point2> = even.iter().map(|t| true_at(*t)).collect();
                    let m = at.len();
                    let sharp = |k: usize| {
                        let (a, b, c) = (at[(k + m - 1) % m], at[k], at[(k + 1) % m]);
                        let (u, v) = (b - a, c - b);
                        let (lu, lv) = (u.x.hypot(u.y), v.x.hypot(v.y));
                        (closed || (k > 0 && k + 1 < m)) && lu > 1e-12 && lv > 1e-12 && u.x * v.x + u.y * v.y < SHARP_TURN.cos() * lu * lv
                    };
                    let mut out: Vec<f64> = Vec::with_capacity(m);
                    for k in 0..m {
                        out.push(even[k]);
                        let next = match even.get(k + 1) {
                            Some(t) => *t,
                            None if closed => 1.0,
                            None => continue,
                        };
                        if sharp(k) || sharp((k + 1) % m) {
                            out.extend((1..16).map(|j| even[k] + (next - even[k]) * j as f64 / 16.0));
                        }
                    }
                    out
                };
                let mut dense: Vec<Point2> = places.iter().map(|t| true_at(*t)).collect();
                let ends = if closed { Ends::Closed } else { Ends::Open };
                let laid = Laid { places: dense.iter().zip(&places).map(|(p, t)| (key_of(*p), *t)).collect(), samples: 2000.0, ends };
                if !closed {
                    // near a free end bending tighter than `d` the offset turns back towards the curve without crossing
                    // itself: on the S of the probes it began 2.981 mm from the curve offset by 3. What stands nearer
                    // than `d` is cut from either end
                    let near = |q: &Point2| self.nearest(*q).off < d.abs() - 1e-6;
                    let head = dense.iter().take_while(|q| near(q)).count();
                    let tail = dense[head..].iter().rev().take_while(|q| near(q)).count();
                    dense.truncate(dense.len() - tail);
                    dense.drain(..head);
                    if dense.len() < 2 {
                        return Vec::new();
                    }
                }
                // HOW FAR A POINT STANDS OFF THE TRUE OFFSET: off the distance from the curve, its foot sought from the place it
                // should stand by. Held to the samples of the true offset instead - a polyline, whose chords stand off the
                // curve by more than the tolerance where it bends tight - the fit of an offset of a spline of 39 nodes never
                // came within it, and laid pieces of 7 000 nodes in minutes
                let off = |q: Point2, t: f64| {
                    let foot = self.polish(q, if closed { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) });
                    (self.at(foot).dist(q) - d.abs()).abs()
                };
                let laid_out = dense.len();
                let runs = without_loops(dense, ends);
                // EACH PIECE IS LAID ALONG THE CURVE ITSELF, by its own places and ways: as few nodes as keep it within the
                // tolerance. Through points of it chosen evenly, a closed spline took 512 nodes, its pieces past the bend of
                // a notch 2 048. A whole loop stays a loop; a piece left by a loop cut away runs from its corner, open there
                // one run that lost no more than a couple of points is the whole of it: a crossing found by the pieces next
                // to the seam of a loop, at a bend tight as the distance, cut nothing away
                let whole = runs.len() == 1 && runs[0].len() + 3 >= laid_out;
                // where the bends of the curve change: the knots of a spline, eight even places of an ellipse; a loop's
                // a second time round, past 1
                let once: Vec<f64> = match self {
                    Curve::Spline { b, .. } => std::iter::once(0.0).chain(b.inner_knots()).chain(std::iter::once(1.0)).collect(),
                    _ => (0..=8).map(|k| k as f64 / 8.0).collect(),
                };
                let bends: Vec<f64> = once.iter().copied().chain(once.iter().skip(1).map(|k| k + 1.0)).collect();
                runs.iter()
                    .filter_map(|run| {
                        let span = if whole && closed { [0.0, 1.0] } else { self.span_of_run(&true_at, run, &laid) };
                        fitted_offset(&true_at, OffsetRun { span, ends: if whole { ends } else { Ends::Open }, source: ends, run, off: &off, bends: &bends })
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// A KEY OF THE CURVE BY THE BITS OF ITS NUMBERS: two curves of one key are one curve.
    pub(crate) fn key(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        let bits = |p: Point2, h: &mut dyn std::hash::Hasher| {
            h.write_u64(p.x.to_bits());
            h.write_u64(p.y.to_bits());
        };
        std::mem::discriminant(self).hash(h);
        match self {
            Curve::Line { a, b } => [*a, *b].iter().for_each(|p| bits(*p, h)),
            Curve::Round { centre, r, from, sweep } => {
                bits(*centre, h);
                [*r, *from, *sweep].iter().for_each(|v| h.write_u64(v.to_bits()));
            }
            Curve::Oval { centre, major, minor, from, sweep } => {
                [*centre, *major, *minor].iter().for_each(|p| bits(*p, h));
                [*from, *sweep].iter().for_each(|v| h.write_u64(v.to_bits()));
            }
            Curve::Spline { b, ends } => {
                b.poles.iter().for_each(|p| bits(*p, h));
                b.knots.iter().for_each(|k| h.write_u64(k.to_bits()));
                ends.hash(h);
            }
        }
    }
}

/// AN OFFSET LAID BEFORE, by the key of its curve and distance.
struct Remembered {
    key: u64,
    pieces: Vec<Curve>,
}

/// How many offsets are remembered: a preview asks each frame the offsets of the curves picked, on both sides.
const REMEMBERED: usize = 32;

thread_local! {
    static OFFSETS: std::cell::RefCell<Vec<Remembered>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// THE OFFSET OF CURVE `c` BY `d`, AS LAID BEFORE where it was: the preview of the offset tool asked it three times a frame
/// with nothing changed, 20 ms each for a closed spline of 22 nodes.
fn remembered(c: &Curve, d: f64) -> Vec<Curve> {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    c.key(&mut h);
    h.write_u64(d.to_bits());
    let key = h.finish();
    if let Some(found) = OFFSETS.with(|m| m.borrow().iter().find(|r| r.key == key).map(|r| r.pieces.clone())) {
        return found;
    }
    let pieces = c.offset_laid(d);
    OFFSETS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= REMEMBERED {
            m.remove(0);
        }
        m.push(Remembered { key, pieces: pieces.clone() });
    });
    pieces
}
