//! EDITING WHAT IS ALREADY DRAWN: trimming, extending, breaking, rounding and cutting corners, offsetting,
//! mirroring, deleting, moving, copying, turning and laying out in patterns.
//!
//! Each check draws what it needs with the auto constraints turned off, so that the geometry stays where it was put
//! and what the tool did to it is the only change. What came of it is read off the sheet: how many lines and arcs
//! there are, where their ends stand, and how far the drawing reaches.
use qymcad::{Key, Kind, Modifiers, PointerButton, Session, Widget};
use qymcad_acceptance::build::{draw, empty_sketch, line, pick};
use qymcad_acceptance::probe;

/// Take the tool whose hint is `hint` and answer what the program said about it.
fn take(s: &mut Session, hint: &str) -> String {
    let hint = s.word(hint);
    s.press_hint(&hint);
    s.status()
}

/// Type `text` into a field of the bar found by sight rather than by a caption: the fields of a pattern stand in a
/// row and only the first of them is named.
fn fill_widget(s: &mut Session, w: &Widget, text: &str) {
    s.click(w.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(text);
}

/// The fields of the bar of options, left to right.
fn bar_fields(s: &mut Session) -> Vec<Widget> {
    let mut fields: Vec<Widget> = s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && w.rect.top() < 120.0).collect();
    fields.sort_by(|a, b| a.rect.left().total_cmp(&b.rect.left()));
    fields
}

/// THE FIELDS STANDING ON THE SHEET - the little boxes a gesture puts up - told apart from the fields of the bar and of
/// the panels by where they stand: on the sheet, not beside it. Told apart by height, a box put up beside a corner near
/// the top of the sheet was taken for a field of the bar.
fn sheet_fields(s: &mut Session) -> Vec<Widget> {
    let sheet = s.canvas();
    s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && sheet.contains(w.rect.center())).collect()
}

/// THE LITTLE BOX THAT POPPED UP BY THE GESTURE: the field on the sheet nearest to `at` - the radius at a corner, the
/// angle at the centre of a turn.
fn field_near(s: &mut Session, at: qymcad::Pos2) -> Widget {
    let fields: Vec<Widget> = sheet_fields(s);
    fields
        .iter()
        .min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at)))
        .cloned()
        .unwrap_or_else(|| panic!("no little box popped up near {at:?}; the fields on screen are {fields:?}"))
}

/// How far the drawing reaches: the corners of the box around it.
fn box_of(s: &mut Session) -> ([f64; 2], [f64; 2]) {
    let sk = s.document().sketches[0].clone();
    (sk.min, sk.max)
}

/// Whether the box around the drawing runs from `min` to `max`, to a thousandth of a millimetre.
fn box_is(s: &mut Session, min: [f64; 2], max: [f64; 2]) -> bool {
    let (a, b) = box_of(s);
    a.iter().zip(min).all(|(x, y)| (x - y).abs() < 1e-3) && b.iter().zip(max).all(|(x, y)| (x - y).abs() < 1e-3)
}

/// A PICK WITH SHIFT: it does not take the corner away and make a new one, it joins the one in hand.
fn pick_shift(s: &mut Session, x: f64, y: f64) {
    let at = s.on_sketch(x, y);
    s.click_with(at, PointerButton::Primary, Modifiers::SHIFT);
}

/// FOUR LINES STANDING ON ONE POINT, each a spoke of it, and the place to click each of them: the middle of the
/// spoke. They are drawn a long way out, because the field of a corner stands where the corner was picked, and a pick
/// with Shift that lands on its buttons answers the corner instead of joining the set.
fn cross_of_four_lines(s: &mut Session) -> [(f64, f64); 4] {
    [((0.0, 0.0), (3000.0, 0.0)), ((0.0, 0.0), (0.0, 3000.0)), ((0.0, 0.0), (-3000.0, 0.0)), ((0.0, 0.0), (0.0, -3000.0))]
        .into_iter()
        .map(|(a, b)| {
            line(s, a, b);
            ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| unreachable!("four spokes make four places to click them"))
}

/// How many lines, arcs and circles the sketch holds.
fn counts(s: &mut Session) -> (usize, usize, usize) {
    let sk = s.document().sketches[0].clone();
    (sk.lines, sk.arcs, sk.circles)
}

/// Whether any end of the geometry stands at `p`.
fn stands_at(s: &mut Session, p: (f64, f64)) -> bool {
    s.document().sketches[0].places.iter().any(|q| (q[0] - p.0).hypot(q[1] - p.1) < 1e-6)
}

probe! {
    /// TRIM BY A CLICK: the piece of a line beyond the crossing goes, and the rest of the drawing stays.
    fn trim_by_a_click_takes_the_piece_beyond_the_crossing() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (40.0, 0.0));
        line(&mut s, (20.0, -10.0), (20.0, 10.0));
        take(&mut s, "tb-trim-hint");
        s.click_on_sketch(35.0, 0.0);
        let (min, max) = box_of(&mut s);
        assert!((max[0] - 20.0).abs() < 1e-6, "the piece beyond the crossing is still there: the drawing reaches {}", max[0]);
        assert!((min[0]).abs() < 1e-3 && (min[1] + 10.0).abs() < 1e-3, "trimming moved the rest of the drawing: it starts at {min:?}");
        assert!(counts(&mut s).0 == 2, "trimming took a whole line away: {:?}", counts(&mut s));
    }
}

probe! {
    /// TRIM BY A DRAG: everything the cursor passes through is trimmed, in one gesture.
    fn trim_by_a_drag_takes_everything_it_passes_through() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        for x in [10.0, 30.0, 50.0] {
            line(&mut s, (x, -10.0), (x, 10.0));
        }
        take(&mut s, "tb-trim-hint");
        s.drag_on_sketch((5.0, -5.0), (55.0, -5.0));
        let (min, _) = box_of(&mut s);
        assert!((min[1]).abs() < 1e-6, "the pieces the drag passed through are still there: the drawing reaches down to {}", min[1]);
    }
}

probe! {
    /// MOVING THE SHEET WITH THE MIDDLE BUTTON CUTS NOTHING, the trim in hand: only the left button trims by a drag.
    /// Reported behaviour: the sheet moved with the middle button to bring a point into view cut the line in two.
    fn moving_the_sheet_with_the_trim_in_hand_cuts_nothing() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        for x in [10.0, 30.0, 50.0] {
            line(&mut s, (x, -10.0), (x, 10.0));
        }
        take(&mut s, "tb-trim-hint");
        // grabbed ON the upright line at 30, as the sheet is grabbed wherever the pointer happens to stand: the sheet
        // follows the pointer, so the point grabbed stays under it all the way
        let a = s.on_sketch(30.0, -5.0);
        let b = s.seen_on_sketch(40.0, -5.0).unwrap_or_else(|| panic!("the end of the move is out of view"));
        s.drag(a, b, qymcad::PointerButton::Middle, qymcad::Modifiers::NONE);
        s.key(qymcad::Key::Escape); // the trim put down, so that what lies at a point is read, not trimmed
        let under = s.sketch_under(30.0, -8.0);
        assert!(matches!(under, Some(qymcad::SketchPick::Line { .. })), "moving the sheet with the middle button cut the upright line it was grabbed on: under (30, -8) lies {under:?}");
    }
}

