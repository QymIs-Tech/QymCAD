//! A CORNER AT A CROSSING: two lines through each other share no point, yet they make four corners where they cross. A
//! click of the sketch fillet or chamfer inside one of them, by the crossing, cuts both lines there, rounds or cuts that
//! corner, and keeps the other two halves as lines.
//!
//! Reported behaviour: "a sketch fillet or chamfer cannot be put where two lines cross - a click at the crossing picks no
//! corner, and nothing is made".
use qymcad::{Key, Kind, Modifiers, Session, Widget};
use qymcad_acceptance::build::{empty_sketch, line};
use qymcad_acceptance::probe;

/// The field on the sheet nearest to `at`.
fn field_near(s: &mut Session, at: qymcad::Pos2) -> Option<Widget> {
    let sheet = s.canvas();
    s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && sheet.contains(w.rect.center())).min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at)))
}

/// A CUT OF A CORNER as a person asks for it: the tool by its hint, the mode of a chamfer by its word on the bar, the
/// values typed, and the arcs it makes. Each cuts 5 along both lines of a square corner.
#[derive(Clone, Copy)]
struct Cut {
    hint: &'static str,
    mode: Option<&'static str>,
    first: &'static str,
    second: Option<&'static str>,
    arcs: usize,
}

/// THE FOUR QUARTERS OF A CROSS, by the sign of each axis.
const QUARTERS: [(f64, f64); 4] = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];

/// WHERE THE CROSS STANDS: off the origin, and on it.
const AT: [(f64, f64); 2] = [(5.0, 10.0), (0.0, 0.0)];

/// A cross about `o`: the line 40 long across it and the line 40 long up through it, drawn apart, with no point in
/// common.
fn a_cross(o: (f64, f64)) -> Session {
    let mut s = empty_sketch();
    line(&mut s, (o.0 - 20.0, o.1), (o.0 + 20.0, o.1));
    line(&mut s, (o.0, o.1 - 20.0), (o.0, o.1 + 20.0));
    s
}

/// The tool whose hint is `hint` clicked at `at`, inside a corner by the crossing, and `value` typed at it: answers
/// what is wrong after, with the corner of the quarter `quarter` (+1 or -1 along X, along Y) cut 5 along each line.
fn cut_in(cut: Cut, o: (f64, f64), quarter: (f64, f64)) -> Option<String> {
    let mut s = a_cross(o);
    cut_in_session(&mut s, cut, o, quarter).or_else(|| {
        // the far ends of the cross stay where they were
        let sk = s.document().sketches[0].clone();
        let gone: Vec<(f64, f64)> =
            [(-20.0, 0.0), (20.0, 0.0), (0.0, -20.0), (0.0, 20.0)].into_iter().filter(|&(x, y)| !sk.places.iter().any(|p| (p[0] - o.0 - x).abs() < 1e-3 && (p[1] - o.1 - y).abs() < 1e-3)).collect();
        (!gone.is_empty()).then(|| format!("{} at {o:?} in the quarter {quarter:?}: the far ends of the cross are gone from {gone:?}", cut.hint))
    })
}

