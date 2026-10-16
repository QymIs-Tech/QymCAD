//! The tools that cut and lengthen: trim, break, extend, offset.

use super::*;

/// What a click that cuts geometry leaves: a whole shape made of what was there, with nothing of its own to click
/// before it, no field and no mode.
pub(crate) const CUTS: &[(u8, &str)] = &[
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
        constraints: Some(4),
        dof: Some(4),
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
/// at their distance. The circle is drawn about the origin and held on it, so what is left free is its radius.
pub(crate) fn offset_ring(d: f64) -> Outcome {
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
        // the radius of the circle drawn with no size typed: the copy keeps its distance from it
        dof: Some(1),
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
        dof: Some(1),
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
