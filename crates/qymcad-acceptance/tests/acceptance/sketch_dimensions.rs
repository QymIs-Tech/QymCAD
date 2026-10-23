//! THE DIMENSIONS OF A SKETCH BEYOND THE THREE ON THE PANEL: the length of an arc, the gap between two round edges, a
//! dimension made a reference by hand and one made a reference by the program, the label of a dimension dragged aside,
//! and a value or a constraint that contradicts what is already set.
//!
//! The panel's own three - linear, angular, radius - go through the contract (`contract::dim_linear` and the two
//! beside it); what is here has no button of its own and lives in the menu of the sheet, in the popup of a dimension
//! and in the list of constraints.
use qymcad::{Key, Modifiers, PointerButton, Session};
use qymcad_acceptance::contract::fixtures::Fixture;
use qymcad_acceptance::probe;

/// THE DIAMETER OF THE CIRCLE through `rim` LAID BY HAND: a circle drawn with no size typed carries none, and a double
/// click on it opens the field of its diameter; `value` typed and Enter lay it.
fn diameter_laid(s: &mut Session, rim: (f64, f64), value: &str) {
    let at = s.on_sketch(rim.0, rim.1);
    s.double_click(at);
    s.type_text(value).key(Key::Enter);
}

/// The middle of the canvas, the place to look for the words drawn on the sheet.
const SHEET: qymcad::Pos2 = qymcad::pos2(640.0, 400.0);

/// Open the menu of the sheet at (x, y) and press the item `key` names, going down `sub` first when there is one.
fn menu_item(s: &mut Session, x: f64, y: f64, sub: Option<&str>, key: &str) {
    let at = s.on_sketch(x, y);
    s.click_with(at, PointerButton::Secondary, Modifiers::default());
    let item = s.word(key);
    match sub {
        Some(sub) => {
            let sub = s.word(sub);
            s.menu(&[&sub, &item]);
        }
        None => {
            s.press_word_near(&item, at);
        }
    }
}

/// Put the arrow down and pick what lies at (x, y) of the sheet, adding to what is picked when `add`.
fn pick(s: &mut Session, x: f64, y: f64, add: bool) {
    let at = s.on_sketch(x, y);
    if add {
        s.click_with(at, PointerButton::Primary, Modifiers::SHIFT);
    } else {
        s.click(at);
    }
}

/// Type `text` into the dimension whose label reads `label`, the way the sketch itself tells a person to: "a dimension
/// value is set by double-clicking".
fn type_into_dimension(s: &mut Session, label: &str, text: &str) {
    let at = s.find(label, SHEET).unwrap_or_else(|| panic!("the dimension {label:?} is not on the sheet; on screen: {:?}", s.words()));
    s.double_click(at.center());
    let field = s.word("f-number-or-expr");
    s.fill_hinted(&field, text).key(Key::Enter);
}

/// A 40 x 30 rectangle with the length of its lower side set to 50 by a dimension, the arrow back in hand.
fn rectangle_with_a_dimension(s: &mut Session) {
    let hint = s.word("tb-dim-hint");
    let field = s.word("f-number-or-expr");
    s.press_hint(&hint);
    s.click_on_sketch(20.0, 0.0);
    s.click_on_sketch(20.0, -10.0);
    s.fill_hinted(&field, "50").key(Key::Enter);
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
}

probe! {
    /// AN ARC GETS A DIMENSION OF ITS LENGTH from the menu of the sheet, and the number typed into it holds the arc.
    fn an_arc_gets_its_length_dimensioned() {
        let mut s = Fixture::ArcInSketch.start();
        pick(&mut s, 7.07, 7.07, false);
        menu_item(&mut s, 7.07, 7.07, None, "sk-arc-length-dim");
        let kinds = s.document().sketches[0].constraint_kinds.clone();
        assert!(kinds.iter().any(|k| k == "ArcLength"), "the arc has no dimension of its length: {kinds:?}");
        // a quarter of a circle of radius 10 is 15.708 long, and that is what the dimension shows: the number, with the
        // arc mark drawn over it (where it stands is checked on its geometry, `arc_length_dim_geom`)
        assert!(s.find("15.7", SHEET).is_some(), "the length of the arc is not written on the sheet; on screen: {:?}", s.words());
        // the arc holds by its length now: no radius is labelled beside it as if it held by that
        assert!(s.find("R10.0", SHEET).is_none(), "a radius the arc does not hold by is still labelled beside it; on screen: {:?}", s.words());
        type_into_dimension(&mut s, "15.7", "20");
        assert!(s.find("20.0", SHEET).is_some(), "the arc did not take the length that was typed; on screen: {:?}", s.words());
    }
}

