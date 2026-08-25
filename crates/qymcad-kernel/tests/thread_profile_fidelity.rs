//! The main check of shape: a built thread has to remove exactly the volume its profile describes.
//!
//! The reference comes from Pappus's theorem. A helical motion is a rotation plus a translation along the axis,
//! and that translation lies in the plane of the profile, so it sweeps no volume. The material removed is
//! therefore the area of the profile — the part below the surface — times the path length of its centroid. That
//! is a truth independent of the kernel and can be checked against.
//!
//! On measurement: the volume is taken from the mesh, not from `Shape::volume()`. The kernel's integrator,
//! `BRepGProp::VolumeProperties`, is monstrously wrong on helical surfaces — down to −211% on the very part
//! whose mesh volume agrees with the calculation to 0.1%. It was that lying instrument that led the
//! investigation astray: by it the construction appeared to remove 12–26% more than asked, while the shape was
//! correct.
use qymcad_kernel::Shape;
use qymcad_core::geom::{Point2, ProfEdge};
use qymcad_core::thread::{encode_edges, AugerSpec, ThreadSpec, ThreadStandard};

/// A profile as a dense polyline, arcs being split finely so the area comes out exact.
fn poly(edges: &[ProfEdge]) -> Vec<Point2> {
    let mut p = Vec::new();
    for e in edges {
        match *e {
            ProfEdge::Line { a, b } => {
                p.push(a);
                p.push(b);
            }
            ProfEdge::Arc { a, b, center, ccw } => {
                let r = (a.x - center.x).hypot(a.y - center.y);
                let (t0, t1) = ((a.y - center.y).atan2(a.x - center.x), (b.y - center.y).atan2(b.x - center.x));
                let mut d = t1 - t0;
                if ccw {
                    while d <= 0.0 {
                        d += std::f64::consts::TAU;
                    }
                } else {
                    while d >= 0.0 {
                        d -= std::f64::consts::TAU;
                    }
                }
                let n = ((d.abs() / 0.005).ceil() as usize).max(2);
                for i in 0..=n {
                    let t = t0 + d * i as f64 / n as f64;
                    p.push(Point2::new(center.x + r * t.cos(), center.y + r * t.sin()));
                }
            }
            ProfEdge::Circle { .. } => {}
        }
    }
    p
}

/// The part of a profile on one side of the surface: its area and the radial coordinate of its centroid.
/// `below` selects what goes into the material of the shaft, y ≤ 0, and its opposite what stands out of it, as
/// the flight of an auger does.
fn part_area_centroid(pts: &[Point2], below: bool) -> (f64, f64) {
    let keep = |y: f64| if below { y <= 0.0 } else { y >= 0.0 };
    let mut out: Vec<Point2> = Vec::new();
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        if keep(a.y) {
            out.push(a);
        }
        if keep(a.y) != keep(b.y) {
            let t = a.y / (a.y - b.y);
            out.push(Point2::new(a.x + (b.x - a.x) * t, 0.0));
        }
    }
    let (mut area, mut cy) = (0.0, 0.0);
    for i in 0..out.len() {
        let (a, b) = (out[i], out[(i + 1) % out.len()]);
        let cr = a.x * b.y - b.x * a.y;
        area += cr;
        cy += (a.y + b.y) * cr;
    }
    area *= 0.5;
    if area.abs() > 1e-12 {
        cy /= 6.0 * area;
    }
    (area.abs(), cy)
}

fn mesh_volume(s: &Shape) -> f64 {
    s.tessellate(0.01).iter().map(|b| b.0.volume()).sum()
}

/// Build a thread on a shaft and return what was removed by the mesh and what the profile predicts.
fn cut_vs_profile(edges: &[ProfEdge], r0: f64, len: f64, lead: f64) -> (f64, f64, bool) {
    let rod = Shape::cylinder(r0, len + 10.0).expect("the shaft"); // the thread runs from the lower end, so the
                                                                   // overrun of the turn goes into the air
    let cut = rod
        .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(edges), len, lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("the thread built");
    let (area, cy) = part_area_centroid(&poly(edges), true);
    let want = area * (len / lead) * 2.0 * std::f64::consts::PI * (r0 + cy);
    (mesh_volume(&rod) - mesh_volume(&cut), want, cut.is_valid())
}

