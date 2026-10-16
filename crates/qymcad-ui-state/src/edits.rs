//! UNDO AND REDO: an edit begun, committed, folded, taken back.

use super::*;

/// Abort a lasting operation: the document comes back as it was, and no step is left behind.
pub fn abort_edit(rc: &mut RebuildCtx) {
    rc.edits.depth = rc.edits.depth.saturating_sub(1);
    if rc.edits.depth > 0 {
        return;
    }
    let Some((_, before)) = rc.edits.open.take() else { return };
    rc.edits.open_kind = OpenEdit::Working;
    rc.regen.pending = false; // the operation was rolled back: there is nothing to rebuild
    let _ = restore(rc, before); // an aborted operation leaves the window where it is
    rc.edits.committed_key = doc_key(rc.project);
}

/// Close an operation that is whole when it returns, leaving a step only if it changed the document: the first
/// click of a circle only holds its centre in the tool, and a step for it made one circle take two presses of
/// Ctrl+Z, the first of them undoing nothing. Not for an operation whose result lands later (a build in the
/// background) - there the change is not in the document yet when it closes.
pub fn commit_edit_if_changed(rc: &mut RebuildCtx) {
    let unchanged = rc.edits.depth == 1 && rc.edits.open.as_ref().is_some_and(|(_, before)| doc_key(&before.project) == doc_key(rc.project));
    if unchanged {
        rc.edits.depth = 0;
        rc.edits.open = None;
        rc.edits.open_kind = OpenEdit::Working;
    } else {
        commit_edit(rc);
    }
}

/// FOLD WHAT CHANGED SINCE INTO THE LAST STEP: undoing that step takes this change with it, and no step of its own
/// is made. Only between operations; with one open, it is the open one that takes the change.
/// THE LAST STEP TAKEN BACK AND FORGOTTEN: the document as it was before it, and no step to redo - for a step a tool laid
/// on its way and is put down without finishing (the lines a corner tool cut at a crossing, the corner never cut).
pub fn take_back_last_step(edits: &mut Edits, project: &mut Project, regen: &mut Rebuilding) {
    if edits.open.is_some() {
        return;
    }
    let Some(step) = edits.undo.pop() else { return };
    *project = step.snap.project.clone();
    edits.baseline = step.snap;
    edits.committed_key = doc_key(project);
    invalidate(regen);
}

pub fn fold_into_last_step(edits: &mut Edits, project: &Project) {
    if edits.open.is_some() || edits.undo.is_empty() {
        return;
    }
    edits.baseline = Snapshot { project: project.clone() };
    edits.committed_key = doc_key(project);
    edits.redo.clear();
}

/// LAY WHAT A PANEL CHANGED AS ONE NAMED STEP, measured from the last committed state: for a panel whose widgets write
/// straight into the document (the values of a joint), where the state before the change is no longer at hand once it
/// is known that something changed. `continuing` - the same drag as the step before - folds the change into that step,
/// so a drag across forty frames is one step, not forty.
pub fn settle_step(edits: &mut Edits, project: &Project, name: impl Into<String>, continuing: bool) {
    if edits.open.is_some() {
        return; // an open operation takes the change itself
    }
    let k = doc_key(project);
    if k == edits.committed_key {
        return;
    }
    if continuing && !edits.undo.is_empty() {
        fold_into_last_step(edits, project);
        edits.committed_key = k;
        return;
    }
    let name = name.into();
    note_step(&name);
    let before = std::mem::replace(&mut edits.baseline, Snapshot { project: project.clone() });
    edits.undo.push(Step { name, snap: before });
    edits.redo.clear();
    edits.committed_key = k;
}

/// CLOSE AN EDIT OF THE SKETCH where only the journal and the document are at hand (an `Editing`): the named step goes
/// onto the undo stack, or nothing does when the document did not change. The rebuild is the frame's - the edit has
/// already marked it. Without a boundary of its own an edit of the sketch was laid by the frame's safety net as a step
/// called "Edit", in every language.
pub fn close_edit(edits: &mut Edits, project: &Project) {
    edits.depth = edits.depth.saturating_sub(1);
    if edits.depth > 0 {
        return; // a nested operation: the outer one will sum it up
    }
    let Some((name, before)) = edits.open.take() else { return };
    if doc_key(&before.project) == doc_key(project) {
        return;
    }
    edits.undo.push(Step { name, snap: before });
    edits.redo.clear();
    edits.baseline = Snapshot { project: project.clone() };
    edits.committed_key = doc_key(project);
}

