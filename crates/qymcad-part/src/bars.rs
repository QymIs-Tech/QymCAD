//! THE BARS OF THE PART: the tool bar, the options of a command, the bars of a section and of a pattern of components.

use qymcad_ui_state::Notice;
use super::*;

/// The top bar of a body-to-body boolean, in the style of the sketcher: the kind of operation, the hint
/// about picking body B, and a cancel. It is shown while body A is picked and the click on B is awaited
/// (`bool_pick`).
pub fn bool_tool_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    use qymcad_core::feature::FeatureKind;
    // CREATING: body A is picked, the click on B and the choice of kind are awaited
    if let Some((a, mut op)) = pc.boolean.pick {
        let ctx = ui.ctx().clone();
        // Enter before body B is taken has nothing to make; with B taken it applies
        let mut apply = pc.boolean.b.is_some() && ((!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(&ctx));
        if pc.boolean.b.is_none() {
            qymcad_ui_state::enter_not_ready(&ctx, pc.status, &qymcad_i18n::tr("tb-bool-pick-b"));
        }
        let a_name = pc.project.mesh_index(a).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi))).unwrap_or_else(|| "?".into());
        let b_name = pc.boolean.b.and_then(|b| pc.project.mesh_index(b)).map(|mi| qymcad_i18n::name(&pc.project.mesh_name(mi)));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab"));
            ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union"));
            ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect"));
            ui.separator();
            let said = match &b_name {
                Some(b) => qymcad_i18n::tr2("cmd-bool-a-b", "a", &a_name, "b", b),
                None => qymcad_i18n::tr1("cmd-bool-a-is", "name", &a_name),
            };
            ui.label(egui::RichText::new(said).color(pc.scheme.pal.hint()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-cancel-esc")).clicked() {
                    cancel = true;
                }
                if ui.add_enabled(b_name.is_some(), egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
            });
        });
        if cancel {
            (pc.boolean.pick, pc.boolean.b) = (None, None);
        } else {
            pc.boolean.pick = Some((a, op));
            if apply {
                apply_boolean(pc);
            }
        }
        return;
    }
    // EDITING: a double click on the boolean node changes its KIND in the bar (as reopening the command
    // does for an extrude)
    if let Some(ti) = pc.boolean.edit {
        let Some(FeatureKind::BodyBoolean { op, .. }) = pc.project.timeline.get(ti).map(|n| n.kind.clone()) else {
            pc.boolean.edit = None;
            return;
        };
        let mut op = op;
        let (mut done, mut changed) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::INTERSECT, qymcad_i18n::tr("bool-bodies-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("f-kind"));
            changed |= ui.selectable_value(&mut op, 0u8, qymcad_i18n::tr("f-cut-ab")).changed();
            changed |= ui.selectable_value(&mut op, 1u8, qymcad_i18n::tr("f-union")).changed();
            changed |= ui.selectable_value(&mut op, 2u8, qymcad_i18n::tr("f-intersect")).changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("f-done")).clicked() {
                    done = true;
                }
            });
        });
        if changed {
            // a change of kind is one step of undo, like any edit of a node
            qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("f-operation"));
            if let Some(FeatureKind::BodyBoolean { op: o, .. }) = pc.project.timeline.get_mut(ti).map(|n| &mut n.kind) {
                *o = op;
            }
            pc.project.timeline[ti].dirty = true;
            qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
            qymcad_ui_state::commit_edit(&mut pc.rebuild());
        }
        if done || pc.project.timeline.get(ti).is_none_or(|n| !matches!(n.kind, FeatureKind::BodyBoolean { .. })) {
            pc.boolean.edit = None;
        }
    }
}

