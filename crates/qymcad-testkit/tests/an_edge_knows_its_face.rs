//! AN EDGE HAS A SECOND AXIS — TAKEN FROM ITS NEIGHBOURING FACE.
//!
//! An edge has one axis of its own: along itself. The roll of a connector frame is undefined without
//! a second axis, and it used to be derived from the WORLD Z axis — that is, from however the part
//! happened to lie. Two parts being mated ended up with different secondary axes and the joint set
//! the part at an arbitrary roll; in the solver that was patched over with a caveat that rigid and
//! slider joints on an edge do hold roll after all.
//!
//! The second axis must come from the NEIGHBOURING FACE. What is checked here is that the kernel
//! really returns it, and returns something meaningful: the direction must be unit length,
//! perpendicular to the edge, and equal to the normal of one of the faces meeting at that edge.
use qymcad_core::feature::AnchorRef;
use qymcad_core::model::Project;

/// A 10x10x10 block part at the origin. Returns (document, component, body).
fn a_box() -> (Project, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let s = p.new_sketch("a");
    let sid = p.sketches[s].id;
    p.add_sketch_node(sid, "a");
    p.add_rect_entity(s, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(s);
    let body = p.add_extrude(sid, 10.0);
    p.finish_base_body(body, 1);
    let (r, _) = qymcad_testkit::regenerate(&mut p);
    assert!(r.errors.is_empty(), "the block did not build: {:?}", r.errors);
    (p, a, body)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// EVERY EDGE OF A BLOCK CARRIES A REFERENCE DIRECTION, AND IT IS A REAL FACE NORMAL.
#[test]
fn every_edge_of_a_box_carries_a_real_face_normal() {
    let (p, _a, body) = a_box();
    let edges = p.regen_edges.get(&body).cloned().unwrap_or_default();
    assert!(edges.len() >= 12, "a block must have 12 edges, and it has {}", edges.len());

    for e in &edges {
        let r = e.ref_dir;
        let len = dot(r, r).sqrt();
        assert!((len - 1.0).abs() < 1e-6, "edge {}: the reference direction {r:?} has length {len:.3e} — neither unit nor empty", e.id);
        // perpendicular to the edge: the normal of a face the edge lies in is perpendicular to it
        let along = dot(r, e.dir).abs();
        assert!(along < 1e-6, "edge {}: the reference direction {r:?} leans along the edge {:?} (cosine {along:.3e})", e.id, e.dir);
        // on a block the face normals are the world axes; the reference direction must be one of them
        let axis = r.iter().filter(|c| (c.abs() - 1.0).abs() < 1e-6).count();
        assert_eq!(axis, 1, "edge {}: the reference direction {r:?} matched none of the block faces", e.id);
    }
}

/// A CONNECTOR FRAME ON AN EDGE TURNS WITH ITS FACE, NOT WITH THE WORLD.
///
/// A check on itself: if the secondary axis were still taken from world Z, it would be the same for
/// every edge pointing the same way. Here two edges running ALONG THE SAME AXIS but belonging to
/// different faces are taken — and their frames must come out DIFFERENT.
#[test]
fn two_parallel_edges_on_different_faces_give_different_frames() {
    let (mut p, a, body) = a_box();
    let edges = p.regen_edges.get(&body).cloned().unwrap_or_default();

    // All edges along Z are the four vertical edges of the block, each on its own pair of faces.
    let vertical: Vec<_> = edges.iter().filter(|e| e.dir[2].abs() > 0.999).collect();
    assert_eq!(vertical.len(), 4, "a block has four vertical edges, and {} were found", vertical.len());

    let mut seen: Vec<[f64; 3]> = Vec::new();
    for e in &vertical {
        let c = p.add_connector(a, AnchorRef::EdgeMid(body, e.id));
        let conn = p.connector(c).expect("the connector").clone();
        let fr = p.connector_frame(&conn).expect("the connector frame on the edge");
        seen.push(fr.x);
    }
    let same = seen.iter().filter(|x| dot(**x, seen[0]) > 0.999).count();
    assert!(
        same < 4,
        "all four vertical edges share one secondary axis {:?} — it is derived from the world rather than from the faces of the part",
        seen[0]
    );
}
