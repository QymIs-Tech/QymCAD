//! THE KEYS OF A REBUILD: what a node depends on, the state a rebuild is keyed by, the copy without source data.

use super::*;

impl Project {
    /// Copy of the project without the bytes of the embedded sources. The `sources` records themselves stay,
    /// with their name, extension and an empty `data`.
    ///
    /// Used for undo snapshots: import originals never change yet weigh tens of megabytes, so 40 undo steps
    /// on a real assembly would hold gigabytes and copy about 90 MB on every edit. The bytes are put back
    /// by [`Project::take_source_data_from`].
    pub fn clone_without_source_data(&self) -> Project {
        let mut p = self.clone();
        for s in &mut p.sources {
            s.data = Vec::new();
        }
        p
    }

    /// A STATE BROUGHT BACK BY UNDO OR REDO HANDS OUT NO ID THE LIVE DOCUMENT HAS HANDED OUT ALREADY: the counter goes on
    /// from the higher of the two. Brought back as it was, the counter gave the next node the id of the body the undo
    /// took away, and a pick still held on that body named the new node instead - a shell laid on its own body, red.
    pub fn keep_ids_past(&mut self, live: &Project) {
        self.next_id = self.next_id.max(live.next_id);
    }

    /// Take the source bytes from `live` (matched by id) into this project: the inverse of
    /// [`Project::clone_without_source_data`], used when restoring a snapshot. Sources no longer present in
    /// the live project stay empty, since nothing will read them.
    pub fn take_source_data_from(&mut self, live: &mut Project) {
        for s in &mut self.sources {
            if s.data.is_empty() {
                if let Some(src) = live.sources.iter_mut().find(|l| l.id == s.id) {
                    s.data = std::mem::take(&mut src.data);
                }
            }
        }
    }

    /// Take the placement from the live document: where the components stand and what the mates were told
    /// to hold.
    ///
    /// Needed wherever the document is replaced by a result computed on a copy: a background rebuild takes a
    /// snapshot into a thread and returns a whole document. Its geometry is fresh, which is what the rebuild
    /// was for, but its placement is stale, taken at the moment the snapshot was sent. Meanwhile the pointer
    /// was dragging a component, and accepting the stale placement discards that work.
    ///
    /// The alternative — discarding the result on such a collision — produces two visible symptoms: the
    /// discarded result immediately asks to be recomputed, so the modal rebuild window flashes on every
    /// drag step (measured: eight drag steps, eight rebuild requests), and component placements only catch
    /// up once the pointer stops and the loop opens, which reads as rubber-banding.
    ///
    /// What counts as placement: the transform of a component and the specified value of a mate (`drive`) —
    /// exactly what dragging changes. The mating side (`flip`) counts too: the solver chooses it from the
    /// current placement, so taking it from an older copy means restoring an answer chosen for a different
    /// placement.
    pub fn take_placement_from(&mut self, live: &Project) {
        for c in &mut self.components {
            if let Some(l) = live.components.iter().find(|l| l.id == c.id) {
                c.transform = l.transform;
            }
        }
        for j in &mut self.joints {
            if let Some(l) = live.joints.iter().find(|l| l.id == j.id) {
                // A slot given by an expression keeps the value the rebuild evaluated: the live one is the old
                // value of the parameter. Reported behaviour: a hinge angle typed as pa stayed at 90 deg after
                // pa became 45.
                let exprs = self.feat_dims.get(&j.id);
                for (slot, key) in ["angle", "offset", "offset2"].into_iter().enumerate() {
                    if !exprs.and_then(|d| d.get(key)).is_some_and(|e| !e.trim().is_empty()) {
                        j.drive[slot] = l.drive[slot];
                    }
                }
                j.flip = l.flip;
                j.roll_flip = l.roll_flip;
                j.flip_decided = l.flip_decided;
            }
        }
    }

