//! Basic geometry, in f64 for CAD accuracy.
//!
//! A contour is a polyline; arcs are tessellated on import. Arc segments (bulge) for an arc-aware offset can be
//! added later if the need arises.

use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn sub(self, o: Point2) -> Point2 {
        Point2::new(self.x - o.x, self.y - o.y)
    }

    pub fn add(self, o: Point2) -> Point2 {
        Point2::new(self.x + o.x, self.y + o.y)
    }

    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn dist(self, o: Point2) -> f64 {
        self.sub(o).len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// A 2D point raised to height `z`.
    pub fn at(p: Point2, z: f64) -> Self {
        Self { x: p.x, y: p.y, z }
    }
}

/// An axis-aligned bounding rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub min: Point2,
    pub max: Point2,
}

impl Bbox {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
}

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
    pub fn loop_block_named(&self, name_of: &dyn Fn(crate::model::Id) -> u32) -> Vec<f64> {
        let mut v = Vec::new();
        let name = |i: usize| -> u32 { name_of(self.edge_src.get(i).copied().unwrap_or(0)) };
        if !self.edges.is_empty() {
            v.push(self.edges.len() as f64);
            for (i, e) in self.edges.iter().enumerate() {
                v.extend_from_slice(&encode_edge(*e, name(i)));
            }
        } else {
            let n = self.points.len();
            v.push(n as f64);
            for i in 0..n {
                let (a, b) = (self.points[i], self.points[(i + 1) % n]);
                v.extend_from_slice(&encode_edge(ProfEdge::Line { a, b }, name(i)));
            }
        }
        v
    }

    /// A contour block without names (0 means the provenance is unknown): profiles that do not come from a sketch,
    /// such as a thread computed from a standard, and tests of the format.
    pub fn loop_block(&self) -> Vec<f64> {
        self.loop_block_named(&|_| 0)
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

/// An axis-aligned bounding box in 3D.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bbox3 {
    pub min: Point3,
    pub max: Point3,
}

/// A triangle mesh, used for STL and STEP output and for 3D machining.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub verts: Vec<Point3>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn triangle(&self, i: usize) -> [Point3; 3] {
        let t = self.tris[i];
        [self.verts[t[0] as usize], self.verts[t[1] as usize], self.verts[t[2] as usize]]
    }

    /// The volume of a closed mesh, as the sum of the signed volumes of tetrahedra over the origin. It is needed
    /// where no live B-rep is at hand and the result of an operation still has to be verified: a thread that removed
    /// nothing is a failure, not a success. On an open mesh the value is meaningless.
    pub fn volume(&self) -> f64 {
        let mut v = 0.0;
        for i in 0..self.tris.len() {
            let t = self.triangle(i);
            v += (t[0].x * (t[1].y * t[2].z - t[2].y * t[1].z) - t[0].y * (t[1].x * t[2].z - t[2].x * t[1].z)
                + t[0].z * (t[1].x * t[2].y - t[2].x * t[1].y))
                / 6.0;
        }
        v.abs()
    }

    pub fn bounds(&self) -> Option<Bbox3> {
        let mut it = self.verts.iter();
        let first = *it.next()?;
        let (mut mn, mut mx) = (first, first);
        for v in it {
            mn.x = mn.x.min(v.x);
            mn.y = mn.y.min(v.y);
            mn.z = mn.z.min(v.z);
            mx.x = mx.x.max(v.x);
            mx.y = mx.y.max(v.y);
            mx.z = mx.z.max(v.z);
        }
        Some(Bbox3 { min: mn, max: mx })
    }

    /// Translate the mesh by the given offsets, for example to drop its lowest Z onto the table or to centre it in
    /// XY about the origin.
    pub fn translate(&mut self, dx: f64, dy: f64, dz: f64) {
        for v in &mut self.verts {
            v.x += dx;
            v.y += dy;
            v.z += dz;
        }
    }

    /// Apply an affine 3x4 row-major matrix (axes and translation) to every vertex.
    pub fn transform(&mut self, m: &[f64; 12]) {
        for v in &mut self.verts {
            let (x, y, z) = (v.x, v.y, v.z);
            v.x = m[0] * x + m[1] * y + m[2] * z + m[3];
            v.y = m[4] * x + m[5] * y + m[6] * z + m[7];
            v.z = m[8] * x + m[9] * y + m[10] * z + m[11];
        }
    }

    /// Rotate about the vertical axis through (cx, cy) by `deg` degrees.
    pub fn rotate_z(&mut self, cx: f64, cy: f64, deg: f64) {
        let (s, co) = deg.to_radians().sin_cos();
        for v in &mut self.verts {
            let (dx, dy) = (v.x - cx, v.y - cy);
            v.x = cx + dx * co - dy * s;
            v.y = cy + dx * s + dy * co;
        }
    }

    /// Scale uniformly about the point (cx, cy, cz).
    pub fn scale(&mut self, cx: f64, cy: f64, cz: f64, f: f64) {
        for v in &mut self.verts {
            v.x = cx + (v.x - cx) * f;
            v.y = cy + (v.y - cy) * f;
            v.z = cz + (v.z - cz) * f;
        }
    }

    /// The normal, area and centroid of triangle `i`.
    pub fn tri_normal_area(&self, i: usize) -> ([f64; 3], f64, Point3) {
        let t = self.triangle(i);
        let u = [t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z];
        let v = [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z];
        let c = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        let n = if len > 1e-12 { [c[0] / len, c[1] / len, c[2] / len] } else { [0.0, 0.0, 1.0] };
        let ctr = Point3::new(
            (t[0].x + t[1].x + t[2].x) / 3.0,
            (t[0].y + t[1].y + t[2].y) / 3.0,
            (t[0].z + t[1].z + t[2].z) / 3.0,
        );
        (n, len * 0.5, ctr)
    }

    /// Smoothed per-vertex normals for Gouraud shading. The normal of a vertex is the average of the normals of the
    /// adjacent triangles weighted by their area, so larger facets weigh more, which is stable under subdivision.
    ///
    /// This yields correct smoothing groups without any angle heuristic because the mesh from OCCT is assembled face
    /// by face: the vertices of every B-rep face live in their own block (see `add_face` in occt_bridge). Across a
    /// sharp edge between two faces the vertices are therefore duplicated — different indices at the same position —
    /// and are not averaged, so the edge stays sharp. Within one smooth face, a cylinder or a sphere, the triangles
    /// share indices, the normals merge and the surface is smooth. The topology of the mesh already encodes the
    /// smoothing groups, so no separate pass over angles is needed. Planar faces yield the same normal either way.
    pub fn vertex_normals(&self) -> Vec<[f64; 3]> {
        let mut acc = vec![[0.0_f64; 3]; self.verts.len()];
        for i in 0..self.tris.len() {
            // The unnormalised triangle normal: its length is twice the area, a natural area weight.
            let t = self.triangle(i);
            let u = [t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z];
            let v = [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z];
            let c = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            for &vi in &self.tris[i] {
                let a = &mut acc[vi as usize];
                a[0] += c[0];
                a[1] += c[1];
                a[2] += c[2];
            }
        }
        for a in &mut acc {
            let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
            if len > 1e-12 {
                a[0] /= len;
                a[1] /= len;
                a[2] /= len;
            } else {
                *a = [0.0, 0.0, 1.0];
            }
        }
        acc
    }

    /// Detect faces: adjacent coplanar triangles are merged into one planar face by growing a region along the
    /// normal. `angle_tol_deg` is the permitted deviation.
    pub fn detect_faces(&self, angle_tol_deg: f64) -> Vec<MeshFace> {
        use std::collections::HashMap;
        let nt = self.tris.len();
        if nt == 0 {
            return Vec::new();
        }
        // A map from an edge to the triangles that share it.
        let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, t) in self.tris.iter().enumerate() {
            for e in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let key = if e.0 < e.1 { (e.0, e.1) } else { (e.1, e.0) };
                edges.entry(key).or_default().push(i);
            }
        }
        let norms: Vec<[f64; 3]> = (0..nt).map(|i| self.tri_normal_area(i).0).collect();
        let cos_tol = angle_tol_deg.to_radians().cos();
        let mut visited = vec![false; nt];
        let mut faces = Vec::new();

        for seed in 0..nt {
            if visited[seed] {
                continue;
            }
            let sn = norms[seed];
            visited[seed] = true;
            let mut stack = vec![seed];
            let mut tris_in = Vec::new();
            let (mut acc_n, mut area, mut acc_c) = ([0.0; 3], 0.0, [0.0; 3]);
            while let Some(t) = stack.pop() {
                tris_in.push(t as u32);
                let (n, a, c) = self.tri_normal_area(t);
                for k in 0..3 {
                    acc_n[k] += n[k] * a;
                }
                acc_c[0] += c.x * a;
                acc_c[1] += c.y * a;
                acc_c[2] += c.z * a;
                area += a;
                let tr = self.tris[t];
                for e in [(tr[0], tr[1]), (tr[1], tr[2]), (tr[2], tr[0])] {
                    let key = if e.0 < e.1 { (e.0, e.1) } else { (e.1, e.0) };
                    if let Some(adj) = edges.get(&key) {
                        for &j in adj {
                            if !visited[j] {
                                let nj = norms[j];
                                if nj[0] * sn[0] + nj[1] * sn[1] + nj[2] * sn[2] >= cos_tol {
                                    visited[j] = true;
                                    stack.push(j);
                                }
                            }
                        }
                    }
                }
            }
            let nl = (acc_n[0] * acc_n[0] + acc_n[1] * acc_n[1] + acc_n[2] * acc_n[2]).sqrt().max(1e-12);
            let ar = area.max(1e-9);
            faces.push(MeshFace {
                triangles: tris_in,
                normal: [acc_n[0] / nl, acc_n[1] / nl, acc_n[2] / nl],
                centroid: Point3::new(acc_c[0] / ar, acc_c[1] / ar, acc_c[2] / ar),
                area,
                id: 0, // Mesh detection has no B-rep, so the id is unknown.
            });
        }
        // `partial_cmp(..).unwrap()` brought the application down on a NaN area, which comes from a degenerate
        // triangle in a malformed STL, and importing foreign meshes is routine. The total order of `total_cmp`
        // rules the panic out, and the NaN faces themselves are dropped: there is nothing to compute from them.
        faces.retain(|f| f.area.is_finite() && f.normal.iter().all(|v| v.is_finite()));
        faces.sort_by(|a, b| b.area.total_cmp(&a.area));
        faces
    }

    /// Build a face from a set of triangle indices, with the normal, centroid and area weighted by area. Used for
    /// faces that come from real B-rep topology.
    pub fn meshface_from_triangles(&self, triangles: Vec<u32>) -> MeshFace {
        let (mut acc_n, mut area, mut acc_c) = ([0.0_f64; 3], 0.0_f64, [0.0_f64; 3]);
        for &ti in &triangles {
            let (n, a, c) = self.tri_normal_area(ti as usize);
            for k in 0..3 {
                acc_n[k] += n[k] * a;
            }
            acc_c[0] += c.x * a;
            acc_c[1] += c.y * a;
            acc_c[2] += c.z * a;
            area += a;
        }
        let nl = (acc_n[0] * acc_n[0] + acc_n[1] * acc_n[1] + acc_n[2] * acc_n[2]).sqrt().max(1e-12);
        let ar = area.max(1e-9);
        MeshFace {
            triangles,
            normal: [acc_n[0] / nl, acc_n[1] / nl, acc_n[2] / nl],
            centroid: Point3::new(acc_c[0] / ar, acc_c[1] / ar, acc_c[2] / ar),
            area,
            id: 0, // The kernel fills in the persistent id (doc_to_bodies); mesh detection leaves it at zero.
        }
    }
}

