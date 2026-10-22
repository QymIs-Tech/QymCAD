//! THE EDITORS AT THE GEOMETRY of a sketch: the value of a dimension, the size of a circle or an arc, the value of the
//! corners of a set, the distance of an offset - each a tool the frame drives (`qymcad_ui_state::tool_in_hand`), with its
//! box at the geometry, its Enter and its Esc, opened by a double click or by a click at a corner.

use qymcad_core::model::Constraint;
use qymcad_ui_state::{Described, Did, Ends, FramedTool, Gate, Head, NotApplied, Notice, Opt, OptKey, Shown, Stepped};
use super::*;

/// WHICH EDITOR IS OPEN on the sketch, if one is: a dimension, the corners of a set, an offset.
pub(crate) enum Editor {
    Dim(Dim),
    Corner(CornerAt),
    Offset(OffsetAt),
}

/// The editor open on the sketch now, if one is; one whose geometry went from under it is put away.
pub(crate) fn editor_open(sk: &mut qymcad_ui_state::SketchCtx) -> Option<Editor> {
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return None };
    if let Some(ci) = sk.inline.dim() {
        let rect = sk.canvas;
        let ok = sk.project.sketches.get(si).and_then(|s| s.constraints.get(ci)).and_then(kind_of).is_some() && dim_label_pos(sk, rect, si, ci).is_some();
        if ok {
            return Some(Editor::Dim(Dim { si, ci }));
        }
        put_away(sk);
        return None;
    }
    if let Some((csi, pid, chamfer)) = sk.corner.at {
        // A CORNER FOLLOWING THE POINTER HAS NO BOX YET: the box would stand beside a corner that moves, and run from the
        // pointer that goes to it; and a set with nothing to cut in it has nothing the value is for
        if sk.corner.aim != qymcad_ui_state::Aim::Following && !sk.corner.set.standing().is_empty() {
            return Some(Editor::Corner(CornerAt { si: csi, pid, chamfer: if chamfer { Cut::Chamfer } else { Cut::Fillet } }));
        }
        return None;
    }
    let at = sk.sel_sk.offset.at.filter(|_| sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset))?;
    if offset_curves(sk.sel_sk).is_empty() {
        sk.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        return None;
    }
    Some(Editor::Offset(OffsetAt { si, at }))
}

/// The editor `e` over sketch context `sk`, as a tool of the frame.
pub(crate) fn editor_tool<'a>(sk: &'a mut qymcad_ui_state::SketchCtx<'_>, e: Editor) -> Box<dyn FramedTool + 'a> {
    match e {
        Editor::Dim(dim) => Box::new(DimEdit { sk, dim, asks: Vec::new() }),
        Editor::Corner(at) => Box::new(CornerEdit { sk, at, asks: Vec::new() }),
        Editor::Offset(at) => Box::new(OffsetEdit { sk, at, asks: Vec::new() }),
    }
}

/// The box of a dimension put away, and nothing typed into it kept.
fn put_away(sk: &mut qymcad_ui_state::SketchCtx) {
    sk.inline.clear();
    sk.dim.put_away();
}

/// The head of an editor: its name by its catalogue key, Apply and Cancel; applied, the tool under it stays in hand.
fn head(icon: &'static str, name: &str) -> Head {
    Head { icon, name: qymcad_i18n::tr(name), ends: Ends::ApplyCancel, after: qymcad_ui_state::AfterApply::Stays }
}

/// WHERE A TRIAL OF VALUES STARTS FROM: the document as it was when the box opened - a value laid anew over what it
/// stood at - or as it stands, for a shape that follows its fields step by step.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum From {
    Snapshot,
    AsItStands,
}

/// WHERE THE VALUES KEPT BY Enter GO: a step of their own, or the step that made the dimension they size - making a
/// dimension and typing its value is one act, undone by one Ctrl+Z.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kept {
    OwnStep,
    IntoTheLast,
}

/// IS A TRIAL OF VALUES OPEN on the document.
pub(crate) fn trying(sk: &qymcad_ui_state::SketchCtx) -> bool {
    sk.edits.open.is_some() && sk.edits.open_kind == qymcad_ui_state::OpenEdit::Trying
}

/// THE VALUES OF A BOX TRIED ON THE DOCUMENT AS THEY ARE TYPED: the geometry takes a good value at once, so what is typed
/// is seen where it goes - while the document and undo get it only by Enter (`trial_kept`), and Esc puts everything back
/// with no step (`trial_dropped`). The trial is one open edit; every value is laid anew over the document as the box
/// found it, so erasing "100" passes through 10 and 1 on the screen only, as nothing is kept until Enter. A value refused
/// or half typed is not tried: the geometry stays as the last good value left it. None while another operation holds
/// the document.
pub(crate) fn try_on<T>(sk: &mut qymcad_ui_state::SketchCtx, from: From, lay: impl FnOnce(&mut qymcad_ui_state::SketchCtx) -> T) -> Option<T> {
    if sk.edits.open.is_none() {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
        sk.edits.open_kind = qymcad_ui_state::OpenEdit::Trying;
    }
    if !trying(sk) {
        return None;
    }
    if from == From::Snapshot {
        if let Some((_, before)) = &sk.edits.open {
            *sk.project = before.project.clone();
        }
    }
    let laid = lay(sk);
    qymcad_ui_state::invalidate(&mut *sk.regen);
    Some(laid)
}

/// THE VALUES TRIED KEPT, by Enter: one step of undo, or the step that made the dimension.
pub(crate) fn trial_kept(sk: &mut qymcad_ui_state::SketchCtx, kept: Kept) {
    if !trying(sk) {
        return;
    }
    match kept {
        Kept::OwnStep => qymcad_ui_state::commit_edit(&mut sk.rebuild()),
        Kept::IntoTheLast => {
            sk.edits.open = None;
            sk.edits.depth = 0;
            sk.edits.open_kind = qymcad_ui_state::OpenEdit::Working;
            qymcad_ui_state::fold_into_last_step(&mut *sk.edits, &*sk.project);
        }
    }
}

/// THE VALUES TRIED TAKEN BACK, by Esc or by the box going: the document as the box found it, and no step.
pub(crate) fn trial_dropped(sk: &mut qymcad_ui_state::SketchCtx) {
    if trying(sk) {
        qymcad_ui_state::abort_edit(&mut sk.rebuild());
    }
}

