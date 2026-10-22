//! THE MIDDLE OF A LINE HOLDS WHAT IS TIED TO IT: a point held at the middle of a line by Midpoint takes a Vertical with
//! the centre of a circle and a distance to another point; a change of the line's length moves its middle and what is
//! tied there with it; deleting the line takes the middle away, and what was tied to it.
//!
//! Reported behaviour (#57): a constraint from the middle of a line needed a point put there by hand first.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, Project, SketchPoint};

/// A line from (0, 0) to (40, 0) with a point held at its middle, a circle about (25, 30) whose centre is held Vertical
/// with the middle, and the line's length dimensioned. Answers the project, the sketch, the line, its middle, the centre
/// and the index of the length.
fn tied() -> (Project, usize, u64, u64, u64, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let line = p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, Purpose::Real);
    p.add_circle_entity(si, 25.0, 30.0, 5.0, Purpose::Real);
    let s = &p.sketches[si];
    let (a, b) = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() { Some((a, b)) } else { None }).expect("the line");
    let c = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::CircleParts { center, .. }) = e.kind.as_circle() { Some(center) } else { None }).expect("the circle");
    let mid = p.alloc_id();
    p.sketches[si].points.push(SketchPoint { id: mid, x: 20.0, y: 0.0 });
    p.sketches[si].constraints.push(Constraint::Midpoint { p: mid, a, b });
    p.sketches[si].constraints.push(Constraint::Vertical { a: mid, b: c });
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 40.0, off: 5.0, expr: String::new(), driven: false, axis: 0, at: None });
    let len = p.sketches[si].constraints.len() - 1;
    p.solve_sketch(si);
    (p, si, line, mid, c, len)
}

fn at(p: &Project, si: usize, id: u64) -> Option<(f64, f64)> {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y))
}

#[test]
fn a_length_changed_moves_the_middle_and_what_is_tied_there() {
    let (mut p, si, _, mid, c, len) = tied();
    if let Constraint::Distance { d, .. } = &mut p.sketches[si].constraints[len] {
        *d = 60.0;
    }
    p.solve_sketch(si);
    let (m, k) = (at(&p, si, mid).expect("the middle"), at(&p, si, c).expect("the centre"));
    let s = &p.sketches[si];
    let (a, b) = s.entities.iter().find_map(|e| if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() { Some((a, b)) } else { None }).expect("the line");
    let (pa, pb) = (at(&p, si, a).expect("an end"), at(&p, si, b).expect("an end"));
    assert!((pa.0 - pb.0).abs() > 59.999, "the length did not change: {pa:?}-{pb:?}");
    assert!((m.0 - (pa.0 + pb.0) / 2.0).abs() < 1e-6 && (m.0 - k.0).abs() < 1e-6, "middle {m:?} of {pa:?}-{pb:?}, centre {k:?}");
}

#[test]
fn deleting_the_line_takes_its_middle_and_what_was_tied_to_it() {
    let (mut p, si, line, mid, c, _) = tied();
    p.delete_entities(si, &[line]);
    let s = &p.sketches[si];
    assert!(at(&p, si, mid).is_none(), "the middle of a deleted line is left as a point");
    let tied_left: Vec<&Constraint> = s.constraints.iter().filter(|k| k.points().contains(&mid) || matches!(k, Constraint::Vertical { b, .. } if *b == c)).collect();
    assert!(tied_left.is_empty(), "what was tied to the middle is left: {tied_left:?}");
}
