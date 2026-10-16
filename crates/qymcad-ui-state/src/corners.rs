//! THE CORNER TOOLS OF A SKETCH: rounding and chamfering a corner, modifying, mirroring.

use crate::Notice;
use super::*;

/// Apply an edit operation to the selection. Returns true when it was applied.
pub fn try_modify(mut ed: Editing, sel_sk: &mut SketchSelection, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) -> bool {
    // THE BOUNDARY OF AN OPERATION: one step of undo, named after the tool
    let name = match op {
        EditTool::Delete => "sk-delete",
        EditTool::Mirror => "tool-mirror",
        EditTool::LinearPattern => "tool-lin-array",
        EditTool::CircularPattern => "tool-circ-array",
        EditTool::Fillet => "tool-fillet",
        EditTool::Chamfer => "tool-chamfer",
        EditTool::Offset => "tool-offset",
    };
    begin_edit(ed.edits, &*ed.project, qymcad_i18n::tr(name));
    let ok = try_modify_in(ed.reborrow(), sel_sk, sk_pat, tool_prefs, op);
    close_edit(ed.edits, ed.project);
    ok
}

pub(crate) fn try_modify_in(ed: Editing, sel_sk: &mut SketchSelection, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) -> bool {
    let Sel::Sketch(si) = *ed.sel else { return false };
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return false;
    }
    let ok = match op {
        EditTool::Delete => {
            ed.project.delete_entities(si, &eids);
            sel_sk.clear(); // the selection and whatever was waiting for it
            true
        }
        // THE MIRROR IS NOT APPLIED FROM A CLICK. It has two steps of its own (`modify_button` takes the
        // selection, `mirror_about_axis` or `mirror_about_line` finishes it), and applying here would mean
        // mirroring on the FIRST entity clicked - which is how several things could never be mirrored
        // together, and how a misclick became an edit.
        EditTool::Mirror => return false,
        EditTool::LinearPattern => {
            ed.project.array_linear(si, &eids, sk_pat.dx, sk_pat.dy, sk_pat.count);
            true
        }
        EditTool::CircularPattern => {
            let (cx, cy) = if let Some(p) = sel_point_ids(sel_sk).first().and_then(|id| sketch_pt(ed.project, si, *id)) { (p.x, p.y) } else { ed.project.entities_centroid(si, &eids) };
            ed.project.array_circular(si, &eids, cx, cy, sk_pat.count, sk_pat.angle);
            true
        }
        EditTool::Fillet => {
            eids.len() >= 2 && ed.project.fillet_lines_by(si, eids[0], eids[1], fillet_size(tool_prefs)) && {
                sel_sk.clear(); // the selection and whatever was waiting for it
                true
            }
        }
        EditTool::Chamfer => {
            eids.len() >= 2 && ed.project.chamfer_lines(si, eids[0], eids[1], chamfer_legs(tool_prefs)) && {
                sel_sk.clear(); // the selection and whatever was waiting for it
                true
            }
        }
        // the distance field refusing its value refuses the offset too: the last good distance is not what was typed
        EditTool::Offset if bar_field_bad("sk_offset") => {
            *ed.status = Notice::refusal(qymcad_i18n::tr("sk-offset-field-bad"));
            false
        }
        EditTool::Offset => ed.project.offset_entities(si, &eids, tool_prefs.offset) > 0,
    };
    if ok {
        invalidate(ed.regen);
    }
    ok
}

/// What the corner popup touches: its own input, the document, the sketch selection it clears, the rebuild
/// flag, the status line and the sticky radius offered next time. Six fields, and no more - which is the
/// point of writing them down.
pub struct CornerCtx<'a> {
    pub corner: &'a mut CornerInput,
    pub project: &'a mut qymcad_core::model::Project,
    /// The undo journal: a fillet or a chamfer of a corner is one step of it, named after its tool.
    pub edits: &'a mut Edits,
    pub sel_sk: &'a mut SketchSelection,
    /// The sheet the corner stands on: where the field was opened says which side of the point the corner is.
    pub view: &'a View2d,
    pub regen: &'a mut Rebuilding,
    pub status: &'a mut Notice,
    pub tool_prefs: &'a mut SketchToolPrefs,
    /// THE SCHEME IN HAND, and where every colour of the preview is read from. See `PopupLooks` for why a colour
    /// is not kept in the state of the tool.
    pub scheme: &'a SchemeUi,
    /// THE SETTINGS OF THE SHEET, and where the width of the zone of a point is read from: the zone is three times
    /// the reach of a pick of a point, and that reach is a setting of the person.
    pub set: &'a Settings,
    /// THE SKETCH OPEN FOR EDITING, or what is selected where there is none: the preview on the sheet is drawn
    /// whether or not the field is up, and before the first click there is no field and no corner to say which
    /// sketch - so the sheet the preview belongs to has to be reachable without the field.
    pub sel: &'a Sel,
    /// THE TOOL IN HAND, and so whether the corner under the cursor would be rounded or cut straight.
    pub armed: &'a Armed,
}

impl<'a> CornerCtx<'a> {
    /// WHAT THE CURSOR CATCHES, read the way the canvas reads it - the zone of a point that can name a corner is
    /// three times the reach of a pick of one, which is wider than any pick the canvas itself does.
    pub fn pick(&self) -> PickCtx<'_> {
        PickCtx { project: self.project, view: self.view, set: self.set }
    }
}

/// WHAT THE SIZE POPUPS OF THE SKETCH TOOLS TAKE FROM THE APPLICATION besides the drawing itself: the values the
/// tools remember between two corners, and the colours of the scheme in hand.
///
/// **A COLOUR IS NOT STATE OF A TOOL, AND THAT WAS THE FAULT.** The two colours of a corner preview were kept in
/// `CornerInput` and written by the one window that had a scheme to hand — so every other way of opening a corner
/// field left them at the transparent default of a `Color32` and drew nothing at all, and `CornerInput::clear()`
/// (which every applied corner and every mode change calls) took them away again. Reported: with the corner tool,
/// the lines of the preview were not drawn at all. The scheme is read where the drawing is done instead, on every
/// frame, so that every way of opening a field draws alike and a person who changes the scheme sees the change.
pub struct PopupLooks<'a> {
    /// THE VALUES THE TOOLS REMEMBER: the radius or the leg that was answered last, and the fillets of the bar.
    pub tool_prefs: &'a mut SketchToolPrefs,
    /// THE SCHEME IN HAND — the source of every colour drawn over the sheet.
    pub scheme: &'a SchemeUi,
    /// THE SETTINGS OF THE SHEET, and where the width of the zone of a point that names a corner is read from.
    pub set: &'a Settings,
}

