//! A SPLINE CUTS THE PLANE AS A LINE OR AN ARC DOES: a region bounded partly by a spline, or by a spline with itself,
//! is a closed contour of the sketch, and a part tool can take it.
//!
//! Reported behaviour: a region closed by a spline together with lines, or by a spline crossing itself, could not be
//! picked in the extrusion or any other part tool.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::{Contour, Point2};
use qymcad_core::model::Project;

/// A project with one empty sketch, and its index.
fn sketch() -> (Project, usize) {
    let mut p = Project::default();
    let _part = p.new_document();
    let sid = p.add_sketch("splines", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).expect("the sketch is there");
    (p, si)
}

/// The closed contours of the sketch, as the part tools see them.
fn closed(p: &Project, si: usize) -> Vec<Contour> {
    p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).map(|i| p.contours[i].clone()).filter(|c| c.closed).collect()
}

/// The area under a polyline down to the chord joining its ends (the shoelace of the polyline closed by that chord).
fn shoelace(pts: &[Point2]) -> f64 {
    let n = pts.len();
    ((0..n).map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y).sum::<f64>() / 2.0).abs()
}

fn p2(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

#[test]
fn a_spline_closed_by_two_lines_is_one_region_of_its_area() {
    let (mut p, si) = sketch();
    p.add_spline(si, vec![p2(0.0, 0.0), p2(5.0, 6.0), p2(10.0, 0.0)], Ends::Open, Purpose::Real);
    p.add_line_entity(si, 10.0, 0.0, 5.0, -5.0, Purpose::Real);
    p.add_line_entity(si, 5.0, -5.0, 0.0, 0.0, Purpose::Real);
    // the cap of the curve over its chord, plus the triangle under the chord: 10 x 5 / 2
    let want = shoelace(&p.spline_polyline(si, 0)) + 25.0;
    let regions = closed(&p, si);
    let areas: Vec<f64> = regions.iter().map(|c| c.area()).collect();
    assert_eq!(regions.len(), 1, "a spline and two lines close one region; the areas found: {areas:?}");
    assert!((areas[0] - want).abs() < 0.01 * want, "the region of {} mm^2 is not the {want} mm^2 the spline and the lines bound", areas[0]);
}

#[test]
fn a_spline_crossing_itself_gives_its_two_lobes() {
    let (mut p, si) = sketch();
    // a figure of eight: the second and the fourth spans cross at the origin
    p.add_spline(si, vec![p2(-10.0, 5.0), p2(-10.0, -5.0), p2(10.0, 5.0), p2(10.0, -5.0)], Ends::Closed, Purpose::Real);
    let areas: Vec<f64> = closed(&p, si).iter().map(|c| c.area()).collect();
    assert_eq!(areas.len(), 2, "a spline crossing itself gives two lobes; the areas found: {areas:?}");
    assert!((areas[0] - areas[1]).abs() < 0.02 * areas[0], "the lobes of a symmetric figure of eight are equal: {areas:?}");
}

#[test]
fn a_closed_spline_over_a_rectangle_gives_the_parts_the_two_make() {
    let (mut p, si) = sketch();
    p.add_spline(si, vec![p2(10.0, 0.0), p2(0.0, 10.0), p2(-10.0, 0.0), p2(0.0, -10.0)], Ends::Closed, Purpose::Real);
    let corners = [(-5.0, -15.0), (5.0, -15.0), (5.0, 15.0), (-5.0, 15.0)];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real);
    }
    let regions = closed(&p, si);
    let areas: Vec<f64> = regions.iter().map(|c| c.area()).collect();
    // the middle the two share, the two sides of the spline left and right, the two ends of the rectangle above and below
    assert_eq!(regions.len(), 5, "a closed spline over a rectangle makes five parts; the areas found: {areas:?}");
    let union = shoelace(&p.spline_polyline(si, 0)) + 10.0 * 30.0 - areas.iter().copied().fold(0.0, f64::max);
    let sum: f64 = areas.iter().sum();
    assert!((sum - union).abs() < 0.01 * union, "the parts cover {sum} mm^2, not the {union} mm^2 of the two together");
}
