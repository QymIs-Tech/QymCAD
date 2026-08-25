//! A FACE ALWAYS HAS A PRINCIPAL DIRECTION.
//!
//! The principal direction of a face is its "long side". The travel axis of a slider on a planar face
//! and the secondary axis of a connector are taken from it: without it a joint has no geometric
//! direction and falls back to the world axes. That is what showed up as a slider whose direction was
//! wrong no matter which faces were picked.
//!
//! WHY. The direction is found by power iteration over the covariance of the face points, and the
//! starting vector was the WORLD X AXIS. On a face whose normal points along X, all the points lie in
//! the YZ plane: the very first iteration step gives zero and the function honestly answered "I do
//! not know". So every face turned towards world X had no direction AT ALL — and in any assembly that
//! is a third of them.
//!
//! Found by the gate on the scenario document: a slider on a pair of 600 mm^2 faces with two
//! triangles each, and `principal=None` on both.
use qymcad_core::feature::FaceKey;
use qymcad_core::model::{Id, Project};

/// A 40x30x20 plate at the origin: it has faces with all six normals along the world axes.
fn plate(p: &mut Project) -> Id {
    let c = p.add_part("A");
    p.set_active_component(Some(c));
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 20.0);
    p.finish_base_body(body, 1);
    body
}

/// EVERY PLANAR FACE HAS A LONG SIDE, AND IT LIES IN THE PLANE OF THAT FACE.
#[test]
fn every_flat_face_has_a_long_side_whichever_way_it_faces() {
    let mut p = Project::default();
    p.new_document();
    let body = plate(&mut p);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "the plate did not build: {:?}", r.errors);

    let faces = p.regen_faces.get(&body).cloned().unwrap_or_default();
    assert!(faces.len() >= 6, "the plate has six faces, and {} were found", faces.len());

    let mut without = Vec::new();
    for f in &faces {
        let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
        match p.face_principal_dir(body, &k) {
            None => without.push(f.normal),
            Some(d) => {
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                assert!((len - 1.0).abs() < 1e-9, "face {:?}: the principal direction {d:?} is not unit length", f.normal);
                let across = (d[0] * f.normal[0] + d[1] * f.normal[1] + d[2] * f.normal[2]).abs();
                assert!(across < 1e-9, "face {:?}: the principal direction {d:?} sticks OUT of the face (cosine {across:.3e})", f.normal);
            }
        }
    }
    assert!(without.is_empty(), "faces with normals {without:?} have no principal direction at all — a joint on them will travel along the world axes");
}

/// THE PRINCIPAL DIRECTION REALLY IS THE LONG SIDE, not just any axis in the plane.
#[test]
fn the_long_side_is_the_long_one() {
    let mut p = Project::default();
    p.new_document();
    let body = plate(&mut p);
    let r = qymcad_testkit::open_like_the_app(&mut p);
    assert!(r.errors.is_empty(), "the plate did not build: {:?}", r.errors);

    // The 40x20 side face (normal along -Y): the long side is X.
    let f = p
        .regen_faces
        .get(&body)
        .and_then(|fs| fs.iter().find(|f| f.normal[1] < -0.99).cloned())
        .expect("the 40x20 side face");
    let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let d = p.face_principal_dir(body, &k).expect("the side face has a long side");
    assert!(d[0].abs() > 0.999, "the long side of a 40x20 face runs along X, and {d:?} came out");
}
