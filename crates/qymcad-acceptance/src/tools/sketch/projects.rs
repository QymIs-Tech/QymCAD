//! The tools that bring geometry in from the bodies: an edge, a face, a whole body.

use super::*;

/// The edge of the block along X brought into the sketch on its top face: one line from (0, 0) to (40, 0), driven by
/// the part - it follows the body and cannot be dragged, so it leaves no degree of freedom.
pub(crate) const PROJECTED_EDGE: Outcome = Outcome::Sketch {
    points: 2,
    lines: 1,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: Some(0),
    box_of: Some(([0.0, 0.0], [40.0, 0.0])),
    size_of: None,
    under: &[(20.0, 0.0, Under::Line)],
};

/// The whole outline of the top face brought in at once: the four edges of the 40 x 30 rectangle, driven.
pub(crate) const PROJECTED_FACE: Outcome = Outcome::Sketch {
    points: 4,
    lines: 4,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: Some(0),
    box_of: Some(([0.0, 0.0], [40.0, 30.0])),
    size_of: None,
    under: &[(20.0, 0.0, Under::Line), (40.0, 15.0, Under::Line)],
};

/// What a projection leaves out of the contract: it takes the edge it projects by the click itself, has no field and
/// makes nothing between clicks.
pub(crate) const PROJECTS: &[(u8, &str)] = &[
    (3, "the edge is taken by the click that projects it, not by a pick before it"),
    (4, "a click away from an edge is answered by the tool itself, in words: see the drawing tools"),
    (5, "the projection has no field: what it brings in is given by the click"),
    (6, "the projection has no field to refuse a value in"),
    (7, "the edge is brought in by one click: there is nothing drawn between clicks to follow the pointer"),
    DRAWS[2],
    (14, "a change above the sketch reopens the part's timeline, which the open sketch stands in front of: the edge following the part is the matter of the sketch's own contract"),
    (15, "the body under the open sketch is deleted from the tree, and the open sketch puts its own list where the tree stands"),
    DRAWS[5],
];

/// PROJECT_BODY: the edge of the block along X clicked in the sketch on its top face.
pub static PROJECT_BODY: Tool = Tool {
    id: "sketch.project-body",
    flow: Flow::Drawing(&[(20.0, 0.0)], Finish::LastClick),
    title: "tool-project-body",
    entries: &[Entry::Button("tb-project-body-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnBlockTop,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "opt-edge", clicks: None, outcome: Some(PROJECTED_EDGE) },
        Mode { word: "opt-face-outline", clicks: Some(&[(20.0, 15.0)]), outcome: Some(PROJECTED_FACE) },
    ]],
    result: PROJECTED_EDGE,
    node: "Sketch",
    undo: "sk-project",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // one tool, one article: the projection of edges and of a body are the same tool (the owner's word of 26.09)
    help: "sketch/15-project",
    not_applicable: PROJECTS,
};
