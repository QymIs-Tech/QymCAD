//! AN EDGE KEEPS ITS NAME WHEN AN EDGE OF THE SAME PAIR OF FACES GOES. Edges between the same two faces were numbered
//! as the first number free in the order of the faces at their ends, and an edit that took one of them away renumbered
//! the others. Reported behaviour, measured on the case example: a chamfer on "the wall with the end cap, 2" took the
//! edge next to its own after an edit of the sketch, and refused. An edge is named by its pair and the faces at its
//! ends (`crates/qymcad-kernel/NAMING.md`, "What a name is").
//!
//! A block of width `w` (x from -w/2 to w/2), 20 deep (y from -10 to 10) and 10 high; one cut of three notches in the
//! front wall, at x -25..-20, -5..0 and 15..20, 5 into the block and 4 deep: the edge between the front wall and the top
//! is in four. `w` from 80 to 50 brings the left notch to the end of the block and the left piece of that edge goes; the
//! pieces between the notches do not move and keep their names. Live OCCT, headless.
use qymcad_core::feature::{Extent, Purpose, SketchPlane};
use qymcad_core::model::{CombineSpan, Id, Param, PlaneDef, Project, WorkPlane};

/// The name of the edge between the front wall and the top whose middle is at `x`.
fn front_top_edge(p: &Project, body: Id, x: f64) -> Option<u32> {
    p.regen_edges.get(&body)?.iter().find(|e| (e.mid[0] - x).abs() < 1e-6 && (e.mid[1] + 10.0).abs() < 1e-6 && (e.mid[2] - 10.0).abs() < 1e-6).map(|e| e.id)
}

/// How many pieces the edge between the front wall and the top is in.
fn pieces_of_the_front_top_edge(p: &Project, body: Id) -> usize {
    p.regen_edges.get(&body).map_or(0, |es| es.iter().filter(|e| e.dir[0].abs() > 0.99 && (e.mid[1] + 10.0).abs() < 1e-6 && (e.mid[2] - 10.0).abs() < 1e-6).count())
}

#[test]
fn the_edges_between_the_notches_keep_their_names_when_the_edge_beside_them_goes() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "w".into(), expr: "80".into(), value: 80.0 });
    let block = p.add_box(80.0, 20.0, 10.0);
    p.set_feat_dim(block, "dx", "w".into());
    let plane = p.add_plane(WorkPlane { origin: [0.0, 0.0, 10.0], def: PlaneDef::OffsetBase { base: qymcad_core::feature::BasePlane::XY, dist: 10.0 }, ..Default::default() });
    let si = p.new_sketch("notches");
    p.sketches[si].plane = SketchPlane::Datum(plane);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "notches");
    for x in [-25.0, -5.0, 15.0] {
        p.add_rect_entity(si, x, -15.0, x + 5.0, -5.0, Purpose::Real);
    }
    p.regen_sketch(si);
    let notches: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert_eq!(notches.len(), 3, "setup: the three notches are not three contours");
    let cut = p.add_combine_multi_op(block, sid, notches, CombineSpan { height: 0.0, down: 4.0, extent: Extent::default(), fill: &[] }, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the block is not built: {:?}", report.errors);
    assert_eq!(pieces_of_the_front_top_edge(&p, cut), 4, "setup: the edge between the front wall and the top is not in four");
    let between = [-12.5, 7.5];
    let before: Vec<u32> = between.iter().map(|x| front_top_edge(&p, cut, *x).expect("setup: an edge between the notches")).collect();

    p.parameters[0].expr = "50".into();
    assert!(p.eval_parameters().is_empty(), "the parameter did not take 50");
    p.mark_param_dependents_dirty_for("w");
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "the rebuild after the edit refused: {:?}", report.errors);
    assert_eq!(pieces_of_the_front_top_edge(&p, cut), 3, "setup: the left piece of the edge did not go");
    let mut renamed = Vec::new();
    for (x, was) in between.iter().zip(&before) {
        match front_top_edge(&p, cut, *x) {
            Some(now) if now == *was => {}
            Some(now) => renamed.push(format!("the edge at x = {x} was {}, is {}", p.names.describe(*was), p.names.describe(now))),
            None => renamed.push(format!("the edge at x = {x} is gone")),
        }
    }
    assert!(renamed.is_empty(), "edges that did not move were renamed:\n  {}", renamed.join("\n  "));
}
