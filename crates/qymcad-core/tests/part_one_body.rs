//! A part is one body. The policy lives in `finish_base_body`, which the interface calls after a material
//! feature; here it is called explicitly, in the role of the orchestrator. The first material feature is the
//! seed and the ones that follow are fused into a single body.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

fn square(p: &mut Project, name: &str) -> Id {
    let sid = p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, name);
    sid
}

/// A material feature as the interface makes it: an adder plus the single-body policy. Returns the resulting
/// body.
fn extrude(p: &mut Project, name: &str, h: f64) -> Id {
    let s = square(p, name);
    let b = p.add_extrude(s, h);
    p.finish_base_body(b, 1)
}

/// The live, unconsumed bodies of a part.
fn live_bodies(p: &Project, part: Id) -> Vec<Id> {
    let consumed = p.consumed_bodies();
    p.component_bodies(part).into_iter().filter(|b| !consumed.contains(b)).collect()
}

#[test]
fn first_feature_is_seed_single_body() {
    let mut p = Project::default();
    let part = p.new_document();
    let b = extrude(&mut p, "s", 5.0);
    // the first material feature is the seed: exactly one body, with no boolean
    assert_eq!(live_bodies(&p, part), vec![b], "the first feature is the seed, giving one body");
}

#[test]
fn two_extrudes_in_part_make_one_body() {
    let mut p = Project::default();
    let part = p.new_document();
    let b1 = extrude(&mut p, "s1", 5.0);
    let b2 = extrude(&mut p, "s2", 5.0);

    let live = live_bodies(&p, part);
    assert_eq!(live.len(), 1, "after two extrusions the part has one body: {live:?}");
    assert_eq!(live[0], b2, "the live body is the result of the fusion returned by finish_base_body");
    assert!(p.consumed_bodies().contains(&b1), "the first body was consumed by the fusion");
}

#[test]
fn primitive_after_extrude_merges_into_one_body() {
    let mut p = Project::default();
    let part = p.new_document();
    extrude(&mut p, "s", 5.0);
    let bx = p.add_box(4.0, 4.0, 4.0);
    let bx = p.finish_base_body(bx, 1);
    let live = live_bodies(&p, part);
    assert_eq!(live.len(), 1, "an extrusion plus a primitive gives one body: {live:?}");
    assert_eq!(live[0], bx, "the live one is the result of the fusion");
}

#[test]
fn three_material_features_still_one_body() {
    let mut p = Project::default();
    let part = p.new_document();
    extrude(&mut p, "s", 5.0);
    let cy = p.add_cylinder(3.0, 6.0);
    p.finish_base_body(cy, 1);
    extrude(&mut p, "s3", 2.0);
    assert_eq!(live_bodies(&p, part).len(), 1, "three material features give the part one body");
}

#[test]
fn separate_parts_keep_separate_bodies() {
    let mut p = Project::default();
    let part1 = p.new_document();
    extrude(&mut p, "s1", 5.0);
    // a second part
    let part2 = p.add_part("Part 2");
    p.set_active_component(Some(part2));
    extrude(&mut p, "s2", 5.0);
    // each part has one body of its own; they did not fuse across parts
    assert_eq!(live_bodies(&p, part1).len(), 1, "part 1 has one body");
    assert_eq!(live_bodies(&p, part2).len(), 1, "part 2 has one body of its own");
}

#[test]
fn finish_base_body_seed_returns_same_no_boolean() {
    // the first feature: `finish_base_body` creates no boolean for the seed and returns the same body
    let mut p = Project::default();
    p.new_document();
    let s = square(&mut p, "s");
    let b = p.add_extrude(s, 5.0);
    let n_before = p.timeline.len();
    let r = p.finish_base_body(b, 1);
    assert_eq!(r, b, "seed: the same body is returned");
    assert_eq!(p.timeline.len(), n_before, "seed: no boolean was added");
}
