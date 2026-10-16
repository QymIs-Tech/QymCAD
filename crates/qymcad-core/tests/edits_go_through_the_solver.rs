//! EVERY EDIT THAT MOVES SKETCH GEOMETRY ENDS AT THE SOLVER.
//!
//! Reported behaviour: "I took the move tool, moved the circle far away, and the coincidence with the
//! origin stayed - and stayed GREEN. Reopen the project and the circle is back." The edit shifted the
//! coordinates and never asked the solver, so the screen showed a shape standing where its constraints do
//! not allow it to stand, and the lie held until the next edit put it back.
//!
//! The move tool was repaired at its call site in the interface, which leaves the core method itself still
//! able to lie to the next caller. These tests hold the rule at the layer where the edit lives: after the
//! call, the constraints of the sketch are satisfied.
use qymcad_core::model::{Constraint, EntityKind, Project};

fn new_sketch() -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();
    (p, si)
}

/// The endpoints of the first line, as ids and as positions.
fn line(p: &Project, si: usize) -> ((u64, f64, f64), (u64, f64, f64)) {
    let (a, b) = p.sketches[si]
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some((a, b)),
            _ => None,
        })
        .expect("the sketch holds a line");
    let at = |id: u64| {
        let q = p.sketches[si].points.iter().find(|q| q.id == id).expect("the point exists");
        (id, q.x, q.y)
    };
    (at(a), at(b))
}

