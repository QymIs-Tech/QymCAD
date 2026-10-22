//! `Shape::shares_geometry` — whether two shapes share an element: a face, an edge or a vertex that is one kernel
//! sub-shape in both. The batch rebuild asks this to keep bodies that share a sub-shape on one worker thread, because
//! meshing or writing the tolerances of a shared sub-shape on two threads at once rewrites it twice over.
use crate::Shape;

#[test]
fn split_pieces_share_the_cut_face_but_separate_bodies_do_not() {
    let cyl = Shape::cylinder(5.0, 10.0).expect("a cylinder builds");
    let pieces = cyl.split_by_plane([0.0, 0.0, 5.0], [0.0, 0.0, 1.0], 7).expect("the cut gives pieces");
    assert_eq!(pieces.len(), 2, "a plane through the middle makes two pieces");

    // The two pieces come from one cut and hold the SAME cut face (one TShape, not a copy).
    assert!(pieces[0].shares_geometry(&pieces[1]), "the two pieces of one cut share the cut face");

    // Bodies built apart share nothing.
    let a = Shape::cylinder(5.0, 10.0).expect("a");
    let b = Shape::cylinder(5.0, 10.0).expect("b");
    assert!(!a.shares_geometry(&b), "two cylinders built apart share no sub-shape");
    assert!(!pieces[0].shares_geometry(&a), "a cut piece and an unrelated cylinder share no sub-shape");
}

/// TWO BODIES THAT TOUCH AT ONE CORNER SHARE ITS VERTEX, and are kept on one thread: an algorithm writes the tolerance
/// of a vertex it is handed, and the two bodies hold one. Two blocks 10 x 10 x 10, the second at (10, 10, 10), united:
/// the union is two solids meeting at the point (10, 10, 10) in one vertex, and no face or edge between them - the
/// check by faces and edges let them go to two threads.
#[test]
fn bodies_that_touch_at_a_corner_share_its_vertex() {
    let square = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    let a = Shape::extrude(&square, 10.0).expect("a block");
    let b = Shape::extrude(&square, 10.0).expect("a block").transformed(&[1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0, 10.0]).expect("the block placed");
    let union = Shape::fuse_many(&[&a, &b]).expect("the union");
    assert_eq!(union.solid_count(), 2, "setup: the union of two blocks touching at a corner is two solids");
    let (first, second) = (union.solid_at(0).expect("the first solid"), union.solid_at(1).expect("the second solid"));
    assert!(first.shares_geometry(&second), "two solids meeting at one vertex were told to share nothing");
    assert!(!a.shares_geometry(&b), "two blocks built apart were told to share an element");
}
