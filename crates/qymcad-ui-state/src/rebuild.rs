//! THE REBUILD: the live B-rep, the kernel lent, the rebuild when the document is dirty.

use crate::Notice;
use super::*;

/// THE LIVE BODIES A REBUILD LEAVES: those the document holds a mesh for. A body whose node stopped building keeps
/// both its mesh and its live body - the last good state, for the author to repair and to measure.
///
/// IMPORTED bodies are kept even without a mesh. Their B-rep cannot be rebuilt from a recipe - only by parsing the
/// embedded STEP again (tens of seconds), so their shape is kept even while the body does not build (a rollback or a
/// suppression): move the rollback bar back, and the import is on screen at once.
pub fn keep_live_shapes(live: &mut LiveGeom, project: &Project) {
    let imports: std::collections::HashSet<Id> = project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    live.shapes.retain(|body, _| imports.contains(body) || project.mesh_index(*body).is_some());
}

/// A SKETCH OPENED FOR EDITING: the session takes it, and the key it is opened at.
pub fn open_sketch_session(ses: &mut SketchSession, project: &Project, sid: Id) {
    ses.editing = Some(sid);
    ses.opened = project.sketch_key(sid);
}

/// THE BODIES ARE REBUILT WHEN A SKETCH CLOSES, NOT WHILE IT IS OPEN - and only when it changed.
///
/// While a sketch is open the scheduler runs only what is asked for explicitly (`rebuild_if_dirty`): what an edit in
/// it marks - the dependents of a driver whose value moved, say - waits for the sketch to close, as it does in the
/// professional systems. The sketch itself is marked here, when its key differs from the one it was opened at. Reported behaviour:
/// a sketch of the filter of the examples opened and closed with nothing touched rebuilt the cuts on it and a rounding
/// of 36 edges, 5.7 s; naming a driver in it rebuilt 0 nodes for 0.8 s under the sign of a rebuild.
///
/// A project from a bundle has no live B-rep, and the feature on a marked sketch asks for its source body: the missing
/// ones are built first (`ensure_brep`), exactly those and not the whole project.
pub fn rebuild_after_sketch(rc: &mut RebuildCtx, closed: SketchSession) {
    if let Some(sid) = closed.editing.filter(|sid| rc.project.sketch_key(*sid) != closed.opened) {
        ensure_brep(rc);
        rc.project.mark_sketch_dirty(sid);
    }
    rebuild_if_dirty(rc);
}

