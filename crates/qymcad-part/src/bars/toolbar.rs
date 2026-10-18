//! The tool bar of the path and the switches, and the modes of fillet and chamfer.

use super::*;

pub fn toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    // WRAPPED, so that the bar put at a side lays its words in lines rather than running out of its strip
    ui.horizontal_wrapped(|ui| {
        // CONTEXT BREADCRUMBS instead of tabs (drilling in and out): Assembly > Part > ... (> Sketch)
        qymcad_ui_state::ensure_active_path(&mut *bc.active_path, &mut *bc.project);
        let path = bc.active_path.clone();
        for (i, cid) in path.iter().enumerate() {
            if i > 0 {
                ui.label("›");
            }
            let name = bc.project.components.iter().find(|c| c.id == *cid).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-assembly"));
            // the root and an assembly wear the same icon: both hold other things rather than being a shape
            let assembly = i == 0 || bc.project.component_kind(*cid) == Some(qymcad_core::feature::ComponentKind::Assembly);
            let icon = if assembly { ph::STACK } else { ph::CUBE };
            let is_here = i + 1 == path.len() && bc.sketch_ses.editing.is_none();
            if ui.selectable_label(is_here, format!("{icon} {name}")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::GotoPath(i));
            }
        }
        if let Some(sid) = bc.sketch_ses.editing {
            ui.label("›");
            let nm = bc.project.sketches.iter().find(|s| s.id == sid).map(|s| qymcad_i18n::name(&s.name)).unwrap_or_else(|| qymcad_i18n::tr("wb-sketch"));
            ui.label(egui::RichText::new(format!("{} {nm}", ph::PENCIL)).color(bc.scheme.pal.hint()));
        }
        if (bc.sketch_ses.editing.is_some() || bc.active_path.len() > 1) && ui.button(qymcad_i18n::tr("wb-finish")).on_hover_text(qymcad_i18n::tr("wb-finish-hint")).clicked() {
            bc.ask.push(qymcad_ui_state::BarAsk::ExitContext);
        }
        ui.separator();
        // the global parameters are reachable from any workbench
        if ui.selectable_label(bc.win.is(WinKind::Params), qymcad_i18n::tr("wb-params")).on_hover_text(qymcad_i18n::tr("wb-params-hint")).clicked() {
            bc.win.toggle(WinKind::Params);
        }
        ui.separator();

        if let Some(pts) = bc.pending_import.draw_pts.as_ref() {
            // a sketch is being drawn: finish or cancel
            let n = pts.len();
            ui.label(qymcad_i18n::tr1("wb-points-n", "n", &n.to_string()));
            if ui.button(qymcad_i18n::tr("wb-close")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, true);
            }
            if ui.button(qymcad_i18n::tr("wb-line")).clicked() {
                qymcad_ui_state::finish_drawing(qymcad_ui_state::editing_in!(bc), &mut *bc.pending_import, *bc.sketch_ses, false);
            }
            if ui.button(qymcad_i18n::tr("wb-cancel")).clicked() {
                bc.pending_import.draw_pts = None;
            }
            ui.separator();
        }

        // Snap and the grid: while DRAWING a sketch (the cursor) AND in 3D inside a Part or an Assembly (snapping the gizmo to the grid).
        let snap_ctx = bc.sketch_ses.editing.is_some() || (*bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly));
        if snap_ctx {
            ui.toggle_value(&mut bc.set.snap.on, format!("{} {}", ph::MAGNET, qymcad_i18n::tr("wb-snap"))).on_hover_text(qymcad_i18n::tr("wb-snap-hint"));
            if bc.set.snap.on {
                ui.add(egui::DragValue::new(&mut bc.set.snap.grid).speed(0.5).range(0.1..=100.0).prefix(qymcad_i18n::tr("wb-grid")).suffix(qymcad_i18n::tr("unit-mm-suffix")));
                // the gizmo's ROTATION step belongs to 3D inside a Part or an Assembly (a sketch has no use for it)
                if *bc.mode_3d && matches!(bc.workbench, qymcad_ui_state::Workbench::Part | qymcad_ui_state::Workbench::Assembly) {
                    ui.add(egui::DragValue::new(&mut bc.set.snap.rot_deg).speed(1.0).range(0.5..=90.0).prefix(qymcad_i18n::tr("wb-rotation")).suffix(qymcad_i18n::tr("unit-deg-suffix")));
                }
            }
            // the automatic constraints belong to drawing a sketch only
            if bc.sketch_ses.editing.is_some() {
                ui.toggle_value(&mut bc.set.auto_constrain, format!("{} {}", ph::MAGIC_WAND, qymcad_i18n::tr("wb-auto-constraints"))).on_hover_text(qymcad_i18n::tr("wb-auto-constraints-hint"));
            }
            ui.separator();
        }
        // "in context" applies inside ANY component (a part or a subassembly), not at the root: top-down
        // references to the neighbours + showing the PARENT datums. The root has no ancestors, so it is hidden there.
        if qymcad_ui_state::current_ctx_id(&*bc.active_path, &*bc.project) != bc.project.root {
            ui.toggle_value(&mut bc.win.context, format!("{} {}", ph::STACK, qymcad_i18n::tr("wb-in-context"))).on_hover_text(qymcad_i18n::tr("wb-in-context-hint"));
            ui.separator();
        }
    });
}

