//! A REBUILD APPLIED: edges kept, references rewritten, what could not be built cascaded.

use super::*;

impl Project {
    pub(crate) fn live_fillet_edges(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
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
        // A reference resolves to edges of the body as it stands: a number that did not resolve is counted lost
        // (`edge_refs_lost`) and said, not handed on. That the kernel never meets an edge a body does not have is the
        // bridge's to hold (`ops::blend::sharp`), whatever asks it.
        let known: std::collections::HashSet<u32> = cur.into_iter().map(|e| e.id).collect();
        if known.is_empty() {
            return live;
        }
        live.into_iter().filter(|e| known.contains(e)).collect()
    }

    /// HOW MANY OF THE PICKED EDGES FIND NOTHING, each asked on its own: an edge merged with its neighbour finds the
    /// merged one and is not lost; one that is gone finds nothing. The references are only read, not rewritten.
    pub(crate) fn edge_refs_lost(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> usize {
        let cur = kernel.edges(src);
        let translated = self.translate_edge_refs(node_id, src, edges, emap, &cur);
        let keep = self.snap_rebinds.load(std::sync::atomic::Ordering::Relaxed);
        let lost = translated.iter().filter(|&&d| if cur.is_empty() { self.resolve_edge_ids(node_id, src, &[d]).is_empty() } else { self.resolve_edge_ids_in(&cur, node_id, &[d]).is_empty() }).count();
        self.snap_rebinds.store(keep, std::sync::atomic::Ordering::Relaxed);
        lost
    }

    pub(crate) fn live_edge_refs(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, kernel: &dyn crate::feature::Kernel) -> Vec<u32> {
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
    pub(crate) fn rewrite_edge_picks(&mut self, node_id: Id, found: &[u32]) {
        let Some(n) = self.timeline.iter_mut().find(|n| n.id == node_id) else { return };
        match &mut n.kind {
            crate::feature::FeatureKind::Fillet { edges, .. } | crate::feature::FeatureKind::Chamfer { edges, .. } if !edges.query.picked_descs().is_empty() => {
                *edges = crate::refs::Ref::picks(found);
            }
            _ => {}
        }
    }

    /// A single face through the same witness: like `faces_by_ref`, but for references to one face (a face
    /// offset, a hole axis, the neutral face of a draft).
    pub(crate) fn face_by_ref(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, what: &str) -> Result<crate::refs::Candidate, crate::refs::RefError> {
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

    /// Face references resolved through a witness: by name first, and by a place snapshot when the name misses.
    ///
    /// Eight timeline features hold face references, and matching the descriptor is a single path that breaks on
    /// every improvement to naming while some faces are still positional — a thicken went red that way twice in
    /// a real project. The order matches the one used for edges: name, then a recorded merge, then the single
    /// face of a body, then the snapshot (see `resolve_face_id`).
    ///
    /// The snapshot is refreshed on every successful rebuild: a witness has to speak about today's geometry, or
    /// it becomes a source of misses itself.
    pub(crate) fn faces_by_ref(&mut self, node_id: Id, src: Id, r: &crate::refs::Ref, what: &str) -> Result<Vec<u32>, crate::refs::RefError> {
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

    /// Cascade of the unbuilt: the body of a node will not appear, and its consumers are not built either.
    ///
    /// This is how a suppressed base feature behaves (there is nothing to skip, no source to copy) and any node
    /// whose input is already in the cascade. The body is removed from view and marked unbuilt: without the
    /// mark a consumer would try to build on a non-existent body and fail with an obscure error instead of an
    /// honest "the source was not built".
    pub(crate) fn cascade_unbuilt(&mut self, kind: &crate::feature::FeatureKind, unbuilt: &mut std::collections::HashSet<Id>) {
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
    pub(crate) fn isolation_error(&self, i: usize, kind: &crate::feature::FeatureKind) -> Option<crate::errors::CoreError> {
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
    pub(crate) fn tool_extent(&self, src: Id, pl: &[f64; 12], h: f64, down: f64, extent: crate::feature::Extent, op: u8) -> crate::feature::ToolSpan {
        let n = [pl[2], pl[6], pl[10]];
        let o = [pl[3], pl[7], pl[11]];
        if extent.through {
            // Span the whole of body `src` along the normal, both ways: the bounding box gives a start below it
            // and a length of the span plus margins.
            let kept = |start: f64, length: f64| crate::feature::ToolSpan { start, length, ends: crate::feature::ToolEnds::Kept };
            return match self.body_span_along(src, o, n) {
                Some((tmin, tmax)) => {
                    let margin = ((tmax - tmin).abs() * 0.1).max(1.0);
                    kept(tmin - margin, (tmax - tmin) + 2.0 * margin)
                }
                // Fallback: the mesh of `src` has no bounding box (the body is not tessellated yet, or the mesh
                // is empty). "Through all" has to stay a through cut rather than degenerate into a blind pocket
                // of nominal depth.
                None => {
                    let reach = (h.abs() * 100.0).max(1000.0);
                    kept(-reach, 2.0 * reach)
                }
            };
        }
        // The tool direction follows the extrude rules: one-sided, both ways or two-sided. A one-sided cut has its ends
        // met with the body by the bridge (`ToolEnds::OneSidedCut`): an end coincident with a face of the body leaves a
        // cap behind, which is OCCT's to be worked round, not the model's.
        let (start, length) = crate::feature::extrude_extent(h, down, extent.reach);
        let one_sided = op == 0 && extent.reach != crate::feature::Reach::BothWays && down.abs() <= 1e-9;
        crate::feature::ToolSpan { start, length, ends: if one_sided { crate::feature::ToolEnds::OneSidedCut } else { crate::feature::ToolEnds::Kept } }
    }

    pub(crate) fn apply_regen(&mut self, l: Landing, body: Id, res: Result<crate::geom::Built, crate::errors::CoreError>) -> bool {
        let Landing { node: node_id, dirty, report, kernel, emap } = l;
        match res {
            Ok(crate::geom::Built { mesh, faces }) => {
                self.set_body_mesh(body, mesh);
                dirty.insert(body);
                self.regen_faces.insert(body, faces); // Faces into the model, for resolving references by id.
                                                      // What the history was silent about is named by the bridge - the pieces, the seams, the places, the
                                                      // edges - by the table of the document, here on its thread and in the order of the tree.
                self.land_silent_names(node_id, body, kernel, emap);
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
                if let Some(crate::feature::DroppedEdges { asked, dropped }) = kernel.take_dropped_edges(body) {
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
