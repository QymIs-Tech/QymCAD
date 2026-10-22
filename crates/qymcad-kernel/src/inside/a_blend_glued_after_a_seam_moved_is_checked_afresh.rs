//! A BLEND GLUED AFTER ITS SEAM WAS MOVED IS CHECKED AFRESH. An edge along the seam of a round face is rounded with the
//! seam moved away and the lines the moving left glued back (`off_seams`): the glue reshapes the body after the blend
//! checked its result sound and noted the faces it made. What the blend knew is of the body before the glue and is let
//! go: the wrap of the result checks the glued body itself, and the faces made are not known of it.
use crate::Shape;

/// A cylinder R5, 6 high, with the quarter x 0..10, y -10..0 cut out of its foot, 4 high: the upright edge of the cut
/// at (5, 0) stands on the seam line of the round face, the seam running up from its top.
fn a_cylinder_cut_at_its_foot_beside_its_seam() -> Shape {
    let cylinder = Shape::cylinder(5.0, 6.0).expect("the cylinder");
    let quarter = Shape::extrude(&[0.0, -10.0, 10.0, -10.0, 10.0, 0.0, 0.0, 0.0], 4.0).expect("the quarter tool");
    cylinder.boolean(&quarter, 0).expect("the quarter cut")
}

#[test]
fn a_rounding_along_a_seam_hands_out_a_body_with_nothing_known_of_it() {
    let cut = a_cylinder_cut_at_its_foot_beside_its_seam();
    // the upright edge at (5, 0), z 0..4: where the wall of the cut meets the round face, on its seam line
    let along_the_seam = cut
        .edges_info()
        .into_iter()
        .find(|e| e.circle.is_none() && e.poly.iter().all(|p| (p[0] - 5.0).abs() < 1e-3 && p[1].abs() < 1e-3 && p[2] < 4.001))
        .expect("setup: no upright edge of the cut stands on the seam line");
    let rounded = cut.fillet_edges(1.0, &[along_the_seam.id]).expect("R1 rounds the edge along the seam");
    // the faces made are of the body before the glue and are let go; the body is checked sound again as it stands - the
    // wrap of the result checks it, so it comes out marked by that check and sound
    let marks = rounded.marks();
    assert!(!marks.made_faces_known, "the body glued after the seam moved keeps the faces made before the glue");
    assert!(marks.checked_sound && rounded.is_valid(), "the rounding along the seam, glued, is not checked sound as it stands");
}
