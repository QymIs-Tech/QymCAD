//! THE CONFLICTS OF A SKETCH: which constraints argue, found by elimination.

use super::*;

/// Conflicting constraints: the indices of constraints that are contradictory *together* — the system cannot be
/// satisfied until at least one of them is removed or turned into a driven (reference) one.
///
/// The method: reduce the augmented matrix [J | r] to row echelon form. A row whose coefficients on the
/// variables have cancelled out while its residual has not is a linear combination of constraints with an
/// inconsistent right-hand side, and every constraint whose row entered that combination belongs to the
/// conflicting set. This is what lets the sketch point at the specific dimensions that disagree instead of
/// reporting one overall residual.
pub fn conflicts(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<usize> {
    conflicts_remembered(points, radii, constraints, &mut PartMemo::default())
}

/// `conflicts`, the rows of each part taken from `memo` where its print is there (`PartMemo`).
pub fn conflicts_remembered(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], memo: &mut PartMemo) -> Vec<usize> {
    // Part by part, as `dof`: no row of one part combines with a row of another. The rows of every part are judged
    // together, against the largest residual of the sketch, as the rows of the whole were.
    let mut rows = Vec::new();
    let mut kept: HashMap<u64, Vec<Eliminated>> = HashMap::new();
    let mut buf = String::new();
    for part in parts(points, radii, constraints).iter().filter(|p| !p.constraints.is_empty()) {
        let print = part_print(part, points, radii, constraints, 0, &mut buf);
        let mine = match memo.conflicts.remove(&print).or_else(|| kept.get(&print).cloned()) {
            Some(r) => r,
            None => {
                let own = part.own(points, radii, constraints);
                eliminated_sparse(&own.points, &own.radii, &own.constraints)
            }
        };
        rows.extend(mine.iter().map(|r| Eliminated { from: r.from.iter().map(|&k| part.constraints[k]).collect(), ..*r }));
        kept.insert(print, mine);
    }
    memo.conflicts = kept;
    conflicting(&rows)
}

