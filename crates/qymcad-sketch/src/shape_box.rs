//! THE BOX OF A TOOL ON THE SHEET, in the frame of every tool: a shape just drawn - an ellipse, a rectangle, a polygon,
//! a slot - shows its sizes as fields beside it, the turn of a rotation its angle by its centre. A value typed is taken
//! at once; Enter, Close, Cancel and Esc put the box away, as the box of any tool is put away.

use super::*;

/// WHAT THE BOX OF A TOOL ON THE SHEET IS DRAWN WITH: the colours, the camera of the view and the settings.
pub(crate) struct SheetLook<'a> {
    pub(crate) scheme: &'a qymcad_ui_state::SchemeUi,
    pub(crate) cam: &'a qymcad_ui_state::Cam3,
    pub(crate) set: &'a qymcad_ui_state::Settings,
}

impl<'a> SheetLook<'a> {
    /// The look of the boxes of the shapes, from what the popups at the geometry are drawn with.
    pub(crate) fn of(looks: &'a qymcad_ui_state::PopupLooks) -> Self {
        SheetLook { scheme: looks.scheme, cam: looks.cam, set: looks.set }
    }
}

/// WHERE THE BOX STANDS AND WHAT IT HOLDS: its id, the point it stands by and the corners of what it sizes on the
/// screen, how it ends, its fields and whether the first takes the keyboard in this frame.
pub(crate) struct ShapeBox<'f> {
    pub(crate) id: egui::Id,
    pub(crate) at: Pos2,
    pub(crate) around: Vec<Pos2>,
    pub(crate) ends: qymcad_ui_state::Ends,
    pub(crate) fields: &'f mut [qymcad_ui_state::CmdParam],
    pub(crate) focus: &'f mut bool,
}

/// WHAT THE BOX CAME TO in a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ShapeSays {
    /// Up, nothing new typed.
    Up,
    /// Up, a value typed into one of its fields that the field takes: what it sizes follows it.
    Typed,
    /// Put away by Enter in a field or by Apply: what was typed is meant.
    Applied,
    /// Put away by Close or Cancel, or by Esc once no field holds the keyboard.
    Closed,
}

/// THE BOX OF A TOOL ON THE SHEET `rect`, standing by `b`.
pub(crate) fn shape_box(ctx: &egui::Context, look: &SheetLook, project: &Project, rect: Rect, b: ShapeBox) -> ShapeSays {
    let basis = look.cam.basis();
    let box_look = qymcad_ui_state::BoxLook { project, pal: &look.scheme.pal, screen: qymcad_ui_state::Screen { cam: look.cam, set: look.set, rect, basis: &basis }, here: project.root };
    let focus = if std::mem::take(b.focus) { qymcad_ui_state::Focus::Takes } else { qymcad_ui_state::Focus::Waits };
    let the_box = qymcad_ui_state::FieldsBox {
        id: b.id,
        tool: b.id.value(),
        place: qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(b.at, &b.around)),
        focus,
        hidden: &[],
        ends: b.ends,
        esc: qymcad_ui_state::EscBy::Box,
    };
    let before: Vec<f64> = b.fields.iter().map(|p| p.val).collect();
    let answer = qymcad_ui_state::tool_box(ctx, &the_box, &box_look, b.fields, &qymcad_ui_state::Layout::default(), &qymcad_ui_state::Gate::Open);
    match answer.verdict {
        qymcad_ui_state::Verdict::Apply => ShapeSays::Applied,
        qymcad_ui_state::Verdict::Cancel | qymcad_ui_state::Verdict::Back => ShapeSays::Closed,
        _ if b.fields.iter().zip(&before).any(|(p, v)| p.val != *v) => ShapeSays::Typed,
        _ => ShapeSays::Up,
    }
}

/// THE BOX OF THE SIZES OF A SHAPE, its fields `place.fields`, standing by `at` clear of `around` on the sheet `rect`.
pub(crate) fn place_box(ctx: &egui::Context, looks: &qymcad_ui_state::PopupLooks, project: &Project, place: &mut qymcad_ui_state::Placing, rect: Rect, by: SheetSpot) -> ShapeSays {
    let qymcad_ui_state::Placing { fields, focus, .. } = place;
    shape_box(ctx, &SheetLook::of(looks), project, rect, ShapeBox { id: by.id, at: by.at, around: by.around, ends: qymcad_ui_state::Ends::Close, fields, focus })
}

/// WHERE THE BOX OF A SHAPE STANDS: its id, and the point it stands by and the corners of the shape, on the screen.
pub(crate) struct SheetSpot {
    pub(crate) id: egui::Id,
    pub(crate) at: Pos2,
    pub(crate) around: Vec<Pos2>,
}

/// The value field `key` of the box holds.
pub(crate) fn field(place: &qymcad_ui_state::Placing, key: &str) -> Option<f64> {
    place.fields.iter().find(|p| p.key == key).map(|p| p.val)
}
