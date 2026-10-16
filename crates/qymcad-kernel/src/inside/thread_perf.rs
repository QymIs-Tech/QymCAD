//! Performance and behaviour of a thread: the run-out through a homothety law, which is fast and keeps the crest at R,
//! plus the angle across the whole range.
//!
//! The budgets of time hold at the level of the probes of time alone (`qymcad_core::time_budget`); what the thread is -
//! its crest, the weight of its angle - is held at every level. Measured among the 157 probes of the bridge run
//! together: 1.5 s and 2 s budgets read over them, while alone both probes took 3.1 s together.
use crate::Shape;
use std::time::{Duration, Instant};

#[test]
fn runout_fast_and_crest_stays_at_radius() {
    let cyl = Shape::cylinder(15.0, 40.0).unwrap();
    let t = Instant::now();
    let thr = cyl
        .thread(crate::ThreadCut {
            axis: qymcad_core::feature::AxisLine { origin: [0.0, 0.0, 40.0], dir: [0.0, 0.0, -1.0] },
            radius: 15.0,
            length: 10.0,
            pitch: 1.5,
            angle_deg: 60.0,
            depth: 0.9,
            starts: 1,
            hand: crate::Hand::Right,
            site: crate::Site::Shaft,
            form: 0,
            clearance_crest: 0.0,
            clearance_root: 0.0,
            lead_in: 1.5,
            lead_out: 1.5,
        })
        .unwrap();
    let took = t.elapsed();
    let dt = took.as_secs_f64();
    let b = thr.tessellate(0.05);
    let mut tip_max = 0.0f64;
    for v in &b[0].mesh.verts {
        if v.z < 39.7 {
            continue;
        }
        tip_max = tip_max.max((v.x.powi(2) + v.y.powi(2)).sqrt());
    }
    eprintln!("build {dt:.2}s, crest at the end max_r={tip_max:.2}");
    assert!(qymcad_core::time_budget::within(took, Duration::from_millis(1500)), "the run-out by a homothety law is fast, with no booleans: {dt:.2}s");
    assert!(tip_max > 14.5, "the crest stays at R: {tip_max:.2}");
}

#[test]
fn angle_high_builds_fast_and_varies() {
    // a large angle builds, and builds quickly with a true point, and the angle visibly changes the removal
    let cyl = Shape::cylinder(15.0, 20.0).unwrap();
    let v0 = cyl.volume();
    let mut rem = vec![];
    for ang in [30.0f64, 60.0, 90.0, 120.0] {
        let t = Instant::now();
        let r = cyl
            .thread(crate::ThreadCut {
                axis: qymcad_core::feature::AxisLine { origin: [0.0, 0.0, 20.0], dir: [0.0, 0.0, -1.0] },
                radius: 15.0,
                length: 12.0,
                pitch: 2.0,
                angle_deg: ang,
                depth: 1.0,
                starts: 1,
                hand: crate::Hand::Right,
                site: crate::Site::Shaft,
                form: 0,
                clearance_crest: 0.0,
                clearance_root: 0.0,
                lead_in: 0.0,
                lead_out: 0.0,
            })
            .unwrap_or_else(|| panic!("the angle {ang} did not build"));
        assert!(qymcad_core::time_budget::within(t.elapsed(), Duration::from_secs(2)), "the angle {ang} deg builds quickly");
        rem.push(v0 - r.volume());
    }
    assert!(rem[0] > rem[3] + 20.0, "the angle matters: 30° gives {:.1}, far above 120° at {:.1}", rem[0], rem[3]);
}