probe! {
    /// THE GAP BETWEEN TWO ROUND EDGES IS DIMENSIONED, and the number typed into it moves one circle away from the
    /// other: two circles of radius 5 with a gap of 40 between their edges are 50 across.
    fn the_gap_between_two_round_edges_is_dimensioned() {
        let mut s = Fixture::TwoCirclesInSketch.start();
        diameter_laid(&mut s, (5.0, 0.0), "10");
        diameter_laid(&mut s, (35.0, 0.0), "10");
        pick(&mut s, 5.0, 0.0, false);
        pick(&mut s, 35.0, 0.0, true);
        menu_item(&mut s, 35.0, 0.0, None, "sk-tangent-dim");
        let kinds = s.document().sketches[0].constraint_kinds.clone();
        assert!(kinds.iter().any(|k| k == "EdgeDistance"), "the two circles have no dimension of the gap between them: {kinds:?}");
        // the centres are 30 apart and each circle is 5 in radius: the gap between the edges is 20
        let _ = s.on_sketch(15.0, 0.0); // the dimension stands between the circles - it is brought into view
        type_into_dimension(&mut s, "T 20.0", "40");
        let sk = s.document().sketches[0].clone();
        let across = sk.max[0] - sk.min[0];
        // a gap of 40 between the edges plus the two circles themselves (10 each) is 60 across
        assert!((across - 60.0).abs() < 0.5, "with a gap of 40 between two circles of radius 5 the sketch is 60 across, and it is {across}");
    }
}

probe! {
    /// A DIMENSION IS MADE A REFERENCE BY HAND: it stops driving the geometry, keeps showing the value, and gives back
    /// the degree of freedom it held.
    fn a_dimension_is_made_a_reference_by_hand() {
        let mut s = Fixture::RectangleInSketch.start();
        rectangle_with_a_dimension(&mut s);
        let driving = s.document().sketches[0].clone();
        assert!(s.find("50.0", SHEET).is_some(), "the dimension is not on the sheet; on screen: {:?}", s.words());
        let at = s.find("50.0", SHEET).expect("the dimension is on the sheet");
        s.double_click(at.center());
        let word = s.word("sk-ref-short");
        // the switch of the box of the dimension, kept by Enter as every value of a box is
        s.press_word_near(&word, at.center()).key(Key::Enter);
        let now = s.document().sketches[0].clone();
        assert!(now.dof == driving.dof + 1, "a reference dimension holds no degree of freedom: {} became {}", driving.dof, now.dof);
        assert!(s.find("(50.0)", SHEET).is_some(), "a reference dimension is not marked as one on the sheet; on screen: {:?}", s.words());
        let across = now.max[0] - now.min[0];
        assert!((across - 50.0).abs() < 1e-6, "making the dimension a reference moved the geometry: the rectangle is {across} across");
    }
}

