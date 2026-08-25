//! A feature parameter is a driver just like a sketch dimension.
//!
//! Only a sketch dimension used to be nameable: an extrusion height, a fillet radius or a shell thickness could
//! reference drivers but carried no name of their own. In a professional CAD every parameter of the model has a
//! name.
//!
//! One mechanism serves both cases (`DimTarget`), and the checks here guard exactly that: the rules for a taken
//! name, renaming with references, breadcrumbs and values have to work on a feature exactly as they do on a
//! sketch, or the two will drift apart.
use qymcad_core::drivers::DriverKind;
use qymcad_core::feature::{FeatureKind, Reach, ShellSide};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, DimTarget, Id, Param, Project};

/// A part with an extrusion of height `h`. Returns the component and the timeline node.
fn part_with_extrude(p: &mut Project, part: &str, h: f64) -> (Id, Id) {
    let comp = p.add_component(part);
    p.set_active_component(Some(comp));
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)],
        true,
    );
    p.add_sketch_node(sid, "Profile");
    let node = p.add_extrude_on(sid, 0, h, qymcad_core::feature::Reach::Forward, 0.0);
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == node) {
        n.name = "Extrude".into();
    }
    (comp, node)
}

/// The parameter table of a feature matches what the rebuild actually applies.
///
/// If a key appears in `dims()` but the rebuild knows nothing of it, a driver would move something that does
/// not exist; the other way round, a parameter cannot be named at all. The list is therefore single and lives
/// next to the type.
#[test]
fn every_tool_exposes_its_numbers() {
    let edges = qymcad_core::refs::Ref::one(1, Default::default());
    let cases: Vec<(&str, FeatureKind, Vec<&str>)> = vec![
        (
            "extrude",
            FeatureKind::Extrude { sketch: 1, profiles: vec![], height: 25.0, reach: Reach::Forward, down: 0.0, fill: vec![], body: 0 },
            vec!["height", "down"],
        ),
        ("fillet", FeatureKind::Fillet { src: 1, radius: 3.0, edges: edges.clone(), at_vertices: vec![], body: 0 }, vec!["radius"]),
        ("shell", FeatureKind::Shell { src: 1, thickness: 1.2, faces: edges.clone(), side: ShellSide::Inward, body: 0 }, vec!["thickness"]),
        ("draft", FeatureKind::Draft { src: 1, faces: edges.clone(), neutral: edges.clone(), angle: 4.0, flip: false, body: 0 }, vec!["angle"]),
        ("thicken", FeatureKind::Thicken { src: 1, face: 1, thickness: 2.0, join: 0, body: 0 }, vec!["thickness"]),
        ("push face", FeatureKind::PushFace { src: 1, face: edges.clone(), dist: 2.0, body: 0 }, vec!["dist"]),
        ("cylinder", FeatureKind::Cylinder { r: 5.0, h: 10.0, body: 0 }, vec!["r", "h"]),
        ("circular pattern", FeatureKind::CircularArray { src: 1, count: 3, angle: 360.0, axis: 0, body: 0 }, vec!["angle"]),
    ];
    for (what, kind, keys) in &cases {
        let have: Vec<&str> = kind.dims().into_iter().map(|(k, _)| k).collect();
        for k in keys {
            assert!(have.contains(k), "{what} does not expose the parameter {k}: {have:?}, so there is nothing to name as a driver");
        }
        for k in &have {
            assert!(kind.dim(k).is_some(), "{what}: the parameter {k} is listed but no value can be read from it");
        }
    }
}

/// The point: a named feature parameter is visible in formulas.
#[test]
fn a_named_feature_parameter_works_in_formulas() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);

    assert!(p.add_named_feat_dim("vysota".into(), node, "height"), "an extrusion height has to be nameable");
    assert_eq!(p.param_map().get("vysota"), Some(&25.0), "the named height is not visible in formulas");

    // And it can be used: another feature computes its own dimension from it.
    p.parameters.push(Param { name: "zazor".into(), expr: "vysota/5".into(), value: 0.0 });
    assert_eq!(p.eval_expr("vysota/5").unwrap(), 5.0, "a formula over a feature parameter does not evaluate");
}

