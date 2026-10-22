//! A SET OF CORNERS IS CUT WHOLE OR NOT AT ALL: one answer cuts every corner of the set, and where one corner does not
//! take the cut the sketch is left as it was - no corner cut, no point, line or constraint added or moved.
//!
//! Reported behaviour: a set was cut on the corners that took the value and left whole on the one that did not.
use qymcad_core::feature::{ChamferMode, Purpose};
use qymcad_core::model::{ChamferLegs, CornerAt, CornerCut, Project};

/// A quadrilateral with a short left side and a long right one: (0, 0), (40, 0), (40, 50), (0, 30). Answers the
/// corners of its base, the right one first, each by the base and its side.
fn base_corners() -> (Project, usize, [CornerAt; 2]) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let c = [(0.0, 0.0), (40.0, 0.0), (40.0, 50.0), (0.0, 30.0)];
    let sides: Vec<u64> = (0..4).map(|i| p.add_line_entity(si, c[i].0, c[i].1, c[(i + 1) % 4].0, c[(i + 1) % 4].1, Purpose::Real)).collect();
    p.regen_sketch(si);
    let point_at = |p: &Project, x: f64, y: f64| p.sketches[si].points.iter().find(|q| (q.x - x).hypot(q.y - y) < 1e-9).map(|q| q.id).expect("a corner of the quadrilateral");
    let (left, right) = (point_at(&p, 0.0, 0.0), point_at(&p, 40.0, 0.0));
    (p, si, [CornerAt { point: right, pair: (sides[0], sides[1]) }, CornerAt { point: left, pair: (sides[0], sides[3]) }])
}

#[test]
fn a_set_one_corner_of_which_does_not_take_the_chamfer_is_not_cut() {
    let (mut p, si, corners) = base_corners();
    let before = format!("{:?}", p.sketches[si]);
    // legs of 3 on the base and 35 on the side: the right side of 50 takes it, the left one of 30 does not
    let legs = ChamferLegs { mode: ChamferMode::TwoDist, first: 3.0, second: 35.0 };
    assert_eq!(p.cut_corner_set(si, &corners, CornerCut::Chamfer(legs), None), 0, "a set one corner of which does not take the chamfer answered as cut");
    assert_eq!(format!("{:?}", p.sketches[si]), before, "the sketch was changed by a set that was not cut");
}

#[test]
fn a_set_every_corner_of_which_takes_the_chamfer_is_cut_whole() {
    let (mut p, si, corners) = base_corners();
    let legs = ChamferLegs { mode: ChamferMode::TwoDist, first: 3.0, second: 20.0 };
    assert_eq!(p.cut_corner_set(si, &corners, CornerCut::Chamfer(legs), None), 2, "both corners take legs of 3 and 20");
    let lines = p.sketches[si].entities.iter().filter(|e| e.kind.as_line().is_some()).count();
    assert_eq!(lines, 6, "four sides and two cuts");
}
