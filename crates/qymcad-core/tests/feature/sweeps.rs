//! Joints solved by a rebuild, sweeps, lofts, drafts and the node of many profiles.

use super::*;

#[test]
fn regenerate_solves_joint_placement() {
    // The placement pass inside regenerate: the body is built in local space and the mate places the
    // component.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    p.add_extrude(sa, 5.0);
    // A is grounded and displaced; B is rigidly mated origin to origin.
    p.set_grounded(a, true);
    p.set_component_transform(a, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);

    let k = MockKernel::default();
    p.regenerate(&k);
    // A numeric solver converges to a tolerance, so exact equality must not be demanded of it. An earlier path
    // placed the body by pure matrix composition and therefore matched bit for bit, which was a property of the
    // implementation rather than a requirement of the problem. A micrometre here is orders of magnitude below
    // any design significance.
    let got = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let err = ((got[0] - 10.0).powi(2) + got[1].powi(2) + got[2].powi(2)).sqrt();
    assert!(err < 1e-6, "regenerate must place B by the mate: {got:?}, off by {err:.3e}");
}

#[test]
fn moving_component_does_not_rebuild_bodies() {
    // Placing a component (`Component.transform`) does not rebuild any body, only its position in the assembly.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);
    let owner = p.body_owner(body).unwrap();
    p.move_component(owner, [10.0, 0.0, 0.0]);
    p.rotate_component(owner, 2, 30.0);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 1, "moving or rotating a component must not disturb the kernel");
    assert!(rep.built.is_empty());
}

#[test]
fn same_component_face_sketch_builds() {
    // A sketch on a face of a body of the same part is not a cross-component reference, so it builds.
    let mut p = part_project();
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let body1 = p.add_extrude(s1, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let s2 = square(&mut p, "s2");
    let si = p.sketch_index(s2).unwrap();
    p.sketches[si].plane = SketchPlane::Face(body1, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 });
    p.add_sketch_node(s2, "s2");
    let body2 = p.add_combine(body1, s2, 3.0, 0);
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "the same part is not a cross-component reference: {:?}", rep.errors);
    assert!(p.mesh_index(body2).is_some(), "the cutting body must be built");
}

#[test]
fn sweep_path_encoded_open_chain_carries_exact_edges() {
    // The path of a sweep is an open contour of two segments, encoded as [1.0, 2.0, two edges of nine fields].
    // The ninth field of an edge is its provenance, the id of the sketch entity, which the kernel uses to name
    // the side faces so that a name follows the sketch line rather than the ordinal of an edge in the contour.
    let mut p = part_project();
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 10.0), Point2::new(5.0, 10.0)], false);
    let enc = p.sweep_path_encoded(path, 0).expect("an open path must encode");
    assert_eq!(enc[0], 1.0, "exactly one path contour");
    assert_eq!(enc[1], 2.0, "two exact edges; the contour is open, so there is no spurious end-to-start edge");
    assert_eq!(enc.len(), 2 + 2 * qymcad_core::geom::EDGE_FIELDS, "one loop count, one edge count, two edges");
    // The first edge is the line from (0,0) to (0,10).
    assert_eq!(&enc[2..10], &[0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0]);
    // The ninth field is the name descriptor of the future face. A path has none and must have none: the side
    // faces of a sweep are produced by the profile edges, while the path only guides them, so there is nowhere
    // for a name to come from.
    assert_eq!(enc[10], 0.0, "a path carries no names: {}", enc[10]);
}

#[test]
fn add_sweep_builds_node_with_both_sketch_inputs() {
    // A sweep node depends on two sketches, the profile and the path, and regenerate calls `kernel.sweep`.
    let mut p = part_project();
    let prof = square(&mut p, "profile");
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 20.0)], false);
    p.add_sketch_node(prof, "profile");
    p.add_sketch_node(path, "path");
    let body = p.add_sweep(prof, Vec::new(), path, 0);
    let node = p.timeline.iter().find(|n| n.id == body).expect("the sweep node must be in the timeline");
    assert_eq!(node.kind.inputs(), vec![prof, path], "it must depend on the profile and the path");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a sweep must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the swept body must be built");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("sweep ")), "the kernel must receive a sweep call: {:?}", k.calls.borrow());
}