    /// Cheap state key of the model: has anything worth saving changed.
    ///
    /// Computing such a key by RON-serialising half the project on every frame is both expensive and
    /// incomplete: it misses structure, so creating an empty part, renaming it, moving a component, adding
    /// joints or external references would not make the project dirty, and closing the window would not ask
    /// about unsaved work.
    ///
    /// This is hashing with no text made: what has no placement in it is fed whole through its serialization
    /// (`hash_ser`), the placement and a few fields the program writes are hashed by hand (see `key_of`). Derived
    /// data is excluded (`regen_faces`, `regen_edges`, `regen_errors`, mesh geometry), and so are the source
    /// bytes — they never change, so id, name and size suffice.
    pub fn state_key(&self) -> u64 {
        self.key_of(true)
    }

    /// What the timeline is built from: a state key without the placement.
    ///
    /// "Is there unsaved work" and "does anything need rebuilding" are different questions, and confusing
    /// them is expensive. Dragging a component along a degree of freedom changes the placement (component
    /// transforms, mate values) and changes no body at all: the timeline is built from recipes, sketches and
    /// parameters, not from where a component stands. Comparing the full document key makes the scheduler
    /// read every drag frame as "the document moved, rebuild it".
    ///
    /// Measured on that path: eight drag steps produced eight rebuild requests, that is, eight flashes of
    /// the modal rebuild window.
    ///
    /// Placement that does affect geometry is not lost here: a sketch on a face of another part is an
    /// external reference, and moving the source marks its consumers dirty
    /// (`mark_external_consumers_dirty`). The scheduler queries those marks separately, so that rebuild
    /// still happens — once, when the component is released, instead of twenty times a second.
    /// THE KEY OF ONE SKETCH: what it is drawn as and where it stands, labels moved and notes aside - what the bodies on
    /// it are built from. 0 when there is no such sketch.
    pub fn sketch_key(&self, sid: Id) -> u64 {
        use std::hash::Hasher;
        let Some(s) = self.sketches.iter().find(|s| s.id == sid) else { return 0 };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        feed_sketch(s, &mut h);
        h.finish()
    }

    pub fn rebuild_key(&self) -> u64 {
        self.key_of(false)
    }

