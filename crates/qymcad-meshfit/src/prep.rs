//! Preparing a mesh for recognition. Meshes come from scanners, print programs and exporters torn: a corner
//! repeated for every triangle that touches it, slivers with no area, windings mixed, holes. Nothing downstream
//! works on that - a region grows across shared sides, and a side that is not known to be shared stops it. So:
//! weld the corners, drop what has no area, turn every triangle to one winding with its normal out of the solid,
//! and find the triangle across each side and the loops the border runs in.

use qymcad_core::geom::{Mesh, Point3};
use std::collections::{HashMap, HashSet};

/// Marks a side of a triangle with no single neighbour across it: a border, or an edge that more than two
/// triangles share.
pub const NO_NEIGHBOUR: u32 = u32::MAX;

/// A mesh ready for recognition.
pub struct Prepared {
    /// Welded corners and the triangles left, each wound so that its normal points out of the solid.
    pub mesh: Mesh,
    /// For triangle `t`, the triangle across side `k` - the side from corner `k` to corner `k + 1` - or
    /// `NO_NEIGHBOUR`.
    pub neighbours: Vec<[u32; 3]>,
    /// The unit normal of every triangle.
    pub normals: Vec<[f64; 3]>,
    /// Every border as a loop of corner indices, in the order the border runs.
    pub holes: Vec<Vec<u32>>,
    /// Triangles dropped: no area, two corners welded into one, a repeat of a triangle already there, or a corner
    /// that is not a finite number.
    pub dropped: usize,
    /// Edges that three or more triangles share. No neighbour is chosen across them: which two of the
    /// triangles belong together is a question the mesh does not answer.
    pub non_manifold: usize,
    /// Triangles whose winding was turned.
    pub flipped: usize,
    /// Triangles kept although flat, because they close a seam. Their normal means nothing.
    pub slivers: Vec<u32>,
    /// For every triangle kept, its index in the mesh as given: what is found here can be shown on the file.
    pub origin: Vec<u32>,
}

impl Prepared {
    /// The angle between the normals of triangle `t` and of its neighbour across side `k`, in degrees: 0 on a
    /// flat run, 90 across the edge of a cube. `None` where the side has no single neighbour.
    pub fn dihedral_deg(&self, t: usize, k: usize) -> Option<f64> {
        let u = *self.neighbours.get(t)?.get(k)?;
        if u == NO_NEIGHBOUR {
            return None;
        }
        let c = dot(self.normals[t], self.normals[u as usize]).clamp(-1.0, 1.0);
        Some(c.acos().to_degrees())
    }
}

/// A millionth of the diagonal of the bounding box: 1.7e-4 mm on a 100 mm cube. That is twenty times the step
/// of an f32 coordinate at 100 (7.6e-6), the precision STL and glTF store, and far below any feature a part has.
pub fn weld_tolerance(mesh: &Mesh) -> f64 {
    let Some(b) = mesh.bounds() else { return 1e-9 };
    let d = sub(b.max, b.min);
    (len(d) * 1e-6).max(1e-9)
}

