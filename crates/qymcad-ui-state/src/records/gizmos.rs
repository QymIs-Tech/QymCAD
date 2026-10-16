//! The gizmos, the drags and the edits made on the sheet, and the measure.

use super::*;

/// THE COMPONENT GIZMO IN AN ASSEMBLY: what is being dragged, along which axis or ring.
#[derive(Clone, Copy, Default)]
pub struct CompGizmo {
    pub drag: Option<(Id, [f64; 12], [f64; 3], f64)>,
    pub snap: bool,
    pub axis: Option<u8>,
    pub ring: Option<u8>,
}

/// TYPING A ROTATION ANGLE: the value, the buffer and the focus — one unfinished input.
#[derive(Clone, Default)]
pub struct RotInput {
    pub angle: f64,
    pub buf: String,
    pub focus: bool,
}

/// WHAT IS BEING DRAGGED IN THE SKETCH RIGHT NOW.
///
/// AN ENUM, NOT SIX `Option`s: exactly ONE THING can be dragged. As separate fields a pair of them
/// could end up occupied at once, and the drag handlers fought over a single pointer — a state that
/// never happens, but which the type allowed.
#[derive(Clone, Default, PartialEq)]
pub enum Dragging {
    #[default]
    None,
    /// a sketch point, as (sketch index, point index)
    Point(usize, usize),
    /// a spline handle, as (sketch, spline, node)
    Handle(usize, usize, usize),
    /// moving the whole selected geometry
    Move(usize, Vec<Id>),
    /// a dimension label / a note / a text
    Dim(usize),
    Note(usize),
    Text(usize),
}

impl Dragging {
    /// Is anything being dragged?
    pub fn active(&self) -> bool {
        *self != Dragging::None
    }

    pub fn clear(&mut self) {
        *self = Dragging::None;
    }

    pub fn pt(&self) -> Option<(usize, usize)> {
        match *self {
            Dragging::Point(a, b) => Some((a, b)),
            _ => None,
        }
    }

    pub fn handle(&self) -> Option<(usize, usize, usize)> {
        match *self {
            Dragging::Handle(a, b, c) => Some((a, b, c)),
            _ => None,
        }
    }

    pub fn mov(&self) -> Option<(usize, Vec<Id>)> {
        match self {
            Dragging::Move(a, ids) => Some((*a, ids.clone())),
            _ => None,
        }
    }

    pub fn dim(&self) -> Option<usize> {
        match *self {
            Dragging::Dim(i) => Some(i),
            _ => None,
        }
    }

    pub fn note(&self) -> Option<usize> {
        match *self {
            Dragging::Note(i) => Some(i),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<usize> {
        match *self {
            Dragging::Text(i) => Some(i),
            _ => None,
        }
    }
}

/// WHAT A GIZMO STANDS ON, for a number typed at it: a body in a part (by its mesh), or a part in an assembly.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoOf {
    Body(usize),
    Part(Id),
}

/// THE BODY GIZMO: moving or rotating a body with the mouse, plus typing the same value as a number.
/// The same pairing as `CompGizmo` has for a component: "what is being dragged" and "what is being
/// typed" are one intent, and as separate fields the input could outlive the release of the gizmo.
#[derive(Clone, Default)]
pub struct BodyGizmo {
    pub axis: Option<u8>,
    pub ring: Option<u8>,
    pub drag: Option<(usize, [f64; 3], f64)>,
    pub snap: bool,
    /// the gizmo itself is being dragged (an arrow or a ring has been grabbed)
    pub dragging: bool,
    /// typed input: (what the gizmo stands on, axis, is it a rotation?) plus its buffer and focus
    pub num: Option<(GizmoOf, u8, bool)>,
    pub num_buf: String,
    pub num_focus: bool,
}

/// IN-PLACE EDITING OF A SKETCH ELEMENT: what exactly is being edited right on the canvas. Exactly one
/// thing is edited at a time — hence an enum rather than four `Option`s.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum InlineEdit {
    #[default]
    None,
    Note(usize),
    Text(usize),
    Dim(usize),
    Circle(Id),
}

impl InlineEdit {
    pub fn note(&self) -> Option<usize> {
        match *self {
            InlineEdit::Note(i) => Some(i),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<usize> {
        match *self {
            InlineEdit::Text(i) => Some(i),
            _ => None,
        }
    }
    pub fn dim(&self) -> Option<usize> {
        match *self {
            InlineEdit::Dim(i) => Some(i),
            _ => None,
        }
    }
    pub fn circle(&self) -> Option<Id> {
        match *self {
            InlineEdit::Circle(i) => Some(i),
            _ => None,
        }
    }
    pub fn clear(&mut self) {
        *self = InlineEdit::None;
    }
}

/// EDITING AN ANNOTATION (a note or a text) in a sketch: what is being edited and what has been typed.
#[derive(Clone, Default)]
pub struct AnnotEdit {
    pub note: Option<usize>,
    pub note_buf: String,
    pub text: Option<usize>,
    pub text_buf: String,
    pub text_h: f64,
    /// The text field of the popup has just been opened and asks for the caret.
    ///
    /// Without it the popup opened with the focus nowhere: a person had to click into the field before
    /// typing, and Enter - which is what applies the change - did nothing at all until they did.
    pub text_focus: bool,
}

/// THE MEASURING TOOL: whether it is on, together with the points collected — one state, not a flag
/// apart from the points (otherwise the points outlive the switch-off and pop up the next time it is
/// turned on).
#[derive(Clone, Default)]
pub struct Measuring {
    pub pts: Vec<Point2>,
}

impl Measuring {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
