//! THE TOOL OF A SKETCH IN HAND, as the frame drives it (`qymcad_ui_state::tool_in_hand`): while a sketch is open there
//! is always one - with nothing in hand it is the selection. Its bar, the box of the sizes of a shape and of the angle of
//! a turn, its Enter and its Esc are the frame's; what is the sketch's own is here.

use qymcad_ui_state::{AfterApply, Described, Did, Ends, FramedTool, Gate, Head, NotApplied, Notice, Opt, Stepped};
use super::*;

/// THE TOOL OF THE SKETCH OPEN FOR EDITING; none while no sketch is open.
pub fn in_hand<'a>(sk: &'a mut qymcad_ui_state::SketchCtx<'_>) -> Option<Box<dyn FramedTool + 'a>> {
    qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)?;
    // THE BOX GOES WITH ITS TOOL: put down, the tool takes its box with it
    if !sk.place.stands_with(sk.armed) {
        sk.place.clear();
    }
    // A BOX GONE WITHOUT Enter OR Esc - another tool taken, the sketch left - takes what it tried back with it
    if trying(sk) && sk.inline.dim().is_none() && !sk.place.active() {
        trial_dropped(sk);
    }
    // AN EDITOR OPEN AT THE GEOMETRY is the tool in hand: a dimension, a circle, the corners of a set, an offset
    if let Some(e) = editor_open(sk) {
        return Some(editor_tool(sk, e));
    }
    Some(Box::new(InSketch { sk, boxed: Boxed::Nothing, seen: Vec::new(), ctx: None, gate: Gate::Open, asks: Vec::new(), free: Vec::new() }))
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
    /// a label of the sketch edited: its string, font and height
    Label,
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
    /// what the frame kept of the names and references of the rows of the box of a shape
    asks: Vec<qymcad_ui_state::FieldAsk>,
    /// the values of the box no dimension holds: their drivers are parameters of the project
    free: Vec<&'static str>,
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
            let shape = shape_box(sk.place, &now);
            spec.layout = shape.layout;
            spec.names = shape.names;
            self.free = shape.free;
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
        let row = qymcad_ui_state::Row::new(qymcad_i18n::tr("sk-angle-deg")).field("angle");
        spec.layout = qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(vec![row])], foot: Vec::new() };
        spec.names = vec![qymcad_ui_state::FieldName::of("angle", String::new(), None)];
        self.free = vec!["angle"];
        self.boxed = Boxed::Turn;
        Some(spec)
    }
}

/// THE BOX OF A PATTERN, as the pattern of a part is: a linear one by its directions - the copies and the steps of each
/// under its title, the second folded until it is opened, which switches it on; a circular one a value a row. The numbers
/// come from the bar (`numbers_into_the_box`).
fn pattern_box(armed: &qymcad_ui_state::Armed, pat: &qymcad_ui_state::SketchPattern) -> Option<qymcad_ui_state::BoxSpec> {
    use qymcad_ui_state::{AsLit, OptKey, Row, Section};
    let kind = armed.pattern()?;
    let rows = |keys: &[(&str, &str)]| keys.iter().map(|(c, k)| Row::new(qymcad_i18n::tr(c)).field(k)).collect::<Vec<Row>>();
    let direction = |n: usize| qymcad_i18n::tr1("box-direction-n", "n", &n.to_string());
    let sections = if kind == qymcad_ui_state::PatternKind::Linear {
        vec![
            Section::titled(direction(1), qymcad_ui_state::Folded::No, rows(&[("cmd-copies", "skpat_count"), ("opt-step-x", "skpat_dx"), ("opt-step-y", "skpat_dy")])),
            Section::switched(direction(2), OptKey::of(SECOND_DIRECTION), (pat.count2 > 1).lit(), rows(&[("cmd-copies", "skpat_count2"), ("opt-step-x", "skpat_dx2"), ("opt-step-y", "skpat_dy2")])),
        ]
    } else {
        vec![Section::plain(rows(&[("cmd-copies", "skpat_count"), ("cmd-angle", "skpat_angle")]))]
    };
    let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::close_by(qymcad_ui_state::By::ViewCorner));
    spec.layout.sections = sections;
    Some(spec)
}

