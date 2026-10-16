//! The tools that make and finish a solid: extrude, fillet, chamfer, hole, shell, revolve.

use super::*;

/// A 40 x 30 box of height `h` standing on XY from `z0`.
pub(crate) const fn box_outcome(z0: f64, h: f64) -> Outcome {
    Outcome::Body { volume: 1200.0 * h, faces: 6, edges: 12, min: [0.0, 0.0, z0], max: [40.0, 30.0, z0 + h] }
}

/// The 40 x 30 box extruded `h` high.
pub(crate) fn extruded(h: f64) -> Outcome {
    box_outcome(0.0, h)
}

/// EXTRUDE: the rectangle of the sketch made a body, 10 high by default.
pub static EXTRUDE: Tool = Tool {
    id: "part.extrude",
    flow: Flow::Command,
    title: "cmd-extrude",
    entries: &[Entry::Button("tb-extrude-hint"), Entry::Key(Modifiers::NONE, Key::E), Entry::Search("cmd-extrude")],
    other: (Entry::Button("tb-revolve-hint"), "cmd-revolve"),
    fixture: Fixture::RectangleSketch,
    picks: &[],
    pick_trial: &[Pick::Space([20.0, 15.0, 0.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-length", by_placeholder: false, when: When::Before, class: Class::Length, typical: 10.0, lo: 0.1, hi: 10000.0, zero: false, negative: false, outcome: extruded }],
    modes: &[
        &[
            Mode { word: "cmd-add", clicks: None, outcome: Some(box_outcome(0.0, 10.0)) },
            Mode { word: "cmd-cut", clicks: None, outcome: None },
            Mode { word: "cmd-intersect", clicks: None, outcome: None },
        ],
        &[
            Mode { word: "cmd-to-length", clicks: None, outcome: Some(box_outcome(0.0, 10.0)) },
            Mode { word: "cmd-symmetric", clicks: None, outcome: Some(box_outcome(-5.0, 10.0)) },
            Mode { word: "cmd-two-sides", clicks: None, outcome: Some(box_outcome(-0.1, 10.1)) },
        ],
        &[Mode { word: "cmd-flip", clicks: None, outcome: Some(box_outcome(-10.0, 10.0)) }],
    ],
    result: box_outcome(0.0, 10.0),
    node: "Extrude",
    undo: "f-extrusion",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::SketchDimension { shown: "40", value: 50.0, then: Outcome::Body { volume: 15000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [50.0, 30.0, 10.0] } }),
    dependency: Some(Node { kind: "Sketch", row: None }),
    contexts: &[Context::SecondPart, Context::AfterTool(&FILLET)],
    refusal: None,
    budget: (30, 2000),
    help: "part/01-extrude",
    not_applicable: &[(17, "a length of the right form over a closed contour always builds a prism: there is nothing for the kernel to refuse")],
};

/// The block with the top front edge rounded to `r`: the edge's quarter of a square taken off, less the quarter disc.
pub(crate) fn filleted(r: f64) -> Outcome {
    Outcome::Body { volume: 12000.0 - 40.0 * r * r * (1.0 - PI / 4.0), faces: 7, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

/// FILLET: the top front edge of the block rounded, 2 by default.
pub static FILLET: Tool = Tool {
    id: "part.fillet",
    flow: Flow::Command,
    title: "cmd-fillet",
    entries: &[Entry::Button("tb-fillet-body-hint"), Entry::Key(Modifiers::NONE, Key::F), Entry::Search("cmd-fillet")],
    other: (Entry::Button("tb-chamfer-body-hint"), "cmd-chamfer"),
    fixture: Fixture::Block,
    picks: &[Pick::Edge([20.0, 0.0, 10.0])],
    pick_trial: &[Pick::Edge([20.0, 0.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-radius", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 2.0, lo: 0.05, hi: 9.5, zero: false, negative: false, outcome: filleted }],
    modes: &[],
    result: Outcome::Body { volume: 12000.0 - 160.0 * (1.0 - PI / 4.0), faces: 7, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] },
    node: "Fillet",
    undo: "f-fillet",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0 - 160.0 * (1.0 - PI / 4.0), faces: 7, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart, Context::AfterTool(&EXTRUDE)],
    refusal: Some(12.0),
    budget: (30, 2000),
    help: "part/05-fillet",
    not_applicable: &[(2, "the fillet's bar has no modes: it takes edges and a radius")],
};

/// The block with the top front edge cut by a chamfer of leg `d`: the triangle d^2 / 2 taken off along all 40 mm.
pub(crate) const fn chamfered(d: f64) -> Outcome {
    Outcome::Body { volume: 12000.0 - 20.0 * d * d, faces: 7, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

/// CHAMFER: the top front edge of the block cut, the leg 1.5 by default. Its two asymmetric modes start at the same
/// cut - a second leg of 1.5, or 1.5 and 45 degrees - so all three make the same body of the ordinary picks.
pub static CHAMFER: Tool = Tool {
    id: "part.chamfer",
    flow: Flow::Command,
    title: "cmd-chamfer",
    entries: &[Entry::Button("tb-chamfer-body-hint"), Entry::Key(Modifiers::NONE, Key::C), Entry::Search("cmd-chamfer")],
    other: (Entry::Button("tb-fillet-body-hint"), "cmd-fillet"),
    fixture: Fixture::Block,
    picks: &[Pick::Edge([20.0, 0.0, 10.0])],
    pick_trial: &[Pick::Edge([20.0, 0.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-leg", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 1.5, lo: 0.05, hi: 9.5, zero: false, negative: false, outcome: chamfered }],
    modes: &[&[
        Mode { word: "cmd-symmetric", clicks: None, outcome: Some(chamfered(1.5)) },
        Mode { word: "cmd-two-distances", clicks: None, outcome: Some(chamfered(1.5)) },
        Mode { word: "cmd-leg-angle", clicks: None, outcome: Some(chamfered(1.5)) },
    ]],
    result: chamfered(1.5),
    node: "Chamfer",
    undo: "f-chamfer",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0 - 45.0, faces: 7, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart, Context::AfterTool(&EXTRUDE)],
    refusal: Some(12.0),
    budget: (30, 2000),
    help: "part/06-chamfer",
    not_applicable: &[],
};

/// The block with a hole of diameter `d` bored into the middle of its top: right through when it is 10 deep or more -
/// the wall alone - blind otherwise, which adds its floor. The wall is one cylindrical face closed by a seam, so it
/// brings three edges either way: the circle at each end and the seam between them.
pub(crate) const fn holed(d: f64, depth: f64) -> Outcome {
    let through = depth >= 10.0;
    let taken = PI * d * d / 4.0 * if through { 10.0 } else { depth };
    Outcome::Body { volume: 12000.0 - taken, faces: if through { 7 } else { 8 }, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

pub(crate) fn hole_across(d: f64) -> Outcome {
    holed(d, 15.0)
}

pub(crate) fn hole_deep(depth: f64) -> Outcome {
    holed(6.0, depth)
}

/// HOLE: a hole 6 across bored square into the face clicked, 15 deep by default - right through a block 10 thick.
pub static HOLE: Tool = Tool {
    id: "part.hole",
    flow: Flow::Command,
    title: "cmd-hole",
    entries: &[Entry::Button("tb-hole-hint"), Entry::Key(Modifiers::NONE, Key::O), Entry::Search("cmd-hole")],
    other: (Entry::Button("tb-shell-hint"), "cmd-shell"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[
        Field { caption: "f-diameter", by_placeholder: false, when: When::Before, class: Class::Length, typical: 6.0, lo: 0.1, hi: 29.0, zero: false, negative: false, outcome: hole_across },
        Field { caption: "f-depth", by_placeholder: false, when: When::Before, class: Class::Length, typical: 15.0, lo: 0.1, hi: 10000.0, zero: false, negative: false, outcome: hole_deep },
    ],
    modes: &[
        // placement: by a face is the one the ordinary picks are made for; by a sketch must ask for a sketch in words
        &[Mode { word: "cmd-by-face", clicks: None, outcome: Some(holed(6.0, 15.0)) }, Mode { word: "cmd-by-sketch", clicks: None, outcome: None }],
        // kind: a recess 12 across and 4 deep opens over the hole - a flat shoulder for a counterbore, a cone for a
        // countersink; each takes what the hole of 6 has not taken there already
        &[
            Mode { word: "cmd-simple", clicks: None, outcome: Some(holed(6.0, 15.0)) },
            Mode {
                word: "cmd-counterbore",
                clicks: None,
                outcome: Some(Outcome::Body { volume: 12000.0 - PI * 9.0 * 10.0 - PI * (36.0 - 9.0) * 4.0, faces: 9, edges: 18, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }),
            },
            Mode {
                word: "cmd-countersink",
                clicks: None,
                outcome: Some(Outcome::Body {
                    volume: 12000.0 - PI * 9.0 * 10.0 - PI * 4.0 / 3.0 * (36.0 + 18.0 + 9.0) + PI * 9.0 * 4.0,
                    faces: 8,
                    edges: 17,
                    min: [0.0, 0.0, 0.0],
                    max: [40.0, 30.0, 10.0],
                }),
            },
        ],
    ],
    result: holed(6.0, 15.0),
    node: "Hole",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0 - PI * 9.0 * 15.0, faces: 8, edges: 15, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart, Context::AfterTool(&EXTRUDE)],
    refusal: Some(60.0),
    budget: (30, 2000),
    help: "part/08-hole",
    not_applicable: &[],
};

/// The block with the wall `t` standing outside its surface, less `cavity` inside it: the five faces of the block
/// under the open top move out by `t` (5200 mm^2 of them at t = 1), the eight edges between them become quarter
/// cylinders and the four bottom corners eighths of a sphere. The faces meeting the open top neither move nor round.
pub(crate) const fn offset_wall(t: f64, cavity: f64) -> Outcome {
    let grown = 12000.0 + 2600.0 * t + 180.0 * PI * t * t / 4.0 + 4.0 * PI * t * t * t / 6.0;
    Outcome::Body { volume: grown - cavity, faces: 23, edges: 48, min: [-t, -t, -t], max: [40.0 + t, 30.0 + t, 10.0] }
}

/// The block hollowed to the wall `t`, open at its top: the cavity is a box of the wall inside it, and the top
/// becomes a rim. Five faces of the cavity stand beside the six of the block; the rim keeps its own inner loop.
pub(crate) const fn shelled(t: f64) -> Outcome {
    Outcome::Body { volume: 12000.0 - (40.0 - 2.0 * t) * (30.0 - 2.0 * t) * (10.0 - t), faces: 11, edges: 24, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

/// SHELL: the block hollowed through its top, the wall 2 by default. The wall goes inwards from the surface, or
/// outwards, or half each way, which moves the box of the body accordingly.
pub static SHELL: Tool = Tool {
    id: "part.shell",
    flow: Flow::Command,
    title: "cmd-shell",
    entries: &[Entry::Button("tb-shell-hint"), Entry::Key(Modifiers::NONE, Key::H), Entry::Search("cmd-shell")],
    other: (Entry::Button("tb-hole-hint"), "cmd-hole"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-thickness", by_placeholder: false, when: When::Before, class: Class::Radius, typical: 2.0, lo: 0.1, hi: 9.5, zero: false, negative: false, outcome: shelled }],
    modes: &[&[
        Mode { word: "cmd-inwards", clicks: None, outcome: Some(shelled(2.0)) },
        // outwards the old surface becomes the cavity and the wall stands outside it, following the surface: each
        // face moves out by the wall, each edge between two of them becomes a quarter cylinder of that radius and
        // each corner an eighth of a sphere, so the body grows by area * t + length * pi t^2 / 4 + 4 * pi t^3 / 6.
        // The face left open neither moves nor rounds, so only the bottom corners round: 5200 + 180 pi + 16 pi / 3.
        Mode { word: "cmd-outwards", clicks: None, outcome: Some(offset_wall(2.0, 12000.0)) },
        // centred: half the wall outside the surface, half inside - the outer side rounds the same way, and the
        // cavity is the block short of half the wall on every face but the open one
        Mode { word: "cmd-centred", clicks: None, outcome: Some(offset_wall(1.0, 38.0 * 28.0 * 9.0)) },
    ]],
    result: shelled(2.0),
    node: "Shell",
    undo: "f-shell",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0 - 36.0 * 26.0 * 18.0, faces: 11, edges: 24, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart, Context::AfterTool(&EXTRUDE)],
    refusal: Some(10.5),
    budget: (30, 2000),
    help: "part/07-shell",
    not_applicable: &[],
};

/// The rectangle turned a whole turn about the X axis it stands on: a cylinder of radius 30 and length 40 - its
/// side, its two ends, and the seam with the circle at each end.
pub(crate) const fn whole_turn() -> Outcome {
    Outcome::Body { volume: PI * 900.0 * 40.0, faces: 3, edges: 3, min: [0.0, -30.0, -30.0], max: [40.0, 30.0, 30.0] }
}

/// The same turned by `angle`: part of a turn is a wedge of that cylinder, cut by a plane at each end of the sweep,
/// and those two planes meet on the axis.
pub(crate) fn turned(angle: f64) -> Outcome {
    if angle >= 360.0 {
        return whole_turn();
    }
    let (sin, cos) = (angle.to_radians().sin(), angle.to_radians().cos());
    let (y0, y1) = if angle > 90.0 { (30.0 * cos.min(0.0), 30.0) } else { (0.0, 30.0) };
    // past a quarter turn the far corner has gone over the top: the height is the whole 30
    let (z0, z1) = if angle > 180.0 {
        (-30.0, 30.0)
    } else if angle >= 90.0 {
        (0.0, 30.0)
    } else {
        (0.0, 30.0 * sin.max(0.0))
    };
    Outcome::Body { volume: PI * 900.0 * 40.0 * angle / 360.0, faces: 5, edges: 9, min: [0.0, y0, z0], max: [40.0, y1, z1] }
}

/// REVOLVE: the rectangle of the sketch turned about the axis it stands on, a whole turn by default.
pub static REVOLVE: Tool = Tool {
    id: "part.revolve",
    flow: Flow::Command,
    title: "cmd-revolve",
    entries: &[Entry::Button("tb-revolve-hint"), Entry::Key(Modifiers::NONE, Key::R), Entry::Search("cmd-revolve")],
    other: (Entry::Button("tb-extrude-hint"), "cmd-extrude"),
    fixture: Fixture::RectangleSketch,
    picks: &[],
    pick_trial: &[Pick::Space([20.0, 15.0, 0.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-angle", by_placeholder: false, when: When::Before, class: Class::Angle, typical: 360.0, lo: 1.0, hi: 360.0, zero: false, negative: false, outcome: turned }],
    modes: &[
        // a first body can only be added: there is nothing yet to cut from or to meet
        &[
            Mode { word: "cmd-add", clicks: None, outcome: Some(whole_turn()) },
            Mode { word: "cmd-cut", clicks: None, outcome: None },
            Mode { word: "cmd-intersect", clicks: None, outcome: None },
        ],
        // a whole turn comes out the same laid to one side, laid about the middle, or reversed
        &[Mode { word: "cmd-one-side", clicks: None, outcome: Some(whole_turn()) }, Mode { word: "cmd-symmetric", clicks: None, outcome: Some(whole_turn()) }],
        &[Mode { word: "cmd-flip-btn", clicks: None, outcome: Some(whole_turn()) }],
    ],
    result: whole_turn(),
    node: "Revolve",
    undo: "f-revolution",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::SketchDimension { shown: "40", value: 50.0, then: Outcome::Body { volume: PI * 900.0 * 50.0, faces: 3, edges: 3, min: [0.0, -30.0, -30.0], max: [50.0, 30.0, 30.0] } }),
    dependency: Some(Node { kind: "Sketch", row: None }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/02-revolve",
    not_applicable: &[(
        17,
        "every angle of 1 to 360 degrees turns a contour lying beside its axis into a body; what the kernel cannot build is a contour lying across the axis, which is a matter of what is drawn, not of the value typed",
    )],
};
