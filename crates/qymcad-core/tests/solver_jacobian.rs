//! The analytic Jacobian is checked against a numerical derivative, for every constraint type.
//!
//! The derivatives in the solver are written out by hand. An error in such a formula is the quietest failure
//! available: the solution simply converges a little worse, which goes unnoticed until a part shows up where a
//! 130 mm dimension came out as −129.99900 and a cut left a film across a wall. Every constraint is checked here
//! on a non-degenerate configuration: the analytic form has to match a central difference.
use qymcad_core::model::{Constraint, SketchPoint};
use qymcad_core::solver::{jacobian_mismatch, RadiusVar};

fn pts() -> Vec<SketchPoint> {
    // deliberately awkward coordinates: zeros and symmetries mask errors in the formulas
    vec![
        SketchPoint { id: 1, x: 0.37, y: -1.21 },
        SketchPoint { id: 2, x: 4.13, y: 2.77 },
        SketchPoint { id: 3, x: -2.09, y: 3.41 },
        SketchPoint { id: 4, x: 5.63, y: -0.88 },
        SketchPoint { id: 5, x: 1.11, y: 6.02 },
    ]
}

fn radii() -> Vec<RadiusVar> {
    vec![RadiusVar { center: 3, value: 2.35 }, RadiusVar { center: 4, value: 1.47 }]
}

fn check(name: &str, c: Constraint) {
    let m = jacobian_mismatch(&pts(), &radii(), &c);
    assert!(m < 1e-6, "{name}: the analytic derivative differs from the numerical one by {m:.3e}");
}

#[test]
fn every_constraint_has_correct_analytic_derivatives() {
    check("Fixed", Constraint::Fixed { p: 1 });
    check("Horizontal", Constraint::Horizontal { a: 1, b: 2 });
    check("Vertical", Constraint::Vertical { a: 1, b: 2 });
    check("Coincident", Constraint::Coincident { a: 1, b: 2 });
    check("Concentric", Constraint::Concentric { c1: 3, c2: 4 });
    check("Distance (aligned)", Constraint::Distance { a: 1, b: 2, d: 3.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    check("Distance (along X)", Constraint::Distance { a: 1, b: 2, d: 3.0, off: 0.0, expr: String::new(), driven: false, axis: 1 });
    check("Distance (along Y)", Constraint::Distance { a: 1, b: 2, d: 3.0, off: 0.0, expr: String::new(), driven: false, axis: 2 });
    check("EdgeDistance", Constraint::EdgeDistance { c1: 3, c2: 4, d: 7.0, m1: 1, m2: -1, off: 0.0, expr: String::new(), driven: false });
    check("Parallel", Constraint::Parallel { a: 1, b: 2, c: 3, d: 4 });
    check("Perpendicular", Constraint::Perpendicular { a: 1, b: 2, c: 3, d: 4 });
    check("Equal", Constraint::Equal { a: 1, b: 2, c: 3, d: 4 });
    check("Collinear", Constraint::Collinear { a: 1, b: 2, c: 3, d: 4 });
    check("Midpoint", Constraint::Midpoint { p: 5, a: 1, b: 2 });
    check("Tangent", Constraint::Tangent { a: 1, b: 2, c: 3, r: 2.35 });
    check("CircleTangent (external)", Constraint::CircleTangent { c1: 3, c2: 4, external: true });
    check("CircleTangent (internal)", Constraint::CircleTangent { c1: 3, c2: 4, external: false });
    check("Symmetric", Constraint::Symmetric { a: 1, b: 2, la: 3, lb: 4 });
    check("Angle", Constraint::Angle { a: 1, b: 2, c: 3, deg: 42.0, expr: String::new(), driven: false });
    check("AngleLines", Constraint::AngleLines { a: 1, b: 2, c: 3, d: 4, deg: 42.0, expr: String::new(), driven: false });
    check("PointOnLine", Constraint::PointOnLine { p: 5, a: 1, b: 2 });
    check("DistancePL", Constraint::DistancePL { p: 5, a: 1, b: 2, d: 1.5, off: 0.0, expr: String::new(), driven: false });
    check("Diameter", Constraint::Diameter { c: 3, d: 4.0, diam: true, off: 0.0, expr: String::new(), driven: false });
    check("EqualRadius", Constraint::EqualRadius { c1: 3, c2: 4 });
    check("PointOnCircle", Constraint::PointOnCircle { p: 5, c: 3 });
    check("ArcLength(ccw)", Constraint::ArcLength { c: 3, a: 1, b: 2, ccw: true, len: 5.0, off: 0.0, expr: String::new(), driven: false });
    check("ArcLength(cw)", Constraint::ArcLength { c: 3, a: 1, b: 2, ccw: false, len: 5.0, off: 0.0, expr: String::new(), driven: false });
}
