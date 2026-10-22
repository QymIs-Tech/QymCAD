//! THE KEYS AND THE MENU of a sketch.

use qymcad_ui_state::Notice;
use super::*;

/// LINES A CORNER TOOL CUT AT A CROSSING, the tool put down with no corner cut, are whole again: looked at every frame.
pub fn corner_crossing_upkeep(sk: &mut qymcad_ui_state::SketchCtx) {
    if sk.corner.crossing == qymcad_ui_state::CrossingCut::Left {
        qymcad_ui_state::take_back_last_step(&mut *sk.edits, &mut *sk.project, &mut *sk.regen);
        sk.corner.crossing = qymcad_ui_state::CrossingCut::None;
    }
}

/// THE ESC LADDER: THE INNERMOST state is cancelled, and only at the very bottom is the selection let go. The order
/// matters - reports arrive exactly when some rung is missing and Esc "falls through" (the measure tool and the selected
/// elements were two such rungs). While a sketch is open the frame of the tool in hand takes Esc and steps down by it;
/// with no sketch open the window does.
pub fn escape_step(sk: &mut qymcad_ui_state::SketchCtx) {
    let mut said = tools_said(sk);
    let t = &mut said.tools;
    if t.inline.dim().is_some() || t.inline.circle().is_some() {
        // the popup for editing a dimension is open (a double click on a dimension or a circle) - Esc cancels ONLY the
        // popup rather than closing the whole sketch
        t.inline.clear();
        t.dim.put_away();
    } else if t.pending_import.curves.is_some() {
        t.pending_import.curves = None;
        *said.status = Notice::info(qymcad_i18n::tr("in-import-cancelled"));
    } else if let Some(k) = t.picking.cancel_key() {
        t.picking.clear(); // the modes Esc simply puts down; the words come from the mode itself
        *said.status = Notice::info(qymcad_i18n::tr(k));
    } else if t.picking.plane_face().is_some() {
        t.picking.set_plane_face(None);
        *said.status = Notice::info(qymcad_i18n::tr("in-plane-face-cancelled"));
    } else if t.sel_sk.constraint.is_some() || t.sel_sk.modify.is_some() {
        qymcad_ui_state::escape_waiting_tool(t, said.status);
    } else if t.leave_click_tool(said.status) {
    } else if t.armed.pattern().is_some() {
        *t.armed = qymcad_ui_state::Armed::None;
        t.pat.edit = None;
        t.pat.center = None;
        *said.status = Notice::info(qymcad_i18n::tr("in-array-cancelled"));
    } else if t.armed.moving().is_some() {
        *t.armed = qymcad_ui_state::Armed::None;
        t.tool.move_base = None;
        *said.status = Notice::info(qymcad_i18n::tr("in-move-cancelled"));
    } else {
        escape_lower_step(sk);
    }
    // the selection let go takes what it held from the next tool
    sk.gsel.before.note(None, qymcad_ui_state::Adding::No);
}

/// THE LOWER RUNGS OF THE ESC LADDER: a copy or an insert waiting, a dimension being placed, its first reference, a
/// construction under way, the tool in hand, the points of an import, the selection, and at the bottom what is chosen.
fn escape_lower_step(sk: &mut qymcad_ui_state::SketchCtx) {
    if sk.clip.geom_place {
        sk.clip.geom_place = false;
        *sk.status = Notice::info(qymcad_i18n::tr("in-insert-cancelled"));
    } else if sk.clip.geom_pending.is_some() {
        sk.clip.geom_pending = None;
        *sk.status = Notice::info(qymcad_i18n::tr("in-copy-cancelled"));
    } else if sk.place.dim.is_some() {
        cancel_placing_dim(sk); // a provisional length goes without a trace
    } else if sk.dim.first.is_some() {
        sk.dim.first = None; // cancel the first reference (the point)
    } else if !sk.tool.pts.is_empty() {
        end_construction(sk); // a spline is finished, anything else broken off
    } else if let Some(msg) = qymcad_ui_state::release_armed_sketch_tool(&mut tools_said(sk).tools) {
        // ONE RUNG FOR EVERY SKETCH TOOL IN HAND, and the tool itself says what to clear. There used to be a rung per
        // family here and none for the editing tools, so mirror, offset, fillet, chamfer and the arrays could not be put
        // down at all - reported on the mirror, true of all of them.
        *sk.status = Notice::info(qymcad_i18n::tr(msg));
    } else if sk.pending_import.draw_pts.is_some() {
        sk.pending_import.draw_pts = None;
    } else if sk.sketch_ses.editing.is_some() && (!sk.sel_sk.items.is_empty() || sk.annot.text.is_some() || sk.annot.note.is_some()) {
        // With elements SELECTED, Esc first clears the selection - as in any grown-up CAD. The selection used to take no
        // part in the ladder, and the very first Esc closed the sketch.
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        sk.annot.text = None;
        sk.annot.note = None;
        *sk.status = Notice::info(qymcad_i18n::tr("in-selection-cleared"));
    } else {
        // THE LADDER ENDS HERE: Esc gives things back, it does not finish a context. Leaving the sketch was the rung
        // below; it is Ctrl+Enter now - see `the_sketch_is_left_by_ctrl_enter.rs`.
        *sk.sel = qymcad_ui_state::Sel::None;
    }
}

