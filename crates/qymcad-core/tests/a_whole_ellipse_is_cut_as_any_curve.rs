//! A WHOLE ELLIPSE IS CUT AS ANY CURVE: a line, a spline, a circle or an arc crossing a whole ellipse cut it into closed
//! regions, as they cut a circle, and every piece of the ellipse on the edge of a region is a piece of that ellipse,
//! not the segments of its points. An ellipse no curve crosses is one region with one edge, the whole ellipse.
//!
//! Reported behaviour: neither a line nor a spline nor anything else crossing a whole ellipse cut it into closed
//! regions; the ellipse stayed one region of its own.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::{Contour, Point2, ProfEdge};
use qymcad_core::model::{EllipseArcSpec, Id, Project};

/// The ellipse of every case: about (50, 10), semi-axes 40 and 8.
const AREA: f64 = std::f64::consts::PI * 40.0 * 8.0;

/// The id of the ellipse of sketch `si`: the door that lays it answers its centre.
fn the_ellipse(p: &Project, si: usize) -> Id {
    p.sketches[si].entities.iter().find(|e| e.kind.as_ellipse().is_some()).map(|e| e.id).expect("the ellipse")
}

/// The closed regions of the sketch `si`, smallest first.
fn regions(p: &Project, si: usize) -> Vec<Contour> {
    let mut out: Vec<Contour> = p.sketches[si].contour_ids.iter().filter_map(|c| p.contour_index(*c)).map(|i| p.contours[i].clone()).filter(|c| c.closed).collect();
    out.sort_by(|a, b| a.area().abs().total_cmp(&b.area().abs()));
    out
}

/// A CASE: what crosses the ellipse, and the regions awaited inside the ellipse, by their share of its area.
struct Case {
    name: &'static str,
    draw: fn(&mut Project, usize),
    shares: &'static [f64],
}

fn line(p: &mut Project, si: usize) {
    p.add_line_entity(si, 0.0, 10.0, 100.0, 10.0, Purpose::Real);
}
fn upright(p: &mut Project, si: usize) {
    p.add_line_entity(si, 50.0, 0.0, 50.0, 20.0, Purpose::Real);
}
fn spline(p: &mut Project, si: usize) {
    let nodes = [[50.0, -5.0], [52.0, 10.0], [50.0, 25.0]].map(|q| Point2::new(q[0], q[1])).to_vec();
    p.add_spline(si, nodes, Ends::Open, Purpose::Real);
}
fn circle(p: &mut Project, si: usize) {
    p.add_circle_entity(si, 90.0, 10.0, 6.0, Purpose::Real);
}
fn arc(p: &mut Project, si: usize) {
    let half = EllipseArcSpec { centre: Point2::new(50.0, 2.0), radii: Point2::new(20.0, 20.0), rot: 0.0, angles: [0.0, std::f64::consts::PI], winding: Winding::Ccw };
    p.add_ellipse_arc_entity(si, half, Purpose::Real);
}

#[test]
fn a_whole_ellipse_is_cut_as_any_curve() {
    let cases = [
        Case { name: "a line through its centre", draw: line, shares: &[0.5, 0.5] },
        Case { name: "an upright line through its centre", draw: upright, shares: &[0.5, 0.5] },
        Case { name: "a spline across it", draw: spline, shares: &[0.48, 0.52] },
        Case { name: "a circle over its end", draw: circle, shares: &[] },
        Case { name: "an arc across it", draw: arc, shares: &[] },
    ];
    let mut failures = Vec::new();
    for Case { name, draw, shares } in cases {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch(name);
        p.add_ellipse_entity(si, Point2::new(50.0, 10.0), 40.0, 8.0, 0.0, Purpose::Real);
        let ellipse = the_ellipse(&p, si);
        draw(&mut p, si);
        p.merge_close_points(si, 1e-9);
        p.regen_sketch(si);
        let all = regions(&p, si);
        let on_ellipse: Vec<&Contour> = all.iter().filter(|c| c.edge_src.contains(&ellipse)).collect();
        if on_ellipse.len() < 2 {
            failures.push(format!("{name}: the ellipse is the edge of {} region(s), not cut", on_ellipse.len()));
            continue;
        }
        if !shares.is_empty() {
            let got: Vec<f64> = on_ellipse.iter().map(|c| c.area().abs() / AREA).collect();
            if got.len() != shares.len() || got.iter().zip(shares.iter()).any(|(g, s)| (g - s).abs() > 0.02) {
                failures.push(format!("{name}: the regions inside the ellipse hold {got:?} of it, not {shares:?}"));
            }
        }
        let segments = on_ellipse.iter().flat_map(|c| c.edge_src.iter().zip(&c.edges)).filter(|(s, e)| **s == ellipse && !matches!(e, ProfEdge::Oval { .. })).count();
        if segments > 0 {
            failures.push(format!("{name}: {segments} edges of the ellipse are no piece of it"));
        }
    }
    // an ellipse no curve crosses, inside a rectangle: two regions, the ellipse one whole edge
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("alone");
    p.add_ellipse_entity(si, Point2::new(50.0, 10.0), 40.0, 8.0, 0.0, Purpose::Real);
    let ellipse = the_ellipse(&p, si);
    p.add_rect_entity(si, 0.0, -10.0, 100.0, 30.0, Purpose::Real);
    p.regen_sketch(si);
    let all = regions(&p, si);
    let whole = all.iter().find(|c| c.edge_src.contains(&ellipse));
    match whole {
        Some(c) if all.len() == 2 && c.edges.len() == 1 && c.edges[0].whole() && (c.area().abs() - AREA).abs() < 0.01 * AREA => {}
        _ => failures.push(format!("an ellipse alone in a rectangle: {} regions, the ellipse of {:?} edges", all.len(), whole.map(|c| c.edges.len()))),
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
