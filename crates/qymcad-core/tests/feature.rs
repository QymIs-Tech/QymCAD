//! Data model of the feature timeline: the sketch plane, lifting 2D into 3D, projecting the pools into
//! `timeline`, and the serialisation round trip.

use std::cell::RefCell;
use std::collections::HashSet;

use qymcad_core::feature::{apply12, AnchorRef, BasePlane, FaceKey, JointKind, Kernel, SketchPlane, PLACE_IDENTITY};
use qymcad_core::geom::{Mesh, MeshFace, Point2, Point3};
use qymcad_core::model::{Id, Project};

/// Mock geometry kernel: a call log plus the set of bodies that have a shape (for combine, fillet, chamfer).
/// A body is marked by a single vertex, parameterised along x and placed by the `place` transform.
#[derive(Default)]
struct MockKernel {
    calls: RefCell<Vec<String>>,
    shapes: RefCell<HashSet<Id>>,
    fail: bool,
    /// Configurable body edges, for an associative `FromEdge` axis. Empty by default.
    edges: RefCell<Vec<qymcad_core::geom::MeshEdge>>,
    /// Configurable face axis, for `FromFace`.
    face_axis: RefCell<Option<([f64; 3], [f64; 3])>>,
    /// The "edge to its two faces" topology, from which the model derives edge names.
    edge_pairs: RefCell<Vec<(u32, u32, u32)>>,
    /// What the model asked to be renamed, as old to new.
    renamed: RefCell<Vec<(u32, u32)>>,
    /// Return no face for a body.
    ///
    /// By default an extrude returns one face named 1: without a single face the body has no pool for
    /// references to resolve against, and a shell (or any face-based tool) legitimately answers "face not
    /// found", so the test would be checking the absence of faces rather than the model. Tests that fill
    /// `regen_faces` themselves set this flag to keep the mock out of the way.
    no_faces: bool,
}
impl MockKernel {
    fn placed(x: f64, m: [f64; 12]) -> (Mesh, Vec<MeshFace>) {
        let v = Point3::new(m[0] * x + m[3], m[4] * x + m[7], m[8] * x + m[11]);
        (Mesh { verts: vec![v], tris: vec![] }, Vec::new())
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
    fn extrude(&self, body: Id, _p: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("extrude h={height}"));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        let (mesh, mut faces) = Self::placed(height, place);
        if !self.no_faces {
            let c = mesh.verts.first().copied().unwrap_or(Point3::new(0.0, 0.0, 0.0));
            faces.push(MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: c, area: 1.0, id: 1 });
        }
        Ok((mesh, faces))
    }
    fn revolve(&self, body: Id, _p: &[f64], _axis: u8, angle: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve a={angle}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn sweep(&self, body: Id, profile: &[f64], _pp: [f64; 12], path: &[f64], path_place: [f64; 12], _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The lengths of the profile and path encodings are logged; no kernel geometry is built here.
        self.calls.borrow_mut().push(format!("sweep prof={} path={}", profile.len(), path.len()));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(path.len() as f64, path_place))
    }
    fn loft(&self, body: Id, sections: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, _kind: qymcad_core::feature::LoftBody, _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
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
    fn loft_combine(&self, body: Id, src: Id, _sections: &[f64], offsets: &[usize], _places: &[f64], walls: qymcad_core::feature::LoftWalls, op: u8, _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // Lofted boolean: the number of sections and the operation are logged, and the target body `src` is
        // required to exist.
        let src_ok = self.shapes.borrow().contains(&src);
        let ruled = walls == qymcad_core::feature::LoftWalls::Ruled;
        self.calls.borrow_mut().push(format!("loft_combine nsec={} op={op} src_ok={src_ok} ruled={ruled}", offsets.len().saturating_sub(1)));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(op as f64, PLACE_IDENTITY))
    }
    fn draft(&self, body: Id, src: Id, face_ids: &[u32], angle: f64, pull: [f64; 3], np_origin: [f64; 3], _np_normal: [f64; 3], _sides: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The face count, the angle, the pull direction and the neutral origin are logged.
        self.calls.borrow_mut().push(format!("draft n={} angle={angle} pull={pull:?} np_o={np_origin:?}", face_ids.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, PLACE_IDENTITY))
    }
    fn boolean(&self, body: Id, _b: &[f64], _bh: f64, _t: &[f64], _th: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("boolean op={op}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(op as f64, place))
    }
    fn combine(&self, body: Id, src: Id, _p: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("combine op={op}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn extrude_region(&self, body: Id, _profile: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("extrude h={height}"));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn revolve_region(&self, body: Id, _profile: &[f64], _axis: u8, angle: f64, place: [f64; 12], _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve a={angle}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn revolve_region_axis(&self, body: Id, _profile: &[f64], origin: [f64; 3], dir: [f64; 3], angle: f64, place: [f64; 12], _caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("revolve_axis o={origin:?} d={dir:?} a={angle}"));
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(angle, place))
    }
    fn hole(&self, body: Id, src: Id, kind: u8, place: [f64; 12], dia: f64, depth: f64, dia2: f64, depth2: f64, _bore: u32, _extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("hole k={kind} dia={dia} depth={depth} dia2={dia2} depth2={depth2}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(dia, place))
    }
    fn holes(&self, body: Id, src: Id, kind: u8, pls: &[[f64; 12]], dia: f64, depth: f64, dia2: f64, depth2: f64, _bores: &[u32], _extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("holes k={kind} n={} dia={dia} depth={depth} dia2={dia2} depth2={depth2}", pls.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(dia, pls.first().copied().unwrap_or(PLACE_IDENTITY)))
    }
    fn cylinder(&self, body: Id, _r: f64, h: f64, _names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("cylinder".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(h, PLACE_IDENTITY))
    }
    fn sphere(&self, body: Id, r: f64, _names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("sphere".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(r, PLACE_IDENTITY))
    }
    fn cone(&self, body: Id, r1: f64, _r2: f64, _h: f64, _names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("cone".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(r1, PLACE_IDENTITY))
    }
    fn torus(&self, body: Id, major: f64, _minor: f64, _names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("torus".into());
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(major, PLACE_IDENTITY))
    }
    fn combine_region(&self, body: Id, src: Id, _profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("combine op={op}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn combine_region_multi(&self, body: Id, src: Id, profiles: &[Vec<f64>], height: f64, op: u8, place: [f64; 12], _caps: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("combine_multi n={} op={op} src={src} h={height}", profiles.len()));
        if self.fail {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        if src != 0 {
            self.need_src(src)?;
        }
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(height, place))
    }
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], _names: &[u32], _corners: &[u32], _all: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("fillet r={radius} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(radius, PLACE_IDENTITY))
    }
    fn helical(&self, h: qymcad_core::feature::Helical<'_>) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The axis and radius resolved from the edge are logged, which is what makes the associativity
        // checkable; the mock builds no geometry.
        self.calls.borrow_mut().push(format!(
            "helical r={} oz={} lead={} L={} starts={} fuse={} prof={}",
            h.radius, h.origin[2], h.lead, h.length, h.starts, h.fuse, h.profile.len()
        ));
        self.need_src(h.src)?;
        self.shapes.borrow_mut().insert(h.body);
        Ok(Self::placed(h.radius, PLACE_IDENTITY))
    }
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], _names: &[u32], _corners: &[u32], _all: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("chamfer d={dist} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(dist, PLACE_IDENTITY))
    }
    fn chamfer_ex(&self, body: Id, src: Id, d1: f64, d2: f64, mode: qymcad_core::feature::ChamferMode, flip: bool, ref_face: u32, edges: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("chamfer_ex d1={d1} d2={d2} mode={mode:?} flip={flip} rf={ref_face} n={}", edges.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(d1, PLACE_IDENTITY))
    }
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("shell t={thickness} out={outward} n={}", faces.len()));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(thickness, PLACE_IDENTITY))
    }
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The translations of each instance are recorded too, so a test can check the parametric step and
        // angle.
        let txs: Vec<String> = transforms.iter().map(|m| format!("({:.1},{:.1},{:.1})", m[3], m[7], m[11])).collect();
        self.calls.borrow_mut().push(format!("pattern n={} txs=[{}]", transforms.len(), txs.join(";")));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(transforms.len() as f64, PLACE_IDENTITY))
    }
    fn mirror(&self, body: Id, src: Id, plane: u8, keep: bool) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("mirror p={plane} keep={keep}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(plane as f64, PLACE_IDENTITY))
    }
    fn mirror_plane(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3], keep: bool) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push(format!("mirror_plane o={origin:?} n={normal:?} keep={keep}"));
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, PLACE_IDENTITY))
    }
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("move".into());
        self.need_src(src)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, mat))
    }
    fn body_boolean(&self, body: Id, a: Id, b: Id, _op: u8) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.calls.borrow_mut().push("body_boolean".into());
        self.need_src(a)?;
        self.need_src(b)?;
        self.shapes.borrow_mut().insert(body);
        Ok(Self::placed(1.0, qymcad_core::feature::PLACE_IDENTITY))
    }
    fn edges(&self, _body: Id) -> Vec<qymcad_core::geom::MeshEdge> {
        self.edges.borrow().clone()
    }
    fn edge_face_pairs(&self, _body: Id) -> Vec<(u32, u32, u32)> {
        self.edge_pairs.borrow().clone()
    }
    fn rename_edges(&self, _body: Id, pairs: &[(u32, u32)]) {
        self.renamed.borrow_mut().extend_from_slice(pairs);
    }
    fn face_axis(&self, _body: Id, _face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        *self.face_axis.borrow()
    }
}

/// A closed square sketch; returns its id.
fn square(p: &mut Project, name: &str) -> u64 {
    p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true)
}


// Topological naming: when a stored edge id of a feature comes loose (the topology above it changed), the
// reference is repaired from the geometric snapshot — the nearest current edge by midpoint and direction, and
// only on an unambiguous match.
#[test]
fn fillet_edge_ref_heals_stale_id_by_snapshot() {
    use qymcad_core::geom::MeshEdge;
    let mut p = Project::default();
    let (src, fid) = (100u64, 200u64);
    p.regen_edges.insert(src, vec![
        MeshEdge { id: 7, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
        MeshEdge { id: 8, mid: [10.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
    ]);
    // The feature selected the edge that used to have id 3; its snapshot is the position (0,0,5) along Z.
    p.edge_refs.insert(fid, vec![(3, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0])]);
    // Id 3 is gone, so the snapshot repairs it to the current edge 7, the nearest and unambiguous one.
    assert_eq!(p.resolve_edge_ids(fid, src, &[3]), vec![7], "a lost id must be repaired from the snapshot");
    // A valid id passes through unchanged.
    assert_eq!(p.resolve_edge_ids(fid, src, &[8]), vec![8], "a valid id must be left alone");
    // An ambiguous match (two equally close edges) is not repaired and the edge drops out.
    p.regen_edges.insert(src, vec![
        MeshEdge { id: 7, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
        MeshEdge { id: 9, mid: [0.0, 0.0, 5.0], dir: [0.0, 0.0, 1.0], ..Default::default() },
    ]);
    assert!(p.resolve_edge_ids(fid, src, &[3]).is_empty(), "an ambiguous match must not be repaired");
}

#[test]
fn thread_feature_associative_via_circular_edge() {
    // A thread takes its axis and radius from the circular edge of `src`, associatively and from the actual
    // geometry, rather than from a snapshot.
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 4.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 10.0, pitch: 1.5, internal: false, ..Default::default() }, 8.0, 0.0, 0.0);
    p.regenerate(&k);
    assert!(p.mesh_index(thr).is_some(), "the thread body must be built (the modifier produced a body)");
    // The kernel received the axis (centre at z = 5) and the radius (4) from the edge: associativity.
    assert!(
        k.calls.borrow().iter().any(|c| c.contains("helical r=4 oz=5")),
        "the thread must be called with the axis and radius taken from the edge: {:?}",
        k.calls.borrow()
    );
}

// Thread parameters are validated before the kernel, so bad values give a readable error on a red node rather
// than a silent no-op or a crash in the sweep.
#[test]
fn thread_validates_depth_vs_radius() {
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 4.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    // External thread: a depth of 5 against a radius of 4 would push the profile past the axis, which has to
    // produce a readable error.
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 30.0, pitch: 20.0, internal: false, ..Default::default() }, 8.0, 0.0, 0.0);
    p.regenerate(&k);
    let err = p.regen_errors.get(&thr).cloned();
    eprintln!("thread error (depth 5 > r 4): {err:?}");
    // Checked by error code rather than by a word: a substring check would go blind on any text edit or
    // translation.
    assert!(
        matches!(err, Some(qymcad_core::errors::CoreError::ThreadDepthTooDeep { .. })),
        "a depth at or beyond the radius must give a readable error about the depth specifically, but got: {err:?}"
    );
    assert!(!k.calls.borrow().iter().any(|c| c.contains("helical r=")), "the kernel must not be called with an invalid depth");
}

// A deep, sharp profile (depth close to the pitch, a large angle) is no longer rejected by validation: the
// kernel builds a true point, capping the depth of a sharp V, quickly and validly. Rejecting it blocked large
// angles, which could not be specified beyond about 70 to 80 degrees.
#[test]
fn thread_deep_profile_reaches_kernel_no_error() {
    use qymcad_core::geom::MeshEdge;
    let edge = MeshEdge { id: 42, center: [0.0, 0.0, 5.0], axis: [0.0, 0.0, 1.0], radius: 25.0, ..Default::default() };
    let k = MockKernel { edges: RefCell::new(vec![edge]), ..Default::default() };
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    // A depth of 5 against a pitch of 5 at 60 degrees used to be rejected; it now builds as a sharp V with a
    // capped depth.
    let thr = p.add_thread(src, 42, qymcad_core::thread::ThreadSpec { nominal_d: 10.0, pitch: 5.0, internal: false, ..Default::default() }, 100.0, 0.0, 0.0);
    p.regenerate(&k);
    let err = p.regen_errors.get(&thr).cloned();
    eprintln!("deep, sharp profile: err={err:?}");
    assert!(err.is_none(), "a deep profile must no longer be rejected: {err:?}");
    assert!(k.calls.borrow().iter().any(|c| c.contains("helical r=")), "the kernel must be called, the profile being valid");
}

#[test]
fn thread_axis_orients_into_material() {
    use qymcad_core::geom::Point3;
    use qymcad_core::model::orient_axis_into_mesh;
    let center = [0.0, 0.0, 0.0];
    // The rim is at (0,0,0) and the material (the vertices) lies at z > 0, so the axis has to point at +Z.
    let verts_up: Vec<Point3> = (1..=10).map(|i| Point3::new(0.0, 0.0, i as f64)).collect();
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, -1.0], &verts_up), [0.0, 0.0, 1.0], "the axis must be turned towards the material (+Z)");
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, 1.0], &verts_up), [0.0, 0.0, 1.0], "an already correct axis must be left alone");
    // Material at z < 0 puts the axis at -Z.
    let verts_dn: Vec<Point3> = (1..=10).map(|i| Point3::new(0.0, 0.0, -(i as f64))).collect();
    assert_eq!(orient_axis_into_mesh(center, [0.0, 0.0, 1.0], &verts_dn), [0.0, 0.0, -1.0], "the axis must be turned towards the material (-Z)");
}

