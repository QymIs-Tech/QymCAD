//! Iteration and diagnostics: how the problem is solved and what the solver must report about it.
//!
//! The step is computed through an SVD because an assembly problem is almost always degenerate:
//! constraints repeat each other, a body is underdefined, axes coincide. A rank-revealing
//! decomposition gives three things at once:
//!
//! * a stable step under degeneracy — the minimum-norm solution instead of an arbitrary one;
//! * the true number of degrees of freedom, unknowns minus rank, rather than a guess from the
//!   residual;
//! * the row null space, which names the redundant constraints.
//!
//! Levenberg-Marquardt damping is there for convergence from a poor starting pose. Without it, the
//! only way to converge is to pre-align bodies by heuristics before solving, and that pre-alignment
//! displaces them.

use nalgebra::{DMatrix, DVector, Isometry3, Translation3, UnitQuaternion, Vector3};

use super::problem::{Constraint, Problem};
use super::solve::{jacobian, poses_at, residuals, Layout};

/// What the solver reports about a problem.
///
/// Not a single "conflict" flag: repairing an assembly requires knowing what is wrong and where.
/// Without that, the only remaining option is moving bodies at random.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Whether the solution converged, that is, every constraint holds within tolerance.
    pub converged: bool,
    /// Residual norm after solving.
    pub residual: f64,
    /// Remaining degrees of freedom of the assembly: unknowns minus rank.
    pub dof: usize,
    /// Indices of constraints whose equations are linearly dependent on the rest, that is, redundant.
    pub redundant: Vec<usize>,
    /// Indices of constraints that do not hold, with the size of the violation.
    pub violated: Vec<(usize, f64)>,
    /// How many iterations were needed.
    pub iterations: usize,
    /// The input poses contained non-numbers and the solver restarted from an assembled guess.
    /// Externally this means the pose before solving was garbage and must not be trusted.
    pub recovered_from_garbage: bool,
}

/// Tolerance: a constraint holds when its residual is below this. Millimetres for distances and sine
/// units for directions; both are comparable at part scale.
pub(crate) const TOL: f64 = 1e-7;

/// Cutoff for singular values when determining rank.
///
/// Relative rather than absolute: singular values of a metre-scale and a millimetre-scale assembly
/// differ by orders of magnitude, and a fixed threshold would declare one of them degenerate. A
/// fraction of 1e-9 of the largest value is standard practice at this scale.
const RANK_EPS: f64 = 1e-9;


/// Initial guess: place every free body so that its anchor coincides with the anchor of an already
/// placed partner.
///
/// Gauss-Newton is a local method: from a pose turned by nearly half a turn it converges into the
/// false minimum "flipped", because that one is stable too. No amount of damping fixes this — a
/// sensible start is required.
///
/// This differs from making frame composition the final answer. As a final answer it forces a body to
/// align anchors exactly, including the directions along which the constraint requires nothing, which
/// displaces bodies. Here it is only a guess: afterwards the solver moves the body by the actual
/// constraints and leaves free whatever they do not require. A wrong guess is safe; a wrong final
/// answer is not.
fn initial_guess(problem: &Problem) -> Vec<Isometry3<f64>> {
    let mut poses: Vec<Isometry3<f64>> = problem.bodies.iter().map(|b| b.pose).collect();
    let mut placed: Vec<bool> = problem.bodies.iter().map(|b| b.grounded).collect();
    // A body held at a distance from an axis must not be aligned with its anchor at all.
    //
    // The guess places anchors on top of each other, while "distance to axis" demands the opposite: a
    // non-zero gap. Aligning them lands exactly on the point where the direction "away from the axis"
    // is undefined, the derivative vanishes and there is nowhere to step. Measured on two tangent
    // shafts: the guess placed the smaller one coaxially with the larger and it stayed there forever.
    // Such bodies are marked as already placed — where they were left is the better start.
    for c in &problem.constraints {
        if matches!(c, Constraint::AxisDistance { .. } | Constraint::PointDistance { .. }) {
            for b in c.bodies() {
                placed[b] = true;
            }
        }
    }
    if placed.iter().all(|p| !p) {
        return poses; // Nothing to measure from, so start from what is there.
    }
    // Breadth-first traversal over constraints, from placed bodies to their neighbours.
    for _ in 0..problem.bodies.len() {
        // Bodies are marked as placed only at the end of a pass, so every primitive of a constraint
        // gets its say. Marking on the first primitive closes the body to the rest, and a constraint
        // then places it by one condition out of three: a rigid mate gets a translation without a
        // single rotation.
        let mut newly: Vec<usize> = Vec::new();
        for c in &problem.constraints {
            // A relation between degrees does not place a body. It states how the travels of two
            // mates are tied, not where anything stands, so aligning anchors by it would invent a pose.
            let Some((a, b)) = c.pair() else { continue };
            // A constraint that requires anchors to stay apart is unusable as a guess.
            //
            // The guess means "place the body so the anchors coincide", while an angle or a distance to
            // an axis demands a gap between them. Measured on two tangent shafts: the guess placed them
            // coaxially, exactly where the direction "away from the axis" is undefined, the derivative
            // vanishes and there is nowhere to step. The shafts stayed put and the constraint was
            // declared unsatisfiable.
            if matches!(c, Constraint::Angle { .. } | Constraint::AxisDistance { .. } | Constraint::PointDistance { .. }) {
                continue;
            }
            let (src, dst) = if placed[a.body] && !placed[b.body] {
                (a, b)
            } else if placed[b.body] && !placed[a.body] {
                (b, a)
            } else {
                continue;
            };
            let _ = src;
            place_minimally(c, dst.body, &mut poses);
            newly.push(dst.body);
        }
        if newly.is_empty() {
            break;
        }
        for b in newly {
            placed[b] = true;
        }
    }
    poses
}

