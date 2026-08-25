//! Construction geometry must not reach the profile of a contour.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

#[test]
fn construction_line_excluded_from_profile() {
    let mut p = Project::default();
    let sid = p.add_line_sketch(
        "sq",
        vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    let cid = p.sketches[si].contour_ids[0];
    let before = p.contours[p.contour_index(cid).unwrap()].points.len();

    p.add_line_entity(si, 5.0, -5.0, 5.0, 15.0, qymcad_core::feature::Purpose::Construction); // a construction line, carried by the same flag
    p.regen_sketch(si);

    let after = p.contours[p.contour_index(cid).unwrap()].points.len();
    assert_eq!(before, after, "a construction line must not change the profile");
    // but its points exist and remain available as snap targets
    assert!(p.sketches[si].points.len() >= 6, "the points of the construction line were added to the sketch");
    assert!(p.sketches[si].entities.iter().any(|e| e.construction), "a construction entity is present");
}

#[test]
fn delete_keeps_system_points() {
    // Deleting entities must not remove the system points, the origin and the axes. The protected set used to
    // forget the axes, so any deletion made them disappear.
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.ensure_origin(si);
    p.ensure_axis(si, 0);
    p.ensure_axis(si, 1);
    let origin = p.sketches[si].origin;
    let axes = p.sketches[si].axis_pts;
    assert!(origin != 0 && axes[0] != 0 && axes[1] != 0, "the system points are materialised");
    let eid = p.sketches[si].entities[0].id;
    p.delete_entities(si, &[eid]);
    let has = |id: u64| p.sketches[si].points.iter().any(|q| q.id == id);
    assert!(has(origin), "the origin survived the deletion");
    assert!(has(axes[0]), "the X axis survived");
    assert!(has(axes[1]), "the Y axis survived");
    // and the single source of truth returns exactly those
    let sys = p.sketches[si].system_ids();
    assert_eq!(sys.len(), 3, "system_ids holds the origin and two axes: {sys:?}");
}

#[test]
fn construction_spline_excluded_from_profile() {
    // a construction spline does not reach the profile or the contours, though its points remain snap targets
    use qymcad_core::geom::Point2;
    use qymcad_core::model::Project;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let before = p.contours.len();
    p.add_spline(si, vec![Point2::new(0.0, 0.0), Point2::new(5.0, 5.0), Point2::new(10.0, 0.0)], qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::Construction); // construction
    assert_eq!(p.contours.len(), before, "a construction spline added no contour");
    assert!(p.sketches[si].splines.iter().any(|s| s.construction), "the spline is marked as construction");
    // the polyline is still available, for drawing it dashed
    assert!(p.spline_polyline(si, 0).len() >= 2, "the polyline of the spline tessellates");
    // an ordinary spline does add a contour
    p.add_spline(si, vec![Point2::new(0.0, 10.0), Point2::new(5.0, 15.0), Point2::new(10.0, 10.0)], qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::Real);
    assert_eq!(p.contours.len(), before + 1, "an ordinary spline produced a contour");
}
