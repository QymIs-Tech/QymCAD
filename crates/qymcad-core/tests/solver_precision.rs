//! Solver accuracy: the geometry has to land on the dimension rather than near it.
//!
//! The Jacobian was computed by a one-sided difference with an absolute step of 1e-6. At coordinates of hundreds
//! of millimetres the relative step falls below machine precision: the difference loses significant digits and
//! the solution stalls with a residual around 1e-5. On a real part that showed up as a 130 mm dimension with the
//! geometry at −129.99900, and a cut leaving a film of a micron instead of an opening.
use qymcad_core::model::{Constraint, Project};

fn line(p: &mut Project, x0: f64, y0: f64, x1: f64, y1: f64) -> (usize, u64, u64) {
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_line_entity(si, x0, y0, x1, y1, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    (si, a, b)
}

/// A dimension is met exactly, both at small values and at hundreds of millimetres, which is where it used to
/// break.
#[test]
fn dimension_is_met_exactly_at_any_scale() {
    for target in [5.0_f64, 40.0, 130.0, 500.0] {
        let mut p = Project::default();
        p.new_document();
        let (si, a, b) = line(&mut p, 0.0, 0.0, target * 0.6, 0.0);
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });
        p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
        p.sketches[si].constraints.push(Constraint::Distance { a, b, d: target, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        p.solve_sketch(si);
        let s = &p.sketches[si];
        let len = (s.points[1].x - s.points[0].x).hypot(s.points[1].y - s.points[0].y);
        assert!((len - target).abs() < 1e-9, "dimension {target}: length {len:.12}, error {:.2e}", (len - target).abs());
    }
}

/// The failing case in its pure form: a vertical placed 130 mm from an axis by a dimension, and a rectangle
/// collinear with it. All of it has to land on −130.000000000 rather than −129.99900, or the cut leaves a film
/// across the wall instead of an opening.
#[test]
fn reference_line_lands_exactly_on_its_dimension() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    let (axis_o, axis_up) = p.ensure_axis(si, 1); // the Y axis of the sketch: origin and direction, both anchored
    let aux = p.add_line_entity(si, -129.999, 25.0, -129.9993, -234.0, qymcad_core::feature::Purpose::Construction);
    p.add_rect_entity(si, -129.9991, -176.0, -124.999, -36.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let (a0, a1) = match p.sketches[si].entities.iter().find(|e| e.id == aux).unwrap().kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        _ => unreachable!(),
    };
    // the left side of the rectangle, its own points only; the ends of the construction line must not enter
    let rect_left: Vec<u64> = p.sketches[si]
        .points
        .iter()
        .filter(|q| q.id != a0 && q.id != a1 && (q.x + 129.9991).abs() < 1e-6)
        .map(|q| q.id)
        .collect();
    assert_eq!(rect_left.len(), 2, "the left side of the rectangle has two points");

    let s = &mut p.sketches[si];
    s.constraints.push(Constraint::Vertical { a: a0, b: a1 });
    s.constraints.push(Constraint::DistancePL { p: a1, a: axis_o, b: axis_up, d: 130.0, off: 0.0, expr: String::new(), driven: false });
    s.constraints.push(Constraint::Collinear { a: a0, b: a1, c: rect_left[0], d: rect_left[1] });
    p.solve_sketch(si);
    // 1) the line itself landed on the dimension
    for id in [a0, a1] {
        let x = p.sketches[si].points.iter().find(|q| q.id == id).unwrap().x;
        assert!((x + 130.0).abs() < 1e-7, "the construction line sits at {x:.12} and has to sit at -130, error {:.2e}", (x + 130.0).abs());
    }
    // 2) the rectangle followed it, the collinearity being satisfied: an opening rather than a film
    for id in &rect_left {
        let x = p.sketches[si].points.iter().find(|q| q.id == *id).unwrap().x;
        assert!((x + 130.0).abs() < 1e-6, "the side of the rectangle sits at {x:.12} and has to sit at -130", );
    }
    // 3) and no constraint was left unsatisfied
    let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
    assert!(worst < 1e-7, "worst constraint residual: {worst:.3e}");
}