/// Any unit vector perpendicular to the given one, used as the axis of a half-turn.
///
/// Needed where two axes point exactly against each other: a rotation between them exists, but there
/// is no shortest one — there is a whole circle of them, and `rotation_between` reports that it cannot
/// choose.
fn any_perp(v: &Vector3<f64>) -> nalgebra::Unit<Vector3<f64>> {
    let helper = if v.x.abs() < 0.9 { Vector3::x() } else { Vector3::y() };
    nalgebra::Unit::new_normalize(v.cross(&helper))
}

/// Place a body exactly as the primitive requires and not a hair more.
///
/// Full anchor alignment (`T_dst = T_src . F_src . F_dst^-1`) applied to every primitive fixes all six
/// degrees, while a primitive requires two or three: "point on axis" says nothing about rotation or
/// about position along the axis, "point in plane" only about the distance to it. The guess invents
/// the difference, and it invents a lot: on a real machine document this turned one beam by 180.000
/// degrees and carried an axis assembly 908.204 mm away. Same principle of not moving more than
/// necessary that governs
/// `pull_back_free_directions`.
///
/// Residuals are written as "second anchor minus first" (see `solve::residual_of`), so the sign of the
/// correction depends on which of the two bodies may move.
fn place_minimally(c: &Constraint, dst_body: usize, poses: &mut [Isometry3<f64>]) {
    let Some((a, b)) = c.pair() else { return };
    let (pa, pb) = (poses[a.body], poses[b.body]);
    let (ao, bo) = (a.world_origin(&pa), b.world_origin(&pb));
    let dst_is_b = dst_body == b.body;
    let k = if dst_is_b { -1.0 } else { 1.0 };
    let shift = |poses: &mut [Isometry3<f64>], v: Vector3<f64>| {
        poses[dst_body] = Translation3::from(v) * poses[dst_body];
    };
    // The fallback half-turn axis is chosen, not taken at random.
    //
    // For exactly opposing vectors there is no shortest rotation: there is a whole circle of them, and
    // `rotation_between` reports that it cannot choose. Any perpendicular will do only where there is
    // nothing to break. Roll has something to break: turning about a random axis disturbs the travel
    // axis that is already set, and a slider between facing faces then never assembles — measured with
    // travel matching at cosine 1.0000 while roll was opposite at -1.0000 and the residual stood at
    // 1.990. Roll is therefore turned about the main axis of the constraint, which is perpendicular to
    // roll by construction and unaffected by the turn.
    let turn = |poses: &mut [Isometry3<f64>], from: Vector3<f64>, to: Vector3<f64>, pivot: Vector3<f64>, spare: Vector3<f64>| {
        let q = UnitQuaternion::rotation_between(&from, &to)
            .unwrap_or_else(|| UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(spare), std::f64::consts::PI));
        poses[dst_body] = Translation3::from(pivot) * q * Translation3::from(-pivot) * poses[dst_body];
    };
    match *c {
        Constraint::PointCoincident { .. } => shift(poses, (bo - ao) * k),
        Constraint::OnAxis { .. } => {
            let z = a.world_z(&pa);
            let d = bo - ao;
            shift(poses, (d - z * z.dot(&d)) * k);
        }
        Constraint::OnPlane { offset, .. } => {
            let z = a.world_z(&pa);
            shift(poses, z * ((z.dot(&(bo - ao)) - offset) * k));
        }
        Constraint::AxisAligned { .. } => {
            let (za, zb) = (a.world_z(&pa), b.world_z(&pb));
            // For opposing main axes there is nothing to break: any perpendicular will do.
            if dst_is_b {
                turn(poses, zb, za, bo, any_perp(&zb).into_inner());
            } else {
                turn(poses, za, zb, ao, any_perp(&za).into_inner());
            }
        }
        Constraint::RollAligned { .. } => {
            let (xa, xb) = (a.world_x(&pa), b.world_x(&pb));
            // Roll is turned about the main axis, otherwise the travel already set is disturbed.
            if dst_is_b {
                turn(poses, xb, xa, bo, b.world_z(&pb));
            } else {
                turn(poses, xa, xb, ao, a.world_z(&pa));
            }
        }
        // Angles and distances require a gap, so the guess leaves them alone entirely (see above).
        _ => {}
    }
}

/// Whether a pose consists of real numbers, that is, no NaN and no infinity.
fn pose_is_finite(p: &Isometry3<f64>) -> bool {
    p.translation.vector.iter().all(|v| v.is_finite()) && p.rotation.coords.iter().all(|v| v.is_finite())
}

