//! A BROKEN RECTANGLE LEAVES ITS LINES WHAT THEY PLAINLY HAVE: a rectangle losing its top side leaves three lines that
//! stand level and square; related as a person drawing them would get them, they are held so - Horizontal and Vertical
//! on an upright rectangle, Perpendicular and Parallel on a turned one, Equal between the two of one length - with
//! nothing over-defined, and a corner dragged keeps them level and square.
//!
//! Reported behaviour (#72): the lines were left plain, held by nothing, and a dragged corner pulled them out of shape.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Project};

/// A rectangle 40 x 30, upright or turned by 30 deg about its first corner (10, 10). Answers the project and the sketch.
fn drawn(turned: bool) -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    if turned {
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let (a, b) = (Point2::new(10.0, 10.0), Point2::new(10.0 + 40.0 * c, 10.0 + 40.0 * s));
        drop(p.add_rect3_entity(si, a, b, Point2::new(b.x - 30.0 * s, b.y + 30.0 * c), Purpose::Real));
    } else {
        drop(p.add_rect_entity(si, 10.0, 10.0, 50.0, 40.0, Purpose::Real));
    }
    p.solve_sketch(si);
    (p, si)
}

/// The same rectangle with its side `k` deleted (0 the first, 2 the one across from it). Answers the project, the
/// sketch, and how many constraints the lines left hold.
fn broken(turned: bool, k: usize) -> (Project, usize, usize) {
    let (mut p, si) = drawn(turned);
    let side = p.sketches[si].rects()[0].sides[k];
    p.delete_entities(si, &[side]);
    let held = p.sketches[si].constraints.len();
    (p, si, held)
}

/// The lines of the sketch as unit directions, after a drag of the free corner of the first line by (7, 4).
fn dragged(p: &mut Project, si: usize) -> Vec<(f64, f64)> {
    let s = &p.sketches[si];
    let ends: Vec<(u64, u64)> = s.entities.iter().filter_map(|e| if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() { Some((a, b)) } else { None }).collect();
    let corner = ends[0].0;
    let q = s.points.iter().find(|q| q.id == corner).map(|q| (q.x, q.y)).expect("a corner");
    p.solve_sketch_drag(si, Some((corner, q.0 + 7.0, q.1 + 4.0)));
    p.solve_sketch(si);
    let s = &p.sketches[si];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    ends.iter()
        .map(|&(a, b)| {
            let ((ax, ay), (bx, by)) = (at(a), at(b));
            let l = (bx - ax).hypot(by - ay);
            ((bx - ax) / l, (by - ay) / l)
        })
        .collect()
}

#[test]
fn an_upright_rectangle_broken_leaves_its_lines_level_and_upright() {
    let (mut p, si, laid) = broken(false, 2);
    let s = &p.sketches[si];
    let kinds = |pred: fn(&Constraint) -> bool| s.constraints.iter().filter(|c| pred(c)).count();
    let (h, v) = (kinds(|c| matches!(c, Constraint::Horizontal { .. })), kinds(|c| matches!(c, Constraint::Vertical { .. })));
    assert!(laid > 0 && h == 1 && v == 2, "the bottom Horizontal and the two uprights Vertical: {laid} laid, {h} Horizontal, {v} Vertical: {:?}", s.constraints);
    assert_eq!(p.sketch_dof(si).1, 0, "the lines left are over-defined");
    let dirs = dragged(&mut p, si);
    assert!(dirs.iter().all(|(x, y)| x.abs() < 1e-6 || y.abs() < 1e-6), "a corner dragged pulled the lines off level and upright: {dirs:?}");
}

#[test]
fn a_turned_rectangle_broken_leaves_its_lines_square() {
    let (mut p, si, laid) = broken(true, 2);
    assert!(laid > 0, "nothing was laid on the lines of a turned rectangle");
    assert_eq!(p.sketch_dof(si).1, 0, "the lines left are over-defined");
    let dirs = dragged(&mut p, si);
    // three lines of a U: the first square to the second, the third parallel to the first
    let dot = |a: (f64, f64), b: (f64, f64)| a.0 * b.0 + a.1 * b.1;
    let cross = |a: (f64, f64), b: (f64, f64)| a.0 * b.1 - a.1 * b.0;
    let square = (0..dirs.len()).flat_map(|i| (i + 1..dirs.len()).map(move |j| (i, j))).filter(|&(i, j)| dot(dirs[i], dirs[j]).abs() < 1e-6).count();
    let parallel = (0..dirs.len()).flat_map(|i| (i + 1..dirs.len()).map(move |j| (i, j))).filter(|&(i, j)| cross(dirs[i], dirs[j]).abs() < 1e-6).count();
    assert!(square == 2 && parallel == 1, "a corner dragged pulled the turned lines out of square: {dirs:?}");
}

/// The left side deleted: the top and the bottom, of one length, are held Equal; level by Horizontal, not Parallel on
/// top of it.
#[test]
fn the_two_sides_of_one_length_are_held_equal() {
    let (p, si, _) = broken(false, 3);
    let s = &p.sketches[si];
    let equal = s.constraints.iter().filter(|c| matches!(c, Constraint::Equal { .. })).count();
    let parallel = s.constraints.iter().filter(|c| matches!(c, Constraint::Parallel { .. })).count();
    assert!(equal == 1 && parallel == 0, "the top and the bottom: {equal} Equal, {parallel} Parallel: {:?}", s.constraints);
    assert_eq!(p.sketch_dof(si).1, 0, "the lines left are over-defined");
}

/// A rectangle broken by deleting one of its own constraints - the four sides stay - leaves them related the same way.
#[test]
fn a_rectangle_broken_by_a_constraint_leaves_its_sides_related() {
    let (mut p, si) = drawn(false);
    let r = p.sketches[si].rects()[0].clone();
    let ci = p.sketches[si].constraints.iter().position(|c| c.points().iter().all(|q| r.corners.contains(q)) && !matches!(c, Constraint::Distance { .. })).expect("an own constraint of the rectangle");
    assert!(p.delete_sketch_constraint(si, ci), "the constraint deleted");
    assert!(p.sketches[si].rects().is_empty(), "setup: the rectangle is broken");
    let s = &p.sketches[si];
    let (h, v) = (s.constraints.iter().filter(|c| matches!(c, Constraint::Horizontal { .. })).count(), s.constraints.iter().filter(|c| matches!(c, Constraint::Vertical { .. })).count());
    assert!(h == 2 && v == 2, "four sides of a rectangle broken by a constraint: {h} Horizontal, {v} Vertical: {:?}", s.constraints);
    assert_eq!(p.sketch_dof(si).1, 0, "the sides left are over-defined");
}
