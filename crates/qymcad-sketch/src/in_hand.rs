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
    Some(Box::new(InSketch { sk, boxed: Boxed::Nothing, seen: Vec::new(), ctx: None, gate: Gate::Open, asks: Vec::new(), keeping: Keeping::NONE }))
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
    /// a note of the sketch edited: its words
    Note,
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
    /// the values of the box an item keeps, and the item
    keeping: Keeping,
}

/// THE VALUES OF THE BOX AN ITEM KEEPS - the sides and the turn of a polygon, the counts of a pattern, the height of a
/// label - and the item, where it stands already.
#[derive(Clone, Copy)]
struct Keeping {
    kept: &'static [KeptBy],
    item: Option<Id>,
}

impl Keeping {
    /// No value an item keeps.
    const NONE: Keeping = Keeping { kept: &[], item: None };
}

/// A VALUE OF THE BOX OF THE SKETCH AN ITEM KEEPS: the field of the box, and the key the item keeps it under.
pub(crate) struct KeptBy {
    field: &'static str,
    key: &'static str,
}

/// The values of a polygon its centre keeps.
const POLYGON_KEPT: [KeptBy; 2] = [KeptBy { field: "a", key: "turn" }, KeptBy { field: "poly_n", key: "sides" }];
/// The values of an ellipse its centre keeps.
const ELLIPSE_KEPT: [KeptBy; 3] = [KeptBy { field: "w", key: "w" }, KeptBy { field: "h", key: "h" }, KeptBy { field: "turn", key: "turn" }];
/// The values of a pattern of a sketch it keeps.
const PATTERN_KEPT: [KeptBy; 7] = [
    KeptBy { field: "skpat_count", key: "count" },
    KeptBy { field: "skpat_dx", key: "dx" },
    KeptBy { field: "skpat_dy", key: "dy" },
    KeptBy { field: "skpat_count2", key: "count2" },
    KeptBy { field: "skpat_dx2", key: "dx2" },
    KeptBy { field: "skpat_dy2", key: "dy2" },
    KeptBy { field: "skpat_angle", key: "angle" },
];
/// The value of a label it keeps.
const TEXT_KEPT: [KeptBy; 1] = [KeptBy { field: "text_h", key: "height" }];

/// The values of item `node` a box set, as the frame helper takes them.
fn item_values(kept: &[KeptBy], node: Id) -> Vec<qymcad_ui_state::ItemValue<'_>> {
    kept.iter().map(|k| qymcad_ui_state::ItemValue { field: k.field, node, key: k.key }).collect()
}

/// What holds the hand of the sketch.
fn now(sk: &qymcad_ui_state::SketchCtx) -> Now {
    if sk.picking.fillet_all() {
        return Now::FilletAll;
    }
    // EDIT -> COPY, CUT, INSERT hold the hand too: the base point or the place is awaited. Their words stood only in the
    // status line, and the bar said "Select" over a click that would copy. Another tool taken over them is what is in
    // hand, and puts them down.
    if !matches!(*sk.armed, qymcad_ui_state::Armed::None) {
        return Now::Tool;
    }
    if let Some((_, cut)) = sk.clip.geom_pending.as_ref() {
        return Now::Waiting(if *cut { Wait { name: "menu-cut", ask: "g-cut-base-point" } } else { Wait { name: "tool-copy", ask: "g-copy-base-point" } });
    }
    if sk.clip.geom_place {
        return Now::Waiting(Wait { name: "win-insert", ask: "g-insert-click" });
    }
    Now::Tool
}