/// HOW A SKETCH CHAMFER IS GIVEN, on the bar of the chamfer: equal legs, two legs, or a leg and an angle - the words of
/// the chamfer of a part - and, for the last two, the second value beside the first leg. Reported (issue #35): the
/// sketch chamfer took one distance only, and a chamfer of 5 x 3 or of 5 at 30 deg had to be built by hand.
/// The ways of a sketch fillet, before its field: by its radius, its chord or the length of its arc.
pub(crate) fn fillet_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::model::FilletBy;
    for by in [FilletBy::Radius, FilletBy::Chord, FilletBy::ArcLength] {
        if ui.selectable_label(bc.tool_prefs.fillet_by == by, qymcad_i18n::tr(qymcad_ui_state::fillet_label(by))).clicked() {
            bc.tool_prefs.fillet_by = by;
        }
    }
    ui.separator();
}

/// The modes of a sketch chamfer, before its fields: the fields of one chamfer stand together after them.
pub(crate) fn chamfer_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::ChamferMode;
    for (mode, word) in [(ChamferMode::Symmetric, "cmd-symmetric"), (ChamferMode::TwoDist, "cmd-two-distances"), (ChamferMode::DistAngle, "cmd-leg-angle")] {
        if ui.selectable_label(bc.tool_prefs.chamfer_mode == mode, qymcad_i18n::tr(word)).clicked() {
            qymcad_ui_state::set_chamfer_mode(bc.tool_prefs, mode);
        }
    }
    ui.separator();
}

/// The second field of a sketch chamfer of two legs or of a leg and an angle, right after the first.
pub(crate) fn chamfer_second_field(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::ChamferMode;
    if bc.tool_prefs.chamfer_mode != ChamferMode::Symmetric {
        let angle = bc.tool_prefs.chamfer_mode == ChamferMode::DistAngle;
        ui.label(qymcad_i18n::tr(qymcad_ui_state::chamfer_d2_label(bc.tool_prefs.chamfer_mode)));
        bc.tool_prefs.chamfer_second = qymcad_ui_state::num_or_expr(
            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
            ui,
            "sk_chamfer2",
            bc.tool_prefs.chamfer_second,
            qymcad_ui_state::NumFormat {
                lo: 0.01,
                hi: if angle { 179.99 } else { 10000.0 },
                integer: false,
                suffix: &qymcad_i18n::tr(if angle { "unit-deg-suffix" } else { "unit-mm-suffix" }),
                nonzero: false,
            },
        );
    }
}
