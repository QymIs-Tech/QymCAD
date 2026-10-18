//! THE OFFSET of curves in a sketch: its side, its box, its curves.

use qymcad_ui_state::Notice;
use super::*;

/// THE SIDE OF AN OFFSET FIXED BY A CLICK: with curves chosen, a click that names no curve fixes the side of the copy
/// where it is, and the box of the distance opens beside the copy (`offset_box`). A click on a curve adds it, as before.
pub(crate) fn offset_side_click(sk: &mut qymcad_ui_state::SketchCtx, hit: Option<(u8, Id)>, at: Point2) -> bool {
    if sk.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset) || matches!(hit, Some((1, _))) || !sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
        return false;
    }
    sk.sel_sk.offset = qymcad_ui_state::OffsetAim { at: Some(at), buf: qymcad_core::expr::fmt_num(sk.tool_prefs.offset), focus: true };
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
    sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect()
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

/// THE OFFSET MADE BY ENTER before a click has fixed the side: on the side of the pointer, by the distance of the bar,
/// Enter in the field of the bar included.
pub(crate) fn offset_on_enter(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    if sk.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset) || sk.sel_sk.offset.at.is_some() || offset_curves(sk.sel_sk).is_empty() {
        return;
    }
    let pressed = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if !pressed && !qymcad_ui_state::bar_enter_take(ctx) {
        return;
    }
    if qymcad_ui_state::bar_field_bad("sk_offset") {
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-offset-field-bad"));
        return;
    }
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).map(|p| qymcad_ui_state::to_world(&*sk.view, rect, p));
    let dist = sk.tool_prefs.offset;
    offset_make(
        OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
        si,
        dist,
        pointer,
    );
}

/// THE BOX OF THE DISTANCE OF AN OFFSET whose side a click has fixed: beside the copy, the distance of the bar in it and
/// the keyboard given to it. What is typed is the bar's distance too, and the preview follows it; Enter or the tick
/// makes the copy, Esc or the cross frees the side and the copy follows the pointer again.
pub fn offset_box(oc: OffsetCtx, view: &qymcad_ui_state::View2d, si: usize, ctx: &egui::Context, rect: Rect) {
    let Some(at) = oc.sel_sk.offset.at.filter(|_| oc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset)) else { return };
    let eids = offset_curves(oc.sel_sk);
    if eids.is_empty() {
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        return;
    }
    // the box stands off the point of the copy nearest the click, on the side away from the source
    let dist = oc.project.offset_toward(si, &eids, oc.tool_prefs.offset, at);
    let sh = qymcad_ui_state::Sheet { view: *view, rect };
    let near = oc.project.offset_preview(si, &eids, dist).into_iter().flatten().min_by(|a, b| a.dist(at).total_cmp(&b.dist(at))).unwrap_or(at);
    let pos = qymcad_ui_state::clamp_popup(sh.at(near) + egui::vec2(OFFSET_BOX_GAP, OFFSET_BOX_GAP), rect);
    let want_focus = std::mem::take(&mut oc.sel_sk.offset.focus);
    let mut buf = std::mem::take(&mut oc.sel_sk.offset.buf);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut apply, mut cancel) = (false, false);
    let why = |project: &qymcad_core::model::Project, text: &str| match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
        Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
        Ok(v) if !v.is_finite() || v.abs() <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
        Ok(_) => None,
    };
    egui::Area::new(egui::Id::new(("offsetbox", si))).fixed_pos(pos).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(qymcad_i18n::tr("opt-distance"));
                let r = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                let said = why(&*oc.project, &buf);
                if r.changed() && said.is_none() {
                    oc.tool_prefs.offset = parse_num(&*oc.project, &buf).unwrap_or(oc.tool_prefs.offset);
                }
                if let Some(w) = &said {
                    qymcad_ui_state::tone_mark_in(ui, qymcad_ui_state::Tone::Refusal).on_hover_text(w);
                }
                apply |= enter && (r.has_focus() || r.lost_focus());
                apply |= ui.button(ph::CHECK).clicked();
                cancel |= ui.button(ph::X).clicked();
            });
        });
    });
    let refused = why(&*oc.project, &buf);
    oc.sel_sk.offset.buf = buf;
    if apply && refused.is_some() {
        *oc.status = Notice::refusal(refused.unwrap_or_default());
    } else if apply {
        let d = oc.tool_prefs.offset;
        offset_make(oc, si, d, Some(at));
    } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *oc.status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
    }
}

/// How far the box of the offset distance stands off the copy, in pixels, right and down.
pub(crate) const OFFSET_BOX_GAP: f32 = 16.0;
