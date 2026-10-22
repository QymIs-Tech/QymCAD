//! AN ELLIPSE THROUGH POINTS: the conic `A x^2 + B xy + C y^2 + D x + E y = 1` fitted to the points by least squares,
//! taken for an ellipse when every point stands on it. A flat circle or ellipse seen from any side is an ellipse on the
//! plane it is seen on, so the points an edge of a body is laid out by say whether it is one - no word of the kind of
//! the edge is needed.

use super::*;

/// How far a point may stand off the fitted ellipse, in millimetres, for the points to be taken as on it: the edges of a
/// body come laid out in single precision, a few millionths of a millimetre off at 20 mm, and a cubic span a tenth of a
/// millimetre off a conic at the least.
const ON_IT: f64 = 1e-4;

impl Curve {
    /// THE ELLIPSE, OR THE ARC OF ONE, THROUGH `points` in their order: a whole ellipse when the first and the last point
    /// meet, else the arc from the first to the last the way the points run. `None` for fewer than six points, or points
    /// on no ellipse - a line, a spline, a curve of another kind.
    pub fn oval_through(points: &[Point2]) -> Option<Curve> {
        if points.len() < 6 {
            return None;
        }
        // about the middle of the points and in units of their spread, for the sums to keep their digits
        let n = points.len() as f64;
        let mid = points.iter().fold(Point2::new(0.0, 0.0), |s, p| Point2::new(s.x + p.x / n, s.y + p.y / n));
        let scale = points.iter().map(|p| p.dist(mid)).fold(0.0, f64::max);
        if scale < 1e-9 {
            return None;
        }
        let local: Vec<Point2> = points.iter().map(|p| Point2::new((p.x - mid.x) / scale, (p.y - mid.y) / scale)).collect();
        let [a, b, c, d, e] = least_squares_conic(&local)?;
        // an ellipse, its centre where the gradient of the conic is nought
        let det = 4.0 * a * c - b * b;
        if det <= 1e-12 {
            return None;
        }
        let (x0, y0) = ((b * e - 2.0 * c * d) / det, (b * d - 2.0 * a * e) / det);
        let k = 1.0 - (a * x0 * x0 + b * x0 * y0 + c * y0 * y0 + d * x0 + e * y0);
        // the axes are the eigenvectors of [[a, b/2], [b/2, c]], the semi-axes sqrt(k / eigenvalue)
        let (tr, gap) = (a + c, ((a - c).powi(2) + b * b).sqrt());
        let (l_small, l_big) = ((tr - gap) / 2.0, (tr + gap) / 2.0);
        if l_small <= 0.0 || k <= 0.0 {
            return None;
        }
        let turn = 0.5 * b.atan2(a - c) + std::f64::consts::FRAC_PI_2;
        let (major_len, minor_len) = ((k / l_small).sqrt() * scale, (k / l_big).sqrt() * scale);
        let major = Point2::new(turn.cos() * major_len, turn.sin() * major_len);
        let minor = Point2::new(-turn.sin() * minor_len, turn.cos() * minor_len);
        let centre = Point2::new(mid.x + x0 * scale, mid.y + y0 * scale);
        let angle = |p: Point2| {
            let q = p - centre;
            ((q.x * minor.x + q.y * minor.y) / (minor_len * minor_len)).atan2((q.x * major.x + q.y * major.y) / (major_len * major_len))
        };
        let (first, last) = (points[0], points[points.len() - 1]);
        let whole = first.dist(last) < ON_IT;
        let (from, sweep) = if whole {
            (0.0, TURN)
        } else {
            // the way the points run round the centre, read off the turn from each to the next
            let turned: f64 = points.windows(2).map(|w| (angle(w[1]) - angle(w[0]) + std::f64::consts::PI).rem_euclid(TURN) - std::f64::consts::PI).sum();
            let (t0, t1) = (angle(first), angle(last));
            (t0, if turned > 0.0 { (t1 - t0).rem_euclid(TURN) } else { -(t0 - t1).rem_euclid(TURN) })
        };
        let oval = Curve::Oval { centre, major, minor, from, sweep };
        points.iter().all(|p| oval.nearest(*p).off < ON_IT).then_some(oval)
    }
}

/// The conic `a x^2 + b xy + c y^2 + d x + e y = 1` nearest `points` by least squares, from its normal equations.
fn least_squares_conic(points: &[Point2]) -> Option<[f64; 5]> {
    let mut m = [[0.0; 6]; 5];
    for p in points {
        let row = [p.x * p.x, p.x * p.y, p.y * p.y, p.x, p.y];
        for i in 0..5 {
            for j in 0..5 {
                m[i][j] += row[i] * row[j];
            }
            m[i][5] += row[i];
        }
    }
    // Gauss-Jordan with the largest pivot of each column
    for col in 0..5 {
        let pivot = (col..5).max_by(|x, y| m[*x][col].abs().total_cmp(&m[*y][col].abs()))?;
        if m[pivot][col].abs() < 1e-14 {
            return None;
        }
        m.swap(col, pivot);
        let lead = m[col];
        for (r, row) in m.iter_mut().enumerate() {
            if r != col {
                let f = row[col] / lead[col];
                for (x, l) in row.iter_mut().zip(lead) {
                    *x -= f * l;
                }
            }
        }
    }
    Some([0, 1, 2, 3, 4].map(|i| m[i][5] / m[i][i]))
}
