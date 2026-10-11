//! A MESH MADE LIGHTER WITHIN A DEVIATION: edges collapsed one by one, cheapest first, where the corner left in their
//! place stays within `tol` of the planes of every triangle it stood on (quadric error metrics). A mesh exported from
//! a CAD is fine where its surface bends and just as fine where it is flat or bends slowly; a body made of it carries
//! every one of its triangles into every later operation. Measured on a 178 032-triangle mesh of a smooth handle: a
//! body of 144 000 faces, and a node that took minutes to rebuild after every change.

use crate::geom::{Mesh, Point3};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// A symmetric 4 x 4 quadric, its ten numbers: a a, a b, a c, a d, b b, b c, b d, c c, c d, d d; and the weight of the
/// planes summed in it.
#[derive(Clone, Copy, Default)]
struct Quadric([f64; 10], f64);

impl Quadric {
    fn of_plane(n: [f64; 3], d: f64, w: f64) -> Quadric {
        let [a, b, c] = n;
        Quadric([a * a * w, a * b * w, a * c * w, a * d * w, b * b * w, b * c * w, b * d * w, c * c * w, c * d * w, d * d * w], w)
    }

    fn add(&mut self, o: &Quadric) {
        for k in 0..10 {
            self.0[k] += o.0[k];
        }
        self.1 += o.1;
    }

    fn sum(a: &Quadric, b: &Quadric) -> Quadric {
        let mut q = *a;
        q.add(b);
        q
    }

    /// The mean squared distance of `p` from the planes the quadric holds, by their weights. The sum grows with every
    /// collapse however near the planes the point stays: measured on a ball of 57 120 triangles, the sum held it at
    /// 19 254 within 0.01 mm where the triangles' own sag allowed a tenth of that.
    fn error(&self, p: [f64; 3]) -> f64 {
        let q = &self.0;
        let [x, y, z] = p;
        (q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z + 2.0 * q[6] * y + q[7] * z * z + 2.0 * q[8] * z + q[9]) / self.1.max(1e-300)
    }

