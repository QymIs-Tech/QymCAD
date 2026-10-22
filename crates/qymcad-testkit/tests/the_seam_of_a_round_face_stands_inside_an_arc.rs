//! THE SEAM OF A ROUND FACE MADE OF ARCS STANDS INSIDE AN ARC, not where two arcs met. A circle the lines of its sketch
//! cut into arcs comes out of an extrusion as one round face, and its seam stood on one of the points where the arcs
//! met - where the other lines touch the circle, and where the walls of later steps come to it. A rounding of an edge
//! there met the seam going straight on from its end and was refused at any radius. Reported behaviour, measured on a
//! vacuum cleaner: of eight edges of a rounding R8.6 one was left out after an edit moved the seam of a round post onto
//! its edge. The seam stands in the middle of the arc first by the recipe (`crates/qymcad-kernel/NAMING.md`, "What a
//! name is"), and moves within it when an edit moves the points where the arcs meet.
//!
//! A post: the circle of radius `R` about the origin, cut by the line x = 2 into two regions, both extruded 6 high. A
//! boss x 2..20, y -20..20, 4 high, stands against it: its wall x = 2 meets the post along the lines through the points
//! where the line cut the circle. Both of those edges are rounded R1. `R` from 5 to 5.5 moves the points along the
//! line. Live OCCT, headless.
use qymcad_core::feature::{Extent, FeatureKind, Purpose};
use qymcad_core::model::{CombineSpan, Constraint, Id, Param, Project};

/// The post, the boss and the rounding of the two edges where the boss meets the post. Built; gives the rounding.
fn post_and_boss(p: &mut Project) -> Id {
    p.new_document();
    p.parameters.push(Param { name: "R".into(), expr: "5".into(), value: 5.0 });
    let si = p.new_sketch("post");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "post");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, Purpose::Real);
    p.add_line_entity(si, 2.0, -10.0, 2.0, 10.0, Purpose::Real);
    let centre = p.sketches[si].points.iter().find(|q| q.x.abs() < 1e-9 && q.y.abs() < 1e-9).map(|q| q.id).expect("setup: the centre of the circle");
    p.sketches[si].constraints.push(Constraint::Diameter { c: centre, d: 5.0, off: 0.0, expr: "R".into(), driven: false, diam: false, at: None });
    p.solve_sketch(si);
    p.regen_sketch(si);
    let regions: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert_eq!(regions.len(), 2, "setup: the line does not cut the circle into two regions");
    let post = p.add_extrude_multi(sid, regions, 6.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    p.finish_base_body(post, 1);
    let si = p.new_sketch("boss");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "boss");
    p.add_rect_entity(si, 2.0, -20.0, 20.0, 20.0, Purpose::Real);
    p.regen_sketch(si);
    let region: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let boss = p.add_combine_multi_op(post, sid, region, CombineSpan { height: 4.0, down: 0.0, extent: Extent::default(), fill: &[] }, 1);
    let (report, _) = qymcad_testkit::regenerate(&mut p.clone());
    assert!(report.errors.is_empty(), "setup: the post and the boss do not build: {:?}", report.errors);
    let _ = qymcad_testkit::regenerate(p);
    let meet = meeting_edges(p, boss);
    assert_eq!(meet.len(), 2, "setup: the boss meets the post along {} edges, not 2", meet.len());
    p.add_fillet(boss, 1.0, meet)
}

/// The upright edges of `body` along x = 2 beside the post, where the wall of the boss meets it.
fn meeting_edges(p: &Project, body: Id) -> Vec<u32> {
    p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.dir[2].abs() > 0.99 && (e.mid[0] - 2.0).abs() < 1e-6 && e.mid[1].abs() < 10.0 && (e.mid[2] - 2.0).abs() < 1e-6).map(|e| e.id).collect()
}

/// Where the seams of the round faces of `body` stand: the middle of each edge with one face on both sides.
fn seams(p: &Project, body: Id) -> Vec<[f64; 3]> {
    p.regen_edges.get(&body).into_iter().flatten().filter(|e| p.names.edge(e.id).is_some_and(|n| n.faces[0] == n.faces[1])).map(|e| e.mid.map(f64::from)).collect()
}

/// What is wrong with the seams of the post and the rounding at `r`: a seam on the line x = 2, where the arcs met, and
/// the rounding refused or leaving an edge out.
fn faults(p: &Project, round: Id, r: f64) -> Vec<String> {
    let mut out = Vec::new();
    let post = p.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Extrude { .. })).and_then(|n| n.kind.body()).unwrap_or(0);
    for s in seams(p, post) {
        if (s[0] - 2.0).abs() < 1e-6 {
            out.push(format!("R = {r}: the seam of the post stands where the arcs met, at ({:.3}, {:.3})", s[0], s[1]));
        }
    }
    if let Some(e) = p.regen_errors.get(&round) {
        out.push(format!("R = {r}: the rounding refused: {e:?}"));
    }
    if let Some(w) = p.regen_warnings.get(&round) {
        out.push(format!("R = {r}: the rounding left edges out: {w:?}"));
    }
    out
}

#[test]
fn the_seam_of_the_post_stands_inside_an_arc_and_the_edge_at_a_joint_is_rounded() {
    let mut p = Project::default();
    let round = post_and_boss(&mut p);
    let (_, shapes) = qymcad_testkit::regenerate(&mut p);
    let mut out = faults(&p, round, 5.0);
    p.parameters[0].expr = "5.5".into();
    assert!(p.eval_parameters().is_empty(), "the parameter did not take 5.5");
    p.mark_param_dependents_dirty_for("R");
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    out.extend(faults(&p, round, 5.5));
    assert!(out.is_empty(), "{}", out.join("\n"));
}
