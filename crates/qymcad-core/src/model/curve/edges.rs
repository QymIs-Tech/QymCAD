//! A CURVE AS THE EDGES OF A PROFILE, AND BACK: what the kernel builds a face from, exact - a line, an arc, a circle, an
//! ellipse or an arc of one, a spline as its B-spline - and what an edge read from a file is laid into a sketch as.
use super::*;
use crate::geom::ProfEdge;

impl Curve {
    /// THE EXACT EDGES OF THE CURVE, in its order: one edge, a spline its B-spline.
    pub fn to_edges(&self) -> Vec<ProfEdge> {
        match self {
            Curve::Line { a, b } => vec![ProfEdge::Line { a: *a, b: *b }],
            Curve::Round { centre, r, .. } if self.closed() => vec![ProfEdge::Circle { center: *centre, r: *r }],
            Curve::Round { centre, sweep, .. } => {
                let [a, b] = self.ends();
                vec![ProfEdge::Arc { a, b, center: *centre, ccw: *sweep >= 0.0 }]
            }
            Curve::Oval { centre, major, minor, from, sweep } => vec![ProfEdge::Oval { centre: *centre, major: *major, minor: *minor, from: *from, sweep: *sweep }],
            Curve::Spline { b, .. } => vec![ProfEdge::Spline(b.clone())],
        }
    }

    /// THE SPLINE OF PIECES OF SPLINES joined end to end, each over the share of the parameter its length takes - a loop
    /// when the last goes on smoothly into the first. `None` for edges that are not splines joined end to end.
    pub fn of_splines(pieces: &[ProfEdge]) -> Option<Curve> {
        let curves: Vec<Curve> = pieces.iter().map(|e| matches!(e, ProfEdge::Spline(_)).then(|| Curve::of_edge(e))).collect::<Option<_>>()?;
        let (first, last) = (curves.first()?, curves.last()?);
        if curves.windows(2).any(|w| w[0].ends()[1].dist(w[1].ends()[0]) > 1e-9) {
            return None;
        }
        let closed = curves.len() > 1 && last.goes_on_smoothly(first);
        let mut whole = first.b_spline()?.clone();
        let mut along = first.length();
        for c in &curves[1..] {
            let l = c.length();
            whole = whole.joined(c.b_spline()?, along / (along + l).max(1e-300));
            along += l;
        }
        Some(Curve::Spline { b: whole, ends: if closed { crate::feature::Ends::Closed } else { crate::feature::Ends::Open } })
    }

    /// WHETHER `next` GOES ON FROM THIS CURVE SMOOTHLY: it starts where this one ends, the way this one ends with, to a
    /// millionth of a radian - a joint of one curve, not a corner between two.
    pub fn goes_on_smoothly(&self, next: &Curve) -> bool {
        let (u, v) = (self.tangent(1.0), next.tangent(0.0));
        self.ends()[1].dist(next.ends()[0]) < 1e-9 && (u.x * v.y - u.y * v.x).abs() < 1e-6 && u.x * v.x + u.y * v.y > 0.0
    }

    /// THE CURVE OF AN EDGE: a spline edge the spline of its B-spline.
    pub fn of_edge(e: &ProfEdge) -> Curve {
        let turn = std::f64::consts::TAU;
        match *e {
            ProfEdge::Line { a, b } => Curve::Line { a, b },
            ProfEdge::Arc { a, b, center, ccw } => {
                let (wa, wb) = ((a.y - center.y).atan2(a.x - center.x), (b.y - center.y).atan2(b.x - center.x));
                let sweep = if ccw { (wb - wa).rem_euclid(turn) } else { -(wa - wb).rem_euclid(turn) };
                Curve::Round { centre: center, r: a.dist(center), from: wa, sweep }
            }
            ProfEdge::Circle { center, r } => Curve::Round { centre: center, r, from: 0.0, sweep: turn },
            ProfEdge::Oval { centre, major, minor, from, sweep } => Curve::Oval { centre, major, minor, from, sweep },
            ProfEdge::Spline(ref b) => Curve::Spline { b: b.clone(), ends: crate::feature::Ends::Open },
        }
    }
}
