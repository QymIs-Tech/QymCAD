//! A SKETCH SOLVED BY ITS PARTS COMES OUT AS SOLVED WHOLE: every kind of sketch (`sketch_kinds`) at sizes to ~300
//! unknowns is solved by `solver::solve_full_iter`, which solves each part that no constraint ties to the rest as a
//! system of its own, and by `solver::solve_full_iter_whole`, the whole sketch as one system as it was solved before.
//! The points, the radii and the residual agree. A drag frame moves the part of the dragged point as the whole solve
//! moves it, and leaves every other part where it stood. The failures are gathered and reported together.
mod sketch_kinds;

use qymcad_core::model::{Constraint, EntityKind, Id, Project, SketchPoint};
use qymcad_core::solver::{self, RadiusVar};
use sketch_kinds::{build, Kind, KINDS};

/// What the solver is given for a sketch: what `Project::solve_sketch` gives it for one of these sketches.
struct Input {
    points: Vec<SketchPoint>,
    radii: Vec<RadiusVar>,
    constraints: Vec<Constraint>,
}

fn input(p: &Project) -> Input {
    let s = &p.sketches[0];
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap_or_default();
    let mut radii = Vec::new();
    let mut constraints = s.constraints.clone();
    for e in &s.entities {
        match e.kind {
            EntityKind::Circle { center, r } => radii.push(RadiusVar { center, value: r }),
            EntityKind::Arc { center, a, b, .. } => {
                let (c, q) = (at(center), at(a));
                radii.push(RadiusVar { center, value: (q.0 - c.0).hypot(q.1 - c.1) });
                constraints.push(Constraint::PointOnCircle { p: a, c: center });
                constraints.push(Constraint::PointOnCircle { p: b, c: center });
            }
            _ => {}
        }
    }
    Input { points: s.points.clone(), radii, constraints }
}

/// The largest difference between two solved states, coordinates and radii alike.
fn apart(a: &Input, b: &Input) -> f64 {
    let points = a.points.iter().zip(&b.points).map(|(p, q)| (p.x - q.x).abs().max((p.y - q.y).abs()));
    let radii = a.radii.iter().zip(&b.radii).map(|(r, s)| (r.value - s.value).abs());
    points.chain(radii).fold(0.0, f64::max)
}

/// A solved state agrees with the reference to 1e-6 mm: the polish of a solved sketch reaches 1e-12, and the parts
/// and the whole stop their iterations at steps of their own size.
const AGREE: f64 = 1e-6;

/// The number of shapes of `kind` that makes about 300 unknowns: the dense whole solve costs their cube.
fn size_of(kind: Kind) -> usize {
    let unknowns = |n: usize| -> usize {
        let p = build(kind, n);
        p.sketches[0].points.len() * 2 + input(&p).radii.len()
    };
    let mut n = 1;
    while unknowns(n + 1) <= 300 {
        n += 1;
    }
    n
}

#[test]
fn a_sketch_solved_by_its_parts_comes_out_as_solved_whole() {
    let mut failures = Vec::new();
    for kind in KINDS {
        for n in [1, 3, size_of(kind)] {
            let p = build(kind, n);
            let (mut parts, mut whole) = (input(&p), input(&p));
            let r_parts = solver::solve_full_iter(&mut parts.points, &mut parts.radii, &parts.constraints, None, 120);
            let r_whole = solver::solve_full_iter_whole(&mut whole.points, &mut whole.radii, &whole.constraints, None, 120);
            let d = apart(&parts, &whole);
            if d > AGREE || (r_parts - r_whole).abs() > 1e-9 {
                failures.push(format!("{kind:?} x{n}, solve: {d:.2e} mm apart, residual {r_parts:.2e} against {r_whole:.2e}"));
            }
        }
    }
    assert!(failures.is_empty(), "the parts and the whole disagree:\n{}", failures.join("\n"));
}

