//! THE ASSEMBLY WORKBENCH: joints and mates.
//!
//! Creating a joint, picking its anchors, dragging the gizmo, animating a degree of freedom, the list of
//! relations. Every function here works over `qymcad_ui_state::JointCtx` - the records the assembly edits - and asks the
//! application for nothing.

use egui::{Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::*;

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
pub fn joint_slot_drag(ui: &mut egui::Ui, jj: &mut qymcad_core::feature::Joint, slot: usize, speed: f64) -> bool {
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
        Some(_) => (ph::LOCK, &qymcad_i18n::tr("j-value-set")),
        None => (ph::LOCK_OPEN, &qymcad_i18n::tr("j-value-free")),
    };
    if ui.small_button(icon).on_hover_text(tip).clicked() {
        jj.drive[slot] = driven.is_none().then_some(measured);
        changed = true;
    }
    changed
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
        let s = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect: rect, basis: basis }.at(p).0;
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
                if ui
                    .selectable_label(armed, format!("{} {}", ph::CROSSHAIR, qymcad_i18n::tr("j-axis-pick")))
                    .on_hover_text(qymcad_i18n::tr("j-axis-pick-hint"))
                    .clicked()
                {
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
                    changed |= ui
                        .add(egui::DragValue::new(&mut off[k]).speed(0.1).suffix(qymcad_i18n::tr("unit-mm-suffix")))
                        .on_hover_text(qymcad_i18n::tr("j-slide-hint"))
                        .changed();
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

/// WHETHER THE LIVE GEOMETRY an anchor derives its axes from is ready.
///
/// The origin of a part and a base plane are given by the component itself - they need no kernel. Faces,
/// edges and vertices live in the B-rep: while it is absent, there is nowhere to take the anchor's axes
/// from.
pub fn geometry_ready_for_anchor(live: &qymcad_ui_state::LiveGeom, project: &Project, anchor: &qymcad_core::feature::AnchorRef) -> bool {
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
        AnchorRef::FaceCenter(b, _) => project.regen_faces.get(b).is_some_and(|f| !f.is_empty()) || live.shapes.contains_key(b),
        // AN EDGE AND A VERTEX DO NEED THE MODEL EDGES: the anchor direction of an edge is read from
        // them, and without them the joint would take the WORLD axes. Here there really is something to
        // wait for.
        AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => project.regen_edges.get(b).is_some_and(|e| !e.is_empty()) || live.shapes.contains_key(b),
    }
}

