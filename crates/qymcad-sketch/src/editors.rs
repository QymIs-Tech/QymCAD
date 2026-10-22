//! THE EDITORS AT THE GEOMETRY of a sketch: the value of a dimension, the size of a circle or an arc, the value of the
//! corners of a set, the distance of an offset - each a tool the frame drives (`qymcad_ui_state::tool_in_hand`), with its
//! box at the geometry, its Enter and its Esc, opened by a double click or by a click at a corner.

use qymcad_core::model::Constraint;
use qymcad_ui_state::{Described, Did, Ends, FramedTool, Gate, Head, NotApplied, Notice, Opt, OptKey, Stepped};
use super::*;

/// WHICH EDITOR IS OPEN on the sketch, if one is: a dimension, a circle or an arc, the corners of a set, an offset.
pub(crate) enum Editor {
    Dim(Dim),
    Round(Round),
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
    if let Some(eid) = sk.inline.circle() {
        if round_now(sk, si, eid).is_some() {
            return Some(Editor::Round(Round { si, eid }));
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
        Editor::Dim(dim) => Box::new(DimEdit { sk, dim }),
        Editor::Round(at) => Box::new(RoundEdit { sk, at }),
        Editor::Corner(at) => Box::new(CornerEdit { sk, at }),
        Editor::Offset(at) => Box::new(OffsetEdit { sk, at }),
    }
}

/// The box of a dimension or of a circle put away, and nothing typed into it kept.
fn put_away(sk: &mut qymcad_ui_state::SketchCtx) {
    sk.inline.clear();
    sk.dim.put_away();
}

/// The head of an editor: its name by its catalogue key, Apply and Cancel.
fn head(icon: &'static str, name: &str) -> Head {
    Head { icon, name: qymcad_i18n::tr(name), ends: Ends::ApplyCancel }
}

/// A box standing by `at` on the sheet, clear of `around`.
fn box_by(at: Pos2, around: &[Pos2], focus: &mut bool) -> qymcad_ui_state::BoxSpec {
    let mut spec = qymcad_ui_state::BoxSpec::by(qymcad_ui_state::Place::by(qymcad_ui_state::sheet_spot(at, around)));
    spec.focus = qymcad_ui_state::Focus::take(focus);
    spec
}

// ---------------------------------------------------------------------------------------------------------------------
// THE SIZE OF A CIRCLE OR AN ARC

/// A CIRCLE OR AN ARC IN ITS BOX: its sketch and the entity.
#[derive(Clone, Copy)]
pub(crate) struct Round {
    si: usize,
    eid: Id,
}

/// IS A ROUND ENTITY AN ARC: it is sized by its radius then, a circle by its diameter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arc {
    Yes,
    No,
}

/// A ROUND ENTITY AS IT STANDS: its centre and radius, and whether it is an arc.
struct RoundNow {
    centre: Point2,
    r: f64,
    arc: Arc,
}

/// The circle or arc `eid` of sketch `si` as it stands; none when it went.
fn round_now(sk: &qymcad_ui_state::SketchCtx, si: usize, eid: Id) -> Option<RoundNow> {
    let s = sk.project.sketches.get(si)?;
    let e = s.entity(eid)?;
    let k = qymcad_core::model::circle_of(s, e)?;
    let centre = qymcad_ui_state::sketch_pt(&*sk.project, si, k.center)?;
    Some(RoundNow { centre, r: k.r, arc: if e.kind.as_arc().is_some() { Arc::Yes } else { Arc::No } })
}

/// The field of the size of a circle - its diameter - or of an arc - its radius.
fn size_field(arc: Arc, r: f64) -> qymcad_ui_state::CmdParam {
    let label = if arc == Arc::Yes { "sk-radius" } else { "sk-diameter" };
    let v = if arc == Arc::Yes { r } else { 2.0 * r };
    let mut p = qymcad_ui_state::CmdParam::new(label, "size", v, 0.0, MAX_SKETCH_LENGTH).nonzero();
    // THROUGH THE COMMON DOOR: no longer than four digits, with no tail. A raw `format!("{}")` printed the whole truth
    // about an f64 - "12.750000000000002" in the radius field.
    p.txt = qymcad_core::expr::fmt_num(v);
    p
}