#[test]
fn a_drag_moves_its_own_part_alone() {
    let mut failures = Vec::new();
    for kind in KINDS {
        for n in [1, 3, size_of(kind)] {
            let p = build(kind, n);
            let mut solved = input(&p);
            solver::solve_full_iter_whole(&mut solved.points, &mut solved.radii, &solved.constraints, None, 120);
            let dragged = solved.points[0];
            let drag = Some((dragged.id, dragged.x + 1.0, dragged.y + 1.0));
            let (mut parts, mut whole) = (Input { points: solved.points.clone(), radii: solved.radii.clone(), constraints: solved.constraints.clone() }, solved);
            let before = parts.points.clone();
            solver::solve_full_iter(&mut parts.points, &mut parts.radii, &parts.constraints, drag, 40);
            solver::solve_full_iter_whole(&mut whole.points, &mut whole.radii, &whole.constraints, drag, 40);
            // the part of the dragged point: the points the drag moved in the whole solve
            let moved: Vec<bool> = before.iter().zip(&whole.points).map(|(b, w)| (b.x - w.x).abs().max((b.y - w.y).abs()) > 1e-12).collect();
            if !moved[0] {
                failures.push(format!("{kind:?} x{n}: the whole solve left the dragged point where it stood"));
            }
            for (i, ((b, q), w)) in before.iter().zip(&parts.points).zip(&whole.points).enumerate() {
                let (own, still) = ((q.x - w.x).abs().max((q.y - w.y).abs()), (q.x - b.x).abs().max((q.y - b.y).abs()));
                if moved[i] && own > AGREE {
                    failures.push(format!("{kind:?} x{n}, drag: point {i} of the dragged part {own:.2e} mm from the whole solve"));
                }
                if !moved[i] && still > 0.0 {
                    failures.push(format!("{kind:?} x{n}, drag: point {i} of another part moved by {still:.2e} mm"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "a drag frame by parts disagrees:\n{}", failures.join("\n"));
}

#[test]
fn the_sparse_algebra_solves_as_the_dense() {
    // every part, whatever its size, by the sparse Cholesky, against the dense Gauss-Jordan of the whole
    let mut failures = Vec::new();
    for kind in KINDS {
        for n in [1, 3, size_of(kind)] {
            let p = build(kind, n);
            let (mut sparse, mut dense) = (input(&p), input(&p));
            let r_sparse = solver::solve_full_iter_sparse(&mut sparse.points, &mut sparse.radii, &sparse.constraints, None, 120);
            let r_dense = solver::solve_full_iter_whole(&mut dense.points, &mut dense.radii, &dense.constraints, None, 120);
            let d = apart(&sparse, &dense);
            if d > AGREE || (r_sparse - r_dense).abs() > 1e-9 {
                failures.push(format!("{kind:?} x{n}: {d:.2e} mm apart, residual {r_sparse:.2e} against {r_dense:.2e}"));
            }
        }
    }
    assert!(failures.is_empty(), "the sparse and the dense algebra disagree:\n{}", failures.join("\n"));
}

/// The same sketch made to argue: every third constraint laid twice (redundant), and the first that can be contradicted
/// contradicted - a
/// Horizontal by a vertical dimension of 5 mm on the same points (a Vertical would not: both hold where the points
/// meet), a dimension by the same one 5 mm longer.
fn arguing(mut i: Input) -> Input {
    let had = i.constraints.clone();
    i.constraints.extend(had.iter().step_by(3).cloned());
    let against = had.iter().find_map(|c| match c {
        Constraint::Horizontal { a, b } => Some(Constraint::Distance { dimension: Default::default(), a: *a, b: *b, d: 5.0, off: 2.0, expr: String::new(), driven: false, axis: 2, at: None }),
        Constraint::Distance { a, b, d, axis, .. } => {
            Some(Constraint::Distance { dimension: Default::default(), a: *a, b: *b, d: d + 5.0, off: 2.0, expr: String::new(), driven: false, axis: *axis, at: None })
        }
        Constraint::Diameter { c, d, .. } => Some(Constraint::Diameter { dimension: Default::default(), c: *c, d: d + 5.0, off: 0.0, expr: String::new(), driven: false, diam: false, at: None }),
        _ => None,
    });
    i.constraints.extend(against);
    i
}

/// The redundant constraints of the whole sketch as they were counted before the parts: each of the first `own` whose
/// removal leaves the degrees of freedom of the whole as they are, while the sketch has an excess at all.
fn redundant_whole(i: &Input, own: usize) -> Vec<usize> {
    let (free_all, excess) = solver::dof_whole(&i.points, &i.radii, &i.constraints);
    if excess <= 0 {
        return Vec::new();
    }
    (0..own)
        .filter(|&k| {
            let without: Vec<Constraint> = i.constraints.iter().enumerate().filter(|(t, _)| *t != k).map(|(_, c)| c.clone()).collect();
            solver::dof_whole(&i.points, &i.radii, &without).0 == free_all
        })
        .collect()
}

#[test]
fn the_diagnostics_of_the_parts_are_those_of_the_whole() {
    let mut failures = Vec::new();
    for kind in KINDS {
        for n in [1, 3, size_of(kind)] {
            for (how, i) in [("plain", input(&build(kind, n))), ("arguing", arguing(input(&build(kind, n))))] {
                let case = format!("{kind:?} x{n}, {how}");
                let (dof, dof_whole) = (solver::dof(&i.points, &i.radii, &i.constraints), solver::dof_whole(&i.points, &i.radii, &i.constraints));
                if dof != dof_whole {
                    failures.push(format!("{case}: degrees of freedom {dof:?} against {dof_whole:?}"));
                }
                if solver::free_points(&i.points, &i.radii, &i.constraints) != solver::free_points_whole(&i.points, &i.radii, &i.constraints) {
                    failures.push(format!("{case}: the free points differ"));
                }
                let (c, c_whole) = (solver::conflicts(&i.points, &i.radii, &i.constraints), solver::conflicts_whole(&i.points, &i.radii, &i.constraints));
                if c != c_whole {
                    failures.push(format!("{case}: conflicts {c:?} against {c_whole:?}"));
                }
                let own = i.constraints.len();
                let (r, r_whole) = (solver::redundant(&i.points, &i.radii, &i.constraints, own), redundant_whole(&i, own));
                if r != r_whole {
                    failures.push(format!("{case}: redundant {r:?} against {r_whole:?}"));
                }
                // the checks counted together, from one elimination a part, are the ones counted apart
                let together = solver::checks(&i.points, &i.radii, &i.constraints, own);
                let apart = solver::Checks { dof, free: solver::free_points(&i.points, &i.radii, &i.constraints), redundant: r };
                if together != apart {
                    failures.push(format!("{case}: the checks together {:?} {:?} against {:?} {:?}", together.dof, together.redundant, apart.dof, apart.redundant));
                }
                if how == "arguing" && c_whole.is_empty() && kind != Kind::Mixed {
                    failures.push(format!("{case}: the contradiction made no conflict - the check checks nothing"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "the diagnostics of the parts and of the whole disagree:\n{}", failures.join("\n"));
}

#[test]
fn the_freedom_a_constraint_takes_is_counted_in_its_part_as_in_the_whole() {
    let mut failures = Vec::new();
    for kind in KINDS {
        for (how, i) in [("plain", input(&build(kind, 3))), ("arguing", arguing(input(&build(kind, 3))))] {
            let all = solver::dof_whole(&i.points, &i.radii, &i.constraints).0;
            // and the same, the sketch solved first with every constraint, as a constraint laid while drawing is judged
            let mut solved = Input { points: i.points.clone(), radii: i.radii.clone(), constraints: i.constraints.clone() };
            solver::solve_full_iter(&mut solved.points, &mut solved.radii, &solved.constraints, None, 120);
            let all_solved = solver::dof_whole(&solved.points, &solved.radii, &solved.constraints).0;
            for k in 0..i.constraints.len() {
                let without: Vec<Constraint> = i.constraints.iter().enumerate().filter(|(t, _)| *t != k).map(|(_, c)| c.clone()).collect();
                let whole = solver::dof_whole(&i.points, &i.radii, &without).0 - all;
                let part = solver::freedom_taken(&i.points, &i.radii, &i.constraints, k);
                if part != whole {
                    failures.push(format!("{kind:?}, {how}, constraint {k}: takes {part} in its part, {whole} in the whole"));
                }
                let whole_solved = solver::dof_whole(&solved.points, &solved.radii, &without).0 - all_solved;
                let part_solved = solver::freedom_taken_where_solved(&i.points, &i.radii, &i.constraints, k);
                if part_solved != whole_solved {
                    failures.push(format!("{kind:?}, {how}, constraint {k} where solved: takes {part_solved} in its part, {whole_solved} in the whole"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "the freedom a constraint takes disagrees:\n{}", failures.join("\n"));
}

/// The first dimension of a sketch made 1 mm longer, or the first Horizontal turned Vertical where there is no
/// dimension: an edit of one shape.
fn edited(mut i: Input) -> Input {
    let at = i.constraints.iter().position(|c| matches!(c, Constraint::Distance { .. } | Constraint::Diameter { .. } | Constraint::Horizontal { .. }));
    match at.map(|k| &mut i.constraints[k]) {
        Some(Constraint::Distance { d, .. } | Constraint::Diameter { d, .. }) => *d += 1.0,
        Some(c @ Constraint::Horizontal { .. }) => {
            if let Constraint::Horizontal { a, b } = *c {
                *c = Constraint::Vertical { a, b };
            }
        }
        _ => {}
    }
    i
}

#[test]
fn a_solved_sketch_edited_in_one_shape_is_solved_as_the_whole() {
    // the parts that hold are passed over; the answer is that of the whole solve all the same
    let mut failures = Vec::new();
    for kind in KINDS {
        for n in [3, size_of(kind)] {
            let mut solved = input(&build(kind, n));
            solver::solve_full_iter_whole(&mut solved.points, &mut solved.radii, &solved.constraints, None, 120);
            let (mut parts, mut whole) = (edited(Input { points: solved.points.clone(), radii: solved.radii.clone(), constraints: solved.constraints.clone() }), edited(solved));
            let r_parts = solver::solve_full_iter(&mut parts.points, &mut parts.radii, &parts.constraints, None, 120);
            let r_whole = solver::solve_full_iter_whole(&mut whole.points, &mut whole.radii, &whole.constraints, None, 120);
            let d = apart(&parts, &whole);
            if d > AGREE || (r_parts - r_whole).abs() > 1e-9 {
                failures.push(format!("{kind:?} x{n}, one shape edited: {d:.2e} mm apart, residual {r_parts:.2e} against {r_whole:.2e}"));
            }
        }
    }
    assert!(failures.is_empty(), "the parts and the whole disagree:\n{}", failures.join("\n"));
}

/// A fixed pseudo-random sequence in 0..n.
struct Seq(u64);

impl Seq {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// A random chain of `n` points, one part: between neighbours and between points a few apart, Horizontal, Vertical,
/// Coincident, Equal and dimensions, some of the dimensions laid twice at different lengths.
fn random_chain(seed: u64, n: usize) -> Input {
    let mut s = Seq(seed);
    let points: Vec<SketchPoint> = (0..n).map(|k| SketchPoint { id: 1_000 + k as u64, x: (s.next(400) as f64) * 0.25, y: (s.next(400) as f64) * 0.25 }).collect();
    let id = |k: usize| points[k].id;
    let mut constraints = Vec::new();
    for k in 1..n {
        let back = 1 + s.next(4.min(k as u64)) as usize;
        let (a, b) = (id(k - back), id(k));
        let d = 5.0 + s.next(10) as f64;
        constraints.push(match s.next(6) {
            0 => Constraint::Horizontal { a, b },
            1 => Constraint::Vertical { a, b },
            2 => Constraint::Equal { a: id(0), b: id(1), c: a, d: b },
            3 => Constraint::Coincident { a, b },
            _ => Constraint::Distance { dimension: Default::default(), a, b, d, off: 2.0, expr: String::new(), driven: false, axis: s.next(3) as u8, at: None },
        });
        if s.next(5) == 0 {
            constraints.push(Constraint::Distance { dimension: Default::default(), a, b, d: d + 3.0, off: 2.0, expr: String::new(), driven: false, axis: 0, at: None });
        }
    }
    Input { points, radii: Vec::new(), constraints }
}

#[test]
fn the_diagnostics_of_a_random_chain_are_those_of_the_whole() {
    let mut failures = Vec::new();
    let mut with_conflicts = 0;
    for seed in 0..60 {
        let i = random_chain(seed, 40 + (seed as usize % 5) * 20);
        let (c, c_whole) = (solver::conflicts(&i.points, &i.radii, &i.constraints), solver::conflicts_whole(&i.points, &i.radii, &i.constraints));
        with_conflicts += usize::from(!c_whole.is_empty());
        if c != c_whole {
            failures.push(format!("seed {seed}: conflicts {c:?} against {c_whole:?}"));
        }
        if solver::dof(&i.points, &i.radii, &i.constraints) != solver::dof_whole(&i.points, &i.radii, &i.constraints) {
            failures.push(format!("seed {seed}: the degrees of freedom differ"));
        }
        if solver::free_points(&i.points, &i.radii, &i.constraints) != solver::free_points_whole(&i.points, &i.radii, &i.constraints) {
            failures.push(format!("seed {seed}: the free points differ"));
        }
    }
    assert!(with_conflicts > 20, "the chains argue too seldom to say anything: {with_conflicts} of 60");
    assert!(failures.is_empty(), "the sparse diagnostics and the whole disagree:\n{}", failures.join("\n"));
}

#[test]
fn the_checks_remembered_through_edits_are_those_counted_anew() {
    // one memory carried through a run of edits - a point moved, a constraint laid twice, one taken out - and after each
    // the checks and the conflicts it gives are those of a count with nothing remembered
    let mut failures = Vec::new();
    for kind in KINDS {
        let mut i = input(&build(kind, size_of(kind)));
        let mut memo = solver::PartMemo::default();
        let edit = |i: &mut Input, step: usize| match step {
            0 => {}
            1 => i.points[0].x += 0.7,
            2 => {
                let c = i.constraints[0].clone();
                i.constraints.push(c);
            }
            3 => {
                let _ = i.constraints.pop();
            }
            _ => {
                if let Some(last) = i.points.last_mut() {
                    last.y -= 0.3;
                }
            }
        };
        for step in 0..5 {
            edit(&mut i, step);
            let own = i.constraints.len();
            let (kept, anew) = (solver::checks_remembered(&i.points, &i.radii, &i.constraints, own, &mut memo), solver::checks(&i.points, &i.radii, &i.constraints, own));
            if kept != anew {
                failures.push(format!("{kind:?}, edit {step}: the checks remembered {:?} {:?} against {:?} {:?}", kept.dof, kept.redundant, anew.dof, anew.redundant));
            }
            let (kept, anew) = (solver::conflicts_remembered(&i.points, &i.radii, &i.constraints, &mut memo), solver::conflicts(&i.points, &i.radii, &i.constraints));
            if kept != anew {
                failures.push(format!("{kind:?}, edit {step}: the conflicts remembered {kept:?} against {anew:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "the checks remembered and counted anew disagree:\n{}", failures.join("\n"));
}
