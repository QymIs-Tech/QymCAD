//! THE POPUP OF A JOINT at the geometry, and its values stepped.

use qymcad_ui_state::Notice;
use super::*;

/// Where the window of mate `jid` was placed last, kept between frames.
#[derive(Clone, Copy)]
pub(crate) struct PopupPin {
    pub(crate) joint: Id,
    pub(crate) at: Pos2,
}

/// WHERE THE WINDOW OF MATE `jid` STANDS: by the mate (`by_mate`), except while `anim` runs that same mate - then where
/// it stood when the run began. The window follows the view, not the motion it causes itself: a run of the travel of
/// a cylindrical mate carries anchor B along the axis, the midpoint of the anchors goes with it, and the window
/// travelled 21-41 px over one second of the run, the button that stops the run riding along under the pointer.
pub(crate) fn popup_place(ctx: &egui::Context, jid: Id, anim: Option<&qymcad_ui_state::JointAnim>, by_mate: Pos2) -> Pos2 {
    let id = egui::Id::new("joint_edit_popup_pin");
    let pinned = ctx.data(|d| d.get_temp::<PopupPin>(id)).filter(|p| p.joint == jid && anim.is_some_and(|a| a.joint == jid));
    let at = pinned.map_or(by_mate, |p| p.at);
    ctx.data_mut(|d| d.insert_temp(id, PopupPin { joint: jid, at }));
    at
}

/// The joint EDIT popup AT THE GEOMETRY (a double click on the glyph): EVERY parameter of the joint -
/// the angle, the offset and the second offset as drag values, the `f=` expressions over global
/// variables, flipping the side, driving from the root, and the min/max limits. Anchors A and B are
/// edited in the TOP bar `joint_edit_bar` ("swap anchor"). One tool-command experience. A single click
/// does NOT open the popup - it only shows the gizmo of the freedoms.
pub fn joint_popup(jc: &mut qymcad_ui_state::JointCtx, ctx: &egui::Context, rect: Rect) {
    use qymcad_core::feature::{AnchorRef, JointKind};
    let Some(jid) = jc.joint.edit else { return };
    if !matches!(jc.workbench, qymcad_ui_state::Workbench::Assembly) || !jc.mode_3d {
        return;
    }
    let Some(j) = jc.project.joints.iter().find(|x| x.id == jid).cloned() else {
        jc.joint.edit = None; // the joint is gone (deleted) - leave the edit
        return;
    };
    let basis = jc.cam.basis();
    let mid = qymcad_ui_state::joint_endpoints(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, &j, rect, &basis)
        .map(|(a, b)| Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
        .unwrap_or_else(|| rect.center());
    let face_rigid = matches!(j.kind, JointKind::Rigid)
        && jc.project.connector(j.a).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)))
        && jc.project.connector(j.b).is_some_and(|c| matches!(c.anchor, AnchorRef::FaceCenter(..)));
    let nested = jc.project.joint_home(&j).is_some_and(|h| h != jc.project.root);
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
    let before = jc.project.joints.clone();
    let vars = jc.project.param_map();
    let mut exprs = Vec::new();
    let at = popup_place(ctx, jid, jc.joint_anim.as_ref(), qymcad_ui_state::clamp_popup(mid, rect) + egui::vec2(12.0, -12.0));
    egui::Area::new(egui::Id::new("joint_edit_popup")).fixed_pos(at).order(qymcad_ui_state::AT_GEOMETRY).show(ctx, |ui| {
        qymcad_ui_state::at_geometry_ui(ui);
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(260.0);
            ui.horizontal(|ui| {
                // THE NAME GOES THROUGH THE NAME TRANSLATOR. The name of a joint is stored as a
                // catalogue key with a number (`name-joint-kind-rigid-n#3`), and printed as it is it
                // shows a service code. That is exactly what an earlier screenshot about the list of
                // kinds was about.
                ui.label(egui::RichText::new(format!("{} {}", ph::LINK, qymcad_i18n::name(&j.name))).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(ph::X).on_hover_text(qymcad_i18n::tr("j-done-esc")).clicked() {
                        close = true;
                    }
                });
            });
            ui.separator();
            // the direct drag values plus the toggles for the side and for driving from the root
            if let Some(jj) = jc.project.joints.iter_mut().find(|x| x.id == jid) {
                ui.horizontal_wrapped(|ui| {
                    if has_angle {
                        changed |= joint_slot_drag(ui, jj, 0, 1.0, &vars, &mut exprs);
                    }
                    if has_off {
                        changed |= joint_slot_drag(ui, jj, 1, 0.5, &vars, &mut exprs);
                    }
                    if has_off2 {
                        changed |= joint_slot_drag(ui, jj, 2, 0.5, &vars, &mut exprs);
                    }
                });
                if nested {
                    changed |= ui.checkbox(&mut jj.global, qymcad_i18n::tr("j-drive-from-root")).on_hover_text(qymcad_i18n::tr("j-expose-hint")).changed();
                }
            }
            // THE SIDE OF THE JOINT GOES THROUGH THE CORE'S DOOR, NOT THROUGH EDITING A FIELD.
            //
            // The tick box sat straight on the `flip` field, and the solver overwrites that same field
            // with its own answer: tick it, and the next solve saw "no side chosen", took the nearest
            // one (the same one) and silently cleared the tick. What is shown is the side IN EFFECT,
            // and a click asks for the opposite one.
            if face_rigid {
                let mut on = jc.project.joint_side_flipped(jid);
                if ui.checkbox(&mut on, qymcad_i18n::tr("j-flip-side")).on_hover_text(qymcad_i18n::tr("j-coplanar-hint")).changed() {
                    jc.project.flip_joint_side(jid);
                    changed = true;
                }
            }
            // RUNNING A DEGREE THROUGH (an animation): once a mechanism is assembled, it can be watched
            // moving. Numbers do not show this, and dragging a part by mouse to make sure it reaches the
            // end is guesswork, not a check.
            let anim_on = jc.joint_anim.as_ref().is_some_and(|a| a.joint == jid);
            ui.horizontal(|ui| {
                if anim_on {
                    if ui.button(format!("{} {}", ph::STOP, qymcad_i18n::tr("j-anim-stop"))).clicked() {
                        stop_joint_anim(jc);
                    }
                } else {
                    for slot in 0..3 {
                        if jc.project.joint_anim_range(jid, slot).is_none() {
                            continue; // this degree has nowhere to run - so it gets no button
                        }
                        let label = qymcad_i18n::tr(match slot {
                            0 => "j-anim-angle",
                            1 => "j-anim-offset",
                            _ => "j-anim-offset2",
                        });
                        if ui.button(format!("{} {label}", ph::PLAY)).on_hover_text(qymcad_i18n::tr("j-anim-hint")).clicked() {
                            qymcad_ui_state::start_joint_anim(jc.joint_anim, jc.project, jid, slot);
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
            let as_built_now = jc.project.joints.iter().find(|x| x.id == jid).and_then(|x| x.as_built).is_some();
            ui.horizontal(|ui| {
                if ui.button(qymcad_i18n::tr(if as_built_now { "j-as-built-again" } else { "j-as-built" })).on_hover_text(qymcad_i18n::tr("j-as-built-hint")).clicked() {
                    if jc.project.set_joint_as_built(jid) {
                        *jc.status = Notice::info(qymcad_i18n::tr("j-as-built-ok"));
                        changed = true;
                    } else {
                        // NO SILENT REFUSAL: the anchor may have failed to resolve, and that must be
                        // learned here rather than from a part that does not move.
                        *jc.status = Notice::error(qymcad_i18n::tr("j-as-built-failed"));
                    }
                }
                if as_built_now && ui.button(qymcad_i18n::tr("j-as-built-off")).clicked() {
                    jc.project.clear_joint_as_built(jid);
                    *jc.status = Notice::info(qymcad_i18n::tr("j-as-built-cleared"));
                    changed = true;
                }
            });
            // THE ANCHORS OF A JOINT - tuning them IS the answer to "why did the part end up in the
            // wrong place". The attachment point on a cylinder and the turn of the secondary axis solve
            // that directly, and without them there is nothing to correct a wrong guess with.
            changed |= connector_controls(jc, ui, jid);
            // the parametric expression fields (global variables, `f=`) work like sketch dimensions
            if has_angle {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr("j-angle-expr"));
                    dim_expr_field_in(&mut jc.rebuild(), ui, jid, "angle", "joint_popup");
                });
            }
            if has_off {
                ui.horizontal(|ui| {
                    ui.label(format!("ƒ {off_lbl}"));
                    dim_expr_field_in(&mut jc.rebuild(), ui, jid, "offset", "joint_popup");
                });
            }
            if has_off2 {
                ui.horizontal(|ui| {
                    ui.label(format!("ƒ {off2_lbl}"));
                    dim_expr_field_in(&mut jc.rebuild(), ui, jid, "offset2", "joint_popup");
                });
            }
            // the min/max limits over the free slots - always expanded
            if free.iter().any(|&f| f) {
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr("j-limits")).strong());
                if let Some(jj) = jc.project.joints.iter_mut().find(|x| x.id == jid) {
                    // THE INDEX IS AN ARGUMENT: `joint_slot_limits` needs the slot's number to know which
                    // of the three it is editing. Walking `free` by value would hide it.
                    for (slot, _) in free.iter().enumerate().filter(|(_, f)| **f) {
                        changed |= joint_slot_limits(ui, jj, slot);
                    }
                }
            }
        });
    });
    if let Err(e) = keep_slot_exprs(jc.project, jid, exprs) {
        *jc.status = qymcad_ui_state::Notice::refusal(qymcad_i18n::error_words::expr_error_text(&e));
    }
    joint_values_step(ctx, jc.edits, jc.project, before, changed);
    if changed {
        qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    }
    if close {
        joint_create_finish(jc, true); // the popup's Done keeps a joint just made
        qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    }
}

