//! A CONTOUR: its edges, its loops, its area, orientation and transforms.

use super::*;

/// An exact profile edge for the B-rep kernel: a real curve rather than a tessellation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProfEdge {
    /// A segment from a to b.
    Line { a: Point2, b: Point2 },
    /// An arc from a to b about a centre, counter-clockwise when `ccw`.
    Arc { a: Point2, b: Point2, center: Point2, ccw: bool },
    /// A full circle (centre and radius), which is a closed contour in its own right.
    Circle { center: Point2, r: f64 },
}

/// A closed or open polyline contour in a plane (XY at the `z` of the operation).
///
/// `edges` holds the exact curves when they are known from the sketch entities; they yield an exact B-rep, a real
/// cylinder rather than a faceted one. When it is empty the kernel takes the `points` polyline as segments, which is
/// the case for imported DXF.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub points: Vec<Point2>,
    pub closed: bool,
    #[serde(default)]
    pub edges: Vec<ProfEdge>,
    /// The provenance of every edge: the id of the sketch entity it was cut from, parallel to `edges`.
    ///
    /// This is what makes face names stable. A side wall of an extrusion is named after the profile edge, so if the
    /// name of an edge were its ordinal, inserting an edge in the middle of a contour would shift every wall after
    /// it. The id of a sketch entity does not shift: a line stays itself however many neighbours are added. It is
    /// empty for contours not built from a sketch (import, offset), where names stay positional.
    #[serde(default)]
    pub edge_src: Vec<crate::model::Id>,
}

impl Contour {
    pub fn open(points: Vec<Point2>) -> Self {
        Self { points, closed: false, edges: Vec::new(), edge_src: Vec::new() }
    }

    pub fn closed(points: Vec<Point2>) -> Self {
        Self { points, closed: true, edges: Vec::new(), edge_src: Vec::new() }
    }

    /// One contour block of the kernel profile encoding: `[nedges, edge x EDGE_FIELDS...]`. Without exact edges the
    /// polyline is encoded as segments.
    ///
    /// `name_of` maps the sketch entity that produced an edge to the name descriptor of the face that edge will
    /// create (see `names::NameTable`). The id of the entity itself used to go into the encoding and the kernel
    /// appended an offset to it, which meant two places knew the naming scheme, the kernel and its Rust binding. The
    /// name is now derived entirely in Rust and the kernel only carries it.
    ///
    /// The edge itself goes to `name_of` beside its entity: one entity cut by its neighbours gives several pieces, in
    /// one contour or in several, and each piece is a face of its own.
    pub fn loop_block_named(&self, name_of: &dyn Fn(crate::model::Id, &ProfEdge) -> u32) -> Vec<f64> {
        let mut v = Vec::new();
        let src = |i: usize| self.edge_src.get(i).copied().unwrap_or(0);
        if !self.edges.is_empty() {
            v.push(self.edges.len() as f64);
            for (i, e) in self.edges.iter().enumerate() {
                v.extend_from_slice(&encode_edge(*e, name_of(src(i), e)));
            }
        } else {
            let n = self.points.len();
            v.push(n as f64);
            for i in 0..n {
                let e = ProfEdge::Line { a: self.points[i], b: self.points[(i + 1) % n] };
                v.extend_from_slice(&encode_edge(e, name_of(src(i), &e)));
            }
        }
        v
    }

    /// The edges of the contour with the sketch entity each came from: the exact edges, or the polyline taken as
    /// segments when there are none - the same edges `loop_block_named` encodes.
    pub fn sourced_edges(&self) -> Vec<(crate::model::Id, ProfEdge)> {
        let src = |i: usize| self.edge_src.get(i).copied().unwrap_or(0);
        if !self.edges.is_empty() {
            self.edges.iter().enumerate().map(|(i, e)| (src(i), *e)).collect()
        } else {
            let n = self.points.len();
            (0..n).map(|i| (src(i), ProfEdge::Line { a: self.points[i], b: self.points[(i + 1) % n] })).collect()
        }
    }

    /// A contour block without names (0 means the provenance is unknown): profiles that do not come from a sketch,
    /// such as a thread computed from a standard, and tests of the format.
    pub fn loop_block(&self) -> Vec<f64> {
        self.loop_block_named(&|_, _| 0)
    }

