//! THE BODIES OF A DOCUMENT: meshes added and named, names of their faces, ids, the document ensured.

use super::*;

impl Project {
    /// Add a regular polygon as a parametric group of entities: a construction circumscribed circle plus n
    /// side lines held by constraints (each vertex on the circle, equal sides, a radius dimension). The
    /// centre and the first vertex give the radius and the orientation.
    ///
    /// The polygon stays regular under the solver, scales with the circle dimension and rotates by
    /// dragging.
    pub fn add_polygon_entity(&mut self, si: usize, c: crate::geom::Point2, v: crate::geom::Point2, n: u32, purpose: crate::feature::Purpose) -> Vec<Id> {
        let ((cx, cy), (vx, vy)) = ((c.x, c.y), (v.x, v.y));
        let construction = purpose == crate::feature::Purpose::Construction;
        self.add_polygon_param(si, Point2::new(cx, cy), Point2::new(vx, vy), n, crate::feature::Purpose::of(construction)).1
    }

    // --- Sketches: the grouping layer above contours. ---

    /// Delete a sketch together with its contours and its embedded source.
    pub fn remove_sketch(&mut self, sketch_index: usize) {
        if sketch_index >= self.sketches.len() {
            return;
        }
        let sk = self.sketches.remove(sketch_index);
        for cid in &sk.contour_ids {
            if let Some(idx) = self.contour_index(*cid) {
                self.contours.remove_at(idx); // The id, the provenance and the nesting go with it.
            }
        }
        if let Some(src) = sk.source {
            self.sources.retain(|s| s.id != src);
        }
    }

    // --- Meshes. ---

    pub fn add_mesh(&mut self, m: crate::geom::Mesh) -> Id {
        let id = self.alloc_id();
        let name = format!("name-body#{}", self.bodies.len() + 1);
        self.bodies.push(Body { id, name, mesh: m, faces: Vec::new(), visible: true, sheet: false });
        id
    }

    pub fn set_meshes(&mut self, ms: Vec<crate::geom::Mesh>) {
        self.bodies.clear();
        self.bodies.clear();
        self.bodies.clear();
        for m in ms {
            self.add_mesh(m);
        }
    }

    /// Name map from body `from` to body `to`, matched by place.
    ///
    /// Needed when a node is deleted: its consumers are moved onto the source body, but their geometry
    /// references still name the faces and edges of the deleted body. An edge name is derived from the
    /// pair of faces meeting at it, and the source body has different faces, hence a different name, even
    /// though the edge itself is the same and sits in the same place.
    ///
    /// Without this translation, deleting a fillet turns a chamfer red on the opposite side of the part —
    /// geometry that never touched it. What broke the chamfer was the missing name translation, which is
    /// done here by matching place, because the place of that edge did not change.
    ///
    /// Anything not found is not translated: the reference goes red honestly instead of latching onto
    /// something similar. Nor is anything that finds two at its place - the wall of a tube and the wall of its bore
    /// share a centroid on the axis - by the rule `resolve_face_id` keeps for a snapshot: only an unambiguous match.
    pub(crate) fn geom_name_map(&self, from: Id, to: Id) -> std::collections::HashMap<u32, u32> {
        const TOL: f64 = 1e-6;
        let mut map = std::collections::HashMap::new();
        if let (Some(a), Some(b)) = (self.regen_edges.get(&from), self.regen_edges.get(&to)) {
            for e in a.iter().filter(|e| e.id != 0) {
                let same = |t: &&crate::geom::MeshEdge| t.id != 0 && (0..3).all(|i| (t.mid[i] - e.mid[i]).abs() < TOL);
                if let Some(t) = the_one(b.iter().filter(same)) {
                    if t.id != e.id {
                        map.insert(e.id, t.id);
                    }
                }
            }
        }
        if let (Some(a), Some(b)) = (self.regen_faces.get(&from), self.regen_faces.get(&to)) {
            for f in a.iter().filter(|f| f.id != 0) {
                let same = |x: &&crate::geom::MeshFace| {
                    x.id != 0
                        && (x.centroid.x - f.centroid.x).abs() < TOL
                        && (x.centroid.y - f.centroid.y).abs() < TOL
                        && (x.centroid.z - f.centroid.z).abs() < TOL
                        && x.normal[0] * f.normal[0] + x.normal[1] * f.normal[1] + x.normal[2] * f.normal[2] > 0.9
                };
                if let Some(t) = the_one(b.iter().filter(same)) {
                    if t.id != f.id {
                        map.insert(f.id, t.id);
                    }
                }
            }
        }
        map
    }