/// Prepares `mesh`: corners closer than `weld` become one, repeated triangles and loose flat ones go, windings
/// are made consistent and outward, neighbours and borders are found.
pub fn prepare(mesh: &Mesh, weld: f64) -> Prepared {
    let (verts, remap) = weld_corners(&mesh.verts, weld);
    let mut dropped = 0;
    let mut seen = HashSet::new();
    let mut tris = Vec::with_capacity(mesh.tris.len());
    let mut is_flat = Vec::with_capacity(mesh.tris.len());
    let mut orig = Vec::with_capacity(mesh.tris.len());
    for (ti, t) in mesh.tris.iter().enumerate() {
        let at = |i: u32| remap.get(i as usize).copied();
        let (Some(a), Some(b), Some(c)) = (at(t[0]), at(t[1]), at(t[2])) else {
            dropped += 1;
            continue;
        };
        // A CORNER THAT IS NOT A NUMBER has no place to be: an ASCII STL may write `NaN`, and it parses. Kept, the
        // triangles round it were flat and closed, so they stayed slivers no surface could hold; the regions read the
        // owner of an unplaced sliver past their end ("index out of bounds: the len is 6 but the index is
        // 4294967295"), and past that the kernel, given the corner, never came back from building the face.
        let finite = |i: u32| verts.get(i as usize).is_some_and(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
        let mut key = [a, b, c];
        key.sort_unstable();
        if !(finite(a) && finite(b) && finite(c)) || a == b || b == c || a == c || !seen.insert(key) {
            dropped += 1;
            continue;
        }
        is_flat.push(flat(&verts, [a, b, c], weld));
        tris.push([a, b, c]);
        orig.push(ti as u32);
    }

    // A flat triangle goes only where it hangs loose: a side of it no other triangle shares, or shares with more
    // than one. Where each of its three sides has exactly one partner it closes a seam - a 3MF of printed gears
    // closes every T-junction so, and dropping those opened 37 holes in sound gears - so it stays, marked.
    let shared = sides_of(&tris);
    let loose = |t: &[u32; 3]| (0..3).any(|k| shared.get(&side_key(t[k], t[(k + 1) % 3])).is_none_or(|l| l.len() != 2));
    let mut woven = Vec::with_capacity(tris.len());
    let mut origin = Vec::with_capacity(tris.len());
    let mut slivers = Vec::new();
    for (k, (t, &f)) in tris.iter().zip(&is_flat).enumerate() {
        if f && loose(t) {
            dropped += 1;
            continue;
        }
        if f {
            slivers.push(woven.len() as u32);
        }
        woven.push(*t);
        origin.push(orig[k]);
    }
    // A FLAT TRIANGLE CLOSING A SEAM IS MENDED, NOT KEPT: the triangle across its longest side is split at its middle corner
    // and the flat one goes. Kept, it made no face in the kernel, which dropped it - a hole the length of that side: on
    // gears 14 and 15 of `cube_gears` four such, and the body a shell. One that cannot be mended so - its partner flat
    // too, its middle corner off the side - stays a sliver as before.
    let mut gone = vec![false; woven.len()];
    for &s in &slivers {
        let s = s as usize;
        let t = woven[s];
        let at = |i: u32| verts[i as usize];
        let long = (0..3).max_by(|&i, &j| len(sub(at(t[(i + 1) % 3]), at(t[i]))).total_cmp(&len(sub(at(t[(j + 1) % 3]), at(t[j]))))).unwrap_or(0);
        let (a, b, m) = (t[long], t[(long + 1) % 3], t[(long + 2) % 3]);
        let side = sub(at(b), at(a));
        let along = dot(sub(at(m), at(a)), side) / dot(side, side).max(1e-300);
        if !(0.0 < along && along < 1.0) {
            continue;
        }
        let sides = sides_of(&woven);
        let Some(on) = sides.get(&side_key(a, b)).filter(|l| l.len() == 2) else { continue };
        let Some(partner) = on.iter().map(|&(u, _)| u as usize).find(|&u| u != s && !gone[u]) else { continue };
        if slivers.contains(&(partner as u32)) {
            continue;
        }
        let q = woven[partner];
        let Some(k) = (0..3).find(|&k| q[k] != a && q[k] != b) else { continue };
        let (r, u, w) = (q[k], q[(k + 1) % 3], q[(k + 2) % 3]);
        woven[partner] = [r, u, m];
        woven.push([r, m, w]);
        origin.push(origin[partner]);
        gone.push(false);
        gone[s] = true;
    }
    let mended: Vec<u32> = (0..gone.len()).filter(|&k| gone[k]).map(|k| k as u32).collect();
    if !mended.is_empty() {
        let mut renumber = vec![u32::MAX; woven.len()];
        let (mut kept_tris, mut kept_origin) = (Vec::with_capacity(woven.len()), Vec::with_capacity(woven.len()));
        for k in 0..woven.len() {
            if !gone[k] {
                renumber[k] = kept_tris.len() as u32;
                kept_tris.push(woven[k]);
                kept_origin.push(origin[k]);
            }
        }
        slivers = slivers.iter().filter(|&&s| !gone[s as usize]).map(|&s| renumber[s as usize]).collect();
        woven = kept_tris;
        origin = kept_origin;
    }
    let mut tris = woven;

    // Only the corners a triangle still uses stay, numbered in the order the triangles first reach them.
    let mut new_id = vec![NO_NEIGHBOUR; verts.len()];
    let mut kept = Vec::new();
    for t in &mut tris {
        for i in t.iter_mut() {
            let old = *i as usize;
            if new_id[old] == NO_NEIGHBOUR {
                new_id[old] = kept.len() as u32;
                kept.push(verts[old]);
            }
            *i = new_id[old];
        }
    }

    let turned = orient(&kept, &tris);
    let flipped = turned.iter().filter(|&&f| f).count();
    for (t, &f) in tris.iter_mut().zip(&turned) {
        *t = wound(*t, f);
    }
    let (neighbours, border, non_manifold) = adjacency(&tris);
    let normals = tris.iter().map(|t| unit(normal_of(&kept, *t))).collect();
    Prepared { mesh: Mesh { verts: kept, tris }, neighbours, normals, holes: loops(&border), dropped, non_manifold, flipped, slivers, origin }
}

/// Joins corners closer than `tol`. The space is cut into cubes of side `tol`, so a partner can only lie in the
/// same cube or one of the 26 around it; the first corner met in a place stands for all the others.
fn weld_corners(src: &[Point3], tol: f64) -> (Vec<Point3>, Vec<u32>) {
    let cell = tol.max(1e-12);
    let cell_of = |p: &Point3| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    let mut out: Vec<Point3> = Vec::new();
    let mut remap = Vec::with_capacity(src.len());
    for p in src {
        let (x, y, z) = cell_of(p);
        let mut found = None;
        'look: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &i in grid.get(&(x + dx, y + dy, z + dz)).map(Vec::as_slice).unwrap_or(&[]) {
                        if len(sub(out[i as usize], *p)) <= tol {
                            found = Some(i);
                            break 'look;
                        }
                    }
                }
            }
        }
        let id = found.unwrap_or_else(|| {
            let i = out.len() as u32;
            out.push(*p);
            grid.entry((x, y, z)).or_default().push(i);
            i
        });
        remap.push(id);
    }
    (out, remap)
}

