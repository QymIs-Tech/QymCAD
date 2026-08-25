//! A NAME THAT DISAPPEARS MUST LEAD SOMEWHERE.
//!
//! This is the replacement for a gate that stood on one particular document on one particular machine. Such a
//! gate skips for everybody else - silently - and three of those were found asleep for months. The part here
//! is built by the test itself, so the check runs anywhere OCCT does.
//!
//! WHAT IS ASKED IS NOT "no name may vanish". A sketch edit legitimately merges two coplanar walls into one
//! face, and one face cannot carry two names: demanding otherwise would forbid merging. What matters to a
//! person is different - a name that goes away must FORWARD to the face it yielded to, so that everything
//! standing on it (a fillet, a hole, a sketch) stays where it was. A name that vanishes with no forwarding
//! address is precisely a dangling reference.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

/// A plate with four rounded corners: the fillet records the names of the edges it rounded.
fn plate_with_fillets() -> Project {
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

    // the four upright edges, the way a person picks corners with the mouse
    let upright: Vec<u32> = p.regen_edges[&body].iter().filter(|e| (e.a[2] - e.b[2]).abs() > 1.0).map(|e| e.id).collect();
    assert_eq!(upright.len(), 4, "a rectangular plate has four upright edges, and this one has {}", upright.len());
    p.add_fillet(body, 3.0, upright);
    qymcad_testkit::regenerate(&mut p);
    p
}

#[test]
fn a_sketch_edit_leaves_no_name_without_a_forwarding_address() {
    let mut p = plate_with_fillets();
    let last = p.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the filleted body");
    let faces_before: std::collections::HashSet<u32> = p.regen_faces.get(&last).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    let edges_before: std::collections::HashSet<u32> = p.regen_edges.get(&last).map(|e| e.iter().map(|x| x.id).collect()).unwrap_or_default();
    assert!(faces_before.len() >= 6, "the plate must be built before the edit: {} faces", faces_before.len());

    // THE MOST ORDINARY EDIT A PERSON MAKES: move a corner of the sketch.
    let sid = p.sketches[0].id;
    let si = p.sketch_index(sid).expect("the sketch");
    let pi = (0..p.sketches[si].points.len())
        .max_by(|&a, &b| {
            let (pa, pb) = (&p.sketches[si].points[a], &p.sketches[si].points[b]);
            (pa.x * pa.x + pa.y * pa.y).total_cmp(&(pb.x * pb.x + pb.y * pb.y))
        })
        .expect("a point to move");
    p.sketches[si].points[pi].x += 5.0;
    p.sketches[si].points[pi].y += 3.0;
    p.solve_sketch(si);
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "moving a corner must not break the part: {:?}", report.errors);

    // THE EDIT MUST HAVE MOVED SOMETHING, or the check below asks nothing of anybody. Measured by the volume:
    // a corner pulled out by 5 x 3 mm adds material, and a guard that passes on an unchanged part is a guard
    // that would pass on a broken one too.
    let volume = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
    assert!(volume > 60.0 * 40.0 * 14.0 + 1.0, "the edit did not change the part (volume {volume:.0}), so nothing was actually checked");

    let faces_after: std::collections::HashSet<u32> = p.regen_faces.get(&last).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
    let edges_after: std::collections::HashSet<u32> = p.regen_edges.get(&last).map(|e| e.iter().map(|x| x.id).collect()).unwrap_or_default();

    let orphan = |before: &std::collections::HashSet<u32>, after: &std::collections::HashSet<u32>, what: &str| {
        let lost: Vec<String> = before
            .difference(after)
            .filter(|d| qymcad_core::names::NameTable::is_named(**d))
            .filter(|d| !p.names.absorbed_target(**d).is_some_and(|t| after.contains(&t)))
            .map(|d| p.names.describe(*d))
            .collect();
        assert!(lost.is_empty(), "{what} disappeared with no forwarding address, so everything standing on them dangles: {lost:?}");
    };
    orphan(&faces_before, &faces_after, "names of faces");
    orphan(&edges_before, &edges_after, "names of edges");
}
