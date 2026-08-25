//! A multi-loop sketch: an outer contour with holes, as a real sketcher produces.
use qymcad_core::model::Project;

#[test]
fn rect_plus_circle_makes_two_contours() {
    let mut p = Project::default();
    let si = p.new_sketch("part");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real); // the outer contour, of four lines
    p.add_circle_entity(si, 20.0, 15.0, 5.0, qymcad_core::feature::Purpose::Real); // a hole
    // two contours are expected: the rectangular loop and the circle
    let cids = &p.sketches[si].contour_ids;
    assert_eq!(cids.len(), 2, "an outer contour plus a circle gives two contours, got {}", cids.len());
    let mut closed = 0;
    for &cid in cids {
        let c = &p.contours[p.contour_index(cid).unwrap()];
        assert!(c.closed, "both loops are closed");
        closed += 1;
    }
    assert_eq!(closed, 2);
}

#[test]
fn two_separate_loops_found() {
    let mut p = Project::default();
    let si = p.new_sketch("two loops");
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si, 20.0, 0.0, 30.0, 10.0, qymcad_core::feature::Purpose::Real); // a second one, unconnected
    assert_eq!(p.sketches[si].contour_ids.len(), 2, "two separate loops");
}

#[test]
fn construction_rect_not_a_contour() {
    let mut p = Project::default();
    let si = p.new_sketch("c");
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si, 2.0, 2.0, 8.0, 8.0, qymcad_core::feature::Purpose::Construction); // construction geometry, outside the profile
    assert_eq!(p.sketches[si].contour_ids.len(), 1, "only the profile loop becomes a contour");
}

#[test]
fn polygon_tool_makes_closed_loop() {
    let mut p = Project::default();
    let si = p.new_sketch("poly");
    p.add_polygon_entity(si, 0.0, 0.0, 10.0, 0.0, 6, qymcad_core::feature::Purpose::Real);
    assert_eq!(p.sketches[si].contour_ids.len(), 1, "a polygon is one contour");
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    assert!(c.closed && c.points.len() == 6, "a closed hexagon");
}

#[test]
fn slot_tool_makes_closed_loop() {
    let mut p = Project::default();
    let si = p.new_sketch("slot");
    p.add_slot_entity(si, 0.0, 0.0, 20.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    assert_eq!(p.sketches[si].contour_ids.len(), 1, "a slot is one contour");
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    assert!(c.closed, "the slot is closed");
    assert!(c.points.len() > 4, "a slot with arc caps: {} points", c.points.len());
}
