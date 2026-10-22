//! THE CONSTRAINTS on every pair of kinds they make sense for: coincidence and a point on every curve, tangency between
//! every pair of a straight and a round or a free curve, equality and concentricity of round curves, symmetry and
//! fixing of any point, level and upright lines, axes and pairs of points, parallel, square and collinear straights, the
//! middle of a line and of an arc. Each laid by the two ways a person has - the geometry picked before the button, or the
//! button and then the clicks - and each must hold when a point of the geometry is dragged.
use super::*;

/// WHAT IS CLICKED for a constraint: a curve of the first or the second shape drawn, or a point standing at a place.
#[derive(Clone, Copy, Debug)]
enum Pick {
    /// the first curve of the first shape
    First,
    /// the first curve of the second shape
    Second,
    /// the point standing at this place when the shapes are drawn
    Point(Point2),
    /// the curve under this place - the line two points are mirrored about
    Curve(Point2),
}

/// HOW A CONSTRAINT IS LAID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    /// the geometry picked, then the button
    PickedBefore,
    /// the button, then the clicks
    ButtonFirst,
}

/// WHAT A CONSTRAINT HOLDS, read from the curves and points after it.
#[derive(Clone, Copy, Debug)]
enum Holds {
    /// the point lies on the curve - on the whole line through a straight one
    OnCurve,
    /// the two points stand on one spot
    Together,
    /// the two curves touch and run the same way where they touch
    Touch,
    /// the same length of two straights, the same radius of two round curves, the same semi-axes of two ellipses
    Same,
    /// the two round curves share their centre
    OneCentre,
    /// the two points mirror each other about the line, the third curve picked
    Mirror,
    /// the point stays where it is when dragged
    Fixed,
    /// the line, the major axis of the ellipse or the two points stand level
    Level,
    /// they stand upright
    Upright,
    /// the two straights run the same way
    Parallel,
    /// the two straights stand square
    Square,
    /// the second straight lies on the line through the first
    OneLine,
    /// the point stands in the middle of the curve
    Middle,
    /// the constraint makes no sense on what is picked - a level spline, two parallel circles: none is laid, the
    /// drawing stays, and the status line refuses in words
    Refused,
}

/// A SHAPE DRAWN BY HAND for a constraint case.
type Draw = fn(&mut Hand);

/// A CASE OF THE MATRIX: the constraint by its code (as the panel numbers its buttons), the two shapes, what is clicked,
/// what must hold, and the point dragged after it to see that it holds.
struct Case {
    name: &'static str,
    code: qymcad_ui_state::ConstraintKind,
    first: Draw,
    second: Draw,
    picks: &'static [Pick],
    holds: Holds,
    drag: Point2,
}

fn put_down(h: &mut Hand) {
    h.key(egui::Key::Escape).key(egui::Key::Escape);
    h.sk_select();
}

fn line_at(h: &mut Hand, seg: [Point2; 2]) {
    h.sk_tool(qymcad_ui_state::SketchShape::Line);
    h.click2d(seg[0].x, seg[0].y).click2d(seg[1].x, seg[1].y);
    put_down(h);
}

fn circle_at(h: &mut Hand, c: Point2, r: f64) {
    h.sk_tool(qymcad_ui_state::SketchShape::Circle);
    h.click2d(c.x, c.y).click2d(c.x + r, c.y);
    put_down(h);
}

/// An arc about `c` of radius 6 from its right through a quarter turn up.
fn arc_at(h: &mut Hand, c: Point2) {
    h.sk_tool(qymcad_ui_state::SketchShape::Arc);
    h.click2d(c.x, c.y).click2d(c.x + 6.0, c.y).click2d(c.x - 4.0, c.y + 4.5);
    put_down(h);
}

/// An ellipse about `c`, 8 along X and 4 up.
fn ellipse_at(h: &mut Hand, c: Point2) {
    h.sk_tool(qymcad_ui_state::SketchShape::Ellipse);
    h.click2d(c.x, c.y).click2d(c.x + 8.0, c.y).click2d(c.x, c.y + 4.0);
    put_down(h);
}

