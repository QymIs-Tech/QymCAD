//! A PROFILE COMES IN AS IT IS DRAWN OR READ: whatever made it - a sketch, a DXF, a glyph of a font - the bridge takes
//! it and gives the body the profile means. Three things a profile carries that OCCT does not take: a segment of zero
//! length (a glyph closed by returning to its first point), a hole running the same way round as the outer loop, a
//! loop that misses closing by a micrometre. The model cleans its own sketches of them; a profile reaching the bridge
//! another way was refused or came out a slab.
use crate::Shape;
use qymcad_core::geom::{encode_loops, Point2, ProfEdge};

const fn p(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

/// The lines through `pts` in order, closed back to the first.
fn lines(pts: &[Point2]) -> Vec<ProfEdge> {
    (0..pts.len()).map(|i| ProfEdge::Line { a: pts[i], b: pts[(i + 1) % pts.len()] }).collect()
}

/// The square 20 x 20 counter-clockwise, and the square 10 x 10 in its middle as a hole running clockwise.
fn outer() -> Vec<ProfEdge> {
    lines(&[p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)])
}

fn volume_of(loops: &[&[ProfEdge]]) -> Option<f64> {
    Shape::extrude_profile(&encode_loops(loops), 10.0).map(|s| s.volume())
}

#[test]
fn a_dirty_profile_comes_in_clean() {
    let mut fails = Vec::new();
    let mut check = |what: &str, got: Option<f64>, want: f64| match got {
        Some(v) if (v - want).abs() < 1e-3 => {}
        other => fails.push(format!("{what}: {other:?} mm^3, {want} expected")),
    };
    // a segment of zero length at the corner where the loop closes, as a glyph of a TTF closes
    let mut zero = outer();
    zero.push(ProfEdge::Line { a: p(0.0, 0.0), b: p(0.0, 0.0) });
    check("a segment of zero length", volume_of(&[&zero]), 4000.0);
    // a hole running counter-clockwise like the outer loop: still a hole
    let hole_same_way = lines(&[p(5.0, 5.0), p(15.0, 5.0), p(15.0, 15.0), p(5.0, 15.0)]);
    check("a hole running the same way as the outer loop", volume_of(&[&outer(), &hole_same_way]), 3000.0);
    // the outer loop clockwise: the profile is the same square
    let clockwise = lines(&[p(0.0, 0.0), p(0.0, 20.0), p(20.0, 20.0), p(20.0, 0.0)]);
    check("the outer loop running clockwise", volume_of(&[&clockwise]), 4000.0);
    // the last edge ends a micrometre short of the first point
    let mut gap = outer();
    if let Some(ProfEdge::Line { b, .. }) = gap.last_mut() {
        *b = p(0.0, 1e-4);
    }
    check("a loop a tenth of a micrometre short of closing", volume_of(&[&gap]), 4000.0);
    assert!(fails.is_empty(), "the bridge does not take a profile as it comes:\n{}", fails.join("\n"));
}