/// Control profiles whose answer is known exactly: a rectangle, the same with rounded corners, a semicircular
/// root, and a trapezoid. The former construction removed nothing at all on the rounded rectangle.
#[test]
fn control_profiles_cut_exactly_what_they_describe() {
    let p = |a: f64, b: f64| Point2::new(a, b);
    let (x, yt, yb, r) = (0.5, 0.25, -1.75, 0.25);
    let cases: Vec<(&str, Vec<ProfEdge>)> = vec![
        (
            "a rectangle",
            vec![
                ProfEdge::Line { a: p(x, yt), b: p(x, yb) },
                ProfEdge::Line { a: p(x, yb), b: p(-x, yb) },
                ProfEdge::Line { a: p(-x, yb), b: p(-x, yt) },
                ProfEdge::Line { a: p(-x, yt), b: p(x, yt) },
            ],
        ),
        (
            "a root rounded at the corners",
            vec![
                ProfEdge::Line { a: p(x, yt), b: p(x, yb + r) },
                ProfEdge::Arc { a: p(x, yb + r), b: p(x - r, yb), center: p(x - r, yb + r), ccw: false },
                ProfEdge::Line { a: p(x - r, yb), b: p(-(x - r), yb) },
                ProfEdge::Arc { a: p(-(x - r), yb), b: p(-x, yb + r), center: p(-(x - r), yb + r), ccw: false },
                ProfEdge::Line { a: p(-x, yb + r), b: p(-x, yt) },
                ProfEdge::Line { a: p(-x, yt), b: p(x, yt) },
            ],
        ),
        (
            "a semicircular root",
            vec![
                ProfEdge::Line { a: p(x, yt), b: p(x, yb) },
                ProfEdge::Arc { a: p(x, yb), b: p(-x, yb), center: p(0.0, yb), ccw: false },
                ProfEdge::Line { a: p(-x, yb), b: p(-x, yt) },
                ProfEdge::Line { a: p(-x, yt), b: p(x, yt) },
            ],
        ),
        (
            "a trapezoid",
            vec![
                ProfEdge::Line { a: p(1.531, 0.525), b: p(1.531, 0.0) },
                ProfEdge::Line { a: p(1.531, 0.0), b: p(0.438, -2.147) },
                ProfEdge::Line { a: p(0.438, -2.147), b: p(-0.438, -2.147) },
                ProfEdge::Line { a: p(-0.438, -2.147), b: p(-1.531, 0.0) },
                ProfEdge::Line { a: p(-1.531, 0.0), b: p(-1.531, 0.525) },
                ProfEdge::Line { a: p(-1.531, 0.525), b: p(1.531, 0.525) },
            ],
        ),
    ];
    for (name, edges) in cases {
        let (got, want, valid) = cut_vs_profile(&edges, 15.0, 25.0, 5.0);
        let err = (got - want) / want * 100.0;
        eprintln!("{name:26}: removed {got:8.2} mm³ against a predicted {want:8.2} -> {err:+5.2}%");
        assert!(valid, "{name}: the body is valid");
        assert!(err.abs() < 2.0, "{name}: removed {got:.2} instead of {want:.2} ({err:+.2}%), so the built shape does not equal the profile");
    }
}

/// Every standard at several pitches: the groove has to match its own profile.
#[test]
fn every_standard_cuts_exactly_its_profile() {
    for std in [ThreadStandard::MetricIso, ThreadStandard::TrapezoidalTr, ThreadStandard::Acme, ThreadStandard::RoundRd, ThreadStandard::Buttress] {
        for (d, pitch, fit) in [(30.0, 3.5, 0.0), (30.0, 5.0, 0.2), (12.0, 1.75, 0.15)] {
            let g = ThreadSpec { standard: std, nominal_d: d, pitch, fit, ..Default::default() }.geometry();
            let (got, want, valid) = cut_vs_profile(&g.groove, g.stock_d * 0.5, 20.0, g.lead);
            let err = (got - want) / want * 100.0;
            eprintln!("{std:?} Ø{d}×{pitch}, fit {fit}: removed {got:8.2} against a predicted {want:8.2} -> {err:+5.2}%");
            assert!(valid, "{std:?} Ø{d}×{pitch}: the body is valid");
            assert!(err.abs() < 2.0, "{std:?} Ø{d}×{pitch}: removed {got:.2} instead of {want:.2} ({err:+.2}%)");
        }
    }
}

