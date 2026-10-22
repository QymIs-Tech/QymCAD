//! EVERY FIGURE IS ONE RECORD: a rectangle, a regular polygon and a slot drawn in one sketch lie as three figures of one
//! kind of record, each with its curves; the rectangle is a figure of its four sides with the rest of its frame, and holds
//! itself by its turn as the figures hold theirs - by the one rule of the figures.
//!
//! The rectangle had a record of its own beside the figures, with its rule of what holds it.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{FigureShape, Project};

#[test]
fn a_rectangle_a_polygon_and_a_slot_are_figures_of_one_record() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let sides = p.add_rect_entity(si, 0.0, 0.0, 20.0, 10.0, Purpose::Real);
    p.add_polygon_entity(si, Point2::new(40.0, 5.0), Point2::new(46.0, 5.0), 6, Purpose::Real);
    p.add_slot_entity(si, Point2::new(60.0, 5.0), Point2::new(70.0, 5.0), 3.0, Purpose::Real);
    p.solve_sketch(si);
    let s = &p.sketches[si];
    let shapes: Vec<&str> = s
        .figures
        .iter()
        .map(|f| match f.shape {
            FigureShape::Rectangle(_) => "rectangle",
            FigureShape::Polygon => "polygon",
            FigureShape::Slot => "slot",
        })
        .collect();
    assert_eq!(shapes, ["rectangle", "polygon", "slot"], "the figures of the sketch");
    let rect = s.figures.iter().find(|f| matches!(f.shape, FigureShape::Rectangle(_))).expect("the rectangle");
    assert_eq!(rect.curves, sides, "the curves of the rectangle are its four sides");
    // what holds each figure, read by the one rule of the figures, is all the sketch hides
    let held_by_figures = s.constraints.iter().filter(|c| s.figures.iter().any(|f| f.holds(c))).count();
    assert!(held_by_figures > 0, "no figure holds itself by a constraint of the sketch");
    assert_eq!(s.held_by_shapes().len(), held_by_figures, "the constraints hidden are those the figures hold themselves by");
}
