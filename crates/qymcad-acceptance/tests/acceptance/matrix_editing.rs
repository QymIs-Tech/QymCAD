//! EDITING A SKETCH ON FIGURES AS THE TOOLS DRAW THEM - the auto constraints on, as a person has them: a rectangle
//! with its sides level and upright, a circle with its diameter. A corner rounded or cut, all corners rounded, a circle
//! trimmed across its dimension, a contour offset, a dimensioned circle mirrored. Each must do what it says, and none
//! may leave the sketch over-defined.
use qymcad::{Key, Kind, Modifiers, Session};
use qymcad_acceptance::build::{circle, draw, into_the_first_part, line, pick};
use qymcad_acceptance::probe;

struct Edit {
    figure: fn(&mut Session),
    edit: fn(&mut Session),
    /// Lines, arcs and circles the sketch holds after.
    counts: (usize, usize, usize),
    what: &'static str,
}

fn sketched(s: &mut Session, draw: fn(&mut Session)) {
    into_the_first_part(s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    draw(s);
}

fn run_all(all: Vec<Edit>) {
    let mut failed = Vec::new();
    for c in &all {
        let problem = qymcad_acceptance::refusal(|| {
            let mut s = Session::start();
            sketched(&mut s, c.figure);
            (c.edit)(&mut s);
            let sk = s.document().sketches[0].clone();
            let got = (sk.lines, sk.arcs, sk.circles);
            assert!(got == c.counts, "the sketch holds {got:?} lines, arcs and circles, not {:?}; the status line says {:?}", c.counts, s.status());
            // what agrees with what is there already may stay as a reference, when the sketch says so in words
            assert!(
                sk.redundant == 0 || qymcad_acceptance::says_redundant(&mut s),
                "the sketch is left over-defined ({} redundant) without a word; the status line says {:?}",
                sk.redundant,
                s.status()
            );
        });
        if !problem.is_empty() {
            failed.push(format!("{}: {problem}", c.what));
        }
    }
    assert!(failed.is_empty(), "{} of {} edits did not hold:\n{}", failed.len(), all.len(), failed.join("\n"));
}

fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Type `text` into the little box nearest to `at` that the gesture popped up.
fn into_the_box_near(s: &mut Session, at: qymcad::Pos2, text: &str) {
    let fields: Vec<qymcad::Widget> = s.widgets().into_iter().filter(|w| w.kind == Kind::TextField && w.rect.top() > 120.0).collect();
    let field = fields.iter().min_by(|a, b| a.rect.center().distance(at).total_cmp(&b.rect.center().distance(at))).cloned().unwrap_or_else(|| panic!("no little box popped up near {at:?}"));
    s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text(text);
    s.key(Key::Enter);
}

fn rectangle(s: &mut Session) {
    draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
}

fn circle_across_a_line(s: &mut Session) {
    circle(s, (0.0, 0.0), (10.0, 0.0));
    line(s, (-20.0, 0.0), (20.0, 0.0));
}

fn circle_and_axis(s: &mut Session) {
    circle(s, (20.0, 0.0), (25.0, 0.0));
    line(s, (0.0, -20.0), (0.0, 20.0));
}

fn corner(s: &mut Session, hint: &str) {
    take(s, hint);
    let at = s.on_sketch(0.0, 0.0);
    s.click(at);
    into_the_box_near(s, at, "5");
}

fn edit(figure: fn(&mut Session), edit: fn(&mut Session), counts: (usize, usize, usize), what: &'static str) -> Edit {
    Edit { figure, edit, counts, what }
}

probe! {
    budget = 1800;
    /// EDITING FIGURES AS DRAWN: a corner of a rectangle rounded 5 and cut 5, all its corners rounded 5, its contour
    /// offset 5; a dimensioned circle trimmed across a line through it, and mirrored about an upright line.
    fn editing_figures_as_drawn() {
        run_all(vec![
            edit(rectangle, |s| corner(s, "tb-fillet-sketch-hint"), (4, 1, 0), "a corner of the rectangle rounded 5"),
            edit(rectangle, |s| corner(s, "tb-chamfer-sketch-hint"), (5, 0, 0), "a corner of the rectangle cut 5"),
            edit(
                rectangle,
                |s| {
                    take(s, "tb-fillet-all-hint");
                    s.click_on_sketch(20.0, 0.0);
                    let middle = s.canvas().center();
                    into_the_box_near(s, middle, "5");
                },
                (4, 4, 0),
                "all corners of the rectangle rounded 5",
            ),
            edit(
                rectangle,
                |s| {
                    take(s, "tb-offset-hint");
                    let distance = s.word("opt-distance");
                    s.fill(&distance, "5");
                    for (x, y) in [(20.0, 0.0), (40.0, 15.0), (20.0, 30.0), (0.0, 15.0)] {
                        pick(s, x, y, (x, y) != (20.0, 0.0));
                    }
                    // the side of the copy shown by the pointer, inside - the corners of the copy sharp - and Enter
                    // makes it
                    let inside = s.on_sketch(20.0, 15.0);
                    s.move_to(inside);
                    s.key(qymcad::Key::Enter);
                },
                (8, 0, 0),
                "the rectangle offset 5",
            ),
            edit(
                circle_across_a_line,
                |s| {
                    take(s, "tb-trim-hint");
                    s.click_on_sketch(0.0, -10.0);
                },
                (1, 1, 0),
                "the dimensioned circle trimmed below the line through it",
            ),
            edit(
                circle_and_axis,
                |s| {
                    pick(s, 20.0, 5.0, false);
                    take(s, "tb-mirror-sketch-hint");
                    pick(s, 0.0, 10.0, false);
                },
                (1, 0, 2),
                "the dimensioned circle mirrored about an upright line",
            ),
        ]);
    }
}
