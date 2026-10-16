//! THE BARS OF A JOINT: editing it, making it, the tool bar.

use qymcad_ui_state::Notice;
use super::*;

/// The top bar of JOINT EDIT MODE (a double click on the glyph), in the style of the tool bars: the
/// joint's name, the kind of anchor, and a "swap anchor" button that drops down A and B for picking a
/// new face, edge or vertex on the fly. The parameters (angle, offset, limits, flip, global) live in
/// the popup at the glyph (`joint_popup`). One tool-command experience, with no trips to the right-hand
/// panel.
pub fn joint_edit_bar(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui) {
    let Some(jid) = jc.joint.edit else { return };
    if !matches!(jc.workbench, qymcad_ui_state::Workbench::Assembly) || !jc.mode_3d {
        return;
    }
    let Some(j) = jc.project.joints.iter().find(|x| x.id == jid).cloned() else {
        jc.joint.edit = None;
        return;
    };
    let desc_a = jc
        .project
        .connector(j.a)
        .map(|c| qymcad_ui_state::anchor_desc(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, &c.anchor))
        .unwrap_or_default();
    let desc_b = jc
        .project
        .connector(j.b)
        .map(|c| qymcad_ui_state::anchor_desc(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, &c.anchor))
        .unwrap_or_default();
    let repick = jc.joint.edit_repick;
    let (creating, mut finish) = (jc.joint.creating, None::<bool>);
    let mut done = false;
    let mut set_repick: Option<Option<(Id, bool)>> = None;
    let mut set_kind: Option<qymcad_core::feature::JointKind> = None;
    let (mut flip_axis, mut swap_roles) = (false, false);
    // THE PLACE BELONGS TO THE SHELL. This used to open its own top panel, and the frame order was
    // then held by the order of lines rather than written down anywhere.
    {
        let ui = &mut *ui;
        use qymcad_core::feature::JointKind;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::LINK, qymcad_i18n::tr1("jt-editing", "name", &qymcad_i18n::name(&j.name)))).strong());
            // A JOINT WITHOUT AN ANCHOR SAYS SO OUT LOUD. The solver silently drops such joints from
            // the problem: the assembly looks assembled, the parts do not move, and there is no
            // explanation. That was reported as parts not moving with the direction being wrong no
            // matter what was picked.
            // The reason comes from A SINGLE source (`joint_faults`), which also speaks through the
            // solver report and the list of joints: two places with the same words and different
            // truths is a thing already been through.
            if let Some((_, why)) = jc.project.joint_faults().iter().find(|(id, _)| *id == jid) {
                qymcad_ui_state::say(ui, &jc.scheme.pal, &qymcad_ui_state::Notice::error(qymcad_i18n::tr(why)), qymcad_ui_state::NoticeSize::Line)
                    .on_hover_text(qymcad_i18n::tr(&format!("{why}-hint")));
            }
            ui.separator();
            // changing the KIND of a joint right in the edit bar (the anchors are kept).
            ui.label(qymcad_i18n::tr("j-kind"));
            let mut nk = j.kind;
            egui::ComboBox::from_id_salt("joint_edit_kind").selected_text(qymcad_i18n::tr(j.kind.label())).show_ui(ui, |ui| {
                for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                    ui.selectable_value(&mut nk, kk, qymcad_i18n::tr(kk.label()));
                }
            });
            if nk != j.kind {
                set_kind = Some(nk);
            }
            ui.separator();
            ui.label(qymcad_i18n::tr("j-anchor"));
            ui.label(egui::RichText::new(qymcad_i18n::tr("j-anchor-inferred")).weak()).on_hover_text(qymcad_i18n::tr("j-anchor-inferred-hint"));
            ui.separator();
            ui.menu_button(format!("{} {}", ph::MAGNET, qymcad_i18n::tr("jt-swap-anchor")), |ui| {
                if ui.button(format!("A: {desc_a}")).clicked() {
                    set_repick = Some(Some((jid, false)));
                    ui.close();
                }
                if ui.button(format!("B: {desc_b}")).clicked() {
                    set_repick = Some(Some((jid, true)));
                    ui.close();
                }
            });
            // THE JOINT CONSOLE: flip the main axis, and swap the roles of the parts.
            //
            // Both handles fix what almost never comes out right the first time and used to be
            // correctable only by recreating the joint - along with its drivers, its limits and its
            // name.
            ui.separator();
            if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, qymcad_i18n::tr("jt-flip-axis"))).on_hover_text(qymcad_i18n::tr("jt-flip-axis-hint")).clicked() {
                flip_axis = true;
            }
            if ui.button(format!("{} {}", ph::SWAP, qymcad_i18n::tr("jt-swap-roles"))).on_hover_text(qymcad_i18n::tr("jt-swap-roles-hint")).clicked() {
                swap_roles = true;
            }
            if let Some((_, is_b)) = repick {
                let t = qymcad_i18n::tr("j-place-lower");
                ui.label(egui::RichText::new(qymcad_i18n::tr2("jt-click-new", "what", &t, "side", if is_b { "B" } else { "A" })).color(jc.scheme.pal.hint()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !creating {
                    if ui.button(qymcad_i18n::tr("j-done-esc")).clicked() {
                        done = true;
                    }
                } else {
                    if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                        finish = Some(false);
                    }
                    if ui.button(qymcad_i18n::tr("cmd-apply-enter")).clicked() {
                        finish = Some(true);
                    }
                }
            });
        });
    }
    let ctx = ui.ctx().clone();
    if creating && ((!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(&ctx)) {
        finish = Some(true);
    }
    if let Some(v) = set_repick {
        jc.joint.edit_repick = v;
    }
    if let Some(k) = set_kind {
        if qymcad_ui_state::change_joint_kind(jc.project, jc.status, jid, k) {
            qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
        }
    }
    if flip_axis {
        joint_hud_flip_axis(jc, jid);
    }
    if swap_roles {
        joint_hud_swap_roles(jc, jid);
    }
    if done {
        qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    }
    if let Some(keep) = finish {
        joint_create_finish(jc, keep);
    }
}

