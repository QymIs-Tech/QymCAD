//! THE OFFSET OF CURVES of a sketch.

use super::*;
use crate::model::curve::Curve;

/// THE CURVES OF `ents` WHOSE OFFSET IS NO CURVE OF THEIR KIND - an ellipse, an arc of one, a spline - each with the
/// distance to its left it is offset by for `dist`: outwards of a closed curve, as a circle grows by it, to the left of
/// an open one; construction geometry left out, as the lines and arcs leave it.
fn offset_freely(s: &Sketch, ents: &[SketchEntity], dist: f64) -> Vec<Freely> {
    ents.iter()
        .filter(|e| !e.construction)
        .filter_map(|e| {
            let curve = Curve::of(s, e).filter(|c| !c.offsets_exactly())?;
            // a closed curve running counter-clockwise has its inside on its left
            let left = if !curve.closed() {
                dist
            } else if winds_counter_clockwise(&curve) {
                -dist
            } else {
                dist
            };
            Some(Freely { eid: e.id, curve, left })
        })
        .collect()
}

/// A CURVE OFFSET ON ITS OWN: its id, the curve, and the distance to its left it is offset by.
struct Freely {
    eid: Id,
    curve: Curve,
    left: f64,
}

/// Whether a closed curve runs counter-clockwise: the area its laid out points enclose is positive.
fn winds_counter_clockwise(c: &Curve) -> bool {
    let p = c.polyline();
    let n = p.len();
    (0..n).map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y).sum::<f64>() > 0.0
}

