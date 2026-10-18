//! A LINE BROKEN IN TWO KEEPS WHAT HELD IT: a horizontal line broken by a click is two horizontal lines meeting at the
//! break, a horizontal each - left as it was, the horizontal held the far ends only and the break point was free to
//! leave the line.
//!
//! Reported behaviour: after a break only one half stayed horizontal - two constraints and seven freedoms where there
//! are three and six.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};

#[test]
fn a_broken_horizontal_line_is_two_horizontal_lines() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let l = p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, Purpose::Real);
    let (a, b) = match p.sketches[si].entities.iter().find(|e| e.id == l).map(|e| e.kind) {
        Some(EntityKind::Line { a, b }) => (a, b),
        _ => panic!("the line"),
    };
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    p.regen_sketch(si);
    assert!(p.break_line(si, l, 5.0, 0.0), "the line broke");
    let lines: Vec<(u64, u64)> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2, "two halves");
    for (x, y) in lines {
        let held = p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Horizontal { a, b } if (*a == x && *b == y) || (*a == y && *b == x)));
        assert!(held, "the half {x}-{y} is not horizontal: {:?}", p.sketches[si].constraints);
    }
}

/// A LINE BROKEN IN TWO IS THE LINE IT WAS AND A LINE ADDED: the half from its start keeps its id, so what was built
/// from the line goes on naming it.
#[test]
fn the_first_half_of_a_broken_line_is_the_line_it_was() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let l = p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, Purpose::Real);
    p.regen_sketch(si);
    assert!(p.break_line(si, l, 5.0, 0.0), "the line broke");
    let start = |p: &Project, id: u64| match p.sketches[si].entities.iter().find(|e| e.id == id).map(|e| e.kind) {
        Some(EntityKind::Line { a, .. }) => p.point_xy(si, a),
        _ => None,
    };
    assert!(start(&p, l).is_some_and(|q| (q.0 + 20.0).abs() < 1e-9), "the half from the start of the line is not the line it was");
    assert_eq!(p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { .. })).count(), 2, "two halves");
}
