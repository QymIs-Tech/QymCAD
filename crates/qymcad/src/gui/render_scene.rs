//! THE SCENE HANDED TO THE CARD: vertices for the GPU, component thumbnails, the caps of a section, the
//! edges of the selected body.
//!
//! Split out of `render.rs`: this side prepares what is to be drawn, the other side draws it.

use super::*;

// THE SCENE AND THE DRAWING CACHES: assembling the scene for the GPU, the component preview, the section caps
// for a frame, refreshing the cache of the selected body's edges.
impl App {
    /// Assemble the scene's vertices for the GPU: world triangles + the face normal (for culling in the
    /// fragment shader) + a shaded colour AT EVERY VERTEX. Smooth shading (Gouraud) computes the colour from the
    /// vertex's SMOOTHED normal and lets the GPU interpolate across the triangle (the `color` varying); sharp
    /// edges stay sharp (the mesh topology is split by face). Flat mode puts the face normal into all three.
    /// Returns `(vertices, opaque_count)`: the opaque ones FIRST, in `[0..opaque_count)`, and the ghosts
    /// (alpha<255) AFTER (two passes: opaque writes depth, then transparent tests without writing and alpha-blends).
    pub(super) fn gpu_scene(&self) -> (Vec<crate::viewport_gpu::GpuVert>, u32) {
        let light = v_norm([0.35, 0.5, 0.78]);
        let smooth = self.set.smooth_shading;
        if smooth {
            self.ensure_vertex_normals();
        }
        let ncache = self.cache.norm.borrow();
        let items = self.visible_mesh_items();
        let (mut opaque, mut transp) = (Vec::new(), Vec::new());
        // EVERY BODY IS COMPUTED AS ITS OWN BLOCK AND CACHED.
        //
        // MEASURED ON A REAL ASSEMBLY (138 bodies, 463,878 vertices, a release build): rebuilding the scene
        // buffer took 30-48 ms, and it was done on EVERY frame while a part was being dragged - because the
        // position is baked into the vertices and the buffer key is tied to the geometry revision that the drag
        // keeps changing. Reported behaviour: the part moves as if on elastic, and the joint lines are visibly
        // stretching - the glyph is drawn from live numbers while the body arrives a frame late.
        //
        // But a drag moves ONE body. So one is what has to be recomputed: a body's block depends on its mesh,
        // its position, its highlight and the shared display settings - and that is exactly the block's key.
        // The other 137 blocks are taken ready-made.
        let mut blocks = self.cache.scene_blocks.borrow_mut();
        let common = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            self.regen.geom_rev.hash(&mut h); // the SHAPE of the bodies; this body's position enters the key separately (`wt`)
            smooth.hash(&mut h);
            self.set.ghost_alpha.hash(&mut h);
            self.scheme.pal.fingerprint().hash(&mut h);
            // the section cuts the triangles IN THE WORLD, so the block depends on it too
            if let Some((o, n)) = self.section_eff() {
                for v in o.iter().chain(n.iter()) {
                    v.to_bits().hash(&mut h);
                }
            }
            h.finish()
        };
        let mut live: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut order: Vec<usize> = Vec::new();
        let mut stats = [0u32; 3]; // [rebuilt, shifted, taken ready-made]
        for (mi, hot, ghost, base, mesh, wt) in items {
            live.insert(mi);
            order.push(mi);
            // THE KEY IS ABOUT SHAPE AND APPEARANCE ONLY. The position lives separately (`SceneBlock::at`) and
            // a move does not invalidate the block: see `SceneBlock`.
            let shape = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                common.hash(&mut h);
                (hot, ghost, base).hash(&mut h);
                h.finish()
            };
            // IT MOVED, IT DID NOT CHANGE - ADD THE DIFFERENCE INSTEAD OF BUILDING IT AGAIN.
            //
            // ONLY a pure translation is carried over, and only with no section. A rotation changes the world
            // normals, and both the colour and the backface culling are computed from them; a section cuts the
            // triangles in the world. Either of those calls for an honest rebuild, and there is nothing to fake.
            //
            // The accumulated error: the vertices are f32 and each drag step is added to already shifted ones.
            // On a one-metre extent that is about 1e-4 mm per step, and it clears completely on the block's very
            // next rebuild (any change of highlight, shape, section or display settings).
            match blocks.get_mut(&mi) {
                Some(b) if b.shape == shape && b.at == wt => {
                    stats[2] += 1;
                    continue;
                }
                Some(b) if b.shape == shape && self.section.plane.is_none() && same_rotation12(&b.at, &wt) => {
                    stats[1] += 1;
                    let d = [(wt[3] - b.at[3]) as f32, (wt[7] - b.at[7]) as f32, (wt[11] - b.at[11]) as f32];
                    for v in b.opaque.iter_mut().chain(b.transp.iter_mut()) {
                        v.pos[0] += d[0];
                        v.pos[1] += d[1];
                        v.pos[2] += d[2];
                    }
                    b.at = wt;
                    continue;
                }
                _ => {}
            }
            let (mut opaque, mut transp) = (Vec::new(), Vec::new());
            let ident = qymcad_core::feature::is_identity12(&wt);
            // the smoothed local vertex normals of this body (when enabled and present in the cache)
            let vn = if smooth { ncache.1.get(mi) } else { None };
            for i in 0..mesh.tris.len() {
                let tri = mesh.tris[i];
                let pos_w = |vi: u32| {
                    let p = mesh.verts[vi as usize];
                    let a = [p.x, p.y, p.z];
                    if ident { a } else { qymcad_core::feature::apply12(&wt, a) }
                };
                let (a, b, c) = (pos_w(tri[0]), pos_w(tri[1]), pos_w(tri[2]));
                // THE SECTION: an HONEST clip of the triangle by the plane (a cut exactly along it, with no needles)
                let clip = self.section_clip_tri([a, b, c]);
                if !clip.whole && clip.verts.is_empty() {
                    continue; // wholly on the hidden side
                }
                // the FACE normal (in world) - for backface culling in the fragment shader (the silhouette goes by
                // the face, not by the smoothed normal, otherwise the culling wanders at an edge)
                let fn_w = v_norm(v_cross(v_sub(b, a), v_sub(c, a)));
                let nf = [fn_w[0] as f32, fn_w[1] as f32, fn_w[2] as f32];
                // the colour AT EVERY vertex: from the smoothed normal (Gouraud) or from the face normal (flat)
                let col_at = |vi: u32| -> [u8; 4] {
                    let nrm = match vn {
                        Some(list) => Self::rotate_normal(&wt, list[vi as usize]),
                        None => fn_w,
                    };
                    Self::shade_tri(&self.scheme.pal, self.set.ghost_alpha, hot, ghost, base, nrm, light).to_array()
                };
                let al = if ghost { self.set.ghost_alpha } else { 255 };
                let dst = if al < 255 { &mut transp } else { &mut opaque };
                let cols = [col_at(tri[0]), col_at(tri[1]), col_at(tri[2])];
                let mut push = |p: [f64; 3], w: [f64; 3]| {
                    let mix = |k: usize| (cols[0][k] as f64 * w[0] + cols[1][k] as f64 * w[1] + cols[2][k] as f64 * w[2]).round().clamp(0.0, 255.0) as u8;
                    dst.push(crate::viewport_gpu::GpuVert {
                        pos: [p[0] as f32, p[1] as f32, p[2] as f32],
                        nrm: nf,
                        color: u32::from_le_bytes([mix(0), mix(1), mix(2), cols[0][3]]),
                        _pad: 0,
                    });
                };
                if clip.whole {
                    for (p, w) in [(a, [1.0, 0.0, 0.0]), (b, [0.0, 1.0, 0.0]), (c, [0.0, 0.0, 1.0])] {
                        push(p, w);
                    }
                } else {
                    // a fan over the clipped polygon (3..4 vertices -> 1..2 triangles)
                    for k in 1..clip.verts.len().saturating_sub(1) {
                        for &vi in &[0, k, k + 1] {
                            let cv = clip.verts[vi];
                            push(cv.pos, cv.w);
                        }
                    }
                }
            }
            stats[0] += 1;
            blocks.insert(mi, super::SceneBlock { shape, at: wt, opaque, transp });
        }
        self.cache.scene_stats.set(stats);
        blocks.retain(|mi, _| live.contains(mi)); // bodies that are no longer visible hold no memory
        // THE ASSEMBLY ORDER IS THE ONE IT ALWAYS WAS: the display order of the bodies, not the order in the
        // hash map. For translucent bodies the order is visible to the eye (they blend), so it must not change.
        // THE SIZE IS KNOWN IN ADVANCE and should be asked for at once. The concatenation runs over 138 pieces
        // into an empty vector, that is, with a dozen and a half reallocations and copies of an ever-growing
        // buffer; at 463,878 vertices that is a noticeable share of the frame's cost, taken for nothing.
        opaque.reserve(order.iter().filter_map(|mi| blocks.get(mi)).map(|b| b.opaque.len()).sum());
        transp.reserve(order.iter().filter_map(|mi| blocks.get(mi)).map(|b| b.transp.len()).sum());
        for mi in &order {
            if let Some(b) = blocks.get(mi) {
                opaque.extend_from_slice(&b.opaque);
                transp.extend_from_slice(&b.transp);
            }
        }
        // THE SECTION CAPS: an amber fill, two-sided (the cut is visible from both sides)
        if self.section.plane.is_some() {
            let caps = self.section_caps_for_frame();
            let (col, coln) = (u32::from_le_bytes([224, 168, 92, 255]), u32::from_le_bytes([176, 128, 66, 255]));
            if let Some((_, n)) = self.section_eff() {
                let nf = [n[0] as f32, n[1] as f32, n[2] as f32];
                let nb = [-nf[0], -nf[1], -nf[2]];
                // THE CAP IS NUDGED A HAIR INTO THE CUT-AWAY SIDE (the bodies keep the half-space d <= 0, so
                // at d = +eps NOTHING occludes the cap - there is no material left there). It cannot lie exactly
                // in the plane: the thread turns run almost tangent to the cut, their clipped triangles stand in
                // the same plane and win the depth test against the cap - and the fill then disappears in patches
                // precisely in the threaded zone (reported: the bottom of the part filled, the thread empty,
                // while the cap itself covers 99.8% of the outline). The nudge is thousandths of the extent and
                // does not affect the geometry.
                let eps = caps
                    .iter()
                    .filter_map(|m| m.bounds())
                    .map(|b| (b.max.x - b.min.x).max(b.max.y - b.min.y).max(b.max.z - b.min.z))
                    .fold(0.0_f64, f64::max)
                    .max(1.0)
                    * 1.0e-3;
                let off = [n[0] * eps, n[1] * eps, n[2] * eps];
                for mesh in caps.iter() {
                    for t in 0..mesh.tris.len() {
                        let tri = mesh.triangle(t).map(|p| qymcad_core::geom::Point3::new(p.x + off[0], p.y + off[1], p.z + off[2]));
                        for (nrm, color, order) in [(nb, col, [0usize, 1, 2]), (nf, coln, [0, 2, 1])] {
                            for &k in &order {
                                let p = tri[k];
                                opaque.push(crate::viewport_gpu::GpuVert {
                                    pos: [p.x as f32, p.y as f32, p.z as f32],
                                    nrm,
                                    color,
                                    _pad: 0,
                                });
                            }
                        }
                    }
                }
            }
        }
        let opaque_count = opaque.len() as u32;
        opaque.append(&mut transp); // [opaque… | transparent…]
        (opaque, opaque_count)
    }

    /// A preview of component `cid`'s body: a self-contained 256x256 orthographic raster (the isometric view of
    /// the default camera), WITHOUT mutating the camera or visibility state. It renders only the subtree's bodies
    /// in their own frame. `None` means there are no bodies (nothing was built). Used for the product's `thumb.png`.
    pub(super) fn render_component_thumbnail(&self, cid: qymcad_core::model::Id) -> Option<egui::ColorImage> {
        use qymcad_core::feature::{apply12, is_identity12};
        const TS: usize = 256;
        let mut subtree: std::collections::HashSet<qymcad_core::model::Id> = self.project.descendants(cid).into_iter().collect();
        subtree.insert(cid);
        // the subtree's bodies + their transform RELATIVE TO cid (the part at its own origin)
        let mut items: Vec<(usize, [f64; 12])> = Vec::new();
        for mi in 0..self.project.bodies.len() {
            let Some(b) = self.project.mesh_id(mi) else { continue };
            if self.project.body_owner(b).map(|o| subtree.contains(&o)) != Some(true) {
                continue;
            }
            if self.project.bodies[mi].mesh.verts.is_empty() {
                continue;
            }
            items.push((mi, self.project.body_display_transform(b, cid)));
        }
        if items.is_empty() {
            return None;
        }
        // the subtree's world bbox
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for &(mi, wt) in &items {
            let ident = is_identity12(&wt);
            for v in &self.project.bodies[mi].mesh.verts {
                let p = if ident { [v.x, v.y, v.z] } else { apply12(&wt, [v.x, v.y, v.z]) };
                for a in 0..3 {
                    mn[a] = mn[a].min(p[a]);
                    mx[a] = mx[a].max(p[a]);
                }
            }
        }
        if !mn[0].is_finite() {
            return None;
        }
        let center = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
        let ext = (mx[0] - mn[0]).max(mx[1] - mn[1]).max(mx[2] - mn[2]).max(1e-3);
        let (right, up, fwd) = Cam3::default().basis(); // a fixed isometric view, independent of the current camera
        let light = v_norm([0.35, 0.5, 0.78]);
        let s = (TS as f64 * 0.42) / ext; // the scale that fits it into the frame
        let hc = TS as f64 / 2.0;
        let proj = |p: [f64; 3]| -> (f64, f64, f64) {
            let dv = [p[0] - center[0], p[1] - center[1], p[2] - center[2]];
            (hc + v_dot(dv, right) * s, hc - v_dot(dv, up) * s, v_dot(dv, fwd)) // (x, y, depth along the view)
        };
        let ef = |ux: f64, uy: f64, vx: f64, vy: f64, px: f64, py: f64| (vx - ux) * (py - uy) - (vy - uy) * (px - ux);
        let mut color = vec![self.scheme.pal.thumbnail_bg(); TS * TS]; // a dark background, as in the viewport
        let mut zbuf = vec![f64::INFINITY; TS * TS];
        for &(mi, wt) in &items {
            let mesh = &self.project.bodies[mi].mesh;
            let base = self.project.mesh_color(mi);
            let ident = is_identity12(&wt);
            let pw = |vi: u32| {
                let p = mesh.verts[vi as usize];
                let a = [p.x, p.y, p.z];
                if ident { a } else { apply12(&wt, a) }
            };
            for tri in &mesh.tris {
                let (a, b, cc) = (pw(tri[0]), pw(tri[1]), pw(tri[2]));
                let n = v_norm(v_cross(v_sub(b, a), v_sub(cc, a)));
                // THIS ONE STAYS ORTHOGRAPHIC: the thumbnail has a fixed isometric view of its own, unrelated to
                // the viewport camera, and one ray serves the whole frame.
                if v_dot(n, fwd) >= 0.0 {
                    continue; // the bodies are oriented outwards
                }
                let col = Self::shade_tri(&self.scheme.pal, self.set.ghost_alpha, false, false, base, n, light);
                let (ax, ay, az) = proj(a);
                let (bx, by, bz) = proj(b);
                let (cx, cy, cz) = proj(cc);
                let area = ef(ax, ay, bx, by, cx, cy);
                if area.abs() < 1e-9 {
                    continue;
                }
                let minx = ax.min(bx).min(cx).floor().max(0.0) as usize;
                let maxx = (ax.max(bx).max(cx).ceil() as usize).min(TS);
                let miny = ay.min(by).min(cy).floor().max(0.0) as usize;
                let maxy = (ay.max(by).max(cy).ceil() as usize).min(TS);
                for py in miny..maxy {
                    for px in minx..maxx {
                        let (fx, fy) = (px as f64 + 0.5, py as f64 + 0.5);
                        let (w0, w1, w2) = (ef(bx, by, cx, cy, fx, fy) / area, ef(cx, cy, ax, ay, fx, fy) / area, ef(ax, ay, bx, by, fx, fy) / area);
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let depth = w0 * az + w1 * bz + w2 * cz;
                        let idx = py * TS + px;
                        if depth < zbuf[idx] {
                            zbuf[idx] = depth;
                            color[idx] = col;
                        }
                    }
                }
            }
        }
        Some(egui::ColorImage { size: [TS, TS], pixels: color })
    }

    /// THE SECTION CAPS: an exact planar cut of every visible body, in world coordinates, cached by (plane,
    /// geom_rev, visible bodies).
    pub(super) fn section_caps_for_frame(&self) -> std::rc::Rc<Vec<qymcad_core::geom::Mesh>> {
        let Some((o, n)) = self.section_eff() else {
            return std::rc::Rc::new(Vec::new());
        };
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for v in o.iter().chain(n.iter()) {
                v.to_bits().hash(&mut h);
            }
            self.view_rev().hash(&mut h); // the section cuts IN THE WORLD, so it depends on the layout too
            for (mi, _, ghost, _, _, _) in self.visible_mesh_items() {
                (mi as u32, ghost).hash(&mut h);
            }
            h.finish()
        };
        {
            let c = self.cache.section_caps.borrow();
            if c.0 == key {
                return c.1.clone();
            }
        }
        // THE CAP IS COMPUTED FROM THE MESH rather than by a kernel boolean. Reported behaviour: no amber cap
        // and hollow bodies inside - no CAD cuts that way. A Common boolean needed a live B-rep, which is not
        // there right after opening from a bundle, will never be there for an imported STL, and cost minutes on
        // a thousand bodies. A mesh is ALWAYS there and cuts orders of magnitude faster, so the cap is now drawn
        // during a gizmo drag as well - the section looks like a closed body at any moment.
        let mut caps: Vec<qymcad_core::geom::Mesh> = Vec::new();
        for (mi, _, ghost, _, _, wt) in self.visible_mesh_items() {
            if ghost {
                continue; // ghosts get no cap
            }
            // A CHEAP REJECT by extent - the plane cuts a handful of bodies out of a thousand
            if !self.mesh_crosses_plane(mi, o, n) {
                continue;
            }
            let Some(mesh) = self.project.bodies.get(mi).map(|b| &b.mesh) else { continue };
            // the body's mesh is in its local frame, so the plane is carried there too (wt is rigid: p_l = R^T (p - t))
            let r = |v: [f64; 3]| [wt[0] * v[0] + wt[4] * v[1] + wt[8] * v[2], wt[1] * v[0] + wt[5] * v[1] + wt[9] * v[2], wt[2] * v[0] + wt[6] * v[1] + wt[10] * v[2]];
            let po = [o[0] - wt[3], o[1] - wt[7], o[2] - wt[11]];
            let (ol, nl) = (r(po), r(n));
            let tris = qymcad_core::geom::mesh_section_cap(mesh, ol, nl);
            if tris.is_empty() {
                continue;
            }
            let mut cap = qymcad_core::geom::Mesh::default();
            for t in tris {
                let base = cap.verts.len() as u32;
                cap.verts.extend(t);
                cap.tris.push([base, base + 1, base + 2]);
            }
            if !qymcad_core::feature::is_identity12(&wt) {
                cap.transform(&wt); // the cap into world coordinates
            }
            caps.push(cap);
        }
        let rc = std::rc::Rc::new(caps);
        *self.cache.section_caps.borrow_mut() = (key, rc.clone());
        rc
    }

    /// WHETHER A LIVE B-REP IS NEEDED RIGHT NOW - as one list, not as a condition scattered across places.
    ///
    /// A project opened from a bundle carries no live B-rep: it is built ON DEMAND. Whoever does not state the
    /// demand silently gets NOTHING, and that looks like "the tool does not work". And so it was: a joint anchor
    /// on an edge found nothing, a sketch on a face did not show that face's outline, and a thread answered a
    /// click on a cylinder with "missed - click a cylindrical face". The list must be a single one, otherwise the
    /// next edge-based tool will forget about it all over again.
    fn needs_live_brep(&self) -> bool {
        // chamfer (4), fillet (5) and thread (24) work by edges and by face axes
        if matches!(self.cmd.kind, 4 | 5 | 24) || !self.gsel.edges.is_empty() {
            return true;
        }
        // the interference check computes the volume of the common part through the kernel - without a live B-rep no pairs can be found
        if self.set.show_interference && matches!(self.workbench, Workbench::Assembly) {
            return true;
        }
        // choosing or replacing a sketch plane snaps to edges and vertices
        if self.picking.is_sketch_plane() || self.picking.replace_sketch().is_some() {
            return true;
        }
        // A MATE ANCHOR OF ANY KIND, not only an edge or a vertex (both picking a joint and changing an anchor).
        //
        // This used to test `anchor_mode` against {1, 2}, that is, a face did not count as needing geometry. But
        // a SLIDER's default anchor is exactly a face - and its travel axis is taken from the face's PRINCIPAL
        // DIRECTION, which simply does not exist without a live B-rep. The document's first joint was placed
        // while `regen_faces` was still empty: the axis came from the world axes and the part moved off in the
        // wrong direction. The geometry was raised afterwards (a joint already placed demands it), but the part
        // stayed where it had come to rest - a minimal displacement does not move it for nothing. One wrong
        // second, and the assembly is crooked for good.
        if self.joint.pick_faces || self.joint.edit_repick.is_some() {
            return true;
        }
        // JOINTS ALREADY PLACED ON EDGES AND VERTICES COUNT TOO.
        //
        // Reported behaviour: the joints do not move and a slider's direction is simply wrong however it is
        // picked. Measured on that document: edges were gathered for 2 bodies out of 138, and all five joints on
        // edges (all three sliders) had no connector frame at all - their bodies are imported, and an import is
        // not raised without being asked. A connector's axis is read from `regen_edges`; no edges means no axis,
        // and the solver then moves the part not along the picked edge but along whatever it substitutes for the
        // emptiness.
        //
        // THE PRICE OF THIS DECISION IS STATED PLAINLY: an assembly whose joints sit on edges raises the live
        // B-rep on opening - the "Preparing B-rep" step that lazy building was meant to avoid. There is no
        // cheaper way: a joint without an axis is not "slightly slower", it is a wrong assembly.
        //
        // FACES TOO. An anchor on a face without live geometry gives a frame with no known roll: a rigid joint
        // then leaves the part with a degree of freedom, and a slider on a flat face does not know the principal
        // direction and travels anywhere. A check by matrix showed exactly that.
        if self.project.connectors.iter().any(|c| {
            matches!(
                c.anchor,
                qymcad_core::feature::AnchorRef::EdgeMid(..)
                    | qymcad_core::feature::AnchorRef::Vertex(..)
                    | qymcad_core::feature::AnchorRef::FaceCenter(..)
            ) && self.project.joints.iter().any(|j| j.a == c.id || j.b == c.id)
        }) {
            return true;
        }
        // an open sketch ON A FACE draws that face's outline
        self.sketch_ses
            .editing
            .and_then(|si| self.project.sketches.iter().find(|s| s.id == si))
            .is_some_and(|s| matches!(s.plane, qymcad_core::feature::SketchPlane::Face(..)))
    }

    pub(super) fn refresh_edges(&mut self) {
        // the B-rep cache is brought up ONLY when the edges are really needed - under the fillet and chamfer
        // commands or while edges are actively being picked. This method is called EVERY FRAME in 3D, and an
        // unconditional `ensure_brep` turned lazy B-rep building into eager: "Preparing B-rep" started right
        // after any project was opened, before anything had been done at all.
        //
        // Reported behaviour: the choice of a sketch origin on a face had disappeared. Binding a sketch's origin
        // takes VERTICES and EDGES from the live B-rep, which is no longer built on opening. Open a file, start a
        // new sketch, hover a face - there is nothing to snap to, the green marker never appears, and the origin
        // silently falls back to the default. Picking a sketch plane is as much "the edges are really needed" as
        // a chamfer is.
        if self.needs_live_brep() {
            self.ensure_brep();
        }
        // EDITING a fillet or a chamfer: the edges always belong to the feature's SOURCE BODY (a selection fix
        // put `sel` back on the node being edited, and `selected_body` then returned the OUTPUT body, so the edge
        // selection was cleared and the highlight vanished). The source of the feature being edited is aimed at
        // explicitly.
        let edit_src = self.cmd.edit.and_then(|fid| {
            self.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
                qymcad_core::feature::FeatureKind::Fillet { src, .. } | qymcad_core::feature::FeatureKind::Chamfer { src, .. } => Some(src),
                _ => None,
            })
        });
        // a Part is one body, so under Chamfer/Fillet the edges of that single body are available at once,
        // without clicking the body first (press the button, then click edges). When no body is explicitly
        // selected, the context's active_body is taken.
        //
        // PATCH BELONGS HERE TOO. It gathers EDGES just as the fillet and the chamfer do, but it used to take the
        // body only from the tree selection: press Patch without selecting a part, and not a single edge could be
        // picked. To a person that is "the tool does not work", indistinguishable from having clicked the wrong place.
        let cur = edit_src.or_else(|| self.selected_body()).or_else(|| if matches!(self.cmd.kind, 4 | 5 | 32) { self.current_body() } else { None });
        let body_changed = cur != self.edges.body;
        // the refresh happens not only when the body CHANGES but also when the same body is REBUILT (geom_rev
        // changed) - otherwise edge_polys and edge_ids stay with the old topology, and the highlight lands on
        // edges belonging to something else, or on ones that no longer exist.
        if body_changed || self.edges.rev != self.regen.geom_rev {
            self.edges.body = cur;
            self.edges.rev = self.regen.geom_rev;
            if body_changed {
                self.gsel.edges.clear(); // a different body - the edge selection is not carried over
            }
            let (polys, ids) = cur
                .and_then(|b| self.live.shapes.get(&b).map(|s| s.edges_full_smooth()))
                .map(|(p, i, _, sm)| {
                    if matches!(self.cmd.kind, 4 | 5) {
                        // SMOOTH edges (the tangent seams of fillets) are NOT offered for a chamfer or a fillet -
                        // there is nothing there to round, and picking them only piled up red nodes
                        let (mut fp, mut fi) = (Vec::new(), Vec::new());
                        for k in 0..i.len() {
                            if !sm.get(k).copied().unwrap_or(false) {
                                fp.push(p[k].clone());
                                fi.push(i[k]);
                            }
                        }
                        (fp, fi)
                    } else {
                        (p, i)
                    }
                })
                .unwrap_or_default();
            self.edges.polys = polys;
            self.edges.ids = ids; // the persistent edge ids, parallel to the polylines
            // drop the picked ids the body no longer has (it was rebuilt), so that no phantoms are lit
            if !self.gsel.edges.is_empty() {
                let live: std::collections::HashSet<u32> = self.edges.ids.iter().copied().collect();
                self.gsel.edges.retain(|id| live.contains(id));
            }
        }
    }
}
