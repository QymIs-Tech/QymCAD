//! THE MEASURE IN A SKETCH: two clicks and the program says how far apart they are, snapping to what is drawn.
use qymcad::{Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The measure taken, in a sketch holding a 40 x 30 rectangle with a corner at the origin.
fn a_rectangle_and_the_measure() -> Session {
    let mut s = build::empty_sketch();
    build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
    let hint = s.word("tb-measure-hint");
    s.press_hint(&hint);
    s
}

/// Whether the status line holds all of these, as a person reads them off it.
fn says_all(s: &mut Session, numbers: &[&str]) -> bool {
    let said = s.status();
    numbers.iter().all(|n| said.contains(n))
}

probe! {
    /// TWO CLICKS TELL THE DISTANCE and how it splits along the axes: the diagonal of a 40 by 30 rectangle is 50.
    fn two_clicks_tell_the_distance_and_its_parts() {
        let mut s = a_rectangle_and_the_measure();
        assert!(s.status() == s.word("in-measure-hint"), "the measure does not say what it waits for: {:?}", s.status());
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(40.0, 30.0);
        assert!(says_all(&mut s, &["50.000", "40.000", "30.000"]), "the diagonal of a 40 by 30 rectangle is 50 with parts of 40 and 30, and the status line says {:?}", s.status());
    }
}

probe! {
    /// THE MEASURE SNAPS TO WHAT IS DRAWN, not to the grid: the rectangle is given a side of 40.37, so that its
    /// corners stand off every whole millimetre, and a click a little off a corner still measures the rectangle
    /// itself, to the last thousandth.
    fn the_measure_snaps_to_the_corners() {
        let mut s = build::empty_sketch();
        // away from the origin, which is a place to snap to as well and would answer instead of a corner
        build::draw(&mut s, "tb-rect-hint", &[(10.0, 10.0), (50.0, 40.0)]);
        let (dim, field) = (s.word("tb-dim-hint"), s.word("f-number-or-expr"));
        s.press_hint(&dim);
        s.click_on_sketch(30.0, 10.0);
        s.click_on_sketch(30.0, 4.0); // not on the X axis, which would be taken as a second reference
        s.fill_hinted(&field, "40.37").key(Key::Enter);
        let sk = s.document().sketches[0].clone();
        let (w, h) = (sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]);
        assert!((w - 40.37).abs() < 1e-6, "the side was not set to 40.37: the rectangle is {w} across");
        let hint = s.word("tb-measure-hint");
        s.press_hint(&hint);
        s.click_on_sketch(sk.min[0] + 0.3, sk.min[1] + 0.3);
        s.click_on_sketch(sk.max[0] - 0.3, sk.max[1] - 0.3);
        let want = [format!("{:.3}", w.hypot(h)), format!("{:.3}", w), format!("{:.3}", h)];
        assert!(says_all(&mut s, &want.iter().map(String::as_str).collect::<Vec<_>>()), "the clicks did not snap to the corners of the {w} by {h} rectangle: the status line says {:?}", s.status());
    }
}

probe! {
    /// A THIRD CLICK STARTS A NEW MEASUREMENT instead of adding to the old one.
    fn a_third_click_starts_a_new_measurement() {
        let mut s = a_rectangle_and_the_measure();
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(40.0, 30.0);
        s.click_on_sketch(0.0, 30.0);
        s.click_on_sketch(40.0, 30.0);
        assert!(says_all(&mut s, &["40.000", "0.000"]), "the new measurement along the top side is 40 with no rise, and the status line says {:?}", s.status());
    }
}

probe! {
    /// ESC LEAVES THE MEASURE: the program says so and the tool is out of hand.
    fn esc_leaves_the_measure() {
        let mut s = a_rectangle_and_the_measure();
        s.click_on_sketch(0.0, 0.0);
        s.key(Key::Escape);
        let said = s.status();
        assert!(said == s.word("in-measure-cancelled") || said == s.word("in-measure-off"), "nothing says the measure is left: the status line says {said:?}");
        let arrow = s.word("tool-select");
        assert!(s.in_hand().contains(&arrow), "the measure is still in hand: the bar says {:?}", s.in_hand());
    }
}

probe! {
    /// THE MEASURE CHANGES NOTHING: the sketch after measuring is the sketch before it.
    fn the_measure_changes_nothing() {
        let mut s = a_rectangle_and_the_measure();
        let before = s.document();
        s.click_on_sketch(0.0, 0.0);
        s.click_on_sketch(40.0, 30.0);
        assert!(s.document() == before, "measuring changed the document");
    }
}
