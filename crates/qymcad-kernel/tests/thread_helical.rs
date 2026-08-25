//! A helical rib or groove built from an exact profile on the real kernel.
//!
//! The profile is computed by `qymcad_core::thread` from the standard, and the kernel only sweeps it along the
//! helix: a thread subtracts the groove, an auger welds on the flight. The checks are by volume and B-rep
//! validity rather than by eye — the earlier implementation built the profile inside the kernel as a polygon
//! from invented coefficients.
use qymcad_kernel::Shape;
use qymcad_core::thread::{encode_edges, AugerSpec, ThreadSpec, ThreadStandard};

const PI: f64 = std::f64::consts::PI;

/// A cylinder of diameter `d` and height `h` along Z from the origin.
fn rod(d: f64, h: f64) -> Shape {
    Shape::cylinder(d * 0.5, h).expect("the cylinder")
}

/// An external metric thread M10×1.5 on a Ø10 rod: the groove has to remove material without destroying the
/// rod — the volume lands between the core at the minor diameter and the full cylinder at the major one.
#[test]
fn external_metric_thread_removes_sane_volume() {
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 10.0, pitch: 1.5, ..Default::default() };
    let g = spec.geometry();
    let (len, blank) = (20.0, rod(g.stock_d, 25.0));
    let v0 = blank.volume();
    let cut = blank
        .helical_profile([0.0, 0.0, 2.0], [0.0, 0.0, 1.0], g.stock_d * 0.5, &encode_edges(&g.groove), len, g.lead, spec.starts, if spec.left { qymcad_kernel::Hand::Left } else { qymcad_kernel::Hand::Right }, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("the thread built");
    let v1 = cut.volume();
    assert!(cut.is_valid(), "the threaded body is valid");
    let core = PI * (g.minor_d * 0.5).powi(2) * 25.0; // as if everything were cut away down to the root
    assert!(v1 < v0, "the thread removed material: {v0:.1} -> {v1:.1}");
    assert!(v1 > core, "but it did not eat the rod down to the core ({core:.1}): {v1:.1}");
    // about half the volume of the ring between the major and minor diameters over the threaded length is
    // removed, the profile taking roughly half the pitch
    let ring = PI * ((g.major_d * 0.5).powi(2) - (g.minor_d * 0.5).powi(2)) * len;
    let removed = v0 - v1;
    assert!(removed > 0.2 * ring && removed < 0.9 * ring, "{removed:.2} mm³ removed against a ring of {ring:.2} mm³");
}