/// THE STATES OF THE SKETCH THE POPUPS AT THE GEOMETRY WRITE: the corner being cut, the shape being placed, what
/// is picked in the drawing, and the tool in hand.
///
/// Four separate arguments that always travel together - the bar that carries a corner set and the box that
/// carries a shape are drawn in one go, from the same frame of the same panel.
pub struct PopupTools<'a> {
    /// The corner set and the value being typed for it.
    pub corner: &'a mut CornerInput,
    /// The shape being placed, and the size being typed for it.
    pub place: &'a mut Placing,
    /// What is picked in the drawing.
    pub sel_sk: &'a mut SketchSelection,
    /// The tool in hand: which of the corner tools the preview on the sheet belongs to.
    pub armed: &'a Armed,
}

/// What the three shape popups touch. All three edit the same thing - the shape being placed - through
/// named document operations, so they share one context rather than three alike.
pub struct PlaceCtx<'a> {
    pub place: &'a mut Placing,
    pub project: &'a mut qymcad_core::model::Project,
    pub view: &'a View2d,
    pub regen: &'a mut Rebuilding,
}

/// THE SAME FIELD, BUT IT TAKES FOCUS ITSELF AND SELECTS THE FORMER VALUE.
///
/// That is how the dimension popup opens: a dimension is clicked and a new number is typed straight
/// away, with no aiming of the mouse at the field and no erasing of the old one. This behaviour must
/// not be lost when moving onto the shared field.
pub fn expr_field_autofocus(ui: &mut egui::Ui, project: &Project, id: egui::Id, model: &str, w: f32, hint: &str, want_focus: bool) -> ExprOut {
    field(ui, project, id, model, w, hint, FieldRules { valid: &|_| true, list: NameList::Offered, focus: if want_focus { Focus::Takes } else { Focus::Waits } })
}

/// The editing button: with a ready selection it applies at once, otherwise it waits for one (Esc cancels).
pub fn modify_button(mut ed: Editing, t: &mut Tools, sk_pat: SketchPattern, tool_prefs: &SketchToolPrefs, op: EditTool) {
    // PRESSED AGAIN WITH THE TOOL IN HAND it is put down, as every tool button does - except the mirror with
    // geometry selected, where the press is the way on to the axis (see below)
    let held = t.sel_sk.modify == Some(op);
    let mirror_forward = op == EditTool::Mirror && t.sel_sk.items.iter().any(|(k, _)| *k == 1);
    exit_draw_tools(&mut t.reborrow());
    if held && !mirror_forward {
        return;
    }
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool: _ } = t;
    sel_sk.constraint = None;
    **armed = Armed::Modify(op);
    // THE MIRROR HAS TWO STEPS OF ITS OWN: what, then about what. Pressing the button with geometry
    // selected answers the first and asks the second; pressing it with a selection made while the tool was
    // already in hand does the same, which is why the button is the way forward rather than a stray click.
    if op == EditTool::Mirror {
        let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        if !eids.is_empty() {
            // THE SELECTION STAYS ON SCREEN. It used to be moved into the tool and cleared, so the person
            // saw nothing selected, could not tell what was about to be reflected, and read the next click
            // as "it mirrored everything". Reported behaviour: "the geometry simply loses its selection -
            // it has to stay highlighted so one can see it is still chosen."
            sel_sk.mirror_of = eids;
            sel_sk.modify = Some(op); // the tool in hand on its second step too, so a click or a box is read as its own
            *ed.status = Notice::info(qymcad_i18n::tr("sk-mirror-pick-axis"));
        } else {
            sel_sk.clear();
            sel_sk.modify = Some(op);
            *ed.status = Notice::info(qymcad_i18n::tr("sk-mirror-pick-what"));
        }
        return;
    }
    // THE OFFSET IS NOT MADE BY THE PRESS: what is chosen stays chosen, the copy is previewed on the side of the pointer,
    // and Enter makes it there (`qymcad_sketch::sketch_tool_keys`). Made at once, it had no side but the sign of the
    // distance, and a click on a line - one side of a polyline - offset that line alone before the rest was chosen.
    if op == EditTool::Offset {
        sel_sk.modify = Some(op);
        sel_sk.offset = OffsetAim::default(); // a side fixed for the copy before is not this copy's side
        let curves = sel_sk.items.iter().any(|(k, _)| *k == 1);
        // points chosen and no curve: said what the tool takes, not "pick the entities" as though nothing were chosen
        *ed.status = match (curves, sel_sk.items.is_empty()) {
            (true, _) => Notice::info(qymcad_i18n::tr("sk-offset-side")),
            (false, false) => Notice::refusal(qymcad_i18n::tr("sk-offset-takes")),
            (false, true) => Notice::info(qymcad_i18n::tr("g-pick-for-op")),
        };
        return;
    }
    let chosen = !sel_sk.items.is_empty();
    if try_modify(ed.reborrow(), sel_sk, sk_pat, tool_prefs, op) {
        sel_sk.modify = None;
    } else {
        sel_sk.clear(); // the selection, and whatever was waiting on it: pick anew, with nothing stuck from before
        sel_sk.modify = Some(op);
        // SOMETHING WAS CHOSEN AND NOTHING OF IT COULD BE OFFSET (points, construction): said so, and what the tool
        // takes. "Pick the entities" read as though nothing had been chosen, and the tool looked broken.
        *ed.status = if chosen && op == EditTool::Offset { Notice::refusal(qymcad_i18n::tr("sk-offset-takes")) } else { Notice::info(qymcad_i18n::tr("g-pick-for-op")) };
    }
}

/// MIRROR WHAT THE TOOL HOLDS ABOUT ONE OF THE SKETCH AXES (0 = X, 1 = Y).
///
/// The axes are drawn in every sketch and were, until now, the only drawn thing a person could not point
/// at: mirroring about Y happened silently when no line was selected, and X could not be asked for at all.
pub fn mirror_about_axis(ed: Editing, sel_sk: &mut SketchSelection, which: usize) {
    let along = if which == 0 { Point2::new(1.0, 0.0) } else { Point2::new(0.0, 1.0) };
    mirror_about(ed, sel_sk, MirrorAbout::Line(Point2::new(0.0, 0.0), along));
}

/// MIRROR WHAT THE TOOL HOLDS ABOUT A LINE OF THE SKETCH, given by its two points.
///
/// Construction geometry serves as well as ordinary geometry: what matters is that two points define a
/// line, and a construction line is exactly what people draw to mirror about.
pub fn mirror_about_line(ed: Editing, sel_sk: &mut SketchSelection, a: Id, b: Id) {
    let Sel::Sketch(si) = *ed.sel else { return };
    let (Some(pa), Some(pb)) = (sketch_pt(ed.project, si, a), sketch_pt(ed.project, si, b)) else { return };
    mirror_about(ed, sel_sk, MirrorAbout::Line(pa, pb));
}

/// MIRROR WHAT THE TOOL HOLDS ABOUT A POINT OF THE SKETCH: each point of it to the far side of that point, the same
/// distance off. Reported behaviour: "a plain click on something mirrors about it - a point, a line and so on".
pub fn mirror_about_point(ed: Editing, sel_sk: &mut SketchSelection, p: Id) {
    let Sel::Sketch(si) = *ed.sel else { return };
    let Some(at) = sketch_pt(ed.project, si, p) else { return };
    mirror_about(ed, sel_sk, MirrorAbout::Point(at));
}

