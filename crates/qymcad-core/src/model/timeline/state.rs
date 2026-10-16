//! WHAT A NODE CHANGED AND WHO OWNS IT: dirty marks, owners, creators, dependents, consumed and active bodies.

use super::*;

impl Project {
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
        // the rollback bar decides which recipes are built at all: moved, it changes the bodies from the higher of
        // its two places down, as `set_rollback` marks them - undone and done again, the bar left each of them with
        // the meshes of the other state
        let moved_from = if self.rollback != other.rollback { self.rollback_place().min(other.rollback_place()) } else { usize::MAX };
        for (i, n) in self.timeline.iter().enumerate() {
            let bar_moved = i >= moved_from;
            let outs = n.kind.bodies();
            if outs.is_empty() {
                continue;
            }
            let mine = self.node_recipe_key(n.id);
            let theirs = if other.timeline.iter().any(|o| o.id == n.id) { other.node_recipe_key(n.id) } else { 0 };
            if bar_moved || mine != theirs || theirs == 0 {
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
        let on_datum = |p: &Self, sid: Id| -> bool { p.sketches.iter().find(|s| s.id == sid).is_some_and(|s| matches!(s.plane, SketchPlane::Datum(_))) };
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
        self.timeline.iter().find(|n| n.kind.owns_body(body)).and_then(|n| n.owner_of(body))
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
        } else if self.timeline.iter().any(|n| matches!(n.kind, crate::feature::FeatureKind::MeshPiece { body: b, .. } if b == body)) {
            ExportKind::MeshOnly // A piece of an imported mesh: its mesh is all there is, and it never had a B-rep.
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

    /// WHAT NODE `node` READS THAT THE DOCUMENT NO LONGER HOLDS: an input that is no body, sketch, plane, datum axis or
    /// point, component or node of the timeline. A command that picked a face and then saw Ctrl+Z take the body away
    /// holds such an input; the node laid on it would only stand red.
    pub fn gone_inputs(&self, node: Id) -> Vec<Id> {
        let Some(n) = self.timeline.iter().find(|n| n.id == node) else { return Vec::new() };
        let own = n.kind.bodies();
        // a datum reads its body through its definition, not through the inputs of its node
        let base = match n.kind {
            crate::feature::FeatureKind::Plane { plane: d } | crate::feature::FeatureKind::DatumAxis { axis: d } => self.datum_base_body(d),
            _ => None,
        };
        n.kind
            .inputs()
            .into_iter()
            .chain(n.kind.consumed())
            .chain(base)
            .filter(|&i| i != 0 && !own.contains(&i))
            .filter(|&i| {
                self.mesh_index(i).is_none()
                    && self.sketch_index(i).is_none()
                    && !self.planes.iter().any(|p| p.id == i)
                    && !self.datum_axes.iter().any(|a| a.id == i)
                    && !self.datum_points.iter().any(|q| q.id == i)
                    && !self.components.iter().any(|c| c.id == i)
                    && !self.timeline.iter().any(|t| t.id == i)
            })
            .fold(Vec::new(), |mut out, i| {
                if !out.contains(&i) {
                    out.push(i); // a consumed body is an input too: named once
                }
                out
            })
    }

    /// Bodies consumed by active modifier features, which are hidden so that only the result of the chain is
    /// visible. It accounts for the rollback bar: features below the bar are not built and therefore consume no
    /// source. One source for both the core and the interface.
    pub fn consumed_bodies(&self) -> std::collections::HashSet<Id> {
        let limit = self.rollback.unwrap_or(usize::MAX);
        // A NODE THAT FAILED WITHOUT EVER BUILDING CONSUMES NOTHING: its inputs stay in sight until something stands
        // in their place (a node merely not built yet still consumes - the document is read that way before any
        // rebuild). An intersection of two pieces that only touch comes out empty and red, and it took both pieces out
        // of view with it - 8000 mm^3 of the part became 4000. A node that built once and fails later keeps its last
        // good body, so its inputs stay consumed and nothing is shown twice.
        let built: std::collections::HashSet<Id> = self.bodies.iter().filter(|b| !b.mesh.tris.is_empty()).map(|b| b.id).collect();
        self.timeline
            .iter()
            .enumerate()
            .filter(|(ti, n)| *ti < limit && (!self.regen_errors.contains_key(&n.id) || n.kind.bodies().is_empty() || n.kind.bodies().iter().any(|b| built.contains(b))))
            .flat_map(|(_, n)| n.kind.consumed())
            .collect()
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
        let consumed: std::collections::HashSet<Id> =
            self.timeline.iter().enumerate().filter(|(ti, _)| *ti < self.rollback.unwrap_or(usize::MAX) && *ti < upto).flat_map(|(_, n)| n.kind.consumed()).collect();
        // The rollback bar counts: bodies below it are not built (their mesh was removed during regenerate)
        // and cannot be active. Without this a cut or a shell would target a rolled-back body with no mesh and
        // the feature would fail.
        let limit = self.rollback.unwrap_or(usize::MAX).min(upto);
        self.timeline.iter().take(limit).rev().flat_map(|n| n.kind.bodies()).find(|b| !consumed.contains(b) && !self.is_sheet_body(*b) && self.body_owner(*b) == Some(ctx))
    }

    /// Is `body` a sheet - a surface with no volume, a face copy or a patch? A sheet stands beside the part's solid and
    /// is never the body an operation of the part lands on: a boss united with a sheet failed and ate the sheet.
    pub(crate) fn is_sheet_body(&self, body: Id) -> bool {
        self.bodies.iter().any(|b| b.id == body && b.sheet)
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
        let prior = bodies.iter().rev().copied().find(|b| *b != new_body && !consumed.contains(b) && !self.is_sheet_body(*b) && self.body_owner(*b) == Some(ctx));
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
        // followed by `finish_base_body`, which always joins the new body as the tool `b`). A node that leaves
        // several bodies - a split - is never joined that way: a boolean after it is the person's own and has a
        // row of its own, or it could not be found in the tree to reopen. Nor is a node that grows a body it takes
        // (a boss joined into a piece): it joins by itself, and a union after it is the person's too.
        if let Some(next) = self.timeline.get(pos + 1) {
            if let FK::BodyBoolean { b, op: 1, .. } = next.kind {
                let grows_a_body = self.timeline[pos].kind.inputs().iter().any(|i| self.bodies.iter().any(|x| x.id == *i));
                if b == body && self.timeline[pos].kind.bodies().len() == 1 && !grows_a_body {
                    return vec![first_id, next.id];
                }
            }
        }
        vec![first_id]
    }
}
