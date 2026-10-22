//! A POLYGON AND A SLOT ARE FIGURES, as a rectangle is: one shape with constraints of its own - a polygon's corners on
//! its circle and its sides equal, a slot's ends of one radius touched by its sides - that the list and the badges do not
//! show. A figure holds its shape when its size changes, and a curve of it deleted breaks it into plain curves, its own
//! constraints gone with it.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, FigureShape, Project};

/// Where the point of `id` stands in sketch `si`.
fn at(p: &Project, si: usize, id: u64) -> Point2 {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y)).expect("the point")
}

/// The ends of every line of sketch `si`.
fn lines(p: &Project, si: usize) -> Vec<[Point2; 2]> {
    p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind.as_line() {
            Some(qymcad_core::model::LineParts { a, b }) if !e.construction => Some([at(p, si, a), at(p, si, b)]),
            _ => None,
        })
        .collect()
}

#[test]
fn a_hexagon_is_a_figure_that_stays_regular_and_breaks_when_a_side_goes() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    let (centre, sides) = p.add_polygon_param(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 6, Purpose::Real);
    let s = &p.sketches[si];
    assert!(s.figures.len() == 1 && s.figures[0].shape == FigureShape::Polygon && s.figures[0].curves.len() == 7, "the hexagon is no figure of its sides and circle: {:?}", s.figures);
    let held = s.held_by_shapes();
    let own: Vec<usize> = s.constraints.iter().enumerate().filter(|(_, c)| matches!(c, Constraint::PointOnCircle { .. } | Constraint::Equal { .. })).map(|(k, _)| k).collect();
    assert!(own.len() == 11 && own.iter().all(|k| held.contains(k)), "the six corners on the circle and the five equal sides are not all hidden: {own:?} of {held:?}");
    // its radius set to 14: the corners go out to the circle, every side as long, a regular hexagon still
    let k = p.sketches[si].constraints.iter().position(|c| matches!(c, Constraint::Diameter { c, .. } if *c == centre)).expect("the radius");
    if let Constraint::Diameter { d, .. } = &mut p.sketches[si].constraints[k] {
        *d = 14.0;
    }
    p.solve_sketch(si);
    let c = at(&p, si, centre);
    for [a, b] in lines(&p, si) {
        assert!((a.dist(c) - 14.0).abs() < 1e-6 && (a.dist(b) - 14.0).abs() < 1e-6, "a side from {a:?} to {b:?} is off the hexagon of radius 14");
    }
    // a side deleted: the record and its own constraints go, five plain lines and the circle stay
    p.delete_entities(si, &[sides[2]]);
    let s = &p.sketches[si];
    assert!(s.figures.is_empty(), "a hexagon with a side deleted is still a figure");
    assert!(!s.constraints.iter().any(|c| matches!(c, Constraint::PointOnCircle { .. } | Constraint::Equal { .. })), "the own constraints of the broken hexagon stay: {:?}", s.constraints);
    assert_eq!(lines(&p, si).len(), 5, "the other five sides went with the deleted one");
}

#[test]
fn a_slot_is_a_figure_that_keeps_its_sides_touching_and_breaks_when_a_side_goes() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_slot_entity(si, Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), 4.0, Purpose::Real);
    let s = &p.sketches[si];
    assert!(s.figures.len() == 1 && s.figures[0].shape == FigureShape::Slot && s.figures[0].curves.len() == 4, "the slot is no figure of its four curves: {:?}", s.figures);
    let held = s.held_by_shapes();
    let own: Vec<usize> = s.constraints.iter().enumerate().filter(|(_, c)| matches!(c, Constraint::EqualRadius { .. } | Constraint::Tangent { .. })).map(|(k, _)| k).collect();
    assert!(own.len() == 5 && own.iter().all(|k| held.contains(k)), "the equal ends and the four touchings are not all hidden: {own:?} of {held:?}");
    // one end dragged 6 up, half a millimetre a frame as a hand leads it: both sides still run off the line of the
    // centres by the radius of the ends - the radius itself is free, no dimension holds it
    let centres: Vec<u64> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind.as_arc() {
            Some(qymcad_core::model::ArcParts { center, .. }) => Some(center),
            _ => None,
        })
        .collect();
    let k = p.sketches[si].points.iter().position(|q| q.id == centres[0]).expect("a centre");
    for _ in 0..12 {
        p.sketches[si].points[k].y += 0.5;
        p.solve_sketch(si);
    }
    let (c1, c2) = (at(&p, si, centres[0]), at(&p, si, centres[1]));
    let r = p.sketches[si]
        .entities
        .iter()
        .find_map(|e| match e.kind.as_arc() {
            Some(qymcad_core::model::ArcParts { center, a, .. }) if center == centres[0] => Some(at(&p, si, a).dist(c1)),
            _ => None,
        })
        .expect("the end arc");
    let off = |q: Point2| ((c2.x - c1.x) * (q.y - c1.y) - (c2.y - c1.y) * (q.x - c1.x)).abs() / c1.dist(c2);
    assert!((c1.y - c2.y).abs() > 1.0, "the dragged end did not move up: {c1:?} {c2:?}");
    for [a, b] in lines(&p, si) {
        assert!((off(a) - r).abs() < 1e-6 && (off(b) - r).abs() < 1e-6, "a side from {a:?} to {b:?} does not run {r:.4} off the centres {c1:?} {c2:?}");
    }
    // a side deleted: the record and its own constraints go
    let side = p.sketches[si].entities.iter().find(|e| e.kind.as_line().is_some()).map(|e| e.id).expect("a side");
    p.delete_entities(si, &[side]);
    let s = &p.sketches[si];
    assert!(s.figures.is_empty(), "a slot with a side deleted is still a figure");
    assert!(!s.constraints.iter().any(|c| matches!(c, Constraint::EqualRadius { .. } | Constraint::Tangent { .. })), "the own constraints of the broken slot stay: {:?}", s.constraints);
}
