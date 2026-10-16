//! THE GIZMOS: of a body, of a component, of a section, and the arrows of a command at its profile.

use super::*;

/// Barycentric interpolation of the vertex colours (Gouraud). `b0` to `b2` are normalised weights summing
/// to about 1, paired with `cols[0..2]`. It works for premultiplied colour too, being linear.
#[inline]
/// WHICH BODY AN OPERATION WORKS ON: the one selected, failing that the one the context stands in.
///
/// Three lines, and twenty-seven methods of the Part workbench used to hang off it - the commonest question
/// a feature asks. Free, it stops holding them all inside the crate that declares `App`.
pub fn op_target_body(dc: &DrawCtx, sel: Sel) -> Option<Id> {
    selected_body(dc.project, &sel).or_else(|| current_body(dc))
}

/// The step vector for direction `dir` (0 = X, 1 = Y, 2 = Z) of magnitude `step`.
pub fn arr_vec(dir: u8, step: f64) -> (f64, f64, f64) {
    match dir {
        1 => (0.0, step, 0.0),
        2 => (0.0, 0.0, step),
        _ => (step, 0.0, 0.0),
    }
}

/// THE DEPTH OF A FRAME: how the distance along the line of sight becomes the number the depth buffer holds.
///
/// REVERSED, NEARER IS LARGER, the way the card compares (`Greater`, cleared to 0). In perspective the number is
/// `z_near / zc` - 1 at the near plane, 0 at infinity - so a float keeps the same relative precision at every distance
/// and there is no far plane to cut a part off; in orthographic it is linear over the depth of the scene. The depth used
/// to be tight round the scene's bounding sphere with the near plane clamped to 1/50 of the eye's distance and the far
/// one at the sphere: in orthographic a part 5 m in front of the one looked at, 100 mm across, fell outside it and was
/// cut away.
#[derive(Clone, Copy, Debug)]
pub struct ProjParams {
    /// 1 / the distance from the eye to the target, 0 in orthographic.
    pub inv_d_eye: f64,
    /// The near plane in perspective: a thousandth of the eye's distance.
    pub z_near: f64,
    /// Half the depth of the orthographic range round the target, along the line of sight.
    pub depth_half: f64,
}

/// The number the depth buffer holds for a world `depth = rel . fwd` (see `ProjParams`). In perspective it is
/// `clip_z / clip_w`, planar on the screen, so the linear interpolation in `raster_band` gives the right visibility; in
/// orthographic it is linear in the world. THE SAME formula as the shader's `vs_mesh`, so both paths agree.
pub fn depth_ndc(world_depth: f64, proj: &ProjParams) -> f32 {
    if proj.inv_d_eye > 0.0 {
        let zc = (world_depth + 1.0 / proj.inv_d_eye).max(proj.z_near); // in front of the eye: the raster clips what is not
        (proj.z_near / zc) as f32
    } else {
        (0.5 - world_depth / (2.0 * proj.depth_half)) as f32
    }
}

/// Rotate a normal by the linear (3x3) part of a body's world transform, ignoring the translation. The
/// components' transforms are rigid and uniform, so R * n is enough without an inverse transpose; the
/// result is normalised.
pub fn rotate_normal(wt: &[f64; 12], n: [f64; 3]) -> [f64; 3] {
    v_norm([wt[0] * n[0] + wt[1] * n[1] + wt[2] * n[2], wt[4] * n[0] + wt[5] * n[1] + wt[6] * n[2], wt[8] * n[0] + wt[9] * n[1] + wt[10] * n[2]])
}

/// The outline polyline of an ellipse entity in WORLD coordinates (for hit-testing and drawing).
pub fn ellipse_outline_world(project: &qymcad_core::model::Project, si: usize, c: Id, ma: Id, mi: Id) -> Vec<Point2> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let (Some((cx, cy)), Some((max, may)), Some((mix, miy))) = (p(c), p(ma), p(mi)) else { return Vec::new() };
    let major = ((max - cx).powi(2) + (may - cy).powi(2)).sqrt().max(1e-6);
    let minor = ((mix - cx).powi(2) + (miy - cy).powi(2)).sqrt().max(1e-6);
    let (ux, uy) = ((max - cx) / major, (may - cy) / major);
    let (vx, vy) = (-uy, ux);
    let n = 72;
    (0..=n)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / n as f64;
            let (ct, st) = (t.cos(), t.sin());
            Point2::new(cx + major * ct * ux + minor * st * vx, cy + major * ct * uy + minor * st * vy)
        })
        .collect()
}

/// THE LINE OF SIGHT AT POINT `p` — the ray FROM THE EYE, not the camera's overall direction.
///
/// In orthographic mode there is one ray for the whole frame, and it is `fwd`. In perspective every
/// point has its own, diverging from `fwd` towards the edges of the frame the more, the wider the field
/// of view. Back-face culling computed from `fwd` therefore lied both ways: visible faces were thrown
/// away (gaps in the body) and invisible ones stayed (a ring fell apart into ribbons). One formula for
/// the raster and for the shader.
pub fn view_dir_at(cam: &Cam3, p: [f64; 3], fwd: [f64; 3], inv_d: f64) -> [f64; 3] {
    if inv_d <= 0.0 {
        return fwd;
    }
    let d_eye = 1.0 / inv_d;
    let eye = [cam.target[0] - fwd[0] * d_eye, cam.target[1] - fwd[1] * d_eye, cam.target[2] - fwd[2] * d_eye];
    v_norm(v_sub(p, eye))
}

