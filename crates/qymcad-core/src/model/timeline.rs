//! Feature timeline: the nodes, their order, their owners, the rollback bar and editing the feature tree.
//!
//! The split is mechanical: methods that touch `timeline` and `rollback` and nothing else in the model live
//! here. Those interwoven with sketches, assemblies or datums stay where they are, since moving them would
//! relocate the entanglement rather than untangle it.
//!
//! The rebuild itself lives separately (`model::regen`); this module is about the structure of the tree rather
//! than about building geometry from it.

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

    /// Delete a sketch as a cascade: the `Sketch` node in the timeline, every feature built on it (extrude,
    /// cut, revolve) and everything downstream (fillets, patterns and so on) together with their bodies and
    /// parametric dimensions, then the sketch itself. Returns the ids of the deleted bodies, for the interface
    /// caches.
    pub fn delete_sketch(&mut self, sid: Id) -> Vec<Id> {
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
        let bodies: Vec<Id> = doomed.iter().copied().filter(|id| *id != sid).collect();
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
    fn break_external_refs_of_dead_bodies(&mut self) {
        let live: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        let dead: Vec<Id> = self
            .external_refs
            .iter()
            .filter(|r| r.source_body().is_some_and(|b| !live.contains(&b)))
            .map(|r| r.id)
            .collect();
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
    fn freeze_sketches_on_dead_faces(&mut self) {
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
            // The snapshot has to land exactly where the live frame stood: freezing does not move the
            // sketch.
            if let (Some(before), Some(pi)) = (before, self.planes.iter().position(|p| p.id == pid)) {
                self.planes[pi].origin = before.origin;
                self.planes[pi].normal = before.normal();
            }
        }
    }

    pub(super) fn drop_orphan_bodies(&mut self) {
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
        for b in removed {
            if !live.contains(b) && !self.dead_bodies.contains(b) {
                self.dead_bodies.push(*b);
            }
        }
        let _ = dead;
    }

    pub fn delete_feature_op(&mut self, node_id: Id) -> std::collections::HashSet<Id> {
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
        // A node may have several outputs (a split gives one body per piece) and every one of them has to be
        // re-pointed: handling only the first leaves the rest as orphaned meshes with no timeline node — ghost
        // bodies absent from the tree yet present in the project and surfacing in the root assembly.
        let outs = node.kind.bodies();
        let Some(&_first) = outs.first() else {
            self.timeline.retain(|n| n.id != node_id);
            self.drop_orphan_bodies();
            return std::collections::HashSet::new();
        };
        let src = node.kind.consumed_body().filter(|s| *s != 0 && !outs.contains(s));
        let Some(src) = src else {
            // Nothing to re-point to (a base feature), so the cascade runs over every output.
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
    pub(super) fn remove_bodies(&mut self, bodies: &std::collections::HashSet<Id>) {
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
    pub(super) fn mark_copiers_dirty(&mut self, owners: &std::collections::HashSet<Id>) {
        if owners.is_empty() {
            return;
        }
        for n in self.timeline.iter_mut() {
            let src = match n.kind {
                crate::feature::FeatureKind::MirrorPart { src_comp, .. } => Some(src_comp),
                crate::feature::FeatureKind::PartInstance { src_comp, .. } => Some(src_comp),
                _ => None,
            };
            if src.is_some_and(|c| owners.contains(&c)) {
                n.dirty = true;
            }
        }
    }

    /// The live body of this lineage: walk forward through `consumed` while the body is consumed.
    ///
    /// Every operation creates a new body and consumes its input, so any reference recorded earlier (a sketch
    /// plane on a face, a tree selection) points at a body the model no longer holds from the second operation
    /// onwards. Many places need to ask which body is the current one, and the answer has to come from one
    /// place: copies of this walk drift apart silently and put geometry from the past on screen.
    ///
    /// A body that is not consumed is returned as it is. The chain is bounded from above: a broken timeline (a
    /// cycle in `consumed`) must not hang the application.
    pub fn live_body(&self, body: Id) -> Id {
        let consumed = self.consumed_bodies();
        let mut cur = body;
        for _ in 0..256 {
            if !consumed.contains(&cur) {
                break;
            }
            match self.timeline.iter().find(|n| n.kind.consumed().contains(&cur)).and_then(|n| n.kind.body()) {
                Some(next) => cur = next,
                None => break,
            }
        }
        cur
    }

    /// Lineage root of a body: walk back through `consumed_body` to the feature that consumes no body (an
    /// extrude, a primitive, a boolean — the original part). A colour is keyed by that root and therefore stays
    /// stable across operations.
    pub fn lineage_root(&self, body: Id) -> Id {
        let mut b = body;
        for _ in 0..10_000 {
            match self.timeline.iter().find(|n| n.kind.bodies().contains(&b)).and_then(|n| n.kind.consumed_body()) {
                Some(src) => b = src,
                None => break,
            }
        }
        b
    }

    /// Stable part index by lineage root (the order in which root bodies appear in the timeline), used by the
    /// default palette.
    pub(super) fn part_color_index(&self, root: Id) -> usize {
        let mut idx = 0;
        for n in &self.timeline {
            if n.kind.body().is_some() && n.kind.consumed_body().is_none() {
                if n.kind.bodies().contains(&root) {
                    return idx;
                }
                idx += 1;
            }
        }
        0
    }

    /// Set the list of filled contours (`fill`) for an extrude or an operation feature, by its body `body`, and
    /// mark the node dirty for the rebuild. Contours in `fill` are not subtracted as holes (see
    /// `feature_profile_encoded_fill`).
    pub fn set_feature_fill(&mut self, body: Id, fill: Vec<Id>) {
        use crate::feature::FeatureKind;
        if let Some(n) = self.timeline.iter_mut().find(|n| n.id == body) {
            match &mut n.kind {
                FeatureKind::Extrude { fill: f, .. } | FeatureKind::Combine { fill: f, .. } => {
                    *f = fill;
                    n.dirty = true;
                }
                _ => {}
            }
        }
    }

    /// Index of a timeline node by id.
    pub fn timeline_index(&self, id: Id) -> Option<usize> {
        self.timeline.iter().position(|n| n.id == id)
    }

    /// Add a sketch node to the timeline, when it is not there yet. Returns the node id, which equals the
    /// sketch id.
    pub fn add_sketch_node(&mut self, sketch: Id, name: impl Into<String>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if self.timeline_index(sketch).is_none() {
            let parent = Some(self.active_ctx());
            self.push_timeline(FeatureNode { id: sketch, name: name.into(), kind: FeatureKind::Sketch { sketch }, parent, dirty: false, suppressed: false });
        }
        sketch
    }

    /// Extrude a specific closed contour `profile` (zero takes the first). Holes are subtracted automatically
    /// and `reach` says which way the body grows from the sketch plane. Returns the body id.
    pub fn add_extrude_on(&mut self, sketch: Id, profile: Id, height: f64, reach: crate::feature::Reach, down: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-extrude".into(), kind: FeatureKind::Extrude { sketch, profiles: vec![profile], height, reach, down, fill: Vec::new(), body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Extrude several contours as one operation: one node, one body. An empty `profiles` takes the first
    /// contour.
    pub fn add_extrude_multi(&mut self, sketch: Id, profiles: Vec<Id>, height: f64, reach: crate::feature::Reach, down: f64, fill: Vec<Id>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-extrude".into(), kind: FeatureKind::Extrude { sketch, profiles, height, reach, down, fill, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Revolve. `axis_datum` is a datum axis id (zero uses the sketch X or Y axis), including an associative
    /// one built from an edge or a cylindrical face (`add_axis_from_edge`, `add_axis_from_face`). `axis_line` is
    /// the id of a sketch centreline (zero means none) and takes priority over the datum and the sketch axes.
    /// The axis has to lie in the sketch plane.
    pub fn add_revolve_axis(&mut self, sketch: Id, profiles: Vec<Id>, axis: u8, angle: f64, axis_datum: Id, axis_line: Id) -> Id {
        self.add_revolve_multi_op(sketch, profiles, axis, angle, axis_datum, axis_line, crate::feature::Reach::default(), 0, 1)
    }

    /// Revolve every contour and apply the boolean against body `src` within one node.
    ///
    /// A command over two contours used to create one `Revolve` node per contour plus one `BodyBoolean` node per
    /// cut — four nodes for one action. In the timeline that reads as a revolve falling apart into two features
    /// doing an add instead of a cut; it could only be edited one contour at a time, and deleting any of the
    /// nodes took the whole chain below with it. One operation is now one node, as for an extrude.
    #[allow(clippy::too_many_arguments)]
    pub fn add_revolve_multi_op(&mut self, sketch: Id, profiles: Vec<Id>, axis: u8, angle: f64, axis_datum: Id, axis_line: Id, reach: crate::feature::Reach, src: Id, op: u8) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-revolve".into(),
            kind: FeatureKind::Revolve { sketch, profiles, axis, angle, axis_datum, axis_line, reach, src, op, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Revolve with the direction the angle is swept in: forwards, back, or half each way.
    pub fn add_revolve_axis_ex(&mut self, sketch: Id, profiles: Vec<Id>, axis: u8, angle: f64, axis_datum: Id, axis_line: Id, reach: crate::feature::Reach) -> Id {
        self.add_revolve_multi_op(sketch, profiles, axis, angle, axis_datum, axis_line, reach, 0, 1)
    }

    /// Sweep: a profile (`sketch`, `profile`) along a path (`path_sketch`, `path`). A zero `profile` or `path`
    /// picks the first suitable contour automatically. Returns the body id; the node is dirty and is built by
    /// the next rebuild.
    pub fn add_sweep(&mut self, sketch: Id, profiles: Vec<Id>, path_sketch: Id, path: Id) -> Id {
        self.add_sweep_multi_op(sketch, profiles, path_sketch, path, 0, 1)
    }

    /// Sweep every profile contour and apply the boolean against body `src` within one node. Same reasoning as
    /// [`Project::add_revolve_multi_op`].
    pub fn add_sweep_multi_op(&mut self, sketch: Id, profiles: Vec<Id>, path_sketch: Id, path: Id, src: Id, op: u8) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-sweep".into(), kind: FeatureKind::Sweep { sketch, profiles, path_sketch, path, src, op, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Loft: a body through an ordered set of section sketches `sketches` (at least two). `contours` gives the
    /// selected contour per section (zero takes the first closed one; a shorter list leaves the rest automatic)
    /// and `ruled` produces straight faces.
    ///
    /// A zero `src` makes a separate new body; otherwise the lofted solid is combined with body `src` through
    /// `op` (0 cut, 1 union, 2 intersection), giving a lofted cut or boss. Returns the body id; the node is
    /// dirty and is built by the next rebuild.
    pub fn add_loft(&mut self, sketches: Vec<Id>, contours: Vec<Id>, ruled: bool, src: Id, op: u8, surface: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-loft".into(), kind: FeatureKind::Loft { sketches, contours, ruled, src, op, surface, body }, parent, dirty: true, suppressed: false });
        body
    }


    /// Owning component of a timeline node, by id (the node parent); `None` is the document root.
    pub fn node_component(&self, node_id: Id) -> Option<Id> {
        self.timeline.iter().find(|n| n.id == node_id).and_then(|n| n.parent)
    }

    /// Bodies belonging to component `id`: the timeline nodes of that component that produce a body.
    pub fn component_bodies(&self, id: Id) -> Vec<Id> {
        self.timeline.iter().filter(|n| n.parent == Some(id)).flat_map(|n| n.kind.bodies()).collect()
    }

    /// Sketches belonging to component `id`.
    pub fn sketches_of_component(&self, id: Id) -> Vec<Id> {
        self.timeline
            .iter()
            .filter(|n| n.parent == Some(id))
            .filter_map(|n| match n.kind {
                crate::feature::FeatureKind::Sketch { sketch } => Some(sketch),
                _ => None,
            })
            .collect()
    }

    /// The single engine for deep-cloning the subtree of component `id` from source project `from` into project
    /// `dest` under `target_parent`.
    ///
    /// It serves both a clone within one project (`clone_component`, with `from` a snapshot of self) and a clone
    /// between projects (`graft` for inserting a product, `subproject_of` for extracting one). Every output id —
    /// components, sketches, planes, points, axes, bodies, nodes and `feat_dims` — is remapped through
    /// `dest.alloc_id` and therefore cannot collide with the ids of the host. References leading outside the
    /// subtree are cut, as they are on removal. Bodies are marked dirty and are rebuilt by `regenerate(kernel)`.
    /// Returns the id of the cloned root in `dest`.
    pub(super) fn clone_subtree_into(dest: &mut Project, from: &Project, id: Id, target_parent: Id) -> Option<Id> {
        use crate::feature::{Component, FeatureKind, FeatureNode};
        use std::collections::HashMap;
        if id == from.root || !from.components.iter().any(|c| c.id == id) || !dest.components.iter().any(|c| c.id == target_parent) {
            return None;
        }
        if dest.component_is_part(target_parent) {
            return None; // A part inside a part is not allowed: a component goes into an assembly only.
        }
        // The component subtree: the component itself plus its descendants.
        let mut subtree: Vec<Id> = from.descendants(id);
        subtree.insert(0, id);
        let sub_set: std::collections::HashSet<Id> = subtree.iter().copied().collect();

        // Id map: the components plus every output of the timeline nodes of the subtree (sketches, planes,
        // points, axes, bodies).
        let mut map: HashMap<Id, Id> = HashMap::new();
        for &c in &subtree {
            map.insert(c, dest.alloc_id());
        }
        let sub_nodes: Vec<FeatureNode> = from.timeline.iter().filter(|n| n.parent.is_some_and(|p| sub_set.contains(&p))).cloned().collect();
        for n in &sub_nodes {
            for out in from.node_output_ids(&n.kind) {
                map.entry(out).or_insert_with(|| dest.alloc_id());
            }
            map.entry(n.id).or_insert_with(|| dest.alloc_id());
        }

        // Clone the components; a parent inside the subtree maps to its clone and the subtree root maps to
        // `target_parent`.
        for &c in &subtree {
            let Some(src) = from.components.iter().find(|x| x.id == c).cloned() else { continue };
            let parent = if c == id { Some(target_parent) } else { src.parent.and_then(|p| map.get(&p).copied()) };
            dest.components.push(Component { id: map[&c], name: src.name.clone(), kind: src.kind, parent, transform: src.transform, visible: src.visible, grounded: src.grounded });
        }

        // Topological name map: a cloned feature has a different id, so its faces carry different names.
        //
        // Measured on a thread: a copied part went red with "thread crest not found" and drifted, because
        // `Thread.edge` in the copy still pointed at an edge of the original. Ids have been remapped for a long
        // time, but a face name is derived from the recipe (which feature, in what role, from which entity)
        // rather than from an id, and on a copy that recipe reads differently. Shell, draft, fillet and a hole
        // on a face hold references of the same kind.
        //
        // The names depend on each other: an edge is named by its pair of faces and a fillet by its edge. The
        // map is therefore built in passes to a fixed point rather than in one list: the order in the table
        // guarantees nothing, and a missed name would silently leave a reference to somebody else's
        // geometry.
        let mut nmap: HashMap<u32, u32> = HashMap::new();
        loop {
            let mut progress = false;
            for (i, gn) in from.names.faces().iter().enumerate() {
                let desc = crate::names::NAMED | i as u32;
                if nmap.contains_key(&desc) {
                    continue;
                }
                // A name belonging to a feature outside the subtree is not this map's business: an outward
                // reference is cut anyway.
                let Some(&nfeat) = map.get(&gn.feature) else { continue };
                // `src` comes in two kinds: a sketch entity, whose id is the same in the clone, and the name of
                // another face or edge (a fillet, a corner patch, a pattern image), which waits for its own
                // mapping.
                let nsrc = if gn.src <= u32::MAX as Id && crate::names::NameTable::is_named(gn.src as u32) {
                    match nmap.get(&(gn.src as u32)) {
                        Some(&v) => v as Id,
                        None => continue,
                    }
                } else {
                    gn.src
                };
                let nd = dest.names.intern_face(crate::names::GeoName { feature: nfeat, role: gn.role, src: nsrc, split: gn.split });
                nmap.insert(desc, nd);
                progress = true;
            }
            for (i, en) in from.names.edges().iter().enumerate() {
                let desc = crate::names::EDGE | i as u32;
                if nmap.contains_key(&desc) {
                    continue;
                }
                let (Some(&a), Some(&b)) = (nmap.get(&en.faces[0]), nmap.get(&en.faces[1])) else { continue };
                let nd = dest.names.intern_edge(crate::names::EdgeName::new(a, b, en.index));
                nmap.insert(desc, nd);
                progress = true;
            }
            if !progress {
                break;
            }
        }

        // Clone the sketches (remapping `source` and `plane`) and build the contour map used by `profile`.
        let mut cmap: HashMap<Id, Id> = HashMap::new();
        for n in &sub_nodes {
            if let FeatureKind::Sketch { sketch } = n.kind {
                if let Some(si) = from.sketch_index(sketch) {
                    let old_cids = from.sketches[si].contour_ids.clone();
                    let mut sk = from.sketches[si].clone();
                    sk.id = map[&sketch];
                    sk.contour_ids.clear();
                    if let Some(src) = sk.source {
                        sk.source = Some(map.get(&src).copied().unwrap_or(src));
                    }
                    sk.plane = match sk.plane {
                        crate::feature::SketchPlane::Datum(p) => crate::feature::SketchPlane::Datum(map.get(&p).copied().unwrap_or(p)),
                        // The anchor face is addressed by name rather than by id, and on a copy that name
                        // differs (see `nmap`).
                        crate::feature::SketchPlane::Face(b, mut f) => {
                            if let Some(&nd) = nmap.get(&f.id) {
                                f.id = nd;
                            }
                            crate::feature::SketchPlane::Face(map.get(&b).copied().unwrap_or(b), f)
                        }
                        other => other,
                    };
                    dest.sketches.push(sk);
                    let ni = dest.sketches.len() - 1;
                    dest.regen_sketch(ni);
                    let new_cids = dest.sketches[ni].contour_ids.clone();
                    for (k, oc) in old_cids.iter().enumerate() {
                        if let Some(nc) = new_cids.get(k) {
                            cmap.insert(*oc, *nc);
                        }
                    }
                }
            }
        }

        // Clone the datum planes, remapping their definitions.
        for n in &sub_nodes {
            if let FeatureKind::Plane { plane } = n.kind {
                if let Some(pl) = from.planes.iter().find(|p| p.id == plane).cloned() {
                    let mut np = pl;
                    np.id = map[&plane];
                    np.def = match np.def {
                        // The anchor face goes by name, which differs for a cloned feature.
                        PlaneDef::OffsetFace { body, mut face, dist } => {
                            if let Some(&nd) = nmap.get(&face.id) {
                                face.id = nd;
                            }
                            PlaneDef::OffsetFace { body: map.get(&body).copied().unwrap_or(body), face, dist }
                        }
                        PlaneDef::OffsetPlane { plane, dist } => PlaneDef::OffsetPlane { plane: map.get(&plane).copied().unwrap_or(plane), dist },
                        other => other,
                    };
                    dest.planes.push(np);
                }
            }
        }
        // Clone the datum points.
        for n in &sub_nodes {
            if let FeatureKind::DatumPoint { point } = n.kind {
                if let Some(dp) = from.datum_points.iter().find(|p| p.id == point).cloned() {
                    dest.datum_points.push(DatumPoint { id: map[&point], ..dp });
                }
            }
        }
        // Clone the datum axes, remapping their definitions.
        for n in &sub_nodes {
            if let FeatureKind::DatumAxis { axis } = n.kind {
                if let Some(da) = from.datum_axes.iter().find(|a| a.id == axis).cloned() {
                    let mut na = da;
                    na.id = map[&axis];
                    na.def = match na.def {
                        AxisDef::TwoPoints { a, b } => AxisDef::TwoPoints { a: map.get(&a).copied().unwrap_or(a), b: map.get(&b).copied().unwrap_or(b) },
                        AxisDef::FromEdge { body, edge } => {
                            AxisDef::FromEdge { body: map.get(&body).copied().unwrap_or(body), edge: nmap.get(&edge).copied().unwrap_or(edge) }
                        }
                        AxisDef::FromFace { body, face } => {
                            AxisDef::FromFace { body: map.get(&body).copied().unwrap_or(body), face: nmap.get(&face).copied().unwrap_or(face) }
                        }
                        other => other,
                    };
                    dest.datum_axes.push(na);
                }
            }
        }

        // Clone the timeline nodes in their original order, remapping the references and the profiles and
        // marking the bodies dirty.
        let mut insert_at = dest.timeline.len();
        for n in &sub_nodes {
            let mut kind = n.kind.clone();
            kind.remap_ids(&map);
            kind.remap_names(&nmap);
            kind.remap_profile(&cmap);
            let parent = n.parent.and_then(|p| map.get(&p).copied());
            let dirty = kind.body().is_some();
            dest.timeline.insert(insert_at, FeatureNode { id: map[&n.id], name: n.name.clone(), kind, parent, dirty, suppressed: n.suppressed });
            insert_at += 1;
        }

        // Clone the parametric body dimensions (`feat_dims` under the new body ids).
        let dim_clones: Vec<(Id, std::collections::HashMap<String, String>)> =
            from.feat_dims.iter().filter_map(|(k, v)| map.get(k).map(|nk| (*nk, v.clone()))).collect();
        for (nk, v) in dim_clones {
            dest.feat_dims.insert(nk, v);
        }

        Some(map[&id])
    }

    /// Owning component of a sketch, by id, through its timeline node. `None` means the sketch has no node
    /// yet.
    pub fn sketch_owner(&self, sketch: Id) -> Option<Id> {
        self.timeline
            .iter()
            .find(|n| matches!(n.kind, crate::feature::FeatureKind::Sketch { sketch: s } if s == sketch))
            .and_then(|n| n.parent)
    }

    /// Owning component of a datum plane, through its `Plane` node; `None` when it was not found.
    pub fn plane_owner(&self, plane: Id) -> Option<Id> {
        self.timeline
            .iter()
            .find(|n| matches!(n.kind, crate::feature::FeatureKind::Plane { plane: p } if p == plane))
            .and_then(|n| n.parent)
    }

    /// Add a box primitive as a dirty node. Returns the body id.
    pub fn add_box(&mut self, dx: f64, dy: f64, dz: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-box".into(), kind: FeatureKind::Box3 { dx, dy, dz, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a cylinder primitive as a dirty node. Returns the body id.
    pub fn add_cylinder(&mut self, r: f64, h: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-cylinder".into(), kind: FeatureKind::Cylinder { r, h, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a sphere primitive as a dirty node. Returns the body id.
    pub fn add_sphere(&mut self, r: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-sphere".into(), kind: FeatureKind::Sphere { r, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a cone primitive as a dirty node. Returns the body id.
    pub fn add_cone(&mut self, r1: f64, r2: f64, h: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-cone".into(), kind: FeatureKind::Cone { r1, r2, h, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a torus primitive as a dirty node. Returns the body id.
    pub fn add_torus(&mut self, major: f64, minor: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-torus".into(), kind: FeatureKind::Torus { major, minor, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a regular prism primitive as a dirty node. Returns the body id.
    pub fn add_prism(&mut self, r: f64, n: u32, h: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-prism".into(), kind: FeatureKind::Prism { r, n, h, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Operation on body `src` using a specific sketch contour `profile` (`op` is 0 cut, 1 boss, 2
    /// intersection). How far the tool goes is `ext`; `down` behaves as for an extrude when the extent is not
    /// a through one.
    #[allow(clippy::too_many_arguments)]
    pub fn add_combine_on(&mut self, src: Id, sketch: Id, profile: Id, height: f64, op: u8, ext: crate::feature::Extent, down: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let name = ["feat-name-combine-cut", "feat-name-combine-boss", "feat-name-combine-intersect"][(op as usize).min(2)].to_string();
        self.push_timeline(FeatureNode { id: body, name, kind: FeatureKind::Combine { src, sketch, profiles: vec![profile], height, op, extent: ext, down, fill: Vec::new(), body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Operation on body `src` using several contours `profiles` within one node: one boolean, one body.
    #[allow(clippy::too_many_arguments)]
    pub fn add_combine_multi_op(&mut self, src: Id, sketch: Id, profiles: Vec<Id>, height: f64, op: u8, ext: crate::feature::Extent, down: f64, fill: Vec<Id>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let name = ["feat-name-combine-cut", "feat-name-combine-boss", "feat-name-combine-intersect"][(op as usize).min(2)].to_string();
        self.push_timeline(FeatureNode { id: body, name, kind: FeatureKind::Combine { src, sketch, profiles, height, op, extent: ext, down, fill, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Add a fillet on the edges of body `src` (an empty `edges` means every edge). Returns the result id.
    /// Fillet from a query reference: "every edge of this face" instead of a snapshot of a list.
    ///
    /// A separate entry point rather than a flag on the existing one: a descriptive reference has nothing to
    /// capture with an edge snapshot (`capture_edge_refs`), already referring to today's geometry.
    pub fn add_fillet_ref(&mut self, src: Id, radius: f64, edges: crate::refs::Ref) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-fillet".into(), kind: FeatureKind::Fillet { src, radius, edges, at_vertices: Vec::new(), body }, parent, dirty: true, suppressed: false });
        body
    }

    /// The same for a chamfer.
    pub fn add_chamfer_ref(&mut self, src: Id, dist: f64, edges: crate::refs::Ref) -> Id {
        use crate::feature::{ChamferMode, FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-chamfer".into(),
            kind: FeatureKind::Chamfer { src, dist, edges, mode: ChamferMode::Symmetric, d2: 0.0, flip: false, ref_face: 0, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    pub fn add_fillet(&mut self, src: Id, radius: f64, edges: Vec<u32>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.capture_edge_refs(body, src, &edges);
        self.push_timeline(FeatureNode { id: body, name: "feat-name-fillet".into(), kind: FeatureKind::Fillet { src, radius, edges: crate::refs::Ref::picks(&edges), at_vertices: Vec::new(), body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Patch: a surface spanned over the selected edges of a body. The source is not consumed.
    pub fn add_patch(&mut self, src: Id, edges: crate::refs::Ref, tangent: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let picked = edges.query.picked_descs();
        if !picked.is_empty() {
            self.capture_edge_refs(body, src, &picked);
        }
        self.push_timeline(FeatureNode { id: body, name: "feat-name-patch".into(), kind: FeatureKind::Patch { src, edges, tangent, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Replace faces of a body with a surface: the node that stitches the surface layer into the timeline.
    ///
    /// The faces come from a query, so after the base rebuilds the node replaces the same place rather than
    /// yesterday's numbers. Both inputs are consumed: what continues down the timeline is the result, and
    /// anything can be built on it — fillets, holes, a shell.
    pub fn add_surface_replace(&mut self, src: Id, faces: crate::refs::Ref, surface: Id) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-surface-replace".into(), kind: FeatureKind::SurfaceReplace { src, faces, surface, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Copy faces into a separate surface: the bridge from the parametric model into the surface layer.
    ///
    /// The faces come from a query — "every face of this feature", "every parallel one", a specific pick — so
    /// the copy follows its base like everything else. The source is not consumed.
    /// Trim a surface: `src` is cut by body `tool` and the piece at point `keep` is retained.
    pub fn add_trim(&mut self, src: Id, tool: Id, keep: [f64; 3]) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-trim".into(), kind: FeatureKind::Trim { src, tool, keep, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Stitch sheets: `parts` are the sheet bodies and `tol` is the edge coincidence tolerance.
    pub fn add_stitch(&mut self, parts: Vec<Id>, tol: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-stitch".into(), kind: FeatureKind::Stitch { parts, tol, body }, parent, dirty: true, suppressed: false });
        body
    }

    pub fn add_face_copy(&mut self, src: Id, faces: crate::refs::Ref) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-face-copy".into(), kind: FeatureKind::FaceCopy { src, faces, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Variable fillet specified at vertices: a base `radius` plus a "vertex to radius" table.
    ///
    /// The vertices are taken by name (descriptors from `vertex_pool`) and stored as references: a vertex name
    /// is derived from its edges, so editing the neighbours does not disturb it. An empty table means a
    /// constant radius, which is not a special case but the same expression with zero entries.
    pub fn add_fillet_at_vertices(&mut self, src: Id, radius: f64, edges: crate::refs::Ref, at: Vec<(u32, f64)>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let picked = edges.query.picked_descs();
        if !picked.is_empty() {
            self.capture_edge_refs(body, src, &picked); // A hand-picked set is translated through the rename
                                                        // map.
        }
        let at_vertices: Vec<(crate::refs::Ref, f64)> = at.into_iter().map(|(desc, r)| (crate::refs::Ref::one(desc, crate::refs::Fingerprint::default()), r)).collect();
        self.push_timeline(FeatureNode { id: body, name: "feat-name-fillet".into(), kind: FeatureKind::Fillet { src, radius, edges, at_vertices, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Parametric body-to-body boolean: `op` applied to bodies `a` (the base) and `b` (the tool). Both are
    /// consumed and hidden, leaving the result visible. The node is appended to the end of the timeline, after
    /// both operands.
    pub fn add_body_boolean(&mut self, a: Id, b: Id, op: u8) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let name = ["feat-name-body-cut", "feat-name-body-union", "feat-name-body-intersect"][(op as usize).min(2)].to_string();
        self.push_timeline(FeatureNode { id: body, name, kind: FeatureKind::BodyBoolean { a, b, op, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Chamfer with a mode: `TwoDist` (d1 and d2) or `DistAngle` (setback d1 plus angle d2 in degrees); `flip`
    /// selects the reference face. `ref_face` is the persistent id of a manually chosen reference face (zero
    /// selects it automatically from `flip`). Asymmetry requires an explicit edge selection; for "every edge"
    /// the kernel falls back to a symmetric chamfer.
    #[allow(clippy::too_many_arguments)]
    pub fn add_chamfer_ex(&mut self, src: Id, dist: f64, d2: f64, mode: crate::feature::ChamferMode, flip: bool, ref_face: u32, edges: Vec<u32>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.capture_edge_refs(body, src, &edges);
        self.push_timeline(FeatureNode { id: body, name: "feat-name-chamfer".into(), kind: FeatureKind::Chamfer { src, dist, edges: crate::refs::Ref::picks(&edges), mode, d2, flip, ref_face, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Shell, with the side the wall goes to.
    pub fn add_shell_mode(&mut self, src: Id, thickness: f64, faces: Vec<u32>, side: crate::feature::ShellSide) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-shell".into(), kind: FeatureKind::Shell { src, thickness, faces: crate::refs::Ref::picks(&faces), side, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Offset a face: planar face `face` of body `src` moves by `dist` along its own normal.
    ///
    /// Parametric like everything else: `dist` lives as an expression (the `dist` feature dimension) and the
    /// face reference goes by name, so it survives edits higher up the timeline.
    pub fn add_push_face(&mut self, src: Id, face: crate::feature::FaceKey, dist: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-push-face".into(), kind: FeatureKind::PushFace { src, face: crate::refs::Ref::one(face.id, crate::refs::Fingerprint { centroid: face.centroid, normal: face.normal }), dist, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Delete faces and heal. The face references go by name and therefore survive edits higher up the
    /// timeline.
    pub fn add_remove_face(&mut self, src: Id, faces: Vec<crate::feature::FaceKey>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-remove-face".into(), kind: FeatureKind::RemoveFace { src, faces: crate::refs::Ref::picks(&faces.iter().map(|f| f.id).collect::<Vec<_>>()), body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Thicken a face: face `face` of body `src` becomes a plate of `thickness` as a separate body. The source
    /// stays — a skin is made from a face of the housing while the housing remains. The thickness is parametric
    /// (the `thickness` feature dimension).
    /// The home part of a sheet: the body the sheet grew out of (a face copy, a patch, a trim, a stitch).
    ///
    /// Needed by thicken: the plate has to return into the part the surface was taken from, or the part is left
    /// holding two bodies, visible on screen as a differently coloured piece.
    fn sheet_origin_body(&self, sheet: Id) -> Id {
        use crate::feature::FeatureKind as FK;
        let mut cur = sheet;
        for _ in 0..64 {
            let Some(node) = self.timeline.iter().find(|n| n.kind.bodies().contains(&cur)) else { return 0 };
            let src = match node.kind {
                FK::FaceCopy { src, .. } | FK::Patch { src, .. } | FK::Trim { src, .. } => src,
                FK::Stitch { ref parts, .. } => parts.first().copied().unwrap_or(0),
                _ => return 0,
            };
            if src == 0 {
                return 0;
            }
            // A solid was reached: that is what to weld onto.
            if !self.bodies.iter().any(|b| b.id == src && b.sheet) {
                return src;
            }
            cur = src;
        }
        0
    }

    pub fn add_thicken(&mut self, src: Id, face: u32, thickness: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        // A sheet returns into its own part. For a face of a part the plate is welded onto the source anyway;
        // for a sheet the source is the sheet itself, and without this the part was left holding a second body.
        //
        // It also welds onto the live body rather than the one the sheet was taken from: while the surface was
        // being edited the part may have moved on by several operations, and welding onto the old body would
        // leave two bodies in the part, the previous one and the new one.
        let join = if self.bodies.iter().any(|b| b.id == src && b.sheet) {
            let origin = self.sheet_origin_body(src);
            if origin == 0 { 0 } else { self.live_body(origin) }
        } else {
            0
        };
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-thicken".into(), kind: FeatureKind::Thicken { src, face, thickness, join, body }, parent, dirty: true, suppressed: false });
        // A face snapshot serves as a witness for when its id changes (a positional number becoming a
        // name).
        self.capture_face_refs(body, src, &[face]);
        body
    }

    /// Split faces by a plane without cutting the body: the body stays one and gains faces, which marks out a
    /// region rather than breaking the part apart. The plane is a reference (a datum or a world plane) plus a
    /// parametric offset, as for a body split.
    pub fn add_split_face(&mut self, src: Id, plane: u8, datum: Id, offset: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-split-face".into(), kind: FeatureKind::SplitFace { src, plane, datum, offset, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Split a body by a plane. The plane is a reference (`datum`, or the world plane in `plane`), as for a
    /// mirror, which makes the split associative; `offset` shifts along the normal and is parametric.
    /// `pieces` is how many pieces the split produces. The count is computed by the caller from the live B-rep;
    /// it cannot be derived from the plane alone, since one plane cuts a U-shaped part into three.
    ///
    /// Returns the ids of every piece, from bottom to top along the normal; the first also serves as the node
    /// id.
    pub fn add_split_body(&mut self, src: Id, plane: u8, datum: Id, offset: f64, pieces: usize) -> Vec<Id> {
        use crate::feature::{FeatureKind, FeatureNode};
        assert!(pieces >= 2, "split-single-piece");
        let bodies: Vec<Id> = (0..pieces).map(|_| self.alloc_id()).collect();
        let parent = Some(self.body_parent());
        let id = bodies[0];
        self.push_timeline(FeatureNode { id, name: "feat-name-split-body".into(), kind: FeatureKind::SplitBody { src, plane, datum, offset, bodies: bodies.clone() }, parent, dirty: true, suppressed: false });
        bodies
    }

    pub fn add_draft(&mut self, src: Id, faces: Vec<u32>, neutral: u32, angle: f64, flip: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-draft".into(), kind: FeatureKind::Draft { src, faces: crate::refs::Ref::picks(&faces), neutral: crate::refs::Ref::one(neutral, crate::refs::Fingerprint::default()), angle, flip, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Linear pattern as a full 3D grid: three independent directions (count by count2 by count3).
    #[allow(clippy::too_many_arguments)]
    pub fn add_linear_array_grid3(&mut self, src: Id, dx: f64, dy: f64, dz: f64, count: u32, dx2: f64, dy2: f64, dz2: f64, count2: u32, dx3: f64, dy3: f64, dz3: f64, count3: u32) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-linear-array".into(), kind: FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Circular pattern about a chosen axis: `axis` is a datum axis id (resolved during regenerate) or zero for
    /// world Z.
    pub fn add_circular_array_axis(&mut self, src: Id, count: u32, angle: f64, axis: Id) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-circular-array".into(), kind: FeatureKind::CircularArray { src, count, angle, axis, body }, parent, dirty: true, suppressed: false });
        body
    }

    pub fn add_mirror(&mut self, src: Id, plane: u8, keep: bool, datum: Id) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-mirror".into(), kind: FeatureKind::Mirror { src, plane, keep, datum, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Translate and rotate body `src` by matrix `mat` (3x4) as a parametric feature, moving the B-rep.
    pub fn add_move(&mut self, src: Id, mat: [f64; 12]) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-move".into(), kind: FeatureKind::Move { src, mat, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Hole with a type: `kind` is 0 for a plain hole, 1 for a counterbore and 2 for a countersink; `dia2` and
    /// `depth2` are the parameters of the recess.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hole_typed(&mut self, src: Id, face: crate::feature::FaceKey, diameter: f64, depth: f64, kind: u8, dia2: f64, depth2: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let (point, normal) = (face.centroid, face.normal);
        // A pick gives a specific face, which becomes an `Id` query. The fingerprint travels with it not for
        // matching but so that a refusal can one day say where that face was when it was picked.
        let face = crate::refs::Ref::one(face.id, crate::refs::Fingerprint { centroid: face.centroid, normal: face.normal });
        self.push_timeline(FeatureNode { id: body, name: "feat-name-hole".into(), kind: FeatureKind::Hole { src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch: 0, flip: false, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Holes at sketch points: one hole per isolated point of `sketch`. `src` is the stock body, drilled along
    /// the sketch normal (`flip` reverses it). The hole parameters (`kind`, `dia2`, `depth2`) are as for a
    /// single hole. One timeline node covers every hole.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hole_from_sketch(&mut self, src: Id, sketch: Id, diameter: f64, depth: f64, kind: u8, dia2: f64, depth2: f64, flip: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        // The sketch-driven form picks no face at all: the reference is empty and never reaches
        // resolution.
        let face = crate::refs::Ref::one(0, crate::refs::Fingerprint::default());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-holes-sketch".into(), kind: FeatureKind::Hole { src, face, point: [0.0; 3], normal: [0.0, 0.0, 1.0], diameter, depth, kind, dia2, depth2, sketch, flip, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Thread on a cylinder (external) or in a hole (internal) of body `src`, associated with the circular edge
    /// `edge`, whose rim supplies the axis and the radius from `regen_edges` on every rebuild. The real turn
    /// geometry (a helix swept and cut) is removed from `src`, producing a new body.
    #[allow(clippy::too_many_arguments)]
    pub fn add_thread(&mut self, src: Id, edge: u32, spec: crate::thread::ThreadSpec, length: f64, lead_in: f64, lead_out: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-thread".into(), kind: FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Auger: a helical flight on shaft `src`, associated with the circular edge `edge`.
    pub fn add_auger(&mut self, src: Id, edge: u32, spec: crate::thread::AugerSpec, length: f64, lead_in: f64, lead_out: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-auger".into(), kind: FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Mark a sketch node dirty, so that editing the sketch rebuilds its consumers on the next
    /// regenerate.
    pub fn mark_sketch_dirty(&mut self, sid: Id) {
        use crate::feature::FeatureKind;
        for nd in &mut self.timeline {
            if matches!(nd.kind, FeatureKind::Sketch { sketch } if sketch == sid) {
                nd.dirty = true;
            }
        }
    }

    /// Mark a node dirty by id, after editing the parameters of a feature.
    pub fn mark_node_dirty(&mut self, id: Id) {
        if let Some(i) = self.timeline_index(id) {
            self.timeline[i].dirty = true;
        }
    }

    /// Bodies whose geometry may have changed between `self` and `other`, by comparing recipe fingerprints.
    ///
    /// Needed by undo: a snapshot carries the meshes but not the live B-rep (`Shape` is not cloned), so after a
    /// restore the kernel cache has to be rebuilt — but only for the bodies that actually changed rather than
    /// forcibly across the whole project, which on an assembly of a thousand imports takes tens of seconds.
    pub fn changed_bodies_vs(&self, other: &Project) -> Vec<Id> {
        let mut out = Vec::new();
        for n in &self.timeline {
            let outs = n.kind.bodies();
            if outs.is_empty() {
                continue;
            }
            let mine = self.node_recipe_key(n.id);
            let theirs = if other.timeline.iter().any(|o| o.id == n.id) { other.node_recipe_key(n.id) } else { 0 };
            if mine != theirs || theirs == 0 {
                out.extend(outs);
            }
        }
        // Bodies absent from `self` entirely (the node exists only in the other state) are rebuilt as
        // well.
        for n in &other.timeline {
            if !self.timeline.iter().any(|m| m.id == n.id) {
                out.extend(n.kind.bodies());
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Mark as dirty the nodes that depend on datums (a plane, an axis, a point): their geometry resolves from
    /// the datum and has to travel with it.
    ///
    /// Forcing a full rebuild after editing a datum costs tens of seconds of freeze on an assembly of a
    /// thousand bodies, while only a handful of nodes depend on the datum. A node counts as dependent when it
    /// is a datum node itself, when its input sketch sits on a datum plane, or when it references a datum axis
    /// or point.
    pub fn mark_datum_consumers_dirty(&mut self) {
        use crate::feature::{FeatureKind, SketchPlane};
        let on_datum = |p: &Self, sid: Id| -> bool {
            p.sketches.iter().find(|s| s.id == sid).is_some_and(|s| matches!(s.plane, SketchPlane::Datum(_)))
        };
        let ids: Vec<Id> = self
            .timeline
            .iter()
            .filter(|n| match n.kind {
                FeatureKind::Plane { .. } | FeatureKind::DatumPoint { .. } | FeatureKind::DatumAxis { .. } => true,
                FeatureKind::Revolve { sketch, axis_datum, .. } => axis_datum != 0 || on_datum(self, sketch),
                _ => n.kind.inputs().into_iter().any(|inp| on_datum(self, inp)),
            })
            .map(|n| n.id)
            .collect();
        for id in ids {
            self.mark_node_dirty(id);
        }
    }

    pub fn mark_external_consumers_dirty(&mut self) {
        let mut ids = Vec::new();
        for nd in &self.timeline {
            let Some(owner) = nd.parent else { continue };
            if nd.kind.inputs().into_iter().any(|inp| self.sketch_plane_body(inp).and_then(|pb| self.body_owner(pb)).is_some_and(|bo| bo != owner)) {
                ids.push(nd.id);
            }
        }
        for id in ids {
            self.mark_node_dirty(id);
        }
    }

    /// Owning component of body `body`: the parent of the feature that built it. `None` for an import with no
    /// feature.
    pub fn body_owner(&self, body: Id) -> Option<Id> {
        self.timeline.iter().find(|n| n.kind.bodies().contains(&body)).and_then(|n| n.parent)
    }

    /// What a body is for the purpose of export: one policy for STEP and STL.
    ///
    /// `has_shape` says whether the body has a live B-rep in the kernel cache (`App.shapes`), which only the
    /// caller knows.
    ///
    /// Without a shared policy, STEP silently skipped a body without a B-rep while STL just as silently
    /// exported its last mesh, so two exports of one project differed in content. Both exports now follow this
    /// classification and report it the same way.
    pub fn export_kind(&self, body: Id, has_shape: bool) -> ExportKind {
        if has_shape {
            ExportKind::Brep
        } else if self.timeline.iter().any(|n| n.kind.bodies().contains(&body)) {
            ExportKind::Stale // A recipe exists without a B-rep: a failed rebuild, with the last good mesh
                              // still visible.
        } else {
            ExportKind::MeshOnly // A body with no timeline node can only be an imported mesh, which never had
                                 // a B-rep.
        }
    }

    /// What created object `id`: the timeline node that introduced it (a body, a sketch, a datum).
    ///
    /// `id` may be a timeline node itself, in which case it is returned as is. That gives the properties panel
    /// one question for every kind of selection rather than a separate lookup per kind.
    pub fn creator_of(&self, id: Id) -> Option<Id> {
        self.timeline.iter().find(|n| n.id == id || n.kind.declares().contains(&id)).map(|n| n.id)
    }

    /// What depends on it: the timeline nodes built on object `id`, or on whatever node `id` introduced.
    ///
    /// The answer comes in timeline order and without duplicates. One source for the properties panel and for
    /// every "what breaks if this is deleted" question: implemented as a loop in the interface it knew about
    /// bodies only, and dependencies on a sketch or a datum never reached the panel.
    pub fn dependents_of(&self, id: Id) -> Vec<Id> {
        // The objects the subject accounts for: itself plus everything it introduces, when it is a node.
        let mut own: Vec<Id> = vec![id];
        if let Some(n) = self.timeline.iter().find(|n| n.id == id) {
            own.extend(n.kind.declares());
        }
        self.timeline.iter().filter(|n| n.id != id && n.kind.inputs().iter().any(|i| own.contains(i))).map(|n| n.id).collect()
    }

    /// Bodies consumed by active modifier features, which are hidden so that only the result of the chain is
    /// visible. It accounts for the rollback bar: features below the bar are not built and therefore consume no
    /// source. One source for both the core and the interface.
    pub fn consumed_bodies(&self) -> std::collections::HashSet<Id> {
        let limit = self.rollback.unwrap_or(usize::MAX);
        self.timeline.iter().enumerate().filter(|(ti, _)| *ti < limit).flat_map(|(_, n)| n.kind.consumed()).collect()
    }

    /// Active body of context `ctx`: the last unconsumed body belonging to `ctx` directly (`parent == ctx`).
    ///
    /// A cut or an operation on a body takes the body of the current part rather than the globally last one or
    /// one of its subcomponents; otherwise an operation in one part would attach to another part's body and
    /// that part would appear to vanish.
    pub fn active_body(&self, ctx: Id) -> Option<Id> {
        self.active_body_before(ctx, usize::MAX)
    }

    /// The active body as seen by a node at position `upto`: only what is built above it.
    ///
    /// In a build history a node may not rest on something that does not exist yet at its point. `active_body`
    /// looks at the whole timeline, so a mirrored part received a body produced by a node below it: a boolean
    /// consumed the source body and produced a new one, the mirror saw that new one, and it had not been built
    /// yet. That is why one document could end up with different sets of bodies — building along the way left
    /// the mirror on the old body and it survived, while building once at the end made it fail.
    pub fn active_body_before(&self, ctx: Id, upto: usize) -> Option<Id> {
        let consumed: std::collections::HashSet<Id> = self
            .timeline
            .iter()
            .enumerate()
            .filter(|(ti, _)| *ti < self.rollback.unwrap_or(usize::MAX) && *ti < upto)
            .flat_map(|(_, n)| n.kind.consumed())
            .collect();
        // The rollback bar counts: bodies below it are not built (their mesh was removed during regenerate)
        // and cannot be active. Without this a cut or a shell would target a rolled-back body with no mesh and
        // the feature would fail.
        let limit = self.rollback.unwrap_or(usize::MAX).min(upto);
        self.timeline
            .iter()
            .take(limit)
            .rev()
            .flat_map(|n| n.kind.bodies())
            .find(|b| !consumed.contains(b) && self.body_owner(*b) == Some(ctx))
    }

    /// One part is one body. Finish a base (material-adding) feature: when the part already has a result body,
    /// `new_body` is united into it; the first material feature becomes the seed and is returned as is.
    ///
    /// This way every base feature (extrude, revolve, sweep, loft, primitives) carries one body for the part
    /// instead of producing independent ones. Disconnected pieces form a single disconnected union, which is
    /// normal. Returns the resulting body, either the boolean result or the seed, and inserts a node into the
    /// timeline.
    ///
    /// It is called after a material base feature, by the interface and the tests, rather than from inside the
    /// adders: that keeps the low-level adders composable while the "one part is one body" invariant is applied
    /// at the orchestration level.
    pub fn finish_base_body(&mut self, new_body: Id, op: u8) -> Id {
        let Some(ctx) = self.body_owner(new_body) else { return new_body };
        let consumed = self.consumed_bodies();
        let limit = self.rollback.unwrap_or(usize::MAX);
        // The previous result body of the same part: unconsumed, above the rollback bar, and not the one just
        // added.
        let bodies: Vec<Id> = self.timeline.iter().take(limit).flat_map(|n| n.kind.bodies()).collect();
        let prior = bodies.iter().rev().copied().find(|b| *b != new_body && !consumed.contains(b) && self.body_owner(*b) == Some(ctx));
        match prior {
            Some(p) => self.add_body_boolean(p, new_body, op),
            None => new_body,
        }
    }

    /// The timeline nodes of one user-level operation, so the tree can show them as a single row.
    ///
    /// An operation from a sketch is one combine or extrude node, with every contour in `profiles`. A revolve,
    /// sweep or loft that adds to a part is the feature plus the immediately following body boolean from
    /// `finish_base_body`, and the two collapse into one row. Everything else is a single node.
    pub fn feature_op_span(&self, first_id: Id) -> Vec<Id> {
        use crate::feature::FeatureKind as FK;
        let Some(pos) = self.timeline.iter().position(|n| n.id == first_id) else { return vec![first_id] };
        let Some(body) = self.timeline[pos].kind.body() else { return vec![first_id] };
        // Absorb the next node when it is the body boolean joining this very body (a revolve, sweep or loft
        // followed by `finish_base_body`).
        if let Some(next) = self.timeline.get(pos + 1) {
            if let FK::BodyBoolean { a, b, .. } = next.kind {
                if a == body || b == body {
                    return vec![first_id, next.id];
                }
            }
        }
        vec![first_id]
    }

    /// Rollback bar: build only the first `n` timeline nodes (`None` builds everything). Marks everything
    /// dirty.
    pub fn set_rollback(&mut self, n: Option<usize>) {
        self.rollback = n.filter(|&n| n < self.timeline.len());
        for nd in &mut self.timeline {
            nd.dirty = true;
        }
    }

    /// Append a node to the timeline, or insert it at the rollback bar when one is active, which is how
    /// building between features works: the new node becomes the last active one and the bar moves past it.
    /// Every `add_*` goes through this, which makes the rollback bar genuinely the place where building
    /// continues. Returns the node id.
    pub(super) fn push_timeline(&mut self, node: crate::feature::FeatureNode) -> Id {
        let id = node.id;
        match self.rollback {
            Some(r) if r <= self.timeline.len() => {
                self.timeline.insert(r, node);
                self.rollback = Some(r + 1);
            }
            _ => self.timeline.push(node),
        }
        id
    }

    /// Suppress or unsuppress feature `ti`. The node is marked dirty, so regenerate removes its body from view
    /// (cascading to its consumers) or rebuilds it when it is switched back on. Returns the new state.
    pub fn set_feature_suppressed(&mut self, ti: usize, on: bool) -> bool {
        if let Some(n) = self.timeline.get_mut(ti) {
            n.suppressed = on;
            n.dirty = true;
        }
        // Everything below becomes dirty too. Marking a single node rebuilds that node while the features
        // resting on its body keep the previous shape, so the model on screen stops being the result of its own
        // timeline; switching the feature back on then appears to change nothing until a full rebuild is
        // forced.
        //
        // The rollback bar (`set_rollback`) has always marked everything, and suppression is the same kind of
        // structural edit.
        //
        // Marking runs from the node downwards rather than over everything: the timeline is ordered, and what
        // stands above cannot rest on the suppressed feature by construction.
        for n in self.timeline.iter_mut().skip(ti + 1) {
            n.dirty = true;
        }
        on
    }

    /// Output ids of a node, that is, what it produces: a body plus a datum or a sketch (plane, axis, point,
    /// sketch). Used by the ordering check.
    fn node_output_ids(&self, kind: &crate::feature::FeatureKind) -> Vec<Id> {
        use crate::feature::FeatureKind as FK;
        let mut o: Vec<Id> = kind.bodies();
        match *kind {
            FK::Sketch { sketch } => o.push(sketch),
            FK::Plane { plane } => o.push(plane),
            FK::DatumAxis { axis } => o.push(axis),
            FK::DatumPoint { point } => o.push(point),
            _ => {}
        }
        o
    }

    /// Whether the node order is valid: every reference of a node (a body, a sketch, a datum) is produced
    /// before it, with no forward references. Datum dependencies count too (an offset plane above its source, a
    /// sketch on a datum and so on).
    fn order_valid(&self, order: &[usize]) -> bool {
        let all: std::collections::HashSet<Id> = order.iter().flat_map(|&i| self.node_output_ids(&self.timeline[i].kind)).collect();
        let mut produced: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for &idx in order {
            let k = &self.timeline[idx].kind;
            for req in self.node_required_refs(k) {
                if all.contains(&req) && !produced.contains(&req) {
                    return false; // A reference to something not yet built above it in the timeline.
                }
            }
            for out in self.node_output_ids(k) {
                produced.insert(out);
            }
        }
        true
    }

    /// Whether moving `from` to `to` would be valid, without mutating. Used to grey out the "move up" and
    /// "move down" commands when the move would break a dependency, as in a linear chain of a fillet over an
    /// extrude.
    pub fn can_reorder_feature(&self, from: usize, to: usize) -> bool {
        if from >= self.timeline.len() || to >= self.timeline.len() || from == to {
            return false;
        }
        let to2 = if to > from { to - 1 } else { to };
        let mut order: Vec<usize> = (0..self.timeline.len()).collect();
        let item = order.remove(from);
        order.insert(to2, item);
        self.order_valid(&order)
    }

    /// Move timeline node `from` to position `to`, reordering the history. Refused when it would break a
    /// dependency (an input body ending up below its consumer). Marks everything dirty. Returns whether it was
    /// done.
    pub fn reorder_feature(&mut self, from: usize, to: usize) -> bool {
        if from >= self.timeline.len() || to >= self.timeline.len() || from == to {
            return false;
        }
        let to2 = if to > from { to - 1 } else { to };
        let mut order: Vec<usize> = (0..self.timeline.len()).collect();
        let item = order.remove(from);
        order.insert(to2, item);
        if !self.order_valid(&order) {
            return false;
        }
        let n = self.timeline.remove(from);
        self.timeline.insert(to2, n);
        for nd in &mut self.timeline {
            nd.dirty = true;
        }
        true
    }

    /// Translate the stored edge references of body `src` through the rename map of this pass and write the
    /// result back into the node, so that the guess happens exactly once, as it does for faces. The snapshots in
    /// `edge_refs` travel with the references: they are needed to repair a real topology change.
    pub(super) fn translate_edge_refs(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, cur: &[crate::geom::MeshEdge]) -> Vec<u32> {
        let Some(map) = emap.get(&src) else { return edges.to_vec() };
        let alive = |id: u32| cur.is_empty() || cur.iter().any(|e| e.id == id);
        // Only what is lost gets translated. The map accumulates along the whole ancestor chain, and an old
        // number from a distant ancestor may happen to hit a live edge of the current body, in which case
        // translating would move the reference onto somebody else's geometry. So: if the number is alive the
        // reference is correct and is left alone; if it is gone and the translation yields an existing edge, it
        // is translated; otherwise it is left to the snapshot repair.
        let out: Vec<u32> = edges
            .iter()
            .map(|e| match map.get(e) {
                Some(&new) if !alive(*e) && alive(new) => new,
                _ => *e,
            })
            .collect();
        if out == edges {
            return out;
        }
        if let Some(n) = self.timeline.iter_mut().find(|n| n.id == node_id) {
            match &mut n.kind {
                // Translated names are written back only for a hand-picked set: a descriptive query stores no
                // names, and replacing it with a list of numbers would take away everything it exists for.
                crate::feature::FeatureKind::Fillet { edges, .. } | crate::feature::FeatureKind::Chamfer { edges, .. } => {
                    if !edges.query.picked_descs().is_empty() {
                        *edges = crate::refs::Ref::picks(&out);
                    }
                }
                _ => {}
            }
        }
        if let Some(snaps) = self.edge_refs.get_mut(&node_id) {
            for (id, _, _) in snaps.iter_mut() {
                if let Some(&new) = map.get(id) {
                    *id = new;
                }
            }
        }
        out
    }

}