#[test]
fn thread_consumes_src_and_survives_serde() {
    // A thread is a modifier: it hides `src` and leaves one body, and every parameter survives
    // serialisation.
    use qymcad_core::feature::FeatureKind;
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);
    let spec = qymcad_core::thread::ThreadSpec {
        standard: qymcad_core::thread::ThreadStandard::TrapezoidalTr,
        nominal_d: 12.0,
        pitch: 1.25,
        starts: 2,
        left: true,
        internal: true,
        fit: 0.15,
        ..Default::default()
    };
    let thr = p.add_thread(src, 7, spec, 6.0, 1.0, 1.0);
    let node = p.timeline.iter().find(|n| n.id == thr).unwrap();
    assert_eq!(node.kind.consumed_body(), Some(src), "a thread consumes `src`, leaving one body rather than two");
    assert_eq!(node.kind.body(), Some(thr), "the output of a thread is its body");

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let p2 = qymcad_core::model::from_ron(&ron).unwrap();
    let n2 = p2.timeline.iter().find(|n| n.id == thr).unwrap();
    match n2.kind {
        FeatureKind::Thread { src: s, edge, spec, length, lead_in, lead_out, .. } => {
            assert_eq!((s, edge, spec.internal, spec.starts, spec.left), (src, 7, true, 2, true), "the parameters must survive serialisation");
            assert_eq!(spec.standard, qymcad_core::thread::ThreadStandard::TrapezoidalTr, "the standard must survive serialisation");
            assert!((spec.pitch - 1.25).abs() < 1e-9 && (length - 6.0).abs() < 1e-9, "the pitch and the length must be intact");
            assert!((spec.fit - 0.15).abs() < 1e-9, "the fit clearance must be intact");
            assert!((lead_in - 1.0).abs() < 1e-9 && (lead_out - 1.0).abs() < 1e-9, "the lead-in and lead-out must be intact");
        }
        _ => panic!("after serialisation this must still be a thread feature"),
    }
}

/// A fresh document with an active part, so bodies and sketches land inside it: strict isolation forbids
/// bodies in an assembly.
fn part_project() -> Project {
    let mut p: Project = Default::default();
    p.new_document();
    p
}

#[test]
fn sketch_defaults_to_world_xy() {
    let mut p = part_project();
    let si = p.new_sketch("s");
    assert_eq!(p.sketches[si].plane, SketchPlane::World(BasePlane::XY), "a new sketch sits on the global XY plane");
    // The XY frame lifts (x,y) into (x,y,0).
    let f = p.sketch_frame(si).unwrap();
    let q = f.lift(Point2::new(3.0, 4.0));
    assert_eq!((q.x, q.y, q.z), (3.0, 4.0, 0.0));
}

#[test]
fn plane_frames_lift_correctly() {
    // XY: (x,y) -> (x,y,0)
    let q = BasePlane::XY.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (2.0, 5.0, 0.0));
    // XZ: (x,y) -> (x,0,y)
    let q = BasePlane::XZ.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (2.0, 0.0, 5.0));
    // YZ: (x,y) -> (0,x,y)
    let q = BasePlane::YZ.frame().lift(Point2::new(2.0, 5.0));
    assert_eq!((q.x, q.y, q.z), (0.0, 2.0, 5.0));
    // The normals are unit vectors along the expected axes.
    assert_eq!(BasePlane::XY.frame().normal(), [0.0, 0.0, 1.0]);
    assert_eq!(BasePlane::YZ.frame().normal(), [1.0, 0.0, 0.0]);
}

#[test]
fn sketch_plane_and_timeline_survive_serde() {
    let mut p = part_project();
    let si = p.new_sketch("on a face");
    // Place the sketch on a face of a body, which exercises serialisation of the nested `FaceKey`.
    p.sketches[si].plane = SketchPlane::Face(42, FaceKey { index: 3, centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], id: 0 });
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "on a face");

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();

    match back.sketches[si].plane {
        SketchPlane::Face(body, key) => {
            assert_eq!(body, 42);
            assert_eq!(key.index, 3);
            assert_eq!(key.centroid, [1.0, 2.0, 3.0]);
        }
        _ => panic!("the face plane was lost in serialisation"),
    }
    assert_eq!(back.timeline.len(), p.timeline.len(), "the timeline must survive serialisation");
}


#[test]
fn regenerate_builds_extrude_then_clean_noop() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();

    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 1, "the kernel must be called once");
    assert_eq!(rep.built.len(), 1);
    assert_eq!(rep.built[0].0, body);
    assert!(p.mesh_index(body).is_some(), "the body mesh must be created under its id");
    // The mesh marker is the height: the mock encodes the parameter into a vertex.
    assert_eq!(p.bodies[p.mesh_index(body).unwrap()].mesh.verts[0].x, 5.0);
    // The dirty flag was cleared, so a second regenerate builds nothing.
    let rep2 = p.regenerate(&k);
    assert_eq!(k.count(), 1, "a clean regenerate must not call the kernel");
    assert!(rep2.built.is_empty());
}

#[test]
fn editing_sketch_rebuilds_dependent_body() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);

    // Editing the sketch marks it dirty and the downstream body rebuilds.
    p.mark_sketch_dirty(sid);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 2, "after editing the sketch the kernel must be called again");
    assert_eq!(rep.built.len(), 1);
    assert_eq!(rep.built[0].0, body);
}

#[test]
fn a_feature_rebuilds_only_when_its_own_sketch_is_dirty() {
    // This test used to rest on a `FeatureKind::Boolean` variant that turned out to be a leftover: only this
    // test created it, no interface path and no saved document contained it, and the capability is covered
    // twice over by an extrude with an operation and by a body-to-body boolean. What the test checks does not
    // depend on the variant and is preserved: editing an unrelated sketch does not touch the feature.
    let mut p = part_project();
    let base = square(&mut p, "base");
    let other = square(&mut p, "other"); // An independent sketch.
    p.add_sketch_node(base, "base");
    p.add_sketch_node(other, "other");
    let body = p.add_extrude(base, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);

    // Editing an independent sketch does not touch the feature.
    p.mark_sketch_dirty(other);
    p.regenerate(&k);
    assert_eq!(k.count(), 1, "an unrelated sketch must not rebuild the feature");

    // Editing its own sketch rebuilds the feature.
    p.mark_sketch_dirty(base);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 2);
    assert_eq!(rep.built[0].0, body);
}

#[test]
fn regen_error_keeps_node_dirty_for_retry() {
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let kf = MockKernel { fail: true, ..Default::default() };
    let rep = p.regenerate(&kf);
    assert_eq!(rep.errors.len(), 1, "the kernel error must be recorded");
    assert!(rep.built.is_empty());
    // The node stays dirty, so the next successful pass rebuilds it.
    let ok = MockKernel::default();
    let rep2 = p.regenerate(&ok);
    assert_eq!(rep2.built.len(), 1);
    assert_eq!(rep2.built[0].0, body);
}

#[test]
fn regen_errors_track_failed_then_clear_on_success() {
    // A kernel failure marks the node in `regen_errors`, which the tree reddens with a hint, and a successful
    // rebuild clears the mark. That makes a node which fell through to the pass-through fallback visible as one
    // whose feature was not applied.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let node = p.timeline.iter().find(|n| n.kind.body() == Some(body)).unwrap().id;
    p.regenerate(&MockKernel { fail: true, ..Default::default() });
    assert!(p.regen_errors.contains_key(&node), "the failure must be recorded as a mark on the node");
    p.regenerate(&MockKernel::default());
    assert!(!p.regen_errors.contains_key(&node), "success must clear the mark");
}

#[test]
fn from_origin_normal_builds_orthonormal_frame() {
    use qymcad_core::feature::PlaneFrame;
    // Normal +Z with no rotation, so the axes coincide with world X and Y, plus the origin shift.
    let f = PlaneFrame::from_origin_normal([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0);
    let q = f.lift(Point2::new(2.0, 3.0)); // A 2D lift is `lift3` with z = 0.
    assert_eq!((q.x, q.y, q.z), (12.0, 3.0, 0.0));
    // The frame normal matches the one given.
    let n = f.normal();
    assert!((n[2] - 1.0).abs() < 1e-9);
}

#[test]
fn place_body_lifts_only_when_non_identity() {
    use qymcad_core::feature::{BasePlane, PlaneFrame};
    let mesh = Mesh { verts: vec![Point3::new(1.0, 2.0, 3.0)], tris: vec![] };
    // XY is the identity, so nothing changes.
    let xy = BasePlane::XY.frame();
    assert!(xy.is_identity());
    let (m0, _) = xy.place_body(mesh.clone(), vec![]);
    assert_eq!((m0.verts[0].x, m0.verts[0].y, m0.verts[0].z), (1.0, 2.0, 3.0));
    // A datum offset along +X moves the vertex by +10 along X.
    let d = PlaneFrame::from_origin_normal([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0);
    let (m1, _) = d.place_body(mesh, vec![]);
    assert_eq!((m1.verts[0].x, m1.verts[0].y, m1.verts[0].z), (11.0, 2.0, 3.0));
}

#[test]
fn sketch_frame_resolves_datum_and_face() {
    use qymcad_core::feature::SketchPlane;
    let mut p = part_project();
    let si = p.new_sketch("s");
    // Datum.
    let plane_id = p.add_plane(qymcad_core::model::WorkPlane { id: 0, name: "d".into(), origin: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: qymcad_core::model::PlaneDef::Manual });
    p.sketches[si].plane = SketchPlane::Datum(plane_id);
    let f = p.sketch_frame(si).expect("the datum frame");
    // The origin is the projection of the world origin onto the plane z = 3, that is (0,0,3), with XY matching
    // the world.
    assert_eq!(f.origin, [0.0, 0.0, 3.0]);
    // A face at y = 5 (centre (5,5,5), normal +Y): the origin is the projection of the part origin onto the
    // plane, that is (0,5,0), matching the behaviour of a world plane or a datum.
    p.sketches[si].plane = SketchPlane::Face(99, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 1.0, 0.0], id: 0 });
    let g = p.sketch_frame(si).expect("the face frame");
    assert_eq!(g.origin, [0.0, 5.0, 0.0]);
    assert!((g.normal()[1].abs() - 1.0).abs() < 1e-9, "the face normal must be plus or minus Y");
}

#[test]
fn extrude_on_datum_plane_is_placed_in_world() {
    use qymcad_core::feature::SketchPlane;
    let mut p = part_project();
    let sid = square(&mut p, "s");
    // A datum at z = 10 with normal +Z lifts the body to z = 10 while XY is preserved.
    let plane_id = p.add_plane(qymcad_core::model::WorkPlane { id: 0, name: "d".into(), origin: [0.0, 0.0, 10.0], normal: [0.0, 0.0, 1.0], rot_deg: 0.0, def: qymcad_core::model::PlaneDef::Manual });
    let si = p.sketch_index(sid).unwrap();
    p.sketches[si].plane = SketchPlane::Datum(plane_id);
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);

    let k = MockKernel::default();
    p.regenerate(&k);
    // The mock places a vertex at (5,0,0), and the datum frame at z = 10 lifts it to (5,0,10).
    let mi = p.mesh_index(body).unwrap();
    let v = p.bodies[mi].mesh.verts[0];
    assert_eq!((v.x, v.y, v.z), (5.0, 0.0, 10.0), "the body must be moved onto the sketch plane (z = 10, XY as in the world)");
}

#[test]
fn timeline_regenerates_after_save_load_roundtrip() {
    // The parametric chain survives serialisation: the sketch and the nodes are saved, and after loading
    // regenerate rebuilds the body from the sketch, the meshes being only a cache.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 7.0);

    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let mut back = qymcad_core::model::from_ron(&ron).unwrap();

    // The timeline and the sketch are in place.
    assert!(back.timeline.iter().any(|n| n.id == body));
    assert!(back.sketch_index(sid).is_some());
    // The body is rebuilt from the sketch after loading.
    let k = MockKernel::default();
    let rep = back.regenerate(&k);
    assert!(rep.built.iter().any(|(b, _)| *b == body), "the body must be rebuilt after loading");
    assert!(back.mesh_index(body).is_some());
}

#[test]
fn active_body_scoped_to_context_not_global() {
    // `active_body(ctx)` has to take the body of the current part rather than the globally last one; otherwise
    // a cut in part 1 attaches to the body of part 2 and part 2 appears to vanish.
    let mut p = part_project();
    let d1 = p.add_component("Part 1");
    p.set_active_component(Some(d1));
    let sid1 = square(&mut p, "s1");
    p.add_sketch_node(sid1, "s1");
    let b1 = p.add_extrude(sid1, 5.0);
    // The second part, whose body comes later in the timeline.
    let d2 = p.add_component("Part 2");
    p.set_active_component(Some(d2));
    let sid2 = square(&mut p, "s2");
    p.add_sketch_node(sid2, "s2");
    let b2 = p.add_extrude(sid2, 5.0);
    assert_eq!(p.active_body(d1), Some(b1), "part 1 must give its own body b1, not the globally last b2");
    assert_eq!(p.active_body(d2), Some(b2), "part 2 must give its own body b2");
}

#[test]
fn active_body_respects_rollback() {
    // Ignoring the rollback bar makes a rolled-back body (which has no mesh) count as active, so a cut or a
    // shell targets a body that does not exist.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let b1 = p.add_extrude(sid, 5.0);
    let b2 = p.add_extrude(sid, 7.0);
    assert_eq!(p.active_body(d), Some(b2), "without a rollback bar the last body is active");
    // Rolling back to before node b2 leaves b2 unbuilt, so the previous body b1 is active.
    let idx = p.timeline_index(b2).unwrap();
    p.set_rollback(Some(idx));
    assert_eq!(p.active_body(d), Some(b1), "with the rollback bar b2 is rolled back, so b1 is active");
}

#[test]
fn delete_plane_cascades_sketch_on_datum() {
    // Deleting a datum plane used to leave a sketch hanging on it: without a frame the feature failed
    // silently. The sketch and its body now go in a cascade.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let pl = p.add_offset_plane(BasePlane::XY, 20.0);
    let si = p.new_sketch("s");
    p.sketches[si].plane = SketchPlane::Datum(pl);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    assert!(p.delete_plane(pl));
    assert!(p.sketch_index(sid).is_none(), "a sketch on a deleted plane must be removed in the cascade");
    assert!(p.timeline_index(body).is_none(), "the body of the sketch must be removed in the cascade");
    assert!(!p.planes.iter().any(|x| x.id == pl), "the plane must be deleted");
}

#[test]
fn delete_datum_axis_degrades_circular_array() {
    // Deleting an axis used to degrade the pattern inside regenerate without marking it dirty, leaving stale
    // geometry.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let ax = p.add_axis_manual([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let arr = p.add_circular_array_axis(body, 4, 360.0, ax);
    let axis_of = |p: &Project, id: Id| p.timeline.iter().find(|n| n.id == id).and_then(|n| match n.kind { FeatureKind::CircularArray { axis, .. } => Some(axis), _ => None });
    assert_eq!(axis_of(&p, arr), Some(ax), "the pattern must reference the axis");
    assert!(p.delete_datum_axis(ax));
    assert_eq!(axis_of(&p, arr), Some(0), "the pattern must degrade to world Z (axis 0)");
    assert!(p.timeline.iter().find(|n| n.id == arr).unwrap().dirty, "the consumer must be marked dirty so it rebuilds");
}

#[test]
fn reorder_respects_datum_dependency() {
    // Checking only body dependencies lets an offset plane be moved above its source, so it resolves against
    // stale data. Datum dependencies gate the reorder as well.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let base = p.add_offset_plane(BasePlane::XY, 10.0);
    let child = p.add_offset_from_plane(base, 5.0);
    let bi = p.timeline_index(base).unwrap();
    let ci = p.timeline_index(child).unwrap();
    assert!(ci > bi, "the child plane must be created after its source");
    assert!(!p.can_reorder_feature(ci, bi), "an offset plane must not be movable above its source");
}