/// An internal thread: the groove goes into the wall, and what it removes has to match the profile just the
/// same.
#[test]
fn internal_thread_cuts_exactly_its_profile() {
    let g = ThreadSpec { standard: ThreadStandard::Acme, nominal_d: 30.0, pitch: 5.0, internal: true, fit: 0.2, ..Default::default() }.geometry();
    let (r0, len) = (g.stock_d * 0.5, 30.0);
    let tube = Shape::cylinder(r0 + 15.0, len + 10.0).unwrap().boolean(&Shape::cylinder(r0, len + 20.0).unwrap(), 0).expect("the bushing");
    let cut = tube
        .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&g.groove), len, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("the internal thread built");
    // in a hole the material lies outside, so the cutting part of the profile is the one running away from
    // the axis, y ≥ 0
    let (area, cy) = part_area_centroid(&poly(&g.groove), false);
    let want = area * (len / g.lead) * 2.0 * std::f64::consts::PI * (r0 + cy);
    let got = mesh_volume(&tube) - mesh_volume(&cut);
    let err = (got - want) / want * 100.0;
    eprintln!("internal ACME Ø30×5: removed {got:.2} against a predicted {want:.2} -> {err:+.2}%");
    assert!(cut.is_valid(), "the body is valid");
    assert!(err.abs() < 2.0, "the internal thread removed {got:.2} instead of {want:.2} ({err:+.2}%)");
}

/// An auger: the flight is fused on, and the volume added has to match the profile of the flight as well.
#[test]
fn auger_flight_adds_exactly_its_profile() {
    let a = AugerSpec { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, edge_r: 0.8, ..Default::default() };
    let (r0, len) = (a.shaft_d * 0.5, 60.0);
    let shaft = Shape::cylinder(r0, len + 10.0).unwrap();
    let prof = a.flight_profile();
    let auger = shaft
        .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&prof), len, a.lead(), a.starts, if a.left { qymcad_kernel::Hand::Left } else { qymcad_kernel::Hand::Right }, qymcad_kernel::Helix::Rib, 0.0, 0.0, &[], &[], 0.0)
        .expect("the auger built");
    let (area, cy) = part_area_centroid(&poly(&prof), false); // the flight grows outward from the shaft
    // The length given is the length of the flight itself, which starts exactly at one end face and finishes at
    // the other, so the sweep is shorter by the thickness of the flight; otherwise half that thickness would
    // stand out past the end of the shaft.
    let pts = poly(&prof);
    let span = pts.iter().fold(f64::MIN, |m, p| m.max(p.x)) - pts.iter().fold(f64::MAX, |m, p| m.min(p.x));
    let want = area * ((len - span) / a.lead()) * 2.0 * std::f64::consts::PI * (r0 + cy);
    let got = mesh_volume(&auger) - mesh_volume(&shaft);
    let err = (got - want) / want * 100.0;
    eprintln!("auger Ø10 to Ø30 at a pitch of 20: added {got:.2} against a predicted {want:.2} -> {err:+.2}%");
    assert!(auger.is_valid(), "the auger is a valid body");
    assert!(err.abs() < 3.0, "the auger added {got:.2} instead of {want:.2} ({err:+.2}%)");
}