pub fn regenerate_now(rc: &mut RebuildCtx) {
    prune_dangling_features(rc.live, rc.project); // anti-ghost: on EVERY regen, orphan meshes and dangling features go
                                                  // THE REBUILD GRAPH: a parameter may have changed (including a named sketch dimension - those are in
                                                  // `param_map`) -> ONLY the features that refer to it are rebuilt. This used to mark ALL features
                                                  // carrying expressions, and it fired on EVERY rebuild: any trifle dragged the whole parametrics of
                                                  // the project through a recount.
    mark_changed_params_dirty(rc.params_seen, rc.project);
    let _gate = qymcad_kernel::kernel_gate();
    // HOW MANY CORES THE KERNEL MAY TAKE, said where a rebuild starts rather than remembered somewhere.
    //
    // One atomic store, and it cannot fall out of step with the setting. One core means single-threaded, which
    // is the switch a person reaches for when a parallel pass is suspected of lying.
    qymcad_kernel::set_parallel(rc.set.kernel_threads != 1, rc.set.kernel_threads);
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k(), ..Default::default() };
    // HOW OFTEN THE GEOMETRIC FALLBACK FIRED is counted AROUND the rebuild. This is an event of a
    // different kind from rebinding a reference: there a name was found and moved, here no name was
    // found at all and the element was identified BY PLACE, by resemblance. Staying silent about it is
    // not allowed - that is exactly how a reference lands on a neighbouring face, and it is discovered
    // three operations later.
    let snaps_before = rc.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
    let report = rc.project.regenerate(&kernel);
    let snaps = rc.project.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed).saturating_sub(snaps_before);
    rc.live.shapes = kernel.shapes.into_inner();
    settle_params_seen(rc.params_seen, rc.project); // the rebuild HAPPENED - only now are the values "seen"
                                                    // the cache of live B-rep must match what the project actually holds. A regen REMOVES the mesh of a
                                                    // body that stopped building (a rollback, a suppression, a cascade of an error) - while its former
                                                    // shape stayed in the cache as a ghost: a foreign volume in the counts, wasted memory, and the risk
                                                    // of handing dead geometry outside.
    keep_live_shapes(rc.live, rc.project);
    for (body, faces) in report.built {
        set_body_faces(rc.live, rc.project, body, faces); // both into the index-parallel `faces` and into the cache by body Id
    }
    // REBINDS OF GEOMETRY REFERENCES ARE VISIBLE. A lost reference used to latch onto a "similar" face
    // again on every rebuild, silently - now it is an event that gets announced.
    rc.regen.rebinds = report.rebinds.clone();
    // WHAT DID NOT BUILD IS SAID IN RED, the first node and how many, as the rebuild in the background says it
    let left = crate::rebuild_left(rc.project);
    if let Some(said) = left.clone().filter(|n| n.tone == crate::Tone::Error) {
        *rc.status = said;
    } else if let Some((_, e)) = report.errors.first() {
        // the error text is in the person's language: the core returned a CODE, the words are found here
        *rc.status = Notice::error(format!("{} {}", qymcad_i18n::tr("status-rebuild"), qymcad_i18n::error_words::error_text(e)));
    } else if !report.rebinds.is_empty() {
        let first = &report.rebinds[0];
        *rc.status = Notice::warning(if report.rebinds.len() == 1 {
            qymcad_i18n::tr1("io-rebound-one", "what", &first.what)
        } else {
            qymcad_i18n::tr2("io-rebound-many", "n", &report.rebinds.len().to_string(), "first", &first.what)
        });
    } else if snaps > 0 {
        *rc.status = Notice::warning(qymcad_i18n::tr1("io-rebound-by-place", "n", &snaps.to_string()));
    } else if let Some(said) = left {
        *rc.status = said; // built, but some of it only in part
    }
    invalidate(rc.regen);
}

pub fn ensure_brep(rc: &mut RebuildCtx) {
    if rc.live.ready || rc.live.wait.is_some() {
        return; // done, or an attempt is ALREADY running - a second launch would make the overlay blink
    }
    // THE CACHE OF LIVE SHAPES HAS GONE TO THE THREAD - THERE IS NOTHING TO JUDGE BY.
    //
    // A rebuild TAKES the cache for the duration of the computation (`mem::take` in `spawn_regen`):
    // otherwise the thread has nothing to work with. All that time the application has ZERO live shapes
    // - not because there are none, but because the thread holds them. The preparation, asked at that
    // moment, honestly answered that not a single body was up and ordered a rebuild on top of the
    // running one; the answer depended on whether a computation was in flight, and the "we have already
    // tried this" guard missed every other time.
    //
    // A MEASUREMENT IN A LIVE WINDOW, a document with 138 imports: "136 live shapes" and "0 live shapes"
    // alternated endlessly and the rebuild window blinked without stopping. Judging by what you do not
    // hold is not allowed - while the computation runs, the preparation stays silent.
    if rc.regen.regen_running() {
        return;
    }
    // building the B-rep is DERIVED work: the model does not change from it, the caches do. If the
    // project was clean (just opened) it must stay clean, otherwise closing asks whether to save after
    // nothing at all has been touched.
    let was_clean = !is_dirty(rc);
    // retrying on EVERY call is pointless - if nothing changed since the last attempt, the result will
    // be the same. But declaring the cache ready is not allowed either: that would be a lie, and because
    // of it operations on imports silently failed with "body not built" right up to a restart.
    if rc.live.tried_rev == Some(brep_input_key(rc.live, rc.project)) {
        return;
    }
    rc.live.tried_rev = Some(brep_input_key(rc.live, rc.project)); // guards against re-entering from inside a rebuild
                                                                   // ONLY the nodes whose bodies have no live B-rep are rebuilt (their sources - also without shapes -
                                                                   // land on the same list, so the chain comes whole). A forced regen of the entire project here would
                                                                   // cost a full re-tessellation of 1170 imports for nothing.
                                                                   // only what waits for a B-rep: a mesh piece never has one (see `FeatureKind::waits_for_brep`)
    let missing: Vec<Id> =
        rc.project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body().map(|b| (n.id, b))).filter(|(_, b)| !rc.live.shapes.contains_key(b)).map(|(id, _)| id).collect();
    if missing.is_empty() {
        rc.live.ready = true;
        fill_model_edges_for_anchors(rc); // see above: otherwise edge anchors are dead after opening
        if was_clean {
            rc.edits.saved_key = edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path });
        }
        return;
    }
    *rc.status = Notice::info(qymcad_i18n::tr1("io-brep-prepare", "n", &missing.len().to_string()));
    for n in &mut rc.project.timeline {
        if missing.contains(&n.id) {
            n.dirty = true;
        }
    }
    let deferred = rc.regen.ui_running; // in a live window the rebuild goes to a thread and comes back later
    mark_dirty_for_rebuild(rc); // the document is marked; the planner does the counting
    if deferred {
        // Reported: pressing "create sketch" made the rebuild window blink back and forth. The rebuild
        // HAS NOT RUN YET - it is queued. Marking "tried at revision N" now is not allowed: the rebuild,
        // once it arrives, moves the revision, the mark stops matching, and the next frame (and picking
        // a sketch plane calls us on EVERY frame) starts it all over - an endless loop. The outcome is
        // drawn by `settle_brep_wait` when the result gets here.
        rc.live.wait = Some(was_clean);
        return;
    }
    settle_brep_wait(rc, was_clean);
}

