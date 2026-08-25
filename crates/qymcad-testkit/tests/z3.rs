mod common;
#[test]
fn combine_src0_makes_valid_new_body() {
    let mut p = common::testbug();
    // a combine with no source is a new body: the contour is extruded as a boss, but with no source it is
    // simply a new body
    p.active_component = Some(277);
    let body = p.add_combine_multi_op(0, 291, vec![302], 5.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    assert_ne!(body, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let errs: Vec<_> = report.errors.iter().filter(|(id,_)| *id == body).collect();
    assert!(errs.is_empty(), "a new body, with no source, built without error: {errs:?}");
    let s = shapes.get(&body).expect("the body with no source was built");
    eprintln!("a combine with no source: valid={}, solids={}, V={:.1}", s.is_valid(), s.tessellate(0.5).len(), s.volume());
    assert!(s.is_valid() && s.volume() > 1.0, "a valid, non-empty new body");
}