/// THE BAR UNDER AN EDITOR AT THE GEOMETRY IS THE BAR OF THE TOOL IN HAND, its modes and its hint as they stand without
/// the box: the ways of a fillet, the hint of the dimension tool. A number the box types itself - the radius of a fillet,
/// the distance of an offset - is the box's field, and is not given twice.
fn bar_under(sk: &qymcad_ui_state::SketchCtx, typed_by_box: &[&'static str]) -> Vec<Opt> {
    let mut opts = tool_opts(sk);
    opts.retain(|o| !matches!(o, Opt::Number { key, .. } if typed_by_box.contains(&key.name)));
    opts
}

/// A box standing by `at` on the sheet, clear of `around`.
fn box_by(at: Pos2, around: &[Pos2], focus: &mut bool) -> qymcad_ui_state::BoxSpec {
    let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(at, around)));
    spec.focus = qymcad_ui_state::Focus::take(focus);
    spec
}

// ---------------------------------------------------------------------------------------------------------------------
// THE SIZE OF A CIRCLE OR AN ARC

/// THE SIZE OF CIRCLE OR ARC `eid` OF SKETCH `si` LAID AS ITS DIMENSION, of radius `rr`: one step of undo, named as a
/// dimension is.
pub(crate) fn set_round_size(sk: &mut qymcad_ui_state::SketchCtx, si: usize, eid: Id, rr: f64) {
    let is_arc = sk.project.sketches.get(si).and_then(|s| s.entity(eid)).is_some_and(|e| e.kind.as_arc().is_some());
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    if is_arc {
        // a fillet arc has its radius edited through the dimension constraint (parametrically); failing that, the
        // fillet is recomputed geometrically; failing that, it is a plain arc, given its dimension as a circle has
        if !sk.project.set_fillet_radius_dim(si, eid, rr) && !sk.project.set_fillet_radius(si, eid, rr) {
            sk.project.put_arc_radius_dim(si, eid, rr);
        }
    } else {
        // A DIAMETER TYPED IS LAID AS A DIMENSION, of the value typed
        let centre = sk.project.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)).and_then(|e| {
            let centre = e.kind.as_circle()?.center;
            e.kind.set_radius(rr);
            Some(centre)
        });
        if let Some(ci) = centre.and_then(|c| sk.project.ensure_diameter(si, c, true)) {
            if let Some(Constraint::Diameter { d, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                *d = 2.0 * rr;
            }
        }
        // THE SOLVER, not a rebuild. The radius of a circle is a variable of the solve (it is keyed by the centre), so a
        // radius written straight into the entity is a value the solver has not seen: a tangency or an equality holding
        // this circle would go on being drawn as satisfied while it no longer is.
        sk.project.solve_sketch(si);
    }
    qymcad_ui_state::invalidate(&mut *sk.regen);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
}

// ---------------------------------------------------------------------------------------------------------------------
// THE VALUE OF A DIMENSION

/// The key of the field of the value of a dimension.
const VALUE: &str = "value";
/// The switch between the radius and the diameter of a dimension of a circle, by the place of the dimension in its box.
pub(crate) const ROUND: &str = "dim-radius-or-diameter";
/// The near or the far edge of a circle a tangent gap is measured to.
const EDGE: &str = "dim-edge";

/// A DIMENSION IN ITS BOX: its sketch and its place among the constraints of the sketch.
#[derive(Clone, Copy)]
pub(crate) struct Dim {
    pub(crate) si: usize,
    pub(crate) ci: usize,
}

/// WHAT KIND OF A DIMENSION A BOX HOLDS, as far as the box goes: the caption of its value and what it is switched
/// between.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Length,
    Angle,
    /// Of a circle, shown as its radius or its diameter.
    Round(Shown),
    /// A tangent gap to a circle, to its near or its far edge.
    Edge,
}

/// A DIMENSION AS ITS BOX SHOWS IT: its constraint and its kind.
struct DimShown {
    c: Constraint,
    kind: Kind,
}

/// What kind of a dimension `c` is, none for a constraint that is not a dimension.
fn kind_of(c: &Constraint) -> Option<Kind> {
    Some(match c {
        Constraint::Angle { .. } | Constraint::AngleLines { .. } => Kind::Angle,
        Constraint::Diameter { diam: true, .. } => Kind::Round(Shown::Diameter),
        Constraint::Diameter { .. } => Kind::Round(Shown::Radius),
        Constraint::EdgeDistance { .. } => Kind::Edge,
        Constraint::Distance { .. } | Constraint::DistancePL { .. } | Constraint::CurveDistance { .. } | Constraint::ArcLength { .. } => Kind::Length,
        _ => return None,
    })
}

/// The value a dimension shows: `DistancePL` stores a SIGNED d, which side of the line, and the magnitude is what is
/// shown and edited.
fn shown_value(c: &Constraint) -> f64 {
    match c {
        Constraint::DistancePL { d, .. } => d.abs(),
        c => c.dim_value().unwrap_or(0.0),
    }
}

/// What the field of a dimension shows: its formula, or its number when it has none.
fn shown_text(c: &Constraint) -> String {
    match c.expr() {
        Some(e) => e.to_string(),
        None => qymcad_core::expr::fmt_num(shown_value(c)),
    }
}

/// The caption key of the value of a dimension of kind `kind`.
fn caption_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Angle => "sk-angle",
        Kind::Edge => "sk-tangent-short",
        Kind::Length | Kind::Round(_) => "sk-dim",
    }
}

/// The caption of the value of a dimension of kind `kind` as its row says it: a dimension of a circle by the sign of
/// what it holds, Ø or R, as the dimension on the sheet is written.
pub(crate) fn row_caption(kind: Kind) -> String {
    match kind {
        Kind::Round(Shown::Diameter) => "\u{d8}".to_string(),
        Kind::Round(Shown::Radius) => "R".to_string(),
        k => qymcad_i18n::tr(caption_of(k)),
    }
}

/// The field of the value of dimension `c` of kind `kind`. Its limits are the dimension's own (`dim_value_refusal`),
/// said in its own words: the field takes any number.
fn value_field(c: &Constraint, kind: Kind) -> qymcad_ui_state::CmdParam {
    let mut p = qymcad_ui_state::CmdParam::new(caption_of(kind), VALUE, shown_value(c), f64::MIN, f64::MAX);
    p.txt = shown_text(c);
    p
}

