//! THE CURVE OF A SKETCH, ONE FOR EVERY KIND: a line, a circle or an arc of one, an ellipse or an arc of one, a spline -
//! read off its entity once (`Curve::of`) and asked the same questions whatever it is: the point and the direction at a
//! place along it, its length, the place nearest a point, the points it is drawn by, where it meets another curve.
//!
//! The kind of an entity is told apart here and in the model that declares it, and nowhere else: a tool, a snap, a
//! preview or the solver asks the curve, it does not take the kind apart itself.
//!
//! A place along a curve is its parameter `t` from 0 at its start to 1 at its end. On a line that is the share of its
//! length, on a round curve the share of its sweep, on an ellipse the share of the sweep of its eccentric angle, on a
//! spline the parameter of its B-spline (`geom::bspline`), whose knots stand at its nodes by the lengths of the chords
//! between them.
use super::*;
use crate::geom::bspline::BSpline;

mod arrange;
mod blend;
mod cut;
mod edges;
mod fit;
mod marks;
pub mod kinds;
mod meet;
pub mod refs;
mod shift;
mod stretch;
mod touch;
pub(crate) use cut::Trimmed;
pub use blend::{centres_touching, chamfer, radius_in, rounding, Arm, Half, Rounding};
pub use marks::OvalAxes;
pub use meet::Meet;
pub(crate) use meet::LaidPiece;
pub use shift::{Map, OFFSET_TOLERANCE};
pub use stretch::End;
pub use touch::WayOn;

/// How many places the nearest hollow of the even places is looked at finely in, over two of them either side.
const FINE_LOOKS: usize = 32;

/// How many even places of each span of a spline the search for the nearest place starts from.
const NEAR_LOOKS: usize = 8;

/// How many even places of a curve, at the least, the search for the nearest place starts from.
const NEAR_LOOKS_LEAST: usize = 64;

/// How far a laid out curve may stray from the true one, at most, in millimetres: 0.005 mm, checked at a quarter, a half
/// and three quarters of every piece - one look at the middle of a piece misses the sag of a spline at a turn of its
/// curvature, measured at 0.03 mm on an S-shaped span laid out by its middles alone.
pub const SAG: f64 = 0.005;

/// A CURVE OF A SKETCH AS GEOMETRY: where it stands, read off its entity.
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    /// a straight piece from `a` to `b`
    Line { a: Point2, b: Point2 },
    /// a circle, or an arc of one: from the angle `from`, through the signed `sweep` (a whole turn for a circle)
    Round { centre: Point2, r: f64, from: f64, sweep: f64 },
    /// an ellipse, or an arc of one: its semi-axes as vectors from the centre, from the eccentric angle `from` through the
    /// signed `sweep`
    Oval { centre: Point2, major: Point2, minor: Point2, from: f64, sweep: f64 },
    /// a spline: a cubic B-spline, a loop when `ends` closes it - the one form a spline has, in the sketch and in the
    /// kernel
    Spline { b: BSpline, ends: crate::feature::Ends },
}

/// A PLACE ON A CURVE: its parameter, the point there and how far it is from what it was asked about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct At {
    pub t: f64,
    pub point: Point2,
    pub off: f64,
}

/// A whole turn.
const TURN: f64 = std::f64::consts::TAU;

/// AN ARC OF A CIRCLE BY ITS CENTRE AND ITS ENDS, and the way it runs from the first to the second.
#[derive(Clone, Copy, Debug)]
struct ArcEnds {
    centre: Point2,
    from: Point2,
    to: Point2,
    winding: crate::feature::Winding,
}

impl Curve {
    /// THE CURVE OF ENTITY `e` of sketch `s`, where its points stand; `None` when a point of it is missing.
    pub fn of(s: &Sketch, e: &SketchEntity) -> Option<Curve> {
        Curve::of_placed(&|id: Id| s.point(id).map(|p| Point2::new(p.x, p.y)), e)
    }