/// THE SECTION CONTROL BAR: offset, tilts, flip, switching off.
///
/// This is PANEL DRAWING rather than a phase of the frame - yet it used to sit in the middle of `update`,
/// mixed in with the prologue, the keyboard and holding the selection. While panels live in the shared body,
/// "what the frame does" cannot be told from "what it draws", and editing one touches the other.
/// WHAT THE BAR OF THE TOOLS WAITING ON A PLANE TOUCHES: the section, the mirrored copy of a part and the pencil.
pub struct PlaneBarCtx<'a> {
    pub regen: &'a mut qymcad_ui_state::Rebuilding,
    pub section: &'a mut qymcad_ui_state::SectionTool,
    pub mirror: &'a mut qymcad_ui_state::MirrorParams,
    pub picking: &'a mut qymcad_ui_state::Picking,
    pub project: &'a mut qymcad_core::model::Project,
    pub edits: &'a mut qymcad_ui_state::Edits,
    pub active_path: &'a [qymcad_core::model::Id],
    pub status: &'a mut qymcad_ui_state::Notice,
    pub m3: &'a mut qymcad_ui_state::Measure3,
}

pub fn section_bar(pb: &mut PlaneBarCtx, ui: &mut egui::Ui) {
    // THE 3D MEASURING TOOL holds the hand as any other: its name, what is taken, the answer, the way out. Its words stood
    // only in the status line, and nothing said which elements were taken.
    if pb.m3.on {
        // the answer comes with the second element clicked: Enter before it has nothing to show
        if pb.m3.picks.len() < 2 {
            qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), pb.status, &qymcad_i18n::tr("m3-hint-short"));
        }
        let mut off = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::RULER, qymcad_i18n::tr("hotkey-part-i"))).strong());
            ui.separator();
            if pb.m3.picks.is_empty() {
                ui.label(egui::RichText::new(qymcad_i18n::tr("m3-hint-short")).weak());
            } else {
                let taken: Vec<String> = pb.m3.picks.iter().map(|p| p.what.clone()).collect();
                ui.label(taken.join(" - "));
                ui.separator();
                ui.label(egui::RichText::new(pb.status.words.as_str()).strong());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    off = true;
                }
            });
        });
        if off {
            pb.m3.clear();
            *pb.status = Notice::info(qymcad_i18n::tr("in-measure-off"));
        }
        return;
    }
    // A COMMAND WAITING FOR ITS SKETCH is in hand as well: its name, what to point at, the way out. It waited with only
    // the status line saying so, and the bar stood empty over a tool held
    if let Some(kind) = pb.picking.sketch_for() {
        let name = match kind {
            3 => "cmd-revolve",
            8 => "cmd-sweep",
            9 => "cmd-loft",
            _ => "cmd-extrude",
        };
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(qymcad_i18n::tr(name)).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("g-pick-sketch-for")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        // the sketch is pointed at, not typed: Enter before it has nothing to start
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), pb.status, &qymcad_i18n::tr("g-pick-sketch-for"));
        if cancel {
            *pb.status = Notice::info(pb.picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default());
            pb.picking.clear();
        }
        return;
    }
    let PlaneBarCtx { regen, section, mirror, picking, project, edits, active_path, status, .. } = pb;
    let (regen, section, mirror, picking, project, edits, active_path, status) =
        (&mut **regen, &mut **section, &mut **mirror, &mut **picking, &mut **project, &mut **edits, *active_path, &mut **status);
    // THE PENCIL WAITING FOR A PLANE shares the place too: what it is, what to click, and the way out, as the bar of any
    // tool in hand. Without it the words stood only in the status line and nothing on the bar said a tool was held.
    if picking.is_sketch_plane() {
        // the sketch opens on the click that names its plane: Enter before it has nothing to open
        qymcad_ui_state::enter_not_ready(&ui.ctx().clone(), status, &qymcad_i18n::tr("g-sketch-pick-plane"));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::PENCIL_SIMPLE, qymcad_i18n::tr("status-new-sketch"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("g-sketch-pick-plane")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            *status = Notice::info(picking.cancel_key().map(qymcad_i18n::tr).unwrap_or_default()); // the words Esc says for the same
            picking.clear();
        }
        return;
    }
    // THE MIRRORED COPY OF A PART shares the place: it too waits for a plane, and a tool that waits with no bar left a
    // person clicking at a view that gave no sign anything was held
    if mirror.waiting && mirror.part.is_none() {
        let ctx = ui.ctx().clone();
        qymcad_ui_state::enter_not_ready(&ctx, status, &qymcad_i18n::tr("tb-mirror-pick-part"));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::FLIP_HORIZONTAL, qymcad_i18n::tr("mp-title"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("tb-mirror-pick-part")).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked();
            });
        });
        if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            mirror.drop_part();
        }
        return;
    }
    if let Some(src) = mirror.part {
        let name = project.components.iter().find(|c| c.id == src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
        let ctx = ui.ctx().clone();
        let taken = mirror.at.is_some();
        // Enter makes the copy once a plane is taken, and says what is missing before
        let mut apply = taken && ((!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(&ctx));
        if !taken {
            qymcad_ui_state::enter_not_ready(&ctx, status, &qymcad_i18n::tr1("mp-pick", "name", &name));
        }
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::FLIP_HORIZONTAL, qymcad_i18n::tr("mp-title"))).strong());
            ui.separator();
            let said = if taken { qymcad_i18n::tr1("mp-plane-taken-of", "name", &name) } else { qymcad_i18n::tr1("mp-pick", "name", &name) };
            ui.label(egui::RichText::new(said).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                    cancel = true;
                }
                if ui.add_enabled(taken, egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
            });
        });
        // Esc puts it down from here: the window's ladder of Esc has no rung for a copy waiting on its plane
        cancel |= ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if cancel {
            mirror.drop_part();
        } else if apply {
            mirror_part_apply(mirror, project, edits, regen, active_path, status);
        }
        return;
    }
    // WAITING FOR THE PLANE the tool is already in hand, and the bar says so and what it wants, with the way out:
    // a tool taken with no bar left a person clicking at a view that gave no sign anything was held
    // ENTER OF THE COMMAND applies once a plane is taken and says what is missing before. Esc goes down the window's
    // ladder, which puts the tool down through `section_cancel` and takes a section still being placed with it.
    // Enter typed into the offset or a tilt closes that field and is the command's Enter as well: one press, as the bar
    // of every command takes it
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if section.pick && enter {
        if section.plane.is_some() {
            section.pick = false;
            *status = Notice::info(qymcad_i18n::tr("vp-section-on"));
        } else {
            *status = Notice::refusal(qymcad_i18n::tr1("cmd-not-ready", "ask", &qymcad_i18n::tr("tb-section-pick")));
        }
    }
    if section.plane.is_none() && section.pick {
        let mut off = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, qymcad_i18n::tr("sec-btn"))).strong());
            ui.separator();
            ui.label(egui::RichText::new(qymcad_i18n::tr("tb-section-pick")).weak());
            if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("sec-off-btn"))).clicked() {
                off = true;
            }
        });
        if off {
            section.pick = false;
            *status = Notice::info(qymcad_i18n::tr("tb-section-off"));
            qymcad_ui_state::invalidate(regen);
        }
        return;
    }
    // THE SECTION: the control bar (offset, tilts, flip, off) for as long as the section is active
    if section.plane.is_some() {
        let (mut changed, mut off, mut apply, mut cancel) = (false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::SQUARE_HALF, qymcad_i18n::tr("sec-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("sec-offset"));
            changed |= ui.add(egui::DragValue::new(&mut section.offset).speed(0.5).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
            ui.label(qymcad_i18n::tr("sec-tilt1"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[0]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            ui.label(qymcad_i18n::tr("sec-tilt2"));
            changed |= ui.add(egui::DragValue::new(&mut section.rot[1]).speed(0.5).range(-89.0..=89.0).suffix("°")).changed();
            if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("sec-flip-btn"))).on_hover_text(qymcad_i18n::tr("sec-flip-hint")).clicked() {
                if let Some((_, n)) = &mut section.plane {
                    n[0] = -n[0];
                    n[1] = -n[1];
                    n[2] = -n[2];
                }
                section.offset = -section.offset;
                changed = true;
            }
            if section.pick {
                // STILL BEING PLACED: the command's own way on and way back, as every tool has them
                if ui.button(format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                    apply = true;
                }
                if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-esc"))).clicked() {
                    cancel = true;
                }
            } else if ui.button(format!("{} {}", ph::X, qymcad_i18n::tr("sec-off-btn"))).clicked() {
                off = true;
            }
        });
        // the status line said "the section is on" after it was switched off here: it says so now
        if off {
            section.plane = None;
            section.pick = false;
            *status = Notice::info(qymcad_i18n::tr("tb-section-off"));
            changed = true;
        }
        if apply {
            section.pick = false;
            *status = Notice::info(qymcad_i18n::tr("vp-section-on"));
        }
        if cancel {
            section_cancel(section, regen);
            *status = Notice::info(qymcad_i18n::tr("tb-section-off"));
        }
        if changed {
            qymcad_ui_state::invalidate(regen);
        }
    }
}

