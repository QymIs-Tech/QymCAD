//! Splitting faces as a feature of the timeline: marking out an area, not breaking the part apart.
//!
//! The difference from cutting a body is fundamental and is checked first: the body stays one.
use qymcad_core::model::Project;

fn part_with_cube() -> (Project, u64) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("a part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    (p, body)
}

fn faces_of(p: &Project, b: u64) -> usize {
    p.regen_faces.get(&b).map(|f| f.len()).unwrap_or(0)
}

fn volume_of(p: &Project, b: u64) -> f64 {
    p.bodies.iter().find(|x| x.id == b).map(|x| x.mesh.volume()).unwrap_or(0.0)
}

/// The body stays one and the face count grows.
#[test]
fn the_body_stays_whole_and_gains_faces() {
    let (mut p, body) = part_with_cube();
    let f0 = faces_of(&p, body);
    let nb = p.add_split_face(body, 0, 0, 10.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the split has to pass: {:?}", rep.errors);

    assert!((volume_of(&p, nb) - 8000.0).abs() < 1.0, "the body is not cut: the volume has to stay 8000, but it became {}", volume_of(&p, nb));
    assert!(faces_of(&p, nb) > f0, "the face count has to grow: it was {f0} and became {}", faces_of(&p, nb));
    // one output: this is not a cut of the body
    let node = p.timeline.iter().find(|n| n.id == nb).expect("the node");
    assert_eq!(node.kind.bodies().len(), 1, "splitting faces has exactly one output");
}

/// The offset is parametric: editing the expression moves the dividing line.
#[test]
fn the_offset_is_parametric() {
    let (mut p, body) = part_with_cube();
    let nb = p.add_split_face(body, 0, 0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let f1 = faces_of(&p, nb);

    p.parameters.push(qymcad_core::model::Param { name: "z".into(), expr: "5".into(), value: 5.0 });
    p.set_feat_dim(nb, "offset", "z".into());
    p.mark_node_dirty(nb);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild from the expression has to pass: {:?}", rep.errors);
    assert_eq!(faces_of(&p, nb), f1, "there are as many splits as before; only the line moved");
    assert!((volume_of(&p, nb) - 8000.0).abs() < 1.0, "the body is still whole");
}

/// A plane clear of the body is an honest error on the node.
#[test]
fn a_plane_that_misses_reports_an_error() {
    let (mut p, body) = part_with_cube();
    let nb = p.add_split_face(body, 0, 0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    p.set_feat_dim(nb, "offset", "100".into());
    p.mark_node_dirty(nb);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(!rep.errors.is_empty(), "a plane that misses has to be noticed");
    assert!(p.regen_errors.contains_key(&nb), "the node has to go red in the tree");
}