    /// THE CURVE OF ENTITY `e` with its points where `at` puts them; `None` when a point of it is missing.
    pub fn of_placed(at: &dyn Fn(Id) -> Option<Point2>, e: &SketchEntity) -> Option<Curve> {
        Some(match &e.kind {
            EntityKind::Line { a, b } => Curve::Line { a: at(*a)?, b: at(*b)? },
            EntityKind::Circle { center, r } => Curve::Round { centre: at(*center)?, r: *r, from: 0.0, sweep: TURN },
            EntityKind::Arc { center, a, b, ccw } => {
                let winding = if *ccw { crate::feature::Winding::Ccw } else { crate::feature::Winding::Cw };
                Curve::arc_of(ArcEnds { centre: at(*center)?, from: at(*a)?, to: at(*b)?, winding })
            }
            EntityKind::Ellipse { c, ma, mi, span } => Curve::oval_of(at, [*c, *ma, *mi], span.as_ref())?,
            EntityKind::Spline { nodes, handles, ends } => Curve::spline(nodes.iter().map(|id| at(*id)).collect::<Option<_>>()?, handles, *ends),
        })
    }

    /// THE ARC ABOUT ITS CENTRE FROM ONE END ROUND TO THE OTHER, the way `winding` says, of the radius its first end
    /// stands at.
    fn arc_of(ends: ArcEnds) -> Curve {
        let ArcEnds { centre, from, to, winding } = ends;
        let angle = |p: Point2| (p.y - centre.y).atan2(p.x - centre.x);
        let (a0, a1) = (angle(from), angle(to));
        let sweep = match winding {
            crate::feature::Winding::Ccw => (a1 - a0).rem_euclid(TURN),
            crate::feature::Winding::Cw => -(a0 - a1).rem_euclid(TURN),
        };
        Curve::Round { centre, r: centre.dist(from), from: a0, sweep }
    }

    /// THE ELLIPSE OF FRAME `[centre, major end, minor end]`, its points where `at` puts them: whole, or between the ends of
    /// `span` the way it goes.
    fn oval_of(at: &dyn Fn(Id) -> Option<Point2>, [c, ma, mi]: [Id; 3], span: Option<&EllipseSpan>) -> Option<Curve> {
        let (pc, pa, pi) = (at(c)?, at(ma)?, at(mi)?);
        let (major, minor) = (pa - pc, pi - pc);
        let (from, sweep) = match span {
            None => (0.0, TURN),
            Some(sp) => {
                let axes = [pc, pa, pi];
                let (t0, t1) = (tess::ellipse_angle(axes, at(sp.from)?), tess::ellipse_angle(axes, at(sp.to)?));
                let sweep = match sp.winding {
                    crate::feature::Winding::Ccw => (t1 - t0).rem_euclid(TURN),
                    crate::feature::Winding::Cw => -(t0 - t1).rem_euclid(TURN),
                };
                (t0, sweep)
            }
        };
        Some(Curve::Oval { centre: pc, major, minor, from, sweep })
    }

    /// THE CURVE A CONSTRAINT REFERS TO (`CurveRef`), its points where `at` puts them and the radius of a circle about a
    /// centre where `radius` reads it: a line between its ends, a circle whole, an arc between its ends, an ellipse whole,
    /// a spline as drawn. `None` for a point, or where a point or a radius is missing.
    pub fn of_ref(on: &crate::model::CurveRef, at: &dyn Fn(Id) -> Option<Point2>, radius: &dyn Fn(Id) -> Option<f64>) -> Option<Curve> {
        use crate::model::CurveRef;
        Some(match on {
            CurveRef::Point { .. } => return None,
            CurveRef::Line { a, b } => Curve::Line { a: at(*a)?, b: at(*b)? },
            CurveRef::Round { c } => Curve::Round { centre: at(*c)?, r: radius(*c)?, from: 0.0, sweep: TURN },
            CurveRef::Arc { c, a, b, winding } => Curve::arc_of(ArcEnds { centre: at(*c)?, from: at(*a)?, to: at(*b)?, winding: *winding }),
            CurveRef::Oval { c, ma, mi, span } => Curve::oval_of(at, [*c, *ma, *mi], span.as_ref())?,
            CurveRef::Spline { .. } => return None,
            CurveRef::SplineNow { nodes, handles, ends } => Curve::spline(nodes.iter().map(|id| at(*id)).collect::<Option<_>>()?, handles, *ends),
        })
    }