/// WHAT THE MIRROR REFLECTS ABOUT: the straight line through two places, or a place.
#[derive(Clone, Copy)]
pub(crate) enum MirrorAbout {
    Line(Point2, Point2),
    Point(Point2),
}

/// The doors above meet here: reflect what is held - one step of undo, named after the tool.
pub(crate) fn mirror_about(mut ed: Editing, sel_sk: &mut SketchSelection, about: MirrorAbout) {
    begin_edit(ed.edits, &*ed.project, qymcad_i18n::tr("tool-mirror"));
    mirror_about_in(ed.reborrow(), sel_sk, about);
    close_edit(ed.edits, ed.project);
}

pub(crate) fn mirror_about_in(ed: Editing, sel_sk: &mut SketchSelection, about: MirrorAbout) {
    let Sel::Sketch(si) = *ed.sel else { return };
    // THE AXIS IS NOT MIRRORED WITH THE REST. It used to be: the line serving as the axis sat in the same
    // selection as everything else, so the tool reflected it onto itself along with the geometry.
    let eids: Vec<Id> = std::mem::take(&mut sel_sk.mirror_of);
    if eids.is_empty() {
        return;
    }
    match about {
        MirrorAbout::Line(a, b) => ed.project.mirror_entities(si, &eids, a.x, a.y, b.x, b.y),
        MirrorAbout::Point(p) => ed.project.mirror_about_point(si, &eids, p.x, p.y),
    }
    // THE TOOL STAYS IN HAND for the next geometry, its choice let go. Cleared with the selection, its mode went while
    // the bar went on saying Mirror, and the next click chose as the arrow does.
    sel_sk.clear();
    sel_sk.modify = Some(EditTool::Mirror);
    *ed.status = Notice::info(qymcad_i18n::tr("sk-mirror-done"));
}

/// THE MIRROR TAKES WHAT IS CHOSEN as what it reflects: the curves of the selection, which stays lit on screen. The
/// first click with the tool in hand and nothing held, a Shift + click and a box all choose through here.
pub fn mirror_takes_the_choice(sel_sk: &mut SketchSelection, status: &mut Notice) {
    sel_sk.mirror_of = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    *status = Notice::info(qymcad_i18n::tr(if sel_sk.mirror_of.is_empty() { "sk-mirror-pick-what" } else { "sk-mirror-pick-axis" }));
}

/// ESC FROM AN EDITING BUTTON THAT WORKS ON A CLICK, whatever it is: the tool goes down, and what it was working on
/// goes with it.
///
/// **THE CORNER TOOLS TAKE THEIR WHOLE SET WITH THEM.** The fillet and the chamfer work on a set of lines chosen with
/// Shift, and Esc puts that set down with the tool: what was lit goes dark, the corners drawn on it come off, and the
/// field stands no longer. It used to take the tool down alone - the lines stayed lit with their corners drawn on
/// them, a set that was on the screen and in no tool, and the next click landed in it.
///
/// The other editing buttons act on one click and keep nothing of their own, so for them Esc takes the button alone.
///
/// `None` means no editing button is in hand and the ladder should carry on to its next rung.
pub fn leave_editing_tool(t: &mut Tools, status: &mut Notice) -> Option<()> {
    let corner = match t.armed.click_op() {
        0 => return None,
        4 | 5 => true,
        _ => false,
    };
    // EXTEND HOLDING A LINE lets it go first and stays in hand, as a drawing tool drops its points before itself
    if t.armed.click_op() == 2 && t.tool.extend.take().is_some() {
        *status = Notice::info(qymcad_i18n::tr("opt-extend-hint"));
        return Some(());
    }
    *t.armed = Armed::None;
    if corner {
        t.corner.put_down();
        t.sel_sk.clear();
        *status = Notice::info(qymcad_i18n::tr("in-selection-cleared"));
    }
    Some(())
}

impl Tools<'_> {
    /// ESC FROM AN EDITING BUTTON THAT WORKS ON A CLICK, said as a question: was one in hand, and has it been put
    /// down with what it was working on. `leave_editing_tool` says what each of them takes with it; this is the same
    /// answer in the shape the ladder of cancellations reads.
    pub fn leave_click_tool(&mut self, status: &mut Notice) -> bool {
        leave_editing_tool(self, status).is_some()
    }
}

/// ESC WITH A CONSTRAINT OR AN EDITING TOOL WAITING FOR ITS CURVES. An offset whose side a click has fixed lets the side
/// go and stays in hand - the box of its distance closes and the copy follows the pointer again; a second Esc puts it
/// down. egui takes the keyboard from a field at an Escape before the frame is drawn, so the box never hears it: the
/// ladder did, and put the whole tool down from inside its box.
///
/// Otherwise the tool goes AND THE EDIT MODE WITH IT. Only the expected pick was extinguished while the mode stayed:
/// the tool bar went on saying "Mirror" and the button in the panel stayed pressed - switched off yet looking switched
/// on, and a click did nothing.
pub fn escape_waiting_tool(t: &mut Tools, status: &mut Notice) {
    if t.sel_sk.modify == Some(EditTool::Offset) && t.sel_sk.offset.at.is_some() {
        t.sel_sk.offset = OffsetAim::default();
        *status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
        return;
    }
    t.sel_sk.constraint = None;
    t.sel_sk.modify = None;
    t.sel_sk.offset = OffsetAim::default();
    t.sel_sk.mirror_of.clear();
    *t.armed = Armed::None;
}

/// PUT DOWN THE SKETCH TOOL THAT IS IN HAND, and say what to tell the person.
///
/// Reported behaviour: "Esc does not reset the Mirror tool to the default Select - the selection is lost
/// and the tool stays active with its bar at the top." Switched off yet looking switched on is the worst
/// kind of cancellation.
///
/// THE LADDER CARRIED A RUNG PER FAMILY and had none for the editing tools, so mirror, offset, fillet,
/// chamfer and the arrays were all unreleasable - the mirror was simply the one somebody pressed. A rung
/// per variant written out in the ladder is a list of THIS enum living somewhere else, and it falls behind
/// silently: a family nobody added is a tool Escape does not put down (D19).
///
/// `None` means nothing was in hand, and the ladder should carry on to its next rung.
pub fn release_armed_sketch_tool(t: &mut Tools) -> Option<&'static str> {
    let Tools { armed, dim, measure, sel_sk, tool, .. } = t.reborrow();
    let msg = match *armed {
        Armed::Dimension(_) => {
            dim.pick.clear();
            None
        }
        Armed::Draw(_) => None,
        Armed::Measure => {
            measure.clear();
            Some("in-measure-cancelled")
        }
        Armed::Modify(_) => {
            // the editing family: what it was waiting for goes with it, or the next click feeds a tool
            // that is no longer in hand
            sel_sk.modify = None;
            sel_sk.mirror_of.clear();
            Some("in-modify-cancelled")
        }
        _ => return None,
    };
    tool.pts.clear();
    *armed = Armed::None;
    Some(msg.unwrap_or("in-tool-released"))
}