/// Sum up the B-rep preparation ON THE FACT of a rebuild having happened (synchronous or arrived
/// from a thread). `was_clean` says whether the document was clean before the preparation.
pub fn settle_brep_wait(rc: &mut RebuildCtx, was_clean: bool) {
    rc.live.tried_rev = Some(brep_input_key(rc.live, rc.project));
    // the flag follows THE FACT. If a body is left without a B-rep (an import waiting to be restored
    // from the embedded STEP), the cache is NOT ready, and the next attempt will happen once new data
    // appears.
    rc.live.ready = rc.project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body()).all(|b| rc.live.shapes.contains_key(&b));
    fill_model_edges_for_anchors(rc); // anchors on edges are dead after opening otherwise (see io_jobs)
    if was_clean {
        rc.edits.saved_key = edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path });
        // rebuilding the cache is not an edit made by hand
    }
}

/// BRING UP the cache of live B-rep when the project was opened from a bundle WITHOUT a rebuild (the
/// geometry is shown at once, but the `Shape`s of feature bodies do not exist yet). The first operation
/// that needs a real B-rep pays for the rebuild once - the same idea as resolving a lightweight
/// document. Until a project has been opened the flag is set and this is a pure no-op.
///
/// EDGES FOR ANCHORS GO INTO THE MODEL as soon as the live B-rep is up.
///
/// The first fix here was incomplete: joints on edges worked in the session where they were made and
/// died on the next OPENING - "anchor lost", travel 0.000 mm. The two sources of edges were reconciled
/// only at the moment an anchor is PLACED (`ensure_model_edges` while picking) and not reconciled when
/// the document is opened: `regen_edges` is filled by the post-pass of a rebuild, and imported bodies
/// are not rebuilt at all - they have no timeline nodes.
///
/// The core is asked only about the bodies that EDGE anchors and VERTEX anchors refer to: there are a
/// handful of those, and going through all hundred-odd would be pointless.
pub fn fill_model_edges_for_anchors(rc: &mut RebuildCtx) {
    use qymcad_core::feature::AnchorRef;
    let want: Vec<Id> = rc
        .project
        .connectors
        .iter()
        .filter_map(|c| match c.anchor {
            AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => Some(b),
            _ => None,
        })
        .filter(|b| !rc.project.regen_edges.contains_key(b) && rc.live.shapes.contains_key(b))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for b in want {
        ensure_model_edges(rc, b);
    }
}

