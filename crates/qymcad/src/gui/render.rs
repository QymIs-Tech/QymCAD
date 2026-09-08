//! DRAWING - everything that paints the scene, the gizmos, the highlights and the overlays.

pub(crate) use qymcad_ui_state::DrawCtx;
pub(crate) use qymcad_render::*;
use super::*;

impl App {
    /// THE ONE PLACE THE BORROWS ARE SPLIT for drawing in three dimensions.
    ///
    /// Five shared borrows handed out together instead of the application entire. Every `draw_*` below
    /// reads exactly these and nothing else, so once it takes the context it stops being a method.
    pub(super) fn draw_ctx(&self) -> DrawCtx<'_> {
        DrawCtx { cam: &self.viewing.cam, set: &self.set, scheme: &self.scheme, project: &self.project, active_path: &self.active_path }
    }


    pub(super) fn draw_contours(&self, painter: &egui::Painter, rect: Rect) {
        draw_contours(&self.painting(), painter, rect)
    }

    /// Highlighting the edges of the reference body in the 2D sketcher - the edges only (no fill, no 3D body).
    pub(super) fn draw_sketch_face_edges(&self, painter: &egui::Painter, rect: Rect) {
        draw_sketch_face_edges(&self.painting(), painter, rect)
    }

    /// Highlighting the selected entities + the glyphs of the geometric constraints.
    pub(super) fn draw_sketch_constraints(&self, painter: &egui::Painter, rect: Rect, si: usize) {
        draw_sketch_constraints(&self.painting(), painter, rect, si)
    }


    pub(super) fn draw_sketch_preview(&self, painter: &egui::Painter, rect: Rect) {
        draw_sketch_preview(&self.painting(), painter, rect)
    }

    /// The hover preview for trim, extend and break: what will happen if the entity under the cursor is
    /// clicked. Trim lights the span that will be removed in red; break puts a marker at the point; extend
    /// lights in green the end that will be pulled.
    pub(super) fn draw_trim_preview(&self, painter: &egui::Painter, rect: Rect) {
        draw_trim_preview(&self.painting(), painter, rect)
    }

    /// The pattern preview: ghosts of the copies from the row's current parameters. The source is either the
    /// selection (for a new pattern) or the source of the pattern being edited.
    pub(super) fn draw_pattern_preview(&self, painter: &egui::Painter, rect: Rect) {
        draw_pattern_preview(&self.painting(), painter, rect)
    }





    /// Waiting for a base point after Ctrl+C/X: the selected geometry is lit green (a hint that this is what
    /// will be copied) and a green crosshair is drawn under the cursor (a hint to click the base point).
    pub(super) fn draw_clip_pending(&self, painter: &egui::Painter, rect: Rect) {
        draw_clip_pending(&self.painting(), painter, rect)
    }


    pub(super) fn draw_move_preview(&self, painter: &egui::Painter, rect: Rect) {
        draw_move_preview(&self.painting(), painter, rect)
    }

    /// Draw the associative dimensions and constraints of the selected sketch in the viewport.
    pub(super) fn draw_sketch_dims(&self, painter: &egui::Painter, rect: Rect, si: usize) {
        draw_sketch_dims(&self.painting(), painter, rect, si)
    }


    pub(super) fn draw_mesh(&self, painter: &egui::Painter, rect: Rect) {
        draw_mesh(&self.painting(), painter, rect)
    }


    pub(super) fn draw_3d(&self, painter: &egui::Painter, rect: Rect) {
        draw_3d(&self.painting(), painter, rect)
    }


























































































































}

/// THE PADDING INSIDE THE REBUILD CARD, and the room its parts take. Named once, because the size of the
/// card and the places of its pieces have to agree - when they did not, the text stood outside the card.


/// THE ONE PLACE THE BORROWS ARE SPLIT for drawing in three dimensions.
///
/// Five shared borrows handed out together instead of the application entire. Every `draw_*` below
/// reads exactly these and nothing else, so once it takes the context it stops being a method.

