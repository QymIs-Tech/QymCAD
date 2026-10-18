//! A BLEND LEAVES THE BODY IT WAS HANDED AS IT WAS. The chamfer and the fillet of OCCT raise the tolerances of the edges
//! and vertices of the body they are given: measured on the case of the examples, a chamfer of 3 took the sum of the
//! tolerances of its body from 0.0720624 to 0.0744600, a chamfer of 2 from 0.0408696 to 0.0432672. Those edges and
//! vertices are shared with the bodies before it and beside it in the cache, and a neighbouring branch of the rebuild
//! reads and writes them on its own thread: the chamfer 11963 of the case came out invalid and was stepped back in 3 of
//! 40 rebuilds with the filter rebuilt before it in one process, and in none of 20 on one thread.
//!
//! The case is rebuilt whole, its blends are marked and rebuilt on the bodies of the cache, and every body a blend was
//! handed and nothing rebuilt is to be, to the byte of its BRep, the body it was before. Live OCCT, headless.
use qymcad_core::feature::FeatureKind;
use qymcad_core::model::Id;
use std::collections::HashSet;

/// THE CASE OF THE EXAMPLES, opened as the window opens it.
const CASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/QymBoxPc-Case.qcad");

/// The body a node blends, where it is a fillet or a chamfer.
fn blended(kind: &FeatureKind) -> Option<Id> {
    match kind {
        FeatureKind::Fillet { src, .. } | FeatureKind::Chamfer { src, .. } => Some(*src),
        _ => None,
    }
}

#[test]
fn the_bodies_the_blends_of_the_case_are_handed_stay_as_they_were() {
    let mut p = qymcad_io::load_project(CASE).expect("the case opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (report, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "setup: the case is built whole: {:?}", report.errors);
    // the bodies built by a blend are rebuilt with it; the ones handed to a blend and built by anything else stay
    let made_by_a_blend: HashSet<Id> = p.timeline.iter().filter(|n| blended(&n.kind).is_some()).flat_map(|n| n.kind.bodies()).collect();
    let handed: Vec<Id> = p.timeline.iter().filter_map(|n| blended(&n.kind)).filter(|b| !made_by_a_blend.contains(b)).collect();
    assert!(!handed.is_empty(), "setup: no blend of the case stands on a body built by something else");
    let before: Vec<Option<Vec<u8>>> = handed.iter().map(|b| shapes.get(b).and_then(|s| s.to_brep_bytes())).collect();
    for n in p.timeline.iter_mut().filter(|n| blended(&n.kind).is_some()) {
        n.dirty = true;
    }
    let (_, shapes) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let moved: Vec<String> = handed.iter().zip(&before).filter(|(b, was)| shapes.get(b).and_then(|s| s.to_brep_bytes()) != **was).map(|(b, _)| format!("body {b}")).collect();
    assert!(moved.is_empty(), "{} of {} bodies handed to a blend changed under it: {}", moved.len(), handed.len(), moved.join(", "));
}