/// ESC WITH THE SECTION TOOL PICKING: the tool is put down, a section still being placed with it; what the status line
/// says then.
pub fn section_escape(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding) -> String {
    section_cancel(section, regen);
    qymcad_i18n::tr("tb-section-off")
}

/// PUT THE SECTION TOOL DOWN: a section still being placed goes with it (Esc, Cancel, another tool taken); one
/// already applied stays until "Off".
pub fn section_cancel(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding) {
    if section.pick && section.plane.is_some() {
        section.plane = None;
        qymcad_ui_state::invalidate(regen);
    }
    section.pick = false;
    section.drag = false;
    section.drag_anchor = None;
}

/// THE TOP BAR OF A COMPONENT PATTERN (in an assembly) - the same interaction as the body pattern: the
/// count and the direction here, the step or the angle in an expression field at the geometry, Enter applies.
///
/// The bars used to be 645 lines sitting straight inside `update`: two thirds of what was left in the frame
/// after the earlier extractions. While a panel lives in the frame's body, "what the frame does" and "what it
/// draws" cannot be told apart, and editing one command touches the whole life cycle of the frame.
pub fn comp_array_bar(pc: &mut qymcad_ui_state::PartCtx, ui: &mut egui::Ui) {
    if pc.carr.mode == 0 {
        return;
    }
    let (mut apply, mut cancel) = (false, false);
    ui.horizontal_wrapped(|ui| {
        let title = if pc.carr.mode == 2 { qymcad_i18n::tr("cmd-comp-circ-array") } else { qymcad_i18n::tr("cmd-comp-lin-array") };
        ui.label(egui::RichText::new(format!("{} {title}", ph::STACK)).strong());
        ui.separator();
        let name = pc.project.components.iter().find(|c| c.id == pc.carr.src).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
        ui.label(egui::RichText::new(qymcad_i18n::tr1("cmd-source", "name", &name)).weak());
        ui.separator();
        ui.label(qymcad_i18n::tr("cmd-copies"));
        pc.arr.count = qymcad_ui_state::num_or_expr(
            &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
            ui,
            "carr_count",
            pc.arr.count as f64,
            qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
        ) as u32;
        ui.separator();
        if pc.carr.mode == 2 {
            ui.label(qymcad_i18n::tr("cmd-axis"));
            // AN AXIS OF ITS OWN, picked in the view as a pattern of bodies picks it: a datum axis, a straight edge or
            // a cylindrical face, kept as a reference the pattern follows. Reported behaviour: the axis of a circular
            // pattern of parts could only be X, Y or Z through the origin of the assembly.
            let picked = pc.project.datum_axes.iter().find(|d| d.id == pc.arr.axis).map(|d| qymcad_i18n::name(&d.name));
            if ui
                .selectable_label(pc.arr.axis_pick, format!("{} {}", ph::CROSSHAIR, picked.clone().unwrap_or_else(|| qymcad_i18n::tr("cmd-pick-axis"))))
                .on_hover_text(qymcad_i18n::tr("cmd-revolve-pick-axis3d"))
                .clicked()
            {
                pc.arr.axis_pick = !pc.arr.axis_pick;
                if pc.arr.axis_pick {
                    refresh_axis_edges(pc); // the straight edges of every visible body, as axis candidates
                    *pc.status = Notice::info(qymcad_i18n::tr("cmd-array-axis-hint"));
                }
            }
            if picked.is_some() {
                if ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("cmd-axis-drop")).clicked() {
                    (pc.arr.axis, pc.arr.axis_pick) = (0, false);
                }
            } else {
                ui.selectable_value(&mut pc.carr.axis, 0u8, "X");
                ui.selectable_value(&mut pc.carr.axis, 1u8, "Y");
                ui.selectable_value(&mut pc.carr.axis, 2u8, "Z");
            }
            ui.separator();
            let was = pc.arr.full;
            ui.checkbox(&mut pc.arr.full, qymcad_i18n::tr("cmd-full-circle-short"));
            if was != pc.arr.full {
                // the angle field appears and disappears with the checkbox, as in the body pattern
                pc.cmd.params = if pc.arr.full { vec![] } else { vec![qymcad_ui_state::CmdParam::new("cmd-angle", "cangle", 180.0, -3600.0, 3600.0)] };
            }
        } else {
            ui.label(qymcad_i18n::tr("cmd-direction"));
            ui.selectable_value(&mut pc.carr.dir, 0u8, "X");
            ui.selectable_value(&mut pc.carr.dir, 1u8, "Y");
            ui.selectable_value(&mut pc.carr.dir, 2u8, "Z");
            // THE SECOND AND THIRD DIRECTIONS, as a pattern of bodies has them: a grid of parts, each direction with
            // its count, its axis and its step. Reported behaviour: a linear pattern of parts ran one way only.
            ui.separator();
            // a direction ticked on opens as a row of two along the next axis, so the tick itself makes a grid
            if ui.checkbox(&mut pc.arr.two, qymcad_i18n::tr("cmd-dir2")).changed() && pc.arr.two {
                (pc.arr.count2, pc.arr.dir2) = (pc.arr.count2.max(2), (pc.carr.dir + 1) % 3);
            }
            if pc.arr.two {
                pc.arr.count2 = qymcad_ui_state::num_or_expr(
                    &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
                    ui,
                    "carr_count2",
                    pc.arr.count2 as f64,
                    qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                ) as u32;
                ui.selectable_value(&mut pc.arr.dir2, 0u8, "X");
                ui.selectable_value(&mut pc.arr.dir2, 1u8, "Y");
                ui.selectable_value(&mut pc.arr.dir2, 2u8, "Z");
                ui.separator();
                if ui.checkbox(&mut pc.arr.three, qymcad_i18n::tr("cmd-dir3")).changed() && pc.arr.three {
                    (pc.arr.count3, pc.arr.dir3) = (pc.arr.count3.max(2), 3 - pc.carr.dir.min(2) - pc.arr.dir2.min(2));
                }
                if pc.arr.three {
                    pc.arr.count3 = qymcad_ui_state::num_or_expr(
                        &mut qymcad_ui_state::ExprBarCtx { bar_exprs: pc.bar_exprs, project: pc.project, scheme: pc.scheme },
                        ui,
                        "carr_count3",
                        pc.arr.count3 as f64,
                        qymcad_ui_state::NumFormat { lo: 1.0, hi: 512.0, integer: true, suffix: "", nonzero: false },
                    ) as u32;
                    ui.selectable_value(&mut pc.arr.dir3, 0u8, "X");
                    ui.selectable_value(&mut pc.arr.dir3, 1u8, "Y");
                    ui.selectable_value(&mut pc.arr.dir3, 2u8, "Z");
                }
            } else {
                pc.arr.three = false; // the third direction only sits on top of the second
            }
            sync_comp_array_params(*pc.arr, pc.cmd);
        }
        if !pc.cmd.params.is_empty() {
            ui.separator();
            apply |= comp_array_fields(pc, ui, "bar");
        }
        ui.separator();
        // a step or an angle the field refuses closes Apply, as a field of every tool does
        let valid = cmd_exprs_valid(pc.cmd, pc.project) && !qymcad_ui_state::bar_field_bad("carr_count");
        apply |= confirm_button(&pc.scheme.pal, ui, valid, format!("{} {}", ph::CHECK, qymcad_i18n::tr("cmd-apply-enter")));
        cancel = refuse_button(&pc.scheme.pal, ui, format!("{} {}", ph::X, qymcad_i18n::tr("cmd-cancel-btn")));
    });
    if apply {
        apply_comp_array(pc);
    } else if cancel {
        *pc.carr = qymcad_ui_state::CompArrayCmd::default();
        pc.cmd.params.clear();
        *pc.status = Notice::info(qymcad_i18n::tr("msg-comp-array-cancelled"));
    }
}

