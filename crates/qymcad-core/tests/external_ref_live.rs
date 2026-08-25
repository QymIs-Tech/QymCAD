//! A live external reference, top-down, instead of a one-off snapshot.
//!
//! A sketch of a consuming part placed on a face of another part's body has to follow its source: both when the
//! neighbour moves within the assembly and when its geometry changes. Plus an honest break of the link: the
//! geometry freezes as a snapshot exactly where it stood, and the part does not fall into an isolation error.
use qymcad_core::feature::{FaceKey, SketchPlane};
use qymcad_core::geom::{MeshFace, Point3};
use qymcad_core::model::{Id, Project};

/// A source body inside the part `part` with a single face, persistent id 1, in its local frame.
fn source_body_with_face(p: &mut Project, part: Id, z: f64) -> (Id, FaceKey) {
    p.set_active_component(Some(part));
    let body = p.add_box(10.0, 10.0, z);
    let face = MeshFace {
        triangles: Vec::new(),
        normal: [0.0, 0.0, 1.0],
        centroid: Point3::new(5.0, 5.0, z),
        area: 100.0,
        id: 1,
    };
    p.regen_faces.insert(body, vec![face]);
    (body, FaceKey { index: 0, centroid: [5.0, 5.0, z], normal: [0.0, 0.0, 1.0], id: 1 })
}

/// The scene: an assembly with two parts, where B holds a sketch on the top face of the body of A through a
/// live external reference.
fn scene() -> (Project, Id, Id, Id, Id, usize) {
    let mut p = Project::default();
    let asm = p.new_document();
    let a = p.add_part("A");
    let b = p.add_part("B");
    let (body, key) = source_body_with_face(&mut p, a, 20.0);
    p.set_active_component(Some(b));
    let si = p.new_sketch("Sketch on the neighbour");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Face(body, key);
    p.add_sketch_node(sid, "Sketch on the neighbour".to_string());
    p.add_external_face_ref(b, body, key); // authorise the cross-reference, as `resolve_placement_plane` does
    (p, asm, a, b, body, si)
}

#[test]
fn sketch_frame_follows_source_component_placement() {
    let (mut p, _asm, a, _b, _body, si) = scene();
    let f0 = p.sketch_frame(si).expect("frame of the sketch on the face of the neighbour");
    // the neighbour moved 30 along X, and the live reference has to carry the sketch with it
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 30.0;
    p.set_component_transform(a, mat);
    let f1 = p.sketch_frame(si).expect("frame after the source moved");
    assert!((f1.origin[0] - f0.origin[0] - 30.0).abs() < 1e-9, "the sketch did not follow the neighbour: {:?} -> {:?}", f0.origin, f1.origin);
}

#[test]
fn sketch_frame_follows_source_geometry_change() {
    let (mut p, _asm, _a, _b, body, si) = scene();
    let z0 = p.sketch_frame(si).expect("frame").origin[2];
    // the neighbour grew and the face moved up, as after editing an extrusion height
    if let Some(faces) = p.regen_faces.get_mut(&body) {
        faces[0].centroid.z = 35.0;
    }
    let z1 = p.sketch_frame(si).expect("frame after the neighbour was edited").origin[2];
    assert!((z1 - 35.0).abs() < 1e-9 && (z0 - 20.0).abs() < 1e-9, "the sketch did not follow the face: {z0} -> {z1}, expected 20 -> 35");
}

#[test]
fn breaking_ref_freezes_geometry_and_keeps_part_valid() {
    let (mut p, _asm, a, b, body, si) = scene();
    let before = p.sketch_frame(si).expect("frame before the break");
    let rid = p.external_ref_for(b, body).expect("the external reference is registered").id;

    let frozen = p.break_external_ref(rid);
    assert_eq!(frozen, 1, "exactly one sketch was frozen");
    assert!(p.external_ref_for(b, body).is_none(), "the reference is removed");
    assert!(matches!(p.sketches[si].plane, SketchPlane::Datum(_)), "the plane of the sketch became a datum snapshot");

    // the geometry did not stir
    let after = p.sketch_frame(si).expect("frame after the break");
    for k in 0..3 {
        assert!((after.origin[k] - before.origin[k]).abs() < 1e-9, "the break moved the sketch: {:?} -> {:?}", before.origin, after.origin);
    }
    // and it no longer follows the neighbour
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 30.0;
    p.set_component_transform(a, mat);
    let moved = p.sketch_frame(si).expect("frame after the neighbour moved");
    assert!((moved.origin[0] - before.origin[0]).abs() < 1e-9, "after the break the sketch is still driven by the neighbour");
}

