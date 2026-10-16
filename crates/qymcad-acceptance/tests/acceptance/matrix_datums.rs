//! THE DATUMS ACROSS WHAT THEY ARE MADE FROM: a plane off a plane of the world and off the faces of bodies - flat,
//! slanted, round-topped - at offsets out, in, small and large; an axis along straight edges and round faces - a
//! cylinder, a hole, a boss, a rounding; a point at corners and at coordinates. Each is read back from the document
//! and must stand where the geometry puts it: a plane through the right point square to the right normal, an axis
//! through the right point along the right way, a point at the right place.
use qymcad::{Datum, Key, Session};
use qymcad_acceptance::bodies::{BLOCK, BOSSED, CHAMFERED, CYLINDER, EMPTY, HOLED, ROUNDED};
use qymcad_acceptance::matrix::{show, spot, Body, Spot};
use qymcad_acceptance::probe;

/// What a datum must be.
enum Want {
    /// A plane through `through`, square to `normal` (either way).
    Plane { through: [f64; 3], normal: [f64; 3] },
    /// An axis through `through`, along `along` (either way).
    Axis { through: [f64; 3], along: [f64; 3] },
    /// A point at `at`.
    Point { at: [f64; 3] },
    /// No datum: the tool refuses in words.
    Nothing,
}