/// A REBUILD FROM THE THREAD HAS LANDED ON A DOCUMENT NOBODY TOUCHED SINCE: the state the next undo comes back to is
/// the one built. The state was laid down when the step closed, before the thread finished, and brought back from
/// there it carried the faces as they were before the rebuild named them - a seam face undone and done again came
/// back under the kernel's raw number 11 instead of its name.
pub fn settle_baseline(edits: &mut Edits, project: &Project) {
    if edits.open.is_none() && edits.committed_key == doc_key(project) {
        edits.baseline = Snapshot { project: project.clone() };
    }
}

/// Close a lasting operation: a named step goes onto the undo stack.
pub fn commit_edit(rc: &mut RebuildCtx) {
    rc.edits.depth = rc.edits.depth.saturating_sub(1);
    if rc.edits.depth > 0 {
        return; // a nested operation: the outer one will sum it up
    }
    let Some((name, before)) = rc.edits.open.take() else { return };
    rc.edits.open_kind = OpenEdit::Working;
    rc.edits.undo.push(Step { name, snap: before });
    rc.regen.pending = false;
    rebuild_if_dirty(rc); // ONE point: the scheduler decides for itself whether to compute
    if rc.edits.undo.len() > rc.set.undo_cap.max(1) {
        rc.edits.undo.remove(0);
    }
    rc.edits.redo.clear();
    rc.edits.baseline = Snapshot { project: rc.project.clone() };
    rc.edits.committed_key = doc_key(rc.project);
}

/// THE PATH OF CONTEXTS DOWN TO COMPONENT `cid`: the root, the chain of its ancestors, the component itself - what
/// `active_path` holds while working inside it. Built from the parents, at most 256 deep.
pub fn context_path_to(project: &Project, cid: Id) -> Vec<Id> {
    let root = project.root;
    let (mut chain, mut cur) = (vec![cid], cid);
    for _ in 0..256 {
        if cur == root {
            break;
        }
        match project.components.iter().find(|c| c.id == cur).and_then(|c| c.parent) {
            Some(p) => {
                chain.push(p);
                cur = p;
            }
            None => break,
        }
    }
    if chain.last() != Some(&root) {
        chain.push(root);
    }
    chain.reverse();
    chain
}

/// UNDO AND REDO BY KEY: Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y - (undo, redo). A text field keeps them for its own text,
/// except the field a new shape opens for its size: the shape is already in the document, and Ctrl+Z right after
/// drawing a circle is meant for the circle. Such a field goes with the undo - its shape is gone.
pub fn undo_keys(ctx: &egui::Context, place: &mut Placing, inline: &mut InlineEdit, in_sketch: bool) -> (bool, bool) {
    // the field of a new shape stands in the sketch being edited: out of it, a field in focus is some other one (the
    // search of the tree) and keeps its keys - a shape left sizing when the sketch was finished took them from it
    let sizing = in_sketch && (place.active() || place.dim.is_some() || inline.dim().is_some() || inline.circle().is_some());
    if ctx.egui_wants_keyboard_input() && !sizing {
        return (false, false);
    }
    let (undo, redo) = ctx.input(|i| {
        let (cmd, z, y) = (i.modifiers.command, i.key_pressed(egui::Key::Z), i.key_pressed(egui::Key::Y));
        (cmd && z && !i.modifiers.shift, cmd && ((z && i.modifiers.shift) || y))
    });
    if undo && sizing {
        place.clear();
        inline.clear();
    }
    (undo, redo)
}

pub fn begin_edit(edits: &mut Edits, project: &Project, name: impl Into<String>) {
    if edits.open.is_none() {
        let name = name.into();
        // THE TRAIL FOR A CRASH REPORT IS WRITTEN HERE, NOT AT THE COMMIT. A crash happens in the
        // middle of an operation, so a trail of committed steps is missing exactly the one that
        // killed the program - the only one worth having.
        note_step(&name);
        edits.open = Some((name, Snapshot { project: project.clone() }));
    }
    edits.depth += 1;
}

/// OPEN A LASTING OPERATION (one that lives across frames: a sketch editing session, a drag).
/// The `Edit` guard holds `&mut App` and suits only a short operation inside a single call; a session
/// is closed explicitly, by `commit_edit` or `abort_edit`.
/// AN UNDO STEP OVER A CONTEXT, not over the whole application. `App::edit` exists for the frame; a panel
/// that already holds a rebuild context has no application to ask.
pub fn edit_over<'a>(rc: RebuildCtx<'a>, name: impl Into<String>) -> Edit<'a> {
    begin_edit(rc.edits, rc.project, name);
    Edit { rc, done: false }
}

