//! The sketch plane, lifting into 3D, the first rebuilds, the frames of datums and the save round trip.

use super::*;

#[test]
fn sketch_defaults_to_world_xy() {
    let mut p = part_project();
    let si = p.new_sketch("s");
    assert_eq!(p.sketches[si].plane, SketchPlane::World(BasePlane::XY), "a new sketch sits on the global XY plane");
    // The XY frame lifts (x,y) into (x,y,0).
    let f = p.sketch_frame(si).unwrap();
    let q = f.lift(Point2::new(3.0, 4.0));
    assert_eq!((q.x, q.y, q.z), (3.0, 4.0, 0.0));
}

#[test]
fn plane_frames_lift_correctly() {
    // XY: (x,y) -> (x,y,0)
    let q = BasePlane::XY.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (2.0, 5.0, 0.0));
    // XZ: (x,y) -> (x,0,y)
    let q = BasePlane::XZ.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (2.0, 0.0, 5.0));
    // YZ: (x,y) -> (0,x,y)
    let q = BasePlane::YZ.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (0.0, 2.0, 5.0));
    // The normals are unit vectors along the expected axes.
    assert_eq!(BasePlane::XY.frame().normal(), [0.0, 0.0, 1.0]);
    assert_eq!(BasePlane::YZ.frame().normal(), [1.0, 0.0, 0.0]);
}

#[test]
fn sketch_plane_and_timeline_survive_serde() {
    let mut p = part_project();
    let si = p.new_sketch("on a face");
    // Place the sketch on a face of a body, which exercises serialisation of the nested `FaceKey`.
    p.sketches[si].plane = SketchPlane::Face(42, FaceKey { index: 3, centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], id: 0 });
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "on a face");

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();

    match back.sketches[si].plane {
        SketchPlane::Face(body, key) => {
            assert_eq!(body, 42);
            assert_eq!(key.index, 3);
            assert_eq!(key.centroid, [1.0, 2.0, 3.0]);
        }
        _ => panic!("the face plane was lost in serialisation"),
    }
    assert_eq!(back.timeline.len(), p.timeline.len(), "the timeline must survive serialisation");
}

#[test]
fn regenerate_builds_extrude_then_clean_noop() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();

    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 1, "the kernel must be called once");
    assert_eq!(rep.built.len(), 1);
    assert_eq!(rep.built[0].0, body);
    assert!(p.mesh_index(body).is_some(), "the body mesh must be created under its id");
    // The mesh marker is the height: the mock encodes the parameter into a vertex.
    assert_eq!(p.bodies[p.mesh_index(body).unwrap()].mesh.verts[0].x, 5.0);
    // The dirty flag was cleared, so a second regenerate builds nothing.
    let rep2 = p.regenerate(&k);
    assert_eq!(k.count(), 1, "a clean regenerate must not call the kernel");
    assert!(rep2.built.is_empty());
}

#[test]
fn editing_sketch_rebuilds_dependent_body() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);

    // Editing the sketch marks it dirty and the downstream body rebuilds.
    p.mark_sketch_dirty(sid);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 2, "after editing the sketch the kernel must be called again");
    assert_eq!(rep.built.len(), 1);
    assert_eq!(rep.built[0].0, body);
}

#[test]
fn a_feature_rebuilds_only_when_its_own_sketch_is_dirty() {
    // This test used to rest on a `FeatureKind::Boolean` variant that turned out to be a leftover: only this
    // test created it, no interface path and no saved document contained it, and the capability is covered
    // twice over by an extrude with an operation and by a body-to-body boolean. What the test checks does not
    // depend on the variant and is preserved: editing an unrelated sketch does not touch the feature.
    let mut p = part_project();
    let base = square(&mut p, "base");
    let other = square(&mut p, "other"); // An independent sketch.
    p.add_sketch_node(base, "base");
    p.add_sketch_node(other, "other");
    let body = p.add_extrude(base, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);

    // Editing an independent sketch does not touch the feature.
    p.mark_sketch_dirty(other);
    p.regenerate(&k);
    assert_eq!(k.count(), 1, "an unrelated sketch must not rebuild the feature");

    // Editing its own sketch rebuilds the feature.
    p.mark_sketch_dirty(base);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 2);
    assert_eq!(rep.built[0].0, body);
}

#[test]
fn regen_error_keeps_node_dirty_for_retry() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let kf = MockKernel { fail: true, ..Default::default() };
    let rep = p.regenerate(&kf);
    assert_eq!(rep.errors.len(), 1, "the kernel error must be recorded");
    assert!(rep.built.is_empty());
    // The node stays dirty, so the next successful pass rebuilds it.
    let ok = MockKernel::default();
    let rep2 = p.regenerate(&ok);
    assert_eq!(rep2.built.len(), 1);
    assert_eq!(rep2.built[0].0, body);
}

#[test]
fn regen_errors_track_failed_then_clear_on_success() {
    // A kernel failure marks the node in `regen_errors`, which the tree reddens with a hint, and a successful
    // rebuild clears the mark. That makes a node which fell through to the pass-through fallback visible as one
    // whose feature was not applied.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let node = p.timeline.iter().find(|n| n.kind.body() == Some(body)).unwrap().id;
    p.regenerate(&MockKernel { fail: true, ..Default::default() });
    assert!(p.regen_errors.contains_key(&node), "the failure must be recorded as a mark on the node");
    p.regenerate(&MockKernel::default());
    assert!(!p.regen_errors.contains_key(&node), "success must clear the mark");
}

