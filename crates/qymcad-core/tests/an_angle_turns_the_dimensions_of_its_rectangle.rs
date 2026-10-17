//! AN ANGLE ON A SIDE TURNS THE DIMENSIONS OF THE RECTANGLE WITH IT: a width and a height laid horizontal and vertical,
//! the rectangle then turned by an angle dimension on a side, go along their sides and keep reading the width and the
//! height, the rectangle square and turned where the angle says.
//!
//! Reported behaviour (#71): turned, the rectangle's dimensions stayed horizontal and vertical, measuring along the
//! axes; by an angle, a horizontal width of 40 held the projection of the turned side at 40 and pulled the side longer.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};

#[test]
fn an_angle_on_a_side_turns_the_dimensions_of_the_rectangle() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    drop(p.add_rect_entity(si, 10.0, 10.0, 50.0, 40.0, Purpose::Real));
    p.solve_sketch(si);
    let r = p.sketches[si].rects[0].clone();
    let [c0, c1, c2, _] = r.corners;
    // the width below the bottom side and the height right of the right one, horizontal and vertical
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: c0, b: c1, d: 40.0, off: 8.0, expr: String::new(), driven: false, axis: 1, at: None });
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: c1, b: c2, d: 30.0, off: 8.0, expr: String::new(), driven: false, axis: 2, at: None });
    p.solve_sketch(si);
    // a fixed line from (10, 10) at -20 deg, the angle measured between it and the bottom side
    let a = p.add_line_entity(si, 10.0, 10.0, 40.0, 10.0 - 30.0 * 20f64.to_radians().tan(), Purpose::Real);
    let line = match p.sketches[si].entities.iter().find(|e| e.id == a).map(|e| e.kind) {
        Some(EntityKind::Line { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    p.sketches[si].constraints.extend([Constraint::Fixed { p: line.0 }, Constraint::Fixed { p: line.1 }]);
    let angle = Constraint::AngleLines { dimension: Default::default(), a: line.0, b: line.1, c: c0, d: c1, deg: 20.0, expr: String::new(), driven: false, off: 0.0, at: None };
    p.give_rect_turn_to(si, &angle);
    p.sketches[si].constraints.push(angle);
    let ci = p.sketches[si].constraints.len() - 1;
    if let Constraint::AngleLines { deg, .. } = &mut p.sketches[si].constraints[ci] {
        *deg = 35.0;
    }
    let resid = p.solve_sketch(si);
    let xy = |id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a corner");
    let (q0, q1, q2) = (xy(c0), xy(c1), xy(c2));
    let (w, h) = ((q1.0 - q0.0).hypot(q1.1 - q0.1), (q2.0 - q1.0).hypot(q2.1 - q1.1));
    let turn = (q1.1 - q0.1).atan2(q1.0 - q0.0).to_degrees();
    // found by their points: laying the angle takes constraints out, and the places of the rest move
    let axes: Vec<u8> = [(c0, c1), (c1, c2)]
        .iter()
        .filter_map(|&(x, y)| {
            p.sketches[si].constraints.iter().find_map(|c| match *c {
                Constraint::Distance { a, b, axis, .. } if a == x && b == y => Some(axis),
                _ => None,
            })
        })
        .collect();
    assert!(
        resid < 1e-6 && (w - 40.0).abs() < 1e-6 && (h - 30.0).abs() < 1e-6 && (turn - 15.0).abs() < 1e-4,
        "turned by the angle to 15 deg: sides {w:.4} x {h:.4} at {turn:.4} deg, residual {resid:e}; the dimensions along axes {axes:?}"
    );
    assert_eq!(axes, vec![0, 0], "the dimensions of the turned rectangle are still horizontal and vertical");
}
