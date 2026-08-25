//! A query-based reference is judged by whether it survives an edit, not by whether it compiles.
//!
//! The central test is "a fifth side appeared in the sketch": the query picks up the new wall on its own, while
//! a list of numbers silently stays four elements long. Without that test the rest only prove that the code
//! builds.
use super::*;
use crate::names::{EdgeName, GeoName, NameTable, Role};

/// Resolving against faces, where the same pool serves in both roles.
fn resolve_f(q: &Query, pool: &[Candidate], names: &NameTable) -> Vec<u32> {
    resolve(q, pool, names, pool)
}

/// The fixture: a body from two features, each with its own named faces.
fn stand() -> (NameTable, Vec<Candidate>, crate::model::Id, crate::model::Id) {
    let mut names = NameTable::default();
    let (f_base, f_hole) = (7, 9);
    let mut pool = Vec::new();
    let mut put = |names: &mut NameTable, name: GeoName, centroid: [f64; 3], normal: [f64; 3], area: f64| {
        let desc = names.intern_face(name);
        pool.push(Candidate { desc, centroid, normal, area, edge: None });
        desc
    };
    // the extrusion: two caps and four walls
    put(&mut names, GeoName::new(f_base, Role::CapEnd, 0), [0.0, 0.0, 10.0], [0.0, 0.0, 1.0], 600.0);
    put(&mut names, GeoName::new(f_base, Role::CapStart, 0), [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], 600.0);
    put(&mut names, GeoName::new(f_base, Role::Wall, 101), [15.0, 0.0, 5.0], [1.0, 0.0, 0.0], 200.0);
    put(&mut names, GeoName::new(f_base, Role::Wall, 102), [-15.0, 0.0, 5.0], [-1.0, 0.0, 0.0], 200.0);
    put(&mut names, GeoName::new(f_base, Role::Wall, 103), [0.0, 10.0, 5.0], [0.0, 1.0, 0.0], 300.0);
    put(&mut names, GeoName::new(f_base, Role::Wall, 104), [0.0, -10.0, 5.0], [0.0, -1.0, 0.0], 300.0);
    // the hole: a cylinder and a floor
    put(&mut names, GeoName::new(f_hole, Role::Hole, 0), [0.0, 0.0, 5.0], [1.0, 0.0, 0.0], 90.0);
    put(&mut names, GeoName::new(f_hole, Role::Hole, 1), [0.0, 0.0, 2.0], [0.0, 0.0, -1.0], 30.0);
    (names, pool, f_base, f_hole)
}

/// The query for the faces of a feature finds exactly its faces.
#[test]
fn a_query_finds_exactly_what_the_feature_made() {
    let (names, pool, base, hole) = stand();
    let all_base = resolve_f(&Query::OfFeature { feature: base, role: None }, &pool, &names);
    assert_eq!(all_base.len(), 6, "the extrusion has two caps and four walls");
    let walls = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::Wall) }, &pool, &names);
    assert_eq!(walls.len(), 4, "there are four walls");
    let holes = resolve_f(&Query::OfFeature { feature: hole, role: None }, &pool, &names);
    assert_eq!(holes.len(), 2, "the hole has a cylinder and a floor");
}

/// Descriptive queries work off the live geometry rather than off names.
#[test]
fn descriptive_queries_read_the_geometry_itself() {
    let (names, pool, _, _) = stand();
    let up = resolve_f(&Query::Oriented { dir: [0.0, 0.0, 1.0], tol_deg: 10.0 }, &pool, &names);
    assert_eq!(up.len(), 1, "only the top cap faces upwards");
    let top = resolve_f(&Query::Extreme { axis: Axis::Z, max: true }, &pool, &names);
    assert_eq!(top.len(), 1, "there is a single topmost face");
    let big = resolve_f(&Query::Largest, &pool, &names);
    assert_eq!(big.len(), 2, "two caps of 600 are both the largest, and claiming a single one would be a lie");
}