#[test]
fn sweep_contour_candidates_order_and_multi() {
    // Candidate profiles are the closed contours of at least three points; candidate paths list the open ones
    // first.
    let mut p = part_project();
    // The helper cannot build one sketch holding two loops, so the check uses a profile sketch with a single
    // contour together with separate open and closed path sketches.
    let prof = square(&mut p, "profile");
    let prof_cands = p.sweep_profile_contours(prof);
    assert_eq!(prof_cands.len(), 1, "a square gives one profile contour");
    // An open chain gives exactly one open path candidate.
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 8.0), Point2::new(6.0, 8.0)], false);
    let path_cands = p.sweep_path_contours(path);
    assert_eq!(path_cands.len(), 1, "one open chain gives one candidate path");
    // A closed sketch is a valid path as well, as a closed trajectory, though not as an open candidate.
    let ring = square(&mut p, "ring path");
    let ring_cands = p.sweep_path_contours(ring);
    assert_eq!(ring_cands.len(), 1, "a closed contour is a valid path");
    // `add_sweep` with explicit contours records them in the node.
    p.add_sketch_node(prof, "profile");
    p.add_sketch_node(path, "path");
    let body = p.add_sweep(prof, vec![prof_cands[0]], path, path_cands[0]);
    let node = p.timeline.iter().find(|n| n.id == body).unwrap();
    if let qymcad_core::feature::FeatureKind::Sweep { ref profiles, path: pth, .. } = node.kind {
        assert_eq!(profiles, &vec![prof_cands[0]], "the profile contour must be recorded");
        assert_eq!(pth, path_cands[0], "the path contour must be recorded");
    } else {
        panic!("the node is not a sweep");
    }
}

#[test]
fn loft_encoded_concatenates_sections_with_offsets_and_places() {
    // The loft encoding concatenates the loop block of every section: `offsets` starts at zero and holds one
    // boundary per section, and `places` holds twelve placement fields per section, the plane of its sketch.
    let mut p = part_project();
    let s0 = square(&mut p, "bottom"); // On XY at z = 0.
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 20.0); // origin z=20
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.regenerate(&MockKernel::default()); // Resolves the origin of the datum plane at z = 20.
    let enc = p.loft_encoded(&[s0, s1], &[0, 0]).expect("two closed sections must encode");
    let qymcad_core::model::LoftSectionsEncoded { data, offsets, places } = enc;
    assert_eq!(offsets.len(), 3, "one boundary per section: [0, len0, len0 + len1]");
    assert_eq!(offsets[0], 0, "the first section starts at zero");
    assert_eq!(offsets[2], data.len(), "the last boundary is the end of the data");
    assert_eq!(places.len(), 2 * 12, "twelve placement fields for each of the two sections");
    // The placement of the top section carries the origin z = 20 (column 3 is fields 9, 10 and 11).
    assert_eq!(places[12 + 11], 20.0, "the top section must sit at z = 20");
    // Fewer than two sections cannot be encoded.
    assert!(p.loft_encoded(&[s0], &[0]).is_none(), "a single section is not a loft");
}

