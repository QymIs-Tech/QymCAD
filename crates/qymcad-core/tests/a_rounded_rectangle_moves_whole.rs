//! A RECTANGLE ROUNDED ALL ROUND MOVES AND TURNS AS ONE SHAPE when it is taken by its four sides: its corners are
//! virtual sharps no side ends at and its arcs stand between the sides, and they go with it - a turn of 30 deg turns
//! the sides by 30 deg and leaves every arc a quarter of R3; a move by (10, 5) carries the drawing whole.
//!
//! Reported behaviour: Rotate of a rectangle 45 x 35 with its sizes, rounded R3 all round, was refused as held - the
//! rectangle stood on the axes and every arc had flipped outward into three quarters of a circle.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};

/// A rectangle 45 x 35 at (0, 0) with its width and height laid as dimensions, its four corners rounded R3.
fn rounded() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_entity(si, 0.0, 0.0, 45.0, 35.0, Purpose::Real);
    let c = p.sketches[si].rects[0].corners;
    let dim = |a, b, d| Constraint::Distance { dimension: Default::default(), a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    p.sketches[si].constraints.extend([dim(c[0], c[1], 45.0), dim(c[1], c[2], 35.0)]);
    p.solve_sketch(si);
    assert_eq!(p.fillet_all_corners(si, 3.0), 4, "four corners rounded");
    (p, si)
}

/// How far each arc turns, in degrees.
fn sweeps(p: &Project, si: usize) -> Vec<f64> {
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point of the arc");
    s.entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Arc { center, a, b, ccw } => {
                let (c, pa, pb) = (at(center), at(a), at(b));
                let (a0, a1) = ((pa.1 - c.1).atan2(pa.0 - c.0), (pb.1 - c.1).atan2(pb.0 - c.0));
                Some(if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) }.to_degrees())
            }
            _ => None,
        })
        .collect()
}

fn worst(p: &Project, si: usize) -> f64 {
    p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max)
}

#[test]
fn a_rounded_rectangle_turns_by_its_sides() {
    let (mut p, si) = rounded();
    let sides = p.sketches[si].rects[0].sides;
    p.rotate_entities(si, &sides, 22.5, 17.5, 30.0);
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    let turn = s.entities.iter().find(|e| e.id == sides[0]).and_then(|e| match e.kind {
        EntityKind::Line { a, b } => Some((at(b).1 - at(a).1).atan2(at(b).0 - at(a).0).to_degrees().rem_euclid(180.0)),
        _ => None,
    });
    let arcs = sweeps(&p, si);
    assert!(turn.is_some_and(|t| (t - 30.0).abs() < 1e-6), "the bottom side turned to {turn:?} deg, not 30");
    assert!(arcs.len() == 4 && arcs.iter().all(|a| (a - 90.0).abs() < 1e-6), "the arcs after the turn are not quarters: {arcs:?}");
    assert!(worst(&p, si) < 1e-6, "the sketch does not solve after the turn: {:.2e}", worst(&p, si));
}

#[test]
fn a_rounded_rectangle_moves_by_its_sides() {
    let (mut p, si) = rounded();
    let before: Vec<(u64, f64, f64)> = p.sketches[si].points.iter().map(|q| (q.id, q.x, q.y)).collect();
    let sides = p.sketches[si].rects[0].sides;
    p.move_entities(si, &sides, 10.0, 5.0);
    let left: Vec<String> = before
        .iter()
        .filter_map(|&(id, x, y)| {
            let q = p.sketches[si].points.iter().find(|q| q.id == id)?;
            let off = (q.x - x - 10.0).hypot(q.y - y - 5.0);
            // the origin and the ends of the axes stay; every point of the rectangle goes by (10, 5)
            (off > 1e-6 && (q.x - x).hypot(q.y - y) > 1e-9).then(|| format!("point {id} went ({:.3}, {:.3})", q.x - x, q.y - y))
        })
        .collect();
    assert!(left.is_empty(), "the rounded rectangle did not move whole by (10, 5): {left:?}");
    assert!(worst(&p, si) < 1e-6, "the sketch does not solve after the move: {:.2e}", worst(&p, si));
}
