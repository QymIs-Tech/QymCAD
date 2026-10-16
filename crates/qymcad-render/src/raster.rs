//! THE SOFTWARE RASTER: triangles banded and blended, the mesh drawn, the 3D view rasterised.

use super::*;

/// Rasterise triangles into the buffer band [y0, y0 + rows) with a per-pixel z test.
/// `color` and `zbuf` cover exactly that band (their indexing is local to it). A vertex is
/// `[px, py, depth]` in global pixel coordinates (the depth measured along fwd, smaller being nearer).
/// A TRIANGLE READY FOR THE RASTERISER: three screen vertices with depth, and a colour at each.
///
/// It was the four-place tuple `([f32; 3], [f32; 3], [f32; 3], [Color32; 3])`, written out four times, and
/// the third `[f32; 3]` looked exactly like the first two while meaning a different vertex. The `[f32; 3]`
/// itself is `(x, y, ndc_z)` - screen coordinates plus the screen-linear depth the z buffer takes.
pub struct RasterTri {
    /// The three vertices, each `(x, y, ndc_z)`.
    pub v: [[f32; 3]; 3],
    /// The colour at each vertex, in the same order.
    pub cols: [Color32; 3],
}

pub fn raster_band(color: &mut [Color32], zbuf: &mut [f32], w: usize, y0: usize, rows: usize, tris: &[RasterTri]) {
    let ymax = (y0 + rows) as i32;
    for RasterTri { v: [v0, v1, v2], cols } in tris {
        let minx = v0[0].min(v1[0]).min(v2[0]).floor().max(0.0) as i32;
        let maxx = v0[0].max(v1[0]).max(v2[0]).ceil().min(w as f32 - 1.0) as i32;
        let miny = (v0[1].min(v1[1]).min(v2[1]).floor() as i32).max(y0 as i32);
        let maxy = (v0[1].max(v1[1]).max(v2[1]).ceil() as i32).min(ymax - 1);
        if minx > maxx || miny > maxy {
            continue;
        }
        let area = qymcad_ui_state::edge(v0[0], v0[1], v1[0], v1[1], v2[0], v2[1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let inv = 1.0 / area;
        let flat = cols[0] == cols[1] && cols[1] == cols[2]; // a flat-shaded triangle needs no interpolation
        for py in miny..=maxy {
            let row = (py as usize - y0) * w;
            for px in minx..=maxx {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w0 = qymcad_ui_state::edge(v1[0], v1[1], v2[0], v2[1], fx, fy);
                let w1 = qymcad_ui_state::edge(v2[0], v2[1], v0[0], v0[1], fx, fy);
                let w2 = qymcad_ui_state::edge(v0[0], v0[1], v1[0], v1[1], fx, fy);
                let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
                if !inside {
                    continue;
                }
                let depth = (w0 * v0[2] + w1 * v1[2] + w2 * v2[2]) * inv;
                let idx = row + px as usize;
                if depth < zbuf[idx] {
                    zbuf[idx] = depth;
                    color[idx] = if flat { cols[0] } else { interp_color(cols, w0 * inv, w1 * inv, w2 * inv) };
                }
            }
        }
    }
}

/// The second pass: the translucent (ghost) triangles go OVER the opaque ones, and a ghost is seen as ITS NEAREST
/// SURFACE only - one pane of glass over what stands behind it. First the nearest ghost depth of every pixel is found
/// (solid bodies still occlude a ghost through `zbuf`), then only the fragments at that depth are blended, once. Every
/// face blended over the others made a mess of layers: the walls of a hole through a ghost, and the faces behind
/// them, heaped up darker than the rest. The blend is premultiplied-over: `Color32` stores premultiplied colour, so
/// out = src + dst * (1 - src_a). It mirrors the GPU path (`mesh_pipeline_ghost_depth`, then `mesh_pipeline_ghost`).
pub fn raster_band_blend(color: &mut [Color32], zbuf: &[f32], w: usize, y0: usize, rows: usize, tris: &[RasterTri]) {
    let mut near = vec![f32::INFINITY; color.len()];
    each_fragment(w, y0, rows, tris, |idx, depth, _, _| {
        if depth < zbuf[idx] && depth < near[idx] {
            near[idx] = depth;
        }
    });
    each_fragment(w, y0, rows, tris, |idx, depth, cols, b| {
        let n = near[idx];
        if depth >= zbuf[idx] || n == f32::NEG_INFINITY || depth > n + 1e-6 * n.abs().max(1.0) {
            return; // behind an opaque body, blended already, or behind the nearest face of a ghost
        }
        let flat = cols[0] == cols[1] && cols[1] == cols[2];
        let src = if flat { cols[0] } else { interp_color(cols, b[0], b[1], b[2]) }.to_array(); // premultiplied
        let ia = 255 - src[3] as u32; // 1 - src_a, on a 0..255 scale
        let dst = color[idx].to_array();
        let blend = |s: u8, d: u8| (s as u32 + (d as u32 * ia + 127) / 255).min(255) as u8;
        color[idx] = Color32::from_rgba_premultiplied(blend(src[0], dst[0]), blend(src[1], dst[1]), blend(src[2], dst[2]), blend(src[3], dst[3]));
        near[idx] = f32::NEG_INFINITY; // blended once: a second face at the same depth - a seam - does not darken it
    });
}

/// Every pixel the triangles cover in the band, with its depth, the colours of its triangle and its weights.
pub(crate) fn each_fragment(w: usize, y0: usize, rows: usize, tris: &[RasterTri], mut f: impl FnMut(usize, f32, &[Color32; 3], [f32; 3])) {
    let ymax = (y0 + rows) as i32;
    for RasterTri { v: [v0, v1, v2], cols } in tris {
        let minx = v0[0].min(v1[0]).min(v2[0]).floor().max(0.0) as i32;
        let maxx = v0[0].max(v1[0]).max(v2[0]).ceil().min(w as f32 - 1.0) as i32;
        let miny = (v0[1].min(v1[1]).min(v2[1]).floor() as i32).max(y0 as i32);
        let maxy = (v0[1].max(v1[1]).max(v2[1]).ceil() as i32).min(ymax - 1);
        if minx > maxx || miny > maxy {
            continue;
        }
        let area = qymcad_ui_state::edge(v0[0], v0[1], v1[0], v1[1], v2[0], v2[1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let inv = 1.0 / area;
        for py in miny..=maxy {
            let row = (py as usize - y0) * w;
            for px in minx..=maxx {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w0 = qymcad_ui_state::edge(v1[0], v1[1], v2[0], v2[1], fx, fy);
                let w1 = qymcad_ui_state::edge(v2[0], v2[1], v0[0], v0[1], fx, fy);
                let w2 = qymcad_ui_state::edge(v0[0], v0[1], v1[0], v1[1], fx, fy);
                let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
                if !inside {
                    continue;
                }
                let depth = (w0 * v0[2] + w1 * v1[2] + w2 * v2[2]) * inv;
                f(row + px as usize, depth, cols, [w0 * inv, w1 * inv, w2 * inv]);
            }
        }
    }
}

pub fn interp_color(cols: &[Color32; 3], b0: f32, b1: f32, b2: f32) -> Color32 {
    let (a, b, c) = (cols[0].to_array(), cols[1].to_array(), cols[2].to_array());
    // rounded, not cut down: the weights add up to one only to the float, and 254.99 cut down made a point of an
    // opaque body see-through and half a step darker
    let ch = |i: usize| (b0 * a[i] as f32 + b1 * b[i] as f32 + b2 * c[i] as f32).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3))
}

pub fn draw_mesh(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    // a top view (or IN THE PLANE of the active sketch when editing on a face or a datum - so that the
    // body lines up with the sketch's coordinates instead of hanging above it). Shading goes by depth
    // along the normal.
    let frame = qymcad_ui_state::active_2d_sketch(pn.armed, pn.cmd, pn.project, pn.sel, pn.sketch_ses).and_then(|si| pn.project.sketch_frame(si)).filter(|f| !f.is_identity());
    // while a sketch is being edited the body is drawn as a GHOST (dimmed and translucent) so that the
    // sketch geometry reads on top of it instead of being drowned by a bright fill.
    let ghost = pn.sketch_ses.editing.is_some();
    let proj = |v: qymcad_core::geom::Point3| -> (Point2, f64) {
        match &frame {
            Some(f) => (f.project(v), f.depth(v)),
            None => (Point2::new(v.x, v.y), v.z),
        }
    };
    for (mi, mesh) in pn.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        if !qymcad_ui_state::body_shown(pn.body_view(), mi) {
            continue;
        }
        let (mut dmin, mut dmax) = (f64::MAX, f64::MIN);
        for v in &mesh.verts {
            let d = proj(*v).1;
            dmin = dmin.min(d);
            dmax = dmax.max(d);
        }
        let span = (dmax - dmin).max(1e-6);
        // a body in interference gets a red fill (not in ghost mode)
        let clash = !ghost && pn.project.mesh_id(mi).is_some_and(|b| qymcad_ui_state::body_interferes(pn.interference, b));
        let mut tris: Vec<(f64, [Pos2; 3], Color32)> = Vec::with_capacity(mesh.tris.len());
        for i in 0..mesh.tris.len() {
            let t = mesh.triangle(i);
            // THE SECTION: an honest clip (the CPU path) - 0..2 sub-triangles exactly along the plane
            let clip = qymcad_ui_state::section_clip_tri(pn.section, [[t[0].x, t[0].y, t[0].z], [t[1].x, t[1].y, t[1].z], [t[2].x, t[2].y, t[2].z]]);
            if !clip.whole && clip.verts.is_empty() {
                continue;
            }
            let sub: Vec<[qymcad_core::geom::Point3; 3]> = if clip.whole {
                vec![[t[0], t[1], t[2]]]
            } else {
                (1..clip.verts.len().saturating_sub(1))
                    .map(|k| {
                        let g = |i: usize| qymcad_core::geom::Point3::new(clip.verts[i].pos[0], clip.verts[i].pos[1], clip.verts[i].pos[2]);
                        [g(0), g(k), g(k + 1)]
                    })
                    .collect()
            };
            for t in sub {
                let (a, da) = proj(t[0]);
                let (b, db) = proj(t[1]);
                let (c, dc) = proj(t[2]);
                let zc = (da + db + dc) / 3.0;
                let shade = ((zc - dmin) / span).clamp(0.0, 1.0) as f32;
                // Lighting: the floor comes from the palette, 1.0 at the nearest face. The palette stores the
                // body colour AS IT IS ON THE BRIGHTEST FACE - deeper down it simply fades proportionally.
                let k = qymcad_scheme::lit(pn.scheme.pal.shade_floor_mesh, shade);
                let col = if ghost {
                    // a ghost: dim and translucent, so the sketch on top of it reads
                    qymcad_scheme::a(qymcad_scheme::tint(pn.scheme.pal.body_ghost(), k), 72)
                } else if clash {
                    // the "collision" fill, keeping the shading by depth
                    qymcad_scheme::tint(pn.scheme.pal.body_clash(), k)
                } else {
                    qymcad_scheme::tint(pn.scheme.pal.body_face(), k)
                };
                tris.push((zc, [sh.at(a), sh.at(b), sh.at(c)], col));
            }
        }
        tris.sort_by(|a, b| a.0.total_cmp(&b.0)); // the bottom first, the top over it
        let mut emesh = egui::Mesh::default();
        for (_, pts, col) in &tris {
            let base = emesh.vertices.len() as u32;
            for p in pts {
                emesh.colored_vertex(*p, *col);
            }
            emesh.add_triangle(base, base + 1, base + 2);
        }
        if !emesh.is_empty() {
            painter.add(egui::Shape::mesh(emesh));
        }
    }
    // THE SECTION CAPS (the CPU path): an amber fill on top (sorted by depth within the caps)
    if pn.section.plane.is_some() {
        let caps = section_caps_for_frame(pn);
        let mut ctris: Vec<(f64, [Pos2; 3])> = Vec::new();
        let basis = pn.cam.basis();
        for mesh in caps.iter() {
            for t in 0..mesh.tris.len() {
                let tri = mesh.triangle(t);
                let pr = |p: qymcad_core::geom::Point3| qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at([p.x, p.y, p.z]);
                let (a, da) = pr(tri[0]);
                let (b, db) = pr(tri[1]);
                let (c, dc) = pr(tri[2]);
                ctris.push(((da + db + dc) / 3.0, [a, b, c]));
            }
        }
        ctris.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut cm = egui::Mesh::default();
        let col = pn.scheme.pal.cam_stock();
        for (_, pts) in &ctris {
            let base = cm.vertices.len() as u32;
            for p in pts {
                cm.colored_vertex(*p, col);
            }
            cm.add_triangle(base, base + 1, base + 2);
        }
        if !cm.is_empty() {
            painter.add(egui::Shape::mesh(cm));
        }
    }
}

/// A corner of a triangle on its way to the screen: where it stands in the world and its colour.
#[derive(Clone, Copy)]
struct Corner {
    at: [f64; 3],
    colour: Color32,
}

/// THE PART OF A TRIANGLE IN FRONT OF THE NEAR PLANE, as a polygon of up to four corners.
///
/// In perspective a point behind the eye has no place on the screen, and projected all the same it landed far out with
/// the nearest depth there is: a part standing behind the eye, 5 m along the line of sight from the one looked at at
/// 100 mm, smeared 139 128 pixels of its colour over the frame. The card clips at the near plane; the raster now does
/// the same. In orthographic nothing is behind the eye and the triangle is whole.
fn in_front(tri: &[Corner; 3], cam: &qymcad_ui_state::Cam3, basis: &([f64; 3], [f64; 3], [f64; 3]), proj: &qymcad_ui_state::ProjParams) -> Vec<Corner> {
    if proj.inv_d_eye <= 0.0 {
        return tri.to_vec();
    }
    let d_eye = 1.0 / proj.inv_d_eye;
    let ahead = |k: &Corner| qymcad_ui_state::v_dot(qymcad_ui_state::v_sub(k.at, cam.target), basis.2) + d_eye - proj.z_near;
    if tri.iter().all(|k| ahead(k) >= 0.0) {
        return tri.to_vec();
    }
    let mut out = Vec::with_capacity(4);
    for i in 0..3 {
        let (p, q) = (tri[i], tri[(i + 1) % 3]);
        let (dp, dq) = (ahead(&p), ahead(&q));
        if dp >= 0.0 {
            out.push(p);
        }
        if (dp >= 0.0) != (dq >= 0.0) {
            let t = dp / (dp - dq);
            let at = std::array::from_fn(|a| p.at[a] + (q.at[a] - p.at[a]) * t);
            let (cp, cq) = (p.colour.to_array(), q.colour.to_array());
            let ch = |c: usize| (cp[c] as f64 + (cq[c] as f64 - cp[c] as f64) * t).round() as u8;
            out.push(Corner { at, colour: Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3)) });
        }
    }
    out
}

