//! THE REFERENCES OF A REBUILD: faces of sketches lost and rebound, points and axes resolved.

use super::*;
use crate::model::refs::SketchFaceFix;

impl Project {
    /// Whether sketch `sketch` sits on a face that the current rebuild of its body no longer has.
    ///
    /// In that case resolution falls through to the heuristic (the nearest co-directed face) and may land on
    /// the wrong one. Returns the carrying body so the rebuild report can warn about it honestly.
    pub fn sketch_face_ref_lost(&self, sketch: Id) -> Option<Id> {
        let s = self.sketches.iter().find(|s| s.id == sketch)?;
        if let crate::feature::SketchPlane::Face(body, ref key) = s.plane {
            if key.id != 0 {
                if let Some(faces) = self.regen_faces.get(&body) {
                    if !faces.iter().any(|f| f.id == key.id) {
                        return Some(body);
                    }
                }
            }
        }
        None
    }

    /// Repair references pointing at body `body` immediately after it has been rebuilt.
    ///
    /// Repairing once at the start of the pass works against the faces of the previous build, which is enough
    /// for an ordinary edit but not when the naming scheme itself changes (the move to recipe-based names):
    /// at the start of the pass the new names do not exist yet and there is nothing to repair against, so
    /// every reference falls through to "nearest match taken". Repairing here happens where the new names are
    /// already known.
    pub(crate) fn rebind_refs_to_body(&mut self, body: Id, before: &[crate::geom::MeshFace], report: &mut crate::feature::RegenReport) {
        use crate::feature::{FeatureKind, Rebind, SketchPlane};
        let Some(faces) = self.regen_faces.get(&body).cloned() else { return };
        let known = |id: u32| id != 0 && faces.iter().any(|f| f.id == id);
        // A matching number is not the same face, references carrying a fingerprint (a sketch on a face, a
        // hole) included. Accepting "the name is alive" is not enough: when the naming scheme changes, the
        // freed number goes to a different face — positional numbers start at 1 just as the old names did — and
        // the reference lands on the wrong face silently, with no repair in the report and a green node, while
        // a boss moved onto the neighbouring wall (measured: +320 mm^3, after which the fillets below it fell
        // apart). The key carries its own fingerprint, so that is what gets checked rather than the mere
        // presence of a number.
        let key_matches = |key: &crate::feature::FaceKey| -> bool {
            let Some(now) = faces.iter().find(|f| f.id == key.id) else { return false };
            let n2 = key.normal[0].powi(2) + key.normal[1].powi(2) + key.normal[2].powi(2);
            if n2 < 0.5 {
                return true; // The key carries no fingerprint, so there is nothing to check against and the
                             // number is trusted.
            }
            let dot = now.normal[0] * key.normal[0] + now.normal[1] * key.normal[1] + now.normal[2] * key.normal[2];
            let d = (now.centroid.x - key.centroid[0]).hypot(now.centroid.y - key.centroid[1]).hypot(now.centroid.z - key.centroid[2]);
            let span = faces.iter().map(|f| f.area.sqrt()).fold(1.0_f64, f64::max);
            dot > 0.9 && d < span.max(1.0)
        };
        // Guard against a number colliding with a name. This is not a migration and has no expiry.
        //
        // The body is already named by recipe while the reference carries a number from the old positional
        // scheme. Such a reference must not count as alive even when the number happens to exist: positional
        // numbers start at one just as structural names do, so a match means nothing here and resolution would
        // land on a different face, silently. The reference goes through the fingerprint instead, and the
        // result is written back into the key.
        //
        // Old references live longer than expected: re-saving does not rewrite them, and a key is updated only
        // when the feature itself is edited. Measured on real documents: 2 such references out of 15 in one, 11
        // out of 76 in another.
        let body_named = faces.iter().any(|f| crate::names::NameTable::is_named(f.id));
        let ref_ok = |key: &crate::feature::FaceKey| {
            if body_named && !crate::names::NameTable::is_named(key.id) {
                return false;
            }
            known(key.id) && key_matches(key)
        };

        // Sketches placed on a face of this body: the face by its name, and by place only exactly and marked
        // (`find_face`).
        let mut sfix: Vec<SketchFaceFix> = Vec::new();
        for sk in &self.sketches {
            if let SketchPlane::Face(b, ref key) = sk.plane {
                if b == body && !ref_ok(key) {
                    // An empty repair is not an event. The candidate may equal the previous number, meaning
                    // the reference was correct all along and was merely re-checked; reporting that as a repair
                    // sends the reader to inspect something that did not change.
                    if let Some(found) = self.find_face(&faces, key.id, Some(key)).filter(|f| f.face != key.id) {
                        sfix.push(SketchFaceFix { sketch: sk.id, body, face: found.face, by_place: found.by_place, what: format!("rebind-sketch-face#{} -> {}", key.id, found.face) });
                    }
                }
            }
        }
        for fix in sfix {
            if let Some(sk) = self.sketches.iter_mut().find(|s| s.id == fix.sketch) {
                if let SketchPlane::Face(_, key) = &mut sk.plane {
                    key.id = fix.face;
                }
            }
            if fix.by_place {
                report.rebinds.push(Rebind { node: fix.sketch, body, what: fix.what });
                self.note_found_by_place(fix.sketch);
            }
        }
        // Holes are deliberately not handled here: their reference became a query, which either finds the face
        // by recipe or refuses honestly. Matching "the nearest co-directed face" on its behalf is exactly the
        // silent guessing that queries were introduced to remove.
        //
        // Bare face numbers (the reference face of a chamfer) carry no fingerprint of their own, so it is taken
        // from the previous build, where the old id knew its centre and normal.
        //
        // A matching name is not the same face. When the naming scheme changes, an old number can go to a
        // different face: what used to be a wall under "1" becomes the bottom, and a shell referencing "face 1"
        // starts opening the wrong one (measured: the volume moved by 5 per cent). So the geometry is compared:
        // if the face under that number now faces another way or sits elsewhere, the reference is repaired.
        let same_face = |old_id: u32| -> bool {
            let (Some(was), Some(now)) = (before.iter().find(|f| f.id == old_id), faces.iter().find(|f| f.id == old_id)) else {
                return false;
            };
            let dot = was.normal[0] * now.normal[0] + was.normal[1] * now.normal[1] + was.normal[2] * now.normal[2];
            let d = (was.centroid.x - now.centroid.x).hypot(was.centroid.y - now.centroid.y).hypot(was.centroid.z - now.centroid.z);
            let span = faces.iter().map(|f| f.area.sqrt()).fold(1.0_f64, f64::max);
            dot > 0.9 && d < span.max(1.0)
        };
        let by_old = |old_id: u32| -> Option<u32> {
            if old_id == 0 {
                return None; // Not set.
            }
            if body_named && !crate::names::NameTable::is_named(old_id) {
                // See `ref_ok`: an old number on a body carrying new names always goes through the
                // fingerprint.
            } else if known(old_id) && (before.is_empty() || same_face(old_id)) {
                return None; // The name is alive and points at the same face: nothing to do.
            }
            let was = before.iter().find(|f| f.id == old_id);
            let key = was.map(|was| crate::feature::FaceKey { index: 0, centroid: [was.centroid.x, was.centroid.y, was.centroid.z], normal: was.normal, id: 0 });
            self.find_face(&faces, old_id, key.as_ref()).map(|f| f.face).filter(|&ni| ni != old_id)
        };
        // A NAMED RECORD, not a three-place tuple: the list is filled in one loop and read in another, and
        // between the two `.0`, `.1`, `.2` say nothing about which is the node and which the mapping.
        struct FaceRebind {
            /// The timeline node whose face reference is being repaired.
            node: Id,
            /// Old face number -> new one.
            map: Vec<(u32, u32)>,
            /// What to write in the report.
            what: String,
        }
        let mut nfix: Vec<FaceRebind> = Vec::new();
        for n in &self.timeline {
            let (src, ids): (Id, Vec<u32>) = match n.kind {
                // Only the chamfer remains here: its reference face is still a bare number. Draft, face offset
                // and face deletion use query references and need no similarity matching — they either find the
                // face by recipe or refuse.
                FeatureKind::Chamfer { src, ref_face, .. } => (src, vec![ref_face]),
                _ => continue,
            };
            if src != body {
                continue;
            }
            let map: Vec<(u32, u32)> = ids.iter().filter_map(|&i| by_old(i).map(|ni| (i, ni))).collect();
            if !map.is_empty() {
                nfix.push(FaceRebind { node: n.id, what: format!("rebind-faces#{map:?}"), map });
            }
        }
        for FaceRebind { node, map, what } in nfix {
            if let Some(n) = self.timeline.iter_mut().find(|n| n.id == node) {
                let fix = |v: &mut u32| {
                    if let Some(&(_, ni)) = map.iter().find(|(o, _)| o == v) {
                        *v = ni;
                    }
                };
                if let FeatureKind::Chamfer { ref_face, .. } = &mut n.kind {
                    fix(ref_face)
                }
            }
            report.rebinds.push(Rebind { node, body, what });
        }
    }