#[test]
fn draft_resolves_neutral_face_and_routes_pull_direction() {
    // A draft node depends on its source body. The neutral face, addressed by a persistent id, resolves to a
    // normal that becomes the pull direction, and `flip` inverts it. The angle is parametric and reaches the
    // kernel.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let src = p.add_extrude_on(sid, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    let k = MockKernel::default();
    p.regenerate(&k); // Builds src; the mock produces no faces.
                      // The faces of the source: the neutral one (persistent id 42, normal +Z, the bottom of the mould) and two
                      // drafted ones.
                      //
                      // The drafted faces 7 and 8 are put in the scene for more than completeness: with query references the
                      // feature verifies that the faces exist and refuses when they do not. Their ids used to reach the kernel
                      // unchecked, so a draft could be aimed at faces the body no longer had.
    p.regen_faces.insert(
        src,
        vec![
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(5.0, 5.0, 0.0), area: 1.0, id: 42 },
            MeshFace { triangles: vec![], normal: [1.0, 0.0, 0.0], centroid: Point3::new(10.0, 5.0, 5.0), area: 1.0, id: 7 },
            MeshFace { triangles: vec![], normal: [0.0, 1.0, 0.0], centroid: Point3::new(5.0, 10.0, 5.0), area: 1.0, id: 8 },
        ],
    );
    // A plain draft pulls along the normal of the neutral face, +Z.
    let body = p.add_draft(src, vec![7, 8], 42, 5.0, false);
    let node = p.timeline.iter().find(|n| n.id == body).unwrap();
    assert_eq!(node.kind.inputs(), vec![src], "a draft depends on its source body");
    let k2 = MockKernel::default();
    k2.shapes.borrow_mut().insert(src); // src is already built (need_src); only the draft is rebuilt.
    let rep = p.regenerate(&k2);
    assert!(rep.errors.is_empty(), "regenerating a draft must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the drafted body must be built");
    assert!(
        k2.calls.borrow().iter().any(|c| c.starts_with("draft n=2 angle=5") && c.contains("pull=[0.0, 0.0, 1.0]")),
        "a +Z neutral face gives a +Z pull over two faces at an angle of 5: {:?}",
        k2.calls.borrow()
    );
    // `flip` pulls the other way, along -Z. Face 9 must exist as well, for the reason given above.
    p.regen_faces.entry(src).or_default().push(MeshFace { triangles: vec![], normal: [-1.0, 0.0, 0.0], centroid: Point3::new(0.0, 5.0, 5.0), area: 1.0, id: 9 });
    let body2 = p.add_draft(src, vec![9], 42, 3.0, true);
    let k3 = MockKernel::default();
    k3.shapes.borrow_mut().insert(src);
    p.regenerate(&k3);
    assert!(k3.calls.borrow().iter().any(|c| c.contains("angle=3") && c.contains("pull=[-0.0, -0.0, -1.0]")), "flip must pull along -Z: {:?}", k3.calls.borrow());
    assert!(p.mesh_index(body2).is_some(), "the flipped draft body must be built");
}

#[test]
fn add_loft_builds_node_and_regenerates() {
    // A loft node depends on every section sketch, in order, and regenerate calls `kernel.loft` with nsec = 2.
    let mut p = part_project();
    let s0 = square(&mut p, "bottom");
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 15.0);
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(s0, "bottom");
    p.add_sketch_node(s1, "top");
    let body = p.add_loft(vec![s0, s1], vec![0, 0], false, 0, 0, false);
    let node = p.timeline.iter().find(|n| n.id == body).expect("the loft node must be in the timeline");
    assert_eq!(node.kind.inputs(), vec![s0, s1], "it must depend on both sections, in order");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a loft must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the lofted body must be built");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("loft nsec=2 ")), "the kernel must receive a loft over two sections: {:?}", k.calls.borrow());
}

#[test]
fn subtractive_loft_booleans_with_target_and_consumes_it() {
    // A loft with a non-zero src calls `loft_combine` rather than `loft`: it depends on the target body and
    // consumes it.
    let mut p = part_project();
    // The target body first, a plain extruded square; it becomes the active body and the target of the cut.
    let base_sk = square(&mut p, "base");
    p.add_sketch_node(base_sk, "base");
    let target = p.add_extrude(base_sk, 20.0);
    // Two loft sections on different planes.
    let s0 = square(&mut p, "bottom");
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 15.0);
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(s0, "bottom");
    p.add_sketch_node(s1, "top");
    let body = p.add_loft(vec![s0, s1], vec![0, 0], false, target, 0, false); // op = 0 is a cut.
    let node = p.timeline.iter().find(|n| n.id == body).expect("the cut-loft node must exist");
    assert!(node.kind.inputs().contains(&target), "a cut-loft depends on the target body");
    assert_eq!(node.kind.consumed_body(), Some(target), "a cut-loft consumes the target body");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a cut-loft must report no errors: {:?}", rep.errors);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c.starts_with("loft_combine nsec=2 op=0 src_ok=true")), "the kernel must receive loft_combine for the cut with the built body: {:?}", calls);
    assert!(!calls.iter().any(|c| c.starts_with("loft nsec")), "a plain loft must not be called for a boolean");
}