#[test]
fn cut_overshoots_sketch_plane_to_avoid_coincident_cap() {
    // A cut from a sketch on a face used to leave a cap at the entry, where the tool end face coincided with
    // the body face, so the hole was not a through one. An end face lying exactly on the sketch plane is now
    // pushed outwards by a small overshoot.
    let mut p = part_project();
    let d = p.add_component("Part");
    p.set_active_component(Some(d));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // A cut from the same sketch: op 0, one-sided, no flip, so the end face sits on the sketch plane at
    // z = 0.
    let cut = p.add_combine_on(base, sid, 0, 5.0, 0, qymcad_core::feature::Extent::default(), 0.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let mi = p.mesh_index(cut).expect("the cut body must be built");
    let v = p.bodies[mi].mesh.verts[0]; // In the mock, x is the tool height (total) and z is the start offset
                                        // along +Z.
    assert!(v.x > 5.0, "the tool height must be increased by the overshoot (it was 5): {}", v.x);
    assert!(v.z < 0.0, "the tool start must be pushed outwards past the sketch plane: {}", v.z);
    // Control: a boss (op 1) has no overshoot and grows exactly from the plane.
    let joinb = p.add_combine_on(base, sid, 0, 5.0, 1, qymcad_core::feature::Extent::default(), 0.0);
    p.regenerate(&k);
    let vj = p.bodies[p.mesh_index(joinb).unwrap()].mesh.verts[0];
    assert!((vj.z).abs() < 1e-9 && (vj.x - 5.0).abs() < 1e-9, "a boss must have no overshoot (start 0, total 5): x={} z={}", vj.x, vj.z);
}

#[test]
fn active_component_nests_new_nodes() {
    let mut p = part_project();
    // Create a component and make it active.
    let comp = p.add_component("Part 1");
    p.set_active_component(Some(comp));
    // New nodes land in the active component.
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    assert_eq!(p.node_component(sid), Some(comp), "the sketch must nest inside the active component");
    assert_eq!(p.node_component(body), Some(comp), "the body must nest inside the active component");
    // Outside an active component nodes land in the root assembly, an explicit root rather than `None`.
    p.set_active_component(None);
    let sid2 = square(&mut p, "s2");
    p.add_sketch_node(sid2, "s2");
    assert_eq!(p.node_component(sid2), Some(p.root), "outside a component nodes land in the root assembly");
}

#[test]
fn components_survive_serde() {
    let mut p: Project = Default::default();
    let comp = p.add_component("Node");
    p.set_active_component(Some(comp));
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.components.len(), 2, "the root assembly plus the created part");
    assert_eq!(back.active_component, Some(comp));
    assert_eq!(back.node_component(sid), Some(comp));
}

#[test]
fn primitives_build_via_kernel() {
    let mut p = part_project();
    let bx = p.add_box(10.0, 20.0, 5.0);
    let cy = p.add_cylinder(4.0, 8.0);
    let sp = p.add_sphere(3.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    // Three primitives were built by the mock kernel and their bodies created.
    assert!(p.mesh_index(bx).is_some(), "box");
    assert!(p.mesh_index(cy).is_some(), "cylinder");
    assert!(p.mesh_index(sp).is_some(), "sphere");
    assert_eq!(k.count(), 3);
    // Primitives have no sketch inputs.
    let bn = p.timeline.iter().find(|n| n.id == bx).unwrap();
    assert!(bn.kind.inputs().is_empty());
    assert_eq!(bn.kind.body(), Some(bx));
}

#[test]
fn combine_cut_needs_built_src_then_rebuilds() {
    let mut p = part_project();
    // The base body: a box.
    let base = p.add_box(20.0, 20.0, 10.0);
    // The tool sketch of the cut.
    let cut = square(&mut p, "cut");
    p.add_sketch_node(cut, "cut");
    let body = p.add_combine(base, cut, 10.0, 0); // A cut.
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    // The source box was built first, so the combine found its shape and produced a body.
    assert!(rep.built.iter().any(|(b, _)| *b == body), "the cut must be built");
    assert!(p.mesh_index(body).is_some());
    // It depends on both the source body and the sketch.
    let n = p.timeline.iter().find(|n| n.id == body).unwrap();
    assert_eq!(n.kind.inputs(), vec![base, cut]);
    assert_eq!(n.kind.consumed_body(), Some(base), "the box must be consumed by the cut and hidden");
}

#[test]
fn fillet_chamfer_consume_source_body() {
    let mut p = part_project();
    let cyl = p.add_cylinder(10.0, 20.0);
    let fil = p.add_fillet(cyl, 2.0, vec![]);
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.built.iter().any(|(b, _)| *b == fil), "the fillet must be built");
    let n = p.timeline.iter().find(|n| n.id == fil).unwrap();
    assert_eq!(n.kind.inputs(), vec![cyl]);
    assert_eq!(n.kind.consumed_body(), Some(cyl));
    // Editing the radius of the source cylinder rebuilds the fillet downstream.
    let before = k.count();
    p.mark_node_dirty(cyl);
    p.regenerate(&k);
    assert!(k.count() > before, "the source changed, so the fillet must be rebuilt");
}

#[test]
fn chamfer_modes_route_and_are_parametric() {
    use qymcad_core::feature::ChamferMode;
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "k".into(), expr: "3".into(), value: 3.0 });

    // A symmetric chamfer over every edge calls the plain `chamfer` rather than `chamfer_ex`.
    let cyl = p.add_cylinder(10.0, 20.0);
    let sym = p.add_chamfer(cyl, 1.5, vec![]);
    // Two setbacks over selected edges call `chamfer_ex`; d1 and d2 are parametric and `flip` reaches the
    // kernel.
    let two = p.add_chamfer_ex(sym, 2.0, 1.0, ChamferMode::TwoDist, true, 0, vec![7]);
    p.set_feat_dim(two, "dist", "k".into()); // d1 = k = 3
    p.set_feat_dim(two, "d2", "k/2".into()); // d2 = 1.5
    // Setback plus angle calls `chamfer_ex` in `DistAngle` mode with a manually chosen reference face, so
    // `ref_face` reaches the kernel.
    let da = p.add_chamfer_ex(two, 2.5, 30.0, ChamferMode::DistAngle, false, 42, vec![9]);

    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c == "chamfer d=1.5 n=0"), "a symmetric chamfer must call `chamfer`: {calls:?}");
    assert!(calls.iter().any(|c| c == "chamfer_ex d1=3 d2=1.5 mode=TwoDist flip=true rf=0 n=1"), "two setbacks must be parametric: {calls:?}");
    assert!(calls.iter().any(|c| c == "chamfer_ex d1=2.5 d2=30 mode=DistAngle flip=false rf=42 n=1"), "setback plus angle with a manual reference face: {calls:?}");
    drop(calls);

    // Changing parameter k rebuilds the two-setback chamfer with new d1 and d2.
    p.parameters[0].value = 4.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c == "chamfer_ex d1=4 d2=2 mode=TwoDist flip=true rf=0 n=1"), "after setting k = 4: {:?}", k2.calls.borrow());
    let _ = da;
}

#[test]
fn combine_fails_if_src_missing() {
    // A combine against a non-existent source body errors and the node stays dirty.
    let mut p = part_project();
    let cut = square(&mut p, "cut");
    p.add_sketch_node(cut, "cut");
    let body = p.add_combine(999, cut, 5.0, 0); // Source 999 does not exist.
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.iter().any(|(n, _)| *n == body), "an error must be reported: no source");
    assert!(rep.built.iter().all(|(b, _)| *b != body));
}

#[test]
fn cone_torus_prism_build() {
    let mut p = part_project();
    let cn = p.add_cone(10.0, 0.0, 20.0);
    let tr = p.add_torus(12.0, 4.0);
    let pr = p.add_prism(10.0, 6, 20.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    for b in [cn, tr, pr] {
        assert!(p.mesh_index(b).is_some(), "the primitive must be built");
        let n = p.timeline.iter().find(|n| n.id == b).unwrap();
        assert!(n.kind.inputs().is_empty(), "a primitive has no inputs");
        assert_eq!(n.kind.body(), Some(b));
    }
    // A cone and a torus are revolved, a prism is extruded.
    assert_eq!(k.count(), 3);
}

#[test]
fn shell_and_arrays_build_and_consume_source() {
    let mut p = part_project();
    let cube = p.add_box(20.0, 20.0, 20.0);
    let sh = p.add_shell(cube, 2.0, vec![1], false);
    let la = p.add_linear_array(sh, 25.0, 0.0, 0.0, 3);
    let ca = p.add_circular_array(la, 6, 360.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    for b in [sh, la, ca] {
        assert!(p.mesh_index(b).is_some());
    }
    // Every modifier consumes its own source body.
    let node = |id: u64| p.timeline.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node(sh).kind.consumed_body(), Some(cube));
    assert_eq!(node(la).kind.consumed_body(), Some(sh));
    assert_eq!(node(ca).kind.consumed_body(), Some(la));
    // A circular pattern of 6 copies calls the kernel with 6 transforms.
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")));
    // Editing the box rebuilds the whole chain below it.
    let before = k.count();
    p.mark_node_dirty(cube);
    p.regenerate(&k);
    assert!(k.count() >= before + 4, "box, shell, linear pattern and circular pattern must all rebuild");
}

#[test]
fn mirror_and_hole_build_consume_source() {
    let mut p = part_project();
    let cube = p.add_box(20.0, 20.0, 20.0);
    let hole = p.add_hole(cube, FaceKey { index: 0, centroid: [10.0, 10.0, 20.0], normal: [0.0, 0.0, 1.0], id: 0 }, 6.0, 15.0);
    let mir = p.add_mirror(hole, 1, true, 0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(hole).is_some() && p.mesh_index(mir).is_some());
    // A hole goes through `kernel.hole` (a cutting tool) and a mirror through `mirror`.
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("hole k=0")), "a hole must be a cut");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("mirror")), "the mirror must be called");
    let node = |id: u64| p.timeline.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node(hole).kind.consumed_body(), Some(cube));
    assert_eq!(node(mir).kind.consumed_body(), Some(hole));
}

#[test]
fn extrude_picks_chosen_contour() {
    // A sketch with two closed contours: a specific one is extruded (`profile = cid`) rather than the
    // first.
    let mut p = part_project();
    let sid = p.add_line_sketch("two shapes", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let si = p.sketch_index(sid).unwrap();
    // The second square contour is added as entities followed by a regenerate.
    p.add_rect_entity(si, 20.0, 0.0, 30.0, 8.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let closed: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_profile_xy(*cid).is_some()).collect();
    assert!(closed.len() >= 2, "there must be two closed contours: {}", closed.len());
    let chosen = closed[1];
    let body = p.add_extrude_on(sid, chosen, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // The feature holds the selected profile.
    let prof = p.timeline.iter().find_map(|n| if let qymcad_core::feature::FeatureKind::Extrude { profiles, .. } = &n.kind { profiles.first().copied() } else { None }).unwrap();
    assert_eq!(prof, chosen, "the selected contour must be the one extruded");
    // The feature profile is the XY of the second contour rather than the first.
    let xy = p.feature_profile_xy(sid, chosen).unwrap();
    let xs: Vec<f64> = xy.iter().step_by(2).copied().collect();
    assert!(xs.iter().cloned().fold(0.0_f64, f64::max) >= 20.0, "the second contour must be taken (x up to 30): {xs:?}");
    let _ = body;
}

#[test]
fn extrude_two_sided_and_combine_through_stored() {
    // The extent parameters reach the features: `down` for two sides and `through` for through all.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let sid = p.add_line_sketch("rectangle", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 3.0); // Second side of 3.
    let down = p.timeline.iter().find_map(|n| if let FeatureKind::Extrude { down, .. } = n.kind { Some(down) } else { None }).unwrap();
    assert_eq!(down, 3.0, "the second side must be stored in the feature");
    let cut = p.add_combine_on(base, sid, 0, 5.0, 0, qymcad_core::feature::Extent { through: true, ..Default::default() }, 0.0); // A through cut.
    let through = p.timeline.iter().find_map(|n| if let FeatureKind::Combine { extent, .. } = n.kind { Some(extent.through) } else { None }).unwrap();
    assert!(through, "through all must be stored in the cut feature");
    let _ = cut;
}

#[test]
fn feature_dim_expression_is_parametric() {
    // A feature dimension can be an expression over a global parameter and rebuilds when that parameter
    // changes.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "h".into(), expr: "20".into(), value: 20.0 });
    let sid = p.add_line_sketch("square", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0); // The plain number 5.
    p.set_feat_dim(body, "height", "h".into()); // height = h
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.contains("h=20")), "height=h → 20: {:?}", k.calls.borrow());
    // Changing the parameter dirties the dependent features and they rebuild with the new value.
    p.parameters[0].value = 30.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c.contains("h=30")), "after the parameter change it must be 30: {:?}", k2.calls.borrow());
}

#[test]
fn delete_sketch_cascades_dependent_bodies() {
    // Deleting a sketch removes the bodies built on it together with their timeline nodes.
    let mut p = part_project();
    let sid = p.add_line_sketch("square", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(body, 1.0, Vec::<u32>::new()); // Depends on the body, so it cascades too.
    let before = p.timeline.len();
    assert!(before >= 3, "the nodes are a sketch, an extrude and a fillet");
    let removed = p.delete_sketch(sid);
    assert!(removed.contains(&body) && removed.contains(&fil), "both bodies must be removed by the cascade: {removed:?}");
    assert!(p.sketch_index(sid).is_none(), "the sketch must be deleted");
    assert!(p.timeline.iter().all(|n| n.kind.body() != Some(body) && n.kind.body() != Some(fil)), "the body nodes must be deleted");
    assert!(p.mesh_index(body).is_none() && p.mesh_index(fil).is_none(), "the meshes must be deleted");
    // The rebuild caches go too. Dropping the mesh while leaving `regen_faces` behind leaves a body that no
    // timeline node produces counted as alive: measured on a loft, a two-node timeline still reported the body
    // as live with its previous three faces. Everything that enumerates live bodies counts such a ghost, from
    // the document tree to the machine output.
    assert!(
        !p.regen_faces.contains_key(&body) && !p.regen_faces.contains_key(&fil),
        "the faces of deleted bodies must leave the rebuild cache: {:?}",
        p.regen_faces.keys().collect::<Vec<_>>()
    );
    assert!(!p.regen_edges.contains_key(&body) && !p.regen_edges.contains_key(&fil), "the edges of deleted bodies must leave the cache");
}

#[test]
fn face_ref_resolves_by_persistent_id() {
    // A face reference holds on to the persistent id, so after a rebuild the sketch travels with the face even
    // when the key still carries a stale centre.
    let mut p = part_project();
    let si = p.new_sketch("s");
    let body = 777u64;
    p.regen_faces.insert(
        body,
        vec![
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(1.0, 1.0, 5.0), area: 1.0, id: 10 },
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(9.0, 9.0, 5.0), area: 1.0, id: 20 },
        ],
    );
    // A sketch on face id 20, whose key holds a stale centre from before the rebuild.
    let key = FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 20 };
    // Resolving by the persistent id 20 gives the current face centre (9,9,5) rather than the stale key.
    assert_eq!(p.resolve_face(body, &key).0, [9.0, 9.0, 5.0], "the face must resolve by id 20 to its current centre");
    p.sketches[si].plane = SketchPlane::Face(body, key);
    // The 2D zero of a sketch on a face is the projection of the part origin onto the face plane at z = 5,
    // giving [0,0,5].
    let f = p.sketch_frame(si).unwrap();
    assert_eq!(f.origin, [0.0, 0.0, 5.0], "the origin of a sketch on a face is the projection of the part zero onto the plane");
}

