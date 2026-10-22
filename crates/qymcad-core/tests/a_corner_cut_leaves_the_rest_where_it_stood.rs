//! A CORNER CUT KEEPS WHAT A LENGTH SAID: two sides held equal were equal from corner to corner, and a chamfer or a
//! fillet on one of their corners leaves them so - the equality stays on the virtual sharp, not on the shortened piece.
//!
//! Reported behaviour: a set of three chamfers on a triangle with two equal sides moved the drawing tied to it, and a
//! rounded triangle 200 mm away, tied through the same polygon, was found shrunk to a point - its sides of 58 mm went
//! to 0. The equality had been carried onto the piece the cut left, 55 mm against 58, and the solver bent everything
//! tied to the triangle to make them equal.
use qymcad_core::feature::{ChamferMode, Purpose};
use qymcad_core::model::{ChamferLegs, Constraint, Project};

/// A triangle of two equal sides, held equal: the base from (0, 0) to (60, 0), the apex at (30, 50).
fn isosceles() -> (Project, usize, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let c = [(0.0, 0.0), (60.0, 0.0), (30.0, 50.0)];
    let sides: Vec<u64> = (0..3).map(|i| p.add_line_entity(si, c[i].0, c[i].1, c[(i + 1) % 3].0, c[(i + 1) % 3].1, Purpose::Real)).collect();
    p.regen_sketch(si);
    let ends = |p: &Project, l: u64| match p.sketches[si].entities.iter().find(|e| e.id == l).and_then(|e| e.kind.as_line()) {
        Some(qymcad_core::model::LineParts { a, b }) => (a, b),
        _ => panic!("the side {l} is a line"),
    };
    let ((a, b), (cc, d)) = (ends(&p, sides[1]), ends(&p, sides[2]));
    p.sketches[si].constraints.push(Constraint::Equal { a, b, c: cc, d });
    p.solve_sketch(si);
    (p, si, sides)
}

/// Where the two corners of the base stand, which the cut at the apex does not touch.
fn base(p: &Project, si: usize) -> Vec<(f64, f64)> {
    let mut v: Vec<(f64, f64)> = p.sketches[si].points.iter().filter(|q| q.y.abs() < 1e-6).map(|q| (q.x, q.y)).collect();
    v.sort_by(|u, w| u.0.total_cmp(&w.0));
    v
}

fn same_base(before: &[(f64, f64)], after: &[(f64, f64)]) -> bool {
    before.len() == after.len() && before.iter().zip(after).all(|(u, w)| (u.0 - w.0).hypot(u.1 - w.1) < 1e-6)
}

#[test]
fn a_chamfer_at_the_apex_leaves_the_equal_sides_equal_from_corner_to_corner() {
    let (mut p, si, sides) = isosceles();
    let stood = base(&p, si);
    let legs = ChamferLegs { mode: ChamferMode::TwoDist, first: 3.0, second: 8.0 };
    assert!(p.chamfer_lines_of_pair(si, (sides[1], sides[2]), legs, None), "the apex is chamfered");
    let now = base(&p, si);
    assert!(same_base(&stood, &now), "a chamfer of 3 and 8 at the apex moved the base: {stood:?} -> {now:?}");
    let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
    assert!(worst < 1e-6 && p.sketch_conflicts(si).is_empty(), "the triangle still solves after the chamfer: worst residual {worst:.2e}");
}

#[test]
fn a_fillet_at_a_base_corner_leaves_the_equal_sides_equal_from_corner_to_corner() {
    let (mut p, si, sides) = isosceles();
    let stood = base(&p, si);
    assert!(p.fillet_at_pair(si, (sides[0], sides[1]), 5.0), "the corner at (60, 0) is rounded");
    let apex = p.sketches[si].points.iter().find(|q| (q.x - 30.0).abs() < 1e-6 && (q.y - 50.0).abs() < 1e-6).is_some();
    let left = p.sketches[si].points.iter().find(|q| q.x.abs() < 1e-6 && q.y.abs() < 1e-6).is_some();
    let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
    assert!(apex && left, "the corners the fillet did not touch stand where they stood: apex {apex}, (0, 0) {left}; base before {stood:?}");
    assert!(worst < 1e-6 && p.sketch_conflicts(si).is_empty(), "the triangle still solves after the fillet: worst residual {worst:.2e}");
}