/// A spline through `nodes`, closed when its last node is its first.
fn spline_at(h: &mut Hand, nodes: &[Point2]) {
    h.sk_tool(qymcad_ui_state::SketchShape::Spline);
    let closed = nodes.len() > 3 && nodes[0].dist(nodes[nodes.len() - 1]) < 1e-9;
    for (k, p) in nodes.iter().enumerate() {
        if k + 1 == nodes.len() && !closed {
            h.double_click2d(p.x, p.y);
        } else {
            h.click2d(p.x, p.y);
        }
    }
    put_down(h);
}

const fn p(at: [f64; 2]) -> Point2 {
    Point2 { x: at[0], y: at[1] }
}

/// The shapes of the cases. The first stands left, about (30, 30); the second right, about (52, 30).
mod shapes {
    use super::*;
    pub fn line_left(h: &mut Hand) {
        line_at(h, [p([22.0, 26.0]), p([34.0, 32.0])]);
    }
    pub fn line_right(h: &mut Hand) {
        line_at(h, [p([46.0, 22.0]), p([58.0, 40.0])]);
    }
    pub fn upright(h: &mut Hand) {
        line_at(h, [p([41.0, 14.0]), p([41.0, 46.0])]);
    }
    pub fn circle_left(h: &mut Hand) {
        circle_at(h, p([30.0, 30.0]), 5.0);
    }
    pub fn circle_right(h: &mut Hand) {
        circle_at(h, p([52.0, 31.0]), 4.0);
    }
    pub fn arc_left(h: &mut Hand) {
        arc_at(h, p([30.0, 30.0]));
    }
    pub fn arc_right(h: &mut Hand) {
        arc_at(h, p([52.0, 29.0]));
    }
    pub fn ellipse_left(h: &mut Hand) {
        ellipse_at(h, p([28.0, 30.0]));
    }
    pub fn ellipse_right(h: &mut Hand) {
        ellipse_at(h, p([54.0, 32.0]));
    }
    pub fn spline_left(h: &mut Hand) {
        spline_at(h, &[p([20.0, 30.0]), p([25.0, 35.0]), p([31.0, 27.0]), p([36.0, 32.0])]);
    }
    pub fn spline_right(h: &mut Hand) {
        spline_at(h, &[p([46.0, 28.0]), p([51.0, 34.0]), p([56.0, 26.0]), p([61.0, 31.0])]);
    }
    pub fn closed_spline_right(h: &mut Hand) {
        spline_at(h, &[p([46.0, 30.0]), p([52.0, 36.0]), p([58.0, 30.0]), p([52.0, 24.0]), p([46.0, 30.0])]);
    }
    pub fn nothing(_: &mut Hand) {}
    pub fn circle_and_upright(h: &mut Hand) {
        circle_left(h);
        upright(h);
    }
    pub fn spline_and_upright(h: &mut Hand) {
        spline_left(h);
        upright(h);
    }
    pub fn arc_and_upright(h: &mut Hand) {
        arc_left(h);
        upright(h);
    }
}

use shapes::*;

/// The end of the left line, which goes onto the curve on its right.
const LINE_END: Point2 = p([34.0, 32.0]);

/// A place on the upright line two points are mirrored about.
const AXIS: Point2 = p([41.0, 42.0]);

