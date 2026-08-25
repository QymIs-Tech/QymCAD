//! Thread geometry is checked against the reference tables of the standards rather than by eye.
//!
//! The profile used to be built from invented coefficients, 0.48·P and 0.30·P, tied to no standard at all,
//! while the depth and the angle were typed in by hand — which is why the tool did not work.
//!
//! The references: ISO 68-1 and ISO 261 for metric threads, ISO 2901 and ISO 2904 for trapezoidal Tr, and
//! DIN 405 for round Rd.
use qymcad_core::geom::ProfEdge;
use qymcad_core::thread::{metric_coarse_pitch, AugerSpec, ThreadSpec, ThreadStandard};

fn spec(std: ThreadStandard, d: f64, p: f64) -> ThreadSpec {
    ThreadSpec { standard: std, nominal_d: d, pitch: p, ..Default::default() }
}

/// The coarse pitch of a metric thread, per ISO 261.
#[test]
fn metric_coarse_pitch_matches_iso_261() {
    let mut bad = Vec::new();
    for (d, p) in [(3.0, 0.5), (4.0, 0.7), (5.0, 0.8), (6.0, 1.0), (8.0, 1.25), (10.0, 1.5), (12.0, 1.75), (16.0, 2.0), (20.0, 2.5), (24.0, 3.0), (30.0, 3.5), (36.0, 4.0)] {
        let got = metric_coarse_pitch(d);
        if (got - p).abs() > 1e-9 {
            bad.push(format!("M{d}: pitch {got}, ISO 261 says {p}"));
        }
    }
    assert!(bad.is_empty(), "the coarse pitch table disagrees with the standard:\n{}", bad.join("\n"));
}

/// Metric ISO: the pitch and minor diameters follow the ISO 68-1 formulas — d2 = d − 0.649519P,
/// d3 = d − 1.226869P, h3 = 0.613434P — and are checked against the tabulated values.
#[test]
fn metric_diameters_match_iso_68() {
    let mut bad = Vec::new();
    // (designation, d, P, tabulated d2, tabulated d3)
    let cases = [
        ("M6", 6.0, 1.0, 5.350, 4.773),
        ("M8", 8.0, 1.25, 7.188, 6.466),
        ("M10", 10.0, 1.5, 9.026, 8.160),
        ("M12", 12.0, 1.75, 10.863, 9.853),
        ("M20", 20.0, 2.5, 18.376, 16.933),
    ];
    for (name, d, p, d2, d3) in cases {
        let g = spec(ThreadStandard::MetricIso, d, p).geometry();
        if (g.pitch_d - d2).abs() > 0.002 {
            bad.push(format!("{name}: d2 = {:.3}, tabulated {d2:.3}", g.pitch_d));
        }
        if (g.minor_d - d3).abs() > 0.002 {
            bad.push(format!("{name}: d3 = {:.3}, tabulated {d3:.3}", g.minor_d));
        }
        if (g.depth - 0.613_434 * p).abs() > 1e-6 {
            bad.push(format!("{name}: depth {:.4}, ISO gives 0.6134·P = {:.4}", g.depth, 0.613_434 * p));
        }
        if (g.angle_deg - 60.0).abs() > 1e-9 {
            bad.push(format!("{name}: angle {} instead of 60°", g.angle_deg));
        }
    }
    assert!(bad.is_empty(), "the metric thread disagrees with ISO 68-1:\n{}", bad.join("\n"));
}

/// Trapezoidal Tr per ISO 2901: d2 = d − 0.5P, h3 = 0.5P + ac, d3 = d − 2h3, at an angle of 30°. Checked
/// against the ISO 2904 table, where Tr20×4 gives d2 = 18.0 and d3 = 15.5, and Tr40×7 gives d2 = 36.5 and
/// d3 = 32.0.
#[test]
fn trapezoidal_diameters_match_iso_2904() {
    let mut bad = Vec::new();
    for (name, d, p, d2, d3) in [("Tr20x4", 20.0, 4.0, 18.0, 15.5), ("Tr40x7", 40.0, 7.0, 36.5, 32.0), ("Tr8x1.5", 8.0, 1.5, 7.25, 6.2)] {
        let g = spec(ThreadStandard::TrapezoidalTr, d, p).geometry();
        if (g.pitch_d - d2).abs() > 0.01 {
            bad.push(format!("{name}: d2 = {:.3}, ISO 2904 gives {d2:.3}", g.pitch_d));
        }
        if (g.minor_d - d3).abs() > 0.01 {
            bad.push(format!("{name}: d3 = {:.3}, ISO 2904 gives {d3:.3}", g.minor_d));
        }
        if (g.angle_deg - 30.0).abs() > 1e-9 {
            bad.push(format!("{name}: angle {} instead of 30°", g.angle_deg));
        }
    }
    assert!(bad.is_empty(), "the trapezoidal thread disagrees with ISO 2904:\n{}", bad.join("\n"));
}

