//! A SWEEP RUNS ALONG AN EDGE OF A BODY, as it runs along a path drawn in a sketch, and follows the edge by its name when
//! the body is rebuilt: a square of 4 drawn on the side plane over the top front edge of a block 40 x 30 x 10, swept
//! along that edge and added to the block, lays a rail of 4 x 4 along it.
use qymcad_core::feature::{BasePlane, EdgePath, Purpose, SketchPlane};
use qymcad_core::errors::CoreError;
use qymcad_core::model::{Id, Project};

/// The block 40 x 30 x 10 a person draws: a rectangle from the origin on the table, extruded 10. Its body.
fn block(p: &mut Project) -> Id {
    p.new_document();
    let si = p.new_sketch("block");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "block");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 10.0);
    p.finish_base_body(body, 1)
}

/// The edge of the block that runs along X at y = 0 on its top, `top` high: the one the profile stands over.
fn top_front_edge(p: &Project, block: Id, top: f64) -> Option<u32> {
    p.regen_edges.get(&block)?.iter().find(|e| e.a[1].abs() < 1e-6 && e.b[1].abs() < 1e-6 && (e.a[2] - top).abs() < 1e-6 && (e.b[2] - top).abs() < 1e-6).map(|e| e.id)
}

#[test]
fn a_sweep_runs_along_an_edge_of_a_body() {
    let mut p = Project::default();
    let block = block(&mut p);
    let _ = qymcad_testkit::regenerate(&mut p);
    let edge = top_front_edge(&p, block, 10.0).unwrap_or_else(|| panic!("no top front edge among {:?}", p.regen_edges.get(&block).map(|es| es.iter().map(|e| [e.a, e.b]).collect::<Vec<_>>())));
    // the profile: a square of 4 on the side plane x = 0, sitting on the top of the block at its front, y 0..4, z 10..14
    let prof = p.new_sketch("profile");
    p.sketches[prof].plane = SketchPlane::World(BasePlane::YZ);
    let prof_sid = p.sketches[prof].id;
    p.add_sketch_node(prof_sid, "profile");
    p.add_rect_entity(prof, 0.0, 10.0, 4.0, 14.0, Purpose::Real);
    p.regen_sketch(prof);
    let rail = p.add_sweep_along(prof_sid, Vec::new(), EdgePath { body: block, edges: qymcad_core::refs::Ref::picks(&[edge]) }, block, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let mut fails = Vec::new();
    if !report.errors.is_empty() {
        fails.push(format!("the sweep along the edge went red: {:?}", report.errors));
    }
    // the block and the rail of 4 x 4 x 40 on it
    let v = shapes.get(&rail).map(|s| s.volume()).unwrap_or(0.0);
    if (v - 12640.0).abs() > 0.5 {
        fails.push(format!("the block with a rail of 4 x 4 along its top front edge holds 12640, and the body holds {v:.1}"));
    }
    // the block made 12 tall: the edge rises with the top, keeps its name, and the path still runs along it from x = 0;
    // the rail stays where the square is drawn, 2 of it now standing above the top
    for n in p.timeline.iter_mut() {
        if let qymcad_core::feature::FeatureKind::Extrude { height, .. } = &mut n.kind {
            *height = 12.0;
            n.dirty = true;
        }
    }
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    if !report.errors.is_empty() {
        fails.push(format!("the block made taller, the sweep along its edge went red: {:?}", report.errors));
    }
    let v = shapes.get(&rail).map(|s| s.volume()).unwrap_or(0.0);
    if (v - (14400.0 + 320.0)).abs() > 0.5 {
        fails.push(format!("the block 12 tall with the rail standing 2 above it holds 14720, and the body holds {v:.1}"));
    }
    assert!(fails.is_empty(), "\n{}", fails.join("\n"));
}

#[test]
fn edges_that_stand_apart_are_refused_as_a_path() {
    let mut p = Project::default();
    let block = block(&mut p);
    let _ = qymcad_testkit::regenerate(&mut p);
    let front = top_front_edge(&p, block, 10.0).expect("the top front edge of the block");
    // the bottom back edge: it touches the front one nowhere
    let back = p
        .regen_edges
        .get(&block)
        .and_then(|es| es.iter().find(|e| (e.a[1] - 30.0).abs() < 1e-6 && (e.b[1] - 30.0).abs() < 1e-6 && e.a[2].abs() < 1e-6 && e.b[2].abs() < 1e-6).map(|e| e.id))
        .expect("the bottom back edge");
    let prof = p.new_sketch("profile");
    p.sketches[prof].plane = SketchPlane::World(BasePlane::YZ);
    let prof_sid = p.sketches[prof].id;
    p.add_sketch_node(prof_sid, "profile");
    p.add_rect_entity(prof, 0.0, 10.0, 4.0, 14.0, Purpose::Real);
    p.regen_sketch(prof);
    let rail = p.add_sweep_along(prof_sid, Vec::new(), EdgePath { body: block, edges: qymcad_core::refs::Ref::picks(&[front, back]) }, block, 1);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    let said: Vec<_> = report.errors.iter().filter(|e| e.0 == rail).map(|e| e.1.clone()).collect();
    assert_eq!(said, vec![CoreError::PathEdgesApart], "two edges that touch nowhere are refused as a path that does not join into one chain");
}

/// The square of 4 over the top front edge of the block, on the side plane x = 0: the profile of a rail.
fn square_over_the_front(p: &mut Project) -> Id {
    let prof = p.new_sketch("profile");
    p.sketches[prof].plane = SketchPlane::World(BasePlane::YZ);
    let prof_sid = p.sketches[prof].id;
    p.add_sketch_node(prof_sid, "profile");
    p.add_rect_entity(prof, 0.0, 10.0, 4.0, 14.0, Purpose::Real);
    p.regen_sketch(prof);
    prof_sid
}

/// AN EDGE NAMED TWICE IS ONE EDGE OF THE PATH: the selection of a person may hold it twice - a click on the edge, then
/// on a face that has it - and the rail is laid along it as along the edge named once.
#[test]
fn an_edge_named_twice_is_one_edge_of_the_path() {
    let mut p = Project::default();
    let block = block(&mut p);
    let _ = qymcad_testkit::regenerate(&mut p);
    let edge = top_front_edge(&p, block, 10.0).expect("the top front edge of the block");
    let prof_sid = square_over_the_front(&mut p);
    let rail = p.add_sweep_along(prof_sid, Vec::new(), EdgePath { body: block, edges: qymcad_core::refs::Ref::picks(&[edge, edge]) }, block, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let v = shapes.get(&rail).map(|s| s.volume()).unwrap_or(0.0);
    assert!(report.errors.is_empty() && (v - 12640.0).abs() < 0.5, "the edge named twice: the body holds {v:.1}, not the block with its rail, 12640; red: {:?}", report.errors);
}

/// THE BODY OF THE EDGES IS NAMED ONCE AMONG THE INPUTS of a sweep that is added to that same body.
#[test]
fn the_body_of_the_edges_is_an_input_once() {
    let mut p = Project::default();
    let block = block(&mut p);
    let _ = qymcad_testkit::regenerate(&mut p);
    let edge = top_front_edge(&p, block, 10.0).expect("the top front edge of the block");
    let prof_sid = square_over_the_front(&mut p);
    let rail = p.add_sweep_along(prof_sid, Vec::new(), EdgePath { body: block, edges: qymcad_core::refs::Ref::picks(&[edge]) }, block, 1);
    let inputs = p.timeline.iter().find(|n| n.id == rail).map(|n| n.kind.inputs()).unwrap_or_default();
    assert_eq!(inputs.iter().filter(|i| **i == block).count(), 1, "the block is named {inputs:?} among the inputs of the rail laid on it");
}

/// A PATH THAT LOSES AN EDGE TO AN EDIT ABOVE IS REFUSED IN WORDS, not swept along what is left: the path runs along the
/// top front edge of a block notched at its front right corner, and on along the top edge of the notch; the notch cut
/// only 5 deep from below, its top edge is gone, and the sweep says it lost 1 of its 2 edges.
#[test]
fn a_path_that_loses_an_edge_upstream_is_refused_in_words() {
    let mut p = Project::default();
    let block = block(&mut p);
    let notch = p.new_sketch("notch");
    let notch_sid = p.sketches[notch].id;
    p.add_sketch_node(notch_sid, "notch");
    p.add_rect_entity(notch, 30.0, 0.0, 40.0, 10.0, Purpose::Real);
    p.regen_sketch(notch);
    let notched = p.add_combine(block, notch_sid, 10.0, 0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let top = |e: &&qymcad_core::geom::MeshEdge| (e.a[2] - 10.0).abs() < 1e-6 && (e.b[2] - 10.0).abs() < 1e-6;
    let edges = p.regen_edges.get(&notched).cloned().unwrap_or_default();
    let front = edges.iter().filter(top).find(|e| e.a[1].abs() < 1e-6 && e.b[1].abs() < 1e-6).map(|e| e.id).expect("the top front edge of the notched block");
    let side = edges.iter().filter(top).find(|e| (e.a[0] - 30.0).abs() < 1e-6 && (e.b[0] - 30.0).abs() < 1e-6).map(|e| e.id).expect("the top edge of the notch at x = 30");
    let prof_sid = square_over_the_front(&mut p);
    let rail = p.add_sweep_along(prof_sid, Vec::new(), EdgePath { body: notched, edges: qymcad_core::refs::Ref::picks(&[front, side]) }, notched, 1);
    let _ = qymcad_testkit::regenerate(&mut p);
    for n in p.timeline.iter_mut() {
        if let qymcad_core::feature::FeatureKind::Combine { height, .. } = &mut n.kind {
            *height = 5.0;
            n.dirty = true;
        }
    }
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    let said: Vec<_> = report.errors.iter().filter(|e| e.0 == rail).map(|e| e.1.clone()).collect();
    let want = CoreError::PathEdgesLost { asked: 2, lost: 1 };
    assert_eq!(said, vec![want.clone()], "the notch cut 5 deep: the path lost the top edge of the notch");
    assert_eq!(want.key(), "error-path-edges-lost", "the loss is said by the words of a path that lost edges");
}
