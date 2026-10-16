//! A MESH: its triangles, faces and edges.

use super::*;

/// A triangle mesh, used for STL and STEP output and for 3D machining.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub verts: Vec<Point3>,
    pub tris: Vec<[u32; 3]>,
    /// THE SURFACE'S OWN NORMAL AT EVERY VERTEX, from the kernel (`add_face`), facing out of the body; empty for a mesh
    /// with no surface behind it (a file of triangles), and (0, 0, 0) at a vertex whose face gave none. The smooth light
    /// is shaded by it (`vertex_normals`): an average of the triangles around a vertex stood up to 6.4 deg off a
    /// sphere's own normal and broke the light along its seam and where a rounding meets the face it is tangent to.
    /// Packed by `pack_direction`, four bytes a vertex: the reference engine holds 23 million vertices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub norms: Vec<u32>,
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
            v += (t[0].x * (t[1].y * t[2].z - t[2].y * t[1].z) - t[0].y * (t[1].x * t[2].z - t[2].x * t[1].z) + t[0].z * (t[1].x * t[2].y - t[2].x * t[1].y)) / 6.0;
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
        self.turn_normals(|n| crate::feature::apply12_dir(m, n));
    }

    /// The surface's normals turned as the vertices were, and made unit again.
    pub(crate) fn turn_normals(&mut self, turn: impl Fn([f64; 3]) -> [f64; 3]) {
        for n in &mut self.norms {
            if let Some(d) = unpack_direction(*n) {
                *n = pack_direction(turn(d));
            }
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
        self.turn_normals(|n| [n[0] * co - n[1] * s, n[0] * s + n[1] * co, n[2]]);
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
        let ctr = Point3::new((t[0].x + t[1].x + t[2].x) / 3.0, (t[0].y + t[1].y + t[2].y) / 3.0, (t[0].z + t[1].z + t[2].z) / 3.0);
        (n, len * 0.5, ctr)
    }

    /// Smoothed normals at the corners of the triangles, `3 * t + k` for corner `k` of triangle `t`, smoothed only
    /// where the surface turns by less than `crease_deg` between two triangles. For a mesh that shares its vertices
    /// across its sharp edges too - a mesh from a file - where `vertex_normals` smooths the light over every edge: a
    /// corner takes the area-weighted normals of the triangles around its vertex that turn from its own by less than
    /// the crease, so a fillet stays round and an edge stays sharp.
    pub fn corner_normals(&self, crease_deg: f64) -> Vec<[f64; 3]> {
        let raw: Vec<[f64; 3]> = (0..self.tris.len())
            .map(|i| {
                let t = self.triangle(i);
                let u = [t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z];
                let v = [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z];
                [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
            })
            .collect();
        let len = |a: [f64; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let unit: Vec<[f64; 3]> = raw.iter().map(|&a| if len(a) > 0.0 { a.map(|c| c / len(a)) } else { [0.0; 3] }).collect();
        // the triangles around every vertex, as ranges of one list
        let mut start = vec![0usize; self.verts.len() + 1];
        for t in &self.tris {
            for &v in t {
                start[v as usize + 1] += 1;
            }
        }
        for i in 0..self.verts.len() {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut around = vec![0u32; start[self.verts.len()]];
        for (i, t) in self.tris.iter().enumerate() {
            for &v in t {
                around[fill[v as usize]] = i as u32;
                fill[v as usize] += 1;
            }
        }
        let cos = crease_deg.to_radians().cos();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let mut out = Vec::with_capacity(self.tris.len() * 3);
        for (i, t) in self.tris.iter().enumerate() {
            for &v in t {
                let mut a = [0.0; 3];
                for &s in &around[start[v as usize]..start[v as usize + 1]] {
                    let s = s as usize;
                    if s == i || dot(unit[s], unit[i]) >= cos {
                        (0..3).for_each(|k| a[k] += raw[s][k]);
                    }
                }
                out.push(if len(a) > 0.0 { a.map(|c| c / len(a)) } else { unit[i] });
            }
        }
        out
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
        let averaged = self.averaged_normals();
        if self.norms.len() != self.verts.len() {
            return averaged;
        }
        // THE SURFACE'S OWN NORMAL where the kernel gave one, the average of the triangles where it gave none
        self.norms.iter().zip(averaged).map(|(&n, a)| unpack_direction(n).unwrap_or(a)).collect()
    }

    /// The area-weighted average of the triangle normals around every vertex - for a mesh with no surface behind it.
    pub(crate) fn averaged_normals(&self) -> Vec<[f64; 3]> {
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

impl MeshFace {
    /// The face of a mesh moved by the 3x4 matrix `m` as its mesh is (`Mesh::transform`): its centroid placed, its normal
    /// turned; a move or a turn keeps its area.
    pub fn transformed(&self, m: &[f64; 12]) -> MeshFace {
        let c = crate::feature::apply12(m, [self.centroid.x, self.centroid.y, self.centroid.z]);
        MeshFace { centroid: Point3::new(c[0], c[1], c[2]), normal: crate::feature::apply12_dir(m, self.normal), ..self.clone() }
    }

    /// The face of a mesh scaled by `f` about the origin as its mesh is: its centroid with it, its area as the square.
    pub fn scaled(&self, f: f64) -> MeshFace {
        MeshFace { centroid: Point3::new(self.centroid.x * f, self.centroid.y * f, self.centroid.z * f), area: self.area * f * f, ..self.clone() }
    }
}

/// WHAT THE KERNEL GIVES BACK FOR ONE NODE: the triangles of the body and the faces they are grouped into, each
/// face carrying its persistent id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    pub mesh: Mesh,
    pub faces: Vec<MeshFace>,
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
    /// The names of the two ends, at `a` and at `b`: the vertex of the topology each end stands on, named by the edges
    /// that meet at it. Given by the bridge with the edge; a vertex has nothing of its own but its place and its edges.
    #[serde(default)]
    pub ends: [crate::names::VertexName; 2],
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