/// A detected planar face of a mesh: a group of coplanar triangles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshFace {
    pub triangles: Vec<u32>,
    pub normal: [f64; 3],
    pub centroid: Point3,
    pub area: f64,
    /// The persistent id of the face from the B-rep, stable across a rebuild of the recipe. 0 means unknown, as
    /// after mesh detection.
    #[serde(default)]
    pub id: u32,
}

/// An edge of a body from the B-rep: a persistent id together with a midpoint and a tangent, which an axis
/// connector anchors to. Like `MeshFace` it is derived from the kernel and is not stored; a regenerate restores it.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct MeshEdge {
    /// The persistent id of the edge from the B-rep: stable across a rebuild, so a reference does not drift.
    pub id: u32,
    /// The midpoint of the edge, by polyline length, in the local frame of the body.
    pub mid: [f64; 3],
    /// The unit tangent, the direction of the edge, at that midpoint.
    pub dir: [f64; 3],
    /// The start of the edge, the first point of the polyline, which a vertex connector anchors to.
    #[serde(default)]
    pub a: [f64; 3],
    /// The end of the edge, the last point of the polyline, which a vertex connector anchors to.
    #[serde(default)]
    pub b: [f64; 3],
    /// For a circular or arc edge, the centre of the circle in the local frame of the body; otherwise unset (see
    /// `radius`). An axis connector or a datum built on a circular edge, such as the rim of a hole, anchors at the
    /// centre rather than on the rim.
    #[serde(default)]
    pub center: [f64; 3],
    /// The axis of the circle, the unit normal to its plane. A zero vector when the edge is not circular.
    #[serde(default)]
    pub axis: [f64; 3],
    /// The radius of the circle or arc; `0` means the edge is not circular (a line or spline), so the anchor uses
    /// `mid` and `dir`.
    #[serde(default)]
    pub radius: f64,
    /// The reference direction of the edge: the normal of the adjacent face at the midpoint of the edge, in the
    /// local frame of the body. A zero vector means there is no adjacent face (a dangling edge) or its normal is
    /// undefined.
    ///
    /// An edge carries only one axis of its own, the one along it. The second axis, the roll of a connector, used to
    /// be derived from the world Z axis, that is from however the part happened to lie. Two parts being mated then
    /// derived different axes and the joint placed the part at an arbitrary roll. Taking the secondary axis from the
    /// adjacent face removes that dependency.
    #[serde(default)]
    pub ref_dir: [f64; 3],
}