/// A triangle whose height over its longest side is below the weld tolerance has no normal worth trusting. A
/// coordinate that is not a number makes the triangle flat too.
fn flat(v: &[Point3], t: [u32; 3], weld: f64) -> bool {
    let [a, b, c] = t.map(|i| v[i as usize]);
    let twice_area = len(cross(sub(b, a), sub(c, a)));
    let longest = len(sub(b, a)).max(len(sub(c, b))).max(len(sub(a, c)));
    // a NaN from a degenerate corner counts as a sliver too
    matches!(twice_area.partial_cmp(&(weld * longest)), None | Some(std::cmp::Ordering::Less))
}

/// The undirected sides of all triangles, each with the triangles on it and which of their sides it is.
fn sides_of(tris: &[[u32; 3]]) -> HashMap<(u32, u32), Vec<(u32, u8)>> {
    let mut sides: HashMap<(u32, u32), Vec<(u32, u8)>> = HashMap::with_capacity(tris.len() * 3 / 2);
    for (t, tri) in tris.iter().enumerate() {
        for k in 0..3 {
            sides.entry(side_key(tri[k], tri[(k + 1) % 3])).or_default().push((t as u32, k as u8));
        }
    }
    sides
}

/// Which triangles to turn. The winding spreads from a seed across every side two triangles share: a neighbour
/// wound the same way runs the shared side in the opposite direction. Then each connected piece is turned as a
/// whole if its signed volume is negative, that is, if its normals point in.
fn orient(verts: &[Point3], tris: &[[u32; 3]]) -> Vec<bool> {
    let sides = sides_of(tris);
    let mut turn: Vec<Option<bool>> = vec![None; tris.len()];
    for seed in 0..tris.len() {
        if turn[seed].is_some() {
            continue;
        }
        turn[seed] = Some(false);
        let mut piece = vec![seed];
        let mut stack = vec![seed];
        while let Some(t) = stack.pop() {
            let tt = wound(tris[t], turn[t] == Some(true));
            for k in 0..3 {
                let (a, b) = (tt[k], tt[(k + 1) % 3]);
                let Some(list) = sides.get(&side_key(a, b)) else { continue };
                if list.len() != 2 {
                    continue;
                }
                for &(u, _) in list {
                    let u = u as usize;
                    if u == t || turn[u].is_some() {
                        continue;
                    }
                    turn[u] = Some(runs(tris[u], a, b));
                    piece.push(u);
                    stack.push(u);
                }
            }
        }
        if signed_volume(verts, tris, &turn, &piece) < 0.0 {
            for &t in &piece {
                turn[t] = turn[t].map(|f| !f);
            }
        }
    }
    turn.into_iter().map(|f| f == Some(true)).collect()
}

