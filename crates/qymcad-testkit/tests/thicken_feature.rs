//! THICKEN AS A TIMELINE FEATURE — a parametric plate, not a one-off offset.
//!
//! THE PLATE IS GLUED TO THE PART rather than living beside it as a separate body. It used to stay a
//! body of its own and the part became two: a differently coloured piece on screen, as if a second
//! part had been added. The rule "one part, ONE body" does not allow that.
//!
//! The kernel primitive is untouched: `Shape::thicken_face` still returns a PLAIN plate and is checked
//! for an exact volume by its own tests (`qymcad-kernel/tests/thicken.rs`). The gluing is the feature's
//! business, and it has its own entry point, `thicken_face_join`.
use qymcad_core::model::Project;

fn part_with_cube() -> (Project, u64, u32) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let face = p
        .regen_faces
        .get(&body)
        .and_then(|fs| fs.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap()))
        .map(|f| f.id)
        .expect("the top face");
    (p, body, face)
}

fn volume_of(p: &Project, b: u64) -> f64 {
    p.bodies.iter().find(|x| x.id == b).map(|x| x.mesh.volume()).unwrap_or(0.0)
}

/// The feature builds, the part grows by exactly the plate and stays ONE body.
#[test]
fn the_feature_adds_the_plate_to_the_part() {
    let (mut p, body, face) = part_with_cube();
    let nb = p.add_thicken(body, face, 3.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the thicken must build: {:?}", rep.errors);

    // the box 8000 plus a plate 20x20x3 = 1200
    assert!((volume_of(&p, nb) - 9200.0).abs() < 1.0, "the part must become 9200 (box 8000 plus plate 1200), and it came out {}", volume_of(&p, nb));
    // THE SOURCE IS CONSUMED: otherwise there are two bodies of different colours on screen instead of one part
    assert!(p.consumed_bodies().contains(&body), "the thicken must consume its source — there is one part");
}

/// The thickness is PARAMETRIC: edit the expression and the plate rebuilds.
#[test]
fn the_thickness_is_parametric() {
    let (mut p, body, face) = part_with_cube();
    let nb = p.add_thicken(body, face, 3.0);
    let _ = qymcad_testkit::regenerate(&mut p);

    p.parameters.push(qymcad_core::model::Param { name: "t".into(), expr: "5".into(), value: 5.0 });
    p.set_feat_dim(nb, "thickness", "t".into());
    p.mark_node_dirty(nb);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild driven by the expression must pass: {:?}", rep.errors);
    assert!((volume_of(&p, nb) - 10000.0).abs() < 1.0, "with \"t\"=5 the part must become 10000 (8000 plus 2000), and it came out {}", volume_of(&p, nb));
}

/// THE PLATE FOLLOWS THE PART: change the size of the source and the skin is recomputed.
#[test]
fn the_plate_follows_the_source_face() {
    let (mut p, body, face) = part_with_cube();
    let nb = p.add_thicken(body, face, 2.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!((volume_of(&p, nb) - 8800.0).abs() < 1.0, "setup: box 8000 plus a plate 20x20x2 = 8800, and it came out {}", volume_of(&p, nb));

    if let Some(n) = p.timeline.iter_mut().find(|n| n.kind.bodies().contains(&body)) {
        if let qymcad_core::feature::FeatureKind::Box3 { dx, .. } = &mut n.kind {
            *dx = 40.0;
        }
        n.dirty = true;
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild must pass: {:?}", rep.errors);
    // the box became 40x20x20 = 16000, the plate 40x20x2 = 1600
    assert!((volume_of(&p, nb) - 17600.0).abs() < 1.0, "the face became 40x20 — the part must become 17600, and it came out {}", volume_of(&p, nb));
}

/// Zero thickness is an honest node error.
#[test]
fn zero_thickness_reports_an_error() {
    let (mut p, body, face) = part_with_cube();
    let nb = p.add_thicken(body, face, 2.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    p.set_feat_dim(nb, "thickness", "0".into());
    p.mark_node_dirty(nb);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(!rep.errors.is_empty(), "zero thickness must be noticed");
    assert!(p.regen_errors.contains_key(&nb), "the node must be flagged with an error in the tree");
}