    /// THE CURVE REACHING `k` OF ITS LENGTHS PAST EACH END ALONG THE STRAIGHT IT LIES ON, where it is a line - as a line is
    /// touched anywhere along it; a curve of any other kind as it is.
    pub fn reaching(&self, k: f64) -> Curve {
        match *self {
            Curve::Line { a, b } => {
                let run = b - a;
                Curve::Line { a: a - Point2::new(run.x * k, run.y * k), b: b + Point2::new(run.x * k, run.y * k) }
            }
            _ => self.clone(),
        }
    }

    /// THE SPLINE THROUGH `nodes` with `handles`, open or closed, as the sketch draws it: continuous curvature through the
    /// nodes no handle holds (`BSpline::through`); fewer than two nodes, a line of no length where the one stands.
    pub fn spline(nodes: Vec<Point2>, handles: &[Handle], ends: crate::feature::Ends) -> Curve {
        match BSpline::through(&nodes, handles, ends) {
            Some(b) => Curve::Spline { b, ends },
            None => {
                let at = nodes.first().copied().unwrap_or(Point2::new(0.0, 0.0));
                Curve::Line { a: at, b: at }
            }
        }
    }

    /// THE B-SPLINE OF A SPLINE; `None` for a curve of another kind.
    pub fn b_spline(&self) -> Option<&BSpline> {
        match self {
            Curve::Spline { b, .. } => Some(b),
            _ => None,
        }
    }

    /// Whether the curve closes on itself: a whole circle, a whole ellipse, a closed spline.
    pub fn closed(&self) -> bool {
        match self {
            Curve::Line { .. } => false,
            Curve::Round { sweep, .. } | Curve::Oval { sweep, .. } => sweep.abs() >= TURN - 1e-12,
            Curve::Spline { ends, .. } => *ends == crate::feature::Ends::Closed,
        }
    }

    /// The number of spans of a spline between its knots, one for any other curve.
    fn spans(&self) -> usize {
        match self {
            Curve::Spline { b, .. } => b.spans(),
            _ => 1,
        }
    }

    /// THE HANDLES OF A SPLINE, the tangent each node is arrived at and left by, as a spline entity keeps them - a node at
    /// every knot (`BSpline::hermite`); `None` for a curve of another kind.
    pub fn handles(&self) -> Option<Vec<crate::model::Handle>> {
        let Curve::Spline { b, ends } = self else { return None };
        Some(b.hermite(*ends).tangents.iter().map(|t| crate::model::Handle { arrive: Some([t.arrive.x, t.arrive.y]), leave: Some([t.leave.x, t.leave.y]) }).collect())
    }

    /// THE NODES OF A SPLINE, a node at every knot; none for a curve of another kind.
    pub fn nodes(&self) -> Vec<Point2> {
        match self {
            Curve::Spline { b, ends } => b.hermite(*ends).nodes,
            _ => Vec::new(),
        }
    }

    /// THE POINT at place `t`.
    pub fn at(&self, t: f64) -> Point2 {
        match self {
            Curve::Line { a, b } => Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
            Curve::Round { centre, r, from, sweep } => {
                let w = from + sweep * t;
                Point2::new(centre.x + r * w.cos(), centre.y + r * w.sin())
            }
            Curve::Oval { centre, major, minor, from, sweep } => {
                let w = from + sweep * t;
                Point2::new(centre.x + major.x * w.cos() + minor.x * w.sin(), centre.y + major.y * w.cos() + minor.y * w.sin())
            }
            Curve::Spline { b, .. } => b.at(t),
        }
    }

    /// THE DERIVATIVE dP/dt at place `t`: the way the curve runs there, as long as it runs per unit of its parameter.
    pub fn velocity(&self, t: f64) -> Point2 {
        match self {
            Curve::Line { a, b } => *b - *a,
            Curve::Round { r, from, sweep, .. } => {
                let w = from + sweep * t;
                Point2::new(-r * sweep * w.sin(), r * sweep * w.cos())
            }
            Curve::Oval { major, minor, from, sweep, .. } => {
                let w = from + sweep * t;
                Point2::new(sweep * (-major.x * w.sin() + minor.x * w.cos()), sweep * (-major.y * w.sin() + minor.y * w.cos()))
            }
            Curve::Spline { b, .. } => b.velocity(t),
        }
    }

