//! A SHAPE BUILT: wrapped, extruded, revolved, swept, lofted, primitive, boolean, helical, threaded.

use super::*;

impl Shape {
    pub(crate) fn wrap(p: *mut QymShape) -> Option<Shape> {
        if p.is_null() {
            None
        } else {
            Some(Shape { ptr: p })
        }
    }
    /// A wrapper that checks validity, for operations the kernel does build — returning a non-null result —
    /// while the result may be geometrically broken: a fillet of a radius larger than the wall thickness, or a
    /// shell on unsuitable geometry. A broken result becomes `None`, so the feature falls into a red node and
    /// the part stays as it was, instead of showing half-transparent walls.
    pub(crate) fn wrap_valid(p: *mut QymShape) -> Option<Shape> {
        let s = Self::wrap(p)?;
        if unsafe { qym_shape_is_valid(s.ptr) } != 0 {
            return Some(s);
        }
        // An unsound shape is repaired rather than discarded. An operation that built a body with a minor
        // flaw used to return nothing from here, so "the operation failed" was reported instead of a part
        // that a standard repair puts right. If the repair fails, then nothing is returned.
        s.healed()
    }
    /// Extrude a profile into a shape.
    /// A shell of 4×4 Bezier patches: how the kernel receives a subdivision cage.
    ///
    /// It returns the shape and the number of unstitched edges. Zero means the shell closed on itself without
    /// a single hole, and that number is the whole point of the exercise. A non-zero one is not hidden: a cage
    /// with extraordinary points has holes by construction, and knowing how many matters more than seeing a
    /// picture.
    #[cfg(test)]
    pub(crate) fn from_bezier_patches(patches: &[[[[f64; 3]; 4]; 4]], tol: f64, make_solid: bool) -> Option<(Shape, u32)> {
        if patches.is_empty() {
            refuse("bezier/asked", "there is not one patch to build a surface from")?;
        }
        let mut flat: Vec<f64> = Vec::with_capacity(patches.len() * 48);
        for p in patches {
            for row in p {
                for pt in row {
                    flat.extend_from_slice(pt);
                }
            }
        }
        let mut free = 0u32;
        let s = unsafe { Self::wrap(qym_shape_bezier_shell(flat.as_ptr(), patches.len(), tol, make_solid as i32, &mut free)) }?;
        Some((s, free))
    }

    pub(crate) fn extrude(xy: &[f64], h: f64) -> Option<Shape> {
        if xy.len() / 2 < 3 {
            return refuse("extrude/asked", "the contour has fewer than three points, so it encloses nothing");
        }
        unsafe { Self::wrap(qym_shape_extrude(xy.as_ptr(), xy.len() / 2, h)) }
    }
    /// Revolve a profile into a shape.
    pub(crate) fn revolve(xy: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
        if xy.len() / 2 < 3 {
            return refuse("revolve/asked", "the contour has fewer than three points, so it encloses nothing");
        }
        let ang = if angle_deg.abs() < 1e-6 { 360.0 } else { angle_deg };
        unsafe { Self::wrap(qym_shape_revolve(xy.as_ptr(), xy.len() / 2, axis as i32, ang)) }
    }
    /// Extrude an exact profile — encoded by `Profile::encode`, with contours of real edges: lines, arcs and
    /// circles — to a height of `h`, giving a body with exact faces, so that a cylinder is three faces rather
    /// than a faceted prism. Without names, since the profile does not come from a sketch: format tests and
    /// auxiliary bodies.
    pub(crate) fn extrude_profile(data: &[f64], h: f64) -> Option<Shape> {
        Self::extrude_profile_named(data, h, [0, 0])
    }

    #[cfg(test)]
    pub(crate) fn extrude_profiles_fused(profiles: &[Vec<f64>], h: f64) -> Option<Shape> {
        Self::extrude_profiles_fused_named(profiles, h, &[])
    }

    /// `caps` holds the name descriptors of the caps, at the start and the end; zero means a positional name
    /// from the kernel.
    pub(crate) fn extrude_profile_named(data: &[f64], h: f64, caps: [u32; 2]) -> Option<Shape> {
        if data.is_empty() {
            return refuse("extrude/asked", "there is no profile to extrude");
        }
        unsafe { Self::wrap(qym_shape_extrude_profile(data.as_ptr(), data.len(), h, caps[0], caps[1])) }
    }
    /// Extrude N exact profiles as a single 2D-fused face, so that touching ones give no seam edges. `caps`
    /// holds triples of region key, bottom and top, the key being the smallest wall name of the region.
    pub(crate) fn extrude_profiles_fused_named(profiles: &[Vec<f64>], h: f64, caps: &[u32]) -> Option<Shape> {
        if profiles.is_empty() || profiles.iter().any(|p| p.is_empty()) {
            return refuse("extrude/asked", "one of the profiles to extrude is empty");
        }
        let mut data: Vec<f64> = Vec::new();
        let mut offsets: Vec<usize> = vec![0];
        for pr in profiles {
            data.extend_from_slice(pr);
            offsets.push(data.len());
        }
        unsafe { Self::wrap(qym_shape_extrude_profiles_fused(data.as_ptr(), offsets.as_ptr(), profiles.len(), h, caps.as_ptr(), caps.len())) }
    }