/// A COINCIDENCE WITH SOMETHING THAT IS NOT MOVING HOLDS.
///
/// This is the reported case itself: a shape tied to a point outside the selection was carried away, the tie
/// stayed drawn and stayed green, and the next edit put the shape back. The move lifts the ties of what goes to what
/// stays: the shape goes where it was taken, and no tie is left drawn that does not hold.
///
/// `Constraint::Fixed` would not show it: it pins a point to where it IS, not to a remembered place, so a
/// move takes the anchor along and the constraint is satisfied by the new position. That case is held by
/// `an_editing_tool_leaves_a_pinned_point_where_it_is`, not here.
#[test]
fn moving_a_shape_lifts_its_coincidence_with_a_point_left_behind() {
    let (mut p, si) = new_sketch();
    let moved = p.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    // A second line standing apart, so its end is a point of its own rather than a shared node.
    let _still = p.add_line_entity(si, 10.0, 20.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
    let ends = |p: &Project, eid: u64| {
        p.sketches[si]
            .entities
            .iter()
            .find(|e| e.id == eid)
            .and_then(|e| match e.kind {
                EntityKind::Line { a, b } => Some((a, b)),
                _ => None,
            })
            .expect("the line is there")
    };
    let (ma, _) = ends(&p, moved);
    let (sa, _) = ends(&p, _still);
    p.sketches[si].constraints.push(Constraint::Coincident { a: ma, b: sa });

    p.move_entities(si, &[moved], 5.0, 3.0);

    let at = |id: u64| {
        let q = p.sketches[si].points.iter().find(|q| q.id == id).expect("the point exists");
        (q.x, q.y)
    };
    let (u, v) = (at(ma), at(sa));
    let tied = p.sketches[si].constraints.iter().any(|c| matches!(*c, Constraint::Coincident { a, b } if (a, b) == (ma, sa)));
    assert!(!tied && (u.0 - 15.0).hypot(u.1 - 3.0) < 1e-6, "the moved end stands at {u:?}, the end left behind at {v:?}, the coincidence kept: {tied}");
}

#[test]
fn rotating_a_horizontal_line_leaves_it_horizontal() {
    let (mut p, si) = new_sketch();
    let eid = p.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    let (a, b) = line(&p, si);
    p.sketches[si].constraints.push(Constraint::Horizontal { a: a.0, b: b.0 });

    p.rotate_entities(si, &[eid], 10.0, 0.0, 30.0);

    let (a2, b2) = line(&p, si);
    eprintln!("after a 30 deg turn the ends sit at y={} and y={}", a2.2, b2.2);
    assert!((a2.2 - b2.2).abs() < 1e-6, "the line is no longer horizontal though a horizontal constraint holds it: y={} and y={}", a2.2, b2.2);
}

#[test]
fn scaling_a_dimensioned_line_leaves_the_dimension_true() {
    let (mut p, si) = new_sketch();
    let eid = p.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
    let (a, b) = line(&p, si);
    p.sketches[si].constraints.push(Constraint::Distance { a: a.0, b: b.0, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });

    p.scale_entities(si, &[eid], 10.0, 0.0, 2.0);

    let (a2, b2) = line(&p, si);
    let len = ((b2.1 - a2.1).powi(2) + (b2.2 - a2.2).powi(2)).sqrt();
    eprintln!("the dimension says 20, the line measures {len}");
    assert!((len - 20.0).abs() < 1e-6, "the dimension says 20 and the line is {len} long");
}

/// EVERY EDITING TOOL AT ONCE, one row per tool.
///
/// Fixing them one at a time is how the defect came back through the next door: the move tool was repaired
/// in the interface while rotate, scale, trim, extend and break went on shifting points with nobody asking
/// the solver. The rule is one, so it is held in one place.
///
/// The setup is the same for every row: two lines end in the same spot with points of their OWN, tied by a
/// coincidence. The tool is used on the first line; whatever it does to it, the tie either holds - the drawing
/// follows, or the tool is refused - or is lifted, as Move and Rotate lift the ties of what goes to what stays. What
/// must never happen is a tie that is drawn, is counted as satisfied and is not true.
#[test]
fn no_editing_tool_leaves_a_constraint_unsatisfied() {
    // moved line (10,0)-(30,0), standing line (30,0)-(50,0), the two inner ends tied
    let build = || {
        let (mut p, si) = new_sketch();
        let mover = p.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        let ends = |p: &Project, eid: u64| {
            p.sketches[si]
                .entities
                .iter()
                .find(|e| e.id == eid)
                .and_then(|e| match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    _ => None,
                })
                .expect("the line is there")
        };
        let (_, mb) = ends(&p, mover);
        // THE STANDING LINE IS BUILT BY HAND, because drawing it from (30,0) would REUSE the end already
        // there and the two would be one node - and a shared node is not what is under test. Two points in
        // one place tied by a coincidence is the ordinary state of a sketch: it is what a coincidence made
        // by hand leaves behind.
        let (sa, sb, seid) = (p.alloc_id(), p.alloc_id(), p.alloc_id());
        let s = &mut p.sketches[si];
        s.points.push(qymcad_core::model::SketchPoint { id: sa, x: 30.0, y: 0.0 });
        s.points.push(qymcad_core::model::SketchPoint { id: sb, x: 50.0, y: 0.0 });
        s.entities.push(qymcad_core::model::SketchEntity { id: seid, kind: EntityKind::Line { a: sa, b: sb }, construction: false });
        assert_ne!(mb, sa, "setup: the two ends have to be points of their own, not one shared node");
        p.sketches[si].constraints.push(Constraint::Coincident { a: mb, b: sa });
        (p, si, mover, mb, sa)
    };

    /// An editing tool by its name, and what it does to entity `e` of sketch `si`.
    struct Row {
        name: &'static str,
        act: fn(&mut Project, usize, u64),
    }
    fn row(name: &'static str, act: fn(&mut Project, usize, u64)) -> Row {
        Row { name, act }
    }
    let rows: [Row; 6] = [
        row("move", |p, si, e| p.move_entities(si, &[e], 5.0, 3.0)),
        row("rotate", |p, si, e| p.rotate_entities(si, &[e], 10.0, 0.0, 30.0)),
        row("scale", |p, si, e| p.scale_entities(si, &[e], 10.0, 0.0, 2.0)),
        row("trim", |p, si, e| {
            p.add_line_entity(si, 20.0, -10.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real); // the cutting line
            p.trim_line(si, e, 25.0, 0.0);
        }),
        row("extend", |p, si, e| {
            p.add_line_entity(si, 45.0, -10.0, 45.0, 10.0, qymcad_core::feature::Purpose::Real); // the line extended up to
            p.extend_line(si, e, 29.0, 0.0);
        }),
        row("break", |p, si, e| {
            p.break_line(si, e, 20.0, 0.0);
        }),
    ];

    let mut sins: Vec<String> = Vec::new();
    for Row { name, act } in rows {
        let (mut p, si, mover, mb, sa) = build();
        act(&mut p, si, mover);
        let at = |id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        // A point may legitimately disappear (a break replaces the line with two halves); a tie whose end is gone
        // is not a broken tie.
        let kept = p.sketches[si].constraints.iter().any(|c| matches!(*c, Constraint::Coincident { a, b } if (a, b) == (mb, sa)));
        if let (Some(u), Some(v), true) = (at(mb), at(sa), kept) {
            let gap = (u.0 - v.0).hypot(u.1 - v.1);
            if gap > 1e-6 {
                sins.push(format!("{name}: the tie was torn - {u:?} against {v:?}, a gap of {gap:.3} mm"));
            }
        }
    }
    assert!(sins.is_empty(), "edits that left their constraints unsatisfied:\n{}", sins.join("\n"));
}

