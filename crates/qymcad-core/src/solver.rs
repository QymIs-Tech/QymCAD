//! Geometric constraint solver for a sketch (2D).
//!
//! The variables are point coordinates *and* circle radii: a radius is a real scalar unknown of the solver
//! rather than a field of an entity. Every constraint contributes a residual, and the sum of squares is
//! minimised by Levenberg-Marquardt over an analytic Jacobian.

use std::collections::HashMap;

use crate::model::{Constraint, Id, SketchPoint};

/// Radius unknown of a circle, keyed by the id of its centre (every circle owns its centre).
///
/// Treating the radius as a solver variable on par with coordinates is what makes the degree-of-freedom count,
/// tangency and equal-radius constraints come out right.
#[derive(Clone, Copy, Debug)]
pub struct RadiusVar {
    pub center: Id,
    pub value: f64,
}

/// Solve the constraints over points alone, without radius unknowns — for tests on pure point sets.
pub fn solve(points: &mut [SketchPoint], constraints: &[Constraint]) -> f64 {
    solve_full(points, &mut Vec::new(), constraints, None)
}

/// Solve with a dragged point pulled towards the cursor, without radius unknowns.
pub fn solve_drag(points: &mut [SketchPoint], constraints: &[Constraint], drag: Option<(Id, f64, f64)>) -> f64 {
    solve_full(points, &mut Vec::new(), constraints, drag)
}

/// The full solver: points plus circle radii as unknowns. `drag = Some((id, x, y))` pulls the dragged point
/// towards the cursor; fully constrained geometry resists that pull.
pub fn solve_full(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>) -> f64 {
    solve_full_iter(points, radii, constraints, drag, 120)
}

/// Same as `solve_full`, but with an explicit Levenberg-Marquardt iteration budget. An interactive drag gets a
/// smaller budget: responsiveness matters more than accuracy there, the final exact solve happens on release,
/// and every frame is a warm start from the previous coordinates, so iterations accumulate anyway.
///
/// On top of LM sits a side flip for axis dimensions. The residual |Δ| − d has more than a kink at zero — it has
/// a barrier: for a horizontal or vertical dimension to change sides, Δ must pass through 0, which means climbing
/// over a local maximum of the error. A local method never crosses that, and instead of a solvable system it
/// returns a least-squares compromise in which every dimension is off and the nodes go red. The cure is a
/// multi-start: a violated axis dimension gets its point mirrored to the other side and the system is solved
/// again, and the result is accepted only if the residual strictly dropped. Disabled during a drag, where frame
/// stability and responsiveness outweigh it.
pub fn solve_full_iter(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize) -> f64 {
    solve_within(points, radii, constraints, drag, Budget { steps: max_iter, time: None, sizes: Sizes::Free }).residual
}

/// WHAT A SOLVE MAY SPEND: Levenberg-Marquardt steps a run, and the time of the whole solve. The steps make the answer -
/// the same on any machine; the time is a guard, set well above what a solve takes, against a sketch no budget of
/// steps keeps short.
#[derive(Clone, Copy)]
pub struct Budget {
    pub steps: usize,
    /// `None`: no guard of time
    pub time: Option<std::time::Duration>,
    /// whether the size of a circle is held softly where it stood (`solve_lm`)
    pub sizes: Sizes,
}

/// WHAT A SOLVE DOES WITH THE SIZE OF A CIRCLE NOTHING SETS: held softly where it stood, so a circle tied to lines that
/// move goes with them, or left free as a position is - a frame of a drag, which must come out the same whether the
/// drag keeps its part from frame to frame or solves the frame alone.
#[derive(Clone, Copy, PartialEq)]
pub enum Sizes {
    Held,
    Free,
}

impl Budget {
    /// A solve of a sketch: 120 steps a run, and 10 s for all of it - 70 000 separate lines take 1 s in a release build.
    pub const FULL: Budget = Budget { steps: 120, time: Some(std::time::Duration::from_secs(10)), sizes: Sizes::Held };
    /// A frame of a drag: fewer steps, the release solves in full; and half a second, past which the pointer is not
    /// followed anyway.
    pub const FRAME: Budget = Budget { steps: 40, time: Some(std::time::Duration::from_millis(500)), sizes: Sizes::Free };
    /// THE SOLVE A REBUILD AND AN OPEN DEPEND ON: the steps of a full solve and no clock. What is built and saved from a
    /// sketch must not depend on how fast the machine is or how busy: cut by a time, a sketch of 100 000 circles held
    /// tangent to lines left 24 580 parts unsolved after 12.7 s on one thread (a debug build), and the rebuild built
    /// from them and the file saved them.
    pub const WHOLE: Budget = Budget { steps: 120, time: None, sizes: Sizes::Held };
}

/// What a solve came to.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    /// the residual of every constraint together
    pub residual: f64,
    /// the parts the time ran out before; they are left as they stood, and the next solve takes them up
    pub left: usize,
}

/// `solve_full_iter` within `budget`: the parts not begun when the time is out are left as they stood and counted.
pub fn solve_within(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, budget: Budget) -> Outcome {
    let scale = Scale::of(points);
    let until = budget.time.map(|t| std::time::Instant::now() + t);
    solve_by_parts(points, radii, constraints, drag, budget.steps, Solve { scale, algebra: Algebra::BySize, until, sizes: budget.sizes })
}