/// The same on the cross of session `s`.
fn cut_in_session(s: &mut Session, cut: Cut, o: (f64, f64), quarter: (f64, f64)) -> Option<String> {
    let Cut { hint, mode, first, second, arcs } = cut;
    let word = s.word(hint);
    s.press_hint(&word);
    if let Some(mode) = mode {
        let word = s.word(mode);
        s.press_word_near(&word, qymcad::pos2(400.0, 0.0));
    }
    let at = s.on_sketch(o.0 + quarter.0, o.1 + quarter.1);
    s.click(at);
    // the crossing has four corners to give: a second click in the quarter fixes the corner there
    let fix = s.word("sk-corner-click-to-fix");
    if s.status() != fix {
        return Some(format!("{hint} at {o:?} in the quarter {quarter:?}: a click at the crossing did not ask for the side, the status says {:?}", s.status()));
    }
    let side = s.on_sketch(o.0 + 1.5 * quarter.0, o.1 + 1.5 * quarter.1);
    s.click(side);
    if field_near(s, at).is_none() {
        return Some(format!("{hint} at {o:?} in the quarter {quarter:?}: no field opened, the status says {:?}", s.status()));
    }
    // the click on the corner puts the caret in its first field, as a person sees it
    s.chord(Modifiers::COMMAND, Key::A).type_text(first);
    if let Some(second) = second {
        s.key(Key::Tab).chord(Modifiers::COMMAND, Key::A).type_text(second);
    }
    s.key(Key::Enter);
    let sk = s.document().sketches[0].clone();
    let at = |x: f64, y: f64| sk.places.iter().any(|p| (p[0] - o.0 - x).abs() < 1e-3 && (p[1] - o.1 - y).abs() < 1e-3);
    // the corner cut 5 along each of its lines, the other two halves meeting at the crossing
    let (qx, qy) = quarter;
    let want = [(5.0 * qx, 0.0), (0.0, 5.0 * qy), (0.0, 0.0)];
    let missing: Vec<(f64, f64)> = want.into_iter().filter(|&(x, y)| !at(x, y)).collect();
    (!missing.is_empty() || sk.arcs != arcs || sk.redundant != 0).then(|| {
        format!("{hint} at {o:?} in the quarter {quarter:?}: nothing stands at {missing:?}, {} arcs, {} lines, {} redundant; the sketch shows {:?}", sk.arcs, sk.lines, sk.redundant, sk.places)
    })
}