/// THE MENU OF THE RIGHT BUTTON ON A SKETCH OPEN FOR EDITING, over what is selected: delete, construction, a projection
/// made ordinary geometry, the dimensions an arc and two round edges take, the constraints, select all.
pub fn sketch_context_menu(sk: &mut qymcad_ui_state::SketchCtx, ui: &mut egui::Ui, si: usize) {
    let has_sel = !sk.sel_sk.items.is_empty();
    let has_ent = sk.sel_sk.items.iter().any(|it| it.kind == qymcad_ui_state::ItemKind::Curve);
    if ui.add_enabled(has_sel, egui::Button::new(format!("{} {}", ph::TRASH, qymcad_i18n::tr("props-delete")))).clicked() {
        qymcad_ui_state::delete_sketch_sel(&mut *sk.project, &mut *sk.regen, &mut *sk.sel_sk, &mut *sk.status, si);
        ui.close();
    }
    if ui.add_enabled(has_ent, egui::Button::new(qymcad_i18n::tr("sk-construction-toggle"))).clicked() {
        let eids: Vec<Id> = sk.sel_sk.curves().collect();
        sk.project.toggle_construction(si, &eids);
        sk.project.solve_sketch(si);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        ui.close();
    }
    // THE CURVES LYING HERE: a curve picked that others lie over - a side drawn over another - lists them all, each by
    // its name and length, and the one clicked becomes the selection
    let picked: Vec<Id> = sk.sel_sk.curves().collect();
    if let [one] = picked.as_slice() {
        let here = sk.project.overlapping_curves(si, *one);
        if here.len() > 1 {
            ui.menu_button(qymcad_i18n::tr1("sk-overlapping", "n", &here.len().to_string()), |ui| {
                for id in here {
                    let name = sketch_entity_name(&*sk.project, si, id);
                    let len = qymcad_i18n::num(curve_length(&*sk.project, si, id), 1);
                    // the rectangle the curve is a side of, by its number in the sketch: two sides of one length are told
                    // apart by the shape they belong to
                    let rect = sk.project.sketches[si].rects().iter().position(|r| r.sides.contains(&id));
                    let label = match rect {
                        Some(k) => qymcad_i18n::trn("sk-overlapping-side", &[("name", &name), ("rect", &(k + 1).to_string()), ("len", &len)]),
                        None => qymcad_i18n::tr2("sk-overlapping-item", "name", &name, "len", &len),
                    };
                    if ui.button(label).clicked() {
                        sk.sel_sk.items = vec![qymcad_ui_state::SketchItem::curve(id)];
                        *sk.status = Notice::info(qymcad_i18n::tr1("sk-overlapping-picked", "name", &name));
                        ui.close();
                    }
                }
            });
        }
    }
    // A PROJECTION MADE ORDINARY: the curves selected that a body drives are let go of it, and stay where they are
    let projected: Vec<Id> = sk
        .sel_sk
        .items
        .iter()
        .filter(|qymcad_ui_state::SketchItem { kind: k, id }| *k == qymcad_ui_state::ItemKind::Curve && sk.project.sketches[si].projected_entities().contains(id))
        .map(|qymcad_ui_state::SketchItem { id, .. }| *id)
        .collect();
    if !projected.is_empty() && ui.button(qymcad_i18n::tr("sk-make-ordinary")).on_hover_text(qymcad_i18n::tr("sk-make-ordinary-hint")).clicked() {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-make-ordinary"));
        sk.project.release_projections_of(si, &projected);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        *sk.status = Notice::info(qymcad_i18n::tr("sk-made-ordinary"));
        ui.close();
    }
    // an arc length dimension, for the picked arc
    let arc_eid =
        sk.sel_sk.items.iter().filter(|it| it.kind == qymcad_ui_state::ItemKind::Curve).find_map(|qymcad_ui_state::SketchItem { id, .. }| {
            sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == *id)).and_then(|e| e.kind.as_arc().is_some().then_some(*id))
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
                .filter_map(|&qymcad_ui_state::SketchItem { kind: k, id }| {
                    if k == qymcad_ui_state::ItemKind::Curve {
                        s.entity(id).and_then(|e| e.kind.round_centre()).map(|c| (c, -1i8))
                    } else if k == qymcad_ui_state::ItemKind::Point {
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
            (&qymcad_i18n::tr("sk-coincident"), qymcad_ui_state::ConstraintKind::Coincident),
            (&qymcad_i18n::tr("sk-horizontal"), qymcad_ui_state::ConstraintKind::Horizontal),
            (&qymcad_i18n::tr("sk-vertical"), qymcad_ui_state::ConstraintKind::Vertical),
            (&qymcad_i18n::tr("sk-parallel"), qymcad_ui_state::ConstraintKind::Parallel),
            (&qymcad_i18n::tr("sk-perpendicular"), qymcad_ui_state::ConstraintKind::Perpendicular),
            (&qymcad_i18n::tr("sk-equal"), qymcad_ui_state::ConstraintKind::Equal),
            (&qymcad_i18n::tr("sk-tangency"), qymcad_ui_state::ConstraintKind::Tangent),
            (&qymcad_i18n::tr("sk-midpoint"), qymcad_ui_state::ConstraintKind::Midpoint),
            (&qymcad_i18n::tr("sk-fix"), qymcad_ui_state::ConstraintKind::Fix),
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
    let last = sk.project.sketches[si].entities.last().filter(|e| e.kind.as_arc().is_some()).map(|e| e.id);
    if let Some(eid) = last {
        open_the_size_of_the_new_shape(sk, eid, purpose);
    }
}
