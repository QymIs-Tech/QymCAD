//! Timeline rebuild: one pass that builds the model from the feature tree.
//!
//! `regenerate` lives here together with the helpers nothing else calls. The split is mechanical, by call
//! graph rather than by name, which makes this a closed unit: the module has exactly one entry point,
//! `regenerate`, and all the machinery of a single pass (re-pointing references at new bodies, face names from
//! recipes, the extent of a tool body, resolving a revolve axis) stays hidden.
//!
//! The rebuild is the most interconnected part of the core, and keeping it in a general file next to two
//! hundred unrelated methods means starting every edit by working out what here is even connected.

use super::*;
use crate::feature::FeatureKind;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

/// EVERYTHING ONE NODE'S REBUILD NEEDS besides the document itself.
///
/// Forty kinds of feature are rebuilt in one pass, and every one of them wants the same handful of things:
/// the kernel, the parameter values, the dimension expressions, the rename map, the set of bodies that
/// changed and the report. Passing those as six arguments to forty functions is how a signature grows past
/// reading — a complaint this very refactor exists to answer — so they travel as one.
///
/// The fields are locals OF THE PASS, not parts of the document. That is what makes the split possible at
/// all: a `&mut Project` and a `&mut Pass` are disjoint borrows, so a branch can take the document mutably
/// and still write its findings here.
struct Pass<'a> {
    kernel: &'a dyn crate::feature::Kernel,
    /// The values of the global parameters, for the dimension expressions.
    vars: &'a std::collections::HashMap<String, f64>,
    /// The dimension expressions of the node being rebuilt, if it has any.
    dims: Option<&'a std::collections::HashMap<String, String>>,
    /// "Old edge number -> new name" for the bodies built so far in this pass.
    emap: &'a mut EdgeRenames,
    /// The bodies that changed, whose consumers must therefore rebuild.
    dirty: &'a mut std::collections::HashSet<Id>,
    report: &'a mut crate::feature::RegenReport,
    /// The node being rebuilt: the owner of every name minted while it is built.
    node: Id,
}

impl Pass<'_> {
    /// A dimension of this node: the expression where there is one, the stored number otherwise.
    fn dim(&self, key: &str, stored: f64) -> f64 {
        eval_dim(self.dims, key, stored, self.vars)
    }
}

/// Estimate of how much work a rebuild will be; see [`Project::regen_plan`].
#[derive(Default, Debug, Clone)]
pub struct RegenPlan {
    /// Nodes the rebuild will touch, with a margin.
    pub nodes: Vec<Id>,
    /// Total number of nodes in the timeline, to tell a local edit from a full rebuild.
    pub total: usize,
    /// Whether the affected set contains a known slow operation (a thread).
    pub heavy: bool,
}