probe! {
    /// A DIMENSION THAT SAYS WHAT IS ALREADY SET BECOMES A REFERENCE BY ITSELF: the sketch keeps its degrees of
    /// freedom, the geometry does not move, and the sheet marks the new dimension as a reference.
    fn a_dimension_that_repeats_another_becomes_a_reference() {
        let mut s = Fixture::RectangleInSketch.start();
        rectangle_with_a_dimension(&mut s);
        let one = s.document().sketches[0].clone();
        // the same side, a second time: the length is already set, so the new dimension can only show it
        let hint = s.word("tb-dim-hint");
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0);
        s.click_on_sketch(20.0, -25.0);
        let two = s.document().sketches[0].clone();
        assert!(two.dof == one.dof, "the repeated dimension took a degree of freedom: {} became {}", one.dof, two.dof);
        assert!(s.find("(50.0)", SHEET).is_some(), "the repeated dimension is not marked as a reference; on screen: {:?}", s.words());
        let across = two.max[0] - two.min[0];
        assert!((across - 50.0).abs() < 1e-6, "the repeated dimension moved the geometry: the rectangle is {across} across");
    }
}

probe! {
    /// THE LABEL OF A DIMENSION IS DRAGGED ASIDE, and the geometry it measures does not move with it.
    fn the_label_of_a_dimension_is_dragged_aside() {
        let mut s = Fixture::RectangleInSketch.start();
        rectangle_with_a_dimension(&mut s);
        let before = s.document().sketches[0].clone();
        let label = s.find("50.0", SHEET).unwrap_or_else(|| panic!("the dimension is not on the sheet; on screen: {:?}", s.words()));
        s.drag(label.center(), label.center() + qymcad::vec2(0.0, 60.0), PointerButton::Primary, Modifiers::default());
        let moved = s.find("50.0", SHEET).expect("the dimension is still on the sheet");
        assert!(moved.center().y > label.center().y + 30.0, "the label did not follow the drag: {label:?} became {moved:?}");
        let after = s.document().sketches[0].clone();
        assert!(after.max == before.max && after.min == before.min, "dragging the label moved the geometry: {:?} became {:?}", before.max, after.max);
    }
}

probe! {
    /// THE LABEL OF AN ANGLE IS DRAGGED ASIDE as a length's is - farther out, then past a side - the geometry stays,
    /// and Ctrl+Z puts the label back where the last drag took it from.
    fn the_label_of_an_angle_is_dragged_aside() {
        let mut s = Fixture::TwoLinesInSketch.start();
        let (hint, field) = (s.word("tb-dim-angle-hint"), s.word("f-number-or-expr"));
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0);
        s.click_on_sketch(14.14, 14.14);
        s.click_on_sketch(12.0, 5.0);
        s.fill_hinted(&field, "45").key(Key::Enter);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        let before = s.document().sketches[0].clone();
        let origin = s.on_sketch(0.0, 0.0); // the vertex brought into view first: the screen stands still from here
        let label = s.find("45°", SHEET).unwrap_or_else(|| panic!("the angle is not on the sheet; on screen: {:?}", s.words()));
        let out = origin + (label.center() - origin) * 1.4;
        s.drag(label.center(), out, PointerButton::Primary, Modifiers::default());
        let moved = s.find("45°", SHEET).expect("the angle is still on the sheet");
        assert!(moved.center().distance(out) < 6.0, "the label of the angle did not follow the drag to {out:?}: it stands at {:?}", moved.center());
        // below the first side, which runs to the right of the vertex on the screen
        let below = origin + qymcad::vec2(200.0, 60.0);
        s.drag(moved.center(), below, PointerButton::Primary, Modifiers::default());
        let past = s.find("45°", SHEET).expect("the angle is still on the sheet");
        assert!(past.center().distance(below) < 6.0, "the label of the angle did not go past the side to {below:?}: it stands at {:?}", past.center());
        let after = s.document().sketches[0].clone();
        assert!(after.max == before.max && after.min == before.min, "dragging the label of the angle moved the geometry: {:?} became {:?}", before.max, after.max);
        s.chord(Modifiers::COMMAND, Key::Z);
        let back = s.find("45°", SHEET).expect("the angle is still on the sheet");
        assert!(back.center().distance(moved.center()) < 6.0, "Ctrl+Z did not put the label back where the last drag took it from ({:?}): it stands at {:?}", moved.center(), back.center());
    }
}

