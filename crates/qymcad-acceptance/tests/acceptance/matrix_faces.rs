//! THE TOOLS THAT WORK ON FACES ACROSS BODIES, PICKS AND VALUES: a shell, a hole, a face pushed, a face thickened,
//! faces split, faces removed, a face tilted by a draft - on a block, a cylinder, a block with a hole, a chamfer, a
//! rounding, a notch or a boss; values ordinary, small, near the limit the geometry sets and past it, zero and below.
//! What each case must come to is worked out from the geometry.
//!
//! The level before a release adds undo and redo, save and open, and rebuilding everything to every body built.
use std::f64::consts::PI;

use qymcad_acceptance::bodies::{BLOCK, BOSSED, CHAMFERED, CYLINDER, HOLED, NOTCHED, ROUNDED, TAPPED, THROUGH};
use qymcad_acceptance::chains::Tier;
use qymcad_acceptance::iso::{inner_groove, outer_groove};
use qymcad_acceptance::matrix::{fillet_area, run_all, short, spot, Added, Body, Case, Expect, PartTool, Pick, Spot, Then};
use qymcad_acceptance::probe;

const SHELL: PartTool = PartTool { hint: "tb-shell-hint", caption: "cmd-thickness", node: "Shell", counts: false };
const HOLE: PartTool = PartTool { hint: "tb-hole-hint", caption: "f-diameter", node: "Hole", counts: false };
const PUSH: PartTool = PartTool { hint: "tb-push-face-hint", caption: "f-offset", node: "PushFace", counts: false };
const THICKEN: PartTool = PartTool { hint: "tb-thicken-hint", caption: "f-thickness", node: "Thicken", counts: false };
const SPLIT: PartTool = PartTool { hint: "tb-split-face-hint", caption: "f-offset", node: "SplitFace", counts: false };
const REMOVE: PartTool = PartTool { hint: "tb-remove-face-hint", caption: "", node: "RemoveFace", counts: false };
const DRAFT: PartTool = PartTool { hint: "tb-draft-hint", caption: "cmd-angle", node: "Draft", counts: false };
const THREAD: PartTool = PartTool { hint: "tb-thread-hint", caption: "th-nominal-d", node: "Thread", counts: false };

/// Faces of the bodies, by a point on each and another point on it.
const TOP: Spot = spot([20.0, 15.0, 10.0], [10.0, 8.0, 10.0]);
/// The top of the block away from its middle, where a hole may be.
const TOP_ASIDE: Spot = spot([8.0, 8.0, 10.0], [32.0, 8.0, 10.0]);
const FRONT: Spot = spot([20.0, 0.0, 5.0], [10.0, 0.0, 5.0]);
const RIGHT: Spot = spot([40.0, 15.0, 5.0], [40.0, 8.0, 5.0]);
/// The top of the cylinder, and its side at the front right.
const CYLINDER_TOP: Spot = spot([20.0, 15.0, 10.0], [16.0, 15.0, 10.0]);
const CYLINDER_SIDE: Spot = spot([27.071, 7.929, 5.0], [27.071, 7.929, 3.0]);
/// The face of the chamfer of 2 on the top front edge, the face of the rounding of 2 on it at 45 degrees: narrow
/// faces, scaled up to until their edge with the top stands clear of the click.
const CHAMFER_FACE: Spot = short([20.0, 1.0, 9.0], [10.0, 1.0, 9.0], [20.0, 2.0, 10.0]);
const ROUND_FACE: Spot = short([20.0, 0.586, 9.414], [10.0, 0.586, 9.414], [20.0, 2.0, 10.0]);
/// The wall of the hole at its back, seen through the opening, and its floor.
const HOLE_WALL: Spot = spot([20.0, 20.0, 7.5], [21.71, 19.7, 7.5]);
/// The wall of the hole drilled 8.5 for M10, at its back, seen through the opening.
const TAPPED_WALL: Spot = spot([20.0, 19.25, 6.0], [21.45, 18.97, 6.0]);
const HOLE_FLOOR: Spot = Spot { at: [20.0, 15.0, 5.0], alt: [21.0, 14.0, 5.0], from: [0, 0, 1], end: None };
/// The side of the boss at the front right and its top.
const BOSS_SIDE: Spot = spot([24.243, 10.757, 12.5], [24.243, 10.757, 11.5]);
const BOSS_TOP: Spot = spot([20.0, 15.0, 15.0], [18.0, 15.0, 15.0]);
/// The faces of the notch: its floor, its back wall, its side walls.
const NOTCH_FLOOR: Spot = spot([20.0, 5.0, 5.0], [18.0, 3.0, 5.0]);
const NOTCH_BACK: Spot = spot([20.0, 10.0, 7.5], [18.0, 10.0, 7.0]);
const NOTCH_LEFT: Spot = spot([15.0, 5.0, 7.5], [15.0, 3.0, 7.0]);
const NOTCH_RIGHT: Spot = Spot { at: [25.0, 5.0, 7.5], alt: [25.0, 3.0, 7.0], from: [-1, -1, 1], end: None };

