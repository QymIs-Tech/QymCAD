//! The tools that size: the linear, radial and angular dimensions.

use super::*;

/// The rectangle of the fixture with its bottom side given a length: the width follows the number typed.
pub(crate) const fn rect_of_width(w: f64) -> Outcome {
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
        constraints: Some(7),
        dof: Some(1),
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
pub(crate) const fn circle_of_diameter(d: f64) -> Outcome {
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
pub(crate) const fn angle_of(_deg: f64) -> Outcome {
    Outcome::Sketch { points: 3, lines: 2, arcs: 0, circles: 0, ellipses: 0, splines: 0, texts: 0, notes: 0, constraints: Some(4), dof: Some(1), box_of: None, size_of: None, under: &[] }
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
