//! THE FREEDOM LEFT IN A SKETCH: elimination, free points, redundant constraints, the checks remembered.

use super::*;

/// THE JACOBIAN OF A PART BY DIFFERENCES, KEPT SPARSE: each row the non-zero derivatives of a constraint, by the
/// column. The same differences as `dof_whole` takes - a variable stepped by `h`, the rows of the constraints that hold
/// it computed again - but only the rows of those constraints, where `dof_whole` computes every row of the part for
/// every variable and keeps a matrix of every row by every variable: 8 000 by 8 000 for an array of 1 000 rectangles.
pub(crate) struct Differences {
    pub(crate) rows: Vec<Vec<(usize, f64)>>,
    /// the constraint of each row, by its place among the constraints given
    pub(crate) of: Vec<usize>,
    pub(crate) nv: usize,
}

pub(crate) fn differences(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Differences {
    let np = points.len();
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(j, rv)| (rv.center, np * 2 + j)).collect();
    let has = |id: Id| idx.contains_key(&id);
    let is_center = |id: Id| ridx.contains_key(&id);
    let given: Vec<usize> = (0..constraints.len()).filter(|&k| cons_ok(&constraints[k], &has, &is_center)).collect();
    let cons: Vec<Constraint> = given.iter().map(|&k| constraints[k].clone()).collect();
    let nv = np * 2 + radii.len();
    let anchor: HashMap<Id, (f64, f64)> = cons
        .iter()
        .filter_map(|c| match *c {
            Constraint::Fixed { p } => idx.get(&p).map(|&i| (p, (points[i].x, points[i].y))),
            _ => None,
        })
        .collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|rv| rv.value));
    let mut of = Vec::new();
    // the rows of every constraint at x, where they start, and the constraints that hold each variable
    let mut r0: Vec<Vec<f64>> = Vec::with_capacity(cons.len());
    let mut first = Vec::with_capacity(cons.len());
    let mut m = 0;
    let mut holding: Vec<Vec<usize>> = vec![Vec::new(); nv];
    for (ci, c) in cons.iter().enumerate() {
        let mut r = Vec::new();
        con_rows(c, &x, &x, &idx, &ridx, &anchor, &mut r); // the side of an axis dimension is read off the current configuration
        first.push(m);
        m += r.len();
        of.extend(std::iter::repeat_n(given[ci], r.len()));
        r0.push(r);
        for id in c.points() {
            let vars = idx.get(&id).map(|&i| [2 * i, 2 * i + 1]).into_iter().flatten().chain(ridx.get(&id).copied());
            for k in vars {
                if holding[k].last() != Some(&ci) {
                    holding[k].push(ci);
                }
            }
        }
    }
    let h = 1e-6;
    let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m];
    let mut xp = x.clone();
    let mut rp = Vec::new();
    for k in 0..nv {
        xp[k] = x[k] + h;
        for &ci in &holding[k] {
            rp.clear();
            con_rows(&cons[ci], &xp, &x, &idx, &ridx, &anchor, &mut rp);
            for (t, (&after, &before)) in rp.iter().zip(&r0[ci]).enumerate() {
                let v = (after - before) / h;
                if v != 0.0 {
                    rows[first[ci] + t].push((k, v));
                }
            }
        }
        xp[k] = x[k];
    }
    // each row scaled to unit length, summed by the column as `normalize_rows` sums it: the rank must not depend on
    // the scale of the sketch or on constraint weights
    for row in rows.iter_mut() {
        let n = row.iter().map(|(_, v)| v * v).sum::<f64>().sqrt();
        if n > 1e-12 {
            for (_, v) in row.iter_mut() {
                *v /= n;
            }
        }
    }
    Differences { rows, of, nv }
}

/// THE PIVOT COLUMNS OF A SPARSE MATRIX, by the elimination of `pivot_columns` step for step: the columns in order,
/// in each the row of the largest value among the rows not yet taken - the first of them in the order the rows stand,
/// rows swapped as that elimination swaps them - taken when above 1e-7, and its column cleared from every row not yet
/// taken. The rows taken are not cleared further: they are never looked at again. Only the non-zero values are kept
/// and worked: an array of 1 000 rectangles is 8 000 columns of a few values each.
pub(crate) fn pivot_columns_sparse(rows: Vec<Vec<(usize, f64)>>, cols: usize) -> Vec<usize> {
    eliminate_sparse(rows, cols, Keep::Pivots).pivots
}

