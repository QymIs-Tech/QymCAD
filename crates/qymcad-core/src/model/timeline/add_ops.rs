//! THE OPERATIONS ADDED TO THE TIMELINE: primitives, fillets, chamfers, surfaces, faces, splits, drafts, arrays, holes, threads.

use super::*;

impl Project {
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
    pub fn add_combine_on(&mut self, src: Id, sketch: Id, profile: Id, span: super::CombineSpan, op: u8) -> Id {
        // One contour is the many-contour case with a list of one. It used to be a second copy of the same
        // eight lines, and the copy quietly dropped `fill`.
        self.add_combine_multi_op(src, sketch, vec![profile], span, op)
    }

    /// Operation on body `src` using several contours `profiles` within one node: one boolean, one body.
    pub fn add_combine_multi_op(&mut self, src: Id, sketch: Id, profiles: Vec<Id>, span: super::CombineSpan, op: u8) -> Id {
        let super::CombineSpan { height, down, extent: ext, fill } = span;
        let fill = fill.to_vec();
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let name = ["feat-name-combine-cut", "feat-name-combine-boss", "feat-name-combine-intersect"][(op as usize).min(2)].to_string();
        self.push_timeline(FeatureNode {
            id: body,
            name,
            kind: FeatureKind::Combine { src, sketch, profiles, height, op, extent: ext, down, fill, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
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
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-fillet".into(),
            kind: FeatureKind::Fillet { src, radius, edges, at_vertices: Vec::new(), body },
            parent,
            dirty: true,
            suppressed: false,
        });
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
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-fillet".into(),
            kind: FeatureKind::Fillet { src, radius, edges: crate::refs::Ref::picks(&edges), at_vertices: Vec::new(), body },
            parent,
            dirty: true,
            suppressed: false,
        });
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

    /// Turn mesh body `src` - made lighter within `simplify` mm first, zero for none - into a solid - a polyhedron of its
    /// flat faces - in the part the mesh stands in, whatever
    /// part is being worked in: the part is to hold one body, and it is the mesh's. The mesh is consumed; what is cut
    /// and drilled from here on is the solid.
    pub fn add_mesh_solid(&mut self, src: Id, simplify: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = self.timeline.iter().find(|n| n.kind.owns_body(src)).and_then(|n| n.parent).or_else(|| Some(self.body_parent()));
        self.push_timeline(FeatureNode { id: body, name: "feat-name-mesh-solid".into(), kind: FeatureKind::MeshSolid { src, body, simplify }, parent, dirty: true, suppressed: false });
        body
    }

