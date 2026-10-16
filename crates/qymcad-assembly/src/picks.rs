//! WHAT A CLICK TAKES IN AN ASSEMBLY: anchors, edges, faces, origins, widths, groups, tangents, relations, ground.

use qymcad_ui_state::Notice;
use super::*;

/// WHETHER A TOOL IS READING CLICKS ON PARTS: the anchors of a mate, width, tangent, group, grounding, relation or a
/// connector. While it is, a drag across a part turns the view and never carries the part.
pub fn joint_picking(joint: &qymcad_ui_state::JointCommand) -> bool {
    joint.pick_faces || joint.ground_pick || joint.group_pick.is_some() || joint.width_pick.is_some() || joint.tangent_pick.is_some() || joint.relation_pick.is_some() || joint.conn_pick
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
pub fn width_pick_click(active_path: &[Id], joint: &mut qymcad_ui_state::JointCommand, project: &mut Project, status: &mut qymcad_ui_state::Notice, body: Id, key: qymcad_core::feature::FaceKey) {
    use qymcad_core::feature::AnchorRef;
    if joint.width_pick.is_none() {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let Some(owner) = project.body_owner(body) else {
        *status = Notice::refusal(qymcad_i18n::tr("j-face-no-part"));
        return;
    };
    let comp = project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let n = key.normal;
    let Some(sel) = joint.width_pick.as_mut() else { return };
    // THE SAME FACE AGAIN lets it go, as a second click lets go every pick - it was taken twice, two anchors of three
    let same = |a: &AnchorRef| matches!(a, AnchorRef::FaceCenter(b, k) if *b == body && (if k.id != 0 { k.id == key.id } else { k.index == key.index }));
    if let Some(i) = sel.iter().position(|(_, a)| same(a)) {
        sel.remove(i);
        *status = Notice::info(qymcad_i18n::tr1("j-width-picked", "n", &sel.len().to_string()));
        return;
    }
    if sel.len() == 1 {
        if let Some((_, AnchorRef::FaceCenter(_, k0))) = sel.first() {
            let dot = k0.normal[0] * n[0] + k0.normal[1] * n[1] + k0.normal[2] * n[2];
            if dot.abs() < 0.999 {
                *status = Notice::refusal(qymcad_i18n::tr("j-width-walls-differ"));
                return;
            }
        }
    }
    if sel.len() >= 3 {
        sel.clear(); // one too many - start the set over rather than piling up silently
    }
    sel.push((comp, AnchorRef::FaceCenter(body, key)));
    let n = sel.len();
    *status = Notice::info(qymcad_i18n::tr1("j-width-picked", "n", &n.to_string()));
}

/// A click on a body while the group tool is active: add its part to the set or take it out.
pub fn group_pick_click(active_path: &[Id], joint: &mut qymcad_ui_state::JointCommand, project: &mut Project, status: &mut qymcad_ui_state::Notice, body: Id) {
    if joint.group_pick.is_none() {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(active_path, project);
    let Some(owner) = project.body_owner(body) else {
        *status = Notice::refusal(qymcad_i18n::tr("j-body-no-part"));
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
    *status = Notice::info(qymcad_i18n::tr1("j-group-picked", "n", &n.to_string()));
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_ground_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.ground_pick = on;
    jc.joint.pick_first = None;
    *jc.status = Notice::info(if on { qymcad_i18n::tr("tb-ground-pick") } else { qymcad_i18n::tr("tb-ground-off") });
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_group_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.group_pick = on.then(Vec::new);
    *jc.status = Notice::info(qymcad_i18n::tr(if on { "j-group-pick" } else { "j-group-off" }));
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_tangent_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.tangent_pick = on.then(Vec::new);
    *jc.status = Notice::info(qymcad_i18n::tr(if on { "j-tangent-pick" } else { "j-tangent-off" }));
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_width_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.width_pick = on.then(Vec::new);
    *jc.status = Notice::info(qymcad_i18n::tr(if on { "j-width-pick" } else { "j-width-off" }));
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_conn_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.conn_pick = on;
    *jc.status = Notice::info(qymcad_i18n::tr(if on { "j-conn-pick" } else { "j-conn-off" }));
}

/// The arming half of the tool: the reading of the flag and the clearing of every OTHER tool stayed with
/// the application, because mutual exclusion between workbenches is its decision and not the assembly's.
pub fn start_relation_pick_armed(jc: &mut qymcad_ui_state::JointCtx, on: bool) {
    jc.joint.relation_pick = on.then(qymcad_ui_state::RelationPick::default);
    *jc.status = Notice::info(qymcad_i18n::tr(if on { "j-relation-pick" } else { "j-relation-off" }));
}

/// TAKE A PICK FOR THE SECONDARY AXIS: a click on an edge or a face sets the axis of the connector.
///
/// The direction is taken from the picked geometry and laid perpendicular to the main axis of the
/// anchor - the pick sets THE SIDE, it does not replace the anchor. Geometry that has no side (a vertex,
/// the origin of a part) never gets here: it does not enter the pick.
pub fn joint_axis_pick_apply(jc: &mut qymcad_ui_state::JointCtx, anchor: qymcad_core::feature::AnchorRef) {
    let Some(cid) = jc.joint.axis_pick else { return };
    if jc.project.anchor_direction(&anchor).is_none() {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-axis-no-direction"));
        return;
    }
    jc.project.set_connector_axis_ref(cid, Some(anchor));
    jc.joint.axis_pick = None;
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    *jc.status = Notice::info(qymcad_i18n::tr("j-axis-set"));
}

/// Apply a newly picked anchor while editing (swap the face, edge or vertex of A or B on the fly). The
/// checks are the same as at creation: the part is in the active context, and the anchors are not in one
/// subassembly. The connector is updated through the core's `set_connector_anchor`, then a rebuild.
pub fn joint_edit_repick_apply(jc: &mut qymcad_ui_state::JointCtx, body: Id, anchor: qymcad_core::feature::AnchorRef) {
    let Some((jid, is_b)) = jc.joint.edit_repick else { return };
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let Some(body_owner) = jc.project.body_owner(body) else {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-anchor-no-part"));
        return;
    };
    if !jc.project.component_is_within(body_owner, ctx) {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-outside-assembly"));
        return;
    }
    let place = jc.project.ancestor_child_of(ctx, body_owner).unwrap_or(body_owner);
    let Some(j) = jc.project.joints.iter().find(|x| x.id == jid) else { return };
    let (cid, other) = if is_b { (j.b, j.a) } else { (j.a, j.b) };
    if jc.project.connector(other).map(|c| c.owner) == Some(place) {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-same-part"));
        return;
    }
    jc.project.set_connector_anchor(cid, place, anchor);
    jc.joint.edit_repick = None;
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    *jc.status = Notice::info(qymcad_i18n::tr1("jt-anchor-replaced", "side", if is_b { "B" } else { "A" }));
}

/// A thin wrapper: a click on AN EDGE -> an `EdgeMid` axis anchor.
pub fn joint_pick_edge_click(jc: &mut qymcad_ui_state::JointCtx, body: Id, edge_id: u32) {
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-edge-no-part"));
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
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-face-no-part"));
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
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-anchor-no-part"));
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
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-face-no-part"));
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
        *jc.status = Notice::info(qymcad_i18n::tr2("j-relation-picked", "n", &have.to_string(), "need", &need.to_string()));
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
    *jc.status = Notice::info(qymcad_i18n::tr("j-relation-made-ok"));
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// A click on A JOINT while the relation tool is active: take its degree of the required sort.
pub fn relation_pick_click(jc: &mut qymcad_ui_state::JointCtx, joint: Id) {
    let Some(pick) = jc.joint.relation_pick.clone() else { return };
    let need = relation_picks_needed(pick.kind);
    // A MATE TAKEN AND CLICKED AGAIN IS LET GO, as a second click on any pick; it used to answer "two different mates"
    if pick.picks.iter().any(|(id, _)| *id == joint) {
        if let Some(p) = jc.joint.relation_pick.as_mut() {
            p.picks.retain(|(id, _)| *id != joint);
            let got = if need == 1 { 0 } else { p.picks.len() };
            *jc.status = Notice::info(qymcad_i18n::tr2("j-relation-picked", "n", &got.to_string(), "need", &need.to_string()));
        }
        return;
    }
    let (rot_a, rot_b) = pick.kind.slots_are_rotations();
    // THE SCREW TAKES BOTH DEGREES OF ONE JOINT - the angle and the travel of a cylindrical one.
    let wanted: Vec<bool> = if need == 1 { vec![rot_a, rot_b] } else { vec![if pick.picks.is_empty() { rot_a } else { rot_b }] };
    let mut taken: Vec<(Id, usize)> = Vec::new();
    for &want in &wanted {
        let Some(slot) = relation_slot_of(jc.project, joint, want) else {
            // NO SILENT REFUSAL: the click has already happened and an answer is expected.
            *jc.status = Notice::refusal(qymcad_i18n::tr(if want { "j-relation-need-turn" } else { "j-relation-need-travel" }));
            return;
        };
        taken.push((joint, slot));
    }
    let Some(pick) = jc.joint.relation_pick.as_mut() else { return };
    pick.picks.extend(taken);
    let got = if need == 1 { 1 } else { pick.picks.len() };
    *jc.status = Notice::info(qymcad_i18n::tr2("j-relation-picked", "n", &got.to_string(), "need", &need.to_string()));
}

/// DELETE AN ANCHOR - or say why that is not possible.
pub fn delete_connector_asked(jc: &mut qymcad_ui_state::JointCtx, cid: Id) {
    let users = jc.project.connector_users(cid).len();
    if users > 0 {
        // NO SILENT REFUSAL: the cross was pressed, and what held the anchor back must be said.
        *jc.status = Notice::refusal(qymcad_i18n::tr1("j-conn-in-use", "n", &users.to_string()));
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-conn-deleted"));
    jc.project.delete_connector(cid);
    if *jc.sel_conn == Some(cid) {
        *jc.sel_conn = None;
    }
    *jc.status = Notice::info(qymcad_i18n::tr("j-conn-deleted"));
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// Confirm the width set: two walls and the piece between them.
pub fn width_pick_confirm(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(sel) = jc.joint.width_pick.clone() else { return };
    if sel.len() < 3 {
        // NO SILENT REFUSAL: some of the anchors have already been shown and a result is expected.
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-width-need-three"));
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-width-made")); // THE BOUNDARY OF AN OPERATION
    let ids: Vec<Id> = sel.iter().map(|(owner, a)| jc.project.add_connector(*owner, a.clone())).collect();
    jc.project.add_width(&[ids[0], ids[1]], ids[2]);
    jc.joint.width_pick = None;
    *jc.status = Notice::info(qymcad_i18n::tr("j-width-made-ok"));
    joint_console_settle(jc); // the tab goes halfway now, not on some later solve
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
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-face-no-part"));
        return;
    };
    let comp = jc.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let is_cyl = jc.project.face_cylinder(body, &key).is_some();
    let Some(sel) = jc.joint.tangent_pick.as_mut() else { return };
    if sel.len() >= 2 {
        return; // the condition is made and waits for Apply or Esc; a further click adds nothing
    }
    // the surface taken and clicked again is let go, as a second click on any pick
    if let Some(k) = sel.iter().position(|(_, a)| matches!(a, AnchorRef::FaceCenter(b, fk) if *b == body && fk.index == key.index)) {
        sel.remove(k);
        *jc.status = Notice::info(qymcad_i18n::tr("j-tangent-pick"));
        return;
    }
    if let Some((_, AnchorRef::FaceCenter(b0, k0))) = sel.first().cloned() {
        // the second surface must complement the first: a cylinder to a plane and the other way round
        if jc.project.face_cylinder(b0, &k0).is_some() == is_cyl {
            *jc.status = Notice::refusal(qymcad_i18n::tr("j-tangent-need-cylinder"));
            return;
        }
    }
    let Some(sel) = jc.joint.tangent_pick.as_mut() else { return };
    sel.push((comp, AnchorRef::FaceCenter(body, key)));
    if sel.len() < 2 {
        *jc.status = Notice::info(qymcad_i18n::tr("j-tangent-second"));
        return;
    }
    let pair = sel.clone();
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-tangent-made")); // THE BOUNDARY OF AN OPERATION
    jc.project.add_tangent(pair[0].0, pair[0].1.clone(), pair[1].0, pair[1].1.clone());
    *jc.status = Notice::info(qymcad_i18n::tr("j-tangent-made-ok"));
    joint_console_settle(jc); // the round part rests on the flat one now, not on some later solve
                              // THE OPERATION STAYS OPEN: the part rests where the condition puts it as a preview until Apply or Enter keeps it and
                              // Esc takes it away (`tangent_finish`); the two picks stay in `tangent_pick` as the mark of that
}

/// FINISH A TANGENT CONDITION JUST MADE by its second pick: kept, its open operation closes as one step; not kept, the
/// operation is rolled back. The tool is put down either way. Nothing happens while the second surface is awaited.
pub fn tangent_finish(jc: &mut qymcad_ui_state::JointCtx, keep: bool) {
    if !jc.joint.tangent_pick.as_ref().is_some_and(|s| s.len() >= 2) {
        return;
    }
    jc.joint.tangent_pick = None;
    if keep {
        qymcad_ui_state::close_edit(jc.edits, jc.project); // closed, not committed: committing drops the asked rebuild
    } else {
        qymcad_ui_state::abort_edit(&mut jc.rebuild());
        *jc.status = Notice::info(qymcad_i18n::tr("j-tangent-off"));
    }
}

/// Confirm the set and make a group of it. Fewer than two parts leaves nothing to fasten.
pub fn group_pick_confirm(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(sel) = jc.joint.group_pick.clone() else { return };
    if sel.len() < 2 {
        // NO SILENT REFUSAL: a part has already been clicked and a result is expected.
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-group-need-two"));
        return;
    }
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("j-group-made")); // THE BOUNDARY OF AN OPERATION
    jc.project.add_group(&sel);
    jc.joint.group_pick = None;
    *jc.status = Notice::info(qymcad_i18n::tr1("j-group-made-n", "n", &sel.len().to_string()));
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// A CLICK OF THE GROUNDING TOOL: on a part it fixes or releases it, past every part it says so.
pub fn ground_click(jc: &mut qymcad_ui_state::JointCtx, hit: Option<Id>) {
    match hit {
        Some(body) => joint_pick_ground_click(jc, body),
        None => *jc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-part-ground")),
    }
}

/// The grounding tool: a click on a body fixes or releases its part (at the level of the context).
/// As when creating a joint, what gets grounded is the DIRECT child of the context (the subassembly)
/// rather than a leaf body.
pub fn joint_pick_ground_click(jc: &mut qymcad_ui_state::JointCtx, body: Id) {
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    let Some(owner) = jc.project.body_owner(body) else {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-body-no-part"));
        return;
    };
    let comp = jc.project.ancestor_child_of(ctx, owner).unwrap_or(owner);
    let g = jc.project.is_grounded(comp);
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("jt-ground-btn")); // THE BOUNDARY OF AN OPERATION
    jc.project.set_grounded(comp, !g);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // the document is marked; the planner does the counting
    let name = jc.project.components.iter().find(|c| c.id == comp).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
    *jc.status = Notice::info(if !g { qymcad_i18n::tr1("jt-grounded", "name", &name) } else { qymcad_i18n::tr1("jt-released", "name", &name) });
    qymcad_ui_state::close_edit(jc.edits, jc.project); // closed, not committed: committing drops the asked rebuild, and the assembly would not be solved
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
        *jc.status = Notice::info(qymcad_i18n::tr("j-conn-made-ok"));
        qymcad_ui_state::commit_edit(&mut jc.rebuild());
        return;
    }
    // a joint only ever goes between parts of the ACTIVE context. Ghost parts of other subassemblies
    // are visible so they can be referred to, but they are dimmed for a reason - no joint lands on them.
    let ctx = qymcad_ui_state::current_ctx_id(jc.active_path, jc.project);
    if !jc.project.component_is_within(owner, ctx) {
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-outside-assembly-joint"));
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
        *jc.status = Notice::refusal(qymcad_i18n::tr("j-anchor-on-moving-part-refused"));
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
        *jc.status = if jc.live.ready { Notice::refusal(qymcad_i18n::tr("j-geometry-missing")) } else { Notice::info(qymcad_i18n::tr("j-geometry-on-its-way")) };
        return;
    }
    match jc.joint.pick_first.take() {
        None => {
            jc.joint.pick_first = Some((owner, anchor));
            *jc.status = Notice::info(qymcad_i18n::tr("j-anchor-a-picked"));
        }
        Some((owner_a, anchor_a)) => {
            // THE SAME PLACE AGAIN lets anchor A go, as a second click lets go every pick: the joint asks for A anew
            if owner_a == owner && anchor_a == anchor {
                let target = if jc.joint.anchor_mode == 3 { qymcad_i18n::tr("j-origin-lower") } else { qymcad_i18n::tr("j-place-lower") };
                *jc.status = Notice::info(qymcad_i18n::tr1("jt-click-a", "what", &target));
                return;
            }
            if owner_a == owner {
                jc.joint.pick_first = Some((owner_a, anchor_a)); // the same part - wait for another one
                *jc.status = Notice::info(qymcad_i18n::tr("j-pick-other-part"));
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
                *jc.status = Notice::refusal(qymcad_i18n::tr("j-same-subassembly"));
                return;
            }
            // The "anchor compatibility" check is gone from here: it always answered "compatible", so
            // the refusal existed only in the text. An anchor is a full coordinate frame, and any kind
            // of joint works with any pair; which degrees of freedom are left is stated by the kind.
            // the side of the contact is decided by the solver (coplanar, minimal motion) plus the
            // `joint.flip` toggle in the panel - forcing a flip on the connector is no longer needed
            // (it caused a spurious 180 deg turn).
            qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("jt-joint-btn")); // THE BOUNDARY OF AN OPERATION
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
                *jc.status = Notice::error(qymcad_i18n::tr("j-as-built-failed"));
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
            *jc.status = Notice::info(qymcad_i18n::tr("j-created"));
            // THE OPERATION STAYS OPEN: the joint stands solved as a preview until Apply or Enter keeps it and Esc takes it
            // away (`joint_create_finish`). It is solved here: no rebuild runs while an operation is open, and the parts
            // would stand where they were until Enter.
            joint_console_settle(jc);
            jc.joint.creating = true;
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
        joint_console_settle(jc);
    }
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}

/// WHAT A HANDLE OF THE JOINT CONSOLE LEAVES BEHIND: the assembly solved again and its placement marked, at once. Marking
/// the document for a rebuild alone moved nothing - a rebuild computes shapes, and the placement the flip or the swap
/// changes comes from the solve; "Done" was said over a part standing where it stood.
pub(crate) fn joint_console_settle(jc: &mut qymcad_ui_state::JointCtx) {
    jc.project.solve_joints();
    qymcad_ui_state::invalidate_placement(jc.regen);
    qymcad_ui_state::mark_dirty_for_rebuild(&mut jc.rebuild()); // what leans on the placement is rebuilt
}

/// THE CONSOLE HANDLE "FLIP THE AXIS".
///
/// The main axis is flipped at the FIRST anchor: it sets the direction the second one is brought to,
/// and flipping the second would be the same thing inside out. What that changes in the document is
/// THE CORE's business (`flip_joint_side`): the interface only presses the handle.
pub fn joint_hud_flip_axis(jc: &mut qymcad_ui_state::JointCtx, jid: Id) {
    qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("jt-flip-axis")); // THE BOUNDARY OF AN OPERATION
    if jc.project.flip_joint_side(jid) {
        joint_console_settle(jc);
    }
    qymcad_ui_state::commit_edit(&mut jc.rebuild());
}
