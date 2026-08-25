//! Tests of the operations and the toolpath they produce.

use qymcad_core::geom::{Contour, Point2};
use qymcad_core::ir::{DrillKind, Move};
use qymcad_core::ops::{ContourOp, DrillOp, Heights, Operation, Passes, Ramp, Side, Tabs};
use qymcad_core::ops::params::Feeds;
use qymcad_core::tool::{Tool, ToolType};

fn endmill(d: f64) -> Tool {
    Tool {
        number: 1,
        name: format!("EM{d}"),
        kind: ToolType::FlatEnd,
        diameter: d,
        corner_radius: 0.0,
        flutes: 2,
        v_angle: None,
    }
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
fn contour_inside_structure_and_passes() {
    let op = ContourOp {
        name: "profile".into(),
        tool: endmill(4.0), // a radius of 2
        contours: vec![square(10.0)],
        side: Side::Inside,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -2.0 },
        passes: Passes { stepdown: 1.0, stepover: 2.0, stock_to_leave: 0.0 },
        feeds: Feeds::default(),
        tabs: Tabs::default(),
        ramp: Ramp { enabled: false, angle_deg: 3.0 }, // a vertical plunge, so they can be counted
    };
    let tp = op.generate();

    // the preamble
    assert!(matches!(tp.moves[0], Move::Comment { .. }));
    assert!(matches!(tp.moves[1], Move::ToolChange { .. }));
    assert!(matches!(tp.moves[2], Move::SpindleOn { .. }));

    // one path over two levels, z = −1 and −2, giving two plunges
    let plunges = tp.moves.iter().filter(|m| matches!(m, Move::Plunge { .. })).count();
    assert_eq!(plunges, 2, "two plunges were expected, one per Z level");

    // there are cutting moves
    assert!(tp.moves.iter().any(|m| matches!(m, Move::Linear { .. })));
}

#[test]
fn adaptive_fills_region_with_loops_inside_bounds() {
    use qymcad_core::model::{OpKind, OperationDef, Project};
    let mut p = Project::default();
    p.set_contours(vec![square(40.0)]); // a 40×40 area
    p.tools = vec![endmill(6.0)];
    let mut op = OperationDef::new("Adapt", 1, OpKind::Adaptive2D);
    op.heights = Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -2.0 };
    op.passes = Passes { stepdown: 1.0, stepover: 4.0, stock_to_leave: 0.0 };
    p.operations.push(op);

    let tp = &p.build_program("a").toolpaths[0];
    let cuts: Vec<_> = tp.moves.iter().filter_map(|m| match m {
        Move::Linear { to, .. } => Some((to.x, to.y)),
        _ => None,
    }).collect();
    assert!(cuts.len() > 50, "a trochoidal path gives many moves, {}", cuts.len());
    // every cutting point lies inside the area, with a margin for the radius
    for (x, y) in &cuts {
        assert!(*x >= -1.0 && *x <= 41.0 && *y >= -1.0 && *y <= 41.0, "a point outside the area: {x},{y}");
    }
    // there is oscillation along Y, the loops, rather than a monotonic line
    let ys: Vec<f64> = cuts.iter().map(|(_, y)| *y).collect();
    let mut reversals = 0;
    for w in ys.windows(3) {
        if (w[1] - w[0]).signum() != (w[2] - w[1]).signum() {
            reversals += 1;
        }
    }
    assert!(reversals > 10, "loops were expected, as reversals along Y, got {reversals}");
}

#[test]
fn drill_emits_single_cycle_with_all_points() {
    let op = DrillOp {
        name: "drill".into(),
        tool: endmill(3.0),
        points: vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(5.0, 5.0)],
        kind: DrillKind::Peck,
        heights: Heights { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -6.0 },
        peck: Some(2.0),
        dwell: None,
        feeds: Feeds::default(),
    };
    let tp = op.generate();

    let cycle = tp.moves.iter().find_map(|m| match m {
        Move::DrillCycle { points, kind, .. } => Some((points.len(), *kind)),
        _ => None,
    });
    assert_eq!(cycle, Some((3, DrillKind::Peck)));
}