/// THE BOX OF THE TEXT TOOL: the string, the font it is written in, its height - and, for a text placed anew, whether it
/// is a note. A label double-clicked is edited in the same box at the label, its string taking the keyboard; nothing of
/// it stands in the bar.
fn text_box(sk: &mut qymcad_ui_state::SketchCtx) -> Option<qymcad_ui_state::BoxSpec> {
    use qymcad_ui_state::{AsLit, Opt, OptKey, Row, Section};
    if sk.armed.drawing() != Some(qymcad_ui_state::SketchShape::Text) {
        return None;
    }
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return None };
    let editing = sk.inline.text();
    let focus = qymcad_ui_state::Focus::take(&mut sk.annot.text_focus);
    let prefs = &*sk.tool_prefs;
    let text = Opt::Text { key: OptKey::of("text"), text: prefs.text.clone(), placeholder: String::new(), unfit: None, hint: None, focus };
    // WHICH FONT IT IS WRITTEN IN, named on the button itself: "Font..." alone said nothing
    let font = Opt::press(OptKey::of("font"), qymcad_ui_state::font_label(&prefs.font, "opt-font")).hint(qymcad_i18n::tr("opt-pick-font"));
    let mut rows = vec![Row::new(qymcad_i18n::tr("tool-text")).opt(text), Row::new(qymcad_i18n::tr("box-font")).opt(font), Row::new(qymcad_i18n::tr("sk-height")).field("text_h")];
    if editing.is_none() {
        rows.push(Row::new("").opt(Opt::switch(OptKey::of("note"), qymcad_i18n::tr("opt-note"), prefs.text_note.lit()).hint(qymcad_i18n::tr("opt-text-as-note"))));
    }
    // a label edited has its box at the label; a text to place, at the corner of the view
    let place = match editing.and_then(|ti| sk.project.sketch_text_bbox(si, ti)) {
        Some((bx, _, _, maxy)) => {
            let at = qymcad_ui_state::Sheet { view: *sk.view, rect: sk.canvas }.at(Point2::new(bx, maxy));
            qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(at, &[at]))
        }
        None => qymcad_ui_state::Place::close_by(qymcad_ui_state::By::ViewCorner),
    };
    let mut spec = qymcad_ui_state::BoxSpec::by(place);
    spec.layout.sections = vec![Section::plain(rows)];
    Some(spec)
}

/// The key of the title of the second direction of a linear pattern of a sketch: opened, the direction is on.
pub(crate) const SECOND_DIRECTION: &str = "skpat_dir2";

