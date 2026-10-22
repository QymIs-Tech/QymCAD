//! A SKETCH CHAMFER IS GIVEN THREE WAYS: by the length of a symmetric cut, by two legs, or by a leg and an angle - the
//! first leg along the line the corner was clicked nearer to. It keeps its size as dimensions - the cut on its two ends,
//! the legs from the sharp corner - so a dimension changed afterwards moves the cut. A size the corner cannot take is
//! refused.
//!
//! Reported (issue #35): the sketch chamfer took one distance only; a chamfer of 5 x 3, or of 5 at 30 deg, had to be built
//! by hand from lines and dimensions.
use qymcad_core::feature::{ChamferMode, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferLegs, Constraint, Project};

/// The square corner (30, 0) -> (0, 0) -> (0, 30), held as a drawing holds it - the one line horizontal, the other
/// upright, the far ends pinned - so that a dimension of the chamfer changed afterwards moves the chamfer and not the
/// corner; (project, sketch, the corner point).
fn corner() -> (Project, usize, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let l1 = p.add_line_entity(si, 30.0, 0.0, 0.0, 0.0, Purpose::Real);
    let l2 = p.add_line_entity(si, 0.0, 0.0, 0.0, 30.0, Purpose::Real);
    let ends = |p: &Project, id| match p.sketches[si].entities.iter().find(|e| e.id == id).and_then(|e| e.kind.as_line()) {
        Some(qymcad_core::model::LineParts { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    let ((a1, b1), (a2, b2)) = (ends(&p, l1), ends(&p, l2));
    p.sketches[si].constraints.extend([Constraint::Horizontal { a: a1, b: b1 }, Constraint::Vertical { a: a2, b: b2 }, Constraint::Fixed { p: a1 }, Constraint::Fixed { p: b2 }]);
    p.solve_sketch(si);
    (p, si, b1)
}

/// Is there a point of the sketch at (x, y)?
fn has_point(p: &Project, si: usize, x: f64, y: f64) -> bool {
    p.sketches[si].points.iter().any(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6)
}

/// Set the value of the `n`-th dimension of the chamfer (a distance or an angle, in the order they were laid) and solve.
fn set_dim(p: &mut Project, si: usize, n: usize, v: f64) {
    let dims: Vec<usize> = p.sketches[si].constraints.iter().enumerate().filter(|(_, c)| matches!(c, Constraint::Distance { .. } | Constraint::AngleLines { .. })).map(|(i, _)| i).collect();
    match &mut p.sketches[si].constraints[dims[n]] {
        Constraint::Distance { d, .. } => *d = v,
        Constraint::AngleLines { deg, .. } => *deg = v,
        _ => unreachable!(),
    }
    let resid = p.solve_sketch(si);
    assert!(resid < 1e-6, "the dimension changed to {v} left the sketch unsolved: residual {resid:e}");
}

/// A chamfer at the corner, the click near the horizontal line or near the upright one.
fn chamfered(legs: ChamferLegs, near_horizontal: bool) -> Option<(Project, usize)> {
    let (mut p, si, pc) = corner();
    let toward = if near_horizontal { Point2::new(10.0, 1.0) } else { Point2::new(1.0, 10.0) };
    p.chamfer_at_vertex(si, pc, legs, Some(toward)).then_some((p, si))
}

#[test]
fn a_chamfer_by_two_legs_or_a_leg_and_an_angle() {
    let mut sins = Vec::new();
    let two = |first: f64, second: f64| ChamferLegs { mode: ChamferMode::TwoDist, first, second };

    // a symmetric cut of 5 on a square corner: legs of 5 / sqrt(2); its dimension changed to 7 keeps it symmetric
    let leg = |cut: f64| cut / std::f64::consts::SQRT_2;
    match chamfered(ChamferLegs::equal(5.0), true) {
        Some((mut p, si)) => {
            if !has_point(&p, si, leg(5.0), 0.0) || !has_point(&p, si, 0.0, leg(5.0)) {
                sins.push(format!("a symmetric cut of 5 does not cut {:.4} from both lines", leg(5.0)));
            }
            set_dim(&mut p, si, 0, 7.0);
            if !has_point(&p, si, leg(7.0), 0.0) || !has_point(&p, si, 0.0, leg(7.0)) {
                sins.push(format!("a symmetric cut: the dimension changed to 7 did not move both ends to {:.4}", leg(7.0)));
            }
        }
        None => sins.push("a symmetric cut of 5 refused".to_string()),
    }

    // two legs, 5 along the line clicked nearer to and 3 along the other; the second changed to 4 moves its end
    match chamfered(two(5.0, 3.0), true) {
        Some((mut p, si)) => {
            if !has_point(&p, si, 5.0, 0.0) || !has_point(&p, si, 0.0, 3.0) {
                sins.push("two legs, clicked near the horizontal line: not 5 along it and 3 up".to_string());
            }
            set_dim(&mut p, si, 1, 4.0);
            if !has_point(&p, si, 0.0, 4.0) || !has_point(&p, si, 5.0, 0.0) {
                sins.push("two legs: the second dimension changed to 4 did not move its end alone".to_string());
            }
        }
        None => sins.push("two legs of 5 and 3 refused".to_string()),
    }
    match chamfered(two(5.0, 3.0), false) {
        Some((p, si)) if has_point(&p, si, 0.0, 5.0) && has_point(&p, si, 3.0, 0.0) => {}
        _ => sins.push("two legs, clicked near the upright line: not 5 up it and 3 along the other".to_string()),
    }

    // a leg of 5 at 30 deg from the first line: on a square corner the second leg is 5 tan 30 = 2.887
    let second = 5.0 * 30f64.to_radians().tan();
    match chamfered(ChamferLegs { mode: ChamferMode::DistAngle, first: 5.0, second: 30.0 }, true) {
        Some((mut p, si)) => {
            if !has_point(&p, si, 5.0, 0.0) || !has_point(&p, si, 0.0, second) {
                sins.push(format!("a leg of 5 at 30 deg: not 5 along the line and {second:.3} up"));
            }
            set_dim(&mut p, si, 1, 45.0);
            if !has_point(&p, si, 0.0, 5.0) {
                sins.push("a leg and an angle: the angle changed to 45 deg did not bring the other end to 5".to_string());
            }
        }
        None => sins.push("a leg of 5 at 30 deg refused".to_string()),
    }

    // what the corner cannot take
    if chamfered(ChamferLegs { mode: ChamferMode::DistAngle, first: 5.0, second: 95.0 }, true).is_some() {
        sins.push("an angle of 95 deg on a square corner leaves no triangle and must be refused".to_string());
    }
    if chamfered(two(5.0, 40.0), true).is_some() {
        sins.push("a second leg of 40 on a line of 30 must be refused".to_string());
    }
    assert!(sins.is_empty(), "{}", sins.join("\n"));
}
