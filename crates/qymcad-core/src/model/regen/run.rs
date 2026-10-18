//! THE REBUILD RUN: what a node needs, the waves, the plan, the watched rebuild.

use super::*;

impl Project {
    pub fn regenerate(&mut self, kernel: &dyn crate::feature::Kernel) -> crate::feature::RegenReport {
        self.regenerate_watched(kernel, &crate::feature::NoWatch)
    }

    /// DOES THIS NODE HAVE TO BE REBUILT, given what is dirty so far.
    ///
    /// Asked in three places - the plan, the waves and the rebuild itself - and the three had grown their own
    /// copies of the answer. A copy that forgets one of the reasons (a moved sketch plane, the live input of a
    /// mirrored part) does not fail: it silently rebuilds less than it should, and the document comes out
    /// almost right.
    pub(crate) fn node_needs(&self, i: usize, dirty: &std::collections::HashSet<Id>) -> bool {
        let Some(nd) = self.timeline.get(i) else { return false };
        let self_dirty = nd.dirty || self.regen_errors.get(&nd.id).is_some_and(|e| !e.retryable());
        self_dirty || self.node_reads_any(i, dirty)
    }

    /// DOES NODE `i` READ ANYTHING IN `set` - its inputs, the body under its sketch, or the live source of a
    /// mirrored or instanced part.
    ///
    /// One answer for two questions: "is its input dirty" (`node_needs`) and "is its input still to be rebuilt"
    /// (`ready_from`). The second used to be a copy of the first that had lost the part mirror and the part
    /// instance on the way.
    pub(crate) fn node_reads_any(&self, i: usize, set: &std::collections::HashSet<Id>) -> bool {
        let Some(nd) = self.timeline.get(i) else { return false };
        let kind = &nd.kind;
        kind.inputs().iter().any(|id| set.contains(id))
            || kind.inputs().iter().any(|&inp| self.sketch_plane_body(inp).is_some_and(|pb| set.contains(&pb)))
            || matches!(kind, FeatureKind::MirrorPart { src_comp, .. } if self.active_body_before(*src_comp, i).is_some_and(|b| set.contains(&b)))
            || matches!(kind, FeatureKind::PartInstance { .. } | FeatureKind::ComponentPattern { .. })
                && kind.copy_source().and_then(|s| self.active_body_before(s, i)).is_some_and(|b| set.contains(&b))
    }

    /// THE NODES OF A REBUILD LAID OUT IN WAVES: those that can be computed at the same time stand together.
    ///
    /// The first wave holds the nodes whose inputs nothing in this rebuild produces; the second, those that
    /// depend only on the first, and so on. Nothing here touches geometry: it is arithmetic over the graph the
    /// timeline already carries (`kind.inputs()`), and it answers whether there is anything to compute side by
    /// side at all.
    ///
    /// The order within a wave is the order of the timeline, and the union of the waves is exactly the set a
    /// sequential pass would rebuild: the waves say WHEN, never WHETHER.
    pub fn rebuild_waves(&self) -> Vec<Vec<Id>> {
        let plan = self.regen_plan();
        let want: std::collections::HashSet<Id> = plan.nodes.iter().copied().collect();
        // what each output of this rebuild is ready after: the wave of the node that made it
        let mut ready_after: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
        let mut waves: Vec<Vec<Id>> = Vec::new();
        for (i, nd) in self.timeline.iter().enumerate() {
            if !want.contains(&nd.id) {
                continue;
            }
            // THE INPUTS THAT THIS REBUILD ITSELF PRODUCES. An input made outside it (a body nobody touches)
            // is ready before the first wave and puts no one in a queue.
            let mut after = 0usize;
            let note = |id: Id, after: &mut usize| {
                if let Some(w) = ready_after.get(&id) {
                    *after = (*after).max(*w + 1);
                }
            };
            for id in nd.kind.inputs() {
                note(id, &mut after);
                // a sketch drawn on a face of a body waits for that body, and the timeline does not say so
                // through `inputs`
                if let Some(pb) = self.sketch_plane_body(id) {
                    note(pb, &mut after);
                }
            }
            // the dynamic inputs: a mirrored part and a pattern instance follow the ACTIVE body of their
            // source, which is not in `inputs` either
            if let Some(src_comp) = nd.kind.copy_source() {
                if let Some(b) = self.active_body_before(src_comp, i) {
                    note(b, &mut after);
                }
            }
            if waves.len() <= after {
                waves.resize(after + 1, Vec::new());
            }
            waves[after].push(nd.id);
            for out in nd.kind.declares() {
                ready_after.insert(out, after);
            }
        }
        waves
    }