    /// Re-point non-body references from body `from` to body `to`: sketches on a face, datum axes on an
    /// edge or a face, datum planes offset from a face. Face and edge keys are persistent, so they resolve
    /// on the new body by themselves; whatever does not resolve goes red on rebuild instead of
    /// disappearing silently.
    ///
    /// `nmap` translates names from body `from` to body `to` (see `geom_name_map`).
    ///
    /// Changing the body number is not enough: a sketch on a face, a datum axis on an edge and a plane
    /// offset from a face also hold the name of that geometry, and on the source body the name differs,
    /// being derived from the surroundings. Without the translation such an anchor points at nothing after
    /// the node is deleted, and silently builds nothing.
    pub(crate) fn rebind_body_refs(&mut self, from: Id, to: Id, nmap: &std::collections::HashMap<u32, u32>) {
        use crate::feature::SketchPlane;
        let m = |x: u32| nmap.get(&x).copied().unwrap_or(x);
        let mkey = |k: &crate::feature::FaceKey| {
            let mut k = *k;
            k.id = m(k.id);
            k
        };
        for s in self.sketches.iter_mut() {
            if let SketchPlane::Face(b, ref key) = s.plane {
                if b == from {
                    s.plane = SketchPlane::Face(to, mkey(key));
                }
            }
        }
        for a in self.datum_axes.iter_mut() {
            match a.def {
                AxisDef::FromEdge { body, edge } if body == from => a.def = AxisDef::FromEdge { body: to, edge: m(edge) },
                AxisDef::FromFace { body, face } if body == from => a.def = AxisDef::FromFace { body: to, face: m(face) },
                _ => {}
            }
        }
        for pl in self.planes.iter_mut() {
            if let PlaneDef::OffsetFace { body, ref face, dist } = pl.def {
                if body == from {
                    pl.def = PlaneDef::OffsetFace { body: to, face: mkey(face), dist };
                }
            }
        }
    }

