//! THE UNION OF A PATTERN GROWS WITH ITS COPIES, NOT WITH THEIR SQUARE. Reported behaviour (issue #136): carrying the
//! names of the faces and edges across the union walked all of the result twice per copy, so 1,000 copies of a box
//! rebuilt in 16.0-16.9 s and 2,000 in 50.8-59.6 s; with one record of the names kept for the whole union, 3.4-3.5 s
//! and 6.7-6.8 s (back to back, a loaded machine).
//!
//! Beside it a sanity guard that every face of a pattern has a name of its own. (It cannot go red from a stale record of
//! the names: each copy's names are its own. The case the record decides is in qymcad-kernel,
//! `a_union_gives_every_face_a_name_of_its_own`.)
use qymcad_core::model::{ArrayAxis, Project};
use std::time::{Duration, Instant};

/// The copies of a grid pattern, along x and along y.
#[derive(Clone, Copy)]
struct Grid {
    along_x: u32,
    along_y: u32,
}

/// A document holding one 5 mm box and a linear pattern of it on `grid`, the copies 10 mm apart. Returns the document
/// and the pattern's body.
fn pattern_of_boxes(grid: Grid) -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(5.0, 5.0, 5.0);
    let axis = |d: [f64; 3], count: u32| ArrayAxis { d, count };
    let node = p.add_linear_array_grid3(block, [axis([10.0, 0.0, 0.0], grid.along_x), axis([0.0, 10.0, 0.0], grid.along_y), axis([0.0, 0.0, 10.0], 1)]);
    let body = p.timeline.iter().find(|n| n.id == node).and_then(|n| n.kind.body()).expect("the pattern makes a body");
    (p, body)
}

/// How long one rebuild of a pattern of boxes on `grid` takes.
fn rebuild_time(grid: Grid) -> Duration {
    let (mut p, _) = pattern_of_boxes(grid);
    let t = Instant::now();
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    let took = t.elapsed();
    assert!(report.errors.is_empty(), "the pattern did not build: {:?}", report.errors);
    took
}

#[test]
fn every_face_of_a_pattern_has_a_name_of_its_own() {
    let (mut p, body) = pattern_of_boxes(Grid { along_x: 4, along_y: 5 });
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let faces = p.regen_faces.get(&body).expect("the pattern's faces");
    assert_eq!(faces.len(), 6 * 20, "20 separate boxes have 120 faces");
    let mut ids: Vec<u32> = faces.iter().map(|f| f.id).collect();
    assert!(ids.iter().all(|&id| id != 0), "a face of the pattern has no name");
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 120, "two faces of the pattern share a name");
}

/// Four times the copies must take less than seven times as long. Measured, the best of two rebuilds each, 500 and then
/// 2,000 copies: without one record of the names 2.34 s and 25.15 s in one run (10.7 times), 2.34 s and 24.40 s in
/// another (10.4 times); with it 1.48 s and 6.51 s (4.4 times, as linear work gives). At 200 and 800 copies the old walk did not yet show (7.0 against 4.3), so the sizes are the
/// larger pair. The sizes are timed in turn and the best of two kept, so a slower spell of the machine falls on both.
#[test]
fn four_times_the_copies_take_about_four_times_as_long() {
    let (mut small, mut large) = (Duration::MAX, Duration::MAX);
    for _ in 0..2 {
        small = small.min(rebuild_time(Grid { along_x: 20, along_y: 25 }));
        large = large.min(rebuild_time(Grid { along_x: 40, along_y: 50 }));
    }
    assert!(large < small * 7, "500 copies took {small:?} and 2,000 took {large:?}: the union grew faster than its copies");
}