probe! {
    /// EXTEND: the line taken by a click, and a click past its end stretches it to the line it stops short of.
    fn extend_stretches_a_line_to_what_it_stops_short_of() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (30.0, -10.0), (30.0, 10.0));
        take(&mut s, "tb-extend-hint");
        s.click_on_sketch(18.0, 0.0);
        s.click_on_sketch(25.0, 3.0);
        assert!(stands_at(&mut s, (30.0, 0.0)), "the line was not stretched to the crossing at (30, 0): the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// BREAK: one line becomes two, and the drawing keeps its shape.
    fn break_splits_a_line_in_two() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (40.0, 0.0));
        let before = box_of(&mut s);
        take(&mut s, "tb-break-hint");
        s.click_on_sketch(20.0, 0.0);
        assert!(counts(&mut s).0 == 2, "the line was not split in two: {:?}", counts(&mut s));
        assert!(box_of(&mut s) == before, "breaking the line moved it: {:?} became {:?}", before, box_of(&mut s));
    }
}

probe! {
    /// FILLET A CORNER: the corner of two lines becomes an arc of the radius that was typed.
    fn fillet_rounds_a_corner_with_the_radius_typed() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(30.0, 0.0), (0.0, 0.0), (0.0, 30.0)]);
        take(&mut s, "tb-fillet-sketch-hint");
        let corner = s.on_sketch(0.0, 0.0);
        s.click(corner);
        let field = field_near(&mut s, corner);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).1 == 1, "the corner did not become an arc: {:?}", counts(&mut s));
        // the corner is cut back by the radius: the lines now start 5 away from where they met
        assert!(stands_at(&mut s, (5.0, 0.0)) && stands_at(&mut s, (0.0, 5.0)), "the arc of radius 5 does not meet the lines at (5, 0) and (0, 5): the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// CHAMFER A CORNER: the corner of two lines becomes a third line across it, as long as the size typed - a cut of 5
    /// on a square corner stands 5 / sqrt(2) along each line.
    fn chamfer_cuts_a_corner_with_the_size_typed() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(30.0, 0.0), (0.0, 0.0), (0.0, 30.0)]);
        take(&mut s, "tb-chamfer-sketch-hint");
        let corner = s.on_sketch(0.0, 0.0);
        s.click(corner);
        let field = field_near(&mut s, corner);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 3, "the corner did not become a line across: {:?}", counts(&mut s));
        let leg = 5.0 / std::f64::consts::SQRT_2;
        assert!(stands_at(&mut s, (leg, 0.0)) && stands_at(&mut s, (0.0, leg)), "the cut of 5 does not meet the lines at {leg:.4} from the corner: the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// A CHAMFER POINTED AT BY ITS TWO LINES: the cursor finds the corner without finding the point. Reported: the
    /// corner of a long edge could only be taken by hitting the vertex itself within a few pixels.
    fn chamfer_of_two_lines_that_meet_is_taken_by_them_alone() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(60.0, 0.0), (0.0, 0.0), (0.0, 60.0)]);
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // the middle of the first line: not a point of the drawing at all
        pick_shift(&mut s, 0.0, 30.0); // and the middle of the second: the corner is where the two meet
        let second = s.on_sketch(0.0, 30.0);
        let field = field_near(&mut s, second);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 3, "the two lines did not cut the corner between them: {:?}", counts(&mut s));
        let leg = 5.0 / std::f64::consts::SQRT_2; // a cut of 5 on a square corner
        assert!(stands_at(&mut s, (leg, 0.0)) && stands_at(&mut s, (0.0, leg)), "the cut of 5 does not meet the lines at {leg:.4} from the corner: the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// LEAVING THE CORNER MODE LETS GO OF WHAT IT HAD CHOSEN. Reported: the lines it was offered went on standing lit
    /// with nothing in hand, and the next tool found a selection that was not its own.
    fn leaving_the_corner_mode_clears_what_it_had_chosen() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(60.0, 0.0), (0.0, 0.0), (0.0, 60.0)]);
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // the first line of the corner
        pick_shift(&mut s, 0.0, 30.0); // and the second: the field of their corner opens
        let corner_at = s.on_sketch(0.0, 30.0);
        assert_eq!(field_near(&mut s, corner_at).kind, Kind::TextField, "the corner of the two chosen lines did not open its field");
        take(&mut s, "tb-line-hint"); // another tool: leaving the chamfer takes its picks with it
        take(&mut s, "tb-chamfer-sketch-hint"); // and the chamfer waits for a corner rather than offering an old one
        let left: Vec<Widget> = sheet_fields(&mut s);
        assert!(left.is_empty(), "the mode was taken again and stood with the field of the two lines the mode that ended had let go of: {left:?}");
    }
}

probe! {
    /// THE CHAMFER TAKEN WITH THE TWO LINES ALREADY CHOSEN: the corner is where they meet, and the field opens at the
    /// first click without asking for the lines again. Reported: the selection stood lit and the tool ignored it,
    /// asking for the corner again.
    fn a_chamfer_of_two_lines_already_chosen_is_offered_the_corner_they_share() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(60.0, 0.0), (0.0, 0.0), (0.0, 60.0)]);
        pick(&mut s, 30.0, 0.0, false);
        pick(&mut s, 0.0, 30.0, true); // the two lines of the corner, chosen before the tool
        take(&mut s, "tb-chamfer-sketch-hint");
        let empty = s.on_sketch(200.0, -200.0);
        s.click(empty); // a click on nothing opens the field and leaves the two lines as the set
        let field = field_near(&mut s, empty);
        assert_eq!(field.kind, Kind::TextField, "the chamfer of the two lines chosen before it opened no field: {}", s.status());
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 3, "the corner of the two chosen lines did not become a line across: {:?}", counts(&mut s));
        let leg = 5.0 / std::f64::consts::SQRT_2; // a cut of 5 on a square corner
        assert!(stands_at(&mut s, (leg, 0.0)) && stands_at(&mut s, (0.0, leg)), "the cut of 5 does not meet the lines at {leg:.4} from the corner: the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// TWO LINES THAT SHARE NO CORNER: nothing is offered, and the tool waits for a corner of its own.
    fn two_lines_that_meet_nowhere_leave_the_selection_alone_to_wait() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (30.0, -30.0), (30.0, 30.0)); // it crosses the first, but shares no point with it
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 30.0, 15.0, true);
        take(&mut s, "tb-chamfer-sketch-hint");
        let boxes: Vec<_> = sheet_fields(&mut s);
        assert!(boxes.is_empty(), "two lines with no corner in common opened the box anyway: {boxes:?}");
        assert!(counts(&mut s).0 == 2, "nothing was cut: {:?}", counts(&mut s));
    }
}

