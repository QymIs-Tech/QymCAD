//! Threads, and an edge of a fillet healed by its snapshot.

use super::*;

// Topological naming: when a stored edge id of a feature comes loose (the topology above it changed), the
// reference is repaired from the geometric snapshot — the nearest current edge by midpoint and direction, and
// only on an unambiguous match.
#[test]
fn fillet_edge_ref_heals_stale_id_by_snapshot() {
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let (src, fid) = (100u64, 200u64);
    p.regen_edges.insert(
        src,
        vec![
            MeshEdge { id: 7, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
            MeshEdge { id: 8, mid: [10.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
        ],
    );
    // The feature selected the edge that used to have id 3; its snapshot is the position (0,0,5) along Z.
    p.edge_refs.insert(fid, vec![ElemSnapshot { id: 3, at: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0] }]);
    // Id 3 is gone, so the snapshot repairs it to the current edge 7, the nearest and unambiguous one.
    assert_eq!(p.resolve_edge_ids(fid, src, &[3]), vec![7], "a lost id must be repaired from the snapshot");
    // A valid id passes through unchanged.
    assert_eq!(p.resolve_edge_ids(fid, src, &[8]), vec![8], "a valid id must be left alone");
    // An ambiguous match (two equally close edges) is not repaired and the edge drops out.
    p.regen_edges.insert(
        src,
        vec![
            MeshEdge { id: 7, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
            MeshEdge { id: 9, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
        ],
    );
    assert!(p.resolve_edge_ids(fid, src, &[3]).is_empty(), "an ambiguous match must not be repaired");
}

#[test]
fn thread_feature_associative_via_circular_edge() {
    // A thread takes its axis and radius from the circular edge of `src`, associatively and from the actual
    // geometry, rather than from a snapshot.
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 4.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 8.0, pitch: 1.25, internal: false, ..Default::default() }, 8.0, 0.0, 0.0); // M8 on the edge of radius 4
    p.regenerate(&k);
    assert!(p.mesh_index(thr).is_some(), "the thread body must be built (the modifier produced a body)");
    // The kernel received the axis (centre at z = 5) and the radius (4) from the edge: associativity.
    assert!(k.calls.borrow().iter().any(|c| c.contains("helical r=4 oz=5")), "the thread must be called with the axis and radius taken from the edge: {:?}", k.calls.borrow());
}

// Thread parameters are validated before the kernel, so bad values give a readable error on a red node rather
// than a silent no-op or a crash in the sweep.
#[test]
fn thread_validates_depth_vs_radius() {
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 4.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    // External M8 with a pitch of 20: a depth of 12.3 against a radius of 4 would push the profile past the axis,
    // which has to produce a readable error.
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 8.0, pitch: 20.0, internal: false, ..Default::default() }, 8.0, 0.0, 0.0);
    p.regenerate(&k);
    let err = p.regen_errors.get(&thr).cloned();
    eprintln!("thread error (depth 12.3 > r 4): {err:?}");
    // Checked by error code rather than by a word: a substring check would go blind on any text edit or
    // translation.
    assert!(
        matches!(err, Some(qymcad_core::errors::CoreError::ThreadDepthTooDeep { .. })),
        "a depth at or beyond the radius must give a readable error about the depth specifically, but got: {err:?}"
    );
    assert!(!k.calls.borrow().iter().any(|c| c.contains("helical r=")), "the kernel must not be called with an invalid depth");
}

// A deep, sharp profile (depth close to the pitch, a large angle) is no longer rejected by validation: the
// kernel builds a true point, capping the depth of a sharp V, quickly and validly. Rejecting it blocked large
// angles, which could not be specified beyond about 70 to 80 degrees.
#[test]
fn thread_deep_profile_reaches_kernel_no_error() {
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 25.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    // A depth of 5 against a pitch of 5 at 60 degrees used to be rejected; it now builds as a sharp V with a
    // capped depth.
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 50.0, pitch: 5.0, internal: false, ..Default::default() }, 100.0, 0.0, 0.0); // M50 on the edge of radius 25
    p.regenerate(&k);
    let err = p.regen_errors.get(&thr).cloned();
    eprintln!("deep, sharp profile: err={err:?}");
    assert!(err.is_none(), "a deep profile must no longer be rejected: {err:?}");
    assert!(k.calls.borrow().iter().any(|c| c.contains("helical r=")), "the kernel must be called, the profile being valid");
}

#[test]
fn thread_axis_orients_into_material() {
    use qymcad_core::geom::Point3;
    use qymcad_core::model::orient_axis_into_mesh;
    let center = [0.0, 0.0, 0.0];
    // The rim is at (0,0,0) and the material (the vertices) lies at z > 0, so the axis has to point at +Z.
    let verts_up: Vec<Point3> = (1..=10).map(|i| Point3::new(0.0, 0.0, i as f64)).collect();
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, -1.0], &verts_up), [0.0, 0.0, 1.0], "the axis must be turned towards the material (+Z)");
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, 1.0], &verts_up), [0.0, 0.0, 1.0], "an already correct axis must be left alone");
    // Material at z < 0 puts the axis at -Z.
    let verts_dn: Vec<Point3> = (1..=10).map(|i| Point3::new(0.0, 0.0, -(i as f64))).collect();
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, 1.0], &verts_dn), [0.0, 0.0, -1.0], "the axis must be turned towards the material (-Z)");
}

#[test]
fn thread_consumes_src_and_survives_serde() {
    // A thread is a modifier: it hides `src` and leaves one body, and every parameter survives
    // serialisation.
    use qymcad_core::feature::FeatureKind;
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    let spec = qymcad_core::thread::ThreadSpec {
        standard: qymcad_core::thread::ThreadStandard::TrapezoidalTr,
        nominal_d: 12.0,
        pitch: 1.25,
        starts: 2,
        left: true,
        internal: true,
        fit: 0.15,
        ..Default::default()
    };
    let thr = p.add_thread(src, 7, spec, 6.0, 1.0, 1.0);
    let node = p.timeline.iter().find(|n| n.id == thr).unwrap();
    assert_eq!(node.kind.consumed_body(), Some(src), "a thread consumes `src`, leaving one body rather than two");
    assert_eq!(node.kind.body(), Some(thr), "the output of a thread is its body");

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let p2 = qymcad_core::model::from_ron(&ron).unwrap();
    let n2 = p2.timeline.iter().find(|n| n.id == thr).unwrap();
    match n2.kind {
        FeatureKind::Thread { src: s, edge, spec, length, lead_in, lead_out, .. } => {
            assert_eq!((s, edge, spec.internal, spec.starts, spec.left), (src, 7, true, 2, true), "the parameters must survive serialisation");
            assert_eq!(spec.standard, qymcad_core::thread::ThreadStandard::TrapezoidalTr, "the standard must survive serialisation");
            assert!((spec.pitch - 1.25).abs() < 1e-9 && (length - 6.0).abs() < 1e-9, "the pitch and the length must be intact");
            assert!((spec.fit - 0.15).abs() < 1e-9, "the fit clearance must be intact");
            assert!((lead_in - 1.0).abs() < 1e-9 && (lead_out - 1.0).abs() < 1e-9, "the lead-in and lead-out must be intact");
        }
        _ => panic!("after serialisation this must still be a thread feature"),
    }
}
