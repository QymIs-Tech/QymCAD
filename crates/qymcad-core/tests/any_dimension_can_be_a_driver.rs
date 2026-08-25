//! Any dimension can become a driver, not only a distance between two points.
//!
//! The field for naming a driver appeared in some dimension popups and not in others, with no way to explain
//! the difference. There was a reason, but not one that could be justified: the name of a driver was stored as
//! a pair of points (`NamedDim { a, b }`) and its value was looked up as a `Constraint::Distance` between them.
//! The field therefore showed on exactly one kind of dimension, the linear one between two points. An angle, a
//! diameter, a distance to a line, an arc length or an edge gap could not be named at all, because the model
//! had nowhere to put them. That was a limitation of the storage, not a decision about which dimensions deserve
//! to be drivers.
//!
//! A dimension is now identified by a set of entities, and any of them can be named.
use qymcad_core::model::{Constraint, Id, Project};

fn sketch_with_points(p: &mut Project, n: usize) -> (usize, Vec<Id>) {
    use qymcad_core::geom::Point2;
    let pts: Vec<Point2> = (0..n).map(|i| Point2::new(i as f64 * 10.0, if i % 2 == 0 { 0.0 } else { 10.0 })).collect();
    let sid = p.add_line_sketch("Sketch", pts, true);
    let si = p.sketch_index(sid).unwrap();
    let ids = p.sketches[si].points.iter().map(|q| q.id).collect();
    (si, ids)
}

/// Every kind of dimension is recognised, so its popup offers a driver name.
#[test]
fn every_dimension_kind_is_recognised() {
    let cases: Vec<(&str, Constraint)> = vec![
        ("distance", Constraint::Distance { a: 1, b: 2, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0 }),
        ("angle", Constraint::Angle { a: 1, b: 2, c: 3, deg: 45.0, expr: String::new(), driven: false }),
        ("angle between lines", Constraint::AngleLines { a: 1, b: 2, c: 3, d: 4, deg: 30.0, expr: String::new(), driven: false }),
        ("diameter", Constraint::Diameter { c: 5, d: 12.0, off: 0.0, expr: String::new(), driven: false, diam: true }),
        ("distance to a line", Constraint::DistancePL { p: 1, a: 2, b: 3, d: 7.0, off: 0.0, expr: String::new(), driven: false }),
        ("edge gap", Constraint::EdgeDistance { c1: 5, c2: 6, d: 3.0, m1: 1, m2: -1, off: 0.0, expr: String::new(), driven: false }),
        ("arc length", Constraint::ArcLength { c: 5, a: 1, b: 2, ccw: true, len: 15.0, off: 0.0, expr: String::new(), driven: false }),
    ];
    for (what, c) in &cases {
        let refs = Project::dim_refs(c);
        assert!(refs.is_some(), "{what} is not recognised as a dimension, so its popup will offer no driver name");
        assert!(!refs.unwrap().is_empty(), "{what} is recognised by an empty set, so the dimension cannot be found by it");
        assert!(Project::dim_value_of(c).is_some(), "the value of {what} cannot be read");
    }
}

/// A constraint without a dimension does not become a driver. Otherwise the name field would appear on
/// coincidence and parallelism, where there is nothing to name.
#[test]
fn a_plain_constraint_is_not_a_dimension() {
    for c in [
        Constraint::Coincident { a: 1, b: 2 },
        Constraint::Horizontal { a: 1, b: 2 },
        Constraint::Parallel { a: 1, b: 2, c: 3, d: 4 },
        Constraint::Fixed { p: 1 },
    ] {
        assert!(Project::dim_refs(&c).is_none(), "a non-dimension was recognised as a dimension: {c:?}");
        assert!(Project::dim_value_of(&c).is_none(), "a value was read from a non-dimension: {c:?}");
    }
}

/// An angle named as a driver is visible in formulas, which was not possible at all before.
#[test]
fn a_named_angle_becomes_a_driver_with_a_value() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 4);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Angle { a: ids[0], b: ids[1], c: ids[2], deg: 37.0, expr: String::new(), driven: false });

    let refs = Project::dim_refs(p.sketches[si].constraints.last().unwrap()).expect("an angle is a dimension");
    assert!(p.add_named_dim("ugol".into(), sid, refs), "an angle has to be nameable as a driver");

    let d = p.drivers().into_iter().find(|d| d.name == "ugol").expect("the driver is in the list");
    assert_eq!(d.value, Some(37.0), "the value of the angle did not reach the driver list");
    assert_eq!(p.param_map().get("ugol"), Some(&37.0), "the angle is not visible in formulas");
}