/// REOPEN A PATTERN OF COMPONENTS for editing (a double click on its row): the command as it was applied - the count,
/// the axis, the step or the angle, and the second and third directions of a grid.
pub fn open_comp_array_edit(pc: &mut qymcad_ui_state::PartCtx, pid: Id) {
    use qymcad_core::model::CompPatternKind;
    use qymcad_ui_state::CmdParam;
    let Some(pat) = pc.project.comp_pattern(pid) else { return };
    let (mode, dir, axis) = match pat.kind {
        CompPatternKind::Linear { dir, .. } => (1u8, axis_of(dir), 2u8),
        CompPatternKind::Circular { dir, .. } => (2u8, 0u8, axis_of(dir)),
    };
    *pc.carr = qymcad_ui_state::CompArrayCmd { mode, src: pat.src, dir, axis, edit: pid };
    *pc.mode_3d = true;
    match pat.kind {
        CompPatternKind::Circular { angle, count, axis, .. } => {
            pc.arr.count = count.max(1);
            pc.arr.axis = axis;
            pc.arr.full = (angle - 360.0).abs() < 0.1;
            pc.cmd.params = if pc.arr.full { vec![] } else { vec![CmdParam::new("f-angle", "cangle", angle, -3600.0, 3600.0)] };
        }
        CompPatternKind::Linear { step, count, more, .. } => {
            pc.arr.count = count.max(1);
            pc.cmd.params = vec![CmdParam::new("f-pitch", "cstep", step, 0.01, 100000.0)];
            pc.arr.two = more[0].2 > 1;
            pc.arr.three = pc.arr.two && more[1].2 > 1;
            (pc.arr.count2, pc.arr.dir2, pc.arr.count3, pc.arr.dir3) = (more[0].2.max(1), axis_of(more[0].0), more[1].2.max(1), axis_of(more[1].0));
            sync_comp_array_params(*pc.arr, pc.cmd);
            for (key, s) in [("cstep2", more[0].1), ("cstep3", more[1].1)] {
                if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == key) {
                    (p.val, p.txt) = (s, qymcad_core::expr::fmt_num(s));
                }
            }
        }
    }
    *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-comp-array"));
}

