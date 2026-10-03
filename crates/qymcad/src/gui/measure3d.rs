//! MEASURING IN 3D — resolving a click into the element being measured, and the tool itself.
//!
//! The arithmetic lives in the kernel (`qymcad_core::measure`) and is checked on known geometry;
//! what is here is only "what did the cursor hit". Measuring used to be possible ONLY in a sketch and
//! only between two points on a plane: the gap between parts, the distance between faces, the angle
//! of convergence and the diameter of a hole in 3D had nothing to measure them with.
pub use qymcad_ui_state::MeasurePick;
pub(crate) use qymcad_ui_state::measure_text;
use super::{App, Id};
use egui::{Pos2, Rect};
use qymcad_core::feature::{apply12, apply12_dir};
use qymcad_core::measure::{MeasureItem};

impl App {
    /// THE RESULT TEXT — the only place where the numbers turn into a string (the status line and the
    /// plate at the geometry say the same thing: two wordings of one measurement drift apart
    /// silently).
    pub(super) fn measure_text(&self) -> String {
        measure_text(&self.painting())
    }

    /// SWITCH the 3D measuring tool ON or OFF.
    pub(super) fn toggle_measure_3d(&mut self) {
        let on = !self.side.m3.on;
        self.cancel_all_tools(); // exclusivity: the measuring tool puts down the previous one
        self.side.m3.clear();
        self.side.m3.on = on;
        if on {
            self.viewing.mode_3d = true;
            self.status = crate::i18n::tr("m3-hint");
        }
    }

    /// A click of the measuring tool: resolve the hit and update the result.
    pub(super) fn measure_3d_click(&mut self, rect: Rect, pos: Pos2) {
        let Some(p) = self.measure_resolve(rect, pos) else {
            self.status = crate::i18n::tr("m3-miss");
            return;
        };
        self.side.m3.take(p);
        self.status = if self.side.m3.picks.is_empty() { crate::i18n::tr("m3-hint") } else { self.measure_text() };
    }



    /// WHAT THE CURSOR HIT: vertex -> edge -> face.
    ///
    /// The order is exactly that (from small to large), as in every CAD: a vertex lies ON an edge and
    /// an edge on a face, so "the nearest thing of all" would always give the face, and neither a
    /// vertex nor an edge could ever be picked.
    pub(super) fn measure_resolve(&mut self, rect: Rect, pos: Pos2) -> Option<MeasurePick> {
        // WHAT IS OCCLUDED IS NOT PICKED. An isometric view folds the far bottom corner of a part
        // exactly onto the middle of its own top face: without a depth check a click on a visible face
        // returned THE EDGE ON THE FAR SIDE, and instead of the thickness of the part a diagonal came
        // out. A small element beats a face only if it is IN FRONT of it (or on it — the silhouette).
        let in_front = |me: &Self, w: [f64; 3]| crate::gui::pick::point_not_hidden(&me.painting(), rect, w);
        if let Some(w) = crate::gui::pick::pick_vertex_pos(&self.painting(), rect, pos) {
            if in_front(self, w) {
                return Some(MeasurePick { item: MeasureItem::Point(w), what: crate::i18n::tr("m3-vertex"), at: w });
            }
        }
        if let Some(p) = self.measure_edge_at(rect, pos) {
            if in_front(self, p.at) {
                return Some(p);
            }
        }
        self.measure_face_at(rect, pos)
    }