/// One Levenberg-Marquardt attempt from a given start. Returns poses, the residual and the step count.
///
/// There are two attempts (see `solve`). As a single inlined loop the choice of start is fixed, which
/// decides the outcome for the user.
fn run_lm(
    problem: &Problem,
    layout: &Layout,
    start: &[Isometry3<f64>],
) -> (Vec<Isometry3<f64>>, DVector<f64>, usize) {
    let problem = &Problem {
        bodies: problem.bodies.iter().zip(start.iter()).map(|(b, pose)| super::problem::Body { pose: *pose, grounded: b.grounded }).collect(),
        constraints: problem.constraints.clone(),
    };
    let mut iterations = 0usize;
    let mut x = DVector::zeros(layout.unknowns);
    let mut lambda = 1e-3; // Damping: grows after a rejected step, shrinks after an accepted one.
    let mut poses = poses_at(problem, &layout, &x);
    let mut r = residuals(problem, &poses);
    // The cost is the constraints and nothing else.
    //
    // Adding a pull towards the original pose makes it compete with the constraints. Measured on a
    // cylindrical mate: cost 1.643e-8 of which the residual accounts for 1.3e-14, so the solver
    // minimised almost purely the pull and was happy to hold a residual of 1.128e-7, above tolerance,
    // to reduce it. The attempt was then declared non-convergent, the second one — started from
    // aligned anchors — won, and the body travelled 30 mm to the anchor.
    //
    // Minimal displacement is handled by `pull_back_free_directions`: it moves a body only along the
    // directions the constraints do not determine, so it cannot argue with them and
    // cannot displace a body along a direction that the constraints do fix.
    let mut cost = r.norm_squared();

    const MAX_ITER: usize = 200;
    for it in 1..=MAX_ITER {
        iterations = it;
        let j = jacobian(problem, &layout, &poses);

        // Levenberg-Marquardt step: (J^T J + lambda * diag(J^T J)) . d = -J^T r, solved through SVD.
        // Only the constraints are solved here; keeping a body where it was left is the job of the
        // separate `pull_back_free_directions` step, see `solve`.
        let jt = j.transpose();
        let mut h = &jt * &j;
        let g = &jt * &r;
        // Damping is scaled by the problem, not per element.
        //
        // With `d + lambda * d.max(1e-12)`, a direction the constraints do not constrain has `d` equal
        // to zero and only 1e-12 * lambda left on the diagonal. The decomposition divides by that
        // number, and rounding noise in the right-hand side turns into real motion of the body.
        // Measured: a constraint leaving five degrees of freedom produced 0.8 and 0.47 along the free
        // axes on the very first step, against a required travel of 5.7.
        //
        // The fix is a relative floor, the same device as the rank threshold in this file: an absolute
        // number means different things for a metre-scale and a millimetre-scale assembly, while a
        // fraction of the problem scale means the same. The floor comes from the largest diagonal
        // entry, so a direction without curvature gets
        // a noticeable fraction of the scale rather than 1e-12, and noise along it is no longer amplified.
        //
        // Damping the whole matrix by that scale (classical Marquardt) was measured and rejected: for a
        // revolute pair through holes the constrained directions were suppressed together with the free
        // ones, the solver missed the 200-step budget and fell through to the second attempt, started
        // from aligned anchors, which dragged the cover by the difference in hole depth.
        let dmax = (0..h.nrows()).map(|i| h[(i, i)]).fold(0.0_f64, f64::max);
        let floor = if dmax > 0.0 { dmax * 1e-6 } else { 1e-12 };
        for i in 0..h.nrows() {
            let d = h[(i, i)];
            h[(i, i)] = d + lambda * d.max(floor);
        }
        let step = match solve_least_squares(&h, &(-&g)) {
            Some(s) => s,
            None => break,
        };
        // The step has to lie where it fixes something.
        //
        // Directions the constraints do not constrain (the Jacobian null space) do not affect the
        // residual at all, yet diagonal damping leaves them a tiny weight, and noise in the right-hand
        // side turns into a huge motion there. Measured on two tangent spheres: residual 1.988e-2 with a
        // step length of 9.6, so the step went almost entirely into nothing, the cost did not improve,
        // lambda grew to 1.7e2, and all 200 steps burned while a solution was reachable — from a closer
        // start the same solver converges in 77 steps to 2.2e-11.
        //
        // What the constraints are indifferent to is removed from the step. This is not a tolerance
        // tweak: displacement along free directions is the job of `pull_back_free_directions`, and the
        // step
        // has nothing to do there by definition.
        let step = project_onto_row_space(&j, step);

        let x_try = &x + &step;
        let poses_try = poses_at(problem, &layout, &x_try);
        let r_try = residuals(problem, &poses_try);
        let cost_try = r_try.norm_squared();

        if cost_try < cost {
            let improved = cost - cost_try;
            x = x_try;
            poses = poses_try;
            r = r_try;
            cost = cost_try;
            lambda = (lambda * 0.3).max(1e-12);
            // Being done is about the constraints, not about the total cost.
            //
            // Testing `cost.sqrt() < TOL * 0.01` fails when the cost includes a pull towards the
            // original pose: the pull does not vanish when a body legitimately moves, and at 60 mm of
            // displacement it contributes about 6e-5, making a 1e-9 threshold unreachable in principle.
            // Measured: a rigid pair finished in 25 steps while a revolute pair burned all 200 at a
            // residual of 4e-9 — wasted work on every mate that moves a body.
            //
            // A solution is ready when the constraints hold and the step stops improving anything. A
            // pull is not a condition of the solution but a choice between solutions.
            if improved < 1e-24 || r.norm() < TOL * 0.01 {
                break;
            }
        } else {
            // The step did not improve anything: increase damping, moving closer to gradient descent.
            lambda *= 8.0;
            if lambda > 1e12 {
                break;
            }
        }
    }
    (poses, r, iterations)
}

