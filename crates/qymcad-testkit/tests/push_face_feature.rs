//! Pushing a face is a feature of the timeline, parametric rather than a one-off edit.
//!
//! Direct modelling does not abolish history: pushing a face adds a step to the timeline that can afterwards be
//! edited by a number or a formula. Otherwise it would be a one-off deformation of the mesh, and the whole
//! associativity would fall apart.
use qymcad_core::feature::FaceKey;
use qymcad_core::model::Project;

fn part_with_cube() -> (Project, u64, FaceKey) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("a part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the top face of the cube
    let f = p
        .regen_faces
        .get(&body)
        .and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()))
        .cloned()
        .expect("there is a top face");
    let key = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    (p, body, key)
}

/// The feature builds and changes the volume by exactly what was pushed.
#[test]
fn the_feature_builds_and_changes_volume_by_exactly_the_distance() {
    let (mut p, body, face) = part_with_cube();
    let v0: f64 = p.bodies.iter().map(|b| b.mesh.volume()).sum();

    let nb = p.add_push_face(body, face, 5.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the feature has to build: {:?}", rep.errors);
    let live: f64 = p.bodies.iter().filter(|b| Some(b.id) == Some(nb)).map(|b| b.mesh.volume()).sum();
    assert!((live - 10000.0).abs() < 1.0, "a cube of 8000 plus the pushed 20·20·5 makes 10000, but it came out {live}, having been {v0}");
}

/// The offset is parametric: editing the expression rebuilds the body.
#[test]
fn the_distance_is_parametric() {
    let (mut p, body, face) = part_with_cube();
    let nb = p.add_push_face(body, face, 5.0);
    let _ = qymcad_testkit::regenerate(&mut p);

    p.parameters.push(qymcad_core::model::Param { name: "h".into(), expr: "8".into(), value: 8.0 });
    p.set_feat_dim(nb, "dist", "h".into());
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == nb) {
        n.dirty = true;
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild from the expression has to pass: {:?}", rep.errors);
    let live: f64 = p.bodies.iter().filter(|b| Some(b.id) == Some(nb)).map(|b| b.mesh.volume()).sum();
    assert!((live - 11200.0).abs() < 1.0, "at h = 8 the volume has to become 8000 + 3200 = 11200, but it came out {live}");
}
