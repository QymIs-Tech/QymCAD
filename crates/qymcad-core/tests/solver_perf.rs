//! A sentinel: the solver has to stay fast on a large sketch, thanks to the sparse Jacobian.
//!
//! The bounds are generous, build machines being noisy, but they catch a return to a dense O(nv·m) Jacobian with
//! an O(nv²·m) JᵀJ. They hold the solve's work, not the share of the machine it was given (`time_budget::Cost`; the
//! parts of a sketch are solved on threads of their own, so the process is the measure).

use qymcad_core::model::Project;
use qymcad_core::time_budget::{cost_of, over, Doer};
use std::time::Duration;

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
    let full = cost_of(Doer::Process, || p.solve_sketch(si));
    let resid = full.out;
    eprintln!("[perf] points={np} constraints={nc} full solve: {}, resid={resid:.2e}", full.cost);
    assert!(resid < 1e-3, "it solved: resid={resid:.2e}");
    let budget = Duration::from_millis(3000);
    assert!(full.cost.work < budget, "{} - against more than 10 s on a dense Jacobian", over("a full solve of a large sketch", full.cost, budget));
    // a drag iteration: the fast path taken on every mouse frame
    let some_pt = p.sketches[si].points[10].id;
    let drag = cost_of(Doer::Process, || {
        for k in 0..10 {
            p.solve_sketch_drag_fast(si, Some((some_pt, 5.0 + k as f64, 5.0)));
        }
    });
    eprintln!("[perf] ten drag frames: {}", drag.cost);
    let budget = Duration::from_millis(150) * 10;
    assert!(drag.cost.work < budget, "{} - 150 ms a frame keeps the interaction alive", over("ten drag frames on a large sketch", drag.cost, budget));
}