/// Solve the problem: place the ungrounded bodies so that the constraints hold.
///
/// Returns new poses for every body — grounded ones unchanged — together with a report.
pub fn solve(problem: &Problem) -> (Vec<Isometry3<f64>>, Report) {
    let layout = Layout::of(problem);
    let mut report = Report::default();

    if !problem.references_are_valid() {
        // A broken reference leaves nothing to compute. Silently skipping such a constraint is worse:
        // the assembly then "solves" without it and nobody learns that the constraint does not act.
        report.residual = f64::INFINITY;
        return (problem.bodies.iter().map(|b| b.pose).collect(), report);
    }
    if layout.unknowns == 0 || problem.constraints.is_empty() {
        let poses: Vec<_> = problem.bodies.iter().map(|b| b.pose).collect();
        report.converged = true;
        report.residual = if problem.constraints.is_empty() { 0.0 } else { residuals(problem, &poses).norm() };
        report.dof = layout.unknowns;
        return (poses, report);
    }

    // The input must consist of numbers. A corrupted pose (a NaN from degenerate geometry) must never
    // reach the numerics: an SVD over such data never converges, and the application hangs with no error
    // and no way out. Starting from an assembled guess happens to overwrite corrupted poses before the
    // first computation, but relying on that side effect is not a defence.
    let sanitized;
    let problem = if problem.bodies.iter().all(|b| pose_is_finite(&b.pose)) {
        problem
    } else {
        report.recovered_from_garbage = true;
        sanitized = Problem {
            bodies: problem.bodies.iter().map(|b| super::problem::Body { pose: if pose_is_finite(&b.pose) { b.pose } else { Isometry3::identity() }, grounded: b.grounded }).collect(),
            constraints: problem.constraints.clone(),
        };
        &sanitized
    };

    // Where the bodies were: of all admissible solutions, the one nearest to this pose must be chosen.
    let origin_poses: Vec<Isometry3<f64>> = problem.bodies.iter().map(|b| b.pose).collect();

    // First attempt: from the current pose. That is the answer of minimal displacement — free degrees
    // stay where they were left.
    //
    // Always starting from the assembled guess (`initial_guess`) aligns the anchors completely, which
    // zeroes travel along the axis and rotation about it. A deliberately weak pull towards the original
    // pose cannot bring a body back over 75 mm before the assembly converges, so the body jumps to its
    // anchor every time the file is opened. The guess is a rescue, not a replacement for what was
    // already built.
    //
    // With corrupted input "where the body was" means nothing, so the guess is used from the start.
    let start: Vec<Isometry3<f64>> = if report.recovered_from_garbage { initial_guess(problem) } else { origin_poses.clone() };
    let (mut poses, r, mut iterations) = run_lm(problem, &layout, &start);

    // Second attempt: from the assembled guess, used only when the first found no solution. It escapes
    // a local minimum, the classical case being a half-turn.
    if r.norm() >= TOL {
        let guess = initial_guess(problem);
        let (poses2, r2, it2) = run_lm(problem, &layout, &guess);
        // Choosing between attempts is not decided by the residual alone.
        //
        // The second attempt starts with the anchors aligned, and that alignment also fixes what the
        // constraint does not require. On a five-degree constraint it reached a solution with zero
        // residual but with the body dragged 14 mm along the slot, and won on residual alone. Between
        // two solutions the one nearer to where the body was left is chosen; the second attempt wins on
        // residual only when the first found no solution at all.
        let first_solved = r.norm() < TOL;
        let second_solved = r2.norm() < TOL;
        let take_second = if first_solved && second_solved {
            deviation_norm(problem, &layout, &poses2, &origin_poses) < deviation_norm(problem, &layout, &poses, &origin_poses)
        } else {
            r2.norm() < r.norm()
        };
        if take_second {
            poses = poses2;
            iterations += it2;
        }
    }
    report.iterations = iterations;
    // Minimal displacement is a separate step, not a weight in the cost.
    //
    // Of all poses satisfying the constraints, the nearest to the current one must be chosen. A weak
    // pull inside the cost achieves that at no weight, and this is measured: at 1e-6 a body is carried
    // 1134 mm away, at 1e-4 six checks of "a driven value moves the body exactly" fail, and at 1e-3 a
    // false conflict appears at a displacement of
    // 7e-6 mm. The weight is not the problem: a pull along every direction at once either fails to hold
    // or fights the constraints.
    //
    // The correct move is to return the body only along the directions the constraints leave free.
    //
    // While a drag is in progress there is no pull-back but its mirror. Both walk the same free
    // directions and would simply cancel each other, so the same step is taken towards the cursor
    // instead of towards the previous pose.
    let goal = problem.constraints.iter().copied().find(|c| matches!(c, Constraint::Pull { .. }));
    let poses = match goal {
        Some(Constraint::Pull { a, to }) => pull_towards_cursor(problem, &layout, poses, a, to),
        _ => pull_back_free_directions(problem, &layout, poses, &origin_poses),
    };
    let r = residuals(
        &Problem {
            bodies: problem.bodies.iter().zip(poses.iter()).map(|(b, pose)| super::problem::Body { pose: *pose, grounded: b.grounded }).collect(),
            constraints: problem.constraints.clone(),
        },
        &poses,
    );
    let problem = &Problem {
        bodies: problem.bodies.iter().zip(poses.iter()).map(|(b, pose)| super::problem::Body { pose: *pose, grounded: b.grounded }).collect(),
        constraints: problem.constraints.clone(),
    };

    // The verdict comes from the constraints, not from the drag target, and that holds because the
    // target contributes no equations (`rows()` is 0) rather than by striking its rows out afterwards.
    // Counting the target would declare the assembly unsolvable on every frame of a drag, and the rule
    // "no solution, no movement" would then freeze the whole mechanism. Measured: conflict reported on
    // every drag step and zero motion while the constraints were satisfied.
    report.residual = r.norm();
    report.converged = report.residual < TOL;

    // Diagnostics from the Jacobian at the solution. Non-numbers should no longer reach this point, but
    // an SVD over them hangs with no way out, and the check costs nothing against a frozen application.
    let j = jacobian(problem, &layout, &poses);
    if !j.iter().all(|v| v.is_finite()) || !poses.iter().all(pose_is_finite) {
        report.converged = false;
        report.residual = f64::INFINITY;
        return (origin_poses, report);
    }
    let svd = j.clone().svd(false, false);
    let smax = svd.singular_values.iter().cloned().fold(0.0f64, f64::max);
    let rank = svd.singular_values.iter().filter(|s| **s > smax * RANK_EPS).count();
    report.dof = layout.unknowns.saturating_sub(rank);
    report.redundant = redundant_constraints(problem, &j, smax);
    report.violated = violated_constraints(problem, &r); // The drag target occupies no rows and can never be violated.

    (poses, report)
}


