//! THE JACOBIAN OF THE CONSTRAINTS: their rows, checked against differences, the degrees of freedom.

use super::*;

/// Analytic Jacobian of a single constraint: the exact derivatives of every residual row with respect to its own
/// variables.
///
/// The Jacobian used to be computed by finite differences, and that scheme has two flaws. Accuracy is limited by
/// the step: at coordinates of hundreds of millimetres the solution stalled with an error around 1e-5, so a
/// 130 mm dimension came out as −129.99900 and a cut left a film instead of an opening. And the cost is an extra
/// residual evaluation per variable per iteration. With the derivatives written out, the solution settles onto a
/// dimension to machine precision and exactly as many rows are computed as are needed.
///
/// Format: `out[k]` is row k of this constraint, a list of `(variable index, ∂r_k/∂x)`. The row order matches
/// `con_rows` exactly, which the numerical cross-check test verifies for every type.
pub(crate) fn con_jac(c: &Constraint, x: &[f64], x0: &[f64], idx: &HashMap<Id, usize>, ridx: &HashMap<Id, usize>, out: &mut Vec<Vec<(usize, f64)>>) {
    let g = |id: Id| -> (f64, f64) {
        let i = idx[&id];
        (x[2 * i], x[2 * i + 1])
    };
    let vx = |id: Id| -> usize { 2 * idx[&id] };
    let vy = |id: Id| -> usize { 2 * idx[&id] + 1 };
    let rv = |id: Id| -> Option<usize> { ridx.get(&id).copied() };
    // derivatives of the distance |P−Q| with respect to the coordinates of P and Q
    let dist_rows = |p: Id, q: Id, sign: f64, row: &mut Vec<(usize, f64)>| {
        let ((px, py), (qx, qy)) = (g(p), g(q));
        let (dx, dy) = (px - qx, py - qy);
        let l = (dx * dx + dy * dy).sqrt().max(1e-12);
        row.push((vx(p), sign * dx / l));
        row.push((vy(p), sign * dy / l));
        row.push((vx(q), -sign * dx / l));
        row.push((vy(q), -sign * dy / l));
    };

    match *c {
        Constraint::Fixed { p } => {
            const W_FIX: f64 = 50.0;
            out.push(vec![(vx(p), W_FIX)]);
            out.push(vec![(vy(p), W_FIX)]);
        }
        Constraint::Horizontal { a, b } => out.push(vec![(vy(a), 1.0), (vy(b), -1.0)]),
        Constraint::Vertical { a, b } => out.push(vec![(vx(a), 1.0), (vx(b), -1.0)]),
        Constraint::Orientation { a, b, deg } => {
            let (sn, cs) = deg.to_radians().sin_cos();
            out.push(vec![(vx(b), sn), (vy(b), -cs), (vx(a), -sn), (vy(a), cs)]);
        }
        Constraint::Coincident { a, b } | Constraint::Concentric { c1: a, c2: b } => {
            out.push(vec![(vx(a), 1.0), (vx(b), -1.0)]);
            out.push(vec![(vy(a), 1.0), (vy(b), -1.0)]);
        }
        Constraint::Distance { a, b, axis, .. } => match axis {
            1 | 2 => {
                let k = if axis == 1 { 0 } else { 1 };
                let (ia, ib) = (idx[&a], idx[&b]);
                let cur = x[2 * ia + k] - x[2 * ib + k];
                let d0 = x0[2 * ia + k] - x0[2 * ib + k];
                // |Δ| − d away from the degenerate case, where the derivative is the sign of Δ; signed Δ − d at
                // zero, see `con_rows`
                let s = if d0.abs() > 1e-9 {
                    if cur < 0.0 {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                out.push(vec![(2 * ia + k, s), (2 * ib + k, -s)]);
            }
            _ => {
                let mut row = Vec::new();
                dist_rows(a, b, 1.0, &mut row);
                out.push(row);
            }
        },
        Constraint::EdgeDistance { c1, c2, m1, m2, .. } => {
            let mut row = Vec::new();
            dist_rows(c2, c1, 1.0, &mut row); // dist = |P2−P1|
            if let Some(i) = rv(c1) {
                row.push((i, m1 as f64));
            }
            if let Some(i) = rv(c2) {
                row.push((i, m2 as f64));
            }
            out.push(row);
        }
        Constraint::Parallel { a, b, c: cc, d } => {
            let ((ax, ay), (bx, by)) = (g(a), g(b));
            let ((cx, cy), (dx, dy)) = (g(cc), g(d));
            let (ux, uy) = (bx - ax, by - ay);
            let (wx, wy) = (dx - cx, dy - cy);
            out.push(vec![(vx(a), -wy), (vy(a), wx), (vx(b), wy), (vy(b), -wx), (vx(cc), uy), (vy(cc), -ux), (vx(d), -uy), (vy(d), ux)]);
        }
        Constraint::Perpendicular { a, b, c: cc, d } => {
            let ((ax, ay), (bx, by)) = (g(a), g(b));
            let ((cx, cy), (dx, dy)) = (g(cc), g(d));
            let (ux, uy) = (bx - ax, by - ay);
            let (wx, wy) = (dx - cx, dy - cy);
            out.push(vec![(vx(a), -wx), (vy(a), -wy), (vx(b), wx), (vy(b), wy), (vx(cc), -ux), (vy(cc), -uy), (vx(d), ux), (vy(d), uy)]);
        }
        Constraint::Equal { a, b, c: cc, d } => {
            let mut row = Vec::new();
            dist_rows(a, b, 1.0, &mut row);
            dist_rows(cc, d, -1.0, &mut row);
            out.push(row);
        }
        // two points on a line, the rows of `PointOnLine` for each end of the second segment - see the residual
        Constraint::Collinear { a, b, c: cc, d } => {
            let ((ax, ay), (bx, by)) = (g(a), g(b));
            let (dx, dy) = (bx - ax, by - ay);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            for far in [cc, d] {
                let (px, py) = g(far);
                let cross = dx * (py - ay) - dy * (px - ax);
                let dc = [(vx(far), -dy), (vy(far), dx), (vx(a), -(py - ay) + dy), (vy(a), -dx + (px - ax)), (vx(b), py - ay), (vy(b), -(px - ax))];
                let dl = [(vx(a), -dx / len), (vy(a), -dy / len), (vx(b), dx / len), (vy(b), dy / len)];
                let mut row: Vec<(usize, f64)> = dc.iter().map(|&(i, v)| (i, v / len)).collect();
                for &(i, v) in &dl {
                    row.push((i, -cross * v / (len * len)));
                }
                out.push(row);
            }
        }
        Constraint::Midpoint { p, a, b } => {
            out.push(vec![(vx(p), 1.0), (vx(a), -0.5), (vx(b), -0.5)]);
            out.push(vec![(vy(p), 1.0), (vy(a), -0.5), (vy(b), -0.5)]);
        }
        Constraint::Tangent { a, b, c: cc, .. } => {
            let ((ax, ay), (bx, by), (cx, cy)) = (g(a), g(b), g(cc));
            let (dx, dy) = (bx - ax, by - ay);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let cross = dx * (cy - ay) - dy * (cx - ax);
            let s = if cross < 0.0 { -1.0 } else { 1.0 };
            // f = s·cross/len  →  ∂f = s·(∂cross/len − cross·∂len/len²)
            let dc = [(vx(a), -(cy - ay) + dy), (vy(a), -dx + (cx - ax)), (vx(b), cy - ay), (vy(b), -(cx - ax)), (vx(cc), -dy), (vy(cc), dx)];
            let dl = [(vx(a), -dx / len), (vy(a), -dy / len), (vx(b), dx / len), (vy(b), dy / len)];
            let mut row: Vec<(usize, f64)> = dc.iter().map(|&(i, v)| (i, s * v / len)).collect();
            for &(i, v) in &dl {
                row.push((i, -s * cross * v / (len * len)));
            }
            if let Some(i) = rv(cc) {
                row.push((i, -1.0));
            }
            out.push(row);
        }
        Constraint::CircleTangent { c1, c2, external } => {
            let mut row = Vec::new();
            dist_rows(c2, c1, 1.0, &mut row);
            let (r1, r2) = (rv(c1).map(|i| x[i]).unwrap_or(0.0), rv(c2).map(|i| x[i]).unwrap_or(0.0));
            let (g1, g2) = if external {
                (-1.0, -1.0)
            } else {
                let s = if r1 - r2 < 0.0 { -1.0 } else { 1.0 };
                (-s, s)
            };
            if let Some(i) = rv(c1) {
                row.push((i, g1));
            }
            if let Some(i) = rv(c2) {
                row.push((i, g2));
            }
            out.push(row);
        }
        Constraint::Symmetric { a, b, la, lb } => {
            let ((ax, ay), (bx, by)) = (g(a), g(b));
            let ((lax, lay), (lbx, lby)) = (g(la), g(lb));
            let (dx, dy) = (lbx - lax, lby - lay);
            let (mx, my) = (0.5 * (ax + bx), 0.5 * (ay + by));
            // both residuals divided by the length of the axis, L (`con_rows`): r = f / L, so dr = df / L - f d.dd / L^3,
            // the axis's own ends carrying the second term
            let l = dx.hypot(dy).max(1e-9);
            let (f1, f2) = (dx * (my - lay) - dy * (mx - lax), (bx - ax) * dx + (by - ay) * dy);
            let along = |f: f64| (f * dx / (l * l * l), f * dy / (l * l * l));
            let (g1x, g1y) = along(f1);
            out.push(vec![
                (vx(a), -dy * 0.5 / l),
                (vy(a), dx * 0.5 / l),
                (vx(b), -dy * 0.5 / l),
                (vy(b), dx * 0.5 / l),
                (vx(la), (-(my - lay) + dy) / l + g1x),
                (vy(la), ((mx - lax) - dx) / l + g1y),
                (vx(lb), (my - lay) / l - g1x),
                (vy(lb), -(mx - lax) / l - g1y),
            ]);
            let (g2x, g2y) = along(f2);
            out.push(vec![
                (vx(a), -dx / l),
                (vy(a), -dy / l),
                (vx(b), dx / l),
                (vy(b), dy / l),
                (vx(la), -(bx - ax) / l + g2x),
                (vy(la), -(by - ay) / l + g2y),
                (vx(lb), (bx - ax) / l - g2x),
                (vy(lb), (by - ay) / l - g2y),
            ]);
        }
        Constraint::Angle { a, b, c: cc, .. } => {
            let ((ax, ay), (bx, by), (cx, cy)) = (g(a), g(b), g(cc));
            let (ux, uy) = (ax - bx, ay - by);
            let (wx, wy) = (cx - bx, cy - by);
            let cross = ux * wy - uy * wx;
            let dot = ux * wx + uy * wy;
            let s = if cross < 0.0 { -1.0 } else { 1.0 };
            let den = (cross * cross + dot * dot).max(1e-18);
            // θ = atan2(|cross|, dot): ∂θ/∂cross = s·dot/den, ∂θ/∂dot = −|cross|/den
            let (kc, kd) = (s * dot / den, -cross.abs() / den);
            let dcross = [(vx(a), wy), (vy(a), -wx), (vx(cc), -uy), (vy(cc), ux), (vx(b), -wy + uy), (vy(b), wx - ux)];
            let ddot = [(vx(a), wx), (vy(a), wy), (vx(cc), ux), (vy(cc), uy), (vx(b), -wx - ux), (vy(b), -wy - uy)];
            let mut row: Vec<(usize, f64)> = dcross.iter().map(|&(i, v)| (i, kc * v)).collect();
            for &(i, v) in &ddot {
                row.push((i, kd * v));
            }
            out.push(row);
        }
        Constraint::AngleLines { a, b, c: cc, d, .. } => {
            let ((ax, ay), (bx, by)) = (g(a), g(b));
            let ((cx, cy), (dx, dy)) = (g(cc), g(d));
            let (ux, uy) = (bx - ax, by - ay);
            let (wx, wy) = (dx - cx, dy - cy);
            let cross = ux * wy - uy * wx;
            let dot = ux * wx + uy * wy;
            let s = if cross < 0.0 { -1.0 } else { 1.0 };
            let den = (cross * cross + dot * dot).max(1e-18);
            let (kc, kd) = (s * dot / den, -cross.abs() / den);
            let dcross = [(vx(a), -wy), (vy(a), wx), (vx(b), wy), (vy(b), -wx), (vx(cc), uy), (vy(cc), -ux), (vx(d), -uy), (vy(d), ux)];
            let ddot = [(vx(a), -wx), (vy(a), -wy), (vx(b), wx), (vy(b), wy), (vx(cc), -ux), (vy(cc), -uy), (vx(d), ux), (vy(d), uy)];
            let mut row: Vec<(usize, f64)> = dcross.iter().map(|&(i, v)| (i, kc * v)).collect();
            for &(i, v) in &ddot {
                row.push((i, kd * v));
            }
            out.push(row);
        }
        Constraint::PointOnLine { p, a, b } | Constraint::DistancePL { p, a, b, .. } => {
            let ((px, py), (ax, ay), (bx, by)) = (g(p), g(a), g(b));
            let (dx, dy) = (bx - ax, by - ay);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let cross = dx * (py - ay) - dy * (px - ax);
            let dc = [(vx(p), -dy), (vy(p), dx), (vx(a), -(py - ay) + dy), (vy(a), -dx + (px - ax)), (vx(b), py - ay), (vy(b), -(px - ax))];
            let dl = [(vx(a), -dx / len), (vy(a), -dy / len), (vx(b), dx / len), (vy(b), dy / len)];
            let mut row: Vec<(usize, f64)> = dc.iter().map(|&(i, v)| (i, v / len)).collect();
            for &(i, v) in &dl {
                row.push((i, -cross * v / (len * len)));
            }
            out.push(row);
        }
        Constraint::Diameter { c: cc, .. } => {
            if let Some(i) = rv(cc) {
                out.push(vec![(i, 1.0)]);
            }
        }
        Constraint::EqualRadius { c1, c2 } => {
            if let (Some(i1), Some(i2)) = (rv(c1), rv(c2)) {
                out.push(vec![(i1, 1.0), (i2, -1.0)]);
            }
        }
        Constraint::PointOnCircle { p, c: cc } => {
            let mut row = Vec::new();
            dist_rows(p, cc, 1.0, &mut row);
            if let Some(i) = rv(cc) {
                row.push((i, -1.0));
            }
            out.push(row);
        }
        Constraint::ArcLength { c: cc, a, b, ccw, .. } => {
            let ((cx, cy), (ax, ay), (bx, by)) = (g(cc), g(a), g(b));
            let (rx, ry) = (ax - cx, ay - cy);
            let rad = (rx * rx + ry * ry).sqrt().max(1e-12);
            let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
            let theta = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
            let sgn = if ccw { 1.0 } else { -1.0 };
            // r = R·θ − len; ∂R with respect to (a, c); ∂θ through ∂atan2 at both endpoints
            let (bx0, by0) = (bx - cx, by - cy);
            let rb2 = (bx0 * bx0 + by0 * by0).max(1e-18);
            let ra2 = (rx * rx + ry * ry).max(1e-18);
            out.push(vec![
                (vx(a), theta * rx / rad + rad * (-sgn) * (-ry / ra2)),
                (vy(a), theta * ry / rad + rad * (-sgn) * (rx / ra2)),
                (vx(b), rad * sgn * (-by0 / rb2)),
                (vy(b), rad * sgn * (bx0 / rb2)),
                (vx(cc), -theta * rx / rad + rad * (sgn * (-ry / ra2) - sgn * (-by0 / rb2))),
                (vy(cc), -theta * ry / rad + rad * (sgn * (rx / ra2) - sgn * (bx0 / rb2))),
            ]);
        }
    }
}

/// Residuals of a single constraint, one or two rows, appended to `r`. One body of code serves both the full
/// residual vector and the sparse Jacobian: a constraint touches at most eight variables, so only those and only
/// its own rows are differentiated.
pub(crate) fn con_rows(c: &Constraint, x: &[f64], x0: &[f64], idx: &HashMap<Id, usize>, ridx: &HashMap<Id, usize>, anchor: &HashMap<Id, (f64, f64)>, r: &mut Vec<f64>) {
    let g = |id: Id| -> (f64, f64) {
        let i = idx[&id];
        (x[2 * i], x[2 * i + 1])
    };
    // radius of the circle centred at `id` as a solver variable, when there is one
    let gr = |id: Id| -> Option<f64> { ridx.get(&id).map(|&i| x[i]) };
    {
        match *c {
            Constraint::Fixed { p } => {
                // A hard anchor: the large weight keeps an anchored point from drifting while the geometry is
                // edited and from yielding to the drag residual. Scaling a row does not change the rank, so
                // the degree-of-freedom count stays correct.
                const W_FIX: f64 = 50.0;
                let (px, py) = g(p);
                let (ax, ay) = anchor[&p];
                r.push(W_FIX * (px - ax));
                r.push(W_FIX * (py - ay));
            }
            Constraint::Horizontal { a, b } => r.push(g(a).1 - g(b).1),
            Constraint::Vertical { a, b } => r.push(g(a).0 - g(b).0),
            Constraint::Orientation { a, b, deg } => {
                // the side crossed with the direction held: linear in the points, and at 0 deg exactly a Horizontal. An
                // angle residual (atan2) has a slope of 1 / length^2 and let a side squeezed short by contradicting
                // constraints drift a point by 7.6 mm from one solve to the next.
                let ((ax, ay), (bx, by)) = (g(a), g(b));
                let (sn, cs) = deg.to_radians().sin_cos();
                r.push((bx - ax) * sn - (by - ay) * cs);
            }
            Constraint::Coincident { a, b } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                r.push(ax - bx);
                r.push(ay - by);
            }
            Constraint::Distance { a, b, d, axis, .. } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                match axis {
                    // The kink of |Δ| at zero. Away from zero the absolute value is smooth and keeps both
                    // sides of the solution available, which matters: a chain of dimensions may require
                    // flipping a side, and a signed residual would then freeze a perfectly solvable system
                    // into a least-squares compromise. Exactly at zero, however, there is no derivative at
                    // all, and the dimension used to be pulled out only by an accident of the forward
                    // difference, where h > 0 produced +1 where no honest derivative exists; switching to a
                    // central difference would have zeroed the row and the dimension would have stopped
                    // working silently. Hence: a non-degenerate start keeps |Δ| − d, while a degenerate one —
                    // a point collapsed onto a point, or a strictly vertical segment — uses the signed
                    // residual Δ − d, with a deterministic positive direction and a real derivative of 1.
                    1 | 2 => {
                        let (ia, ib) = (idx[&a], idx[&b]);
                        let k = if axis == 1 { 0 } else { 1 };
                        let cur = if k == 0 { ax - bx } else { ay - by };
                        let d0 = x0[2 * ia + k] - x0[2 * ib + k];
                        if d0.abs() > 1e-9 {
                            r.push(cur.abs() - d);
                        } else {
                            r.push(cur - d);
                        }
                    }
                    _ => r.push(((ax - bx).powi(2) + (ay - by).powi(2)).sqrt() - d), // aligned distance
                }
            }
            Constraint::EdgeDistance { c1, c2, d, m1, m2, .. } => {
                // distance between centres ± the radii, i.e. measured to the rim: a tangent dimension
                let (x1, y1) = g(c1);
                let (x2, y2) = g(c2);
                let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                let off = m1 as f64 * gr(c1).unwrap_or(0.0) + m2 as f64 * gr(c2).unwrap_or(0.0);
                r.push(dist + off - d);
            }
            Constraint::Parallel { a, b, c, d } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (dx, dy) = g(d);
                r.push((bx - ax) * (dy - cy) - (by - ay) * (dx - cx));
            }
            Constraint::Perpendicular { a, b, c, d } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (dx, dy) = g(d);
                r.push((bx - ax) * (dx - cx) + (by - ay) * (dy - cy));
            }
            Constraint::Equal { a, b, c, d } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (dx, dy) = g(d);
                let l1 = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                let l2 = ((cx - dx).powi(2) + (cy - dy).powi(2)).sqrt();
                r.push(l1 - l2);
            }
            // BOTH ENDS OF THE SECOND SEGMENT LIE ON THE LINE OF THE FIRST, each as a distance - the cross product divided by
            // the length, as `PointOnLine` has it. Undivided, the residual was met by a first segment of no length: two
            // lines made collinear came out with the first shrunk to a point and the residual 0.
            Constraint::Collinear { a, b, c, d } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (ux, uy) = (bx - ax, by - ay);
                let len = (ux * ux + uy * uy).sqrt().max(1e-9);
                for (px, py) in [g(c), g(d)] {
                    r.push((ux * (py - ay) - uy * (px - ax)) / len);
                }
            }
            Constraint::Midpoint { p, a, b } => {
                let (px, py) = g(p);
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                r.push(px - 0.5 * (ax + bx));
                r.push(py - 0.5 * (ay + by));
            }
            Constraint::Tangent { a, b, c, r: rad } => {
                // the line a→b touches the circle centred at c: distance(c, line) = radius. The radius is the
                // circle's own solver variable when it has one, otherwise the fixed `rad` (an arc, or a
                // fallback).
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (dx, dy) = (bx - ax, by - ay);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                let dist = ((dx * (cy - ay) - dy * (cx - ax)) / len).abs();
                r.push(dist - gr(c).unwrap_or(rad));
            }
            Constraint::CircleTangent { c1, c2, external } => {
                // tangency of two circles: centre distance = r1 + r2 for external, |r1 − r2| for internal
                let (x1, y1) = g(c1);
                let (x2, y2) = g(c2);
                let d = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                let (r1, r2) = (gr(c1).unwrap_or(0.0), gr(c2).unwrap_or(0.0));
                let target = if external { r1 + r2 } else { (r1 - r2).abs() };
                r.push(d - target);
            }
            Constraint::Symmetric { a, b, la, lb } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (lax, lay) = g(la);
                let (lbx, lby) = g(lb);
                let (dx, dy) = (lbx - lax, lby - lay);
                let (mx, my) = (0.5 * (ax + bx), 0.5 * (ay + by));
                // DIVIDED BY THE LENGTH OF THE AXIS: the distance of the middle from the axis, and the run of the pair
                // along it. Times the length, as they were, both vanish on an axis of no length, and the solve met the
                // symmetry by shrinking its axis to a point - an axis line held on the Y axis by one end collapsed to
                // (0, 0) under a pair at (10, 5) and (30, 25), and the constraint was refused.
                let l = dx.hypot(dy).max(1e-9);
                r.push((dx * (my - lay) - dy * (mx - lax)) / l);
                r.push(((bx - ax) * dx + (by - ay) * dy) / l);
            }
            Constraint::Angle { a, b, c, deg, .. } => {
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (ux, uy) = (ax - bx, ay - by);
                let (vx, vy) = (cx - bx, cy - by);
                // Length-independent angular residual: θ(actual) − θ(target), with θ = atan2(|u×v|, u·v) in
                // [0, π]. The radial component of the gradient is zero, so this only rotates and the lengths
                // are held by `w_len`. A cosine residual was used before, and its gradient dies at 0° and
                // 180°: the solver stopped one or two degrees short of the target, resolving 179° as 177.3°.
                // The atan2 form keeps |∂r/∂θ| = 1 across the whole range and is sign-agnostic through
                // |cross|, so it does not mirror sides.
                let cross = ux * vy - uy * vx;
                let dot = ux * vx + uy * vy;
                r.push(cross.abs().atan2(dot) - deg.to_radians());
            }
            Constraint::PointOnLine { p, a, b } => {
                // signed perpendicular distance from point p to the line a→b equals zero
                let (px, py) = g(p);
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (dx, dy) = (bx - ax, by - ay);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                r.push((dx * (py - ay) - dy * (px - ax)) / len);
            }
            Constraint::DistancePL { p, a, b, d, .. } => {
                // signed perpendicular distance from point p to the line a→b equals d. Keeping d signed
                // preserves the side, so the point is not mirrored across the line during a solve.
                let (px, py) = g(p);
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (dx, dy) = (bx - ax, by - ay);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                let perp = (dx * (py - ay) - dy * (px - ax)) / len;
                r.push(perp - d);
            }
            Constraint::Diameter { c, d, diam, .. } => {
                // the circle's radius variable equals the given value (a diameter halves to r = d/2); it
                // counts as a real constraint in the degree-of-freedom analysis
                if let Some(rv) = gr(c) {
                    r.push(rv - if diam { d * 0.5 } else { d });
                }
            }
            Constraint::EqualRadius { c1, c2 } => {
                if let (Some(r1), Some(r2)) = (gr(c1), gr(c2)) {
                    r.push(r1 - r2);
                }
            }
            Constraint::PointOnCircle { p, c } => {
                // point p lies on the circle centred at c: distance(p, c) equals the radius variable of c.
                // This is what keeps the endpoints of an arc intrinsically on its own circle, making the arc
                // a real one.
                let (px, py) = g(p);
                let (cx, cy) = g(c);
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                r.push(d - gr(c).unwrap_or(d));
            }
            Constraint::Concentric { c1, c2 } => {
                let (ax, ay) = g(c1);
                let (bx, by) = g(c2);
                r.push(ax - bx);
                r.push(ay - by);
            }
            Constraint::ArcLength { c, a, b, ccw, len, .. } => {
                // arc length = R·θ, with R = |c→a| and θ the angle swept from a to b in the `ccw` direction
                let (cx, cy) = g(c);
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let rad = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
                let (a0, a1) = ((ay - cy).atan2(ax - cx), (by - cy).atan2(bx - cx));
                let theta = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
                r.push(rad * theta - len);
            }
            Constraint::AngleLines { a, b, c, d, deg, .. } => {
                // angle between the directions of the segments a→b and c→d
                let (ax, ay) = g(a);
                let (bx, by) = g(b);
                let (cx, cy) = g(c);
                let (dx, dy) = g(d);
                let (ux, uy) = (bx - ax, by - ay);
                let (vx, vy) = (dx - cx, dy - cy);
                // atan2 residual, as in `Angle`: θ − θ₀ instead of a cosine residual, so the gradient does
                // not die at 0° and 180°. Length-independent and sign-agnostic; the lengths are held by
                // `w_len`.
                let cross = ux * vy - uy * vx;
                let dot = ux * vx + uy * vy;
                r.push(cross.abs().atan2(dot) - deg.to_radians());
            }
        }
    }
}

