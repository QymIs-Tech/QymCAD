//! The subdivision cage: a coarse polyhedron turned into a smooth surface by Catmull-Clark.
//!
//! This is half of the design layer: the points of a coarse cage are what gets dragged, and a smooth shape is
//! what is seen. A cage is forgiving, and that is the whole difference from editing NURBS control points
//! directly, where smoothness across patch seams has to be maintained by hand.
//!
//! Pure mathematics, with no kernel involved: no B-rep here, only vertices and faces. The thread tables are
//! built the same way — whatever can be computed without the kernel is computed without it and covered by tests
//! in full. The kernel enters later, when the limit surface has to be handed over as NURBS.
//!
//! # The Catmull-Clark rules
//!
//! One subdivision step turns every n-gon into n quadrilaterals:
//!
//! * the **face point** is the average of its vertices;
//! * the **edge point** is the average of the two endpoints and the two points of the adjacent faces; at a
//!   border it is simply the midpoint;
//! * the **new vertex position** is `(F + 2R + (n−3)P) / n`, where `F` is the average of the adjacent face
//!   points, `R` the average of the adjacent edge midpoints, and `n` the valence.
//!
//! The border rules differ, and that is not a detail: without them an open cage — a piece of surface rather
//! than a closed volume — would pull towards its centre on every step. A boundary vertex is computed as a
//! curve; see [`Cage::subdivide`].
//!
//! # The limit point
//!
//! Subdivision can be repeated indefinitely, and the limit is known in closed form and computed directly
//! ([`Cage::limit_points`]). This matters for correctness rather than for speed: the nodes of a cage have to lie
//! exactly on the surface, or snapping to the base geometry — a cage point landing on a face — is off by an
//! amount that no number of steps reduces.

/// A cage: vertices and polygonal faces, with vertex indices given counter-clockwise.
///
/// Triangles and n-gons are allowed: the first subdivision step turns everything into quadrilaterals, and from
/// then on the cage is purely quadrilateral.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cage {
    pub verts: Vec<[f64; 3]>,
    pub faces: Vec<Vec<u32>>,
}

/// An edge as a pair of vertices in normalised order, used as a key when looking up neighbours.
fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn mid(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    scale(add(a, b), 0.5)
}

/// The connectivity of a cage, computed once: what borders what.
struct Topo {
    /// edge to the faces it belongs to
    edge_faces: std::collections::HashMap<(u32, u32), Vec<usize>>,
    /// vertex to its adjacent edges
    vert_edges: Vec<Vec<(u32, u32)>>,
    /// vertex to its adjacent faces
    vert_faces: Vec<Vec<usize>>,
}

impl Cage {
    /// A cube of side `s` about the origin, the most common starting cage.
    pub fn cube(s: f64) -> Cage {
        let h = s / 2.0;
        let verts = vec![
            [-h, -h, -h],
            [h, -h, -h],
            [h, h, -h],
            [-h, h, -h],
            [-h, -h, h],
            [h, -h, h],
            [h, h, h],
            [-h, h, h],
        ];
        let faces = vec![
            vec![0, 3, 2, 1], // bottom
            vec![4, 5, 6, 7], // top
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ];
        Cage { verts, faces }
    }