/// Sets add up and subtract.
#[test]
fn sets_combine_the_way_sets_should() {
    let (names, pool, base, hole) = stand();
    let both = Query::Union(
        Box::new(Query::OfFeature { feature: base, role: Some(Role::Wall) }),
        Box::new(Query::OfFeature { feature: hole, role: None }),
    );
    assert_eq!(resolve_f(&both, &pool, &names).len(), 6, "four walls plus the two faces of the hole");

    // "every face of the base except the top one" is the most common query there is
    let sides = Query::Minus(
        Box::new(Query::OfFeature { feature: base, role: None }),
        Box::new(Query::Oriented { dir: [0.0, 0.0, 1.0], tol_deg: 10.0 }),
    );
    assert_eq!(resolve_f(&sides, &pool, &names).len(), 5, "one top face left out of six");

    let vertical_walls = Query::Filter(
        Box::new(Query::OfFeature { feature: base, role: Some(Role::Wall) }),
        Box::new(Query::Oriented { dir: [1.0, 0.0, 0.0], tol_deg: 10.0 }),
    );
    assert_eq!(resolve_f(&vertical_walls, &pool, &names).len(), 1, "there is a single wall facing +X");
}

/// Ambiguity is a refusal, not a pick of the first candidate.
///
/// This rule is what the whole design exists for: silently taking the first match is precisely the behaviour
/// that moved a fillet onto a neighbouring edge.
#[test]
fn ambiguity_is_refused_instead_of_guessed() {
    let (names, pool, base, _) = stand();
    let r = Ref { query: Query::OfFeature { feature: base, role: Some(Role::Wall) }, expect: Cardinality::One, hint: Fingerprint::default() };
    match r.resolve("ref-what-fillet-edge", &pool, &names, &pool) {
        Err(RefError::Ambiguous { found, .. }) => assert_eq!(found, 4, "a refusal has to state how many were found"),
        other => panic!("expected a refusal for ambiguity, got {other:?}"),
    }
    // with a cardinality of "however many" the same reference is legitimate
    let r = Ref { query: Query::OfFeature { feature: base, role: Some(Role::Wall) }, expect: Cardinality::Any, hint: Fingerprint::default() };
    assert_eq!(r.resolve("ref-what-walls", &pool, &names, &pool).expect("the set is legitimate").len(), 4);
}

/// A loss is a refusal with an explanation, not a quiet match against something similar.
#[test]
fn a_lost_reference_says_what_it_was_looking_for() {
    let (names, pool, _, _) = stand();
    let hint = Fingerprint { centroid: [12.0, 3.0, 40.0], normal: [0.0, 0.0, 1.0] };
    let r = Ref { query: Query::Id(0xDEAD), expect: Cardinality::One, hint };
    match r.resolve("ref-what-hole-face", &pool, &names, &pool) {
        Err(e @ RefError::Lost { .. }) => {
            assert_eq!(e.key(), "ref-lost", "the kind of refusal has to be named by a catalogue key");
            assert_eq!(e.what(), "ref-what-hole-face", "a refusal has to remember what was sought");
            let RefError::Lost { was, .. } = &e else { unreachable!() };
            assert_eq!(was.centroid[0], 12.0, "and where that geometry was at the moment it was picked");
        }
        other => panic!("expected a loss, got {other:?}"),
    }
}