/// The hard case of a break: the neighbour is rotated. The axes of a live frame then come from the local frame
/// of the source through that rotation, while a fresh datum snapshot would choose its own by the dominant-axis
/// rule and take its 2D origin from the consumer, so the break would rotate and shift the whole geometry of the
/// sketch. The snapshot has to land exactly on the live frame: the origin and both axes.
#[test]
fn breaking_ref_preserves_frame_even_for_rotated_source() {
    let (mut p, _asm, a, b, body, si) = scene();
    let (s, c) = 30f64.to_radians().sin_cos();
    p.set_component_transform(a, [c, -s, 0.0, 12.0, s, c, 0.0, -4.0, 0.0, 0.0, 1.0, 3.0]);
    let live = p.sketch_frame(si).expect("live frame with the neighbour rotated");
    let rid = p.external_ref_for(b, body).expect("the reference").id;
    assert_eq!(p.break_external_ref(rid), 1);
    let frozen = p.sketch_frame(si).expect("frame of the snapshot");
    let mut bad = Vec::new();
    for k in 0..3 {
        if (frozen.origin[k] - live.origin[k]).abs() > 1e-9 {
            bad.push(format!("origin[{k}]: {} != {}", frozen.origin[k], live.origin[k]));
        }
        if (frozen.x[k] - live.x[k]).abs() > 1e-9 {
            bad.push(format!("axis X[{k}]: {} != {}", frozen.x[k], live.x[k]));
        }
        if (frozen.y[k] - live.y[k]).abs() > 1e-9 {
            bad.push(format!("axis Y[{k}]: {} != {}", frozen.y[k], live.y[k]));
        }
    }
    assert!(bad.is_empty(), "breaking the link moved or rotated the sketch:\n{}", bad.join("\n"));
}

/// Without an external reference a cross-component sketch is forbidden by isolation: there is no frame at all,
/// so the rebuild stops the feature honestly instead of building it somewhere arbitrary. That is the contract a
/// live reference rests on.
#[test]
fn unauthorized_cross_component_sketch_has_no_frame() {
    let (mut p, _asm, _a, b, body, si) = scene();
    let rid = p.external_ref_for(b, body).expect("the reference").id;
    p.remove_external_ref(rid); // a raw removal rather than `break_external_ref`: the authorisation is gone
    assert!(p.sketch_frame(si).is_none(), "without authorisation a cross-reference must not yield a frame");
}

/// When the source is deleted the consumer stays valid instead of hanging on a dead face.
///
/// Measured before the fix: after a cascading deletion of the source body — and likewise after deleting its
/// sketch — the external reference stayed in the document, pointing at a body no timeline node produces. The
/// sketch of the consumer meanwhile went on sitting on a `SketchPlane::Face` of that dead body.
///
/// Simply dropping the record would have solved only half of it: the sketch would still be on a dead face. The
/// core has `break_external_ref` for exactly this case — it freezes the sketch as a datum snapshot precisely
/// where it stood — and deleting a body now calls it.
#[test]
fn deleting_the_source_body_freezes_the_consumer_sketch() {
    let (mut p, _asm, _a, b, body, si) = scene();
    let before = p.sketch_frame(si).expect("frame before the source is deleted");
    assert!(p.external_ref_for(b, body).is_some(), "the external reference is registered");
    assert!(matches!(p.sketches[si].plane, SketchPlane::Face(..)), "the sketch sits on a face of the neighbour");

    p.delete_body_cascade(body);

    assert!(p.external_ref_for(b, body).is_none(), "the dangling external reference was removed along with the body");
    assert!(
        matches!(p.sketches[si].plane, SketchPlane::Datum(_)),
        "the sketch of the consumer has to freeze as a datum snapshot rather than hang on a dead face"
    );
    let after = p.sketch_frame(si).expect("frame after the source is deleted");
    for k in 0..3 {
        assert!(
            (after.origin[k] - before.origin[k]).abs() < 1e-9,
            "freezing moved the sketch: {:?} -> {:?}",
            before.origin,
            after.origin
        );
    }
}
