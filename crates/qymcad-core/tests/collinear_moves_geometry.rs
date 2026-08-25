//! Making a rectangle edge collinear with a construction line did not move it.
//!
//! The rectangle stayed 11 microns off the reference line, so the cut left a film instead of an opening; the
//! only thing that helped was widening it to 5.02.
//!
//! The configuration is reproduced exactly: a construction vertical fixed by a dimension at x = −130, a
//! rectangle 5 wide standing at −130.011, both lines already vertical. Collinearity has to move the rectangle
//! onto the line.
use qymcad_core::model::{Constraint, Project};

#[test]
fn collinear_pulls_the_rect_onto_the_reference_line() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    // a construction vertical at x = −130, standing in for a projected face, with its ends anchored
    let aux = p.add_line_entity(si, -130.0, 25.0, -130.0, -234.0, qymcad_core::feature::Purpose::Construction);
    // the slot rectangle: 5 wide, its left edge 11 microns off
    let rect = p.add_rect_entity(si, -130.011, -176.0, -125.011, -36.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);

    let aux_pts = {
        let s = &p.sketches[si];
        let e = s.entities.iter().find(|e| e.id == aux).expect("the construction line");
        match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => (a, b),
            _ => panic!("not a line"),
        }
    };
    for pt in [aux_pts.0, aux_pts.1] {
        p.sketches[si].constraints.push(Constraint::Fixed { p: pt });
    }
    // the left side of the rectangle: two points at x of about −130.011
    let left: Vec<_> = {
        let s = &p.sketches[si];
        s.points.iter().filter(|pt| (pt.x + 130.011).abs() < 1e-6).map(|pt| pt.id).collect()
    };
    assert_eq!(left.len(), 2, "the left side of the slot has two points");
    let _ = rect;

    p.sketches[si].constraints.push(Constraint::Collinear { a: aux_pts.0, b: aux_pts.1, c: left[0], d: left[1] });
    let resid = p.solve_sketch(si);
    p.regen_sketch(si);
    eprintln!("residual after the solve: {resid:.6}");

    let s = &p.sketches[si];
    for id in &left {
        let pt = s.points.iter().find(|q| q.id == *id).unwrap();
        eprintln!("x = {:.12}", pt.x);
        assert!(
            (pt.x + 130.0).abs() < 1e-4,
            "collinearity did not move the side of the slot: x={:.4}, expecting -130.0, leaving {:.4} mm of film",
            pt.x,
            (pt.x + 130.0).abs()
        );
    }
}
