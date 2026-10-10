//! A CROSSING OF TWO LINES IS CUT INTO CORNERS: two lines going through each other - or one ending on the middle of the
//! other - are cut where they cross, the pieces sharing one ordinary point there, so the corners of the crossing are
//! corners of the drawing. Two lines meeting at their ends are a corner already, and parallel lines do not cross.
//!
//! Reported behaviour: "a sketch fillet or chamfer cannot be put where two lines cross".
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{EntityKind, Project};

/// TWO LINES DRAWN APART: their ends.
struct Two {
    one: [(f64, f64); 2],
    other: [(f64, f64); 2],
}

/// The sketch with the two lines, and what is cut near `near`: answers how many lines end at the point the cut answered,
/// or `None` when no crossing was found.
fn cut(two: Two, near: (f64, f64)) -> Option<usize> {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    for [a, b] in [two.one, two.other] {
        p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real);
    }
    let crossing = p.crossing_near(si, Point2::new(near.0, near.1), 2.0)?;
    let at = p.cut_at_crossing(si, crossing)?;
    let s = &p.sketches[si];
    assert!(!s.system_ids().contains(&at), "the cut answered a point of the frame, which is no corner of the drawing");
    Some(s.entities.iter().filter(|e| matches!(e.kind, EntityKind::Line { a, b } if a == at || b == at)).count())
}

#[test]
fn a_crossing_is_cut_and_a_corner_is_not() {
    let mut failures = Vec::new();
    let cases = [
        ("a cross off the origin", Two { one: [(-15.0, 10.0), (25.0, 10.0)], other: [(5.0, -10.0), (5.0, 30.0)] }, (6.0, 11.0), Some(4)),
        ("a cross on the origin", Two { one: [(-20.0, 0.0), (20.0, 0.0)], other: [(0.0, -20.0), (0.0, 20.0)] }, (1.0, 1.0), Some(4)),
        ("a T: one line ends on the middle of the other", Two { one: [(-20.0, 0.0), (20.0, 0.0)], other: [(5.0, 0.0), (5.0, 20.0)] }, (6.0, 1.0), Some(3)),
        ("a slanted cross", Two { one: [(0.0, 0.0), (30.0, 10.0)], other: [(0.0, 10.0), (30.0, 0.0)] }, (15.0, 6.0), Some(4)),
        ("two parallel lines", Two { one: [(0.0, 0.0), (30.0, 0.0)], other: [(0.0, 1.0), (30.0, 1.0)] }, (15.0, 0.5), None),
        ("a corner: the two end there", Two { one: [(0.0, 0.0), (30.0, 0.0)], other: [(0.0, 0.0), (0.0, 30.0)] }, (1.0, 1.0), None),
    ];
    for (name, two, near, want) in cases {
        let got = cut(two, near);
        if got != want {
            failures.push(format!("{name}: {got:?} lines end at the cut, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
