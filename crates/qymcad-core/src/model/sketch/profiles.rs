//! THE PROFILES A FEATURE TAKES of a sketch: regions, paths, sections, holes.

use super::*;

impl Project {
    pub fn sketch_index(&self, id: Id) -> Option<usize> {
        self.sketches.iter().position(|s| s.id == id)
    }
    /// The sketch a contour belongs to, by contour id.
    pub fn sketch_of_contour(&self, cid: Id) -> Option<Id> {
        self.sketches.iter().find(|s| s.contour_ids.contains(&cid)).map(|s| s.id)
    }
    /// Ids of contours that belong to no sketch (drawn or free-standing).
    pub fn loose_contour_ids(&self) -> Vec<Id> {
        self.contours.ids().iter().copied().filter(|cid| self.sketch_of_contour(*cid).is_none()).collect()
    }
    pub fn contour_id(&self, index: usize) -> Option<Id> {
        self.contours.id_at(index)
    }
    pub fn contour_index(&self, id: Id) -> Option<usize> {
        self.contours.index_of(id)
    }
    /// Frame of the plane sketch `si` is placed on: a world plane, a datum or a face, resolved into an
    /// orthonormal frame.
    pub fn sketch_frame(&self, si: usize) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{PlaneFrame, SketchPlane};
        // Shift of the origin within the plane (snapped to an edge or a vertex). It is applied to the base
        // frame, in its own axes, before any outer transform: u*X + v*Y is invariant to the placement of the
        // body.
        let uv = self.sketches.get(si)?.origin_uv;
        let shift = |mut f: PlaneFrame| -> PlaneFrame {
            if let Some(uv) = uv {
                let o = f.lift(uv);
                f.origin = [o.x, o.y, o.z];
            }
            f
        };
        match self.sketches.get(si)?.plane {
            SketchPlane::World(b) => Some(shift(b.frame())),
            // The sketch origin is the projection of the world origin onto the plane, so the sketch XY axes
            // coincide with the world ones and what is drawn in 2D lands in the same place in 3D.
            SketchPlane::Datum(id) => self.planes.iter().find(|p| p.id == id).map(|p| shift(PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg))),
            // A face is resolved by persistent id against the faces of the current rebuild of the body, so the
            // sketch keeps holding the same face and travels with it; the centroid and normal fingerprint is the
            // fallback match.
            SketchPlane::Face(body, key) => {
                let sid = self.sketches.get(si)?.id;
                // The face resolves in the local space of the source body (`regen_faces` is local), and the
                // frame is built in that space: the 2D zero is the projection of the source origin onto the face
                // plane, as for a world plane or a datum, and the axes are stable, with no mirroring.
                let (c, n) = self.resolve_face(body, &key);
                let frame = shift(PlaneFrame::world_aligned(c, n, 0.0));
                // Isolation: a sketch on a face of another component's body is allowed only with an explicit
                // external reference. The frame is then carried whole into the local space of the consumer
                // through `relative_transform`, so the sketch is anchored to the source face and follows it
                // along every axis rather than only along the normal, which is all the projection of the
                // consumer origin would give. Without the reference there is no frame, and regenerate blocks the
                // node.
                if let (Some(so), Some(bo)) = (self.sketch_owner(sid), self.body_owner(body)) {
                    if so != bo {
                        if !self.external_authorized(so, body) {
                            return None;
                        }
                        return Some(frame.transformed(&self.relative_transform(bo, so)));
                    }
                }
                Some(frame)
            }
        }
    }
    /// Isolated points of sketch `sid`: those that are neither endpoints of entities or splines nor system
    /// points (the origin and the axes). These are the marks placed for drilling. Returns their world
    /// coordinates, through the sketch frame, in sketch point order.
    pub fn sketch_isolated_points(&self, sid: Id) -> Vec<[f64; 3]> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let Some(frame) = self.sketch_frame(si) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        // Endpoints of the entities.
        used.extend(s.entities.iter().flat_map(entity_points));
        s.points
            .iter()
            .filter(|p| !used.contains(&p.id))
            .map(|p| {
                let w = frame.lift(Point2 { x: p.x, y: p.y });
                [w.x, w.y, w.z]
            })
            .collect()
    }
    /// Ids of the same isolated points, in the same order as `sketch_isolated_points`. The hole walls are named
    /// after them, so adding a point to the sketch must not rename the existing holes.
    pub fn sketch_hole_point_ids(&self, sid: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sid) else { return Vec::new() };
        let s = &self.sketches[si];
        let mut used: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
        used.extend(s.entities.iter().flat_map(entity_points));
        s.points.iter().filter(|p| !used.contains(&p.id)).map(|p| p.id).collect()
    }
    /// Profile of sketch `si` for an extrude or a revolve: its first closed contour, flattened to XY.
    pub fn sketch_profile_xy(&self, si: usize) -> Option<(Id, Vec<f64>)> {
        let s = self.sketches.get(si)?;
        for &cid in &s.contour_ids {
            if let Some(ci) = self.contour_index(cid) {
                let c = &self.contours[ci];
                if c.closed && c.points.len() >= 3 {
                    let xy: Vec<f64> = c.points.iter().flat_map(|p| [p.x, p.y]).collect();
                    return Some((cid, xy));
                }
            }
        }
        None
    }
    /// XY of a specific closed contour by id, used to choose which shape an extrude, cut or revolve takes.
    pub fn contour_profile_xy(&self, cid: Id) -> Option<Vec<f64>> {
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.closed && c.points.len() >= 3 {
            Some(c.points.iter().flat_map(|p| [p.x, p.y]).collect())
        } else {
            None
        }
    }
    /// Planar regions of a sketch: the minimal closed faces of the arrangement of every entity (intersections,
    /// then splitting, then faces). This makes areas selectable that are not whole loops — a strip between two
    /// circles cut by a line, for instance. Used to pick an extrude profile.
    pub fn sketch_regions(&self, si: usize) -> Vec<crate::geom::Contour> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        arrangement_regions(&s.points, &s.entities)
    }
    /// Like `feature_profile_encoded`, but the contours listed in `fill` are not subtracted as holes, having
    /// been explicitly selected as filled. Two concentric circles with both selected put the inner one in
    /// `fill` and give a solid cylinder rather than a tube.
    ///
    /// Note that this is one face: an outer contour plus its holes. Nesting deeper than two levels also
    /// produces islands, which only [`Project::profile_faces`] and [`Project::encode_profiles_fill`] return.
    pub fn feature_profile_encoded_fill(&self, sketch: Id, profile: Id, fill: &[Id]) -> Option<Vec<f64>> {
        let (outer_cid, hole_ids) = self.profile_faces(sketch, &[profile], fill).into_iter().next()?;
        let outer = self.contours.get(self.contour_index(outer_cid)?)?;
        let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
        Some(crate::geom::encode_profile(outer, &holes))
    }
    /// THE SOURCE OF EVERY PIECE OF EVERY SKETCH ENTITY in the contours `loops` of the sketch `sketch`: the entity, and
    /// which piece of it - each at its place in a list (counted from 1).
    ///
    /// A source per entity is not enough. An entity cut by its neighbours gives several pieces, and regions that meet
    /// only at points keep them all as faces of one body: measured on a rectangle with a slanted bar over its corner,
    /// the band where the two overlap left out, 18 faces under 12 names and 36 edges under 15, so the cursor on one
    /// edge lit another. A piece is told by what cut it off: the entity crossing it at its start along the entity
    /// (`geom::piece_start`) - the piece holding the start of the entity, the usual case of one piece, keeps the name it
    /// always had - and numbered among the pieces starting at a crossing of the same entity. Numbered by their place
    /// along the entity, the pieces after a crossing taken away took the numbers before them: on the vacuum cleaner an
    /// arc that did not move went from the first piece of its circle to the second, and its wall was renamed.
    pub(crate) fn piece_sources(&self, sketch: Id, loops: &[Id]) -> PieceSources {
        let si = self.sketch_index(sketch);
        let at = |pid: Id| si.and_then(|si| self.sketches[si].points.iter().find(|q| q.id == pid)).map(|q| crate::geom::Point2::new(q.x, q.y));
        let ends_of = |src: Id| -> Vec<crate::geom::Point2> {
            // the two ends of the curve only: a node of a spline or the end of an axis of an ellipse lies on it inside
            si.and_then(|si| self.sketches[si].entities.iter().find(|e| e.id == src)).and_then(crate::model::entity_ends).map(|ends| ends.into_iter().filter_map(at).collect()).unwrap_or_default()
        };
        let near = |p: crate::geom::Point2, q: crate::geom::Point2| (p.x - q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6;
        // the entities meeting at every end of every piece of the sketch, over all of its regions: what crosses a piece is
        // there whether or not the region beyond the crossing is in the profile
        let spot = |q: crate::geom::Point2| [(q.x * 1e6).round() as i64, (q.y * 1e6).round() as i64];
        let mut meeting: std::collections::HashMap<[i64; 2], std::collections::BTreeMap<Id, std::collections::BTreeSet<[i64; 2]>>> = std::collections::HashMap::new();
        let regions: Vec<&crate::geom::Contour> = si.map(|si| self.sketches[si].contour_ids.iter().filter_map(|c| self.contour_index(*c).map(|i| &self.contours[i])).collect()).unwrap_or_default();
        for region in regions {
            for (src, e) in region.sourced_edges() {
                let key = crate::geom::piece_of(&e).0;
                for q in crate::geom::piece_ends(&e).into_iter().flatten() {
                    meeting.entry(spot(q)).or_default().entry(src).or_default().insert(key);
                }
            }
        }
        let mut pieces: std::collections::BTreeMap<Id, Vec<PieceAt>> = std::collections::BTreeMap::new();
        for cid in loops {
            let Some(i) = self.contour_index(*cid) else { continue };
            for (src, e) in self.contours[i].sourced_edges() {
                if src == 0 {
                    continue;
                }
                let (key, along) = crate::geom::piece_of(&e);
                // what cuts the piece off at its start, the smallest by its id: inside its entity, any other entity meeting it
                // there; at an end of its entity, only one running on through that point (pieces on both sides of it) - an
                // entity only ending there is the corner the two meet at, and the piece keeps the name it always had
                let cut = crate::geom::piece_start(&e)
                    .and_then(|start| {
                        let at_its_end = ends_of(src).into_iter().any(|q| near(q, start));
                        meeting.get(&spot(start))?.iter().find(|(other, pieces)| **other != src && **other != 0 && (!at_its_end || pieces.len() >= 2)).map(|(other, _)| *other)
                    })
                    .unwrap_or(0);
                let of = pieces.entry(src).or_default();
                if !of.iter().any(|p| p.key == key) {
                    of.push(PieceAt { key, along, cut });
                }
            }
        }
        let mut out = PieceSources::default();
        for (src, mut of) in pieces {
            of.sort_by(|a, b| a.along.total_cmp(&b.along).then(a.key.cmp(&b.key)));
            let mut counted: std::collections::HashMap<Id, u16> = std::collections::HashMap::new();
            for p in of {
                let split = counted.entry(p.cut).or_insert(0);
                out.sources.push(crate::feature::SideSource { src, split: *split, cut: p.cut });
                *split = split.saturating_add(1);
                out.slot.insert(PieceKey { src, key: p.key }, out.sources.len() as u32);
            }
        }
        out
    }
    /// Resolve a selection into faces: `(outer contour, its holes)`, correct at any depth of nesting.
    ///
    /// The model is simple and predictable: every selected contour is its own region, holding the material
    /// between it and its direct children, which become its holes. The regions are then fused in the kernel, so
    /// selecting adjacent nesting levels composes by itself — selecting the outer and the middle of three
    /// nested rectangles gives `(outer, [middle])` together with `(middle, [inner])`, that is, a plate with a
    /// hole the size of the inner rectangle. This works for any number of nesting levels.
    ///
    /// Marking a nested selected contour as filled on its parent instead, without giving it a region of its
    /// own, ends the descent there and loses everything deeper: three nested rectangles extrude solid. `fill` is
    /// therefore read simply as one more selected contour, which repairs older files without editing the
    /// feature.
    ///
    /// An empty `profiles`, or a zero, means the first closed contour of the sketch.
    pub fn profile_faces(&self, sketch: Id, profiles: &[Id], fill: &[Id]) -> Vec<(Id, Vec<Id>)> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Closed contours of the sketch usable as a profile.
        let ids: Vec<Id> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| self.contour_index(*cid).is_some_and(|i| self.contours[i].closed && self.contours[i].points.len() >= 3) && self.contour_profile_xy(*cid).is_some())
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        // The selection is `profiles` plus `fill`; the older "filled" mark reads as one more selected
        // contour.
        let mut sel: Vec<Id> = profiles.iter().chain(fill).copied().map(|p| if p == 0 { ids[0] } else { p }).filter(|p| ids.contains(p)).collect();
        if sel.is_empty() {
            sel.push(ids[0]);
        }
        sel.sort_unstable();
        sel.dedup();
        sel.into_iter().map(|c| (c, self.feature_holes(sketch, c))).collect()
    }
    /// THE PROFILES OF AN OPERATION WITH THE SOURCES OF THEIR SIDES: each region encoded, the slot of the name of every
    /// edge holding the place of its source (`crate::feature::SourcedProfiles`). The contours in `fill` are filled, not
    /// holes. The pieces are counted over every region at once: the pieces of one entity in two regions are two faces of
    /// the one body. What the sides are called from that is the bridge's rule.
    pub fn encode_profiles_sourced(&self, sketch: Id, profiles: &[Id], fill: &[Id]) -> Option<crate::feature::SourcedProfiles> {
        let faces = self.profile_faces(sketch, profiles, fill);
        if faces.is_empty() {
            return None;
        }
        let loops: Vec<Id> = faces.iter().flat_map(|(outer, holes)| std::iter::once(*outer).chain(holes.iter().copied())).collect();
        let regions: Vec<Id> = faces.iter().map(|(outer, _)| *outer).collect();
        let pieces = self.piece_sources(sketch, &loops);
        let slot_of = |src: Id, e: &crate::geom::ProfEdge| -> u32 { pieces.slot.get(&PieceKey { src, key: crate::geom::piece_of(e).0 }).copied().unwrap_or(0) };
        let encoded: Option<Vec<Vec<f64>>> = faces
            .into_iter()
            .map(|(outer_cid, hole_ids)| {
                let outer = self.contours.get(self.contour_index(outer_cid)?)?;
                let holes: Vec<&crate::geom::Contour> = hole_ids.iter().filter_map(|h| self.contour_index(*h).map(|i| &self.contours[i])).collect();
                Some(crate::geom::encode_profile_named(outer, &holes, &slot_of))
            })
            .collect();
        Some(crate::feature::SourcedProfiles { profiles: encoded?, sources: pieces.sources, regions })
    }
    /// Encode the path of a sweep: one sketch contour, open or closed, as `[1.0, loop_block]` with exact lines
    /// and arcs (`build_exact_wire`). A `path` of zero takes the first contour with at least two points,
    /// preferring an open one, which is the typical path. Sketch contours always carry exact edges.
    pub fn sweep_path_encoded(&self, sketch: Id, path: Id) -> Option<Vec<f64>> {
        let has_pts = |cid: &&Id| self.contour_index(**cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let cid = if path != 0 {
            path
        } else {
            let si = self.sketch_index(sketch)?;
            let ids = &self.sketches[si].contour_ids;
            let open = |cid: &&Id| self.contour_index(**cid).map(|i| !self.contours[i].closed).unwrap_or(false) && has_pts(cid);
            *ids.iter().find(open).or_else(|| ids.iter().find(has_pts))?
        };
        let c = self.contours.get(self.contour_index(cid)?)?;
        if c.points.len() < 2 {
            return None;
        }
        let mut v = vec![1.0];
        v.extend(c.loop_block());
        Some(v)
    }
    /// Candidate profile contours for a sweep (closed, at least three points), in sketch order. The first is
    /// the automatic choice for a `profile` of zero. Used to switch contours in the interface and to preview
    /// them; it matches `feature_profile_encoded`.
    pub fn sweep_profile_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| self.contour_index(*cid).map(|i| self.contours[i].closed && self.contours[i].points.len() >= 3).unwrap_or(false) && self.contour_profile_xy(*cid).is_some())
            .collect()
    }
    /// Candidate path contours (at least two points), open ones first as the typical path, then closed ones.
    /// The first is the automatic choice for a `path` of zero, matching the order used by
    /// `sweep_path_encoded`.
    pub fn sweep_path_contours(&self, sketch: Id) -> Vec<Id> {
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        let has_pts = |cid: Id| self.contour_index(cid).map(|i| self.contours[i].points.len() >= 2).unwrap_or(false);
        let is_open = |cid: Id| self.contour_index(cid).map(|i| !self.contours[i].closed).unwrap_or(false);
        let ids: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|c| has_pts(*c)).collect();
        let mut open: Vec<Id> = ids.iter().copied().filter(|c| is_open(*c)).collect();
        let closed: Vec<Id> = ids.iter().copied().filter(|c| !is_open(*c)).collect();
        open.extend(closed);
        open
    }
    /// Loft contour for section sketch `sketch`, honouring the selection `cid` (zero takes the first closed
    /// profile). Returns the id of a contour usable as a section: closed, with at least three points.
    pub fn loft_section_contour(&self, sketch: Id, cid: Id) -> Option<Id> {
        if cid != 0 {
            let ci = self.contour_index(cid)?;
            return (self.contours[ci].closed && self.contours[ci].points.len() >= 3).then_some(cid);
        }
        self.sweep_profile_contours(sketch).first().copied()
    }
    /// Like `loft_encoded`, but the edges of the first section carry the sources of the future loft faces: a face
    /// between sections is named after the edge that swept it, there being no other source in the recipe.
    pub fn loft_encoded_sourced(&self, sketches: &[Id], contours: &[Id]) -> Option<LoftEncoded> {
        if sketches.len() < 2 {
            return None;
        }
        let first_cid = self.loft_section_contour(sketches[0], contours.first().copied().unwrap_or(0))?;
        let mut srcs: Vec<Id> = self.contour_index(first_cid).map(|i| self.contours[i].edge_src.iter().copied().filter(|s| *s != 0).collect()).unwrap_or_default();
        srcs.sort_unstable();
        srcs.dedup();
        let sources: Vec<crate::feature::SideSource> = srcs.iter().map(|src| crate::feature::SideSource { src: *src, split: 0, cut: 0 }).collect();
        let slots: std::collections::HashMap<Id, u32> = srcs.into_iter().enumerate().map(|(k, src)| (src, k as u32 + 1)).collect();
        let sections = self.loft_encoded_with(sketches, contours, &slots)?;
        Some(LoftEncoded { sections, sources })
    }
    pub(crate) fn loft_encoded_with(&self, sketches: &[Id], contours: &[Id], names: &std::collections::HashMap<Id, u32>) -> Option<LoftSectionsEncoded> {
        if sketches.len() < 2 {
            return None;
        }
        let mut data = Vec::new();
        let mut offsets = vec![0usize];
        let mut places = Vec::new();
        for (i, &sid) in sketches.iter().enumerate() {
            let cid = self.loft_section_contour(sid, contours.get(i).copied().unwrap_or(0))?;
            let ci = self.contour_index(cid)?;
            let c = &self.contours[ci];
            if !c.closed || c.points.len() < 3 {
                return None;
            }
            if i == 0 {
                data.extend(c.loop_block_named(&|src, _| names.get(&src).copied().unwrap_or(0)));
            } else {
                data.extend(c.loop_block());
            }
            offsets.push(data.len());
            let pl = self.sketch_frame_by_id(sid).map(|f| f.matrix12()).unwrap_or(crate::feature::PLACE_IDENTITY);
            places.extend_from_slice(&pl);
        }
        Some(LoftSectionsEncoded { data, offsets, places })
    }
    /// Points of a contour, by id.
    pub(crate) fn contour_pts(&self, cid: Id) -> Vec<Point2> {
        self.contour_index(cid).map(|ci| self.contours[ci].points.clone()).unwrap_or_default()
    }
    /// Recompute the nesting of the sketch contours and record it in `contour_parent`. Computed once when a
    /// sketch is regenerated rather than by every consumer on every build.
    ///
    /// The parent is the smallest strictly enclosing contour. "Strictly" is the key word: a contour that touches
    /// its enclosing one is not a child — it is part of the boundary rather than a hole. Measured failure
    /// without it: a pad flush against a wall was counted as a hole, the hole touched the outer loop, the face
    /// failed to build, and the whole region silently disappeared from the body.
    pub(crate) fn rebuild_contour_nesting(&mut self, cids: &[Id]) {
        let data: Vec<(Id, Vec<Point2>, f64)> = cids
            .iter()
            .filter_map(|&cid| {
                let c = self.contours.get(self.contour_index(cid)?)?;
                (c.closed && c.points.len() >= 3).then(|| (cid, c.points.clone(), c.signed_area().abs()))
            })
            .collect();
        for &cid in cids {
            self.contours.clear_parent(cid);
        }
        // A contour holds another only if every point of the other is inside it, so only one whose box meets the box of
        // the other is tried (`meeting_boxes`), in the order of the list: 10 000 rectangles were 1e8 tries of a polygon.
        let boxes: Vec<[f64; 4]> =
            data.iter().map(|(_, pts, _)| pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)])).collect();
        let mut meets: Vec<Vec<usize>> = vec![Vec::new(); data.len()];
        for super::tess::BoxPair { first, second } in super::tess::meeting_boxes(&boxes) {
            meets[first].push(second);
            meets[second].push(first);
        }
        for (k, (cid, pts, _)) in data.iter().enumerate() {
            meets[k].sort_unstable();
            let parent = meets[k]
                .iter()
                .map(|&o| &data[o])
                .filter(|(oid, opts, _)| oid != cid && poly_contains(opts, pts) && !polys_touch(opts, pts))
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(oid, _, _)| *oid);
            // A root is marked explicitly (parent 0) rather than by a missing entry: otherwise "the graph is
            // built" and "there is no nesting" are indistinguishable, and a sketch without nesting would be
            // recomputed on every build.
            self.contours.set_parent(*cid, parent.unwrap_or(0));
        }
    }
    pub fn feature_holes(&self, sketch: Id, outer: Id) -> Vec<Id> {
        if outer == 0 {
            return Vec::new();
        }
        let Some(si) = self.sketch_index(sketch) else { return Vec::new() };
        // Nesting is a stored relation (`contour_parent`), computed once when the sketch is regenerated. The
        // direct children are the contours whose parent is this one — no point-in-polygon recomputation on
        // every build and no per-consumer tolerances.
        let by_graph: Vec<Id> = self.sketches[si].contour_ids.iter().copied().filter(|cid| self.contours.parent_of(*cid).as_ref() == Some(&outer)).collect();
        if !by_graph.is_empty() || self.sketches[si].contour_ids.iter().any(|c| self.contours.parent_of(*c).is_some()) {
            return by_graph;
        }
        // The graph is not built yet (the sketch was not regenerated in this session): it is computed here, and
        // nothing beyond it is guessed.
        let outer_pts = self.contour_pts(outer);
        if outer_pts.len() < 3 {
            return Vec::new();
        }
        let cands: Vec<(Id, Vec<Point2>)> = self.sketches[si]
            .contour_ids
            .iter()
            .copied()
            .filter(|cid| *cid != outer && self.contour_profile_xy(*cid).is_some())
            .map(|cid| (cid, self.contour_pts(cid)))
            .filter(|(_, p)| p.len() >= 3 && poly_contains(&outer_pts, p) && !polys_touch(&outer_pts, p))
            .collect();
        // A direct child of `outer` is one that lies inside no other candidate.
        cands.iter().filter(|(id, p)| !cands.iter().any(|(id2, p2)| id2 != id && poly_contains(p2, p))).map(|(id, _)| *id).collect()
    }
    /// Local revolve axis (origin and direction in sketch 2D space, z = 0) from centreline `line`. A `line` of
    /// zero, or one that is not a line or is degenerate, gives `None` and the caller falls back to a datum or the
    /// sketch X or Y axis. `dir` is normalised. The 2D coordinates of the line are already in sketch plane space,
    /// so no inverse placement is needed, unlike a datum axis given in world space.
    pub(crate) fn revolve_axis_from_line(&self, sketch: Id, line: Id) -> Option<([f64; 3], [f64; 3])> {
        if line == 0 {
            return None;
        }
        let si = self.sketch_index(sketch)?;
        let (a, b) = self.line_ends(si, line)?;
        let s = &self.sketches[si];
        let pa = s.points.iter().find(|q| q.id == a).map(|q| (q.x, q.y))?;
        let pb = s.points.iter().find(|q| q.id == b).map(|q| (q.x, q.y))?;
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt();
        (len > 1e-9).then(|| ([pa.0, pa.1, 0.0], [dx / len, dy / len, 0.0]))
    }
}