/// The GPU pass over the bodies: it pushes a paint callback into the viewport rect. The vertices are
/// re-uploaded only when `gpu_scene_key` changes; while orbiting, only the camera uniform is updated.
pub(crate) fn draw_3d_gpu(pn: &qymcad_ui_state::Painting, painter: &egui::Painter, rect: Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) {
    let ppp = painter.ctx().pixels_per_point();
    let key = qymcad_ui_state::gpu_scene_key(pn);
    let (inv_d, z_near, z_far, _) = proj_params(pn, rect, key);
    let cam = crate::viewport_gpu::CamRaw::new(basis, pn.cam.scale, pn.cam.target, rect.size(), inv_d as f32, crate::viewport_gpu::ZRange { near: z_near as f32, far: z_far as f32 });
    let size_px = [(rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32];
    let (verts, opaque_count) = if pn.cache.gpu_scene_key.get() != key {
        pn.cache.gpu_scene_key.set(key);
        let (v, oc) = render_scene::gpu_scene(pn);
        (Some(std::sync::Arc::new(v)), oc)
    } else {
        (None, 0)
    };
    painter.add(eframe::egui_wgpu::Callback::new_paint_callback(rect, crate::viewport_gpu::MeshPaint::new(cam, size_px, verts, opaque_count, key)));
}

pub(crate) fn draw_3d(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let basis = pn.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect: rect, basis: &basis };
    let p3 = |p: [f64; 3]| scr.at(p).0;


    // THE FLOOR GRID at Z=0 is the bearing in the 3D view (NOT the machine table: that one sits under
    // cam_mode). A minor line every `step`, a major one every 5; drawn UNDER the geometry (before the
    // raster mesh), plus the coloured global axes. The grid is CENTRED on the look-at point (cam.target)
    // rather than on the world origin, and the number of lines is computed from the screen size. Otherwise
    // on large projects (a machine over 1 m, coordinates far from the origin) the machine ended up at the
    // faded edge of the grid or beyond it, and looked like it was hanging in the air.
    {
        // an adaptive step for the zoom: the on-screen interval of a line is at least ~9px (as in the sketch grid), a major line every 5
        let sc = pn.cam.scale as f64;
        let mut step = 10.0_f64;
        while step * sc < 9.0 {
            step *= 5.0;
        }
        // A smooth LOD: near the switching threshold (step x 5) the minor lines become ~9px and crowd
        // together, then vanish abruptly when the step jumps. The minor lines are faded as the interval
        // approaches the threshold (9 -> 18px: 0 -> 1) while the major ones (every 5) stay solid, so the
        // scale changes without a jerk.
        let px = step * sc; // the on-screen interval of a minor line, in [9, 45)
        let minor_lod = (((px - 9.0) / 9.0).clamp(0.0, 1.0)) as f32;
        // the grid centre is the look-at point projected onto the Z=0 floor and snapped to the step (the lines stay put while panning)
        let cx = (pn.cam.target[0] / step).round() * step;
        let cy = (pn.cam.target[1] / step).round() * step;
        // the number of lines each way: cover half the screen diagonal (orbiting tilts the floor) plus a
        // margin, with a ceiling so that drawing does not blow up when zoomed far out
        let reach_px = (rect.width().hypot(rect.height()) * 0.5) as f64;
        let n = (((reach_px / (step * sc)).ceil() as i32) + 2).clamp(8, 160);
        let lim = step * n as f64;
        for i in -n..=n {
            let tx = cx + i as f64 * step;
            let ty = cy + i as f64 * step;
            // a fade towards the edges of the grid (a soft vignette, so the lines do not break off hard)
            let fade = (1.0 - (i.abs() as f32 / n as f32).powi(2)).clamp(0.0, 1.0);
            // a major line goes by an absolute coordinate that is a multiple of 5 * step (so the fives stay put while panning)
            let major_x = ((tx / step).round() as i64).rem_euclid(5) == 0;
            let major_y = ((ty / step).round() as i64).rem_euclid(5) == 0;
            let stroke = |major: bool| {
                let (base, a) = if major { (pn.scheme.pal.grid(), 230.0) } else { (pn.scheme.pal.grid_minor(), 170.0) };
                // the minor lines fade further by the LOD near the step's switching threshold
                let lod = if major { 1.0 } else { minor_lod };
                Stroke::new(1.0, qymcad_scheme::a(base, (a * fade * lod) as u8))
            };
            // a line along Y (at x=tx) and one along X (at y=ty), stretched over the grid's reach around
            // the centre. The grey line at zero is not drawn - a coloured axis runs there instead (green Y
            // at x=0, red X at y=0).
            if tx.abs() > step * 0.5 {
                painter.line_segment([p3([tx, cy - lim, 0.0]), p3([tx, cy + lim, 0.0])], stroke(major_x));
            }
            if ty.abs() > step * 0.5 {
                painter.line_segment([p3([cx - lim, ty, 0.0]), p3([cx + lim, ty, 0.0])], stroke(major_y));
            }
        }
        // the coloured global axes: X red, Y green (on the floor), Z blue upwards; stretched across the
        // grid's current reach around the look-at point (visible for as long as the origin is in view).
        painter.line_segment([p3([cx - lim, 0.0, 0.0]), p3([cx + lim, 0.0, 0.0])], Stroke::new(1.6, pn.scheme.pal.grid_axis_x()));
        painter.line_segment([p3([0.0, cy - lim, 0.0]), p3([0.0, cy + lim, 0.0])], Stroke::new(1.6, pn.scheme.pal.grid_axis_y()));
        painter.line_segment([p3([0.0, 0.0, 0.0]), p3([0.0, 0.0, step * 4.0])], Stroke::new(1.8, pn.scheme.pal.grid_axis_z()));
    }


    // the mesh: either the GPU pass (wgpu, a depth buffer) or the CPU fallback (software rasterisation
    // with a Z buffer - an exact per-pixel order, with none of the painter's-algorithm artefacts on
    // coplanar faces).
    if pn.gpu_ok && pn.set.gpu_viewport {
        // a paint callback into the rect, UNDER the 2D overlays. The CPU cache is not involved.
        draw_3d_gpu(pn, painter, rect, &basis);
    } else {
        // The texture is cached by the view key and redrawn only when that changes.
        let ppp = painter.ctx().pixels_per_point();
        let quality = if pn.view_dragging { 0.5 } else { 1.0 };
        let key = qymcad_ui_state::view_key(pn, rect, ppp);
        let cached = pn.cache.view.borrow().as_ref().map(|(k, _)| *k) == Some(key);
        if !cached {
            if let Some(img) = rasterize_3d(pn, rect, &basis, ppp, quality) {
                let filter = if quality < 1.0 { egui::TextureOptions::LINEAR } else { egui::TextureOptions::NEAREST };
                let tex = painter.ctx().load_texture("qym_view3d", img, filter);
                *pn.cache.view.borrow_mut() = Some((key, tex));
            } else {
                *pn.cache.view.borrow_mut() = None;
            }
        }
        if let Some((_, tex)) = pn.cache.view.borrow().as_ref() {
            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        }
    }

    // the sketch outlines go OVER the bodies (an overlay), so that they can be seen at all.
    // In an assembly they can be switched off with a single toggle; that toggle does not hide the sketch
    // being edited or picked as a command's profile - what is invisible cannot be selected.
    let forced_cids = active_sketch_contour_ids(pn.armed, pn.cmd, pn.project, pn.sketch_ses);
    if forced_cids.is_some() || !contours_switched_off(pn.set, pn.workbench) {
        let selected: &[Id] = &[];
        let obj_sel = if let Sel::Contour(c) = pn.sel { Some(c) } else { None };
        let sketch_sel = if let Sel::Sketch(s) = pn.sel { pn.project.sketches.get(s).map(|sk| sk.contour_ids.clone()) } else { None };
        // hidden sketches (the checkbox is off) are not drawn - the same computation as in the sketcher
        let hidden_cids = hidden_contour_ids(pn.project, pn.sketch_hidden);
        // the sketches of OTHER components (outside the active context) are not drawn - sketch isolation
        let foreign_cids = qymcad_pick::foreign_contour_ids(pn);
        // the outlines of sketches on NON-world planes are drawn lifted onto their own plane
        let mut cframe: std::collections::HashMap<usize, qymcad_core::feature::PlaneFrame> = std::collections::HashMap::new();
        for si in 0..pn.project.sketches.len() {
            if let Some(f) = pn.project.sketch_frame(si) {
                if !f.is_identity() {
                    for &cid in &pn.project.sketches[si].contour_ids {
                        if let Some(ci) = pn.project.contour_index(cid) {
                            cframe.insert(ci, f);
                        }
                    }
                }
            }
        }
        for (ci, c) in pn.project.contours.iter().enumerate() {
            if c.points.len() < 2 {
                continue;
            }
            let cid = pn.project.contour_id(ci);
            if cid.is_some_and(|id| hidden_cids.contains(&id)) {
                continue; // the sketch is hidden by its checkbox
            }
            if cid.is_some_and(|id| foreign_cids.contains(&id)) {
                continue; // a sketch of another component - outside the active context
            }
            if contours_switched_off(pn.set, pn.workbench) && !forced_cids.as_ref().is_some_and(|only| cid.is_some_and(|id| only.contains(&id))) {
                continue; // the assembly toggle is off: only the sketch currently in hand is visible
            }
            // a selection in the tree (a contour or a sketch) is bright green; one in an operation is yellow
            let in_sketch = sketch_sel.as_ref().is_some_and(|ids| cid.is_some_and(|id| ids.contains(&id)));
            // THE SKETCH UNDER THE CURSOR while a tool waits for one - lit the way a face or an edge is lit
            // under the tools that ask for those, so that pointing at it looks like pointing at anything else.
            let hovered = pn.hover.sketch_3d.and_then(|hi| pn.project.sketches.get(hi)).is_some_and(|sk| cid.is_some_and(|id| sk.contour_ids.contains(&id)));
            let (w, col) = if hovered {
                (2.5, pn.scheme.pal.highlight())
            } else if obj_sel == Some(ci) || in_sketch {
                (2.5, pn.scheme.pal.ok())
            } else if cid.is_some_and(|id| selected.contains(&id)) {
                (2.0, pn.scheme.pal.active())
            } else {
                (1.5, pn.scheme.pal.sketch_edge_3d())
            };
            let st = Stroke::new(w, col);
            let n = c.points.len();
            let last = if c.closed { n } else { n - 1 };
            let lift = |q: Point2| -> [f64; 3] {
                match cframe.get(&ci) {
                    Some(f) => {
                        let w = f.lift(q);
                        [w.x, w.y, w.z]
                    }
                    None => [q.x, q.y, 0.0],
                }
            };
            for k in 0..last {
                let a = c.points[k];
                let b = c.points[(k + 1) % n];
                painter.line_segment([p3(lift(a)), p3(lift(b))], st);
            }
        }
    }

    // the edges of the selected body (for picking under a chamfer or a fillet)
    draw_body_edges(pn, painter, rect);
    // the live wireframe preview of the active command (extrude, cut, ...) + the length arrow.
    // There is no permanent gizmo at the selected feature - editing goes through a double click in the tree.
    draw_feat_cmd_preview(pn, painter, rect);
    draw_sketch_plane_picker(pn, painter, rect);
    draw_joint_pick_highlight(pn, painter, rect);
    draw_component_gizmo(pn, painter, rect);
    draw_body_gizmo(pn, painter, rect); // the body gizmo inside a Part
    draw_section_gizmo(pn, painter, rect); // the section plane + the offset arrow
    draw_joints(pn, painter, rect); // the mate glyphs
    draw_grounded_glyphs(pn, painter, rect); // the anchor at the grounded parts

    // face highlighting happens ONLY under the Shell/Hole command (outside them a face is not pickable).
    // Shell (6) lights EVERY face of the multi-selection by persistent id; Hole (7) lights one Sel::Face.
    let fill_face = |painter: &egui::Painter, face: &qymcad_core::geom::MeshFace, mesh: &qymcad_core::geom::Mesh| {
        let mut hm = egui::Mesh::default();
        for &ti in &face.triangles {
            let t = mesh.triangle(ti as usize);
            let base = hm.vertices.len() as u32;
            for v in &t {
                hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, pn.scheme.pal.selected());
            }
            hm.add_triangle(base, base + 1, base + 2);
        }
        if !hm.is_empty() {
            painter.add(egui::Shape::mesh(hm));
        }
    };
    if pn.armed.cmd_kind() == 6 {
        for (mi, faces) in pn.project.bodies.iter().map(|b| &b.faces).enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue; // only the target body's faces are lit (the ids are local to a body)
            }
            if let Some(mesh) = pn.project.bodies.get(mi).map(|b| &b.mesh) {
                for face in faces.iter().filter(|f| pn.gsel.faces.contains(&f.id)) {
                    fill_face(painter, face, mesh);
                }
            }
        }
    } else if pn.armed.cmd_kind() == 7 {
        if pn.hole.mode == 1 {
            // the "from a sketch" mode: diameter circles at every isolated marker point of the sketch
            if let Some(sid) = pn.hole.sketch {
                let dia = qymcad_ui_state::cmd_val(pn.cmd, "diameter").max(0.1);
                let col = pn.scheme.pal.preview();
                for at in pn.project.sketch_isolated_points(sid) {
                    let sc = p3(at);
                    // a cross + a circle of radius diameter/2 (roughly, in screen scale via an offset point)
                    painter.line_segment([sc + egui::vec2(-6.0, 0.0), sc + egui::vec2(6.0, 0.0)], Stroke::new(1.5, col));
                    painter.line_segment([sc + egui::vec2(0.0, -6.0), sc + egui::vec2(0.0, 6.0)], Stroke::new(1.5, col));
                    // the radius on screen: project a point offset along the sketch's X
                    let rp = (scr.at([at[0] + dia * 0.5, at[1], at[2]]).0 - sc).length();
                    painter.circle_stroke(sc, rp.max(2.0), Stroke::new(1.5, col));
                }
            }
        } else if let Sel::Face(mi, fi) = pn.sel {
            if let (Some(face), Some(mesh)) = (pn.project.bodies.get(mi).and_then(|b| b.faces.get(fi)), pn.project.bodies.get(mi).map(|b| &b.mesh)) {
                fill_face(painter, face, mesh);
            }
        }
    } else if pn.armed.cmd_kind() == 28 {
        // THICKEN: the selected face is lit + a PREVIEW of the plate - the face outline shifted by the
        // thickness. The sign is visible at once: whether the material goes outwards or inwards.
        let t = qymcad_ui_state::cmd_val(pn.cmd, "thickness");
        for (mi, body) in pn.project.bodies.iter().enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue;
            }
            let Some(face) = body.faces.iter().find(|f| pn.gsel.faces.contains(&f.id)) else { continue };
            let mesh = &body.mesh;
            let mut hm = egui::Mesh::default();
            for &ti in &face.triangles {
                let tri = mesh.triangle(ti as usize);
                let base = hm.vertices.len() as u32;
                for v in &tri {
                    hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(pn.scheme.pal.add(), 110));
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
            // THE OFFSET OUTLINE - where the plate's second surface will go
            let n = face.normal;
            let off = |p: [f64; 3]| [p[0] + n[0] * t, p[1] + n[1] * t, p[2] + n[2] * t];
            let st = Stroke::new(1.6, pn.scheme.pal.add());
            for &ti in &face.triangles {
                let tri = mesh.triangle(ti as usize);
                for k in 0..3 {
                    let (a, b) = (tri[k], tri[(k + 1) % 3]);
                    let (pa, pb) = (off([a.x, a.y, a.z]), off([b.x, b.y, b.z]));
                    painter.line_segment([scr.at(pa).0, scr.at(pb).0], st);
                }
            }
        }
    } else if matches!(pn.armed.cmd_kind(), 30 | 31) {
        // COPY FACE (30) and REPLACE FACE (31): show the selection TO THE EYE, not only in the model.
        //
        // The first edition gathered faces silently - nothing changed on the screen, and that read, fairly,
        // as "no face gets selected at all". A tool without a preview is blind: a person sees neither what
        // they picked nor what will come of it.
        //
        // The colour carries the MEANING: for a copy it is "will be added" (a surface appears), for a
        // replacement "will go" (a sheet takes these faces' place). The sheet surface itself is lit separately.
        let taken = if pn.armed.cmd_kind() == 30 { pn.scheme.pal.add() } else { pn.scheme.pal.remove() };
        for (mi, body) in pn.project.bodies.iter().enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue;
            }
            let mesh = &body.mesh;
            let mut hm = egui::Mesh::default();
            for face in body.faces.iter().filter(|f| pn.gsel.faces.contains(&f.id)) {
                for &ti in &face.triangles {
                    let t = mesh.triangle(ti as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(taken, 120));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        }
        // THE SELECTED SURFACE (31) whole, so that it is visible WHAT exactly will return to the body
        if let Some(surf) = pn.repl_surface {
            if let Some(mi) = pn.project.mesh_index(surf) {
                let mesh = &pn.project.bodies[mi].mesh;
                let mut hm = egui::Mesh::default();
                for ti in 0..mesh.tris.len() {
                    let t = mesh.triangle(ti);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(pn.scheme.pal.add(), 130));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
                if !hm.is_empty() {
                    painter.add(egui::Shape::mesh(hm));
                }
            }
        }
    } else if pn.armed.cmd_kind() == 34 {
        // TRIM: the sheet being kept in the "stays" colour, the tool in the "goes" colour.
        for (body, add) in [(pn.trim.keep.map(|(b, _)| b), true), (pn.trim.tool, false)] {
            let Some(mi) = body.and_then(|b| pn.project.mesh_index(b)) else { continue };
            let mesh = &pn.project.bodies[mi].mesh;
            let mut hm = egui::Mesh::default();
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let base = hm.vertices.len() as u32;
                let col = if add { pn.scheme.pal.add() } else { pn.scheme.pal.remove() };
                for v in &t {
                    hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(col, 120));
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        }
        // and the SPOT that was clicked: it is what decides which piece stays
        if let Some((_, at)) = pn.trim.keep {
            let p = scr.at(at).0;
            painter.circle_filled(p, 5.0, pn.scheme.pal.add());
        }
    } else if pn.armed.cmd_kind() == 33 {
        // STITCH: the selected sheets filled in the "will be added" colour. A person must see WHAT
        // exactly will become one surface: clicking blind into an invisible set is not acceptable.
        for part in pn.stitch_parts {
            let Some(mi) = pn.project.mesh_index(*part) else { continue };
            let mesh = &pn.project.bodies[mi].mesh;
            let mut hm = egui::Mesh::default();
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let base = hm.vertices.len() as u32;
                for v in &t {
                    hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(pn.scheme.pal.add(), 130));
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        }
    } else if pn.armed.cmd_kind() == 26 {
        // DELETE FACE: the selected faces filled in red (what will cease to exist).
        for (mi, body) in pn.project.bodies.iter().enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue;
            }
            let mesh = &body.mesh;
            let mut hm = egui::Mesh::default();
            for face in body.faces.iter().filter(|f| pn.gsel.faces.contains(&f.id)) {
                for &ti in &face.triangles {
                    let t = mesh.triangle(ti as usize);
                    let base = hm.vertices.len() as u32;
                    for v in &t {
                        hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(pn.scheme.pal.remove(), 120));
                    }
                    hm.add_triangle(base, base + 1, base + 2);
                }
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        }
    } else if pn.armed.cmd_kind() == 25 {
        // PUSH FACE: the selected face is lit + a PREVIEW of the result - the face outline shifted by the
        // given offset, with the edges of the future prism between them. Without this the tool was blind:
        // neither what was picked nor where it would move was clear.
        let dist = qymcad_ui_state::cmd_val(pn.cmd, "dist");
        for (mi, body) in pn.project.bodies.iter().enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue;
            }
            let Some(face) = body.faces.iter().find(|f| pn.gsel.faces.contains(&f.id)) else { continue };
            let mesh = &body.mesh;
            // the face itself, filled in orange (as for the shell and the draft)
            let mut hm = egui::Mesh::default();
            for &ti in &face.triangles {
                let t = mesh.triangle(ti as usize);
                let base = hm.vertices.len() as u32;
                for v in &t {
                    hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, qymcad_scheme::a(pn.scheme.pal.modify(), 110));
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
            // THE PREVIEW: the same face shifted along the normal by dist, dashed, plus posts at the corners
            if dist.abs() > 1e-9 {
                let n = face.normal;
                let off = |v: [f64; 3]| [v[0] + n[0] * dist, v[1] + n[1] * dist, v[2] + n[2] * dist];
                let col = if dist > 0.0 { pn.scheme.pal.ok() } else { pn.scheme.pal.offset_in() };
                for &ti in &face.triangles {
                    let t = mesh.triangle(ti as usize);
                    let p: Vec<Pos2> = t.iter().map(|v| scr.at(off([v.x, v.y, v.z])).0).collect();
                    for k in 0..3 {
                        painter.add(egui::Shape::dashed_line(&[p[k], p[(k + 1) % 3]], Stroke::new(1.2, col), 5.0, 4.0));
                    }
                }
                // the direction: an arrow from the face centre to the shifted centre
                let c = [face.centroid.x, face.centroid.y, face.centroid.z];
                let (a, b) = (scr.at(c).0, scr.at(off(c)).0);
                painter.add(egui::Shape::line_segment([a, b], Stroke::new(2.0, col)));
                painter.circle_filled(b, 3.5, col);
            }
        }
    } else if pn.armed.cmd_kind() == 23 {
        // Draft: the faces being tilted in orange (as for the shell), the neutral face in blue.
        let fill_col = |painter: &egui::Painter, face: &qymcad_core::geom::MeshFace, mesh: &qymcad_core::geom::Mesh, col: Color32| {
            let mut hm = egui::Mesh::default();
            for &ti in &face.triangles {
                let t = mesh.triangle(ti as usize);
                let base = hm.vertices.len() as u32;
                for v in &t {
                    hm.colored_vertex(scr.at([v.x, v.y, v.z]).0, col);
                }
                hm.add_triangle(base, base + 1, base + 2);
            }
            if !hm.is_empty() {
                painter.add(egui::Shape::mesh(hm));
            }
        };
        for (mi, faces) in pn.project.bodies.iter().map(|b| &b.faces).enumerate() {
            if pn.project.mesh_id(mi) != pn.gsel.faces_body {
                continue; // the draft and neutral faces come ONLY from the target body (the ids are local to a body)
            }
            if let Some(mesh) = pn.project.bodies.get(mi).map(|b| &b.mesh) {
                for face in faces.iter() {
                    if pn.draft.neutral != 0 && face.id == pn.draft.neutral {
                        fill_col(painter, face, mesh, pn.scheme.pal.reference());
                    } else if pn.gsel.faces.contains(&face.id) {
                        fill_col(painter, face, mesh, pn.scheme.pal.selected());
                    }
                }
            }
        }
    }
    // THE COMMAND HANDLE - one for every tool that has a direction and a distance along it (push face,
    // thicken, shell, both splits). It is drawn AFTER the preview branches so that no call has to be
    // added inside each of them: those would drift apart, the way the popups once did.
    draw_face_arrow(pn, painter, rect, &basis);
    draw_fillet_vertices(pn, painter, rect); // the points a per-vertex radius is set at
    // the wireframe preview of the primitive being created
    draw_prim_preview(pn, painter, rect);
    draw_array_preview(pn, painter, rect);
    draw_comp_array_preview(pn, painter, rect);
    draw_measure_3d(pn, painter, rect);
    draw_mirror_preview(pn, painter, rect);
    draw_split_preview(pn, painter, rect);
    draw_axis_picker(pn, painter, rect);
    draw_datum_preview(pn, painter, rect);

    // the work planes
    for (pi, pl) in pn.project.planes.iter().enumerate() {
        // as for points and axes: a datum plane is drawn ONLY when it is visible in the context and not
        // hidden by its checkbox (datum_render_transform returns None otherwise). The origin and normal
        // live in the owner's frame and are carried into the view.
        let Some(wt) = qymcad_ui_state::datum_render_transform(pn, pl.id) else { continue };
        let ident = qymcad_core::feature::is_identity12(&wt);
        let o = if ident { pl.origin } else { qymcad_core::feature::apply12(&wt, pl.origin) };
        let n = qymcad_ui_state::v_norm(if ident { pl.normal } else { qymcad_core::feature::apply12_dir(&wt, pl.normal) });
        let ax = if n[0].abs() < 0.9 { qymcad_ui_state::v_norm(qymcad_ui_state::v_cross(n, [1.0, 0.0, 0.0])) } else { qymcad_ui_state::v_norm(qymcad_ui_state::v_cross(n, [0.0, 1.0, 0.0])) };
        let ay = qymcad_ui_state::v_cross(n, ax);
        let s = 25.0;
        let corner = |sx: f64, sy: f64| [o[0] + ax[0] * sx + ay[0] * sy, o[1] + ax[1] * sx + ay[1] * sy, o[2] + ax[2] * sx + ay[2] * sy];
        let c = [corner(-s, -s), corner(s, -s), corner(s, s), corner(-s, s)];
        let sel = pn.sel == Sel::Plane(pi);
        let col = if sel { pn.scheme.pal.highlight() } else { pn.scheme.pal.plane_idle() };
        let st = Stroke::new(if sel { 2.0 } else { 1.0 }, col);
        for k in 0..4 {
            painter.line_segment([p3(c[k]), p3(c[(k + 1) % 4])], st);
        }
        // the normal
        let tip = [o[0] + n[0] * 20.0, o[1] + n[1] * 20.0, o[2] + n[2] * 20.0];
        painter.line_segment([p3(o), p3(tip)], Stroke::new(1.5, pn.scheme.pal.plane_normal()));
    }

    // datum POINTS (a cross marker) and datum AXES (a segment) are drawn in 3D, the selected one brighter.
    // Datums travel with their own part in an assembly (the owner's transform), and isolation hides the
    // components of others.
    let sel_col = pn.scheme.pal.highlight();
    let dap = |wt: &[f64; 12], v: [f64; 3]| if qymcad_core::feature::is_identity12(wt) { v } else { qymcad_core::feature::apply12(wt, v) };
    for (i, dp) in pn.project.datum_points.iter().enumerate() {
        let Some(wt) = qymcad_ui_state::datum_render_transform(pn, dp.id) else { continue };
        let s = p3(dap(&wt, dp.at));
        let sel = pn.sel == Sel::DatumPoint(i);
        let col = if sel { sel_col } else { pn.scheme.pal.datum_point() };
        let r = if sel { 6.0 } else { 4.5 };
        painter.line_segment([s + egui::vec2(-r, 0.0), s + egui::vec2(r, 0.0)], Stroke::new(1.6, col));
        painter.line_segment([s + egui::vec2(0.0, -r), s + egui::vec2(0.0, r)], Stroke::new(1.6, col));
        painter.circle_stroke(s, r * 0.7, Stroke::new(1.0, col));
    }
    for (i, da) in pn.project.datum_axes.iter().enumerate() {
        let Some(wt) = qymcad_ui_state::datum_render_transform(pn, da.id) else { continue };
        let o = dap(&wt, da.origin());
        let d = qymcad_ui_state::v_norm(if qymcad_core::feature::is_identity12(&wt) { da.dir() } else { qymcad_core::feature::apply12_dir(&wt, da.dir()) });
        let l = 45.0;
        let a = [o[0] - d[0] * l, o[1] - d[1] * l, o[2] - d[2] * l];
        let b = [o[0] + d[0] * l, o[1] + d[1] * l, o[2] + d[2] * l];
        let sel = pn.sel == Sel::DatumAxis(i);
        let col = if sel { sel_col } else { pn.scheme.pal.datum_axis() };
        painter.line_segment([p3(a), p3(b)], Stroke::new(if sel { 2.6 } else { 1.5 }, col));
    }

}
