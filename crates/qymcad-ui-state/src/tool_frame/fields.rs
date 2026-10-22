//! THE BOX OF THE TOOL IN HAND AT THE GEOMETRY: its fields - formulas, each judged as it is typed - the options it
//! describes for the box, Apply and Cancel, and Tab going round the fields.

use super::*;

/// WHERE A BOX OF FIELDS STANDS and how it is told apart.
pub struct FieldsBox<'a> {
    /// The box itself: egui keeps its size between frames under this id.
    pub id: egui::Id,
    /// What the fields of this tool are told apart from those of another by: a field with the same key in the box of
    /// another tool keeps nothing typed into this one.
    pub tool: u64,
    /// What it stands by: the frame lays it beside that by one rule for every box (`place_by_the_rule`).
    pub place: Place,
    /// The first field takes the keyboard in this frame, its value selected.
    pub focus: Focus,
    /// The keys of the fields that take no part now and are not shown.
    pub hidden: &'a [&'a str],
    /// How the box ends, as the bar of its tool does.
    pub ends: Ends,
    /// Who takes Esc: the bar of its tool, or the box itself.
    pub esc: EscBy,
    /// How its fields and options are laid: rows, sections, the buttons of its foot.
    pub layout: &'a Layout,
    /// The tool's own word on whether it can be applied: the picks, the trial build. The fields are judged by the frame.
    pub gate: &'a Gate,
}

/// The id of field `key` of the box of tool `tool`: the field keeps what is typed into it under it, and a check reaches
/// the field by it.
pub fn field_id(tool: u64, key: &str) -> egui::Id {
    egui::Id::new(("cmdparam", tool, key.to_string()))
}

/// WHO TAKES Esc FOR A BOX: the bar of its tool, or the box itself where its tool has no bar - the numeric entry at a
/// gizmo. A field holding the keyboard takes it first either way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EscBy {
    Bar,
    Box,
}

/// WHAT A BOX OF FIELDS IS DRAWN WITH: the document its formulas read, the colours, and the view the fields standing at
/// a point of their own are placed on - made by the frame of what every workbench draws with (`DrawCtx`).
pub(crate) struct BoxLook<'a> {
    pub project: &'a Project,
    pub pal: &'a qymcad_scheme::Palette,
    pub screen: Screen<'a>,
    /// The context worked in: a body is found on the screen where it is drawn in it.
    pub here: Id,
}

/// One field of a box: its caption and its formula; whether it holds a value it can take, the reason when not; and
/// whether Enter was pressed in it.
fn field_row(ui: &mut egui::Ui, look: &BoxLook, p: &mut CmdParam, id: egui::Id, focus: Focus) -> FieldSays {
    ui.label(field_caption(p));
    field_input(ui, look, p, id, focus)
}

/// The formula of one field, its caption standing elsewhere - in the row of a section.
pub(crate) fn field_input(ui: &mut egui::Ui, look: &BoxLook, p: &mut CmdParam, id: egui::Id, focus: Focus) -> FieldSays {
    let o = expr_field_autofocus(ui, look.project, id, &p.txt, 74.0, &qymcad_i18n::tr("f-number-or-expr"), focus == Focus::Takes);
    p.txt = o.text;
    let enter = o.resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let mut did = Vec::new();
    if o.resp.changed() {
        did.push(FieldDid::Typed);
    }
    if o.cancelled {
        did.push(FieldDid::LeftByEsc);
    }
    FieldSays { value: field_value(p, &look.project.param_map()), enter, did }
}

/// WHAT A FIELD SAID IN A FRAME: its value or why it has none, whether Enter was pressed in it, and what else it did.
pub(crate) struct FieldSays {
    value: Result<f64, String>,
    enter: bool,
    did: Vec<FieldDid>,
}

/// WHAT A FIELD OF A BOX DID in a frame besides its value, told to its tool: it was typed into - a value typed equal to
/// the one it held is typed all the same - or it gave the keyboard up by Esc, the field taking Esc first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldDid {
    Typed,
    LeftByEsc,
}

