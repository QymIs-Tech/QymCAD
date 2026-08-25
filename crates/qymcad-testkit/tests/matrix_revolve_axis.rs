//! A8. The revolve tool used to accept only the sketch X or Y axis — no custom axis — so a circle
//! drawn in a sketch could not be turned into a sphere. MATRIX OF REVOLVES by axis: what the kernel
//! actually supports. Failures accumulate so the whole picture is visible at once.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

const PI: f64 = std::f64::consts::PI;

/// Half-disc of radius `r`: an arc plus the diameter ALONG the sketch X axis, centre at (cx, 0).
/// This is how a solid of revolution is built: the profile TOUCHES the axis without crossing it.
fn half_disc(p: &mut Project, si: usize, cx: f64, r: f64) {
    let n = 48;
    let mut pts = vec![Point2::new(cx - r, 0.0)];
    for k in 1..n {
        let a = PI * k as f64 / n as f64;
        pts.push(Point2::new(cx - r * a.cos(), r * a.sin()));
    }
    pts.push(Point2::new(cx + r, 0.0));
    for w in pts.windows(2) {
        p.add_line_entity(si, w[0].x, w[0].y, w[1].x, w[1].y, qymcad_core::feature::Purpose::Real);
    }
    p.add_line_entity(si, cx + r, 0.0, cx - r, 0.0, qymcad_core::feature::Purpose::Real); // diameter along the axis
}

fn build(p: &mut Project, body: u64) -> (f64, Vec<String>) {
    let last = p.finish_base_body(body, 1);
    let (report, shapes) = qymcad_testkit::regenerate(p);
    let errs = report.errors.iter().map(|(id, e)| format!("node {id}: {e}")).collect();
    (shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0), errs)
}

fn near(got: f64, exp: f64, tol: f64) -> bool {
    exp > 0.0 && ((got - exp) / exp).abs() < tol
}

/// SPHERE from a half-disc around the sketch X axis — the base scenario for any CAD.
#[test]
fn half_disc_around_x_makes_sphere() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("sphere");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "sphere");
    half_disc(&mut p, si, 0.0, 10.0);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("half-disc contour");
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, 0);
    let (v, errs) = build(&mut p, body);
    assert!(errs.is_empty(), "regen errors: {errs:?}");
    let exp = 4.0 / 3.0 * PI * 1000.0;
    assert!(near(v, exp, 0.02), "sphere r=10: V={v:.1}, expected {exp:.1}");
}

/// TORUS: a circle OFFSET from the axis, revolved around X.
#[test]
fn offset_circle_around_x_makes_torus() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("torus");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "torus");
    p.add_circle_entity(si, 0.0, 30.0, 5.0, qymcad_core::feature::Purpose::Real); // centre 30 above the X axis
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids[0];
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, 0);
    let (v, errs) = build(&mut p, body);
    assert!(errs.is_empty(), "regen errors: {errs:?}");
    let exp = 2.0 * PI * PI * 30.0 * 25.0; // 2*pi^2*R*r^2
    assert!(near(v, exp, 0.02), "torus R=30 r=5: V={v:.1}, expected {exp:.1}");
}

/// ARBITRARY SKETCH AXIS (a construction line): a half-disc around a VERTICAL line — the same
/// sphere. This is the case the axis-limited command could not express at all.
#[test]
fn arbitrary_sketch_line_as_axis_makes_sphere() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("sphere-2");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "sphere-2");
    // a half-disc pressed against the VERTICAL line x=25 (a custom axis, neither sketch X nor Y)
    let (cx, r) = (25.0, 8.0);
    let n = 48;
    let mut pts = vec![Point2::new(cx, -r)];
    for k in 1..n {
        let a = PI * k as f64 / n as f64;
        pts.push(Point2::new(cx + r * a.sin(), -r * a.cos()));
    }
    pts.push(Point2::new(cx, r));
    for w in pts.windows(2) {
        p.add_line_entity(si, w[0].x, w[0].y, w[1].x, w[1].y, qymcad_core::feature::Purpose::Real);
    }
    p.add_line_entity(si, cx, r, cx, -r, qymcad_core::feature::Purpose::Real); // diameter along the future axis
    let axis = p.add_line_entity(si, cx, -50.0, cx, 50.0, qymcad_core::feature::Purpose::Construction); // construction line
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("contour");
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, axis);
    let (v, errs) = build(&mut p, body);
    assert!(errs.is_empty(), "regen errors: {errs:?}");
    let exp = 4.0 / 3.0 * PI * r * r * r;
    assert!(near(v, exp, 0.03), "sphere around a custom axis: V={v:.1}, expected {exp:.1}");
}