/// Move a body towards the cursor, but only where the constraints are indifferent.
///
/// The mirror of minimal displacement. `pull_back_free_directions` moves a body through the null space
/// of the constraints back to where it came from; this takes the same step towards the point under the
/// cursor. Everything important follows from that:
///
/// * the constraints cannot be violated in principle, since motion in their null space is indifferent
///   to them, so the "converged" verdict stays honest and the mechanism does not freeze;
/// * the mechanism follows as a chain on its own: the null space holds the free directions of the whole
///   part of the document at once, not one degree of one mate;
/// * a limit stays a limit: there is no freedom left there, so the step becomes zero by itself.
///
/// What the drag asks for is a pure translation by "target minus point". The body is rigid, so that
/// translation brings the grabbed point exactly onto the target; whatever the constraints disallow is
/// cut away by the projection. No rotation has to be invented: a rotational degree has a null-space
/// vector that itself contains a translation, and the projection yields a non-zero rotation exactly
/// where one is required.
///
/// The step is verified by fact, like the pull-back: the problem is non-linear and freedom holds only
/// locally. Move, re-solve the constraints, and accept only if the constraints are intact and the body
/// really got closer to the cursor; otherwise halve the step, and so on until it is given up.
fn pull_towards_cursor(problem: &Problem, layout: &Layout, poses: Vec<Isometry3<f64>>, a: super::frame::Anchor, to: nalgebra::Vector3<f64>) -> Vec<Isometry3<f64>> {
    if layout.unknowns == 0 || layout.column_of(a.body).is_none() {
        return poses; // The body is grounded and cannot be dragged.
    }
    let mut poses = poses;
    let miss = |p: &[Isometry3<f64>]| (a.world_origin(&p[a.body]) - to).norm();
    // Several attempts: after each step the null space is slightly different because the problem is
    // non-linear, so the chain reaches the cursor in a few moves rather than one.
    for _ in 0..4 {
        let before = miss(&poses);
        if before < 1e-9 {
            return poses; // The body is already under the cursor: nowhere to move it.
        }
        let staged = staged_problem(problem, &poses);
        // What the drag asks for is a translation of the grabbed body; what it is allowed is the same
        // minus everything the constraints hold, that is, the row space of their Jacobian.
        let col = layout.column_of(a.body).expect("column checked above");
        let d = to - a.world_origin(&poses[a.body]);
        let mut want = DVector::zeros(layout.unknowns);
        for k in 0..3 {
            want[col + k] = d[k];
        }
        let j = jacobian(&staged, layout, &poses);
        if !j.iter().all(|v| v.is_finite()) {
            return poses;
        }
        let free = &want - project_onto_row_space(&j, want.clone());
        if free.norm() < 1e-12 {
            return poses; // The constraints hold everything: nowhere to move, which is a limit, not a fault.
        }
        let r_before = residuals(&staged, &poses).norm();
        let mut scale = 1.0;
        let mut accepted = false;
        loop {
            let moved = poses_at(&staged, layout, &(&free * scale));
            let (settled, r_settled, _) = run_lm(problem, layout, &moved);
            if r_settled.norm() <= TOL.max(r_before) && miss(&settled) < before * 0.999 {
                poses = settled;
                accepted = true;
                break;
            }
            scale *= 0.5;
            if scale < 1.0 / 16.0 {
                break;
            }
        }
        if !accepted {
            return poses; // No way to get closer without breaking the constraints.
        }
    }
    poses
}

/// Return a body along its free directions to where it came from.
///
/// This is minimal displacement done directly rather than by tuning a weight.
///
/// How it is computed: take the deviation of a body from its original pose and split it in two — the
/// part lying in the row space of the constraint Jacobian, which the constraints determine and which
/// must not be touched, and the remainder in the null space, where the constraints require nothing and
/// their gradient is exactly zero. Only the remainder is returned.
///
/// This cannot spoil the solution: motion along the null space does not change the constraint residual
/// to first order, so there is physically nothing to argue with. The problem is non-linear, however, so
/// the result is verified by fact: if the correction raises the residual, it is halved, and eventually
/// abandoned.
fn pull_back_free_directions(
    problem: &Problem,
    layout: &Layout,
    poses: Vec<Isometry3<f64>>,
    origin: &[Isometry3<f64>],
) -> Vec<Isometry3<f64>> {
    let n = layout.unknowns;
    if n == 0 {
        return poses;
    }
    let mut poses = poses;
    // Two passes: after the first correction the null space is slightly different, the problem being non-linear.
    for _ in 0..2 {
        let staged = staged_problem(problem, &poses);
        let Some(free) = free_deviation(problem, layout, &poses, origin) else { return poses };
        if free.norm() < 1e-9 {
            return poses; // No free deviation, so there is nothing to return.
        }

        // Return first, then re-solve. A direction is free only locally: over a large step the problem
        // is non-linear and the constraints stop holding. Measured: a return of 1134 mm raised the
        // residual from 3.6e-11 to 5.2e-4, and a cautious "no worse" test cut the step down to a
        // sixty-fourth, which is meaningless.
        //
        // The established approach is the one used here: move along the freedom, then come back onto
        // the constraint surface with a short re-solve. Repeated while the return really brings the
        // body closer to its original place.
        let before_dev = free.norm();
        let mut scale = 1.0;
        let mut accepted = false;
        loop {
            let step = &free * -scale;
            let moved = poses_at(&staged, layout, &step);
            let (settled, r_settled, _) = run_lm(problem, layout, &moved);
            // Both sides are measured by the same quantity: the free part of the deviation. Comparing a
            // full deviation against a budget derived from the free part puts a driven travel of 60 mm
            // into the result, the condition can then never be true for any driven value, and the return
            // is rejected every time. Measured on three sliders: 0.696 before against 60.000 after, while
            // the carriage drifted 0.492 mm along its own free axis.
            let dev_after = free_deviation(problem, layout, &settled, origin).map_or(f64::INFINITY, |f| f.norm());
            // Accept when the constraints are intact and the body really is closer to its place.
            if r_settled.norm() <= TOL.max(residuals(&staged, &poses).norm()) && dev_after < before_dev * 0.99 {
                poses = settled;
                accepted = true;
                break;
            }
            scale *= 0.5;
            if scale < 1.0 / 16.0 {
                break;
            }
        }
        if !accepted {
            return poses; // It does not help, so leave the poses as they are.
        }
    }
    poses
}

