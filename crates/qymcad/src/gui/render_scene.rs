//! THE SCENE HANDED TO THE CARD: vertices for the GPU, component thumbnails, the caps of a section, the
//! edges of the selected body.
//!
//! Split out of `render.rs`: this side prepares what is to be drawn, the other side draws it.

pub(crate) use qymcad_ui_state::{section_caps_for_frame};
use super::*;

// THE SCENE AND THE DRAWING CACHES: assembling the scene for the GPU, the component preview, the section caps
// for a frame, refreshing the cache of the selected body's edges.
impl App {}

enum Framing {
    Extent3D(f64),
    FitScreen2D(f64),
}

struct ThumbnailOptions {
    bg: egui::Color32,
    framing: Framing,
}

/// A preview of component `cid`'s body: a self-contained 256x256 orthographic raster (the isometric view of
/// the default camera), WITHOUT mutating the camera or visibility state. It renders only the subtree's bodies
/// in their own frame. `None` means there are no bodies (nothing was built). Used for UI dialog previews.
pub(crate) fn render_component_thumbnail(dc: &qymcad_ui_state::DrawCtx, cid: qymcad_core::model::Id) -> Option<egui::ColorImage> {
    render_project_mesh_raster(dc.project, &dc.scheme.pal, dc.set.ghost_alpha, cid, ThumbnailOptions { bg: dc.scheme.pal.thumbnail_bg(), framing: Framing::Extent3D(0.42) })
}

/// A preview raster specially crafted for embedded file bundles (`thumb.png` in `.qcad`): renders with a
/// completely transparent background, fits the model to 95% of the frame, overlays the application logo
/// watermark in the bottom-right corner, and encodes to PNG. Runs independently of the UI context.
pub(crate) fn render_project_file_preview_png(project: &qymcad_core::model::Project, pal: &qymcad_scheme::Palette, ghost_alpha: u8) -> Option<Vec<u8>> {
    let mut img = render_project_mesh_raster(project, pal, ghost_alpha, project.root, ThumbnailOptions { bg: egui::Color32::TRANSPARENT, framing: Framing::FitScreen2D(0.95) })?;
    overlay_watermark(&mut img.pixels, img.size[0], img.size[1]);
    color_image_to_png(&img)
}