/// A PINNED POINT DOES NOT TRAVEL WITH THE EDIT.
///
/// `Constraint::Fixed` holds no coordinates: it pins the point to where it IS at the moment of the solve.
/// So an edit that shifts the point first and asks the solver afterwards gets a constraint that is satisfied
/// by the new place - the pin follows the drawing instead of holding it. On screen the glyph stays green and
/// the person is told nothing.
///
/// The rule: an editing tool leaves pinned points where they are. What the rest of the shape does is then
/// the solver's business - it may follow, and it may be held.
#[test]
fn an_editing_tool_leaves_a_pinned_point_where_it_is() {
    let mut sins: Vec<String> = Vec::new();
    /// An editing tool by its name, and what it does to entity `e` of sketch `si`.
    struct Row {
        name: &'static str,
        act: fn(&mut Project, usize, u64),
    }
    fn row(name: &'static str, act: fn(&mut Project, usize, u64)) -> Row {
        Row { name, act }
    }
    let rows: [Row; 3] = [
        row("move", |p, si, e| p.move_entities(si, &[e], 5.0, 3.0)),
        row("rotate", |p, si, e| p.rotate_entities(si, &[e], 0.0, 0.0, 30.0)),
        row("scale", |p, si, e| p.scale_entities(si, &[e], 0.0, 0.0, 2.0)),
    ];
    for Row { name, act } in rows {
        let (mut p, si) = new_sketch();
        let eid = p.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        let (a, _) = {
            let (u, v) = p.sketches[si]
                .entities
                .iter()
                .find_map(|e| match e.kind {
                    EntityKind::Line { a, b } => Some((a, b)),
                    _ => None,
                })
                .expect("the line is there");
            (u, v)
        };
        p.sketches[si].constraints.push(Constraint::Fixed { p: a });

        act(&mut p, si, eid);

        let at = p.sketches[si].points.iter().find(|q| q.id == a).map(|q| (q.x, q.y));
        match at {
            Some((x, y)) if (x - 10.0).abs() < 1e-6 && y.abs() < 1e-6 => {}
            Some((x, y)) => sins.push(format!("{name}: the pinned point travelled from (10, 0) to ({x}, {y})")),
            None => sins.push(format!("{name}: the pinned point is gone")),
        }
    }
    assert!(sins.is_empty(), "edits that carried a pinned point along:\n{}", sins.join("\n"));
}

