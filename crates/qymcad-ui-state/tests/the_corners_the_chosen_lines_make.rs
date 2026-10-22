//! THE CORNERS THE CHOSEN LINES MAKE, AND WHAT A CLICK AT A POINT DOES TO THEM.
//!
//! The fillet and the chamfer are cut off corners, and a corner is not a thing a person points at: it is where two
//! chosen lines meet. So the set of corners is not written down by a person picking corners one after another — it
//! follows from the lines that are chosen, and every rule here is about what follows from what.
//!
//! **THE LINES WAITING AT A POINT ARE PAIRED TWO AT A TIME, IN THE ORDER THEY WERE CHOSEN.** Four lines through one
//! point give two corners: the first with the second, the third with the fourth.
//!
//! **A CORNER IS REMEMBERED, NOT READ OFF THE SELECTION EVERY TIME.** Letting a line go takes its corner with it,
//! and the line that stays does not pair again with a neighbour of the one that left — so of A, B, C, D chosen
//! across one point, letting B go leaves C D standing and A waiting, and letting C go as well leaves A D as the
//! corner. Reading the pairs off the selection afresh made A C instead, which nobody had asked for.
//!
//! **A CLICK AT A POINT IS ABOUT THAT POINT ALONE**: the chosen lines do not move. A corner named there is taken
//! away, a corner of the lines standing there is put away (and comes back on the same click), and a point where
//! nothing stands takes the corner the cursor read.
use qymcad_core::feature::Purpose;
use qymcad_core::model::Project;
use qymcad_ui_state::{CornerSet, PointAct};

/// The four lines of a cross standing on one point, in the order a person chose them.
fn cross() -> (Project, usize, [u64; 4], u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ids: [u64; 4] =
        [((0.0, 0.0), (2000.0, 0.0)), ((0.0, 0.0), (0.0, 2000.0)), ((0.0, 0.0), (-2000.0, 0.0)), ((0.0, 0.0), (0.0, -2000.0))].map(|(a, b)| p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real));
    p.regen_sketch(si);
    let at = p.sketches[si].points.iter().find(|q| q.x.abs() < 1e-6 && q.y.abs() < 1e-6).expect("the four lines meet on one point").id;
    assert_eq!(p.vertex_edges(si, at).len(), 4, "setup: four lines do not stand on the point");
    (p, si, ids, at)
}

/// Two lines along ONE straight line through (20, 0), a third coming down from it, and a triangle elsewhere.
fn straight_joint() -> (Project, usize, [u64; 4], u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let mut ids = Vec::new();
    for (a, b) in [((0.0, 0.0), (20.0, 0.0)), ((20.0, 0.0), (40.0, 0.0)), ((20.0, 0.0), (20.0, 20.0)), ((0.0, 30.0), (30.0, 30.0))] {
        ids.push(p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real));
    }
    p.regen_sketch(si);
    let at = p.sketches[si].points.iter().find(|q| (q.x - 20.0).abs() < 1e-6 && q.y.abs() < 1e-6).expect("the two lines meet at (20, 0)").id;
    let ids: [u64; 4] = ids.try_into().unwrap_or_else(|_| unreachable!("four lines were drawn"));
    (p, si, ids, at)
}

/// A closed triangle of three lines, and the point each pair of them makes.
fn triangle() -> (Project, usize, [u64; 3], [u64; 3]) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ids: [u64; 3] = [((0.0, 0.0), (60.0, 0.0)), ((60.0, 0.0), (30.0, 50.0)), ((30.0, 50.0), (0.0, 0.0))].map(|(a, b)| p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real));
    p.regen_sketch(si);
    let points = [
        p.corner_of_pair(si, ids[0], ids[1]).expect("the first two lines meet"),
        p.corner_of_pair(si, ids[1], ids[2]).expect("the second and the third meet"),
        p.corner_of_pair(si, ids[2], ids[0]).expect("the third and the first meet"),
    ];
    (p, si, ids, points)
}

/// THE PAIRS OF THE CORNERS THAT STAND, each written the same way round, so that a comparison says what it means.
fn pairs(set: &CornerSet) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = set.made.iter().map(|c| (c.pair.0.min(c.pair.1), c.pair.0.max(c.pair.1))).collect();
    out.extend(set.points.iter().map(|c| (c.pair.0.min(c.pair.1), c.pair.0.max(c.pair.1))));
    out.sort_unstable();
    out
}

