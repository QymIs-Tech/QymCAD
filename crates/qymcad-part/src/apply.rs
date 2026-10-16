//! A COMMAND APPLIED: the feature it makes or edits.

use qymcad_ui_state::Notice;
use super::*;

pub fn apply_feat_cmd_inner(pc: &mut qymcad_ui_state::PartCtx) {
    // THE DERIVED VALUE IS COMPUTED HERE rather than when the command opens: the operation may have
    // been switched in the bar after opening (see `smart_flip`).
    if pc.feat.flip_auto {
        if let Some(si) = pc.cmd.sketch.filter(|i| *i < pc.project.sketches.len()) {
            pc.feat.flip = smart_flip(pc.armed.cmd_kind(), pc.feat.op, &pc.project.sketches[si].plane);
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
    if matches!(pc.armed.cmd_kind(), 1 | 3) && pc.cmd.edit.is_none() && !*pc.mode_3d && !pc.gsel.profiles.is_empty() {
        qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, pc.view_restore); // the view is borrowed for the command and returned on exit
        *pc.mode_3d = true;
        *pc.status = Notice::info(qymcad_i18n::tr("msg-profile-picked"));
        return;
    }
    // a hard backstop: DO NOT apply while any dimension carries an invalid expression (a stale value
    // would go through otherwise). The gate is duplicated on the buttons, but Enter can arrive past
    // them.
    if !cmd_exprs_valid(pc.cmd, pc.project) {
        *pc.status = Notice::refusal(qymcad_i18n::tr1("cmd-expr-error", "icon", ph::X));
        return;
    }
    let before: std::collections::HashSet<Id> = pc.project.timeline.iter().map(|n| n.id).collect();
    // editing an existing feature (a double click) updates it rather than creating a new one
    let last = if let Some(fid) = pc.cmd.edit {
        update_feat(pc, fid)
    } else {
        match pc.armed.cmd_kind() {
            1 | 3 => apply_sketch_cmd(pc, pc.armed.cmd_kind()),
            8 => apply_sweep_cmd(*pc.feat, pc.project, pc.status, *pc.sweep),
            9 => apply_loft_cmd(pc),
            4 | 5 => apply_edge_cmd(pc, pc.armed.cmd_kind()),
            6 => apply_shell_cmd(pc),
            23 => apply_draft_cmd(pc),
            25 => apply_push_face_cmd(pc),
            26 => apply_remove_face_cmd(pc),
            30 => apply_face_copy_cmd(pc),
            36 => apply_offset_surface_cmd(pc),
            31 => apply_surface_replace_cmd(pc),
            32 => apply_patch_cmd(pc),
            33 => apply_stitch_cmd(pc.cmd, pc.project, pc.status, pc.stitch_parts),
            35 => apply_recognise_cmd(pc),
            34 => apply_trim_cmd(pc.project, pc.status, pc.trim),
            27 => apply_split_cmd(pc),
            28 => apply_thicken_cmd(pc),
            29 => apply_split_face_cmd(pc),
            24 => apply_thread_cmd(pc.cmd, pc.project, pc.status, *pc.thread),
            7 => apply_hole_cmd(pc.cmd, *pc.hole, pc.project, *pc.sel, pc.status),
            10..=15 => apply_prim_cmd(pc),                                                           // the primitives
            16 => apply_mirror_cmd(pc),                                                              // the mirror
            17 | 18 => apply_array_cmd(pc),                                                          // the array
            20..=22 => apply_datum_cmd(pc.armed, pc.cmd, pc.datum, pc.edges, pc.project, pc.status), // a datum plane, point or axis
            _ => None,
        }
    };
    let Some(body) = last else { return }; // on failure we stay in the command (the status is already set)
                                           // WHAT WAS PICKED IS GONE: Ctrl+Z with the picks held took away the step that made them, and the node laid on them
                                           // would only stand red. It is not laid - the operation rolls back - the picks go, and the command asks for new ones.
                                           // Every node the operation laid is asked - a datum lays one with no body of its own.
    let fresh: Vec<Id> = pc.project.timeline.iter().map(|n| n.id).filter(|id| !before.contains(id)).collect();
    if fresh.iter().any(|&node| !pc.project.gone_inputs(node).is_empty()) {
        *pc.cmd_failed = true;
        qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
        *pc.status = Notice::refusal(qymcad_i18n::tr1("cmd-picks-gone", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim)));
        return;
    }
    // WHAT WAS MADE IS NAMED: the node the command left, not a bare "Done" (reported on the block after Enter)
    let made = pc.project.timeline.iter().find(|n| n.kind.body() == Some(body)).map(|n| qymcad_i18n::name(&n.name));
    // A CLEAN finish: the command is dropped and the selection is cleared, so neither the gizmo arrows
    // nor the right-hand panel remain
    pc.cmd.open(pc.armed, 0, *pc.mode_3d); // a clean slate, then open

    qymcad_ui_state::clear_feat_picks(qymcad_ui_state::feat_picks_in!(pc));
    // back from the flat half-sketcher into 3D: RETURN the view that was borrowed rather than refitting
    // it. Refitting threw away everything that had been set up by hand, and finishing a command looked
    // as if the viewport had flown off.
    qymcad_ui_state::return_view(pc);
    *pc.status = Notice::info(made.map_or_else(|| qymcad_i18n::tr("f-done"), |what| qymcad_i18n::tr1("f-done-what", "what", &what)));
    // AFTER the status: if the regen fails, `qymcad_ui_state::regenerate_all` writes its own rebuild message - do NOT
    // overwrite it with "done" (otherwise the feature silently fails to build while the screen says
    // "done", which reads as nothing happening at all).
    qymcad_ui_state::mark_dirty_for_rebuild(&mut pc.rebuild()); // the document is marked; the planner does the counting
    *pc.sel = qymcad_ui_state::Sel::None;
}

