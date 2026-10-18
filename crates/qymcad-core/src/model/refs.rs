//! THE REFERENCES TO FACES, EDGES AND VERTICES: captured, resolved, rebound when lost.

use super::*;

/// A SKETCH ON A FACE WHOSE NAME IS GONE and the face it is to stand on now: found by the name (a merge, a piece's
/// whole, the same recipe) or by place, which marks it for the person.
pub(crate) struct SketchFaceFix {
    pub sketch: Id,
    pub body: Id,
    pub face: u32,
    pub by_place: bool,
    pub what: String,
}

/// A FACE FOUND FOR A REFERENCE whose name is not on its body, and whether it was by place.
pub(crate) struct FaceFind {
    pub face: u32,
    pub by_place: bool,
}

/// HOW CLOSE AN ELEMENT FOUND BY PLACE STANDS TO WHERE A REFERENCE SAW IT: a micrometre, the tolerance a node deleted
/// below translates its references with. The place is the last step, after the name, its recorded merge and its
/// structure, and only the element standing exactly there is taken: within 2% of the body (at least 0.5 mm) the
/// neighbour an edit moved into the place was taken, and nothing said so.
const PLACE_EXACT: f64 = 1e-6;

impl Project {
    /// Repair lost face references in one pass before the build, instead of guessing inside every resolve.
    ///
    /// Falling back to a geometric fingerprint (co-directed normal plus nearest centre) inside each resolve
    /// repeats the guess on every rebuild: the reference is "found" every time, possibly a different one
    /// every time, and nothing reports it. Instead:
    ///
    /// * resolution goes by id, and the fingerprint only proposes a candidate;
    /// * the id found is written back into the key, so the guess happens once rather than every time;
    /// * the repair is recorded in the report and visible in the tree instead of dissolving.
    pub fn rebind_lost_face_refs_for_test(&mut self, report: &mut crate::feature::RegenReport) {
        self.rebind_lost_face_refs(report);
    }

    pub(crate) fn rebind_lost_face_refs(&mut self, report: &mut crate::feature::RegenReport) {
        use crate::feature::{Rebind, SketchPlane};
        // Holes are deliberately not handled here: their reference became a query, which needs no matching
        // — it either finds the face by recipe or refuses with a named reason.
        //
        // Sketches placed on a face of a body, whose face is not on it by its name: the name first - the record of a
        // merge the face yielded in, a piece returned to its whole, the one face of the same recipe - and the place
        // last, marked for the person. Asked by place alone, a sketch on a wall merged into its neighbour was taken
        // over by place, though the record said where the wall went.
        let mut fixes: Vec<SketchFaceFix> = Vec::new();
        for s in &self.sketches {
            let SketchPlane::Face(body, ref key) = s.plane else { continue };
            let Some(fs) = self.regen_faces.get(&body) else { continue };
            let live: Vec<u32> = fs.iter().map(|f| f.id).collect();
            if key.id == 0 || live.contains(&key.id) {
                continue;
            }
            let Some(found) = self.find_face(fs, key.id, Some(key)) else { continue };
            let what = format!("rebind-sketch-face#{}: {} -> {}", s.name, key.id, found.face);
            fixes.push(SketchFaceFix { sketch: s.id, body, face: found.face, by_place: found.by_place, what });
        }
        for fix in fixes {
            if let Some(s) = self.sketches.iter_mut().find(|s| s.id == fix.sketch) {
                if let SketchPlane::Face(_, key) = &mut s.plane {
                    key.id = fix.face;
                }
            }
            if fix.by_place {
                report.rebinds.push(Rebind { node: fix.sketch, body: fix.body, what: fix.what });
                self.note_found_by_place(fix.sketch);
            }
        }
    }

    /// WHERE THE FACE A REFERENCE HOLDS AS `id` STANDS NOW among `faces`, its name not on them: by the name first - the
    /// record of a merge it yielded in, a piece returned to its whole, the one face of the same recipe - and by place
    /// last, the one face standing exactly where the reference saw it (`seen`, to `PLACE_EXACT`), facing the same way.
    /// The nearest face facing roughly the same way (within 25 deg) was taken before, however far, and a sketch went
    /// onto a face an edit had moved into the place, saying nothing.
    pub(crate) fn find_face(&self, faces: &[crate::geom::MeshFace], id: u32, seen: Option<&crate::feature::FaceKey>) -> Option<FaceFind> {
        let live: Vec<u32> = faces.iter().map(|f| f.id).collect();
        if let Some(face) = self.names.absorbed_target_among(id, &live).or_else(|| self.names.same_recipe(id, &live)) {
            return Some(FaceFind { face, by_place: false });
        }
        let key = seen?;
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let off = |c: &crate::geom::Point3| ((c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2)).sqrt();
        let there: Vec<u32> = faces.iter().filter(|f| f.id != 0 && dot(f.normal, key.normal) > 1.0 - PLACE_EXACT && off(&f.centroid) < PLACE_EXACT).map(|f| f.id).collect();
        match there.as_slice() {
            [one] => Some(FaceFind { face: *one, by_place: true }),
            _ => None,
        }
    }

