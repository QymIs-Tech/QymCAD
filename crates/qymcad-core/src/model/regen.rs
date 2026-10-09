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
/// WHERE THE RESULT OF ONE REBUILD LANDS: the node it belongs to, the kernel that built it, and the
/// three books kept for the whole pass - what changed, what went wrong, and how edges were renamed.
///
/// Every branch of the rebuild ends by handing its result over with exactly these five, and they were
/// spelled out at thirty-seven call sites.
struct Landing<'a, 'b> {
    /// The node being rebuilt: the owner of every name minted while it is built.
    node: Id,
    /// The bodies that changed, whose consumers must therefore rebuild.
    dirty: &'a mut std::collections::HashSet<Id>,
    report: &'a mut crate::feature::RegenReport,
    kernel: &'b dyn crate::feature::Kernel,
    /// "Old edge number -> new name" for the bodies built so far in this pass.
    emap: &'a mut EdgeRenames,
}

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

/// A NODE'S PARCEL in a wave: the node, the body it writes, and the work that builds it.
struct Parcel {
    node: Id,
    body: Id,
    job: crate::feature::KernelJob,
}

/// A parcel sent to another thread, with the worker that holds the bodies it reads.
struct Travelling {
    parcel: Parcel,
    worker: Box<dyn crate::feature::KernelWorker>,
}

/// A parcel back from its thread: what it built, and the worker to give back to the shared kernel.
struct Returned {
    node: Id,
    computed: Computed,
    worker: Box<dyn crate::feature::KernelWorker>,
}

/// What a node's work came to, kept until the walk writes it down: the body it writes and the result.
struct Computed {
    body: Id,
    res: Result<crate::geom::Built, crate::errors::CoreError>,
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

/// WHICH BODY IS BEING REBUILT, AND HOW THE RESULT JOINS IT.
///
/// The same three arguments trail the end of nearly every rebuild branch. Together they are one thing:
/// take `src`, make something, and add or cut it into `body` according to `op`.
#[derive(Clone, Copy)]
pub struct BodyOp {
    /// The body the operation reads.
    pub src: Id,
    /// The kernel's own code for join or cut.
    pub op: u8,
    /// The body the result lands in.
    pub body: Id,
}

/// A HELICAL CUT ALONG AN EDGE: where it runs, how long, and how it eases in and out.
///
/// A thread and an auger differ only in the profile of the groove; everything about WHERE the helix goes
/// is the same six things, and both branches took them one by one.
#[derive(Clone, Copy)]
pub(crate) struct HelixRun {
    /// The body the helix is cut into.
    pub src: Id,
    /// The circular edge it follows.
    pub edge: u32,
    /// How far along the axis it runs.
    pub length: f64,
    /// A run-up before the full depth is reached, so the tool does not bite at once.
    pub lead_in: f64,
    /// The same at the far end.
    pub lead_out: f64,
    /// The body the result lands in.
    pub body: Id,
}

/// THE DRAFT ITSELF, apart from the faces it is applied to: the face that stays put, the angle, and
/// which side of it leans.
pub(crate) struct DraftShape {
    /// The neutral face - the one that keeps its size while the others lean.
    pub neutral: crate::refs::Ref,
    /// The lean, in degrees.
    pub angle: f64,
    /// Lean the other way.
    pub flip: bool,
}

/// HOW FAR AN EXTRUSION RUNS AND WHICH CONTOURS STAY SOLID.
///
/// The body spans [-down, +height] along the sketch normal, `reach` says which of the two sides is
/// actually used, and `fill` names the nested contours that are NOT to become holes.
#[derive(Clone, Copy)]
pub struct ExtrudeSpan<'a> {
    pub height: f64,
    pub down: f64,
    pub reach: crate::feature::Reach,
    pub fill: &'a [Id],
}

/// THE SAME FOR A JOIN OR A CUT, where the stop can also be "the whole way through".
#[derive(Clone, Copy)]
pub struct CombineSpan<'a> {
    pub height: f64,
    pub down: f64,
    pub extent: crate::feature::Extent,
    pub fill: &'a [Id],
}

/// WHICH PROFILE OF WHICH SKETCH. Never one without the other.
#[derive(Clone, Copy)]
pub(crate) struct Profile<'a> {
    pub sketch: Id,
    pub profiles: &'a [Id],
}

/// HOW FAR A REVOLVE TURNS, and which way it goes from the sketch plane.
#[derive(Clone, Copy)]
pub struct RevolveTurn {
    /// Degrees. 360 makes a full body of revolution.
    pub angle: f64,
    pub reach: crate::feature::Reach,
}

/// THE AXIS A REVOLVE TURNS ABOUT, in the three ways it can be given.
#[derive(Clone, Copy)]
pub struct RevolveAxis {
    /// A world axis by number, when neither of the two below is set.
    pub axis: u8,
    /// A datum axis.
    pub datum: Id,
    /// A line of the sketch itself.
    pub line: Id,
}

/// THE SHAPE OF A CHAMFER: symmetric, two-distance or angled, and which face the sizes are measured from.
#[derive(Clone, Copy)]
pub struct ChamferShape {
    pub mode: crate::feature::ChamferMode,
    /// The second distance (or the angle), for the asymmetric modes.
    pub d2: f64,
    /// Measure from the other side of the edge.
    pub flip: bool,
    /// The face the distances are measured from; zero lets the kernel choose.
    pub ref_face: u32,
}

/// WHERE A HOLE IS DRILLED: either a face found by recipe, or the isolated points of a sketch.
///
/// The two ways are exclusive - `sketch` non-zero means the points, and then `face` is not consulted -
/// and they used to travel as five loose arguments in the middle of a signature of fourteen.
pub(crate) struct HoleSite {
    /// The face, found by recipe. Not consulted when a sketch places the holes.
    pub face: crate::refs::Ref,
    /// The point and the normal recorded when the reference was made - the fallback if the recipe misses.
    pub point: [f64; 3],
    pub normal: [f64; 3],
    /// The sketch whose isolated points place the holes; zero means the face above.
    pub sketch: Id,
    /// Drill against the sketch's normal.
    pub flip: bool,
}

/// WHAT DRILLS IT: the cylinder, and the counterbore or countersink above it.
#[derive(Clone, Copy)]
pub struct HoleTool {
    /// Plain, counterbored or countersunk - the kernel's own code.
    pub kind: u8,
    /// The through cylinder.
    pub diameter: f64,
    pub depth: f64,
    /// The wider part at the top; zero when there is none.
    pub dia2: f64,
    pub depth2: f64,
}

/// ONE DIRECTION OF A LINEAR PATTERN: the step vector and how many copies go along it.
///
/// The rebuild used to take these twelve numbers and three counts as fifteen separate arguments, and the
/// signature ran to sixteen places. Reading it, one had to count commas to tell `dy2` from `dz2`.
#[derive(Clone, Copy)]
pub struct ArrayAxis {
    /// The step, as a vector in the body's space.
    pub d: [f64; 3],
    /// How many copies along it, the original included. One or less means this direction is unused.
    pub count: u32,
}

impl ArrayAxis {
    /// A DIRECTION THAT IS NOT USED. A grid of one row still has three directions; two of them are this.
    pub fn none() -> Self {
        ArrayAxis { d: [0.0; 3], count: 1 }
    }
}

impl<'a> Pass<'a> {
    /// The five things a finished branch hands its result over with.
    fn landing(&mut self) -> Landing<'_, 'a> {
        Landing { node: self.node, dirty: self.dirty, report: self.report, kernel: self.kernel, emap: self.emap }
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
            faces.iter().filter(|f| f.id != 0 && dot(f.normal, key.normal) > 0.9).min_by(|a, b| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal)).map(|f| f.id)
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
                        sfix.push((sk.id, newid, format!("rebind-sketch-face#{} -> {newid}", key.id)));
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

    /// DOES THIS NODE HAVE TO BE REBUILT, given what is dirty so far.
    ///
    /// Asked in three places - the plan, the waves and the rebuild itself - and the three had grown their own
    /// copies of the answer. A copy that forgets one of the reasons (a moved sketch plane, the live input of a
    /// mirrored part) does not fail: it silently rebuilds less than it should, and the document comes out
    /// almost right.
    fn node_needs(&self, i: usize, dirty: &std::collections::HashSet<Id>) -> bool {
        let Some(nd) = self.timeline.get(i) else { return false };
        let self_dirty = nd.dirty || self.regen_errors.get(&nd.id).is_some_and(|e| !e.retryable());
        self_dirty || self.node_reads_any(i, dirty)
    }

    /// DOES NODE `i` READ ANYTHING IN `set` - its inputs, the body under its sketch, or the live source of a
    /// mirrored or instanced part.
    ///
    /// One answer for two questions: "is its input dirty" (`node_needs`) and "is its input still to be rebuilt"
    /// (`ready_from`). The second used to be a copy of the first that had lost the part mirror and the part
    /// instance on the way.
    pub(super) fn node_reads_any(&self, i: usize, set: &std::collections::HashSet<Id>) -> bool {
        let Some(nd) = self.timeline.get(i) else { return false };
        let kind = &nd.kind;
        kind.inputs().iter().any(|id| set.contains(id))
            || kind.inputs().iter().any(|&inp| self.sketch_plane_body(inp).is_some_and(|pb| set.contains(&pb)))
            || matches!(kind, FeatureKind::MirrorPart { src_comp, .. } if self.active_body_before(*src_comp, i).is_some_and(|b| set.contains(&b)))
            || matches!(kind, FeatureKind::PartInstance { .. } | FeatureKind::ComponentPattern { .. })
                && kind.copy_source().and_then(|s| self.active_body_before(s, i)).is_some_and(|b| set.contains(&b))
    }