/// The volume a piece encloses, signed by its winding, measured from the mean of its corners. For a piece with a
/// hole the number is not a volume, but its sign still says which way the normals face.
fn signed_volume(verts: &[Point3], tris: &[[u32; 3]], turn: &[Option<bool>], piece: &[usize]) -> f64 {
    let mut c = [0.0; 3];
    for &t in piece {
        for &i in &tris[t] {
            let p = verts[i as usize];
            c = [c[0] + p.x, c[1] + p.y, c[2] + p.z];
        }
    }
    let n = (piece.len() * 3) as f64;
    let c = Point3::new(c[0] / n, c[1] / n, c[2] / n);
    piece
        .iter()
        .map(|&t| {
            let [a, b, d] = wound(tris[t], turn[t] == Some(true)).map(|i| sub(verts[i as usize], c));
            dot(a, cross(b, d))
        })
        .sum::<f64>()
        / 6.0
}

/// Neighbours across every side, the border sides in the direction their triangle runs them, and the count of
/// sides shared by three or more triangles. The border is listed in triangle order so that the loops come out the
/// same on every run.
fn adjacency(tris: &[[u32; 3]]) -> (Vec<[u32; 3]>, Vec<(u32, u32)>, usize) {
    let sides = sides_of(tris);
    let mut neighbours = vec![[NO_NEIGHBOUR; 3]; tris.len()];
    let mut non_manifold = 0;
    for list in sides.values() {
        match list.as_slice() {
            [(t, k), (u, j)] => {
                neighbours[*t as usize][*k as usize] = *u;
                neighbours[*u as usize][*j as usize] = *t;
            }
            [_] => {}
            _ => non_manifold += 1,
        }
    }
    let mut border = Vec::new();
    for tri in tris {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if sides.get(&side_key(a, b)).is_some_and(|l| l.len() == 1) {
                border.push((a, b));
            }
        }
    }
    (neighbours, border, non_manifold)
}

/// Chains the border sides into loops. A chain that runs into a dead end - a border broken by an edge that three
/// triangles share - is returned as it stands.
fn loops(border: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut from: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, &(a, _)) in border.iter().enumerate() {
        from.entry(a).or_default().push(i);
    }
    let mut used = vec![false; border.len()];
    let mut out = Vec::new();
    for start in 0..border.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut at) = border[start];
        let mut ring = vec![first];
        while at != first {
            ring.push(at);
            let Some(i) = from.get(&at).and_then(|l| l.iter().copied().find(|&i| !used[i])) else { break };
            used[i] = true;
            at = border[i].1;
        }
        out.push(ring);
    }
    out
}

fn side_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Whether triangle `t` runs the side from `a` to `b` in that direction.
fn runs(t: [u32; 3], a: u32, b: u32) -> bool {
    (0..3).any(|k| t[k] == a && t[(k + 1) % 3] == b)
}

fn wound(t: [u32; 3], turned: bool) -> [u32; 3] {
    if turned {
        [t[0], t[2], t[1]]
    } else {
        t
    }
}

fn normal_of(v: &[Point3], t: [u32; 3]) -> [f64; 3] {
    let [a, b, c] = t.map(|i| v[i as usize]);
    cross(sub(b, a), sub(c, a))
}

