//! THE KERNEL KEEPS ITS TIME: the examples rebuilt whole, three roundings on three parts, and what the threads of the
//! kernel buy. A probe of time, run at the level of the probes of time alone (`QYMCAD_TIER=time`, `tools/gate.py time`),
//! one at a time; anywhere else it says so and passes over. Its time is the work of the program on threads of its own
//! (`time_budget::cost_of`, `Doer::Process`), the best of three, so a slower spell of the machine is not taken for the
//! program. Live OCCT, headless.
use qymcad_core::feature::{FeatureKind, Purpose};
use qymcad_core::model::{Id, Project};
use qymcad_core::time_budget::{cost_of, over, tell, within, Cost, Doer, MEASURED};
use std::time::Duration;

/// AT THE LEVEL OF THE PROBES OF TIME: anywhere else the probe says so and passes over.
fn passed_over(name: &str) -> bool {
    let timed = qymcad_core::time_budget::at_the_level_of_time();
    if !timed {
        eprintln!("PASSED OVER: {name} is a probe of time, run at the time level (QYMCAD_TIER=time)");
    }
    !timed
}

/// The cost of the quickest of three runs of `work`.
fn best_of_three(mut work: impl FnMut()) -> Cost {
    (0..3).map(|_| cost_of(Doer::Process, &mut work).cost).min_by_key(|c| c.work).unwrap_or_default()
}

/// The example `name` as it ships.
fn example(name: &str) -> Project {
    let path = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    qymcad_io::load_project(&path).unwrap_or_else(|e| panic!("the example {name} opens: {e:?}"))
}

/// The cost of rebuilding `p` whole, the best of three, on the threads the kernel is given.
fn rebuilt_whole(p: &Project) -> Cost {
    best_of_three(|| {
        let mut q = p.clone();
        let shapes = qymcad_testkit::restore_import_shapes(&q);
        let (report, _) = qymcad_testkit::regenerate_with_shapes(&mut q, shapes);
        assert!(report.errors.iter().all(|(n, _)| p.regen_errors.contains_key(n)), "the example rebuilt with new red nodes: {:?}", report.errors);
    })
}

/// AN EXAMPLE REBUILT WHOLE: its name and its budget of work on every core but one - the rebuild measured on the main
/// line before the kernel was made safe on threads, and a twentieth on top. Measured on a quiet machine (a load of 3-9 on
/// 20 threads), the best of three: the case 9.14 s, the filter 9.21 s; on the branch, read as the work of the program on
/// the counters of the system, 8.04 s and 7.75 s (a load of 6).
struct Whole {
    name: &'static str,
    budget: Duration,
}

const WHOLE: [Whole; 2] = [Whole { name: "QymBoxPc-Case.qcad", budget: Duration::from_millis(9_600) }, Whole { name: "Filter-v2.qcad", budget: Duration::from_millis(9_670) }];

#[test]
fn the_examples_rebuild_no_slower_than_before() {
    if passed_over("the_examples_rebuild_no_slower_than_before") {
        return;
    }
    qymcad_kernel::set_parallel(true, 0);
    let mut slow = Vec::new();
    for w in WHOLE {
        let cost = rebuilt_whole(&example(w.name));
        tell(&format!("{} rebuilt whole", w.name), cost);
        if !within(cost.work, w.budget) {
            slow.push(over(&format!("{} rebuilt whole", w.name), cost, w.budget));
        }
    }
    assert!(slow.is_empty(), "{}", slow.join("\n"));
}

/// THREE PARTS ROUNDED: the project and the three roundings in it.
struct Rounded {
    project: Project,
    roundings: Vec<Id>,
}