impl MeshEdge {
    /// Whether the edge is a circle or arc with a real centre and axis, which gives a concentric anchor for holes
    /// and cylinders.
    pub fn is_circular(&self) -> bool {
        self.radius > 1e-9
    }
    /// The axis reference of the edge: (centre, circle axis) for a circular edge, otherwise (midpoint, tangent).
    pub fn axis_ref(&self) -> ([f64; 3], [f64; 3]) {
        if self.is_circular() {
            (self.center, self.axis)
        } else {
            (self.mid, self.dir)
        }
    }
}

impl Mesh {
    /// The boundary of a face — the edges belonging to a single triangle of that face — stitched into closed
    /// contours projected onto XY. Useful for the top and bottom planar faces.
    pub fn face_outline_xy(&self, face: &MeshFace) -> Vec<Contour> {
        use std::collections::HashMap;
        let mut edge_count: HashMap<(u32, u32), (u32, (u32, u32))> = HashMap::new();
        for &ti in &face.triangles {
            let t = self.tris[ti as usize];
            for e in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let key = if e.0 < e.1 { (e.0, e.1) } else { (e.1, e.0) };
                let ent = edge_count.entry(key).or_insert((0, e));
                ent.0 += 1;
            }
        }
        let mut segs = Vec::new();
        for (_, (cnt, (a, b))) in edge_count {
            if cnt == 1 {
                let pa = self.verts[a as usize];
                let pb = self.verts[b as usize];
                segs.push((Point2::new(pa.x, pa.y), Point2::new(pb.x, pb.y)));
            }
        }
        stitch_segments(segs, 1e-3)
    }
}

/// Stitch segments into polylines by their coincident endpoints.
pub fn stitch_segments(segs: Vec<(Point2, Point2)>, tol: f64) -> Vec<Contour> {
    let mut chains: Vec<Vec<Point2>> = segs.into_iter().map(|(a, b)| vec![a, b]).collect();
    let mut out: Vec<Contour> = Vec::new();
    while let Some(mut chain) = chains.pop() {
        loop {
            // A chain is non-empty by construction, since each one is born from a segment of two points, but that
            // invariant must not be enforced by a panic: a CAD core has no business dying on data. If it is
            // violated, leave the chain rather than take the application down along with unsaved work.
            let (Some(&head), Some(&tail)) = (chain.first(), chain.last()) else { break };
            let mut matched = None;
            for (i, s) in chains.iter().enumerate() {
                let (Some(&a), Some(&b)) = (s.first(), s.last()) else { continue };
                if b.dist(head) <= tol {
                    matched = Some((i, true, false));
                    break;
                } else if a.dist(head) <= tol {
                    matched = Some((i, true, true));
                    break;
                } else if a.dist(tail) <= tol {
                    matched = Some((i, false, false));
                    break;
                } else if b.dist(tail) <= tol {
                    matched = Some((i, false, true));
                    break;
                }
            }
            match matched {
                Some((i, at_head, rev)) => {
                    let mut s = chains.remove(i);
                    if rev {
                        s.reverse();
                    }
                    if at_head {
                        s.pop();
                        s.extend(chain);
                        chain = s;
                    } else {
                        chain.extend(s.into_iter().skip(1));
                    }
                }
                None => break,
            }
        }
        let mut pts: Vec<Point2> = Vec::with_capacity(chain.len());
        for p in chain {
            if pts.last().map_or(true, |l: &Point2| l.dist(p) > tol) {
                pts.push(p);
            }
        }
        if pts.len() < 2 {
            continue;
        }
        // `pts` is non-empty by the check above, but it is read safely: an invariant is no reason to panic.
        let ends = pts.first().copied().zip(pts.last().copied());
        let gap = ends.map(|(f, l)| f.dist(l)).unwrap_or(f64::INFINITY);
        let closed = pts.len() >= 3 && gap <= tol * 4.0;
        if closed {
            if gap <= tol {
                pts.pop();
            }
            out.push(Contour::closed(pts));
        } else {
            out.push(Contour::open(pts));
        }
    }
    out
}

/// Dogbone corner relief points: for every sharp corner of a closed contour, a pair of (vertex, tip of the overcut)
/// running outwards along the bisector. It points away from the centre of the contour, into the wall, so that the
/// resulting socket accepts a square corner.
pub fn dogbone_overcuts(contour: &Contour, length: f64) -> Vec<(Point2, Point2)> {
    let p = &contour.points;
    let n = p.len();
    if !contour.closed || n < 3 {
        return Vec::new();
    }
    let centroid = contour.centroid();
    let norm = |v: Point2| {
        let l = v.len().max(1e-9);
        Point2::new(v.x / l, v.y / l)
    };
    let mut out = Vec::new();
    for i in 0..n {
        let prev = p[(i + n - 1) % n];
        let v = p[i];
        let next = p[(i + 1) % n];
        let t1 = norm(v.sub(prev));
        let t2 = norm(next.sub(v));
        // Sharp corners only, meaning a turn of more than about 60 degrees.
        if t1.x * t2.x + t1.y * t2.y > 0.5 {
            continue;
        }
        let mut bis = norm(t2.sub(t1));
        let outward = v.sub(centroid);
        if bis.x * outward.x + bis.y * outward.y < 0.0 {
            bis = Point2::new(-bis.x, -bis.y);
        }
        out.push((v, Point2::new(v.x + bis.x * length, v.y + bis.y * length)));
    }
    out
}

/// The nesting depth of contour `i` within `all`: how many other closed contours contain it. An even depth (0, 2,
/// ...) marks an outer boundary, an odd one marks a hole.
pub fn nesting_depth(all: &[Contour], i: usize) -> usize {
    let me = &all[i];
    if me.points.is_empty() {
        return 0;
    }
    let probe = me.centroid();
    all.iter()
        .enumerate()
        .filter(|(k, c)| *k != i && c.closed && c.points.len() >= 3 && c.contains(probe))
        .count()
}

/// Tessellate an arc into points. The angles are in radians and `ccw` gives the direction. `max_sag` is the largest
/// permitted deviation of a chord from the arc, that is the accuracy of the linearisation.
pub fn tessellate_arc(
    cx: f64,
    cy: f64,
    radius: f64,
    start: f64,
    end: f64,
    ccw: bool,
    max_sag: f64,
) -> Vec<Point2> {
    // Normalise the swept angle to the requested direction.
    let mut sweep = end - start;
    if ccw {
        while sweep <= 0.0 {
            sweep += TAU;
        }
    } else {
        while sweep >= 0.0 {
            sweep -= TAU;
        }
    }

    // The segment count follows from the chord sagitta tolerance: max_sag = r(1 - cos(dtheta/2)).
    let sag = max_sag.max(1e-6).min(radius);
    let dtheta_max = 2.0 * (1.0 - sag / radius).clamp(-1.0, 1.0).acos();
    let segs = (sweep.abs() / dtheta_max.max(1e-3)).ceil().max(1.0) as usize;

    let mut pts = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let t = start + sweep * (i as f64 / segs as f64);
        pts.push(Point2::new(cx + radius * t.cos(), cy + radius * t.sin()));
    }
    pts
}