/// THE CASES.
const CASES: &[Case] = &[
    // a point on every curve: the end of the left line put on the curve right of it
    Case {
        name: "an end of a line on a line",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: line_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([46.0, 22.0]),
    },
    Case {
        name: "an end of a line on a circle",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: circle_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "an end of a line on an arc",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: arc_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([52.0, 29.0]),
    },
    Case {
        name: "an end of a line on an ellipse",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: ellipse_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([54.0, 32.0]),
    },
    Case {
        name: "an end of a line on an open spline",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: spline_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([56.0, 26.0]),
    },
    Case {
        name: "an end of a line on a closed spline",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: closed_spline_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::OnCurve,
        drag: p([58.0, 30.0]),
    },
    Case {
        name: "an end of a line on an end of a spline",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: line_left,
        second: spline_right,
        picks: &[Pick::Point(LINE_END), Pick::Point(p([46.0, 28.0]))],
        holds: Holds::Together,
        drag: p([56.0, 26.0]),
    },
    Case {
        name: "a centre of a circle on an end of an arc",
        code: qymcad_ui_state::ConstraintKind::Coincident,
        first: circle_left,
        second: arc_right,
        picks: &[Pick::Point(p([30.0, 30.0])), Pick::Point(p([58.0, 29.0]))],
        holds: Holds::Together,
        drag: p([52.0, 29.0]),
    },
    // tangency of every straight-round, round-round and free pair
    Case {
        name: "a line and a circle",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: line_left,
        second: circle_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "a line and an arc",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: line_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([52.0, 29.0]),
    },
    Case {
        name: "a line and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: line_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([54.0, 32.0]),
    },
    Case {
        name: "a line and an open spline",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: line_left,
        second: spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([56.0, 26.0]),
    },
    Case {
        name: "two circles",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: circle_left,
        second: circle_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "a circle and an arc",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: circle_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([52.0, 29.0]),
    },
    Case { name: "two arcs", code: qymcad_ui_state::ConstraintKind::Tangent, first: arc_left, second: arc_right, picks: &[Pick::First, Pick::Second], holds: Holds::Touch, drag: p([52.0, 29.0]) },
    Case {
        name: "a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: circle_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([54.0, 32.0]),
    },
    Case {
        name: "a circle and a closed spline",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: circle_left,
        second: closed_spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([58.0, 30.0]),
    },
    Case {
        name: "an ellipse and an open spline",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: ellipse_left,
        second: spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([56.0, 26.0]),
    },
    Case {
        name: "two open splines",
        code: qymcad_ui_state::ConstraintKind::Tangent,
        first: spline_left,
        second: spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Touch,
        drag: p([56.0, 26.0]),
    },
    // equality
    Case { name: "two lines", code: qymcad_ui_state::ConstraintKind::Equal, first: line_left, second: line_right, picks: &[Pick::First, Pick::Second], holds: Holds::Same, drag: p([58.0, 40.0]) },
    Case {
        name: "two circles",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: circle_left,
        second: circle_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Same,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "a circle and an arc",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: circle_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Same,
        drag: p([52.0, 29.0]),
    },
    Case { name: "two arcs", code: qymcad_ui_state::ConstraintKind::Equal, first: arc_left, second: arc_right, picks: &[Pick::First, Pick::Second], holds: Holds::Same, drag: p([52.0, 29.0]) },
    Case {
        name: "two ellipses",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: ellipse_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Same,
        drag: p([54.0, 32.0]),
    },
    // concentricity
    Case {
        name: "two circles",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: circle_left,
        second: circle_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneCentre,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "a circle and an arc",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: circle_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneCentre,
        drag: p([58.0, 29.0]),
    },
    Case {
        name: "two arcs",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: arc_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneCentre,
        drag: p([58.0, 29.0]),
    },
    Case {
        name: "a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: circle_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneCentre,
        drag: p([62.0, 32.0]),
    },
    Case {
        name: "two ellipses",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: ellipse_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneCentre,
        drag: p([62.0, 32.0]),
    },
    // symmetry of any two points about a line
    Case {
        name: "the centres of a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Symmetric,
        first: circle_and_upright,
        second: ellipse_right,
        picks: &[Pick::Point(p([30.0, 30.0])), Pick::Point(p([54.0, 32.0])), Pick::Curve(AXIS)],
        holds: Holds::Mirror,
        drag: p([30.0, 30.0]),
    },
    Case {
        name: "the ends of two splines",
        code: qymcad_ui_state::ConstraintKind::Symmetric,
        first: spline_and_upright,
        second: spline_right,
        picks: &[Pick::Point(p([36.0, 32.0])), Pick::Point(p([46.0, 28.0])), Pick::Curve(AXIS)],
        holds: Holds::Mirror,
        drag: p([25.0, 35.0]),
    },
    Case {
        name: "an end of an arc and of a line",
        code: qymcad_ui_state::ConstraintKind::Symmetric,
        first: arc_and_upright,
        second: line_right,
        picks: &[Pick::Point(p([36.0, 30.0])), Pick::Point(p([46.0, 22.0])), Pick::Curve(AXIS)],
        holds: Holds::Mirror,
        drag: p([58.0, 40.0]),
    },
    // fixing any point
    Case {
        name: "a node of a spline",
        code: qymcad_ui_state::ConstraintKind::Fix,
        first: spline_left,
        second: nothing,
        picks: &[Pick::Point(p([25.0, 35.0]))],
        holds: Holds::Fixed,
        drag: p([25.0, 35.0]),
    },
    Case {
        name: "the centre of an ellipse",
        code: qymcad_ui_state::ConstraintKind::Fix,
        first: ellipse_left,
        second: nothing,
        picks: &[Pick::Point(p([28.0, 30.0]))],
        holds: Holds::Fixed,
        drag: p([28.0, 30.0]),
    },
    Case { name: "an end of an arc", code: qymcad_ui_state::ConstraintKind::Fix, first: arc_left, second: nothing, picks: &[Pick::Point(p([36.0, 30.0]))], holds: Holds::Fixed, drag: p([36.0, 30.0]) },
    // level and upright: a line, the axis of an ellipse, two centres
    Case { name: "a line", code: qymcad_ui_state::ConstraintKind::Horizontal, first: line_left, second: nothing, picks: &[Pick::First], holds: Holds::Level, drag: p([22.0, 26.0]) },
    Case { name: "an ellipse", code: qymcad_ui_state::ConstraintKind::Horizontal, first: ellipse_right, second: nothing, picks: &[Pick::First], holds: Holds::Level, drag: p([62.0, 32.0]) },
    Case {
        name: "the centres of a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Horizontal,
        first: circle_left,
        second: ellipse_right,
        picks: &[Pick::Point(p([30.0, 30.0])), Pick::Point(p([54.0, 32.0]))],
        holds: Holds::Level,
        drag: p([54.0, 32.0]),
    },
    Case { name: "a line", code: qymcad_ui_state::ConstraintKind::Vertical, first: line_left, second: nothing, picks: &[Pick::First], holds: Holds::Upright, drag: p([22.0, 26.0]) },
    Case { name: "an ellipse", code: qymcad_ui_state::ConstraintKind::Vertical, first: ellipse_right, second: nothing, picks: &[Pick::First], holds: Holds::Upright, drag: p([62.0, 32.0]) },
    Case {
        name: "the centres of a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Vertical,
        first: circle_left,
        second: ellipse_right,
        picks: &[Pick::Point(p([30.0, 30.0])), Pick::Point(p([54.0, 32.0]))],
        holds: Holds::Upright,
        drag: p([54.0, 32.0]),
    },
    // parallel, square, collinear: two lines, a line and the axis of an ellipse
    Case {
        name: "two lines",
        code: qymcad_ui_state::ConstraintKind::Parallel,
        first: line_left,
        second: line_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Parallel,
        drag: p([58.0, 40.0]),
    },
    Case {
        name: "a line and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Parallel,
        first: line_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Parallel,
        drag: p([62.0, 32.0]),
    },
    Case {
        name: "two lines",
        code: qymcad_ui_state::ConstraintKind::Perpendicular,
        first: line_left,
        second: line_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Square,
        drag: p([58.0, 40.0]),
    },
    Case {
        name: "a line and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Perpendicular,
        first: line_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Square,
        drag: p([62.0, 32.0]),
    },
    Case {
        name: "two lines",
        code: qymcad_ui_state::ConstraintKind::Collinear,
        first: line_left,
        second: line_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::OneLine,
        drag: p([58.0, 40.0]),
    },
    // the middle of a line and of an arc
    Case {
        name: "a centre of a circle in the middle of a line",
        code: qymcad_ui_state::ConstraintKind::Midpoint,
        first: circle_left,
        second: line_right,
        picks: &[Pick::Point(p([30.0, 30.0])), Pick::Second],
        holds: Holds::Middle,
        drag: p([58.0, 40.0]),
    },
    Case {
        name: "an end of a line in the middle of an arc",
        code: qymcad_ui_state::ConstraintKind::Midpoint,
        first: line_left,
        second: arc_right,
        picks: &[Pick::Point(LINE_END), Pick::Second],
        holds: Holds::Middle,
        drag: p([52.0, 29.0]),
    },
    // no sense on these: the window refuses, in words, and the drawing stays
    Case { name: "an open spline", code: qymcad_ui_state::ConstraintKind::Horizontal, first: spline_right, second: nothing, picks: &[Pick::First], holds: Holds::Refused, drag: p([56.0, 26.0]) },
    Case { name: "a circle", code: qymcad_ui_state::ConstraintKind::Vertical, first: circle_left, second: nothing, picks: &[Pick::First], holds: Holds::Refused, drag: p([30.0, 30.0]) },
    Case {
        name: "two circles",
        code: qymcad_ui_state::ConstraintKind::Parallel,
        first: circle_left,
        second: circle_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([52.0, 31.0]),
    },
    Case {
        name: "a line and an arc",
        code: qymcad_ui_state::ConstraintKind::Perpendicular,
        first: line_left,
        second: arc_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([52.0, 29.0]),
    },
    Case {
        name: "two lines",
        code: qymcad_ui_state::ConstraintKind::Concentric,
        first: line_left,
        second: line_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([58.0, 40.0]),
    },
    Case {
        name: "a line and an open spline",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: line_left,
        second: spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([56.0, 26.0]),
    },
    // a circle has one size and an ellipse two, and the length of a spline is held by nothing a sketch keeps: as in the
    // sketchers a person comes from, neither is made equal
    Case {
        name: "a circle and an ellipse",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: circle_left,
        second: ellipse_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([54.0, 32.0]),
    },
    Case {
        name: "two open splines",
        code: qymcad_ui_state::ConstraintKind::Equal,
        first: spline_left,
        second: spline_right,
        picks: &[Pick::First, Pick::Second],
        holds: Holds::Refused,
        drag: p([56.0, 26.0]),
    },
    Case { name: "two arcs", code: qymcad_ui_state::ConstraintKind::Collinear, first: arc_left, second: arc_right, picks: &[Pick::First, Pick::Second], holds: Holds::Refused, drag: p([52.0, 29.0]) },
];

