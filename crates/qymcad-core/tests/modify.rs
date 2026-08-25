//! Modifying and replicating sketch entities.
use qymcad_core::model::Project;

fn rect_sketch() -> (Project, usize, Vec<u64>) {
    let mut p = Project::default();
    let si = p.new_sketch("r");
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let eids: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    (p, si, eids)
}

#[test]
fn delete_removes_entities_and_points() {
    let (mut p, si, eids) = rect_sketch();
    p.delete_entities(si, &eids);
    assert!(p.sketches[si].entities.is_empty(), "the entities must be deleted");
    assert!(p.sketches[si].points.is_empty(), "the orphaned points must be deleted");
    assert!(p.sketches[si].contour_ids.is_empty(), "no contour must be left");
}

#[test]
fn move_translates() {
    let (mut p, si, eids) = rect_sketch();
    p.move_entities(si, &eids, 5.0, 3.0);
    let minx = p.sketches[si].points.iter().map(|q| q.x).fold(f64::MAX, f64::min);
    assert!((minx - 5.0).abs() < 1e-9, "the shift along X is 5: {minx}");
}

#[test]
fn rotate_turns_geometry_about_center() {
    // Rotating the selected geometry by 90 degrees about (0,0) maps a point (x,y) to (-y, x).
    let (mut p, si, eids) = rect_sketch();
    let before: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect();
    p.rotate_entities(si, &eids, 0.0, 0.0, 90.0);
    let after: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect();
    for ((x0, y0), (x1, y1)) in before.iter().zip(after.iter()) {
        assert!((x1 - (-y0)).abs() < 1e-9 && (y1 - x0).abs() < 1e-9, "a 90 degree rotation about the origin: ({x0},{y0}) -> ({x1},{y1})");
    }
}

#[test]
fn mirror_doubles_entities() {
    let (mut p, si, eids) = rect_sketch();
    let before = p.sketches[si].entities.len();
    p.mirror_entities(si, &eids, 20.0, 0.0, 20.0, 10.0); // Mirror about the vertical line x = 20.
    assert_eq!(p.sketches[si].entities.len(), before * 2, "the copy must be added");
    assert_eq!(p.sketches[si].contour_ids.len(), 2, "two loops");
}

#[test]
fn linear_array_makes_copies() {
    let (mut p, si, eids) = rect_sketch();
    p.array_linear(si, &eids, 20.0, 0.0, 3); // Three in total, so two copies.
    assert_eq!(p.sketches[si].contour_ids.len(), 3, "three instances");
}

#[test]
fn circular_array_makes_copies() {
    let (mut p, si, eids) = rect_sketch();
    p.array_circular(si, &eids, 50.0, 0.0, 4, 360.0);
    assert_eq!(p.sketches[si].contour_ids.len(), 4, "four around the circle");
}

#[test]
fn fillet_rounds_corner() {
    let (mut p, si, eids) = rect_sketch();
    // Two adjacent sides of the rectangle, sharing a vertex.
    let ok = p.fillet_lines(si, eids[0], eids[1], 2.0);
    assert!(ok, "the fillet must apply");
    // The contour stays closed and grows more complex: an arc appears, so there are more points.
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    assert!(c.closed && c.points.len() > 4, "the filleted contour has {} points", c.points.len());
}

#[test]
fn chamfer_cuts_corner() {
    let (mut p, si, eids) = rect_sketch();
    let ok = p.chamfer_lines(si, eids[0], eids[1], 2.0);
    assert!(ok, "the chamfer must apply");
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    assert!(c.closed && c.points.len() == 5, "five corners after a chamfer: {}", c.points.len());
}

#[test]
fn offset_makes_inner_loop() {
    let (mut p, si, eids) = rect_sketch();
    let n = p.offset_entities(si, &eids, -2.0); // Inwards.
    assert!(n >= 1, "an offset contour must be produced");
    assert!(p.sketches[si].contour_ids.len() >= 2, "the original contour plus the offset one");
}

#[test]
fn trim_removes_middle_segment() {
    use qymcad_core::geom::Point2;
    let mut p = Project::default();
    let si = p.new_sketch("t");
    // A horizontal line from 0 to 30 along y = 0.
    p.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    // Two vertical cutting lines at x = 10 and x = 20.
    p.add_line_entity(si, 10.0, -5.0, 10.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 20.0, -5.0, 20.0, 5.0, qymcad_core::feature::Purpose::Real);
    let hid = p.sketches[si].entities[0].id; // The horizontal line.
    let n_before = p.sketches[si].entities.len();
    // Trim the middle piece, picked at x = 15.
    let ok = p.trim_line(si, hid, 15.0, 0.0);
    assert!(ok, "the trim must apply");
    // The horizontal line is replaced by two pieces (0..10 and 20..30), which is one entity more.
    assert_eq!(p.sketches[si].entities.len(), n_before + 1, "the middle piece is removed and two remain");
    let _ = Point2::new(0.0, 0.0);
}

#[test]
fn fillet_radius_change_keeps_shape() {
    let (mut p, si, eids) = rect_sketch(); // A rectangle from 0 to 10.
    p.fillet_lines(si, eids[0], eids[1], 2.0);
    // Find the arc.
    let arc = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        qymcad_core::model::EntityKind::Arc { .. } => Some(e.id),
        _ => None,
    }).unwrap();
    // Increasing the radius must not send the geometry flying.
    let ok = p.set_fillet_radius(si, arc, 4.0);
    assert!(ok, "set_fillet_radius must apply to a fillet arc");
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    let b = c.bbox().unwrap();
    assert!(b.min.x >= -0.5 && b.min.y >= -0.5 && b.max.x <= 10.5 && b.max.y <= 10.5, "the contour must stay within the 0..10 bounds: {:?}", b);
    assert!(c.closed, "the contour must stay closed");
}

