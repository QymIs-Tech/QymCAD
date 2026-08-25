//! Robustness of the sketch solver against unusable input.
use qymcad_core::model::{Constraint, SketchPoint};
use qymcad_core::solver;

fn pt(id: u64, x: f64, y: f64) -> SketchPoint { SketchPoint { id, x, y } }

/// The sketch solver lets no non-numbers out.
///
/// Once a NaN reaches the coordinates of a point it never leaves: it is saved into the document and spreads
/// into bodies, bounding boxes and the view — the sketch is broken for good, without a single message. The
/// assembly solver received that protection; the sketch solver had not. One requirement: if the result is not a
/// number, restore the sketch as it was and report honestly that it did not converge, rather than hand back
/// rubbish in silence.
#[test]
fn the_sketch_solver_never_returns_garbage() {
    for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY] {
        let before = vec![pt(1, 0.0, 0.0), pt(2, bad, bad), pt(3, 10.0, 0.0)];
        let mut points = before.clone();
        let cons = vec![
            Constraint::Fixed { p: 1 },
            Constraint::Distance { a: 1, b: 2, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 },
            Constraint::Distance { a: 2, b: 3, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 },
            Constraint::Vertical { a: 1, b: 2 },
        ];
        let res = solver::solve(&mut points, &cons);
        assert!(!res.is_finite(), "unusable input has to give an honest failure to converge rather than a number: {res}");
        for (i, (a, b)) in points.iter().zip(before.iter()).enumerate() {
            assert!(
                (a.x.is_nan() && b.x.is_nan()) || a.x == b.x,
                "point {i}: the solver has to restore the sketch as it was rather than overwrite it with rubbish"
            );
        }
    }
}