/// A TILTED arbitrary axis (45 degrees) — a solid of revolution must build around it too.
#[test]
fn tilted_sketch_line_as_axis_builds() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("tilt");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "tilt");
    // a square offset from the origin, and an axis at 45 degrees through the origin
    p.add_rect_entity(si, 20.0, 5.0, 30.0, 15.0, qymcad_core::feature::Purpose::Real);
    let axis = p.add_line_entity(si, -50.0, -50.0, 50.0, 50.0, qymcad_core::feature::Purpose::Construction);
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("contour");
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, axis);
    let (v, errs) = build(&mut p, body);
    assert!(errs.is_empty(), "regen errors: {errs:?}");
    assert!(v > 0.0, "solid around a tilted axis was built: V={v:.1}");
}

/// HONEST REFUSAL: the profile CROSSES the axis (a full circle centred ON the axis). A revolve like
/// that has no defined result, so it must be reported plainly instead of quietly building garbage.
#[test]
fn profile_crossing_axis_is_honest_error() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("crossing");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "crossing");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real); // centre ON the X axis -> the profile crosses it
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids[0];
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, 0, 0);
    let last = p.finish_base_body(body, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let v = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
    let sphere = 4.0 / 3.0 * PI * 1000.0;
    let honest_error = !report.errors.is_empty();
    let sane_solid = near(v, sphere, 0.05); // either an honest error, or a MEANINGFUL solid (a sphere)
    eprintln!("[A8] circle centred ON the axis: V={v:.1}, errors: {:?}", report.errors);
    assert!(honest_error || sane_solid, "profile crosses the axis: neither an error nor a meaningful solid (V={v:.1}); errors: {:?}", report.errors);
}

/// THE CROSSING CHECK AND THE BUILD LOOK AT THE SAME AXIS — verified through a datum axis.
///
/// Axis priority (sketch construction line -> datum through pl^-1 -> X/Y fallback) was spelled out
/// TWICE in the revolve branch: once for the "profile crosses the axis" check, once for the kernel
/// call. While the copies agreed everything worked; the moment they diverged the check would
/// validate an axis OTHER than the one the solid is built around, silently letting through a profile
/// that cannot be revolved at all. The datum case is the dangerous one: only there is the axis
/// mapped into sketch-local space by an inverse matrix.
#[test]
fn crossing_check_and_build_agree_on_a_datum_axis() {
    let mut p = Project::default();
    p.new_document();
    let ax = p.add_datum_axis(qymcad_core::model::DatumAxis::manual("axis X", [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    let si = p.new_sketch("circle on the axis");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "circle on the axis");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real); // centre ON the datum axis -> the profile crosses it
    p.regen_sketch(si);
    let cid = p.sketches[si].contour_ids[0];
    let body = p.add_revolve_axis(sid, vec![cid], 0, 360.0, ax, 0);
    let last = p.finish_base_body(body, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let v = shapes.get(&last).map(|s| s.volume()).unwrap_or(0.0);
    let sphere = 4.0 / 3.0 * PI * 1000.0;
    eprintln!("[datum axis] V={v:.1}, errors: {:?}", report.errors);
    assert!(
        !report.errors.is_empty() || near(v, sphere, 0.05),
        "around the datum axis: neither an honest error nor a meaningful solid (V={v:.1}) — the check and the build disagree on the axis; errors: {:?}",
        report.errors
    );
}