#[test]
fn fillet_stays_tangent_after_dimensioning() {
    use qymcad_core::model::{Constraint, EntityKind};
    let (mut p, si, eids) = rect_sketch(); // From 0 to 10.
    p.fillet_lines(si, eids[0], eids[1], 2.0);
    // Stretch the left side to 20 mm; its corners are intact, since the filleted corner is at (10,0).
    let pts: Vec<(u64, f64, f64)> = p.sketches[si].points.iter().map(|q| (q.id, q.x, q.y)).collect();
    let bl = pts.iter().find(|(_, x, y)| x.abs() < 0.1 && y.abs() < 0.1).unwrap().0;
    let tl = pts.iter().find(|(_, x, y)| x.abs() < 0.1 && (*y - 10.0).abs() < 0.1).unwrap().0;
    p.sketches[si].constraints.push(Constraint::Fixed { p: bl });
    p.sketches[si].constraints.push(Constraint::Distance { a: bl, b: tl, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    p.solve_sketch(si);
    // The radius of the arc, found by id.
    fn arc_radius(p: &Project, si: usize, arc: u64) -> f64 {
        let (cen, a, _) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
            EntityKind::Arc { center, a, b, .. } => Some((center, a, b)),
            _ => None,
        }).unwrap();
        let _ = arc;
        let pt = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
        let (cx, cy) = pt(cen);
        let (ax, ay) = pt(a);
        ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt()
    }
    let arc_id = p.sketches[si].entities.iter().find_map(|e| match e.kind {
        EntityKind::Arc { .. } => Some(e.id),
        _ => None,
    }).unwrap();
    // The fillet arc stays valid and its radius holds at about 2 mm.
    assert!((arc_radius(&p, si, arc_id) - 2.0).abs() < 0.1, "the fillet radius must stay at about 2 mm");
    let bb = p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()].bbox().unwrap();
    assert!(bb.max.x < 60.0 && bb.max.y < 60.0, "the geometry must not fly off: {:?}", bb);
    // Change the radius geometrically, which is stable and does not let the shape creep.
    assert!(p.set_fillet_radius(si, arc_id, 4.0), "the radius must be editable");
    assert!((arc_radius(&p, si, arc_id) - 4.0).abs() < 0.1, "the radius must become 4 mm");
    let bb2 = p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()].bbox().unwrap();
    assert!(bb2.max.x < 60.0 && bb2.max.y < 60.0, "the geometry must not fly off after the radius edit: {:?}", bb2);
}

#[test]
fn ellipse_axes_editable() {
    // An ellipse is a real entity: its semi-axes are edited through the centre handle and the contour is rebuilt.
    let mut p = Project::default();
    let si = p.new_sketch("e");
    let center = p.add_ellipse_entity(si, 0.0, 0.0, 5.0, 3.0, 0.0, qymcad_core::feature::Purpose::Real);
    assert!(p.set_ellipse_axes(si, center, 10.0, 4.0), "the axes of an ellipse must be editable");
    let (a, b) = p.ellipse_axes(si, center).unwrap();
    assert!((a - 10.0).abs() < 0.05 && (b - 4.0).abs() < 0.05, "the semi-axes must be 10 and 4: {a},{b}");
    // The contour of the ellipse is rebuilt for the new semi-axes: major 10 along X, minor 4 along Y.
    let cid = p.sketches[si].contour_ids[0];
    let bb = p.contours[p.contour_index(cid).unwrap()].bbox().unwrap();
    assert!((bb.max.x - 10.0).abs() < 0.2 && (bb.max.y - 4.0).abs() < 0.2, "the contour must follow the new semi-axes: {:?}", bb);
}

#[test]
fn delete_ellipse_entity_removes_it() {
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("e");
    p.add_ellipse_entity(si, 0.0, 0.0, 5.0, 3.0, 0.0, qymcad_core::feature::Purpose::Real);
    let eid = p.sketches[si].entities.iter().find_map(|e| matches!(e.kind, EntityKind::Ellipse { .. }).then_some(e.id)).unwrap();
    assert_eq!(p.sketches[si].contour_ids.len(), 1, "the ellipse must have a contour");
    p.delete_entities(si, &[eid]);
    assert!(!p.sketches[si].entities.iter().any(|e| matches!(e.kind, EntityKind::Ellipse { .. })), "the ellipse entity must be deleted");
    assert!(p.sketches[si].contour_ids.is_empty(), "the contour must be removed");
}

#[test]
fn extend_reaches_crossing() {
    let mut p = Project::default();
    let si = p.new_sketch("e");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real); // A short horizontal line.
    p.add_line_entity(si, 20.0, -5.0, 20.0, 5.0, qymcad_core::feature::Purpose::Real); // A crossing line further out.
    let hid = p.sketches[si].entities[0].id;
    assert!(p.extend_line(si, hid, 9.0, 0.0), "the extend must apply");
    // The right end reaches x = 20.
    let maxx = p.sketches[si].points.iter().map(|q| q.x).fold(f64::MIN, f64::max);
    assert!((maxx - 20.0).abs() < 1e-6, "it must extend to the crossing at x = 20: {maxx}");
}

#[test]
fn break_splits_in_two() {
    let mut p = Project::default();
    let si = p.new_sketch("b");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let id = p.sketches[si].entities[0].id;
    assert!(p.break_line(si, id, 5.0, 0.0), "the break must apply");
    let lines = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).count();
    assert_eq!(lines, 2, "the segment must be split in two");
}

#[test]
fn spline_makes_smooth_contour() {
    use qymcad_core::geom::Point2;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_spline(si, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 10.0), Point2::new(20.0, 0.0), Point2::new(30.0, 10.0)], qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::Real);
    assert_eq!(p.sketches[si].contour_ids.len(), 1, "a spline gives one contour");
    let c = &p.contours[p.contour_index(p.sketches[si].contour_ids[0]).unwrap()];
    assert!(c.points.len() > 10, "the curve must be sampled smoothly: {} points", c.points.len());
}

