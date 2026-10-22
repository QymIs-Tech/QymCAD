//! WHERE TWO CURVES MEET: exactly for a line and a line, a line and a round curve, two round curves; for any other pair
//! found between the points the two are laid out by and narrowed by Newton's steps on both places at once, to the
//! precision of the curves themselves - not of their laid out points.
use super::*;

/// A PLACE TWO CURVES SHARE: the place along the first, along the second, and the point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meet {
    pub ta: f64,
    pub tb: f64,
    pub at: Point2,
}

/// How close two places must come to count as one, in millimetres: the tolerance the sketch stitches points at.
const SAME: f64 = 1e-7;

/// A STRAIGHT PIECE OF THE LAYOUT OF A CURVE: its two points and the places of the curve at them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LaidPiece {
    pub ends: [Point2; 2],
    pub run: [f64; 2],
}

/// The places along a curve with their points, laid out once for a search.
struct Laid {
    t: Vec<f64>,
    p: Vec<Point2>,
}

impl Curve {
    /// THE PLACES WHERE THIS CURVE MEETS `other`, crossing or touching, each once.
    pub fn meet(&self, other: &Curve) -> Vec<Meet> {
        let found = match (self, other) {
            (Curve::Line { .. }, Curve::Line { .. }) => line_line([self, other]),
            (Curve::Line { .. }, Curve::Round { .. }) => line_round([self, other]),
            (Curve::Round { .. }, Curve::Line { .. }) => line_round([other, self]).into_iter().map(|m| Meet { ta: m.tb, tb: m.ta, at: m.at }).collect(),
            (Curve::Round { .. }, Curve::Round { .. }) => round_round([self, other]),
            _ => self.meet_laid(other),
        };
        let mut out: Vec<Meet> = Vec::new();
        for m in found {
            if !out.iter().any(|o| o.at.dist(m.at) < SAME * 10.0) {
                out.push(m);
            }
        }
        out.sort_by(|x, y| x.ta.total_cmp(&y.ta));
        out
    }

    /// The places of the curve as a search lays them out.
    fn laid(&self) -> Laid {
        let t = self.places();
        let p = t.iter().map(|t| self.at(*t)).collect();
        Laid { t, p }
    }

    /// THE MEETINGS OF ANY TWO CURVES: every piece of one against every piece of the other that comes within a few sags of
    /// it, the place where the pieces cross - or come nearest - taken as a start, then Newton's steps on both places
    /// until the two points are one. A start that does not settle onto one point is no meeting.
    fn meet_laid(&self, other: &Curve) -> Vec<Meet> {
        let (la, lb) = (self.laid(), other.laid());
        let mut out = Vec::new();
        for i in 0..la.p.len().saturating_sub(1) {
            let (a0, a1) = (la.p[i], la.p[i + 1]);
            for j in 0..lb.p.len().saturating_sub(1) {
                let (b0, b1) = (lb.p[j], lb.p[j + 1]);
                let near = 4.0 * SAG + 1e-9;
                if a0.x.max(a1.x) + near < b0.x.min(b1.x) || b0.x.max(b1.x) + near < a0.x.min(a1.x) || a0.y.max(a1.y) + near < b0.y.min(b1.y) || b0.y.max(b1.y) + near < a0.y.min(a1.y) {
                    continue;
                }
                let pieces = [LaidPiece { ends: [a0, a1], run: [la.t[i], la.t[i + 1]] }, LaidPiece { ends: [b0, b1], run: [lb.t[j], lb.t[j + 1]] }];
                out.extend(self.meets_near(other, pieces));
            }
        }
        out
    }

