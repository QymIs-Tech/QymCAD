//! ALL TWELVE CONSTRAINTS OF A SKETCH, each on a pair a person would put it on.
//!
//! Every check draws what it needs with the auto constraints turned off - so that the only constraint in the sketch is
//! the one being checked - picks the geometry with the arrow, presses the button of the constraint and then asks the
//! sheet itself what came of it: where the lines, the circles and the points now are. The program's own word ("the
//! constraint is added", with no residual) is read as well, but it is not what decides: the geometry is.
use qymcad::{Session, SketchPick};
use qymcad_acceptance::build::{circle, empty_sketch, line, pick, point};
use qymcad_acceptance::probe;

/// Press the constraint whose hint is `hint` and answer what the program said about it.
fn constrain(s: &mut Session, hint: &str) -> String {
    let hint = s.word(hint);
    s.press_hint(&hint);
    s.status()
}

/// THE CONSTRAINT IS IN THE SKETCH, THE SKETCH SOLVED, AND A DEGREE OF FREEDOM IS GONE: what every constraint owes,
/// whatever it ties. `kind` is the program's own name for the kind, `took` how many degrees of freedom it takes.
fn added(s: &mut Session, said: &str, kind: &str, dof_before: i32, took: i32) {
    let sk = s.document().sketches[0].clone();
    assert!(sk.constraint_kinds.iter().any(|k| k == kind), "the sketch has no {kind} constraint: {:?}", sk.constraint_kinds);
    assert!(said == s.word("sk-constraint-added"), "the constraint did not solve: the status line says {said:?}");
    assert!(sk.dof == dof_before - took, "the {kind} constraint had to take {took} degree(s) of freedom of {dof_before}, and {} are left", sk.dof);
}

/// The line under (x, y) of the sheet, as its two ends.
fn line_at(s: &mut Session, x: f64, y: f64) -> ((f64, f64), (f64, f64)) {
    match s.sketch_under(x, y) {
        Some(SketchPick::Line { from, to }) => (from, to),
        other => panic!("no line lies at ({x}, {y}) of the sheet: {other:?}"),
    }
}

/// The point under (x, y) of the sheet.
fn point_at(s: &mut Session, x: f64, y: f64) -> (f64, f64) {
    match s.sketch_under(x, y) {
        Some(SketchPick::Point { at }) => at,
        other => panic!("no point lies at ({x}, {y}) of the sheet: {other:?}"),
    }
}

/// The direction of a line, from end to end.
fn dir(l: ((f64, f64), (f64, f64))) -> (f64, f64) {
    ((l.1 .0 - l.0 .0), (l.1 .1 - l.0 .1))
}

/// The length of a line.
fn len(l: ((f64, f64), (f64, f64))) -> f64 {
    let (dx, dy) = dir(l);
    dx.hypot(dy)
}

/// How far apart the two nearest of these places are.
fn nearest_pair(places: &[[f64; 2]]) -> f64 {
    let mut nearest = f64::MAX;
    for (i, a) in places.iter().enumerate() {
        for b in places.iter().skip(i + 1) {
            nearest = nearest.min(apart((a[0], a[1]), (b[0], b[1])));
        }
    }
    nearest
}

/// How far apart two places are.
fn apart(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

probe! {
    /// COINCIDENT: the end of one line and the end of another come to stand in one place.
    fn coincident_puts_two_ends_in_one_place() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (30.0, 10.0), (50.0, 10.0));
        let sk = s.document().sketches[0].clone();
        assert!(nearest_pair(&sk.places) > 1.0, "the four ends of the two lines already stand together: {:?}", sk.places);
        pick(&mut s, 20.0, 0.0, false);
        pick(&mut s, 30.0, 10.0, true);
        let said = constrain(&mut s, "con-coincident-hint");
        added(&mut s, &said, "Coincident", sk.dof, 2);
        let places = s.document().sketches[0].places.clone();
        assert!(nearest_pair(&places) < 1e-9, "no two ends stand in one place: the ends of the lines are at {places:?}");
    }
}

probe! {
    /// HORIZONTAL: a slanted line lies down flat.
    fn horizontal_lays_a_line_flat() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 20.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 10.0, false);
        let said = constrain(&mut s, "con-horizontal-hint");
        added(&mut s, &said, "Horizontal", dof, 1);
        let l = line_at(&mut s, 10.0, 10.0);
        assert!(dir(l).1.abs() < 1e-6, "the line is not horizontal: {l:?}");
    }
}