#[cfg(test)]
mod tests {
    use super::{PieceKey, PieceSources};
    use crate::feature::{Purpose, SideSource};
    use crate::model::{Id, Project};

    /// The pieces of every closed contour of the sketch `si`.
    fn pieces(p: &Project, si: usize) -> PieceSources {
        let loops: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
        p.piece_sources(p.sketches[si].id, &loops)
    }

    /// The source of the piece of the entity `src` whose middle is at `mid`.
    fn source_at(pieces: &PieceSources, src: Id, mid: [f64; 2]) -> Option<SideSource> {
        let key = mid.map(|v| (v * 1e6).round() as i64);
        pieces.slot.get(&PieceKey { src, key }).map(|k| pieces.sources[*k as usize - 1])
    }

    /// A PIECE OF A SKETCH LINE KEEPS ITS SOURCE WHEN A PIECE BEFORE IT GOES. The pieces of an entity were numbered by
    /// their place along it, and a line crossing it taken away renumbered the pieces after the crossing. Measured on the
    /// vacuum cleaner: an arc of a circle that did not move went from the first to the second piece when an edit moved
    /// another crossing over the start of the circle, and the wall swept from it was renamed. The bottom of a rectangle
    /// 100 x 20 crossed at x = 10 and x = 30: the crossing at 10 moved off the rectangle, the piece from 30 to 100 keeps
    /// its source.
    #[test]
    fn a_piece_of_a_line_keeps_its_source_when_a_crossing_before_it_goes() {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("pieces");
        p.add_rect_entity(si, 0.0, 0.0, 100.0, 20.0, Purpose::Real);
        p.add_line_entity(si, 30.0, -5.0, 30.0, 25.0, Purpose::Real);
        let moving = p.add_line_entity(si, 10.0, -5.0, 10.0, 25.0, Purpose::Real);
        p.regen_sketch(si);
        let y_of = |p: &Project, pid: Id| p.sketches[si].points.iter().find(|q| q.id == pid).map_or(f64::NAN, |q| q.y);
        let bottom = p.sketches[si].entities.iter().find(|e| e.kind.as_line().is_some_and(|l| y_of(&p, l.a) == 0.0 && y_of(&p, l.b) == 0.0)).map(|e| e.id).expect("the bottom of the rectangle");
        let before = source_at(&pieces(&p, si), bottom, [65.0, 0.0]).expect("setup: the piece from 30 to 100");
        let ends: Vec<Id> = p.sketches[si].entities.iter().find(|e| e.id == moving).map(crate::model::entity_points).unwrap_or_default();
        for q in p.sketches[si].points.iter_mut().filter(|q| ends.contains(&q.id)) {
            q.x = -5.0;
        }
        p.regen_sketch(si);
        let after = source_at(&pieces(&p, si), bottom, [65.0, 0.0]).expect("the piece from 30 to 100 is gone");
        assert_eq!(after, before, "the piece from 30 to 100 did not move and its source changed");
    }
    /// The source of the piece of the entity `src` whose middle is nearest to `near`.
    fn source_near(p: &Project, si: usize, src: Id, near: [f64; 2]) -> Option<SideSource> {
        let all = pieces(p, si);
        let keys = all.slot.keys().filter(|k| k.src == src).map(|k| k.key);
        let dist = |key: [i64; 2]| (key[0] as f64 * 1e-6 - near[0]).hypot(key[1] as f64 * 1e-6 - near[1]);
        let key = keys.min_by(|a, b| dist(*a).total_cmp(&dist(*b)))?;
        source_at(&all, src, key.map(|v| v as f64 * 1e-6))
    }