/// The imported bodies of `p`.
pub(crate) fn import_bodies(p: &Project) -> std::collections::HashSet<Id> {
    p.timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Import { body, .. } => Some(body),
            _ => None,
        })
        .collect()
}

/// Between two states of the document: the live shapes of the imports leaving it go to the shelf, those of the
/// imports coming back are taken off it, so the rebuild after the restore tessellates them with named faces.
pub(crate) fn shelve_imports(live: &mut LiveGeom, was: &Project, now: &Project) {
    let (before, after) = (import_bodies(was), import_bodies(now));
    for b in before.difference(&after) {
        if let Some(s) = live.shapes.remove(b) {
            live.shelved.insert(*b, s);
        }
    }
    for b in &after {
        if !live.shapes.contains_key(b) {
            if let Some(s) = live.shelved.remove(b) {
                live.shapes.insert(*b, s);
            }
        }
    }
}

/// Between two states of the document: the bytes of the sources leaving it go to the shelf, those of the sources
/// coming back without bytes are taken off it.
pub(crate) fn shelve_source_data(live: &mut LiveGeom, was: &mut Project, now: &mut Project) {
    for src in &mut was.sources {
        if !src.data.is_empty() && !now.sources.iter().any(|n| n.id == src.id) {
            live.shelved_sources.insert(src.id, std::mem::take(&mut src.data));
        }
    }
    for src in &mut now.sources {
        if src.data.is_empty() {
            if let Some(d) = live.shelved_sources.remove(&src.id) {
                src.data = d;
            }
        }
    }
}

/// After a component's placement changes: with external references present, rebuild the consumers
/// (top-down associativity — the source face travelled with the part); otherwise simply invalidate the cache.
pub fn after_placement_change(rc: &mut RebuildCtx) {
    // a mirrored part is placed FREELY by the gizmo, so moving either the source or the mirror needs no
    // regeneration of the shape (the plane is fixed in the source's local frame)
    if !rc.project.external_refs.is_empty() {
        rc.project.mark_external_consumers_dirty(); // the consumers' `sketch_frame` has moved, so rebuild
        mark_dirty_for_rebuild(rc); // the document is marked; the scheduler does the computing
    } else {
        invalidate(rc.regen);
    }
}

pub fn regenerate_all(rc: &mut RebuildCtx) {
    // ONE REBUILD PER ACTION. Inside an operation it is asked for several times (the command touches
    // the sketch, then the timeline, then the caches) — but the action is one, so the rebuild is one
    // too: the request is accumulated and carried out when the operation closes. Every request used
    // to recompute the model from scratch.
    if rc.edits.open.is_some() {
        rc.regen.pending = true;
        return;
    }
    rc.regen.computing_depth = rc.edits.undo.len();
    if rc.regen.ui_running {
        rc.regen.wanted = true;
        return;
    }
    regenerate_now(rc);
    rc.regen.computed_depth = rc.regen.computing_depth;
}

/// ONE STEP BACK (`redo` false) OR FORWARD through the history; answers the context path to stand at, or None when
/// there is no step. A command open at that moment keeps going from the state brought back: its snapshot was taken
/// before the step, and restoring it when the command was put down brought the undone step back. Reported behaviour
/// found by the check: with a mate tool holding its picks, Ctrl+Z took "Extrusion" off the list and the extrusion
/// stayed in the document.
pub fn step_through_history(rc: &mut RebuildCtx, redo: bool) -> Option<Vec<Id>> {
    // A QUESTION WAITING FOR ITS ANSWER IS ONE STEP NOT YET TAKEN: Ctrl+Z takes it back - its window cancels it on the
    // next frame, as its Cancel does - and nothing under it. Reported behaviour: with an STL asking about its scale, Ctrl+Z
    // undid the step before the import, took the person to the part of that step, and left the window open.
    if rc.edits.open.is_some() && rc.edits.open_kind != OpenEdit::Working {
        if !redo {
            rc.edits.open_kind = OpenEdit::AskingDropped;
        }
        return None;
    }
    let step = if redo { rc.edits.redo.pop() } else { rc.edits.undo.pop() }?;
    // an operation still open - a command, a sketch session - now starts from the state brought back: put down, it
    // returns there. Letting it go instead broke the sketch session that owned it, and rebuilding everything after an
    // undo in a sketch never came to rest.
    if let Some((_, before)) = rc.edits.open.as_mut() {
        *before = step.snap.clone();
    }
    let cur = Step { name: step.name.clone(), snap: std::mem::replace(&mut rc.edits.baseline, step.snap.clone()) };
    if redo {
        rc.edits.undo.push(cur)
    } else {
        rc.edits.redo.push(cur)
    }
    let path = restore(rc, step.snap);
    rc.edits.committed_key = doc_key(rc.project);
    *rc.status = Notice::info(qymcad_i18n::tr1(if redo { "g-redone" } else { "g-undone" }, "what", &step.name));
    Some(path)
}