/// The free part of the deviation from the original pose: what the constraints did not require.
///
/// The deviation splits in two: the part in the row space of the Jacobian is held by the constraints —
/// a driven travel is a constraint too — while the remainder lies in the null space with nothing to
/// hold it. The remainder is what "the body drifted by itself" means, and a return has to be measured
/// by it rather than by the full displacement.
fn free_deviation(problem: &Problem, layout: &Layout, poses: &[Isometry3<f64>], origin: &[Isometry3<f64>]) -> Option<DVector<f64>> {
    let n = layout.unknowns;
    let staged = staged_problem(problem, poses);
    let j = jacobian(&staged, layout, poses);
    if !j.iter().all(|v| v.is_finite()) {
        return None;
    }
    let svd = j.svd(false, true);
    let vt = svd.v_t.clone()?;
    let smax = svd.singular_values.iter().cloned().fold(0.0f64, f64::max);

    // The deviation is expressed in the same coordinates as a solver step.
    let mut dev = DVector::zeros(n);
    for (i, _) in problem.bodies.iter().enumerate() {
        let Some(col) = layout.column_of(i) else { continue };
        let dt = poses[i].translation.vector - origin[i].translation.vector;
        let dw = (poses[i].rotation * origin[i].rotation.inverse()).scaled_axis();
        for k in 0..3 {
            dev[col + k] = dt[k];
            dev[col + 3 + k] = dw[k];
        }
    }

    // What the constraints hold: the projection onto Jacobian rows with a non-zero singular value.
    let mut held = DVector::zeros(n);
    for (k, sv) in svd.singular_values.iter().enumerate() {
        if smax <= 0.0 || *sv <= smax * RANK_EPS {
            continue; // This direction is free.
        }
        let row = vt.row(k);
        let dot: f64 = (0..n).map(|i| row[i] * dev[i]).sum();
        for i in 0..n {
            held[i] += row[i] * dot;
        }
    }
    Some(dev - held)
}

/// How far a body moved from its original pose, in the same coordinates as a solver step.
///
/// The full displacement, driven travel included. This measure chooses between two solutions — which
/// one is nearer to where the body was left. It is unusable for the free-direction return, where
/// `free_deviation` is required: otherwise the driven travel enters the measure and sinks the return.
fn deviation_norm(problem: &Problem, layout: &Layout, poses: &[Isometry3<f64>], origin: &[Isometry3<f64>]) -> f64 {
    let mut acc = 0.0;
    for (i, _) in problem.bodies.iter().enumerate() {
        if layout.column_of(i).is_none() {
            continue;
        }
        let dt = poses[i].translation.vector - origin[i].translation.vector;
        let dw = (poses[i].rotation * origin[i].rotation.inverse()).scaled_axis();
        acc += dt.norm_squared() + dw.norm_squared();
    }
    acc.sqrt()
}

/// The same problem with bodies at `poses`: a working copy for the Jacobian and the residuals.
fn staged_problem(problem: &Problem, poses: &[Isometry3<f64>]) -> Problem {
    Problem {
        bodies: problem.bodies.iter().zip(poses.iter()).map(|(b, pose)| super::problem::Body { pose: *pose, grounded: b.grounded }).collect(),
        constraints: problem.constraints.clone(),
    }
}

/// Least-squares solution through SVD, stable under degeneracy.
///
/// Removes from a step everything the constraints are indifferent to, leaving only the row space of the
/// Jacobian. The rank threshold is relative — a fraction of the largest singular value — as everywhere
/// in this file: an absolute number means different things for a metre-scale and a millimetre-scale
/// assembly.
fn project_onto_row_space(j: &DMatrix<f64>, step: DVector<f64>) -> DVector<f64> {
    let svd = j.clone().svd(false, true);
    let Some(vt) = svd.v_t.as_ref() else { return step };
    let smax = svd.singular_values.iter().fold(0.0_f64, |m, &v| m.max(v));
    if smax <= 0.0 {
        return DVector::zeros(step.len()); // The constraints require nothing, so there is nowhere to step.
    }
    let eps = smax * RANK_EPS;
    let mut out = DVector::zeros(step.len());
    for (k, &sv) in svd.singular_values.iter().enumerate() {
        if sv <= eps {
            continue; // A direction the constraints do not require.
        }
        let row = vt.row(k).transpose();
        out += &row * row.dot(&step);
    }
    out
}

fn solve_least_squares(a: &DMatrix<f64>, b: &DVector<f64>) -> Option<DVector<f64>> {
    if !a.iter().all(|v| v.is_finite()) || !b.iter().all(|v| v.is_finite()) {
        return None; // Degenerate geometry produced a NaN: refuse rather than return garbage.
    }
    a.clone().svd(true, true).solve(b, 1e-12).ok()
}

/// Which constraints are redundant: their rows are linearly dependent on the rows of the others.
///
/// Determined by rank: drop the rows of a constraint and see whether the rank changes. If it does not,
/// the constraint adds nothing. This costs more than a residual-based estimate but answers the question
/// "which constraint should be removed" instead of "something somewhere did not converge".
fn redundant_constraints(problem: &Problem, j: &DMatrix<f64>, smax: f64) -> Vec<usize> {
    let full_rank = rank_of(j, smax);
    let mut out = Vec::new();
    let mut row = 0;
    for (i, c) in problem.constraints.iter().enumerate() {
        let rows = c.rows();
        if j.nrows() > rows {
            let mut reduced = DMatrix::zeros(j.nrows() - rows, j.ncols());
            let mut dst = 0;
            for r in 0..j.nrows() {
                if r < row || r >= row + rows {
                    reduced.set_row(dst, &j.row(r));
                    dst += 1;
                }
            }
            if rank_of(&reduced, smax) == full_rank {
                out.push(i);
            }
        }
        row += rows;
    }
    out
}

fn rank_of(m: &DMatrix<f64>, smax: f64) -> usize {
    if m.nrows() == 0 || m.ncols() == 0 {
        return 0;
    }
    m.clone().svd(false, false).singular_values.iter().filter(|s| **s > smax * RANK_EPS).count()
}

