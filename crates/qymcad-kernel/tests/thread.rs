//! A real thread — a helix, a pipe shell and a boolean cut — as genuine B-rep rather than cosmetics.
use qymcad_kernel::Shape;

#[test]
fn external_thread_cuts_real_grooves() {
    // A Ø10 cylinder, height 20, about the Z axis, with an external thread close to M10: pitch 1.5, angle 60°,
    // thread depth 0.9, length 16.
    let cyl = Shape::cylinder(5.0, 20.0).expect("a Ø10 cylinder of height 20");
    let v0 = cyl.volume();
    assert!(v0 > 1550.0 && v0 < 1590.0, "the volume of the cylinder is πr²h = π·25·20 ≈ 1571: {v0:.1}");

    let thr = cyl
        .thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0)
        .expect("the thread is cut: helix, pipe and cut gave a valid body");
    let v1 = thr.volume();
    assert!(v1 > 0.0, "the volume of the threaded body is above zero");
    assert!(v1 < v0, "the thread removed material, the grooves being cut: {v0:.1} before, {v1:.1} after");
    assert!(v1 > 0.5 * v0, "it removed a part rather than half the body, the groove being shallow: {v1:.1} against {v0:.1}");

    let bodies = thr.tessellate(0.1);
    assert_eq!(bodies.len(), 1, "one body");
    let (mesh, faces) = &bodies[0];
    assert!(!mesh.tris.is_empty(), "the mesh is not empty, so the body tessellates");
    assert!(faces.len() > 3, "the thread added B-rep faces, a bare cylinder having three: {} now", faces.len());
}

#[test]
fn internal_thread_on_hole() {
    // A bushing: Ø20 with a Ø10 hole. An internal thread on the hole cuts grooves into the wall, so material
    // goes away.
    let outer = Shape::cylinder(10.0, 20.0).expect("the outer cylinder");
    let inner = Shape::cylinder(5.0, 20.0).expect("the hole");
    let tube = outer.boolean(&inner, 0).expect("the bushing, by a cut");
    let v0 = tube.volume();
    assert!(v0 > 0.0);

    let thr = tube
        .thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Bore, 0, 0.0, 0.0, 0.0, 0.0)
        .expect("the internal thread is cut");
    let v1 = thr.volume();
    assert!(v1 < v0 && v1 > 0.5 * v0, "the internal thread removed a part of the wall: {v0:.1} -> {v1:.1}");
    assert_eq!(thr.tessellate(0.1).len(), 1, "one body");
}

#[test]
fn two_start_thread_is_valid_solid() {
    // A two-start thread: fusing the two grooves and then cutting gives a valid body — it does not fail, it is
    // one solid, material is removed and the thread faces are added. The exact ratio of volumes is confused by
    // the width of the profile, which follows the pitch, so validity is checked rather than the amount removed.
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let v0 = cyl.volume();
    let two = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 2, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("two starts are cut");
    let v2 = two.volume();
    assert!(v2 > 0.0 && v2 < v0, "a two-start thread is a valid body with material removed: {v0:.1} -> {v2:.1}");
    let bodies = two.tessellate(0.15);
    assert_eq!(bodies.len(), 1, "one body");
    assert!(bodies[0].1.len() > 3, "the thread faces are added, the cylinder having had three: {}", bodies[0].1.len());
}

#[test]
fn angle_drives_flanks() {
    // The crest is set by the form while the angle sets the flanks, hence the width of the root:
    // hw_bot = hw_top − depth·tan(angle/2). A sharp 30° leaves the flanks nearly radial, so the groove stays wide
    // down to the root and more is removed; at 90° it narrows to a sharp root and less is removed.
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let v0 = cyl.volume();
    let a30 = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 30.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("a 30° thread");
    let a90 = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 90.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("a 90° thread");
    let (v30, v90) = (a30.volume(), a90.volume());
    assert!(v30 < v0 && v90 < v0, "both removed material");
    assert!(v30 < v90 - 1.0, "the angle governs the flanks: 30° removed markedly more than 90°: 30° -> {v30:.1}, 90° -> {v90:.1}");
}

