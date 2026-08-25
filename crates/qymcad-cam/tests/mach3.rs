//! Tests of the Mach3 post-processor on a realistic program: a contour plus drilling.

use qymcad_core::geom::{Contour, Point2};
use qymcad_core::ir::{DrillKind, Program, Units};
use qymcad_core::ops::params::Feeds;
use qymcad_core::ops::{ContourOp, DrillOp, Heights, Operation, Passes, Ramp, Side, Tabs};
use qymcad_core::tool::{Tool, ToolType};
use qymcad_core::model::PostKind;
use qymcad_cam::{mach3, post_for, PostOptions};

fn endmill(n: u32, d: f64) -> Tool {
    Tool { number: n, name: format!("EM{d}"), kind: ToolType::FlatEnd, diameter: d, corner_radius: 0.0, flutes: 2, v_angle: None }
}

fn square(side: f64) -> Contour {
    Contour::closed(vec![
        Point2::new(0.0, 0.0),
        Point2::new(side, 0.0),
        Point2::new(side, side),
        Point2::new(0.0, side),
    ])
}

#[test]
fn posts_contour_and_drill() {
    let contour = ContourOp {
        name: "outer profile".into(),
        tool: endmill(1, 6.0),
        contours: vec![square(40.0)],
        side: Side::Outside,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -3.0 },
        passes: Passes { stepdown: 1.5, stepover: 3.0, stock_to_leave: 0.0 },
        feeds: Feeds::default(),
        tabs: Tabs::default(),
        ramp: Ramp::default(),
    }
    .generate();

    let drill = DrillOp {
        name: "holes".into(),
        tool: endmill(2, 5.0),
        points: vec![Point2::new(10.0, 10.0), Point2::new(30.0, 10.0)],
        kind: DrillKind::Peck,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -8.0 },
        peck: Some(2.0),
        dwell: None,
        feeds: Feeds::default(),
    }
    .generate();

    let program = Program { name: "demo".into(), units: Units::Mm, toolpaths: vec![contour, drill] };
    let g = mach3::post(&program, &PostOptions::default());

    // the structure of a Mach3 program
    assert!(g.contains("(demo)"), "the program header");
    assert!(g.contains("(Controller: Mach3)"), "the controller name in Latin script, which is all a controller can display");
    assert!(g.contains("G17 G54 G40 G49 G80 G90"), "the preamble");
    assert!(g.contains("G21"), "metric units");
    assert!(g.contains("T1 M6") && g.contains("G43 H1"), "a tool change plus the tool length offset");
    assert!(g.contains("M3"), "the spindle");
    assert!(g.contains("G0"), "rapid moves");
    assert!(g.contains("G1"), "cutting moves");
    // a real drilling cycle rather than an expanded one
    assert!(g.contains("G98"), "the return mode of the cycle");
    assert!(g.contains("G83") && g.contains("Q2"), "a peck cycle with Q");
    assert!(g.contains("G80"), "cancelling the cycle");
    // the postamble
    assert!(g.trim_end().ends_with("M2"), "the program ends with M2");

    // Axis-modal output: no line should repeat an unchanged coordinate. A rough check that F is not printed on
    // every line.
    let f_lines = g.lines().filter(|l| l.contains('F')).count();
    let g1_lines = g.lines().filter(|l| l.starts_with("G1")).count();
    assert!(f_lines < g1_lines.max(1) + 5, "F must not be repeated on every line");
}

#[test]
fn translate_drill_cycles_expands_to_moves() {
    let drill = DrillOp {
        name: "holes".into(),
        tool: endmill(2, 5.0),
        points: vec![Point2::new(10.0, 10.0)],
        kind: DrillKind::Peck,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -4.0 },
        peck: Some(2.0),
        dwell: None,
        feeds: Feeds::default(),
    }
    .generate();
    let program = Program { name: "d".into(), units: Units::Mm, toolpaths: vec![drill] };

    let mut opts = PostOptions::default();
    opts.translate_drill_cycles = true;
    let g = mach3::post(&program, &opts);

    assert!(!g.contains("G83"), "the cycles have to be expanded");
    assert!(!g.contains("G81"), "the cycles have to be expanded");
    assert!(g.contains("G1"), "there have to be G1 plunges");
}

#[test]
fn grbl_dialect_drops_toolchange_tlo_and_cycles() {
    let drill = DrillOp {
        name: "holes".into(),
        tool: endmill(1, 5.0),
        points: vec![Point2::new(10.0, 10.0)],
        kind: DrillKind::Peck,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -4.0 },
        peck: Some(2.0),
        dwell: None,
        feeds: Feeds::default(),
    }
    .generate();
    let program = Program { name: "g".into(), units: Units::Mm, toolpaths: vec![drill] };

    let grbl = post_for(&program, PostKind::Grbl, &PostOptions::default());
    assert!(!grbl.contains("M6"), "GRBL: no M6");
    assert!(!grbl.contains("G43"), "GRBL: no tool length offset");
    assert!(!grbl.contains("G83"), "GRBL: the cycles are expanded");
    assert!(grbl.contains("M3"), "GRBL: the spindle is there");

    let lcnc = post_for(&program, PostKind::LinuxCnc, &PostOptions::default());
    assert!(lcnc.contains("G83"), "LinuxCNC: the cycles are supported");
}

#[test]
fn post_emits_wcs_per_setup() {
    let tp = ContourOp {
        name: "p".into(),
        tool: endmill(1, 6.0),
        contours: vec![square(40.0)],
        side: Side::Outside,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -2.0 },
        passes: Passes { stepdown: 1.0, stepover: 3.0, stock_to_leave: 0.0 },
        feeds: Feeds::default(),
        tabs: Tabs::default(),
        ramp: Ramp::default(),
    }
    .generate();
    let mut a = tp.clone();
    a.meta.wcs = 54;
    let mut b = tp.clone();
    b.meta.wcs = 55;
    let program = Program { name: "wcs".into(), units: Units::Mm, toolpaths: vec![a, b] };
    let g = mach3::post(&program, &PostOptions::default());
    assert!(g.contains("G55"), "the second setup gives G55:\n{g}");
}