/// THE BOX OF THE SIZE OF A CIRCLE OR AN ARC, beside it.
///
/// APPLIED ON ENTER, NOT ON EVERY LETTER, as a linear dimension is. Reported behaviour: "you place a dimension, try to
/// type it in, and while you are erasing, 100 becomes 1 - and at that moment the geometry gets thrown about". Erasing
/// "100" rebuilt the circle at 10 and at 1 on the way. Only a value typed is laid as a dimension.
struct RoundEdit<'a, 'b> {
    sk: &'a mut qymcad_ui_state::SketchCtx<'b>,
    at: Round,
}

impl FramedTool for RoundEdit<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let sk = &mut *self.sk;
        let Some(now) = round_now(sk, self.at.si, self.at.eid) else {
            return Described { head: head(ph::CIRCLE, "sk-diameter"), bar: Vec::new(), gate: Gate::Open, says: Vec::new(), boxed: None };
        };
        // the box opened again on another circle, or on the same one, starts from what is drawn
        if sk.dim.focus || sk.dim.field.is_empty() {
            sk.dim.field = vec![size_field(now.arc, now.r)];
            sk.dim.typed = false;
        }
        let sheet = qymcad_ui_state::Sheet { view: *sk.view, rect: sk.canvas };
        let c = now.centre;
        let around = [sheet.at(Point2::new(c.x - now.r, c.y - now.r)), sheet.at(Point2::new(c.x + now.r, c.y + now.r))];
        let boxed = box_by(sheet.at(Point2::new(c.x + now.r, c.y)), &around, &mut sk.dim.focus);
        let name = if now.arc == Arc::Yes { "sk-radius" } else { "sk-diameter" };
        Described { head: head(ph::CIRCLE, name), bar: Vec::new(), gate: Gate::Open, says: Vec::new(), boxed: Some(boxed) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.dim.field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        let typed = self.sk.dim.typed;
        let v = self.sk.dim.field.first().map(|p| p.val);
        let arc = round_now(self.sk, self.at.si, self.at.eid).map(|n| n.arc);
        put_away(self.sk);
        if let Some(v) = v.filter(|_| typed) {
            if let Some(arc) = arc {
                set_round_size(self.sk, self.at, if arc == Arc::Yes { v } else { v * 0.5 });
            }
        }
        Ok(())
    }

    fn cancel(&mut self) {
        put_away(self.sk);
    }

    fn back(&mut self) -> Stepped {
        self.cancel();
        Stepped::Back
    }

    /// A value typed is remembered as typed - the size it held typed again included: only a size typed is laid as a
    /// dimension.
    fn field_did(&mut self, did: qymcad_ui_state::FieldDid) {
        match did {
            qymcad_ui_state::FieldDid::Typed => self.sk.dim.typed = true,
            qymcad_ui_state::FieldDid::LeftByEsc => self.cancel(),
        }
    }
}

/// THE SIZE TYPED INTO THE BOX OF A CIRCLE OR AN ARC, laid as its dimension: one step of undo, named as a dimension is.
fn set_round_size(sk: &mut qymcad_ui_state::SketchCtx, at: Round, rr: f64) {
    let Round { si, eid } = at;
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
/// The choice between the radius and the diameter of a dimension of a circle.
const ROUND: OptKey = OptKey::of("dim-radius-or-diameter");
/// The near or the far edge of a circle a tangent gap is measured to.
const EDGE: OptKey = OptKey::of("dim-edge");
/// A reference dimension, which shows its value and drives nothing.
const DRIVEN: OptKey = OptKey::of("dim-driven");
/// The name the formulas of the document read the dimension by.
pub const DIM_NAME: OptKey = OptKey::of("dim-name");

/// A DIMENSION IN ITS BOX: its sketch and its place among the constraints of the sketch.
#[derive(Clone, Copy)]
pub(crate) struct Dim {
    si: usize,
    ci: usize,
}

/// WHAT KIND OF A DIMENSION A BOX HOLDS, as far as the box goes: the caption of its value and what it is switched
/// between.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Length,
    Angle,
    /// Of a circle, shown as its radius or its diameter.
    Round(Shown),
    /// A tangent gap to a circle, to its near or its far edge.
    Edge,
}