/// Whether a body takes part in an interference (for the red highlight in `draw_mesh`).
pub fn body_interferes(interference: &Interference, body: Id) -> bool {
    interference.pairs.iter().any(|(a, b)| *a == body || *b == body)
}

/// The world contours of the loft's sections (in `loft_sids` order), each a closed loop of world points.
pub fn loft_preview(loft: &LoftParams, project: &Project) -> Vec<Vec<[f64; 3]>> {
    let mut out = Vec::with_capacity(loft.sids.len());
    for (i, &sid) in loft.sids.iter().enumerate() {
        let cid = project.loft_section_contour(sid, loft.cids.get(i).copied().unwrap_or(0));
        let (Some(cid), Some(pf)) = (cid, project.sketch_frame_by_id(sid)) else { continue };
        let Some(idx) = project.contour_index(cid) else { continue };
        let c = &project.contours[idx];
        if !c.closed || c.points.len() < 3 {
            continue;
        }
        let loop_w: Vec<[f64; 3]> = c
            .points
            .iter()
            .map(|p| {
                let w = pf.lift(*p);
                [w.x, w.y, w.z]
            })
            .collect();
        out.push(loop_w);
    }
    out
}

pub fn selected_body(project: &Project, sel: &Sel) -> Option<Id> {
    match *sel {
        Sel::Mesh(mi) => project.mesh_id(mi),
        Sel::Feature(ti) => project.timeline.get(ti).and_then(|n| n.kind.body()),
        _ => None,
    }
}

/// The readout of a component's gizmo (mm or degrees), taking snapping into account.
pub fn comp_giz_readout(comp_giz: &CompGizmo, set: &Settings, snap: bool) -> Option<String> {
    let (_, _, _, amt) = comp_giz.drag?;
    if comp_giz.axis.is_some() {
        let step = set.snap.grid.max(0.01);
        let d = if snap { (amt / step).round() * step } else { amt };
        Some(qymcad_i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
    } else if comp_giz.ring.is_some() {
        let step = set.snap.rot_deg.max(0.1);
        let g = if snap { (amt / step).round() * step } else { amt };
        Some(format!("{g:+.1}{}", qymcad_i18n::tr("unit-deg-suffix")))
    } else {
        None
    }
}

/// The current number for a body gizmo's readout (mm for an axis, degrees for a ring), with snapping applied.
pub fn body_giz_readout(body_giz: &BodyGizmo, set: &Settings, snap: bool) -> Option<String> {
    let (_, _, amt) = body_giz.drag?;
    if body_giz.axis.is_some() {
        let step = body_snap_mm(set);
        let d = if snap { (amt / step).round() * step } else { amt };
        Some(qymcad_i18n::tr1("giz-shift-mm", "v", &format!("{d:+.2}")))
    } else if body_giz.ring.is_some() {
        let g = if snap { (amt / body_snap_deg(set)).round() * body_snap_deg(set) } else { amt };
        Some(format!("{g:+.1}{}", qymcad_i18n::tr("unit-deg-suffix")))
    } else {
        None
    }
}

/// The radius of a circle centred at `p` that touches the base edge (the perpendicular distance to a
/// line, or |dist - R| to a circle).
pub fn tangent_radius_to_edge(dc: &DrawCtx, si: usize, eref: EdgeRef, p: Point2) -> f64 {
    match eref {
        EdgeRef::Line { a, b } => match (sketch_pt(dc.project, si, a), sketch_pt(dc.project, si, b)) {
            (Some(pa), Some(pb)) => {
                let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                ((dx * (p.y - pa.y) - dy * (p.x - pa.x)) / len).abs()
            }
            _ => 0.0,
        },
        EdgeRef::Circle { center, r } => match sketch_pt(dc.project, si, center) {
            Some(pc) => (((p.x - pc.x).powi(2) + (p.y - pc.y).powi(2)).sqrt() - r).abs(),
            None => 0.0,
        },
    }
}

/// The world (origin, normal) of the plane the command has picked — for the PREVIEW, without creating a
/// datum (unlike `resolve_mirror_plane`). For a face, the local centroid and normal are taken into
/// world space through the body's display transform.
pub fn mirror_plane_world(dc: &DrawCtx, sp: &qymcad_core::feature::SketchPlane) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, SketchPlane};
    match sp {
        // the side a plane offset from it goes to, as the plane made goes (`BasePlane::offset_direction`)
        SketchPlane::World(b) => Some(([0.0, 0.0, 0.0], b.offset_direction())),
        SketchPlane::Datum(id) => dc.project.planes.iter().find(|p| p.id == *id).map(|p| (p.origin, p.normal)),
        SketchPlane::Face(body, key) => {
            let ctx = current_ctx_id(dc.active_path, dc.project);
            let wt = dc.project.body_display_transform(*body, ctx);
            let o = apply12(&wt, key.centroid);
            let z = apply12(&wt, [0.0, 0.0, 0.0]);
            let n = [apply12(&wt, key.normal)[0] - z[0], apply12(&wt, key.normal)[1] - z[1], apply12(&wt, key.normal)[2] - z[2]];
            Some((o, n))
        }
    }
}

