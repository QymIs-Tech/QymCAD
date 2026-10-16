//! DELETING ON THE TIMELINE: a sketch, a body, a feature, with their dependents or leaving them red.

use super::*;

impl Project {
    /// Rebuild graph: what depends on node `id`, transitively, associativity through faces included.
    ///
    /// The timeline is linear and already a topological order, so dirt propagates correctly along it. What was
    /// missing is a way to ask the graph, and without it the application marked nodes dirty with a margin in
    /// several places. Three kinds of edge are counted: an input body or sketch (`kind.inputs()`), a sketch on
    /// a face of a body (including a top-down external reference), and a mirrored part depending on the active
    /// body of its source.
    pub fn dependents(&self, id: Id) -> std::collections::HashSet<Id> {
        use crate::feature::FeatureKind;
        let mut hit: std::collections::HashSet<Id> = std::collections::HashSet::from([id]);
        // The bodies of the root node join the frontier too, consumers referencing a body rather than a node.
        // A split produces several bodies, so all of them are taken (`bodies`); otherwise every piece but the
        // first would drop out of the frontier.
        if let Some(n) = self.timeline.iter().find(|n| n.id == id) {
            hit.extend(n.kind.bodies());
        }
        loop {
            let mut grew = false;
            for n in &self.timeline {
                if hit.contains(&n.id) {
                    continue;
                }
                let by_input = n.kind.inputs().iter().any(|inp| hit.contains(inp));
                let by_plane = n.kind.inputs().iter().any(|inp| self.sketch_plane_body(*inp).is_some_and(|pb| hit.contains(&pb)));
                let by_mirror = matches!(n.kind, FeatureKind::MirrorPart { src_comp, .. } if self.active_body(src_comp).is_some_and(|b| hit.contains(&b)));
                if by_input || by_plane || by_mirror {
                    hit.insert(n.id);
                    hit.extend(n.kind.bodies());
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        hit.remove(&id);
        hit
    }

    /// Delete a sketch and its `Sketch` node, and nothing else: every feature built on it stays in the timeline,
    /// marked for a rebuild, where it fails and says why - no profile to build from. The person repairs it or
    /// deletes it; losing the work above the sketch without a word is not the delete's to decide. The cascade is
    /// `delete_sketch_with_dependents`.
    pub fn delete_sketch(&mut self, sid: Id) {
        use crate::feature::FeatureKind;
        self.timeline.retain(|nd| !matches!(nd.kind, FeatureKind::Sketch { sketch } if sketch == sid));
        for nd in self.timeline.iter_mut() {
            if nd.kind.inputs().contains(&sid) {
                nd.dirty = true;
            }
        }
        if let Some(si) = self.sketch_index(sid) {
            self.remove_sketch(si);
        }
    }

    /// Delete a sketch as a cascade: the `Sketch` node in the timeline, every feature built on it (extrude,
    /// cut, revolve) and everything downstream (fillets, patterns and so on) together with their bodies and
    /// parametric dimensions, then the sketch itself. Returns the ids of the deleted bodies, for the interface
    /// caches.
    pub fn delete_sketch_with_dependents(&mut self, sid: Id) -> Vec<Id> {
        use crate::feature::FeatureKind;
        // Transitive closure of the doomed set: everything that depends on the sketch directly or
        // indirectly.
        let mut doomed: std::collections::HashSet<Id> = std::collections::HashSet::new();
        doomed.insert(sid);
        loop {
            let mut changed = false;
            for nd in &self.timeline {
                for body in nd.kind.bodies() {
                    if !doomed.contains(&body) && nd.kind.inputs().iter().any(|i| doomed.contains(i)) {
                        doomed.insert(body);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        // by id: a set's order is chosen at random for every set, and this list is returned and its bodies remembered
        let mut bodies: Vec<Id> = doomed.iter().copied().filter(|id| *id != sid).collect();
        bodies.sort_unstable();
        // Owners of the doomed bodies, captured before the deletion: they are used to mark whoever copies
        // those parts.
        let orphaned: std::collections::HashSet<Id> = bodies.iter().filter_map(|b| self.body_owner(*b)).collect();
        // Drop the timeline nodes: the `Sketch` node plus everything producing a doomed body.
        self.timeline.retain(|nd| {
            if matches!(nd.kind, FeatureKind::Sketch { sketch } if sketch == sid) {
                return false;
            }
            !nd.kind.bodies().iter().any(|b| doomed.contains(b))
        });
        // Bodies and the parametric dimensions of the doomed features.
        for b in &bodies {
            if let Some(mi) = self.mesh_index(*b) {
                self.remove_mesh(mi);
            }
            self.feat_dims.remove(b);
        }
        // The sketch itself, from the pools.
        if let Some(si) = self.sketch_index(sid) {
            self.remove_sketch(si);
        }
        // The rebuild caches go too. Dropping the mesh and the dimensions while leaving `regen_faces` and
        // `regen_edges` behind produced a body that no timeline node builds (the timeline went from 4 nodes to
        // 2) yet still counts as alive in the cache, so everything enumerating live bodies counts it. The
        // helper for exactly this failure exists next door; it simply was not called from here.
        self.drop_orphan_bodies();
        self.drop_connectors_of_dead_bodies(&bodies);
        self.break_external_refs_of_dead_bodies();
        self.freeze_sketches_on_dead_faces();
        self.mark_copiers_dirty(&orphaned);
        bodies
    }

    /// Cascading deletion of body `body`: the body itself plus every feature that transitively consumes it
    /// (move, chamfer, boolean and so on), whose results would otherwise linger as ghosts. It clears the
    /// timeline nodes, the meshes, the feature dimensions and the rebuild caches. Returns the set of deleted
    /// bodies, which the application uses to clear its own shape cache.
    pub fn delete_body_cascade(&mut self, body: Id) -> std::collections::HashSet<Id> {
        let mut doomed: std::collections::HashSet<Id> = std::collections::HashSet::from([body]);
        loop {
            let mut grew = false;
            for n in &self.timeline {
                let outs = n.kind.bodies();
                // The pieces of a split live and die together: they are the output of one operation, and
                // leaving half of them without a node would produce a ghost body with no recipe.
                let hit = outs.iter().any(|b| doomed.contains(b)) || n.kind.inputs().iter().any(|inp| doomed.contains(inp));
                if hit && outs.iter().any(|b| !doomed.contains(b)) {
                    doomed.extend(outs);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        self.remove_bodies(&doomed);
        let doomed_list: Vec<Id> = doomed.iter().copied().collect();
        // No body sweep is needed here: `remove_bodies` already clears the caches, verified by removing the
        // call and finding the whole suite still green. It does not touch connectors or external references,
        // though, and without the next two lines the tests fail.
        self.drop_connectors_of_dead_bodies(&doomed_list);
        self.break_external_refs_of_dead_bodies();
        self.freeze_sketches_on_dead_faces();
        doomed
    }

    /// Delete the whole operation node `node_id` belongs to — its entire span — plus everything downstream.
    ///
    /// Cascading from the first body only leaves the sibling extrudes of a collapsed "extrude N contours" row
    /// as orphaned bodies: the consumers are removed while the siblings, which do not consume the first body,
    /// remain. The part then falls apart into separate bodies and deleting again resurrects an apparently
    /// deleted node, a span sibling surfacing as a new row. The whole span is removed at once instead.
    /// An external reference does not outlive its source body, and it must not be cut off crudely either.
    ///
    /// A consumer sketch sits on a face of another part; with the body removed and the record left behind, the
    /// reference leads nowhere. `break_external_ref` covers that case: it freezes the sketch into a snapshot
    /// datum exactly where it stood, leaving the consumer part usable. Simply dropping the record would leave
    /// the sketch hanging on a dead face.
    pub(crate) fn break_external_refs_of_dead_bodies(&mut self) {
        let live: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        let dead: Vec<Id> = self.external_refs.iter().filter(|r| r.source_body().is_some_and(|b| !live.contains(&b))).map(|r| r.id).collect();
        for rid in dead {
            self.break_external_ref(rid);
        }
    }

    /// A sketch on a face of a removed body freezes into a snapshot.
    ///
    /// Measured: a sketch placed on a face of its own body survived the removal of that body and stayed bound
    /// to the `Face` of a body that no longer exists. The geometry did not move, the frame resolving from the
    /// old snapshot, which makes the failure a quiet one: the sketch looks attached to a face that is gone and
    /// every later rebuild silently uses a stale fingerprint. The neighbouring case — a sketch on a face of
    /// another part — is frozen by `break_external_ref`; this is the same treatment where no external reference
    /// record exists.
    pub(crate) fn freeze_sketches_on_dead_faces(&mut self) {
        use crate::feature::SketchPlane;
        let live: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        let targets: Vec<(usize, Id, Id, crate::feature::FaceKey)> = self
            .sketches
            .iter()
            .enumerate()
            .filter_map(|(si, s)| match s.plane {
                SketchPlane::Face(b, key) if !live.contains(&b) => Some((si, s.id, b, key)),
                _ => None,
            })
            .collect();
        for (si, sid, body, key) in targets {
            let owner = self.sketch_owner(sid).unwrap_or(0);
            let before = self.sketch_frame(si);
            let pid = self.snapshot_face_plane_for(owner, body, &key);
            self.sketches[si].plane = SketchPlane::Datum(pid);
            // the snapshot says where the face stood, and that the face is gone: the sketch goes red on it
            if let Some(pl) = self.planes.iter_mut().find(|p| p.id == pid) {
                pl.def = crate::model::PlaneDef::FaceGone;
            }
            self.mark_node_dirty(sid); // the rebuild visits it and marks it red
                                       // The snapshot has to land exactly where the live frame stood: freezing does not move the
                                       // sketch.
            if let (Some(before), Some(pi)) = (before, self.planes.iter().position(|p| p.id == pid)) {
                self.planes[pi].origin = before.origin;
                self.planes[pi].normal = before.normal();
            }
        }
    }

    pub(crate) fn drop_orphan_bodies(&mut self) {
        let produced: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        self.bodies.retain(|b| produced.contains(&b.id));
        self.regen_faces.retain(|b, _| produced.contains(b));
        self.regen_edges.retain(|b, _| produced.contains(b));
    }

    /// A connector does not outlive its body.
    ///
    /// An anchor references a face, an edge or a vertex of a specific body; once that body is removed the
    /// anchor resolves to the old centroid fingerprint and the solver scatters the bodies to garbage
    /// coordinates. Component deletion handled this long ago while the body cascade and sketch deletion did
    /// not: measured, both paths left a dangling connector and a surviving joint.
    /// Remember the bodies that were just deleted, so a mate on them goes red instead of disappearing.
    ///
    /// `removed` holds the bodies taken away by this very operation. The test "the body is not in the timeline"
    /// will not do: it does not separate "deleted" from "not built yet", and those differ — in the second case
    /// the mate has to come back to life by itself once the geometry is raised.
    pub fn drop_connectors_of_dead_bodies(&mut self, removed: &[Id]) {
        use crate::feature::AnchorRef;
        let live: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        let dead: std::collections::HashSet<Id> = self
            .connectors
            .iter()
            .filter(|c| match &c.anchor {
                AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => !live.contains(b),
                _ => false,
            })
            .map(|c| c.id)
            .collect();
        // A mate outlives its geometry and goes red.
        //
        // Deleting the mates and the connectors here had a sound motive: an anchor on a vanished body resolves
        // to the old centroid fingerprint stored in the face key, the solver gets a garbage frame and scatters
        // the bodies to wild coordinates. But that treated the symptom at the author's expense: deleting one
        // body lost the assembly work, discovered only when parts turned out to be missing.
        //
        // The document now remembers the deleted bodies, `connector_frame` honestly answers "none" for them,
        // and the reason is named by `joint_faults`, by the panel and by the solver report. Repairing or
        // deleting is the author's decision.
        //
        // What is remembered is the bodies rather than the connectors: a body disappears once and for all, and
        // from it one can both say which anchor died and avoid confusing that with a body that is not built
        // yet.
        //
        // Each deletion's bodies are appended by id. The callers hand them over in the order of a `HashSet`, which is
        // chosen at random for every set, and this list is saved with the document: the same edit saved twice gave two
        // files. (The list as a whole is in the order of the deletions; a file saved before keeps the order it had.)
        let mut newly: Vec<Id> = removed.iter().copied().filter(|b| !live.contains(b) && !self.dead_bodies.contains(b)).collect();
        newly.sort_unstable();
        newly.dedup();
        self.dead_bodies.extend(newly);
        let _ = dead;
    }

    /// Delete operation node `node_id` alone. A node in the middle hands its consumers to its source (below); a
    /// base node - an extrusion, a primitive - has no source, and what stood on it stays in the timeline, marked
    /// for a rebuild, where it fails and says why. The cascade over it is `delete_feature_with_dependents`.
    pub fn delete_feature_op(&mut self, node_id: Id) -> std::collections::HashSet<Id> {
        self.delete_feature_node(node_id, false)
    }

    /// Delete operation node `node_id` and everything built on it, whether or not it has a source to hand it to.
    pub fn delete_feature_with_dependents(&mut self, node_id: Id) -> std::collections::HashSet<Id> {
        self.delete_feature_node(node_id, true)
    }

    pub(crate) fn delete_feature_node(&mut self, node_id: Id, dependents: bool) -> std::collections::HashSet<Id> {
        // A body goes with its node. A mesh nobody produces any more stayed in the document and surfaced in
        // the root assembly as its own row, showing bodies in the tree that were never made and belong to no
        // part. The cleanup sits at the end of the deletion, in one place nothing can bypass.
        let _ = &node_id;
        // An error goes with its node. Otherwise the document keeps a failure belonging to something that no
        // longer exists: an error is shown with no node under it in the tree.
        self.regen_errors.remove(&node_id);
        // One operation is one node, with several contours held in `profiles`.
        //
        // Measured: deleting one operation in the middle also removed two chamfers and a cut, leaving a wall
        // where an opening was expected. The cause: in a chain timeline every following operation takes the
        // previous body as its source, so removing the consumers downstream removes the entire timeline below.
        // That loses work.
        //
        // Consumers are re-pointed instead: a consumer of the deleted node attaches to its source and the
        // timeline below lives on. Along with the bodies, whatever sits on faces of the deleted body is
        // re-pointed too — sketches on a face, datum axes on an edge or a face, datum planes offset from a face
        // — otherwise they hang on a body that does not exist and silently build nothing. Faces are looked up by
        // persistent key, so they are found on the source body by themselves; whatever is not found goes red
        // honestly on rebuild.
        //
        // The cascade stays where there is nothing to re-point to: a base operation (an extrude, a primitive)
        // has no source, and everything resting on it really does lose its support.
        let Some(node) = self.timeline.iter().find(|n| n.id == node_id) else {
            return std::collections::HashSet::new();
        };
        // A PATTERN OF COMPONENTS goes whole: its node and its copies, the source staying - the one door every
        // deletion of the pattern takes, from its row in the tree or from a copy
        if let crate::feature::FeatureKind::ComponentPattern { bodies, .. } = &node.kind {
            let gone: std::collections::HashSet<Id> = bodies.iter().copied().collect();
            self.delete_comp_pattern(node_id);
            return gone;
        }
        // A node may have several outputs (a split gives one body per piece) and every one of them has to be
        // re-pointed: handling only the first leaves the rest as orphaned meshes with no timeline node — ghost
        // bodies absent from the tree yet present in the project and surfacing in the root assembly.
        let outs = node.kind.bodies();
        let Some(&_first) = outs.first() else {
            self.timeline.retain(|n| n.id != node_id);
            self.drop_orphan_bodies();
            return std::collections::HashSet::new();
        };
        let src = node.kind.consumed_body().filter(|s| *s != 0 && !outs.contains(s) && !dependents);
        let Some(src) = src else {
            if !dependents {
                // The base goes alone: its consumers keep naming its bodies, find none on the rebuild and go red.
                let gone: std::collections::HashSet<Id> = outs.into_iter().collect();
                // a datum on a face or an edge of it names the body in its definition rather than among its inputs
                let datums: std::collections::HashSet<Id> = self
                    .timeline
                    .iter()
                    .filter_map(|n| match n.kind {
                        crate::feature::FeatureKind::Plane { plane: d } | crate::feature::FeatureKind::DatumAxis { axis: d } => self.datum_base_body(d).filter(|b| gone.contains(b)).map(|_| n.id),
                        _ => None,
                    })
                    .collect();
                for n in self.timeline.iter_mut() {
                    if n.kind.inputs().iter().any(|i| gone.contains(i)) || datums.contains(&n.id) {
                        n.dirty = true;
                    }
                }
                self.remove_bodies(&gone);
                let list: Vec<Id> = gone.iter().copied().collect();
                self.drop_connectors_of_dead_bodies(&list);
                self.break_external_refs_of_dead_bodies();
                self.freeze_sketches_on_dead_faces();
                return gone;
            }
            // Nothing to re-point to (a base feature), or the dependents were asked for: the cascade runs over every output.
            let mut gone = std::collections::HashSet::new();
            for o in outs {
                gone.extend(self.delete_body_cascade(o));
            }
            self.drop_orphan_bodies();
            return gone;
        };
        // Re-pointing also means translating the names. A consumer attaches to the source body while its
        // references still name the faces and edges of the deleted one: an edge name is derived from its pair
        // of faces, and the source has different faces. Measured symptom: deleting a fillet turned a chamfer
        // red on the opposite side of the part, geometry the fillet never touched.
        let nmaps: Vec<(Id, std::collections::HashMap<u32, u32>)> = outs.iter().map(|&o| (o, self.geom_name_map(o, src))).collect();
        for n in self.timeline.iter_mut() {
            if n.id == node_id {
                continue;
            }
            for (o, nmap) in &nmaps {
                let touched = n.kind.inputs().contains(o);
                n.kind.remap_body_input(*o, src);
                if touched {
                    n.kind.remap_names(nmap); // The same places, under the names of the source body.
                    n.dirty = true; // It attached to a different body, so it rebuilds.
                }
            }
        }
        for (o, nmap) in &nmaps {
            self.rebind_body_refs(*o, src, nmap);
        }
        let gone: std::collections::HashSet<Id> = outs.into_iter().collect();
        self.remove_bodies(&gone);
        gone
    }

    /// Strip everything from the bodies in `bodies`: the mesh, the id and the name, the feature dimensions, the
    /// face and edge rebuild caches, and the timeline nodes that produce them.
    pub(crate) fn remove_bodies(&mut self, bodies: &std::collections::HashSet<Id>) {
        // Whoever copies a part losing its body has to be recomputed.
        //
        // A mirrored part and a pattern instance reference a component rather than a body, and take its active
        // body. The ordinary input cascade does not reach them, that body not being in `inputs()`.
        // Measured failure: deleting a sketch left a part without a body while its mirror still showed geometry
        // from the previous build on screen, and only failed once something finally forced it to recompute. The
        // owners are collected before the deletion, since afterwards `body_owner` no longer answers.
        let orphaned: std::collections::HashSet<Id> = bodies.iter().filter_map(|b| self.body_owner(*b)).collect();
        for &db in bodies {
            self.feat_dims.remove(&db);
            self.drop_body_from_view(db);
        }
        self.timeline.retain(|n| !n.kind.bodies().iter().any(|nb| bodies.contains(nb)));
        self.mark_copiers_dirty(&orphaned);
    }

    /// Mark as dirty whoever copies a part that lost its body.
    ///
    /// One method for every deletion path, by the same rule as the other topology edits: separate copies would
    /// leave a mirror hanging on screen with geometry from the previous build on one path while recomputing
    /// honestly on another. The owners are passed in from before the deletion, since afterwards `body_owner` no
    /// longer answers.
    pub(crate) fn mark_copiers_dirty(&mut self, owners: &std::collections::HashSet<Id>) {
        if owners.is_empty() {
            return;
        }
        for n in self.timeline.iter_mut() {
            if n.kind.copy_source().is_some_and(|c| owners.contains(&c)) {
                n.dirty = true;
            }
        }
    }
}
