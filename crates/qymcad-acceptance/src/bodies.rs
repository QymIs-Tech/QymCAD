//! THE BODIES THE MATRICES OF THE PART WORK ON, each built through the window as a person builds it: a block, a
//! cylinder, a block with a hole, a chamfer, a rounding, a notch or a boss. Each says in its name what it is, and the
//! matrices work out what their tools must do to it from that.
use qymcad::{Key, Session};

use crate::build;
use crate::matrix::Body;

pub static BLOCK: Body = Body { name: "the block 40 x 30 x 10", build: build::block };
pub static CYLINDER: Body = Body { name: "a cylinder 20 across and 10 tall", build: cylinder };
pub static HOLED: Body = Body { name: "the block with a hole 10 across and 5 deep in its top", build: holed };
pub static THROUGH: Body = Body { name: "the block with a hole 10 across right through it", build: through };
pub static TAPPED: Body = Body { name: "the block with a hole 8.5 across and 8 deep in its top, drilled for M10", build: tapped };
pub static CHAMFERED: Body = Body { name: "the block with its top front edge chamfered 2", build: chamfered };
pub static ROUNDED: Body = Body { name: "the block with its top front edge rounded 2", build: rounded };
pub static NOTCHED: Body = Body { name: "the block with a notch 10 wide and 5 deep cut across its top front edge", build: notched };
pub static BOSSED: Body = Body { name: "the block with a round boss 12 across and 5 tall on its top", build: bossed };

/// Close the sketch and extrude what it holds by the length the tool offers.
fn finish_and_extrude(s: &mut Session) {
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude).key(Key::Enter);
}

/// A cylinder of radius 10 about (20, 15), 10 tall: a circle on XY extruded.
pub fn cylinder(s: &mut Session) {
    build::into_the_first_part(s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    build::circle(s, (20.0, 15.0), (30.0, 15.0));
    finish_and_extrude(s);
}

/// Take the tool whose hint is `hint`, click what `at` gives, type `value` under `caption`, and press Enter.
pub fn apply(s: &mut Session, hint: &str, at: impl FnOnce(&mut Session) -> qymcad::Pos2, caption: &str, value: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
    let p = at(s);
    s.click(p);
    let caption = s.word(caption);
    s.fill(&caption, value);
    s.key(Key::Enter);
}

/// The block with a hole 10 across and 5 deep bored in the middle of its top.
pub fn holed(s: &mut Session) {
    build::block(s);
    let hint = s.word("tb-hole-hint");
    s.press_hint(&hint);
    let top = s.face_at([20.0, 15.0, 10.0]);
    s.click(top);
    let (d, depth) = (s.word("f-diameter"), s.word("f-depth"));
    s.fill(&d, "10");
    s.fill(&depth, "5");
    s.key(Key::Enter);
}

/// The block with a hole 10 across bored right through it from the middle of its top: 12 deep in a block 10 tall.
pub fn through(s: &mut Session) {
    build::block(s);
    let hint = s.word("tb-hole-hint");
    s.press_hint(&hint);
    let top = s.face_at([20.0, 15.0, 10.0]);
    s.click(top);
    let (d, depth) = (s.word("f-diameter"), s.word("f-depth"));
    s.fill(&d, "10");
    s.fill(&depth, "12");
    s.key(Key::Enter);
}

/// The block with a hole 8.5 across and 8 deep bored in the middle of its top - the drill for a thread of M10.
pub fn tapped(s: &mut Session) {
    build::block(s);
    let hint = s.word("tb-hole-hint");
    s.press_hint(&hint);
    let top = s.face_at([20.0, 15.0, 10.0]);
    s.click(top);
    let (d, depth) = (s.word("f-diameter"), s.word("f-depth"));
    s.fill(&d, "8.5");
    s.fill(&depth, "8");
    s.key(Key::Enter);
}

/// The block with its top front edge chamfered 2.
pub fn chamfered(s: &mut Session) {
    build::block(s);
    apply(s, "tb-chamfer-body-hint", |s| s.edge_at([20.0, 0.0, 10.0]), "f-leg", "2");
}

/// The block with its top front edge rounded 2.
pub fn rounded(s: &mut Session) {
    build::block(s);
    apply(s, "tb-fillet-body-hint", |s| s.edge_at([20.0, 0.0, 10.0]), "f-radius", "2");
}

/// Press the word `key` names on the bar of options across the top.
pub fn bar(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 0.0));
}

