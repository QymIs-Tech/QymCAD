//! THE GIZMO OF A JOINT: its glyphs, its handles, grabbed and dragged.

use super::*;

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
            let (a, b) =
                qymcad_ui_state::joint_endpoints(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, j, rect, &basis)?;
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
    let Some(qymcad_ui_state::JointGizmo { origin: o, handles: hs }) =
        qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid)
    else {
        return;
    };
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
    let qymcad_ui_state::JointGizmo { origin: o, handles: hs } =
        qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid)?;
    let l = 60.0 / jc.cam.scale as f64;
    // the translation arrows first
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs.iter().filter(|h| !h.ring) {
        let _ = ring;
        let scr = qymcad_ui_state::Screen { cam: jc.cam, set: jc.set, rect, basis };
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

/// STOPPING THE SWEEP of a joint's degree of freedom: the reading goes back to what it was before, and the
/// document is marked. It lived in `gui.rs` for no better reason than that the animation field does.
pub fn stop_joint_anim(jc: &mut qymcad_ui_state::JointCtx) {
    let Some(a) = jc.joint_anim.take() else { return };
    // a preview gives everything back: the mates as they read (the drive of the swept degree with them) and the parts
    // exactly where they stood - solving again would only add the solver's noise to what was there
    jc.project.joints = a.mates;
    for (id, m) in a.placed {
        if let Some(c) = jc.project.components.iter_mut().find(|c| c.id == id) {
            c.transform = m;
        }
    }
    qymcad_ui_state::invalidate_placement(jc.regen);
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
    // A PART NO MATE HOLDS is moved as it is, following the pointer
    if let Some(comp) = jc.project.free_pull_component(owner, ctx) {
        let p0 = qymcad_core::feature::apply12(&jc.project.relative_transform(comp, ctx), [0.0, 0.0, 0.0]);
        qymcad_ui_state::begin_edit(jc.edits, jc.project, qymcad_i18n::tr("status-move-component"));
        *jc.part_pull = Some((comp, [0.0, 0.0, 0.0], p0));
        return true;
    }
    let Some(jid) = jc.project.drive_joint_in_context(owner, ctx) else { return false };
    let Some(qymcad_ui_state::JointGizmo { origin: o, handles: hs }) =
        qymcad_ui_state::joint_giz_handles(&qymcad_ui_state::DrawCtx { cam: jc.cam, set: jc.set, scheme: jc.scheme, project: jc.project, active_path: jc.active_path }, jid)
    else {
        return false;
    };
    if hs.is_empty() {
        return false; // no freedoms left - there is nothing to drive
    }
    // THE AXIS OF A DEGREE ON SCREEN, compared with the direction of the pull.
    let scr = qymcad_ui_state::Screen { cam: jc.cam, set: jc.set, rect, basis };
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
        // a part no mate holds has nothing to solve: it goes where the hand leads, in the coordinates of the assembly
        // it stands in (the context itself)
        if jc.project.free_pull_component(comp, ctx) == Some(comp) {
            let mut m = jc.project.component_transform(comp);
            for (a, k) in [3, 7, 11].into_iter().enumerate() {
                m[k] += to[a] - from[a];
            }
            jc.project.set_component_transform(comp, m);
            qymcad_ui_state::invalidate_placement(jc.regen);
            return;
        }
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