    /// Shared computation of both keys; they must not diverge, so there is exactly one implementation.
    /// `placement` selects whether what changes the document and builds nothing is included: where components stand,
    /// what the mates were told to hold, where a label sits, and the document's properties, colours and notes. (The
    /// mesh quality builds: the bodies are meshed to it.)
    ///
    /// Reported behaviour (issue #101): the title, author and comment and a part's colour were left out, so changing
    /// only them left the document looking saved, and closing it lost the change. Found with them: a circle's radius,
    /// a sketch's notes, a connector's place, the contents of a mate and more. What has no placement in it is now fed
    /// to the hasher whole, through its own serialization (`hash_ser`), so a field added to it later is in the key
    /// without anyone remembering.
    pub(crate) fn key_of(&self, placement: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        use hash_ser::feed;
        // EVERY FIELD OF THE DOCUMENT IS DECIDED HERE, in the key or out of it, and a field added to `Project` does
        // not compile until it is: the key and the file must not drift apart again.
        let Project {
            units: _,
            meta: _,
            geom_quality: _,
            next_id: _,
            imported_bodies: _,
            part_colors: _,
            face_colors: _,
            tri_colors: _,
            planes: _,
            sketches: _,
            timeline: _,
            components: _,
            root: _,
            active_component: _,
            datum_points: _,
            datum_axes: _,
            connectors: _,
            joints: _,
            mate_constraints: _,
            relations: _,
            external_refs: _,
            named_dims: _,
            sources: _,
            parameters: _,
            feat_dims: _,
            bodies: _,
            rollback: _,
            placed_refs: _,
            // OUT: written by the program, not by an edit - the rebuild interns the names, recomputes the contours,
            // records the bodies a deletion killed, snapshots referenced faces and edges; the solver lists the
            // violated mates. Counting them would make opening a document and touching nothing ask to save.
            names: _,
            contours: _,
            dead_bodies: _,
            edge_refs: _,
            face_refs: _,
            mates_violated: _,
            // OUT: what the last rebuild made and found - its faces and edges, its errors and warnings (the last two
            // are saved, as node_errors and node_warnings, and come from the rebuild all the same) - and the drag and
            // solve state of the session, which is not saved at all
            regen_faces: _,
            regen_edges: _,
            regen_errors: _,
            regen_warnings: _,
            mates_conflict: _,
            drag_pull: _,
            snap_rebinds: _,
            placed_now: _,
            held_rebuilds: _, // OUT: the rebuilds a tool holds back while it lays many elements, not written
        } = self;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let bits = |v: f64, h: &mut std::collections::hash_map::DefaultHasher| v.to_bits().hash(h);
        self.next_id.hash(&mut h);
        self.root.hash(&mut h);
        self.active_component.hash(&mut h);
        self.rollback.hash(&mut h);
        (self.units as u8).hash(&mut h);
        // the mesh quality: the bodies are meshed to it, so a change of it is rebuilt
        feed(&self.geom_quality, &mut h);
        // the document's title, author and comment and the colours of parts, faces and triangles change what is saved
        // and nothing that is built
        if placement {
            // when the document was started and which build wrote it the program stamps, on the first save after the
            // saved key is taken and on the copy written to the file; the rest is the author's
            let DocMeta { title, author, version, comment, created: _, saved_by: _ } = &self.meta;
            (title, author, version, comment).hash(&mut h);
            feed(&self.part_colors, &mut h);
            feed(&self.face_colors, &mut h);
            feed(&self.tri_colors, &mut h);
            // a reference found by place and accepted is saved, and builds nothing
            feed(&self.placed_refs, &mut h);
        }
        // THE GEOMETRY THE FILE KEEPS, by its size - what a body's mesh holds, not its points. The bodies are written by
        // the program and stay out of what is rebuilt, but the file stores them: a rebuild that brought new meshes
        // changes what would be saved. Reported behaviour: Rebuild everything mended the roundings of an old document
        // (its meshes came with the surface's normals), the document was not marked changed, nothing asked to save it,
        // and opened again its roundings were broken as before. Sizes only: opening a document and touching nothing
        // rebuilds nothing, and counting is free where hashing millions of points on every frame is not.
        if placement {
            for b in &self.bodies {
                (b.id, b.mesh.verts.len(), b.mesh.tris.len(), b.mesh.norms.len()).hash(&mut h);
            }
        }
        // Components: name, kind, parent, visibility, grounding and placement.
        for c in &self.components {
            // every field decided, as for the document above
            let crate::feature::Component { id: _, name: _, kind: _, parent: _, visible: _, grounded: _, transform: _ } = c;
            (c.id, &c.name, c.kind as u8, c.parent, c.visible, c.grounded).hash(&mut h);
            if placement {
                for v in c.transform {
                    bits(v, &mut h);
                }
            }
        }
        // The whole timeline, feature parameters included.
        //
        // Leaving the parameters out and relying on the application's `geom_rev` — a revision of the drawing
        // cache — makes "has the document changed" answer yes to any rebuild: opening a project, touching
        // nothing and closing it asks whether to save. Three patches in a row treated the symptom; the cause
        // was that the document key did not describe the document.
        //
        // The parameters are taken by serialising the recipe, which is complete by construction. A hand-
        // written field list across forty feature kinds would be cheaper per tick and more dangerous in
        // substance — a forgotten field means a lost edit, which is worse than one extra question. The recipe
        // is fed to the hasher as it serializes, with no text made: on 1,000 parts the whole key took 0.96-1.21 ms
        // with the recipes written out as RON text and 0.69-0.72 ms fed (release build, fastest of ten).
        for n in &self.timeline {
            // `dirty` is the scheduler's mark, not the author's
            let crate::feature::FeatureNode { id, name, kind, parent, suppressed, dirty: _ } = n;
            (id, name, parent, suppressed, kind.body()).hash(&mut h);
            feed(kind, &mut h);
        }
        let mut dims: Vec<(&Id, &std::collections::HashMap<String, String>)> = self.feat_dims.iter().collect();
        dims.sort_by_key(|(k, _)| **k);
        for (id, m) in dims {
            id.hash(&mut h);
            let mut kv: Vec<(&String, &String)> = m.iter().collect();
            kv.sort();
            kv.hash(&mut h);
        }
        // Sketches: points, counts and dimension values, so an edited number is visible before the solve
        // runs.
        for s in &self.sketches {
            feed_sketch(s, &mut h);
            // A LABEL MOVED IS AN EDIT OF THE DOCUMENT - it goes into the file - but not of what is built: the
            // constraints whole, labels included, are counted with the placement, so dragging a label asks to save
            // and rebuilds nothing. A note is the same: written beside the drawing, building nothing.
            if placement {
                feed(&s.constraints, &mut h);
                feed(&s.notes, &mut h);
            }
        }
        // Datums and planes.
        for p in &self.planes {
            let WorkPlane { id, name, origin, normal, rot_deg, tilt, def } = p;
            (id, name).hash(&mut h);
            for v in origin.iter().chain(normal.iter()) {
                bits(*v, &mut h);
            }
            bits(*rot_deg, &mut h);
            bits(tilt.about_x, &mut h);
            bits(tilt.about_y, &mut h);
            // how the plane is defined - on a face, through points, at an offset - not only where it stands now
            feed(def, &mut h);
        }
        for d in &self.datum_points {
            let DatumPoint { id, name, at, def } = d;
            (id, name).hash(&mut h);
            for v in at {
                bits(*v, &mut h);
            }
            feed(def, &mut h);
        }
        // an axis by its definition only: where it stands is a cache the rebuild fills (`set_resolved`), empty in a
        // document just opened, so counting it made an untouched document with an axis on an edge ask to save. A
        // hand-placed axis holds its coordinates in the definition.
        for a in &self.datum_axes {
            let DatumAxis { id, name, def, origin_cache: _, dir_cache: _ } = a;
            (id, name).hash(&mut h);
            feed(def, &mut h);
        }
        // Assembly: connectors, joints, external references. Where a connector sits on its part and which way it
        // faces, and what a mate constraint holds, move components and build nothing: counted with the placement.
        for c in &self.connectors {
            (c.id, c.owner).hash(&mut h);
        }
        if placement {
            feed(&self.connectors, &mut h);
            feed(&self.mate_constraints, &mut h);
        }
        for j in &self.joints {
            // every field decided; the mating side is left out, see below
            let crate::feature::Joint {
                id: _,
                name: _,
                a: _,
                b: _,
                kind: _,
                angle: _,
                offset: _,
                offset2: _,
                drive: _,
                limit_min: _,
                limit_max: _,
                global: _,
                as_built: _,
                flip: _,
                roll_flip: _,
                flip_decided: _,
            } = j;
            (j.id, &j.name, j.a, j.b, j.kind as u8).hash(&mut h);
            // The offset and angle of a mate are placement too: they move a component and touch no body.
            // They belong in the "is there unsaved work" key and not in the "does it need rebuilding" one.
            if placement {
                bits(j.angle, &mut h);
                bits(j.offset, &mut h);
                bits(j.offset2, &mut h);
                // Specified values, limits and "hold as built" are part of the document, not derived data.
                //
                // Leaving them out lets a mate whose specified value moved nothing (the value is unchanged
                // and the component already stands there) pass silently: the document counts as saved and
                // closing asks nothing, although an edit was made that the file does not know about.
                //
                // The mating side (`flip`, `roll_flip`, `flip_decided`) is deliberately excluded: the solver
                // writes it, and including it would declare every solve an edit, outside any operation
                // boundary.
                for slot in 0..3 {
                    if let Some(v) = j.drive[slot] {
                        bits(v, &mut h)
                    }
                    if let Some(v) = j.limit_min[slot] {
                        bits(v, &mut h)
                    }
                    if let Some(v) = j.limit_max[slot] {
                        bits(v, &mut h)
                    }
                    (j.drive[slot].is_some(), j.limit_min[slot].is_some(), j.limit_max[slot].is_some()).hash(&mut h);
                }
                j.global.hash(&mut h);
                if let Some(m) = &j.as_built {
                    for v in m {
                        bits(*v, &mut h);
                    }
                }
                j.as_built.is_some().hash(&mut h);
            }
        }
        // relations between mates move components and touch no body: their numbers are placement, like a mate's angle
        for r in &self.relations {
            let crate::feature::MateRelation { id: _, name: _, kind: _, a: _, slot_a: _, b: _, slot_b: _, value: _, phase: _, reversed: _ } = r;
            (r.id, &r.name, r.kind as u8, r.a, r.slot_a, r.b, r.slot_b).hash(&mut h);
            if placement {
                bits(r.value, &mut h);
                bits(r.phase, &mut h);
                r.reversed.hash(&mut h);
            }
        }
        for r in &self.external_refs {
            let crate::feature::ExternalRef { id: _, from_component: _, to_geometry: _ } = r;
            (r.id, r.from_component, r.source_body()).hash(&mut h);
        }
        // Parameters and named dimensions (text, and cheap to hash). A parameter's value is computed from its
        // expression.
        for p in &self.parameters {
            let Param { name, expr, value: _ } = p;
            (name, expr).hash(&mut h);
        }
        for d in &self.named_dims {
            let NamedDim { name: _, target: _ } = d;
            d.name.hash(&mut h);
            match &d.target {
                DimTarget::Sketch { sketch, refs } => (0u8, sketch, refs).hash(&mut h),
                DimTarget::Feature { node, key } => (1u8, node, key).hash(&mut h),
            }
        }
        // Bodies and meshes: membership and names only. The geometry is derived from the timeline and was
        // already accounted for by the nodes.
        for b in &self.bodies {
            let Body { id, name, visible, mesh: _, faces: _, sheet: _ } = b;
            (id, name, visible).hash(&mut h);
        }
        let mut imported: Vec<Id> = self.imported_bodies.iter().copied().collect();
        imported.sort_unstable();
        imported.hash(&mut h);
        // Import sources: id, name and size, but not the bytes — tens of megabytes of data that never
        // change.
        for s in &self.sources {
            let SourceFile { id, name, ext, data } = s;
            (id, name, ext, data.len()).hash(&mut h);
        }
        h.finish()
    }