/// OPEN THE LITTLE BOX OF THE CORNER: at a vertex, with the tool's value already in it.
///
/// The value at the corner is the one the bar carries, and it is carried on afterwards: the next corner offers the
/// same number. One place writes the box down, so the corner found by clicking the point and the corner found by
pub fn corner_reach(set: &Settings) -> f32 {
    grab::grab(set, grab::Grab::Corner)
}

/// THE CORNER AS A POINT IN THE DRAWING: where the field was opened, read on the sheet. It is the side of the point
/// the person pressed on, which is what says which corner of several is meant where more than two lines meet there.
pub fn corner_where(cc: &CornerCtx, rect: Rect) -> Option<(f64, f64)> {
    let at = cc.corner.pos?;
    let w = to_world(cc.view, rect, at);
    Some((w.x, w.y))
}
/// THE TWO EDGES THE CORNER IN THE FIELD IS, right now.
///
/// **A PAIR THE TWO LINES NAMED IS THE CORNER, and the cursor does not move it.** Two edges that share a point are
/// one corner of the drawing and the person named it by picking both of them: at a point where four lines meet,
/// letting the cursor choose a different pair of them put the arc where nobody had asked for it - it followed the
/// pointer to the far corner of the point and was cut there on Enter. That is what `pinned` says: the pair was picked,
/// not read off the cursor.
///
/// Where the pair was READ OFF THE CURSOR it is the cursor's to move while it is over the point - and the last one it
/// named stands while it is away, which is the whole of a field: a person typing the radius has the pointer in the
/// field, and the preview under the value must not jump back to another corner when the pointer leaves the point.
///
/// `through` narrows the question where the corner was named by an edge and a point together: then only the corners
/// that edge takes part in are among the answers, and the cursor says which of them.
pub fn corner_pair_now(project: &Project, si: usize, pid: Id, named: Option<(Id, Id)>, through: Option<Id>, pinned: bool, cursor: Option<(f64, f64)>) -> Option<(Id, Id)> {
    // THE PAIR IN FORCE STANDS WHILE THE CURSOR IS AWAY: it is the corner the field was opened for, and the value in
    // the field belongs to it. `through` has to be in the pair, or the corner would change its own subject.
    let stands = |pair: Option<(Id, Id)>| pair.filter(|p| project.corner_of_pair(si, p.0, p.1) == Some(pid) && through.is_none_or(|line| p.0 == line || p.1 == line));
    if let Some(line) = through {
        // THE CORNER IS ONE OF THIS LINE'S, so the pair is read off the side the cursor stands on
        return cursor.and_then(|c| project.vertex_pair_through(si, pid, line, Some(c))).or_else(|| stands(named)).or_else(|| project.vertex_pair_through(si, pid, line, None));
    }
    if pinned {
        return stands(named); // the two lines that were picked: the cursor has no word in it
    }
    if project.vertex_edges(si, pid).len() > 2 {
        if let Some(pair) = cursor.and_then(|c| project.vertex_pair(si, pid, Some(c))) {
            return Some(pair);
        }
    }
    stands(named).or_else(|| project.vertex_pair(si, pid, None))
}

/// THE CURSOR AS A SAY IN THE CORNER, and only while it stands near the point.
///
/// The cursor answers "which of the corners at this point" by the side of the point it is on, which is only worth
/// asking while it is ON the point: ten units away it is over some other part of the drawing, and taking its word for
/// it made the corner change as the pointer crossed the sheet, so the radius being typed was cut at a corner nobody
/// was looking at. `track_px` is the reach of the aim for the point itself, `Grab::Corner` - a shade wider than the
/// point is caught from, because a person who aimed there has the corner open and cannot move it.
///
/// `None` - no cursor over the sheet, or one out of reach - says nothing, and the pair stands as it was named.
pub fn corner_cursor(project: &Project, si: usize, pid: Id, cursor: Option<(f64, f64)>, track_px: f32, scale: f32) -> Option<(f64, f64)> {
    let c = cursor?;
    let s = project.sketches.get(si)?;
    let (px, py) = s.points.iter().find(|q| q.id == pid).map(|q| (q.x, q.y))?;
    (((c.0 - px).hypot(c.1 - py) * scale as f64) <= track_px as f64).then_some(c)
}

/// A CORNER THE SELECTED LINES HAVE MADE, and the point it stands at.
///
/// The two lines ARE the corner; the point is where it is. It is remembered rather than read off the selection
/// afresh every time: a line that leaves the selection takes its corner with it, and the line that stays does not
/// pair again with a neighbour of the one that left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCorner {
    pub point: Id,
    pub pair: (Id, Id),
}

/// A CORNER NAMED AT A POINT, where no chosen line arrives. One at a point at most, and it dies the moment a line
/// corner stands there in its place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointCorner {
    pub point: Id,
    pub pair: (Id, Id),
}

/// WHAT A CLICK AT A POINT DID, so that the words to say are chosen at the place the click was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointAct {
    /// NOTHING STANDS THERE: the click named no corner, and the selection of lines does not move either.
    Nothing,
    /// A CORNER WAS NAMED AT THE POINT, and is in the set.
    PointAdded,
    /// THE CORNER NAMED AT THE POINT WAS TAKEN AWAY, and the point is free for it again.
    PointRemoved,
    /// A CORNER OF THE LINES WAS PUT AWAY, not deleted: the same click brings it back.
    LineHidden,
    /// THE CORNER OF THE LINES THAT WAS PUT AWAY STANDS AGAIN.
    LineShown,
    /// THE POINT IS NOT FREE FOR A CORNER: a corner of the lines is standing there.
    TakenByLines,
}

/// WHAT THE SELECTED LINES HAVE MADE OF THE DRAWING: the corners that stand, the ones put away by a click at
/// their point, and the ones named at a point.
///
/// ONE RECORD, because the three are one thing: a change of the selection moves all of them at once, and a reader
/// holding two of the three is reading half a state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerSet {
    /// THE CORNERS THAT STAND, each with its two lines and the point between them.
    pub made: Vec<LineCorner>,
    /// THE CORNERS PUT AWAY BY A CLICK AT THEIR POINT, remembered as the pair of lines and NOT deleted: the same
    /// click at the same point brings that very corner back. A corner that is put away is not shown and not cut,
    /// and it stays that way as long as both its lines stand in the selection.
    pub hidden: Vec<(Id, Id)>,
    /// THE CORNERS NAMED AT A POINT, one at a point at most.
    pub points: Vec<PointCorner>,
}

