//! MEASURING IN 3D — resolving a click into the element being measured, and the tool itself.
//!
//! The arithmetic lives in the kernel (`qymcad_core::measure`) and is checked on known geometry;
//! what is here is only "what did the cursor hit". Measuring used to be possible ONLY in a sketch and
//! only between two points on a plane: the gap between parts, the distance between faces, the angle
//! of convergence and the diameter of a hole in 3D had nothing to measure them with.
use super::{App, Id};
use egui::{Pos2, Rect};
use qymcad_core::feature::{apply12, apply12_dir};
use qymcad_core::measure::{measure_one, measure_pair, MeasureItem, MeasureResult};

/// What the measuring tool has clicked: the element itself plus a human-readable name (shown in the
/// hint — otherwise it is not clear what exactly the click caught).
#[derive(Clone, Debug)]
pub(crate) struct MeasurePick {
    pub item: MeasureItem,
    pub what: String,
    /// The point at which to draw the label and run the leader.
    pub at: [f64; 3],
}

/// THE STATE OF THE 3D MEASURING TOOL: up to two elements. A third click starts a new measurement,
/// the same as in the sketch measuring tool, and that is the one behaviour nobody confuses.
#[derive(Clone, Debug, Default)]
pub(crate) struct Measure3 {
    pub on: bool,
    pub picks: Vec<MeasurePick>,
}

impl Measure3 {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl App {
    /// SWITCH the 3D measuring tool ON or OFF.
    pub(super) fn toggle_measure_3d(&mut self) {
        let on = !self.m3.on;
        self.cancel_all_tools(); // exclusivity: the measuring tool puts down the previous one
        self.m3.clear();
        self.m3.on = on;
        if on {
            self.mode_3d = true;
            self.status = crate::i18n::tr("m3-hint");
        }
    }

    /// A click of the measuring tool: resolve the hit and update the result.
    pub(super) fn measure_3d_click(&mut self, rect: Rect, pos: Pos2) {
        let Some(p) = self.measure_resolve(rect, pos) else {
            self.status = crate::i18n::tr("m3-miss");
            return;
        };
        if self.m3.picks.len() >= 2 {
            self.m3.picks.clear(); // a third click means a new measurement
        }
        self.m3.picks.push(p);
        self.status = self.measure_text();
    }

    /// THE RESULT TEXT — the only place where the numbers turn into a string (the status line and the
    /// plate at the geometry say the same thing: two wordings of one measurement drift apart
    /// silently).
    pub(super) fn measure_text(&self) -> String {
        let Some(r) = self.measure_result() else { return crate::i18n::tr("m3-hint-short") };
        let names: Vec<&str> = self.m3.picks.iter().map(|p| p.what.as_str()).collect();
        let mut parts: Vec<String> = Vec::new();
        if let Some((label, v)) = r.value {
            parts.push(crate::i18n::tr2("m3-value-mm", "label", label, "v", &crate::i18n::num(v, 3)));
        }
        if let Some(d) = r.distance {
            parts.push(crate::i18n::tr1("m3-distance", "v", &crate::i18n::num(d, 3)));
        }
        if let Some(a) = r.angle_deg {
            parts.push(crate::i18n::tr1("m3-angle", "v", &crate::i18n::num(a, 3)));
        }
        if let Some(d) = r.delta {
            parts.push(format!("Δ {:.3} / {:.3} / {:.3}", d[0], d[1], d[2]));
        }
        if parts.is_empty() {
            // HONESTLY: there is no meaningful number for this pair (converging faces, for one — the
            // distance between them depends on where it is measured). Silence is worse: a person will
            // decide the tool is broken.
            return crate::i18n::tr1("m3-not-parallel", "what", &names.join(" - "));
        }
        format!("{}: {}", names.join(" - "), parts.join(" · "))
    }

    /// The result for what was clicked: one element gives its own size, two give a pair.
    pub(super) fn measure_result(&self) -> Option<MeasureResult> {
        match self.m3.picks.len() {
            1 => Some(measure_one(&self.m3.picks[0].item)),
            2 => Some(measure_pair(&self.m3.picks[0].item, &self.m3.picks[1].item)),
            _ => None,
        }
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
        let basis = self.cam.basis();
        let face = self.pick_face_ray(rect, pos);
        let face_depth = face.map(|(_, _, hit)| self.project3(hit, rect, &basis).1);
        let in_front = |d: f64| face_depth.is_none_or(|fd| d <= fd + 0.5); // 0.5 mm of tolerance for the silhouette
        if let Some(w) = self.pick_vertex_pos(rect, pos) {
            if in_front(self.project3(w, rect, &basis).1) {
                return Some(MeasurePick { item: MeasureItem::Point(w), what: crate::i18n::tr("m3-vertex"), at: w });
            }
        }
        if let Some(p) = self.measure_edge_at(rect, pos) {
            if in_front(self.project3(p.at, rect, &basis).1) {
                return Some(p);
            }
        }
        self.measure_face_at(rect, pos)
    }

    /// The edge under the cursor -> a line or a circle in the WORLD coordinates of the active context.
    fn measure_edge_at(&mut self, rect: Rect, pos: Pos2) -> Option<MeasurePick> {
        let basis = self.cam.basis();
        let ctx = self.current_ctx_id();
        let mut best: Option<(f32, Id, u32)> = None;
        for (_mi, body) in self.shown_bodies() {
            if !self.body_bbox_hit(body, rect, pos, &basis, 12.0) {
                continue;
            }
            let Some(edges) = self.body_edges_cached(body) else { continue };
            let wt = self.project.body_display_transform(body, ctx);
            for (poly, id) in edges.0.iter().zip(edges.1.iter().copied()) {
                if id == 0 {
                    continue;
                }
                let pts: Vec<Pos2> = poly.iter().map(|p| self.project3(apply12(&wt, [p[0] as f64, p[1] as f64, p[2] as f64]), rect, &basis).0).collect();
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
        let (body, fid, hit) = self.pick_face_ray(rect, pos)?;
        let ctx = self.current_ctx_id();
        let wt = self.project.body_display_transform(body, ctx);
        // A CYLINDER is a kind of its own: on the wall of a hole one measures the diameter and the
        // gap to that wall, not to an imaginary plane it does not have.
        if let Some((o, ax, r)) = self.live.shapes.get(&body).and_then(|s| s.face_cylinder(fid)) {
            let origin = apply12(&wt, o);
            let axis = apply12_dir(&wt, ax);
            return Some(MeasurePick { item: MeasureItem::Cylinder { origin, axis, r }, what: crate::i18n::tr1("m3-cylinder", "d", &crate::i18n::num(2.0 * r, 2)), at: hit });
        }
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: fid };
        let (c, n) = self.project.resolve_face(body, &key);
        let origin = apply12(&wt, c);
        let normal = apply12_dir(&wt, n);
        Some(MeasurePick { item: MeasureItem::Plane { origin, normal }, what: crate::i18n::tr("m3-face"), at: hit })
    }
}