#[test]
fn form_changes_crest_land() {
    // The form changes the crest: a triangle, with a wide groove and a sharp crest, removes more than a
    // trapezoid, with a narrow groove and a wide flat crest. A direct answer to the claim that the crest does
    // not change.
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let v0 = cyl.volume();
    let tri = v0 - cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("the triangular form").volume();
    let trap = v0 - cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 1, 0.0, 0.0, 0.0, 0.0).expect("the trapezoidal form").volume();
    assert!(tri > trap + 5.0, "the form changes the crest: the triangle removed markedly more than the trapezoid: tri={tri:.1}, trap={trap:.1}");
}

#[test]
fn rounded_form_and_clearance_build() {
    // The rounded profile — a cosine bowl with both root and crest rounded, the rolled thread used for 3D
    // printing — together with crest and root clearances builds into a valid body that removes material. And it
    // really cuts: a tangent root once made the boolean a no-op, so the thread did nothing.
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let v0 = cyl.volume();
    let thr = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 2, 0.1, 0.15, 0.0, 0.0).expect("the rounded form with clearances");
    let v1 = thr.volume();
    assert!(v1 < v0 - 20.0, "the round profile really cuts a groove rather than doing nothing: {v0:.1} -> {v1:.1}");
    assert!(v1 > 0.5 * v0, "but it removed a part, not half the body");
    assert_eq!(thr.tessellate(0.1).len(), 1, "one body");
    // the round profile differs from the sharp triangle, otherwise choosing the type would change nothing
    let tri = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("the triangular form").volume();
    assert!((v1 - tri).abs() > 15.0, "the round form differs markedly from the triangle: round={v1:.1}, triangle={tri:.1}");
}

#[test]
fn runout_leaves_material_at_ends() {
    // A smooth run-in and run-out: the depth of the thread melts to zero at the ends, by the homothety law in
    // the pipe shell, so the groove cuts less there and more material remains than with a thread at full depth
    // right to the ends; a longer run leaves more material still. It used to be a chamfered cut-off, with the
    // opposite meaning.
    let cyl = Shape::cylinder(5.0, 20.0).expect("the cylinder");
    let plain = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("without a run-out");
    let run = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 4.0, 4.0).expect("a run-out of 4");
    let big = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, 16.0, 1.5, 60.0, 0.9, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 7.0, 7.0).expect("a run-out of 7");
    let (vp, vc, vb) = (plain.volume(), run.volume(), big.volume());
    assert!(vc > vp + 1.0, "the run-out left the turns at the ends, so there is more material: full {vp:.1} -> run-out {vc:.1}");
    assert!(vb > vc, "a longer run-out leaves more material: run-out 4 = {vc:.1} < run-out 7 = {vb:.1}");
    assert_eq!(run.tessellate(0.1).len(), 1, "one body");
}

#[test]
fn thread_into_shoulder_builds() {
    // A thread on a shaft running into a shoulder: the run-out sinks the turn at the step and the body does not
    // break — the step used to come out broken. A Ø40 flange of height 10 fused with a Ø30 shaft through it, and
    // the thread cut downward to the step.
    let body = Shape::cylinder(20.0, 10.0).unwrap().boolean(&Shape::cylinder(15.0, 40.0).unwrap(), 1).unwrap();
    let vb = body.volume();
    let thr = body.thread([0.0, 0.0, 40.0], [0.0, 0.0, -1.0], 15.0, 30.0, 3.0, 60.0, 1.8, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 3.0, 3.0).expect("the thread down to the shoulder");
    let v1 = thr.volume();
    eprintln!("the step: body {vb:.1} -> thread {v1:.1}");
    assert!(v1 > 0.0 && v1 < vb, "the thread is cut in a body with a step: {vb:.1} -> {v1:.1}");
    assert_eq!(thr.tessellate(0.3).len(), 1, "one body: the shoulder did not fall apart");
}