probe! {
    /// VERTICAL: a slanted line stands up straight.
    fn vertical_stands_a_line_up() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 20.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 10.0, false);
        let said = constrain(&mut s, "con-vertical-hint");
        added(&mut s, &said, "Vertical", dof, 1);
        let l = line_at(&mut s, 10.0, 10.0);
        assert!(dir(l).0.abs() < 1e-6, "the line is not vertical: {l:?}");
    }
}

probe! {
    /// PARALLEL: two lines take one direction.
    fn parallel_gives_two_lines_one_direction() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (0.0, 10.0), (20.0, 30.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 10.0, 20.0, true);
        let said = constrain(&mut s, "con-parallel-hint");
        added(&mut s, &said, "Parallel", dof, 1);
        let (a, b) = (dir(line_at(&mut s, 10.0, 0.0)), dir(line_at(&mut s, 10.0, 20.0)));
        let cross = (a.0 * b.1 - a.1 * b.0) / (a.0.hypot(a.1) * b.0.hypot(b.1));
        assert!(cross.abs() < 1e-6, "the lines are not parallel: their directions are {a:?} and {b:?}");
    }
}

probe! {
    /// PERPENDICULAR: two lines meet at a right angle.
    fn perpendicular_squares_two_lines() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (0.0, 10.0), (20.0, 30.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 10.0, 20.0, true);
        let said = constrain(&mut s, "con-perpendicular-hint");
        added(&mut s, &said, "Perpendicular", dof, 1);
        let (a, b) = (dir(line_at(&mut s, 10.0, 0.0)), dir(line_at(&mut s, 10.0, 20.0)));
        let dot = (a.0 * b.0 + a.1 * b.1) / (a.0.hypot(a.1) * b.0.hypot(b.1));
        assert!(dot.abs() < 1e-6, "the lines are not at a right angle: their directions are {a:?} and {b:?}");
    }
}

probe! {
    /// EQUAL: a long line and a short one come to one length.
    fn equal_gives_two_lines_one_length() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (0.0, 10.0), (50.0, 10.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 25.0, 10.0, true);
        let said = constrain(&mut s, "con-equal");
        added(&mut s, &said, "Equal", dof, 1);
        let (a, b) = (len(line_at(&mut s, 10.0, 0.0)), len(line_at(&mut s, 25.0, 10.0)));
        assert!((a - b).abs() < 1e-6, "the lines are not of one length: {a} and {b}");
    }
}

probe! {
    /// COLLINEAR: two lines come to lie on one straight line.
    fn collinear_puts_two_lines_on_one_line() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        line(&mut s, (30.0, 10.0), (50.0, 10.0));
        // the first line is held by both ends, so the second is what has to come onto it
        pick(&mut s, 0.0, 0.0, false);
        pick(&mut s, 20.0, 0.0, true);
        constrain(&mut s, "con-fix");
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 40.0, 10.0, true);
        let said = constrain(&mut s, "con-collinear-hint");
        added(&mut s, &said, "Collinear", dof, 2);
        let (first, second) = (line_at(&mut s, 10.0, 0.0), line_at(&mut s, 40.0, 0.0));
        let (a, b) = (dir(first), dir(second));
        let turn = (a.0 * b.1 - a.1 * b.0) / (a.0.hypot(a.1) * b.0.hypot(b.1));
        let across = ((second.0.0 - first.0.0) * a.1 - (second.0.1 - first.0.1) * a.0) / a.0.hypot(a.1);
        assert!(turn.abs() < 1e-6 && across.abs() < 1e-6, "the lines are not on one straight line: {first:?} and {second:?} (turn {turn}, offset {across})");
    }
}

probe! {
    /// CONCENTRIC: two circles take one centre - a circle of radius 5 inside one of radius 8 is 16 across, and no
    /// wider.
    fn concentric_gives_two_circles_one_centre() {
        let mut s = empty_sketch();
        circle(&mut s, (0.0, 0.0), (5.0, 0.0));
        circle(&mut s, (30.0, 0.0), (38.0, 0.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 5.0, 0.0, false);
        pick(&mut s, 38.0, 0.0, true);
        let said = constrain(&mut s, "con-concentric-hint");
        added(&mut s, &said, "Concentric", dof, 2);
        let sk = s.document().sketches[0].clone();
        let (w, h) = (sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]);
        assert!((w - 16.0).abs() < 1e-3 && (h - 16.0).abs() < 1e-3, "the circles do not share a centre: together they are {w} by {h}, and two circles of radius 5 and 8 about one centre are 16 by 16");
    }
}