/// With no pitch given the standard coarse one is taken: choosing a size is enough, as it is in any
/// professional CAD.
#[test]
fn omitted_pitch_falls_back_to_standard_coarse() {
    let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 0.0, ..Default::default() }.geometry();
    assert!((g.pitch - 1.5).abs() < 1e-9, "M10 without a pitch takes the coarse 1.5, got {}", g.pitch);
    let t = ThreadSpec { standard: ThreadStandard::TrapezoidalTr, nominal_d: 20.0, pitch: 0.0, ..Default::default() }.geometry();
    assert!((t.pitch - 4.0).abs() < 1e-9, "Tr20 without a pitch takes the coarse 4, got {}", t.pitch);
}

/// The groove profile is a closed contour of exact edges lying within the pitch and the depth.
///
/// The edges matter not for their own sake: a faceted profile cannot be chamfered or filleted afterwards.
#[test]
fn groove_profile_is_closed_exact_and_within_bounds() {
    let mut bad = Vec::new();
    for std in [ThreadStandard::MetricIso, ThreadStandard::TrapezoidalTr, ThreadStandard::Acme, ThreadStandard::RoundRd, ThreadStandard::Buttress] {
        let g = spec(std, 20.0, 2.5).geometry();
        let ends = |e: &ProfEdge| match *e {
            ProfEdge::Line { a, b } => (a, b),
            ProfEdge::Arc { a, b, .. } => (a, b),
            ProfEdge::Circle { center, .. } => (center, center),
        };
        if g.groove.len() < 3 {
            bad.push(format!("{:?}: profile of {} edges", std, g.groove.len()));
            continue;
        }
        // closure: the end of each edge is the start of the next
        for w in 0..g.groove.len() {
            let (_, b) = ends(&g.groove[w]);
            let (a2, _) = ends(&g.groove[(w + 1) % g.groove.len()]);
            if b.dist(a2) > 1e-9 {
                bad.push(format!("{:?}: the contour breaks between edges {w} and {}", std, (w + 1) % g.groove.len()));
                break;
            }
        }
        // extents: no wider than the pitch along the axis and no deeper than the depth radially, plus the
        // overshoot
        for e in &g.groove {
            let (a, b) = ends(e);
            for pnt in [a, b] {
                if pnt.x.abs() > g.pitch * 0.75 {
                    bad.push(format!("{:?}: a profile point lies outside the pitch: x = {:.3} at P = {:.3}", std, pnt.x, g.pitch));
                }
                if pnt.y < -g.depth - 1e-6 {
                    bad.push(format!("{:?}: a point lies deeper than the thread: y = {:.3} at h = {:.3}", std, pnt.y, g.depth));
                }
            }
        }
    }
    assert!(bad.is_empty(), "the groove profile is broken:\n{}", bad.join("\n"));
}

/// Rounding of the root and the crest: on round Rd and on a trapezoidal thread the root is an arc rather than
/// a corner. For 3D printing this matters: a sharp root is a stress concentrator and traps dirt between
/// layers.
#[test]
fn rounded_forms_use_real_arcs() {
    let rd = spec(ThreadStandard::RoundRd, 20.0, 2.5).geometry();
    let arcs = rd.groove.iter().filter(|e| matches!(e, ProfEdge::Arc { .. })).count();
    assert!(arcs >= 1, "a round Rd thread has to carry a root arc; edges: {:?}", rd.groove);

    // an explicitly given root radius also turns into an arc
    let custom = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, root_r: Some(0.3), ..Default::default() }.geometry();
    assert!(custom.groove.iter().any(|e| matches!(e, ProfEdge::Arc { .. })), "a given root radius has to produce an arc");

    // zero radii give a sharp profile with no arcs, the classic metric thread with a flat root
    let sharp = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, root_r: Some(0.0), crest_r: Some(0.0), ..Default::default() }.geometry();
    assert!(sharp.groove.iter().all(|e| matches!(e, ProfEdge::Line { .. })), "zero radii leave no arcs");
}