/// THE STEP FIELDS OF A LINEAR PATTERN OF COMPONENTS follow its directions: a second and a third step appear with the
/// second and third direction ticked, and go with them, as a pattern of bodies has them.
pub fn sync_comp_array_params(arr: qymcad_ui_state::ArrayParams, cmd: &mut qymcad_ui_state::FeatCommand) {
    for (want, key, label) in [(arr.two, "cstep2", "f-pitch2"), (arr.two && arr.three, "cstep3", "f-pitch3")] {
        let has = cmd.params.iter().any(|p| p.key == key);
        if want && !has {
            cmd.params.push(qymcad_ui_state::CmdParam::new(label, key, 30.0, 0.01, 100000.0));
        } else if !want && has {
            cmd.params.retain(|p| p.key != key);
        }
    }
}

/// IS THIS TOOL IN HAND - open, OR waiting for the sketch it needs?
///
/// Reported behaviour: "press the tool and it stays highlighted, with the status asking for a sketch." It
/// did not: the wait lives in `Picking`, and the button lit from `armed` alone, so a tool that WAS waiting
/// looked untouched. A tool that acts and does not look taken is the same trouble as one that looks taken
/// and does not act.
pub(crate) fn tool_is_taken(bc: &qymcad_ui_state::BarCtx, kind: u8) -> bool {
    bc.armed.cmd_kind() == kind || bc.picking.sketch_for() == Some(kind)
}

mod feat_bar;
pub use feat_bar::*;
mod toolbar;
pub use toolbar::*;
mod options;
pub use options::*;
