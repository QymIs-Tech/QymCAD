//! THE DOCUMENT AND ITS COMPONENTS: parts and assemblies added, cloned, grafted, placed, their external references.

use super::*;

impl Project {
    /// Initialise a new document: the root assembly plus one active, empty part.
    ///
    /// Under strict isolation (bodies may exist only inside a part) nothing can be drawn in an empty
    /// assembly, so a document always starts with an active part. Returns the id of that part.
    pub fn new_document(&mut self) -> Id {
        self.ensure_root();
        let part = self.add_part(self.free_part_name());
        self.set_active_component(Some(part));
        part
    }

    /// WHERE A NEW SKETCH GOES: the part being worked in, or - when the context is an assembly - a new part made in it
    /// and stood in, so a sketch always belongs to a part (a part is one body, and a sketch in an assembly has no body
    /// to build). `Some(part)` when a part was made; the window steps into it.
    pub fn part_to_draw_in(&mut self) -> Option<Id> {
        let ctx = self.active_ctx();
        if self.component_is_part(ctx) {
            return None;
        }
        let part = self.add_part(self.free_part_name());
        self.set_active_component(Some(part));
        Some(part)
    }

    /// THE DOCUMENT OF A FIRST START: a part called Cube holding a sketch of a 20 x 20 square on XY and its extrusion
    /// 20 up - a cube of 8000 mm^3. Opened into, it shows how the work goes here: a sketch, and a feature built on it.
    /// Decided 25.09 on a report that a first start met a person with an empty part.
    pub fn cube_sample(&mut self) -> Id {
        let root = self.ensure_root();
        self.set_active_component(Some(root));
        let part = self.add_part("name-sample-cube");
        self.set_active_component(Some(part));
        let si = self.new_sketch("name-sketch-n#1");
        let sid = self.sketches[si].id;
        self.add_sketch_node(sid, "name-sketch-n#1");
        self.add_rect_entity(si, -10.0, -10.0, 10.0, 10.0, crate::feature::Purpose::Real);
        self.regen_sketch(si);
        let body = self.add_extrude(sid, 20.0);
        self.finish_base_body(body, 1);
        // the person stands in the assembly and sees the part in it; stepping into it shows how it is made
        self.set_active_component(Some(root));
        part
    }

    /// AN EMPTY DOCUMENT: the root assembly and nothing else.
    ///
    /// Reported behaviour: "in an empty project remove the empty part - a person opens the CAD and sees an
    /// empty part; the first time round that spoils everything." A part nobody asked for is not a starting
    /// point, it is a thing to delete before starting.
    ///
    /// SEPARATE FROM `new_document` rather than a flag on it. `new_document` is what "New part" gives, and
    /// there the part is exactly what was asked for; the two are different answers to different questions,
    /// and a boolean argument would make every call site say which question it is asking.
    ///
    /// The root is still made: a document without one has nowhere to put anything, and the tree would show
    /// an empty pane rather than a document.
    pub fn new_empty_document(&mut self) -> Id {
        let root = self.ensure_root();
        // the context is the root itself, not a part inside it - named as the window names it, or the window's first
        // frame names it so and the untouched document reads as changed
        self.set_active_component(Some(root));
        root
    }

    /// A part in the active context.
    pub fn add_part(&mut self, name: impl Into<String>) -> Id {
        self.add_component_kind(name, crate::feature::ComponentKind::Part)
    }

    /// A subassembly in the active context.
    pub fn add_assembly(&mut self, name: impl Into<String>) -> Id {
        self.add_component_kind(name, crate::feature::ComponentKind::Assembly)
    }

    /// Whether a context may hold bodies. The root and any assembly may not; only a part may.
    pub fn ctx_holds_bodies(&self, ctx: Id) -> bool {
        self.component_kind(ctx) == Some(crate::feature::ComponentKind::Part)
    }

    /// Add a part component.
    pub fn add_component(&mut self, name: impl Into<String>) -> Id {
        self.add_part(name)
    }

    // --- Mates (joints). ---