    /// The line `line` of sketch `si` moved off to the left, out of everything.
    fn move_off(p: &mut Project, si: usize, line: Id) {
        let ends: Vec<Id> = p.sketches[si].entities.iter().find(|e| e.id == line).map(crate::model::entity_points).unwrap_or_default();
        for q in p.sketches[si].points.iter_mut().filter(|q| ends.contains(&q.id)) {
            q.x = -50.0;
        }
        p.regen_sketch(si);
    }

    /// A PIECE OF AN ELLIPSE OR OF A SPLINE KEEPS ITS SOURCE WHEN A PIECE BEFORE IT GOES, by the rule of the pieces of a
    /// line: named after what cut it off at its start. The pieces of an ellipse and of a spline were numbered by their
    /// place along it, and a line crossing it taken away renumbered the pieces after the crossing.
    ///
    /// The upper half of an ellipse about (50, 10) of semi-axes 40 x 8, closed by its diameter, crossed by upright lines at
    /// x = 30 and x = 70, and at x = 50.3 by a line moved off: its piece beyond x = 30 keeps its source. A whole ellipse is a
    /// region of its own, not cut by the lines crossing it (`curve::arrange`), and its one piece holds its start. A spline over (0, 0), (50, 20), (100, 0) closed by
    /// the line under it, crossed at x = 30 and at x = 10 by a line moved off: its piece from x = 30 to its node at
    /// x = 50 keeps its source.
    #[test]
    fn a_piece_of_an_ellipse_or_a_spline_keeps_its_source_when_a_crossing_before_it_goes() {
        use crate::geom::Point2;
        let mut failures = Vec::new();
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("ellipse");
        let arc = crate::model::EllipseArcSpec { centre: Point2::new(50.0, 10.0), radii: Point2::new(40.0, 8.0), rot: 0.0, angles: [0.0, std::f64::consts::PI], winding: crate::feature::Winding::Ccw };
        let ellipse = p.add_ellipse_arc_entity(si, arc, Purpose::Real);
        p.add_line_entity(si, 90.0, 10.0, 10.0, 10.0, Purpose::Real);
        p.add_line_entity(si, 30.0, 5.0, 30.0, 25.0, Purpose::Real);
        p.add_line_entity(si, 70.0, 5.0, 70.0, 25.0, Purpose::Real);
        let moving = p.add_line_entity(si, 50.3, 5.0, 50.3, 25.0, Purpose::Real);
        p.merge_close_points(si, 1e-9);
        p.regen_sketch(si);
        let before = source_near(&p, si, ellipse, [15.4, 14.0]);
        move_off(&mut p, si, moving);
        let after = source_near(&p, si, ellipse, [15.4, 14.0]);
        if before.is_none() || after != before {
            failures.push(format!("the piece of the arc of the ellipse beyond x = 30: {before:?} -> {after:?}"));
        }
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("spline");
        let nodes = [[0.0, 0.0], [50.0, 20.0], [100.0, 0.0]].map(|q| Point2::new(q[0], q[1])).to_vec();
        let spline = p.add_spline(si, nodes, crate::feature::Ends::Open, Purpose::Real).expect("the spline");
        p.add_line_entity(si, 0.0, 0.0, 100.0, 0.0, Purpose::Real);
        p.add_line_entity(si, 30.0, -5.0, 30.0, 25.0, Purpose::Real);
        let moving = p.add_line_entity(si, 10.0, -5.0, 10.0, 25.0, Purpose::Real);
        p.merge_close_points(si, 1e-9);
        p.regen_sketch(si);
        // the piece from the crossing at x = 30 to the end of the spline, one edge over its node
        let before = source_near(&p, si, spline, [65.0, 18.0]);
        move_off(&mut p, si, moving);
        let after = source_near(&p, si, spline, [65.0, 18.0]);
        if before.is_none() || after != before {
            failures.push(format!("the piece of the spline from x = 30 to its end: {before:?} -> {after:?}"));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// A LINE ENDING INSIDE A SPLINE CUTS IT THERE, at a node of the spline as between its nodes: only the two ends of a
    /// curve are its ends. Every node of a spline was read as an end of it, and a line ending on a node cut nothing - the
    /// piece after it was named as one of the pieces cut by nothing, apart from the same line ending a little beside the
    /// node. A spline over (0, 0), (50, 20), (100, 0) closed by the line under it, and a line from (50, 0) up to its node:
    /// the piece of the spline from the node on is cut off by that line.
    #[test]
    fn a_line_ending_on_a_node_of_a_spline_cuts_it_there() {
        use crate::geom::Point2;
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("spline");
        let nodes = [[0.0, 0.0], [50.0, 20.0], [100.0, 0.0]].map(|q| Point2::new(q[0], q[1])).to_vec();
        let spline = p.add_spline(si, nodes, crate::feature::Ends::Open, Purpose::Real).expect("the spline");
        p.add_line_entity(si, 0.0, 0.0, 100.0, 0.0, Purpose::Real);
        let up = p.add_line_entity(si, 50.0, 0.0, 50.0, 20.0, Purpose::Real);
        p.merge_close_points(si, 1e-9);
        p.regen_sketch(si);
        let source = source_near(&p, si, spline, [75.0, 15.0]).expect("the piece of the spline after its node");
        assert_eq!(source.cut, up, "the piece of the spline after the line ending on its node: {source:?}");
    }
}
