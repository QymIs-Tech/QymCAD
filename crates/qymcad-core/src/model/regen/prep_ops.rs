//! THE OPERATIONS ON A BODY PREPARED: fillets, chamfers, shells, faces, drafts, mirrors, moves, primitives, lofts.

use super::*;

impl Project {
    /// A FILLET: round the picked edges of body `src` by `radius`, with a variable radius at named vertices.
    ///
    /// Returns whether the node's error record may be cleared (see `apply_regen`).
    pub(crate) fn prep_fillet(&mut self, p: &mut Pass, src: Id, radius: f64, edges: &crate::refs::Ref, at_vertices: &[(crate::refs::Ref, f64)], body: Id) -> crate::feature::KernelJob {
        let radius = p.dim("radius", radius);
        // Two paths, and the difference between them is fundamental.
        //
        // A hand-picked set holds recorded edge names, and an edge name is derived from its pair of
        // faces and changes with them. Such references have to be translated through the rename map
        // of the source body — a proven mechanism that must not be disturbed; fillets in a real
        // document once went red on it.
        //
        // A descriptive query ("every edge of this face") stores no names at all, so there is
        // nothing to translate: it asks today's geometry directly.
        //
        // A reference that resolved to nothing is not "every edge". The kernel fillets the whole
        // body when the list is empty, which is correct for the query "fillet everything". But the
        // same empty list arises when one edge was picked and its reference stopped resolving:
        // measured on a box, a fillet over a non-existent edge produced 26 faces — exactly as many
        // as filleting all twelve — while the node stayed green. One edge was asked for, the whole
        // part was rounded, and the timeline said nothing.
        //
        // The distinction is made by the query rather than by the result: when descriptors were
        // named and no live edges were found, the reference is lost and that is reported.
        let asked_count = asked_edge_count(edges);
        let asked_edges = asked_count > 0 || !edges.query.is_pick_list();
        let lost_error = edges_lost(edges, asked_count);
        // A PART OF THE PICKED EDGES GONE - a cut above took one away - the rest are done and the node says, in yellow,
        // how many were left: it is neither a failure of the whole nor to be kept quiet. Reported behaviour: a rounding
        // of four edges stood green with three after a cut above took the fourth.
        self.regen_warnings.remove(&p.node);
        let partly_lost = if asked_count > 0 { self.edge_refs_lost(p.node, src, &edges.query.picked_descs(), p.emap, p.kernel) } else { 0 };
        let edges = &self.live_fillet_edges(p.node, src, edges, p.emap, p.kernel);
        let lost_edges = asked_edges && edges.is_empty();
        if partly_lost > 0 && !edges.is_empty() {
            self.regen_warnings.insert(p.node, crate::errors::CoreError::EdgesDropped { asked: asked_count, dropped: partly_lost });
        }
        // A variable radius is specified at vertices. Each table entry is resolved as a reference:
        // a vertex name is derived from its edges, so editing the neighbours does not disturb it.
        // The value is parametric like the radius itself, under the dimension key
        // `at{descriptor}`.
        let verts: Vec<crate::feature::RadiusAt> = at_vertices
            .iter()
            .filter_map(|(r, val)| {
                let desc = self.resolve_vertex_refs(src, r, "ref-what-fillet-vertex").ok()?.first().copied()?;
                let radius = p.dim(&format!("at{desc}"), *val);
                self.vertex_point(src, desc).map(|at| crate::feature::RadiusAt { at, radius })
            })
            .collect();
        // A solid tool does not operate on a surface. Otherwise it appears to work: the node goes
        // red while a degenerate two-triangle body is left next to the part (found by the fuzzer).
        let on_sheet = p.kernel.body_is_sheet(src);
        let edges: Vec<u32> = edges.to_vec();
        if on_sheet {
            crate::feature::KernelJob::refused(crate::errors::CoreError::NeedsSolidNotSheet)
        } else if lost_edges {
            crate::feature::KernelJob::refused(lost_error)
        } else if !verts.is_empty() && !edges.is_empty() {
            // the faces it makes are named by the bridge, after this node, as a fillet of one radius names them
            let roles = crate::feature::BlendRoles::Named { feature: p.node };
            crate::feature::KernelJob::new(vec![src], move |k| k.fillet_at_vertices(body, src, radius, &edges, &verts, roles))
        } else {
            // the faces it makes are named by the bridge, after this node
            let roles = crate::feature::BlendRoles::Named { feature: p.node };
            crate::feature::KernelJob::new(vec![src], move |k| k.fillet(body, src, radius, &edges, roles))
        }
    }

