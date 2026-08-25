//! A driver knows where it comes from.
//!
//! Drivers are reachable across the whole project, yet the same name may occur in different assemblies, parts
//! and sketches, so both a search and an indication of which part, assembly and sketch a driver belongs to are
//! needed.
//!
//! Measurement showed this is not only about convenience. Two dimensions called `len` in different parts are
//! both accepted, but only the last remains in scope; the first is unreachable and nothing says so:
//!
//! ```text
//! in scope, 'len' = 70.0
//!    driver 'len' in sketch A, value 20.0   <- unreachable
//!    driver 'len' in sketch B, value 70.0
//! ```
use qymcad_core::drivers::DriverKind;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id, Param, Project};

/// A part with a sketch carrying a named driving dimension. Returns the component and the sketch.
fn part_with_driver(p: &mut Project, part: &str, sketch_name: &str, driver: &str, len: f64) -> (Id, Id) {
    let comp = p.add_component(part);
    p.set_active_component(Some(comp));
    let sid = p.add_line_sketch(
        sketch_name,
        vec![Point2::new(0.0, 0.0), Point2::new(len, 0.0), Point2::new(len, 10.0), Point2::new(0.0, 10.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.add_sketch_node(sid, sketch_name);
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance {
        a: pts[0],
        b: pts[1],
        d: len,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 0,
    });
    assert!(p.add_named_dim(driver.into(), sid, vec![pts[0], pts[1]]), "the dimension is named as a driver");
    (comp, sid)
}

#[test]
fn a_sketch_driver_carries_the_path_to_its_part() {
    let mut p = Project::default();
    p.new_document();
    part_with_driver(&mut p, "Housing", "Profile", "len", 20.0);

    let ds = p.drivers();
    let d = ds.iter().find(|d| d.name == "len").expect("the driver is in the list");
    assert_eq!(d.kind, DriverKind::SketchDim);
    assert_eq!(d.path, "Housing.Profile", "the breadcrumbs have to lead to the part and the sketch rather than be empty");
    assert_eq!(d.value, Some(20.0));
    assert_eq!(d.label(), "len — Housing.Profile", "the list has to show where a driver comes from");
}

#[test]
fn a_global_parameter_has_no_path_and_says_so() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 50.0 });

    let ds = p.drivers();
    let d = ds.iter().find(|d| d.name == "w").expect("the parameter is in the list");
    assert_eq!(d.kind, DriverKind::Parameter);
    assert!(d.path.is_empty(), "a global parameter has no path, living in the project itself");
    assert_eq!(d.label(), "w", "a global parameter is not attributed to anything");
}

/// Namesakes are distinguishable, and older documents still contain them.
///
/// A second namesake can no longer be created — `add_named_dim` refuses — but a document saved before that
/// restriction may well hold one. Opening such a file and showing two identical rows is exactly the complaint
/// this work addresses, so the path has to tell them apart here too. The state is assembled directly, since the
/// ordinary route no longer produces it.
#[test]
fn two_drivers_with_the_same_name_are_told_apart() {
    let mut p = Project::default();
    p.new_document();
    part_with_driver(&mut p, "Part A", "Sketch A", "len", 20.0);
    part_with_driver(&mut p, "Part B", "Sketch B", "len_b", 70.0);
    if let Some(n) = p.named_dims.iter_mut().find(|n| n.name == "len_b") {
        n.name = "len".into(); // this is what a document saved before the restriction looks like
    }

    let ds: Vec<_> = p.drivers().into_iter().filter(|d| d.name == "len").collect();
    assert_eq!(ds.len(), 2, "both drivers have to be in the list, not only the one that survived in scope");
    let paths: Vec<String> = ds.iter().map(|d| d.path.clone()).collect();
    assert!(paths.contains(&"Part A.Sketch A".to_string()), "the first one lost its path: {paths:?}");
    assert!(paths.contains(&"Part B.Sketch B".to_string()), "the second one lost its path: {paths:?}");
    let vals: Vec<Option<f64>> = ds.iter().map(|d| d.value).collect();
    assert!(vals.contains(&Some(20.0)) && vals.contains(&Some(70.0)), "the values collapsed together: {vals:?}");

    // And the ambiguity is stated honestly. Without it the list looks as though the choice were obvious,
    // while a bare `len` in a formula picks one of them unpredictably.
    assert!(ds.iter().all(|d| d.ambiguous), "the ambiguity is not flagged: {ds:?}");
}