/// FINISH A JOINT JUST MADE: kept, its open operation closes as one step «Joint»; not kept, the operation is rolled back
/// and the document is as it was before the first pick. Nothing happens when no joint is being made.
pub fn joint_create_finish(jc: &mut qymcad_ui_state::JointCtx, keep: bool) {
    if !std::mem::take(&mut jc.joint.creating) {
        return;
    }
    if keep {
        qymcad_ui_state::close_edit(jc.edits, jc.project); // closed, not committed: committing drops the asked rebuild
    } else {
        qymcad_ui_state::abort_edit(&mut jc.rebuild());
    }
    qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    // A JOINT KEPT LEAVES THE TOOL IN HAND for the next pair, as a mate dialog does; one taken away puts it down
    if keep {
        (jc.joint.pick_faces, jc.joint.pick_first) = (true, None);
        *jc.status = Notice::info(qymcad_i18n::tr("j-created"));
    }
}

/// Esc WITH THE JOINT CONSOLE OPEN: a joint just made is taken away, an edited one is left as edited.
pub fn joint_edit_leave(jc: &mut qymcad_ui_state::JointCtx) {
    if jc.joint.creating {
        joint_create_finish(jc, false);
    } else {
        qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    }
}

pub fn joint_tool_bar(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui) {
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    use qymcad_core::feature::JointKind;
    // the tangency tool bar - a hint and a way out; once the second surface is picked the condition stands as a preview,
    // and the bar offers Apply and Cancel
    if let Some(sel) = jc.joint.tangent_pick.clone() {
        let made = sel.len() >= 2;
        let (mut cancel, mut apply) = (false, false);
        egui::Panel::top("tangent_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::CIRCLE_HALF_TILT, qymcad_i18n::tr("j-tangent-made"))).strong());
                ui.separator();
                let hint = if made {
                    qymcad_i18n::tr("j-tangent-made-ok")
                } else if sel.is_empty() {
                    qymcad_i18n::tr("j-tangent-pick")
                } else {
                    qymcad_i18n::tr("j-tangent-second")
                };
                ui.label(egui::RichText::new(hint).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if made {
                        if ui.button(qymcad_i18n::tr("cmd-cancel-btn")).clicked() {
                            cancel = true;
                        }
                        if ui.button(qymcad_i18n::tr("cmd-apply-enter")).clicked() {
                            apply = true;
                        }
                    } else if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if made {
            apply |= (!ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) || qymcad_ui_state::bar_enter_take(ctx);
        } else {
            // the tangent is made by its second pick: Enter before it has nothing to make
            qymcad_ui_state::enter_not_ready(ctx, jc.status, &qymcad_i18n::tr(if sel.is_empty() { "j-tangent-pick" } else { "j-tangent-second" }));
        }
        if made && (apply || cancel) {
            tangent_finish(jc, apply && !cancel);
        } else if cancel {
            jc.joint.tangent_pick = None;
            *jc.status = Notice::info(qymcad_i18n::tr("j-tangent-off"));
        }
        return;
    }
    // the width tool bar - how many anchors are shown, a "make it" button and a way out.
    if let Some(sel) = jc.joint.width_pick.clone() {
        let (mut make, mut cancel) = (false, false);
        egui::Panel::top("width_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::ARROWS_OUT_LINE_HORIZONTAL, qymcad_i18n::tr("j-width-made"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr1("j-width-picked", "n", &sel.len().to_string())).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                    if ui.button(qymcad_i18n::tr("cmd-apply-enter")).clicked() {
                        make = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            make = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if make {
            width_pick_confirm(jc);
        } else if cancel {
            jc.joint.width_pick = None;
            *jc.status = Notice::info(qymcad_i18n::tr("j-width-off"));
        }
        return;
    }
    // the group tool bar - how many are picked, a "group them" button and a way out. The same shape
    // as every other tool: the hint on top, Enter confirms, Esc cancels.
    if let Some(sel) = jc.joint.group_pick.clone() {
        let (mut make, mut cancel) = (false, false);
        egui::Panel::top("group_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::SELECTION_ALL, qymcad_i18n::tr("j-group-made"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr1("j-group-picked", "n", &sel.len().to_string())).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                    if ui.button(qymcad_i18n::tr("cmd-apply-enter")).clicked() {
                        make = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            make = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if make {
            group_pick_confirm(jc);
        } else if cancel {
            jc.joint.group_pick = None;
            *jc.status = Notice::info(qymcad_i18n::tr("j-group-off"));
        }
        return;
    }
    // the grounding tool bar - a hint and a way out
    if jc.joint.ground_pick {
        let mut cancel = false;
        egui::Panel::top("ground_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::ANCHOR, qymcad_i18n::tr("jt-ground-btn"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr("j-ground-click")).color(jc.scheme.pal.hint()));
                // what the clicks have grounded, named in the bar and not only in the status line
                let grounded: Vec<String> = jc.project.components.iter().filter(|c| c.parent.is_some() && jc.project.is_grounded(c.id)).map(|c| qymcad_i18n::name(&c.name)).collect();
                if !grounded.is_empty() {
                    ui.separator();
                    ui.label(qymcad_i18n::tr1("j-grounded-list", "names", &grounded.join(", ")));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-done-esc")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        // grounding is done by the click itself: Enter has nothing to make
        qymcad_ui_state::enter_not_ready(ui.ctx(), jc.status, &qymcad_i18n::tr("j-ground-click"));
        if cancel {
            jc.joint.ground_pick = false;
            *jc.status = Notice::info(qymcad_i18n::tr("j-ground-off"));
        }
        return;
    }
    // THE ANCHOR TOOL BAR: the kind of anchor and a hint about where to click.
    if jc.joint.conn_pick {
        let mut cancel = false;
        egui::Panel::top("conn_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("j-conn-new"))).strong());
                ui.separator();
                ui.label(qymcad_i18n::tr("j-anchor"));
                ui.label(egui::RichText::new(qymcad_i18n::tr("j-anchor-inferred")).weak()).on_hover_text(qymcad_i18n::tr("j-anchor-inferred-hint"));
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr("j-conn-pick")).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if cancel {
            jc.joint.conn_pick = false;
            *jc.status = Notice::info(qymcad_i18n::tr("j-conn-off"));
        }
        return;
    }
    // THE RELATION TOOL BAR: the kind, the number, the direction, Enter/Esc.
    if jc.joint.relation_pick.is_some() {
        let (mut cancel, mut done) = (false, false);
        let pick = jc.joint.relation_pick.clone().unwrap_or_default();
        let need = relation_picks_needed(pick.kind);
        let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
        egui::Panel::top("relation_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::GEAR_SIX, qymcad_i18n::tr("j-relation-btn"))).strong());
                ui.separator();
                ui.label(qymcad_i18n::tr("j-kind"));
                let mut k = pick.kind;
                egui::ComboBox::from_id_salt("relation_bar_kind").selected_text(qymcad_i18n::tr(k.label())).show_ui(ui, |ui| {
                    for kk in [
                        qymcad_core::feature::RelationKind::Gear,
                        qymcad_core::feature::RelationKind::RackPinion,
                        qymcad_core::feature::RelationKind::Screw,
                        qymcad_core::feature::RelationKind::Linear,
                    ] {
                        ui.selectable_value(&mut k, kk, qymcad_i18n::tr(kk.label()));
                    }
                });
                ui.separator();
                // THE NUMBER MEANS DIFFERENT THINGS FOR DIFFERENT KINDS, and the caption must say so:
                // for gears and linear relations it is a ratio, for a rack and a screw it is the travel
                // per turn in millimetres.
                ui.label(qymcad_i18n::tr(if pick.kind.value_is_per_turn() { "j-relation-per-turn" } else { "j-relation-ratio" }));
                let mut v = pick.value;
                ui.add(egui::DragValue::new(&mut v).speed(0.05));
                let mut rev = pick.reversed;
                ui.checkbox(&mut rev, qymcad_i18n::tr("j-relation-reverse"));
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string())).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                    if ui.add_enabled(have >= need, egui::Button::new(qymcad_i18n::tr("cmd-apply-enter"))).clicked() {
                        done = true;
                    }
                });
                if let Some(p) = jc.joint.relation_pick.as_mut() {
                    p.set(k, v);
                    p.reversed = rev;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            done = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if cancel {
            jc.joint.relation_pick = None;
            *jc.status = Notice::info(qymcad_i18n::tr("j-relation-off"));
        } else if done {
            relation_pick_confirm(jc);
        }
        return;
    }
    if !jc.joint.pick_faces {
        return;
    }
    // WHAT IS BEING ASKED FOR. This used to name the chosen mode - face, edge, vertex: the sort of
    // anchor was declared first and the hint merely repeated that choice back. Now the sort is inferred
    // under the cursor, and what must be asked for is exactly A PLACE ON THE PART.
    let target = if jc.joint.anchor_mode == 3 { qymcad_i18n::tr("j-origin-lower") } else { qymcad_i18n::tr("j-place-lower") };
    let target = &target;
    let hint = if jc.joint.pick_first.is_some() { qymcad_i18n::tr1("jt-click-b", "what", target) } else { qymcad_i18n::tr1("jt-click-a", "what", target) };
    // a joint is laid by its second pick: Enter before it has nothing to make
    qymcad_ui_state::enter_not_ready(ctx, jc.status, &hint);
    let mut cancel = false;
    // THE PLACE BELONGS TO THE SHELL. This used to open its own top panel, and the frame order was
    // then held by the order of lines rather than written down anywhere.
    {
        let ui = &mut *ui;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{} {}", ph::MAGNET, qymcad_i18n::tr("jt-joint-btn"))).strong());
            ui.separator();
            ui.label(qymcad_i18n::tr("j-kind"));
            let mut k = jc.joint.new_kind;
            egui::ComboBox::from_id_salt("joint_bar_kind").selected_text(qymcad_i18n::tr(k.label())).show_ui(ui, |ui| {
                // THERE IS ONE SET OF KINDS: a joint states the degrees of freedom, and that is all.
                // The former "assembly mates" duplicated the mechanical ones (a mate equals planar,
                // concentricity equals cylindrical) but were solved along another path and behaved
                // differently.
                // ALL OF THE KINDS ARE HERE. The rigid one used to be missing: it was created only by
                // its own button in the workbench panel, and removing the buttons would have made that
                // kind unreachable altogether.
                for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                    ui.selectable_value(&mut k, kk, qymcad_i18n::tr(kk.label()));
                }
            });
            if k != jc.joint.new_kind {
                jc.joint.new_kind = k;
                // THE SORT OF ANCHOR IS NO LONGER CHOSEN IN ADVANCE - neither by a person nor on their
                // behalf by the kind of joint. This used to read "coaxial kinds prefer an edge, the rest
                // a face", and changing the kind silently moved the mode: the next click landed
                // somewhere other than where it was aimed.
                jc.joint.anchor_mode = 0;
            }
            ui.separator();
            ui.label(qymcad_i18n::tr("j-anchor"));
            // THE SORT OF ANCHOR IS INFERRED UNDER THE CURSOR.
            //
            // There used to be three switches here - face, edge, vertex. A professional CAD has none:
            // the anchor point is derived from the geometry under the cursor, and that is not a
            // convenience but a condition of working at all - while the sort was declared in advance, a
            // click regularly produced an anchor other than the one intended.
            ui.label(egui::RichText::new(qymcad_i18n::tr("j-anchor-inferred")).weak()).on_hover_text(qymcad_i18n::tr("j-anchor-inferred-hint"));
            // BY ORIGINS IS A FOURTH SORT OF ANCHOR, NOT A SEPARATE BUTTON IN THE PROPERTIES PANEL.
            //
            // This way of working used to live in the right-hand panel: two drop-down lists for A and B
            // and a create button. It went around everything that makes a command a command - picking
            // by click, a preview, Enter/Esc - and taught the wrong habits. It is a useful way (parts
            // with no convenient faces, a quick rough assembly), so it was not thrown out but moved
            // HERE: the same picking by clicking a part, the same bar, the same cancel.
            // A TOGGLE, NOT A ONE-WAY CHOICE: it used to be possible to enter this and impossible to
            // leave - the way back was the "face" button, and that button is gone.
            let mut by_origin = jc.joint.anchor_mode == 3;
            if ui.toggle_value(&mut by_origin, qymcad_i18n::tr("j-origin")).on_hover_text(qymcad_i18n::tr("j-origin-hint")).changed() {
                jc.joint.anchor_mode = if by_origin { 3 } else { 0 };
            }
            // The value of a joint is set through its free degrees: an angle for those that turn, an
            // offset for those that slide. A value left unset leaves the degree free.
            if matches!(jc.joint.new_kind, JointKind::Rigid | JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot) {
                ui.separator();
                ui.label(qymcad_i18n::tr("j-angle-deg"));
                ui.add(egui::DragValue::new(&mut jc.joint.new_angle).speed(1.0).range(-360.0..=360.0));
            }
            if matches!(jc.joint.new_kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical) {
                ui.separator();
                ui.label(qymcad_i18n::tr("j-offset-mm"));
                ui.add(egui::DragValue::new(&mut jc.joint.new_offset).speed(0.2));
            }
            // "KEEP AS IT STANDS" belongs here rather than after creation: see `new_as_built`.
            ui.separator();
            ui.checkbox(&mut jc.joint.new_as_built, qymcad_i18n::tr("j-as-built")).on_hover_text(qymcad_i18n::tr("jt-as-built-hint"));
            ui.separator();
            ui.label(egui::RichText::new(hint).color(jc.scheme.pal.hint()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                    cancel = true;
                }
            });
        });
    }
    if cancel {
        jc.joint.pick_faces = false;
        jc.joint.pick_first = None;
        *jc.status = Notice::info(qymcad_i18n::tr("j-cancelled"));
    }
}

/// The assembly tool bar in one call - a door for checks that look AT A FRAME.
pub fn joint_tool_bar_for_test(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui) {
    joint_tool_bar(jc, ui);
}