/// The solve of `solve_full_iter` with every part solved by the sparse algebra, whatever its size - the check of that
/// algebra against the dense one on parts small enough for both.
pub fn solve_full_iter_sparse(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize) -> f64 {
    let scale = Scale::of(points);
    solve_by_parts(points, radii, constraints, drag, max_iter, Solve { scale, algebra: Algebra::Sparse, until: None, sizes: Sizes::Free }).residual
}

/// THE PART OF A SKETCH THAT HOLDS A POINT, by places in the lists given: what a solve dragging that point solves.
pub struct PartPlaces {
    pub points: Vec<usize>,
    pub radii: Vec<usize>,
    pub constraints: Vec<usize>,
}

/// The part of the point `id` (`PartPlaces`), `None` for a point not in `points`.
pub fn part_holding(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], id: Id) -> Option<PartPlaces> {
    let at = points.iter().position(|p| p.id == id)?;
    parts(points, radii, constraints).into_iter().find(|p| p.points.contains(&at)).map(|p| PartPlaces { points: p.points, radii: p.radii, constraints: p.constraints })
}

/// `solve_within` of one part of a sketch given alone, to the thresholds of the whole sketch `whole` - as a solve of
/// the whole sketch solves that part.
pub fn solve_part_within(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, budget: Budget, whole: &[SketchPoint]) -> Outcome {
    let scale = Scale::of(whole);
    let until = budget.time.map(|t| std::time::Instant::now() + t);
    solve_by_parts(points, radii, constraints, drag, budget.steps, Solve { scale, algebra: Algebra::BySize, until, sizes: budget.sizes })
}

/// The whole sketch as one system, the way it was solved before it was split into parts (`solve_full_iter`). Kept as
/// the reference the parts are checked against: on a sketch whose parts are solved alone, the answer is the same.
pub fn solve_full_iter_whole(points: &mut [SketchPoint], radii: &mut [RadiusVar], constraints: &[Constraint], drag: Option<(Id, f64, f64)>, max_iter: usize) -> f64 {
    let scale = Scale::of(points);
    guarded(points, radii, |points, radii| solve_full_iter_inner(points, radii, constraints, drag, max_iter, Solve { scale, algebra: Algebra::Dense, until: None, sizes: Sizes::Free }))
}

/// Run a solve, and put the sketch back as it was if it comes out with a non-number.
fn guarded(points: &mut [SketchPoint], radii: &mut [RadiusVar], solve: impl FnOnce(&mut [SketchPoint], &mut [RadiusVar]) -> f64) -> f64 {
    // The solver must never be able to corrupt the sketch.
    //
    // A numerical method can produce non-numbers: a degenerate system, huge magnitudes, a division by
    // near-zero inside the Jacobian. Once a NaN reaches point coordinates it never leaves — it is saved into
    // the document, spreads into bodies, bounding boxes and the view, and the sketch is broken for good without
    // a single message. So the input is remembered and the output is checked: if the result is not a number,
    // the previous state is restored and an infinite residual is reported, leaving the sketch red but the
    // geometry intact. Returning garbage silently is not an option.
    let backup: Vec<SketchPoint> = points.to_vec();
    let backup_r: Vec<RadiusVar> = radii.to_vec();
    let res = solve(points, radii);
    let clean = res.is_finite() && points.iter().all(|p| p.x.is_finite() && p.y.is_finite()) && radii.iter().all(|r| r.value.is_finite());
    if clean {
        return res;
    }
    points.clone_from_slice(&backup);
    radii.clone_from_slice(&backup_r);
    f64::INFINITY
}

#[cfg(test)]
mod sparse_elimination {
    use super::{pivot_columns, pivot_columns_sparse};

    /// A fixed pseudo-random sequence in 0..n.
    struct Seq(u64);

    impl Seq {
        fn next(&mut self, n: u64) -> u64 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n
        }
    }

    #[test]
    fn the_sparse_elimination_takes_the_pivot_columns_the_dense_one_takes() {
        let mut s = Seq(3);
        let mut failures = Vec::new();
        for case in 0..400 {
            let (rows, cols) = (2 + s.next(40) as usize, 2 + s.next(40) as usize);
            // values from a few levels, so equal magnitudes meet and the first of them must be taken; rows laid twice
            // and rows made of others, so the rank falls short
            let mut dense: Vec<Vec<f64>> = vec![vec![0.0; cols]; rows];
            for row in dense.iter_mut() {
                for _ in 0..1 + s.next(4) {
                    row[s.next(cols as u64) as usize] = [1.0, -1.0, 0.5, 2.0, 1e-8][s.next(5) as usize];
                }
            }
            for r in 1..rows {
                match s.next(4) {
                    0 => dense[r] = dense[r - 1].clone(),
                    1 => dense[r] = dense[r - 1].iter().zip(&dense[0]).map(|(a, b)| a - b).collect(),
                    _ => {}
                }
            }
            let sparse: Vec<Vec<(usize, f64)>> = dense.iter().map(|row| row.iter().enumerate().filter(|(_, v)| **v != 0.0).map(|(c, &v)| (c, v)).collect()).collect();
            let (by_dense, by_sparse) = (pivot_columns(&mut dense.clone(), cols), pivot_columns_sparse(sparse, cols));
            if by_dense != by_sparse {
                failures.push(format!("case {case}: dense {by_dense:?}, sparse {by_sparse:?}"));
            }
        }
        assert!(failures.is_empty(), "the eliminations disagree:\n{}", failures.join("\n"));
    }
}

mod parts;
pub use parts::*;
mod lm;
pub use lm::*;
mod conflicts;
pub use conflicts::*;
mod jacobian;
pub use jacobian::*;
mod freedom;
pub use freedom::*;
