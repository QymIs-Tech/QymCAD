//! An internal ACME Ø30 P5 thread in a hole showed flat discs on screen instead of turns.
//!
//! The hypothesis under test: the groove profile runs inwards from the surface, with y below zero, while for an
//! internal thread the material lies outside the surface of the hole — so the groove cuts the void and only the
//! overshoot reaches the body.
use qymcad_kernel::Shape;
use qymcad_core::thread::{encode_edges, ThreadSpec, ThreadStandard};

/// A sleeve: a Ø60 cylinder with an axial Ø30 hole, matching the failing part.
fn tube() -> Shape {
    let outer = Shape::cylinder(30.0, 110.0).expect("the outer cylinder");
    let hole = Shape::cylinder(15.0, 120.0).expect("the hole");
    outer.boolean(&hole, 0).expect("the sleeve")
}

/// The exact parameters from the failing file: ACME Ø30 P5, internal, a fit of 0.2, a length of 100 and a
/// run-out of 1.5.
fn user_spec() -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::Acme, nominal_d: 30.0, pitch: 5.0, internal: true, fit: 0.2, ..Default::default() }
}

#[test]
fn internal_thread_must_cut_into_the_wall_not_into_the_void() {
    let g = user_spec().geometry();
    let blank = tube();
    let v0 = blank.volume();
    let cut = blank
        .helical_profile([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], 15.0, &encode_edges(&g.groove), 100.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 1.5, 1.5, &[], &[], 0.0)
        .expect("the internal thread was built");
    let v1 = cut.volume();
    let removed = v0 - v1;
    // how much material an internal thread has to remove: roughly half the ring between Ø30 and Ø(30+2h)
    let ring = std::f64::consts::PI * ((15.0 + g.depth).powi(2) - 15.0_f64.powi(2)) * 100.0;
    assert!(cut.is_valid(), "the body is valid");
    assert!(
        removed > 0.2 * ring,
        "the internal thread removed almost nothing: {removed:.1} mm³ against a ring of {ring:.1} mm³, so the groove is cutting the void rather than the wall"
    );
    assert!(removed < 1.2 * ring, "and it did not gouge out too much: {removed:.1} against {ring:.1}");
}

/// A long thread of twenty turns has to build quickly and validly. The same feature used to take 160 seconds
/// and then fail: the boolean tools overlapped and cut into one another. The budget is generously loose — the
/// sentinel catches a return to the old behaviour, an order of magnitude apart, not a micro-regression.
#[test]
fn long_internal_thread_is_valid_and_not_glacial() {
    let g = user_spec().geometry();
    let t = std::time::Instant::now();
    let cut = tube()
        .helical_profile([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], 15.0, &encode_edges(&g.groove), 100.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 1.5, 1.5, &[], &[], 0.0)
        .expect("the 100 mm thread was built");
    let dt = t.elapsed();
    eprintln!("[perf] internal ACME Ø30 P5 over 100 mm, twenty turns: {dt:?}, V={:.1} mm³", cut.volume());
    assert!(cut.is_valid(), "a long thread gives a valid body");
    assert!(dt < std::time::Duration::from_secs(40), "building a long thread became expensive: {dt:?}");
}

/// A thread is uniform along its length: 100 mm has to remove exactly twice as much as 50 mm on the same body.
/// This catches turns lost or doubled where the spine is glued from several edges.
#[test]
fn thread_removes_volume_proportional_to_length() {
    let g = user_spec().geometry();
    let prof = encode_edges(&g.groove);
    let removed_at = |len: f64| {
        let b = tube();
        let v0 = b.volume();
        v0 - b
            .helical_profile([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], 15.0, &prof, len, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
            .expect("it was built")
            .volume()
    };
    let (half, full) = (removed_at(50.0), removed_at(100.0));
    let ratio = full / half;
    eprintln!("[thread] removed over 50 mm: {half:.1}, over 100 mm: {full:.1}, ratio {ratio:.3}");
    assert!((ratio - 2.0).abs() < 0.05, "turns are lost or doubled at the segment junction: the ratio is {ratio:.3} instead of 2.0");
}