/// A click on a face while the width tool is active: add an anchor to the set.
///
/// THE WALLS MUST FACE THE SAME WAY. "Midway" means something only along a COMMON normal; walls that
/// look in different directions have no midpoint, and taking such an anchor silently would promise
/// something that does not exist.
pub fn width_pick_click(active_path: &[Id], joint: &mut qymcad_ui_state::JointCommand, project: &mut Project, status: &mut String, body: Id, key: qymcad_core::feature::FaceKey) {
    use qymcad_core::feature::AnchorRef;
    if joint.width_pick.is_none() {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let Some(owner) = project.body_owner(body) else {
        *status = qymcad_i18n::tr("j-face-no-part");
        return;
    };
    let comp = project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let n = key.normal;
    let Some(sel) = joint.width_pick.as_mut() else { return };
    if sel.len() == 1 {
        if let Some((_, AnchorRef::FaceCenter(_, k0))) = sel.first() {
            let dot = k0.normal[0] * n[0] + k0.normal[1] * n[1] + k0.normal[2] * n[2];
            if dot.abs() < 0.999 {
                *status = qymcad_i18n::tr("j-width-walls-differ");
                return;
            }
        }
    }
    if sel.len() >= 3 {
        sel.clear(); // one too many - start the set over rather than piling up silently
    }
    sel.push((comp, AnchorRef::FaceCenter(body, key)));
    let n = sel.len();
    *status = qymcad_i18n::tr1("j-width-picked", "n", &n.to_string());
}

/// A click on a body while the group tool is active: add its part to the set or take it out.
pub fn group_pick_click(active_path: &[Id], joint: &mut qymcad_ui_state::JointCommand, project: &mut Project, status: &mut String, body: Id) {
    if joint.group_pick.is_none() {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let Some(owner) = project.body_owner(body) else {
        *status = qymcad_i18n::tr("j-body-no-part");
        return;
    };
    // A joint runs between components AT THE LEVEL OF THE CONTEXT, and so does a group: what gets
    // fastened is what reads as a part of the assembly, not a leaf body inside a subassembly.
    let comp = project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let Some(sel) = joint.group_pick.as_mut() else { return };
    match sel.iter().position(|&c| c == comp) {
        Some(i) => {
            sel.remove(i);
        }
        None => sel.push(comp),
    }
    let n = sel.len();
    *status = qymcad_i18n::tr1("j-group-picked", "n", &n.to_string());
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_ground_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.ground_pick = on;
    jc.joint.pick_first = None;
    *jc.status = if on { qymcad_i18n::tr("tb-ground-pick") } else { qymcad_i18n::tr("tb-ground-off") };
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_group_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.group_pick = on.then(Vec::new);
    *jc.status = qymcad_i18n::tr(if on { "j-group-pick" } else { "j-group-off" });
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_tangent_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.tangent_pick = on.then(Vec::new);
    *jc.status = qymcad_i18n::tr(if on { "j-tangent-pick" } else { "j-tangent-off" });
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_width_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.width_pick = on.then(Vec::new);
    *jc.status = qymcad_i18n::tr(if on { "j-width-pick" } else { "j-width-off" });
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_conn_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.conn_pick = on;
    *jc.status = qymcad_i18n::tr(if on { "j-conn-pick" } else { "j-conn-off" });
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_relation_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.relation_pick = on.then(qymcad_ui_state::RelationPick::default);
    *jc.status = qymcad_i18n::tr(if on { "j-relation-pick" } else { "j-relation-off" });
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

/// The joint under the cursor in 3D (by proximity to its glyph) - for picking and for hover.
pub fn joint_glyph_at(jc: &mut qymcad_ui_state::JointCtx, rect: Rect, pos: Pos2) -> Option<Id> {
    joint_glyphs(jc, rect).into_iter().find(|(_, at, _)| at.distance(pos) <= 11.0).map(|(id, _, _)| id)
}

/// The screen positions of joint glyphs: THE MIDPOINT between A and B. One source for both drawing and
/// hit testing.
pub fn joint_glyphs(jc: &mut qymcad_ui_state::JointCtx, rect: Rect) -> Vec<(Id, Pos2, qymcad_core::feature::JointKind)> {
    let basis = jc.cam.basis();
    jc.project
        .joints
        .iter()
        .filter(|j| qymcad_ui_state::joint_visible(jc.active_path, jc.project, jc.set, jc.workbench, j))
        .filter_map(|j| {
            let (a, b) = qymcad_ui_state::joint_endpoints(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, j, rect, &basis)?;
            Some((j.id, Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5), j.kind))
        })
        .collect()
}

/// Release the part - the same thing `drag_stopped` does in a frame.
pub fn joint_giz_end_for_test(jc: &mut qymcad_ui_state::JointCtx) {
    joint_giz_end(jc);
}

/// RELEASE THE PART - the end of the lasting drag operation.
///
/// One door for the frame and for the checks: the undo step is closed exactly here, and only here.
pub fn joint_giz_end(jc: &mut qymcad_ui_state::JointCtx) {
    if jc.part_pull.take().is_some() {
        jc.project.drag_pull = None;
        jc.project.solve_joints();
        qymcad_ui_state::invalidate_placement(jc.regen);
        qymcad_ui_state::commit_edit(&mut jc.rebuild());
        qymcad_ui_state::after_placement_change(&mut jc.rebuild());
        return;
    }
    let was_dragging = jc.joint.giz_drag.is_some();
    jc.joint.giz_drag = None;
    jc.joint.giz_handle = None;
    if was_dragging {
        qymcad_ui_state::commit_edit(&mut jc.rebuild()); // ONE undo step for the whole drag rather than one per frame
    }
    qymcad_ui_state::after_placement_change(&mut jc.rebuild());
}

/// Start dragging a DOF gizmo handle: pin the frame (o and dir) and the starting value of the parameter.
pub fn joint_giz_begin(jc: &mut qymcad_ui_state::JointCtx, jid: Id, slot: u8, ring: bool) {
    let Some(qymcad_ui_state::JointGizmo { origin: o, handles: hs }) = qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid) else { return };
    let Some(dir) = hs.iter().find(|h| h.slot == slot && h.ring == ring).map(|h| h.dir) else { return };
    // the drag starts from THE DRIVER if there is one, otherwise from the reading: the pull must start
    // from what was asked for, not from how it ended up if the request could not be met
    let start = jc.project.joints.iter().find(|x| x.id == jid).map(|j| j.drive[(slot as usize).min(2)].unwrap_or([j.angle, j.offset, j.offset2][slot as usize])).unwrap_or(0.0);
    // THE BOUNDARY OF THE OPERATION OPENS ONCE - FOR THE WHOLE DRAG, NOT FOR EVERY FRAME.
    //
    // `qymcad_ui_state::begin_edit` takes A FULL COPY of the document, and until now every frame of the drag took one
    // (opened and closed inside `apply_joint_giz`). On a real assembly - 138 bodies with meshes - one
    // frame of dragging cost 13-18 ms for THAT ALONE, before any drawing. It reads as the part following
    // reluctantly: the program cannot keep up with the mouse, and the joints catch up late.
    //
    // A drag is ONE operation lasting across frames; that is exactly what `qymcad_ui_state::begin_edit` is for. The copy
    // is taken at the grab, and the undo step is closed on release (`joint_giz_end`).
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("status-edit-joint"));
    jc.joint.giz_drag = Some(qymcad_ui_state::JointGizDrag { jid, slot, ring, start, amt: 0.0, o, dir });
}

/// The DOF gizmo handle under the cursor: (slot, whether it is a ring). Arrows take priority over rings,
/// as in the six-degree gizmo.
pub fn joint_handle_hit(jc: &mut qymcad_ui_state::JointCtx, jid: Id, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), pp: Pos2) -> Option<(u8, bool)> {
    let qymcad_ui_state::JointGizmo { origin: o, handles: hs } = qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid)?;
    let l = 60.0 / jc.cam.scale as f64;
    // the translation arrows first
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs.iter().filter(|h| !h.ring) {
        let _ = ring;
        let scr = qymcad_ui_state::Screen { cam: jc.cam, set: jc.set, rect: rect, basis: basis };
        let s0 = scr.at(o).0;
        let s1 = scr.at([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l]).0;
        if qymcad_ui_state::screen_dist_seg(pp, s0, s1) <= 13.0 {
            return Some((slot, false));
        }
    }
    // then the rotation rings
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs.iter().filter(|h| h.ring) {
        let _ = ring;
        if joint_ring_screen_dist(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, o, dir, l, rect, basis, pp) <= 10.0 {
            return Some((slot, true));
        }
    }
    None
}

