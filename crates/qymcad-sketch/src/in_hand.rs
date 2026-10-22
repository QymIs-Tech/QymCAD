//! THE TOOL OF A SKETCH IN HAND, as the frame drives it (`qymcad_ui_state::tool_in_hand`): while a sketch is open there
//! is always one - with nothing in hand it is the selection. Its bar, the box of the sizes of a shape and of the angle of
//! a turn, its Enter and its Esc are the frame's; what is the sketch's own is here.

use qymcad_ui_state::{Described, Did, Ends, FramedTool, Gate, Head, NotApplied, Notice, Opt, Stepped};
use super::*;

/// THE TOOL OF THE SKETCH OPEN FOR EDITING; none while no sketch is open.
pub fn in_hand<'a>(sk: &'a mut qymcad_ui_state::SketchCtx<'_>) -> Option<Box<dyn FramedTool + 'a>> {
    qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)?;
    // THE BOX GOES WITH ITS TOOL: put down, the tool takes its box with it
    if !sk.place.stands_with(sk.armed) {
        sk.place.clear();
    }
    // AN EDITOR OPEN AT THE GEOMETRY is the tool in hand: a dimension, a circle, the corners of a set, an offset
    if let Some(e) = editor_open(sk) {
        return Some(editor_tool(sk, e));
    }
    Some(Box::new(InSketch { sk, boxed: Boxed::Nothing, seen: Vec::new(), ctx: None, gate: Gate::Open }))
}

/// WHAT HOLDS THE HAND OF A SKETCH beyond its tool: FILLET ALL, waiting for the shape to round; a copy, a cut or an insert,
/// waiting for its point; or the tool itself.
enum Now {
    FilletAll,
    Waiting(Wait),
    Tool,
}

/// A COPY, A CUT OR AN INSERT WAITING: its name and what it asks for, by their catalogue keys.
struct Wait {
    name: &'static str,
    ask: &'static str,
}

/// WHAT THE BOX OF THE SKETCH HOLDS NOW: the sizes of a shape, the angle of a turn, or nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Boxed {
    Shape,
    Turn,
    Nothing,
}

/// THE TOOL OF THE SKETCH IN HAND: the context it works over, what its box holds, the values of its fields as the frame
/// began, the context of the frame - where the pointer is when Enter is pressed - and the gate its box leaves.
struct InSketch<'a, 'b> {
    sk: &'a mut qymcad_ui_state::SketchCtx<'b>,
    boxed: Boxed,
    seen: Vec<f64>,
    ctx: Option<egui::Context>,
    gate: Gate,
}

/// What holds the hand of the sketch.
fn now(sk: &qymcad_ui_state::SketchCtx) -> Now {
    if sk.picking.fillet_all() {
        return Now::FilletAll;
    }
    // EDIT -> COPY, CUT, INSERT hold the hand too: the base point or the place is awaited. Their words stood only in the
    // status line, and the bar said "Select" over a click that would copy.
    if let Some((_, cut)) = sk.clip.geom_pending.as_ref() {
        return Now::Waiting(if *cut { Wait { name: "menu-cut", ask: "g-cut-base-point" } } else { Wait { name: "tool-copy", ask: "g-copy-base-point" } });
    }
    if sk.clip.geom_place {
        return Now::Waiting(Wait { name: "win-insert", ask: "g-insert-click" });
    }
    Now::Tool
}