/// A REBUILD STOPPED BY A PERSON TAKES BACK THE EDIT IT WAS COMPUTING: a recognition cancelled leaves no recognised body
/// in the tree, as a cancelled command in a professional CAD leaves nothing. Only the one edit on top, and only where the
/// state under it had been computed - a cancelled "rebuild everything" takes back nothing. The edit goes to redo, so
/// it can be brought back. Otherwise the document stays as it is, and the rebuild does not start again by itself
/// until the document changes. The way to go on, where the edit was taken back.
pub fn rebuild_cancelled(rc: &mut RebuildCtx) -> Option<Vec<Id>> {
    let depth = rc.edits.undo.len();
    if depth > 0 && depth == rc.regen.computing_depth && rc.regen.computed_depth + 1 == depth {
        let step = rc.edits.undo.pop()?;
        let now = std::mem::replace(&mut rc.edits.baseline, step.snap.clone());
        rc.edits.redo.push(Step { name: step.name.clone(), snap: now });
        let path = restore(rc, step.snap);
        rc.edits.committed_key = doc_key(rc.project);
        rc.regen.computed_depth = rc.edits.undo.len();
        rc.regen.paused = None;
        *rc.status = Notice::warning(qymcad_i18n::tr1("io-rebuild-cancelled-undone", "what", &step.name));
        return Some(path);
    }
    rc.regen.paused = Some(rc.project.rebuild_key());
    *rc.status = Notice::warning(qymcad_i18n::tr("io-rebuild-cancelled"));
    None
}