/// The direction of the axis must change nothing: the rim is picked at the lower end as readily as at the
/// upper one. In the reported case a thread on Ø30×200 with the rim on top, its axis pointing down, removed
/// nothing at all, because the entry rings went into the same boolean as the groove and intersected it.
#[test]
fn thread_is_the_same_along_either_axis_direction() {
    let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 30.0, pitch: 5.0, fit: 0.2, ..Default::default() }.geometry();
    let rod = Shape::cylinder(15.0, 200.0).expect("the shaft");
    let base = mesh_volume(&rod);
    let cut = |o: [f64; 3], d: [f64; 3], li: f64| -> (f64, bool) {
        let s = rod
            .helical_profile(o, d, 15.0, &encode_edges(&g.groove), 100.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, li, li, &[], &[], 0.0)
            .expect("the thread built");
        (base - mesh_volume(&s), s.is_valid())
    };
    for li in [0.0, 1.5] {
        let (up, ok_up) = cut([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], li);
        let (down, ok_dn) = cut([0.0, 0.0, 200.0], [0.0, 0.0, -1.0], li);
        eprintln!("entry {li}: upward removed {up:.1}, downward {down:.1}");
        assert!(ok_up && ok_dn, "both bodies are valid, entry {li}");
        assert!(up > 1000.0 && down > 1000.0, "the thread has to cut in both directions, entry {li}: {up:.1} and {down:.1}");
        assert!((up - down).abs() < 0.01 * up, "the direction of the axis changes the result: {up:.1} against {down:.1}");
    }
}

/// An entry and a run-out are different things and both have to work. An entry at the open end countersinks the
/// mouth with a cone, which is what a nut is started onto. A run-out at the blind end cuts a relief groove for
/// the thread to leave into: the turn runs into it and is cut off, so no shoulder is left on the crest and the
/// mating part meets on its face when tightened. Both remove more than a bare thread does; the shape of each is
/// held by separate tests.
#[test]
fn lead_in_chamfers_the_entry_and_run_out_cuts_a_relief() {
    let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 30.0, pitch: 3.5, ..Default::default() }.geometry();
    let rod = Shape::cylinder(g.stock_d * 0.5, 50.0).expect("the shaft");
    let base = mesh_volume(&rod);
    let mk = |li: f64, lo: f64| {
        let s = rod
            .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], g.stock_d * 0.5, &encode_edges(&g.groove), 40.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, li, lo, &[], &[], 0.0)
            .expect("it built");
        (base - mesh_volume(&s), s.is_valid())
    };
    let (plain, ok0) = mk(0.0, 0.0);
    let (entry, ok1) = mk(5.0, 0.0);
    let (fade, ok2) = mk(0.0, 5.0);
    eprintln!("M30×3.5: bare {plain:.1}, with an entry {entry:.1}, with a run-out groove {fade:.1}");
    assert!(ok0 && ok1 && ok2, "all three bodies are valid");
    // The threshold is deliberately small: an entry both cuts the crests with a cone, adding to the volume
    // removed, and damps the turn over its own length, subtracting from it, so what the volume shows is the
    // difference of two effects rather than the full cut.
    assert!(entry > plain * 1.02, "the entry does not cut the crests at the end: {entry:.1} against {plain:.1}");
    assert!(fade > plain * 1.05, "the run-out did not cut the relief groove at the blind end: {fade:.1} against {plain:.1}");
}

/// The first turn is like every other one.
///
/// It used to come out wider than the rest and end in a vertical wall, which made a pair impossible to screw
/// together. The cause was that the groove began exactly at the chosen rim, so its end cap — a flat wall in the
/// axial plane — fell inside the part. The turn is now built a full revolution before the end face and the cap
/// goes into the air.
///
/// The check is direct: how much the first turn removes against a settled one somewhere in the middle.
#[test]
fn first_turn_is_like_the_others() {
    for (std, d, pitch) in [(ThreadStandard::MetricIso, 30.0, 3.5), (ThreadStandard::RoundRd, 30.0, 5.0), (ThreadStandard::TrapezoidalTr, 30.0, 5.0)] {
        let g = ThreadSpec { standard: std, nominal_d: d, pitch, fit: 0.2, ..Default::default() }.geometry();
        let r0 = g.stock_d * 0.5;
        let rod = Shape::cylinder(r0, 60.0).expect("the shaft");
        let base = mesh_volume(&rod);
        let cut_turns = |n: f64| -> f64 {
            let s = rod
                .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&g.groove), g.lead * n, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
                .expect("the thread built");
            base - mesh_volume(&s)
        };
        let (one, two, three) = (cut_turns(1.0), cut_turns(2.0), cut_turns(3.0));
        let steady = three - two; // a settled turn
        eprintln!("{std:?} Ø{d}×{pitch}: first turn {one:.1}, settled turn {steady:.1}");
        assert!(
            (one - steady).abs() < 0.06 * steady,
            "{std:?}: the first turn removes {one:.1} against {steady:.1} for the rest, so its start is unlike the others"
        );
        assert!((two - one - steady).abs() < 0.06 * steady, "{std:?}: the second turn has to be settled as well");
    }
}