    /// WHERE THIS CURVE MEETS `other` BY TWO PIECES OF THEIR LAYOUTS: the place the pieces cross and the ends of either
    /// within a few sags of the other (`pieces_starts`) taken as starts, each settled by Newton's steps on both curves at
    /// once (`settle`); each place once.
    pub(crate) fn meets_near(&self, other: &Curve, pieces: [LaidPiece; 2]) -> Vec<Meet> {
        let [a, b] = pieces;
        let mut out: Vec<Meet> = Vec::new();
        for start in pieces_starts([a.ends, b.ends], 4.0 * SAG + 1e-9) {
            let ta = a.run[0] + (a.run[1] - a.run[0]) * start.x;
            let tb = b.run[0] + (b.run[1] - b.run[0]) * start.y;
            if let Some(m) = self.settle(other, Point2 { x: ta, y: tb }) {
                if !out.iter().any(|o| o.at.dist(m.at) < SAME * 10.0) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// Newton's steps on the places `start` (along this curve, along `other`) until the two points are one, within the
    /// curves; `None` when they do not settle.
    pub(crate) fn settle(&self, other: &Curve, start: Point2) -> Option<Meet> {
        let (mut ta, mut tb) = (start.x, start.y);
        for _ in 0..40 {
            let (pa, pb) = (self.at(ta), other.at(tb));
            let f = pa - pb;
            if f.len() < SAME * 1e-3 {
                break;
            }
            let (va, vb) = (self.velocity(ta), other.velocity(tb));
            // [va  -vb] [dta dtb]^T = -f
            let det = va.x * (-vb.y) - va.y * (-vb.x);
            if det.abs() < 1e-18 {
                // the two run the same way here: a touch, settled by stepping along the first and dropping onto the second
                let l2 = va.x * va.x + va.y * va.y;
                if l2 < 1e-30 {
                    break;
                }
                let along = -(f.x * va.x + f.y * va.y) / l2;
                ta = (ta + along).clamp(0.0, 1.0);
                tb = other.nearest(self.at(ta)).t;
                continue;
            }
            let dta = (-f.x * (-vb.y) - (-f.y) * (-vb.x)) / det;
            let dtb = (va.x * (-f.y) - va.y * (-f.x)) / det;
            ta = (ta + dta).clamp(0.0, 1.0);
            tb = (tb + dtb).clamp(0.0, 1.0);
        }
        let (pa, pb) = (self.at(ta), other.at(tb));
        (pa.dist(pb) < SAME).then(|| Meet { ta, tb, at: Point2::new(0.5 * (pa.x + pb.x), 0.5 * (pa.y + pb.y)) })
    }
}

/// WHERE TWO STRAIGHT PIECES CROSS, as the shares along each (x along the first, y along the second); or, where they do
/// not cross but come within `near` of each other, the shares of their nearest points. `None` when they stay apart.
fn pieces_meet(pieces: [[Point2; 2]; 2], near: f64) -> Option<Point2> {
    pieces_starts(pieces, near).first().copied()
}

/// WHERE TWO STRAIGHT PIECES CROSS AND COME NEAR, as shares along each: the crossing first where they cross, then the
/// ends of either that stand within `near` of the other, nearest first. Two curves nearly touching cross twice within a
/// piece of their layouts, and the second crossing is found from an end.
fn pieces_starts(pieces: [[Point2; 2]; 2], near: f64) -> Vec<Point2> {
    let [a, b] = pieces;
    let mut out = Vec::new();
    let (r, s) = (a[1] - a[0], b[1] - b[0]);
    let q = b[0] - a[0];
    let den = r.x * s.y - r.y * s.x;
    if den.abs() > 1e-18 {
        let (t, u) = ((q.x * s.y - q.y * s.x) / den, (q.x * r.y - q.y * r.x) / den);
        if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) {
            out.push(Point2 { x: t.clamp(0.0, 1.0), y: u.clamp(0.0, 1.0) });
        }
    }
    // the ends of either piece near the other
    let share = |p: Point2, seg: [Point2; 2]| {
        let d = seg[1] - seg[0];
        let l2 = d.x * d.x + d.y * d.y;
        if l2 < 1e-30 {
            0.0
        } else {
            (((p.x - seg[0].x) * d.x + (p.y - seg[0].y) * d.y) / l2).clamp(0.0, 1.0)
        }
    };
    let close = |p: Point2, seg: [Point2; 2], at: Point2| {
        let k = share(p, seg);
        Close { off: Point2::new(seg[0].x + (seg[1].x - seg[0].x) * k, seg[0].y + (seg[1].y - seg[0].y) * k).dist(p), at }
    };
    let mut closes: Vec<Close> = [
        close(a[0], b, Point2 { x: 0.0, y: share(a[0], b) }),
        close(a[1], b, Point2 { x: 1.0, y: share(a[1], b) }),
        close(b[0], a, Point2 { x: share(b[0], a), y: 0.0 }),
        close(b[1], a, Point2 { x: share(b[1], a), y: 1.0 }),
    ]
    .into_iter()
    .filter(|c| c.off <= near)
    .collect();
    closes.sort_by(|x, y| x.off.total_cmp(&y.off));
    out.extend(closes.into_iter().map(|c| c.at));
    out
}

/// AN END OF ONE PIECE NEAR THE OTHER: how far it is, and the shares along the two pieces there.
struct Close {
    off: f64,
    at: Point2,
}

/// Where two lines meet: the place they cross within both, or none for parallel ones.
fn line_line(pair: [&Curve; 2]) -> Vec<Meet> {
    let [a, b] = pair;
    let (Curve::Line { a: a0, b: a1 }, Curve::Line { a: b0, b: b1 }) = (a, b) else { return Vec::new() };
    match pieces_meet([[*a0, *a1], [*b0, *b1]], 0.0) {
        Some(k) if (a1.x - a0.x) * (b1.y - b0.y) - (a1.y - a0.y) * (b1.x - b0.x) != 0.0 => vec![Meet { ta: k.x, tb: k.y, at: a.at(k.x) }],
        _ => Vec::new(),
    }
}

/// Where a line meets a round curve: the roots of the line on the circle, kept where both hold them. A line within
/// `SAME` of the circle touches it once, at the foot of the centre - a discriminant near zero tells nothing at any scale.
fn line_round(pair: [&Curve; 2]) -> Vec<Meet> {
    let [line, round] = pair;
    let (Curve::Line { a, b }, Curve::Round { centre, r, .. }) = (line, round) else { return Vec::new() };
    let d = *b - *a;
    let l2 = d.x * d.x + d.y * d.y;
    if l2 < 1e-30 {
        return Vec::new();
    }
    let k = ((centre.x - a.x) * d.x + (centre.y - a.y) * d.y) / l2;
    let foot = Point2::new(a.x + d.x * k, a.y + d.y * k);
    let h = foot.dist(*centre);
    let roots: Vec<f64> = if (h - r).abs() < SAME {
        vec![k]
    } else if h > *r {
        Vec::new()
    } else {
        let w = (r * r - h * h).sqrt() / l2.sqrt();
        vec![k - w, k + w]
    };
    roots
        .into_iter()
        .filter(|t| (-1e-12..=1.0 + 1e-12).contains(t))
        .filter_map(|t| {
            let p = line.at(t.clamp(0.0, 1.0));
            let on = round.nearest(p);
            (on.off < SAME * 10.0).then_some(Meet { ta: t.clamp(0.0, 1.0), tb: on.t, at: p })
        })
        .collect()
}

/// Where two round curves meet: the crossings of their circles, kept where both hold them; one where they touch.
fn round_round(pair: [&Curve; 2]) -> Vec<Meet> {
    let [a, b] = pair;
    let (Curve::Round { centre: c1, r: r1, .. }, Curve::Round { centre: c2, r: r2, .. }) = (a, b) else { return Vec::new() };
    let d = c1.dist(*c2);
    if d < 1e-12 || d > r1 + r2 + SAME || d < (r1 - r2).abs() - SAME {
        return Vec::new();
    }
    let x = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
    let y = (r1 * r1 - x * x).max(0.0).sqrt();
    let (ux, uy) = ((c2.x - c1.x) / d, (c2.y - c1.y) / d);
    let base = Point2::new(c1.x + ux * x, c1.y + uy * x);
    let pts = if y < SAME { vec![base] } else { vec![Point2::new(base.x - uy * y, base.y + ux * y), Point2::new(base.x + uy * y, base.y - ux * y)] };
    pts.into_iter()
        .filter_map(|p| {
            let (on_a, on_b) = (a.nearest(p), b.nearest(p));
            (on_a.off < SAME * 10.0 && on_b.off < SAME * 10.0).then_some(Meet { ta: on_a.t, tb: on_b.t, at: p })
        })
        .collect()
}