    /// THE DIRECTION the curve runs at place `t`, a unit vector; along a degenerate curve, along X.
    pub fn tangent(&self, t: f64) -> Point2 {
        let v = self.velocity(t);
        let l = v.len();
        if l < 1e-15 {
            Point2::new(1.0, 0.0)
        } else {
            Point2::new(v.x / l, v.y / l)
        }
    }

    /// THE START AND THE END of the curve - one point twice for a closed curve.
    pub fn ends(&self) -> [Point2; 2] {
        [self.at(0.0), self.at(1.0)]
    }

    /// THE PLACES THE CURVE IS LAID OUT AT, from 0 to 1, no piece straying from it by more than `SAG`: a line its two
    /// ends; any other curve split until the chord of every piece lies within the sag at a quarter, a half and three
    /// quarters of it.
    pub fn places(&self) -> Vec<f64> {
        if let Curve::Line { .. } = self {
            return vec![0.0, 1.0];
        }
        remembered_places(self)
    }

    /// The places of `places`, laid out afresh, no piece straying from the curve by more than `sag`.
    fn places_laid(&self, sag: f64) -> Vec<f64> {
        let n = (self.spans() * 4).max(8);
        let mut out = vec![0.0];
        let mut a = self.at(0.0);
        for k in 0..n {
            let (t0, t1) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
            let b = self.at(t1);
            self.refine(Laying { piece: Piece { from: t0, to: t1 }, ends: [a, b], sag }, 16, &mut out);
            a = b;
        }
        out
    }

    /// The end of a piece pushed, after the places that split it until it lies within the sag; the points at its ends
    /// handed down, not read again.
    fn refine(&self, laying: Laying, depth: u32, out: &mut Vec<f64>) {
        let Laying { piece, ends: [a, b], sag } = laying;
        let off = [0.25, 0.5, 0.75]
            .iter()
            .map(|k| {
                let p = self.at(piece.from + (piece.to - piece.from) * k);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l = dx.hypot(dy);
                if l < 1e-15 {
                    p.dist(a)
                } else {
                    (dx * (p.y - a.y) - dy * (p.x - a.x)).abs() / l
                }
            })
            .fold(0.0, f64::max);
        if depth == 0 || off <= sag {
            out.push(piece.to);
        } else {
            let mid = 0.5 * (piece.from + piece.to);
            let m = self.at(mid);
            self.refine(Laying { piece: Piece { from: piece.from, to: mid }, ends: [a, m], sag }, depth - 1, out);
            self.refine(Laying { piece: Piece { from: mid, to: piece.to }, ends: [m, b], sag }, depth - 1, out);
        }
    }

    /// THE POINTS THE CURVE IS DRAWN BY, at its places (`places`).
    pub fn polyline(&self) -> Vec<Point2> {
        self.places().into_iter().map(|t| self.at(t)).collect()
    }

    /// THE LENGTH of the curve along it: exact for a line and a round curve, along its laid out points otherwise.
    pub fn length(&self) -> f64 {
        match self {
            Curve::Line { a, b } => a.dist(*b),
            Curve::Round { r, sweep, .. } => r * sweep.abs(),
            _ => self.polyline().windows(2).map(|w| w[0].dist(w[1])).sum(),
        }
    }

