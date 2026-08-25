//! A VARIABLE RADIUS IS SET AT VERTICES, NOT AT THE ENDS OF AN EDGE.
//!
//! "Radius at the start of the edge -> radius at its end" describes ONE edge, which has a direction.
//! A SET of edges has no direction, and such a parameter is fundamentally incompatible with a
//! descriptive reference: "all edges of the top face" has neither a start nor an end. Previously that
//! case silently fell back to an explicit list — associativity was lost without a word.
//!
//! A radius at a VERTEX removes all of that at once: neighbouring edges SHARE the vertex, so the
//! radius there is shared too — the chain meets without a step by the way it is stated, not by any
//! check.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_core::refs::{Query, Ref};

/// A 60x40x12 plate in a part. Returns (project, body).
fn plate() -> (Project, u64) {
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
    let body = p.add_extrude_multi(sid, closed, 12.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    (p, body)
}

/// The top face of a body.
fn top_face(p: &Project, body: u64) -> u32 {
    p.regen_faces[&body].iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)).expect("top face").id
}

/// A VERTEX HAS A NAME, AND IT IS DERIVED FROM ITS EDGES.
///
/// A vertex has no recipe of its own — it is where edges meet. So its name must be derived rather
/// than ordinal: an ordinal one would slide on any edit of a neighbour.
#[test]
fn a_vertex_is_named_by_the_edges_that_meet_there() {
    let (mut p, body) = plate();
    let pool = p.vertex_pool(body);
    assert_eq!(pool.len(), 8, "the plate has eight corners, and {} were found", pool.len());
    for c in &pool {
        assert!(qymcad_core::names::NameTable::is_vertex(c.desc), "a vertex must carry a VERTEX name, not {:#x}", c.desc);
        let name = p.names.vertex(c.desc).expect("the name is in the document table");
        let live: Vec<u32> = name.edges.into_iter().filter(|d| *d != 0).collect();
        assert!(live.len() >= 2, "a vertex is identified by its edges, and it has {}", live.len());
    }
    // the names are DIFFERENT: two corners under one name would mean a radius at one travels to the other
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for c in &pool {
        assert!(seen.insert(c.desc), "two corners share the name {:#x} — there would be nothing to set a radius on one with", c.desc);
    }
}

/// THE VERTEX TABLE WORKS TOGETHER WITH A DESCRIPTION — the point of the whole change.
///
/// "All edges of the top face" plus different radii at the corners: the previous form of the
/// parameter silently fell back to a list of picks here, and the first sketch edit left the fillet on
/// yesterday's numbers.
#[test]
fn a_described_set_can_carry_a_variable_radius() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let edges = Ref::many(Query::Adjacent(Box::new(Query::Id(top))));

    // radius 1 by default, and 3 at two corners
    let mut corners: Vec<(u32, f64)> = Vec::new();
    for c in p.vertex_pool(body) {
        if c.centroid[2] > 11.0 && c.centroid[0] < 1.0 {
            corners.push((c.desc, 3.0)); // the two top corners on the x=0 side
        }
    }
    assert_eq!(corners.len(), 2, "there should be two top corners at x=0, and {} were found", corners.len());

    let fil = p.add_fillet_at_vertices(body, 1.0, edges, corners);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a variable-radius fillet on a DESCRIBED set must build: {:?}", rep.errors);

    // what is stored in the timeline is a DESCRIPTION, not a list
    let stored = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Fillet { ref edges, ref at_vertices, .. } => Some((edges.clone(), at_vertices.clone())),
            _ => None,
        })
        .expect("the fillet is in the timeline");
    assert!(matches!(stored.0.query, Query::Adjacent(_)), "the edge set must stay a description: {:?}", stored.0.query);
    assert_eq!(stored.1.len(), 2, "and the vertex table must be kept whole");

    // THE VOLUME SAYS THE RADIUS REALLY IS VARIABLE: more is cut away than a constant R1 would take
    let v_var = p.bodies.iter().find(|b| b.id == fil).expect("the body").mesh.volume();
    let (mut p2, body2) = plate();
    let top2 = top_face(&p2, body2);
    let f2 = p2.add_fillet_ref(body2, 1.0, Ref::many(Query::Adjacent(Box::new(Query::Id(top2)))));
    qymcad_testkit::regenerate(&mut p2);
    let v_const = p2.bodies.iter().find(|b| b.id == f2).expect("the body").mesh.volume();
    assert!(v_var < v_const - 1.0, "a radius of 3 at two corners must remove MORE material than a constant R1: {v_var:.2} against {v_const:.2}");
}

