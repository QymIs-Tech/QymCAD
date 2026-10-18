//! The tools that read the part - section, measure, recognise - and the one that makes a piece a part.

use super::*;

/// SECTION: the section of the view, the top face of the block clicked as its plane - half the model is hidden and its
/// inside shows; the document is untouched.
pub static SECTION: Tool = Tool {
    id: "part.section",
    flow: Flow::Command,
    title: "sec-btn",
    entries: &[Entry::Button("tb-section-hint-bar"), Entry::Search("cmdname-section")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Section { on: true },
    node: "",
    undo: "tb-section-off",
    undo_steps: 0,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "general/09-viewport",
    not_applicable: &[
        (2, "the section's offset and tilts in the bar move the plane; where it cuts is not read yet"),
        (5, "the section takes its offset from the bar, and where it cuts is not read yet"),
        (6, "the section takes its offset from the bar, and where it cuts is not read yet"),
        (11, "a section is the view, not the document: it lays no step of undo"),
        (12, "a section is the view, not the document: saving and opening does not carry it"),
        (13, "a section lays no node"),
        (14, "nothing of a timeline stands above a section"),
        (15, "a section stands on nothing of the document"),
        (17, "a plane can always cut the view"),
    ],
};

/// What a measure leaves out: it reads, it does not make - no node, no step of undo, nothing saved.
pub(crate) const READS: &[(u8, &str)] = &[
    (2, "the measure has no modes: what is clicked decides what is measured"),
    (5, "the measure takes no value"),
    (6, "the measure takes no value to refuse"),
    (7, "the measure answers on the second click: there is nothing to show before it"),
    (8, "a measure changes nothing and has nothing to apply: its answer stands in the bar as the second element is clicked"),
    (11, "a measure changes nothing: it lays no step of undo"),
    (12, "a measure changes nothing: saving and opening carries nothing of it"),
    (13, "a measure lays no node"),
    (14, "nothing of a timeline stands above a measure"),
    (15, "a measure stands on nothing of the document"),
    (17, "two points can always be measured"),
];

/// MEASURE_3D: the two corners of the front top edge of the block clicked - they are 40 apart, and the status line
/// says so.
pub static MEASURE_3D: Tool = Tool {
    id: "part.measure-3d",
    flow: Flow::Command,
    title: "hotkey-part-i",
    entries: &[Entry::Button("tb-measure3d-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Vertex([0.0, 0.0, 10.0]), Pick::Vertex([40.0, 0.0, 10.0])],
    pick_trial: &[Pick::Vertex([0.0, 0.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Says { text: "40" },
    node: "",
    undo: "hotkey-part-i",
    undo_steps: 0,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "general/11-measure",
    not_applicable: READS,
};

/// RECOGNISE: the mesh of the block clicked and Enter - turned into a body again: by exact surfaces the box it came
/// from, six faces and twelve edges; as it is, a polyhedron of its triangles, the same material.
pub static RECOGNISE: Tool = Tool {
    id: "part.recognise",
    flow: Flow::Command,
    title: "f-recognise",
    entries: &[Entry::Button("tb-recognise-hint"), Entry::SearchByArticle],
    other: (Entry::Button("g-datum-plane-hint"), "cmd-plane"),
    fixture: Fixture::MeshOfBlock,
    // a mesh has no faces to name: the click is on the mesh itself
    picks: &[Pick::Space([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Space([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "cmd-recognise-exact", clicks: None, outcome: Some(Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }) },
        Mode { word: "cmd-recognise-asis", clicks: None, outcome: Some(Outcome::Bodies { count: 1, volume: 12000.0, largest: 12000.0 }) },
    ]],
    result: Outcome::Bodies { count: 1, volume: 12000.0, largest: 12000.0 },
    node: "MeshRecognised", // the node of the timeline is named by what it holds: a mesh recognised
    undo: "f-recognise",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[],
    refusal: None,
    budget: (60, 2000),
    help: "part/26-recognise",
    not_applicable: &[
        (5, "recognising takes the tolerances of the bar as they come; the values are the matrix of meshes' cases"),
        (6, "recognising takes the tolerances of the bar as they come; the values are the matrix of meshes' cases"),
        (14, "nothing of a timeline stands above an imported mesh"),
        (15, "the mesh stands on the file it came from, not on a node"),
        (16, "the fixture builds its own project from the start: there is no second part to take it in"),
        (17, "a mesh that holds no closed volume is refused by the pick, not by a value"),
    ],
};

/// Any name is two parts: the block split 5 below its top, and the upper piece taken into a part of its own.
pub(crate) fn two_parts(_: &str) -> Outcome {
    Outcome::Parts { parts: 2, grounded: 0, joints: 0, assemblies: 0 }
}

/// MAKE A PART: the upper of the two bodies a split left, picked by a double click, made a part of its own through the
/// command search - the name is asked beside the cursor and given with Enter.
pub static PIECE_TO_PART: Tool = Tool {
    id: "part.piece-to-part",
    flow: Flow::Action,
    title: "act-piece-to-part",
    entries: &[Entry::Search("act-piece-to-part")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::OverlappingPiecesUpperPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[Words {
        caption: "piece-part-name",
        by_placeholder: false,
        typical: "Upper",
        // an ordinary name, one of another alphabet, a long one
        valid: &["Upper", "\u{412}\u{435}\u{440}\u{445}", "the upper piece of the block split five below its top"],
        // nothing, and spaces alone, are no name
        invalid: &["", "   "],
        outcome: two_parts,
        enter: true,
    }],
    fields: &[],
    modes: &[],
    result: Outcome::Parts { parts: 2, grounded: 0, joints: 0, assemblies: 0 },
    node: "Piece",
    undo: "act-piece-to-part",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "SplitBody", row: Some("feat-name-split-body") }),
    contexts: &[],
    refusal: None,
    budget: (30, 2000),
    help: "part/13-split-body",
    not_applicable: &[
        (2, "the command holds no bar: it asks one name beside the cursor"),
        (3, "the piece is picked before the command, by a double click; the click taking it after is the command's own case of nothing picked"),
        (4, "the piece is picked before the command: there is no pick of its own to refuse"),
        (7, "a part made of a piece is the piece itself: there is nothing to show before it"),
        (13, "the node of the piece has no command to reopen: it reads the piece, and the cut that made it is edited instead"),
        (14, "above the piece stand the split and the boss; their values are their own point 14, and the piece follows them"),
        (16, "a piece is made a part in the part that shows it: there is no second place to take it"),
        (17, "a piece can always be made a part: the kernel only copies it"),
        (19, "the command is not held, so F1 has no tool in hand to open an article of"),
    ],
};
