use super::{compose12, rot_about_point};
use qymcad_core::feature::{apply12, is_identity12, PLACE_IDENTITY};

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
}

#[test]
fn compose_identity_is_noop() {
    let t = [1.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 6.0]; // a translation of (4,5,6)
    assert_eq!(compose12(&PLACE_IDENTITY, &t), t, "I∘T = T");
    assert_eq!(compose12(&t, &PLACE_IDENTITY), t, "T∘I = T");
}

#[test]
fn compose_translations_add() {
    let a = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0];
    let b = [1.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 6.0];
    let c = compose12(&a, &b); // the two translations add up
    assert!(close(apply12(&c, [0.0, 0.0, 0.0]), [5.0, 7.0, 9.0]), "the translations add up");
}

#[test]
fn sketch_origin_uv_invariant_under_placement() {
    // the snapped origin is stored as (u,v) in the frame's axes. project(snap) on the WORLD frame gives (u,v),
    // and lift(u,v) on the LOCAL frame plus the body's placement reproduces the same world point.
    use qymcad_core::feature::PlaneFrame;
    use qymcad_core::geom::{Point2, Point3};
    let f_local = PlaneFrame::world_aligned([3.0, 4.0, 2.0], [0.0, 0.0, 1.0], 0.0);
    let wt = compose12(&rot_about_point(2, 90.0, [0.0, 0.0, 0.0]), &[1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 20.0, 0.0, 0.0, 1.0, 30.0]);
    let f_world = f_local.transformed(&wt);
    let (u, v) = (5.0, -7.0);
    let w = f_world.lift(Point2::new(u, v)); // the snapped world point on the plane
    let uv = f_world.project(Point3::new(w.x, w.y, w.z)); // pick→project
    assert!((uv.x - u).abs() < 1e-9 && (uv.y - v).abs() < 1e-9, "project after lift is the identity in the world frame");
    let o_local = f_local.lift(uv); // as in sketch_frame: the local origin is shifted
    let o_world = apply12(&wt, [o_local.x, o_local.y, o_local.z]);
    assert!(close(o_world, [w.x, w.y, w.z]), "u*X + v*Y is invariant to the body's placement");
}

#[test]
fn rot_z90_about_origin() {
    let m = rot_about_point(2, 90.0, [0.0, 0.0, 0.0]); // 90 degrees about Z
    assert!(close(apply12(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "X goes to Y at +90 degrees about Z");
    assert!(close(apply12(&m, [0.0, 0.0, 5.0]), [0.0, 0.0, 5.0]), "Z stays put");
}

#[test]
fn rot_about_point_keeps_center_fixed() {
    let c = [10.0, 20.0, 0.0];
    let m = rot_about_point(2, 37.0, c);
    assert!(close(apply12(&m, c), c), "the centre of rotation stays put");
    // accumulating by composition (as during a repeated drag): two 45-degree turns make 90
    let m45 = rot_about_point(2, 45.0, [0.0, 0.0, 0.0]);
    let m90 = compose12(&m45, &m45);
    assert!(close(apply12(&m90, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "45°∘45° = 90°");
    assert!(!is_identity12(&m90), "not the identity");
}

#[test]
fn rot_x_and_y_axes() {
    // 90 degrees about X takes Y to Z
    let mx = rot_about_point(0, 90.0, [0.0, 0.0, 0.0]);
    assert!(close(apply12(&mx, [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]), "about X: Y goes to Z");
    // 90 degrees about Y takes Z to X
    let my = rot_about_point(1, 90.0, [0.0, 0.0, 0.0]);
    assert!(close(apply12(&my, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]), "about Y: Z goes to X");
}

#[test]
fn straight_poly_accepts_line_rejects_arc() {
    use super::is_straight_poly;
    // a straight edge (several points on one line) works as an axis
    let line = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]];
    assert!(is_straight_poly(&line), "collinear points make a straight line");
    // a quarter arc of R=10 is NOT an axis (its chord would become a false one)
    let arc: Vec<[f32; 3]> = (0..=8)
        .map(|i| {
            let a = std::f64::consts::FRAC_PI_2 * i as f64 / 8.0;
            [10.0 * a.cos() as f32, 10.0 * a.sin() as f32, 0.0]
        })
        .collect();
    assert!(!is_straight_poly(&arc), "an arc is not straight");
    // the degenerate cases (one point, or zero length) are not axes
    assert!(!is_straight_poly(&[[1.0f32, 1.0, 1.0]]));
    assert!(!is_straight_poly(&[[1.0f32, 1.0, 1.0], [1.0, 1.0, 1.0]]));
}

#[test]
fn axis_segment_centers_on_origin_along_dir() {
    use super::axis_segment;
    // an axis along Y through (0,5,0) of length +/-10 gives the ends (0,-5,0) and (0,15,0); dir is normalised
    let (a, b) = axis_segment([0.0, 5.0, 0.0], [0.0, 2.0, 0.0], 10.0);
    assert!(close(a, [0.0, -5.0, 0.0]) && close(b, [0.0, 15.0, 0.0]), "plus and minus len*dir from the origin: {a:?} {b:?}");
}

#[test]
fn camera_basis_top_view_stays_orthonormal() {
    use super::Cam3;
    // THE TOP VIEW (a pitch of a quarter turn): the basis used to degenerate here (the cross product with the world Z is zero, giving NaN and a collapsed picture).
    let cam = Cam3 { pitch: std::f64::consts::FRAC_PI_2, ..Default::default() };
    let (r, u, f) = cam.basis();
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let len = |a: [f64; 3]| dot(a, a).sqrt();
    for v in [r, u, f] {
        assert!((len(v) - 1.0).abs() < 1e-6 && v.iter().all(|x| x.is_finite()), "a unit, finite vector: {v:?}");
    }
    assert!(dot(r, u).abs() < 1e-6 && dot(r, f).abs() < 1e-6 && dot(u, f).abs() < 1e-6, "the top view's basis is orthogonal");
    assert!(f[2] < -0.99, "a camera above looks downwards (-Z): {f:?}");
}

#[test]
fn ray_plane_hits_and_skips_parallel() {
    use super::ray_plane;
    // a ray from (2,3,10) downwards along -Z meets the plane Z=0 at (2,3,0)
    let hit = ray_plane([2.0, 3.0, 10.0], [0.0, 0.0, -1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(hit.is_some_and(|w| close(w, [2.0, 3.0, 0.0])), "the intersection with Z=0: {hit:?}");
    // an offset plane at Z=5 gives (2,3,5)
    let hit2 = ray_plane([2.0, 3.0, 10.0], [0.0, 0.0, -1.0], [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
    assert!(hit2.is_some_and(|w| close(w, [2.0, 3.0, 5.0])), "the intersection with Z=5: {hit2:?}");
    // a ray parallel to the plane (along X, with a Z normal) gives None
    assert!(ray_plane([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "parallel means no intersection");
}