fn sub(a: Point3, b: Point3) -> [f64; 3] {
    [a.x - b.x, a.y - b.y, a.z - b.z]
}

fn cross(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

fn dot(u: [f64; 3], v: [f64; 3]) -> f64 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

fn len(u: [f64; 3]) -> f64 {
    dot(u, u).sqrt()
}

fn unit(u: [f64; 3]) -> [f64; 3] {
    let l = len(u);
    if l > 0.0 {
        [u[0] / l, u[1] / l, u[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// HOW FAR A TRIANGLE OF THE MESH STANDS FROM THE SURFACE IT WAS CUT FROM - the chord deflection the mesh was made
/// with, read off the mesh itself. Two smooth neighbours whose normals turn by `theta` over the distance `c` between
/// their middles lie on a curve of radius about c/theta, and a chord across such a curve sags by about c*theta/8. The
/// tessellation of a program holds that sag under its deflection and reaches it where the curve is widest, so the
/// estimate is the sag the widest-curved twentieth of the mesh reaches. Edges sharper than `sharp_deg` are corners of the
/// part, not a sampled curve, and do not count. Zero for a mesh with no smooth curve at all. Measured against the true
/// sag of a ball and a cylinder of our kernel meshed at 0.1 to 0.003: 0.4 to 1.0 of it.
pub fn chord_deflection(p: &Prepared, sharp_deg: f64) -> f64 {
    let cos_sharp = sharp_deg.to_radians().cos();
    let centroid = |t: usize| {
        let [a, b, c] = p.mesh.tris[t];
        let (a, b, c) = (p.mesh.verts[a as usize], p.mesh.verts[b as usize], p.mesh.verts[c as usize]);
        [(a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0, (a.z + b.z + c.z) / 3.0]
    };
    let mut sags: Vec<f64> = Vec::new();
    for (t, around) in p.neighbours.iter().enumerate() {
        for &u in around {
            if u == NO_NEIGHBOUR || (u as usize) < t {
                continue;
            }
            let cos = dot(p.normals[t], p.normals[u as usize]).clamp(-1.0, 1.0);
            if cos < cos_sharp || cos > 1.0 - 1e-12 {
                continue; // a corner of the part, or two triangles on one plane
            }
            // the distance ACROSS the fold: from each middle to the line of the side the two share - long thin triangles
            // along a cylinder lie apart mostly along the fold, and that distance bends nothing
            let [a, b, _] = p.mesh.tris[t];
            let (ea, eb) = (p.mesh.verts[a as usize], p.mesh.verts[b as usize]);
            let shared = p.mesh.tris[u as usize].contains(&a) && p.mesh.tris[u as usize].contains(&b);
            let (ea, eb) = if shared {
                (ea, eb)
            } else {
                let [_, b2, c2] = p.mesh.tris[t];
                if p.mesh.tris[u as usize].contains(&b2) && p.mesh.tris[u as usize].contains(&c2) {
                    (p.mesh.verts[b2 as usize], p.mesh.verts[c2 as usize])
                } else {
                    (p.mesh.verts[c2 as usize], p.mesh.verts[a as usize])
                }
            };
            let along = sub(eb, ea);
            let l = len(along).max(1e-300);
            let off = |q: [f64; 3]| {
                let w = [q[0] - ea.x, q[1] - ea.y, q[2] - ea.z];
                let k = dot(w, along) / (l * l);
                len([w[0] - k * along[0], w[1] - k * along[1], w[2] - k * along[2]])
            };
            // a middle stands a third of its triangle's height off the side, so across the fold the two chords run 1.5
            // times further than the middles: sag = chord * theta / 8 (measured on cylinders: the middles gave 0.67 of
            // the true sag at every deflection)
            let c = off(centroid(t)) + off(centroid(u as usize));
            sags.push(1.5 * c * cos.acos() / 8.0);
        }
    }
    if sags.is_empty() {
        return 0.0;
    }
    let k = ((sags.len() as f64 * 0.95) as usize).min(sags.len() - 1);
    *sags.select_nth_unstable_by(k, f64::total_cmp).1
}