/// A circle as a closed contour.
pub fn circle_contour(cx: f64, cy: f64, radius: f64, max_sag: f64) -> Contour {
    let mut pts = tessellate_arc(cx, cy, radius, 0.0, TAU, true, max_sag);
    pts.pop(); // On a full circle the last point coincides with the first.
    let mut c = Contour::closed(pts);
    // The exact circular edge for the B-rep, which yields a real cylinder when extruded.
    c.edges = vec![ProfEdge::Circle { center: Point2::new(cx, cy), r: radius }];
    c
}

/// A circular contour carrying provenance: one exact edge produced by the sketch entity `src`.
///
/// This is a separate function rather than a field every call site must remember to fill in, because that is exactly
/// how provenance was once lost: the cylindrical wall of a hole was named positionally, although it is the most
/// frequent target of references (a fillet on the rim of a hole, a sketch on its wall).
pub fn circle_contour_from(cx: f64, cy: f64, radius: f64, max_sag: f64, src: crate::model::Id) -> Contour {
    let mut c = circle_contour(cx, cy, radius, max_sag);
    c.edge_src = vec![src];
    c
}

/// How many numbers one edge occupies in the profile encoding.
pub const EDGE_FIELDS: usize = 9;

/// The single source of truth for the edge format of a kernel profile:
/// `[kind, ax, ay, bx, by, cx, cy, ccw, name]`, where kind 0 is a segment, 1 an arc and 2 a circle.
///
/// The ninth number is the name descriptor of the face this edge will produce (0 means no name, and the kernel
/// falls back to a positional index). The name is derived from the recipe as "the wall of feature F from sketch
/// entity E", so inserting an edge in the middle of a contour does not shift the names of its neighbours.
///
/// The format used to be written out in six places (the contour, threads and four sets of tests), and widening the
/// record from 8 numbers to 9 left them out of step: the kernel read the profile with a shift and removed 74 mm³
/// instead of 7515. The record is therefore assembled here alone, both by the product and by the tests.
pub fn encode_edge(e: ProfEdge, name: u32) -> [f64; EDGE_FIELDS] {
    let s = name as f64;
    match e {
        ProfEdge::Line { a, b } => [0.0, a.x, a.y, b.x, b.y, 0.0, 0.0, 0.0, s],
        ProfEdge::Arc { a, b, center, ccw } => [1.0, a.x, a.y, b.x, b.y, center.x, center.y, if ccw { 1.0 } else { 0.0 }, s],
        ProfEdge::Circle { center, r } => [2.0, r, 0.0, 0.0, 0.0, center.x, center.y, 0.0, s],
    }
}

/// One loop of exact edges: `[nedges, edges...]`. Names are absent (0) because the profile does not come from a
/// sketch.
pub fn encode_loop(edges: &[ProfEdge]) -> Vec<f64> {
    let mut v = vec![edges.len() as f64];
    for e in edges {
        v.extend_from_slice(&encode_edge(*e, 0));
    }
    v
}

/// A profile built straight from loops of exact edges: `[nloops, then nedges and the edges of each loop]`. The
/// first loop is the outer one and the rest are holes.
pub fn encode_loops(loops: &[&[ProfEdge]]) -> Vec<f64> {
    let mut v = vec![loops.len() as f64];
    for l in loops {
        v.extend(encode_loop(l));
    }
    v
}

/// The profile encoding for the kernel: an outer contour plus holes flattened into `[L, loop blocks...]` (see
/// `qym_shape_extrude_profile`). Exact edges yield exact faces. `name_of` maps a sketch entity to the name
/// descriptor of the side face it will produce.
pub fn encode_profile_named(outer: &Contour, holes: &[&Contour], name_of: &dyn Fn(crate::model::Id) -> u32) -> Vec<f64> {
    let mut v = vec![(1 + holes.len()) as f64];
    v.extend(outer.loop_block_named(name_of));
    for h in holes {
        v.extend(h.loop_block_named(name_of));
    }
    v
}

/// The same without names, for profiles that do not come from a sketch.
pub fn encode_profile(outer: &Contour, holes: &[&Contour]) -> Vec<f64> {
    encode_profile_named(outer, holes, &|_| 0)
}

/// Is a cylindrical face a hole or a shaft? This decides which thread to build: on a hole the material lies outside
/// the surface, on a shaft inside it, and the groove profiles are mirror images of each other.
///
/// The answer is computed per triangle of the face: whether its own normal looks away from the axis or towards it,
/// weighted by area. The averaged normal of the whole face cannot work here in principle — on a full cylinder the
/// normals around the circle cancel to zero and the centroid lands on the axis, so the sign comes out at random.
/// That is exactly how a shaft of 30 mm diameter was taken for a hole: the thread ran outwards from the shaft and
/// removed 2 cm³ instead of 12.
pub fn cyl_face_is_internal(mesh: &Mesh, tris: &[u32], axis_pt: [f64; 3], axis_dir: [f64; 3]) -> bool {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 {
        return false;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    let mut score = 0.0_f64;
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        let (n, area, c) = mesh.tri_normal_area(ti as usize);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - axis_pt[0], c.y - axis_pt[1], c.z - axis_pt[2]];
        let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
        let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t]; // From the axis to the triangle.
        let rl = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
        if rl < 1e-9 {
            continue; // A triangle on the axis itself says nothing about the radial direction.
        }
        let dot = (radial[0] * n[0] + radial[1] * n[1] + radial[2] * n[2]) / rl;
        score += dot * area;
    }
    score < 0.0 // Normals looking towards the axis mean the material is outside, so it is a hole.
}

/// Which way a thread runs from the selected rim: along the selected face, not towards whichever side holds more
/// vertices of the body. Returns the axis turned towards the side the cylindrical face itself lies on.
///
/// Measuring over the whole mesh does not work: with a chamfer on the end face the rim of the thread sits at the
/// base of that chamfer, and on a part whose bulk is above that rim (a boss on a plate, a flange) most vertices end
/// up on the chamfer side, so the thread ran into thin air towards the end face instead of into the material. The
/// face, by contrast, always lies where the thread belongs.
pub fn axis_along_face(mesh: &Mesh, tris: &[u32], rim: [f64; 3], axis: [f64; 3]) -> [f64; 3] {
    let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if al < 1e-12 {
        return axis;
    }
    let ax = [axis[0] / al, axis[1] / al, axis[2] / al];
    let mut acc = 0.0;
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        let (_, area, c) = mesh.tri_normal_area(ti as usize);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - rim[0], c.y - rim[1], c.z - rim[2]];
        acc += (d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2]) * area;
    }
    if acc < 0.0 {
        [-ax[0], -ax[1], -ax[2]]
    } else {
        ax
    }
}

