//! WHICH SIDE A THREAD GROOVE GOES. Reported case: an "internal" thread was placed on a SOLID d30
//! shaft. The groove went outwards, away from the shaft, removed 2 cm^3 instead of 12, and left flat
//! discs on screen — while the rebuild reported SUCCESS. The matrix: side of the face (shaft or bore)
//! against the flag in the parameters. Failures accumulate so the whole picture is visible at once.
use qymcad_core::model::Project;
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

/// A d`d` shaft of height `h` from a cylinder primitive, ready for a thread. Returns (project, body).
fn shaft(d: f64, h: f64) -> (Project, u64) {
    let mut p = Project::default();
    p.new_document();
    let body = p.add_cylinder(d * 0.5, h);
    (p, body)
}

/// A sleeve: a d`od` shaft with an axial d`id` bore.
fn tube(od: f64, id: f64, h: f64) -> (Project, u64) {
    let (mut p, outer) = shaft(od, h);
    let hole = p.add_cylinder(id * 0.5, h + 10.0);
    let body = p.add_body_boolean(outer, hole, 0);
    (p, body)
}

/// A round edge of radius about `r` on a body — the same thing a person picks with a click.
fn rim(p: &mut Project, body: u64, r: f64) -> u32 {
    let (_report, _shapes) = qymcad_testkit::regenerate(p);
    let e = p.regen_edges.get(&body).cloned().unwrap_or_default();
    e.iter()
        .filter(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05)
        .map(|e| e.id)
        .next()
        .unwrap_or_else(|| panic!("body {body} has no round edge of radius {r}: {:?}", e.iter().map(|e| e.radius).collect::<Vec<_>>()))
}

/// Build the part and return (volume of the STOCK, volume of the result, rebuild errors).
fn build(p: &mut Project, blank: u64, body: u64) -> (f64, f64, Vec<String>) {
    let before = p.mesh_index(blank).map(|i| p.bodies[i].mesh.volume()).unwrap_or(0.0);
    assert!(before > 0.0, "the stock {blank} must be built before the thread");
    let last = p.finish_base_body(body, 1);
    let (report, shapes) = qymcad_testkit::regenerate(p);
    let errs = report.errors.iter().map(|(_, e)| e.to_string()).collect();
    (before, shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0), errs)
}

fn acme(internal: bool) -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::Acme, nominal_d: 30.0, pitch: 5.0, internal, fit: 0.2, ..Default::default() }
}

/// FLAGGED "INTERNAL" ON A SHAFT — the thread must still cut INTO the shaft. The side of the groove
/// comes from the geometry rather than from a checkbox: a groove the other way cuts empty air (on the
/// real part 1.8 cm^3 was removed instead of 13, and flat discs were left instead of turns). No case
/// exists where a thread into thin air would be wanted, so the geometry outweighs the flag.
#[test]
fn wrong_side_flag_on_a_shaft_is_corrected_by_geometry() {
    let (mut p, body) = shaft(30.0, 200.0);
    let e = rim(&mut p, body, 15.0);
    let t = p.add_thread(body, e, acme(true), 100.0, 1.5, 1.5);
    let (v0, v1, errs) = build(&mut p, body, t);
    assert!(errs.is_empty(), "the thread builds without errors: {errs:?}");
    let g = acme(false).geometry();
    let want = std::f64::consts::PI * (15.0_f64.powi(2) - (15.0 - g.depth).powi(2)) * 100.0 * 0.5;
    let got = v0 - v1;
    eprintln!("shaft plus an \"internal\" flag: {got:.0} mm^3 removed against about {want:.0} expected");
    assert!(got > 0.5 * want, "the groove went into thin air: only {got:.0} mm^3 removed against about {want:.0} expected");
}

/// ON A SHAFT plus external is an ordinary thread: about half the ring between d30 and the root is removed.
#[test]
fn external_thread_on_a_shaft_cuts_the_expected_ring() {
    let (mut p, body) = shaft(30.0, 200.0);
    let e = rim(&mut p, body, 15.0);
    let t = p.add_thread(body, e, acme(false), 100.0, 1.5, 1.5);
    let (v0, v1, errs) = build(&mut p, body, t);
    assert!(errs.is_empty(), "an external thread on a shaft builds without errors: {errs:?}");
    let g = acme(false).geometry();
    let want = std::f64::consts::PI * (15.0_f64.powi(2) - (15.0 - g.depth).powi(2)) * 100.0 * 0.5;
    let got = v0 - v1;
    eprintln!("d30 shaft, external ACME P5 over 100 mm: {got:.0} mm^3 removed against about {want:.0} expected");
    assert!(got > 0.5 * want && got < 2.0 * want, "{got:.0} mm^3 removed against about {want:.0} expected");
}

/// IN A BORE plus internal is an ordinary thread — the part that was wanted in the first place.
#[test]
fn internal_thread_in_a_bore_cuts_the_expected_ring() {
    let (mut p, body) = tube(60.0, 30.0, 200.0);
    let e = rim(&mut p, body, 15.0);
    let t = p.add_thread(body, e, acme(true), 100.0, 1.5, 1.5);
    let (v0, v1, errs) = build(&mut p, body, t);
    assert!(errs.is_empty(), "an internal thread in a bore builds without errors: {errs:?}");
    let g = acme(true).geometry();
    let want = std::f64::consts::PI * ((15.0 + g.depth).powi(2) - 15.0_f64.powi(2)) * 100.0 * 0.5;
    let got = v0 - v1;
    eprintln!("d60/d30 sleeve, internal ACME P5 over 100 mm: {got:.0} mm^3 removed against about {want:.0} expected");
    assert!(got > 0.5 * want && got < 2.0 * want, "{got:.0} mm^3 removed against about {want:.0} expected");
}

/// FLAGGED "EXTERNAL" IN A BORE — the mirror case, corrected the same way.
#[test]
fn wrong_side_flag_in_a_bore_is_corrected_by_geometry() {
    let (mut p, body) = tube(60.0, 30.0, 200.0);
    let e = rim(&mut p, body, 15.0);
    let t = p.add_thread(body, e, acme(false), 100.0, 1.5, 1.5);
    let (v0, v1, errs) = build(&mut p, body, t);
    assert!(errs.is_empty(), "the thread builds without errors: {errs:?}");
    let g = acme(true).geometry();
    let want = std::f64::consts::PI * ((15.0 + g.depth).powi(2) - 15.0_f64.powi(2)) * 100.0 * 0.5;
    let got = v0 - v1;
    eprintln!("bore plus an \"external\" flag: {got:.0} mm^3 removed against about {want:.0} expected");
    assert!(got > 0.5 * want, "the groove went into thin air: only {got:.0} mm^3 removed against about {want:.0} expected");
}
