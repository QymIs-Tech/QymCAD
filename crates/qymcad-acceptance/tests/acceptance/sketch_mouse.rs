//! THE MOUSE IN A SKETCH: what a click takes, what a box takes, what a drag carries, what a double click opens, what
//! the cursor snaps to, and what Esc gives back.
use qymcad::{Key, Modifiers, Picture, PointerButton, Session, SketchPick};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// How many things of the sketch are picked.
fn picked(s: &mut Session) -> usize {
    s.document().sketches[0].picked
}

/// Where the points of the sketch stand.
fn places(s: &mut Session) -> Vec<[f64; 2]> {
    s.document().sketches[0].places.clone()
}

/// Whether a point of the sketch stands at `p`, to the last thousandth.
fn stands_at(s: &mut Session, p: (f64, f64)) -> bool {
    places(s).iter().any(|q| (q[0] - p.0).hypot(q[1] - p.1) < 1e-3)
}

/// Drag a box from `a` to `b` of the sheet, as a person drags one to select, standing back first so that both
/// corners of the box are in view - the menu's own "fit the view" does it.
fn box_from(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
    let (view, fit) = (s.word("menu-view"), s.word("menu-fit-view"));
    s.menu(&[&view, &fit]);
    s.drag_on_sketch(a, b);
}

/// A sketch with a 5 x 5 square at (8..13, 5..10) and a tall 5 x 20 shape beside it at (15..20, 0..20). A box drawn
/// about the square alone holds the square wholly and cuts the tall one across, which is what tells a box that takes
/// what is inside from a box that takes what it touches.
fn a_square_and_a_tall_shape() -> Session {
    let mut s = build::empty_sketch();
    build::draw(&mut s, "tb-rect-hint", &[(8.0, 5.0), (13.0, 10.0)]);
    build::draw(&mut s, "tb-rect-hint", &[(15.0, 0.0), (20.0, 20.0)]);
    s
}

/// A rectangle whose corners stand off every whole millimetre: 40.37 wide, so that a snap cannot be explained by the
/// grid. It runs from (9.815, 10) to (50.185, 40).
fn a_rectangle_off_the_grid() -> Session {
    let mut s = build::empty_sketch();
    build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (50.0, 40.0)]);
    let (dim, field) = (s.word("tb-dim-hint"), s.word("sk-expr-example"));
    s.press_hint(&dim);
    s.click_on_sketch(30.0, 10.0);
    s.click_on_sketch(30.0, 4.0);
    s.fill_hinted(&field, "40.37").key(Key::Enter);
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
    s
}

probe! {
    /// A CLICK TAKES ONE THING, A CLICK ASIDE GIVES IT BACK.
    fn a_click_takes_one_thing_and_a_click_aside_gives_it_back() {
        let mut s = a_square_and_a_tall_shape();
        assert!(picked(&mut s) == 0, "something is picked before anything was clicked: {}", picked(&mut s));
        build::pick(&mut s, 10.5, 5.0, false);
        assert!(picked(&mut s) == 1, "a click on a line took {} things", picked(&mut s));
        build::pick(&mut s, 3.0, 15.0, false); // empty sheet beside the shapes
        assert!(picked(&mut s) == 0, "a click on the empty sheet kept {} things picked", picked(&mut s));
    }
}

probe! {
    /// SHIFT ADDS TO WHAT IS PICKED instead of replacing it.
    fn shift_adds_to_what_is_picked() {
        let mut s = a_square_and_a_tall_shape();
        build::pick(&mut s, 10.5, 5.0, false);
        build::pick(&mut s, 17.5, 0.0, true);
        assert!(picked(&mut s) == 2, "two lines clicked with Shift are {} picked", picked(&mut s));
        build::pick(&mut s, 10.5, 10.0, true);
        assert!(picked(&mut s) == 3, "a third line with Shift makes {} picked", picked(&mut s));
    }
}

probe! {
    /// A BOX DRAWN LEFT TO RIGHT TAKES WHAT IS WHOLLY INSIDE IT, and leaves what it merely touches.
    fn a_box_left_to_right_takes_what_is_wholly_inside() {
        let mut s = a_square_and_a_tall_shape();
        // about the square, cutting across the tall shape beside it
        box_from(&mut s, (6.0, 3.0), (17.0, 12.0));
        // the panel counts what is picked: the four lines of the square, its four corners and its centre, and nothing of
        // the tall shape, which the box only cuts across
        assert!(picked(&mut s) == 9, "the box holds the square wholly and only cuts the tall shape: {} things are picked", picked(&mut s));
    }
}