impl CornerSet {
    /// NOTHING OF THE SET, which is what an applied answer and a refused one both leave behind.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// THE ONE CORNER NAMED AT A POINT FOLLOWS THE POINTER, where the point has more than one corner to give - a
    /// crossing cut into four, a T, a point several lines meet at: the corner is the one the sector of the pointer
    /// is, so the preview is drawn and the cut made on the side the pointer is on. A set of more corners than one is
    /// left as it was named: one pointer does not say a side for every point of a set. Answers whether the pointer
    /// steers the corner.
    pub fn steer(&mut self, project: &Project, si: usize, cursor: (f64, f64)) -> Steered {
        if self.choice(project, si) == Choice::One {
            return Steered::No;
        }
        let [one] = self.points.as_mut_slice() else { return Steered::No };
        if let Some(pair) = project.vertex_pair(si, one.point, Some(cursor)) {
            one.pair = pair;
        }
        Steered::Yes
    }

    /// WHETHER THE SET IS ONE CORNER NAMED AT A POINT THAT HAS MORE THAN ONE TO GIVE: a crossing cut into four, a T, a
    /// point the corners of several shapes meet at. Such a corner is chosen by a click on its side, not by the click that
    /// named the point.
    pub fn choice(&self, project: &Project, si: usize) -> Choice {
        let [one] = self.points.as_slice() else { return Choice::One };
        if !self.made.is_empty() {
            return Choice::One;
        }
        let corners = project.vertex_pairs(si, one.point).into_iter().filter(|&(a, b)| project.corner_of_pair(si, a, b).is_some()).count();
        if corners < 2 {
            Choice::One
        } else {
            Choice::Many
        }
    }

    /// THE CORNERS ONE ANSWER CUTS, each with the point it stands at: what stands, in the order it was made.
    pub fn standing(&self) -> Vec<(Id, (Id, Id))> {
        let mut out: Vec<(Id, (Id, Id))> = self.made.iter().map(|c| (c.point, c.pair)).collect();
        out.extend(self.points.iter().map(|c| (c.point, c.pair)));
        out
    }

    /// THE LINES THE SET IS CUT FROM, which is what the lines between the corners have to spare.
    pub fn lines(&self) -> std::collections::HashSet<Id> {
        self.standing().into_iter().flat_map(|(_, (a, b))| [a, b]).collect()
    }

    /// THE LINES THAT STAND IN A CORNER OF THE SET, put away or not: they are spoken for, and a line already in a
    /// corner makes no new one by arriving beside another at the same point.
    pub fn spoken_at_points(&self) -> Vec<Id> {
        self.made.iter().flat_map(|c| [c.pair.0, c.pair.1]).chain(self.points.iter().flat_map(|c| [c.pair.0, c.pair.1])).collect()
    }

    /// WHETHER ANY CORNER STANDS AT A POINT, put away or not. A point that has a corner shows no yellow preview
    /// and takes no second one: it is one place, and a second corner there is a second reading of it.
    pub fn occupied(&self, project: &Project, si: usize, pid: Id) -> bool {
        self.made.iter().any(|c| c.point == pid) || self.points.iter().any(|c| c.point == pid) || self.hidden.iter().any(|&(a, b)| project.corner_of_pair(si, a, b) == Some(pid))
    }

    /// THE CORNERS OF THE SELECTION, brought up to date.
    ///
    /// **A CORNER IS REMEMBERED, NOT READ OFF THE SELECTION EVERY TIME.** Two lines that met are a corner, and
    /// they stay one until one of them leaves: the line that stays does not pair again with a neighbour of the one
    /// that left. Reported, where it was read afresh: four lines through one point were chosen, then one of them
    /// was let go, and the two lines beside it made a corner nobody had asked for.
    ///
    /// **THE LINES WAITING AT A POINT ARE PAIRED TWO AT A TIME, IN THE ORDER THEY WERE CHOSEN**, and a line that is
    /// already in a corner there is spoken for - it makes one corner at that point and no other. So of four lines
    /// through one point the first two make the first corner, the third waits and the fourth makes the second with
    /// it; letting the first go leaves the second corner alone and the first line waiting, for a fourth line that
    /// has not been chosen.
    ///
    /// A pair that is put away stays put while both its lines are chosen: it is not made again while it is
    /// remembered, and it is not shown. It goes with a line of it that is let go, as a corner named at a point does,
    /// and choosing the line again makes the corner afresh - this time standing.
    /// A corner named at a point dies as soon as one of its lines leaves the selection or a corner of the lines
    /// stands there in its place.
    pub fn follow(&mut self, project: &Project, si: usize, selected: &[Id]) {
        self.follow_from(project, si, selected, selected);
    }

    /// THE SAME, WITH THE SELECTION IT WAS LAST READ FROM, which is how a corner named at a point knows that a
    /// line of it was let go: a line that has left the selection is one a person has said they do not want.
    pub fn follow_from(&mut self, project: &Project, si: usize, before: &[Id], selected: &[Id]) {
        let lost: Vec<Id> = before.iter().copied().filter(|l| !selected.contains(l)).collect();
        self.made.retain(|c| selected.contains(&c.pair.0) && selected.contains(&c.pair.1) && project.corner_of_pair(si, c.pair.0, c.pair.1) == Some(c.point));
        // A CORNER PUT AWAY LIVES WHILE BOTH ITS LINES ARE CHOSEN, and it goes with them. It is remembered as a pair
        // of lines and not as a mark on the drawing, so a line that is let go takes the corner at its end with it -
        // put away or not: what is put away is a corner of chosen lines, and a line a person has let go of is one
        // they do not want. Choosing the line again makes the corner again, and it stands.
        self.hidden.retain(|&(a, b)| selected.contains(&a) && selected.contains(&b) && project.corner_of_pair(si, a, b).is_some());
        // EVERY POINT WHERE CHOSEN LINES STAND, and the lines standing there in the order they were chosen.
        let mut at: Vec<(Id, Vec<Id>)> = Vec::new();
        // a construction line chosen before the tool was taken makes no corner either
        for &line in selected.iter().filter(|&&l| !line_is_construction(project, si, l)) {
            let Some((x, y)) = project.edge_ends(si, line) else { continue };
            for p in [x, y] {
                match at.iter_mut().find(|(q, _)| *q == p) {
                    Some((_, ls)) => ls.push(line),
                    None => at.push((p, vec![line])),
                }
            }
        }
        for (p, lines) in at {
            // THE LINES ALREADY IN A CORNER AT THIS POINT, standing or put away: a corner that is put away is
            // still a corner, and its lines are still spoken for.
            let spoken: Vec<Id> = self
                .made
                .iter()
                .filter(|c| c.point == p)
                .flat_map(|c| [c.pair.0, c.pair.1])
                .chain(self.hidden.iter().copied().filter(|(a, b)| project.corner_of_pair(si, *a, *b) == Some(p)).flat_map(|(a, b)| [a, b]))
                .collect();
            let waiting: Vec<Id> = lines.into_iter().filter(|l| !spoken.contains(l)).collect();
            for two in waiting.chunks(2) {
                let [a, b] = two else { continue };
                let pair = (*a, *b);
                // TWO LINES ALONG ONE STRAIGHT LINE THROUGH A POINT MAKE NO ANGLE: there is nothing to cut
                // there, and naming it as a corner would offer a cut where there is none.
                if project.corner_of_pair(si, *a, *b) != Some(p) || self.hidden.contains(&pair) || self.made.iter().any(|c| c.pair == pair) {
                    continue;
                }
                self.made.push(LineCorner { point: p, pair });
            }
        }
        // A CORNER NAMED AT A POINT LIVES WHILE BOTH ITS LINES ARE CHOSEN, and only while no corner of the lines
        // stands in its place: two corners at one point is one place read twice.
        let taken: Vec<Id> = self.made.iter().map(|c| c.point).collect();
        self.points.retain(|c| project.corner_of_pair(si, c.pair.0, c.pair.1) == Some(c.point) && !taken.contains(&c.point) && !lost.iter().any(|l| project.edge_stands_at(si, *l, c.point)));
    }

