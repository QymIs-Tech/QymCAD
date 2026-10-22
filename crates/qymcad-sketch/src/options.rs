//! THE OPTIONS BAR OF THE TOOL IN HAND of a sketch: its name, its switches and its fields, and the ways of a corner.

use qymcad_ui_state::Notice;

pub fn tool_options_bar(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    if qymcad_ui_state::edit_si(&*bc.project, &*bc.sketch_ses).is_none() {
        return;
    }
    // EDIT -> COPY, CUT, INSERT hold the hand too: the base point or the place is awaited. Their words stood only in the
    // status line, and the bar said "Select" over a click that would copy.
    // FILLET ALL holds the hand: its name, what to do, the way out; the radius is typed in the field at the shape
    if bc.picking.fillet_all() {
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr("tool-fillet")).strong());
            ui.separator();
            let ask = if bc.corner.at.is_some() { qymcad_i18n::tr("sk-enter-apply-expr") } else { qymcad_i18n::tr("g-fillet-all-hint") };
            ui.label(egui::RichText::new(ask).weak());
            qymcad_ui_state::end_of_bar(ui, |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        if bc.corner.at.is_none() {
            // the shape is clicked, not typed: Enter before it has nothing to round
            qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), bc.status, &qymcad_i18n::tr("g-fillet-all-hint"));
        }
        if cancel {
            bc.picking.clear();
            bc.corner.clear();
            *bc.status = Notice::info(qymcad_i18n::tr("in-fillet-all-cancelled"));
        }
        return;
    }
    // another tool taken puts the copy down: one thing in hand at a time
    if !matches!(*bc.armed, qymcad_ui_state::Armed::None) {
        bc.clip.drop_geom();
    }
    let waiting = match (bc.clip.geom_pending.as_ref(), bc.clip.geom_place) {
        (Some((_, true)), _) => Some(("menu-cut", "g-cut-base-point")),
        (Some((_, false)), _) => Some(("tool-copy", "g-copy-base-point")),
        (None, true) => Some(("win-insert", "g-insert-click")),
        (None, false) => None,
    };
    if let Some((name, ask)) = waiting {
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr(name)).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr(ask)).weak());
            qymcad_ui_state::end_of_bar(ui, |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        // a point is clicked, not typed: Enter before it has nothing to do
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), bc.status, &qymcad_i18n::tr(ask));
        if cancel {
            bc.clip.drop_geom();
            *bc.status = Notice::info(qymcad_i18n::tr(if name == "win-insert" { "in-insert-cancelled" } else { "in-copy-cancelled" }));
        }
        return;
    }
    ui.horizontal_wrapped(|ui| {
        // the name of the active tool or operation
        let name = if bc.armed.pattern().is_some() {
            if bc.armed.pattern() == Some(qymcad_ui_state::PatternKind::Linear) {
                qymcad_i18n::tr("tool-lin-array")
            } else {
                qymcad_i18n::tr("tool-circ-array")
            }
        } else if let Some(m) = bc.armed.moving() {
            match m {
                qymcad_ui_state::MoveKind::Move => qymcad_i18n::tr("tool-move"),
                qymcad_ui_state::MoveKind::Copy => qymcad_i18n::tr("tool-copy"),
                qymcad_ui_state::MoveKind::Rotate => qymcad_i18n::tr("tool-rotate"),
            }
        } else if bc.armed.click_tool().is_some() {
            match bc.armed.click_tool() {
                Some(qymcad_ui_state::ClickTool::Trim) => qymcad_i18n::tr("tool-trim"),
                Some(qymcad_ui_state::ClickTool::Extend) => qymcad_i18n::tr("tool-extend"),
                Some(qymcad_ui_state::ClickTool::Break) => qymcad_i18n::tr("tool-break"),
                Some(qymcad_ui_state::ClickTool::Corner) => qymcad_i18n::tr("tool-fillet"),
                Some(qymcad_ui_state::ClickTool::CornerChamfer) => qymcad_i18n::tr("tool-chamfer"),
                Some(qymcad_ui_state::ClickTool::Project) => qymcad_i18n::tr("tool-project-body"),
                _ => "—".to_string(),
            }
        } else if bc.armed.dimension().is_some() {
            qymcad_i18n::tr("tool-dim")
        } else if bc.armed.measuring() {
            qymcad_i18n::tr("tool-measure")
        } else if bc.armed.drawing().is_some() {
            match bc.armed.drawing() {
                Some(qymcad_ui_state::SketchShape::Line) => qymcad_i18n::tr("tool-line"),
                Some(qymcad_ui_state::SketchShape::Rect) => qymcad_i18n::tr("tool-rect"),
                Some(qymcad_ui_state::SketchShape::Circle) => qymcad_i18n::tr("tool-circle"),
                Some(qymcad_ui_state::SketchShape::Arc) => qymcad_i18n::tr("tool-arc"),
                Some(qymcad_ui_state::SketchShape::Point) => qymcad_i18n::tr("tool-point"),
                Some(qymcad_ui_state::SketchShape::Polygon) => qymcad_i18n::tr("tool-polygon"),
                Some(qymcad_ui_state::SketchShape::Slot) => qymcad_i18n::tr("tool-slot"),
                Some(qymcad_ui_state::SketchShape::Ellipse) => qymcad_i18n::tr("sk-ellipse"),
                Some(qymcad_ui_state::SketchShape::Spline) => qymcad_i18n::tr("sk-spline"),
                _ => "—".to_string(),
            }
        } else {
            match bc.armed.modify() {
                Some(qymcad_ui_state::EditTool::Fillet) => qymcad_i18n::tr("tool-fillet"),
                Some(qymcad_ui_state::EditTool::Chamfer) => qymcad_i18n::tr("tool-chamfer"),
                Some(qymcad_ui_state::EditTool::Offset) => qymcad_i18n::tr("tool-offset"),
                Some(qymcad_ui_state::EditTool::Mirror) => qymcad_i18n::tr("tool-mirror"),
                Some(qymcad_ui_state::EditTool::LinearPattern) => qymcad_i18n::tr("tool-lin-array-short"),
                Some(qymcad_ui_state::EditTool::CircularPattern) => qymcad_i18n::tr("tool-circ-array-short"),
                _ => qymcad_i18n::tr("tool-select"),
            }
        };
        ui.label(egui::RichText::new(name).strong());
        ui.separator();
        // ONLY the parameters of the active tool or operation
        if bc.armed.drawing().is_some() {
            ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short")).on_hover_text(qymcad_i18n::tr("opt-construction-hint"));
            let mut free = bc.tool.ties == qymcad_ui_state::Ties::Free;
            if ui.checkbox(&mut free, qymcad_i18n::tr("opt-no-ties-short")).on_hover_text(qymcad_i18n::tr("opt-no-ties-hint")).changed() {
                bc.tool.ties = if free { qymcad_ui_state::Ties::Free } else { qymcad_ui_state::Ties::Laid };
            }
            if bc.armed.drawing() == Some(qymcad_ui_state::SketchShape::Text) {
                ui.separator();
                ui.label(qymcad_i18n::tr("tool-text"));
                ui.add(egui::TextEdit::singleline(&mut bc.tool_prefs.text).desired_width(140.0));
                ui.label(qymcad_i18n::tr("opt-height-short"));
                bc.tool_prefs.text_h = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "text_h",
                    bc.tool_prefs.text_h,
                    qymcad_ui_state::NumFormat { lo: 0.1, hi: 1000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                // WHICH FONT IT WILL BE WRITTEN IN, named on the button itself. "Font..." alone said nothing:
                // a person pressed it to find out what they were about to write with.
                let named = qymcad_ui_state::font_label(&bc.tool_prefs.font, "opt-font");
                if ui.button(named).on_hover_text(qymcad_i18n::tr("opt-pick-font")).clicked() {
                    bc.ask.push(qymcad_ui_state::BarAsk::PickFont);
                }
                ui.checkbox(&mut bc.tool_prefs.text_note, qymcad_i18n::tr("opt-note")).on_hover_text(qymcad_i18n::tr("opt-text-as-note"));
            }
            if bc.armed.drawing() == Some(qymcad_ui_state::SketchShape::Polygon) {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-sides"));
                bc.tool_prefs.poly_n = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "poly_n",
                    bc.tool_prefs.poly_n as f64,
                    qymcad_ui_state::NumFormat { lo: 3.0, hi: 64.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 0u8, qymcad_i18n::tr("opt-polygon-inscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 1u8, qymcad_i18n::tr("opt-polygon-circumscribed"));
                ui.selectable_value(&mut bc.tool_prefs.poly_mode, 2u8, qymcad_i18n::tr("opt-by-edge"));
            }
            if bc.armed.drawing() == Some(qymcad_ui_state::SketchShape::Rect) {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rect-mode"));
                if ui.selectable_label(bc.tool_prefs.rect_mode == 0, qymcad_i18n::tr("opt-rect-2corners")).clicked() {
                    bc.tool_prefs.rect_mode = 0;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 1, qymcad_i18n::tr("opt-rect-centre")).clicked() {
                    bc.tool_prefs.rect_mode = 1;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.rect_mode == 2, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-rect-rotated")).clicked() {
                    bc.tool_prefs.rect_mode = 2;
                    bc.tool.pts.clear();
                }
            }
            if bc.armed.drawing() == Some(qymcad_ui_state::SketchShape::Circle) {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-circle-mode"));
                // centre and radius, two points, touching two curves, three points: the modes of one tool, as the arc's
                for &ModeWord { mode, word, hint } in CIRCLE_MODES {
                    let label = ui.selectable_label(bc.tool_prefs.circ_mode == mode, qymcad_i18n::tr(word));
                    let label = match hint {
                        Some(h) => label.on_hover_text(qymcad_i18n::tr(h)),
                        None => label,
                    };
                    if label.clicked() {
                        bc.tool_prefs.circ_mode = mode;
                        bc.tool.pts.clear();
                        bc.tool.circ_tan.clear();
                    }
                }
                if bc.tool_prefs.circ_mode == 2 {
                    ui.label(qymcad_i18n::tr("opt-radius"));
                    bc.tool_prefs.circ_r = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "sk_circle_touching_r",
                        bc.tool_prefs.circ_r,
                        qymcad_ui_state::NumFormat { lo: 0.01, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: true },
                    );
                }
            }
            if bc.armed.drawing() == Some(qymcad_ui_state::SketchShape::Arc) {
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-arc-mode"));
                if ui.selectable_label(bc.tool_prefs.arc_mode == 0, qymcad_i18n::tr("opt-arc-cse")).clicked() {
                    bc.tool_prefs.arc_mode = 0;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 1, qymcad_i18n::tr("opt-rect-3pt")).on_hover_text(qymcad_i18n::tr("opt-arc-3pt")).clicked() {
                    bc.tool_prefs.arc_mode = 1;
                    bc.tool.pts.clear();
                }
                if ui.selectable_label(bc.tool_prefs.arc_mode == 2, qymcad_i18n::tr("opt-tangent")).on_hover_text(qymcad_i18n::tr("opt-arc-smooth-hint")).clicked() {
                    bc.tool_prefs.arc_mode = 2;
                    bc.tool.pts.clear();
                }
            }
        } else if bc.armed.dimension().is_some() {
            ui.label(egui::RichText::new(qymcad_i18n::tr("opt-dim-hint")).weak());
        } else if bc.armed.click_tool().is_some() {
            if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Corner) || bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::CornerChamfer) {
                if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::CornerChamfer) {
                    chamfer_modes(bc, ui);
                } else {
                    fillet_modes(bc, ui);
                }
                ui.label(qymcad_i18n::tr(if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Corner) {
                    qymcad_ui_state::fillet_label(bc.tool_prefs.fillet_by)
                } else {
                    qymcad_ui_state::chamfer_d1_label(bc.tool_prefs.chamfer_mode)
                }));
                // with a corner clicked, the most it takes; short of it, the largest the field ever takes
                let corner_hi = bc
                    .corner
                    .at
                    .and_then(|(si, pid, ch)| {
                        (pid != 0).then(|| qymcad_ui_state::corner_limit_in(&*bc.project, si, pid, qymcad_ui_state::corner_tool(ch, bc.tool_prefs), bc.tool_prefs.fillet_by)).flatten()
                    })
                    .map_or(10000.0, |l| l * (1.0 - 1e-9));
                bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "sk_fillet",
                    bc.tool_prefs.fillet,
                    qymcad_ui_state::NumFormat { lo: 0.01, hi: corner_hi, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::CornerChamfer) {
                    chamfer_second_field(bc, ui);
                }
            }
            if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Project) {
                // WHAT IS TAKEN: one edge under the cursor, or the whole outline of the sketch's host face
                if ui.selectable_label(!bc.tool.proj_face, qymcad_i18n::tr("opt-edge")).on_hover_text(qymcad_i18n::tr("opt-project-edge-hint")).clicked() {
                    bc.tool.proj_face = false;
                }
                if ui.selectable_label(bc.tool.proj_face, qymcad_i18n::tr("opt-face-outline")).on_hover_text(qymcad_i18n::tr("opt-face-outline-hint")).clicked() {
                    bc.tool.proj_face = true;
                }
            }
            if bc.armed.click_tool() == Some(qymcad_ui_state::ClickTool::Extend) {
                ui.checkbox(&mut bc.tool_prefs.extend_both, qymcad_i18n::tr("opt-extend-both")).on_hover_text(qymcad_i18n::tr("opt-extend-both-hint"));
            }
            let h = match bc.armed.click_tool() {
                Some(qymcad_ui_state::ClickTool::Trim) => &qymcad_i18n::tr("opt-trim-hint"),
                // the line held, the tool says what it waits for next
                Some(qymcad_ui_state::ClickTool::Extend) if bc.tool.extend.is_some() => &qymcad_i18n::tr("opt-extend-to-hint"),
                Some(qymcad_ui_state::ClickTool::Extend) => &qymcad_i18n::tr("opt-extend-hint"),
                Some(qymcad_ui_state::ClickTool::Break) => &qymcad_i18n::tr("opt-break-hint"),
                Some(qymcad_ui_state::ClickTool::Corner) => &qymcad_i18n::tr("opt-fillet-hint"),
                Some(qymcad_ui_state::ClickTool::CornerChamfer) => &qymcad_i18n::tr("opt-chamfer-hint"),
                Some(qymcad_ui_state::ClickTool::Project) => &qymcad_i18n::tr("opt-project-hint"),
                _ => "",
            };
            ui.label(egui::RichText::new(h).weak());
        } else if bc.armed.pattern().is_some() {
            // the pattern: the parameters + a hint (the preview is live, Enter applies)
            ui.label(qymcad_i18n::tr("opt-count"));
            bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                ui,
                "skpat_count",
                bc.sk_pat.count as f64,
                qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
            ) as u32;
            if bc.armed.pattern() == Some(qymcad_ui_state::PatternKind::Linear) {
                ui.label(qymcad_i18n::tr("opt-step-x"));
                bc.sk_pat.dx = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_dx",
                    bc.sk_pat.dx,
                    qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                ui.label(qymcad_i18n::tr("opt-step-y"));
                bc.sk_pat.dy = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_dy",
                    bc.sk_pat.dy,
                    qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                );
                ui.separator();
                ui.label(qymcad_i18n::tr("opt-rows"));
                bc.sk_pat.count2 = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_count2",
                    bc.sk_pat.count2 as f64,
                    qymcad_ui_state::NumFormat { lo: 1.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                if bc.sk_pat.count2 > 1 {
                    ui.label(qymcad_i18n::tr("opt-row-step-x"));
                    bc.sk_pat.dx2 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dx2",
                        bc.sk_pat.dx2,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    ui.label(qymcad_i18n::tr("opt-row-step-y"));
                    bc.sk_pat.dy2 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dy2",
                        bc.sk_pat.dy2,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                }
            } else {
                ui.label(qymcad_i18n::tr("cmd-angle"));
                bc.sk_pat.angle = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                    ui,
                    "skpat_angle",
                    bc.sk_pat.angle,
                    qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°", nonzero: false },
                );
                ui.label(egui::RichText::new(if bc.pat.center.is_some() { qymcad_i18n::tr("opt-centre-set") } else { qymcad_i18n::tr("opt-click-rotation-centre") }).weak());
            }
            ui.label(egui::RichText::new(if bc.pat.edit.is_some() { qymcad_i18n::tr("opt-enter-update") } else { qymcad_i18n::tr("opt-enter-apply") }).weak());
            // APPLY AND CANCEL AS BUTTONS, as every command bar has them: a pattern can be finished and refused with the mouse
            ui.separator();
            let ctx = ui.ctx().clone();
            if ui.add_enabled(qymcad_ui_state::bar_fields_valid(&ctx), egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                qymcad_ui_state::bar_apply_press(&ctx);
            }
            if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                bc.ask.push(qymcad_ui_state::BarAsk::CancelAllTools(qymcad_ui_state::Then::Nothing));
            }
        } else {
            match bc.armed.modify() {
                Some(qymcad_ui_state::EditTool::Fillet | qymcad_ui_state::EditTool::Chamfer) => {
                    if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Chamfer) {
                        chamfer_modes(bc, ui);
                    } else {
                        fillet_modes(bc, ui);
                    }
                    ui.label(qymcad_i18n::tr(if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Fillet) {
                        qymcad_ui_state::fillet_label(bc.tool_prefs.fillet_by)
                    } else {
                        qymcad_ui_state::chamfer_d1_label(bc.tool_prefs.chamfer_mode)
                    }));
                    bc.tool_prefs.fillet = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "sk_fillet",
                        bc.tool_prefs.fillet,
                        qymcad_ui_state::NumFormat { lo: 0.01, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    if bc.armed.modify() == Some(qymcad_ui_state::EditTool::Chamfer) {
                        chamfer_second_field(bc, ui);
                    }
                }
                Some(qymcad_ui_state::EditTool::Offset) => {
                    // the box at the copy types the distance: the bar shows it rather than its own old text over it
                    if bc.sel_sk.offset.at.is_some() && !ui.memory(|m| m.has_focus(qymcad_ui_state::bar_field_id("sk_offset"))) {
                        bc.bar_exprs.remove("sk_offset");
                    }
                    ui.label(qymcad_i18n::tr("opt-distance"));
                    bc.tool_prefs.offset = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "sk_offset",
                        bc.tool_prefs.offset,
                        qymcad_ui_state::NumFormat { lo: -10000.0, hi: 10000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: true },
                    );
                }
                Some(qymcad_ui_state::EditTool::LinearPattern) => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_count",
                        bc.sk_pat.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.label(qymcad_i18n::tr("opt-step-x"));
                    bc.sk_pat.dx = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dx",
                        bc.sk_pat.dx,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                    ui.label(qymcad_i18n::tr("opt-step-y"));
                    bc.sk_pat.dy = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_dy",
                        bc.sk_pat.dy,
                        qymcad_ui_state::NumFormat { lo: -100000.0, hi: 100000.0, integer: false, suffix: &qymcad_i18n::tr("unit-mm-suffix"), nonzero: false },
                    );
                }
                Some(qymcad_ui_state::EditTool::CircularPattern) => {
                    ui.label(qymcad_i18n::tr("opt-count"));
                    bc.sk_pat.count = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_count",
                        bc.sk_pat.count as f64,
                        qymcad_ui_state::NumFormat { lo: 2.0, hi: 200.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.label(qymcad_i18n::tr("cmd-angle"));
                    bc.sk_pat.angle = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: &mut *bc.bar_exprs, project: &*bc.project, scheme: &*bc.scheme },
                        ui,
                        "skpat_angle",
                        bc.sk_pat.angle,
                        qymcad_ui_state::NumFormat { lo: -360.0, hi: 360.0, integer: false, suffix: "°", nonzero: false },
                    );
                }
                Some(qymcad_ui_state::EditTool::Mirror) => {
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-mirror-axis-hint")).weak());
                }
                _ => {
                    ui.checkbox(&mut bc.tool.construction, qymcad_i18n::tr("opt-construction-short"));
                    ui.label(egui::RichText::new(qymcad_i18n::tr("opt-select-hint")).weak());
                }
            }
        }
        qymcad_ui_state::end_of_bar(ui, |ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("opt-selected-n", "n", &bc.sel_sk.items.len().to_string())).weak());
        });
    });
}

