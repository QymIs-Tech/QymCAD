//! THE KERNEL AS THE MODEL SEES IT: the trait, the report of a rebuild, its watch, the external references.

use super::*;

pub trait Kernel {
    fn extrude(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    fn revolve(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    fn boolean(&self, body: Id, base: Extruded, tool: Extruded, op: u8, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    /// Extrude a region from the exact profile `profile` (the `geom::encode_profile` encoding: an outer
    /// contour plus holes made of real edges — lines, arcs, circles) to a height, giving a body with exact
    /// faces.
    fn extrude_region(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    /// Extrude a profile and combine it with the already built body `src` (`op`: 0 cut, 1 union, 2
    /// intersection).
    fn combine(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    /// Like `combine`, but the tool is a region from the exact profile `profile` (outer contour plus holes).
    fn combine_region(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    /// Extrude several profiles (all from one sketch and plane `place`, to `height`), merge them into one
    /// tool and apply a single boolean against body `src`, giving one body. A zero `src` produces a new body
    /// from the tool itself and ignores `op`. This replaces chains of extrude plus boolean, keeping one
    /// operation in one node.
    ///
    /// `caps` holds the name descriptors of the start and end caps from the document name table: a cap is not
    /// produced by a profile edge, so its name arrives as a separate parameter rather than inside the
    /// encoding.
    fn combine_region_multi(&self, bo: crate::model::BodyOp, profiles: &[Vec<f64>], span: ToolSpan, place: [f64; 12], sides: Sides<'_>) -> Result<Built, crate::errors::CoreError>;
    /// Revolve the exact profile `profile` about an axis (0 for X, 1 for Y) by an angle, giving a body with
    /// exact faces.
    fn revolve_region(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<Built, crate::errors::CoreError>;
    /// Revolve a region about an arbitrary axis (`origin` and `dir` in profile local space), that is, a datum
    /// axis. The default implementation is a fallback for the mock: an ordinary revolve about X.
    fn revolve_region_axis(&self, body: Id, profile: &[f64], _line: AxisLine, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<Built, crate::errors::CoreError> {
        self.revolve_region(body, profile, 0, angle_deg, place, caps)
    }

    /// Revolve several profiles, merge them into one tool and apply a single boolean against body `src`,
    /// giving one body and one timeline node. A zero `src` produces a new body and ignores `op`. `origin_dir`
    /// is an arbitrary axis in profile local space (a datum or a centreline); `None` uses the X or Y axis
    /// from `axis`.
    ///
    /// The default implementation is a fallback for the mock and for builds without OCCT: the profiles are
    /// revolved one at a time and added together with booleans. The shape comes out the same; what is missing
    /// is merging touching contours into one face.
    fn revolve_region_multi(
        &self,
        bo: crate::model::BodyOp,
        profiles: &[Vec<f64>],
        about: RevolveAbout,
        angle_deg: f64,
        place: [f64; 12],
        _sides: Sides<'_>,
    ) -> Result<Built, crate::errors::CoreError> {
        let crate::model::BodyOp { src, op, body } = bo;
        // The fallback is honest: a single profile without a boolean is reproduced exactly, arbitrary axis
        // included. Merging several profiles or combining with a body needs a real kernel, and silently
        // returning a boss instead of a cut would give a test a green light on the wrong shape.
        if src != 0 || profiles.len() > 1 {
            return Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::RevolveProfile));
        }
        let _ = op;
        let first = profiles.first().ok_or(crate::errors::CoreError::NoContours)?;
        // the fallback names nothing: the names of the sides and caps are the bridge's rule
        let cap2 = [0, 0];
        match about.line {
            Some(line) => self.revolve_region_axis(body, first, line, angle_deg, place, cap2),
            None => self.revolve_region(body, first, about.axis, angle_deg, place, cap2),
        }
    }

    /// Sweep several profiles along one path, merge them and apply a single boolean against body `src`. See
    /// [`Kernel::revolve_region_multi`].
    fn sweep_multi(
        &self,
        bo: crate::model::BodyOp,
        profiles: &[Vec<f64>],
        profile_place: [f64; 12],
        path: &[f64],
        path_place: [f64; 12],
        _sides: Sides<'_>,
    ) -> Result<Built, crate::errors::CoreError> {
        let crate::model::BodyOp { src, op, body } = bo;
        if src != 0 || profiles.len() > 1 {
            return Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Sweep));
        }
        let _ = op;
        let first = profiles.first().ok_or(crate::errors::CoreError::NoContours)?;
        self.sweep(body, first, profile_place, path, path_place, [0, 0])
    }
    /// Sweep: the exact profile `profile` (`encode_profile`), placed by `profile_place`, is swept along the
    /// path `path` (`[1.0, loop_block]`, one open or closed contour) placed by `path_place`, giving a body
    /// with exact faces. The default implementation is a fallback, since there is no sweep without OCCT.
    #[allow(clippy::too_many_arguments)]
    fn sweep(&self, _body: Id, _profile: &[f64], _profile_place: [f64; 12], _path: &[f64], _path_place: [f64; 12], _caps: [u32; 2]) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Sweep))
    }
    /// Loft through sections. `sections` is the concatenation of loop blocks (each `[nedges, nedges*8]`),
    /// `offsets` marks where section i starts within `sections` (length nsec + 1), and `places` holds the 3x4
    /// placement per section (length nsec * 12). `ruled` gives straight faces and `solid` closes the result
    /// into a body. The default implementation is a fallback.
    #[allow(clippy::too_many_arguments)]
    fn loft(&self, _body: Id, _sections: LoftSections, _walls: LoftWalls, _kind: LoftBody, _sides: Sides<'_>) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Loft))
    }
    /// Lofted boolean: the lofted solid acts as a tool and is combined with body `src` (`op`: 0 cut, 1 union,
    /// 2 intersection). The section parameters are as in `loft`. The default implementation is a fallback.
    #[allow(clippy::too_many_arguments)]
    fn loft_combine(&self, _bo: crate::model::BodyOp, _sections: &[f64], _offsets: &[usize], _places: &[f64], _walls: LoftWalls, _sides: Sides<'_>) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::LoftBoolean))
    }
    /// Stepped hole: the tool (a cylinder plus a counterbore or countersink) in frame `pl`. What drills it
    /// travels as one record - see `HoleTool` - because the five numbers describing it are one thing, and
    /// the same record is what the timeline stores. The mock default cuts a plain cylinder through
    /// `combine_region`. The faces it makes are named after node `feature`.
    fn hole(&self, body: Id, src: Id, tool: crate::model::HoleTool, pl: [f64; 12], _feature: Id) -> Result<Built, crate::errors::CoreError> {
        let prof = crate::geom::encode_profile(&crate::geom::circle_contour(0.0, 0.0, tool.diameter / 2.0, 0.05), &[]);
        self.combine_region(body, src, &prof, -tool.depth.abs(), 0, pl)
    }
    /// Many holes at once (at the points of a sketch): one cutting tool per frame in `pls`, all merged and
    /// subtracted by a single boolean. `sites[k]` is the sketch point that placed hole k: its wall is named after it,
    /// so adding a point does not rename the neighbouring holes. The default implementation applies `hole` for each
    /// point in turn.
    fn holes(&self, body: Id, src: Id, tool: crate::model::HoleTool, pls: &[[f64; 12]], _sites: &[Id], feature: Id) -> Result<Built, crate::errors::CoreError> {
        let mut cur = src;
        let mut out: Result<Built, crate::errors::CoreError> = Err(crate::errors::CoreError::NoPointsForHoles);
        for pl in pls {
            out = self.hole(body, cur, tool, *pl, feature);
            // NOT `out.as_ref()?`: that hands back a BORROWED error, which this signature cannot convert.
            #[allow(clippy::question_mark)]
            if out.is_err() {
                return out;
            }
            cur = body; // After the first cut, work on the accumulated body.
        }
        out
    }
    /// Helical rib or groove from an exact profile.
    ///
    /// `profile` is the axial section computed by the model core from the thread standard
    /// (`qymcad_core::thread`) and encoded as an ordinary exact profile (segments plus arcs). The kernel only
    /// sweeps it along a helix of radius `radius` about the axis (`origin`, `dir`) and either subtracts it
    /// (`fuse = false`, a thread) or unites it (`fuse = true`, an auger flight). Thread and auger are thus one
    /// operation, and the profile stays exact, so a chamfer can be applied to it later.
    fn helical(&self, _h: Helical<'_>) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Helix))
    }
    /// Exact cylinder primitive (axis Z, base at z = 0): a native kernel solid with three faces, named after node
    /// `feature` - the same for the sphere, the cone and the torus.
    fn cylinder(&self, body: Id, r: f64, h: f64, feature: Id) -> Result<Built, crate::errors::CoreError>;
    /// Exact sphere (centred at the origin): one face.
    fn sphere(&self, body: Id, r: f64, feature: Id) -> Result<Built, crate::errors::CoreError>;
    /// Exact cone (r1 at z = 0 to r2 at z = h, axis Z).
    fn cone(&self, body: Id, r1: f64, r2: f64, h: f64, feature: Id) -> Result<Built, crate::errors::CoreError>;
    /// Exact torus (in the XY plane, axis Z).
    fn torus(&self, body: Id, major: f64, minor: f64, feature: Id) -> Result<Built, crate::errors::CoreError>;
    /// Fillet the edges of body `src` with radius `radius` (an empty `edges` means every edge).
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], roles: BlendRoles) -> Result<Built, crate::errors::CoreError>;
    /// Variable fillet specified at vertices: `verts` holds a vertex point and the radius there, and the
    /// kernel interpolates along the edge itself. A shared vertex has one radius for both neighbours, so a
    /// chain meets without a step — a property of the way it is specified rather than of any check. An
    /// endpoint without an entry uses `radius`. The mock default produces a constant fillet.
    fn fillet_at_vertices(&self, body: Id, src: Id, radius: f64, edges: &[u32], _verts: &[RadiusAt], roles: BlendRoles) -> Result<Built, crate::errors::CoreError> {
        self.fillet(body, src, radius, edges, roles)
    }
    /// Copy faces into a separate sheet: the bridge from the parametric model into the surface layer. `names`
    /// holds the names of the copies, whose provenance the model knows. The default is an honest refusal:
    /// there is nothing to fake a surface with, and silently reporting success would take the timeline into a
    /// state that does not exist.
    fn copy_faces(&self, _body: Id, _src: Id, _faces: &[u32], _feature: Id) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::CopyFaces))
    }

    /// Offset sheet: the faces `faces` of body `src` moved `dist` along their normals as body `body`, each named by
    /// `names`. The default refuses: the mock has no surfaces to move.
    fn offset_faces(&self, _body: Id, _src: Id, _faces: &[u32], _feature: Id, _dist: f64) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::OffsetSurface))
    }

    /// Patch: span a surface over a chain of edges. The default is a refusal.
    fn patch(&self, _body: Id, _src: Id, _edges: &[u32], _tangent: bool, _feature: Id) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Patch))
    }

    /// Replace faces of a body with a surface: the node that stitches the surface layer back into the
    /// timeline. The default is a refusal.
    fn replace_faces(&self, _body: Id, _src: Id, _faces: &[u32], _surface: Id) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::ReplaceFaces))
    }

    /// Trim a surface: the piece nearest to `keep` is retained. The default is a refusal.
    fn trim(&self, _body: Id, _src: Id, _tool: Id, _keep: [f64; 3]) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Trim))
    }

    /// Stitch sheets into one surface. If the result closes, the output is a solid. The default is a
    /// refusal.
    fn stitch(&self, _body: Id, _parts: &[Id], _tol: f64) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Stitch))
    }

    /// Turn a mesh into a solid of its flat faces, a polyhedron. The default is a refusal: only a real kernel has
    /// faces to sew.
    fn mesh_solid(&self, _body: Id, _mesh: &Mesh) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::MeshSolid))
    }

    /// Recognise a mesh into a body of exact surfaces, `tol` multiplying the distance a corner may lie from its
    /// surface. The default is a refusal: only a real kernel builds the faces found.
    fn mesh_recognised(&self, _body: Id, _mesh: &Mesh, _tol: f64, _sharp: f64) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::MeshRecognise))
    }

    /// Whether a body is a sheet (a surface) rather than a solid. The document has to know: a sheet has no
    /// volume, is not exported to CAM and does not count as the one body of a part. The default is `false`,
    /// since the mock works with solids.
    fn body_is_sheet(&self, _body: Id) -> bool {
        false
    }

    /// How many separate pieces (solids) a body is made of. The default is one, since the mock builds one solid a
    /// body.
    fn body_pieces(&self, _body: Id) -> u32 {
        1
    }

    /// The edges the last blend of `body` left out, taken once: (edges asked, edges left out). `None` when it
    /// took every edge.
    fn take_dropped_edges(&self, _body: Id) -> Option<DroppedEdges> {
        None
    }

    /// The solids body `body` is made of, in the kernel's order: the centre of mass and the volume of each. The default
    /// is none, since the mock builds one solid a body and has nothing to tell apart.
    fn body_solids(&self, _body: Id) -> Vec<([f64; 3], f64)> {
        Vec::new()
    }

    /// The solid number `index` of body `src` (in the order of `body_solids`) as body `body`. The default refuses, since
    /// the mock builds one solid a body.
    fn take_solid(&self, _body: Id, _src: Id, _index: usize) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::BodyInOnePiece)
    }

    /// Chamfer the edges of body `src` by `dist` (an empty `edges` means every edge).
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], roles: BlendRoles) -> Result<Built, crate::errors::CoreError>;
    /// Asymmetric chamfer. `TwoDist` uses setbacks `d1` on the reference face and `d2` on the adjacent one;
    /// `DistAngle` uses setback `d1` plus angle `d2` in degrees; `flip` selects which face is the reference.
    /// `ref_face` is the persistent id of a manually chosen reference face (0 selects it automatically from
    /// `flip`). `Symmetric` falls through to a plain `chamfer(d1)`. It requires an explicit edge selection,
    /// since asymmetry is not defined for "every edge". The mock default is a symmetric `chamfer(d1)`.
    #[allow(clippy::too_many_arguments)]
    fn chamfer_ex(&self, body: Id, src: Id, d1: f64, _shape: crate::model::ChamferShape, edges: &[u32], roles: BlendRoles) -> Result<Built, crate::errors::CoreError> {
        self.chamfer(body, src, d1, edges, roles)
    }
    /// Shell body `src`: open the faces named by `face_ids` (persistent ids) and leave walls of `thickness`.
    /// `outward` puts the wall outside instead of inside.
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, face_ids: &[u32]) -> Result<Built, crate::errors::CoreError>;

    /// Shell with named walls: `walls` holds pairs of "source face to the name of its inner wall".
    ///
    /// A wall is produced by offsetting a face rather than by copying it, so the names have to be seeded
    /// during construction: in the finished body an outer face and its wall are indistinguishable.
    fn shell_named(&self, body: Id, src: Id, thickness: f64, outward: bool, face_ids: &[u32], _feature: Id) -> Result<Built, crate::errors::CoreError> {
        self.shell(body, src, thickness, outward, face_ids)
    }
    /// Centred shell: a wall of `thickness` centred on the surface. The mock default shells inwards as
    /// usual.
    fn shell_center(&self, body: Id, src: Id, thickness: f64, face_ids: &[u32]) -> Result<Built, crate::errors::CoreError> {
        self.shell(body, src, thickness, false, face_ids)
    }
    /// Push-pull a face: planar face `face` of body `src` moves along its own normal by `dist` (positive adds
    /// material, negative removes it), producing body `body`. Direct modelling: a body stops being hostage to
    /// its sketch and can be edited by grabbing a face and pulling it.
    fn push_face(&self, _body: Id, _src: Id, _face: u32, _dist: f64) -> Result<crate::geom::Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::PushFace))
    }
    /// Delete faces and heal: the faces named by `face_ids` are removed from body `src` and the neighbouring
    /// ones extended, producing body `body`. This is how a hole, a boss or a fillet is removed without taking
    /// the timeline apart.
    fn remove_faces(&self, _body: Id, _src: Id, _face_ids: &[u32]) -> Result<crate::geom::Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::RemoveFaces))
    }
    /// Thicken a face: face `face` of body `src` becomes a plate of `thickness` as a new body `body`. The
    /// source stays alive — the plate is a separate part rather than a reworking of the original.
    fn thicken_face(&self, _body: Id, _src: Id, _face: u32, _thickness: f64, _join: Id, _feature: Id) -> Result<crate::geom::Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Thicken))
    }
    /// Split faces by a plane without cutting the body: the body stays one, and the faces the plane crosses
    /// fall into pieces, producing `body`. This marks out a region rather than breaking the part apart.
    fn split_faces(&self, _body: Id, _src: Id, _origin: [f64; 3], _normal: [f64; 3]) -> Result<crate::geom::Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::SplitFaces))
    }
    /// Split a body by a plane: body `src` is cut by the plane (`origin`, `normal`) into pieces, and each
    /// piece becomes an independent body. The pieces are ordered by position along the normal, from bottom to
    /// top; without that ordering they would swap bodies every time the plane moves.
    ///
    /// `bodies` holds pre-allocated ids, one per piece. If the number of pieces differs, an error is
    /// returned: silently losing a piece is worse than refusing.
    fn split_body(&self, _bodies: &[Id], _src: Id, _origin: [f64; 3], _normal: [f64; 3], _feature: Id) -> Result<Vec<crate::geom::Built>, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::SplitBody))
    }
    /// Draft: tilt the faces named by `face_ids` on body `src` by `angle` degrees relative to the neutral
    /// plane (`np_origin`, `np_normal`) along the pull direction `pull`. Requires a real kernel; the mock is a
    /// stub.
    #[allow(clippy::too_many_arguments)]
    fn draft(&self, _body: Id, _src: Id, _face_ids: &[u32], _pull: DraftPull, _neutral: PlaneAt, _feature: Id) -> Result<Built, crate::errors::CoreError> {
        Err(crate::errors::CoreError::KernelRequired(crate::errors::Op::Draft))
    }
    /// Pattern of body `src`: unite its copies placed by the transforms in `transforms`. Every copy but the first is
    /// named as an instance of node `feature`.
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]], feature: Id) -> Result<Built, crate::errors::CoreError>;

    /// Mirror body `src` about world plane `plane`; `made` says what is kept and what the image is called.
    fn mirror(&self, body: Id, src: Id, plane: u8, made: Mirrored) -> Result<Built, crate::errors::CoreError>;

    /// Mirror about an arbitrary plane, that is, a datum or a face; `made` says what is kept and what the image is
    /// called.
    fn mirror_plane(&self, body: Id, src: Id, at: PlaneAt, made: Mirrored) -> Result<Built, crate::errors::CoreError>;
    /// Rigid translation and rotation of the B-rep of body `src` by matrix `mat` (3x4), producing `body`.
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<Built, crate::errors::CoreError>;
    /// A BODY BROUGHT IN FROM A FILE, landed through the door without losing it: the shape the kernel holds, cleaned and
    /// checked as every result is when it passes, kept as it came - and said - when it does not. `None` means the kernel
    /// does not hold that body.
    fn land_import(&self, _body: Id) -> Option<ImportLanded> {
        None
    }
    /// Re-tessellate the already built or imported shape of body `body` from the kernel cache (an external
    /// STEP solid has no recipe). `None` means the kernel does not hold that body.
    fn tessellate(&self, _body: Id) -> Option<Built> {
        None
    }
    /// Body-to-body boolean: `op` applied to the B-reps of bodies `a` and `b`, producing `body`. `op` is 0 for
    /// a cut (a minus b), 1 for a union and 2 for an intersection. Both operands have to be built.
    fn body_boolean(&self, body: Id, a: Id, b: Id, op: u8) -> Result<Built, crate::errors::CoreError>;

    /// HOW MANY THREADS THE KERNEL IS WILLING TO BE DIVIDED INTO. One means "compute in this thread", which is
    /// what a mock says and what a person asking for a single-threaded rebuild gets.
    fn workers(&self) -> usize {
        1
    }

    /// A KERNEL OF ITS OWN FOR ANOTHER THREAD, holding the bodies asked for AND NOTHING ELSE.
    ///
    /// This is what lets a wave of independent nodes be computed at once. The shapes are MOVED rather than
    /// shared: two threads over one shape is precisely what this kernel is not safe for, and a copy of a body
    /// costs as much as the operation would. `None` means the kernel cannot divide itself, and the rebuild
    /// stays on one thread - which is the honest answer for a mock.
    fn split_off(&self, _bodies: &[Id]) -> Option<Box<dyn KernelWorker>> {
        None
    }

    /// Whether two live bodies share a sub-shape (a face or an edge). The batch rebuild asks this to keep bodies
    /// that share geometry on one worker thread: meshing or healing a shared sub-shape on two threads at once is not
    /// safe. The default is `false` - a kernel whose bodies never share geometry, or a test stub, needs no grouping.
    fn shares(&self, _a: Id, _b: Id) -> bool {
        false
    }

    /// The bodies come home: the shapes the worker was given, plus whatever it built.
    fn absorb(&self, _worker: Box<dyn KernelWorker>) {}

    /// Edge names: pairs of "edge to its two faces", by face name. Empty when the kernel does not supply
    /// them.
    fn edge_face_pairs(&self, _body: Id) -> Vec<EdgeFaces> {
        Vec::new()
    }

    /// NAME WHAT THE HISTORY OF AN OPERATION WAS SILENT ABOUT: the pieces of a split face, the seams by their named
    /// neighbours, then the faces by where they lie, then the edges by the pair of their faces. The rules live in the
    /// bridge; the names are given by the table the request carries. A kernel with no naming names nothing.
    fn name_silent(&self, _ask: SilentNaming<'_>) -> NamedAfter {
        NamedAfter::default()
    }

    /// Edge geometry of a body: for each edge, its persistent name, a polyline in body local space and, for a
    /// circular edge, its exact centre, axis and radius. Needed by projection into a sketch: a circle has to
    /// project as a circle rather than as a tessellated polyline.
    #[allow(clippy::type_complexity)]
    fn body_edge_geometry(&self, _body: Id) -> Vec<(u32, Vec<[f64; 3]>, Option<([f64; 3], [f64; 3], f64)>)> {
        Vec::new()
    }

    fn edges(&self, _body: Id) -> Vec<crate::geom::MeshEdge> {
        Vec::new()
    }
    /// Axis of a cylindrical or conical face `face_id` of a body (origin and direction in body local space),
    /// used by a datum axis bound to a face. The default is `None`, since a mock supplies no face axis.
    fn face_axis(&self, _body: Id, _face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        None
    }
}

