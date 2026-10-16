//! The datum plane, axis and point.

use super::*;

/// The block as it was: a datum is drawn beside the body and takes nothing from it.
pub(crate) const fn block_alone(_value: f64) -> Outcome {
    Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }
}

/// DATUM_PLANE: a plane 10 above the top face of the block, to build from.
pub static DATUM_PLANE: Tool = Tool {
    id: "part.datum-plane",
    flow: Flow::Command,
    title: "cmd-plane",
    entries: &[Entry::Button("g-datum-plane-hint"), Entry::Key(Modifiers::NONE, Key::D), Entry::Search("cmdname-datum-plane")],
    other: (Entry::Button("g-datum-point-hint"), "cmd-point"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    // a datum plane may sit on the face it is taken from (0) or under it (a negative offset), so both are values of
    // its own; the body is the same whatever the plane does, and where the plane stands is read by what is built on it
    // its tilts about its X and Y and its turn about its normal take any angle either way, and the body stays as it is
    fields: &[
        Field { caption: "f-offset", by_placeholder: false, when: When::Before, class: Class::Length, typical: 10.0, lo: -100000.0, hi: 100000.0, zero: true, negative: true, outcome: block_alone },
        Field { caption: "f-tilt-x", by_placeholder: false, when: When::Before, class: Class::Angle, typical: 15.0, lo: -360.0, hi: 360.0, zero: true, negative: true, outcome: block_alone },
        Field { caption: "f-tilt-y", by_placeholder: false, when: When::Before, class: Class::Angle, typical: 15.0, lo: -360.0, hi: 360.0, zero: true, negative: true, outcome: block_alone },
        Field { caption: "f-turn", by_placeholder: false, when: When::Before, class: Class::Angle, typical: 30.0, lo: -360.0, hi: 360.0, zero: true, negative: true, outcome: block_alone },
    ],
    modes: &[],
    result: block_alone(10.0),
    node: "Plane",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "general/04-datums",
    not_applicable: &[
        (2, "the bar of the datum plane holds no modes: it takes a base and an offset from it"),
        (17, "a plane at an offset from another always builds: there is no value of the right form the kernel cannot make a plane at"),
    ],
};

/// DATUM_AXIS: the axis along an edge of the block.
pub static DATUM_AXIS: Tool = Tool {
    id: "part.datum-axis",
    flow: Flow::Command,
    title: "cmd-axis",
    entries: &[Entry::Button("g-datum-axis-hint"), Entry::Search("cmdname-datum-axis")],
    other: (Entry::Button("g-datum-plane-hint"), "cmd-plane"),
    fixture: Fixture::Block,
    picks: &[Pick::Edge([20.0, 0.0, 10.0])],
    pick_trial: &[Pick::Edge([20.0, 0.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: block_alone(0.0),
    node: "DatumAxis",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "general/04-datums",
    not_applicable: &[
        (5, "the axis takes no value: it lies along the edge or the cylinder picked"),
        (6, "the axis takes no value to refuse"),
        (17, "an axis along an edge always builds: there is no value to give it that the kernel could not make"),
    ],
};

/// DATUM_POINT: a point at the coordinates typed, zero by zero by zero unless a vertex is picked.
pub static DATUM_POINT: Tool = Tool {
    id: "part.datum-point",
    flow: Flow::Command,
    title: "cmd-point",
    entries: &[Entry::Button("g-datum-point-hint"), Entry::Search("cmdname-datum-point")],
    other: (Entry::Button("g-datum-plane-hint"), "cmd-plane"),
    fixture: Fixture::Block,
    // the point is placed by its coordinates; a vertex clicked only fills them in
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[
        Field { caption: "X", by_placeholder: false, when: When::Before, class: Class::Length, typical: 0.0, lo: -10000000.0, hi: 10000000.0, zero: true, negative: true, outcome: block_alone },
        Field { caption: "Y", by_placeholder: false, when: When::Before, class: Class::Length, typical: 0.0, lo: -10000000.0, hi: 10000000.0, zero: true, negative: true, outcome: block_alone },
        Field { caption: "Z", by_placeholder: false, when: When::Before, class: Class::Length, typical: 0.0, lo: -10000000.0, hi: 10000000.0, zero: true, negative: true, outcome: block_alone },
    ],
    modes: &[],
    result: block_alone(0.0),
    node: "DatumPoint",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 24000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 20.0] },
    }),
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "general/04-datums",
    not_applicable: &[
        (2, "the bar of the datum point holds no modes: it takes three coordinates"),
        (3, "the point is placed by its coordinates and takes no pick of its own; a vertex clicked only fills them in"),
        (4, "with nothing to pick there is no wrong pick: a click in space puts the point where it points"),
        (15, "a point at coordinates stands on nothing of the timeline, so there is nothing to delete under it"),
        (17, "a point at coordinates always builds"),
    ],
};
