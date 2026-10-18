//! A PIECE OF A SPLIT FACE IS TOLD BY WHAT CUT IT. A cut that splits a face leaves pieces; which of them keeps the name
//! of the whole, and the number of each other, is told by the faces the cut made beside it - the walls of its tool -
//! not by every named face around, whose names an edit above may change. Reported behaviour: on a sample an edit above
//! took a rounding of an older node away from one of two pieces of a wall, and the piece that did not move gave its name
//! to the other.
use qymcad_core::model::{DimTarget, Project};

/// The faces of `body` named as pieces of the wall `wall`, by where they stand.
fn pieces(p: &Project, body: u64, wall: &str) -> Vec<String> {
    let mut v: Vec<String> = p
        .regen_faces
        .get(&body)
        .into_iter()
        .flatten()
        .filter(|f| p.names.describe(f.id).starts_with(wall))
        .map(|f| format!("{} at y {:.3}, {:.3} mm^2", p.names.describe(f.id), f.centroid.y, f.area))
        .collect();
    v.sort();
    v
}

/// THE PRIVATE SAMPLE vacuumCleaner: the cut 250 splits the wall of 176 from line 158 in two; the dimension [154, 151]
/// of sketch 3 from 55 to 49.5 moves the larger piece and not the smaller (at y 17.317, 21.467 mm^2), which keeps its
/// name.
#[test]
fn the_piece_of_a_wall_that_did_not_move_keeps_its_name() {
    let path = format!("{}/../../samples/vacuumCleaner.qcad", env!("CARGO_MANIFEST_DIR"));
    if !std::path::Path::new(&path).exists() {
        eprintln!("PASSED OVER: the private sample vacuumCleaner.qcad is not in this tree - the check on it runs only where the samples are");
        return;
    }
    let mut p = qymcad_io::load_project(&path).expect("the sample opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let small = |p: &Project| pieces(p, 250, "wall f176 src158").into_iter().find(|s| s.contains("at y 17.317"));
    let before = small(&p).expect("the small piece of the wall before the edit");
    p.set_dim_target_value(&DimTarget::Sketch { sketch: 3, refs: vec![154, 151] }, 49.5);
    let si = p.sketch_index(3).expect("sketch 3");
    p.solve_sketch(si);
    p.mark_sketch_dirty(3);
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let after = small(&p).expect("the small piece of the wall after the edit");
    assert_eq!(after, before, "the piece that did not move was renamed; the pieces now: {:?}", pieces(&p, 250, "wall f176 src158"));
}