/// How close the volume must come: a thousandth of what changes, never finer than the numbers of the exact body.
fn tol(change: f64) -> f64 {
    (change.abs() * 1e-3).max(1e-4)
}

fn built(change: f64, added: Option<Added>) -> Expect {
    Expect::Built { change, tol: tol(change), added }
}

fn planes(n: i32) -> Added {
    Added { plane: n, ..Default::default() }
}

fn faces(plane: i32, cylinder: i32, cone: i32, blend: i32) -> Added {
    Added { plane, cylinder, cone, blend }
}

/// A case on `body`: `picks` faces clicked, `value` typed into the tool's own field and `more` into others.
fn case(body: &'static Body, picks: &[Spot], value: f64, more: &[(&'static str, f64)], expect: Expect, what: &str) -> Case {
    Case {
        body,
        picks: picks.iter().map(|p| Pick::Face(*p)).collect(),
        bar: Vec::new(),
        value: format!("{value}"),
        more: more.iter().map(|(c, v)| (*c, format!("{v}"))).collect(),
        expect,
        what: what.to_string(),
        then: None,
        after: Vec::new(),
    }
}

/// The case, and then the block made 15 tall instead of 10 above the tool: its extrusion reopened and the length
/// typed anew. The block then holds 18000 less what came before the tool, and the tool takes `change` of it.
fn taller(mut c: Case, base: f64, change: f64) -> Case {
    c.then = Some(Then { row: "cmd-extrude", caption: "f-length", value: "15", base, change, tol: tol(change) });
    c
}

/// The level before a release checks the round trips as well.
fn round_trips() -> bool {
    Tier::asked() == Tier::Release
}

/// SHELL: the body hollowed to the thickness `t`, open at the faces clicked. The cavity is every point of the body
/// farther than `t` from every face left - a box less the walls; round a hole, the hole and all within `t` of it (the
/// hole swept by a ball of `t`). An open top leaves a rim in its plane.
fn shell_cases() -> Vec<Case> {
    let mut out = Vec::new();
    for t in [2.0f64, 0.5, 4.5] {
        let top = |h: f64| -(40.0 - 2.0 * t) * (30.0 - 2.0 * t) * (h - t);
        let top_front = |h: f64| -(40.0 - 2.0 * t) * (30.0 - t) * (h - t);
        out.push(taller(case(&BLOCK, &[TOP], t, &[], built(top(10.0), Some(planes(5))), "the block, open at the top"), 18000.0, top(15.0)));
        out.push(taller(case(&BLOCK, &[TOP, FRONT], t, &[], built(top_front(10.0), Some(planes(4))), "the block, open at the top and the front"), 18000.0, top_front(15.0)));
        out.push(case(&CYLINDER, &[CYLINDER_TOP], t, &[], built(-PI * (10.0 - t).powi(3), Some(faces(1, 1, 0, 0))), "the cylinder, open at the top"));
    }
    // round a hole right through, the wall of 5 + t about it stands on the floor: the cavity is the box less that
    // wall, and the rim of the wall is a face of its own in the plane of the top
    for t in [1.0f64, 0.5, 2.0] {
        let cavity = (40.0 - 2.0 * t) * (30.0 - 2.0 * t) * (10.0 - t) - PI * (5.0 + t).powi(2) * (10.0 - t);
        out.push(case(&THROUGH, &[TOP_ASIDE], t, &[], built(-cavity, Some(faces(6, 1, 0, 0))), "the block with a hole right through, open at the top"));
    }
    // round a blind hole 5 deep, the cup of t about it hangs in the cavity until it reaches the floor at t = 2.5:
    // a body of two pieces, which a part cannot be - refused in words
    for t in [1.0f64, 2.0] {
        out.push(case(&HOLED, &[TOP_ASIDE], t, &[], Expect::Refused, "the block with a blind hole, open at the top, the cup round the hole hanging free"));
    }
    // a shell of a block whose open face meets a rounding tangent: built whole or refused in words
    out.push(case(&ROUNDED, &[TOP], 1.0, &[], Expect::Either, "the rounded block, open at the top"));
    for (t, what) in [(10.5, "past the 10 of the height"), (16.0, "past half the 30 of the width"), (0.0, "nothing"), (-2.0, "below zero")] {
        out.push(case(&BLOCK, &[TOP], t, &[], Expect::Refused, &format!("the block, open at the top, {what}")));
    }
    out
}

