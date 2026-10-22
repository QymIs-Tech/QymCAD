//! LINEAR SYSTEMS OF THE CORE: a sparse symmetric positive definite matrix, given by the entries of its lower triangle,
//! factored by Cholesky (`faer`, ordered to keep the factor sparse) and solved for one right side or several. The step
//! of the sketch solver (`solver::lm`) and the tangents of a spline (`model::tess`) both solve such a system.
use faer::sparse::Triplet;

/// THE ENTRIES SUMMED BY PLACE, sorted by column, then row: what a matrix is assembled from, each place once.
pub(crate) fn summed_by_place(mut parts: Vec<Triplet<usize, usize, f64>>) -> Vec<Triplet<usize, usize, f64>> {
    parts.sort_unstable_by(|a, b| a.col.cmp(&b.col).then(a.row.cmp(&b.row)));
    let mut sums: Vec<Triplet<usize, usize, f64>> = Vec::with_capacity(parts.len());
    for e in parts {
        match sums.last_mut() {
            Some(last) if last.row == e.row && last.col == e.col => last.val += e.val,
            _ => sums.push(e),
        }
    }
    sums
}

/// THE SOLUTION OF `A x = b` for every right side `b` of `sides`, `A` of `n` unknowns given by the entries of its lower
/// triangle, each place once. `None` where `A` is not positive definite, or a solution not finite.
pub(crate) fn solve_spd(n: usize, lower: &[Triplet<usize, usize, f64>], sides: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    use faer::linalg::solvers::Solve;
    let matrix = faer::sparse::SparseColMat::<usize, f64>::try_new_from_triplets(n, n, lower).ok()?;
    let factor = matrix.sp_cholesky(faer::Side::Lower).ok()?;
    let x = factor.solve(faer::Mat::<f64>::from_fn(n, sides.len(), |r, c| sides[c][r]));
    let out: Vec<Vec<f64>> = (0..sides.len()).map(|c| x.col(c).iter().copied().collect()).collect();
    out.iter().flatten().all(|v| v.is_finite()).then_some(out)
}
