//! OpenSubdiv-compatible Catmull-Clark subdivision with semi-sharp edge and vertex creases.
//!
//! Standard Catmull-Clark smooths all edges unconditionally, turning a cube into a sphere-like shape.
//! Industrial surface design requires controlling sharpness:
//!
//! * **Sharpness 0.0**: completely smooth Catmull-Clark surface;
//! * **Sharpness >= levels**: infinitely sharp crease (hard mechanical feature edge);
//! * **Fractional sharpness (0 < s < levels)**: semi-sharp crease with controlled roundness,
//!   evaluated via the Chaikin/Hoppe/DeRose crease decay rule.
//!
//! At each subdivision step, edge sharpness decrements by 1.0: `s_{k+1} = max(0, s_k - 1)`. When sharpness
//! is between 0 and 1, the new vertex position is a linear blend between the sharp rule and the smooth rule.

use super::Cage;
use std::collections::HashMap;

/// An unordered pair of vertex indices representing an edge in a subdivision cage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubdivEdge {
    /// Lower vertex index.
    pub v0: u32,
    /// Higher vertex index.
    pub v1: u32,
}

impl SubdivEdge {
    /// Creates a normalized edge key from an endpoint pair.
    pub fn of(edge: [u32; 2]) -> Self {
        let (a, b) = (edge[0], edge[1]);
        if a < b {
            Self { v0: a, v1: b }
        } else {
            Self { v0: b, v1: a }
        }
    }
}

/// Edge crease specification: an edge key and a sharpness weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeCrease {
    /// Vertex indices of the edge.
    pub edge: SubdivEdge,
    /// Sharpness value (0.0 = smooth, 1.0..10.0 = semi-sharp, >= 10.0 = infinitely sharp).
    pub sharpness: f64,
}

impl EdgeCrease {
    /// Creates a new edge crease.
    pub fn new(edge: SubdivEdge, sharpness: f64) -> Self {
        Self { edge, sharpness: sharpness.max(0.0) }
    }
}

/// A sharp incident edge tracked during vertex position evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
struct IncidentSharpEdge {
    edge: SubdivEdge,
    sharpness: f64,
}

/// Point in 3D Euclidean space used for subdivision math without neighbouring duplicate arguments.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Pt3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Pt3 {
    fn from_array(p: [f64; 3]) -> Self {
        Self { x: p[0], y: p[1], z: p[2] }
    }

    fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    fn add(self, other: Pt3) -> Pt3 {
        Pt3 { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }

    fn scale(self, k: f64) -> Pt3 {
        Pt3 { x: self.x * k, y: self.y * k, z: self.z * k }
    }

    fn mid(self, other: Pt3) -> Pt3 {
        self.add(other).scale(0.5)
    }

    fn blend(self, sharp: Pt3, s: f64) -> Pt3 {
        let t = s.clamp(0.0, 1.0);
        self.scale(1.0 - t).add(sharp.scale(t))
    }
}

/// A subdivision cage augmented with edge and vertex crease sharpness maps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenSubdivCage {
    /// Base geometric cage (vertices and faces).
    pub cage: Cage,
    /// Sharpness weight per normalized edge.
    pub edge_creases: HashMap<SubdivEdge, f64>,
    /// Sharpness weight per vertex index.
    pub vert_creases: HashMap<u32, f64>,
}

impl OpenSubdivCage {
    /// Wraps a base cage with empty crease maps.
    pub fn new(cage: Cage) -> Self {
        Self { cage, edge_creases: HashMap::new(), vert_creases: HashMap::new() }
    }

    /// Sets the sharpness of an edge.
    pub fn set_edge_crease(&mut self, edge: SubdivEdge, sharpness: f64) {
        if sharpness > 1e-6 {
            self.edge_creases.insert(edge, sharpness);
        } else {
            self.edge_creases.remove(&edge);
        }
    }

    /// Sets the sharpness of all edges on a given face.
    pub fn set_face_edges_crease(&mut self, face_index: usize, sharpness: f64) {
        if let Some(face) = self.cage.faces.get(face_index).cloned() {
            for k in 0..face.len() {
                let a = face[k];
                let b = face[(k + 1) % face.len()];
                self.set_edge_crease(SubdivEdge::of([a, b]), sharpness);
            }
        }
    }

    /// Sets sharpness on all edges of the cage (creates an infinitely sharp or bevelled polyhedron).
    pub fn set_all_edges_crease(&mut self, sharpness: f64) {
        let faces = self.cage.faces.clone();
        for f in &faces {
            for k in 0..f.len() {
                let a = f[k];
                let b = f[(k + 1) % f.len()];
                self.set_edge_crease(SubdivEdge::of([a, b]), sharpness);
            }
        }
    }