/// The WORLD frame of a candidate sketch plane, built the same way `Project::sketch_frame` builds it
/// (World gives `b.frame()`, Datum and Face give `world_aligned`), so that the (u, v) snapping is
/// computed in THE SAME axes the origin is later lifted in. For a face it is the local base frame
/// carried into world space by `body_display_transform`.
pub fn world_frame_of_plane(dc: &DrawCtx, sp: &qymcad_core::feature::SketchPlane) -> Option<qymcad_core::feature::PlaneFrame> {
    use qymcad_core::feature::{PlaneFrame, SketchPlane};
    match sp {
        SketchPlane::World(b) => Some(b.frame()),
        SketchPlane::Datum(id) => dc.project.planes.iter().find(|p| p.id == *id).map(|p| PlaneFrame::world_aligned(p.origin, p.normal, p.rot_deg)),
        SketchPlane::Face(body, key) => {
            let (c, n) = dc.project.resolve_face(*body, key);
            let wt = dc.project.body_display_transform(*body, current_ctx_id(dc.active_path, dc.project));
            Some(PlaneFrame::world_aligned(c, n, 0.0).transformed(&wt))
        }
    }
}

/// The contour under the cursor (for the hover highlight), within about 8 px.
pub fn hovered_contour(cursor: Option<Point2>, project: &Project, view: View2d) -> Option<usize> {
    let cur = cursor?;
    let thresh = 8.0 / view.scale as f64;
    let mut best: Option<(usize, f64)> = None;
    for (i, c) in project.contours.iter().enumerate() {
        let d = dist_to_contour(c, cur);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.filter(|(_, d)| *d <= thresh).map(|(i, _)| i)
}

/// THE NORMALS A BODY IS LIT BY in smooth shading. A body from the kernel is tessellated face by face, so a vertex is
/// shared only within a face and its averaged normal is right (`Mesh::vertex_normals`). A piece of a mesh from a file
/// shares its vertices across its sharp edges too, and is lit at the corners of its triangles, smoothed only where the
/// surface turns by less than `CREASE_DEG` (`Mesh::corner_normals`).
pub enum Normals {
    AtVertex(Vec<[f64; 3]>),
    AtCorner(Vec<[f64; 3]>),
}

impl Normals {
    /// The normal at corner `k` of triangle `t`, whose vertex is `v`.
    pub fn at(&self, t: usize, k: usize, v: u32) -> [f64; 3] {
        match self {
            Normals::AtVertex(n) => n[v as usize],
            Normals::AtCorner(n) => n[3 * t + k],
        }
    }
}

/// The turn between two triangles of a mesh piece across which its light is still smoothed.
pub const CREASE_DEG: f64 = 30.0;

/// Refresh the cache of smoothed normals (`Normals`) for the current `geom_rev` (lazily, indexed as
/// `project.meshes` is). The heavy pass over the triangles is done ONCE per change of geometry rather
/// than every frame.
pub fn ensure_vertex_normals(cache: &Caches, project: &Project, regen: &Rebuilding) {
    let mut c = cache.norm.borrow_mut();
    if c.rev == regen.geom_rev && c.value.len() == project.bodies.len() {
        return;
    }
    let pieces: std::collections::HashSet<qymcad_core::model::Id> = project
        .timeline
        .iter()
        .filter_map(|n| match n.kind {
            qymcad_core::feature::FeatureKind::MeshPiece { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    c.value = project.bodies.iter().map(|b| if pieces.contains(&b.id) { Normals::AtCorner(b.mesh.corner_normals(CREASE_DEG)) } else { Normals::AtVertex(b.mesh.vertex_normals()) }).collect();
    c.rev = regen.geom_rev;
}

/// The scene key for the GPU buffer: the same as in `view_key`, BUT without the camera, the size or the
/// dragging flag (the vertices and their colours do not depend on the camera). When it changes, the
/// vertex buffer is uploaded anew.
pub fn gpu_scene_key(pn: &Painting) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    view_rev(pn.regen).hash(&mut h);
    (pn.set.shading == Shading::Smooth).hash(&mut h); // smooth carries a vertex normal, flat carries none - a different buffer
                                                      // THE COLOURS ARE NOT HERE ANY MORE, and neither is the highlight. Both live in the look table beside the
                                                      // vertices: changing a scheme or moving the pointer over the model rewrites two numbers per body instead
                                                      // of the whole scene. Measured on the reference engine: 739 MB re-uploaded for a change of highlight.
    if let Some((o, n)) = section_eff(pn.section) {
        for v in o.iter().chain(n.iter()) {
            v.to_bits().hash(&mut h); // the section moved or turned, so the buffer is rebuilt
        }
    }
    // WHICH BODIES ARE DRAWN AND WHERE EACH ONE STANDS - the two things the vertices are made of.
    //
    // The context used to be hashed as itself, together with the component transforms and the visibility
    // ticks. Stepping into a subassembly changed the key even when not a single vertex moved, and the whole
    // scene went to the card again: 739 MB on the reference engine, which is what "cannot enter the
    // assembly, it hangs" was. Membership and placement are asked of the same function that builds the
    // scene, so the key cannot drift away from what is in the buffer; it costs one composed transform per
    // visible body (139 of them on the reference assembly, 2000 on the engine).
    for m in visible_mesh_items(pn) {
        (m.index as u32).hash(&mut h);
        for v in m.world {
            v.to_bits().hash(&mut h); // the gizmo's drag preview is already inside this transform
        }
    }
    h.finish()
}

/// The key of the 3D render cache: the view (camera, size, pixels per point), the geometry revision and
/// the visibility.
pub fn view_key(pn: &Painting, rect: Rect, ppp: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    pn.cam.yaw.to_bits().hash(&mut h);
    pn.cam.pitch.to_bits().hash(&mut h);
    (pn.cam.scale as f64).to_bits().hash(&mut h);
    for t in pn.cam.target {
        t.to_bits().hash(&mut h);
    }
    ((rect.width() * ppp) as u32).hash(&mut h);
    ((rect.height() * ppp) as u32).hash(&mut h);
    pn.view_dragging.hash(&mut h); // the low-resolution and full-resolution pictures are separate cache entries
    pn.regen.geom_rev.hash(&mut h);
    // the components' placements: moving a part in an assembly changes the picture, and the raster cache must see it
    for c in &pn.project.components {
        for v in c.transform {
            v.to_bits().hash(&mut h);
        }
        c.visible.hash(&mut h); // visibility is hierarchical (`body_shown` follows the chain of ticks), so it goes into the key
    }
    (pn.set.projection == Projection::Perspective).hash(&mut h); // perspective against orthographic changes the projection, so it goes into the raster cache key
    (pn.set.shading == Shading::Smooth).hash(&mut h); // smooth against flat shading changes the vertex colours, so it goes in too
    pn.scheme.pal.fingerprint().hash(&mut h); // the raster is already coloured by the scheme: change the scheme and it is drawn anew
                                              // The selection highlight is painted into the texture, so the WHOLE set of highlighted bodies goes into the
                                              // key - the one both drawing paths colour by (a body lights itself, a component its whole subtree), in a
                                              // fixed order, the set being a hash set. Without it a click on a part in the tree lit nothing on the
                                              // software picture until the camera moved.
    let mut lit: Vec<usize> = highlight_mesh_set(pn.project, &pn.sel).into_iter().collect();
    lit.sort_unstable();
    lit.hash(&mut h);
    // a body switched off by its own tick is not drawn
    for (i, v) in pn.project.bodies.iter().map(|b| b.visible).enumerate() {
        if v {
            (i as u32).hash(&mut h);
        }
    }
    // in-context mode and the active context change which bodies are visible and which are ghosted, so the raster cache must see them
    pn.win.context.hash(&mut h);
    current_ctx_id(pn.active_path, pn.project).hash(&mut h);
    // EDITING A FEATURE CHANGES WHICH BODIES ARE DRAWN — AND THE KEY MUST SEE THAT.
    //
    // On entering the editing of a modifier, the program shows the state BEFORE it: the feature's result
    // and its whole descendant chain are hidden (`edit_hidden_bodies`), and the consumed source is
    // shown. On leaving, it puts everything back. Neither went into the key, and the picture changed
    // only by accident: if the camera happened to move as well, or `geom_rev` happened to tick.
    //
    // Reported on a freshly opened document: enter a fillet, press Esc — the fillet and the chamfer
    // under it are gone, and the features are not in error, they simply are not drawn. The model was
    // intact the whole time; the raster was left over from the editing mode: entering it had been
    // redrawn by a heavy rebuild (which ticked `geom_rev`), and on leaving there was nothing left to
    // change the key.
    pn.cmd.edit.hash(&mut h);
    h.finish()
}

/// The slot's current contour (for highlighting in the half-sketcher). 0 means the first suitable
/// contour of the sketch, chosen automatically.
pub fn slot_current_cid(loft: &LoftParams, project: &Project, sweep: SweepParams, slot: ContourSlot) -> Id {
    let resolve = |sid: Id, cid: Id, path: bool| -> Id {
        if cid != 0 {
            return cid;
        }
        let cands = if path { project.sweep_path_contours(sid) } else { project.sweep_profile_contours(sid) };
        cands.first().copied().unwrap_or(0)
    };
    match slot {
        ContourSlot::SweepProfile => resolve(sweep.prof_sid, sweep.prof_cid, false),
        ContourSlot::SweepPath => resolve(sweep.path_sid, sweep.path_cid, true),
        ContourSlot::LoftSection(i) => {
            let sid = loft.sids.get(i).copied().unwrap_or(0);
            resolve(sid, loft.cids.get(i).copied().unwrap_or(0), false)
        }
    }
}

/// The origin of a body's gizmo (the centre of the mesh's bounding box, in the Part's display frame,
/// which is the local one) and the length of an axis (about 60 px). During a drag the origin is fixed
/// (`body_giz_drag.1`); otherwise it is the current centre of the box.
pub fn body_gizmo_geometry(body_giz: &BodyGizmo, cam: Cam3, project: &Project, set: &Settings, mi: usize) -> ([f64; 3], f64) {
    let o = if let Some((dmi, org, _)) = body_giz.drag {
        if dmi == mi {
            // THE GIZMO FOLLOWS the body during a drag (the preview moves the body by the same
            // accumulator). For a move the origin travels along; for a rotation
            // apply12(rot_about_org, org) = org, so the centre stays put. Otherwise the gizmo would
            // stand still while the body moved, and would jump to the body on release.
            match body_giz_accum(body_giz, set, body_giz.snap) {
                Some(acc) => qymcad_core::feature::apply12(&acc, org),
                None => org,
            }
        } else {
            mesh_center(project, mi)
        }
    } else {
        mesh_center(project, mi)
    };
    (o, 60.0 / cam.scale as f64)
}

/// The gizmo's origin (in the display frame, that is, the component's transform) and the length of an
/// axis (about 60 px). During a drag the origin is FIXED, as it is for a body, so the axis projection
/// does not float along with the component.
pub fn gizmo_geometry(cam: Cam3, comp_giz: CompGizmo, project: &Project, comp: Id) -> ([f64; 3], f64) {
    let o = match comp_giz.drag {
        Some((dc, _, origin, _)) if dc == comp => origin,
        _ => {
            let t = project.component_transform(comp);
            [t[3], t[7], t[11]]
        }
    };
    (o, 60.0 / cam.scale as f64)
}

/// The component the placement gizmo applies to: a component selected in the tree whose parent is the
/// current context (so its transform is the display frame), in the Assembly workbench, and not the root.
pub fn gizmo_component(active_path: &[Id], project: &Project, sel: Sel, workbench: Workbench) -> Option<Id> {
    if !matches!(workbench, Workbench::Assembly) {
        return None;
    }
    if let Sel::Component(ci) = sel {
        let c = project.components.get(ci)?;
        if c.id != project.root && c.parent == Some(current_ctx_id(active_path, project)) {
            return Some(c.id);
        }
    }
    None
}

/// A `PatternKind` built from the parameter fields (for a circular one the centre is the point picked,
/// or the centroid of the selection).
pub fn current_pattern_kind(armed: &Armed, pat: PatternTool, project: &Project, sk_pat: SketchPattern, si: usize, eids: &[Id]) -> qymcad_core::model::PatternKind {
    use qymcad_core::model::PatternKind;
    if armed.pat_op() == 2 {
        // the centre in order of priority: the one explicitly clicked, then the stored one (while editing), then the selection's centroid
        let (cx, cy) = if let Some(c) = pat.center {
            (c.x, c.y)
        } else if let Some(PatternKind::Circular { cx, cy, .. }) = pat.edit.and_then(|pi| project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.kind)) {
            (cx, cy)
        } else {
            project.entities_centroid(si, eids)
        };
        PatternKind::Circular { cx, cy, count: sk_pat.count, total_deg: sk_pat.angle }
    } else {
        PatternKind::Linear { dx: sk_pat.dx, dy: sk_pat.dy, count: sk_pat.count, dx2: sk_pat.dx2, dy2: sk_pat.dy2, count2: sk_pat.count2 }
    }
}

/// A PROPER clip of a triangle by the section plane (Sutherland-Hodgman): the visible side (dist <= 0)
/// is cut EXACTLY along the plane, giving 0 to 2 triangles. With no section, the original triangle
/// comes back. A resulting vertex is (position, source weights [w0, w1, w2]) — for interpolating the
/// colour on the GPU.
pub fn section_clip_tri(section: &SectionTool, v: [[f64; 3]; 3]) -> smallvec_tris::ClipTris {
    use smallvec_tris::*;
    let Some((o, n)) = section_eff(section) else {
        return ClipTris::whole();
    };
    let d = [
        (v[0][0] - o[0]) * n[0] + (v[0][1] - o[1]) * n[1] + (v[0][2] - o[2]) * n[2],
        (v[1][0] - o[0]) * n[0] + (v[1][1] - o[1]) * n[1] + (v[1][2] - o[2]) * n[2],
        (v[2][0] - o[0]) * n[0] + (v[2][1] - o[1]) * n[1] + (v[2][2] - o[2]) * n[2],
    ];
    clip_by_dists(v, d)
}

/// The axis of the active sketch-based command (extrude, cut and so on): the centroid of the
/// profiles in world space, the normal, and the length.
pub fn feat_cmd_axis(cmd: &FeatCommand, gsel: &GeomSelection, project: &Project) -> Option<([f64; 3], [f64; 3], f64)> {
    let si = cmd.sketch?;
    let f = project.sketch_frame(si)?;
    let mut polys: Vec<Vec<f64>> = Vec::new();
    for cid in &gsel.profiles {
        if let Some(xy) = project.contour_profile_xy(*cid) {
            polys.push(xy);
        }
    }
    if polys.is_empty() {
        if let Some((_, xy)) = project.sketch_profile_xy(si) {
            polys.push(xy);
        }
    }
    let (mut sx, mut sy, mut cnt) = (0.0_f64, 0.0_f64, 0.0_f64);
    for xy in &polys {
        for k in 0..xy.len() / 2 {
            sx += xy[2 * k];
            sy += xy[2 * k + 1];
            cnt += 1.0;
        }
    }
    if cnt == 0.0 {
        return None;
    }
    let base = f.lift(Point2::new(sx / cnt, sy / cnt));
    Some(([base.x, base.y, base.z], f.normal(), cmd_val(cmd, "height")))
}

/// ONE ARROW OF A COMMAND AT ITS PROFILE, as it is drawn and as it is grabbed: one geometry for both, so the handle is
/// where the eye sees it.
pub struct CmdArrow {
    pub base: [f64; 3],
    /// The unit direction the arrow points in - the one the body grows in.
    pub dir: [f64; 3],
    pub len: f64,
    /// The field of the command it drives.
    pub key: &'static str,
}

impl CmdArrow {
    pub fn tip(&self) -> [f64; 3] {
        [self.base[0] + self.dir[0] * self.len, self.base[1] + self.dir[1] * self.len, self.base[2] + self.dir[2] * self.len]
    }
}

/// THE ARROWS OF THE COMMAND IN HAND: the first side along the direction in force (a flip turns it against the sketch's
/// normal), and with two sides a second arrow the other way, driving the second value. Reported behaviour: after a
/// flip the arrow could not be grabbed - it was drawn turned and grabbed where it stood before the flip - and two
/// sides showed no second arrow.
pub fn feat_cmd_arrows(cmd: &FeatCommand, gsel: &GeomSelection, project: &Project, flip: bool) -> Vec<CmdArrow> {
    let Some((base, n, h)) = feat_cmd_axis(cmd, gsel, project) else { return Vec::new() };
    let dir = if flip { [-n[0], -n[1], -n[2]] } else { n };
    let mut out = vec![CmdArrow { base, dir, len: h, key: "height" }];
    if cmd.extent.two_sided() {
        out.push(CmdArrow { base, dir: [-dir[0], -dir[1], -dir[2]], len: cmd_val(cmd, "down"), key: "down" });
    }
    out
}

/// A PRESS ON AN ARROW OF THE COMMAND takes it: the one whose tip is within 14 px of the pointer, the nearest first.
/// The field of a value lets go of the keyboard, so what was typed in it is taken as it stands and the arrow drives
/// the value from there: a drag is not a click, and the field kept the caret and wrote its own text back over the
/// arrow on every frame. Answers whether an arrow was taken.
pub fn grab_cmd_arrow(ctx: &egui::Context, cmd: &mut FeatCommand, gsel: &GeomSelection, project: &Project, flip: bool, scr: &Screen, at: Pos2) -> bool {
    let arrows = feat_cmd_arrows(cmd, gsel, project, flip);
    let hit = arrows.iter().map(|a| (a.key, scr.at(a.tip()).0.distance(at))).filter(|(_, d)| *d <= 14.0).min_by(|a, b| a.1.total_cmp(&b.1));
    cmd.drag = hit.map(|(k, _)| k);
    if cmd.drag.is_some() {
        ctx.memory_mut(|m| m.stop_text_input());
    }
    cmd.drag.is_some()
}

/// A PRESS ON THE ARROW AT A FACE takes the value of field `key` as it stands, and the field lets go of the keyboard -
/// for the same reason as with [`grab_cmd_arrow`]: what was typed there wrote itself back over the arrow every frame.
pub fn take_arrow_value(ctx: &egui::Context, cmd: &FeatCommand, key: &str) -> f64 {
    ctx.memory_mut(|m| m.stop_text_input());
    cmd_val(cmd, key)
}

/// THE ARROW TAKEN FOLLOWS THE POINTER: the pointer is projected onto the arrow's line on the screen. The first side
/// pulls both ways - back past the profile it turns the direction (flip) and the value is the distance; the second
/// side only lengthens and shortens.
pub fn drag_cmd_arrow(cmd: &mut FeatCommand, gsel: &GeomSelection, project: &Project, feat: &mut FeatTarget, scr: &Screen, cur: Pos2) {
    let Some(key) = cmd.drag else { return };
    let Some((base, n, _)) = feat_cmd_axis(cmd, gsel, project) else { return };
    let dir = if key == "down" {
        if feat.flip {
            n
        } else {
            [-n[0], -n[1], -n[2]]
        }
    } else {
        n
    };
    let (s0, s1) = (scr.at(base).0, scr.at([base[0] + dir[0], base[1] + dir[1], base[2] + dir[2]]).0);
    let pd = s1 - s0;
    let denom = (pd.x * pd.x + pd.y * pd.y) as f64;
    if denom <= 1e-6 {
        return;
    }
    let t = ((cur.x - s0.x) * pd.x + (cur.y - s0.y) * pd.y) as f64 / denom;
    if key == "height" {
        feat.set_flip(t < 0.0);
    }
    if let Some(p) = cmd.params.iter_mut().find(|p| p.key == key) {
        p.val = if key == "height" { t.abs() } else { t }.max(0.1);
        p.txt = format!("{:.2}", p.val);
    }
}

/// The index of the sketch that defines the viewport's 2D projection: while editing it is the open
/// sketch, otherwise (in the half-sketcher of a Part command) it is the command's profile sketch, so
/// that the body is projected IN THE PLANE of the drawing rather than seen from above.
pub fn active_2d_sketch(armed: &Armed, cmd: &FeatCommand, project: &Project, sel: Sel, sketch_ses: SketchSession) -> Option<usize> {
    if let Some(si) = edit_si(project, &sketch_ses) {
        return Some(si);
    }
    if armed.commanding() {
        if let Some(si) = cmd.sketch {
            return Some(si);
        }
        if let Sel::Sketch(si) = sel {
            return Some(si);
        }
    }
    None
}

/// The body under the gizmo in a Part: the selected body of the active part (`Sel::Mesh`, or
/// `Sel::Feature` carrying a body). Returns (the body's Id, the mesh index). Only in the Part workbench.
pub fn body_gizmo_target(bv: BodyView, sel: Sel) -> Option<(Id, usize)> {
    // A PART IS ONE BODY, so the Part workbench does NOT offer a gizmo for an INDIVIDUAL body. Dragging
    // a body would bake in a Move feature and tear the body away from its sketch. A part's body is moved
    // in an assembly by the COMPONENT gizmo (its placement). The body gizmo is kept ONLY for a "free"
    // imported body (an STL with no owning component) — that one cannot be moved by a component gizmo.
    if bv.armed.commanding() {
        return None; // during a command (extrude, cut and so on) the body gizmo stays out of the way
    }
    let body = visible_lineage_body(bv.project, selected_body(bv.project, &sel)?);
    // a body that BELONGS to a part (it has an owner) gets NO body gizmo, neither in the part nor in the
    // assembly: a part is a body and is moved by its component. The body gizmo is only for an imported
    // mesh with no owner.
    if bv.project.body_owner(body).is_some() {
        return None;
    }
    let mi = bv.project.mesh_index(body)?;
    body_shown(bv, mi).then_some((body, mi))
}

/// The frame's projection parameters (`ProjParams`): the single source for the CPU (`depth_ndc`) and for the GPU
/// (`CamRaw`). The orthographic range holds the whole scene round the target, wherever the target is.
pub fn proj_params(pn: &Painting, rect: Rect, scene_key: u64) -> ProjParams {
    let half = (rect.width().max(rect.height()) * 0.5) as f64;
    let scale = (pn.cam.scale as f64).max(1e-4);
    let inv_d = persp_inv_d_eye(&pn.cam, pn.set, rect.height() * 0.5);
    let z_near = if inv_d > 0.0 { 1e-3 / inv_d } else { 0.0 };
    // the depth of what is on screen, as the orthographic range was, and at least the scene round the target
    let mut depth_half = (half / scale) * 50.0 + 1000.0;
    if let Some(Sphere { centre: c, radius: r }) = scene_sphere_cached(pn, scene_key) {
        let (_, _, fwd) = pn.cam.basis();
        let dc = v_dot(v_sub(c, pn.cam.target), fwd);
        depth_half = depth_half.max((dc.abs() + r) * 1.05);
    }
    ProjParams { inv_d_eye: inv_d, z_near, depth_half }
}

pub fn face_arrow_geometry(pn: &Painting) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    let key = face_arrow_key(pn.armed)?;
    let (o, n) = cmd_arrow_ref(pn)?;
    let d = cmd_val(pn.cmd, key);
    // AT ZERO THE LENGTH COMES FROM THE SIZE OF THE PART. Otherwise a zero value leaves the handle
    // invisible, and there is nothing to drag at exactly the moment one needs to.
    let span = op_target_body(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, pn.sel)
        .or(pn.gsel.faces_body)
        .and_then(|b| pn.project.mesh_index(b))
        .and_then(|mi| pn.project.bodies[mi].mesh.bounds())
        .map(|b| (b.max.x - b.min.x).abs().max(1.0) * 0.25)
        .unwrap_or(5.0);
    let l = if d.abs() > 1e-6 { d } else { span };
    Some((o, [o[0] + n[0] * l, o[1] + n[1] * l, o[2] + n[2] * l], n))
}

/// The geometry of the section GIZMO: the centre of the quad on the plane, u, v, the half-size, and the arrow's tip.
/// THE SECTION'S HANDLE, as it is drawn: a square lying in the cutting plane with an arrow out of it.
///
/// It was `Option<([f64; 3], [f64; 3], [f64; 3], f64, [f64; 3])>` - three unnamed triples, a number and a
/// fourth triple. One caller reads it as `(_, _, _, _, tip)`.
pub struct SectionGizmo {
    /// The scene's centre projected onto the cutting plane - where the handle sits.
    pub centre: [f64; 3],
    /// The two axes of the square, in the plane.
    pub u: [f64; 3],
    pub v: [f64; 3],
    /// Half the square's side, from the scene's diagonal.
    pub half: f64,
    /// The end of the arrow along the normal - what is grabbed to move the plane.
    pub tip: [f64; 3],
}

pub fn section_gizmo_geom(pn: &Painting) -> Option<SectionGizmo> {
    let (o, n) = section_eff(pn.section)?;
    // the bounding box of the visible scene, in world space
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
        if let Some(b) = mesh.bounds() {
            for c in [[b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]] {
                let w = qymcad_core::feature::apply12(&wt, c);
                for k in 0..3 {
                    lo[k] = lo[k].min(w[k]);
                    hi[k] = hi[k].max(w[k]);
                }
            }
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let d = (c[0] - o[0]) * n[0] + (c[1] - o[1]) * n[1] + (c[2] - o[2]) * n[2];
    let cp = [c[0] - d * n[0], c[1] - d * n[1], c[2] - d * n[2]]; // the scene's centre projected onto the plane
    let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt().max(1.0);
    let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = v_norm(v_cross(a, n));
    let v = v_cross(n, u);
    let half = diag * 0.55;
    let tip = [cp[0] + n[0] * diag * 0.35, cp[1] + n[1] * diag * 0.35, cp[2] + n[2] * diag * 0.35];
    Some(SectionGizmo { centre: cp, u, v, half, tip })
}

/// The live body of a lineage — a delegate to the core (`Project::live_body`). The FORWARD walk lives
/// there in a single definition: a copy in the interface layer would drift from the core silently.
pub fn visible_lineage_body(project: &qymcad_core::model::Project, body: Id) -> Id {
    project.live_body(body)
}

/// The centre of mesh `mi`'s bounding box (world, or the Part's local frame). [0, 0, 0] when the mesh is empty.
pub fn mesh_center(project: &qymcad_core::model::Project, mi: usize) -> [f64; 3] {
    project.bodies.get(mi).and_then(|b| b.mesh.bounds()).map(|b| [(b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0]).unwrap_or([0.0, 0.0, 0.0])
}

/// THE ARROW ON THE SELECTED FACE (the "push the face" handle): its origin is the centre of the face,
/// its direction is the face's normal, and its length is the current offset (at least visible enough).
/// Returns the origin, the tip and the axis length in world space.
/// WHAT THE HANDLE DRAGS: the name of a field of the active command. One handle serves every command
/// whose number has ONE direction — pushing a face, thickening, shelling, splitting a body. Giving each
/// tool its own handle would spread one and the same behaviour across copies, as the popups once were.
///
/// FILLETS, CHAMFERS AND FACE SPLITS GET NO HANDLE, and that is not an unfinished corner. There one
/// selects a batch of elements pointing in different directions: an arrow on one of them would show a
/// direction the operation does not have — that is, it would lie.
pub fn face_arrow_key(armed: &Armed) -> Option<&'static str> {
    match armed.cmd_kind() {
        20 | 25 => Some("dist"),
        6 | 28 => Some("thickness"),
        27 => Some("offset"),
        _ => None,
    }
}

pub fn current_body(dc: &DrawCtx) -> Option<Id> {
    // THE CURRENT part: the last unconsumed body of THIS context (not the globally last one — otherwise
    // a cut in one part would catch hold of another part's body, and the second part appeared to
    // vanish). The logic lives in the core, under test.
    // A FALLBACK ON THE DOCUMENT'S ACTIVE COMPONENT: the GUI navigation (`active_path`) may stand at the
    // root when the part was never "entered" by a double click — the commands must still see the active
    // part's body, otherwise the part looks empty and new bodies start multiplying.
    dc.project.active_body(current_ctx_id(dc.active_path, dc.project)).or_else(|| dc.project.active_body(dc.project.current_ctx()))
}

/// A loop of corners round a hole of a body made of a mesh that did not close, in the body's own coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenBorder {
    pub corners: Vec<[f64; 3]>,
}

/// A sphere in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub centre: [f64; 3],
    pub radius: f64,
}

