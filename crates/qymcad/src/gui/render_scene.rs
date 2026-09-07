//! THE SCENE HANDED TO THE CARD: vertices for the GPU, component thumbnails, the caps of a section, the
//! edges of the selected body.
//!
//! Split out of `render.rs`: this side prepares what is to be drawn, the other side draws it.

pub(crate) use qymcad_ui_state::{section_caps_for_frame};
use super::*;

// THE SCENE AND THE DRAWING CACHES: assembling the scene for the GPU, the component preview, the section caps
// for a frame, refreshing the cache of the selected body's edges.
impl App {







}

/// A preview of component `cid`'s body: a self-contained 256x256 orthographic raster (the isometric view of
/// the default camera), WITHOUT mutating the camera or visibility state. It renders only the subtree's bodies
/// in their own frame. `None` means there are no bodies (nothing was built). Used for the product's `thumb.png`.
pub(crate) fn render_component_thumbnail(dc: &qymcad_ui_state::DrawCtx, cid: qymcad_core::model::Id) -> Option<egui::ColorImage> {
    use qymcad_core::feature::{apply12, is_identity12};
    const TS: usize = 256;
    let mut subtree: std::collections::HashSet<qymcad_core::model::Id> = dc.project.descendants(cid).into_iter().collect();
    subtree.insert(cid);
    // the subtree's bodies + their transform RELATIVE TO cid (the part at its own origin)
    let mut items: Vec<(usize, [f64; 12])> = Vec::new();
    for mi in 0..dc.project.bodies.len() {
        let Some(b) = dc.project.mesh_id(mi) else { continue };
        if dc.project.body_owner(b).map(|o| subtree.contains(&o)) != Some(true) {
            continue;
        }
        if dc.project.bodies[mi].mesh.verts.is_empty() {
            continue;
        }
        items.push((mi, dc.project.body_display_transform(b, cid)));
    }
    if items.is_empty() {
        return None;
    }
    // the subtree's world bbox
    let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for &(mi, wt) in &items {
        let ident = is_identity12(&wt);
        for v in &dc.project.bodies[mi].mesh.verts {
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
    let mut color = vec![dc.scheme.pal.thumbnail_bg(); TS * TS]; // a dark background, as in the viewport
    let mut zbuf = vec![f64::INFINITY; TS * TS];
    for &(mi, wt) in &items {
        let mesh = &dc.project.bodies[mi].mesh;
        let base = dc.project.mesh_color(mi);
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
            let col = qymcad_pick::shade_tri(&dc.scheme.pal, dc.set.ghost_alpha, false, false, base, n, light);
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
    Some(egui::ColorImage { size: [TS, TS], source_size: egui::Vec2::new([TS, TS][0] as f32, [TS, TS][1] as f32), pixels: color })
}

/// Assemble the scene's vertices for the GPU: world triangles + the face normal (for culling in the
/// fragment shader) + a shaded colour AT EVERY VERTEX. Smooth shading (Gouraud) computes the colour from the
/// vertex's SMOOTHED normal and lets the GPU interpolate across the triangle (the `color` varying); sharp
/// edges stay sharp (the mesh topology is split by face). Flat mode puts the face normal into all three.
/// Returns `(vertices, opaque_count)`: the opaque ones FIRST, in `[0..opaque_count)`, and the ghosts
/// (alpha<255) AFTER (two passes: opaque writes depth, then transparent tests without writing and alpha-blends).
pub(crate) fn gpu_scene(pn: &qymcad_ui_state::Painting) -> (Vec<qymcad_ui_state::GpuVert>, u32) {
    let light = v_norm([0.35, 0.5, 0.78]);
    let smooth = pn.set.shading == qymcad_ui_state::Shading::Smooth;
    if smooth {
        qymcad_ui_state::ensure_vertex_normals(pn.cache, pn.project, pn.regen);
    }
    let ncache = pn.cache.norm.borrow();
    let items = qymcad_ui_state::visible_mesh_items(pn);
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
    let mut blocks = pn.cache.scene_blocks.borrow_mut();
    let common = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        pn.regen.geom_rev.hash(&mut h); // the SHAPE of the bodies; this body's position enters the key separately (`wt`)
        smooth.hash(&mut h);
        pn.set.ghost_alpha.hash(&mut h);
        pn.scheme.pal.fingerprint().hash(&mut h);
        // the section cuts the triangles IN THE WORLD, so the block depends on it too
        if let Some((o, n)) = qymcad_ui_state::section_eff(pn.section) {
            for v in o.iter().chain(n.iter()) {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    };
    let mut live: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut order: Vec<usize> = Vec::new();
    let mut stats = [0u32; 3]; // [rebuilt, shifted, taken ready-made]
    for qymcad_ui_state::SceneMesh { index: mi, hot, ghost, tint: base, mesh, world: wt } in items {
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
            Some(b) if b.shape == shape && pn.section.plane.is_none() && same_rotation12(&b.at, &wt) => {
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
        let vn = if smooth { ncache.value.get(mi) } else { None };
        for i in 0..mesh.tris.len() {
            let tri = mesh.tris[i];
            let pos_w = |vi: u32| {
                let p = mesh.verts[vi as usize];
                let a = [p.x, p.y, p.z];
                if ident { a } else { qymcad_core::feature::apply12(&wt, a) }
            };
            let (a, b, c) = (pos_w(tri[0]), pos_w(tri[1]), pos_w(tri[2]));
            // THE SECTION: an HONEST clip of the triangle by the plane (a cut exactly along it, with no needles)
            let clip = qymcad_ui_state::section_clip_tri(pn.section, [a, b, c]);
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
                    Some(list) => qymcad_ui_state::rotate_normal(&wt, list[vi as usize]),
                    None => fn_w,
                };
                qymcad_pick::shade_tri(&pn.scheme.pal, pn.set.ghost_alpha, hot, ghost, base, nrm, light).to_array()
            };
            let al = if ghost { pn.set.ghost_alpha } else { 255 };
            let dst = if al < 255 { &mut transp } else { &mut opaque };
            let cols = [col_at(tri[0]), col_at(tri[1]), col_at(tri[2])];
            let mut push = |p: [f64; 3], w: [f64; 3]| {
                let mix = |k: usize| (cols[0][k] as f64 * w[0] + cols[1][k] as f64 * w[1] + cols[2][k] as f64 * w[2]).round().clamp(0.0, 255.0) as u8;
                dst.push(qymcad_ui_state::GpuVert {
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
    pn.cache.scene_stats.set(stats);
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
    if pn.section.plane.is_some() {
        let caps = section_caps_for_frame(pn);
        let (col, coln) = (u32::from_le_bytes([224, 168, 92, 255]), u32::from_le_bytes([176, 128, 66, 255]));
        if let Some((_, n)) = qymcad_ui_state::section_eff(pn.section) {
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
                            opaque.push(qymcad_ui_state::GpuVert {
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
