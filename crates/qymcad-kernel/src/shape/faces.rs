//! A SHAPE FROM FACES AND TO TRIANGLES: made of a mesh or of faces, its bounds, its tessellation.

use super::*;

impl Shape {
    /// A TRIANGLE MESH TURNED INTO A BODY a person can cut, drill and sketch on - a polyhedron with the mesh's
    /// flat neighbours merged into single faces (see `qym_shape_from_mesh`). A closed mesh gives a solid, an open
    /// one a surface body. `None` for a mesh with no triangle that has an area.
    pub(crate) fn from_mesh(mesh: &qymcad_core::geom::Mesh) -> Option<Shape> {
        let verts: Vec<f64> = mesh.verts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
        let tris: Vec<u32> = mesh.tris.iter().flat_map(|t| *t).collect();
        Shape::wrap(unsafe { qym_shape_from_mesh(verts.as_ptr(), mesh.verts.len(), tris.as_ptr(), mesh.tris.len()) })
    }

    /// A BODY FROM FACES RECOGNISED ON A MESH, and the triangles `loose` of regions left as mesh as flat faces: every face on its surface, bounded by its loops, the edges exact lines
    /// and circles where they are, the faces sewn within ten `tol` and a closed shell made a solid (see
    /// `qym_shape_from_faces`). Returns the body and, for every face, the area it took of its surface - `None` where it
    /// could not be built; `None` for all of it where no face was, with the kernel's words in [`last_kernel_refusal`];
    /// and how many sides no second face met when the faces were sewn - none means the shell closed.
    pub(crate) fn from_faces(corners: &[[f64; 3]], edges: &[FaceEdge], faces: &[BoundedFace], loose: &[[[f64; 3]; 3]], tol: f64) -> Option<BuiltBody> {
        Self::from_faces_as(corners, edges, faces, loose, tol, false)
    }

    /// The area each face took of its surface, the faces built as `from_faces` builds them but neither sewn nor made a
    /// solid: what the rounds that take wrong faces back ask, at a fraction of the cost - sewing and mending the shell
    /// were 9 s of every 16 on a mesh of 30 636 triangles.
    pub fn face_areas(corners: &[[f64; 3]], edges: &[FaceEdge], faces: &[BoundedFace], tol: f64) -> Option<Vec<Option<f64>>> {
        Self::from_faces_as(corners, edges, faces, &[], tol, true).map(|b| b.areas)
    }

    /// Keep the faces `from_faces` and `face_areas` build, on this thread, until `false`: the rounds of one recognition
    /// build the same faces again and again.
    pub(crate) fn keep_faces(on: bool) {
        unsafe { qym_faces_cache(on as i32) }
    }

