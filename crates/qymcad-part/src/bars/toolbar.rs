//! The tool bar and the button panel of the Part workbench, and the modes of fillet and chamfer.

use qymcad_ui_state::Notice;
use super::*;

/// The shared Create buttons: THE DATUMS — they exist both in a Part and in an Assembly. A sketch is
/// added separately and ONLY in a Part (see `create_panel_part`).
///
/// SKETCHES ARE GONE FROM ASSEMBLIES. The button stood there marked "skeleton", so the intent of
/// top-down layout was there, but nothing could refer to that skeleton: a joint anchor understands
/// `Origin`, `BasePlane`, `FaceCenter`, `EdgeMid` and `Vertex` — there is no reference to a sketch, and
/// there is no extrude in the Assembly workbench either. A sketch there was inert: it could be drawn
/// but not used. Layout belongs in the part-modelling workbench.
///
/// DATUMS STAY IN AN ASSEMBLY — they are not inert: a mirrored copy of a component and the view's
/// section both consume them.
pub fn create_panel_common(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    let _ = (BasePlane::XY, SketchPlane::default);
    // Datums are COMMANDS: an options bar, fields at the geometry, click-picked references, a preview, Enter and Esc
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumPlane, &qymcad_i18n::tr("g-datum-plane-hint"), bc.armed.cmd_kind() == 20) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 20, qymcad_ui_state::BarAsk::FeatCmd(20)));
    }
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumPoint, &qymcad_i18n::tr("g-datum-point-hint"), bc.armed.cmd_kind() == 21) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 21, qymcad_ui_state::BarAsk::FeatCmd(21)));
    }
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::DatumAxis, &qymcad_i18n::tr("g-datum-axis-hint"), bc.armed.cmd_kind() == 22) {
        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 22, qymcad_ui_state::BarAsk::FeatCmd(22)));
    }
}

/// The one doorway into the properties panel.
/// The Sketch button — ONLY in the Part workbench (in an assembly a sketch is inert, see `create_panel_common`).
pub fn create_panel_sketch_button(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPickPlane, &qymcad_i18n::tr("g-sketch-pick-hint"), bc.picking.is_sketch_plane()) {
        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSketchPick);
    }
}

pub fn toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
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

