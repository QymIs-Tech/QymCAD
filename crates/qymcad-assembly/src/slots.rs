//! THE SLOTS OF A JOINT: their labels, limits, drag and expressions; the controls of a connector.

use super::*;

/// THE NAME AND UNIT OF A SLOT for a given kind of joint - one definition for the whole interface.
pub fn joint_slot_label(kind: qymcad_core::feature::JointKind, slot: usize) -> (String, String) {
    use qymcad_core::feature::JointKind;
    match (slot, kind) {
        (0, _) => (qymcad_i18n::tr("j-angle-lower"), qymcad_i18n::tr("unit-deg-suffix")),
        (1, JointKind::Ball) => (qymcad_i18n::tr("j-angle-x"), qymcad_i18n::tr("unit-deg-suffix")),
        (1, JointKind::Planar) => (qymcad_i18n::tr("j-offset-x"), qymcad_i18n::tr("unit-mm-suffix")),
        (1, JointKind::Rigid) => (qymcad_i18n::tr("j-gap"), qymcad_i18n::tr("unit-mm-suffix")),
        (1, _) => (qymcad_i18n::tr("j-offset-lower"), qymcad_i18n::tr("unit-mm-suffix")),
        (2, JointKind::Ball) => (qymcad_i18n::tr("j-angle-y"), qymcad_i18n::tr("unit-deg-suffix")),
        _ => (qymcad_i18n::tr("j-offset-y"), qymcad_i18n::tr("unit-mm-suffix")),
    }
}

/// THE LIMITS OF ONE SLOT: a min and a max with tick boxes. One widget serves both the popup at the glyph
/// and the right-hand panel - these used to be two copies, and they drifted apart silently, exactly as the
/// copies of the value editor did.
pub fn joint_slot_limits(ui: &mut egui::Ui, jj: &mut qymcad_core::feature::Joint, slot: usize) -> bool {
    let (name, suff) = joint_slot_label(jj.kind, slot);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(name);
        let mut has_min = jj.limit_min[slot].is_some();
        if ui.checkbox(&mut has_min, qymcad_i18n::tr("j-min")).changed() {
            jj.limit_min[slot] = has_min.then_some(0.0);
            changed = true;
        }
        if let Some(v) = jj.limit_min[slot].as_mut() {
            changed |= ui.add(egui::DragValue::new(v).speed(0.5).suffix(suff.clone())).changed();
        }
        let mut has_max = jj.limit_max[slot].is_some();
        if ui.checkbox(&mut has_max, qymcad_i18n::tr("j-max")).changed() {
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
///
/// THE FIELD TAKES AN EXPRESSION, as every value field does: `90*2/2` or a parameter name is evaluated over `vars`
/// (the project's parameters) and turns the part as the number would. Reported behaviour: a number turned the part,
/// while `90*2/2` or `pa` turned it by 0 deg. A typed expression lands in `exprs` as (slot, text) for the caller to keep
/// as the slot's formula, so the part follows the parameter later; a typed number or a drag lands there as an empty
/// text, which takes an old formula off - the last thing typed is what drives.
pub fn joint_slot_drag(ui: &mut egui::Ui, jj: &mut qymcad_core::feature::Joint, slot: usize, speed: f64, vars: &std::collections::HashMap<String, f64>, exprs: &mut Vec<(usize, String)>) -> bool {
    let (label, suffix) = joint_slot_label(jj.kind, slot);
    let measured = match slot {
        0 => jj.angle,
        1 => jj.offset,
        _ => jj.offset2,
    };
    let driven = jj.drive[slot];
    let mut v = driven.unwrap_or(measured);
    ui.label(label);
    let typed = std::cell::RefCell::new(String::new());
    let parser = |s: &str| {
        let t = s.trim().replace('\u{2212}', "-");
        let v = qymcad_core::expr::eval(&t, vars).ok()?;
        *typed.borrow_mut() = if t.parse::<f64>().is_ok() { String::new() } else { t };
        Some(v)
    };
    // THE FIELD IS AS WIDE AS THE WIDEST NUMBER IT SHOWS, not as its text of the moment: a run passes angles with
    // decimals, and a field 40 px wide at "99" and 52 px at "100.12" jumped every frame, pushing the rest of the line
    // sideways. The decimals are fixed at two: egui picks them by the pixel size of the screen (three for a travel on
    // a 1x screen, two on a 2x one), and a width reserved for one count would not hold the other. Four digits before
    // the point cover a travel up to 9999 mm; a larger number widens the field as before.
    let widest = egui::WidgetText::from(format!("-8888.88{suffix}")).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, ui.style().drag_value_text_style.clone());
    let width = widest.size().x + 2.0 * ui.spacing().button_padding.x;
    let r = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.x = ui.spacing().interact_size.x.max(width);
            ui.add(egui::DragValue::new(&mut v).speed(speed).max_decimals(2).suffix(suffix).custom_parser(parser))
        })
        .inner;
    // where the typing or the drag begins and ends is where its step of undo opens and closes (`joint_values_step`):
    // the field writes the value while it is being typed, before it reports a change
    if r.gained_focus() || r.drag_started() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("joint_values_began"), true));
    }
    if r.lost_focus() || r.drag_stopped() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("joint_values_ended"), true));
    }
    let mut changed = r.changed();
    if changed {
        jj.drive[slot] = Some(v);
        exprs.push((slot, typed.into_inner()));
    }
    let (icon, tip) = match driven {
        Some(_) => (ph::LOCK, &qymcad_i18n::tr("j-value-set")),
        None => (ph::LOCK_OPEN, &qymcad_i18n::tr("j-value-free")),
    };
    if ui.small_button(icon).on_hover_text(tip).clicked() {
        jj.drive[slot] = driven.is_none().then_some(measured);
        changed = true;
    }
    changed
}