/// TAKE A PICK FOR THE SECONDARY AXIS: a click on an edge or a face sets the axis of the connector.
///
/// The direction is taken from the picked geometry and laid perpendicular to the main axis of the
/// anchor - the pick sets THE SIDE, it does not replace the anchor. Geometry that has no side (a vertex,
/// the origin of a part) never gets here: it does not enter the pick.
pub fn joint_axis_pick_apply(jc: &mut qymcad_ui_state::JointCtx, anchor: qymcad_core::feature::AnchorRef) {
    let Some(cid) = jc.joint.axis_pick else { return };
    if jc.project.anchor_direction(&anchor).is_none() {
        *jc.status = qymcad_i18n::tr("j-axis-no-direction");
        return;
    }
    jc.project.set_connector_axis_ref(cid, Some(anchor));
    jc.joint.axis_pick = None;
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    *jc.status = qymcad_i18n::tr("j-axis-set");
}

/// Apply a newly picked anchor while editing (swap the face, edge or vertex of A or B on the fly). The
/// checks are the same as at creation: the part is in the active context, and the anchors are not in one
/// subassembly. The connector is updated through the core's `set_connector_anchor`, then a rebuild.
pub fn joint_edit_repick_apply(jc: &mut qymcad_ui_state::JointCtx, body: Id, anchor: qymcad_core::feature::AnchorRef) {
    let Some((jid, is_b)) = jc.joint.edit_repick else { return };
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let Some(body_owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-anchor-no-part");
        return;
    };
    if !jc.project.component_is_within(body_owner, ctx) {
        *jc.status = qymcad_i18n::tr("j-outside-assembly");
        return;
    }
    let place = jc.project.ancestor_child_of(ctx, body_owner).unwrap_or(body_owner);
    let Some(j) = jc.project.joints.iter().find(|x| x.id == jid) else { return };
    let (cid, other) = if is_b { (j.b, j.a) } else { (j.a, j.b) };
    if jc.project.connector(other).map(|c| c.owner) == Some(place) {
        *jc.status = qymcad_i18n::tr("j-same-part");
        return;
    }
    jc.project.set_connector_anchor(cid, place, anchor);
    jc.joint.edit_repick = None;
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    *jc.status = qymcad_i18n::tr1("jt-anchor-replaced", "side", if is_b { "B" } else { "A" });
}

/// A thin wrapper: a click on AN EDGE -> an `EdgeMid` axis anchor.
pub fn joint_pick_edge_click(jc: &mut qymcad_ui_state::JointCtx, body: Id, edge_id: u32) {
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-edge-no-part");
        return;
    };
    joint_pick_anchor_click(jc, owner, qymcad_core::feature::AnchorRef::EdgeMid(body, edge_id));
}

/// A thin wrapper: a click on A FACE -> a `FaceCenter` anchor.
pub fn joint_pick_face_click(jc: &mut qymcad_ui_state::JointCtx, body: Id, key: qymcad_core::feature::FaceKey) {
    // while "by origins" is on, a face is only a way to point at the part.
    if jc.joint.anchor_mode == 3 {
        joint_pick_origin_click(jc, body);
        return;
    }
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-face-no-part");
        return;
    };
    joint_pick_anchor_click(jc, owner, qymcad_core::feature::AnchorRef::FaceCenter(body, key));
}

/// AN ANCHOR INFERRED UNDER THE CURSOR, by the body it was found on.
///
/// One door for every sort of anchor: the sort is no longer declared in advance by a switch, and reading
/// a click stopped depending on what was chosen in the tool bar a minute earlier.
pub fn joint_pick_anchor_at(jc: &mut qymcad_ui_state::JointCtx, body: Id, anchor: qymcad_core::feature::AnchorRef) {
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-anchor-no-part");
        return;
    };
    joint_pick_anchor_click(jc, owner, anchor);
}

/// A click on a body while "by origins" is on -> the `Origin` anchor of its PART.
///
/// The body here is only a way to point at the part: the anchor becomes the origin of the component
/// rather than anything on its surface. So any click will do - a face, an edge, wherever it landed.
pub fn joint_pick_origin_click(jc: &mut qymcad_ui_state::JointCtx, body: Id) {
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-face-no-part");
        return;
    };
    joint_pick_anchor_click(jc, owner, qymcad_core::feature::AnchorRef::Origin);
}

pub fn joint_pick_active_for_test(jc: &mut qymcad_ui_state::JointCtx) -> bool {
    jc.joint.pick_faces
}

pub fn joint_pick_first_anchor_for_test(jc: &mut qymcad_ui_state::JointCtx) -> Option<qymcad_core::feature::AnchorRef> {
    jc.joint.pick_first.as_ref().map(|(_, a)| a.clone())
}

pub fn joint_pick_edge_click_for_test(jc: &mut qymcad_ui_state::JointCtx, body: Id, edge: u32) {
    joint_pick_edge_click(jc, body, edge);
}

pub fn joint_pick_face_click_for_test(jc: &mut qymcad_ui_state::JointCtx, body: Id, key: qymcad_core::feature::FaceKey) {
    joint_pick_face_click(jc, body, key);
}

pub fn joint_pick_origin_click_for_test(jc: &mut qymcad_ui_state::JointCtx, body: Id) {
    joint_pick_origin_click(jc, body);
}

pub fn set_joint_anchor_mode_for_test(jc: &mut qymcad_ui_state::JointCtx, mode: u8) {
    jc.joint.anchor_mode = mode;
}

