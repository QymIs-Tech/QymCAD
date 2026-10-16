use super::tri_depth_at;
use egui::Pos2;

/// Thin 2 mm walls: a click near the FAR edge of a large triangle on the outer face. The mean depth of the outer
/// triangle's vertices, (0+0+30)/3=10, is DEEPER than the centroid of the small inner triangle (2), so the old
/// pick returned the inner wall. The barycentric depth AT THE CLICK POINT must return the outer one (0.6 < 2.0).
#[test]
fn thin_wall_outer_face_wins_at_click_point() {
    // the outer face: a huge triangle tilted away into the depth (vertex C is far off)
    let (a, da) = (Pos2::new(0.0, 0.0), 0.0);
    let (b, db) = (Pos2::new(100.0, 0.0), 0.0);
    let (c, dc) = (Pos2::new(0.0, 100.0), 30.0);
    // the inner face (2 mm deeper at the click point): a small triangle around the click
    let (a2, da2) = (Pos2::new(5.0, -5.0), 2.0);
    let (b2, db2) = (Pos2::new(25.0, 15.0), 2.0);
    let (c2, dc2) = (Pos2::new(5.0, 15.0), 2.0);
    let click = Pos2::new(10.0, 2.0); // inside both
    let outer_depth = tri_depth_at(click, a, da, b, db, c, dc); // about 0.6 (the click is near the close edge)
    let inner_depth = tri_depth_at(click, a2, da2, b2, db2, c2, dc2); // 2.0
    assert!(outer_depth < inner_depth, "the outer wall is nearer AT THE CLICK POINT: {outer_depth:.2} < {inner_depth:.2}");
    // the old (centroid) criterion chose WRONGLY - this records that the fault was real
    let outer_centroid = (da + db + dc) / 3.0;
    assert!(outer_centroid > inner_depth, "the centroid criterion gave the outer face a depth of {outer_centroid:.2} > {inner_depth:.2}, hence the wrong pick (the proof of the fault)");
}

/// A flat triangle (a constant depth): the interpolation matches the mean.
#[test]
fn flat_triangle_depth_constant() {
    let d = tri_depth_at(Pos2::new(3.0, 3.0), Pos2::new(0.0, 0.0), 7.0, Pos2::new(10.0, 0.0), 7.0, Pos2::new(0.0, 10.0), 7.0);
    assert!((d - 7.0).abs() < 1e-9);
}
