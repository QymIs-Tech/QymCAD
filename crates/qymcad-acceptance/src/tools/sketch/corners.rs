//! The tools of corners: rounded, cut, chamfered, every corner rounded.

use super::*;

/// The right-angled corner rounded with radius `r`: the arc touches each line r from the corner, its centre at (r, r),
/// and it passes the corner as close as r - r / sqrt(2) = 0.293 r on the diagonal. What stays: the two far ends, the two
/// points of touching, the centre of the arc and the corner itself, kept as the virtual sharp; the box of the whole is
/// untouched.
pub(crate) fn corner_rounded(r: f64) -> Outcome {
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
pub(crate) const ROUNDED_BY_CHORD_3: Outcome = Outcome::Sketch {
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
pub(crate) const ROUNDED_BY_ARC_3: Outcome = Outcome::Sketch {
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
pub(crate) const ROUNDED_BY_RADIUS_3: Outcome = Outcome::Sketch {
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
pub(crate) fn corner_cut(cut: f64) -> Outcome {
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
pub(crate) const CORNER_SYMMETRIC_3: Outcome = Outcome::Sketch {
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
pub(crate) const CORNER_CUT_3: Outcome = Outcome::Sketch {
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

/// The rectangle 40 x 30 with every corner rounded with radius `r`: four sides cut back by r at each end, four arcs of
/// quarter turns - eight points where arcs touch sides and four centres, and the rectangle's own five: its four
/// corners left as virtual sharps, drawn and picked like any point (a click on the far corner (40, 30) takes it), and
/// its centre; the box of the whole is untouched.
pub(crate) fn all_corners_rounded(r: f64) -> Outcome {
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
    entries: &[Entry::Button("tb-fillet-all-hint"), Entry::Search("cmdname-fillet-all")],
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
