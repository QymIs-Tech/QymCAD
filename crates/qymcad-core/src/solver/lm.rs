//! THE LEAST SQUARES OF A SOLVE: the iteration, its pull, its steps dense and sparse, the residuals.

use super::*;

pub(crate) fn solve_full_iter_inner(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize, how: Solve) -> f64 {
    let scale = how.scale;
    // Two stages. First, a solve with a pull towards the previous state, which selects the solution closest to
    // how the sketch currently looks — without it the free degrees of freedom, such as the rotation of a
    // polygon, drift anywhere. Second, a polish without that pull, started from the solution just found: the
    // constraints are driven to machine precision, and there is nowhere left to travel in the null space
    // because the start already sits in the right place.
    let mut best = solve_lm(points, radii, constraints, drag, max_iter, Pull::SETTLE, how);
    // The polish runs only if the system is solvable. For a contradictory sketch the solution is a
    // least-squares compromise, and there is a whole set of such compromises: the polish would wander across
    // it, and re-solving would shift the geometry — the property test for a sketch drifting between solves
    // caught a 27 mm move. When the constraints are satisfiable, driving them to machine precision is exactly
    // what is wanted.
    //
    // The threshold scales with the sketch, it is not absolute: on a 500 mm part the residual left by the first
    // stage exceeds any fixed threshold on its own, the polish never started, and a dimension landed with a
    // 2e-4 error. Same lesson as the thresholds for dimension conflicts.
    let span = scale.span;
    // The arms of angle dimensions are held at their old lengths only to make an arm turn rather than stretch;
    // where the other constraints force an arm to a new length, that hold fights them and the first stage stops
    // at a compromise. A chamfer by a leg and an angle, its angle changed from 30 to 45 deg, must lengthen its
    // cut from 5.77 to 7.07 and stopped at a residual of 0.057. Solved again from there without the hold, and
    // taken only if that solves the sketch, so a contradictory one keeps the compromise it had. Measured over the probes
    // of the core: 240 solves of 48 063 ran it, all on parts of at most 36 unknowns, 0.27 s in all, and 2 were solved
    // by it. It repeats the part it is called for, not the sketch (`solve_by_parts`).
    //
    // Only a part with an angle dimension has arms to hold: in one without, the run without the hold is the first run
    // again, step for step, and its answer is thrown away as the first one's is. A contradiction in 1 000 tied rectangles
    // took 0.42 s of its 0.52 s solve in it, run for nothing.
    //
    // A radius held where it stood (`Sizes::Held`) fights a dimension that sets a new one the same way, and is let go the
    // same way: a sketch fillet given a new radius of 6 stopped at a residual of 6.9e-3 under the hold, and the polish
    // never ran.
    let holds_arms = constraints.iter().any(|c| matches!(c, Constraint::Angle { .. } | Constraint::AngleLines { .. })) || (how.sizes == Sizes::Held && !radii.is_empty());
    if drag.is_none() && best >= 1e-4 * span && holds_arms {
        let mut trial: Vec<SketchPoint> = points.to_vec();
        let mut trial_radii: Vec<RadiusVar> = radii.to_vec();
        let r = solve_lm(&mut trial, &mut trial_radii, constraints, drag, max_iter, Pull { hold_arms: false, ..Pull::SETTLE }, how);
        if r < 1e-4 * span {
            best = r;
            points.copy_from_slice(&trial);
            radii.copy_from_slice(&trial_radii);
        }
    }
    if best < 1e-4 * span {
        best = solve_lm(points, radii, constraints, drag, 40, Pull::POLISH, how).min(best);
    }
    const SOLVED: f64 = 1e-4; // below this the system counts as solved and there is nothing left to try
    if drag.is_some() || best <= SOLVED {
        return best;
    }
    for (a, b, axis) in violated_axis_dims(points, constraints, scale.extent, 6) {
        let (Some(ia), Some(ib)) = (points.iter().position(|p| p.id == a), points.iter().position(|p| p.id == b)) else { continue };
        let mut trial: Vec<SketchPoint> = points.to_vec();
        let mut trial_radii: Vec<RadiusVar> = radii.to_vec();
        // mirror point `b` about `a` along the axis of the dimension: start from the other side of the barrier
        if axis == 1 {
            trial[ib].x = 2.0 * trial[ia].x - trial[ib].x;
        } else {
            trial[ib].y = 2.0 * trial[ia].y - trial[ib].y;
        }
        let r = solve_lm(&mut trial, &mut trial_radii, constraints, drag, max_iter, Pull::SETTLE, how);
        if r < best - 1e-9 {
            best = r;
            points.copy_from_slice(&trial);
            radii.copy_from_slice(&trial_radii);
            if best <= SOLVED {
                break;
            }
        }
    }
    best
}