    /// THE PLACE OF THE CURVE NEAREST TO `p`: exact for a line and a round curve; found among the laid out places and
    /// narrowed by Newton's steps on the derivative of the distance otherwise.
    pub fn nearest(&self, p: Point2) -> At {
        let t = match self {
            Curve::Line { a, b } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                if l2 < 1e-30 {
                    0.0
                } else {
                    (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
                }
            }
            Curve::Round { centre, from, sweep, .. } => {
                let w = (p.y - centre.y).atan2(p.x - centre.x);
                let inside = ((w - from) * sweep.signum()).rem_euclid(TURN) / sweep.abs();
                if inside <= 1.0 || self.closed() {
                    inside.min(1.0)
                } else if p.dist(self.at(0.0)) <= p.dist(self.at(1.0)) {
                    0.0
                } else {
                    1.0
                }
            }
            // THE NEAREST OF EVEN PLACES ALONG IT, POLISHED: 8 a span of a spline, 64 at the least. The places the curve is
            // laid out at to the sag (`places`) were made afresh on every call and cost 0.19 ms on a spline of 30 nodes;
            // read for every edge of a loop of one, they made a rebuild of the sketch take 1 s
            // THE NEAREST HOLLOW OF THE EVEN PLACES LOOKED AT FINELY, and polished from the nearest of the fine places, and
            // the next nearest hollow polished as it is; the nearer of the two taken. At a bend tight as the spacing of
            // the even places the distance dips twice between two of them - a point 3 off a spline at such a bend, polished
            // from the even places, came to the dip 3.31 off and not to the one 3.007 off - and at a notch the nearest
            // even place lies in the hollow next to the one the curve is nearest in
            _ => {
                let n = (self.spans() * NEAR_LOOKS).max(NEAR_LOOKS_LEAST);
                let off: Vec<f64> = (0..=n).map(|k| self.at(k as f64 / n as f64).dist(p)).collect();
                let closed = self.closed();
                let hollow = |k: usize| {
                    let before = if k > 0 {
                        off[k - 1]
                    } else if closed {
                        off[n - 1]
                    } else {
                        f64::MAX
                    };
                    let after = if k < n {
                        off[k + 1]
                    } else if closed {
                        off[1]
                    } else {
                        f64::MAX
                    };
                    off[k] <= before && off[k] <= after
                };
                let mut hollows: Vec<usize> = (0..=n).filter(|&k| hollow(k)).collect();
                hollows.sort_by(|a, b| off[*a].total_cmp(&off[*b]));
                let hold = |t: f64| if closed { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) };
                let finely = |k: usize| -> f64 {
                    let (from, step) = ((k as f64 - 2.0) / n as f64, 4.0 / (n as f64 * FINE_LOOKS as f64));
                    (0..=FINE_LOOKS).map(|j| hold(from + step * j as f64)).min_by(|a, b| self.at(*a).dist(p).total_cmp(&self.at(*b).dist(p))).unwrap_or(k as f64 / n as f64)
                };
                let starts: Vec<f64> = hollows.first().map(|&k| finely(k)).into_iter().chain(hollows.get(1).map(|&k| k as f64 / n as f64)).collect();
                starts.iter().map(|&s| self.polish(p, s)).min_by(|a, b| self.at(*a).dist(p).total_cmp(&self.at(*b).dist(p))).unwrap_or(0.0)
            }
        };
        let point = self.at(t);
        At { t, point, off: point.dist(p) }
    }

    /// THE PIECE OF THE CURVE FROM PLACE `run[0]` TO PLACE `run[1]`, the same kind and the same shape: a line of a line, an arc
    /// of a circle or of an arc, an arc of an ellipse of an ellipse or of an arc of one, a spline of a spline. On a closed
    /// curve the second place may go past 1, round its start: the piece from 0.75 to 1.25 is the quarter on either side
    /// of its start.
    ///
    /// A piece of a spline is the piece of its B-spline (`BSpline::piece`); a piece round the start of a loop, the two on
    /// either side of the start joined.
    pub fn piece(&self, run: [f64; 2]) -> Curve {
        let [from, to] = run;
        match self {
            Curve::Line { .. } => Curve::Line { a: self.at(from), b: self.at(to) },
            Curve::Round { centre, r, from: w, sweep } => Curve::Round { centre: *centre, r: *r, from: w + sweep * from, sweep: sweep * (to - from) },
            Curve::Oval { centre, major, minor, from: w, sweep } => Curve::Oval { centre: *centre, major: *major, minor: *minor, from: w + sweep * from, sweep: sweep * (to - from) },
            Curve::Spline { b, .. } => {
                let from = from.clamp(0.0, 1.0);
                let reach = if self.closed() { from + 1.0 } else { 1.0 };
                // a piece of no length is the place it stands at, a hair long
                let to = to.clamp(from, reach).max(from + 1e-12);
                // a piece from the very end of a loop is the run past its start alone: joined to nothing before it, its
                // joint stood at a share of 0
                let piece = if to > 1.0 && from >= 1.0 - 1e-12 {
                    b.piece([0.0, (to - 1.0).max(1e-12)])
                } else if to > 1.0 {
                    let (before, after) = (b.piece([from.min(1.0 - 1e-12), 1.0]), b.piece([0.0, (to - 1.0).max(1e-12)]));
                    before.joined(&after, (1.0 - from) / (to - from))
                } else {
                    b.piece([from, to])
                };
                Curve::Spline { b: piece, ends: crate::feature::Ends::Open }
            }
        }
    }

    /// The place `t` near `start` where the curve is nearest to `p`, by Newton's steps on the derivative of the squared
    /// distance, held within the curve - on a closed curve going on round past its start: a point 0.001 of the way
    /// before the start of a closed spline, held at 0 by the end of the curve, was answered 0.025 mm off it. A step that
    /// would stand farther from `p` is halved until it does not: near a bend the step of Newton's leapt to the far side
    /// of a spline, and a point 2 off it was answered 42 off.
    pub(crate) fn polish(&self, p: Point2, start: f64) -> f64 {
        let closed = self.closed();
        let hold = |t: f64| if closed { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) };
        let mut t = start;
        let h = 1e-6;
        for _ in 0..30 {
            let f = |t: f64| {
                let (q, v) = (self.at(hold(t)), self.velocity(hold(t)));
                (q.x - p.x) * v.x + (q.y - p.y) * v.y
            };
            let (lo, hi) = if closed { (t - h, t + h) } else { ((t - h).max(0.0), (t + h).min(1.0)) };
            let (g, dg) = (f(t), (f(hi) - f(lo)) / (hi - lo));
            if dg.abs() < 1e-18 {
                break;
            }
            let here = self.at(hold(t)).dist(p);
            let mut step = -g / dg;
            let mut next = hold(t + step);
            for _ in 0..30 {
                if self.at(next).dist(p) <= here {
                    break;
                }
                step *= 0.5;
                next = hold(t + step);
            }
            if self.at(next).dist(p) > here || (next - t).abs() < 1e-14 {
                break;
            }
            t = next;
        }
        t
    }
}