probe! {
    /// A FILLET IN EACH QUARTER OF A CROSS: the click inside the quarter by the crossing, 5 typed - that corner is
    /// rounded 5 on both lines, the far ends stay, and the other two halves still meet at the crossing.
    fn a_fillet_is_put_in_each_corner_of_a_crossing() {
        let fillet = Cut { hint: "tb-fillet-sketch-hint", mode: None, first: "5", second: None, arcs: 1 };
        let failures: Vec<String> = AT.into_iter().flat_map(|o| QUARTERS.into_iter().filter_map(move |q| cut_in(fillet, o, q))).collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

probe! {
    /// A CHAMFER OF EACH KIND IN EACH QUARTER OF A CROSS: a cut of 5 * sqrt(2) across a square corner, two legs of 5, a
    /// leg of 5 at 45 deg - a line across the corner, 5 along each line.
    fn a_chamfer_of_each_kind_is_put_in_each_corner_of_a_crossing() {
        let kinds = [
            Cut { hint: "tb-chamfer-sketch-hint", mode: Some("cmd-symmetric"), first: "7.0710678", second: None, arcs: 0 },
            Cut { hint: "tb-chamfer-sketch-hint", mode: Some("cmd-two-distances"), first: "5", second: Some("5"), arcs: 0 },
            Cut { hint: "tb-chamfer-sketch-hint", mode: Some("cmd-leg-angle"), first: "5", second: Some("45"), arcs: 0 },
        ];
        let failures: Vec<String> = kinds.into_iter().flat_map(|k| AT.into_iter().flat_map(move |o| QUARTERS.into_iter().filter_map(move |q| cut_in(k, o, q)))).collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

probe! {
    /// THE CUT AT A CROSSING IS PART OF THE CORNER: a fillet put at a crossing is taken back by one Ctrl+Z - two whole
    /// lines again; a click at the crossing left with Esc, before any value, leaves the two lines whole.
    fn the_cut_at_a_crossing_is_taken_back_with_its_corner() {
        let whole = |s: &mut Session| {
            let sk = s.document().sketches[0].clone();
            (sk.lines, sk.arcs, sk.places.len())
        };
        let mut failures = Vec::new();
        let mut s = a_cross(AT[0]);
        let before = whole(&mut s);
        if let Some(e) = cut_in_session(&mut s, Cut { hint: "tb-fillet-sketch-hint", mode: None, first: "5", second: None, arcs: 1 }, AT[0], QUARTERS[0]) {
            failures.push(format!("GUARD: {e}"));
        }
        s.chord(Modifiers::COMMAND, Key::Z);
        if whole(&mut s) != before {
            failures.push(format!("one Ctrl+Z after a fillet at a crossing: lines, arcs, points {:?}, they were {before:?}", whole(&mut s)));
        }
        let mut s = a_cross(AT[0]);
        let word = s.word("tb-fillet-sketch-hint");
        s.press_hint(&word);
        s.click_on_sketch(AT[0].0 + 1.0, AT[0].1 + 1.0);
        s.key(Key::Escape);
        if whole(&mut s) != before {
            failures.push(format!("a click at a crossing left with Esc: lines, arcs, points {:?}, they were {before:?}", whole(&mut s)));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

/// The fields of the bar of options, left to right.
fn bar_fields(s: &mut Session) -> Vec<Widget> {
    let mut fields: Vec<Widget> = s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && w.rect.top() < 120.0).collect();
    fields.sort_by(|a, b| a.rect.left().total_cmp(&b.rect.left()));
    fields
}

/// The line through (x, y) laid out three times by a linear pattern, `step` apart.
fn three_of(s: &mut Session, at: (f64, f64), step: (f64, f64)) {
    qymcad_acceptance::build::pick(s, at.0, at.1, false);
    let hint = s.word("tb-lin-array-hint");
    s.press_hint(&hint);
    let count = s.word("opt-count");
    s.fill(&count, "3");
    let fields = bar_fields(s);
    for (field, v) in fields[1..3].iter().zip([step.0, step.1]) {
        s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(&v.to_string());
    }
    s.key(Key::Enter).key(Key::Enter);
    s.key(Key::Escape).key(Key::Escape);
}

probe! {
    /// A FILLET AT A CROSSING OF A GRID OF TWO PATTERNS: a level line and an upright one, each laid out three times 20
    /// apart; the crossing (20, 20) in the middle of the grid rounded 5 in its upper right quarter. The corner is rounded,
    /// the rest of the grid stays, nothing is redundant.
    fn a_fillet_is_put_at_a_crossing_of_a_grid_of_two_patterns() {
        let mut s = empty_sketch();
        line(&mut s, (-10.0, 0.0), (50.0, 0.0));
        three_of(&mut s, (15.0, 0.0), (0.0, 20.0));
        line(&mut s, (0.0, -10.0), (0.0, 50.0));
        three_of(&mut s, (0.0, 15.0), (20.0, 0.0));
        let grid = s.document().sketches[0].clone();
        assert!(grid.lines == 6, "GUARD: a grid of 3 by 3 lines: {} lines; the sketch shows {:?}", grid.lines, grid.places);
        let e = cut_in_session(&mut s, Cut { hint: "tb-fillet-sketch-hint", mode: None, first: "5", second: None, arcs: 1 }, (20.0, 20.0), QUARTERS[0]);
        let sk = s.document().sketches[0].clone();
        let far = [(-10.0, 0.0), (50.0, 40.0), (40.0, -10.0), (0.0, 50.0)].into_iter().filter(|&(x, y)| !sk.places.iter().any(|p| (p[0] - x).abs() < 1e-3 && (p[1] - y).abs() < 1e-3)).collect::<Vec<_>>();
        assert!(e.is_none() && far.is_empty(), "a fillet at the middle of a grid: {e:?}; the ends of the grid gone from {far:?}");
    }
}

probe! {
    /// TWO CROSSING LINES CHOSEN WITH SHIFT, THE CORNER WHERE THE POINTER IS, FIXED BY A CLICK: the fillet taken, the
    /// level line and the upright one chosen with Shift away from the crossing, the pointer led into a quarter by the
    /// crossing and clicked there, then led away to the opposite side, 5 typed, Enter - the quarter clicked is rounded;
    /// the same for a chamfer. Reported behaviour: "a click on the crossing is needed
    /// first; it must work as well with two lines chosen with Shift, the side of the fillet or chamfer chosen by where
    /// the cursor is, with the preview drawn there".
    fn two_crossing_lines_chosen_with_shift_take_the_corner_the_pointer_is_in() {
        let mut failures = Vec::new();
        for (hint, arcs) in [("tb-fillet-sketch-hint", 1), ("tb-chamfer-sketch-hint", 0)] {
            for q in QUARTERS {
                let o = AT[0];
                let mut s = a_cross(o);
                let word = s.word(hint);
                s.press_hint(&word);
                for p in [(o.0 - 12.0 * q.0, o.1), (o.0, o.1 - 12.0 * q.1)] {
                    let at = s.on_sketch(p.0, p.1);
                    s.click_with(at, qymcad::PointerButton::Primary, Modifiers::SHIFT);
                }
                let pointer = s.on_sketch(o.0 + 2.0 * q.0, o.1 + 2.0 * q.1);
                s.move_to(pointer);
                s.click(pointer);
                // led on to the quarter across: the corner was fixed by the click and stays
                let away = s.on_sketch(o.0 - 2.0 * q.0, o.1 - 2.0 * q.1);
                s.move_to(away);
                s.chord(Modifiers::COMMAND, Key::A).type_text(if arcs == 1 { "5" } else { "7.0710678" });
                s.key(Key::Enter);
                let sk = s.document().sketches[0].clone();
                let at = |x: f64, y: f64| sk.places.iter().any(|p| (p[0] - o.0 - x).abs() < 1e-3 && (p[1] - o.1 - y).abs() < 1e-3);
                let missing: Vec<(f64, f64)> = [(5.0 * q.0, 0.0), (0.0, 5.0 * q.1), (0.0, 0.0)].into_iter().filter(|&(x, y)| !at(x, y)).collect();
                if !missing.is_empty() || sk.arcs != arcs || sk.redundant != 0 {
                    failures.push(format!("{hint}, the lines chosen with Shift, the pointer in the quarter {q:?}: nothing stands at {missing:?}, {} arcs, {} redundant; status {:?}; the sketch shows {:?}", sk.arcs, sk.redundant, s.status(), sk.places));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

probe! {
    /// THE CORNER OF A POINT SEVERAL SHAPES SHARE IS CHOSEN BY A CLICK ON ITS SIDE: two squares touching at their corner
    /// (20, 20), the chamfer clicked at that point - it asks for a side - then clicked by the upper square, the pointer
    /// led to the lower one, the size typed, Enter: the upper square is cut, the lower keeps its sharp corner.
    fn the_corner_of_a_point_of_two_shapes_is_on_the_side_clicked() {
        let mut s = qymcad_acceptance::build::empty_sketch();
        qymcad_acceptance::build::draw(&mut s, "tb-rect-hint", &[(0.0, 0.0), (20.0, 20.0)]);
        qymcad_acceptance::build::draw(&mut s, "tb-rect-hint", &[(20.0, 20.0), (40.0, 40.0)]);
        let word = s.word("tb-chamfer-sketch-hint");
        s.press_hint(&word);
        let mode = s.word("cmd-symmetric");
        s.press_word_near(&mode, qymcad::pos2(400.0, 0.0));
        let at = s.on_sketch(19.8, 19.8);
        s.click(at);
        let asked = s.status() == s.word("sk-corner-click-to-fix");
        let side = s.on_sketch(21.5, 21.5);
        s.click(side);
        let pointer = s.on_sketch(18.5, 18.5);
        s.move_to(pointer);
        s.chord(Modifiers::COMMAND, Key::A).type_text("7.0710678");
        s.key(Key::Enter);
        let sk = s.document().sketches[0].clone();
        let at = |x: f64, y: f64| sk.places.iter().any(|p| (p[0] - x).abs() < 1e-3 && (p[1] - y).abs() < 1e-3);
        assert!(asked && at(25.0, 20.0) && at(20.0, 25.0) && !at(15.0, 20.0) && !at(20.0, 15.0), "the chamfer clicked at the shared point (asked for a side {asked}), then by the upper square, the pointer led to the lower: cut at (25, 20) {}, (20, 25) {}, the lower square cut {}; the sketch shows {:?}", at(25.0, 20.0), at(20.0, 25.0), at(15.0, 20.0) || at(20.0, 15.0), sk.places);
    }
}
