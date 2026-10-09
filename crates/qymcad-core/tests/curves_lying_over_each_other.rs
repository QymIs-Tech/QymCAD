//! THE CURVES LYING OVER EACH OTHER: a side of a rectangle drawn against another lies over the side of that one, and
//! the right button lists both so a person can name the one meant. A line on the same straight with nothing in common,
//! a line only touching at an end, a circle of another radius - none of them lies over.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{EntityKind, Project};

/// The lines of sketch `si` through (x, y), in the order of the sketch.
fn lines_through(p: &Project, si: usize, x: f64, y: f64) -> Vec<u64> {
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point");
    s.entities
        .iter()
        .filter(|e| match e.kind {
            EntityKind::Line { a, b } => {
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
    let circle = p.sketches[si].entities.iter().find(|e| matches!(e.kind, EntityKind::Circle { r, .. } if (r - 10.0).abs() < 1e-9)).map(|e| e.id).expect("the circle");
    if p.overlapping_curves(si, circle) != vec![circle] {
        failures.push(format!("a circle with one of another radius round the same centre lists {:?}", p.overlapping_curves(si, circle)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