    /// Whether the cross-component reference from `consumer` to `body` is authorised by an explicit external reference.
    pub fn external_authorized(&self, consumer: Id, body: Id) -> bool {
        self.external_ref_for(consumer, body).is_some()
    }

    /// Break an external reference: the geometry stays exactly where it is and the associativity ends.
    ///
    /// Every consumer sketch sitting on a face of the source body is moved onto a snapshot — a fixed datum
    /// plane in the local space of the consumer. Without that, simply deleting the reference would drop the
    /// part into an isolation error ("a sketch on a face of another component's body without an external
    /// reference"). Returns how many sketches were frozen.
    pub fn break_external_ref(&mut self, ref_id: Id) -> usize {
        use crate::feature::SketchPlane;
        let Some(r) = self.external_refs.iter().find(|r| r.id == ref_id) else { return 0 };
        let consumer = r.from_component;
        let Some(body) = r.source_body() else {
            self.remove_external_ref(ref_id);
            return 0;
        };
        let targets: Vec<(usize, Id, crate::feature::FaceKey)> = self
            .sketches
            .iter()
            .enumerate()
            .filter_map(|(si, s)| match s.plane {
                SketchPlane::Face(b, key) if b == body && self.sketch_owner(s.id) == Some(consumer) => Some((si, s.id, key)),
                _ => None,
            })
            .collect();
        let mut frozen = 0;
        for (si, sid, key) in targets {
            // The frame before freezing, in consumer local space; the snapshot has to land exactly on it.
            let live = self.sketch_frame(si);
            let pid = self.snapshot_face_plane_for(consumer, body, &key);
            self.sketches[si].plane = SketchPlane::Datum(pid);
            if let (Some(live), Some(pi)) = (live, self.planes.iter().position(|p| p.id == pid)) {
                // A datum defines only the plane: its 2D zero is the projection of the consumer origin,
                // whereas on the live face it was the projection of the source origin, and a tilted neighbour
                // also picks its axes differently (the dominant-axis rule in `world_aligned`). Without this
                // correction, breaking the link would shift and rotate all the 2D geometry, while freezing
                // has to be invisible. The rotation goes into `rot_deg` and the remainder into the sketch
                // `origin_uv` — the same 2D-zero shift a manual snap to an edge produces.
                let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                let (po, pn) = (self.planes[pi].origin, self.planes[pi].normal);
                let base0 = crate::feature::PlaneFrame::world_aligned(po, pn, 0.0);
                let rot = dot(live.x, base0.y).atan2(dot(live.x, base0.x)).to_degrees();
                self.planes[pi].rot_deg = rot;
                let base = crate::feature::PlaneFrame::world_aligned(po, pn, rot);
                let d = [live.origin[0] - base.origin[0], live.origin[1] - base.origin[1], live.origin[2] - base.origin[2]];
                self.sketches[si].origin_uv = Some(crate::geom::Point2::new(dot(d, base.x), dot(d, base.y)));
            }
            self.mark_sketch_dirty(sid); // Rebuild bodies on this sketch: its plane became a snapshot.
            frozen += 1;
        }
        self.remove_external_ref(ref_id);
        frozen
    }