/// THE VALUES OF A JOINT TYPED OR DRAGGED IN A PANEL ARE ONE STEP OF UNDO. The field writes into the document while it
/// is typed or dragged, before it reports a change, and a rebuild in between takes the written value as the committed
/// state - so the step opens where the typing or the drag begins (`before` is the joints as they stood before the widgets
/// of this frame) and closes where it ends. A change of any other widget of the panel is laid as a step of its own.
/// Called after the widgets and before the solve, so the step begins before the part moves.
pub fn joint_values_step(ctx: &egui::Context, edits: &mut qymcad_ui_state::Edits, project: &mut qymcad_core::model::Project, before: Vec<qymcad_core::feature::Joint>, changed: bool) {
    let take = |key: &str| ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(key))).unwrap_or(false);
    let (began, ended) = (take("joint_values_began"), take("joint_values_ended"));
    let open_id = egui::Id::new("joint_values_open");
    let mut open = ctx.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false) && edits.open.is_some();
    if began && !open && edits.open.is_none() {
        let after = std::mem::replace(&mut project.joints, before);
        qymcad_ui_state::begin_edit(edits, project, qymcad_i18n::tr("status-edit-joint"));
        project.joints = after;
        open = true;
    }
    if open && ended {
        qymcad_ui_state::close_edit(edits, project);
        open = false;
    } else if changed && !open {
        qymcad_ui_state::settle_step(edits, project, qymcad_i18n::tr("status-edit-joint"), false);
    }
    ctx.data_mut(|d| d.insert_temp(open_id, open));
}
