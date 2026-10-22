//! A COMMAND APPLIED: the feature it makes or edits.

use qymcad_ui_state::{Notice, NotApplied};
use super::*;

/// NOT APPLIED: what the command holds will not do, in the words of catalogue key `key`.
pub(crate) fn refused(key: &str) -> NotApplied {
    NotApplied::Refused(qymcad_i18n::tr(key))
}

/// NOT APPLIED: the body the command works on is not there - never picked, or taken away.
pub(crate) fn no_body() -> NotApplied {
    refused("msg-no-body")
}

/// NOT APPLIED: the operation was laid and did not come out.
pub(crate) fn op_failed() -> NotApplied {
    NotApplied::Failed(qymcad_i18n::tr("cmd-op-failed"))
}

/// NOT APPLIED: a formula the document refused as the command wrote it - one coming back to the name of its own number.
/// The command asks the same door before Enter ([`field_refused_by_document`]), so this is the backstop of the write.
pub(crate) fn formula_refused(e: &qymcad_core::errors::ExprError) -> NotApplied {
    NotApplied::Refused(qymcad_i18n::error_words::expr_error_text(e))
}

pub fn apply_feat_cmd_inner(pc: &mut qymcad_ui_state::PartCtx) -> Result<(), NotApplied> {
    // THE DERIVED VALUE IS COMPUTED HERE rather than when the command opens: the operation may have
    // been switched in the bar after opening (see `smart_flip`).
    if pc.feat.flip_auto {
        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
            pc.feat.flip = pc.armed.part_tool().is_some_and(|t| smart_flip(t, pc.feat.op, &pc.project.sketches[si].plane));
        }
    }
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // after opening from a bundle there are no live shapes yet - bring the cache up once
                                                     // A sketch command (extrude or revolve) takes two steps when SEVERAL contours are picked: while we
                                                     // are in the flat half-sketcher (`mode_3d = false`) and the profiles ARE picked, Apply or Enter first
                                                     // CONFIRMS the selection and moves to 3D (the gizmo plus the size popup) rather than creating the
                                                     // feature silently at the default height. Otherwise the Apply button (which is enabled as soon as the
                                                     // profile selection is non-empty) would apply 10 mm straight away during a Ctrl multi-pick, with no
                                                     // gizmo and no popup, and would breed a chain of nodes. A single contour already moves to 3D at the
                                                     // start.
    if takes_into_3d(pc) {
        qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit
        *pc.mode_3d = true;
        *pc.status = Notice::info(qymcad_i18n::tr("msg-profile-picked"));
        return Ok(());
    }
    // a hard backstop: DO NOT apply while any dimension carries an invalid expression (a stale value
    // would go through otherwise). The gate is duplicated on the buttons, but Enter can arrive past
    // them.
    if !cmd_exprs_valid(pc.cmd, pc.project) {
        return Err(NotApplied::Refused(qymcad_i18n::tr1("cmd-expr-error", "icon", ph::X)));
    }
    let before: std::collections::HashSet<Id> = pc.project.timeline.iter().map(|n| n.id).collect();
    // editing an existing feature (a double click) updates it rather than creating a new one
    let last = if let Some(fid) = pc.cmd.edit {
        update_feat(pc, fid)
    } else {
        match pc.armed.part_tool() {
            Some(t @ (qymcad_ui_state::PartTool::Extrude | qymcad_ui_state::PartTool::Revolve)) => apply_sketch_cmd(pc, t),
            Some(qymcad_ui_state::PartTool::Sweep) => apply_sweep_cmd(*pc.feat, pc.project, *pc.sweep),
            Some(qymcad_ui_state::PartTool::Loft) => apply_loft_cmd(pc),
            Some(t @ (qymcad_ui_state::PartTool::Fillet | qymcad_ui_state::PartTool::Chamfer)) => apply_edge_cmd(pc, t),
            Some(qymcad_ui_state::PartTool::Shell) => apply_shell_cmd(pc),
            Some(qymcad_ui_state::PartTool::Draft) => apply_draft_cmd(pc),
            Some(qymcad_ui_state::PartTool::PushFace) => apply_push_face_cmd(pc),
            Some(qymcad_ui_state::PartTool::RemoveFace) => apply_remove_face_cmd(pc),
            Some(qymcad_ui_state::PartTool::FaceCopy) => apply_face_copy_cmd(pc),
            Some(qymcad_ui_state::PartTool::OffsetSurface) => apply_offset_surface_cmd(pc),
            Some(qymcad_ui_state::PartTool::SurfaceReplace) => apply_surface_replace_cmd(pc),
            Some(qymcad_ui_state::PartTool::Patch) => apply_patch_cmd(pc),
            Some(qymcad_ui_state::PartTool::Stitch) => apply_stitch_cmd(pc.cmd, pc.project, pc.stitch_parts),
            Some(qymcad_ui_state::PartTool::Recognise) => apply_recognise_cmd(pc),
            Some(qymcad_ui_state::PartTool::Trim) => apply_trim_cmd(pc.project, pc.trim),
            Some(qymcad_ui_state::PartTool::SplitBody) => apply_split_cmd(pc),
            Some(qymcad_ui_state::PartTool::Thicken) => apply_thicken_cmd(pc),
            Some(qymcad_ui_state::PartTool::SplitFace) => apply_split_face_cmd(pc),
            Some(qymcad_ui_state::PartTool::Thread) => apply_thread_cmd(pc.cmd, pc.project, *pc.thread),
            Some(qymcad_ui_state::PartTool::Hole) => apply_hole_cmd(pc.cmd, *pc.hole, pc.project, *pc.sel),
            Some(k) if k.is_a_primitive() => apply_prim_cmd(pc),             // the primitives
            Some(qymcad_ui_state::PartTool::Mirror) => apply_mirror_cmd(pc), // the mirror
            Some(qymcad_ui_state::PartTool::LinearPattern) | Some(qymcad_ui_state::PartTool::CircularPattern) => apply_array_cmd(pc), // the array
            Some(k) if k.is_a_datum() => apply_datum_cmd(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project), // a datum plane, point or axis
            _ => Err(op_failed()),
        }
    };
    let body = last?; // not applied: the command stays in hand, the frame says why
                      // WHAT WAS PICKED IS GONE: Ctrl+Z with the picks held took away the step that made them, and the node laid on them
                      // would only stand red. It is not laid - the operation rolls back - the picks go, and the command asks for new ones.
                      // Every node the operation laid is asked - a datum lays one with no body of its own.
    let fresh: Vec<Id> = pc.project.timeline.iter().map(|n| n.id).filter(|id| !before.contains(id)).collect();
    if fresh.iter().any(|&node| !pc.project.gone_inputs(node).is_empty()) {
        qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
        return Err(NotApplied::Refused(qymcad_i18n::tr1("cmd-picks-gone", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim))));
    }
    // WHAT WAS MADE IS NAMED: the node the command left, not a bare "Done" (reported on the block after Enter)
    let made = pc.project.timeline.iter().find(|n| n.kind.body() == Some(body)).map(|n| qymcad_i18n::name(&n.name));
    *pc.status = Notice::info(made.map_or_else(|| qymcad_i18n::tr("f-done"), |what| qymcad_i18n::tr1("f-done-what", "what", &what)));
    // AFTER the status: if the regen fails, `qymcad_ui_state::regenerate_all` writes its own rebuild message - do NOT
    // overwrite it with "done" (otherwise the feature silently fails to build while the screen says
    // "done", which reads as nothing happening at all).
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
    Ok(())
}

