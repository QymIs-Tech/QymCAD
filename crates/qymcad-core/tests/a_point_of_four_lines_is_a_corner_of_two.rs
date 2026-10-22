//! A POINT WHERE FOUR LINES MEET IS A CORNER OF TWO OF THEM.
//!
//! Reported: two squares drawn to share a single point could not be rounded or bevelled at that point at all - the
//! fillet and the chamfer asked for a corner "of exactly two lines" and found four, and said nothing at all. There is
//! a corner there, four of them: which one is meant is said by where the cursor stands, or by the two lines it was
//! named by. The point alone does not say, and refusing it outright is what made the tool look broken.
//!
//! And the other half of the same question: two lines lying along ONE straight line share a point and no angle, so
//! they make no corner either - the search for one goes on rather than stopping on a pair that has nothing to round.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{ChamferLegs, Project};

/// Two squares meeting in one point at (20, 20) - the first from 0 to 20, the second from 20 to 40. Returns the
/// project, the sketch and the point they share.
fn two_squares_at_one_point() -> (Project, usize, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let line = |p: &mut Project, (ax, ay): (i32, i32), (bx, by): (i32, i32)| p.add_line_entity(si, ax as f64, ay as f64, bx as f64, by as f64, Purpose::Real);
    for (a, b) in [((0, 0), (20, 0)), ((20, 0), (20, 20)), ((20, 20), (0, 20)), ((0, 20), (0, 0))] {
        line(&mut p, a, b);
    }
    for (a, b) in [((20, 20), (40, 20)), ((40, 20), (40, 40)), ((40, 40), (20, 40)), ((20, 40), (20, 20))] {
        line(&mut p, a, b);
    }
    p.regen_sketch(si);
    let shared = point_at(&p, si, 20.0, 20.0).expect("the point the two squares share");
    assert_eq!(p.vertex_edges(si, shared).len(), 4, "setup: the two squares do not meet in four lines at one point");
    (p, si, shared)
}

/// The id of the point of the sketch standing at (x, y).
fn point_at(p: &Project, si: usize, x: f64, y: f64) -> Option<u64> {
    p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-6 && (q.y - y).abs() < 1e-6).map(|q| q.id)
}

/// Whether the drawing holds a point at (x, y).
fn has_point(p: &Project, si: usize, x: f64, y: f64) -> bool {
    point_at(p, si, x, y).is_some()
}

/// Where every point of the drawing stands.
fn all_points(p: &Project, si: usize) -> Vec<(f64, f64)> {
    p.sketches[si].points.iter().map(|q| (q.x, q.y)).collect()
}

/// How many arcs the sketch holds.
fn arcs(p: &Project, si: usize) -> usize {
    p.sketches[si].entities.iter().filter(|e| e.kind.as_arc().is_some()).count()
}

/// The edge at `pid` running out towards `to`.
fn edge_towards(p: &Project, si: usize, pid: u64, to: (f64, f64)) -> u64 {
    p.vertex_edges(si, pid)
        .into_iter()
        .find(|&e| {
            let ends = match p.sketches[si].entities.iter().find(|x| x.id == e).and_then(|x| x.kind.as_line()) {
                Some(qymcad_core::model::LineParts { a, b }) => Some((a, b)),
                _ => None,
            };
            let Some((a, b)) = ends else { return false };
            let other = if a == pid { b } else { a };
            p.sketches[si].points.iter().any(|q| q.id == other && (q.x - to.0).abs() < 1e-6 && (q.y - to.1).abs() < 1e-6)
        })
        .unwrap_or_else(|| panic!("no edge at the shared point runs to {to:?}"))
}

#[test]
fn the_point_alone_names_no_corner_where_four_lines_meet() {
    let (mut p, si, shared) = two_squares_at_one_point();
    assert_eq!(p.vertex_pair(si, shared, None), None, "four lines through a point named a corner out of thin air: which pair is meant is not in the geometry");
    assert_eq!(p.vertex_pairs(si, shared).len(), 6, "the six pairs at the point are what a corner could be made of");
    // and the old way round it - by the point alone - still refuses rather than cutting a corner nobody named
    assert!(!p.chamfer_at_vertex(si, shared, ChamferLegs::equal(5.0), None), "the chamfer cut one of the four corners without being told which");
    assert!(!p.fillet_at_vertex(si, shared, 5.0), "the fillet rounded one of the four corners without being told which");
}