/// Draw a sketch on the top face of the block with `draw`, then extrude it with the operation of `operation` by
/// `length`. The axes of a sketch on the top face are those of the world, so what is drawn at (x, y) stands at (x, y).
pub fn on_top(s: &mut Session, draw: impl FnOnce(&mut Session), operation: Option<&str>, length: &str) {
    // in the middle of the face, away from its edges and corners: a click near one binds the origin of the sketch to it
    build::sketch_on_face(s, [20.0, 15.0, 10.0]);
    draw(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    if let Some(op) = operation {
        bar(s, op);
    }
    let caption = s.word("f-length");
    s.fill(&caption, length).key(Key::Enter);
}

/// The block with a notch cut across the middle of its top front edge: 10 wide (x from 15 to 25), 10 back (to y =
/// 10) and 5 deep. The top front edge is two pieces now, and the notch has hollow edges along its floor.
pub fn notched(s: &mut Session) {
    build::block(s);
    on_top(s, |s| build::draw(s, "tb-rect-hint", &[(15.0, -2.0), (25.0, 10.0)]), Some("cmd-cut"), "5");
}

/// The block with a round boss of radius 6 about (20, 15) standing 5 tall on its top: a hollow round edge where it
/// meets the top, a round edge on its own top.
pub fn bossed(s: &mut Session) {
    build::block(s);
    on_top(s, |s| build::circle(s, (20.0, 15.0), (26.0, 15.0)), None, "5");
}

// SKETCHES READY FOR A TOOL: the sketch finished, nothing else taken. A sketch in an empty part gives the first body;
// a sketch on the top face of the block works on the block.

pub static EMPTY: Body = Body { name: "an empty part", build: build::into_the_first_part };
pub static RECTANGLE: Body = Body { name: "a rectangle 40 x 30 at the origin, in an empty part", build: rectangle };
pub static CIRCLE: Body = Body { name: "a circle of radius 10 about (20, 15), in an empty part", build: circle };
pub static NESTED: Body = Body { name: "a rectangle 40 x 30 with a circle of radius 5 inside it, in an empty part", build: nested };
pub static TWO_APART: Body = Body { name: "two squares of 10 apart, in an empty part", build: two_apart };
pub static ACROSS_THE_AXIS: Body = Body { name: "a circle of radius 10 about (20, 5), across the X axis, in an empty part", build: across_the_axis };
pub static SQUARE_ON_TOP: Body = Body { name: "the block with a square of 10 drawn on its top, from (10, 10)", build: square_on_top };
pub static OVER_THE_EDGE: Body = Body { name: "the block with a circle of radius 5 drawn on its top over its front edge, about (20, 0)", build: over_the_edge };

/// A sketch on XY of the first part, drawn by `draw` and finished.
fn on_xy(s: &mut Session, draw: impl FnOnce(&mut Session)) {
    build::into_the_first_part(s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    draw(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

/// A sketch on the top face of the block, drawn by `draw` and finished; its axes are the world's.
fn on_the_top(s: &mut Session, draw: impl FnOnce(&mut Session)) {
    build::block(s);
    build::sketch_on_face(s, [20.0, 15.0, 10.0]);
    draw(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

pub fn rectangle(s: &mut Session) {
    on_xy(s, |s| build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]));
}

pub fn circle(s: &mut Session) {
    on_xy(s, |s| build::circle(s, (20.0, 15.0), (30.0, 15.0)));
}

pub fn nested(s: &mut Session) {
    on_xy(s, |s| {
        build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]);
        build::circle(s, (20.0, 15.0), (25.0, 15.0));
    });
}

pub fn two_apart(s: &mut Session) {
    on_xy(s, |s| {
        build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]);
        build::draw(s, "tb-rect-hint", &[(20.0, 0.0), (30.0, 10.0)]);
    });
}

pub fn across_the_axis(s: &mut Session) {
    on_xy(s, |s| build::circle(s, (20.0, 5.0), (30.0, 5.0)));
}

pub fn square_on_top(s: &mut Session) {
    on_the_top(s, |s| build::draw(s, "tb-rect-hint", &[(10.0, 10.0), (20.0, 20.0)]));
}

pub fn over_the_edge(s: &mut Session) {
    on_the_top(s, |s| build::circle(s, (20.0, 0.0), (25.0, 0.0)));
}

// SKETCHES FOR A SWEEP AND A LOFT: the profile, or the first section, drawn on XY; the path on XZ, or the next
// section on a datum plane 30 above; the profile's row picked in the tree last, as the tool wants it before it is
// taken.

