//! A BODY MULTIPLIED: reflected in a plane, laid out in a row and round an axis.
//!
//! The block every check starts from is 40 by 30 by 10, holding 12000 mm^3, with a corner at the origin. Each check
//! reads what came of it: how much material the part holds and how far it now reaches.
use qymcad::{Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The block of the first part.
fn a_block() -> Session {
    let mut s = Session::start();
    build::block(&mut s);
    s
}

/// How much material the part holds, and the box round everything in it.
fn all_of_it(s: &mut Session) -> (f64, [f64; 3], [f64; 3]) {
    let bodies: Vec<qymcad::Solid> = s.document().bodies.into_iter().filter(|b| !b.consumed && !b.sheet).collect();
    assert!(!bodies.is_empty(), "the part holds no body");
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for b in &bodies {
        for i in 0..3 {
            min[i] = min[i].min(b.min[i]);
            max[i] = max[i].max(b.max[i]);
        }
    }
    (bodies.iter().map(|b| b.volume).sum(), min, max)
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Type `text` into the field under the caption `key` names.
fn field(s: &mut Session, key: &str, text: &str) {
    let caption = s.word(key);
    s.fill(&caption, text);
}

probe! {
    /// MIRROR: the body is reflected in the plane that was clicked, and the part holds both halves.
    fn a_mirrored_body_stands_opposite_the_original() {
        let mut s = a_block();
        take(&mut s, "tb-mirror-body-hint");
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body); // the body to reflect
        let side = s.face_at([40.0, 15.0, 5.0]); // the face at x = 40, which is the plane to reflect in
        s.click(side);
        s.key(Key::Enter);
        let (volume, min, max) = all_of_it(&mut s);
        assert!((volume - 24000.0).abs() < 1.0, "the block and its reflection hold 24000, and the part holds {volume}");
        assert!((min[0]).abs() < 1e-3 && (max[0] - 80.0).abs() < 1e-3, "the reflection does not stand beyond the face it was reflected in: the part runs from {} to {} across", min[0], max[0]);
    }
}

probe! {
    /// A LINEAR PATTERN: the body is laid out in a row of three, 50 apart.
    fn a_linear_pattern_of_a_body_lays_out_a_row() {
        let mut s = a_block();
        take(&mut s, "tb-lin-array-body-hint");
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body);
        field(&mut s, "cmd-copies", "3");
        field(&mut s, "f-pitch", "50");
        s.key(Key::Enter);
        let (volume, min, max) = all_of_it(&mut s);
        assert!((volume - 36000.0).abs() < 1.0, "three blocks hold 36000, and the part holds {volume}");
        assert!((min[0]).abs() < 1e-3 && (max[0] - 140.0).abs() < 1e-3, "three blocks 50 apart run from 0 to 140, and the part runs from {} to {}", min[0], max[0]);
    }
}

probe! {
    /// A CIRCULAR PATTERN: the body is laid out four times about the upright axis, a quarter turn apart.
    fn a_circular_pattern_of_a_body_lays_out_a_ring() {
        let mut s = a_block();
        take(&mut s, "tb-circ-array-body-hint");
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body);
        field(&mut s, "cmd-copies", "4");
        // the angle the copies spread over stands at the whole turn as the box opens
        s.key(Key::Enter);
        let (volume, min, max) = all_of_it(&mut s);
        assert!((volume - 48000.0).abs() < 1.0, "four blocks hold 48000, and the part holds {volume}");
        // the block runs 0..40 by 0..30 from the origin; turned half a turn it runs -40..0, so the ring is 40 each way
        assert!((max[0] - 40.0).abs() < 1e-3 && (min[0] + 40.0).abs() < 1e-3, "the ring of four does not stand about the origin: it runs from {} to {} across", min[0], max[0]);
    }
}

probe! {
    /// A CIRCULAR PATTERN ABOUT AN AXIS PICKED BY A CLICK: the upright edge at the far corner of the block is clicked as
    /// the axis, and the block turned half a turn about it stands beyond that corner. Reported behaviour: the axis of
    /// a circular pattern could not be picked, in a part or in an assembly.
    fn a_circular_pattern_turns_about_an_edge_clicked_as_its_axis() {
        let mut s = a_block();
        take(&mut s, "tb-circ-array-body-hint");
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body);
        field(&mut s, "cmd-copies", "2");
        let axis = s.word("cmd-axis-world-z");
        s.press_word(&axis); // the axis is picked in the view, not typed
        let edge = s.edge_at([40.0, 30.0, 5.0]);
        s.click(edge);
        s.key(Key::Enter);
        let (volume, min, max) = all_of_it(&mut s);
        assert!((volume - 24000.0).abs() < 1.0, "two blocks hold 24000, and the part holds {volume}; the status line says {:?}", s.status());
        // turned half a turn about the edge at x = 40, y = 30 the block runs 40..80 by 30..60
        assert!((min[0]).abs() < 1e-3 && (max[0] - 80.0).abs() < 1e-3 && (max[1] - 60.0).abs() < 1e-3, "the pair does not stand about the edge clicked: it runs {min:?}..{max:?}");
    }
}

probe! {
    /// A CIRCULAR PATTERN ABOUT AN AXIS THROUGH TWO POINTS CLICKED: the top and bottom corners of the far upright edge
    /// are clicked one after the other, and the block turned half a turn about the line through them stands beyond
    /// that corner, as about the edge itself.
    fn a_circular_pattern_turns_about_two_points_clicked() {
        let mut s = a_block();
        take(&mut s, "tb-circ-array-body-hint");
        let body = s.face_at([20.0, 15.0, 10.0]);
        s.click(body);
        field(&mut s, "cmd-copies", "2");
        let axis = s.word("cmd-axis-world-z");
        s.press_word(&axis);
        for corner in [[40.0, 30.0, 10.0], [40.0, 30.0, 0.0]] {
            let at = s.vertex_at(corner);
            s.click(at);
        }
        s.key(Key::Enter);
        let (volume, min, max) = all_of_it(&mut s);
        assert!((volume - 24000.0).abs() < 1.0, "two blocks hold 24000, and the part holds {volume}; the status line says {:?}", s.status());
        assert!((min[0]).abs() < 1e-3 && (max[0] - 80.0).abs() < 1e-3 && (max[1] - 60.0).abs() < 1e-3, "the pair does not stand about the line through the two corners: it runs {min:?}..{max:?}");
    }
}
