//! A POLYGON GIVEN ANOTHER COUNT OF SIDES KEEPS ITS OWN POINTS AND LINES (`set_polygon_sides`): the vertices it has and
//! the sides between them stay, and what holds its first vertex holds it after. Laid anew whole, it took new ids for
//! every count typed, and a tie on a vertex went with it.

use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Project};

#[test]
fn the_vertices_and_sides_a_polygon_keeps_are_the_same_ones() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let (c, sides) = p.add_polygon_param(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 6, qymcad_core::feature::Purpose::Real);
    let ends = |p: &Project, id: u64| p.sketches[si].entity(id).and_then(|e| e.kind.as_line()).map(|l| [l.a, l.b]).expect("GUARD: a side");
    let first = ends(&p, sides[0])[0];
    // a line of its own, its end held on the first vertex
    let line = p.add_line_entity(si, 30.0, 30.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    let end = ends(&p, line)[1];
    p.sketches[si].constraints.push(Constraint::Coincident { a: end, b: first });
    for n in [8u32, 4] {
        assert!(p.set_polygon_sides(si, c, n), "the polygon did not take {n} sides");
        let held = p.sketches[si].constraints.iter().any(|k| matches!(*k, Constraint::Coincident { a, b } if (a == end && b == first) || (a == first && b == end)));
        assert!(held, "{n} sides: the tie on the first vertex is gone");
        assert!(p.sketches[si].entity(sides[0]).is_some(), "{n} sides: the first side is another line");
        assert!(p.sketches[si].points.iter().any(|q| q.id == first), "{n} sides: the first vertex is another point");
        let count = p.kept_number(c, "sides").unwrap_or(0.0);
        assert_eq!(count, n as f64, "the polygon holds {count} sides, not {n}");
    }
}
