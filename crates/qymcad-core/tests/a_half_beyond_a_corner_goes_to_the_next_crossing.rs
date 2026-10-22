//! A HALF BEYOND A CORNER GOES AS FAR AS THE NEXT CROSSING, and a closed curve crossed once takes a corner and keeps its
//! rest. A spline through a circle, crossing it twice: the corner at the first crossing rounded R1 outside the circle -
//! the half of the spline beyond it goes as far as the second crossing, and its tail past that stays. A spline ending
//! inside a circle, crossing it once: the crossing is cut, the corner outside the circle rounded R1 - the piece of the
//! spline inside goes, and the circle stays as an arc round the long way.
//!
//! Reported behaviour: a fillet at one crossing of a spline and a circle took the half of the spline beyond it with the
//! second crossing on it; at a spline ending inside a circle the fillet and the chamfer did nothing.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{CornerAt, CornerCut, FilletSize, Project};

/// The circle about (20, 20) of radius 10 and a spline through `nodes`, the crossing of the two nearest `near` cut, the
/// corner of the quarter radially outside the circle rounded R1 and the halves beyond it let go: the drawing, or what
/// went wrong.
fn rounded(nodes: Vec<Point2>, near: Point2) -> Result<Project, String> {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_circle_entity(si, 20.0, 20.0, 10.0, Purpose::Real);
    p.add_spline(si, nodes, Ends::Open, Purpose::Real).ok_or("no spline")?;
    let crossing = p.crossing_near(si, near, 3.0).ok_or("no crossing near")?;
    let at = p.cut_at_crossing(si, crossing).ok_or("the crossing was not cut")?;
    let x = crossing.at;
    let out = Point2::new(x.x - 20.0, x.y - 20.0);
    let toward = Point2::new(x.x + 0.5 * out.x / 10.0, x.y + 0.5 * out.y / 10.0);
    let pair = p.vertex_pair(si, at, Some((toward.x, toward.y))).ok_or("no corner outside the circle")?;
    let corner = CornerAt { point: at, pair };
    if p.cut_corner_set(si, &[corner], CornerCut::Fillet(FilletSize::radius(1.0)), None) != 1 {
        return Err("the corner was not rounded".into());
    }
    p.drop_halves_beyond_corner(si, corner);
    Ok(p)
}

/// The curves of the drawing.
fn curves(p: &Project) -> Vec<Curve> {
    let s = &p.sketches[0];
    s.entities.iter().filter_map(|e| Curve::of(s, e)).collect()
}

/// Whether a curve of the drawing ends at `q`.
fn ends_at(p: &Project, q: Point2) -> bool {
    curves(p).iter().any(|c| !c.closed() && c.ends().iter().any(|e| e.dist(q) < 1e-6))
}

#[test]
fn the_half_of_a_spline_beyond_a_corner_goes_to_the_second_crossing() {
    // in from (4, 34), down through (20, 24) inside the circle, out again and up to (36, 34)
    let nodes = vec![Point2::new(4.0, 34.0), Point2::new(20.0, 24.0), Point2::new(36.0, 34.0)];
    let p = rounded(nodes, Point2::new(12.0, 28.0)).unwrap_or_else(|why| panic!("{why}"));
    let tail = ends_at(&p, Point2::new(36.0, 34.0));
    let start = ends_at(&p, Point2::new(4.0, 34.0));
    assert!(tail && start, "after the fillet at the first crossing: the tail of the spline past the second crossing stands {tail}, its start {start}; the drawing holds {:?}", curves(&p));
}

#[test]
fn a_circle_a_spline_ends_inside_takes_a_corner_and_keeps_its_rest() {
    // in from (4, 34), ending at (18, 22) inside the circle
    let nodes = vec![Point2::new(4.0, 34.0), Point2::new(12.0, 27.0), Point2::new(18.0, 22.0)];
    let p = rounded(nodes, Point2::new(12.0, 27.0)).unwrap_or_else(|why| panic!("{why}"));
    let inside_gone = !ends_at(&p, Point2::new(18.0, 22.0));
    let rest = curves(&p).iter().filter(|c| matches!(c, Curve::Round { r, .. } if (r - 10.0).abs() < 1e-3)).map(|c| c.length()).sum::<f64>();
    let whole = std::f64::consts::TAU * 10.0;
    assert!(inside_gone && rest > 0.8 * whole, "the piece of the spline inside the circle gone {inside_gone}, the circle left {rest:.2} long of {whole:.2}; the drawing holds {:?}", curves(&p));
}
