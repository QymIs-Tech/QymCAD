//! A THREAD SAYS WHAT ITS COUNTERPART NEEDS.
//!
//! Asked for plainly: for an external thread as well as an internal one, a person must be told what
//! parameters the mating thread takes and what diameter of shaft or hole it needs, so that the two screw
//! together. The numbers are all computed already; what was missing was saying them out loud.
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

fn m(d: f64, pitch: f64, internal: bool, fit: f64) -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, internal, fit, ..Default::default() }
}

/// THE COUNTERPART IS THE SAME THREAD ON THE OTHER SIDE.
#[test]
fn the_counterpart_differs_only_by_its_side() {
    let a = ThreadSpec { starts: 2, left: true, ..m(12.0, 1.75, false, 0.15) };
    let b = a.mating();
    assert!(b.internal, "the counterpart of an external thread is an internal one");
    assert_eq!(b.standard, a.standard);
    assert_eq!(b.nominal_d, a.nominal_d);
    assert_eq!(b.pitch, a.pitch);
    assert_eq!(b.starts, a.starts, "a two-start thread mates with a two-start one");
    assert_eq!(b.left, a.left, "a left-hand thread mates with a left-hand one");
    assert_eq!(b.fit, a.fit, "the fit is one value for the pair: it thins one side and thickens the other");
    assert!(!b.mating().internal, "the counterpart of the counterpart is the thread itself");
}

/// THE BLANK DIAMETERS: a shaft for the outside, a drilled hole for the inside.
#[test]
fn the_blank_diameters_are_the_shaft_and_the_drill() {
    let ext = m(10.0, 1.5, false, 0.2);
    let (own, mate) = ext.blank_diameters();
    assert_eq!(own, ext.geometry().major_d, "an external thread is cut from a shaft at the major diameter");
    assert_eq!(mate, ext.mating().geometry().minor_d, "its counterpart is cut in a hole at the minor diameter");
    assert!(mate < own, "the drill is smaller than the shaft, or there would be nothing left to cut: {mate} against {own}");

    // AND THE OTHER WAY ROUND, so that the answer does not depend on which side is being asked about.
    let int = m(10.0, 1.5, true, 0.2);
    let (own_i, mate_i) = int.blank_diameters();
    assert_eq!(own_i, mate, "the hole an internal thread is cut in is the same number");
    assert_eq!(mate_i, own, "and the shaft of its counterpart is the same too");
}

/// THE HOLE IS THE ONE THIS PROGRAM'S OWN THREAD NEEDS, and it is close to the standard's.
///
/// Two different numbers get called "the hole for an M10x1.5" and they must not be confused. A tap drill
/// from a workshop table is 8.5 mm: that is chosen for cutting metal with a tap at about 75% engagement.
/// The MODELLED thread has no tap - the groove is cut outwards from the bore - so the bore has to be the
/// thread's own minor diameter, and telling a person 8.5 here would leave the model with a loose thread.
///
/// What is checked is that the number stays near the standard's D1 = D - 1.0825·P, so that a part made
/// here still matches a real bolt.
#[test]
fn the_hole_is_the_minor_diameter_and_near_the_standard() {
    for (d, pitch) in [(10.0, 1.5), (8.0, 1.25), (6.0, 1.0), (5.0, 0.8)] {
        let (_own, hole) = m(d, pitch, false, 0.0).blank_diameters();
        let iso_d1 = d - 1.0825 * pitch;
        assert!(
            (hole - iso_d1).abs() < 0.25,
            "M{d}x{pitch}: the hole comes out {hole:.3} mm against the standard's D1 = {iso_d1:.3} - a part made here would not fit a real bolt"
        );
    }
}

/// A DEPTH THE GROOVE CANNOT REACH IS NOT A THREAD.
///
/// Reported: at Ø40, pitch 5, an included angle of 80 degrees and a depth of 3.6 the turns come out as flat
/// plates and nothing screws into them.
///
/// And so they must. The groove's half-width at the surface is what the pitch has left after the crest's
/// flat, capped so that a web stays between neighbouring turns. At 80 degrees the flanks close to a point
/// 2.68 mm down, so a 3.6 mm groove cannot be cut at all: what comes out is a shallower V with a sliver of
/// land between the turns — the plates that were reported.
#[test]
fn a_groove_wider_than_the_pitch_is_refused() {
    let reported = ThreadSpec {
        standard: ThreadStandard::Custom,
        nominal_d: 40.0,
        pitch: 5.0,
        custom_angle: 80.0,
        custom_depth: 3.6,
        ..Default::default()
    };
    let (width, max_depth, min_pitch) = reported.profile_overflow().expect("this profile does not fit and must say so");
    assert!((width - 6.04).abs() < 0.05, "the V at that depth would be {width:.2} mm across, expected about 6.04");
    assert!((max_depth - 2.68).abs() < 0.05, "at this angle and pitch the depth reaches {max_depth:.2}, expected about 2.68");
    assert!(min_pitch > 5.0, "at this depth the pitch must be larger than 5, and {min_pitch:.2} is offered");

    // THE SAME PROFILE WITH ROOM: a depth that fits is accepted without a word.
    let ok = ThreadSpec { custom_depth: 2.5, ..reported.clone() };
    assert!(ok.profile_overflow().is_none(), "a depth of 2.5 fits at a pitch of 5 and must not be refused");

    // AND EVERY STANDARD PROFILE FITS at its own coarse pitch - or the check would refuse ordinary work.
    for (d, pitch) in [(10.0, 1.5), (20.0, 2.5), (6.0, 1.0), (40.0, 4.5)] {
        let m = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, ..Default::default() };
        assert!(m.profile_overflow().is_none(), "M{d}x{pitch} was refused, and it is an ordinary thread");
    }
}