/// The right side of the rectangle of the fixture given a height by the formula `text`, its dimension line led out to
/// x = 50; the arrow back in hand. Answers where on the screen the dimension line runs.
fn right_side_with_a_height(s: &mut Session, text: &str) -> f32 {
    let (hint, field) = (s.word("tb-dim-hint"), s.word("f-number-or-expr"));
    s.press_hint(&hint);
    s.click_on_sketch(40.0, 15.0);
    s.click_on_sketch(50.0, 15.0);
    s.fill_hinted(&field, text).key(Key::Enter);
    s.key(Key::Escape);
    let arrow = s.word("tb-select-hint");
    s.press_hint(&arrow);
    s.on_sketch(50.0, 15.0).x
}

/// Open the settings at the sketch and put them away after `f`.
fn in_the_sketch_settings(s: &mut Session, f: impl FnOnce(&mut Session)) {
    let (windows, settings) = (s.word("menu-windows"), s.word("menu-settings"));
    s.menu(&[&windows, &settings]);
    let section = s.word("settings-sec-sketch");
    // the word of the section nearest the title of the window: the tree beside it says "Sketch" too
    let title = s.word("win-settings");
    let near = s.find(&title, qymcad::pos2(700.0, 200.0)).map_or(qymcad::pos2(700.0, 200.0), |r| r.center());
    s.press_word_near(&section, near);
    f(s);
    let title = s.word("win-settings");
    s.close_window(&title);
}

probe! {
    /// THE TEXT OF A VERTICAL DIMENSION DOES NOT LIE ON ITS LINE: it stands beside the line, clear of it. Reported
    /// behaviour: the text of a dimension crosses the dimension line itself.
    fn the_text_of_a_dimension_stands_clear_of_its_line() {
        let mut s = Fixture::RectangleInSketch.start();
        let line = right_side_with_a_height(&mut s, "30");
        let text = s.find("30.0", SHEET).unwrap_or_else(|| panic!("the dimension is not on the sheet; on screen: {:?}", s.words()));
        assert!(text.max.x < line || text.min.x > line, "the text {text:?} of the vertical dimension lies across its line at x = {line}");
    }
}

probe! {
    /// THE SETTINGS OF THE SKETCH DECIDE WHAT A LABEL SAYS AND HOW LARGE: the formula shown ahead of the value once it
    /// is asked for, and the label grows with the plus beside its size.
    fn the_settings_decide_what_a_label_says_and_how_large() {
        let mut s = Fixture::RectangleInSketch.start();
        right_side_with_a_height(&mut s, "2*15");
        let plain = s.find("30.0", SHEET).unwrap_or_else(|| panic!("the dimension set by a formula shows its value alone at first; on screen: {:?}", s.words()));
        in_the_sketch_settings(&mut s, |s| {
            let formula = s.word("settings-dim-formula");
            s.toggle(&formula);
        });
        assert!(s.find("2*15 = 30.0", SHEET).is_some(), "the formula was asked for and the label does not show it; on screen: {:?}", s.words());
        in_the_sketch_settings(&mut s, |s| {
            let larger = s.word("settings-dim-font-larger");
            for _ in 0..6 {
                s.press_hint(&larger);
            }
        });
        let large = s.find("2*15 = 30.0", SHEET).expect("the label is still on the sheet");
        assert!(large.height() > plain.height() * 1.3, "six presses of the plus left the label {:.1} px tall, it was {:.1}", large.height(), plain.height());
    }
}

probe! {
    /// THE TEXT OF A LENGTH IS LED ALONG ITS LINE AND PAST THE ARROW, where it stands on a shelf, and the geometry
    /// stays.
    fn the_text_of_a_length_is_led_past_its_arrow() {
        let mut s = Fixture::RectangleInSketch.start();
        rectangle_with_a_dimension(&mut s);
        let before = s.document().sketches[0].clone();
        let end = s.on_sketch(50.0, 0.0); // the right end of the side the dimension measures, in view from here on
        let label = s.find("50.0", SHEET).unwrap_or_else(|| panic!("the dimension is not on the sheet; on screen: {:?}", s.words()));
        let past = qymcad::pos2(end.x + 60.0, label.center().y);
        s.drag(label.center(), past, PointerButton::Primary, Modifiers::default());
        let moved = s.find("50.0", SHEET).expect("the dimension is still on the sheet");
        assert!(moved.min.x > end.x && (moved.center().x - past.x).abs() < 6.0, "the text led past the arrow at x {} stands at {moved:?}", end.x);
        let after = s.document().sketches[0].clone();
        assert!(after.max == before.max && after.min == before.min, "leading the text moved the geometry: {:?} became {:?}", before.max, after.max);
    }
}

