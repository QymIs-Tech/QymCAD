//! JOINTS - picking them, solving them, their glyphs and their panel.

use super::*;
use qymcad_core::feature::RelationKind;

/// THE NAME AND UNIT OF A SLOT for a given kind of joint - one definition for the whole interface.
pub(super) fn joint_slot_label(kind: qymcad_core::feature::JointKind, slot: usize) -> (String, String) {
    use qymcad_core::feature::JointKind;
    match (slot, kind) {
        (0, _) => (crate::i18n::tr("j-angle-lower"), crate::i18n::tr("unit-deg-suffix")),
        (1, JointKind::Ball) => (crate::i18n::tr("j-angle-x"), crate::i18n::tr("unit-deg-suffix")),
        (1, JointKind::Planar) => (crate::i18n::tr("j-offset-x"), crate::i18n::tr("unit-mm-suffix")),
        (1, JointKind::Rigid) => (crate::i18n::tr("j-gap"), crate::i18n::tr("unit-mm-suffix")),
        (1, _) => (crate::i18n::tr("j-offset-lower"), crate::i18n::tr("unit-mm-suffix")),
        (2, JointKind::Ball) => (crate::i18n::tr("j-angle-y"), crate::i18n::tr("unit-deg-suffix")),
        _ => (crate::i18n::tr("j-offset-y"), crate::i18n::tr("unit-mm-suffix")),
    }
}

/// THE LIMITS OF ONE SLOT: a min and a max with tick boxes. One widget serves both the popup at the glyph
/// and the right-hand panel - these used to be two copies, and they drifted apart silently, exactly as the
/// copies of the value editor did.
pub(super) fn joint_slot_limits(ui: &mut egui::Ui, jj: &mut qymcad_core::feature::Joint, slot: usize) -> bool {
    let (name, suff) = joint_slot_label(jj.kind, slot);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(name);
        let mut has_min = jj.limit_min[slot].is_some();
        if ui.checkbox(&mut has_min, &crate::i18n::tr("j-min")).changed() {
            jj.limit_min[slot] = has_min.then_some(0.0);
            changed = true;
        }
        if let Some(v) = jj.limit_min[slot].as_mut() {
            changed |= ui.add(egui::DragValue::new(v).speed(0.5).suffix(suff.clone())).changed();
        }
        let mut has_max = jj.limit_max[slot].is_some();
        if ui.checkbox(&mut has_max, &crate::i18n::tr("j-max")).changed() {
            jj.limit_max[slot] = has_max.then_some(0.0);
            changed = true;
        }
        if let Some(v) = jj.limit_max[slot].as_mut() {
            changed |= ui.add(egui::DragValue::new(v).speed(0.5).suffix(suff.clone())).changed();
        }
    });
    changed
}

/// ONE DEGREE OF FREEDOM OF A JOINT IN THE INTERFACE: a reading while it is not driven, and a driver once
/// it is.
///
/// One widget serves both places an edit can happen (the popup at the glyph and the right-hand panel) -
/// these used to be two copies, and they drifted apart silently. While the degree is free, the field shows
/// WHERE THE PART ENDED UP and can be edited freely; editing the value turns driving on (the usual
/// behaviour: drag the value and it becomes driven). The lock beside it takes the driver off again and
/// gives the part its freedom back.
pub(super) fn joint_slot_drag(ui: &mut egui::Ui, jj: &mut qymcad_core::feature::Joint, slot: usize, speed: f64) -> bool {
    let (label, suffix) = joint_slot_label(jj.kind, slot);
    let measured = match slot {
        0 => jj.angle,
        1 => jj.offset,
        _ => jj.offset2,
    };
    let driven = jj.drive[slot];
    let mut v = driven.unwrap_or(measured);
    ui.label(label);
    let mut changed = ui.add(egui::DragValue::new(&mut v).speed(speed).suffix(suffix)).changed();
    if changed {
        jj.drive[slot] = Some(v);
    }
    let (icon, tip) = match driven {
        Some(_) => (ph::LOCK, &crate::i18n::tr("j-value-set")),
        None => (ph::LOCK_OPEN, &crate::i18n::tr("j-value-free")),
    };
    if ui.small_button(icon).on_hover_text(tip).clicked() {
        jj.drive[slot] = driven.is_none().then_some(measured);
        changed = true;
    }
    changed
}

impl App {
    /// The assembly tool bar in one call - a door for checks that look AT A FRAME.
    #[cfg(test)]
    pub(crate) fn joint_tool_bar_for_test(&mut self, ui: &mut egui::Ui) {
        self.joint_tool_bar(ui);
    }


