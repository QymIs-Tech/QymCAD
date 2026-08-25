//! Touching profiles in one operation give a single body with no seam edges, the profiles being fused by a 2D
//! boolean before the extrusion. N prisms and a fuse used to leave doubled edges along the line of contact.

use qymcad_core::model::Project;

/// Two 20×20 squares touching along the side at x = 20, extruded by one operation. The result has to be a
/// topologically single 40×20×10 box: exactly 12 edges and no seams at x = 20.
#[test]
fn touching_profiles_no_seam_edges() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si, 20.0, 0.0, 40.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert_eq!(closed.len(), 2, "two touching squares");
    let e = p.add_extrude_multi(sid, closed, 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    let s = shapes.get(&body).expect("the body");
    let v = s.volume();
    assert!((v - 8000.0).abs() < 8.0, "a 40×20×10 box: V={v:.1}");
    let edges = p.regen_edges.get(&body).map(|e| e.len()).unwrap_or(0);
    assert_eq!(edges, 12, "a single box is 12 edges, not {edges}: the extra ones are seams along the line of contact");
    // and along the line of contact at x = 20 there are no vertical edges at all
    let seam = p
        .regen_edges
        .get(&body)
        .map(|es| es.iter().filter(|ed| (ed.a[0] - 20.0).abs() < 1e-6 && (ed.b[0] - 20.0).abs() < 1e-6).count())
        .unwrap_or(0);
    assert_eq!(seam, 0, "edges on the former line of contact at x = 20: {seam}");
}

/// Overlapping profiles, and not only touching ones, fuse cleanly as well: an L shape from two rectangles.
#[test]
fn overlapping_profiles_merge_clean() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    p.add_rect_entity(si, 0.0, 0.0, 30.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    let e = p.add_extrude_multi(sid, closed, 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let body = p.finish_base_body(e, 1);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the rebuild is clean: {:?}", report.errors);
    let v = shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
    // the union of the laid-out areas — three regions, two arms and their 10×10 overlap — is an L of 500 by 10
    assert!((v - 5000.0).abs() < 5.0, "an L of 500 by 10: V={v:.1}");
}