probe! {
    /// TANGENT: a line comes to touch a circle - the round shape and the line together are no wider than the circle
    /// itself.
    fn tangent_brings_a_line_to_touch_a_circle() {
        let mut s = empty_sketch();
        circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        line(&mut s, (20.0, -5.0), (20.0, 5.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 0.0, false);
        pick(&mut s, 20.0, 0.0, true);
        let said = constrain(&mut s, "con-tangent-hint");
        added(&mut s, &said, "Tangent", dof, 1);
        let sk = s.document().sketches[0].clone();
        let (w, h) = (sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]);
        assert!((w - h).abs() < 1e-3, "the line does not touch the circle: together they are {w} by {h}, and a line touching a circle adds nothing to its 20 by 20");
    }
}

probe! {
    /// SYMMETRY: two points stand as mirror images about the Y axis - level with each other, and the same distance
    /// from the axis on either side.
    fn symmetry_mirrors_two_points_about_an_axis() {
        let mut s = empty_sketch();
        point(&mut s, (5.0, 10.0));
        point(&mut s, (-25.0, -30.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 5.0, 10.0, false);
        pick(&mut s, -25.0, -30.0, true);
        pick(&mut s, 0.0, 20.0, true); // the Y axis of the sheet
        let said = constrain(&mut s, "con-symmetric-hint");
        added(&mut s, &said, "Symmetric", dof, 2);
        let sk = s.document().sketches[0].clone();
        let (w, h) = (sk.max[0] - sk.min[0], sk.max[1] - sk.min[1]);
        assert!(h < 1e-6, "the two points are not level with each other: they stand {h} apart across");
        assert!((sk.min[0] + sk.max[0]).abs() < 1e-6, "the two points are not the same distance from the axis: they stand at {} and {} ({w} apart)", sk.min[0], sk.max[0]);
    }
}

probe! {
    /// MIDPOINT: a point lands in the middle of a line whose ends are held.
    fn midpoint_puts_a_point_in_the_middle_of_a_line() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        point(&mut s, (10.0, 15.0));
        // the line is held by both ends, so the point is what has to move
        pick(&mut s, 0.0, 0.0, false);
        pick(&mut s, 20.0, 0.0, true);
        constrain(&mut s, "con-fix");
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 10.0, 15.0, false);
        pick(&mut s, 10.0, 0.0, true);
        let said = constrain(&mut s, "con-midpoint-hint");
        added(&mut s, &said, "Midpoint", dof, 2);
        let at = point_at(&mut s, 10.0, 0.0);
        assert!(apart(at, (10.0, 0.0)) < 1e-6, "the point is not in the middle of the line: it is at {at:?}, and the middle is (10, 0)");
    }
}

probe! {
    /// FIX: a point that is held does not follow the mouse.
    fn fix_holds_a_point_where_it_is() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 0.0));
        let dof = s.document().sketches[0].dof;
        pick(&mut s, 20.0, 0.0, false);
        let said = constrain(&mut s, "con-fix");
        added(&mut s, &said, "Fixed", dof, 2);
        s.drag_on_sketch((20.0, 0.0), (22.0, 2.0));
        let at = point_at(&mut s, 20.0, 0.0);
        assert!(apart(at, (20.0, 0.0)) < 1e-6, "the held point followed the mouse: it is at {at:?}");
    }
}

probe! {
    /// THE BUTTON MAY BE PRESSED FIRST AND THE GEOMETRY PICKED AFTER: the program says what it waits for, and the
    /// constraint goes on as soon as the picking is done.
    fn a_constraint_may_be_pressed_before_the_geometry_is_picked() {
        let mut s = empty_sketch();
        line(&mut s, (0.0, 0.0), (20.0, 20.0));
        let dof = s.document().sketches[0].dof;
        let said = constrain(&mut s, "con-horizontal-hint");
        assert!(said == s.word("sk-pick-for-constraint"), "with nothing picked the program says {said:?} instead of asking for the geometry");
        assert!(s.document().sketches[0].constraint_kinds.is_empty(), "something was tied although nothing was picked");
        pick(&mut s, 10.0, 10.0, false);
        let said = s.status();
        added(&mut s, &said, "Horizontal", dof, 1);
        let l = line_at(&mut s, 10.0, 10.0);
        assert!(dir(l).1.abs() < 1e-6, "the line is not horizontal: {l:?}");
    }
}