/// The constraint of dimension `dim`, if it still stands.
fn constraint_of(sk: &qymcad_ui_state::SketchCtx, dim: Dim) -> Option<Constraint> {
    sk.project.sketches.get(dim.si).and_then(|s| s.constraints.get(dim.ci)).cloned()
}

/// What dimension `c` of sketch `si` is told by among the names of the document: none for one with no entities to tell.
fn target_of(sk: &qymcad_ui_state::SketchCtx, si: usize, c: &Constraint) -> Option<qymcad_core::model::DimTarget> {
    let refs = Project::dim_refs(c)?;
    Some(qymcad_core::model::DimTarget::Sketch { sketch: sk.project.sketches[si].id, refs: Project::dim_key_pub(&refs) })
}

/// THE BOX OF THE VALUE OF A DIMENSION, at its label: the value - a number or a formula - with what the dimension is
/// switched between, and the name the formulas of the document read it by.
struct DimEdit<'a, 'b> {
    sk: &'a mut qymcad_ui_state::SketchCtx<'b>,
    dim: Dim,
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for DimEdit<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let Dim { si, ci } = self.dim;
        let rect = self.sk.canvas;
        let at = dim_label_pos(self.sk, rect, si, ci);
        let sk = &mut *self.sk;
        let head = head(ph::RULER, "sk-dim");
        let shown = constraint_of(sk, self.dim).and_then(|c| kind_of(&c).map(|kind| DimShown { c, kind }));
        let Some(DimShown { c, kind }) = shown else {
            return Described { head, bar: Vec::new(), gate: Gate::Open, says: Vec::new(), boxed: None };
        };
        let Some(at) = at else {
            return Described { head, bar: Vec::new(), gate: Gate::Open, says: Vec::new(), boxed: None };
        };
        if sk.dim.focus || sk.dim.field.is_empty() {
            sk.dim.field = vec![value_field(&c, kind)];
        }
        let target = target_of(sk, si, &c);
        let gate = value_gate(sk.project, &c, target.as_ref(), &sk.dim.field);
        let mut spec = box_by(at, &[at], &mut sk.dim.focus);
        // A DRIVER BELONGS TO ANY DIMENSION, NOT ONLY TO A LINEAR ONE: an angle, a diameter, a distance to a line, an arc
        // length or a tangent gap is named as well, a driver being told by the set of entities it sizes; the frame keeps
        // the name and the reference as they are typed and switched (`take_asks`)
        let had = target.as_ref().map(|t| qymcad_i18n::name(&sk.project.name_of_target(t))).unwrap_or_default();
        let lit = if c.is_driven() { qymcad_ui_state::Lit::On } else { qymcad_ui_state::Lit::Off };
        spec.names = vec![qymcad_ui_state::FieldName::of(VALUE, had, target).with_driven(lit)];
        spec.layout = dim_layout(dim_rows(DimRows { caption: row_caption(kind), field: VALUE, at: 0, kind }));
        Described { head, bar: bar_under(sk, &[]), gate, says: Vec::new(), boxed: Some(spec) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.dim.field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    /// A refused Enter is said on the status line of the sketch too, as the boxes of a sketch said it on main.
    fn refused(&mut self, words: &Notice) {
        *self.sk.status = words.clone();
    }

    fn took(&mut self, did: Did) {
        let sk = &mut *self.sk;
        let asked = match did {
            Did::Pressed { key } if key.name == ROUND => Asked { round: Ask::Yes, ..Asked::default() },
            Did::Pressed { key } if key.name == EDGE => Asked { edge: Ask::Yes, ..Asked::default() },
            _ => {
                opt_took(sk, did);
                return;
            }
        };
        // a switch carries no value, so nothing of it is refused; a refusal would still be said
        if let Err(key) = change_dimension(sk, self.dim, asked) {
            *sk.status = Notice::refusal(qymcad_i18n::tr(key));
        }
        // the field shows the dimension anew: a diameter switched to a radius, an edge taken, a reference made
        let Dim { si, ci } = self.dim;
        sk.dim.field = sk.project.sketches[si].constraints.get(ci).and_then(|c| kind_of(c).map(|k| vec![value_field(c, k)])).unwrap_or_default();
    }

    /// The value typed goes in - laid once more over the document as the box found it and kept as one step, or joined to
    /// the step that made the dimension - refused with its words when the dimension cannot take it, the box staying open
    /// on what was typed; and the box goes.
    fn apply(&mut self) -> Result<(), NotApplied> {
        if let Some(Err(key)) = self.try_typed(Tried::Again) {
            return Err(NotApplied::Refused(qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0))));
        }
        let fresh = self.sk.dim.fresh.is_some_and(|(fc, steps)| fc == self.dim.ci && steps == self.sk.edits.undo.len());
        trial_kept(self.sk, if fresh { Kept::IntoTheLast } else { Kept::OwnStep });
        // the reference switched and the name typed in the row of the value go in as the value did
        if let Some(ask) = self.asks.first().cloned() {
            let driven_was = constraint_of(self.sk, self.dim).is_some_and(|c| c.is_driven());
            if (ask.driven == qymcad_ui_state::Lit::On) != driven_was {
                let _ = change_dimension(self.sk, self.dim, Asked { driven: Ask::Yes, ..Asked::default() });
            }
            name_dimension(self.sk, self.dim, &ask.name);
        }
        put_away(self.sk);
        self.sk.dim.fresh = None;
        Ok(())
    }

    /// The box goes, and the value tried on the geometry with it.
    fn cancel(&mut self) {
        trial_dropped(self.sk);
        put_away(self.sk);
        self.sk.dim.fresh = None;
    }

    /// A good value typed is tried on the geometry at once (`try_on`).
    fn after(&mut self) {
        let _ = self.try_typed(Tried::OnceAText);
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    fn back(&mut self) -> Stepped {
        self.cancel();
        Stepped::Back
    }

    fn field_did(&mut self, did: qymcad_ui_state::FieldDid) {
        if did == qymcad_ui_state::FieldDid::LeftByEsc {
            self.cancel();
        }
    }
}

/// IS A VALUE TRIED AGAIN even when its text was tried already: once a text while it is typed, again on Enter, over the
/// document as the box found it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tried {
    OnceAText,
    Again,
}