/// THE SHAPES OF A CASE DRAWN BY HAND, and what of them a cell reads: the curves of each shape and the points picked.
struct Laid {
    first: Vec<qymcad_ui_state::SketchItem>,
    second: Vec<qymcad_ui_state::SketchItem>,
    points: Vec<qymcad_ui_state::SketchItem>,
    line: Option<qymcad_ui_state::SketchItem>,
    dragged: qymcad_ui_state::SketchItem,
}

/// A DOCUMENT WITH THE SHAPES OF A CASE DRAWN, and what of them a cell reads.
struct Ready {
    app: App,
    laid: Laid,
}

fn lay(case: &Case) -> Result<Ready, String> {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    let mut hand = Hand::new(&mut app);
    let keys = |hand: &Hand| drawing(hand.app).curves.iter().map(|c| c.key).collect::<Vec<_>>();
    (case.first)(&mut hand);
    let first = keys(&hand);
    (case.second)(&mut hand);
    let second: Vec<qymcad_ui_state::SketchItem> = keys(&hand).into_iter().filter(|k| !first.contains(k)).collect();
    let d = drawing(hand.app);
    let dot = |at: Point2| d.points.iter().find(|q| q.at.dist(at) < 0.5).map(|q| q.key).ok_or_else(|| format!("no point at ({:.1}, {:.1}) when the shapes are drawn", at.x, at.y));
    let points = case.picks.iter().filter_map(|k| if let Pick::Point(at) = k { Some(dot(*at)) } else { None }).collect::<Result<Vec<_>, _>>()?;
    let line = case.picks.iter().find_map(|k| if let Pick::Curve(at) = k { d.curves.iter().find(|c| off_path(*at, &c.path) < NEAR).map(|c| c.key) } else { None });
    let dragged = dot(case.drag)?;
    if first.is_empty() && points.is_empty() {
        return Err("the first shape left nothing on the sheet".into());
    }
    Ok(Ready { app, laid: Laid { first, second, points, line, dragged } })
}