/// A diameter likewise. It is given by a single circle and could not be addressed by a pair of points at
/// all.
#[test]
fn a_named_diameter_becomes_a_driver() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("Circle");
    let sid = p.sketches[si].id;
    let cid = p.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.sketches[si].constraints.push(Constraint::Diameter { c: cid, d: 20.0, off: 0.0, expr: String::new(), driven: false, diam: true });

    let refs = Project::dim_refs(p.sketches[si].constraints.last().unwrap()).expect("a diameter is a dimension");
    assert_eq!(refs.len(), 1, "a diameter is identified by a single circle: {refs:?}");
    assert!(p.add_named_dim("diam".into(), sid, refs), "a diameter has to be nameable as a driver");
    assert_eq!(p.param_map().get("diam"), Some(&20.0), "the diameter is not visible in formulas");
}

/// The order of the entities does not matter: a distance from A to B and from B to A are the same dimension,
/// and the name has to be found either way, or a driver would be lost when the points of a constraint are
/// swapped.
#[test]
fn the_reference_order_does_not_matter() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance {
        a: ids[0],
        b: ids[1],
        d: 25.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 0,
    });

    assert!(p.add_named_dim("len".into(), sid, vec![ids[1], ids[0]]), "the name is set with the order reversed");
    assert_eq!(p.param_map().get("len"), Some(&25.0), "the dimension was not found with the points in reverse order");
}

/// Naming the same dimension again replaces the previous name rather than adding a second driver.
#[test]
fn renaming_the_same_dimension_replaces_the_old_name() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance {
        a: ids[0],
        b: ids[1],
        d: 25.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 0,
    });

    assert!(p.add_named_dim("staroe".into(), sid, vec![ids[0], ids[1]]));
    assert!(p.add_named_dim("novoe".into(), sid, vec![ids[1], ids[0]]));
    assert_eq!(p.named_dims.len(), 1, "one dimension ended up with two names: {:?}", p.named_dims);
    assert_eq!(p.named_dims[0].name, "novoe");
}

/// Two drivers with the same name cannot be created.
///
/// Naming `len` in two sketches of one part produced two identical rows with different numbers in the parameter
/// list. In a formula the name is one, and the scope holds one value, so the second driver is always
/// unreachable. The name used to be accepted regardless, and the model became quietly broken.
#[test]
fn a_second_driver_cannot_take_a_used_name() {
    let mut p = Project::default();
    p.new_document();
    let (si1, ids1) = sketch_with_points(&mut p, 3);
    let sid1 = p.sketches[si1].id;
    p.sketches[si1].constraints.push(Constraint::Distance { a: ids1[0], b: ids1[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    assert!(p.add_named_dim("len".into(), sid1, vec![ids1[0], ids1[1]]), "the first driver is named");

    let (si2, ids2) = sketch_with_points(&mut p, 3);
    let sid2 = p.sketches[si2].id;
    p.sketches[si2].constraints.push(Constraint::Distance { a: ids2[0], b: ids2[1], d: 90.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });

    assert!(!p.add_named_dim("len".into(), sid2, vec![ids2[0], ids2[1]]), "the name is taken, so the second driver has to be rejected");
    assert_eq!(p.named_dims.len(), 1, "two namesakes remain in the project: {:?}", p.named_dims);
    assert_eq!(p.param_map().get("len"), Some(&20.0), "the wrong dimension was left visible in formulas");
}

/// The name of a global parameter is taken too: both live in one scope.
#[test]
fn a_driver_cannot_take_a_global_parameter_name() {
    use qymcad_core::model::Param;
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 50.0 });
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance { a: ids[0], b: ids[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });

    assert!(!p.add_named_dim("w".into(), sid, vec![ids[0], ids[1]]), "the name of a global parameter has to count as taken");
    assert!(p.driver_name_taken("w", sid, &[ids[0], ids[1]]), "the interface has to learn the name is taken before the button is pressed");
}

/// Renaming oneself is allowed, or a dimension could not be renamed at all.
#[test]
fn renaming_the_same_dimension_is_still_allowed() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance { a: ids[0], b: ids[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });

    assert!(p.add_named_dim("len".into(), sid, vec![ids[0], ids[1]]));
    assert!(p.add_named_dim("len".into(), sid, vec![ids[0], ids[1]]), "the same dimension under the same name is not a conflict");
    assert!(p.add_named_dim("dlina".into(), sid, vec![ids[0], ids[1]]), "renaming one's own dimension has to be possible");
    assert_eq!(p.named_dims.len(), 1);
    assert_eq!(p.named_dims[0].name, "dlina");
}