/// The point of it all: a set grows with the model and a list of numbers does not.
///
/// A scene from real work: chamfer every wall of this extrusion. Later a fifth side is drawn into the sketch.
/// The list of numbers silently stays four elements long, the new wall goes unchamfered, and nothing says so.
/// The query picks up the fifth one by itself.
///
/// That is the real cost of a reference by number. An earlier version of this test tried to show the number
/// being lost against a fresh name table, and showed something else instead: the number was not lost but
/// silently found someone else's face, because the descriptors in the second table had been issued anew. That
/// scene was unrealistic — a name table lives in the document and only grows — but the lesson stands: the
/// danger is not the loss, it is the quiet substitution.
#[test]
fn a_query_grows_with_the_model_while_a_list_of_ids_does_not() {
    let (mut names, mut pool, base, _) = stand();
    let walls_now = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::Wall) }, &pool, &names);
    assert_eq!(walls_now.len(), 4);

    // a reference as a list of numbers: exactly what features carry today in `edges: Vec<u32>`
    let by_ids = walls_now.iter().fold(Query::Id(walls_now[0]), |acc, &d| Query::Union(Box::new(acc), Box::new(Query::Id(d))));
    let by_ids = Ref { query: by_ids, expect: Cardinality::Any, hint: Fingerprint::default() };
    // a reference as a query: every wall of this feature
    let by_query = Ref::many(Query::OfFeature { feature: base, role: Some(Role::Wall) });

    // an edit earlier in the timeline: a fifth side appeared in the sketch and the feature produced another
    // wall
    let desc = names.intern_face(GeoName::new(base, Role::Wall, 105));
    pool.push(Candidate { desc, centroid: [8.0, 8.0, 5.0], normal: [0.7, 0.7, 0.0], area: 150.0, edge: None });

    let ids = by_ids.resolve("ref-what-chamfer-edges", &pool, &names, &pool).expect("the list of numbers resolves");
    let q = by_query.resolve("ref-what-chamfer-edges", &pool, &names, &pool).expect("the query resolves");
    assert_eq!(ids.len(), 4, "the list of numbers did not see the new wall, and said nothing about it");
    assert_eq!(q.len(), 5, "the query has to pick up the new wall as well");
}

/// And the other half: when the source entity is gone, the query refuses honestly.
///
/// A query does not find something similar; it either finds by recipe or reports that it did not. That is
/// exactly what fingerprint matching lacked.
#[test]
fn a_query_refuses_instead_of_finding_something_similar() {
    let (names, pool, _, _) = stand();
    let gone = Ref {
        query: Query::FromSource { src: 999 },
        expect: Cardinality::One,
        hint: Fingerprint { centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0] },
    };
    assert!(
        matches!(gone.resolve("ref-what-cut-wall", &pool, &names, &pool), Err(RefError::Lost { .. })),
        "with the source entity gone there has to be a refusal, not the nearest similar face"
    );
}

/// Edges are described through their faces, an edge having no recipe of its own.
///
/// Two queries are what this is for: every edge of this face, for filleting a rim, and where these two sets
/// meet, for filleting the junction of a boss with a plate. Neither is expressible as a list of numbers: trim
/// the face and there are more edges, while the list stays as it was.
#[test]
fn edges_are_described_through_the_faces_they_separate() {
    let (mut names, faces, base, hole) = stand();
    // edges are named by a pair of faces; here the top cap of the base and the walls
    let top = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::CapEnd) }, &faces, &names)[0];
    let walls = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::Wall) }, &faces, &names);
    let hole_faces = resolve_f(&Query::OfFeature { feature: hole, role: None }, &faces, &names);

    let mut edges: Vec<Candidate> = Vec::new();
    let mut put = |names: &mut NameTable, a: u32, b: u32, mid: [f64; 3]| {
        let desc = names.intern_edge(EdgeName::new(a, b, 0));
        edges.push(Candidate { desc, centroid: mid, normal: [1.0, 0.0, 0.0], area: 10.0, edge: None });
        desc
    };
    // the four edges of the top rim, where the cap meets the walls
    for (i, &w) in walls.iter().enumerate() {
        put(&mut names, top, w, [i as f64, 0.0, 10.0]);
    }
    // and one edge at the mouth of the hole, where the cap meets the cylinder
    let mouth = put(&mut names, top, hole_faces[0], [0.0, 0.0, 10.0]);

    let all_top = resolve(&Query::Adjacent(Box::new(Query::Id(top))), &edges, &names, &faces);
    assert_eq!(all_top.len(), 5, "the top cap has four rim edges plus the mouth of the hole");

    // where the cap meets the hole: the mouth alone, with none of the outer rim
    let seam = Query::Between(
        Box::new(Query::Id(top)),
        Box::new(Query::OfFeature { feature: hole, role: None }),
    );
    let found = resolve(&seam, &edges, &names, &faces);
    assert_eq!(found, vec![mouth], "the junction has to yield exactly the mouth of the hole: {found:?}");
}