/// WHAT A FIELD COMES TO, its value taken into it: a value it cannot take - by its own form and limits, or the
/// geometry's word on it (`trial`) - is marked beside it with why; Enter pressed in it is Enter either way, answered by
/// the gate in words when the value is refused.
pub(crate) fn judge(ui: &mut egui::Ui, pal: &qymcad_scheme::Palette, p: &mut CmdParam, said: FieldSays, trial: Option<&Refusal>) -> Typed {
    let why = match said.value {
        Err(why) => Some(why),
        Ok(v) => {
            p.val = v;
            trial.filter(|r| r.key == p.key).map(|r| r.why.clone())
        }
    };
    let refused = why.map(|why| {
        tone_mark(ui, pal, Tone::Refusal).on_hover_text(&why);
        Refusal { key: p.key.clone(), field: field_caption(p), why }
    });
    Typed { refused, enter: if said.enter { Enter::Pressed } else { Enter::Not }, did: said.did }
}

/// WHAT THE FIELDS OF A BOX CAME TO TOGETHER: the first value refused, and whether Enter was pressed in one of them.
#[derive(Default)]
pub(crate) struct Typed {
    refused: Option<Refusal>,
    enter: Enter,
    did: Vec<FieldDid>,
}

impl Typed {
    pub(crate) fn add(&mut self, field: Typed) {
        if let Some(r) = field.refused {
            self.refused.get_or_insert(r);
        }
        if field.enter == Enter::Pressed {
            self.enter = Enter::Pressed;
        }
        self.did.extend(field.did);
    }
}

/// THE BOX OF THE TOOL IN HAND, at the geometry: a field with a point of its own (a radius at a vertex) in a small box
/// at that point, the rest in one box by `b.place` as its `layout` lays them - rows of a caption and a value, in sections -
/// the buttons of its foot and the ends of its tool at the bottom.
/// Enter in a field applies through the gate; a field refusing its value marks itself with why and shuts the gate, and
/// a refused Enter is said with the words of the tool. `b.gate` is the tool's own - the picks and the trial build; the
/// fields are judged here, as they are typed. The box stands in the view `rect` of what `draw` draws.
pub fn tool_box(ctx: &egui::Context, draw: &DrawCtx, rect: egui::Rect, b: &FieldsBox, fields: &mut [CmdParam]) -> Answer {
    let basis = draw.cam.basis();
    let here = current_ctx_id(draw.active_path, draw.project);
    let look = &BoxLook { project: draw.project, pal: &draw.scheme.pal, screen: Screen { cam: draw.cam, set: draw.set, rect, basis: &basis }, here };
    let layout = b.layout;
    let gate = b.gate;
    // asked before a field of the box is drawn: a field takes Esc and gives the keyboard up in this frame
    let typing = ctx.egui_wants_keyboard_input();
    let trial = match gate {
        Gate::Refused(r) => Some(r.clone()),
        _ => None,
    };
    let mut seen = Typed::default();
    // THE FIELDS AT A POINT OF THEIR OWN: six identical fields in a column cannot be told apart, and which of them is
    // which corner is the only thing that matters there
    for (i, p) in fields.iter_mut().enumerate() {
        let Some(w) = p.at else { continue };
        let id = b.id.with(("at", i));
        let at = box_corner(ctx, id, Place::close_by(By::Point(w)), look);
        let shown = egui::Area::new(id).current_pos(at).movable(true).order(AT_GEOMETRY).show(ctx, |ui| {
            at_geometry_ui(ui);
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let said = field_row(ui, look, p, field_id(b.tool, &p.key), Focus::Waits);
                    seen.add(judge(ui, look.pal, p, said, trial.as_ref()));
                });
            });
        });
        moved_by_hand(ctx, id, &shown.response);
    }
    tab_round(ctx, b, fields, layout);
    let mut pressed = Pressed::Nothing;
    let mut did = Vec::new();
    // THE BOX IS MOVED BY ITS FREE SPACE - its captions, its margins - and stands where it is put until its tool goes
    let at = box_corner(ctx, b.id, b.place, look);
    let shown = egui::Area::new(b.id).current_pos(at).movable(true).order(AT_GEOMETRY).show(ctx, |ui| {
        at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            let drawing = Drawing { b, look, trial: trial.as_ref() };
            let drawn = draw_rows(ui, &drawing, fields, layout, b.focus);
            seen.add(drawn.typed);
            did = drawn.did;
            let can = if shut_by(gate, &seen, ui.ctx()).open() { Can::Yes } else { Can::No };
            pressed = box_ends(ui, b, look.pal, can);
        });
    });
    moved_by_hand(ctx, b.id, &shown.response);
    let esc = b.esc == EscBy::Box && !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape) && !i.modifiers.any());
    let keys = Keys { enter: seen.enter, esc: if esc { Esc::Pressed } else { Esc::Not } };
    let verdict = verdict(ctx, pressed, keys, &shut_by(gate, &seen, ctx));
    Answer { verdict, did, fields: seen.did }
}

