//! Two concentric circles, both selected for an extrusion, give a solid cylinder rather than a tube.
//!
//! The mechanism changed: a selected inner contour is no longer a fill flag on the outer one but a region in its
//! own right, and the faces — the outer one with a hole, and the solid inner one — fuse into a disc. The old
//! `fill` flag in already saved files is read as "this contour is selected too".
use qymcad_core::geom::circle_contour;
use qymcad_core::model::Project;

fn scene() -> (Project, u64, u64, u64) {
    let mut p = Project::default();
    let _part = p.new_document();
    let outer = circle_contour(0.0, 0.0, 10.0, 0.02);
    let inner = circle_contour(0.0, 0.0, 5.0, 0.02);
    let sid = p.add_sketch("ring", vec![outer, inner], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    let cids = p.sketches[si].contour_ids.clone();
    assert_eq!(cids.len(), 2, "two contours");
    (p, sid, cids[0], cids[1])
}

#[test]
fn only_outer_selected_is_a_tube() {
    let (p, sid, outer, _inner) = scene();
    let faces = p.profile_faces(sid, &[outer], &[]);
    assert_eq!(faces.len(), 1, "one selected contour gives one region: {faces:?}");
    assert_eq!(faces[0], (outer, vec![_inner]), "the region of the outer one is itself minus its direct child: a tube");
    let prof = p.feature_profile_encoded(sid, outer).expect("the tube profile");
    assert_eq!(prof[0], 2.0, "two loops, the outer one and the hole, give a tube");
}

#[test]
fn both_selected_make_solid_disc() {
    let (mut p, sid, outer, inner) = scene();
    for (label, profiles, fill) in [("both in profiles", vec![outer, inner], vec![]), ("an old file with the inner one in fill", vec![outer], vec![inner])] {
        let faces = p.profile_faces(sid, &profiles, &fill);
        assert_eq!(faces.len(), 2, "{label}: two regions, the ring and the inner disc: {faces:?}");
        assert!(faces.contains(&(outer, vec![inner])), "{label}: the region of the outer one is the ring");
        assert!(faces.contains(&(inner, vec![])), "{label}: the region of the inner one is a solid disc");
        let encoded = p.encode_profiles_fill(0, sid, &profiles, &fill).expect("the encoding");
        assert_eq!(encoded.len(), 2, "{label}: two faces reach the kernel, and the fusion gives a solid disc");
    }
}