/// Result of a `regenerate` pass: which bodies were rebuilt, together with their faces (the application puts
/// those into its own face cache), and the per-node errors, whose nodes stay dirty.
#[derive(Default, Debug)]
pub struct RegenReport {
    /// (body id, faces) for the bodies rebuilt in this pass.
    pub built: Vec<(Id, Vec<MeshFace>)>,
    /// (node id, error code) for the nodes that could not be rebuilt.
    ///
    /// A code rather than a phrase: the core is a library with no language of its own, while the application
    /// has to show text in the user's language (see `crate::errors`). It also makes errors distinguishable
    /// programmatically instead of by substring matching.
    pub errors: Vec<(Id, crate::errors::CoreError)>,
    /// Repairs: geometry references resolved not by persistent id but by geometric fingerprint.
    ///
    /// Doing that silently and afresh on every rebuild means the reference is "found" every time, possibly the
    /// wrong one, with nothing reporting it. The event now appears in the report and the key itself is updated
    /// with the id found, so the guess happens once rather than every time.
    pub rebinds: Vec<Rebind>,
    /// The rebuild was stopped part-way (see [`RegenWatch`]). The report is then incomplete, and concluding
    /// from it that a feature failed to build is wrong — the pass simply never reached it. The caller has to
    /// discard such a result entirely rather than apply it partially.
    pub cancelled: bool,
    /// HOW MANY NODES WENT INTO EACH BATCH COMPUTED SIDE BY SIDE.
    ///
    /// One number per batch, in the order they were computed. An empty list means the rebuild went node by
    /// node - the honest answer for a single-threaded setting, for a mock, and for a document whose every node
    /// waits for the one before it.
    pub waves: Vec<usize>,
    /// WHAT EACH NODE COST, in microseconds: (node id, the kind's name, the time it took).
    ///
    /// Only the nodes that were actually rebuilt are here - a pass that walks past a clean node costs nothing
    /// and says nothing. Without this, "the rebuild is slow" can only be answered with a stopwatch over the
    /// whole document, and a stopwatch cannot tell a boolean from a tessellation. The cost is one clock
    /// reading per rebuilt node.
    pub spent: Vec<(Id, String, u128)>,
}