probe! {
    /// THE TEXT OF A DIAMETER IS LAID ON THE DIMENSION LINE when led inside the circle, and goes back onto its shelf
    /// when led out past the rim.
    fn the_text_of_a_diameter_lies_on_its_line() {
        let mut s = Fixture::CircleInSketch.start();
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        diameter_laid(&mut s, (10.0, 0.0), "20");
        let centre = s.on_sketch(0.0, 0.0);
        let rim = s.on_sketch(10.0, 0.0);
        let r = rim.x - centre.x;
        let label = s.find("Ø20.0", SHEET).unwrap_or_else(|| panic!("the diameter of the circle is not on the sheet; on screen: {:?}", s.words()));
        let inside = centre + qymcad::vec2(r * 0.4, -r * 0.3);
        s.drag(label.center(), inside, PointerButton::Primary, Modifiers::default());
        let on_line = s.find("Ø20.0", SHEET).or_else(|| s.find("(Ø20.0)", SHEET)).expect("the diameter is still on the sheet");
        assert!(on_line.center().distance(centre) < r, "the text led inside the circle stands at {:?}, outside the circle of {r} px about {centre:?}", on_line.center());
        let out = centre + qymcad::vec2(r * 2.0, -10.0);
        s.drag(on_line.center(), out, PointerButton::Primary, Modifiers::default());
        let shelf = s.find("Ø20.0", SHEET).or_else(|| s.find("(Ø20.0)", SHEET)).expect("the diameter is still on the sheet");
        assert!(shelf.center().distance(centre) > r, "the text led out past the rim did not leave the circle: it stands at {:?}", shelf.center());
    }
}

probe! {
    /// A VERTICAL DIMENSION IS WRITTEN UPRIGHT, along its line and beyond it from the geometry - and level once the
    /// settings of the sketch ask for level text.
    fn a_vertical_dimension_is_written_along_its_line() {
        let mut s = Fixture::RectangleInSketch.start();
        let line = right_side_with_a_height(&mut s, "30");
        let upright = s.find("30.0", SHEET).unwrap_or_else(|| panic!("the dimension is not on the sheet; on screen: {:?}", s.words()));
        assert!(upright.height() > upright.width(), "the text of the vertical dimension is not upright: {upright:?}");
        assert!(upright.min.x > line, "the dimension led out to the right of the geometry has its text at {upright:?}, not right of its line at x = {line}");
        in_the_sketch_settings(&mut s, |s| {
            let level = s.word("settings-dim-text-level");
            let title = s.word("win-settings");
            let near = s.find(&title, qymcad::pos2(700.0, 200.0)).map_or(qymcad::pos2(700.0, 200.0), |r| r.center());
            s.press_word_near(&level, near);
        });
        let level = s.find("30.0", SHEET).expect("the dimension is still on the sheet");
        assert!(level.width() > level.height() && level.min.x > line, "asked for level text, the vertical dimension shows {level:?}");
    }
}

