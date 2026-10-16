//! The tools that repeat a body: mirror, the linear and the circular arrays.

use super::*;

/// The block and its reflection in its right face, joined along it: one block twice as long.
pub(crate) const fn doubled() -> Outcome {
    Outcome::Body { volume: 24000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [80.0, 30.0, 10.0] }
}

/// MIRROR: the block reflected in its own right face. The copy stands against the face and is joined to the body
/// along it, so a block doubled is a block. It takes no value: the body is clicked, then the plane.
pub static MIRROR: Tool = Tool {
    id: "part.mirror",
    flow: Flow::Command,
    title: "cmd-mirror",
    entries: &[Entry::Button("tb-mirror-body-hint"), Entry::Key(Modifiers::NONE, Key::M), Entry::SearchByArticle],
    other: (Entry::Button("tb-lin-array-body-hint"), "cmd-linear-array"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0]), Pick::Face([40.0, 15.0, 5.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0]), Pick::Face([40.0, 15.0, 5.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    // the original is kept by default; the switch turned off leaves the reflection alone, standing where it was
    modes: &[&[Mode { word: "cmd-with-original", clicks: None, outcome: Some(Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [40.0, 0.0, 0.0], max: [80.0, 30.0, 10.0] }) }]],
    result: doubled(),
    node: "Mirror",
    undo: "f-mirror",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 48000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [80.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/16-mirror",
    not_applicable: &[
        (5, "the mirror takes no value: what it makes is settled by the body and the plane clicked"),
        (6, "the mirror takes no value, so there is none to refuse"),
        (17, "a body reflected in a plane always builds: there is no value for the kernel to fail on"),
    ],
};

/// A ROW of three copies of the block along X, `pitch` apart. Nearer than its 40 they overlap into one block as long
/// as the row; at 40 they touch; farther apart each copy is a body of its own.
pub(crate) fn row_of_three(pitch: f64) -> Outcome {
    if pitch > 40.0 {
        return Outcome::Pieces { pieces: 3, volume: 36000.0 };
    }
    let length = 40.0 + 2.0 * pitch;
    Outcome::Body { volume: length * 300.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [length, 30.0, 10.0] }
}

/// The same row of `copies` at the ordinary pitch of 25, which overlaps: one block 40 + 25 (copies - 1) long.
pub(crate) const fn row_of(copies: f64) -> Outcome {
    let length = 40.0 + 25.0 * (copies - 1.0);
    Outcome::Body { volume: length * 300.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [length, 30.0, 10.0] }
}

/// LINEAR PATTERN: three copies of the block along X, 25 apart - nearer than the block is long, so they overlap into
/// one block 90 long.
pub static LINEAR_ARRAY: Tool = Tool {
    id: "part.linear-array",
    flow: Flow::Command,
    title: "cmd-linear-array",
    entries: &[Entry::Button("tb-lin-array-body-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[
        Field { caption: "f-pitch", by_placeholder: false, when: When::Before, class: Class::Length, typical: 25.0, lo: 0.01, hi: 100000.0, zero: false, negative: false, outcome: row_of_three },
        Field { caption: "cmd-copies", by_placeholder: false, when: When::Before, class: Class::Count, typical: 3.0, lo: 2.0, hi: 100.0, zero: false, negative: false, outcome: row_of },
    ],
    modes: &[],
    result: row_of(3.0),
    node: "LinearArray",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Body { volume: 90.0 * 30.0 * 20.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [90.0, 30.0, 20.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/17-linear-array",
    not_applicable: &[
        (2, "the bar of the pattern holds its count and the direction it goes, not modes that put each other down"),
        (17, "copies of a body laid along a line always build: what the kernel could not make of them is a matter of where they fall, not of the value typed"),
    ],
};

/// `copies` of the far block turned about the vertical axis. Seen from that axis the block is 5.7 degrees wide, so up
/// to 63 copies stand apart: each is a piece of its own, and they multiply the 40 x 20 x 10 block.
pub(crate) const fn ring_of(copies: f64) -> Outcome {
    Outcome::Pieces { pieces: copies as u32, volume: 8000.0 * copies }
}

/// CIRCULAR_ARRAY: six copies of the block turned the whole way about the vertical axis.
pub static CIRCULAR_ARRAY: Tool = Tool {
    id: "part.circular-array",
    flow: Flow::Command,
    title: "cmd-circular-array",
    entries: &[Entry::Button("tb-circ-array-body-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockOffAxis,
    picks: &[Pick::Face([220.0, 0.0, 10.0])],
    pick_trial: &[Pick::Face([220.0, 0.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "cmd-copies", by_placeholder: false, when: When::Before, class: Class::Count, typical: 6.0, lo: 2.0, hi: 52.0, zero: false, negative: false, outcome: ring_of }],
    // the whole circle switched off puts an angle field at the geometry, and 180 degrees over six copies still leaves
    // them 30 degrees apart - the same six pieces, standing over half the turn
    modes: &[&[Mode { word: "cmd-full-circle", clicks: None, outcome: Some(ring_of(6.0)) }]],
    result: ring_of(6.0),
    node: "CircularArray",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen { node: Node { kind: "Extrude", row: Some("cmd-extrude") }, caption: "f-length", value: 20.0, then: Outcome::Pieces { pieces: 6, volume: 96000.0 } }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/18-circular-array",
    not_applicable: &[(17, "copies of a body turned about an axis always build: what the kernel could not make of them is a matter of where they fall, not of the count typed")],
};
