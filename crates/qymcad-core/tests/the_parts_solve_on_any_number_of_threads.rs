//! THE PARTS OF A SKETCH ARE SOLVED ON THE THREADS THE SETTING GIVES, and come out the same on any number of them. A
//! file of its own: the number of threads is one for the process (`solver::set_threads`).
mod sketch_kinds;

use qymcad_core::solver;
use sketch_kinds::{build, Kind};
use qymcad_core::time_budget::{cost_of, Doer};

/// The checks of this file set the number of threads one after another, never side by side: run together, one set a
/// single thread in the middle of the other's measure.
static THREADS_HELD: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn the_parts_come_out_the_same_on_one_thread_and_on_many() {
    let _held = THREADS_HELD.lock().unwrap_or_else(|e| e.into_inner());
    let many = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(2);
    for kind in [Kind::Tangents, Kind::Mixed, Kind::Rectangles] {
        let p = build(kind, 2_000);
        let s = &p.sketches[0];
        let solve = |threads: usize| {
            solver::set_threads(threads);
            let (mut points, mut radii) = (s.points.clone(), Vec::new());
            let r = solver::solve_full_iter(&mut points, &mut radii, &s.constraints, None, 120);
            (points, r)
        };
        let ((one, r_one), (all, r_all)) = (solve(1), solve(many));
        let same = one.iter().zip(&all).all(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits());
        assert!(same && r_one.to_bits() == r_all.to_bits(), "{kind:?}: the answer on {many} threads differs from the one on one thread");
    }
    solver::set_threads(0);
}

#[test]
fn many_separate_shapes_are_solved_on_the_threads_of_the_setting() {
    let _held = THREADS_HELD.lock().unwrap_or_else(|e| e.into_inner());
    solver::set_threads(0);
    let many = solver::threads();
    if many < 4 {
        eprintln!("PASSED OVER: {many} threads on this machine, the measure needs 4");
        return;
    }
    // 30 000 tangent lines with circles, on one thread and on the setting's, each the best of two: told against each
    // other rather than against a clock. Each is its work (`time_budget::Cost`): the wall less the share its threads
    // stood ready with no processor - timed by the wall, the threads of a machine busy with other work have no free
    // processor to run on, and the many read as slow as the one. Measured idle in a test build: 3.3 s on one thread,
    // 1.2 s on 19.
    let p = build(Kind::Tangents, 30_000);
    let timed = |threads: usize| {
        solver::set_threads(threads);
        (0..2)
            .map(|_| {
                let mut copy = p.clone();
                cost_of(Doer::Process, || copy.solve_sketch(0)).cost.work
            })
            .min()
            .unwrap_or_default()
    };
    let (one, all) = (timed(1), timed(many));
    solver::set_threads(0);
    eprintln!("Tangents x30000, solve: {one:?} on one thread, {all:?} on {many}");
    assert!(all.as_secs_f64() < 0.6 * one.as_secs_f64(), "30 000 tangents took {all:?} on {many} threads against {one:?} on one: the threads bring nothing");
}