/// THE PLACES A CURVE WAS LAID OUT AT, by the key of the curve.
struct LaidPlaces {
    key: u64,
    places: std::rc::Rc<[f64]>,
}

/// How many layouts are remembered: a rebuild of a sketch lays each curve out for the arrangement of its regions and
/// again for the points of every region it bounds.
const REMEMBERED_PLACES: usize = 64;

thread_local! {
    static PLACES: std::cell::RefCell<Vec<LaidPlaces>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// THE PLACES OF CURVE `c`, AS LAID OUT BEFORE where it was (`Curve::places`): a rebuild of a sketch of a spline of 688
/// nodes laid it out to its sag four times over.
fn remembered_places(c: &Curve) -> Vec<f64> {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    c.key(&mut h);
    let key = h.finish();
    if let Some(found) = PLACES.with(|m| m.borrow().iter().find(|r| r.key == key).map(|r| r.places.clone())) {
        return found.to_vec();
    }
    let places: std::rc::Rc<[f64]> = c.places_laid(SAG).into();
    PLACES.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= REMEMBERED_PLACES {
            m.remove(0);
        }
        m.push(LaidPlaces { key, places: places.clone() });
    });
    places.to_vec()
}

/// A vector `v` stretched by `k`.
fn mul(v: Point2, k: f64) -> Point2 {
    Point2::new(v.x * k, v.y * k)
}

/// A PIECE OF A CURVE between two of its places.
#[derive(Clone, Copy)]
struct Piece {
    from: f64,
    to: f64,
}

/// A PIECE BEING LAID OUT: its places, the points at them, and the sag it is laid to.
#[derive(Clone, Copy)]
struct Laying {
    piece: Piece,
    ends: [Point2; 2],
    sag: f64,
}