impl InSketch<'_, '_> {
    /// THE VALUES OF THE BOX AN ITEM KEEPS, NAMED as every value of a box is, through the item: the sides and the turn of
    /// a polygon and the axes of an ellipse in the box of the shape, the counts and the steps of a pattern, the height of
    /// a text - a reference where the item holds the value with no formula. A text placed anew and a pattern made anew
    /// are named once they stand. The distance of an offset names the dimension it lays.
    fn name_kept_values(&mut self, d: &mut Described) {
        let sk = &*self.sk;
        if self.keeping.kept.is_empty() {
            if sk.armed.pattern().is_some() {
                let pattern = sk.pat.edit.and_then(|pi| qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).and_then(|si| sk.project.sketches[si].patterns.get(pi)).map(|p| p.id));
                self.keeping = Keeping { kept: &PATTERN_KEPT, item: pattern };
            } else if sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text) {
                let label = sk.inline.text().and_then(|ti| qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).and_then(|si| sk.project.sketches[si].texts.get(ti)).map(|t| t.id));
                self.keeping = Keeping { kept: &TEXT_KEPT, item: label };
            }
        }
        let Some(spec) = d.boxed.as_mut() else { return };
        let numbers: Vec<&str> = d.bar.iter().filter_map(|o| if let Opt::Number { key, .. } = o { Some(key.name) } else { None }).collect();
        let held = |field: &str| sk.place.fields.iter().any(|p| p.key == field) || numbers.contains(&field);
        for k in self.keeping.kept.iter().filter(|k| held(k.field)) {
            if !spec.names.iter().any(|n| n.field == k.field) {
                spec.names.push(qymcad_ui_state::item_field_name(&*sk.project, k.field, self.keeping.item, k.key));
            }
        }
        if numbers.contains(&"sk_offset") {
            spec.names.push(qymcad_ui_state::FieldName::of("sk_offset", String::new(), None));
        }
    }

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
            self.keeping = match sk.place.shape {
                qymcad_ui_state::PlacingShape::Poly(c) => Keeping { kept: &POLYGON_KEPT, item: Some(c) },
                qymcad_ui_state::PlacingShape::Ellipse(c, _) => Keeping { kept: &ELLIPSE_KEPT, item: Some(c) },
                _ => Keeping::NONE,
            };
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
        // a turn is made once: no item keeps its angle, and it has no driver to name
        self.boxed = Boxed::Turn;
        Some(spec)
    }
}

/// THE BOX OF A PATTERN, by the description of every pattern (`pattern_directions`): a linear one by its directions - the
/// copies and the steps along X and Y of each, the second folded until its title switches it on; a circular one a value a
/// row. The numbers come from the bar (`numbers_into_the_box`).
fn pattern_box(armed: &qymcad_ui_state::Armed, pat: &qymcad_ui_state::SketchPattern) -> Option<qymcad_ui_state::BoxSpec> {
    use qymcad_ui_state::{AsLit, DirectionSwitch, OptKey, PatternDirection, Row, Section};
    let kind = armed.pattern()?;
    let rows = |of: &[PatternRow]| of.iter().map(|r| Row::new(qymcad_i18n::tr(r.caption)).field(r.field)).collect::<Vec<Row>>();
    let sections = if kind == qymcad_ui_state::PatternKind::Linear {
        qymcad_ui_state::pattern_directions(vec![
            PatternDirection { rows: rows(&FIRST_DIRECTION), switch: None },
            PatternDirection { rows: rows(&SECOND_DIRECTION_ROWS), switch: Some(DirectionSwitch { key: OptKey::of(SECOND_DIRECTION), on: (pat.count2 > 1).lit() }) },
        ])
    } else {
        vec![Section::plain(rows(&AROUND))]
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

/// ONE ROW OF THE BOX OF A PATTERN: its caption by its catalogue key, and the key of its field.
struct PatternRow {
    caption: &'static str,
    field: &'static str,
}

/// The rows of the first direction of a linear pattern.
const FIRST_DIRECTION: [PatternRow; 3] = [
    PatternRow { caption: "cmd-copies", field: "skpat_count" },
    PatternRow { caption: "opt-step-x", field: "skpat_dx" },
    PatternRow { caption: "opt-step-y", field: "skpat_dy" },
];
/// The rows of the second direction of a linear pattern.
const SECOND_DIRECTION_ROWS: [PatternRow; 3] = [
    PatternRow { caption: "cmd-copies", field: "skpat_count2" },
    PatternRow { caption: "opt-step-x", field: "skpat_dx2" },
    PatternRow { caption: "opt-step-y", field: "skpat_dy2" },
];
/// The rows of a circular pattern.
const AROUND: [PatternRow; 2] = [PatternRow { caption: "cmd-copies", field: "skpat_count" }, PatternRow { caption: "cmd-angle", field: "skpat_angle" }];

/// The key of the field of the words of a note edited.
const NOTE_TEXT: &str = "note_text";

/// THE BOX OF A NOTE EDITED, at the note: its words, taking the keyboard, Apply and Cancel as every box. A note had a
/// small field of its own over the sheet.
fn note_box(sk: &mut qymcad_ui_state::SketchCtx) -> Option<qymcad_ui_state::BoxSpec> {
    use qymcad_ui_state::{Opt, OptKey, Row, Section};
    let ni = sk.inline.note()?;
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return None };
    let n = sk.project.sketches.get(si)?.notes.get(ni)?;
    let at = qymcad_ui_state::Sheet { view: *sk.view, rect: sk.canvas }.at(Point2::new(n.x, n.y));
    let focus = qymcad_ui_state::Focus::take(&mut sk.annot.note_focus);
    let words = Opt::Text { key: OptKey::of(NOTE_TEXT), text: sk.annot.note_buf.clone(), placeholder: String::new(), unfit: None, hint: None, focus };
    let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(at, &[at])));
    spec.layout.sections = vec![Section::plain(vec![Row::new(qymcad_i18n::tr("opt-note")).opt(words)])];
    Some(spec)
}