/// What an elimination keeps besides its pivot columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Keep {
    Pivots,
    /// and the rows left without a pivot as sums of the rows given (`Elimination::dependent`)
    Dependencies,
}

pub(crate) struct Elimination {
    pub(crate) pivots: Vec<usize>,
    /// the row of each pivot as it was taken, from its pivot column on, in the order of `pivots`
    pub(crate) echelon: Vec<Vec<(usize, f64)>>,
    /// each row no pivot was taken from, as the rows given summed into it with their factors: a sum of rows that is
    /// nothing, a dependency among them
    pub(crate) dependent: Vec<std::collections::BTreeMap<usize, f64>>,
}

/// `pivot_columns_sparse`, and with `Keep::Dependencies` the dependencies among the rows besides: the rows no pivot was
/// taken from - each brought to nothing - give one dependency each, the sum of the rows given that the row came to stand
/// for. They are as many as the rows less the rank, and one of them holds a row exactly where the row follows from the
/// others. The elimination writes down only what it took from each row - the row of the pivot and the factor - and the
/// sums are made afterwards for those rows and the pivots they reach alone: followed for every row as it went, the sums
/// grew with the fill, 42 ms of a count against 9 ms of the elimination on a sketch of 1 062 points on lines.
pub(crate) fn eliminate_sparse(rows: Vec<Vec<(usize, f64)>>, cols: usize, keep: Keep) -> Elimination {
    let n = rows.len();
    let mut a: Vec<std::collections::BTreeMap<usize, f64>> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    // what was taken from each row: the row of the pivot and its factor
    let mut taken: Vec<Vec<(usize, f64)>> = match keep {
        Keep::Pivots => Vec::new(),
        Keep::Dependencies => vec![Vec::new(); n],
    };
    let mut in_col: Vec<Vec<usize>> = vec![Vec::new(); cols];
    for (r, row) in a.iter().enumerate() {
        for &c in row.keys() {
            in_col[c].push(r);
        }
    }
    let mut at: Vec<usize> = (0..n).collect(); // the row standing at each place
    let mut place: Vec<usize> = (0..n).collect(); // the place of each row
    let mut pivots = Vec::new();
    let mut echelon = Vec::new();
    let mut row = 0usize;
    for col in 0..cols {
        if row >= n {
            break;
        }
        let value = |a: &[std::collections::BTreeMap<usize, f64>], r: usize| a[r].get(&col).copied().unwrap_or(0.0);
        // the first place from `row` on holding the largest value; places without a value in the column hold 0
        let mut piv = row;
        let mut best = value(&a, at[row]).abs();
        for &r in &in_col[col] {
            let (p, v) = (place[r], value(&a, r).abs());
            if p > row && (v > best || v == best && p < piv) {
                (piv, best) = (p, v);
            }
        }
        if best < 1e-7 {
            continue;
        }
        let (pr, rr) = (at[piv], at[row]);
        at.swap(row, piv);
        (place[pr], place[rr]) = (row, piv);
        let pivot: Vec<(usize, f64)> = a[pr].range(col..).map(|(&c, &v)| (c, v)).collect();
        let d = pivot[0].1;
        for &r in &in_col[col].clone() {
            if place[r] <= row {
                continue;
            }
            let f = value(&a, r) / d;
            if f == 0.0 {
                continue;
            }
            for &(c, v) in &pivot {
                let e = a[r].entry(c).or_insert_with(|| {
                    in_col[c].push(r);
                    0.0
                });
                *e -= f * v;
            }
            if let Some(t) = taken.get_mut(r) {
                t.push((pr, f));
            }
        }
        pivots.push(col);
        echelon.push(pivot);
        row += 1;
    }
    let dependent = match keep {
        Keep::Pivots => Vec::new(),
        Keep::Dependencies => sums_of(&at[row..], &at[..row], &taken),
    };
    Elimination { pivots, echelon, dependent }
}