impl FramedTool for InSketch<'_, '_> {
    fn describe(&mut self, ctx: &egui::Context) -> Described {
        self.ctx = Some(ctx.clone());
        self.boxed = Boxed::Nothing;
        self.gate = Gate::Open;
        self.free.clear();
        let sk = &mut *self.sk;
        let head = |name: String, ends| Head { icon: ph::PENCIL_SIMPLE, name, ends, after: AfterApply::Stays };
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
                // a text, a move and a copy do one thing too, finished by a click (`finished_by_click`)
                let by_click = sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text) || sk.armed.moving().is_some();
                let empty = matches!(*sk.armed, qymcad_ui_state::Armed::None);
                let pattern_laid = pattern_box(sk.armed, sk.sk_pat);
                let text_laid = text_box(sk);
                if text_laid.is_some() && sk.inline.text().is_some() {
                    self.boxed = Boxed::Label;
                }
                let boxed = self.box_now().or(pattern_laid).or(text_laid);
                let ends = match self.boxed {
                    Boxed::Turn => Ends::ApplyCancel,
                    // THE BOX OF A SHAPE ENDS AS EVERY BOX DOES: Apply lays what was typed, Cancel - as Esc - leaves the shape as
                    // it stands without it
                    Boxed::Shape | Boxed::Label => Ends::ApplyCancel,
                    Boxed::Nothing if pattern => Ends::ApplyCancel,
                    Boxed::Nothing if empty => Ends::Nothing,
                    Boxed::Nothing => Ends::Close,
                };
                // THE TURN AND THE PATTERN DO ONE THING: made, the selection is in hand; a shape, a dimension, the corner and
                // the offset stay for the next
                let after = if self.boxed == Boxed::Turn || (self.boxed == Boxed::Nothing && (pattern || by_click)) { AfterApply::Select } else { AfterApply::Stays };
                let mut d = Described { head: Head { after, ..head(name, ends) }, bar, gate: self.gate.clone(), says: Vec::new(), boxed };
                // THE NUMBERS OF THE BAR ARE NAMED AS EVERY VALUE OF A BOX IS - the sides of a polygon, the height of a
                // text, the counts and steps of a pattern; no dimension holds them, their drivers are parameters
                self.free.extend(qymcad_ui_state::name_bar_numbers(&mut d));
                d
            }
        }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        match self.boxed {
            Boxed::Turn => &mut self.sk.rot.field,
            Boxed::Shape | Boxed::Label | Boxed::Nothing => &mut self.sk.place.fields,
        }
    }

    fn bar_texts(&mut self) -> Option<&mut std::collections::HashMap<&'static str, String>> {
        Some(&mut *self.sk.bar_exprs)
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    /// A refused Enter is said on the status line of the sketch too, as the boxes of a sketch said it on main.
    fn refused(&mut self, words: &Notice) {
        *self.sk.status = words.clone();
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
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
            // ENTER IN THE BOX OF A SHAPE: its sizes laid - once more over the document as the box found it, for a shape
            // laid anew - and kept as one step with what was tried on it; the box goes, the tool stays
            Boxed::Shape => {
                let named = |sk: &mut qymcad_ui_state::SketchCtx| {
                    shape_lay(sk, si, &self.asks);
                    qymcad_ui_state::name_free_values(sk.project, &self.asks, &self.free);
                };
                if follows(&sk.place.shape) == Follows::LaidAnew {
                    try_on(sk, From::Snapshot, named);
                } else {
                    named(sk);
                }
                trial_kept(sk, Kept::OwnStep);
                sk.place.clear();
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
                name_in_own_step(sk, &self.asks, &self.free);
                return Ok(());
            }
            // ENTER IN THE BOX OF A LABEL: the label takes its string and height, re-baked in its own font, as one step with
            // the drivers typed; the editing ends with the tool
            Boxed::Label => {
                qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-text"));
                qymcad_ui_state::text_edit_apply(sk, si);
                qymcad_ui_state::name_free_values(sk.project, &self.asks, &self.free);
                qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
                qymcad_ui_state::end_label_edit(sk);
                return Ok(());
            }
            Boxed::Nothing => {}
        }
        let pointer = self.ctx.as_ref().and_then(|c| c.input(|i| i.pointer.hover_pos())).filter(|p| sk.canvas.contains(*p));
        if sk.armed.pattern().is_some() {
            return confirm_pattern(sk, si, &Named { asks: &self.asks, free: &self.free });
        }
        name_in_own_step(sk, &self.asks, &self.free);
        if sk.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Extend) {
            // the extension Extend previews, made as a click makes it
            let Some(held) = sk.tool.extend else { return Ok(()) };
            let Some(pos) = pointer else { return Ok(()) };
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-extend"));
            extend_apply(sk, ExtendAt { rect: sk.canvas, pos, si, curve: None }, held);
            qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        } else if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset) && sk.sel_sk.offset.at.is_none() && !offset_curves(sk.sel_sk).is_empty() {
            // THE OFFSET MADE BY ENTER before a click has fixed the side: on the side of the pointer, by the distance of its
            // box - a distance refused there shuts the gate before it comes here
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
                Boxed::Turn => self.put_down(),
                Boxed::Label => qymcad_ui_state::end_label_edit(sk),
                Boxed::Nothing => {
                    if sk.armed.pattern().is_some() {
                        self.put_down();
                        *self.sk.status = Notice::info(qymcad_i18n::tr("in-array-cancelled"));
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

    /// The turn made, the pattern made: every tool of the sketch put down, the arrow in hand.
    /// The tools of the sketch put down, the arrow in hand - what the tool said of its work kept on the status line: the
    /// arrow taken says how much is selected, and over "Rotated by 90 deg" or a letter refused that told nothing.
    fn put_down(&mut self) {
        let said = self.sk.status.clone();
        qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(self.sk));
        *self.sk.status = said;
    }

    /// A text placed, a move or a copy made by its target click: said once, and the frame puts the tool down.
    fn finished_by_click(&mut self) -> bool {
        std::mem::take(&mut self.sk.tool.finished) == qymcad_ui_state::Finished::ByClick
    }

    /// ONE STEP OF THE Esc OF A SKETCH (`escape_step`). The box of the sizes of a shape is a step of its own: it goes, the
    /// tool that drew the shape stays for the next, and the next Esc puts the tool down.
    fn back(&mut self) -> Stepped {
        if self.boxed == Boxed::Shape {
            trial_dropped(self.sk);
            self.sk.place.clear();
            return Stepped::Back;
        }
        escape_step(self.sk);
        Stepped::Back
    }

    /// ONE Esc ENDS THE BOX OF A SHAPE, its field holding the keyboard or not: the shape stays as it stands, the sizes
    /// typed are not laid, and the tool that drew it stays in hand. A value typed into the box of a shape is a size
    /// meant: Enter lays it.
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
            // A GOOD VALUE TYPED IS TRIED ON THE SHAPE at once: a shape drawn follows its fields step by step, one whose
            // sizes are dimensions has them laid anew over the document as the box found it (`try_on`)
            Boxed::Shape => {
                let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
                if sk.place.fields.iter().map(|p| p.val).ne(self.seen.iter().copied()) {
                    match follows(&sk.place.shape) {
                        Follows::StepByStep => try_on(sk, From::AsItStands, |sk| follow_typed(sk, si)),
                        Follows::LaidAnew => try_on(sk, From::Snapshot, |sk| shape_lay(sk, si, &self.asks)),
                    };
                }
            }
            Boxed::Turn => sk.rot.angle = sk.rot.field.first().map_or(0.0, |p| p.val),
            Boxed::Label | Boxed::Nothing => {}
        }
    }
}

