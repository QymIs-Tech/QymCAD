//! THE OPERATIONS ON A SHAPE: shells, removed and pushed faces, splits, drafts, holes, thickening, the names of faces and edges.

use super::*;

impl Shape {
    pub(crate) fn shell(&self, offset: f64, face_ids: &[u32], walls: &[ImageOf]) -> Option<Shape> {
        let gf: Vec<u32> = walls.iter().map(|w| w.source).collect();
        let gt: Vec<u32> = walls.iter().map(|w| w.name).collect();
        // No validity check here. It stands further on, in the shared `finish` funnel, where an unsound shape
        // is first repaired and only then called broken. Dropping the shape right here turned a clear fault
        // into an unnamed "the operation failed".
        unsafe { Self::wrap(qym_shape_shell(self.ptr, offset, face_ids.as_ptr(), face_ids.len(), gf.as_ptr(), gt.as_ptr(), gf.len())) }
    }
    /// A draft: tilt the faces `face_ids` by `angle_deg` relative to the neutral plane given by `np_origin`
    /// and `np_normal`, whose line of intersection with a face stays put, in the pull direction `pull`, which
    /// is usually the normal of the neutral plane. The faces keep their persistent ids.
    /// Remove faces and heal: the faces go, the neighbours are extended and the body stays closed. This is
    /// how a fillet, a chamfer or a boss is taken off without unpicking the timeline.
    ///
    /// Extending the neighbours is not always possible, a face sometimes carrying the whole shape, and then
    /// `None` comes back: better a refusal than a broken body.
    #[cfg(test)]
    pub(crate) fn remove_faces(&self, face_ids: &[u32]) -> Option<Shape> {
        self.remove_faces_why(face_ids).ok()
    }

    /// The same, but with the cause of a refusal: a face missing from the body and neighbours that cannot be
    /// extended call for different remedies, and one message for both leaves nothing but guesswork.
    pub(crate) fn remove_faces_why(&self, face_ids: &[u32]) -> Result<Shape, String> {
        if face_ids.is_empty() {
            return Err("cad-no-faces-picked".into());
        }
        let mut reason: i32 = 0;
        let p = unsafe { qym_shape_remove_faces(self.ptr, face_ids.as_ptr(), face_ids.len(), &mut reason) };
        match Self::wrap(p) {
            Some(sh) => Ok(sh),
            None if reason == -1 => Err("cad-faces-not-in-body".into()),
            None => Err("cad-neighbours-not-extendable".into()),
        }
    }

