//! A POINT FOUR LINES MEET AT STAYS THE CORNER OF EACH CUT AT IT: a cross of four lines, two opposite corners chamfered,
//! and the point dragged - every shortened line still runs into the point, and each stays horizontal or vertical.
//!
//! Reported behaviour: after two chamfers on a cross the point was dragged, and the two lines of the first cut came
//! loose from it - one turned, the cut swung round, and their constraints stood on nothing.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{ChamferLegs, Constraint, Project};

/// A cross of four lines from (0, 0), each held horizontal or vertical as the automatic constraints hold a line drawn
/// along an axis. Answers the project, the sketch, the point at the middle and the lines: right, up, left, down.
fn a_cross() -> (Project, usize, u64, [u64; 4]) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ends = [(20.0, 0.0), (0.0, 20.0), (-20.0, 0.0), (0.0, -20.0)];
    let lines = ends.map(|(x, y)| p.add_line_entity(si, 0.0, 0.0, x, y, Purpose::Real));
    p.regen_sketch(si);
    let middle = p.sketches[si].points.iter().find(|q| q.x.abs() < 1e-9 && q.y.abs() < 1e-9).map(|q| q.id).expect("the middle");
    for (k, &l) in lines.iter().enumerate() {
        let Some(qymcad_core::model::LineParts { a, b }) = p.sketches[si].entities.iter().find(|e| e.id == l).and_then(|e| e.kind.as_line()) else { panic!("a line") };
        p.sketches[si].constraints.push(if k % 2 == 0 { Constraint::Horizontal { a, b } } else { Constraint::Vertical { a, b } });
    }
    p.solve_sketch(si);
    (p, si, middle, lines)
}

/// HOW A CORNER OF THE CROSS IS CUT, the one way or the other.
#[derive(Clone, Copy, Debug)]
enum Cut {
    Chamfer,
    Fillet,
}

#[test]
fn two_chamfers_on_a_cross_keep_every_line_on_its_corner() {
    two_cuts_on_a_cross_keep_every_line_on_its_corner(Cut::Chamfer);
}

#[test]
fn two_fillets_on_a_cross_keep_every_line_on_its_corner() {
    two_cuts_on_a_cross_keep_every_line_on_its_corner(Cut::Fillet);
}

fn two_cuts_on_a_cross_keep_every_line_on_its_corner(cut: Cut) {
    let (mut p, si, middle, l) = a_cross();
    for pair in [(l[0], l[1]), (l[2], l[3])] {
        let done = match cut {
            Cut::Chamfer => p.chamfer_lines_of_pair(si, pair, ChamferLegs::equal(3.0), None),
            Cut::Fillet => p.fillet_at_pair(si, pair, 3.0),
        };
        assert!(done, "{cut:?}: the corner {pair:?} is cut");
    }
    p.solve_sketch_drag(si, Some((middle, 5.0, 4.0)));
    p.solve_sketch(si);

    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    let m = at(middle);
    let mut sins = Vec::new();
    for (k, &line) in l.iter().enumerate() {
        let Some(qymcad_core::model::LineParts { a, b }) = s.entities.iter().find(|e| e.id == line).and_then(|e| e.kind.as_line()) else { panic!("a line") };
        let (pa, pb) = (at(a), at(b));
        // the line, extended, runs through the middle point
        let off = ((pb.0 - pa.0) * (m.1 - pa.1) - (pb.1 - pa.1) * (m.0 - pa.0)).abs() / (pb.0 - pa.0).hypot(pb.1 - pa.1);
        if off > 1e-6 {
            sins.push(format!("line {k} stands {off:.3} off the corner"));
        }
        let along_axis = if k % 2 == 0 { (pb.1 - pa.1).abs() } else { (pb.0 - pa.0).abs() };
        if along_axis > 1e-6 {
            sins.push(format!("line {k} left its axis by {along_axis:.3}"));
        }
    }
    let worst = p.sketch_residuals(si).into_iter().fold(0.0_f64, f64::max);
    if worst > 1e-6 {
        sins.push(format!("the sketch does not solve: worst residual {worst:.2e}"));
    }
    assert!(sins.is_empty(), "{cut:?}: the point was dragged to {m:?}:\n{}", sins.join("\n"));
}