/// HOLE: a round hole of the diameter and the depth typed, bored square into the face clicked, at the click: what it
/// takes is its cylinder; a blind one adds its wall and its floor, one right through only its wall.
fn hole_cases() -> Vec<Case> {
    let hole = |d: f64, depth: f64| -PI * d * d / 4.0 * depth;
    let blind = Some(faces(1, 1, 0, 0));
    let mut out = Vec::new();
    for d in [10.0, 1.0, 28.0] {
        out.push(taller(case(&BLOCK, &[TOP], d, &[("f-depth", 5.0)], built(hole(d, 5.0), blind), "the top of the block, 5 deep"), 18000.0, hole(d, 5.0)));
    }
    out.push(case(&BLOCK, &[TOP], 10.0, &[("f-depth", 12.0)], built(hole(10.0, 10.0), Some(faces(0, 1, 0, 0))), "the top of the block, right through"));
    out.push(taller(case(&BLOCK, &[FRONT], 6.0, &[("f-depth", 10.0)], built(hole(6.0, 10.0), blind), "the front of the block, 10 deep"), 18000.0, hole(6.0, 10.0)));
    out.push(case(&CYLINDER, &[CYLINDER_TOP], 8.0, &[("f-depth", 5.0)], built(hole(8.0, 5.0), blind), "the top of the cylinder"));
    out.push(case(&CHAMFERED, &[CHAMFER_FACE], 2.0, &[("f-depth", 1.0)], built(hole(2.0, 1.0), blind), "the face of the chamfer, square to it"));
    out.push(case(&BOSSED, &[BOSS_TOP], 6.0, &[("f-depth", 8.0)], built(hole(6.0, 8.0), blind), "the top of the boss, on into the block"));
    for (d, depth, what) in [(0.0, 5.0, "nothing across"), (-2.0, 5.0, "below zero across"), (10.0, 0.0, "nothing deep"), (10.0, -2.0, "below zero deep")] {
        out.push(case(&BLOCK, &[TOP], d, &[("f-depth", depth)], Expect::Refused, &format!("the top of the block, {what}")));
    }
    out
}

/// PUSH FACE: the face moves along its own normal by the offset, out for more and in for less; its neighbours follow.
/// A chamfer face pushed out by `o` sits `o` farther from the corner, so its leg is 2 - o * sqrt 2.
fn push_cases() -> Vec<Case> {
    let chamfer = |o: f64| {
        let leg = 2.0 - o * 2f64.sqrt();
        40.0 * (2.0 - leg * leg / 2.0)
    };
    let same = Some(Added::default());
    vec![
        taller(case(&BLOCK, &[TOP], 5.0, &[], built(40.0 * 30.0 * 5.0, same), "the top of the block, out"), 18000.0, 40.0 * 30.0 * 5.0),
        taller(case(&BLOCK, &[TOP], -3.0, &[], built(-40.0 * 30.0 * 3.0, same), "the top of the block, in"), 18000.0, -40.0 * 30.0 * 3.0),
        taller(case(&BLOCK, &[FRONT], -5.0, &[], built(-40.0 * 5.0 * 10.0, same), "the front of the block, in"), 18000.0, -40.0 * 5.0 * 15.0),
        case(&CYLINDER, &[CYLINDER_TOP], 5.0, &[], built(PI * 100.0 * 5.0, same), "the top of the cylinder, out"),
        case(&CYLINDER, &[CYLINDER_SIDE], 2.0, &[], built(PI * (144.0 - 100.0) * 10.0, same), "the side of the cylinder, out"),
        case(&CHAMFERED, &[CHAMFER_FACE], 1.0, &[], built(chamfer(1.0), same), "the face of the chamfer, out"),
        case(&CHAMFERED, &[CHAMFER_FACE], -1.0, &[], built(chamfer(-1.0), same), "the face of the chamfer, in"),
        case(&BLOCK, &[TOP], -10.0, &[], Expect::Refused, "the top of the block, in by all its height"),
        case(&BLOCK, &[TOP], 0.0, &[], Expect::Refused, "the top of the block, by nothing"),
    ]
}