/// The curve of `key`, as the drawing holds it.
fn curve<'a>(d: &'a Drawing, key: Option<&qymcad_ui_state::SketchItem>) -> Result<&'a Curve, String> {
    key.and_then(|k| d.curves.iter().find(|c| c.key == *k)).ok_or_else(|| "a curve of the case is gone".to_string())
}

/// Where the point of `key` stands.
fn dot_at(d: &Drawing, key: qymcad_ui_state::SketchItem) -> Option<Point2> {
    d.points.iter().find(|q| q.key == key).map(|q| q.at)
}

/// The centre of a round curve or an ellipse, from its points.
fn centre(c: &Curve) -> Point2 {
    if c.kind == "ellipse" {
        let n = (c.path.len() - 1) as f64;
        let sum = c.path[..c.path.len() - 1].iter().fold(Point2::new(0.0, 0.0), |s, q| s + *q);
        return Point2::new(sum.x / n, sum.y / n);
    }
    let n = c.path.len();
    centre_of([c.path[0], c.path[n / 3], c.path[2 * n / 3]])
}

/// The size of a curve as equality reads it: a straight's length, a round curve's radius, an ellipse's two semi-axes.
fn size(c: &Curve) -> [f64; 2] {
    match c.kind {
        "line" => [c.path[0].dist(c.path[c.path.len() - 1]), 0.0],
        "ellipse" => {
            let m = centre(c);
            let far = c.path.iter().map(|q| q.dist(m)).fold(0.0, f64::max);
            let near = c.path.iter().map(|q| q.dist(m)).fold(f64::MAX, f64::min);
            [far, near]
        }
        _ => [centre(c).dist(c.path[0]), 0.0],
    }
}