probe! {
    /// A VALUE THE GEOMETRY CANNOT TAKE EITHER HOLDS OR IS REFUSED IN WORDS: a diagonal of 10 across a rectangle whose
    /// side is already set to 50 cannot be reached, and a dimension that is not reached has to be answered.
    fn a_value_the_geometry_cannot_take_is_refused() {
        let mut s = Fixture::RectangleInSketch.start();
        rectangle_with_a_dimension(&mut s);
        let before = s.document().sketches[0].clone();
        // corner to corner: the side is 50 and the height 30, so the diagonal is 58.3 and 10 is impossible. The rectangle
        // was drawn from (0, 0) and grew from there to 50: its corners stand at (0, 0) and (50, 30)
        let hint = s.word("tb-dim-hint");
        let field = s.word("f-number-or-expr");
        s.press_hint(&hint);
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(50.0, 30.0);
        // placed off the middle of the diagonal square to it, so the dimension is aligned with the diagonal: put to
        // the side it would be a vertical one, and a height of 10 is one the rectangle can take
        s.click_on_sketch(18.8, 25.3);
        s.fill_hinted(&field, "10").key(Key::Enter);
        let said = s.status();
        let after = s.document().sketches[0].clone();
        let (w, h) = (after.max[0] - after.min[0], after.max[1] - after.min[1]);
        let diagonal = (w * w + h * h).sqrt();
        assert!(
            (diagonal - 10.0).abs() < 0.1 || said != s.word("sk-dim-placed"),
            "the rectangle does not take the diagonal of 10 that was typed (it is {diagonal:.1} across the diagonal, and it was {:?} before), and nothing is said about it: the status line says {said:?}",
            before.max
        );
    }
}

probe! {
    /// A CONSTRAINT THAT CONTRADICTS A DIMENSION IS ANSWERED: an angle of 45 degrees between two lines and then
    /// "parallel" on the same two cannot hold together, and the program has to say so - it marks the conflict, offers
    /// the dimension to be made a reference, or refuses the constraint.
    fn a_constraint_that_contradicts_a_dimension_is_flagged() {
        let mut s = Fixture::TwoLinesInSketch.start();
        let hint = s.word("tb-dim-angle-hint");
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0);
        s.click_on_sketch(14.14, 14.14);
        s.key(Key::Escape);
        let arrow = s.word("tb-select-hint");
        s.press_hint(&arrow);
        let kinds = s.document().sketches[0].constraint_kinds.clone();
        assert!(kinds.iter().any(|k| k == "AngleLines"), "the two lines have no angular dimension: {kinds:?}");
        pick(&mut s, 20.0, 0.0, false);
        pick(&mut s, 14.14, 14.14, true);
        menu_item(&mut s, 14.14, 14.14, Some("sk-constraint"), "sk-parallel");
        let said = s.status();
        let way_out = s.find_hint(&s.word("sk-make-driven-hint")).is_some();
        let lines_parallel = s.document().sketches[0].constraint_kinds.iter().any(|k| k == "Parallel");
        assert!(
            !lines_parallel || way_out || said.contains(&s.word("sk-dim-conflict")) || said.contains(&s.word("sk-length-conflict")) || said.contains(&s.word("sk-angle-conflict")),
            "the angle of 45 degrees and the parallel constraint are both in the sketch, nothing is marked as conflicting and there is no way out offered; the status line says {said:?}"
        );
    }
}

probe! {
    /// ENTER PUTS DOWN A DIMENSION TOOL THAT MEASURED NOTHING, as Esc does: the radius taken and nothing clicked, Enter
    /// leaves nothing in hand waiting for a click with no bar saying so, and lays nothing.
    fn enter_puts_down_the_radius_that_measured_nothing() {
        let mut s = Fixture::CircleInSketch.start();
        s.key(Key::Escape).key(Key::Escape);
        let (before, bar) = (s.document().sketches[0].constraint_kinds.clone(), s.in_hand());
        let hint = s.word("tb-dim-radius-hint");
        s.press_hint(&hint);
        assert_ne!(s.in_hand(), bar, "setup: the radius is not in hand");
        s.key(Key::Enter);
        assert_eq!(s.in_hand(), bar, "Enter left the radius in hand: the bar says {:?}", s.in_hand());
        assert_eq!(s.document().sketches[0].constraint_kinds, before, "Enter on the radius that measured nothing laid something");
    }
}