impl InSketch<'_, '_> {
    /// The box of the sketch now, and its fields made as it opens: the sizes of a shape drawn or reopened, or the angle of a
    /// turn about the centre picked.
    fn box_now(&mut self) -> Option<qymcad_ui_state::BoxSpec> {
        let sk = &mut *self.sk;
        let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return None };
        let sheet = qymcad_ui_state::Sheet { view: *sk.view, rect: sk.canvas };
        if let Some(now) = shape_now(sk.project, sk.place, si) {
            if sk.place.fields.is_empty() {
                sk.place.fields = now.fields.clone();
            }
            let around: Vec<Pos2> = now.around.iter().map(|p| sheet.at(*p)).collect();
            let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(sheet.at(now.at), &around)));
            spec.focus = qymcad_ui_state::Focus::take(&mut sk.place.focus);
            let shape = shape_box(sk.project, sk.place, &now);
            spec.layout = shape.layout;
            spec.hidden = shape.hidden;
            spec.names = shape.names;
            self.gate = shape.gate;
            self.boxed = Boxed::Shape;
            self.seen = sk.place.fields.iter().map(|p| p.val).collect();
            return Some(spec);
        }
        if sk.place.active() {
            sk.place.clear(); // the shape went from under the box
        }
        let base = sk.tool.move_base.filter(|_| sk.armed.moving() == Some(qymcad_ui_state::MoveKind::Rotate))?;
        turn_of(sk, si, base)?;
        if sk.rot.field.is_empty() {
            sk.rot.field = vec![turn_field()];
        }
        let at = sheet.at(base);
        let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(at, &[at])));
        spec.focus = qymcad_ui_state::Focus::take(&mut sk.rot.focus);
        let row = qymcad_ui_state::Row::new(format!("{} {}", ph::ARROWS_CLOCKWISE, qymcad_i18n::tr("sk-angle-deg"))).field("angle").said_under();
        spec.layout = qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(vec![row])], foot: Vec::new() };
        self.boxed = Boxed::Turn;
        Some(spec)
    }
}

impl FramedTool for InSketch<'_, '_> {
    fn describe(&mut self, ctx: &egui::Context) -> Described {
        self.ctx = Some(ctx.clone());
        self.boxed = Boxed::Nothing;
        self.gate = Gate::Open;
        let sk = &mut *self.sk;
        let head = |name: String, ends| Head { icon: ph::PENCIL_SIMPLE, name, ends };
        match now(sk) {
            // FILLET ALL holds the hand: its name, what to do, the way out; the radius is typed in the field at the shape
            Now::FilletAll => {
                let ask = qymcad_i18n::tr(if sk.corner.at.is_some() { "sk-enter-apply-expr" } else { "g-fillet-all-hint" });
                // the shape is clicked, not typed: Enter before it has nothing to round
                let gate = if sk.corner.at.is_some() { Gate::Open } else { Gate::Waiting(qymcad_i18n::tr("g-fillet-all-hint")) };
                Described { head: head(qymcad_i18n::tr("tool-fillet"), Ends::Close), bar: vec![Opt::Readout(ask)], gate, says: Vec::new(), boxed: None }
            }
            // a point is clicked, not typed: Enter before it has nothing to do
            Now::Waiting(w) => Described {
                head: head(qymcad_i18n::tr(w.name), Ends::Close),
                bar: vec![Opt::Readout(qymcad_i18n::tr(w.ask))],
                gate: Gate::Waiting(qymcad_i18n::tr(w.ask)),
                says: Vec::new(),
                boxed: None,
            },
            Now::Tool => {
                // another tool taken puts the copy down: one thing in hand at a time
                if !matches!(*sk.armed, qymcad_ui_state::Armed::None) {
                    sk.clip.drop_geom();
                }
                let bar = tool_opts(sk);
                let name = tool_name(sk.armed);
                let pattern = sk.armed.pattern().is_some();
                let empty = matches!(*sk.armed, qymcad_ui_state::Armed::None);
                let boxed = self.box_now();
                let ends = match self.boxed {
                    Boxed::Turn => Ends::ApplyCancel,
                    // THE BOX OF A SHAPE ENDS AS EVERY BOX DOES: Apply lays what was typed, Cancel - as Esc - leaves the shape as
                    // it stands without it
                    Boxed::Shape => Ends::ApplyCancel,
                    Boxed::Nothing if pattern => Ends::ApplyCancel,
                    Boxed::Nothing if empty => Ends::Nothing,
                    Boxed::Nothing => Ends::Close,
                };
                Described { head: head(name, ends), bar, gate: self.gate.clone(), says: Vec::new(), boxed }
            }
        }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        match self.boxed {
            Boxed::Turn => &mut self.sk.rot.field,
            Boxed::Shape | Boxed::Nothing => &mut self.sk.place.fields,
        }
    }

    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        Some(&mut *self.sk.bar_exprs)
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    /// An option of a row of a size of the shape in the box - kept until Enter lays it - or of the bar.
    fn took(&mut self, did: Did) {
        if let (Boxed::Shape, qymcad_ui_state::Sel::Sketch(si)) = (self.boxed, *self.sk.sel) {
            if shape_took(self.sk, si, &did) {
                return;
            }
        }
        opt_took(self.sk, did);
    }

