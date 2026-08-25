use qymcad_core::model::Project;
fn load() -> Project { qymcad_core::model::from_ron(include_str!("doc2.ron")).expect("load doc2") }
#[test]
fn sketch_extrudes() {
    let mut p = load();
    p.active_component = Some(2);
    let si = p.sketch_index(3).expect("the sketch");
    // the closed contours of the sketch, as the interface takes its targets
    p.regen_sketch(si); // as the interface does on entering or editing a sketch
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_profile_xy(*cid).is_some()).collect();
    eprintln!("closed contours of the sketch: {} ({closed:?})", closed.len());
    assert!(!closed.is_empty(), "the sketch has a closed contour to extrude");
    // create the extrusion as one node, as `apply_sketch_cmd` does with no part, giving a new body
    let body = p.add_combine_multi_op(0, 3, closed.clone(), 10.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    assert_ne!(body, 0, "the node was created");
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let _ = &report;
    let s = shapes.get(&body);
    eprintln!("the body was extruded: {}", s.is_some());
    if let Some(s)=s { eprintln!("valid={}, V={:.1}", s.is_valid(), s.volume()); }
    assert!(s.is_some(), "the sketch extruded into a body");
}