    /// A flat `nx`×`ny` grid of quadrilaterals measuring `w`×`h` in the XY plane: an open cage with a border,
    /// which is what the boundary rules are checked on.
    pub fn grid(nx: usize, ny: usize, w: f64, h: f64) -> Cage {
        let (nx, ny) = (nx.max(1), ny.max(1));
        let mut verts = Vec::with_capacity((nx + 1) * (ny + 1));
        for j in 0..=ny {
            for i in 0..=nx {
                verts.push([w * (i as f64 / nx as f64 - 0.5), h * (j as f64 / ny as f64 - 0.5), 0.0]);
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut faces = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                faces.push(vec![idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        Cage { verts, faces }
    }

    /// An `nu`×`nv` torus: a cage without extraordinary vertices, being closed with every vertex of valence
    /// four.
    ///
    /// It exists not as a shape but as a reference for checking the conversion into patches: on a torus every
    /// face has an exact patch and the shell has to close without a single hole. A cube cannot serve that
    /// purpose — it has eight extraordinary corners, and holes around them appear by construction.
    pub fn torus(nu: usize, nv: usize, r_major: f64, r_minor: f64) -> Cage {
        let (nu, nv) = (nu.max(3), nv.max(3));
        let mut verts = Vec::with_capacity(nu * nv);
        for i in 0..nu {
            let a = std::f64::consts::TAU * i as f64 / nu as f64;
            for j in 0..nv {
                let b = std::f64::consts::TAU * j as f64 / nv as f64;
                let r = r_major + r_minor * b.cos();
                verts.push([r * a.cos(), r * a.sin(), r_minor * b.sin()]);
            }
        }
        let idx = |i: usize, j: usize| ((i % nu) * nv + (j % nv)) as u32;
        let mut faces = Vec::with_capacity(nu * nv);
        for i in 0..nu {
            for j in 0..nv {
                faces.push(vec![idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        Cage { verts, faces }
    }

    /// Whether the cage is closed, meaning every edge has exactly two faces. An open one is a piece of
    /// surface with a border.
    pub fn is_closed(&self) -> bool {
        self.topo().edge_faces.values().all(|f| f.len() == 2)
    }

    /// The valence of a vertex: the number of edges adjacent to it.
    pub fn valence(&self, v: usize) -> usize {
        self.topo().vert_edges.get(v).map(|e| e.len()).unwrap_or(0)
    }

    fn topo(&self) -> Topo {
        let mut edge_faces: std::collections::HashMap<(u32, u32), Vec<usize>> = std::collections::HashMap::new();
        let mut vert_edges: Vec<Vec<(u32, u32)>> = vec![Vec::new(); self.verts.len()];
        let mut vert_faces: Vec<Vec<usize>> = vec![Vec::new(); self.verts.len()];
        for (fi, f) in self.faces.iter().enumerate() {
            for (k, &a) in f.iter().enumerate() {
                let b = f[(k + 1) % f.len()];
                let key = edge_key(a, b);
                let e = edge_faces.entry(key).or_default();
                if !e.contains(&fi) {
                    e.push(fi);
                }
                for v in [a, b] {
                    if let Some(list) = vert_edges.get_mut(v as usize) {
                        if !list.contains(&key) {
                            list.push(key);
                        }
                    }
                }
                if let Some(list) = vert_faces.get_mut(a as usize) {
                    if !list.contains(&fi) {
                        list.push(fi);
                    }
                }
            }
        }
        Topo { edge_faces, vert_edges, vert_faces }
    }

    /// The centre of a face.
    fn face_point(&self, f: &[u32]) -> [f64; 3] {
        let s = f.iter().fold([0.0; 3], |acc, &v| add(acc, self.verts[v as usize]));
        scale(s, 1.0 / f.len() as f64)
    }

    /// One subdivision step.
    ///
    /// Every n-gon yields n quadrilaterals: face centre, edge point, new vertex, point of the adjacent edge.
    pub fn subdivide(&self) -> Cage {
        let t = self.topo();
        let mut verts: Vec<[f64; 3]> = Vec::with_capacity(self.verts.len() * 4);

        // 1) new positions of the original vertices
        for (vi, &p) in self.verts.iter().enumerate() {
            let edges = &t.vert_edges[vi];
            let faces = &t.vert_faces[vi];
            let boundary: Vec<(u32, u32)> = edges.iter().copied().filter(|e| t.edge_faces[e].len() == 1).collect();
            if !boundary.is_empty() {
                // A boundary vertex is computed as a curve rather than as a surface: the border of a cage has
                // to behave like a cubic spline along itself, or an open cage would pull inwards on every step
                // and the patch would shrink on its own.
                if boundary.len() == 2 {
                    let other = |e: (u32, u32)| if e.0 as usize == vi { e.1 } else { e.0 };
                    let (a, b) = (self.verts[other(boundary[0]) as usize], self.verts[other(boundary[1]) as usize]);
                    verts.push(scale(add(add(a, b), scale(p, 6.0)), 1.0 / 8.0));
                } else {
                    verts.push(p); // a corner of the border, with one or more than two boundary edges, stays put
                }
                continue;
            }
            let n = edges.len() as f64;
            if n < 3.0 {
                verts.push(p);
                continue;
            }
            let f_avg = scale(faces.iter().fold([0.0; 3], |acc, &fi| add(acc, self.face_point(&self.faces[fi]))), 1.0 / faces.len() as f64);
            let r_avg = scale(
                edges.iter().fold([0.0; 3], |acc, &(a, b)| add(acc, mid(self.verts[a as usize], self.verts[b as usize]))),
                1.0 / n,
            );
            // (F + 2R + (n−3)P) / n
            verts.push(scale(add(add(f_avg, scale(r_avg, 2.0)), scale(p, n - 3.0)), 1.0 / n));
        }

        // 2) face points
        let mut face_idx: Vec<u32> = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            face_idx.push(verts.len() as u32);
            verts.push(self.face_point(f));
        }

        // 3) edge points
        let mut edge_idx: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for (&(a, b), fs) in &t.edge_faces {
            let m = mid(self.verts[a as usize], self.verts[b as usize]);
            let p = if fs.len() == 2 {
                let fp = add(self.face_point(&self.faces[fs[0]]), self.face_point(&self.faces[fs[1]]));
                scale(add(scale(m, 2.0), fp), 0.25)
            } else {
                m // at a border the midpoint is used, or the edge drifts inwards
            };
            edge_idx.insert((a, b), verts.len() as u32);
            verts.push(p);
        }

        // 4) new faces
        let mut faces = Vec::with_capacity(self.faces.iter().map(|f| f.len()).sum());
        for (fi, f) in self.faces.iter().enumerate() {
            let c = face_idx[fi];
            for (k, &v) in f.iter().enumerate() {
                let prev = f[(k + f.len() - 1) % f.len()];
                let next = f[(k + 1) % f.len()];
                let e_prev = edge_idx[&edge_key(prev, v)];
                let e_next = edge_idx[&edge_key(v, next)];
                faces.push(vec![c, e_prev, v, e_next]);
            }
        }
        Cage { verts, faces }
    }

    /// `n` steps in a row.
    pub fn subdivided(&self, n: usize) -> Cage {
        let mut c = self.clone();
        for _ in 0..n {
            c = c.subdivide();
        }
        c
    }

    /// The limit positions of the vertices: where a point ends up after infinitely many steps.
    ///
    /// The Halstead mask: `(n²·V + 4·ΣE + ΣF) / (n·(n+5))`, where `E` are the neighbours across edges, `F` the
    /// opposite corners of the adjacent quadrilaterals, and `n` the valence.
    ///
    /// Two corrections, both found by measurement rather than by reasoning.
    ///
    /// The first: the mask takes the neighbouring vertices and the opposite corners, not the edge midpoints and
    /// the face centres. An earlier version took the latter and gave 0.75·h for a cube corner instead of the
    /// correct 0.5·h — a discrepancy of 2.17 mm on a cage of side 10, caught by comparison against seven
    /// subdivision steps.
    ///
    /// The second: the subdivision step before the mask is needed not because the mask only works afterwards —
    /// the limit does not depend on subdivision at all, measured at a discrepancy of 9e-16. It is needed because
    /// an opposite corner is defined only for a quadrilateral, while a cage may be drawn in any shape. One step
    /// makes it entirely quadrilateral and gives the mask something to measure. On a cage already made of
    /// quadrilaterals the step does not change the answer at all.
    ///
    /// None of this is for speed. A cage node snapped to a face of the base has to lie on the surface exactly
    /// rather than nearly: otherwise the anchor is off by an amount that no number of steps reduces.
    pub fn limit_points(&self) -> Vec<[f64; 3]> {
        let s = self.subdivide(); // the original vertices keep their indices, which is what this rests on
        let t = s.topo();
        let mut out = Vec::with_capacity(self.verts.len());
        for vi in 0..self.verts.len() {
            let p = s.verts[vi];
            let edges = &t.vert_edges[vi];
            let boundary: Vec<(u32, u32)> = edges.iter().copied().filter(|e| t.edge_faces[e].len() == 1).collect();
            if !boundary.is_empty() {
                // at a border the limit is that of a cubic curve: (A + 4P + B) / 6
                if boundary.len() == 2 {
                    let other = |e: (u32, u32)| if e.0 as usize == vi { e.1 } else { e.0 };
                    let (a, b) = (s.verts[other(boundary[0]) as usize], s.verts[other(boundary[1]) as usize]);
                    out.push(scale(add(add(a, b), scale(p, 4.0)), 1.0 / 6.0));
                } else {
                    out.push(p); // a corner of the border stays put
                }
                continue;
            }
            let n = edges.len() as f64;
            if n < 3.0 {
                out.push(p);
                continue;
            }
            let other = |e: &(u32, u32)| if e.0 as usize == vi { e.1 } else { e.0 };
            let e_sum = edges.iter().fold([0.0; 3], |acc, e| add(acc, s.verts[other(e) as usize]));
            // the opposite corner of a quadrilateral; after the step every face is one
            let f_sum = t.vert_faces[vi].iter().fold([0.0; 3], |acc, &fi| {
                let f = &s.faces[fi];
                let k = f.iter().position(|&x| x as usize == vi).unwrap_or(0);
                add(acc, s.verts[f[(k + 2) % f.len()] as usize])
            });
            let w = n * (n + 5.0);
            out.push(scale(add(add(scale(p, n * n), scale(e_sum, 4.0)), f_sum), 1.0 / w));
        }
        out
    }

    /// The bounding box of a cage, used by checks and to fit the camera.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in &self.verts {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        (lo, hi)
    }

    /// The volume of a closed cage, by the divergence theorem with the faces fanned from their first vertex.
    ///
    /// It is meaningless for an open cage, but does not fail either: it computes as if the holes were closed.
    pub fn volume(&self) -> f64 {
        let mut v = 0.0;
        for f in &self.faces {
            for k in 1..f.len().saturating_sub(1) {
                let (a, b, c) = (self.verts[f[0] as usize], self.verts[f[k] as usize], self.verts[f[k + 1] as usize]);
                v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        v.abs()
    }
}

pub mod patch;

#[cfg(test)]
mod tests;