    /// THE NODES OF A REBUILD LAID OUT IN WAVES: those that can be computed at the same time stand together.
    ///
    /// The first wave holds the nodes whose inputs nothing in this rebuild produces; the second, those that
    /// depend only on the first, and so on. Nothing here touches geometry: it is arithmetic over the graph the
    /// timeline already carries (`kind.inputs()`), and it answers whether there is anything to compute side by
    /// side at all.
    ///
    /// The order within a wave is the order of the timeline, and the union of the waves is exactly the set a
    /// sequential pass would rebuild: the waves say WHEN, never WHETHER.
    pub fn rebuild_waves(&self) -> Vec<Vec<Id>> {
        let plan = self.regen_plan();
        let want: std::collections::HashSet<Id> = plan.nodes.iter().copied().collect();
        // what each output of this rebuild is ready after: the wave of the node that made it
        let mut ready_after: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
        let mut waves: Vec<Vec<Id>> = Vec::new();
        for (i, nd) in self.timeline.iter().enumerate() {
            if !want.contains(&nd.id) {
                continue;
            }
            // THE INPUTS THAT THIS REBUILD ITSELF PRODUCES. An input made outside it (a body nobody touches)
            // is ready before the first wave and puts no one in a queue.
            let mut after = 0usize;
            let note = |id: Id, after: &mut usize| {
                if let Some(w) = ready_after.get(&id) {
                    *after = (*after).max(*w + 1);
                }
            };
            for id in nd.kind.inputs() {
                note(id, &mut after);
                // a sketch drawn on a face of a body waits for that body, and the timeline does not say so
                // through `inputs`
                if let Some(pb) = self.sketch_plane_body(id) {
                    note(pb, &mut after);
                }
            }
            // the dynamic inputs: a mirrored part and a pattern instance follow the ACTIVE body of their
            // source, which is not in `inputs` either
            if let Some(src_comp) = nd.kind.copy_source() {
                if let Some(b) = self.active_body_before(src_comp, i) {
                    note(b, &mut after);
                }
            }
            if waves.len() <= after {
                waves.resize(after + 1, Vec::new());
            }
            waves[after].push(nd.id);
            for out in nd.kind.declares() {
                ready_after.insert(out, after);
            }
        }
        waves
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
            let needs = self.node_needs(i, &dirty);
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
        // the part of `unbuilt` that is so because a node above FAILED, not because it was suppressed: what stands
        // on it is red with the reason, while what stands on a suppressed node is quietly left out with it
        let mut broken: std::collections::HashSet<Id> = std::collections::HashSet::new();
        // every body some node makes or an import brings: a source outside it was deleted with its node
        let made: std::collections::HashSet<Id> = self.timeline.iter().flat_map(|n| n.kind.bodies()).chain(self.imported_bodies.iter().copied()).collect();
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
                pass!($node, kernel)
            };
            // a node computed by a worker is APPLIED with that worker's kernel: the names of its new faces live
            // there, not in the shared one
            ($node:expr, $k:expr) => {
                Pass { kernel: $k, vars: &vars, dims: feat_dims.get(&$node), emap: &mut emap, dirty: &mut dirty, report: &mut report, node: $node }
            };
        }
        let plan_total = self.regen_plan().nodes.len();
        // THE WAVES, AND WHAT HAS ALREADY BEEN COMPUTED FROM THEM.
        //
        // A wave is a set of nodes that do not depend on one another (see `rebuild_waves`), so their geometry
        // can be built at the same time. The document itself stays on this thread: a worker is given the bodies
        // its node needs and nothing else, and what it built comes home before anything is written down.
        let hands = kernel.workers();
        let mut computed: std::collections::HashMap<Id, Computed> = std::collections::HashMap::new();
        let mut work = 0usize;
        // THE ORDER OF THE WALK: the timeline, except that a batch just computed is written down straight away.
        //
        // Held back until the walk reached each of them, the results of a batch kept every chain standing on them
        // waiting: the first batch took the 27 heads of the scenario document, and from then on each step found
        // one node ready - batches [27, 1, 4, 1, 1, 1], and the time inside the nodes overlapped by nothing.
        // Written down at once, a head frees its chain for the next batch.
        //
        // Safe for the reason the batch itself is: a member depends on nothing standing between the node the walk
        // is on and itself (`ready_from`), and nothing standing there may read what a member makes - that would
        // be a reference to a node below it.
        let mut next_up: Vec<usize> = Vec::new();
        let mut walked: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut cursor = 0usize;
        loop {
            let i = match next_up.pop() {
                Some(j) => j,
                None => {
                    while walked.contains(&cursor) {
                        cursor += 1;
                    }
                    if cursor >= self.timeline.len() {
                        break;
                    }
                    cursor
                }
            };
            walked.insert(i);
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
                        if self.apply_regen(Landing { node: node_id, dirty: &mut dirty, report: &mut report, kernel, emap: &mut emap }, b, res) {
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
            // Stands on a body whose node was deleted: red with that reason, and what stands on it red in turn.
            // A body read without being consumed - the source of a face copy, a patch, a trim - counts the same when its
            // node was deleted: the live body it leaves behind is not a source, and building on it made the node
            // green step by step and red rebuilt from the start.
            let deleted_input = kind.inputs().iter().any(|s| *s != 0 && !made.contains(s) && self.dead_bodies.contains(s));
            if deleted_input || kind.consumed().iter().any(|s| *s != 0 && !made.contains(s)) {
                self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                broken.extend(kind.bodies());
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            if kind.inputs().iter().any(|id| unbuilt.contains(id)) {
                if kind.inputs().iter().any(|id| broken.contains(id)) {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyNotBuilt);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyNotBuilt));
                    broken.extend(kind.bodies());
                }
                self.cascade_unbuilt(&kind, &mut unbuilt);
                continue;
            }
            // External and base dependency: a node whose sketch sits on a face of a body (its own, or an
            // external one through an `ExternalRef`) rebuilds when that source body rebuilds, because the face
            // frame may have moved. The source body has to come earlier in the timeline.
            //
            // The bodies of the node are captured before the match, whose branches destructure `kind`.
            let out_bodies = kind.bodies();

            // A node that failed earlier is recomputed. Otherwise it is not rebuilt on the next pass, there is
            // no fresh error, and the "do not build on a failure" cascade does not see it, so everything above
            // it builds on a ghost again. Simply marking its bodies unbuilt is not an option either: the error
            // may no longer reproduce (the input was fixed), and that would freeze a working chain forever.
            let needs = self.node_needs(i, &dirty);
            if needs {
                work += 1;
            }
            let started = needs.then(std::time::Instant::now);
            // Isolation: only a part builds bodies, and references are confined to the owning component.
            if needs && kind.body().is_some() {
                if let Some(err) = self.isolation_error(i, &kind) {
                    report.errors.push((node_id, err));
                    continue;
                }
            }
            // A VALUE WRITTEN AS AN EXPRESSION THAT NO LONGER COUNTS - a parameter it names was deleted - makes the node
            // red with the expression's own words; the body stays at its last good state. Built from the number the
            // expression last gave, the node stood green on a value nothing in the document holds any more.
            if needs {
                if let Some(e) = self.dim_expr_error(node_id, &vars) {
                    self.regen_errors.insert(node_id, e.clone());
                    report.errors.push((node_id, e));
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
            // A UNION OR AN INTERSECTION OF TWO BODIES HAS NO SINGLE SOURCE TO PASS THROUGH: a copy of the first took the
            // second out of the part - an empty intersection of two touching pieces lost half the material - so it
            // shows nothing of its own and its two inputs stay in sight (see `consumed_bodies`). A cut does pass
            // through: the base without the tool, when the tool takes nothing, is the base.
            let consumed_src = kind.consumed_body().filter(|_| !matches!(kind, FeatureKind::BodyBoolean { op: 1 | 2, .. }));
            let out_body = kind.body();
            // an addition to the body and a shell of it never make pieces of a whole body; a cut or an intersection
            // may leave islands on purpose, a pattern or a mirror lays copies, and splitting a body is what makes pieces
            let keeps_one_piece = matches!(
                kind,
                FeatureKind::Combine { op: 1, .. } | FeatureKind::Revolve { op: 1, .. } | FeatureKind::Sweep { op: 1, .. } | FeatureKind::Loft { op: 1, .. } | FeatureKind::Shell { .. }
            );
            let mut clear = true;
            // EVERYTHING THAT IS READY RIGHT NOW, COMPUTED TOGETHER.
            //
            // A wave says which nodes do not depend on one another (`rebuild_waves`), but a wave is not what
            // can be computed at THIS moment: a node of the same wave standing further down the timeline may
            // take its source from a node standing between here and there, and that source has not been
            // written into the document yet. Preparing it then resolves its references against geometry that
            // is not there - measured on the scenario document as 13 refusals and half the bodies missing.
            //
            // So the rule is readiness rather than the wave: from here on, a node joins the batch while
            // nothing it needs is made by a node in between. Preparing a parcel changes the document (it mints
            // the names of the faces to come), so preparation stays here, in timeline order, exactly as a
            // sequential pass would do it.
            if needs && hands > 1 && !suppressed && !computed.contains_key(&node_id) {
                let held: std::collections::HashSet<Id> = computed.keys().copied().collect();
                let ready = self.ready_from(i, &dirty, &held, &walked, limit);
                if ready.len() > 1 {
                    let mut parcels: Vec<Parcel> = Vec::new();
                    for &j in &ready {
                        let nid = self.timeline[j].id;
                        let kind_here = self.timeline[j].kind.clone();
                        let mut p = pass!(nid);
                        if let Some((out, job)) = self.prep_node(&mut p, &kind_here) {
                            parcels.push(Parcel { node: nid, body: out, job });
                        }
                    }
                    // A body wanted by two parcels keeps both of them here: the shape cannot be in two threads
                    // at once, and copying it would cost what the work costs.
                    let mut wanted: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
                    for parcel in &parcels {
                        for b in parcel.job.inputs() {
                            *wanted.entry(*b).or_insert(0) += 1;
                        }
                    }
                    let (mut alone, mut here): (Vec<_>, Vec<_>) = (Vec::new(), Vec::new());
                    for parcel in parcels {
                        if parcel.job.inputs().iter().all(|b| wanted.get(b).copied().unwrap_or(0) <= 1) {
                            alone.push(parcel);
                        } else {
                            here.push(parcel);
                        }
                    }
                    // the parcels that travel: each with the bodies it needs, taken out of the shared kernel
                    let mut sent: Vec<Travelling> = Vec::new();
                    for parcel in alone {
                        match kernel.split_off(parcel.job.inputs()) {
                            Some(worker) => sent.push(Travelling { parcel, worker }),
                            None => here.push(parcel),
                        }
                    }
                    if !sent.is_empty() {
                        report.waves.push(sent.len());
                        // as many threads as the kernel allows, each taking its share of the parcels in turn
                        let mut lanes: Vec<Vec<Travelling>> = (0..hands.min(sent.len())).map(|_| Vec::new()).collect();
                        for (n, parcel) in sent.into_iter().enumerate() {
                            let lane = n % lanes.len();
                            lanes[lane].push(parcel);
                        }
                        let mut back: Vec<Returned> = Vec::new();
                        std::thread::scope(|scope| {
                            let mut running = Vec::new();
                            for lane in lanes {
                                running.push(scope.spawn(move || {
                                    lane.into_iter()
                                        .map(|Travelling { parcel, worker }| {
                                            let res = parcel.job.run(worker.kernel());
                                            Returned { node: parcel.node, computed: Computed { body: parcel.body, res }, worker }
                                        })
                                        .collect::<Vec<_>>()
                                }));
                            }
                            for h in running {
                                if let Ok(done) = h.join() {
                                    back.extend(done);
                                }
                            }
                        });
                        // THE BODIES COME HOME AT ONCE, before anything else runs: the five node kinds still
                        // computed in place take their source from the shared kernel, and a source that had
                        // travelled would not be there. What a worker built carries its own face names inside
                        // the shape, so applying it later from the shared kernel answers the same.
                        for Returned { node, computed: done, worker } in back {
                            kernel.absorb(worker);
                            computed.insert(node, done);
                        }
                    }
                    // and the ones that stayed: computed here, on the shared kernel
                    for Parcel { node, body, job } in here {
                        let res = job.run(kernel);
                        computed.insert(node, Computed { body, res });
                    }
                    // written down next, in timeline order, before the walk moves on (see the order of the walk)
                    for &j in ready.iter().rev() {
                        if j != i && computed.contains_key(&self.timeline[j].id) {
                            next_up.push(j);
                        }
                    }
                }
            }

            // ONE DOOR FOR EVERY NODE THAT HAS A PARCEL. Before, each kind had its own arm here calling its own
            // rebuilder; now the kind names the parcel and the three steps are the same for all of them -
            // prepare, compute, apply. That is what lets a wave be computed at once (see `prep_node`): the same
            // parcels, several at a time.
            let mut done = false;
            if let Some(Computed { body: out, res }) = computed.remove(&node_id) {
                // already built, by this thread or another: what is left is to write it into the document
                let mut p = pass!(node_id);
                clear = self.apply_regen(p.landing(), out, res);
                done = true;
            } else if needs {
                let mut p = pass!(node_id);
                if let Some((out, job)) = self.prep_node(&mut p, &kind) {
                    let res = job.run(p.kernel);
                    clear = self.apply_regen(p.landing(), out, res);
                    done = true;
                }
            }
            match kind {
                _ if done => {}
                // Datums are resolved unconditionally (they are cheap) and in timeline order: a parametric plane
                // or axis is computed from its definition before the consumers below it.
                // A DATUM STANDING ON A BODY THAT IS GONE is red with that reason, as every node standing on a deleted
                // body is: resolved against the fingerprint the face left, it stood green on nothing. Reported
                // behaviour: the extrusion a plane from a face stood on was deleted, and the plane stayed green.
                FeatureKind::Plane { plane } if self.datum_base_body(plane).is_some_and(|b| !made.contains(&b)) => {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                    clear = false;
                }
                FeatureKind::DatumAxis { axis } if self.datum_base_body(axis).is_some_and(|b| !made.contains(&b)) => {
                    self.regen_errors.insert(node_id, crate::errors::CoreError::SourceBodyDeleted);
                    report.errors.push((node_id, crate::errors::CoreError::SourceBodyDeleted));
                    clear = false;
                }
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
                    // A SKETCH WHOSE FACE IS GONE stands on the snapshot of where the face stood (the deletion froze it
                    // there, so it does not move), and goes red with the reason rather than standing clean. Put on
                    // another face or plane, or the deletion undone, it is clean again.
                    use crate::errors::CoreError::{SketchFaceGone, SketchPlaneGone};
                    use crate::model::PlaneDef;
                    let gone = match self.sketch_index(sketch).map(|si| self.sketches[si].plane) {
                        Some(crate::feature::SketchPlane::Datum(pid)) => self.planes.iter().find(|p| p.id == pid).and_then(|p| match p.def {
                            PlaneDef::FaceGone => Some(SketchFaceGone),
                            PlaneDef::PlaneGone => Some(SketchPlaneGone),
                            _ => None,
                        }),
                        Some(crate::feature::SketchPlane::Face(b, _)) => (!self.timeline.iter().any(|n| n.kind.bodies().contains(&b))).then_some(SketchFaceGone),
                        _ => None,
                    };
                    if let Some(e) = gone {
                        self.regen_errors.insert(node_id, e.clone());
                        report.errors.push((node_id, e));
                    } else if matches!(self.regen_errors.get(&node_id), Some(SketchFaceGone | SketchPlaneGone)) {
                        self.regen_errors.remove(&node_id);
                    }
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
                                // SOLVED, NOT ONLY REBUILT: what is tied to the projection - an end snapped to its corner, a
                                // dimension from it - follows it to where the body put it
                                self.solve_sketch(si);
                                dirty.insert(sketch); // The projection moved, so the consumers of the contour follow.
                            }
                        }
                    }
                    if needs {
                        dirty.insert(sketch); // The sketch changed, so its consumers rebuild.
                    }
                }
                FeatureKind::SplitBody { src, plane, datum, offset, ref bodies, face } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_splitbody(&mut p, src, self.op_plane(plane, datum, face), offset, bodies);
                }
                FeatureKind::Import { body, .. } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_import(&mut p, body);
                }
                FeatureKind::PartInstance { src_comp, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_partinstance(&mut p, i, src_comp, body);
                }
                FeatureKind::ComponentPattern { src, ref bodies, .. } if needs => {
                    // every copy's body is the source's as it is; the pattern moves the copies, not the shapes
                    let mut p = pass!(node_id);
                    for b in bodies.clone() {
                        clear &= self.regen_partinstance(&mut p, i, src, b);
                    }
                }
                FeatureKind::MirrorPart { src_comp, ln, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_mirrorpart(&mut p, i, src_comp, ln, body);
                }
                FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, body } if needs => {
                    let mut p = pass!(node_id);
                    clear = self.regen_auger(&mut p, HelixRun { src, edge, length, lead_in, lead_out, body }, spec);
                }
                _ => {}
            }
            // Pass-through fallback: the modifier failed to build (broken edges or faces after a suppression or
            // an edit higher up the timeline — for example a suppressed fillet whose edges a chamfer below it
            // referenced) while its source exists, so it is skipped and the output body becomes a copy of the
            // source. That keeps the chain from holding stale geometry and makes the effect of the change
            // visible, instead of the model looking unchanged after a suppression. The error is already in the
            // report.
            // A PART IS ONE BODY OF ONE PIECE: an operation that left more pieces than its source had is refused in
            // words and passes through. Reported behaviour: an addition that touched nothing stood green with the body
            // in two pieces (12565 mm^3), a shell around a blind hole left a cup hanging in the cavity.
            if clear && keeps_one_piece {
                if let (Some(src), Some(b)) = (consumed_src, out_body) {
                    if kernel.body_pieces(b) > kernel.body_pieces(src) {
                        self.regen_errors.insert(node_id, crate::errors::CoreError::BodyInPieces);
                        report.errors.push((node_id, crate::errors::CoreError::BodyInPieces));
                        clear = false;
                    }
                }
            }

            // A CUT THAT PARTS THE BODY MAKES A BODY OF EVERY PIECE (see `CutPiece`) - when it was built in this pass: a node
            // left as it was holds its own piece alone already, and read again it lost the others
            if clear && needs && self.timeline[i].kind.cut_pieces_mut().is_some() {
                if let Some(b) = out_body {
                    clear = self.part_the_pieces(i, b, kernel, &mut dirty, &mut report, &mut emap);
                }
            }
            // A NODE OF SEVERAL BODIES KEEPS ITS LAST GOOD PIECES: the source copied into its first piece stood the whole
            // body beside the other pieces. Reported behaviour: a split moved to where it cuts nothing held 18000 mm^3 -
            // the block of 12000 in the first piece and the piece of 6000 beside it.
            let several = out_bodies.len() > 1;
            if !clear && !several {
                if let (Some(src), Some(b)) = (consumed_src.filter(|s| !unbuilt.contains(s)), out_body) {
                    if let Ok(crate::geom::Built { mesh, faces }) = kernel.transform_body(b, src, crate::feature::PLACE_IDENTITY) {
                        self.set_body_mesh(b, mesh);
                        // the copy is what the source was, a sheet or a solid: a sheet passed through a red trim
                        // counted as a solid of one face, a second body of the part
                        if let Some(i) = self.mesh_index(b) {
                            self.bodies[i].sheet = kernel.body_is_sheet(b);
                        }
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
            if let Some(t) = started {
                report.spent.push((node_id, self.timeline[i].name.clone(), t.elapsed().as_micros()));
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
                    broken.insert(b);
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
        // the copies of a pattern turned about a datum axis follow the axis this pass resolved
        self.resolve_comp_patterns();
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
        // A STOP ASKED WHILE A NODE WAS BEING BUILT cancels the rebuild as a stop between nodes does: the kernel breaks
        // a long operation off (the recognition of a mesh listens for it) and hands back a refusal that is no failure
        // of the node. Taken for one, it left the node red; with one node in the plan, the stop between nodes was
        // never even asked, and Cancel cancelled nothing.
        report.cancelled |= watch.stopped();
        report
    }

    /// A FILLET: round the picked edges of body `src` by `radius`, with a variable radius at named vertices.
    ///
    /// Returns whether the node's error record may be cleared (see `apply_regen`).
    fn prep_fillet(&mut self, p: &mut Pass, src: Id, radius: f64, edges: &crate::refs::Ref, at_vertices: &[(crate::refs::Ref, f64)], body: Id) -> crate::feature::KernelJob {
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
        let edges: Vec<u32> = edges.to_vec();
        let job = if on_sheet {
            crate::feature::KernelJob::refused(crate::errors::CoreError::NeedsSolidNotSheet)
        } else if lost_edges {
            crate::feature::KernelJob::refused(lost_error)
        } else if !verts.is_empty() && !edges.is_empty() {
            crate::feature::KernelJob::new(vec![src], move |k| k.fillet_at_vertices(body, src, radius, &edges, &verts))
        } else {
            // The name of a fillet surface comes from the edge that produced it, there being no
            // other source in the recipe. The edges are already named, so the name is
            // predictable.
            let names: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Blend, *e as Id)).collect();
            // Corner patch: a face produced by the vertex where fillets meet (see
            // `Role::Corner`).
            let corners: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Corner, *e as Id)).collect();
            let all = self.blend_names_all(p.node, src, p.kernel);
            crate::feature::KernelJob::new(vec![src], move |k| k.fillet(body, src, radius, &edges, crate::feature::BlendNames { surfaces: &names, corners: &corners, all: &all }))
        };
        job
    }

    /// Chamfer: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_chamfer(&mut self, p: &mut Pass, src: Id, dist: f64, edges: &crate::refs::Ref, shape: ChamferShape, body: Id) -> crate::feature::KernelJob {
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
        let job = if lost_edges {
            crate::feature::KernelJob::refused(lost_error)
        } else if mode != crate::feature::ChamferMode::Symmetric && !edges.is_empty() {
            crate::feature::KernelJob::new(vec![src], move |k| k.chamfer_ex(body, src, dist, ChamferShape { mode, d2, flip, ref_face }, &edges))
        } else {
            // The name of a chamfer surface comes from the edge that produced it and the patch name
            // from the vertex: the same recipe as for a fillet.
            let names: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Blend, *e as Id)).collect();
            let corners: Vec<u32> = edges.iter().map(|e| self.intern_name(p.node, crate::names::Role::Corner, *e as Id)).collect();
            let all = self.blend_names_all(p.node, src, p.kernel);
            crate::feature::KernelJob::new(vec![src], move |k| k.chamfer(body, src, dist, &edges, crate::feature::BlendNames { surfaces: &names, corners: &corners, all: &all }))
        };
        job
    }

    /// Shell: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_shell(&mut self, p: &mut Pass, src: Id, thickness: f64, faces: &crate::refs::Ref, side: crate::feature::ShellSide, body: Id) -> crate::feature::KernelJob {
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
        let asked_faces = if faces.query.is_pick_list() { faces.query.picked_descs().len() } else { 0 };

        match self.faces_by_ref(p.node, src, faces, "ref-what-shell-faces") {
            Err(_) => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
            Ok(ids) if asked_faces > 0 && ids.len() < asked_faces => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
            Ok(ids) => match side {
                crate::feature::ShellSide::Centred => crate::feature::KernelJob::new(vec![src], move |k| k.shell_center(body, src, thickness, &ids)),
                side => crate::feature::KernelJob::new(vec![src], move |k| k.shell_named(body, src, thickness, side == crate::feature::ShellSide::Outward, &ids, &walls)),
            },
        }
    }

    fn prep_removeface(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> crate::feature::KernelJob {
        match self.faces_by_ref(p.node, src, faces, "ref-what-removed-faces") {
            Ok(ids) if !ids.is_empty() => crate::feature::KernelJob::new(vec![src], move |k| k.remove_faces(body, src, &ids)),
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    fn prep_patch(&mut self, p: &Pass, src: Id, edges: &crate::refs::Ref, tangent: bool, body: Id) -> crate::feature::KernelJob {
        let edges = edges.clone();
        let live = self.live_fillet_edges(p.node, src, &edges, p.emap, p.kernel);
        if live.is_empty() {
            return crate::feature::KernelJob::refused(edges_lost(&edges, asked_edge_count(&edges)));
        }
        let name = self.intern_name(p.node, crate::names::Role::Patch, 0);
        crate::feature::KernelJob::new(vec![src], move |k| k.patch(body, src, &live, tangent, name))
    }

    fn prep_trim(&mut self, src: Id, tool: Id, keep: [f64; 3], body: Id) -> crate::feature::KernelJob {
        crate::feature::KernelJob::new(vec![src, tool], move |k| k.trim(body, src, tool, keep))
    }

    fn prep_stitch(&mut self, p: &Pass, parts: &[Id], tol: f64, body: Id) -> crate::feature::KernelJob {
        let t = p.dim("tol", tol);
        let parts = parts.to_vec();
        if parts.len() < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::OpFailed(crate::errors::Op::Stitch));
        }
        crate::feature::KernelJob::new(parts.clone(), move |k| k.stitch(body, &parts, t))
    }

    /// The mesh is the node's recipe: it travels with the work, and no live shape is read.
    // the mesh is made lighter inside the job, off the frame: a mesh of 178 032 triangles takes seconds
    fn prep_mesh_solid(&mut self, src: Id, body: Id, simplify: f64) -> crate::feature::KernelJob {
        let Some(mesh) = self.mesh_index(src).map(|i| self.bodies[i].mesh.clone()) else {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::SourceBodyNotBuilt);
        };
        crate::feature::KernelJob::new(Vec::new(), move |k| k.mesh_solid(body, &lighter(mesh, simplify)))
    }

    fn prep_mesh_recognised(&mut self, src: Id, body: Id, tol: f64, sharp: f64, simplify: f64) -> crate::feature::KernelJob {
        let Some(mesh) = self.mesh_index(src).map(|i| self.bodies[i].mesh.clone()) else {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::SourceBodyNotBuilt);
        };
        crate::feature::KernelJob::new(Vec::new(), move |k| {
            let mesh = lighter(mesh, simplify);
            let tol = within_simplification(&mesh, tol, simplify);
            k.mesh_recognised(body, &mesh, tol, sharp)
        })
    }

    fn prep_pushface(&mut self, p: &Pass, src: Id, face: &crate::refs::Ref, dist: f64, body: Id) -> crate::feature::KernelJob {
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

    fn prep_surfacereplace(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, surface: Id, body: Id) -> crate::feature::KernelJob {
        let faces = faces.clone();
        match self.faces_by_ref(p.node, src, &faces, "ref-what-surface-replace") {
            Ok(ids) if !ids.is_empty() => crate::feature::KernelJob::new(vec![src, surface], move |k| k.replace_faces(body, src, &ids, surface)),
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    fn prep_facecopy(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, body: Id) -> crate::feature::KernelJob {
        let faces = faces.clone();
        match self.faces_by_ref(p.node, src, &faces, "ref-what-face-copy") {
            Ok(ids) if !ids.is_empty() => {
                let names: Vec<u32> = ids.iter().map(|f| self.intern_name(p.node, crate::names::Role::Instance, *f as Id)).collect();
                crate::feature::KernelJob::new(vec![src], move |k| k.copy_faces(body, src, &ids, &names))
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    /// OffsetSurface: the faces, resolved as a face copy resolves them, and the distance, a dimension of its own (`dist`)
    /// a formula or a global parameter drives.
    fn prep_offset_surface(&mut self, p: &Pass, src: Id, faces: &crate::refs::Ref, dist: f64, body: Id) -> crate::feature::KernelJob {
        let d = eval_dim(p.dims, "dist", dist, p.vars);
        match self.faces_by_ref(p.node, src, faces, "ref-what-offset-surface") {
            Ok(ids) if !ids.is_empty() => {
                let names: Vec<u32> = ids.iter().map(|f| self.intern_name(p.node, crate::names::Role::Instance, *f as Id)).collect();
                crate::feature::KernelJob::new(vec![src], move |k| k.offset_faces(body, src, &ids, &names, d))
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::FacesNotFound),
        }
    }

    /// Thicken: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_thicken(&mut self, p: &mut Pass, src: Id, face: u32, thickness: f64, join: Id, body: Id) -> crate::feature::KernelJob {
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
            let (face_names, edge_names) = self.thicken_names(p.node, src, p.kernel);
            let inputs = if join != 0 { vec![src, join] } else { vec![src] };
            crate::feature::KernelJob::new(inputs, move |k| k.thicken_face(body, src, face, t, join, crate::feature::NameMaps { faces: &face_names, edges: &edge_names }))
        }
    }

    /// SplitFace: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_splitface(&mut self, p: &mut Pass, src: Id, at: Option<([f64; 3], [f64; 3])>, offset: f64, body: Id) -> crate::feature::KernelJob {
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
    fn prep_draft(&mut self, p: &mut Pass, src: Id, faces: &crate::refs::Ref, shape: DraftShape, body: Id) -> crate::feature::KernelJob {
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
        let job = match (self.faces_by_ref(p.node, src, faces, "ref-what-draft-faces"), self.face_by_ref(p.node, src, &neutral, "ref-what-draft-neutral")) {
            (Ok(ids), _) if asked_faces > 0 && ids.len() < asked_faces => crate::feature::KernelJob::refused(crate::errors::CoreError::DraftNeedsFaces),
            (Ok(ids), Ok(np)) if !ids.is_empty() => {
                let np_o = np.centroid;
                let np_n = if flip { [-np.normal[0], -np.normal[1], -np.normal[2]] } else { np.normal };
                {
                    // The side face of a draft is named after the face that was tilted: drafting
                    // produces a new face next to it, which without a name of its own would take a
                    // positional number.
                    let sides: Vec<u32> = ids.iter().filter(|f| crate::names::NameTable::is_named(**f)).flat_map(|f| [*f, self.intern_name(p.node, crate::names::Role::DraftSide, *f as Id)]).collect();
                    crate::feature::KernelJob::new(vec![src], move |k| {
                        k.draft(body, src, &ids, crate::feature::DraftPull { angle, dir: np_n }, crate::feature::PlaneAt { origin: np_o, normal: np_n }, &sides)
                    })
                }
            }
            _ => crate::feature::KernelJob::refused(crate::errors::CoreError::DraftNeedsFaces),
        };
        job
    }

    /// Mirror: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_mirror(&mut self, p: &mut Pass, src: Id, world: Option<u8>, keep: bool, at: Option<([f64; 3], [f64; 3])>, body: Id) -> crate::feature::KernelJob {
        // A datum plane or a face supplies its origin and normal (see `op_plane`) and the mirror is taken about it;
        // otherwise a world plane 0, 1 or 2 is used.
        // An image is not the original face: with `keep` the body holds both halves, and without
        // names of their own they are indistinguishable — the same collision a pattern had.
        let seed = self.instance_name_seeds(p.node, src, 2).pop().unwrap_or_default();
        // A deleted plane means no mirror. Falling back to a world plane sounds defensible — a
        // mirror about another plane is still a mirror — but the measurement refutes it: the part
        // moves. A mirror about a datum at x = 50 gave face centres from x = 10 to x = 90, and
        // deleting the datum moved them to -30..30 with no red node. A split already refuses in the
        // same situation, for exactly the same reason.
        match (world, at) {
            (Some(plane), _) => crate::feature::KernelJob::new(vec![src], move |k| k.mirror_named(body, src, plane, keep, &seed)),
            (None, Some((o, n))) => crate::feature::KernelJob::new(vec![src], move |k| k.mirror_plane_named(body, src, o, n, keep, &seed)),
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
    fn op_plane(&self, plane: u8, datum: Id, face: Option<(Id, crate::feature::FaceKey)>) -> Option<([f64; 3], [f64; 3])> {
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
    fn prep_move(&mut self, src: Id, mat: [f64; 12], body: Id) -> crate::feature::KernelJob {
        crate::feature::KernelJob::new(vec![src], move |k| k.transform_body(body, src, mat))
    }

    /// BodyBoolean: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_bodyboolean(&mut self, a: Id, b: Id, op: u8, body: Id) -> crate::feature::KernelJob {
        // Parametric body-to-body boolean: `op` applied to the B-reps of `a` and `b`, producing
        // `body`; both operands are consumed.
        crate::feature::KernelJob::new(vec![a, b], move |k| k.body_boolean(body, a, b, op))
    }

    /// Box3: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_box3(&mut self, p: &Pass, dx: f64, dy: f64, dz: f64, body: Id) -> crate::feature::KernelJob {
        let (dx, dy, dz) = (p.dim("dx", dx), p.dim("dy", dy), p.dim("dz", dz));
        let profile = rect_profile(dx, dy);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.extrude(body, &profile, dz, crate::feature::PLACE_IDENTITY))
    }

    /// Cylinder: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    /// THE PARCEL OF A CYLINDER, and of every primitive below it: numbers and names, nothing borrowed from
    /// the document.
    ///
    /// Preparing one takes `&mut self` and always will: minting the names of the faces to come is a change to
    /// the document, and it has to happen before the geometry is built. That is why the split is where it is -
    /// the preparation stays on the thread that owns the document, and only the parcel travels.
    fn prep_cylinder(&mut self, p: &Pass, r: f64, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        let nm = self.primitive_names(p.node);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.cylinder(body, r, h, nm))
        // Exact B-rep cylinder, three faces.
    }

    /// Sphere: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_sphere(&mut self, p: &Pass, r: f64, body: Id) -> crate::feature::KernelJob {
        let r = eval_dim(p.dims, "r", r, p.vars);
        let nm = self.primitive_names(p.node);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.sphere(body, r, nm))
        // Exact sphere, one face.
    }

    /// Cone: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_cone(&mut self, p: &Pass, r1: f64, r2: f64, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r1, r2, h) = (p.dim("r1", r1), p.dim("r2", r2), p.dim("h", h));
        let nm = self.primitive_names(p.node);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.cone(body, r1, r2, h, nm))
        // Exact cone.
    }

    /// Torus: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_torus(&mut self, p: &Pass, major: f64, minor: f64, body: Id) -> crate::feature::KernelJob {
        let (major, minor) = (p.dim("major", major), p.dim("minor", minor));
        // a tube as thick as the ring or thicker passes through itself: a ring of 5 and a tube of 5 stood green as a
        // spindle of 2467 mm^3
        if minor >= major {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::TorusThroughItself);
        }
        let nm = self.primitive_names(p.node);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.torus(body, major, minor, nm))
        // Exact torus, one face.
    }

    /// Prism: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_prism(&mut self, p: &Pass, r: f64, n: u32, h: f64, body: Id) -> crate::feature::KernelJob {
        let (r, h) = (p.dim("r", r), p.dim("h", h));
        let profile = polygon_profile(r, n);
        crate::feature::KernelJob::new(Vec::new(), move |k| k.extrude(body, &profile, h, crate::feature::PLACE_IDENTITY))
    }

    /// Loft: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_loft(&mut self, p: &mut Pass, sketches: Vec<Id>, contours: Vec<Id>, ruled: bool, surface: bool, bo: BodyOp) -> crate::feature::KernelJob {
        let BodyOp { src, op, body } = bo;
        // Each section sits on its own plane (the placement is encoded in `places`), and two or more
        // sections produce a body. A zero `src` makes a separate body; otherwise the lofted solid is
        // combined with body `src` as a lofted cut or boss.
        let caps = self.cap_names(p.node);

        match self.loft_encoded_named(p.node, &sketches, &contours) {
            // A surface is the same loft, not closed into a solid. It admits no boolean: there is
            // nothing to combine a surface with a body by until it has been given a thickness.
            Some((data, offsets, places)) if src == 0 => crate::feature::KernelJob::new(Vec::new(), move |k| {
                let body_kind = if surface { crate::feature::LoftBody::Sheet } else { crate::feature::LoftBody::Solid };
                k.loft(body, crate::feature::LoftSections { data: &data, offsets: &offsets, places: &places }, walls(ruled), body_kind, caps)
            }),
            Some((data, offsets, places)) => crate::feature::KernelJob::new(vec![src], move |k| k.loft_combine(BodyOp { src, op, body }, &data, &offsets, &places, walls(ruled), caps)),
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::LoftNeedsTwoSections),
        }
    }

    /// Import: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_import(&mut self, p: &mut Pass, body: Id) -> bool {
        // An external STEP solid has no recipe, so this only re-tessellates the shape from the kernel
        // cache. Without a shape in the kernel (a mock, or before restoration from the source) the
        // already loaded mesh is kept.
        if let Some(crate::geom::Built { mesh, faces }) = p.kernel.tessellate(body) {
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
        self.apply_regen(p.landing(), body, res)
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
        self.apply_regen(p.landing(), body, res)
    }

    /// WHAT REFUSES A THREAD BEFORE THE KERNEL IS ASKED, on the cylinder of radius `r` whose rim centre and axis
    /// along the face are `rim`: the side of the material, then the size, the depth, the pitch, the turns, and the
    /// length against the face. The thread's geometry and the radius it is cut at when nothing refuses.
    fn thread_verdict(&self, src: Id, rim: ([f64; 3], [f64; 3], f64), spec: &mut crate::thread::ThreadSpec, length: f64) -> Result<(crate::thread::ThreadGeom, f64), crate::errors::CoreError> {
        let (c, ax, r) = rim;
        // The material side comes from the geometry rather than from the checkbox. A groove
        // pointing the wrong way cuts empty space: on a shaft marked "internal" it removed
        // 1.8 cm^3 instead of 13 and left flat discs instead of turns. There is no case
        // where a thread into empty space is wanted, so the geometry wins.
        if let Some(side) = self.cyl_side_of_body(src, c, ax, r, length) {
            spec.internal = side;
        }
        // an ISO nut's groove starts at the wall of the hole as drilled, the standard's profile cut off there;
        // a hole under D1 is cut from D1. Set in the void short of the wall, at D1 of a hole of 8.5, the
        // sweep cut nothing at all past a length of 4.
        let g = spec.geometry_in(Some(2.0 * r));
        let self_r = r; // the radius of the face itself
        let r = if spec.internal && spec.standard == crate::thread::ThreadStandard::MetricIso { r.max(g.minor_d * 0.5) } else { r };
        // Validation before the kernel: bad parameters otherwise produce a silent no-op or
        // crash the sweep.
        let turns = length / g.lead.max(1e-9);
        // A THREAD OF ITS OWN SIZE: the face is the size the thread is - a shaft within 5 % (or half a pitch) of
        // the nominal, a hole drilled from the basic minor diameter less half a pitch up to 5 % over the nominal.
        // Reported behaviour: M10 on a cylinder of 20 stood green, 519 mm^3 cut off it in 76 faces.
        let face_d = 2.0 * self_r;
        let off_size = if spec.internal { face_d < g.minor_d - 0.5 * g.pitch || face_d > g.major_d * 1.05 } else { (face_d - g.major_d).abs() > (0.05 * g.major_d).max(0.5 * g.pitch) };
        let bad = if off_size {
            Some(crate::errors::CoreError::ThreadNotItsSize { face: face_d, nominal: spec.nominal_d })
        } else if !spec.internal && g.depth >= r * 0.95 {
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
        if let Some(e) = bad {
            return Err(e);
        }
        // A THREAD NO LONGER THAN ITS FACE: past the end of the cylinder the sweep runs in the air or into a
        // shoulder. Reported behaviour: a thread of 101 on a cylinder 10 long was taken and went red after the
        // rebuild. The face's length is read off the mesh; a mesh that does not show it refuses nothing here.
        if let Some(face) = self.mesh_index(src).and_then(|mi| crate::geom::cyl_span_from_mesh(&self.bodies[mi].mesh, c, ax, self_r)) {
            if length > face + 1e-3 {
                return Err(crate::errors::CoreError::ThreadLongerThanFace { length, face });
            }
        }
        Ok((g, r))
    }

    /// HOW LONG THE CYLINDER A THREAD IS PUT ON RUNS from its rim, read off the mesh, the axis turned along the face
    /// as the rebuild turns it; `None` when the mesh does not show it.
    pub fn thread_face_span(&self, src: Id, rim: ([f64; 3], [f64; 3], f64)) -> Option<f64> {
        let (c, axis, r) = rim;
        let mi = self.mesh_index(src)?;
        let ax = crate::geom::axis_along_cylinder(&self.bodies[mi].mesh, c, axis, r).unwrap_or(axis);
        crate::geom::cyl_span_from_mesh(&self.bodies[mi].mesh, c, ax, r)
    }

    /// WHAT REFUSES THE THREAD a command would make, for the command to say it beside its field before Enter: the
    /// rim as the command picked it, its axis turned along the face as the rebuild turns it.
    pub fn thread_refusal(&self, src: Id, rim: ([f64; 3], [f64; 3], f64), spec: crate::thread::ThreadSpec, length: f64) -> Option<crate::errors::CoreError> {
        let (c, axis, r) = rim;
        let ax = self.mesh_index(src).and_then(|mi| crate::geom::axis_along_cylinder(&self.bodies[mi].mesh, c, axis, r)).unwrap_or(axis);
        let mut spec = spec;
        self.thread_verdict(src, (c, ax, r), &mut spec, length).err()
    }

    /// Thread: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_thread(&mut self, p: &mut Pass, run: HelixRun, spec: crate::thread::ThreadSpec) -> crate::feature::KernelJob {
        let HelixRun { src, edge, length, lead_in, lead_out, body } = run;
        let mut spec = spec;
        // Parametric like every feature: the size, pitch and clearance may be expressions.
        spec.nominal_d = p.dim("nominal", spec.nominal_d);
        spec.pitch = p.dim("pitch", spec.pitch);
        spec.fit = p.dim("fit", spec.fit);
        // The crest and root radii are parametric too; zero means "as the standard says".
        spec.crest_r = {
            let v = p.dim("crest_r", spec.crest_r.unwrap_or(0.0));
            (v > 1e-9).then_some(v)
        };
        spec.root_r = {
            let v = p.dim("root_r", spec.root_r.unwrap_or(0.0));
            (v > 1e-9).then_some(v)
        };
        let length = p.dim("length", length);
        let lead_in = p.dim("lead_in", lead_in);
        let lead_out = p.dim("lead_out", lead_out);

        match self.helical_axis(p.kernel, src, edge) {
            Some((c, ax, r)) => {
                match self.thread_verdict(src, (c, ax, r), &mut spec, length) {
                    Err(e) => crate::feature::KernelJob::refused(e),
                    Ok((g, r)) => {
                        // The turn faces are named before the kernel call: the recipe supplies the names
                        // (a profile edge plus the lead).
                        let gnames = self.groove_names(p.node, g.groove.len(), spec.starts.max(1));
                        let rnames = self.relief_names(p.node);
                        // the volume of the source is read BEFORE the parcel leaves: the check below compares
                        // against it, and the document does not travel with the worker
                        let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                        let profile = crate::thread::encode_edges(&g.groove);
                        let (lead, starts, left, relief) = (g.lead, spec.starts.max(1), spec.left, spec.radial_relief());
                        crate::feature::KernelJob::new(vec![src], move |k| {
                            k.helical(crate::feature::Helical {
                                body,
                                src,
                                origin: c,
                                dir: ax,
                                radius: r,
                                profile: &profile,
                                length,
                                lead,
                                starts,
                                left,
                                fuse: false, // a thread is subtracted
                                lead_in,
                                lead_out,
                                gnames: &gnames,
                                rnames: &rnames,
                                crest_relief: relief,
                            })
                            .and_then(|built| {
                                // The result is checked, not only the input: a thread that removed
                                // nothing is a refusal rather than a success, or a smooth part is
                                // reported as done. The groove side already came from the geometry, so
                                // what is caught here is the rest: too fine a pitch, a degenerate
                                // profile, a miss against the face.
                                let got = src_v - built.mesh.volume();
                                if src_v > 0.0 && got < 1e-6 * src_v {
                                    Err(crate::errors::CoreError::ThreadRemovedNothing { before: src_v, after: built.mesh.volume() })
                                } else {
                                    Ok(built)
                                }
                            })
                        })
                    }
                }
            }
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::ThreadRimNotFound),
        }
    }

    /// Auger: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_auger(&mut self, p: &mut Pass, run: HelixRun, spec: crate::thread::AugerSpec) -> bool {
        let HelixRun { src, edge, length, lead_in, lead_out, body } = run;
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
                        .and_then(|built| {
                            // An auger flight is welded on, so the volume has to grow. If it did
                            // not, the flight landed beside the shaft, and reporting success would
                            // mean a smooth shaft.
                            let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                            if src_v > 0.0 && built.mesh.volume() <= src_v * 1.001 {
                                Err(crate::errors::CoreError::AugerAddedNothing { before: src_v, after: built.mesh.volume() })
                            } else {
                                Ok(built)
                            }
                        })
                }
            }
            None => Err(crate::errors::CoreError::AugerRimNotFound),
        };
        self.apply_regen(p.landing(), body, res)
    }

    /// Hole: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_hole(&mut self, p: &mut Pass, src: Id, at: HoleSite, tool: HoleTool, body: Id) -> crate::feature::KernelJob {
        let HoleSite { face, point, normal, sketch, flip } = at;
        // THE PARAMETRIC OVERRIDES ARE APPLIED HERE, and the tool is rebuilt from them. Handing the
        // stored `tool` to the kernel would drop every expression driving a hole's diameter or depth -
        // silently, because the numbers are of the right type either way.
        let tool =
            HoleTool { kind: tool.kind, diameter: p.dim("diameter", tool.diameter), depth: p.dim("depth", tool.depth).abs(), dia2: p.dim("dia2", tool.dia2), depth2: p.dim("depth2", tool.depth2) };
        let job = if sketch != 0 {
            // At the isolated points of a sketch: one frame per point, with every cut applied by a
            // single boolean.
            let pls = self.sketch_hole_points(sketch, flip);
            if pls.is_empty() {
                crate::feature::KernelJob::refused(crate::errors::CoreError::NoIsolatedPointsForHoles)
            } else {
                // The wall of each hole is named after the sketch point that placed it, so adding a
                // point does not rename the neighbouring holes.
                let pts = self.sketch_hole_point_ids(sketch);
                let bores: Vec<u32> = (0..pls.len()).map(|i| self.intern_name(p.node, crate::names::Role::Hole, pts.get(i).copied().unwrap_or(i as Id + 1))).collect();
                let names = self.hole_tool_names(p.node);
                crate::feature::KernelJob::new(vec![src], move |k| k.holes(body, src, tool, &pls, &bores, &names))
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
                Err(_) if picked => crate::feature::KernelJob::refused(crate::errors::CoreError::FaceNotFound),
                resolved => {
                    // THE HOLE STANDS WHERE IT WAS PLACED: the stored point brought onto the face as it stands now
                    // (along its normal), not the face's centre - two holes in one face stood in each other, the
                    // second green and drilling nothing. A hole placed before holds the centre it was given then.
                    let (point, normal) = match resolved {
                        Ok(c) => {
                            let off = (0..3).map(|i| (point[i] - c.centroid[i]) * c.normal[i]).sum::<f64>();
                            ([point[0] - off * c.normal[0], point[1] - off * c.normal[1], point[2] - off * c.normal[2]], c.normal)
                        }
                        Err(_) => (point, normal),
                    };
                    // The tool (a cylinder plus a counterbore or countersink) is placed at the face
                    // point along its normal and cuts inwards.
                    let pl = crate::feature::PlaneFrame::from_origin_normal(point, normal, 0.0).matrix12();
                    let bore = self.intern_name(p.node, crate::names::Role::Hole, 0);
                    let names = self.hole_tool_names(p.node);
                    crate::feature::KernelJob::new(vec![src], move |k| k.hole(body, src, tool, pl, bore, &names))
                }
            }
        };
        job
    }

    /// A CUT OR AN INTERSECTION THAT PARTED BODY `body` IN SEVERAL SOLIDS makes each solid a body of the node's part, as
    /// splitting a body does. Every piece the node had keeps its body: it takes the free solid nearest the point it was
    /// at, the node's own body first. A piece with no solid left loses its body (what stood on it turns red); a solid
    /// with no piece gets a new body, the biggest first. Returns whether every piece landed.
    fn part_the_pieces(
        &mut self,
        ti: usize,
        body: Id,
        kernel: &dyn crate::feature::Kernel,
        dirty: &mut std::collections::HashSet<Id>,
        report: &mut crate::feature::RegenReport,
        emap: &mut EdgeRenames,
    ) -> bool {
        let node = self.timeline[ti].id;
        // only a body that was one piece is parted: the copies of a pattern are pieces of one body before any cut, and
        // a rod through the row leaves them so; a body that is to stay whole counts as one solid, and the bodies of the
        // pieces it had go
        let parts = self.timeline[ti].kind.consumed_body().is_none_or(|s| kernel.body_pieces(s) <= 1);
        let solids = if parts { kernel.body_solids(body) } else { Vec::new() };
        let had: Vec<crate::feature::CutPiece> = self.timeline[ti].kind.cut_pieces().to_vec();
        let dist = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>();
        let mut free: Vec<usize> = (0..solids.len()).collect();
        let mut plan: Vec<(Id, usize)> = Vec::new();
        if solids.len() >= 2 {
            let biggest = free.iter().copied().max_by(|&x, &y| solids[x].1.total_cmp(&solids[y].1)).unwrap_or(0);
            let asked: Vec<(Id, [f64; 3])> = if had.is_empty() { vec![(body, solids[biggest].0)] } else { had.iter().map(|p| (p.body, p.at)).collect() };
            for (id, at) in asked {
                let Some(pos) = (0..free.len()).min_by(|&x, &y| dist(solids[free[x]].0, at).total_cmp(&dist(solids[free[y]].0, at))) else { break };
                plan.push((id, free.remove(pos)));
            }
            free.sort_by(|&x, &y| solids[y].1.total_cmp(&solids[x].1));
            for k in free {
                plan.push((self.alloc_id(), k));
            }
        }
        // the bodies of pieces that have no solid any more go, and with them their faces
        for p in had.iter().filter(|p| p.body != body && !plan.iter().any(|(id, _)| *id == p.body)) {
            if let Some(mi) = self.mesh_index(p.body) {
                self.remove_mesh(mi);
            }
            self.regen_faces.remove(&p.body);
            dirty.insert(p.body);
        }
        let mut clear = true;
        // the other pieces are taken out of the whole first, and the node's own body keeps its piece last
        for &(id, k) in plan.iter().skip(1).chain(plan.first()) {
            let res = kernel.take_solid(id, body, k);
            clear &= self.apply_regen(Landing { node, dirty: &mut *dirty, report: &mut *report, kernel, emap: &mut *emap }, id, res);
        }
        if let Some(v) = self.timeline[ti].kind.cut_pieces_mut() {
            *v = plan.iter().map(|&(id, k)| crate::feature::CutPiece { body: id, at: solids[k].0 }).collect();
        }
        clear
    }

    /// SplitBody: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn regen_splitbody(&mut self, p: &mut Pass, src: Id, at: Option<([f64; 3], [f64; 3])>, offset: f64, bodies: &[Id]) -> bool {
        // The cutting plane is a reference (as for a mirror): a world plane, a datum or a face (see `op_plane`), resolved
        // here, in timeline order, so a face that moved carries the split with it; otherwise a split would be a one-off
        // cut against forgotten coordinates.
        // A deleted plane means no cut. Falling back to a world plane is not acceptable here: a
        // mirror about another plane is still a mirror, while a split along another plane breaks the
        // body in a different place, silently. A red node beats a quietly different part.
        let lost = at.is_none();
        let (o0, n) = at.unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]));
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
                        clear &= self.apply_regen(p.landing(), *b, Ok(part));
                    }
                }
            }
            Err(e) => {
                for &b in bodies {
                    clear &= self.apply_regen(p.landing(), b, Err(e.clone()));
                }
            }
        }
        clear
    }

    /// LinearArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_lineararray(&mut self, p: &mut Pass, src: Id, axes: [ArrayAxis; 3], body: Id) -> crate::feature::KernelJob {
        // the counts are dimensions of the node as the steps are: a count typed as `k` follows `k`
        let mut axes = axes;
        for (a, key) in axes.iter_mut().zip(["count", "count2", "count3"]) {
            a.count = p.dim(key, a.count as f64).round().max(1.0) as u32;
        }
        // a pattern of one copy is the body alone: it stood green and changed nothing
        if axes.iter().map(|a| a.count.max(1)).product::<u32>() < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::ArrayOfOne);
        }
        // Parametric: the step lives as an expression per vector component, so global parameters move
        // the pattern. An empty expression keeps the stored number. The keys are the field names in the
        // document ("dx", "dx2", "dx3"), so the suffix is the axis number - the first carries none.
        let step: Vec<[f64; 3]> = axes
            .iter()
            .enumerate()
            .map(|(n, a)| {
                let sfx = if n == 0 { String::new() } else { (n + 1).to_string() };
                [p.dim(&format!("dx{sfx}"), a.d[0]), p.dim(&format!("dy{sfx}"), a.d[1]), p.dim(&format!("dz{sfx}"), a.d[2])]
            })
            .collect();
        // A 3D grid: direction one (i*d1) by two (j*d2) by three (k*d3). A count of one or less in
        // the second or third direction reduces the dimensionality.
        let (c1, c2, c3) = (axes[0].count.max(1), axes[1].count.max(1), axes[2].count.max(1));
        let mut ts: Vec<[f64; 12]> = Vec::with_capacity((c1 * c2 * c3) as usize);
        for i in 0..c1 {
            for j in 0..c2 {
                for k in 0..c3 {
                    let n = [i as f64, j as f64, k as f64];
                    let at = |c: usize| n[0] * step[0][c] + n[1] * step[1][c] + n[2] * step[2][c];
                    ts.push(translate_mat(at(0), at(1), at(2)));
                }
            }
        }
        let seeds = self.instance_name_seeds(p.node, src, ts.len());
        crate::feature::KernelJob::new(vec![src], move |k| k.pattern_named(body, src, &ts, &seeds))
    }

    /// CircularArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_circulararray(&mut self, p: &mut Pass, src: Id, count: u32, angle: f64, axis: Id, body: Id) -> crate::feature::KernelJob {
        // Parametric: the angle is an expression over global parameters. An empty expression keeps
        // the stored number.
        let angle = p.dim("angle", angle);
        // the count is a dimension of the node as the angle is: a count typed as `k` follows `k`
        let count = p.dim("count", count as f64).round().max(1.0) as u32;
        if count < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::ArrayOfOne);
        }
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
        crate::feature::KernelJob::new(vec![src], move |k| k.pattern_named(body, src, &ts, &seeds))
    }

    /// WHAT CAN BE COMPUTED RIGHT NOW, starting from node `from`.
    ///
    /// A node joins the list while nothing it needs is made by a node standing between here and it. That is a
    /// stricter question than "which nodes are in one wave": a wave says they do not depend on ONE ANOTHER,
    /// while this says they do not depend on anything not yet written into the document. Preparing a node
    /// whose source is computed but not yet applied resolves its references against geometry that is not
    /// there - measured on the scenario document as 13 refusals and half the bodies missing.
    ///
    /// `held` are the nodes already computed and waiting to be written down: they must not be prepared a
    /// second time (a preparation mints the names of the faces to come), and what they make is not current
    /// until the rebuild reaches them.
    ///
    /// `walked` are the places this pass has already been through. A batch is written down as soon as it is
    /// computed, ahead of the walk, so the next batch starts ABOVE nodes that are finished - and each of them
    /// still looks as if it needed rebuilding, its input being dirty. Offered again, they were rebuilt once per
    /// batch: 74 times each on the scenario document. What they made is current, so they hold nothing back.
    ///
    /// Except `from` itself: that is the place the walk is standing on, not one it has been through. Skipping it
    /// left its body out of what is still to be built, the node standing on it was prepared on the body from
    /// before the edit, and a thread on the scenario document could no longer find its rim.
    pub(crate) fn ready_from(&self, from: usize, dirty: &std::collections::HashSet<Id>, held: &std::collections::HashSet<Id>, walked: &std::collections::HashSet<usize>, limit: usize) -> Vec<usize> {
        let mut ahead: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut ready: Vec<usize> = Vec::new();
        for j in from..self.timeline.len().min(limit) {
            if j != from && walked.contains(&j) {
                continue;
            }
            let mine = &self.timeline[j];
            let behind = self.node_reads_any(j, &ahead);
            if mine.suppressed || !self.node_needs(j, dirty) {
                // CLEAN FOR NOW IS NOT CLEAN. Its source is still to be rebuilt, so it will be rebuilt too when the
                // walk reaches it (a suppressed modifier passes the new source through), and what it makes is not
                // current either. Skipping it silently offered the node standing on it a body from before the edit.
                if behind {
                    ahead.extend(mine.kind.declares());
                }
                continue;
            }
            let waiting = held.contains(&mine.id) || behind;
            if !waiting {
                ready.push(j);
            }
            // whatever this node makes is not current until it has been rebuilt
            for d in mine.kind.declares() {
                ahead.insert(d);
            }
        }
        ready
    }

    /// WHAT WOULD BE COMPUTED SIDE BY SIDE from node `from`, with the document fully dirty: the rule above,
    /// asked from outside. A check of the rule is worth more than a check of the timing it buys.
    pub fn ready_batch_from(&self, from: usize) -> Vec<Id> {
        let dirty = std::collections::HashSet::new();
        let held = std::collections::HashSet::new();
        let walked = std::collections::HashSet::new();
        self.ready_from(from, &dirty, &held, &walked, self.rollback.unwrap_or(usize::MAX)).into_iter().filter_map(|j| self.timeline.get(j).map(|n| n.id)).collect()
    }

    /// THE ONE DOOR TO A NODE'S PARCEL: what has to be built for this node, and what it needs to build it.
    ///
    /// `None` means the node is one of the five that are computed in place: an import re-tessellates a ready
    /// shape, a body split returns several bodies at once, a part instance and a part mirror follow the active
    /// body of their source at this point of the timeline, and an auger checks the volume it removed against the
    /// document.
    ///
    /// Preparing a parcel CHANGES THE DOCUMENT (it mints the names of the faces to come), so this is called
    /// from the thread that owns it - once per node, in timeline order, exactly as before.
    fn prep_node(&mut self, p: &mut Pass, kind: &FeatureKind) -> Option<(Id, crate::feature::KernelJob)> {
        let job = match kind {
            FeatureKind::Extrude { sketch, profiles, height, reach, down, fill, body } => {
                (*body, self.prep_extrude(p, Profile { sketch: *sketch, profiles }, ExtrudeSpan { height: *height, down: *down, reach: *reach, fill }, *body))
            }
            FeatureKind::Revolve { sketch, profiles, axis, angle, axis_datum, axis_line, reach, src, op, body, .. } => {
                let prof = Profile { sketch: *sketch, profiles };
                let ax = RevolveAxis { axis: *axis, datum: *axis_datum, line: *axis_line };
                (*body, self.prep_revolve(p, prof, ax, *angle, *reach, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Sweep { sketch, profiles, path_sketch, path, src, op, body, .. } => {
                (*body, self.prep_sweep(p, Profile { sketch: *sketch, profiles }, *path_sketch, *path, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Loft { sketches, contours, ruled, src, op, surface, body, .. } => {
                (*body, self.prep_loft(p, sketches.clone(), contours.clone(), *ruled, *surface, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Box3 { dx, dy, dz, body } => (*body, self.prep_box3(p, *dx, *dy, *dz, *body)),
            FeatureKind::Cylinder { r, h, body } => (*body, self.prep_cylinder(p, *r, *h, *body)),
            FeatureKind::Sphere { r, body } => (*body, self.prep_sphere(p, *r, *body)),
            FeatureKind::Cone { r1, r2, h, body } => (*body, self.prep_cone(p, *r1, *r2, *h, *body)),
            FeatureKind::Torus { major, minor, body } => (*body, self.prep_torus(p, *major, *minor, *body)),
            FeatureKind::Prism { r, n, h, body } => (*body, self.prep_prism(p, *r, *n, *h, *body)),
            FeatureKind::Combine { src, sketch, profiles, height, op, extent, down, fill, body, .. } => {
                let prof = Profile { sketch: *sketch, profiles };
                let span = CombineSpan { height: *height, down: *down, extent: *extent, fill };
                (*body, self.prep_combine(p, prof, span, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Fillet { src, radius, edges, at_vertices, body } => (*body, self.prep_fillet(p, *src, *radius, edges, at_vertices, *body)),
            FeatureKind::Chamfer { src, dist, edges, mode, d2, flip, ref_face, body } => {
                (*body, self.prep_chamfer(p, *src, *dist, edges, ChamferShape { mode: *mode, d2: *d2, flip: *flip, ref_face: *ref_face }, *body))
            }
            FeatureKind::Shell { src, thickness, faces, side, body } => (*body, self.prep_shell(p, *src, *thickness, faces, *side, *body)),
            FeatureKind::RemoveFace { src, faces, body } => (*body, self.prep_removeface(p, *src, faces, *body)),
            FeatureKind::PushFace { src, face, dist, body } => (*body, self.prep_pushface(p, *src, face, *dist, *body)),
            FeatureKind::Patch { src, edges, tangent, body } => (*body, self.prep_patch(p, *src, edges, *tangent, *body)),
            FeatureKind::SurfaceReplace { src, faces, surface, body } => (*body, self.prep_surfacereplace(p, *src, faces, *surface, *body)),
            FeatureKind::FaceCopy { src, faces, body } => (*body, self.prep_facecopy(p, *src, faces, *body)),
            FeatureKind::OffsetSurface { src, faces, dist, body } => (*body, self.prep_offset_surface(p, *src, faces, *dist, *body)),
            FeatureKind::Trim { src, tool, keep, body } => (*body, self.prep_trim(*src, *tool, *keep, *body)),
            FeatureKind::Stitch { parts, tol, body } => (*body, self.prep_stitch(p, parts, *tol, *body)),
            FeatureKind::MeshSolid { src, body, simplify } => (*body, self.prep_mesh_solid(*src, *body, *simplify)),
            FeatureKind::MeshRecognised { src, body, tol, sharp, simplify } => (*body, self.prep_mesh_recognised(*src, *body, *tol, *sharp, *simplify)),
            FeatureKind::Thicken { src, face, thickness, join, body } => (*body, self.prep_thicken(p, *src, *face, *thickness, *join, *body)),
            FeatureKind::SplitFace { src, plane, datum, offset, body, face } => (*body, self.prep_splitface(p, *src, self.op_plane(*plane, *datum, *face), *offset, *body)),
            FeatureKind::Draft { src, faces, neutral, angle, flip, body } => (*body, self.prep_draft(p, *src, faces, DraftShape { neutral: neutral.clone(), angle: *angle, flip: *flip }, *body)),
            FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body } => {
                let axes = [ArrayAxis { d: [*dx, *dy, *dz], count: *count }, ArrayAxis { d: [*dx2, *dy2, *dz2], count: *count2 }, ArrayAxis { d: [*dx3, *dy3, *dz3], count: *count3 }];
                (*body, self.prep_lineararray(p, *src, axes, *body))
            }
            FeatureKind::CircularArray { src, count, angle, axis, body } => (*body, self.prep_circulararray(p, *src, *count, *angle, *axis, *body)),
            FeatureKind::Mirror { src, plane, keep, datum, body, face } => {
                (*body, self.prep_mirror(p, *src, (*datum == 0 && face.is_none()).then_some(*plane), *keep, self.op_plane(*plane, *datum, *face), *body))
            }
            FeatureKind::Move { src, mat, body } => (*body, self.prep_move(*src, *mat, *body)),
            FeatureKind::Piece { src, body } => {
                let (src, body) = (*src, *body);
                (body, crate::feature::KernelJob::new(vec![src], move |k| k.transform_body(body, src, crate::feature::PLACE_IDENTITY)))
            }
            FeatureKind::BodyBoolean { a, b, op, body, .. } => (*body, self.prep_bodyboolean(*a, *b, *op, *body)),
            FeatureKind::Hole { src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch, flip, body, .. } => {
                let at = HoleSite { face: face.clone(), point: *point, normal: *normal, sketch: *sketch, flip: *flip };
                let tool = HoleTool { kind: *kind, diameter: *diameter, depth: *depth, dia2: *dia2, depth2: *depth2 };
                (*body, self.prep_hole(p, *src, at, tool, *body))
            }
            FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, body } => {
                let run = HelixRun { src: *src, edge: *edge, length: *length, lead_in: *lead_in, lead_out: *lead_out, body: *body };
                (*body, self.prep_thread(p, run, *spec))
            }
            _ => return None,
        };
        Some(job)
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

    /// THE AXIS A REVOLUTION TURNS ABOUT, in the sketch's own frame, with whether it was named (a line or a datum) rather
    /// than the sketch's X or Y. Priority: a sketch centreline (already local), then a datum axis (world, including one
    /// bound to an edge or a face, brought in through the inverse placement), then the sketch X or Y axis. One
    /// computation for the rebuild, its check of a profile crossing the axis and the preview before Enter: copies
    /// would drift, and the preview would turn about an axis the body is not built about.
    pub fn revolve_axis_local(&self, sketch: Id, ax: RevolveAxis) -> ([f64; 3], [f64; 3], bool) {
        let axis_od = (ax.datum != 0).then(|| self.datum_axes.iter().find(|d| d.id == ax.datum).map(|d| (d.origin(), d.dir()))).flatten();
        match (self.revolve_axis_from_line(sketch, ax.line), axis_od) {
            (Some((lo, ld)), _) => (lo, ld, true),
            (None, Some((wo, wd))) => {
                let inv = crate::feature::mat_inv12(&self.sketch_place(sketch));
                (crate::feature::apply12(&inv, wo), crate::feature::apply12_dir(&inv, wd), true)
            }
            _ => ([0.0, 0.0, 0.0], if ax.axis == 0 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }, false),
        }
    }

    /// Extrude: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_extrude(&mut self, p: &mut Pass, prof: Profile, span: ExtrudeSpan, body: Id) -> crate::feature::KernelJob {
        let (Profile { sketch, profiles }, ExtrudeSpan { height, down, reach, fill }) = (prof, span);
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

        match self.encode_profiles_fill(p.node, sketch, profiles, fill) {
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::ProfileNotFound),
            Some(profs) => {
                let caps = self.region_cap_names(p.node, &profs);
                crate::feature::KernelJob::new(Vec::new(), move |k| k.combine_region_multi(BodyOp { src: 0, op: 1, body }, &profs, total, pl, &caps))
            }
        }
    }

    /// Revolve: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_revolve(&mut self, p: &mut Pass, prof: Profile, ax: RevolveAxis, angle: f64, reach: crate::feature::Reach, t: BodyOp) -> crate::feature::KernelJob {
        let (sketch, profiles) = (prof.sketch, prof.profiles);
        let axis = ax.axis;
        let (src, op, body) = (t.src, t.op, t.body);
        let pl = self.sketch_place(sketch);
        let angle = eval_dim(p.dims, "angle", angle, p.vars);
        let (ax_o, ax_d, named) = self.revolve_axis_local(sketch, ax);
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

        match self.encode_profiles_role(p.node, sketch, profiles, &[], crate::names::Role::Revolved) {
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::ProfileNotFound),
            Some(profs) => (|| {
                let caps = self.region_cap_names(p.node, &profs);
                // Honest diagnostics before the kernel: a profile crossing the axis produces readable
                // text with a hint rather than a faceless "revolve failed".
                //
                // The axis in sketch local space is resolved once and serves both the check and the
                // kernel call. Writing the priority (centreline, then datum through the inverse
                // placement, then the X or Y fallback) twice — once for diagnostics and once for the
                // call — lets the copies drift, and the "profile crosses the axis" check would then
                // validate an axis other than the one the body is built about, lying silently.
                // The axis check runs over every contour rather than one: the command takes them all,
                // and a second contour crossing the axis fails the operation just as the first would.
                let chk: Vec<Id> = if profiles.is_empty() { self.sketch_index(sketch).map(|si| self.sketches[si].contour_ids.clone()).unwrap_or_default() } else { profiles.to_vec() };
                for cid in chk {
                    if let Some(xy) = self.contour_profile_xy(cid) {
                        if let Some(msg) = self.revolve_profile_crosses_axis(&xy, ax_o, ax_d) {
                            return crate::feature::KernelJob::refused(msg);
                        }
                    }
                }
                let line = named.then_some(crate::feature::AxisLine { origin: ax_o, dir: ax_d });
                let about = crate::feature::RevolveAbout { axis, line };
                let place = with_start(ax_o, ax_d, pl);
                let inputs = if src != 0 { vec![src] } else { Vec::new() };
                crate::feature::KernelJob::new(inputs, move |k| k.revolve_region_multi(BodyOp { src, op, body }, &profs, about, angle, place, &caps))
            })(),
        }
    }

    /// Sweep: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_sweep(&mut self, p: &mut Pass, prof: Profile, path_sketch: Id, path: Id, bo: BodyOp) -> crate::feature::KernelJob {
        let (Profile { sketch, profiles }, BodyOp { src, op, body }) = (prof, bo);
        // The profile and the path are different sketches, each with its own placement (the frame
        // of its plane).
        let prof_pl = self.sketch_place(sketch);
        let path_pl = self.sketch_place(path_sketch);
        let pth = self.sweep_path_encoded(path_sketch, path);

        match (self.encode_profiles_role(p.node, sketch, profiles, &[], crate::names::Role::Swept), pth) {
            (Some(profs), Some(pth)) => {
                let caps = self.region_cap_names(p.node, &profs);
                let inputs = if src != 0 { vec![src] } else { Vec::new() };
                crate::feature::KernelJob::new(inputs, move |k| k.sweep_multi(BodyOp { src, op, body }, &profs, prof_pl, &pth, path_pl, &caps))
            }
            (None, _) => crate::feature::KernelJob::refused(crate::errors::CoreError::SweepProfileMissing),
            (_, None) => crate::feature::KernelJob::refused(crate::errors::CoreError::SweepPathMissing),
        }
    }

    /// Combine: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    fn prep_combine(&mut self, p: &mut Pass, prof: Profile, span: CombineSpan, t: BodyOp) -> crate::feature::KernelJob {
        let CombineSpan { height, down, extent, fill } = span;
        let (sketch, profiles) = (prof.sketch, prof.profiles);
        let (src, op, body) = (t.src, t.op, t.body);
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

        match self.encode_profiles_fill(p.node, sketch, profiles, fill) {
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::ProfileNotFound),
            Some(profs) => {
                let caps = self.region_cap_names(p.node, &profs);
                let inputs = if src != 0 { vec![src] } else { Vec::new() };
                crate::feature::KernelJob::new(inputs, move |k| k.combine_region_multi(BodyOp { src, op, body }, &profs, h, pl, &caps))
            }
        }
    }

    fn live_fillet_edges(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
        // The two paths are told apart by the kind of query rather than by whether it contains numbers.
        //
        // Asking "are there recorded descriptors" is wrong: `Adjacent(Id(face))` has them, but they are face
        // numbers. Translating them onwards as edges hands the kernel a non-existent edge, and the kernel
        // segfaults, killing the program on "shell, then fillet its face".
        //
        // A description is resolved against the edges the kernel holds now, not against `regen_edges`: that is
        // filled by the post pass and is not in a bundle. Reported behaviour: a fillet of the top face of a
        // cylinder opened from a file found no edges and was refused, while the same steps on a cylinder built
        // in the session worked.
        let cur = kernel.edges(src);
        let live = if r.query.is_pick_list() {
            self.live_edge_refs(node_id, src, &r.query.picked_descs(), emap, kernel)
        } else if cur.is_empty() {
            self.resolve_edge_refs(src, r, "ref-what-fillet-edge").unwrap_or_default()
        } else {
            self.resolve_edge_refs_in(src, &cur, r, "ref-what-fillet-edge").unwrap_or_default()
        };
        // And no foreign number reaches the kernel. The kernel does not refuse a non-existent edge, it
        // crashes, so the check belongs on this side. Cheap insurance against a whole class of failures.
        let known: std::collections::HashSet<u32> = cur.into_iter().map(|e| e.id).collect();
        if known.is_empty() {
            return live;
        }
        live.into_iter().filter(|e| known.contains(e)).collect()
    }

    /// HOW MANY OF THE PICKED EDGES FIND NOTHING, each asked on its own: an edge merged with its neighbour finds the
    /// merged one and is not lost; one that is gone finds nothing. The references are only read, not rewritten.
    fn edge_refs_lost(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> usize {
        let cur = kernel.edges(src);
        let translated = self.translate_edge_refs(node_id, src, edges, emap, &cur);
        let keep = self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let lost = translated.iter().filter(|&&d| if cur.is_empty() { self.resolve_edge_ids(node_id, src, &[d]).is_empty() } else { self.resolve_edge_ids_in(&cur, node_id, &[d]).is_empty() }).count();
        self.snap_rebinds.store(keep, std::sync::atomic::Ordering::Relaxed);
        lost
    }

    fn live_edge_refs(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
        let cur = kernel.edges(src);
        let translated = self.translate_edge_refs(node_id, src, edges, emap, &cur);
        let before = self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let out = if cur.is_empty() { self.resolve_edge_ids(node_id, src, &translated) } else { self.resolve_edge_ids_in(&cur, node_id, &translated) };
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
            crate::feature::FeatureKind::Fillet { edges, .. } | crate::feature::FeatureKind::Chamfer { edges, .. } if !edges.query.picked_descs().is_empty() => {
                *edges = crate::refs::Ref::picks(found);
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
        // the named exception: the piece a person made a part of its own reads the body of the part it came from,
        // which is what keeps the two parts one cut
        let piece_out = matches!(kind, crate::feature::FeatureKind::Piece { .. });
        if let Some(bad) = kind.inputs().into_iter().find(|&inp| !piece_out && self.ref_owner(inp).is_some_and(|ro| ro != owner)) {
            return Some(crate::errors::CoreError::CrossComponentInput { input: bad });
        }
        // An input sketch on a face of another component's body is forbidden, except with an explicit external
        // reference, where the cross-component link is authorised and resolves into local space.
        if let Some(bad) = kind
            .inputs()
            .into_iter()
            .find(|&inp| self.sketch_plane_body(inp).and_then(|pb| self.body_owner(pb).map(|ro| (pb, ro))).is_some_and(|(pb, ro)| ro != owner && !self.external_authorized(owner, pb)))
        {
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

    fn apply_regen(&mut self, l: Landing, body: Id, res: Result<crate::geom::Built, crate::errors::CoreError>) -> bool {
        let Landing { node: node_id, dirty, report, kernel, emap } = l;
        match res {
            Ok(crate::geom::Built { mesh, faces }) => {
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
                                                    // and what it left out is said, or the old word taken away
                                                    // what the kernel left out adds to what the references lost before it
                if let Some((asked, dropped)) = kernel.take_dropped_edges(body) {
                    let (asked0, lost0) = match self.regen_warnings.get(&node_id) {
                        Some(crate::errors::CoreError::EdgesDropped { asked, dropped }) => (*asked, *dropped),
                        _ => (0, 0),
                    };
                    self.regen_warnings.insert(node_id, crate::errors::CoreError::EdgesDropped { asked: asked.max(asked0), dropped: dropped + lost0 });
                }
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

impl Project {
    /// The error of the first value of node `node` written as an expression that does not count with `vars`.
    fn dim_expr_error(&self, node: Id, vars: &std::collections::HashMap<String, f64>) -> Option<crate::errors::CoreError> {
        self.feat_dims.get(&node)?.values().map(|e| e.trim()).filter(|e| !e.is_empty()).find_map(|e| crate::expr::eval(e, vars).err()).map(crate::errors::CoreError::Expr)
    }
}

/// `mesh` made lighter within `simplify` mm, or as it is where that is zero.
fn lighter(mesh: crate::geom::Mesh, simplify: f64) -> crate::geom::Mesh {
    if simplify > 0.0 {
        crate::mesh_simplify::simplify(&mesh, simplify)
    } else {
        mesh
    }
}

/// The recognition's tolerance factor, raised so that a surface is held within `simplify` mm at least: a mesh made
/// lighter stands off the surface it was made from by up to that, and held to the recognition's own hundred-thousandth
/// of its size no plane or cylinder lay on it - measured on a smooth handle simplified by 0.01 mm, 8 544 regions came
/// out of 21 234 triangles and 1 164 of them stayed mesh; within the simplification, 159 regions and none.
fn within_simplification(mesh: &crate::geom::Mesh, tol: f64, simplify: f64) -> f64 {
    let Some(b) = mesh.bounds().filter(|_| simplify > 0.0) else { return tol };
    let size = ((b.max.x - b.min.x).powi(2) + (b.max.y - b.min.y).powi(2) + (b.max.z - b.min.z).powi(2)).sqrt();
    // the recognition holds a corner within a hundred-thousandth of the mesh's size times this factor
    tol.max(simplify / (size * 1e-5).max(1e-12))
}

/// How many edges a reference names: the length of a plain pick list, and none for a description, whose numbers
/// name the faces or the seed it is phrased through rather than edges.
fn asked_edge_count(edges: &crate::refs::Ref) -> usize {
    if edges.query.is_pick_list() {
        edges.query.picked_descs().len()
    } else {
        0
    }
}

/// The refusal for an edge reference that found nothing: the `asked` named edges lost for a pick list, a description
/// that finds nothing otherwise. Reported behaviour: "every edge of the top face" was refused as "Not one of the 1
/// named edges is left", the face number counted as an edge.
fn edges_lost(edges: &crate::refs::Ref, asked: usize) -> crate::errors::CoreError {
    if edges.query.is_pick_list() {
        crate::errors::CoreError::EdgesNotFound { asked }
    } else {
        crate::errors::CoreError::DescribedEdgesNotFound
    }
}
