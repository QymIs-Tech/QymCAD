//! WHAT A DRAWING TOOL PUTS DOWN IS TIED TO WHAT IT LANDS ON, construction or not, unless "No ties" on its bar says
//! otherwise: a line from the corner of a rectangle starts on that corner and is held level; with "No ties" ticked it
//! stands where it was clicked, nothing snaps and nothing ties it.
//!
//! Reported behaviour: "construction geometry is built without ties".
use qymcad::{Key, Session};
use qymcad_acceptance::build::{draw, into_the_first_part};
use qymcad_acceptance::probe;

/// A sketch on XY with the auto constraints on, a rectangle (0, 0) - (40, 20) in it.
fn a_rectangle() -> Session {
    let mut s = Session::start();
    into_the_first_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 20.0)]);
    s
}

/// The switch on the bar whose word is `key` set to `on`.
fn switch(s: &mut Session, key: &str, on: bool) {
    let word = s.word(key);
    let w = s.widgets().into_iter().find(|w| w.label.ends_with(&word) && w.checked.is_some()).unwrap_or_else(|| panic!("the bar has no switch {word:?}; on screen: {:?}", s.words()));
    if w.checked != Some(on) {
        s.click(w.rect.center());
    }
}

/// A line drawn from `from` to `to` with the line tool, its bar set by `bar` first: answers how many points and how
/// many constraints the sketch gained.
fn a_line(s: &mut Session, from: (f64, f64), to: (f64, f64), bar: impl FnOnce(&mut Session)) -> (usize, usize) {
    let before = s.document().sketches[0].clone();
    let hint = s.word("tb-line-hint");
    s.press_hint(&hint);
    bar(s);
    s.click_on_sketch(from.0, from.1);
    s.click_on_sketch(to.0, to.1);
    s.key(Key::Escape).key(Key::Escape);
    let after = s.document().sketches[0].clone();
    (after.places.len() - before.places.len(), after.constraints - before.constraints)
}

probe! {
    /// A CONSTRUCTION LINE IS TIED AS ANY LINE IS: "Constr." ticked, a line from the corner (40, 20) to (70, 20) starts
    /// on the corner - one point more, its far end - and is held level - a constraint more.
    fn a_construction_line_is_tied_as_any_line() {
        let mut s = a_rectangle();
        let (points, constraints) = a_line(&mut s, (40.0, 20.0), (70.0, 20.0), |s| switch(s, "opt-construction-short", true));
        assert!(points == 1 && constraints >= 1, "a construction line from the corner of the rectangle: {points} points more (1: it starts on the corner), {constraints} constraints more (it is held level)");
    }
}

probe! {
    /// "NO TIES" PUTS A LINE DOWN AS IT IS CLICKED: ticked, a line clicked 0.3 off the corner (40, 20) starts there and
    /// not on the corner, two points more, no constraint; the same for a construction line.
    fn no_ties_puts_a_line_down_as_it_is_clicked() {
        let mut failures = Vec::new();
        for construction in [false, true] {
            let mut s = a_rectangle();
            let (points, constraints) = a_line(&mut s, (40.3, 20.2), (70.0, 20.7), |s| {
                switch(s, "opt-construction-short", construction);
                switch(s, "opt-no-ties-short", true);
            });
            let sk = s.document().sketches[0].clone();
            let stands = sk.places.iter().any(|p| (p[0] - 40.3).abs() < 1e-6 && (p[1] - 20.2).abs() < 1e-6);
            if points != 2 || constraints != 0 || !stands {
                failures.push(format!("construction {construction}: {points} points more (2), {constraints} constraints more (0), the start stands where it was clicked {stands}; the sketch shows {:?}", sk.places));
            }
        }
        assert!(failures.is_empty(), "no ties:\n{}", failures.join("\n"));
    }
}
