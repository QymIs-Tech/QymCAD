//! ANY LEFT CLICK WITHOUT SHIFT IS A SINGLE SELECTION — WHATEVER IT LANDED ON.
//!
//! The fillet and the chamfer cut a set of corners, and the set is what the chosen lines make. More than one line is
//! chosen with Shift held, and a multi-selection that outlives the key is a mode a person cannot see and cannot get
//! out of: reported, a click that was meant for one corner cut the whole set instead. So every left click without
//! Shift leaves the multi-selection behind — a line, a point, or nothing at all — and what it then chooses is a
//! matter of what it landed on, and only that.
//!
//! **THE LINES CHOSEN STAY CHOSEN WHERE THE CLICK MEANT NOTHING.** A click on empty space says which mode the tool
//! is in; it is not a command to forget the drawing.
//!
//! **A CLICK ON A POINT WITHOUT SHIFT IS A SINGLE SELECTION OF A POINT, LIKE A CLICK ON A LINE IS OF A LINE.** It was
//! the one exception, and the exception is what made the tool read as two tools. A point already in the set clicked
//! again without Shift is the one thing to let go of, and the whole set goes with it.
//!
//! **THE CLICK THAT OPENS THE FIELD GROWS THE SET RATHER THAN SINGLING IT OUT**: the field is closed when the tool
//! is taken, so there is no set in hand to single out, and the lines standing chosen are the beginning of one.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::{CornerClick, CornerInput, CornerOver, SketchSelection, CORNER_SET};

/// A closed square of four lines, and the id each of them answers to.
fn square() -> (Project, usize, [u64; 4]) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let ids: [u64; 4] =
        [((0.0, 0.0), (60.0, 0.0)), ((60.0, 0.0), (60.0, 40.0)), ((60.0, 40.0), (0.0, 40.0)), ((0.0, 40.0), (0.0, 0.0))].map(|(a, b)| p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real));
    p.regen_sketch(si);
    (p, si, ids)
}

/// THE TOOL IN HAND WITH THE FIELD OPEN FOR THE WHOLE SET, and a selection that starts empty.
fn tool() -> (CornerInput, SketchSelection) {
    (CornerInput { at: Some((0, CORNER_SET, false)), ..CornerInput::default() }, SketchSelection::default())
}

/// THE TOOL TAKEN AND NOTHING POINTED AT YET: the field is closed until the first click.
fn taken() -> (CornerInput, SketchSelection) {
    (CornerInput::default(), SketchSelection::default())
}

/// THE LINES CHOSEN, in the order they were chosen: that order is what pairs them two at a time at a point.
fn chosen(sel: &SketchSelection) -> Vec<Id> {
    sel.curves().collect()
}

/// A LINE WITH SHIFT JOINS THE SET, AND A SECOND ONE WITH SHIFT MAKES A CORNER OF THE TWO.
#[test]
fn shift_clicks_grow_the_set_and_the_second_makes_a_corner() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true), CornerClick::LineChosen(l[0]));
    assert_eq!(corner.set.standing().len(), 0, "one chosen line makes no corner: it is half of one");
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true), CornerClick::LineChosen(l[1]));
    assert_eq!(corner.set.standing().len(), 1, "the second chosen line made the first corner: {:?}", corner.set);
    assert!(corner.shifted, "a Shift click leaves the tool in the mode that makes a set");
}

/// A CLICK WITHOUT SHIFT IS A SINGLE SELECTION: only the line under the cursor is chosen, and the corners of the
/// set are the ones that follow from it.
#[test]
fn a_click_without_shift_leaves_one_line_and_the_corners_that_follow() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true);
    assert_eq!(corner.set.standing().len(), 1, "setup: two lines chosen make one corner");
    // AND A CLICK WITHOUT SHIFT ON A LINE: the whole set is not forgotten piecemeal, it is that one line.
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[2]), false), CornerClick::LineChosen(l[2]));
    assert_eq!(chosen(&sel), vec![l[2]], "a click without Shift chose more than the line under the cursor");
    assert_eq!(corner.set.standing().len(), 0, "the corners of the set the click left behind are still standing");
    assert!(!corner.shifted, "a click without Shift left the tool in the mode that makes a set");
}

/// A CLICK ON NOTHING IS STILL A CLICK, AND IT IS STILL ONE WITHOUT SHIFT: THE MODE IS SINGLE FROM HERE, AND THE
/// LINES CHOSEN STAY CHOSEN.
#[test]
fn a_click_on_nothing_leaves_the_mode_and_the_lines_alone() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true);
    let before = corner.set.standing();
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Nothing, false), CornerClick::SingleMode);
    assert_eq!(chosen(&sel), vec![l[0], l[1]], "a click on nothing at all forgot the lines that were chosen");
    assert_eq!(corner.set.standing(), before, "a click on nothing at all took the corners of the set away");
    assert!(!corner.shifted, "a click on nothing without Shift left the mode that makes a set");
}