#[test]
fn merge_close_points_stitches_split_corner() {
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    // Two segments whose ends almost coincide, 0.05 apart: a corner that has come apart.
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.02, 0.03, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let before = p.sketches[si].points.len();
    let n = p.merge_close_points(si, 0.2);
    assert_eq!(n, 1, "one pair of close ends must be merged");
    assert_eq!(p.sketches[si].points.len(), before - 1);
    // Both lines now share a vertex at (10,0).
    let shared: Vec<u64> = p.sketches[si].entities.iter().filter_map(|e| match e.kind {
        EntityKind::Line { a, b } => Some(vec![a, b]),
        _ => None,
    }).flatten().collect();
    // The shared vertex has degree two, belonging to both lines.
    let v10 = p.sketches[si].points.iter().find(|q| (q.x - 10.0).abs() < 0.1 && q.y.abs() < 0.1).unwrap().id;
    assert_eq!(shared.iter().filter(|&&id| id == v10).count(), 2, "the corner must become shared");
}

#[test]
fn deleting_edge_drops_constraints_on_it() {
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("r");
    // A rectangle of four lines.
    p.add_line_entity(si, 0.0, 0.0, 50.0, 0.0, qymcad_core::feature::Purpose::Real);   // Bottom.
    p.add_line_entity(si, 50.0, 0.0, 50.0, 30.0, qymcad_core::feature::Purpose::Real); // Right.
    p.add_line_entity(si, 50.0, 30.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real); // Top.
    p.add_line_entity(si, 0.0, 30.0, 0.0, 0.0, qymcad_core::feature::Purpose::Real);   // Left.
    p.merge_close_points(si, 0.1);
    let id_at = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x-x).abs()<0.1 && (q.y-y).abs()<0.1).unwrap().id;
    let bl = id_at(&p, 0.0, 0.0); let br = id_at(&p, 50.0, 0.0);
    let tr = id_at(&p, 50.0, 30.0); let tl = id_at(&p, 0.0, 30.0);
    // An equality between the right edge (br-tr) and the left one (tl-bl).
    p.sketches[si].constraints.push(Constraint::Equal { a: br, b: tr, c: tl, d: bl });
    // Find and delete the right edge (br-tr).
    let right = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line{a,b} if (a==br&&b==tr)||(a==tr&&b==br))).unwrap().id;
    p.delete_entities(si, &[right]);
    // The equality that hung on the right edge must go; the points br and tr survive, being shared.
    assert!(p.sketches[si].points.iter().any(|q| q.id == br), "point br must survive, being shared with the bottom");
    assert!(!p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Equal{..})), "the equality on the deleted edge must be removed");
}

#[test]
fn parametric_polygon_rebuilds_on_radius() {
    // A parametric regular polygon is a circumscribed circle plus constraints. The radius is edited through the
    // dimension of that circle and the solver rebuilds the shape: the vertices stay on the circle and the sides
    // stay equal.
    let mut p = Project::default();
    let si = p.new_sketch("poly");
    let (center, sides) = p.add_polygon_param(si, 0.0, 0.0, 10.0, 0.0, 6, qymcad_core::feature::Purpose::Real);
    assert_eq!(sides.len(), 6, "six sides");
    let r0 = p.polygon_circle(si, center).map(|(_, _, r)| r).unwrap();
    assert!((r0 - 10.0).abs() < 0.1, "the circumscribed circle has r = 10, got {r0}");
    // Changing the radius through the dimension makes the solver rebuild the polygon.
    assert!(p.set_polygon_radius(si, center, 25.0), "the radius must be editable");
    let r1 = p.polygon_circle(si, center).map(|(_, _, r)| r).unwrap();
    assert!((r1 - 25.0).abs() < 0.1, "the radius must become 25, got {r1}");
    // The contour is rebuilt for the new radius, with the first vertex at (25,0) and an angle of zero.
    let cid = p.sketches[si].contour_ids[0];
    let bb = p.contours[p.contour_index(cid).unwrap()].bbox().unwrap();
    assert!((bb.max.x - 25.0).abs() < 0.5, "it must be rebuilt for a radius of 25: {:?}", bb);
}

#[test]
fn parametric_fillet_is_tangent_and_resizes_via_solver() {
    // A fillet is parametric: the arc is held by tangency to both walls plus a radius dimension. Checked here are
    // tangency right after creation, and that editing the radius through the dimension, that is through the solver,
    // preserves tangency at the new radius. Editing it used to blow the shape apart.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("fil");
    p.add_line_entity(si, 0.0, 0.0, 12.0, 0.0, qymcad_core::feature::Purpose::Real); // Horizontal.
    p.add_line_entity(si, 0.0, 0.0, 0.0, 12.0, qymcad_core::feature::Purpose::Real); // Vertical, sharing the corner at (0,0).
    let e1 = p.sketches[si].entities[0].id;
    let e2 = p.sketches[si].entities[1].id;
    assert!(p.fillet_lines(si, e1, e2, 2.0), "the fillet must apply");

    // For every tangency constraint, check that the distance from the centre to the line equals the arc radius.
    fn check_tangent(p: &Project, si: usize, want_r: f64) {
        let pt = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
        let (cen, a) = p.sketches[si].entities.iter().find_map(|e| match e.kind {
            EntityKind::Arc { center, a, .. } => Some((center, a)), _ => None }).unwrap();
        let (cx, cy) = pt(cen);
        let (ax, ay) = pt(a);
        let r = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        assert!((r - want_r).abs() < 0.05, "the arc radius must be about {want_r}, got {r}");
        let mut n = 0;
        for c in &p.sketches[si].constraints {
            if let Constraint::Tangent { a: la, b: lb, c: cc, .. } = *c {
                let ((lax, lay), (lbx, lby)) = (pt(la), pt(lb));
                let (dx, dy) = (lbx - lax, lby - lay);
                let len = (dx * dx + dy * dy).sqrt();
                let d = ((dx * (cy - lay) - dy * (cx - lax)) / len).abs();
                assert!((d - r).abs() < 0.05, "the wall must be tangent: dist={d}, r={r}");
                let _ = cc; n += 1;
            }
        }
        assert_eq!(n, 2, "two tangency constraints, one per wall");
    }
    p.solve_sketch(si);
    check_tangent(&p, si, 2.0);

    // Editing the radius through the dimension: the solver rebuilds and tangency holds.
    let arc = p.sketches[si].entities.iter().find_map(|e| matches!(e.kind, EntityKind::Arc { .. }).then_some(e.id)).unwrap();
    assert!(p.set_fillet_radius_dim(si, arc, 4.0), "the radius must be editable through the dimension");
    check_tangent(&p, si, 4.0);
}