#[test]
fn a_unique_name_is_not_marked_ambiguous() {
    let mut p = Project::default();
    p.new_document();
    part_with_driver(&mut p, "Housing", "Profile", "len", 20.0);
    p.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 50.0 });

    let ds = p.drivers();
    assert!(ds.iter().all(|d| !d.ambiguous), "unique names are flagged as ambiguous: {ds:?}");
}

/// A subassembly enters the breadcrumbs too, giving `Subassembly.Part.Sketch`.
#[test]
fn a_nested_part_shows_the_whole_chain() {
    let mut p = Project::default();
    p.new_document();
    // A subassembly, not a part. The core does not place a part inside a part, and rightly so: a new
    // component rises to the nearest assembly. Making the outer component a part yields a chain of only
    // `Cover.Outline`, which is a mistake in the scenario rather than a lost breadcrumb.
    let asm = p.add_assembly("Unit");
    p.set_active_component(Some(asm));
    let part = p.add_component("Cover");
    p.set_active_component(Some(part));
    let sid = p.add_line_sketch(
        "Outline",
        vec![Point2::new(0.0, 0.0), Point2::new(30.0, 0.0), Point2::new(30.0, 10.0), Point2::new(0.0, 10.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.add_sketch_node(sid, "Outline");
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance {
        a: pts[0],
        b: pts[1],
        d: 30.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 0,
    });
    assert!(p.add_named_dim("shirina".into(), sid, vec![pts[0], pts[1]]));

    let d = p.drivers().into_iter().find(|d| d.name == "shirina").expect("the driver is in the list");
    assert_eq!(d.path, "Unit.Cover.Outline", "the chain has to lead from the subassembly down to the sketch");
}

/// The search works by name and by path alike: what is remembered is either what it was called or where it
/// sits.
#[test]
fn search_finds_by_name_and_by_path() {
    let mut p = Project::default();
    p.new_document();
    part_with_driver(&mut p, "Part A", "Sketch A", "len", 20.0);
    part_with_driver(&mut p, "Part B", "Sketch B", "shirina", 70.0);
    p.parameters.push(Param { name: "wall".into(), expr: "3".into(), value: 3.0 });

    let by_name = p.drivers_matching("shir");
    assert_eq!(by_name.len(), 1, "the search by name found the wrong thing: {by_name:?}");
    assert_eq!(by_name[0].name, "shirina");

    let by_path = p.drivers_matching("Part A");
    assert_eq!(by_path.len(), 1, "the search by path found the wrong thing: {by_path:?}");
    assert_eq!(by_path[0].name, "len", "searching by the name of a part has to find its driver");

    let all = p.drivers_matching("");
    assert_eq!(all.len(), 3, "an empty query has to show everything: the list opens before the first letter");
}

/// A match at the start of the name comes first. Otherwise typing `len` puts an unrelated path at the top.
#[test]
fn a_name_prefix_outranks_a_path_match() {
    let mut p = Project::default();
    p.new_document();
    part_with_driver(&mut p, "lensuz", "Sketch", "dlina", 20.0);
    p.parameters.push(Param { name: "len".into(), expr: "5".into(), value: 5.0 });

    let hits = p.drivers_matching("len");
    assert_eq!(hits.len(), 2, "not both were found: {hits:?}");
    assert_eq!(hits[0].name, "len", "the one whose name starts with what was typed has to come first: {hits:?}");
}

/// A sketch without a timeline node is not passed off as global. An empty path means a project parameter, and
/// giving one to a dimension silently would suggest it is visible from everywhere.
#[test]
fn a_sketch_without_a_timeline_node_still_shows_its_name() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Orphan",
        vec![Point2::new(0.0, 0.0), Point2::new(12.0, 0.0), Point2::new(12.0, 5.0), Point2::new(0.0, 5.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance {
        a: pts[0],
        b: pts[1],
        d: 12.0,
        off: 0.0,
        expr: String::new(),
        driven: false,
        axis: 0,
    });
    assert!(p.add_named_dim("bez_uzla".into(), sid, vec![pts[0], pts[1]]));

    let d = p.drivers().into_iter().find(|d| d.name == "bez_uzla").expect("the driver is in the list");
    assert_eq!(d.path, "Orphan", "a sketch without a node still has its own name as a path");
    assert!(!d.path.is_empty(), "an empty path would claim a global parameter, which is untrue");
}
