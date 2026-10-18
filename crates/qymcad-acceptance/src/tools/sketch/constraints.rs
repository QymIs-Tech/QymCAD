//! The tools of constraints, and construction geometry.

use super::*;

/// What a constraint leaves out of the contract: it is an action on the geometry picked before it (a person picks,
/// Shift-picks the second, presses the button), it has no field and no bar, and nothing stands over it but its sketch.
pub(crate) const CONSTRAINS: &[(u8, &str)] = &[
    (2, "a constraint has no bar of options: its button does the work"),
    (3, "the geometry is picked before the button, and the pick is what the result is read on"),
    (4, "a constraint on geometry it cannot hold is refused in words - that is the matter of the constraint matrix"),
    (5, "a constraint has no field"),
    (6, "a constraint has no field to refuse a value in"),
    (7, "a constraint is laid at once: there is nothing to show before it"),
    (9, "a constraint is laid at once: there is nothing to cancel, only to undo, which is point 11"),
    DRAWS[2],
    DRAWS[3],
    DRAWS[4],
    DRAWS[5],
    (19, "a constraint is not held, so F1 has no tool in hand to open an article of - and the help writes of no constraint at all"),
];

/// CON_HORIZONTAL: the slanted line of the two made level.
pub static CON_HORIZONTAL: Tool = Tool {
    id: "sketch.con-horizontal",
    flow: Flow::Action,
    title: "sk-horizontal",
    entries: &[Entry::Button("con-horizontal-hint"), Entry::Search("con-horizontal")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_VERTICAL: the slanted line made upright.
pub static CON_VERTICAL: Tool = Tool {
    id: "sketch.con-vertical",
    flow: Flow::Action,
    title: "sk-vertical",
    entries: &[Entry::Button("con-vertical-hint"), Entry::Search("con-vertical")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_PARALLEL: the two lines from the origin made parallel - one lies on the other.
pub static CON_PARALLEL: Tool = Tool {
    id: "sketch.con-parallel",
    flow: Flow::Action,
    title: "sk-parallel",
    entries: &[Entry::Button("con-parallel-hint"), Entry::Search("con-parallel")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_PERPENDICULAR: the two lines from the origin made square to each other.
pub static CON_PERPENDICULAR: Tool = Tool {
    id: "sketch.con-perpendicular",
    flow: Flow::Action,
    title: "sk-perpendicular",
    entries: &[Entry::Button("con-perpendicular-hint"), Entry::Search("con-perpendicular")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_EQUAL: the bottom and the left side of the 40 x 30 rectangle tied to one length - drawn unequal, so the sketch
/// did not tie them itself, and one degree of freedom of the rectangle's four goes.
pub static CON_EQUAL: Tool = Tool {
    id: "sketch.con-equal",
    flow: Flow::Action,
    title: "sk-equal",
    entries: &[Entry::Button("con-equal"), Entry::Search("con-equal")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[Pick::Sketch(20.0, 0.0), Pick::Sketch(0.0, 15.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // four corners and the centre of the rectangle
    result: Outcome::Sketch { points: 5, lines: 4, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: None, dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_COLLINEAR: the two lines from the origin put on one line.
pub static CON_COLLINEAR: Tool = Tool {
    id: "sketch.con-collinear",
    flow: Flow::Action,
    title: "con-collinear",
    entries: &[Entry::Button("con-collinear-hint"), Entry::Search("con-collinear")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_CONCENTRIC: the two circles given one centre.
pub static CON_CONCENTRIC: Tool = Tool {
    id: "sketch.con-concentric",
    flow: Flow::Action,
    title: "con-concentric",
    entries: &[Entry::Button("con-concentric-hint"), Entry::Search("con-concentric")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoCirclesInSketch,
    picks: &[Pick::Sketch(0.0, 5.0), Pick::Sketch(30.0, 5.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // the concentric alone: the circles drawn with no size typed carry none, and the centre and both radii are free
    result: Outcome::Sketch { points: 2, lines: 0, arcs: 0, circles: 2, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(2), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_TANGENT: the line across the circle made to touch it.
pub static CON_TANGENT: Tool = Tool {
    id: "sketch.con-tangent",
    flow: Flow::Action,
    title: "con-tangent",
    entries: &[Entry::Button("con-tangent-hint"), Entry::Search("con-tangent")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineThroughCircle,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(0.0, 10.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // the horizontal of the line and the tangent; the circle drawn with no size typed carries none
    result: Outcome::Sketch { points: 3, lines: 1, arcs: 0, circles: 1, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(2), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_MIDPOINT: the far end of the level line put in the middle of the slanted one - two degrees of freedom taken:
/// the point lands on the line, and halfway along it.
pub static CON_MIDPOINT: Tool = Tool {
    id: "sketch.con-midpoint",
    flow: Flow::Action,
    title: "sk-midpoint",
    entries: &[Entry::Button("con-midpoint-hint"), Entry::Search("con-midpoint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    // two lines of different lengths: of one length they are tied Equal as drawn, and the midpoint is then met only by
    // both shrinking to a point - which is refused
    fixture: Fixture::TwoLinesOfTwoLengths,
    picks: &[Pick::Sketch(30.0, 0.0), Pick::Sketch(5.0, 5.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_COINCIDENT: the far ends of the two lines made one point. The two lines were drawn 30 long each, and the sketch
/// tied them equal as they were drawn: with one end on the other, the equal length follows from the rest, so of the two
/// degrees of freedom a meeting takes one is taken already - 4 become 3.
pub static CON_COINCIDENT: Tool = Tool {
    id: "sketch.con-coincident",
    flow: Flow::Action,
    title: "sk-coincident",
    entries: &[Entry::Button("con-coincident-hint"), Entry::Search("con-coincident")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(30.0, 0.0), Pick::Sketch(21.21, 21.21)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_FIX: the far end of the level line fixed where it stands.
pub static CON_FIX: Tool = Tool {
    id: "sketch.con-fix",
    flow: Flow::Action,
    title: "sk-fix",
    entries: &[Entry::Button("con-fix"), Entry::Search("con-fix")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(30.0, 0.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CON_SYMMETRY: the two ends of the slanted line made symmetric about the upright line through the origin - two
/// degrees of freedom taken: the ends are mirror images of each other across the axis.
pub static CON_SYMMETRY: Tool = Tool {
    id: "sketch.con-symmetry",
    flow: Flow::Action,
    title: "sk-constraint",
    entries: &[Entry::Button("con-symmetric-hint"), Entry::Search("con-symmetric")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[Pick::Sketch(10.0, 5.0), Pick::Sketch(30.0, 25.0), Pick::Sketch(0.0, 10.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 4, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(4), box_of: None, size_of: None, under: &[] },
    node: "Sketch",
    // one step of undo named by the kind of edit, "Constraint", as the other constraints name theirs
    undo: "sk-constraint",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no constraint: there is no article to name
    help: "sketch/17-constraints",
    not_applicable: CONSTRAINS,
};

/// CONSTRUCTION: the picked bottom side of the rectangle turned into construction geometry - still a line, drawn
/// dashed, outside every profile.
pub static CONSTRUCTION: Tool = Tool {
    id: "sketch.construction",
    flow: Flow::Action,
    title: "tb-construction-hint",
    entries: &[Entry::Button("tb-construction-hint"), Entry::Key(Modifiers::NONE, Key::X), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleSidePicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Construction { count: 1, lines: 4 },
    node: "Sketch",
    undo: "tool-construction",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no construction geometry: there is no article to name
    help: "sketch/23-construction",
    not_applicable: ACTS,
};