/// A CLICK ON A POINT WITHOUT SHIFT IS A SINGLE SELECTION OF A POINT: the lines chosen go, the corners they made go
/// with them, and the point is left alone in the set.
#[test]
fn a_click_on_a_point_without_shift_leaves_that_point_alone() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true);
    // A POINT OF THE SQUARE WHERE NO CORNER OF THE CHOSEN LINES STANDS, so that what the click does there is
    // about that point and about nothing else.
    let free = p.corner_of_pair(si, l[2], l[3]).expect("the other two lines of the square meet");
    let (x, y) = p.point_xy(si, free).expect("the point stands in the drawing");
    assert_eq!(corner.set.standing().len(), 1, "setup: the two chosen lines make a corner");
    let act = qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Point { id: free, at: (x, y) }, false);
    // THE WHOLE SET GOES AND THIS POINT IS ALL THAT IS LEFT OF IT.
    assert_eq!(chosen(&sel), Vec::<Id>::new(), "a click on a point without Shift kept the lines chosen");
    assert_eq!(act, CornerClick::AtPoint(qymcad_ui_state::PointAct::PointAdded), "a click at a free point named no corner: {act:?}");
    assert_eq!(corner.set.standing(), vec![(free, (l[2], l[3]))], "the set is not this point alone: {:?}", corner.set);
    // AND IT IS A CLICK WITHOUT SHIFT, so the mode is single from here.
    assert!(!corner.shifted, "a click on a point without Shift left the mode that makes a set");
}

/// A POINT ALREADY IN THE SET, CLICKED AGAIN WITHOUT SHIFT, IS THE ONE THING TO LET GO OF: the whole set goes, so
/// that a set of nothing is not left standing as though it were an answer.
#[test]
fn a_chosen_point_clicked_again_without_shift_empties_the_set() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true);
    let free = p.corner_of_pair(si, l[2], l[3]).expect("the other two lines of the square meet");
    let (x, y) = p.point_xy(si, free).expect("the point stands in the drawing");
    let at = CornerOver::Point { id: free, at: (x, y) };
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, at, false);
    assert_eq!(corner.set.standing().len(), 1, "setup: the point alone is in the set");
    let act = qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, at, false);
    assert_eq!(act, CornerClick::AtPoint(qymcad_ui_state::PointAct::PointRemoved), "the second click said something else: {act:?}");
    assert!(corner.set.standing().is_empty(), "a set of nothing is standing as though it were an answer: {:?}", corner.set);
    assert_eq!(chosen(&sel), Vec::<Id>::new(), "a click that emptied the set left a line chosen");
}

/// WITH SHIFT A CLICK ON A POINT IS STILL ABOUT THAT POINT ALONE, and the lines chosen stay chosen.
#[test]
fn a_shift_click_on_a_point_keeps_the_lines_chosen() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), true);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true);
    let corners_before = corner.set.standing();
    let free = p.corner_of_pair(si, l[2], l[3]).expect("the other two lines of the square meet");
    let (x, y) = p.point_xy(si, free).expect("the point stands in the drawing");
    let act = qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Point { id: free, at: (x, y) }, true);
    assert_eq!(chosen(&sel), vec![l[0], l[1]], "a Shift click on a point changed which lines are chosen");
    assert!(
        corners_before.iter().all(|&(pid, _)| corner.set.standing().contains(&(pid, corner.set.made.iter().find(|c| c.point == pid).expect("the corner of the lines stands").pair))),
        "a Shift click on a point took a corner of the chosen lines away"
    );
    assert_eq!(act, CornerClick::AtPoint(qymcad_ui_state::PointAct::PointAdded), "a Shift click at a free point named no corner: {act:?}");
    assert_eq!(corner.set.standing().len(), 2, "the point was not added to the set: {:?}", corner.set);
}

/// THE FIELD IS CLOSED WHEN THE TOOL IS TAKEN, SO THE FIRST CLICK GROWS THE SET RATHER THAN SINGLING IT OUT: a
/// person who chose a contour and then took the fillet meant that contour.
#[test]
fn the_first_click_after_taking_the_tool_grows_the_set() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = taken();
    sel.items.push(qymcad_ui_state::SketchItem::curve(l[0]));
    sel.items.push(qymcad_ui_state::SketchItem::curve(l[1]));
    assert!(corner.at.is_none(), "the field was open before anything was pointed at");
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[2]), false), CornerClick::LineChosen(l[2]));
    assert_eq!(chosen(&sel), vec![l[0], l[1], l[2]], "the first click after taking the tool threw the chosen lines away");
    // THREE SIDES OF A SQUARE MEET TWO CORNERS, one at (60, 0) and one at (60, 40): the first two lines make the
    // first, and the third the second.
    assert_eq!(corner.set.standing().len(), 2, "three chosen sides of a square do not make two corners: {:?}", corner.set);
}

