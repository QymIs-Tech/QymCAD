//! The single implementation of the geometric kernel of the model (`qymcad_core::feature::Kernel`) over OCCT.
//!
//! This code used to live as two copies, one in the application and one in the reproduction harness: 34 methods,
//! 445 and 450 lines, 99.5 per cent identical text. The copies had drifted apart — the harness the tests run
//! against tessellated at a fixed deflection of 0.5 while the application used one adaptive to the size of the
//! body. A green test on the volume or area of a mesh therefore did not prove that the same thing appeared on
//! screen, and an edit to the kernel in one copy silently failed to reach the other. There is now one
//! implementation for everybody.
#![allow(clippy::too_many_arguments)]
use qymcad_core::geom::{Built, Mesh};
use qymcad_core::model::Id;

/// The implementation of the geometric kernel through OCCT. It caches live B-rep shapes by body id, in world
/// coordinates, so that modifying features — cuts, chamfers, fillets — can take their source body from it.
#[derive(Default)]
pub struct OcctKernel {
    pub shapes: std::cell::RefCell<std::collections::HashMap<Id, crate::Shape>>,
    /// The geometric tolerance of the document: the fraction of the diagonal used as the tessellation
    /// deflection.
    ///
    /// The kernel does not choose it but receives it: it is a property of the file (`Project::geom_quality`),
    /// and keeping it as a constant here would hand the appearance of a part and the contents of an STL to
    /// whichever machine happened to open it. A zero, the factory value of a record, reads as the ordinary
    /// tolerance; see `k()`.
    pub quality_k: f64,
    /// THE EDGES A BLEND LEFT OUT, per body built: (edges asked, edges left out). A rounding that cannot take an
    /// edge builds the rest, and the node says so in words rather than going red or keeping quiet.
    pub dropped: std::cell::RefCell<std::collections::HashMap<Id, qymcad_core::feature::DroppedEdges>>,
    /// THE REQUEST TO STOP, shared with whoever started the rebuild. The rebuild itself looks at it between nodes;
    /// an operation that runs long inside one node (recognising a mesh: 11 s of 14 s on a 160k-triangle ball in a
    /// debug build) looks at it as it goes, so that neither Cancel nor deleting the source waits for it to finish.
    pub stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// THE ROLES OF THE FACES AN OPERATION MADE, per body built, awaiting their names: the table of the document gives
    /// them when the result lands (`naming::silent`).
    pub roles: std::cell::RefCell<std::collections::HashMap<Id, crate::door::Roles>>,
}

/// THE KERNEL REFUSED AN OPERATION: the coded fact goes on to the user, the kernel's own words to the log.
///
/// Two readers want two different things from one failure. A person drawing a part wants "the fillet did not
/// take" in their own language, which is what `CoreError` carries. A person fixing the program wants the
/// place inside the kernel and whatever OCCT said about it — untranslated, naming internals, useless and
/// alarming in a window. So the words are not put into `CoreError`; they are written out here, marked
/// `QYMWHY`, and the coded error travels on unchanged.
///
/// The buffer is cleared once read, so an old refusal is never handed to a later failure as its reason.
pub(crate) fn refused(op: qymcad_core::errors::Op) -> qymcad_core::errors::CoreError {
    if let Some(why) = crate::last_kernel_refusal() {
        eprintln!("QYMWHY {op:?} refused by the kernel -- {why}");
        // AND IT IS KEPT, not only printed. The refusal channel belongs to the CALLING thread and is
        // cleared right here, while the work runs on a background thread and the interface reads on its
        // own - so by the time a person goes to report the trouble, the words are gone. On a packaged
        // Windows build there is no terminal to print into at all (`windows_subsystem = "windows"`), so
        // the line above reaches nobody. This copy is what a problem report can carry.
        crate::keep_for_report(op, &why);
        crate::clear_kernel_refusal();
    }
    qymcad_core::errors::CoreError::OpFailed(op)
}

/// `s` placed by the 3x4 matrix `place`, or as it is when that is the identity.
fn placed(s: crate::Shape, place: [f64; 12]) -> Result<crate::Shape, qymcad_core::errors::CoreError> {
    if place == qymcad_core::feature::PLACE_IDENTITY {
        Ok(s)
    } else {
        s.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))
    }
}

/// One rebuild at a time per process, held for the whole duration of the work with the kernel.
///
/// OCCT is not built for two computations at once: parts of its machinery keep shared state without any
/// protection. While the kernel used only high-level booleans this went unnoticed; resolving pinched faces
/// calls the low-level ones (`BOPAlgo_Builder`, `BOPAlgo_BuilderFace`), and the price showed at once — in a run
/// a neighbouring thread received a chamfer in the wrong place or a fillet of the wrong depth, once in every
/// four launches. Single-threaded, the result is stable.
///
/// The cost is nothing: the application rebuilds a document one at a time anyway — there is one background
/// rebuild and its result is awaited. The lock merely makes that a rule rather than a coincidence.
pub fn kernel_gate() -> std::sync::MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The lock of the kernel if nobody holds it: a question the window asks every frame is not worth waiting minutes for
/// a rebuild, and asked beside one it would race it inside OCCT. `None` while a rebuild runs.
pub fn try_kernel_gate() -> Option<std::sync::MutexGuard<'static, ()>> {
    match GATE.try_lock() {
        Ok(g) => Some(g),
        Err(std::sync::TryLockError::Poisoned(e)) => Some(e.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    }
}

static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl OcctKernel {
    /// The names of the faces of body `src`, none when it is not built.
    pub(crate) fn face_ids(&self, src: Id) -> Vec<u32> {
        self.shapes.borrow().get(&src).map(|s| s.face_places().iter().map(|p| p.id).collect()).unwrap_or_default()
    }

    /// The fraction of the diagonal. A zero means the record was written without a tolerance, so the ordinary
    /// one is taken: the very value the program lived by before the setting existed.
    pub(crate) fn k(&self) -> f64 {
        if self.quality_k > 0.0 {
            self.quality_k
        } else {
            qymcad_core::model::GeomQuality::Normal.deflection_k()
        }
    }
}

