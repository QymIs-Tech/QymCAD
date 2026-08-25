//! A conflict and a residual behave the same at any scale.
//!
//! What used to be checked here was the heuristic of a dimension value disagreeing with the geometry, and it
//! called a single unsatisfied dimension a conflict. These are different questions and must not be confused:
//!
//! * a dimension not yet satisfied, the sketch being unsolved, is a residual, and a solve removes it;
//! * two dimensions contradicting each other are a conflict, which no solve cures: a constraint has to go.
//!
//! Both answers have to work the same on a part of hundredths of a millimetre and on a frame of metres.
use qymcad_core::model::{Constraint, Project};

fn line_with_dim(k: f64, d: f64, driven: bool) -> (Project, usize, u64, u64) {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let eid = p.add_line_entity(si, 0.0, 0.0, 10.0 * k, 0.0, qymcad_core::feature::Purpose::Real);
    let (a, b) = match p.sketches[si].entities.iter().find(|e| e.id == eid).unwrap().kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        _ => unreachable!(),
    };
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven, axis: 0 });
    (p, si, a, b)
}

/// Two contradictory dimensions are recognised as a conflict at any scale, and both are named.
#[test]
fn contradicting_dimensions_conflict_at_any_scale() {
    let mut bad = Vec::new();
    for k in [0.001, 1.0, 1000.0] {
        let (mut p, si, a, b) = line_with_dim(k, 10.0 * k, false);
        p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 12.0 * k, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        p.solve_sketch(si);
        let c = p.sketch_conflicts(si);
        if c.len() < 2 {
            bad.push(format!("scale ×{k}: two dimensions conflict, 10 and 12, and {c:?} were named"));
        }
    }
    assert!(bad.is_empty(), "the conflict is not recognised at every scale:\n{}", bad.join("\n"));
}

/// A single unsatisfied dimension is not a conflict: it shows as a residual and goes away after a solve.
#[test]
fn a_single_unsatisfied_dimension_is_residual_not_conflict() {
    for k in [0.001, 1.0, 1000.0] {
        let (mut p, si, _, _) = line_with_dim(k, 12.0 * k, false); // geometry at 10k with a dimension of 12k
        assert!(p.sketch_conflicts(si).is_empty(), "scale ×{k}: a single dimension conflicts with nothing");
        let before = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
        assert!(before > 0.5 * k, "scale ×{k}: before the solve the dimension is unsatisfied, residual {before:.3e}");
        p.solve_sketch(si);
        let after = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
        assert!(after < 1e-6 * k.max(1.0), "scale ×{k}: after the solve there is no residual ({after:.3e})");
    }
}

/// A consistent dimension goes red at no scale: a false alarm from numerical noise is a defect just the same.
#[test]
fn satisfied_dimension_is_never_flagged() {
    for k in [0.001, 1.0, 1000.0] {
        let (mut p, si, _, _) = line_with_dim(k, 10.0 * k, false);
        p.solve_sketch(si);
        assert!(p.sketch_conflicts(si).is_empty(), "scale ×{k}: a consistent dimension is not a conflict");
        let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
        assert!(worst < 1e-6 * k.max(1.0), "scale ×{k}: residual {worst:.3e}");
    }
}