/// THICKEN: the face grows a plate of the thickness outwards, joined to the body - a block made taller keeps six
/// faces, not its sides cut where the plate meets them.
fn thicken_cases() -> Vec<Case> {
    let same = Some(Added::default());
    vec![
        taller(case(&BLOCK, &[TOP], 3.0, &[], built(40.0 * 30.0 * 3.0, same), "the top of the block"), 18000.0, 40.0 * 30.0 * 3.0),
        case(&BLOCK, &[TOP], 0.2, &[], built(40.0 * 30.0 * 0.2, same), "the top of the block, thin"),
        taller(case(&BLOCK, &[FRONT], 2.0, &[], built(40.0 * 10.0 * 2.0, same), "the front of the block"), 18000.0, 40.0 * 15.0 * 2.0),
        case(&CYLINDER, &[CYLINDER_TOP], 3.0, &[], built(PI * 100.0 * 3.0, same), "the top of the cylinder"),
        case(&BLOCK, &[TOP], 0.0, &[], Expect::Refused, "the top of the block, nothing"),
        case(&BLOCK, &[TOP], -2.0, &[], Expect::Refused, "the top of the block, below zero"),
    ]
}

/// SPLIT FACES: a plane parallel to the face clicked, at the offset, divides every face it crosses; the body is the
/// same. A plane that misses the body, or lies in the face, divides nothing and must say so.
fn split_cases() -> Vec<Case> {
    vec![
        taller(case(&BLOCK, &[TOP], -5.0, &[], built(0.0, Some(planes(4))), "the block, across its middle"), 18000.0, 0.0),
        case(&BLOCK, &[TOP], -2.5, &[], built(0.0, Some(planes(4))), "the block, near its top"),
        case(&CYLINDER, &[CYLINDER_TOP], -5.0, &[], built(0.0, Some(faces(0, 1, 0, 0))), "the cylinder, across its middle"),
        case(&BLOCK, &[TOP], -12.0, &[], Expect::Refused, "the block, by a plane below it"),
        case(&BLOCK, &[TOP], 0.0, &[], Expect::Refused, "the block, by the plane of its top"),
    ]
}

/// REMOVE FACES: the faces of a feature clicked, the body healed as if the feature had never been.
fn remove_cases() -> Vec<Case> {
    vec![
        case(&ROUNDED, &[ROUND_FACE], 0.0, &[], built(40.0 * fillet_area(2.0, PI / 2.0), Some(faces(0, -1, 0, 0))), "the face of the rounding"),
        case(&CHAMFERED, &[CHAMFER_FACE], 0.0, &[], built(80.0, Some(planes(-1))), "the face of the chamfer"),
        case(&HOLED, &[HOLE_WALL, HOLE_FLOOR], 0.0, &[], built(PI * 25.0 * 5.0, Some(faces(-1, -1, 0, 0))), "the wall and the floor of the hole"),
        case(&BOSSED, &[BOSS_SIDE, BOSS_TOP], 0.0, &[], built(-PI * 36.0 * 5.0, Some(faces(-1, -1, 0, 0))), "the side and the top of the boss"),
        case(&NOTCHED, &[NOTCH_FLOOR, NOTCH_BACK, NOTCH_LEFT, NOTCH_RIGHT], 0.0, &[], built(500.0, Some(planes(-4))), "the four faces of the notch"),
        case(&BLOCK, &[TOP], 0.0, &[], Expect::Refused, "the top of the block, which nothing can heal"),
    ]
}

/// DRAFT: the face clicked first tilts by the angle about its edge on the face clicked second; the wedge it sweeps is
/// half the height squared by the tangent, along the edge - out or in by the program's own sign.
fn draft_cases() -> Vec<Case> {
    // the face to tilt, then the neutral face by its button and a click
    let about_top = |mut c: Case| {
        c.picks.insert(1, Pick::Bar("cmd-neutral-face"));
        c
    };
    let wedge = |h: f64, along: f64, deg: f64| h * h * deg.to_radians().tan() / 2.0 * along;
    let mut out = Vec::new();
    for deg in [10.0, 3.0, 30.0] {
        out.push(about_top(case(
            &BLOCK,
            &[FRONT, TOP],
            deg,
            &[],
            Expect::Magnitude { change: wedge(10.0, 40.0, deg), tol: tol(wedge(10.0, 40.0, deg)), added: Some(Added::default()) },
            "the front of the block, about its top edge",
        )));
        out.push(about_top(case(
            &BLOCK,
            &[RIGHT, TOP],
            deg,
            &[],
            Expect::Magnitude { change: wedge(10.0, 30.0, deg), tol: tol(wedge(10.0, 30.0, deg)), added: Some(Added::default()) },
            "the right of the block, about its top edge",
        )));
    }
    out.push(about_top(case(&BLOCK, &[FRONT, TOP], 0.0, &[], Expect::Refused, "the front of the block, by nothing")));
    out.push(about_top(case(&BLOCK, &[FRONT, TOP], 90.0, &[], Expect::Refused, "the front of the block, by a right angle")));
    out
}