/// The trapezoidal Tr20×4, the profile wanted for 3D printing and for feed screws.
///
/// Its groove is wider and shallower than the metric one, so the volume removed is compared against the metric
/// thread on the same rod. The volume is taken from the mesh: the kernel's integrator is out by a factor on
/// helical surfaces, as `thread_profile_fidelity.rs` shows, and against it this test measured the wrong thing.
#[test]
fn trapezoidal_thread_builds_and_differs_from_metric() {
    let mesh_v = |s: &Shape| -> f64 { s.tessellate(0.02).iter().map(|b| b.0.volume()).sum() };
    let blank_v = mesh_v(&rod(20.0, 30.0));
    let cut_of = |std: ThreadStandard| {
        let g = ThreadSpec { standard: std, nominal_d: 20.0, pitch: 4.0, ..Default::default() }.geometry();
        rod(20.0, 30.0)
            .helical_profile([0.0, 0.0, 3.0], [0.0, 0.0, 1.0], 10.0, &encode_edges(&g.groove), 24.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
            .map(|s| (mesh_v(&s), s.is_valid()))
    };
    let (vm, okm) = cut_of(ThreadStandard::MetricIso).expect("the metric thread built");
    let (vt, okt) = cut_of(ThreadStandard::TrapezoidalTr).expect("the trapezoidal thread built");
    assert!(okm && okt, "both bodies are valid");
    assert!(vm < blank_v && vt < blank_v, "both threads removed material");
    assert!((vm - vt).abs() > 1e-3, "different profiles give different volumes: metric {vm:.2}, Tr {vt:.2}");
}

/// The round Rd thread of DIN 405, whose root and crest are rounded with arcs: the body has to build and stay
/// valid, or the rounded profile exists only on paper.
#[test]
fn round_rd_thread_with_arcs_builds_valid_solid() {
    let g = ThreadSpec { standard: ThreadStandard::RoundRd, nominal_d: 16.0, pitch: 3.0, ..Default::default() }.geometry();
    assert!(g.groove.iter().any(|e| matches!(e, qymcad_core::geom::ProfEdge::Arc { .. })), "the Rd profile contains arcs");
    let s = rod(16.0, 24.0)
        .helical_profile([0.0, 0.0, 2.0], [0.0, 0.0, 1.0], 8.0, &encode_edges(&g.groove), 20.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("the round thread built");
    assert!(s.is_valid(), "the body is valid");
    assert!(s.volume() > 0.0 && s.volume() < rod(16.0, 24.0).volume());
}

/// Multiple starts and a left hand: two starts remove markedly more material than one, and a left-hand thread
/// builds just the same.
#[test]
fn multi_start_and_left_hand_build() {
    let g = ThreadSpec { standard: ThreadStandard::TrapezoidalTr, nominal_d: 20.0, pitch: 4.0, starts: 2, ..Default::default() }.geometry();
    // the comparison is fair only at the same lead: two starts lay twice as many grooves
    let blank = rod(20.0, 30.0).volume();
    let one = rod(20.0, 30.0)
        .helical_profile([0.0, 0.0, 3.0], [0.0, 0.0, 1.0], 10.0, &encode_edges(&g.groove), 24.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("a single start at the same lead")
        .volume();
    let two = rod(20.0, 30.0)
        .helical_profile([0.0, 0.0, 3.0], [0.0, 0.0, 1.0], 10.0, &encode_edges(&g.groove), 24.0, g.lead, 2, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("a two-start thread")
        .volume();
    let (r1, r2) = (blank - one, blank - two);
    assert!(r2 > 1.5 * r1, "two starts at the same lead remove twice as much: {r1:.1} -> {r2:.1} mm³");
    let left = rod(20.0, 30.0)
        .helical_profile([0.0, 0.0, 3.0], [0.0, 0.0, 1.0], 10.0, &encode_edges(&g.groove), 24.0, g.lead, 2, qymcad_kernel::Hand::Left, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
        .expect("the left-hand thread");
    assert!(left.is_valid(), "the left-hand thread is valid");
    assert!((left.volume() - two).abs() / two < 0.05, "a left-hand thread removes as much as a right-hand one");
}

/// An auger: the flight is fused onto the shaft rather than cut from it, which was not possible at all before.
/// The volume has to grow by roughly the volume of the helical ribbon.
#[test]
fn auger_flight_adds_material_to_shaft() {
    let a = AugerSpec { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, edge_r: 0.8, ..Default::default() };
    let (len, h) = (60.0, a.flight_height());
    let shaft = rod(a.shaft_d, 70.0);
    let v0 = shaft.volume();
    let auger = shaft
        .helical_profile([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], a.shaft_d * 0.5, &encode_edges(&a.flight_profile()), len, a.lead(), a.starts, if a.left { qymcad_kernel::Hand::Left } else { qymcad_kernel::Hand::Right }, qymcad_kernel::Helix::Rib, 0.0, 0.0, &[], &[], 0.0)
        .expect("the auger built");
    let v1 = auger.volume();
    assert!(auger.is_valid(), "the auger is a valid body");
    assert!(v1 > v0, "the flight added material: {v0:.1} -> {v1:.1}");
    // an estimate of the ribbon volume: the length of a turn times the section, thickness by height
    let turns = len / a.lead();
    let mid_r = (a.shaft_d * 0.5 + a.outer_d * 0.5) * 0.5;
    let ribbon = turns * (2.0 * PI * mid_r).hypot(a.lead()) * a.thickness * h;
    let added = v1 - v0;
    assert!(added > 0.5 * ribbon && added < 1.5 * ribbon, "{added:.1} mm³ added against an estimated ribbon of {ribbon:.1} mm³");
}

/// The entry is a countersink at the open end: it cuts the crests of the first turns so a nut can be started.
///
/// The run-out at the blind end is a relief groove for the thread to exit into — a full ring to the depth of the
/// profile, so that when tightened home the parts meet on their faces rather than on incomplete turns; this was
/// confirmed in print. Both remove more than a bare thread does, and the shape of each is held by the tests in
/// `thread_profile_fidelity`. The volume is taken from the mesh, the kernel's integrator being wrong on helical
/// surfaces.
#[test]
fn lead_in_chamfers_the_first_turns() {
    let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 12.0, pitch: 1.75, ..Default::default() }.geometry();
    let mesh_v = |s: &Shape| -> f64 { s.tessellate(0.01).iter().map(|b| b.0.volume()).sum() };
    let blank = mesh_v(&rod(12.0, 30.0));
    // The thread runs from the end face of the shaft, the only place an entry means anything, being a
    // countersink of the mouth. The far end, at z = 24 on a shaft of height 30, runs into the body, so the
    // fading of depth applies there.
    let mk = |li: f64, lo: f64| {
        rod(12.0, 30.0)
            .helical_profile([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 6.0, &encode_edges(&g.groove), 24.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, li, lo, &[], &[], 0.0)
            .map(|s| (blank - mesh_v(&s), s.is_valid()))
    };
    let (cut_plain, ok0) = mk(0.0, 0.0).expect("the bare thread");
    let (cut_entry, ok1) = mk(3.0, 0.0).expect("with an entry");
    let (cut_fade, ok2) = mk(0.0, 3.0).expect("with a run-out");
    eprintln!("M12×1.75: bare {cut_plain:.1}, with an entry {cut_entry:.1}, with a run-out {cut_fade:.1}");
    assert!(ok0 && ok1 && ok2, "all the bodies are valid");
    assert!(cut_entry > cut_plain * 1.02, "the entry does not cut the crests of the turns at the end: {cut_entry:.2} against {cut_plain:.2}");
    assert!(cut_fade > cut_plain * 1.05, "the run-out did not cut the relief groove at the blind end: {cut_fade:.2} against {cut_plain:.2}");
}

/// Degenerate parameters give an honest `None` rather than rubbish or a crash.
#[test]
fn degenerate_helical_inputs_return_none() {
    let g = ThreadSpec::default().geometry();
    let prof = encode_edges(&g.groove);
    let s = rod(10.0, 20.0);
    assert!(s.helical_profile([0.0; 3], [0.0, 0.0, 1.0], 0.0, &prof, 10.0, 1.5, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none(), "a zero radius");
    assert!(s.helical_profile([0.0; 3], [0.0, 0.0, 1.0], 5.0, &prof, 0.0, 1.5, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none(), "a zero length");
    assert!(s.helical_profile([0.0; 3], [0.0, 0.0, 1.0], 5.0, &prof, 10.0, 0.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none(), "a zero lead");
    assert!(s.helical_profile([0.0; 3], [0.0, 0.0, 1.0], 5.0, &[], 10.0, 1.5, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0).is_none(), "an empty profile");
}

/// A long fine thread of a hundred turns, the extreme case the sweep used to break on: with a spine of a single
/// edge the kernel refused from about 60 turns — M6×1 over 70 mm was refused in 0.17 s — and the tool silently
/// produced nothing. A spine of several edges builds even 200 turns.
#[test]
fn very_long_fine_thread_builds() {
    for (d, p, len) in [(6.0, 1.0, 70.0), (10.0, 1.5, 150.0), (6.0, 1.0, 200.0)] {
        let g = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch: p, ..Default::default() }.geometry();
        let blank = rod(g.stock_d, len + 10.0);
        let v0 = blank.volume();
        let t = std::time::Instant::now();
        let s = blank
            .helical_profile([0.0, 0.0, 2.0], [0.0, 0.0, 1.0], g.stock_d * 0.5, &encode_edges(&g.groove), len, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 2.0, 2.0, &[], &[], 0.0)
            .unwrap_or_else(|| panic!("M{d}×{p} over {len} mm, {:.0} turns, did not build", len / g.lead));
        eprintln!("M{d}×{p} over {len} mm, {:.0} turns: {:?}", len / g.lead, t.elapsed());
        assert!(s.is_valid(), "M{d}×{p} over {len} mm: the body is valid");
        assert!(s.volume() < v0, "M{d}×{p} over {len} mm: material was removed");
    }
}

/// A geometry budget: a thread has to stay a body of reasonable size.
///
/// This catches the class of defect that made the tool come out misshapen: walking the crest arc the wrong way,
/// 312° instead of 48°, swept nearly a full circle and inflated one face to 113 thousand triangles — 437
/// thousand for a thread instead of five. There is nothing here for a heavy mesh to come from: three dozen turns
/// and a profile of ten edges.
#[test]
fn thread_mesh_stays_within_a_sane_budget() {
    for std in [ThreadStandard::MetricIso, ThreadStandard::TrapezoidalTr, ThreadStandard::Acme, ThreadStandard::RoundRd] {
        let g = ThreadSpec { standard: std, nominal_d: 30.0, pitch: 3.5, ..Default::default() }.geometry();
        let blank = rod(g.stock_d, 40.0);
        let cut = blank
            .helical_profile([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], g.stock_d * 0.5, &encode_edges(&g.groove), 30.0, g.lead, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Helix::Groove, 0.0, 0.0, &[], &[], 0.0)
            .unwrap_or_else(|| panic!("{std:?} did not build"));
        assert!(cut.is_valid(), "{std:?}: the body is valid");
        let tris: usize = cut.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k()).iter().map(|b| b.0.tris.len()).sum();
        eprintln!("{std:?} Ø30×3.5 over 30 mm: {tris} triangles, V={:.1}", cut.volume());
        assert!(tris > 500, "{std:?}: the mesh is suspiciously empty, {tris} triangles");
        assert!(tris < 60_000, "{std:?}: the mesh is inflated — {tris} triangles for one thread, budget 60 thousand");
    }
}