#[test]
fn trim_circle_into_arc() {
    // A circle of r = 10 at (0,0) with a vertical cutting line at x = 0 gives two intersections, top and bottom.
    // A pick on the right (x > 0) removes the right arc and leaves the left one, so the circle becomes an arc.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("t");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, -15.0, 0.0, 15.0, qymcad_core::feature::Purpose::Real); // The cutting line along the Y axis.
    let cid = p.sketches[si].entities.iter().find_map(|e| matches!(e.kind, EntityKind::Circle { .. }).then_some(e.id)).unwrap();
    assert!(p.trim_curve(si, cid, 10.0, 0.0), "the trim must apply for a pick on the right");
    // The circle is gone and arcs have appeared.
    assert!(!p.sketches[si].entities.iter().any(|e| matches!(e.kind, EntityKind::Circle { .. })), "the circle must be removed");
    let arcs = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    assert!(arcs >= 1, "at least one arc must appear: {arcs}");
    // The remaining geometry lies in the left half-plane, with the midpoint of the arc at x <= 0.
    let pt = |id: u64| { let q = p.sketches[si].points.iter().find(|q| q.id == id).unwrap(); (q.x, q.y) };
    for e in &p.sketches[si].entities {
        if let EntityKind::Arc { center, a, b, ccw } = e.kind {
            let (cx, cy) = pt(center);
            let (ax, ay) = pt(a); let (bx, by) = pt(b);
            // The midpoint of the arc along its winding: the angle g0 plus half the sweep.
            let g0 = (ay - cy).atan2(ax - cx);
            let g1 = (by - cy).atan2(bx - cx);
            let sweep = if ccw { (g1 - g0).rem_euclid(std::f64::consts::TAU) } else { (g0 - g1).rem_euclid(std::f64::consts::TAU) };
            let mid = if ccw { g0 + sweep / 2.0 } else { g0 - sweep / 2.0 };
            let mx = cx + 10.0 * mid.cos();
            assert!(mx < 1.0, "the remaining arc must lie on the left, midpoint x = {mx}");
        }
    }
}

#[test]
fn trim_arc_shortens_it() {
    // A semicircular arc, the upper one from 0 to 180 degrees with r = 10; the cutting line at x = 0 meets it at
    // (0,10). A pick near 135 degrees removes the left part and leaves the right one, from 0 to about 90.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("ta");
    // An arc from (10,0) to (-10,0) counter-clockwise, the upper semicircle.
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, -10.0, 0.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, -15.0, 0.0, 15.0, qymcad_core::feature::Purpose::Real); // The cutting line x = 0 meets the arc at (0,10).
    let aid = p.sketches[si].entities.iter().find_map(|e| matches!(e.kind, EntityKind::Arc { .. }).then_some(e.id)).unwrap();
    let n_before = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    // A pick at the upper left, near 135 degrees, removes the left piece.
    let ok = p.trim_curve(si, aid, -7.0, 7.0);
    assert!(ok, "trimming the arc must apply");
    let n_after = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    assert_eq!(n_before, 1);
    assert!(n_after >= 1, "an arc must remain");
}

#[test]
fn offset_circle_stays_circle() {
    // Offsetting a circle of r = 10 outwards by 3 gives a circle of r = 13, not a polygon.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("oc");
    p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let cid = p.sketches[si].entities[0].id;
    let n = p.offset_entities(si, &[cid], 3.0);
    assert_eq!(n, 1, "one offset contour");
    // A new circle entity of about r = 13 must appear, giving two circles in all.
    let circles: Vec<f64> = p.sketches[si].entities.iter().filter_map(|e| match e.kind { EntityKind::Circle { r, .. } => Some(r), _ => None }).collect();
    assert_eq!(circles.len(), 2, "the original and the offset circle: {circles:?}");
    assert!(circles.iter().any(|r| (r - 13.0).abs() < 0.05), "a circle of about r = 13 must appear: {circles:?}");
    // No polygon lines appear, so the circle is not flattened.
    assert!(!p.sketches[si].entities.iter().any(|e| matches!(e.kind, EntityKind::Line { .. })), "it must not turn into a polygon");
}

#[test]
fn offset_loop_with_arc_keeps_arcs() {
    // A loop of lines and arcs, a rounded rectangle: after an outward offset the arcs stay arcs.
    use qymcad_core::model::EntityKind;
    let (mut p, si, eids) = rect_sketch(); // From 0 to 10.
    // Round one corner, which puts an arc into the loop.
    p.fillet_lines(si, eids[0], eids[1], 2.0);
    let loop_eids: Vec<u64> = p.sketches[si].entities.iter().filter(|e| !e.construction).map(|e| e.id).collect();
    let arcs_before_offset = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    assert_eq!(arcs_before_offset, 1, "one fillet arc in the loop");
    let n = p.offset_entities(si, &loop_eids, 1.0);
    assert!(n >= 1, "an offset must be produced");
    // The newly added entities include an arc, so the offset preserved the curvature instead of flattening it.
    let total_arcs = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    assert!(total_arcs >= 2, "the offset must carry an arc too, preserving the curve: arcs={total_arcs}");
}

