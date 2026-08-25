//! Projecting body geometry into a sketch.
//!
//! Body edges used to be drawn in the sketcher as a backdrop only: they could be snapped to, but not taken as
//! entities. In a professional CAD this is the most common sketch tool after the primitives — project the
//! outline of a face and trace along it.
//!
//! Three things make a projection a projection rather than a one-off copy of points:
//!
//! 1. **The reference is by name, not by number.** The source is the persistent id of an edge or a face, so an
//!    edit earlier in the timeline does not move the projection onto a neighbouring edge.
//! 2. **The ids of the derived points are stable.** A constraint may be placed on a corner of a projection; if
//!    the points were created afresh on every rebuild, that constraint would fall off silently. As long as the
//!    structure of the source is unchanged — the same curves in the same order — the points only move.
//! 3. **When the source disappears the projection goes red rather than vanishing.** Constraints and dimensions
//!    reference it, and removing it silently would break the sketch.
use super::{EntityKind, Id, ProjSource, Project, SketchEntity, SketchPoint};
use crate::feature::PlaneFrame;
use crate::geom::Point3;

/// A curve ready to be placed into a sketch, already in the 2D coordinates of the sketch plane.
enum Curve2 {
    Line([f64; 2], [f64; 2]),
    Circle([f64; 2], f64),
    /// A polyline: everything that is neither a straight line nor a circle — elliptical arcs, splines,
    /// intersection curves.
    Poly(Vec<[f64; 2]>),
}

impl Curve2 {
    /// How many points the curve will occupy in the sketch; used to check whether the structure changed.
    fn point_count(&self) -> usize {
        match self {
            Curve2::Line(..) => 2,
            Curve2::Circle(..) => 1,
            Curve2::Poly(p) => p.len(),
        }
    }

    /// How many entities the curve yields: a polyline has one segment fewer than it has points.
    fn entity_count(&self) -> usize {
        match self {
            Curve2::Line(..) | Curve2::Circle(..) => 1,
            Curve2::Poly(p) => p.len().saturating_sub(1),
        }
    }

    /// A kind tag, for comparing structure: a line, a circle, or a polyline of the same length.
    fn shape_key(&self) -> (u8, usize) {
        match self {
            Curve2::Line(..) => (0, 2),
            Curve2::Circle(..) => (1, 1),
            Curve2::Poly(p) => (2, p.len()),
        }
    }
}

impl Project {
    /// Project body geometry into a sketch: create the projection and resolve it at once.
    ///
    /// Returns the id of the projection; zero means the source yielded no curves and none was created.
    pub fn add_sketch_projection(&mut self, si: usize, body: Id, src: ProjSource, kernel: &dyn crate::feature::Kernel) -> Id {
        let Some(s) = self.sketches.get(si) else { return 0 };
        // Picking the same source again means it is already projected, not that a second copy goes on top of
        // the first.
        if let Some(p) = s.projections.iter().find(|p| p.body == body && p.src == src) {
            return p.id;
        }
        let id = self.alloc_id();
        self.sketches[si].projections.push(super::SketchProjection { id, body, src, points: Vec::new(), entities: Vec::new(), lost: false });
        self.resolve_sketch_projections(si, kernel);
        // the source yielded nothing, so no empty record is left behind to look like completed work
        let empty = self.sketches[si].projections.iter().any(|p| p.id == id && p.entities.is_empty());
        if empty {
            self.sketches[si].projections.retain(|p| p.id != id);
            return 0;
        }
        self.regen_sketch(si);
        id
    }

