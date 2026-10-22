//! THE SEAM OF A ROUND BODY KEEPS ITS NAME WHATEVER ORDER THE EDITS COME IN. Reported behaviour, measured on a
//! vacuum cleaner: the seam of a revolved wall was numbered as the first number free among the edges of its face with
//! itself, and an edge of another pair wore its number through the history; an edit that brought that history no more
//! renamed the seam (`[wall | wall]:1` to `:0`). A seam is numbered by where it comes from and where it stands
//! (`crates/qymcad-kernel/NAMING.md`, "What a name is").
//!
//! A cylinder of radius 10, `2 b` high, and a cut of the quarter beside its seam, `a` high: along the cut the seam line
//! runs between the cylinder and a wall of the cut, above it it is the seam. Measured before: that edge of the wall wore
//! the name of the seam ("the cylinder with itself") and the seam took the next number. Opened, then edited `a` then
//! `b`, and opened again and edited `b` then `a`: one edge wears the name of the seam, alike both ways. Live OCCT,
//! headless.
use qymcad_core::feature::{Extent, Purpose};
use qymcad_core::model::{CombineSpan, Id, Param, Project};

/// A cylinder of radius 10 on XY, `b` high, and a cut of the quarter beside its seam - the square x 0..15, y 0..15 - up
/// from XY by `a`: the wall of the cut in the plane y = 0 runs along the seam of the cylinder. Built; gives the cut.
fn tube(p: &mut Project) -> Id {
    p.new_document();
    for name in ["a", "b"] {
        p.parameters.push(Param { name: name.into(), expr: "20".into(), value: 20.0 });
    }
    let si = p.new_sketch("round");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "round");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, Purpose::Real);
    p.regen_sketch(si);
    let cylinder = p.add_extrude(sid, 40.0);
    p.set_feat_dim(cylinder, "height", "b * 2".into());
    let si = p.new_sketch("quarter");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "quarter");
    p.add_rect_entity(si, 0.0, 0.0, 15.0, 15.0, Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the quarter");
    let cut = p.add_combine_multi_op(cylinder, sid, vec![c], CombineSpan { height: 20.0, down: 0.0, extent: Extent::default(), fill: &[] }, 0);
    p.set_feat_dim(cut, "height", "a".into());
    cut
}

/// The names of the edges of `body` with one face on both sides, in order.
fn seams(p: &Project, body: Id) -> Vec<String> {
    let mut out: Vec<String> = p.regen_edges.get(&body).into_iter().flatten().filter(|e| p.names.edge(e.id).is_some_and(|n| n.faces[0] == n.faces[1])).map(|e| p.names.describe(e.id)).collect();
    out.sort();
    out
}

/// The document built, then the parameters set in the order given, each rebuilt as the Parameters window rebuilds it.
fn edited(order: [&str; 2]) -> Vec<String> {
    let mut p = Project::default();
    let body = tube(&mut p);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the tube is not built: {:?}", report.errors);
    for name in order {
        let i = p.parameters.iter().position(|q| q.name == name).expect("the parameter");
        p.parameters[i].expr = "22".into();
        assert!(p.eval_parameters().is_empty(), "the parameter {name} did not take 22");
        p.mark_param_dependents_dirty_for(name);
        let (report, again) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
        assert!(report.errors.is_empty(), "the rebuild after {name} refused: {:?}", report.errors);
        shapes = again;
    }
    seams(&p, body)
}

#[test]
fn the_seam_of_a_cylinder_is_named_once_and_alike_whatever_order_the_edits_come_in() {
    let one_way = edited(["a", "b"]);
    let other_way = edited(["b", "a"]);
    // one edge has the cylinder on both sides: the seam above the cut; along the cut the seam line runs between the
    // cylinder and the wall of the cut, an edge of that pair - wearing the name of the seam, it took the seam's number
    assert_eq!(one_way.len(), 1, "edges of another pair wear the name of the seam: {one_way:?}");
    assert_eq!(one_way, other_way, "the seams are named by the order of the edits");
}
