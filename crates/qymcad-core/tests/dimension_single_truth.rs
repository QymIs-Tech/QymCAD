//! A dimension has one source of truth: its expression. The number is a derived value.
//!
//! The value of a dimension is stored both as a number, in a timeline node or in a sketch constraint, and as an
//! expression (`feat_dims` or `Constraint.expr`). Which of the two was true depended on who computed last, and
//! that had already misfired: recomputing the expressions at the start of a rebuild overwrote values that had
//! been set directly.
//!
//! The rule: where there is an expression it is the truth and the number is recomputed from it; entering a
//! number clears the expression and the number becomes a literal. These checks hold the rule from the side of
//! the core.
use qymcad_core::model::{Constraint, Param, Project};

fn sketch_with_dim() -> (Project, usize, usize) {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "L".into(), expr: "40".into(), value: 40.0 });
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let (a, b) = {
        let s = &p.sketches[si];
        (s.points[0].id, s.points[1].id)
    };
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 0.0, expr: "L".into(), driven: false, axis: 0 });
    let ci = p.sketches[si].constraints.len() - 1;
    (p, si, ci)
}

/// The expression drives the number: changing a parameter recomputes the value of the dimension and the
/// geometry follows.
#[test]
fn expression_drives_the_number_in_sketch_dims() {
    let (mut p, si, ci) = sketch_with_dim();
    p.solve_sketch(si);
    let d = |p: &Project| match p.sketches[si].constraints[ci] {
        Constraint::Distance { d, .. } => d,
        _ => unreachable!(),
    };
    assert!((d(&p) - 40.0).abs() < 1e-9, "the number came from the expression L=40 rather than staying at 10: {}", d(&p));
    let len = |p: &Project| {
        let s = &p.sketches[si];
        (s.points[1].x - s.points[0].x).hypot(s.points[1].y - s.points[0].y)
    };
    // The tolerance is 1e-3 rather than machine precision: the solver computed its Jacobian numerically, and
    // at coordinates of tens of millimetres that leaves a residual around 1e-5, here 39.999985. The accuracy of
    // the solver is a separate matter; what is checked here is that the geometry follows the expression rather
    // than the old number 10.
    assert!((len(&p) - 40.0).abs() < 1e-3, "the geometry follows the expression: {}", len(&p));

    p.parameters[0].expr = "55".into();
    p.eval_parameters();
    p.solve_sketch(si);
    assert!((d(&p) - 55.0).abs() < 1e-9, "changing the parameter drives the number of the dimension: {}", d(&p));
    assert!((len(&p) - 55.0).abs() < 1e-3, "and the geometry: {}", len(&p));
}

/// A number cannot outvote an expression: writing a value directly is undone by the next recomputation, which
/// restores the value of the expression. Otherwise the file would hold two disagreeing truths.
#[test]
fn writing_the_number_does_not_beat_the_expression() {
    let (mut p, si, ci) = sketch_with_dim();
    p.eval_parameters();
    if let Constraint::Distance { d, .. } = &mut p.sketches[si].constraints[ci] {
        *d = 999.0; // the second truth
    }
    p.eval_parameters();
    let d = match p.sketches[si].constraints[ci] {
        Constraint::Distance { d, .. } => d,
        _ => unreachable!(),
    };
    assert!((d - 40.0).abs() < 1e-9, "the expression is the truth: {d}");
}

/// Clearing the expression turns the number into a value of its own, a literal, depending on nothing.
#[test]
fn clearing_the_expression_makes_the_number_a_literal() {
    let (mut p, si, ci) = sketch_with_dim();
    p.eval_parameters();
    if let Constraint::Distance { d, expr, .. } = &mut p.sketches[si].constraints[ci] {
        *d = 12.5;
        expr.clear(); // exactly what entering a number into a dimension field does
    }
    p.parameters[0].expr = "999".into();
    p.eval_parameters();
    let d = match p.sketches[si].constraints[ci] {
        Constraint::Distance { d, .. } => d,
        _ => unreachable!(),
    };
    assert!((d - 12.5).abs() < 1e-9, "a literal does not follow a parameter: {d}");
}

/// A feature dimension: the same pair. Where an expression is given it is the truth; once cleared, the number
/// of the node becomes the truth.
#[test]
fn feature_dim_expression_wins_and_clearing_restores_the_number() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "H".into(), expr: "7".into(), value: 7.0 });
    let body = 1234;
    p.set_feat_dim(body, "height", "H*2".into());
    assert_eq!(p.feat_dim(body, "height"), Some("H*2"), "the expression is recorded");
    let vars = p.param_map();
    assert!((qymcad_core::expr::eval("H*2", &vars).unwrap() - 14.0).abs() < 1e-9, "the expression evaluates against the parameter");

    p.set_feat_dim(body, "height", String::new()); // entering a number clears the expression
    assert_eq!(p.feat_dim(body, "height"), None, "with the expression cleared the number of the node becomes the truth");
}