impl Project {
    /// Offset the selected entities: their closed loops are moved by `dist`, inwards or outwards, and added as
    /// new entities.
    /// THE COPY AN OFFSET OF `eids` BY `dist` WOULD MAKE, as polylines with their arcs drawn out, the sketch left as it
    /// is: what the tool previews before Enter. Circles, loops and open chains alike (`offset_entities`).
    pub fn offset_preview(&self, si: usize, eids: &[Id], dist: f64) -> Vec<Vec<Point2>> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).cloned().collect();
        let mut out = Vec::new();
        for e in &ents {
            if let Some(crate::model::CircleParts { center, r }) = e.kind.as_circle() {
                let nr = r + dist;
                if let (Some(c), true) = (s.points.iter().find(|p| p.id == center), nr > 0.05) {
                    out.push((0..=72).map(|k| f64::from(k) / 72.0 * std::f64::consts::TAU).map(|t| Point2::new(c.x + nr * t.cos(), c.y + nr * t.sin())).collect());
                }
            }
        }
        for lp in entity_bulge_loops(&s.points, &ents) {
            out.extend(crate::offset::offset_bulge(&lp, dist).iter().map(|v| crate::offset::bulge_points(v, true)));
        }
        for ch in entity_bulge_chains(&s.points, &ents) {
            out.extend(crate::offset::offset_bulge_open(&ch, dist).iter().map(|v| crate::offset::bulge_points(v, false)));
        }
        out.extend(offset_freely(s, &ents, dist).iter().flat_map(|f| f.curve.offset(f.left)).map(|c| c.polyline()));
        out
    }

    /// THE OFFSET OF `eids` TOWARD THE POINTER: `dist` signed for the side whose copy passes nearer `pointer` - the
    /// inside or the outside of a loop, the left or the right of a chain. A person shows the side rather than types its
    /// sign; a negative distance typed takes the other side, as in the systems a person comes from.
    pub fn offset_toward(&self, si: usize, eids: &[Id], dist: f64, pointer: Point2) -> f64 {
        let near = |d: f64| -> f64 { self.offset_preview(si, eids, d).iter().flat_map(|pl| pl.windows(2).map(|w| seg_dist(pointer, w[0], w[1])).collect::<Vec<_>>()).fold(f64::MAX, f64::min) };
        let d = dist.abs();
        let toward = if near(-d) < near(d) { -d } else { d };
        if dist < 0.0 {
            -toward
        } else {
            toward
        }
    }

    pub fn offset_entities(&mut self, si: usize, eids: &[Id], dist: f64) -> usize {
        let (pts, ents) = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            let ents: Vec<SketchEntity> = s.entities.iter().filter(|e| eids.contains(&e.id)).cloned().collect();
            (s.points.clone(), ents)
        };
        let mut count = 0;
        let before: std::collections::HashSet<Id> = self.sketches[si].entities.iter().map(|e| e.id).collect();
        // A circle entity gives a concentric circle (r plus or minus dist) rather than a polygon, HELD TO ITS SOURCE: the
        // centres concentric and the radii apart by the distance, a dimension of the offset. Laid free, the copy was a
        // circle of its own - five freedoms where the pair has two, and moving or resizing the source left it behind.
        for e in &ents {
            if let Some(crate::model::CircleParts { center, r }) = e.kind.as_circle() {
                if let Some(c) = pts.iter().find(|p| p.id == center) {
                    let nr = r + dist;
                    if nr > 0.05 {
                        let copy = self.add_circle_entity(si, c.x, c.y, nr, crate::feature::Purpose::of(e.construction));
                        let copy_centre = self.sketches[si].entities.iter().find(|x| x.id == copy).and_then(|x| match x.kind.as_circle() {
                            Some(crate::model::CircleParts { center, .. }) => Some(center),
                            _ => None,
                        });
                        if let Some(cc) = copy_centre.filter(|cc| *cc != center) {
                            let (m1, m2) = if dist > 0.0 { (-1, 1) } else { (1, -1) }; // r(copy) - r(source) = dist
                            self.sketches[si].constraints.push(Constraint::Concentric { c1: center, c2: cc });
                            self.sketches[si].constraints.push(Constraint::EdgeDistance { c1: center, c2: cc, d: dist.abs(), m1, m2, off: 0.0, expr: String::new(), driven: false, at: None });
                        }
                        count += 1;
                    }
                }
            }
        }
        // Lines and arcs are collected into bulge loops, offset with the arcs preserved, and reassembled into
        // lines and arcs - HELD TO THEIR SOURCE, as the circle is. Laid free, a rectangle's copy had eight freedoms of
        // its own, and resizing the rectangle left the copy where it was. See `hold_offset` for how.
        let src_lines: Vec<SourceLine> = ents
            .iter()
            .filter(|e| !e.construction)
            .filter_map(|e| match e.kind.as_line() {
                Some(crate::model::LineParts { a, b }) => Some(SourceLine { a, b, pa: at_of(&pts, a)?, pb: at_of(&pts, b)? }),
                _ => None,
            })
            .collect();
        let src_arcs: Vec<SourceArc> = ents
            .iter()
            .filter(|e| !e.construction)
            .filter_map(|e| match e.kind.as_arc() {
                Some(crate::model::ArcParts { center, a, b, .. }) => {
                    let (c, p) = (at_of(&pts, center)?, at_of(&pts, a)?);
                    Some(SourceArc { center, a, b, at: c, radius: (p.0 - c.0).hypot(p.1 - c.1) })
                }
                _ => None,
            })
            .collect();
        for loop_v in entity_bulge_loops(&pts, &ents) {
            for oloop in crate::offset::offset_bulge(&loop_v, dist) {
                let n = oloop.len();
                if n < 2 {
                    continue;
                }
                let ids: Vec<Id> = oloop.iter().map(|v| self.sketch_point_at(si, v.x, v.y, 1e-6)).collect();
                let mut kinds: Vec<(EntityKind, OffsetSource)> = Vec::new();
                for k in 0..n {
                    let (v, w) = (oloop[k], oloop[(k + 1) % n]);
                    let (a, b) = (ids[k], ids[(k + 1) % n]);
                    if v.bulge.abs() < 1e-9 {
                        kinds.push((LineParts { a, b }.into(), offset_line_source(&src_lines, (v.x, v.y), (w.x, w.y), dist)));
                    } else {
                        let (cx, cy, ccw) = arc_center_from_bulge(v.x, v.y, w.x, w.y, v.bulge);
                        // a centre of its own even where the source's stands: the radius of an arc is kept by its centre
                        let center = self.alloc_id();
                        self.sketches[si].points.push(SketchPoint { id: center, x: cx, y: cy });
                        let rc = (v.x - cx).hypot(v.y - cy);
                        kinds.push((ArcParts { center, a, b, ccw }.into(), offset_arc_source(&src_arcs, &src_lines, (cx, cy), rc, dist)));
                    }
                }
                let held = side_of_place(hold_offset(&ids, &kinds, dist, OffsetShape::Closed), &self.sketches[si]);
                for (kind, _) in kinds {
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
                }
                self.sketches[si].constraints.extend(held);
                count += 1;
            }
        }
        // AN OPEN CHAIN of lines and arcs is offset to one side of its way - positive to the left - its joints kept
        // (the convex ones rounded about the source corner, the concave ones cut), its ends square to the ends of the
        // source, and held to the source as a loop is. Reported behaviour: a polyline of three lines chosen, Offset, a
        // distance - no copy, and the window said to choose something, as though nothing were chosen.
        for chain in entity_bulge_chains(&pts, &ents) {
            let src_id = |v: &crate::offset::BVert| pts.iter().find(|p| (p.x - v.x).hypot(p.y - v.y) < 1e-9).map(|p| p.id);
            let (Some(first), Some(last)) = (chain.first(), chain.last()) else { continue };
            let ends = [(*first, src_id(first)), (*last, src_id(last))];
            for piece in crate::offset::offset_bulge_open(&chain, dist) {
                let n = piece.len();
                let ids: Vec<Id> = piece.iter().map(|v| self.sketch_point_at(si, v.x, v.y, 1e-6)).collect();
                let mut kinds: Vec<(EntityKind, OffsetSource)> = Vec::new();
                for k in 0..n - 1 {
                    let (v, w) = (piece[k], piece[k + 1]);
                    let (a, b) = (ids[k], ids[k + 1]);
                    if v.bulge.abs() < 1e-9 {
                        kinds.push((LineParts { a, b }.into(), offset_line_source(&src_lines, (v.x, v.y), (w.x, w.y), dist)));
                    } else {
                        let (cx, cy, ccw) = arc_center_from_bulge(v.x, v.y, w.x, w.y, v.bulge);
                        let center = self.alloc_id();
                        self.sketches[si].points.push(SketchPoint { id: center, x: cx, y: cy });
                        let rc = (v.x - cx).hypot(v.y - cy);
                        kinds.push((ArcParts { center, a, b, ccw }.into(), offset_arc_source(&src_arcs, &src_lines, (cx, cy), rc, dist)));
                    }
                }
                // an end of the copy stands square to an end of the source where it is the offset away from it; a piece
                // cut short where the chain turns back on itself has an end of its own
                let end_of = |v: crate::offset::BVert| {
                    ends.iter().find(|(e, id)| id.is_some() && ((e.x - v.x).hypot(e.y - v.y) - dist.abs()).abs() < 1e-6 * (1.0 + dist.abs())).and_then(|(_, id)| id.map(|src| EndOf { src }))
                };
                let shape = OffsetShape::Open { first: end_of(piece[0]), last: end_of(piece[n - 1]) };
                let held = side_of_place(hold_offset(&ids, &kinds, dist, shape), &self.sketches[si]);
                for (kind, _) in kinds {
                    let id = self.alloc_id();
                    self.sketches[si].entities.push(SketchEntity { id, kind, construction: false });
                }
                self.sketches[si].constraints.extend(held);
                count += 1;
            }
        }
        // AN ELLIPSE, AN ARC OF ONE, A SPLINE: each offset alone, as splines within 0.01 mm of the true offset
        // (`Curve::offset`), HELD TO ITS SOURCE by a record of its own (`lay_held_offset`): laid free, it stayed where
        // it was when the source was edited
        for f in offset_freely(&self.sketches[si], &ents, dist) {
            if self.lay_held_offset(si, f.eid, f.left) {
                count += 1;
            }
        }
        if count > 0 {
            // a joint laid is solved at once: the copy cut at the crossing and the end of the line put on it
            if self.join_offset_chain(si, &before, dist) {
                self.solve_sketch(si);
            } else {
                self.regen_sketch(si);
            }
        }
        count
    }
}
