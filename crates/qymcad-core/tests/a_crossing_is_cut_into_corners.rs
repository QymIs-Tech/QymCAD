//! A CROSSING OF TWO LINES IS CUT INTO CORNERS: two lines going through each other - or one ending on the middle of the
//! other - are cut where they cross, the pieces sharing one ordinary point there, so the corners of the crossing are
//! corners of the drawing. Two lines meeting at their ends are a corner already, and parallel lines do not cross.
//!
//! Reported behaviour: "a sketch fillet or chamfer cannot be put where two lines cross".
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;

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
    Some(s.entities.iter().filter(|e| e.kind.as_line().is_some_and(|qymcad_core::model::LineParts { a, b }| a == at || b == at)).count())
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

/// A FILLET IN ONE QUARTER OF A CROSSING LEAVES ITS TWO SIDES ALONE: the cross cut at its crossing, the corner of the
/// lower left quarter rounded R3, and the halves beyond the crossing dropped - two lines and the arc are left, and
/// nothing stands above or right of the crossing.
#[test]
fn the_halves_beyond_a_corner_of_a_crossing_go() {
    use qymcad_core::model::{CornerAt, CornerCut, FilletSize};
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let one = p.add_line_entity(si, -15.0, 10.0, 25.0, 10.0, Purpose::Real);
    let other = p.add_line_entity(si, 5.0, -10.0, 5.0, 30.0, Purpose::Real);
    let crossing = p.crossing_of(si, [one, other], Point2::new(4.0, 9.0)).expect("the crossing");
    let at = p.cut_at_crossing(si, crossing).expect("the point of the cut");
    // the two halves running left and down from the crossing
    let s = &p.sketches[si];
    let far_end =
        |id: u64, x: f64, y: f64| s.entity(id).and_then(|e| e.kind.as_line()).is_some_and(|l| [l.a, l.b].iter().any(|q| s.point(*q).is_some_and(|q| (q.x - x).abs() < 1e-9 && (q.y - y).abs() < 1e-9)));
    let half = |x: f64, y: f64| s.entities.iter().map(|e| e.id).find(|id| far_end(*id, x, y)).expect("a half");
    let pair = (half(-15.0, 10.0), half(5.0, -10.0));
    assert!(p.cut_corner_set(si, &[CornerAt { point: at, pair }], CornerCut::Fillet(FilletSize::radius(3.0)), None) == 1, "the corner was not rounded");
    let dropped = p.drop_halves_beyond_corner(si, CornerAt { point: at, pair });
    let s = &p.sketches[si];
    let curves: Vec<qymcad_core::model::curve::Curve> = s.entities.iter().filter_map(|e| qymcad_core::model::curve::Curve::of(s, e)).collect();
    let beyond = curves.iter().any(|c| (0..=40).map(|k| c.at(k as f64 / 40.0)).any(|q| q.x > 5.5 || q.y > 10.5));
    assert!(dropped == 2 && curves.len() == 3 && !beyond, "{dropped} halves dropped, {} curves left, one beyond the crossing {beyond}", curves.len());
}
