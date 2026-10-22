//! A SPLINE IS A CURVE OF THE SKETCH like a line or a circle: an entity with an id of its own, standing on its nodes as a
//! line stands on its ends. Deleting other geometry leaves its nodes, deleting it takes them with it, and its tangent
//! handles hold the shape at a node.
//!
//! Reported behaviour (#166): deleting a circle in a sketch also deleted the control points of a spline in that sketch -
//! the sweep of points no entity used did not count a spline among the users of its points.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A point of the sheet by its two coordinates.
fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// Where the nodes of spline `eid` stand, `None` for a node that is gone or no such spline.
fn nodes(p: &Project, si: usize, eid: u64) -> Option<Vec<Point2>> {
    let s = &p.sketches[si];
    let nodes = s.entities.iter().find(|e| e.id == eid)?.kind.as_spline()?.nodes;
    nodes.iter().map(|id| s.points.iter().find(|q| q.id == *id).map(|q| Point2::new(q.x, q.y))).collect()
}

#[test]
fn deleting_a_circle_keeps_the_nodes_of_a_spline_beside_it() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let sp = p.add_spline(si, vec![p2([0.0, 0.0]), p2([10.0, 8.0]), p2([20.0, 0.0])], Ends::Open, Purpose::Real).expect("the spline");
    let circle = p.add_circle_entity(si, 50.0, 0.0, 5.0, Purpose::Real);
    p.delete_entities(si, &[circle]);
    assert_eq!(nodes(&p, si, sp), Some(vec![p2([0.0, 0.0]), p2([10.0, 8.0]), p2([20.0, 0.0])]), "the spline lost its nodes with the circle");
    assert!(p.spline_polyline(si, sp).len() > 3, "the spline is no longer drawn");
}

#[test]
fn deleting_a_spline_takes_its_nodes() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let sp = p.add_spline(si, vec![p2([0.0, 0.0]), p2([10.0, 8.0]), p2([20.0, 0.0])], Ends::Closed, Purpose::Real).expect("the spline");
    p.delete_entities(si, &[sp]);
    let unseen = p.sketches[si].unseen_points();
    let left = p.sketches[si].points.iter().filter(|q| !unseen.contains(&q.id)).count();
    assert!(p.sketches[si].entities.is_empty() && left == 0, "{} curves and {left} points stay after the spline is deleted", p.sketches[si].entities.len());
}

#[test]
fn a_handle_set_both_ways_bows_the_spline_and_reset_flattens_it() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let sp = p.add_spline(si, vec![p2([0.0, 0.0]), p2([10.0, 0.0]), p2([20.0, 0.0])], Ends::Open, Purpose::Real).expect("the spline");
    let high = |p: &Project| p.spline_polyline(si, sp).iter().map(|q| q.y.abs()).fold(0.0, f64::max);
    assert!(high(&p) < 1e-9, "three nodes on a line lay a straight spline");
    assert!(p.set_spline_handle(si, sp, 1, p2([16.0, 6.0])));
    let handles = p.spline_handles(si, sp);
    assert!(handles[1].end.dist(p2([16.0, 6.0])) < 1e-9, "the handle ends where it was dragged to: {:?}", handles[1]);
    assert!(high(&p) > 0.5, "a handle turned up out of the line bows the spline: {}", high(&p));
    assert!(p.reset_spline_handle(si, sp, 1));
    assert!(high(&p) < 1e-9, "a handle reset to automatic lays the spline straight again: {}", high(&p));
}
