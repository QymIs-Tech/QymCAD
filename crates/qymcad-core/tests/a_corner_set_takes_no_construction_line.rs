//! A CONSTRUCTION LINE IS NO SIDE OF A CORNER in the set of corners the fillet and the chamfer cut: a rectangle drawn from
//! its centre has its construction diagonals ending at its corners, and a diagonal picked with a side must not make a
//! corner with it - only the two sides of a corner do.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

#[test]
fn a_picked_diagonal_makes_no_corner_with_a_side() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_from_centre(si, Point2::new(20.0, 15.0), Point2::new(40.0, 30.0), Purpose::Real);
    let r = p.sketches[si].rects()[0].clone();
    let diagonal = r.diagonals.expect("a rectangle drawn from its centre has its diagonals")[0];
    // the first side and the diagonal from its first corner, picked together; then the two sides of that corner
    let with_diagonal = p.corners_of_lines(si, &[r.sides[0], diagonal]);
    let two_sides = p.corners_of_lines(si, &[r.sides[3], r.sides[0]]);
    assert!(with_diagonal.is_empty(), "a side and a construction diagonal were taken for a corner: {with_diagonal:?}");
    assert_eq!(two_sides.len(), 1, "the two sides of a corner make one corner: {two_sides:?}");
}