/// Whether a constraint can be evaluated at all. It is not enough that its points exist — it must also have
/// something to measure against.
///
/// `PointOnCircle { p, c }` needs the radius of the circle centred at `c`. If `c` is not a centre, as in a
/// damaged file or under an outside caller, there is no radius, the residual degenerates into "distance minus
/// the same distance" = 0, and the constraint becomes a silent no-op: the point looks constrained, the solver
/// does not see it, and the degree-of-freedom count does not drop. Rejecting it outright is better than
/// accepting it and not counting it.
pub(crate) fn cons_ok(c: &Constraint, has: &impl Fn(Id) -> bool, is_center: &impl Fn(Id) -> bool) -> bool {
    match *c {
        Constraint::Fixed { p } => has(p),
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => {
            has(a) && has(b)
        }
        Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Equal { a, b, c, d } | Constraint::Collinear { a, b, c, d } => {
            has(a) && has(b) && has(c) && has(d)
        }
        Constraint::Angle { a, b, c, .. } => has(a) && has(b) && has(c),
        Constraint::Midpoint { p, a, b } => has(p) && has(a) && has(b),
        Constraint::Tangent { a, b, c, .. } => has(a) && has(b) && has(c),
        Constraint::Symmetric { a, b, la, lb } => has(a) && has(b) && has(la) && has(lb),
        Constraint::PointOnLine { p, a, b } => has(p) && has(a) && has(b),
        Constraint::DistancePL { p, a, b, .. } => has(p) && has(a) && has(b),
        Constraint::EdgeDistance { c1, c2, .. } => has(c1) && has(c2),
        Constraint::Diameter { c, .. } => has(c),
        Constraint::EqualRadius { c1, c2 } => has(c1) && has(c2),
        Constraint::CircleTangent { c1, c2, .. } => has(c1) && has(c2),
        Constraint::PointOnCircle { p, c } => has(p) && has(c) && is_center(c),
        Constraint::Concentric { c1, c2 } => has(c1) && has(c2),
        Constraint::ArcLength { c, a, b, .. } => has(c) && has(a) && has(b),
        Constraint::AngleLines { a, b, c, d, .. } => has(a) && has(b) && has(c) && has(d),
    }
}

/// Degrees of freedom of a sketch, from the rank of the constraint Jacobian; this accounts for redundancy and
/// for radius variables. Returns (degrees of freedom, number of redundant constraint equations).
pub fn dof(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> (i32, i32) {
    // The rank of a Jacobian whose parts share no unknown is the sum of the ranks of the parts: a sketch is counted
    // part by part, and a part with no constraint is all freedom. 10 000 separate lines took 49 s counted whole.
    let parts = parts(points, radii, constraints);
    let mut free = (radii.len() - parts.iter().map(|p| p.radii.len()).sum::<usize>()) as i32; // radii of no point
    let mut redundant = 0;
    for part in &parts {
        if part.constraints.is_empty() {
            free += (part.points.len() * 2 + part.radii.len()) as i32;
            continue;
        }
        let own = part.own(points, radii, constraints);
        let (f, r) = dof_sparse(&own.points, &own.radii, &own.constraints);
        free += f;
        redundant += r;
    }
    (free, redundant)
}
