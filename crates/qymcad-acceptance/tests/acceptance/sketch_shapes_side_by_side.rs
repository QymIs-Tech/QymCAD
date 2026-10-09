//! SHAPES DRAWN SIDE BY SIDE, sharing sides and corners, worked on by the tools as a person works them - the auto
//! constraints on: two rectangles beside each other and a square on top of the right one, the corner of the left one
//! rounded where the three meet; then Extend on the side the two rectangles share, its end the end of the rounding, and
//! a line drawn from the other end of the rounding to the corner.
//!
//! Reported behaviour: Extend on the shared side moved the whole drawing and extended nothing, with no word why; a line
//! drawn after it from the end of the rounding made the lines of the left rectangle vanish and left the points.
use qymcad::{Key, Kind, Modifiers, Picture, Pos2, Session, Widget};
use qymcad_acceptance::build::{draw, into_the_first_part, open_project};
use qymcad_acceptance::probe;

/// A sketch on XY with the auto constraints on, as a person has them.
fn a_sketch() -> Session {
    let mut s = Session::start();
    into_the_first_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    s
}

/// The field on the sheet nearest to `at`.
fn field_near(s: &mut Session, at: Pos2) -> Widget {
    let sheet = s.canvas();
    let fields: Vec<Widget> = s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && sheet.contains(w.rect.center())).collect();
    fields.iter().min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at))).cloned().unwrap_or_else(|| panic!("no little box popped up near {at:?}"))
}

/// The corner where the lines through `one` and `other` meet rounded by `r` with the sketch fillet: each line picked with
/// Shift, the radius typed at the corner.
fn round(s: &mut Session, one: (f64, f64), other: (f64, f64), r: &str) {
    let hint = s.word("tb-fillet-sketch-hint");
    s.press_hint(&hint);
    for p in [one, other] {
        let at = s.on_sketch(p.0, p.1);
        s.click_with(at, qymcad::PointerButton::Primary, Modifiers::SHIFT);
    }
    let at = s.on_sketch(other.0, other.1);
    let field = field_near(s, at);
    s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(r);
    s.key(Key::Enter);
    s.key(Key::Escape).key(Key::Escape);
}

/// Whether a curve is drawn through `at` on the picture: a pixel within 2 of it brighter than the sheet and its grid.
fn drawn_at(pic: &Picture, at: Pos2) -> bool {
    let (cx, cy) = (at.x.round() as i64, at.y.round() as i64);
    (-2..=2).any(|dy| {
        (-2..=2).any(|dx| {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x as usize >= pic.width || y as usize >= pic.height {
                return false;
            }
            let k = (y as usize * pic.width + x as usize) * 4;
            pic.rgba[k..k + 3].iter().any(|c| *c > 90)
        })
    })
}

/// Extend taken, the line through `on` clicked, and clicked again to extend it: answers the status line, and the places
/// of the points before and after.
fn extend(s: &mut Session, sketch: usize, on: (f64, f64)) -> (String, Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let before = s.document().sketches[sketch].places.clone();
    let hint = s.word("tb-extend-hint");
    s.press_hint(&hint);
    s.click_on_sketch(on.0, on.1);
    s.click_on_sketch(on.0, on.1);
    let said = s.status();
    let after = s.document().sketches[sketch].places.clone();
    s.key(Key::Escape).key(Key::Escape);
    (said, before, after)
}

/// What is wrong once the line is drawn from `from` to `to`: the places of sketch `sketch` the picture no longer
/// shows a curve through, and the curves lost.
fn a_line_drawn(s: &mut Session, sketch: usize, from: (f64, f64), to: (f64, f64), shown: &[(f64, f64)]) -> Vec<String> {
    let curves = |s: &mut Session| {
        let k = &s.document().sketches[sketch];
        k.lines + k.arcs
    };
    let before = curves(s);
    draw(s, "tb-line-hint", &[from, to]);
    let mut failures = Vec::new();
    if curves(s) != before + 1 {
        failures.push(format!("a line drawn left {} curves, there were {before}", curves(s)));
    }
    // each place brought into view first, as the eye goes to it, then the picture taken: bringing it into view may move
    // the sheet
    let gone: Vec<(f64, f64)> = shown
        .iter()
        .copied()
        .filter(|p| {
            let at = s.on_sketch(p.0, p.1);
            !drawn_at(&s.snapshot(), at)
        })
        .collect();
    if !gone.is_empty() {
        failures.push(format!("a line drawn from {from:?} to {to:?}: nothing is drawn any more at {gone:?}"));
    }
    failures
}