    /// Bring the contour to canonical form. The exact edges are the single source of truth about it.
    ///
    /// A contour is described twice, by the `points` polyline and by the exact `edges`, and it is the edges that
    /// reach the kernel. Nothing used to reconcile the two descriptions, and they diverged in winding direction: on
    /// one part the points of the outer loop measured counter-clockwise while the edges ran clockwise. OCCT does not
    /// check orientation, so a hole became co-directed with the outer loop, its area added instead of subtracting,
    /// the body came out as a slab (48835 mm² instead of 9020) and the chamfers failed after it.
    ///
    /// The cure is applied at the root: when exact edges are present the polyline is recomputed from them, and a
    /// closed contour is brought to the canonical counter-clockwise winding, leaving nothing to diverge. Contours
    /// without exact edges (DXF or STL import) are canonicalised from the polyline itself.
    pub fn canonicalize(&mut self) {
        if !self.edges.is_empty() {
            // 1) The winding of the edges: a closed contour must run counter-clockwise.
            if self.closed && self.edges_signed_area() < 0.0 {
                self.edges.reverse();
                self.edge_src.reverse(); // Provenance travels with the edges, otherwise face names shift.
                for e in self.edges.iter_mut() {
                    *e = match *e {
                        ProfEdge::Line { a, b } => ProfEdge::Line { a: b, b: a },
                        ProfEdge::Arc { a, b, center, ccw } => ProfEdge::Arc { a: b, b: a, center, ccw: !ccw },
                        c @ ProfEdge::Circle { .. } => c,
                    };
                }
            }
            // 2) The polyline is derived from the edges rather than being an independent description.
            self.points = self.points_from_edges();
        } else if self.closed && self.signed_area() < 0.0 {
            self.points.reverse();
        }
    }

    /// The polyline built from the exact edges, in winding order. Arcs and circles are tessellated to a sagitta
    /// proportional to the radius, as in the sketch tessellation.
    pub fn points_from_edges(&self) -> Vec<Point2> {
        let mut pts: Vec<Point2> = Vec::new();
        for e in &self.edges {
            match *e {
                ProfEdge::Line { a, b } => {
                    if pts.is_empty() {
                        pts.push(a);
                    }
                    pts.push(b);
                }
                ProfEdge::Arc { a, b, center, ccw } => {
                    let r = (a.x - center.x).hypot(a.y - center.y);
                    let (g0, g1) = ((a.y - center.y).atan2(a.x - center.x), (b.y - center.y).atan2(b.x - center.x));
                    let arc = tessellate_arc(center.x, center.y, r, g0, g1, ccw, (r * 0.0016).clamp(0.003, 0.05));
                    if pts.is_empty() {
                        if let Some(f) = arc.first() {
                            pts.push(*f);
                        }
                    }
                    pts.extend(arc.iter().skip(1).cloned());
                }
                ProfEdge::Circle { center, r } => {
                    pts.extend(circle_contour(center.x, center.y, r, (r * 0.0016).clamp(0.003, 0.05)).points);
                }
            }
        }
        // A closed contour does not repeat its start point at the end.
        if self.closed {
            if let (Some(f), Some(l)) = (pts.first().copied(), pts.last().copied()) {
                if (f.x - l.x).abs() < 1e-9 && (f.y - l.y).abs() < 1e-9 && pts.len() > 1 {
                    pts.pop();
                }
            }
        }
        pts
    }

    /// The signed area computed from the exact edges rather than the polyline; the sign gives the winding.
    pub fn edges_signed_area(&self) -> f64 {
        if self.edges.is_empty() {
            return self.signed_area();
        }
        let pts = self.points_from_edges();
        if pts.len() < 3 {
            // A single circular edge: the circle itself sets the direction, always counter-clockwise.
            return if matches!(self.edges.first(), Some(ProfEdge::Circle { .. })) { 1.0 } else { 0.0 };
        }
        let mut a = 0.0;
        for i in 0..pts.len() {
            let j = (i + 1) % pts.len();
            a += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
        }
        0.5 * a
    }

    pub fn signed_area(&self) -> f64 {
        let p = &self.points;
        if p.len() < 3 {
            return 0.0;
        }
        let mut a = 0.0;
        for i in 0..p.len() {
            let j = (i + 1) % p.len();
            a += p[i].x * p[j].y - p[j].x * p[i].y;
        }
        a * 0.5
    }

    pub fn area(&self) -> f64 {
        self.signed_area().abs()
    }

    pub fn is_ccw(&self) -> bool {
        self.signed_area() > 0.0
    }