    /// Performs one OpenSubdiv Catmull-Clark subdivision step with crease evaluation.
    pub fn subdivide(&self) -> OpenSubdivCage {
        let t = self.cage.topo();
        let mut new_verts = Vec::with_capacity(self.cage.verts.len() * 4);
        let mut next_edge_creases = HashMap::new();
        let mut next_vert_creases = HashMap::new();

        // 1) Vertex points
        for (vi, &p_raw) in self.cage.verts.iter().enumerate() {
            let p = Pt3::from_array(p_raw);
            let edges = &t.vert_edges[vi];
            let faces = &t.vert_faces[vi];

            // Count sharp incident edges
            let mut sharp_edges: Vec<IncidentSharpEdge> = Vec::new();
            for &e_raw in edges {
                let e = SubdivEdge::of([e_raw.0, e_raw.1]);
                if let Some(&s) = self.edge_creases.get(&e) {
                    if s > 1e-4 {
                        sharp_edges.push(IncidentSharpEdge { edge: e, sharpness: s });
                    }
                }
            }

            let vert_sharpness = self.vert_creases.get(&(vi as u32)).copied().unwrap_or(0.0);
            let boundary: Vec<SubdivEdge> = edges.iter().copied().filter(|e| t.edge_faces[e].len() == 1).map(|e| SubdivEdge::of([e.0, e.1])).collect();

            let other = |e: SubdivEdge| if e.v0 as usize == vi { e.v1 } else { e.v0 };

            // Base smooth Catmull-Clark vertex
            let p_smooth = if !boundary.is_empty() {
                if boundary.len() == 2 {
                    let a = Pt3::from_array(self.cage.verts[other(boundary[0]) as usize]);
                    let b = Pt3::from_array(self.cage.verts[other(boundary[1]) as usize]);
                    a.add(b).add(p.scale(6.0)).scale(1.0 / 8.0)
                } else {
                    p
                }
            } else {
                let n = edges.len() as f64;
                if n < 3.0 {
                    p
                } else {
                    let f_sum = faces.iter().fold(Pt3::default(), |acc, &fi| acc.add(Pt3::from_array(self.cage.face_point(&self.cage.faces[fi]))));
                    let f_avg = f_sum.scale(1.0 / faces.len() as f64);
                    let r_sum = edges.iter().fold(Pt3::default(), |acc, &(a, b)| {
                        let va = Pt3::from_array(self.cage.verts[a as usize]);
                        let vb = Pt3::from_array(self.cage.verts[b as usize]);
                        acc.add(va.mid(vb))
                    });
                    let r_avg = r_sum.scale(1.0 / n);
                    f_avg.add(r_avg.scale(2.0)).add(p.scale(n - 3.0)).scale(1.0 / n)
                }
            };

            // Crease rules:
            // k = 0 or 1: smooth / dart
            // k = 2: crease rule (cubic B-spline along crease)
            // k >= 3 or corner: fixed corner point
            let (p_final, updated_vert_s) = if vert_sharpness > 0.0 || sharp_edges.len() >= 3 {
                // Corner vertex: remains fixed
                (p, (vert_sharpness - 1.0).max(0.0))
            } else if sharp_edges.len() == 2 {
                // Crease vertex: average along the two creased edges
                let a = Pt3::from_array(self.cage.verts[other(sharp_edges[0].edge) as usize]);
                let b = Pt3::from_array(self.cage.verts[other(sharp_edges[1].edge) as usize]);
                let p_crease = a.add(b).add(p.scale(6.0)).scale(1.0 / 8.0);
                let avg_s = (sharp_edges[0].sharpness + sharp_edges[1].sharpness) * 0.5;
                (p_smooth.blend(p_crease, avg_s), 0.0)
            } else {
                // Dart or smooth vertex
                (p_smooth, 0.0)
            };

            if updated_vert_s > 1e-4 {
                next_vert_creases.insert(vi as u32, updated_vert_s);
            }
            new_verts.push(p_final.to_array());
        }

        // 2) Face points
        let mut face_idx = Vec::with_capacity(self.cage.faces.len());
        for f in &self.cage.faces {
            face_idx.push(new_verts.len() as u32);
            new_verts.push(self.cage.face_point(f));
        }

        // 3) Edge points
        let mut edge_idx = HashMap::new();
        for (&(a, b), fs) in &t.edge_faces {
            let va = Pt3::from_array(self.cage.verts[a as usize]);
            let vb = Pt3::from_array(self.cage.verts[b as usize]);
            let m = va.mid(vb);
            let edge = SubdivEdge::of([a, b]);
            let s = self.edge_creases.get(&edge).copied().unwrap_or(0.0);

            let p_smooth = if fs.len() == 2 {
                let fp0 = Pt3::from_array(self.cage.face_point(&self.cage.faces[fs[0]]));
                let fp1 = Pt3::from_array(self.cage.face_point(&self.cage.faces[fs[1]]));
                let fp = fp0.add(fp1);
                m.scale(2.0).add(fp).scale(0.25)
            } else {
                m
            };

            // Sharp edge rule: exact edge midpoint
            let p_edge = if s > 0.0 { p_smooth.blend(m, s) } else { p_smooth };

            let e_center_id = new_verts.len() as u32;
            edge_idx.insert(edge, e_center_id);
            new_verts.push(p_edge.to_array());

            // Decrement edge sharpness for child segments
            let child_s = (s - 1.0).max(0.0);
            if child_s > 1e-4 {
                next_edge_creases.insert(SubdivEdge::of([a, e_center_id]), child_s);
                next_edge_creases.insert(SubdivEdge::of([e_center_id, b]), child_s);
            }
        }

        // 4) New quad faces
        let mut new_faces = Vec::with_capacity(self.cage.faces.iter().map(|f| f.len()).sum());
        for (fi, f) in self.cage.faces.iter().enumerate() {
            let c = face_idx[fi];
            for (k, &v) in f.iter().enumerate() {
                let prev = f[(k + f.len() - 1) % f.len()];
                let next = f[(k + 1) % f.len()];
                let e_prev = edge_idx[&SubdivEdge::of([prev, v])];
                let e_next = edge_idx[&SubdivEdge::of([v, next])];
                new_faces.push(vec![c, e_prev, v, e_next]);
            }
        }

        OpenSubdivCage { cage: Cage { verts: new_verts, faces: new_faces }, edge_creases: next_edge_creases, vert_creases: next_vert_creases }
    }

