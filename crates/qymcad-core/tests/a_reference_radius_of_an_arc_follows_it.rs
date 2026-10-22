//! A REFERENCE RADIUS OF AN ARC READS THE ARC AS IT STANDS after every solve, as a reference radius of a circle does.
//!
//! Reported behaviour (#167): an arc drawn at radius 20 with a driving radius of 25 and a reference one came to 25, and
//! the reference still said 20.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Project};

#[test]
fn a_reference_radius_of_an_arc_follows_its_driving_one() {
    let mut p = Project::default();
    let si = p.new_sketch("arc");
    p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(0.0, 20.0), Winding::Ccw, Purpose::Real);
    let s = &p.sketches[si];
    let centre = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::ArcParts { center, .. }) = e.kind.as_arc() { Some(center) } else { None }).expect("the arc");
    let radius = |d: f64, driven: bool| Constraint::Diameter { c: centre, d, off: 0.0, expr: String::new(), driven, diam: false, at: None };
    p.sketches[si].constraints.push(radius(25.0, false));
    p.sketches[si].constraints.push(radius(20.0, true));
    p.solve_sketch(si);
    let reference = p.sketches[si].constraints.iter().find_map(|c| if let Constraint::Diameter { d, driven: true, .. } = c { Some(*d) } else { None }).expect("the reference radius");
    assert!((reference - 25.0).abs() < 1e-6, "the reference radius says {reference:.4}, the arc is 25");
}

#[test]
fn a_reference_radius_of_an_arc_follows_a_drag_of_its_end() {
    let mut p = Project::default();
    let si = p.new_sketch("arc");
    p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(0.0, 20.0), Winding::Ccw, Purpose::Real);
    let s = &p.sketches[si];
    let (centre, end) = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::ArcParts { center, a, .. }) = e.kind.as_arc() { Some((center, a)) } else { None }).expect("the arc");
    p.sketches[si].constraints.push(Constraint::Fixed { p: centre });
    p.sketches[si].constraints.push(Constraint::Diameter { c: centre, d: 20.0, off: 0.0, expr: String::new(), driven: true, diam: false, at: None });
    p.solve_sketch(si);
    // the first end dragged out to 30 in frames of a drag, as the window drags it, and let go
    for k in 1..=5 {
        p.solve_sketch_drag_fast(si, Some((end, 20.0 + 2.0 * k as f64, 0.0)));
    }
    let during = p.sketches[si].constraints.iter().find_map(|c| if let Constraint::Diameter { d, driven: true, .. } = c { Some(*d) } else { None }).expect("the reference radius");
    let s = &p.sketches[si];
    let r = s.points.iter().find(|q| q.id == end).map(|q| q.x.hypot(q.y)).expect("the end");
    assert!((during - r).abs() < 1e-6, "while dragged the reference radius says {during:.4}, the arc is {r:.4}");
}