/// THE CORNER THE CROSS EXAMPLE ENDS AT, step by step: A, then AB, then CD, and what letting a line go does.
#[test]
fn the_lines_through_one_point_are_paired_two_at_a_time_in_the_order_they_were_chosen() {
    let (p, si, l, _at) = cross();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0]]);
    assert_eq!(pairs(&set), Vec::<(u64, u64)>::new(), "one chosen line makes no corner: it is half of one");
    set.follow(&p, si, &[l[0], l[1]]);
    assert_eq!(pairs(&set), vec![(l[0], l[1])], "the second chosen line makes the first corner");
    set.follow(&p, si, &[l[0], l[1], l[2]]);
    assert_eq!(pairs(&set), vec![(l[0], l[1])], "a third line on that point waits: the first two are spoken for");
    set.follow(&p, si, &[l[0], l[1], l[2], l[3]]);
    assert_eq!(pairs(&set), vec![(l[0], l[1]), (l[2], l[3])], "the fourth line makes the second corner, with the third");
    // LET THE SECOND OF THE FIRST PAIR GO: its corner goes with it, and the line that stays does not pair again.
    set.follow(&p, si, &[l[0], l[2], l[3]]);
    assert_eq!(pairs(&set), vec![(l[2], l[3])], "letting the second line go took AB away and made AC, which nobody asked for");
    // AND LET THE THIRD GO AS WELL: what is left standing is the pair of the two lines that are left.
    set.follow(&p, si, &[l[0], l[3]]);
    assert_eq!(pairs(&set), vec![(l[0], l[3])], "of A, C and D with C let go, the corner is AD and not AC");
}

/// TWO LINES ALONG ONE STRAIGHT LINE ARE NO CONFLICT, and a third one arriving there is.
#[test]
fn a_third_line_at_a_straight_joint_is_refused_with_the_point_named() {
    let (p, si, l, at) = straight_joint();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0], l[1]]);
    assert_eq!(pairs(&set), Vec::<(u64, u64)>::new(), "two lines along one straight line make no corner: there is no angle there");
    assert_eq!(qymcad_ui_state::straight_joint_in_the_way(&p, si, l[2], &[l[0], l[1]], &[]), Some(at), "a third line at that point was not seen as a conflict");
    assert_eq!(qymcad_ui_state::straight_joint_in_the_way(&p, si, l[0], &[l[1]], &[]), None, "one chosen line at a point is no straight joint yet");
    assert_eq!(qymcad_ui_state::straight_joint_in_the_way(&p, si, l[3], &[l[0], l[1]], &[]), None, "a line that does not reach the joint is no conflict");
}

/// A CLOSED CONTOUR OF CHOSEN LINES IS THREE CORNERS, cut by one answer.
#[test]
fn a_contour_of_chosen_lines_is_a_corner_at_each_of_its_points() {
    let (p, si, l, points) = triangle();
    assert_eq!(points.len(), 3, "setup: a triangle does not have three corner points");
    let mut set = CornerSet::default();
    set.follow(&p, si, &l);
    assert_eq!(pairs(&set).len(), 3, "three lines of a closed contour make three corners: {:?}", pairs(&set));
    for c in &points {
        assert!(set.made.iter().any(|k| k.point == *c), "no corner stands at {c}: {:?}", set.made);
    }
    // AND THE VALUE IS HELD BY THE TIGHTEST OF THEM, with the lines between them spending on themselves.
    let limit = set.limit(&p, si, qymcad_core::model::CornerTool::Fillet, qymcad_core::model::FilletBy::Radius).expect("three corners hold a value");
    let one = p.corner_limit_of_pair(si, points[0], (l[0], l[1]), qymcad_core::model::CornerTool::Fillet).expect("one corner holds a value");
    assert!(limit <= one + 1e-9, "one value cuts all three, so the set takes no more than the tightest of them");
    assert!(limit > 0.0, "the corners of a triangle hold a positive value");
}