probe! {
    /// TWO LINES THAT SHARE NO CORNER: the first is let go, the second stands as the first of the next pair, and
    /// the search goes on with its neighbour.
    fn a_line_without_a_corner_becomes_the_first_of_the_next_one() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-line-hint", &[(60.0, 0.0), (0.0, 0.0), (0.0, 60.0)]);
        draw(&mut s, "tb-line-hint", &[(150.0, 0.0), (120.0, 0.0), (120.0, 30.0)]); // a second angle, far away
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // the first arm of the near angle
        pick_shift(&mut s, 135.0, 0.0); // an arm of the far angle: no corner in common with the first
        pick_shift(&mut s, 120.0, 15.0); // the other arm of the far angle, the neighbour of the one now chosen
        let third = s.on_sketch(120.0, 15.0);
        let field = field_near(&mut s, third);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).1 == 1, "the search did not carry on from the line clicked last: {:?}", counts(&mut s));
        assert!(stands_at(&mut s, (125.0, 0.0)) && stands_at(&mut s, (120.0, 5.0)), "the arc of radius 5 does not meet the far lines at (125, 0) and (120, 5): the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// TWO SQUARES SHARING A SINGLE POINT: the corner there is taken between the two lines that meet at it, and the
    /// other square keeps its sharp corner. Reported: nothing could be done at such a point at all, and nothing said.
    fn a_point_of_four_lines_is_a_corner_of_the_two_chosen() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (20.0, 20.0)]);
        draw(&mut s, "tb-rect-hint", &[(20.0, 20.0), (40.0, 40.0)]); // sharing the point (20, 20) and nothing else
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 20.0, 10.0); // the right side of the near square
        pick_shift(&mut s, 10.0, 20.0); // and its top side: the corner they meet at
        let second = s.on_sketch(10.0, 20.0);
        let field = field_near(&mut s, second);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 9, "the corner of the shared point was not cut: {:?}", counts(&mut s));
        let leg = 5.0 / std::f64::consts::SQRT_2; // a cut of 5 on a square corner
        assert!(stands_at(&mut s, (20.0, 20.0 - leg)) && stands_at(&mut s, (20.0 - leg, 20.0)), "the cut of 5 does not meet the sides of the near square at {leg:.4} from the corner: the ends stand at {:?}", s.document().sketches[0].places);
        assert!(!stands_at(&mut s, (25.0, 20.0)) && !stands_at(&mut s, (20.0, 25.0)), "the far square lost its corner as well: four lines through one point made two cuts of one corner");
    }
}

probe! {
    /// TWO HALVES OF ONE STRAIGHT LINE: they share a point and make an angle of 180 degrees, which is no corner. The
    /// search goes on, and the neighbour of the second half is the corner that is offered.
    fn a_straight_joint_is_not_a_corner_and_the_search_carries_on() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (20.0, 0.0), (40.0, 0.0)); // the same straight line in two pieces
        line(&mut s, (40.0, 0.0), (40.0, 30.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 10.0, 0.0); // the first half
        pick_shift(&mut s, 30.0, 0.0); // the second: 180 degrees is no corner, so it stands as the first of the next pair
        pick_shift(&mut s, 40.0, 15.0); // its neighbour: here there IS a corner
        let third = s.on_sketch(40.0, 15.0);
        let field = field_near(&mut s, third);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).1 == 1, "the joint of two halves of one straight line was rounded: {:?}", counts(&mut s));
        assert!(stands_at(&mut s, (35.0, 0.0)) && stands_at(&mut s, (40.0, 5.0)), "the arc of radius 5 does not meet the two lines at (35, 0) and (40, 5): the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// FILLET EVERY CORNER: all four corners of a rectangle become arcs at once.
    fn fillet_all_corners_rounds_the_whole_rectangle() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        take(&mut s, "tb-fillet-all-hint");
        s.click_on_sketch(20.0, 0.0); // the shape to round, as the bar asks
        let middle = s.canvas().center();
        let field = field_near(&mut s, middle);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert!(counts(&mut s).1 == 4, "the four corners did not all become arcs: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [0.0, 0.0], [40.0, 30.0]), "rounding the corners changed the size of the rectangle: {:?}", box_of(&mut s));
    }
}

probe! {
    /// OFFSET: the picked contour gets a copy of itself at the distance that was set.
    fn offset_makes_a_copy_of_the_contour_at_a_distance() {
        let mut s = empty_sketch();
        draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        take(&mut s, "tb-offset-hint"); // taken first: the distance stands in its bar
        let distance = s.word("opt-distance");
        s.fill(&distance, "5");
        for (x, y) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)] {
            pick(&mut s, x, y, (x, y) != (20.0, 0.0));
        }
        // the copy goes to the side the pointer is on - outside - and Enter makes it
        let outside = s.on_sketch(50.0, 15.0);
        s.move_to(outside);
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 8, "the contour was not copied: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [-5.0, -5.0], [45.0, 35.0]), "the copy does not stand 5 outside the contour: {:?}", box_of(&mut s));
    }
}

probe! {
    /// MIRROR: what is picked is reflected about the axis pointed at.
    fn mirror_reflects_what_is_picked_about_an_axis() {
        let mut s = empty_sketch();
        line(&mut s, (10.0, 0.0), (30.0, 20.0));
        line(&mut s, (0.0, -20.0), (0.0, 20.0)); // the axis to reflect about
        pick(&mut s, 20.0, 10.0, false);
        take(&mut s, "tb-mirror-sketch-hint");
        pick(&mut s, 0.0, 10.0, false);
        assert!(counts(&mut s).0 == 3, "the picked line was not reflected: {:?}", counts(&mut s));
        let (min, max) = box_of(&mut s);
        assert!((min[0] + 30.0).abs() < 1e-6 && (max[0] - 30.0).abs() < 1e-6, "the reflection does not stand opposite the original: the drawing runs from {} to {} across", min[0], max[0]);
    }
}

probe! {
    /// DELETE: what is picked goes, and the rest stays.
    fn delete_takes_away_what_is_picked() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (0.0, 10.0), (20.0, 10.0));
        pick(&mut s, 10.0, 10.0, false);
        take(&mut s, "tb-delete-hint");
        assert!(counts(&mut s).0 == 1, "the picked line was not taken away: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [0.0, 0.0], [20.0, 0.0]), "the line that was not picked went with it: the drawing runs {:?}", box_of(&mut s));
    }
}

probe! {
    /// MOVE: what is picked goes from the base point to the target, and nothing is added.
    fn move_carries_what_is_picked_from_one_point_to_another() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        pick(&mut s, 10.0, 0.0, false);
        take(&mut s, "tb-move-hint");
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(10.0, 10.0);
        assert!(counts(&mut s).0 == 1, "moving added geometry: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [10.0, 10.0], [30.0, 10.0]), "the line did not move by (10, 10): it runs {:?}", box_of(&mut s));
    }
}

probe! {
    /// COPY: the original stays where it was and a copy stands at the target.
    fn copy_leaves_the_original_and_puts_a_copy_at_the_target() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        pick(&mut s, 10.0, 0.0, false);
        take(&mut s, "tb-copy-hint");
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(10.0, 10.0);
        assert!(counts(&mut s).0 == 2, "there is no copy: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [0.0, 0.0], [30.0, 10.0]), "the original and its copy do not stand 10 by 10 apart: {:?}", box_of(&mut s));
    }
}

probe! {
    /// ROTATE: what is picked turns about the centre by the angle that was typed.
    fn rotate_turns_what_is_picked_by_the_angle_typed() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        pick(&mut s, 10.0, 0.0, false);
        take(&mut s, "tb-rotate-hint");
        s.click_on_sketch(0.0, 0.0);
        let angle = s.word("sk-angle-placeholder");
        s.fill_hinted(&angle, "90").key(Key::Enter);
        assert!(counts(&mut s).0 == 1, "turning the line added geometry: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [0.0, 0.0], [0.0, 20.0]), "the line did not turn a quarter about the origin: it runs {:?}", box_of(&mut s));
    }
}

