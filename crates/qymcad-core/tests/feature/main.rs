//! Data model of the feature timeline: the sketch plane, lifting 2D into 3D, projecting the pools into
//! `timeline`, and the serialisation round trip.

use std::cell::RefCell;
use std::collections::HashSet;

use qymcad_core::feature::{apply12, AnchorRef, BasePlane, FaceKey, JointKind, Kernel, SketchPlane, PLACE_IDENTITY};
use qymcad_core::geom::{Built, Mesh, MeshFace, Point2, Point3};
use qymcad_core::model::{ElemSnapshot, Id, Project};
use qymcad_core::model::ArrayAxis;

/// Mock geometry kernel: a call log plus the set of bodies that have a shape (for combine, fillet, chamfer).
/// A body is marked by a single vertex, parameterised along x and placed by the `place` transform.
#[derive(Default)]
pub(crate) struct MockKernel {
    calls: RefCell<Vec<String>>,
    /// The spans of the tools the model handed, in order.
    spans: RefCell<Vec<qymcad_core::feature::ToolSpan>>,
    shapes: RefCell<HashSet<Id>>,
    fail: bool,
    /// Configurable body edges, for an associative `FromEdge` axis. Empty by default.
    edges: RefCell<Vec<qymcad_core::geom::MeshEdge>>,
    /// Configurable face axis, for `FromFace`.
    face_axis: RefCell<Option<([f64; 3], [f64; 3])>>,
    /// Return no face for a body.
    ///
    /// By default an extrude returns one face named 1: without a single face the body has no pool for
    /// references to resolve against, and a shell (or any face-based tool) legitimately answers "face not
    /// found", so the test would be checking the absence of faces rather than the model. Tests that fill
    /// `regen_faces` themselves set this flag to keep the mock out of the way.
    no_faces: bool,
}
impl MockKernel {
    fn placed(x: f64, m: [f64; 12]) -> Built {
        let v = Point3::new(m[0] * x + m[3], m[4] * x + m[7], m[8] * x + m[11]);
        Built { mesh: Mesh { verts: vec![v], tris: vec![], ..Default::default() }, faces: Vec::new() }
    }
    fn count(&self) -> usize {
        self.calls.borrow().len()
    }
    fn need_src(&self, src: Id) -> Result<(), qymcad_core::errors::CoreError> {
        if self.shapes.borrow().contains(&src) {
            Ok(())
        } else {
            Err(qymcad_core::errors::CoreError::SourceBodyNotBuilt)
        }
    }
}
impl Kernel for MockKernel {
    fn extrude(&self, body: Id, _p: &[f64], height: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("extrude h={height}"));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        let Built { mesh, mut faces } = Self::placed(height, place);
        if !self.no_faces {
            let c = mesh.verts.first().copied().unwrap_or(Point3::new(0.0, 0.0, 0.0));
            faces.push(MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: c, area: 1.0, id: 1 });
        }
        Ok(Built { mesh, faces })
    }
    fn revolve(&self, body: Id, _p: &[f64], _axis: u8, angle: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve a={angle}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn sweep(&self, body: Id, profile: &[f64], _pp: [f64; 12], path: &[f64], path_place: [f64; 12], _caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        // The lengths of the profile and path encodings are logged; no kernel geometry is built here.
        self.calls.borrow_mut().push(format!("sweep prof={} path={}", profile.len(), path.len()));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(path.len() as f64, path_place))
    }
    fn loft(
        &self,
        body: Id,
        sections: qymcad_core::feature::LoftSections,
        walls: qymcad_core::feature::LoftWalls,
        _kind: qymcad_core::feature::LoftBody,
        _sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::feature::LoftSections { data: sections, offsets, places } = sections;
        // The number of sections (`offsets.len() - 1`) and the lengths of the data and placements are logged.
        // The log keeps the old word: what is checked is the value that reached the kernel, not its spelling.
        let ruled = walls == qymcad_core::feature::LoftWalls::Ruled;
        self.calls.borrow_mut().push(format!("loft nsec={} data={} places={} ruled={ruled}", offsets.len().saturating_sub(1), sections.len(), places.len()));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(offsets.len() as f64, places.get(0..12).map(|s| s.try_into().unwrap()).unwrap_or(PLACE_IDENTITY)))
    }
    fn loft_combine(
        &self,
        bo: qymcad_core::model::BodyOp,
        _sections: &[f64],
        offsets: &[usize],
        _places: &[f64],
        walls: qymcad_core::feature::LoftWalls,
        _sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        // Lofted boolean: the number of sections and the operation are logged, and the target body `src` is
        // required to exist.
        let src_ok = self.shapes.borrow().contains(&src);
        let ruled = walls == qymcad_core::feature::LoftWalls::Ruled;
        self.calls.borrow_mut().push(format!("loft_combine nsec={} op={op} src_ok={src_ok} ruled={ruled}", offsets.len().saturating_sub(1)));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(op as f64, PLACE_IDENTITY))
    }
    fn draft(&self, body: Id, src: Id, face_ids: &[u32], pull: qymcad_core::feature::DraftPull, neutral: qymcad_core::feature::PlaneAt, _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::feature::DraftPull { angle, dir: pull } = pull;
        let qymcad_core::feature::PlaneAt { origin: np_origin, normal: _np_normal } = neutral;
        // The face count, the angle, the pull direction and the neutral origin are logged.
        self.calls.borrow_mut().push(format!("draft n={} angle={angle} pull={pull:?} np_o={np_origin:?}", face_ids.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, PLACE_IDENTITY))
    }
    fn boolean(&self, body: Id, _base: qymcad_core::feature::Extruded, _tool: qymcad_core::feature::Extruded, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("boolean op={op}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(op as f64, place))
    }
    fn combine(&self, body: Id, src: Id, _p: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("combine op={op}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn extrude_region(&self, body: Id, _profile: &[f64], height: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("extrude h={height}"));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn revolve_region(&self, body: Id, _profile: &[f64], _axis: u8, angle: f64, place: [f64; 12], _caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve a={angle}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn revolve_region_axis(&self, body: Id, _profile: &[f64], line: qymcad_core::feature::AxisLine, angle: f64, place: [f64; 12], _caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve_axis o={:?} d={:?} a={angle}", line.origin, line.dir));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn hole(&self, body: Id, src: Id, tool: qymcad_core::model::HoleTool, place: [f64; 12], _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::HoleTool { kind, diameter, depth, dia2, depth2 } = tool;
        self.calls.borrow_mut().push(format!("hole k={kind} dia={diameter} depth={depth} dia2={dia2} depth2={depth2}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(diameter, place))
    }
    fn holes(&self, body: Id, src: Id, tool: qymcad_core::model::HoleTool, pls: &[[f64; 12]], _sites: &[Id], _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::HoleTool { kind, diameter, depth, dia2, depth2 } = tool;
        self.calls.borrow_mut().push(format!("holes k={kind} n={} dia={diameter} depth={depth} dia2={dia2} depth2={depth2}", pls.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(diameter, pls.first().copied().unwrap_or(PLACE_IDENTITY)))
    }
    fn cylinder(&self, body: Id, _r: f64, h: f64, _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("cylinder".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(h, PLACE_IDENTITY))
    }
    fn sphere(&self, body: Id, r: f64, _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("sphere".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(r, PLACE_IDENTITY))
    }
    fn cone(&self, body: Id, r1: f64, _r2: f64, _h: f64, _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("cone".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(r1, PLACE_IDENTITY))
    }
    fn torus(&self, body: Id, major: f64, _minor: f64, _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("torus".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(major, PLACE_IDENTITY))
    }
    fn combine_region(&self, body: Id, src: Id, _profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("combine op={op}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn combine_region_multi(
        &self,
        bo: qymcad_core::model::BodyOp,
        profiles: &[Vec<f64>],
        span: qymcad_core::feature::ToolSpan,
        place: [f64; 12],
        _sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        // as the bridge places the tool: from where it starts; meeting its ends with the body is the bridge's rule
        let height = span.length;
        let mut place = place;
        place[3] += place[2] * span.start;
        place[7] += place[6] * span.start;
        place[11] += place[10] * span.start;
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        self.calls.borrow_mut().push(format!("combine_multi n={} op={op} src={src} h={height}", profiles.len()));
        self.spans.borrow_mut().push(span);
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        if src != 0 {
            self.need_src(src)?;
        }
        self.shapes.borrow_mut().insert(body);
        // a new body has the face an extrusion has: a box is laid this way as well as a sketch extruded
        let Built { mesh, mut faces } = Self::placed(height, place);
        if src == 0 && !self.no_faces {
            let c = mesh.verts.first().copied().unwrap_or(Point3::new(0.0, 0.0, 0.0));
            faces.push(MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: c, area: 1.0, id: 1 });
        }
        Ok(Built { mesh, faces })
    }
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], _roles: qymcad_core::feature::BlendRoles) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("fillet r={radius} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(radius, PLACE_IDENTITY))
    }
    fn helical(&self, h: qymcad_core::feature::Helical<'_>) -> Result<Built, qymcad_core::errors::CoreError> {
        // The axis and radius resolved from the edge are logged, which is what makes the associativity
        // checkable; the mock builds no geometry.
        self.calls.borrow_mut().push(format!("helical r={} oz={} lead={} L={} starts={} fuse={} prof={}", h.radius, h.origin[2], h.lead, h.length, h.starts, h.fuse, h.profile.len()));
        self.need_src(h.src)?;
        self.shapes.borrow_mut().insert(h.body);
        Ok(Self::placed(h.radius, PLACE_IDENTITY))
    }
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], _roles: qymcad_core::feature::BlendRoles) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("chamfer d={dist} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(dist, PLACE_IDENTITY))
    }
    fn chamfer_ex(
        &self,
        body: Id,
        src: Id,
        d1: f64,
        shape: qymcad_core::model::ChamferShape,
        edges: &[u32],
        _roles: qymcad_core::feature::BlendRoles,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::ChamferShape { mode, d2, flip, ref_face } = shape;
        self.calls.borrow_mut().push(format!("chamfer_ex d1={d1} d2={d2} mode={mode:?} flip={flip} rf={ref_face} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(d1, PLACE_IDENTITY))
    }
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("shell t={thickness} out={outward} n={}", faces.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(thickness, PLACE_IDENTITY))
    }
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]], _feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        // The translations of each instance are recorded too, so a test can check the parametric step and
        // angle.
        let txs: Vec<String> = transforms.iter().map(|m| format!("({:.1},{:.1},{:.1})", m[3], m[7], m[11])).collect();
        self.calls.borrow_mut().push(format!("pattern n={} txs=[{}]", transforms.len(), txs.join(";")));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(transforms.len() as f64, PLACE_IDENTITY))
    }
    fn mirror(&self, body: Id, src: Id, plane: u8, made: qymcad_core::feature::Mirrored) -> Result<Built, qymcad_core::errors::CoreError> {
        let keep = made.keeps == qymcad_core::feature::MirrorKeeps::Both;
        self.calls.borrow_mut().push(format!("mirror p={plane} keep={keep}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(plane as f64, PLACE_IDENTITY))
    }
    fn mirror_plane(&self, body: Id, src: Id, at: qymcad_core::feature::PlaneAt, made: qymcad_core::feature::Mirrored) -> Result<Built, qymcad_core::errors::CoreError> {
        let keep = made.keeps == qymcad_core::feature::MirrorKeeps::Both;
        self.calls.borrow_mut().push(format!("mirror_plane o={:?} n={:?} keep={keep}", at.origin, at.normal));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, PLACE_IDENTITY))
    }
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("move".into());
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, mat))
    }
    fn body_boolean(&self, body: Id, a: Id, b: Id, _op: u8) -> Result<Built, qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("body_boolean".into());
        self.need_src(a)?;
        self.need_src(b)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, qymcad_core::feature::PLACE_IDENTITY))
    }
    fn edges(&self, _body: Id) -> Vec<qymcad_core::geom::MeshEdge> {
        self.edges.borrow().clone()
    }
    fn face_axis(&self, _body: Id, _face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        *self.face_axis.borrow()
    }
}

/// A closed square sketch; returns its id.
fn square(p: &mut Project, name: &str) -> u64 {
    p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true)
}

/// A fresh document with an active part, so bodies and sketches land inside it: strict isolation forbids
/// bodies in an assembly.
fn part_project() -> Project {
    let mut p: Project = Default::default();
    p.new_document();
    p
}

mod threads;
mod timeline;
mod building;
mod modifiers;
mod suppress;
mod datums;
mod sweeps;
mod plans;
