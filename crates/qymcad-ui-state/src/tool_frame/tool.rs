//! THE TOOL IN HAND, DRIVEN BY THE FRAME: a workbench hands the frame its tool and answers its questions - what the tool
//! is now, its fields, what Apply does; the frame draws the bar and the box, takes the keys, goes through the doors and
//! says the words. A new workbench writes `impl FramedTool` for its tools and gets all of it.

use super::*;

/// A TOOL IN HAND, as a workbench hands it to the frame. The workbench answers; the frame does everything else.
pub trait FramedTool {
    /// What it is now: its name, the options of its bar, its gate, its words, and its box if it has one.
    fn describe(&mut self, ctx: &egui::Context) -> Described;
    /// The fields of its box. The frame takes them for the time it draws and gives them back, so the tool holding its
    /// whole context and the fields being typed into never meet in one borrow.
    fn fields(&mut self) -> &mut Vec<CmdParam>;
    /// Where the numbers of its bar keep their text while they are typed; none for a bar with no numbers.
    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        None
    }
    /// What the frame draws with: the method of the context of its workbench handed on (`self.pc.draw()`).
    fn look(&self) -> DrawCtx<'_>;
    /// An option of the bar or the box was touched.
    fn took(&mut self, _did: Did) {}
    /// What Apply does - the one thing every tool has of its own; why not, in words.
    fn apply(&mut self) -> Result<(), NotApplied>;
    /// Put the tool down, what it made on the way taken away.
    fn cancel(&mut self) {}
    /// Put the tool down, what it made kept: asked by the frame after an Apply that went through, when the head says the
    /// selection comes after it ([`AfterApply::Select`]). A tool that stays in hand is never asked.
    fn put_down(&mut self) {}
    /// Esc with no field holding the keyboard: let go of one thing, or say there is nothing left to let go of - a tool
    /// that picks nothing along the way has nothing, and Esc puts it down.
    fn back(&mut self) -> Stepped {
        Stepped::NothingLeft
    }
    /// A field of its box was typed into, or gave the keyboard up by Esc - the field takes Esc first, as every field
    /// does; a tool that answers either says so here.
    fn field_did(&mut self, _did: FieldDid) {}
    /// Called by the frame every frame, after the bar and the box and before their doors: the values typed taken, the
    /// fields kept in step with the mode, the values, a gizmo dragged, the start of the tool.
    fn after(&mut self) {}
}

/// THE BOX OF A TOOL, as its description gives it: what it stands by, how its fields and options are laid, which fields
/// take no part now, and whether the first field takes the keyboard in this frame. Its ids, drawing, Tab, Enter and Esc
/// are the frame's.
pub struct BoxSpec {
    pub place: Place,
    pub layout: Layout,
    pub hidden: Vec<&'static str>,
    pub focus: Focus,
    /// the fields named as drivers, each with the row of its name under it
    pub names: Vec<FieldName>,
}

impl BoxSpec {
    /// A box standing by `place`, each field in a row of its own, all of them shown, the keyboard left where it is.
    pub fn by(place: Place) -> BoxSpec {
        BoxSpec { place, layout: Layout::default(), hidden: Vec::new(), focus: Focus::Waits, names: Vec::new() }
    }
}

