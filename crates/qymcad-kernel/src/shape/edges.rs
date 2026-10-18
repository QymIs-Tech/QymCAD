//! THE EDGES OF A SHAPE: listed with their ids and vertices, smooth or sharp; filleted and chamfered.

use super::*;

impl Shape {
    /// The edges of the body as polylines, in the kernel's own edge order, for drawing and picking.
    pub fn edges(&self) -> Vec<Vec<[f32; 3]>> {
        self.edges_with_ids().0
    }
    /// The edges together with their persistent ids, in parallel. An id survives a rebuild, so a selection of
    /// edges does not slide off.
    pub fn edges_with_ids(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>) {
        unsafe {
            let e = qym_shape_edges(self.ptr);
            if e.is_null() {
                return (Vec::new(), Vec::new());
            }
            let n = qym_edges_count(e);
            let mut out = Vec::with_capacity(n);
            let mut ids = Vec::with_capacity(n);
            for i in 0..n {
                let pc = qym_edge_point_count(e, i);
                let mut buf = vec![0f32; pc * 3];
                if pc > 0 {
                    qym_edge_copy_points(e, i, buf.as_mut_ptr());
                }
                out.push((0..pc).map(|k| [buf[3 * k], buf[3 * k + 1], buf[3 * k + 2]]).collect());
                ids.push(qym_edge_id(e, i));
            }
            qym_edges_free(e);
            (out, ids)
        }
    }
    /// The edges, their ids, and what is known about circular ones: for each edge `Some((centre, axis,
    /// radius))` if it is a circle or an arc, an analytic curve, and `None` otherwise. This is what a
    /// concentric anchor on a hole or a cylinder rests on.
    ///
    /// A slice of `edges_info`: a historical view of three fields out of five.
    #[allow(clippy::type_complexity)]
    pub fn edges_full(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>, Vec<Option<([f64; 3], [f64; 3], f64)>>) {
        let (p, i, c, _s) = self.edges_full_smooth();
        (p, i, c)
    }

    /// As `edges_full`, but with a smoothness flag per edge: a tangent junction of faces near 180° has
    /// nothing left to round or to cut, so such edges are not offered for a fillet or a chamfer and are
    /// filtered out by the kernel.
    #[allow(clippy::type_complexity)]
    pub fn edges_full_smooth(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>, Vec<Option<([f64; 3], [f64; 3], f64)>>, Vec<bool>) {
        let mut info = self.edges_info();
        let mut out = (Vec::with_capacity(info.len()), Vec::with_capacity(info.len()), Vec::with_capacity(info.len()), Vec::with_capacity(info.len()));
        for e in info.drain(..) {
            out.0.push(e.poly);
            out.1.push(e.id);
            out.2.push(e.circle);
            out.3.push(e.smooth);
        }
        out
    }