#[test]
fn move_feature_transforms_brep() {
    // Moving a body is a parametric feature: it moves the B-rep through the kernel.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let _moved = p.add_move(body, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "move"), "`transform_body` must be called: {:?}", k.calls.borrow());
}

#[test]
fn rollback_suppresses_later_features() {
    // The rollback bar suppresses the last nodes, so their bodies are neither built nor shown.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(body).is_some(), "the body must be built before the rollback");
    let n = p.timeline.len();
    p.set_rollback(Some(n - 1)); // Do not build the last node, the extrude.
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(body).is_none(), "after the rollback the body must be suppressed");
}

#[test]
fn mirror_by_datum_plane_uses_plane_origin_normal() {
    // A mirror about a datum plane calls `mirror_plane` with the origin and normal of that plane rather than
    // of a world one.
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let pl = p.add_plane(WorkPlane { id: 0, name: "d".into(), origin: [10.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], rot_deg: 0.0, def: PlaneDef::Manual });
    let mir = p.add_mirror(base, 0, false, 0);
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == mir) {
        if let qymcad_core::feature::FeatureKind::Mirror { datum, .. } = &mut n.kind {
            *datum = pl;
        }
        n.dirty = true;
    }
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("mirror_plane") && c.contains("[10.0, 0.0, 0.0]")), "the mirror must use the origin of the datum plane: {:?}", k.calls.borrow());
    assert!(p.mesh_index(mir).is_some(), "the mirrored body must be built");
}

#[test]
fn linear_array_two_directions_grid() {
    // A linear pattern with two directions gives a grid of count by count2 copies.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array(base, 25.0, 0.0, 0.0, 3); // Three along X; by default there is no second direction.
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == la) {
        if let qymcad_core::feature::FeatureKind::LinearArray { dy2, count2, .. } = &mut n.kind {
            *dy2 = 25.0;
            *count2 = 2; // Plus two along Y, giving a 3 by 2 grid of 6.
        }
        n.dirty = true;
    }
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")), "a 3 by 2 grid must give 6 copies: {:?}", k.calls.borrow());
    assert!(p.mesh_index(la).is_some(), "the pattern body must be built");
}

#[test]
fn linear_array_grid_helper_builds_two_directions() {
    // `add_linear_array_grid` is the single core method the interface calls: a count by count2 grid in one
    // call.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array_grid(base, 20.0, 0.0, 0.0, 3, 0.0, 15.0, 0.0, 2); // 3×2 = 6
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=6")), "3×2=6: {:?}", k.calls.borrow());
    assert!(p.mesh_index(la).is_some());
}

#[test]
fn linear_array_grid3_builds_full_3d_grid() {
    // Three independent directions along X, Y and Z give a full 3D grid of count by count2 by count3. Both the
    // number of copies and the position of the far instance at the corner of the box (i, j, k at their maxima)
    // are checked.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array_grid3(base, 10.0, 0.0, 0.0, 2, 0.0, 20.0, 0.0, 3, 0.0, 0.0, 30.0, 2); // 2×3×2 = 12
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    let pat = calls.iter().find(|c| c.starts_with("pattern n=12")).expect("2 by 3 by 2 gives 12 copies").clone();
    // The far corner: i = 1 (10 along X), j = 2 (40 along Y), k = 1 (30 along Z).
    assert!(pat.contains("(10.0,40.0,30.0)"), "corner of the 3D grid: {pat}");
    drop(calls);
    assert!(p.mesh_index(la).is_some());
}

#[test]
fn linear_array_step_is_parametric_via_feat_dim() {
    // The step lives as an expression on a vector component (`dx`), so a global parameter moves the pattern:
    // regenerate reads `feat_dims` rather than the stored number. The actual instance translations are
    // checked.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "gap".into(), expr: "30".into(), value: 30.0 });
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let la = p.add_linear_array(base, 10.0, 0.0, 0.0, 3); // The stored dx is 10...
    p.set_feat_dim(la, "dx", "gap".into()); // ...but the expression dx = gap = 30 overrides it.
    let k = MockKernel::default();
    p.regenerate(&k);
    // The instances land at 0, 30 and 60 along X rather than 0, 10 and 20: parametric.
    let calls = k.calls.borrow();
    let pat = calls.iter().find(|c| c.starts_with("pattern n=3")).expect("the pattern must be built").clone();
    drop(calls);
    assert!(pat.contains("(0.0,0.0,0.0)") && pat.contains("(30.0,0.0,0.0)") && pat.contains("(60.0,0.0,0.0)"), "the step must be parametric (gap = 30): {pat}");
    // Changing the parameter moves the pattern without editing the feature.
    p.parameters[0].value = 50.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    let calls2 = k2.calls.borrow();
    let pat2 = calls2.iter().find(|c| c.starts_with("pattern n=3")).expect("it must be rebuilt");
    assert!(pat2.contains("(100.0,0.0,0.0)"), "with gap = 50 the far instance must sit at 100: {pat2}");
}

#[test]
fn circular_array_angle_is_parametric_via_feat_dim() {
    // The angle of a circular pattern is an expression in `feat_dims`. A rotation does not change the
    // translation, so what is checked is that regenerate reads the parameter (building the right number of
    // copies) and does not fail when the global parameter changes.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "ang".into(), expr: "180".into(), value: 180.0 });
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ca = p.add_circular_array_axis(base, 4, 90.0, 0); // The stored angle is 90...
    p.set_feat_dim(ca, "angle", "ang".into()); // ...and the expression ang = 180 overrides it.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("pattern n=4")), "four copies from the angle parameter: {:?}", k.calls.borrow());
    assert!(p.mesh_index(ca).is_some());
    p.parameters[0].value = 270.0;
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(ca).is_some(), "it must rebuild when the angle parameter changes");
}

#[test]
fn delete_datums_remove_from_vec_and_timeline() {
    // Datum deletions (plane, point, axis) are single project methods: they remove the entry from its vector
    // and the timeline node together.
    use qymcad_core::model::{DatumAxis, DatumPoint, WorkPlane};
    let mut p = part_project();
    let pl = p.add_plane(WorkPlane::default());
    assert!(p.planes.iter().any(|x| x.id == pl) && p.timeline.iter().any(|n| n.id == pl), "the plane must exist in both the vector and the timeline");
    assert!(p.delete_plane(pl));
    assert!(!p.planes.iter().any(|x| x.id == pl) && !p.timeline.iter().any(|n| n.id == pl), "the plane must be removed from both the vector and the timeline");
    let dp = p.add_datum_point(DatumPoint::default());
    assert!(p.delete_datum_point(dp));
    assert!(!p.datum_points.iter().any(|x| x.id == dp) && !p.timeline.iter().any(|n| n.id == dp), "the point must be removed from both the vector and the timeline");
    let ax = p.add_datum_axis(DatumAxis::default());
    assert!(p.delete_datum_axis(ax));
    assert!(!p.datum_axes.iter().any(|x| x.id == ax) && !p.timeline.iter().any(|n| n.id == ax), "the axis must be removed from both the vector and the timeline");
    // Deleting again returns false: there is no such entry.
    assert!(!p.delete_plane(pl) && !p.delete_datum_point(dp) && !p.delete_datum_axis(ax));
}

#[test]
fn delete_base_cascades_to_move() {
    // Deleting a base body (a primitive) cascades into the move that consumes it, leaving no ghosts.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 50.0; // Offset along X.
    let mv = p.add_move(base, mat);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(base).is_some() && p.mesh_index(mv).is_some(), "both bodies must be built");
    let removed = p.delete_body_cascade(base);
    assert!(removed.contains(&base) && removed.contains(&mv), "the cascade must remove both the base and the move: {removed:?}");
    assert!(p.timeline.iter().all(|n| n.kind.body() != Some(mv)), "the move node must be removed by the cascade");
    assert!(p.mesh_index(mv).is_none() && p.mesh_index(base).is_none(), "both meshes must be deleted");
}

#[test]
fn prune_removes_dangling_move_and_orphan_mesh() {
    // Simulating an older defect: the base node is deleted directly, without a cascade, leaving an orphaned
    // mesh and a dangling move.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let mut mat = qymcad_core::feature::PLACE_IDENTITY;
    mat[3] = 50.0;
    let mv = p.add_move(base, mat);
    let k = MockKernel::default();
    p.regenerate(&k);
    p.timeline.retain(|n| n.kind.body() != Some(base)); // Remove only the base node; the mesh is left orphaned.
    assert!(p.mesh_index(base).is_some(), "the base mesh must be left orphaned");
    let removed = p.prune_dangling();
    assert!(removed.contains(&base) && removed.contains(&mv), "pruning must remove the orphaned base and the dangling move: {removed:?}");
    assert!(p.mesh_index(base).is_none() && p.mesh_index(mv).is_none(), "the ghost meshes must be deleted");
    assert!(p.timeline.iter().all(|n| n.kind.body() != Some(mv)), "pruning must remove the move node");
}

#[test]
fn prune_keeps_imported_body() {
    // An imported body (no node, but listed in `imported_bodies`) is not an orphan: pruning leaves it and the
    // feature built on it alone.
    let mut p = part_project();
    let imp = p.add_mesh(qymcad_core::geom::Mesh::default());
    p.imported_bodies.insert(imp);
    let _fil = p.add_fillet(imp, 1.0, Vec::<u32>::new());
    let removed = p.prune_dangling();
    assert!(removed.is_empty(), "the import and the feature on it must be left alone: {removed:?}");
    assert!(p.mesh_index(imp).is_some(), "the imported mesh must stay");
}