    /// Recognise a mesh into a body of exact surfaces; `tol` multiplies the distance a corner may lie from its surface,
    /// `sharp` is the angle in degrees from which an edge between two triangles is sharp; `simplify` the deviation in mm
    /// the mesh is made lighter within first, zero for none.
    pub fn add_mesh_recognised(&mut self, src: Id, tol: f64, sharp: f64, simplify: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = self.timeline.iter().find(|n| n.kind.owns_body(src)).and_then(|n| n.parent).or_else(|| Some(self.body_parent()));
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-mesh-recognised".into(),
            kind: FeatureKind::MeshRecognised { src, body, tol, sharp, simplify },
            parent,
            dirty: true,
            suppressed: false,
        });
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

    /// An offset sheet of the faces `faces` of `src`, `dist` along their normals (see `FeatureKind::OffsetSurface`).
    pub fn add_offset_surface(&mut self, src: Id, faces: crate::refs::Ref, dist: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-offset-surface".into(), kind: FeatureKind::OffsetSurface { src, faces, dist, body }, parent, dirty: true, suppressed: false });
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
        self.push_timeline(FeatureNode { id: body, name, kind: FeatureKind::BodyBoolean { a, b, op, body, pieces: Vec::new() }, parent, dirty: true, suppressed: false });
        body
    }

    /// Chamfer with a mode: `TwoDist` (d1 and d2) or `DistAngle` (setback d1 plus angle d2 in degrees); `flip`
    /// selects the reference face. `ref_face` is the persistent id of a manually chosen reference face (zero
    /// selects it automatically from `flip`). Asymmetry requires an explicit edge selection; for "every edge"
    /// the kernel falls back to a symmetric chamfer.
    #[allow(clippy::too_many_arguments)]
    pub fn add_chamfer_ex(&mut self, src: Id, dist: f64, shape: super::ChamferShape, edges: Vec<u32>) -> Id {
        let super::ChamferShape { mode, d2, flip, ref_face } = shape;
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.capture_edge_refs(body, src, &edges);
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-chamfer".into(),
            kind: FeatureKind::Chamfer { src, dist, edges: crate::refs::Ref::picks(&edges), mode, d2, flip, ref_face, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Shell, with the side the wall goes to.
    pub fn add_shell_mode(&mut self, src: Id, thickness: f64, faces: Vec<u32>, side: crate::feature::ShellSide) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-shell".into(),
            kind: FeatureKind::Shell { src, thickness, faces: crate::refs::Ref::picks(&faces), side, body },
            parent,
            dirty: true,
            suppressed: false,
        });
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
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-push-face".into(),
            kind: FeatureKind::PushFace { src, face: crate::refs::Ref::one(face.id, crate::refs::Fingerprint { centroid: face.centroid, normal: face.normal }), dist, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Delete faces and heal. The face references go by name and therefore survive edits higher up the
    /// timeline.
    pub fn add_remove_face(&mut self, src: Id, faces: Vec<crate::feature::FaceKey>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-remove-face".into(),
            kind: FeatureKind::RemoveFace { src, faces: crate::refs::Ref::picks(&faces.iter().map(|f| f.id).collect::<Vec<_>>()), body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Thicken a face: face `face` of body `src` becomes a plate of `thickness` as a separate body. The source
    /// stays — a skin is made from a face of the housing while the housing remains. The thickness is parametric
    /// (the `thickness` feature dimension).
    /// The home part of a sheet: the body the sheet grew out of (a face copy, a patch, a trim, a stitch).
    ///
    /// Needed by thicken: the plate has to return into the part the surface was taken from, or the part is left
    /// holding two bodies, visible on screen as a differently coloured piece.
    pub(crate) fn sheet_origin_body(&self, sheet: Id) -> Id {
        use crate::feature::FeatureKind as FK;
        let mut cur = sheet;
        for _ in 0..64 {
            let Some(node) = self.timeline.iter().find(|n| n.kind.bodies().contains(&cur)) else { return 0 };
            let src = match node.kind {
                FK::FaceCopy { src, .. } | FK::OffsetSurface { src, .. } | FK::Patch { src, .. } | FK::Trim { src, .. } => src,
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
            if origin == 0 {
                0
            } else {
                self.live_body(origin)
            }
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
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-split-face".into(),
            kind: FeatureKind::SplitFace { src, plane, datum, offset, body, face: None },
            parent,
            dirty: true,
            suppressed: false,
        });
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
        self.push_timeline(FeatureNode {
            id,
            name: "feat-name-split-body".into(),
            kind: FeatureKind::SplitBody { src, plane, datum, offset, bodies: bodies.clone(), face: None },
            parent,
            dirty: true,
            suppressed: false,
        });
        bodies
    }

    pub fn add_draft(&mut self, src: Id, faces: Vec<u32>, neutral: u32, angle: f64, flip: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-draft".into(),
            kind: FeatureKind::Draft { src, faces: crate::refs::Ref::picks(&faces), neutral: crate::refs::Ref::one(neutral, crate::refs::Fingerprint::default()), angle, flip, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Linear pattern as a full 3D grid: three independent directions (count by count2 by count3).
    #[allow(clippy::too_many_arguments)]
    pub fn add_linear_array_grid3(&mut self, src: Id, axes: [crate::model::ArrayAxis; 3]) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        // The document still holds the twelve numbers loose; only the way in is named.
        let ([dx, dy, dz], [dx2, dy2, dz2], [dx3, dy3, dz3]) = (axes[0].d, axes[1].d, axes[2].d);
        let (count, count2, count3) = (axes[0].count, axes[1].count, axes[2].count);
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-linear-array".into(),
            kind: FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Circular pattern about a chosen axis: `axis` is a datum axis id (resolved during regenerate) or zero for
    /// world Z.
    pub fn add_circular_array_axis(&mut self, src: Id, count: u32, angle: f64, axis: Id) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-circular-array".into(),
            kind: FeatureKind::CircularArray { src, count, angle, axis, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// THE PLANE OF A MIRROR OR A SPLIT MADE A FACE: `node` then reads its plane off face `key` of body `body` at every
    /// rebuild - one node of the timeline, not a datum plane of its own beside it. Nothing when `node` is neither.
    pub fn set_op_face(&mut self, node: Id, body: Id, key: crate::feature::FaceKey) {
        use crate::feature::FeatureKind;
        if let Some(n) = self.timeline.iter_mut().find(|n| n.id == node) {
            if let FeatureKind::Mirror { face, datum, .. } | FeatureKind::SplitBody { face, datum, .. } | FeatureKind::SplitFace { face, datum, .. } = &mut n.kind {
                *face = Some((body, key));
                *datum = 0;
                n.dirty = true;
            }
        }
    }

    pub fn add_mirror(&mut self, src: Id, plane: u8, keep: bool, datum: Id) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode { id: body, name: "feat-name-mirror".into(), kind: FeatureKind::Mirror { src, plane, keep, datum, body, face: None }, parent, dirty: true, suppressed: false });
        body
    }

    /// Translate and rotate body `src` by matrix `mat` (3x4) as a parametric feature, moving the B-rep.
    pub fn add_move(&mut self, src: Id, mat: [f64; 12]) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        // the move of a body stands in the part the body stands in: laid in the assembly in hand, it read a body of
        // another component and was refused
        let parent = Some(self.body_owner(src).unwrap_or_else(|| self.body_parent()));
        self.push_timeline(FeatureNode { id: body, name: "feat-name-move".into(), kind: FeatureKind::Move { src, mat, body }, parent, dirty: true, suppressed: false });
        body
    }

    /// Hole with a type: `kind` is 0 for a plain hole, 1 for a counterbore and 2 for a countersink; `dia2` and
    /// `depth2` are the parameters of the recess.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hole_typed(&mut self, src: Id, face: crate::feature::FaceKey, tool: super::HoleTool) -> Id {
        self.add_hole_at(src, face, face.centroid, tool)
    }

    /// A hole in `face` of `src` whose centre is `at`, a point on that face in the body's frame (where it was
    /// clicked): the rebuild drills at that point brought onto the face as it stands then, so two holes in one
    /// face stand apart and a hole keeps its place when the face moves along its normal.
    pub fn add_hole_at(&mut self, src: Id, face: crate::feature::FaceKey, at: [f64; 3], tool: super::HoleTool) -> Id {
        let super::HoleTool { kind, diameter, depth, dia2, depth2 } = tool;
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        let (point, normal) = (at, face.normal);
        // A pick gives a specific face, which becomes an `Id` query. The fingerprint travels with it not for
        // matching but so that a refusal can one day say where that face was when it was picked.
        let face = crate::refs::Ref::one(face.id, crate::refs::Fingerprint { centroid: face.centroid, normal: face.normal });
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-hole".into(),
            kind: FeatureKind::Hole { src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch: 0, flip: false, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Holes at sketch points: one hole per isolated point of `sketch`. `src` is the stock body, drilled along
    /// the sketch normal (`flip` reverses it). The hole parameters (`kind`, `dia2`, `depth2`) are as for a
    /// single hole. One timeline node covers every hole.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hole_from_sketch(&mut self, src: Id, sketch: Id, tool: super::HoleTool, flip: bool) -> Id {
        let super::HoleTool { kind, diameter, depth, dia2, depth2 } = tool;
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        // The sketch-driven form picks no face at all: the reference is empty and never reaches
        // resolution.
        let face = crate::refs::Ref::one(0, crate::refs::Fingerprint::default());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-holes-sketch".into(),
            kind: FeatureKind::Hole { src, face, point: [0.0; 3], normal: [0.0, 0.0, 1.0], diameter, depth, kind, dia2, depth2, sketch, flip, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
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
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-thread".into(),
            kind: FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Auger: a helical flight on shaft `src`, associated with the circular edge `edge`.
    pub fn add_auger(&mut self, src: Id, edge: u32, spec: crate::thread::AugerSpec, length: f64, lead_in: f64, lead_out: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-auger".into(),
            kind: FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }
}
