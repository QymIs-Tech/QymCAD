//! THE LIMIT OF A CORNER OF ANY CURVES: the field of the fillet and the chamfer refuses, in words, a value past the most
//! the corner takes - for a corner of a line and an arc or a spline as for two lines. At the limit the cut is laid, past
//! it by a hundredth it is refused; for two lines the limit is the one of their angle and the shorter line.
//!
//! Reported behaviour: on a corner with a curve the field took any value, and a value too big refused only on Enter.
use qymcad_core::feature::{ChamferMode, Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferLegs, CornerAt, CornerCut, CornerTool, FilletSize, Id, Project};

/// A CORNER DRAWN: the drawing, the corner and its two curves.
struct Drawn {
    p: Project,
    corner: CornerAt,
}

/// A level line from (20, 0) to the corner at (0, 0), and the other side laid by `other` from the corner.
fn drawn(other: fn(&mut Project, usize) -> Id) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let line = p.add_line_entity(si, 20.0, 0.0, 0.0, 0.0, Purpose::Real);
    let side = other(&mut p, si);
    p.merge_close_points(si, 1e-9);
    let point = p.sketches[si].points.iter().find(|q| q.x.hypot(q.y) < 1e-9).map(|q| q.id).expect("the corner");
    Drawn { p, corner: CornerAt::of(point, [line, side]) }
}

fn upright(p: &mut Project, si: usize) -> Id {
    p.add_line_entity(si, 0.0, 0.0, 0.0, 12.0, Purpose::Real)
}
fn arc(p: &mut Project, si: usize) -> Id {
    p.add_arc_entity(si, Point2::new(10.0, 0.0), Point2::new(0.0, 0.0), Point2::new(10.0, 10.0), Winding::Cw, Purpose::Real);
    p.sketches[si].entities.iter().find(|e| e.kind.as_arc().is_some()).map(|e| e.id).expect("the arc")
}
fn spline(p: &mut Project, si: usize) -> Id {
    p.add_spline(si, vec![Point2::new(0.0, 0.0), Point2::new(3.0, 8.0), Point2::new(-1.0, 15.0)], Ends::Open, Purpose::Real).expect("the spline")
}

/// A SIDE OF A CORNER: what it is and how it is laid.
struct Side {
    name: &'static str,
    lay: fn(&mut Project, usize) -> Id,
}

#[test]
fn every_corner_has_a_limit_its_cut_keeps_to() {
    let sides = [Side { name: "a line", lay: upright }, Side { name: "an arc", lay: arc }, Side { name: "a spline", lay: spline }];
    let tools = [CornerTool::Fillet, CornerTool::Chamfer(ChamferMode::Symmetric), CornerTool::Chamfer(ChamferMode::TwoDist)];
    let cut = |tool: CornerTool, v: f64| match tool {
        CornerTool::Fillet => CornerCut::Fillet(FilletSize::radius(v)),
        CornerTool::Chamfer(ChamferMode::Symmetric) => CornerCut::Chamfer(ChamferLegs::equal(v)),
        CornerTool::Chamfer(_) => CornerCut::Chamfer(ChamferLegs { mode: ChamferMode::TwoDist, first: v, second: v }),
    };
    let mut failures = Vec::new();
    for Side { name, lay } in sides {
        for tool in tools {
            let Drawn { p, corner } = drawn(lay);
            let Some(limit) = p.corner_limit_of_pair(0, corner.point, corner.pair, tool) else {
                failures.push(format!("{name}, {tool:?}: no limit"));
                continue;
            };
            let shown = |v: f64| p.corner_blend(0, corner.point, corner.pair, cut(tool, v)).is_some();
            if !shown(limit * 0.999) || shown(limit * 1.01) {
                failures.push(format!("{name}, {tool:?}: the limit {limit:.4} - just under it shown {}, a hundredth past it shown {}", shown(limit * 0.999), shown(limit * 1.01)));
            }
        }
    }
    // two lines at a right angle, the shorter 12: a fillet up to 12 tan 45 deg, a symmetric cut up to 12 x 2 sin 45 deg
    let Drawn { p, corner } = drawn(upright);
    let fillet = p.corner_limit_of_pair(0, corner.point, corner.pair, CornerTool::Fillet).unwrap_or(0.0);
    let cut_limit = p.corner_limit_of_pair(0, corner.point, corner.pair, CornerTool::Chamfer(ChamferMode::Symmetric)).unwrap_or(0.0);
    if (fillet - 12.0).abs() > 1e-6 || (cut_limit - 12.0 * std::f64::consts::SQRT_2).abs() > 1e-6 {
        failures.push(format!("two lines: a fillet up to {fillet:.6} (12), a symmetric cut up to {cut_limit:.6} (16.970563)"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
