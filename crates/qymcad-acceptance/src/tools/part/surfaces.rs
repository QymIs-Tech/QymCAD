//! The tools of faces and surfaces: copy a face, offset, remove, boolean, patch, sketch on a face, stitch, replace, trim.

use super::*;

/// FACE_COPY: the top face of the block taken out as a surface of its own - the block stays whole, and the sheet
/// beside it holds no volume.
pub static FACE_COPY: Tool = Tool {
    id: "part.face-copy",
    flow: Flow::Command,
    title: "feat-name-face-copy",
    entries: &[Entry::Button("tb-face-copy-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Bodies { count: 2, volume: 12000.0, largest: 12000.0 },
    node: "FaceCopy",
    // the step of undo is named by the tool, as the window writes it: "Copy face"
    undo: "f-face-copy",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Bodies { count: 2, volume: 24000.0, largest: 24000.0 },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/20-face-copy",
    not_applicable: &[
        (2, "the bar of the face copy holds no modes: it takes the faces to copy and nothing else"),
        (5, "the face copy takes no value: it repeats the face picked"),
        (6, "the face copy takes no value to refuse"),
        (17, "a face the kernel can draw it can copy: there is no value to give it that it could not make"),
    ],
};

/// The top of the block, 40 x 30, lifted `d`: a sheet of 1200 at z 10 + d.
pub(crate) fn top_lifted(d: f64) -> Outcome {
    Outcome::Sheet { area: 1200.0, min: [0.0, 0.0, 10.0 + d], max: [40.0, 30.0, 10.0 + d] }
}

/// OFFSET_SURFACE: the top face of the block taken out as a sheet lifted 5 - the block stays whole, the sheet of 1200
/// stands at z 15. The distance is a length that may be zero (a copy in place) or below it (into the block).
pub static OFFSET_SURFACE: Tool = Tool {
    id: "part.offset-surface",
    flow: Flow::Command,
    title: "feat-name-offset-surface",
    entries: &[Entry::Button("tb-offset-surface-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "f-distance", by_placeholder: false, when: When::Before, class: Class::Length, typical: 5.0, lo: -1000.0, hi: 1000.0, zero: true, negative: true, outcome: top_lifted }],
    modes: &[],
    result: Outcome::Sheet { area: 1200.0, min: [0.0, 0.0, 15.0], max: [40.0, 30.0, 15.0] },
    node: "OffsetSurface",
    undo: "feat-name-offset-surface",
    undo_steps: 1,
    stays: false,
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Extrude", row: Some("cmd-extrude") },
        caption: "f-length",
        value: 20.0,
        then: Outcome::Sheet { area: 1200.0, min: [0.0, 0.0, 25.0], max: [40.0, 30.0, 25.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/27-offset-surface",
    not_applicable: &[
        (2, "the bar of the offset holds no modes: the side is the sign of the distance"),
        (17, "a flat face moves any distance: the kernel refuses only a curved face turned inside out, which the block has none of"),
    ],
};

/// REMOVE_FACE: the wall of the hole taken away, the neighbouring faces closed over it - the block as it was before
/// the hole was made.
pub static REMOVE_FACE: Tool = Tool {
    id: "part.remove-face",
    flow: Flow::Command,
    title: "feat-name-remove-face",
    entries: &[Entry::Button("tb-remove-face-hint"), Entry::Search("feat-name-remove-face")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockWithHole,
    picks: &[Pick::Face([20.0, 25.0, 5.0])],
    pick_trial: &[Pick::Face([20.0, 25.0, 5.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] },
    node: "RemoveFace",
    undo: "f-remove-face",
    undo_steps: 1,
    stays: false,
    // the hole above it is opened wider: its wall is another wall, and taking it away gives the same plain block.
    // The extrusion is not the node to change here - a block made 20 thick leaves the hole of 15 blind, and a blind
    // hole is a wall AND a bottom, which is a different feature from the one the pick named
    upstream: Some(Upstream::Reopen {
        node: Node { kind: "Hole", row: Some("feat-name-hole") },
        // 24 across still stands clear of the 30 the block is wide; a hole wider than that would cut the sides away
        // and leave no wall to take
        caption: "f-diameter",
        value: 24.0,
        then: Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] },
    }),
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/12-remove-face",
    not_applicable: &[
        (2, "the bar of remove face holds no modes: it takes the faces of the feature to take away"),
        (5, "remove face takes no value: it takes the faces picked away"),
        (6, "remove face takes no value to refuse"),
        (17, "a wall the kernel made it can take away; a face it cannot close over is a matter of the pick, not of a value"),
    ],
};

/// BOOLEAN: body A (the upper half of the cut block, picked beforehand) and body B (the lower half grown by a boss that
/// reaches 1000 into A, clicked with the tool in hand). Taking B away from A leaves A with the boss's pocket, 5000, and
/// uses B up; joining them gives the whole block back, its faces merged across the seam; their common part is the
/// 1000 where the boss runs into A. Pieces that only touch are refused in words (`part_booleans`).
pub static BOOLEAN: Tool = Tool {
    id: "part.boolean",
    flow: Flow::Command,
    title: "bool-bodies-btn",
    entries: &[Entry::Button("tb-bool-bodies-hint")],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::OverlappingPiecesUpperPicked,
    picks: &[Pick::Face([20.0, 0.0, 2.5])],
    pick_trial: &[Pick::Face([20.0, 0.0, 2.5])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "f-cut-ab", clicks: None, outcome: Some(Outcome::Bodies { count: 1, volume: 5000.0, largest: 5000.0 }) },
        Mode { word: "f-union", clicks: None, outcome: Some(Outcome::Body { volume: 12000.0, faces: 6, edges: 12, min: [0.0, 0.0, 0.0], max: [40.0, 30.0, 10.0] }) },
        Mode { word: "f-intersect", clicks: None, outcome: Some(Outcome::Bodies { count: 1, volume: 1000.0, largest: 1000.0 }) },
    ]],
    result: Outcome::Bodies { count: 1, volume: 5000.0, largest: 5000.0 },
    node: "BodyBoolean",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "SplitBody", row: Some("feat-name-split-body") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/25-boolean",
    not_applicable: &[
        (5, "the boolean takes no value: its kind is a mode of the bar, and its bodies are picked"),
        (6, "the boolean takes no value to refuse"),
        (14, "above it stand the cut that made its two bodies and the boss that grew one into the other; their values are their own point 14, and this one follows them as the result does"),
        (17, "pieces that only touch are refused in words before a node, by the cut and by the intersection alike (part_booleans)"),
    ],
};

/// The block with its hole and a flat disc of surface over the top of the hole: the disc holds no volume, the body keeps
/// its 8858.41.
pub(crate) const PATCHED: Outcome = Outcome::Bodies { count: 2, volume: 8858.407346410208, largest: 8858.407346410208 };

/// PATCH: a surface spanned across the top edge of the hole - one closed circle, which is a boundary by itself, as in
/// the professional systems. On a flat circle "by position" and "smooth" give the same disc.
pub static PATCH: Tool = Tool {
    id: "part.patch",
    flow: Flow::Command,
    title: "feat-name-patch",
    entries: &[Entry::Button("tb-patch-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockWithHole,
    picks: &[Pick::Edge([30.0, 15.0, 10.0])],
    pick_trial: &[Pick::Edge([30.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[Mode { word: "cmd-patch-flat", clicks: None, outcome: Some(PATCHED) }, Mode { word: "cmd-patch-tangent", clicks: None, outcome: Some(PATCHED) }]],
    result: PATCHED,
    node: "Patch",
    undo: "f-patch",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "Hole", row: Some("feat-name-hole") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/22-patch",
    not_applicable: &[
        (5, "the patch takes no value: its surface is given by the edges picked and the mode"),
        (6, "the patch takes no value to refuse"),
        (14, "the hole under it is changed in its own contract, and the disc follows the edge it was spanned on - which point 15 checks from the other side"),
        (17, "edges that do not close a boundary are refused by the picks, not by a value"),
    ],
};

/// SKETCH_ON_FACE: the pencil, then a click on the top face of the block - a sketch opens on that face, empty, a node
/// of the timeline standing on the extrusion under it.
pub static SKETCH_ON_FACE: Tool = Tool {
    id: "part.sketch-on-face",
    flow: Flow::Command,
    title: "status-new-sketch",
    entries: &[Entry::Button("g-sketch-pick-hint"), Entry::Key(Modifiers::NONE, Key::K)],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::Block,
    picks: &[Pick::Face([20.0, 15.0, 10.0])],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: crate::contract::EMPTY_SKETCH,
    node: "Sketch",
    undo: "status-new-sketch",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "general/04-datums",
    not_applicable: &[
        (2, "the pencil has no bar of modes: a click on a plane, a datum or a face is the whole of it"),
        (3, "the face is taken by the click that opens the sketch on it: there is no taking it back but leaving the sketch"),
        (5, "the pencil takes no value"),
        (6, "the pencil takes no value to refuse"),
        (7, "the sketch opens on the click: there is nothing to show before it"),
        (8, "the sketch is opened by the click on the face, and it is left by \"Finish\", not applied with Enter"),
        (9, "Esc before the click puts the pencil down, which is point 1; after it the sketch is open and left by \"Finish\""),
        (13, "the sketch is reopened by a double click on its row, which is the sketch's own contract"),
        (14, "a change above is carried by the sketch on the face, which is the matter of in-context references"),
        (17, "any flat face can carry a sketch: there is nothing the kernel could refuse"),
    ],
};

/// The block and the one surface the two copied faces make once stitched along the edge they meet at.
pub(crate) const fn stitched(_tol: f64) -> Outcome {
    Outcome::Bodies { count: 2, volume: 12000.0, largest: 12000.0 }
}

/// STITCH: the two sheets clicked in turn and Enter - one surface of the two, the pieces used up; the block beside it
/// is untouched.
pub static STITCH: Tool = Tool {
    id: "part.stitch",
    flow: Flow::Command,
    title: "f-stitch",
    entries: &[Entry::Button("tb-stitch-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockAndTwoSheets,
    picks: &[Pick::Face([20.0, 15.0, 10.0]), Pick::Face([20.0, 0.0, 5.0])],
    pick_trial: &[Pick::Face([20.0, 15.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    // how far apart two edges may be and still be one seam: the sheets meet exactly, so any tolerance joins them
    fields: &[Field {
        caption: "f-stitch-tol",
        by_placeholder: false,
        when: When::Before,
        class: Class::Tolerance,
        typical: 0.01,
        lo: 0.000001,
        hi: 10.0,
        zero: false,
        negative: false,
        outcome: stitched,
    }],
    modes: &[],
    result: stitched(0.01),
    node: "Stitch",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/23-stitch",
    not_applicable: &[
        (2, "the stitch has no modes: the sheets and the tolerance are the whole of it"),
        (14, "the change above is a change of the copies it is made of, which is the face copy's own point 14"),
        (17, "sheets that do not meet are refused by the picks, not by a value"),
    ],
};

/// REPLACE_FACE: the top face of the block and the sheet copied from it - the face takes the sheet's shape, which is its
/// own, so the block is what it was; the sheet handed back is used up, the other one stays.
pub static REPLACE_FACE: Tool = Tool {
    id: "part.replace-face",
    flow: Flow::Command,
    title: "f-surface-replace",
    entries: &[Entry::Button("tb-surface-replace-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockAndTwoSheets,
    picks: &[Pick::Face([20.0, 15.0, 10.0]), Pick::Face([20.0, 15.0, 10.0])],
    // the right face: no sheet lies on it, so the second click lands on the same face - on the top a sheet lies over
    // the face, and the second click there rightly takes the sheet as the surface
    pick_trial: &[Pick::Face([40.0, 15.0, 5.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Bodies { count: 2, volume: 12000.0, largest: 12000.0 },
    node: "SurfaceReplace",
    undo: "f-surface-replace",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/21-surface-replace",
    not_applicable: &[
        (2, "replacing a face has no modes: the face and the surface are the whole of it"),
        (5, "replacing a face takes no value"),
        (6, "replacing a face takes no value to refuse"),
        (14, "the change above is a change of the copy it hands back, which is the face copy's own point 14"),
        (17, "a surface that does not cover the face is refused by the picks, not by a value"),
    ],
};

/// The block with the boss 10 across and 10 tall on its top.
pub(crate) const BOSSED: f64 = 12000.0 + PI * 25.0 * 10.0;

/// TRIM_SURFACE: the sheet copied off the top face clicked where it is kept, off the boss, then the top of the boss
/// clicked as the body to cut with, and Enter - the disc under the boss is cut out of the sheet; the part stays one
/// body of 12000 + pi x 5^2 x 10 = 12785.4 with the trimmed sheet beside it.
pub static TRIM_SURFACE: Tool = Tool {
    id: "part.trim-surface",
    flow: Flow::Command,
    title: "feat-name-trim",
    entries: &[Entry::Button("tb-trim-surface-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-mirror-body-hint"), "cmd-mirror"),
    fixture: Fixture::BlockSheetAndBoss,
    picks: &[Pick::Face([5.0, 5.0, 10.0]), Pick::Face([20.0, 15.0, 20.0])],
    pick_trial: &[Pick::Face([5.0, 5.0, 10.0])],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Bodies { count: 2, volume: BOSSED, largest: BOSSED },
    node: "Trim",
    undo: "f-operation",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: Some(Node { kind: "Extrude", row: Some("cmd-extrude") }),
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (30, 2000),
    help: "part/24-trim",
    not_applicable: &[
        (2, "the trim has no modes: the side kept and the body to cut with are the whole of it"),
        (5, "the trim takes no value: where it cuts is given by the body"),
        (6, "the trim takes no value to refuse"),
        (14, "the change above is a change of the copy it cuts, which is the face copy's own point 14"),
        (17, "a body that does not reach the sheet is refused by the picks, not by a value"),
    ],
};
