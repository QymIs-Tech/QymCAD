// A fillet or a shell that would produce a broken, self-intersecting solid is rejected, so the feature fails
// into a red node rather than returning translucent walls. Valid ones pass.
use qymcad_kernel::Shape;

fn thin_wall_box() -> Shape { // a 40 mm box minus an inner one, leaving 2 mm walls
    let outer = Shape::extrude(&[0.0,0.0, 40.0,0.0, 40.0,40.0, 0.0,40.0], 40.0).unwrap();
    let inner = Shape::extrude(&[2.0,2.0, 38.0,2.0, 38.0,38.0, 2.0,38.0], 40.0).unwrap();
    outer.boolean(&inner, 0).unwrap()
}

#[test]
fn oversize_fillet_on_thin_wall_rejected_small_ok() {
    let wall = thin_wall_box();
    assert!(wall.fillet_all(0.5).is_some(), "a small fillet of 0.5 mm on a 2 mm wall is valid");
    assert!(wall.fillet_all(5.0).is_none(), "a 5 mm fillet on a 2 mm wall gives a broken solid and is rejected");
}

#[test]
fn oversize_fillet_on_solid_rejected() {
    let box_ = Shape::extrude(&[0.0,0.0, 20.0,0.0, 20.0,20.0, 0.0,20.0], 20.0).unwrap();
    assert!(box_.fillet_all(3.0).is_some(), "a 3 mm fillet on a 20 mm cube is valid");
    assert!(box_.fillet_all(15.0).is_none(), "a 15 mm fillet, more than half the cube, self-intersects and is rejected");
}
