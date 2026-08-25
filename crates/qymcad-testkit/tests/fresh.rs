use qymcad_core::model::Project;
use qymcad_core::geom::Point2;
#[test]
fn fresh_part_sketch_extrude() {
    let mut p = Project::default();
    let part = p.new_document(); // the first part, which becomes active
    eprintln!("part={part}, active={:?}", p.active_component);
    let sid = p.add_line_sketch("Sketch 1", vec![Point2::new(0.0,0.0), Point2::new(30.0,0.0), Point2::new(30.0,30.0), Point2::new(0.0,30.0)], true);
    let si = p.sketch_index(sid).unwrap();
    p.regen_sketch(si);
    if let Some(o) = p.sketch_owner(sid) { p.set_active_component(Some(o)); }
    p.add_sketch_node(sid, "Sketch 1");
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    eprintln!("closed contours: {} ({closed:?})", closed.len());
    assert!(!closed.is_empty(), "the sketch has a closed contour");
    let body = p.add_extrude_multi(sid, closed.clone(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id,e) in &report.errors { eprintln!("error {id}: {e}"); }
    eprintln!("the body was built: {}", shapes.contains_key(&body));
    assert!(shapes.contains_key(&body), "a freshly drawn sketch in a part extrudes");
    eprintln!("V={:.1}", shapes.get(&body).unwrap().volume());
}