/// THE SINGLE REBUILD SCHEDULER.
///
/// The UI does not rebuild the model — it CHANGES THE DOCUMENT. The document knows on its own that
/// it is dirty (the `dirty` flags on the timeline nodes are set by `Project` methods), and deciding
/// whether it is time to compute, and where — here or in a thread — must be one place's job. That is
/// how professional CAD works: an edit marks the tree, and the engine carries out the regeneration
/// itself.
///
/// Called from two points and from those alone: when an operation closes (an edit made by hand) and
/// once a frame (whatever was marked not by hand but by the system — a background task arriving,
/// the B-rep being loaded in).
pub fn rebuild_if_dirty(rc: &mut RebuildCtx) {
    if rc.edits.open.is_some() {
        return; // an operation is under way — its closing will sum it up (one rebuild per action)
    }
    let placement = mark_changed_params_dirty(rc.params_seen, rc.project);
    // THE REBUILD RUNNING NOW IS FOR A DOCUMENT THAT IS NO MORE: its result is thrown away on arrival, so it is asked
    // to stop rather than left to finish. Asked before anything below may return: deleting a part leaves no node
    // dirty. Reported behaviour: a part deleted while its mesh was being recognised, and the spinner went on turning
    // for as long as the recognition took (14 s on a 160k-triangle ball in a debug build), over nothing.
    if let Some(pulse) = rc.regen.busy.as_ref().and_then(|b| b.pulse.as_ref()).filter(|p| p.stamp != rc.project.rebuild_key()) {
        pulse.ask_stop();
    }
    let asked = std::mem::take(&mut rc.regen.pending);
    // AN OPEN SKETCH HOLDS WHAT IS NOT ASKED FOR: the marks wait for it to close (`rebuild_after_sketch`); an explicit
    // request - the live B-rep a projection needs, say - goes through
    if !asked && rc.sketch_ses.editing.is_some() {
        return;
    }
    // A MATE COUNTING FROM A CHANGED VALUE asks for the rebuild by itself: it has no node to mark, and only the placement
    // pass of a rebuild moves it. Reported behaviour: pa made 45 under a hinge of angle pa, the part stayed at 90.
    let asked = asked || placement;
    // THE REBUILD WAS STOPPED BY HAND — it does not start again on its own. An explicit request
    // (`asked`) clears the mark: "Rebuild everything" and any edit of the document both mean
    // "compute again".
    if rc.regen.paused == Some(rc.project.rebuild_key()) && !asked {
        return;
    }
    rc.regen.paused = None;
    if !asked && !rc.project.timeline.iter().any(|n| n.dirty) {
        rc.edits.committed_key = doc_key(rc.project); // the "dirty" marks set above are derived too
        return; // the document is clean and nobody asked — there is nothing to compute
    }
    // NOTHING HAS CHANGED SINCE THE LAST REBUILD — THERE IS NOTHING TO COMPUTE, EVEN WITH DIRTY NODES.
    //
    // A node that failed to build stays dirty DELIBERATELY: the attempt must happen again once its
    // input appears (a live B-rep after the file is opened). But the scheduler read "dirty" as
    // "compute now" — and took it on EVERY frame. Reported behaviour: the rebuild window flickers
    // wildly, twenty frames a second, saying "feature 0 of 0". A single red feature made the program
    // unusable.
    //
    // So what is asked is not the mark but THREE things at once: whether the document changed,
    // whether the set of dirty nodes changed, and whether a new live B-rep has appeared. Any one of
    // them is a reason to compute; none of them means there is nothing to repeat, the inputs are the
    // same.
    //
    // WE ASK ABOUT THE TIMELINE, NOT ABOUT THE WHOLE DOCUMENT (`rebuild_key`, not `doc_key`). The
    // full document key used to stand here, and it includes the PLACEMENT — where the components
    // stand. Driving a part along a degree of freedom moves it every frame while rebuilding no body
    // at all, and the scheduler read every frame of the drag as "the document has moved". Reported
    // behaviour: while a joint is being moved, the modal rebuild window flickers endlessly, saying
    // "parts 0 of 0". Measured along that path: eight steps of the drag, eight rebuild requests.
    let now = (rc.project.rebuild_key(), rc.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect::<Vec<_>>(), rc.live.shapes.len());
    if !asked && now == rc.regen.last {
        return;
    }
    regenerate_all(rc);
    rc.regen.last = (rc.project.rebuild_key(), rc.project.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect(), rc.live.shapes.len());
    // A NODE THAT NEEDS A LIVE B-rep IS EXACTLY WHAT "ON DEMAND" MEANS.
    //
    // After a file is opened no body has a live B-rep: the geometry comes from the bundle, and the
    // kernel is raised lazily. For nodes built from a recipe that is enough; for those that need the
    // source's B-rep (thickening a sheet, replacing a face, trimming) it is not, and they were stuck
    // on "the source has not been built" FOREVER: there was nobody to raise the B-rep, and the
    // scheduler only computes on changes. This showed up as a red node that nothing would cure.
    //
    // The demand for a B-rep is now visible from the refusal itself: it is temporary (`retryable`),
    // so asking the kernel makes sense. We ask once — after that `ensure_brep` does not repeat itself.
    if !rc.live.ready && rc.project.regen_errors.values().any(|e| e.retryable()) {
        ensure_brep(rc);
    }
    // ORDER MATTERS: the snapshot of "where we stopped" is written BEFORE the kernel is asked.
    // Otherwise it would remember nodes already marked for rebuilding, the scheduler would see no
    // changes and would not compute — that is, the very guard against endless computing would
    // suppress a legitimate repeat.
    // A REBUILD IS NOT AN EDIT MADE BY HAND. It touches the document with authority: it clears the
    // "dirty" marks, writes meshes and faces, assigns names. The guard on operation boundaries has
    // to account for that, otherwise it declares the scheduler's own work an edit made outside
    // `App::edit` — which is exactly why the application panicked on the FIRST frame: at start-up no
    // parameter has been seen yet, every dependent node was marked dirty, and the document key
    // changed before a single operation had taken place.
    rc.edits.committed_key = doc_key(rc.project);
}