probe! {
    /// A LINEAR PATTERN: the picked line is laid out three times along X, ten apart, and the pattern can be opened
    /// again by a double click on one of the copies.
    fn a_linear_pattern_lays_out_copies_along_a_direction() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (5.0, 0.0));
        pick(&mut s, 2.5, 0.0, false);
        take(&mut s, "tb-lin-array-hint");
        let count = s.word("opt-count");
        s.fill(&count, "3");
        let fields = bar_fields(&mut s);
        fill_widget(&mut s, &fields[1], "10");
        fill_widget(&mut s, &fields[2], "0");
        s.key(Key::Enter).key(Key::Enter); // the first Enter only leaves the field - see the check below
        assert!(counts(&mut s).0 == 3, "three copies were asked for: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [0.0, 0.0], [25.0, 0.0]), "the copies do not stand 10 apart along X: {:?}", box_of(&mut s));
        // opened again by a double click on a copy: the bar comes back with the count that was set
        let at = s.on_sketch(22.5, 0.0);
        s.double_click(at);
        let again = bar_fields(&mut s);
        assert!(again.first().map(|w| w.value.clone()).unwrap_or_default() == "3", "the pattern did not open again with its own count: the bar shows {:?}", again.iter().map(|w| w.value.clone()).collect::<Vec<_>>());
    }
}

probe! {
    /// A CIRCULAR PATTERN: the picked line is laid out four times about the origin, a quarter turn apart.
    fn a_circular_pattern_lays_out_copies_about_a_centre() {
        let mut s = empty_sketch();
        line(&mut s, (10.0, 0.0), (20.0, 0.0));
        pick(&mut s, 15.0, 0.0, false);
        take(&mut s, "tb-circ-array-hint");
        let count = s.word("opt-count");
        s.fill(&count, "4");
        let fields = bar_fields(&mut s);
        fill_widget(&mut s, &fields[1], "360");
        s.click_on_sketch(0.0, 0.0); // the centre to turn about
        s.key(Key::Enter).key(Key::Enter);
        assert!(counts(&mut s).0 == 4, "four copies were asked for: {:?}", counts(&mut s));
        assert!(box_is(&mut s, [-20.0, -20.0], [20.0, 20.0]), "the copies do not stand a quarter turn apart about the origin: {:?}", box_of(&mut s));
    }
}

probe! {
    /// THE ENTER THE BAR ASKS FOR APPLIES AT ONCE: "set the step and count above, press Enter" - one press, not two.
    fn the_enter_the_bar_asks_for_applies_at_once() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (5.0, 0.0));
        pick(&mut s, 2.5, 0.0, false);
        take(&mut s, "tb-lin-array-hint");
        let count = s.word("opt-count");
        s.fill(&count, "3");
        let fields = bar_fields(&mut s);
        fill_widget(&mut s, &fields[1], "10");
        s.key(Key::Enter);
        assert!(counts(&mut s).0 == 3, "one Enter after the numbers did not make the pattern: {:?} instead of 3 lines; the status line says {:?}", counts(&mut s), s.status());
    }
}

probe! {
    /// A TRIM AFTER A NEW PROJECT IN THE SAME WINDOW takes the piece clicked, as it does in a window just opened: a
    /// rectangle drawn, File -> New project, then a circle of 10 with a level line through it, and the lower half of
    /// the circle trimmed - one line and one arc left.
    fn a_trim_after_a_new_project_takes_the_piece_clicked() {
        let mut s = qymcad::Session::start();
        qymcad_acceptance::build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let (file, new) = (s.word("menu-file"), s.word("file-new"));
        s.menu(&[&file, &new]);
        let dont = s.word("nav-dont-save");
        if let Some(b) = s.find(&dont, qymcad::pos2(0.0, 0.0)) {
            s.click(b.center());
        }
        qymcad_acceptance::build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        qymcad_acceptance::build::circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        line(&mut s, (-20.0, 0.0), (20.0, 0.0));
        take(&mut s, "tb-trim-hint");
        s.click_on_sketch(0.0, -10.0);
        let sk = s.document().sketches[0].clone();
        assert!((sk.lines, sk.arcs, sk.circles) == (1, 1, 0), "the trim left {} lines, {} arcs and {} circles, not one line and one arc; the status line says {:?}", sk.lines, sk.arcs, sk.circles, s.status());
    }
}

probe! {
    /// A CONTOUR OF CHOSEN LINES IS CUT BY ONE ANSWER. The corner is not something one points at: it is what the
    /// chosen lines make, so three lines of a triangle with Shift are three corners, and one value and one Enter cut
    /// all of them. Reported: a shape whose corners were all wanted had to be rounded one corner at a time, each one
    /// its own value typed into a field that opened over a different corner every time.
    fn a_contour_of_chosen_lines_is_cut_by_one_answer() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a closed contour of three lines
        assert_eq!(counts(&mut s).0, 3, "setup: the triangle is not three lines: {:?}", counts(&mut s));
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // the middle of the first line
        pick_shift(&mut s, 45.0, 25.0); // and of the second: a corner stands
        pick_shift(&mut s, 15.0, 25.0); // and of the third, which closes the contour
        assert!(s.status().contains('3'), "the set was said to hold {:?} corners, not the three the contour makes", s.status());
        let at = s.on_sketch(30.0, 0.0);
        let field = field_near(&mut s, at);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert_eq!(counts(&mut s).0, 6, "the three corners of the triangle were not cut by one answer: {:?}", counts(&mut s));
    }
}

probe! {
    /// FOUR LINES ON ONE POINT ARE TAKEN TWO AT A TIME, IN THE ORDER THEY WERE CHOSEN: the third waits, and the
    /// fourth makes the second corner - with the third, not with the first. Reported: every pair of the four was a
    /// corner there, so naming a shape round a cross cut places nobody had asked for.
    fn the_lines_of_a_cross_are_paired_two_at_a_time_in_the_order_they_were_chosen() {
        let mut s = empty_sketch();
        let mid = cross_of_four_lines(&mut s);
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, mid[0].0, mid[0].1);
        assert!(!s.status().contains('1'), "one chosen line of the cross made a corner of itself: {:?}", s.status());
        pick_shift(&mut s, mid[1].0, mid[1].1);
        assert!(s.status().contains('1'), "the second chosen line did not make the first corner: {:?}", s.status());
        pick_shift(&mut s, mid[2].0, mid[2].1);
        assert!(s.status().contains('1'), "a third line on that point made a corner of its own: {:?}", s.status());
        pick_shift(&mut s, mid[3].0, mid[3].1);
        assert!(s.status().contains('2'), "the fourth line did not make the second corner, with the third: {:?}", s.status());
    }
}