    /// Fingerprint of a node recipe, used to judge whether its geometry could have changed between two
    /// project states (undo and redo).
    ///
    /// The fingerprint holds only what the body is built from: the `kind` itself (the feature parameters),
    /// the contents of the input sketches and of the planes they sit on, and the dimension expressions of
    /// the feature. Meshes, faces and bodies are derived and are not part of it.
    pub fn node_recipe_key(&self, node_id: Id) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let Some(n) = self.timeline.iter().find(|n| n.id == node_id) else { return 0 };
        ron::ser::to_string(&n.kind).unwrap_or_default().hash(&mut h);
        n.suppressed.hash(&mut h);
        n.parent.hash(&mut h);
        for inp in n.kind.inputs() {
            if let Some(s) = self.sketches.iter().find(|s| s.id == inp) {
                ron::ser::to_string(s).unwrap_or_default().hash(&mut h);
                if let crate::feature::SketchPlane::Datum(pid) = s.plane {
                    if let Some(pl) = self.planes.iter().find(|p| p.id == pid) {
                        ron::ser::to_string(pl).unwrap_or_default().hash(&mut h);
                    }
                }
            }
        }
        if let Some(d) = self.feat_dims.get(&node_id) {
            let mut kv: Vec<(&String, &String)> = d.iter().collect();
            kv.sort();
            kv.hash(&mut h);
        }
        h.finish()
    }

    /// Every id a node depends on, that is, everything that has to be produced before it: bodies and
    /// sketches (`inputs`) plus datum references — a sketch on a datum or a face, an offset plane, an axis
    /// through two points or along an edge or face, and a mirror, revolve or pattern around a datum.
    pub(crate) fn node_required_refs(&self, kind: &crate::feature::FeatureKind) -> Vec<Id> {
        use crate::feature::{FeatureKind as FK, SketchPlane};
        let mut r = kind.inputs();
        match *kind {
            FK::Sketch { sketch } => {
                if let Some(s) = self.sketches.iter().find(|s| s.id == sketch) {
                    match s.plane {
                        SketchPlane::Datum(pid) => r.push(pid),
                        SketchPlane::Face(body, _) => r.push(body),
                        SketchPlane::World(_) => {}
                    }
                }
            }
            FK::Plane { plane } => {
                if let Some(p) = self.planes.iter().find(|p| p.id == plane) {
                    match p.def {
                        PlaneDef::OffsetPlane { plane: src, .. } => r.push(src),
                        PlaneDef::OffsetFace { body, .. } => r.push(body),
                        _ => {}
                    }
                }
            }
            FK::DatumAxis { axis } => {
                if let Some(a) = self.datum_axes.iter().find(|a| a.id == axis) {
                    match a.def {
                        AxisDef::TwoPoints { a: pa, b: pb } => {
                            r.push(pa);
                            r.push(pb);
                        }
                        AxisDef::FromEdge { body, .. } | AxisDef::FromFace { body, .. } => r.push(body),
                        AxisDef::Manual { .. } => {}
                    }
                }
            }
            FK::Mirror { datum, .. } if datum != 0 => r.push(datum),
            FK::Revolve { axis_datum, .. } if axis_datum != 0 => r.push(axis_datum),
            FK::CircularArray { axis, .. } if axis != 0 => r.push(axis),
            _ => {}
        }
        r
    }

    /// Span of a body along a normal: the projection of the bounding box of `body` onto axis `n`, measured
    /// from `origin`.
    ///
    /// Extracted because the extent computation needed it twice with identical code: once for "through the
    /// whole body" and once to check whether the tool end landed exactly on the far face.
    pub(crate) fn body_span_along(&self, body: Id, origin: [f64; 3], n: [f64; 3]) -> Option<(f64, f64)> {
        let bb = self.mesh_index(body).and_then(|i| self.bodies[i].mesh.bounds())?;
        let corners = [
            [bb.min.x, bb.min.y, bb.min.z],
            [bb.max.x, bb.min.y, bb.min.z],
            [bb.min.x, bb.max.y, bb.min.z],
            [bb.max.x, bb.max.y, bb.min.z],
            [bb.min.x, bb.min.y, bb.max.z],
            [bb.max.x, bb.min.y, bb.max.z],
            [bb.min.x, bb.max.y, bb.max.z],
            [bb.max.x, bb.max.y, bb.max.z],
        ];
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for c in corners {
            let t = (c[0] - origin[0]) * n[0] + (c[1] - origin[1]) * n[1] + (c[2] - origin[2]) * n[2];
            tmin = tmin.min(t);
            tmax = tmax.max(t);
        }
        Some((tmin, tmax))
    }

    /// Remove a body from view: the mesh and the face and edge caches go together.
    ///
    /// Three lines repeated in four places of the rebuild, where any copy could forget one of them and leave
    /// the face cache and the edge cache disagreeing about which bodies exist at all. The feature itself
    /// stays in the timeline: this hides a result (rollback, suppression) rather than deleting a node.
    pub(crate) fn drop_body_from_view(&mut self, b: Id) {
        if let Some(mi) = self.mesh_index(b) {
            self.remove_mesh(mi);
        }
        self.regen_faces.remove(&b);
        self.regen_edges.remove(&b);
    }

    /// THE NAMES THE BRIDGE GAVE WHERE THE HISTORY WAS SILENT, landed in the document: the faces it lists, the colours a
    /// file gave single faces (held by the id of the face, as any reference to it), the edges the references of this
    /// rebuild are translated by.
    pub(crate) fn land_silent_names(&mut self, feature: Id, body: Id, kernel: &dyn crate::feature::Kernel, emap: &mut EdgeRenames) {
        let listed: Vec<u32> = self.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        let scale = self.import_scale(body).filter(|f| *f > 0.0).unwrap_or(1.0);
        let named = kernel.name_silent(crate::feature::SilentNaming { body, feature, listed: Some(&listed), scale, names: &mut self.names });
        if !named.faces.is_empty() {
            // all at once: a face may take the name another face gives up in the same renaming
            let now: std::collections::HashMap<u32, u32> = named.faces.iter().map(|r| (r.was, r.now)).collect();
            if let Some(faces) = self.regen_faces.get_mut(&body) {
                faces.iter_mut().for_each(|f| f.id = now.get(&f.id).copied().unwrap_or(f.id));
            }
            let root = self.lineage_root(body);
            if let Some(colours) = self.face_colors.get_mut(&root) {
                colours.iter_mut().for_each(|c| c.0 = now.get(&c.0).copied().unwrap_or(c.0));
            }
        }
        if !named.edges.is_empty() {
            emap.entry(body).or_default().extend(named.edges.iter().map(|r| (r.was, r.now)));
        }
        // The names merged faces yielded, in the final names. Stale records are cleared first: a face whose name is
        // alive again yielded to nobody, and an old record about it only misleads.
        let live: Vec<u32> = self.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        self.names.forget_absorbed(&live);
        for a in &named.absorbed {
            self.names.absorb(a.loser, a.winner);
        }
    }
}

