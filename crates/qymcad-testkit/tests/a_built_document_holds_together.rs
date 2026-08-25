//! WHAT A DOCUMENT MUST HOLD AFTER BEING BUILT, checked on a part the test builds itself.
//!
//! These are the replacements for gates that stood on documents living on one machine. Such a gate skips
//! everywhere else and says nothing while it skips; three were found asleep for months, and one of them, once
//! woken, caught a real defect within a minute. What follows asks the same questions of a part assembled here.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A plate with rounded corners, a hole and a shell: enough kinds of reference to be worth asking about.
fn part() -> Project {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Sketch 1",
        vec![Point2::new(0.0, 0.0), Point2::new(60.0, 0.0), Point2::new(60.0, 40.0), Point2::new(0.0, 40.0)],
        true,
    );
    let si = p.sketch_index(sid).expect("the sketch");
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_extrude_multi(sid, closed, 14.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    let upright: Vec<u32> = p.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() > 1.0).map(|e| e.id).collect();
    let filleted = p.add_fillet(body, 3.0, upright);
    qymcad_testkit::regenerate(&mut p);

    let top = p.regen_faces[&filleted]
        .iter()
        .filter(|f| f.normal[2] > 0.9)
        .max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z))
        .expect("a top face")
        .clone();
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id };
    p.add_hole(filleted, key, 8.0, 5.0);
    qymcad_testkit::regenerate(&mut p);
    p
}

fn last_body(p: &Project) -> u64 {
    p.timeline.iter().rev().find_map(|n| n.kind.body()).expect("a body")
}

/// ONE NAME BELONGS TO ONE FACE. Two faces under one name means a reference that lands on either of them by
/// chance, and the part changes shape depending on which.
#[test]
fn one_name_is_never_worn_by_two_faces() {
    let p = part();
    for (body, faces) in p.regen_faces.iter() {
        let mut seen: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for f in faces {
            *seen.entry(f.id).or_default() += 1;
        }
        let doubled: Vec<(u32, usize)> = seen.into_iter().filter(|(_, n)| *n > 1).collect();
        assert!(doubled.is_empty(), "body {body}: one name on several faces at once: {doubled:?}");
    }
}

/// TWO REBUILDS IN A ROW GIVE THE SAME THING. Otherwise a document changes by being opened.
#[test]
fn rebuilding_twice_changes_nothing() {
    let mut p = part();
    let body = last_body(&p);
    let names_a: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let (_, shapes_a) = qymcad_testkit::regenerate(&mut p);
    let volume_a = shapes_a.get(&body).map(|s| s.volume()).unwrap_or(0.0);

    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (report, shapes_b) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the second rebuild broke what the first one built: {:?}", report.errors);
    let names_b: Vec<u32> = p.regen_faces[&body].iter().map(|f| f.id).collect();
    let volume_b = shapes_b.get(&body).map(|s| s.volume()).unwrap_or(0.0);

    assert!(volume_a > 0.0, "the part must have a volume to compare");
    assert!((volume_a - volume_b).abs() < 1e-6, "two rebuilds gave different volumes: {volume_a:.6} against {volume_b:.6}");
    assert_eq!(names_a, names_b, "two rebuilds gave different names to the same faces");
}

/// A BUILT DOCUMENT KEEPS ITS BODIES. The floor catches a catastrophe - half the tree gone - rather than
/// ordinary work.
#[test]
fn a_built_document_keeps_its_bodies_and_geometry() {
    let p = part();
    assert!(!p.bodies.is_empty(), "the document has no bodies at all");
    assert!(p.bodies.iter().any(|b| !b.mesh.tris.is_empty()), "there is no geometry in the document");
    let body = last_body(&p);
    assert!(p.regen_faces.get(&body).is_some_and(|f| f.len() >= 6), "the part lost its faces");
}

/// THE GARBAGE COLLECTOR FINDS NOTHING, AND FINDS IT TWICE.
///
/// `prune_dangling` runs on every rebuild and clears dangling joints, connectors and external references. It
/// was written as insurance for older documents, so the question is not rhetorical: if it still finds
/// something on a part just built, the invariant is being broken right now, and the source needs fixing
/// rather than the sweeping up after it every frame. The second pass must be empty in any case, or the
/// collector does not converge and every frame pays for the same work.
#[test]
fn the_garbage_collector_finds_nothing_on_a_freshly_built_part() {
    let mut p = part();
    let removed = p.prune_dangling();
    assert!(removed.is_empty(), "the collector found dangling links on a part just built: {} of them", removed.len());
    let again = p.prune_dangling();
    assert!(again.is_empty(), "the second pass cleared another {}, so the collector does not converge", again.len());
}

/// AN EDIT IN THE MIDDLE OF THE TIMELINE ADDS NO FAILURES. That is what associativity is for.
#[test]
fn an_edit_in_the_middle_adds_no_failures() {
    let mut p = part();
    let (before, _) = qymcad_testkit::regenerate(&mut p);
    let was: std::collections::HashSet<u64> = before.errors.iter().map(|(n, _)| *n).collect();

    let sid = p.sketches[0].id;
    let si = p.sketch_index(sid).expect("the sketch");
    p.sketches[si].points[0].x += 1.5;
    p.solve_sketch(si);
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (after, _) = qymcad_testkit::regenerate(&mut p);
    let added: Vec<String> = after.errors.iter().filter(|(n, _)| !was.contains(n)).map(|(n, e)| format!("node {n}: {e:?}")).collect();
    assert!(added.is_empty(), "an ordinary edit added failures: {}", added.join("; "));
}