/// A MODE OF A DRAWING TOOL ON ITS BAR: its number, its word, and the hint over it.
struct ModeWord {
    mode: u8,
    word: &'static str,
    hint: Option<&'static str>,
}

/// THE MODES OF THE CIRCLE: a centre and a radius, the two ends of a diameter, touching two curves, three points.
const CIRCLE_MODES: &[ModeWord] = &[
    ModeWord { mode: 0, word: "opt-circle-centre-radius", hint: None },
    ModeWord { mode: 1, word: "opt-circle-2pt", hint: Some("opt-circle-diameter-ends") },
    ModeWord { mode: 2, word: "opt-tangent-m", hint: Some("opt-tangent-hint") },
    ModeWord { mode: 3, word: "opt-rect-3pt", hint: Some("opt-circle-3pt-hint") },
];

/// HOW A SKETCH CHAMFER IS GIVEN, on the bar of the chamfer: equal legs, two legs, or a leg and an angle - the words of
/// the chamfer of a part - and, for the last two, the second value beside the first leg. Reported (issue #35): the
/// sketch chamfer took one distance only, and a chamfer of 5 x 3 or of 5 at 30 deg had to be built by hand.
/// The ways of a sketch fillet, before its field: by its radius, its chord or the length of its arc.
fn fillet_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    use qymcad_core::model::FilletBy;
    for by in [FilletBy::Radius, FilletBy::Chord, FilletBy::ArcLength] {
        if ui.selectable_label(bc.tool_prefs.fillet_by == by, qymcad_i18n::tr(qymcad_ui_state::fillet_label(by))).clicked() {
            bc.tool_prefs.fillet_by = by;
            // the field of a corner already open takes the keys back: the click on the word took them
            bc.corner.focus |= bc.corner.at.is_some();
        }
    }
    ui.separator();
}