/// THE FORMULAS TYPED INTO THE SLOT FIELDS OF JOINT `jid` (`joint_slot_drag`) kept as the slots' expressions, the
/// same ones the `f=` fields edit: an empty text takes the formula off, and one already standing is not written again.
pub fn keep_slot_exprs(project: &mut Project, jid: Id, exprs: Vec<(usize, String)>) {
    for (slot, text) in exprs {
        let key = ["angle", "offset", "offset2"][slot.min(2)];
        if project.feat_dim(jid, key).unwrap_or("") != text {
            project.set_feat_dim(jid, key, text);
        }
    }
}

/// HOW MANY JOINTS A KIND EXPECTS: two for all of them except the screw, which makes do with one.
pub fn relation_picks_needed(kind: qymcad_core::feature::RelationKind) -> usize {
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
pub fn relation_slot_of(project: &qymcad_core::model::Project, joint: Id, want_rotation: bool) -> Option<usize> {
    let j = project.joints.iter().find(|j| j.id == joint)?;
    let kind = qymcad_core::asm::bridge::kind_of(j.kind);
    let free = j.kind.free_slots();
    (0..3).find(|&slot| free[slot] && qymcad_core::asm::joint::slot_axis(kind, slot).is_some_and(|(_, rot)| rot == want_rotation))
}

/// The minimum screen distance to a ring of radius `l` about the axis `dir` centred at `o` (for hit
/// testing and for drawing).
pub fn joint_ring_screen_dist(dc: &qymcad_ui_state::DrawCtx, o: [f64; 3], dir: [f64; 3], l: f64, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> f32 {
    let (u, v) = qymcad_ui_state::perp_basis(dir);
    let mut prev: Option<Pos2> = None;
    let mut dmin = f32::MAX;
    for k in 0..=48 {
        let a = k as f64 / 48.0 * std::f64::consts::TAU;
        let p = [o[0] + l * (u[0] * a.cos() + v[0] * a.sin()), o[1] + l * (u[1] * a.cos() + v[1] * a.sin()), o[2] + l * (u[2] * a.cos() + v[2] * a.sin())];
        let s = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis }.at(p).0;
        if let Some(pr) = prev {
            dmin = dmin.min(qymcad_ui_state::screen_dist_seg(pp, pr, s));
        }
        prev = Some(s);
    }
    dmin
}

/// THE HANDLES OF ONE ANCHOR: the attachment point, the turn, the axis pick, the offsets.
///
/// They are split out because an anchor became AN ELEMENT IN ITS OWN RIGHT: the same handles are needed
/// in the list of anchors, where there is no joint anywhere nearby. A copy of them would be a second
/// truth about how an anchor is tuned - and would part from the first at the very first edit.
pub fn one_connector_controls(joint: &mut qymcad_ui_state::JointCommand, project: &mut Project, ui: &mut egui::Ui, tag: &str, cid: Id) -> bool {
    use qymcad_core::asm::connector::AttachPoint;
    let mut changed = false;
    {
        {
            let Some(c) = project.connectors.iter().find(|c| c.id == cid) else { return false };
            // an attachment point means something only where the geometry has a length along its axis
            let axial = matches!(c.anchor, qymcad_core::feature::AnchorRef::FaceCenter(..) | qymcad_core::feature::AnchorRef::EdgeMid(..));
            let (mut point, mut rot, mut off) = (c.point, c.rot_deg, c.offset_xyz);
            ui.horizontal(|ui| {
                ui.label(format!("{tag}:"));
                if axial {
                    for p in [AttachPoint::Middle, AttachPoint::Start, AttachPoint::End] {
                        if ui.selectable_label(point == p, qymcad_i18n::tr(p.label())).on_hover_text(qymcad_i18n::tr("j-offset-hint")).clicked() {
                            point = p;
                            changed = true;
                        }
                    }
                }
                // THE TURN: a +90 deg button for the common case and a field for anything else. The
                // angle used to be stored in QUARTER TURNS, and there was nothing to set a slot at
                // 30 deg to the axis of a part with.
                if ui.button(format!("{} 90{}", ph::ARROW_CLOCKWISE, qymcad_i18n::tr("unit-deg-suffix"))).on_hover_text(qymcad_i18n::tr("j-roll-hint")).clicked() {
                    rot = (rot + 90.0).rem_euclid(360.0);
                    changed = true;
                }
                changed |= ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix(qymcad_i18n::tr("unit-deg-suffix"))).on_hover_text(qymcad_i18n::tr("j-roll-hint")).changed();
            });
            // THE SECONDARY AXIS BY A PICK (the "second pick"). It is normally derived from the
            // geometry, but a square face has no long side at all and the answer is arbitrary - then an
            // edge is pointed at by hand.
            ui.horizontal(|ui| {
                let armed = joint.axis_pick == Some(cid);
                let has = project.connector(cid).is_some_and(|c| c.axis_ref.is_some());
                if ui.selectable_label(armed, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("j-axis-pick"))).on_hover_text(qymcad_i18n::tr("j-axis-pick-hint")).clicked() {
                    joint.axis_pick = if armed { None } else { Some(cid) };
                }
                if has && ui.button(qymcad_i18n::tr("j-axis-auto")).on_hover_text(qymcad_i18n::tr("j-axis-auto-hint")).clicked() {
                    changed |= project.set_connector_axis_ref(cid, None);
                }
            });
            // AN OFFSET ALONG ALL THREE AXES OF THE CONNECTOR rather than along the main one only: an
            // anchor almost never coincides with the centre of a face exactly, and there was nothing to
            // move it sideways with - the part got moved at random instead.
            ui.horizontal(|ui| {
                ui.label(qymcad_i18n::tr("j-offset-lower"));
                for (k, name) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*name);
                    changed |= ui.add(egui::DragValue::new(&mut off[k]).speed(0.1).suffix(qymcad_i18n::tr("unit-mm-suffix"))).on_hover_text(qymcad_i18n::tr("j-slide-hint")).changed();
                }
            });
            if changed {
                project.set_connector_placement(cid, point, rot, off);
            }
        }
    }
    changed
}

