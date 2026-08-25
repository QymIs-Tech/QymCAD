//! A TANGENT CHAIN AGAINST THE LIVE KERNEL.
//!
//! The unit tests in `qymcad-core/src/refs` compute smoothness on invented segments and arcs. Here
//! the same thing goes through real OCCT: a rounded vertical corner turns the edge of the top face
//! into "line, arc, line", and the chain must run through it without stumbling on the arc.
//!
//! Without this, moving the chain onto a query is proved only by the fact that it compiles.
use qymcad_core::geom::{MeshEdge, Point2};
use qymcad_core::model::Project;

/// A 60x40x12 plate with one rounded vertical corner; returns (project, body).
fn plate_with_a_rounded_corner() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let plate = p.add_extrude_multi(sid, closed, 12.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    // fillet ONE vertical corner: the edge of the top face becomes "line, arc, line"
    let vert = p.regen_edges[&plate]
        .iter()
        .filter(|e| (e.a[2] - e.b[2]).abs() > 1.0) // vertical
        .min_by(|a, b| (a.mid[0] + a.mid[1]).total_cmp(&(b.mid[0] + b.mid[1])))
        .expect("a vertical edge")
        .id;
    let rounded = p.add_fillet_ref(plate, 6.0, qymcad_core::refs::Ref::picks(&[vert]));
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: the corner must round: {:?}", rep.errors);
    (p, rounded)
}

/// The edges of the top face of a body.
fn top_edges(p: &Project, body: u64) -> Vec<MeshEdge> {
    let top = p.regen_edges[&body].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
    p.regen_edges[&body].iter().filter(|e| (e.a[2] - top).abs() < 1e-6 && (e.b[2] - top).abs() < 1e-6).cloned().collect()
}

/// THE CHAIN RUNS THROUGH THE ARC and stops at the sharp corners.
#[test]
fn a_chain_runs_through_the_rounded_corner_and_stops_at_the_sharp_ones() {
    let (p, body) = plate_with_a_rounded_corner();
    let edges = top_edges(&p, body);
    assert_eq!(edges.len(), 5, "the top face must have 4 straight pieces and an arc: {}", edges.len());

    // the seed is the arc: from it the chain must run into both adjoining straight pieces
    let arc = edges.iter().find(|e| e.radius > 1e-9).expect("the fillet arc");
    let q = qymcad_core::refs::Ref::many(qymcad_core::refs::Query::TangentChain {
        seed: Box::new(qymcad_core::refs::Query::Id(arc.id)),
        tol_deg: 5.0,
    });
    let got = p.resolve_edge_refs(body, &q, "ref-what-fillet-edge").expect("the chain resolved");
    assert_eq!(got.len(), 3, "the arc plus the two straight pieces it blends: it came out {} — {got:?}", got.len());

    // and the chain ends at the sharp corners: the fourth and fifth edges did not join it
    let outside: Vec<u32> = edges.iter().map(|e| e.id).filter(|id| !got.contains(id)).collect();
    assert_eq!(outside.len(), 2, "the two far edges must stay outside the chain: {outside:?}");
}

/// AND IT WORKS AS A REFERENCE: a fillet on the chain builds and takes more than one edge.
#[test]
fn a_fillet_built_on_a_chain_takes_the_whole_chain() {
    let (mut p, body) = plate_with_a_rounded_corner();
    let arc = top_edges(&p, body).into_iter().find(|e| e.radius > 1e-9).expect("the arc").id;
    let before = p.regen_edges[&body].len();

    let q = qymcad_core::refs::Ref::many(qymcad_core::refs::Query::TangentChain {
        seed: Box::new(qymcad_core::refs::Query::Id(arc)),
        tol_deg: 5.0,
    });
    let node = p.add_fillet_ref(body, 1.0, q);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a fillet on a chain must build: {:?}", rep.errors);
    let s = shapes.get(&node).expect("the body was built");
    assert!(s.is_valid(), "the body must pass kernel validation");

    // THREE edges were filleted rather than one: there are noticeably more edges now
    let after = p.regen_edges[&node].len();
    assert!(after > before + 2, "the whole chain was filleted: there were {before} edges, now {after}");
}