/// IS A DIMENSION OF A CIRCLE SHOWN AS ITS RADIUS OR ITS DIAMETER.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shown {
    Radius,
    Diameter,
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
fn row_caption(kind: Kind) -> String {
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
            sk.dim.name = None;
        }
        let target = target_of(sk, si, &c);
        let gate = value_gate(sk.project, &c, target.as_ref(), &sk.dim.field);
        let mut spec = box_by(at, &[at], &mut sk.dim.focus);
        spec.layout = dim_layout(sk, &c, kind, target.as_ref());
        if c.is_driven() {
            spec.hidden = vec![VALUE];
        }
        Described { head, bar: Vec::new(), gate, says: Vec::new(), boxed: Some(spec) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.dim.field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    fn took(&mut self, did: Did) {
        let sk = &mut *self.sk;
        let asked = match did {
            Did::Chose { key, .. } if key == ROUND => Asked { round: Ask::Yes, ..Asked::default() },
            Did::Pressed { key } if key == EDGE => Asked { edge: Ask::Yes, ..Asked::default() },
            Did::Switched { key } if key == DRIVEN => Asked { driven: Ask::Yes, ..Asked::default() },
            Did::Retyped { key, text } if key == DIM_NAME => {
                sk.dim.name = Some(text);
                return;
            }
            Did::Entered { key, text } if key == DIM_NAME => {
                name_dimension(sk, self.dim, &text);
                return;
            }
            _ => return,
        };
        // a switch carries no value, so nothing of it is refused; a refusal would still be said
        if let Err(key) = change_dimension(sk, self.dim, asked) {
            *sk.status = Notice::refusal(qymcad_i18n::tr(key));
        }
        // the field shows the dimension anew: a diameter switched to a radius, an edge taken, a reference made
        let Dim { si, ci } = self.dim;
        sk.dim.field = sk.project.sketches[si].constraints.get(ci).and_then(|c| kind_of(c).map(|k| vec![value_field(c, k)])).unwrap_or_default();
    }

    /// The value typed goes in - refused with its words when the dimension cannot take it, the box staying open on what
    /// was typed - and the box goes.
    fn apply(&mut self) -> Result<(), NotApplied> {
        let sk = &mut *self.sk;
        if let Some(c) = constraint_of(sk, self.dim).filter(|c| !c.is_driven()) {
            let typed = sk.dim.field.first().filter(|p| p.txt.trim() != shown_text(&c).trim()).map(|p| Value { number: p.val, text: p.txt.trim().to_string() });
            if let Some(value) = typed {
                let target = target_of(sk, self.dim.si, &c);
                if let Some(words) = value_gate(sk.project, &c, target.as_ref(), &sk.dim.field).answer() {
                    return Err(NotApplied::Refused(words.words));
                }
                if let Err(key) = change_dimension(sk, self.dim, Asked { value: Some(value), ..Asked::default() }) {
                    return Err(NotApplied::Refused(qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0))));
                }
            }
        }
        self.cancel();
        Ok(())
    }

    fn cancel(&mut self) {
        put_away(self.sk);
        self.sk.dim.fresh = None;
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

/// THE ROWS OF THE BOX OF A DIMENSION: the value with what it is switched between, and the name of a driver.
///
/// A DRIVER BELONGS TO ANY DIMENSION, NOT ONLY TO A LINEAR ONE: an angle, a diameter, a distance to a line, an arc
/// length or a tangent gap is named as well, a driver being told by the set of entities it sizes.
fn dim_layout(sk: &qymcad_ui_state::SketchCtx, c: &Constraint, kind: Kind, target: Option<&qymcad_core::model::DimTarget>) -> qymcad_ui_state::Layout {
    let mut value = qymcad_ui_state::Row::new(row_caption(kind)).field(VALUE);
    if kind != Kind::Angle {
        value = value.unit("sk-mm");
    }
    if c.is_driven() {
        value = value.opt(Opt::Readout(qymcad_i18n::tr1("sk-driven-value", "v", &qymcad_i18n::num(shown_value(c), 3))));
    }
    if let Kind::Round(shown) = kind {
        let way = |key: &str| qymcad_ui_state::Mode::new(qymcad_i18n::tr(key)).hint(qymcad_i18n::tr("sk-toggle-rad-dia"));
        value = value.opt(Opt::Modes { key: ROUND, caption: None, choices: vec![way("sk-radius"), way("sk-diameter")], current: Some(usize::from(shown == Shown::Diameter)) });
    }
    if kind == Kind::Edge {
        value = value.opt(Opt::press(EDGE, qymcad_i18n::tr("sk-edge-toggle")).hint(qymcad_i18n::tr("sk-near-far-edge")));
    }
    let lit = if c.is_driven() { qymcad_ui_state::Lit::On } else { qymcad_ui_state::Lit::Off };
    value = value.opt(Opt::switch(DRIVEN, qymcad_i18n::tr("sk-ref-short"), lit).hint(qymcad_i18n::tr("sk-driven-hint")));
    let mut rows = vec![value];
    if let Some(target) = target {
        let had = qymcad_i18n::name(&sk.project.name_of_target(target));
        let typed = sk.dim.name.clone().unwrap_or(had);
        let unfit = name_unfit(sk.project, target, typed.trim());
        let text = Opt::Text { key: DIM_NAME, text: typed, placeholder: qymcad_i18n::tr("sk-name-placeholder"), unfit, hint: Some(qymcad_i18n::tr("sk-driver-name-hint")) };
        rows.push(qymcad_ui_state::Row::new(qymcad_i18n::tr("sk-driver-label")).opt(text));
    }
    qymcad_ui_state::Layout { sections: vec![qymcad_ui_state::Section::plain(rows)], foot: Vec::new() }
}

/// WHY NAME `nm` WILL NOT DO for the dimension `target`, none when it will. An empty name is legitimate: the dimension
/// simply stops being a driver.
///
/// A TAKEN NAME IS SAID WITH WHOSE IT IS, not merely "taken": otherwise the namesake has to be hunted for across the
/// whole project by hand. A name with no owner is unusable in itself, and the line and its hint say why - the hint said
/// "taken" over `tau`, which nobody holds. A name that would close a circle of formulas says the chain.
fn name_unfit(project: &Project, target: &qymcad_core::model::DimTarget, nm: &str) -> Option<qymcad_ui_state::Unfit> {
    let circle = project.naming_closes_circle(target, nm);
    let taken = project.driver_name_taken_by(nm, target);
    if nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !taken && circle.is_none()) {
        return None;
    }
    let owner = project.name_owner(nm).filter(|_| taken);
    let unfit = match circle {
        Some(chain) => qymcad_i18n::error_words::expr_error_text(&qymcad_core::errors::ExprError::LeansOnItself(chain)),
        None => qymcad_i18n::error_words::ident_refusal_text(nm),
    };
    Some(match owner {
        Some(o) => qymcad_ui_state::Unfit {
            said: qymcad_i18n::tr2("par-name-taken", "name", "", "where", &if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { qymcad_ui_state::path_in_words(&o.path) }),
            hint: qymcad_i18n::tr("sk-driver-name-taken-hint"),
        },
        None => qymcad_ui_state::Unfit { said: unfit.clone(), hint: unfit },
    })
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
struct Value {
    number: f64,
    text: String,
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
        // THE ONE BARE PARSE OF THE SKETCHER, asked once: is this a number or a formula? The answer decides whether the
        // formula is kept beside the value.
        let as_number = v.text.parse::<f64>().ok();
        match set_dim_value(&mut *sk.project, si, ci, v.number) {
            Ok(()) => {
                if as_number.is_none() {
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
    let Some(refs) = Project::dim_refs(&c) else { return };
    let Some(target) = target_of(sk, dim.si, &c) else { return };
    let sid = sk.project.sketches[dim.si].id;
    let nm = typed.trim().to_string();
    let old = qymcad_i18n::name(&sk.project.name_of_target(&target)).trim().to_string();
    if nm == old || name_unfit(sk.project, &target, &nm).is_some() {
        return;
    }
    sk.dim.name = None;
    let mut ed = qymcad_ui_state::edit_over(sk.rebuild(), qymcad_i18n::tr("sk-driver-step"));
    if !old.is_empty() && !nm.is_empty() {
        // RENAMING CARRIES THE FORMULAS WITH IT - otherwise the expressions stay pointed at a name that no longer
        // exists, and the model breaks silently.
        let _ = ed.project().rename_driver(&old, &nm);
    } else {
        ed.project().add_named_dim(nm.clone(), sid, refs);
    }
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
        let standing = sk.corner.set.standing();
        let corner = if self.at.chamfer == Cut::Chamfer { Corner::Chamfer } else { Corner::Fillet };
        let name = if corner == Corner::Chamfer { "tool-chamfer" } else { "tool-fillet" };
        let rect = sk.canvas;
        let spot = corner_box_spot(&corner_ctx(sk), rect, self.at.si, standing.first().copied(), corner == Corner::Chamfer);
        let Some(spot) = spot else {
            return Described { head: head(ph::BEZIER_CURVE, name), bar: vec![corner_ways(sk, corner)], gate, says: Vec::new(), boxed: None };
        };
        let mut spec = box_by(spot.at, &spot.around, &mut sk.corner.focus);
        if !second {
            spec.hidden = vec!["r2"];
        }
        // HOW MUCH IS IN HAND, said in the box: a person typing one value for four corners has to know that it is four,
        // and the one answer that cuts them is one Enter
        spec.layout.foot = vec![Opt::Readout(qymcad_i18n::tr1("sk-corner-set-n", "n", &standing.len().to_string()))];
        Described { head: head(ph::BEZIER_CURVE, name), bar: vec![corner_ways(sk, corner)], gate, says: Vec::new(), boxed: Some(spec) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.corner.fields
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    fn took(&mut self, did: Did) {
        opt_took(self.sk, did);
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        let CornerAt { si, pid, chamfer } = self.at;
        corner_apply(&mut corner_ctx(self.sk), CornerOf { si, pid, chamfer: chamfer == Cut::Chamfer })
    }

    /// The lines cut at a crossing for a corner never cut are whole again, and the set goes.
    fn cancel(&mut self) {
        let sk = &mut *self.sk;
        if sk.corner.crossing != qymcad_ui_state::CrossingCut::None {
            qymcad_ui_state::take_back_last_step(&mut *sk.edits, &mut *sk.project, &mut *sk.regen);
        }
        sk.corner.clear();
    }

    fn back(&mut self) -> Stepped {
        self.cancel();
        escape_step(self.sk);
        Stepped::Back
    }

    /// ONE Esc PUTS THE TOOL DOWN, its field holding the keyboard or not: the set goes and the tool steps back, as Esc
    /// does with the field let go.
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
}

impl FramedTool for OffsetEdit<'_, '_> {
    fn describe(&mut self, _ctx: &egui::Context) -> Described {
        let sk = &mut *self.sk;
        let OffsetAt { si, at } = self.at;
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
        let boxed = box_by(spot, &[spot, sheet.at(at)], &mut sk.sel_sk.offset.focus);
        Described { head: head(ph::COPY, "tool-offset"), bar: Vec::new(), gate: Gate::Open, says: Vec::new(), boxed: Some(boxed) }
    }

    fn fields(&mut self) -> &mut Vec<qymcad_ui_state::CmdParam> {
        &mut self.sk.sel_sk.offset.field
    }

    fn look(&self) -> qymcad_ui_state::DrawCtx<'_> {
        self.sk.draw()
    }

    fn apply(&mut self) -> Result<(), NotApplied> {
        let sk = &mut *self.sk;
        let d = sk.tool_prefs.offset;
        offset_make(
            OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
            self.at.si,
            d,
            Some(self.at.at),
        );
        Ok(())
    }

    /// The side is freed, and the copy follows the pointer again.
    fn cancel(&mut self) {
        self.sk.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *self.sk.status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
    }

    fn back(&mut self) -> Stepped {
        self.cancel();
        escape_step(self.sk);
        Stepped::Back
    }

    /// ONE Esc PUTS THE TOOL DOWN, its field holding the keyboard or not: the set goes and the tool steps back, as Esc
    /// does with the field let go.
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