/// The conflicting constraints of the whole sketch from one elimination (`conflicts`); the reference of the parts.
pub fn conflicts_whole(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<usize> {
    conflicting(&eliminated(points, radii, constraints))
}

/// A row of the augmented matrix [J | r] after the elimination of `eliminated`.
#[derive(Clone)]
pub(crate) struct Eliminated {
    /// no coefficient is left on a free unknown: moving the unanchored geometry can no longer satisfy the row
    pub(crate) null: bool,
    pub(crate) residual: f64,
    /// the constraints whose rows went into it
    pub(crate) from: Vec<usize>,
}

/// The constraints of the null rows whose residual is left: those that disagree together.
pub(crate) fn conflicting(rows: &[Eliminated]) -> Vec<usize> {
    // residual scale: compare against a typical magnitude rather than against absolute zero
    let scale = rows.iter().map(|r| r.residual.abs()).fold(0.0_f64, f64::max).max(1.0);
    let mut bad: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for r in rows.iter().filter(|r| r.null && r.residual.abs() > 1e-6 * scale) {
        bad.extend(r.from.iter().copied());
    }
    bad.into_iter().collect()
}

/// The rows of [J | r] of a sketch reduced to row echelon form, see `conflicts`.
/// THE ROWS OF [J | r] OF A SKETCH for `conflicts`: every row of every constraint that can disagree, its non-zero
/// coefficients by the column as the analytic Jacobian gives them (summed where a constraint names a variable twice),
/// its residual and its constraint; with the variables that are not free and the number of variables.
pub(crate) struct ConflictRows {
    pub(crate) rows: Vec<ConflictRow>,
    pub(crate) fixed_var: Vec<bool>,
    pub(crate) nv: usize,
}

pub(crate) struct ConflictRow {
    pub(crate) coeffs: Vec<(usize, f64)>,
    pub(crate) residual: f64,
    pub(crate) from: usize,
}

pub(crate) fn conflict_rows(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> ConflictRows {
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
    let nv = np * 2 + radii.len();

    // Anchored points are not variables here either, exactly as in the solve. Otherwise the analysis treats
    // their coordinates as free, and a constraint that is impossible precisely because of an anchor — say a
    // tangency dimension between two pinned circles — is never recognised as contradictory.
    let mut fixed_var = vec![false; nv];
    for c in constraints.iter() {
        if let Constraint::Fixed { p } = *c {
            if let Some(&i) = idx.get(&p) {
                fixed_var[2 * i] = true;
                fixed_var[2 * i + 1] = true;
            }
        }
    }
    let mut rows = Vec::new();
    for (ci, c) in constraints.iter().enumerate() {
        if !cons_ok(c, &has, &is_center) || matches!(c, Constraint::Fixed { .. }) {
            continue; // an anchor is part of the statement of the problem, not a dimension that can disagree
        }
        if c.is_driven() {
            continue; // a driven dimension constrains nothing
        }
        let mut r = Vec::new();
        con_rows(c, &x, &x, &idx, &ridx, &anchor, &mut r);
        let mut j = Vec::new();
        con_jac(c, &x, &x, &idx, &ridx, &mut j);
        for (t, rv) in r.iter().enumerate() {
            // summed by the column, in the order the Jacobian gives them, as a dense row sums them
            let mut by_col: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
            for &(k, v) in j.get(t).into_iter().flatten() {
                *by_col.entry(k).or_insert(0.0) += v;
            }
            rows.push(ConflictRow { coeffs: by_col.into_iter().collect(), residual: *rv, from: ci });
        }
    }
    ConflictRows { rows, fixed_var, nv }
}

/// The rows of [J | r] of a sketch reduced to row echelon form, see `conflicts`. Dense: the reference of
/// `eliminated_sparse`.
pub(crate) fn eliminated(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<Eliminated> {
    if points.is_empty() {
        return Vec::new();
    }
    let ConflictRows { rows, fixed_var, nv } = conflict_rows(points, radii, constraints);
    // rows: [coefficients per variable | residual | set of constraints that went into the row]
    let mut rows: Vec<(Vec<f64>, f64, Vec<usize>)> = rows
        .into_iter()
        .map(|r| {
            let mut dense = vec![0.0; nv];
            for (k, v) in r.coeffs {
                dense[k] += v;
            }
            (dense, r.residual, vec![r.from])
        })
        .collect();

    // forward elimination with pivoting
    let mut used = vec![false; rows.len()];
    for col in (0..nv).filter(|k| !fixed_var[*k]) {
        let piv = (0..rows.len()).filter(|&i| !used[i]).max_by(|&a, &b| rows[a].0[col].abs().partial_cmp(&rows[b].0[col].abs()).unwrap_or(std::cmp::Ordering::Equal));
        let Some(piv) = piv.filter(|&i| rows[i].0[col].abs() > 1e-9) else { continue };
        used[piv] = true;
        for i in 0..rows.len() {
            if i == piv || used[i] || rows[i].0[col].abs() <= 1e-12 {
                continue;
            }
            let f = rows[i].0[col] / rows[piv].0[col];
            for k in col..nv {
                rows[i].0[k] -= f * rows[piv].0[k];
            }
            rows[i].1 -= f * rows[piv].1;
            let src: Vec<usize> = rows[piv].2.clone();
            rows[i].2.extend(src);
        }
    }
    rows.into_iter()
        .map(|(dense, residual, from)| {
            // a row counts as null when no coefficients are left on the free variables: moving the unanchored
            // geometry can no longer satisfy it
            let null = (0..nv).filter(|k| !fixed_var[*k]).all(|k| dense[k].abs() <= 1e-7);
            Eliminated { null, residual, from }
        })
        .collect()
}

/// The elimination of `eliminated` step for step, on the non-zero coefficients only: for each free column the row
/// not yet taken with the largest coefficient - the last of equal ones, as `max_by` takes it - when above 1e-9, and
/// the column cleared from every row not yet taken where it stands above 1e-12. A part of 1 000 rectangles is
/// 8 000 columns of a few coefficients each, where the dense rows are 8 000 wide.
pub(crate) fn eliminated_sparse(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<Eliminated> {
    if points.is_empty() {
        return Vec::new();
    }
    let ConflictRows { rows, fixed_var, nv } = conflict_rows(points, radii, constraints);
    let n = rows.len();
    let mut coeffs: Vec<std::collections::BTreeMap<usize, f64>> = Vec::with_capacity(n);
    let mut residual = Vec::with_capacity(n);
    // the constraint of each row, and the pivot rows each was taken from; the constraints a row came to stand for are
    // made afterwards for the null rows alone (`from_of`): gathered for every row as it went, the sets grew with the fill,
    // 56 ms of a count of a sketch of 1 062 points on lines
    let mut own: Vec<usize> = Vec::with_capacity(n);
    let mut taken: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut order: Vec<usize> = Vec::new();
    for r in rows {
        coeffs.push(r.coeffs.into_iter().collect());
        residual.push(r.residual);
        own.push(r.from);
    }
    let mut in_col: Vec<Vec<usize>> = vec![Vec::new(); nv];
    for (r, row) in coeffs.iter().enumerate() {
        for &c in row.keys() {
            in_col[c].push(r);
        }
    }
    let mut used = vec![false; n];
    for col in (0..nv).filter(|k| !fixed_var[*k]) {
        let value = |coeffs: &[std::collections::BTreeMap<usize, f64>], r: usize| coeffs[r].get(&col).copied().unwrap_or(0.0);
        let mut piv: Option<usize> = None;
        for &r in in_col[col].iter().filter(|&&r| !used[r]) {
            let v = value(&coeffs, r).abs();
            if piv.is_none_or(|p| v > value(&coeffs, p).abs() || v == value(&coeffs, p).abs() && r > p) {
                piv = Some(r);
            }
        }
        let Some(piv) = piv.filter(|&p| value(&coeffs, p).abs() > 1e-9) else { continue };
        used[piv] = true;
        order.push(piv);
        let pivot: Vec<(usize, f64)> = coeffs[piv].range(col..).map(|(&c, &v)| (c, v)).collect();
        let (d, pivot_residual) = (value(&coeffs, piv), residual[piv]);
        for &i in &in_col[col].clone() {
            if used[i] || value(&coeffs, i).abs() <= 1e-12 {
                continue;
            }
            let f = value(&coeffs, i) / d;
            for &(c, v) in &pivot {
                let e = coeffs[i].entry(c).or_insert_with(|| {
                    in_col[c].push(i);
                    0.0
                });
                *e -= f * v;
            }
            residual[i] -= f * pivot_residual;
            taken[i].push(piv);
        }
    }
    let null: Vec<bool> = coeffs.iter().map(|row| row.iter().filter(|(k, _)| !fixed_var[**k]).all(|(_, v)| v.abs() <= 1e-7)).collect();
    let mut from = from_of(&null, &own, &taken, &order);
    null.into_iter().zip(residual).enumerate().map(|(r, (null, residual))| Eliminated { null, residual, from: std::mem::take(&mut from[r]) }).collect()
}

/// The constraints each null row came to stand for: its own and those of every pivot row it was taken from, through
/// theirs. A pivot row is taken from only by pivots taken before it (`order`), and stands as it was once taken; the rows
/// not null are left without, nothing reads them.
pub(crate) fn from_of(null: &[bool], own: &[usize], taken: &[Vec<usize>], order: &[usize]) -> Vec<Vec<usize>> {
    let mut reached = vec![false; own.len()];
    let mut stack: Vec<usize> = (0..own.len()).filter(|&r| null[r]).flat_map(|r| taken[r].iter().copied()).collect();
    while let Some(p) = stack.pop() {
        if !std::mem::replace(&mut reached[p], true) {
            stack.extend(taken[p].iter().copied());
        }
    }
    let mut of: Vec<Option<std::collections::BTreeSet<usize>>> = vec![None; own.len()];
    let gather = |r: usize, of: &[Option<std::collections::BTreeSet<usize>>]| -> std::collections::BTreeSet<usize> {
        let mut s: std::collections::BTreeSet<usize> = std::iter::once(own[r]).collect();
        for &p in &taken[r] {
            s.extend(of[p].iter().flatten().copied());
        }
        s
    };
    for &p in order.iter().filter(|&&p| reached[p]) {
        of[p] = Some(gather(p, &of));
    }
    (0..own.len()).map(|r| if null[r] { gather(r, &of).into_iter().collect() } else { Vec::new() }).collect()
}

/// Jacobian cross-check, used by tests: the largest discrepancy between the analytic derivatives of a
/// constraint and numerical ones.
///
/// An error in a hand-written formula is the quietest failure available — the solution simply converges a
/// little worse, which goes unnoticed until a part shows up where a 130 mm dimension lands at −129.99900. Every
/// constraint type is therefore checked here against a central difference on a random, non-degenerate
/// configuration.
pub fn jacobian_mismatch(points: &[SketchPoint], radii: &[RadiusVar], c: &Constraint) -> f64 {
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let np = points.len();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(i, r)| (r.center, np * 2 + i)).collect();
    let anchor: HashMap<Id, (f64, f64)> = points.iter().map(|p| (p.id, (p.x, p.y))).collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|r| r.value));
    let x0 = x.clone();

    let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
    con_jac(c, &x, &x0, &idx, &ridx, &mut rows);
    let r_at = |xx: &[f64]| -> Vec<f64> {
        let mut r = Vec::new();
        con_rows(c, xx, &x0, &idx, &ridx, &anchor, &mut r);
        r
    };
    let base = r_at(&x);
    let mut worst = 0.0_f64;
    for k in 0..x.len() {
        let h = 1e-6 * x[k].abs().max(1.0);
        let mut xp = x.clone();
        xp[k] = x[k] + h;
        let rp = r_at(&xp);
        xp[k] = x[k] - h;
        let rm = r_at(&xp);
        for t in 0..base.len() {
            let num = (rp[t] - rm[t]) / (2.0 * h);
            let ana: f64 = rows.get(t).map(|row| row.iter().filter(|(i, _)| *i == k).map(|(_, v)| *v).sum()).unwrap_or(0.0);
            let scale = num.abs().max(ana.abs()).max(1.0);
            worst = worst.max((num - ana).abs() / scale);
        }
    }
    worst
}
