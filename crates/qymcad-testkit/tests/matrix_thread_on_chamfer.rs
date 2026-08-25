//! A thread on a shaft that already carries a chamfer.
//!
//! With a chamfer placed on the cylinder beforehand the thread failed to build — half a turn in some cases, an
//! outright error in others. The cause was that on a rebuild the direction of the thread was taken from the
//! whole mesh, by where most of the vertices of the body lay. With a chamfer the rim of the thread sits at the
//! base of that chamfer, and on a part whose mass is above the rim the thread ran off into the air: on the
//! reported part it removed 23 mm³ instead of 5900. The preview showed the right direction all along — what
//! diverged were the face selection and the rebuild.
use qymcad_core::model::Project;
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

/// A shaft of diameter `d` and height `h` with a chamfer `c` on its upper rim. It returns the project, the
/// body and the id of that rim.
fn shaft_with_chamfer(d: f64, h: f64, c: f64) -> (Project, u64, u32) {
    let mut p = Project::default();
    p.new_document();
    let body = p.add_cylinder(d * 0.5, h);
    let _ = qymcad_testkit::regenerate(&mut p);
    // the upper rim, which is what the chamfer is taken off
    let top = p
        .regen_edges
        .get(&body)
        .and_then(|es| es.iter().filter(|e| e.radius > 1e-9).max_by(|a, b| a.center[2].total_cmp(&b.center[2])).map(|e| e.id))
        .expect("the upper rim");
    let ch = p.add_chamfer(body, c, vec![top]);
    let _ = qymcad_testkit::regenerate(&mut p);
    // after the chamfer the rim of the thread is the circle of the cylinder radius at the base of that chamfer
    let rim = p
        .regen_edges
        .get(&ch)
        .and_then(|es| es.iter().filter(|e| (e.radius - d * 0.5).abs() < 0.05).max_by(|a, b| a.center[2].total_cmp(&b.center[2])).map(|e| e.id))
        .expect("the rim at the base of the chamfer");
    (p, ch, rim)
}

#[test]
fn thread_on_a_chamfered_shaft_cuts_into_the_body() {
    let (d, h, c, len) = (20.0, 60.0, 2.0, 30.0);
    let (mut p, body, rim) = shaft_with_chamfer(d, h, c);
    let before = p.mesh_index(body).map(|i| p.bodies[i].mesh.volume()).unwrap_or(0.0);
    assert!(before > 0.0, "the shaft with a chamfer was built");
    let spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch: 2.5, fit: 0.2, ..Default::default() };
    let g = spec.geometry();
    let t = p.add_thread(body, rim, spec, len, 0.0, 0.0);
    let last = p.finish_base_body(t, 1);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "a thread on a chamfered shaft builds without errors: {:?}", rep.errors);
    let after = shapes.get(&last).map(|s| s.tessellate(0.05).iter().map(|b| b.0.volume()).sum::<f64>()).unwrap_or(0.0);
    let removed = before - after;
    // the ring between the major diameter and the root over the threaded length; the groove takes about half
    // of it
    let ring = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * len;
    eprintln!("a Ø{d} shaft with a chamfer of {c}: removed {removed:.1} mm³ against a ring of {ring:.1}");
    assert!(
        removed > 0.2 * ring,
        "the thread ran clear of the body: {removed:.1} mm³ removed against a ring of {ring:.1}, so the direction from the rim was not taken along the cylinder"
    );
    assert!(removed < 1.2 * ring, "and it did not eat away too much: {removed:.1} against {ring:.1}");
}