#[test]
fn body_boolean_builds_and_consumes_both_operands() {
    // A body-to-body boolean builds the result and consumes both operands, hiding a and b from view.
    let mut p = part_project();
    let s1 = square(&mut p, "square 1");
    p.add_sketch_node(s1, "square 1");
    let a = p.add_extrude_on(s1, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let s2 = square(&mut p, "square 2");
    p.add_sketch_node(s2, "square 2");
    let b = p.add_extrude_on(s2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let res = p.add_body_boolean(a, b, 0); // The cut a minus b.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(a).is_some() && p.mesh_index(b).is_some() && p.mesh_index(res).is_some(), "both operands and the result must be built by the kernel");
    assert!(k.calls.borrow().iter().any(|c| c == "body_boolean"), "the kernel must call `body_boolean`: {:?}", k.calls.borrow());
    // Both operands are consumed and only the result stays visible: `consumed()` of the boolean node is
    // [a, b].
    let node = p.timeline.iter().find(|n| n.id == res).unwrap();
    let consumed = node.kind.consumed();
    assert!(consumed.contains(&a) && consumed.contains(&b), "`consumed()` of the boolean must be [a, b]: {consumed:?}");
}

#[test]
fn body_boolean_survives_serde() {
    // A body boolean node survives the RON round trip.
    let mut p = part_project();
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let a = p.add_extrude_on(s1, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b = p.add_extrude_on(s2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let res = p.add_body_boolean(a, b, 2); // Intersection.
    let ron = qymcad_core::model::to_ron(&p).expect("serialisation");
    let p2 = qymcad_core::model::from_ron(&ron).expect("deserialisation");
    let node = p2.timeline.iter().find(|n| n.id == res).expect("the boolean node must exist after loading");
    assert!(matches!(node.kind, qymcad_core::feature::FeatureKind::BodyBoolean { op: 2, .. }), "the intersection operation must be preserved");
}

#[test]
fn suppress_feature_cascades_to_dependents() {
    // Suppressing a single feature leaves its body unbuilt and cascades to its consumers, such as a cut on
    // that body.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let cut = p.add_combine_on(base, sid, 0, 4.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, 0.0); // Depends on the base.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(base).is_some() && p.mesh_index(cut).is_some(), "both bodies must be built");
    // Suppressing the extrude must, by cascade, leave the cut unbuilt as well.
    let ti = p.timeline_index(base).unwrap();
    assert!(p.set_feature_suppressed(ti, true), "suppression must be switched on");
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(base).is_none(), "a suppressed feature must not be built");
    assert!(p.mesh_index(cut).is_none(), "a consumer of a suppressed feature must not be built either");
    // Switching it back on restores both bodies.
    p.set_feature_suppressed(ti, false);
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(p.mesh_index(base).is_some(), "after switching back on the base must be built");
    assert!(p.mesh_index(cut).is_some(), "after switching back on the consumer must be restored by the dirty cascade");
}

#[test]
fn suppress_modifier_passes_through_keeping_chain() {
    // Suppressing a modifier in the middle of a chain is a pass-through: its effect is removed while its
    // output is a copy of its source, so the consumers below it keep building. The chain does not collapse, as
    // it did with a cascade that switched off everything below a suppressed middle feature.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let ext = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(ext, 1.0, Vec::<u32>::new());
    let cha = p.add_chamfer(fil, 0.5, Vec::<u32>::new());
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(p.mesh_index(ext).is_some() && p.mesh_index(fil).is_some() && p.mesh_index(cha).is_some(), "all of them must be built");
    let ti = p.timeline_index(fil).unwrap();
    p.set_feature_suppressed(ti, true); // Suppress the middle modifier, the fillet.
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(p.mesh_index(fil).is_some(), "a suppressed modifier is a pass-through whose output is a copy of its source");
    assert!(p.mesh_index(cha).is_some(), "the consumer must keep building: the chain does not collapse");
    // Switching it back on makes the fillet real again and the chain stays intact.
    p.set_feature_suppressed(ti, false);
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(p.mesh_index(cha).is_some(), "after switching back on the chain must be intact");
}

#[test]
fn suppressed_flag_survives_serde() {
    // The suppressed flag of a timeline node survives saving and loading.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ti = p.timeline_index(body).unwrap();
    p.set_feature_suppressed(ti, true);
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert!(back.timeline.iter().find(|n| n.id == body).unwrap().suppressed, "the suppressed flag must survive serialisation");
}

#[test]
fn body_color_stable_across_lineage() {
    // A colour is keyed by the lineage root of a part, so it does not change with operations; different parts
    // get different colours; a manual colour paints the whole chain and clearing it returns the palette
    // entry.
    use qymcad_core::model::default_part_color;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let ext = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(ext, 1.0, Vec::<u32>::new()); // Consumes the extrude: the same part.
    let k = MockKernel::default();
    p.regenerate(&k);
    let (ie, iff) = (p.mesh_index(ext).unwrap(), p.mesh_index(fil).unwrap());
    assert_eq!(p.mesh_color(iff), default_part_color(0), "the colour comes from the lineage root (part 0)");
    assert_eq!(p.mesh_color(ie), p.mesh_color(iff), "the colour must not jump with an operation on the same part");
    // A second part gets a different, stable colour.
    let sid2 = p.add_line_sketch("square 2", vec![Point2::new(20.0, 0.0), Point2::new(30.0, 0.0), Point2::new(30.0, 10.0), Point2::new(20.0, 10.0)], true);
    p.add_sketch_node(sid2, "square 2");
    let ext2 = p.add_extrude_on(sid2, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    p.regenerate(&k);
    let ie2 = p.mesh_index(ext2).unwrap();
    assert_eq!(p.mesh_color(ie2), default_part_color(1), "the second part must take colour 1");
    assert_ne!(p.mesh_color(ie2), p.mesh_color(iff), "different parts must get different colours");
    // A manual colour on the filleted body paints the whole part through its root; clearing it returns the
    // palette.
    p.set_mesh_color(iff, [10, 20, 30]);
    assert_eq!(p.mesh_color(iff), [10, 20, 30], "the manual colour must be applied");
    assert_eq!(p.mesh_color(ie), [10, 20, 30], "the manual colour must cover the whole chain sharing that root");
    p.reset_mesh_color(ie);
    assert_eq!(p.mesh_color(iff), default_part_color(0), "clearing it must return the palette entry for the root");
}

#[test]
fn reorder_respects_body_dependencies() {
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let fil = p.add_fillet(body, 1.0, Vec::<u32>::new());
    let ei = p.timeline_index(body).unwrap();
    let fi = p.timeline_index(fil).unwrap();
    assert!(!p.reorder_feature(fi, ei), "moving a chamfer above the extrude that produces its input body must be refused");
}

// --- Component isolation ---

#[test]
fn body_tool_in_assembly_lands_in_a_part() {
    // Invariant: a body never belongs to an assembly, so `body_parent` creates a part.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root)); // Active in the root assembly.
    let body = p.add_box(10.0, 10.0, 10.0);
    let owner = p.body_owner(body).expect("the body has an owner");
    assert!(p.component_is_part(owner), "the body must land in a part rather than in an assembly");
    assert_ne!(owner, root, "the owner must not be the root assembly");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "the build must produce no isolation errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the body must be built");
}

#[test]
fn cross_component_sketch_on_face_is_blocked() {
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    // Part A with a body.
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    let body_a = p.add_extrude(sa, 5.0);
    // Part B, whose sketch sits on a face of the body of A: a cross-component reference.
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_active_component(Some(b));
    let sb = square(&mut p, "sb");
    let si = p.sketch_index(sb).unwrap();
    p.sketches[si].plane = SketchPlane::Face(body_a, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 });
    p.add_sketch_node(sb, "sb");
    let body_b = p.add_extrude(sb, 5.0);

    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(p.mesh_index(body_a).is_some(), "the body of its own part A must be built");
    assert!(rep.errors.iter().any(|(id, _)| *id == body_b), "the cross-component reference of body B must be blocked: {:?}", rep.errors);
    assert!(p.mesh_index(body_b).is_none(), "body B must not be built");
    assert_eq!(k.count(), 1, "the kernel must be called only for the valid body A");
}

#[test]
fn external_ref_unblocks_cross_component_face_sketch() {
    // Controlled top-down design: a sketch of part B sits on a face of the body of part A. Without an external
    // reference isolation blocks it (see `cross_component_sketch_on_face_is_blocked`). With an explicit
    // `ExternalRef` the cross-component reference is authorised and the source face is carried into the local
    // space of the consumer through `world_transform`.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    // Part A with a body, providing the source face.
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    let body_a = p.add_extrude(sa, 5.0);
    // Part B with a sketch on a face of the body of A: a cross-component reference.
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_active_component(Some(b));
    let sb = square(&mut p, "sb");
    let si = p.sketch_index(sb).unwrap();
    let key = FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    p.sketches[si].plane = SketchPlane::Face(body_a, key);
    p.add_sketch_node(sb, "sb");
    let body_b = p.add_extrude(sb, 5.0);

    // Without an external reference it is blocked and the frame of the foreign face does not resolve.
    let k = MockKernel { no_faces: true, ..Default::default() };
    let rep = p.regenerate(&k);
    assert!(rep.errors.iter().any(|(id, _)| *id == body_b), "without an external reference body B must be blocked");
    assert!(p.mesh_index(body_b).is_none(), "without an external reference body B must not be built");
    assert!(p.sketch_frame_by_id(sb).is_none(), "without an external reference the frame of a foreign face must be `None`");

    // Register an external reference from B to a face of body A, then move A by +10 along X to exercise the
    // conversion between local spaces.
    p.set_component_transform(a, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let ext = p.add_external_face_ref(b, body_a, key);
    assert!(p.external_authorized(b, body_a), "the external reference must be authorised");

    let k2 = MockKernel { no_faces: true, ..Default::default() };
    let rep2 = p.regenerate(&k2);
    assert!(!rep2.errors.iter().any(|(id, _)| *id == body_b), "with an external reference body B must not be in the errors: {:?}", rep2.errors);
    assert!(p.mesh_index(body_b).is_some(), "with an external reference body B must be built");

    // The frame of sketch B is anchored to the face of A and carried whole into the local space of B. Its 2D
    // zero is the projection of the origin of source A onto the face ([0,0,5] in the space of A), and after
    // moving A by +10 along X it becomes [10,0,5] in the space of B.
    // Moving A along any axis carries the origin with it, not only along the face normal.
    let f = p.sketch_frame_by_id(sb).expect("the frame must resolve through the external reference");
    assert!(
        (f.origin[0] - 10.0).abs() < 1e-9 && (f.origin[1]).abs() < 1e-9 && (f.origin[2] - 5.0).abs() < 1e-9,
        "the 2D zero must be anchored to the source face and follow it along every axis: {:?}",
        f.origin
    );

    // Top-down associativity: the source body A changed, so the consumer body B must rebuild.
    p.mark_node_dirty(body_a);
    let k3 = MockKernel { no_faces: true, ..Default::default() };
    p.regenerate(&k3);
    assert_eq!(k3.count(), 2, "editing source body A must rebuild the dependent body B as well");

    // Registration is idempotent.
    assert_eq!(p.add_external_face_ref(b, body_a, key), ext, "registering again must return the same id");
    assert_eq!(p.external_refs.len(), 1, "no duplicate external reference must be created");
}

#[test]
fn rigid_joint_offset_translates_along_normal() {
    // A rigid mate honours the gap along the connector normal, leaving the faces a given distance apart.
    use qymcad_core::feature::{JointKind, PLACE_IDENTITY};
    assert_eq!(JointKind::Rigid.motion(0.0, 0.0, 0.0), PLACE_IDENTITY, "a zero offset means flush, that is, the identity");
    let m = JointKind::Rigid.motion(0.0, 7.0, 0.0);
    assert_eq!(m[11], 7.0, "an offset of 7 must translate by +7 along the connector Z axis");
    assert_eq!((m[3], m[7]), (0.0, 0.0), "there must be no shift along X or Y");
}

#[test]
fn joint_angle_is_parametric_via_global() {
    // The angle of a mate can be an expression over a global parameter, like a feature or sketch dimension. It
    // is stored in `feat_dims` under the joint id and evaluated during regenerate before the mates are solved,
    // so editing the parameter recomputes it.
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Param;
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    p.set_grounded(a, true);
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    let j = p.add_joint(ca, cb, JointKind::Revolute);
    // The global parameter `ang` is 90 and the joint angle is the expression `ang`.
    p.parameters.push(Param { name: "ang".into(), expr: "90".into(), value: 90.0 });
    p.set_feat_dim(j, "angle", "ang".into());

    let k = MockKernel { no_faces: true, ..Default::default() };
    p.regenerate(&k);
    assert!((p.joints.iter().find(|x| x.id == j).unwrap().drive[0].unwrap_or(f64::NAN) - 90.0).abs() < 1e-9, "the angle must be taken from the global parameter");
    // The point (1,0,0) fixed in B is turned by 90 degrees into (0,1,0).
    let q = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!(q[0].abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9, "the rotation must use the angle from the parameter (90 degrees): {q:?}");

    // Changing the global parameter recomputes both the angle and the placement.
    p.parameters[0].value = 180.0;
    p.regenerate(&k);
    // The tolerance follows the contract of the solver rather than luck. A tolerance of 1e-9 degrees is
    // tighter than anything the solver promises, its own tolerance being a residual below 1e-7. The check
    // passed only because the solver silently took a hundred and ninety extra steps: its exit threshold was
    // compared against a cost that included the pull towards the original place and was never reached. Once the
    // exit started comparing against the constraint residual (7 steps instead of 200) the angle came out as
    // 179.99999999, that is 1.2e-8 degrees or 2e-10 radians. Demanding more would test rounding rather than the
    // model.
    let got = p.joints.iter().find(|x| x.id == j).unwrap().angle;
    assert!((got - 180.0).abs() < 1e-6, "the angle must be recomputed: {got}");
    let q2 = apply12(&p.world_transform(b), [1.0, 0.0, 0.0]);
    assert!((q2[0] + 1.0).abs() < 1e-9 && q2[1].abs() < 1e-9, "a rotation of 180 degrees: {q2:?}");
}

#[test]
fn skeleton_named_dim_drives_parameter() {
    // A skeleton sketch as a driver: a distance dimension named `len` becomes available as a parameter, so a
    // part references it from an expression in top-down fashion. Editing the skeleton dimension moves the
    // parameter with it.
    use qymcad_core::model::Constraint;
    let mut p = part_project();
    let sid = p.add_line_sketch("skeleton", vec![Point2::new(0.0, 0.0), Point2::new(30.0, 0.0)], false);
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|pt| pt.id).collect();
    let (a, b) = (pts[0], pts[1]);
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    assert!(p.add_named_dim("len".into(), sid, vec![a, b]), "the dimension must be named as a driver");
    assert_eq!(p.param_map().get("len"), Some(&30.0), "a named dimension must be visible as a parameter");
    assert_eq!(p.eval_expr("len*2").unwrap(), 60.0, "a part must be able to reference it from an expression");
    // Editing the skeleton dimension moves the parameter with it.
    if let Constraint::Distance { d, .. } = &mut p.sketches[si].constraints[0] {
        *d = 50.0;
    }
    assert_eq!(p.param_map().get("len"), Some(&50.0), "editing the skeleton dimension must give the parameter its new value");
    // serde
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert_eq!(back.named_dims.len(), 1, "the named dimension must survive serialisation");
    assert_eq!(back.param_map().get("len"), Some(&50.0), "it must still resolve after serialisation");
}

#[test]
fn extrude_extent_direction() {
    // Extrude and cut direction: one-sided, reversed (flip against the normal), symmetric, and two-sided.
    use qymcad_core::feature::extrude_extent;
    assert_eq!(extrude_extent(5.0, 0.0, qymcad_core::feature::Reach::Forward), (0.0, 5.0), "one-sided: [0, +h]");
    assert_eq!(extrude_extent(5.0, 0.0, qymcad_core::feature::Reach::Backward), (-5.0, 5.0), "flipped: [-h, 0] against the normal");
    assert_eq!(extrude_extent(6.0, 0.0, qymcad_core::feature::Reach::BothWays), (-3.0, 6.0), "symmetric: [-h/2, +h/2]");
    assert_eq!(extrude_extent(5.0, 3.0, qymcad_core::feature::Reach::Forward), (-3.0, 8.0), "two-sided: [-down, +h]");
    assert_eq!(extrude_extent(5.0, 3.0, qymcad_core::feature::Reach::Backward), (-5.0, 8.0), "two-sided plus flip: up and down swap");
}

#[test]
fn regen_applies_direction_to_placement() {
    // Regenerate really does apply the direction and the flip to the placement handed to the kernel, not only
    // to `extrude_extent` in isolation. The mock encodes `start` in the translation columns; with the sketch on
    // world XY (X = (1,0,0), N = (0,0,1)) the vertex becomes (total, 0, start), where z is the offset of the
    // start along the normal.
    let mut p = part_project();
    let sid = square(&mut p, "square"); // On world XY.
    p.add_sketch_node(sid, "square");
    let up = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0); // Outwards: [0, +5].
    let dn = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Backward, 0.0); // Flipped: [-5, 0].
    let sym = p.add_extrude_on(sid, 0, 6.0, qymcad_core::feature::Reach::BothWays, 0.0); // Symmetric: [-3, +6].
    let two = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 3.0); // Two-sided: [-3, +8].
    let k = MockKernel::default();
    p.regenerate(&k);
    let z = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].z;
    let x = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].x;
    assert_eq!(z(up), 0.0, "outwards: the profile sits on the plane (start 0)");
    assert_eq!(x(up), 5.0, "outwards: a height of 5 (total)");
    assert_eq!(z(dn), -5.0, "flipped: the profile moved against the normal (start -5)");
    assert_eq!(z(sym), -3.0, "symmetric: start is -h/2");
    assert_eq!(x(sym), 6.0, "symmetric: total is h");
    assert_eq!(z(two), -3.0, "two-sided: start is -down");
    assert_eq!(x(two), 8.0, "two-sided: total is h plus down");
}

#[test]
fn regen_combine_direction_matches_extrude() {
    // A cut or a boss is directional in the same way as an extrude (flip, symmetry, two sides) rather than
    // only "through all". The tool placement is checked through the mock kernel, where z is the start along the
    // normal.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    // One-sided plus flip starts the tool against the normal, at -h.
    let cut_flip = p.add_combine_on(base, sid, 0, 4.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::Backward, ..Default::default() }, 0.0);
    // Two-sided with down = 2 starts at -down.
    let cut_two = p.add_combine_on(base, sid, 0, 4.0, 0, qymcad_core::feature::Extent::default(), 2.0);
    // Symmetric starts at -h/2.
    let cut_sym = p.add_combine_on(base, sid, 0, 4.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::BothWays, ..Default::default() }, 0.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let z = |b| p.bodies[p.mesh_index(b).unwrap()].mesh.verts[0].z;
    assert_eq!(z(cut_flip), -4.0, "a flipped cut puts the tool against the normal (start -h)");
    assert_eq!(z(cut_two), -2.0, "two-sided: start is -down");
    assert_eq!(z(cut_sym), -2.0, "symmetric: start is -h/2");
}

#[test]
fn combine_direction_survives_serde() {
    // The reach and the second side of a combine survive saving and loading.
    use qymcad_core::feature::FeatureKind;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let cut = p.add_combine_on(base, sid, 0, 4.0, 0, qymcad_core::feature::Extent { reach: qymcad_core::feature::Reach::BothWays, ..Default::default() }, 2.5);
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    let (reach, down) = back
        .timeline
        .iter()
        .find_map(|n| if let FeatureKind::Combine { extent, down, .. } = n.kind { Some((extent.reach, down)) } else { None })
        .unwrap();
    assert_eq!(reach, qymcad_core::feature::Reach::BothWays, "the direction of the tool must be saved");
    assert_eq!(down, 2.5, "the second side must be saved");
    let _ = cut;
}

#[test]
fn offset_plane_resolves_from_base() {
    // A parametric plane is an offset from a base plane, and its origin and normal are resolved during
    // regenerate.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let wp = WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::XY, dist: 20.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.normal, [0.0, 0.0, 1.0], "the normal of the base XY plane is +Z");
    assert_eq!(pl.origin, [0.0, 0.0, 20.0], "the origin must be offset by 20 along Z");
}

#[test]
fn offset_plane_dist_is_parametric() {
    // The distance of an offset plane is an expression over a global parameter (the `dist` feature dimension).
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{Param, PlaneDef, WorkPlane};
    let mut p = part_project();
    p.parameters.push(Param { name: "h".into(), expr: "30".into(), value: 30.0 });
    let wp = WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::XZ, dist: 5.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.set_feat_dim(pid, "dist", "h".into()); // dist = h = 30; the XZ plane has normal -Y.
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.origin[1].abs(), 30.0, "the distance from the expression h = 30 must apply along the XZ normal");
}

#[test]
fn datum_point_coords_are_parametric() {
    // The coordinates of a datum point are expressions (the x, y and z feature dimensions), so a global
    // parameter moves the point.
    use qymcad_core::model::Param;
    let mut p = part_project();
    p.parameters.push(Param { name: "g".into(), expr: "7".into(), value: 7.0 });
    let id = p.add_point_at([1.0, 2.0, 3.0]);
    p.set_feat_dim(id, "z", "g*2".into()); // z = 14
    p.regenerate(&MockKernel::default());
    let pt = p.datum_points.iter().find(|d| d.id == id).unwrap();
    assert_eq!(pt.at, [1.0, 2.0, 14.0], "z comes from the expression g*2 = 14 while x and y are stored numbers");
    // Changing the parameter moves the point without editing the feature.
    p.parameters[0].value = 10.0;
    p.mark_param_dependents_dirty();
    p.regenerate(&MockKernel::default());
    assert_eq!(p.datum_points.iter().find(|d| d.id == id).unwrap().at[2], 20.0, "g=10 → z=20");
}