impl DimEdit<'_, '_> {
    /// THE VALUE IN THE FIELD TRIED ON THE GEOMETRY, when it is a value the dimension takes and differs from what it
    /// held: none when there is nothing to try - a reference, a value refused, the text the dimension held - or the
    /// dimension's own refusal of the value. The text the dimension held, typed back, takes the geometry back.
    fn try_typed(&mut self, tried: Tried) -> Option<Result<(), &'static str>> {
        let dim = self.dim;
        let sk = &mut *self.sk;
        let before = sk.edits.open.as_ref().filter(|_| trying(sk)).and_then(|(_, b)| b.project.sketches.get(dim.si).and_then(|s| s.constraints.get(dim.ci)).cloned());
        let c = before.or_else(|| constraint_of(sk, dim)).filter(|c| !c.is_driven())?;
        let p = sk.dim.field.first()?.clone();
        let text = p.txt.trim().to_string();
        if text == shown_text(&c).trim() {
            if sk.dim.tried.take().is_some() {
                try_on(sk, From::Snapshot, |_| ());
            }
            return None;
        }
        let target = target_of(sk, dim.si, &c);
        if qymcad_ui_state::field_value(&p, &sk.project.param_map()).is_err() || !value_gate(sk.project, &c, target.as_ref(), &sk.dim.field).open() {
            return None;
        }
        if tried == Tried::OnceAText && sk.dim.tried.as_deref() == Some(text.as_str()) {
            return None;
        }
        sk.dim.tried = Some(text.clone());
        try_on(sk, From::Snapshot, |sk| change_dimension(sk, dim, Asked { value: Some(Value { number: p.val, text }), ..Asked::default() }))
    }
}

/// THE TOOL'S OWN WORD ON THE VALUE TYPED: a formula coming back to the name of this very dimension, or a value the
/// dimension cannot take - an angle past its opening, a length of zero or past 10 m.
///
/// A FORMULA THAT COMES BACK TO THE NAME OF THIS VERY DIMENSION has no value to take, as the table of parameters says
/// (`check_dim_target_text`): taken, `dia + 1` written for `dia` added one on every solve - measured 20 -> 24 over the
/// solves of one edit and two more.
fn value_gate(project: &Project, c: &Constraint, own: Option<&qymcad_core::model::DimTarget>, field: &[qymcad_ui_state::CmdParam]) -> Gate {
    let Some(p) = field.first() else { return Gate::Open };
    if c.is_driven() || p.txt.trim().is_empty() {
        return Gate::Open;
    }
    let why = match own.map(|t| project.check_dim_target_text(t, &p.txt)) {
        Some(Err(e)) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
        _ => qymcad_ui_state::field_value(p, &project.param_map()).ok().and_then(|v| dim_value_refusal(c, v)).map(|key| qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0))),
    };
    match why {
        Some(why) => Gate::Refused(qymcad_ui_state::Refusal { key: VALUE.to_string(), field: qymcad_ui_state::field_caption(p), why }),
        None => Gate::Open,
    }
}

/// ONE DIMENSION IN A BOX, as its rows say it: its caption, the key of its field, its place among the dimensions of the
/// box - which tells their options apart - its kind, and whether it is a reference. Its name is the frame's row under the
/// field (`BoxSpec::names`).
pub(crate) struct DimRows {
    pub(crate) caption: String,
    pub(crate) field: &'static str,
    pub(crate) at: usize,
    pub(crate) kind: Kind,
}

/// THE ROWS OF ONE DIMENSION, the one description of a dimension wherever it is typed - laid already, or laid by its box
/// on Enter: the value - a number or a formula - with what it comes to said under it, the switch of a radius and a
/// diameter or of the near and the far edge, the unit, and "ref.".
pub(crate) fn dim_rows(d: DimRows) -> Vec<qymcad_ui_state::Row> {
    // THE ROW OF A VALUE, as every box lays it: the caption, the field, its unit, then what the value is switched by -
    // the radius or the diameter, the near or the far edge; its name and its reference follow, laid by the frame
    let unit = if d.kind == Kind::Angle { "unit-deg-suffix" } else { "sk-mm" };
    let mut value = qymcad_ui_state::Row::new(d.caption).field(d.field).unit(unit);
    // THE SIGN OF A ROUND VALUE IS THE BUTTON THAT SWITCHES IT, in the place of the caption: "Ø" shown, a click makes it
    // a radius
    if let Kind::Round(shown) = d.kind {
        let sign = if shown == Shown::Diameter { "\u{d8}" } else { "R" };
        value = value.lead(Opt::press(OptKey::at(ROUND, d.at), sign).hint(qymcad_i18n::tr("sk-toggle-rad-dia")));
    }
    if d.kind == Kind::Edge {
        value = value.opt(Opt::press(OptKey::at(EDGE, d.at), qymcad_i18n::tr("sk-edge-toggle")).hint(qymcad_i18n::tr("sk-near-far-edge")));
    }
    vec![value]
}

/// THE BOX OF DIMENSIONS laid out of their rows.
pub(crate) fn dim_layout(rows: Vec<qymcad_ui_state::Row>) -> qymcad_ui_state::Layout {
    qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(rows)], foot: Vec::new() }
}

/// WHAT WAS ASKED OF A DIMENSION BEFORE ITS BOX LAID IT, given to it now it stands: its radius or diameter, the formula
/// typed, whether it is a reference, its name.
pub(crate) fn ask_laid(sk: &mut qymcad_ui_state::SketchCtx, dim: Dim, ask: AskOf, formula: Option<Value>) {
    let refuse = |sk: &mut qymcad_ui_state::SketchCtx, r: Result<(), &'static str>| {
        if let Err(key) = r {
            *sk.status = Notice::refusal(qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0)));
        }
    };
    if let (Some(shown), Some(Constraint::Diameter { diam, .. })) = (ask.shown, constraint_of(sk, dim)) {
        if diam != (shown == Shown::Diameter) {
            let r = change_dimension(sk, dim, Asked { round: Ask::Yes, ..Asked::default() });
            refuse(sk, r);
        }
    }
    if let Some(value) = formula {
        let r = change_dimension(sk, dim, Asked { value: Some(value), ..Asked::default() });
        refuse(sk, r);
    }
    let Some(row) = ask.row else { return };
    if row.driven == qymcad_ui_state::Lit::On && !constraint_of(sk, dim).is_some_and(|c| c.is_driven()) {
        let r = change_dimension(sk, dim, Asked { driven: Ask::Yes, ..Asked::default() });
        refuse(sk, r);
    }
    if !row.name.trim().is_empty() {
        name_dimension(sk, dim, &row.name);
    }
}