#[test]
fn spline_tessellation_is_adaptive() {
    // Tessellation is adaptive: a nearly straight spline yields few points and a curved one yields many, since
    // the density follows the curvature instead of a fixed 14 steps. The knots remain solver points.
    use qymcad_core::geom::Point2;
    let count = |pts: Vec<Point2>| {
        let mut p = Project::default();
        let si = p.new_sketch("s");
        p.add_spline(si, pts, qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::Real);
        let cid = p.sketches[si].contour_ids[0];
        p.contours[p.contour_index(cid).unwrap()].points.len()
    };
    // Nearly straight: four knots on a line with a slight deviation.
    let straight = count(vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.1), Point2::new(20.0, 0.0), Point2::new(30.0, 0.1)]);
    // The same span, but a steep zigzag.
    let curvy = count(vec![Point2::new(0.0, 0.0), Point2::new(10.0, 12.0), Point2::new(20.0, -12.0), Point2::new(30.0, 0.0)]);
    assert!(curvy > straight, "a curved spline must be sampled more densely: curvy={curvy}, straight={straight}");
    assert!(straight < 30, "a nearly straight spline must not be oversampled: {straight} points");
}

#[test]
fn spline_tangent_handle_changes_shape() {
    // Dragging the tangent handle of a knot changes the shape of the spline, which is a fit-point spline with
    // handles. The tangent is automatic to begin with (Catmull-Rom); `set_spline_handle` makes it explicit.
    use qymcad_core::geom::Point2;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_spline(si, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(20.0, 0.0)], qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::Real);
    // There is one handle per knot.
    let h = p.spline_handles(si, 0);
    assert_eq!(h.len(), 3, "one handle per knot");
    // The height of the contour starts near zero, since the knots lie on a line.
    let cid = p.sketches[si].contour_ids[0];
    let h0 = p.contours[p.contour_index(cid).unwrap()].points.iter().map(|q| q.y.abs()).fold(0.0, f64::max);
    assert!(h0 < 0.5, "the spline must start flat, max|y| = {h0}");
    // Pull the handle of the middle knot sharply upwards, which bows the curve.
    assert!(p.set_spline_handle(si, 0, 1, 10.0, 15.0), "the handle must be set");
    let h1 = p.contours[p.contour_index(cid).unwrap()].points.iter().map(|q| q.y).fold(f64::MIN, f64::max);
    assert!(h1 > 1.0, "the tangent handle must bow the spline upwards, max y = {h1}");
    // Resetting to automatic returns it to flat.
    assert!(p.reset_spline_handle(si, 0, 1));
    let h2 = p.contours[p.contour_index(cid).unwrap()].points.iter().map(|q| q.y.abs()).fold(0.0, f64::max);
    assert!(h2 < 0.5, "resetting the handle must make it flat again, max|y| = {h2}");
}

#[test]
fn fillet_has_single_radius_dim_no_linear() {
    // A fillet attaches exactly one radius dimension (a `Diameter` in radius mode) and no extra linear callout
    // for the distance to the centre; a `Distance` used to be added as well.
    use qymcad_core::model::Constraint;
    let (mut p, si, eids) = rect_sketch();
    p.fillet_lines(si, eids[0], eids[1], 3.0);
    let rdims = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Diameter { diam: false, .. })).count();
    let dists = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Distance { .. })).count();
    assert_eq!(rdims, 1, "exactly one radius dimension");
    assert_eq!(dists, 0, "no extra linear dimensions to the centre");
}

#[test]
fn fixed_point_resists_drag() {
    // A fixed point is rigid: even a strong drag residual (weight 5) does not move it, because `Fixed` carries a
    // weight of 50.
    use qymcad_core::model::Constraint;
    let mut p = Project::default();
    let si = p.new_sketch("f");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let a = p.sketch_point_at(si, 0.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: a });
    // Attempt to drag the fixed point to (20,20).
    p.solve_sketch_drag(si, Some((a, 20.0, 20.0)));
    let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
    assert!(pa.x.abs() < 0.5 && pa.y.abs() < 0.5, "a fixed point must stay at (0,0): ({},{})", pa.x, pa.y);
}

#[test]
fn trim_split_pieces_stay_on_line() {
    // The pieces of a split line stay on one straight line: the inner cut points are held by `PointOnLine` against
    // the outer ones, so an edit or a drag cannot move them off it. Without this the pieces drifted apart.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("c");
    p.add_line_entity(si, -10.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real); // The cutting circle.
    let lid = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line { .. })).unwrap().id;
    let la = p.sketch_point_at(si, -10.0, 0.0, 1e-6);
    let lb = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    p.sketches[si].constraints.push(Constraint::Fixed { p: la });
    p.sketches[si].constraints.push(Constraint::Fixed { p: lb });
    p.trim_line(si, lid, 0.0, 0.0); // Two segments; the inner ends at (±5,0) are held on the line la-lb.
    assert_eq!(p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).count(), 2, "two segments");
    let onl = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::PointOnLine { .. })).count();
    assert_eq!(onl, 2, "both inner cut points must be held on the line");
    // Perturb the cut end near (5,0) upwards and solve: it must return to the line at y near zero.
    let cut = p.sketches[si].points.iter().find(|q| (q.x - 5.0).abs() < 0.1 && q.y.abs() < 0.1).map(|q| q.id).unwrap();
    p.sketches[si].points.iter_mut().find(|q| q.id == cut).unwrap().y += 5.0;
    p.solve_sketch(si);
    let y = p.sketches[si].points.iter().find(|q| q.id == cut).unwrap().y;
    assert!(y.abs() < 0.2, "the cut end must return to the line at y near zero, got {y}");
}