#[test]
fn the_cursor_stands_in_the_corner_it_names() {
    let (p, si, shared) = two_squares_at_one_point();
    let left = edge_towards(&p, si, shared, (0.0, 20.0));
    let down = edge_towards(&p, si, shared, (20.0, 0.0));
    let pair = p.vertex_pair(si, shared, Some((19.0, 19.0))).expect("the cursor inside the first square names the corner it stands in");
    assert_eq!(p.corner_of_pair(si, pair.0, pair.1), p.corner_of_pair(si, left, down), "the pair named is not the two sides of the square the cursor is in: {pair:?}");
    // and the far quadrant names the other square's corner
    let right = edge_towards(&p, si, shared, (40.0, 20.0));
    let up = edge_towards(&p, si, shared, (20.0, 40.0));
    let pair2 = p.vertex_pair(si, shared, Some((21.0, 21.0))).expect("the cursor inside the second square names its corner");
    assert_eq!(p.corner_of_pair(si, pair2.0, pair2.1), p.corner_of_pair(si, right, up), "the far quadrant named the near corner: {pair2:?}");
}

#[test]
fn a_corner_of_the_shared_point_is_cut_where_the_cursor_stands() {
    let (mut p, si, shared) = two_squares_at_one_point();
    assert!(
        p.chamfer_at_vertex_near(si, shared, ChamferLegs { mode: qymcad_core::feature::ChamferMode::TwoDist, first: 5.0, second: 5.0 }, 19.0, 19.0, None),
        "the chamfer of the corner the cursor stands in did not apply"
    );
    assert!(
        has_point(&p, si, 15.0, 20.0) && has_point(&p, si, 20.0, 15.0),
        "the cut of 5 does not meet the sides of the first square at (15, 20) and (20, 15): the points stand at {:?}",
        all_points(&p, si)
    );
    // the second square is untouched: only the corner under the cursor is cut
    assert!(has_point(&p, si, 40.0, 20.0) && has_point(&p, si, 20.0, 40.0), "the far square lost its corner as well: four lines through the point made two cuts of one corner");
}

#[test]
fn a_corner_of_the_shared_point_is_rounded_by_its_two_lines() {
    let (mut p, si, shared) = two_squares_at_one_point();
    let right = edge_towards(&p, si, shared, (40.0, 20.0));
    let up = edge_towards(&p, si, shared, (20.0, 40.0));
    assert!(p.fillet_at_pair(si, (right, up), 5.0), "the two lines of the far corner did not round it: the pair was refused");
    assert_eq!(arcs(&p, si), 1, "the far corner did not become an arc");
    assert!(has_point(&p, si, 20.0, 25.0), "the arc of radius 5 does not meet the sides of the far square 5 from the shared point: {:?}", all_points(&p, si));
}

#[test]
fn two_lines_in_one_straight_line_make_no_corner() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    // one line drawn as two halves: they meet at a point and make a straight angle of 180 degrees
    let a = p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    let b = p.add_line_entity(si, 20.0, 0.0, 40.0, 0.0, Purpose::Real);
    p.regen_sketch(si);
    let mid = point_at(&p, si, 20.0, 0.0).expect("the point where the two halves meet");
    assert_eq!(p.shared_vertex(si, a, b), Some(mid), "setup: the two halves do not share their point");
    assert_eq!(p.corner_of_pair(si, a, b), None, "a straight angle of 180 degrees named a corner: there is nothing to round along one straight line");
    assert_eq!(p.vertex_pair(si, mid, None), None, "the pair of two straight halves was taken for a corner by the point alone");
    assert_eq!(p.vertex_pair(si, mid, Some((25.0, 5.0))), None, "the cursor above the line named a corner out of two halves lying along one line");
    assert!(!p.chamfer_lines_of_pair(si, (a, b), ChamferLegs::equal(5.0), None), "a chamfer was cut along a straight line between its own halves");
    assert!(!p.fillet_at_pair(si, (a, b), 5.0), "an arc was put between two halves of one straight line");
}

