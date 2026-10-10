//! THE TOOLS OF THE SKETCH.
use qymcad::{Key, Modifiers};

use crate::contract::fixtures::Fixture;
use crate::contract::{When, Class, Context, Entry, Field, Finish, Flow, Mode, Outcome, Pick, Tool, Under, Words};

/// Why the points of the contract that belong to a command do not apply to a tool that draws.
const DRAWS: &[(u8, &str)] = &[
    (3, "a drawing takes no reference: a click puts a point down, it does not pick something already there"),
    (4, "a drawing takes no reference, so there is no wrong one to refuse"),
    (13, "a drawn shape has no node in the tree to double-click: its dimensions are edited on the sheet (see the dimensions of B1)"),
    (14, "nothing of the timeline stands above a drawing but the plane of its sketch, which is checked with the sketch itself"),
    (15, "a drawing stands on its sketch alone: deleting the sketch takes the drawing with it, and that is checked with the sketch"),
    (17, "the kernel builds nothing here: a sketch is drawn in the plane, and the geometry holds whatever was clicked"),
];

/// The points above, and those of a tool with no field of its own.
const DRAWS_WITHOUT_FIELDS: &[(u8, &str)] = &[
    (3, DRAWS[0].1),
    (4, DRAWS[1].1),
    (5, "the tool has no field: what it draws is given by its clicks, and sized afterwards by a dimension"),
    (6, "the tool has no field to refuse a value in"),
    (13, DRAWS[2].1),
    (14, DRAWS[3].1),
    (15, DRAWS[4].1),
    (17, DRAWS[5].1),
];

/// A LINE, drawn as a chain: two segments of one chain, from the origin.
pub static LINE: Tool = Tool {
    id: "sketch.line",
    flow: Flow::Drawing(&[(0.0, 0.0), (40.0, 0.0), (40.0, 30.0)], Finish::LastClick),
    title: "tool-line",
    entries: &[Entry::Button("tb-line-hint"), Entry::Key(Modifiers::NONE, Key::L), Entry::Search("tool-line")],
    other: (Entry::Button("tb-rect-hint"), "tool-rect"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 3,
        lines: 2,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(20.0, 0.0, Under::Line), (40.0, 15.0, Under::Line), (0.0, 0.0, Under::Point), (20.0, 15.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-line",
    undo_steps: 2,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/01-line",
    not_applicable: &[
        (2, "the line has no mode of its own: it draws a chain of segments"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A 40 x 30 rectangle by its two corners, its centre a point of its own.
const RECT_BY_CORNERS: Outcome = Outcome::Sketch {
    points: 5,
    lines: 4,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(20.0, 0.0, Under::Line), (0.0, 30.0, Under::Point), (20.0, 15.0, Under::Point), (10.0, 8.0, Under::Nothing)],
};

/// The same rectangle from a centre at the origin: 80 x 60, its sides through the corner clicked, its centre a point of
/// its own and its two construction diagonals through it.
const RECT_FROM_CENTRE: Outcome = Outcome::Sketch {
    points: 5,
    lines: 6,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(0.0, 30.0, Under::Line), (40.0, 30.0, Under::Point), (20.0, 20.0, Under::Nothing)],
};

/// A rectangle by three points: a side from the first two, the height to the third; its centre a point of its own.
const RECT_BY_THREE: Outcome = Outcome::Sketch {
    points: 5,
    lines: 4,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(20.0, 0.0, Under::Line), (0.0, 0.0, Under::Point), (20.0, 10.0, Under::Point), (10.0, 5.0, Under::Nothing)],
};

/// A RECTANGLE: two corners, or a centre and a corner, or three points.
pub static RECT: Tool = Tool {
    id: "sketch.rect",
    flow: Flow::Drawing(&[(0.0, 0.0), (40.0, 30.0)], Finish::LastClick),
    title: "tool-rect",
    entries: &[Entry::Button("tb-rect-hint"), Entry::Key(Modifiers::NONE, Key::R), Entry::Search("tool-rect")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "opt-rect-2corners", clicks: None, outcome: Some(RECT_BY_CORNERS) },
        Mode { word: "opt-rect-centre", clicks: None, outcome: Some(RECT_FROM_CENTRE) },
        Mode { word: "opt-rect-3pt", clicks: Some(&[(0.0, 0.0), (40.0, 0.0), (40.0, 20.0)]), outcome: Some(RECT_BY_THREE) },
    ]],
    result: RECT_BY_CORNERS,
    node: "Sketch",
    undo: "sk-rect",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/02-rect",
    not_applicable: &[
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A circle of radius 10 about the origin.
const CIRCLE_BY_RADIUS: Outcome = Outcome::Sketch {
    points: 1,
    lines: 0,
    arcs: 0,
    circles: 1,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(10.0, 0.0, Under::Circle), (0.0, 0.0, Under::Point), (5.0, 0.0, Under::Nothing)],
};

/// A circle on the two points clicked as the ends of its diameter: centre (5, 0), radius 5.
const CIRCLE_BY_TWO: Outcome = Outcome::Sketch {
    points: 1,
    lines: 0,
    arcs: 0,
    circles: 1,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(10.0, 0.0, Under::Circle), (5.0, 3.0, Under::Nothing)],
};