#[test]
fn offset_plane_from_datum_plane_resolves() {
    // A plane offset from another datum plane is parametric and follows its source.
    use qymcad_core::feature::BasePlane;
    let mut p = part_project();
    let base = p.add_offset_plane(BasePlane::XY, 10.0); // origin z=10, normal +Z
    let child = p.add_offset_from_plane(base, 5.0); // Another +5 along the source normal, giving z = 15.
    p.regenerate(&MockKernel::default());
    let c = p.planes.iter().find(|x| x.id == child).unwrap();
    assert_eq!(c.normal, [0.0, 0.0, 1.0]);
    assert!((c.origin[2] - 15.0).abs() < 1e-9, "z must be 15 (10 from the base plus 5): {:?}", c.origin);
    // Moving the source (dist = 30) moves the child to z = 35. Datums are resolved on every regenerate rather
    // than only when dirty.
    p.set_feat_dim(base, "dist", "30".into());
    p.regenerate(&MockKernel::default());
    assert!((p.planes.iter().find(|x| x.id == child).unwrap().origin[2] - 35.0).abs() < 1e-9, "the child must follow its source");
}

#[test]
fn datum_point_at_vertex_is_associative() {
    // An `AtVertex` datum point resolves through the kernel, as an endpoint of a persistent edge, and travels
    // with that vertex.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let pa = p.add_point_at_vertex([0.0; 3], body, 7, false); // The start of edge 7.
    let pb = p.add_point_at_vertex([0.0; 3], body, 7, true); // The end of edge 7.
    let k = MockKernel::default();
    k.edges.borrow_mut().push(qymcad_core::geom::MeshEdge { id: 7, mid: [0.0; 3], dir: [0.0, 0.0, 1.0], a: [1.0, 2.0, 3.0], b: [4.0, 5.0, 6.0], ..Default::default() });
    p.regenerate(&k);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pa).unwrap().at, [1.0, 2.0, 3.0], "`AtVertex` with end = false is the start of the edge");
    assert_eq!(p.datum_points.iter().find(|d| d.id == pb).unwrap().at, [4.0, 5.0, 6.0], "`AtVertex` with end = true is the end of the edge");
    // The vertex moved (the source was rebuilt) and the point follows.
    k.edges.borrow_mut()[0].a = [9.0, 9.0, 9.0];
    p.regenerate(&k);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pa).unwrap().at, [9.0, 9.0, 9.0], "an `AtVertex` point must follow its vertex");
}

#[test]
fn datum_axis_from_edge_and_face_are_associative() {
    // A `FromEdge` or `FromFace` axis resolves through the kernel (the midpoint and tangent of an edge, or the
    // axis of a face) and travels with its source.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let body = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let axe = p.add_axis_from_edge(body, 7);
    let axf = p.add_axis_from_face(body, 3);
    let k = MockKernel::default();
    k.edges.borrow_mut().push(qymcad_core::geom::MeshEdge { id: 7, mid: [1.0, 2.0, 3.0], dir: [0.0, 0.0, 1.0], a: [0.0; 3], b: [0.0; 3], ..Default::default() });
    *k.face_axis.borrow_mut() = Some(([4.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    p.regenerate(&k);
    let e = p.datum_axes.iter().find(|a| a.id == axe).unwrap();
    assert_eq!((e.origin(), e.dir()), ([1.0, 2.0, 3.0], [0.0, 0.0, 1.0]), "`FromEdge` gives the midpoint and tangent of the edge");
    let f = p.datum_axes.iter().find(|a| a.id == axf).unwrap();
    assert_eq!((f.origin(), f.dir()), ([4.0, 0.0, 0.0], [1.0, 0.0, 0.0]), "`FromFace` gives the axis of the face");
    // The edge moved and the axis follows: associativity.
    k.edges.borrow_mut()[0].mid = [9.0, 9.0, 9.0];
    p.regenerate(&k);
    assert_eq!(p.datum_axes.iter().find(|a| a.id == axe).unwrap().origin(), [9.0, 9.0, 9.0], "a `FromEdge` axis must follow its edge");
}

#[test]
fn revolve_around_datum_axis_uses_axis_kernel() {
    // A non-zero `axis_datum` on a revolve makes regenerate convert the datum axis from world into local space
    // and call `revolve_region_axis`.
    use qymcad_core::model::DatumAxis;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    // A datum axis along Y through (-3,0,0) in world space; the sketch is on XY with an identity placement, so
    // the local axis is the same. The axis is deliberately placed beyond the profile: a profile crossing the
    // axis is now rejected honestly, and checking axis resolution on such a profile would test the wrong
    // thing.
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [-3.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let rev = p.add_revolve_axis(sid, Vec::new(), 0, 270.0, ax, 0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c.starts_with("revolve_axis o=[-3.0, 0.0, 0.0] d=[0.0, 1.0, 0.0] a=270")), "it must revolve about the datum axis: {calls:?}");
    drop(calls);
    assert!(p.mesh_index(rev).is_some());
    // With the axis deleted it falls back to an ordinary revolve about the sketch X or Y axis.
    p.delete_datum_axis(ax);
    p.mark_param_dependents_dirty();
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == rev) {
        n.dirty = true;
    }
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c.starts_with("revolve a=270")), "it must fall back to the sketch axes: {:?}", k2.calls.borrow());
}

#[test]
fn revolve_around_sketch_centerline_uses_line_local() {
    // A non-zero `axis_line` on a revolve makes regenerate take the 2D endpoints of the sketch centreline as
    // the local axis — no inverse placement is needed, those already being sketch plane coordinates — and call
    // `revolve_region_axis`.
    let mut p = part_project();
    let si = p.new_sketch("sphere");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "sphere");
    // The profile does not cross the axis, the circle sitting to its right, which is how a body of revolution
    // is built anywhere. The crossing case is checked separately, where it now produces an honest error with a
    // hint.
    p.add_circle_entity(si, 6.0, 5.0, 3.0, qymcad_core::feature::Purpose::Real);
    let axis = p.add_line_entity(si, 0.0, -10.0, 0.0, 10.0, qymcad_core::feature::Purpose::Construction); // The centreline at x = 0.
    let rev = p.add_revolve_axis(sid, Vec::new(), 0, 360.0, 0, axis);
    let k = MockKernel::default();
    p.regenerate(&k);
    let calls = k.calls.borrow();
    // In local space the line endpoints are taken as they are (the placement is the identity): the origin is
    // the first endpoint and the direction is the unit vector along the line.
    assert!(calls.iter().any(|c| c.starts_with("revolve_axis o=[0.0, -10.0, 0.0] d=[0.0, 1.0, 0.0] a=360")), "it must revolve about the sketch centreline: {calls:?}");
    drop(calls);
    assert!(p.mesh_index(rev).is_some(), "the body of revolution must be built");

    // A centreline outranks a datum: with a non-zero `axis_line` the line is used even when `axis_datum` is
    // set.
    use qymcad_core::model::DatumAxis;
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [50.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    if let Some(qymcad_core::feature::FeatureKind::Revolve { axis_datum, .. }) = p.timeline.iter_mut().find(|n| n.id == rev).map(|n| &mut n.kind) {
        *axis_datum = ax;
    }
    p.mark_param_dependents_dirty();
    if let Some(n) = p.timeline.iter_mut().find(|n| n.id == rev) {
        n.dirty = true;
    }
    let k3 = MockKernel::default();
    p.regenerate(&k3);
    assert!(k3.calls.borrow().iter().any(|c| c.starts_with("revolve_axis o=[0.0, -10.0, 0.0]")), "the centreline must outrank the datum: {:?}", k3.calls.borrow());
}

#[test]
fn hole_typed_routes_kind_and_recess_to_kernel() {
    // The hole type (counterbore or countersink) together with the diameter and depth of the recess reach the
    // kernel through `kernel.hole`.
    use qymcad_core::feature::FaceKey;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    let key = FaceKey { index: 0, centroid: [5.0, 5.0, 10.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let h = p.add_hole_typed(base, key, 4.0, 8.0, 1, 9.0, 3.0); // A counterbore.
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "hole k=1 dia=4 depth=8 dia2=9 depth2=3"), "the type and the recess must reach the kernel: {:?}", k.calls.borrow());
    assert!(p.mesh_index(h).is_some());
    // A plain hole gives kind 0 with a zero recess.
    let h2 = p.add_hole(base, key, 3.0, 5.0);
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "hole k=0 dia=3 depth=5 dia2=0 depth2=0"), "a plain hole: {:?}", k.calls.borrow());
    let _ = h2;
}