    /// Delete a projection together with the geometry it drives.
    pub fn remove_sketch_projection(&mut self, si: usize, pid: Id) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let Some(k) = s.projections.iter().position(|p| p.id == pid) else { return false };
        let p = s.projections.remove(k);
        let (mut pts, ents) = (p.points, p.entities);
        self.delete_entities(si, &ents);
        // points are cleaned up after the entities: while an entity lives, its endpoints count as in use
        pts.sort_unstable();
        pts.dedup(); // a shared corner occupies several slots
        self.delete_points(si, &pts);
        self.regen_sketch(si);
        true
    }

    /// Recompute every projection of a sketch from the live geometry of the bodies. Called from the rebuild in
    /// timeline order, by which point the source bodies are already built, so a projection follows its part on
    /// its own.
    pub fn resolve_sketch_projections(&mut self, si: usize, kernel: &dyn crate::feature::Kernel) {
        if self.sketches.get(si).is_none_or(|s| s.projections.is_empty()) {
            return;
        }
        let Some(frame) = self.sketch_frame(si) else { return };
        let n = self.sketches[si].projections.len();
        for k in 0..n {
            let (body, src) = {
                let p = &self.sketches[si].projections[k];
                (p.body, p.src)
            };
            let curves = self.projected_curves(body, src, &frame, kernel);
            match curves {
                // The source is gone. The geometry stays as it is, since constraints reference it, but is
                // marked broken: a projection that vanished silently would break the sketch, and one that
                // stayed silently would lie.
                None => self.sketches[si].projections[k].lost = true,
                Some(c) => {
                    self.sketches[si].projections[k].lost = false;
                    self.apply_curves(si, k, c);
                }
            }
        }
    }

    /// A fingerprint of the driven geometry of a sketch, which shows whether a projection moved. Without it the
    /// consumers of the contour would have to be rebuilt on every regeneration, even when the part did not
    /// change.
    pub(super) fn sketch_projection_key(&self, si: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let Some(s) = self.sketches.get(si) else { return 0 };
        for p in &s.projections {
            p.id.hash(&mut h);
            p.lost.hash(&mut h);
            p.entities.hash(&mut h);
            for pid in &p.points {
                if let Some(pt) = s.points.iter().find(|x| x.id == *pid) {
                    (pt.x.to_bits(), pt.y.to_bits()).hash(&mut h);
                }
            }
            for eid in &p.entities {
                if let Some(EntityKind::Circle { r, .. }) = s.entities.iter().find(|x| x.id == *eid).map(|e| e.kind) {
                    r.to_bits().hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// The curves of the source, already converted into the 2D of the sketch. `None` means the source was not
    /// found in the body.
    fn projected_curves(&self, body: Id, src: ProjSource, frame: &PlaneFrame, kernel: &dyn crate::feature::Kernel) -> Option<Vec<Curve2>> {
        let geom = kernel.body_edge_geometry(body);
        if geom.is_empty() {
            return None; // the body is not built or the kernel cannot do it; this is not a vanished source
        }
        let want: Vec<u32> = match src {
            ProjSource::Edge(e) => vec![e],
            // the edges of a face are taken from the live topology rather than from a remembered list: a
            // fillet or a cut changes the composition of the outline, and a remembered list would fall behind
            // the part
            ProjSource::Face(f) => kernel.edge_face_pairs(body).into_iter().filter(|(_, a, b)| *a == f || *b == f).map(|(e, _, _)| e).collect(),
        };
        if want.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for (id, poly, circ) in geom {
            if !want.contains(&id) || poly.len() < 2 {
                continue;
            }
            out.push(Self::curve_to_2d(&poly, circ, frame));
        }
        (!out.is_empty()).then_some(out)
    }

    /// An edge becomes a sketch curve. A circle stays a circle and a straight edge stays a line; everything
    /// else becomes a polyline.
    fn curve_to_2d(poly: &[[f64; 3]], circ: Option<([f64; 3], [f64; 3], f64)>, frame: &PlaneFrame) -> Curve2 {
        let to2 = |p: &[f64; 3]| {
            let q = frame.project(Point3::new(p[0], p[1], p[2]));
            [q.x, q.y]
        };
        // A closed circle lying parallel to the sketch plane becomes a circle rather than a sixty-segment
        // polyline. At an angle a circle projects as an ellipse, and passing that off as a circle is not
        // acceptable: a dimension taken from such a circle would give the wrong number.
        if let Some((c, axis, r)) = circ {
            let n = frame.normal();
            let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt().max(1e-12);
            let dot = (axis[0] * n[0] + axis[1] * n[1] + axis[2] * n[2]) / al;
            let closed = {
                let (a, b) = (poly[0], poly[poly.len() - 1]);
                (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]) < 1e-6
            };
            if closed && dot.abs() > 0.999 && r > 1e-9 {
                return Curve2::Circle(to2(&c), r);
            }
        }
        let pts: Vec<[f64; 2]> = poly.iter().map(to2).collect();
        // A straight edge, where every intermediate point lies on the segment between the ends, is stored as a
        // segment rather than a polyline; otherwise an ordinary dimension or a parallel constraint could not be
        // placed on it.
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len > 1e-9 {
            let straight = pts.iter().all(|p| ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len < 1e-6);
            if straight {
                return Curve2::Line(a, b);
            }
        }
        Curve2::Poly(pts)
    }

    /// Place the curves into the sketch, preserving ids while the structure of the source is unchanged.
    fn apply_curves(&mut self, si: usize, k: usize, curves: Vec<Curve2>) {
        let same = {
            let p = &self.sketches[si].projections[k];
            let keys: Vec<(u8, usize)> = curves.iter().map(|c| c.shape_key()).collect();
            let slots: usize = curves.iter().map(|c| c.point_count()).sum();
            let segs: usize = curves.iter().map(|c| c.entity_count()).sum();
            p.entities.len() == segs && p.points.len() == slots && self.entity_keys_match(si, &p.entities, &keys)
        };
        if same {
            self.move_projected_points(si, k, &curves);
            return;
        }
        // The structure of the source changed — an edge appeared, a corner was cut — so the old driven
        // geometry is removed and built again. The constraints that referenced it go with the points, as they
        // should: keeping them on geometry that no longer exists in the part would mean lying to the solver.
        let (old_pts, old_ents) = {
            let p = &self.sketches[si].projections[k];
            (p.points.clone(), p.entities.clone())
        };
        self.delete_entities(si, &old_ents);
        let mut uniq = old_pts.clone();
        uniq.sort_unstable();
        uniq.dedup(); // a shared corner occupies two slots and need not be removed twice
        self.delete_points(si, &uniq);
        // The list of points follows the slots of the curves and contains repeats. A corner shared by two
        // edges appears in both slots under the same id, so the recomputation for an unchanged structure runs
        // positionally, without guessing which point belongs to which, and the ids survive the rebuild along
        // with the constraints placed on them.
        let (mut pts, mut ents): (Vec<Id>, Vec<Id>) = (Vec::new(), Vec::new());
        // A shared corner is one point. The edges of a face meet at vertices, and if every segment created its
        // own endpoints the outline would come out open: it could not be extruded, and tracing along the
        // projection would not close. The deduplication stays within this projection only — attaching to a
        // point placed by hand would make the projection move it on every rebuild of the body.
        let mut mine: Vec<(Id, [f64; 2])> = Vec::new();
        for c in &curves {
            match c {
                Curve2::Line(a, b) => {
                    let (pa, pb) = (self.proj_point_at(si, *a, &mut mine), self.proj_point_at(si, *b, &mut mine));
                    pts.push(pa);
                    pts.push(pb);
                    ents.push(self.push_proj_entity(si, EntityKind::Line { a: pa, b: pb }));
                }
                Curve2::Circle(c0, r) => {
                    // the centre of a circle is shared with nobody: it carries the radius variable of the
                    // solver, and a shared centre would collapse the radii of two curves
                    let pc = self.push_proj_point(si, *c0);
                    pts.push(pc);
                    ents.push(self.push_proj_entity(si, EntityKind::Circle { center: pc, r: *r }));
                }
                Curve2::Poly(p) => {
                    let ids: Vec<Id> = p.iter().map(|q| self.proj_point_at(si, *q, &mut mine)).collect();
                    for w in ids.windows(2) {
                        if w[0] != w[1] {
                            ents.push(self.push_proj_entity(si, EntityKind::Line { a: w[0], b: w[1] }));
                        }
                    }
                    pts.extend(ids);
                }
            }
        }
        // a polyline yields one segment fewer than it has points; the structure check looks at the keys rather
        // than at the length of `entities`, so the actual lists are stored as they are
        let p = &mut self.sketches[si].projections[k];
        p.points = pts;
        p.entities = ents;
    }

    /// Whether the kinds of the existing driven entities match the new curves.
    fn entity_keys_match(&self, si: usize, ents: &[Id], keys: &[(u8, usize)]) -> bool {
        let s = &self.sketches[si];
        // expand the curve keys into a sequence of entity kinds, a polyline giving n − 1 segments
        let mut want: Vec<u8> = Vec::new();
        for k in keys {
            match k.0 {
                1 => want.push(1),
                2 => want.extend(std::iter::repeat_n(0u8, k.1.saturating_sub(1))),
                _ => want.push(0),
            }
        }
        if want.len() != ents.len() {
            return false;
        }
        ents.iter().zip(want).all(|(e, w)| {
            s.entities.iter().find(|x| x.id == *e).is_some_and(|ent| match (ent.kind, w) {
                (EntityKind::Line { .. }, 0) => true,
                (EntityKind::Circle { .. }, 1) => true,
                _ => false,
            })
        })
    }

    /// The structure is unchanged, so this is only a move: new coordinates, the same ids, and the constraints
    /// placed on them stay alive.
    fn move_projected_points(&mut self, si: usize, k: usize, curves: &[Curve2]) {
        let ids = self.sketches[si].projections[k].points.clone();
        let ents = self.sketches[si].projections[k].entities.clone();
        let mut i = 0;
        let mut ei = 0;
        for c in curves {
            match c {
                Curve2::Line(a, b) => {
                    self.set_point_xy(si, ids.get(i).copied(), *a);
                    self.set_point_xy(si, ids.get(i + 1).copied(), *b);
                    i += 2;
                    ei += 1;
                }
                Curve2::Circle(c0, r) => {
                    self.set_point_xy(si, ids.get(i).copied(), *c0);
                    // the radius lives in the entity itself rather than in the points, so it moves too
                    if let Some(e) = ents.get(ei).and_then(|id| self.sketches[si].entities.iter_mut().find(|x| x.id == *id)) {
                        if let EntityKind::Circle { r: er, .. } = &mut e.kind {
                            *er = *r;
                        }
                    }
                    i += 1;
                    ei += 1;
                }
                Curve2::Poly(p) => {
                    for q in p {
                        self.set_point_xy(si, ids.get(i).copied(), *q);
                        i += 1;
                    }
                    ei += p.len().saturating_sub(1);
                }
            }
        }
    }

    fn set_point_xy(&mut self, si: usize, pid: Option<Id>, at: [f64; 2]) {
        let Some(pid) = pid else { return };
        if let Some(p) = self.sketches[si].points.iter_mut().find(|p| p.id == pid) {
            p.x = at[0];
            p.y = at[1];
        }
    }

    /// A driven point of a projection. There is deliberately no deduplication against foreign points:
    /// attaching to a point placed by hand would make the projection move it on every rebuild of the body.
    fn push_proj_point(&mut self, si: usize, at: [f64; 2]) -> Id {
        let id = self.alloc_id();
        self.sketches[si].points.push(SketchPoint { id, x: at[0], y: at[1] });
        id
    }

    /// A driven point of a projection, deduplicated within that same projection, so corners shared by edges
    /// become one point.
    fn proj_point_at(&mut self, si: usize, at: [f64; 2], mine: &mut Vec<(Id, [f64; 2])>) -> Id {
        if let Some((id, _)) = mine.iter().find(|(_, q)| (q[0] - at[0]).hypot(q[1] - at[1]) < 1e-6) {
            return *id;
        }
        let id = self.push_proj_point(si, at);
        mine.push((id, at));
        id
    }

    fn push_proj_entity(&mut self, si: usize, kind: EntityKind) -> Id {
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
        id
    }
}
