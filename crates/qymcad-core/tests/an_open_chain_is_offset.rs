//! AN OPEN CHAIN IS OFFSET to one side of its way, positive to the left: its joints kept - the convex ones rounded about
//! the source corner, the concave ones cut - its ends square to the ends of the source, and held to the source, so the
//! copy adds no freedom of its own and follows the source when that is changed.
//!
//! Reported behaviour: a polyline of three lines chosen, Offset, a distance - no copy, as though nothing were chosen.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{EntityKind, Project};

/// A sketch with the chain through `corners`, lines between them, joined end to end.
fn chain(corners: &[(f64, f64)]) -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    for w in corners.windows(2) {
        p.add_line_entity(si, w[0].0, w[0].1, w[1].0, w[1].1, Purpose::Real);
    }
    p.merge_close_points(si, 1e-9);
    (p, si)
}

/// Whether a point of the sketch stands at (x, y).
fn stands(p: &Project, si: usize, x: f64, y: f64) -> bool {
    p.sketches[si].points.iter().any(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6)
}

fn arcs(p: &Project, si: usize) -> usize {
    p.sketches[si].entities.iter().filter(|e| matches!(e.kind, EntityKind::Arc { .. })).count()
}

/// The point of the sketch standing at (x, y).
fn point_at(p: &Project, si: usize, x: f64, y: f64) -> u64 {
    p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-9 && (q.y - y).abs() < 1e-9).map(|q| q.id).expect("the point")
}

#[test]
fn a_polyline_is_offset_to_its_left_its_joints_kept_and_held() {
    // east, north, east: the left of the way is up, then to the west, then up again. Held as a person holds a drawing:
    // its start anchored, the first and the last line level, the middle one upright
    let (mut p, si) = chain(&[(0.0, 0.0), (20.0, 0.0), (20.0, 15.0), (40.0, 15.0)]);
    let [o, c1, c2, e] = [(0.0, 0.0), (20.0, 0.0), (20.0, 15.0), (40.0, 15.0)].map(|(x, y)| point_at(&p, si, x, y));
    p.sketches[si].constraints.extend([
        qymcad_core::model::Constraint::Fixed { p: o },
        qymcad_core::model::Constraint::Horizontal { a: o, b: c1 },
        qymcad_core::model::Constraint::Vertical { a: c1, b: c2 },
        qymcad_core::model::Constraint::Horizontal { a: c2, b: e },
    ]);
    p.solve_sketch(si);
    let before = p.sketch_dof(si);
    let all: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let made = p.offset_entities(si, &all, 3.0);
    let mut failures = Vec::new();
    // the concave joint (20, 0) cut to (17, 3); the convex joint (20, 15) rounded about it from (17, 15) to (20, 18)
    let want = [(0.0, 3.0), (17.0, 3.0), (17.0, 15.0), (20.0, 18.0), (40.0, 18.0)];
    let missing: Vec<_> = want.iter().filter(|&&(x, y)| !stands(&p, si, x, y)).collect();
    if made != 1 || !missing.is_empty() || arcs(&p, si) != 1 {
        failures.push(format!("the polyline offset 3: {made} copies, nothing at {missing:?}, {} arcs", arcs(&p, si)));
    }
    if p.sketch_dof(si) != before {
        failures.push(format!("the copy is not held to its source: freedoms and redundant {:?}, the source alone {before:?}", p.sketch_dof(si)));
    }
    // the upper corner of the source dragged up 5, to (20, 20): the copy follows - its rounded joint up to (17, 20) -
    // (20, 23), its last line at 23, its cut joint (17, 3) kept
    p.solve_sketch_drag(si, Some((c2, 20.0, 20.0)));
    p.solve_sketch(si);
    // the far end of the source is free along its line: the end of the copy stands 3 above it, wherever it went
    let far = p.sketches[si].points.iter().find(|q| q.id == e).map(|q| (q.x, q.y)).expect("the far end");
    if !stands(&p, si, 17.0, 20.0) || !stands(&p, si, 20.0, 23.0) || !stands(&p, si, far.0, far.1 + 3.0) || !stands(&p, si, 17.0, 3.0) {
        failures.push(format!("the source corner moved to (20, 20): the copy does not follow; the sketch shows {:?}", p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>()));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_single_line_an_arc_and_a_chain_with_an_arc_are_offset() {
    let mut failures = Vec::new();
    // one line, offset 2 to its left: (0, 2) - (30, 2)
    let (mut p, si) = chain(&[(0.0, 0.0), (30.0, 0.0)]);
    let all: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let before = p.sketch_dof(si);
    if p.offset_entities(si, &all, 2.0) != 1 || !stands(&p, si, 0.0, 2.0) || !stands(&p, si, 30.0, 2.0) || p.sketch_dof(si) != before {
        failures.push(format!("a line offset 2: the sketch shows {:?}, freedoms {:?} against {before:?}", p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>(), p.sketch_dof(si)));
    }
    // one arc about the origin of radius 10, counter-clockwise from (10, 0) to (0, 10); its left is inside: radius 8
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(0.0, 10.0), qymcad_core::feature::Winding::Ccw, Purpose::Real);
    let all: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let before = p.sketch_dof(si);
    if p.offset_entities(si, &all, 2.0) != 1 || !stands(&p, si, 8.0, 0.0) || !stands(&p, si, 0.0, 8.0) || p.sketch_dof(si) != before {
        failures.push(format!("an arc offset 2: the sketch shows {:?}, freedoms {:?} against {before:?}", p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>(), p.sketch_dof(si)));
    }
    // a line (-20, 10) - (0, 10) going on into the arc above, from (0, 10) round to (10, 0), clockwise: left is outside
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, -20.0, 10.0, 0.0, 10.0, Purpose::Real);
    p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(0.0, 10.0), Point2::new(10.0, 0.0), qymcad_core::feature::Winding::Cw, Purpose::Real);
    p.merge_close_points(si, 1e-9);
    let all: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let before = p.sketch_dof(si);
    if p.offset_entities(si, &all, 2.0) != 1 || !stands(&p, si, -20.0, 12.0) || !stands(&p, si, 0.0, 12.0) || !stands(&p, si, 12.0, 0.0) || p.sketch_dof(si) != before {
        failures.push(format!(
            "a line going on into an arc, offset 2: the sketch shows {:?}, freedoms {:?} against {before:?}",
            p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>(),
            p.sketch_dof(si)
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