    /// THE MOST ONE ANSWER TAKES OFF EVERY CORNER OF THE SET AT ONCE, and what the lines between them can spare.
    ///
    /// One value cuts them all, so it is held by the tightest of them; and a line between two corners spends the
    /// value on itself at both ends, which is why rounding every corner of a rectangle takes half its short side.
    /// `None` when the set holds no corner, and a value then is held by nothing.
    ///
    /// A FILLET GIVEN BY ITS CHORD OR ITS ARC LENGTH is held in that way: the radius bound is the same for every corner,
    /// and each corner turns it into the chord or the arc of its own sweep - the set takes the least of them. Held by
    /// the radius alone, a chord of 18 passed on a corner of 135 deg where it is a radius of 23.5 against 20.5.
    pub fn limit(&self, project: &Project, si: usize, tool: qymcad_core::model::CornerTool, by: qymcad_core::model::FilletBy) -> Option<f64> {
        let least = |m: Option<f64>, v: f64| Some(m.map_or(v, |m: f64| m.min(v)));
        let standing = self.standing();
        let of_corners = standing.iter().filter_map(|&(pid, pair)| project.corner_limit_of_pair(si, pid, pair, tool)).fold(None, least);
        let lines = self.lines();
        let radius = match (of_corners, project.all_corners_limit(si, Some(&lines), tool)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }?;
        if tool != qymcad_core::model::CornerTool::Fillet || by == qymcad_core::model::FilletBy::Radius {
            return Some(radius);
        }
        standing.iter().filter_map(|&(pid, pair)| project.corner_sweep_of_pair(si, pid, pair)).map(|sweep| qymcad_core::model::FilletSize::of_radius(by, radius, sweep)).fold(None, least)
    }

    /// WHAT A CLICK AT A POINT DOES TO THE SET, and nothing else: the selection of lines does not move, and a
    /// point is not part of it.
    ///
    /// * A corner named at that point is **taken away**, with Shift or without, and the point is free again.
    /// * A corner of the lines standing there is **put away**, and the same click brings it back.
    /// * A point where nothing stands takes the corner the click read - the two lines at it, or the two the
    ///   cursor stands between where more than two are there.
    pub fn click_at_point(&mut self, project: &Project, si: usize, pid: Id, read: Option<(Id, Id)>) -> PointAct {
        if let Some(k) = self.points.iter().position(|c| c.point == pid) {
            self.points.remove(k);
            return PointAct::PointRemoved;
        }
        let made_here: Vec<(Id, Id)> = self.made.iter().filter(|c| c.point == pid).map(|c| c.pair).collect();
        let hidden_here: Vec<(Id, Id)> = self.hidden.iter().copied().filter(|(a, b)| project.corner_of_pair(si, *a, *b) == Some(pid)).collect();
        if !made_here.is_empty() {
            // A CORNER OF THE LINES IS PUT AWAY, NOT DELETED: the lines keep the ends they have, and the same
            // click at the same point brings that very corner back.
            self.hidden.extend(made_here.iter().copied());
            self.made.retain(|c| c.point != pid);
            return PointAct::LineHidden;
        }
        if !hidden_here.is_empty() {
            // EVERY CORNER THAT WAS PUT AWAY AT THIS POINT STANDS AGAIN, all of them at once: one click hides them
            // all, and the same click brings them all back.
            self.hidden.retain(|&(a, b)| !hidden_here.contains(&(a, b)));
            self.made.extend(hidden_here.into_iter().map(|pair| LineCorner { point: pid, pair }));
            return PointAct::LineShown;
        }
        match read {
            Some(pair) if project.corner_of_pair(si, pair.0, pair.1) == Some(pid) => {
                self.points.push(PointCorner { point: pid, pair });
                PointAct::PointAdded
            }
            _ => PointAct::Nothing,
        }
    }

    /// THE CORNER A CLICK AT A POINT WOULD NAME, which is what the yellow preview shows: the two lines at the
    /// point, or - where more than two stand there - the two the cursor is between. `None` where the point is not
    /// free for a corner, or where there is nothing to read.
    ///
    /// **THE CURSOR IS THE ONLY WORD, AND IT SAYS WHICH CORNER IS MEANT.** A point of three lines or more is a
    /// corner of every PAIR of them, and the drawing cannot say which pair a person means: the sector the cursor
    /// stands in is the corner being pointed at. `vertex_pair` answers exactly that, and answers it for a point of
    /// two lines as well - there is only one pair there and no word is needed.
    pub fn read_at_point(&self, project: &Project, si: usize, pid: Id, cursor: Option<(f64, f64)>) -> Option<(Id, Id)> {
        if self.occupied(project, si, pid) {
            return None;
        }
        project.vertex_pair(si, pid, cursor)
    }
}

/// HOW WIDE THE ZONE OF A POINT THAT CAN NAME A CORNER IS, in pixels: the reach of a pick of a point, THREE TIMES
/// OVER.
///
/// Three, and not one, because a corner at a point is named by a click and a person aims at a point rather than at
/// a spot beside it: with the reach of the pick itself the zone was a disc a shade wider than the dot, and a click
/// that a person meant for the point landed on the rest of the sheet instead. The zone is generous on purpose -
/// what a wide zone costs is a click on a point that was meant for a line, and that line is one Shift+click away,
/// while a zone too narrow costs the corner the person was sure they had named.
pub fn corner_zone_px(set: &Settings) -> f32 {
    grab::grab(set, grab::Grab::Point) * 3.0
}

/// WHAT THE CURSOR IS OVER, said by the sheet and not by the drawing.
///
/// **A POINT OF THE ZONE OUTRANKS A LINE UNDER THE CURSOR.** A point is the end of the lines that meet there, so
/// a click near a point is near both, and a rule that asked the line first would answer about a line whenever the
/// cursor was near its own end - which is where a person aims when they mean the corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerOver {
    /// NOTHING OF THE DRAWING, or something the corner tools have no word for.
    Nothing,
    /// A POINT WITHIN THE ZONE, and where the cursor stands in the drawing's own coordinates: it is what says
    /// which of several corners at that point is meant.
    Point { id: Id, at: (f64, f64) },
    /// A LINE, with no point of the zone near it.
    Line(Id),
}