probe! {
    /// ENTER PUTS A DIMENSION BEGUN WHERE THE POINTER IS: the side of a rectangle picked by the linear dimension, the pointer
    /// moved below it, and Enter lays the dimension there, as the click that places it does.
    fn enter_puts_a_dimension_begun_where_the_pointer_is() {
        let mut s = Fixture::RectangleInSketch.start();
        s.key(Key::Escape).key(Key::Escape);
        let before = s.document().sketches[0].constraint_kinds.len();
        let hint = s.word("tb-dim-hint");
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0);
        let below = s.on_sketch(20.0, -10.0);
        s.move_to(below);
        s.key(Key::Enter);
        let after = s.document().sketches[0].constraint_kinds.len();
        assert_eq!(after, before + 1, "Enter after the side was picked laid {} constraints, not the one dimension; the status line says {:?}", after - before, s.status());
    }
}

/// A LINEAR DIMENSION CLICKED ON THE POINTS OF TWO CIRCLES their centre lines cross them at, `ends`, placed above them and
/// given `value`: answers how wide the sketch stands then.
fn across_after_a_dimension_at(ends: [(f64, f64); 2], value: &str) -> f64 {
    let mut s = Fixture::TwoCirclesInSketch.start();
    diameter_laid(&mut s, (5.0, 0.0), "10");
    diameter_laid(&mut s, (35.0, 0.0), "10");
    let hint = s.word("tb-dim-hint");
    s.press_hint(&hint);
    for (x, y) in ends {
        let at = s.on_sketch(x, y);
        s.click(at);
    }
    let above = s.on_sketch(15.0, 12.0);
    s.click(above);
    s.type_text(value).key(Key::Enter);
    let sk = s.document().sketches[0].clone();
    sk.max[0] - sk.min[0]
}

probe! {
    /// A DIMENSION STANDS ON THE POINTS CLICKED: clicked on the far sides of two circles, where their centre lines cross
    /// them, it measures from far side to far side - two circles of diameter 10 given 50 so stand 50 across. Reported
    /// behaviour: the dimension went to the near sides, and the circles stood 60 across.
    fn a_dimension_between_the_far_sides_of_two_circles_stands_on_them() {
        let across = across_after_a_dimension_at([(-5.0, 0.0), (35.0, 0.0)], "50");
        assert!((across - 50.0).abs() < 0.01, "a dimension of 50 clicked on the far sides of two circles of diameter 10: the sketch stands {across} across, not 50");
    }
}

probe! {
    /// A DIMENSION FROM THE FAR SIDE OF ONE CIRCLE TO THE NEAR SIDE OF THE OTHER stands on the two points clicked: given
    /// 40, the circles of diameter 10 stand 50 across - 40 from the far side of the first to the near side of the second,
    /// and the second whole beyond it.
    fn a_dimension_from_a_far_side_to_a_near_side_stands_on_them() {
        let across = across_after_a_dimension_at([(-5.0, 0.0), (25.0, 0.0)], "40");
        assert!((across - 50.0).abs() < 0.01, "a dimension of 40 clicked from the far side of one circle to the near side of the other: the sketch stands {across} across, not 50");
    }
}

probe! {
    /// A DIMENSION ON THE POINTS OF TWO CIRCLES ADDS NO BADGES: the points it stands on are its own, held on their
    /// circles level with the centres, and show no constraint of their own. Reported behaviour: the centre lines carried
    /// badges of a level line and the ends of the dimension the badge of a point on a circle.
    fn a_dimension_on_the_points_of_two_circles_adds_no_badges() {
        let mut s = Fixture::TwoCirclesInSketch.start();
        diameter_laid(&mut s, (5.0, 0.0), "10");
        diameter_laid(&mut s, (35.0, 0.0), "10");
        let before = s.badges().len();
        let hint = s.word("tb-dim-hint");
        s.press_hint(&hint);
        for (x, y) in [(-5.0, 0.0), (35.0, 0.0), (15.0, 12.0)] {
            let at = s.on_sketch(x, y);
            s.click(at);
        }
        s.key(Key::Enter);
        s.key(Key::Escape).key(Key::Escape);
        let badges = s.badges();
        assert!(badges.len() == before, "the dimension on the far sides of two circles added badges: {badges:?}, there were {before}");
    }
}