/// A box in world coordinates, along the axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// The world bounding sphere of the visible scene, taken from `visible_mesh_items`
/// (exactly what gets drawn). Cached by the scene key: recomputed only when the scene changes, not
/// every frame.
pub fn scene_sphere_cached(pn: &Painting, scene_key: u64) -> Option<Sphere> {
    let cached = pn.cache.bounds.get();
    if cached.rev == scene_key {
        return cached.value;
    }
    let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut any = false;
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
        let ident = qymcad_core::feature::is_identity12(&wt);
        for v in &mesh.verts {
            let p = if ident { [v.x, v.y, v.z] } else { qymcad_core::feature::apply12(&wt, [v.x, v.y, v.z]) };
            any = true;
            for a in 0..3 {
                mn[a] = mn[a].min(p[a]);
                mx[a] = mx[a].max(p[a]);
            }
        }
    }
    let sphere = any.then(|| {
        let c = [(mn[0] + mx[0]) * 0.5, (mn[1] + mx[1]) * 0.5, (mn[2] + mx[2]) * 0.5];
        let r = (((mx[0] - mn[0]).powi(2) + (mx[1] - mn[1]).powi(2) + (mx[2] - mn[2]).powi(2)).sqrt() * 0.5).max(1e-3);
        Sphere { centre: c, radius: r }
    });
    pn.cache.bounds.set(Cached { rev: scene_key, value: sphere });
    sphere
}

/// WHERE THE HANDLE STARTS AND WHERE IT POINTS: the point it acts on and a unit direction.
///
/// For face commands that is the centroid of the selected face and its normal; for splits it is the
/// origin and normal of the chosen plane. Everything after that is shared: the drawing, the grabbing
/// and the dragging.
pub fn cmd_arrow_ref(pn: &Painting) -> Option<([f64; 3], [f64; 3])> {
    match pn.armed.cmd_kind() {
        6 | 25 | 28 => {
            let mi = pn.project.mesh_index(pn.gsel.faces_body?)?;
            let f = pn.project.bodies.get(mi)?.faces.iter().find(|f| pn.gsel.faces.contains(&f.id))?;
            Some(([f.centroid.x, f.centroid.y, f.centroid.z], f.normal))
        }
        27 => pn.split.plane.as_ref().and_then(|sp| mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp)),
        // a datum plane: from the reference picked, along the side its offset goes to (`BasePlane::offset_direction`)
        20 => pn.datum.plane_pick.as_ref().and_then(|sp| mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp)),
        _ => None,
    }
}
