//! A FACE THAT DID NOT TESSELLATE LEAVES A HOLE IN THE BODY.
//!
//! Reported with a screenshot: a gap in an imported part where the shell should be - you look through the
//! surface into the inside. Measured on the reporter's document: 399 bodies out of 1296 have an unclosed
//! shell, with open edges up to 102 mm long. That length rules out a seam that failed to weld: a whole face
//! is missing.
//!
//! WHAT IT TURNED OUT TO BE, measured here: in that place the model itself has no face. Every one of the
//! solid`s 339 faces tessellates, the accuracy changes nothing, the triangles are wound outwards (asked by a
//! ray from each face), and the gap is bounded by two circles of 368 and 353 mm - the two ends of a missing
//! band of a cylinder. The kernel calls the solid unsound, and neither its own repair nor sewing with a
//! tolerance of up to half a millimetre closes it. Healing a model that arrives with a face missing is a piece
//! of work of its own, and this measurement is what it would start from.
//!
//! The fixture is one solid of the reporter's file, saved aside: `target/look/body-490.brep`, 365 KB against
//! the 287 MB the document weighs.
use crate::Shape;

fn fixture() -> Option<Shape> {
    let path = std::env::var("QYM_BREP").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/look/body-490.brep").to_string());
    std::fs::read(&path).ok().and_then(|b| Shape::from_brep_bytes(&b))
}

/// The edges of a mesh that belong to ONE triangle: the boundary of a hole.
fn open_edges(m: &qymcad_core::geom::Mesh) -> (usize, f64) {
    let key = |v: qymcad_core::geom::Point3| [(v.x * 1e4).round() as i64, (v.y * 1e4).round() as i64, (v.z * 1e4).round() as i64];
    let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
    let mut weld: Vec<u32> = Vec::with_capacity(m.verts.len());
    let mut pos: Vec<qymcad_core::geom::Point3> = Vec::new();
    for v in &m.verts {
        let n = at.len() as u32;
        let w = *at.entry(key(*v)).or_insert(n);
        if w as usize == pos.len() {
            pos.push(*v);
        }
        weld.push(w);
    }
    let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
    for t in &m.tris {
        for (a, c) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, c) = (weld[a as usize], weld[c as usize]);
            *edges.entry((a.min(c), a.max(c))).or_insert(0) += 1;
        }
    }
    let open: Vec<(u32, u32)> = edges.into_iter().filter(|(_, n)| *n != 2).map(|(e, _)| e).collect();
    let longest = open
        .iter()
        .map(|e| {
            let (a, c) = (pos[e.0 as usize], pos[e.1 as usize]);
            ((a.x - c.x).powi(2) + (a.y - c.y).powi(2) + (a.z - c.z).powi(2)).sqrt()
        })
        .fold(0.0_f64, f64::max);
    (open.len(), longest)
}

/// FACES WOUND AGAINST THEIR NEIGHBOURS. In a correctly oriented closed shell every shared edge is walked in
/// OPPOSITE directions by the two triangles that meet on it. A face turned inside out walks its edges the same
/// way as its neighbour - and, being back-facing, it is culled: you look through the part into its inside.
fn wound_against_a_neighbour(m: &qymcad_core::geom::Mesh) -> (usize, usize) {
    let key = |v: qymcad_core::geom::Point3| [(v.x * 1e4).round() as i64, (v.y * 1e4).round() as i64, (v.z * 1e4).round() as i64];
    let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
    let mut weld: Vec<u32> = Vec::with_capacity(m.verts.len());
    for v in &m.verts {
        let n = at.len() as u32;
        weld.push(*at.entry(key(*v)).or_insert(n));
    }
    // how each edge was walked: the count in one direction and in the other
    let mut walked: std::collections::HashMap<(u32, u32), (u32, u32)> = std::collections::HashMap::new();
    for t in &m.tris {
        for (a, c) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, c) = (weld[a as usize], weld[c as usize]);
            let e = walked.entry((a.min(c), a.max(c))).or_insert((0, 0));
            if a < c {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
    }
    let shared = walked.values().filter(|(f, b)| f + b == 2).count();
    let against = walked.values().filter(|(f, b)| *f == 2 || *b == 2).count();
    (against, shared)
}

