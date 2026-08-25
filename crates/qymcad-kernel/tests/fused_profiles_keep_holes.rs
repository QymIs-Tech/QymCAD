//! Fusing profiles must not lose the inner loops.
//!
//! The report: selecting a large contour, another contour and a second nested one, expecting them to extrude as
//! one body with a cut in the centre from the smallest, produced a solid slab instead.
//!
//! Profiles are fused into one planar face by a 2D boolean before extruding, so that touching contours give a
//! body without seam edges. What is checked here is that the union keeps the inner loops: a ring inside a ring
//! with an island has to give material with a hole in the centre rather than a solid slab.
use qymcad_core::geom::{encode_profile, Contour, Point2};

fn rect(cx: f64, cy: f64, w: f64, h: f64) -> Contour {
    let (hw, hh) = (0.5 * w, 0.5 * h);
    Contour::closed(vec![
        Point2::new(cx - hw, cy - hh),
        Point2::new(cx + hw, cy - hh),
        Point2::new(cx + hw, cy + hh),
        Point2::new(cx - hw, cy + hh),
    ])
}

#[test]
fn fused_profiles_keep_their_holes() {
    // a 100×100 ring with a 60×60 hole, and inside it a 60×60 ring with a 20×20 hole, leaving 10000 − 400 =
    // 9600 of material
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    let mid = rect(0.0, 0.0, 60.0, 60.0);
    let inner = rect(0.0, 0.0, 20.0, 20.0);
    let profiles = vec![encode_profile(&outer, &[&mid]), encode_profile(&mid, &[&inner])];
    let s = qymcad_kernel::Shape::extrude_profiles_fused(&profiles, 1.0).expect("fusing the profiles");
    let v = s.volume();
    assert!(
        (v - 9600.0).abs() < 1.0,
        "the fusion has to keep the hole in the centre: V={v:.1}, expecting 9600; a solid slab would give 10000 and the sum of the outer loops 13600"
    );
}

/// The same case with an island inside the hole: the island is material and the hole remains around it.
#[test]
fn fused_profiles_keep_holes_with_island() {
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    let mid = rect(0.0, 0.0, 60.0, 60.0);
    let island = rect(0.0, 0.0, 10.0, 10.0);
    let profiles = vec![encode_profile(&outer, &[&mid]), encode_profile(&island, &[])];
    let s = qymcad_kernel::Shape::extrude_profiles_fused(&profiles, 1.0).expect("fusing the profiles");
    let v = s.volume();
    assert!((v - 6500.0).abs() < 1.0, "a ring of 6400 plus an island of 100 gives 6500, got {v:.1}");
}

/// The main point: the outer loop and the hole may arrive wound the same way, which is what happened in the
/// failing sketch — the exact edges of the outer loop ran clockwise and those of the hole counter-clockwise.
/// The kernel does not check orientation: a blind `Reversed()` made the hole co-directed with the outer loop,
/// the face came out broken and the areas added instead of subtracting, so a slab appeared where a body with a
/// cut was expected. Orientation has to be computed from the geometry.
#[test]
fn hole_is_subtracted_whatever_the_input_winding() {
    let outer_ccw = rect(0.0, 0.0, 100.0, 100.0);
    let hole_ccw = rect(0.0, 0.0, 40.0, 40.0);
    let mut outer_cw = outer_ccw.clone();
    outer_cw.points.reverse();
    let mut hole_cw = hole_ccw.clone();
    hole_cw.points.reverse();

    for (name, outer, hole) in [
        ("outer counter-clockwise, hole counter-clockwise", &outer_ccw, &hole_ccw),
        ("outer counter-clockwise, hole clockwise", &outer_ccw, &hole_cw),
        ("outer clockwise, hole counter-clockwise", &outer_cw, &hole_ccw),
        ("outer clockwise, hole clockwise", &outer_cw, &hole_cw),
    ] {
        let prof = encode_profile(outer, &[hole]);
        let s = qymcad_kernel::Shape::extrude_profile(&prof, 1.0).expect("the extrusion");
        let v = s.volume();
        assert!(s.is_valid(), "{name}: the face has to be valid");
        assert!((v - 8400.0).abs() < 1.0, "{name}: the hole subtracts, giving V={v:.1}, expecting 8400; adding would give 11600");
    }
}