/// Which way a thread runs from a rim when no face is selected, which is all a timeline rebuild has, since it knows
/// only the edge: take the triangles lying on the cylinder of that radius and see which way they extend.
///
/// Measuring over the whole mesh does not work for the reason given on [`axis_along_face`]: with a chamfer on the
/// end face the thread ran into thin air. This gives the same answer, derived from the radius instead of the
/// face.
pub fn axis_along_cylinder(mesh: &Mesh, rim: [f64; 3], axis: [f64; 3], radius: f64) -> Option<[f64; 3]> {
    let al = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if al < 1e-12 || radius <= 1e-9 {
        return None;
    }
    let ax = [axis[0] / al, axis[1] / al, axis[2] / al];
    let band = (0.05 * radius).max(1e-3);
    let (mut acc, mut hits) = (0.0, 0usize);
    for ti in 0..mesh.tris.len() {
        let (_, area, c) = mesh.tri_normal_area(ti);
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        let d = [c.x - rim[0], c.y - rim[1], c.z - rim[2]];
        let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
        let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
        let r = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
        if (r - radius).abs() > band {
            continue;
        }
        acc += t * area;
        hits += 1;
    }
    if hits < 3 || acc.abs() < 1e-9 {
        return None;
    }
    Some(if acc < 0.0 { [-ax[0], -ax[1], -ax[2]] } else { ax })
}