/// WHAT WAS ASKED OF A DIMENSION A BOX LAYS: its radius or diameter, and the row of its name and reference the frame kept.
#[derive(Clone, Copy)]
pub(crate) struct AskOf<'a> {
    pub(crate) shown: Option<Shown>,
    pub(crate) row: Option<&'a qymcad_ui_state::FieldAsk>,
}

/// A FORMULA TYPED INTO FIELD `p`, kept beside the value it comes to; none for a plain number, which the dimension holds
/// as it is.
pub(crate) fn formula_of(p: &qymcad_ui_state::CmdParam) -> Option<Value> {
    let text = p.txt.trim();
    is_formula(text).then(|| Value { number: p.val, text: text.to_string() })
}

/// IS `text` A FORMULA rather than a plain number - THE ONE BARE PARSE OF THE SKETCHER, asked wherever a value is typed:
/// a formula is kept beside the value it comes to, a number is the value.
pub(crate) fn is_formula(text: &str) -> bool {
    text.trim().parse::<f64>().is_err()
}

/// IS SOMETHING ASKED of a dimension.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Ask {
    #[default]
    No,
    Yes,
}

/// WHAT THE PERSON ASKED OF A DIMENSION: a value, a switch of its radius and diameter, of its edge or of whether it
/// drives.
#[derive(Default)]
struct Asked {
    value: Option<Value>,
    round: Ask,
    edge: Ask,
    driven: Ask,
}

/// A VALUE TYPED for a dimension: its number, and the text it was typed as - a number or a formula.
pub(crate) struct Value {
    pub(crate) number: f64,
    pub(crate) text: String,
}

/// THE CHANGES ASKED OF A DIMENSION, made as one step of undo named as a dimension is; the value of a dimension just
/// made joins the step that made it. A value that over-defines the sketch is rolled back with the positions it moved; a
/// value the dimension cannot take is refused with the catalogue key of why.
fn change_dimension(sk: &mut qymcad_ui_state::SketchCtx, dim: Dim, asked: Asked) -> Result<(), &'static str> {
    let Dim { si, ci } = dim;
    // a snapshot of the dimension AND of the point positions BEFORE the edit, so it can be ROLLED BACK if the new value
    // CONFLICTS (over-defines the sketch). Otherwise the solver silently tilted a vertical or moved a fixed point - the
    // soft least-squares compromise.
    let old_con = sk.project.sketches[si].constraints.get(ci).cloned();
    // the value of a dimension just made joins the step that made it; any other change of a dimension is a step of its own
    let fold = sk.dim.fresh.is_some_and(|(fc, steps)| fc == ci && steps == sk.edits.undo.len());
    if !fold {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    }
    let mut changed = false;
    let mut refused = Ok(());
    if let Some(c) = sk.project.sketches[si].constraints.get_mut(ci) {
        if asked.driven == Ask::Yes {
            match c {
                Constraint::Distance { driven, .. }
                | Constraint::DistancePL { driven, .. }
                | Constraint::Angle { driven, .. }
                | Constraint::Diameter { driven, .. }
                | Constraint::AngleLines { driven, .. }
                | Constraint::ArcLength { driven, .. }
                | Constraint::EdgeDistance { driven, .. }
                | Constraint::CurveDistance { driven, .. } => *driven = !*driven,
                _ => {}
            }
            changed = true;
        }
        if asked.round == Ask::Yes {
            if let Constraint::Diameter { d, diam, .. } = c {
                *d = if *diam { *d * 0.5 } else { *d * 2.0 }; // diameter to radius: the value is recomputed
                *diam = !*diam;
            }
            changed = true;
        }
    }
    // THE VALUE IS APPLIED ON COMMIT, NOT ON EVERY LETTER: typing "125" was three rebuilds, and on the "1" and the "12"
    // between the sketch rebuilt itself to a different size. A LENGTH THAT CANNOT EXIST IS REFUSED rather than handed over
    // (`set_dim_value`); a formula is judged by what it evaluates to now.
    if let Some(v) = asked.value {
        let formula = is_formula(&v.text);
        match set_dim_value(&mut *sk.project, si, ci, v.number) {
            Ok(()) => {
                if formula {
                    if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
                        *e = v.text;
                    }
                }
                changed = true;
            }
            Err(key) => refused = Err(key),
        }
    }
    if asked.edge == Ask::Yes {
        // near edge against far edge: the non-zero m values are inverted and d is recomputed from the geometry, so
        // nothing jumps
        if let Some(Constraint::EdgeDistance { c1, c2, m1, m2, .. }) = sk.project.sketches[si].constraints.get(ci).cloned() {
            let nd = sk.project.measure_edge_distance(si, c1, -m1, c2, -m2);
            if let Some(Constraint::EdgeDistance { m1: a, m2: b, d, expr, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                *a = -m1;
                *b = -m2;
                *d = nd;
                expr.clear();
            }
            changed = true;
        }
    }
    if changed {
        solve_or_roll_back(sk, dim, old_con);
        qymcad_ui_state::invalidate(&mut *sk.regen);
    }
    if fold {
        if changed {
            qymcad_ui_state::fold_into_last_step(&mut *sk.edits, &*sk.project);
        }
    } else {
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
    }
    refused
}

/// THE SKETCH SOLVED WITH A DIMENSION CHANGED, or put back as it was. A large residual means the system is
/// INCONSISTENT - the new dimension conflicts with a vertical, a fixed point or another constraint. The solver converges
/// to about 1e-6; the threshold (`SKETCH_FITS`) is above the noise and below any real conflict.
fn solve_or_roll_back(sk: &mut qymcad_ui_state::SketchCtx, dim: Dim, old_con: Option<Constraint>) {
    let Dim { si, ci } = dim;
    let old_pts: Vec<Point2> = sk.project.sketches[si].points.iter().map(|p| Point2::new(p.x, p.y)).collect();
    let resid = sk.project.solve_sketch(si);
    if resid > qymcad_core::model::SKETCH_FITS {
        if let Some(oc) = old_con {
            if let Some(c) = sk.project.sketches[si].constraints.get_mut(ci) {
                *c = oc;
            }
        }
        for (p, was) in sk.project.sketches[si].points.iter_mut().zip(old_pts) {
            p.x = was.x;
            p.y = was.y;
        }
        sk.project.solve_sketch(si);
        *sk.status = Notice::refusal(qymcad_i18n::tr1("sk-dim-incompatible", "r", &qymcad_i18n::num(resid, 2)));
    }
}

