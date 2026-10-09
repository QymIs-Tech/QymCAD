//! A SOLVE OUT OF TIME SAYS SO, AND NOTHING BREAKS A SOLVE: the parts a solve had no time for stand as they stood, are
//! counted on the sketch, and the next solve takes them up; degenerate and contradictory sketches of every kind are
//! solved within the time of a solve, without a panic, and a shape that came out a non-number spoils no other.
mod sketch_kinds;

use qymcad_core::model::{Constraint, EntityKind, SketchPoint};
use qymcad_core::solver::Budget;
use sketch_kinds::{build, Kind, KINDS};
use std::time::{Duration, Instant};

#[test]
fn the_parts_a_solve_had_no_time_for_are_counted_and_solved_next() {
    let mut p = build(Kind::Tangents, 1_000);
    let before = p.sketches[0].points.clone();
    p.solve_sketch_within(0, Budget { time: Some(Duration::ZERO), ..Budget::FULL });
    assert_eq!(p.sketches[0].left_unsolved, 1_000, "a solve with no time left every part to the next");
    let moved = p.sketches[0].points.iter().zip(&before).filter(|(a, b)| a.x != b.x || a.y != b.y).count();
    assert_eq!(moved, 0, "a part the time ran out before was moved");
    let residual = p.solve_sketch(0);
    assert_eq!(p.sketches[0].left_unsolved, 0, "the next solve, with its time, left parts unsolved");
    assert!(residual < 1e-6, "the next solve did not solve them: residual {residual:e}");
}

/// The sketch of `kind` made degenerate: every other line drawn to a point, circles in pairs laid one on the other and
/// held tangent (not in a chain, which would tie the whole sketch into one part), and the first point standing at no
/// number.
fn degenerate(kind: Kind, n: usize) -> qymcad_core::model::Project {
    let mut p = build(kind, n);
    let s = &mut p.sketches[0];
    let at: std::collections::HashMap<u64, usize> = s.points.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
    let mut circles_before: Option<u64> = None;
    for (k, e) in s.entities.clone().iter().enumerate() {
        match e.kind {
            EntityKind::Line { a, b } if k % 2 == 0 => {
                let pa = s.points[at[&a]];
                s.points[at[&b]] = SketchPoint { id: b, ..pa };
            }
            EntityKind::Circle { center, .. } => {
                if let Some(c) = circles_before.take() {
                    let pc = s.points[at[&c]];
                    s.points[at[&center]] = SketchPoint { id: center, ..pc };
                    s.constraints.push(Constraint::CircleTangent { c1: c, c2: center, external: true });
                } else {
                    circles_before = Some(center);
                }
            }
            _ => {}
        }
    }
    if let Some(q) = s.points.first_mut() {
        q.x = f64::NAN;
    }
    p
}

#[test]
fn a_degenerate_sketch_is_solved_without_a_panic_and_keeps_its_other_shapes() {
    let mut failures = Vec::new();
    for kind in KINDS {
        let mut p = degenerate(kind, 300);
        let nan_id = p.sketches[0].points[0].id;
        let started = Instant::now();
        let residual = p.solve_sketch(0);
        let t = started.elapsed();
        if t > Duration::from_secs(10) {
            failures.push(format!("{kind:?}: {t:?}, past the 10 s of a solve"));
        }
        if !residual.is_infinite() {
            failures.push(format!("{kind:?}: a point at no number gave the residual {residual:e}, not the infinite of a part put back"));
        }
        let s = &p.sketches[0];
        let fine: Vec<&SketchPoint> = s.points.iter().filter(|q| q.id != nan_id).collect();
        if fine.iter().any(|q| !q.x.is_finite() || !q.y.is_finite()) {
            failures.push(format!("{kind:?}: a point other than the one at no number came out at no number"));
        }
        // the shapes away from the first are solved: most of their Horizontals hold (a circle laid on another and
        // held tangent outside it is a contradiction of its own, and keeps a compromise)
        let horizontal: Vec<bool> = s
            .constraints
            .iter()
            .filter_map(|c| match *c {
                Constraint::Horizontal { a, b } if a != nan_id && b != nan_id => {
                    let (pa, pb) = (s.points.iter().find(|q| q.id == a)?, s.points.iter().find(|q| q.id == b)?);
                    Some((pa.y - pb.y).abs() < 1e-6)
                }
                _ => None,
            })
            .collect();
        let held = horizontal.iter().filter(|h| **h).count();
        if held * 2 < horizontal.len() {
            failures.push(format!("{kind:?}: {held} of {} Horizontals away from the point at no number hold", horizontal.len()));
        }
    }
    assert!(failures.is_empty(), "degenerate sketches:\n{}", failures.join("\n"));
}

#[test]
fn a_contradicted_sketch_of_every_kind_is_solved_within_its_time() {
    for kind in KINDS {
        let mut p = build(kind, 300);
        let s = &mut p.sketches[0];
        // every Horizontal held against a vertical dimension of 5 mm, every dimension laid twice 5 mm apart
        let against: Vec<Constraint> = s
            .constraints
            .iter()
            .filter_map(|c| match *c {
                Constraint::Horizontal { a, b } => Some(Constraint::Distance { a, b, d: 5.0, off: 2.0, expr: String::new(), driven: false, axis: 2, at: None }),
                Constraint::Distance { a, b, d, axis, .. } => Some(Constraint::Distance { a, b, d: d + 5.0, off: 2.0, expr: String::new(), driven: false, axis, at: None }),
                _ => None,
            })
            .collect();
        s.constraints.extend(against);
        let started = Instant::now();
        let residual = p.solve_sketch(0);
        let t = started.elapsed();
        assert!(residual.is_finite(), "{kind:?}: a contradicted sketch gave the residual {residual}");
        assert!(t < Duration::from_secs(10), "{kind:?}: a contradicted sketch of 300 shapes took {t:?}, past the 10 s of a solve");
    }
}
