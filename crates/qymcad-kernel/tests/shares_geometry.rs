//! `Shape::shares_geometry` — whether two shapes share a face or an edge. The batch rebuild asks this to keep
//! bodies that share a sub-shape on one worker thread, because meshing or healing a shared sub-shape on two
//! threads at once crashes the process.
use qymcad_kernel::Shape;

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
