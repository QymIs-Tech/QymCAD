//! A RECTANGLE IS SIZED BY MOVING ITS CORNERS (`size_rect`): the width along its first side, the height along the next,
//! the corner it was drawn from - or its centre - left where it stands, and the same points and lines after it.

use qymcad_core::geom::Point2;
use qymcad_core::model::{Project, RectSize};

#[test]
fn a_rectangle_drawn_from_a_corner_keeps_that_corner() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    // drawn from (50, 80) to (10, 20): the first corner is the top right one
    let ids = p.add_rect_entity(si, 50.0, 80.0, 10.0, 20.0, qymcad_core::feature::Purpose::Real);
    let points: Vec<u64> = p.sketches[si].points.iter().map(|q| q.id).collect();
    assert!(p.size_rect(si, ids[0], RectSize { width: 25.0, height: 10.0 }), "the side is no rectangle's");
    let c = p.rect_corners(si, ids[0]).expect("the same rectangle");
    assert!((c[0].dist(c[1]) - 25.0).abs() < 1e-9 && (c[1].dist(c[2]) - 10.0).abs() < 1e-9, "sized {} by {}", c[0].dist(c[1]), c[1].dist(c[2]));
    assert!(c.iter().any(|q| q.dist(Point2::new(50.0, 80.0)) < 1e-9), "the corner it was drawn from moved: {c:?}");
    assert_eq!(p.sketches[si].points.iter().map(|q| q.id).collect::<Vec<_>>(), points, "the points of the rectangle are other ones");
}

#[test]
fn a_rectangle_drawn_from_its_centre_keeps_the_centre() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ids = p.add_rect_from_centre(si, Point2::new(10.0, 20.0), Point2::new(30.0, 50.0), qymcad_core::feature::Purpose::Real);
    assert!(p.size_rect(si, ids[0], RectSize { width: 8.0, height: 6.0 }));
    let c = p.rect_corners(si, ids[0]).expect("the same rectangle");
    let mid = Point2::new((c[0].x + c[2].x) / 2.0, (c[0].y + c[2].y) / 2.0);
    assert!(mid.dist(Point2::new(10.0, 20.0)) < 1e-9, "the centre moved to {mid:?}");
    assert!((c[0].dist(c[1]) - 8.0).abs() < 1e-9 && (c[1].dist(c[2]) - 6.0).abs() < 1e-9, "sized {} by {}", c[0].dist(c[1]), c[1].dist(c[2]));
}