/// A relief groove for the thread to leave into at the blind end, as the standard prescribes.
///
/// This was confirmed on a printed pair: if the thread simply stops, tightening it home tears out the
/// incomplete turns of the mating part. In metal a full circular groove is cut there to the depth of the
/// profile — the thread ends inside it, the end of the turn is complete, and the parts meet face to face. The
/// width of the groove is the run-out that was asked for.
#[test]
fn blind_end_gets_a_relief_groove() {
    let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 30.0, pitch: 3.5, ..Default::default() }.geometry();
    let (r0, len, lo) = (g.stock_d * 0.5, 20.0, 7.0);
    let rod = Shape::cylinder(r0, 40.0).expect("the shaft");
    let s = rod
        .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&g.groove), len, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, lo, &[], &[], 0.0)
        .expect("the thread built");
    // the smallest radius of material over slices: in the run-out zone the root has to rise towards the
    // surface
    let verts: Vec<_> = s.tessellate(0.02).into_iter().flat_map(|b| b.0.verts).collect();
    let depth_at = |z: f64| -> f64 {
        let inner = verts.iter().filter(|v| (v.z - z).abs() < 0.35).fold(r0, |m, v| m.min(v.x.hypot(v.y)));
        r0 - inner
    };
    // in the groove the material is removed all the way round to the root, so the largest and smallest radii
    // are equal
    let ring_at = |z: f64| -> (f64, f64) {
        let sel: Vec<f64> = verts.iter().filter(|v| (v.z - z).abs() < 0.35).map(|v| v.x.hypot(v.y)).collect();
        (sel.iter().cloned().fold(f64::MAX, f64::min), sel.iter().cloned().fold(0.0_f64, f64::max))
    };
    let before = depth_at(len - lo - 1.0);
    let (lo_r, hi_r) = ring_at(len - lo * 0.5);
    eprintln!("before the groove the depth is {before:.3}, the full one being {:.3}; in the groove the radius runs {lo_r:.3}..{hi_r:.3} against a blank of {r0:.1}", g.depth);
    assert!(before > 0.8 * g.depth, "before the groove the thread is at full depth: {before:.3} of {:.3}", g.depth);
    assert!(hi_r - lo_r < 0.15, "the groove has to be an even ring: the radius runs {lo_r:.3}..{hi_r:.3}");
    assert!(r0 - hi_r > 0.8 * g.depth, "the groove has to reach the depth of the profile: {:.3} removed of {:.3}", r0 - hi_r, g.depth);
    // The mesh has to stay whole as well. A groove exactly at the root of the thread gave coincident surfaces:
    // the kernel called the body valid while the tessellator tore it apart — the mesh volume came out larger
    // than the blank, which on screen means holes. The groove therefore goes a little deeper than the root.
    let blank = mesh_volume(&rod);
    let got = mesh_volume(&s);
    eprintln!("volume by mesh: blank {blank:.1} -> with the groove {got:.1}");
    assert!(got < blank, "the mesh of the part with the groove is broken: {got:.1} against a blank of {blank:.1}");
    assert!(got > 0.6 * blank, "the groove ate away too much: {got:.1} of {blank:.1}");
}

