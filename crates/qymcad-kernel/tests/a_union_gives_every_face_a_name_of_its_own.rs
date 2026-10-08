//! A UNION GIVES EVERY FACE A NAME OF ITS OWN, even when two of its arguments arrive with the same structural name on a
//! face. Which number the second one may keep is decided by the record of the names already taken in the result - the
//! record the union keeps for all its arguments at once, updated as each one is carried in. A record that missed a name
//! would let the second face take it too, and a reference to one face would then lead to both.
use qymcad_core::names::NAMED;
use qymcad_kernel::Shape;

#[test]
fn two_copies_named_alike_come_out_named_apart() {
    let block = Shape::extrude(&[0.0, 0.0, 5.0, 0.0, 5.0, 5.0, 0.0, 5.0], 5.0).expect("a box");
    let ids: Vec<u32> = block.tessellate(0.5).iter().flat_map(|b| b.faces.iter().map(|f| f.id)).collect();
    assert_eq!(ids.len(), 6, "a box has six faces");
    let moved = |x: f64| block.transformed(&[1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).expect("a moved copy");
    let (a, b) = (moved(0.0), moved(20.0));
    // the same structural name on one face of each copy, as a recipe could give them
    let shared = NAMED | 4242;
    a.rename_faces(&[(ids[0], shared)]);
    b.rename_faces(&[(ids[0], shared)]);

    let both = Shape::fuse_many(&[&a, &b]).expect("the two copies unite");
    let mut names: Vec<u32> = both.tessellate(0.5).iter().flat_map(|b| b.faces.iter().map(|f| f.id)).collect();
    assert_eq!(names.len(), 12, "two separate boxes have twelve faces");
    assert_eq!(names.iter().filter(|&&n| n == shared).count(), 1, "the shared name stays on one face only: {names:?}");
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 12, "two faces of the union share a name");
}