    pub(crate) fn from_faces_as(corners: &[[f64; 3]], edges: &[FaceEdge], faces: &[BoundedFace], loose: &[[[f64; 3]; 3]], tol: f64, faces_only: bool) -> Option<BuiltBody> {
        let (mut skind, mut sparam) = (Vec::with_capacity(faces.len()), Vec::with_capacity(SURFACE_NUMBERS * faces.len()));
        for f in faces {
            let (kind, head): (i32, &[f64]) = match f.surface {
                FaceSurface::Plane { point: o, normal: n } => (0, &[o[0], o[1], o[2], n[0], n[1], n[2]]),
                FaceSurface::Cylinder { point: o, axis: a, radius } => (1, &[o[0], o[1], o[2], a[0], a[1], a[2], radius]),
                FaceSurface::Cone { apex: o, axis: a, half_angle } => (2, &[o[0], o[1], o[2], a[0], a[1], a[2], half_angle]),
                FaceSurface::Sphere { center: o, radius } => (3, &[o[0], o[1], o[2], 0.0, 0.0, 1.0, radius]),
                FaceSurface::Torus { center: o, axis: a, major, minor } => (4, &[o[0], o[1], o[2], a[0], a[1], a[2], major, minor]),
                FaceSurface::Free { normal: n } => (5, &[0.0, 0.0, 0.0, n[0], n[1], n[2]]),
                FaceSurface::Spline { nu, nv, frame: f } => (9, &[nu as f64, nv as f64, f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9]]),
                FaceSurface::Helix { point: o, axis: a, reference: x, rise, radial, axial, offset, turn, reach } => {
                    (6, &[o[0], o[1], o[2], a[0], a[1], a[2], x[0], x[1], x[2], rise, radial, axial, offset, turn[0], turn[1], reach[0], reach[1]])
                }
                FaceSurface::RoundHelix { point: o, axis: a, reference: x, rise, middle, height, round, turn, reach } => {
                    (8, &[o[0], o[1], o[2], a[0], a[1], a[2], x[0], x[1], x[2], rise, middle, height, round, turn[0], turn[1], reach[0], reach[1]])
                }
                FaceSurface::Coil { point: o, axis: a, reference: x, rise, radius, lift, wire, turn } => {
                    (7, &[o[0], o[1], o[2], a[0], a[1], a[2], x[0], x[1], x[2], rise, radius, lift, wire, turn[0], turn[1]])
                }
            };
            let mut p = [0.0; SURFACE_NUMBERS];
            p[..head.len()].copy_from_slice(head);
            skind.push(kind);
            sparam.extend_from_slice(&p);
        }
        let vxyz: Vec<f64> = corners.iter().flatten().copied().collect();
        let (mut ekind, mut eparam, mut eends, mut epstart, mut epts) = (Vec::new(), Vec::new(), Vec::new(), vec![0usize], Vec::new());
        for e in edges {
            let (kind, p): (i32, [f64; 11]) = match e.curve {
                EdgeCurve::Line { point: o, dir: d } => (0, [o[0], o[1], o[2], d[0], d[1], d[2], 0.0, 0.0, 0.0, 0.0, 0.0]),
                EdgeCurve::Circle { center: o, axis: a, radius } => (1, [o[0], o[1], o[2], a[0], a[1], a[2], radius, 0.0, 0.0, 0.0, 0.0]),
                EdgeCurve::Points { close } => (2, [if close { 1.0 } else { 4.0 }, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
                EdgeCurve::Polyline => (3, [0.0; 11]),
                EdgeCurve::Ellipse { center: o, normal: n, major_dir: m, major, minor } => (4, [o[0], o[1], o[2], n[0], n[1], n[2], m[0], m[1], m[2], major, minor]),
            };
            ekind.push(kind);
            eparam.extend_from_slice(&p);
            match e.ends {
                Some([a, z]) => eends.extend([a as i64, z as i64]),
                None => eends.extend([-1i64, -1]),
            }
            epts.extend(e.points.iter().flatten().copied());
            epstart.push(epts.len() / 3);
        }
        let fsurface: Vec<i32> = (0..faces.len() as i32).collect();
        let foutward: Vec<i32> = faces.iter().map(|f| f.outward as i32).collect();
        let finside: Vec<f64> = faces.iter().flat_map(|f| f.inside).collect();
        let (mut floops, mut lstart, mut litems) = (vec![0usize], vec![0usize], Vec::new());
        for f in faces {
            for l in &f.loops {
                litems.extend(l.iter().map(|&(e, forward)| if forward { e as i64 + 1 } else { -(e as i64 + 1) }));
                lstart.push(litems.len());
            }
            floops.push(lstart.len() - 1);
        }
        let (mut fpstart, mut fpts) = (vec![0usize], Vec::new());
        for f in faces {
            fpts.extend(f.points.iter().flatten().copied());
            fpstart.push(fpts.len() / 3);
        }
        let flat_loose: Vec<f64> = loose.iter().flatten().flatten().copied().collect();
        let mut built = 0u32;
        let mut free = 0u32;
        let mut free_at = vec![0.0f64; 3 * 256];
        let mut dropped = 0u32;
        let mut areas = vec![-1.0f64; faces.len()];
        let body = Shape::wrap(unsafe {
            qym_shape_from_faces(
                faces.len(),
                skind.as_ptr(),
                sparam.as_ptr(),
                corners.len(),
                vxyz.as_ptr(),
                edges.len(),
                ekind.as_ptr(),
                eparam.as_ptr(),
                eends.as_ptr(),
                epstart.as_ptr(),
                epts.as_ptr(),
                faces.len(),
                fsurface.as_ptr(),
                foutward.as_ptr(),
                finside.as_ptr(),
                floops.as_ptr(),
                lstart.as_ptr(),
                litems.as_ptr(),
                loose.len(),
                flat_loose.as_ptr(),
                tol,
                &mut built,
                areas.as_mut_ptr(),
                &mut free,
                free_at.as_mut_ptr(),
                256,
                &mut dropped,
                faces_only as i32,
                fpstart.as_ptr(),
                fpts.as_ptr(),
            )
        })?;
        let places = (free as usize).min(256);
        Some(BuiltBody {
            shape: body,
            areas: areas.into_iter().map(|a| (a >= 0.0).then_some(a)).collect(),
            free_edges: free as usize,
            free_at: (0..places).map(|k| [free_at[3 * k], free_at[3 * k + 1], free_at[3 * k + 2]]).collect(),
            loose_dropped: dropped as usize,
        })
    }

    /// The bounding box of the body in its own frame, as `[xmin, ymin, zmin, xmax, ymax, zmax]` in mm.
    /// `None` means an empty or broken shape.
    pub fn bbox(&self) -> Option<[f64; 6]> {
        let mut out = [0.0f64; 6];
        (unsafe { qym_shape_bbox(self.ptr, out.as_mut_ptr()) } == 1).then_some(out)
    }

    /// The diagonal of the bounding box in mm: the characteristic size of the body. A zero means the box
    /// could not be computed.
    pub fn bbox_diag(&self) -> f64 {
        let Some(b) = self.bbox() else { return 0.0 };
        let (dx, dy, dz) = (b[3] - b[0], b[4] - b[1], b[5] - b[2]);
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        if d.is_finite() {
            d
        } else {
            0.0
        }
    }

    /// Tessellate into bodies, for drawing and for machining.
    pub(crate) fn tessellate(&self, deflection: f64) -> Vec<Built> {
        let defl = if deflection > 0.0 { deflection } else { 0.5 };
        unsafe { doc_to_bodies(qym_shape_tessellate(self.ptr, defl)) }
    }

    /// Tessellation with a deflection adapted to the size of the body; see [`adaptive_deflection`]. For probes.
    #[cfg(any(test, feature = "specimens"))]
    pub(crate) fn tessellate_auto(&self, k: f64) -> Vec<Built> {
        self.tessellate(adaptive_deflection(self.bbox_diag(), k))
    }

    /// [`Shape::tessellate_merged`] with an adaptive deflection: the working path for drawing.
    pub(crate) fn tessellate_merged_auto(&self, k: f64) -> Option<Built> {
        self.tessellate_merged(adaptive_deflection(self.bbox_diag(), k))
    }

    /// Tessellation of the whole shape into one mesh: the solids of a compound are merged — a pattern, a
    /// mirror together with its original, a cut that fell into pieces. Otherwise one body node would show only
    /// the first solid. `None` means empty.
    pub(crate) fn tessellate_merged(&self, deflection: f64) -> Option<Built> {
        let bodies = self.tessellate(deflection);
        if bodies.is_empty() {
            return None;
        }
        let (mut verts, mut tris, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        let mut norms: Vec<u32> = Vec::new();
        for Built { mesh: m, faces: fs } in bodies {
            let voff = verts.len() as u32;
            let toff = tris.len() as u32;
            // THE SURFACE'S NORMALS GO WITH THEIR VERTICES; a solid that came without them leaves zeros in its place,
            // so the ones after it stay beside their own vertices
            if m.norms.len() == m.verts.len() {
                norms.extend(m.norms);
            } else {
                norms.resize(norms.len() + m.verts.len(), 0);
            }
            verts.extend(m.verts);
            tris.extend(m.tris.into_iter().map(|t| [t[0] + voff, t[1] + voff, t[2] + voff]));
            for mut f in fs {
                f.triangles.iter_mut().for_each(|ti| *ti += toff);
                faces.push(f);
            }
        }
        Some(Built { mesh: qymcad_core::geom::Mesh { verts, tris, norms }, faces })
    }
}