/// WHAT A CLICK DID TO THE CORNER TOOL, so that the words to say are chosen where the click was read and not in
/// the branch that happened to run first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerClick {
    /// THE FIELD WAS NOT OPEN, and is open now for the whole set: the corners are made by the lines as they are
    /// chosen, so there is no single corner to wait to be told about.
    Opened,
    /// THE LINE IS CHOSEN NOW.
    LineChosen(Id),
    /// THE LINE IS LET GO OF, and the corner it made goes with it.
    LineLetGo(Id),
    /// THE LINE IS NOT CHOSEN: it would be a THIRD at a straight joint, and this is the point it would have stood
    /// at. Said rather than cut, because there is nothing to cut there.
    StraightJoint(Id),
    /// WHAT THE CLICK AT THE POINT DID TO THE CORNER THERE.
    AtPoint(PointAct),
    /// A LEFT CLICK WITHOUT SHIFT, ON NOTHING: THE MODE IS SINGLE FROM HERE, and the lines chosen stay chosen.
    SingleMode,
}

/// A CLICK WITH THE CORNER TOOL IN HAND: what is chosen of the drawing, and what that does to the set.
///
/// **THE CLICK THAT OPENS THE FIELD IS THE ONE THAT DOES NOT RESET ANYTHING.** Taking the tool leaves the field
/// closed, so there is no set in hand to single out and the lines that stood chosen are the beginning of one - a
/// person who chose a contour and then took the fillet meant that contour, and the first click adds to it rather
/// than throwing it away. Every click after that is an ordinary one.
///
/// **EVERY LEFT CLICK WITHOUT SHIFT LEAVES THE MULTI-SELECTION, WHATEVER IT LANDED ON - a line, a point, or
/// nothing at all.** Shift is the only way more than one line is chosen, so the set is begun and left with Shift
/// held: a multi-selection that outlives the key is a mode a person cannot see and cannot get out of, and the
/// report was that a click meant for one corner cut the whole set instead. What the click then chooses is a
/// matter of what it landed on, and only that.
///
/// **A CLICK ON A POINT WITHOUT SHIFT IS A SINGLE SELECTION OF A POINT, THE SAME AS A CLICK ON A LINE IS OF A
/// LINE.** It was the one exception, and the exception is what made the tool read as two: a click that landed on
/// a line left the set behind, a click that landed on a point beside one kept it, so what a click did depended on
/// a few pixels of aim. The lines chosen are let go of, the corners they made go with them, the corners named at
/// the other points - and this point is all that is left of the set.
///
/// **THE POINT CLICKED WITHOUT SHIFT IS ITSELF THE ONE EXCEPTION TO THAT**: a point already in the set clicked
/// again without Shift is the one thing to let go of, and the whole set goes with it rather than leaving a set of
/// nothing standing as though it were an answer. With Shift the click is about that point alone and the rest stays.
pub fn corner_click(project: &Project, si: usize, corner: &mut CornerInput, sel_sk: &mut SketchSelection, over: CornerOver, shift: bool) -> CornerClick {
    // THE FIELD OPENS FOR THE WHOLE SET, with the lines that are chosen already chosen: a person who drew a contour,
    // chose it and took the tool meant that contour, and asking again would be asking twice. **THE CLICK THAT OPENS
    // IT IS STILL A CLICK**, and it is acted upon as one - the tool is taken and a corner is pointed at in the same
    // gesture, and making the click only open the field would leave "click a corner, type the value, press Enter" -
    // the whole of the tool - naming nothing at all.
    let was_open = corner.at.is_some();
    if !was_open {
        corner.at = Some((si, CORNER_SET, false));
    }
    let before: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    // THE CLICK THAT OPENS THE FIELD GROWS THE SET RATHER THAN SINGLING IT OUT: there was no set in hand to single
    // out, and the lines standing chosen are the start of one.
    let grow = shift || !was_open;
    // A CONSTRUCTION LINE IS NO SIDE OF A CORNER - the diagonal of a rectangle ends at its corners, and a cut between it
    // and a side cuts the drawing against a line that is not part of it - so a click on one is a click on nothing.
    let over = match over {
        CornerOver::Line(eid) if line_is_construction(project, si, eid) => CornerOver::Nothing,
        other => other,
    };
    let act = match over {
        // A CLICK AT A POINT. WITH SHIFT it is about that point alone, as it always was. WITHOUT SHIFT it is a
        // single selection, the same as a click on a line, so everything chosen before is let go of first - and
        // what it leaves behind is either this point alone or, when this point was in the set already, nothing.
        CornerOver::Point { id, at } => {
            corner.shifted = grow;
            if !grow {
                let was_there = corner.set.occupied(project, si, id);
                // THE WHOLE OF THE SET GOES: the lines chosen, the corners they made, the corners named at the
                // other points. What is put back is this point and this point alone.
                sel_sk.items.retain(|&(k, _)| k != 1);
                corner.set.clear();
                let act = if was_there {
                    PointAct::PointRemoved
                } else {
                    let read = corner.set.read_at_point(project, si, id, Some(at));
                    corner.set.click_at_point(project, si, id, read)
                };
                let none: Vec<Id> = Vec::new();
                corner.set.follow_from(project, si, &none, &none);
                return CornerClick::AtPoint(act);
            }
            let read = corner.set.read_at_point(project, si, id, Some(at));
            CornerClick::AtPoint(corner.set.click_at_point(project, si, id, read))
        }
        CornerOver::Line(eid) => {
            corner.shifted = grow;
            let chosen = before.contains(&eid);
            if chosen {
                // A LINE THAT IS CHOSEN AND IS CLICKED AGAIN IS LET GO OF: the corner it made goes with it, and the
                // line that stays does not pair again with a neighbour of the one that left.
                sel_sk.items.retain(|&(k, id)| !(k == 1 && id == eid));
                CornerClick::LineLetGo(eid)
            } else if grow {
                // A THIRD LINE AT A STRAIGHT JOINT IS NOT TAKEN, and the point it would have stood at is named:
                // the corners at a point are taken two at a time, and this one would have to be cut against a
                // straight joint, where there is no corner to cut.
                if let Some(pid) = straight_joint_in_the_way(project, si, eid, &before, &corner.set.spoken_at_points()) {
                    return CornerClick::StraightJoint(pid);
                }
                sel_sk.items.push((1, eid));
                CornerClick::LineChosen(eid)
            } else {
                // A CLICK WITHOUT SHIFT IS A SINGLE SELECTION: only this line is chosen, and the corners that
                // follow from it are the ones one answer cuts.
                sel_sk.items.retain(|&(k, _)| k != 1);
                sel_sk.items.push((1, eid));
                CornerClick::LineChosen(eid)
            }
        }
        // A CLICK ON NOTHING AT ALL IS STILL A CLICK, and it is still one without Shift: the mode is single from
        // here and the lines chosen stay chosen. Said rather than nothing, so that a click which changed no
        // geometry is still a rule and not a corner that went missing.
        CornerOver::Nothing => {
            corner.shifted = shift;
            return if was_open { CornerClick::SingleMode } else { CornerClick::Opened };
        }
    };
    let after: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    corner.set.follow_from(project, si, &before, &after);
    act
}

