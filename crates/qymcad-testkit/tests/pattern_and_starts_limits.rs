//! A PATTERN HOLDS AT MOST `MAX_PATTERN_INSTANCES`, A THREAD OR AN AUGER AT MOST `MAX_HELIX_STARTS` STARTS. Reported
//! behaviour (issue #103): a 65,536 x 65,536 component grid wrapped to a total of 0 and the next rebuild divided by it;
//! a count from a formula made a pattern reserve room for every copy before anything checked it.
//!
//! A binary of its own: on a tree without the limits the circular pattern aborts the process on a failed request for
//! 384,000,000,000 bytes, and an abort here names this file rather than the checks beside it. (The linear and the
//! component grid panicked on the overflowing product; the thread and the auger with a billion starts were still
//! running after five minutes.)
use qymcad_core::errors::CoreError;
use qymcad_core::feature::{pattern_instances, FeatureKind, MAX_HELIX_STARTS, MAX_PATTERN_INSTANCES};
use qymcad_core::model::{ArrayAxis, CompPatternKind, Project};
use qymcad_core::thread::{AugerSpec, ThreadSpec, ThreadStandard};

fn refused(p: &mut Project, node: u64) -> Vec<CoreError> {
    let (report, _) = qymcad_testkit::regenerate(p);
    report.errors.iter().filter(|(id, _)| *id == node).map(|(_, e)| e.clone()).collect()
}

/// The limit itself, on the count alone: building 1,000 real copies in a check takes seconds it does not need.
#[test]
fn the_count_stops_at_the_limit_and_does_not_wrap() {
    assert_eq!(MAX_PATTERN_INSTANCES, 1_000);
    assert_eq!(pattern_instances(&[10, 100]), Some(1_000), "the limit itself is allowed");
    assert_eq!(pattern_instances(&[10, 101]), None, "one row past it is not");
    assert_eq!(pattern_instances(&[65_536, 65_536]), None, "a product past 32 bits is refused, not wrapped to 0");
    assert_eq!(pattern_instances(&[u32::MAX, u32::MAX, u32::MAX]), None);
    assert_eq!(pattern_instances(&[0, 3]), Some(3), "a count of 0 reads as 1, as before");
}

#[test]
fn a_linear_pattern_past_the_limit_is_refused_in_words() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(10.0, 10.0, 10.0);
    let axis = |d: [f64; 3], count: u32| ArrayAxis { d, count };
    let lin = p.add_linear_array_grid3(block, [axis([20.0, 0.0, 0.0], 100_000), axis([0.0, 20.0, 0.0], 100_000), axis([0.0, 0.0, 20.0], 1)]);
    let errs = refused(&mut p, lin);
    assert!(errs.iter().any(|e| matches!(e, CoreError::PatternTooLarge { asked: 10_000_000_000, limit } if *limit == MAX_PATTERN_INSTANCES)), "{errs:?}");
}

#[test]
fn a_circular_pattern_past_the_limit_is_refused_in_words() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(10.0, 10.0, 10.0);
    let circ = p.add_circular_array(block, 4_000_000_000, 360.0);
    let errs = refused(&mut p, circ);
    assert!(errs.iter().any(|e| matches!(e, CoreError::PatternTooLarge { asked: 4_000_000_000, .. })), "{errs:?}");
}

/// A COMPONENT GRID WHOSE PRODUCT WRAPPED: it is not added, not set, and when a file brings one, the rebuild refuses it
/// in words instead of dividing by zero.
#[test]
fn a_component_grid_past_the_limit_is_refused_not_divided_by() {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("Bolt");
    p.set_active_component(Some(part));
    p.add_box(20.0, 20.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);

    let huge = CompPatternKind::Linear { dir: [1.0, 0.0, 0.0], step: 30.0, count: 65_536, more: [([0.0, 1.0, 0.0], 30.0, 65_536), ([0.0, 0.0, 1.0], 0.0, 1)] };
    assert_eq!(huge.count(), 0, "past the limit the count says so");
    assert_eq!(p.add_comp_pattern(part, huge), 0, "a grid past the limit was added");

    let id = p.add_comp_pattern(part, CompPatternKind::linear([1.0, 0.0, 0.0], 30.0, 3));
    assert_ne!(id, 0, "a pattern of three is added");
    let _ = qymcad_testkit::regenerate(&mut p);
    assert!(!p.set_comp_pattern(id, huge), "a pattern was set to a grid past the limit");

    // as a file would bring it: the layout written straight into the node, its two copies still there
    for n in p.timeline.iter_mut().filter(|n| n.id == id) {
        if let FeatureKind::ComponentPattern { kind, .. } = &mut n.kind {
            *kind = huge;
        }
        n.dirty = true;
    }
    let errs = refused(&mut p, id);
    assert!(errs.iter().any(|e| matches!(e, CoreError::PatternTooLarge { .. })), "{errs:?}");
}

#[test]
fn a_thread_with_too_many_starts_is_refused_in_words() {
    let mut p = Project::default();
    p.new_document();
    let blank = p.add_cylinder(5.0, 20.0);
    let e = qymcad_testkit::round_edge(&mut p, blank, 5.0);
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, starts: 1_000_000_000, ..Default::default() };
    let t = p.add_thread(blank, e, spec, 20.0, 0.0, 0.0);
    let errs = refused(&mut p, t);
    assert!(errs.iter().any(|e| matches!(e, CoreError::TooManyStarts { starts: 1_000_000_000, limit } if *limit == MAX_HELIX_STARTS)), "{errs:?}");
}

#[test]
fn an_auger_with_too_many_starts_is_refused_in_words() {
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(5.0, 40.0);
    let e = qymcad_testkit::round_edge(&mut p, shaft, 5.0);
    let spec = AugerSpec { starts: 1_000_000_000, ..Default::default() };
    let a = p.add_auger(shaft, e, spec, 40.0, 0.0, 0.0);
    let errs = refused(&mut p, a);
    assert!(errs.iter().any(|e| matches!(e, CoreError::TooManyStarts { starts: 1_000_000_000, .. })), "{errs:?}");
}