probe! {
    /// A PAIR THAT CANNOT TAKE THE CONSTRAINT IS REFUSED IN WORDS, and nothing is tied: two points are not parallel,
    /// two lines have no common centre and do not touch, and a midpoint needs a line, not a second point.
    fn a_pair_that_cannot_take_a_constraint_is_refused() {
        let mut problems = Vec::new();
        for (hint, what) in [("con-parallel-hint", "two points"), ("con-concentric-hint", "two lines"), ("con-tangent-hint", "two lines"), ("con-midpoint-hint", "two points")] {
            let mut s = empty_sketch();
            line(&mut s, (0.0, 0.0), (20.0, 0.0));
            line(&mut s, (0.0, 10.0), (20.0, 30.0));
            let (first, second) = if what == "two points" { ((0.0, 0.0), (20.0, 0.0)) } else { ((10.0, 0.0), (10.0, 20.0)) };
            let before = s.document();
            pick(&mut s, first.0, first.1, false);
            pick(&mut s, second.0, second.1, true);
            let said = constrain(&mut s, hint);
            let after = s.document();
            if said == s.word("sk-pick-for-constraint") {
                problems.push(format!("{hint} on {what}: nothing says the pair cannot take it - the program only asks for the geometry again ({said:?})"));
            }
            if after.sketches[0].constraint_kinds != before.sketches[0].constraint_kinds {
                problems.push(format!("{hint} on {what}: the sketch was tied anyway: {:?}", after.sketches[0].constraint_kinds));
            }
        }
        assert!(problems.is_empty(), "a pair that cannot take a constraint:\n{}", problems.join("\n"));
    }
}

/// The places of the sheet.
fn places(s: &mut Session) -> Vec<(f64, f64)> {
    s.document().sketches[0].places.iter().map(|p| (p[0], p[1])).collect()
}

/// A line from `from` to `to`, tangent to a circle about `c`, touches it at `t`: the radius to `t` is square to the line,
/// so `t` is the foot of the centre on the line - the point a tangent line touches at. The circle is free and the solve
/// may carry and size it too, so its centre is read off the sheet and its radius is not assumed.
fn touches_at(c: (f64, f64), t: (f64, f64), from: (f64, f64), to: (f64, f64)) -> bool {
    let (rx, ry) = (t.0 - c.0, t.1 - c.1);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    rx.hypot(ry) > 1.0 && (dx * rx + dy * ry).abs() < 1e-6 * dx.hypot(dy).max(1.0) * rx.hypot(ry)
}

probe! {
    /// TANGENT TO A CIRCLE THE LINE DOES NOT REACH: the line is run on to the circle - its nearer end becomes the point
    /// it touches at - not only turned so that the straight line it lies on touches it. Reported behaviour: "a circle
    /// and a line apart from it, Tangent: the line is not lengthened".
    fn tangent_runs_a_line_apart_from_a_circle_on_to_it() {
        let mut s = empty_sketch();
        circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        line(&mut s, (20.0, -30.0), (40.0, -20.0));
        pick(&mut s, 35.0, -22.5, false);
        pick(&mut s, 10.0, 0.0, true);
        let said = constrain(&mut s, "con-tangent-hint");
        let places = places(&mut s);
        let touched = places.iter().any(|&c| places.iter().any(|&t| t != c && places.iter().any(|&o| o != t && o != c && touches_at(c, t, t, o))));
        assert!(said == s.word("sk-constraint-added") && touched, "the line was not run on to the circle: the status says {said:?}, the sheet holds {places:?}");
    }
}

probe! {
    /// TANGENT WITH THE LINE PICKED AT ITS MIDDLE: the line is carried onto the circle by its middle - the middle is the
    /// point it touches at. Reported behaviour: "with the centre of the line picked, the line is not carried onto the
    /// circle by its middle point".
    fn tangent_carries_a_line_picked_at_its_middle_onto_the_circle_by_it() {
        let mut s = empty_sketch();
        circle(&mut s, (0.0, 0.0), (10.0, 0.0));
        line(&mut s, (20.0, -30.0), (40.0, -20.0));
        pick(&mut s, 30.0, -25.0, false);
        let at_middle = s.status() == s.word("sk-midpoint-picked");
        pick(&mut s, 10.0, 0.0, true);
        let said = constrain(&mut s, "con-tangent-hint");
        let places = places(&mut s);
        let middle_of = |a: (f64, f64), b: (f64, f64), m: (f64, f64)| a != b && (m.0 - (a.0 + b.0) / 2.0).hypot(m.1 - (a.1 + b.1) / 2.0) < 1e-6;
        let by_middle = places.iter().any(|&c| places.iter().any(|&a| places.iter().any(|&b| places.iter().any(|&m| middle_of(a, b, m) && touches_at(c, m, a, b)))));
        assert!(at_middle && said == s.word("sk-constraint-added") && by_middle, "picked at its middle ({at_middle}), Tangent: the line is not carried onto the circle by its middle; the status says {said:?}, the sheet holds {places:?}");
    }
}
