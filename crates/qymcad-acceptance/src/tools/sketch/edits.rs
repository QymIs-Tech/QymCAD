//! The edit menu on the sheet: copy, cut, insert.

use super::*;

/// What the items of the Edit menu leave out in a sketch: what they work on is picked before them.
pub(crate) const EDITS: &[(u8, &str)] = &[
    (2, "the item has no mode"),
    (3, "what is copied or cut is picked before the item; its clicks put points"),
    (4, "a click on empty sheet is a place like any other: there is no wrong click to refuse"),
    (5, "the item has no field"),
    (6, "the item has no field to refuse a value in"),
    DRAWS[2],
    DRAWS[3],
    DRAWS[4],
    DRAWS[5],
];

/// EDIT_COPY: Edit -> Copy on the picked bottom side: the base point on it, and the side is in the clipboard - the
/// sketch as it was, nothing placed and no step of undo; a paste places it (`EDIT_INSERT`).
pub static EDIT_COPY: Tool = Tool {
    id: "sketch.edit-copy",
    flow: Flow::Drawing(&[(20.0, 0.0)], Finish::LastClick),
    title: "tool-copy",
    entries: &[Entry::Menu(&["menu-edit", "menu-copy"])],
    // over Trim, not a drawing tool: a drawing tool taken drops the selection, and with nothing selected there is nothing
    // to copy
    other: (Entry::Button("tb-trim-hint"), "tool-trim"),
    fixture: Fixture::RectangleSidePicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: RECT_BY_CORNERS,
    node: "Sketch",
    undo: "tool-copy",
    undo_steps: 0,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: EDITS,
};

/// EDIT_CUT: Edit -> Cut on the picked bottom side: the base point clicked on it, and the side is gone to the
/// clipboard - three sides left.
pub static EDIT_CUT: Tool = Tool {
    id: "sketch.edit-cut",
    flow: Flow::Drawing(&[(20.0, 0.0)], Finish::LastClick),
    title: "menu-cut",
    entries: &[Entry::Menu(&["menu-edit", "menu-cut"])],
    // over Trim, not a drawing tool: a drawing tool taken drops the selection, and with nothing selected there is nothing
    // to copy
    other: (Entry::Button("tb-trim-hint"), "tool-trim"),
    fixture: Fixture::RectangleSidePicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 4,
        lines: 3,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, 0.0], [40.0, 30.0])),
        size_of: None,
        under: &[(20.0, 0.0, Under::Nothing), (40.0, 15.0, Under::Line)],
    },
    node: "Sketch",
    undo: "menu-cut",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: EDITS,
};

/// EDIT_INSERT: Edit -> Insert after the bottom side was cut: a click 30 below where it was puts it back there.
pub static EDIT_INSERT: Tool = Tool {
    id: "sketch.edit-insert",
    flow: Flow::Drawing(&[(20.0, -30.0)], Finish::LastClick),
    title: "win-insert",
    entries: &[Entry::Menu(&["menu-edit", "win-insert"])],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleSideCut,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 6,
        lines: 4,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, -30.0], [40.0, 30.0])),
        size_of: None,
        under: &[(20.0, -30.0, Under::Line), (20.0, 0.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "win-insert",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: EDITS,
};
