//! EVERY TOOL OF A SKETCH KEEPS ITS TIME ON A BIG SKETCH: each tool, through the door of the project the window calls,
//! on sketches of thousands of shapes - separate rectangles, a grid of two patterns, a pattern tied into one part, a
//! mixture - takes no more than a few rebuilds of that sketch. A tool that rebuilds the whole sketch for every element
//! it lays takes as many rebuilds as elements, and is caught however fast the machine.
//!
//! Reported behaviour (#95): a line and one across it, each patterned 200 times, and every operation waited 30 s - the
//! pattern rebuilt the sketch for each of its copies. The measure of the solve and the checks had not looked at the
//! tools that change a sketch.
mod sketch_kinds;
mod sketch_tools;

use qymcad_core::model::{PatternKind, Project};
use sketch_kinds::{build, Kind};
use sketch_tools::{lay_targets, loops, shows, tools};
use std::time::{Duration, Instant};

/// A big sketch of one kind, the sketch being the first of the project.
struct Big {
    name: &'static str,
    project: Project,
}

/// The grid of the report: a horizontal line and a vertical one, each patterned `n` times 20 mm apart.
fn grid(n: u32) -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let len = 20.0 * n as f64;
    let h = p.add_line_entity(si, 0.0, -10.0, len, -10.0, qymcad_core::feature::Purpose::Real);
    let v = p.add_line_entity(si, -10.0, 0.0, -10.0, len, qymcad_core::feature::Purpose::Real);
    p.add_pattern(si, &[h], PatternKind::Linear { dx: 0.0, dy: 20.0, count: n, dx2: 0.0, dy2: 0.0, count2: 0 });
    p.add_pattern(si, &[v], PatternKind::Linear { dx: 20.0, dy: 0.0, count: n, dx2: 0.0, dy2: 0.0, count2: 0 });
    p
}

fn bigs() -> Vec<Big> {
    let solved = |mut p: Project| {
        p.solve_sketch(0);
        p
    };
    vec![
        Big { name: "2 000 separate rectangles", project: solved(build(Kind::Rectangles, 2_000)) },
        Big { name: "a grid of two patterns, 60 by 60", project: grid(60) },
        Big { name: "a pattern of 200 rectangles in one part", project: solved(build(Kind::Array, 200)) },
        Big { name: "2 000 mixed shapes", project: solved(build(Kind::Mixed, 2_000)) },
    ]
}

#[test]
fn every_tool_takes_a_few_rebuilds_of_a_big_sketch() {
    let mut failures = Vec::new();
    for big in bigs() {
        // the targets of the tools laid beside the sketch
        let mut base = big.project.clone();
        let targets = lay_targets(&mut base);
        // one rebuild of this sketch, the best of three: the unit the tools are told against
        let rebuild = (0..3)
            .map(|_| {
                let mut p = base.clone();
                let started = Instant::now();
                p.regen_sketch_whole(0);
                started.elapsed()
            })
            .min()
            .unwrap_or_default();
        let budget = rebuild * 5 + Duration::from_millis(200);
        for tool in tools() {
            let mut p = base.clone();
            let started = Instant::now();
            (tool.work)(&mut p, &targets);
            let t = started.elapsed();
            eprintln!("{:<45} {:<48} {t:>12.3?}  (a rebuild {rebuild:.3?})", big.name, tool.name);
            if !qymcad_core::time_budget::within(t, budget) {
                failures.push(format!(
                    "{} - {}: {t:.3?}, {:.0} rebuilds of the sketch ({rebuild:.3?} each), budget {budget:.3?}",
                    big.name,
                    tool.name,
                    t.as_secs_f64() / rebuild.as_secs_f64().max(1e-9)
                ));
            }
            // what it left behind, and the loops of a rebuild from all the curves
            if let Some(wrong) = shows(tool.effect, &base, &p) {
                failures.push(format!("{} - {}: {wrong}", big.name, tool.name));
            }
            let mut whole = p.clone();
            whole.regen_sketch_whole(0);
            if loops(&p) != loops(&whole) {
                failures.push(format!("{} - {}: the loops are not those of a rebuild", big.name, tool.name));
            }
        }
    }
    assert!(failures.is_empty(), "tools slow or wrong on a big sketch:\n{}", failures.join("\n"));
}
