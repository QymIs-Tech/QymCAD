//! WHAT A SHAPE IS: its volume, slivers, sheets, solids, kinds of faces, validity, bounds; copies, patches and trims.

use super::*;

/// WHAT IS KNOWN OF A BODY, for a check of the marks.
#[cfg(test)]
pub(crate) struct Marks {
    pub(crate) checked_sound: bool,
    pub(crate) made_faces_known: bool,
}

impl Shape {
    /// The volume of the body, in mm³, through `GProp`, for tests and geometric checks.
    pub fn volume(&self) -> f64 {
        unsafe { qym_shape_volume(self.ptr) }
    }
    /// A BODY OF ITS OWN, with the names of its faces and edges: what is kept for later shares nothing with this one.
    pub(crate) fn own_copy(&self) -> Option<Shape> {
        Shape::wrap(unsafe { qym_shape_own_copy(self.ptr) })
    }
    /// THE STATE OF THE BODY AS A NUMBER: the same for a body built again or carried through its bytes, another for one
    /// whose geometry or names changed. Zero when it cannot be read.
    pub(crate) fn state(&self) -> u64 {
        unsafe { qym_shape_state(self.ptr) }
    }
    /// THE VOLUME TO A PERCENT, for the checks that a result holds any and fits the box it came from - the exact one where
    /// it was counted already. What a person reads (properties, mass) is `volume`.
    pub(crate) fn rough_volume(&self) -> f64 {
        unsafe { qym_shape_volume_rough(self.ptr) }
    }
    /// B-rep validity, through `BRepCheck_Analyzer`: true for a correct body, false for one broken, doubled
    /// or self-intersecting.
    /// Resolve pinched faces into real ones, a face lying in two places not being a face. It returns how
    /// many were resolved and edits the shape in place together with the name maps.
    pub(crate) fn heal_pinched_faces(&self) -> i32 {
        unsafe { qym_shape_heal_pinched_faces(self.ptr) }
    }
    /// Take out the faces narrower than 1e-6 - cracks a limiting case leaves, not faces - letting their neighbours
    /// meet. It returns how many were taken out and edits the shape in place together with the name maps.
    /// How many faces of the body `drop_slivers` would take for cracks.
    pub fn sliver_faces(&self) -> i32 {
        unsafe { qym_shape_sliver_count(self.ptr) }
    }
    pub(crate) fn drop_slivers(&self) -> i32 {
        self.drop_slivers_under(1e-5)
    }
    /// The same for strips up to `width` wide - those a step back of that much leaves where the asked size eats a face
    /// whole.
    pub(crate) fn drop_slivers_under(&self, width: f64) -> i32 {
        unsafe { qym_shape_drop_slivers(self.ptr, width) }
    }
    /// A copy of faces as a separate sheet: `idx` says which to copy and `names` how to name the copies.
    pub(crate) fn copy_faces(&self, idx: &[u32], names: &[u32]) -> Option<Shape> {
        if idx.len() != names.len() {
            return refuse("face copy/asked", "there is not one name per face to copy");
        }
        unsafe { Self::wrap_valid(qym_shape_copy_faces(self.ptr, idx.as_ptr(), names.as_ptr(), idx.len())) }
    }

    /// An offset sheet: the faces `idx` moved `dist` along their normals, outward for the faces of a solid, each named
    /// by `names` after the one it came from.
    pub(crate) fn offset_faces(&self, idx: &[u32], names: &[u32], dist: f64) -> Option<Shape> {
        if idx.len() != names.len() {
            return refuse("offset surface/asked", "there is not one name per face to offset");
        }
        unsafe { Self::wrap_valid(qym_shape_offset_faces(self.ptr, idx.as_ptr(), names.as_ptr(), idx.len(), dist)) }
    }