probe! {
    /// A LINE CHOSEN WITH SHIFT THAT MEETS NOTHING OF THE SET WAITS THERE: it is half of the next corner, and the
    /// corners already standing do not go off the sheet. Reported: the preview of the first corner was taken down by a
    /// line a person had only meant to put in the set.
    fn a_line_that_meets_nothing_of_the_set_waits_and_keeps_the_corners_standing() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (40.0, 0.0));
        line(&mut s, (0.0, 0.0), (0.0, 40.0));
        line(&mut s, (130.0, 140.0), (200.0, 140.0)); // and a line that meets neither of them
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 20.0, 0.0); // the middle of the first line
        pick_shift(&mut s, 0.0, 20.0); // and of the second: a corner stands at their meeting
        assert!(s.status().contains('1'), "two lines that meet did not make a corner: {:?}", s.status());
        pick_shift(&mut s, 165.0, 140.0); // and a line that meets nothing: it waits
        let at = s.on_sketch(20.0, 0.0);
        assert_eq!(field_near(&mut s, at).kind, Kind::TextField, "the field of the corner went off the sheet when a line that meets nothing was chosen: {}", s.status());
        assert!(s.status().contains('1'), "a line that meets nothing of the set made a corner of its own: {:?}", s.status());
    }
}

probe! {
    /// A CLICK AT A POINT WITH SHIFT PUTS THE CORNER STANDING THERE AWAY, AND THE SAME CLICK BRINGS IT BACK. A corner
    /// was said by its lines, and a point naming it again would be a second reading of the same place - so the click
    /// puts it away rather than reading it: the lines keep their ends, and the same click brings that very corner
    /// back. Without Shift the click is a single selection, and what it leaves is that point alone.
    fn a_click_at_a_point_puts_the_corner_away_and_the_same_click_brings_it_back() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (40.0, 0.0));
        line(&mut s, (0.0, 0.0), (0.0, 40.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 20.0, 0.0);
        pick_shift(&mut s, 0.0, 20.0);
        assert!(s.status().contains('1'), "setup: two lines that meet did not make a corner: {:?}", s.status());
        let at = s.on_sketch(0.0, 0.0);
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // AT THE POINT: the corner is put away, not deleted
        assert!(!s.status().contains('1'), "the corner stood on after the click that put it away: {:?}", s.status());
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // AND THE SAME CLICK BRINGS IT BACK
        assert!(s.status().contains('1'), "the same click at the point did not bring the corner back: {:?}", s.status());
    }
}

probe! {
    /// A POINT THAT NAMED A CORNER OF ITS OWN IS TAKEN AWAY BY THE SAME CLICK WITH SHIFT. Reported: naming a point
    /// again could only add a second reading of the same place, so a corner could be named but never un-named once it
    /// was there. Without Shift the same click is a single selection: the set is that point alone.
    fn a_click_at_a_point_takes_away_the_corner_named_there() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // ONE CHOSEN LINE: no corner stands at either end of it
        let free = s.on_sketch(60.0, 0.0); // where this line meets the other
        s.click_with(free, PointerButton::Primary, Modifiers::SHIFT); // A CORNER NAMED AT THE POINT, of the two lines
        assert!(s.status().contains('1'), "setup: the corner named at the point is not in the set: {:?}", s.status());
        // THE SAME CLICK WITH SHIFT TAKES IT AWAY AGAIN: a second reading of the same place is not a second corner.
        s.click_with(free, PointerButton::Primary, Modifiers::SHIFT);
        assert!(!s.status().contains('1'), "a corner named at a point outlived the click that should take it away: {:?}", s.status());
    }
}

probe! {
    /// A CORNER OF THE LINES TAKES THE PLACE OF ONE NAMED AT A POINT, AND A CORNER NAMED AT A POINT DIES WITH ONE OF
    /// ITS LINES. Two corners at one point would be one place read twice, and a corner named at a point is a corner
    /// of the two lines that stand there - so letting one of them go takes it.
    fn a_corner_named_at_a_point_gives_way_to_the_lines_and_to_their_going() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // ONE CHOSEN LINE: no corner stands at either end of it
        let at = s.on_sketch(60.0, 0.0);
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // A CORNER IS NAMED AT THE POINT where this line meets the other
        let field = field_near(&mut s, at);
        assert_eq!(field.kind, Kind::TextField, "the corner named at the point did not stand in hand: {}", s.status());
        // AND NOW THE SECOND LINE IS CHOSEN: it arrives at the same point and takes the place of the named one.
        pick_shift(&mut s, 45.0, 25.0);
        assert!(!s.status().contains('2'), "a corner of the lines and a corner named at a point both stand at one place: {:?}", s.status());
    }
}

probe! {
    /// ANY LEFT CLICK WITHOUT SHIFT IS A SINGLE SELECTION, AND A MULTI-SELECTION IS MADE WITH SHIFT. Reported: after
    /// two lines had been chosen with Shift, a click without Shift on a line of its own cut the whole set with it, and
    /// there was no way to take one line out of a set without giving the set up altogether. So the plain click is the
    /// single one, and the set is begun and left with Shift held.
    fn a_click_without_shift_chooses_one_line_and_leaves_the_set() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a closed contour of three lines
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        assert!(s.status().contains('1'), "setup: two lines that meet did not make a corner: {:?}", s.status());
        // A CLICK WITHOUT SHIFT ON A THIRD LINE: only that line is chosen from here, and the set is that line's corners.
        s.click_on_sketch(15.0, 25.0);
        assert!(!s.status().contains('1'), "a click without Shift kept the corners the set had: {:?}", s.status());
        // THE TWO LINES OF ONE CORNER CHOSEN AGAIN, and the corner they make:
        s.click_on_sketch(30.0, 0.0); // a click without Shift: this line alone
        pick_shift(&mut s, 45.0, 25.0); // and with Shift the second joins it
        assert!(s.status().contains('1'), "setup: the two lines chosen make their corner again: {:?}", s.status());
        // AND A CLICK WITHOUT SHIFT ON EMPTY SPACE LEAVES THE LINES CHOSEN AND SPEAKS ABOUT THE MODE ALONE.
        let empty = s.on_sketch(200.0, -200.0);
        s.click(empty);
        assert!(s.status().contains('1'), "a click on nothing at all took the chosen lines away: {:?}", s.status());
    }
}

probe! {
    /// A THIRD CHOSEN LINE AT A STRAIGHT JOINT IS REFUSED, WITH THE POINT NAMED. Two lines along one straight line
    /// make no corner - there is nothing to cut there - and the corners at a point are taken two at a time, so the
    /// third would have to be cut against that joint. The line is not taken and the reason is said.
    fn a_third_line_at_a_straight_joint_is_refused_and_the_point_is_named() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0)); // two segments of one straight line
        line(&mut s, (20.0, 0.0), (40.0, 0.0));
        line(&mut s, (20.0, 0.0), (20.0, 20.0)); // and a third arriving at the point they share
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 10.0, 0.0);
        pick_shift(&mut s, 30.0, 0.0);
        assert!(!s.status().contains('1'), "two lines along one straight line made a corner: {:?}", s.status());
        // THE THIRD ONE: not taken, and the point named.
        pick_shift(&mut s, 20.0, 10.0);
        assert!(s.status().contains("20"), "the third line at the straight joint was taken without a word: {:?}", s.status());
    }
}