/// FACES WHOSE TRIANGLES POINT THE OTHER WAY from the face itself.
///
/// Every face carries the normal of its own surface, taken analytically at the middle (`MeshFace::normal`),
/// with the face's orientation already applied. The triangles of that face must agree with it. Where they do
/// not, the face is drawn from the wrong side - and back-face culling removes it, leaving a hole through which
/// the inside of the part shows.
fn faces_against_their_own_normal(mesh: &qymcad_core::geom::Mesh, faces: &[qymcad_core::geom::MeshFace]) -> Vec<(u32, f64)> {
    let mut out = Vec::new();
    for f in faces {
        let (mut agree, mut against) = (0.0_f64, 0.0_f64);
        for &t in &f.triangles {
            let Some(tri) = mesh.tris.get(t as usize) else { continue };
            let p = |i: usize| mesh.verts[tri[i] as usize];
            let (a, b, c) = (p(0), p(1), p(2));
            let (u, v) = ([b.x - a.x, b.y - a.y, b.z - a.z], [c.x - a.x, c.y - a.y, c.z - a.z]);
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let area = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if area < 1e-12 {
                continue;
            }
            let dot = (n[0] * f.normal[0] + n[1] * f.normal[1] + n[2] * f.normal[2]) / area;
            if dot >= 0.0 {
                agree += area;
            } else {
                against += area;
            }
        }
        if against > agree && against > 0.0 {
            out.push((f.id, against * 0.5)); // half the cross product is the area of a triangle
        }
    }
    out
}

/// DOES THIS TRIANGLE REALLY FACE OUTWARDS - asked of the body itself, not of a flag.
///
/// A ray from just outside the triangle, along its own normal, leaves the body: if the surface is closed, the
/// number of times it crosses it is EVEN when the normal points outwards and ODD when it points into the
/// material. This is the ground truth the other two measures are checked against.
fn points_outwards(m: &qymcad_core::geom::Mesh, tri: [u32; 3]) -> Option<bool> {
    let p = |i: u32| m.verts[i as usize];
    let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
    let (u, v) = ([b.x - a.x, b.y - a.y, b.z - a.z], [c.x - a.x, c.y - a.y, c.z - a.z]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-12 {
        return None;
    }
    let n = [n[0] / len, n[1] / len, n[2] / len];
    let eps = 1e-4;
    let o = [(a.x + b.x + c.x) / 3.0 + n[0] * eps, (a.y + b.y + c.y) / 3.0 + n[1] * eps, (a.z + b.z + c.z) / 3.0 + n[2] * eps];
    // Moeller-Trumbore against every triangle of the body
    let mut hits = 0usize;
    for t in &m.tris {
        let (q0, q1, q2) = (p(t[0]), p(t[1]), p(t[2]));
        let e1 = [q1.x - q0.x, q1.y - q0.y, q1.z - q0.z];
        let e2 = [q2.x - q0.x, q2.y - q0.y, q2.z - q0.z];
        let h = [n[1] * e2[2] - n[2] * e2[1], n[2] * e2[0] - n[0] * e2[2], n[0] * e2[1] - n[1] * e2[0]];
        let det = e1[0] * h[0] + e1[1] * h[1] + e1[2] * h[2];
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let s = [o[0] - q0.x, o[1] - q0.y, o[2] - q0.z];
        let uu = (s[0] * h[0] + s[1] * h[1] + s[2] * h[2]) * inv;
        if !(0.0..=1.0).contains(&uu) {
            continue;
        }
        let q = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
        let vv = (n[0] * q[0] + n[1] * q[1] + n[2] * q[2]) * inv;
        if vv < 0.0 || uu + vv > 1.0 {
            continue;
        }
        let tt = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) * inv;
        if tt > 1e-7 {
            hits += 1;
        }
    }
    Some(hits.is_multiple_of(2))
}