/// Confirm the picks and create the relation.
pub fn relation_pick_confirm(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(pick) = jc.joint.relation_pick.clone() else { return };
    let need = relation_picks_needed(pick.kind);
    let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
    if have < need || pick.picks.len() < 2 {
        *jc.status = qymcad_i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string());
        return;
    }
    let ((ja, sa), (jb, sb)) = (pick.picks[0], pick.picks[1]);
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-relation-made")); // THE BOUNDARY OF AN OPERATION
    let id = jc.project.add_relation(pick.kind, ja, sa, jb, sb, pick.value);
    if pick.reversed {
        if let Some(r) = jc.project.relations.iter_mut().find(|r| r.id == id) {
            r.reversed = true;
            // the phase was taken BEFORE the reversal - take it again, or the relation jerks the part
            let copy = r.clone();
            let ph = jc.project.relation_phase(&copy).unwrap_or(0.0);
            if let Some(r) = jc.project.relations.iter_mut().find(|r| r.id == id) {
                r.phase = ph;
            }
        }
    }
    jc.joint.relation_pick = None;
    *jc.status = qymcad_i18n::tr("j-relation-made-ok");
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// A click on A JOINT while the relation tool is active: take its degree of the required sort.
pub fn relation_pick_click(jc: &mut qymcad_ui_state::JointCtx, joint: Id) {
    let Some(pick) = jc.joint.relation_pick.clone() else { return };
    let (rot_a, rot_b) = pick.kind.slots_are_rotations();
    let need = relation_picks_needed(pick.kind);
    // THE SCREW TAKES BOTH DEGREES OF ONE JOINT - the angle and the travel of a cylindrical one.
    let wanted: Vec<bool> = if need == 1 { vec![rot_a, rot_b] } else { vec![if pick.picks.is_empty() { rot_a } else { rot_b }] };
    let mut taken: Vec<(Id, usize)> = Vec::new();
    for &want in &wanted {
        let Some(slot) = relation_slot_of(jc.project, joint, want) else {
            // NO SILENT REFUSAL: the click has already happened and an answer is expected.
            *jc.status = qymcad_i18n::tr(if want { "j-relation-need-turn" } else { "j-relation-need-travel" });
            return;
        };
        taken.push((joint, slot));
    }
    if need == 2 && pick.picks.iter().any(|(id, _)| *id == joint) {
        *jc.status = qymcad_i18n::tr("j-relation-need-two");
        return;
    }
    let Some(pick) = jc.joint.relation_pick.as_mut() else { return };
    pick.picks.extend(taken);
    let got = if need == 1 { 1 } else { pick.picks.len() };
    *jc.status = qymcad_i18n::tr2("j-relation-picked", "n", &got.to_string(), "need", &need.to_string());
}