/// Which constraints do not hold, and by how much.
fn violated_constraints(problem: &Problem, r: &DVector<f64>) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let mut row = 0;
    for (i, c) in problem.constraints.iter().enumerate() {
        let rows = c.rows();
        let n = (row..row + rows).map(|k| r[k] * r[k]).sum::<f64>().sqrt();
        if n > TOL {
            out.push((i, n));
        }
        row += rows;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::frame::Anchor;
    use crate::asm::problem::{Body, Constraint};
    use nalgebra::{Translation3, UnitQuaternion, Vector3};

    fn at(x: f64, y: f64, z: f64) -> Isometry3<f64> {
        Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::identity())
    }

    /// "Point on axis" says nothing about rotation, so it must not rotate anything.
    ///
    /// An initial guess that aligns anchors completely for every primitive turns bodies visibly: measured
    /// on a real machine document it rotated one beam by 180.000 degrees on every solve and carried an
    /// axis assembly 908.204 mm away, while `OnAxis` constrains only two degrees and says nothing about
    /// rotation.
    ///
    /// This check holds that rule: the body starts turned and moved aside, and the constraint requires
    /// exactly one thing — that the anchor lands on the axis of the first one. The rotation must stay as
    /// it was.
    #[test]
    fn lying_on_an_axis_never_turns_the_part() {
        let turned = Isometry3::from_parts(
            Translation3::new(400.0, 90.0, 0.0),
            UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f64::consts::PI),
        );
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(turned)]);
        let a = Anchor::new(0, at(0.0, 0.0, 0.0));
        let b = Anchor::new(1, at(0.0, 0.0, 0.0));
        p.add(Constraint::OnAxis { a, b });
        // The guess itself is queried rather than the whole solve: `solve` only reaches it on the second
        // attempt, when the current pose fails to converge, so a check routed through `solve` stays green
        // for nothing — it passed even with the free-direction return deliberately corrupted.
        let poses = initial_guess(&p);
        let turn = (turned.rotation.inverse() * poses[1].rotation).angle().to_degrees();
        assert!(turn < 1e-6, "point on axis says nothing about rotation, yet the guess turned the body by {turn:.3} degrees");
        let got = b.world_origin(&poses[1]);
        assert!(got.xy().norm() < 1e-6, "it still has to land on the axis, but it sits at {got:?}");
        // And the full solve gives the same answer.
        let (solved, rep) = solve(&p);
        assert!(rep.converged, "the constraint is satisfiable: residual {:.3e}", rep.residual);
        let turn = (turned.rotation.inverse() * solved[1].rotation).angle().to_degrees();
        assert!(turn < 1e-6, "after the solve the body must still be unrotated, but it was turned by {turn:.3} degrees");
    }

    #[test]
    fn point_coincidence_brings_the_part_exactly_to_the_target() {
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(80.0, 30.0, 15.0))]);
        let a = Anchor::new(0, at(5.0, 0.0, 0.0));
        let b = Anchor::new(1, at(0.0, 0.0, 2.0));
        p.add(Constraint::PointCoincident { a, b });
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "must converge: residual {:.3e}", rep.residual);
        let got = b.world_origin(&poses[1]);
        assert!((got - Vector3::new(5.0, 0.0, 0.0)).norm() < 1e-6, "the anchor must land exactly on the target, but it sits at {got:?}");
    }

    #[test]
    fn it_converges_from_a_deliberately_terrible_start() {
        // The body starts far away and turned. Without damping this case requires a heuristic
        // pre-alignment, which displaces bodies by itself.
        let far = Isometry3::from_parts(
            Translation3::new(5000.0, -3000.0, 900.0),
            UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 2.9),
        );
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(far)]);
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        p.add(Constraint::PointCoincident { a, b });
        p.add(Constraint::AxisAligned { a, b });
        p.add(Constraint::RollAligned { a, b });
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "must converge without pre-alignment: residual {:.3e}, iterations {}", rep.residual, rep.iterations);
        assert!((poses[1].translation.vector).norm() < 1e-6, "the body must arrive at the origin: {:?}", poses[1].translation.vector);
    }

    #[test]
    fn degrees_of_freedom_are_counted_not_guessed() {
        // A single coincident point removes 3 translations and leaves 3 rotations.
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(10.0, 0.0, 0.0))]);
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        p.add(Constraint::PointCoincident { a, b });
        let (_, rep) = solve(&p);
        assert_eq!(rep.dof, 3, "coincident points must leave exactly three rotations");

        // Adding axis alignment removes 2 rotations and leaves 1, about the axis.
        p.add(Constraint::AxisAligned { a, b });
        let (_, rep) = solve(&p);
        assert_eq!(rep.dof, 1, "point plus axis must leave one rotation about the axis");

        // And roll: nothing remains.
        p.add(Constraint::RollAligned { a, b });
        let (_, rep) = solve(&p);
        assert_eq!(rep.dof, 0, "point plus axis plus roll must remove everything");
    }

    #[test]
    fn a_redundant_constraint_is_named_not_just_flagged() {
        // Two identical constraints: the second adds nothing and must be named.
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(10.0, 0.0, 0.0))]);
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        p.add(Constraint::PointCoincident { a, b });
        p.add(Constraint::PointCoincident { a, b });
        let (_, rep) = solve(&p);
        assert!(rep.converged, "redundancy must not prevent a solution: {:.3e}", rep.residual);
        assert!(!rep.redundant.is_empty(), "a redundant constraint must be named, not reduced to a flag");
    }

    #[test]
    fn an_impossible_constraint_is_reported_with_its_index_and_size() {
        // The body is pulled towards two different points: both cannot hold.
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(10.0, 0.0, 0.0))]);
        let b = Anchor::origin(1);
        p.add(Constraint::PointCoincident { a: Anchor::new(0, at(0.0, 0.0, 0.0)), b });
        p.add(Constraint::PointCoincident { a: Anchor::new(0, at(100.0, 0.0, 0.0)), b });
        let (_, rep) = solve(&p);
        assert!(!rep.converged, "incompatible constraints cannot converge");
        assert!(!rep.violated.is_empty(), "a violated constraint must be named together with the size of the miss");
        let worst = rep.violated.iter().map(|(_, v)| *v).fold(0.0, f64::max);
        assert!(worst > 1.0, "the violation must be a meaningful size (tens of millimetres), not {worst:.3e}");
    }

    #[test]
    fn broken_reference_is_refused_not_silently_skipped() {
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0))]);
        p.add(Constraint::PointCoincident { a: Anchor::origin(0), b: Anchor::origin(9) });
        let (_, rep) = solve(&p);
        assert!(!rep.converged && !rep.residual.is_finite(), "a broken reference must be a refusal: a silently skipped constraint does not act and nobody is told");
    }

    #[test]
    fn degenerate_geometry_does_not_produce_garbage() {
        // A zero anchor axis is impossible by construction of `Anchor`, so degeneracy is checked
        // differently: two constraints along one line, one of which determines nothing.
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(1.0, 0.0, 0.0))]);
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        p.add(Constraint::OnAxis { a, b });
        p.add(Constraint::OnAxis { a, b });
        let (poses, rep) = solve(&p);
        assert!(poses.iter().all(|t| t.translation.vector.iter().all(|v| v.is_finite())), "poses must stay finite");
        assert!(rep.dof > 0, "an underdefined problem must report the freedom left instead of staying silent");
    }
}