    pub(super) fn joint_tool_bar(&mut self, ui: &mut egui::Ui) {
        // The panel lives inside a `Ui` now; the context is still wanted for windows,
        // input and viewport commands, and it comes from the same place.
        let ctx = &ui.ctx().clone();
        use qymcad_core::feature::JointKind;
        // the tangency tool bar - a hint and a way out. There is nothing to confirm: the condition is set
        // on the second pick, and tangency has no connectors.
        if let Some(sel) = self.joint.tangent_pick.clone() {
            let mut cancel = false;
            egui::Panel::top("tangent_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::CIRCLE_HALF_TILT, crate::i18n::tr("j-tangent-made"))).strong());
                    ui.separator();
                    let hint = if sel.is_empty() { crate::i18n::tr("j-tangent-pick") } else { crate::i18n::tr("j-tangent-second") };
                    ui.label(egui::RichText::new(hint).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
            if cancel {
                self.joint.tangent_pick = None;
                self.status = crate::i18n::tr("j-tangent-off");
            }
            return;
        }
        // the width tool bar - how many anchors are shown, a "make it" button and a way out.
        if let Some(sel) = self.joint.width_pick.clone() {
            let (mut make, mut cancel) = (false, false);
            egui::Panel::top("width_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::ARROWS_OUT_LINE_HORIZONTAL, crate::i18n::tr("j-width-made"))).strong());
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::tr1("j-width-picked", "n", &sel.len().to_string())).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                            cancel = true;
                        }
                        if ui.button(format!("{} {}", ph::CHECK, crate::i18n::tr("j-width-make-btn"))).clicked() {
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
                self.width_pick_confirm();
            } else if cancel {
                self.joint.width_pick = None;
                self.status = crate::i18n::tr("j-width-off");
            }
            return;
        }
        // the group tool bar - how many are picked, a "group them" button and a way out. The same shape
        // as every other tool: the hint on top, Enter confirms, Esc cancels.
        if let Some(sel) = self.joint.group_pick.clone() {
            let (mut make, mut cancel) = (false, false);
            egui::Panel::top("group_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::SELECTION_ALL, crate::i18n::tr("j-group-made"))).strong());
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::tr1("j-group-picked", "n", &sel.len().to_string())).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                            cancel = true;
                        }
                        if ui.button(format!("{} {}", ph::CHECK, crate::i18n::tr("j-group-make-btn"))).clicked() {
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
                self.group_pick_confirm();
            } else if cancel {
                self.joint.group_pick = None;
                self.status = crate::i18n::tr("j-group-off");
            }
            return;
        }
        // the grounding tool bar - a hint and a way out
        if self.joint.ground_pick {
            let mut cancel = false;
            egui::Panel::top("ground_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::ANCHOR, crate::i18n::tr("jt-ground-btn"))).strong());
                    ui.separator();
                    ui.label(egui::RichText::new(&crate::i18n::tr("j-ground-click")).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-done-esc")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
            if cancel {
                self.joint.ground_pick = false;
                self.status = crate::i18n::tr("j-ground-off");
            }
            return;
        }
        // THE ANCHOR TOOL BAR: the kind of anchor and a hint about where to click.
        if self.joint.conn_pick {
            let mut cancel = false;
            egui::Panel::top("conn_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::CROSSHAIR, crate::i18n::tr("j-conn-new"))).strong());
                    ui.separator();
                    ui.label(&crate::i18n::tr("j-anchor"));
                    ui.label(egui::RichText::new(&crate::i18n::tr("j-anchor-inferred")).weak()).on_hover_text(&crate::i18n::tr("j-anchor-inferred-hint"));
                    ui.separator();
                    ui.label(egui::RichText::new(&crate::i18n::tr("j-conn-pick")).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
            if cancel {
                self.joint.conn_pick = false;
                self.status = crate::i18n::tr("j-conn-off");
            }
            return;
        }
        // THE RELATION TOOL BAR: the kind, the number, the direction, Enter/Esc.
        if self.joint.relation_pick.is_some() {
            let (mut cancel, mut done) = (false, false);
            let pick = self.joint.relation_pick.clone().unwrap_or_default();
            let need = Self::relation_picks_needed(pick.kind);
            let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
            egui::Panel::top("relation_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", ph::GEAR_SIX, crate::i18n::tr("j-relation-btn"))).strong());
                    ui.separator();
                    ui.label(&crate::i18n::tr("j-kind"));
                    let mut k = pick.kind;
                    egui::ComboBox::from_id_salt("relation_bar_kind").selected_text(crate::i18n::tr(k.label())).show_ui(ui, |ui| {
                        for kk in [RelationKind::Gear, RelationKind::RackPinion, RelationKind::Screw, RelationKind::Linear] {
                            ui.selectable_value(&mut k, kk, crate::i18n::tr(kk.label()));
                        }
                    });
                    ui.separator();
                    // THE NUMBER MEANS DIFFERENT THINGS FOR DIFFERENT KINDS, and the caption must say so:
                    // for gears and linear relations it is a ratio, for a rack and a screw it is the travel
                    // per turn in millimetres.
                    ui.label(&crate::i18n::tr(if pick.kind.value_is_per_turn() { "j-relation-per-turn" } else { "j-relation-ratio" }));
                    let mut v = pick.value;
                    ui.add(egui::DragValue::new(&mut v).speed(0.05));
                    let mut rev = pick.reversed;
                    ui.checkbox(&mut rev, &crate::i18n::tr("j-relation-reverse"));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string())).color(self.scheme.pal.hint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                            cancel = true;
                        }
                        if ui.add_enabled(have >= need, egui::Button::new(&crate::i18n::tr("j-done-enter"))).clicked() {
                            done = true;
                        }
                    });
                    if let Some(p) = self.joint.relation_pick.as_mut() {
                        // CHANGING THE KIND CLEARS THE PICKS: the degrees that were picked were of the
                        // right sort for the PREVIOUS kind, and keeping them would build the relation on
                        // the wrong ones.
                        if k != p.kind {
                            p.kind = k;
                            p.picks.clear();
                        }
                        p.value = v;
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
                self.joint.relation_pick = None;
                self.status = crate::i18n::tr("j-relation-off");
            } else if done {
                self.relation_pick_confirm();
            }
            return;
        }
        if !self.joint.pick_faces {
            return;
        }
        // WHAT IS BEING ASKED FOR. This used to name the chosen mode - face, edge, vertex: the sort of
        // anchor was declared first and the hint merely repeated that choice back. Now the sort is inferred
        // under the cursor, and what must be asked for is exactly A PLACE ON THE PART.
        let target = if self.joint.anchor_mode == 3 { crate::i18n::tr("j-origin-lower") } else { crate::i18n::tr("j-place-lower") };
        let target = &target;
        let hint = if self.joint.pick_first.is_some() { crate::i18n::tr1("jt-click-b", "what", &target) } else { crate::i18n::tr1("jt-click-a", "what", &target) };
        let mut cancel = false;
        egui::Panel::top("joint_tool_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::MAGNET, crate::i18n::tr("jt-joint-btn"))).strong());
                ui.separator();
                ui.label(&crate::i18n::tr("j-kind"));
                let mut k = self.joint.new_kind;
                egui::ComboBox::from_id_salt("joint_bar_kind").selected_text(crate::i18n::tr(k.label())).show_ui(ui, |ui| {
                    // THERE IS ONE SET OF KINDS: a joint states the degrees of freedom, and that is all.
                    // The former "assembly mates" duplicated the mechanical ones (a mate equals planar,
                    // concentricity equals cylindrical) but were solved along another path and behaved
                    // differently.
                    // ALL OF THE KINDS ARE HERE. The rigid one used to be missing: it was created only by
                    // its own button in the workbench panel, and removing the buttons would have made that
                    // kind unreachable altogether.
                    for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                        ui.selectable_value(&mut k, kk, crate::i18n::tr(kk.label()));
                    }
                });
                if k != self.joint.new_kind {
                    self.joint.new_kind = k;
                    // THE SORT OF ANCHOR IS NO LONGER CHOSEN IN ADVANCE - neither by a person nor on their
                    // behalf by the kind of joint. This used to read "coaxial kinds prefer an edge, the rest
                    // a face", and changing the kind silently moved the mode: the next click landed
                    // somewhere other than where it was aimed.
                    self.joint.anchor_mode = 0;
                }
                ui.separator();
                ui.label(&crate::i18n::tr("j-anchor"));
                // THE SORT OF ANCHOR IS INFERRED UNDER THE CURSOR.
                //
                // There used to be three switches here - face, edge, vertex. A professional CAD has none:
                // the anchor point is derived from the geometry under the cursor, and that is not a
                // convenience but a condition of working at all - while the sort was declared in advance, a
                // click regularly produced an anchor other than the one intended.
                ui.label(egui::RichText::new(&crate::i18n::tr("j-anchor-inferred")).weak()).on_hover_text(&crate::i18n::tr("j-anchor-inferred-hint"));
                // BY ORIGINS IS A FOURTH SORT OF ANCHOR, NOT A SEPARATE BUTTON IN THE PROPERTIES PANEL.
                //
                // This way of working used to live in the right-hand panel: two drop-down lists for A and B
                // and a create button. It went around everything that makes a command a command - picking
                // by click, a preview, Enter/Esc - and taught the wrong habits. It is a useful way (parts
                // with no convenient faces, a quick rough assembly), so it was not thrown out but moved
                // HERE: the same picking by clicking a part, the same bar, the same cancel.
                // A TOGGLE, NOT A ONE-WAY CHOICE: it used to be possible to enter this and impossible to
                // leave - the way back was the "face" button, and that button is gone.
                let mut by_origin = self.joint.anchor_mode == 3;
                if ui.toggle_value(&mut by_origin, &crate::i18n::tr("j-origin")).on_hover_text(&crate::i18n::tr("j-origin-hint")).changed() {
                    self.joint.anchor_mode = if by_origin { 3 } else { 0 };
                }
                // The value of a joint is set through its free degrees: an angle for those that turn, an
                // offset for those that slide. A value left unset leaves the degree free.
                if matches!(self.joint.new_kind, JointKind::Rigid | JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot) {
                    ui.separator();
                    ui.label(&crate::i18n::tr("j-angle-deg"));
                    ui.add(egui::DragValue::new(&mut self.joint.new_angle).speed(1.0).range(-360.0..=360.0));
                }
                if matches!(self.joint.new_kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical) {
                    ui.separator();
                    ui.label(&crate::i18n::tr("j-offset-mm"));
                    ui.add(egui::DragValue::new(&mut self.joint.new_offset).speed(0.2));
                }
                // "KEEP AS IT STANDS" belongs here rather than after creation: see `new_as_built`.
                ui.separator();
                ui.checkbox(&mut self.joint.new_as_built, &crate::i18n::tr("j-as-built")).on_hover_text(&crate::i18n::tr("jt-as-built-hint"));
                ui.separator();
                ui.label(egui::RichText::new(hint).color(self.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(&crate::i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if cancel {
            self.joint.pick_faces = false;
            self.joint.pick_first = None;
            self.status = crate::i18n::tr("j-cancelled");
        }
    }


    /// The top bar of JOINT EDIT MODE (a double click on the glyph), in the style of the tool bars: the
    /// joint's name, the kind of anchor, and a "swap anchor" button that drops down A and B for picking a
    /// new face, edge or vertex on the fly. The parameters (angle, offset, limits, flip, global) live in
    /// the popup at the glyph (`joint_popup`). One tool-command experience, with no trips to the right-hand
    /// panel.
    pub(super) fn joint_edit_bar(&mut self, ui: &mut egui::Ui) {
        let Some(jid) = self.joint.edit else { return };
        if !matches!(self.workbench, Workbench::Assembly) || !self.mode_3d {
            return;
        }
        let Some(j) = self.project.joints.iter().find(|x| x.id == jid).cloned() else {
            self.joint.edit = None;
            return;
        };
        let desc_a = self.project.connector(j.a).map(|c| self.anchor_desc(&c.anchor)).unwrap_or_default();
        let desc_b = self.project.connector(j.b).map(|c| self.anchor_desc(&c.anchor)).unwrap_or_default();
        let repick = self.joint.edit_repick;
        let mut done = false;
        let mut set_repick: Option<Option<(Id, bool)>> = None;
        let mut set_kind: Option<qymcad_core::feature::JointKind> = None;
        let (mut flip_axis, mut swap_roles) = (false, false);
        egui::Panel::top("joint_edit_bar").frame(self.tool_bar_frame())
.show(ui, |ui| {
            use qymcad_core::feature::JointKind;
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::LINK, crate::i18n::tr1("jt-editing", "name", &crate::i18n::name(&j.name)))).strong());
                // A JOINT WITHOUT AN ANCHOR SAYS SO OUT LOUD. The solver silently drops such joints from
                // the problem: the assembly looks assembled, the parts do not move, and there is no
                // explanation. That was reported as parts not moving with the direction being wrong no
                // matter what was picked.
                // The reason comes from A SINGLE source (`joint_faults`), which also speaks through the
                // solver report and the list of joints: two places with the same words and different
                // truths is a thing already been through.
                if let Some((_, why)) = self.project.joint_faults().iter().find(|(id, _)| *id == jid) {
                    ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, crate::i18n::tr(why))).color(self.scheme.pal.error_mild()))
                        .on_hover_text(crate::i18n::tr(&format!("{why}-hint")));
                }
                ui.separator();
                // changing the KIND of a joint right in the edit bar (the anchors are kept).
                ui.label(&crate::i18n::tr("j-kind"));
                let mut nk = j.kind;
                egui::ComboBox::from_id_salt("joint_edit_kind").selected_text(crate::i18n::tr(j.kind.label())).show_ui(ui, |ui| {
                    for kk in [JointKind::Rigid, JointKind::Revolute, JointKind::Slider, JointKind::Cylindrical, JointKind::Planar, JointKind::Ball, JointKind::PinSlot, JointKind::Parallel] {
                        ui.selectable_value(&mut nk, kk, crate::i18n::tr(kk.label()));
                    }
                });
                if nk != j.kind {
                    set_kind = Some(nk);
                }
                ui.separator();
                ui.label(&crate::i18n::tr("j-anchor"));
                ui.label(egui::RichText::new(&crate::i18n::tr("j-anchor-inferred")).weak()).on_hover_text(&crate::i18n::tr("j-anchor-inferred-hint"));
                ui.separator();
                ui.menu_button(format!("{} {}", ph::MAGNET, crate::i18n::tr("jt-swap-anchor")), |ui| {
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
                if ui.button(format!("{} {}", ph::ARROWS_DOWN_UP, crate::i18n::tr("jt-flip-axis"))).on_hover_text(&crate::i18n::tr("jt-flip-axis-hint")).clicked() {
                    flip_axis = true;
                }
                if ui.button(format!("{} {}", ph::SWAP, crate::i18n::tr("jt-swap-roles"))).on_hover_text(&crate::i18n::tr("jt-swap-roles-hint")).clicked() {
                    swap_roles = true;
                }
                if let Some((_, is_b)) = repick {
                    let t = crate::i18n::tr("j-place-lower");
                    ui.label(egui::RichText::new(crate::i18n::tr2("jt-click-new", "what", &t, "side", if is_b { "B" } else { "A" })).color(self.scheme.pal.hint()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(&crate::i18n::tr("j-done-esc")).clicked() {
                        done = true;
                    }
                });
            });
        });
        if let Some(v) = set_repick {
            self.joint.edit_repick = v;
        }
        if let Some(k) = set_kind {
            if self.change_joint_kind(jid, k) {
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
            }
        }
        if flip_axis {
            self.joint_hud_flip_axis(jid);
        }
        if swap_roles {
            self.joint_hud_swap_roles(jid);
        }
        if done {
            self.exit_joint_edit();
        }
    }


    /// THE CONSOLE HANDLE "FLIP THE AXIS".
    ///
    /// The main axis is flipped at the FIRST anchor: it sets the direction the second one is brought to,
    /// and flipping the second would be the same thing inside out. What that changes in the document is
    /// THE CORE's business (`flip_joint_side`): the interface only presses the handle.
    pub(super) fn joint_hud_flip_axis(&mut self, jid: Id) {
        self.begin_edit(&crate::i18n::tr("jt-flip-axis")); // THE BOUNDARY OF AN OPERATION
        self.project.flip_joint_side(jid);
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
    }


    /// THE CONSOLE HANDLE "SWAP THE ROLES": which part stands still and which one moves.
    pub(super) fn joint_hud_swap_roles(&mut self, jid: Id) {
        self.begin_edit(&crate::i18n::tr("jt-swap-roles")); // THE BOUNDARY OF AN OPERATION
        if self.project.swap_joint_roles(jid) {
            self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        }
    }


    /// THE TEST FACADE FOR PICKING AN ANCHOR: the same door a click on the frame goes through.
    #[cfg(test)]
    pub(crate) fn joint_pick_anchor_click_for_test(&mut self, owner: Id, anchor: qymcad_core::feature::AnchorRef) {
        self.joint.pick_faces = true;
        self.joint_pick_anchor_click(owner, anchor);
    }


    /// THE TEST FACADES OF THE CONSOLE: the same door the buttons of the bar go through.
    #[cfg(test)]
    pub(crate) fn joint_hud_swap_roles_for_test(&mut self, jid: Id) {
        self.joint_hud_swap_roles(jid);
    }


    #[cfg(test)]
    pub(crate) fn joint_hud_flip_axis_for_test(&mut self, jid: Id) {
        self.joint_hud_flip_axis(jid);
    }


    /// Which joint the console currently holds.
    #[cfg(test)]
    pub(crate) fn joint_edit_for_test(&self) -> Option<Id> {
        self.joint.edit
    }


    /// Clicking an anchor (a face or an edge) for a joint: the first pick becomes A, the second one (on
    /// ANOTHER part) becomes B, and a joint of the chosen kind is created. Face to face flips B so the
    /// normals meet; the axis of an edge takes no flip. A face anchor is persistent (a `FaceKey`), so it
    /// travels with the face through a rebuild.
    ///
    /// THE EDGES OF A BODY GO INTO THE MODEL once the live B-rep is up.
    ///
    /// An anchor on an edge or on a vertex is resolved by the core through `Project::regen_edges`, and that
    /// map is filled by THE POST-PASS of a rebuild. Opening a file does not rebuild: the bundle holds
    /// meshes and faces but no edges. A click still HITS an edge - the pick takes them from the live B-rep
    /// - and the joint was born dead: "anchor lost", travel 0.000 mm, no axis of travel. Measured on a real
    /// document: 138 bodies, faces on all 138, EDGES ON TWO, live B-rep on all 138.
    ///
    /// The two sources of edges are reconciled here, at the point where the anchor is created: the core is
    /// asked through the same call a rebuild uses to fill them.
    pub(super) fn ensure_model_edges(&mut self, body: Id) {
        if self.project.regen_edges.contains_key(&body) || !self.live.shapes.contains_key(&body) {
            return;
        }
        let edges = self.with_kernel(|_, k| k.edges(body));
        if !edges.is_empty() {
            self.project.regen_edges.insert(body, edges);
        }
    }

    pub(super) fn joint_pick_anchor_click(&mut self, owner: Id, anchor: qymcad_core::feature::AnchorRef) {
        // the edges of a body go into the model BEFORE an anchor refers to them (see `ensure_model_edges`)
        if let qymcad_core::feature::AnchorRef::EdgeMid(b, _) | qymcad_core::feature::AnchorRef::Vertex(b, _, _) = &anchor {
            let b = *b;
            self.ensure_model_edges(b);
        }
        // A STANDALONE ANCHOR: the same geometry, the same reading of the click - but no joint is created.
        // The connector is made on its own and waits for a joint to be put on it later.
        if self.joint.conn_pick {
            if !self.geometry_ready_for_anchor(&anchor) {
                return; // the readiness check has already said what is wrong
            }
            self.begin_edit(&crate::i18n::tr("j-conn-made")); // THE BOUNDARY OF AN OPERATION
            let cid = self.project.add_connector_standalone(owner, anchor);
            self.joint.conn_pick = false;
            self.sel_conn = Some(cid);
            self.status = crate::i18n::tr("j-conn-made-ok");
            self.commit_edit();
            return;
        }
        // a joint only ever goes between parts of the ACTIVE context. Ghost parts of other subassemblies
        // are visible so they can be referred to, but they are dimmed for a reason - no joint lands on them.
        let ctx = self.current_ctx_id();
        if !self.project.component_is_within(owner, ctx) {
            self.status = crate::i18n::tr("j-outside-assembly-joint");
            return;
        }
        // AN ANCHOR ON A MOVING PART IS REPORTED AT ONCE, NOT LATER.
        //
        // Such an anchor makes an assembly unstable for good: the joint holds on to a part that itself
        // travels inside the same assembly, and every recount takes it further. On a real document that
        // cost 60 mm per pass, endlessly, and looked like an assembly drifting apart by itself.
        //
        // This used to be found out AFTERWARDS, from a mark in the timeline. But an anchor is placed
        // deliberately, and the person must be stopped in the second they point at the wrong part. There is
        // one truth: the core is asked through the same call that computes the fault.
        //
        // IT IS ASKED BEFORE WAITING FOR GEOMETRY: "you pointed at the wrong part" is an objection on the
        // merits, and it holds whether or not the live B-rep has come up.
        if self.project.anchor_sits_on_moving_part(owner, &anchor) {
            self.status = crate::i18n::tr("j-anchor-on-moving-part-refused");
            return;
        }
        // WHILE THE GEOMETRY IS ON ITS WAY, NO ANCHOR IS TAKEN.
        //
        // In a live window the B-rep preparation goes to a BACKGROUND thread, while face picking works off
        // the mesh and is available at once. On a big assembly the preparation takes seconds: clicking
        // before the geometry arrives is a matter of one second. A joint made without it takes its axis
        // from the WORLD axes (a face has no principal direction, an edge has no neighbouring face), the
        // part travels to the wrong place and STAYS there: minimal displacement afterwards will not move it
        // for nothing, however long it is computed.
        //
        // The rule is the same as for any other preparation: say so and wait.
        if !self.geometry_ready_for_anchor(&anchor) {
            // TWO DIFFERENT CASES, TWO DIFFERENT ANSWERS. While the preparation runs, "one moment" is
            // the truth. But if the preparation IS ALREADY OVER and there is still no live body (an import
            // that did not restore, an operation that did not build), then "one moment" is a lie: there is
            // nothing to wait for, and the face will be clicked until it is given up on.
            self.status = crate::i18n::tr(if self.live.ready { "j-geometry-missing" } else { "j-geometry-on-its-way" });
            return;
        }
        match self.joint.pick_first.take() {
            None => {
                self.joint.pick_first = Some((owner, anchor));
                self.status = crate::i18n::tr("j-anchor-a-picked");
            }
            Some((owner_a, anchor_a)) => {
                if owner_a == owner {
                    self.joint.pick_first = Some((owner_a, anchor_a)); // the same part - wait for another one
                    self.status = crate::i18n::tr("j-pick-other-part");
                    return;
                }
                // a joint runs between components AT THE LEVEL OF THE CONTEXT (the direct children of ctx,
                // that is, the subassemblies) rather than between leaf bodies. The anchor stays on the leaf
                // body; the owner of the connector is the subassembly, and `place_tree` moves that whole
                // subassembly as one.
                let place_a = self.project.ancestor_child_of(ctx, owner_a).unwrap_or(owner_a);
                let place_b = self.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
                if place_a == place_b {
                    self.joint.pick_first = Some((owner_a, anchor_a)); // both anchors in one subassembly - an internal joint
                    self.status = crate::i18n::tr("j-same-subassembly");
                    return;
                }
                // The "anchor compatibility" check is gone from here: it always answered "compatible", so
                // the refusal existed only in the text. An anchor is a full coordinate frame, and any kind
                // of joint works with any pair; which degrees of freedom are left is stated by the kind.
                // the side of the contact is decided by the solver (coplanar, minimal motion) plus the
                // `joint.flip` toggle in the panel - forcing a flip on the connector is no longer needed
                // (it caused a spurious 180 deg turn).
                let ca = self.project.add_connector(place_a, anchor_a);
                let cb = self.project.add_connector(place_b, anchor);
                let jid = self.project.add_joint(ca, cb, self.joint.new_kind);
                // The values from the tool bar are A DRIVER, not a reading. Untouched fields (zero) do not
                // count as a driver: a joint is born with free degrees, and they can be pinned with the
                // lock in the popup at the glyph.
                if let Some(j) = self.project.joints.iter_mut().find(|j| j.id == jid) {
                    j.drive[1] = (self.joint.new_offset != 0.0).then_some(self.joint.new_offset);
                    j.drive[0] = (self.joint.new_angle != 0.0).then_some(self.joint.new_angle);
                }
                // "AS IT STANDS" IS TAKEN BEFORE THE FIRST SOLVE, while the parts are still where they
                // were placed. After a solve there is nothing left to take: the joint mates the anchors and
                // drags the part away.
                if self.joint.new_as_built && !self.project.set_joint_as_built(jid) {
                    self.status = crate::i18n::tr("j-as-built-failed");
                }
                let children = self.project.component_children(ctx);
                if !children.iter().any(|&c| self.project.is_grounded(c)) {
                    self.project.set_grounded(place_a, true); // the first anchor grounds the context-level subassembly
                }
                self.joint.pick_faces = false;
                // THE CONSOLE OPENS BY ITSELF: after the second pick the handles for flipping the axis,
                // swapping the roles, changing the kind and changing the anchor are right there - the very
                // ones needed at once, because the side and the order come out right the first time far
                // from always. The joint used to appear silently in the list, and mending it required first
                // guessing to look for it there.
                self.joint.edit = Some(jid);
                self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
                self.status = crate::i18n::tr("j-created");
            }
        }
    }


    /// The grounding tool: a click on a body fixes or releases its part (at the level of the context).
    /// As when creating a joint, what gets grounded is the DIRECT child of the context (the subassembly)
    /// rather than a leaf body.
    pub(super) fn joint_pick_ground_click(&mut self, body: Id) {
        let ctx = self.current_ctx_id();
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-body-no-part");
            return;
        };
        let comp = self.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
        let g = self.project.is_grounded(comp);
        self.project.set_grounded(comp, !g);
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        let name = self.project.components.iter().find(|c| c.id == comp).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
        self.status = if !g { crate::i18n::tr1("jt-grounded", "name", &name) } else { crate::i18n::tr1("jt-released", "name", &name) };
    }


    /// TOGGLE THE GROUNDING TOOL. Through one door, like joint picking: it now has two ways in - the panel
    /// button and the command search.
    pub(crate) fn start_ground_pick(&mut self) {
        let on = !self.joint.ground_pick;
        self.cancel_all_tools(); // mutually exclusive with the create tools and the other joint tools
        self.joint.ground_pick = on;
        self.joint.pick_first = None;
        self.status = if on { crate::i18n::tr("tb-ground-pick") } else { crate::i18n::tr("tb-ground-off") };
    }

    /// TOGGLE THE GROUP TOOL.
    ///
    /// A group fastens a set of parts TO ONE ANOTHER where they stand. It picks no anchors, so it is not
    /// gathered the way a joint is: click as many parts as needed and confirm.
    pub(crate) fn start_group_pick(&mut self) {
        let on = self.joint.group_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.group_pick = on.then(Vec::new);
        self.status = crate::i18n::tr(if on { "j-group-pick" } else { "j-group-off" });
    }

    /// A click on a body while the group tool is active: add its part to the set or take it out.
    pub(super) fn group_pick_click(&mut self, body: Id) {
        if self.joint.group_pick.is_none() {
            return;
        }
        let ctx = self.current_ctx_id();
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-body-no-part");
            return;
        };
        // A joint runs between components AT THE LEVEL OF THE CONTEXT, and so does a group: what gets
        // fastened is what reads as a part of the assembly, not a leaf body inside a subassembly.
        let comp = self.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
        let Some(sel) = self.joint.group_pick.as_mut() else { return };
        match sel.iter().position(|&c| c == comp) {
            Some(i) => {
                sel.remove(i);
            }
            None => sel.push(comp),
        }
        let n = sel.len();
        self.status = crate::i18n::tr1("j-group-picked", "n", &n.to_string());
    }

    /// Confirm the set and make a group of it. Fewer than two parts leaves nothing to fasten.
    pub(super) fn group_pick_confirm(&mut self) {
        let Some(sel) = self.joint.group_pick.clone() else { return };
        if sel.len() < 2 {
            // NO SILENT REFUSAL: a part has already been clicked and a result is expected.
            self.status = crate::i18n::tr("j-group-need-two");
            return;
        }
        self.begin_edit(&crate::i18n::tr("j-group-made")); // THE BOUNDARY OF AN OPERATION
        self.project.add_group(&sel);
        self.joint.group_pick = None;
        self.status = crate::i18n::tr1("j-group-made-n", "n", &sel.len().to_string());
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.commit_edit();
    }

    /// TOGGLE THE TANGENCY TOOL.
    ///
    /// Tangency needs no connectors - two surfaces are enough, so there is nothing to confirm: the second
    /// pick sets the condition straight away.
    pub(crate) fn start_tangent_pick(&mut self) {
        let on = self.joint.tangent_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.tangent_pick = on.then(Vec::new);
        self.status = crate::i18n::tr(if on { "j-tangent-pick" } else { "j-tangent-off" });
    }

    /// A click on a face while the tangency tool is active.
    ///
    /// THE PAIR MUST BE A CYLINDER AND A PLANE. Tangency holds a distance equal to the radius; a pair of
    /// planes has no radius, and "tangency" between them means merely "coincident" - and there is a planar
    /// joint for that. Accepting such a pair silently would set a condition that does nothing.
    pub(super) fn tangent_pick_click(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        use qymcad_core::feature::AnchorRef;
        if self.joint.tangent_pick.is_none() {
            return;
        }
        let ctx = self.current_ctx_id();
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-face-no-part");
            return;
        };
        let comp = self.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
        let is_cyl = self.project.face_cylinder(body, &key).is_some();
        let Some(sel) = self.joint.tangent_pick.as_mut() else { return };
        if let Some((_, AnchorRef::FaceCenter(b0, k0))) = sel.first().cloned() {
            // the second surface must complement the first: a cylinder to a plane and the other way round
            if self.project.face_cylinder(b0, &k0).is_some() == is_cyl {
                self.status = crate::i18n::tr("j-tangent-need-cylinder");
                return;
            }
        }
        let Some(sel) = self.joint.tangent_pick.as_mut() else { return };
        sel.push((comp, AnchorRef::FaceCenter(body, key)));
        if sel.len() < 2 {
            self.status = crate::i18n::tr("j-tangent-second");
            return;
        }
        let pair = sel.clone();
        self.begin_edit(&crate::i18n::tr("j-tangent-made")); // THE BOUNDARY OF AN OPERATION
        self.project.add_tangent(pair[0].0, pair[0].1.clone(), pair[1].0, pair[1].1.clone());
        self.joint.tangent_pick = None;
        self.status = crate::i18n::tr("j-tangent-made-ok");
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.commit_edit();
    }

    /// TOGGLE THE WIDTH TOOL.
    ///
    /// Width puts a part MIDWAY between two walls. Three faces have to be shown: the two walls and the
    /// piece between them; the order matters.
    pub(crate) fn start_width_pick(&mut self) {
        let on = self.joint.width_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.width_pick = on.then(Vec::new);
        self.status = crate::i18n::tr(if on { "j-width-pick" } else { "j-width-off" });
    }

    /// A click on a face while the width tool is active: add an anchor to the set.
    ///
    /// THE WALLS MUST FACE THE SAME WAY. "Midway" means something only along a COMMON normal; walls that
    /// look in different directions have no midpoint, and taking such an anchor silently would promise
    /// something that does not exist.
    pub(super) fn width_pick_click(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        use qymcad_core::feature::AnchorRef;
        if self.joint.width_pick.is_none() {
            return;
        }
        let ctx = self.current_ctx_id();
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-face-no-part");
            return;
        };
        let comp = self.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
        let n = key.normal;
        let Some(sel) = self.joint.width_pick.as_mut() else { return };
        if sel.len() == 1 {
            if let Some((_, AnchorRef::FaceCenter(_, k0))) = sel.first() {
                let dot = k0.normal[0] * n[0] + k0.normal[1] * n[1] + k0.normal[2] * n[2];
                if dot.abs() < 0.999 {
                    self.status = crate::i18n::tr("j-width-walls-differ");
                    return;
                }
            }
        }
        if sel.len() >= 3 {
            sel.clear(); // one too many - start the set over rather than piling up silently
        }
        sel.push((comp, AnchorRef::FaceCenter(body, key)));
        let n = sel.len();
        self.status = crate::i18n::tr1("j-width-picked", "n", &n.to_string());
    }

    /// Confirm the width set: two walls and the piece between them.
    pub(super) fn width_pick_confirm(&mut self) {
        let Some(sel) = self.joint.width_pick.clone() else { return };
        if sel.len() < 3 {
            // NO SILENT REFUSAL: some of the anchors have already been shown and a result is expected.
            self.status = crate::i18n::tr("j-width-need-three");
            return;
        }
        self.begin_edit(&crate::i18n::tr("j-width-made")); // THE BOUNDARY OF AN OPERATION
        let ids: Vec<Id> = sel.iter().map(|(owner, a)| self.project.add_connector(*owner, a.clone())).collect();
        self.project.add_width(&[ids[0], ids[1]], ids[2]);
        self.joint.width_pick = None;
        self.status = crate::i18n::tr("j-width-made-ok");
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.commit_edit();
    }

    /// TOGGLE THE ANCHOR TOOL - creating a standalone connector.
    pub(crate) fn start_conn_pick(&mut self) {
        let on = !self.joint.conn_pick;
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.conn_pick = on;
        self.status = crate::i18n::tr(if on { "j-conn-pick" } else { "j-conn-off" });
    }

    /// DELETE AN ANCHOR - or say why that is not possible.
    pub(crate) fn delete_connector_asked(&mut self, cid: Id) {
        let users = self.project.connector_users(cid).len();
        if users > 0 {
            // NO SILENT REFUSAL: the cross was pressed, and what held the anchor back must be said.
            self.status = crate::i18n::tr1("j-conn-in-use", "n", &users.to_string());
            return;
        }
        self.begin_edit(&crate::i18n::tr("j-conn-deleted"));
        self.project.delete_connector(cid);
        if self.sel_conn == Some(cid) {
            self.sel_conn = None;
        }
        self.status = crate::i18n::tr("j-conn-deleted");
        self.commit_edit();
    }

    /// TOGGLE THE RELATION TOOL.
    ///
    /// A relation ties together the degrees of freedom of joints that already exist, so what is picked in
    /// it is THE JOINTS THEMSELVES, by clicking a row in the list of joints. There is no geometry to choose
    /// here: neither a face nor an edge concerns a relation.
    pub(crate) fn start_relation_pick(&mut self) {
        let on = self.joint.relation_pick.is_none();
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.relation_pick = on.then(crate::gui::RelationPick::default);
        self.status = crate::i18n::tr(if on { "j-relation-pick" } else { "j-relation-off" });
    }

    /// HOW MANY JOINTS A KIND EXPECTS: two for all of them except the screw, which makes do with one.
    fn relation_picks_needed(kind: qymcad_core::feature::RelationKind) -> usize {
        if kind.needs_two_mates() {
            2
        } else {
            1
        }
    }

    /// THE DEGREE OF THE RIGHT SORT IN A JOINT: the number of the slot that turns (or travels).
    ///
    /// It is looked for among the FREE degrees of the kind: a pinned degree does not exist for the joint at
    /// all, and there is nothing to tie to it.
    pub(super) fn relation_slot_of(&self, joint: Id, want_rotation: bool) -> Option<usize> {
        let j = self.project.joints.iter().find(|j| j.id == joint)?;
        let kind = qymcad_core::asm::bridge::kind_of(j.kind);
        let free = j.kind.free_slots();
        (0..3).find(|&slot| free[slot] && qymcad_core::asm::joint::slot_axis(kind, slot).is_some_and(|(_, rot)| rot == want_rotation))
    }

    /// A click on A JOINT while the relation tool is active: take its degree of the required sort.
    pub(super) fn relation_pick_click(&mut self, joint: Id) {
        let Some(pick) = self.joint.relation_pick.clone() else { return };
        let (rot_a, rot_b) = pick.kind.slots_are_rotations();
        let need = Self::relation_picks_needed(pick.kind);
        // THE SCREW TAKES BOTH DEGREES OF ONE JOINT - the angle and the travel of a cylindrical one.
        let wanted: Vec<bool> = if need == 1 { vec![rot_a, rot_b] } else { vec![if pick.picks.is_empty() { rot_a } else { rot_b }] };
        let mut taken: Vec<(Id, usize)> = Vec::new();
        for &want in &wanted {
            let Some(slot) = self.relation_slot_of(joint, want) else {
                // NO SILENT REFUSAL: the click has already happened and an answer is expected.
                self.status = crate::i18n::tr(if want { "j-relation-need-turn" } else { "j-relation-need-travel" });
                return;
            };
            taken.push((joint, slot));
        }
        if need == 2 && pick.picks.iter().any(|(id, _)| *id == joint) {
            self.status = crate::i18n::tr("j-relation-need-two");
            return;
        }
        let Some(pick) = self.joint.relation_pick.as_mut() else { return };
        pick.picks.extend(taken);
        let got = if need == 1 { 1 } else { pick.picks.len() };
        self.status = crate::i18n::tr2("j-relation-picked", "n", &got.to_string(), "need", &need.to_string());
    }

    /// Confirm the picks and create the relation.
    pub(super) fn relation_pick_confirm(&mut self) {
        let Some(pick) = self.joint.relation_pick.clone() else { return };
        let need = Self::relation_picks_needed(pick.kind);
        let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
        if have < need || pick.picks.len() < 2 {
            self.status = crate::i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string());
            return;
        }
        let ((ja, sa), (jb, sb)) = (pick.picks[0], pick.picks[1]);
        self.begin_edit(&crate::i18n::tr("j-relation-made")); // THE BOUNDARY OF AN OPERATION
        let id = self.project.add_relation(pick.kind, ja, sa, jb, sb, pick.value);
        if pick.reversed {
            if let Some(r) = self.project.relations.iter_mut().find(|r| r.id == id) {
                r.reversed = true;
                // the phase was taken BEFORE the reversal - take it again, or the relation jerks the part
                let copy = r.clone();
                let ph = self.project.relation_phase(&copy).unwrap_or(0.0);
                if let Some(r) = self.project.relations.iter_mut().find(|r| r.id == id) {
                    r.phase = ph;
                }
            }
        }
        self.joint.relation_pick = None;
        self.status = crate::i18n::tr("j-relation-made-ok");
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.commit_edit();
    }

    /// LAUNCH A COMMAND BY ITS CATALOGUE CODE.
    ///
    /// The only entry point for the search - and it leads to EXACTLY the same calls the panel button makes.
    /// A second launch path would be worse than having no search at all: it would start doing what the
    /// button does not, and the divergence would surface for whoever uses the program rather than in a test.
    pub(crate) fn run_command(&mut self, code: &str) {
        use crate::command_catalog::Launch;
        let Some(cmd) = crate::command_catalog::by_code(code) else { return };
        match cmd.launch {
            Launch::Feat(n) => self.start_feat_cmd(n),
            Launch::Prim(n) => self.start_prim_cmd(n),
            Launch::SkTool(n) => self.set_sk_tool(n),
            Launch::Dim(n) => self.set_dim_tool(n),
            Launch::ClickOp(n) => self.set_click_op(n),
            Launch::Modify(n) => self.modify_button(n),
            Launch::Action("joint") => self.start_joint_pick(),
            Launch::Action("ground") => self.start_ground_pick(),
            Launch::Action(_) => {}
        }
    }

    /// THE KEY HINT, ALLOWING FOR FOCUS: "U" while the keyboard is free, and "Alt+U" while the caret sits
    /// in an input field.
    ///
    /// The rule "hold Alt when focused" would be a secret without this hint, and secret mechanisms go
    /// unused: `U` gets pressed once, nothing happens, and it is never tried again.
    pub(crate) fn hotkey_hint(&self, ctx: &egui::Context, action: &str) -> String {
        let k = self.hotkey_key(action);
        if k.is_empty() {
            return String::new();
        }
        if ctx.egui_wants_keyboard_input() {
            format!("Alt+{k}")
        } else {
            k
        }
    }

    /// START PICKING A JOINT. One door for every way in: the workbench button, the button in the
    /// properties, the `J` key. The body of this launch used to be written straight into the panel button,
    /// and a second way in would have had to copy it - and a copy falls behind sooner or later.
    pub(super) fn start_joint_pick(&mut self) {
        self.cancel_all_tools(); // mutually exclusive with the other tools
        self.joint.pick_faces = true;
        self.joint.ground_pick = false;
        self.joint.pick_first = None;
        // THE SORT OF ANCHOR IS INFERRED UNDER THE CURSOR. There used to be an "anchor chosen by kind"
        // here: rotation got an edge, everything else a face. The guess was half right (rotation also
        // happens about a cylindrical face) and cost dearly - aiming at a face produced an edge.
        self.joint.anchor_mode = 0;
        let what = crate::i18n::tr("j-place-lower");
        self.status = crate::i18n::tr2("jt-pick-a-then-b", "kind", &crate::i18n::tr(self.joint.new_kind.label()), "what", &what);
    }



    /// THE TEST FACADES. A test must walk the same path a person does - through the command and its picks
    /// - rather than poking at fields directly, or it only ever checks an invention of its own.
    #[cfg(test)]
    /// Take up the joint tool through the same door the button or the `J` key uses.
    #[cfg(test)]
    pub(crate) fn arm_joint_pick_for_test(&mut self) {
        if !self.joint.pick_faces {
            let mode = self.joint.anchor_mode;
            self.start_joint_pick();
            self.joint.anchor_mode = mode; // the check picks the anchor mode itself
        }
    }

    /// Draw the highlight of the picks - the same thing a frame does.
    #[cfg(test)]
    /// THE WHOLE HIGHLIGHT PASS UNDER THE CURSOR in one call - for the guard that says a tool must not
    /// leave a person aiming blind. What is gathered here is exactly what the frame draws while picking.
    #[cfg(test)]
    pub(crate) fn draw_pick_highlights_for_test(&self, painter: &egui::Painter, rect: egui::Rect) {
        self.draw_joint_pick_highlight(painter, rect);
    }


    #[cfg(test)]
    pub(crate) fn draw_joint_pick_highlight_for_test(&self, painter: &egui::Painter, rect: egui::Rect) {
        self.draw_joint_pick_highlight(painter, rect);
    }

    #[cfg(test)]
    pub(crate) fn set_joint_anchor_mode_for_test(&mut self, mode: u8) {
        self.joint.anchor_mode = mode;
    }

    #[cfg(test)]
    pub(crate) fn joint_pick_origin_click_for_test(&mut self, body: Id) {
        self.joint_pick_origin_click(body);
    }

    #[cfg(test)]
    pub(crate) fn joint_pick_face_click_for_test(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        self.joint_pick_face_click(body, key);
    }

    #[cfg(test)]
    pub(crate) fn joint_pick_edge_click_for_test(&mut self, body: Id, edge: u32) {
        self.joint_pick_edge_click(body, edge);
    }

    #[cfg(test)]
    pub(crate) fn joint_pick_first_anchor_for_test(&self) -> Option<qymcad_core::feature::AnchorRef> {
        self.joint.pick_first.as_ref().map(|(_, a)| a.clone())
    }

    #[cfg(test)]
    pub(crate) fn joint_pick_active_for_test(&self) -> bool {
        self.joint.pick_faces
    }

    /// WHETHER THE LIVE GEOMETRY an anchor derives its axes from is ready.
    ///
    /// The origin of a part and a base plane are given by the component itself - they need no kernel. Faces,
    /// edges and vertices live in the B-rep: while it is absent, there is nowhere to take the anchor's axes
    /// from.
    fn geometry_ready_for_anchor(&self, anchor: &qymcad_core::feature::AnchorRef) -> bool {
        use qymcad_core::feature::AnchorRef;
        match anchor {
            AnchorRef::Origin | AnchorRef::BasePlane(_) => true,
            // A FACE IS ENOUGH BY ITSELF. This used to demand a LIVE B-rep for any anchor at all, and that
            // locked away exactly what is worked with: right after a document is opened not one body has a
            // live B-rep (it comes up on demand, in the background), while the face partition exists for
            // ALL of them - measured on a real document: 138 bodies out of 138. An anchor on a face needs
            // no live body whatsoever: the centre, the normal and the principal direction of the face are
            // already computed and sit in the model.
            //
            // Reported: faces could be picked only through the "origin" button and no other way. That is
            // exactly what happened: a click on a face ran into this gate and was told the geometry was
            // being prepared, one moment - and the moment never ended.
            AnchorRef::FaceCenter(b, _) => self.project.regen_faces.get(b).is_some_and(|f| !f.is_empty()) || self.live.shapes.contains_key(b),
            // AN EDGE AND A VERTEX DO NEED THE MODEL EDGES: the anchor direction of an edge is read from
            // them, and without them the joint would take the WORLD axes. Here there really is something to
            // wait for.
            AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => self.project.regen_edges.get(b).is_some_and(|e| !e.is_empty()) || self.live.shapes.contains_key(b),
        }
    }

    /// A click on a body while "by origins" is on -> the `Origin` anchor of its PART.
    ///
    /// The body here is only a way to point at the part: the anchor becomes the origin of the component
    /// rather than anything on its surface. So any click will do - a face, an edge, wherever it landed.
    pub(super) fn joint_pick_origin_click(&mut self, body: Id) {
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-face-no-part");
            return;
        };
        self.joint_pick_anchor_click(owner, qymcad_core::feature::AnchorRef::Origin);
    }


    /// AN ANCHOR INFERRED UNDER THE CURSOR, by the body it was found on.
    ///
    /// One door for every sort of anchor: the sort is no longer declared in advance by a switch, and reading
    /// a click stopped depending on what was chosen in the tool bar a minute earlier.
    pub(super) fn joint_pick_anchor_at(&mut self, body: Id, anchor: qymcad_core::feature::AnchorRef) {
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-anchor-no-part");
            return;
        };
        self.joint_pick_anchor_click(owner, anchor);
    }


    /// A thin wrapper: a click on A FACE -> a `FaceCenter` anchor.
    #[cfg(test)]
    pub(super) fn joint_pick_face_click(&mut self, body: Id, key: qymcad_core::feature::FaceKey) {
        // while "by origins" is on, a face is only a way to point at the part.
        if self.joint.anchor_mode == 3 {
            self.joint_pick_origin_click(body);
            return;
        }
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-face-no-part");
            return;
        };
        self.joint_pick_anchor_click(owner, qymcad_core::feature::AnchorRef::FaceCenter(body, key));
    }


    /// A thin wrapper: a click on AN EDGE -> an `EdgeMid` axis anchor.
    #[cfg(test)]
    pub(super) fn joint_pick_edge_click(&mut self, body: Id, edge_id: u32) {
        let Some(owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-edge-no-part");
            return;
        };
        self.joint_pick_anchor_click(owner, qymcad_core::feature::AnchorRef::EdgeMid(body, edge_id));
    }


    /// Apply a newly picked anchor while editing (swap the face, edge or vertex of A or B on the fly). The
    /// checks are the same as at creation: the part is in the active context, and the anchors are not in one
    /// subassembly. The connector is updated through the core's `set_connector_anchor`, then a rebuild.
    pub(super) fn joint_edit_repick_apply(&mut self, body: Id, anchor: qymcad_core::feature::AnchorRef) {
        let Some((jid, is_b)) = self.joint.edit_repick else { return };
        let ctx = self.current_ctx_id();
        let Some(body_owner) = self.project.body_owner(body) else {
            self.status = crate::i18n::tr("j-anchor-no-part");
            return;
        };
        if !self.project.component_is_within(body_owner, ctx) {
            self.status = crate::i18n::tr("j-outside-assembly");
            return;
        }
        let place = self.project.ancestor_child_of(ctx, body_owner).unwrap_or(body_owner);
        let Some(j) = self.project.joints.iter().find(|x| x.id == jid) else { return };
        let (cid, other) = if is_b { (j.b, j.a) } else { (j.a, j.b) };
        if self.project.connector(other).map(|c| c.owner) == Some(place) {
            self.status = crate::i18n::tr("j-same-part");
            return;
        }
        self.project.set_connector_anchor(cid, place, anchor);
        self.joint.edit_repick = None;
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.status = crate::i18n::tr1("jt-anchor-replaced", "side", if is_b { "B" } else { "A" });
    }


    /// TAKE A PICK FOR THE SECONDARY AXIS: a click on an edge or a face sets the axis of the connector.
    ///
    /// The direction is taken from the picked geometry and laid perpendicular to the main axis of the
    /// anchor - the pick sets THE SIDE, it does not replace the anchor. Geometry that has no side (a vertex,
    /// the origin of a part) never gets here: it does not enter the pick.
    pub(super) fn joint_axis_pick_apply(&mut self, anchor: qymcad_core::feature::AnchorRef) {
        let Some(cid) = self.joint.axis_pick else { return };
        if self.project.anchor_direction(&anchor).is_none() {
            self.status = crate::i18n::tr("j-axis-no-direction");
            return;
        }
        self.project.set_connector_axis_ref(cid, Some(anchor));
        self.joint.axis_pick = None;
        self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        self.status = crate::i18n::tr("j-axis-set");
    }

    /// The geometry of the DOF gizmo handles for a kind of joint: (slot [0=angle, 1=offset, 2=offset2],
    /// whether it is a ring, the frame axis 0/1/2). It matches `JointKind::motion`: the angle turns about Z;
    /// translations and extra angles follow the axes, as in `motion`.
    pub(super) fn joint_slot_geom(kind: qymcad_core::feature::JointKind) -> Vec<(u8, bool, u8)> {
        use qymcad_core::feature::JointKind::*;
        match kind {
            Rigid => vec![],
            Revolute => vec![(0, true, 2)],                        // an angle about Z
            Slider => vec![(1, false, 2)],                         // a shift along Z
            Cylindrical => vec![(0, true, 2), (1, false, 2)],      // an angle and a shift along Z
            PinSlot => vec![(0, true, 2), (1, false, 0)],          // an angle about Z plus a shift along X
            Planar => vec![(0, true, 2), (1, false, 0), (2, false, 1)], // an angle about Z plus shifts along X and Y
            Ball => vec![(0, true, 2), (1, true, 0), (2, true, 1)],     // angles about Z, X and Y
            // parallelism has no handles: it holds a direction, not a value - there is nothing to drag
            Parallel => vec![],
        }
    }


    /// The DOF gizmo handles of a joint in the frame of the context: (the origin, [(slot, whether it is a
    /// ring, the direction axis)]). Parameters driven by an expression (`feat_dim`) are NOT free to drag and
    /// are left out.
    pub(super) fn joint_giz_handles(&self, jid: Id) -> Option<([f64; 3], Vec<(u8, bool, [f64; 3])>)> {
        let kind = self.project.joints.iter().find(|x| x.id == jid)?.kind;
        let m = self.project.joint_frame(jid, self.current_ctx_id())?;
        let o = [m[3], m[7], m[11]];
        let key = |slot: u8| match slot {
            0 => "angle",
            1 => "offset",
            _ => "offset2",
        };
        let mut hs = Vec::new();
        for (slot, ring, _ax) in Self::joint_slot_geom(kind) {
            if self.project.feat_dim(jid, key(slot)).map_or(false, |e| !e.trim().is_empty()) {
                continue; // an expression drives this parameter - dragging must not touch it
            }
            // THE DIRECTION OF A DEGREE IS ASKED OF THE CORE rather than derived from the joint frame. The
            // frame is built on the FIRST anchor, while for a pin-slot the travel belongs to the SECOND: the
            // arrow pointed one way and the part moved another. There must be one source of direction - the
            // same one the solver computes by.
            let Some(dir) = self.project.joint_slot_axis(jid, slot as usize, self.current_ctx_id()) else { continue };
            hs.push((slot, ring, dir));
        }
        Some((o, hs))
    }


    /// The DOF gizmo handle under the cursor: (slot, whether it is a ring). Arrows take priority over rings,
    /// as in the six-degree gizmo.
    pub(super) fn joint_handle_hit(&self, jid: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<(u8, bool)> {
        let (o, hs) = self.joint_giz_handles(jid)?;
        let l = 60.0 / self.cam.scale as f64;
        // the translation arrows first
        for &(slot, ring, dir) in hs.iter().filter(|(_, r, _)| !*r) {
            let _ = ring;
            let s0 = self.project3(o, rect, basis).0;
            let s1 = self.project3([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l], rect, basis).0;
            if screen_dist_seg(pp, s0, s1) <= 13.0 {
                return Some((slot, false));
            }
        }
        // then the rotation rings
        for &(slot, ring, dir) in hs.iter().filter(|(_, r, _)| *r) {
            let _ = ring;
            if self.joint_ring_screen_dist(o, dir, l, rect, basis, pp) <= 10.0 {
                return Some((slot, true));
            }
        }
        None
    }


    /// The minimum screen distance to a ring of radius `l` about the axis `dir` centred at `o` (for hit
    /// testing and for drawing).
    pub(super) fn joint_ring_screen_dist(&self, o: [f64; 3], dir: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> f32 {
        let (u, v) = perp_basis(dir);
        let mut prev: Option<Pos2> = None;
        let mut dmin = f32::MAX;
        for k in 0..=48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
            let s = self.project3(p, rect, basis).0;
            if let Some(pr) = prev {
                dmin = dmin.min(screen_dist_seg(pp, pr, s));
            }
            prev = Some(s);
        }
        dmin
    }


    /// Start dragging a DOF gizmo handle: pin the frame (o and dir) and the starting value of the parameter.
    pub(super) fn joint_giz_begin(&mut self, jid: Id, slot: u8, ring: bool) {
        let Some((o, hs)) = self.joint_giz_handles(jid) else { return };
        let Some(&(_, _, dir)) = hs.iter().find(|(s, r, _)| *s == slot && *r == ring) else { return };
        // the drag starts from THE DRIVER if there is one, otherwise from the reading: the pull must start
        // from what was asked for, not from how it ended up if the request could not be met
        let start = self.project.joints.iter().find(|x| x.id == jid).map(|j| j.drive[(slot as usize).min(2)].unwrap_or([j.angle, j.offset, j.offset2][slot as usize])).unwrap_or(0.0);
        // THE BOUNDARY OF THE OPERATION OPENS ONCE - FOR THE WHOLE DRAG, NOT FOR EVERY FRAME.
        //
        // `begin_edit` takes A FULL COPY of the document, and until now every frame of the drag took one
        // (opened and closed inside `apply_joint_giz`). On a real assembly - 138 bodies with meshes - one
        // frame of dragging cost 13-18 ms for THAT ALONE, before any drawing. It reads as the part following
        // reluctantly: the program cannot keep up with the mouse, and the joints catch up late.
        //
        // A drag is ONE operation lasting across frames; that is exactly what `begin_edit` is for. The copy
        // is taken at the grab, and the undo step is closed on release (`joint_giz_end`).
        self.begin_edit(&crate::i18n::tr("status-edit-joint"));
        self.joint.giz_drag = Some(JointGizDrag { jid, slot, ring, start, amt: 0.0, o, dir });
    }

    /// RELEASE THE PART - the end of the lasting drag operation.
    ///
    /// One door for the frame and for the checks: the undo step is closed exactly here, and only here.
    pub(super) fn joint_giz_end(&mut self) {
        if self.part_pull.take().is_some() {
            self.project.drag_pull = None;
            self.project.solve_joints();
            self.invalidate_placement();
            self.commit_edit();
            self.after_placement_change();
            return;
        }
        let was_dragging = self.joint.giz_drag.is_some();
        self.joint.giz_drag = None;
        self.joint.giz_handle = None;
        if was_dragging {
            self.commit_edit(); // ONE undo step for the whole drag rather than one per frame
        }
        self.after_placement_change();
    }


    /// GRABBING THE PART ITSELF: take a part in the frame and pull, and it moves along the degrees of
    /// freedom it has left.
    ///
    /// The degree handles existed before, but pulling was possible ONLY by a gizmo arrow: miss it, and the
    /// mechanism does not stir. A professional CAD lets a part be grabbed anywhere. The difference is not
    /// convenience: while a thin arrow must be hit, the mechanism is never handled, and so it is never
    /// noticed that it was assembled wrongly.
    ///
    /// WHICH DEGREE IS BEING DRIVEN is decided by THE DIRECTION OF THE GRAB: the one whose axis on screen is
    /// closest to the motion of the cursor. A joint with one freedom leaves no choice and no question.
    pub(super) fn joint_grab_part_at(&mut self, rect: Rect, from: Pos2, towards: egui::Vec2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
        if self.joint.giz_drag.is_some() {
            return false; // a handle is already being held
        }
        let Some((body, _)) = self.pick_part_face_at(rect, from) else { return false };
        let Some(owner) = self.project.body_owner(body) else { return false };
        let ctx = self.current_ctx_id();
        // WHAT THE HAND DRIVES IS THE CORE'S BUSINESS (`drive_joint_in_context`): the question is not
        // "which node is visible from here" but "which joint ACTS on this part from here". The first
        // question used to be asked, and a joint lifted out of a subassembly into the root could not be
        // grabbed by its part - only by a gizmo handle.
        //
        // THE PART ITSELF IS PULLED, NOT ONE OF ITS DEGREES.
        //
        // Which component moves is again the core's business (`pull_target_component`): whether the grabbed
        // part takes part in joints itself or travels along with a subassembly. An empty answer means "not
        // driven by hand", and the drag must fall through to the view - hence `return false` here rather
        // than a silent swallow.
        //
        // THE GRAB POINT IS THE ORIGIN OF THE DRIVEN COMPONENT, and it needs no change of coordinates: the
        // part is rigid, and wherever the origin goes, everything else goes with it.
        if let Some(comp) = self.project.pull_target_component(owner, ctx) {
            let p0 = qymcad_core::feature::apply12(&self.project.relative_transform(comp, ctx), [0.0, 0.0, 0.0]);
            self.begin_edit(&crate::i18n::tr("status-edit-joint"));
            self.part_pull = Some((comp, [0.0, 0.0, 0.0], p0));
            return true;
        }
        let Some(jid) = self.project.drive_joint_in_context(owner, ctx) else { return false };
        let Some((o, hs)) = self.joint_giz_handles(jid) else { return false };
        if hs.is_empty() {
            return false; // no freedoms left - there is nothing to drive
        }
        // THE AXIS OF A DEGREE ON SCREEN, compared with the direction of the pull.
        let centre = self.project3(o, rect, basis).0;
        let len = 60.0 / self.cam.scale as f64;
        let mut best: Option<(f64, u8, bool)> = None;
        for &(slot, ring, dir) in &hs {
            let tip = [o[0] + dir[0] * len, o[1] + dir[1] * len, o[2] + dir[2] * len];
            let s = self.project3(tip, rect, basis).0 - centre;
            let n = (s.x * s.x + s.y * s.y).sqrt();
            if n < 1e-3 {
                continue; // the degree points into the screen: no motion tells it apart
            }
            // a rotation moves ACROSS its axis, a shift moves along it; the comparison is by magnitude and
            // the drag itself sorts out the sign
            let along = ((towards.x * s.x + towards.y * s.y) / n).abs() as f64;
            if best.map_or(true, |(b, _, _)| along > b) {
                best = Some((along, slot, ring));
            }
        }
        let Some((_, slot, ring)) = best else { return false };
        self.joint.giz_handle = Some((slot, ring));
        self.joint_giz_begin(jid, slot, ring);
        self.joint.giz_drag.is_some()
    }


    /// A PART IS BEING DRIVEN BY HAND RIGHT NOW - ONE QUESTION FOR THE WHOLE FRAME.
    ///
    /// There are two ways: a DOF gizmo handle (`joint.giz_drag`) and pulling the part itself (`part_pull`),
    /// and the mouse reader is indifferent to which - all it needs is whether the hand is busy. The question
    /// is brought into ONE door deliberately: while there were two, the frame asked only about the handle,
    /// and PULLING A PART DID NOT WORK AT ALL - the grab fired but the drag never reached
    /// `joint_giz_drag_to`. The checks did not catch it, because they called the drag directly, going around
    /// the reading of the frame.
    pub(crate) fn joint_drag_active(&self) -> bool {
        self.joint.giz_drag.is_some() || self.part_pull.is_some()
    }

    /// THE TEST FACADES FOR DRAGGING: the same calls the mouse reader makes in a frame.
    #[cfg(test)]
    pub(crate) fn joint_grab_part_at_for_test(&mut self, rect: Rect, from: Pos2, towards: egui::Vec2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
        self.joint_grab_part_at(rect, from, towards, basis)
    }


    #[cfg(test)]
    pub(crate) fn joint_giz_drag_to_for_test(&mut self, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        self.joint_giz_drag_to(cursor, d, rect, basis);
    }


    /// Release the part - the same thing `drag_stopped` does in a frame.
    #[cfg(test)]
    pub(crate) fn joint_giz_end_for_test(&mut self) {
        self.joint_giz_end();
    }


    /// Dragging a DOF gizmo handle: accumulate degrees or millimetres along the pinned frame, write them
    /// into the joint parameter, and solve.
    pub(super) fn joint_giz_drag_to(&mut self, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
        if let Some((comp, local, from)) = self.part_pull {
            // WHERE THE HAND LEADS is computed in the coordinates of WHAT IS ON SCREEN, that is, of the
            // current context (`body_display_transform` draws everything relative to it). The solver, on
            // the other hand, lives in THE WORLD, so the target is converted to world coordinates just
            // before it is handed over.
            let k = 1.0 / self.cam.scale as f64;
            let (right, up, _) = basis;
            let mut to = from;
            for a in 0..3 {
                to[a] += right[a] * d.x as f64 * k - up[a] * d.y as f64 * k;
            }
            self.part_pull = Some((comp, local, to));
            let ctx = self.current_ctx_id();
            let to_world = qymcad_core::feature::apply12(&self.project.world_transform(ctx), to);
            self.project.drag_pull = Some((comp, local, to_world));
            self.project.solve_joints();
            self.project.drag_pull = None;
            self.invalidate_placement();
            let _ = (cursor, rect);
            return;
        }
        let Some(dg) = self.joint.giz_drag else { return };
        let inc = if dg.ring {
            let center = self.project3(dg.o, rect, basis).0;
            let radial = cursor - center;
            let r2 = (radial.x * radial.x + radial.y * radial.y) as f64;
            if r2 < 4.0 {
                return;
            }
            let ccw = -(radial.x * d.y - radial.y * d.x) as f64 / r2;
            // THE SIGN COMES FROM THE SINGLE DEFINITION (`ring_drag_sign`), the same one the body gizmo
            // uses.
            //
            // A formula of its own used to stand here, and it gave EXACTLY THE OPPOSITE sign: an axis
            // towards the viewer (a projection on the view direction below zero) got -1 instead of +1. The
            // body gizmo turned correctly while the joint gizmo turned against the mouse, and no reading of
            // the code could show it: both formulas look sensible and differ only in the answer. The
            // convention is documented at `ring_drag_sign`: an axis TOWARDS THE VIEWER plus a visually
            // counter-clockwise motion is a positive angle (the right-hand rule).
            let depth = dg.dir[0] * basis.2[0] + dg.dir[1] * basis.2[1] + dg.dir[2] * basis.2[2];
            ccw.to_degrees() * super::ring_drag_sign(depth)
        } else {
            let l = 60.0 / self.cam.scale as f64;
            let s0 = self.project3(dg.o, rect, basis).0;
            let s1 = self.project3([dg.o[0] + dg.dir[0] * l, dg.o[1] + dg.dir[1] * l, dg.o[2] + dg.dir[2] * l], rect, basis).0;
            let pd = s1 - s0;
            let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
            if denom < 1e-6 {
                return;
            }
            (d.x * pd.x + d.y * pd.y) as f64 * l / denom
        };
        if let Some(dg) = &mut self.joint.giz_drag {
            dg.amt += inc;
        }
        self.apply_joint_giz();
    }


    /// The value of the parameter with snapping applied (the degree or millimetre step from the panel).
    pub(super) fn joint_giz_value(&self, snap: bool) -> Option<(f64, bool)> {
        let dg = self.joint.giz_drag?;
        let step = if dg.ring { self.set.snap.rot_deg.max(0.1) } else { self.set.snap.grid.max(0.01) };
        let amt = if snap { (dg.amt / step).round() * step } else { dg.amt };
        Some((dg.start + amt, dg.ring))
    }


    /// The screen points of connectors A and B of a joint in the active context (world coordinates through
    /// `world_transform`).
    pub(super) fn joint_endpoints(&self, j: &qymcad_core::feature::Joint, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<(Pos2, Pos2)> {
        let ctx = self.current_ctx_id();
        let pt = |cid: Id| -> Option<Pos2> {
            let conn = self.project.connector(cid)?;
            let fr = self.project.connector_frame(conn)?;
            let w = qymcad_core::feature::apply12(&self.project.relative_transform(conn.owner, ctx), fr.origin);
            Some(self.project3(w, rect, basis).0)
        };
        Some((pt(j.a)?, pt(j.b)?))
    }


    /// The joint EDIT popup AT THE GEOMETRY (a double click on the glyph): EVERY parameter of the joint -
    /// the angle, the offset and the second offset as drag values, the `f=` expressions over global
    /// variables, flipping the side, driving from the root, and the min/max limits. Anchors A and B are
    /// edited in the TOP bar `joint_edit_bar` ("swap anchor"). One tool-command experience. A single click
    /// does NOT open the popup - it only shows the gizmo of the freedoms.
    pub(super) fn joint_popup(&mut self, ctx: &egui::Context, rect: Rect) {
        use qymcad_core::feature::{AnchorRef, JointKind};
        let Some(jid) = self.joint.edit else { return };
        if !matches!(self.workbench, Workbench::Assembly) || !self.mode_3d {
            return;
        }
        let Some(j) = self.project.joints.iter().find(|x| x.id == jid).cloned() else {
            self.joint.edit = None; // the joint is gone (deleted) - leave the edit
            return;
        };
        let basis = self.cam.basis();
        let mid = self
            .joint_endpoints(&j, rect, &basis)
            .map(|(a, b)| Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
            .unwrap_or_else(|| rect.center());
        let face_rigid = matches!(j.kind, JointKind::Rigid)
            && self.project.connector(j.a).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)))
            && self.project.connector(j.b).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)));
        let nested = self.project.joint_home(&j).is_some_and(|h| h != self.project.root);
        // A RIGID JOINT ALSO SHOWS AN ANGLE: it has no freedom, but it has two parameters - the gap and
        // the ROTATION about the axis of the joint. While the field was missing, there was nothing to turn
        // a fastened part with.
        let has_angle = matches!(j.kind, JointKind::Rigid | JointKind::Revolute | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Ball | JointKind::Planar);
        let has_off = matches!(j.kind, JointKind::Rigid | JointKind::Slider | JointKind::Cylindrical | JointKind::PinSlot | JointKind::Planar | JointKind::Ball);
        let has_off2 = matches!(j.kind, JointKind::Planar | JointKind::Ball);
        // the slot captions come from one definition (`joint_slot_label`) rather than from a copy per place
        let (off_lbl, off2_lbl) = (joint_slot_label(j.kind, 1).0, joint_slot_label(j.kind, 2).0);
        let free = j.kind.free_slots();
        let mut changed = false;
        let mut close = false;
        egui::Area::new(egui::Id::new("joint_edit_popup")).fixed_pos(self.clamp_popup(mid, rect) + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(260.0);
                ui.horizontal(|ui| {
                    // THE NAME GOES THROUGH THE NAME TRANSLATOR. The name of a joint is stored as a
                    // catalogue key with a number (`name-joint-kind-rigid-n#3`), and printed as it is it
                    // shows a service code. That is exactly what an earlier screenshot about the list of
                    // kinds was about.
                    ui.label(egui::RichText::new(format!("{} {}", ph::LINK, crate::i18n::name(&j.name))).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(ph::X).on_hover_text(&crate::i18n::tr("j-done-esc")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.separator();
                // the direct drag values plus the toggles for the side and for driving from the root
                if let Some(jj) = self.project.joints.iter_mut().find(|x| x.id == jid) {
                    ui.horizontal_wrapped(|ui| {
                        if has_angle {
                            changed |= joint_slot_drag(ui, jj, 0, 1.0);
                        }
                        if has_off {
                            changed |= joint_slot_drag(ui, jj, 1, 0.5);
                        }
                        if has_off2 {
                            changed |= joint_slot_drag(ui, jj, 2, 0.5);
                        }
                    });
                    if nested {
                        changed |= ui.checkbox(&mut jj.global, &crate::i18n::tr("j-drive-from-root")).on_hover_text(&crate::i18n::tr("j-expose-hint")).changed();
                    }
                }
                // THE SIDE OF THE JOINT GOES THROUGH THE CORE'S DOOR, NOT THROUGH EDITING A FIELD.
                //
                // The tick box sat straight on the `flip` field, and the solver overwrites that same field
                // with its own answer: tick it, and the next solve saw "no side chosen", took the nearest
                // one (the same one) and silently cleared the tick. What is shown is the side IN EFFECT,
                // and a click asks for the opposite one.
                if face_rigid {
                    let mut on = self.project.joint_side_flipped(jid);
                    if ui.checkbox(&mut on, &crate::i18n::tr("j-flip-side")).on_hover_text(&crate::i18n::tr("j-coplanar-hint")).changed() {
                        self.project.flip_joint_side(jid);
                        changed = true;
                    }
                }
                // RUNNING A DEGREE THROUGH (an animation): once a mechanism is assembled, it can be watched
                // moving. Numbers do not show this, and dragging a part by mouse to make sure it reaches the
                // end is guesswork, not a check.
                let anim_on = self.joint_anim.as_ref().is_some_and(|a| a.joint == jid);
                ui.horizontal(|ui| {
                    if anim_on {
                        if ui.button(format!("{} {}", ph::STOP, crate::i18n::tr("j-anim-stop"))).clicked() {
                            self.stop_joint_anim();
                        }
                    } else {
                        for slot in 0..3 {
                            if self.project.joint_anim_range(jid, slot).is_none() {
                                continue; // this degree has nowhere to run - so it gets no button
                            }
                            let label = crate::i18n::tr(match slot {
                                0 => "j-anim-angle",
                                1 => "j-anim-offset",
                                _ => "j-anim-offset2",
                            });
                            if ui.button(format!("{} {label}", ph::PLAY)).on_hover_text(&crate::i18n::tr("j-anim-hint")).clicked() {
                                self.start_joint_anim(jid, slot);
                            }
                        }
                        // A "LIMITS REQUIRED" NOTICE USED TO STAND HERE. A slider with no bounds had
                        // nowhere to run, and the reason was explained - but the reason was made up: a
                        // rotation did have a default range (a full turn), it is only travel that has no
                        // "full turn". Now default travel is MEASURED off the part itself
                        // (`joint_anim_range`), and there is nothing to explain: the run button is always
                        // there.
                    }
                });
                // "KEEP AS IT STANDS" (as built): the joint declares THE CURRENT placement to be its own.
                // Needed where the parts already stand as they should - placed by hand or arrived by import:
                // there is no point mating them, the joint is there only to keep them from drifting apart.
                let as_built_now = self.project.joints.iter().find(|x| x.id == jid).and_then(|x| x.as_built).is_some();
                ui.horizontal(|ui| {
                    if ui.button(&crate::i18n::tr(if as_built_now { "j-as-built-again" } else { "j-as-built" })).on_hover_text(&crate::i18n::tr("j-as-built-hint")).clicked() {
                        if self.project.set_joint_as_built(jid) {
                            self.status = crate::i18n::tr("j-as-built-ok");
                            changed = true;
                        } else {
                            // NO SILENT REFUSAL: the anchor may have failed to resolve, and that must be
                            // learned here rather than from a part that does not move.
                            self.status = crate::i18n::tr("j-as-built-failed");
                        }
                    }
                    if as_built_now && ui.button(&crate::i18n::tr("j-as-built-off")).clicked() {
                        self.project.clear_joint_as_built(jid);
                        self.status = crate::i18n::tr("j-as-built-cleared");
                        changed = true;
                    }
                });
                // THE ANCHORS OF A JOINT - tuning them IS the answer to "why did the part end up in the
                // wrong place". The attachment point on a cylinder and the turn of the secondary axis solve
                // that directly, and without them there is nothing to correct a wrong guess with.
                changed |= self.connector_controls(ui, jid);
                // the parametric expression fields (global variables, `f=`) work like sketch dimensions
                if has_angle {
                    ui.horizontal(|ui| {
                        ui.label(&crate::i18n::tr("j-angle-expr"));
                        self.dim_expr_field_in(ui, jid, "angle", "joint_popup");
                    });
                }
                if has_off {
                    ui.horizontal(|ui| {
                        ui.label(format!("ƒ {off_lbl}"));
                        self.dim_expr_field_in(ui, jid, "offset", "joint_popup");
                    });
                }
                if has_off2 {
                    ui.horizontal(|ui| {
                        ui.label(format!("ƒ {off2_lbl}"));
                        self.dim_expr_field_in(ui, jid, "offset2", "joint_popup");
                    });
                }
                // the min/max limits over the free slots - always expanded
                if free.iter().any(|&f| f) {
                    ui.separator();
                    ui.label(egui::RichText::new(&crate::i18n::tr("j-limits")).strong());
                    if let Some(jj) = self.project.joints.iter_mut().find(|x| x.id == jid) {
                        for slot in 0..3usize {
                            if !free[slot] {
                                continue;
                            }
                            changed |= joint_slot_limits(ui, jj, slot);
                        }
                    }
                }
            });
        });
        if changed {
            self.mark_dirty_for_rebuild(); // the document is marked; the planner does the counting
        }
        if close {
            self.exit_joint_edit();
        }
    }


    /// Whether a joint is visible in the current view: the joints tick box is on, the workbench is Assembly,
    /// and the joint's home is the active context (the joints of nested subassemblies are not shown in the
    /// parent - they get in the way).
    pub(super) fn joint_visible(&self, j: &qymcad_core::feature::Joint) -> bool {
        self.set.show_joints
            && matches!(self.workbench, Workbench::Assembly)
            && self.project.joint_in_context(j, self.current_ctx_id())
    }


    /// The screen positions of joint glyphs: THE MIDPOINT between A and B. One source for both drawing and
    /// hit testing.
    pub(super) fn joint_glyphs(&self, rect: Rect) -> Vec<(Id, Pos2, qymcad_core::feature::JointKind)> {
        let basis = self.cam.basis();
        self.project
            .joints
            .iter()
            .filter(|j| self.joint_visible(j))
            .filter_map(|j| {
                let (a, b) = self.joint_endpoints(j, rect, &basis)?;
                Some((j.id, Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5), j.kind))
            })
            .collect()
    }


    /// The joint under the cursor in 3D (by proximity to its glyph) - for picking and for hover.
    pub(super) fn joint_glyph_at(&self, rect: Rect, pos: Pos2) -> Option<Id> {
        self.joint_glyphs(rect).into_iter().find(|(_, at, _)| at.distance(pos) <= 11.0).map(|(id, _, _)| id)
    }
    /// TUNING THE ANCHORS OF A JOINT: where exactly they sit and how they are turned.
    ///
    /// This brings into the interface what is done with a mate connector elsewhere: choose the attachment
    /// point (on a hole, the middle or an end face), turn the secondary axis, slide along the main one.
    /// Without these handles the only way to correct a position is to move the part at random.
    pub(super) fn connector_controls(&mut self, ui: &mut egui::Ui, jid: Id) -> bool {
        
        let Some((ca, cb)) = self.project.joints.iter().find(|j| j.id == jid).map(|j| (j.a, j.b)) else { return false };
        let mut changed = false;
        // EXPANDED BY DEFAULT. Tuning the anchors IS the answer to "why did the part end up in the wrong
        // place": the attachment point, the turn, the offsets, the pick of an axis. Hidden behind a
        // collapsed header, it is not found by whoever is in trouble - and they are exactly who looks for
        // it.
        egui::CollapsingHeader::new(&crate::i18n::tr("j-anchors")).id_salt("joint_anchors").default_open(true).show(ui, |ui| {
            for (tag, cid) in [("A", ca), ("B", cb)] {
                changed |= self.one_connector_controls(ui, tag, cid);
            }
        });
        changed
    }

    /// THE HANDLES OF ONE ANCHOR: the attachment point, the turn, the axis pick, the offsets.
    ///
    /// They are split out because an anchor became AN ELEMENT IN ITS OWN RIGHT: the same handles are needed
    /// in the list of anchors, where there is no joint anywhere nearby. A copy of them would be a second
    /// truth about how an anchor is tuned - and would part from the first at the very first edit.
    pub(super) fn one_connector_controls(&mut self, ui: &mut egui::Ui, tag: &str, cid: Id) -> bool {
        use qymcad_core::asm::connector::AttachPoint;
        let mut changed = false;
        {
            {
                let Some(c) = self.project.connectors.iter().find(|c| c.id == cid) else { return false };
                // an attachment point means something only where the geometry has a length along its axis
                let axial = matches!(c.anchor, qymcad_core::feature::AnchorRef::FaceCenter(..) | qymcad_core::feature::AnchorRef::EdgeMid(..));
                let (mut point, mut rot, mut off) = (c.point, c.rot_deg, c.offset_xyz);
                ui.horizontal(|ui| {
                    ui.label(format!("{tag}:"));
                    if axial {
                        for p in [AttachPoint::Middle, AttachPoint::Start, AttachPoint::End] {
                            if ui.selectable_label(point == p, crate::i18n::tr(p.label())).on_hover_text(&crate::i18n::tr("j-offset-hint")).clicked() {
                                point = p;
                                changed = true;
                            }
                        }
                    }
                    // THE TURN: a +90 deg button for the common case and a field for anything else. The
                    // angle used to be stored in QUARTER TURNS, and there was nothing to set a slot at
                    // 30 deg to the axis of a part with.
                    if ui.button(format!("{} 90{}", ph::ARROW_CLOCKWISE, crate::i18n::tr("unit-deg-suffix"))).on_hover_text(&crate::i18n::tr("j-roll-hint")).clicked() {
                        rot = (rot + 90.0).rem_euclid(360.0);
                        changed = true;
                    }
                    changed |= ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix(crate::i18n::tr("unit-deg-suffix"))).on_hover_text(&crate::i18n::tr("j-roll-hint")).changed();
                });
                // THE SECONDARY AXIS BY A PICK (the "second pick"). It is normally derived from the
                // geometry, but a square face has no long side at all and the answer is arbitrary - then an
                // edge is pointed at by hand.
                ui.horizontal(|ui| {
                    let armed = self.joint.axis_pick == Some(cid);
                    let has = self.project.connector(cid).is_some_and(|c| c.axis_ref.is_some());
                    if ui
                        .selectable_label(armed, format!("{} {}", ph::CROSSHAIR, crate::i18n::tr("j-axis-pick")))
                        .on_hover_text(&crate::i18n::tr("j-axis-pick-hint"))
                        .clicked()
                    {
                        self.joint.axis_pick = if armed { None } else { Some(cid) };
                    }
                    if has && ui.button(&crate::i18n::tr("j-axis-auto")).on_hover_text(&crate::i18n::tr("j-axis-auto-hint")).clicked() {
                        changed |= self.project.set_connector_axis_ref(cid, None);
                    }
                });
                // AN OFFSET ALONG ALL THREE AXES OF THE CONNECTOR rather than along the main one only: an
                // anchor almost never coincides with the centre of a face exactly, and there was nothing to
                // move it sideways with - the part got moved at random instead.
                ui.horizontal(|ui| {
                    ui.label(&crate::i18n::tr("j-offset-lower"));
                    for (k, name) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.label(*name);
                        changed |= ui
                            .add(egui::DragValue::new(&mut off[k]).speed(0.1).suffix(crate::i18n::tr("unit-mm-suffix")))
                            .on_hover_text(&crate::i18n::tr("j-slide-hint"))
                            .changed();
                    }
                });
                if changed {
                    if let Some(c) = self.project.connectors.iter_mut().find(|c| c.id == cid) {
                        c.point = point;
                        c.rot_deg = rot;
                        c.offset_xyz = off;
                    }
                }
            }
        }
        changed
    }
}
