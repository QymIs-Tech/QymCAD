//! A sentinel: the solver has to stay fast on a large sketch, thanks to the sparse Jacobian.
//!
//! The bounds are generous, build machines being noisy, but they catch a return to a dense O(nv·m) Jacobian with
//! an O(nv²·m) JᵀJ.

use qymcad_core::model::Project;
use std::time::Instant;

/// An 8×8 grid of rectangles with the horizontal and vertical constraints the tool creates: about 512 points
/// and 640 constraints.
fn big_sketch() -> (Project, usize) {
    let mut p = Project::default();
    let si = p.new_sketch("big");
    for gy in 0..8 {
        for gx in 0..8 {
            let (x0, y0) = (gx as f64 * 30.0, gy as f64 * 30.0);
            p.add_rect_entity(si, x0, y0, x0 + 20.0, y0 + 20.0, qymcad_core::feature::Purpose::Real);
        }
    }
    (p, si)
}

#[test]
fn big_sketch_solve_is_fast() {
    let (mut p, si) = big_sketch();
    let np = p.sketches[si].points.len();
    let nc = p.sketches[si].constraints.len();
    // a warm-up plus a full solve, 120 iterations
    let t = Instant::now();
    let resid = p.solve_sketch(si);
    let full_ms = t.elapsed().as_millis();
    eprintln!("[perf] points={np} constraints={nc} full solve: {full_ms} ms, resid={resid:.2e}");
    assert!(resid < 1e-3, "it solved: resid={resid:.2e}");
    assert!(full_ms < 3000, "a full solve of a large sketch fits the budget: {full_ms} ms, against more than 10 s on a dense Jacobian");
    // a drag iteration: the fast path taken on every mouse frame
    let some_pt = p.sketches[si].points[10].id;
    let t = Instant::now();
    for k in 0..10 {
        p.solve_sketch_drag_fast(si, Some((some_pt, 5.0 + k as f64, 5.0)));
    }
    let drag_ms = t.elapsed().as_millis() / 10;
    eprintln!("[perf] drag frame: {drag_ms} ms");
    assert!(drag_ms < 150, "a drag frame on a large sketch: {drag_ms} ms, so interaction stays alive");
}