/// The pulls of one Levenberg-Marquardt run towards the state it started from: `reg` towards the old coordinates,
/// `hold_arms` - the arms of angle dimensions kept at their old lengths - and the starting damping `lambda0`.
#[derive(Clone, Copy)]
pub(crate) struct Pull {
    pub(crate) reg: f64,
    pub(crate) lambda0: f64,
    pub(crate) hold_arms: bool,
}

impl Pull {
    /// The first stage: the solution nearest to how the sketch looks now.
    pub(crate) const SETTLE: Pull = Pull { reg: 1e-3, lambda0: 1e-3, hold_arms: true };
    /// The polish of a solved sketch to machine precision. It starts on a solution, so there is no turn left to
    /// choose, and an arm held at a length 1e-6 off its solved one kept a residual of 1.9e-6.
    pub(crate) const POLISH: Pull = Pull { reg: 1e-9, lambda0: 0.0, hold_arms: false };
}

/// Axis (horizontal or vertical) dimensions that the current geometry does not satisfy — the candidates for a
/// side flip. The tolerance scales with the sketch, on the same principle as `sketch_conflicts`: the solver
/// repairs exactly what is shown as red, and behaves the same on a 2 mm part and on a 3 m frame. At most
/// `limit` of them.
pub(crate) fn violated_axis_dims(points: &[SketchPoint], constraints: &[Constraint], extent: f64, limit: usize) -> Vec<(Id, Id, u8)> {
    let pos: HashMap<Id, (f64, f64)> = points.iter().map(|p| (p.id, (p.x, p.y))).collect();
    // Tolerance scaled by the extent of the sketch (`Scale::extent`), the same principle as in `sketch_conflicts`: a
    // fixed 0.05 mm kept the multi-start from firing on a metre-sized frame, and on a tiny part it would have
    // declared almost everything violated.
    let tol = (extent * 1e-4).max(1e-6);
    let mut out = Vec::new();
    for c in constraints {
        if let Constraint::Distance { a, b, d, axis, driven, .. } = c {
            if *driven || (*axis != 1 && *axis != 2) {
                continue;
            }
            let (Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(a), pos.get(b)) else { continue };
            let m = if *axis == 1 { (ax - bx).abs() } else { (ay - by).abs() };
            if (m - *d).abs() > tol {
                out.push((*a, *b, *axis));
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// A single Levenberg-Marquardt run, without the multi-start side flip.
// THE INDEX IS THE MEANING: `a[i][i]` is the DIAGONAL of the normal matrix. An iterator over rows would
// still have to index the column, and the damping would stop being visibly a diagonal one.
#[allow(clippy::needless_range_loop)]
pub(crate) fn solve_lm(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize, pull: Pull, how: Solve) -> f64 {
    let Solve { algebra, until, sizes, .. } = how;
    let Pull { reg: w_reg, lambda0, hold_arms } = pull;
    if points.is_empty() {
        return 0.0;
    }
    let np = points.len();
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    // radius unknowns go into x after the point coordinates: x[np * 2 + j]
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(j, rv)| (rv.center, np * 2 + j)).collect();

    let has = |id: Id| idx.contains_key(&id);
    let is_center = |id: Id| ridx.contains_key(&id);
    let cons: Vec<Constraint> = constraints.iter().filter(|&c| cons_ok(c, &has, &is_center)).cloned().collect();
    let drag = drag.and_then(|(id, x, y)| idx.get(&id).map(|&i| (i, x, y)));
    if cons.is_empty() && drag.is_none() {
        return 0.0;
    }

    let anchor: HashMap<Id, (f64, f64)> = cons
        .iter()
        .filter_map(|c| match *c {
            Constraint::Fixed { p } => idx.get(&p).map(|&i| (p, (points[i].x, points[i].y))),
            _ => None,
        })
        .collect();

    let nv = np * 2 + radii.len();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|rv| rv.value));

    let x0 = x.clone();
    // `w_reg` is the weight of the pull towards the previous coordinates and is supplied from the outside, see
    // `solve_full_iter`.
    //
    // It exists as a tie-breaker: out of the set of solutions, pick the one closest to how the sketch looks
    // now. Without it the free degrees of freedom drift — a polygon rotates about its centre — and for a
    // contradictory system the compromise is not unique at all, so the solution moves between calls.
    //
    // It must not, however, compete with the constraints. At weight 1e-3 the solution settled into an
    // equilibrium between a dimension and the pull towards the old position, with an error of roughly
    // 1e-6 · displacement — enough to yield −129.99900 instead of −130. Hence the two stages: weight 1e-3
    // first, which steers towards the nearest solution, then a polish at 1e-9, which keeps the tie-breaker
    // while pushing the distortion of dimensions below machine precision.
    //
    // The mouse weight sits below the weight of the constraints, not above it. At 5.0 the cursor outranked any
    // dimension fivefold, and a fully constrained 40×25 rectangle stretched some fifty millimetres after the
    // mouse before snapping back on release. The dimensions themselves were fine; what lied was the display,
    // and the reading was that the dimensions do not hold. Fully constrained geometry must not move at all
    // under a drag — the reasoning is written up at `solve_sketch_drag`.
    //
    // Why a small weight suffices instead of projecting onto the null space: along the free directions the
    // constraints have no gradient, so nothing competes with the mouse and the point reaches the cursor at any
    // weight. Along the constrained directions the equilibrium distorts a constraint by w²/(1+w²) of the
    // displacement — 96 % at weight 5.0, which is what was observed, and 0.04 % at 0.02, below line thickness
    // for any reasonable drag.
    //
    // The weight is bounded from below twice: by the `w_reg` tie-breaker (1e-3), which the mouse has to
    // outweigh or the point sticks to its old position, and by the conditioning of the system, see the
    // measurements below.
    //
    // The value comes from a measured curve rather than from picking something small. At 0.02 the quality is
    // excellent — a constrained sketch does not budge — but the performance guard showed the drag frame on a
    // large sketch rising to 191 ms from 97: too small a weight conditions the system poorly and LM burns the
    // whole iteration budget. Point by point:
    //
    //     weight   drag frame   drift of a constrained sketch
    //     5.0        101 ms       51.9 mm   <- mouse stronger than the constraints
    //     1.0         81 ms       33.5 mm
    //     0.3         35 ms        7.7 mm
    //     0.1         94 ms        1.0 mm
    //     0.05        55 ms      < 0.5 mm   <- chosen
    //     0.02       191 ms      < 0.5 mm
    //
    // 0.05 takes both: the constraints hold, with drift below line thickness, and the frame costs half of what
    // it used to.
    let w_drag = 0.05_f64;
    // Preserve the arm lengths of angle constraints. The angular residual is length-independent — it only
    // rotates — but a free arm keeps its radial freedom, so the solver projected the endpoint onto the nearest
    // point of the ray and destroyed the length. Holding the arm lengths softly at their pre-solve values, taken
    // from `x0`, makes a rotation about the vertex satisfy both the angle and the length at once (both residuals
    // reach zero), so the arm rotates instead of stretching. The weight is above the positional regularisation,
    // which is what selects rotation, and far below `Distance` constraints, so explicit lengths still win.
    let w_len = 1e-1_f64;
    // THE SIZE OF A CIRCLE IS HELD SOFTLY, as the arms of an angle are: the pull towards the old value is 30 times the
    // positional one for a radius. With one weight for all, a circle tangent to two sides that moved 1 mm towards it each
    // took the cheapest step - the radius 2/3 mm smaller, the centre 1/3 mm over - and the sides moved 10 mm in 10 steps
    // left a circle of 5.5 at 2, then past the side - measured on a circle tied by tangency to two sides of a projected
    // outline, the body made 10 mm smaller: it came out outside the outline. Held, it keeps its 5.5 and moves with the
    // sides. The circle the mouse drags - by its centre or by a point on it - takes the plain weight: a rim dragged must
    // still resize it. A frame of a drag holds nothing (`Sizes::Free`): the part the drag keeps and the frame solved
    // alone differed in the last bits under the hold, the alone frame settling the parts the mouse is not on.
    let dragged = drag.map(|(i, _, _)| points[i].id);
    let held_radius: Vec<bool> = radii
        .iter()
        .map(|rv| hold_arms && sizes == Sizes::Held && dragged.is_none_or(|d| rv.center != d && !cons.iter().any(|c| matches!(*c, Constraint::PointOnCircle { p, c } if p == d && c == rv.center))))
        .collect();
    let reg_of = |k: usize| if k >= np * 2 && held_radius[k - np * 2] { 30.0 * w_reg } else { w_reg };
    let angle_arms: Vec<(usize, usize, f64)> = if !hold_arms {
        Vec::new()
    } else {
        // arms whose length is already set by a `Distance` dimension are left alone: the dimension holds them
        // and there is nothing to interfere with
        let dimensioned: std::collections::HashSet<(Id, Id)> = cons
            .iter()
            .filter_map(|c| match *c {
                Constraint::Distance { a, b, .. } => Some(if a < b { (a, b) } else { (b, a) }),
                _ => None,
            })
            .collect();
        let arm = |p: Id, q: Id| -> Option<(usize, usize, f64)> {
            if dimensioned.contains(&if p < q { (p, q) } else { (q, p) }) {
                return None;
            }
            let (ip, iq) = (*idx.get(&p)?, *idx.get(&q)?);
            let d = ((x0[2 * ip] - x0[2 * iq]).powi(2) + (x0[2 * ip + 1] - x0[2 * iq + 1]).powi(2)).sqrt();
            Some((ip, iq, d))
        };
        cons.iter()
            .flat_map(|c| match *c {
                Constraint::Angle { a, b, c: cc, .. } => vec![arm(b, a), arm(b, cc)],
                Constraint::AngleLines { a, b, c: cc, d, .. } => vec![arm(a, b), arm(cc, d)],
                _ => Vec::new(),
            })
            .flatten()
            .collect()
    };
    let residuals = |x: &[f64]| {
        let mut r = residuals_of(x, &x0, &idx, &ridx, &anchor, &cons);
        for k in 0..nv {
            r.push(reg_of(k) * (x[k] - x0[k]));
        }
        for &(ip, iq, l0) in &angle_arms {
            let (dx, dy) = (x[2 * ip] - x[2 * iq], x[2 * ip + 1] - x[2 * iq + 1]);
            r.push(w_len * ((dx * dx + dy * dy).sqrt() - l0));
        }
        if let Some((i, tx, ty)) = drag {
            r.push(w_drag * (x[2 * i] - tx));
            r.push(w_drag * (x[2 * i + 1] - ty));
        }
        r
    };

    // row count of every constraint, used to lay the Jacobian out in the shared matrix
    let con_nrows: Vec<usize> = cons
        .iter()
        .map(|c| {
            let mut t = Vec::new();
            con_rows(c, &x, &x0, &idx, &ridx, &anchor, &mut t);
            t.len()
        })
        .collect();
    // variables of anchored points are not solved for, see the assembly of the system below
    let mut fixed_var = vec![false; nv];
    for c in cons.iter() {
        if let Constraint::Fixed { p } = *c {
            if let Some(&i) = idx.get(&p) {
                fixed_var[2 * i] = true;
                fixed_var[2 * i + 1] = true;
            }
        }
    }
    // ...but the dragged point is always solved for: the gesture outranks the anchor while the mouse holds it
    if let Some((i, _, _)) = drag {
        fixed_var[2 * i] = false; // by this point `drag.0` is the index of the point, not its id: see the call site
        fixed_var[2 * i + 1] = false;
    }
    let mut lambda = lambda0;
    for _ in 0..max_iter.max(1) {
        if until.is_some_and(|t| std::time::Instant::now() >= t) {
            break; // the time of the solve is out: the best reached so far is the answer
        }
        let r = residuals(&x);
        let m = r.len();
        let err: f64 = r.iter().map(|v| v * v).sum();
        if m == 0 || err < 1e-24 {
            break;
        }
        // Sparse Jacobian. The earlier scheme recomputed the whole residual vector `nv` times, O(nv·m), and
        // built a dense JᵀJ, O(nv²·m), on every iteration, which made dragging a large sketch lag. Now a
        // constraint is differentiated only with respect to its own variables and only its own rows are
        // recomputed, O(m·8); the regularisation, arm and drag rows are analytic; and JᵀJ is accumulated from
        // sparse outer products.
        //
        // The Jacobian is analytic: the derivative of every constraint is written out exactly in `con_jac`,
        // with no finite differences. The numerical scheme was limited by its step — at coordinates of
        // hundreds of millimetres the solution stalled with an error around 1e-5, so a 130 mm dimension came
        // out as −129.99900 and a cut left a film across a wall instead of an opening. The formulas are kept
        // honest by a test that compares each one against a numerical derivative for every constraint type
        // (`solver_jacobian.rs`); an error in a formula would otherwise be silent.
        let mut jrows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m];
        {
            let mut base = 0usize;
            let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
            for (ci, c) in cons.iter().enumerate() {
                let n = con_nrows[ci];
                if n > 0 {
                    rows.clear();
                    con_jac(c, &x, &x0, &idx, &ridx, &mut rows);
                    debug_assert_eq!(rows.len(), n, "constraint Jacobian has a different row count than its residuals");
                    for (t, row) in rows.iter().enumerate().take(n) {
                        jrows[base + t] = row.clone();
                    }
                }
                base += n;
            }
            for k in 0..nv {
                jrows[base + k].push((k, reg_of(k))); // regularisation: ∂/∂xk = its weight
            }
            base += nv;
            for &(ip, iq, _) in &angle_arms {
                let (dx, dy) = (x[2 * ip] - x[2 * iq], x[2 * ip + 1] - x[2 * iq + 1]);
                let l = (dx * dx + dy * dy).sqrt().max(1e-12);
                jrows[base].push((2 * ip, w_len * dx / l));
                jrows[base].push((2 * ip + 1, w_len * dy / l));
                jrows[base].push((2 * iq, -w_len * dx / l));
                jrows[base].push((2 * iq + 1, -w_len * dy / l));
                base += 1;
            }
            if let Some((di, _, _)) = drag {
                jrows[base].push((2 * di, w_drag));
                jrows[base + 1].push((2 * di + 1, w_drag));
            }
        }
        let mut grad = vec![0.0; nv];
        for (rowi, row) in jrows.iter().enumerate() {
            for &(i, vi) in row {
                grad[i] += vi * r[rowi];
            }
        }
        let anchored = fixed_var.iter().filter(|f| **f).count();
        let dense = match algebra {
            Algebra::Dense => true,
            Algebra::Sparse => false,
            Algebra::BySize => nv - anchored <= DENSE_UP_TO,
        };
        let delta = match if dense { dense_step(&jrows, &grad, &fixed_var, lambda) } else { sparse_step(&jrows, &grad, &fixed_var, lambda) } {
            Some(d) => d,
            // The normal equations are singular — a rank deficiency of a degenerate configuration: coincident
            // points, zero lengths, collinearity. Rather than give up, raise the damping, which grows the
            // diagonal through `a[i][i] += lambda` until the system becomes solvable, and retry on the next
            // iteration. A `break` here left the sketch under-solved on any momentary degeneracy, which showed
            // up as jerks.
            None => {
                lambda = (lambda * 4.0).clamp(1e-12, 1e6); // the lower bound also lifts lambda back out of zero
                continue;
            }
        };
        let mut trial = x.clone();
        for i in 0..nv {
            trial[i] += delta[i];
        }
        let new_err: f64 = residuals(&trial).iter().map(|v| v * v).sum();
        // The stopping criterion is relative. A fixed threshold of 1e-9 on the step meant "no further to
        // go" regardless of the size of the sketch: on a part of hundreds of millimetres that is still
        // coarse, leaving the solution 2e-9 away from the dimension, while on a tiny part it would have
        // prevented stopping in time.
        let step = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
        if new_err < err {
            x = trial;
            lambda *= 0.5;
            let scale = x.iter().map(|v| v * v).sum::<f64>().sqrt().max(1.0);
            if step < 1e-14 * scale {
                break;
            }
        } else {
            // A STEP REFUSED BELOW THE THRESHOLD ENDS THE RUN as an accepted one does: x, J and r stay as they are,
            // the damping only grows, and a Levenberg-Marquardt step shrinks as the damping grows - whatever step is
            // taken next is shorter still and would end the run on its own. Running on, a circle with its diameter
            // to change took all 120 steps of the first stage (5 with this), an arc 120 (23): 80 us a part, 5.6 s on
            // 70 000 circles.
            let scale = x.iter().map(|v| v * v).sum::<f64>().sqrt().max(1.0);
            if step < 1e-14 * scale {
                break;
            }
            lambda = (lambda * 4.0).clamp(1e-12, 1e6);
        }
    }

    for (i, p) in points.iter_mut().enumerate() {
        p.x = x[2 * i];
        p.y = x[2 * i + 1];
    }
    for (j, rv) in radii.iter_mut().enumerate() {
        rv.value = x[np * 2 + j].max(0.001); // a radius stays positive
    }
    residuals_of(&x, &x0, &idx, &ridx, &anchor, &cons).iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// One step of Levenberg-Marquardt on the dense normal matrix: `(JᵀJ + lambda·I) d = -Jᵀr` over the free unknowns,
/// `None` where the matrix is singular.
// THE INDEX IS THE MEANING: `a[i][i]` is the DIAGONAL of the normal matrix, see `solve_lm`.
#[allow(clippy::needless_range_loop)]
pub(crate) fn dense_step(jrows: &[Vec<(usize, f64)>], grad: &[f64], fixed_var: &[bool], lambda: f64) -> Option<Vec<f64>> {
    let nv = grad.len();
    let mut a = vec![vec![0.0; nv]; nv];
    for row in jrows {
        for &(i, vi) in row {
            for &(j, vj) in row {
                a[i][j] += vi * vj;
            }
        }
    }
    // Levenberg damping, plain λ·I. It is needed at the start so that the step does not fly off on
    // degenerate configurations; the accuracy of the final answer comes from the polish stage at λ = 0,
    // which is pure Gauss-Newton with quadratic convergence.
    //
    // Scaling the diagonal instead (the Marquardt form) was tried and dropped: along degenerate directions
    // the damping vanished, the solution no longer reached the minimum within the iteration budget, and the
    // next call carried on descending, so the sketch drifted between solves — the property test caught it.
    // Once the real causes of the accuracy loss were fixed (analytic Jacobian, hard anchoring, splitting the
    // pull towards the previous state into two stages), the scaled form bought nothing anyway.
    for i in 0..nv {
        a[i][i] += lambda;
    }
    // An anchored point is not a variable. `Fixed` used to be a penalty row of weight 50, so the anchor
    // held only softly: the sketch axis drifted by about 1e-9 during a solve and tilted slightly, and
    // everything measured from it inherited the drift — a 130 mm dimension produced the coordinate
    // −129.9999997 while the constraint itself was formally satisfied, the point being correct relative to
    // the axis that had moved. Anchored geometry is therefore excluded from the unknowns: the system is
    // solved over the free variables only, and anchored ones stay exactly where they were anchored.
    let free: Vec<usize> = (0..nv).filter(|k| !fixed_var[*k] && a[*k][*k] > 1e-12).collect();
    let ared: Vec<Vec<f64>> = free.iter().map(|&i| free.iter().map(|&j| a[i][j]).collect()).collect();
    let rhs: Vec<f64> = free.iter().map(|&i| -grad[i]).collect();
    let d = solve_linear(ared, rhs)?;
    let mut full = vec![0.0; nv];
    for (t, &k) in free.iter().enumerate() {
        full[k] = d[t];
    }
    Some(full)
}

/// The same step on the sparse normal matrix, by its Cholesky factor (`faer`, ordered to keep the factor sparse): the
/// same sums, the same damping and the same free unknowns as `dense_step`, at the cost of the non-zeros instead of
/// the cube of the unknowns. `None` where the matrix is not positive definite - singular, as the dense step says.
pub(crate) fn sparse_step(jrows: &[Vec<(usize, f64)>], grad: &[f64], fixed_var: &[bool], lambda: f64) -> Option<Vec<f64>> {
    use faer::linalg::solvers::Solve;
    use faer::sparse::{SparseColMat, Triplet};
    let nv = grad.len();
    // the lower triangle of JᵀJ + lambda·I, every product as the dense matrix sums it, then summed by place
    let mut parts: Vec<Triplet<usize, usize, f64>> = (0..nv).map(|k| Triplet::new(k, k, lambda)).collect();
    for row in jrows {
        for &(i, vi) in row {
            for &(j, vj) in row {
                if i >= j {
                    parts.push(Triplet::new(i, j, vi * vj));
                }
            }
        }
    }
    parts.sort_unstable_by_key(|e| (e.col, e.row));
    let mut sums: Vec<Triplet<usize, usize, f64>> = Vec::with_capacity(parts.len());
    for e in parts {
        match sums.last_mut() {
            Some(last) if last.row == e.row && last.col == e.col => last.val += e.val,
            _ => sums.push(e),
        }
    }
    let mut diagonal = vec![0.0; nv];
    for e in sums.iter().filter(|e| e.row == e.col) {
        diagonal[e.row] = e.val;
    }
    // the free unknowns as `dense_step` takes them, and the place of each among them
    let free: Vec<usize> = (0..nv).filter(|&k| !fixed_var[k] && diagonal[k] > 1e-12).collect();
    let mut place = vec![usize::MAX; nv];
    for (t, &k) in free.iter().enumerate() {
        place[k] = t;
    }
    let own: Vec<Triplet<usize, usize, f64>> = sums.iter().filter(|e| place[e.row] != usize::MAX && place[e.col] != usize::MAX).map(|e| Triplet::new(place[e.row], place[e.col], e.val)).collect();
    let n = free.len();
    let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &own).ok()?;
    let factor = matrix.sp_cholesky(faer::Side::Lower).ok()?;
    let d = factor.solve(faer::Mat::<f64>::from_fn(n, 1, |t, _| -grad[free[t]]));
    let mut full = vec![0.0; nv];
    for (t, &k) in free.iter().enumerate() {
        full[k] = d[(t, 0)];
    }
    full.iter().all(|v| v.is_finite()).then_some(full)
}