/// DELETE AN ANCHOR - or say why that is not possible.
pub fn delete_connector_asked(jc: &mut qymcad_ui_state::JointCtx, cid: Id) {
    let users = jc.project.connector_users(cid).len();
    if users > 0 {
        // NO SILENT REFUSAL: the cross was pressed, and what held the anchor back must be said.
        *jc.status = qymcad_i18n::tr1("j-conn-in-use", "n", &users.to_string());
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-conn-deleted"));
    jc.project.delete_connector(cid);
    if *jc.sel_conn == Some(cid) {
        *jc.sel_conn = None;
    }
    *jc.status = qymcad_i18n::tr("j-conn-deleted");
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// Confirm the width set: two walls and the piece between them.
pub fn width_pick_confirm(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(sel) = jc.joint.width_pick.clone() else { return };
    if sel.len() < 3 {
        // NO SILENT REFUSAL: some of the anchors have already been shown and a result is expected.
        *jc.status = qymcad_i18n::tr("j-width-need-three");
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-width-made")); // THE BOUNDARY OF AN OPERATION
    let ids: Vec<Id> = sel.iter().map(|(owner, a)| jc.project.add_connector(*owner, a.clone())).collect();
    jc.project.add_width(&[ids[0], ids[1]], ids[2]);
    jc.joint.width_pick = None;
    *jc.status = qymcad_i18n::tr("j-width-made-ok");
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// A click on a face while the tangency tool is active.
///
/// THE PAIR MUST BE A CYLINDER AND A PLANE. Tangency holds a distance equal to the radius; a pair of
/// planes has no radius, and "tangency" between them means merely "coincident" - and there is a planar
/// joint for that. Accepting such a pair silently would set a condition that does nothing.
pub fn tangent_pick_click(jc: &mut qymcad_ui_state::JointCtx, body: Id, key: qymcad_core::feature::FaceKey) {
    use qymcad_core::feature::AnchorRef;
    if jc.joint.tangent_pick.is_none() {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-face-no-part");
        return;
    };
    let comp = jc.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let is_cyl = jc.project.face_cylinder(body, &key).is_some();
    let Some(sel) = jc.joint.tangent_pick.as_mut() else { return };
    if let Some((_, AnchorRef::FaceCenter(b0, k0))) = sel.first().cloned() {
        // the second surface must complement the first: a cylinder to a plane and the other way round
        if jc.project.face_cylinder(b0, &k0).is_some() == is_cyl {
            *jc.status = qymcad_i18n::tr("j-tangent-need-cylinder");
            return;
        }
    }
    let Some(sel) = jc.joint.tangent_pick.as_mut() else { return };
    sel.push((comp, AnchorRef::FaceCenter(body, key)));
    if sel.len() < 2 {
        *jc.status = qymcad_i18n::tr("j-tangent-second");
        return;
    }
    let pair = sel.clone();
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-tangent-made")); // THE BOUNDARY OF AN OPERATION
    jc.project.add_tangent(pair[0].0, pair[0].1.clone(), pair[1].0, pair[1].1.clone());
    jc.joint.tangent_pick = None;
    *jc.status = qymcad_i18n::tr("j-tangent-made-ok");
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// Confirm the set and make a group of it. Fewer than two parts leaves nothing to fasten.
pub fn group_pick_confirm(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(sel) = jc.joint.group_pick.clone() else { return };
    if sel.len() < 2 {
        // NO SILENT REFUSAL: a part has already been clicked and a result is expected.
        *jc.status = qymcad_i18n::tr("j-group-need-two");
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-group-made")); // THE BOUNDARY OF AN OPERATION
    jc.project.add_group(&sel);
    jc.joint.group_pick = None;
    *jc.status = qymcad_i18n::tr1("j-group-made-n", "n", &sel.len().to_string());
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// The grounding tool: a click on a body fixes or releases its part (at the level of the context).
/// As when creating a joint, what gets grounded is the DIRECT child of the context (the subassembly)
/// rather than a leaf body.
pub fn joint_pick_ground_click(jc: &mut qymcad_ui_state::JointCtx, body: Id) {
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = qymcad_i18n::tr("j-body-no-part");
        return;
    };
    let comp = jc.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let g = jc.project.is_grounded(comp);
    jc.project.set_grounded(comp, !g);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    let name = jc.project.components.iter().find(|c| c.id == comp).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
    *jc.status = if !g { qymcad_i18n::tr1("jt-grounded", "name", &name) } else { qymcad_i18n::tr1("jt-released", "name", &name) };
}

pub fn joint_pick_anchor_click(jc: &mut qymcad_ui_state::JointCtx, owner: Id, anchor: qymcad_core::feature::AnchorRef) {
    // the edges of a body go into the model BEFORE an anchor refers to them (see `qymcad_ui_state::ensure_model_edges`)
    if let qymcad_core::feature::AnchorRef::EdgeMid(b, _) | qymcad_core::feature::AnchorRef::Vertex(b, _, _) = &anchor {
        let b = *b;
        qymcad_ui_state::ensure_model_edges(&mut jc.rebuild(), b);
    }
    // A STANDALONE ANCHOR: the same geometry, the same reading of the click - but no joint is created.
    // The connector is made on its own and waits for a joint to be put on it later.
    if jc.joint.conn_pick {
        if !geometry_ready_for_anchor(jc.live, jc.project, &anchor) {
            return; // the readiness check has already said what is wrong
        }
        qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-conn-made")); // THE BOUNDARY OF AN OPERATION
        let cid = jc.project.add_connector_standalone(owner, anchor);
        jc.joint.conn_pick = false;
        *jc.sel_conn = Some(cid);
        *jc.status = qymcad_i18n::tr("j-conn-made-ok");
        qymcad_ui_state::commit_edit(&mut jc.rebuild());
        return;
    }
    // a joint only ever goes between parts of the ACTIVE context. Ghost parts of other subassemblies
    // are visible so they can be referred to, but they are dimmed for a reason - no joint lands on them.
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    if !jc.project.component_is_within(owner, ctx) {
        *jc.status = qymcad_i18n::tr("j-outside-assembly-joint");
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
    if jc.project.anchor_sits_on_moving_part(owner, &anchor) {
        *jc.status = qymcad_i18n::tr("j-anchor-on-moving-part-refused");
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
    if !geometry_ready_for_anchor(jc.live, jc.project, &anchor) {
        // TWO DIFFERENT CASES, TWO DIFFERENT ANSWERS. While the preparation runs, "one moment" is
        // the truth. But if the preparation IS ALREADY OVER and there is still no live body (an import
        // that did not restore, an operation that did not build), then "one moment" is a lie: there is
        // nothing to wait for, and the face will be clicked until it is given up on.
        *jc.status = qymcad_i18n::tr(if jc.live.ready { "j-geometry-missing" } else { "j-geometry-on-its-way" });
        return;
    }
    match jc.joint.pick_first.take() {
        None => {
            jc.joint.pick_first = Some((owner, anchor));
            *jc.status = qymcad_i18n::tr("j-anchor-a-picked");
        }
        Some((owner_a, anchor_a)) => {
            if owner_a == owner {
                jc.joint.pick_first = Some((owner_a, anchor_a)); // the same part - wait for another one
                *jc.status = qymcad_i18n::tr("j-pick-other-part");
                return;
            }
            // a joint runs between components AT THE LEVEL OF THE CONTEXT (the direct children of ctx,
            // that is, the subassemblies) rather than between leaf bodies. The anchor stays on the leaf
            // body; the owner of the connector is the subassembly, and `place_tree` moves that whole
            // subassembly as one.
            let place_a = jc.project.ancestor_child_of(ctx, owner_a).unwrap_or(owner_a);
            let place_b = jc.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
            if place_a == place_b {
                jc.joint.pick_first = Some((owner_a, anchor_a)); // both anchors in one subassembly - an internal joint
                *jc.status = qymcad_i18n::tr("j-same-subassembly");
                return;
            }
            // The "anchor compatibility" check is gone from here: it always answered "compatible", so
            // the refusal existed only in the text. An anchor is a full coordinate frame, and any kind
            // of joint works with any pair; which degrees of freedom are left is stated by the kind.
            // the side of the contact is decided by the solver (coplanar, minimal motion) plus the
            // `joint.flip` toggle in the panel - forcing a flip on the connector is no longer needed
            // (it caused a spurious 180 deg turn).
            let ca = jc.project.add_connector(place_a, anchor_a);
            let cb = jc.project.add_connector(place_b, anchor);
            let jid = jc.project.add_joint(ca, cb, jc.joint.new_kind);
            // The values from the tool bar are A DRIVER, not a reading. Untouched fields (zero) do not
            // count as a driver: a joint is born with free degrees, and they can be pinned with the
            // lock in the popup at the glyph.
            jc.project.set_joint_drive(jid, 1, (jc.joint.new_offset != 0.0).then_some(jc.joint.new_offset));
            jc.project.set_joint_drive(jid, 0, (jc.joint.new_angle != 0.0).then_some(jc.joint.new_angle));
            // "AS IT STANDS" IS TAKEN BEFORE THE FIRST SOLVE, while the parts are still where they
            // were placed. After a solve there is nothing left to take: the joint mates the anchors and
            // drags the part away.
            if jc.joint.new_as_built && !jc.project.set_joint_as_built(jid) {
                *jc.status = qymcad_i18n::tr("j-as-built-failed");
            }
            let children = jc.project.component_children(ctx);
            if !children.iter().any(|&c| jc.project.is_grounded(c)) {
                jc.project.set_grounded(place_a, true); // the first anchor grounds the context-level subassembly
            }
            jc.joint.pick_faces = false;
            // THE CONSOLE OPENS BY ITSELF: after the second pick the handles for flipping the axis,
            // swapping the roles, changing the kind and changing the anchor are right there - the very
            // ones needed at once, because the side and the order come out right the first time far
            // from always. The joint used to appear silently in the list, and mending it required first
            // guessing to look for it there.
            jc.joint.edit = Some(jid);
            qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
            *jc.status = qymcad_i18n::tr("j-created");
        }
    }
}

/// Which joint the console currently holds.
pub fn joint_edit_for_test(jc: &mut qymcad_ui_state::JointCtx) -> Option<Id> {
    jc.joint.edit
}

pub fn joint_hud_flip_axis_for_test(jc: &mut qymcad_ui_state::JointCtx, jid: Id) {
    joint_hud_flip_axis(jc, jid);
}

/// THE TEST FACADES OF THE CONSOLE: the same door the buttons of the bar go through.
pub fn joint_hud_swap_roles_for_test(jc: &mut qymcad_ui_state::JointCtx, jid: Id) {
    joint_hud_swap_roles(jc, jid);
}

/// THE TEST FACADE FOR PICKING AN ANCHOR: the same door a click on the frame goes through.
pub fn joint_pick_anchor_click_for_test(jc: &mut qymcad_ui_state::JointCtx, owner: Id, anchor: qymcad_core::feature::AnchorRef) {
    jc.joint.pick_faces = true;
    joint_pick_anchor_click(jc, owner, anchor);
}

/// THE CONSOLE HANDLE "SWAP THE ROLES": which part stands still and which one moves.
pub fn joint_hud_swap_roles(jc: &mut qymcad_ui_state::JointCtx, jid: Id) {
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("jt-swap-roles")); // THE BOUNDARY OF AN OPERATION
    if jc.project.swap_joint_roles(jid) {
        qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    }
}

/// THE CONSOLE HANDLE "FLIP THE AXIS".
///
/// The main axis is flipped at the FIRST anchor: it sets the direction the second one is brought to,
/// and flipping the second would be the same thing inside out. What that changes in the document is
/// THE CORE's business (`flip_joint_side`): the interface only presses the handle.
pub fn joint_hud_flip_axis(jc: &mut qymcad_ui_state::JointCtx, jid: Id) {
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("jt-flip-axis")); // THE BOUNDARY OF AN OPERATION
    jc.project.flip_joint_side(jid);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
}

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
    let desc_a = jc.project.connector(j.a).map(|c| qymcad_ui_state::anchor_desc(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, &c.anchor)).unwrap_or_default();
    let desc_b = jc.project.connector(j.b).map(|c| qymcad_ui_state::anchor_desc(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, &c.anchor)).unwrap_or_default();
    let repick = jc.joint.edit_repick;
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
                ui.label(egui::RichText::new(format!("{} {}", ph::WARNING, qymcad_i18n::tr(why))).color(jc.scheme.pal.error_mild()))
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
                if ui.button(qymcad_i18n::tr("j-done-esc")).clicked() {
                    done = true;
                }
            });
        });
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
}

pub fn joint_tool_bar(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui) {
    // The panel lives inside a `Ui` now; the context is still wanted for windows,
    // input and viewport commands, and it comes from the same place.
    let ctx = &ui.ctx().clone();
    use qymcad_core::feature::JointKind;
    // the tangency tool bar - a hint and a way out. There is nothing to confirm: the condition is set
    // on the second pick, and tangency has no connectors.
    if let Some(sel) = jc.joint.tangent_pick.clone() {
        let mut cancel = false;
        egui::Panel::top("tangent_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::CIRCLE_HALF_TILT, qymcad_i18n::tr("j-tangent-made"))).strong());
                ui.separator();
                let hint = if sel.is_empty() { qymcad_i18n::tr("j-tangent-pick") } else { qymcad_i18n::tr("j-tangent-second") };
                ui.label(egui::RichText::new(hint).color(jc.scheme.pal.hint()));
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
            jc.joint.tangent_pick = None;
            *jc.status = qymcad_i18n::tr("j-tangent-off");
        }
        return;
    }
    // the width tool bar - how many anchors are shown, a "make it" button and a way out.
    if let Some(sel) = jc.joint.width_pick.clone() {
        let (mut make, mut cancel) = (false, false);
        egui::Panel::top("width_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::ARROWS_OUT_LINE_HORIZONTAL, qymcad_i18n::tr("j-width-made"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr1("j-width-picked", "n", &sel.len().to_string())).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                    if ui.button(format!("{} {}", ph::CHECK, qymcad_i18n::tr("j-width-make-btn"))).clicked() {
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
            *jc.status = qymcad_i18n::tr("j-width-off");
        }
        return;
    }
    // the group tool bar - how many are picked, a "group them" button and a way out. The same shape
    // as every other tool: the hint on top, Enter confirms, Esc cancels.
    if let Some(sel) = jc.joint.group_pick.clone() {
        let (mut make, mut cancel) = (false, false);
        egui::Panel::top("group_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::SELECTION_ALL, qymcad_i18n::tr("j-group-made"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr1("j-group-picked", "n", &sel.len().to_string())).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-cancel-esc")).clicked() {
                        cancel = true;
                    }
                    if ui.button(format!("{} {}", ph::CHECK, qymcad_i18n::tr("j-group-make-btn"))).clicked() {
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
            *jc.status = qymcad_i18n::tr("j-group-off");
        }
        return;
    }
    // the grounding tool bar - a hint and a way out
    if jc.joint.ground_pick {
        let mut cancel = false;
        egui::Panel::top("ground_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::ANCHOR, qymcad_i18n::tr("jt-ground-btn"))).strong());
                ui.separator();
                ui.label(egui::RichText::new(qymcad_i18n::tr("j-ground-click")).color(jc.scheme.pal.hint()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(qymcad_i18n::tr("j-done-esc")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if cancel {
            jc.joint.ground_pick = false;
            *jc.status = qymcad_i18n::tr("j-ground-off");
        }
        return;
    }
    // THE ANCHOR TOOL BAR: the kind of anchor and a hint about where to click.
    if jc.joint.conn_pick {
        let mut cancel = false;
        egui::Panel::top("conn_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
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
            *jc.status = qymcad_i18n::tr("j-conn-off");
        }
        return;
    }
    // THE RELATION TOOL BAR: the kind, the number, the direction, Enter/Esc.
    if jc.joint.relation_pick.is_some() {
        let (mut cancel, mut done) = (false, false);
        let pick = jc.joint.relation_pick.clone().unwrap_or_default();
        let need = relation_picks_needed(pick.kind);
        let have = if need == 1 { pick.picks.len() / 2 } else { pick.picks.len() };
        egui::Panel::top("relation_tool_bar").frame(qymcad_ui_state::tool_bar_frame(jc.scheme))
.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} {}", ph::GEAR_SIX, qymcad_i18n::tr("j-relation-btn"))).strong());
                ui.separator();
                ui.label(qymcad_i18n::tr("j-kind"));
                let mut k = pick.kind;
                egui::ComboBox::from_id_salt("relation_bar_kind").selected_text(qymcad_i18n::tr(k.label())).show_ui(ui, |ui| {
                    for kk in [qymcad_core::feature::RelationKind::Gear, qymcad_core::feature::RelationKind::RackPinion, qymcad_core::feature::RelationKind::Screw, qymcad_core::feature::RelationKind::Linear] {
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
                    if ui.add_enabled(have >= need, egui::Button::new(qymcad_i18n::tr("j-done-enter"))).clicked() {
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
            *jc.status = qymcad_i18n::tr("j-relation-off");
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
        *jc.status = qymcad_i18n::tr("j-cancelled");
    }
}

/// The assembly tool bar in one call - a door for checks that look AT A FRAME.
pub fn joint_tool_bar_for_test(jc: &mut qymcad_ui_state::JointCtx, ui: &mut egui::Ui) {
    joint_tool_bar(jc, ui);
}

/// STOPPING THE SWEEP of a joint's degree of freedom: the reading goes back to what it was before, and the
/// document is marked. It lived in `gui.rs` for no better reason than that the animation field does.
pub fn stop_joint_anim(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(a) = jc.joint_anim.take() else { return };
    jc.project.set_joint_drive(a.joint, a.slot, a.saved);
    jc.project.solve_joints();
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the scheduler does the computing
}

/// APPLY THE GIZMO DRAG: the joint parameter becomes start plus accumulated -> `solve_joints` (cheap, with
/// no rebuild of bodies) -> invalidate. It sat among the Part commands, which is where the gizmo state
/// lives, but what it does is move a joint.
pub fn apply_joint_giz(jc: &mut qymcad_ui_state::JointCtx) {
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("status-edit-joint")); // THE BOUNDARY OF AN OPERATION
    let Some(dg) = jc.joint.giz_drag else { return };
    let snap = jc.comp_giz.snap;
    let Some((val, _)) = qymcad_ui_state::joint_giz_value(jc.joint, jc.set, snap) else { return };
    // DRAGGING A GIZMO IS A DELIBERATE ACT, so it is A DRIVER. It used to write into the reading field, and
    // the part would then "by itself" return to where the previous solve had left it. The limit is held by
    // the model rather than here - see `Project::set_joint_drive`.
    jc.project.set_joint_drive(dg.jid, (dg.slot as usize).min(2), Some(val));
    jc.project.solve_joints();
    qymcad_ui_state::invalidate_placement(jc.regen); // the joint placed the parts: THE PLACEMENT changed, not the shape
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// GRAB A PART AND PULL IT: everything after the pick. What is under the cursor is decided by the
/// application - picking reads the whole scene - and the assembly is handed the body it caught.
pub fn joint_grab_part(jc: &mut qymcad_ui_state::JointCtx, body: Id, rect: Rect, towards: egui::Vec2, basis: &([f64; 3], [f64; 3], [f64; 3])) -> bool {
    let Some(owner) = jc.project.body_owner(body) else { return false };
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
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
    if let Some(comp) = jc.project.pull_target_component(owner, ctx) {
        let p0 = qymcad_core::feature::apply12(&jc.project.relative_transform(comp, ctx), [0.0, 0.0, 0.0]);
        qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("status-edit-joint"));
        *jc.part_pull = Some((comp, [0.0, 0.0, 0.0], p0));
        return true;
    }
    let Some(jid) = jc.project.drive_joint_in_context(owner, ctx) else { return false };
    let Some(qymcad_ui_state::JointGizmo { origin: o, handles: hs }) = qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid) else { return false };
    if hs.is_empty() {
        return false; // no freedoms left - there is nothing to drive
    }
    // THE AXIS OF A DEGREE ON SCREEN, compared with the direction of the pull.
    let scr = qymcad_ui_state::Screen { cam: jc.cam, set: jc.set, rect: rect, basis: basis };
    let centre = scr.at(o).0;
    let len = 60.0 / jc.cam.scale as f64;
    let mut best: Option<(f64, u8, bool)> = None;
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in &hs {
        let tip = [o[0] + dir[0] * len, o[1] + dir[1] * len, o[2] + dir[2] * len];
        let s = scr.at(tip).0 - centre;
        let n = (s.x * s.x + s.y * s.y).sqrt();
        if n < 1e-3 {
            continue; // the degree points into the screen: no motion tells it apart
        }
        // a rotation moves ACROSS its axis, a shift moves along it; the comparison is by magnitude and
        // the drag itself sorts out the sign
        let along = ((towards.x * s.x + towards.y * s.y) / n).abs() as f64;
        if best.is_none_or(|(b, _, _)| along > b) {
            best = Some((along, slot, ring));
        }
    }
    let Some((_, slot, ring)) = best else { return false };
    jc.joint.giz_handle = Some((slot, ring));
    joint_giz_begin(jc, jid, slot, ring);
    jc.joint.giz_drag.is_some()
}

/// Dragging a DOF gizmo handle: accumulate degrees or millimetres along the pinned frame, write them
/// into the joint parameter, and solve.
pub fn joint_giz_drag_to(jc: &mut qymcad_ui_state::JointCtx, cursor: Pos2, d: egui::Vec2, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
    let scr = qymcad_ui_state::Screen { cam: jc.cam, set: jc.set, rect, basis };
    // THE TUPLE IS COPIED OUT of the borrowed field: matching on `&mut Option<..>` binds every part as
    // `&mut`, and everything downstream then wants a value.
    if let Some((comp, local, from)) = *jc.part_pull {
        // WHERE THE HAND LEADS is computed in the coordinates of WHAT IS ON SCREEN, that is, of the
        // current context (`body_display_transform` draws everything relative to it). The solver, on
        // the other hand, lives in THE WORLD, so the target is converted to world coordinates just
        // before it is handed over.
        let k = 1.0 / jc.cam.scale as f64;
        let (right, up, _) = basis;
        let mut to = from;
        for a in 0..3 {
            to[a] += right[a] * d.x as f64 * k - up[a] * d.y as f64 * k;
        }
        *jc.part_pull = Some((comp, local, to));
        let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
        let to_world = qymcad_core::feature::apply12(&jc.project.world_transform(ctx), to);
        jc.project.drag_pull = Some((comp, local, to_world));
        jc.project.solve_joints();
        jc.project.drag_pull = None;
        qymcad_ui_state::invalidate_placement(jc.regen);
        let _ = (cursor, rect);
        return;
    }
    let Some(dg) = jc.joint.giz_drag else { return };
    let inc = if dg.ring {
        let center = scr.at(dg.o).0;
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
        ccw.to_degrees() * ring_drag_sign(depth)
    } else {
        let l = 60.0 / jc.cam.scale as f64;
        let s0 = scr.at(dg.o).0;
        let s1 = scr.at([dg.o[0] + dg.dir[0] * l, dg.o[1] + dg.dir[1] * l, dg.o[2] + dg.dir[2] * l]).0;
        let pd = s1 - s0;
        let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
        if denom < 1e-6 {
            return;
        }
        (d.x * pd.x + d.y * pd.y) as f64 * l / denom
    };
    if let Some(dg) = &mut jc.joint.giz_drag {
        dg.amt += inc;
    }
    apply_joint_giz(jc);
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
    egui::Area::new(egui::Id::new("joint_edit_popup")).fixed_pos(qymcad_ui_state::clamp_popup(mid, rect) + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
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
                        *jc.status = qymcad_i18n::tr("j-as-built-ok");
                        changed = true;
                    } else {
                        // NO SILENT REFUSAL: the anchor may have failed to resolve, and that must be
                        // learned here rather than from a part that does not move.
                        *jc.status = qymcad_i18n::tr("j-as-built-failed");
                    }
                }
                if as_built_now && ui.button(qymcad_i18n::tr("j-as-built-off")).clicked() {
                    jc.project.clear_joint_as_built(jid);
                    *jc.status = qymcad_i18n::tr("j-as-built-cleared");
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
    if changed {
        qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    }
    if close {
        qymcad_ui_state::exit_joint_edit(jc.joint, jc.status);
    }
}