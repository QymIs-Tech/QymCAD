//! A SKETCH ON A FACE FOLLOWS THE FACE BY ITS NAME. A face merged into its neighbour yields its name to it, and the
//! record of that says where it went; a piece of a face returned to its whole is the whole. A sketch standing on such a
//! face follows that record, as a reference of a node does, and is not found by place - marked for the person - while
//! the name says where it is. Reported behaviour: on the case of the examples a dimension by -10% merged the wall a
//! sketch stood on into the next one, and the sketch was taken over to it by place.
use qymcad_core::model::{DimTarget, Project};

/// The case of the examples opened, `refs` of sketch `sketch` set to `factor` times its value: the sketches that had
/// to be found by place.
fn found_by_place(sketch: u64, refs: Vec<u64>, factor: f64) -> Vec<String> {
    let path = format!("{}/../../examples/QymBoxPc-Case.qcad", env!("CARGO_MANIFEST_DIR"));
    let mut p = qymcad_io::load_project(&path).expect("the example opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let si = p.sketch_index(sketch).expect("the sketch");
    let value = p.sketches[si].constraints.iter().find(|c| Project::dim_refs(c).is_some_and(|r| r == refs)).and_then(Project::dim_value_of).expect("the dimension");
    p.set_dim_target_value(&DimTarget::Sketch { sketch, refs }, value * factor);
    p.solve_sketch(si);
    p.mark_sketch_dirty(sketch);
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    report.rebinds.iter().map(|r| r.what.clone()).collect()
}

#[test]
fn a_sketch_follows_its_wall_merged_into_the_next() {
    let placed = found_by_place(5497, vec![5256, 5257], 0.9);
    assert!(placed.is_empty(), "a sketch on a wall merged into the next was found by place: {placed:?}");
}

#[test]
fn a_sketch_follows_its_piece_of_a_face_returned_to_the_whole() {
    let placed = found_by_place(5182, vec![5330, 11996, 5336], 1.1);
    assert!(placed.is_empty(), "a sketch on a piece of a face returned to its whole was found by place: {placed:?}");
}