    /// Enter: the sizes of the shape in the box laid, the turn made, the pattern made, the extension Extend previews made
    /// as a click makes it, the offset made on the side of the pointer; anything else has nothing to make.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let sk = &mut *self.sk;
        let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return Ok(()) };
        match self.boxed {
            Boxed::Shape => {
                shape_apply(sk, si);
                return Ok(());
            }
            Boxed::Turn => {
                let turn = sk.tool.move_base.and_then(|base| turn_of(sk, si, base));
                let angle = sk.rot.field.first().map(|p| p.val);
                if let Some(turn) = turn {
                    if let Some(v) = angle {
                        turn_by(sk, &turn, v);
                    }
                }
                *sk.armed = qymcad_ui_state::Armed::None;
                sk.tool.move_base = None;
                return Ok(());
            }
            Boxed::Nothing => {}
        }
        let pointer = self.ctx.as_ref().and_then(|c| c.input(|i| i.pointer.hover_pos())).filter(|p| sk.canvas.contains(*p));
        if sk.armed.pattern().is_some() {
            confirm_pattern(sk, si);
        } else if sk.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Extend) {
            // the extension Extend previews, made as a click makes it
            let Some(held) = sk.tool.extend else { return Ok(()) };
            let Some(pos) = pointer else { return Ok(()) };
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-extend"));
            extend_apply(sk, ExtendAt { rect: sk.canvas, pos, si, curve: None }, held);
            qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        } else if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset) && sk.sel_sk.offset.at.is_none() && !offset_curves(sk.sel_sk).is_empty() {
            // THE OFFSET MADE BY ENTER before a click has fixed the side: on the side of the pointer, by the distance of the bar
            if qymcad_ui_state::bar_field_bad("sk_offset") {
                return Err(NotApplied::Refused(qymcad_i18n::tr("sk-offset-field-bad")));
            }
            let at = pointer.map(|p| qymcad_ui_state::to_world(&*sk.view, sk.canvas, p));
            let dist = sk.tool_prefs.offset;
            offset_make(
                OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
                si,
                dist,
                at,
            );
        }
        Ok(())
    }

    /// The button that puts the tool down: the box of a shape or of a turn goes, FILLET ALL and a copy waiting are let
    /// go, a pattern is put down with its centre, any other tool is released as Esc releases it.
    fn cancel(&mut self) {
        let sk = &mut *self.sk;
        match now(sk) {
            Now::FilletAll => {
                sk.picking.clear();
                sk.corner.clear();
                *sk.status = Notice::info(qymcad_i18n::tr("in-fillet-all-cancelled"));
            }
            Now::Waiting(w) => {
                sk.clip.drop_geom();
                *sk.status = Notice::info(qymcad_i18n::tr(if w.name == "win-insert" { "in-insert-cancelled" } else { "in-copy-cancelled" }));
            }
            Now::Tool => match self.boxed {
                Boxed::Shape => {
                    self.back();
                }
                Boxed::Turn => {
                    *sk.armed = qymcad_ui_state::Armed::None;
                    sk.tool.move_base = None;
                }
                Boxed::Nothing => {
                    if sk.armed.pattern().is_some() {
                        *sk.armed = qymcad_ui_state::Armed::None;
                        sk.pat.edit = None;
                        sk.pat.center = None;
                        *sk.status = Notice::info(qymcad_i18n::tr("in-array-cancelled"));
                    } else {
                        let mut said = tools_said(sk);
                        if let Some(msg) = qymcad_ui_state::release_armed_sketch_tool(&mut said.tools) {
                            *said.status = Notice::info(qymcad_i18n::tr(msg));
                        }
                    }
                }
            },
        }
    }

    /// ONE STEP OF THE Esc OF A SKETCH (`escape_step`). Esc in the box of the sizes of a shape is a step of its own: the
    /// shape stays without the sizes typed, and the drawing tool stays in hand - the next Esc puts it down.
    fn back(&mut self) -> Stepped {
        if self.boxed == Boxed::Shape {
            self.sk.place.clear();
            return Stepped::Back;
        }
        escape_step(self.sk);
        Stepped::Back
    }

    /// ONE Esc ENDS THE BOX OF A SHAPE, its field holding the keyboard or not: the shape stays as it stands, the sizes
    /// typed are not laid, the tool stays in hand. A value typed into the box of a shape is a size meant: Enter lays it.
    fn field_did(&mut self, did: qymcad_ui_state::FieldDid) {
        match did {
            qymcad_ui_state::FieldDid::Typed if self.boxed == Boxed::Shape => self.sk.place.typed = true,
            qymcad_ui_state::FieldDid::LeftByEsc if self.boxed == Boxed::Shape => {
                self.back();
            }
            _ => {}
        }
    }

    /// A value typed into the box of a shape is taken at once by a shape that follows its fields; the angle typed turns
    /// the live preview.
    fn after(&mut self) {
        let sk = &mut *self.sk;
        match self.boxed {
            Boxed::Shape => {
                let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
                if sk.place.fields.iter().map(|p| p.val).ne(self.seen.iter().copied()) {
                    follow_typed(sk, si);
                }
            }
            Boxed::Turn => sk.rot.angle = sk.rot.field.first().map_or(0.0, |p| p.val),
            Boxed::Nothing => {}
        }
    }
}