/// The key of the title of the second direction of a linear pattern of a sketch: opened, the direction is on.
pub(crate) const SECOND_DIRECTION: &str = "skpat_dir2";

impl FramedTool for InSketch<'_, '_> {
    fn describe(&mut self, ctx: &egui::Context) -> Described {
        self.ctx = Some(ctx.clone());
        self.boxed = Boxed::Nothing;
        self.gate = Gate::Open;
        self.keeping = Keeping::NONE;
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
                // A DIMENSION TOOL THAT MEASURED NOTHING is put down by Enter as by Esc; one begun is put where the pointer is
                let measured_nothing = sk.armed.dimension().is_some() && sk.dim.first.is_none() && sk.dim.pick.is_empty();
                // A CIRCULAR PATTERN TURNS ABOUT A CENTRE CLICKED: Enter before it is refused in words, and nothing is made
                let no_centre = sk.armed.pattern() == Some(qymcad_ui_state::PatternKind::Circular) && sk.pat.center.is_none();
                let placing_text = sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text) && sk.inline.text().is_none();
                // a text, a move and a copy do one thing too, finished by a click (`finished_by_click`)
                let by_click = sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text) || sk.armed.moving().is_some();
                let empty = matches!(*sk.armed, qymcad_ui_state::Armed::None);
                let pattern_laid = pattern_box(sk.armed, sk.sk_pat);
                let text_laid = text_box(sk);
                if text_laid.is_some() && sk.inline.text().is_some() {
                    self.boxed = Boxed::Label;
                }
                let note_laid = note_box(sk);
                if note_laid.is_some() {
                    self.boxed = Boxed::Note;
                }
                let boxed = self.box_now().or(pattern_laid).or(text_laid).or(note_laid);
                let ends = match self.boxed {
                    Boxed::Turn => Ends::ApplyCancel,
                    // THE BOX OF A SHAPE ENDS AS EVERY BOX DOES: Apply lays what was typed, Cancel - as Esc - leaves the shape as
                    // it stands without it
                    Boxed::Shape | Boxed::Label | Boxed::Note => Ends::ApplyCancel,
                    Boxed::Nothing if pattern => Ends::ApplyCancel,
                    Boxed::Nothing if empty => Ends::Nothing,
                    Boxed::Nothing => Ends::Close,
                };
                // THE TURN AND THE PATTERN DO ONE THING: made, the selection is in hand; a shape, a dimension, the corner and
                // the offset stay for the next
                let after = if self.boxed == Boxed::Turn || (self.boxed == Boxed::Nothing && (pattern || by_click || measured_nothing)) { AfterApply::Select } else { AfterApply::Stays };
                let mut bar = bar;
                // the radius of the next circle touching two curves is no size of the circle in the box: its own field is
                if self.boxed == Boxed::Shape {
                    bar.retain(|o| !matches!(o, Opt::Number { key, .. } if key.name == "sk_circle_touching_r"));
                }
                let gate = if no_centre {
                    Gate::Waiting(qymcad_i18n::tr("opt-click-rotation-centre"))
                } else if placing_text && self.boxed == Boxed::Nothing {
                    // A TEXT IS PLACED BY A CLICK: Enter before it is refused in words, and the tool stays with what was typed
                    Gate::Waiting(qymcad_i18n::tr("sk-text-click-to-place"))
                } else {
                    self.gate.clone()
                };
                let mut d = Described { head: Head { after, ..head(name, ends) }, bar, gate, says: Vec::new(), boxed };
                self.name_kept_values(&mut d);
                d
            }
        }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        match self.boxed {
            Boxed::Turn => &mut self.sk.rot.field,
            Boxed::Shape | Boxed::Label | Boxed::Note | Boxed::Nothing => &mut self.sk.place.fields,
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
        if let Did::Retyped { key, text } = &did {
            if key.name == NOTE_TEXT {
                self.sk.annot.note_buf = text.clone();
                return;
            }
        }
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
                let Keeping { kept, item } = self.keeping;
                let named = |sk: &mut qymcad_ui_state::SketchCtx| {
                    shape_lay(sk, si, &self.asks);
                    if let Some(c) = item {
                        // THE SIDES TYPED ARE THE SIDES OF THIS POLYGON: they went to the next one drawn
                        if kept.iter().any(|k| k.key == "sides") {
                            sk.project.set_polygon_sides(si, c, sk.tool_prefs.poly_n);
                        }
                        qymcad_ui_state::keep_item_values(sk.project, &self.asks, &item_values(kept, c));
                    }
                };
                if follows(&sk.place.shape) == Follows::LaidAnew {
                    // the part the figures were tried on taken back alone, or the document as the box found it
                    match sk.dim.trial.as_ref().filter(|_| trying(sk)) {
                        Some(t) => {
                            sk.project.trial_restore(t);
                            named(sk);
                        }
                        None => {
                            try_on(sk, From::Snapshot, named);
                        }
                    }
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
                return Ok(());
            }
            // ENTER IN THE BOX OF A LABEL: the label takes its string and height, re-baked in its own font, as one step with
            // the drivers typed; the editing ends with the tool
            Boxed::Label => {
                qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-text"));
                qymcad_ui_state::text_edit_apply(sk, si);
                if let Some(t) = self.keeping.item {
                    qymcad_ui_state::keep_item_values(sk.project, &self.asks, &item_values(self.keeping.kept, t));
                }
                qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
                qymcad_ui_state::end_label_edit(sk);
                return Ok(());
            }
            // ENTER IN THE BOX OF A NOTE: the note takes its words, as one step; the editing ends
            Boxed::Note => {
                if let Some(ni) = sk.inline.note() {
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("opt-note"));
                    sk.project.set_note_text(si, ni, sk.annot.note_buf.clone());
                    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
                }
                sk.inline.clear();
                return Ok(());
            }
            Boxed::Nothing => {}
        }
        let pointer = self.ctx.as_ref().and_then(|c| c.input(|i| i.pointer.hover_pos())).filter(|p| sk.canvas.contains(*p));
        if sk.armed.pattern().is_some() {
            return confirm_pattern(sk, si, &self.asks);
        }
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
            let before = sk.project.sketches[si].constraints.len();
            offset_make(
                OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
                si,
                dist,
                at,
            );
            // its driver names the distance it laid, as the box of an offset names it
            if let Some(a) = self.asks.iter().find(|a| a.field == "sk_offset") {
                name_what_was_laid(sk, Laying { si, since: before, value: dist.abs() }, a);
            }
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
                Boxed::Note => sk.inline.clear(),
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
        let by_click = std::mem::take(&mut self.sk.tool.finished) == qymcad_ui_state::Finished::ByClick;
        // A TEXT PLACED BY THE CLICK keeps the height typed for it, and its driver: the label stands only now
        let sk = &mut *self.sk;
        let label = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).and_then(|si| sk.project.sketches[si].texts.last().map(|t| t.id));
        if let (true, Some(id)) = (by_click && sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text), label) {
            let typed = sk.bar_exprs.get("text_h").cloned();
            let asks: Vec<qymcad_ui_state::FieldAsk> = self.asks.iter().cloned().map(|a| qymcad_ui_state::FieldAsk { formula: typed.clone().unwrap_or(a.formula), ..a }).collect();
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("box-driver-step"));
            qymcad_ui_state::keep_item_values(sk.project, &asks, &item_values(&TEXT_KEPT, id));
            qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        }
        by_click
    }

    /// ONE STEP OF THE Esc OF A SKETCH (`escape_step`). The box of the sizes of a shape is a step of its own: it goes, the
    /// tool that drew the shape stays for the next, and the next Esc puts the tool down.
    fn back(&mut self) -> Stepped {
        if self.boxed == Boxed::Note {
            self.sk.inline.clear();
            return Stepped::Back;
        }
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
                        Follows::StepByStep => {
                            try_on(sk, From::AsItStands, |sk| follow_typed(sk, si));
                        }
                        Follows::LaidAnew => try_shape(sk, si, &self.asks),
                    }
                }
                // THE SIDES TYPED FOLLOWED BY THE POLYGON IN THE BOX: a number of the bar, they went to the next one drawn
                if let qymcad_ui_state::PlacingShape::Poly(c) = sk.place.shape {
                    let n = sk.tool_prefs.poly_n;
                    if sk.project.kept_number(c, "sides").is_some_and(|now| now.round() as u32 != n.max(3)) {
                        try_on(sk, From::AsItStands, |sk| {
                            sk.project.set_polygon_sides(si, c, n);
                        });
                    }
                }
            }
            Boxed::Turn => sk.rot.angle = sk.rot.field.first().map_or(0.0, |p| p.val),
            Boxed::Label | Boxed::Note | Boxed::Nothing => {}
        }
    }
}