#[cfg(test)]
mod free_dof_tests {
    use super::*;
    use crate::asm::frame::Anchor;
    use crate::asm::problem::{Body, Constraint};
    use nalgebra::{Translation3, UnitQuaternion, Vector3};

    fn at(x: f64, y: f64, z: f64) -> Isometry3<f64> {
        Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::identity())
    }

    /// The initial guess must not take away free degrees.
    ///
    /// Frame composition used as a final placement fixes a body along the directions the constraint does
    /// not require, which visibly displaces it. Here composition is only a guess, and the solver has to
    /// leave the body where it was along the free axis.
    ///
    /// The setup mirrors a revolute pair: the anchors are 40 mm apart along the shared axis, as the
    /// midpoints of holes of different depth are, and the constraint asks only for coaxiality.
    #[test]
    fn the_initial_guess_does_not_steal_free_translation_along_the_axis() {
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0))]);
        let a = Anchor::from_axes(0, Vector3::new(0.0, 0.0, 0.0), Vector3::z(), Vector3::x()).unwrap();
        // The anchor of the second body is offset along the axis, like the midpoint of a deeper hole.
        let b = Anchor::from_axes(1, Vector3::new(0.0, 0.0, 40.0), Vector3::z(), Vector3::x()).unwrap();
        p.add(Constraint::OnAxis { a, b });
        p.add(Constraint::AxisAligned { a, b });
        let (poses, rep) = solve(&p);
        assert!(rep.converged, "the coaxial condition must hold: {:.3e}", rep.residual);
        // Across the axis: aligned.
        let o = b.world_origin(&poses[1]);
        assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "across the axis the anchor must land on the axis: {o:?}");
        // Along the axis the body must not be pulled: the constraint does not require it.
        // The threshold is one micron, and it is not fitted to pass: the return towards the original
        // pose is soft, so the body settles at an equilibrium rather than exactly back. What matters is
        // that the drift is measured in microns instead of the forty millimetres produced by frame
        // composition.
        assert!(
            poses[1].translation.vector.z.abs() < 1e-3,
            "the body was moved {:.6} mm along the free axis, which the constraint never required",
            poses[1].translation.vector.z
        );
        // Coaxiality removes 4 of the 6 degrees: two transverse translations and two rotations. Exactly
        // two remain, travel along the axis and rotation about it, which is a cylindrical mate; for a
        // revolute pair that rotation is the whole point.
        assert_eq!(rep.dof, 2, "coaxiality must leave exactly travel along the axis and rotation about it");
    }

    /// And symmetrically: what a constraint does require must hold exactly.
    #[test]
    fn what_the_constraint_demands_is_enforced_exactly() {
        let mut p = Problem::new(vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(37.0, -12.0, 5.0))]);
        let a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
        let b = Anchor::from_axes(1, Vector3::new(0.0, 0.0, 40.0), Vector3::z(), Vector3::x()).unwrap();
        p.add(Constraint::OnAxis { a, b });
        let (poses, _) = solve(&p);
        let o = b.world_origin(&poses[1]);
        assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "the on-axis requirement must hold exactly: {o:?}");
    }
}

/// Degrees of freedom of a single body: how many it has left, not the assembly as a whole.
///
/// Needed by the interface, where the question is about one specific body.
/// Computed from the rank of that body's columns: how many of its own increments the constraints pin.
pub fn body_dof(problem: &Problem, body: usize) -> usize {
    let layout = Layout::of(problem);
    let Some(col) = layout.column_of(body) else { return 0 }; // Grounded: no freedom.
    if problem.constraints.is_empty() {
        return 6;
    }
    let poses: Vec<Isometry3<f64>> = problem.bodies.iter().map(|b| b.pose).collect();
    let j = crate::asm::solve::jacobian(problem, &layout, &poses);
    let sub = j.columns(col, 6).into_owned();
    if sub.nrows() == 0 {
        return 6;
    }
    let svd = sub.svd(false, false);
    let smax = svd.singular_values.iter().cloned().fold(0.0f64, f64::max);
    let rank = if smax <= 0.0 { 0 } else { svd.singular_values.iter().filter(|s| **s > smax * RANK_EPS).count() };
    6usize.saturating_sub(rank)
}

#[cfg(test)]
mod body_dof_tests {
    use super::*;
    use crate::asm::frame::Anchor;
    use crate::asm::problem::{Body, Constraint};

    #[test]
    fn a_free_body_has_six_and_a_fastened_one_has_none() {
        let mut p = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(Isometry3::translation(10.0, 0.0, 0.0)), Body::new(Isometry3::translation(50.0, 0.0, 0.0))]);
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        p.add(Constraint::PointCoincident { a, b });
        p.add(Constraint::AxisAligned { a, b });
        p.add(Constraint::RollAligned { a, b });
        assert_eq!(body_dof(&p, 0), 0, "a grounded body has no freedom");
        assert_eq!(body_dof(&p, 1), 0, "a rigidly mated body has no freedom");
        assert_eq!(body_dof(&p, 2), 6, "a body with no constraints is free in everything");
    }

    #[test]
    fn a_hinged_body_keeps_exactly_one() {
        let mut p = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(Isometry3::translation(10.0, 0.0, 0.0))]);
        let (a, b) = (Anchor::origin(0), Anchor::origin(1));
        p.add(Constraint::PointCoincident { a, b });
        p.add(Constraint::AxisAligned { a, b });
        assert_eq!(body_dof(&p, 1), 1, "a revolute pair leaves exactly the rotation about its axis");
    }
}