/// THE ONE ENTRY OF THE FRAME for the tool a workbench has in hand, if it has one: the bar in `ui`, the box over the view
/// `canvas` from the same call. It draws them, takes the keys, keeps the fields of the tool in step (`after`), goes
/// through its doors and says its words.
pub fn tool_in_hand(ui: &mut egui::Ui, canvas: egui::Rect, tool: Option<Box<dyn FramedTool + '_>>) {
    let Some(mut tool) = tool else { return };
    let ctx = ui.ctx().clone();
    let canvas = view_or_window(&ctx, canvas);
    let mut described = tool.describe(&ctx);
    let mut fields = std::mem::take(tool.fields());
    let mut texts = tool.bar_texts().map(std::mem::take);
    let moved = numbers_into_the_box(&mut described, texts.as_ref());
    let own = fields.len();
    fields.extend(moved.iter().map(|m| m.field.clone()));
    let look = tool.look();
    // A VALUE THE BOX CANNOT TAKE SHUTS THE BAR TOO: Apply of the bar stood lit over an angle of "abc" refused in the box
    // - only the fields the box shows: a field hidden now takes no part, as the second leg of a fillet takes none
    if let Some(spec) = &described.boxed {
        let shown: Vec<CmdParam> = fields.iter().filter(|p| !spec.hidden.contains(&p.key.as_str())).cloned().collect();
        described.gate = described.gate.with_fields(&shown, look.project);
    }
    let bar = tool_bar(ui, &mut BarLook { scheme: look.scheme, project: look.project, bar_exprs: texts.as_mut(), canvas }, &described);
    let boxed = described.boxed.as_ref().filter(|spec| !fields.is_empty() || spec.layout.sections.iter().any(|s| !s.rows.is_empty())).map(|spec| {
        let id = box_id(&described.head);
        let the_box = FieldsBox {
            id,
            tool: id.value(),
            place: spec.place,
            focus: spec.focus,
            hidden: &spec.hidden,
            ends: described.head.ends,
            esc: EscBy::Bar,
            layout: &spec.layout,
            gate: &described.gate,
            names: &spec.names,
        };
        let frame = ctx.cumulative_frame_nr(); // asked before the data is locked: the context takes the same lock
        ctx.data_mut(|d| d.insert_temp(drawn_box_id(), DrawnBox { frame, id }));
        tool_box(&ctx, &look, canvas, &the_box, &mut fields)
    });
    // THE NUMBERS OF THE BAR GO BACK where the bar kept them: the text as typed, a value taken given to the tool
    let typed = numbers_back(fields.split_off(own), &moved, texts.as_mut(), look.project);
    *tool.fields() = fields;
    if let Some(texts) = texts {
        if let Some(kept) = tool.bar_texts() {
            *kept = texts;
        }
    }
    let mut verdict = bar.verdict;
    for d in bar.did.into_iter().chain(typed) {
        tool.took(d);
    }
    let mut fields_did = Vec::new();
    if let Some(answer) = boxed {
        for d in answer.did {
            tool.took(d);
        }
        if verdict == Verdict::Stay {
            verdict = answer.verdict;
        }
        fields_did = answer.fields;
    }
    for d in fields_did.iter().filter(|d| **d == FieldDid::Typed) {
        tool.field_did(*d);
    }
    tool.after();
    for d in fields_did.into_iter().filter(|d| *d == FieldDid::LeftByEsc) {
        tool.field_did(d);
    }
    go_through(&ctx, &described.gate, verdict, &mut Through { tool: &mut *tool, after: described.head.after });
}

/// A NUMBER OF THE BAR MOVED INTO THE BOX: the option it was, the value the tool holds, and the field it is typed in.
struct Moved {
    key: OptKey,
    was: f64,
    field: CmdParam,
}