    /// What a rebuild will actually touch, without a single kernel call.
    ///
    /// The progress counter used to show the position in the timeline rather than the work, so cutting one hole
    /// in one part reported rebuilding the whole project. Untouched nodes are skipped during the rebuild (the
    /// `needs` condition below), but that is invisible from outside, and "walked past 25 nodes" cannot be told
    /// from "rebuilt 25 nodes".
    ///
    /// Here the same estimate is computed in advance and for free: a dependency walk over the timeline with no
    /// geometry involved. The answer is needed three times — for an honest counter, for choosing between a modal
    /// window and a status line, and for measurement in tests.
    ///
    /// The estimate deliberately errs on the high side: missing a node that then rebuilds is worse than naming
    /// one too many, so it analyses neither suppressed chains nor the rollback bar.
    ///
    /// The property that has to hold: the set of bodies actually rebuilt is a subset of this estimate.
    pub fn regen_plan(&self) -> RegenPlan {
        let mut dirty: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut plan = RegenPlan::default();
        let limit = self.rollback.unwrap_or(usize::MAX);
        for (i, nd) in self.timeline.iter().enumerate() {
            if i >= limit {
                break;
            }
            let kind = &nd.kind;
            // The same reasons as `needs` inside the rebuild itself: the node's own dirty flag, an
            // unrecoverable error from last time, a dirty input, a moved sketch base face, and the dynamic
            // inputs of a mirrored part and a pattern instance.
            let needs = self.node_needs(i, &dirty);
            if !needs || nd.suppressed {
                continue;
            }
            plan.nodes.push(nd.id);
            // Heavy work is named explicitly. A thread takes seconds to cut and looks indistinguishable from
            // nothing happening, so it has to be reported even when a single node rebuilds.
            if matches!(kind, FeatureKind::Thread { .. }) {
                plan.heavy = true;
            }
            for out in kind.bodies() {
                dirty.insert(out);
            }
            if let FeatureKind::Plane { plane } = kind {
                dirty.insert(*plane);
            }
            if let FeatureKind::DatumAxis { axis } = kind {
                dirty.insert(*axis);
            }
            // a sketch rebuilt carries on to what stands on it, as in the rebuild itself; left out, a parameter added
            // on the filter of the examples planned 2 of 23 nodes and rebuilt the cuts and a rounding of 36 edges
            // for 7.8 s as "a quiet rebuild of 2 nodes"
            if let FeatureKind::Sketch { sketch } = kind {
                dirty.insert(*sketch);
            }
        }
        plan.total = self.timeline.len();
        plan
    }