/// THREE PARTS, each a block 40 x 30 x 10 with its four upright edges rounded R2. Built.
fn three_rounded_parts() -> Rounded {
    let mut p = Project::default();
    p.new_document();
    let mut roundings = Vec::new();
    for k in 0..3 {
        let part = p.add_part(format!("block {k}"));
        p.set_active_component(Some(part));
        let s = p.new_sketch("block");
        let sid = p.sketches[s].id;
        p.add_sketch_node(sid, "block");
        p.add_rect_entity(s, 0.0, 0.0, 40.0, 30.0, Purpose::Real);
        p.regen_sketch(s);
        let body = p.add_extrude(sid, 10.0);
        p.finish_base_body(body, 1);
        let _ = qymcad_testkit::regenerate(&mut p);
        let upright: Vec<u32> = p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.dir[2].abs() > 0.99).map(|e| e.id).collect();
        assert_eq!(upright.len(), 4, "setup: the block has four upright edges");
        roundings.push(p.add_fillet(body, 2.0, upright));
    }
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "setup: the three rounded blocks do not build: {:?}", report.errors);
    Rounded { project: p, roundings }
}

/// THE BUDGET OF THREE ROUNDINGS ON THREE PARTS, rebuilt together on every core but one. Measured, the best of three:
/// 75 ms of work (a load of 6 on 20 threads); the budget is about twice that.
const THREE_ROUNDINGS: Duration = Duration::from_millis(150);

#[test]
fn three_roundings_on_three_parts_keep_their_time() {
    if passed_over("three_roundings_on_three_parts_keep_their_time") {
        return;
    }
    qymcad_kernel::set_parallel(true, 0);
    let Rounded { project: p, roundings } = three_rounded_parts();
    let cost = best_of_three(|| {
        let mut q = p.clone();
        for n in q.timeline.iter_mut().filter(|n| matches!(n.kind, FeatureKind::Fillet { .. })) {
            n.dirty = true;
        }
        let (report, _) = qymcad_testkit::regenerate(&mut q);
        assert!(roundings.iter().all(|r| !report.errors.iter().any(|(n, _)| n == r)), "a rounding refused: {:?}", report.errors);
    });
    tell("three roundings on three parts", cost);
    assert!(within(cost.work, THREE_ROUNDINGS), "{}", over("three roundings on three parts", cost, THREE_ROUNDINGS));
}

/// HOW MUCH QUICKER AN EXAMPLE REBUILDS ON EVERY CORE BUT ONE THAN ON ONE, at the least: its name and the ratio of the
/// work on one thread to the work on all of them.
struct OnThreads {
    name: &'static str,
    least: f64,
}

/// Measured as the work of the program on the counters of the system, the best of three. The case: 8.10 s on one thread,
/// 5.99 s on 19 (1.35 times); on the main line before the kernel was made safe on threads 1.26 times, and the least is
/// that less a twentieth. The filter: 11.33 s on one thread, 9.13 s on 19 (1.24 times) on the wall; 12.63 s and 8.15 s
/// (1.55 times) as the work.
const ON_THREADS: [OnThreads; 2] = [OnThreads { name: "QymBoxPc-Case.qcad", least: 1.20 }, OnThreads { name: "Filter-v2.qcad", least: 1.15 }];

#[test]
fn the_threads_buy_the_examples_their_time() {
    if passed_over("the_threads_buy_the_examples_their_time") {
        return;
    }
    let threads = qymcad_kernel::workers().max(1);
    let mut short = Vec::new();
    for t in ON_THREADS {
        let p = example(t.name);
        qymcad_kernel::set_parallel(false, 1);
        let one = rebuilt_whole(&p).work;
        qymcad_kernel::set_parallel(true, 0);
        let many = rebuilt_whole(&p).work;
        let ratio = one.as_secs_f64() / many.as_secs_f64().max(1e-9);
        eprintln!("{MEASURED}{} rebuilt whole: {one:.3?} of work on one thread, {many:.3?} on {threads} ({ratio:.2} times)", t.name);
        if ratio < t.least {
            short.push(format!("{} rebuilds {ratio:.2} times quicker on {threads} threads than on one, not {}", t.name, t.least));
        }
    }
    assert!(short.is_empty(), "{}", short.join("\n"));
}