/// A set of edges grows with the body too.
///
/// The same story as with the walls: trim a face and there are more edges. A list of numbers never learns of it
/// and says nothing, while "every edge of this face" picks the new one up by itself.
#[test]
fn an_edge_set_grows_when_the_face_gains_a_new_edge() {
    let (mut names, faces, base, _) = stand();
    let top = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::CapEnd) }, &faces, &names)[0];
    let walls = resolve_f(&Query::OfFeature { feature: base, role: Some(Role::Wall) }, &faces, &names);

    let mut edges: Vec<Candidate> = Vec::new();
    for (i, &w) in walls.iter().enumerate() {
        let desc = names.intern_edge(EdgeName::new(top, w, 0));
        edges.push(Candidate { desc, centroid: [i as f64, 0.0, 10.0], normal: [1.0, 0.0, 0.0], area: 10.0, edge: None });
    }
    let by_ids = Ref::picks(&edges.iter().map(|c| c.desc).collect::<Vec<_>>());
    let by_query = Ref::many(Query::Adjacent(Box::new(Query::Id(top))));

    // the edit: a fifth edge appeared on the cap because the face was trimmed
    let new_wall = names.intern_face(GeoName::new(base, Role::Wall, 105));
    let mut faces2 = faces.clone();
    faces2.push(Candidate { desc: new_wall, centroid: [8.0, 8.0, 5.0], normal: [0.7, 0.7, 0.0], area: 150.0, edge: None });
    let desc = names.intern_edge(EdgeName::new(top, new_wall, 0));
    edges.push(Candidate { desc, centroid: [9.0, 9.0, 10.0], normal: [1.0, 0.0, 0.0], area: 10.0, edge: None });

    let ids = by_ids.resolve("ref-what-fillet-edge", &edges, &names, &faces2).expect("the list resolves");
    let q = by_query.resolve("ref-what-fillet-edge", &edges, &names, &faces2).expect("the query resolves");
    assert_eq!(ids.len(), 4, "the list of numbers did not see the new edge");
    assert_eq!(q.len(), 5, "the query has to pick up the new edge as well");
}

// ── tangent chain ────────────────────────────────────────────────────────────────────────────────
//
// Click one edge and the whole run around the part comes with it. A list cannot express that: trim the shape
// and the chain is different while the list stays as it was.

/// A straight edge in the pool.
fn seg(pool: &mut Vec<Candidate>, desc: u32, a: [f64; 3], b: [f64; 3]) {
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    pool.push(Candidate {
        desc,
        centroid: mid,
        normal: [d[0] / l, d[1] / l, d[2] / l],
        area: l,
        edge: Some(crate::refs::EdgeGeom { a, b, ..Default::default() }),
    });
}

/// A quarter-circle arc in the XY plane, from `from` to `to` about `center`.
fn arc(pool: &mut Vec<Candidate>, desc: u32, center: [f64; 3], r: f64, from: [f64; 3], to: [f64; 3]) {
    pool.push(Candidate {
        desc,
        centroid: from,
        normal: [0.0, 0.0, 0.0],
        area: r * std::f64::consts::FRAC_PI_2,
        edge: Some(crate::refs::EdgeGeom { a: from, b: to, center, axis: [0.0, 0.0, 1.0], radius: r }),
    });
}