/// A PART IS BEING DRIVEN BY HAND RIGHT NOW - ONE QUESTION FOR THE WHOLE FRAME.
///
/// There are two ways: a DOF gizmo handle (`joint.giz_drag`) and pulling the part itself (`part_pull`),
/// and the mouse reader is indifferent to which - all it needs is whether the hand is busy. The question
/// is brought into ONE door deliberately: while there were two, the frame asked only about the handle,
/// and PULLING A PART DID NOT WORK AT ALL - the grab fired but the drag never reached
/// `joint_giz_drag_to`. The checks did not catch it, because they called the drag directly, going around
/// the reading of the frame.
pub fn joint_drag_active(joint: &qymcad_ui_state::JointCommand, part_pull: &Option<(Id, [f64; 3], [f64; 3])>) -> bool {
    joint.giz_drag.is_some() || part_pull.is_some()
}

/// TUNING THE ANCHORS OF A JOINT: where exactly they sit and how they are turned.
///
/// This brings into the interface what is done with a mate connector elsewhere: choose the attachment
/// point (on a hole, the middle or an end face), turn the secondary axis, slide along the main one.
/// Without these handles the only way to correct a position is to move the part at random.
pub fn connector_controls(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui, jid: Id) -> bool {
    let Some((ca, cb)) = jc.project.joints.iter().find(|j| j.id == jid).map(|j| (j.a, j.b)) else { return false };
    let mut changed = false;
    // EXPANDED BY DEFAULT. Tuning the anchors IS the answer to "why did the part end up in the wrong
    // place": the attachment point, the turn, the offsets, the pick of an axis. Hidden behind a
    // collapsed header, it is not found by whoever is in trouble - and they are exactly who looks for
    // it.
    egui::CollapsingHeader::new(qymcad_i18n::tr("j-anchors")).id_salt("joint_anchors").default_open(true).show(ui, |ui| {
        for (tag, cid) in [("A", ca), ("B", cb)] {
            changed |= one_connector_controls(jc.joint, jc.project, ui, tag, cid);
        }
    });
    changed
}
