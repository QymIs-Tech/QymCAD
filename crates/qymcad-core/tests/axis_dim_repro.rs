//! A dimension to a coordinate axis is consistent and is not a conflict, so nothing goes red.
//!
//! The visual symptoms — stray anchor glyphs on system points and degenerate drawing — belong to the
//! interface.
use qymcad_core::model::{Constraint, Project};

fn dpl_to_axis(p: &mut Project, si: usize, pid: u64, which: usize) -> usize {
    let (o, ax) = p.ensure_axis(si, which);
    let (px, py) = { let q = p.sketches[si].points.iter().find(|q| q.id == pid).unwrap(); (q.x, q.y) };
    let (ox, oy) = { let q = p.sketches[si].points.iter().find(|q| q.id == o).unwrap(); (q.x, q.y) };
    let (bx, by) = { let q = p.sketches[si].points.iter().find(|q| q.id == ax).unwrap(); (q.x, q.y) };
    let (dx, dy) = (bx - ox, by - oy);
    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
    let d = (dx * (py - oy) - dy * (px - ox)) / len;
    p.sketches[si].constraints.push(Constraint::DistancePL { p: pid, a: o, b: ax, d, off: 0.0, expr: String::new(), driven: false });
    p.sketches[si].constraints.len() - 1
}

#[test]
fn axis_dims_are_consistent_not_conflicting() {
    let mut p = Project::default();
    p.new_document();
    let sid = p.add_sketch("t", vec![], None);
    p.add_sketch_node(sid, "Sketch");
    let si = p.sketch_index(sid).unwrap();

    p.add_line_entity(si, 5.0, 3.0, 15.0, 3.0, qymcad_core::feature::Purpose::Real);
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 5.0).abs() < 1e-6 && (q.y - 3.0).abs() < 1e-6).unwrap().id;

    dpl_to_axis(&mut p, si, corner, 0);
    dpl_to_axis(&mut p, si, corner, 1);
    p.solve_sketch(si);

    // the position of the corner is held by dimensions to the axes and the values agree, so this is no
    // conflict
    assert!(p.sketch_conflicts(si).is_empty(), "dimensions to the axes must not conflict");
    let q = p.sketches[si].points.iter().find(|q| q.id == corner).unwrap();
    assert!((q.x - 5.0).abs() < 1e-3 && (q.y - 3.0).abs() < 1e-3, "the corner is in place: {:?}", (q.x, q.y));
}
