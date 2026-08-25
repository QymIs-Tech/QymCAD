//! SHARED METAL IS EITHER MEASURED OR REFUSED, NEVER GUESSED.
//!
//! Reported behaviour: the same slab of the same threaded rod weighs a different amount depending on where
//! along the rod it is taken — by tens of per cent. `interference_volume` is what an assembly uses to notice
//! that two parts occupy the same space, so a wrong answer there means a part can sit inside another one and
//! nobody is told.
//!
//! Two things are checked, and they are different things. First, that the number is right where arithmetic
//! knows the answer. Second, that when the kernel cannot produce a right number it says so instead of
//! returning a zero — because a zero already means "they do not touch", and one word cannot carry both
//! meanings.
//!
//! On the ground truth used here: a thread is PERIODIC. A slab one pitch thick, taken anywhere inside the
//! threaded length, holds the same metal as the same slab one pitch further along - the same core, the same
//! turn, the same groove, only shifted. Two such slabs that weigh differently are not two different pieces of
//! steel; they are one piece measured twice by an instrument that drifts.
use qymcad_kernel::Shape;
use qymcad_core::thread::{encode_edges, ThreadSpec, ThreadStandard};

const PI: f64 = std::f64::consts::PI;

/// A slab of space: a disc wide enough to swallow anything under test, `t` thick, its bottom at `z`.
fn slab(t: f64, z: f64) -> Shape {
    Shape::cylinder(50.0, t)
        .expect("the slab")
        .transformed(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z])
        .expect("the slab moved into place")
}

/// M10x1.5 cut over the whole of a 30 mm rod, with no run-out at either end.
///
/// No run-out on purpose: the run-out changes the turn near the ends, and this file is about the
/// INSTRUMENT, not about the thread. Everything measured here is taken from the middle, where every pitch
/// is the twin of the next.
fn threaded_rod() -> (Shape, f64, f64, f64) {
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, ..Default::default() };
    let g = spec.geometry();
    let rod = Shape::cylinder(g.stock_d * 0.5, 30.0).expect("the rod");
    let cut = rod
        .helical_profile(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            g.stock_d * 0.5,
            &encode_edges(&g.groove),
            30.0,
            g.lead,
            spec.starts,
            if spec.left { qymcad_kernel::Hand::Left } else { qymcad_kernel::Hand::Right }, qymcad_kernel::Helix::Groove,
            0.0,
            0.0,
            &[],
            &[],
            0.0,
        )
        .expect("the thread built");
    (cut, spec.pitch, g.major_d * 0.5, g.minor_d * 0.5)
}

/// WHERE ARITHMETIC KNOWS THE ANSWER, the instrument has to give it.
///
/// A plain rod against a flat slab: the shared metal is a disc, and its volume is pi r^2 t on paper. Nothing
/// here is hard for a kernel, and this check exists to separate "the instrument drifts" from "the instrument
/// is broken" - if this one fails too, nothing below means anything.
#[test]
fn a_plain_rod_shares_what_arithmetic_says() {
    let (r, t) = (5.0, 1.0);
    let rod = Shape::cylinder(r, 20.0).expect("the rod");
    let want = PI * r * r * t;
    for i in 0..18 {
        let z = 0.5 + i as f64;
        let got = rod.interference_volume(&slab(t, z)).expect("the kernel measured a plain rod");
        let off = (got - want) / want * 100.0;
        assert!(off.abs() < 1.0, "a slab at z={z} shares {got:.3} mm^3 against {want:.3} on paper, off by {off:.1}%");
    }
}

/// THE SAME SLAB OF A THREAD WEIGHS THE SAME WHEREVER IT IS TAKEN. This is the complaint, as one number.
///
/// A pitch-thick slab is stepped through the middle of the rod. Every one of them is the same metal, so the
/// spread between the lightest and the heaviest says exactly how far the instrument can be trusted. Measured
/// before the fix: tens of per cent, and once a flat zero - the kernel reporting that a rod does not touch a
/// slab it passes straight through.
#[test]
fn the_same_slab_of_a_thread_weighs_the_same_wherever_it_is_taken() {
    let (rod, pitch, r_major, r_minor) = threaded_rod();
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for i in 0..16 {
        let z = 8.0 + 0.25 * i as f64; // the middle of the rod, well clear of both ends
        let got = rod.interference_volume(&slab(pitch, z)).expect("the kernel measured a threaded rod");
        seen.push((z, got));
    }
    let lo = seen.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
    let hi = seen.iter().map(|s| s.1).fold(0.0, f64::max);
    let spread = (hi - lo) / hi * 100.0;
    eprintln!("a pitch-thick slab of the same thread: {lo:.3} to {hi:.3} mm^3, a spread of {spread:.1}%");
    for (z, v) in &seen {
        eprintln!("  z={z:.2}: {v:.3} mm^3");
    }

    // THE BOUNDS ARITHMETIC GIVES. The slab cannot hold less than the bare core and cannot hold more than a
    // solid rod at the crest; anything outside that is not a measurement at all.
    let (floor, ceiling) = (PI * r_minor * r_minor * pitch, PI * r_major * r_major * pitch);
    for (z, v) in &seen {
        assert!(*v > floor && *v < ceiling, "a slab at z={z:.2} shares {v:.3} mm^3, outside {floor:.3}..{ceiling:.3} which is the bare core and the solid rod");
    }
    assert!(spread < 2.0, "the same slab of the same thread weighs {lo:.3} in one place and {hi:.3} in another, a spread of {spread:.1}%");
}

/// "COULD NOT MEASURE" IS NOT "DOES NOT TOUCH".
///
/// The two answers used to leave the kernel as the same zero, and an assembly reading that zero says the
/// parts are clear of each other. They have to be different answers: a body clear of another gives a
/// measured zero, and only an actual failure gives nothing at all.
#[test]
fn a_clear_pair_measures_zero_rather_than_refusing() {
    let rod = Shape::cylinder(5.0, 20.0).expect("the rod");
    let far = slab(1.0, 100.0); // nowhere near the rod
    let got = rod.interference_volume(&far);
    assert_eq!(got, Some(0.0), "two bodies far apart share a measured nothing, not a refusal: {got:?}");
}

/// THE INTEGRATOR AGAINST A SECOND OPINION. Run by hand: it is a measurement, not a rule.
///
/// The shared solid is built here by hand so that its volume can be taken twice - once by `GProp`, which is
/// what `interference_volume` reports, and once by summing the tetrahedra of its mesh, which is arithmetic
/// on triangles and has no opinion about surfaces. A gap between the two is the instrument's error, in
/// per cent, on exactly the kind of body an assembly asks it about.
#[test]
#[ignore]
fn how_far_the_integrator_drifts_on_a_thread() {
    let (rod, pitch, _, _) = threaded_rod();
    for t in [0.1, 0.25, 0.5, pitch, 2.0 * pitch] {
        for i in 0..4 {
            let z = 8.0 + 0.37 * i as f64;
            let cut = rod.boolean(&slab(t, z), 2).expect("the shared solid");
            let by_gprop = cut.volume();
            let by_mesh: f64 = cut.tessellate(0.005).iter().map(|b| b.0.volume()).sum();
            let off = if by_mesh > 0.0 { (by_gprop - by_mesh) / by_mesh * 100.0 } else { f64::NAN };
            eprintln!("slab {t:>5} mm at z={z:.2}: GProp {by_gprop:>9.3}, mesh {by_mesh:>9.3}, off by {off:>7.1}%");
        }
    }
}