/// The way a straight runs: a line from its start to its end, an ellipse along its major axis.
fn way_of(c: &Curve) -> Point2 {
    let (a, b) = if c.kind == "ellipse" {
        let m = centre(c);
        (m, c.path.iter().copied().max_by(|x, y| x.dist(m).total_cmp(&y.dist(m))).unwrap_or(m))
    } else {
        (c.path[0], c.path[c.path.len() - 1])
    };
    let l = a.dist(b).max(1e-12);
    Point2::new((b.x - a.x) / l, (b.y - a.y) / l)
}

/// THE WHOLE CURVE A CONSTRAINT TIES TO: the circle of an arc - a point put on an arc, or a line made to touch it, may
/// stand past its ends, as in every sketcher - and the curve itself for any other kind. A round curve is read at 720
/// points: read at the 96 it is drawn by, the place two touching circles meet was a 96th of a turn off, and the ways of
/// the two there 2.6 deg apart.
fn whole(c: &Curve) -> Vec<Point2> {
    if !matches!(c.kind, "arc" | "circle") {
        return c.path.clone();
    }
    let (m, r) = (centre(c), centre(c).dist(c.path[0]));
    (0..=720).map(|k| f64::from(k) / 720.0 * std::f64::consts::TAU).map(|t| Point2::new(m.x + r * t.cos(), m.y + r * t.sin())).collect()
}

/// The distance from `p` to the whole line through a straight.
fn off_line(p: Point2, c: &Curve) -> f64 {
    let (a, w) = (c.path[0], way_of(c));
    ((p.x - a.x) * w.y - (p.y - a.y) * w.x).abs()
}

/// The direction of a curve at `p`: square to the radius of a round curve, along a line, and on any other curve square to
/// the radius of the circle through the three points of it about `p` - along one segment it tilted by half the turn of
/// the curve over the segment, 10 deg at the end of an ellipse 14 by 4.
fn tangent_of(c: &Curve, p: Point2) -> Point2 {
    let square_to = |m: Point2| {
        let r = m.dist(p);
        Point2::new(-(p.y - m.y) / r, (p.x - m.x) / r)
    };
    if matches!(c.kind, "arc" | "circle") {
        return square_to(centre(c));
    }
    let n = c.path.len();
    let k = c.path.windows(2).enumerate().min_by(|a, b| off_path(p, a.1).total_cmp(&off_path(p, b.1))).map_or(0, |(k, _)| k);
    if c.kind != "line" && n >= 3 {
        let k = (0..n).min_by(|a, b| c.path[*a].dist(p).total_cmp(&c.path[*b].dist(p))).unwrap_or(0).clamp(1, n - 2);
        let [a, m, b] = [c.path[k - 1], c.path[k], c.path[k + 1]];
        if ((m.x - a.x) * (b.y - a.y) - (m.y - a.y) * (b.x - a.x)).abs() > 1e-12 {
            return square_to(centre_of([a, m, b]));
        }
    }
    let (a, b) = (c.path[k], c.path[(k + 1).min(n - 1)]);
    let l = a.dist(b).max(1e-12);
    Point2::new((b.x - a.x) / l, (b.y - a.y) / l)
}

