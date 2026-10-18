//! A BLEND IS NAMED AFTER ITS OWN EDGE, even when an edge before it in the list is left out. The kernel takes the
//! names in parallel with the edges; filtering the edges alone shifted every name after a dropped one onto the next
//! edge. Reported behaviour: of 8 edges of a rounding the face credited to one edge was the blend of its neighbour.
use qymcad_core::model::Project;

#[test]
fn a_smooth_edge_left_out_takes_its_name_with_it() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(40.0, 30.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let es = p.regen_edges.get(&block).cloned().expect("the edges of the block");
    let top = es.iter().map(|e| e.a[2].max(e.b[2])).fold(f64::MIN, f64::max);
    let flat: Vec<&qymcad_core::geom::MeshEdge> = es.iter().filter(|e| (e.a[2] - top).abs() < 1e-6 && (e.b[2] - top).abs() < 1e-6).collect();
    let (first, other_mid) = (flat[0].id, flat[2].mid); // two top edges that do not meet
    let round = p.add_fillet(block, 1.0, vec![first]);
    let _ = qymcad_testkit::regenerate(&mut p);
    // a tangent edge the first rounding left: smooth, nothing to round there
    let ids_before: std::collections::HashSet<u32> = es.iter().map(|e| e.id).collect();
    let (_, shapes) = qymcad_testkit::regenerate(&mut p);
    let smooth = shapes.get(&round).map(|s| s.smooth_edge_ids()).unwrap_or_default();
    let tangent = *smooth.iter().find(|e| !ids_before.contains(e)).expect("a tangent edge of the first rounding");
    // the other edge by the name it carries on the rounded body, found where it lies
    let other = p.regen_edges.get(&round).and_then(|es| es.iter().find(|e| (0..3).all(|k| (e.mid[k] - other_mid[k]).abs() < 1e-6)).map(|e| e.id)).expect("the other edge on the rounded body");
    let second = p.add_fillet(round, 1.0, vec![tangent, other]);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the second rounding did not build: {:?}", report.errors);
    let blends: Vec<u64> = p
        .regen_faces
        .get(&second)
        .map(|fs| fs.iter().filter_map(|f| p.names.get(f.id)).filter(|g| g.feature == second && format!("{:?}", g.role) == "Blend").map(|g| g.src).collect())
        .unwrap_or_default();
    // picked second, after the tangent edge left out
    assert_eq!(blends, vec![2], "the blend of the second rounding is named after picks {blends:?}; its own edge {other:#x} is the second pick, the tangent one left out {tangent:#x} the first");
}