    /// The two vertices of every edge with an id, by their places in [`Shape::vertices`] counted from 1 (0: none).
    pub(crate) fn edge_vertices(&self) -> Vec<EdgeVertices> {
        let n = unsafe { qym_shape_edge_vertices(self.ptr, std::ptr::null_mut(), std::ptr::null_mut(), 0) };
        let (mut ids, mut ends) = (vec![0u32; n], vec![0u32; n * 2]);
        let got = unsafe { qym_shape_edge_vertices(self.ptr, ids.as_mut_ptr(), ends.as_mut_ptr(), n) }.min(n);
        (0..got).map(|k| EdgeVertices { edge: ids[k], ends: [ends[2 * k], ends[2 * k + 1]] }).collect()
    }
    /// The vertices of the shape and the edges meeting at each - the three smallest ids - from its topology.
    pub(crate) fn vertices(&self) -> Vec<VertexAt> {
        let n = unsafe { qym_shape_vertices(self.ptr, std::ptr::null_mut(), std::ptr::null_mut(), 0) };
        let (mut pts, mut ids) = (vec![0.0f64; n * 3], vec![0u32; n * 3]);
        let got = unsafe { qym_shape_vertices(self.ptr, pts.as_mut_ptr(), ids.as_mut_ptr(), n) }.min(n);
        (0..got).map(|k| VertexAt { at: [pts[3 * k], pts[3 * k + 1], pts[3 * k + 2]], edges: [ids[3 * k], ids[3 * k + 1], ids[3 * k + 2]] }).collect()
    }
    /// Everything the kernel knows about the edges, in one pass. Walking the edges is expensive, a polyline
    /// per edge, and callers want different parts of the answer, so there is one source and `edges_full*` are
    /// slices of it.
    pub fn edges_info(&self) -> Vec<EdgeInfo> {
        unsafe {
            let e = qym_shape_edges(self.ptr);
            if e.is_null() {
                return Vec::new();
            }
            let n = qym_edges_count(e);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let pc = qym_edge_point_count(e, i);
                let mut buf = vec![0f32; pc * 3];
                if pc > 0 {
                    qym_edge_copy_points(e, i, buf.as_mut_ptr());
                }
                let mut c = [0f64; 7];
                let circle = (qym_edge_circle(e, i, c.as_mut_ptr()) != 0).then(|| ([c[0], c[1], c[2]], [c[3], c[4], c[5]], c[6]));
                let mut r = [0f64; 3];
                let ref_dir = (qym_edge_ref_dir(e, i, r.as_mut_ptr()) != 0).then_some(r);
                out.push(EdgeInfo { poly: (0..pc).map(|k| [buf[3 * k], buf[3 * k + 1], buf[3 * k + 2]]).collect(), id: qym_edge_id(e, i), circle, smooth: qym_edge_smooth(e, i) != 0, ref_dir });
            }
            qym_edges_free(e);
            out
        }
    }

    /// The ids of the smooth edges, the tangent junctions, for the fillet and chamfer filter.
    pub fn smooth_edge_ids(&self) -> std::collections::HashSet<u32> {
        let (_, ids, _, sm) = self.edges_full_smooth();
        ids.into_iter().zip(sm).filter(|&(id, s)| s && id != 0).map(|(id, _)| id).collect()
    }
    /// The named edges that are neither smooth nor the point of a pole: what a blend of every edge runs along.
    pub fn sharp_edge_ids(&self) -> Vec<u32> {
        let (polys, ids, _, sm) = self.edges_full_smooth();
        let mut out: Vec<u32> = ids.into_iter().zip(sm).zip(polys).filter(|((id, s), p)| !*s && *id != 0 && !p.is_empty()).map(|((id, _), _)| id).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
    /// Fillet the selected edges, by zero-based index, with radius `r`.
    #[cfg(test)]
    pub(crate) fn fillet_edges(&self, r: f64, idx: &[u32]) -> Option<Shape> {
        self.fillet_edges_named(r, idx, &vec![0u32; idx.len()], &vec![0u32; idx.len()], &[])
    }

    /// `names` gives the name of the fillet surface for each selected edge, in parallel with `idx`.
    pub(crate) fn fillet_edges_named(&self, r: f64, idx: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Option<Shape> {
        let got =
            unsafe { Self::wrap_valid(qym_shape_fillet_edges(self.ptr, r, idx.as_ptr(), idx.len(), Self::opt_ptr(names), Self::opt_ptr(corners), Self::opt_ptr(all_names), all_names.len() / 2)) };
        self.blended(got, r)
    }
    /// A variable fillet of the selected edges: the radius runs linearly from `r1` at the start to `r2` at
    /// the end.
    /// A variable-radius fillet driven by a table of vertices: `verts` gives a point and the radius at it. An
    /// edge takes the radii of its ends and the kernel drives the law between them; an end with no entry takes
    /// `r_default`. The vertices travel as points, names being the model's business while geometry is enough
    /// for the kernel.
    pub(crate) fn fillet_at_vertices(&self, r_default: f64, idx: &[u32], verts: &[qymcad_core::feature::RadiusAt], names: BlendNames<'_>) -> Option<Shape> {
        let pts: Vec<f64> = verts.iter().flat_map(|v| v.at).collect();
        let rads: Vec<f64> = verts.iter().map(|v| v.radius).collect();
        // the point of a vertex comes from a live rebuild of the same body: "nearly the same vertex" would be the next one
        let tol = 1e-6;
        let BlendNames { surfaces, corners, all } = names;
        let reach = rads.iter().copied().fold(r_default, f64::max);
        let got = unsafe {
            Self::wrap_valid(qym_shape_fillet_at_vertices(
                self.ptr,
                r_default,
                idx.as_ptr(),
                idx.len(),
                pts.as_ptr(),
                rads.as_ptr(),
                verts.len(),
                tol,
                Self::opt_ptr(surfaces),
                Self::opt_ptr(corners),
                Self::opt_ptr(all),
                all.len() / 2,
            ))
        };
        self.blended(got, reach)
    }

    #[cfg(test)]
    pub(crate) fn fillet_var(&self, r1: f64, r2: f64, idx: &[u32]) -> Option<Shape> {
        self.blended(unsafe { Self::wrap_valid(qym_shape_fillet_var(self.ptr, r1, r2, idx.as_ptr(), idx.len())) }, r1.max(r2))
    }
    /// An empty slice is not a null pointer. `[].as_ptr()` returns an aligned but invalid address, and a
    /// `if (ptr)` check on the C++ side lets it through, after which rubbish is read. That is how a chamfer
    /// name check brought the process down with a segfault. An honest `null` is passed when there are no
    /// names.
    pub(crate) fn opt_ptr(v: &[u32]) -> *const u32 {
        if v.is_empty() {
            std::ptr::null()
        } else {
            v.as_ptr()
        }
    }

    /// Chamfer the selected edges, by zero-based index, by `d`.
    ///
    /// `names` gives the name of the chamfer surface for each selected edge, in parallel with `idx`, and
    /// `corners` the name of the corner patch. The same recipe as for a fillet: these faces have no source
    /// other than the edge and the vertex that produced them.
    pub(crate) fn chamfer_edges(&self, d: f64, idx: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Option<Shape> {
        let got =
            unsafe { Self::wrap_valid(qym_shape_chamfer_edges(self.ptr, d, idx.as_ptr(), idx.len(), Self::opt_ptr(names), Self::opt_ptr(corners), Self::opt_ptr(all_names), all_names.len() / 2)) };
        self.blended(got, d)
    }
    /// An asymmetric chamfer. Mode 1 takes two distances, `a` on the reference face and `b` on the adjacent
    /// one; mode 2 takes the leg `a` and the angle `b` in degrees from the reference face. `flip` chooses
    /// which of the two adjacent faces is the reference, and `ref_face` is the persistent id of one picked by
    /// hand, zero meaning it follows `flip`.
    pub(crate) fn chamfer_edges_asym(&self, how: AsymChamfer, idx: &[u32], names: BlendNames<'_>) -> Option<Shape> {
        let AsymChamfer { a, b, mode, flip, ref_face } = how;
        let BlendNames { surfaces, corners, all } = names;
        // the second leg: given, or the one the angle makes from the first
        let reach = if mode == 2 { a.max(a * b.to_radians().tan().abs()) } else { a.max(b) };
        let got = unsafe {
            Self::wrap_valid(qym_shape_chamfer_edges_asym(
                self.ptr,
                a,
                b,
                mode,
                flip as i32,
                ref_face,
                idx.as_ptr(),
                idx.len(),
                Self::opt_ptr(surfaces),
                Self::opt_ptr(corners),
                Self::opt_ptr(all),
                all.len() / 2,
            ))
        };
        self.blended(got, reach)
    }
    /// THE LIMITING CASE OF A BLEND, BUILT EXACTLY: the prisms the blend `how` takes off the edges `idx` - each a straight
    /// line between planar faces meeting convexly, no two sharing a vertex - cut from the body, their surfaces numbered as
    /// `on` says. `None` where the edges are not such.
    pub(crate) fn blend_limit(&self, how: BlendLimit, on: NamedEdges<'_>) -> Option<Shape> {
        let NamedEdges { edges, surfaces } = on;
        let got = unsafe { Self::wrap_valid(qym_shape_blend_limit(self.ptr, &how, edges.as_ptr(), edges.len(), Self::opt_ptr(surfaces))) };
        self.blended(got, how.a.max(how.b))
    }
    /// A shell: remove the faces with the persistent ids `face_ids`, stable across a rebuild, and build walls
    /// by the signed `offset` — negative inward, which is the default, positive outward.
    ///
    /// `walls` seeds the names of the inner walls as pairs of a source face and the name of its image. Without
    /// it a wall takes the id of its outer face and becomes indistinguishable from it.
    /// The smallest fillet or chamfer on the body; zero means there are no rounded faces.
    pub fn min_round_radius(&self) -> f64 {
        unsafe { qym_shape_min_round_radius(self.ptr) }
    }
}