probe! {
    /// A BOX DRAWN RIGHT TO LEFT TAKES WHAT IT TOUCHES as well.
    fn a_box_right_to_left_takes_what_it_touches() {
        let mut s = a_square_and_a_tall_shape();
        box_from(&mut s, (17.0, 12.0), (6.0, 3.0));
        assert!(picked(&mut s) > 9, "a crossing box takes what it touches as well, and over the square (9 things) and part of the tall shape it took only {}", picked(&mut s));
    }
}

probe! {
    /// A DRAG CARRIES THE WHOLE SHAPE when the shape is picked, and only the point when it is not.
    fn a_drag_carries_the_whole_shape_that_is_picked() {
        let mut s = a_square_and_a_tall_shape();
        box_from(&mut s, (6.0, 3.0), (14.0, 12.0)); // the square, wholly
        assert!(picked(&mut s) == 9, "the square - its sides, corners and centre - was not taken whole: {} picked", picked(&mut s));
        s.drag_on_sketch((10.5, 5.0), (10.5, 9.0));
        for corner in [(8.0, 9.0), (13.0, 9.0), (8.0, 14.0), (13.0, 14.0)] {
            assert!(stands_at(&mut s, corner), "the square did not move up by 4 as a whole: its corners are {:?}", places(&mut s));
        }
        for corner in [(15.0, 0.0), (20.0, 20.0)] {
            assert!(stands_at(&mut s, corner), "the square that was not picked moved too: the corners are {:?}", places(&mut s));
        }
    }
}

probe! {
    /// A DOUBLE CLICK ON A CIRCLE OPENS ITS SIZE, and the number typed there holds.
    fn a_double_click_on_a_circle_opens_its_size() {
        let mut s = build::empty_sketch();
        build::circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        let at = s.on_sketch(10.0, 0.0);
        s.double_click(at);
        let sk = s.document().sketches[0].clone();
        let field = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::TextField && w.rect.top() > 120.0).unwrap_or_else(|| panic!("no box to type the size in opened by the circle; on screen: {:?}", s.words()));
        assert!(!field.value.is_empty(), "the box that opened is empty: it should hold the size the circle has now");
        s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text("30").key(Key::Enter);
        let now = s.document().sketches[0].clone();
        let across = now.max[0] - now.min[0];
        assert!((across - 30.0).abs() < 1e-3, "the circle did not take the 30 that was typed: it is {across} across (it was {} before)", sk.max[0] - sk.min[0]);
    }
}

probe! {
    /// A DOUBLE CLICK ON A TEXT OPENS IT FOR WRITING, and what is written stands on the sheet.
    fn a_double_click_on_a_text_opens_it_for_writing() {
        let mut s = build::empty_sketch();
        let hint = s.word("tb-text-hint");
        s.press_hint(&hint);
        let field = s.word("tool-text");
        s.fill(&field, "Alpha"); // the line is typed in the bar, then put on the sheet by a click
        s.click_on_sketch(10.0, 10.0);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        assert!(s.document().sketches[0].texts == 1, "the text was not written: {:?}", s.document().sketches[0].texts);
        // ON THE LETTERS, where a person aims, not on the corner the text is anchored by: the sheet is fitted to the
        // text, and at that zoom the corner stands a few points outside the box of the glyphs a double click opens by
        let at = s.on_sketch(11.0, 11.0);
        // THE BOX AT THE TEXT, on the sheet: the bar of the text tool above holds the same words in a field of its own, and
        // the first field holding them was that one - the check wrote into the bar and read the bar back
        let in_box = |s: &mut Session, words: &str| {
            let canvas = s.canvas();
            s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::TextField && w.value == words && canvas.contains(w.rect.center()))
        };
        s.double_click(at);
        let field = in_box(&mut s, "Alpha").unwrap_or_else(|| panic!("the double click opened no box holding the text; the fields are {:?}", s.widgets()));
        s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text("Beta").key(Key::Enter);
        // the text opened again the same way: its box holds what was written
        s.double_click(at);
        let held = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).map(|w| w.value).collect::<Vec<_>>();
        assert!(in_box(&mut s, "Beta").is_some(), "the text was not changed: opened again, the fields hold {held:?}");
    }
}