/// ASK for a rebuild. In a running window it goes into a worker thread and runs behind an indicator:
/// a boolean on a thread takes seconds, and the whole interface used to freeze for that time — the
/// system showed "the application is not responding" and offered to kill it. In headless tests
/// (there is no window) the rebuild runs straight away, so the result is available on the next line.
/// Mark the model as needing a rebuild. It does NOT compute — that is done by the scheduler
/// (`rebuild_if_dirty`) when an operation closes or once a frame. Needed where an edit did not set
/// `dirty` itself (a change of context, the caches); such places should get fewer, not more.
pub fn mark_dirty_for_rebuild(rc: &mut RebuildCtx) {
    rc.regen.pending = true;
    if rc.edits.open.is_none() {
        rebuild_if_dirty(rc);
    }
}

/// Whether there are unsaved changes (the state key has drifted from the moment of saving).
pub fn is_dirty(rc: &mut RebuildCtx) -> bool {
    edit_key(&DrawCtx { cam: rc.cam, set: rc.set, scheme: rc.scheme, project: rc.project, active_path: rc.active_path }) != rc.edits.saved_key
}

/// LEND THE KERNEL a live B-rep cache. The `shapes` cache is the only owner of the `Shape`s, so
/// operations that need geometry (projecting an edge into a sketch) take it from here rather than
/// building a second cache alongside: copies drift apart silently, and the projection then comes from
/// the wrong part.
///
/// THE CLOSURE IS HANDED THE DOCUMENT, not the whole application. It used to get `&mut Self`, and that
/// alone kept the lending - and everything that calls it - inside the crate that declares `App`. Both
/// callers were checked: one ignores the argument entirely, the other reaches for `project` and nothing
/// else. So the wider loan was never used, only paid for.
pub fn with_kernel<R>(rc: &mut RebuildCtx, f: impl FnOnce(&mut qymcad_core::model::Project, &dyn qymcad_core::feature::Kernel) -> R) -> R {
    let _gate = qymcad_kernel::kernel_gate();
    // HOW MANY CORES THE KERNEL MAY TAKE, said where a rebuild starts rather than remembered somewhere.
    //
    // One atomic store, and it cannot fall out of step with the setting. One core means single-threaded, which
    // is the switch a person reaches for when a parallel pass is suspected of lying.
    qymcad_kernel::set_parallel(rc.set.kernel_threads != 1, rc.set.kernel_threads);
    let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(std::mem::take(&mut rc.live.shapes)), quality_k: rc.project.geom_quality.deflection_k(), ..Default::default() };
    let out = f(rc.project, &kernel);
    rc.live.shapes = kernel.shapes.into_inner();
    out
}

/// WHAT IS LEFT TO RAISE — the set of bodies with no live shape, as a fingerprint.
///
/// It answers "we have tried this already and nothing came of it". The preparation asks exactly one
/// question: IS THERE ANYTHING TO DO. So what must be compared is THE WORK, not the state of the
/// document around it.
///
/// THE COST OF THE PREVIOUS KEY WAS ESTABLISHED BY MEASUREMENT IN A LIVE WINDOW. It used to be the
/// picture revision, then the state of the timeline; the rebuild MOVES BOTH ITSELF (topological
/// naming rewrites the node names). The result was an endless round: the preparation asks for a
/// rebuild, the rebuild arrives with a plan of "0 nodes" and changes nothing, yet it touches the
/// document — the key differs, the guard lets it through, the preparation asks again. Reported
/// behaviour: pressing "Rebuild everything" sent the CAD into a fever of endless flickering. The
/// program printed "live shapes 0" on every round.
///
/// A set of unfinished work cannot behave that way: if no body was raised, the set is the same and
/// there is no second round; if even one was, the set changed and the attempt legitimately repeats.
pub fn brep_input_key(live: &LiveGeom, project: &Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut missing: Vec<Id> = project.timeline.iter().filter(|n| n.kind.waits_for_brep()).filter_map(|n| n.kind.body()).filter(|b| !live.shapes.contains_key(b)).collect();
    missing.sort_unstable();
    missing.hash(&mut h);
    h.finish()
}

