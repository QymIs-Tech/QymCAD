//! THE SPEED OF A SKETCH, MEASURED: every kind of sketch at growing sizes, every operation a person causes on it, timed
//! and printed as a table "kind, size -> operation -> time". The measure the work on issue #95 is checked against: the
//! numbers before and after each step of the plan.
//!
//! The sketches are those of `sketch_kinds`.
//!
//! At the release level (`QYMCAD_TIER=release`) the sizes grow from 10 to 70 000 while an operation stays under
//! `CEILING`; past it the operation is not timed at the bigger sizes, and the table says so. At any other level only the
//! sizes to 300 are run - a smoke of the harness, not a measure.
mod sketch_kinds;

use qymcad_core::model::{Constraint, EntityKind, Project, SketchPoint};
use sketch_kinds::{build, Kind, KINDS};
use std::time::{Duration, Instant};

/// Past this an operation is not timed at bigger sizes: the next size would take minutes or hours.
const CEILING: Duration = Duration::from_secs(10);

/// The sizes of a sketch, in shapes of its kind.
const SIZES: [usize; 9] = [10, 30, 100, 300, 1_000, 3_000, 10_000, 30_000, 70_000];

/// An operation a person causes on a sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Solve,
    Drag,
    AddConstraint,
    AddDimension,
    Delete,
    Dof,
    FreePoints,
    Conflicts,
    Redundant,
    Regen,
}

const OPS: [Op; 10] = [Op::Solve, Op::Drag, Op::AddConstraint, Op::AddDimension, Op::Delete, Op::Dof, Op::FreePoints, Op::Conflicts, Op::Redundant, Op::Regen];

/// The first point of the sketch that a shape stands on, to drag and to tie a new constraint to.
fn a_point(p: &Project, si: usize) -> Option<SketchPoint> {
    let s = &p.sketches[si];
    let id = s.entities.first().map(|e| match e.kind {
        EntityKind::Line { a, .. } | EntityKind::Arc { a, .. } => a,
        EntityKind::Circle { center, .. } => center,
        EntityKind::Ellipse { c, .. } => c,
    })?;
    s.points.iter().find(|q| q.id == id).copied()
}

/// Time `op` once on a fresh sketch of `n` shapes of `kind`.
fn time(kind: Kind, n: usize, op: Op) -> Duration {
    let mut p = build(kind, n);
    let si = 0;
    let started = Instant::now();
    match op {
        Op::Solve => {
            let _ = p.solve_sketch(si);
        }
        Op::Drag => {
            if let Some(q) = a_point(&p, si) {
                let _ = p.solve_sketch_drag_fast(si, Some((q.id, q.x + 1.0, q.y + 1.0)));
            }
        }
        Op::AddConstraint => {
            if let Some(SketchPoint { id: a, .. }) = a_point(&p, si) {
                let b = p.sketches[si].points.iter().rev().find(|q| q.id != a).map(|q| q.id).unwrap_or(a);
                let _ = p.add_constraint_if_independent(si, Constraint::Vertical { a, b });
            }
        }
        Op::AddDimension => {
            if let Some(SketchPoint { id: a, .. }) = a_point(&p, si) {
                let b = p.sketches[si].points.iter().rev().find(|q| q.id != a).map(|q| q.id).unwrap_or(a);
                let _ = p.add_constraint_if_independent(si, Constraint::Distance { dimension: Default::default(), a, b, d: 50.0, off: 2.0, expr: String::new(), driven: false, axis: 0, at: None });
            }
        }
        Op::Delete => {
            if let Some(e) = p.sketches[si].entities.first().map(|e| e.id) {
                p.delete_entities(si, &[e]);
            }
        }
        Op::Dof => drop(p.sketch_dof(si)),
        Op::FreePoints => drop(p.sketch_free_points(si)),
        Op::Conflicts => drop(p.sketch_conflicts(si)),
        Op::Redundant => drop(p.sketch_redundant_constraints(si)),
        Op::Regen => p.regen_sketch(si),
    }
    started.elapsed()
}

#[test]
fn the_speed_of_a_sketch_by_kind_size_and_operation() {
    let release = std::env::var("QYMCAD_TIER").as_deref() == Ok("release");
    let sizes: Vec<usize> = SIZES.iter().copied().filter(|&n| release || n <= 300).collect();
    let mut table = String::from("kind        size   | ");
    for op in OPS {
        table.push_str(&format!("{:>13} ", format!("{op:?}")));
    }
    table.push('\n');
    for kind in KINDS {
        let mut stopped = [false; OPS.len()];
        for &n in &sizes {
            table.push_str(&format!("{:<10} {n:>6}   | ", format!("{kind:?}")));
            for (k, op) in OPS.into_iter().enumerate() {
                if stopped[k] {
                    table.push_str(&format!("{:>13} ", "-"));
                    continue;
                }
                let t = time(kind, n, op);
                stopped[k] = t > CEILING;
                table.push_str(&format!("{:>13} ", format!("{:.3} s", t.as_secs_f64())));
            }
            table.push('\n');
        }
    }
    eprintln!("THE SPEED OF A SKETCH (\"-\": past {} s at a smaller size, not timed)\n{table}", CEILING.as_secs());
}