    /// Chamfer: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_chamfer(&mut self, p: &mut Pass, src: Id, dist: f64, edges: &crate::refs::Ref, shape: ChamferShape, body: Id) -> crate::feature::KernelJob {
        let ChamferShape { mode, d2, flip, ref_face } = shape;
        let dist = p.dim("dist", dist);
        let d2 = p.dim("d2", d2);
        // As for a fillet: an empty list means the whole part, and a lost reference must not
        // masquerade as that.
        let asked_count = asked_edge_count(edges);
        let asked_edges = asked_count > 0 || !edges.query.is_pick_list();
        let lost_error = edges_lost(edges, asked_count);
        // A PART OF THE PICKED EDGES GONE - a cut above took one away - the rest are done and the node says, in yellow,
        // how many were left: it is neither a failure of the whole nor to be kept quiet. Reported behaviour: a rounding
        // of four edges stood green with three after a cut above took the fourth.
        self.regen_warnings.remove(&p.node);
        let partly_lost = if asked_count > 0 { self.edge_refs_lost(p.node, src, &edges.query.picked_descs(), p.emap, p.kernel) } else { 0 };
        let edges = &self.live_fillet_edges(p.node, src, edges, p.emap, p.kernel);
        let lost_edges = asked_edges && edges.is_empty();
        if partly_lost > 0 && !edges.is_empty() {
            self.regen_warnings.insert(p.node, crate::errors::CoreError::EdgesDropped { asked: asked_count, dropped: partly_lost });
        }
        // Asymmetry (two setbacks, or setback plus angle) applies only to an explicit edge
        // selection; otherwise the chamfer is symmetric.
        let edges: Vec<u32> = edges.to_vec();
        if lost_edges {
            crate::feature::KernelJob::refused(lost_error)
        } else if mode != crate::feature::ChamferMode::Symmetric && !edges.is_empty() {
            // the faces it makes are named by the bridge, after this node, as a symmetric chamfer names them
            let roles = crate::feature::BlendRoles::Named { feature: p.node };
            crate::feature::KernelJob::new(vec![src], move |k| k.chamfer_ex(body, src, dist, ChamferShape { mode, d2, flip, ref_face }, &edges, roles))
        } else {
            // the faces it makes are named by the bridge, after this node: the same rule as for a fillet
            let roles = crate::feature::BlendRoles::Named { feature: p.node };
            crate::feature::KernelJob::new(vec![src], move |k| k.chamfer(body, src, dist, &edges, roles))
        }
    }

    /// Shell: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_shell(&mut self, p: &mut Pass, src: Id, thickness: f64, faces: &crate::refs::Ref, side: crate::feature::ShellSide, body: Id) -> crate::feature::KernelJob {
        let feature = p.node;
        let thickness = p.dim("thickness", thickness);
        // Names of the inner walls come from the recipe: a wall is produced by offsetting a face
        // and, without a name of its own, takes the name of the outer face, so a reference to an
        // inner edge lands on the outer one.
        // Resolution goes through the query, and a refusal stops the operation: opening the wrong
        // face is worse than opening none, since the body then looks similar with the hole in the
        // wrong place.
        //
        // Asking for two faces and opening one is not success. Measured: a shell given two faces
        // where one reference did not resolve opened one and stayed green (11 faces instead of 10).
        // The part looks similar but is closed on the side where an opening was expected. Only a
        // hand-picked set is checked this way: for a descriptive query the number of descriptors
        // says nothing about the result.
        let asked_faces = if faces.query.is_pick_list() { faces.query.picked_descs().len() } else { 0 };

        match self.faces_by_ref(p.node, src, faces, "ref-what-shell-faces") {
            Err(_) => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
            Ok(ids) if asked_faces > 0 && ids.len() < asked_faces => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
            Ok(ids) => match side {
                crate::feature::ShellSide::Centred => crate::feature::KernelJob::new(vec![src], move |k| k.shell_center(body, src, thickness, &ids)),
                side => crate::feature::KernelJob::new(vec![src], move |k| k.shell_named(body, src, thickness, side == crate::feature::ShellSide::Outward, &ids, feature)),
            },
        }
    }

    pub(crate) fn prep_removeface(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> crate::feature::KernelJob {
        match self.faces_by_ref(p.node, src, faces, "ref-what-removed-faces") {
            Ok(ids) if !ids.is_empty() => crate::feature::KernelJob::new(vec![src], move |k| k.remove_faces(body, src, &ids)),
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    pub(crate) fn prep_patch(&mut self, p: &Pass, src: Id, edges: &crate::refs::Ref, tangent: bool, body: Id) -> crate::feature::KernelJob {
        let edges = edges.clone();
        let live = self.live_fillet_edges(p.node, src, &edges, p.emap, p.kernel);
        if live.is_empty() {
            return crate::feature::KernelJob::refused(edges_lost(&edges, asked_edge_count(&edges)));
        }
        let feature = p.node;
        crate::feature::KernelJob::new(vec![src], move |k| k.patch(body, src, &live, tangent, feature))
    }

    pub(crate) fn prep_trim(&mut self, src: Id, tool: Id, keep: [f64; 3], body: Id) -> crate::feature::KernelJob {
        crate::feature::KernelJob::new(vec![src, tool], move |k| k.trim(body, src, tool, keep))
    }

    pub(crate) fn prep_stitch(&mut self, p: &Pass, parts: &[Id], tol: f64, body: Id) -> crate::feature::KernelJob {
        let t = p.dim("tol", tol);
        let parts = parts.to_vec();
        if parts.len() < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::OpFailed(crate::errors::Op::Stitch));
        }
        crate::feature::KernelJob::new(parts.clone(), move |k| k.stitch(body, &parts, t))
    }

    /// The mesh is the node's recipe: it travels with the work, and no live shape is read.
    // the mesh is made lighter inside the job, off the frame: a mesh of 178 032 triangles takes seconds
    pub(crate) fn prep_mesh_solid(&mut self, src: Id, body: Id, simplify: f64) -> crate::feature::KernelJob {
        let Some(mesh) = self.mesh_index(src).map(|i| self.bodies[i].mesh.clone()) else {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::SourceBodyNotBuilt);
        };
        crate::feature::KernelJob::new(Vec::new(), move |k| k.mesh_solid(body, &lighter(mesh, simplify)))
    }

    pub(crate) fn prep_mesh_recognised(&mut self, src: Id, body: Id, tol: f64, sharp: f64, simplify: f64) -> crate::feature::KernelJob {
        let Some(mesh) = self.mesh_index(src).map(|i| self.bodies[i].mesh.clone()) else {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::SourceBodyNotBuilt);
        };
        crate::feature::KernelJob::new(Vec::new(), move |k| {
            let mesh = lighter(mesh, simplify);
            let tol = within_simplification(&mesh, tol, simplify);
            k.mesh_recognised(body, &mesh, tol, sharp)
        })
    }

    pub(crate) fn prep_pushface(&mut self, p: &Pass, src: Id, face: &crate::refs::Ref, dist: f64, body: Id) -> crate::feature::KernelJob {
        let dist = eval_dim(p.dims, "dist", dist, p.vars);
        if p.kernel.body_is_sheet(src) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PushFaceOnSheet);
        }
        match self.face_by_ref(p.node, src, face, "ref-what-pushed-face") {
            Err(_) => crate::feature::KernelJob::refused(crate::errors::CoreError::FaceNotFound),
            Ok(_) if dist.abs() < 1e-9 => crate::feature::KernelJob::refused(crate::errors::CoreError::ZeroPushDistance),
            Ok(c) => {
                let desc = c.desc;
                crate::feature::KernelJob::new(vec![src], move |k| k.push_face(body, src, desc, dist))
            }
        }
    }

    pub(crate) fn prep_surfacereplace(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, surface: Id, body: Id) -> crate::feature::KernelJob {
        let faces = faces.clone();
        match self.faces_by_ref(p.node, src, &faces, "ref-what-surface-replace") {
            Ok(ids) if !ids.is_empty() => crate::feature::KernelJob::new(vec![src, surface], move |k| k.replace_faces(body, src, &ids, surface)),
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    pub(crate) fn prep_facecopy(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> crate::feature::KernelJob {
        let faces = faces.clone();
        match self.faces_by_ref(p.node, src, &faces, "ref-what-face-copy") {
            Ok(ids) if !ids.is_empty() => {
                let feature = p.node;
                crate::feature::KernelJob::new(vec![src], move |k| k.copy_faces(body, src, &ids, feature))
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    /// OffsetSurface: the faces, resolved as a face copy resolves them, and the distance, a dimension of its own (`dist`)
    /// a formula or a global parameter drives.
    pub(crate) fn prep_offset_surface(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, dist: f64, body: Id) -> crate::feature::KernelJob {
        let d = eval_dim(p.dims, "dist", dist, p.vars);
        match self.faces_by_ref(p.node, src, faces, "ref-what-offset-surface") {
            Ok(ids) if !ids.is_empty() => {
                let feature = p.node;
                crate::feature::KernelJob::new(vec![src], move |k| k.offset_faces(body, src, &ids, feature, d))
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    /// Thicken: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_thicken(&mut self, p: &mut Pass, src: Id, face: u32, thickness: f64, join: Id, body: Id) -> crate::feature::KernelJob {
        let feature = p.node;
        // The thickness is parametric (the `thickness` feature dimension) and the face reference
        // resolves by name.
        let t = eval_dim(p.dims, "thickness", thickness, p.vars);
        // The face reference goes through the shared resolution rather than a comparison of
        // numbers.
        //
        // While a face was a positional number the reference depended on the numbering, and as soon
        // as the source gained names the stored number stopped matching, turning a thicken red with
        // "face not found" in a live project. The order now matches the one used for edges: name,
        // then a recorded merge, then the single face of a sheet, then a place snapshot.
        let face = self.resolve_face_id(p.node, src, face).unwrap_or(face);
        let alive = self.regen_faces.get(&src).is_some_and(|fs| fs.iter().any(|f| f.id == face));

        if face == 0 || !alive {
            crate::feature::KernelJob::refused(crate::errors::CoreError::FaceNotFound)
        } else if t.abs() < 1e-9 {
            crate::feature::KernelJob::refused(crate::errors::CoreError::ZeroThickness)
        } else {
            // Plate names come from the recipe: the offset side is produced by its face and each
            // side wall by its boundary edge, which is their entire recipe and does not depend on
            // numbering. The pairs are prepared in advance, since interning mutates the name table
            // while the kernel only needs the finished substitution.
            let inputs = if join != 0 { vec![src, join] } else { vec![src] };
            crate::feature::KernelJob::new(inputs, move |k| k.thicken_face(body, src, face, t, join, feature))
        }
    }

    /// SplitFace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_splitface(&mut self, p: &mut Pass, src: Id, at: Option<([f64; 3], [f64; 3])>, offset: f64, body: Id) -> crate::feature::KernelJob {
        // The plane is a reference, as for a body split: a world plane, a datum or a face (see `op_plane`).
        let lost = at.is_none();
        let (o0, n) = at.unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]));
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();

        if lost {
            crate::feature::KernelJob::refused(crate::errors::CoreError::SplitPlaneDeleted)
        } else if len < 1e-9 {
            crate::feature::KernelJob::refused(crate::errors::CoreError::ZeroNormal)
        } else {
            let d = p.dim("offset", offset);
            let u = [n[0] / len, n[1] / len, n[2] / len];
            let at = [o0[0] + u[0] * d, o0[1] + u[1] * d, o0[2] + u[2] * d];
            crate::feature::KernelJob::new(vec![src], move |k| k.split_faces(body, src, at, u))
        }
    }

    /// Draft: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_draft(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, shape: DraftShape, body: Id) -> crate::feature::KernelJob {
        let DraftShape { neutral, angle, flip } = shape;
        // The angle is parametric (the `angle` feature dimension); the neutral face resolves into an
        // origin and a normal from the rebuilt source, and the pull direction is that normal,
        // reversed by `flip`.
        let angle = eval_dim(p.dims, "angle", angle, p.vars);
        // Both references are queries. The neutral face defines the pull direction, so taking the
        // wrong one drafts the part the other way and losing it is a refusal.
        //
        // A partial loss is a loss too. Measured: a draft given four live walls and one unresolvable
        // reference drafted four and stayed green. The part looks similar while one wall stands
        // vertical — and the mould is cast against it. As for a shell, only a hand-picked set is
        // checked this way: for a descriptive query the number of descriptors says nothing about the
        // result.
        let asked_faces = if faces.query.is_pick_list() { faces.query.picked_descs().len() } else { 0 };
        // a draft of 0 tilts nothing: it stood green with the body as it was
        if angle.abs() < 1e-9 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::DraftAngleZero);
        }
        match (self.faces_by_ref(p.node, src, faces, "ref-what-draft-faces"), self.face_by_ref(p.node, src, &neutral, "ref-what-draft-neutral")) {
            (Ok(ids), _) if asked_faces > 0 && ids.len() < asked_faces => crate::feature::KernelJob::refused(crate::errors::CoreError::DraftNeedsFaces),
            (Ok(ids), Ok(np)) if !ids.is_empty() => {
                let np_o = np.centroid;
                let np_n = if flip { [-np.normal[0], -np.normal[1], -np.normal[2]] } else { np.normal };
                // the side faces a draft makes are named by the bridge, after the node
                let feature = p.node;
                crate::feature::KernelJob::new(vec![src], move |k| {
                    k.draft(body, src, &ids, crate::feature::DraftPull { angle, dir: np_n }, crate::feature::PlaneAt { origin: np_o, normal: np_n }, feature)
                })
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::DraftNeedsFaces),
        }
    }

    /// Mirror: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_mirror(&mut self, p: &mut Pass, src: Id, world: Option<u8>, keep: bool, at: Option<([f64; 3], [f64; 3])>, body: Id) -> crate::feature::KernelJob {
        // A datum plane or a face supplies its origin and normal (see `op_plane`) and the mirror is taken about it;
        // otherwise a world plane 0, 1 or 2 is used.
        // An image is not the original face: with `keep` the body holds both halves, and without
        // names of their own they are indistinguishable — the same collision a pattern had.
        let keeps = if keep { crate::feature::MirrorKeeps::Both } else { crate::feature::MirrorKeeps::Image };
        let made = crate::feature::Mirrored { keeps, names: crate::feature::ImageNames::Instance { feature: p.node } };
        // A deleted plane means no mirror. Falling back to a world plane sounds defensible — a
        // mirror about another plane is still a mirror — but the measurement refutes it: the part
        // moves. A mirror about a datum at x = 50 gave face centres from x = 10 to x = 90, and
        // deleting the datum moved them to -30..30 with no red node. A split already refuses in the
        // same situation, for exactly the same reason.
        match (world, at) {
            (Some(plane), _) => crate::feature::KernelJob::new(vec![src], move |k| k.mirror(body, src, plane, made)),
            (None, Some((o, n))) => crate::feature::KernelJob::new(vec![src], move |k| k.mirror_plane(body, src, crate::feature::PlaneAt { origin: o, normal: n }, made)),
            (None, None) => crate::feature::KernelJob::refused(crate::errors::CoreError::MirrorPlaneDeleted),
        }
    }

    /// The body a datum plane or axis stands on - its face or its edge - by the datum's id; `None` for a datum on a
    /// world plane, another datum or coordinates.
    pub(crate) fn datum_base_body(&self, id: Id) -> Option<Id> {
        use crate::model::{AxisDef, PlaneDef};
        if let Some(p) = self.planes.iter().find(|p| p.id == id) {
            return match p.def {
                PlaneDef::OffsetFace { body, .. } => Some(body),
                _ => None,
            };
        }
        self.datum_axes.iter().find(|a| a.id == id).and_then(|a| match a.def {
            AxisDef::FromEdge { body, .. } | AxisDef::FromFace { body, .. } => Some(body),
            _ => None,
        })
    }

    /// THE PLANE A MIRROR OR A SPLIT STANDS ON, as origin and normal: a face of a body (read off it at this rebuild, so
    /// it follows the face), a datum plane (resolved earlier in the pass), or a world plane 0 XY, 1 XZ, 2 YZ. `None`
    /// when the face's body or the datum is gone: the operation refuses rather than cut along another plane.
    pub(crate) fn op_plane(&self, plane: u8, datum: Id, face: Option<(Id, crate::feature::FaceKey)>) -> Option<([f64; 3], [f64; 3])> {
        if let Some((body, key)) = face {
            return self.regen_faces.contains_key(&body).then(|| self.resolve_face(body, &key));
        }
        if datum != 0 {
            return self.planes.iter().find(|p| p.id == datum).map(|p| (p.origin, p.normal));
        }
        Some(match plane {
            1 => ([0.0; 3], [0.0, 1.0, 0.0]),
            2 => ([0.0; 3], [1.0, 0.0, 0.0]),
            _ => ([0.0; 3], [0.0, 0.0, 1.0]),
        })
    }

    /// Move: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_move(&mut self, src: Id, mat: [f64; 12], body: Id) -> crate::feature::KernelJob {
        // A BODY MADE OF A MESH MOVES IN THE DOCUMENT: it has no B-rep for the kernel to move - its mesh and the faces found
        // on it are moved as they are, and nothing is found on it again
        if self.mesh_born(src) {
            let Some(mi) = self.mesh_index(src) else { return crate::feature::KernelJob::refused(crate::errors::CoreError::SourceBodyNotBuilt) };
            let mut mesh = self.bodies[mi].mesh.clone();
            mesh.transform(&mat);
            let faces: Vec<crate::geom::MeshFace> = self.bodies[mi].faces.iter().map(|f| f.transformed(&mat)).collect();
            return crate::feature::KernelJob::new(Vec::new(), move |_| Ok(crate::geom::Built { mesh, faces }));
        }
        crate::feature::KernelJob::new(vec![src], move |k| k.transform_body(body, src, mat))
    }

    /// BodyBoolean: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_bodyboolean(&mut self, a: Id, b: Id, op: u8, body: Id) -> crate::feature::KernelJob {
        // Parametric body-to-body boolean: `op` applied to the B-reps of `a` and `b`, producing
        // `body`; both operands are consumed.
        crate::feature::KernelJob::new(vec![a, b], move |k| k.body_boolean(body, a, b, op))
    }

    /// Box3: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_box3(&mut self, p: &Pass, dx: f64, dy: f64, dz: f64, body: Id) -> crate::feature::KernelJob {
        let (dx, dy, dz) = (p.dim("dx", dx), p.dim("dy", dy), p.dim("dz", dz));
        if !above_zero(&[dx, dy, dz]) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        self.named_prism_job(p.node, &rect_profile(dx, dy), dz, body)
    }

    /// A PRIMITIVE EXTRUDED FROM A POLYGON, every face named by recipe: side k of the polygon (from 1) makes the wall
    /// `Role::Wall` of number k, the ends are the caps of the node. The number of a side is its place in the recipe of
    /// the primitive - the side from the first corner of a box, the side from the first vertex of a prism - and not
    /// the place the face takes in the walk of the result: measured, a box and a prism laid with an unnamed profile
    /// had 6 of 6 and 8 of 8 faces on positional numbers, and a fillet of an edge of a box held a number.
    pub(crate) fn named_prism_job(&mut self, feature: Id, xy: &[f64], height: f64, body: Id) -> crate::feature::KernelJob {
        let points: Vec<crate::geom::Point2> = xy.as_chunks::<2>().0.iter().map(|[x, y]| crate::geom::Point2::new(*x, *y)).collect();
        let sides = points.len() as Id;
        let mut outline = crate::geom::Contour::closed(points);
        outline.edge_src = (1..=sides).collect();
        // side k is the source at place k: its slot holds k itself
        let sources: Vec<crate::feature::SideSource> = (1..=sides).map(|k| crate::feature::SideSource { src: k, split: 0 }).collect();
        let profiles = vec![crate::geom::encode_profile_named(&outline, &[], &|src: Id, _: &crate::geom::ProfEdge| src as u32)];
        crate::feature::KernelJob::new(Vec::new(), move |k| {
            k.combine_region_multi(
                BodyOp { src: 0, op: 1, body },
                &profiles,
                crate::feature::ToolSpan { start: 0.0, length: height, ends: crate::feature::ToolEnds::Kept },
                crate::feature::PLACE_IDENTITY,
                crate::feature::Sides { feature, sources: &sources },
            )
        })
    }

    /// Cylinder: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    /// THE PARCEL OF A CYLINDER, and of every primitive below it: numbers and names, nothing borrowed from
    /// the document.
    ///
    /// Preparing one takes `&mut self` and always will: minting the names of the faces to come is a change to
    /// the document, and it has to happen before the geometry is built. That is why the split is where it is -
    /// the preparation stays on the thread that owns the document, and only the parcel travels.
    pub(crate) fn prep_cylinder(&mut self, p: &Pass, r: f64, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        if !above_zero(&[r, h]) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        let feature = p.node;
        crate::feature::KernelJob::new(Vec::new(), move |k| k.cylinder(body, r, h, feature))
        // Exact B-rep cylinder, three faces.
    }

    /// Sphere: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_sphere(&mut self, p: &Pass, r: f64, body: Id) -> crate::feature::KernelJob {
        let r = eval_dim(p.dims, "r", r, p.vars);
        if !above_zero(&[r]) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        let feature = p.node;
        crate::feature::KernelJob::new(Vec::new(), move |k| k.sphere(body, r, feature))
        // Exact sphere, one face.
    }

    /// Cone: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_cone(&mut self, p: &Pass, r1: f64, r2: f64, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r1, r2, h) = (p.dim("r1", r1), p.dim("r2", r2), p.dim("h", h));
        // one end may close to a point, not both, and neither end is narrower than nothing
        if !above_zero(&[h, r1.max(r2)]) || r1.min(r2) < 0.0 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        let feature = p.node;
        crate::feature::KernelJob::new(Vec::new(), move |k| k.cone(body, r1, r2, h, feature))
        // Exact cone.
    }

    /// Torus: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_torus(&mut self, p: &Pass, major: f64, minor: f64, body: Id) -> crate::feature::KernelJob {
        let (major, minor) = (p.dim("major", major), p.dim("minor", minor));
        if !above_zero(&[major, minor]) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        // a tube as thick as the ring or thicker passes through itself: a ring of 5 and a tube of 5 stood green as a
        // spindle of 2467 mm^3
        if minor >= major {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::TorusThroughItself);
        }
        let feature = p.node;
        crate::feature::KernelJob::new(Vec::new(), move |k| k.torus(body, major, minor, feature))
        // Exact torus, one face.
    }

    /// Prism: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_prism(&mut self, p: &Pass, r: f64, n: u32, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        if !above_zero(&[r, h]) {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::PrimitiveSizeNotAboveZero);
        }
        self.named_prism_job(p.node, &polygon_profile(r, n), h, body)
    }

    /// Loft: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_loft(&mut self, p: &mut Pass, sketches: Vec<Id>, contours: Vec<Id>, ruled: bool, surface: bool, bo: BodyOp) -> crate::feature::KernelJob {
        let BodyOp { src, op, body } = bo;
        // Each section sits on its own plane (the placement is encoded in `places`), and two or more
        // sections produce a body. A zero `src` makes a separate body; otherwise the lofted solid is
        // combined with body `src` as a lofted cut or boss.
        let feature = p.node;
        match self.loft_encoded_sourced(&sketches, &contours) {
            // A surface is the same loft, not closed into a solid. It admits no boolean: there is
            // nothing to combine a surface with a body by until it has been given a thickness.
            Some(enc) if src == 0 => crate::feature::KernelJob::new(Vec::new(), move |k| {
                let body_kind = if surface { crate::feature::LoftBody::Sheet } else { crate::feature::LoftBody::Solid };
                let s = &enc.sections;
                k.loft(body, crate::feature::LoftSections { data: &s.data, offsets: &s.offsets, places: &s.places }, walls(ruled), body_kind, crate::feature::Sides { feature, sources: &enc.sources })
            }),
            Some(enc) => crate::feature::KernelJob::new(vec![src], move |k| {
                let s = &enc.sections;
                k.loft_combine(BodyOp { src, op, body }, &s.data, &s.offsets, &s.places, walls(ruled), crate::feature::Sides { feature, sources: &enc.sources })
            }),
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::LoftNeedsTwoSections),
        }
    }
}
