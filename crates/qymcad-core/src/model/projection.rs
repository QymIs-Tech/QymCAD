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
//! 3. **A curve whose edge is gone stays as ordinary geometry**, and a projection whose source is gone altogether
//!    becomes ordinary geometry whole. Constraints and dimensions reference it: deleted, the sketch would lose them; kept
//!    driven by nothing, it would be a broken mark nobody can act on.
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
    /// How many entities the curve yields: a polyline has one segment fewer than it has points.
    fn entity_count(&self) -> usize {
        match self {
            Curve2::Line(..) | Curve2::Circle(..) => 1,
            Curve2::Poly(p) => p.len().saturating_sub(1),
        }
    }
}

/// WHAT THE BODY SAYS OF THE SOURCE OF A PROJECTION: its curves, that it is gone, or nothing yet - the body is not live
/// (a document just opened builds its solids on demand). Taken for gone, a projection was let go on opening a file.
enum Source {
    Found(Vec<(u32, Curve2)>),
    Gone,
    Unknown,
}

/// WHAT A PASS OF `apply_curves` HAS PLACED: each point given its place, and the centres of circles among them.
#[derive(Default)]
struct Placing {
    at: Vec<(Id, [f64; 2])>,
    centres: Vec<Id>,
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
        self.sketches[si].projections.push(super::SketchProjection { id, body, src, points: Vec::new(), entities: Vec::new(), edges: Vec::new() });
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

    /// A PROJECTION MADE ORDINARY GEOMETRY OF THE SKETCH: the record that ties it to the body goes, its points and curves
    /// stay where they stand. Nothing pins them any more - the pins of a projection are laid from the record at every
    /// solve - so they are dragged, deleted and given dimensions as any geometry, and a change of the body no longer
    /// moves them. Answers whether there was such a projection.
    pub fn release_sketch_projection(&mut self, si: usize, pid: Id) -> bool {
        let Some(s) = self.sketches.get_mut(si) else { return false };
        let before = s.projections.len();
        s.projections.retain(|p| p.id != pid);
        let released = s.projections.len() < before;
        if released {
            self.solve_sketch(si);
        }
        released
    }

    /// THE PROJECTIONS ANY OF `eids` IS A CURVE OF, made ordinary geometry (`release_sketch_projection`). Answers how many.
    pub fn release_projections_of(&mut self, si: usize, eids: &[Id]) -> usize {
        let Some(s) = self.sketches.get(si) else { return 0 };
        let pids: Vec<Id> = s.projections.iter().filter(|p| p.entities.iter().any(|e| eids.contains(e))).map(|p| p.id).collect();
        pids.into_iter().filter(|&pid| self.release_sketch_projection(si, pid)).count()
    }

    /// Recompute every projection of a sketch from the live geometry of the bodies. Called from the rebuild in
    /// timeline order, by which point the source bodies are already built, so a projection follows its part on
    /// its own. A projection whose source is gone altogether is made ordinary geometry (`release_sketch_projection`).
    pub fn resolve_sketch_projections(&mut self, si: usize, kernel: &dyn crate::feature::Kernel) {
        if self.sketches.get(si).is_none_or(|s| s.projections.is_empty()) {
            return;
        }
        let Some(frame) = self.sketch_frame(si) else { return };
        let mut gone = Vec::new();
        for k in 0..self.sketches[si].projections.len() {
            let (pid, body, src) = {
                let p = &self.sketches[si].projections[k];
                (p.id, p.body, p.src)
            };
            match self.projected_curves(body, src, &frame, kernel) {
                Source::Found(curves) => self.apply_curves(si, k, curves),
                Source::Gone => gone.push(pid),
                Source::Unknown => {}
            }
        }
        // the source is gone: the geometry stays as ordinary geometry - the constraints on it stay met - rather than a
        // broken mark pinned in place
        for pid in gone {
            if let Some(s) = self.sketches.get_mut(si) {
                s.projections.retain(|p| p.id != pid);
            }
        }
    }

