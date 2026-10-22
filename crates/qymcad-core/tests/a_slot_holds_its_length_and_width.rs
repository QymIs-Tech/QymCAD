//! A SLOT HOLDS ITS LENGTH AND ITS WIDTH: its sizes are the distance between the centres of its ends and the distance
//! across an end; a size set is laid as a dimension, set again it is the same dimension; solved, the slot takes it and
//! stays a slot - its sides touching both ends, its ends of one radius.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Project, SlotSize};

/// AN END OF THE SLOT: its centre and its radius.
struct End {
    centre: Point2,
    r: f64,
}

#[test]
fn a_length_and_a_width_set_are_the_slot_s() {
    let mut p = Project::default();
    let si = p.new_sketch("slot");
    p.add_slot_entity(si, Point2::new(10.0, 8.0), Point2::new(20.0, 8.0), 3.0, Purpose::Real);
    let side = p.sketches[si].figures[0].curves[0];
    let read = |p: &Project| [p.slot_size(si, side, SlotSize::Length), p.slot_size(si, side, SlotSize::Width)];
    let [length, width] = read(&p);
    assert!(length.is_some_and(|l| (l - 10.0).abs() < 1e-9) && width.is_some_and(|w| (w - 6.0).abs() < 1e-9), "the slot reads {length:?} by {width:?}, not 10 by 6");
    let dims = |p: &Project| p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Distance { .. })).count();
    assert_eq!(dims(&p), 0, "a slot drawn holds a dimension nobody typed");
    assert!(p.set_slot_size(si, side, SlotSize::Length, 12.0) && p.set_slot_size(si, side, SlotSize::Length, 15.0), "the length was not taken");
    assert!(p.set_slot_size(si, side, SlotSize::Width, 8.0), "the width was not taken");
    assert_eq!(dims(&p), 2, "set twice, the length laid a second dimension");
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y)).expect("a point");
    let ends: Vec<End> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind.as_arc() {
            Some(qymcad_core::model::ArcParts { center, a, .. }) => Some(End { centre: at(center), r: at(center).dist(at(a)) }),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 2, "the slot has {} ends", ends.len());
    let length = ends[0].centre.dist(ends[1].centre);
    assert!((length - 15.0).abs() < 1e-6, "the centres stand {length} apart, not 15");
    assert!(ends.iter().all(|e| (e.r - 4.0).abs() < 1e-6), "the ends are of radii {:?}, not 4", ends.iter().map(|e| e.r).collect::<Vec<f64>>());
    for e in &s.entities {
        if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() {
            let (pa, pb) = (at(a), at(b));
            let u = Point2::new(pb.x - pa.x, pb.y - pa.y);
            let off = |c: Point2| ((c.x - pa.x) * u.y - (c.y - pa.y) * u.x).abs() / u.x.hypot(u.y);
            assert!(ends.iter().all(|e| (off(e.centre) - 4.0).abs() < 1e-6), "a side stands {:?} off the centres, not 4", ends.iter().map(|e| off(e.centre)).collect::<Vec<f64>>());
        }
    }
    assert_eq!(p.sketches[si].figures.len(), 1, "the slot is no longer a figure");
}

#[test]
fn a_size_of_a_slot_tells_the_dimension_it_is_held_by() {
    let mut p = Project::default();
    let si = p.new_sketch("slot");
    p.add_slot_entity(si, Point2::new(10.0, 8.0), Point2::new(20.0, 8.0), 3.0, Purpose::Real);
    let side = p.sketches[si].figures[0].curves[0];
    assert_eq!(p.slot_size_dim(si, side, SlotSize::Length), None, "a slot drawn tells a dimension it does not hold");
    p.set_slot_size(si, side, SlotSize::Width, 8.0);
    p.set_slot_size(si, side, SlotSize::Length, 15.0);
    let held = |which: SlotSize| p.slot_size_dim(si, side, which).and_then(|ci| p.sketches[si].constraints.get(ci)).and_then(Constraint::dim_value);
    assert_eq!(held(SlotSize::Length), Some(15.0), "the length is not told by its own dimension");
    assert_eq!(held(SlotSize::Width), Some(8.0), "the width is not told by its own dimension");
}