/// The stored `ruled` flag as the word it means.
fn walls(ruled: bool) -> crate::feature::LoftWalls {
    if ruled {
        crate::feature::LoftWalls::Ruled
    } else {
        crate::feature::LoftWalls::Smooth
    }
}

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
    fn rebind_refs_to_body(&mut self, body: Id, before: &[crate::geom::MeshFace], report: &mut crate::feature::RegenReport) {
        use crate::feature::{FeatureKind, Rebind, SketchPlane};
        let Some(faces) = self.regen_faces.get(&body).cloned() else { return };
        let candidate = |key: &crate::feature::FaceKey| -> Option<u32> {
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let d2 = |c: &crate::geom::Point3| (c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2);
            faces
                .iter()
                .filter(|f| f.id != 0 && dot(f.normal, key.normal) > 0.9)
                .min_by(|a, b| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal))
                .map(|f| f.id)
        };
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

        // Sketches placed on a face of this body.
        let mut sfix: Vec<(Id, u32, String)> = Vec::new();
        for sk in &self.sketches {
            if let SketchPlane::Face(b, ref key) = sk.plane {
                if b == body && !ref_ok(key) {
                    // An empty repair is not an event. The candidate may equal the previous number, meaning
                    // the reference was correct all along and was merely re-checked; reporting that as a repair
                    // sends the reader to inspect something that did not change.
                    if let Some(newid) = candidate(key).filter(|&n| n != key.id) {
                        sfix.push((sk.id, newid, format!("rebind-sketch-face#{} → {newid}", key.id)));
                    }
                }
            }
        }
        for (sid, newid, what) in sfix {
            if let Some(sk) = self.sketches.iter_mut().find(|s| s.id == sid) {
                if let SketchPlane::Face(_, key) = &mut sk.plane {
                    key.id = newid;
                }
            }
            report.rebinds.push(Rebind { node: sid, body, what });
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
            let was = before.iter().find(|f| f.id == old_id)?;
            let key = crate::feature::FaceKey { index: 0, centroid: [was.centroid.x, was.centroid.y, was.centroid.z], normal: was.normal, id: 0 };
            candidate(&key).filter(|&ni| ni != old_id)
        };
        let mut nfix: Vec<(Id, Vec<(u32, u32)>, String)> = Vec::new();
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
                nfix.push((n.id, map.clone(), format!("rebind-faces#{map:?}")));
            }
        }
        for (node, map, what) in nfix {
            if let Some(n) = self.timeline.iter_mut().find(|n| n.id == node) {
                let fix = |v: &mut u32| {
                    if let Some(&(_, ni)) = map.iter().find(|(o, _)| o == v) {
                        *v = ni;
                    }
                };
                match &mut n.kind {
                    FeatureKind::Chamfer { ref_face, .. } => fix(ref_face),
                    _ => {}
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
        if flip { n = [-n[0], -n[1], -n[2]]; }
        self.sketch_isolated_points(sid)
            .into_iter()
            .map(|p| crate::feature::PlaneFrame::from_origin_normal(p, n, 0.0).matrix12())
            .collect()
    }

    /// Face names of a primitive: the end face at -z, the end face at +z, and the side surface. A primitive has
    /// no sketch, so the source of a role is the role itself (`src` = 0): the recipe defines the whole
    /// topology.
    pub fn primitive_names(&mut self, feature: Id) -> [u32; 3] {
        [
            self.intern_name(feature, crate::names::Role::CapStart, 0),
            self.intern_name(feature, crate::names::Role::CapEnd, 0),
            self.intern_name(feature, crate::names::Role::Side, 0),
        ]
    }

    /// Cap names per region: triples of [region key, bottom, top].
    ///
    /// One pair per feature is not enough: extruding several contours produces several bodies, and one cap name
    /// for all of them would mean two different faces under one name. The region key is the lowest wall name
    /// among its edges: profiles merge into one planar face before the extrude, so "profile number k" no longer
    /// exists afterwards while the set of region edges does.
    pub fn region_cap_names(&mut self, feature: Id, profiles: &[Vec<f64>]) -> Vec<u32> {
        let mut out = Vec::new();
        // A single region carries no key. The key is the lowest wall name, and that changes as soon as an
        // entity is added to or removed from the sketch, so the cap name would move on any profile edit (the
        // test `face_ids_survive_a_topology_change_in_the_sketch` catches this). The key is needed only to
        // separate several bodies of one feature, where it is more honest than a positional number.
        if profiles.len() == 1 {
            let [c0, c1] = self.cap_names(feature);
            return vec![0, c0, c1];
        }
        for prof in profiles {
            // Encoding: [nloops, then per loop nedges plus the edges laid out by EDGE_FIELDS]; the edge name is
            // the last field of a record.
            let mut key = 0u32;
            let mut i = 1usize;
            let loops = prof.first().copied().unwrap_or(0.0) as usize;
            for _ in 0..loops {
                let n = prof.get(i).copied().unwrap_or(0.0) as usize;
                i += 1;
                for k in 0..n {
                    let at = i + k * crate::geom::EDGE_FIELDS + crate::geom::EDGE_FIELDS - 1;
                    let v = prof.get(at).copied().unwrap_or(0.0) as u32;
                    if v != 0 && (key == 0 || v < key) {
                        key = v;
                    }
                }
                i += n * crate::geom::EDGE_FIELDS;
            }
            let c0 = self.intern_name(feature, crate::names::Role::CapStart, key as Id);
            let c1 = self.intern_name(feature, crate::names::Role::CapEnd, key as Id);
            out.extend_from_slice(&[key, c0, c1]);
        }
        out
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
        let span = profile_xy
            .chunks(2)
            .fold(0.0_f64, |m, c| m.max(c[0].abs()).max(c[1].abs()))
            .max(1.0);
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
    fn resolve_point_into(&mut self, point_id: Id, vars: &std::collections::HashMap<String, f64>, dim: Option<&std::collections::HashMap<String, String>>, kernel: &dyn crate::feature::Kernel) {
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
    fn resolve_axis_into(&mut self, axis_id: Id, kernel: &dyn crate::feature::Kernel) {
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
    fn helical_axis(&self, kernel: &dyn crate::feature::Kernel, src: Id, edge: u32) -> Option<([f64; 3], [f64; 3], f64)> {
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
    fn cyl_side_of_body(&self, src: Id, center: [f64; 3], axis: [f64; 3], r: f64, length: f64) -> Option<bool> {
        let mi = self.mesh_index(src)?;
        crate::geom::cyl_side_from_mesh(&self.bodies[mi].mesh, center, axis, r, 0.0, length)
    }

    pub fn regenerate(&mut self, kernel: &dyn crate::feature::Kernel) -> crate::feature::RegenReport {
        self.regenerate_watched(kernel, &crate::feature::NoWatch)
    }

    /// What a rebuild will actually touch, without a single kernel call.
    ///
    /// The progress counter used to show the position in the timeline rather than the work, so cutting one hole
    /// in one part reported rebuilding the whole project. Untouched nodes are skipped during the rebuild (the
    /// `needs` condition below), but that is invisible from outside, and "walked past 25 nodes" cannot be told
    /// from "rebuilt 25 nodes".
    ///
    /// Here the same estimate is computed in advance and for free: a dependency walk over the timeline with no
    /// geometry involved. The answer is needed three times — for an honest counter, for choosing between a modal
    /// window and a status line, and for measurement in tests.
    ///
    /// The estimate deliberately errs on the high side: missing a node that then rebuilds is worse than naming
    /// one too many, so it analyses neither suppressed chains nor the rollback bar.
    ///
    /// The property that has to hold: the set of bodies actually rebuilt is a subset of this estimate.
    pub fn regen_plan(&self) -> RegenPlan {
        let mut dirty: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut plan = RegenPlan::default();
        let limit = self.rollback.unwrap_or(usize::MAX);
        for (i, nd) in self.timeline.iter().enumerate() {
            if i >= limit {
                break;
            }
            let kind = &nd.kind;
            // The same reasons as `needs` inside the rebuild itself: the node's own dirty flag, an
            // unrecoverable error from last time, a dirty input, a moved sketch base face, and the dynamic
            // inputs of a mirrored part and a pattern instance.
            let self_dirty = nd.dirty || self.regen_errors.get(&nd.id).is_some_and(|e| !e.retryable());
            let needs = self_dirty
                || kind.inputs().iter().any(|id| dirty.contains(id))
                || kind.inputs().iter().any(|&inp| self.sketch_plane_body(inp).is_some_and(|pb| dirty.contains(&pb)))
                || matches!(kind, FeatureKind::MirrorPart { src_comp, .. } if self.active_body_before(*src_comp, i).is_some_and(|b| dirty.contains(&b)))
                || matches!(kind, FeatureKind::PartInstance { src_comp, .. } if self.active_body_before(*src_comp, i).is_some_and(|b| dirty.contains(&b)));
            if !needs || nd.suppressed {
                continue;
            }
            plan.nodes.push(nd.id);
            // Heavy work is named explicitly. A thread takes seconds to cut and looks indistinguishable from
            // nothing happening, so it has to be reported even when a single node rebuilds.
            if matches!(kind, FeatureKind::Thread { .. }) {
                plan.heavy = true;
            }
            for out in kind.bodies() {
                dirty.insert(out);
            }
            if let FeatureKind::Plane { plane } = kind {
                dirty.insert(*plane);
            }
            if let FeatureKind::DatumAxis { axis } = kind {
                dirty.insert(*axis);
            }
        }
        plan.total = self.timeline.len();
        plan
    }

    /// A rebuild that reports progress and obeys cancellation; see [`crate::feature::RegenWatch`].
    ///
    /// A separate name rather than an extra parameter on `regenerate`: the observer is needed by exactly one
    /// caller (the application's background rebuild), and threading it through a hundred and fifty call sites
    /// would mean paying for cancellation where there is nobody to cancel.
    pub fn regenerate_watched(&mut self, kernel: &dyn crate::feature::Kernel, watch: &dyn crate::feature::RegenWatch) -> crate::feature::RegenReport {
        // An error without a node is not an error. A node can be deleted from anywhere, and an error record
        // that outlives the deletion shows a failure with nothing behind it in the tree. Cleared here rather
        // than only in the deletion paths: one place nothing can bypass.
        self.regen_errors.retain(|id, _| self.timeline.iter().any(|n| n.id == *id));
        use crate::feature::{FeatureKind, RegenReport};
        self.settle_sketches(); // A body is never built from an unsolved sketch (see the method).
        let mut report = RegenReport::default();
        // The faces from before the rebuild bridge the old name to the new one. References that store a face
        // number as a bare integer carry no fingerprint of their own, so without this there is nothing to repair
        // them against: the old name is gone and what it denoted is unknown. Here it is known — the old id maps
        // to its centre and normal from the previous build, and the same face is then looked up among the new
        // ones.
        //
        // For a file that has just been opened the rebuild cache is empty (it is not serialised), so the faces
        // come from the bodies themselves: they live in the bundle next to the mesh and carry exactly those old
        // names.
        let mut faces_before: std::collections::HashMap<Id, Vec<crate::geom::MeshFace>> = self.regen_faces.clone();
        for b in &self.bodies {
            faces_before.entry(b.id).or_insert_with(|| b.faces.clone());
        }
        self.rebind_lost_face_refs(&mut report); // Lost references are repaired once, and visibly.
        let mut dirty: std::collections::HashSet<Id> = std::collections::HashSet::new();
        // Snapshot for parametric feature dimensions (expressions over global parameters).
        let vars = self.param_map();
        let feat_dims = self.feat_dims.clone();
        let limit = self.rollback.unwrap_or(usize::MAX); // Rollback bar: build only the first N nodes.
        // Component patterns come before bodies: the copies have to be in place by the time their bodies are
        // built and the mates are solved. Otherwise the first frame after an edit shows the copies in their old
        // positions and the change only becomes visible on the second rebuild.
        self.resolve_comp_patterns();
        let mut unbuilt: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut emap: EdgeRenames = EdgeRenames::new();
        // The counter counts work, not steps. Reporting "node i of the whole timeline" always ends at the full
        // count however many nodes were skipped, which reads as the whole project being rebuilt — reasonably so,
        // there being no other source of information.
        // THE PER-NODE CONTEXT, assembled the same way for every branch below.
        //
        // A macro and not a function because it borrows six locals of this pass at once; written out as a
        // call that would be seven arguments repeated forty times, which is the very thing the branches were
        // split up to avoid.
        // The node is named at the call site: it is a local of the loop, and a macro sees only what was in
        // scope where it was written.
        macro_rules! pass {
            ($node:expr) => {
                Pass { kernel, vars: &vars, dims: feat_dims.get(&$node), emap: &mut emap, dirty: &mut dirty, report: &mut report, node: $node }
            };
        }
        let plan_total = self.regen_plan().nodes.len();
        let mut work = 0usize;
        for i in 0..self.timeline.len() {
            // Cancellation happens between nodes rather than inside a kernel operation: a boolean cannot be
            // interrupted half-way, and pretending otherwise would yield half a body. This is a boundary where
            // the document is still whole, and the incomplete result is simply discarded by the caller.
            if !watch.step(work, plan_total) {
                report.cancelled = true;
                return report;
            }
            let (node_id, kind, self_dirty, suppressed) = {
                let nd = &self.timeline[i];
                (nd.id, nd.kind.clone(), nd.dirty, nd.suppressed)
            };
            // Suppressed by the rollback bar: the body is neither built nor shown.
            if i >= limit {
                for b in kind.bodies() {
                    self.drop_body_from_view(b);
                }
                continue;
            }
            // Suppressed feature. A modifier (fillet, chamfer, combine, shell, hole, pattern, mirror, move) is
            // skipped as a no-op: its output body is a copy of the source (a pass-through with an identity
            // transform) and consumers continue on the pre-modifier body, so suppressing one feature does not
            // break the chain. A base feature (with no source), or a modifier whose source was not built,
            // cascades instead: the body goes into `unbuilt` and its consumers are not built either.
            if suppressed {
                // A suppressed split leaves the source whole. Copying it into the first piece is wrong: the
                // remaining pieces would hang around as stale geometry next to the whole body, showing the
                // material twice.
                if let FeatureKind::SplitBody { src, ref bodies, .. } = kind {
                    for &b in bodies {
                        self.drop_body_from_view(b);
                        unbuilt.insert(b);
                    }
                    if !unbuilt.contains(&src) {
                        dirty.insert(src); // The source is visible whole again, so its consumers rebuild from
                                           // it.
                    }
                    continue;
                }
                let pass = kind.consumed_body().filter(|s| !unbuilt.contains(s));
                if let (Some(src), Some(b)) = (pass, kind.body()) {
                    // The source is built, so the modifier is skipped: the output is a copy of `src` and
                    // everything downstream continues.
                    if self_dirty || dirty.contains(&src) {
                        let res = kernel.transform_body(b, src, crate::feature::PLACE_IDENTITY);
                        if self.apply_regen(node_id, b, res, &mut dirty, &mut report, kernel, &mut emap) {
                            self.timeline[i].dirty = false;
                        }
                    }
                    continue;
                }
                // Nothing to skip (a base feature, or an unbuilt source), so it cascades.
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            // Depends on a body that is not suppressed yet was not built (a cascading source, or a failure
            // above), so it is neither built nor shown.
            if kind.inputs().iter().any(|id| unbuilt.contains(id)) {
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            // External and base dependency: a node whose sketch sits on a face of a body (its own, or an
            // external one through an `ExternalRef`) rebuilds when that source body rebuilds, because the face
            // frame may have moved. The source body has to come earlier in the timeline.
            //
            // The bodies of the node are captured before the match, whose branches destructure `kind`.
            let out_bodies = kind.bodies();

            let plane_dirty = kind.inputs().iter().any(|&inp| self.sketch_plane_body(inp).is_some_and(|pb| dirty.contains(&pb)));
            // A node that failed earlier is recomputed. Otherwise it is not rebuilt on the next pass, there is
            // no fresh error, and the "do not build on a failure" cascade does not see it, so everything above
            // it builds on a ghost again. Simply marking its bodies unbuilt is not an option either: the error
            // may no longer reproduce (the input was fixed), and that would freeze a working chain forever.
            let self_dirty = self_dirty || self.regen_errors.get(&node_id).is_some_and(|e| !e.retryable());
            let needs = self_dirty || kind.inputs().iter().any(|id| dirty.contains(id)) || plane_dirty
                // A mirrored part depends on the active body of its source: a dynamic input.
                || matches!(kind, FeatureKind::MirrorPart { src_comp, .. } if self.active_body_before(src_comp, i).is_some_and(|b| dirty.contains(&b)))
                // A pattern instance has the same dynamic dependency: editing the source moves the copies.
                || matches!(kind, FeatureKind::PartInstance { src_comp, .. } if self.active_body_before(src_comp, i).is_some_and(|b| dirty.contains(&b)));
            if needs {
                work += 1;
            }
            // Isolation: only a part builds bodies, and references are confined to the owning component.
            if needs && kind.body().is_some() {
                if let Some(err) = self.isolation_error(i, &kind) {
                    report.errors.push((node_id, err));
                    continue;
                }
            }
            // Reference honesty: the base face of an input sketch was lost by persistent id, so resolution
            // falls through to the heuristic (the nearest co-directed face) and the feature may land on the
            // wrong one. The node is warned about without failing the build, so the reason a rebuild came out
            // differently is visible.
            if needs && kind.body().is_some() {
                for inp in kind.inputs() {
                    if let Some(pb) = self.sketch_face_ref_lost(inp) {
                        report.errors.push((node_id, crate::errors::CoreError::SketchFaceRefLost { sketch: inp, body: pb }));
                    }
                }
            }
            // The "old edge number to new name" map is inherited from the inputs before the build: the
            // references of the node itself (a chamfer or a fillet selects edges of its input) are translated
            // through it below.
            for out in kind.bodies() {
                inherit_edge_renames(&mut emap, out, &kind.inputs());
            }
            // Captured before the match, which moves `kind`, for the pass-through fallback when a modifier
            // fails.
            let consumed_src = kind.consumed_body();
            let out_body = kind.body();
            let mut clear = true;
            match kind {
                // Datums are resolved unconditionally (they are cheap) and in timeline order: a parametric plane
                // or axis is computed from its definition before the consumers below it.
                FeatureKind::Plane { plane } => {
                    self.resolve_plane_into(plane, &vars, feat_dims.get(&node_id));
                    if needs {
                        dirty.insert(plane); // The datum moved, so its consumers (sketches, mirror, split)
                                             // rebuild.
                    }
                }
                FeatureKind::DatumPoint { point } => {
                    self.resolve_point_into(point, &vars, feat_dims.get(&node_id), kernel);
                }
                FeatureKind::DatumAxis { axis } => {
                    self.resolve_axis_into(axis, kernel);
                }
                FeatureKind::Sketch { sketch } => {
                    // Projections of body geometry are recomputed here, in timeline order: the source bodies
                    // above are already built and the consumers of the sketch (an extrude over the projected
                    // contour) come below. They are recomputed unconditionally rather than only under `needs`,
                    // because a projection depends on another body and "the sketch itself did not change" says
                    // nothing about whether that body did.
                    if let Some(si) = self.sketch_index(sketch) {
                        if !self.sketches[si].projections.is_empty() {
                            let before = self.sketch_projection_key(si);
                            self.resolve_sketch_projections(si, kernel);
                            if self.sketch_projection_key(si) != before {
                                self.regen_sketch(si);
                                dirty.insert(sketch); // The projection moved, so the consumers of the contour follow.
                            }
                        }
                    }
                    if needs {
                        dirty.insert(sketch); // The sketch changed, so its consumers rebuild.
                    }
                }
                FeatureKind::Extrude { sketch, ref profiles, height, reach, down, ref fill, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_extrude(&mut p, sketch, profiles, height, reach, down, fill, body);
                }
                FeatureKind::Revolve { sketch, ref profiles, axis, angle, axis_datum, axis_line, reach, src, op, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_revolve(&mut p, sketch, profiles, axis, angle, axis_datum, axis_line, reach, src, op, body);
                }
                FeatureKind::Sweep { sketch, ref profiles, path_sketch, path, src, op, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_sweep(&mut p, sketch, profiles, path_sketch, path, src, op, body);
                }
                FeatureKind::Loft { sketches, contours, ruled, src, op, surface, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_loft(&mut p, sketches, contours, ruled, src, op, surface, body);
                }
                FeatureKind::Box3 { dx, dy, dz, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_box3(&mut p, dx, dy, dz, body);
                }
                FeatureKind::Cylinder { r, h, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_cylinder(&mut p, r, h, body);
                }
                FeatureKind::Sphere { r, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_sphere(&mut p, r, body);
                }
                FeatureKind::Cone { r1, r2, h, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_cone(&mut p, r1, r2, h, body);
                }
                FeatureKind::Torus { major, minor, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_torus(&mut p, major, minor, body);
                }
                FeatureKind::Prism { r, n, h, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_prism(&mut p, r, n, h, body);
                }
                FeatureKind::Combine { src, sketch, ref profiles, height, op, extent, down, ref fill, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_combine(&mut p, src, sketch, profiles, height, op, extent, down, fill, body);
                }
                FeatureKind::Fillet { src, radius, ref edges, ref at_vertices, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_fillet(&mut p, src, radius, edges, at_vertices, body);
                }
                FeatureKind::Chamfer { src, dist, ref edges, mode, d2, flip, ref_face, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_chamfer(&mut p, src, dist, edges, mode, d2, flip, ref_face, body);
                }
                FeatureKind::Shell { src, thickness, ref faces, side, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_shell(&mut p, src, thickness, faces, side, body);
                }
                FeatureKind::RemoveFace { src, ref faces, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_removeface(&mut p, src, faces, body);
                }
                FeatureKind::PushFace { src, ref face, dist, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_pushface(&mut p, src, face, dist, body);
                }
                FeatureKind::Patch { src, ref edges, tangent, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_patch(&mut p, src, edges, tangent, body);
                }
                FeatureKind::SurfaceReplace { src, ref faces, surface, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_surfacereplace(&mut p, src, faces, surface, body);
                }
                FeatureKind::FaceCopy { src, ref faces, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_facecopy(&mut p, src, faces, body);
                }
                FeatureKind::Trim { src, tool, keep, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_trim(&mut p, src, tool, keep, body);
                }
                FeatureKind::Stitch { ref parts, tol, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_stitch(&mut p, parts, tol, body);
                }
                FeatureKind::Thicken { src, face, thickness, join, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_thicken(&mut p, src, face, thickness, join, body);
                }
                FeatureKind::SplitFace { src, plane, datum, offset, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_splitface(&mut p, src, plane, datum, offset, body);
                }
                FeatureKind::SplitBody { src, plane, datum, offset, ref bodies } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_splitbody(&mut p, src, plane, datum, offset, bodies);
                }
                FeatureKind::Draft { src, ref faces, neutral, angle, flip, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_draft(&mut p, src, faces, neutral, angle, flip, body);
                }
                FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_lineararray(&mut p, src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body);
                }
                FeatureKind::CircularArray { src, count, angle, axis, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_circulararray(&mut p, src, count, angle, axis, body);
                }
                FeatureKind::Mirror { src, plane, keep, datum, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_mirror(&mut p, src, plane, keep, datum, body);
                }
                FeatureKind::Move { src, mat, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_move(&mut p, src, mat, body);
                }
                FeatureKind::BodyBoolean { a, b, op, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_bodyboolean(&mut p, a, b, op, body);
                }
                FeatureKind::Import { body, .. } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_import(&mut p, body);
                }
                FeatureKind::Hole { src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch, flip, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_hole(&mut p, src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch, flip, body);
                }
                FeatureKind::PartInstance { src_comp, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_partinstance(&mut p, i, src_comp, body);
                }
                FeatureKind::MirrorPart { src_comp, ln, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_mirrorpart(&mut p, i, src_comp, ln, body);
                }
                FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_thread(&mut p, src, edge, spec, length, lead_in, lead_out, body);
                }
                FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_auger(&mut p, src, edge, spec, length, lead_in, lead_out, body);
                }
                _ => {}
            }
            // Pass-through fallback: the modifier failed to build (broken edges or faces after a suppression or
            // an edit higher up the timeline — for example a suppressed fillet whose edges a chamfer below it
            // referenced) while its source exists, so it is skipped and the output body becomes a copy of the
            // source. That keeps the chain from holding stale geometry and makes the effect of the change
            // visible, instead of the model looking unchanged after a suppression. The error is already in the
            // report.
            if !clear {
                if let (Some(src), Some(b)) = (consumed_src.filter(|s| !unbuilt.contains(s)), out_body) {
                    if let Ok((mesh, faces)) = kernel.transform_body(b, src, crate::feature::PLACE_IDENTITY) {
                        self.set_body_mesh(b, mesh);
                        self.regen_faces.insert(b, faces.clone());
                        dirty.insert(b); // Consumers rebuild on the pass-through body.
                        report.built.push((b, faces));
                        clear = true; // The state is current; the real build returns once the failure above is
                                      // fixed.
                    }
                }
            }
            // References to this body are repaired immediately: its new face names are already known while the
            // consumers below it have not been built yet. That survives both an ordinary topology change and a
            // change of the naming scheme itself, without "nearest match taken" on every other node.
            if let Some(b) = out_body {
                self.rebind_refs_to_body(b, faces_before.get(&b).map(|v| v.as_slice()).unwrap_or(&[]), &mut report);
            }
            if clear {
                self.timeline[i].dirty = false;
            }
            // When a node fails, everything resting on it is not built either.
            //
            // The cascade existed but errors never reached it: the next operation took the body of the failed
            // one as its source, complete with its stale mesh, and built on a ghost. That produced two visible
            // bodies in one part and the avalanche where fixing one feature breaks another, because everything
            // below was already built on nothing.
            //
            // A temporary error (the source is not built yet, the live B-rep is not raised) does not count here:
            // such a node is waiting its turn rather than broken.
            if report.errors.iter().any(|(id, e)| *id == node_id && !e.retryable()) {
                for b in out_bodies {
                    unbuilt.insert(b);
                }
            }
        }
        // Edges of the built bodies are copied into the model, keyed by body id, so axis connectors
        // (`EdgeMid`) resolve by persistent id. Only the bodies rebuilt in this pass, as for `regen_faces`; a
        // mock supplies none.
        let built_bodies: Vec<Id> = report.built.iter().map(|(b, _)| *b).collect();
        for b in built_bodies {
            let edges = kernel.edges(b);
            if edges.is_empty() {
                self.regen_edges.remove(&b);
            } else {
                self.regen_edges.insert(b, edges);
            }
        }
        // Parametric mate expressions (angle and offsets over global parameters, like feature and sketch
        // dimensions): `feat_dims[joint id]["angle" | "offset" | "offset2"]` becomes a number before the solve,
        // and an empty expression keeps the stored number.
        //
        // An expression is a specified value: it states what the value has to equal. It is written into `drive`
        // rather than into the reading, or the solver would simply overwrite it with the measured fact.
        for j in &mut self.joints {
            let Some(d) = feat_dims.get(&j.id) else { continue };
            for (slot, key) in [(0usize, "angle"), (1, "offset"), (2, "offset2")] {
                let has = d.get(key).is_some_and(|e| !e.trim().is_empty());
                if has {
                    j.drive[slot] = Some(eval_dim(Some(d), key, j.drive[slot].unwrap_or(0.0), &vars));
                }
            }
        }
        // Placement pass: the bodies are built in local space, so the mates are solved and the components are
        // placed in the assembly. Run unconditionally, because editing a mate or a parameter does not dirty any
        // body while the placement still has to be recomputed.
        self.solve_joints();
        // Clear error marks from nodes that are no longer in the timeline.
        if !self.regen_errors.is_empty() || !self.edge_refs.is_empty() {
            let live: std::collections::HashSet<Id> = self.timeline.iter().map(|n| n.id).collect();
            self.regen_errors.retain(|id, _| live.contains(id));
            self.edge_refs.retain(|id, _| live.contains(id)); // Edge snapshots of deleted features.
        }
        report
    }

    /// A FILLET: round the picked edges of body `src` by `radius`, with a variable radius at named vertices.
    ///
    /// Returns whether the node's error record may be cleared (see `apply_regen`).
    fn regen_fillet(&mut self, p: &mut Pass, src: Id, radius: f64, edges: &crate::refs::Ref, at_vertices: &[(crate::refs::Ref, f64)], body: Id) -> bool {
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
        let asked_count = edges.query.picked_descs().len();
        let asked_edges = asked_count > 0;
        let edges = &self.live_fillet_edges(p.node, src, edges, p.emap, p.kernel);
        let lost_edges = asked_edges && edges.is_empty();
        // A variable radius is specified at vertices. Each table entry is resolved as a reference:
        // a vertex name is derived from its edges, so editing the neighbours does not disturb it.
        // The value is parametric like the radius itself, under the dimension key
        // `at{descriptor}`.
        let verts: Vec<([f64; 3], f64)> = at_vertices
            .iter()
            .filter_map(|(r, val)| {
                let desc = self.resolve_vertex_refs(src, r, "ref-what-fillet-vertex").ok()?.first().copied()?;
                let val = p.dim(&format!("at{desc}"), *val);
                self.vertex_point(src, desc).map(|p| (p, val))
            })
            .collect();
        // A solid tool does not operate on a surface. Otherwise it appears to work: the node goes
        // red while a degenerate two-triangle body is left next to the part (found by the fuzzer).
        let on_sheet = p.kernel.body_is_sheet(src);
        let res = if !verts.is_empty() && !edges.is_empty() {
            p.kernel.fillet_at_vertices(body, src, radius, edges, &verts)
        } else {
            // The name of a fillet surface comes from the edge that produced it, there being no
            // other source in the recipe. The edges are already named, so the name is
            // predictable.
            let names: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Blend, *e as Id)).collect();
            // Corner patch: a face produced by the vertex where fillets meet (see
            // `Role::Corner`).
            let corners: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Corner, *e as Id)).collect();
            p.kernel.fillet(body, src, radius, edges, &names, &corners, &self.blend_names_all(p.node, src, p.kernel))
        };
        let res = if on_sheet { Err(crate::errors::CoreError::NeedsSolidNotSheet) } else { res };
        let res = if lost_edges { Err(crate::errors::CoreError::EdgesNotFound { asked: asked_count }) } else { res };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }




    /// Chamfer: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_chamfer(&mut self, p: &mut Pass, src: Id, dist: f64, edges: &crate::refs::Ref, mode: crate::feature::ChamferMode, d2: f64, flip: bool, ref_face: u32, body: Id) -> bool {
        let dist = p.dim("dist", dist);
        let d2 = p.dim("d2", d2);
        // As for a fillet: an empty list means the whole part, and a lost reference must not
        // masquerade as that.
        let asked_count = edges.query.picked_descs().len();
        let asked_edges = asked_count > 0;
        let edges = &self.live_fillet_edges(p.node, src, edges, p.emap, p.kernel);
        let lost_edges = asked_edges && edges.is_empty();
        // Asymmetry (two setbacks, or setback plus angle) applies only to an explicit edge
        // selection; otherwise the chamfer is symmetric.
        let res = if mode != crate::feature::ChamferMode::Symmetric && !edges.is_empty() {
            p.kernel.chamfer_ex(body, src, dist, d2, mode, flip, ref_face, edges)
        } else {
            // The name of a chamfer surface comes from the edge that produced it and the patch name
            // from the vertex: the same recipe as for a fillet.
            let names: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Blend, *e as Id)).collect();
            let corners: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Corner, *e as Id)).collect();
            p.kernel.chamfer(body, src, dist, edges, &names, &corners, &self.blend_names_all(p.node, src, p.kernel))
        };
        let res = if lost_edges { Err(crate::errors::CoreError::EdgesNotFound { asked: asked_count }) } else { res };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Shell: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_shell(&mut self, p: &mut Pass, src: Id, thickness: f64, faces: &crate::refs::Ref, side: crate::feature::ShellSide, body: Id) -> bool {
        let thickness = p.dim("thickness", thickness);
        // Names of the inner walls come from the recipe: a wall is produced by offsetting a face
        // and, without a name of its own, takes the name of the outer face, so a reference to an
        // inner edge lands on the outer one.
        let walls = self.shell_wall_names(p.node, src);
        // Resolution goes through the query, and a refusal stops the operation: opening the wrong
        // face is worse than opening none, since the body then looks similar with the hole in the
        // wrong place.
        //
        // Asking for two faces and opening one is not success. Measured: a shell given two faces
        // where one reference did not resolve opened one and stayed green (11 faces instead of 10).
        // The part looks similar but is closed on the side where an opening was expected. Only a
        // hand-picked set is checked this way: for a descriptive query the number of descriptors
        // says nothing about the result.
        let asked_faces = faces.query.is_pick_list().then(|| faces.query.picked_descs().len()).unwrap_or(0);
        let res = match self.faces_by_ref(p.node, src, &faces, "ref-what-shell-faces") {
            Err(_) => Err(crate::errors::CoreError::FacesNotFound),
            Ok(ids) if asked_faces > 0 && ids.len() < asked_faces => Err(crate::errors::CoreError::FacesNotFound),
            Ok(ids) => match side {
                crate::feature::ShellSide::Centred => p.kernel.shell_center(body, src, thickness, &ids),
                side => p.kernel.shell_named(body, src, thickness, side == crate::feature::ShellSide::Outward, &ids, &walls),
            },
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// RemoveFace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_removeface(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> bool {
        // Face references resolve by recipe through the query. Nothing found means a refusal rather
        // than removing something similar: the kernel would otherwise silently remove the wrong face,
        // or nothing at all, which is worse — the step exists in the timeline and the bodies are
        // unchanged.
        let res = match self.faces_by_ref(p.node, src, &faces, "ref-what-removed-faces") {
            Ok(ids) if !ids.is_empty() => p.kernel.remove_faces(body, src, &ids),
            _ => Err(crate::errors::CoreError::FacesNotFound),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Patch: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_patch(&mut self, p: &mut Pass, src: Id, edges: &crate::refs::Ref, tangent: bool, body: Id) -> bool {
        // The edges come from a query, so a patch follows its base: trimming the shape changes the
        // boundary while the description stays correct. A hand-picked set is translated through the
        // rename map of the source body, the same way a fillet is.
        let edges = edges.clone();
        let live = self.live_fillet_edges(p.node, src, &edges, p.emap, p.kernel);
        let res = if live.is_empty() { Err(crate::errors::CoreError::EdgesNotFound { asked: edges.query.picked_descs().len() }) } else { p.kernel.patch(body, src, &live, tangent, self.intern_name(p.node, crate::names::Role::Patch, 0)) };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Trim: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_trim(&mut self, p: &mut Pass, src: Id, tool: Id, keep: [f64; 3], body: Id) -> bool {
        // The sheet is cut by another body and the piece at the clicked point is kept. The tool is
        // not consumed: it usually trims several surfaces in turn.
        let res = p.kernel.trim(body, src, tool, keep);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Stitch: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_stitch(&mut self, p: &mut Pass, parts: &[Id], tol: f64, body: Id) -> bool {
        // The pieces of a surface become one. The tolerance is parametric like any feature number;
        // the inputs are bodies, so there are no name references here and nothing to translate.
        let t = p.dim("tol", tol);
        let res = if parts.len() < 2 { Err(crate::errors::CoreError::OpFailed(crate::errors::Op::Stitch)) } else { p.kernel.stitch(body, &parts, t) };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// PushFace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_pushface(&mut self, p: &mut Pass, src: Id, face: &crate::refs::Ref, dist: f64, body: Id) -> bool {
        // Offsetting a face: the distance is parametric (the `dist` feature dimension) and the face
        // reference resolves by name (a persistent id), so it survives edits higher up the timeline.
        // Resolution yields the current face id in the rebuilt source; a positional number would not
        // do, since it moves with any operation placed before this one.
        let dist = eval_dim(p.dims, "dist", dist, p.vars);
        // The face is found by recipe through the query; there is no separate repair pass any more,
        // so nothing found means a refusal with a reason rather than offsetting a similar face.
        //
        // A sheet has no face to offset. This is an operation for solids: it moves a face and builds
        // the walls behind it, while a surface has neither volume nor neighbours to follow. A silent
        // refusal explains neither what is wrong nor that thickness is given to a sheet by the
        // thicken feature.
        let sheet = p.kernel.body_is_sheet(src);
        let res = if sheet {
            Err(crate::errors::CoreError::PushFaceOnSheet)
        } else {
            match self.face_by_ref(p.node, src, &face, "ref-what-pushed-face") {
                Err(_) => Err(crate::errors::CoreError::FaceNotFound),
                Ok(_) if dist.abs() < 1e-9 => Err(crate::errors::CoreError::ZeroPushDistance),
                Ok(c) => p.kernel.push_face(body, src, c.desc, dist),
            }
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// SurfaceReplace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_surfacereplace(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, surface: Id, body: Id) -> bool {
        // The node that stitches the surface layer back into the timeline. The faces come from a
        // query, and a query that finds nothing produces a named refusal rather than a silent
        // substitution: surface work is expensive manual effort, and the program has no right to
        // change where it lands.
        let faces = faces.clone();
        let res = match self.faces_by_ref(p.node, src, &faces, "ref-what-surface-replace") {
            Ok(ids) if !ids.is_empty() => p.kernel.replace_faces(body, src, &ids, surface),
            _ => Err(crate::errors::CoreError::FacesNotFound),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// FaceCopy: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_facecopy(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> bool {
        // The faces come from a query and are copied into a separate sheet. The name of a copy is
        // structural — "the image of face N in the copy" (`Role::Instance`) — so it can be referenced
        // further like any other geometry.
        let faces = faces.clone();
        let res = match self.faces_by_ref(p.node, src, &faces, "ref-what-face-copy") {
            Ok(ids) if !ids.is_empty() => {
                let names: Vec<u32> = ids.iter().map(|f| self.intern_name(p.node, crate::names::Role::Instance, *f as Id)).collect();
                p.kernel.copy_faces(body, src, &ids, &names)
            }
            _ => Err(crate::errors::CoreError::FacesNotFound),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Thicken: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_thicken(&mut self, p: &mut Pass, src: Id, face: u32, thickness: f64, join: Id, body: Id) -> bool {
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
        let res = if face == 0 || !alive {
            Err(crate::errors::CoreError::FaceNotFound)
        } else if t.abs() < 1e-9 {
            Err(crate::errors::CoreError::ZeroThickness)
        } else {
            // Plate names come from the recipe: the offset side is produced by its face and each
            // side wall by its boundary edge, which is their entire recipe and does not depend on
            // numbering. The pairs are prepared in advance, since interning mutates the name table
            // while the kernel only needs the finished substitution.
            let (face_names, edge_names) = self.thicken_names(p.node, src, p.kernel);
            p.kernel.thicken_face(body, src, face, t, join, &face_names, &edge_names)
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// SplitFace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_splitface(&mut self, p: &mut Pass, src: Id, plane: u8, datum: Id, offset: f64, body: Id) -> bool {
        // The plane is a reference, as for a body split: a datum (including a face snapshot) or a
        // world plane.
        let dpl = (datum != 0).then(|| self.planes.iter().find(|p| p.id == datum)).flatten().map(|p| (p.origin, p.normal));
        let lost = datum != 0 && dpl.is_none();
        let (o0, n) = dpl.unwrap_or_else(|| match plane {
            1 => ([0.0; 3], [0.0, 1.0, 0.0]),
            2 => ([0.0; 3], [1.0, 0.0, 0.0]),
            _ => ([0.0; 3], [0.0, 0.0, 1.0]),
        });
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let res = if lost {
            Err(crate::errors::CoreError::SplitPlaneDeleted)
        } else if len < 1e-9 {
            Err(crate::errors::CoreError::ZeroNormal)
        } else {
            let d = p.dim("offset", offset);
            let u = [n[0] / len, n[1] / len, n[2] / len];
            p.kernel.split_faces(body, src, [o0[0] + u[0] * d, o0[1] + u[1] * d, o0[2] + u[2] * d], u)
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Draft: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_draft(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, neutral: crate::refs::Ref, angle: f64, flip: bool, body: Id) -> bool {
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
        let asked_faces = faces.query.is_pick_list().then(|| faces.query.picked_descs().len()).unwrap_or(0);
        let res = match (self.faces_by_ref(p.node, src, &faces, "ref-what-draft-faces"), self.face_by_ref(p.node, src, &neutral, "ref-what-draft-neutral")) {
            (Ok(ids), _) if asked_faces > 0 && ids.len() < asked_faces => Err(crate::errors::CoreError::DraftNeedsFaces),
            (Ok(ids), Ok(np)) if !ids.is_empty() => {
                let np_o = np.centroid;
                let np_n = if flip { [-np.normal[0], -np.normal[1], -np.normal[2]] } else { np.normal };
                {
                    // The side face of a draft is named after the face that was tilted: drafting
                    // produces a new face next to it, which without a name of its own would take a
                    // positional number.
                    let sides: Vec<u32> = ids
                        .iter()
                        .filter(|f| crate::names::NameTable::is_named(**f))
                        .flat_map(|f| [*f, self.intern_name(p.node, crate::names::Role::DraftSide, *f as Id)])
                        .collect();
                    p.kernel.draft(body, src, &ids, angle, np_n, np_o, np_n, &sides)
                }
            }
            _ => Err(crate::errors::CoreError::DraftNeedsFaces),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Mirror: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_mirror(&mut self, p: &mut Pass, src: Id, plane: u8, keep: bool, datum: Id, body: Id) -> bool {
        // A datum plane supplies its origin and normal (resolved earlier in this pass) and the
        // mirror is taken about it; otherwise a world plane 0, 1 or 2 is used.
        // An image is not the original face: with `keep` the body holds both halves, and without
        // names of their own they are indistinguishable — the same collision a pattern had.
        let seed = self.instance_name_seeds(p.node, src, 2).pop().unwrap_or_default();
        let dpl = (datum != 0).then(|| self.planes.iter().find(|p| p.id == datum)).flatten().map(|p| (p.origin, p.normal));
        // A deleted plane means no mirror. Falling back to a world plane sounds defensible — a
        // mirror about another plane is still a mirror — but the measurement refutes it: the part
        // moves. A mirror about a datum at x = 50 gave face centres from x = 10 to x = 90, and
        // deleting the datum moved them to -30..30 with no red node. A split already refuses in the
        // same situation, for exactly the same reason.
        let res = match dpl {
            Some((o, n)) => p.kernel.mirror_plane_named(body, src, o, n, keep, &seed),
            None if datum != 0 => Err(crate::errors::CoreError::MirrorPlaneDeleted),
            None => p.kernel.mirror_named(body, src, plane, keep, &seed),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Move: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_move(&mut self, p: &mut Pass, src: Id, mat: [f64; 12], body: Id) -> bool {
        let res = p.kernel.transform_body(body, src, mat);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// BodyBoolean: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_bodyboolean(&mut self, p: &mut Pass, a: Id, b: Id, op: u8, body: Id) -> bool {
        // Parametric body-to-body boolean: `op` applied to the B-reps of `a` and `b`, producing
        // `body`; both operands are consumed.
        let res = p.kernel.body_boolean(body, a, b, op);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Box3: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_box3(&mut self, p: &mut Pass, dx: f64, dy: f64, dz: f64, body: Id) -> bool {
        let (dx, dy, dz) = (p.dim("dx", dx), p.dim("dy", dy), p.dim("dz", dz));
        let res = p.kernel.extrude(body, &rect_profile(dx, dy), dz, crate::feature::PLACE_IDENTITY);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Cylinder: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_cylinder(&mut self, p: &mut Pass, r: f64, h: f64, body: Id) -> bool {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        let nm = self.primitive_names(p.node);
        let res = p.kernel.cylinder(body, r, h, nm); // Exact B-rep cylinder, three faces.
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Sphere: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_sphere(&mut self, p: &mut Pass, r: f64, body: Id) -> bool {
        let r = eval_dim(p.dims, "r", r, p.vars);
        let nm = self.primitive_names(p.node);
        let res = p.kernel.sphere(body, r, nm); // Exact sphere, one face.
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Cone: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_cone(&mut self, p: &mut Pass, r1: f64, r2: f64, h: f64, body: Id) -> bool {
        let (r1, r2, h) = (p.dim("r1", r1), p.dim("r2", r2), p.dim("h", h));
        let nm = self.primitive_names(p.node);
        let res = p.kernel.cone(body, r1, r2, h, nm); // Exact cone.
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Torus: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_torus(&mut self, p: &mut Pass, major: f64, minor: f64, body: Id) -> bool {
        let (major, minor) = (p.dim("major", major), p.dim("minor", minor));
        let nm = self.primitive_names(p.node);
        let res = p.kernel.torus(body, major, minor, nm); // Exact torus, one face.
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Prism: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_prism(&mut self, p: &mut Pass, r: f64, n: u32, h: f64, body: Id) -> bool {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        let res = p.kernel.extrude(body, &polygon_profile(r, n), h, crate::feature::PLACE_IDENTITY);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Loft: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_loft(&mut self, p: &mut Pass, sketches: Vec<Id>, contours: Vec<Id>, ruled: bool, src: Id, op: u8, surface: bool, body: Id) -> bool {
        // Each section sits on its own plane (the placement is encoded in `places`), and two or more
        // sections produce a body. A zero `src` makes a separate body; otherwise the lofted solid is
        // combined with body `src` as a lofted cut or boss.
        let caps = self.cap_names(p.node);
        let res = match self.loft_encoded_named(p.node, &sketches, &contours) {
            // A surface is the same loft, not closed into a solid. It admits no boolean: there is
            // nothing to combine a surface with a body by until it has been given a thickness.
            Some((data, offsets, places)) if src == 0 => p.kernel.loft(body, &data, &offsets, &places, walls(ruled), if surface { crate::feature::LoftBody::Sheet } else { crate::feature::LoftBody::Solid }, caps),
            Some((data, offsets, places)) => p.kernel.loft_combine(body, src, &data, &offsets, &places, walls(ruled), op, caps),
            None => Err(crate::errors::CoreError::LoftNeedsTwoSections),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Import: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_import(&mut self, p: &mut Pass, body: Id) -> bool {
        // An external STEP solid has no recipe, so this only re-tessellates the shape from the kernel
        // cache. Without a shape in the kernel (a mock, or before restoration from the source) the
        // already loaded mesh is kept.
        if let Some((mesh, faces)) = p.kernel.tessellate(body) {
            self.set_body_mesh(body, mesh);
            p.dirty.insert(body);
            self.regen_faces.insert(body, faces.clone());
            p.report.built.push((body, faces));
            self.regen_errors.remove(&p.node);
        }
        true // The body is valid (a mesh exists), so the node is clean and its consumers continue.
    }

    /// PartInstance: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_partinstance(&mut self, p: &mut Pass, at: usize, src_comp: Id, body: Id) -> bool {
        // An instance body is a copy of the active source body as it is. The position comes from the
        // transform of the copied component, driven by the pattern, so the geometry is not moved.
        //
        // Only from what lies above: the same forward-reference rule as for a mirror.
        let res = match self.active_body_before(src_comp, at) {
            None => Err(crate::errors::CoreError::SourcePartHasNoBody),
            Some(sb) => p.kernel.transform_body(body, sb, crate::feature::PLACE_IDENTITY),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// MirrorPart: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_mirrorpart(&mut self, p: &mut Pass, at: usize, src_comp: Id, ln: [f64; 3], body: Id) -> bool {
        // The mirror plane passes through the local zero of the source, and the normal `ln` was fixed
        // in its local space at creation (see `add_mirror_part` and `add_mirror_part_rigid`), so the
        // coordinate system of the copy is not mirrored and keeps the orientation of the source. The
        // placement of a mirror is never touched by regenerate — it is moved by hand — while the
        // shape is associative and rebuilds with the source.
        let res = if ln[0] * ln[0] + ln[1] * ln[1] + ln[2] * ln[2] < 0.25 {
            Err(crate::errors::CoreError::MirrorPlaneUnset)
        } else {
            // Only from what lies above: a timeline node may not rest on a body built by a node
            // below it.
            //
            // Reverting this restriction on the suspicion that it broke a real document was wrong:
            // measured, that document contained zero mirrors and could not have been affected, and
            // it failed to open because of the nesting ladder in the selected-edge list — an
            // unrelated defect. The restriction stands, and "the part has no source" is stated
            // honestly: that part really has no body left.
            match self.active_body_before(src_comp, at) {
                None => Err(crate::errors::CoreError::SourcePartHasNoBody),
                Some(sb) => p.kernel.mirror_plane(body, sb, [0.0; 3], ln, false),
            }
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Thread: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_thread(&mut self, p: &mut Pass, src: Id, edge: u32, spec: crate::thread::ThreadSpec, length: f64, lead_in: f64, lead_out: f64, body: Id) -> bool {
        let mut spec = spec;
        // Parametric like every feature: the size, pitch and clearance may be expressions.
        spec.nominal_d = p.dim("nominal", spec.nominal_d);
        spec.pitch = p.dim("pitch", spec.pitch);
        spec.fit = p.dim("fit", spec.fit);
        // The crest and root radii are parametric too; zero means "as the standard says".
        spec.crest_r = { let v = p.dim("crest_r", spec.crest_r.unwrap_or(0.0)); (v > 1e-9).then_some(v) };
        spec.root_r = { let v = p.dim("root_r", spec.root_r.unwrap_or(0.0)); (v > 1e-9).then_some(v) };
        let length = p.dim("length", length);
        let lead_in = p.dim("lead_in", lead_in);
        let lead_out = p.dim("lead_out", lead_out);
        let res = match self.helical_axis(p.kernel, src, edge) {
            Some((c, ax, r)) => {
                // The material side comes from the geometry rather than from the checkbox. A groove
                // pointing the wrong way cuts empty space: on a shaft marked "internal" it removed
                // 1.8 cm^3 instead of 13 and left flat discs instead of turns. There is no case
                // where a thread into empty space is wanted, so the geometry wins.
                if let Some(side) = self.cyl_side_of_body(src, c, ax, r, length) {
                    spec.internal = side;
                }
                let g = spec.geometry();
                // The turn faces are named before the kernel call: the recipe supplies the names
                // (a profile edge plus the lead).
                let gnames = self.groove_names(p.node, g.groove.len(), spec.starts.max(1));
                let rnames = self.relief_names(p.node);
                // Validation before the kernel: bad parameters otherwise produce a silent no-op or
                // crash the sweep.
                let turns = length / g.lead.max(1e-9);
                let bad = if !spec.internal && g.depth >= r * 0.95 {
                    Some(crate::errors::CoreError::ThreadDepthTooDeep { depth: g.depth, radius: r, dia: spec.nominal_d, pitch: g.pitch })
                } else if g.pitch < 0.05 {
                    Some(crate::errors::CoreError::ThreadPitchTooSmall { pitch: g.pitch })
                } else if turns > 400.0 {
                    Some(crate::errors::CoreError::ThreadTooManyTurns { turns })
                } else if length <= 1e-6 {
                    Some(crate::errors::CoreError::ThreadLengthUnset)
                } else {
                    None
                };
                match bad {
                    Some(e) => Err(e),
                    None => p.kernel
                        .helical(crate::feature::Helical {
                            body,
                            src,
                            origin: c,
                            dir: ax,
                            radius: r,
                            profile: &crate::thread::encode_edges(&g.groove),
                            length,
                            lead: g.lead,
                            starts: spec.starts.max(1),
                            left: spec.left,
                            fuse: false, // a thread is subtracted
                            lead_in,
                            lead_out,
                            gnames: &gnames,
                            rnames: &rnames,
                            crest_relief: spec.radial_relief(),
                        })
                        .and_then(|(m, f)| {
                            // The result is checked, not only the input: a thread that removed
                            // nothing is a refusal rather than a success, or a smooth part is
                            // reported as done. The groove side already came from the geometry, so
                            // what is caught here is the rest: too fine a pitch, a degenerate
                            // profile, a miss against the face.
                            let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                            let got = src_v - m.volume();
                            if src_v > 0.0 && got < 1e-6 * src_v {
                                Err(crate::errors::CoreError::ThreadRemovedNothing { before: src_v, after: m.volume() })
                            } else {
                                Ok((m, f))
                            }
                        }),
                }
            }
            None => Err(crate::errors::CoreError::ThreadRimNotFound),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Auger: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_auger(&mut self, p: &mut Pass, src: Id, edge: u32, spec: crate::thread::AugerSpec, length: f64, lead_in: f64, lead_out: f64, body: Id) -> bool {
        let mut spec = spec;
        spec.outer_d = p.dim("outer", spec.outer_d);
        spec.pitch = p.dim("pitch", spec.pitch);
        spec.thickness = p.dim("thickness", spec.thickness);
        spec.edge_r = p.dim("edge_r", spec.edge_r);
        let length = p.dim("length", length);
        let lead_in = p.dim("lead_in", lead_in);
        let lead_out = p.dim("lead_out", lead_out);
        let res = match self.helical_axis(p.kernel, src, edge) {
            Some((c, ax, r)) => {
                spec.shaft_d = r * 2.0; // The shaft diameter is taken from the geometry, so it stays
                                        // associative.
                // The flight faces are named by the same recipe as the thread turns.
                let gnames = self.groove_names(p.node, spec.flight_profile().len(), spec.starts.max(1));
                let rnames = self.relief_names(p.node);
                if spec.flight_height() <= 1e-6 {
                    Err(crate::errors::CoreError::AugerOuterNotBigger { outer: spec.outer_d, shaft: spec.shaft_d })
                } else if spec.pitch <= 1e-6 || length <= 1e-6 {
                    Err(crate::errors::CoreError::AugerBadPitchOrLength)
                } else {
                    p.kernel
                        .helical(crate::feature::Helical {
                            body,
                            src,
                            origin: c,
                            dir: ax,
                            radius: r,
                            profile: &crate::thread::encode_edges(&spec.flight_profile()),
                            length,
                            lead: spec.lead(),
                            starts: spec.starts.max(1),
                            left: spec.left,
                            fuse: true, // an auger flight is welded on
                            lead_in,
                            lead_out,
                            gnames: &gnames,
                            rnames: &rnames,
                            crest_relief: 0.0,
                        })
                        .and_then(|(m, f)| {
                            // An auger flight is welded on, so the volume has to grow. If it did
                            // not, the flight landed beside the shaft, and reporting success would
                            // mean a smooth shaft.
                            let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                            if src_v > 0.0 && m.volume() <= src_v * 1.001 {
                                Err(crate::errors::CoreError::AugerAddedNothing { before: src_v, after: m.volume() })
                            } else {
                                Ok((m, f))
                            }
                        })
                }
            }
            None => Err(crate::errors::CoreError::AugerRimNotFound),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Hole: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_hole(&mut self, p: &mut Pass, src: Id, face: crate::refs::Ref, point: [f64; 3], normal: [f64; 3], diameter: f64, depth: f64, kind: u8, dia2: f64, depth2: f64, sketch: Id, flip: bool, body: Id) -> bool {
        let (diameter, depth) = (p.dim("diameter", diameter), p.dim("depth", depth));
        let (dia2, depth2) = (p.dim("dia2", dia2), p.dim("depth2", depth2));
        let res = if sketch != 0 {
            // At the isolated points of a sketch: one frame per point, with every cut applied by a
            // single boolean.
            let pls = self.sketch_hole_points(sketch, flip);
            if pls.is_empty() {
                Err(crate::errors::CoreError::NoIsolatedPointsForHoles)
            } else {
                // The wall of each hole is named after the sketch point that placed it, so adding a
                // point does not rename the neighbouring holes.
                let pts = self.sketch_hole_point_ids(sketch);
                let bores: Vec<u32> = (0..pls.len()).map(|i| self.intern_name(p.node, crate::names::Role::Hole, pts.get(i).copied().unwrap_or(i as Id + 1))).collect();
                p.kernel.holes(body, src, kind, &pls, diameter, depth.abs(), dia2, depth2, &bores, &self.hole_tool_names(p.node))
            }
        } else {
            // The face is found by recipe through the query, and the hole travels with it.
            //
            // A refusal stops the operation rather than merely marking the node. Recording an error
            // and drilling at the stale point anyway lets the success immediately clear the mark,
            // which is exactly the silent guessing queries were introduced to remove. Found by an
            // end-to-end run against the real kernel.
            let picked = !matches!(face.query, crate::refs::Query::Id(0)); // The sketch-driven form
                                                                            // picks no face.
            match self.face_by_ref(p.node, src, &face, "ref-what-hole-face") {
                Err(_) if picked => Err(crate::errors::CoreError::FaceNotFound),
                resolved => {
                    let (point, normal) = match resolved {
                        Ok(c) => (c.centroid, c.normal),
                        Err(_) => (point, normal),
                    };
                    // The tool (a cylinder plus a counterbore or countersink) is placed at the face
                    // point along its normal and cuts inwards.
                    let pl = crate::feature::PlaneFrame::from_origin_normal(point, normal, 0.0).matrix12();
                    let bore = self.intern_name(p.node, crate::names::Role::Hole, 0);
                    p.kernel.hole(body, src, kind, pl, diameter, depth.abs(), dia2, depth2, bore, &self.hole_tool_names(p.node))
                }
            }
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// SplitBody: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_splitbody(&mut self, p: &mut Pass, src: Id, plane: u8, datum: Id, offset: f64, bodies: &[Id]) -> bool {
        // The cutting plane is a reference (as for a mirror): a datum (including a face snapshot) or
        // a world plane.
        // The datum is resolved here, in timeline order, so a face that moved carries the split with
        // it; otherwise a split would be a one-off cut against forgotten coordinates.
        let dpl = (datum != 0).then(|| self.planes.iter().find(|p| p.id == datum)).flatten().map(|p| (p.origin, p.normal));
        // A deleted plane means no cut. Falling back to a world plane is not acceptable here: a
        // mirror about another plane is still a mirror, while a split along another plane breaks the
        // body in a different place, silently. A red node beats a quietly different part.
        let lost = datum != 0 && dpl.is_none();
        let (o0, n) = dpl.unwrap_or_else(|| match plane {
            1 => ([0.0; 3], [0.0, 1.0, 0.0]),
            2 => ([0.0; 3], [1.0, 0.0, 0.0]),
            _ => ([0.0; 3], [0.0, 0.0, 1.0]),
        });
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let res = if lost {
            Err(crate::errors::CoreError::CutPlaneDeleted)
        } else if len < 1e-9 {
            Err(crate::errors::CoreError::ZeroNormal)
        } else {
            // The offset along the normal is parametric (the `offset` feature dimension), so the
            // split moves by formula and follows a global parameter like an extrude dimension.
            let d = eval_dim(p.dims, "offset", offset, p.vars);
            let u = [n[0] / len, n[1] / len, n[2] / len];
            let o = [o0[0] + u[0] * d, o0[1] + u[1] * d, o0[2] + u[2] * d];
            p.kernel.split_body(bodies, src, o, u, self.intern_name(p.node, crate::names::Role::CutSection, 0))
        };
        // A split writes SEVERAL bodies, so the answer is the conjunction: the node is clean only if every
        // piece of it landed.
        let mut clear = true;
        match res {
            Ok(parts) => {
                // The number of pieces is fixed at creation. Moving the plane so the body divides
                // differently is not a reason to lose a piece silently or to leave a ghost body from
                // the previous split: the node reports an honest error and the bodies stay as they
                // were.
                if parts.len() != bodies.len() {
                    let err = crate::errors::CoreError::SplitPieceCount { got: parts.len(), want: bodies.len() };
                    self.regen_errors.insert(p.node, err.clone());
                    p.report.errors.push((p.node, err));
                    clear = false;
                } else {
                    for (b, part) in bodies.iter().zip(parts) {
                        clear &= self.apply_regen(p.node, *b, Ok(part), p.dirty, p.report, p.kernel, p.emap);
                    }
                }
            }
            Err(e) => {
                for &b in bodies {
                    clear &= self.apply_regen(p.node, b, Err(e.clone()), p.dirty, p.report, p.kernel, p.emap);
                }
            }
        }
        clear
    }

    /// LinearArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_lineararray(&mut self, p: &mut Pass, src: Id, dx: f64, dy: f64, dz: f64, count: u32, dx2: f64, dy2: f64, dz2: f64, count2: u32, dx3: f64, dy3: f64, dz3: f64, count3: u32, body: Id) -> bool {
        // Parametric: the step lives as an expression per vector component (dx, dy, dz), so global
        // parameters move the pattern. An empty expression keeps the stored number.
        let (dx, dy, dz) = (p.dim("dx", dx), p.dim("dy", dy), p.dim("dz", dz));
        let (dx2, dy2, dz2) = (p.dim("dx2", dx2), p.dim("dy2", dy2), p.dim("dz2", dz2));
        let (dx3, dy3, dz3) = (p.dim("dx3", dx3), p.dim("dy3", dy3), p.dim("dz3", dz3));
        // A 3D grid: direction one (i*d1) by two (j*d2) by three (k*d3). A count of one or less in
        // the second or third direction reduces the dimensionality.
        let (c1, c2, c3) = (count.max(1), count2.max(1), count3.max(1));
        let mut ts: Vec<[f64; 12]> = Vec::with_capacity((c1 * c2 * c3) as usize);
        for i in 0..c1 {
            for j in 0..c2 {
                for k in 0..c3 {
                    ts.push(translate_mat(
                        i as f64 * dx + j as f64 * dx2 + k as f64 * dx3,
                        i as f64 * dy + j as f64 * dy2 + k as f64 * dy3,
                        i as f64 * dz + j as f64 * dz2 + k as f64 * dz3,
                    ));
                }
            }
        }
        let seeds = self.instance_name_seeds(p.node, src, ts.len());
        let res = p.kernel.pattern_named(body, src, &ts, &seeds);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// CircularArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_circulararray(&mut self, p: &mut Pass, src: Id, count: u32, angle: f64, axis: Id, body: Id) -> bool {
        // Parametric: the angle is an expression over global parameters. An empty expression keeps
        // the stored number.
        let angle = p.dim("angle", angle);
        let c = count.max(1);
        let step = if angle.abs() >= 359.9 { 360.0 / c as f64 } else { angle / c as f64 };
        // Rotation axis: world Z when `axis` is zero, otherwise a datum axis (origin and direction,
        // resolved earlier in this pass).
        let (org, dir) = if axis != 0 {
            self.datum_axes.iter().find(|d| d.id == axis).map(|d| (d.origin(), d.dir())).unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]))
        } else {
            ([0.0; 3], [0.0, 0.0, 1.0])
        };
        let ts: Vec<[f64; 12]> = (0..c).map(|i| rot_about_axis(org, dir, i as f64 * step)).collect();
        let seeds = self.instance_name_seeds(p.node, src, ts.len());
        let res = p.kernel.pattern_named(body, src, &ts, &seeds);
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Live edges of a query reference: recorded names are translated, a description is asked again.
    ///
    /// Two steps, both required: translate the old numbers through the rename map of the source body
    /// (`EdgeRenames`), and repair a selection that came loose when the topology changed, using the geometric
    /// snapshot. Spelling that out separately in the fillet and in the chamfer invites a third copy with the
    /// next modifier, and a copy that forgets the second step lets an edge selection drift silently after any
    /// edit higher up the timeline, putting a fillet on the wrong edge.
    ///
    /// The distinction between the two paths is not cosmetic. A recorded edge name goes stale together with the
    /// names of its faces, and translating through the rename map is the only honest way to catch up. A
    /// description has nothing to catch up with: it already refers to today's geometry.
    ///
    /// Fresh edges come from the kernel rather than from `regen_edges`, which is filled by a later pass and at
    /// this point still describes the previous topology.
    /// WHERE A SKETCH'S GEOMETRY LANDS: its plane as a transform for the kernel, the identity for a world
    /// plane.
    fn sketch_place(&self, sketch: Id) -> [f64; 12] {
        self.sketch_frame_by_id(sketch).map(|f| f.matrix12()).unwrap_or(crate::feature::PLACE_IDENTITY)
    }

    /// Extrude: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_extrude(&mut self, p: &mut Pass, sketch: Id, profiles: &[Id], height: f64, reach: crate::feature::Reach, down: f64, fill: &[Id], body: Id) -> bool {
        let mut pl = self.sketch_place(sketch);
        // Parametric dimension expressions, where present, override the stored numbers.
        let height = p.dim("height", height);
        let down = p.dim("down", down);
        // Extent along the normal: both ways, two-sided or one-sided (see `extrude_extent`).
        let (start, total) = crate::feature::extrude_extent(height, down, reach);
        if start.abs() > 1e-9 {
            // Shift the origin by `start` along the normal (N is the column [2,6,10]).
            pl[3] += pl[2] * start;
            pl[7] += pl[6] * start;
            pl[11] += pl[10] * start;
        }
        // Encode every contour and extrude them in one node (`combine_region_multi`; a zero `src`
        // makes a new body).
        let res = self
            .encode_profiles_fill(p.node, sketch, profiles, fill)
            .ok_or(crate::errors::CoreError::ProfileNotFound)
            .and_then(|profs| {
                let caps = self.region_cap_names(p.node, &profs);
                p.kernel.combine_region_multi(body, 0, &profs, total, 1, pl, &caps)
            });
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Revolve: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_revolve(&mut self, p: &mut Pass, sketch: Id, profiles: &[Id], axis: u8, angle: f64, axis_datum: Id, axis_line: Id, reach: crate::feature::Reach, src: Id, op: u8, body: Id) -> bool {
        let pl = self.sketch_place(sketch);
        let angle = eval_dim(p.dims, "angle", angle, p.vars);
        // Axis priority: a sketch centreline (already in local space, so no inverse placement is
        // needed), then a datum axis (in world space, including one bound to an edge or a face,
        // converted to local space through the inverse placement), then the sketch X or Y axis.
        let axis_ln = self.revolve_axis_from_line(sketch, axis_line);
        let axis_od = (axis_datum != 0).then(|| self.datum_axes.iter().find(|d| d.id == axis_datum).map(|d| (d.origin(), d.dir()))).flatten();
        // Symmetric means a start angle of -angle/2, and a flip means -angle, sweeping the other
        // way. Implemented by pre-rotating the result about the local axis: pl' = pl * Rot(axis, t0).
        // The kernel is untouched — it builds [0, angle] and the rotation carries that to
        // [t0, t0 + angle].
        let theta0 = match reach {
            crate::feature::Reach::BothWays => -angle / 2.0,
            crate::feature::Reach::Backward => -angle,
            crate::feature::Reach::Forward => 0.0,
        };
        let with_start = |o: [f64; 3], d: [f64; 3], pl: [f64; 12]| -> [f64; 12] {
            if theta0.abs() < 1e-12 {
                pl
            } else {
                crate::feature::compose12(&pl, &crate::feature::rot12_axis(o, d, theta0))
            }
        };
        let res = self.encode_profiles_role(p.node, sketch, &profiles, &[], crate::names::Role::Revolved).ok_or(crate::errors::CoreError::ProfileNotFound).and_then(|profs| {
            let caps = self.region_cap_names(p.node, &profs);
            // Honest diagnostics before the kernel: a profile crossing the axis produces readable
            // text with a hint rather than a faceless "revolve failed".
            //
            // The axis in sketch local space is resolved once and serves both the check and the
            // kernel call. Writing the priority (centreline, then datum through the inverse
            // placement, then the X or Y fallback) twice — once for diagnostics and once for the
            // call — lets the copies drift, and the "profile crosses the axis" check would then
            // validate an axis other than the one the body is built about, lying silently.
            let (ax_o, ax_d) = match (axis_ln, axis_od) {
                (Some((lo, ld)), _) => (lo, ld),
                (None, Some((wo, wd))) => {
                    let inv = crate::feature::mat_inv12(&pl);
                    (crate::feature::apply12(&inv, wo), crate::feature::apply12_dir(&inv, wd))
                }
                _ => ([0.0, 0.0, 0.0], if axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }),
            };
            // The axis check runs over every contour rather than one: the command takes them all,
            // and a second contour crossing the axis fails the operation just as the first would.
            let chk: Vec<Id> = if profiles.is_empty() {
                self.sketch_index(sketch).map(|si| self.sketches[si].contour_ids.clone()).unwrap_or_default()
            } else {
                profiles.to_vec()
            };
            for cid in chk {
                if let Some(xy) = self.contour_profile_xy(cid) {
                    if let Some(msg) = self.revolve_profile_crosses_axis(&xy, ax_o, ax_d) {
                        return Err(msg);
                    }
                }
            }
            let od = (axis_ln.is_some() || axis_od.is_some()).then_some((ax_o, ax_d));
            p.kernel.revolve_region_multi(body, src, &profs, axis, od, angle, with_start(ax_o, ax_d, pl), op, &caps)
        });
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Sweep: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_sweep(&mut self, p: &mut Pass, sketch: Id, profiles: &[Id], path_sketch: Id, path: Id, src: Id, op: u8, body: Id) -> bool {
        // The profile and the path are different sketches, each with its own placement (the frame
        // of its plane).
        let prof_pl = self.sketch_place(sketch);
        let path_pl = self.sketch_place(path_sketch);
        let pth = self.sweep_path_encoded(path_sketch, path);
        let res = match (self.encode_profiles_role(p.node, sketch, &profiles, &[], crate::names::Role::Swept), pth) {
            (Some(profs), Some(pth)) => {
                let caps = self.region_cap_names(p.node, &profs);
                p.kernel.sweep_multi(body, src, &profs, prof_pl, &pth, path_pl, op, &caps)
            }
            (None, _) => Err(crate::errors::CoreError::SweepProfileMissing),
            (_, None) => Err(crate::errors::CoreError::SweepPathMissing),
        };
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    /// Combine: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_combine(&mut self, p: &mut Pass, src: Id, sketch: Id, profiles: &[Id], height: f64, op: u8, extent: crate::feature::Extent, down: f64, fill: &[Id], body: Id) -> bool {
        let mut pl = self.sketch_place(sketch);
        let h0 = p.dim("height", height).abs();
        let down = p.dim("down", down);
        // Extent of the tool along the normal: one named computation (see `tool_extent`).
        let (start, h) = self.tool_extent(src, &pl, h0, down, extent, op);
        if start.abs() > 1e-9 {
            pl[3] += pl[2] * start;
            pl[7] += pl[6] * start;
            pl[11] += pl[10] * start;
        }
        // Encode every tool contour and apply them with a single boolean
        // (`combine_region_multi`).
        let res = self
            .encode_profiles_fill(p.node, sketch, profiles, fill)
            .ok_or(crate::errors::CoreError::ProfileNotFound)
            .and_then(|profs| {
                let caps = self.region_cap_names(p.node, &profs);
                p.kernel.combine_region_multi(body, src, &profs, h, op, pl, &caps)
            });
        self.apply_regen(p.node, body, res, p.dirty, p.report, p.kernel, p.emap)
    }

    fn live_fillet_edges(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
        // The two paths are told apart by the kind of query rather than by whether it contains numbers.
        //
        // Asking "are there recorded descriptors" is wrong: `Adjacent(Id(face))` has them, but they are face
        // numbers. Translating them onwards as edges hands the kernel a non-existent edge, and the kernel
        // segfaults, killing the program on "shell, then fillet its face".
        let live = if r.query.is_pick_list() {
            self.live_edge_refs(node_id, src, &r.query.picked_descs(), emap, kernel)
        } else {
            self.resolve_edge_refs(src, r, "ref-what-fillet-edge").unwrap_or_default()
        };
        // And no foreign number reaches the kernel. The kernel does not refuse a non-existent edge, it
        // crashes, so the check belongs on this side. Cheap insurance against a whole class of failures.
        let known: std::collections::HashSet<u32> = kernel.edges(src).into_iter().map(|e| e.id).collect();
        if known.is_empty() {
            return live;
        }
        live.into_iter().filter(|e| known.contains(e)).collect()
    }

    fn live_edge_refs(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
        let cur = kernel.edges(src);
        let translated = self.translate_edge_refs(node_id, src, edges, emap, &cur);
        let before = self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let out = if cur.is_empty() {
            self.resolve_edge_ids(node_id, src, &translated)
        } else {
            self.resolve_edge_ids_in(&cur, node_id, &translated)
        };
        // The fallback fires once rather than on every rebuild.
        //
        // Finding an element by snapshot reveals its current name, and that name has to be written back into
        // the reference. Otherwise the reference stays a positional number from an old file forever and the
        // fallback carries it on every rebuild: the link rests on similarity although the name is already
        // known. The `fallback_silent` guard caught exactly that — one fillet reference lived that way in a
        // real project.
        //
        // The names are recorded element by element rather than as a whole list. Some references may not
        // resolve at all, and their refusal has to stay honest, so only those whose current name is now known
        // are rewritten.
        if self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed) > before {
            let keep = self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed); // Per-element lookups must
                                                                                       // not inflate the
                                                                                       // fallback counter.
            let mut picks: Vec<u32> = Vec::with_capacity(translated.len());
            for &d in &translated {
                let one = if cur.is_empty() { self.resolve_edge_ids(node_id, src, &[d]) } else { self.resolve_edge_ids_in(&cur, node_id, &[d]) };
                picks.push(one.first().copied().unwrap_or(d));
            }
            self.snap_rebinds.store(keep, std::sync::atomic::Ordering::Relaxed);
            if picks != translated {
                self.rewrite_edge_picks(node_id, &picks);
            }
        }
        out
    }

    /// Write the edge names found today back into a feature reference. Hand-picked sets only: a descriptive
    /// query stores no names, and replacing it with a list would take away its meaning.
    fn rewrite_edge_picks(&mut self, node_id: Id, found: &[u32]) {
        let Some(n) = self.timeline.iter_mut().find(|n| n.id == node_id) else { return };
        match &mut n.kind {
            crate::feature::FeatureKind::Fillet { edges, .. } | crate::feature::FeatureKind::Chamfer { edges, .. } => {
                if !edges.query.picked_descs().is_empty() {
                    *edges = crate::refs::Ref::picks(found);
                }
            }
            _ => {}
        }
    }

    /// Names of the inner walls of a shell: pairs of "source face to the name of its wall".
    ///
    /// A wall is not the same face but a new surface produced by offsetting it: `Role::ShellWall` with `src`
    /// set to the original face. The names are seeded during construction, because in the finished body an
    /// outer face and its wall are indistinguishable by id (unlike a pattern, whose copies stay separate until
    /// they are merged).
    /// Names of the turn faces, from the profile edge and the start.
    ///
    /// Without this a thread names none of the faces it produces: in a probe of "cylinder, thread, cut, fillet"
    /// only 6 faces out of 78 were named by recipe while the rest carried a traversal number, so moving one
    /// sketch point disturbed all of them, and with them the edges between them and the fillet references.
    ///
    /// The groove profile is computed from the thread standard and has the same edges (flanks, crest, root) at
    /// any diameter, pitch and length, so the edge number within the profile is a recipe. `split` is the start
    /// number: for a multi-start thread the copies of the groove have to carry different names, or a reference
    /// to a face of the second start would silently lead to the first.
    ///
    /// The list is ordered in blocks by start — start 0 edges, then start 1 edges — which is exactly how the
    /// kernel reads it.
    /// Face references resolved through a witness: by name first, and by a place snapshot when the name misses.
    ///
    /// Eight timeline features hold face references, and matching the descriptor is a single path that breaks on
    /// every improvement to naming while some faces are still positional — a thicken went red that way twice in
    /// a real project. The order matches the one used for edges: name, then a recorded merge, then the single
    /// face of a body, then the snapshot (see `resolve_face_id`).
    ///
    /// The snapshot is refreshed on every successful rebuild: a witness has to speak about today's geometry, or
    /// it becomes a source of misses itself.
    /// A single face through the same witness: like `faces_by_ref`, but for references to one face (a face
    /// offset, a hole axis, the neutral face of a draft).
    fn face_by_ref(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, what: &str) -> Result<crate::refs::Candidate, crate::refs::RefError> {
        match self.resolve_face_ref(src, r, what) {
            Ok(c) => {
                self.capture_face_refs(node_id, src, &[c.desc]);
                Ok(c)
            }
            Err(e) => {
                let picks = r.query.picked_descs();
                let Some(d) = picks.first().and_then(|d| self.resolve_face_id(node_id, src, *d)) else { return Err(e) };
                let pool = self.face_pool(src);
                let Some(c) = pool.iter().find(|c| c.desc == d).copied() else { return Err(e) };
                self.capture_face_refs(node_id, src, &[c.desc]);
                Ok(c)
            }
        }
    }

    fn faces_by_ref(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        let out = match self.resolve_face_refs(src, r, what) {
            Ok(v) => v,
            Err(e) => {
                let picks = r.query.picked_descs();
                if picks.is_empty() {
                    return Err(e);
                }
                let healed: Vec<u32> = picks.iter().filter_map(|d| self.resolve_face_id(node_id, src, *d)).collect();
                if healed.len() != picks.len() {
                    return Err(e);
                }
                healed
            }
        };
        self.capture_face_refs(node_id, src, &out);
        Ok(out)
    }

    /// Names of the remaining drill faces: the tip, the countersink and its bottom. The wall is named
    /// separately, after the sketch point that placed the hole.
    /// A fillet surface name for every named edge of a body, as "edge to name" pairs.
    ///
    /// A fillet is not limited to the selected edges: the kernel continues the surface across tangent
    /// neighbours, and those faces are produced by an edge too — just not the one that was picked. Their recipe
    /// is the same, so names are prepared for every named edge of the source (measured: otherwise 2 to 4
    /// nameless faces per operation).
    fn blend_names_all(&mut self, feature: Id, src: Id, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
        let all: Vec<u32> = kernel.edges(src).into_iter().map(|e| e.id).collect();
        // A name is issued only to something that can be told apart. While two edges of a body share one
        // number (the ordinal within a pair of faces is not yet assigned by recipe), naming their surfaces
        // identically makes the reference ambiguous and it would lead to two faces at once. Measured: a mirror
        // produced 7 such pairs out of 20 edges, giving 3 duplicate face names on the following chamfer.
        let mut cnt: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for e in &all {
            *cnt.entry(*e).or_default() += 1;
        }
        all.iter()
            .copied()
            .filter(|e| crate::names::NameTable::is_named(*e) && cnt.get(e) == Some(&1))
            .collect::<Vec<u32>>()
            .into_iter()
            .flat_map(|e| [e, self.intern_name(feature, crate::names::Role::Blend, e as Id)])
            .collect()
    }

    fn hole_tool_names(&mut self, feature: Id) -> Vec<u32> {
        (0..3).map(|k| self.intern_name(feature, crate::names::Role::HoleTool, k as Id)).collect()
    }

    fn relief_names(&mut self, feature: Id) -> Vec<u32> {
        // Disabled for now, and this is a recorded decision rather than a forgotten loose end.
        //
        // The names themselves are correct, but issuing them changes the edge naming scheme of already saved
        // documents: edges that were positional (because they met a nameless relief face) become named. Carrying
        // references across that transition does not hold — measured: 12 of 36 edges of an R2.0 fillet moved to
        // the wrong place and it stopped building entirely. Landing on the wrong element silently is worse than
        // staying nameless.
        //
        // To be re-enabled only once reference migration across a naming-scheme change is worked out and held
        // by a threshold check.
        let mut out = Vec::with_capacity(16);
        for tool in 0..4u32 {
            for surf in 0..4u32 {
                let name = crate::names::GeoName { feature, role: crate::names::Role::ThreadRelief, src: tool as Id, split: surf as u16 };
                out.push(self.names.intern_face(name));
            }
        }
        out
    }

    /// Names of a thickened plate: pairs of "source face name to the name of its offset side" and "edge name to
    /// the name of the wall it produced", as flat lists (the kernel reads them in pairs).
    ///
    /// Without this a thicken names nothing: the whole plate takes positional numbers and any reference to its
    /// faces rests on the numbering alone — 6 nameless faces out of 6 in a probe, 18 out of 84 in a scenario
    /// document. The operation history is complete and unambiguous (measured): a face produces the offset side
    /// and an edge produces its wall.
    fn thicken_names(&mut self, feature: Id, src: Id, kernel: &dyn crate::feature::Kernel) -> (Vec<u32>, Vec<u32>) {
        let faces: Vec<u32> = self.regen_faces.get(&src).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        // The edges are asked of the kernel rather than of the cache: in the middle of a rebuild `regen_edges`
        // is not filled yet and the list comes out empty, leaving the walls nameless although names were
        // prepared for them.
        let edges: Vec<u32> = kernel.edges(src).into_iter().map(|e| e.id).collect();
        let mut fmap = Vec::with_capacity(faces.len() * 2);
        for f in faces {
            let name = crate::names::GeoName { feature, role: crate::names::Role::Thickened, src: f as Id, split: 0 };
            let d = self.names.intern_face(name);
            fmap.push(f);
            fmap.push(d);
        }
        let mut emap = Vec::with_capacity(edges.len() * 2);
        for e in edges {
            let name = crate::names::GeoName { feature, role: crate::names::Role::ThickenWall, src: e as Id, split: 0 };
            let d = self.names.intern_face(name);
            emap.push(e);
            emap.push(d);
        }
        (fmap, emap)
    }

    fn groove_names(&mut self, feature: Id, edges: usize, starts: u32) -> Vec<u32> {
        let mut out = Vec::with_capacity(edges * starts.max(1) as usize);
        for start in 0..starts.max(1) {
            for e in 0..edges {
                let name = crate::names::GeoName { feature, role: crate::names::Role::ThreadGroove, src: e as Id, split: start as u16 };
                out.push(self.names.intern_face(name));
            }
        }
        out
    }

    fn shell_wall_names(&mut self, feature: Id, src: Id) -> Vec<(u32, u32)> {
        let faces: Vec<u32> = self.regen_faces.get(&src).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
        faces
            .into_iter()
            // Only structurally named faces get a name: for a positional one there is nothing to derive a wall
            // name from.
            .filter(|f| crate::names::NameTable::is_named(*f))
            .map(|f| {
                let name = crate::names::GeoName { feature, role: crate::names::Role::ShellWall, src: f as Id, split: 0 };
                (f, self.names.intern_face(name))
            })
            .collect()
    }

    /// Face names per pattern instance: `seeds[k]` holds pairs of "source face to its name in copy k".
    ///
    /// An instance is not the same face but the image of a face under the k-th transform, so the name comes from
    /// the recipe: `Role::Instance` with `src` set to the descriptor of the source face and `split` to the copy
    /// number. Without this every copy carries the names of the original (measured: 18 faces sharing 6 names)
    /// and a reference to a face of the second copy silently resolves to the first, putting a feature in the
    /// wrong place.
    ///
    /// Copy zero keeps the original names: it is the source in its own place, and references made before the
    /// pattern existed have to keep working.
    fn instance_name_seeds(&mut self, feature: Id, src: Id, count: usize) -> Vec<Vec<(u32, u32)>> {
        let faces: Vec<u32> = self.regen_faces.get(&src).map(|fs| fs.iter().map(|f| f.id).collect()).unwrap_or_default();
        if faces.is_empty() || count < 2 {
            return Vec::new();
        }
        (0..count)
            .map(|k| {
                if k == 0 {
                    return Vec::new();
                }
                faces
                    .iter()
                    .map(|&f| {
                        let name = crate::names::GeoName { feature, role: crate::names::Role::Instance, src: f as Id, split: k as u16 };
                        (f, self.names.intern_face(name))
                    })
                    .collect()
            })
            .collect()
    }

    /// Cascade of the unbuilt: the body of a node will not appear, and its consumers are not built either.
    ///
    /// This is how a suppressed base feature behaves (there is nothing to skip, no source to copy) and any node
    /// whose input is already in the cascade. The body is removed from view and marked unbuilt: without the
    /// mark a consumer would try to build on a non-existent body and fail with an obscure error instead of an
    /// honest "the source was not built".
    fn cascade_unbuilt(&mut self, kind: &crate::feature::FeatureKind, unbuilt: &mut std::collections::HashSet<Id>) {
        for b in kind.bodies() {
            self.drop_body_from_view(b);
            unbuilt.insert(b);
        }
    }

    /// Component isolation: why this node must not be built, or `None` when it may be.
    ///
    /// Only a part builds bodies — an assembly holds none — and the references of a feature are confined to its
    /// owning component. Three kinds of cross-component links are forbidden: a body in an assembly or at the
    /// root; an input body or sketch from another component; and an input sketch placed on a face of another
    /// component's body. The last is allowed exactly when an explicit external reference exists, which is
    /// controlled top-down design rather than an accidental link.
    fn isolation_error(&self, i: usize, kind: &crate::feature::FeatureKind) -> Option<crate::errors::CoreError> {
        let owner = self.timeline[i].parent?;
        if !self.component_is_part(owner) {
            return Some(crate::errors::CoreError::BodyOnlyInPart);
        }
        if let Some(bad) = kind.inputs().into_iter().find(|&inp| self.ref_owner(inp).is_some_and(|ro| ro != owner)) {
            return Some(crate::errors::CoreError::CrossComponentInput { input: bad });
        }
        // An input sketch on a face of another component's body is forbidden, except with an explicit external
        // reference, where the cross-component link is authorised and resolves into local space.
        if let Some(bad) = kind.inputs().into_iter().find(|&inp| {
            self.sketch_plane_body(inp)
                .and_then(|pb| self.body_owner(pb).map(|ro| (pb, ro)))
                .is_some_and(|(pb, ro)| ro != owner && !self.external_authorized(owner, pb))
        }) {
            return Some(crate::errors::CoreError::SketchOnForeignFace { input: bad });
        }
        None
    }

    /// Extent of a tool along the sketch normal: where to start and how far to go.
    ///
    /// An extrude has always called `feature::extrude_extent` for this while a combine computed the same thing
    /// inline over a hundred lines, and only the latter handled "through all" and coincident end faces. Any
    /// change to the meaning of an extent then had to be made in two different places; here it is one.
    ///
    /// Returns `(start along the normal, length)`. The target body `src` is needed so the extent knows the
    /// geometry it cuts: "through all" without a bounding box is a blind pocket, not a through cut.
    #[allow(clippy::too_many_arguments)]
    fn tool_extent(&self, src: Id, pl: &[f64; 12], h: f64, down: f64, extent: crate::feature::Extent, op: u8) -> (f64, f64) {
        let n = [pl[2], pl[6], pl[10]];
        let o = [pl[3], pl[7], pl[11]];
        if extent.through {
            // Span the whole of body `src` along the normal, both ways: the bounding box gives a start below it
            // and a length of the span plus margins.
            return match self.body_span_along(src, o, n) {
                Some((tmin, tmax)) => {
                    let margin = ((tmax - tmin).abs() * 0.1).max(1.0);
                    (tmin - margin, (tmax - tmin) + 2.0 * margin)
                }
                // Fallback: the mesh of `src` has no bounding box (the body is not tessellated yet, or the mesh
                // is empty). "Through all" has to stay a through cut rather than degenerate into a blind pocket
                // of nominal depth.
                None => {
                    let reach = (h.abs() * 100.0).max(1000.0);
                    (-reach, 2.0 * reach)
                }
            };
        }
        // The tool direction follows the extrude rules: one-sided, both ways or two-sided.
        let (mut start, mut total) = crate::feature::extrude_extent(h, down, extent.reach);
        // For a one-sided cut, an end face of the tool coincident with a face of the body leaves a cap behind
        // and the hole comes out closed. When the end of the tool almost touches the body boundary it crosses,
        // it is pushed outwards by a small clearance: a cut exactly as deep as the wall becomes a through cut,
        // while an honest pocket (its end far from the far face) keeps its exact depth.
        if op == 0 && extent.reach != crate::feature::Reach::BothWays && down.abs() <= 1e-9 {
            let eps = (h.abs() * 0.01).max(0.05);
            let (mut lo, mut hi) = (start, start + total);
            if let Some((tmin, tmax)) = self.body_span_along(src, o, n) {
                let touch = ((tmax - tmin).abs() * 1e-3).max(1e-4);
                if hi > 1e-9 && (hi - tmax).abs() <= touch {
                    hi = tmax + eps;
                }
                if lo < -1e-9 && (lo - tmin).abs() <= touch {
                    lo = tmin - eps;
                }
            }
            // The entry end face sits on the sketch plane (coordinate 0), which usually lies on a face of the
            // body, so a coincident end face would leave a cap. The coincidence is broken by a micro clearance:
            // 0.1 mm would shave a visible step when there is material behind the sketch plane, while a 1
            // micrometre seam is invisible and removes the coplanarity reliably.
            let seam = 1.0e-3;
            if lo.abs() < 1e-9 {
                lo -= seam;
            }
            if hi.abs() < 1e-9 {
                hi += seam;
            }
            start = lo;
            total = hi - lo;
        }
        (start, total)
    }

    fn apply_regen(
        &mut self,
        node_id: Id,
        body: Id,
        res: Result<(crate::geom::Mesh, Vec<crate::geom::MeshFace>), crate::errors::CoreError>,
        dirty: &mut std::collections::HashSet<Id>,
        report: &mut crate::feature::RegenReport,
        kernel: &dyn crate::feature::Kernel,
        emap: &mut EdgeRenames,
    ) -> bool {
        match res {
            Ok((mesh, faces)) => {
                self.set_body_mesh(body, mesh);
                dirty.insert(body);
                self.regen_faces.insert(body, faces); // Faces into the model, for resolving references by id.
                // Absorptions come before the names of split pieces and edges: merging faces changes which
                // names exist at all, and everything after this has to work from the new picture.
                //
                // Stale records are cleared first: a face whose name is alive again yielded to nobody, and an
                // old record about it only misleads.
                let live_faces: Vec<u32> = self.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
                self.names.forget_absorbed(&live_faces);
                for (loser, winner) in kernel.absorbed_names(body) {
                    self.names.absorb(loser, winner);
                }
                self.name_face_splits_of(body, kernel); // First finish naming the split face pieces.
                self.name_seam_faces_of(node_id, body, kernel); // Then the seams: faces with no provenance,
                                                                // named by their neighbours.
                self.name_edges_of(body, kernel, emap); // Then the edges, derived from the face names.
                // Sheet or solid: asked of the kernel and recorded in the document. A sheet has no volume, and
                // everything that computes mass, cuts toolpaths or enforces "one part is one body" has to tell
                // them apart without guessing from the geometry.
                if let Some(i) = self.mesh_index(body) {
                    self.bodies[i].sheet = kernel.body_is_sheet(body);
                }
                // The report carries the faces after renaming rather than the ones the kernel returned.
                //
                // The application lays out body faces from the report: the pointer hits those, and those go into
                // the file. Reference resolution meanwhile goes through `regen_faces`, where the names are
                // already complete. Putting the pre-rename list into the report leaves two places knowing
                // different names for one face: clicking a piece of a split face yields a positional number, the
                // feature records it, and the very next rebuild answers that the face no longer exists.
                //
                // Measured symptom: the opposite face offsets without trouble while this one does not — the
                // opposite face being the one that kept its original name.
                let named = self.regen_faces.get(&body).cloned().unwrap_or_default();
                report.built.push((body, named));
                self.regen_errors.remove(&node_id); // The feature built, so its error mark is cleared.
                true
            }
            Err(e) => {
                // The body stays at its last good state, and that is a rule rather than an omission.
                //
                // Removing the geometry of a failed node here looks like keeping the document in step with the
                // timeline, but it is wrong: the node goes red honestly while the model stays on screen for the
                // author to repair. Taking the part away at that moment is the worst possible response.
                //
                // There is one condition: the node has to go red. Silence with the previous geometry is the real
                // failure, and it is cured by marking the node dirty (see `mark_copiers_dirty`) rather than by
                // removing the body.
                self.regen_errors.insert(node_id, e.clone()); // Mark the node; the mark survives the
                                                              // pass-through fallback.
                report.errors.push((node_id, e));
                false
            }
        }
    }
}