fn render_project_mesh_raster(project: &qymcad_core::model::Project, pal: &qymcad_scheme::Palette, ghost_alpha: u8, cid: qymcad_core::model::Id, opts: ThumbnailOptions) -> Option<egui::ColorImage> {
    use qymcad_core::feature::{apply12, is_identity12};
    const TS: usize = 256;
    let mut subtree: std::collections::HashSet<qymcad_core::model::Id> = project.descendants(cid).into_iter().collect();
    subtree.insert(cid);
    // the subtree's bodies + their transform RELATIVE TO cid (the part at its own origin)
    let mut items: Vec<(usize, [f64; 12])> = Vec::new();
    for mi in 0..project.bodies.len() {
        let Some(b) = project.mesh_id(mi) else { continue };
        if project.body_owner(b).map(|o| subtree.contains(&o)) != Some(true) {
            continue;
        }
        if project.bodies[mi].mesh.verts.is_empty() {
            continue;
        }
        items.push((mi, project.body_display_transform(b, cid)));
    }
    if items.is_empty() {
        return None;
    }
    let (right, up, fwd) = Cam3::default().basis(); // a fixed isometric view, independent of the current camera
    let (center_u, center_v, s) = match opts.framing {
        Framing::Extent3D(fill) => {
            let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for &(mi, wt) in &items {
                let ident = is_identity12(&wt);
                for v in &project.bodies[mi].mesh.verts {
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
            let s = (TS as f64 * fill) / ext;
            (v_dot(center, right), v_dot(center, up), s)
        }
        Framing::FitScreen2D(fill) => {
            let (mut mn_u, mut mx_u) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut mn_v, mut mx_v) = (f64::INFINITY, f64::NEG_INFINITY);
            for &(mi, wt) in &items {
                let ident = is_identity12(&wt);
                for v in &project.bodies[mi].mesh.verts {
                    let p = if ident { [v.x, v.y, v.z] } else { apply12(&wt, [v.x, v.y, v.z]) };
                    let u = v_dot(p, right);
                    let v = v_dot(p, up);
                    mn_u = mn_u.min(u);
                    mx_u = mx_u.max(u);
                    mn_v = mn_v.min(v);
                    mx_v = mx_v.max(v);
                }
            }
            if !mn_u.is_finite() {
                return None;
            }
            let span_u = (mx_u - mn_u).max(1e-4);
            let span_v = (mx_v - mn_v).max(1e-4);
            let cu = (mn_u + mx_u) / 2.0;
            let cv = (mn_v + mx_v) / 2.0;
            let target = (TS as f64) * fill;
            let s = (target / span_u).min(target / span_v);
            (cu, cv, s)
        }
    };
    let light = qymcad_ui_state::scene_light();
    let hc = TS as f64 / 2.0;
    let proj = |p: [f64; 3]| -> (f64, f64, f64) {
        let u = v_dot(p, right);
        let v = v_dot(p, up);
        let depth = v_dot(p, fwd);
        (hc + (u - center_u) * s, hc - (v - center_v) * s, depth)
    };
    let ef = |ux: f64, uy: f64, vx: f64, vy: f64, px: f64, py: f64| (vx - ux) * (py - uy) - (vy - uy) * (px - ux);
    let mut color = vec![opts.bg; TS * TS];
    let mut zbuf = vec![f64::INFINITY; TS * TS];
    for &(mi, wt) in &items {
        let mesh = &project.bodies[mi].mesh;
        let base = project.mesh_color(mi);
        let ident = is_identity12(&wt);
        let pw = |vi: u32| {
            let p = mesh.verts[vi as usize];
            let a = [p.x, p.y, p.z];
            if ident {
                a
            } else {
                apply12(&wt, a)
            }
        };
        for tri in &mesh.tris {
            let (a, b, cc) = (pw(tri[0]), pw(tri[1]), pw(tri[2]));
            let n = v_norm(v_cross(v_sub(b, a), v_sub(cc, a)));
            // THIS ONE STAYS ORTHOGRAPHIC: the thumbnail has a fixed isometric view of its own, unrelated to
            // the viewport camera, and one ray serves the whole frame.
            if v_dot(n, fwd) >= 0.0 {
                continue; // the bodies are oriented outwards
            }
            let col = qymcad_pick::shade_tri(pal, ghost_alpha, false, false, base, n, light);
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

fn overlay_watermark(pixels: &mut [egui::Color32], width: usize, height: usize) {
    let Some(logo) = watermark_logo() else { return };
    let (lw, lh) = logo.dimensions();
    let (lw, lh) = (lw as usize, lh as usize);
    const MARGIN: usize = 8;
    if width < lw + MARGIN || height < lh + MARGIN {
        return;
    }
    let start_x = width - lw - MARGIN;
    let start_y = height - lh - MARGIN;
    for ly in 0..lh {
        for lx in 0..lw {
            let sp = logo.get_pixel(lx as u32, ly as u32);
            let sa = sp[3] as f32 / 255.0;
            if sa <= 1e-4 {
                continue;
            }
            let idx = (start_y + ly) * width + (start_x + lx);
            let dst = pixels[idx];
            let da = dst.a() as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a > 1e-4 {
                let sr = sp[0] as f32;
                let sg = sp[1] as f32;
                let sb = sp[2] as f32;
                let dr = dst.r() as f32;
                let dg = dst.g() as f32;
                let db = dst.b() as f32;
                let out_r = ((sr * sa + dr * da * (1.0 - sa)) / out_a).round() as u8;
                let out_g = ((sg * sa + dg * da * (1.0 - sa)) / out_a).round() as u8;
                let out_b = ((sb * sa + db * da * (1.0 - sa)) / out_a).round() as u8;
                let out_a_u8 = (out_a * 255.0).round().min(255.0) as u8;
                pixels[idx] = egui::Color32::from_rgba_unmultiplied(out_r, out_g, out_b, out_a_u8);
            }
        }
    }
}

fn watermark_logo() -> Option<&'static image::RgbaImage> {
    static LOGO: std::sync::OnceLock<Option<image::RgbaImage>> = std::sync::OnceLock::new();
    LOGO.get_or_init(|| {
        const BYTES: &[u8] = include_bytes!("../../../../assets/icons/linux/64x64.png");
        image::load_from_memory(BYTES).ok().map(|img| img.to_rgba8())
    })
    .as_ref()
}

/// Assemble the scene's vertices for the GPU: world triangles + the face normal (for culling in the
/// fragment shader) + a shaded colour AT EVERY VERTEX. Smooth shading (Gouraud) computes the colour from the
/// vertex's SMOOTHED normal and lets the GPU interpolate across the triangle (the `color` varying); sharp
/// edges stay sharp (the mesh topology is split by face). Flat mode puts the face normal into all three.
/// Returns `(vertices, opaque_count)`: the opaque ones FIRST, in `[0..opaque_count)`, and the ghosts
/// (alpha<255) AFTER (two passes: opaque writes depth, then transparent tests without writing and alpha-blends).
/// THE SCENE AS ONE VECTOR - FOR MEASUREMENT ONLY.
///
/// The drawing path never glues the pieces: that was a second full copy of the scene in memory, 739 MB of it
/// on the reference engine. A check that wants to walk every vertex glues them itself, and the cost falls on
/// the check rather than on every frame a person sees.
#[cfg(test)]
pub(crate) fn gpu_scene_flat(pn: &qymcad_ui_state::Painting) -> (Vec<qymcad_ui_state::GpuVert>, Vec<qymcad_ui_state::BodyLook>) {
    let scene = gpu_scene(pn);
    // THE TRIANGLES AS THE CARD SEES THEM: the indices followed through, so a check reads the same three
    // vertices per triangle whether or not the scene is indexed.
    let verts = scene.pieces.iter().flat_map(|p| p.idx.iter().map(|&i| p.verts[i as usize])).collect();
    (verts, scene.looks)
}

/// WHAT EVERY VISIBLE BODY LOOKS LIKE RIGHT NOW - without touching a single vertex.
///
/// This is the cheap half of the scene, and it is asked for on EVERY frame: the pointer moves over the model,
/// a subassembly is entered, a colour is changed - all of that lives here. The order is the display order, the
/// same one `gpu_scene` puts the pieces in, so the number in a vertex names the same row of this table.
pub(crate) fn scene_looks(pn: &qymcad_ui_state::Painting) -> Vec<qymcad_ui_state::BodyLook> {
    let mut looks: Vec<qymcad_ui_state::BodyLook> = Vec::new();
    for m in qymcad_ui_state::visible_mesh_items(pn) {
        let state = if m.hot { qymcad_ui_state::LOOK_HOT } else { 0 } | if m.ghost { qymcad_ui_state::LOOK_GHOST } else { 0 };
        // the body's row, then a row per colour of its faces: the layout `gpu_scene` gives the vertices
        let (palette, _) = qymcad_ui_state::face_palette(pn.project, m.index, 0);
        for c in std::iter::once(m.tint).chain(palette) {
            let b = qymcad_scheme::brighten(c, pn.scheme.pal.body_lighten, pn.scheme.pal.body_saturate);
            looks.push(qymcad_ui_state::BodyLook { tint: u32::from_le_bytes([b[0], b[1], b[2], 0]), state });
        }
    }
    if pn.section.plane.is_some() {
        // the caps keep the last row, as they do in `gpu_scene`
        looks.push(qymcad_ui_state::BodyLook { tint: u32::from_le_bytes([224, 168, 92, 0]), state: qymcad_ui_state::LOOK_CAP });
    }
    looks
}

/// A normal packed into four bytes: one signed byte per axis.
///
/// 1/127 of accuracy - far finer than shading can show, and four bytes instead of twelve.
fn pack_normal(n: [f64; 3]) -> u32 {
    let b = |v: f64| ((v.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8;
    u32::from_le_bytes([b(n[0]), b(n[1]), b(n[2]), 0])
}

/// A BLOCK TAKEN READY-MADE ANSWERS TO THE BODY'S NUMBER OF THIS FRAME.
///
/// The number in a vertex is the body's row in the look table, its place in the list of what is shown - and that
/// list changes with the context: the pin of the reference assembly is the third body at the top and the first
/// inside its subassembly. A block kept under its old number sent the card to another body's row, or past the end
/// of the table. Reported behaviour: the parts of a subassembly stepped into stayed drawn lighter, as if selected,
/// until a tool rebuilt every block. The vertices are renumbered in place, the same pass a move already makes; the
/// pieces go to the card again anyway, since what is shown is part of the scene's key.
fn answer_to(block: &mut qymcad_ui_state::SceneBlock, no: u32) {
    if block.body == no {
        return;
    }
    // the block's rows are its body's and, right after it, one per colour of its faces: they shift together
    let from = block.body;
    for part in block.parts.iter_mut() {
        for v in std::sync::Arc::make_mut(&mut part.verts).iter_mut() {
            v.body = v.body - from + no;
        }
    }
    block.body = no;
}

pub(crate) fn gpu_scene(pn: &qymcad_ui_state::Painting) -> qymcad_ui_state::GpuScene {
    let smooth = pn.set.shading == qymcad_ui_state::Shading::Smooth;
    if smooth {
        qymcad_ui_state::ensure_vertex_normals(pn.cache, pn.project, pn.regen);
    }
    let ncache = pn.cache.norm.borrow();
    let items = qymcad_ui_state::visible_mesh_items(pn);
    let mut caps_verts: Vec<qymcad_ui_state::GpuVert> = Vec::new();
    let mut looks: Vec<qymcad_ui_state::BodyLook> = Vec::new();
    let mut look_of: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut rows_of: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
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
        // THE LOOK OF THE BODY, gathered every frame and costing nothing: it is two numbers, not vertices.
        let body_no = looks.len() as u32;
        looks.push(qymcad_ui_state::BodyLook {
            tint: {
                let b = qymcad_scheme::brighten(base, pn.scheme.pal.body_lighten, pn.scheme.pal.body_saturate);
                u32::from_le_bytes([b[0], b[1], b[2], 0])
            },
            state: if hot { qymcad_ui_state::LOOK_HOT } else { 0 } | if ghost { qymcad_ui_state::LOOK_GHOST } else { 0 },
        });
        look_of.insert(mi, body_no);
        // THE COLOURS OF ITS FACES, where a file coloured faces apart: a row each, right after the body's, in the body's
        // state - the pass is chosen by the body's row, the colour by the vertex's
        let (palette, per_tri) = qymcad_ui_state::face_palette(pn.project, mi, mesh.tris.len());
        for c in &palette {
            let b = qymcad_scheme::brighten(*c, pn.scheme.pal.body_lighten, pn.scheme.pal.body_saturate);
            looks.push(qymcad_ui_state::BodyLook {
                tint: u32::from_le_bytes([b[0], b[1], b[2], 0]),
                state: if hot { qymcad_ui_state::LOOK_HOT } else { 0 } | if ghost { qymcad_ui_state::LOOK_GHOST } else { 0 },
            });
        }
        rows_of.insert(mi, 1 + palette.len() as u32);
        // THE KEY IS ABOUT SHAPE ALONE. The position lives separately (`SceneBlock::at`) and a move does not
        // invalidate the block; the LOOK - highlight, ghosting, tint - is not here at all any more. It used
        // to be, and that is what made stepping into a subassembly rebuild every block of the scene: the look
        // of every body changes at once. What a body looks like now travels beside the vertices, in a table
        // of its own.
        // which triangle names which row is written into the vertices, so it belongs to the block's key
        let shape = if per_tri.is_empty() {
            common
        } else {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            common.hash(&mut h);
            per_tri.hash(&mut h);
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
                answer_to(b, body_no);
                continue;
            }
            Some(b) if b.shape == shape && pn.section.plane.is_none() && same_rotation12(&b.at, &wt) => {
                stats[1] += 1;
                let d = [(wt[3] - b.at[3]) as f32, (wt[7] - b.at[7]) as f32, (wt[11] - b.at[11]) as f32];
                for part in b.parts.iter_mut() {
                    for v in std::sync::Arc::make_mut(&mut part.verts).iter_mut() {
                        v.pos[0] += d[0];
                        v.pos[1] += d[1];
                        v.pos[2] += d[2];
                    }
                }
                b.at = wt;
                answer_to(b, body_no);
                continue;
            }
            _ => {}
        }

        let ident = qymcad_core::feature::is_identity12(&wt);
        // the smoothed local vertex normals of this body (when enabled and present in the cache)
        let vn = if smooth { ncache.value.get(mi) } else { None };
        // A VERTEX OF THE MESH IS WRITTEN ONCE and pointed at by every triangle that touches it. `of_mesh`
        // says where a mesh vertex ended up in this part; `u32::MAX` means it has not been written yet.
        // Measured on a closed mesh: a vertex belongs to about six triangles, and unrolled it was written out
        // three times over (each triangle its own three).
        let mut parts: Vec<qymcad_ui_state::BlockPart> = Vec::new();
        let mut own: Vec<qymcad_ui_state::GpuVert> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        let mut of_mesh: Vec<u32> = vec![u32::MAX; mesh.verts.len()];
        let mut row_of: Vec<u32> = vec![body_no; mesh.verts.len()]; // the row the vertex written for a mesh vertex names
        let mut nrm_of: Vec<u32> = vec![0; mesh.verts.len()]; // and the normal it carries
        for i in 0..mesh.tris.len() {
            let tri = mesh.tris[i];
            let pos_w = |vi: u32| {
                let p = mesh.verts[vi as usize];
                let a = [p.x, p.y, p.z];
                if ident {
                    a
                } else {
                    qymcad_core::feature::apply12(&wt, a)
                }
            };
            let (a, b, c) = (pos_w(tri[0]), pos_w(tri[1]), pos_w(tri[2]));
            // THE SECTION: an HONEST clip of the triangle by the plane (a cut exactly along it, with no needles)
            let clip = qymcad_ui_state::section_clip_tri(pn.section, [a, b, c]);
            if !clip.whole && clip.verts.is_empty() {
                continue; // wholly on the hidden side
            }
            // ONE TRIANGLE ADDS AT MOST 6 VERTICES AND 6 INDICES (a clipped quadrilateral), so the part is
            // closed while that much still fits: a part must lie inside ONE buffer of the card.
            if own.len() + 6 > qymcad_ui_state::BLOCK_VERTEX_CAP || idx.len() + 6 > qymcad_ui_state::BLOCK_INDEX_CAP {
                parts.push(qymcad_ui_state::BlockPart { verts: std::sync::Arc::new(std::mem::take(&mut own)), idx: std::sync::Arc::new(std::mem::take(&mut idx)) });
                of_mesh.iter_mut().for_each(|v| *v = u32::MAX); // the new part starts with vertices of its own
            }
            // THE NORMAL AT EVERY VERTEX, for smooth shading: the vertex normal turned into the world. In flat
            // shading nothing is stored - the fragment takes the face normal from the derivatives.
            let nrm_at = |k: usize| -> u32 {
                match vn {
                    Some(list) => pack_normal(qymcad_ui_state::rotate_normal(&wt, list.at(i, k, tri[k]))),
                    None => 0,
                }
            };
            let nrms = [nrm_at(0), nrm_at(1), nrm_at(2)];
            let row = per_tri.get(i).copied().flatten().map_or(body_no, |k| body_no + 1 + k as u32);
            if clip.whole {
                for (k, (p, vi)) in [(a, tri[0]), (b, tri[1]), (c, tri[2])].into_iter().enumerate() {
                    let mut at = of_mesh[vi as usize];
                    // a vertex is shared only by triangles of one row - the row is carried flat, from one vertex - and
                    // of one normal: the corners of a mesh piece across a sharp edge are lit apart
                    if at == u32::MAX || row_of[vi as usize] != row || nrm_of[vi as usize] != nrms[k] {
                        at = own.len() as u32;
                        own.push(qymcad_ui_state::GpuVert { pos: [p[0] as f32, p[1] as f32, p[2] as f32], body: row, nrm: nrms[k] });
                        of_mesh[vi as usize] = at;
                        row_of[vi as usize] = row;
                        nrm_of[vi as usize] = nrms[k];
                    }
                    idx.push(at);
                }
            } else {
                // A CLIPPED TRIANGLE BRINGS POINTS OF ITS OWN - they are in no mesh, so they are appended and
                // indexed in order. A fan over the clipped polygon (3..4 vertices -> 1..2 triangles).
                for k in 1..clip.verts.len().saturating_sub(1) {
                    for &vi in &[0, k, k + 1] {
                        let cv = clip.verts[vi];
                        // at a clipped vertex the normal of the nearest corner is taken: the weights are
                        // barycentric, so the largest of them names the corner the point came from
                        let w = cv.w;
                        let nk = if w[0] >= w[1] && w[0] >= w[2] {
                            0
                        } else if w[1] >= w[2] {
                            1
                        } else {
                            2
                        };
                        idx.push(own.len() as u32);
                        own.push(qymcad_ui_state::GpuVert { pos: [cv.pos[0] as f32, cv.pos[1] as f32, cv.pos[2] as f32], body: row, nrm: nrms[nk] });
                    }
                }
            }
        }
        if !own.is_empty() {
            parts.push(qymcad_ui_state::BlockPart { verts: std::sync::Arc::new(own), idx: std::sync::Arc::new(idx) });
        }
        stats[0] += 1;
        blocks.insert(mi, super::SceneBlock { shape, at: wt, body: body_no, parts });
    }
    pn.cache.scene_stats.set(stats);
    blocks.retain(|mi, _| live.contains(mi)); // bodies that are no longer visible hold no memory
                                              // THE ASSEMBLY ORDER IS THE ONE IT ALWAYS WAS: the display order of the bodies, not the order in the
                                              // hash map. For translucent bodies the order is visible to the eye (they blend), so it must not change.
                                              // THE SIZE IS KNOWN IN ADVANCE and should be asked for at once. The concatenation runs over 138 pieces
                                              // into an empty vector, that is, with a dozen and a half reallocations and copies of an ever-growing
                                              // buffer; at 463,878 vertices that is a noticeable share of the frame's cost, taken for nothing.
                                              // THE PIECES ARE COLLECTED, NOT GLUED, and in a STABLE order: the display order of the bodies, whatever
                                              // each of them looks like right now. Gluing meant a second full copy of the scene in memory; tying the
                                              // order to the look meant re-uploading it whenever a body turned into a ghost.
    let mut pieces: Vec<qymcad_ui_state::ScenePiece> = Vec::new();
    for mi in &order {
        if let (Some(b), Some(&no)) = (blocks.get(mi), look_of.get(mi)) {
            for part in b.parts.iter().filter(|p| !p.idx.is_empty()) {
                pieces.push(qymcad_ui_state::ScenePiece { verts: part.verts.clone(), idx: part.idx.clone(), body: no, rows: rows_of.get(mi).copied().unwrap_or(1) });
            }
        }
    }
    // THE SECTION CAPS: an amber fill, two-sided (the cut is visible from both sides). They belong to no body,
    // so they get a look of their own at the end of the table.
    if pn.section.plane.is_some() {
        let caps = section_caps_for_frame(pn);
        if let Some((_, n)) = qymcad_ui_state::section_eff(pn.section) {
            // THE CAP IS NUDGED A HAIR INTO THE CUT-AWAY SIDE (the bodies keep the half-space d <= 0, so
            // at d = +eps NOTHING occludes the cap - there is no material left there). It cannot lie exactly
            // in the plane: the thread turns run almost tangent to the cut, their clipped triangles stand in
            // the same plane and win the depth test against the cap - and the fill then disappears in patches
            // precisely in the threaded zone (reported: the bottom of the part filled, the thread empty,
            // while the cap itself covers 99.8% of the outline). The nudge is thousandths of the extent and
            // does not affect the geometry.
            let eps = caps.iter().filter_map(|m| m.bounds()).map(|b| (b.max.x - b.min.x).max(b.max.y - b.min.y).max(b.max.z - b.min.z)).fold(0.0_f64, f64::max).max(1.0) * 1.0e-3;
            let off = [n[0] * eps, n[1] * eps, n[2] * eps];
            let cap_no = looks.len() as u32;
            // AMBER, AND NOT SHADED BY THE LIGHT: a cut is a section, not a surface of the part, and it reads
            // as a fill. The `LOOK_CAP` bit says exactly that to the fragment.
            looks.push(qymcad_ui_state::BodyLook { tint: u32::from_le_bytes([224, 168, 92, 0]), state: qymcad_ui_state::LOOK_CAP });
            for mesh in caps.iter() {
                for t in 0..mesh.tris.len() {
                    let tri = mesh.triangle(t).map(|p| qymcad_core::geom::Point3::new(p.x + off[0], p.y + off[1], p.z + off[2]));
                    // both sides of the cap: the same triangle wound two ways, so it is seen from either side
                    for order in [[0usize, 1, 2], [0, 2, 1]] {
                        for &k in &order {
                            let p = tri[k];
                            caps_verts.push(qymcad_ui_state::GpuVert { pos: [p.x as f32, p.y as f32, p.z as f32], body: cap_no, nrm: 0 });
                        }
                    }
                }
            }
            if !caps_verts.is_empty() {
                // THE CAPS ARE NOT INDEXED: every triangle of a cut brings its own points (and twice over, for
                // the two windings), so there is nothing to share - the indices simply run in order.
                let idx: Vec<u32> = (0..caps_verts.len() as u32).collect();
                pieces.push(qymcad_ui_state::ScenePiece { verts: std::sync::Arc::new(std::mem::take(&mut caps_verts)), idx: std::sync::Arc::new(idx), body: cap_no, rows: 1 });
            }
        }
    }
    qymcad_ui_state::GpuScene { pieces, looks }
}