#[test]
fn trim_both_orders_make_notch() {
    // Trimming works in both orders, circle then line and line then circle, giving an arc and two pieces of the
    // line.
    use qymcad_core::model::EntityKind;
    let build = || {
        let mut p = Project::default();
        let si = p.new_sketch("n");
        p.add_line_entity(si, -10.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
        p.add_circle_entity(si, 0.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
        (p, si)
    };
    let counts = |p: &Project, si: usize| {
        let a = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
        let l = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).count();
        (a, l)
    };
    // Order one: the circle, then the line.
    let (mut p, si) = build();
    let cid = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Circle { .. })).unwrap().id;
    let lid = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line { .. })).unwrap().id;
    assert!(p.trim_curve(si, cid, 0.0, 5.0), "the circle must be trimmed");
    p.trim_line(si, lid, 0.0, 0.0);
    assert_eq!(counts(&p, si), (1, 2), "circle then line gives one arc and two pieces");
    // Order two: the line, then the circle.
    let (mut p, si) = build();
    let cid = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Circle { .. })).unwrap().id;
    let lid = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line { .. })).unwrap().id;
    p.trim_line(si, lid, 0.0, 0.0);
    assert!(p.trim_curve(si, cid, 0.0, 5.0), "the circle must be trimmed after the line");
    assert_eq!(counts(&p, si), (1, 2), "line then circle gives one arc and two pieces");
}

#[test]
fn slot_stays_parametric() {
    // A slot carries constraints — equal end radii plus tangencies — and holds its shape under the solver, so a
    // dimension on one end pulls the other along.
    use qymcad_core::model::{Constraint, EntityKind};
    let mut p = Project::default();
    let si = p.new_sketch("slot");
    p.add_slot_entity(si, 0.0, 0.0, 20.0, 0.0, 5.0, qymcad_core::feature::Purpose::Real);
    let n_eq = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::EqualRadius { .. })).count();
    let n_tan = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Tangent { .. })).count();
    assert_eq!(n_eq, 1, "the end radii must be equal");
    assert_eq!(n_tan, 4, "four tangencies of the flanks to the ends");
    let centers: Vec<u64> = p.sketches[si].entities.iter().filter_map(|e| if let EntityKind::Arc { center, .. } = e.kind { Some(center) } else { None }).collect();
    assert_eq!(centers.len(), 2, "two end arcs");
    let (c1, c2) = (centers[0], centers[1]);
    // A radius of 8 on one end; the other must follow.
    p.sketches[si].constraints.push(Constraint::Diameter { c: c1, d: 8.0, off: 0.0, expr: String::new(), driven: false, diam: false });
    p.solve_sketch(si);
    let rad = |cid: u64| -> f64 {
        let c = p.sketches[si].points.iter().find(|q| q.id == cid).unwrap();
        let a = p.sketches[si].entities.iter().find_map(|e| if let EntityKind::Arc { center, a, .. } = e.kind { (center == cid).then_some(a) } else { None }).unwrap();
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt()
    };
    assert!((rad(c1) - 8.0).abs() < 2e-2, "end 1 must be R8: {}", rad(c1));
    assert!((rad(c2) - 8.0).abs() < 2e-2, "end 2 must follow to R8: {}", rad(c2));
}

#[test]
fn rect3_is_a_rectangle() {
    // A rotated rectangle from three points: four sides, the right perimeter and square corners.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("r3");
    let ids = p.add_rect3_entity(si, 0.0, 0.0, 6.0, 8.0, -8.0, 6.0, qymcad_core::feature::Purpose::Real); // Side (0,0)-(6,8) of length 10, height 10.
    assert_eq!(ids.len(), 4, "four sides");
    let mut per = 0.0;
    for e in &p.sketches[si].entities {
        if let EntityKind::Line { a, b } = e.kind {
            let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
            let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
            per += ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
        }
    }
    assert!((per - 40.0).abs() < 1e-6, "the perimeter is 2 * (10 + 10): {per}");
}

#[test]
fn sketch_text_is_editable_object() {
    // Text is a parametric object: it moves, it is re-baked, it is deleted, and its glyphs become contours for the
    // profile and for CAM.
    use qymcad_core::geom::Point2;
    let mut p = Project::default();
    let si = p.new_sketch("t");
    // Stand in for the glyphs the application bakes: two closed contours, the letters.
    let g0 = vec![Point2::new(0.0, 0.0), Point2::new(2.0, 0.0), Point2::new(2.0, 5.0), Point2::new(0.0, 5.0)];
    let g1 = vec![Point2::new(3.0, 0.0), Point2::new(5.0, 0.0), Point2::new(5.0, 5.0)];
    let id = p.add_sketch_text(si, 0.0, 0.0, 5.0, 0.0, "AB".into(), qymcad_core::feature::Purpose::Real, vec![g0, g1]);
    assert!(id != 0 && p.sketches[si].texts.len() == 1, "the text object must be created");
    let contours_before = p.contours.len();
    assert!(contours_before >= 2, "the glyphs must become contours for the profile and CAM: {contours_before}");
    // A move shifts both the parameters and the glyphs.
    p.move_sketch_text(si, 0, 10.0, 4.0);
    let t = &p.sketches[si].texts[0];
    assert!((t.x - 10.0).abs() < 1e-9 && (t.y - 4.0).abs() < 1e-9, "the position must move");
    assert!((t.glyphs[0][0].x - 10.0).abs() < 1e-9 && (t.glyphs[0][0].y - 4.0).abs() < 1e-9, "the glyphs must move");
    // Editing the string or the height replaces the glyphs.
    let newg = vec![vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), Point2::new(1.0, 9.0)]];
    p.set_sketch_text(si, 0, 10.0, 4.0, 9.0, 0.0, "C".into(), newg);
    assert_eq!(p.sketches[si].texts[0].text, "C");
    assert!((p.sketches[si].texts[0].height - 9.0).abs() < 1e-9, "the height must be updated");
    // Deleting removes the object together with its contours.
    p.delete_sketch_text(si, 0);
    assert!(p.sketches[si].texts.is_empty(), "the text must be deleted");
}

