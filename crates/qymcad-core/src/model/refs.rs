//! THE REFERENCES TO FACES, EDGES AND VERTICES: captured, resolved, rebound when lost.

use super::*;

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
        // Candidate by fingerprint among the faces of the body: co-directed normal, nearest centre.
        let candidate = |faces: &[crate::geom::MeshFace], key: &crate::feature::FaceKey| -> Option<u32> {
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let d2 = |c: &crate::geom::Point3| (c.x - key.centroid[0]).powi(2) + (c.y - key.centroid[1]).powi(2) + (c.z - key.centroid[2]).powi(2);
            faces.iter().filter(|f| f.id != 0 && dot(f.normal, key.normal) > 0.9).min_by(|a, b| d2(&a.centroid).partial_cmp(&d2(&b.centroid)).unwrap_or(std::cmp::Ordering::Equal)).map(|f| f.id)
        };
        let known = |me: &Self, body: Id, id: u32| -> bool { id != 0 && me.regen_faces.get(&body).is_some_and(|fs| fs.iter().any(|f| f.id == id)) };
        // Holes are deliberately not handled here: their reference became a query, which needs no matching
        // — it either finds the face by recipe or refuses with a named reason.
        //
        // Sketches placed on a face of a body:
        let mut sfixes: Vec<(Id, Id, u32, String)> = Vec::new();
        for s in &self.sketches {
            if let SketchPlane::Face(body, ref key) = s.plane {
                if !known(self, body, key.id) {
                    if let Some(fs) = self.regen_faces.get(&body) {
                        if let Some(newid) = candidate(fs, key) {
                            sfixes.push((s.id, body, newid, format!("rebind-sketch-face#{}: {} -> {newid}", s.name, key.id)));
                        }
                    }
                }
            }
        }
        for (sid, body, newid, what) in sfixes {
            if let Some(s) = self.sketches.iter_mut().find(|s| s.id == sid) {
                if let SketchPlane::Face(b, key) = &mut s.plane {
                    let _ = b;
                    key.id = newid;
                }
            }
            report.rebinds.push(Rebind { node: sid, body, what });
        }
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

    /// Resolve a face reference: by name first, then by a recorded merge, and only then by snapshot.
    ///
    /// The rules match those for edges, for the same reason: a name is an address and a positional number
    /// is not. A number is trusted only while the snapshot agrees with it — as soon as some faces gain
    /// names, the numbering of the rest shifts and a number stored in the file starts pointing at a
    /// different face, silently. The snapshot fallback is used only on an unambiguous match.
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
        if let Some(t) = self.names.absorbed_target(stored) {
            if live(t).is_some() {
                return Some(t);
            }
        }
        // The only face of the body: there is nothing to confuse it with, so this is proof rather than
        // similarity.
        if fs.len() == 1 {
            return Some(fs[0].id);
        }
        let (c, n) = snap?;
        let tol = self.face_snap_tol(fs);
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
        let hit = best.0 < tol && (second - best.0) > tol * 0.6;
        if hit {
            self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
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
    /// A valid id passes through as is. A stale one (absent from `regen_edges[src]` because the topology
    /// changed) is repaired from the snapshot: the nearest edge by midpoint and direction, but only on an
    /// unambiguous match — the best candidate has to be clearly closer than the second and within
    /// tolerance. Otherwise the edge drops out. Returns the current ids for the kernel.
    pub fn resolve_edge_ids(&self, fid: Id, src: Id, stored: &[u32]) -> Vec<u32> {
        self.resolve_edge_ids_in(self.regen_edges.get(&src).map(|v| v.as_slice()).unwrap_or(&[]), fid, stored)
    }

    /// Like `resolve_edge_ids`, but against an explicit list of current edges: regenerate has fresh ones
    /// from the kernel, while `regen_edges` may be empty or stale in the middle of a pass.
    ///
    /// The repair metric is the distance from the old midpoint to the candidate segment, not midpoint to
    /// midpoint. A merged collinear edge (produced by seam stitching) contains the old midpoint, giving an
    /// unambiguous match, whereas midpoint-to-midpoint drifts by half the merge length and the edge drops
    /// out.
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
        // The unambiguity requirement stays and grows with the tolerance: a fillet on the wrong edge is
        // worse than a lost one, so the second candidate has to be clearly worse than the first.
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for e in es {
            for k in 0..3 {
                lo[k] = lo[k].min(e.mid[k]);
                hi[k] = hi[k].max(e.mid[k]);
            }
        }
        let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
        let tol = (diag * 0.02).max(0.5);
        let gap = (tol * 0.6).max(0.3);
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
            if let Some(t) = self.names.absorbed_target(eid) {
                if es.iter().any(|e| e.id == t) {
                    if !out.contains(&t) {
                        out.push(t);
                    }
                    continue;
                }
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
            if let Some(&ElemSnapshot { at: mid, dir, .. }) = snaps.and_then(|v| v.iter().find(|s| s.id == eid)) {
                let seg_dist = |e: &crate::geom::MeshEdge| -> f64 {
                    let (a, b) = (e.a, e.b);
                    let v = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
                    if l2 < 1e-18 {
                        return ((e.mid[0] - mid[0]).powi(2) + (e.mid[1] - mid[1]).powi(2) + (e.mid[2] - mid[2]).powi(2)).sqrt();
                    }
                    let t = (((mid[0] - a[0]) * v[0] + (mid[1] - a[1]) * v[1] + (mid[2] - a[2]) * v[2]) / l2).clamp(0.0, 1.0);
                    let p = [a[0] + v[0] * t, a[1] + v[1] * t, a[2] + v[2] * t];
                    ((p[0] - mid[0]).powi(2) + (p[1] - mid[1]).powi(2) + (p[2] - mid[2]).powi(2)).sqrt()
                };
                let mut scored: Vec<(f64, u32)> = es
                    .iter()
                    .map(|e| {
                        let dot = (e.dir[0] * dir[0] + e.dir[1] * dir[1] + e.dir[2] * dir[2]).abs().min(1.0);
                        (seg_dist(e) + (1.0 - dot) * 10.0, e.id)
                    })
                    .collect();
                scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(&(best, bid)) = scored.first() {
                    let second = scored.get(1).map(|x| x.0).unwrap_or(f64::MAX);
                    if best < tol && (second - best) > gap && !out.contains(&bid) {
                        self.snap_rebinds.fetch_add(1, std::sync::atomic::Ordering::Relaxed); // The fallback fired.
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