/// A VALUE TYPED INTO THE BOX OF A SHAPE WHOSE SIZES ARE DIMENSIONS, TRIED: the part of the sketch the shape stands in is
/// found with the first figure (`Project::trial_round`), before the dimensions are laid; every figure takes back that
/// part alone and solves it alone, the dimensions the box laid with it.
fn try_shape(sk: &mut qymcad_ui_state::SketchCtx, si: usize, asks: &[qymcad_ui_state::FieldAsk]) {
    if !trying(sk) {
        sk.dim.trial = None;
    }
    if sk.dim.trial.is_some() {
        try_on(sk, From::AsItStands, |sk| {
            if let Some(t) = sk.dim.trial.as_ref() {
                sk.project.trial_restore(t);
            }
            shape_lay(sk, si, asks)
        });
        return;
    }
    // THE FIRST FIGURE MAKES THE TRIAL BEFORE IT LAYS ANYTHING: the part the shape stands in, found around a point of it,
    // the dimensions the box lays solved with that part alone - over the document as it stands, which nothing was tried
    // on yet and the trial keeps for Esc. Laid over a copy of the document taken back, and each of them solving the whole
    // sketch, the first figure among 10 000 lines cost 100-115 ms.
    if !trying(sk) {
        if let Some(seed) = shape_seed(sk, si) {
            try_on(sk, From::AsItStands, |sk| {
                sk.dim.trial = sk.project.trial_round(si, seed);
                shape_lay(sk, si, asks)
            });
            return;
        }
    }
    // a shape with no point to stand by, or a figure tried before with no part found: over the document as the box found it
    try_on(sk, From::Snapshot, |sk| shape_lay(sk, si, asks));
}