/// NEIGHBOURING EDGES TAKE ONE RADIUS AT THE VERTEX THEY SHARE — no step at the junction, by
/// construction.
///
/// This is why the radius moved from the ends of an edge to the vertex. It is not judged by eye: with
/// the previous form of the parameter TWO neighbouring edges got DIFFERENT values at their shared
/// corner (one has its end there, the other its start), and a seam was unavoidable.
#[test]
fn neighbours_share_one_radius_at_the_vertex_they_share() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let corner = p
        .vertex_pool(body)
        .into_iter()
        .find(|c| c.centroid[2] > 11.0 && c.centroid[0] < 1.0 && c.centroid[1] < 1.0)
        .expect("the top corner at (0,0)");
    // the edges that meet at that corner
    let name = p.names.vertex(corner.desc).expect("the vertex name");
    let meeting: Vec<u32> = name.edges.into_iter().filter(|d| *d != 0).collect();
    assert!(meeting.len() >= 2, "at least two edges meet at a corner");

    let edges = Ref::many(Query::Adjacent(Box::new(Query::Id(top))));
    let fil = p.add_fillet_at_vertices(body, 1.0, edges, vec![(corner.desc, 2.0)]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the fillet must build: {:?}", rep.errors);
    assert!(p.bodies.iter().any(|b| b.id == fil && !b.mesh.tris.is_empty()), "the body must be built");
}

/// A VERTEX REFERENCE SURVIVES AN EDIT THAT MOVED THE GEOMETRY.
///
/// That is why the name is derived from the edges: stretch the sketch and the corner moves elsewhere,
/// but it is THE SAME corner, and the radius must stay on it rather than get lost or jump to a
/// neighbour.
#[test]
fn the_vertex_reference_survives_a_sketch_edit() {
    let (mut p, body) = plate();
    let top = top_face(&p, body);
    let corner = p.vertex_pool(body).into_iter().find(|c| c.centroid[2] > 11.0 && c.centroid[0] > 59.0 && c.centroid[1] > 39.0).expect("the top corner at (60,40)");
    let edges = Ref::many(Query::Adjacent(Box::new(Query::Id(top))));
    let fil = p.add_fillet_at_vertices(body, 1.0, edges, vec![(corner.desc, 2.0)]);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: {:?}", rep.errors);

    // THE EDIT: the plate got longer — the corner moved but stayed the same corner
    let sid = p.sketches[0].id;
    for pt in &mut p.sketches[0].points {
        if (pt.x - 60.0).abs() < 1e-9 {
            pt.x = 80.0;
        }
    }
    p.solve_sketch(0);
    p.regen_sketch(0);
    p.mark_sketch_dirty(sid);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "after the edit the fillet must rebuild: {:?}", rep.errors);

    let stored = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Fillet { src, ref at_vertices, .. } => Some((src, at_vertices.clone())),
            _ => None,
        })
        .expect("the fillet is in the timeline");
    let found = p.resolve_vertex_refs(stored.0, &stored.1[0].0, "ref-what-fillet-vertex").expect("the vertex reference must resolve after the edit too");
    let pt = p.vertex_point(stored.0, found[0]).expect("the vertex has a position");
    assert!((pt[0] - 80.0).abs() < 1e-6 && (pt[1] - 40.0).abs() < 1e-6, "the radius must stay on THE SAME corner, which moved with the plate, and it is at {pt:?}");
    assert!(p.bodies.iter().any(|b| b.id == fil && !b.mesh.tris.is_empty()), "the body is there");
}