/// Software rasterisation of the visible bodies and the floor into RGBA with a Z buffer. The buffer holds the
/// screen-linear depth of the card (`depth_ndc`) negated, so nearer is smaller as the raster compares, and the linear
/// interpolation in `raster_band` is exact both in orthographic and in perspective. The background is transparent.
pub fn rasterize_3d(pn: &Painting, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3]), ppp: f32, quality: f32) -> Option<egui::ColorImage> {
    let w = (rect.width() * ppp * quality).round() as usize;
    let h = (rect.height() * ppp * quality).round() as usize;
    if w == 0 || h == 0 || w.saturating_mul(h) > 16_000_000 {
        return None;
    }
    let lights = qymcad_ui_state::scene_lights(basis);
    let (_, _, fwd) = basis;
    let mut color = vec![Color32::TRANSPARENT; w * h];
    let mut zbuf = vec![f32::INFINITY; w * h];

    let smooth = pn.set.shading == qymcad_ui_state::Shading::Smooth;
    if smooth {
        qymcad_ui_state::ensure_vertex_normals(pn.cache, pn.project, pn.regen);
    }
    let ncache = pn.cache.norm.borrow();
    let items = qymcad_ui_state::visible_mesh_items(pn);
    // the depth of this frame, as the card takes it
    let proj = qymcad_ui_state::proj_params(pn, rect, qymcad_ui_state::gpu_scene_key(pn));
    let inv_d = proj.inv_d_eye;
    let rays = qymcad_ui_state::ViewRays::new(&pn.cam, rect, ppp * quality, proj);
    let look = qymcad_ui_state::floor_look(pn, rect);
    let floor = FloorFrame { rays: &rays, look: &look, size: [w, h] }.picture(&pn.cache.floor);

    // 1) project the visible front-facing triangles into pixel coordinates. The colour is PER VERTEX
    // (Gouraud): from the smoothed normal, interpolated in the raster; in flat mode the face normal goes
    // into all three.
    let (ox, oy) = (rect.min.x, rect.min.y);
    let pscale = ppp * quality;
    // the opaque ones (first pass, z-write) and the translucent ghosts (second pass, blend, no z-write)
    let mut tris: Vec<RasterTri> = Vec::new();
    let mut ghost_tris: Vec<RasterTri> = Vec::new();
    for qymcad_ui_state::SceneMesh { index: mi, hot, ghost, tint: base, mesh, world: wt } in items {
        let ident = qymcad_core::feature::is_identity12(&wt);
        let vn = if smooth { ncache.value.get(mi) } else { None };
        let (palette, per_tri) = qymcad_ui_state::face_palette(pn.project, mi, mesh.tris.len());
        for i in 0..mesh.tris.len() {
            let tri_idx = mesh.tris[i];
            // a face the file coloured apart is drawn in its colour, the rest in the body's
            let tint = per_tri.get(i).copied().flatten().map_or(base, |k| palette[k as usize]);
            let pw = |vi: u32| {
                let p = mesh.verts[vi as usize];
                let a = [p.x, p.y, p.z];
                if ident {
                    a
                } else {
                    qymcad_core::feature::apply12(&wt, a)
                }
            };
            let (a, b, c) = (pw(tri_idx[0]), pw(tri_idx[1]), pw(tri_idx[2]));
            let n = qymcad_ui_state::v_norm(qymcad_ui_state::v_cross(qymcad_ui_state::v_sub(b, a), qymcad_ui_state::v_sub(c, a)));
            // CULLING GOES BY THE RAY FROM THE EYE (in orthographic that is `fwd`): in perspective a
            // shared `fwd` lies towards the edges of the frame, the more so the wider the field of view
            // (gaps in a body, ribbons instead of a ring).
            if qymcad_ui_state::v_dot(n, qymcad_ui_state::view_dir_at(&pn.cam, a, *fwd, inv_d)) >= 0.0 {
                continue; // the bodies are oriented outwards
            }
            let col_at = |k: usize| -> Color32 {
                let nrm = match vn {
                    // a vertex normal pointing into the body (a sheet wound the other way) is turned to the eye with its face
                    Some(list) => {
                        let v = qymcad_ui_state::rotate_normal(&wt, list.at(i, k, tri_idx[k]));
                        if qymcad_ui_state::v_dot(v, n) < 0.0 {
                            [-v[0], -v[1], -v[2]]
                        } else {
                            v
                        }
                    }
                    None => n,
                };
                qymcad_pick::shade_tri(&pn.scheme.pal, pn.set.ghost_alpha, hot, ghost, tint, nrm, &lights)
            };
            let cols = [col_at(0), col_at(1), col_at(2)];
            let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis };
            let corners = [Corner { at: a, colour: cols[0] }, Corner { at: b, colour: cols[1] }, Corner { at: c, colour: cols[2] }];
            // the z buffer takes the depth the card takes, negated: nearer is smaller, as the raster compares
            let to_screen = |k: &Corner| {
                let (p, d) = scr.at(k.at);
                [(p.x - ox) * pscale, (p.y - oy) * pscale, -qymcad_ui_state::depth_ndc(d, &proj)]
            };
            let ahead = in_front(&corners, &pn.cam, basis, &proj);
            for k in 1..ahead.len().saturating_sub(1) {
                let tri = RasterTri { v: [to_screen(&ahead[0]), to_screen(&ahead[k]), to_screen(&ahead[k + 1])], cols: [ahead[0].colour, ahead[k].colour, ahead[k + 1].colour] };
                if ghost {
                    ghost_tris.push(tri)
                } else {
                    tris.push(tri)
                }
            }
        }
    }

    // 2) rasterisation with a Z buffer, in parallel over horizontal bands
    let nthreads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
    if nthreads <= 1 || h < 64 {
        raster_band(&mut color, &mut zbuf, w, 0, h, &tris);
        lay_floor(&mut color, &zbuf, &floor);
        raster_band_blend(&mut color, &zbuf, w, 0, h, &ghost_tris);
    } else {
        let band = h.div_ceil(nthreads);
        std::thread::scope(|s| {
            let (mut crest, mut zrest) = (&mut color[..], &mut zbuf[..]);
            let mut y0 = 0;
            let tref = &tris;
            let gref = &ghost_tris;
            let mut frest = &floor[..];
            while y0 < h {
                let rows = band.min(h - y0);
                let (cb, c2) = crest.split_at_mut(rows * w);
                let (zb, z2) = zrest.split_at_mut(rows * w);
                crest = c2;
                zrest = z2;
                let (fb, f2) = frest.split_at(rows * w);
                frest = f2;
                let y = y0;
                // within a band: the opaque ones first (they fill z), the floor where none is, then the ghosts on top
                // (z test, blend)
                s.spawn(move || {
                    raster_band(cb, zb, w, y, rows, tref);
                    lay_floor(cb, zb, fb);
                    raster_band_blend(cb, zb, w, y, rows, gref);
                });
                y0 += rows;
            }
        });
    }
    Some(egui::ColorImage { size: [w, h], source_size: egui::Vec2::new([w, h][0] as f32, [w, h][1] as f32), pixels: color })
}