/// DOES Enter TAKE THE COMMAND INTO 3D rather than apply it: an extrusion or a revolution in the flat choice of its
/// contours, the contours picked, goes into 3D to set its size there - the gizmo or the field.
pub fn takes_into_3d(pc: &qymcad_ui_state::PartCtx) -> bool {
    pc.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::picks_profiles) && pc.cmd.edit.is_none() && !*pc.mode_3d && !pc.gsel.profiles.is_empty()
}

/// THE COMMAND PUT DOWN ONCE IT MADE ITS FEATURE - the door of the frame after Apply (`AfterApply::Select`): a clean
/// finish, the command dropped and the selection cleared, so neither the gizmo arrows nor the right-hand panel remain;
/// back from the flat half-sketcher into 3D the view borrowed is RETURNED rather than refitted - refitting threw away
/// everything set up by hand, and finishing a command looked as if the viewport had flown off.
pub fn put_down_command(pc: &mut qymcad_ui_state::PartCtx) {
    pc.cmd.close(pc.armed);
    qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
    qymcad_ui_state::return_view(pc);
    *pc.sel = qymcad_ui_state::Sel::None;
}

/// APPLY THE COMMAND AND PUT IT DOWN, as Enter and the door of the frame after it do - for a check or a build that
/// applies a command with no frame. Taken into 3D, the command stays in hand.
pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) -> Result<(), NotApplied> {
    let into_3d = takes_into_3d(pc);
    apply_feat_cmd_kept(pc)?;
    if !into_3d {
        put_down_command(pc);
    }
    Ok(())
}