#[test]
fn trim_uses_construction_as_boundary() {
    // Construction geometry is a valid trimming boundary.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("c");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 5.0, -5.0, 5.0, 5.0, qymcad_core::feature::Purpose::Construction); // A construction vertical at x = 5.
    let eid = p.sketches[si].entities.iter().find(|e| !e.construction).unwrap().id;
    assert!(p.trim_line(si, eid, 8.0, 0.0), "a trim against a construction boundary must work");
    // The left half (0,0)-(5,0) remains: exactly one non-construction line of length 5.
    let lines: Vec<_> = p.sketches[si].entities.iter().filter(|e| !e.construction && matches!(e.kind, EntityKind::Line { .. })).collect();
    assert_eq!(lines.len(), 1, "one line must remain");
    if let EntityKind::Line { a, b } = lines[0].kind {
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
        let len = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
        assert!((len - 5.0).abs() < 1e-6, "the remaining half has length 5: {len}");
    }
}

#[test]
fn extend_curve_reaches_boundary_line() {
    // Extending an arc along its circle up to the intersection with a boundary line.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("e");
    // A quarter arc of r = 10 from (10,0) at 0 degrees counter-clockwise to (0,10) at 90 degrees.
    p.add_arc_entity(si, 0.0, 0.0, 10.0, 0.0, 0.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    // The boundary is a segment along y = 0 with x in [-15,-5], meeting the circle at (-10,0), 180 degrees.
    p.add_line_entity(si, -15.0, 0.0, -5.0, 0.0, qymcad_core::feature::Purpose::Real);
    let arc_eid = p.sketches[si].entities.iter().find_map(|e| if matches!(e.kind, EntityKind::Arc { .. }) { Some(e.id) } else { None }).unwrap();
    // Pull end b, near (0,10).
    assert!(p.extend_curve(si, arc_eid, 0.0, 10.0), "the arc must be extended");
    let b = p.sketches[si].entities.iter().find_map(|e| if let EntityKind::Arc { b, .. } = e.kind { Some(b) } else { None }).unwrap();
    let pb = p.sketches[si].points.iter().find(|q| q.id == b).unwrap();
    assert!((pb.x + 10.0).abs() < 1e-2 && pb.y.abs() < 1e-2, "the end of the arc must reach (-10,0): ({}, {})", pb.x, pb.y);
}

#[test]
fn copy_and_move_entities_work() {
    // Interactive move and copy by a vector.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("cm");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    let eid = p.sketches[si].entities[0].id;
    let n0 = p.sketches[si].entities.len();
    // A copy offset by (0,5).
    let ids = p.copy_entities(si, &[eid], 0.0, 5.0);
    assert_eq!(ids.len(), 1, "one copy");
    assert_eq!(p.sketches[si].entities.len(), n0 + 1, "the entity count must grow");
    if let Some(EntityKind::Line { a, .. }) = p.sketches[si].entities.iter().find(|e| e.id == ids[0]).map(|e| e.kind) {
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        assert!((pa.y - 5.0).abs() < 1e-9, "the copy must be offset by 5 along y: {}", pa.y);
    } else {
        panic!("the copy is not a line");
    }
    // Move the original by (3,0).
    p.move_entities(si, &[eid], 3.0, 0.0);
    if let EntityKind::Line { a, .. } = p.sketches[si].entities.iter().find(|e| e.id == eid).unwrap().kind {
        let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
        assert!((pa.x - 3.0).abs() < 1e-9, "the original must move by 3 along x: {}", pa.x);
    }
}

#[test]
fn fillet_curves_line_line() {
    // A general line-to-line fillet: an arc of r = 2 in the corner of the axes, centred at (2,2), touching at
    // (2,0) and (0,2).
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("ff");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let e1 = p.sketches[si].entities[0].id;
    let e2 = p.sketches[si].entities[1].id;
    assert!(p.fillet_curves(si, e1, e2, 2.0, 1.0, 1.0), "a line-to-line fillet must be built");
    let (center, a) = p.sketches[si].entities.iter().find_map(|e| if let EntityKind::Arc { center, a, .. } = e.kind { Some((center, a)) } else { None }).expect("the fillet arc must exist");
    let c = p.sketches[si].points.iter().find(|q| q.id == center).unwrap();
    assert!((c.x - 2.0).abs() < 1e-2 && (c.y - 2.0).abs() < 1e-2, "the centre must be (2,2): ({}, {})", c.x, c.y);
    let pa = p.sketches[si].points.iter().find(|q| q.id == a).unwrap();
    let rr = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
    assert!((rr - 2.0).abs() < 1e-2, "the radius must be 2: {rr}");
}

#[test]
fn fillet_curves_line_arc_tangent() {
    // A line-to-arc fillet is tangent to the line, its centre at distance r, and to the arc, the distance between
    // centres being R ± r. The line runs along y = 0 and the arc lies on a circle centred at (5,5) with R = sqrt(50),
    // sharing the vertex (0,0).
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("fa");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_arc_entity(si, 5.0, 5.0, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
    let e1 = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Line { .. })).unwrap().id;
    let e2 = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Arc { .. })).unwrap().id;
    let r = 1.5;
    assert!(p.fillet_curves(si, e1, e2, r, 1.0, 1.0), "a line-to-arc fillet must be built");
    // The new fillet is the arc whose centre is not (5,5).
    let rad_big = (50.0_f64).sqrt();
    let fillet_cen = p.sketches[si].entities.iter().filter_map(|e| if let EntityKind::Arc { center, .. } = e.kind { Some(center) } else { None }).find_map(|cid| {
        let c = p.sketches[si].points.iter().find(|q| q.id == cid).unwrap();
        (((c.x - 5.0).powi(2) + (c.y - 5.0).powi(2)).sqrt() > 1.0).then_some((c.x, c.y))
    }).expect("the fillet arc must exist");
    assert!((fillet_cen.1.abs() - r).abs() < 5e-2, "tangency to the line y = 0 means |cy| = r: cy={}", fillet_cen.1);
    let d = ((fillet_cen.0 - 5.0).powi(2) + (fillet_cen.1 - 5.0).powi(2)).sqrt();
    let tang = (d - (rad_big + r)).abs() < 5e-2 || (d - (rad_big - r)).abs() < 5e-2;
    assert!(tang, "tangency to the arc: dist={} against R ± r ({} ± {})", d, rad_big, r);
}