/// The fit clearance, which is what printed threads exist for: the larger the clearance, the wider the groove
/// and the thinner the thread, so that a bolt and a nut screw together. Monotonicity is checked, along with the
/// thread not disappearing altogether.
#[test]
fn print_fit_widens_groove_monotonically() {
    let width = |fit: f64| {
        let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, fit, ..Default::default() }.geometry();
        g.groove
            .iter()
            .flat_map(|e| match *e {
                ProfEdge::Line { a, b } => vec![a.x, b.x],
                ProfEdge::Arc { a, b, .. } => vec![a.x, b.x],
                ProfEdge::Circle { center, .. } => vec![center.x],
            })
            .fold(0.0_f64, |m, x| m.max(x.abs()))
    };
    let (w0, w1, w2) = (width(0.0), width(0.2), width(0.4));
    assert!(w1 > w0, "the clearance has to widen the groove: {w0:.3} -> {w1:.3}");
    assert!(w2 >= w1, "and must not narrow it back: {w1:.3} -> {w2:.3}");
    // The limit: the groove stops at 0.49·P, so the web between turns survives even an absurd clearance.
    // Otherwise the thread would consume itself and the body would fall apart.
    assert!(w2 < 0.75 * 1.5, "the web between turns remains: {w2:.3} at P = 1.5");
    let huge = width(5.0);
    assert!(huge < 0.75 * 1.5, "even an absurd clearance of 5 mm does not consume the thread: {huge:.3}");
}

/// An internal thread is cut from a hole of the minor diameter and an external one from a shaft of the major
/// diameter. Confusing the two produces a thread of the wrong size.
#[test]
fn stock_diameter_depends_on_internal_or_external() {
    let ext = ThreadSpec { nominal_d: 10.0, pitch: 1.5, internal: false, ..Default::default() }.geometry();
    let int = ThreadSpec { nominal_d: 10.0, pitch: 1.5, internal: true, ..Default::default() }.geometry();
    assert!((ext.stock_d - ext.major_d).abs() < 1e-9, "an external thread is cut from a shaft where d = major");
    assert!((int.stock_d - int.minor_d).abs() < 1e-9, "an internal thread is cut from a hole where d = minor");
    assert!(int.minor_d < int.major_d, "the minor diameter is smaller than the major one");
}

/// Multiple starts: the lead is the pitch times the number of starts, which is the governing parameter for
/// augers and fast threads.
#[test]
fn lead_accounts_for_multiple_starts() {
    let g = ThreadSpec { nominal_d: 20.0, pitch: 2.5, starts: 3, ..Default::default() }.geometry();
    assert!((g.lead - 7.5).abs() < 1e-9, "a three-start 2.5 mm thread has a lead of 7.5, got {}", g.lead);
}

/// An auger: the flight is material added outwards from the shaft rather than a groove cut into it. The tool
/// used to be able to cut only, so an auger could not be made at all.
#[test]
fn auger_flight_is_additive_ribbon_with_rounded_edges() {
    let a = AugerSpec { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, edge_r: 1.0, ..Default::default() };
    assert!((a.flight_height() - 10.0).abs() < 1e-9, "the flight height is (30 − 10)/2");
    let prof = a.flight_profile();
    let ys: Vec<f64> = prof
        .iter()
        .flat_map(|e| match *e {
            ProfEdge::Line { a, b } => vec![a.y, b.y],
            ProfEdge::Arc { a, b, .. } => vec![a.y, b.y],
            ProfEdge::Circle { center, .. } => vec![center.y],
        })
        .collect();
    assert!(ys.iter().cloned().fold(f64::MIN, f64::max) > 0.0, "the flight runs outwards from the shaft, y > 0");
    assert!(ys.iter().cloned().fold(f64::MAX, f64::min) < 0.0, "the bottom of the flight is sunk into the shaft, giving a clean union");
    assert!(prof.iter().filter(|e| matches!(e, ProfEdge::Arc { .. })).count() >= 2, "the edges of the flight are rounded by arcs");
    assert!((a.lead() - 20.0).abs() < 1e-9);
    let sharp = AugerSpec { edge_r: 0.0, ..a }.flight_profile();
    assert!(sharp.iter().all(|e| matches!(e, ProfEdge::Line { .. })), "without rounding it is a rectangle");
}

/// Degenerate inputs must not produce garbage: a zero or negative pitch, a zero diameter.
#[test]
fn degenerate_specs_are_sane() {
    let g = ThreadSpec { nominal_d: 0.0, pitch: -1.0, ..Default::default() }.geometry();
    assert!(g.pitch > 0.0 && g.depth > 0.0 && g.groove.len() >= 3, "a degenerate input still yields a valid profile: {g:?}");
    let a = AugerSpec { shaft_d: 30.0, outer_d: 10.0, ..Default::default() }; // the shaft is thicker than the flight
    assert_eq!(a.flight_height(), 0.0, "a flight height is never negative");
}