/// The sums of the rows given that `rows` came to stand for, from what the elimination took from each (`taken`): a row is
/// itself less each factor times the sum of the pivot row it was taken by. The pivot rows reached are made first, in the
/// order they became pivots (`pivots`) - a pivot row was itself taken from by earlier pivots alone.
pub(crate) fn sums_of(rows: &[usize], pivots: &[usize], taken: &[Vec<(usize, f64)>]) -> Vec<std::collections::BTreeMap<usize, f64>> {
    let mut reached: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut stack: Vec<usize> = rows.iter().flat_map(|&r| taken[r].iter().map(|&(p, _)| p)).collect();
    while let Some(p) = stack.pop() {
        if reached.insert(p) {
            stack.extend(taken[p].iter().map(|&(q, _)| q));
        }
    }
    let mut made: std::collections::HashMap<usize, std::collections::BTreeMap<usize, f64>> = std::collections::HashMap::new();
    let make = |r: usize, made: &std::collections::HashMap<usize, std::collections::BTreeMap<usize, f64>>| {
        let mut s: std::collections::BTreeMap<usize, f64> = std::iter::once((r, 1.0)).collect();
        for &(p, f) in &taken[r] {
            for (&k, &v) in &made[&p] {
                *s.entry(k).or_insert(0.0) -= f * v;
            }
        }
        s
    };
    for &p in pivots.iter().filter(|p| reached.contains(p)) {
        let s = make(p, &made);
        made.insert(p, s);
    }
    rows.iter().map(|&r| make(r, &made)).collect()
}

/// A COORDINATE MOVES WHEN SOME MOTION THE CONSTRAINTS ALLOW MOVES IT: a vector of the null space of the Jacobian with a
/// share in it. Taken as "its column has no pivot", the freedom falls on the columns eliminated last: a rectangle drawn
/// from its centre, its centre fixed, shows one corner free and three held, though a width typed moves all four
/// (reported behaviour: "only one corner is yellow, the rest green"). A column with no pivot moves by itself; a pivot
/// column moves when the back substitution of the echelon rows gives it a share of a free column above 1e-6 - the
/// share is the value its row of the reduced echelon form holds in that column, which `free_points_whole` reads. The
/// free columns are carried as sparse sums, so the work is the non-zeros of the echelon times the shares that reach
/// each row.
pub(crate) fn movable(pivots: &[usize], echelon: &[Vec<(usize, f64)>], cols: usize) -> Vec<bool> {
    let mut is_pivot = vec![false; cols];
    for &c in pivots {
        is_pivot[c] = true;
    }
    let mut moved: Vec<bool> = is_pivot.iter().map(|p| !p).collect();
    let mut share: Vec<Option<HashMap<usize, f64>>> = (0..cols).map(|c| (!is_pivot[c]).then(|| std::iter::once((c, 1.0)).collect())).collect();
    for (&c, row) in pivots.iter().zip(echelon).rev() {
        let d = row[0].1;
        let mut sum: HashMap<usize, f64> = HashMap::new();
        for &(k, v) in &row[1..] {
            if let Some(of_k) = &share[k] {
                for (&f, &w) in of_k {
                    *sum.entry(f).or_insert(0.0) -= v * w / d;
                }
            }
        }
        sum.retain(|_, w| w.abs() > 1e-12);
        moved[c] = sum.values().any(|w| w.abs() > 1e-6);
        share[c] = Some(sum);
    }
    moved
}

