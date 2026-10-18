//! A CORNER PATCH OF A ROUNDING IS NAMED AFTER ITS VERTEX: the end of the edge of the lowest place in the pick among
//! those meeting there. Named after the edge alone, the two corners at the two ends of one edge came out "corner" and
//! "piece 1 of corner", the piece told by the order the vertices were walked in - and a corner patch is what a mode of
//! the corner (rolling ball, setback) will change, which must not rename the patch at the other end.
use qymcad_core::model::{Id, Project};
use qymcad_core::names::Role;

/// The id of the edge of `body` whose middle stands nearest `at`, within 0.2 mm.
fn edge_near(p: &Project, body: Id, at: [f64; 3]) -> Option<u32> {
    let d = |m: [f64; 3]| (0..3).map(|i| (m[i] - at[i]).powi(2)).sum::<f64>().sqrt();
    p.regen_edges.get(&body)?.iter().filter(|e| d(e.mid) < 0.2).min_by(|a, b| d(a.mid).total_cmp(&d(b.mid))).map(|e| e.id)
}

/// A CORNER PATCH of the rounding: its descriptor and its name.
struct Patch {
    id: u32,
    name: qymcad_core::names::GeoName,
}

#[test]
fn the_corners_at_the_two_ends_of_one_edge_are_two_names() {
    let mut p = Project::default();
    p.new_document();
    let cube = p.add_box(10.0, 10.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the front top edge first, then the two side top edges and the two front uprights meeting it at its two ends: three
    // roundings at each end, a patch at each
    let edges: Vec<u32> = [[0.0, -5.0, 10.0], [5.0, 0.0, 10.0], [-5.0, 0.0, 10.0], [5.0, -5.0, 5.0], [-5.0, -5.0, 5.0]].iter().map(|at| edge_near(&p, cube, *at).expect("setup: a top edge")).collect();
    let node = p.add_fillet(cube, 2.0, edges);
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!(!p.regen_errors.contains_key(&node), "setup: the rounding does not build: {:?}", p.regen_errors.get(&node));
    let corners: Vec<Patch> = p.regen_faces.get(&node).into_iter().flatten().filter_map(|f| p.names.get(f.id).filter(|n| n.role == Role::Corner).map(|name| Patch { id: f.id, name })).collect();
    assert!(corners.len() >= 2, "setup: the two corners of the front edge have no patch: {:?}", corners.iter().map(|c| p.names.describe(c.id)).collect::<Vec<_>>());
    let pieces: Vec<String> = corners.iter().filter(|c| c.name.split != 0).map(|c| p.names.describe(c.id)).collect();
    assert!(pieces.is_empty(), "a corner patch is named a piece of another, by the order of the walk: {pieces:?}");
    let mut sources: Vec<u64> = corners.iter().map(|c| c.name.src).collect();
    sources.sort_unstable();
    sources.dedup();
    assert_eq!(sources.len(), corners.len(), "two corner patches under one name: {:?}", corners.iter().map(|c| p.names.describe(c.id)).collect::<Vec<_>>());
}