    /// THE PROJECTIONS OF A SKETCH FOLLOW THE BODIES, AND WHAT IS TIED TO THEM FOLLOWS: the projections resolved
    /// (`resolve_sketch_projections`), and when they moved, the sketch solved along the way in steps of at most 1 mm
    /// (at most 32), the projected points led from where they stood to where the body put them. A dimension holds its
    /// side by where its points stand when a solve begins: moved in one jump, a projection carried the geometry tied 5 off
    /// its inside past its corner, and the solve held it 5 off the outside. Reported behaviour: "tie to the projection by
    /// dimensions, change the body above: it falls apart" - the base made 30 x 20 from 60 x 60, the square tied 5 inside
    /// the projected outline came out 40 x 30, outside it. Answers whether a projection moved.
    pub fn follow_sketch_projections(&mut self, si: usize, kernel: &dyn crate::feature::Kernel) -> bool {
        let before = self.sketch_projection_key(si);
        let was: std::collections::HashMap<Id, [f64; 2]> = self.projected_places(si);
        self.resolve_sketch_projections(si, kernel);
        if self.sketch_projection_key(si) == before {
            return false;
        }
        let now = self.projected_places(si);
        let jump = now.iter().filter_map(|(id, b)| was.get(id).map(|a| (b[0] - a[0]).hypot(b[1] - a[1]))).fold(0.0_f64, f64::max);
        let steps = (jump.ceil() as usize).clamp(1, 32);
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            for (id, b) in &now {
                let a = was.get(id).copied().unwrap_or(*b);
                self.set_point_xy(si, Some(*id), [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
            self.solve_sketch(si);
        }
        true
    }

    /// Where every point of the projections of sketch `si` stands, by its id.
    fn projected_places(&self, si: usize) -> std::collections::HashMap<Id, [f64; 2]> {
        let Some(s) = self.sketches.get(si) else { return Default::default() };
        let ids: std::collections::HashSet<Id> = s.projected_points();
        s.points.iter().filter(|q| ids.contains(&q.id)).map(|q| (q.id, [q.x, q.y])).collect()
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
    fn projected_curves(&self, body: Id, src: ProjSource, frame: &PlaneFrame, kernel: &dyn crate::feature::Kernel) -> Source {
        let geom = kernel.body_edge_geometry(body);
        if geom.is_empty() {
            // the body is not built here, or not yet live - a document just opened keeps its meshes and builds the solid
            // on demand: this says nothing about the source
            return Source::Unknown;
        }
        let want: Vec<u32> = match src {
            ProjSource::Edge(e) => vec![e],
            // the edges of a face are taken from the live topology rather than from a remembered list: a
            // fillet or a cut changes the composition of the outline, and a remembered list would fall behind
            // the part
            ProjSource::Face(f) => kernel.edge_face_pairs(body).into_iter().filter(|(_, a, b)| *a == f || *b == f).map(|(e, _, _)| e).collect(),
        };
        if want.is_empty() {
            return Source::Gone;
        }
        let mut out = Vec::new();
        for (id, poly, circ) in geom {
            if !want.contains(&id) || poly.len() < 2 {
                continue;
            }
            out.push((id, Self::curve_to_2d(&poly, circ, frame)));
        }
        if out.is_empty() {
            Source::Gone
        } else {
            Source::Found(out)
        }
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

    /// PLACE THE CURVES INTO THE SKETCH, each matched to the curve its edge gave before. A curve of an edge still there keeps
    /// its id and moves, its ends keeping theirs - the dimensions and the ties on it stay; a corner two edges shared stays
    /// one point while they still meet there, and parts when they do not (a fillet put between them). A new edge gives new
    /// curves. A curve whose edge is gone, or whose shape changed (a line become a polyline), is let go of the projection
    /// and stays as ordinary geometry: deleted, the constraints on it would go with it. Reported behaviour: "if other
    /// geometry is tied to the projection by dimensions or constraints, the projection may fall apart when the model it
    /// came from is rebuilt" - the curves were matched by their place in the list of edges, and a change of the
    /// structure deleted them all and made them anew.
    fn apply_curves(&mut self, si: usize, k: usize, curves: Vec<(u32, Curve2)>) {
        let (old_ents, old_edges) = {
            let p = &self.sketches[si].projections[k];
            // a projection made before the edges were kept: taken in the order the edges come, as it was laid
            let edges = if p.edges.len() == p.entities.len() { p.edges.clone() } else { curves.iter().flat_map(|(e, c)| std::iter::repeat_n(*e, c.entity_count())).collect() };
            (p.entities.clone(), edges)
        };
        let mut by_edge: std::collections::HashMap<u32, Vec<Id>> = std::collections::HashMap::new();
        for (e, edge) in old_ents.iter().zip(&old_edges) {
            by_edge.entry(*edge).or_default().push(*e);
        }
        let mut placing = Placing::default();
        let (mut ents, mut edges, mut pts): (Vec<Id>, Vec<u32>, Vec<Id>) = (Vec::new(), Vec::new(), Vec::new());
        for (edge, c) in &curves {
            let olds = by_edge.remove(edge).unwrap_or_default();
            let kinds: Vec<EntityKind> = olds.iter().filter_map(|id| self.sketches[si].entities.iter().find(|e| e.id == *id).map(|e| e.kind)).collect();
            match c {
                Curve2::Line(a, b) => {
                    let old = match kinds.as_slice() {
                        [EntityKind::Line { a, b }] => Some((olds[0], *a, *b)),
                        _ => None,
                    };
                    let pa = self.proj_point(si, &mut placing, old.map(|o| o.1), *a);
                    let pb = self.proj_point(si, &mut placing, old.map(|o| o.2), *b);
                    let e = self.proj_entity(si, old.map(|o| o.0), EntityKind::Line { a: pa, b: pb });
                    ents.push(e);
                    edges.push(*edge);
                    pts.extend([pa, pb]);
                }
                Curve2::Circle(c0, r) => {
                    let old = match kinds.as_slice() {
                        [EntityKind::Circle { center, .. }] => Some((olds[0], *center)),
                        _ => None,
                    };
                    // the centre of a circle is shared with nobody: it carries the radius variable of the solver
                    let pc = self.proj_centre(si, &mut placing, old.map(|o| o.1), *c0);
                    let e = self.proj_entity(si, old.map(|o| o.0), EntityKind::Circle { center: pc, r: *r });
                    ents.push(e);
                    edges.push(*edge);
                    pts.push(pc);
                }
                Curve2::Poly(p) => {
                    // the chain of points the old segments ran through, when they were as many lines as the new
                    let chain: Option<Vec<Id>> = (kinds.len() + 1 == p.len() && kinds.iter().all(|k| matches!(k, EntityKind::Line { .. }))).then(|| {
                        kinds
                            .iter()
                            .enumerate()
                            .flat_map(|(n, k)| {
                                if let EntityKind::Line { a, b } = k {
                                    if n == 0 {
                                        vec![*a, *b]
                                    } else {
                                        vec![*b]
                                    }
                                } else {
                                    Vec::new()
                                }
                            })
                            .collect()
                    });
                    let ids: Vec<Id> = p.iter().enumerate().map(|(n, q)| self.proj_point(si, &mut placing, chain.as_ref().map(|c| c[n]), *q)).collect();
                    for (n, w) in ids.windows(2).filter(|w| w[0] != w[1]).enumerate() {
                        let old = chain.as_ref().and(olds.get(n).copied());
                        let e = self.proj_entity(si, old, EntityKind::Line { a: w[0], b: w[1] });
                        ents.push(e);
                        edges.push(*edge);
                    }
                    pts.extend(ids);
                }
            }
        }
        for (pid, at) in &placing.at {
            self.set_point_xy(si, Some(*pid), *at);
        }
        // what the projection no longer holds - the curves of gone edges, a curve reshaped - stays as ordinary geometry
        let p = &mut self.sketches[si].projections[k];
        p.points = pts;
        p.entities = ents;
        p.edges = edges;
    }

    /// A POINT OF THE PROJECTION AT `at`: the corner already placed there in this pass, or the point `old` stood for when
    /// no other place has taken it, or a new one.
    fn proj_point(&mut self, si: usize, placing: &mut Placing, old: Option<Id>, at: [f64; 2]) -> Id {
        // a corner is shared by the curves that meet there; the centre of a circle on that spot is not one of them
        if let Some((id, _)) = placing.at.iter().find(|(id, q)| !placing.centres.contains(id) && (q[0] - at[0]).hypot(q[1] - at[1]) < 1e-6) {
            return *id;
        }
        let id = match old.filter(|o| !placing.at.iter().any(|(id, _)| id == o) && self.sketches[si].points.iter().any(|q| q.id == *o)) {
            Some(o) => o,
            None => self.push_proj_point(si, at),
        };
        placing.at.push((id, at));
        id
    }

    /// The centre of a projected circle: its own point, never shared with a corner.
    fn proj_centre(&mut self, si: usize, placing: &mut Placing, old: Option<Id>, at: [f64; 2]) -> Id {
        let id = match old.filter(|o| !placing.at.iter().any(|(id, _)| id == o) && self.sketches[si].points.iter().any(|q| q.id == *o)) {
            Some(o) => o,
            None => self.push_proj_point(si, at),
        };
        placing.centres.push(id);
        placing.at.push((id, at));
        id
    }

    /// The curve `old` given its new shape, or a new curve when there is none to keep.
    fn proj_entity(&mut self, si: usize, old: Option<Id>, kind: EntityKind) -> Id {
        if let Some(e) = old.and_then(|o| self.sketches[si].entities.iter_mut().find(|e| e.id == o)) {
            e.kind = kind;
            return e.id;
        }
        self.push_proj_entity(si, kind)
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

    fn push_proj_entity(&mut self, si: usize, kind: EntityKind) -> Id {
        let id = self.alloc_id();
        self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
        id
    }
}
