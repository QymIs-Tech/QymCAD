//! A FACE NAME SURVIVES A MERGE.
//!
//! After a boolean, unification (`ShapeUpgrade_UnifySameDomain`) merges coplanar pieces into one face
//! — otherwise the seam left by cutting leaves doubled faces and tools see two edges where there is
//! one. But the merge inherited the id of whichever piece the traversal met first, and a traversal
//! does not know which is a name and which is a positional number.
//!
//! The cost showed up on a real part: one face push lost 7 structural names out of 13. Walls became
//! unnamed, and the thicken standing on them lost its face on the very next edit — "the tool is
//! broken" out of nowhere. Now names are handed out on a first pass and numbers on a second: if even
//! one piece of the merge was named, the name survives the merge.
//!
//! What is checked is the PROPERTY — "a named face of the source that did not disappear stays named"
//! — rather than particular numbers: the numbers change with any kernel edit, the property must hold
//! always.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A 40x30x20 box with a 2 mm shell open at both ends.
fn through_shell() -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 30.0), Point2::new(0.0, 30.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let b = p.add_extrude_multi(sid, closed, 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);
    let open: Vec<u32> = p.regen_faces[&b].iter().filter(|f| f.normal[2].abs() > 0.9).map(|f| f.id).collect();
    let shell = p.add_shell_mode(b, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    (p, shell)
}

/// NAMED FACES OF THE SOURCE STAY NAMED AFTER A PUSH.
#[test]
fn named_faces_keep_their_names_through_a_push() {
    let (mut p, shell) = through_shell();
    let named_before: Vec<u32> = p.face_pool(shell).iter().map(|c| c.desc).filter(|d| qymcad_core::names::NameTable::is_named(*d)).collect();
    assert!(named_before.len() >= 6, "setup: the shell has named faces ({})", named_before.len());

    // push the outer wall: its neighbours merge with the sides of the prism — that is where the merge happens
    let wall = p.regen_faces[&shell].iter().filter(|f| f.normal[0] > 0.9).max_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x)).expect("the outer wall").clone();
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [wall.centroid.x, wall.centroid.y, wall.centroid.z], normal: wall.normal, id: wall.id };
    let node = p.add_push_face(shell, key, 3.0);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the push must build: {:?}", rep.errors);

    let after: std::collections::HashSet<u32> = p.face_pool(node).iter().map(|c| c.desc).collect();
    let lost: Vec<String> = named_before
        .iter()
        .filter(|d| !after.contains(d))
        .map(|d| {
            let c = p.face_pool(shell).into_iter().find(|c| c.desc == *d);
            format!("{d:#x} (pushed={} centre {:?})", *d == wall.id, c.map(|c| c.centroid.map(|v| (v * 10.0).round() / 10.0)))
        })
        .collect();
    assert!(
        lost.is_empty(),
        "{} names lost in the merge — references to those faces will die on the next edit: {lost:?}",
        lost.len()
    );
}

/// AND THE SAME UNDER A CUT: a boolean with a seam is the main producer of merges.
#[test]
fn named_faces_keep_their_names_through_a_cut() {
    let (mut p, shell) = through_shell();
    let named_before: Vec<u32> = p.face_pool(shell).iter().map(|c| c.desc).filter(|d| qymcad_core::names::NameTable::is_named(*d)).collect();

    // a cut inwards: the same wall, a negative offset
    let wall = p.regen_faces[&shell].iter().filter(|f| f.normal[0] > 0.9).max_by(|a, b| a.centroid.x.total_cmp(&b.centroid.x)).expect("the outer wall").clone();
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [wall.centroid.x, wall.centroid.y, wall.centroid.z], normal: wall.normal, id: wall.id };
    let node = p.add_push_face(shell, key, -0.5);
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the cut must build: {:?}", rep.errors);

    let after: std::collections::HashSet<u32> = p.face_pool(node).iter().map(|c| c.desc).collect();
    let lost: Vec<String> = named_before.iter().filter(|d| !after.contains(d)).map(|d| format!("{d:#x}")).collect();
    assert!(lost.is_empty(), "{} names lost in the cut: {lost:?}", lost.len());
}
