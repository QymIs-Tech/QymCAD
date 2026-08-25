// Performance and behaviour of a thread: the run-out through a homothety law, which is fast and keeps the
// crest at R, plus the angle across the whole range.
use qymcad_kernel::Shape;
use std::time::Instant;

#[test]
fn runout_fast_and_crest_stays_at_radius() {
    let cyl = Shape::cylinder(15.0, 40.0).unwrap();
    let t = Instant::now();
    let thr = cyl.thread([0.0,0.0,40.0],[0.0,0.0,-1.0], 15.0, 10.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0,0.0, 1.5,1.5).unwrap();
    let dt = t.elapsed().as_secs_f64();
    let b = thr.tessellate(0.05);
    let mut tip_max = 0.0f64;
    for v in &b[0].0.verts { if (v.z as f64) < 39.7 { continue; }
        tip_max = tip_max.max(((v.x as f64).powi(2) + (v.y as f64).powi(2)).sqrt()); }
    eprintln!("build {dt:.2}s, crest at the end max_r={tip_max:.2}");
    assert!(dt < 1.5, "the run-out by a homothety law is fast, with no booleans: {dt:.2}s");
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
        let r = cyl.thread([0.0,0.0,20.0],[0.0,0.0,-1.0], 15.0, 12.0, 2.0, ang, 1.0, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0,0.0, 0.0,0.0)
            .unwrap_or_else(|| panic!("the angle {ang} did not build"));
        assert!(t.elapsed().as_secs_f64() < 2.0, "the angle {ang}° builds quickly");
        rem.push(v0 - r.volume());
    }
    assert!(rem[0] > rem[3] + 20.0, "the angle matters: 30° gives {:.1}, far above 120° at {:.1}", rem[0], rem[3]);
}