probe! {
    /// THE LINES CHOSEN BEFORE THE TOOL ARE THE SET THEY WOULD HAVE BEEN WITH SHIFT, AND THE FIELD OPENS FOR THE
    /// WHOLE SET AT THE FIRST CLICK. Reported: taking the chamfer with a contour selected dropped the selection, and
    /// the corners of that contour had to be named one by one again. The field is not up before something is pointed
    /// at - a box lying over the drawing takes the clicks meant for the drawing - but what it holds is the contour.
    fn the_lines_chosen_before_the_tool_are_cut_as_the_set() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (60.0, 40.0));
        line(&mut s, (60.0, 40.0), (0.0, 40.0));
        line(&mut s, (0.0, 40.0), (0.0, 0.0)); // a closed contour of four lines
        assert_eq!(counts(&mut s).0, 4, "setup: the square is not four lines: {:?}", counts(&mut s));
        pick(&mut s, 30.0, 0.0, false);
        pick(&mut s, 60.0, 20.0, true);
        pick(&mut s, 30.0, 40.0, true);
        pick(&mut s, 0.0, 20.0, true); // and the whole contour chosen, the last three with Shift
        take(&mut s, "tb-chamfer-sketch-hint");
        // THE FIELD IS NOT UP YET - nothing has been pointed at - AND NO FIELD LIES OVER THE CONTOUR, which is what
        // a box standing in the middle of the sheet does to the clicks meant for its own lines.
        let before_click: Vec<Widget> = sheet_fields(&mut s);
        assert!(before_click.is_empty(), "the chamfer stood with a field over the contour before anything was pointed at: {before_click:?}");
        let empty = s.on_sketch(200.0, -200.0);
        s.click(empty); // the first click opens the field and leaves the four chosen lines as the set
        let field = field_near(&mut s, empty);
        assert_eq!(field.kind, Kind::TextField, "the chamfer of a contour chosen before the tool opened no field: {}", s.status());
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert_eq!(counts(&mut s).0, 8, "the four corners of the chosen contour were not cut by one answer: {:?}", counts(&mut s));
    }
}

probe! {
    /// THE SAME SET IS CUT BY A CHAMFER AND BY A FILLET: what the chosen lines make is the set either way, and one
    /// answer cuts all of it. The two tools differ in what they leave behind - a straight cut or an arc - and in
    /// nothing else about the set.
    fn the_same_set_is_cut_by_the_chamfer_and_by_the_fillet() {
        // THE CHAMFER
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0));
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        let at = s.on_sketch(30.0, 0.0);
        let f = field_near(&mut s, at);
        fill_widget(&mut s, &f, "5");
        s.key(Key::Enter);
        let chamfered = counts(&mut s);
        assert_eq!(chamfered.0, 6, "the chamfer did not cut the three corners of the contour: {:?}", chamfered);
        assert_eq!(chamfered.1, 0, "a chamfer left an arc behind: {:?}", chamfered);

        // AND THE FILLET, ON THE SAME CONTOUR
        let mut t = empty_sketch();
        line(&mut t, (0.0, 0.0), (60.0, 0.0));
        line(&mut t, (60.0, 0.0), (30.0, 50.0));
        line(&mut t, (30.0, 50.0), (0.0, 0.0));
        take(&mut t, "tb-fillet-sketch-hint");
        pick_shift(&mut t, 30.0, 0.0);
        pick_shift(&mut t, 45.0, 25.0);
        pick_shift(&mut t, 15.0, 25.0);
        let at = t.on_sketch(30.0, 0.0);
        let f = field_near(&mut t, at);
        fill_widget(&mut t, &f, "5");
        t.key(Key::Enter);
        let filleted = counts(&mut t);
        // THREE ARCS BETWEEN THE SAME THREE LINES: the fillet takes the corner away without adding a line of its
        // own, where the chamfer lays a straight one across it - and that is the whole difference between the two.
        assert_eq!(filleted.0, 3, "the fillet did not cut the three corners of the contour: {:?}", filleted);
        assert_eq!(filleted.1, 3, "a fillet of three corners left not three arcs behind: {:?}", filleted);
    }
}

probe! {
    /// THE WHOLE SET IS ONE STEP OF UNDO. A person naming three corners of a shape meant one act, and undo taking
    /// back one corner of it would leave the drawing in a state nobody had asked for.
    fn the_whole_set_is_one_step_of_undo() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0));
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        let at = s.on_sketch(30.0, 0.0);
        let f = field_near(&mut s, at);
        fill_widget(&mut s, &f, "5");
        s.key(Key::Enter);
        assert_eq!(counts(&mut s).0, 6, "setup: the three corners of the contour were not cut: {:?}", counts(&mut s));
        let step = s.document().undo.last().cloned().expect("the step that cut the set");
        // THE EDIT MENU, named at the head of the menu bar: the left bar names its groups with the same word.
        let named = |line: String| line.replace("{ $what }", &step).replace("{$what}", &step);
        let bar = qymcad::pos2(50.0, 11.0);
        s.press_word_near(&s.word("menu-edit"), bar);
        s.press_word(&named(s.word("menu-undo-named")));
        assert_eq!(counts(&mut s).0, 3, "one undo of the whole set took back only part of it: {:?}", counts(&mut s));
    }
}

probe! {
    /// ESC CANCELS THE WHOLE SET, and nothing of it is cut: the lines keep the ends they have.
    fn esc_cancels_the_whole_set_and_nothing_is_cut() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0));
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        let at = s.on_sketch(30.0, 0.0);
        let f = field_near(&mut s, at);
        fill_widget(&mut s, &f, "5");
        s.key(Key::Escape);
        assert_eq!(counts(&mut s).0, 3, "Esc cut the set instead of cancelling it: {:?}", counts(&mut s));
    }
}

probe! {
    /// A CORNER PUT AWAY STAYS PUT AWAY, whatever else changes in the selection afterwards. Reported: a corner that a
    /// person had put away came back by itself the moment another line was chosen, and there was no way to keep it out.
    fn a_corner_put_away_stays_put_away() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a closed contour, three corners
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        assert!(s.status().contains('3'), "setup: the contour of three lines is not three corners: {:?}", s.status());
        let at = s.on_sketch(60.0, 0.0); // THE POINT where the first two lines meet, so the click is at a point
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // ONE CORNER PUT AWAY
        assert!(s.status().contains('2'), "the click at the point did not put one corner away: {:?}", s.status());
        // AND THE SELECTION IS READ AGAIN WITH THE LINES UNCHANGED: the corner put away is remembered, so reading
        // the set afresh does not make it a second time.
        pick_shift(&mut s, 15.0, 25.0); // the third line let go
        pick_shift(&mut s, 15.0, 25.0); // and chosen again
        assert!(!s.status().contains('3'), "a corner that was put away came back by itself: {:?}", s.status());
    }
}