/// Direct solution of the linear system `a·x = b` by Gauss-Jordan elimination with partial pivoting.
// THE INDEX IS THE MEANING: Gaussian elimination subtracts row `col` from row `r` starting at column
// `col`. Rows and columns are what the method is about, and hiding them behind iterators hides the method.
#[allow(clippy::needless_range_loop)]
pub(crate) fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = a.len();
    for col in 0..n {
        let mut piv = col;
        for r in col + 1..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let d = a[col][col];
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[r][col] / d;
            if f != 0.0 {
                for c in col..n {
                    a[r][c] -= f * a[col][c];
                }
                b[r] -= f * b[col];
            }
        }
    }
    Some((0..n).map(|i| b[i] / a[i][i]).collect())
}

/// Residuals of every constraint at coordinates `x`; shared by the solve and by the degree-of-freedom analysis.
pub(crate) fn residuals_of(x: &[f64], x0: &[f64], idx: &HashMap<Id, usize>, ridx: &HashMap<Id, usize>, anchor: &HashMap<Id, (f64, f64)>, cons: &[Constraint]) -> Vec<f64> {
    let mut r = Vec::new();
    for c in cons {
        con_rows(c, x, x0, idx, ridx, anchor, &mut r);
    }
    r
}

/// Constraint diagnostics: the residual of each constraint separately, in the order of `constraints`.
///
/// A single residual for the whole sketch says only that something does not fit; finding out which constraints
/// disagree then means switching them off one at a time. Per-constraint residuals show it directly.
pub fn residual_per_constraint(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<f64> {
    let np = points.len();
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(j, rv)| (rv.center, np * 2 + j)).collect();
    let has = |id: Id| idx.contains_key(&id);
    let is_center = |id: Id| ridx.contains_key(&id);
    let anchor: HashMap<Id, (f64, f64)> = constraints
        .iter()
        .filter_map(|c| match *c {
            Constraint::Fixed { p } => idx.get(&p).map(|&i| (p, (points[i].x, points[i].y))),
            _ => None,
        })
        .collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|rv| rv.value));
    constraints
        .iter()
        .map(|c| {
            if !cons_ok(c, &has, &is_center) {
                return 0.0;
            }
            let mut r = Vec::new();
            con_rows(c, &x, &x, &idx, &ridx, &anchor, &mut r);
            r.iter().map(|v| v * v).sum::<f64>().sqrt()
        })
        .collect()
}
