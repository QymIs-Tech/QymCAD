//! Every dimensional constraint can be named and read independently of its participants.
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
        ("distance", Constraint::Distance { dimension: Default::default(), a: 1, b: 2, d: 10.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None }),
        ("angle", Constraint::Angle { dimension: Default::default(), a: 1, b: 2, c: 3, deg: 45.0, expr: String::new(), driven: false, off: 0.0, at: None }),
        ("angle between lines", Constraint::AngleLines { dimension: Default::default(), a: 1, b: 2, c: 3, d: 4, deg: 30.0, expr: String::new(), driven: false, off: 0.0, at: None }),
        ("diameter", Constraint::Diameter { dimension: Default::default(), c: 5, d: 12.0, off: 0.0, expr: String::new(), driven: false, diam: true, at: None }),
        ("distance to a line", Constraint::DistancePL { dimension: Default::default(), p: 1, a: 2, b: 3, d: 7.0, off: 0.0, expr: String::new(), driven: false, at: None }),
        ("edge gap", Constraint::EdgeDistance { dimension: Default::default(), c1: 5, c2: 6, d: 3.0, m1: 1, m2: -1, off: 0.0, expr: String::new(), driven: false, at: None }),
        ("arc length", Constraint::ArcLength { dimension: Default::default(), c: 5, a: 1, b: 2, ccw: true, len: 15.0, off: 0.0, expr: String::new(), driven: false }),
    ];
    for (what, c) in &cases {
        assert!(c.dimension_id().is_some(), "{what} is not recognised as a dimension, so its popup will offer no driver name");
        assert!(Project::dim_value_of(c).is_some(), "the value of {what} cannot be read");
    }
}

/// A constraint without a dimension does not become a driver. Otherwise the name field would appear on
/// coincidence and parallelism, where there is nothing to name.
#[test]
fn a_plain_constraint_is_not_a_dimension() {
    for c in [Constraint::Coincident { a: 1, b: 2 }, Constraint::Horizontal { a: 1, b: 2 }, Constraint::Parallel { a: 1, b: 2, c: 3, d: 4 }, Constraint::Fixed { p: 1 }] {
        assert!(c.dimension_id().is_none(), "a non-dimension was recognised as a dimension: {c:?}");
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
    p.sketches[si].constraints.push(Constraint::Angle { dimension: Default::default(), a: ids[0], b: ids[1], c: ids[2], deg: 37.0, expr: String::new(), driven: false, off: 0.0, at: None });

    assert!(
        p.name_sketch_dimension("ugol".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "an angle has to be nameable as a driver"
    );

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
    p.sketches[si].constraints.push(Constraint::Diameter { dimension: Default::default(), c: cid, d: 20.0, off: 0.0, expr: String::new(), driven: false, diam: true, at: None });

    assert!(
        p.name_sketch_dimension("diam".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "a diameter has to be nameable as a driver"
    );
    assert_eq!(p.param_map().get("diam"), Some(&20.0), "the diameter is not visible in formulas");
}

/// Reversing the endpoints of the same record keeps its name and value.
#[test]
fn the_reference_order_does_not_matter() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids[0], b: ids[1], d: 25.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    assert!(p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }));
    if let Constraint::Distance { a, b, .. } = p.sketches[si].constraints.last_mut().unwrap() {
        std::mem::swap(a, b);
    }
    assert_eq!(p.param_map().get("len"), Some(&25.0), "the dimension was not found with the points in reverse order");
}

/// Naming the same dimension again replaces the previous name rather than adding a second driver.
#[test]
fn renaming_the_same_dimension_replaces_the_old_name() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids[0], b: ids[1], d: 25.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    assert!(p.name_sketch_dimension("staroe".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }));
    assert!(p.name_sketch_dimension("novoe".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }));
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
    p.sketches[si1].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids1[0], b: ids1[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    assert!(p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid1, index: p.sketches[si1].constraints.len() - 1 }), "the first driver is named");

    let (si2, ids2) = sketch_with_points(&mut p, 3);
    let sid2 = p.sketches[si2].id;
    p.sketches[si2].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids2[0], b: ids2[1], d: 90.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    assert!(
        !p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid2, index: p.sketches[si2].constraints.len() - 1 }),
        "the name is taken, so the second driver has to be rejected"
    );
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
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids[0], b: ids[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    assert!(
        !p.name_sketch_dimension("w".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "the name of a global parameter has to count as taken"
    );
    assert!(
        p.sketch_dimension_name_taken("w", qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "the interface has to learn the name is taken before the button is pressed"
    );
}

/// Renaming oneself is allowed, or a dimension could not be renamed at all.
#[test]
fn renaming_the_same_dimension_is_still_allowed() {
    let mut p = Project::default();
    p.new_document();
    let (si, ids) = sketch_with_points(&mut p, 3);
    let sid = p.sketches[si].id;
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: ids[0], b: ids[1], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    assert!(p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }));
    assert!(
        p.name_sketch_dimension("len".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "the same dimension under the same name is not a conflict"
    );
    assert!(
        p.name_sketch_dimension("dlina".into(), qymcad_core::model::SketchDimensionSelection { sketch: sid, index: p.sketches[si].constraints.len() - 1 }),
        "renaming one's own dimension has to be possible"
    );
    assert_eq!(p.named_dims.len(), 1);
    assert_eq!(p.named_dims[0].name, "dlina");
}