#[test]
fn fillet_all_corners_rounds_rectangle() {
    // Filleting all four corners of a rectangle gives four arcs.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("rc");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
    let n = p.fillet_all_corners(si, 2.0);
    assert_eq!(n, 4, "all four corners must be filleted");
    let arcs = p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count();
    assert_eq!(arcs, 4, "four fillet arcs");
}

#[test]
fn fillet_at_vertex_picks_corner_edges() {
    // Picking a corner: a vertex with two edges is filleted.
    use qymcad_core::model::EntityKind;
    let mut p = Project::default();
    let si = p.new_sketch("cv");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    let pc = p.sketch_point_at(si, 10.0, 0.0, 1e-6);
    assert!(p.fillet_at_vertex(si, pc, 3.0), "the corner must be filleted from its vertex");
    assert_eq!(p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count(), 1);
}

#[test]
fn pattern_is_editable() {
    // A pattern is an editable feature: count = 3 gives two copies, editing it to 4 gives three, and deleting it
    // cleans up.
    use qymcad_core::model::{EntityKind, PatternKind};
    let mut p = Project::default();
    let si = p.new_sketch("pat");
    p.add_circle_entity(si, 0.0, 0.0, 1.0, qymcad_core::feature::Purpose::Real);
    let src = p.sketches[si].entities[0].id;
    let n0 = p.sketches[si].entities.len();
    let pid = p.add_pattern(si, &[src], PatternKind::Linear { dx: 5.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 0.0, count2: 1 });
    let pi = p.sketches[si].patterns.iter().position(|q| q.id == pid).unwrap();
    assert_eq!(p.sketches[si].entities.len(), n0 + 2, "three instances means two copies");
    // Editing the count to 4 gives three copies.
    p.update_pattern(si, pi, PatternKind::Linear { dx: 5.0, dy: 0.0, count: 4, dx2: 0.0, dy2: 0.0, count2: 1 });
    assert_eq!(p.sketches[si].entities.len(), n0 + 3, "after editing the count to 4 there must be three copies");
    // The copies sit at x = 5, 10 and 15.
    let xs: Vec<f64> = p.sketches[si].entities.iter().filter_map(|e| if let EntityKind::Circle { center, .. } = e.kind { p.sketches[si].points.iter().find(|q| q.id == center).map(|q| q.x) } else { None }).collect();
    assert!(xs.iter().any(|x| (x - 15.0).abs() < 1e-6), "a copy must sit at x = 15: {xs:?}");
    // Deleting the pattern removes the copies and leaves the source.
    p.delete_pattern(si, pi);
    assert_eq!(p.sketches[si].entities.len(), n0, "only the source must remain after deletion");
}

#[test]
fn linear_pattern_2d_grid() {
    // A linear grid pattern of rows by columns: count = 3 by count2 = 2 gives six instances, that is five
    // copies.
    use qymcad_core::model::{EntityKind, PatternKind};
    let mut p = Project::default();
    let si = p.new_sketch("grid");
    p.add_circle_entity(si, 0.0, 0.0, 1.0, qymcad_core::feature::Purpose::Real);
    let src = p.sketches[si].entities[0].id;
    let n0 = p.sketches[si].entities.len();
    p.add_pattern(si, &[src], PatternKind::Linear { dx: 5.0, dy: 0.0, count: 3, dx2: 0.0, dy2: 4.0, count2: 2 });
    assert_eq!(p.sketches[si].entities.len(), n0 + 5, "a 3 by 2 grid gives six instances, five copies");
    // There is a copy at the far corner of the grid, (10,4).
    let has = |x: f64, y: f64| p.sketches[si].entities.iter().any(|e| matches!(e.kind, EntityKind::Circle { center, .. } if p.sketches[si].points.iter().any(|q| q.id == center && (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6)));
    assert!(has(10.0, 4.0), "a copy must sit at the far corner of the grid, (10,4)");
    assert!(has(0.0, 4.0), "a copy must sit in the second row, (0,4)");
}

#[test]
fn region_inner_contour_is_hole() {
    // A contour with another contour inside it: the outer region takes the inner one as a hole.
    let mut p = Project::default();
    let si = p.new_sketch("plate");
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real); // The outer one.
    p.add_rect_entity(si, 10.0, 10.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real); // The inner one, the hole.
    p.regen_sketch(si);
    let sid = p.sketches[si].id;
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert_eq!(closed.len(), 2, "two closed contours");
    // The outer contour is the one with the larger span in X.
    let span = |cid: u64| { let xy = p.contour_profile_xy(cid).unwrap(); let xs: Vec<f64> = xy.iter().step_by(2).copied().collect(); xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min) };
    let (outer, inner) = if span(closed[0]) >= span(closed[1]) { (closed[0], closed[1]) } else { (closed[1], closed[0]) };
    assert_eq!(p.feature_holes(sid, outer), vec![inner], "the inner contour is a hole of the outer one");
    assert!(p.feature_holes(sid, inner).is_empty(), "the inner contour has no holes of its own");
}