/// A WAY OF A SKETCH CHAMFER ON ITS BAR: the mode and its word.
struct ChamferWay {
    mode: qymcad_core::feature::ChamferMode,
    word: &'static str,
}

/// THE WAYS OF A SKETCH CHAMFER: equal legs, two legs, a leg and an angle - the words of the chamfer of a part.
const CHAMFER_WAYS: [ChamferWay; 3] = [
    ChamferWay { mode: qymcad_core::feature::ChamferMode::Symmetric, word: "cmd-symmetric" },
    ChamferWay { mode: qymcad_core::feature::ChamferMode::TwoDist, word: "cmd-two-distances" },
    ChamferWay { mode: qymcad_core::feature::ChamferMode::DistAngle, word: "cmd-leg-angle" },
];

/// The modes of a sketch chamfer, before its fields: the fields of one chamfer stand together after them.
fn chamfer_modes(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
    for way in CHAMFER_WAYS {
        if ui.selectable_label(bc.tool_prefs.chamfer_mode == way.mode, qymcad_i18n::tr(way.word)).clicked() {
            qymcad_ui_state::set_chamfer_mode(bc.tool_prefs, way.mode);
            // the field of a corner already open takes the keys back: the click on the word took them, and the values
            // typed after choosing two distances on a chamfer of two lines chosen before the tool went nowhere
            bc.corner.focus |= bc.corner.at.is_some();
        }
    }
    ui.separator();
}

/// The second field of a sketch chamfer of two legs or of a leg and an angle, right after the first.
fn chamfer_second_field(bc: &mut qymcad_ui_state::BarCtx, ui: &mut egui::Ui) {
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