pub fn wb_toolbar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    // Two columns of buttons (wrapping by width) + vertical scrolling - reliable at any window size.
    // The width 94 = 16 (the 8+8 margins) + 6 (the floating scrollbar) + 72 (two 34-wide button columns + a
    // 3-point gap + a margin). Before that, 90 with 38-wide buttons could not fit the second column, giving
    // one column and a wide empty strip on the right.
    // show_separator_line(false): by default egui draws a faint vertical line at the right edge of ANY panel,
    // and it read as a stray gap between the tools and the tree.
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
        ui.horizontal_wrapped(|ui| {
            match bc.workbench {
                qymcad_ui_state::Workbench::Sketch => {
                    // a full-width category label, which therefore starts a new row (breaking the columns)
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // "Finish" lives in the breadcrumbs (one place for it); here there are only sketch tools
                    // --- Creation ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSelect, &qymcad_i18n::tr("tb-select-hint"), qymcad_ui_state::in_select_mode(bc.armed)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::SketchSelectMode);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPoint, &qymcad_i18n::tr("tb-point-hint"), bc.armed.draw_kind() == 5) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 5, qymcad_ui_state::BarAsk::SketchTool(5)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchLine, &qymcad_i18n::tr("tb-line-hint"), bc.armed.draw_kind() == 1) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 1, qymcad_ui_state::BarAsk::SketchTool(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchRect, &qymcad_i18n::tr("tb-rect-hint"), bc.armed.draw_kind() == 2) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 2, qymcad_ui_state::BarAsk::SketchTool(2)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCircle, &qymcad_i18n::tr("tb-circle-hint"), bc.armed.draw_kind() == 3) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 3, qymcad_ui_state::BarAsk::SketchTool(3)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCircle3Pt, &qymcad_i18n::tr("tb-circle-3pt"), bc.armed.draw_kind() == 10) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 10, qymcad_ui_state::BarAsk::SketchTool(10)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArc, &qymcad_i18n::tr("tb-arc-hint"), bc.armed.draw_kind() == 4) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 4, qymcad_ui_state::BarAsk::SketchTool(4)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchPolygon, &qymcad_i18n::tr("tb-polygon-hint"), bc.armed.draw_kind() == 6) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 6, qymcad_ui_state::BarAsk::SketchTool(6)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSlot, &qymcad_i18n::tr("tb-slot-hint"), bc.armed.draw_kind() == 7) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 7, qymcad_ui_state::BarAsk::SketchTool(7)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchEllipse, &qymcad_i18n::tr("tb-ellipse-hint"), bc.armed.draw_kind() == 8) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 8, qymcad_ui_state::BarAsk::SketchTool(8)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchSpline, &qymcad_i18n::tr("tb-spline-hint"), bc.armed.draw_kind() == 9) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 9, qymcad_ui_state::BarAsk::SketchTool(9)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchText, &qymcad_i18n::tr("tb-text-hint"), bc.armed.draw_kind() == 11) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.draw_kind() == 11, qymcad_ui_state::BarAsk::SketchTool(11)));
                    }
                    // --- The line kind ---
                    cat(ui, &qymcad_i18n::tr("tb-type"));
                    // WHAT IS SELECTED IS TURNED, as the construction toggle of the professional systems does; with nothing selected
                    // the button switches what is drawn next
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchConstruction, &qymcad_i18n::tr("tb-construction-hint"), bc.tool.construction) {
                        qymcad_ui_state::construction_toggle(qymcad_ui_state::editing_in!(bc), &*bc.sel_sk, &*bc.sketch_ses, &mut bc.tool.construction);
                    }
                    // --- Editing and replication (over the selected entities) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-edit"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDelete, &qymcad_i18n::tr("tb-delete-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Delete)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Delete);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMirror, &qymcad_i18n::tr("tb-mirror-sketch-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Mirror)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Mirror);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArrayLinear, &qymcad_i18n::tr("tb-lin-array-hint"), bc.armed.pat_op() == 1) {
                        start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchArrayCircular, &qymcad_i18n::tr("tb-circ-array-hint"), bc.armed.pat_op() == 2) {
                        start_pattern(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchProject, &qymcad_i18n::tr("tb-project-body-hint"), bc.armed.click_op() == 6) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 6);
                        *bc.status = Notice::info(qymcad_i18n::tr("tb-project-hint"));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchFillet, &qymcad_i18n::tr("tb-fillet-sketch-hint"), bc.armed.click_op() == 4) {
                        qymcad_ui_state::start_corner_tool(bc, 4);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchChamfer, &qymcad_i18n::tr("tb-chamfer-sketch-hint"), bc.armed.click_op() == 5) {
                        qymcad_ui_state::start_corner_tool(bc, 5);
                    }
                    // "FILLET ALL" IS A TOOL IN HAND like its neighbours: pressed, it puts down what is held and takes what is
                    // selected or waits for a shape; pressed again, it is put down
                    let fillet_all_held = bc.picking.fillet_all() || bc.corner.at.is_some_and(|(_, pid, _)| pid == 0);
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchFilletAll, &qymcad_i18n::tr("tb-fillet-all-hint"), fillet_all_held) {
                        qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(bc));
                        if !fillet_all_held {
                            qymcad_ui_state::fillet_all_corners(&*bc.project, &mut *bc.corner, &mut *bc.picking, *bc.sel, &*bc.sel_sk, &mut *bc.status, &*bc.tool_prefs);
                        }
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchOffset, &qymcad_i18n::tr("tb-offset-hint"), bc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset)) {
                        qymcad_ui_state::modify_button(qymcad_ui_state::editing_in!(bc), &mut qymcad_ui_state::tools_in!(bc), *bc.sk_pat, &*bc.tool_prefs, qymcad_ui_state::EditTool::Offset);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMove, &qymcad_i18n::tr("tb-move-hint"), bc.armed.move_op() == 1) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchCopy, &qymcad_i18n::tr("tb-copy-hint"), bc.armed.move_op() == 2) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchRotate, &qymcad_i18n::tr("tb-rotate-hint"), bc.armed.move_op() == 3) {
                        start_move_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.status, 3);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchTrim, &qymcad_i18n::tr("tb-trim-hint"), bc.armed.click_op() == 1) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchExtend, &qymcad_i18n::tr("tb-extend-hint"), bc.armed.click_op() == 2) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchBreak, &qymcad_i18n::tr("tb-break-hint"), bc.armed.click_op() == 3) {
                        qymcad_ui_state::set_click_op(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, 3);
                    }
                    // --- Dimensions ---
                    cat(ui, &qymcad_i18n::tr("tb-group-dim"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimLinear, &qymcad_i18n::tr("tb-dim-hint"), bc.armed.dim_kind() == 1) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 1);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimAngle, &qymcad_i18n::tr("tb-dim-angle-hint"), bc.armed.dim_kind() == 2) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 2);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchDimRadius, &qymcad_i18n::tr("tb-dim-radius-hint"), bc.armed.dim_kind() == 3) {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_in!(bc), &mut *bc.mode_3d, &*bc.project, *bc.sel, *bc.sketch_ses, &mut *bc.status, 3);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::SketchMeasure, &qymcad_i18n::tr("tb-measure-hint"), bc.armed.measuring()) {
                        let on = !bc.armed.measuring();
                        // THROUGH THE DOOR, in the order every other tool uses. Setting the flag here and asking
                        // for "back to selection" afterwards turned the tool on and then straight off again.
                        qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on);
                        if on {
                            *bc.mode_3d = false;
                            *bc.status = Notice::info(qymcad_i18n::tr("in-measure-hint"));
                        } else {
                            *bc.status = Notice::info(qymcad_i18n::tr("in-measure-off"));
                        }
                    }
                    // --- Constraints ---
                    cat(ui, &qymcad_i18n::tr("tb-group-constraints"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintCoincident, &qymcad_i18n::tr("con-coincident-hint"), bc.sel_sk.constraint == Some(0)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(0));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintHorizontal, &qymcad_i18n::tr("con-horizontal-hint"), bc.sel_sk.constraint == Some(1)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(1));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintVertical, &qymcad_i18n::tr("con-vertical-hint"), bc.sel_sk.constraint == Some(2)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(2));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintParallel, &qymcad_i18n::tr("con-parallel-hint"), bc.sel_sk.constraint == Some(3)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(3));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintPerpendicular, &qymcad_i18n::tr("con-perpendicular-hint"), bc.sel_sk.constraint == Some(4)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(4));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintEqual, &qymcad_i18n::tr("con-equal"), bc.sel_sk.constraint == Some(5)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(5));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintCollinear, &qymcad_i18n::tr("con-collinear-hint"), bc.sel_sk.constraint == Some(7)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(7));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintConcentric, &qymcad_i18n::tr("con-concentric-hint"), bc.sel_sk.constraint == Some(8)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(8));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintTangent, &qymcad_i18n::tr("con-tangent-hint"), bc.sel_sk.constraint == Some(9)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(9));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintSymmetric, &qymcad_i18n::tr("con-symmetric-hint"), bc.sel_sk.constraint == Some(10)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(10));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintMidpoint, &qymcad_i18n::tr("con-midpoint-hint"), bc.sel_sk.constraint == Some(11)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(11));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::ConstraintLock, &qymcad_i18n::tr("con-fix"), bc.sel_sk.constraint == Some(6)) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Constraint(6));
                    }
                }
                qymcad_ui_state::Workbench::Part => {
                    // a full-width category label, breaking the columns (as in the Sketch)
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // --- Create (a sketch + the datums) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    create_panel_sketch_button(bc, ui);
                    create_panel_common(bc, ui);
                    // --- From a sketch ---
                    cat(ui, &qymcad_i18n::tr("tb-group-sketch3d"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartExtrude, &qymcad_i18n::tr("tb-extrude-hint"), tool_is_taken(bc, 1)) {
                        bc.feat.op = 0;
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 1), qymcad_ui_state::BarAsk::FeatCmd(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRevolve, &qymcad_i18n::tr("tb-revolve-hint"), tool_is_taken(bc, 3)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 3), qymcad_ui_state::BarAsk::FeatCmd(3)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSweep, &qymcad_i18n::tr("tb-sweep-hint"), tool_is_taken(bc, 8)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 8), qymcad_ui_state::BarAsk::FeatCmd(8)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartLoft, &qymcad_i18n::tr("tb-loft-hint"), tool_is_taken(bc, 9)) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(tool_is_taken(bc, 9), qymcad_ui_state::BarAsk::FeatCmd(9)));
                    }
                    // --- The 3D primitives (a command: sizes at the geometry + a preview + Enter/Esc) ---
                    cat(ui, &qymcad_i18n::tr("tb-group-prim"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartBox, &qymcad_i18n::tr("tb-box-hint"), bc.armed.cmd_kind() == 10) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 10, qymcad_ui_state::BarAsk::PrimCmd(10)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartCylinder, &qymcad_i18n::tr("tb-cylinder-hint"), bc.armed.cmd_kind() == 11) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 11, qymcad_ui_state::BarAsk::PrimCmd(11)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSphere, &qymcad_i18n::tr("tb-sphere-hint"), bc.armed.cmd_kind() == 12) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 12, qymcad_ui_state::BarAsk::PrimCmd(12)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartCone, &qymcad_i18n::tr("tb-cone-hint"), bc.armed.cmd_kind() == 13) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 13, qymcad_ui_state::BarAsk::PrimCmd(13)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartTorus, &qymcad_i18n::tr("tb-torus-hint"), bc.armed.cmd_kind() == 14) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 14, qymcad_ui_state::BarAsk::PrimCmd(14)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPrism, &qymcad_i18n::tr("tb-prism-hint"), bc.armed.cmd_kind() == 15) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 15, qymcad_ui_state::BarAsk::PrimCmd(15)));
                    }
                    // --- Operations on a body ---
                    cat(ui, &qymcad_i18n::tr("tb-body"));
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartFillet, &qymcad_i18n::tr("tb-fillet-body-hint"), bc.armed.cmd_kind() == 4) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 4, qymcad_ui_state::BarAsk::FeatCmd(4)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartChamfer, &qymcad_i18n::tr("tb-chamfer-body-hint"), bc.armed.cmd_kind() == 5) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 5, qymcad_ui_state::BarAsk::FeatCmd(5)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartShell, &qymcad_i18n::tr("tb-shell-hint"), bc.armed.cmd_kind() == 6) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 6, qymcad_ui_state::BarAsk::FeatCmd(6)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSection, &qymcad_i18n::tr("tb-section-hint-bar"), bc.section.pick || bc.section.plane.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartHole, &qymcad_i18n::tr("tb-hole-hint"), bc.armed.cmd_kind() == 7) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 7, qymcad_ui_state::BarAsk::FeatCmd(7)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartDraft, &qymcad_i18n::tr("tb-draft-hint"), bc.armed.cmd_kind() == 23) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 23, qymcad_ui_state::BarAsk::FeatCmd(23)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPushFace, &qymcad_i18n::tr("tb-push-face-hint"), bc.armed.cmd_kind() == 25) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 25, qymcad_ui_state::BarAsk::FeatCmd(25)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRemoveFace, &qymcad_i18n::tr("tb-remove-face-hint"), bc.armed.cmd_kind() == 26) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 26, qymcad_ui_state::BarAsk::FeatCmd(26)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartThicken, &qymcad_i18n::tr("tb-thicken-hint"), bc.armed.cmd_kind() == 28) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 28, qymcad_ui_state::BarAsk::FeatCmd(28)));
                    }
                    // the bridge from the parametric side into the design layer: a face becomes a surface
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartFaceCopy, &qymcad_i18n::tr("tb-face-copy-hint"), bc.armed.cmd_kind() == 30) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 30, qymcad_ui_state::BarAsk::FeatCmd(30)));
                    }
                    // a face taken out as a sheet moved along its normal - the offset surface of the professional systems
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartOffsetSurface, &qymcad_i18n::tr("tb-offset-surface-hint"), bc.armed.cmd_kind() == 36) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 36, qymcad_ui_state::BarAsk::FeatCmd(36)));
                    }
                    // the far end of that bridge: a surface goes back into the body
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSurfaceReplace, &qymcad_i18n::tr("tb-surface-replace-hint"), bc.armed.cmd_kind() == 31) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 31, qymcad_ui_state::BarAsk::FeatCmd(31)));
                    }
                    // the first shape the body did not have: a surface built from the edges
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartPatch, &qymcad_i18n::tr("tb-patch-hint"), bc.armed.cmd_kind() == 32) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 32, qymcad_ui_state::BarAsk::FeatCmd(32)));
                    }
                    // pieces of surface become one, and a shell that closes becomes a body
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartStitch, &qymcad_i18n::tr("tb-stitch-hint"), bc.armed.cmd_kind() == 33) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 33, qymcad_ui_state::BarAsk::FeatCmd(33)));
                    }
                    // trim a surface with the neighbouring geometry
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartTrimSurface, &qymcad_i18n::tr("tb-trim-surface-hint"), bc.armed.cmd_kind() == 34) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 34, qymcad_ui_state::BarAsk::FeatCmd(34)));
                    }
                    // an imported mesh becomes a body a person can work on
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartRecognise, &qymcad_i18n::tr("tb-recognise-hint"), bc.armed.cmd_kind() == 35) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 35, qymcad_ui_state::BarAsk::FeatCmd(35)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSplitBody, &qymcad_i18n::tr("tb-split-body-hint"), bc.armed.cmd_kind() == 27) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 27, qymcad_ui_state::BarAsk::FeatCmd(27)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartMeasure, &qymcad_i18n::tr("tb-measure3d-hint"), bc.m3.on) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleMeasure3d);
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartSplitFace, &qymcad_i18n::tr("tb-split-face-hint"), bc.armed.cmd_kind() == 29) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 29, qymcad_ui_state::BarAsk::FeatCmd(29)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartThread, &qymcad_i18n::tr("tb-thread-hint"), bc.armed.cmd_kind() == 24) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 24, qymcad_ui_state::BarAsk::FeatCmd(24)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartBooleanBodies, &qymcad_i18n::tr("tb-bool-bodies-hint"), bc.boolean.pick.is_some()) {
                        if bc.boolean.pick.is_some() {
                            // pressed again with the tool in hand: put down, as every tool button does
                            bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
                        } else {
                            // taking the boolean puts down the tool in hand first, or both bars stood at once
                            let a = qymcad_ui_state::selected_body(&*bc.project, &*bc.sel);
                            bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Boolean(a)));
                        }
                    }
                    // THE MIRROR AND THE BODY PATTERNS LIVE HERE, UNDER "BODY". A separate "Patterns" category
                    // used to stand beside it showing the same two icons as "Body" above: those were COMPONENT
                    // patterns (an assembly tool), these are BODY patterns. What that looked like was
                    // duplicates - and it was worse than duplicates, because two buttons that looked alike did
                    // different things. The component ones moved into the Assembly, where they belong.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartMirror, &qymcad_i18n::tr("tb-mirror-body-hint"), bc.armed.cmd_kind() == 16) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 16, qymcad_ui_state::BarAsk::FeatCmd(16)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartArrayLinear, &qymcad_i18n::tr("tb-lin-array-body-hint"), bc.armed.cmd_kind() == 17) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 17, qymcad_ui_state::BarAsk::FeatCmd(17)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::PartArrayCircular, &qymcad_i18n::tr("tb-circ-array-body-hint"), bc.armed.cmd_kind() == 18) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.armed.cmd_kind() == 18, qymcad_ui_state::BarAsk::FeatCmd(18)));
                    }
                }
                qymcad_ui_state::Workbench::Assembly => {
                    let cat = |ui: &mut egui::Ui, s: &str| {
                        ui.add_sized([84.0, 12.0], egui::Label::new(egui::RichText::new(s).size(8.5).weak()));
                    };
                    // --- Create ---
                    cat(ui, &qymcad_i18n::tr("tb-group-create"));
                    create_panel_common(bc, ui); // the datums; a sketch is inert in an assembly, so it was removed
                                                 // A NEW PART OR SUBASSEMBLY IS ONE STEP OF UNDO of its own name
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyNewPart, &qymcad_i18n::tr("tb-new-part-hint"), false) {
                        qymcad_ui_state::begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-n"));
                        let id = bc.project.add_part(bc.project.free_part_name());
                        qymcad_ui_state::close_edit(bc.edits, bc.project);
                        bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyNewSubassembly, &qymcad_i18n::tr("tb-new-subassembly-hint"), false) {
                        qymcad_ui_state::begin_edit(bc.edits, bc.project, qymcad_i18n::tr("hotkey-assembly-u"));
                        let id = bc.project.add_assembly(qymcad_i18n::tr1("node-assembly-n", "n", &bc.project.components.len().to_string()));
                        qymcad_ui_state::close_edit(bc.edits, bc.project);
                        bc.ask.push(qymcad_ui_state::BarAsk::EnterComponent(id));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyInsertComponent, &qymcad_i18n::tr("tb-insert-component-hint"), false) {
                        bc.ask.push(qymcad_ui_state::BarAsk::Import(qymcad_ui_state::Want::Part));
                    }
                    // COMPONENT PATTERNS ARE AN ASSEMBLY TOOL. They used to sit in the PART workbench, where
                    // there are no components, and there they duplicated the look of the body patterns.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyArrayLinear, &qymcad_i18n::tr("tb-comp-lin-array-hint"), bc.carr.mode == 1) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.carr.mode == 1, qymcad_ui_state::BarAsk::CompArray(1)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyArrayCircular, &qymcad_i18n::tr("tb-comp-circ-array-hint"), bc.carr.mode == 2) {
                        bc.ask.push(qymcad_ui_state::take_or_drop(bc.carr.mode == 2, qymcad_ui_state::BarAsk::CompArray(2)));
                    }
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyMirror, &qymcad_i18n::tr("tb-mirror-part-hint"), bc.mirror.in_hand()) {
                        // both a PART and a SUBASSEMBLY are accepted (the whole subtree is mirrored); with the tool in
                        // hand the press goes to the door as it is, which puts it down
                        let src = bc.mirror.part.or_else(|| match *bc.sel {
                            qymcad_ui_state::Sel::Component(ci) => bc.project.components.get(ci).map(|c| c.id),
                            _ => qymcad_ui_state::selected_body(&*bc.project, &*bc.sel).and_then(|b| bc.project.body_owner(b)),
                        });
                        // THROUGH A DOOR, in the order that works: release the others, THEN take this one. Pushing the release
                        // and arming straight after put them the other way round - the release runs after the frame - and the
                        // button did nothing. With nothing picked before it the tool waits for a click on a part.
                        bc.ask.push(qymcad_ui_state::BarAsk::MirrorPart(src));
                    }
                    // ONE DOOR, not a second copy of it. This block used to repeat what `toggle_section` does,
                    // and the copy could not release the other tools from here, so it pushed the request and
                    // armed the section straight after. The request runs AFTER the frame and clears exactly
                    // that: switching the section ON through this button did nothing at all.
                    if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblySection, &qymcad_i18n::tr("tb-section-hint"), bc.section.pick || bc.section.plane.is_some()) {
                        bc.ask.push(qymcad_ui_state::BarAsk::ToggleSection);
                    }
                    // --- The mates: buttons per mate kind, as in the sketcher. Clicking a kind starts the
                    // face pick with that kind (face A, then face B). The list and the editing of existing ones
                    // live in the properties panel on the right, as the sketch dimensions do.
                    cat(ui, &qymcad_i18n::tr("tb-group-joint"));
                    {
                        // ONE BUTTON, with the kind chosen in the top bar. There used to be seven buttons, one
                        // per kind, while the kind was changed by a combo box in that same bar anyway: the
                        // choice was made twice and took up the whole category.
                        let tip = qymcad_i18n::tr1("jt-joint-tip", "kind", &qymcad_i18n::tr(bc.joint.new_kind.label()));
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyJoint, &tip, bc.joint.pick_faces) {
                            bc.ask.push(qymcad_ui_state::take_or_drop(bc.joint.pick_faces, qymcad_ui_state::BarAsk::JointPick));
                        }
                        // the Ground tool: a click on a part fixes or releases it.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyGround, &qymcad_i18n::tr("tb-ground-hint"), bc.joint.ground_pick) {
                            bc.ask.push(qymcad_ui_state::BarAsk::GroundPick);
                        }
                        // Group: fasten a set of parts to one another without pairwise joints.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyGroup, &qymcad_i18n::tr("j-group-tip"), bc.joint.group_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::GroupPick);
                        }
                        // Width: place a part midway between two walls.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyWidth, &qymcad_i18n::tr("j-width-tip"), bc.joint.width_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::WidthPick);
                        }
                        // Tangent: lay a cylinder onto a plane.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyTangent, &qymcad_i18n::tr("j-tangent-tip"), bc.joint.tangent_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::TangentPick);
                        }
                        // Relation: tie the degrees of freedom of two mates together.
                        if qymcad_ui_state::icon_tool(ui, qymcad_ui_state::IconId::AssemblyRelation, &qymcad_i18n::tr("j-relation-tip"), bc.joint.relation_pick.is_some()) {
                            bc.ask.push(qymcad_ui_state::BarAsk::RelationPick);
                        }
                    }
                }
            }
        });
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