/// A pair screws together: the decisive check for printing.
///
/// A bolt and a nut of the same designation are placed coaxially and the intersection of the bodies is
/// measured; if it is appreciable the parts jam and cannot be screwed together. In the reported case two such
/// parts would never enter one another: the clearance existed only along the axis, while radially the crest of
/// the bolt ran into the root of the nut.
#[test]
fn bolt_and_nut_actually_screw_together() {
    let (d, pitch, len) = (20.0, 5.0, 10.0); // two turns: booleans on helical bodies are expensive
    let mk = |internal: bool, fit: f64| {
        let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: d, pitch, internal, fit, ..Default::default() }.geometry();
        let r0 = g.stock_d * 0.5;
        let blank = if internal {
            Shape::cylinder(d * 0.7, len + 10.0).unwrap().boolean(&Shape::cylinder(r0, len + 30.0).unwrap(), 0).expect("the bushing")
        } else {
            Shape::cylinder(r0, len + 10.0).expect("the shaft")
        };
        blank
            .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&g.groove), len, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
            .expect("the thread built")
    };
    let bolt = mk(false, 0.4);
    let nut = mk(true, 0.2);
    let bolt_v = mesh_volume(&bolt);
    // Screwing a nut on turns it and moves it along the axis at the same time, that is, it changes the phase.
    // "They screw together" therefore means that some shift exists at which the parts do not touch each other.
    let mut best = f64::MAX;
    let mut best_at = 0.0;
    for k in 0..4 {
        let dz = pitch * k as f64 / 4.0;
        let m = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, dz];
        let moved = nut.transformed(&m).expect("the shifted nut");
        let v = mesh_volume(&bolt.boolean(&moved, 2).expect("the intersection was computed"));
        eprintln!("shift {dz:.3} mm -> intersection {v:.2} mm³");
        if v < best {
            best = v;
            best_at = dz;
        }
    }
    eprintln!("an Rd Ø20×5 pair at fits of 0.4 and 0.2: best intersection {best:.2} mm³ at a shift of {best_at:.3}, the bolt being {bolt_v:.1}");
    assert!(best < 0.005 * bolt_v, "the parts jam at every phase: the smallest intersection is {best:.2} mm³, so they cannot be screwed together");
}

/// A through nut: a run-out at both ends, so it can be started from either side. The lower one used not to
/// build at all. A mouth with a run-out asked for is countersunk with a cone down to the root diameter; where
/// none is asked for, the thread reaches the face as a complete turn, with no wall.
#[test]
fn through_nut_is_countersunk_on_both_ends() {
    let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 20.0, pitch: 5.0, internal: true, fit: 0.2, ..Default::default() }.geometry();
    let (r0, h) = (g.stock_d * 0.5, 20.0);
    let tube = Shape::cylinder(20.0, h).unwrap().boolean(&Shape::cylinder(r0, h * 2.0).unwrap(), 0).expect("the nut");
    // THE MOUTH IS MEASURED AT THE FACE ITSELF, in a thin band.
    //
    // A countersink is a cone: it narrows as it goes in, so a wide sampling band does not measure the mouth
    // but the cone a little way inside it. At 2 mm of countersink the cone loses 1.25 mm of radius per
    // millimetre, so a band of 0.4 already under-reads by half a millimetre — and what a person cares about
    // is whether a nut can be STARTED, which happens at the face.
    let bore_at = |s: &Shape, z: f64| -> f64 {
        let verts: Vec<_> = s.tessellate(0.02).into_iter().flat_map(|b| b.0.verts).collect();
        verts.iter().filter(|v| (v.z - z).abs() < 0.06).fold(f64::MAX, |m, v| m.min(v.x.hypot(v.y))) * 2.0
    };
    let mk = |li: f64, lo: f64| {
        tube.helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], r0, &encode_edges(&g.groove), h, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, li, lo, &[], &[], 0.0)
            .expect("the thread built")
    };
    let plain = mk(0.0, 0.0);
    let both = mk(2.0, 2.0);
    let (p0, p1) = (bore_at(&plain, 0.0), bore_at(&plain, h));
    let (b0, bm, b1) = (bore_at(&both, 0.0), bore_at(&both, h * 0.5), bore_at(&both, h));
    eprintln!("without run-outs the mouths are Ø{p0:.2} and Ø{p1:.2}; with run-outs of 2 and 2: Ø{b0:.2} ... Ø{bm:.2} ... Ø{b1:.2}");
    assert!(plain.is_valid() && both.is_valid(), "both bodies are valid");
    let bore = g.stock_d;
    assert!((p0 - bore).abs() < 0.2 && (p1 - bore).abs() < 0.2, "without run-outs the thread reaches both faces as a complete turn: {p0:.2} and {p1:.2} against a bore of {bore:.2}");
    let want = g.stock_d + 2.0 * g.depth;
    assert!(b0 > want - 0.3, "the upper mouth is not countersunk: Ø{b0:.2}, expecting about Ø{want:.2}");
    assert!(b1 > want - 0.3, "the lower mouth is not countersunk: Ø{b1:.2}, expecting about Ø{want:.2}, so the nut cannot be started from that side");
    assert!((bm - bore).abs() < 0.2, "in the middle of the nut the thread is full: Ø{bm:.2} against a bore of {bore:.2}");
}