/// The amplitude of the turn, the largest radius minus the smallest, in a thin slab between `zlo` and `zhi`.
/// Near zero it means a smooth surface, the turn having run out.
fn ring_amplitude(s: &Shape, zlo: f64, zhi: f64) -> f64 {
    let bodies = s.tessellate(0.05);
    let (mesh, _) = &bodies[0];
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    for v in &mesh.verts {
        let (x, y, z) = (v.x as f64, v.y as f64, v.z as f64);
        if z < zlo || z >= zhi { continue; }
        let r = (x * x + y * y).sqrt();
        lo = lo.min(r); hi = hi.max(r);
    }
    if hi > 0.0 { hi - lo } else { 0.0 }
}

#[test]
fn runout_sinks_crest_at_tip() {
    // A vanishing thread: at the rim the depth melts to zero, so the amplitude there is small and the surface
    // smooth, while deeper in it is full. The thread runs from the top end at z = 40 downward, as it does in
    // practice. The crest stays at R, the homothety being towards the spine, so the outer diameter does not sink
    // at the end and a nut starts onto a smooth cone.
    let cyl = Shape::cylinder(10.0, 40.0).expect("the cylinder");
    let thr = cyl.thread([0.0, 0.0, 40.0], [0.0, 0.0, -1.0], 10.0, 20.0, 3.0, 60.0, 1.5, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 3.0, 3.0).expect("a thread with a run-out");
    let tip = ring_amplitude(&thr, 39.75, 40.0);   // the very entry, at the top rim
    let middle = ring_amplitude(&thr, 28.0, 30.0); // deeper in, a full turn
    assert!(middle > 1.0, "deeper in the turn is full: amp={middle:.2}");
    assert!(tip < 0.35, "at the end the turn has melted away, a smooth run-out: amp={tip:.2}");
}

/// A smooth run-out together with segmentation: a long thread with a run-out builds, segmentation having not
/// broken the law; the run-out leaves material at the ends as the depth melts, yet the thread is cut in the
/// middle.
#[test]
fn long_thread_with_runout_builds() {
    let cyl = Shape::cylinder(5.0, 34.0).expect("the cylinder");
    let len = 30.0; // 30 turns at a pitch of 1
    let plain = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, len, 1.0, 60.0, 0.6, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0).expect("without a run-out");
    let run = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, len, 1.0, 60.0, 0.6, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 1.5, 1.5).expect("with a run-out");
    eprintln!("a long thread: full {:.1}, with a run-out {:.1}", plain.volume(), run.volume());
    assert!(run.volume() > plain.volume(), "the run-out left the turns at the ends, the depth melting and material remaining");
    assert!(run.volume() < cyl.volume(), "but the thread is cut, material being removed in the middle");
}

/// Segmentation: a long thread of more than about 30 turns used to tear the pipe shell. It is now segmented at
/// no more than 18 turns per segment and has to build. The range checked is the one that gets printed, up to
/// about 50 turns.
#[test]
fn long_thread_builds_via_segmentation() {
    for (len, pitch) in [(30.0, 1.0), (50.0, 1.0)] {
        let turns = len / pitch;
        let cyl = Shape::cylinder(5.0, len + 4.0).expect("the cylinder");
        let thr = cyl.thread([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 5.0, len, pitch, 60.0, 0.6, 1, qymcad_kernel::Hand::Right, qymcad_kernel::Site::Shaft, 0, 0.0, 0.0, 0.0, 0.0);
        eprintln!("{turns:.0} turns: {}", if thr.is_some() { "built" } else { "failed" });
        let thr = thr.unwrap_or_else(|| panic!("a thread of {turns:.0} turns has to build, through segmentation"));
        assert!(thr.volume() > 0.0 && thr.volume() < cyl.volume(), "material was removed");
    }
}