/// The value is taken exactly as the rebuild takes it: an expression is evaluated when present, otherwise the
/// stored number is used. Otherwise a driver would show one thing while the part was built from another.
#[test]
fn the_value_follows_the_expression_when_there_is_one() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 50.0 });
    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));

    p.set_feat_dim(node, "height", "w/2".into());
    assert_eq!(p.param_map().get("vysota"), Some(&25.0), "the value has to be computed from the expression of the feature");

    p.parameters[0].expr = "80".into();
    p.parameters[0].value = 80.0;
    assert_eq!(p.param_map().get("vysota"), Some(&40.0), "the value did not follow the parameter it depends on");
}

/// The breadcrumbs lead to the feature, giving `Part.Extrude`. Sketches use the same shape of path, and
/// features have to match it, or a list entry says nothing about what it refers to.
#[test]
fn a_feature_driver_carries_the_path_to_its_feature() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);
    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));

    let d = p.drivers().into_iter().find(|d| d.name == "vysota").expect("the driver is in the list");
    assert_eq!(d.kind, DriverKind::FeatDim);
    assert_eq!(d.path, "Housing.Extrude", "the breadcrumbs have to lead to the part and the feature");
    assert_eq!(d.value, Some(25.0));
    assert_eq!(d.label(), "vysota — Housing.Extrude");
}

/// The search finds it on equal terms with sketch dimensions, by name and by path alike.
#[test]
fn search_finds_feature_drivers_too() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);
    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));

    assert_eq!(p.drivers_matching("vys").len(), 1, "it was not found by name");
    let by_path = p.drivers_matching("Extrude");
    assert_eq!(by_path.len(), 1, "it was not found by the name of the feature: {by_path:?}");
    assert_eq!(by_path[0].name, "vysota");
}

/// The scope is shared: the name of a sketch dimension is taken for a feature parameter too, and the other
/// way round.
#[test]
fn sketch_and_feature_names_share_one_scope() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);
    let sid = p.sketches[0].id;
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().take(2).map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance { a: pts[0], b: pts[1], d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    assert!(p.add_named_dim("len".into(), sid, pts.clone()));

    assert!(!p.add_named_feat_dim("len".into(), node, "height"), "the name of a sketch dimension has to count as taken for a feature as well");
    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));
    assert!(!p.add_named_dim("vysota".into(), sid, pts), "the name of a feature parameter has to count as taken for a sketch as well");

    // And the owner is named, so the interface has something to say beyond a bare "taken".
    let owner = p.name_owner("vysota").expect("the owner is found");
    assert_eq!(owner.path, "Housing.Extrude");
}

/// Renaming a feature parameter carries the formulas with it: the same rule as for a sketch.
#[test]
fn renaming_a_feature_driver_updates_references() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);
    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));
    p.parameters.push(Param { name: "zazor".into(), expr: "vysota/5".into(), value: 5.0 });

    assert_eq!(p.rename_driver("vysota", "h_korpusa"), Ok(1));
    assert_eq!(p.parameters[0].expr, "h_korpusa/5", "the formula still points at the vanished name");
    assert_eq!(p.param_map().get("h_korpusa"), Some(&25.0));
}

/// An empty name removes the driver, and a second name on the same parameter replaces the first.
#[test]
fn naming_the_same_parameter_again_replaces_the_name() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);

    assert!(p.add_named_feat_dim("staroe".into(), node, "height"));
    assert!(p.add_named_feat_dim("novoe".into(), node, "height"));
    assert_eq!(p.named_dims.len(), 1, "one parameter ended up with two names: {:?}", p.named_dims);
    assert_eq!(p.named_dims[0].name, "novoe");

    assert!(!p.add_named_feat_dim(String::new(), node, "height"), "an empty name removes the driver");
    assert!(p.named_dims.is_empty(), "the name was not removed: {:?}", p.named_dims);
}

/// Different parameters of one feature are different drivers. Otherwise naming the height would remove the
/// name of the depth.
#[test]
fn two_parameters_of_one_feature_are_told_apart() {
    let mut p = Project::default();
    p.new_document();
    let (_c, node) = part_with_extrude(&mut p, "Housing", 25.0);

    assert!(p.add_named_feat_dim("vysota".into(), node, "height"));
    assert!(p.add_named_feat_dim("vniz".into(), node, "down"));
    assert_eq!(p.named_dims.len(), 2, "the second parameter displaced the first: {:?}", p.named_dims);
    assert_eq!(p.name_of_target(&DimTarget::Feature { node, key: "height".into() }), "vysota");
    assert_eq!(p.name_of_target(&DimTarget::Feature { node, key: "down".into() }), "vniz");
}
