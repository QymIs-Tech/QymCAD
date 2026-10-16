//! THE KEYS AND THE MENU of a sketch.

use qymcad_ui_state::Notice;
use super::*;

/// THE KEYS OF THE SKETCH TOOLS IN HAND, each frame: Enter makes the extension Extend previews, as a click does; and
/// the field of the rotation angle at its centre (`sketch_rotate_popup`).
pub fn sketch_tool_keys(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    offset_on_enter(sk, ctx, rect);
    // LINES A CORNER TOOL CUT AT A CROSSING, the tool put down with no corner cut, are whole again
    if sk.corner.crossing == qymcad_ui_state::CrossingCut::Left {
        qymcad_ui_state::take_back_last_step(&mut *sk.edits, &mut *sk.project, &mut *sk.regen);
        sk.corner.crossing = qymcad_ui_state::CrossingCut::None;
    }
    sketch_rotate_popup(sk, ctx, rect);
    let (Some(held), qymcad_ui_state::Sel::Sketch(si)) = (sk.tool.extend, *sk.sel) else { return };
    if sk.armed.click_op() != 2 || ctx.egui_wants_keyboard_input() || !ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-extend"));
    extend_apply(sk, ExtendAt { rect, pos, si, line: None }, held);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
}

/// THE MENU OF THE RIGHT BUTTON ON A SKETCH OPEN FOR EDITING, over what is selected: delete, construction, a projection
/// made ordinary geometry, the dimensions an arc and two round edges take, the constraints, select all.
pub fn sketch_context_menu(sk: &mut qymcad_ui_state::SketchCtx, ui: &mut egui::Ui, si: usize) {
    let has_sel = !sk.sel_sk.items.is_empty();
    let has_ent = sk.sel_sk.items.iter().any(|(k, _)| *k == 1);
    if ui.add_enabled(has_sel, egui::Button::new(format!("{} {}", ph::TRASH, qymcad_i18n::tr("props-delete")))).clicked() {
        qymcad_ui_state::delete_sketch_sel(&mut *sk.project, &mut *sk.regen, &mut *sk.sel_sk, &mut *sk.status, si);
        ui.close();
    }
    if ui.add_enabled(has_ent, egui::Button::new(qymcad_i18n::tr("sk-construction-toggle"))).clicked() {
        let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        sk.project.toggle_construction(si, &eids);
        sk.project.solve_sketch(si);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        ui.close();
    }
    // THE CURVES LYING HERE: a curve picked that others lie over - a side drawn over another - lists them all, each by
    // its name and length, and the one clicked becomes the selection
    let picked: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if let [one] = picked.as_slice() {
        let here = sk.project.overlapping_curves(si, *one);
        if here.len() > 1 {
            ui.menu_button(qymcad_i18n::tr1("sk-overlapping", "n", &here.len().to_string()), |ui| {
                for id in here {
                    let name = sketch_entity_name(&*sk.project, si, id);
                    let len = qymcad_i18n::num(curve_length(&*sk.project, si, id), 1);
                    // the rectangle the curve is a side of, by its number in the sketch: two sides of one length are told
                    // apart by the shape they belong to
                    let rect = sk.project.sketches[si].rects.iter().position(|r| r.sides.contains(&id));
                    let label = match rect {
                        Some(k) => qymcad_i18n::trn("sk-overlapping-side", &[("name", &name), ("rect", &(k + 1).to_string()), ("len", &len)]),
                        None => qymcad_i18n::tr2("sk-overlapping-item", "name", &name, "len", &len),
                    };
                    if ui.button(label).clicked() {
                        sk.sel_sk.items = vec![(1, id)];
                        *sk.status = Notice::info(qymcad_i18n::tr1("sk-overlapping-picked", "name", &name));
                        ui.close();
                    }
                }
            });
        }
    }
    // A PROJECTION MADE ORDINARY: the curves selected that a body drives are let go of it, and stay where they are
    let projected: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, id)| *k == 1 && sk.project.sketches[si].projected_entities().contains(id)).map(|(_, id)| *id).collect();
    if !projected.is_empty() && ui.button(qymcad_i18n::tr("sk-make-ordinary")).on_hover_text(qymcad_i18n::tr("sk-make-ordinary-hint")).clicked() {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-make-ordinary"));
        sk.project.release_projections_of(si, &projected);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        *sk.status = Notice::info(qymcad_i18n::tr("sk-made-ordinary"));
        ui.close();
    }
    // an arc length dimension, for the picked arc
    let arc_eid = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, id)| {
        sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == *id)).and_then(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. }).then_some(*id))
    });
    if let Some(aid) = arc_eid {
        if ui.button(qymcad_i18n::tr("sk-arc-length-dim")).clicked() {
            if let Some(ci) = sk.project.ensure_arc_length(si, aid) {
                qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
                sk.place.dim = Some(ci);
            }
            ui.close();
        }
    }
    // a tangent (edge-to-edge) dimension: two references are picked, at least one of
    // them a circle or an arc
    let edge_refs: Vec<(Id, i8)> = sk
        .project
        .sketches
        .get(si)
        .map(|s| {
            sk.sel_sk
                .items
                .iter()
                .filter_map(|&(k, id)| {
                    if k == 1 {
                        s.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                            qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some((center, -1i8)),
                            _ => None,
                        })
                    } else if k == 0 {
                        Some((id, 0i8))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let can_edge = edge_refs.len() == 2 && edge_refs.iter().any(|(_, m)| *m != 0);
    if ui.add_enabled(can_edge, egui::Button::new(qymcad_i18n::tr("sk-tangent-dim"))).on_hover_text(qymcad_i18n::tr("sk-gap-hint")).clicked() {
        let ((c1, m1), (c2, m2)) = (edge_refs[0], edge_refs[1]);
        let d = sk.project.measure_edge_distance(si, c1, m1, c2, m2);
        let ci = sk.project.sketches[si].constraints.len();
        sk.project.sketches[si].constraints.push(qymcad_core::model::Constraint::EdgeDistance { c1, c2, d, m1, m2, off: 0.0, expr: String::new(), driven: false, at: None });
        qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
        sk.gsel.constraint = Some(ci);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        ui.close();
    }
    ui.separator();
    ui.menu_button(qymcad_i18n::tr("sk-constraint"), |ui| {
        for (label, code) in [
            (&qymcad_i18n::tr("sk-coincident"), 0u8),
            (&qymcad_i18n::tr("sk-horizontal"), 1),
            (&qymcad_i18n::tr("sk-vertical"), 2),
            (&qymcad_i18n::tr("sk-parallel"), 3),
            (&qymcad_i18n::tr("sk-perpendicular"), 4),
            (&qymcad_i18n::tr("sk-equal"), 5),
            (&qymcad_i18n::tr("sk-tangency"), 9),
            (&qymcad_i18n::tr("sk-midpoint"), 11),
            (&qymcad_i18n::tr("sk-fix"), 6),
        ] {
            if ui.button(label).clicked() {
                constraint_button(sk, code);
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button(qymcad_i18n::tr("sk-select-all")).clicked() {
        qymcad_ui_state::select_all_sketch(&mut *sk.annot, &mut *sk.gsel, &*sk.project, &mut *sk.sel_sk, &mut *sk.status, si);
        ui.close();
    }
}

/// A click with the dimension tool. Returns true when the click was handled.
/// A CIRCLE OR AN ARC DRAWN OPENS THE FIELD OF ITS SIZE, and lays no dimension of its own. The size is laid as a
/// dimension when a value is typed and the field closed with Enter or the tick - a diameter for a circle, a radius for an
/// arc; closed untouched, or with Esc, the shape is left free, as the rectangle is (`rect_input_popup`).
///
/// Reported behaviour: "on circles a radius is always put ... if no value was typed and nothing was edited, do not put
/// it, and let it be deleted - it is nailed down. Sometimes a mess comes of it." A diameter laid on every circle drawn
/// over-defined a sketch the moment a tangent or an equal radius was laid on it.
///
/// Construction geometry carries no dimensions and opens no field.
pub(crate) fn open_the_size_of_the_new_shape(sk: &mut qymcad_ui_state::SketchCtx, eid: Id, purpose: qymcad_core::feature::Purpose) {
    if purpose == qymcad_core::feature::Purpose::Construction {
        return;
    }
    *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid);
    sk.dim.focus = true;
}

/// The arc the tool has just added, the last entity of the sketch, has the field of its radius opened.
pub(crate) fn open_the_size_of_the_new_arc(sk: &mut qymcad_ui_state::SketchCtx, si: usize, purpose: qymcad_core::feature::Purpose) {
    let last = sk.project.sketches[si].entities.last().filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. })).map(|e| e.id);
    if let Some(eid) = last {
        open_the_size_of_the_new_shape(sk, eid, purpose);
    }
}