/// A CLICK AT A POINT PUTS A CORNER OF THE LINES AWAY, AND THE SAME CLICK BRINGS IT BACK.
#[test]
fn a_click_at_a_point_puts_a_corner_away_and_the_same_click_brings_it_back() {
    let (p, si, l, at) = cross();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0], l[1], l[2], l[3]]);
    let both = pairs(&set);
    assert_eq!(both.len(), 2, "setup: the four lines of the cross did not make two corners");
    assert_eq!(set.click_at_point(&p, si, at, None), PointAct::LineHidden, "a click at a point with a corner of the lines on it did not put that corner away");
    assert!(!pairs(&set).contains(&(l[0], l[1])), "the corner put away is still in the set");
    // A CORNER PUT AWAY IS NOT DELETED: the lines keep the ends they have, and the same click brings it back.
    assert_eq!(set.click_at_point(&p, si, at, None), PointAct::LineShown, "the second click at the point did not bring the corner back");
    assert_eq!(pairs(&set), both, "the corner came back as it was");
}

/// A CORNER PUT AWAY STAYS PUT AWAY, whatever else changes in the selection.
#[test]
fn a_corner_put_away_stays_put_away() {
    let (p, si, l, at) = cross();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0], l[1], l[2], l[3]]);
    set.click_at_point(&p, si, at, None);
    set.follow(&p, si, &[l[0], l[1], l[2], l[3]]); // the selection is read again and says the same thing
    assert!(!set.made.iter().any(|c| c.point == at), "a corner that was put away came back by itself");
    set.follow(&p, si, &[l[0], l[1]]); // and the lines it was made of are still chosen
    assert!(!set.made.iter().any(|c| c.point == at), "a corner that was put away came back while its lines were still chosen");
}

/// A POINT WHERE NOTHING STANDS TAKES THE CORNER THE CURSOR READ, AND THE SAME CLICK TAKES IT AWAY.
#[test]
fn a_corner_named_at_a_point_is_taken_away_by_the_same_click() {
    let (p, si, l, at) = cross();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0]]); // one line chosen: no corner stands at the point
    let read = set.read_at_point(&p, si, at, Some((10.0, 10.0))).expect("a corner is read where a line stands and a point is free");
    assert!(read.0 == l[0] || read.1 == l[0], "the corner is not of the line it is read through: {read:?}");
    assert_eq!(set.click_at_point(&p, si, at, Some(read)), PointAct::PointAdded, "the click at the point did not name a corner");
    assert_eq!(set.points.len(), 1, "the corner named at the point is not in the set");
    assert!(set.occupied(at), "a point with a corner of its own is taken: it is one place, read once");
    assert_eq!(set.read_at_point(&p, si, at, Some((10.0, 10.0))), None, "a point with a corner is read again as a free one");
    assert_eq!(set.click_at_point(&p, si, at, Some(read)), PointAct::PointRemoved, "the second click at the point did not take the corner away");
    assert!(set.points.is_empty(), "the corner named at the point is still there");
}

/// A CORNER NAMED AT A POINT DIES WHEN A CORNER OF THE LINES STANDS IN ITS PLACE, AND WHEN A LINE IS LET GO.
#[test]
fn a_corner_named_at_a_point_gives_way_to_the_lines_and_to_their_going() {
    let (p, si, l, at) = cross();
    let mut set = CornerSet::default();
    set.follow(&p, si, &[l[0]]);
    let read = set.read_at_point(&p, si, at, Some((10.0, 10.0))).expect("a corner is read");
    set.click_at_point(&p, si, at, Some(read));
    // THE LINES ARRIVE: a corner of two chosen lines is the corner at that point, and the one named there goes.
    set.follow(&p, si, &[l[0], l[1]]);
    assert!(set.points.is_empty(), "the corner named at the point stood on beside the corner of the lines: {:?}", set);
    assert_eq!(set.made.len(), 1, "the corner of the two chosen lines is the one at the point");
    // AND A CORNER NAMED AT A POINT DIES WITH ONE OF ITS LINES: it is a corner of those two lines.
    let (q, sqi, ql, qpoints) = triangle();
    let qat = qpoints[0]; // the point where the first two lines meet
    let mut other = CornerSet::default();
    other.follow(&q, sqi, &[ql[0]]);
    let read = other.read_at_point(&q, sqi, qat, None).expect("the corner of the two lines at the point is read");
    other.click_at_point(&q, sqi, qat, Some(read));
    assert_eq!(other.points.len(), 1, "setup: the corner named at the point is not in the set");
    other.follow(&q, sqi, &[ql[0]]); // the same one line is still chosen: the corner stands
    assert_eq!(other.points.len(), 1, "a corner named at a point died while both its lines were still chosen");
    other.follow_from(&q, sqi, &[ql[0]], &[]);
    assert!(other.points.is_empty(), "a corner named at a point outlived the line it was named from");
}