pub static SWEEP_STRAIGHT: Body = Body { name: "a square of 10 on XY and an upright line of 40 on XZ", build: sweep_straight };
pub static SWEEP_ARC: Body = Body { name: "a square of 10 about the origin on XY and a quarter circle of 30 rising from it on XZ", build: sweep_arc };
pub static SWEEP_ROUND: Body = Body { name: "a circle of radius 5 about the origin on XY and a quarter circle of 30 rising from it on XZ", build: sweep_round };
pub static LOFT_SQUARES: Body = Body { name: "a square of 10 on XY and a square of 20 about the same middle 30 above", build: loft_squares };
pub static LOFT_CIRCLES: Body = Body { name: "a circle of radius 10 on XY and one of radius 5 about the same middle 30 above", build: loft_circles };
pub static LOFT_SAME: Body = Body { name: "a square of 10 on XY and the same square 30 above", build: loft_same };

/// A new sketch on the plane of the world `key` names, drawn by `draw` and finished.
fn on_world_plane(s: &mut Session, key: &str, draw: impl FnOnce(&mut Session)) {
    s.key(Key::Escape);
    let plane = s.word(key);
    s.press_word(&plane);
    draw(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

/// Pick the row of the sketch numbered `i` in the tree.
pub fn sketch_row(s: &mut Session, i: usize) {
    s.key(Key::Escape);
    let name = s.document().sketches.get(i).map(|k| k.name.clone()).unwrap_or_else(|| panic!("there is no sketch numbered {i}"));
    let row = s.find(&name, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("{name:?} is not in the tree; on screen: {:?}", s.words()));
    s.click(row.center());
}

/// A datum plane `z` above the table, and a sketch on it drawn by `draw` and finished.
fn above(s: &mut Session, z: f64, draw: impl FnOnce(&mut Session)) {
    s.key(Key::Escape);
    let datum = s.word("g-datum-plane-hint");
    s.press_hint(&datum);
    let middle = s.in_space([0.0, 0.0, 0.0]);
    s.click(middle);
    // the offset typed into its own field, by its caption: the plane has its tilts and its turn beside it
    let offset = s.word("f-offset");
    s.fill(&offset, &format!("{z}")).key(Key::Enter);
    let pencil = s.word("g-sketch-pick-hint");
    s.press_hint(&pencil);
    let on_datum = s.in_space([0.0, 0.0, z]);
    s.click(on_datum);
    draw(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
}

/// A quarter circle of radius 30 on XZ about (-30, 0), from the origin up to (-30, 30): it leaves the origin going up.
fn quarter_circle(s: &mut Session) {
    build::draw(s, "tb-arc-hint", &[(-30.0, 0.0), (0.0, 0.0), (-30.0, 30.0)]);
}

pub fn sweep_straight(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]));
    on_world_plane(s, "plane-xz-front", |s| build::line(s, (0.0, 0.0), (0.0, 40.0)));
    sketch_row(s, 0);
}

pub fn sweep_arc(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::draw(s, "tb-rect-hint", &[(-5.0, -5.0), (5.0, 5.0)]));
    on_world_plane(s, "plane-xz-front", quarter_circle);
    sketch_row(s, 0);
}

pub fn sweep_round(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::circle(s, (0.0, 0.0), (5.0, 0.0)));
    on_world_plane(s, "plane-xz-front", quarter_circle);
    sketch_row(s, 0);
}

pub fn loft_squares(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]));
    above(s, 30.0, |s| build::draw(s, "tb-rect-hint", &[(-5.0, -5.0), (15.0, 15.0)]));
    sketch_row(s, 0);
}

pub fn loft_circles(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::circle(s, (0.0, 0.0), (10.0, 0.0)));
    above(s, 30.0, |s| build::circle(s, (0.0, 0.0), (5.0, 0.0)));
    sketch_row(s, 0);
}

pub fn loft_same(s: &mut Session) {
    build::into_the_first_part(s);
    on_world_plane(s, "plane-xy-table", |s| build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]));
    above(s, 30.0, |s| build::draw(s, "tb-rect-hint", &[(0.0, 0.0), (10.0, 10.0)]));
    sketch_row(s, 0);
}

pub static WEDGE: Body = Body { name: "a wedge: the triangle (0, 0), (40, 0), (0, 10) on XZ, 30 thick about the plane", build: wedge };

/// A wedge: a right triangle 40 long and 10 tall drawn on XZ, extruded 30 symmetric about the plane, so it runs from
/// y = -15 to 15. Its sharp edge at x = 40 on the table is 14.04 degrees across; the edge at the top of its back is
/// 75.96 degrees.
pub fn wedge(s: &mut Session) {
    build::into_the_first_part(s);
    let xz = s.word("plane-xz-front");
    s.press_word(&xz);
    build::draw(s, "tb-line-hint", &[(0.0, 0.0), (40.0, 0.0), (0.0, 10.0), (0.0, 0.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    let symmetric = s.word("cmd-symmetric");
    s.press_word_near(&symmetric, qymcad::pos2(640.0, 0.0));
    let length = s.word("f-length");
    s.fill(&length, "30").key(Key::Enter);
}
