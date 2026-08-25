//! AN EDGE NAME IS PRIVATE. Two edges of one body may not carry the same descriptor.
//!
//! Reported behaviour: on a shell open at both ends, the top and bottom rims are 2 mm wide and a
//! single edge cannot be picked on them — the whole face gets selected instead, that is, both the
//! inner and the outer edge. It was not a mis-click: the outer and inner rim edges HAD THE SAME id.
//! There was nothing to select one with — a click on either highlighted both, and a fillet cut both.
//!
//! The cause is in how names are numbered. An edge name is a pair of faces plus an ordinal within
//! that pair. The outer edge arrived from the extrude already named and was skipped, while the inner
//! one — a new edge — was numbered from zero. Number 0 in that pair was already taken.
//!
//! What is checked here is the PROPERTY itself (all names of a body are distinct) rather than a
//! particular numbering scheme: the property survives any replacement of the naming scheme, whereas a
//! check of "the number is computed like this" does not.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A 40x30x20 box with a SHELL OPEN AT BOTH ENDS: the top and bottom faces removed, wall 2 mm.
/// Returns (project, shell body).
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
    let box_body = p.add_extrude_multi(sid, closed, 20.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    qymcad_testkit::regenerate(&mut p);

    // remove BOTH horizontal faces — the result is an open frame with a 2 mm wall
    let open: Vec<u32> = p.regen_faces[&box_body].iter().filter(|f| f.normal[2].abs() > 0.9).map(|f| f.id).collect();
    assert_eq!(open.len(), 2, "the box must have a top and a bottom");
    let shell = p.add_shell_mode(box_body, 2.0, open, qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    (p, shell)
}

/// EVERY EDGE OF A BODY IS TOLD APART BY NAME.
#[test]
fn every_edge_of_a_through_shell_has_its_own_name() {
    let (p, shell) = through_shell();
    let pool = p.edge_pool(shell);
    assert!(pool.len() >= 24, "the frame built: {} edges", pool.len());

    let mut seen: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for c in &pool {
        *seen.entry(c.desc).or_default() += 1;
    }
    let dupes: Vec<(u32, usize)> = seen.into_iter().filter(|(_, n)| *n > 1).collect();
    assert!(
        dupes.is_empty(),
        "{} edges share a name with a neighbour — there is NOTHING to pick such an edge on its own with, their id is common: {:?}",
        dupes.iter().map(|(_, n)| n).sum::<usize>(),
        dupes.iter().map(|(d, n)| format!("{d:#x}x{n}")).collect::<Vec<_>>()
    );
}

/// AND THE CONSEQUENCE: a reference to one edge stays a reference to ONE edge.
///
/// The check is not about the name table but about what a person sees: click the outer rim edge and
/// that is what must be filleted, not the inner one along with it.
#[test]
fn a_reference_to_one_rim_edge_resolves_to_one_edge() {
    let (mut p, shell) = through_shell();
    // the outer edge of the top rim: both ends at the top level, and the longest of those
    let top_z = p.regen_edges[&shell].iter().flat_map(|e| [e.a[2], e.b[2]]).fold(f64::MIN, f64::max);
    let len = |e: &qymcad_core::geom::MeshEdge| {
        let d = [e.b[0] - e.a[0], e.b[1] - e.a[1], e.b[2] - e.a[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    let outer = p.regen_edges[&shell]
        .iter()
        .filter(|e| (e.a[2] - top_z).abs() < 1e-6 && (e.b[2] - top_z).abs() < 1e-6)
        .max_by(|a, b| len(a).total_cmp(&len(b)))
        .expect("the outer rim edge")
        .id;

    let r = qymcad_core::refs::Ref::picks(&[outer]);
    let got = p.resolve_edge_refs(shell, &r, "ref-what-fillet-edge").expect("the reference resolved");
    assert_eq!(got.len(), 1, "a reference to one edge gave {} — a neighbour also matches it: {got:?}", got.len());

    // and a fillet on it builds
    let fil = p.add_fillet_ref(shell, 0.5, r);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    let err = rep.errors.iter().find(|(id, _)| *id == fil);
    assert!(err.is_none(), "filleting one edge must build: {err:?}");
    assert!(shapes.contains_key(&fil), "the filleted body was built");
}