/// `dof_whole` of a part, by `differences` and `pivot_columns_sparse`: the same count, at the cost of the non-zeros.
pub(crate) fn dof_sparse(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> (i32, i32) {
    if points.is_empty() {
        return (0, 0);
    }
    let Differences { rows, nv, .. } = differences(points, radii, constraints);
    let m = rows.len();
    if m == 0 {
        return (nv as i32, 0);
    }
    let rank = pivot_columns_sparse(rows, nv).len() as i32;
    (nv as i32 - rank, m as i32 - rank)
}

/// `free_points_whole` of a part, by `differences` and `pivot_columns_sparse`.
pub(crate) fn free_points_sparse(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<bool> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let Differences { rows, nv, .. } = differences(points, radii, constraints);
    if rows.is_empty() {
        return vec![true; n]; // no constraints at all, so everything is free
    }
    let Elimination { pivots, echelon, .. } = eliminate_sparse(rows, nv, Keep::Pivots);
    let moved = movable(&pivots, &echelon, nv);
    (0..n).map(|i| moved[2 * i] || moved[2 * i + 1]).collect()
}

/// The degrees of freedom of the whole sketch counted as one Jacobian, the way they were counted before the sketch
/// was split into parts (`dof`); the reference of the parts.
pub fn dof_whole(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> (i32, i32) {
    if points.is_empty() {
        return (0, 0);
    }
    let np = points.len();
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(j, rv)| (rv.center, np * 2 + j)).collect();
    let has = |id: Id| idx.contains_key(&id);
    let is_center = |id: Id| ridx.contains_key(&id);
    let cons: Vec<Constraint> = constraints.iter().filter(|&c| cons_ok(c, &has, &is_center)).cloned().collect();
    let nv = np * 2 + radii.len();
    let anchor: HashMap<Id, (f64, f64)> = cons
        .iter()
        .filter_map(|c| match *c {
            Constraint::Fixed { p } => idx.get(&p).map(|&i| (p, (points[i].x, points[i].y))),
            _ => None,
        })
        .collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|rv| rv.value));
    let r0 = residuals_of(&x, &x, &idx, &ridx, &anchor, &cons); // the side of an axis dimension is read off the current configuration
    let m = r0.len();
    if m == 0 {
        return (nv as i32, 0);
    }
    let h = 1e-6;
    let mut jac = vec![vec![0.0_f64; nv]; m];
    for k in 0..nv {
        let mut xp = x.clone();
        xp[k] += h;
        let rp = residuals_of(&xp, &x, &idx, &ridx, &anchor, &cons);
        for i in 0..m {
            jac[i][k] = (rp[i] - r0[i]) / h;
        }
    }
    normalize_rows(&mut jac);
    let rank = pivot_columns(&mut jac, nv).len() as i32;
    let dof = nv as i32 - rank;
    let redundant = m as i32 - rank;
    (dof, redundant)
}

/// Normalise the rows of the Jacobian to unit length before taking the rank. The pivot threshold (1e-7) is
/// absolute, while the magnitudes of the derivatives depend on the size of the sketch and on the constraint
/// weights — `Fixed` carries ×50, `Parallel` scales with length, tangencies sit near 1 — so without
/// normalisation the rank drifted, reporting false redundancy or under-constraint on large and on tiny parts.
/// Null rows, left by a degenerate constraint, are left alone: they are genuinely dependent.
pub(crate) fn normalize_rows(jac: &mut [Vec<f64>]) {
    for row in jac.iter_mut() {
        let n = row.iter().map(|v| v * v).sum::<f64>().sqrt();
        if n > 1e-12 {
            for v in row.iter_mut() {
                *v /= n;
            }
        }
    }
}

/// Which points can still move (`true`), used to highlight the under-constrained ones.
pub fn free_points(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<bool> {
    // part by part, as `dof`: a point of a part with no constraint is free
    let mut out = vec![true; points.len()];
    for part in parts(points, radii, constraints).iter().filter(|p| !p.constraints.is_empty()) {
        let own = part.own(points, radii, constraints);
        for (&i, f) in part.points.iter().zip(free_points_sparse(&own.points, &own.radii, &own.constraints)) {
            out[i] = f;
        }
    }
    out
}

/// THE DEGREES OF FREEDOM `constraints[k]` TAKES: those of its part without it less those with it - 0 for a
/// constraint that follows from the others. Counted in its own part alone: no other part changes its rank with it.
/// A constraint laid while drawing in a sketch of 10 000 lines counted the whole sketch twice.
pub fn freedom_taken(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], k: usize) -> i32 {
    let Some(PartOf { part, at }) = part_of(points, radii, constraints, k) else { return 0 };
    let own = part.own(points, radii, constraints);
    taken(&own, at)
}