probe! {
    /// A CLICK ON A POINT WITHOUT SHIFT IS A SINGLE SELECTION OF A POINT, THE SAME AS A CLICK ON A LINE IS OF A
    /// LINE, AND WHAT IT POINTS AT IS ALL THAT IS LEFT. Reported: a click that landed on a line threw the whole set
    /// away, and a click that landed on a point beside one kept it, so what a click did depended on a few pixels of
    /// aim and the two rules could not both be remembered.
    fn a_click_on_a_point_without_shift_leaves_that_point_alone() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a contour of three lines
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // the first line
        pick_shift(&mut s, 15.0, 25.0); // and the third: a corner stands where they meet at (0, 0)
        assert!(s.status().contains('1'), "setup: two lines that meet did not make a corner: {:?}", s.status());
        // THE POINT WHERE THE FIRST LINE MEETS THE SECOND, which is free: the second line is not in the set.
        let at = s.on_sketch(60.0, 0.0);
        s.click(at);
        assert!(s.status().contains('1'), "the click on the point left the set at another number of corners: {:?}", s.status());
        // AND ONE ENTER CUTS THAT ONE CORNER: the corner of the two chosen lines is not cut with it.
        let field = field_near(&mut s, at);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert_eq!(counts(&mut s).1, 1, "one answer did not cut the one corner that was left: {:?}", counts(&mut s));
        // AND THE CORNER OF THE TWO CHOSEN LINES IS STILL SHARP: the click on the point left that corner behind, and
        // the answer that cut the one that was left did not carry it along.
        assert!(stands_at(&mut s, (0.0, 0.0)), "the corner of the two chosen lines was cut by the same answer: the ends stand at {:?}", s.document().sketches[0].places);
    }
}

probe! {
    /// A POINT ALREADY IN THE SET, CLICKED AGAIN WITHOUT SHIFT, EMPTIES THE SET, AND THE BOX GOES WITH IT: it is the one
    /// thing to let go of, a set of nothing is not left standing as though it were an answer, and a box with nothing
    /// to cut in it is not drawn.
    fn a_chosen_point_clicked_again_without_shift_empties_the_set() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0); // one chosen line: no corner stands at either end of it
        let at = s.on_sketch(60.0, 0.0); // where this line meets the other
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // a corner named at that point
        assert!(s.status().contains('1'), "setup: the corner named at the point is not in the set: {:?}", s.status());
        s.click(at); // THE SAME POINT WITHOUT SHIFT: the whole set goes
        assert!(!s.status().contains('1'), "a set of nothing is standing as though it were an answer: {:?}", s.status());
        // AND THE BOX WENT WITH IT: a box with nothing to cut in it is not drawn, so there is nothing to type a
        // value into, and nothing that could be cut.
        let left: Vec<Widget> = sheet_fields(&mut s);
        assert!(left.is_empty(), "a box stood with nothing to cut in it: {left:?}");
        assert_eq!(counts(&mut s).0, 2, "an empty set was cut anyway: {:?}", counts(&mut s));
    }
}

probe! {
    /// THE FIELD IS NOT UP UNTIL SOMETHING IS POINTED AT. It used to stand in the middle of the canvas, which is
    /// where the drawing is: a Shift+click on a line that ran under the box was taken by the box, and a set could not
    /// be built out of a part whose lines pass through the middle of the sheet.
    fn the_field_of_the_corner_is_not_up_before_the_first_click() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        let up: Vec<Widget> = sheet_fields(&mut s);
        assert!(up.is_empty(), "the corner field stood over the drawing before anything was pointed at: {up:?}");
        // THE FIRST LINE IS HALF A CORNER: it is chosen, and there is still nothing to cut, so there is still no box.
        let half = s.on_sketch(30.0, 0.0);
        pick_shift(&mut s, 30.0, 0.0);
        let still: Vec<Widget> = sheet_fields(&mut s);
        assert!(still.is_empty(), "a box stood over the drawing with a half-corner in it: {still:?}");
        // AND THE SECOND LINE UNDER WHERE THE BOX WOULD HAVE STOOD IS STILL TAKEN: it did not come down on the sheet.
        pick_shift(&mut s, 45.0, 25.0);
        assert!(s.status().contains('1'), "the second Shift+click was taken by the box, not by the line: {:?}", s.status());
        assert_eq!(field_near(&mut s, half).kind, Kind::TextField, "the corner that is in the set opened no field to type its value in: {}", s.status());
    }
}

/// How many pixels of the screen are of the colour `want`, to a shade.
fn pixels_of(s: &mut Session, want: [u8; 3]) -> usize {
    let p = s.snapshot();
    (0..p.width * p.height)
        .filter(|&i| {
            let px = &p.rgba[i * 4..i * 4 + 3];
            (0..3).all(|k| (px[k] as i32 - want[k] as i32).abs() <= 6)
        })
        .count()
}

/// THE AMBER AND VIOLET OF THE SCHEME, which are what tells a corner in the set from a corner the cursor is only
/// pointing at.
const PREVIEW_NEW: [u8; 3] = [240, 200, 90];
const PREVIEW_FIXED: [u8; 3] = [157, 122, 235];

probe! {
    /// THE CORNERS IN THE SET ARE DRAWN, AND SO IS THE ONE THE CURSOR IS POINTING AT BUT HAS NOT NAMED - the first in
    /// the accent colour of the scheme, the second in amber. Reported: no preview was drawn at all, neither colour,
    /// whatever the radius was - the field had already taken the value out of the tool by the time the preview read
    /// it, so the read found nothing and there was nothing to draw.
    fn the_corners_of_the_set_are_drawn_and_the_one_under_the_cursor_too() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        assert!(s.status().contains('1'), "setup: two lines that meet did not make a corner: {:?}", s.status());
        let violet = pixels_of(&mut s, PREVIEW_FIXED);
        assert!(violet > 40, "the corner of the set is not drawn in the colour of the set: {violet} pixels of it");
        // THE CURSOR OVER A POINT THAT IS NOT IN THE SET: the amber preview of the corner a click there would name.
        let hover = s.on_sketch(60.0, 0.0);
        s.move_to(hover);
        let amber = pixels_of(&mut s, PREVIEW_NEW);
        assert!(amber > 40, "the corner under the cursor is not drawn in the colour of a new one: {amber} pixels of it");
    }
}

probe! {
    /// THE AMBER CORNER IS DRAWN BEFORE ANYTHING IS CHOSEN, with no field in sight. The preview belongs to the sheet
    /// and not to the box, and it used to be drawn from inside the box - so the one corner a person could not see
    /// before naming it was the first one, and seeing it first is the whole of what the amber is for.
    fn the_first_corner_is_shown_in_amber_before_anything_is_chosen() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        take(&mut s, "tb-fillet-sketch-hint");
        // NO FIELD IS UP: nothing has been pointed at, so there is nothing to type a value into.
        let up: Vec<Widget> = sheet_fields(&mut s);
        assert!(up.is_empty(), "setup: a field stood before anything was pointed at: {up:?}");
        let before = pixels_of(&mut s, PREVIEW_NEW);
        let at = s.on_sketch(60.0, 0.0);
        s.move_to(at);
        let after = pixels_of(&mut s, PREVIEW_NEW);
        assert!(after > before + 40, "the corner under the cursor is not drawn before anything is chosen: {before} pixels of amber before, {after} with the cursor on it");
    }
}

probe! {
    /// A CLICK ON EMPTY SHEET CHANGES NOTHING AND SAYS NOTHING. It used to answer the click with a complaint that
    /// there was no line or point under it, which is the one thing a person who clicked empty sheet knows, written
    /// over the status line of a tool that was working.
    fn a_click_on_empty_sheet_is_not_answered() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        take(&mut s, "tb-fillet-sketch-hint");
        let first = s.on_sketch(30.0, 0.0);
        s.click_with(first, PointerButton::Primary, Modifiers::SHIFT); // the field opens and the line is chosen
        let said = s.status();
        let empty = s.on_sketch(200.0, -200.0);
        s.click(empty);
        assert_eq!(s.status(), said, "a click on empty sheet was answered: it said {:?}", s.status());
        // AND THE SET IS AS IT WAS: a click that named nothing is not a command to forget the drawing.
        let again = s.on_sketch(30.0, 0.0);
        s.click_with(again, PointerButton::Primary, Modifiers::SHIFT);
        assert_eq!(s.status(), said, "the line chosen before it did not answer after the click on empty sheet: {:?}", s.status());
    }
}