/// A CIRCLE: a centre and a point on it, two ends of a diameter, or tangent to what is drawn.
pub static CIRCLE: Tool = Tool {
    id: "sketch.circle",
    flow: Flow::Drawing(&[(0.0, 0.0), (10.0, 0.0)], Finish::LastClick),
    title: "tool-circle",
    entries: &[Entry::Button("tb-circle-hint"), Entry::Key(Modifiers::NONE, Key::C), Entry::Search("tool-circle")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "opt-circle-centre-radius", clicks: None, outcome: Some(CIRCLE_BY_RADIUS) },
        Mode { word: "opt-circle-2pt", clicks: None, outcome: Some(CIRCLE_BY_TWO) },
        // there is nothing drawn to be tangent to, so the tool must say so rather than draw anything
        Mode { word: "opt-tangent", clicks: None, outcome: None },
    ]],
    result: CIRCLE_BY_RADIUS,
    node: "Sketch",
    undo: "sk-circle",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/03-circle",
    not_applicable: &[
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A POINT: one click puts a node of its own on the sheet.
pub static POINT: Tool = Tool {
    id: "sketch.point",
    flow: Flow::Drawing(&[(5.0, 5.0)], Finish::LastClick),
    title: "tool-point",
    entries: &[Entry::Button("tb-point-hint"), Entry::Key(Modifiers::NONE, Key::P), Entry::Search("tool-point")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 1,
        lines: 0,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(5.0, 5.0, Under::Point), (20.0, 20.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-point",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/05-point",
    not_applicable: &[
        (2, "a point has no mode: one click is the whole of it"),
        (7, "a point has nothing to show before it is put down: it appears where the click lands"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// An arc of radius 10 about the origin, from (10, 0) to (0, 10).
const ARC_BY_CENTRE: Outcome = Outcome::Sketch {
    points: 3,
    lines: 0,
    arcs: 1,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(7.07, 7.07, Under::Arc), (0.0, 0.0, Under::Point), (5.0, 5.0, Under::Nothing)],
};

/// An arc through three points: the ends at (0, 0) and (20, 0), the middle through (10, 5).
const ARC_BY_THREE: Outcome = Outcome::Sketch {
    points: 3,
    lines: 0,
    arcs: 1,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(10.0, 5.0, Under::Arc), (20.0, 0.0, Under::Point), (10.0, 0.0, Under::Nothing)],
};

/// AN ARC: a centre and two ends, or three points on it, or a smooth continuation of what is drawn.
pub static ARC: Tool = Tool {
    id: "sketch.arc",
    flow: Flow::Drawing(&[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)], Finish::LastClick),
    title: "tool-arc",
    entries: &[Entry::Button("tb-arc-hint"), Entry::Key(Modifiers::NONE, Key::A), Entry::Search("tool-arc")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[&[
        Mode { word: "opt-arc-cse", clicks: None, outcome: Some(ARC_BY_CENTRE) },
        Mode { word: "opt-rect-3pt", clicks: Some(&[(0.0, 0.0), (20.0, 0.0), (10.0, 5.0)]), outcome: Some(ARC_BY_THREE) },
        // there is nothing drawn to continue smoothly from, so the tool must say so rather than draw anything
        Mode { word: "opt-tangent", clicks: None, outcome: None },
    ]],
    result: ARC_BY_CENTRE,
    node: "Sketch",
    undo: "sk-arc",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/04-arc",
    not_applicable: &[
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A CIRCLE THROUGH THREE POINTS: the circle of radius 10 about the origin, through (10, 0), (0, 10), (-10, 0).
pub static CIRCLE3: Tool = Tool {
    id: "sketch.circle3",
    flow: Flow::Drawing(&[(10.0, 0.0), (0.0, 10.0), (-10.0, 0.0)], Finish::LastClick),
    title: "tool-circle-3pt",
    entries: &[Entry::Button("tb-circle-3pt"), Entry::Search("cmdname-circle-3pt")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 1,
        lines: 0,
        arcs: 0,
        circles: 1,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(0.0, -10.0, Under::Circle), (0.0, 0.0, Under::Point), (5.0, 5.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-circle",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/03-circle",
    not_applicable: &[
        (2, "the circle through three points has no mode: three clicks are the whole of it"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A polygon of `n` sides about the origin: its corners, the circle they sit on, and the corner clicked at (15, 0).
const fn polygon_shape(n: usize) -> Outcome {
    Outcome::Sketch {
        points: n + 1,
        lines: n,
        arcs: 0,
        circles: 1,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(15.0, 0.0, Under::Point), (0.0, 0.0, Under::Point)],
    }
}

/// A hexagon circumscribed about the click: the click is the middle of an edge, a line under it and no point.
const POLYGON_CIRCUMSCRIBED: Outcome = Outcome::Sketch {
    points: 7,
    lines: 6,
    arcs: 0,
    circles: 1,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: None,
    size_of: None,
    under: &[(15.0, 0.0, Under::Line), (0.0, 0.0, Under::Point)],
};

/// A polygon of as many sides as the field above says.
fn polygon_of(sides: f64) -> Outcome {
    polygon_shape(sides as usize)
}

/// A POLYGON: the number of sides above, then a centre and a corner.
pub static POLYGON: Tool = Tool {
    id: "sketch.polygon",
    flow: Flow::Drawing(&[(0.0, 0.0), (15.0, 0.0)], Finish::LastClick),
    title: "tool-polygon",
    entries: &[Entry::Button("tb-polygon-hint"), Entry::Key(Modifiers::NONE, Key::G), Entry::Search("tool-polygon")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "opt-sides", by_placeholder: false, when: When::Before, class: Class::Count, typical: 6.0, lo: 3.0, hi: 12.0, zero: false, negative: false, outcome: polygon_of }],
    modes: &[&[
        Mode { word: "opt-polygon-inscribed", clicks: None, outcome: Some(polygon_shape(6)) },
        Mode { word: "opt-polygon-circumscribed", clicks: None, outcome: Some(POLYGON_CIRCUMSCRIBED) },
        Mode { word: "opt-by-edge", clicks: None, outcome: Some(polygon_shape(6)) },
    ]],
    result: polygon_shape(6),
    node: "Sketch",
    undo: "sk-polygon",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/06-polygon",
    not_applicable: &[DRAWS[0], DRAWS[1], DRAWS[2], DRAWS[3], DRAWS[4], DRAWS[5]],
};

/// A SLOT: two centres and a width - two straight sides and two ends.
pub static SLOT: Tool = Tool {
    id: "sketch.slot",
    flow: Flow::Drawing(&[(0.0, 0.0), (30.0, 0.0), (30.0, 5.0)], Finish::LastClick),
    title: "tool-slot",
    entries: &[Entry::Button("tb-slot-hint"), Entry::Key(Modifiers::NONE, Key::O), Entry::Search("tool-slot")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 6,
        lines: 2,
        arcs: 2,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(15.0, 5.0, Under::Line), (0.0, 0.0, Under::Point), (15.0, 0.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-slot",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/07-slot",
    not_applicable: &[
        (2, "the slot has no mode: a centre line and a width are the whole of it"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// AN ELLIPSE: a centre, the end of the major axis, the end of the minor one.
pub static ELLIPSE: Tool = Tool {
    id: "sketch.ellipse",
    flow: Flow::Drawing(&[(0.0, 0.0), (20.0, 0.0), (0.0, 10.0)], Finish::LastClick),
    title: "sk-ellipse",
    entries: &[Entry::Button("tb-ellipse-hint"), Entry::Key(Modifiers::NONE, Key::E), Entry::Search("sk-ellipse")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 3,
        lines: 0,
        arcs: 0,
        circles: 0,
        ellipses: 1,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(14.14, 7.07, Under::Ellipse), (20.0, 0.0, Under::Point), (5.0, 2.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-ellipse",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/08-ellipse",
    not_applicable: &[
        (2, "the ellipse has no mode: a centre and two axes are the whole of it"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// A SPLINE: as many nodes as a person clicks, ended by a double click on the last one.
pub static SPLINE: Tool = Tool {
    id: "sketch.spline",
    flow: Flow::Drawing(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)], Finish::DoubleClick),
    title: "sk-spline",
    entries: &[Entry::Button("tb-spline-hint"), Entry::Key(Modifiers::NONE, Key::N), Entry::Search("sk-spline")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 3,
        lines: 0,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 1,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: None,
        size_of: None,
        under: &[(10.0, 10.0, Under::Point), (0.0, 0.0, Under::Point), (15.0, 8.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "sk-spline",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/09-spline",
    not_applicable: &[
        (2, "the spline has no mode: its nodes are its shape"),
        // decided 25.09: Esc FINISHES a spline on the points clicked so far, as a double click or Enter does
        (9, "Esc finishes the spline on its points rather than cancelling it - the owner's decision of 25.09"),
        DRAWS_WITHOUT_FIELDS[0],
        DRAWS_WITHOUT_FIELDS[1],
        DRAWS_WITHOUT_FIELDS[2],
        DRAWS_WITHOUT_FIELDS[3],
        DRAWS_WITHOUT_FIELDS[4],
        DRAWS_WITHOUT_FIELDS[5],
        DRAWS_WITHOUT_FIELDS[6],
        DRAWS_WITHOUT_FIELDS[7],
    ],
};

/// One text on the sheet: its contours are the geometry, and one string is one text, whatever it says.
const ONE_TEXT: Outcome =
    Outcome::Sketch { points: 0, lines: 0, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 1, notes: 0, constraints: None, dof: None, box_of: None, size_of: None, under: &[] };

/// A string of text placed on the sheet.
fn text_of(_string: &str) -> Outcome {
    ONE_TEXT
}

/// The same words placed as a note: words on the sheet, and no geometry at all.
const NOTE: Outcome = Outcome::Sketch { points: 0, lines: 0, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 1, constraints: None, dof: None, box_of: None, size_of: None, under: &[] };

/// A height of the letters: the text is the same one text, whatever it is.
fn text_of_height(_h: f64) -> Outcome {
    ONE_TEXT
}

/// TEXT: a string and a height above, then a click places it - as geometry, or as a note.
pub static TEXT: Tool = Tool {
    id: "sketch.text",
    flow: Flow::Drawing(&[(0.0, 0.0)], Finish::LastClick),
    title: "tool-text",
    entries: &[Entry::Button("tb-text-hint"), Entry::Key(Modifiers::NONE, Key::T), Entry::Search("tool-text")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::SketchOnXy,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[Words {
        caption: "tool-text",
        by_placeholder: false,
        typical: "CAD",
        // an ordinary string, another alphabet, and a long one
        valid: &["CAD", "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}", "a line of text long enough to be a caption"],
        // empty, spaces alone, and letters the program has no glyphs for - a box on the sheet is not a word
        invalid: &["", "   ", "\u{4e2d}\u{6587}"],
        outcome: text_of,
        enter: false,
    }],
    fields: &[Field {
        caption: "opt-height-short",
        by_placeholder: false,
        when: When::Before,
        class: Class::Length,
        typical: 10.0,
        lo: 0.1,
        hi: 1000.0,
        zero: false,
        negative: false,
        outcome: text_of_height,
    }],
    modes: &[&[Mode { word: "opt-note", clicks: None, outcome: Some(NOTE) }]],
    result: ONE_TEXT,
    node: "Sketch",
    undo: "sk-text",
    undo_steps: 1,
    // a label is placed once: the tool is put down, as in the CAD programs people know (found checking issue #32)
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/10-text",
    not_applicable: &[DRAWS[0], DRAWS[1], DRAWS[2], DRAWS[3], DRAWS[4], DRAWS[5]],
};

/// The rectangle of the fixture with its bottom side given a length: the width follows the number typed.
const fn rect_of_width(w: f64) -> Outcome {
    Outcome::Sketch {
        points: 5,
        lines: 4,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        // the four turns of its sides, its centre on the middle, the dimension
        constraints: Some(6),
        dof: Some(3),
        box_of: None,
        // a dimension sets the size; where the line stands is for the solver, which moves both of its free ends
        size_of: Some([w, 30.0]),
        under: &[],
    }
}

/// A LINEAR DIMENSION: a line clicked, the dimension placed with a second click, the number typed into it.
pub static DIM_LINEAR: Tool = Tool {
    id: "sketch.dim",
    flow: Flow::Drawing(&[(20.0, 0.0), (20.0, -10.0)], Finish::Placed),
    title: "tool-dim",
    entries: &[Entry::Button("tb-dim-hint"), Entry::Key(Modifiers::NONE, Key::D), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[Field {
        caption: "sk-expr-example",
        by_placeholder: true,
        when: When::After,
        class: Class::Length,
        typical: 40.0,
        lo: 1.0,
        hi: 1000.0,
        zero: false,
        negative: false,
        outcome: rect_of_width,
    }],
    modes: &[],
    result: rect_of_width(40.0),
    node: "Sketch",
    undo: "sk-dim",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/11-dimensions",
    not_applicable: &[
        (2, "the linear dimension has no mode: what it measures follows from what is clicked"),
        (3, "a dimension takes what it is put on by the clicks that place it, and those are its own points of the contract"),
        (4, "a click away from geometry is answered by the tool itself - see the wrong picks of the drawing tools"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The circle of the fixture given a diameter: the circle grows about its centre.
const fn circle_of_diameter(d: f64) -> Outcome {
    Outcome::Sketch {
        points: 1,
        lines: 0,
        arcs: 0,
        circles: 1,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        // the radius of a circle is not a degree of freedom of its own in this sketcher: the dimension gives the
        // circle its size, and the centre stays as free as it was
        constraints: None,
        dof: None,
        box_of: None,
        size_of: Some([d, d]),
        under: &[],
    }
}

/// A RADIUS OR DIAMETER: a circle clicked, and the number typed into the field that opens on it.
pub static DIM_RADIUS: Tool = Tool {
    id: "sketch.dim-radius",
    flow: Flow::Drawing(&[(10.0, 0.0)], Finish::LastClick),
    title: "tool-dim",
    entries: &[Entry::Button("tb-dim-radius-hint"), Entry::Search("cmdname-dim-radius")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::CircleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // 30 rather than the 20 the circle already has: typing what stands there changes nothing and proves nothing
    fields: &[Field {
        caption: "sk-expr-example",
        by_placeholder: true,
        when: When::After,
        class: Class::Radius,
        typical: 30.0,
        lo: 1.0,
        hi: 1000.0,
        zero: false,
        negative: false,
        outcome: circle_of_diameter,
    }],
    modes: &[],
    result: circle_of_diameter(30.0),
    node: "Sketch",
    undo: "sk-dim",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/11-dimensions",
    not_applicable: &[
        (2, "the radius has no mode: a click on a circle is the whole of it"),
        (7, "the radius is put down by one click: there is nothing drawn between clicks to follow the pointer"),
        (3, "a dimension takes the circle it is put on by the click that places it"),
        (4, "a click away from a circle is answered by the tool itself - see the wrong picks of the drawing tools"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The two lines of the fixture with the angle between them set: the lines stay, the angle is one more constraint.
const fn angle_of(_deg: f64) -> Outcome {
    Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] }
}

/// AN ANGULAR DIMENSION: two lines clicked, the dimension placed with a third click, the number typed into it.
pub static DIM_ANGLE: Tool = Tool {
    id: "sketch.dim-angle",
    flow: Flow::Drawing(&[(20.0, 0.0), (14.14, 14.14), (12.0, 5.0)], Finish::LastClick),
    title: "tool-dim",
    entries: &[Entry::Button("tb-dim-angle-hint"), Entry::Search("cmdname-dim-angle")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[Field { caption: "sk-expr-example", by_placeholder: true, when: When::After, class: Class::Angle, typical: 45.0, lo: 1.0, hi: 179.0, zero: false, negative: false, outcome: angle_of }],
    modes: &[],
    result: angle_of(45.0),
    node: "Sketch",
    undo: "sk-dim",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/11-dimensions",
    not_applicable: &[
        (2, "the angular dimension has no mode: two lines or three points are the whole of it"),
        (3, "a dimension takes the lines it is put on by the clicks that place it"),
        (4, "a click away from a line is answered by the tool itself - see the wrong picks of the drawing tools"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// What a click that cuts geometry leaves: a whole shape made of what was there, with nothing of its own to click
/// before it, no field and no mode.
const CUTS: &[(u8, &str)] = &[
    (2, "the tool has no mode: a click on a piece is the whole of it"),
    (3, "the piece is taken by the click that cuts it, not by a pick before it"),
    (4, "a click on empty sheet is answered by the tool itself, in words: see the drawing tools"),
    (5, "the tool has no field: where it cuts is given by the click"),
    (6, "the tool has no field to refuse a value in"),
    (7, "the cut is made by one click: there is nothing drawn between clicks to follow the pointer"),
    DRAWS[2],
    DRAWS[3],
    DRAWS[4],
    DRAWS[5],
];

/// TRIM: the lower half of the circle clicked away between the two places the line crosses it. What stays is the line
/// whole, from -20 to 20, and the upper half of the circle as an arc: its ends lie on the line, its centre where the
/// circle's was - two ends of the line, three points of the arc.
pub static TRIM: Tool = Tool {
    id: "sketch.trim",
    flow: Flow::Drawing(&[(0.0, -10.0)], Finish::LastClick),
    title: "tool-trim",
    entries: &[Entry::Button("tb-trim-hint"), Entry::Key(Modifiers::NONE, Key::K), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineThroughCircle,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 5,
        lines: 1,
        arcs: 1,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([-20.0, 0.0], [20.0, 10.0])),
        size_of: None,
        under: &[(0.0, 10.0, Under::Arc), (0.0, -10.0, Under::Nothing), (15.0, 0.0, Under::Line), (-15.0, 0.0, Under::Line)],
    },
    node: "Sketch",
    undo: "tool-trim",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/12-trim",
    not_applicable: CUTS,
};

/// BREAK: the line split where it was clicked, at x = 15. Two lines meet at the new point, and both stay what the
/// line was: horizontal, joined - the one point more slides along the line, one degree of freedom, not two.
pub static BREAK: Tool = Tool {
    id: "sketch.break",
    flow: Flow::Drawing(&[(15.0, 0.0)], Finish::LastClick),
    title: "tool-break",
    entries: &[Entry::Button("tb-break-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineThroughCircle,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 4,
        lines: 2,
        arcs: 0,
        circles: 1,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        // a horizontal for each half; the circle, drawn with no size typed, carries none and its radius is free
        constraints: Some(2),
        dof: Some(7),
        box_of: Some(([-20.0, -10.0], [20.0, 10.0])),
        size_of: None,
        under: &[(17.0, 0.0, Under::Line), (-17.0, 0.0, Under::Line)],
    },
    node: "Sketch",
    undo: "tool-break",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/14-break",
    not_applicable: CUTS,
};

/// EXTEND: the line that stops at x = 20 taken by a click on it, and a click past its end stretches it to the line
/// standing across its way at x = 30, as the preview shows. Nothing is added or taken away: the end moves, and the
/// stretch is a line where there was none.
pub static EXTEND: Tool = Tool {
    id: "sketch.extend",
    flow: Flow::Drawing(&[(18.0, 0.0), (25.0, 3.0)], Finish::LastClick),
    title: "tool-extend",
    entries: &[Entry::Button("tb-extend-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineShortOfLine,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 4,
        lines: 2,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, -10.0], [30.0, 10.0])),
        size_of: None,
        under: &[(25.0, 0.0, Under::Line), (10.0, 0.0, Under::Line)],
    },
    node: "Sketch",
    undo: "tool-extend",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/13-extend",
    not_applicable: CUTS,
};

/// The circle of radius 10 and its copy `d` away: outwards for a positive distance, inwards for a negative one. The
/// copy follows the circle it was made from, as an offset does in the professional systems: the two stand concentric
/// at their distance, so between them they keep the two degrees of freedom of the one centre.
fn offset_ring(d: f64) -> Outcome {
    let across = 20.0 + 2.0 * d.max(0.0);
    Outcome::Sketch {
        points: 2,
        lines: 0,
        arcs: 0,
        circles: 2,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        // the centre, and the radius of the circle drawn with no size typed: the copy keeps its distance from it
        dof: Some(3),
        box_of: None,
        size_of: Some([across, across]),
        under: &[],
    }
}

/// OFFSET: the circle copied 3 outwards - the distance stands in the bar, the circle is clicked, the pointer stands
/// outside it, Enter; a negative distance takes the other side, inwards.
pub static OFFSET: Tool = Tool {
    id: "sketch.offset",
    flow: Flow::Drawing(&[(10.0, 0.0)], Finish::EnterAt((20.0, 0.0))),
    title: "tool-offset",
    entries: &[Entry::Button("tb-offset-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::CircleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // an inward copy is a negative distance, down to the size of the circle itself; zero copies the circle onto
    // itself, which is no offset
    fields: &[Field {
        caption: "opt-distance",
        by_placeholder: false,
        when: When::Before,
        class: Class::Length,
        typical: 3.0,
        lo: -9.9,
        hi: 10000.0,
        zero: false,
        negative: true,
        outcome: offset_ring,
    }],
    modes: &[],
    result: Outcome::Sketch {
        points: 2,
        lines: 0,
        arcs: 0,
        circles: 2,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        // the centre, and the radius of the circle drawn with no size typed: the copy keeps its distance from it
        dof: Some(3),
        box_of: None,
        size_of: Some([26.0, 26.0]),
        under: &[],
    },
    node: "Sketch",
    undo: "tool-offset",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/20-offset",
    not_applicable: &[
        (2, "the tool has no mode: a click on a contour is the whole of it"),
        (3, "the contour is taken by the click that copies it, not by a pick before it"),
        (4, "a click on empty sheet is answered by the tool itself, in words: see the drawing tools"),
        (7, "the copy is made by one click: there is nothing drawn between clicks to follow the pointer"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The right-angled corner rounded with radius `r`: the arc touches each line r from the corner, its centre at (r, r),
/// and it passes the corner as close as r - r / sqrt(2) = 0.293 r on the diagonal. What stays: the two far ends, the two
/// points of touching, the centre of the arc and the corner itself, kept as the virtual sharp; the box of the whole is
/// untouched.
fn corner_rounded(r: f64) -> Outcome {
    let near = r - r / std::f64::consts::SQRT_2;
    // the line is looked for halfway along what is left of it (a radius of 29.9 leaves 0.1 of it); the gap and the arc at
    // the corner only for a radius the screen tells apart from the corner itself (0.01 lies within a click of it)
    let mut at = if 30.0 - r >= 2.0 { vec![((r + 30.0) / 2.0, 0.0, Under::Line)] } else { Vec::new() }; // a stub of the line under 2 lies within a click of its ends
    if r >= 1.0 {
        at.extend([(r / 2.0, 0.0, Under::Nothing), (near, near, Under::Arc)]);
    }
    let under: &'static [(f64, f64, Under)] = Box::leak(at.into_boxed_slice());
    Outcome::Sketch {
        points: 6,
        lines: 2,
        arcs: 1,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, 0.0], [30.0, 30.0])),
        size_of: None,
        under,
    }
}

/// The square corner rounded by a fillet of radius `r`, as `corner_rounded` gives it, for a size other than a radius:
/// a chord of 3 is a radius of 3 / sqrt 2 = 2.1213, an arc of 3 a radius of 6 / pi = 1.9099. The line halfway along
/// what is left of it and the arc r - r / sqrt 2 out on the diagonal; the gap at half the radius along the line is not
/// looked for: the arc of a radius of 2.1 passes 0.25 from it, within a click.
const ROUNDED_BY_CHORD_3: Outcome = Outcome::Sketch {
    points: 6,
    lines: 2,
    arcs: 1,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: Some(([0.0, 0.0], [30.0, 30.0])),
    size_of: None,
    under: &[(16.0607, 0.0, Under::Line), (0.6213, 0.6213, Under::Arc)],
};

/// See `ROUNDED_BY_CHORD_3`: an arc of 3, a radius of 1.9099.
const ROUNDED_BY_ARC_3: Outcome = Outcome::Sketch {
    points: 6,
    lines: 2,
    arcs: 1,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: Some(([0.0, 0.0], [30.0, 30.0])),
    size_of: None,
    under: &[(15.9549, 0.0, Under::Line), (0.5594, 0.5594, Under::Arc)],
};

/// The corner rounded with a radius of 3, the result of the tool as the bar opens.
const ROUNDED_BY_RADIUS_3: Outcome = Outcome::Sketch {
    points: 6,
    lines: 2,
    arcs: 1,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    size_of: None,
    box_of: Some(([0.0, 0.0], [30.0, 30.0])),
    under: &[(1.5, 0.0, Under::Nothing), (20.0, 0.0, Under::Line), (0.8787, 0.8787, Under::Arc)],
};

/// CORNER: the corner of the two lines rounded with the radius the bar holds, a click on the corner itself.
pub static CORNER: Tool = Tool {
    id: "sketch.corner",
    flow: Flow::Drawing(&[(0.0, 0.0)], Finish::LastClick),
    title: "tool-fillet",
    // the search finds this article by its title, and its command takes this tool
    entries: &[Entry::Button("tb-fillet-sketch-hint"), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::CornerInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // the click on the corner opens a field at it, as a dimension does, and the radius is typed there. An arc that
    // touches the lines further from the corner than they are long (r > 30) cannot be drawn: 29.9 is the largest
    // radius that fits, and a larger one must be refused
    fields: &[Field {
        caption: "g-expr-placeholder",
        by_placeholder: true,
        when: When::After,
        class: Class::Radius,
        typical: 3.0,
        lo: 0.01,
        hi: 29.9,
        zero: false,
        negative: false,
        outcome: corner_rounded,
    }],
    modes: &[&[
        Mode { word: "opt-radius", clicks: None, outcome: Some(ROUNDED_BY_RADIUS_3) },
        Mode { word: "opt-fillet-chord", clicks: None, outcome: Some(ROUNDED_BY_CHORD_3) },
        Mode { word: "opt-fillet-arc-length", clicks: None, outcome: Some(ROUNDED_BY_ARC_3) },
    ]],
    result: ROUNDED_BY_RADIUS_3,
    node: "Sketch",
    undo: "tool-fillet",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/16-corner",
    not_applicable: &[
        (3, "the corner is taken by the click that rounds it, not by a pick before it"),
        (4, "a click away from a corner is answered by the tool itself, in words: see the drawing tools"),
        (7, "the corner is rounded by one click: there is nothing drawn between clicks to follow the pointer"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The right-angled corner cut with a chamfer of `d`: a line from (d, 0) to (0, d) in place of the corner. What stays:
/// the two far ends, the two ends of the cut, and the sharp corner itself, unseen, which the dimensions of the chamfer
/// are measured from; the box of the whole is untouched.
fn corner_cut(cut: f64) -> Outcome {
    // the field of the symmetric chamfer, the mode the tool is taken in, is the length of the cut: on a square corner each
    // leg is cut / sqrt(2)
    let d = cut / std::f64::consts::SQRT_2;
    // as for the rounding: the line halfway along what is left of it, the corner only for a leg the screen tells apart
    let mut at = if 30.0 - d >= 2.0 { vec![((d + 30.0) / 2.0, 0.0, Under::Line)] } else { Vec::new() }; // a stub of the line under 2 lies within a click of its ends
    if d >= 1.0 {
        at.extend([(d / 4.0, 0.0, Under::Nothing), (d / 2.0, d / 2.0, Under::Line)]);
    }
    let under: &'static [(f64, f64, Under)] = Box::leak(at.into_boxed_slice());
    Outcome::Sketch {
        points: 5,
        lines: 3,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, 0.0], [30.0, 30.0])),
        size_of: None,
        under,
    }
}

/// The corner cut with a symmetric chamfer of 3: the cut is 3 long, each leg 3 / sqrt(2) = 2.1213 on a square corner.
const CORNER_SYMMETRIC_3: Outcome = Outcome::Sketch {
    points: 5,
    lines: 3,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: Some(([0.0, 0.0], [30.0, 30.0])),
    size_of: None,
    under: &[(0.53, 0.0, Under::Nothing), (20.0, 0.0, Under::Line), (1.06, 1.06, Under::Line)],
};

/// The corner cut with a chamfer of 3 by two legs or by a leg and an angle, its second value left as the mode sets it:
/// the second leg as the first, the angle 45 deg - legs of 3 either way.
const CORNER_CUT_3: Outcome = Outcome::Sketch {
    points: 5,
    lines: 3,
    arcs: 0,
    circles: 0,
    ellipses: 0,
    splines: 0,
    texts: 0,
    notes: 0,
    constraints: None,
    dof: None,
    box_of: Some(([0.0, 0.0], [30.0, 30.0])),
    size_of: None,
    under: &[(0.75, 0.0, Under::Nothing), (20.0, 0.0, Under::Line), (1.5, 1.5, Under::Line)],
};

/// CORNER_CHAMFER: the same corner cut with a chamfer of 3, the size typed in the field at the corner; a symmetric cut of
/// that length, two legs, or a leg and an angle.
pub static CORNER_CHAMFER: Tool = Tool {
    id: "sketch.corner-chamfer",
    flow: Flow::Drawing(&[(0.0, 0.0)], Finish::LastClick),
    title: "tool-chamfer",
    // searched by its own name: the article is the corner fillet's too, and its title finds the fillet first
    entries: &[Entry::Button("tb-chamfer-sketch-hint"), Entry::Search("cmdname-corner-chamfer")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::CornerInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // a cut whose legs are as long as the lines it cuts cannot be drawn: 29.9 is short of it in every mode. The field is the first on the bar showing the grey
    // words: its caption changes with the mode (size, first leg, length), and with two values the bar holds a second
    // field of the same grey words after it
    fields: &[Field {
        caption: "g-expr-placeholder",
        by_placeholder: true,
        when: When::After,
        class: Class::Length,
        typical: 3.0,
        lo: 0.01,
        hi: 29.9,
        zero: false,
        negative: false,
        outcome: corner_cut,
    }],
    modes: &[&[
        Mode { word: "cmd-symmetric", clicks: None, outcome: Some(CORNER_SYMMETRIC_3) },
        Mode { word: "cmd-two-distances", clicks: None, outcome: Some(CORNER_CUT_3) },
        Mode { word: "cmd-leg-angle", clicks: None, outcome: Some(CORNER_CUT_3) },
    ]],
    result: CORNER_SYMMETRIC_3,
    node: "Sketch",
    undo: "tool-chamfer",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/16-corner",
    not_applicable: &[
        (3, "the corner is taken by the click that cuts it, not by a pick before it"),
        (4, "a click away from a corner is answered by the tool itself, in words: see the drawing tools"),
        (7, "the corner is cut by one click: there is nothing drawn between clicks to follow the pointer"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The edge of the block along X brought into the sketch on its top face: one line from (0, 0) to (40, 0), driven by
/// the part - it follows the body and cannot be dragged, so it leaves no degree of freedom.
const PROJECTED_EDGE: Outcome = Outcome::Sketch {
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
const PROJECTED_FACE: Outcome = Outcome::Sketch {
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
const PROJECTS: &[(u8, &str)] = &[
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

/// DELETE: the bottom side of the rectangle, picked, taken away. Three sides stay, and the four corners with them:
/// the two the bottom side ended at still end the sides beside it.
pub static DELETE: Tool = Tool {
    id: "sketch.delete",
    flow: Flow::Action,
    title: "sk-delete",
    entries: &[Entry::Button("tb-delete-hint"), Entry::Key(Modifiers::NONE, Key::Delete), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
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
        under: &[(20.0, 0.0, Under::Nothing), (40.0, 15.0, Under::Line), (20.0, 30.0, Under::Line), (0.0, 15.0, Under::Line)],
    },
    node: "Sketch",
    undo: "sk-delete",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/18-delete",
    not_applicable: ACTS,
};

/// What an action leaves out of the contract: it is not held, it takes nothing of its own, has no field and no bar, and
/// makes nothing between a click and its end.
const ACTS: &[(u8, &str)] = &[
    (2, "an action has no bar of options: its way in does the work"),
    (3, "an action works on what is selected before it; it takes nothing itself"),
    (4, "an action takes nothing, so there is no wrong pick to refuse"),
    (5, "the action has no field"),
    (6, "the action has no field to refuse a value in"),
    (7, "an action is done at once: there is nothing to show before it"),
    (9, "an action is done at once: there is nothing to cancel, only to undo, which is point 11"),
    DRAWS[2],
    DRAWS[3],
    DRAWS[4],
    DRAWS[5],
    (19, "an action is not held, so F1 has no tool in hand to open the article of"),
];

/// MIRROR: the picked line from (10, 0) to (30, 20) reflected about the upright line through the origin - a copy from
/// (-10, 0) to (-30, 20), the drawing now standing from -30 to 30 across.
pub static MIRROR: Tool = Tool {
    id: "sketch.mirror",
    flow: Flow::Drawing(&[(0.0, 10.0)], Finish::LastClick),
    title: "tool-mirror",
    entries: &[Entry::Button("tb-mirror-sketch-hint"), Entry::Key(Modifiers::NONE, Key::M), Entry::SearchByArticle],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 6,
        lines: 3,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([-30.0, -20.0], [30.0, 20.0])),
        size_of: None,
        under: &[(-20.0, 10.0, Under::Line), (20.0, 10.0, Under::Line)],
    },
    node: "Sketch",
    undo: "tool-mirror",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/19-mirror",
    not_applicable: &[
        (2, "the mirror has no mode: what is reflected is picked before it, the axis is clicked"),
        (3, "the axis is taken by the click that reflects about it, not by a pick before it"),
        (4, "a click away from a line is answered by the tool itself, in words: see the drawing tools"),
        (5, "the mirror has no field"),
        (6, "the mirror has no field to refuse a value in"),
        (7, "the reflection is made by one click: there is nothing drawn between clicks to follow the pointer"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// What a constraint leaves out of the contract: it is an action on the geometry picked before it (a person picks,
/// Shift-picks the second, presses the button), it has no field and no bar, and nothing stands over it but its sketch.
const CONSTRAINS: &[(u8, &str)] = &[
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
    entries: &[Entry::Button("con-horizontal-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-vertical-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-parallel-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-perpendicular-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-equal")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[Pick::Sketch(20.0, 0.0), Pick::Sketch(0.0, 15.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // four corners and the centre of the rectangle
    result: Outcome::Sketch { points: 5, lines: 4, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: None, dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-collinear-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(10.6, 10.6)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-concentric-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoCirclesInSketch,
    picks: &[Pick::Sketch(0.0, 5.0), Pick::Sketch(30.0, 5.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // the concentric alone: the circles drawn with no size typed carry none, and the centre and both radii are free
    result: Outcome::Sketch { points: 2, lines: 0, arcs: 0, circles: 2, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(1), dof: Some(4), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-tangent-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineThroughCircle,
    picks: &[Pick::Sketch(15.0, 0.0), Pick::Sketch(0.0, 10.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    // the horizontal of the line and the tangent; the circle drawn with no size typed carries none
    result: Outcome::Sketch { points: 3, lines: 1, arcs: 0, circles: 1, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(2), dof: Some(5), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-midpoint-hint")],
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
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(2), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-coincident-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(30.0, 0.0), Pick::Sketch(21.21, 21.21)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(3), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("con-fix")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::TwoLinesInSketch,
    picks: &[Pick::Sketch(30.0, 0.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(3), dof: Some(2), box_of: None, size_of: None, under: &[] },
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

/// MOVE: the picked line from (10, 0) to (30, 20) moved by a base point clicked on its end and a target 30 below it.
pub static MOVE: Tool = Tool {
    id: "sketch.move",
    flow: Flow::Drawing(&[(10.0, 0.0), (10.0, -30.0)], Finish::LastClick),
    title: "tool-move",
    entries: &[Entry::Button("tb-move-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 4,
        lines: 2,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, -30.0], [30.0, 20.0])),
        size_of: None,
        under: &[(20.0, -20.0, Under::Line), (20.0, 10.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "tool-move",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: &[
        (2, "the tool has no mode: what is moved is picked before it, the base and the target are clicked"),
        (3, "the base and the target are clicks that put points, not picks of what is there"),
        (4, "a click on empty sheet is a place like any other: there is no wrong click to refuse"),
        (5, "the tool has no field: how far is given by the two clicks"),
        (6, "the tool has no field to refuse a value in"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// COPY: the picked line from (10, 0) to (30, 20) copied by a base point clicked on its end and a target 30 below it.
pub static COPY: Tool = Tool {
    id: "sketch.copy",
    flow: Flow::Drawing(&[(10.0, 0.0), (10.0, -30.0)], Finish::LastClick),
    title: "tool-copy",
    entries: &[Entry::Button("tb-copy-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 6,
        lines: 3,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, -30.0], [30.0, 20.0])),
        size_of: None,
        under: &[(20.0, -20.0, Under::Line), (20.0, 10.0, Under::Line)],
    },
    node: "Sketch",
    undo: "tool-copy",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: &[
        (2, "the tool has no mode: what is copied is picked before it, the base and the target are clicked"),
        (3, "the base and the target are clicks that put points, not picks of what is there"),
        (4, "a click on empty sheet is a place like any other: there is no wrong click to refuse"),
        (5, "the tool has no field: how far is given by the two clicks"),
        (6, "the tool has no field to refuse a value in"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The picked line from (10, 0) to (30, 20) turned by `deg` about the origin, the upright line from (0, -20) to (0, 20)
/// beside it: the box of the two, and the middle of the turned line where the line lies.
fn turned_line(deg: f64) -> Outcome {
    let (c, sn) = (deg.to_radians().cos(), deg.to_radians().sin());
    let turn = |x: f64, y: f64| (x * c - y * sn, x * sn + y * c);
    let (a, b, m) = (turn(10.0, 0.0), turn(30.0, 20.0), turn(20.0, 10.0));
    let xs = [a.0, b.0, 0.0];
    let ys = [a.1, b.1, -20.0, 20.0];
    let lo = [xs.iter().copied().fold(f64::MAX, f64::min), ys.iter().copied().fold(f64::MAX, f64::min)];
    let hi = [xs.iter().copied().fold(f64::MIN, f64::max), ys.iter().copied().fold(f64::MIN, f64::max)];
    let under: &'static [(f64, f64, Under)] = Box::leak(vec![(m.0, m.1, Under::Line)].into_boxed_slice());
    Outcome::Sketch { points: 4, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: None, dof: None, box_of: Some((lo, hi)), size_of: None, under }
}

/// ROTATE: the picked line turned 90 degrees about the origin, the angle typed in the field that opens at the centre.
pub static ROTATE: Tool = Tool {
    id: "sketch.rotate",
    flow: Flow::Drawing(&[(0.0, 0.0)], Finish::LastClick),
    title: "tool-rotate",
    entries: &[Entry::Button("tb-rotate-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // a turn of 0 turns nothing and must not lay a step; a whole turn either way is the most there is
    fields: &[Field {
        caption: "sk-angle-placeholder",
        by_placeholder: true,
        when: When::After,
        class: Class::Angle,
        typical: 90.0,
        lo: -360.0,
        hi: 360.0,
        zero: false,
        negative: true,
        outcome: turned_line,
    }],
    modes: &[],
    result: Outcome::Sketch {
        points: 4,
        lines: 2,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([-20.0, -20.0], [0.0, 30.0])),
        size_of: None,
        under: &[(-10.0, 20.0, Under::Line), (20.0, 10.0, Under::Nothing)],
    },
    node: "Sketch",
    undo: "tool-rotate",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/21-move-copy-rotate",
    not_applicable: &[
        (2, "the tool has no mode: what is turned is picked before it, the centre is clicked"),
        (7, "the turn is shown as its angle is typed, after the click on the centre; the contract reads the preview of a drawing by the pointer, which is not where a turn moves"),
        (3, "the centre is a click that puts a point, not a pick of what is there"),
        (4, "a click on empty sheet is a centre like any other: there is no wrong click to refuse"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// What a pattern of the sketch leaves out: it works on the selection made before it, and Enter lays it.
const PATTERNS: &[(u8, &str)] = &[
    (2, "the bar of the pattern holds its count and spacing, not modes that put each other down"),
    (4, "the pattern takes the selection made before it: there is no pick of its own to refuse"),
    (5, "the count and spacing are numbers of the bar, checked by the pattern matrix"),
    (6, "the count and spacing are numbers of the bar, checked by the pattern matrix"),
    DRAWS[2],
    DRAWS[3],
    DRAWS[4],
    DRAWS[5],
];

/// LINEAR_PATTERN: the picked line repeated three times, 20 apart along X - the bar's count and spacing as they
/// open - so the drawing reaches x = 70.
pub static LINEAR_PATTERN: Tool = Tool {
    id: "sketch.linear-pattern",
    flow: Flow::Command,
    title: "tool-lin-array",
    entries: &[Entry::Button("tb-lin-array-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 8,
        lines: 4,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, -20.0], [70.0, 20.0])),
        size_of: None,
        under: &[(40.0, 10.0, Under::Line), (60.0, 10.0, Under::Line)],
    },
    node: "", // the pattern adds to the sketch open, it lays no node of the timeline
    undo: "g-sketch-array",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/22-array",
    not_applicable: &[
        (3, "the pattern takes the selection made before it, and nothing else is clicked"),
        PATTERNS[0],
        PATTERNS[1],
        PATTERNS[2],
        PATTERNS[3],
        PATTERNS[4],
        PATTERNS[5],
        PATTERNS[6],
        PATTERNS[7],
    ],
};

/// CIRCULAR_PATTERN: the picked line repeated three times about the origin, clicked as the centre - at 0, 120
/// and 240 degrees. Turned 120 degrees the far end (30, 20) comes to (-32.32, 15.98); turned 240, to (2.32, -35.98).
pub static CIRCULAR_PATTERN: Tool = Tool {
    id: "sketch.circular-pattern",
    flow: Flow::Command,
    title: "tool-circ-array",
    entries: &[Entry::Button("tb-circ-array-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[Pick::Sketch(0.0, 0.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch {
        points: 8,
        lines: 4,
        arcs: 0,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([-32.320508, -35.980762], [30.0, 20.0])),
        size_of: None,
        under: &[],
    },
    node: "", // the pattern adds to the sketch open, it lays no node of the timeline
    undo: "g-sketch-array",
    undo_steps: 1,
    stays: false,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/22-array",
    not_applicable: &[
        (3, "the centre is a click that puts a point, not a pick of what is there"),
        PATTERNS[0],
        PATTERNS[1],
        PATTERNS[2],
        PATTERNS[3],
        PATTERNS[4],
        PATTERNS[5],
        PATTERNS[6],
        PATTERNS[7],
    ],
};

/// What the items of the Edit menu leave out in a sketch: what they work on is picked before them.
const EDITS: &[(u8, &str)] = &[
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

/// CON_SYMMETRY: the two ends of the slanted line made symmetric about the upright line through the origin - two
/// degrees of freedom taken: the ends are mirror images of each other across the axis.
pub static CON_SYMMETRY: Tool = Tool {
    id: "sketch.con-symmetry",
    flow: Flow::Action,
    title: "sk-constraint",
    entries: &[Entry::Button("con-symmetric-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::LineAndAxisPicked,
    picks: &[Pick::Sketch(10.0, 0.0), Pick::Sketch(30.0, 20.0), Pick::Sketch(0.0, 10.0)],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Sketch { points: 4, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(2), dof: Some(5), box_of: None, size_of: None, under: &[] },
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
    entries: &[Entry::Button("tb-construction-hint"), Entry::Key(Modifiers::NONE, Key::X)],
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

/// MEASURE: two clicks on the ends of the bottom side of the rectangle - 40 apart, and the status line says so; the
/// sketch is untouched.
pub static MEASURE: Tool = Tool {
    id: "sketch.measure",
    flow: Flow::Drawing(&[(0.0, 0.0), (40.0, 0.0)], Finish::LastClick),
    title: "tool-measure",
    entries: &[Entry::Button("tb-measure-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Says { text: "40" },
    node: "Sketch",
    undo: "tool-measure",
    undo_steps: 0,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "general/11-measure",
    not_applicable: &[
        (2, "the measure has no mode: two clicks are the whole of it"),
        (3, "the two clicks put the points measured between; they take nothing"),
        (4, "a click on empty sheet is a point like any other: there is no wrong click to refuse"),
        (5, "the measure takes no value"),
        (6, "the measure takes no value to refuse"),
        (11, "a measure changes nothing: it lays no step of undo"),
        (12, "a measure changes nothing: saving and opening carries nothing of it"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// The rectangle 40 x 30 with every corner rounded with radius `r`: four sides cut back by r at each end, four arcs of
/// quarter turns - eight points where arcs touch sides and four centres, and the rectangle's own five: its four
/// corners left as virtual sharps, drawn and picked like any point (a click on the far corner (40, 30) takes it), and
/// its centre; the box of the whole is untouched.
fn all_corners_rounded(r: f64) -> Outcome {
    let near = r - r / std::f64::consts::SQRT_2;
    // what lies under the pointer is asked only of an arc the pointer can tell from its ends: at 0.01 the whole arc is
    // under one pixel with its two ends, and the pick rightly finds a point there
    let under: &'static [(f64, f64, Under)] = if r >= 1.0 {
        Box::leak(vec![(r / 2.0, 0.0, Under::Nothing), (20.0, 0.0, Under::Line), (near, near, Under::Arc), (40.0 - near, 30.0 - near, Under::Arc), (40.0, 30.0, Under::Point)].into_boxed_slice())
    } else {
        &[(20.0, 0.0, Under::Line)]
    };
    Outcome::Sketch {
        points: 17,
        lines: 4,
        arcs: 4,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, 0.0], [40.0, 30.0])),
        size_of: None,
        under,
    }
}

/// FILLET_ALL: a click on the rectangle takes the whole contour, and the radius typed in the field that opens rounds all
/// four corners at once.
pub static FILLET_ALL: Tool = Tool {
    id: "sketch.fillet-all",
    flow: Flow::Drawing(&[(20.0, 0.0)], Finish::LastClick),
    title: "tool-fillet",
    entries: &[Entry::Button("tb-fillet-all-hint")],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    // the field at the shape, by the words beside it ("R of all corners"); the radius cannot exceed half the short side
    // (15), where two arcs of a side would meet
    fields: &[Field {
        caption: "sk-r-all-corners",
        by_placeholder: false,
        when: When::After,
        class: Class::Radius,
        typical: 5.0,
        lo: 0.01,
        hi: 14.9,
        zero: false,
        negative: false,
        outcome: all_corners_rounded,
    }],
    modes: &[],
    // see `all_corners_rounded`: twelve points of the arcs, the four virtual sharps - drawn and picked - and the centre
    result: Outcome::Sketch {
        points: 17,
        lines: 4,
        arcs: 4,
        circles: 0,
        ellipses: 0,
        splines: 0,
        texts: 0,
        notes: 0,
        constraints: None,
        dof: None,
        box_of: Some(([0.0, 0.0], [40.0, 30.0])),
        size_of: None,
        under: &[(2.5, 0.0, Under::Nothing), (20.0, 0.0, Under::Line), (40.0, 30.0, Under::Point)],
    },
    node: "Sketch",
    undo: "tool-fillet",
    undo_steps: 1,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    help: "sketch/16-corner",
    not_applicable: &[
        (2, "the tool has no mode: a click on a contour is the whole of it"),
        (3, "the contour is taken by the click that rounds it, not by a pick before it"),
        (4, "a click away from a contour is answered by the tool itself, in words: see the drawing tools"),
        (7, "the corners are rounded after the radius is typed: there is nothing drawn between clicks to follow the pointer"),
        DRAWS[2],
        DRAWS[3],
        DRAWS[4],
        DRAWS[5],
    ],
};

/// SELECT: the arrow taken back from the line tool, and the bottom side of the rectangle clicked - that one side is
/// selected, nothing drawn.
pub static SELECT: Tool = Tool {
    id: "sketch.select",
    flow: Flow::Drawing(&[(20.0, 0.0)], Finish::LastClick),
    title: "tool-select",
    entries: &[Entry::Button("tb-select-hint"), Entry::Key(Modifiers::NONE, Key::S)],
    other: (Entry::Button("tb-line-hint"), "tool-line"),
    fixture: Fixture::RectangleInSketch,
    picks: &[],
    pick_trial: &[],
    wrong_picks: &[],
    words: &[],
    fields: &[],
    modes: &[],
    result: Outcome::Picked { count: 1 },
    node: "",
    undo: "tb-select-hint",
    undo_steps: 0,
    stays: true,
    upstream: None,
    dependency: None,
    contexts: &[Context::SecondPart],
    refusal: None,
    budget: (10, 2000),
    // the help writes of no selecting in a sketch: there is no article to name
    help: "sketch/24-select",
    not_applicable: &[
        (2, "the arrow has no bar of options: it is the hand with no tool in it"),
        (4, "a click on empty space with the arrow clears the selection, as in the professional systems: it is not a wrong pick"),
        (5, "the arrow has no field"),
        (6, "the arrow has no field to refuse a value in"),
        (7, "a click selects at once: there is nothing to show before it"),
        (11, "a selection is not the document: it lays no step of undo"),
        (12, "a selection is not the document: saving and opening does not carry it"),
        (13, DRAWS[2].1),
        (14, DRAWS[3].1),
        (15, DRAWS[4].1),
        (17, DRAWS[5].1),
    ],
};
