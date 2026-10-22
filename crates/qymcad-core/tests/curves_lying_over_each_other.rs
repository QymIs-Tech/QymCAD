//! THE CURVES LYING OVER EACH OTHER: a side of a rectangle drawn against another lies over the side of that one, and
//! the right button lists both so a person can name the one meant. A line on the same straight with nothing in common,
//! a line only touching at an end, a circle of another radius - none of them lies over.
use qymcad_core::feature::Purpose;
use qymcad_core::model::Project;

/// The lines of sketch `si` through (x, y), in the order of the sketch.
fn lines_through(p: &Project, si: usize, x: f64, y: f64) -> Vec<u64> {
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point");
    s.entities
        .iter()
        .filter(|e| match e.kind.as_line() {
            Some(qymcad_core::model::LineParts { a, b }) => {
                let (a, b) = (at(a), at(b));
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l = dx.hypot(dy);
                let t = ((x - a.0) * dx + (y - a.1) * dy) / (l * l);
                t > 0.0 && t < 1.0 && ((x - a.0) * dy - (y - a.1) * dx).abs() / l < 1e-6
            }
            _ => false,
        })
        .map(|e| e.id)
        .collect()
}

#[test]
fn the_sides_drawn_over_each_other_are_listed_and_nothing_else() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 60.0, Purpose::Real);
    p.add_rect_entity(si, 20.0, 0.0, 40.0, 60.0, Purpose::Real);
    p.merge_close_points(si, 1e-6);
    // a line on the straight of the shared side, above it with nothing in common, and one touching its top end
    p.add_line_entity(si, 20.0, 70.0, 20.0, 90.0, Purpose::Real);
    p.add_line_entity(si, 20.0, 60.0, 20.0, 65.0, Purpose::Real);
    p.add_circle_entity(si, 100.0, 0.0, 10.0, Purpose::Real);
    p.add_circle_entity(si, 100.0, 0.0, 12.0, Purpose::Real);
    let mut failures = Vec::new();
    let shared = lines_through(&p, si, 20.0, 30.0);
    if shared.len() != 2 {
        failures.push(format!("GUARD: the shared side is two lines: {shared:?}"));
    }
    let listed = p.overlapping_curves(si, shared[0]);
    if listed != shared {
        failures.push(format!("the shared side lists {listed:?}, the two sides there are {shared:?}"));
    }
    let top = lines_through(&p, si, 10.0, 60.0);
    if p.overlapping_curves(si, top[0]) != top {
        failures.push(format!("the top of the left rectangle, alone there, lists {:?}", p.overlapping_curves(si, top[0])));
    }
    let circle = p.sketches[si].entities.iter().find(|e| e.kind.as_circle().is_some_and(|qymcad_core::model::CircleParts { r, .. }| (r - 10.0).abs() < 1e-9)).map(|e| e.id).expect("the circle");
    if p.overlapping_curves(si, circle) != vec![circle] {
        failures.push(format!("a circle with one of another radius round the same centre lists {:?}", p.overlapping_curves(si, circle)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// CURVES OF ANY KIND LYING OVER EACH OTHER are listed as lines are: an ellipse drawn twice in one place, a spline drawn
/// twice, two arcs of one circle sharing a stretch - each lists the other; an ellipse of other axes round the same
/// centre lists nothing.
#[test]
fn curves_of_any_kind_drawn_over_each_other_are_listed() {
    use qymcad_core::feature::{Ends, Winding};
    use qymcad_core::geom::Point2;
    let mut failures = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ellipse = |p: &mut Project, rx: f64| {
        p.add_ellipse_entity(si, Point2::new(0.0, 0.0), rx, 5.0, 0.0, Purpose::Real);
        p.sketches[si].entities.last().map(|e| e.id).expect("the ellipse")
    };
    let (e1, e2, e3) = (ellipse(&mut p, 10.0), ellipse(&mut p, 10.0), ellipse(&mut p, 12.0));
    let spline = |p: &mut Project| p.add_spline(si, vec![Point2::new(30.0, 0.0), Point2::new(40.0, 8.0), Point2::new(50.0, 0.0)], Ends::Open, Purpose::Real).expect("the spline");
    let (s1, s2) = (spline(&mut p), spline(&mut p));
    let arc = |p: &mut Project, from: Point2, to: Point2| {
        p.add_arc_entity(si, Point2::new(0.0, 40.0), from, to, Winding::Ccw, Purpose::Real);
        p.sketches[si].entities.last().map(|e| e.id).expect("the arc")
    };
    let a1 = arc(&mut p, Point2::new(10.0, 40.0), Point2::new(0.0, 50.0));
    let a2 = arc(&mut p, Point2::new(7.0710678, 47.0710678), Point2::new(-10.0, 40.0));
    /// A CURVE AND THE ONE IT LIES OVER.
    struct Over {
        name: &'static str,
        of: u64,
        want: u64,
    }
    for Over { name, of, want } in [
        Over { name: "an ellipse drawn twice", of: e1, want: e2 },
        Over { name: "a spline drawn twice", of: s1, want: s2 },
        Over { name: "two arcs of one circle", of: a1, want: a2 },
    ] {
        let listed = p.overlapping_curves(si, of);
        if !listed.contains(&want) {
            failures.push(format!("{name}: lists {listed:?}, not {want}"));
        }
    }
    if p.overlapping_curves(si, e1).contains(&e3) {
        failures.push("an ellipse of other axes round the same centre is listed as lying over".into());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