/// WHETHER `eid` IS A CONSTRUCTION LINE of the sketch: no side of a corner the fillet or the chamfer cuts.
pub(crate) fn line_is_construction(project: &Project, si: usize, eid: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == eid && e.construction))
}

/// THE POINT AT WHICH A LINE MUST NOT BE ADDED TO THE SELECTION, which is the point where two chosen lines stand
/// along ONE straight line and this one would be a third.
///
/// Two lines that meet at a straight angle are no conflict at all: there is nothing to cut there, and their other
/// ends may carry corners of their own. A third line arriving is a conflict, because the corners at that point are
/// taken two at a time and this one would have to be cut against a straight joint - so the line is not taken, and
/// the reason is said.
///
/// **ONLY THE LINES STILL WAITING AT THE POINT COUNT, NOT THE WHOLE SELECTION.** The lines that already stand in a
/// corner there are spoken for, and a straight joint among them says nothing about the line arriving now: at a
/// point of four lines, two of which lie along one straight line, the third and the fourth are a corner of their
/// own and the joint between the first two is none of their business. Refused there, the fourth line of a cross
/// could not be taken at all, and a cross is the one shape where four corners meet.
pub fn straight_joint_in_the_way(project: &Project, si: usize, line: Id, selected: &[Id], spoken: &[Id]) -> Option<Id> {
    let (x, y) = project.edge_ends(si, line)?;
    [x, y].into_iter().find(|&p| {
        let there: Vec<Id> = selected.iter().copied().filter(|&l| l != line && !spoken.contains(&l) && project.edge_stands_at(si, l, p)).collect();
        there.iter().enumerate().any(|(i, &a)| there[i + 1..].iter().any(|&b| project.shared_vertex(si, a, b) == Some(p) && project.corner_of_pair(si, a, b).is_none()))
    })
}

/// WHETHER THE PICK IS STILL THERE once the corner has been taken off.
///
/// The point of a corner goes with the corner, so a pick that was standing on it cannot stand lit afterwards: the
/// selection would show a point the drawing has not got.
pub fn pick_still_stands(project: &Project, si: usize, id: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.points.iter().any(|q| q.id == id) || s.entities.iter().any(|e| e.id == id))
}

/// THE CORNER TWO PICKS NAME: the point it is at, and how the picks say which of the corners there is meant.
///
/// Which of several corners at one point is meant cannot be read off the point alone, so the picks carry it: two
/// edges that share a point name the pair outright, while an edge named together with the point says only that the
/// corner is one of those this edge takes part in - `through` - and the cursor says which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerChoice {
    pub pid: Id,
    /// THE TWO EDGES THAT NAME THE CORNER, where two edges do. `None` where a point was picked, or a point and an
    /// edge: the pair is read off the cursor instead.
    pub edges: Option<(Id, Id)>,
    /// THE EDGE THE CORNER MUST BE TAKEN FROM, where an edge and a point name it together.
    pub through: Option<Id>,
}

/// TAKE THE FILLET OR THE CHAMFER INTO HAND (4 = fillet, 5 = chamfer) and hold it to what is already chosen.
pub fn start_corner_tool(bc: &mut BarCtx, op: u8) {
    set_click_op(&mut tools_in!(bc), &mut *bc.mode_3d, op);
    if bc.armed.click_op() != op {
        return; // pressed again: the tool goes back down, and what was chosen stays chosen
    }
    *bc.status = Notice::info(qymcad_i18n::tr(if op == 5 { "tb-chamfer-sketch-hint" } else { "tb-fillet-sketch-hint" }));
    bc.corner.track_px = corner_reach(&*bc.set);
    let si = match *bc.sel {
        Sel::Sketch(si) => si,
        _ => return,
    };
    // WHATEVER STOOD CHOSEN BEFORE THE TOOL WAS TAKEN IS THE SET: the corners those lines make are the corners one
    // answer cuts, and the choice stays lit rather than being asked for again - a person who drew a contour, chose
    // it and took the tool meant that contour.
    //
    // **THE FIELD DOES NOT OPEN UNTIL SOMETHING IS POINTED AT.** It used to open at once, over the middle of the
    // canvas, and the middle of the canvas is where the drawing is: a Shift+click on a line that ran under the box
    // was taken by the box, so a set could not be built out of a part whose lines passed through the middle of the
    // sheet, and the person was left blaming their own aim. A field over the drawing takes clicks away from it, so
    // there is no field until there is something to type into.
    //
    // THE COLOURS OF THE PREVIEW ARE NOT SET HERE: they are read from the scheme where the corner is drawn, on every
    // frame, so that every way of opening a field draws alike and a person who changes the scheme sees it.
    bc.corner.shifted = false; // the tool is single until a Shift is held
    bc.corner.pos = None;
    let chosen: Vec<Id> = bc.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    bc.corner.set.follow(&*bc.project, si, &chosen);
}

/// Turn a click-driven editing operation on or off (1 = trim, 2 = extend, 3 = break).
pub fn set_click_op(t: &mut Tools, mode_3d: &mut bool, op: u8) {
    let cur = t.armed.click_op();
    exit_draw_tools(&mut t.reborrow());
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk, tool } = t;
    **armed = Armed::None;
    sel_sk.constraint = None;
    sel_sk.modify = None;
    **armed = if cur == op { Armed::None } else { Armed::ClickOp(op) };
    // EXTEND TAKES THE CURVE SELECTED BEFORE IT, as every tool takes its selection; nothing selected, it waits for a pick
    tool.extend = if armed.click_op() == 2 { sel_sk.items.iter().find(|(k, _)| *k == 1).map(|(_, id)| *id) } else { None };
    if armed.click_op() != 0 {
        *mode_3d = false;
    }
}

/// Turn the dimension tool on or off (k: 1 = linear, 2 = angular, 3 = radius).
pub fn set_dim_tool(t: &mut Tools, mode_3d: &mut bool, project: &Project, sel: Sel, sketch_ses: SketchSession, status: &mut Notice, k: u8) {
    let cur = t.armed.dim_kind();
    exit_draw_tools(&mut t.reborrow()); // entering a tool means leaving all the others, in one move
    let Tools { armed, annot: _, cmd: _, corner: _, dim: _, drag: _, gsel: _, inline: _, measure: _, pat: _, pending_import: _, picking: _, place: _, sel_sk: _, tool: _ } = t;
    **armed = if cur == k { Armed::None } else { Armed::Dimension(k) };
    if armed.dim_kind() != 0 {
        *mode_3d = false;
        if edit_si(project, &sketch_ses).is_none() && !matches!(sel, Sel::Sketch(_)) {
            *status = Notice::info(qymcad_i18n::tr("g-open-sketch-dim"));
        } else {
            *status = Notice::info(qymcad_i18n::tr("g-dim-hint"));
        }
    }
}