    /// Revolve an exact profile about an axis — 0 for X, 1 for Y — through an angle, giving a body.
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn revolve_profile(data: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
        if data.is_empty() {
            return refuse("revolve/asked", "there is no profile to revolve");
        }
        unsafe { Self::wrap(qym_shape_revolve_profile(data.as_ptr(), data.len(), axis as i32, angle_deg)) }
    }
    /// Revolve an exact profile about an arbitrary axis, given as an origin and a direction in the local
    /// frame of the sketch, through an angle.
    #[cfg(test)]
    pub(crate) fn revolve_profile_axis(data: &[f64], origin: [f64; 3], dir: [f64; 3], angle_deg: f64) -> Option<Shape> {
        Self::revolve_profile_axis_named(data, origin, dir, angle_deg, [0, 0])
    }

    /// `caps` holds the names of the end faces, present on a partial revolution; zero means positional names
    /// from the kernel.
    pub(crate) fn revolve_profile_axis_named(data: &[f64], origin: [f64; 3], dir: [f64; 3], angle_deg: f64, caps: [u32; 2]) -> Option<Shape> {
        if data.is_empty() {
            return refuse("revolve/asked", "there is no profile to revolve");
        }
        unsafe { Self::wrap(qym_shape_revolve_profile_axis(data.as_ptr(), data.len(), origin.as_ptr(), dir.as_ptr(), angle_deg, caps[0], caps[1])) }
    }
    /// A sweep: the exact profile `prof` along the path `path`, which is a single contour in the format
    /// `[1.0, loop_block]`. `prof_tf` and `path_tf` are the 3×4 placements of the two sketches into world
    /// coordinates. A profile face becomes a solid body; empty or insufficient data gives `None`.
    #[cfg(test)]
    pub(crate) fn sweep_profile(prof: &[f64], prof_tf: &[f64; 12], path: &[f64], path_tf: &[f64; 12]) -> Option<Shape> {
        Self::sweep_profile_named(prof, prof_tf, path, path_tf, [0, 0])
    }

    /// `caps` holds the names of the end faces of the sweep, at the start and the end of the path.
    pub(crate) fn sweep_profile_named(prof: &[f64], prof_tf: &[f64; 12], path: &[f64], path_tf: &[f64; 12], caps: [u32; 2]) -> Option<Shape> {
        if prof.is_empty() {
            return refuse("sweep/asked", "there is no profile to sweep");
        }
        if path.is_empty() {
            return refuse("sweep/asked", "there is no path to sweep the profile along");
        }
        unsafe { Self::wrap(qym_shape_sweep(prof.as_ptr(), prof.len(), prof_tf.as_ptr(), path.as_ptr(), path.len(), path_tf.as_ptr(), caps[0], caps[1])) }
    }

    /// A loft through sections. `data` is the concatenation of the `loop_block`s of the sections, `offsets`,
    /// of length nsec+1, gives where each section starts in `data`, and `places`, of nsec×12, gives the 3×4
    /// placements of the section planes. `ruled` asks for ruled faces rather than a smooth surface, and
    /// `solid` closes the result into a body. The lengths are validated; otherwise `None`.
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn loft_sections(data: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, kind: qymcad_core::feature::LoftBody) -> Option<Shape> {
        Self::loft_sections_named(data, offsets, places, walls, kind, [0, 0])
    }

    /// `caps` holds the names of the end faces of the loft, at the first and last sections.
    pub(crate) fn loft_sections_named(data: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, kind: qymcad_core::feature::LoftBody, caps: [u32; 2]) -> Option<Shape> {
        let (ruled, solid) = (walls == qymcad_core::feature::LoftWalls::Ruled, kind == qymcad_core::feature::LoftBody::Solid);
        let nsec = offsets.len().saturating_sub(1);
        if nsec < 2 {
            return refuse("loft/asked", "a loft needs two sections or more, and it was given fewer");
        }
        if data.is_empty() {
            return refuse("loft/asked", "the sections carry no points");
        }
        if places.len() != nsec * 12 {
            return refuse("loft/asked", "there is not one placement per section");
        }
        unsafe { Self::wrap(qym_shape_loft(data.as_ptr(), data.len(), offsets.as_ptr(), nsec, places.as_ptr(), ruled as i32, solid as i32, caps[0], caps[1])) }
    }
    /// An exact cylinder about Z with its base at z = 0: three faces, not a faceted one.
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn cylinder(r: f64, h: f64) -> Option<Shape> {
        Self::cylinder_named(r, h, [0, 0, 0])
    }
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn sphere(r: f64) -> Option<Shape> {
        Self::sphere_named(r, [0, 0, 0])
    }
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn cone(r1: f64, r2: f64, h: f64) -> Option<Shape> {
        Self::cone_named(r1, r2, h, [0, 0, 0])
    }
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn torus(major: f64, minor: f64) -> Option<Shape> {
        Self::torus_named(major, minor, [0, 0, 0])
    }