#[test]
fn holes_from_sketch_points_route_to_kernel() {
    // Holes at the isolated points of a sketch make one `kernel.holes` call with N placements.
    use qymcad_core::model::Param;
    let mut p = part_project();
    let base_sk = square(&mut p, "square");
    p.add_sketch_node(base_sk, "square");
    let base = p.add_extrude_on(base_sk, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    // A separate sketch with three drill marks (isolated points) plus a segment, whose endpoints do not
    // count.
    let hsi = p.new_sketch("drill marks");
    let holes_sk = p.sketches[hsi].id;
    p.add_sketch_node(holes_sk, "drill marks");
    p.sketch_point_at(hsi, 2.0, 2.0, 1e-6);
    p.sketch_point_at(hsi, 8.0, 2.0, 1e-6);
    p.sketch_point_at(hsi, 5.0, 8.0, 1e-6);
    p.add_line_entity(hsi, 0.0, 0.0, 10.0, 0.0, qymcad_core::feature::Purpose::Real); // The endpoints (0,0) and (10,0) are not drill marks.
    assert_eq!(p.sketch_isolated_points(holes_sk).len(), 3, "there must be three isolated points");
    let h = p.add_hole_from_sketch(base, holes_sk, 4.0, 8.0, 0, 0.0, 0.0, false);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert!(k.calls.borrow().iter().any(|c| c == "holes k=0 n=3 dia=4 depth=8 dia2=0 depth2=0"), "three holes must reach the kernel in one call: {:?}", k.calls.borrow());
    assert!(p.mesh_index(h).is_some());
    // A parametric diameter driven by a global parameter.
    p.parameters.push(Param { name: "hd".into(), expr: "6".into(), value: 6.0 });
    p.set_feat_dim(h, "diameter", "hd".into());
    p.mark_param_dependents_dirty();
    let k2 = MockKernel::default();
    p.regenerate(&k2);
    assert!(k2.calls.borrow().iter().any(|c| c == "holes k=0 n=3 dia=6 depth=8 dia2=0 depth2=0"), "the diameter parameter must apply: {:?}", k2.calls.borrow());
}

#[test]
fn feature_inserted_at_rollback_line() {
    // With the rollback bar active a new feature is inserted at the bar, becoming the last active node, and
    // the bar moves past it.
    let mut p = part_project();
    let _a = p.add_box(10.0, 10.0, 10.0);
    let c = p.add_box(5.0, 5.0, 5.0);
    let ti = |p: &qymcad_core::model::Project, id: u64| p.timeline.iter().position(|n| n.id == id).unwrap();
    let ci = ti(&p, c);
    p.set_rollback(Some(ci)); // The bar sits before c, so only the first box is active.
    let d = p.add_box(2.0, 2.0, 2.0); // It must land at the bar, before c.
    assert_eq!(ti(&p, d), ci, "the new feature must sit at the position of the bar");
    assert!(ti(&p, d) < ti(&p, c), "d must be above c");
    assert_eq!(p.rollback, Some(ci + 1), "the bar must move past the new feature");
    // Without a rollback bar it goes to the end.
    p.set_rollback(None);
    let e = p.add_box(1.0, 1.0, 1.0);
    assert_eq!(ti(&p, e), p.timeline.len() - 1, "without a rollback bar it must go to the end of the timeline");
}

#[test]
fn datum_helpers_build_expected_defs() {
    // The single datum constructors the interface calls: an offset plane, a point, a manual axis and an axis
    // through two points.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{AxisDef, PlaneDef};
    let mut p = part_project();
    let pl = p.add_offset_plane(BasePlane::XY, 12.0);
    assert!(matches!(p.planes.iter().find(|x| x.id == pl).unwrap().def, PlaneDef::OffsetBase { base: BasePlane::XY, dist } if (dist - 12.0).abs() < 1e-9));
    let pt = p.add_point_at([3.0, 4.0, 5.0]);
    assert_eq!(p.datum_points.iter().find(|d| d.id == pt).unwrap().at, [3.0, 4.0, 5.0]);
    let axm = p.add_axis_manual([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let am = p.datum_axes.iter().find(|a| a.id == axm).unwrap();
    assert!(matches!(am.def, AxisDef::Manual { .. }) && am.dir() == [0.0, 1.0, 0.0]);
    let ax2 = p.add_axis_two_points(pt, axm); // Arbitrary ids: only the definition is checked.
    assert!(matches!(p.datum_axes.iter().find(|a| a.id == ax2).unwrap().def, AxisDef::TwoPoints { .. }));
    // Every datum is a timeline node.
    for id in [pl, pt, axm, ax2] {
        assert!(p.timeline.iter().any(|n| n.id == id), "datum {id} must be in the timeline");
    }
}

#[test]
fn axis_through_two_points_resolves() {
    // An axis through two datum points gives origin = A and dir = norm(B - A), and follows the coordinates of
    // those points.
    use qymcad_core::model::{AxisDef, DatumAxis, DatumPoint};
    let mut p = part_project();
    let ida = p.add_datum_point(DatumPoint { at: [1.0, 0.0, 0.0], ..Default::default() });
    let idb = p.add_datum_point(DatumPoint { at: [1.0, 0.0, 10.0], ..Default::default() });
    let ax = DatumAxis::from_def("Axis", AxisDef::TwoPoints { a: ida, b: idb });
    let axid = p.add_datum_axis(ax);
    p.regenerate(&MockKernel::default());
    let res = p.datum_axes.iter().find(|x| x.id == axid).unwrap();
    assert_eq!(res.origin(), [1.0, 0.0, 0.0], "the origin must be point A");
    assert_eq!(res.dir(), [0.0, 0.0, 1.0], "dir = norm(B−A) = +Z");
    // Moving point B reorients the axis.
    let bi = p.datum_points.iter().position(|d| d.id == idb).unwrap();
    p.datum_points[bi].at = [11.0, 0.0, 0.0];
    p.regenerate(&MockKernel::default());
    let res2 = p.datum_axes.iter().find(|x| x.id == axid).unwrap();
    assert_eq!(res2.dir(), [1.0, 0.0, 0.0], "after moving B the axis must run along +X: associativity");
}

#[test]
fn offset_plane_from_face_resolves() {
    // A plane offset from a face of a body, by persistent key. The mock kernel supplies no faces, so resolution
    // falls back to the fingerprint (centroid and normal) and the origin becomes the face centre plus the normal
    // times the distance.
    use qymcad_core::feature::FaceKey;
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let key = FaceKey { index: 0, centroid: [1.0, 2.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let wp = WorkPlane { def: PlaneDef::OffsetFace { body: base, face: key, dist: 3.0 }, ..Default::default() };
    let pid = p.add_plane(wp);
    p.regenerate(&MockKernel::default());
    let pl = p.planes.iter().find(|x| x.id == pid).unwrap();
    assert_eq!(pl.normal, [0.0, 0.0, 1.0], "the face normal");
    assert_eq!(pl.origin, [1.0, 2.0, 8.0], "the origin must be the face centre plus the normal times 3");
    // `OffsetFace` survives serialisation.
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert!(matches!(back.planes.iter().find(|x| x.id == pid).unwrap().def, PlaneDef::OffsetFace { dist, .. } if (dist - 3.0).abs() < 1e-9));
}

#[test]
fn circular_array_around_datum_axis() {
    // A circular pattern about a chosen datum axis rather than only about Z; the axis is stored and survives
    // serialisation.
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::DatumAxis;
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let base = p.add_extrude_on(sid, 0, 5.0, qymcad_core::feature::Reach::Forward, 0.0);
    let ax = p.add_datum_axis(DatumAxis::manual("Axis", [0.0; 3], [1.0, 0.0, 0.0])); // The X axis.
    let arr = p.add_circular_array_axis(base, 4, 360.0, ax);
    p.regenerate(&MockKernel::default());
    assert!(p.mesh_index(arr).is_some(), "the pattern about a datum axis must be built");
    let stored = p.timeline.iter().find_map(|n| if let FeatureKind::CircularArray { axis, .. } = n.kind { Some(axis) } else { None }).unwrap();
    assert_eq!(stored, ax, "the pattern axis must be the datum axis");
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    let stored2 = back.timeline.iter().find_map(|n| if let FeatureKind::CircularArray { axis, .. } = n.kind { Some(axis) } else { None }).unwrap();
    assert_eq!(stored2, ax, "the pattern axis must survive serialisation");
}

#[test]
fn datum_defs_survive_serde() {
    // Parametric datum definitions survive serialisation.
    use qymcad_core::feature::BasePlane;
    use qymcad_core::model::{AxisDef, DatumAxis, DatumPoint, PlaneDef, WorkPlane};
    let mut p = part_project();
    let pid = p.add_plane(WorkPlane { def: PlaneDef::OffsetBase { base: BasePlane::YZ, dist: 7.0 }, ..Default::default() });
    let a = p.add_datum_point(DatumPoint::default());
    let b = p.add_datum_point(DatumPoint::default());
    let axid = p.add_datum_axis(DatumAxis::from_def("Axis", AxisDef::TwoPoints { a, b }));
    let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).unwrap()).unwrap();
    assert_eq!(back.planes.iter().find(|x| x.id == pid).unwrap().def, PlaneDef::OffsetBase { base: BasePlane::YZ, dist: 7.0 });
    assert_eq!(back.datum_axes.iter().find(|x| x.id == axid).unwrap().def, AxisDef::TwoPoints { a, b });
}

#[test]
fn external_ref_survives_serde() {
    let mut p = part_project();
    let id = p.add_external_face_ref(7, 42, FaceKey { index: 1, centroid: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], id: 9 });
    let ron = qymcad_core::model::to_ron(&p).unwrap();
    let back = qymcad_core::model::from_ron(&ron).unwrap();
    assert_eq!(back.external_refs.len(), 1, "the external reference must survive serialisation");
    let r = &back.external_refs[0];
    assert_eq!(r.id, id);
    assert_eq!(r.from_component, 7);
    assert_eq!(r.source_body(), Some(42));
}

#[test]
fn regenerate_solves_joint_placement() {
    // The placement pass inside regenerate: the body is built in local space and the mate places the
    // component.
    let mut p: Project = Default::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let a = p.add_part("A");
    p.set_active_component(Some(a));
    let sa = square(&mut p, "sa");
    p.add_sketch_node(sa, "sa");
    p.add_extrude(sa, 5.0);
    // A is grounded and displaced; B is rigidly mated origin to origin.
    p.set_grounded(a, true);
    p.set_component_transform(a, [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    p.set_active_component(Some(root));
    let b = p.add_part("B");
    let ca = p.add_connector(a, AnchorRef::Origin);
    let cb = p.add_connector(b, AnchorRef::Origin);
    p.add_joint(ca, cb, JointKind::Rigid);

    let k = MockKernel::default();
    p.regenerate(&k);
    // A numeric solver converges to a tolerance, so exact equality must not be demanded of it. An earlier path
    // placed the body by pure matrix composition and therefore matched bit for bit, which was a property of the
    // implementation rather than a requirement of the problem. A micrometre here is orders of magnitude below
    // any design significance.
    let got = apply12(&p.world_transform(b), [0.0, 0.0, 0.0]);
    let err = ((got[0] - 10.0).powi(2) + got[1].powi(2) + got[2].powi(2)).sqrt();
    assert!(err < 1e-6, "regenerate must place B by the mate: {got:?}, off by {err:.3e}");
}

#[test]
fn moving_component_does_not_rebuild_bodies() {
    // Placing a component (`Component.transform`) does not rebuild any body, only its position in the assembly.
    let mut p = part_project();
    let sid = square(&mut p, "s");
    p.add_sketch_node(sid, "s");
    let body = p.add_extrude(sid, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    assert_eq!(k.count(), 1);
    let owner = p.body_owner(body).unwrap();
    p.move_component(owner, [10.0, 0.0, 0.0]);
    p.rotate_component(owner, 2, 30.0);
    let rep = p.regenerate(&k);
    assert_eq!(k.count(), 1, "moving or rotating a component must not disturb the kernel");
    assert!(rep.built.is_empty());
}

#[test]
fn same_component_face_sketch_builds() {
    // A sketch on a face of a body of the same part is not a cross-component reference, so it builds.
    let mut p = part_project();
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let body1 = p.add_extrude(s1, 5.0);
    let k = MockKernel::default();
    p.regenerate(&k);
    let s2 = square(&mut p, "s2");
    let si = p.sketch_index(s2).unwrap();
    p.sketches[si].plane = SketchPlane::Face(body1, FaceKey { index: 0, centroid: [5.0, 5.0, 5.0], normal: [0.0, 0.0, 1.0], id: 0 });
    p.add_sketch_node(s2, "s2");
    let body2 = p.add_combine(body1, s2, 3.0, 0);
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "the same part is not a cross-component reference: {:?}", rep.errors);
    assert!(p.mesh_index(body2).is_some(), "the cutting body must be built");
}

#[test]
fn sweep_path_encoded_open_chain_carries_exact_edges() {
    // The path of a sweep is an open contour of two segments, encoded as [1.0, 2.0, two edges of nine fields].
    // The ninth field of an edge is its provenance, the id of the sketch entity, which the kernel uses to name
    // the side faces so that a name follows the sketch line rather than the ordinal of an edge in the contour.
    let mut p = part_project();
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 10.0), Point2::new(5.0, 10.0)], false);
    let enc = p.sweep_path_encoded(path, 0).expect("an open path must encode");
    assert_eq!(enc[0], 1.0, "exactly one path contour");
    assert_eq!(enc[1], 2.0, "two exact edges; the contour is open, so there is no spurious end-to-start edge");
    assert_eq!(enc.len(), 2 + 2 * qymcad_core::geom::EDGE_FIELDS, "one loop count, one edge count, two edges");
    // The first edge is the line from (0,0) to (0,10).
    assert_eq!(&enc[2..10], &[0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0]);
    // The ninth field is the name descriptor of the future face. A path has none and must have none: the side
    // faces of a sweep are produced by the profile edges, while the path only guides them, so there is nowhere
    // for a name to come from.
    assert_eq!(enc[10], 0.0, "a path carries no names: {}", enc[10]);
}

#[test]
fn add_sweep_builds_node_with_both_sketch_inputs() {
    // A sweep node depends on two sketches, the profile and the path, and regenerate calls `kernel.sweep`.
    let mut p = part_project();
    let prof = square(&mut p, "profile");
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 20.0)], false);
    p.add_sketch_node(prof, "profile");
    p.add_sketch_node(path, "path");
    let body = p.add_sweep(prof, Vec::new(), path, 0);
    let node = p.timeline.iter().find(|n| n.id == body).expect("the sweep node must be in the timeline");
    assert_eq!(node.kind.inputs(), vec![prof, path], "it must depend on the profile and the path");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a sweep must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the swept body must be built");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("sweep ")), "the kernel must receive a sweep call: {:?}", k.calls.borrow());
}

#[test]
fn sweep_contour_candidates_order_and_multi() {
    // Candidate profiles are the closed contours of at least three points; candidate paths list the open ones
    // first.
    let mut p = part_project();
    // The helper cannot build one sketch holding two loops, so the check uses a profile sketch with a single
    // contour together with separate open and closed path sketches.
    let prof = square(&mut p, "profile");
    let prof_cands = p.sweep_profile_contours(prof);
    assert_eq!(prof_cands.len(), 1, "a square gives one profile contour");
    // An open chain gives exactly one open path candidate.
    let path = p.add_line_sketch("path", vec![Point2::new(0.0, 0.0), Point2::new(0.0, 8.0), Point2::new(6.0, 8.0)], false);
    let path_cands = p.sweep_path_contours(path);
    assert_eq!(path_cands.len(), 1, "one open chain gives one candidate path");
    // A closed sketch is a valid path as well, as a closed trajectory, though not as an open candidate.
    let ring = square(&mut p, "ring path");
    let ring_cands = p.sweep_path_contours(ring);
    assert_eq!(ring_cands.len(), 1, "a closed contour is a valid path");
    // `add_sweep` with explicit contours records them in the node.
    p.add_sketch_node(prof, "profile");
    p.add_sketch_node(path, "path");
    let body = p.add_sweep(prof, vec![prof_cands[0]], path, path_cands[0]);
    let node = p.timeline.iter().find(|n| n.id == body).unwrap();
    if let qymcad_core::feature::FeatureKind::Sweep { ref profiles, path: pth, .. } = node.kind {
        assert_eq!(profiles, &vec![prof_cands[0]], "the profile contour must be recorded");
        assert_eq!(pth, path_cands[0], "the path contour must be recorded");
    } else {
        panic!("the node is not a sweep");
    }
}

#[test]
fn loft_encoded_concatenates_sections_with_offsets_and_places() {
    // The loft encoding concatenates the loop block of every section: `offsets` starts at zero and holds one
    // boundary per section, and `places` holds twelve placement fields per section, the plane of its sketch.
    let mut p = part_project();
    let s0 = square(&mut p, "bottom"); // On XY at z = 0.
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 20.0); // origin z=20
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.regenerate(&MockKernel::default()); // Resolves the origin of the datum plane at z = 20.
    let enc = p.loft_encoded(&[s0, s1], &[0, 0]).expect("two closed sections must encode");
    let (data, offsets, places) = enc;
    assert_eq!(offsets.len(), 3, "one boundary per section: [0, len0, len0 + len1]");
    assert_eq!(offsets[0], 0, "the first section starts at zero");
    assert_eq!(offsets[2], data.len(), "the last boundary is the end of the data");
    assert_eq!(places.len(), 2 * 12, "twelve placement fields for each of the two sections");
    // The placement of the top section carries the origin z = 20 (column 3 is fields 9, 10 and 11).
    assert_eq!(places[12 + 11], 20.0, "the top section must sit at z = 20");
    // Fewer than two sections cannot be encoded.
    assert!(p.loft_encoded(&[s0], &[0]).is_none(), "a single section is not a loft");
}

#[test]
fn draft_resolves_neutral_face_and_routes_pull_direction() {
    // A draft node depends on its source body. The neutral face, addressed by a persistent id, resolves to a
    // normal that becomes the pull direction, and `flip` inverts it. The angle is parametric and reaches the
    // kernel.
    let mut p = part_project();
    let sid = square(&mut p, "square");
    p.add_sketch_node(sid, "square");
    let src = p.add_extrude_on(sid, 0, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
    let k = MockKernel::default();
    p.regenerate(&k); // Builds src; the mock produces no faces.
    // The faces of the source: the neutral one (persistent id 42, normal +Z, the bottom of the mould) and two
    // drafted ones.
    //
    // The drafted faces 7 and 8 are put in the scene for more than completeness: with query references the
    // feature verifies that the faces exist and refuses when they do not. Their ids used to reach the kernel
    // unchecked, so a draft could be aimed at faces the body no longer had.
    p.regen_faces.insert(
        src,
        vec![
            MeshFace { triangles: vec![], normal: [0.0, 0.0, 1.0], centroid: Point3::new(5.0, 5.0, 0.0), area: 1.0, id: 42 },
            MeshFace { triangles: vec![], normal: [1.0, 0.0, 0.0], centroid: Point3::new(10.0, 5.0, 5.0), area: 1.0, id: 7 },
            MeshFace { triangles: vec![], normal: [0.0, 1.0, 0.0], centroid: Point3::new(5.0, 10.0, 5.0), area: 1.0, id: 8 },
        ],
    );
    // A plain draft pulls along the normal of the neutral face, +Z.
    let body = p.add_draft(src, vec![7, 8], 42, 5.0, false);
    let node = p.timeline.iter().find(|n| n.id == body).unwrap();
    assert_eq!(node.kind.inputs(), vec![src], "a draft depends on its source body");
    let k2 = MockKernel::default();
    k2.shapes.borrow_mut().insert(src); // src is already built (need_src); only the draft is rebuilt.
    let rep = p.regenerate(&k2);
    assert!(rep.errors.is_empty(), "regenerating a draft must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the drafted body must be built");
    assert!(
        k2.calls.borrow().iter().any(|c| c.starts_with("draft n=2 angle=5") && c.contains("pull=[0.0, 0.0, 1.0]")),
        "a +Z neutral face gives a +Z pull over two faces at an angle of 5: {:?}",
        k2.calls.borrow()
    );
    // `flip` pulls the other way, along -Z. Face 9 must exist as well, for the reason given above.
    p.regen_faces.entry(src).or_default().push(MeshFace { triangles: vec![], normal: [-1.0, 0.0, 0.0], centroid: Point3::new(0.0, 5.0, 5.0), area: 1.0, id: 9 });
    let body2 = p.add_draft(src, vec![9], 42, 3.0, true);
    let k3 = MockKernel::default();
    k3.shapes.borrow_mut().insert(src);
    p.regenerate(&k3);
    assert!(
        k3.calls.borrow().iter().any(|c| c.contains("angle=3") && c.contains("pull=[-0.0, -0.0, -1.0]")),
        "flip must pull along -Z: {:?}",
        k3.calls.borrow()
    );
    assert!(p.mesh_index(body2).is_some(), "the flipped draft body must be built");
}

#[test]
fn add_loft_builds_node_and_regenerates() {
    // A loft node depends on every section sketch, in order, and regenerate calls `kernel.loft` with nsec = 2.
    let mut p = part_project();
    let s0 = square(&mut p, "bottom");
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 15.0);
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(s0, "bottom");
    p.add_sketch_node(s1, "top");
    let body = p.add_loft(vec![s0, s1], vec![0, 0], false, 0, 0, false);
    let node = p.timeline.iter().find(|n| n.id == body).expect("the loft node must be in the timeline");
    assert_eq!(node.kind.inputs(), vec![s0, s1], "it must depend on both sections, in order");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a loft must report no errors: {:?}", rep.errors);
    assert!(p.mesh_index(body).is_some(), "the lofted body must be built");
    assert!(k.calls.borrow().iter().any(|c| c.starts_with("loft nsec=2 ")), "the kernel must receive a loft over two sections: {:?}", k.calls.borrow());
}

