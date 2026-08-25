//! A NAMED DIMENSION WORKS INSIDE ANOTHER DIMENSION'S FORMULA.
//!
//! Reported behaviour: dimension drivers do not work when written into other dimensions.
//!
//! A name given to a dimension is the same kind of thing as a global parameter `w = 50`: it may be written
//! into any expression field of the document. That is what the completion list promises when it offers the
//! name — and an offer that does not work afterwards is worse than no offer at all.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id, Project};

/// A sketch of four points with a linear dimension between the first two.
fn sketch_with_a_dimension(p: &mut Project, name: &str, len: f64) -> (usize, Id, Vec<Id>) {
    let sid = p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(len, 0.0), Point2::new(len, 10.0), Point2::new(0.0, 10.0)], true);
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance { a: pts[0], b: pts[1], d: len, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    (si, sid, pts)
}

/// The value of the first dimension of sketch `si`.
fn dim_of(p: &Project, si: usize) -> f64 {
    p.sketches[si]
        .constraints
        .iter()
        .find_map(|c| match c {
            Constraint::Distance { d, .. } => Some(*d),
            _ => None,
        })
        .expect("the sketch has a linear dimension")
}

/// A NAME GIVEN TO A DIMENSION RESOLVES IN ANOTHER DIMENSION'S FORMULA.
#[test]
fn a_named_dimension_is_visible_from_another_dimension() {
    let mut p = Project::default();
    let (a_si, a_sid, a_pts) = sketch_with_a_dimension(&mut p, "Base", 20.0);
    assert!(p.add_named_dim("w".into(), a_sid, vec![a_pts[0], a_pts[1]]), "setup: the dimension took the name");

    let (b_si, _, _) = sketch_with_a_dimension(&mut p, "Other", 5.0);
    match &mut p.sketches[b_si].constraints[0] {
        Constraint::Distance { expr, .. } => *expr = "w*2".into(),
        c => panic!("setup: the second sketch has no linear dimension, {c:?}"),
    }

    p.eval_parameters();

    assert_eq!(dim_of(&p, a_si), 20.0, "the driving dimension itself must not move");
    assert_eq!(dim_of(&p, b_si), 40.0, "the formula `w*2` did not see the driver: the dimension kept its old value");
}

/// MOVING THE DRIVER MOVES WHAT DEPENDS ON IT.
#[test]
fn editing_the_driver_moves_the_dependent_dimension() {
    let mut p = Project::default();
    let (a_si, a_sid, a_pts) = sketch_with_a_dimension(&mut p, "Base", 20.0);
    assert!(p.add_named_dim("w".into(), a_sid, vec![a_pts[0], a_pts[1]]), "setup: the dimension took the name");

    let (b_si, _, _) = sketch_with_a_dimension(&mut p, "Other", 5.0);
    match &mut p.sketches[b_si].constraints[0] {
        Constraint::Distance { expr, .. } => *expr = "w+5".into(),
        c => panic!("setup: the second sketch has no linear dimension, {c:?}"),
    }
    p.eval_parameters();
    assert_eq!(dim_of(&p, b_si), 25.0, "setup: the dependency did not work even before the edit");

    // A person changes the driving dimension by hand.
    match &mut p.sketches[a_si].constraints[0] {
        Constraint::Distance { d, .. } => *d = 30.0,
        c => panic!("setup: {c:?}"),
    }
    p.eval_parameters();

    assert_eq!(dim_of(&p, b_si), 35.0, "the driver moved and what depends on it stayed put");
}

/// A GLOBAL PARAMETER AND A NAMED DIMENSION MIX IN ONE FORMULA.
#[test]
fn a_parameter_and_a_driver_mix_in_one_formula() {
    let mut p = Project::default();
    p.parameters.push(qymcad_core::model::Param { name: "k".into(), expr: "3".into(), value: 3.0 });
    let (_, a_sid, a_pts) = sketch_with_a_dimension(&mut p, "Base", 20.0);
    assert!(p.add_named_dim("w".into(), a_sid, vec![a_pts[0], a_pts[1]]), "setup: the dimension took the name");

    let (b_si, _, _) = sketch_with_a_dimension(&mut p, "Other", 5.0);
    match &mut p.sketches[b_si].constraints[0] {
        Constraint::Distance { expr, .. } => *expr = "w+k".into(),
        c => panic!("setup: {c:?}"),
    }
    p.eval_parameters();

    assert_eq!(dim_of(&p, b_si), 23.0, "a parameter and a driver in one formula: expected 20+3");
}

/// A DRIVER FEEDS A GLOBAL PARAMETER TOO.
///
/// The two live in one scope, so the dependency has to work in both directions — otherwise the rule "a name
/// is a name" holds only halfway and nothing says which half.
#[test]
fn a_parameter_may_be_written_through_a_driver() {
    let mut p = Project::default();
    let (_, a_sid, a_pts) = sketch_with_a_dimension(&mut p, "Base", 20.0);
    assert!(p.add_named_dim("w".into(), a_sid, vec![a_pts[0], a_pts[1]]), "setup: the dimension took the name");
    p.parameters.push(qymcad_core::model::Param { name: "half".into(), expr: "w/2".into(), value: 0.0 });

    p.eval_parameters();

    let half = p.parameters.iter().find(|x| x.name == "half").expect("the parameter");
    assert_eq!(half.value, 10.0, "the parameter `w/2` did not see the driver");
}