    /// `names` holds the face at the lower z, the face at the upper z, and the lateral surface.
    pub(crate) fn cylinder_named(r: f64, h: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_cylinder(r, h, names[0], names[1], names[2])) }
    }
    /// An exact sphere centred at the origin: one face.
    pub(crate) fn sphere_named(r: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_sphere(r, names[0], names[1], names[2])) }
    }
    /// An exact cone about Z, of radius `r1` at z = 0 and `r2` at z = h.
    pub(crate) fn cone_named(r1: f64, r2: f64, h: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_cone(r1, r2, h, names[0], names[1], names[2])) }
    }
    /// An exact torus in the XY plane about Z.
    pub(crate) fn torus_named(major: f64, minor: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_torus(major, minor, names[0], names[1], names[2])) }
    }
    /// A boolean against another shape: 0 subtracts, 1 unites, 2 intersects.
    pub(crate) fn boolean(&self, other: &Shape, op: u8) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_boolean(self.ptr, other.ptr, op as i32)) }
    }
    /// A boolean that must give a body: an intersection of bodies that only touch has no volume, and a cut by a tool
    /// that does not reach the base removes nothing. Both come back from OCCT as a valid shape (an empty compound, or
    /// the base unchanged), so the judgement lives here - one place for the rebuild and for the trial the bar makes
    /// before laying a node.
    pub(crate) fn boolean_checked(&self, other: &Shape, op: u8) -> Result<Shape, BoolVerdict> {
        let out = self.boolean(other, op).ok_or(BoolVerdict::Failed)?;
        let v = out.volume();
        if op == 0 && self.volume() > 1e-9 && v >= self.volume() - 1e-6 {
            return Err(BoolVerdict::CutRemovedNothing);
        }
        if op == 2 && v < 1e-9 {
            return Err(BoolVerdict::NothingInCommon);
        }
        Ok(out)
    }
    /// One union of every shape at once rather than a chain of pairwise ones: all the arguments are
    /// intersected a single time, sharing vertices and edges. The first shape carries the counter of
    /// positional numbers.
    pub(crate) fn fuse_many(parts: &[&Shape]) -> Option<Shape> {
        if parts.is_empty() {
            return refuse("fuse/asked", "there is not one body to fuse");
        }
        let ptrs: Vec<*const QymShape> = parts.iter().map(|s| s.ptr as *const QymShape).collect();
        unsafe { Self::wrap(qym_shape_fuse_many(ptrs.as_ptr(), ptrs.len() as i32)) }
    }
    /// A real thread on this body along the axis given by `origin` and `dir`, on a surface of radius
    /// `radius` taken from the geometry itself. A helical groove — of pitch `pitch`, profile angle
    /// `angle_deg`, depth `depth` and `starts` starts, `left` for a left hand and `internal` for one inside a
    /// hole — is cut out of the body. Genuine B-rep, by a helix, a pipe shell and a boolean cut, not
    /// cosmetics.
    #[allow(clippy::too_many_arguments)]
    /// A helical rib or groove from an exact profile (`geom::encode_profile`) computed by the model kernel
    /// from the thread standard. `mode` 0 subtracts, giving a thread, and 1 unites, giving the flight of an
    /// auger. The profile lies in the axial plane: x along the axis, y radially out from the surface of
    /// radius `radius`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn helical_profile(&self, h: HelicalCut) -> Option<Shape> {
        let HelicalCut { axis, radius, profile, length, lead, starts, hand, kind: helix, lead_in, lead_out, gnames, rnames, crest_relief } = h;
        let (origin, dir) = (axis.origin, axis.dir);
        unsafe {
            Self::wrap(qym_shape_helical_profile(
                self.ptr,
                origin.as_ptr(),
                dir.as_ptr(),
                radius,
                profile.as_ptr(),
                profile.len(),
                length,
                lead,
                starts.max(1) as i32,
                (hand == Hand::Left) as i32,
                (helix == Helix::Rib) as i32,
                lead_in,
                lead_out,
                gnames.as_ptr(),
                gnames.len(),
                rnames.as_ptr(),
                rnames.len(),
                crest_relief,
            ))
        }
    }

    #[cfg(test)]
    pub(crate) fn thread(&self, t: ThreadCut) -> Option<Shape> {
        let ThreadCut { axis, radius, length, pitch, angle_deg, depth, starts, hand, site, form, clearance_crest, clearance_root, lead_in, lead_out } = t;
        let (origin, dir) = (axis.origin, axis.dir);
        unsafe {
            Self::wrap(qym_shape_thread(
                self.ptr,
                origin.as_ptr(),
                dir.as_ptr(),
                radius,
                length,
                pitch,
                angle_deg,
                depth,
                starts.max(1) as i32,
                (hand == Hand::Left) as i32,
                (site == Site::Bore) as i32,
                form as i32,
                clearance_crest,
                clearance_root,
                lead_in,
                lead_out,
            ))
        }
    }
}
