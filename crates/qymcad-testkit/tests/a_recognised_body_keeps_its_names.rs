//! A BODY RECOGNISED FROM A MESH KEEPS ITS NAMES when it is recognised again. A mesh has no recipe - no sketch, no
//! node above its faces - so a face of it is named by where it lies, the one rule for a body with nothing else known
//! about it. Recognised again with another tolerance, a face that came out the same stands where it stood, holds its
//! name, and a rounding laid on an edge of it rounds the same edge.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::Project;

/// A cube of side 10 as a bare triangle mesh, brought in as a part of its own.
fn cube(p: &mut Project) -> u64 {
    let v = |x: f64, y: f64, z: f64| Point3::new(x * 10.0, y * 10.0, z * 10.0);
    let mesh = Mesh {
        verts: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(1.0, 1.0, 1.0), v(0.0, 1.0, 1.0)],
        tris: vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]],
        ..Default::default()
    };
    let source = p.add_source("cube.stl", vec![1]);
    let m = p.add_mesh(mesh);
    p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "cube".into(), body: Some(m), mesh: true, ..Default::default() }], source, "cube").expect("the mesh came in");
    m
}

#[test]
fn a_rounding_holds_its_edge_when_the_mesh_is_recognised_again() {
    let mut p = Project::default();
    p.new_document();
    let m = cube(&mut p);
    let body = p.add_mesh_recognised(m, 1.0, 30.0, 0.0);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the mesh was not recognised: {:?}", report.errors);
    let edge = p.regen_edges[&body].iter().find(|e| (e.mid[0] - 5.0).abs() < 1e-3 && e.mid[1].abs() < 1e-3 && e.mid[2].abs() < 1e-3).map(|e| e.id).expect("the edge along x at the bottom front");
    let faces_before: Vec<u32> = {
        let mut f: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
        f.sort_unstable();
        f
    };
    // laid in the part the body stands in, as a person stepped into it lays it
    let part = p.timeline.iter().find(|n| n.kind.owns_body(body)).and_then(|n| n.parent);
    p.set_active_component(part);
    let fillet = p.add_fillet(body, 1.0, vec![edge]);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the rounding did not build: {:?}", report.errors);
    // recognised again, with another tolerance and another sharpness
    for n in p.timeline.iter_mut() {
        if let qymcad_core::feature::FeatureKind::MeshRecognised { tol, sharp, .. } = &mut n.kind {
            *tol = 2.0;
            *sharp = 40.0;
            n.dirty = true;
        }
    }
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "recognised again, something refused: {:?}", report.errors);
    let faces_after: Vec<u32> = {
        let mut f: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
        f.sort_unstable();
        f
    };
    assert_eq!(faces_after, faces_before, "the faces of the cube changed their names when it was recognised again");
    // the rounding stands on the edge it was laid on: its face runs along x at the bottom front
    let round = p.regen_faces[&fillet].iter().find(|f| f.centroid.y < 1.0 && f.centroid.z < 1.0 && (f.centroid.x - 5.0).abs() < 1.0);
    assert!(round.is_some(), "the rounding left the edge it was laid on");
}
