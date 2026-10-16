//! THE GEOMETRY OF ANCHORS: faces, axes, directions, the frames of connectors.

use super::*;

impl Project {
    /// Extent of a cylindrical face along its axis: where its ends are.
    ///
    /// Needed so an anchor can be placed at an end of a hole rather than only at its midpoint. Returns the
    /// coordinates of both ends along the axis, measured from point `axis_pt`.
    pub fn cyl_face_span(&self, body: Id, key: &crate::feature::FaceKey, axis_pt: [f64; 3], axis: [f64; 3]) -> Option<(f64, f64)> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                let Some(v) = mesh.verts.get(vi as usize) else { continue };
                let d = (v.x - axis_pt[0]) * axis[0] + (v.y - axis_pt[1]) * axis[1] + (v.z - axis_pt[2]) * axis[2];
                lo = lo.min(d);
                hi = hi.max(d);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Principal direction of a face: the X axis of its anchor, derived from the body itself.
    ///
    /// Building the anchor frame as "Z is the normal, X is world Z cross the normal" makes the X axis depend on
    /// a world axis rather than on the shape of the body. A mate aligns whole frames, rotation about the normal
    /// included, so that rotation comes out arbitrary relative to the geometry: the normals meet correctly
    /// while the body settles at a random roll.
    ///
    /// An anchor has to be an object whose axes come from the geometry it stands on. Here X is the principal
    /// direction of the face: the direction of greatest spread of its vertices within the face plane, which for
    /// a rectangular patch is its long side. It is deterministic, independent of world axes, and computed the
    /// same way on both bodies.
    pub fn face_principal_dir(&self, body: Id, key: &crate::feature::FaceKey) -> Option<[f64; 3]> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let n = {
            let l = (f.normal[0].powi(2) + f.normal[1].powi(2) + f.normal[2].powi(2)).sqrt();
            if l < 1e-12 {
                return None;
            }
            [f.normal[0] / l, f.normal[1] / l, f.normal[2] / l]
        };
        // Face vertices, each counted once, projected into the face plane.
        //
        // Counting triangle vertices in sequence is wrong: on a rectangular face made of two triangles the
        // endpoints of the shared diagonal are counted twice and the spread skews towards that diagonal.
        // Measured on a 40 by 20 side face: instead of the long side (1,0,0) the result was (0.978, 0, -0.208),
        // a direction between the side and the diagonal. The travel axis of a slider comes from here, so the
        // body would move at an angle.
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                if !seen.insert(vi) {
                    continue;
                }
                if let Some(v) = mesh.verts.get(vi as usize) {
                    let d = [v.x - f.centroid.x, v.y - f.centroid.y, v.z - f.centroid.z];
                    let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
                    pts.push([d[0] - n[0] * along, d[1] - n[1] * along, d[2] - n[2] * along]);
                }
            }
        }
        if pts.len() < 3 {
            return None;
        }
        // Direction of greatest spread: power iteration on the covariance matrix.
        let mut c = [[0.0f64; 3]; 3];
        for p in &pts {
            for i in 0..3 {
                for j in 0..3 {
                    c[i][j] += p[i] * p[j];
                }
            }
        }
        // The iteration starts not from world X but from the axis least aligned with the normal.
        //
        // With world X as the starting vector, a face whose normal points along X has all its points in the YZ
        // plane: the very first step gives zero and the function honestly answers "unknown". Every face turned
        // towards world X — about a third of them in any assembly — then has no principal direction at all,
        // which is why a slider moves in the wrong direction whatever is selected: without a principal
        // direction the travel axis comes from world axes.
        let least = (0..3).min_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs())).unwrap_or(0);
        let mut v = {
            let mut e = [0.0; 3];
            e[least] = 1.0;
            let along = e[0] * n[0] + e[1] * n[1] + e[2] * n[2];
            let w = [e[0] - n[0] * along, e[1] - n[1] * along, e[2] - n[2] * along];
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if l < 1e-12 {
                return None;
            }
            [w[0] / l, w[1] / l, w[2] / l]
        };
        for _ in 0..64 {
            let w = [c[0][0] * v[0] + c[0][1] * v[1] + c[0][2] * v[2], c[1][0] * v[0] + c[1][1] * v[1] + c[1][2] * v[2], c[2][0] * v[0] + c[2][1] * v[1] + c[2][2] * v[2]];
            // Keep the direction within the face plane.
            let along = w[0] * n[0] + w[1] * n[1] + w[2] * n[2];
            let w = [w[0] - n[0] * along, w[1] - n[1] * along, w[2] - n[2] * along];
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if l < 1e-12 {
                return None;
            }
            v = [w[0] / l, w[1] / l, w[2] / l];
        }
        // The sign is made deterministic: the spread is symmetric, so it is fixed by the largest coordinate,
        // or the same face would yield X on one run and -X on the next.
        let k = (0..3).max_by(|&i, &j| v[i].abs().total_cmp(&v[j].abs())).unwrap_or(0);
        if v[k] < 0.0 {
            v = [-v[0], -v[1], -v[2]];
        }
        Some(v)
    }

    /// Axis of a cylindrical face (a hole, a shaft); `None` for a planar face.
    ///
    /// A face anchor that builds its frame from the normal fails on a cylinder, where the normal depends on the
    /// point of the surface and cancels to zero around the circle, leaving the rotation axis arbitrary and
    /// turning the loop inside out. The correct pattern is next door: an anchor on a circular edge takes the
    /// axis of the circle.
    ///
    /// How it is computed: on a cylinder every facet normal is perpendicular to the axis, so the axis is the
    /// direction orthogonal to all of them — the eigenvector of the matrix sum of n * n^T with the smallest
    /// eigenvalue. A planar face differs in that the spread of its normals is near zero, in which case there is
    /// no axis.
    pub fn face_axis(&self, body: Id, key: &crate::feature::FaceKey) -> Option<([f64; 3], [f64; 3])> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        // Facet normals of the face.
        let mut ns: Vec<[f64; 3]> = Vec::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            let (p0, p1, p2) = (mesh.verts.get(t[0] as usize)?, mesh.verts.get(t[1] as usize)?, mesh.verts.get(t[2] as usize)?);
            let u = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let v = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-12 {
                ns.push([n[0] / l, n[1] / l, n[2] / l]);
            }
        }
        if ns.len() < 3 {
            return None;
        }
        // Σ nnᵀ
        let mut a = [[0.0f64; 3]; 3];
        for n in &ns {
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += n[i] * n[j];
                }
            }
        }
        // The eigenvector of the smallest eigenvalue, by inverse iteration from three starting axes.
        let cnt = ns.len() as f64;
        let mut best: Option<([f64; 3], f64)> = None;
        for start in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let mut v = start;
            for _ in 0..64 {
                // v <- (cnt*I - A)*v: shifting the spectrum makes the former smallest eigenvalue the largest.
                let w = [
                    cnt * v[0] - (a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2]),
                    cnt * v[1] - (a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2]),
                    cnt * v[2] - (a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2]),
                ];
                let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
                if l < 1e-12 {
                    break;
                }
                v = [w[0] / l, w[1] / l, w[2] / l];
            }
            // Spread of the normals along v: near zero along the axis of a cylinder.
            let q = v[0] * (a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2]) + v[1] * (a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2]) + v[2] * (a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2]);
            if best.as_ref().is_none_or(|(_, bq)| q < *bq) {
                best = Some((v, q));
            }
        }
        let (axis, q) = best?;
        // On a planar face every normal is the same, so there are two orthogonal directions and the choice
        // between them is arbitrary. A cylinder is recognised by its normals being noticeably spread: the
        // summed normal is much shorter than the facet count, whereas for a plane the two are equal.
        let sum = ns.iter().fold([0.0f64; 3], |s, n| [s[0] + n[0], s[1] + n[1], s[2] + n[2]]);
        let sum_len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
        if sum_len > 0.9 * cnt || q > 1e-6 * cnt {
            return None; // A planar face (or anything but a cylinder) has no axis.
        }
        Some(([f.centroid.x, f.centroid.y, f.centroid.z], axis))
    }

    /// Axis anchor of an edge by id: for a circular edge (the rim of a hole or a cylinder) the centre of the
    /// circle and its axis; for a straight edge the midpoint and the tangent.
    ///
    /// One source for both an axis connector and a datum axis built from an edge, so that a concentric pick on
    /// a hole gives the axis through its centre rather than a point on the rim.
    pub fn resolve_edge_axis(&self, body: Id, edge_id: u32) -> Option<([f64; 3], [f64; 3])> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| e.axis_ref())
    }

    /// The direction a piece of geometry defines, for the optional second pick of a connector's secondary
    /// axis.
    ///
    /// An edge gives its own direction (for a circular edge, the axis of the circle) and a face gives its
    /// normal (for a cylindrical face, its axis). A vertex and the body origin define no direction: expecting a
    /// side from a point would mean inventing the answer, so this honestly returns nothing.
    pub fn anchor_direction(&self, r: &crate::feature::AnchorRef) -> Option<[f64; 3]> {
        use crate::feature::AnchorRef;
        match r {
            AnchorRef::EdgeMid(b, e) => self.resolve_edge_axis(*b, *e).map(|(_, d)| d),
            AnchorRef::FaceCenter(b, k) => match self.face_axis(*b, k) {
                Some((_, ax)) => Some(ax),
                None => Some(self.resolve_face(*b, k).1),
            },
            AnchorRef::BasePlane(p) => Some(p.frame().normal()),
            AnchorRef::Origin | AnchorRef::Vertex(..) => None,
        }
    }

    /// Reference direction of an edge: the normal of its adjacent face at its midpoint, in body local space.
    ///
    /// This is the secondary axis of a connector: an edge has one axis of its own, along itself, and without a
    /// second axis the roll of the frame is undefined. A zero vector in `ref_dir` (a dangling edge, or an
    /// analysis without adjacent faces) honestly means "none", and the caller then falls back to deriving it
    /// from a world axis.
    pub fn edge_ref_dir(&self, body: Id, edge_id: u32) -> Option<[f64; 3]> {
        let e = self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id)?;
        let n = e.ref_dir;
        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] > 1e-12).then_some(n)
    }

    /// Resolve a vertex, that is, an endpoint of an edge of body `body` by edge id (`at_end` selects the end
    /// or the start), into a point in body local space.
    pub fn resolve_vertex(&self, body: Id, edge_id: u32, at_end: bool) -> Option<[f64; 3]> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| if at_end { e.b } else { e.a })
    }

    /// The component in whose local space the anchor geometry of a connector lives.
    ///
    /// For `Origin` and `BasePlane` the geometry is defined in the space of the placement owner itself, that
    /// is, `conn.owner`. For faces, edges and vertices it is the owner of body `body` (a leaf part), even when
    /// `conn.owner` is a subassembly above it.
    pub(crate) fn connector_geom_owner(&self, conn: &crate::feature::MateConnector) -> Option<Id> {
        use crate::feature::AnchorRef;
        match &conn.anchor {
            AnchorRef::Origin | AnchorRef::BasePlane(_) => Some(conn.owner),
            AnchorRef::FaceCenter(body, _) | AnchorRef::EdgeMid(body, _) | AnchorRef::Vertex(body, _, _) => self.body_owner(*body),
        }
    }

    /// Frame of a connector in the local space of its placement owner (`conn.owner`), with the flip and the
    /// offsets applied.
    ///
    /// When `owner` is a subassembly (at the level of the joint context) while the geometry resolves in a leaf
    /// body, the frame is built in leaf space and then carried into the space of `owner` through
    /// `relative_transform`.
    // THE INDEX IS THE MEANING: `k` is the coordinate axis, and the line reads as the vector sum it is.
    #[allow(clippy::needless_range_loop)]
    pub fn connector_frame(&self, conn: &crate::feature::MateConnector) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{AnchorRef, PlaneFrame};
        // An anchor on a deleted body does not resolve.
        //
        // Otherwise `resolve_face` returns the old centroid fingerprint stored in the key itself, the solver
        // gets a garbage frame and scatters bodies to wild coordinates — which is what used to justify deleting
        // the mate outright. The document's memory is queried rather than the timeline: "not in the timeline"
        // covers both "deleted" and "not built yet", and those are different things (see
        // `Project::dead_bodies`).
        if !self.dead_bodies.is_empty() {
            if let AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) = &conn.anchor {
                if self.dead_bodies.contains(b) {
                    return None;
                }
            }
        }
        let mut fr = match &conn.anchor {
            AnchorRef::Origin => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0] },
            AnchorRef::BasePlane(b) => b.frame(),
            AnchorRef::FaceCenter(body, key) => {
                // A cylindrical face (a hole or a shaft) gives an axis rather than a normal. The normal of a
                // cylinder depends on the point and cancels to zero around the circle, so a revolute mate on a
                // hole would spin the body about a random direction. A planar face keeps centre plus normal.
                match self.face_axis(*body, key) {
                    Some((o, ax)) => {
                        // Attachment point on a cylinder: the midpoint or an end. This choice is the point of
                        // the feature — holes of different lengths do not meet at their midpoints, while their
                        // ends meet exactly.
                        let o = match self.cyl_face_span(*body, key, o, ax) {
                            Some((lo, hi)) => {
                                let d = conn.point.along(lo, hi);
                                [o[0] + ax[0] * d, o[1] + ax[1] * d, o[2] + ax[2] * d]
                            }
                            None => o,
                        };
                        PlaneFrame::from_origin_normal(o, ax, 0.0)
                    }
                    None => {
                        let (c, n) = self.resolve_face(*body, key);
                        // The secondary axis comes from the body rather than from world axes.
                        // `from_origin_normal` derives X as world Z cross the normal, so two mating faces get
                        // inconsistent secondary axes and pinning the roll turns the body by a random angle.
                        // That stayed invisible while nothing held the roll.
                        match self.face_principal_dir(*body, key) {
                            Some(x) => PlaneFrame::from_origin_axes(c, n, x),
                            None => PlaneFrame::from_origin_normal(c, n, 0.0),
                        }
                    }
                }
            }
            AnchorRef::EdgeMid(body, eid) => {
                // Axis anchor: a circular edge (the rim of a hole or a cylinder) gives the centre plus the axis
                // of the circle, a concentric anchor like a face axis; a straight edge gives the midpoint plus
                // the tangent along it.
                let (o, dir) = self.resolve_edge_axis(*body, *eid)?;
                // The secondary axis comes from the adjacent face rather than from world Z. Otherwise the roll
                // of the frame depends on how the body happens to lie in the world, the two mating bodies derive
                // it differently, and the mate settles the body at an arbitrary rotation.
                match self.edge_ref_dir(*body, *eid) {
                    Some(x) => PlaneFrame::from_origin_axes(o, dir, x),
                    None => PlaneFrame::from_origin_normal(o, dir, 0.0),
                }
            }
            AnchorRef::Vertex(body, eid, end) => {
                // Point anchor: the origin is the vertex itself, not a projection.
                //
                // The axes come from the edge the vertex belongs to rather than from the world. World axes mean
                // "however the body happens to lie", and a rigid mate on two vertices then turns the body in an
                // arbitrary direction. The main axis points away from the vertex along the edge (opposite
                // directions at the start and at the end, so each corner keeps its own frame) and the secondary
                // axis is the normal of the adjacent face.
                let p = self.resolve_vertex(*body, *eid, *end)?;
                let (_, dir) = self.regen_edges.get(body)?.iter().find(|e| e.id == *eid).map(|e| (e.mid, e.dir))?;
                let z = if *end { [-dir[0], -dir[1], -dir[2]] } else { dir };
                match self.edge_ref_dir(*body, *eid) {
                    Some(x) => PlaneFrame::from_origin_axes(p, z, x),
                    None => PlaneFrame::from_origin_normal(p, z, 0.0),
                }
            }
        };
        // An explicitly picked secondary axis outranks a derived one. A square face has no long side at all
        // and the automatic answer is arbitrary; pointing at an edge then puts the axis along it. It is placed
        // perpendicular to the main axis: the pick chooses a side, it does not replace the anchor.
        if let Some(dir) = conn.axis_ref.as_ref().and_then(|r| self.anchor_direction(r)) {
            fr = PlaneFrame::from_origin_axes(fr.origin, fr.normal(), dir);
        }
        if conn.flip {
            // A 180 degree turn about X flips Y, and with it the normal (X cross Y).
            fr.y = [-fr.y[0], -fr.y[1], -fr.y[2]];
        }
        // Rotation of the secondary axis by an arbitrary angle: the reorientation control. The axis is derived
        // from geometry, but which way it should point is the author's decision.
        if conn.rot_deg != 0.0 {
            let n = fr.normal();
            let ang = conn.rot_deg.to_radians();
            let (s, c) = ang.sin_cos();
            let rot = |v: [f64; 3]| {
                let d = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
                let cr = [n[1] * v[2] - n[2] * v[1], n[2] * v[0] - n[0] * v[2], n[0] * v[1] - n[1] * v[0]];
                [v[0] * c + cr[0] * s + n[0] * d * (1.0 - c), v[1] * c + cr[1] * s + n[1] * d * (1.0 - c), v[2] * c + cr[2] * s + n[2] * d * (1.0 - c)]
            };
            fr.x = rot(fr.x);
            fr.y = rot(fr.y);
        }
        // Offsets along all three connector axes. With only the main one available there is no way to move an
        // anchor sideways, and the body gets moved by guesswork instead.
        let d = conn.offset_xyz;
        if d != [0.0; 3] {
            let n = fr.normal();
            for k in 0..3 {
                fr.origin[k] += fr.x[k] * d[0] + fr.y[k] * d[1] + n[k] * d[2];
            }
        }
        // The geometry resolves in leaf space; when the placement owner is a subassembly above it, the frame
        // (with the flip and offsets already applied in leaf space) is carried into owner space.
        if let Some(leaf) = self.connector_geom_owner(conn) {
            if leaf != conn.owner {
                fr = fr.transformed(&self.relative_transform(leaf, conn.owner));
            }
        }
        Some(fr)
    }

    /// Connector frame built for a joint kind: for a slider on a flat face the travel axis lies in the plane.
    ///
    /// The travel axis is the Z of the frame, and for a planar face Z is the normal, so the body moves away
    /// from the face although the face was picked precisely as a guide. For a slider the frame is therefore
    /// turned: Z becomes the principal direction of the face (the long side of a rail) and the normal moves
    /// into X. The planes stay in contact while the motion runs along them.
    ///
    /// This does not apply to a cylindrical face: there Z is the axis of the cylinder and sliding along the
    /// axis is exactly what is wanted (a shaft in a hole). Turning that frame would break a working case.
    pub fn connector_frame_for_kind(&self, conn: &crate::feature::MateConnector, kind: crate::feature::JointKind) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{AnchorRef, JointKind, PlaneFrame};
        let fr = self.connector_frame(conn)?;
        if !matches!(kind, JointKind::Slider) {
            return Some(fr);
        }
        // An explicitly picked axis is the travel axis, whatever the anchor is.
        //
        // Handling this for face anchors only (below) leaves an edge anchor or an origin anchor changing only
        // the roll while the travel stays as it was. For an origin anchor the frame is the identity and its
        // normal is world Z, so the slider moves up and down no matter what is picked.
        //
        // The pick goes into the X axis of the frame (see `connector_frame`) while a slider travels along the
        // normal, so the frame has to be rebuilt with the picked direction as its normal.
        if let Some(raw) = conn.axis_ref.as_ref().and_then(|r| self.anchor_direction(r)) {
            // The direction is taken from the pick itself rather than from the frame. In the frame it becomes
            // the secondary axis, that is, projected across the main one; for an edge running along the main
            // axis that projection degenerates to zero and the frame honestly answers with an arbitrary
            // rotation. Measured: `pointing_at_an_edge_sets_the_anchor_axis_from_the_frame` went red on such an
            // edge.
            let dir = if let Some(leaf) = self.connector_geom_owner(conn) {
                if leaf != conn.owner {
                    crate::feature::apply12_dir(&self.relative_transform(leaf, conn.owner), raw)
                } else {
                    raw
                }
            } else {
                raw
            };
            return Some(PlaneFrame::from_origin_axes(fr.origin, dir, fr.normal()));
        }
        let AnchorRef::FaceCenter(body, key) = &conn.anchor else { return Some(fr) };
        if self.face_axis(*body, key).is_some() {
            return Some(fr); // A cylinder already has the right axis.
        }
        // The principal direction of the face is what the body slides along: on a rail, its long side.
        //
        // Without one, any direction in the plane is used. A face may be square, or its geometry may not be
        // raised yet, but even an arbitrary in-plane axis beats the normal: along the face the body at least
        // slides, whereas along the normal it separates from it. Answering "unknown" here drops the mate from
        // the problem silently and the body does not move at all.
        //
        // A picked axis outranks a derived one: when an edge was picked, `connector_frame` has already put its
        // direction into the X axis of the frame, and guessing the long side of the face on top of that is
        // wrong.
        let dir = self.face_principal_dir(*body, key).unwrap_or(fr.x); // The picked axis was handled above.
        let n = fr.normal();
        Some(PlaneFrame::from_origin_axes(fr.origin, dir, n))
    }

    /// Bodies grounded inside a moving subassembly, where grounding tells a lie.
    ///
    /// Grounded reads as "stands still and will not move". But a body lives inside a subassembly, and the
    /// subassembly is driven by its own mate: when it moves, the grounded body moves with it without shifting a
    /// millimetre relative to its neighbours. The word has been said and means nothing.
    ///
    /// Measured on a machine document: a grounded part inside a beam subassembly travelled 100.000 mm with the
    /// beam once the gantry was given a travel value. Three such parts existed in that document and nothing
    /// reported them.
    ///
    /// This is a name rather than a prohibition. The arrangement is legitimate — grounding a body inside a
    /// subassembly is ordinary when the subassembly is built on its own — but it has to be known that the body
    /// is not fixed in the world.
    pub fn grounded_inside_moving(&self) -> Vec<Id> {
        self.components
            .iter()
            .filter(|c| c.grounded)
            .filter(|c| {
                // Walk up from the parent: if any ancestor is driven by a mate, the body moves.
                let mut cur = c.parent;
                while let Some(x) = cur {
                    if x == self.root {
                        return false;
                    }
                    if self.drive_joint_for(x).is_some() {
                        return true;
                    }
                    cur = self.components.iter().find(|k| k.id == x).and_then(|k| k.parent);
                }
                false
            })
            .map(|c| c.id)
            .collect()
    }

    /// Whether an anchor stands on a body that moves inside its own declared owner.
    ///
    /// An anchor is declared on a component while taking its geometry from a body. Usually that body lies
    /// inside the same component and stands still within it, which is honest. But when a moving part appears on
    /// the path from the geometry to the declared owner, the anchor moves whenever that part moves: the mate
    /// holds on to something that does not stand still itself.
    ///
    /// One answer to two questions: why a mate is faulty (`joint_faults`) and whether an anchor may be placed
    /// here (the tool, before creation). Two separate checks of the same thing would drift apart silently.
    pub fn anchor_sits_on_moving_part(&self, owner: Id, anchor: &crate::feature::AnchorRef) -> bool {
        use crate::feature::AnchorRef;
        let body = match anchor {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => *b,
            AnchorRef::Origin | AnchorRef::BasePlane(_) => return false,
        };
        let Some(leaf) = self.body_owner(body) else { return false };
        let mut cur = Some(leaf);
        while let Some(x) = cur {
            if x == owner {
                return false; // Reached the declared owner without meeting anything movable on the way.
            }
            if self.drive_joint_for(x).is_some() {
                return true;
            }
            cur = self.components.iter().find(|k| k.id == x).and_then(|k| k.parent);
        }
        false
    }

    /// Mates whose anchor did not resolve, so the interface can report them.
    ///
    /// Measured on a real document: none of its five edge-based mates had a connector frame, because the bodies
    /// were imported, the live B-rep is not raised without being asked, and an edge axis is read from it. The
    /// bridge dropped such mates from the problem silently (`else { continue }`): the assembly looks assembled,
    /// nothing moves, and no explanation is given.
    ///
    /// Silence is the worst outcome, because a broken mate then looks the same as a mishandled drag. Computed
    /// on the spot, without state: the answer depends only on whether the geometry is raised right now.
    pub fn unresolved_joints(&self) -> Vec<Id> {
        self.joint_faults().into_iter().map(|(id, _)| id).collect()
    }
}