    /// Placements (3x4) of holes at the isolated points of sketch `sid`. The Z axis of each frame is the
    /// sketch normal — the tool cuts along -Z, that is, into the body beneath the sketch — and `flip` drills
    /// the other way.
    pub fn sketch_hole_points(&self, sid: Id, flip: bool) -> Vec<[f64; 12]> {
        let Some(frame) = self.sketch_frame_by_id(sid) else { return Vec::new() };
        let mut n = frame.normal();
        if flip {
            n = [-n[0], -n[1], -n[2]];
        }
        self.sketch_isolated_points(sid).into_iter().map(|p| crate::feature::PlaneFrame::from_origin_normal(p, n, 0.0).matrix12()).collect()
    }

    /// Whether the profile crosses the revolve axis.
    ///
    /// A body of revolution cannot be built that way in any CAD, but the kernel answers with a faceless
    /// "revolve failed", which reads as a broken tool. Returns actionable error text, or `None` when there is no
    /// conflict.
    ///
    /// `axis_o` and `axis_d` are the axis in sketch 2D space (z is ignored).
    pub fn revolve_profile_crosses_axis(&self, profile_xy: &[f64], axis_o: [f64; 3], axis_d: [f64; 3]) -> Option<crate::errors::CoreError> {
        let (dx, dy) = (axis_d[0], axis_d[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 || profile_xy.len() < 6 {
            return None;
        }
        // Signed distance from the profile points to the axis line; the tolerance scales with the profile.
        let span = profile_xy.chunks(2).fold(0.0_f64, |m, c| m.max(c[0].abs()).max(c[1].abs())).max(1.0);
        let tol = span * 1e-6;
        let (mut pos, mut neg) = (false, false);
        for c in profile_xy.chunks(2) {
            let side = ((c[0] - axis_o[0]) * dy - (c[1] - axis_o[1]) * dx) / len;
            if side > tol {
                pos = true;
            } else if side < -tol {
                neg = true;
            }
        }
        (pos && neg).then_some(crate::errors::CoreError::RevolveProfileCrossesAxis)
    }

    /// Resolve a parametric datum point: the x, y and z coordinates are expressions (`feat_dim`), so global
    /// parameters move the point. An empty expression keeps the stored number, making this idempotent.
    pub(crate) fn resolve_point_into(
        &mut self,
        point_id: Id,
        vars: &std::collections::HashMap<String, f64>,
        dim: Option<&std::collections::HashMap<String, String>>,
        kernel: &dyn crate::feature::Kernel,
    ) {
        let Some(pi) = self.datum_points.iter().position(|p| p.id == point_id) else { return };
        match self.datum_points[pi].def {
            // Bound to a vertex: `at` is an endpoint of a persistent edge from the kernel, so it travels with
            // the vertex. The edge and vertex live in body local space, which is the space of the owning part
            // (the point is created in the same context), so the value is stored directly. Without the edge (the
            // source is not built yet, or was deleted) the previous `at` is kept.
            PointDef::AtVertex { body, edge, end } => {
                if let Some(e) = kernel.edges(body).into_iter().find(|e| e.id == edge) {
                    self.datum_points[pi].at = if end { e.b } else { e.a };
                }
            }
            PointDef::Manual => {
                let at = self.datum_points[pi].at;
                self.datum_points[pi].at = [eval_dim(dim, "x", at[0], vars), eval_dim(dim, "y", at[1], vars), eval_dim(dim, "z", at[2], vars)];
            }
        }
    }

    /// Resolve a parametric datum axis: `TwoPoints{a,b}` gives origin = a and dir = norm(b - a), while
    /// `FromEdge` and `FromFace` bind associatively to an edge or a face axis through the kernel. `Manual` is
    /// left alone.
    pub(crate) fn resolve_axis_into(&mut self, axis_id: Id, kernel: &dyn crate::feature::Kernel) {
        let Some(ai) = self.datum_axes.iter().position(|d| d.id == axis_id) else { return };
        let set = |axes: &mut [DatumAxis], o: [f64; 3], d: [f64; 3]| {
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if l > 1e-9 {
                axes[ai].set_resolved(o, [d[0] / l, d[1] / l, d[2] / l]);
            }
        };
        match self.datum_axes[ai].def {
            AxisDef::TwoPoints { a, b } => {
                let pa = self.datum_points.iter().find(|d| d.id == a).map(|d| d.at);
                let pb = self.datum_points.iter().find(|d| d.id == b).map(|d| d.at);
                if let (Some(pa), Some(pb)) = (pa, pb) {
                    set(&mut self.datum_axes, pa, [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]]);
                }
            }
            AxisDef::FromEdge { body, edge } => {
                // The axis travels with the edge: a circular edge gives the centre and the axis of the circle,
                // a straight one the midpoint and the tangent.
                if let Some(e) = kernel.edges(body).into_iter().find(|e| e.id == edge) {
                    let (o, d) = e.axis_ref();
                    set(&mut self.datum_axes, o, d);
                }
            }
            AxisDef::FromFace { body, face } => {
                if let Some((o, d)) = kernel.face_axis(body, face) {
                    set(&mut self.datum_axes, o, d);
                }
            }
            AxisDef::Manual { .. } => {}
        }
    }

    /// Axis of a helical operation from the circular edge `edge` of body `src`: a point on the axis, a direction
    /// into the material, and the radius.
    ///
    /// The kernel is the source of truth for geometry here: `regen_edges` is filled by a later pass and is still
    /// empty during this one. The direction is oriented into the material, because the rim normal from the
    /// kernel is arbitrary and a thread or a flight would otherwise grow outwards from the end face.
    pub(crate) fn helical_axis(&self, kernel: &dyn crate::feature::Kernel, src: Id, edge: u32) -> Option<([f64; 3], [f64; 3], f64)> {
        let mi = self.mesh_index(src);
        kernel.edges(src).into_iter().find(|e| e.id == edge && e.is_circular()).map(|e| {
            // The direction runs along the cylinder of this radius rather than towards wherever most body
            // vertices are. The latter is wrong on parts with a chamfer: the thread rim sits at the base of the
            // chamfer, and when the mass of the body lies above it (a boss, a flange) the thread goes into empty
            // space and removes nothing. Measured on a real part: the preview showed the correct direction while
            // the rebuild built the other way.
            let ax = mi
                .and_then(|mi| crate::geom::axis_along_cylinder(&self.bodies[mi].mesh, e.center, e.axis, e.radius))
                .or_else(|| mi.map(|mi| orient_axis_into_mesh(e.center, e.axis, &self.bodies[mi].mesh.verts)))
                .unwrap_or(e.axis);
            (e.center, ax, e.radius)
        })
    }

    /// Whether the cylinder of radius `r` about the axis, over a span of `length`, is a hole or a shaft, decided
    /// from the mesh of body `src`.
    ///
    /// This decides which way the thread groove runs; the checkbox in the panel is only a hint and the geometry
    /// decides (see [`crate::geom::cyl_side_from_mesh`]).
    pub(crate) fn cyl_side_of_body(&self, src: Id, center: [f64; 3], axis: [f64; 3], r: f64, length: f64) -> Option<bool> {
        let mi = self.mesh_index(src)?;
        crate::geom::cyl_side_from_mesh(&self.bodies[mi].mesh, center, axis, r, 0.0, length)
    }
}