/// APPLY OR UPDATE THE PATTERN of the sketch: one step of undo; nothing chosen to repeat, it is refused in words and the
/// tool stays in hand.
fn confirm_pattern(sk: &mut qymcad_ui_state::SketchCtx, si: usize, named: &Named) -> Result<(), NotApplied> {
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
    if let Some(pi) = sk.pat.edit {
        let src = sk.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &src);
        sk.project.update_pattern(si, pi, kind);
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-updated"));
    } else {
        let eids = qymcad_ui_state::shapes_chosen(&*sk.project, si, &*sk.sel_sk);
        if eids.is_empty() {
            qymcad_ui_state::abort_edit(&mut sk.rebuild());
            return Err(NotApplied::Refused(qymcad_i18n::tr("g-select-for-array")));
        }
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &eids);
        sk.project.add_pattern(si, &eids, kind);
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-created"));
    }
    qymcad_ui_state::name_free_values(sk.project, named.asks, named.free);
    qymcad_ui_state::invalidate(&mut *sk.regen);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    Ok(())
}

/// THE DRIVERS TYPED IN A BOX for the values no dimension holds: what was asked, and which values those are.
struct Named<'a> {
    asks: &'a [qymcad_ui_state::FieldAsk],
    free: &'a [&'static str],
}

/// THE DRIVERS OF THE VALUES NO DIMENSION HOLDS, given as one step of undo of their own: a turn made, a number of the bar
/// named with nothing laid.
fn name_in_own_step(sk: &mut qymcad_ui_state::SketchCtx, asks: &[qymcad_ui_state::FieldAsk], free: &[&'static str]) {
    if !asks.iter().any(|a| free.contains(&a.field.as_str()) && !a.name.trim().is_empty()) {
        return;
    }
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-driver-step"));
    qymcad_ui_state::name_free_values(sk.project, asks, free);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
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
