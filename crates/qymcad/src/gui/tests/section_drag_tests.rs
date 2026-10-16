use super::section_drag_delta_offset;
use egui::Pos2;

/// Reported behaviour: the plane reset every time the gizmo was grabbed. At the moment of the grab the cursor
/// stands NOT at o0 (offset=0) but at THE ARROW'S TIP - already far from o0, by both the current offset and the
/// arrow's length. The old formula (an absolute reprojection from o0) jumped by that distance on the very first
/// frame. A delta from the anchor must give EXACTLY off0 (no change) while the cursor has not moved.
#[test]
fn no_jump_at_grab_regardless_of_arrow_length() {
    let s0 = Pos2::new(400.0, 300.0); // the screen projection of o0 (offset=0)
    let s1 = Pos2::new(450.0, 300.0); // the projection of o0 plus the normal (50px = 1 mm along the normal on screen)
                                      // the cursor is GRABBED far from s0 (at the arrow, with 30 mm of offset already accumulated plus the arrow's 80px)
    let p0 = Pos2::new(1200.0, 300.0);
    let off0 = 30.0;
    let new_off = section_drag_delta_offset(off0, p0, s0, s1, p0).expect("not degenerate");
    assert!((new_off - off0).abs() < 1e-9, "the cursor has not moved since the grab, so the offset did not change: {new_off} (was {off0})");
}

/// From then on the drag is proportional to the mouse's movement from the anchor (in px per mm from s0/s1), not from o0.
#[test]
fn tracks_cursor_movement_from_anchor() {
    let s0 = Pos2::new(400.0, 300.0);
    let s1 = Pos2::new(450.0, 300.0); // 50px = 1 mm
    let p0 = Pos2::new(1200.0, 300.0);
    let off0 = 30.0;
    let cur = Pos2::new(1250.0, 300.0); // +50px from the anchor is +1 mm
    let new_off = section_drag_delta_offset(off0, p0, s0, s1, cur).unwrap();
    assert!((new_off - 31.0).abs() < 1e-9, "a movement of +50px (1 mm) from the anchor gives an offset of 30+1=31: {new_off}");
}

/// The degenerate case (the section's normal points straight at the camera, so s1 equals s0 on screen) does not panic.
#[test]
fn degenerate_zero_length_screen_normal_returns_none() {
    let s0 = Pos2::new(400.0, 300.0);
    assert!(section_drag_delta_offset(0.0, s0, s0, s0, s0).is_none());
}