/// Straight edges continuing one another form one chain, and a corner breaks it.
#[test]
fn a_chain_runs_through_smooth_joints_and_stops_at_a_corner() {
    let mut pool = Vec::new();
    seg(&mut pool, 1, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    seg(&mut pool, 2, [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]); // continues the first one straight on
    seg(&mut pool, 3, [20.0, 0.0, 0.0], [20.0, 10.0, 0.0]); // a 90° turn, so not part of the chain
    seg(&mut pool, 9, [50.0, 50.0, 0.0], [60.0, 50.0, 0.0]); // off on its own entirely

    let names = NameTable::default();
    let q = Query::TangentChain { seed: Box::new(Query::Id(1)), tol_deg: 5.0 };
    let mut got = crate::refs::resolve(&q, &pool, &names, &[]);
    got.sort_unstable();
    assert_eq!(got, vec![1, 2], "the chain has to cross the straight junction and stop at the corner: {got:?}");
}

/// An arc tangent to a straight edge is part of the chain too.
///
/// This is why the tangent is computed at the vertex: judged by its direction at the midpoint, an arc looks
/// like a kink even where the junction is perfectly smooth.
#[test]
fn an_arc_tangent_to_a_line_continues_the_chain() {
    let mut pool = Vec::new();
    // a straight edge along X arrives at (10,0), followed by a quarter circle of radius 10 centred at (10,10)
    seg(&mut pool, 1, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    arc(&mut pool, 2, [10.0, 10.0, 0.0], 10.0, [10.0, 0.0, 0.0], [20.0, 10.0, 0.0]);
    // and a straight edge upwards from the end of the arc, smooth again
    seg(&mut pool, 3, [20.0, 10.0, 0.0], [20.0, 20.0, 0.0]);
    // this edge arrives at the same vertex crosswise and is not part of the chain
    seg(&mut pool, 4, [10.0, 0.0, 0.0], [10.0, -10.0, 0.0]);

    let names = NameTable::default();
    let q = Query::TangentChain { seed: Box::new(Query::Id(1)), tol_deg: 5.0 };
    let mut got = crate::refs::resolve(&q, &pool, &names, &[]);
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3], "an arc tangent to a straight edge has to continue the chain: {got:?}");
}

/// The chain survives an edit that adds edges, which is the whole point of describing it.
#[test]
fn a_chain_picks_up_edges_that_appeared_later() {
    let names = NameTable::default();
    let q = Query::TangentChain { seed: Box::new(Query::Id(1)), tol_deg: 5.0 };

    let mut before = Vec::new();
    seg(&mut before, 1, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    seg(&mut before, 2, [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]);
    assert_eq!(crate::refs::resolve(&q, &before, &names, &[]).len(), 2);

    // the edit: the same edge was extended and the rim broke into three pieces
    let mut after = Vec::new();
    seg(&mut after, 1, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    seg(&mut after, 2, [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]);
    seg(&mut after, 7, [20.0, 0.0, 0.0], [30.0, 0.0, 0.0]);
    let mut got = crate::refs::resolve(&q, &after, &names, &[]);
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 7], "the description has to pick up the new edge, where a snapshot would not");
}

/// A tolerance is a tolerance and not exact collinearity: half a degree of kink does not break the chain,
/// while tens of degrees do.
#[test]
fn the_tolerance_decides_what_counts_as_smooth() {
    let mut pool = Vec::new();
    seg(&mut pool, 1, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    seg(&mut pool, 2, [10.0, 0.0, 0.0], [20.0, 0.087, 0.0]); // ≈0.5°
    seg(&mut pool, 3, [20.0, 0.087, 0.0], [25.0, 5.087, 0.0]); // 45°

    let names = NameTable::default();
    let mut got = crate::refs::resolve(&Query::TangentChain { seed: Box::new(Query::Id(1)), tol_deg: 5.0 }, &pool, &names, &[]);
    got.sort_unstable();
    assert_eq!(got, vec![1, 2], "half a degree counts as smooth and forty-five does not: {got:?}");

    let got0 = crate::refs::resolve(&Query::TangentChain { seed: Box::new(Query::Id(1)), tol_deg: 0.1 }, &pool, &names, &[]);
    assert_eq!(got0, vec![1], "at a tolerance of a tenth of a degree even half a degree is a kink");
}
