//! The single implementation of the geometric kernel of the model (`qymcad_core::feature::Kernel`) over OCCT.
//!
//! This code used to live as two copies, one in the application and one in the reproduction harness: 34 methods,
//! 445 and 450 lines, 99.5 per cent identical text. The copies had drifted apart — the harness the tests run
//! against tessellated at a fixed deflection of 0.5 while the application used one adaptive to the size of the
//! body. A green test on the volume or area of a mesh therefore did not prove that the same thing appeared on
//! screen, and an edit to the kernel in one copy silently failed to reach the other. There is now one
//! implementation for everybody.
#![allow(clippy::too_many_arguments)]
use qymcad_core::geom::{Mesh, MeshFace};
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
fn refused(op: qymcad_core::errors::Op) -> qymcad_core::errors::CoreError {
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
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

impl OcctKernel {
    /// The fraction of the diagonal. A zero means the record was written without a tolerance, so the ordinary
    /// one is taken: the very value the program lived by before the setting existed.
    fn k(&self) -> f64 {
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
    /// Tessellate the result and cache its shape under `body`, returning the mesh and faces of the first body.
    fn finish(&self, body: Id, mut shape: crate::Shape) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // Merge every solid into one mesh: a pattern, a mirror together with its original, or a cut that fell
        // apart is a compound of several bodies, while a timeline node is one body. Taking only the first solid
        // made a pattern or a mirror show a single copy.
        //
        // The tessellation deflection adapts to the size of the body; a fixed 0.5 mm left small parts faceted
        // and huge ones unmanageable.
        // A gate: a broken body does not become a part.
        //
        // The result of every kernel operation passes through this funnel, and until now it was accepted
        // silently, sound or not. The price showed in use: pushing a face returned a body with inconsistent
        // face orientations, the timeline said nothing, and everything after it fell apart. On screen there was
        // a hole instead of a wall, the renderer showing the inside; the volume read 160767 instead of 20019,
        // the integral having lost its signs; and the next operation landed on an unnamed face of that corpse.
        //
        // That particular failure was cured in the operation itself, in the order of the boolean operands, but
        // the gate stays: any operation can return an unsound result and accepting it silently is not
        // acceptable. The check lives here rather than in each of the 41 operations — one place nothing gets
        // past.
        // A face lying in two places is not a face. A fillet or a chamfer can eat a face down to zero width:
        // the inner contour meets the outer one, the remainder lies as two patches, and both carry the same id.
        // It showed while pushing a face: clicking one end lifted the opposite one too. It is resolved here in
        // the shared funnel, since any operation can leave such a face behind.
        shape.heal_pinched_faces();
        // Emptiness is asked about first. An empty shape fails the validity check, so a broken body was
        // reported where in fact the operation produced nothing at all. Those are different faults calling for
        // different actions: a broken result is repaired, an empty one is redone. The order of the checks is
        // the answer.
        if shape.is_empty_shape() {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        // Whether the part fell into pieces is deliberately not asked here, tempting as it was: a pattern and
        // a mirror legitimately give a compound of several bodies, and a blanket ban breaks them. The
        // `OperationSplitBody` error stays for the operations where several bodies really are a fault.
        // An unsound shape is repaired before being rejected. The kernel sometimes returns a body with a minor
        // flaw: the check rejects it and a broken body is reported where the part is essentially correct and
        // repairs with a standard tool. A red node is left only for what could not be repaired.
        if !shape.is_valid() {
            match shape.healed() {
                Some(fixed) => shape = fixed,
                None => return Err(qymcad_core::errors::CoreError::BrokenSolid),
            }
        }
        // Emptiness is not a part either. On the same body, pushing an inner wall gave a formally sound result
        // of zero volume: the part disappeared and the timeline said nothing. A negative volume means a shell
        // turned inside out, which is the same corpse.
        //
        // A sheet, however, has no volume by nature. Judging a surface by a zero volume would keep every
        // surface out of the document, that is, close the whole design layer at the door. So what the shape is
        // gets asked first: a solid is measured by volume, a sheet by whether it exists at all.
        if shape.is_empty_shape() {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        if !shape.is_sheet() && shape.volume() <= 1e-9 {
            return Err(qymcad_core::errors::CoreError::EmptyResult);
        }
        let out = shape.tessellate_merged_auto(self.k()).ok_or(qymcad_core::errors::CoreError::EmptyResult)?;
        self.shapes.borrow_mut().insert(body, shape);
        Ok(out)
    }
}

impl qymcad_core::feature::Kernel for OcctKernel {
    fn extrude(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let s = Self::placed_extrude(profile, height, place).ok_or_else(|| refused(qymcad_core::errors::Op::Extrude))?;
        self.finish(body, s)
    }
    fn revolve(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let mut s = crate::Shape::revolve(profile, axis, angle_deg).ok_or_else(|| refused(qymcad_core::errors::Op::Revolve))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            s = s.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        self.finish(body, s)
    }
    fn boolean(&self, body: Id, base: &[f64], base_h: f64, tool: &[f64], tool_h: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let b = crate::Shape::extrude(base, base_h).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
        let t = crate::Shape::extrude(tool, tool_h).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
        let mut res = b.boolean(&t, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            res = res.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        self.finish(body, res)
    }
    fn copy_faces(&self, body: Id, src: Id, faces: &[u32], names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.copy_faces(faces, names).ok_or_else(|| refused(qymcad_core::errors::Op::CopyFaces))?
        };
        self.finish(body, res)
    }
    fn patch(&self, body: Id, src: Id, edges: &[u32], tangent: bool, name: u32) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.patch(edges, tangent, name).ok_or_else(|| refused(qymcad_core::errors::Op::Patch))?
        };
        self.finish(body, res)
    }
    fn replace_faces(&self, body: Id, src: Id, faces: &[u32], surface: Id) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let base = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let surf = shapes.get(&surface).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            // If it did not close, say how many edges were left free. An unnamed "the operation failed" is
            // useless here: confusing the boundary of an opening is easy — the ring of an end face against the
            // whole hole — and the message gave no way to tell that was the problem.
            base.replace_faces(faces, surf, 1e-4).map_err(|free| match free {
                0 => qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::ReplaceFaces),
                n => qymcad_core::errors::CoreError::SurfaceDoesNotClose { free: n },
            })?
        };
        self.finish(body, res)
    }
    fn stitch(&self, body: Id, parts: &[Id], tol: f64) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let mut got: Vec<&crate::Shape> = Vec::with_capacity(parts.len());
            for p in parts {
                got.push(shapes.get(p).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?);
            }
            // Nothing to stitch is not "the operation failed". The kernel stitches along shared edges; if
            // none joined, the surfaces simply do not touch, and that is what has to be said.
            crate::Shape::stitch(&got, tol).map_err(|()| qymcad_core::errors::CoreError::StitchNothingJoined)?
        };
        self.finish(body, res)
    }
    fn trim(&self, body: Id, src: Id, tool: Id, keep: [f64; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let t = shapes.get(&tool).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.trim(t, keep).ok_or_else(|| refused(qymcad_core::errors::Op::Trim))?
        };
        self.finish(body, res)
    }
    fn body_is_sheet(&self, body: Id) -> bool {
        self.shapes.borrow().get(&body).is_some_and(|s| s.is_sheet())
    }
    fn edge_end_faces(&self, body: Id) -> Vec<(u32, u32, u32)> {
        self.shapes.borrow().get(&body).map(|s| s.edge_end_faces()).unwrap_or_default()
    }
    fn absorbed_names(&self, body: Id) -> Vec<(u32, u32)> {
        self.shapes.borrow().get(&body).map(|s| s.absorbed_names()).unwrap_or_default()
    }
    fn clear_face_splits(&self, body: Id) {
        if let Some(s) = self.shapes.borrow().get(&body) {
            s.clear_face_splits();
        }
    }
    fn face_splits(&self, body: Id) -> Vec<(u32, u32, u32)> {
        self.shapes.borrow().get(&body).map(|s| s.face_splits()).unwrap_or_default()
    }
    fn rename_faces(&self, body: Id, pairs: &[(u32, u32)]) {
        if let Some(s) = self.shapes.borrow().get(&body) {
            s.rename_faces(pairs);
        }
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
    fn edge_face_pairs(&self, body: Id) -> Vec<(u32, u32, u32)> {
        self.shapes.borrow().get(&body).map(|s| s.edge_face_pairs()).unwrap_or_default()
    }
    fn rename_edges(&self, body: Id, pairs: &[(u32, u32)]) {
        if let Some(s) = self.shapes.borrow().get(&body) {
            s.rename_edges(pairs);
        }
    }
    fn edges(&self, body: Id) -> Vec<qymcad_core::geom::MeshEdge> {
        // The edges of the built body: a persistent id, the midpoint along the polyline, and the tangent. The
        // geometry comes from the existing `Shape::edges_with_ids` through the FFI; no new C++ is needed.
        let shapes = self.shapes.borrow();
        let Some(s) = shapes.get(&body) else {
            return Vec::new();
        };
        s.edges_info()
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
                let (center, axis, radius) = circle.map(|(c, a, r)| (c, a, r)).unwrap_or_default();
                // The secondary axis of a connector is the normal of the adjacent face. A zero vector honestly
                // means there is no adjacent face: the roll about such an edge is undefined, and the solver
                // leaves it free rather than inventing an orientation.
                let ref_dir = e.ref_dir.unwrap_or_default();
                Some(qymcad_core::geom::MeshEdge { id, mid, dir, a: p(0), b: p(poly.len() - 1), center, axis, radius, ref_dir })
            })
            .collect()
    }
    fn face_axis(&self, body: Id, face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        // the axis of a cylindrical or conical face by its persistent id, through the same FFI used when
        // picking the axis of a pattern
        self.shapes.borrow().get(&body).and_then(|s| s.face_axis(face_id))
    }
    fn extrude_region(&self, body: Id, profile: &[f64], height: f64, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // an exact profile, with contours from real edges, plus holes in one face, giving an exact body
        let mut s = crate::Shape::extrude_profile(profile, height).ok_or_else(|| refused(qymcad_core::errors::Op::ExtrudeProfile))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            s = s.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        self.finish(body, s)
    }
    fn revolve_region(&self, body: Id, profile: &[f64], axis: u8, angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // the X or Y axis of a sketch is a special case of a general axis and takes the same path, so that
        // face names are derived from the recipe rather than from the traversal order
        let (o, d) = ([0.0, 0.0, 0.0], if axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] });
        let mut s = crate::Shape::revolve_profile_axis_named(profile, o, d, angle_deg, caps).ok_or_else(|| refused(qymcad_core::errors::Op::RevolveProfile))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            s = s.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        self.finish(body, s)
    }
    fn revolve_region_axis(&self, body: Id, profile: &[f64], origin: [f64; 3], dir: [f64; 3], angle_deg: f64, place: [f64; 12], caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // revolving about an arbitrary local axis, then placing the sketch
        let mut s = crate::Shape::revolve_profile_axis_named(profile, origin, dir, angle_deg, caps).ok_or_else(|| refused(qymcad_core::errors::Op::RevolveAxis))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            s = s.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        self.finish(body, s)
    }
    fn sweep(&self, body: Id, profile: &[f64], profile_place: [f64; 12], path: &[f64], path_place: [f64; 12], caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // a profile placed on its own plane is swept along a path on its own plane, giving a body
        let s = crate::Shape::sweep_profile_named(profile, &profile_place, path, &path_place, caps)
            .ok_or_else(|| refused(qymcad_core::errors::Op::Sweep))?;
        self.finish(body, s)
    }
    fn revolve_region_multi(
        &self,
        body: Id,
        src: Id,
        profiles: &[Vec<f64>],
        axis: u8,
        origin_dir: Option<([f64; 3], [f64; 3])>,
        angle_deg: f64,
        place: [f64; 12],
        op: u8,
        caps: &[u32],
    ) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // revolve every profile, fuse them into one tool, then take a single boolean against the source
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        let (o, d) = origin_dir.unwrap_or(([0.0, 0.0, 0.0], if axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }));
        let fail = qymcad_core::errors::CoreError::OpFailed(if origin_dir.is_some() { qymcad_core::errors::Op::RevolveAxis } else { qymcad_core::errors::Op::RevolveProfile });
        let mut tool: Option<crate::Shape> = None;
        for (i, prof) in profiles.iter().enumerate() {
            // each region gets its own cap names; see `region_cap_names`
            let cap = [caps.get(i * 3 + 1).copied().unwrap_or(0), caps.get(i * 3 + 2).copied().unwrap_or(0)];
            let s = crate::Shape::revolve_profile_axis_named(prof, o, d, angle_deg, cap).ok_or(fail.clone())?;
            tool = Some(match tool {
                None => s,
                Some(t) => t.boolean(&s, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
            });
        }
        let mut tool = tool.expect("the profiles are not empty, checked above");
        if place != qymcad_core::feature::PLACE_IDENTITY {
            tool = tool.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        if src == 0 {
            return self.finish(body, tool); // a new body: the fused tool itself
        }
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?
        };
        self.finish(body, res)
    }
    fn sweep_multi(
        &self,
        body: Id,
        src: Id,
        profiles: &[Vec<f64>],
        profile_place: [f64; 12],
        path: &[f64],
        path_place: [f64; 12],
        op: u8,
        caps: &[u32],
    ) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        let mut tool: Option<crate::Shape> = None;
        for (i, prof) in profiles.iter().enumerate() {
            let cap = [caps.get(i * 3 + 1).copied().unwrap_or(0), caps.get(i * 3 + 2).copied().unwrap_or(0)];
            let s = crate::Shape::sweep_profile_named(prof, &profile_place, path, &path_place, cap)
                .ok_or_else(|| refused(qymcad_core::errors::Op::Sweep))?;
            tool = Some(match tool {
                None => s,
                Some(t) => t.boolean(&s, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
            });
        }
        let tool = tool.expect("the profiles are not empty, checked above");
        if src == 0 {
            return self.finish(body, tool);
        }
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?
        };
        self.finish(body, res)
    }
    fn loft(&self, body: Id, sections: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, kind: qymcad_core::feature::LoftBody, caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // a body through a set of sections, each on its own plane
        let s = crate::Shape::loft_sections_named(sections, offsets, places, walls, kind, caps)
            .ok_or_else(|| refused(qymcad_core::errors::Op::Loft))?;
        self.finish(body, s)
    }
    fn loft_combine(&self, body: Id, src: Id, sections: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, op: u8, caps: [u32; 2]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // a lofted solid used as a tool, a closed body, taken in a boolean against the source
        let tool = crate::Shape::loft_sections_named(sections, offsets, places, walls, qymcad_core::feature::LoftBody::Solid, caps)
            .ok_or_else(|| refused(qymcad_core::errors::Op::Loft))?;
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::LoftBoolean))?
        };
        self.finish(body, res)
    }
    fn cylinder(&self, body: Id, r: f64, h: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.finish(body, crate::Shape::cylinder_named(r, h, names).ok_or_else(|| refused(qymcad_core::errors::Op::Cylinder))?)
    }
    fn sphere(&self, body: Id, r: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.finish(body, crate::Shape::sphere_named(r, names).ok_or_else(|| refused(qymcad_core::errors::Op::Sphere))?)
    }
    fn cone(&self, body: Id, r1: f64, r2: f64, h: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.finish(body, crate::Shape::cone_named(r1, r2, h, names).ok_or_else(|| refused(qymcad_core::errors::Op::Cone))?)
    }
    fn torus(&self, body: Id, major: f64, minor: f64, names: [u32; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.finish(body, crate::Shape::torus_named(major, minor, names).ok_or_else(|| refused(qymcad_core::errors::Op::Torus))?)
    }
    fn combine(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let tool = Self::placed_extrude(profile, height, place).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?
        };
        self.finish(body, res)
    }
    fn combine_region(&self, body: Id, src: Id, profile: &[f64], height: f64, op: u8, place: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let mut tool = crate::Shape::extrude_profile(profile, height).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
        if place != qymcad_core::feature::PLACE_IDENTITY {
            tool = tool.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?
        };
        self.finish(body, res)
    }
    fn combine_region_multi(&self, body: Id, src: Id, profiles: &[Vec<f64>], height: f64, op: u8, place: [f64; 12], caps: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // extrude every profile and fuse them into one tool, then one boolean against the source, giving one
        // body
        if profiles.is_empty() {
            return Err(qymcad_core::errors::CoreError::NoContours);
        }
        // The profiles are fused into a single planar face by a 2D boolean before extruding, so that touching
        // contours give one body without seam edges; N prisms and N booleans used to leave doubled edges along
        // the line of contact, breaking chamfers and fillets. The fallback is the former path, contour by
        // contour.
        let mut tool = match crate::Shape::extrude_profiles_fused_named(profiles, height, caps) {
            Some(t) => t,
            None => {
                let mut acc: Option<crate::Shape> = None;
                for prof in profiles {
                    let e = crate::Shape::extrude_profile_named(prof, height, [caps.get(1).copied().unwrap_or(0), caps.get(2).copied().unwrap_or(0)]).ok_or_else(|| refused(qymcad_core::errors::Op::ExtrudeContour))?;
                    acc = Some(match acc {
                        None => e,
                        Some(t) => t.boolean(&e, 1).ok_or_else(|| refused(qymcad_core::errors::Op::FuseProfiles))?,
                    });
                }
                acc.unwrap()
            }
        };
        if place != qymcad_core::feature::PLACE_IDENTITY {
            tool = tool.transformed(&place).ok_or_else(|| refused(qymcad_core::errors::Op::Place))?;
        }
        if src == 0 {
            return self.finish(body, tool); // a new body: the fused tool itself
        }
        let res = {
            let shapes = self.shapes.borrow();
            let src_shape = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let out = src_shape.boolean(&tool, op).ok_or_else(|| refused(qymcad_core::errors::Op::Boolean))?;
            // The same as for a boolean of bodies: a cut by a contour lying clear of the part removed nothing
            // and said nothing — measured at an area of 1600.00 before and after, with no red nodes.
            if op == 0 && src_shape.volume() > 1e-9 && out.volume() >= src_shape.volume() - 1e-6 {
                return Err(qymcad_core::errors::CoreError::CutRemovedNothing);
            }
            out
        };
        self.finish(body, res)
    }
    fn fillet(&self, body: Id, src: Id, radius: f64, edges: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            // Smooth edges — the tangent junctions of earlier fillets, with a dihedral angle near 180° —
            // have nothing left to round, and the kernel honestly fails on them. They are filtered out, as
            // elsewhere in the trade; the sharp ones are rounded.
            let sm = s.smooth_edge_ids();
            let edges: Vec<u32> = edges.iter().copied().filter(|e| !sm.contains(e)).collect();
            if edges.is_empty() && !sm.is_empty() {
                return Err(qymcad_core::errors::CoreError::AllEdgesSmooth);
            }
            let edges = &edges[..];
            let mut r = if edges.is_empty() { s.fillet_all(radius) } else { s.fillet_edges_named(radius, edges, names, corners, all_names) };
            // The kernel fails exactly at the boundary of degeneracy — a radius equal to half a face: on a
            // 10 mm cube with r = 5 the face collapses to nothing, while 4.9999999 works. A retry with a
            // step back of about 2e-8, invisible in the geometry, builds the limiting case the way mature
            // systems do.
            for f in [1.0 - 2e-8, 1.0 - 1e-5] {
                if r.is_some() {
                    break;
                }
                let r2 = radius * f;
                r = if edges.is_empty() { s.fillet_all(r2) } else { s.fillet_edges_named(r2, edges, names, corners, all_names) };
            }
            // Honest diagnostics: if the group did not take, each edge is tried on its own and the answer
            // says which edges refuse this radius and which radius they do accept.
            if r.is_none() && !edges.is_empty() {
                let mut bad: Vec<qymcad_core::errors::FilletEdgeIssue> = Vec::new();
                for &e in edges.iter() {
                    if s.fillet_edges_named(radius, &[e], names, corners, all_names).is_some() {
                        continue;
                    }
                    // What exactly is wrong with this edge, told with data rather than a phrase: the
                    // largest radius it does accept, or none at all. The wording is the application's job.
                    let mut takes_up_to = None;
                    for f in [0.5, 0.25, 0.1] {
                        if s.fillet_edges_named(radius * f, &[e], names, corners, all_names).is_some() {
                            takes_up_to = Some(radius * f);
                            break;
                        }
                    }
                    bad.push(qymcad_core::errors::FilletEdgeIssue { edge: e, takes_up_to });
                }
                if !bad.is_empty() {
                    // Automatic partial success: the bad edges are dropped and the rest are rounded, so
                    // the part gets as much as is possible instead of one wholly red node.
                    let bad_ids: Vec<u32> = bad.iter().map(|b| b.edge).collect();
                    let good: Vec<u32> = edges.iter().copied().filter(|e| !bad_ids.contains(e)).collect();
                    if !good.is_empty() {
                        r = s.fillet_edges_named(radius, &good, names, corners, all_names);
                    }
                    if r.is_none() && good.len() > 1 {
                        // One at a time: the ids of untouched edges survive a fillet through
                        // `propagate_ids`, and while the group fails at once, edge by edge on top of the
                        // running result it often builds
                        let mut acc: Option<crate::Shape> = None;
                        let mut done = 0usize;
                        for &e in &good {
                            let base = acc.as_ref().unwrap_or(s);
                            if let Some(ns) = base.fillet_edges_named(radius, &[e], names, corners, all_names) {
                                acc = Some(ns);
                                done += 1;
                            }
                            // it did not take on top of what has accumulated, the rounds conflicting, so
                            // it is skipped and the maximum is built
                        }
                        if done > 0 {
                            r = acc;
                        }
                    }
                    if r.is_none() {
                        return Err(qymcad_core::errors::CoreError::FilletRadiusTooBig { radius, issues: bad.clone(), smooth_skipped: sm.len() });
                    }
                }
                if r.is_none() {
                    // the edges take individually but not together: neighbouring fillets intersect
                    return Err(qymcad_core::errors::CoreError::FilletEdgesOneByOne { radius });
                }
            }
            r.ok_or_else(|| refused(qymcad_core::errors::Op::Fillet))?
        };
        self.finish(body, res)
    }
    fn fillet_at_vertices(&self, body: Id, src: Id, radius: f64, edges: &[u32], verts: &[([f64; 3], f64)]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The radius is given at vertices, and an edge takes the radii of its ends. The match tolerance is
        // 1e-6: the point comes from a live rebuild of the same body, that is from the same geometry, so
        // "nearly the same vertex" here would mean landing on the neighbouring one.
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.fillet_at_vertices(radius, edges, verts, 1e-6).ok_or_else(|| refused(qymcad_core::errors::Op::FilletVar))?
        };
        self.finish(body, res)
    }
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn helical(&self, h: qymcad_core::feature::Helical<'_>) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // The profile has already been computed by the model kernel from the thread standard, as exact
        // segments and arcs; here there is only the sweep along the helix and the boolean — subtract for a
        // thread, unite for the flight of an auger.
        let shapes = self.shapes.borrow();
        let s = shapes.get(&h.src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
        let out = s
            .helical_profile(h.origin, h.dir, h.radius, h.profile, h.length, h.lead, h.starts, if h.left { crate::Hand::Left } else { crate::Hand::Right }, if h.fuse { crate::Helix::Rib } else { crate::Helix::Groove }, h.lead_in, h.lead_out, h.gnames, h.rnames, h.crest_relief)
            .ok_or(if h.fuse { qymcad_core::errors::CoreError::AugerFlightFailed } else { qymcad_core::errors::CoreError::ThreadFailed })?;
        drop(shapes);
        self.finish(h.body, out)
    }
    fn chamfer(&self, body: Id, src: Id, dist: f64, edges: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            // Smooth edges — the tangent junctions of earlier fillets, with a dihedral angle near 180° —
            // have nothing left to cut, and the kernel honestly fails on them. They are filtered out, as
            // elsewhere in the trade; the sharp ones are chamfered.
            let sm = s.smooth_edge_ids();
            let edges: Vec<u32> = edges.iter().copied().filter(|e| !sm.contains(e)).collect();
            if edges.is_empty() && !sm.is_empty() {
                return Err(qymcad_core::errors::CoreError::AllEdgesSmooth);
            }
            let edges = &edges[..];
            let mut r = if edges.is_empty() { s.chamfer_all(dist) } else { s.chamfer_edges(dist, edges, names, corners, all_names) };
            // The kernel fails exactly at the boundary of degeneracy, where the leg equals the size of the
            // face; a retry stepped back builds the limiting case. A chamfer needs a coarser step than a
            // fillet: 1.99999999 failed where 1.99998 worked, so about 1e-5.
            for f in [1.0 - 2e-8, 1.0 - 1e-5] {
                if r.is_some() {
                    break;
                }
                let d2 = dist * f;
                r = if edges.is_empty() { s.chamfer_all(d2) } else { s.chamfer_edges(d2, edges, names, corners, all_names) };
            }
            r.ok_or(qymcad_core::errors::CoreError::ChamferTooBig { dist })?
        };
        self.finish(body, res)
    }
    fn chamfer_ex(&self, body: Id, src: Id, d1: f64, d2: f64, mode: qymcad_core::feature::ChamferMode, flip: bool, ref_face: u32, edges: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        use qymcad_core::feature::ChamferMode;
        // asymmetry applies only to selected edges; for "all edges" it falls back to the symmetric form
        if edges.is_empty() || mode == ChamferMode::Symmetric {
            return self.chamfer(body, src, d1, edges, &[], &[], &[]);
        }
        let m = if mode == ChamferMode::DistAngle { 2 } else { 1 };
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.chamfer_edges_asym(d1, d2, m, flip, ref_face, edges).ok_or_else(|| refused(qymcad_core::errors::Op::ChamferAsym))?
        };
        self.finish(body, res)
    }
    fn shell(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.shell_named(body, src, thickness, outward, faces, &[])
    }
    fn shell_named(&self, body: Id, src: Id, thickness: f64, outward: bool, faces: &[u32], walls: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let offset = if outward { thickness.abs() } else { -thickness.abs() }; // the sign is the direction
            // There is nothing to remove: the face reference matched none, so the kernel is never reached,
            // yet an unnamed "the operation failed" was reported. That is not a broken shell but a lost
            // reference, and it has to be named as one.
            if faces.is_empty() {
                return Err(qymcad_core::errors::CoreError::FacesNotFound);
            }
            // A refusal has to name its cause. An offset thicker than the smallest fillet eats it whole and
            // the kernel will not build such a shell; that is what has to be said, not "it failed".
            // A shell that hollowed nothing is not a shell. Measured on a 30×18×12 box: at a thickness of
            // 12 the kernel returned the source body — six faces, area 2232.00, exactly the blank — and the
            // node stood green, while at a thickness of 9 the same thing arrived with an honest
            // `BrokenSolid`. The report said the shell was built and the part stayed solid. So the volume is
            // asked for: a hollow part has strictly less of it than a solid one.
            let vol_was = s.volume();
            match s.shell(offset, faces, walls) {
                Some(v) if !outward && vol_was > 1e-9 && v.volume() >= vol_was - 1e-6 => {
                    let r = s.min_round_radius();
                    let t = thickness.abs();
                    return Err(if r > 1e-9 && t >= r - 1e-9 {
                        qymcad_core::errors::CoreError::ShellThicknessOverRound { thickness: t, limit: r }
                    } else {
                        qymcad_core::errors::CoreError::ShellNotBuiltHere
                    });
                }
                Some(v) => v,
                None => {
                    // Try first, then judge. The check for a body of several shells used to stand before
                    // the attempt and kept the kernel from even starting, though it can shell each body
                    // separately. A refusal is named only after a real failure.
                    let shells = s.shell_count();
                    let r = s.min_round_radius();
                    let t = thickness.abs();
                    return Err(if r > 1e-9 && t >= r - 1e-9 {
                        qymcad_core::errors::CoreError::ShellThicknessOverRound { thickness: t, limit: r }
                    } else if shells > 1 {
                        qymcad_core::errors::CoreError::ShellOfMultiShellBody { shells }
                    } else {
                        qymcad_core::errors::CoreError::ShellNotBuiltHere
                    });
                }
            }
        };
        self.finish(body, res)
    }
    fn shell_center(&self, body: Id, src: Id, thickness: f64, faces: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // a wall centred on the surface: growth of +t/2 outward plus a hollow wall of −t
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.shell_center(thickness.abs(), faces).ok_or_else(|| refused(qymcad_core::errors::Op::ShellCenter))?
        };
        self.finish(body, res)
    }
    fn push_face(&self, body: Id, src: Id, face: u32, dist: f64) -> Result<(qymcad_core::geom::Mesh, Vec<qymcad_core::geom::MeshFace>), qymcad_core::errors::CoreError> {
        // Pushing a face: a prism raised from the face itself, then a boolean. It follows the original
        // surface rather than its tessellation, which on a curved contour would give a polyline.
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.push_face(face, dist)
                .ok_or_else(|| refused(qymcad_core::errors::Op::PushFace))?
        };
        self.finish(body, res)
    }
    fn remove_faces(&self, body: Id, src: Id, face_ids: &[u32]) -> Result<(qymcad_core::geom::Mesh, Vec<qymcad_core::geom::MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.remove_faces_why(face_ids).map_err(|why| qymcad_core::errors::CoreError::RemoveFacesFailed { why })?
        };
        self.finish(body, res)
    }
    fn thicken_face(&self, body: Id, src: Id, face: u32, thickness: f64, join: Id, fmap: &[u32], emap: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            // A refusal names itself. Both possible failures used to look the same, as "the operation
            // failed", leaving no way to tell an excessive thickness from a plate that did not join.
            let plate = s.thicken_face_join(face, thickness, fmap, emap).ok_or(qymcad_core::errors::CoreError::ThickenFaceRefused)?;
            // The plate returns into the part the surface was taken from. Otherwise a second body stays in
            // the part — a differently coloured piece on screen — breaking the rule that a part is one body.
            match join {
                0 => plate,
                j => {
                    let base = shapes.get(&j).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
                    base.boolean(&plate, 1).ok_or(qymcad_core::errors::CoreError::ThickenPlateNotJoined)?
                }
            }
        };
        self.finish(body, res)
    }
    fn split_faces(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.split_faces(origin, normal).ok_or_else(|| refused(qymcad_core::errors::Op::SplitFaces))?
        };
        self.finish(body, res)
    }
    fn split_body(&self, bodies: &[Id], src: Id, origin: [f64; 3], normal: [f64; 3], section: u32) -> Result<Vec<(Mesh, Vec<MeshFace>)>, qymcad_core::errors::CoreError> {
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
        with_key.into_iter().zip(bodies).map(|((_, sh), &b)| self.finish(b, sh)).collect()
    }
    fn draft(&self, body: Id, src: Id, face_ids: &[u32], angle: f64, pull: [f64; 3], np_origin: [f64; 3], np_normal: [f64; 3], sides: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // draft: tilting faces relative to a neutral plane, in real B-rep, with the faces keeping their ids
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.draft_faces(face_ids, angle, pull, np_origin, np_normal, sides)
                .ok_or(qymcad_core::errors::CoreError::DraftFailed { angle })?
        };
        self.finish(body, res)
    }
    fn hole(&self, body: Id, src: Id, kind: u8, pl: [f64; 12], dia: f64, depth: f64, dia2: f64, depth2: f64, bore: u32, extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // a stepped hole — a cylinder plus a counterbore or countersink — as a real B-rep cut
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.hole_stepped_named(kind, pl, dia, depth, dia2, depth2, bore, extra).ok_or_else(|| refused(qymcad_core::errors::Op::Hole))?
        };
        self.finish(body, res)
    }
    fn holes(&self, body: Id, src: Id, kind: u8, pls: &[[f64; 12]], dia: f64, depth: f64, dia2: f64, depth2: f64, bores: &[u32], extra: &[u32]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        // many holes at sketch points in a single boolean cut: N tools fused, then subtracted
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.holes_stepped_named(kind, pls, dia, depth, dia2, depth2, bores, extra).ok_or_else(|| refused(qymcad_core::errors::Op::Holes))?
        };
        self.finish(body, res)
    }
    fn pattern(&self, body: Id, src: Id, transforms: &[[f64; 12]]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.pattern_named(body, src, transforms, &[])
    }
    fn pattern_named(&self, body: Id, src: Id, transforms: &[[f64; 12]], seeds: &[Vec<(u32, u32)>]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
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
            if rest.is_empty() {
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
            }
        };
        self.finish(body, res)
    }
    fn mirror(&self, body: Id, src: Id, plane: u8, keep: bool) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.mirror_named(body, src, plane, keep, &[])
    }
    fn mirror_named(&self, body: Id, src: Id, plane: u8, keep: bool, seed: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let m = s.mirrored(plane).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
            if !seed.is_empty() {
                m.rename_faces(seed); // the image is named before it is united with the original
            }
            if keep {
                // unite with a copy of the original, which must not be moved out of the cache
                let orig = s.transformed(&qymcad_core::feature::PLACE_IDENTITY).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
                orig.boolean(&m, 1).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?
            } else {
                m
            }
        };
        self.finish(body, res)
    }
    fn mirror_plane(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3], keep: bool) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        self.mirror_plane_named(body, src, origin, normal, keep, &[])
    }
    fn mirror_plane_named(&self, body: Id, src: Id, origin: [f64; 3], normal: [f64; 3], keep: bool, seed: &[(u32, u32)]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            let m = s.mirrored_plane(origin, normal).ok_or_else(|| refused(qymcad_core::errors::Op::MirrorPlane))?;
            if !seed.is_empty() {
                m.rename_faces(seed); // the image is named before it is united with the original
            }
            if keep {
                let orig = s.transformed(&qymcad_core::feature::PLACE_IDENTITY).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?;
                orig.boolean(&m, 1).ok_or_else(|| refused(qymcad_core::errors::Op::Mirror))?
            } else {
                m
            }
        };
        // A refusal names its cause. Mirroring a hollow part about one of its own faces is beyond the
        // kernel: fusing the halves leaves extra shells and no repair resolves them, tried five ways. While
        // that holds, the cause and the way out have to be visible instead of an unexplained red node.
        self.finish(body, res).map_err(|e| {
            if keep && matches!(e, qymcad_core::errors::CoreError::BrokenSolid) {
                qymcad_core::errors::CoreError::MirrorOfHollowBody
            } else {
                e
            }
        })
    }
    fn transform_body(&self, body: Id, src: Id, mat: [f64; 12]) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let s = shapes.get(&src).ok_or(qymcad_core::errors::CoreError::SourceBodyNotBuilt)?;
            s.transformed(&mat).ok_or_else(|| refused(qymcad_core::errors::Op::Move))?
        };
        self.finish(body, res)
    }
    fn tessellate(&self, body: Id) -> Option<(Mesh, Vec<MeshFace>)> {
        // re-tessellation of an already imported STEP shape, through the same `tessellate_merged`
        self.shapes.borrow().get(&body).and_then(|s| s.tessellate_merged_auto(self.k())) // deflection from the size
    }
    fn body_boolean(&self, body: Id, a: Id, b: Id, op: u8) -> Result<(Mesh, Vec<MeshFace>), qymcad_core::errors::CoreError> {
        let res = {
            let shapes = self.shapes.borrow();
            let sa = shapes.get(&a).ok_or(qymcad_core::errors::CoreError::BodyANotBuilt)?;
            let sb = shapes.get(&b).ok_or(qymcad_core::errors::CoreError::BodyBNotBuilt)?;
            let out = sa.boolean(sb, op).ok_or_else(|| refused(qymcad_core::errors::Op::BodyBoolean))?;
            // A cut that removed nothing is worse than a refusal. Measured: a tool not intersecting the
            // base left the area and volume unchanged, the node stood green, and the tool was consumed all
            // the same — a body lost with nothing given back.
            if op == 0 && sa.volume() > 1e-9 && out.volume() >= sa.volume() - 1e-6 {
                return Err(qymcad_core::errors::CoreError::CutRemovedNothing);
            }
            out
        };
        self.finish(body, res)
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