/// A KERNEL THAT CAN TRAVEL: the half of a kernel handed to a worker thread for one wave.
///
/// It is a kernel like any other - the parcels of work know nothing about which one they are run on - and it
/// is `Send`, which the shared one is not: the shared one is reachable from the document's thread.
pub trait KernelWorker: Send {
    fn kernel(&self) -> &dyn Kernel;
    /// For the shared kernel to take its bodies back, it has to recognise its own kind.
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

/// Rebuild observer: learns how much has been done and can stop the rebuild.
///
/// Rebuilding a large assembly takes seconds, during which a window can do nothing but spin: there is no way
/// to report how much is left and nowhere to change one's mind. Both questions concern one place — the loop
/// over the timeline — so one interface answers both.
///
/// The core knows nothing about threads or buttons: it asks before each node and obeys the answer.
pub trait RegenWatch {
    /// Node `done` of `total` is starting. Returning `false` stops the rebuild.
    fn step(&self, done: usize, total: usize) -> bool {
        let _ = (done, total);
        true
    }
    /// Has a stop been asked at all, between the nodes or while one was being built?
    fn stopped(&self) -> bool {
        false
    }
}

/// An observer that does nothing and never interferes, for callers that do not need cancellation.
pub struct NoWatch;
impl RegenWatch for NoWatch {}

/// A record of a geometry reference having been repaired.
#[derive(Clone, Debug, PartialEq)]
pub struct Rebind {
    /// Timeline node whose reference was lost.
    pub node: Id,
    /// Body that was searched.
    pub body: Id,
    /// What was repaired (a face or an edge) and onto what, for display in the tree and in the status
    /// line.
    pub what: String,
}

/// Result of the placement pass (solving the mates): diagnostics of the joint graph.
#[derive(Default, Debug)]
pub struct JointReport {
    /// Joints that close a loop, with both components already placed; these need the iterative solver.
    pub loops: Vec<Id>,
    /// Isolated joints with no path to a grounded anchor.
    pub unsolved: Vec<Id>,
    /// (joint id, error text): an unresolved connector and similar failures.
    pub errors: Vec<(Id, String)>,
}

// --- External references: controlled top-down design. ---

/// Source geometry of an external reference, in the local space of its owning component. It is resolved into
/// the local space of the consumer through `world_transform`, world space being derived.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExternalGeom {
    /// A face of a body in another component: the body id plus the persistent face key.
    Face(Id, FaceKey),
}

/// An external reference: a controlled, directed, cross-component dependency for top-down and in-context
/// design. Without it, component isolation blocks references between parts outright. The dependency is
/// explicit, enumerable and breakable rather than a silent `body_id`. Source geometry is resolved into
/// consumer space as `consumer_local = relative_transform(src_owner, from) * src_local`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalRef {
    pub id: Id,
    /// Consumer component: the one that references outwards.
    pub from_component: Id,
    /// Source geometry in another component.
    pub to_geometry: ExternalGeom,
}

impl ExternalRef {
    /// Source body of the reference, when the geometry is bound to a body.
    pub fn source_body(&self) -> Option<Id> {
        match &self.to_geometry {
            ExternalGeom::Face(body, _) => Some(*body),
        }
    }
}