/// A LINE THAT IS CHOSEN AND IS CLICKED AGAIN IS LET GO OF, AND ITS CORNER GOES WITH IT.
#[test]
fn a_line_clicked_again_is_let_go_of_and_its_corner_goes_with_it() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    for &e in &l[..2] {
        qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(e), true);
    }
    assert_eq!(corner.set.standing().len(), 1, "setup: two lines chosen make one corner");
    // LET THE SECOND GO: the corner goes with it, and the line that stays does not pair again with a neighbour of
    // the one that left.
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), true), CornerClick::LineLetGo(l[1]));
    assert_eq!(chosen(&sel), vec![l[0]], "the line that was let go is still chosen");
    assert_eq!(corner.set.standing().len(), 0, "a corner of a line that was let go is still standing");
}

/// THE FIELD OPENS AT ONCE FOR THE WHOLE SET, AND THE CLICK THAT OPENS IT IS STILL A CLICK: the tool is taken and a
/// corner is pointed at in the same gesture. Making that click only open the field would leave "click a corner, type
/// the value, press Enter" - the whole of the tool - naming nothing at all.
#[test]
fn the_click_that_opens_the_field_is_acted_upon_as_a_click() {
    let (p, si, l) = square();
    let mut corner = CornerInput::default();
    let mut sel = SketchSelection::default();
    let act = qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), false);
    assert!(corner.at.is_some(), "the first click did not open the field");
    assert_eq!(act, CornerClick::LineChosen(l[0]), "the click that opened the field was not acted upon as a click");
    assert_eq!(chosen(&sel), vec![l[0]], "the click that opened the field chose nothing");
}

/// A CLICK ON NOTHING WITH NO FIELD STANDING OPENS IT AND SAYS WHAT THE TOOL WAITS FOR.
#[test]
fn a_click_on_nothing_with_no_field_opens_it_and_says_what_it_waits_for() {
    let (p, si, _l) = square();
    let mut corner = CornerInput::default();
    let mut sel = SketchSelection::default();
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Nothing, false), CornerClick::Opened);
    assert!(corner.at.is_some(), "the click that named nothing left the tool without a field");
    assert!(!corner.shifted, "a click without Shift left the mode that makes a set");
}

/// A THIRD LINE AT A STRAIGHT JOINT IS NOT TAKEN, AND THE POINT IT WOULD HAVE STOOD AT IS NAMED.
#[test]
fn a_third_line_at_a_straight_joint_is_refused_with_the_point_named() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let mut ids = Vec::new();
    for (a, b) in [((0.0, 0.0), (20.0, 0.0)), ((20.0, 0.0), (40.0, 0.0)), ((20.0, 0.0), (20.0, 20.0))] {
        ids.push(p.add_line_entity(si, a.0, a.1, b.0, b.1, Purpose::Real));
    }
    p.regen_sketch(si);
    let (mut corner, mut sel) = tool();
    for &e in &ids[..2] {
        qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(e), true);
    }
    assert_eq!(corner.set.standing().len(), 0, "setup: two lines along one straight line make no corner: there is no angle there");
    // THE POINT BOTH MEET AT — found by their sharing one, not by the corner they make: they make none, for that is
    // the whole of what a straight joint is.
    let at = p.shared_vertex(si, ids[0], ids[1]).expect("the two lines meet at (20, 0)");
    assert_eq!(p.corner_of_pair(si, ids[0], ids[1]), None, "setup: two lines along one straight line do make a corner here");
    // THE THIRD ONE IS NOT TAKEN, and the point it would have stood at is named.
    let act = qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(ids[2]), true);
    assert_eq!(act, CornerClick::StraightJoint(at), "a third line at a straight joint was taken, and nothing was said");
    assert_eq!(chosen(&sel), vec![ids[0], ids[1]], "the refused line was chosen anyway");
}

/// THE SECOND CURVE OF A PAIR IS TAKEN WITHOUT SHIFT: a line chosen alone makes no corner, and a click on a line it meets
/// or crosses makes the pair - a click on each of two curves names their corner, as Shift does. A line it neither meets
/// nor crosses is a single selection, and so is a third click once the pair stands.
#[test]
fn the_second_curve_of_a_pair_is_taken_without_shift() {
    let (p, si, l) = square();
    let (mut corner, mut sel) = tool();
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), false);
    assert_eq!(qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[1]), false), CornerClick::LineChosen(l[1]));
    assert_eq!((chosen(&sel), corner.set.standing().len()), (vec![l[0], l[1]], 1), "a click on the line meeting the one chosen did not make the pair: {:?}", corner.set);
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[2]), false);
    assert_eq!(chosen(&sel), vec![l[2]], "a third click without Shift grew the pair");
    qymcad_ui_state::corner_click(&p, si, &mut corner, &mut sel, CornerOver::Line(l[0]), false);
    assert_eq!(chosen(&sel), vec![l[0]], "a click on a line that neither meets nor crosses the one chosen made a pair of them");
}