    /// Remove ghosts: orphaned meshes, a body with no timeline node that is not an import - for example a
    /// primitive whose node was deleted while the mesh stayed. A body is valid when it has a timeline node or is
    /// listed in `imported_bodies`. A feature whose source is gone is NOT removed: it stays in the timeline, red
    /// with the reason, for the person to repair or delete - taking it away here would be the cascade the delete
    /// no longer makes unless asked. Returns the bodies that were removed.
    pub fn prune_dangling(&mut self) -> std::collections::HashSet<Id> {
        let mut valid: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        valid.extend(self.imported_bodies.iter().copied());
        let removed: std::collections::HashSet<Id> = self.bodies.iter().map(|b| b.id).filter(|b| !valid.contains(b)).collect();
        if !removed.is_empty() {
            self.remove_bodies(&removed);
        }
        // Orphaned connectors and joints: a connector whose owning component is gone, or whose anchor
        // references a body that no longer exists, is broken. Its joint resolves to a garbage frame and the
        // solve then places bodies at wild coordinates, which reads as the assembly blowing apart. Cleaned
        // on every regenerate, which also repairs older files that still carry such orphans.
        use crate::feature::AnchorRef;
        let live_comp: std::collections::HashSet<Id> = self.components.iter().map(|c| c.id).collect();
        let live_body: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).chain(self.imported_bodies.iter().copied()).collect();
        let anchor_ok = |a: &AnchorRef| match a {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => live_body.contains(b),
            _ => true, // Origin and BasePlane reference no body.
        };
        let dead_conn: std::collections::HashSet<Id> = self.connectors.iter().filter(|c| !live_comp.contains(&c.owner) || !anchor_ok(&c.anchor)).map(|c| c.id).collect();
        if !dead_conn.is_empty() {
            self.joints.retain(|j| !dead_conn.contains(&j.a) && !dead_conn.contains(&j.b));
            self.connectors.retain(|c| !dead_conn.contains(&c.id));
        }
        removed
    }

    /// Delete a body by index (mesh, id and name together). A manual colour, keyed by lineage root, stays in
    /// `part_colors`: a harmless leftover entry once the root is gone too.
    pub fn remove_mesh(&mut self, index: usize) {
        if index < self.bodies.len() {
            self.bodies.remove(index);
        }
    }

    pub fn mesh_index(&self, id: Id) -> Option<usize> {
        self.bodies.iter().position(|b| b.id == id)
    }

    /// THE FACES OF A BODY, by its id.
    ///
    /// Two places wrote `bodies.last_mut().faces = fs` after adding a body - a shape import and a rebuild -
    /// and "the last one" is only true while nothing else has been pushed in between. By id it is true
    /// always.
    pub fn set_body_faces(&mut self, body: Id, faces: Vec<crate::geom::MeshFace>) -> bool {
        match self.bodies.iter_mut().find(|b| b.id == body) {
            Some(b) => {
                b.faces = faces;
                true
            }
            None => false,
        }
    }

    pub fn mesh_id(&self, index: usize) -> Option<Id> {
        self.bodies.get(index).map(|b| b.id)
    }

    /// Body name by index, falling back to a generated one.
    pub fn mesh_name(&self, index: usize) -> String {
        self.bodies.get(index).map(|b| b.name.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("name-body#{}", index + 1))
    }

    pub fn set_mesh_name(&mut self, index: usize, name: String) {
        if let Some(b) = self.bodies.get_mut(index) {
            b.name = name;
        }
    }

    /// Id of the first mesh, used as the default target when creating a 3D operation.
    pub fn first_mesh_id(&self) -> Id {
        self.bodies.first().map(|b| b.id).unwrap_or(0)
    }

    /// Embed the original of an imported file and return its id.
    pub fn add_source(&mut self, name: impl Into<String>, data: Vec<u8>) -> Id {
        let name = name.into();
        let ext = std::path::Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("bin").to_lowercase();
        let id = self.alloc_id();
        self.sources.push(SourceFile { id, name, ext, data });
        id
    }

    /// Bring the id arrays in line with the geometry after loading, when ids were missing or of a different
    /// length.
    pub fn ensure_ids(&mut self) {
        // Contours no longer need this: the list and its ids are one structure (`model::contours`) and
        // cannot differ in length. Bodies no longer need it either, since id and name are part of the record
        // itself; only what may have arrived empty from the file is repaired here.
        for i in 0..self.bodies.len() {
            if self.bodies[i].id == 0 {
                self.bodies[i].id = self.alloc_id();
            }
            if self.bodies[i].name.is_empty() {
                self.bodies[i].name = format!("name-body#{}", i + 1);
            }
        }
        // Colours are no longer a parallel array to the meshes: manual ones live in `part_colors` keyed by
        // lineage root, and the rest come from the palette.
    }

    /// Post-load normalisation of a document: make sure the root assembly exists and attach floating nodes
    /// and components (`parent == None`) to it. The timeline is canonical, so no projection from the pools
    /// is needed. Idempotent.
    pub fn ensure_document(&mut self) {
        self.migrate_root();
    }

    /// Make sure the root assembly component exists and attach every floating node and component
    /// (`parent == None`) to it. Afterwards only the root has `parent == None`. Idempotent.
    pub fn migrate_root(&mut self) {
        let root = self.ensure_root();
        // The root has no name of its own, only a catalogue key.
        //
        // The root assembly is not a component someone created and can rename: there is always exactly
        // one. Stored as a plain string, its name freezes in the language of the build that created the
        // document, so an English interface ends up showing a Russian label. A name is only translatable
        // when it is a key, which is why the key is written here on every load: a document whose string
        // already froze would otherwise have no way back.
        if let Some(c) = self.components.iter_mut().find(|c| c.id == root) {
            c.name = "name-assembly".into();
        }
        for n in self.timeline.iter_mut() {
            if n.parent.is_none() {
                n.parent = Some(root);
            }
        }
        for c in self.components.iter_mut() {
            if c.parent.is_none() && c.id != root {
                c.parent = Some(root);
            }
        }
    }
}
