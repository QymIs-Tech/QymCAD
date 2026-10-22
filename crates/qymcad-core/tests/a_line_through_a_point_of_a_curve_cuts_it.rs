//! A LINE THROUGH A POINT OF A CURVE CUTS IT THERE, whichever of the two was drawn first: an arc of an ellipse and a
//! spline enter the regions of a sketch as the segments of their points, and a line passing through one of those points
//! met two segments at their ends.
//!
//! Reported behaviour: the upper half of an ellipse closed by its diameter, crossed by an upright line through its top,
//! stayed one region; the line, hanging into it, was left out of its edge.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{EllipseArcSpec, Project};

/// The way a case is drawn: the curve before the line, or the line before the curve.
#[derive(Clone, Copy, Debug)]
enum Order {
    CurveFirst,
    LineFirst,
}

/// The areas of the regions of a sketch drawn by `draw`, smallest first.
fn areas(draw: impl Fn(&mut Project, usize)) -> Vec<f64> {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("cut");
    draw(&mut p, si);
    p.merge_close_points(si, 1e-9);
    p.regen_sketch(si);
    let mut out: Vec<f64> = p.sketch_regions(si).iter().map(|c| c.area().abs()).collect();
    out.sort_by(f64::total_cmp);
    out
}

#[test]
fn a_line_through_a_point_of_a_curve_cuts_it() {
    let half = |p: &mut Project, si: usize| {
        let arc = EllipseArcSpec { centre: Point2::new(50.0, 10.0), radii: Point2::new(40.0, 8.0), rot: 0.0, angles: [0.0, std::f64::consts::PI], winding: Winding::Ccw };
        p.add_ellipse_arc_entity(si, arc, Purpose::Real);
        p.add_line_entity(si, 90.0, 10.0, 10.0, 10.0, Purpose::Real);
    };
    let up = |p: &mut Project, si: usize| {
        p.add_line_entity(si, 50.0, 5.0, 50.0, 25.0, Purpose::Real);
    };
    let mut failures = Vec::new();
    for order in [Order::CurveFirst, Order::LineFirst] {
        let found = areas(|p, si| match order {
            Order::CurveFirst => {
                half(p, si);
                up(p, si);
            }
            Order::LineFirst => {
                up(p, si);
                half(p, si);
            }
        });
        // a quarter of an ellipse of 40 x 8 is pi x 40 x 8 / 4
        let quarter = std::f64::consts::PI * 40.0 * 8.0 / 4.0;
        if found.len() != 2 || found.iter().any(|a| (a - quarter).abs() > 1.0) {
            failures.push(format!("{order:?}: the half of an ellipse cut through its top - regions of {found:?}, two of {quarter:.1} awaited"));
        }
    }
    // a line from the middle of the bottom of a rectangle 100 x 20 to the middle of its top: two halves of 1 000
    for order in [Order::CurveFirst, Order::LineFirst] {
        let found = areas(|p, si| {
            if matches!(order, Order::LineFirst) {
                p.add_line_entity(si, 50.0, 0.0, 50.0, 20.0, Purpose::Real);
            }
            p.add_rect_entity(si, 0.0, 0.0, 100.0, 20.0, Purpose::Real);
            if matches!(order, Order::CurveFirst) {
                p.add_line_entity(si, 50.0, 0.0, 50.0, 20.0, Purpose::Real);
            }
        });
        if found.len() != 2 || found.iter().any(|a| (a - 1000.0).abs() > 1e-6) {
            failures.push(format!("{order:?}: a rectangle cut from side to side - regions of {found:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