/// THE DRIVER NAME GOES IN AS ONE OPERATION, ON ENTER. Reported behaviour: the name was written into the model on EVERY
/// letter, and every node carrying dimensions was marked dirty after it - the whole project rebuilt per letter. A name
/// that will not do is not given; the box says why under the field.
fn name_dimension(sk: &mut qymcad_ui_state::SketchCtx, dim: Dim, typed: &str) {
    let Some(c) = constraint_of(sk, dim) else { return };
    let Some(target) = target_of(sk, dim.si, &c) else { return };
    let old = qymcad_i18n::name(&sk.project.name_of_target(&target)).trim().to_string();
    let nm = typed.trim().to_string();
    if nm == old || qymcad_ui_state::name_unfit(sk.project, Some(&target), &nm).is_some() {
        return;
    }
    let mut ed = qymcad_ui_state::edit_over(sk.rebuild(), qymcad_i18n::tr("box-driver-step"));
    qymcad_ui_state::give_name(ed.project(), target, &nm);
    drop(ed);
    // ONLY WHAT DEPENDS ON THIS NAME is marked, not the whole timeline of every body; it is rebuilt when the sketch
    // closes (`rebuild_after_sketch`). Asked for here, a name nothing used yet rebuilt 0 nodes for 0.8 s under the sign
    // of a rebuild.
    if !nm.is_empty() {
        sk.project.mark_param_dependents_dirty_for(&nm);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// THE CORNERS OF A SET

/// WHICH CORNER A TOOL CUTS: a fillet or a chamfer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cut {
    Fillet,
    Chamfer,
}

/// THE CORNERS OF A SET IN THE BOX: the sketch, the point the box belongs to - none for every corner of a shape - and
/// whether they are filleted or chamfered.
#[derive(Clone, Copy)]
pub(crate) struct CornerAt {
    si: usize,
    pid: Id,
    chamfer: Cut,
}

/// The context of the corner tools, out of the sketch context.
fn corner_ctx<'s>(sk: &'s mut qymcad_ui_state::SketchCtx) -> qymcad_ui_state::CornerCtx<'s> {
    qymcad_ui_state::CornerCtx {
        corner: &mut *sk.corner,
        project: &mut *sk.project,
        edits: &mut *sk.edits,
        sel_sk: &mut *sk.sel_sk,
        view: &*sk.view,
        regen: &mut *sk.regen,
        status: &mut *sk.status,
        tool_prefs: &mut *sk.tool_prefs,
        scheme: sk.scheme,
        set: sk.set,
        sel: &*sk.sel,
        armed: &*sk.armed,
    }
}

