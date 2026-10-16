use super::ring_drag_sign;

/// The contract: with the axis TOWARDS THE VIEWER (a negative depth component), a visually counter-clockwise
/// drag gives a positive angle (the right-hand rule); with the axis INTO THE SCREEN, the same visual
/// counter-clockwise drag gives a negative one. It used to be the other way round (the part turned against the
/// drag, and dragging to the right gave negative degrees).
#[test]
fn ccw_toward_viewer_is_positive() {
    assert_eq!(ring_drag_sign(-1.0), 1.0, "axis towards the viewer: counter-clockwise is positive");
    assert_eq!(ring_drag_sign(1.0), -1.0, "axis into the screen: counter-clockwise is negative");
}