/// A POINT OF THE SHAPE IN THE BOX, the part of the sketch it stands in being what its sizes move.
fn shape_seed(sk: &qymcad_ui_state::SketchCtx, si: usize) -> Option<qymcad_core::model::Id> {
    use qymcad_ui_state::PlacingShape;
    let curve = match sk.place.shape {
        PlacingShape::RectSide(id) | PlacingShape::Slot(id) | PlacingShape::Round(id) => id,
        _ => return None,
    };
    sk.project.sketches.get(si)?.entity(curve).map(qymcad_core::model::entity_points)?.first().copied()
}

/// APPLY OR UPDATE THE PATTERN of the sketch: one step of undo; nothing chosen to repeat, it is refused in words and the
/// tool stays in hand.
fn confirm_pattern(sk: &mut qymcad_ui_state::SketchCtx, si: usize, asks: &[qymcad_ui_state::FieldAsk]) -> Result<(), NotApplied> {
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("g-sketch-array")); // THE OPERATION BOUNDARY
    if let Some(pi) = sk.pat.edit {
        let src = sk.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default();
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &src);
        sk.project.update_pattern(si, pi, kind);
        if let Some(id) = sk.project.sketches[si].patterns.get(pi).map(|p| p.id) {
            qymcad_ui_state::keep_item_values(sk.project, asks, &item_values(&PATTERN_KEPT, id));
        }
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-updated"));
    } else {
        let eids = qymcad_ui_state::shapes_chosen(&*sk.project, si, &*sk.sel_sk);
        if eids.is_empty() {
            qymcad_ui_state::abort_edit(&mut sk.rebuild());
            return Err(NotApplied::Refused(qymcad_i18n::tr("g-select-for-array")));
        }
        let kind = qymcad_ui_state::current_pattern_kind(&*sk.armed, *sk.pat, &*sk.project, *sk.sk_pat, si, &eids);
        let id = sk.project.add_pattern(si, &eids, kind);
        qymcad_ui_state::keep_item_values(sk.project, asks, &item_values(&PATTERN_KEPT, id));
        *sk.status = Notice::info(qymcad_i18n::tr("g-array-created"));
    }
    qymcad_ui_state::invalidate(&mut *sk.regen);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    Ok(())
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
