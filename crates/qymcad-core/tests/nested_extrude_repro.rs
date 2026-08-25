//! Two nested circles forming a ring: the profile has to be the outer loop with the inner one as a hole.
use qymcad_core::geom::circle_contour;
use qymcad_core::model::Project;

#[test]
fn nested_circles_classify_as_outer_plus_hole() {
    let mut p = Project::default();
    let _part = p.new_document();
    let outer = circle_contour(0.0, 0.0, 10.0, 0.02);
    let inner = circle_contour(0.0, 0.0, 5.0, 0.02);
    let sid = p.add_sketch("ring", vec![outer, inner], None);
    p.add_sketch_node(sid, "Sketch");

    let si = p.sketch_index(sid).unwrap();
    let cids = p.sketches[si].contour_ids.clone();
    assert_eq!(cids.len(), 2, "two contours");
    let (outer_cid, inner_cid) = (cids[0], cids[1]);

    let holes_of_outer = p.feature_holes(sid, outer_cid);
    let holes_of_inner = p.feature_holes(sid, inner_cid);
    eprintln!("holes_of_outer = {holes_of_outer:?} (expecting [inner={inner_cid}])");
    eprintln!("holes_of_inner = {holes_of_inner:?} (expecting [])");
    assert_eq!(holes_of_outer, vec![inner_cid], "the inner one is a hole of the outer");
    assert!(holes_of_inner.is_empty(), "the inner one has no holes");

    // the profile of the outer contour has two loops: the outer one and the hole
    let prof = p.feature_profile_encoded(sid, outer_cid).expect("the profile");
    eprintln!("prof[0], the loop count = {}", prof[0]);
    assert_eq!(prof[0], 2.0, "the outer profile has two loops: the contour and the hole");
}
