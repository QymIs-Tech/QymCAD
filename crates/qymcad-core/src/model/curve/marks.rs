//! THE MARKED PLACES OF A CURVE a person snaps to: its quadrants, and the point half way along it.
use super::*;

/// How far a curve may stand off another and still lie along it, in mm.
const ALONG: f64 = 1e-6;

impl Curve {
    /// THE QUADRANTS OF THE CURVE that lie on it: the four points of a circle or an arc at 0, 90, 180 and 270 deg, the
    /// four ends of the axes of an ellipse or an arc of one; none for a line or a spline.
    pub fn quadrants(&self) -> Vec<Point2> {
        let candidates = match self {
            Curve::Round { centre, r, .. } => [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]].map(|[x, y]| Point2::new(centre.x + r * x, centre.y + r * y)).to_vec(),
            Curve::Oval { centre, major, minor, .. } => [*major, *minor].into_iter().flat_map(|v| [*centre + v, *centre - v]).collect(),
            Curve::Line { .. } | Curve::Spline { .. } => Vec::new(),
        };
        candidates.into_iter().filter(|q| self.nearest(*q).off < 1e-9).collect()
    }

    /// THE POINT HALF WAY ALONG THE CURVE by its length: on an arc the middle of its sweep, on any other the place its
    /// laid out points put half way, taken onto the curve.
    pub fn half_way(&self) -> Point2 {
        if let Curve::Round { .. } = self {
            return self.at(0.5);
        }
        let line = self.polyline();
        let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
        let mut left = total / 2.0;
        for w in line.windows(2) {
            let l = w[0].dist(w[1]);
            if l >= left && l > 0.0 {
                let k = left / l;
                return self.nearest(Point2::new(w[0].x + (w[1].x - w[0].x) * k, w[0].y + (w[1].y - w[0].y) * k)).point;
            }
            left -= l;
        }
        self.at(0.5)
    }

    /// THE PLACES ON THE STRAIGHT THROUGH `a` AND `b` WHERE IT MEETS THE CURVE, as shares of the way from `a` (0) to `b`
    /// (1), past them as well: a line within its own ends, a circle or an ellipse on the stretch of it the curve runs
    /// over. A spline offers none.
    pub fn meets_line(&self, [a, b]: [Point2; 2]) -> Vec<f64> {
        let (p, q) = ([a.x, a.y], [b.x, b.y]);
        let ts: Vec<f64> = match self {
            Curve::Line { a: c, b: d } => line_seg_t(p, q, [c.x, c.y], [d.x, d.y]).into_iter().collect(),
            Curve::Round { centre, r, .. } => line_circle_t(a.x, a.y, b.x, b.y, centre.x, centre.y, *r),
            Curve::Oval { centre, major, minor, .. } => {
                let (ma, mi) = (*centre + *major, *centre + *minor);
                let (ux, uy, big, small) = ellipse_axes(centre.x, centre.y, ma.x, ma.y, mi.x, mi.y);
                line_ellipse_roots(p, q, [centre.x, centre.y], [ux, uy], big, small)
            }
            Curve::Spline { .. } => Vec::new(),
        };
        // a place on the circle or the ellipse past the ends of an arc of it is no place the curve is met
        ts.into_iter().filter(|t| self.straight() || self.nearest(Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)).off < 1e-6).collect()
    }

    /// WHETHER THE CURVE LIES ALONG `other` OVER MORE THAN A POINT, of whatever kinds the two are: a stretch of it - between
    /// the places it is laid out at and the places nearest the ends of the other - stands on the other at both its ends and
    /// its middle, within 1e-6 mm. Two lines along one straight that overlap, two arcs of one circle that share a stretch,
    /// a spline drawn twice.
    pub fn lies_along(&self, other: &Curve) -> bool {
        let on = |p: Point2| other.nearest(p).off < ALONG;
        let mut places = self.places();
        places.extend(other.ends().map(|e| self.nearest(e).t));
        places.sort_by(f64::total_cmp);
        places.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        places.windows(2).any(|w| on(self.at(w[0])) && on(self.at(w[1])) && on(self.at(0.5 * (w[0] + w[1]))))
    }

    /// THE POINTS WHERE THE CURVE MEETS THE SEGMENT FROM `a` TO `b`, of whatever kind the curve is - a spline as well as a
    /// line, a circle or an ellipse - within both.
    pub fn meets_segment(&self, [a, b]: [Point2; 2]) -> Vec<Point2> {
        self.meet(&Curve::Line { a, b }).into_iter().map(|m| m.at).collect()
    }

    /// Whether the curve is a straight line.
    pub fn straight(&self) -> bool {
        matches!(self, Curve::Line { .. })
    }

    /// THE AXES OF AN ELLIPSE OR AN ARC OF ONE: its centre and its two semi-axes as vectors from it; `None` for a curve of
    /// another kind.
    pub fn oval_axes(&self) -> Option<OvalAxes> {
        let Curve::Oval { centre, major, minor, .. } = *self else { return None };
        Some(OvalAxes { centre, major, minor })
    }
}

/// THE AXES OF AN ELLIPSE: its centre, and its major and minor semi-axes as vectors from the centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OvalAxes {
    pub centre: Point2,
    pub major: Point2,
    pub minor: Point2,
}