/// Remove the ghosts (orphan meshes and dangling features) — a delegate to the tested
/// `Project::prune_dangling`; here we only clear the application's shape cache by the list of removed bodies.
pub fn prune_dangling_features(live: &mut LiveGeom, project: &mut Project) {
    for db in project.prune_dangling() {
        live.shapes.remove(&db);
    }
}

/// REMEMBER THE PARAMETER VALUES ON THE FACT OF A FINISHED REBUILD.
///
/// The snapshot is taken AFTER it, not before: named dimensions are derived from the sketch
/// geometry, and the sketches are solved by the rebuild itself (`settle_sketches`). A "before"
/// snapshot would drift from the result for no reason at all, and the next frame would declare the
/// parameter changed again.
pub fn settle_params_seen(params_seen: &mut std::collections::HashMap<String, f64>, project: &mut Project) {
    *params_seen = project.param_map();
}

/// THE BOUNDARY OF AN OPERATION ON THE DOCUMENT.
///
/// It opens under a name and closes either by a commit (the step goes onto the undo stack) or by an
/// abort (the document comes back as it was, leaving no trace). Nested operations merge into ONE: a
/// command touches the sketch, the timeline and the selection — that is one action and one undo step.
///
/// A forgotten `commit` does not lose the step: `Drop` commits. Losing an edit silently is worse than
/// recording one step too many, and this is easy to spot — the step appears under the name of the
/// operation that opened it.
pub struct Edit<'a> {
    /// THE REBUILD CONTEXT, NOT THE APPLICATION. It used to hold `&mut App`, and that single reference kept
    /// every panel opening an operation inside the crate that declares the type. It was needed for exactly
    /// one call - `apply_feat_cmd_inner` - which is now free; the rest only ever reached for the document.
    ///
    /// The nesting is counted by `commit_edit` itself (`edits.depth`), so the guard no longer carries an
    /// `outer` flag: it had already become dead (`let _ = self.outer;`) and said nothing.
    pub(crate) rc: RebuildCtx<'a>,
    pub(crate) done: bool,
}

impl<'a> Edit<'a> {
    /// THE DOCUMENT, and only it. Every user of the guard reaches for this and nothing else - checked by
    /// counting: twelve places in two files, all `ed.project()`.
    pub fn project(&mut self) -> &mut qymcad_core::model::Project {
        self.rc.project
    }

    pub(crate) fn finish(&mut self, ok: bool) {
        if self.done {
            return;
        }
        self.done = true;
        if ok {
            commit_edit(&mut self.rc);
        } else {
            // ABORT: a failed operation leaves no trace, neither in the document nor on the undo stack
            abort_edit(&mut self.rc);
        }
    }
}

impl Drop for Edit<'_> {
    fn drop(&mut self) {
        self.finish(true);
    }
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
///   document: 138 bodies, faces on all 138, EDGES ON TWO, live B-rep on all 138.
///
/// The two sources of edges are reconciled here, at the point where the anchor is created: the core is
/// asked through the same call a rebuild uses to fill them.
pub fn ensure_model_edges(rc: &mut RebuildCtx, body: Id) {
    if rc.project.regen_edges.contains_key(&body) || !rc.live.shapes.contains_key(&body) {
        return;
    }
    let edges = with_kernel(rc, |_, k| k.edges(body));
    if !edges.is_empty() {
        rc.project.regen_edges.insert(body, edges);
    }
}

pub fn set_body_faces(live: &mut LiveGeom, project: &mut Project, body: Id, faces: Vec<MeshFace>) {
    // THE BODY WAS REBUILT — THE STORED BLOB IS STALE. It is dropped here rather than in one of the
    // rebuild branches: the synchronous and the background one meet exactly at this point, and they must
    // not drift apart — a stale blob would go into the file and open as a body FROM THE PAST.
    live.blobs.remove(&body);
    if let Some(idx) = project.mesh_index(body) {
        if idx < project.bodies.len() {
            project.bodies[idx].faces = faces.clone();
        }
    }
    live.faces.insert(body, faces);
}
