//! Boolean operations over 2D contours, used by trimming and regions.
use qymcad_core::geom::{circle_contour, Contour, Point2};
use qymcad_core::offset::boolean_contours;

fn sq(s: f64) -> Contour {
    Contour::closed(vec![Point2::new(0.0, 0.0), Point2::new(s, 0.0), Point2::new(s, s), Point2::new(0.0, s)])
}

#[test]
fn subtract_circle_from_square_makes_hole() {
    let a = sq(40.0);
    let b = circle_contour(20.0, 20.0, 8.0, 0.1); // in the centre
    let res = boolean_contours(&a, &b, 0); // subtraction
    assert!(res.len() >= 2, "an outer contour and a hole, got {}", res.len());
}

#[test]
fn union_overlapping_squares() {
    let a = sq(20.0);
    let mut b = sq(20.0);
    b.translate(10.0, 10.0);
    let res = boolean_contours(&a, &b, 1); // union
    assert_eq!(res.len(), 1, "the union of two overlapping squares is one contour");
}