/// THE BOX OF THE SET OF CORNERS: one value cuts every corner the chosen lines make, and the set is beside the field
/// rather than under the pointer, so it does not move with it - Enter applies, Esc cancels, and both act on the whole set
/// as one step of undo.
struct CornerEdit<'a, 'b> {
    sk: &'a mut qymcad_ui_state::SketchCtx<'b>,
    at: CornerAt,
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl CornerEdit<'_, '_> {
    /// The fields of the box: the value, and the second value of a chamfer of two legs or of a leg and an angle, made of
    /// the values of the bar as the box opens and captioned by the way the bar gives them.
    fn fields_now(&mut self) {
        let sk = &mut *self.sk;
        let prefs = &*sk.tool_prefs;
        // "R of every corner" is the field of FILLET ALL, and of nothing else: it rounds a whole contour rather than a
        // corner of it
        let first = match self.at.chamfer {
            Cut::Chamfer => qymcad_ui_state::chamfer_d1_label(prefs.chamfer_mode),
            Cut::Fillet if self.at.pid == 0 => "sk-r-all-corners",
            Cut::Fillet => qymcad_ui_state::fillet_label(prefs.fillet_by),
        };
        if sk.corner.fields.is_empty() {
            let mut r = qymcad_ui_state::CmdParam::new(first, "r", prefs.fillet, 0.0, f64::MAX).nonzero();
            r.txt = qymcad_core::expr::fmt_num(prefs.fillet);
            let mut r2 = qymcad_ui_state::CmdParam::new(qymcad_ui_state::chamfer_d2_label(prefs.chamfer_mode), "r2", prefs.chamfer_second, 0.0, f64::MAX).nonzero();
            r2.txt = qymcad_core::expr::fmt_num(prefs.chamfer_second);
            sk.corner.fields = vec![r, r2];
        }
        let second = qymcad_ui_state::chamfer_d2_label(prefs.chamfer_mode);
        for p in &mut sk.corner.fields {
            p.set_label(if p.key == "r" { first } else { second });
        }
    }

    /// Does the chamfer in hand take a second value - two legs, or a leg and an angle?
    fn second_used(&self) -> bool {
        self.at.chamfer == Cut::Chamfer && self.sk.tool_prefs.chamfer_mode != qymcad_core::feature::ChamferMode::Symmetric
    }

    /// WHY THE VALUES AS TYPED CANNOT BE TAKEN: past the leg or the radius the corners hold, and nothing is drawn to
    /// promise what cannot be built. ONE VALUE CUTS THE WHOLE SET, so it is held by the tightest corner in it AND by what
    /// the lines between the corners spend on themselves. A fillet takes the bound itself: two fillets of 10 on a side of
    /// 20 meet and the side goes; a leg of a chamfer as long as its line is refused, as `chamfer_lines` refuses it.
    fn gate(&self) -> Gate {
        let sk = &*self.sk;
        let chamfer = self.at.chamfer == Cut::Chamfer;
        let limit = sk.corner.set.limit(sk.project, self.at.si, qymcad_ui_state::corner_tool(chamfer, sk.tool_prefs), sk.tool_prefs.fillet_by);
        let refuse = |key: &str| {
            let field = sk.corner.fields.iter().find(|p| p.key == key).map(qymcad_ui_state::field_caption).unwrap_or_default();
            Gate::Refused(qymcad_ui_state::Refusal { key: key.to_string(), field, why: qymcad_i18n::tr("sk-fillet-too-big") })
        };
        if let Some(v) = sk.corner.value("r") {
            if limit.is_some_and(|l| if chamfer { v >= l * (1.0 - 1e-9) } else { v > l * (1.0 + 1e-9) }) {
                return refuse("r");
            }
        }
        let angle = sk.tool_prefs.chamfer_mode == qymcad_core::feature::ChamferMode::DistAngle;
        if self.second_used() && angle && sk.corner.value("r2").is_some_and(|v| v.to_radians() >= std::f64::consts::PI) {
            return refuse("r2");
        }
        Gate::Open
    }
}

impl FramedTool for CornerEdit<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        self.fields_now();
        let gate = self.gate();
        let second = self.second_used();
        let sk = &mut *self.sk;
        let bar = bar_under(sk, &["sk_fillet", "sk_chamfer2"]);
        let standing = sk.corner.set.standing();
        let corner = if self.at.chamfer == Cut::Chamfer { Corner::Chamfer } else { Corner::Fillet };
        let name = if corner == Corner::Chamfer { "tool-chamfer" } else { "tool-fillet" };
        let rect = sk.canvas;
        // A VALUE THE CORNER CANNOT BE CUT BY draws no preview, and the box stands by the point of the corner then, as on
        // main: gone from under the keys, it took the value typed and the Enter that was to refuse it
        let spot = corner_box_spot(&corner_ctx(sk), rect, self.at.si, standing.first().copied(), corner == Corner::Chamfer).or_else(|| {
            let (pid, _) = standing.first().copied()?;
            let at = qymcad_ui_state::Sheet { view: *sk.view, rect }.at(qymcad_ui_state::sketch_pt(&*sk.project, self.at.si, pid)?);
            Some(BoxSpot { at, around: vec![at] })
        });
        let Some(spot) = spot else {
            return Described { head: head(ph::BEZIER_CURVE, name), bar, gate, says: Vec::new(), boxed: None };
        };
        // THE BOX OF A CORNER STANDS CLEAR OF THE CORNER, its cut ends and its arc - not of the whole sketch, which leaves it
        // no side in a narrow view and pulled it over the corner (as `corner_box_place` on main stood off the corner only)
        let mut spec = box_by(spot.at, &spot.around, &mut sk.corner.focus);
        spec.place = qymcad_ui_state::Place::close_by(qymcad_ui_state::sheet_spot(spot.at, &spot.around));
        if !second {
            spec.hidden = vec!["r2"];
        }
        // THE VALUE OF A CORNER IS A VALUE OF ONE DESCRIPTION: its formula, its unit, what it comes to - or why the
        // corners refuse it - said under it
        let angle = sk.tool_prefs.chamfer_mode == qymcad_core::feature::ChamferMode::DistAngle;
        let row = |p: &qymcad_ui_state::CmdParam, unit: &str| qymcad_ui_state::Row::new(qymcad_ui_state::field_caption(p)).field(&p.key).unit(unit);
        let rows: Vec<qymcad_ui_state::Row> = sk.corner.fields.iter().map(|p| row(p, if p.key == "r2" && angle { "unit-deg-suffix" } else { "sk-mm" })).collect();
        // HOW MUCH IS IN HAND, said in the box: a person typing one value for four corners has to know that it is four,
        // and the one answer that cuts them is one Enter
        let set = if corner == Corner::Chamfer { "sk-corner-set-chamfer-n" } else { "sk-corner-set-n" };
        let foot = vec![Opt::Readout(qymcad_i18n::tr1(set, "n", &standing.len().to_string()))];
        spec.layout = qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(rows)], foot };
        spec.names = sk.corner.fields.iter().map(|p| qymcad_ui_state::FieldName::of(&p.key, String::new(), None)).collect();
        Described { head: head(ph::BEZIER_CURVE, name), bar, gate, says: Vec::new(), boxed: Some(spec) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.corner.fields
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    /// A refused Enter is said on the status line of the sketch too, as the boxes of a sketch said it on main.
    fn refused(&mut self, words: &Notice) {
        *self.sk.status = words.clone();
    }

    fn took(&mut self, did: Did) {
        opt_took(self.sk, did);
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    /// THE SET CUT, AND THE VALUE NAMED: the first dimension the cut lays of the value takes its name, every other one
    /// reads it - one name drives the whole set, as one value cut it.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let CornerAt { si, pid, chamfer } = self.at;
        let asks = self.asks.clone();
        let values: Vec<qymcad_ui_state::CmdParam> = self.sk.corner.fields.clone();
        let before = self.sk.project.sketches[si].constraints.len();
        corner_apply(&mut corner_ctx(self.sk), CornerOf { si, pid, chamfer: chamfer == Cut::Chamfer })?;
        for p in &values {
            if let Some(ask) = asks.iter().find(|a| a.field == p.key) {
                name_what_was_laid(self.sk, Laying { si, since: before, value: p.val.abs() }, &ask.name);
            }
        }
        Ok(())
    }

    /// The lines cut at a crossing for a corner never cut are whole again, and the set goes.
    fn cancel(&mut self) {
        let sk = &mut *self.sk;
        if sk.corner.crossing != qymcad_ui_state::CrossingCut::None {
            qymcad_ui_state::take_back_last_step(&mut *sk.edits, &mut *sk.project, &mut *sk.regen);
        }
        sk.corner.clear();
    }

    /// The set goes and the corner tool stays for the next corner; the next Esc puts it down.
    fn back(&mut self) -> Stepped {
        self.cancel();
        Stepped::Back
    }

    /// ONE Esc ENDS THE BOX, its field holding the keyboard or not: the set goes and the corner tool stays, as Esc does
    /// with the field let go.
    fn field_did(&mut self, did: qymcad_ui_state::FieldDid) {
        if did == qymcad_ui_state::FieldDid::LeftByEsc {
            self.back();
        }
    }

    /// ONE VALUE IN TWO PLACES: the values at the corners are the bar's too, so the next corner offers the same, and the
    /// preview follows what is typed.
    fn after(&mut self) {
        let sk = &mut *self.sk;
        let vars = sk.project.param_map();
        for p in &sk.corner.fields {
            let Ok(v) = qymcad_ui_state::field_value(p, &vars) else { continue };
            if p.key == "r" {
                sk.tool_prefs.fillet = v;
            } else {
                sk.tool_prefs.chamfer_second = v;
            }
        }
    }
}

