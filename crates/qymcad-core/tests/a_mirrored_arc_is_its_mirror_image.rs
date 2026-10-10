//! A MIRRORED ARC IS ITS MIRROR IMAGE: the copy runs between the mirrored ends through the mirrored middle, not round the
//! other side of the same circle. A reflection turns the way round of an arc; a turn about a point does not.
//!
//! Reported behaviour: "mirroring an arc draws the wrong part of the arc on the copy".
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{EntityKind, Project};

/// The middle of the arc `id` of sketch `si`, along the way it runs.
fn middle(p: &Project, si: usize, id: u64) -> (f64, f64) {
    let s = &p.sketches[si];
    let at = |q: u64| s.points.iter().find(|x| x.id == q).map(|x| (x.x, x.y)).expect("the point");
    let Some(EntityKind::Arc { center, a, b, ccw }) = s.entities.iter().find(|e| e.id == id).map(|e| e.kind) else { panic!("no arc {id}") };
    let (c, pa, pb) = (at(center), at(a), at(b));
    let (a0, a1) = ((pa.1 - c.1).atan2(pa.0 - c.0), (pb.1 - c.1).atan2(pb.0 - c.0));
    let tau = std::f64::consts::TAU;
    let sweep = if ccw { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
    let (r, m) = ((pa.0 - c.0).hypot(pa.1 - c.1), a0 + sweep / 2.0);
    (c.0 + r * m.cos(), c.1 + r * m.sin())
}

/// A sketch with a quarter arc about the origin, counter-clockwise from (10, 0) to (0, 10): its middle at 45 deg.
fn quarter() -> (Project, usize, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(0.0, 10.0), Winding::Ccw, Purpose::Real);
    let id = p.sketches[si].entities[0].id;
    (p, si, id)
}

#[test]
fn an_arc_mirrored_about_a_line_or_a_point_runs_through_the_image_of_its_middle() {
    let d = 10.0 * std::f64::consts::FRAC_1_SQRT_2; // the middle of the quarter: (d, d)
    let mut failures = Vec::new();
    // about the upright x = 20: the middle (d, d) goes to (40 - d, d)
    let (mut p, si, src) = quarter();
    p.mirror_entities(si, &[src], 20.0, 0.0, 20.0, 1.0);
    let copy = p.sketches[si].entities.last().map(|e| e.id).expect("the copy");
    let m = middle(&p, si, copy);
    if (m.0 - (40.0 - d)).abs() > 1e-6 || (m.1 - d).abs() > 1e-6 {
        failures.push(format!("mirrored about x = 20: the middle of the copy is at {m:?}, not at ({}, {d})", 40.0 - d));
    }
    // about the point (20, 0): the middle goes to (40 - d, -d)
    let (mut p, si, src) = quarter();
    p.mirror_about_point(si, &[src], 20.0, 0.0);
    let copy = p.sketches[si].entities.last().map(|e| e.id).expect("the copy");
    let m = middle(&p, si, copy);
    if (m.0 - (40.0 - d)).abs() > 1e-6 || (m.1 + d).abs() > 1e-6 {
        failures.push(format!("mirrored about the point (20, 0): the middle of the copy is at {m:?}, not at ({}, {})", 40.0 - d, -d));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
