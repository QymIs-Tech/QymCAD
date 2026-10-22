//! A VALUE OF AN ITEM NO DIMENSION HOLDS - the sides of a polygon, the axes of an ellipse, the count of a pattern of a
//! sketch, the height of a text, the ratio of a relation of mates - IS NAMED AS A NUMBER OF A FEATURE IS: its formula
//! kept under the item's id, its name a driver another formula reads, and a change of what the formula reads carried to
//! the item. Reported behaviour: such a name was a parameter of the project of its own, and changing it moved nothing.

use qymcad_core::geom::Point2;
use qymcad_core::model::{DimTarget, FeatNumber, Param, PatternKind, PlainNumber, Project};

/// A document with one sketch open, and its index.
fn sketch() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    (p, si)
}

/// Parameter `name` given the number `v`, the formulas over it worked out.
fn set_param(p: &mut Project, name: &str, v: f64) {
    match p.parameters.iter_mut().find(|q| q.name == name) {
        Some(q) => q.expr = v.to_string(),
        None => p.parameters.push(Param { name: name.into(), expr: v.to_string(), value: v }),
    }
    p.eval_parameters();
}

/// The value `key` of item `node` as the document holds it.
fn value(p: &Project, node: u64, key: &str) -> f64 {
    p.kept_number(node, key).unwrap_or_else(|| panic!("{key} of {node} holds no value"))
}

#[test]
fn the_sides_of_a_polygon_follow_a_formula_and_are_read_by_a_name() {
    let (mut p, si) = sketch();
    let (c, _) = p.add_polygon_param(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 6, qymcad_core::feature::Purpose::Real);
    assert_eq!(value(&p, c, "sides"), 6.0, "a hexagon is read as six sides");
    // named, the count is read by another formula
    assert!(p.add_named_feat_dim("n".into(), c, "sides"), "the sides of a polygon cannot be named");
    assert_eq!(p.param_map().get("n"), Some(&6.0), "the name of the sides reads no number");
    // driven by a parameter, the polygon is rebuilt with the sides the parameter gives
    set_param(&mut p, "k", 8.0);
    p.write_feat_formula(&FeatNumber { node: c, key: "sides" }, "k", PlainNumber::Stands).expect("the formula of the sides is refused");
    p.eval_parameters();
    assert_eq!(value(&p, c, "sides"), 8.0, "the polygon did not take the sides of its formula");
    set_param(&mut p, "k", 5.0);
    assert_eq!(value(&p, c, "sides"), 5.0, "the polygon did not follow the parameter of its sides");
    assert_eq!(p.param_map().get("n"), Some(&5.0), "the name of the sides did not follow the polygon");
}

#[test]
fn the_axes_and_the_turn_of_an_ellipse_follow_their_formulas() {
    let (mut p, si) = sketch();
    p.add_ellipse_entity(si, Point2::new(0.0, 0.0), 20.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let c = p.sketches[si].entities.iter().find_map(|e| e.kind.as_ellipse().map(|el| el.c)).expect("the centre of the ellipse");
    assert!((value(&p, c, "w") - 40.0).abs() < 1e-6, "the width of an ellipse of semi-axis 20 is not 40: {}", value(&p, c, "w"));
    set_param(&mut p, "b", 30.0);
    p.write_feat_formula(&FeatNumber { node: c, key: "w" }, "b*2", PlainNumber::Stands).expect("the formula of the width is refused");
    p.write_feat_formula(&FeatNumber { node: c, key: "turn" }, "30", PlainNumber::Stands).expect("the formula of the turn is refused");
    p.solve_sketch(si);
    assert!((value(&p, c, "w") - 60.0).abs() < 1e-3, "the width did not take its formula: {}", value(&p, c, "w"));
    assert!((value(&p, c, "turn") - 30.0).abs() < 1e-3, "the turn did not take its formula: {}", value(&p, c, "turn"));
    set_param(&mut p, "b", 25.0);
    p.solve_sketch(si);
    assert!((value(&p, c, "w") - 50.0).abs() < 1e-3, "the width did not follow its parameter: {}", value(&p, c, "w"));
}

#[test]
fn the_count_of_a_pattern_of_a_sketch_follows_a_name() {
    let (mut p, si) = sketch();
    let line = p.add_line_entity(si, 0.0, 0.0, 5.0, 0.0, qymcad_core::feature::Purpose::Real);
    let id = p.add_pattern(si, &[line], PatternKind::Linear { dx: 10.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 });
    let copies = |p: &Project| p.sketches[si].patterns.iter().find(|q| q.id == id).map(|q| q.instances.len()).unwrap_or(0);
    let two = copies(&p);
    assert!(two > 0, "a pattern of three makes no copies");
    // the count named drives the step of another value through a formula reading it
    assert!(p.add_named_feat_dim("cnt".into(), id, "count"), "the count of a pattern cannot be named");
    assert_eq!(p.param_map().get("cnt"), Some(&3.0));
    set_param(&mut p, "m", 5.0);
    p.write_feat_formula(&FeatNumber { node: id, key: "count" }, "m", PlainNumber::Stands).expect("the formula of the count is refused");
    p.eval_parameters();
    assert_eq!(value(&p, id, "count"), 5.0, "the pattern did not take the count of its formula");
    assert!(copies(&p) > two, "a count of five made no more copies than a count of three: {} against {two}", copies(&p));
    assert_eq!(p.eval_expr("cnt*2").ok(), Some(10.0), "a formula reading the name of the count did not follow it");
}

#[test]
fn the_height_of_a_text_follows_its_formula() {
    let (mut p, si) = sketch();
    p.add_sketch_text(
        si,
        qymcad_core::model::TextSpec {
            at: Point2::new(0.0, 0.0),
            height: 10.0,
            angle: 0.0,
            text: "A".into(),
            glyphs: vec![vec![Point2::new(0.0, 0.0), Point2::new(5.0, 10.0)]],
            font: Default::default(),
        },
        qymcad_core::feature::Purpose::Real,
    );
    let id = p.sketches[si].texts[0].id;
    set_param(&mut p, "t", 20.0);
    p.write_feat_formula(&FeatNumber { node: id, key: "height" }, "t", PlainNumber::Stands).expect("the formula of the height is refused");
    p.eval_parameters();
    assert_eq!(value(&p, id, "height"), 20.0, "the text did not take the height of its formula");
    let top = p.sketches[si].texts[0].glyphs[0][1];
    assert!((top.y - 20.0).abs() < 1e-9, "the letters were not scaled with the height: the top stands at {}", top.y);
}

#[test]
fn a_named_value_of_an_item_is_one_name_of_the_document() {
    let (mut p, si) = sketch();
    let (c, _) = p.add_polygon_param(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 6, qymcad_core::feature::Purpose::Real);
    assert!(p.add_named_feat_dim("n".into(), c, "sides"));
    let target = DimTarget::Feature { node: c, key: "sides".into() };
    assert_eq!(p.name_of_target(&target), "n", "the name of the sides is not the polygon's");
    assert!(p.driver_name_taken_by("n", &DimTarget::Feature { node: c, key: "turn".into() }), "the name of the sides is free for the turn of the same polygon");
}