probe! {
    /// THE CHAIN OF THE REPORT, drawn by hand: the left rectangle 20 x 60, the right one beside it, the square 20 x 20 on
    /// top of the right one; the top right corner of the left one rounded 12 by its two lines. Extend on the shared side,
    /// whose upper end is the end of the rounding: nothing moves, and the status line says the end is joined. A line from
    /// the other end of the rounding to the corner: one curve more, and every side and the rounding of the left
    /// rectangle still drawn.
    fn shapes_side_by_side_keep_their_lines_under_extend_and_a_new_line() {
        let mut s = a_sketch();
        for (a, b) in [((0.0, 0.0), (20.0, 60.0)), ((20.0, 0.0), (40.0, 60.0)), ((20.0, 60.0), (40.0, 80.0))] {
            draw(&mut s, "tb-rect-hint", &[a, b]);
        }
        round(&mut s, (10.0, 60.0), (20.0, 30.0), "12");
        let mut failures = Vec::new();
        let (said, before, after) = extend(&mut s, 0, (20.0, 30.0));
        if said != s.word("sk-extend-end-joined") || before != after {
            failures.push(format!("Extend on the side whose end is the end of the rounding: the status line says {said:?}, the points moved {}", before != after));
        }
        let arc = (8.0 + 12.0 * std::f64::consts::FRAC_PI_4.cos(), 48.0 + 12.0 * std::f64::consts::FRAC_PI_4.sin());
        failures.extend(a_line_drawn(&mut s, 0, (8.0, 60.0), (20.0, 60.0), &[(0.0, 30.0), (10.0, 0.0), (4.0, 60.0), arc, (30.0, 0.0), (40.0, 30.0), (30.0, 80.0)]));
        assert!(failures.is_empty(), "shapes side by side:\n{}", failures.join("\n"));
    }
}

probe! {
    /// THE SAME ON THE DOCUMENT OF THE REPORT: its first sketch opened from the tree, Extend on the side the rectangles
    /// share, then a line from the end of the rounding of the left one to the corner.
    fn the_reported_sketch_keeps_its_lines_under_extend_and_a_new_line() {
        let Some(path) = qymcad_acceptance::private_sample("sketch_line_test.qcad") else { return };
        let mut s = Session::start();
        open_project(&mut s, &path);
        let part = s.find("Part 1", qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("no part in the tree: {:?}", s.words()));
        s.double_click(part.center());
        // the first sketch of the document, the one left as built
        let name = s.document().sketches.first().map(|k| k.name.clone()).expect("a sketch");
        let row = s.find(&name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("no {name} in the tree: {:?}", s.words()));
        s.double_click(row.center());
        let k = s.document().sketches.iter().position(|k| k.name == name).expect("the sketch");
        let mut failures = Vec::new();
        let (said, before, after) = extend(&mut s, k, (-75.0, 70.0));
        if said != s.word("sk-extend-end-joined") || before != after {
            failures.push(format!("Extend on the shared side: the status line says {said:?}, the points moved {}", before != after));
        }
        let arc = (-85.0 + 10.0 * std::f64::consts::FRAC_PI_4.cos(), 115.0 + 10.0 * std::f64::consts::FRAC_PI_4.sin());
        failures.extend(a_line_drawn(&mut s, k, (-85.0, 125.0), (-75.0, 125.0), &[(-125.0, 70.0), (-100.0, 25.0), (-100.0, 125.0), arc, (-40.0, 70.0)]));
        assert!(failures.is_empty(), "the reported sketch:\n{}", failures.join("\n"));
    }
}