probe! {
    /// THE CURSOR SNAPS TO A CORNER that stands off the grid: the line drawn to it ends exactly on it.
    fn the_cursor_snaps_to_a_corner_off_the_grid() {
        let mut s = a_rectangle_off_the_grid();
        let corner = places(&mut s).into_iter().find(|p| p[0] > 50.0).map(|p| (p[0], p[1])).unwrap_or_else(|| panic!("the rectangle has no corner past 50: {:?}", places(&mut s)));
        build::line(&mut s, (60.0, 60.0), (corner.0 + 0.3, corner.1 + 0.3));
        let middle = ((60.0 + corner.0) / 2.0, (60.0 + corner.1) / 2.0);
        let line = match s.sketch_under(middle.0, middle.1) {
            Some(SketchPick::Line { from, to }) => (from, to),
            other => panic!("the line drawn to the corner is not at {middle:?}: {other:?}"),
        };
        let met = (line.0.0 - corner.0).hypot(line.0.1 - corner.1).min((line.1.0 - corner.0).hypot(line.1.1 - corner.1));
        assert!(met < 1e-6, "the line did not end on the corner at {corner:?}: it runs {line:?}");
    }
}

probe! {
    /// THE CURSOR SNAPS TO THE MIDDLE OF A LINE that stands off the grid.
    fn the_cursor_snaps_to_the_middle_of_a_line() {
        let mut s = a_rectangle_off_the_grid();
        let sk = s.document().sketches[0].clone();
        let middle = ((sk.min[0] + sk.max[0]) / 2.0, sk.min[1]);
        build::line(&mut s, (30.0, 60.0), (middle.0 + 0.3, middle.1 + 0.3));
        assert!(stands_at(&mut s, middle), "the line did not end at the middle of the lower side, {middle:?}: the points stand at {:?}", places(&mut s));
    }
}

probe! {
    /// THE CURSOR SNAPS TO THE GRID: a click between the millimetres lands on the whole one.
    fn the_cursor_snaps_to_the_grid() {
        let mut s = build::empty_sketch();
        build::line(&mut s, (10.4, 10.4), (20.4, 10.4));
        assert!(stands_at(&mut s, (10.0, 10.0)) && stands_at(&mut s, (20.0, 10.0)), "the line did not land on whole millimetres: its ends are at {:?}", places(&mut s));
    }
}

probe! {
    /// THE CURSOR SNAPS TO AN AXIS OF THE SHEET: a line drawn beside the Y axis starts on it.
    fn the_cursor_snaps_to_an_axis() {
        let mut s = build::empty_sketch();
        build::line(&mut s, (0.3, 20.0), (20.0, 20.0));
        assert!(stands_at(&mut s, (0.0, 20.0)), "the line did not start on the Y axis: its ends are at {:?}", places(&mut s));
    }
}

probe! {
    /// THE LADDER OF ESC: the drawing being made goes first, the tool goes next, then what is picked, and the sketch
    /// itself stays open through all of it.
    fn esc_gives_back_one_thing_at_a_time() {
        let mut s = build::empty_sketch();
        let name = s.document().sketches[0].name.clone();
        let hint = s.word("tb-line-hint");
        s.press_hint(&hint);
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(20.0, 0.0);
        s.click_on_sketch(20.0, 20.0); // a chain of two, a third being drawn
        s.key(Key::Escape);
        let after_first = s.document().sketches[0].lines;
        assert!(after_first == 2, "the first Esc did not break off the chain and left {after_first} line(s)");
        let tool = s.word("tool-line");
        assert!(s.in_hand().contains(&tool), "the first Esc put the tool down as well: the bar says {:?}", s.in_hand());
        s.key(Key::Escape);
        assert!(!s.in_hand().contains(&tool), "the second Esc did not put the tool down: the bar says {:?}", s.in_hand());
        build::pick(&mut s, 10.0, 0.0, false);
        assert!(picked(&mut s) == 1, "the line was not picked");
        s.key(Key::Escape);
        assert!(picked(&mut s) == 0, "the third Esc did not give back what was picked");
        assert!(s.document().editing.as_deref() == Some(name.as_str()), "Esc left the sketch {name:?}: the program is now editing {:?}", s.document().editing);
        assert!(s.document().sketches[0].lines == 2, "Esc took the drawing away");
    }
}