/// THE SAME, judged where the constraints hold: on a copy of the part solved with them all, to the thresholds of the
/// whole sketch, as a solve of the sketch solves that part.
pub fn freedom_taken_where_solved(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], k: usize) -> i32 {
    let Some(PartOf { part, at }) = part_of(points, radii, constraints, k) else { return 0 };
    let mut own = part.own(points, radii, constraints);
    let how = Solve { scale: Scale::of(points), algebra: Algebra::BySize, until: None, sizes: Sizes::Free };
    guarded(&mut own.points, &mut own.radii, |points, radii| solve_full_iter_inner(points, radii, &own.constraints, None, 120, how));
    taken(&own, at)
}

/// The part that holds a constraint, and the place of the constraint among the constraints of the part.
pub(crate) struct PartOf {
    pub(crate) part: Part,
    pub(crate) at: usize,
}

pub(crate) fn part_of(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], k: usize) -> Option<PartOf> {
    parts(points, radii, constraints).into_iter().find_map(|part| {
        let at = part.constraints.iter().position(|&ci| ci == k)?;
        Some(PartOf { part, at })
    })
}

/// The degrees of freedom of a part without its constraint `at` less those with it.
pub(crate) fn taken(own: &Own, at: usize) -> i32 {
    let without: Vec<Constraint> = own.constraints.iter().enumerate().filter(|(t, _)| *t != at).map(|(_, c)| c.clone()).collect();
    dof_sparse(&own.points, &own.radii, &without).0 - dof_sparse(&own.points, &own.radii, &own.constraints).0
}

/// THE REDUNDANT CONSTRAINTS among the first `own` of `constraints` (the rest are those of the entities themselves, such
/// as the ends of an arc on its circle): each whose removal frees no degree of freedom. Counted part by part, and only in
/// a part with an excess: a removal changes the rank of its own part alone. Counted whole, every constraint took two
/// counts of the whole sketch - 49 s on 10 000 lines for one count.
pub fn redundant(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], own: usize) -> Vec<usize> {
    checks(points, radii, constraints, own).redundant
}

/// WHAT THE CHECKS OF A SKETCH COUNT: its degrees of freedom and its excess (`dof`), its free points (`free_points`)
/// and its redundant constraints (`redundant`).
#[derive(Clone, Debug, PartialEq)]
pub struct Checks {
    pub dof: (i32, i32),
    pub free: Vec<bool>,
    pub redundant: Vec<usize>,
}

/// THE CHECKS OF A SKETCH FROM ONE ELIMINATION A PART: the Jacobian of each part made once and eliminated once, its
/// rank the degrees of freedom, its pivot columns the free points, its dependencies where to look for the redundant -
/// the answers of `dof`, `free_points` and `redundant`, each of which made the part and eliminated it again: 9 ms, 9 ms
/// and 9 ms more of 54 on a sketch of 1 062 points on lines, on every change. Each constraint a dependency holds is then
/// counted without it, as `redundant` counted it: the dependencies only say where to look.
pub fn checks(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], own: usize) -> Checks {
    checks_remembered(points, radii, constraints, own, &mut PartMemo::default())
}

/// WHAT THE CHECKS REMEMBER OF EACH PART, by its print (`part_print`): a part whose points stand where they stood, held by
/// the same constraints, has the answers it had. A change touches a part or two; counted again, every part of a sketch
/// of 70 000 segments drawn as rectangles took 126 ms of checks and 99 ms of conflicts on each change, in a release build.
/// Each count keeps the parts it met and lets the others go.
#[derive(Default)]
pub struct PartMemo {
    pub(crate) checks: HashMap<u64, PartChecks>,
    pub(crate) conflicts: HashMap<u64, Vec<Eliminated>>,
}

/// The checks of one part: the constraints by their place in the part.
#[derive(Clone)]
pub(crate) struct PartChecks {
    pub(crate) free: i32,
    pub(crate) excess: i32,
    pub(crate) free_points: Vec<bool>,
    pub(crate) redundant: Vec<usize>,
}