/// APPLY THE COMMAND IN HAND, the command left in hand for the door of the frame to put down.
pub fn apply_feat_cmd_kept(pc: &mut qymcad_ui_state::PartCtx) -> Result<(), NotApplied> {
    // NOTHING TO APPLY YET: Enter is refused in words, the command's own ask repeated, and the document untouched -
    // unless the command names what is missing itself (the axis, the faces, the patch's edges, the trimmed sheet)
    // A FIELD THAT CANNOT BE TAKEN refuses Enter as it closes the Apply button: Enter comes from the keyboard, past it
    let vars = pc.project.param_map();
    // what the document refuses first: a formula reading a circle of names works out to a stale number, and the limits
    // of the field would refuse that number instead of the circle
    let field_refused = |p: &qymcad_ui_state::CmdParam| field_refused_by_document(p, pc.project, pc.cmd.edit).or_else(|| qymcad_ui_state::field_value(p, &vars).err());
    if let Some((label, msg)) = pc.cmd.params.iter().find_map(|p| field_refused(p).map(|m| (qymcad_i18n::tr(p.label_key()), m))) {
        return Err(NotApplied::Refused(format!("{label}: {msg}")));
    }
    if !cmd_ready(pc) && !pc.armed.part_tool().is_some_and(qymcad_ui_state::PartTool::says_what_it_wants) {
        return Err(NotApplied::Refused(qymcad_i18n::tr1("cmd-not-ready", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim))));
    }
    // the counts as their fields hold them in this very frame: Enter in the field of a count applies what was typed
    take_counts(&*pc.armed, &*pc.cmd, Counts { arr: &mut *pc.arr, prim: &mut *pc.prim });
    // THE BOUNDARY OF THE OPERATION: everything the command does to the document is one undo step with
    // a name of its own. It opens HERE rather than in `start_feat_cmd`: opening a dialog does not change
    // the document and there is nothing to undo there. It closes at the end: success commits, a refusal
    // rolls back and leaves no trace.
    let name = feat_cmd_name(&*pc.armed, *pc.feat);
    // OPENED AND CLOSED BY NAME, without the guard. `qymcad_ui_state::commit_edit` counts the nesting itself
    // (`edits.depth`), so the boundary holds the same as before; what goes is the guard's hold on the
    // WHOLE application, which existed for this one call and no other.
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, name);
    let done = apply_feat_cmd_inner(pc);
    // A FACT, NOT A GUESS FROM TEXT: the answer of the command, not words of the status line - once the interface was
    // translated a phrase would no longer match, and a failed operation would land in the undo history as a successful
    // one, Ctrl+Z then rolling back the wrong thing. A refusal rolls back whatever the command laid before it.
    if done.is_err() {
        qymcad_ui_state::abort_edit(&mut pc.rebuild());
    } else {
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
    }
    done
}

/// Apply the world transform `accum` to the body of mesh `mi`: if the body is already a Move feature,
/// accumulate into its matrix (no chain of Move nodes); otherwise create a new Move, for a B-rep and a raw mesh alike.
pub fn apply_body_move(pc: &mut qymcad_ui_state::PartCtx, mi: usize, accum: [f64; 12]) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-move-body")); // THE BOUNDARY OF AN OPERATION
    use qymcad_core::feature::FeatureKind;
    let Some(body) = pc.project.mesh_id(mi) else { return };
    let is_move = pc.project.timeline.iter().any(|n| n.id == body && matches!(n.kind, FeatureKind::Move { .. }));
    if is_move {
        pc.project.accumulate_move(body, &accum);
        qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
        qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, body);
    } else {
        qymcad_ui_state::move_body_at(&mut pc.rebuild(), mi, accum); // a new Move feature (and selection), a raw mesh's moved by the document
    }
    qymcad_ui_state::after_placement_change(&mut pc.rebuild()); // the source moved -> rebuild whoever consumes it
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Ctrl+V OF A TIMELINE NODE: its tool opens again with its values - the diameter, the depth, the kind - as a NEW
/// operation on the part's body as it stands now, and waits for where it goes: a click places it, Enter applies.
/// A copy dropped where the original stands would change nothing, so it is not made by itself.
pub fn paste_feature(pc: &mut qymcad_ui_state::PartCtx, nid: Id) {
    if !pc.project.timeline.iter().any(|n| n.id == nid) {
        *pc.status = Notice::refusal(qymcad_i18n::tr("tree-clipboard-empty"));
        return;
    }
    start_feat_cmd_edit(pc, nid);
    pc.cmd.edit = None; // a new node, not the copied one edited
                        // the face it was made on, found on the body the part ends in now
    let dc = qymcad_ui_state::DrawCtx { cam: pc.cam, set: pc.set, scheme: pc.scheme, project: pc.project, active_path: pc.active_path };
    let now = qymcad_ui_state::current_body(&dc);
    let face = match *pc.sel {
        qymcad_ui_state::Sel::Face(mi, fi) => pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map(|f| qymcad_core::feature::FaceKey {
            index: 0,
            centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
            normal: f.normal,
            id: f.id,
        }),
        _ => None,
    };
    if let (Some(body), Some(key)) = (now, face) {
        if let Some((mi, fi)) = qymcad_pick::resolve_face_sel(&*pc.project, body, &key) {
            *pc.sel = qymcad_ui_state::Sel::Face(mi, fi);
            pc.gsel.faces.clear();
            pc.gsel.faces.insert(key.id);
        }
    }
    *pc.status = Notice::info(qymcad_i18n::tr("g-paste-feature"));
}

mod update;
pub use update::*;
mod solids;
pub use solids::*;
mod faces;
pub use faces::*;
mod copies;
pub use copies::*;