/// An auger, as reported on a Ø20 shaft with a Ø25 flight, a pitch of 10, a thickness of 5 and a length of 190
/// on a shaft of 200: the flight faded out neither at the start nor at the end, coming out as blunt stumps, and
/// at the start it stood out past the end face. The flight is symmetric about its starting point, so half its
/// thickness reached past the face of the shaft — measured at +2.56 mm for a thickness of 5 — and no fade was
/// applied at all.
#[test]
fn auger_flight_is_flush_with_the_face_and_fades_out() {
    let a = AugerSpec { shaft_d: 20.0, outer_d: 25.0, pitch: 10.0, thickness: 5.0, edge_r: 2.0, ..Default::default() };
    let (r0, h, len) = (a.shaft_d * 0.5, 200.0, 190.0);
    let shaft = Shape::cylinder(r0, h).expect("the shaft");
    let mk = |li: f64, lo: f64| {
        shaft
            .helical_profile([0.0, 0.0, h], [0.0, 0.0, -1.0], r0, &encode_edges(&a.flight_profile()), len, a.lead(), 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Rib, li, lo, &[], &[], 0.0)
            .expect("the auger built")
    };
    for (name, li, lo) in [("no fade", 0.0, 0.0), ("a fade of 10 and 10", 10.0, 10.0)] {
        let s = mk(li, lo);
        let bb = s.bbox().expect("the bounding box");
        eprintln!("auger, {name}: z extent {:.2}..{:.2}, valid={}", bb[2], bb[5], s.is_valid());
        assert!(s.is_valid(), "{name}: the body is valid");
        assert!(bb[5] <= h + 0.05 && bb[2] >= -0.05, "{name}: the flight stands out past the ends of the shaft, its extent being {:.2}..{:.2} against a shaft of 0..{h}", bb[2], bb[5]);
    }
    // the fade: the height of the flight melts away at the ends and stays full in the middle
    let s = mk(10.0, 10.0);
    let verts: Vec<_> = s.tessellate(0.1).into_iter().flat_map(|b| b.0.verts).collect();
    let h_at = |z: f64| verts.iter().filter(|v| (v.z - z).abs() < 1.0).fold(r0, |m, v| m.max(v.x.hypot(v.y))) - r0;
    let (start, mid, end) = (h_at(h - 1.0), h_at(h * 0.5), h_at(h - len + 1.0));
    eprintln!("flight height: {start:.2} at the start, {mid:.2} in the middle, {end:.2} at the end, the full one being {:.2}", a.flight_height());
    assert!((mid - a.flight_height()).abs() < 0.2, "in the middle the flight is at full height: {mid:.2} against {:.2}", a.flight_height());
    assert!(start < 0.3 * mid, "the flight does not fade at the start: {start:.2} against {mid:.2}, a stump");
    assert!(end < 0.3 * mid, "the flight does not fade at the end: {end:.2} against {mid:.2}, a stump");
}