/// APPLY OR UPDATE THE PATTERN of the sketch: one step of undo, the tool put down after it.
fn confirm_pattern(sk: &mut qymcad_ui_state::SketchCtx, si: usize) {
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
    if let Some(pi) = sk.pat.edit {
        let src = sk.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &src);
        sk.project.update_pattern(si, pi, kind);
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-updated"));
    } else {
        let eids = qymcad_ui_state::shapes_chosen(&*sk.project, si, &*sk.sel_sk);
        if eids.is_empty() {
            *sk.status = Notice::info(qymcad_i18n::tr("g-select-for-array"));
            return;
        }
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &eids);
        sk.project.add_pattern(si, &eids, kind);
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-created"));
    }
    *sk.armed = qymcad_ui_state::Armed::None;
    sk.pat.edit = None;
    sk.pat.center = None;
    qymcad_ui_state::invalidate(&mut *sk.regen);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
}

/// THE TOOLS OF A SKETCH AND ITS STATUS LINE, borrowed apart: what Esc puts down, and where it says so.
pub(crate) struct ToolsSaid<'s> {
    pub(crate) tools: qymcad_ui_state::Tools<'s>,
    pub(crate) status: &'s mut Notice,
}

/// The tools of sketch context `sk` and its status line.
pub(crate) fn tools_said<'s>(sk: &'s mut qymcad_ui_state::SketchCtx) -> ToolsSaid<'s> {
    ToolsSaid {
        tools: qymcad_ui_state::Tools {
            armed: &mut *sk.armed,
            annot: &mut *sk.annot,
            cmd: &mut *sk.cmd,
            corner: &mut *sk.corner,
            dim: &mut *sk.dim,
            drag: &mut *sk.drag,
            gsel: &mut *sk.gsel,
            inline: &mut *sk.inline,
            measure: &mut *sk.measure,
            pat: &mut *sk.pat,
            pending_import: &mut *sk.pending_import,
            picking: &mut *sk.picking,
            place: &mut *sk.place,
            sel_sk: &mut *sk.sel_sk,
            tool: &mut *sk.tool,
        },
        status: &mut *sk.status,
    }
}
