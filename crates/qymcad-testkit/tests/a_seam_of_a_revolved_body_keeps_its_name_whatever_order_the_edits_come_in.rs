//! THE SEAMS OF A REVOLVED BODY KEEP THEIR NAMES WHATEVER ORDER THE EDITS COME IN. A revolution closes its faces at the
//! plane its profile is drawn in, and a seam is numbered by where it comes from and where it stands, not as the first
//! number free (`crates/qymcad-kernel/NAMING.md`, "What a name is"); the same check on a cylinder is
//! `a_seam_keeps_its_name_whatever_order_the_edits_come_in`. Reported behaviour, measured on a vacuum cleaner: the seam of a
//! revolved wall held its name or not depending on the history of the body.
//!
//! A ring: the rectangle x 10..20, y 0..`b` on XY turned about the sketch's Y axis, and a cut of the quarter beside its
//! seams - the square x 0..25, z 0..25 on XZ, `a` to both sides: the walls of the cut in the plane z = 0 run along the
//! seams of the outer and the inner wall. Opened, then edited `a` then `b`, and opened again and edited `b` then `a`: the
//! seams are named alike both ways, one on each round wall. Live OCCT, headless.
use qymcad_core::feature::{BasePlane, Extent, Purpose, SketchPlane};
use qymcad_core::model::{CombineSpan, Id, Param, Project};

/// The ring, `b` = 30 long, and the cut of the quarter, `a` = 10 to both sides of XZ. Built; gives the cut.
fn ring(p: &mut Project) -> Id {
    p.new_document();
    p.parameters.push(Param { name: "a".into(), expr: "10".into(), value: 10.0 });
    p.parameters.push(Param { name: "b".into(), expr: "30".into(), value: 30.0 });
    let si = p.new_sketch("profile");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "profile");
    p.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the profile");
    let ring = p.add_revolve_axis(sid, vec![c], 1, 360.0, 0, 0);
    let low = p.sketches[si].points.iter().find(|q| q.y.abs() < 1e-9 && (q.x - 10.0).abs() < 1e-9).map(|q| q.id).expect("setup: the corner at the bottom");
    let high = p.sketches[si].points.iter().find(|q| (q.y - 30.0).abs() < 1e-9 && (q.x - 10.0).abs() < 1e-9).map(|q| q.id).expect("setup: the corner at the top");
    // the length of the ring, a dimension of the profile under the formula b
    p.sketches[si].constraints.push(qymcad_core::model::Constraint::Distance { a: low, b: high, d: 30.0, off: 0.0, expr: "b".into(), driven: false, axis: 2, at: None });
    p.solve_sketch(si);
    let si = p.new_sketch("quarter");
    p.sketches[si].plane = SketchPlane::World(BasePlane::XZ);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "quarter");
    p.add_rect_entity(si, 0.0, 0.0, 25.0, 25.0, Purpose::Real);
    p.regen_sketch(si);
    let c = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("the quarter");
    let cut = p.add_combine_multi_op(ring, sid, vec![c], CombineSpan { height: 10.0, down: 10.0, extent: Extent::default(), fill: &[] }, 0);
    p.set_feat_dim(cut, "height", "a".into());
    p.set_feat_dim(cut, "down", "a".into());
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
    let body = ring(&mut p);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the ring is not built: {:?}", report.errors);
    for name in order {
        let i = p.parameters.iter().position(|q| q.name == name).expect("the parameter");
        let value = p.parameters[i].value + 2.0;
        p.parameters[i].expr = qymcad_core::expr::fmt_num(value);
        assert!(p.eval_parameters().is_empty(), "the parameter {name} did not take {value}");
        p.mark_param_dependents_dirty_for(name);
        let (report, again) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
        assert!(report.errors.is_empty(), "the rebuild after {name} refused: {:?}", report.errors);
        shapes = again;
    }
    seams(&p, body)
}

#[test]
fn the_seams_of_a_ring_are_named_alike_whatever_order_the_edits_come_in() {
    let one_way = edited(["a", "b"]);
    let other_way = edited(["b", "a"]);
    assert_eq!(one_way.len(), 2, "the outer and the inner wall have one seam each: {one_way:?}");
    assert_eq!(one_way, other_way, "the seams are named by the order of the edits");
}