/// COLLINEAR KEEPS BOTH LINES WHOLE: the second line comes onto the line of the first, and neither shrinks to a point.
///
/// Reported behaviour (found checking issue #34): two lines made collinear came out with the first one shrunk to a point
/// and the residual 0 - the residual was a cross product not divided by the length, met by a line of no length.
#[test]
fn collinear_keeps_both_lines_whole() {
    let (mut p, si) = new_sketch();
    let l1 = p.add_line_entity(si, 0.0, 0.0, 5.0, 12.0, qymcad_core::feature::Purpose::Real);
    let l2 = p.add_line_entity(si, 20.0, 0.0, 29.0, 12.0, qymcad_core::feature::Purpose::Real);
    let ends = |p: &Project, e: u64| match p.sketches[si].entities.iter().find(|x| x.id == e).map(|x| &x.kind) {
        Some(EntityKind::Line { a, b }) => (*a, *b),
        _ => panic!("a line"),
    };
    let ((a, b), (c, d)) = (ends(&p, l1), ends(&p, l2));
    p.sketches[si].constraints.push(Constraint::Collinear { a, b, c, d });
    let resid = p.solve_sketch(si);
    let at = |id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    let len = |u: u64, v: u64| (at(u).0 - at(v).0).hypot(at(u).1 - at(v).1);
    let off = |q: u64| {
        let (pa, pb, pq) = (at(a), at(b), at(q));
        ((pb.0 - pa.0) * (pq.1 - pa.1) - (pb.1 - pa.1) * (pq.0 - pa.0)).abs() / len(a, b).max(1e-12)
    };
    assert!(resid < 1e-6, "the sketch did not solve: residual {resid}");
    assert!(len(a, b) > 1.0 && len(c, d) > 1.0, "a line shrank: the first {:.4} long, the second {:.4}", len(a, b), len(c, d));
    assert!(off(c) < 1e-6 && off(d) < 1e-6, "the second line is off the first: {:.2e}, {:.2e}", off(c), off(d));
}

/// A RELATION ALREADY HELD IS NOT LAID AGAIN, AND ONE A NEW RELATION IMPLIES IS LIFTED: `independent_of` drops a second
/// Vertical on a line that has one; `drop_implied_relations` lifts the Vertical of a line that Collinear with a vertical
/// line makes redundant, and leaves the sketch with nothing redundant.
#[test]
fn a_held_relation_is_not_laid_again_and_an_implied_one_is_lifted() {
    let (mut p, si) = new_sketch();
    let ends = |p: &Project, e: u64| match p.sketches[si].entities.iter().find(|x| x.id == e).map(|x| &x.kind) {
        Some(EntityKind::Line { a, b }) => (*a, *b),
        _ => panic!("a line"),
    };
    let l1 = p.add_line_entity(si, 0.0, 0.0, 0.0, 12.0, qymcad_core::feature::Purpose::Real);
    let l2 = p.add_line_entity(si, 20.0, 0.0, 20.0, 12.0, qymcad_core::feature::Purpose::Real);
    let ((a, b), (c, d)) = (ends(&p, l1), ends(&p, l2));
    p.sketches[si].constraints.push(Constraint::Vertical { a, b });
    p.sketches[si].constraints.push(Constraint::Vertical { a: c, b: d });
    p.solve_sketch(si);

    let again = p.independent_of(si, vec![Constraint::Vertical { a, b }, Constraint::Vertical { a: c, b: d }]);
    assert!(again.is_empty(), "a second Vertical on a vertical line was kept: {again:?}");

    let had = p.sketches[si].constraints.len();
    let new = p.independent_of(si, vec![Constraint::Collinear { a, b, c, d }]);
    assert_eq!(new.len(), 1, "Collinear constrains the second line onto the first and must be kept");
    p.sketches[si].constraints.extend(new);
    p.solve_sketch(si);
    let among: std::collections::HashSet<u64> = [a, b, c, d].into_iter().collect();
    let lifted = p.drop_implied_relations(si, had, &among);
    let vertical = p.sketches[si].constraints.iter().filter(|k| matches!(k, Constraint::Vertical { .. })).count();
    assert_eq!(lifted, 1, "one Vertical follows from the other and Collinear");
    assert_eq!(vertical, 1, "one Vertical is left");
    assert!(p.sketch_redundant_constraints(si).is_empty(), "something is still redundant: {:?}", p.sketch_redundant_constraints(si));
}