/// THREAD: an outer M20 (pitch 2.5) on the cylinder of 20, an inner M10 (pitch 1.5) in the hole drilled 8.5 for it -
/// with no fit clearance and no run-out, so that what the groove takes is the standard's own profile, to a
/// hundredth. A thread of another size than the face it is cut on, a length of nothing or below zero, and a flat face,
/// are refused.
fn thread_cases() -> Vec<Case> {
    let bare = |mut c: Case| {
        c.more.extend([("th-fit-clearance", "0".to_string()), ("th-lead-in", "0".to_string()), ("th-lead-out", "0".to_string())]);
        c
    };
    let takes = |v: f64| Expect::Built { change: -v, tol: v * 0.01, added: None };
    let outer = |length: f64, what: &str| bare(case(&CYLINDER, &[CYLINDER_SIDE], 20.0, &[("f-length", length)], takes(outer_groove(20.0, 2.5, length)), what));
    let inner = |length: f64, what: &str| {
        let mut c = bare(case(&TAPPED, &[TAPPED_WALL], 10.0, &[("f-length", length)], takes(inner_groove(10.0, 1.5, 8.5, length)), what));
        c.bar = vec!["cmd-thread-internal"];
        c
    };
    vec![
        outer(10.0, "M20 the whole 10 of the cylinder"),
        outer(5.0, "M20 over 5 of the cylinder"),
        inner(8.0, "M10 inside, the whole 8 of the hole"),
        inner(4.0, "M10 inside, over 4 of the hole"),
        case(&CYLINDER, &[CYLINDER_SIDE], 10.0, &[("f-length", 10.0)], Expect::Refused, "M10 on the cylinder of 20"),
        case(&CYLINDER, &[CYLINDER_SIDE], 20.0, &[("f-length", 0.0)], Expect::Refused, "M20 over nothing"),
        case(&CYLINDER, &[CYLINDER_SIDE], 20.0, &[("f-length", -5.0)], Expect::Refused, "M20 over a length below zero"),
        case(&BLOCK, &[TOP], 20.0, &[("f-length", 10.0)], Expect::Refused, "M20 on a flat face"),
    ]
}

macro_rules! face_matrix {
    ($name:ident, $doc:literal, $tool:expr, $cases:expr) => {
        probe! {
            budget = 1800;
            #[doc = $doc]
            fn $name() {
                run_all(&$tool, &$cases(), round_trips());
            }
        }
    };
}

face_matrix!(shells_across_bodies, "SHELL across bodies: open at one face and at two, round a hole, by a rounding; thicknesses past what the body holds, zero and below refused.", SHELL, shell_cases);
face_matrix!(holes_across_faces, "HOLE into the top, the front, a cylinder, a chamfer, a boss; blind and through; nothing across or deep, and below zero, refused.", HOLE, hole_cases);
face_matrix!(pushed_faces_across_bodies, "PUSH FACE out and in on a block, a cylinder's top and side, a chamfer; all the height in, and nothing, refused.", PUSH, push_cases);
face_matrix!(thickened_faces_across_bodies, "THICKEN the top and the front of a block and the top of a cylinder; nothing and below zero refused.", THICKEN, thicken_cases);
face_matrix!(split_faces_across_bodies, "SPLIT FACES across a block and a cylinder; a plane that misses, or lies in the face, refused.", SPLIT, split_cases);
face_matrix!(removed_faces_across_features, "REMOVE FACES of a rounding, a chamfer, a hole, a boss and a notch; a face nothing can heal refused.", REMOVE, remove_cases);
face_matrix!(drafts_across_faces, "DRAFT the front and the right of a block about their top edges at 3, 10 and 30 degrees; nothing and a right angle refused.", DRAFT, draft_cases);
face_matrix!(
    threads_outside_and_inside,
    "THREAD: M20 on a cylinder, M10 in a hole drilled for it, whole and part of the length; another size, no length, below zero and a flat face refused.",
    THREAD,
    thread_cases
);
