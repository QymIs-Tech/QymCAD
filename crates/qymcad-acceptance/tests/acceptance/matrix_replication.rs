//! A BODY MULTIPLIED ACROSS BODIES, PLANES AND COUNTS: reflected in each of its faces, the copy joined to it where they
//! touch; laid out in a row with room between, touching, overlapping; laid out round the upright axis; counts that
//! make nothing, and below zero. What each must hold is worked out from the geometry.
use std::f64::consts::PI;

use qymcad_acceptance::bodies::{BLOCK, CYLINDER, ROUNDED};
use qymcad_acceptance::chains::Tier;
use qymcad_acceptance::matrix::{fillet_area, run_all, spot, Added, Body, Case, Expect, PartTool, Pick, Spot, Then};
use qymcad_acceptance::probe;

const MIRROR: PartTool = PartTool { hint: "tb-mirror-body-hint", caption: "", node: "Mirror", counts: false };
const ROW: PartTool = PartTool { hint: "tb-lin-array-body-hint", caption: "cmd-copies", node: "LinearArray", counts: false };
const RING: PartTool = PartTool { hint: "tb-circ-array-body-hint", caption: "cmd-copies", node: "CircularArray", counts: false };

const TOP: Spot = spot([20.0, 15.0, 10.0], [10.0, 8.0, 10.0]);
const RIGHT: Spot = spot([40.0, 15.0, 5.0], [40.0, 8.0, 5.0]);
const FRONT: Spot = spot([20.0, 0.0, 5.0], [10.0, 0.0, 5.0]);
const CYLINDER_TOP: Spot = spot([20.0, 15.0, 10.0], [16.0, 15.0, 10.0]);
const CYLINDER_SIDE: Spot = spot([27.071, 7.929, 5.0], [27.071, 7.929, 3.0]);

fn tol(v: f64) -> f64 {
    (v.abs() * 1e-3).max(1e-4)
}

fn case(body: &'static Body, picks: Vec<Pick>, value: f64, more: &[(&'static str, f64)], expect: Expect, what: &str) -> Case {
    Case { body, picks, bar: Vec::new(), value: format!("{value}"), more: more.iter().map(|(c, v)| (*c, format!("{v}"))).collect(), expect, what: what.to_string(), then: None, after: Vec::new() }
}

/// The case, and then the block made 15 tall instead of 10 above the tool: the copies follow it, and the tool now adds
/// `change` to a block of 18000.
fn taller(mut c: Case, change: f64) -> Case {
    c.then = Some(Then { row: "cmd-extrude", caption: "f-length", value: "15", base: 18000.0, change, tol: tol(change) });
    c
}

fn round_trips() -> bool {
    Tier::asked() == Tier::Release
}

/// MIRROR in a face of the body: the copy stands against the face and is joined to the body along it - twice the
/// volume, and a block doubled is a block, with the faces a block has.
fn mirror_cases() -> Vec<Case> {
    let doubled = |v: f64, same: bool| Expect::Built { change: v, tol: tol(v), added: same.then(Added::default) };
    let mirror = |body: &'static Body, grip: Spot, plane: Spot, v: f64, same: bool, what: &str| case(body, vec![Pick::Face(grip), Pick::Face(plane)], 0.0, &[], doubled(v, same), what);
    vec![
        taller(mirror(&BLOCK, TOP, RIGHT, 12000.0, true, "the block in its right face"), 18000.0),
        mirror(&BLOCK, TOP, FRONT, 12000.0, true, "the block in its front face"),
        mirror(&BLOCK, FRONT, TOP, 12000.0, true, "the block in its top face"),
        // gripped by its side: a click on the top face is taken as the plane, and a second click on it lets it go
        mirror(&CYLINDER, CYLINDER_SIDE, CYLINDER_TOP, PI * 1000.0, true, "the cylinder in its top face"),
        // the rounding along the top front edge runs on into its reflection: one rounding the whole 80 long
        mirror(&ROUNDED, TOP, RIGHT, 12000.0 - 40.0 * fillet_area(2.0, PI / 2.0), false, "the rounded block in its right face"),
    ]
}

/// A ROW of copies of the block along X by the pitch: with room between them each copy is a piece of its own;
/// touching, or overlapping, they join into one block as long as the row.
fn row_cases() -> Vec<Case> {
    let row = |copies: f64, pitch: f64, expect: Expect, what: &str| case(&BLOCK, vec![Pick::Face(TOP)], copies, &[("f-pitch", pitch)], expect, what);
    let one_block = |length: f64| Expect::Built { change: length * 300.0 - 12000.0, tol: tol(length * 300.0), added: Some(Added::default()) };
    vec![
        row(3.0, 50.0, Expect::Pieces { change: 24000.0, tol: tol(24000.0), pieces: 3 }, "three, 50 apart: room between"),
        row(2.0, 100.0, Expect::Pieces { change: 12000.0, tol: tol(12000.0), pieces: 2 }, "two, 100 apart"),
        taller(row(3.0, 40.0, one_block(120.0), "three, 40 apart: touching, one block 120 long"), 36000.0),
        row(3.0, 30.0, one_block(100.0), "three, 30 apart: overlapping, one block 100 long"),
        row(10.0, 40.0, one_block(400.0), "ten, 40 apart: one block 400 long"),
        row(1.0, 50.0, Expect::Refused, "one: the block alone, nothing made"),
        row(0.0, 50.0, Expect::Refused, "none"),
        row(-2.0, 50.0, Expect::Refused, "below zero"),
        row(3.0, 0.0, Expect::Refused, "three with no pitch: all in one place"),
    ]
}

/// A RING of copies about the upright axis through the origin, spread over the whole turn: four blocks a quarter turn
/// apart touch along their sides and make one body of four blocks' volume.
fn ring_cases() -> Vec<Case> {
    // the count; the angle the copies spread over stands at the whole turn as the box opens
    let ring = |copies: f64, expect: Expect, what: &str| case(&BLOCK, vec![Pick::Face(TOP)], copies, &[], expect, what);
    vec![
        ring(4.0, Expect::Built { change: 36000.0, tol: tol(48000.0), added: None }, "four over the whole turn: touching, one body"),
        ring(6.0, Expect::Whole, "six over the whole turn: overlapping about the axis"),
        ring(1.0, Expect::Refused, "one: nothing made"),
        ring(0.0, Expect::Refused, "none"),
    ]
}

macro_rules! replication_matrix {
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

replication_matrix!(mirrors_in_each_face, "MIRROR a block in its right, front and top faces, a cylinder in its top, a rounded block in its right.", MIRROR, mirror_cases);
replication_matrix!(rows_of_copies, "A ROW of copies with room between, touching, overlapping, ten long; one, none, below zero and no pitch refused.", ROW, row_cases);
replication_matrix!(rings_of_copies, "A RING of four and of six over the whole turn; one and none refused.", RING, ring_cases);
