//! A REBUILD AND AN OPEN SOLVE A SKETCH TO THE END, whatever the machine.
//!
//! Every rebuild and every open settles each sketch with a full solve (`settle_sketches`). That solve had a clock of
//! 10 s: on a sketch it did not finish in time, the parts not reached stayed where they stood and the parts cut off kept
//! their best points so far - and the rebuild built bodies from that and the file saved it, so what was saved depended
//! on how fast and how busy the machine was. The solve a rebuild and an open depend on now has no clock
//! (`Budget::WHOLE`); the clock stays for the frames of a drag.
//!
//! A PROBE OF TIME, run at the `time` level alone (`QYMCAD_TIER=time`, `tools/gate.py time`): the sketch has to be big
//! enough for the old clock to cut it, and how big that is depends on the machine - 100 000 circles held tangent to
//! lines left 24 580 parts unsolved after 12.7 s on one thread of a debug build on one machine, and were solved in time
//! on another. So the size is found on the machine the probe runs on: doubled until the clocked solve of an edit
//! (`solve_sketch`, `Budget::FULL`) leaves parts unsolved, then that sketch is settled as an open does.
mod sketch_kinds;

use qymcad_core::solver;
use sketch_kinds::{build, Kind};

/// AT THE LEVEL OF THE PROBES OF TIME: anywhere else the probe says so and passes over.
fn passed_over(name: &str) -> bool {
    let timed = std::env::var("QYMCAD_TIER").as_deref() == Ok("time");
    if !timed {
        eprintln!("PASSED OVER: {name} is a probe of time, run at the time level (QYMCAD_TIER=time)");
    }
    !timed
}

#[test]
fn a_settled_sketch_is_solved_to_the_end() {
    if passed_over("a_settled_sketch_is_solved_to_the_end") {
        return;
    }
    solver::set_threads(1);
    let mut n = 100_000;
    let cut = loop {
        let mut clocked = build(Kind::Tangents, n);
        clocked.solve_sketch(0);
        if clocked.sketches[0].left_unsolved > 0 {
            break n;
        }
        assert!(n < LARGEST, "the clock of 10 s cut no sketch up to {n} circles: the probe checks nothing on this machine");
        n *= 2;
    };
    let mut p = build(Kind::Tangents, cut);
    p.settle_loaded();
    solver::set_threads(0);
    assert_eq!(p.sketches[0].left_unsolved, 0, "a sketch of {cut} circles, cut by the clock of an edit, was left part unsolved as a document is opened");
}

/// The biggest sketch the probe builds looking for one the clock cuts: 1 600 000 circles, sixteen times the 100 000
/// the clock cut after 12.7 s.
const LARGEST: usize = 1_600_000;
