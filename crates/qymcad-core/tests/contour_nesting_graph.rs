//! The nesting of contours is a stored relation rather than a recomputation with a tolerance.
//!
//! Which contour is a hole of which used to be decided by a point-in-polygon test on every build, each consumer
//! using its own threshold. On a real part that test declared a tab to be inside the outer contour, although the
//! outer one already excludes it through a notch in its own boundary: the tab was recorded as a hole, the hole
//! touched the boundary, the face failed to build, and the whole outer region silently dropped out of the body.
//!
//! The relation is now computed once when the sketch is regenerated and stored in `contour_parent`.
use qymcad_core::model::Project;

fn sketch_with(rects: &[(f64, f64, f64, f64)]) -> (Project, usize, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    for &(x0, y0, x1, y1) in rects {
        p.add_rect_entity(si, x0, y0, x1, y1, qymcad_core::feature::Purpose::Real);
    }
    p.regen_sketch(si);
    let cids = p.sketches[si].contour_ids.clone();
    (p, si, cids)
}

/// The area of a contour, used to tell them apart without relying on order.
fn area(p: &Project, cid: u64) -> f64 {
    p.contour_profile_xy(cid)
        .map(|xy| {
            let n = xy.len() / 2;
            let mut a = 0.0;
            for i in 0..n {
                let j = (i + 1) % n;
                a += xy[2 * i] * xy[2 * j + 1] - xy[2 * j] * xy[2 * i + 1];
            }
            0.5 * a.abs()
        })
        .unwrap_or(0.0)
}

/// Three nested rectangles: the parent of each is the nearest enclosing one rather than the outermost.
#[test]
fn nesting_is_stored_and_points_to_the_closest_parent() {
    let (p, si, cids) = sketch_with(&[(0.0, 0.0, 100.0, 100.0), (10.0, 10.0, 90.0, 90.0), (20.0, 20.0, 80.0, 80.0)]);
    let mut by_area: Vec<u64> = cids.clone();
    by_area.sort_by(|a, b| area(&p, *b).partial_cmp(&area(&p, *a)).unwrap());
    let (big, mid, small) = (by_area[0], by_area[1], by_area[2]);

    assert_eq!(p.contours.parent_of(big), Some(0), "the outer one is marked as a root, with parent 0");
    assert_eq!(p.contours.parent_of(mid).as_ref(), Some(&big), "the middle one is a child of the outer");
    assert_eq!(p.contours.parent_of(small).as_ref(), Some(&mid), "the inner one is a child of the middle one rather than of the outer");

    let sid = p.sketches[si].id;
    assert_eq!(p.feature_holes(sid, big), vec![mid], "the only hole of the outer one is the middle one");
    assert_eq!(p.feature_holes(sid, mid), vec![small], "the hole of the middle one is the inner one");
    assert!(p.feature_holes(sid, small).is_empty(), "the inner one has no holes");
}

/// The failing case: a contour touching the boundary of the enclosing one is not a hole — it is part of the
/// boundary rather than a cavity. It is what made the whole outer region of the first extrusion drop out.
#[test]
fn a_contour_touching_the_boundary_is_not_a_hole() {
    // an outer 100×100 and a 10×20 tab pressed against its boundary by its left side, at x = 0
    let (p, si, cids) = sketch_with(&[(0.0, 0.0, 100.0, 100.0), (0.0, 40.0, 10.0, 60.0)]);
    let mut by_area: Vec<u64> = cids.clone();
    by_area.sort_by(|a, b| area(&p, *b).partial_cmp(&area(&p, *a)).unwrap());
    let (outer, tab) = (by_area[0], by_area[1]);
    assert!(area(&p, tab) > 0.0, "the tab exists");

    assert_eq!(p.contours.parent_of(tab), Some(0), "a touching contour is a root rather than a child");
    let sid = p.sketches[si].id;
    assert!(!p.feature_holes(sid, outer).contains(&tab), "and does not enter the holes: {:?}", p.feature_holes(sid, outer));
}

/// Two non-nested contours side by side: neither is a parent of the other, or an extrusion would consume its
/// neighbour.
#[test]
fn disjoint_contours_have_no_parent() {
    let (p, _si, cids) = sketch_with(&[(0.0, 0.0, 10.0, 10.0), (50.0, 50.0, 60.0, 60.0)]);
    for cid in &cids {
        assert_eq!(p.contours.parent_of(*cid), Some(0), "a standalone contour is a root");
    }
}

/// The graph is recomputed when the sketch is edited: adding an outer contour makes the previous outer one a
/// child.
#[test]
fn nesting_follows_sketch_edits() {
    let (mut p, si, _cids) = sketch_with(&[(10.0, 10.0, 20.0, 20.0)]);
    let inner = p.sketches[si].contour_ids[0];
    assert_eq!(p.contours.parent_of(inner), Some(0), "while it stands alone it is a root");

    p.add_rect_entity(si, 0.0, 0.0, 100.0, 100.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let cids = p.sketches[si].contour_ids.clone();
    let outer = *cids.iter().max_by(|a, b| area(&p, **a).partial_cmp(&area(&p, **b)).unwrap()).unwrap();
    let small = *cids.iter().min_by(|a, b| area(&p, **a).partial_cmp(&area(&p, **b)).unwrap()).unwrap();
    assert_eq!(p.contours.parent_of(small).as_ref(), Some(&outer), "after the edit the small one became a child of the new outer one");
}