#[test]
fn the_field_is_bounded_by_the_tightest_of_the_corners() {
    let (p, si, shared) = two_squares_at_one_point();
    let tightest = p
        .vertex_pairs(si, shared)
        .iter()
        .filter_map(|pair| p.corner_limit_of_pair(si, shared, *pair, qymcad_core::model::CornerTool::Chamfer(qymcad_core::feature::ChamferMode::TwoDist)))
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))));
    assert!(tightest.is_some(), "no corner at the shared point has a bound at all");
    assert_eq!(
        p.corner_limit(si, shared, qymcad_core::model::CornerTool::Chamfer(qymcad_core::feature::ChamferMode::TwoDist)),
        tightest,
        "the bound of a point with four lines through it is not the tightest corner among them"
    );
    // the corner under the cursor is bounded by its own two sides
    let bound = p.corner_limit_near(si, shared, qymcad_core::model::CornerTool::Chamfer(qymcad_core::feature::ChamferMode::TwoDist), 19.0, 19.0).expect("the corner the cursor stands in has a bound");
    assert!(bound > 0.0 && bound <= tightest.unwrap_or(0.0), "the bound of the corner under the cursor ({bound}) does not fit among the corners there");
}

/// THE CORNER OF ONE NAMED LINE at a point where four meet: the pair is that line and one of the two neighbours of it,
/// and the cursor says which of them by the sector it stands in.
#[test]
fn the_corner_of_a_named_line_is_the_sector_the_cursor_stands_in() {
    let (p, si, shared) = two_squares_at_one_point();
    let down = edge_towards(&p, si, shared, (20.0, 0.0));
    let left = edge_towards(&p, si, shared, (0.0, 20.0));
    let right = edge_towards(&p, si, shared, (40.0, 20.0));
    let up = edge_towards(&p, si, shared, (20.0, 40.0));
    let same = |a: Option<(u64, u64)>, b: (u64, u64)| a.is_some_and(|q| (q.0 == b.0 && q.1 == b.1) || (q.0 == b.1 && q.1 == b.0));
    // THE NEAR SQUARE is down and to the left of the point, the far one up and to the right: the two sectors of the
    // line running down are the corners either side of it
    assert!(same(p.vertex_pair_through(si, shared, down, Some((19.0, 19.0))), (down, left)), "the cursor in the near square did not name the corner on that side of the line");
    assert!(same(p.vertex_pair_through(si, shared, down, Some((21.0, 21.0))), (down, right)), "the cursor in the far square is nearer the middle of the other sector, so that is the corner it names");
    // and with no cursor to read, the turn of the circle gives the first of them
    assert!(same(p.vertex_pair_through(si, shared, down, None), (down, right)), "with no cursor the drawing does not begin at the corner of the line, and the two are not equal");
    // WHATEVER THE CURSOR, EVERY CORNER IT NAMES IS ONE THE LINE TAKES PART IN
    for line in [down, left, right, up] {
        for at in [(19.0, 19.0), (21.0, 21.0), (25.0, 20.0), (20.0, 25.0), (15.0, 20.0), (20.0, 15.0)] {
            let pair = p.vertex_pair_through(si, shared, line, Some(at)).unwrap_or_else(|| panic!("the cursor at {at:?} named no corner of the line {line}"));
            assert!(pair.0 == line || pair.1 == line, "the corner named is not one of the line's: {pair:?} against the line {line}");
            assert_eq!(p.corner_of_pair(si, pair.0, pair.1), Some(shared), "the pair named is not a corner at the point: {pair:?}");
        }
    }
}

/// A LINE THAT DOES NOT REACH THE POINT NAMES NO CORNER THROUGH IT, whatever the cursor says.
#[test]
fn a_line_that_does_not_reach_the_point_names_no_corner_through_it() {
    let (mut p, si, shared) = two_squares_at_one_point();
    let away = p.add_line_entity(si, 80.0, 80.0, 100.0, 80.0, Purpose::Real);
    p.regen_sketch(si);
    assert_eq!(p.vertex_pair_through(si, shared, away, Some((21.0, 21.0))), None, "a line that never touches the point named a corner at it");
}