    /// Say that a reference of step `node` was found by place: the rebuild marks the step when it ends.
    pub(crate) fn note_found_by_place(&self, node: Id) {
        if let Ok(mut now) = self.placed_now.lock() {
            now.insert(node);
        }
    }

    /// THE STEPS FOUND BY PLACE IN THIS REBUILD JOIN THOSE AWAITING THE PERSON, and each of those standing in the
    /// timeline built, with nothing else to say, is yellow with the words (`CoreError::RefFoundByPlace`).
    pub(crate) fn mark_found_by_place(&mut self) {
        let now: Vec<Id> = self.placed_now.lock().map(|mut s| std::mem::take(&mut *s).into_iter().collect()).unwrap_or_default();
        self.placed_refs.extend(now);
        let live: std::collections::HashSet<Id> = self.timeline.iter().map(|n| n.id).collect();
        self.placed_refs.retain(|n| live.contains(n));
        for n in &self.placed_refs {
            if !self.regen_errors.contains_key(n) && !self.regen_warnings.contains_key(n) {
                self.regen_warnings.insert(*n, crate::errors::CoreError::RefFoundByPlace);
            }
        }
    }

    /// WHAT STEP `node` WARNS OF: what its last rebuild left it saying, or - in a document opened and not yet rebuilt -
    /// that a reference of it was found by place and awaits the person.
    pub fn node_warning(&self, node: Id) -> Option<crate::errors::CoreError> {
        self.regen_warnings.get(&node).cloned().or_else(|| self.placed_refs.contains(&node).then_some(crate::errors::CoreError::RefFoundByPlace))
    }

    /// THE PERSON ACCEPTS what was found by place for step `node`: the mark goes, the reference stays as it was found.
    /// Answers whether there was a mark.
    pub fn accept_found_by_place(&mut self, node: Id) -> bool {
        if matches!(self.regen_warnings.get(&node), Some(crate::errors::CoreError::RefFoundByPlace)) {
            self.regen_warnings.remove(&node);
        }
        self.placed_refs.remove(&node)
    }