#[test]
#[ignore = "a measurement over a fixture taken from the reporter's document"]
fn how_many_faces_go_missing_and_at_which_accuracy() {
    let Some(shape) = fixture() else {
        eprintln!("no fixture - run the eye with QYM_SAVE_BODY to make one");
        return;
    };
    eprintln!("MEASURED the solid of body 490");
    eprintln!("  the kernel calls this solid {}", if shape.is_valid() { "sound" } else { "NOT sound" });
    // the same solid after the kernel's own repair: if the holes are an orientation of the shell, they close
    let fixed = crate::fix_shell(&shape);
    if let Some(f) = &fixed {
        let qymcad_core::geom::Built { mesh, faces } = f.tessellate(0.5).into_iter().next().expect("the repaired solid tessellates");
        let (open, longest) = open_edges(&mesh);
        let (against, shared) = wound_against_a_neighbour(&mesh);
        let flipped = faces_against_their_own_normal(&mesh, &faces);
        eprintln!(
            "  after the repair: {} faces, open edges {open} (the longest {longest:.3} mm), wound against a neighbour {against} of {shared}, \
             against their own normal {}; the kernel calls it {}",
            faces.len(),
            flipped.len(),
            if f.is_valid() { "sound" } else { "NOT sound" }
        );
    } else {
        eprintln!("  the kernel refused to repair this solid");
    }
    // AND WITH THE FACES SEWN: an imported file may hand over faces that do not meet within tolerance
    for tol in [0.01_f64, 0.1, 0.5] {
        match crate::sew(&shape, tol) {
            Some(f) => {
                let qymcad_core::geom::Built { mesh, faces } = f.tessellate(0.5).into_iter().next().expect("the sewn solid tessellates");
                let (open, longest) = open_edges(&mesh);
                eprintln!("  sewn with {tol} mm: {} faces, open edges {open} (the longest {longest:.3} mm); the kernel calls it {}", faces.len(), if f.is_valid() { "sound" } else { "NOT sound" });
            }
            None => eprintln!("  sewing with {tol} mm was refused"),
        }
    }
    // HOW MANY CONNECTED PIECES the shell breaks into once the vertices are welded: a piece that stands
    // apart cannot be oriented by its neighbours, and if it is not closed the volume cannot decide either.
    {
        let qymcad_core::geom::Built { mesh, .. } = shape.tessellate(0.5).into_iter().next().expect("the solid tessellates");
        let grid = 1.0e-4;
        let key = |v: qymcad_core::geom::Point3| [(v.x / grid).round() as i64, (v.y / grid).round() as i64, (v.z / grid).round() as i64];
        let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
        let mut weld: Vec<u32> = Vec::with_capacity(mesh.verts.len());
        for v in &mesh.verts {
            let n = at.len() as u32;
            weld.push(*at.entry(key(*v)).or_insert(n));
        }
        let mut on_edge: std::collections::HashMap<(u32, u32), Vec<u32>> = std::collections::HashMap::new();
        for (i, t) in mesh.tris.iter().enumerate() {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (weld[a as usize], weld[b as usize]);
                on_edge.entry((a.min(b), a.max(b))).or_default().push(i as u32);
            }
        }
        let mut seen = vec![false; mesh.tris.len()];
        let mut sizes: Vec<usize> = Vec::new();
        for start in 0..mesh.tris.len() {
            if seen[start] {
                continue;
            }
            let mut n = 0usize;
            let mut queue = std::collections::VecDeque::from([start as u32]);
            seen[start] = true;
            while let Some(i) = queue.pop_front() {
                n += 1;
                let t = mesh.tris[i as usize];
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (a, b) = (weld[a as usize], weld[b as usize]);
                    for &j in on_edge.get(&(a.min(b), a.max(b))).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if !seen[j as usize] {
                            seen[j as usize] = true;
                            queue.push_back(j);
                        }
                    }
                }
            }
            sizes.push(n);
        }
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        eprintln!("  the shell breaks into {} pieces; the largest: {:?}", sizes.len(), &sizes[..sizes.len().min(8)]);
        // and which way each piece faces, asked by rays from nine of its triangles
        let mut seen = vec![false; mesh.tris.len()];
        for start in 0..mesh.tris.len() {
            if seen[start] {
                continue;
            }
            let mut piece: Vec<u32> = Vec::new();
            let mut queue = std::collections::VecDeque::from([start as u32]);
            seen[start] = true;
            while let Some(i) = queue.pop_front() {
                piece.push(i);
                let t = mesh.tris[i as usize];
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (a, b) = (weld[a as usize], weld[b as usize]);
                    for &j in on_edge.get(&(a.min(b), a.max(b))).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if !seen[j as usize] {
                            seen[j as usize] = true;
                            queue.push_back(j);
                        }
                    }
                }
            }
            let (mut out, mut into) = (0, 0);
            for &i in piece.iter().step_by((piece.len() / 9).max(1)).take(9) {
                match points_outwards(&mesh, mesh.tris[i as usize]) {
                    Some(true) => out += 1,
                    Some(false) => into += 1,
                    None => {}
                }
            }
            eprintln!("    a piece of {} triangles: {out} rays say outwards, {into} say inwards", piece.len());
        }
    }
    // WHAT THE OPEN EDGES BOUND: gathered into loops, with the perimeter of each. A loop of a few microns is
    // a seam that failed to weld; a loop of hundreds of millimetres is a piece of surface that is not there.
    {
        let qymcad_core::geom::Built { mesh, .. } = shape.tessellate(0.5).into_iter().next().expect("the solid tessellates");
        let grid = 1.0e-4;
        let key = |v: qymcad_core::geom::Point3| [(v.x / grid).round() as i64, (v.y / grid).round() as i64, (v.z / grid).round() as i64];
        let mut at: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
        let mut weld: Vec<u32> = Vec::with_capacity(mesh.verts.len());
        let mut pos: Vec<qymcad_core::geom::Point3> = Vec::new();
        for v in &mesh.verts {
            let n = at.len() as u32;
            let w = *at.entry(key(*v)).or_insert(n);
            if w as usize == pos.len() {
                pos.push(*v);
            }
            weld.push(w);
        }
        let mut count: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
        for t in &mesh.tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (weld[a as usize], weld[b as usize]);
                *count.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        let open: Vec<(u32, u32)> = count.into_iter().filter(|(_, n)| *n != 2).map(|(e, _)| e).collect();
        let mut next: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for &(a, b) in &open {
            next.entry(a).or_default().push(b);
            next.entry(b).or_default().push(a);
        }
        let mut walked: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        let mut loops: Vec<f64> = Vec::new();
        for &(a, b) in &open {
            if !walked.insert((a, b)) {
                continue;
            }
            let (mut len, mut prev, mut cur) = (0.0_f64, a, b);
            len += (pos[a as usize].x - pos[b as usize].x).hypot(pos[a as usize].y - pos[b as usize].y).hypot(pos[a as usize].z - pos[b as usize].z);
            while let Some(step) = next.get(&cur).and_then(|v| v.iter().copied().find(|&n| n != prev && walked.insert((cur.min(n), cur.max(n))))) {
                len += (pos[cur as usize].x - pos[step as usize].x).hypot(pos[cur as usize].y - pos[step as usize].y).hypot(pos[cur as usize].z - pos[step as usize].z);
                prev = cur;
                cur = step;
                if cur == a {
                    break;
                }
            }
            loops.push(len);
        }
        loops.sort_by(|a, b| b.total_cmp(a));
        eprintln!("  the open edges make {} loops; the longest perimeters: {:?} mm", loops.len(), loops.iter().take(6).map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>());
    }
    // THE GROUND TRUTH over a sample of faces: does the mesh really face outwards where it says it does
    {
        let qymcad_core::geom::Built { mesh, faces } = shape.tessellate(0.5).into_iter().next().expect("the solid tessellates");
        let flipped: std::collections::HashSet<u32> = faces_against_their_own_normal(&mesh, &faces).into_iter().map(|(id, _)| id).collect();
        let (mut inward, mut asked, mut agreed) = (0usize, 0usize, 0usize);
        for f in faces.iter().step_by((faces.len() / 60).max(1)) {
            let Some(&t) = f.triangles.first() else { continue };
            let Some(out) = points_outwards(&mesh, mesh.tris[t as usize]) else { continue };
            asked += 1;
            if !out {
                inward += 1;
            }
            if !out == flipped.contains(&f.id) {
                agreed += 1;
            }
        }
        eprintln!("  by the ray: {inward} of {asked} sampled faces point INTO the body; the normal test agrees about {agreed} of them");
    }
    for defl in [0.5_f64, 0.2, 0.1, 0.05, 0.01] {
        let t0 = std::time::Instant::now();
        let bodies = shape.tessellate(defl);
        let took = t0.elapsed();
        match bodies.first() {
            Some(qymcad_core::geom::Built { mesh, faces }) => {
                let (open, longest) = open_edges(mesh);
                let (against, shared) = wound_against_a_neighbour(mesh);
                let (unmeshed, all_faces) = crate::unmeshed_faces(&shape, defl);
                let flipped = faces_against_their_own_normal(mesh, faces);
                let area: f64 = flipped.iter().map(|(_, a)| a).sum();
                eprintln!("    of {all_faces} faces of the solid, {unmeshed} have no triangulation; {} are triangulated against their own normal ({area:.0} mm^2)", flipped.len());
                eprintln!(
                    "  deflection {defl:.3} mm: {} faces, {} triangles, open edges {open} (the longest {longest:.3} mm), \
                     wound against a neighbour {against} of {shared} shared, {:.0} ms",
                    faces.len(),
                    mesh.tris.len(),
                    took.as_secs_f64() * 1000.0
                );
            }
            None => eprintln!("  deflection {defl:.3} mm: nothing came out at all"),
        }
    }
}