/// Rounding the crest of a thread. The crest is the material between grooves, so the rounding is taken off the
/// upper corner of the groove by an arc. The check is that the arcs appear and that the profile stays
/// closed.
#[test]
fn crest_rounding_adds_arcs_and_keeps_contour_closed() {
    let sharp = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, crest_r: Some(0.0), root_r: Some(0.0), ..Default::default() }.geometry();
    let round = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, crest_r: Some(0.25), root_r: Some(0.0), ..Default::default() }.geometry();
    let arcs = |g: &qymcad_core::thread::ThreadGeom| g.groove.iter().filter(|e| matches!(e, ProfEdge::Arc { .. })).count();
    assert_eq!(arcs(&sharp), 0, "without a crest radius there are no arcs");
    assert_eq!(arcs(&round), 2, "a crest radius gives one arc on each side of the groove");

    // the contour is closed, or the kernel cannot build a wire from it
    let ends = |e: &ProfEdge| match *e {
        ProfEdge::Line { a, b } => (a, b),
        ProfEdge::Arc { a, b, .. } => (a, b),
        ProfEdge::Circle { center, .. } => (center, center),
    };
    for k in 0..round.groove.len() {
        let (_, b) = ends(&round.groove[k]);
        let (a2, _) = ends(&round.groove[(k + 1) % round.groove.len()]);
        assert!(b.dist(a2) < 1e-9, "the contour breaks at edge {k}: {b:?} -> {a2:?}");
    }

    // round Rd rounds both the root and the crest, giving four arcs
    let rd = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 20.0, pitch: 2.5, ..Default::default() }.geometry();
    assert!(arcs(&rd) >= 3, "round Rd has both root and crest rounded: {} arcs", arcs(&rd));
}

/// An internal thread is cut into the wall of the hole rather than into its void.
///
/// The profile is built relative to the surface: y = 0 is the surface itself, and going into the material means
/// y < 0 on a shaft but y > 0 inside a hole. While one profile served both, an internal thread removed only the
/// overshoot and produced flat discs instead of turns.
#[test]
fn internal_groove_goes_into_the_wall_external_into_the_shaft() {
    let ys = |g: &qymcad_core::thread::ThreadGeom| {
        let mut v: Vec<f64> = Vec::new();
        for e in &g.groove {
            match *e {
                ProfEdge::Line { a, b } => v.extend([a.y, b.y]),
                ProfEdge::Arc { a, b, center, .. } => v.extend([a.y, b.y, center.y]),
                ProfEdge::Circle { center, .. } => v.push(center.y),
            }
        }
        (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max))
    };
    for std in [ThreadStandard::MetricIso, ThreadStandard::TrapezoidalTr, ThreadStandard::Acme, ThreadStandard::RoundRd] {
        let base = ThreadSpec { standard: std, nominal_d: 30.0, pitch: 5.0, ..Default::default() };
        let (ext_lo, ext_hi) = ys(&base.geometry());
        let (int_lo, int_hi) = ys(&ThreadSpec { internal: true, ..base }.geometry());
        assert!(ext_lo < 0.0 && ext_hi > 0.0, "{std:?}: the external groove goes into the shaft, y < 0, with the overshoot outwards");
        assert!(int_hi > 0.0 && int_lo < 0.0, "{std:?}: the internal groove goes into the wall, y > 0, with the overshoot into the hole");
        // and it is exactly a mirror: the same depth with the opposite sign
        assert!((int_hi + ext_lo).abs() < 1e-9, "{std:?}: internal depth {int_hi:.4} is not the mirror of the external {ext_lo:.4}");
        assert!((int_lo + ext_hi).abs() < 1e-9, "{std:?}: internal overshoot {int_lo:.4} is not the mirror of the external {ext_hi:.4}");
    }
    // the contour of an internal thread stays closed: mirroring must not break the traversal of the arcs
    let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 30.0, pitch: 5.0, internal: true, ..Default::default() }.geometry();
    let ends = |e: &ProfEdge| match *e {
        ProfEdge::Line { a, b } => (a, b),
        ProfEdge::Arc { a, b, .. } => (a, b),
        ProfEdge::Circle { center, .. } => (center, center),
    };
    for k in 0..g.groove.len() {
        let (_, b) = ends(&g.groove[k]);
        let (a2, _) = ends(&g.groove[(k + 1) % g.groove.len()]);
        assert!(b.dist(a2) < 1e-9, "mirroring broke the contour at edge {k}");
    }
}