/// THE CORNER THE CURSOR READS AT A POINT IS THE ONE IT STANDS BETWEEN, WHERE MORE THAN TWO LINES ARE THERE.
#[test]
fn the_corner_read_at_a_point_is_the_one_the_cursor_stands_between() {
    let (p, si, l, at) = cross();
    let set = CornerSet::default();
    // THE TWO SECTORS EITHER SIDE OF THE FIRST LINE: the cursor stands above it, and below it.
    let above = set.read_at_point(&p, si, at, Some((10.0, 10.0)));
    let below = set.read_at_point(&p, si, at, Some((10.0, -10.0)));
    let (Some(a), Some(b)) = (above, below) else { panic!("a corner is read on either side of the line") };
    assert_ne!(Some(a), Some(b), "the same corner was read on both sides of the line: the cursor has no word in it");
    for pair in [a, b] {
        assert!(pair.0 == l[0] || pair.1 == l[0], "the corner read through a line is not of that line: {pair:?}");
        assert_ne!(pair.0, pair.1, "a line is a corner of itself");
    }
}
/// **A CORNER PUT AWAY GOES WITH ITS LINES.** What a click at a point puts away is a corner OF CHOSEN LINES, and a
/// line a person has let go of is one they no longer want: the corner at its end is gone with it, put away or not.
/// Choosing the line again makes the corner again, and this time it stands - a corner that has been forgotten is a
/// corner nobody asked to keep.
#[test]
fn a_corner_put_away_goes_when_a_line_of_it_is_let_go() {
    let (p, si, l, points) = triangle();
    let mut set = CornerSet::default();
    set.follow(&p, si, &l);
    assert_eq!(pairs(&set).len(), 3, "setup: a contour of three lines makes three corners");
    assert_eq!(set.click_at_point(&p, si, points[0], None), PointAct::LineHidden, "the corner was not put away");
    assert_eq!((set.hidden.len(), pairs(&set).len()), (1, 2), "a corner put away is remembered, not deleted");
    // A LINE OF THAT CORNER LET GO: it takes the corner with it, and the point is free for a corner again.
    set.follow_from(&p, si, &l, &[l[1], l[2]]);
    assert_eq!(pairs(&set).len(), 1, "only the corner of the two lines that stayed is left standing: {:?}", pairs(&set));
    assert!(set.hidden.is_empty(), "the corner put away outlived the line it was made of: {:?}", set);
    assert!(!set.occupied(points[0]), "the point of a corner whose line was let go still answers as taken");
    // AND THE LINE CHOSEN AGAIN: the corner is made afresh, and it STANDS rather than being put away a second time.
    set.follow_from(&p, si, &[l[1], l[2]], &l);
    assert_eq!(pairs(&set).len(), 3, "three lines make three corners and the one that was let go and chosen again is one of them: {:?}", pairs(&set));
    assert!(set.made.iter().any(|c| c.point == points[0]), "the corner that came back is not standing at its point: {:?}", set);
    assert!(set.hidden.is_empty(), "the corner came back put away instead of standing: {:?}", set);
    // A STRANGER TO THAT CORNER CHANGES NOTHING: the corner of the two lines is put away, and letting a third line go
    // and choosing it again leaves it exactly where it was.
    let mut set = CornerSet::default();
    set.follow(&p, si, &l);
    assert_eq!(set.click_at_point(&p, si, points[0], None), PointAct::LineHidden);
    set.follow_from(&p, si, &l, &[l[0], l[1]]);
    assert_eq!(set.hidden.len(), 1, "a line that is a stranger to the corner put away took it with it: {:?}", set);
    set.follow_from(&p, si, &[l[0], l[1]], &l);
    assert_eq!((set.hidden.len(), pairs(&set).len()), (1, 2), "the corner put away did not survive a line that had nothing to do with it: {:?}", set);
}
