//! THE OFFSET of curves in a sketch: its side, its box, its curves.

use qymcad_ui_state::Notice;
use super::*;

/// THE SIDE OF AN OFFSET FIXED BY A CLICK: with curves chosen, a click that names no curve fixes the side of the copy
/// where it is, and the box of the distance opens beside the copy (`offset_box`). A click on a curve adds it, as before.
pub(crate) fn offset_side_click(sk: &mut qymcad_ui_state::SketchCtx, hit: Option<qymcad_ui_state::SketchItem>, at: Point2) -> bool {
    if sk.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset)
        || matches!(hit, Some(qymcad_ui_state::SketchItem { kind: qymcad_ui_state::ItemKind::Curve, id: _ }))
        || !sk.sel_sk.items.iter().any(|it| it.kind == qymcad_ui_state::ItemKind::Curve)
    {
        return false;
    }
    sk.sel_sk.offset = qymcad_ui_state::OffsetAim { at: Some(at), focus: true, ..Default::default() }; // the box makes its field of the distance of the bar
    *sk.status = Notice::info(qymcad_i18n::tr("sk-offset-type"));
    true
}

/// WHAT THE OFFSET MAKES ITS COPY WITH: the document, its undo, the curves chosen and the tool's state, the distance.
pub struct OffsetCtx<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    pub edits: &'a mut qymcad_ui_state::Edits,
    pub regen: &'a mut qymcad_ui_state::Rebuilding,
    pub sel_sk: &'a mut qymcad_ui_state::SketchSelection,
    pub status: &'a mut qymcad_ui_state::Notice,
    pub tool_prefs: &'a mut qymcad_ui_state::SketchToolPrefs,
}

/// THE CURVES THE OFFSET HOLDS: the lines, arcs and circles chosen.
pub(crate) fn offset_curves(sel_sk: &qymcad_ui_state::SketchSelection) -> Vec<Id> {
    sel_sk.curves().collect()
}

/// MAKE THE COPY: the curves chosen offset by the distance `dist` of the tool on the side of `toward`, one step of undo.
/// The tool stays in hand for the next curves, its choice and its side let go; nothing it can offset says what it takes.
/// It used to put the curves down and keep the tool's name on the bar - the bar said Offset, a click chose as the arrow
/// does, and Enter did nothing: "the tool works every other time".
pub(crate) fn offset_make(oc: OffsetCtx, si: usize, dist: f64, toward: Option<Point2>) {
    let eids = offset_curves(oc.sel_sk);
    let dist = toward.map_or(dist, |w| oc.project.offset_toward(si, &eids, dist, w));
    qymcad_ui_state::begin_edit(&mut *oc.edits, &*oc.project, qymcad_i18n::tr("tool-offset"));
    let made = oc.project.offset_entities(si, &eids, dist);
    qymcad_ui_state::close_edit(&mut *oc.edits, &*oc.project);
    if made > 0 {
        qymcad_ui_state::invalidate(&mut *oc.regen);
        oc.sel_sk.items.clear();
        oc.sel_sk.at_middle.clear();
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *oc.status = Notice::info(qymcad_i18n::tr("sk-offset-made"));
    } else {
        *oc.status = Notice::refusal(qymcad_i18n::tr("sk-offset-takes"));
    }
}
