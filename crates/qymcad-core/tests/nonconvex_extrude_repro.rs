//! A non-convex closed profile — an arch with a notch cut into its top — failed to extrude while a rectangle
//! extruded fine.
//!
//! What is checked: the contour detector sees exactly one closed contour and the profile encodes for the
//! kernel. The geometry is a base of 100 with sloping sides and a square notch at the top.
use qymcad_core::model::Project;

/// The closed contours of sketch `si`, as the interface counts them: closed and with at least three points.
fn closed_contours(p: &Project, si: usize) -> Vec<u64> {
    p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_profile_xy(*cid).is_some()).collect()
}

#[test]
fn nonconvex_notch_profile_is_one_closed_contour() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("arch");
    // the traversal: a slope up and to the left, a square notch in the middle of the top, a slope down and to
    // the right, then the base
    let poly = [(0.0, 0.0), (20.0, 50.0), (40.0, 50.0), (40.0, 20.0), (60.0, 20.0), (60.0, 50.0), (80.0, 50.0), (100.0, 0.0)];
    for k in 0..poly.len() {
        let a = poly[k];
        let b = poly[(k + 1) % poly.len()];
        p.add_line_entity(si, a.0, a.1, b.0, b.1, qymcad_core::feature::Purpose::Real);
    }
    let closed = closed_contours(&p, si);
    assert_eq!(closed.len(), 1, "a non-convex profile has to be one closed contour, found: {closed:?}");
    // and the profile encodes for the kernel, as exact edges
    assert!(p.feature_profile_encoded(p.sketches[si].id, 0).is_some(), "the profile did not encode for extrusion");
}

#[test]
fn profile_with_tiny_corner_gap_still_closes() {
    // The real defect: one corner is torn by a micro-gap. The ends of the lines look like one point but are
    // different nodes — a common case at the origin, which the merge command leaves alone as a system point.
    // The profile still has to close, the loop detector being tolerant of position.
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("gap");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.0, 10.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    // the closing line arrives at (0.0002, 0), which does not merge with (0,0) at an epsilon of 1e-6, leaving a
    // separate node and a gap of 2e-4
    p.add_line_entity(si, 0.0, 10.0, 0.0002, 0.0, qymcad_core::feature::Purpose::Real);
    let closed = closed_contours(&p, si);
    assert_eq!(closed.len(), 1, "a profile with a micro-gap at a corner has to close, found: {closed:?}");
}

#[test]
fn merge_heals_bigger_gap_at_origin() {
    // A real case: the gap at the origin is larger than the automatic welding tolerance of 1e-3, so automatic
    // welding does not take it. The merge command, however, has to attach a non-system point to the origin; it
    // used to leave the system point alone and therefore did not help. After merging, the contour closes.
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("corner at the origin");
    p.ensure_origin(si); // the system origin at (0,0)
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real); // starting from the origin
    p.add_line_entity(si, 10.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 10.0, 10.0, 0.05, 0.0, qymcad_core::feature::Purpose::Real); // closing at (0.05,0), where the end lies on the first line
    // The planar arrangement does find a closed face, the triangle through (0.05,0), (10,0) and (10,10): the
    // end of the third line lies on the first, a real intersection, and the small tail near the origin simply
    // falls outside the region.
    assert_eq!(closed_contours(&p, si).len(), 1, "the arrangement finds a face despite the gap at the origin");
    let n = p.merge_close_points(si, 0.1); // merge at a tolerance of 0.1, attaching (0.05,0) to the origin
    assert!(n >= 1, "at least one point has to attach to the origin, merged: {n}");
    assert_eq!(closed_contours(&p, si).len(), 1, "after merging there is a full triangle from the origin");
}
