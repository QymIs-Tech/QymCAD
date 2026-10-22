//! A BOX OF A TOOL ON A SKETCH LAID FLAT stands by the one rule of every box: beside the shape it sizes, clear of all of
//! it, inside the view. The sheet is no view of the world, so the sketch says where the shape stands on the screen.

use qymcad_ui_state::{laid_by_the_rule, sheet_spot, By};

/// The view of the checks: a canvas of 1000 x 700 at the origin of the screen.
fn view() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))
}

/// The box of a rectangle stands beside the rectangle, not over it, and inside the view.
#[test]
fn the_box_of_a_rectangle_stands_beside_it() {
    let corners = [egui::pos2(300.0, 200.0), egui::pos2(600.0, 200.0), egui::pos2(600.0, 450.0), egui::pos2(300.0, 450.0)];
    let By::OnSheet(spot) = sheet_spot(corners[2], &corners) else { panic!("a place on a sheet is a place on the screen") };
    let size = egui::vec2(240.0, 120.0);
    let the_box = laid_by_the_rule(spot, size, view());
    let shape = egui::Rect::from_two_pos(corners[0], corners[2]);
    assert!(!the_box.intersects(shape), "the box {the_box:?} stands over the rectangle {shape:?} it sizes");
    assert!(view().contains_rect(the_box), "the box {the_box:?} stands out of the view");
}

/// A shape at the right edge of the view: the box goes to its other side rather than out of the view or over it.
#[test]
fn a_shape_at_the_edge_takes_the_box_on_its_other_side() {
    let corners = [egui::pos2(800.0, 300.0), egui::pos2(990.0, 300.0), egui::pos2(990.0, 400.0), egui::pos2(800.0, 400.0)];
    let By::OnSheet(spot) = sheet_spot(corners[1], &corners) else { panic!("a place on a sheet is a place on the screen") };
    let the_box = laid_by_the_rule(spot, egui::vec2(240.0, 120.0), view());
    let shape = egui::Rect::from_two_pos(corners[0], corners[2]);
    assert!(!the_box.intersects(shape), "the box {the_box:?} stands over the shape {shape:?} at the edge");
    assert!(view().contains_rect(the_box), "the box {the_box:?} stands out of the view");
}

/// A point alone - the centre of a circle being sized - is all the box keeps clear of.
#[test]
fn a_point_alone_is_stood_by() {
    let at = egui::pos2(500.0, 350.0);
    let By::OnSheet(spot) = sheet_spot(at, &[]) else { panic!("a place on a sheet is a place on the screen") };
    let the_box = laid_by_the_rule(spot, egui::vec2(200.0, 80.0), view());
    assert!(!the_box.contains(at), "the box {the_box:?} covers the point {at:?} it stands by");
    assert!(the_box.center().distance(at) < 250.0, "the box {the_box:?} went far from its point {at:?}");
}