fn apart(ways: [Point2; 2]) -> f64 {
    let [a, b] = ways;
    (a.x * b.x + a.y * b.y).abs().min(1.0).acos().to_degrees()
}

/// WHETHER WHAT THE CASE TIES HOLDS in `d`, in words when it does not.
fn holds(case: &Case, laid: &Laid, d: &Drawing) -> Result<(), String> {
    let pts: Vec<Option<Point2>> = laid.points.iter().map(|k| dot_at(d, *k)).collect();
    let pt = |k: usize| pts.get(k).copied().flatten().ok_or_else(|| "a point of the case is gone".to_string());
    let a = || curve(d, laid.first.first());
    let b = || curve(d, laid.second.first());
    let close = |v: f64, what: &str| if v < NEAR { Ok(()) } else { Err(format!("{what} is {v:.3} off")) };
    match case.holds {
        Holds::OnCurve => {
            let (p, c) = (pt(0)?, b()?);
            close(if c.kind == "line" { off_line(p, c) } else { off_path(p, &whole(c)) }, "the point")
        }
        Holds::Together => match (pts[0], pts.get(1).copied().flatten()) {
            (Some(p), Some(q)) => close(p.dist(q), "the second point"),
            _ => Ok(()),
        },
        Holds::Touch => {
            let (a, b) = (a()?, b()?);
            let (wa, wb) = (whole(a), whole(b));
            let at = wa.iter().copied().min_by(|x, y| off_path(*x, &wb).total_cmp(&off_path(*y, &wb))).ok_or("no points")?;
            close(off_path(at, &wb), "the nearest point of the two curves")?;
            let turn = apart([tangent_of(a, at), tangent_of(b, at)]);
            if turn > 2.5 {
                return Err(format!("the curves meet at {turn:.2} deg, not touching"));
            }
            Ok(())
        }
        Holds::Same => {
            let (x, y) = (size(a()?), size(b()?));
            close((x[0] - y[0]).abs().max((x[1] - y[1]).abs()), "the size of the second curve")
        }
        Holds::OneCentre => close(centre(a()?).dist(centre(b()?)), "the second centre"),
        Holds::Mirror => {
            let (p, q, l) = (pt(0)?, pt(1)?, curve(d, laid.line.as_ref())?);
            let m = along([p, q], 0.5);
            close(off_line(m, l), "the middle of the two points")?;
            let pq = q - p;
            let w = way_of(l);
            close((pq.x * w.x + pq.y * w.y).abs(), "the run of the two points along the line")
        }
        Holds::Fixed => Ok(()),
        Holds::Level | Holds::Upright => {
            let run = if laid.points.len() >= 2 {
                let (p, q) = (pt(0)?, pt(1)?);
                if matches!(case.holds, Holds::Level) {
                    (p.y - q.y).abs()
                } else {
                    (p.x - q.x).abs()
                }
            } else {
                let w = way_of(a()?);
                if matches!(case.holds, Holds::Level) {
                    w.y.abs()
                } else {
                    w.x.abs()
                }
            };
            close(run, "the rise")
        }
        Holds::Parallel => close(apart([way_of(a()?), way_of(b()?)]).to_radians(), "the turn between them"),
        Holds::Square => close((90.0 - apart([way_of(a()?), way_of(b()?)])).abs().to_radians(), "the turn from square"),
        Holds::OneLine => {
            let (a, b) = (a()?, b()?);
            close(off_line(b.path[0], a).max(off_line(b.path[b.path.len() - 1], a)), "the second line")
        }
        Holds::Middle => {
            let (p, c) = (pt(0)?, b()?);
            close(p.dist(share_along(&c.path, 0.5)), "the point from the middle")
        }
        Holds::Refused => Ok(()),
    }
}