#[test]
fn subtractive_loft_booleans_with_target_and_consumes_it() {
    // A loft with a non-zero src calls `loft_combine` rather than `loft`: it depends on the target body and
    // consumes it.
    let mut p = part_project();
    // The target body first, a plain extruded square; it becomes the active body and the target of the cut.
    let base_sk = square(&mut p, "base");
    p.add_sketch_node(base_sk, "base");
    let target = p.add_extrude(base_sk, 20.0);
    // Two loft sections on different planes.
    let s0 = square(&mut p, "bottom");
    let s1 = square(&mut p, "top");
    let pl = p.add_offset_plane(BasePlane::XY, 15.0);
    let si1 = p.sketch_index(s1).unwrap();
    p.sketches[si1].plane = SketchPlane::Datum(pl);
    p.add_sketch_node(s0, "bottom");
    p.add_sketch_node(s1, "top");
    let body = p.add_loft(vec![s0, s1], vec![0, 0], false, target, 0, false); // op = 0 is a cut.
    let node = p.timeline.iter().find(|n| n.id == body).expect("the cut-loft node must exist");
    assert!(node.kind.inputs().contains(&target), "a cut-loft depends on the target body");
    assert_eq!(node.kind.consumed_body(), Some(target), "a cut-loft consumes the target body");
    let k = MockKernel::default();
    let rep = p.regenerate(&k);
    assert!(rep.errors.is_empty(), "regenerating a cut-loft must report no errors: {:?}", rep.errors);
    let calls = k.calls.borrow();
    assert!(calls.iter().any(|c| c.starts_with("loft_combine nsec=2 op=0 src_ok=true")), "the kernel must receive loft_combine for the cut with the built body: {:?}", calls);
    assert!(!calls.iter().any(|c| c.starts_with("loft nsec")), "a plain loft must not be called for a boolean");
}

// A multi-contour operation is exactly one node holding every contour, not a chain of extrude plus boolean.
#[test]
fn multi_op_is_single_node_with_all_profiles() {
    let mut p = Project::default();
    p.new_document();
    let s = square(&mut p, "s");
    let before = p.timeline.len();
    // Three contours in a single operation; src = 0 makes a new body.
    let body = p.add_combine_multi_op(0, s, vec![101, 102, 103], 5.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    assert_eq!(p.timeline.len(), before + 1, "N contours must give one node, not a chain");
    let node = p.timeline.iter().find(|n| n.kind.body() == Some(body)).unwrap();
    match &node.kind {
        qymcad_core::feature::FeatureKind::Combine { profiles, src, .. } => {
            assert_eq!(profiles, &vec![101, 102, 103], "every contour must live in the one node");
            assert_eq!(*src, 0, "src = 0 makes a new body");
        }
        _ => panic!("expected a Combine node"),
    }
    // `add_extrude_multi` also produces one node holding every contour.
    let before2 = p.timeline.len();
    let e = p.add_extrude_multi(s, vec![201, 202], 3.0, qymcad_core::feature::Reach::Forward, 0.0, vec![]);
    assert_eq!(p.timeline.len(), before2 + 1, "extruding N contours must give one node");
    match &p.timeline.iter().find(|n| n.kind.body() == Some(e)).unwrap().kind {
        qymcad_core::feature::FeatureKind::Extrude { profiles, .. } => assert_eq!(profiles, &vec![201, 202]),
        _ => panic!("expected an Extrude node"),
    }
}

// Deleting a single-node multi-contour operation is clean: the part body survives and no orphan is left.
#[test]
fn delete_one_node_op_clean() {
    let mut p = Project::default();
    p.new_document();
    let s = square(&mut p, "s");
    // The base part: a Combine with src = 0, which makes a new body.
    let base = p.add_combine_multi_op(0, s, vec![101], 10.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    // A boss of three contours added to the part as a single node.
    let op = p.add_combine_multi_op(base, s, vec![201, 202, 203], 5.0, 1, qymcad_core::feature::Extent::default(), 0.0, vec![]);
    let before = p.timeline.iter().filter(|n| n.kind.body().is_some()).count();
    // Delete the operation.
    let removed = p.delete_feature_op(op);
    assert!(removed.contains(&op), "the operation node itself must be removed");
    assert!(!removed.contains(&base), "the base part must not be removed");
    let after: Vec<Id> = p.timeline.iter().filter_map(|n| n.kind.body()).collect();
    assert!(after.contains(&base), "the part must return to its state before the operation");
    assert_eq!(after.len(), before - 1, "exactly one node must be removed, without splitting into bodies");
    assert!(!after.contains(&op), "no orphan of the operation must remain");
}

/// A fresh edge never takes a name that is already worn.
///
/// The name of an edge is the pair of its faces plus an index within that pair, and two places hand names out: the
/// kernel, which carries them across the history of an operation, and the model, which names whatever is left
/// unnamed. Duplicates arriving from the kernel are handled in `carry_ids`; this covers the second place.
///
/// The scene: an edge between faces A and B already carries the name "pair A-B, index 0". A second edge of the same
/// pair appears without a name. Numbering from zero would hand it the very same index, and two distinct edges would
/// share one address: highlighting would take both, a fillet would cut both, and nothing would be left to select one
/// of them. This is what a through shell with a 2 mm wall produced.
#[test]
fn a_fresh_edge_never_takes_a_name_that_is_already_worn() {
    use qymcad_core::names::{EdgeName, GeoName, Role};
    let mut p = Project::default();
    let sid = square(&mut p, "s");
    let src = p.add_extrude(sid, 10.0);

    let fa = p.names.intern_face(GeoName::new(src, Role::Wall, 1));
    let fb = p.names.intern_face(GeoName::new(src, Role::CapEnd, 0));
    let worn = p.names.intern_edge(EdgeName::new(fa, fb, 0)); // The name is already worn by an older edge.

    // The topology of the body: the older edge and a new one (positional id 7) between the same two faces.
    let k = MockKernel { edge_pairs: RefCell::new(vec![(worn, fa, fb), (7, fa, fb)]), ..Default::default() };
    p.regenerate(&k);

    let ren = k.renamed.borrow().clone();
    let got = ren.iter().find(|(from, _)| *from == 7).map(|(_, to)| *to).expect("a new edge must be given a name");
    assert_ne!(got, worn, "the new edge took the name of its neighbour: two edges at one address cannot be told apart");
    assert_eq!(p.names.edge(got).map(|n| n.index), Some(1), "the name must take the next free index within the pair");
}

// ─── Estimating the size of a rebuild (`regen_plan`) ──────────────────────────────────────────
//
// Cutting a hole in one part reported the whole project as rebuilt, 28 of 28. The counter in the
// window showed the position in the timeline rather than the work done, so "walked past" could not
// be told from "recomputed". The plan answers that question up front and without geometry, which
// makes it an over-approximation by contract: everything a rebuild actually touches must appear in
// it.

/// A project of two independent parts, each with its own sketch and its own body.
fn two_independent_parts() -> (Project, Id, Id, Id, Id) {
    let mut p = part_project();
    let d1 = p.add_component("Part 1");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    let d2 = p.add_component("Part 2");
    p.set_active_component(Some(d2));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 5.0);
    (p, d1, d2, b1, b2)
}

/// An edit in one part does not pull in the other, which is what a reader of the plan expects.
#[test]
fn a_plan_for_an_edit_in_one_part_leaves_the_other_alone() {
    let (mut p, _d1, _d2, b1, b2) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k); // A cold build, so every node is clean afterwards.

    let victim = p.timeline.iter().find(|n| n.kind.body() == Some(b1)).map(|n| n.id).expect("the node of body 1");
    p.mark_node_dirty(victim);
    let plan = p.regen_plan();
    assert!(plan.nodes.contains(&victim), "the edited node must appear in the plan");
    let touches_other = p.timeline.iter().any(|n| plan.nodes.contains(&n.id) && n.kind.body() == Some(b2));
    assert!(!touches_other, "the plan pulls in the other part: an edit in one part must not rebuild the second");
    assert!(plan.nodes.len() < plan.total, "the plan named the whole timeline where one node was touched: {plan:?}");
}

/// The plan is an over-approximation: whatever the rebuild built, the plan had to name.
#[test]
fn everything_rebuilt_was_named_by_the_plan() {
    let (mut p, _d1, _d2, b1, _b2) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k);

    let victim = p.timeline.iter().find(|n| n.kind.body() == Some(b1)).map(|n| n.id).expect("the node of body 1");
    p.mark_node_dirty(victim);
    let planned: HashSet<Id> = p.regen_plan().nodes.into_iter().collect();
    let rep = p.regenerate(&k);
    for (body, _) in rep.built.iter() {
        let node = p.timeline.iter().find(|n| n.kind.bodies().contains(body)).map(|n| n.id);
        assert!(
            node.is_some_and(|id| planned.contains(&id)),
            "body {body} was rebuilt while the plan said nothing about it: the counter and the choice between a modal and a status line would both lie"
        );
    }
}

/// A clean document plans nothing; otherwise the rebuild window would pop up for no reason.
#[test]
fn a_clean_document_plans_nothing() {
    let (mut p, ..) = two_independent_parts();
    let k = MockKernel::default();
    p.regenerate(&k);
    let plan = p.regen_plan();
    assert!(plan.nodes.is_empty(), "after a completed rebuild there is nothing left to rebuild, yet the plan named {:?}", plan.nodes.len());
}

// ─── Mirrored parts and the active body of the source ─────────────────────────────────────────
//
// Found by a scenario run: a full rebuild of an unchanged timeline dropped the mirrored part with
// `SourcePartHasNoBody`, turning 131 bodies into 130, even though the mirror built fine while the
// document was being edited. `MirrorPart` takes the body of its source through `active_body`, which
// is structural: the last body of the component that has not been consumed. So the structure was
// what changed, and the question is whether the mirror learns that its source has a new active
// body.

/// A part with a body, a mirror of that part, and an operation that consumes the body of the source.
#[test]
fn a_mirror_follows_the_source_when_its_active_body_is_replaced() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);

    let mirror = p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let k = MockKernel::default();
    p.regenerate(&k);
    let mirror_body = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MirrorPart { body, .. } => Some(body),
            _ => None,
        })
        .expect("the mirror node");
    assert!(p.mesh_index(mirror_body).is_some(), "setup: the mirror must build on the first pass");
    let _ = mirror;

    // The body of the source is consumed and replaced by a new one, exactly what a body boolean or a split does.
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);
    assert_ne!(merged, b1, "setup: a boolean must produce a new body");
    assert_eq!(p.active_body(d1), Some(merged), "setup: the new body must become the active body of the source");

    // The mirror must learn of this at once, not ten steps later.
    let mirror_dirty = p.timeline.iter().find(|n| n.kind.bodies().contains(&mirror_body)).map(|n| n.dirty);
    p.regenerate(&k);
    assert!(
        p.regen_errors.get(&mirror_body).is_none(),
        "the mirror failed after the active body of the source was replaced: {:?} (its dirty flag was {mirror_dirty:?})",
        p.regen_errors.get(&mirror_body)
    );
    assert!(
        p.mesh_index(mirror_body).is_some(),
        "the mirror was left without a body after the active body of the source was replaced (its dirty flag was {mirror_dirty:?})"
    );
}

/// The same document gives the same result, however it was reached.
///
/// The set of bodies used to depend on the history of rebuilds: one document gave three different answers depending
/// on whether it was rebuilt along the way. The cause was not full versus incremental rebuilding but a forward
/// reference: the mirror took the body of its source through `active_body`, which scanned the entire timeline. A
/// boolean consumed the body of the source and produced a new one, and the mirror saw that new body even though the
/// node producing it stands below the mirror itself. Rebuild along the way and the mirror latches onto the old body;
/// rebuild once at the end and it fails.
#[test]
fn rebuilding_the_same_timeline_twice_gives_the_same_bodies() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);

    let k = MockKernel::default();
    let _ = merged;
    p.regenerate(&k);
    let first: HashSet<Id> = p.bodies.iter().map(|b| b.id).collect();

    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    p.regenerate(&k);
    let second: HashSet<Id> = p.bodies.iter().map(|b| b.id).collect();
    assert_eq!(
        first, second,
        "a full rebuild of the same timeline gave a different set of bodies: {:?} vanished, {:?} appeared, so the model is not reproducible",
        first.difference(&second).collect::<Vec<_>>(),
        second.difference(&first).collect::<Vec<_>>()
    );
}

/// Where the mirror node lands in the timeline, measured rather than assumed.
///
/// A mirror copies the body of its source, so in the history it must stand after that body. Standing above it, the
/// document would describe something impossible: a copy of what does not exist yet at that point.
#[test]
fn a_mirror_node_stands_below_the_body_it_copies() {
    let mut p = part_project();
    let d1 = p.add_component("Source");
    p.set_active_component(Some(d1));
    let s1 = square(&mut p, "s1");
    p.add_sketch_node(s1, "s1");
    let b1 = p.add_extrude(s1, 5.0);
    let mirror_comp = p.add_mirror_part(d1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let _ = mirror_comp;

    let pos = |p: &Project, b: Id| p.timeline.iter().position(|n| n.kind.bodies().contains(&b));
    let mirror_body = p
        .timeline
        .iter()
        .find_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MirrorPart { body, .. } => Some(body),
            _ => None,
        })
        .expect("the mirror node");
    assert!(
        pos(&p, mirror_body) > pos(&p, b1),
        "the mirror node stands above the body it copies: mirror at {:?}, source body at {:?}",
        pos(&p, mirror_body),
        pos(&p, b1)
    );

    // ...and it still holds once the source has gained a new active body.
    p.set_active_component(Some(d1));
    let s2 = square(&mut p, "s2");
    p.add_sketch_node(s2, "s2");
    let b2 = p.add_extrude(s2, 3.0);
    let merged = p.add_body_boolean(b1, b2, 1);
    assert_eq!(p.active_body(d1), Some(merged), "setup: the merged body must become the active one");
    let seen = p.active_body_before(d1, pos(&p, mirror_body).expect("the position of the mirror"));
    assert!(
        seen.is_some(),
        "the mirror sees no body of the source above itself, so it copies what does not exist at its point in the timeline"
    );
}

/// A moved sketch does not cost a fillet its edges.
///
/// Taken from a real document: editing the coordinates of a rectangle in a sketch left a fillet, picked by hand over
/// edges further down the timeline, unable to find them. The cutouts stayed sharp and the edges had to be picked
/// again. Measured on that file, healing returned 28 of the 36 picked edges.
///
/// The cause was the measure: a replacement was accepted only while the edge stayed within 0.5 mm of its former
/// place. An edit moves geometry by an amount comparable to the part, not to half a millimetre. The tolerance is now
/// derived from the bounding box of the body, and the same file returns 34 of 36.
#[test]
fn a_moved_sketch_does_not_cost_the_fillet_its_edges() {
    let mut p = Project::default();
    // A snapshot of an edge: id, midpoint and direction, which is what the reference is healed by.
    let edge = |id: u32, mid: [f64; 3]| qymcad_core::geom::MeshEdge {
        id,
        a: [mid[0] - 5.0, mid[1], mid[2]],
        b: [mid[0] + 5.0, mid[1], mid[2]],
        mid,
        dir: [1.0, 0.0, 0.0],
        center: mid,
        axis: [0.0, 0.0, 1.0],
        radius: 0.0,
        ref_dir: [0.0, 0.0, 1.0],
    };
    let fid: Id = 100;
    // A part roughly 40 mm across; the edges used to sit here.
    p.edge_refs.insert(fid, vec![(7, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), (8, [40.0, 0.0, 0.0], [1.0, 0.0, 0.0])]);
    // After the sketch edit the names changed and the edges themselves moved by one and a half millimetres.
    let now = vec![edge(70, [1.5, 0.0, 0.0]), edge(80, [41.5, 0.0, 0.0])];
    let healed = p.resolve_edge_ids_in(&now, fid, &[7, 8]);
    assert_eq!(
        healed.len(),
        2,
        "healing lost the edges that moved with the edit: it returned {healed:?}, leaving unfilleted cutouts and no error to explain them"
    );

    // ...and it does not grab an unrelated edge. An edge at the other end of the part is no replacement for one that vanished.
    let far = vec![edge(90, [200.0, 0.0, 0.0])];
    let wrong = p.resolve_edge_ids_in(&far, fid, &[7]);
    assert!(wrong.is_empty(), "healing grabbed the distant edge {wrong:?}: a fillet landing in the wrong place is worse than a lost one");
}