/// THE BAR HOLDS WHAT IS SWITCHED AND PRESSED, THE BOX WHAT IS TYPED: a number a tool describes for its bar - the sides of
/// a polygon, the count and the steps of a pattern, the height of a text - is a field of its box at the geometry, the
/// same as the field of a length: a formula with its limits and unit, what it comes to or why it is refused said under
/// it, the gate of the box shut by it. A tool with no box now gets one in the corner of the view for its numbers. The
/// text typed is kept where the bar kept it (`bar_texts`); a value taken goes to the tool as the bar gave it
/// (`Did::Typed`). One place for every tool of every workbench: no tool lays a field in its bar.
fn numbers_into_the_box(described: &mut Described, texts: Option<&std::collections::HashMap<&'static str, String>>) -> Vec<Moved> {
    let mut moved = Vec::new();
    let mut rows = Vec::new();
    for o in &described.bar {
        let Opt::Number { key, label, value, limits } = o else { continue };
        let mut p = CmdParam::new(label, key.name, *value, *limits.range.start(), *limits.range.end());
        p.number = limits.number;
        p.nonzero = limits.zero == Zero::Refused;
        p.txt = texts.and_then(|t| t.get(key.name).cloned()).unwrap_or_else(|| qymcad_core::expr::fmt_num(*value));
        let row = Row::new(qymcad_i18n::tr(label)).field(key.name).said_under();
        rows.push(match limits.unit {
            Some(unit) => row.unit(unit),
            None => row,
        });
        moved.push(Moved { key: *key, was: *value, field: p });
    }
    if moved.is_empty() {
        return moved;
    }
    described.bar.retain(|o| !matches!(o, Opt::Number { .. }));
    let spec = described.boxed.get_or_insert_with(|| BoxSpec::by(Place::close_by(By::ViewCorner)));
    spec.layout.sections.push(Section::plain(rows));
    moved
}

/// The fields the numbers of the bar were typed in, given back: each text kept where the bar keeps it, each value taken
/// that differs from what the tool holds told to it.
fn numbers_back(fields: Vec<CmdParam>, moved: &[Moved], mut texts: Option<&mut std::collections::HashMap<&'static str, String>>, project: &Project) -> Vec<Did> {
    let vars = project.param_map();
    let mut did = Vec::new();
    for m in moved {
        let Some(p) = fields.iter().find(|p| p.key == m.key.name) else { continue };
        if let Some(t) = texts.as_deref_mut() {
            t.insert(m.key.name, p.txt.clone());
        }
        if let Some(v) = field_value(p, &vars).ok().filter(|v| (v - m.was).abs() > 1e-12) {
            did.push(Did::Typed { key: m.key, value: v });
        }
    }
    did
}

/// THE BOX OF THE TOOL IN HAND DRAWN LAST: the frame it was drawn in, and its id.
#[derive(Clone, Copy)]
struct DrawnBox {
    frame: u64,
    id: egui::Id,
}

/// Where the box of the tool in hand drawn last is kept.
fn drawn_box_id() -> egui::Id {
    egui::Id::new("the box of the tool in hand drawn last")
}

/// THE ID OF THE BOX OF THE TOOL IN HAND, drawn in this frame or the one before; none while no tool has one. A check
/// finds the box on the screen by it.
pub fn tool_box_id(ctx: &egui::Context) -> Option<egui::Id> {
    let drawn = ctx.data(|d| d.get_temp::<DrawnBox>(drawn_box_id()))?;
    (ctx.cumulative_frame_nr().saturating_sub(drawn.frame) <= 1).then_some(drawn.id)
}

/// The id of the box of the tool with head `head`: the fields of one tool are told apart from those of another by it.
fn box_id(head: &Head) -> egui::Id {
    egui::Id::new(("the box of a tool", head.icon, head.name.as_str()))
}

/// The id of field `key` in the box of the tool with head `head`: a tool asks it to know which of its fields holds the
/// keyboard.
pub fn tool_field_id(head: &Head, key: &str) -> egui::Id {
    field_id(box_id(head).value(), key)
}

/// THE DOORS OF A FRAMED TOOL, as the frame goes through them.
struct Through<'a, 'b> {
    tool: &'a mut (dyn FramedTool + 'b),
    after: AfterApply,
}

impl Doors for Through<'_, '_> {
    /// THE ONE PLACE A TOOL IS PUT DOWN AFTER ITS APPLY: applied, and its head saying the selection comes after it.
    fn apply(&mut self) -> Result<(), NotApplied> {
        self.tool.apply()?;
        if self.after == AfterApply::Select {
            self.tool.put_down();
        }
        Ok(())
    }

    fn cancel(&mut self) {
        self.tool.cancel();
    }

    fn back(&mut self) -> Stepped {
        self.tool.back()
    }
}