/// THE PRINT OF A PART: its points with where they stand, its radii with their values, its constraints as they print,
/// each with whether it is among those counted for redundancy (`own`). The constraints are printed into one buffer and
/// mixed a word at a time: a string made and hashed for each was 32 ms of the 54 of a count of 17 500 rectangles.
pub(crate) fn part_print(part: &Part, points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], own: usize, buf: &mut String) -> u64 {
    use std::fmt::Write;
    let mut h: u64 = part.points.len() as u64;
    let mut mix = |x: u64| h = (h.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    for &i in &part.points {
        [points[i].id, points[i].x.to_bits(), points[i].y.to_bits()].into_iter().for_each(&mut mix);
    }
    for &j in &part.radii {
        [radii[j].center, radii[j].value.to_bits()].into_iter().for_each(&mut mix);
    }
    for &ci in &part.constraints {
        buf.clear();
        let _ = write!(buf, "{:?}", constraints[ci]);
        let bytes = buf.as_bytes();
        mix(bytes.len() as u64);
        for word in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..word.len()].copy_from_slice(word);
            mix(u64::from_le_bytes(w));
        }
        mix(u64::from(ci < own));
    }
    h
}

/// `checks`, each part taken from `memo` where its print is there, and `memo` left holding the parts of this count.
pub fn checks_remembered(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint], own: usize, memo: &mut PartMemo) -> Checks {
    let parts = parts(points, radii, constraints);
    let mut free_count = (radii.len() - parts.iter().map(|p| p.radii.len()).sum::<usize>()) as i32; // radii of no point
    let mut excess = 0;
    let mut free = vec![true; points.len()];
    let mut redundant = Vec::new();
    let mut kept: HashMap<u64, PartChecks> = HashMap::new();
    let mut buf = String::new();
    for part in &parts {
        if part.constraints.is_empty() {
            free_count += (part.points.len() * 2 + part.radii.len()) as i32;
            continue;
        }
        let print = part_print(part, points, radii, constraints, own, &mut buf);
        let answer = match memo.checks.remove(&print).or_else(|| kept.get(&print).cloned()) {
            Some(a) => a,
            None => part_checks(part, &part.own(points, radii, constraints), own),
        };
        free_count += answer.free;
        excess += answer.excess;
        for (&i, &f) in part.points.iter().zip(&answer.free_points) {
            free[i] = f;
        }
        redundant.extend(answer.redundant.iter().map(|&k| part.constraints[k]));
        kept.insert(print, answer);
    }
    memo.checks = kept;
    redundant.sort_unstable();
    Checks { dof: (free_count, excess), free, redundant }
}

/// The checks of one part with constraints.
pub(crate) fn part_checks(part: &Part, mine: &Own, own: usize) -> PartChecks {
    let count = counted(mine);
    let mut redundant = Vec::new();
    if count.excess > 0 {
        for (k, _) in part.constraints.iter().enumerate().filter(|(k, ci)| **ci < own && count.maybe.contains(k)) {
            let without: Vec<Constraint> = mine.constraints.iter().enumerate().filter(|(t, _)| *t != k).map(|(_, c)| c.clone()).collect();
            if dof_sparse(&mine.points, &mine.radii, &without).0 == count.free {
                redundant.push(k);
            }
        }
    }
    PartChecks { free: count.free, excess: count.excess, free_points: count.free_points, redundant }
}

/// What one elimination of a part tells.
pub(crate) struct Counted {
    /// the degrees of freedom of the part and its excess, as `dof_sparse` counts them
    pub(crate) free: i32,
    pub(crate) excess: i32,
    /// each point of the part free, as `free_points_sparse` finds it
    pub(crate) free_points: Vec<bool>,
    /// the constraints a dependency among the rows holds, by their place: a row that follows from the others stands in a
    /// dependency with a factor of its own, and a constraint none of whose rows does is needed whole. A factor down to
    /// 1e-9 counts: the count without the constraint gives the answer, the factor only says where to look
    pub(crate) maybe: std::collections::HashSet<usize>,
}