    /// Force a given winding direction (`true` is counter-clockwise).
    pub fn orient(&mut self, ccw: bool) {
        if self.is_ccw() != ccw {
            self.points.reverse();
        }
    }

    pub fn bbox(&self) -> Option<Bbox> {
        let mut it = self.points.iter();
        let first = it.next()?;
        let mut min = *first;
        let mut max = *first;
        for p in it {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Some(Bbox { min, max })
    }

    /// Translate every point.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        for p in &mut self.points {
            p.x += dx;
            p.y += dy;
        }
        self.edges.clear(); // Hand-moving a standalone contour leaves a polyline; sketch contours are regenerated.
    }

    /// Rotate about point `c` by `deg` degrees.
    pub fn rotate(&mut self, c: Point2, deg: f64) {
        let (s, co) = deg.to_radians().sin_cos();
        for p in &mut self.points {
            let (dx, dy) = (p.x - c.x, p.y - c.y);
            p.x = c.x + dx * co - dy * s;
            p.y = c.y + dx * s + dy * co;
        }
        self.edges.clear();
    }

    /// Scale about point `c`.
    pub fn scale(&mut self, c: Point2, factor: f64) {
        for p in &mut self.points {
            p.x = c.x + (p.x - c.x) * factor;
            p.y = c.y + (p.y - c.y) * factor;
        }
        self.edges.clear();
    }

    /// Mirror the contour: `flip_x` reflects about the vertical axis `x = axis` (changing X), otherwise about the
    /// horizontal axis `y = axis` (changing Y). The order of the points is reversed to preserve the winding, which
    /// decides the side an offset falls on.
    pub fn mirror(&mut self, flip_x: bool, axis: f64) {
        for p in &mut self.points {
            if flip_x {
                p.x = 2.0 * axis - p.x;
            } else {
                p.y = 2.0 * axis - p.y;
            }
        }
        self.points.reverse();
    }

    /// The centroid of the polygon, used for drill points and similar.
    pub fn centroid(&self) -> Point2 {
        let p = &self.points;
        if p.is_empty() {
            return Point2::new(0.0, 0.0);
        }
        let a = self.signed_area();
        if a.abs() < 1e-9 {
            // Degenerate contour: fall back to the mean of the points.
            let (sx, sy) = p.iter().fold((0.0, 0.0), |(x, y), q| (x + q.x, y + q.y));
            return Point2::new(sx / p.len() as f64, sy / p.len() as f64);
        }
        let (mut cx, mut cy) = (0.0, 0.0);
        for i in 0..p.len() {
            let j = (i + 1) % p.len();
            let cross = p[i].x * p[j].y - p[j].x * p[i].y;
            cx += (p[i].x + p[j].x) * cross;
            cy += (p[i].y + p[j].y) * cross;
        }
        Point2::new(cx / (6.0 * a), cy / (6.0 * a))
    }

    /// Returns (centre, radius) when the contour is close to a circle, otherwise `None`.
    pub fn as_circle(&self) -> Option<(Point2, f64)> {
        if !self.closed || self.points.len() < 8 {
            return None;
        }
        let c = self.centroid();
        let rs: Vec<f64> = self.points.iter().map(|p| p.dist(c)).collect();
        let mean = rs.iter().sum::<f64>() / rs.len() as f64;
        if mean < 1e-6 {
            return None;
        }
        let max_dev = rs.iter().map(|r| (r - mean).abs()).fold(0.0, f64::max);
        if max_dev / mean < 0.08 {
            Some((c, mean))
        } else {
            None
        }
    }

    /// Whether a point lies inside a closed contour, by ray casting.
    pub fn contains(&self, pt: Point2) -> bool {
        let p = &self.points;
        if p.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = p.len() - 1;
        for i in 0..p.len() {
            let (pi, pj) = (p[i], p[j]);
            if (pi.y > pt.y) != (pj.y > pt.y) {
                let x_cross = (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y) + pi.x;
                if pt.x < x_cross {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }

    /// The perimeter, used to estimate path length and machining time.
    pub fn length(&self) -> f64 {
        let p = &self.points;
        if p.len() < 2 {
            return 0.0;
        }
        let mut total = 0.0;
        for i in 0..p.len() - 1 {
            total += p[i].dist(p[i + 1]);
        }
        if self.closed {
            total += p[p.len() - 1].dist(p[0]);
        }
        total
    }
}