    /// The point of least error, where the 3 x 3 system is well posed.
    fn optimum(&self) -> Option<[f64; 3]> {
        let q = &self.0;
        let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
        let r = [-q[3], -q[6], -q[8]];
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        let scale = (m[0][0] + m[1][1] + m[2][2]).abs().max(1e-300);
        if det.abs() < 1e-9 * scale * scale * scale {
            return None;
        }
        let solve = |col: usize| {
            let mut a = m;
            for (row, v) in a.iter_mut().zip(r) {
                row[col] = v;
            }
            (a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0]) + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])) / det
        };
        Some([solve(0), solve(1), solve(2)])
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// A collapse waiting in the heap: cheapest first; stale once either corner has changed since it was reckoned.
struct Candidate {
    cost: f64,
    a: u32,
    b: u32,
    stamp: (u32, u32),
    at: [f64; 3],
}
impl PartialEq for Candidate {
    fn eq(&self, o: &Self) -> bool {
        self.cost == o.cost
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Candidate {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.total_cmp(&self.cost).then_with(|| (o.a, o.b).cmp(&(self.a, self.b)))
    }
}

/// How far a triangle next to a collapse may turn: past this it is taken for folding over, and the collapse is not made.
const MOST_TURN_COS: f64 = 0.2;

/// `mesh` made lighter: its corners welded where they coincide, then edges collapsed while the corner left stays within
/// `tol` of the planes of the triangles it replaces. A border and its corners stay where they are. `tol` of zero or less gives the mesh
/// back welded and whole.
pub fn simplify(mesh: &Mesh, tol: f64) -> Mesh {
    // WELDED within a millionth of the mesh's size: a mesh read from a triangle file carries every corner once per
    // triangle, and an exporter's corners on the two sides of a seam differ in their last digits - welded bit for bit, a
    // smooth handle kept 200 sides with no second triangle
    let (lo, hi) = mesh.verts.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), v| ([lo[0].min(v.x), lo[1].min(v.y), lo[2].min(v.z)], [hi[0].max(v.x), hi[1].max(v.y), hi[2].max(v.z)]));
    let weld = (norm(sub(hi, lo)) * 1e-6).max(1e-12);
    let cell = |v: &Point3| [(v.x / weld).floor() as i64, (v.y / weld).floor() as i64, (v.z / weld).floor() as i64];
    let mut grid: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    let mut pos: Vec<[f64; 3]> = Vec::new();
    let mut remap = vec![0u32; mesh.verts.len()];
    for (i, v) in mesh.verts.iter().enumerate() {
        let c = cell(v);
        let mut found = None;
        'look: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &k in grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if norm(sub(pos[k as usize], [v.x, v.y, v.z])) <= weld {
                            found = Some(k);
                            break 'look;
                        }
                    }
                }
            }
        }
        remap[i] = found.unwrap_or_else(|| {
            pos.push([v.x, v.y, v.z]);
            let k = (pos.len() - 1) as u32;
            grid.entry(c).or_default().push(k);
            k
        });
    }
    let mut tris: Vec<[u32; 3]> = mesh.tris.iter().map(|t| t.map(|v| remap[v as usize])).filter(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0]).collect();
    if tol <= 0.0 || tris.is_empty() {
        return compact(&pos, &tris);
    }
    let n = pos.len();
    let mut alive = vec![true; tris.len()];
    let mut around: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (t, tri) in tris.iter().enumerate() {
        for &v in tri {
            around[v as usize].push(t as u32);
        }
    }
    // THE QUADRICS: each corner the planes of its triangles, weighted by their areas
    let mut quad = vec![Quadric::default(); n];
    let mut sides: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in &tris {
        let (p0, p1, p2) = (pos[tri[0] as usize], pos[tri[1] as usize], pos[tri[2] as usize]);
        let nrm = cross(sub(p1, p0), sub(p2, p0));
        let len = norm(nrm);
        if len <= 1e-300 {
            continue;
        }
        let u = [nrm[0] / len, nrm[1] / len, nrm[2] / len];
        let q = Quadric::of_plane(u, -dot(u, p0), 0.5 * len);
        for &v in tri {
            quad[v as usize].add(&q);
        }
        for k in 0..3 {
            let (a, b) = (tri[k].min(tri[(k + 1) % 3]), tri[k].max(tri[(k + 1) % 3]));
            *sides.entry((a, b)).or_insert(0) += 1;
        }
    }
    // A CORNER ON A SIDE WITH NO SECOND TRIANGLE STAYS WHERE IT IS: a crack the exporter left is sewn by the kernel as it
    // stands, and its two banks collapsed apart it opened past sewing - measured on a smooth handle, 72 sides left
    // open and the body full of holes
    let mut fixed = vec![false; n];
    for (&(a, b), &count) in &sides {
        if count != 2 {
            fixed[a as usize] = true;
            fixed[b as usize] = true;
        }
    }
    let limit = tol * tol;
    let mut stamp = vec![0u32; n];
    let place = |q: &Quadric, a: [f64; 3], b: [f64; 3]| -> ([f64; 3], f64) {
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
        let mut best = (mid, q.error(mid));
        for p in q.optimum().into_iter().chain([a, b]) {
            let e = q.error(p);
            if e < best.1 {
                best = (p, e);
            }
        }
        best
    };
    let mut heap = BinaryHeap::new();
    for &(a, b) in sides.keys() {
        let q = Quadric::sum(&quad[a as usize], &quad[b as usize]);
        let (at, cost) = place(&q, pos[a as usize], pos[b as usize]);
        if cost <= limit {
            heap.push(Candidate { cost, a, b, stamp: (0, 0), at });
        }
    }
    let mut gone = vec![false; n];
    let mut drift = vec![0.0f64; n];
    while let Some(c) = heap.pop() {
        let (a, b) = (c.a as usize, c.b as usize);
        if gone[a] || gone[b] || fixed[a] || fixed[b] || (stamp[a], stamp[b]) != c.stamp {
            continue;
        }
        // THE LINK CONDITION: the corners the two share are the two across their edge (one on the border), or the
        // collapse pinches the surface
        let ring = |v: usize, alive: &[bool], tris: &[[u32; 3]], around: &[Vec<u32>]| -> Vec<u32> {
            let mut out: Vec<u32> = around[v].iter().filter(|&&t| alive[t as usize]).flat_map(|&t| tris[t as usize]).filter(|&w| w as usize != v).collect();
            out.sort_unstable();
            out.dedup();
            out
        };
        let (ra, rb) = (ring(a, &alive, &tris, &around), ring(b, &alive, &tris, &around));
        let shared = ra.iter().filter(|w| rb.binary_search(w).is_ok()).count();
        let both: Vec<u32> = around[a].iter().copied().filter(|&t| alive[t as usize] && tris[t as usize].contains(&(b as u32))).collect();
        if both.is_empty() || shared != both.len() {
            continue;
        }
        // NO TRIANGLE FOLDS OVER: every triangle that stays turns by less than the limit
        let folds = [a, b].iter().any(|&v| {
            around[v].iter().filter(|&&t| alive[t as usize] && !both.contains(&t)).any(|&t| {
                let tri = tris[t as usize];
                let p = tri.map(|w| pos[w as usize]);
                let moved = tri.map(|w| if w as usize == a || w as usize == b { c.at } else { pos[w as usize] });
                let (n0, n1) = (cross(sub(p[1], p[0]), sub(p[2], p[0])), cross(sub(moved[1], moved[0]), sub(moved[2], moved[0])));
                let (l0, l1) = (norm(n0), norm(n1));
                l1 <= 1e-300 || l0 <= 1e-300 || dot(n0, n1) < MOST_TURN_COS * l0 * l1
            })
        });
        if folds {
            continue;
        }
        // HOW FAR THE SURFACE HAS MOVED, bounded: what each corner carried already, and the farthest the new corner stands
        // from the planes of the triangles round the two. The mean the quadric keeps let one corner of a ball of 10 drift
        // 0.094 mm off it at a deviation of 0.01.
        let from_planes = [a, b]
            .iter()
            .flat_map(|&v| around[v].iter().copied())
            .filter(|&t| alive[t as usize])
            .map(|t| {
                let p = tris[t as usize].map(|w| pos[w as usize]);
                let nrm = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                let len = norm(nrm);
                if len <= 1e-300 {
                    0.0
                } else {
                    dot(nrm, sub(c.at, p[0])).abs() / len
                }
            })
            .fold(0.0, f64::max);
        let carried = drift[a].max(drift[b]) + from_planes;
        if carried > tol {
            continue;
        }
        // b goes into a
        for &t in &both {
            alive[t as usize] = false;
        }
        let from_b: Vec<u32> = around[b].iter().copied().filter(|&t| alive[t as usize]).collect();
        for &t in &from_b {
            for w in tris[t as usize].iter_mut() {
                if *w as usize == b {
                    *w = a as u32;
                }
            }
        }
        around[a].retain(|&t| alive[t as usize]);
        around[a].extend(from_b);
        around[b].clear();
        gone[b] = true;
        pos[a] = c.at;
        drift[a] = carried;
        let merged = Quadric::sum(&quad[a], &quad[b]);
        quad[a] = merged;
        stamp[a] += 1;
        // the edges round the corner left, reckoned again
        for w in ring(a, &alive, &tris, &around) {
            let w = w as usize;
            let q = Quadric::sum(&quad[a], &quad[w]);
            let (at, cost) = place(&q, pos[a], pos[w]);
            if cost <= limit {
                heap.push(Candidate { cost, a: a as u32, b: w as u32, stamp: (stamp[a], stamp[w]), at });
            }
        }
    }
    tris = tris.into_iter().zip(alive).filter(|(_, keep)| *keep).map(|(t, _)| t).collect();
    compact(&pos, &tris)
}

/// The mesh of `tris` over `pos`, keeping only the corners the triangles use.
fn compact(pos: &[[f64; 3]], tris: &[[u32; 3]]) -> Mesh {
    let mut new = vec![u32::MAX; pos.len()];
    let mut verts = Vec::new();
    let tris = tris
        .iter()
        .map(|t| {
            t.map(|v| {
                if new[v as usize] == u32::MAX {
                    new[v as usize] = verts.len() as u32;
                    let p = pos[v as usize];
                    verts.push(Point3::new(p[0], p[1], p[2]));
                }
                new[v as usize]
            })
        })
        .collect();
    Mesh { verts, tris, ..Default::default() }
}