pub(crate) fn counted(own: &Own) -> Counted {
    let n = own.points.len();
    if n == 0 {
        return Counted { free: 0, excess: 0, free_points: Vec::new(), maybe: Default::default() };
    }
    let Differences { rows, of, nv } = differences(&own.points, &own.radii, &own.constraints);
    let m = rows.len();
    if m == 0 {
        return Counted { free: nv as i32, excess: 0, free_points: vec![true; n], maybe: Default::default() };
    }
    let Elimination { pivots, echelon, dependent } = eliminate_sparse(rows, nv, Keep::Dependencies);
    let rank = pivots.len() as i32;
    let moved = movable(&pivots, &echelon, nv);
    let biggest = |d: &std::collections::BTreeMap<usize, f64>| d.values().fold(0.0_f64, |m, v| m.max(v.abs()));
    Counted {
        free: nv as i32 - rank,
        excess: m as i32 - rank,
        free_points: (0..n).map(|i| moved[2 * i] || moved[2 * i + 1]).collect(),
        maybe: dependent.iter().flat_map(|d| d.iter().filter(|(_, v)| v.abs() > 1e-9 * biggest(d)).map(|(&r, _)| of[r])).collect(),
    }
}

/// The free points of the whole sketch from one Jacobian (`free_points`); the reference of the parts.
pub fn free_points_whole(points: &[SketchPoint], radii: &[RadiusVar], constraints: &[Constraint]) -> Vec<bool> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let idx: HashMap<Id, usize> = points.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let ridx: HashMap<Id, usize> = radii.iter().enumerate().map(|(j, rv)| (rv.center, n * 2 + j)).collect();
    let has = |id: Id| idx.contains_key(&id);
    let is_center = |id: Id| ridx.contains_key(&id);
    let cons: Vec<Constraint> = constraints.iter().filter(|&c| cons_ok(c, &has, &is_center)).cloned().collect();
    let nv = n * 2 + radii.len();
    let anchor: HashMap<Id, (f64, f64)> = cons
        .iter()
        .filter_map(|c| match *c {
            Constraint::Fixed { p } => idx.get(&p).map(|&i| (p, (points[i].x, points[i].y))),
            _ => None,
        })
        .collect();
    let mut x: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    x.extend(radii.iter().map(|rv| rv.value));
    let r0 = residuals_of(&x, &x, &idx, &ridx, &anchor, &cons); // the side of an axis dimension is read off the current configuration
    let m = r0.len();
    if m == 0 {
        return vec![true; n]; // no constraints at all, so everything is free
    }
    let h = 1e-6;
    let mut jac = vec![vec![0.0_f64; nv]; m];
    for k in 0..nv {
        let mut xp = x.clone();
        xp[k] += h;
        let rp = residuals_of(&xp, &x, &idx, &ridx, &anchor, &cons);
        for i in 0..m {
            jac[i][k] = (rp[i] - r0[i]) / h;
        }
    }
    normalize_rows(&mut jac); // the rank must not depend on the scale of the sketch or on constraint weights
                              // REDUCED: each row of a pivot holds its pivot and the free columns alone, its values there the shares of the free
                              // columns in the motion of its pivot column (`movable`)
    let pivots = pivot_columns(&mut jac, nv);
    let mut moved = vec![true; nv];
    for &c in &pivots {
        moved[c] = false;
    }
    let free: Vec<usize> = (0..nv).filter(|&c| moved[c]).collect();
    for (r, &c) in pivots.iter().enumerate() {
        moved[c] = free.iter().any(|&f| (jac[r][f] / jac[r][c]).abs() > 1e-6);
    }
    (0..n).map(|i| moved[2 * i] || moved[2 * i + 1]).collect()
}

/// Pivot columns of a rows×cols matrix, by Gaussian elimination with partial pivoting. The number of pivot
/// columns is the rank.
// THE INDEX IS THE MEANING: the same elimination as above, over the columns of the Jacobian.
#[allow(clippy::needless_range_loop)]
pub(crate) fn pivot_columns(a: &mut [Vec<f64>], cols: usize) -> Vec<usize> {
    let rows = a.len();
    let mut pivots = Vec::new();
    let mut row = 0usize;
    for col in 0..cols {
        if row >= rows {
            break;
        }
        // pivot element in the current column
        let mut piv = row;
        for r in row + 1..rows {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-7 {
            continue;
        }
        a.swap(row, piv);
        let d = a[row][col];
        for r in 0..rows {
            if r == row {
                continue;
            }
            let f = a[r][col] / d;
            if f != 0.0 {
                for c in col..cols {
                    a[r][c] -= f * a[row][c];
                }
            }
        }
        pivots.push(col);
        row += 1;
    }
    pivots
}