    /// The edge under the cursor -> a line or a circle in the WORLD coordinates of the active context.
    fn measure_edge_at(&mut self, rect: Rect, pos: Pos2) -> Option<MeasurePick> {
        let basis = self.viewing.cam.basis();
        let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
        let mut best: Option<(f32, Id, u32)> = None;
        for (_mi, body) in crate::gui::pick::shown_bodies(&self.painting()) {
            if !crate::gui::pick::body_bbox_hit(&self.painting(), body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = crate::gui::pick::body_edges_cached(&self.cache, &self.live, &self.regen, body) else { continue };
            let wt = self.project.body_display_transform(body, ctx);
            for (poly, id) in edges.polys.iter().zip(edges.ids.iter().copied()) {
                if id == 0 {
                    continue;
                }
                let pts: Vec<Pos2> = poly.iter().map(|p| qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: &basis }.at(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64])).0).collect();
                for w in pts.windows(2) {
                    let d = super::screen_dist_seg(pos, w[0], w[1]);
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, body, id));
                    }
                }
            }
        }
        let (d, body, eid) = best.filter(|(d, _, _)| *d <= 8.0)?;
        let _ = d;
        let shape = self.live.shapes.get(&body)?;
        let (polys, ids, circles) = shape.edges_full();
        let k = ids.iter().position(|x| *x == eid)?;
        let wt = self.project.body_display_transform(body, ctx);
        let poly = &polys[k];
        if poly.len() < 2 {
            return None;
        }
        // A CIRCULAR EDGE stays a circle: the diameter of a hole is measured by it, and a polyline
        // from the tessellation would give a "length" instead of a diameter.
        if let Some((c, ax, r)) = circles[k] {
            let center = apply12(&wt, c);
            let axis = apply12_dir(&wt, ax);
            return Some(MeasurePick { item: MeasureItem::Circle { center, axis, r }, what: crate::i18n::tr1("m3-circle", "d", &crate::i18n::num(2.0 * r, 2)), at: center });
        }
        let a = apply12(&wt, [poly[0][0] as f64, poly[0][1] as f64, poly[0][2] as f64]);
        let b = apply12(&wt, [poly[poly.len() - 1][0] as f64, poly[poly.len() - 1][1] as f64, poly[poly.len() - 1][2] as f64]);
        // THE LENGTH goes by the polyline rather than the chord: an arc has a shorter chord, and
        // "the length of the edge" would be a lie.
        let mut length = 0.0;
        for w in poly.windows(2) {
            let (p, q) = (apply12(&wt, [w[0][0] as f64, w[0][1] as f64, w[0][2] as f64]), apply12(&wt, [w[1][0] as f64, w[1][1] as f64, w[1][2] as f64]));
            length += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        }
        let dir = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
        Some(MeasurePick { item: MeasureItem::Line { origin: a, dir, len: length }, what: crate::i18n::tr1("m3-edge", "v", &crate::i18n::num(length, 2)), at: mid })
    }

    /// The face under the cursor -> a plane or a cylinder in the WORLD coordinates of the active
    /// context.
    fn measure_face_at(&mut self, rect: Rect, pos: Pos2) -> Option<MeasurePick> {
        let (body, fid, hit) = crate::gui::pick::pick_face_ray(&self.painting(), rect, pos)?;
        let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
        let wt = self.project.body_display_transform(body, ctx);
        // A CYLINDER is a kind of its own: on the wall of a hole one measures the diameter and the
        // gap to that wall, not to an imaginary plane it does not have.
        if let Some((o, ax, r)) = self.live.shapes.get(&body).and_then(|s| s.face_cylinder(fid)) {
            let (origin, axis) = (apply12(&wt, o), apply12_dir(&wt, ax));
            let (area, length) = cylinder_face_data(&self.live, body, fid, r, &wt);
            return Some(MeasurePick { item: MeasureItem::Cylinder { origin, axis, r, area, length }, what: crate::i18n::tr1("m3-cylinder", "d", &crate::i18n::num(2.0 * r, 2)), at: hit });
        }
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: fid };
        let (c, n) = self.project.resolve_face(body, &key);
        let (origin, normal) = (apply12(&wt, c), apply12_dir(&wt, n));
        let (area, perimeter) = planar_face_data(&self.project, body, fid, &wt);
        Some(MeasurePick { item: MeasureItem::Plane { origin, normal, area, perimeter }, what: crate::i18n::tr("m3-face"), at: hit })
    }
}

/// Query area and boundary perimeter of a planar face from project mesh data.
fn planar_face_data(project: &qymcad_core::model::Project, body: Id, fid: u32, wt: &[f64; 12]) -> (Option<f64>, Option<f64>) {
    let scale = (wt[0] * wt[0] + wt[1] * wt[1] + wt[2] * wt[2]).sqrt().max(1e-12);
    let Some(faces) = project.regen_faces.get(&body) else { return (None, None); };
    let Some(f) = faces.iter().find(|f| f.id == fid) else { return (None, None); };
    let area = f.area * scale * scale;
    let mut perim = None;
    if let Some(mi) = project.mesh_index(body) {
        let mesh = &project.bodies[mi].mesh;
        let mut edge_counts = std::collections::HashMap::<(u32, u32), usize>::new();
        for &ti in &f.triangles {
            if let Some(t) = mesh.tris.get(ti as usize) {
                for &(a, b) in &[(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let key = if a < b { (a, b) } else { (b, a) };
                    *edge_counts.entry(key).or_insert(0) += 1;
                }
            }
        }
        let mut loop_len = 0.0;
        for ((a, b), count) in edge_counts {
            if count == 1 {
                if let (Some(va), Some(vb)) = (mesh.verts.get(a as usize), mesh.verts.get(b as usize)) {
                    let (dx, dy, dz) = (va.x - vb.x, va.y - vb.y, va.z - vb.z);
                    loop_len += (dx * dx + dy * dy + dz * dz).sqrt();
                }
            }
        }
        if loop_len > 1e-6 {
            perim = Some(loop_len * scale);
        }
    }
    (Some(area), perim)
}

/// Query surface area and height of a cylindrical face from shape edges.
fn cylinder_face_data(live: &qymcad_ui_state::LiveGeom, body: Id, fid: u32, r: f64, wt: &[f64; 12]) -> (Option<f64>, Option<f64>) {
    let scale = (wt[0] * wt[0] + wt[1] * wt[1] + wt[2] * wt[2]).sqrt().max(1e-12);
    let Some(shape) = live.shapes.get(&body) else { return (None, None); };
    let eids = shape.face_edge_ids(fid);
    let (_polys, ids, circles) = shape.edges_full();
    let mut circ_centers = Vec::new();
    for &eid in &eids {
        if let Some(idx) = ids.iter().position(|&x| x == eid) {
            if let Some((c, ax, _)) = circles[idx] {
                circ_centers.push((c, ax));
            }
        }
    }
    if circ_centers.len() >= 2 {
        let (c1, ax1) = circ_centers[0];
        let (c2, _) = circ_centers[1];
        let diff = [c2[0] - c1[0], c2[1] - c1[1], c2[2] - c1[2]];
        let len_ax = (diff[0] * ax1[0] + diff[1] * ax1[1] + diff[2] * ax1[2]).abs() * scale;
        if len_ax > 1e-6 {
            let area = 2.0 * std::f64::consts::PI * (r * scale) * len_ax;
            return (Some(area), Some(len_ax));
        }
    }
    (None, None)
}