impl OcctKernel {
    /// Extrude a profile and place the shape by `place`, a 3×4 matrix, giving a shape in world coordinates.
    fn placed_extrude(profile: &[f64], h: f64, place: [f64; 12]) -> Option<crate::Shape> {
        let s = crate::Shape::extrude(profile, h)?;
        if place == qymcad_core::feature::PLACE_IDENTITY {
            Some(s)
        } else {
            s.transformed(&place)
        }
    }
}

/// THE HALF OF A KERNEL THAT TRAVELS: a kernel of its own holding the bodies of one node.
///
/// It exists because two threads over one OCCT shape is exactly what this kernel is not safe for. The bodies
/// are moved into it, the parcel of work is run on it, and the shapes - the ones it was lent plus the one it
/// built - go home afterwards.
pub struct OcctWorker {
    pub kernel: OcctKernel,
}

impl qymcad_core::feature::KernelWorker for OcctWorker {
    fn kernel(&self) -> &dyn qymcad_core::feature::Kernel {
        &self.kernel
    }

    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

impl qymcad_core::feature::Kernel for OcctKernel {
    fn extrude(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Extrude, body, |_| Self::placed_extrude(profile, height, place).ok_or_else(|| refused(qymcad_core::errors::Op::Extrude)))
    }
    fn revolve(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Revolve, body, |_| {
            let s = crate::Shape::revolve(profile, axis, angle_deg).ok_or_else(|| refused(qymcad_core::errors::Op::Revolve))?;
            placed(s, place)
        })
    }
    fn boolean(&self, body: Id, base: qymcad_core::feature::Extruded, tool: qymcad_core::feature::Extruded, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        let (qymcad_core::feature::Extruded { profile: base, height: base_h }, qymcad_core::feature::Extruded { profile: tool, height: tool_h }) = (base, tool);
        self.plain(qymcad_core::errors::Op::Boolean, body, |_| {
            let b = crate::Shape::extrude(base, base_h).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
            let t = crate::Shape::extrude(tool, tool_h).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
            placed(b.boolean(&t, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?, place)
        })
    }
    fn copy_faces(&self, body: Id, src: Id, faces: &[u32], feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::faces::instances(feature, faces, &mut roles);
        self.plain_named(qymcad_core::errors::Op::CopyFaces, body, roles, |b| {
            let s = b.get(src)?;
            s.copy_faces(faces, &names).ok_or_else(|| refused(qymcad_core::errors::Op::CopyFaces))
        })
    }
    fn offset_faces(&self, body: Id, src: Id, faces: &[u32], feature: Id, dist: f64) -> Result<Built, qymcad_core::errors::CoreError> {
        if let Some(radius) = self.shapes.borrow().get(&src).and_then(|s| crate::ops::faces::past_radius(s, faces, dist)) {
            return Err(qymcad_core::errors::CoreError::OffsetPastRadius { radius });
        }
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::faces::instances(feature, faces, &mut roles);
        self.plain_named(qymcad_core::errors::Op::OffsetSurface, body, roles, |b| {
            let s = b.get(src)?;
            s.offset_faces(faces, &names, dist).ok_or_else(|| refused(qymcad_core::errors::Op::OffsetSurface))
        })
    }
    fn patch(&self, body: Id, src: Id, edges: &[u32], tangent: bool, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let name = crate::ops::faces::patch(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Patch, body, roles, |b| {
            let s = b.get(src)?;
            s.patch(edges, tangent, name).ok_or_else(|| refused(qymcad_core::errors::Op::Patch))
        })
    }
    fn replace_faces(&self, body: Id, src: Id, faces: &[u32], surface: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::ReplaceFaces, body, |b| {
            let (base, surf) = (b.get(src)?, b.get(surface)?);
            // If it did not close, say how many edges were left free. An unnamed "the operation failed" is
            // useless here: confusing the boundary of an opening is easy — the ring of an end face against the
            // whole hole — and the message gave no way to tell that was the problem.
            base.replace_faces(faces, surf, 1e-4).map_err(|free| match free {
                0 => qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::ReplaceFaces),
                n => qymcad_core::errors::CoreError::SurfaceDoesNotClose { free: n },
            })
        })
    }
    fn stitch(&self, body: Id, parts: &[Id], tol: f64) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Stitch, body, |b| {
            let mut got: Vec<&crate::Shape> = Vec::with_capacity(parts.len());
            for p in parts {
                got.push(b.get(*p)?);
            }
            // Nothing to stitch is not "the operation failed". The kernel stitches along shared edges; if
            // none joined, the surfaces simply do not touch, and that is what has to be said.
            crate::Shape::stitch(&got, tol).map_err(|crate::NothingJoined| qymcad_core::errors::CoreError::StitchNothingJoined)
        })
    }
    fn mesh_solid(&self, body: Id, mesh: &Mesh) -> Result<Built, qymcad_core::errors::CoreError> {
        // a polyhedron of the mesh's flat faces, sewn: a solid where the mesh closes, a surface where it does not
        self.plain(qymcad_core::errors::Op::MeshSolid, body, |_| crate::Shape::from_mesh(mesh).ok_or_else(|| refused(qymcad_core::errors::Op::MeshSolid)))
    }
    fn mesh_recognised(&self, body: Id, mesh: &Mesh, tol: f64, sharp: f64) -> Result<Built, qymcad_core::errors::CoreError> {
        // the whole chain of recognition: prepare, split into regions, fit a surface to each, find the corners and the
        // curves they meet in, build the faces and sew them
        let stopped = || self.stop.load(std::sync::atomic::Ordering::Relaxed);
        self.plain(qymcad_core::errors::Op::MeshRecognise, body, |_| {
            let prepared = qymcad_meshfit::prepare(mesh, qymcad_meshfit::weld_tolerance(mesh));
            if stopped() {
                return Err(refused(qymcad_core::errors::Op::MeshRecognise));
            }
            let mut tolerance = qymcad_meshfit::Tolerance::for_mesh(&prepared);
            tolerance.distance *= if tol > 0.0 { tol } else { 1.0 };
            if sharp > 0.0 {
                tolerance.sharp_deg = sharp;
            }
            let found = qymcad_meshfit::regions_until(&prepared, &tolerance, &stopped).ok_or_else(|| refused(qymcad_core::errors::Op::MeshRecognise))?;
            let bounds = qymcad_meshfit::boundaries(&prepared, &found);
            let curves = qymcad_meshfit::curves(&bounds, &found, &tolerance);
            let made = crate::recognise::recognise_until(&prepared, &found, &bounds, &curves, &tolerance, &stopped).ok_or_else(|| refused(qymcad_core::errors::Op::MeshRecognise))?;
            Ok(made.shape)
        })
    }
    fn trim(&self, body: Id, src: Id, tool: Id, keep: [f64; 3]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Trim, body, |b| b.get(src)?.trim(b.get(tool)?, keep).ok_or_else(|| refused(qymcad_core::errors::Op::Trim)))
    }
    fn body_is_sheet(&self, body: Id) -> bool {
        self.shapes.borrow().get(&body).is_some_and(|s| s.is_sheet())
    }
    fn body_pieces(&self, body: Id) -> u32 {
        self.shapes.borrow().get(&body).map_or(1, |s| s.solid_count().max(1))
    }
    fn body_solids(&self, body: Id) -> Vec<([f64; 3], f64)> {
        self.shapes.borrow().get(&body).map(|s| s.solids_info()).unwrap_or_default()
    }
    fn take_solid(&self, body: Id, src: Id, index: usize) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::SplitBody, body, |b| b.get(src)?.solid_at(index).ok_or(qymcad_core::errors::CoreError::BodyInOnePiece))
    }
    fn workers(&self) -> usize {
        crate::workers()
    }

    fn shares(&self, a: Id, b: Id) -> bool {
        if a == b {
            return true;
        }
        let shapes = self.shapes.borrow();
        match (shapes.get(&a), shapes.get(&b)) {
            (Some(x), Some(y)) => x.shares_geometry(y),
            _ => false,
        }
    }

    fn split_off(&self, bodies: &[Id]) -> Option<Box<dyn qymcad_core::feature::KernelWorker>> {
        let mut mine = self.shapes.borrow_mut();
        let mut taken: std::collections::HashMap<Id, crate::Shape> = std::collections::HashMap::new();
        for b in bodies {
            if let Some(s) = mine.remove(b) {
                taken.insert(*b, s);
            }
        }
        Some(Box::new(OcctWorker { kernel: OcctKernel { shapes: std::cell::RefCell::new(taken), quality_k: self.quality_k, stop: self.stop.clone(), ..Default::default() } }))
    }

    fn absorb(&self, worker: Box<dyn qymcad_core::feature::KernelWorker>) {
        let Ok(w) = worker.as_any().downcast::<OcctWorker>() else { return };
        let mut mine = self.shapes.borrow_mut();
        for (id, shape) in w.kernel.shapes.into_inner() {
            mine.insert(id, shape);
        }
        self.dropped.borrow_mut().extend(w.kernel.dropped.into_inner());
        self.roles.borrow_mut().extend(w.kernel.roles.into_inner());
    }
    fn take_dropped_edges(&self, body: Id) -> Option<qymcad_core::feature::DroppedEdges> {
        self.dropped.borrow_mut().remove(&body)
    }

    fn body_edge_geometry(&self, body: Id) -> Vec<(u32, Vec<[f64; 3]>, Option<([f64; 3], [f64; 3], f64)>)> {
        let shapes = self.shapes.borrow();
        let Some(s) = shapes.get(&body) else { return Vec::new() };
        let (polys, ids, circles) = s.edges_full();
        polys
            .into_iter()
            .zip(ids)
            .zip(circles)
            .filter(|((_, id), _)| *id != 0)
            .map(|((poly, id), circ)| (id, poly.into_iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect(), circ))
            .collect()
    }
    fn edge_face_pairs(&self, body: Id) -> Vec<qymcad_core::feature::EdgeFaces> {
        self.shapes.borrow().get(&body).map(|s| s.edge_face_pairs()).unwrap_or_default()
    }
    fn name_silent(&self, ask: qymcad_core::feature::SilentNaming<'_>) -> qymcad_core::feature::NamedAfter {
        crate::naming::silent(self, ask)
    }
    fn edges(&self, body: Id) -> Vec<qymcad_core::geom::MeshEdge> {
        // The edges of the built body: a persistent id, the midpoint along the polyline, and the tangent. The
        // geometry comes from the existing `Shape::edges_with_ids` through the FFI; no new C++ is needed.
        let shapes = self.shapes.borrow();
        let Some(s) = shapes.get(&body) else {
            return Vec::new();
        };
        let mut edges = s
            .edges_info()
            .into_iter()
            .filter_map(|e| {
                let (poly, id, circle) = (e.poly, e.id, e.circle);
                if poly.len() < 2 || id == 0 {
                    return None;
                }
                let p = |i: usize| [poly[i][0] as f64, poly[i][1] as f64, poly[i][2] as f64];
                let seg = |a: [f64; 3], b: [f64; 3]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
                let total: f64 = (0..poly.len() - 1).map(|i| seg(p(i), p(i + 1))).sum();
                let half = total * 0.5;
                let mut acc = 0.0;
                let mut mid = p(poly.len() - 1);
                let mut dir = [0.0, 0.0, 1.0];
                for i in 0..poly.len() - 1 {
                    let (a, b) = (p(i), p(i + 1));
                    let l = seg(a, b);
                    if acc + l >= half {
                        let t = if l > 1e-9 { (half - acc) / l } else { 0.0 };
                        mid = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
                        let mut d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                        let dl = seg([0.0; 3], d);
                        if dl > 1e-9 {
                            d = [d[0] / dl, d[1] / dl, d[2] / dl];
                        }
                        dir = d;
                        break;
                    }
                    acc += l;
                }
                // For a circular edge, from a hole or a cylinder, the kernel gives the true centre and axis of
                // the circle, which is the concentric anchor rather than a point on the rim. Otherwise the
                // centre and axis come out zero.
                let (center, axis, radius) = circle.unwrap_or_default();
                // The secondary axis of a connector is the normal of the adjacent face. A zero vector honestly
                // means there is no adjacent face: the roll about such an edge is undefined, and the solver
                // leaves it free rather than inventing an orientation.
                let ref_dir = e.ref_dir.unwrap_or_default();
                Some(qymcad_core::geom::MeshEdge { id, mid, dir, a: p(0), b: p(poly.len() - 1), center, axis, radius, ref_dir, ends: Default::default() })
            })
            .collect::<Vec<_>>();
        crate::naming::name_edge_ends(&mut edges, &s.vertices(), &s.edge_vertices());
        edges
    }
    fn face_axis(&self, body: Id, face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        // the axis of a cylindrical or conical face by its persistent id, through the same FFI used when
        // picking the axis of a pattern
        self.shapes.borrow().get(&body).and_then(|s| s.face_axis(face_id))
    }
    fn extrude_region(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        // an exact profile, with contours from real edges, plus holes in one face, giving an exact body
        self.plain(qymcad_core::errors::Op::ExtrudeProfile, body, |_| placed(crate::Shape::extrude_profile(profile, height).ok_or_else(|| refused(qymcad_core::errors::Op::ExtrudeProfile))?, place))
    }
    fn revolve_region(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        // the X or Y axis of a sketch is a special case of a general axis and takes the same path, so that
        // face names are derived from the recipe rather than from the traversal order
        let (o, d) = ([0.0, 0.0, 0.0], if axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] });
        self.plain(qymcad_core::errors::Op::RevolveProfile, body, |_| {
            placed(crate::Shape::revolve_profile_axis_named(profile, o, d, angle_deg, caps).ok_or_else(|| refused(qymcad_core::errors::Op::RevolveProfile))?, place)
        })
    }
    fn revolve_region_axis(&self, body: Id, profile: &[f64], line: qymcad_core::feature::AxisLine, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        // revolving about an arbitrary local axis, then placing the sketch
        self.plain(qymcad_core::errors::Op::RevolveAxis, body, |_| {
            placed(crate::Shape::revolve_profile_axis_named(profile, line.origin, line.dir, angle_deg, caps).ok_or_else(|| refused(qymcad_core::errors::Op::RevolveAxis))?, place)
        })
    }
    fn sweep(&self, body: Id, profile: &[f64], profile_place: [f64; 12], path: &[f64], path_place: [f64; 12], caps: [u32; 2]) -> Result<Built, qymcad_core::errors::CoreError> {
        // a profile placed on its own plane is swept along a path on its own plane, giving a body
        self.plain(qymcad_core::errors::Op::Sweep, body, |_| crate::Shape::sweep_profile_named(profile, &profile_place, path, &path_place, caps).ok_or_else(|| refused(qymcad_core::errors::Op::Sweep)))
    }
    fn revolve_region_multi(
        &self,
        bo: qymcad_core::model::BodyOp,
        profiles: &[Vec<f64>],
        about: qymcad_core::feature::RevolveAbout,
        angle_deg: f64,
        place: [f64; 12],
        sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        // revolve every profile, fuse them into one tool, then take a single boolean against the source
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        let named = crate::ops::profile::name_sides(profiles, sides, qymcad_core::names::Role::Revolved);
        let (profiles, caps) = (&named.profiles, &named.caps);
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        let (o, d) = match about.line {
            Some(l) => (l.origin, l.dir),
            None => ([0.0, 0.0, 0.0], if about.axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }),
        };
        let fail = qymcad_core::errors::CoreError::OpFailed(if about.line.is_some() { qymcad_core::errors::Op::RevolveAxis } else { qymcad_core::errors::Op::RevolveProfile });
        self.plain_named(qymcad_core::errors::Op::RevolveProfile, body, named.roles.clone(), |b| {
            let mut tool: Option<crate::Shape> = None;
            for (i, prof) in profiles.iter().enumerate() {
                // each region gets its own cap names (`ops::profile::name_sides`)
                let cap = [caps.get(i * 3 + 1).copied().unwrap_or(0), caps.get(i * 3 + 2).copied().unwrap_or(0)];
                let s = crate::Shape::revolve_profile_axis_named(prof, o, d, angle_deg, cap).ok_or(fail.clone())?;
                tool = Some(match tool {
                    None => s,
                    Some(t) => t.boolean(&s, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
                });
            }
            let tool = placed(tool.ok_or(qymcad_core::errors::CoreError::NoContours)?, place)?;
            // a new body: the fused tool itself
            if src == 0 {
                return Ok(tool);
            }
            b.get(src)?.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))
        })
    }
    fn sweep_multi(
        &self,
        bo: qymcad_core::model::BodyOp,
        profiles: &[Vec<f64>],
        profile_place: [f64; 12],
        path: &[f64],
        path_place: [f64; 12],
        sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        let named = crate::ops::profile::name_sides(profiles, sides, qymcad_core::names::Role::Swept);
        let (profiles, caps) = (&named.profiles, &named.caps);
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        self.plain_named(qymcad_core::errors::Op::Sweep, body, named.roles.clone(), |b| {
            let mut tool: Option<crate::Shape> = None;
            for (i, prof) in profiles.iter().enumerate() {
                let cap = [caps.get(i * 3 + 1).copied().unwrap_or(0), caps.get(i * 3 + 2).copied().unwrap_or(0)];
                let s = crate::Shape::sweep_profile_named(prof, &profile_place, path, &path_place, cap).ok_or_else(|| refused(qymcad_core::errors::Op::Sweep))?;
                tool = Some(match tool {
                    None => s,
                    Some(t) => t.boolean(&s, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
                });
            }
            let tool = tool.ok_or(qymcad_core::errors::CoreError::NoContours)?;
            if src == 0 {
                return Ok(tool);
            }
            b.get(src)?.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))
        })
    }
    fn loft(
        &self,
        body: Id,
        sections: qymcad_core::feature::LoftSections,
        walls: qymcad_core::feature::LoftWalls,
        kind: qymcad_core::feature::LoftBody,
        sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::feature::LoftSections { data: sections, offsets, places } = sections;
        let named = crate::ops::profile::name_sections(sections, offsets, sides);
        // a body through a set of sections, each on its own plane
        self.plain_named(qymcad_core::errors::Op::Loft, body, named.roles, |_| {
            crate::Shape::loft_sections_named(&named.data, offsets, places, walls, kind, named.caps).ok_or_else(|| refused(qymcad_core::errors::Op::Loft))
        })
    }
    fn loft_combine(
        &self,
        bo: qymcad_core::model::BodyOp,
        sections: &[f64],
        offsets: &[usize],
        places: &[f64],
        walls: qymcad_core::feature::LoftWalls,
        sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        let named = crate::ops::profile::name_sections(sections, offsets, sides);
        // a lofted solid used as a tool, a closed body, taken in a boolean against the source
        self.plain_named(qymcad_core::errors::Op::LoftBoolean, body, named.roles, |b| {
            let tool =
                crate::Shape::loft_sections_named(&named.data, offsets, places, walls, qymcad_core::feature::LoftBody::Solid, named.caps).ok_or_else(|| refused(qymcad_core::errors::Op::Loft))?;
            b.get(src)?.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::LoftBoolean))
        })
    }
    fn cylinder(&self, body: Id, r: f64, h: f64, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::primitives::ends_and_side(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Cylinder, body, roles, |_| crate::Shape::cylinder_named(r, h, names).ok_or_else(|| refused(qymcad_core::errors::Op::Cylinder)))
    }
    fn sphere(&self, body: Id, r: f64, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::primitives::ends_and_side(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Sphere, body, roles, |_| crate::Shape::sphere_named(r, names).ok_or_else(|| refused(qymcad_core::errors::Op::Sphere)))
    }
    fn cone(&self, body: Id, r1: f64, r2: f64, h: f64, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::primitives::ends_and_side(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Cone, body, roles, |_| crate::Shape::cone_named(r1, r2, h, names).ok_or_else(|| refused(qymcad_core::errors::Op::Cone)))
    }
    fn torus(&self, body: Id, major: f64, minor: f64, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = crate::ops::primitives::ends_and_side(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Torus, body, roles, |_| crate::Shape::torus_named(major, minor, names).ok_or_else(|| refused(qymcad_core::errors::Op::Torus)))
    }
    fn combine(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Boolean, body, |b| {
            let tool = Self::placed_extrude(profile, height, place).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
            b.get(src)?.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))
        })
    }
    fn combine_region(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::ExtrudeContour, body, |b| {
            let tool = placed(crate::Shape::extrude_profile(profile, height).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?, place)?;
            b.get(src)?.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))
        })
    }
    fn combine_region_multi(
        &self,
        bo: qymcad_core::model::BodyOp,
        profiles: &[Vec<f64>],
        span: qymcad_core::feature::ToolSpan,
        place: [f64; 12],
        sides: qymcad_core::feature::Sides<'_>,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::model::BodyOp { src, op, body } = bo;
        let named = crate::ops::profile::name_sides(profiles, sides, qymcad_core::names::Role::Wall);
        // the ends of a one-sided cut met with the body it cuts, then the tool placed where it starts
        let reach = {
            let shapes = self.shapes.borrow();
            crate::ops::profile::meet_ends(span, if src != 0 { shapes.get(&src) } else { None }, &place)
        };
        let (height, place) = (reach.length, crate::ops::profile::shifted(place, reach.start));
        let (profiles, caps) = (&named.profiles, &named.caps);
        // extrude every profile and fuse them into one tool, then one boolean against the source, giving one
        // body
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        // The profiles are fused into a single planar face by a 2D boolean before extruding, so that touching
        // contours give one body without seam edges; N prisms and N booleans used to leave doubled edges along
        // the line of contact, breaking chamfers and fillets. The fallback is the former path, contour by
        // contour.
        self.plain_named(qymcad_core::errors::Op::ExtrudeContour, body, named.roles.clone(), |b| {
            let tool = match crate::Shape::extrude_profiles_fused_named(profiles, height, caps) {
                Some(t) => t,
                None => {
                    let mut acc: Option<crate::Shape> = None;
                    for prof in profiles {
                        let e = crate::Shape::extrude_profile_named(prof, height, [caps.get(1).copied().unwrap_or(0), caps.get(2).copied().unwrap_or(0)])
                            .ok_or_else(|| refused(qymcad_core::errors::Op::ExtrudeContour))?;
                        acc = Some(match acc {
                            None => e,
                            Some(t) => t.boolean(&e, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
                        });
                    }
                    acc.ok_or(qymcad_core::errors::CoreError::NoContours)?
                }
            };
            let tool = placed(tool, place)?;
            // a new body: the fused tool itself
            if src == 0 {
                return Ok(tool);
            }
            let src_shape = b.get(src)?;
            let out = src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
            // The same as for a boolean of bodies: a cut by a contour lying clear of the part removed nothing
            // and said nothing — measured at an area of 1600.00 before and after, with no red nodes.
            // A cut that only touches the body is nothing either: a square on the top cut 5 up, away from the block,
            // took 0.1 mm^3 of a tool of 500 along the face it stood on and cut the top into 11 faces. Less than a
            // thousandth of the tool is a touch, not a cut.
            let removed = src_shape.volume() - out.volume();
            if op == 0 && src_shape.volume() > 1e-9 && removed <= (tool.volume().abs() * 1e-3).max(1e-6) {
                return Err(qymcad_core::errors::CoreError::CutRemovedNothing);
            }
            Ok(out)
        })
    }
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], roles: qymcad_core::feature::BlendRoles) -> Result<Built, qymcad_core::errors::CoreError> {
        self.fillet_op(crate::ops::blend::BlendAsk { body, src, size: radius, edges, roles })
    }
    fn fillet_at_vertices(
        &self,
        body: Id,
        src: Id,
        radius: f64,
        edges: &[u32],
        verts: &[qymcad_core::feature::RadiusAt],
        roles: qymcad_core::feature::BlendRoles,
    ) -> Result<Built, qymcad_core::errors::CoreError> {
        self.fillet_at_vertices_op(crate::ops::blend::BlendAsk { body, src, size: radius, edges, roles }, verts)
    }
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], roles: qymcad_core::feature::BlendRoles) -> Result<Built, qymcad_core::errors::CoreError> {
        self.chamfer_op(crate::ops::blend::BlendAsk { body, src, size: dist, edges, roles })
    }
    fn chamfer_ex(&self, body: Id, src: Id, d1: f64, shape: qymcad_core::model::ChamferShape, edges: &[u32], roles: qymcad_core::feature::BlendRoles) -> Result<Built, qymcad_core::errors::CoreError> {
        self.chamfer_asym_op(crate::ops::blend::BlendAsk { body, src, size: d1, edges, roles }, shape)
    }
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn helical(&self, h: qymcad_core::feature::Helical<'_>) -> Result<Built, qymcad_core::errors::CoreError> {
        // The profile has already been computed by the model kernel from the thread standard, as exact
        // segments and arcs; here there is only the sweep along the helix and the boolean — subtract for a
        // thread, unite for the flight of an auger.
        let mut roles = crate::door::Roles::default();
        let turns = crate::ops::holes::turns(h.feature, h.profile, h.starts, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Thread, h.body, roles, |b| {
            let s = b.get(h.src)?;
            let out = s
                .helical_profile(crate::HelicalCut {
                    axis: qymcad_core::feature::AxisLine { origin: h.origin, dir: h.dir },
                    radius: h.radius,
                    profile: h.profile,
                    length: h.length,
                    lead: h.lead,
                    starts: h.starts,
                    hand: if h.left { crate::Hand::Left } else { crate::Hand::Right },
                    kind: if h.fuse { crate::Helix::Rib } else { crate::Helix::Groove },
                    lead_in: h.lead_in,
                    lead_out: h.lead_out,
                    gnames: &turns.grooves,
                    rnames: &turns.reliefs,
                    crest_relief: h.crest_relief,
                })
                .ok_or(if h.fuse { qymcad_core::errors::CoreError::AugerFlightFailed } else { qymcad_core::errors::CoreError::ThreadFailed })?;
            // A GROOVE THAT CUT NOTHING, told by the exact volumes: the check on the meshes let 11546.04 -> 11546.04 through
            // as a thread, the tessellation noise passing for material taken.
            if !h.fuse && out.volume() >= s.volume() * (1.0 - 1e-7) {
                return Err(qymcad_core::errors::CoreError::ThreadRemovedNothing { before: s.volume(), after: out.volume() });
            }
            Ok(out)
        })
    }
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.shell_op(crate::ops::shell::ShellAsk { body, src, thickness, outward, faces }, &[], crate::door::Roles::default())
    }
    fn shell_named(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32], feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let walls = crate::ops::faces::shell_walls(feature, &self.face_ids(src), &mut roles);
        self.shell_op(crate::ops::shell::ShellAsk { body, src, thickness, outward, faces }, &walls, roles)
    }
    fn thicken_face(&self, body: Id, src: Id, face: u32, thickness: f64, join: Id, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let names = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let faces: Vec<u32> = s.face_places().iter().map(|p| p.id).collect();
            // the edges asked of the body itself, every one with a line and a name
            let edges: Vec<u32> = s.edges_info().into_iter().filter(|e| e.poly.len() >= 2 && e.id != 0).map(|e| e.id).collect();
            crate::ops::faces::thickened(feature, &faces, &edges, &mut roles)
        };
        self.thicken_op(crate::ops::shell::ThickenAsk { body, src, face, thickness, join }, &names, roles)
    }
    fn draft(&self, body: Id, src: Id, face_ids: &[u32], pull: qymcad_core::feature::DraftPull, neutral: qymcad_core::feature::PlaneAt, feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let qymcad_core::feature::DraftPull { angle, dir: pull } = pull;
        let qymcad_core::feature::PlaneAt { origin: np_origin, normal: np_normal } = neutral;
        let mut roles = crate::door::Roles::default();
        let sides = crate::ops::faces::draft_sides(feature, face_ids, &mut roles);
        // draft: tilting faces relative to a neutral plane, in real B-rep, with the faces keeping their ids
        self.plain_named(qymcad_core::errors::Op::Draft, body, roles, |b| {
            b.get(src)?.draft_faces(face_ids, angle, pull, np_origin, np_normal, &sides).ok_or(qymcad_core::errors::CoreError::DraftFailed { angle })
        })
    }
    fn shell_center(&self, body: Id, src: Id, thickness: f64, faces: &[u32]) -> Result<Built, qymcad_core::errors::CoreError> {
        // a wall centred on the surface: growth of +t/2 outward plus a hollow wall of −t
        self.plain(qymcad_core::errors::Op::ShellCenter, body, |b| {
            let s = b.get(src)?;
            s.shell_center(thickness.abs(), faces).ok_or_else(|| refused(qymcad_core::errors::Op::ShellCenter))
        })
    }
    fn push_face(&self, body: Id, src: Id, face: u32, dist: f64) -> Result<qymcad_core::geom::Built, qymcad_core::errors::CoreError> {
        // Pushing a face: a prism raised from the face itself, then a boolean. It follows the original
        // surface rather than its tessellation, which on a curved contour would give a polyline.
        self.plain(qymcad_core::errors::Op::PushFace, body, |b| {
            let s = b.get(src)?;
            s.push_face(face, dist).ok_or_else(|| refused(qymcad_core::errors::Op::PushFace))
        })
    }
    fn remove_faces(&self, body: Id, src: Id, face_ids: &[u32]) -> Result<qymcad_core::geom::Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::RemoveFaces, body, |b| b.get(src)?.remove_faces_why(face_ids).map_err(|why| qymcad_core::errors::CoreError::RemoveFacesFailed { why }))
    }
    fn split_faces(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::SplitFaces, body, |b| {
            let s = b.get(src)?;
            s.split_faces(origin, normal).ok_or_else(|| refused(qymcad_core::errors::Op::SplitFaces))
        })
    }
    fn split_body(&self, bodies: &[Id], src: Id, origin: [f64; 3], normal: [f64; 3], feature: Id) -> Result<Vec<Built>, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let section = crate::ops::copies::section(feature, &mut roles);
        // the words of a refusal are this split's (see `door::run`)
        crate::clear_kernel_refusal();
        let parts = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.split_by_plane(origin, normal, section).ok_or_else(|| refused(qymcad_core::errors::Op::SplitBody))?
        };
        // The pieces are ordered by their position along the normal rather than by the kernel's own order;
        // without that, moving the plane would swap which piece is which body and the references of later
        // features would slide onto the neighbouring half.
        let mut with_key: Vec<(f64, crate::Shape)> = parts
            .into_iter()
            .map(|sh| {
                // the centre of the bounding box along the normal: the pieces lie on opposite sides of the
                // plane, so this is enough and needs no mass properties
                let bb = sh.bbox().unwrap_or([0.0; 6]);
                let c = [(bb[0] + bb[3]) * 0.5, (bb[1] + bb[4]) * 0.5, (bb[2] + bb[5]) * 0.5];
                (c[0] * normal[0] + c[1] * normal[1] + c[2] * normal[2], sh)
            })
            .collect();
        with_key.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if with_key.len() != bodies.len() {
            // the timeline needs exactly the bodies it created; a mismatch is for the caller to resolve
            return Err(qymcad_core::errors::CoreError::SplitPieceCount { got: with_key.len(), want: bodies.len() });
        }
        with_key.into_iter().zip(bodies).map(|((_, sh), &b)| self.land_piece(crate::door::Piece { kind: qymcad_core::errors::Op::SplitBody, body: b, roles: roles.clone(), shape: sh })).collect()
    }
    fn hole(&self, body: Id, src: Id, tool: qymcad_core::model::HoleTool, pl: [f64; 12], feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        // a stepped hole — a cylinder plus a counterbore or countersink — as a real B-rep cut
        let mut roles = crate::door::Roles::default();
        let bore = crate::ops::holes::bore(feature, 0, &mut roles);
        let tip = crate::ops::holes::tool_faces(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Hole, body, roles, |b| {
            let s = b.get(src)?;
            s.hole_stepped_named(tool, pl, bore, &tip).ok_or_else(|| refused(qymcad_core::errors::Op::Hole))
        })
    }
    fn holes(&self, body: Id, src: Id, tool: qymcad_core::model::HoleTool, pls: &[[f64; 12]], sites: &[Id], feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        // many holes at sketch points in a single boolean cut: N tools fused, then subtracted
        let mut roles = crate::door::Roles::default();
        let bores: Vec<u32> = sites.iter().map(|site| crate::ops::holes::bore(feature, *site, &mut roles)).collect();
        let tip = crate::ops::holes::tool_faces(feature, &mut roles);
        self.plain_named(qymcad_core::errors::Op::Holes, body, roles, |b| {
            let s = b.get(src)?;
            s.holes_stepped_named(tool, pls, &bores, &tip).ok_or_else(|| refused(qymcad_core::errors::Op::Holes))
        })
    }
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]], feature: Id) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let seeds = crate::ops::copies::instances(feature, &self.face_ids(src), transforms.len(), &mut roles);
        self.plain_named(qymcad_core::errors::Op::Array, body, roles, |b| {
            let s = b.get(src)?;
            let mut copies: Vec<crate::Shape> = Vec::with_capacity(transforms.len());
            for (k, t) in transforms.iter().enumerate() {
                let copy = s.transformed(t).ok_or_else(|| refused(qymcad_core::errors::Op::Array))?;
                // The instance is named before the union. After it every copy carries the original numbers
                // and there is nothing left to tell them apart: ids are unique within one copy, not across
                // all of them.
                if let Some(pairs) = seeds.get(k) {
                    if !pairs.is_empty() {
                        copy.rename_faces(pairs);
                    }
                }
                copies.push(copy);
            }
            let mut it = copies.into_iter();
            let first = it.next().ok_or(qymcad_core::errors::CoreError::ArrayEmpty)?;
            let rest: Vec<crate::Shape> = it.collect();
            Ok(if rest.is_empty() {
                first
            } else {
                // One union rather than a chain of pairwise ones. The chain accumulated error: on a hollow
                // part with a conical wall the third fuse returned an empty shape, and on neighbouring
                // combinations the same union gave different volumes depending on the order of the operands
                // — 3954.2 against 4008.1 — although union is commutative.
                //
                // An empty fuse is not a result but lost copies. An empty shape used to become the
                // accumulator of the chain silently: four copies over 360° gave a green node with a volume
                // of 2078 instead of about 8000 — three copies of four had vanished and the timeline said
                // nothing. That answer is worse than a red node, because nobody sees it.
                let mut all: Vec<&crate::Shape> = Vec::with_capacity(rest.len() + 1);
                all.push(&first);
                all.extend(rest.iter());
                let fused = crate::Shape::fuse_many(&all).ok_or_else(|| refused(qymcad_core::errors::Op::Array))?;
                if fused.is_empty_shape() {
                    return Err(qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::Array));
                }
                fused
            })
        })
    }
    fn mirror(&self, body: Id, src: Id, plane: u8, made: qymcad_core::feature::Mirrored) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let seed = crate::ops::copies::image(made.names, &self.face_ids(src), &mut roles);
        self.plain_named(qymcad_core::errors::Op::Mirror, body, roles, |b| {
            let s = b.get(src)?;
            let m = s.mirrored(plane).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
            if !seed.is_empty() {
                m.rename_faces(&seed); // the image is named before it is united with the original
            }
            Ok(if made.keeps == qymcad_core::feature::MirrorKeeps::Both {
                // unite with a copy of the original, which must not be moved out of the cache
                let orig = s.transformed(&qymcad_core::feature::PLACE_IDENTITY).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
                orig.boolean(&m, 1).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?
            } else {
                m
            })
        })
    }
    fn mirror_plane(&self, body: Id, src: Id, at: qymcad_core::feature::PlaneAt, made: qymcad_core::feature::Mirrored) -> Result<Built, qymcad_core::errors::CoreError> {
        let mut roles = crate::door::Roles::default();
        let seed = crate::ops::copies::image(made.names, &self.face_ids(src), &mut roles);
        let keep = made.keeps == qymcad_core::feature::MirrorKeeps::Both;
        let work = |b: &crate::door::Bodies| {
            let s = b.get(src)?;
            let m = s.mirrored_plane(at.origin, at.normal).ok_or_else(|| refused(qymcad_core::errors::Op::MirrorPlane))?;
            if !seed.is_empty() {
                m.rename_faces(&seed); // the image is named before it is united with the original
            }
            if keep {
                let orig = s.transformed(&qymcad_core::feature::PLACE_IDENTITY).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
                orig.boolean(&m, 1).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))
            } else {
                Ok(m)
            }
        };
        // A refusal names its cause. Mirroring a hollow part about one of its own faces is beyond the
        // kernel: fusing the halves leaves extra shells and no repair resolves them, tried five ways. While
        // that holds, the cause and the way out have to be visible instead of an unexplained red node.
        self.plain_named(qymcad_core::errors::Op::MirrorPlane, body, roles, work).map_err(|e| {
            if keep && matches!(e, qymcad_core::errors::CoreError::BrokenSolid) {
                qymcad_core::errors::CoreError::MirrorOfHollowBody
            } else {
                e
            }
        })
    }
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::Transform, body, |b| b.get(src)?.transformed(&mat).ok_or_else(|| refused(qymcad_core::errors::Op::Move)))
    }
    fn land_import(&self, body: Id) -> Option<qymcad_core::feature::ImportLanded> {
        self.land_as_it_came(body)
    }
    fn tessellate(&self, body: Id) -> Option<Built> {
        // re-tessellation of an already imported STEP shape, through the same `tessellate_merged`
        self.shapes.borrow().get(&body).and_then(|s| s.tessellate_merged_auto(self.k()))
        // deflection from the size
    }
    fn body_boolean(&self, body: Id, a: Id, b: Id, op: u8) -> Result<Built, qymcad_core::errors::CoreError> {
        self.plain(qymcad_core::errors::Op::BodyBoolean, body, |bodies| {
            let sa = bodies.get(a).map_err(|_| qymcad_core::errors::CoreError::BodyANotBuilt)?;
            let sb = bodies.get(b).map_err(|_| qymcad_core::errors::CoreError::BodyBNotBuilt)?;
            // A cut that removed nothing is worse than a refusal. Measured: a tool not intersecting the
            // base left the area and volume unchanged, the node stood green, and the tool was consumed all
            // the same - a body lost with nothing given back.
            match sa.boolean_checked(sb, op) {
                Ok(out) => Ok(out),
                Err(crate::BoolVerdict::CutRemovedNothing) => Err(qymcad_core::errors::CoreError::CutRemovedNothing),
                Err(crate::BoolVerdict::NothingInCommon) => Err(qymcad_core::errors::CoreError::EmptyResult),
                Err(_) => Err(refused(qymcad_core::errors::Op::BodyBoolean)),
            }
        })
    }
}
/// THE KERNEL'S OWN WORDS SURVIVE LONG ENOUGH TO BE REPORTED.
///
/// Before this they went to `stderr` and were cleared in the same breath. Three things then had to line
/// up for a person to ever see them: a terminal (a packaged Windows build has none), the same thread
/// (the work runs on a background one), and reading them within the same call. None of that is true when
/// somebody decides, minutes later, to report the trouble.
#[cfg(test)]
mod report_tests {
    use qymcad_core::errors::Op;

    #[test]
    fn a_refusal_outlives_the_error_it_became() {
        crate::clear_kernel_refusal();
        // A guard on the Rust side speaks into the same channel as OCCT does, so it stands in here for a
        // real failure without needing geometry that fails.
        assert!(crate::refuse("shell/asked", "a wall of zero thickness").is_none());

        let e = super::refused(Op::Shell);
        assert!(matches!(e, qymcad_core::errors::CoreError::OpFailed(Op::Shell)), "the coded error travels on unchanged: {e:?}");

        // Cleared for the NEXT failure - an old refusal must never be offered as a new reason...
        assert_eq!(crate::last_kernel_refusal(), None, "the channel was not cleared for the next failure");

        // ...and kept for the report all the same.
        let kept = crate::refusal_for_report().expect("the refusal was kept for a report");
        assert!(kept.contains("Shell"), "the kept refusal does not name the operation: {kept}");
        assert!(kept.contains("a wall of zero thickness"), "the kept refusal lost the kernel's words: {kept}");
    }
}