/// The place a click picks `pick` by.
fn place_of(hand: &mut Hand, laid: &Laid, pick: Pick) -> Result<Point2, String> {
    let d = drawing(hand.app);
    match pick {
        Pick::First | Pick::Second => {
            let key = if matches!(pick, Pick::First) { laid.first.first() } else { laid.second.first() };
            let c = curve(&d, key)?.clone();
            Ok(spot(hand, &c))
        }
        Pick::Point(at) | Pick::Curve(at) => Ok(at),
    }
}

/// ONE CASE LAID ONE WAY: the constraint holds, and holds again with a point of the geometry dragged.
fn tie(case: &Case, way: Way) -> Result<(), String> {
    let Ready { mut app, laid } = lay(case)?;
    let constraints = |a: &App| a.project.sketches.iter().map(|s| s.constraints.len()).sum::<usize>();
    let (laid_before, drawn_before) = (constraints(&app), drawing(&app));
    let mut hand = Hand::new(&mut app);
    let places = case.picks.iter().map(|k| place_of(&mut hand, &laid, *k)).collect::<Result<Vec<_>, _>>()?;
    hand.sk_select();
    if way == Way::ButtonFirst {
        hand.constraint(case.code);
    }
    for (k, at) in places.iter().enumerate() {
        if k == 0 || way == Way::ButtonFirst {
            hand.click2d(at.x, at.y);
        } else {
            hand.shift_click2d(at.x, at.y);
        }
    }
    if way == Way::PickedBefore {
        hand.constraint(case.code);
    }
    let said = |hand: &Hand, why: String| format!("{why}; the status line says {:?}", hand.app.status);
    if matches!(case.holds, Holds::Refused) {
        let (laid_now, drawn_now) = (constraints(hand.app), drawing(hand.app));
        let moved = drawn_before.curves.iter().zip(&drawn_now.curves).any(|(x, y)| x.path.iter().zip(&y.path).any(|(p, q)| p.dist(*q) > NEAR));
        return match (laid_now == laid_before, moved, hand.app.status.tone == qymcad_ui_state::Tone::Refusal) {
            (true, false, true) => Ok(()),
            _ => Err(said(&hand, format!("laid {} constraints, the drawing moved {moved}", laid_now - laid_before))),
        };
    }
    holds(case, &laid, &drawing(hand.app)).map_err(|why| said(&hand, why))?;
    // a point of the geometry dragged: what the constraint ties goes with it
    let d = drawing(hand.app);
    let from = dot_at(&d, laid.dragged).ok_or("the dragged point is gone")?;
    hand.sk_select();
    hand.drag2d((from.x, from.y), (from.x + 1.5, from.y + 1.0));
    let after = drawing(hand.app);
    if matches!(case.holds, Holds::Fixed) {
        let now = dot_at(&after, laid.dragged).ok_or("the fixed point is gone")?;
        return if now.dist(from) < NEAR { Ok(()) } else { Err(said(&hand, format!("the fixed point went {:.3} with the drag", now.dist(from)))) };
    }
    holds(case, &laid, &after).map_err(|why| said(&hand, format!("after a drag: {why}")))
}

/// The name of a constraint by its code, as the panel numbers its buttons.
fn named(code: qymcad_ui_state::ConstraintKind) -> String {
    format!("{code:?}").to_lowercase()
}

fn constraint_cells(way: Way) {
    let mut failures = Vec::new();
    for case in CASES {
        cell(&mut failures, format!("{} of {} by {way:?}", named(case.code), case.name), || tie(case, way));
    }
    said(&format!("the constraints by {way:?}"), CASES.len(), &failures);
}

#[test]
fn every_constraint_takes_every_kind_picked_before() {
    constraint_cells(Way::PickedBefore);
}

#[test]
fn every_constraint_takes_every_kind_button_first() {
    constraint_cells(Way::ButtonFirst);
}
