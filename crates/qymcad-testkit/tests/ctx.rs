use qymcad_core::model::Project;
fn load() -> Project { qymcad_core::model::from_ron(include_str!("doc2.ron")).expect("load") }
#[test]
fn extrude_from_assembly_scopes_to_sketch_owner() {
    let mut p = load();
    p.active_component = Some(1); // the assembly is active, as it is right after opening
    // the fix, as in `apply_sketch_cmd`: the owner of the sketch is made the active component
    let owner = p.sketch_owner(3);
    eprintln!("the owner of the sketch is {owner:?}");
    if let Some(o) = owner { p.set_active_component(Some(o)); }
    let body = p.add_combine_multi_op(0, 3, vec![29], 10.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    for (id,e) in &report.errors { eprintln!("error {id}: {e}"); }
    let ok = shapes.contains_key(&body);
    eprintln!("the body was built: {ok}");
    assert_eq!(owner, Some(2), "the owner of the sketch is the first part");
    assert!(ok, "with the owner of the sketch active, an extrusion started from the assembly builds a body");
    assert!(shapes.get(&body).unwrap().volume() > 1.0, "the body is not empty");
}
