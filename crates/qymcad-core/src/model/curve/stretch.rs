//! A CURVE STRETCHED FROM ONE OF ITS ENDS TO ANOTHER CURVE: a line along its own line, an arc round its own circle, an arc
//! of an ellipse round its ellipse, a spline by a straight span along the tangent of its end - a new node where that
//! line meets the other curve, the end smooth into it.
use super::*;

/// WHICH END OF A CURVE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Start,
    Finish,
}

/// How far a line or a spline is looked along past its end for the curve it goes to, in millimetres.
const REACH: f64 = 1e5;

impl Curve {
    /// THE SAME CURVE RUN THE OTHER WAY: its start its finish.
    pub fn reversed(&self) -> Curve {
        match self {
            Curve::Line { a, b } => Curve::Line { a: *b, b: *a },
            Curve::Round { centre, r, from, sweep } => Curve::Round { centre: *centre, r: *r, from: from + sweep, sweep: -sweep },
            Curve::Oval { centre, major, minor, from, sweep } => Curve::Oval { centre: *centre, major: *major, minor: *minor, from: from + sweep, sweep: -sweep },
            Curve::Spline { b, ends } => Curve::Spline { b: b.reversed(), ends: *ends },
        }
    }

    /// THE WAY THE CURVE GOES ON PAST ITS FINISH: the line beyond it, the rest of the circle or of the ellipse, the line
    /// along the tangent of a spline's last node. `None` for a closed curve, which has no end.
    fn way_on(&self) -> Option<Curve> {
        if self.closed() {
            return None;
        }
        let end = self.at(1.0);
        Some(match self {
            Curve::Line { .. } | Curve::Spline { .. } => {
                let d = self.tangent(1.0);
                Curve::Line { a: end, b: Point2::new(end.x + d.x * REACH, end.y + d.y * REACH) }
            }
            Curve::Round { centre, r, from, sweep } => Curve::Round { centre: *centre, r: *r, from: from + sweep, sweep: sweep.signum() * (TURN - sweep.abs()) },
            Curve::Oval { centre, major, minor, from, sweep } => Curve::Oval { centre: *centre, major: *major, minor: *minor, from: from + sweep, sweep: sweep.signum() * (TURN - sweep.abs()) },
        })
    }

    /// THE CURVE STRETCHED FROM `end` TO THE NEAREST PLACE ON ITS WAY WHERE IT MEETS `to`, the same kind; `None` for a
    /// closed curve, or when its way never meets `to`.
    pub fn stretched(&self, end: End, to: &Curve) -> Option<Curve> {
        let forward = if end == End::Finish { self.clone() } else { self.reversed() };
        let way = forward.way_on()?;
        let hit = way.meet(to).into_iter().filter(|m| m.ta > 1e-9).min_by(|x, y| x.ta.total_cmp(&y.ta))?;
        let longer = match &forward {
            Curve::Line { a, .. } => Curve::Line { a: *a, b: hit.at },
            Curve::Round { centre, r, from, sweep } => {
                let Curve::Round { sweep: on, .. } = way else { return None };
                Curve::Round { centre: *centre, r: *r, from: *from, sweep: sweep + on * hit.ta }
            }
            Curve::Oval { centre, major, minor, from, sweep } => {
                let Curve::Oval { sweep: on, .. } = way else { return None };
                Curve::Oval { centre: *centre, major: *major, minor: *minor, from: *from, sweep: sweep + on * hit.ta }
            }
            // the straight span on along the tangent of its end, joined on at the pace the curve ends with - smooth to the
            // tangent there
            Curve::Spline { b, ends } => {
                let last = b.at(1.0);
                let run = hit.at - last;
                let on = crate::geom::bspline::BSpline::of_spans(&[crate::geom::bspline::Span { from: last, leave: run, to: hit.at, arrive: run, run: [0.0, 1.0] }])?;
                let (pace, reach) = (b.velocity(1.0).len(), run.len());
                Curve::Spline { b: b.joined(&on, pace / (pace + reach).max(1e-300)), ends: *ends }
            }
        };
        Some(if end == End::Finish { longer } else { longer.reversed() })
    }
}