/// THE CLEARANCE ASKED FOR IS THE CLEARANCE GIVEN — or the person is told it is not.
///
/// The groove's half-width is clamped to 0.49 of the pitch, and for a metric profile it already sits at
/// 0.4375 of it before any clearance is added. Only about 0.05 of the pitch is left for the fit: 0.13 mm at
/// a 2.5 pitch, 0.08 mm at 1.5. A person typing 0.2 gets 0.08 and is told nothing, so the pair binds and
/// nothing explains why. Measured: at M20x2.5 a fit of 0.2 and one of 0.4 gave bit-identical bodies, both
/// sharing 264.3 mm^3 with the mating part.
#[test]
fn a_clearance_that_does_not_fit_is_named() {
    let m20 = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 20.0, pitch: 2.5, fit: 0.2, ..Default::default() };
    let over = m20.fit_overflow();
    assert!(over.is_some(), "a fit of 0.2 at a pitch of 2.5 does not fit and must say so");
    let (asked, given) = over.expect("the numbers");
    assert_eq!(asked, 0.2);
    assert!(given < asked, "the clearance given ({given:.3}) must be honestly smaller than the one asked for");
    assert!((given - 0.131).abs() < 0.01, "at a pitch of 2.5 about 0.131 mm fits, and {given:.3} is reported");

    // TWO DIFFERENT VALUES THAT BOTH OVERFLOW report the same achievable clearance - that is exactly the
    // measurement that gave two identical bodies.
    let m20_wide = ThreadSpec { fit: 0.4, ..m20.clone() };
    assert_eq!(m20_wide.fit_overflow().map(|(_, g)| g), Some(given), "both overflowing values end at the same ceiling");

    // A CLEARANCE THAT FITS IS NOT COMPLAINED ABOUT.
    let small = ThreadSpec { fit: 0.05, ..m20.clone() };
    assert!(small.fit_overflow().is_none(), "0.05 mm fits at a pitch of 2.5 and must pass without a word");
}

/// WHAT WILL NOT FIT INTO THE GROOVE IS TAKEN OFF THE CREST.
///
/// The clearance is capped by the pitch, and the remainder has to come from somewhere or the pair binds. A
/// real fit class takes it off the diameters: the bolt shrinks, the nut opens up. The two are not swapped
/// one for one — a flank stands at the half-angle, so a radial move of `e` is worth `e·sin(beta)` along the
/// flank while widening the groove by `w` is worth `w·cos(beta)`.
#[test]
fn the_clearance_that_does_not_fit_is_taken_off_the_crest() {
    let m10 = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, fit: 0.2, ..Default::default() };
    let (asked, given) = m10.fit_overflow().expect("0.2 does not fit at a pitch of 1.5");
    let e = m10.radial_relief();
    assert!(e > 0.0, "the clearance overflows and nothing is taken off the crest: the pair keeps binding");

    // The relation the two are matched by, checked rather than restated: e·tan(beta) = what was missing.
    let missing = asked - given;
    let t = 30.0_f64.to_radians().tan(); // half of the 60 degree metric profile
    assert!((e * t - missing).abs() < 1e-9, "the crest comes down by {e:.4}, which is worth {:.4} along the flank against the {missing:.4} that was missing", e * t);

    // A CLEARANCE THAT FITS TAKES NOTHING OFF: ordinary work is not touched.
    let small = ThreadSpec { fit: 0.05, ..m10.clone() };
    assert_eq!(small.radial_relief(), 0.0, "a clearance that fits inside the groove must not shrink the thread as well");
    let none = ThreadSpec { fit: 0.0, ..m10.clone() };
    assert_eq!(none.radial_relief(), 0.0);
}