#[test]
fn from_origin_normal_builds_orthonormal_frame() {
    use qymcad_core::feature::PlaneFrame;
    // Normal +Z with no rotation, so the axes coincide with world X and Y, plus the origin shift.
    let f = PlaneFrame::from_origin_normal([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0);
    let q = f.lift(Point2::new(2.0, 3.0)); // A 2D lift is `lift3` with z = 0.
    assert_eq!((q.x, q.y, q.z), (12.0, 3.0, 0.0));
    // The frame normal matches the one given.
    let n = f.normal();
    assert!((n[2] - 1.0).abs() < 1e-9);
}

#[test]
fn place_body_lifts_only_when_non_identity() {
    use qymcad_core::feature::{BasePlane, PlaneFrame};
    let mesh = Mesh { verts: vec![Point3::new(1.0, 2.0, 3.0)], tris: vec![], ..Default::default() };
    // XY is the identity, so nothing changes.
    let xy = BasePlane::XY.frame();
    assert!(xy.is_identity());
    let Built { mesh: m0, .. } = xy.place_body(mesh.clone(), vec![]);
    assert_eq!((m0.verts[0].x, m0.verts[0].y, m0.verts[0].z), (1.0, 2.0, 3.0));
    // A datum offset along +X moves the vertex by +10 along X.
    let d = PlaneFrame::from_origin_normal([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0);
    let Built { mesh: m1, .. } = d.place_body(mesh, vec![]);
    assert_eq!((m1.verts[0].x, m1.verts[0].y, m1.verts[0].z), (11.0, 2.0, 3.0));
}

#[test]
fn sketch_frame_resolves_datum_and_face() {
    use qymcad_core::feature::SketchPlane;
    let mut p = part_project();
    let si = p.new_sketch("s");
    // Datum.
    let plane_id = p.add_plane(qymcad_core::model::WorkPlane {
        id: 0,
        name: "d".into(),
        origin: [1.0, 2.0, 3.0],
        normal: [0.0, 0.0, 1.0],
        rot_deg: 0.0,
        tilt: Default::default(),
        def: qymcad_core::model::PlaneDef::Manual,
    });
    p.sketches[si].plane = SketchPlane::Datum(plane_id);
    let f = p.sketch_frame(si).expect("the datum frame");
    // The origin is the projection of the world origin onto the plane z = 3, that is (0,0,3), with XY matching
    // the world.
    assert_eq!(f.origin, [0.0, 0.0, 3.0]);
    // A face at y = 5 (centre (5,5,5), normal +Y): the origin is the projection of the part origin onto the
    // plane, that is (0,5,0), matching the behaviour of a world plane or a datum.
    p.sketches[si].plane = SketchPlane::Face(99, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 1.0, 0.0], id: 0 });
    let g = p.sketch_frame(si).expect("the face frame");
    assert_eq!(g.origin, [0.0, 5.0, 0.0]);
    assert!((g.normal()[1].abs() - 1.0).abs() < 1e-9, "the face normal must be plus or minus Y");
}

#[test]
fn extrude_on_datum_plane_is_placed_in_world() {
    use qymcad_core::feature::SketchPlane;
    let mut p = part_project();
    let sid = square(&mut p, "s");
    // A datum at z = 10 with normal +Z lifts the body to z = 10 while XY is preserved.
    let plane_id = p.add_plane(qymcad_core::model::WorkPlane {
        id: 0,
        name: "d".into(),
        origin: [0.0, 0.0, 10.0],
        normal: [0.0, 0.0, 1.0],
        rot_deg: 0.0,
        tilt: Default::default(),
        def: qymcad_core::model::PlaneDef::Manual,
    });
    let si = p.sketch_index(sid).unwrap();
    p.sketches[si].plane = SketchPlane::Datum(plane_id);
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);

    let k = MockKernel::default();
    p.regenerate(&k);
    // The mock places a vertex at (5,0,0), and the datum frame at z = 10 lifts it to (5,0,10).
    let mi = p.mesh_index(body).unwrap();
    let v = p.bodies[mi].mesh.verts[0];
    assert_eq!((v.x, v.y, v.z), (5.0, 0.0, 10.0), "the body must be moved onto the sketch plane (z = 10, XY as in the world)");
}

#[test]
fn timeline_regenerates_after_save_load_roundtrip() {
    // The parametric chain survives serialisation: the sketch and the nodes are saved, and after loading
    // regenerate rebuilds the body from the sketch, the meshes being only a cache.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 7.0);

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let mut back = qymcad_core::model::from_ron(&ron).unwrap();

    // The timeline and the sketch are in place.
    assert!(back.timeline.iter().any(|n| n.id == body));
    assert!(back.sketch_index(sid).is_some());
    // The body is rebuilt from the sketch after loading.
    let k = MockKernel::default();
    let rep = back.regenerate(&k);
    assert!(rep.built.iter().any(|(b, _)| *b == body), "the body must be rebuilt after loading");
    assert!(back.mesh_index(body).is_some());
}