struct Made {
    body: &'static Body,
    make: fn(&mut Session),
    want: Want,
    what: &'static str,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt().max(1e-12);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// What is wrong with `d` against `want`, if anything.
fn judged(d: &Datum, want: &Want) -> Option<String> {
    let along = |dir: [f64; 3], want: [f64; 3]| (dot(unit(dir), unit(want)).abs() - 1.0).abs() < 1e-6;
    match *want {
        Want::Plane { through, normal } => {
            let off = dot(sub(through, d.at), unit(d.dir));
            (d.kind != "plane" || !along(d.dir, normal) || off.abs() > 1e-6)
                .then(|| format!("a {} at {:?} facing {:?}, {off} off the point {through:?}, not a plane square to {normal:?}", d.kind, d.at, d.dir))
        }
        Want::Axis { through, along: way } => {
            let v = sub(through, d.at);
            let u = unit(d.dir);
            let t = dot(v, u);
            let apart = dot(v, v) - t * t;
            (d.kind != "axis" || !along(d.dir, way) || apart.max(0.0).sqrt() > 1e-6)
                .then(|| format!("a {} at {:?} along {:?}, {} from {through:?}, not an axis along {way:?}", d.kind, d.at, d.dir, apart.max(0.0).sqrt()))
        }
        Want::Point { at } => (d.kind != "point" || dot(sub(at, d.at), sub(at, d.at)).sqrt() > 1e-6).then(|| format!("a {} at {:?}, not a point at {at:?}", d.kind, d.at)),
        Want::Nothing => Some(format!("a {} at {:?} was made where nothing should be", d.kind, d.at)),
    }
}

/// Run every datum in a session of its own and fail with all that do not stand as they must.
fn run_all(all: Vec<Made>) {
    let mut failed = Vec::new();
    for c in &all {
        let problem = qymcad_acceptance::refusal(|| {
            let mut s = Session::start();
            (c.body.build)(&mut s);
            let before = s.document().datums.len();
            let status = s.status();
            (c.make)(&mut s);
            let datums = s.document().datums;
            if matches!(c.want, Want::Nothing) {
                assert!(datums.len() == before, "{} datums were made where the tool should refuse: {:?}", datums.len() - before, &datums[before..]);
                assert!(s.status() != status, "refused without a word: the status line still says {status:?}");
                return;
            }
            assert!(datums.len() == before + 1, "{} datums were made, not one: {:?}", datums.len() - before, &datums[before..]);
            if let Some(p) = judged(&datums[before], &c.want) {
                panic!("{p}");
            }
        });
        if !problem.is_empty() {
            failed.push(format!("{} / {}: {problem}", c.body.name, c.what));
        }
    }
    assert!(failed.is_empty(), "{} of {} datums did not stand as they must:\n{}", failed.len(), all.len(), failed.join("\n"));
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// A datum plane `dist` off what stands at the spot, typed into the field that opens beside the click.
fn plane_off(s: &mut Session, at: Spot, dist: &str) {
    s.key(Key::Escape);
    take(s, "g-datum-plane-hint");
    let p = show(s, at);
    let click = s.in_space(p);
    s.click(click);
    // the offset typed into its own field, by its caption: the plane has its tilts and its turn beside it
    let offset = s.word("f-offset");
    s.fill(&offset, dist).key(Key::Enter);
}

/// A datum axis along the edge or the round face at the spot.
fn axis_on(s: &mut Session, at: Spot, face: bool) {
    s.key(Key::Escape);
    take(s, "g-datum-axis-hint");
    let p = show(s, at);
    let click = if face { s.face_at(p) } else { s.edge_at(p) };
    s.click(click);
    s.key(Key::Enter);
}

/// A datum axis through the corners at `a` and `b`, clicked in turn with "2 points" pressed.
fn axis_through(s: &mut Session, a: [f64; 3], b: [f64; 3]) {
    s.key(Key::Escape);
    take(s, "g-datum-axis-hint");
    let word = s.word("cmd-two-points");
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
    for at in [a, b] {
        let p = show(s, spot(at, at));
        let corner = s.vertex_at(p);
        s.click(corner);
    }
    s.key(Key::Enter);
}

/// A datum axis typed by hand: its origin `o` and its way `d`, into the fields of "By hand".
fn axis_by_hand(s: &mut Session, o: [f64; 3], d: [f64; 3]) {
    s.key(Key::Escape);
    take(s, "g-datum-axis-hint");
    let word = s.word("cmd-manual");
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
    for (caption, v) in ["O.x", "O.y", "O.z", "Dir.x", "Dir.y", "Dir.z"].into_iter().zip(o.into_iter().chain(d)) {
        s.fill(caption, &format!("{v}"));
    }
    s.key(Key::Enter);
}

/// A datum point on the corner at `at`.
fn point_on(s: &mut Session, at: [f64; 3]) {
    s.key(Key::Escape);
    take(s, "g-datum-point-hint");
    let word = s.word("cmd-to-vertex");
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
    let p = show(s, spot(at, at));
    let corner = s.vertex_at(p);
    s.click(corner);
    s.key(Key::Enter);
}

/// A datum point typed by its coordinates.
fn point_at(s: &mut Session, at: [f64; 3]) {
    s.key(Key::Escape);
    take(s, "g-datum-point-hint");
    let word = s.word("cmd-coordinates");
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
    for (caption, v) in ["X", "Y", "Z"].into_iter().zip(at) {
        s.fill(caption, &format!("{v}"));
    }
    s.key(Key::Enter);
}

const ORIGIN: Spot = spot([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
const TOP: Spot = spot([20.0, 15.0, 10.0], [10.0, 8.0, 10.0]);
const RIGHT: Spot = spot([40.0, 15.0, 5.0], [40.0, 8.0, 5.0]);

fn m(body: &'static Body, make: fn(&mut Session), want: Want, what: &'static str) -> Made {
    Made { body, make, want, what }
}

probe! {
    budget = 1800;
    /// A PLANE off the table and off the faces of a block, out and in, near and far; off a slanted face; off the top
    /// of a cylinder.
    fn planes_off_planes_and_faces() {
        let up = [0.0, 0.0, 1.0];
        let h = std::f64::consts::FRAC_1_SQRT_2;
        run_all(vec![
            m(&EMPTY, |s| plane_off(s, ORIGIN, "20"), Want::Plane { through: [0.0, 0.0, 20.0], normal: up }, "20 over the table"),
            m(&EMPTY, |s| plane_off(s, ORIGIN, "0.1"), Want::Plane { through: [0.0, 0.0, 0.1], normal: up }, "0.1 over the table"),
            m(&EMPTY, |s| plane_off(s, ORIGIN, "-15"), Want::Plane { through: [0.0, 0.0, -15.0], normal: up }, "15 under the table"),
            m(&EMPTY, |s| plane_off(s, ORIGIN, "10000"), Want::Plane { through: [0.0, 0.0, 10000.0], normal: up }, "10000 over the table"),
            m(&BLOCK, |s| plane_off(s, TOP, "5"), Want::Plane { through: [0.0, 0.0, 15.0], normal: up }, "5 out of the top of the block"),
            m(&BLOCK, |s| plane_off(s, TOP, "-5"), Want::Plane { through: [0.0, 0.0, 5.0], normal: up }, "5 into the top of the block"),
            // (the front of the block lies in the plane XZ of the world, which a click there takes as well: the top of a
            // boss stands clear of every plane of the world)
            m(&BOSSED, |s| plane_off(s, spot([20.0, 15.0, 15.0], [18.0, 15.0, 15.0]), "2"), Want::Plane { through: [0.0, 0.0, 17.0], normal: up }, "2 out of the top of the boss"),
            m(&BLOCK, |s| plane_off(s, RIGHT, "2"), Want::Plane { through: [42.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0] }, "2 out of the right of the block"),
            m(&CYLINDER, |s| plane_off(s, TOP, "1"), Want::Plane { through: [0.0, 0.0, 11.0], normal: up }, "1 out of the top of the cylinder"),
            // the chamfer face runs through (20, 0, 8) and (20, 2, 10), facing out along (0, -1, 1)
            m(
                &CHAMFERED,
                |s| plane_off(s, qymcad_acceptance::matrix::short([20.0, 1.0, 9.0], [10.0, 1.0, 9.0], [20.0, 2.0, 10.0]), "1"),
                Want::Plane { through: [20.0, 1.0 - h, 9.0 + h], normal: [0.0, -1.0, 1.0] },
                "1 out of the slanted face of the chamfer",
            ),
        ]);
    }
}

probe! {
    budget = 1800;
    /// AN AXIS along the straight edges of a block, and along the round faces of a cylinder, a hole, a boss and a
    /// rounding - their own axes.
    fn axes_along_edges_and_round_faces() {
        let up = [0.0, 0.0, 1.0];
        run_all(vec![
            m(&BLOCK, |s| axis_on(s, spot([20.0, 0.0, 10.0], [10.0, 0.0, 10.0]), false), Want::Axis { through: [0.0, 0.0, 10.0], along: [1.0, 0.0, 0.0] }, "the top front edge"),
            m(&BLOCK, |s| axis_on(s, spot([40.0, 0.0, 5.0], [40.0, 0.0, 2.5]), false), Want::Axis { through: [40.0, 0.0, 0.0], along: up }, "the upright front right edge"),
            m(&CYLINDER, |s| axis_on(s, spot([27.071, 7.929, 5.0], [27.071, 7.929, 3.0]), true), Want::Axis { through: [20.0, 15.0, 0.0], along: up }, "the side of the cylinder"),
            m(&HOLED, |s| axis_on(s, spot([20.0, 20.0, 7.5], [21.71, 19.7, 7.5]), true), Want::Axis { through: [20.0, 15.0, 0.0], along: up }, "the wall of the hole"),
            m(&BOSSED, |s| axis_on(s, spot([24.243, 10.757, 12.5], [24.243, 10.757, 11.5]), true), Want::Axis { through: [20.0, 15.0, 0.0], along: up }, "the side of the boss"),
            // the rounding of 2 along the top front edge turns about the line y = 2, z = 8
            m(
                &ROUNDED,
                |s| axis_on(s, qymcad_acceptance::matrix::short([20.0, 0.586, 9.414], [10.0, 0.586, 9.414], [20.0, 2.0, 10.0]), true),
                Want::Axis { through: [0.0, 2.0, 8.0], along: [1.0, 0.0, 0.0] },
                "the face of the rounding",
            ),
        ]);
    }
}

probe! {
    budget = 1800;
    /// A POINT at the corners of a block and at coordinates typed - plain, below zero, far.
    fn points_at_corners_and_coordinates() {
        run_all(vec![
            m(&BLOCK, |s| point_on(s, [40.0, 30.0, 10.0]), Want::Point { at: [40.0, 30.0, 10.0] }, "the back right top corner"),
            m(&BLOCK, |s| point_on(s, [40.0, 0.0, 0.0]), Want::Point { at: [40.0, 0.0, 0.0] }, "the front right bottom corner"),
            m(&EMPTY, |s| point_at(s, [5.0, 6.0, 7.0]), Want::Point { at: [5.0, 6.0, 7.0] }, "(5, 6, 7)"),
            m(&EMPTY, |s| point_at(s, [-5.0, -0.5, -12.25]), Want::Point { at: [-5.0, -0.5, -12.25] }, "(-5, -0.5, -12.25)"),
            m(&EMPTY, |s| point_at(s, [1000.0, 0.0, 0.0]), Want::Point { at: [1000.0, 0.0, 0.0] }, "(1000, 0, 0)"),
        ]);
    }
}

probe! {
    budget = 1800;
    /// AN AXIS THROUGH TWO CORNERS and AN AXIS TYPED BY HAND: across the top of a block, up its edge; through a point
    /// along a way, slanted; the same corner twice and a way of nothing refused.
    fn axes_through_points_and_by_hand() {
        run_all(vec![
            m(&BLOCK, |s| axis_through(s, [0.0, 0.0, 10.0], [40.0, 30.0, 10.0]), Want::Axis { through: [0.0, 0.0, 10.0], along: [40.0, 30.0, 0.0] }, "two points across the top"),
            m(&BLOCK, |s| axis_through(s, [40.0, 0.0, 0.0], [40.0, 0.0, 10.0]), Want::Axis { through: [40.0, 0.0, 0.0], along: [0.0, 0.0, 1.0] }, "two points up the front right edge"),
            m(&BLOCK, |s| axis_through(s, [40.0, 0.0, 10.0], [40.0, 0.0, 10.0]), Want::Nothing, "the same corner twice"),
            m(&EMPTY, |s| axis_by_hand(s, [5.0, 5.0, 5.0], [0.0, 1.0, 0.0]), Want::Axis { through: [5.0, 5.0, 5.0], along: [0.0, 1.0, 0.0] }, "by hand through (5, 5, 5) along Y"),
            m(&EMPTY, |s| axis_by_hand(s, [-10.0, 0.0, 2.5], [1.0, 1.0, 1.0]), Want::Axis { through: [-10.0, 0.0, 2.5], along: [1.0, 1.0, 1.0] }, "by hand through (-10, 0, 2.5), slanted"),
            m(&EMPTY, |s| axis_by_hand(s, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]), Want::Nothing, "by hand along a way of nothing"),
        ]);
    }
}
