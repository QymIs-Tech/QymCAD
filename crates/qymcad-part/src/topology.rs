//! WHAT CHANGES THE BODIES OF A PART AT ONCE: a piece made a part, a feature or a body deleted, the resync after it.

use qymcad_ui_state::Notice;
use super::*;

/// MAKE THE PIECE `body` A PART of its own, named `name`, as one step of undo: the tested core method, then the rebuild. The
/// status names the new part, or says why nothing was made.
pub fn piece_to_part(pc: &mut qymcad_ui_state::PartCtx, body: Id, name: String) {
    if name.trim().is_empty() {
        *pc.status = Notice::refusal(qymcad_i18n::tr("piece-part-name-empty")); // a part is given a name; nothing is made without one
        return;
    }
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("act-piece-to-part")); // THE OPERATION BOUNDARY
    match pc.project.piece_to_part(body, name) {
        Some((part, _)) => {
            let name = pc.project.components.iter().find(|c| c.id == part).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
            *pc.sel = qymcad_ui_state::Sel::None;
            resync_after_topology_change(pc);
            *pc.status = Notice::info(qymcad_i18n::tr1("status-piece-to-part", "name", &name));
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("status-piece-to-part-none")),
    }
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Delete feature `ti` (together with its body), clear the caches and rebuild.
pub fn delete_feature(pc: &mut qymcad_ui_state::PartCtx, ti: usize, dependents: bool) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-delete-feature")); // THE OPERATION BOUNDARY
    if ti >= pc.project.timeline.len() {
        return;
    }
    if pc.project.timeline[ti].kind.body().is_some() {
        // THE OPERATION through the tested core method: what stands on it is handed to its source, or - on a
        // base with no source - stays red, unless the confirmation's tick asked for it to go too. Here we only
        // clear the shape cache by the list of what was removed.
        let nid = pc.project.timeline[ti].id;
        let gone = if dependents { pc.project.delete_feature_with_dependents(nid) } else { pc.project.delete_feature_op(nid) };
        for db in gone {
            pc.live.shapes.remove(&db);
        }
    } else {
        pc.project.timeline.remove(ti); // a node with no body (a sketch or a datum in the timeline)
    }
    *pc.sel = qymcad_ui_state::Sel::None;
    resync_after_topology_change(pc);
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Delete a BODY by mesh index `mi` (whichever way the body was selected — in 3D, in the tree, in a
/// panel): take the deepest existing node of the chain and cascade forward (`delete_feature`). An
/// imported mesh with no node is removed directly. One path for every place (the panel, Del, the
/// context menu), so nothing drifts apart.
pub fn delete_body_mesh(pc: &mut qymcad_ui_state::PartCtx, mi: usize, dependents: bool) {
    match pc.project.mesh_id(mi).and_then(|b| qymcad_ui_state::lineage_delete_ti(&*pc.project, b)) {
        Some(ti) => delete_feature(pc, ti, dependents), // ask_delete-exempt: this is the executor itself, called by `execute_delete`
        None => {
            // an imported mesh with no feature in the timeline is removed directly
            pc.project.remove_mesh(mi); // the faces and the visibility go with the body — one record
            *pc.sel = qymcad_ui_state::Sel::None;
            qymcad_ui_state::invalidate(&mut *pc.regen);
        }
    }
}

/// Delete a SKETCH (one path for the tree and for the panel): the core's `delete_sketch` (cascading to
/// the bodies built on it), then clearing the editing session and the selection, then a resync
/// (regeneration plus prune).
pub fn delete_sketch_full(pc: &mut qymcad_ui_state::PartCtx, sid: Id, dependents: bool) {
    qymcad_ui_state::begin_edit(&mut *pc.edits, &*pc.project, qymcad_i18n::tr("status-delete-sketch")); // THE OPERATION BOUNDARY
    let removed = if dependents {
        pc.project.delete_sketch_with_dependents(sid)
    } else {
        pc.project.delete_sketch(sid);
        Vec::new()
    };
    if pc.sketch_ses.editing == Some(sid) {
        pc.sketch_ses.editing = None;
    }
    *pc.sel = qymcad_ui_state::Sel::None;
    resync_after_topology_change(pc);
    *pc.status = Notice::info(if removed.is_empty() { qymcad_i18n::tr("sketch-deleted") } else { qymcad_i18n::tr1("sketch-deleted-n", "n", &removed.len().to_string()) });
    qymcad_ui_state::commit_edit(&mut pc.rebuild());
}

/// Bring the application's caches (faces, visibility) into line with the current list of meshes after a
/// change of topology (deleting a sketch or a feature) and rebuild the bodies from the timeline — the
/// mesh indices may have shifted.
pub fn resync_after_topology_change(pc: &mut qymcad_ui_state::PartCtx) {
    // The faces are RE-HUNG by body Id rather than cleared. A `vec![Vec::new(); ...]` plus a FORCED
    // regeneration used to stand here (and only that regeneration filled the faces back in) — meaning
    // EVERY deletion of a node rebuilt and re-tessellated the WHOLE document. On an assembly of 1170
    // imported solids that is tens of seconds of freeze per Delete. The mesh indices shift after a
    // deletion, but body Ids are stable, so that is what they are laid out by; only what the deletion
    // actually made dirty needs rebuilding.
    rebuild_faces_from_cache(pc);
    pc.gsel.edges.clear();
    pc.edges.body = None;
    // THE TOPOLOGY CHANGED — THE DERIVED CACHES ARE STALE, whether anything was rebuilt or not.
    //
    // Reported behaviour: delete "Push face" from the tree and the body disappears from the viewport
    // until Edit -> Rebuild everything. The cause: `geom_rev` (the key of every derived cache) only
    // ticks inside a rebuild that actually happened, and deleting a leaf modifier leaves no dirty node
    // at all — there is nothing to compute, and the scheduler honestly does nothing. The
    // `qymcad_ui_state::consumed_bodies` cache meanwhile stays as it was on the previous frame, where the source body was
    // still consumed by the deleted feature — and `qymcad_ui_state::body_shown` hides the ONLY remaining body. Bodies
    // would vanish the same way after deleting any modifying feature (remove face, split body, chamfer,
    // shell).
    //
    // So the counter is advanced HERE: a change of topology is itself the event "the derived data is
    // invalid", and it need not coincide with a rebuild of the geometry.
    qymcad_ui_state::invalidate(&mut *pc.regen);
    qymcad_ui_state::regenerate_all(&mut pc.rebuild()); // only the dirty nodes (the deletion cascade has already marked them)
    qymcad_ui_state::detect_missing_faces(&mut *pc.live, &mut *pc.project); // mesh-based detection ONLY for raw meshes with no B-rep
    pc.view.initialized = false;
}

/// Lay the faces out of the `faces_by_body` cache onto the CURRENT mesh indices. The cache is keyed by
/// body Id and so survives the deletions and reorderings that shift the indices — the faces used to be
/// restored after a change of topology only by a full forced regeneration.
pub fn rebuild_faces_from_cache(pc: &mut qymcad_ui_state::PartCtx) {
    let live: std::collections::HashSet<Id> = pc.project.bodies.iter().map(|b| b.id).collect();
    pc.live.faces.retain(|b, _| live.contains(b));
    // THE FACES COME BACK BY ID, one call each: the model decides what a body's faces are, the
    // cache only remembers them.
    let restore: Vec<(qymcad_core::model::Id, Vec<qymcad_core::geom::MeshFace>)> = pc.project.bodies.iter().filter_map(|b| pc.live.faces.get(&b.id).map(|f| (b.id, f.clone()))).collect();
    for (id, faces) in restore {
        pc.project.set_body_faces(id, faces);
    }
}

/// MAKE THE MIRRORED COPY of the part in hand about the plane taken for it: one step of undo, the tool put down. The plane
/// was clicked in the frame of the context shown, and the copy is made in the world.
/// THE PART A WAITING MIRRORED COPY TAKES: the part of the body clicked (`mesh`, an index of `project.bodies`).
pub fn mirror_part_take(pc: &mut qymcad_ui_state::PartCtx, mesh: Option<usize>) {
    let comp = mesh.and_then(|mi| pc.project.mesh_id(mi)).and_then(|b| pc.project.body_owner(b)).filter(|c| *c != pc.project.root);
    match comp {
        Some(c) => {
            (pc.mirror.part, pc.mirror.waiting) = (Some(c), false);
            *pc.status = Notice::info(qymcad_i18n::tr("tb-mirror-pick-plane"));
        }
        None => *pc.status = Notice::info(qymcad_i18n::tr("tb-mirror-pick-part")),
    }
}

pub fn mirror_part_apply(
    mirror: &mut qymcad_ui_state::MirrorParams,
    project: &mut qymcad_core::model::Project,
    edits: &mut qymcad_ui_state::Edits,
    regen: &mut qymcad_ui_state::Rebuilding,
    active_path: &[qymcad_core::model::Id],
    status: &mut qymcad_ui_state::Notice,
) {
    let (Some(src), Some((o, n))) = (mirror.part, mirror.at) else { return };
    qymcad_ui_state::begin_edit(edits, project, qymcad_i18n::tr("mp-title"));
    let cwt = project.world_transform(qymcad_ui_state::current_ctx_id(active_path, project));
    let (wo, wn) = (qymcad_core::feature::apply12(&cwt, o), qymcad_core::feature::apply12_dir(&cwt, n));
    let cnt = project.add_mirror_component(src, wo, wn).len();
    mirror.drop_part();
    regen.pending = true; // the scheduler builds the copy once the operation is closed
    *status = Notice::info(qymcad_i18n::tr1("vp-mirror-created", "n", &cnt.to_string()));
    qymcad_ui_state::close_edit(edits, project);
}