/// A SKETCH INTO THE KEY OF THE DOCUMENT: what it is drawn as and where it stands, its labels and notes aside. One routine
/// for the key of the whole document and for the key of one sketch (`Project::sketch_key`), so the two cannot drift
/// apart.
fn feed_sketch(s: &Sketch, h: &mut std::collections::hash_map::DefaultHasher) {
    use hash_ser::feed;
    use std::hash::Hash;
    let bits = |v: f64, h: &mut std::collections::hash_map::DefaultHasher| v.to_bits().hash(h);
    let Sketch {
        id,
        name,
        contour_ids: _, // OUT: recomputed with the contours
        source,
        points,
        entities,
        closed,
        constraints,
        splines,
        notes: _, // counted with the placement, in `key_of`
        texts,
        patterns,
        rects,
        projections,
        plane,
        origin,
        axis_pts,
        frame,
        origin_uv,
        // OUT: what the solver, a drag and the rebuild of loops keep between calls, none of it written
        left_unsolved: _,
        drag_session: _,
        laid: _,
        checked: _,
        point_at: _,
        entity_at: _,
    } = s;
    (id, name, source, closed, origin, axis_pts, frame).hash(h);
    // the plane whole, save the persistent id of the face a sketch stands on: the rebuild writes that one
    // when it rebinds the sketch to a face renamed by a change upstream (`rebind_lost_face_refs`); the face
    // as it was picked - its index, centroid and normal - is the author's
    std::mem::discriminant(plane).hash(h);
    match plane {
        crate::feature::SketchPlane::Face(body, key) => {
            (body, key.index).hash(h);
            for v in key.centroid.iter().chain(key.normal.iter()) {
                bits(*v, h);
            }
        }
        other => feed(other, h),
    }
    feed(origin_uv, h);
    for p in points {
        p.id.hash(h);
        bits(p.x, h);
        bits(p.y, h);
    }
    // the lines, arcs and circles whole - a circle's radius, a line turned into construction - and what is
    // drawn over them
    feed(entities, h);
    feed(splines, h);
    feed(patterns, h);
    feed(rects, h);
    // how many constraints there are, and the numbers of the dimensions, so an edited number is visible before
    // the solve runs
    constraints.len().hash(h);
    for c in constraints {
        if let Some(d) = c.dim_value() {
            bits(d, h);
        }
    }
    // labels: a placed or retyped label is a change of the document like any line. Its glyphs are made from
    // the text and the font, and are not counted.
    for t in texts {
        let SketchText { id, x, y, height, angle, text, construction, font, glyphs: _ } = t;
        (id, text, construction).hash(h);
        for v in [x, y, height, angle] {
            bits(*v, h);
        }
        feed(font, h);
    }
    // a projection: what it projects; the points and lines it made, and whether its source was lost, the
    // rebuild writes
    for pr in projections {
        let SketchProjection { id, body, src, points: _, entities: _, edges: _ } = pr; // OUT: the edge of each curve, written by the rebuild with them
        (id, body).hash(h);
        feed(src, h);
    }
}