/// THE ENDS OF A BOX, under a line of their own and centred across it. Reported behaviour: Apply and Cancel stood
/// left, 6.6 px under the buttons of the foot of a joint's box, and read as two more of them. The width of the two is
/// the one they took the frame before: a row is laid from the left, and how wide it comes out is known once it is laid.
fn box_ends(ui: &mut egui::Ui, b: &FieldsBox, pal: &qymcad_scheme::Palette, can: Can) -> Pressed {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
    let across = ui.min_rect().width();
    let id = b.id.with("the width of the ends");
    let was = ui.ctx().data(|d| d.get_temp::<f32>(id));
    let mut pressed = Pressed::Nothing;
    ui.horizontal(|ui| {
        ui.add_space(((across - was.unwrap_or(across)) / 2.0).max(0.0));
        let laid = ui.horizontal(|ui| pressed = the_ends(pal, ui, b.ends, can)).response.rect.width();
        // THE WIDEST THE ENDS WERE LAID IS KEPT: by a box pulled against the edge of the view the offset that centres them
        // narrows the row it measures, and a width kept as last measured swung 203.7 <-> 201.7 every frame, asking for a
        // frame each time - the window never came to rest
        if was.is_none_or(|w| laid > w + 0.5) {
            ui.ctx().data_mut(|d| d.insert_temp(id, laid));
            ui.ctx().request_repaint();
        }
    });
    pressed
}

/// BOX `id` MOVED BY A PERSON, as `r` answers: by the primary button, and only once the pointer moved - egui holds the box
/// as dragged from the press of a button in it, and a press of "Run" taken for a move froze the box where it stood; a
/// drag of the view by another button that begins over the box moves the view, not the box.
fn moved_by_hand(ctx: &egui::Context, id: egui::Id, r: &egui::Response) {
    if r.dragged_by(egui::PointerButton::Primary) && r.drag_delta() != egui::Vec2::ZERO {
        box_moved(ctx, id, r.rect.min);
    }
}

/// TAB GOES ROUND THE FIELDS OF THE BOX: from one field to the next, from the last to the first, Shift+Tab back - not on
/// to the buttons under them, where what is typed next goes nowhere. Asked before the fields are drawn, so the key is the
/// box's and egui's own walk through the widgets does not follow it. The field come to by Tab holds its value selected,
/// so what is typed takes its place: "60", Tab, "35" made a height of 30.0035 of the 30.00 the field held.
fn tab_round(ctx: &egui::Context, b: &FieldsBox, fields: &[CmdParam], layout: &Layout) {
    let keys: Vec<String> = shown_keys(ctx, fields, b, layout).into_iter().chain(fields.iter().filter(|p| p.at.is_some()).map(|p| p.key.clone())).collect();
    let ids: Vec<egui::Id> = keys.iter().map(|key| field_id(b.tool, key)).collect();
    let Some(i) = ctx.memory(|m| m.focused()).and_then(|f| ids.iter().position(|id| *id == f)) else { return };
    let back = ctx.input(|i| i.modifiers.shift);
    let held = if back { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
    if ctx.input_mut(|inp| inp.consume_key(held, egui::Key::Tab)) {
        let next = if back { (i + ids.len() - 1) % ids.len() } else { (i + 1) % ids.len() };
        ctx.memory_mut(|m| {
            m.request_focus(ids[next]);
            m.move_focus(egui::FocusDirection::None);
        });
        let held = fields.iter().find(|p| p.key == keys[next]).map_or(0, |p| p.txt.chars().count());
        let mut state = egui::TextEdit::load_state(ctx, ids[next]).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(held))));
        state.store(ctx, ids[next]);
    }
}

/// The gate as the fields of a box leave it: a value refused there first, a field of the bar refusing its value next,
/// the tool's own gate otherwise.
fn shut_by(gate: &Gate, typed: &Typed, ctx: &egui::Context) -> Gate {
    match (&typed.refused, gate) {
        (Some(r), _) => Gate::Refused(r.clone()),
        (None, Gate::Open) if !bar_fields_valid(ctx) => Gate::BarField,
        (None, g) => g.clone(),
    }
}