pub fn apply_feat_cmd(pc: &mut qymcad_ui_state::PartCtx) {
    // NOTHING TO APPLY YET: Enter is refused in words, the command's own ask repeated, and the document untouched -
    // unless the command names what is missing itself (the axis, the faces, the patch's edges, the trimmed sheet)
    // A FIELD THAT CANNOT BE TAKEN refuses Enter as it closes the Apply button: Enter comes from the keyboard, past it
    if let Some((label, msg)) = pc.cmd.params.iter().find_map(|p| field_value(p, &pc.project.param_map()).err().map(|m| (qymcad_i18n::tr(p.label_key()), m))) {
        *pc.status = Notice::refusal(format!("{label}: {msg}"));
        return;
    }
    if !cmd_ready(pc) && !matches!(pc.armed.cmd_kind(), 22 | 25 | 26 | 28 | 30 | 32 | 34) {
        *pc.status = Notice::refusal(qymcad_i18n::tr1("cmd-not-ready", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim)));
        return;
    }
    // THE BOUNDARY OF THE OPERATION: everything the command does to the document is one undo step with
    // a name of its own. It opens HERE rather than in `start_feat_cmd`: opening a dialog does not change
    // the document and there is nothing to undo there. It closes at the end: success commits, a refusal
    // rolls back and leaves no trace.
    let name = feat_cmd_name(&*pc.armed, *pc.feat);
    // OPENED AND CLOSED BY NAME, without the guard. `qymcad_ui_state::commit_edit` counts the nesting itself
    // (`edits.depth`), so the boundary holds the same as before; what goes is the guard's hold on the
    // WHOLE application, which existed for this one call and no other.
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, name);
    *pc.cmd_failed = false;
    apply_feat_cmd_inner(pc);
    // A FACT, NOT A GUESS FROM TEXT. This used to search the status line for a substring meaning "did
    // not succeed" - checking what was written for a person instead of what actually happened. Once the
    // interface was translated the status would no longer match, and a failed operation would land in
    // the undo history as a successful one: Ctrl+Z then rolls back the wrong thing.
    if *pc.cmd_failed {
        qymcad_ui_state::abort_edit(&mut pc.rebuild());
    } else {
        qymcad_ui_state::commit_edit(&mut pc.rebuild());
    }
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