    /// A patch: a surface stretched over a chain of edges of the body. `tangent` asks it to meet the edges
    /// smoothly, tangent to the neighbouring faces, rather than merely coinciding with them in position.
    pub(crate) fn patch(&self, idx: &[u32], tangent: bool, name: u32) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_patch(self.ptr, idx.as_ptr(), idx.len(), i32::from(tangent), name)) }
    }

    /// Trim a surface by another body, keeping the piece nearest the point `keep`.
    pub(crate) fn trim(&self, tool: &Shape, keep: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_trim(self.ptr, tool.ptr, keep.as_ptr())) }
    }

    /// Stitch sheets into one surface. If it closes, a body comes back; if not, a sheet.
    ///
    /// `Err(NothingJoined)` means no edge joined: the sheets do not touch each other, and a stitch would give
    /// the same two islands under one name.
    pub(crate) fn stitch(parts: &[&Shape], tol: f64) -> Result<Shape, NothingJoined> {
        if parts.len() < 2 {
            return Err(NothingJoined);
        }
        let ptrs: Vec<*const QymShape> = parts.iter().map(|p| p.ptr as *const QymShape).collect();
        // The bridge counts the stitched and free edges itself and reports "nothing joined" as a zero, which
        // is the only case that has to be told apart here. The rest is visible in the result: if it closed, a
        // body arrived; if not, a sheet.
        let (mut free, mut joined) = (0u32, 0u32);
        let out = unsafe { Self::wrap_valid(qym_shape_stitch(ptrs.as_ptr(), ptrs.len(), tol, &mut free, &mut joined)) };
        out.ok_or(NothingJoined)
    }

    /// Replace faces of a body with a surface: the node where the design layer joins the timeline.
    ///
    /// `Err(n)` means the surface did not meet the opening, with `n` edges left unpaired; a zero means it
    /// failed for some other reason. That number has to reach the outside, since "it did not work" without it
    /// leaves nothing but guesswork.
    pub(crate) fn replace_faces(&self, idx: &[u32], surf: &Shape, tol: f64) -> Result<Shape, u32> {
        let mut free = 0u32;
        let out = unsafe { Self::wrap_valid(qym_shape_replace_faces(self.ptr, idx.as_ptr(), idx.len(), surf.ptr, tol, &mut free)) };
        out.ok_or(free)
    }

    /// A sheet, that is a surface rather than a body: there are faces but no solids. A sheet has no volume by
    /// nature, and the funnel of the kernel has to measure it differently from a solid.
    pub fn is_sheet(&self) -> bool {
        unsafe { qym_shape_kind(self.ptr) == 2 }
    }

    /// The shape is empty: neither solids nor faces.
    pub fn is_empty_shape(&self) -> bool {
        unsafe { qym_shape_kind(self.ptr) == 0 }
    }

    /// How many shells the shape has: one for a solid body, two for a hollow one. More than that means the
    /// body is assembled from copies, by a pattern or a mirror, and the kernel cannot offset such a thing.
    pub fn shell_count(&self) -> u32 {
        unsafe { qym_shape_shell_count(self.ptr) as u32 }
    }
    /// Whether this shape and `other` share an element - a face, an edge or a vertex that is one kernel sub-shape in
    /// both, wherever each places it, not merely an equal one. Two bodies that share one must not be worked on two
    /// threads at once, so the rebuild keeps them on one thread (see `qymcad_core`'s `Kernel::shares` and the batch
    /// dispatch in `model/regen.rs`).
    pub fn shares_geometry(&self, other: &Shape) -> bool {
        unsafe { qym_shapes_share(self.ptr, other.ptr) != 0 }
    }
    /// How many bodies the shape has, zero for a sheet. More than one means the operation broke the part into
    /// pieces.
    pub fn solid_count(&self) -> u32 {
        unsafe { qym_shape_solid_count(self.ptr) as u32 }
    }
    /// The solids of the shape in the kernel's order: the centre of mass and the volume of each.
    pub fn solids_info(&self) -> Vec<([f64; 3], f64)> {
        let mut max = 16usize;
        loop {
            let (mut c, mut v) = (vec![0.0f64; 3 * max], vec![0.0f64; max]);
            let n = unsafe { qym_shape_solids_info(self.ptr, c.as_mut_ptr(), v.as_mut_ptr(), max as i32) }.max(0) as usize;
            if n <= max {
                return (0..n).map(|i| ([c[3 * i], c[3 * i + 1], c[3 * i + 2]], v[i])).collect();
            }
            max = n;
        }
    }
    /// The solid number `index` of the shape, in the order of `solids_info`, keeping its face and edge ids.
    pub(crate) fn solid_at(&self, index: usize) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_solid_at(self.ptr, index as i32)) }
    }
    /// The faces by the kind of surface they lie on: plane, cylinder, cone, sphere, torus, free form, any other.
    /// `None` when the kernel could not read them.
    pub fn face_kinds(&self) -> Option<[u32; 7]> {
        let mut out = [0i32; 7];
        (unsafe { qym_shape_face_kinds(self.ptr, out.as_mut_ptr()) } != 0).then(|| out.map(|n| n as u32))
    }
    /// Repair minor flaws of the shape, through `ShapeFix`. `None` means it did not help.
    pub(crate) fn healed(&self) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_heal(self.ptr)) }
    }
    pub fn is_valid(&self) -> bool {
        unsafe { qym_shape_is_valid(self.ptr) != 0 }
    }
    /// A rigid transformation, as a row-major 3x4 of the X, Y and N axes plus the origin, giving a placed
    /// shape. Open to the window, unlike the operations: a placement (or a scale of an import) makes no element and
    /// unmakes none, so there is nothing for the door to clean or to name - the faces and edges keep their names.
    pub(crate) fn transformed(&self, m: &[f64; 12]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_transform(self.ptr, m.as_ptr())) }
    }
    /// Fillet every edge with radius `r`, giving a new shape.
    pub(crate) fn fillet_all(&self, r: f64) -> Option<Shape> {
        self.blended(unsafe { Self::wrap_valid(qym_shape_fillet_all(self.ptr, r)) }, r)
    }
    /// Chamfer every edge by `d`, giving a new shape.
    pub(crate) fn chamfer_all(&self, d: f64) -> Option<Shape> {
        self.blended(unsafe { Self::wrap_valid(qym_shape_chamfer_all(self.ptr, d)) }, d)
    }

    /// THE BOX OF THE GEOMETRY ITSELF, not of the poles of its curves and surfaces: the box `bbox` gives of a variable
    /// fillet of R1 to R2 on a plate stood 2.9 mm past the plate, and of a chamfer on a loft 14 mm past it, both bodies
    /// sound and inside it.
    pub(crate) fn bbox_exact(&self) -> Option<[f64; 6]> {
        let mut out = [0.0f64; 6];
        (unsafe { qym_shape_bbox_exact(self.ptr, out.as_mut_ptr()) } == 1).then_some(out)
    }
    /// THE BOX OF THE POLES of the curves and surfaces, the triangulation left aside: a body meshed and one not yet are
    /// boxed alike.
    pub(crate) fn bbox_poles(&self) -> Option<[f64; 6]> {
        let mut out = [0.0f64; 6];
        (unsafe { qym_shape_bbox_poles(self.ptr, out.as_mut_ptr()) } == 1).then_some(out)
    }
    /// THE EXACT BOX OF THE FACES THE OPERATION MADE OR TRIMMED - not those it kept, which stand where they stood; every
    /// face where that is not known. `None` with no such face.
    pub(crate) fn bbox_exact_of_new_faces(&self) -> Option<[f64; 6]> {
        let mut out = [0.0f64; 6];
        (unsafe { qym_shape_bbox_exact_fresh(self.ptr, out.as_mut_ptr()) } == 1).then_some(out)
    }
    /// WHAT IS KNOWN OF THE BODY, for a check: marked checked sound or not, the faces it made known or not.
    #[cfg(test)]
    pub(crate) fn marks(&self) -> Marks {
        let mut out = [0i32; 2];
        let _ = unsafe { qym_shape_marks(self.ptr, out.as_mut_ptr()) };
        Marks { checked_sound: out[0] == 1, made_faces_known: out[1] == 1 }
    }
    /// A BLEND OF EDGES STAYS IN ITS BODY. A fillet or a chamfer takes material off a convex edge and puts it into a
    /// concave corner: either way the result stands inside the box of the body it was made from - give or take `reach`,
    /// the size of the blend - and its volume stays above zero and
    /// within that box. OCCT answers some limiting cases with a body that does neither and that its check calls valid:
    /// a rounding of R1.99999996 on the arc of an R10 rounding ending in the corners of two chamfers that ate their
    /// faces came back 3.6e6 mm^3 reaching 590 mm out of a block of 48,916 mm^3. Such a body is refused, in words.
    ///
    /// The two boxes are read off the poles first, which stand round the geometry by millimetres, both the same way -
    /// the box of a meshed body is read off its triangulation otherwise, and the box of the poles of every sound fillet of
    /// a propeller stood past it: a result whose poles stand within a twentieth of the size of the body past its poles is
    /// taken. Past that, the box of the geometry itself: of the source whole, kept with it once counted (0.6 s on a body
    /// of 80 spline faces), and of the faces the blend made or trimmed - the faces it kept stand where they stood. The
    /// bodies refused stand hundreds of millimetres past (a stair of 3.6e6 mm^3 standing 590 mm out).
    pub(crate) fn blended(&self, got: Option<Shape>, reach: f64) -> Option<Shape> {
        let got = got?;
        let past = |a: [f64; 6], b: [f64; 6]| (0..3).map(|i| (a[i] - b[i]).max(b[i + 3] - a[i + 3])).fold(0.0f64, f64::max);
        let limit = |a: [f64; 6]| reach.abs() + 1e-3 * ((a[3] - a[0]).powi(2) + (a[4] - a[1]).powi(2) + (a[5] - a[2]).powi(2)).sqrt().max(1.0);
        // the poles of a rounding stand out round it by millimetres; a body the kernel botched, by hundreds
        let roomy = |a: [f64; 6]| limit(a) + 0.05 * ((a[3] - a[0]).powi(2) + (a[4] - a[1]).powi(2) + (a[5] - a[2]).powi(2)).sqrt();
        let within = self.bbox_poles().is_some_and(|a| got.bbox_poles().is_some_and(|b| past(a, b) <= roomy(a)));
        if !within {
            if let Some(a) = self.bbox_exact() {
                let beyond = got.bbox_exact_of_new_faces().map_or(0.0, |b| past(a, b));
                if beyond > limit(a) {
                    return refuse("blend/check", &format!("the kernel gave a body standing {beyond:.1} mm past the body it blends"));
                }
            }
        }
        self.volume_kept(got)
    }

    /// THE BLEND OF A BODY KEEPS A VOLUME: above zero and within the box of the body it was made from. A sheet has no
    /// volume to keep.
    fn volume_kept(&self, got: Shape) -> Option<Shape> {
        let Some(a) = self.bbox() else { return Some(got) };
        let (before, after) = (self.rough_volume(), got.rough_volume());
        let room = (a[3] - a[0]) * (a[4] - a[1]) * (a[5] - a[2]);
        if before > 0.0 && (after <= 0.0 || after > room) {
            return refuse("blend/check", &format!("the kernel gave a body of {after:.1} mm^3 from one of {before:.1} mm^3"));
        }
        Some(got)
    }
}
