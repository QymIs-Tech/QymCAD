//! AN ARC OF AN ELLIPSE IS A CURVE OF THE SKETCH: the stretch of an ellipse from one point on it to another, its ends held
//! on the ellipse as the ends of an arc are held on its circle. With a line across its ends it closes a region a part tool
//! takes; alone it is an open chain; the whole ellipse stays a closed region of its own.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{EllipseArcSpec, Project};

/// The upper half of the ellipse of semi-axes 10 and 4 about the origin, from its right end round to its left.
fn upper_half() -> EllipseArcSpec {
    EllipseArcSpec { centre: Point2::new(0.0, 0.0), radii: Point2::new(10.0, 4.0), rot: 0.0, angles: [0.0, std::f64::consts::PI], winding: Winding::Ccw }
}

/// The closed contours of sketch `si`, as the part tools see them, by their areas.
fn closed_areas(p: &Project, si: usize) -> Vec<f64> {
    p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).map(|i| &p.contours[i]).filter(|c| c.closed).map(|c| c.area()).collect()
}

#[test]
fn half_an_ellipse_closed_by_a_line_is_a_region_of_half_its_area() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_ellipse_arc_entity(si, upper_half(), Purpose::Real);
    p.add_line_entity(si, -10.0, 0.0, 10.0, 0.0, Purpose::Real);
    let want = std::f64::consts::PI * 10.0 * 4.0 / 2.0;
    let areas = closed_areas(&p, si);
    assert_eq!(areas.len(), 1, "half an ellipse and its diameter close one region; the areas found: {areas:?}");
    assert!((areas[0] - want).abs() < 0.005 * want, "the region is {} mm^2, not the {want:.3} mm^2 of half the ellipse", areas[0]);
}

#[test]
fn an_arc_of_an_ellipse_alone_is_an_open_chain_and_a_whole_ellipse_a_region() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_ellipse_arc_entity(si, upper_half(), Purpose::Real);
    let open: Vec<usize> = p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).filter(|i| !p.contours[*i].closed).collect();
    assert_eq!(open.len(), 1, "an arc of an ellipse alone is one open chain");
    let chain = &p.contours[open[0]].points;
    let far = chain.iter().map(|q| ((q.x / 10.0).powi(2) + (q.y / 4.0).powi(2) - 1.0).abs()).fold(0.0, f64::max);
    assert!(far < 1e-9 && chain.iter().all(|q| q.y >= -1e-9), "the chain runs off the upper half of the ellipse by {far:.2e}");
    let si2 = p.new_sketch("t");
    p.add_ellipse_entity(si2, Point2::new(0.0, 0.0), 10.0, 4.0, 0.0, Purpose::Real);
    let areas = closed_areas(&p, si2);
    let want = std::f64::consts::PI * 40.0;
    assert!(areas.len() == 1 && (areas[0] - want).abs() < 0.005 * want, "a whole ellipse is one region of {want:.3} mm^2: {areas:?}");
}

#[test]
fn the_ends_of_an_arc_of_an_ellipse_stay_on_it_when_its_axis_is_dragged() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let eid = p.add_ellipse_arc_entity(si, upper_half(), Purpose::Real);
    let Some(qymcad_core::model::EllipseParts { c, ma, mi, span: Some(span) }) = p.sketches[si].entity(eid).and_then(|e| e.kind.as_ellipse()) else { panic!("no span") };
    let at = |p: &Project, id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y)).expect("the point");
    // the major end dragged out to 14: the ellipse grows along X and the ends of its arc go with it
    let k = p.sketches[si].points.iter().position(|q| q.id == ma).expect("the major end");
    p.sketches[si].points[k].x = 14.0;
    p.solve_sketch(si);
    let (pc, pa, pb) = (at(&p, c), at(&p, ma), at(&p, mi));
    assert!(pa.dist(pc) > 10.5, "the ellipse did not grow: its major semi-axis is {:.3}", pa.dist(pc));
    for end in [span.from, span.to] {
        // in the frame of the semi-axes the shares of an end sum, squared, to one
        let (d, a, b) = (at(&p, end) - pc, pa - pc, pb - pc);
        let s = (d.x * a.x + d.y * a.y) / (a.x * a.x + a.y * a.y);
        let t = (d.x * b.x + d.y * b.y) / (b.x * b.x + b.y * b.y);
        assert!((s * s + t * t - 1.0).abs() < 1e-6, "an end of the arc left the ellipse: the sum of its squared shares is {:.6}", s * s + t * t);
    }
}
