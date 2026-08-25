//! WHAT IS DELETED LEAVES NOTHING BEHIND IN THE DOCUMENT.
//!
//! The file used to hold a second check here, reading a document from one particular machine to see
//! that restructuring the model did not leak into the format. A check that stands on a file only one
//! computer has is a check that skips for everybody else, silently, and three of those were found
//! asleep for months. What remains stands on a project built inside the test.

/// Removing a contour takes its origin and its nesting WITH IT.
///
/// Removal used to be written in three places, and each cleaned up its own subset: `remove_contour`
/// and sketch deletion left orphan records in `contour_ents`/`contour_parent`, and those accumulated
/// in the document and went out into the file. Now there is one entry point and nothing can be
/// deleted past it.
#[test]
fn removing_a_contour_takes_its_origin_and_nesting_with_it() {
    use qymcad_core::geom::{Contour, Point2};
    let mut p = qymcad_core::model::Project::default();
    let sq = |o: f64, s: f64| Contour::closed(vec![Point2::new(o, o), Point2::new(o + s, o), Point2::new(o + s, o + s), Point2::new(o, o + s)]);
    let outer = p.add_contour(sq(0.0, 100.0));
    let inner = p.add_contour(sq(10.0, 10.0));
    p.contours.set_ents(inner, vec![1, 2, 3]);
    p.contours.set_parent(inner, outer);

    let idx = p.contour_index(inner).expect("the contour is there");
    p.remove_contour(idx);

    assert!(p.contours.ents_of(inner).is_none(), "the origin was left an orphan");
    assert!(p.contours.parent_of(inner).is_none(), "the nesting was left an orphan");
    assert!(p.contour_index(inner).is_none(), "the contour was not removed");
    assert_eq!(p.contours.len(), p.contours.ids().len(), "the list and its ids diverged");
}