/// The radius of a cylindrical face from its triangles: the mean radius about the axis and the spread (max − min as
/// a fraction of the mean). On a true cylinder the spread is zero; on a chamfer or a cone the radius varies along
/// the axis, which is how the two are told apart. A 1 mm chamfer on a 20 mm diameter gives a spread of about 10%.
///
/// This is needed when choosing the target of a thread: with a chamfer on the end face, the edges of a cylindrical
/// face include rims of different radii, and the edge nearest the click gives the wrong radius, so the thread is
/// built on the wrong surface. Checking against the radius of the face itself picks the right rim.
pub fn cyl_face_radius(mesh: &Mesh, tris: &[u32], axis_pt: [f64; 3], axis_dir: [f64; 3]) -> Option<(f64, f64)> {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 {
        return None;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    // Measured over vertices rather than triangle centres: a centre averages the radii of its vertices and smooths
    // the difference away — a cone from 12 to 8 showed a spread of only 6%, and a small chamfer vanished entirely.
    let (mut sum, mut n) = (0.0, 0usize);
    let (mut rmin, mut rmax) = (f64::MAX, 0.0_f64);
    for &ti in tris {
        if ti as usize >= mesh.tris.len() {
            continue;
        }
        for v in mesh.triangle(ti as usize) {
            let d = [v.x - axis_pt[0], v.y - axis_pt[1], v.z - axis_pt[2]];
            let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
            let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
            let r = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt();
            if !r.is_finite() {
                continue;
            }
            sum += r;
            n += 1;
            rmin = rmin.min(r);
            rmax = rmax.max(r);
        }
    }
    if n < 3 {
        return None;
    }
    let mean = sum / n as f64;
    if mean <= 1e-9 {
        return None;
    }
    Some((mean, (rmax - rmin) / mean))
}

/// The same question with no face selected: only the axis, the radius and a span along the axis are known, which is
/// how a thread is specified from a circular edge. Take the mesh triangles lying on the cylinder of radius `r`
/// within that span and ask them for the same side. `None` means there are no such triangles and nothing to judge
/// by, in which case the explicit choice stands.
pub fn cyl_side_from_mesh(mesh: &Mesh, axis_pt: [f64; 3], axis_dir: [f64; 3], r: f64, z0: f64, z1: f64) -> Option<bool> {
    let al = (axis_dir[0] * axis_dir[0] + axis_dir[1] * axis_dir[1] + axis_dir[2] * axis_dir[2]).sqrt();
    if al < 1e-12 || r <= 1e-9 {
        return None;
    }
    let ax = [axis_dir[0] / al, axis_dir[1] / al, axis_dir[2] / al];
    let (lo, hi) = (z0.min(z1), z0.max(z1));
    let band = (0.05 * r).max(1e-3); // The tolerance for "lies on the cylinder".
    let on_cyl: Vec<u32> = (0..mesh.tris.len() as u32)
        .filter(|&ti| {
            let (_, area, c) = mesh.tri_normal_area(ti as usize);
            if !area.is_finite() || area <= 0.0 {
                return false;
            }
            let d = [c.x - axis_pt[0], c.y - axis_pt[1], c.z - axis_pt[2]];
            let t = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
            if t < lo - band || t > hi + band {
                return false;
            }
            let radial = [d[0] - ax[0] * t, d[1] - ax[1] * t, d[2] - ax[2] * t];
            ((radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt() - r).abs() <= band
        })
        .collect();
    if on_cyl.len() < 3 {
        return None;
    }
    Some(cyl_face_is_internal(mesh, &on_cyl, axis_pt, ax))
}

/// A section cap computed from the mesh: a closed slice of a body by a plane, with no B-rep involved.
///
/// A section that leaves bodies hollow inside is not how a section is expected to read; a plane must draw a closed
/// body. The cap used to be computed as a kernel boolean (the common part of a face and a body), which made it
/// unavailable wherever no live B-rep exists: an imported STL, a body loaded from a bundle before the B-rep is
/// built, and over a thousand bodies a boolean is too expensive anyway. A mesh is always present, so the cap is
/// computed from it.
///
/// The triangles crossed by the plane give the segments of the slice; the segments are stitched into loops
/// (`stitch_segments`), the loops are sorted into outer ones and holes by nesting and triangulated
/// (`triangulate_with_holes`). Returns the triangles of the cap in world coordinates.
pub fn mesh_section_cap(mesh: &Mesh, origin: [f64; 3], normal: [f64; 3]) -> Vec<[Point3; 3]> {
    let nl = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if nl < 1e-12 || mesh.tris.is_empty() {
        return Vec::new();
    }
    let n = [normal[0] / nl, normal[1] / nl, normal[2] / nl];
    // The basis of the plane.
    let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let cross = |p: [f64; 3], q: [f64; 3]| [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]];
    let norm = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let u = norm(cross(a, n));
    let v = cross(n, u);
    let dist = |p: &Point3| (p.x - origin[0]) * n[0] + (p.y - origin[1]) * n[1] + (p.z - origin[2]) * n[2];
    let to2 = |p: Point3| {
        let d = [p.x - origin[0], p.y - origin[1], p.z - origin[2]];
        Point2::new(d[0] * u[0] + d[1] * u[1] + d[2] * u[2], d[0] * v[0] + d[1] * v[1] + d[2] * v[2])
    };
    // The stitching tolerance follows the scale of the scene: a small part and a metre-sized frame must not share
    // one constant.
    let tol = mesh.bounds().map(|b| ((b.max.x - b.min.x).abs() + (b.max.y - b.min.y).abs() + (b.max.z - b.min.z).abs()) * 1e-6).unwrap_or(1e-6).max(1e-9);

    let mut segs: Vec<(Point2, Point2)> = Vec::new();
    for ti in 0..mesh.tris.len() {
        let t = mesh.triangle(ti);
        let d = [dist(&t[0]), dist(&t[1]), dist(&t[2])];
        let (pos, neg, zero) = (d.iter().filter(|x| **x > tol).count(), d.iter().filter(|x| **x < -tol).count(), d.iter().filter(|x| x.abs() <= tol).count());
        if pos > 0 && neg > 0 {
            // A genuine intersection: points on the edges that change sign, plus vertices exactly on the plane.
            let mut pts: Vec<Point2> = Vec::new();
            for k in 0..3 {
                let (p0, p1) = (t[k], t[(k + 1) % 3]);
                let (d0, d1) = (d[k], d[(k + 1) % 3]);
                if d0.abs() <= tol {
                    pts.push(to2(p0));
                }
                if (d0 > tol && d1 < -tol) || (d0 < -tol && d1 > tol) {
                    let s = d0 / (d0 - d1);
                    pts.push(to2(Point3::new(p0.x + (p1.x - p0.x) * s, p0.y + (p1.y - p0.y) * s, p0.z + (p1.z - p0.z) * s)));
                }
            }
            pts.dedup_by(|a, b| a.dist(*b) <= tol);
            if pts.len() >= 2 && pts[0].dist(pts[1]) > tol {
                segs.push((pts[0], pts[1]));
            }
        } else if zero == 2 && (pos + neg) == 1 {
            // An edge lying in the plane, which is what a section taken exactly along a datum produces: no
            // triangle "crosses" the plane at all, and without this branch a piece of the contour was lost, the
            // loop did not close and there was no cap whatsoever. Two triangles share such an edge, so the
            // duplicates are removed below.
            let k = (0..3).find(|&k| d[k].abs() <= tol && d[(k + 1) % 3].abs() <= tol).unwrap_or(0);
            let (a, b) = (to2(t[k]), to2(t[(k + 1) % 3]));
            if a.dist(b) > tol {
                segs.push((a, b));
            }
        }
    }
    // Remove duplicates: an edge in the plane arrives from both adjacent triangles, and stitching would branch.
    segs.dedup_by(|a, b| (a.0.dist(b.0) <= tol && a.1.dist(b.1) <= tol) || (a.0.dist(b.1) <= tol && a.1.dist(b.0) <= tol));
    {
        let mut uniq: Vec<(Point2, Point2)> = Vec::with_capacity(segs.len());
        for s in segs.drain(..) {
            if !uniq.iter().any(|u| (u.0.dist(s.0) <= tol && u.1.dist(s.1) <= tol) || (u.0.dist(s.1) <= tol && u.1.dist(s.0) <= tol)) {
                uniq.push(s);
            }
        }
        segs = uniq;
    }
    if segs.is_empty() {
        return Vec::new();
    }
    let loops: Vec<Contour> = stitch_segments(segs, tol.max(1e-7) * 10.0).into_iter().filter(|c| c.closed && c.points.len() >= 3 && c.area() > 0.0).collect();
    if loops.is_empty() {
        return Vec::new();
    }
    // Nesting: a loop is a hole when it lies inside an odd number of others, by the parity rule.
    //
    // The probe point is deliberately not the centroid: on a concave, C-shaped loop the centroid lies outside the
    // loop itself, so nesting came out wrong and parts of the cap were filled incorrectly. The probe is a point
    // known to be inside the loop: the midpoint of a diagonal running inwards from a vertex, verified by testing it
    // against the loop. If none is found, which means a degenerate loop, it falls back to the centroid.
    let probe = |c: &Contour| -> Point2 {
        let n = c.points.len();
        for i in 0..n {
            let (prev, cur, next) = (c.points[(i + n - 1) % n], c.points[i], c.points[(i + 1) % n]);
            // A point just inside the vertex, along the bisector of the adjacent edges.
            let mid = Point2::new((prev.x + next.x) * 0.5, (prev.y + next.y) * 0.5);
            let p = Point2::new(cur.x + (mid.x - cur.x) * 0.5, cur.y + (mid.y - cur.y) * 0.5);
            if c.contains(p) {
                return p;
            }
        }
        c.centroid()
    };
    let probes: Vec<Point2> = loops.iter().map(&probe).collect();
    let inside = |ip: Point2, ia: f64, outer: &Contour| outer.contains(ip) && outer.area() > ia;
    let depth: Vec<usize> = loops
        .iter()
        .enumerate()
        .map(|(i, c)| loops.iter().enumerate().filter(|(j, o)| *j != i && inside(probes[i], c.area(), o)).count())
        .collect();
    let mut tris: Vec<[Point3; 3]> = Vec::new();
    for (i, c) in loops.iter().enumerate() {
        if depth[i] % 2 != 0 {
            continue; // A hole; the material around it comes from its parent loop.
        }
        // The direct children of this loop are its holes.
        let holes: Vec<Vec<Point2>> = loops
            .iter()
            .enumerate()
            .filter(|(j, h)| depth[*j] == depth[i] + 1 && inside(probes[*j], h.area(), c))
            .map(|(_, h)| h.points.clone())
            .collect();
        for t in triangulate_with_holes(&c.points, &holes) {
            let lift = |p: Point2| {
                Point3::new(
                    origin[0] + u[0] * p.x + v[0] * p.y,
                    origin[1] + u[1] * p.x + v[1] * p.y,
                    origin[2] + u[2] * p.x + v[2] * p.y,
                )
            };
            tris.push([lift(t[0]), lift(t[1]), lift(t[2])]);
        }
    }
    tris
}

/// Triangulate a polygon with holes by ear clipping with bridges to the holes.
///
/// Each hole is cut into the outer contour by a bridge from its rightmost point to a visible vertex of the contour,
/// the classical technique that leaves one simple polygon and ordinary ear clipping. Degenerate input (a loop of
/// fewer than three points, or zero area) yields nothing rather than garbage.
pub fn triangulate_with_holes(outer: &[Point2], holes: &[Vec<Point2>]) -> Vec<[Point2; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let area2 = |p: &[Point2]| -> f64 {
        let n = p.len();
        (0..n).map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y).sum::<f64>()
    };
    // The outer contour runs counter-clockwise and the holes clockwise, so a bridge does not turn the polygon
    // inside out.
    let mut poly: Vec<Point2> = outer.to_vec();
    if area2(&poly) < 0.0 {
        poly.reverse();
    }
    let mut hs: Vec<Vec<Point2>> = holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|h| {
            let mut h = h.clone();
            if area2(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .collect();
    // Holes sorted by the x of their rightmost point, descending, so they are cut in from right to left.
    hs.sort_by(|a, b| {
        let mx = |h: &Vec<Point2>| h.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        mx(b).partial_cmp(&mx(a)).unwrap_or(std::cmp::Ordering::Equal)
    });
    for h in hs {
        let (hi, _) = h.iter().enumerate().fold((0usize, f64::MIN), |(bi, bx), (i, p)| if p.x > bx { (i, p.x) } else { (bi, bx) });
        // The nearest vertex of the outer contour to the right of the cut-in point, a simple and robust bridge.
        let m = h[hi];
        let Some(oi) = (0..poly.len()).filter(|&i| poly[i].x >= m.x - 1e-9).min_by(|&i, &j| poly[i].dist(m).partial_cmp(&poly[j].dist(m)).unwrap_or(std::cmp::Ordering::Equal)).or_else(|| {
            (0..poly.len()).min_by(|&i, &j| poly[i].dist(m).partial_cmp(&poly[j].dist(m)).unwrap_or(std::cmp::Ordering::Equal))
        }) else {
            continue;
        };
        let mut merged: Vec<Point2> = Vec::with_capacity(poly.len() + h.len() + 2);
        merged.extend_from_slice(&poly[..=oi]);
        for k in 0..h.len() {
            merged.push(h[(hi + k) % h.len()]);
        }
        merged.push(m);
        merged.push(poly[oi]);
        merged.extend_from_slice(&poly[oi + 1..]);
        poly = merged;
    }
    // ── ear clipping ─────────────────────────────────────────────────────────────────────────────
    // The tolerance is relative. An absolute one (1e-12) worked on nothing real: the coordinates of a part run to
    // hundreds of millimetres, cross products have the dimension of area, and on the shallow flanks of a thread
    // almost any vertex counted as "inside the ear". No ear was found, the loop exited through `None => break` and
    // silently returned a stub: the section of a threaded part was filled only across its smooth portion — of 424
    // triangles in the cap, 5 fell in the thread.
    let cross = |o: Point2, a: Point2, b: Point2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let (mut lo, mut hi) = (poly[0], poly[0]);
    for p in &poly {
        lo = Point2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let scale = (hi.x - lo.x).hypot(hi.y - lo.y).max(1e-9);
    let eps = 1e-10 * scale * scale; // A threshold on area, not on length.
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    let mut out: Vec<[Point2; 3]> = Vec::new();
    let mut guard = poly.len() * poly.len() + 16;
    while idx.len() > 2 && guard > 0 {
        guard -= 1;
        let n = idx.len();
        let mut cut = None;
        let mut best_convex: Option<(usize, f64)> = None; // The fallback when no ear is found.
        for k in 0..n {
            let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            let conv = cross(a, b, c);
            if conv <= eps {
                continue; // Not a convex corner, or a degenerate one.
            }
            if best_convex.map(|(_, v)| conv > v).unwrap_or(true) {
                best_convex = Some((k, conv));
            }
            // An ear is a convex corner with no reflex vertex inside it. Testing only the reflex vertices is the
            // classical formulation and the only correct one: a convex vertex may legitimately lie on an edge of
            // the ear — after holes are bridged in, such coincidences always occur — and an inclusive test counted
            // that as "inside", so no ear was found. The containment test is therefore strict: a point on an edge
            // does not block the ear.
            //
            // The earlier variant, inclusive and over all vertices, produced overlapping triangles: on one section
            // the cap covered 103.5% of the area of the contour. An area larger than the figure itself can only
            // come from overlaps, and on screen it reads as a torn fill.
            let inside_ear = (0..n).any(|jk| {
                let j = idx[jk];
                if j == ia || j == ib || j == ic {
                    return false;
                }
                // Is vertex j reflex within the current contour?
                let (pj0, pj1, pj2) = (poly[idx[(jk + n - 1) % n]], poly[j], poly[idx[(jk + 1) % n]]);
                if cross(pj0, pj1, pj2) >= -eps {
                    return false; // Convex or nearly straight, so it cannot block the ear.
                }
                // A reflex vertex blocks the ear even when it lies on its boundary: otherwise the ear covers the
                // notch and the area of the cap exceeds the figure itself (725 instead of 700 on a C-shaped
                // profile).
                let p = poly[j];
                cross(a, b, p) >= -eps && cross(b, c, p) >= -eps && cross(c, a, p) >= -eps
            });
            if !inside_ear {
                out.push([a, b, c]);
                cut = Some(k);
                break;
            }
        }
        match cut.or_else(|| best_convex.map(|(k, _)| k)) {
            // The fallback: with no ear found, clip the most convex corner and carry on. One questionable
            // triangle at a degenerate spot is far better than losing the fill of a whole region, which is what
            // the loop used to do by exiting here.
            Some(k) => {
                if cut.is_none() {
                    let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
                    out.push([poly[ia], poly[ib], poly[ic]]);
                }
                idx.remove(k);
            }
            None => break, // No convex corners are left at all: the contour has degenerated.
        }
    }
    out
}

#[cfg(test)]
mod normal_tests {
    use super::*;

    // A flat quad of two triangles with shared vertices: the vertex normal equals the face normal, +Z.
    #[test]
    fn vertex_normals_flat_quad_shares_face_normal() {
        let m = Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            tris: vec![[0, 1, 2], [0, 2, 3]],
        };
        for n in m.vertex_normals() {
            assert!((n[0]).abs() < 1e-9 && (n[1]).abs() < 1e-9 && (n[2] - 1.0).abs() < 1e-9, "a flat quad must give +Z, got {n:?}");
        }
    }

    // A roof: two faces of equal area meeting at 90 degrees share an edge (vertices 1 and 2). The shared vertices
    // are averaged into the bisector with equal weights, while the outer ones keep the normal of their own face and
    // stay sharp. A smooth surface is thus smoothed, while a break in the topology — separate vertices — keeps the
    // edge sharp.
    #[test]
    fn vertex_normals_shared_edge_averages_bisector() {
        // Face A lies in the z plane (normal +Z), face B is vertical (normal +X), and they share the edge x = 1.
        let m = Mesh {
            verts: vec![
                Point3::new(0.0, 0.0, 0.0), // 0, on A only.
                Point3::new(1.0, 0.0, 0.0), // 1, shared.
                Point3::new(1.0, 1.0, 0.0), // 2, shared.
                Point3::new(1.0, 0.0, 1.0), // 3, on B only.
                Point3::new(1.0, 1.0, 1.0), // 4, on B only.
            ],
            tris: vec![[0, 1, 2], [1, 2, 4], [1, 4, 3]],
        };
        let n = m.vertex_normals();
        // Vertex 0 belongs to face A only, so it is +Z.
        assert!((n[0][2] - 1.0).abs() < 1e-9, "vertex0 = +Z, got {:?}", n[0]);
        // Vertex 3 belongs to face B only, so it is +X.
        assert!((n[3][0] - 1.0).abs() < 1e-9, "vertex3 = +X, got {:?}", n[3]);
        // Shared vertex 1 mixes +Z and +X: components x and z positive, y near zero, and unit length.
        let v = n[1];
        assert!(v[0] > 0.1 && v[2] > 0.1 && v[1].abs() < 1e-9, "vertex 1 must be averaged, got {v:?}");
        assert!(((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() - 1.0).abs() < 1e-9, "the normal must be unit length");
    }
    /// The section cap from a mesh: a closed body on the cutting plane with no B-rep involved, so that a section
    /// does not leave bodies hollow inside.
    mod section_cap_tests {
        use super::super::{mesh_section_cap, triangulate_with_holes, Mesh, Point2, Point3};

        /// A box [x0..x1] x [y0..y1] x [z0..z1] as 12 triangles; the normals do not matter for a section.
        fn box_mesh(x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> Mesh {
            let v = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
            let verts = vec![
                v(x0, y0, z0), v(x1, y0, z0), v(x1, y1, z0), v(x0, y1, z0),
                v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1),
            ];
            let tris = vec![
                [0, 2, 1], [0, 3, 2], // Bottom.
                [4, 5, 6], [4, 6, 7], // Top.
                [0, 1, 5], [0, 5, 4], // y0
                [1, 2, 6], [1, 6, 5], // x1
                [2, 3, 7], [2, 7, 6], // y1
                [3, 0, 4], [3, 4, 7], // x0
            ];
            Mesh { verts, tris }
        }

        fn cap_area(tris: &[[Point3; 3]], n: [f64; 3]) -> f64 {
            tris.iter()
                .map(|t| {
                    let (a, b, c) = (t[0], t[1], t[2]);
                    let u = [b.x - a.x, b.y - a.y, b.z - a.z];
                    let w = [c.x - a.x, c.y - a.y, c.z - a.z];
                    let cr = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                    ((cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2]).abs()) * 0.5
                })
                .sum()
        }

        /// NaN coordinates in a mesh, which a malformed STL routinely carries, brought the application down on
        /// `partial_cmp(..).unwrap()` while sorting faces. The expected result is an honest one without degenerate
        /// faces and, above all, without a panic.
        #[test]
        fn detect_faces_survives_nan_mesh() {
            let mut m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let good = m.detect_faces(8.0).len();
            assert!(good > 0, "faces must be found on a sound cube");
            // Corrupt one vertex, as a broken import would.
            m.verts[0] = Point3::new(f64::NAN, f64::NAN, f64::NAN);
            let faces = m.detect_faces(8.0);
            assert!(faces.iter().all(|f| f.area.is_finite() && f.normal.iter().all(|v| v.is_finite())), "the result must contain no NaN faces");
            assert!(faces.len() < good + 1, "some faces are dropped and the rest are still computed: {} of {good}", faces.len());
            // A mesh entirely of NaN must not bring anything down either.
            for v in &mut m.verts {
                *v = Point3::new(f64::NAN, 0.0, 0.0);
            }
            let _ = m.detect_faces(8.0);
        }

        #[test]
        fn cap_of_cube_is_full_cross_section() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let tris = mesh_section_cap(&m, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
            assert!(!tris.is_empty(), "the cap must be built");
            let a = cap_area(&tris, [0.0, 0.0, 1.0]);
            assert!((a - 100.0).abs() < 1e-6, "the area of the cap is the 10x10 cross section of the cube: {a}");
            for t in &tris {
                for p in t {
                    assert!((p.z - 5.0).abs() < 1e-9, "the cap must lie exactly in the plane: z={}", p.z);
                }
            }
        }

        #[test]
        fn cap_has_hole_for_hollow_body() {
            // A tube: an outer box of 20x20 and an inner cavity of 10x10, a separate shell within the same mesh.
            let mut m = box_mesh(0.0, 20.0, 0.0, 20.0, 0.0, 20.0);
            let inner = box_mesh(5.0, 15.0, 5.0, 15.0, -1.0, 21.0);
            let off = m.verts.len() as u32;
            m.verts.extend(inner.verts);
            m.tris.extend(inner.tris.into_iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
            let tris = mesh_section_cap(&m, [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]);
            let a = cap_area(&tris, [0.0, 0.0, 1.0]);
            assert!((a - 300.0).abs() < 1e-6, "the cap of a hollow body is 400 - 100: {a}, so the hole is not filled");
        }

        /// The centroid of a concave, C-shaped loop lies outside the loop itself, so classifying nesting by the
        /// centroid gave the wrong answer and parts of the cap were filled incorrectly. Checked on a C-shaped
        /// profile: the area of the cap must be the area of the C itself, not of its convex hull.
        #[test]
        fn cap_of_concave_c_shaped_body() {
            // A C-shaped profile in the XZ plane, extruded along Y: 30x30 overall with a 20x10 notch on the right.
            //  ┌──────┐
            //  │  ┌───┘   (the notch opens to the right)
            //  │  └───┐
            //  └──────┘
            let prof: [(f64, f64); 8] = [(0.0, 0.0), (30.0, 0.0), (30.0, 10.0), (10.0, 10.0), (10.0, 20.0), (30.0, 20.0), (30.0, 30.0), (0.0, 30.0)];
            let mut m = Mesh::default();
            // Two caps (y = 0 and y = 10) plus the side walls; only the walls matter for a section, but the body
            // is assembled in full.
            let n = prof.len();
            for y in [0.0, 10.0] {
                for p in prof {
                    m.verts.push(Point3::new(p.0, y, p.1));
                }
            }
            for i in 0..n {
                let j = (i + 1) % n;
                let (a0, b0, a1, b1) = (i as u32, j as u32, (i + n) as u32, (j + n) as u32);
                m.tris.push([a0, b0, b1]);
                m.tris.push([a0, b1, a1]);
            }
            // A section by the plane y = 5, across the extrusion, so the cross section is the C profile itself.
            let tris = mesh_section_cap(&m, [0.0, 5.0, 0.0], [0.0, 1.0, 0.0]);
            let area = cap_area(&tris, [0.0, 1.0, 0.0]);
            let exp = 30.0 * 30.0 - 20.0 * 10.0; // 900 - 200 = 700, with the notch left unfilled.
            assert!((area - exp).abs() < 1e-6, "the cap of a concave profile: {area}, expected {exp}");
        }

        #[test]
        fn plane_beside_body_gives_no_cap() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            assert!(mesh_section_cap(&m, [0.0, 0.0, 50.0], [0.0, 0.0, 1.0]).is_empty(), "a plane clear of the body gives no cap");
        }

        #[test]
        fn cap_works_on_tilted_plane() {
            let m = box_mesh(0.0, 10.0, 0.0, 10.0, 0.0, 10.0);
            let n = [0.0, 1.0f64 / 2f64.sqrt(), 1.0 / 2f64.sqrt()]; // At 45 degrees to Z.
            let tris = mesh_section_cap(&m, [5.0, 5.0, 5.0], n);
            let a = cap_area(&tris, n);
            let exp = 10.0 * 10.0 * 2f64.sqrt(); // A rectangle of 10 by 10 * sqrt(2).
            assert!((a - exp).abs() / exp < 1e-6, "a tilted section: {a}, expected {exp}");
        }

        #[test]
        fn triangulation_of_square_with_square_hole() {
            let outer = vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)];
            let hole = vec![Point2::new(4.0, 4.0), Point2::new(6.0, 4.0), Point2::new(6.0, 6.0), Point2::new(4.0, 6.0)];
            let tris = triangulate_with_holes(&outer, &[hole]);
            let area: f64 = tris
                .iter()
                .map(|t| ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x)).abs() * 0.5)
                .sum();
            assert!((area - 96.0).abs() < 1e-6, "the triangulated area is 100 - 4: {area}");
        }

        #[test]
        fn degenerate_input_is_empty_not_garbage() {
            assert!(triangulate_with_holes(&[Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)], &[]).is_empty(), "two points are not a polygon");
            assert!(mesh_section_cap(&Mesh::default(), [0.0; 3], [0.0, 0.0, 1.0]).is_empty(), "an empty mesh");
            assert!(mesh_section_cap(&box_mesh(0.0, 1.0, 0.0, 1.0, 0.0, 1.0), [0.0; 3], [0.0; 3]).is_empty(), "a zero normal");
        }
    }

}