/// THE DIMENSIONS ONE VALUE OF A TOOL HAS JUST LAID: in sketch `si`, among the constraints from `since` on, those of
/// `value`.
pub(crate) struct Laying {
    pub(crate) si: usize,
    pub(crate) since: usize,
    pub(crate) value: f64,
}

/// ONE NAME DRIVES WHAT ONE VALUE LAID: the first dimension the value laid - the radius of the first corner of a set, the
/// distance of the first copy of an offset - takes name `nm`, every other one reads it, as one value made them all.
pub(crate) fn name_what_was_laid(sk: &mut qymcad_ui_state::SketchCtx, at: Laying, nm: &str) {
    let nm = nm.trim().to_string();
    if nm.is_empty() {
        return;
    }
    let Laying { si, since, value } = at;
    let laid: Vec<usize> =
        (since..sk.project.sketches[si].constraints.len()).filter(|&ci| sk.project.sketches[si].constraints[ci].dim_value().is_some_and(|v| (v.abs() - value).abs() < 1e-6)).collect();
    let Some(&first) = laid.first() else { return };
    name_dimension(sk, Dim { si, ci: first }, &nm);
    for &ci in &laid[1..] {
        if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
            *e = nm.clone();
        }
    }
}

/// WHERE THE BOX OF THE SET STANDS: by its first corner, clear of the corner, the two ends the lines are cut at and the
/// arc between them, so they stay in sight. It stood on the place clicked, which is on the corner or on one of its
/// lines: measured on a rectangle 40 x 30, the box covered the corner, both cut ends and the middle of the arc.
pub(crate) struct BoxSpot {
    pub(crate) at: Pos2,
    pub(crate) around: Vec<Pos2>,
}

// ---------------------------------------------------------------------------------------------------------------------
// THE DISTANCE OF AN OFFSET

/// AN OFFSET WHOSE SIDE A CLICK HAS FIXED: its sketch and where the click fixed the side.
#[derive(Clone, Copy)]
pub(crate) struct OffsetAt {
    si: usize,
    at: Point2,
}

/// THE BOX OF THE DISTANCE OF AN OFFSET whose side a click has fixed: beside the copy, the distance of the bar in it
/// and the keyboard given to it. What is typed is the bar's distance too, and the preview follows it; Enter makes the
/// copy, Esc frees the side and the copy follows the pointer again.
struct OffsetEdit<'a, 'b> {
    sk: &'a mut qymcad_ui_state::SketchCtx<'b>,
    at: OffsetAt,
    asks: Vec<qymcad_ui_state::FieldAsk>,
}

impl FramedTool for OffsetEdit<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let sk = &mut *self.sk;
        let OffsetAt { si, at } = self.at;
        let bar = bar_under(sk, &["sk_offset"]);
        if sk.sel_sk.offset.field.is_empty() {
            let mut d = qymcad_ui_state::CmdParam::new("opt-distance", "d", sk.tool_prefs.offset, -10000.0, 10000.0).nonzero();
            d.txt = qymcad_core::expr::fmt_num(sk.tool_prefs.offset);
            sk.sel_sk.offset.field = vec![d];
        }
        // the box stands off the point of the copy nearest the click, on the side away from the source
        let eids = offset_curves(sk.sel_sk);
        let dist = sk.project.offset_toward(si, &eids, sk.tool_prefs.offset, at);
        let near = sk.project.offset_preview(si, &eids, dist).into_iter().flatten().min_by(|a, b| a.dist(at).total_cmp(&b.dist(at))).unwrap_or(at);
        let sheet = qymcad_ui_state::Sheet { view: *sk.view, rect: sk.canvas };
        let spot = sheet.at(near);
        let mut boxed = box_by(spot, &[spot, sheet.at(at)], &mut sk.sel_sk.offset.focus);
        let row = qymcad_ui_state::Row::new(qymcad_i18n::tr("opt-distance")).field("d").unit("sk-mm");
        boxed.layout = qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(vec![row])], foot: Vec::new() };
        boxed.names = vec![qymcad_ui_state::FieldName::of("d", String::new(), None)];
        Described { head: head(ph::COPY, "tool-offset"), bar, gate: Gate::Open, says: Vec::new(), boxed: Some(boxed) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.sel_sk.offset.field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    /// A refused Enter is said on the status line of the sketch too, as the boxes of a sketch said it on main.
    fn refused(&mut self, words: &Notice) {
        *self.sk.status = words.clone();
    }

    /// The copy made, and the distance named: the first dimension the copy lays takes the name, the others read it.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let sk = &mut *self.sk;
        let d = sk.tool_prefs.offset;
        let name = self.asks.first().map(|a| a.name.clone()).unwrap_or_default();
        let before = sk.project.sketches[self.at.si].constraints.len();
        offset_make(
            OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
            self.at.si,
            d,
            Some(self.at.at),
        );
        name_what_was_laid(sk, Laying { si: self.at.si, since: before, value: d.abs() }, &name);
        Ok(())
    }

    fn take_asks(&mut self, asks: Vec<qymcad_ui_state::FieldAsk>) {
        self.asks = asks;
    }

    /// The side is freed, and the copy follows the pointer again.
    fn cancel(&mut self) {
        self.sk.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *self.sk.status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
    }

    /// The side is freed and the offset stays in hand; the next Esc puts it down.
    fn back(&mut self) -> Stepped {
        self.cancel();
        Stepped::Back
    }

    /// ONE Esc FREES THE SIDE, its field holding the keyboard or not, as Esc does with the field let go.
    fn field_did(&mut self, did: qymcad_ui_state::FieldDid) {
        if did == qymcad_ui_state::FieldDid::LeftByEsc {
            self.back();
        }
    }

    /// The distance typed is the bar's, and the preview follows it.
    fn after(&mut self) {
        let sk = &mut *self.sk;
        let vars = sk.project.param_map();
        if let Some(v) = sk.sel_sk.offset.field.first().and_then(|p| qymcad_ui_state::field_value(p, &vars).ok()) {
            sk.tool_prefs.offset = v;
        }
    }
}