probe! {
    /// THE SKETCH IS LEFT BY CTRL+ENTER and by the button, and what was drawn stays.
    fn the_sketch_is_left_by_ctrl_enter_and_by_the_button() {
        let mut s = build::empty_sketch();
        build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        assert!(s.document().editing.is_some(), "the sketch is not open");
        s.chord(Modifiers::COMMAND, Key::Enter);
        assert!(s.document().editing.is_none(), "Ctrl+Enter did not leave the sketch; on screen: {:?}", s.words());
        assert!(s.document().sketches[0].lines == 4, "leaving the sketch took the drawing away");
        // and again, by the button
        let name = s.document().sketches[0].name.clone();
        let row = s.find(&name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the sketch is not in the tree; on screen: {:?}", s.words()));
        s.double_click(row.center());
        assert!(s.document().editing.is_some(), "the sketch did not open again by a double click in the tree");
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        assert!(s.document().editing.is_none(), "the button did not leave the sketch; on screen: {:?}", s.words());
    }
}

probe! {
    /// A DRAG ON A POINT THAT IS NOT PICKED CARRIES THE POINT ALONE.
    fn a_drag_on_a_point_carries_the_point_alone() {
        let mut s = a_square_and_a_tall_shape();
        s.drag_on_sketch((13.0, 10.0), (12.0, 13.0));
        assert!(stands_at(&mut s, (12.0, 13.0)), "the corner did not follow the drag: the points stand at {:?}", places(&mut s));
        assert!(stands_at(&mut s, (8.0, 5.0)) && stands_at(&mut s, (15.0, 0.0)), "the rest of the drawing moved with it: {:?}", places(&mut s));
        assert!(!matches!(s.sketch_under(13.0, 10.0), Some(SketchPick::Point { .. })), "a point is still where the drag began");
    }
}

probe! {
    /// A DRAG WITH THE MIDDLE BUTTON MOVES THE SHEET, not the drawing.
    fn a_drag_with_the_middle_button_moves_the_sheet() {
        let mut s = a_square_and_a_tall_shape();
        let before = places(&mut s);
        let from = s.on_sketch(3.0, 15.0);
        s.drag(from, from + qymcad::vec2(80.0, 40.0), PointerButton::Middle, Modifiers::default());
        assert!(places(&mut s) == before, "dragging the sheet moved the drawing: {before:?} became {:?}", places(&mut s));
    }
}

probe! {
    /// THE HANDLES OF A SPLINE SHOW WHILE IT IS PICKED, and the one under the pointer and the one held say so. A spline
    /// drawn through (0, 0), (20, 24) and (40, 0): its middle node's handle ends at (40, 24), the automatic tangent there
    /// being half the step between its neighbours. Not picked - no handle there; a click on the curve - the handle drawn;
    /// the pointer over it - lit; pressed and pulled up - drawn in another colour and the curve bends after it; let go -
    /// the new shape kept.
    fn a_spline_shows_its_handles_only_while_picked_and_the_one_under_the_pointer_is_lit() {
        let mut s = build::empty_sketch();
        let spline = s.word("tb-spline-hint");
        s.press_hint(&spline);
        s.click_on_sketch(0.0, 0.0).click_on_sketch(20.0, 24.0);
        let end = s.on_sketch(40.0, 0.0);
        s.double_click(end);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        // where a place of the sketch is seen now: the view is read again at every step, not carried over
        let seen = |s: &mut Session, x: f64, y: f64| s.seen_on_sketch(x, y).unwrap_or_else(|| panic!("({x}, {y}) of the sketch is off the canvas"));
        // the colour is read a little off the pointer, clear of its cross
        let near = |p: qymcad::Pos2| p + qymcad::vec2(2.0, 2.0);
        // the wheel takes the view back until the spline and the place the handle is pulled to are seen together
        for _ in 0..12 {
            if s.seen_on_sketch(0.0, 0.0).is_some() && s.seen_on_sketch(40.0, 64.0).is_some() {
                break;
            }
            let middle = s.canvas().center();
            s.wheel(middle, qymcad::vec2(0.0, -120.0), Modifiers::NONE);
        }
        s.on_sketch(20.0, 32.0);
        let corner = s.canvas().min;
        let idle = s.snapshot();
        let sheet = idle.colour_at(corner + qymcad::vec2(12.0, 12.0));
        let handle = seen(&mut s, 40.0, 24.0);
        assert!(Picture::apart(idle.colour_at(near(handle)), sheet) < 8, "a spline not picked shows a handle: {:?} against the sheet {sheet:?}", idle.colour_at(near(handle)));

        // a point of the curve between the first two nodes: the Hermite span at its middle
        let curve = seen(&mut s, 8.75, 13.5);
        s.click(curve);
        assert_eq!(picked(&mut s), 1, "a click on the curve of the spline picked {} things", picked(&mut s));
        s.move_to(corner + qymcad::vec2(30.0, 30.0));
        let handle = seen(&mut s, 40.0, 24.0);
        let shown = s.snapshot().colour_at(near(handle));
        assert!(Picture::apart(shown, sheet) >= 20, "the spline is picked and its handle is not drawn: {shown:?} against the sheet {sheet:?}");

        let handle = seen(&mut s, 40.0, 24.0);
        s.move_to(handle);
        let lit = s.snapshot().colour_at(near(seen(&mut s, 40.0, 24.0)));
        assert!(Picture::apart(lit, shown) >= 20, "the handle under the pointer is not lit: {lit:?}, as it was away from it {shown:?}");

        s.press(PointerButton::Primary);
        let pulled = seen(&mut s, 40.0, 64.0);
        let handle = seen(&mut s, 40.0, 24.0);
        s.move_to(handle + (pulled - handle) * 0.5);
        s.move_to(pulled);
        let held = s.snapshot().colour_at(near(seen(&mut s, 40.0, 64.0)));
        s.release(PointerButton::Primary);
        assert!(Picture::apart(held, lit) >= 20 && Picture::apart(held, sheet) >= 20, "the handle held is not drawn in a colour of its own: {held:?}, lit {lit:?}, sheet {sheet:?}");
        // the tangent at the middle node went from (20, 0) to (20, 40): the middle of the first span drops 5 mm
        assert!(s.sketch_under(8.75, 13.5).is_none(), "the curve did not bend after the handle: it still passes (8.75, 13.5)");
        assert!(matches!(s.sketch_under(8.75, 8.5), Some(SketchPick::Spline { .. })), "the curve let go of does not pass (8.75, 8.5) where its new tangent puts it");
    }
}

probe! {
    /// A SPLINE PICKED BY ITS CURVE IS DELETED BY DELETE, with its nodes, as a line goes with its ends.
    fn a_spline_picked_by_its_curve_is_deleted_whole() {
        let mut s = build::empty_sketch();
        let spline = s.word("tb-spline-hint");
        s.press_hint(&spline);
        s.click_on_sketch(0.0, 0.0).click_on_sketch(20.0, 24.0);
        let end = s.on_sketch(40.0, 0.0);
        s.double_click(end);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        s.click_on_sketch(8.75, 13.5);
        assert_eq!(picked(&mut s), 1, "a click on the curve of the spline picked {} things", picked(&mut s));
        s.key(Key::Delete);
        let sk = &s.document().sketches[0];
        assert!(sk.splines == 0 && sk.places.is_empty(), "Delete left {} splines and the points {:?}", sk.splines, sk.places);
    }
}

probe! {
    /// A SPLINE CLOSED ON ITS FIRST NODE IS A LOOP: nodes clicked at (0, 0), (40, 0) and (20, 30), then the first one
    /// again - one spline of three nodes, no node lying on another, its closing span drawn from (20, 30) back to (0, 0);
    /// a node dragged away, the loop stays closed. Reported behaviour: a spline brought back to its start ended there, two
    /// nodes one over the other with two handles, and fell apart when dragged.
    fn a_spline_clicked_back_on_its_first_node_closes() {
        let mut s = build::empty_sketch();
        let spline = s.word("tb-spline-hint");
        s.press_hint(&spline);
        s.click_on_sketch(0.0, 0.0).click_on_sketch(40.0, 0.0).click_on_sketch(20.0, 30.0).click_on_sketch(0.0, 0.0);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        let sk = &s.document().sketches[0];
        assert!(sk.splines == 1 && sk.places.len() == 3, "the spline closed on its first node holds {} splines and the points {:?}", sk.splines, sk.places);
        // the middle of the closing span, from (20, 30) to (0, 0), the tangents of the loop half the step between the
        // neighbours, (-20, 0) and (10, -15): (10, 15) + (-20, 0) / 8 - (10, -15) / 8 = (6.25, 16.875)
        assert!(matches!(s.sketch_under(6.25, 16.875), Some(SketchPick::Spline { .. })), "no closing span runs from (20, 30) back to (0, 0)");
        s.drag_on_sketch((40.0, 0.0), (50.0, -10.0));
        let sk = &s.document().sketches[0];
        assert!(sk.splines == 1 && sk.places.len() == 3, "the loop dragged by a node holds {} splines and the points {:?}", sk.splines, sk.places);
        // with the node at (50, -10) the tangents are (-25, 5) and (15, -20): the middle comes to (5, 18.125)
        assert!(matches!(s.sketch_under(5.0, 18.125), Some(SketchPick::Spline { .. })), "the loop dragged by a node is no longer closed");
    }
}