    /// Every face of a body as reference resolution sees it: the pool a query is evaluated against.
    ///
    /// Built from the live rebuild rather than from what a reference recorded earlier. That is the whole
    /// point: a query asks today's geometry instead of comparing against last year's snapshot.
    pub fn face_pool(&self, body: Id) -> Vec<crate::refs::Candidate> {
        self.regen_faces
            .get(&body)
            .map(|fs| {
                fs.iter()
                    .map(|f| crate::refs::Candidate {
                        edge: None, // A face has no endpoints; this field belongs to edges.
                        desc: f.id,
                        centroid: [f.centroid.x, f.centroid.y, f.centroid.z],
                        normal: f.normal,
                        area: f.area,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve a reference to a set of faces into descriptors, or into a named refusal.
    ///
    /// The order is stable (it matches the live rebuild), so the result of an operation does not shift
    /// between runs: the kernel builds from this list, and reordering it changes the names of the faces it
    /// produces.
    pub fn resolve_face_refs(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        let pool = self.face_pool(body);
        r.resolve(what, &pool, &self.names, &pool)
    }

    /// Every edge of a body as resolution sees it. The edge direction plays the role of a normal and the
    /// length the role of an area, so descriptive queries ("along the axis", "the longest") work on edges
    /// without any separate code.
    pub fn edge_pool(&self, body: Id) -> Vec<crate::refs::Candidate> {
        self.regen_edges.get(&body).map(|es| Self::edge_candidates(es)).unwrap_or_default()
    }

    /// A list of edges as resolution sees it, wherever the list came from.
    pub(crate) fn edge_candidates(es: &[crate::geom::MeshEdge]) -> Vec<crate::refs::Candidate> {
        es.iter()
            .map(|e| crate::refs::Candidate {
                desc: e.id,
                centroid: e.mid,
                normal: e.dir,
                area: ((e.b[0] - e.a[0]).powi(2) + (e.b[1] - e.a[1]).powi(2) + (e.b[2] - e.a[2]).powi(2)).sqrt(),
                edge: Some(crate::refs::EdgeGeom { a: e.a, b: e.b, center: e.center, axis: e.axis, radius: e.radius }),
            })
            .collect()
    }

    /// Every vertex of a body as resolution sees it: a point and its name. The names are the bridge's, given with the
    /// edges (`MeshEdge::ends`: the vertex of the topology an end stands on, named by the edges meeting at it); the
    /// table holds a name from the first time a vertex is referenced.
    pub fn vertex_pool(&mut self, body: Id) -> Vec<crate::refs::Candidate> {
        self.vertex_spots(body)
            .into_iter()
            .map(|v| {
                let desc = self.names.intern_vertex(v.name);
                crate::refs::Candidate { desc, centroid: v.at, normal: [0.0, 0.0, 1.0], area: 0.0, edge: None }
            })
            .collect()
    }

    /// The vertices of a body as the bridge named them, without their descriptors: where each stands and its name.
    ///
    /// Kept apart from [`Project::vertex_pool`] because this is asked far more often than names are taken: drawing and
    /// hit-testing hold `&self` and take no name into the table, and a name goes in exactly when a vertex is referenced.
    pub fn vertex_spots(&self, body: Id) -> Vec<VertexSpot> {
        let Some(edges) = self.regen_edges.get(&body) else { return Vec::new() };
        let mut spots: Vec<VertexSpot> = Vec::new();
        for e in edges {
            for (at, name) in [(e.a, e.ends[0]), (e.b, e.ends[1])] {
                if name.edges[0] != 0 && !spots.iter().any(|s| s.name == name) {
                    spots.push(VertexSpot { at, name });
                }
            }
        }
        spots
    }

    /// Resolve a reference to vertices. The edge pool is passed alongside the vertex pool, because a vertex
    /// is described through its edges and has no other vocabulary.
    pub fn resolve_vertex_refs(&mut self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        let pool = self.vertex_pool(body);
        let edges = self.edge_pool(body);
        r.resolve(what, &pool, &self.names, &edges)
    }

    /// Where the vertex with this name is: a point, or `None` when the body no longer has it.
    pub fn vertex_point(&mut self, body: Id, desc: u32) -> Option<[f64; 3]> {
        self.vertex_pool(body).into_iter().find(|c| c.desc == desc).map(|c| c.centroid)
    }

    /// Resolve a reference to edges. The face pool is passed alongside the edge pool, because an edge query
    /// is always phrased through faces ("every edge of this face", "the seam between these two sets").
    pub fn resolve_edge_refs(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        r.resolve(what, &self.edge_pool(body), &self.names, &self.face_pool(body))
    }

    /// The same against an explicit list of the body's edges, as the kernel holds them now.
    ///
    /// `regen_edges` is filled by the post pass of a rebuild and is not written into a bundle: in the middle of
    /// a pass it holds the previous state of the body, and right after a file is opened it holds nothing. A
    /// query resolved against it there found no edges at all, while the faces it is phrased through were known.
    pub fn resolve_edge_refs_in(&self, body: Id, edges: &[crate::geom::MeshEdge], r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
        r.resolve(what, &Self::edge_candidates(edges), &self.names, &self.face_pool(body))
    }

    /// Resolve a reference to a single face into a descriptor, a centre and a normal, or into a named
    /// refusal.
    ///
    /// A refusal here is the job, not a failure: it states what was looked for and where it used to be,
    /// instead of silently matching a similar face. Silent matching is what moved holes and fillets onto
    /// neighbouring geometry.
    pub fn resolve_face_ref(&self, body: Id, r: &crate::refs::Ref, what: &str) -> Result<crate::refs::Candidate, crate::refs::RefError> {
        let pool = self.face_pool(body);
        let found = r.resolve(what, &pool, &self.names, &pool)?;
        let d = found.first().copied().unwrap_or(0);
        pool.iter().find(|c| c.desc == d).copied().ok_or_else(|| crate::refs::RefError::Lost { what: what.into(), was: r.hint })
    }

    pub fn resolve_face(&self, body: Id, key: &crate::feature::FaceKey) -> ([f64; 3], [f64; 3]) {
        // Resolution goes by persistent id only. Matching a similar face by fingerprint is deliberately not
        // done here: it silently substituted the wrong face and repeated on every rebuild. Lost references
        // are repaired by the single `rebind_lost_face_refs` pass before the build, which writes the id it
        // found into the key and records the repair in the report. A reference arrives here either with a
        // current id or with its own fingerprint when there was nothing to repair against (the body is not
        // built), and in that case the fingerprint is returned honestly.
        if let Some(faces) = self.regen_faces.get(&body) {
            if key.id != 0 {
                if let Some(f) = faces.iter().find(|f| f.id == key.id) {
                    return ([f.centroid.x, f.centroid.y, f.centroid.z], f.normal);
                }
            }
        }
        (key.centroid, key.normal)
    }

    /// Frame of a sketch plane, by sketch id.
    pub fn sketch_frame_by_id(&self, sid: Id) -> Option<crate::feature::PlaneFrame> {
        self.sketch_index(sid).and_then(|si| self.sketch_frame(si))
    }

    /// Profile of a sketch by id: its first closed contour, flattened to XY.
    pub fn sketch_profile_by_id(&self, sid: Id) -> Option<Vec<f64>> {
        let si = self.sketch_index(sid)?;
        self.sketch_profile_xy(si).map(|(_, xy)| xy)
    }

    /// Profile for a feature: the specific contour `profile` when it is set, otherwise the first closed
    /// contour of the sketch.
    pub fn feature_profile_xy(&self, sketch: Id, profile: Id) -> Option<Vec<f64>> {
        if profile != 0 {
            self.contour_profile_xy(profile)
        } else {
            self.sketch_profile_by_id(sketch)
        }
    }

    /// Exact profile of a feature for the kernel: the outer contour (`profile`, or the first closed one)
    /// plus its holes, in the `geom::encode_profile` encoding — real edges producing exact faces rather
    /// than a faceted approximation.
    pub fn feature_profile_encoded(&self, sketch: Id, profile: Id) -> Option<Vec<f64>> {
        self.feature_profile_encoded_fill(sketch, profile, &[])
    }

    /// Loft encoding. For each section sketch the contour is taken (`contours[i]`, or the first closed one)
    /// and the result is the concatenation of `loop_block`s, `offsets[nsec+1]` marking where each section
    /// starts, and `places[nsec*12]` holding the 3x4 placements of the section planes. `None` when fewer
    /// than two valid sections were found, since a loft needs at least two.
    pub fn loft_encoded(&self, sketches: &[Id], contours: &[Id]) -> Option<LoftSectionsEncoded> {
        self.loft_encoded_with(sketches, contours, &std::collections::HashMap::new())
    }

    /// Capture geometric snapshots of faces `faces` of body `src` and attach them to feature `fid`, the way
    /// `capture_edge_refs` does for edges.
    pub(crate) fn capture_face_refs(&mut self, fid: Id, src: Id, faces: &[u32]) {
        let Some(fs) = self.regen_faces.get(&src) else { return };
        let refs: Vec<ElemSnapshot> =
            faces.iter().filter_map(|&id| fs.iter().find(|f| f.id == id).map(|f| ElemSnapshot { id, at: [f.centroid.x, f.centroid.y, f.centroid.z], dir: f.normal })).collect();
        if !refs.is_empty() {
            self.face_refs.insert(fid, refs);
        }
    }

    /// Resolve a face reference: by its name, the record of a merge it yielded in, a piece returned to its whole, the one
    /// face of the same recipe, and last by place - only the one face standing exactly where the reference saw it, the
    /// step marked for the person (`find_face`). The order and its reasons: `crates/qymcad-kernel/NAMING.md`, "How a
    /// reference finds its element".
    ///
    /// A positional number is trusted only while the snapshot agrees with it: as soon as some faces gain names, the
    /// numbering of the rest shifts and a number stored in the file starts pointing at a different face, silently.
    pub fn resolve_face_id(&self, fid: Id, src: Id, stored: u32) -> Option<u32> {
        let fs = self.regen_faces.get(&src)?;
        if fs.is_empty() {
            return None;
        }
        let snap = self.face_refs.get(&fid).and_then(|v| v.iter().find(|s| s.id == stored)).map(|s| (s.at, s.dir));
        let live = |id: u32| fs.iter().find(|f| f.id == id);
        if let Some(f) = live(stored) {
            let ok = crate::names::NameTable::is_named(stored)
                || match snap {
                    None => true,
                    Some((c, _)) => {
                        let d = (f.centroid.x - c[0]).powi(2) + (f.centroid.y - c[1]).powi(2) + (f.centroid.z - c[2]).powi(2);
                        d.sqrt() <= self.face_snap_tol(fs)
                    }
                };
            if ok {
                return Some(stored);
            }
        }
        let ids: Vec<u32> = fs.iter().map(|f| f.id).collect();
        if let Some(t) = self.names.absorbed_target_among(stored, &ids) {
            return Some(t);
        }
        // Then the structure of the name: the one live piece of the same wall.
        if let Some(t) = self.names.same_recipe(stored, &ids) {
            return Some(t);
        }
        // The only face of the body: there is nothing to confuse it with, so this is proof rather than
        // similarity.
        if fs.len() == 1 {
            return Some(fs[0].id);
        }
        // THE PLACE, LAST, ONLY WHERE IT IS EXACT: one face standing where the reference saw it, to `PLACE_EXACT`.
        let (c, n) = snap?;
        let mut scored: Vec<(f64, u32)> = fs
            .iter()
            .map(|f| {
                let d = ((f.centroid.x - c[0]).powi(2) + (f.centroid.y - c[1]).powi(2) + (f.centroid.z - c[2]).powi(2)).sqrt();
                let dot = (f.normal[0] * n[0] + f.normal[1] * n[1] + f.normal[2] * n[2]).clamp(-1.0, 1.0);
                (d + (1.0 - dot) * 10.0, f.id)
            })
            .collect();
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let best = *scored.first()?;
        let second = scored.get(1).map(|x| x.0).unwrap_or(f64::MAX);
        let hit = best.0 < PLACE_EXACT && second >= PLACE_EXACT;
        if hit {
            self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
            self.note_found_by_place(fid);
            if std::env::var("QYM_SNAP_DEBUG").is_ok() {
                eprintln!("face fallback: feature {fid}, reference {stored} = {} -> {} = {}", self.names.describe(stored), best.1, self.names.describe(best.1));
            }
        }
        hit.then_some(best.1)
    }

    /// Face snapshot tolerance, taken from the bounding box of the body, as for edges: an edit moves
    /// geometry by an amount proportional to the part rather than to a hard-coded number.
    pub(crate) fn face_snap_tol(&self, fs: &[crate::geom::MeshFace]) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for f in fs {
            let c = [f.centroid.x, f.centroid.y, f.centroid.z];
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        (diag * 0.02).max(0.5)
    }

    /// Capture geometric snapshots of edges `edges` of body `src` (from `regen_edges`) and attach them to
    /// feature `fid`, so an edge selection can be repaired when the topology changes. An empty selection
    /// (every edge, or no geometry) captures nothing.
    pub(crate) fn capture_edge_refs(&mut self, fid: Id, src: Id, edges: &[u32]) {
        let Some(es) = self.regen_edges.get(&src) else { return };
        let refs: Vec<ElemSnapshot> = edges.iter().filter_map(|&eid| es.iter().find(|e| e.id == eid).map(|e| ElemSnapshot { id: eid, at: e.mid, dir: e.dir })).collect();
        if !refs.is_empty() {
            self.edge_refs.insert(fid, refs);
        }
    }

    /// Resolve the stored edge ids of feature `fid` against the current edges of body `src`.
    ///
    /// A valid id passes through as is. A stale one (absent from `regen_edges[src]` because the topology changed) is
    /// found by the record of the name it yielded to, the canonical pair of its faces, the structure of its name, and
    /// last by place - only the one edge with its middle exactly where the reference saw it, the step marked for the
    /// person; otherwise it drops out, and the node says so. The order and its reasons: `crates/qymcad-kernel/NAMING.md`,
    /// "How a reference finds its element". Returns the current ids for the kernel.
    pub fn resolve_edge_ids(&self, fid: Id, src: Id, stored: &[u32]) -> Vec<u32> {
        self.resolve_edge_ids_in(self.regen_edges.get(&src).map(|v| v.as_slice()).unwrap_or(&[]), fid, stored)
    }

    /// Like `resolve_edge_ids`, but against an explicit list of current edges: regenerate has fresh ones
    /// from the kernel, while `regen_edges` may be empty or stale in the middle of a pass.
    ///
    /// The place is the middle of the edge and its direction, both as the reference saw them. An edge merged with a
    /// collinear neighbour is found through the record of the merge before the place is asked.
    pub fn resolve_edge_ids_in(&self, es: &[crate::geom::MeshEdge], fid: Id, stored: &[u32]) -> Vec<u32> {
        if es.is_empty() {
            return stored.to_vec();
        }
        let snaps = self.edge_refs.get(&fid);
        // The tolerance comes from the scale of the body, not from a hard-coded number.
        //
        // A fixed "accept an edge within 0.5 mm of its former place" has a measured price: moving one
        // sketch point by 1.5 mm changed 22 edge names, and a fillet over 36 selected edges found 20 —
        // silently, without an error. The repair held snapshots of every lost edge and refused to use them:
        // right by its own yardstick, wrong in substance, since it is the same edge that simply moved with
        // the edit.
        //
        // An edit moves geometry by an amount proportional to the part, not to half a millimetre, so the
        // part is the yardstick: the tolerance is a fraction of its bounding box, with a floor for tiny
        // bodies.
        //
        // The fraction was chosen by measurement. At 0.05 the repair returned 34 of 36 edges but also
        // pulled in edges the fillet cannot build on, so the whole feature failed. At 0.02 all 36 are found
        // and nothing breaks.
        //
        // That yardstick now only witnesses a positional number; the search by place itself, after the name, its
        // recorded merge, its canonical pair and its structure, takes an edge standing exactly where the reference
        // saw it (`PLACE_EXACT`) and nothing nearby: a fillet on the wrong edge is worse than a lost one.
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for e in es {
            for k in 0..3 {
                lo[k] = lo[k].min(e.mid[k]);
                hi[k] = hi[k].max(e.mid[k]);
            }
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        let tol = (diag * 0.02).max(0.5);
        let live_ids: Vec<u32> = es.iter().map(|e| e.id).collect();
        let mut out = Vec::with_capacity(stored.len());
        for &eid in stored {
            // A positional number is trusted only while the snapshot agrees with it.
            //
            // A name is an address: while it lives, it denotes that exact element. A positional number is
            // not an address; it means "this many along in today's numbering". As soon as naming improves
            // and some elements gain names, the numbering of the rest shifts and a number stored in a file
            // starts pointing at a different edge, silently.
            //
            // Measured: enabling names for a thread relief moved all 36 fillet references, some by 118 mm,
            // while neither the references nor the geometry had changed.
            //
            // So the number needs a witness: the place snapshot taken at the same time as the reference. If
            // they agree, the number is used (within a session the numbering does not move, so the behaviour
            // is unchanged). If they disagree, the number proves nothing and resolution falls through to
            // name translation and snapshot repair. With no snapshot the number is trusted as before, there
            // being no other witness.
            let numeric_ok = crate::names::NameTable::is_named(eid)
                || match (es.iter().find(|e| e.id == eid), snaps.and_then(|v| v.iter().find(|s| s.id == eid))) {
                    (Some(cur), Some(snap)) => {
                        let mid = snap.at;
                        let d = ((cur.mid[0] - mid[0]).powi(2) + (cur.mid[1] - mid[1]).powi(2) + (cur.mid[2] - mid[2]).powi(2)).sqrt();
                        d <= tol
                    }
                    _ => true,
                };
            if numeric_ok && es.iter().any(|e| e.id == eid) {
                if !out.contains(&eid) {
                    out.push(eid);
                }
                continue;
            }
            // Name translation through a recorded merge, before and instead of geometry.
            //
            // An edge name is a pair of face names. When a union merges two coplanar faces into one, one
            // face name yields to the other, so the edge is named differently even though it is the same
            // edge of the same body. The kernel reported which name yielded to which, so this is not a
            // search for something similar but a translation of a recorded fact: old pair to new pair. The
            // geometric fallback stays after it, and only for what has no translation.
            if let Some(t) = self.names.absorbed_target_among(eid, &live_ids) {
                if !out.contains(&t) {
                    out.push(t);
                }
                continue;
            }
            // An edge between a piece and its neighbour is the same edge. Once an operation starts cutting
            // a face, the edge is named by the pair "piece plus neighbour" while the reference remembers
            // "whole plus neighbour" (or the other way round). The canonical form of the pair is compared,
            // and the match is accepted only when there is a single candidate: ambiguity is worse than loss,
            // because a fillet on the wrong edge cannot be undone by the user.
            //
            // The ordinal within a pair does not take part in the decision. The edge number inside a pair of
            // faces is assigned by sorting on kernel ids, that is, by order rather than by recipe. When a
            // pair of faces carries several edges, names cannot tell them apart, and taking "the one whose
            // ordinal matches" is a coin flip: measured, that substituted a twin edge and an R2.0 fillet
            // stopped building entirely. So the match is taken only when the pair has exactly one edge; the
            // ambiguous case goes to the geometric fallback, which at least looks at the place.
            if let Some((pair, _)) = self.names.canonical_edge(eid) {
                let mut same = es.iter().filter(|e| self.names.canonical_edge(e.id).map(|(p, _)| p) == Some(pair));
                if let (Some(one), None) = (same.next(), same.next()) {
                    let id = one.id;
                    if !out.contains(&id) {
                        out.push(id);
                    }
                    continue;
                }
            }
            // Then the structure of the name: the one edge between faces of the same recipe as its pair.
            let ids: Vec<u32> = es.iter().map(|e| e.id).collect();
            if let Some(t) = self.names.same_recipe(eid, &ids) {
                if !out.contains(&t) {
                    out.push(t);
                }
                continue;
            }
            if let Some(&ElemSnapshot { at: mid, dir, .. }) = snaps.and_then(|v| v.iter().find(|s| s.id == eid)) {
                // the middle where the reference saw it: an edge on the same line that only runs through that point is
                // another edge (measured: the boundary of a rounding beside the edge, along the same line)
                let off = |e: &crate::geom::MeshEdge| ((e.mid[0] - mid[0]).powi(2) + (e.mid[1] - mid[1]).powi(2) + (e.mid[2] - mid[2]).powi(2)).sqrt();
                let mut scored: Vec<(f64, u32)> = es
                    .iter()
                    .map(|e| {
                        let dot = (e.dir[0] * dir[0] + e.dir[1] * dir[1] + e.dir[2] * dir[2]).abs().min(1.0);
                        (off(e) + (1.0 - dot) * 10.0, e.id)
                    })
                    .collect();
                scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(&(best, bid)) = scored.first() {
                    let second = scored.get(1).map(|x| x.0).unwrap_or(f64::MAX);
                    if best < PLACE_EXACT && second >= PLACE_EXACT && !out.contains(&bid) {
                        self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
                        self.note_found_by_place(fid);
                        if std::env::var("QYM_SNAP_DEBUG").is_ok() {
                            eprintln!("edge fallback: feature {fid}, reference {} = {} -> {bid} = {}", eid, self.names.describe(eid), self.names.describe(bid));
                        }
                        out.push(bid); // Unambiguously repaired (for example a fragment mapping onto a merged edge).
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::{GeoName, Role};

    /// A face of `body` named `id`, standing at `x` on the X axis and facing up.
    fn face(id: u32, x: f64) -> crate::geom::MeshFace {
        crate::geom::MeshFace { triangles: Vec::new(), normal: [0.0, 0.0, 1.0], centroid: crate::geom::Point3 { x, y: 0.0, z: 0.0 }, area: 1.0, id }
    }

    /// A REFERENCE FOUND BY PLACE ONLY WHERE THE PLACE IS EXACT: its name gone, a face standing exactly where the
    /// reference saw it is taken and counted; a face an edit moved 0.3 mm off that place is not - within 2% of the body
    /// (at least 0.5 mm) it was, silently, though it is the neighbour that came there.
    #[test]
    fn a_face_is_found_by_place_only_standing_exactly_there() {
        let mut p = Project::default();
        let (src, node) = (1, 2);
        let gone = p.names.intern_face(GeoName::new(9, Role::Wall, 5));
        let near = p.names.intern_face(GeoName::new(9, Role::CapEnd, 0));
        let far = p.names.intern_face(GeoName::new(9, Role::CapStart, 0));
        p.face_refs.insert(node, vec![ElemSnapshot { id: gone, at: [0.0, 0.0, 0.0], dir: [0.0, 0.0, 1.0] }]);
        p.regen_faces.insert(src, vec![face(near, 0.3), face(far, 10.0)]);
        assert_eq!(p.resolve_face_id(node, src, gone), None, "a face 0.3 mm off the place was taken for the lost one");
        p.regen_faces.insert(src, vec![face(near, 0.0), face(far, 10.0)]);
        let before = p.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(p.resolve_face_id(node, src, gone), Some(near), "the face standing exactly where the reference saw it was not taken");
        assert_eq!(p.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed), before + 1, "a face found by place is not counted");
    }

    /// AN EDGE FOUND BY PLACE STANDS WHERE THE REFERENCE SAW ITS MIDDLE: an edge on the same line that only runs through
    /// that point - the boundary of a rounding beside it - is another edge and is not taken.
    #[test]
    fn an_edge_on_the_same_line_through_the_old_middle_is_not_the_lost_edge() {
        let mut p = Project::default();
        let node = 2;
        let pair = [7, 8].map(|k| p.names.intern_face(GeoName::new(9, Role::Wall, k)));
        let gone = p.names.intern_edge(crate::names::EdgeName::new(pair[0], pair[1], 0));
        let other = p.names.intern_edge(crate::names::EdgeName::new(pair[0], pair[1] + 5, 0));
        let edge = |id: u32, a: f64, b: f64| crate::geom::MeshEdge { id, mid: [(a + b) / 2.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0], a: [a, 0.0, 0.0], b: [b, 0.0, 0.0], ..Default::default() };
        p.edge_refs.insert(node, vec![ElemSnapshot { id: gone, at: [5.0, 0.0, 0.0], dir: [1.0, 0.0, 0.0] }]);
        let longer = [edge(other, 0.0, 30.0)];
        assert!(p.resolve_edge_ids_in(&longer, node, &[gone]).is_empty(), "an edge on the same line through the old middle was taken for the lost one");
        let there = [edge(other, 2.0, 8.0)];
        assert_eq!(p.resolve_edge_ids_in(&there, node, &[gone]), vec![other], "the edge with its middle where the reference saw it was not taken");
    }

    /// A REFERENCE TO A PIECE FOLLOWS ITS WHOLE AND THEN THE NAME THE WHOLE'S EDGE YIELDED TO. A rounding picked "piece 1
    /// of a round wall beside itself"; the edge once wearing "the wall beside itself" lies, after a cut, between the wall
    /// and the wall of the cut, and is named after them, the old name recorded as yielded. Read through its pair alone,
    /// the reference took the seam of the wall - the one other edge between the wall and itself - and the rounding lost
    /// its surface without a word (measured on a sample: 33 faces of a body in the file, 32 rebuilt).
    #[test]
    fn a_reference_to_a_piece_follows_the_name_its_whole_yielded() {
        let mut p = Project::default();
        let node = 2;
        let round = p.names.intern_face(GeoName::new(5, Role::Wall, 96));
        let piece = p.names.intern_face(GeoName { split: 1, ..GeoName::new(5, Role::Wall, 96) });
        let cut = p.names.intern_face(GeoName::new(6, Role::Wall, 157));
        let picked = p.names.intern_edge(crate::names::EdgeName::new(piece, piece, 0));
        let carried = p.names.intern_edge(crate::names::EdgeName::new(round, round, 0));
        let beside_cut = p.names.intern_edge(crate::names::EdgeName::new(round, cut, 0));
        let seam = p.names.intern_edge(crate::names::EdgeName::new(round, round, 1));
        p.names.absorb(carried, beside_cut);
        let edge = |id: u32, x: f64| crate::geom::MeshEdge { id, mid: [x, 0.0, 0.0], dir: [0.0, 0.0, 1.0], ..Default::default() };
        let live = [edge(beside_cut, 0.0), edge(seam, 10.0)];
        assert_eq!(p.resolve_edge_ids_in(&live, node, &[picked]), vec![beside_cut], "the reference to a piece took another edge of its pair");
    }

    /// A PIECE RETURNS TO ITS WHOLE ONLY WHILE THE FACE IS WHOLE AGAIN. A rounding picked the edge of piece 1 of a wall;
    /// the pieces were numbered anew, and the edge of the whole - the piece beside it - was taken for it without a word.
    /// With another piece of the wall alive the reference is found where it stood, and the step is marked.
    #[test]
    fn a_piece_beside_the_whole_is_not_the_whole() {
        let mut p = Project::default();
        let node = 2;
        let wall = GeoName::new(206, Role::Wall, 167);
        let whole = p.names.intern_face(wall);
        let [old_piece, new_piece] = [1, 7].map(|split| p.names.intern_face(GeoName { split, ..wall }));
        let floor = p.names.intern_face(GeoName::new(35, Role::Wall, 30));
        let picked = p.names.intern_edge(crate::names::EdgeName::new(floor, old_piece, 0));
        let of_whole = p.names.intern_edge(crate::names::EdgeName::new(floor, whole, 0));
        let of_piece = p.names.intern_edge(crate::names::EdgeName::new(floor, new_piece, 0));
        let edge = |id: u32, x: f64| crate::geom::MeshEdge { id, mid: [x, 0.0, 0.0], dir: [0.0, 0.0, 1.0], ..Default::default() };
        p.edge_refs.insert(node, vec![ElemSnapshot { id: picked, at: [10.0, 0.0, 0.0], dir: [0.0, 0.0, 1.0] }]);
        let live = [edge(of_whole, 0.0), edge(of_piece, 10.0)];
        assert_eq!(p.resolve_edge_ids_in(&live, node, &[picked]), vec![of_piece], "the edge of a piece went to the edge of the piece beside it");
        assert!(p.placed_now.lock().is_ok_and(|s| s.contains(&node)), "the edge found where it stood is not marked");
    }
}