probe! {
    /// NO CORNER IN THE SET, NO BOX. The box is the value for the set, and where there is no corner there is nothing
    /// the value is for: a box standing over the drawing with a number in it invites a number to be typed for a set
    /// that does not exist. So it goes the moment the set empties, and it comes with the corner that fills it.
    fn the_box_is_not_standing_while_the_set_is_empty() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a contour of three lines
        take(&mut s, "tb-fillet-sketch-hint");
        let boxes = |s: &mut Session| -> Vec<Widget> { sheet_fields(s) };
        // THE FIRST LINE: HALF A CORNER, AND NOTHING TO CUT, SO NOTHING TO TYPE INTO.
        pick_shift(&mut s, 30.0, 0.0);
        assert!(boxes(&mut s).is_empty(), "a box stood over the drawing for one chosen line: {:?}", boxes(&mut s));
        // THE SECOND LINE MAKES THE CORNER, AND WITH IT THE BOX.
        pick_shift(&mut s, 45.0, 25.0);
        assert_eq!(boxes(&mut s).len(), 1, "the corner in the set opened no box to type its value in: {}", s.status());
        // AND THE THIRD, WHICH MAKES TWO MORE CORNERS, LEAVES THE ONE BOX: one value cuts them all.
        pick_shift(&mut s, 15.0, 25.0);
        assert!(s.status().contains('3'), "setup: the contour of three lines is not three corners: {:?}", s.status());
        assert_eq!(boxes(&mut s).len(), 1, "each corner of the set brought a box of its own: {:?}", boxes(&mut s));
    }
}

probe! {
    /// NOTHING OF A CORNER IS DRAWN WITH ANOTHER TOOL IN HAND. The amber corner is what a click would name, and it
    /// belongs to the fillet and to the chamfer alone: the drawing tool rests its cursor over the corner of every
    /// line it passes, and rounding that corner there belongs to nobody.
    ///
    /// THE TWO ARE COUNTED IN ONE SESSION on the same pixel, so the difference is the corner and not the panel it
    /// is counted against - the amber of the button is on the sheet whatever tool is in hand.
    fn no_corner_is_drawn_while_another_tool_is_in_hand() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        let at = s.on_sketch(60.0, 0.0); // the corner the two lines make
        take(&mut s, "tb-fillet-sketch-hint");
        s.move_to(at);
        let rounding = pixels_of(&mut s, PREVIEW_NEW);
        take(&mut s, "tb-line-hint"); // the drawing tool, whose cursor passes over that corner
        s.move_to(at);
        let drawing = pixels_of(&mut s, PREVIEW_NEW);
        assert!(rounding > drawing + 40, "the corner under the cursor is not drawn while the fillet is in hand: {rounding} pixels of amber against {drawing} with the line tool in hand");
    }
}

probe! {
    /// THE VALUE IS TYPED STRAIGHT AWAY, with no click on the field first. Naming three corners is one act and the
    /// answer that cuts them is the next one: egui gives the caret up to a click anywhere else, so the field lost it
    /// with the second Shift click, and a person had to go and find the box before typing - the one answer that cuts
    /// them all was the one act that could not be done in a row.
    fn the_value_of_the_set_is_typed_without_a_click_on_the_field() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0));
        take(&mut s, "tb-chamfer-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        s.type_text("5"); // into whatever holds the keyboard, which is what a person types into
        s.key(qymcad::Key::Enter);
        assert_eq!(counts(&mut s).0, 6, "the value typed after naming the set cut nothing: {:?} ({})", counts(&mut s), s.status());
    }
}

probe! {
    /// A CORNER PUT AWAY GOES WITH THE LINE IT WAS MADE OF. What a click at a point puts away is a corner OF CHOSEN
    /// LINES, and a line a person lets go of is one they do not want: the corner at its end is gone with it, put
    /// away or not. Choosing the line again makes the corner again, and this time it stands.
    fn a_corner_put_away_goes_when_a_line_of_it_is_let_go() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a contour, three corners
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        let at = s.on_sketch(60.0, 0.0); // where the first two lines meet
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT); // that corner put away
        assert!(s.status().contains('2'), "setup: the corner was not put away: {:?}", s.status());
        pick_shift(&mut s, 30.0, 0.0); // the first line let go
        assert!(s.status().contains('1'), "the line let go left the corner that was put away standing: {:?}", s.status());
        pick_shift(&mut s, 30.0, 0.0); // and chosen again
        assert!(s.status().contains('3'), "the corner of the line chosen again did not come back: {:?}", s.status());
        // AND IT STANDS RATHER THAN BEING PUT AWAY A SECOND TIME: three corners in the set is three to be cut.
        let field = field_near(&mut s, at);
        fill_widget(&mut s, &field, "5");
        s.key(Key::Enter);
        assert_eq!(counts(&mut s).1, 3, "the corner that came back was not cut with the other two: {:?} ({})", counts(&mut s), s.status());
    }
}

probe! {
    /// ESC PUTS THE WHOLE CORNER TOOL DOWN: the tool goes back to the arrow, nothing of the drawing stands lit, and
    /// the set of corners is no longer drawn. It used to take the tool down alone, and the lines stayed lit with
    /// their corners drawn on them - a set that was on the screen and in no tool, and the next click landed in it.
    fn esc_from_the_corner_tool_takes_the_whole_set_down() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (60.0, 0.0));
        line(&mut s, (60.0, 0.0), (30.0, 50.0));
        line(&mut s, (30.0, 50.0), (0.0, 0.0)); // a contour, three corners
        let lit = |s: &mut Session| s.lit_in_sketch();
        let tool = |s: &mut Session| s.editing_tool();
        let violet = |s: &mut Session| pixels_of(s, PREVIEW_FIXED);
        take(&mut s, "tb-fillet-sketch-hint");
        pick_shift(&mut s, 30.0, 0.0);
        pick_shift(&mut s, 45.0, 25.0);
        pick_shift(&mut s, 15.0, 25.0);
        assert_eq!(lit(&mut s), 3, "setup: the three lines of the contour are not all chosen: {}", s.status());
        assert_eq!(tool(&mut s), 4, "setup: the fillet is not in hand");
        let drawn = violet(&mut s);
        assert!(drawn > 40, "setup: no corner of the set is drawn in the colour of one that stands: {drawn} pixels of it");
        s.key(Key::Escape);
        assert_eq!(tool(&mut s), 0, "Esc left the fillet in hand: the tool says it is still working");
        assert_eq!(lit(&mut s), 0, "{} lines are still lit after Esc took the corner tool down", lit(&mut s));
        let after = violet(&mut s);
        assert!(after + 40 < drawn, "the corners of the set are still drawn after Esc: {drawn} pixels of violet before, {after} after");
    }
}
