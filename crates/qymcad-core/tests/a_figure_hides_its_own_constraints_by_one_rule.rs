//! A FIGURE HIDES ITS OWN CONSTRAINTS BY ONE RULE: a rectangle in every way it is drawn, a rectangle whose turn an angle
//! dimension took, a polygon and a slot each hold their shape by constraints of their own, and the shapes of the sketch
//! (`held_by_shapes`) name every one of them - what the list and the badges leave out. A constraint a person laid on the
//! figure, and its dimensions, are not the figure's and stay shown.
//!
//! Reported behaviour: a rectangle held its turn hidden by the kind of the constraint and its centre by a rule of the
//! badges alone; once an angle dimension took its turn, its parallel and square sides came out as badges and rows of the
//! list, where a polygon and a slot showed none of theirs.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Project};

/// A FIGURE DRAWN INTO A SKETCH by a case, and the constraints a person laid on it afterwards, by index.
struct Drawn {
    p: Project,
    si: usize,
    laid: Vec<usize>,
}

/// An empty sketch, nothing laid on it yet.
fn sketch() -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    Drawn { p, si, laid: Vec::new() }
}

/// The figure's constraints, by index: every constraint of the sketch but a dimension and those laid afterwards.
fn own(d: &Drawn) -> Vec<usize> {
    let dimension = |c: &Constraint| matches!(c, Constraint::Diameter { .. } | Constraint::Distance { .. } | Constraint::AngleLines { .. });
    d.p.sketches[d.si].constraints.iter().enumerate().filter(|(k, c)| !dimension(c) && !d.laid.contains(k)).map(|(k, _)| k).collect()
}

/// A corner of the rectangle fixed by a person: laid on the figure, not the figure's.
fn fix_a_corner(mut p: Project, si: usize) -> Drawn {
    let corner = p.sketches[si].rects()[0].corners[0];
    p.sketches[si].constraints.push(Constraint::Fixed { p: corner });
    let laid = vec![p.sketches[si].constraints.len() - 1];
    Drawn { p, si, laid }
}

fn by_corners() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    p.add_rect_entity(si, 10.0, 10.0, 50.0, 40.0, Purpose::Real);
    fix_a_corner(p, si)
}

fn from_centre() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    p.add_rect_from_centre(si, Point2::new(30.0, 25.0), Point2::new(50.0, 40.0), Purpose::Real);
    fix_a_corner(p, si)
}

fn by_three_points() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    p.add_rect3_entity(si, Point2::new(10.0, 10.0), Point2::new(10.0 + 40.0 * c, 10.0 + 40.0 * s), Point2::new(10.0 + 40.0 * c - 30.0 * s, 10.0 + 40.0 * s + 30.0 * c), Purpose::Real);
    fix_a_corner(p, si)
}

/// A rectangle whose turn an angle dimension to a fixed line took: it holds its shape by parallel and square sides.
fn turned_by_an_angle() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    p.add_rect_entity(si, 10.0, 10.0, 50.0, 40.0, Purpose::Real);
    let l = p.add_line_entity(si, 0.0, 0.0, 30.0, -10.0, Purpose::Real);
    let ends = p.sketches[si].entities.iter().find(|e| e.id == l).and_then(|e| e.kind.as_line()).expect("the line");
    let r = p.sketches[si].rects()[0].clone();
    let dim = Constraint::AngleLines { a: ends.a, b: ends.b, c: r.corners[0], d: r.corners[1], deg: 20.0, expr: String::new(), driven: false, off: 0.0, at: None };
    p.give_rect_turn_to(si, &dim);
    let s = &mut p.sketches[si];
    s.constraints.extend([Constraint::Fixed { p: ends.a }, Constraint::Fixed { p: ends.b }, dim]);
    let n = s.constraints.len();
    p.solve_sketch(si);
    Drawn { p, si, laid: vec![n - 3, n - 2] }
}

fn polygon() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    let (centre, _) = p.add_polygon_param(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 6, Purpose::Real);
    p.sketches[si].constraints.push(Constraint::Fixed { p: centre });
    let laid = vec![p.sketches[si].constraints.len() - 1];
    Drawn { p, si, laid }
}

fn slot() -> Drawn {
    let Drawn { mut p, si, .. } = sketch();
    p.add_slot_entity(si, Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), 4.0, Purpose::Real);
    let side = p.sketches[si].entities.iter().find_map(|e| e.kind.as_line()).expect("a side");
    p.sketches[si].constraints.push(Constraint::Horizontal { a: side.a, b: side.b });
    let laid = vec![p.sketches[si].constraints.len() - 1];
    Drawn { p, si, laid }
}

/// A FIGURE OF THE MATRIX: its name and how it is drawn.
struct Case {
    name: &'static str,
    draw: fn() -> Drawn,
}

#[test]
fn every_figure_hides_its_own_constraints_and_nothing_laid_on_it() {
    let cases = [
        Case { name: "a rectangle by two corners", draw: by_corners },
        Case { name: "a rectangle from its centre", draw: from_centre },
        Case { name: "a rectangle by three points", draw: by_three_points },
        Case { name: "a rectangle turned by an angle dimension", draw: turned_by_an_angle },
        Case { name: "a polygon", draw: polygon },
        Case { name: "a slot", draw: slot },
    ];
    let mut failures = Vec::new();
    for Case { name, draw } in cases {
        let d = draw();
        let s = &d.p.sketches[d.si];
        let held = s.held_by_shapes();
        let own = own(&d);
        let shown: Vec<String> = own.iter().filter(|k| !held.contains(k)).map(|k| format!("{:?}", s.constraints[*k])).collect();
        let hidden: Vec<String> = held.iter().filter(|k| !own.contains(k)).map(|k| format!("{:?}", s.constraints[*k])).collect();
        if own.is_empty() || !shown.is_empty() || !hidden.is_empty() {
            failures.push(format!("{name}: {} own constraints; shown of them: {shown:?}; hidden not of them: {hidden:?}", own.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