    /// A rebuild that reports progress and obeys cancellation; see [`crate::feature::RegenWatch`].
    ///
    /// A separate name rather than an extra parameter on `regenerate`: the observer is needed by exactly one
    /// caller (the application's background rebuild), and threading it through a hundred and fifty call sites
    /// would mean paying for cancellation where there is nobody to cancel.
    pub fn regenerate_watched(&mut self, kernel: &dyn crate::feature::Kernel, watch: &dyn crate::feature::RegenWatch) -> crate::feature::RegenReport {
        // An error without a node is not an error. A node can be deleted from anywhere, and an error record
        // that outlives the deletion shows a failure with nothing behind it in the tree. Cleared here rather
        // than only in the deletion paths: one place nothing can bypass.
        self.regen_errors.retain(|id, _| self.timeline.iter().any(|n| n.id == *id));
        use crate::feature::{FeatureKind, RegenReport};
        self.settle_sketches(); // A body is never built from an unsolved sketch (see the method).
        let mut report = RegenReport::default();
        // The faces from before the rebuild bridge the old name to the new one. References that store a face
        // number as a bare integer carry no fingerprint of their own, so without this there is nothing to repair
        // them against: the old name is gone and what it denoted is unknown. Here it is known — the old id maps
        // to its centre and normal from the previous build, and the same face is then looked up among the new
        // ones.
        //
        // For a file that has just been opened the rebuild cache is empty (it is not serialised), so the faces
        // come from the bodies themselves: they live in the bundle next to the mesh and carry exactly those old
        // names.
        let mut faces_before: std::collections::HashMap<Id, Vec<crate::geom::MeshFace>> = self.regen_faces.clone();
        for b in &self.bodies {
            faces_before.entry(b.id).or_insert_with(|| b.faces.clone());
        }
        self.rebind_lost_face_refs(&mut report); // Lost references are repaired once, and visibly.
        let mut dirty: std::collections::HashSet<Id> = std::collections::HashSet::new();
        // Snapshot for parametric feature dimensions (expressions over global parameters).
        let vars = self.param_map();
        let feat_dims = self.feat_dims.clone();
        let limit = self.rollback.unwrap_or(usize::MAX); // Rollback bar: build only the first N nodes.
                                                         // Component patterns come before bodies: the copies have to be in place by the time their bodies are
                                                         // built and the mates are solved. Otherwise the first frame after an edit shows the copies in their old
                                                         // positions and the change only becomes visible on the second rebuild.
        self.resolve_comp_patterns();
        let mut unbuilt: std::collections::HashSet<Id> = std::collections::HashSet::new();
        // the part of `unbuilt` that is so because a node above FAILED, not because it was suppressed: what stands
        // on it is red with the reason, while what stands on a suppressed node is quietly left out with it
        let mut broken: std::collections::HashSet<Id> = std::collections::HashSet::new();
        // every body some node makes or an import brings: a source outside it was deleted with its node
        let made: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).chain(self.imported_bodies.iter().copied()).collect();
        let mut emap: EdgeRenames = EdgeRenames::new();
        // The counter counts work, not steps. Reporting "node i of the whole timeline" always ends at the full
        // count however many nodes were skipped, which reads as the whole project being rebuilt — reasonably so,
        // there being no other source of information.
        // THE PER-NODE CONTEXT, assembled the same way for every branch below.
        //
        // A macro and not a function because it borrows six locals of this pass at once; written out as a
        // call that would be seven arguments repeated forty times, which is the very thing the branches were
        // split up to avoid.
        // The node is named at the call site: it is a local of the loop, and a macro sees only what was in
        // scope where it was written.
        macro_rules! pass {
            ($node:expr) => {
                pass!($node, kernel)
            };
            // a node computed by a worker is APPLIED with that worker's kernel: the names of its new faces live
            // there, not in the shared one
            ($node:expr, $k:expr) => {
                Pass { kernel: $k, vars: &vars, dims: feat_dims.get(&$node), emap: &mut emap, dirty: &mut dirty, report: &mut report, node: $node }
            };
        }
        let plan_total = self.regen_plan().nodes.len();
        // THE WAVES, AND WHAT HAS ALREADY BEEN COMPUTED FROM THEM.
        //
        // A wave is a set of nodes that do not depend on one another (see `rebuild_waves`), so their geometry
        // can be built at the same time. The document itself stays on this thread: a worker is given the bodies
        // its node needs and nothing else, and what it built comes home before anything is written down.
        let hands = kernel.workers();
        let mut computed: std::collections::HashMap<Id, Computed> = std::collections::HashMap::new();
        let mut work = 0usize;
        // THE ORDER OF THE WALK: the timeline, except that a batch just computed is written down straight away.
        //
        // Held back until the walk reached each of them, the results of a batch kept every chain standing on them
        // waiting: the first batch took the 27 heads of the scenario document, and from then on each step found
        // one node ready - batches [27, 1, 4, 1, 1, 1], and the time inside the nodes overlapped by nothing.
        // Written down at once, a head frees its chain for the next batch.
        //
        // Safe for the reason the batch itself is: a member depends on nothing standing between the node the walk
        // is on and itself (`ready_from`), and nothing standing there may read what a member makes - that would
        // be a reference to a node below it.
        let mut next_up: Vec<usize> = Vec::new();
        let mut walked: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut cursor = 0usize;
        loop {
            let i = match next_up.pop() {
                Some(j) => j,
                None => {
                    while walked.contains(&cursor) {
                        cursor += 1;
                    }
                    if cursor >= self.timeline.len() {
                        break;
                    }
                    cursor
                }
            };
            walked.insert(i);
            // Cancellation happens between nodes rather than inside a kernel operation: a boolean cannot be
            // interrupted half-way, and pretending otherwise would yield half a body. This is a boundary where
            // the document is still whole, and the incomplete result is simply discarded by the caller.
            if !watch.step(work, plan_total) {
                report.cancelled = true;
                return report;
            }
            let (node_id, kind, self_dirty, suppressed) = {
                let nd = &self.timeline[i];
                (nd.id, nd.kind.clone(), nd.dirty, nd.suppressed)
            };
            // Suppressed by the rollback bar: the body is neither built nor shown.
            if i >= limit {
                for b in kind.bodies() {
                    self.drop_body_from_view(b);
                }
                continue;
            }
            // Suppressed feature. A modifier (fillet, chamfer, combine, shell, hole, pattern, mirror, move) is
            // skipped as a no-op: its output body is a copy of the source (a pass-through with an identity
            // transform) and consumers continue on the pre-modifier body, so suppressing one feature does not
            // break the chain. A base feature (with no source), or a modifier whose source was not built,
            // cascades instead: the body goes into `unbuilt` and its consumers are not built either.
            if suppressed {
                // A suppressed split leaves the source whole. Copying it into the first piece is wrong: the
                // remaining pieces would hang around as stale geometry next to the whole body, showing the
                // material twice.
                if let FeatureKind::SplitBody { src, ref bodies, .. } = kind {
                    for &b in bodies {
                        self.drop_body_from_view(b);
                        unbuilt.insert(b);
                    }
                    if !unbuilt.contains(&src) {
                        dirty.insert(src); // The source is visible whole again, so its consumers rebuild from
                                           // it.
                    }
                    continue;
                }
                let pass = kind.consumed_body().filter(|s| !unbuilt.contains(s));
                if let (Some(src), Some(b)) = (pass, kind.body()) {
                    // The source is built, so the modifier is skipped: the output is a copy of `src` and
                    // everything downstream continues.
                    if self_dirty || dirty.contains(&src) {
                        let res = kernel.transform_body(b, src, crate::feature::PLACE_IDENTITY);
                        if self.apply_regen(Landing { node: node_id, dirty: &mut dirty, report: &mut report, kernel, emap: &mut emap }, b, res) {
                            self.timeline[i].dirty = false;
                        }
                    }
                    continue;
                }
                // Nothing to skip (a base feature, or an unbuilt source), so it cascades.
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            // Depends on a body that is not suppressed yet was not built (a cascading source, or a failure
            // above), so it is neither built nor shown.
            // Stands on a body whose node was deleted: red with that reason, and what stands on it red in turn.
            // A body read without being consumed - the source of a face copy, a patch, a trim - counts the same when its
            // node was deleted: the live body it leaves behind is not a source, and building on it made the node
            // green step by step and red rebuilt from the start.
            let deleted_input = kind.inputs().iter().any(|s| *s != 0 && !made.contains(s) && self.dead_bodies.contains(s));
            if deleted_input || kind.consumed().iter().any(|s| *s != 0 && !made.contains(s)) {
                self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                broken.extend(kind.bodies());
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            if kind.inputs().iter().any(|id| unbuilt.contains(id)) {
                if kind.inputs().iter().any(|id| broken.contains(id)) {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyNotBuilt);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyNotBuilt));
                    broken.extend(kind.bodies());
                }
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            // External and base dependency: a node whose sketch sits on a face of a body (its own, or an
            // external one through an `ExternalRef`) rebuilds when that source body rebuilds, because the face
            // frame may have moved. The source body has to come earlier in the timeline.
            //
            // The bodies of the node are captured before the match, whose branches destructure `kind`.
            let out_bodies = kind.bodies();

            // A node that failed earlier is recomputed. Otherwise it is not rebuilt on the next pass, there is
            // no fresh error, and the "do not build on a failure" cascade does not see it, so everything above
            // it builds on a ghost again. Simply marking its bodies unbuilt is not an option either: the error
            // may no longer reproduce (the input was fixed), and that would freeze a working chain forever.
            let needs = self.node_needs(i, &dirty);
            if needs {
                work += 1;
            }
            let started = needs.then(std::time::Instant::now);
            // Isolation: only a part builds bodies, and references are confined to the owning component.
            if needs && kind.body().is_some() {
                if let Some(err) = self.isolation_error(i, &kind) {
                    report.errors.push((node_id, err));
                    continue;
                }
            }
            // A VALUE WRITTEN AS AN EXPRESSION THAT NO LONGER COUNTS - a parameter it names was deleted - makes the node
            // red with the expression's own words; the body stays at its last good state. Built from the number the
            // expression last gave, the node stood green on a value nothing in the document holds any more.
            if needs {
                if let Some(e) = self.dim_expr_error(node_id, &vars) {
                    self.regen_errors.insert(node_id, e.clone());
                    report.errors.push((node_id, e));
                    continue;
                }
            }
            // Reference honesty: the base face of an input sketch was lost by persistent id, so resolution
            // falls through to the heuristic (the nearest co-directed face) and the feature may land on the
            // wrong one. The node is warned about without failing the build, so the reason a rebuild came out
            // differently is visible.
            if needs && kind.body().is_some() {
                for inp in kind.inputs() {
                    if let Some(pb) = self.sketch_face_ref_lost(inp) {
                        report.errors.push((node_id, crate::errors::CoreError::SketchFaceRefLost { sketch: inp, body: pb }));
                    }
                }
            }
            // The "old edge number to new name" map is inherited from the inputs before the build: the
            // references of the node itself (a chamfer or a fillet selects edges of its input) are translated
            // through it below.
            for out in kind.bodies() {
                inherit_edge_renames(&mut emap, out, &kind.inputs());
            }
            // Captured before the match, which moves `kind`, for the pass-through fallback when a modifier
            // fails.
            // A UNION OR AN INTERSECTION OF TWO BODIES HAS NO SINGLE SOURCE TO PASS THROUGH: a copy of the first took the
            // second out of the part - an empty intersection of two touching pieces lost half the material - so it
            // shows nothing of its own and its two inputs stay in sight (see `consumed_bodies`). A cut does pass
            // through: the base without the tool, when the tool takes nothing, is the base.
            let consumed_src = kind.consumed_body().filter(|_| !matches!(kind, FeatureKind::BodyBoolean { op: 1 | 2, .. }));
            let out_body = kind.body();
            // an addition to the body and a shell of it never make pieces of a whole body; a cut or an intersection
            // may leave islands on purpose, a pattern or a mirror lays copies, and splitting a body is what makes pieces
            let keeps_one_piece = matches!(
                kind,
                FeatureKind::Combine { op: 1, .. } | FeatureKind::Revolve { op: 1, .. } | FeatureKind::Sweep { op: 1, .. } | FeatureKind::Loft { op: 1, .. } | FeatureKind::Shell { .. }
            );
            let mut clear = true;
            // EVERYTHING THAT IS READY RIGHT NOW, COMPUTED TOGETHER.
            //
            // A wave says which nodes do not depend on one another (`rebuild_waves`), but a wave is not what
            // can be computed at THIS moment: a node of the same wave standing further down the timeline may
            // take its source from a node standing between here and there, and that source has not been
            // written into the document yet. Preparing it then resolves its references against geometry that
            // is not there - measured on the scenario document as 13 refusals and half the bodies missing.
            //
            // So the rule is readiness rather than the wave: from here on, a node joins the batch while
            // nothing it needs is made by a node in between. Preparing a parcel changes the document (it mints
            // the names of the faces to come), so preparation stays here, in timeline order, exactly as a
            // sequential pass would do it.
            if needs && hands > 1 && !suppressed && !computed.contains_key(&node_id) {
                let held: std::collections::HashSet<Id> = computed.keys().copied().collect();
                let ready = self.ready_from(i, &dirty, &held, &walked, limit);
                if ready.len() > 1 {
                    let mut parcels: Vec<Parcel> = Vec::new();
                    for &j in &ready {
                        let nid = self.timeline[j].id;
                        let kind_here = self.timeline[j].kind.clone();
                        let mut p = pass!(nid);
                        if let Some((out, job)) = self.prep_node(&mut p, &kind_here) {
                            parcels.push(Parcel { node: nid, body: out, job });
                        }
                    }
                    // TWO PARCELS THAT CANNOT GO TO SEPARATE THREADS stay here, computed in order on the shared
                    // kernel; the rest travel. Two of them clash when they want the same body (the shape cannot be
                    // in two threads at once) OR when the bodies they want SHARE a sub-shape - a face or an edge the
                    // split (or a boolean) left in both. Meshing or healing that shared sub-shape on two threads at
                    // once rewrites it twice over and the process dies, so such parcels are kept together. Bodies
                    // that share nothing still travel, so the parallelism is kept where it is safe.
                    let entangled: Vec<bool> = (0..parcels.len())
                        .map(|i| parcels.iter().enumerate().any(|(k, other)| k != i && parcels[i].job.inputs().iter().any(|a| other.job.inputs().iter().any(|b| a == b || kernel.shares(*a, *b)))))
                        .collect();
                    let (mut alone, mut here): (Vec<_>, Vec<_>) = (Vec::new(), Vec::new());
                    for (i, parcel) in parcels.into_iter().enumerate() {
                        if entangled[i] {
                            here.push(parcel);
                        } else {
                            alone.push(parcel);
                        }
                    }
                    // the parcels that travel: each with the bodies it needs, taken out of the shared kernel
                    let mut sent: Vec<Travelling> = Vec::new();
                    for parcel in alone {
                        match kernel.split_off(parcel.job.inputs()) {
                            Some(worker) => sent.push(Travelling { parcel, worker }),
                            None => here.push(parcel),
                        }
                    }
                    if !sent.is_empty() {
                        report.waves.push(sent.len());
                        // as many threads as the kernel allows, each taking its share of the parcels in turn
                        let mut lanes: Vec<Vec<Travelling>> = (0..hands.min(sent.len())).map(|_| Vec::new()).collect();
                        for (n, parcel) in sent.into_iter().enumerate() {
                            let lane = n % lanes.len();
                            lanes[lane].push(parcel);
                        }
                        let mut back: Vec<Returned> = Vec::new();
                        std::thread::scope(|scope| {
                            let mut running = Vec::new();
                            for lane in lanes {
                                running.push(scope.spawn(move || {
                                    lane.into_iter()
                                        .map(|Travelling { parcel, worker }| {
                                            let res = parcel.job.run(worker.kernel());
                                            Returned { node: parcel.node, computed: Computed { body: parcel.body, res }, worker }
                                        })
                                        .collect::<Vec<_>>()
                                }));
                            }
                            for h in running {
                                if let Ok(done) = h.join() {
                                    back.extend(done);
                                }
                            }
                        });
                        // THE BODIES COME HOME AT ONCE, before anything else runs: the five node kinds still
                        // computed in place take their source from the shared kernel, and a source that had
                        // travelled would not be there. What a worker built carries its own face names inside
                        // the shape, so applying it later from the shared kernel answers the same.
                        for Returned { node, computed: done, worker } in back {
                            kernel.absorb(worker);
                            computed.insert(node, done);
                        }
                    }
                    // and the ones that stayed: computed here, on the shared kernel
                    for Parcel { node, body, job } in here {
                        let res = job.run(kernel);
                        computed.insert(node, Computed { body, res });
                    }
                    // written down next, in timeline order, before the walk moves on (see the order of the walk)
                    for &j in ready.iter().rev() {
                        if j != i && computed.contains_key(&self.timeline[j].id) {
                            next_up.push(j);
                        }
                    }
                }
            }

            // ONE DOOR FOR EVERY NODE THAT HAS A PARCEL. Before, each kind had its own arm here calling its own
            // rebuilder; now the kind names the parcel and the three steps are the same for all of them -
            // prepare, compute, apply. That is what lets a wave be computed at once (see `prep_node`): the same
            // parcels, several at a time.
            let mut done = false;
            if let Some(Computed { body: out, res }) = computed.remove(&node_id) {
                // already built, by this thread or another: what is left is to write it into the document
                let mut p = pass!(node_id);
                clear = self.apply_regen(p.landing(), out, res);
                done = true;
            } else if needs {
                let mut p = pass!(node_id);
                if let Some((out, job)) = self.prep_node(&mut p, &kind) {
                    let res = job.run(p.kernel);
                    clear = self.apply_regen(p.landing(), out, res);
                    done = true;
                }
            }
            match kind {
                _ if done => {}
                // Datums are resolved unconditionally (they are cheap) and in timeline order: a parametric plane
                // or axis is computed from its definition before the consumers below it.
                // A DATUM STANDING ON A BODY THAT IS GONE is red with that reason, as every node standing on a deleted
                // body is: resolved against the fingerprint the face left, it stood green on nothing. Reported
                // behaviour: the extrusion a plane from a face stood on was deleted, and the plane stayed green.
                FeatureKind::Plane { plane } if self.datum_base_body(plane).is_some_and(|b| !made.contains(&b)) => {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                    clear = false;
                }
                FeatureKind::DatumAxis { axis } if self.datum_base_body(axis).is_some_and(|b| !made.contains(&b)) => {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                    clear = false;
                }
                FeatureKind::Plane { plane } => {
                    self.resolve_plane_into(plane, &vars, feat_dims.get(&node_id));
                    if needs {
                        dirty.insert(plane); // The datum moved, so its consumers (sketches, mirror, split)
                                             // rebuild.
                    }
                }
                FeatureKind::DatumPoint { point } => {
                    self.resolve_point_into(point, &vars, feat_dims.get(&node_id), kernel);
                }
                FeatureKind::DatumAxis { axis } => {
                    self.resolve_axis_into(axis, kernel);
                }
                FeatureKind::Sketch { sketch } => {
                    // A SKETCH WHOSE FACE IS GONE stands on the snapshot of where the face stood (the deletion froze it
                    // there, so it does not move), and goes red with the reason rather than standing clean. Put on
                    // another face or plane, or the deletion undone, it is clean again.
                    use crate::errors::CoreError::{SketchFaceGone, SketchPlaneGone};
                    use crate::model::PlaneDef;
                    let gone = match self.sketch_index(sketch).map(|si| self.sketches[si].plane) {
                        Some(crate::feature::SketchPlane::Datum(pid)) => self.planes.iter().find(|p| p.id == pid).and_then(|p| match p.def {
                            PlaneDef::FaceGone => Some(SketchFaceGone),
                            PlaneDef::PlaneGone => Some(SketchPlaneGone),
                            _ => None,
                        }),
                        Some(crate::feature::SketchPlane::Face(b, _)) => (!self.timeline.iter().any(|n| n.kind.bodies().contains(&b))).then_some(SketchFaceGone),
                        _ => None,
                    };
                    if let Some(e) = gone {
                        self.regen_errors.insert(node_id, e.clone());
                        report.errors.push((node_id, e));
                    } else if matches!(self.regen_errors.get(&node_id), Some(SketchFaceGone | SketchPlaneGone)) {
                        self.regen_errors.remove(&node_id);
                    }
                    // Projections of body geometry are recomputed here, in timeline order: the source bodies
                    // above are already built and the consumers of the sketch (an extrude over the projected
                    // contour) come below. They are recomputed unconditionally rather than only under `needs`,
                    // because a projection depends on another body and "the sketch itself did not change" says
                    // nothing about whether that body did.
                    if let Some(si) = self.sketch_index(sketch) {
                        // SOLVED, NOT ONLY REBUILT, and along the way: what is tied to the projection - an end snapped to
                        // its corner, a dimension from it - follows it to where the body put it, on its own side
                        if !self.sketches[si].projections.is_empty() && self.follow_sketch_projections(si, kernel) {
                            dirty.insert(sketch); // The projection moved, so the consumers of the contour follow.
                        }
                    }
                    if needs {
                        dirty.insert(sketch); // The sketch changed, so its consumers rebuild.
                    }
                }
                FeatureKind::SplitBody { src, plane, datum, offset, ref bodies, face } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_splitbody(&mut p, src, self.op_plane(plane, datum, face), offset, bodies);
                }
                FeatureKind::Import { body, .. } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_import(&mut p, body);
                }
                FeatureKind::PartInstance { src_comp, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_partinstance(&mut p, i, src_comp, body);
                }
                FeatureKind::ComponentPattern { src, ref bodies, kind, .. } if needs => {
                    // every copy's body is the source's as it is; the pattern moves the copies, not the shapes
                    if kind.count() == 0 {
                        // a layout past `MAX_PATTERN_INSTANCES`, from a file: refused in words, nothing built
                        let e = crate::feature::pattern_too_large(&kind.counts());
                        self.regen_errors.insert(node_id, e.clone());
                        report.errors.push((node_id, e));
                        clear = false;
                    } else {
                        let mut p = pass!(node_id);
                        for b in bodies.clone() {
                            clear &= self.regen_partinstance(&mut p, i, src, b);
                        }
                    }
                }
                FeatureKind::MirrorPart { src_comp, ln, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_mirrorpart(&mut p, i, src_comp, ln, body);
                }
                FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_auger(&mut p, HelixRun { src, edge, length, lead_in, lead_out, body }, spec);
                }
                _ => {}
            }
            // Pass-through fallback: the modifier failed to build (broken edges or faces after a suppression or
            // an edit higher up the timeline — for example a suppressed fillet whose edges a chamfer below it
            // referenced) while its source exists, so it is skipped and the output body becomes a copy of the
            // source. That keeps the chain from holding stale geometry and makes the effect of the change
            // visible, instead of the model looking unchanged after a suppression. The error is already in the
            // report.
            // A PART IS ONE BODY OF ONE PIECE: an operation that left more pieces than its source had is refused in
            // words and passes through. Reported behaviour: an addition that touched nothing stood green with the body
            // in two pieces (12565 mm^3), a shell around a blind hole left a cup hanging in the cavity.
            if clear && keeps_one_piece {
                if let (Some(src), Some(b)) = (consumed_src, out_body) {
                    if kernel.body_pieces(b) > kernel.body_pieces(src) {
                        self.regen_errors.insert(node_id, crate::errors::CoreError::BodyInPieces);
                        report.errors.push((node_id, crate::errors::CoreError::BodyInPieces));
                        clear = false;
                    }
                }
            }

            // A CUT THAT PARTS THE BODY MAKES A BODY OF EVERY PIECE (see `CutPiece`) - when it was built in this pass: a node
            // left as it was holds its own piece alone already, and read again it lost the others
            if clear && needs && self.timeline[i].kind.cut_pieces_mut().is_some() {
                if let Some(b) = out_body {
                    clear = self.part_the_pieces(i, b, kernel, &mut dirty, &mut report, &mut emap);
                }
            }
            // A NODE OF SEVERAL BODIES KEEPS ITS LAST GOOD PIECES: the source copied into its first piece stood the whole
            // body beside the other pieces. Reported behaviour: a split moved to where it cuts nothing held 18000 mm^3 -
            // the block of 12000 in the first piece and the piece of 6000 beside it.
            let several = out_bodies.len() > 1;
            if !clear && !several {
                if let (Some(src), Some(b)) = (consumed_src.filter(|s| !unbuilt.contains(s)), out_body) {
                    if let Ok(crate::geom::Built { mesh, faces }) = kernel.transform_body(b, src, crate::feature::PLACE_IDENTITY) {
                        self.set_body_mesh(b, mesh);
                        // the copy is what the source was, a sheet or a solid: a sheet passed through a red trim
                        // counted as a solid of one face, a second body of the part
                        if let Some(i) = self.mesh_index(b) {
                            self.bodies[i].sheet = kernel.body_is_sheet(b);
                        }
                        self.regen_faces.insert(b, faces.clone());
                        dirty.insert(b); // Consumers rebuild on the pass-through body.
                        report.built.push((b, faces));
                        clear = true; // The state is current; the real build returns once the failure above is
                                      // fixed.
                    }
                }
            }
            // References to this body are repaired immediately: its new face names are already known while the
            // consumers below it have not been built yet. That survives both an ordinary topology change and a
            // change of the naming scheme itself, without "nearest match taken" on every other node.
            if let Some(b) = out_body {
                self.rebind_refs_to_body(b, faces_before.get(&b).map(|v| v.as_slice()).unwrap_or(&[]), &mut report);
            }
            if clear {
                self.timeline[i].dirty = false;
            }
            if let Some(t) = started {
                report.spent.push((node_id, self.timeline[i].name.clone(), t.elapsed().as_micros()));
            }
            // When a node fails, everything resting on it is not built either.
            //
            // The cascade existed but errors never reached it: the next operation took the body of the failed
            // one as its source, complete with its stale mesh, and built on a ghost. That produced two visible
            // bodies in one part and the avalanche where fixing one feature breaks another, because everything
            // below was already built on nothing.
            //
            // A temporary error (the source is not built yet, the live B-rep is not raised) does not count here:
            // such a node is waiting its turn rather than broken.
            if report.errors.iter().any(|(id, e)| *id == node_id && !e.retryable()) {
                for b in out_bodies {
                    unbuilt.insert(b);
                    broken.insert(b);
                }
            }
        }
        // Edges of the built bodies are copied into the model, keyed by body id, so axis connectors
        // (`EdgeMid`) resolve by persistent id. Only the bodies rebuilt in this pass, as for `regen_faces`; a
        // mock supplies none.
        let built_bodies: Vec<Id> = report.built.iter().map(|(b, _)| *b).collect();
        for b in built_bodies {
            let edges = kernel.edges(b);
            if edges.is_empty() {
                self.regen_edges.remove(&b);
            } else {
                self.regen_edges.insert(b, edges);
            }
        }
        // Parametric mate expressions (angle and offsets over global parameters, like feature and sketch
        // dimensions): `feat_dims[joint id]["angle" | "offset" | "offset2"]` becomes a number before the solve,
        // and an empty expression keeps the stored number.
        //
        // An expression is a specified value: it states what the value has to equal. It is written into `drive`
        // rather than into the reading, or the solver would simply overwrite it with the measured fact.
        for j in &mut self.joints {
            let Some(d) = feat_dims.get(&j.id) else { continue };
            for (slot, key) in [(0usize, "angle"), (1, "offset"), (2, "offset2")] {
                let has = d.get(key).is_some_and(|e| !e.trim().is_empty());
                if has {
                    j.drive[slot] = Some(eval_dim(Some(d), key, j.drive[slot].unwrap_or(0.0), &vars));
                }
            }
        }
        // the copies of a pattern turned about a datum axis follow the axis this pass resolved
        self.resolve_comp_patterns();
        // Placement pass: the bodies are built in local space, so the mates are solved and the components are
        // placed in the assembly. Run unconditionally, because editing a mate or a parameter does not dirty any
        // body while the placement still has to be recomputed.
        self.solve_joints();
        // Clear error marks from nodes that are no longer in the timeline.
        if !self.regen_errors.is_empty() || !self.edge_refs.is_empty() {
            let live: std::collections::HashSet<Id> = self.timeline.iter().map(|n| n.id).collect();
            self.regen_errors.retain(|id, _| live.contains(id));
            self.edge_refs.retain(|id, _| live.contains(id)); // Edge snapshots of deleted features.
        }
        // A STOP ASKED WHILE A NODE WAS BEING BUILT cancels the rebuild as a stop between nodes does: the kernel breaks
        // a long operation off (the recognition of a mesh listens for it) and hands back a refusal that is no failure
        // of the node. Taken for one, it left the node red; with one node in the plan, the stop between nodes was
        // never even asked, and Cancel cancelled nothing.
        report.cancelled |= watch.stopped();
        self.mark_found_by_place();
        report
    }
}