    /// Subdivides the cage `levels` times.
    pub fn subdivided(&self, levels: usize) -> OpenSubdivCage {
        let mut cur = self.clone();
        for _ in 0..levels {
            cur = cur.subdivide();
        }
        cur
    }

    /// Converts all quadrilateral and polygon faces into flat triangle indices for rendering or export.
    pub fn to_triangles(&self) -> Vec<[u32; 3]> {
        let mut tris = Vec::new();
        for f in &self.cage.faces {
            if f.len() >= 3 {
                for k in 1..f.len().saturating_sub(1) {
                    tris.push([f[0], f[k], f[k + 1]]);
                }
            }
        }
        tris
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncreased_cube_matches_smooth_catmull_clark() {
        let cube = Cage::cube(10.0);
        let sub_cage = OpenSubdivCage::new(cube.clone());

        let refined_sub = sub_cage.subdivided(2);
        let refined_pure = cube.subdivided(2);

        assert_eq!(refined_sub.cage.faces.len(), refined_pure.faces.len());
        assert_eq!(refined_sub.cage.verts.len(), refined_pure.verts.len());

        let vol_diff = (refined_sub.cage.volume() - refined_pure.volume()).abs();
        assert!(vol_diff < 1e-6, "volume difference: {vol_diff}");

        let (lo_sub, hi_sub) = refined_sub.cage.bounds();
        let (lo_pure, hi_pure) = refined_pure.bounds();
        for k in 0..3 {
            assert!((lo_sub[k] - lo_pure[k]).abs() < 1e-6);
            assert!((hi_sub[k] - hi_pure[k]).abs() < 1e-6);
        }
    }

    #[test]
    fn infinitely_sharp_cube_preserves_corners_and_bounds() {
        let cube = Cage::cube(10.0);
        let mut sub_cage = OpenSubdivCage::new(cube);
        // Set sharpness to 10.0 (infinitely sharp for 2 subdivision steps)
        sub_cage.set_all_edges_crease(10.0);

        let refined = sub_cage.subdivided(2);
        let (lo, hi) = refined.cage.bounds();

        for k in 0..3 {
            assert!((lo[k] - (-5.0)).abs() < 1e-9, "min bound along {k} was corrupted: {}", lo[k]);
            assert!((hi[k] - 5.0).abs() < 1e-9, "max bound along {k} was corrupted: {}", hi[k]);
        }
    }

    #[test]
    fn semi_sharp_crease_interpolates_between_sharp_and_smooth() {
        let cube = Cage::cube(10.0);

        let smooth = OpenSubdivCage::new(cube.clone()).subdivided(1);

        let mut semi = OpenSubdivCage::new(cube.clone());
        semi.set_all_edges_crease(0.5);
        let semi_res = semi.subdivided(1);

        let mut sharp = OpenSubdivCage::new(cube);
        sharp.set_all_edges_crease(5.0);
        let sharp_res = sharp.subdivided(1);

        // Semi-sharp vertex positions must strictly lie between smooth and sharp positions
        let p_smooth = smooth.cage.verts[0];
        let p_semi = semi_res.cage.verts[0];
        let p_sharp = sharp_res.cage.verts[0];

        for k in 0..3 {
            let min_val = p_smooth[k].min(p_sharp[k]);
            let max_val = p_smooth[k].max(p_sharp[k]);
            assert!(p_semi[k] >= min_val - 1e-5 && p_semi[k] <= max_val + 1e-5);
        }
    }
}
