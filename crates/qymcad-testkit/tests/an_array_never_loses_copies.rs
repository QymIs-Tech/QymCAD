//! A pattern does not report success having lost its copies.
//!
//! On some combinations the kernel returns not a refusal but an empty shape. That shape silently became the
//! accumulator and the next copy was fused with emptiness: on a hollow body of revolution, four copies over
//! 360°, the node stood green with a volume of 2078 instead of about 8000 — three copies of four had vanished
//! and the timeline said nothing about it. An answer with no warning is worse than a red node, because nobody
//! sees it.
//!
//! What is checked here is not that it worked but that it is honest: either the pattern has more faces than its
//! source, or the node is red. There must be no silent middle. The requirement outlives a fix to the fusing
//! itself — every case would then simply take the first branch.
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};

fn hollow_revolve_array(x0: f64, count: u32, angle: f64) -> (Project, Id, Id) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_line_sketch(
        "Profile",
        vec![Point2::new(x0, 0.0), Point2::new(15.0, 0.0), Point2::new(15.0, 16.0), Point2::new(8.0, 16.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) {
        p.set_active_component(Some(o));
    }
    p.add_sketch_node(sid, "Profile");
    let closed: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let body = p.add_revolve_axis(sid, closed, 1, 360.0, 0, 0);
    qymcad_testkit::regenerate(&mut p);
    let big = p.regen_faces[&body].iter().max_by(|a, b| a.area.total_cmp(&b.area)).cloned().expect("a face of the body of revolution");
    let sh = p.add_shell_mode(body, 1.2, vec![big.id], qymcad_core::feature::ShellSide::Inward);
    qymcad_testkit::regenerate(&mut p);
    let arr = p.add_circular_array_axis(sh, count, angle, 0);
    qymcad_testkit::regenerate(&mut p);
    (p, sh, arr)
}

#[test]
fn an_array_either_multiplies_or_says_it_failed() {
    // A profile with one point shifted gives a conical inner surface, and it is on that surface that fusing
    // the copies comes out empty. A profile with a straight wall passes and serves here as the anchor: the
    // requirement is satisfiable rather than a ban on patterns altogether.
    for (x0, count, angle) in [(8.0, 3, 270.0), (9.5, 3, 270.0), (9.5, 4, 360.0), (9.5, 3, 90.0)] {
        let (p, src, arr) = hollow_revolve_array(x0, count, angle);
        let was = p.regen_faces.get(&src).map(|f| f.len()).unwrap_or(0);
        let now = p.regen_faces.get(&arr).map(|f| f.len()).unwrap_or(0);
        let red = p.regen_errors.contains_key(&arr) || p.regen_errors.values().next().is_some();
        assert!(
            now > was || red,
            "pattern x0={x0}, {count} copies, angle {angle}: the source has {was} faces and the pattern {now}, yet the node is green, so copies were lost silently"
        );
    }
}

#[test]
fn an_array_that_works_is_not_broken_by_the_guard() {
    // The guard against an empty fuse must not touch what used to build: on a straight wall the pattern is as
    // it was.
    let (p, src, arr) = hollow_revolve_array(8.0, 3, 270.0);
    let was = p.regen_faces.get(&src).map(|f| f.len()).unwrap_or(0);
    let now = p.regen_faces.get(&arr).map(|f| f.len()).unwrap_or(0);
    assert!(p.regen_errors.is_empty(), "the pattern on a straight wall went red: {:?}", p.regen_errors);
    assert!(now > was, "the pattern on a straight wall did not multiply the body: it was {was} and became {now}");
}