/// Answers the context path the window stands at afterwards: the component the restored document was worked in, when
/// it is still there - a step is taken back and put again where it was made.
#[must_use]
pub fn restore(rc: &mut RebuildCtx, snap: Snapshot) -> Vec<Id> {
    // A snapshot carries THE MESHES but not the live B-rep (`Shape` is not cloneable). `shapes` used
    // to be left over from the undone state: old geometry on screen, new geometry in the kernel, and
    // the NEXT operation built on the undone shape. Silently, because nodes are not dirty after an
    // undo. What gets rebuilt is exactly the bodies whose RECIPE differs between the states (not a
    // forced pass over the whole document — on an assembly of a thousand imports that is tens of
    // seconds).
    let mut changed = rc.project.changed_bodies_vs(&snap.project);
    // A PIECE OF A MESH IS ITS OWN GEOMETRY: the snapshot brings its mesh and faces back as they were, and there is
    // nothing to rebuild it from. Counted as changed, its node is marked and a rebuild of nothing is started.
    let pieces: Vec<Id> = changed.iter().copied().filter(|&b| snap.project.timeline.iter().any(|n| n.kind.owns_body(b) && !n.kind.waits_for_brep())).collect();
    changed.retain(|b| !pieces.contains(b));
    let mut restored = snap.project;
    // THE PARAMETERS THAT DIFFER between the two states change whatever reads them, though no recipe changed: the
    // node still says "k". A snapshot taken when an edit closed holds the new expression beside the old value and the
    // old geometry - both are counted by the rebuild, which comes after - and restored as it was, a chamfer driven by
    // k kept the size of the k before. Measured by the check of undo and redo after every step: k written as 3 came
    // back holding 1.5, the chamfer with it. Asked by the expression as well as the value for that reason.
    // the values counted again from the expressions first: the snapshot may hold an expression its value never caught up
    // with, the value being counted when the table's edit is applied
    let _ = restored.eval_parameters();
    let said = |p: &Project| p.parameters.iter().map(|q| (q.name.clone(), (q.expr.clone(), q.value.to_bits()))).collect::<std::collections::HashMap<_, _>>();
    let (was, now) = (said(rc.project), said(&restored));
    let moved: Vec<String> = was.keys().chain(now.keys()).filter(|k| was.get(*k) != now.get(*k)).cloned().collect();
    shelve_source_data(rc.live, rc.project, &mut restored);
    restored.take_source_data_from(rc.project); // the source bytes come from the live document
    restored.keep_ids_past(rc.project); // an id handed out once is never handed out again
                                        // the derived topology caches never went into the snapshot — bring them back from the live state
                                        // for the bodies the edit did not touch (the changed ones are rebuilt below anyway)
    let (rf, re) = (std::mem::take(&mut rc.project.regen_faces), std::mem::take(&mut rc.project.regen_edges));
    shelve_imports(rc.live, rc.project, &restored);
    *rc.project = restored;
    rc.project.regen_faces = rf;
    rc.project.regen_edges = re;
    // THE LIVE BODIES OF THE CHANGED ONES STAY until the rebuild below replaces them: every node of theirs is marked
    // dirty and built again, and one that no longer builds keeps its last good body, as a partial rebuild keeps it.
    // Reported behaviour: a sketch deleted under its extrusion left the body with 12 edges to pick; the same step
    // undone and done again had none - the body thrown away here, and the red node built nothing to put back.
    // a piece's face cache comes back with it: a stale one is laid over its faces at the next change of topology
    for &b in &pieces {
        if let Some(i) = rc.project.mesh_index(b) {
            rc.live.faces.insert(b, rc.project.bodies[i].faces.clone());
        }
    }
    // every body of the node counts: a pattern of parts and a split body have several and no single `body()`, and a
    // pattern brought back by redo kept its copies without a live shape - no edges to pick on any of them
    let dirty: Vec<Id> = rc.project.timeline.iter().filter(|n| n.kind.bodies().iter().any(|b| changed.contains(b))).map(|n| n.id).collect();
    for n in &mut rc.project.timeline {
        if dirty.contains(&n.id) {
            n.dirty = true;
        }
    }
    for name in &moved {
        rc.project.mark_param_dependents_dirty_for(name);
    }
    *rc.sel = Sel::None;
    rc.regen.geom_rev = rc.regen.geom_rev.wrapping_add(1);
    // THE VIEW IS NOT TOUCHED. Restoring a snapshot is an operation on the DOCUMENT, and the point of
    // view belongs to the person: undoing an edit must not throw away the corner they had zoomed in on.
    //
    // Reported behaviour: in the sketcher, picking the measuring tools and the constraints resets the
    // camera. And so it did, by this line: a constraint button that does not fit the current selection
    // is the ordinary way of working - press the button, then pick - and it ends in `abort_edit`, that
    // is here. `view.initialized = false` made the next frame re-fit the whole sketch.
    if !changed.is_empty() {
        regenerate_all(rc); // bring the B-rep of the changed bodies up to the restored state
    }
    let worked_in = rc.project.active_component.filter(|&c| rc.project.components.iter().any(|x| x.id == c));
    worked_in.map_or_else(|| rc.active_path.clone(), |c| context_path_to(rc.project, c))
}