    /// Push and pull a face: the planar face `face_id` travels along its own normal by `dist`, a positive
    /// value adding material and a negative one cutting it away. This is direct modelling, which the
    /// application did not have at all — a body could only be built from a sketch or a primitive.
    ///
    /// Curved faces are deliberately not supported: moving a cylinder or a sphere is a different operation, a
    /// surface offset, with different behaviour at the junctions, and doing it here as well would give a
    /// silently wrong result on the very first rounded part. Such a face gives `None`.
    pub(crate) fn push_face(&self, face_id: u32, dist: f64) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_push_face(self.ptr, face_id, dist)) }
    }

    /// Split faces by a plane without cutting the body: the body stays one, while the faces the plane crossed
    /// fall into parts. This is how an area for paint, a pad for machining or a zone for a future feature is
    /// marked out without breaking the part apart.
    ///
    /// `None` means the plane passed clear of it, leaving nothing to split, or the kernel did not manage.
    pub(crate) fn split_faces(&self, origin: [f64; 3], normal: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_split_faces(self.ptr, origin.as_ptr(), normal.as_ptr())) }
    }

    /// Split a body by a plane: the body is cut into pieces and each piece becomes a separate body.
    ///
    /// There can be more than two pieces — a U-shaped part is cut into three by one plane — so a list comes
    /// back: two halves is a special case, and counting on it means losing pieces silently. The names of faces
    /// and edges are carried into every piece, or the references of fillets and chamfers would slide after the
    /// cut.
    ///
    /// `None` if the plane passed clear of the body, leaving nothing to cut, or the kernel did not manage:
    /// better an honest refusal than an operation that succeeded and changed nothing.
    ///
    /// `section` is the name of the section face, which the cutting plane itself produces; zero means no name
    /// was given.
    pub(crate) fn split_by_plane(&self, origin: [f64; 3], normal: [f64; 3], section: u32) -> Option<Vec<Shape>> {
        unsafe {
            let l = qym_shape_split_by_plane(self.ptr, origin.as_ptr(), normal.as_ptr(), section);
            if l.is_null() {
                return None;
            }
            let n = qym_shapelist_count(l);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                if let Some(sh) = Shape::wrap(qym_shapelist_get(l, i)) {
                    out.push(sh);
                }
            }
            qym_shapelist_free(l);
            (out.len() >= 2).then_some(out)
        }
    }

    /// `sides` holds pairs of a source face name and the name of the draft side it produced, as a flat list.
    pub(crate) fn draft_faces(&self, face_ids: &[u32], angle_deg: f64, pull: [f64; 3], np_origin: [f64; 3], np_normal: [f64; 3], sides: &[u32]) -> Option<Shape> {
        if face_ids.is_empty() {
            return refuse("draft/asked", "not one face was named to tilt");
        }
        unsafe { Self::wrap(qym_shape_draft(self.ptr, face_ids.as_ptr(), face_ids.len(), angle_deg, pull.as_ptr(), np_origin.as_ptr(), np_normal.as_ptr(), Self::opt_ptr(sides), sides.len() / 2)) }
    }
    /// A stepped hole: a tool — the main cylinder plus a counterbore or countersink — cut in the frame `pl`,
    /// which maps local to world. `kind` is 0 for a plain hole, 1 for a counterbore and 2 for a countersink.
    /// The body with the cut comes back.
    #[cfg(test)]
    pub(crate) fn hole_stepped(&self, tool: qymcad_core::model::HoleTool, pl: [f64; 12], _extra: &[u32]) -> Option<Shape> {
        self.hole_stepped_named(tool, pl, 0, &[])
    }

    /// `bore` is the name of the wall of the main bore, the one fillets and sketches are placed against.
    pub(crate) fn hole_stepped_named(&self, tool: qymcad_core::model::HoleTool, pl: [f64; 12], bore: u32, extra: &[u32]) -> Option<Shape> {
        let qymcad_core::model::HoleTool { kind, diameter: dia, depth, dia2, depth2 } = tool;
        unsafe { Self::wrap(qym_shape_hole_stepped(self.ptr, kind as i32, pl.as_ptr(), dia, depth, dia2, depth2, bore, Self::opt_ptr(extra), extra.len())) }
    }

    /// `bores` gives the wall name for each hole, one per placement.
    pub(crate) fn holes_stepped_named(&self, tool: qymcad_core::model::HoleTool, pls: &[[f64; 12]], bores: &[u32], extra: &[u32]) -> Option<Shape> {
        let qymcad_core::model::HoleTool { kind, diameter: dia, depth, dia2, depth2 } = tool;
        if pls.is_empty() {
            return None;
        }
        let flat: Vec<f64> = pls.iter().flatten().copied().collect();
        unsafe { Self::wrap(qym_shape_holes_stepped(self.ptr, kind as i32, flat.as_ptr(), pls.len(), dia, depth, dia2, depth2, bores.as_ptr(), Self::opt_ptr(extra), extra.len())) }
    }
    /// A shell centred on the surface: a wall of thickness `t` is centred on the original surface, with the
    /// faces `face_ids` left open. The body grows by +t/2 and is then hollowed by −t. `None` means the offset
    /// failed.
    pub(crate) fn shell_center(&self, t: f64, face_ids: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_shell_center(self.ptr, t, face_ids.as_ptr(), face_ids.len())) }
    }
    /// The axis of a cylindrical or conical face by its persistent id, as an origin and a direction in the
    /// local frame of the body. `None` means the face was not found or is not round, being a plane or a
    /// spline. This is what picking the axis of a circular pattern rests on.
    pub fn face_axis(&self, face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        let (mut origin, mut dir) = ([0.0f64; 3], [0.0f64; 3]);
        let ok = unsafe { qym_shape_face_axis(self.ptr, face_id, origin.as_mut_ptr(), dir.as_mut_ptr()) };
        (ok == 1).then_some((origin, dir))
    }
    /// Thicken a face: the face `face_id` becomes a plate of thickness `thickness`, the sign choosing which
    /// side grows. This is how a part is made from a curved surface — take a face of the body, thicken it by
    /// 2 mm and the skin is ready.
    ///
    /// `None` means the face was not found, the thickness is zero, or the offset failed by intersecting
    /// itself.
    ///
    /// The names come from above as pairs of before and after: `fmap` maps the name of a source face to the
    /// name of its offset side, and `emap` the name of an edge to the name of the wall it produced. The naming
    /// scheme is known to the model layer; the kernel only carries it.
    #[cfg(test)]
    pub(crate) fn thicken_face(&self, face_id: u32, thickness: f64, fmap: &[u32], emap: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_thicken_face(self.ptr, face_id, thickness, fmap.as_ptr(), fmap.len() / 2, emap.as_ptr(), emap.len() / 2)) }
    }

    /// The same, but joined to the body, keeping to the rule that a part is one body.
    ///
    /// The names of the source body are carried over, so references to its faces and edges survive the
    /// operation, while the new geometry of the plate gets its own. As a separate plate the part became two
    /// bodies — a differently coloured piece on screen instead of one part.
    pub(crate) fn thicken_face_join(&self, face_id: u32, thickness: f64, fmap: &[u32], emap: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_thicken_face_join(self.ptr, face_id, thickness, fmap.as_ptr(), fmap.len() / 2, emap.as_ptr(), emap.len() / 2)) }
    }

    /// A cylindrical face: the axis and the radius in the local frame of the body. `face_axis` gives no
    /// radius, and a measurement wants exactly that — the wall of a hole is measured for its diameter, not for
    /// where its axis lies. `None` means the face was not found or is not a cylinder, being a plane, a cone or
    /// a spline.
    pub fn face_cylinder(&self, face_id: u32) -> Option<([f64; 3], [f64; 3], f64)> {
        let (mut origin, mut dir, mut r) = ([0.0f64; 3], [0.0f64; 3], 0.0f64);
        let ok = unsafe { qym_shape_face_cylinder(self.ptr, face_id, origin.as_mut_ptr(), dir.as_mut_ptr(), &mut r) };
        (ok == 1).then_some((origin, dir, r))
    }

    /// The persistent ids of every edge of the face `face_id`, so that picking a face selects all its edges
    /// for a chamfer or a fillet. Empty means the face was not found or has no named edges.
    /// Absorbed names, as pairs of a former face name and the name of the common face. Fusing coplanar faces
    /// into a monolith collapses two named faces into one, and one of the names has to give way. The pair
    /// tells the model layer which name it gave way to, so a reference to the former one finds the common face
    /// rather than getting lost. Without this a fillet was lost wherever two walls honestly became one.
    /// What tells two edges of the same pair of faces apart: `(edge, name1, name2)`, the two smallest names
    /// of named faces meeting at its ends, excluding its own two; a zero means there is no such face. This
    /// exists so that the rank of an edge within a pair is not assigned by traversal order; see
    /// the naming of edges in `naming.rs`.
    pub(crate) fn edge_end_faces(&self) -> Vec<qymcad_core::feature::EdgeEnds> {
        unsafe {
            let n = qym_shape_edge_end_faces(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_edge_end_faces(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got * 3);
            buf.as_chunks::<3>().0.iter().map(|[edge, a, b]| qymcad_core::feature::EdgeEnds { edge: *edge, around: [*a, *b] }).collect()
        }
    }

    pub(crate) fn absorbed_names(&self) -> Vec<qymcad_core::feature::Absorbed> {
        unsafe {
            let n = qym_shape_absorbed(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 2];
            let got = qym_shape_absorbed(self.ptr, buf.as_mut_ptr(), n);
            buf.truncate(got * 2);
            buf.as_chunks::<2>().0.iter().map(|[loser, winner]| qymcad_core::feature::Absorbed { loser: *loser, winner: *winner }).collect()
        }
    }

    /// The split faces, as the id of the piece, the name of the source face and the number of the piece.
    /// Clear the records of the pieces. They are single-use: the model layer has read them and handed out the
    /// names, and records left behind would be folded into the next operation's group, which would then elect
    /// a different keeper of the name.
    pub(crate) fn clear_face_splits(&self) {
        unsafe { qym_shape_clear_face_splits(self.ptr) }
    }

    pub(crate) fn face_splits(&self) -> Vec<qymcad_core::feature::FaceSplit> {
        unsafe {
            let n = qym_shape_face_splits(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_face_splits(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got.min(n) * 3);
            buf.as_chunks::<3>().0.iter().map(|[piece, origin, index]| qymcad_core::feature::FaceSplit { piece: *piece, origin: *origin, index: *index }).collect()
        }
    }

    /// Rewrite the face names.
    pub(crate) fn rename_faces(&self, renames: &[qymcad_core::feature::Rename]) {
        if renames.is_empty() {
            return;
        }
        let from: Vec<u32> = renames.iter().map(|r| r.was).collect();
        let to: Vec<u32> = renames.iter().map(|r| r.now).collect();
        unsafe { qym_shape_rename_faces(self.ptr, from.as_ptr(), to.as_ptr(), from.len()) }
    }

    /// Pairs of an edge and its two faces, by name: the id of the edge and the names of faces A and B. Only
    /// edges with exactly two adjacent faces appear, those being the ones an edge name is derived from.
    pub(crate) fn edge_face_pairs(&self) -> Vec<qymcad_core::feature::EdgeFaces> {
        unsafe {
            let n = qym_shape_edge_face_pairs(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_edge_face_pairs(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got.min(n) * 3);
            buf.as_chunks::<3>().0.iter().map(|[edge, a, b]| qymcad_core::feature::EdgeFaces { edge: *edge, faces: [*a, *b] }).collect()
        }
    }

    /// Where every named face lies: the point of its surface at the middle of its parametric bounds, and the normal there.
    pub(crate) fn face_places(&self) -> Vec<qymcad_core::feature::FacePlace> {
        unsafe {
            let n = qym_shape_face_places(self.ptr, std::ptr::null_mut(), std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut ids = vec![0u32; n];
            let mut six = vec![0f64; n * 6];
            let got = qym_shape_face_places(self.ptr, ids.as_mut_ptr(), six.as_mut_ptr(), n).min(n);
            (0..got).map(|k| qymcad_core::feature::FacePlace { id: ids[k], at: [six[6 * k], six[6 * k + 1], six[6 * k + 2]], normal: [six[6 * k + 3], six[6 * k + 4], six[6 * k + 5]] }).collect()
        }
    }

    /// Rewrite the edge names.
    pub(crate) fn rename_edges(&self, renames: &[qymcad_core::feature::Rename]) {
        if renames.is_empty() {
            return;
        }
        let from: Vec<u32> = renames.iter().map(|r| r.was).collect();
        let to: Vec<u32> = renames.iter().map(|r| r.now).collect();
        unsafe { qym_shape_rename_edges(self.ptr, from.as_ptr(), to.as_ptr(), from.len()) }
    }

    pub fn face_edge_ids(&self, face_id: u32) -> Vec<u32> {
        let mut buf = vec![0u32; 256];
        let n = unsafe { qym_shape_face_edge_ids(self.ptr, face_id, buf.as_mut_ptr(), buf.len()) };
        buf.truncate(n);
        buf
    }
    /// Mirror about a plane through the origin — 0 for XY, 1 for XZ, 2 for YZ — giving a new shape.
    /// Mirror about an arbitrary plane, given as an origin and a normal: a datum or a face.
    pub(crate) fn mirrored_plane(&self, origin: [f64; 3], normal: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_mirror_plane(self.ptr, origin.as_ptr(), normal.as_ptr())) }
    }
    pub(crate) fn mirrored(&self, plane: u8) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_mirror(self.ptr, plane as i32)) }
    }
    /// The volume of intersection with another body, in mm³, which is how interference in an assembly is
    /// detected. `Some(0.0)` means they do not penetrate, their bounding boxes being apart or the contact
    /// being only a touch. Both bodies have to be in the same frame; see `transformed`.
    ///
    /// `None` means the kernel could not measure — which is NOT the same as "they are clear of each other",
    /// and must never be treated as one. Whoever reads this decides what an unknown is worth; for an
    /// assembly it is worth a warning, because an unmeasurable pair is exactly the pair worth looking at.
    pub(crate) fn interference_volume(&self, other: &Shape) -> Option<f64> {
        let mut v = 0.0f64;
        (unsafe { qym_shape_interference_volume(self.ptr, other.ptr, &mut v) } == 1).then_some(v)
    }
}