    /// Resolve an edge of body `body` by persistent id into its midpoint and tangent, in body local space.
    /// `None` when the edges are not built yet (`regen_edges` is empty) or the id is unknown.
    pub fn resolve_edge(&self, body: Id, edge_id: u32) -> Option<([f64; 3], [f64; 3])> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| (e.mid, e.dir))
    }

    /// Home assembly of a mate: the lowest common ancestor of its connector owners.
    ///
    /// A mate belongs to exactly one assembly — between parts of the root it belongs to the root, between
    /// parts of a subassembly it belongs to that subassembly. The parent assembly must neither show nor
    /// account for the mates of its nested subassemblies.
    pub fn joint_home(&self, j: &crate::feature::Joint) -> Option<Id> {
        let oa = self.connector(j.a)?.owner;
        let ob = self.connector(j.b)?.owner;
        self.common_ancestor(oa, ob)
    }

    /// Delete component `id` with everything it holds: subcomponents, bodies, sketches, datums, and the
    /// connectors and joints that reference them.
    ///
    /// Returns the ids of the deleted bodies, so the interface can drop its mesh and shape caches. The root
    /// cannot be deleted and yields an empty list. One method, because this is a topology operation.
    pub fn delete_component(&mut self, id: Id) -> Vec<Id> {
        use crate::feature::FeatureKind;
        if id == self.root || !self.components.iter().any(|c| c.id == id) {
            return Vec::new();
        }
        // The whole subtree: the component itself plus its descendants.
        let mut subtree: std::collections::HashSet<Id> = self.descendants(id).into_iter().collect();
        subtree.insert(id);
        // Classify the timeline nodes of the subtree into bodies, sketches and datums.
        let mut bodies = Vec::new();
        let mut sketches = Vec::new();
        let mut planes = Vec::new();
        let mut points = Vec::new();
        let mut axes = Vec::new();
        for nd in &self.timeline {
            if !nd.parent.is_some_and(|p| subtree.contains(&p)) {
                continue;
            }
            bodies.extend(nd.kind.bodies());
            match nd.kind {
                FeatureKind::Sketch { sketch } => sketches.push(sketch),
                FeatureKind::Plane { plane } => planes.push(plane),
                FeatureKind::DatumPoint { point } => points.push(point),
                FeatureKind::DatumAxis { axis } => axes.push(axis),
                _ => {}
            }
        }
        // Drop the timeline nodes of the subtree (bodies, sketches, datums and features at once).
        self.timeline.retain(|nd| !nd.parent.is_some_and(|p| subtree.contains(&p)));
        // Meshes and the parametric dimensions of the bodies.
        for b in &bodies {
            if let Some(mi) = self.mesh_index(*b) {
                self.remove_mesh(mi);
            }
            self.feat_dims.remove(b);
        }
        // Sketches from the pool.
        for s in &sketches {
            if let Some(si) = self.sketch_index(*s) {
                self.remove_sketch(si);
            }
        }
        // Datums from the pools.
        let (ps, pts, axs): (std::collections::HashSet<Id>, std::collections::HashSet<Id>, std::collections::HashSet<Id>) =
            (planes.into_iter().collect(), points.into_iter().collect(), axes.into_iter().collect());
        self.planes.retain(|p| !ps.contains(&p.id));
        self.datum_points.retain(|p| !pts.contains(&p.id));
        self.datum_axes.retain(|a| !axs.contains(&a.id));
        // Broken connectors and their joints, on two counts: the owner is inside the subtree, or the anchor
        // references a body being deleted. The second case is real because the owner of a connector may be
        // the parent context (a subassembly) while the geometry lives in the part being deleted (see
        // `ancestor_child_of` when a joint is created). Without it a joint survives with a broken anchor,
        // `connector_frame` resolves to the old centroid fingerprint and the solve scatters bodies to
        // garbage coordinates.
        use crate::feature::AnchorRef;
        let dead_bodies: std::collections::HashSet<Id> = bodies.iter().copied().collect();
        let anchor_dead = |a: &AnchorRef| matches!(a, AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) if dead_bodies.contains(b));
        let dead_conn: std::collections::HashSet<Id> = self.connectors.iter().filter(|c| subtree.contains(&c.owner) || anchor_dead(&c.anchor)).map(|c| c.id).collect();
        self.joints.retain(|j| !dead_conn.contains(&j.a) && !dead_conn.contains(&j.b));
        self.connectors.retain(|c| !dead_conn.contains(&c.id));
        // The regenerate caches go too, as they do when a sketch is deleted. Dropping the meshes while
        // leaving `regen_faces` and `regen_edges` behind produced a body that no timeline node builds yet
        // that still counts as alive for everything that enumerates live bodies. Every deletion path was
        // checked: plane deletion, body cascade and feature deletion were clean, and the ghost remained for
        // sketch deletion (since fixed) and here.
        self.drop_orphan_bodies();
        // External references whose source body or consumer component is inside the subtree are cut.
        let dead_refs: Vec<Id> =
            self.external_refs.iter().filter(|e| subtree.contains(&e.from_component) || e.source_body().and_then(|b| self.body_owner(b)).is_some_and(|o| subtree.contains(&o))).map(|e| e.id).collect();
        self.external_refs.retain(|e| !dead_refs.contains(&e.id));
        // The components themselves.
        self.components.retain(|c| !subtree.contains(&c.id));
        if self.active_component.is_some_and(|a| subtree.contains(&a)) {
            self.active_component = None;
        }
        self.drop_orphan_sources();
        bodies
    }

    /// AN IMPORT SOURCE GOES WITH WHAT CAME OF IT: a file no node of the timeline (an imported solid, a mesh piece)
    /// and no sketch comes of any more is dropped. Reported behaviour: the part a file came in as was deleted and its
    /// source still hung in the tree.
    pub(crate) fn drop_orphan_sources(&mut self) {
        use crate::feature::FeatureKind;
        let mut used: std::collections::HashSet<Id> = self.sketches.iter().filter_map(|s| s.source).collect();
        used.extend(self.timeline.iter().filter_map(|n| match n.kind {
            FeatureKind::Import { source, .. } | FeatureKind::MeshPiece { source, .. } => Some(source),
            _ => None,
        }));
        self.sources.retain(|s| used.contains(&s.id));
    }

    // --- Tree clipboard: copying and cutting sketches, parts and subassemblies. ---

    /// Clone a single sketch (from the tree clipboard) into a target component. Returns the id of the clone.
    pub fn clone_sketch_node(&mut self, sid: Id, target_parent: Id) -> Option<Id> {
        self.clone_sketch_impl(sid, target_parent, &std::collections::HashMap::new())
    }

    /// Re-parent sketch node `sid` under another component (a cut followed by a paste). Returns whether it
    /// succeeded.
    pub fn move_sketch_node(&mut self, sid: Id, target_parent: Id) -> bool {
        if !self.components.iter().any(|c| c.id == target_parent) {
            return false;
        }
        if let Some(ti) = self.timeline_index(sid) {
            self.timeline[ti].parent = Some(target_parent);
            true
        } else {
            false
        }
    }

    /// Deep clone of component `id`, with its whole subtree (subcomponents, sketches, datums, features,
    /// bodies), under `target_parent`.
    ///
    /// Every id is remapped, and every reference (input bodies, sketches, datums, contour `profile`) is
    /// re-pointed at the clones. Bodies are marked dirty, so the application builds them with
    /// `regenerate(kernel)`. Returns the id of the cloned subtree root.
    ///
    /// A within-project clone is a special case of the cross-project `clone_subtree_into`, with the source
    /// being a snapshot of self, so both share one id-remapping engine.
    pub fn clone_component(&mut self, id: Id, target_parent: Id) -> Option<Id> {
        let from = self.clone();
        let copy = Self::clone_subtree_into(self, &from, id, target_parent)?;
        self.name_apart(&[copy], &|n| n.to_string()); // a copy is told from its original: "Part 1 (2)"
        Some(copy)
    }

    /// Extract a product: build a new minimal `Project` whose root assembly holds a deep clone of component
    /// `component` and its subtree. The `.qpart` format is the serialisation of that project.
    ///
    /// Joints, references and connectors leading outside the subtree are not carried over (as in the
    /// `clone_*` methods). Bodies are dirty and are rebuilt by `regenerate(kernel)` on paste. A `component`
    /// equal to the root yields `None`: only a part or a subassembly can be saved as a product.
    pub fn subproject_of(&self, component: Id) -> Option<Project> {
        let mut out = Project { units: self.units, ..Project::default() };
        let out_root = out.ensure_root();
        let new_root = Self::clone_subtree_into(&mut out, self, component, out_root)?;
        out.next_id = out.next_id.max(self.next_id);
        out.active_component = Some(new_root);
        Some(out)
    }

    /// Insert a product: bring the contents of `other` (a `.qpart` mini-project) into `self` under
    /// `target_parent`, the active assembly.
    ///
    /// Every top-level component of `other` (the children of its root) is cloned with a full id remap, so
    /// the result is first-class build-tree nodes, indistinguishable from geometry modelled in place.
    /// Bodies are marked dirty and the application builds them with `regenerate(kernel)`. Returns the id of
    /// the inserted root (the first one when the product has several components). The mirror image of
    /// `clone_component`, with an external project as the source.
    pub fn graft(&mut self, other: &Project, target_parent: Id, shown: &dyn Fn(&str) -> String) -> Option<Id> {
        if !self.components.iter().any(|c| c.id == target_parent) || self.component_is_part(target_parent) {
            return None;
        }
        let tops: Vec<Id> = other.components.iter().filter(|c| c.parent == Some(other.root)).map(|c| c.id).collect();
        let mut first = None;
        for t in tops {
            if let Some(nid) = Self::clone_subtree_into(self, other, t, target_parent) {
                self.name_apart(&[nid], shown); // it keeps its own name, numbered when one reading the same is here
                first.get_or_insert(nid);
            }
        }
        self.next_id = self.next_id.max(other.next_id);
        first
    }

    /// World transform of a body, through its owning component. A body with no owner (a raw import) gets
    /// the identity.
    pub fn body_world_transform(&self, body: Id) -> [f64; 12] {
        match self.body_owner(body) {
            Some(owner) => self.world_transform(owner),
            None => crate::feature::PLACE_IDENTITY,
        }
    }

    /// Transform of component `comp` relative to an ancestor or context `base`, that is
    /// `inv(world(base)) * world(comp)`.
    ///
    /// `base == comp` gives the identity, so a part is drawn at its own zero while being edited from
    /// inside; `base == root` gives absolute world space, which is the assembly overview.
    pub fn relative_transform(&self, comp: Id, base: Id) -> [f64; 12] {
        use crate::feature::{mat_inv12, mat_mul12};
        mat_mul12(&mat_inv12(&self.world_transform(base)), &self.world_transform(comp))
    }

    /// Transform of a body for drawing and picking, relative to the active context `active`.
    pub fn body_display_transform(&self, body: Id, active: Id) -> [f64; 12] {
        match self.body_owner(body) {
            Some(owner) => self.relative_transform(owner, active),
            None => crate::feature::PLACE_IDENTITY,
        }
    }

    /// Local frame of a component base plane, in the local space of that component. Used as an anchor by
    /// sketches, datums and connectors.
    pub fn component_base_frame(&self, plane: crate::feature::BasePlane) -> crate::feature::PlaneFrame {
        plane.frame()
    }

    /// World frame of a component base plane: the local frame carried through `world_transform`.
    pub fn component_base_frame_world(&self, id: Id, plane: crate::feature::BasePlane) -> crate::feature::PlaneFrame {
        plane.frame().transformed(&self.world_transform(id))
    }

    /// Whether the component is a part (and so may hold bodies).
    pub fn component_is_part(&self, id: Id) -> bool {
        self.component_kind(id) == Some(crate::feature::ComponentKind::Part)
    }

    /// Whether bodies may be created in the active context; used to gate the body tools in the interface.
    pub fn can_add_body(&self) -> bool {
        self.ctx_holds_bodies(self.current_ctx())
    }

    /// Owning component of a node reference (a body or a sketch). Planes, datums and anything unknown give
    /// `None`: isolation does not restrict those, since they are not geometry of another part.
    pub fn ref_owner(&self, id: Id) -> Option<Id> {
        self.body_owner(id).or_else(|| self.sketch_owner(id))
    }

    /// Operation using the first closed contour of a sketch against body `src`.
    pub fn add_combine(&mut self, src: Id, sketch: Id, height: f64, op: u8) -> Id {
        self.add_combine_on(src, sketch, 0, crate::model::CombineSpan { height, down: 0.0, extent: crate::feature::Extent::default(), fill: &[] }, op)
    }
}
