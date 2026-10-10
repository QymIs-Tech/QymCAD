//! OBJ: a mesh as text - vertices and polygons, one object per body.
//!
//! Only the geometry is taken: texture coordinates, normals, materials and smoothing groups are passed over.
//! OBJ carries no unit, so its numbers are taken as millimetres, as every mesh here is.
use std::collections::HashMap;
use std::fmt::Write as _;

use qymcad_core::geom::{Mesh, Point3};

use crate::NamedMesh;

/// Read an OBJ file into meshes, one per object (`o name`) under its name, or a single unnamed one when the file
/// names none.
pub fn import_obj(path: &str) -> Result<Vec<NamedMesh>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("io-obj-read-failed#{e}"))?;
    let Parsed { mut meshes, used, libs } = parse(&text)?;
    // THE MATERIALS BESIDE THE FILE give the colours: `Kd` as a program writes it, taken as sRGB - the owner's print
    // head writes the same numbers into its MTL as its STEP holds for the same parts. A missing library is no colour.
    let dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let kd: HashMap<String, [u8; 3]> = libs.iter().filter_map(|l| std::fs::read_to_string(dir.join(l)).ok()).flat_map(|t| diffuse(&t)).collect();
    for (m, u) in meshes.iter_mut().zip(used) {
        m.color = u.and_then(|n| kd.get(&n).copied());
    }
    Ok(meshes)
}

/// The diffuse colour of every material of an MTL library.
fn diffuse(mtl: &str) -> Vec<(String, [u8; 3])> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in mtl.lines() {
        let mut w = line.split('#').next().unwrap_or("").split_whitespace();
        match w.next() {
            Some("newmtl") => name = Some(w.collect::<Vec<_>>().join(" ")),
            Some("Kd") => {
                let c: Vec<f64> = w.take(3).filter_map(|v| v.parse().ok()).collect();
                if let (Some(n), 3) = (&name, c.len()) {
                    out.push((n.clone(), [crate::srgb_byte(c[0]), crate::srgb_byte(c[1]), crate::srgb_byte(c[2])]));
                }
            }
            _ => {}
        }
    }
    out
}

/// What an OBJ file holds once read.
struct Parsed {
    meshes: Vec<NamedMesh>,
    /// the material each mesh is drawn in: the first it names
    used: Vec<Option<String>>,
    /// the material libraries the file asks for
    libs: Vec<String>,
}

/// A polygon of n corners becomes n - 2 triangles cut inside it (`triangulate`). An index may count back from
/// the end (`-1` is the last vertex so far), and of `a/b/c` only the vertex is kept. The vertices belong to
/// the whole file, so each object keeps only those its faces use - in the file's order, renumbered from zero, so
/// a file this program wrote reads back with the same numbers.
fn parse(text: &str) -> Result<Parsed, String> {
    let joined = text.replace("\\\r\n", " ").replace("\\\n", " "); // a trailing backslash continues the line
    let mut all: Vec<Point3> = Vec::new();
    let mut objects: Vec<(String, Vec<[usize; 3]>, Option<String>)> = vec![(String::new(), Vec::new(), None)];
    let mut libs: Vec<String> = Vec::new();
    for (no, raw) in joined.lines().enumerate() {
        let bad = || format!("io-obj-bad-line#{}", no + 1);
        let line = raw.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let w: Vec<&str> = words.take(3).collect();
                let c: Vec<f64> = w.iter().map(|w| w.parse::<f64>()).collect::<Result<_, _>>().map_err(|_| bad())?;
                if c.len() < 3 {
                    return Err(bad());
                }
                // A CORNER IS A FINITE NUMBER: `NaN` and `inf` parse as f64
                if let Some(k) = c.iter().position(|v| !v.is_finite()) {
                    return Err(crate::not_finite("io-obj-not-finite-line", &[&(no + 1), &w[k]]));
                }
                all.push(Point3::new(c[0], c[1], c[2]));
            }
            Some("f") => {
                let mut corners = Vec::new();
                for w in words {
                    let i: i64 = w.split('/').next().unwrap_or("").parse().map_err(|_| bad())?;
                    let at = if i > 0 { i - 1 } else { all.len() as i64 + i };
                    if i == 0 || at < 0 || at as usize >= all.len() {
                        return Err(format!("io-obj-bad-index#{}", no + 1));
                    }
                    corners.push(at as usize);
                }
                if corners.len() < 3 {
                    return Err(bad());
                }
                let faces = &mut objects.last_mut().expect("there is always an object to add to").1;
                faces.extend(triangulate(&corners, &all));
            }
            // A new object. `g` is not taken for one: it also marks material and smoothing groups inside a
            // single object, and splitting on it would cut one body into pieces.
            Some("o") => {
                let name = words.collect::<Vec<_>>().join(" ");
                match objects.last_mut() {
                    Some(last) if last.1.is_empty() => last.0 = name, // nothing drawn yet under the one before
                    _ => objects.push((name, Vec::new(), None)),
                }
            }
            Some("usemtl") => {
                let material = words.collect::<Vec<_>>().join(" ");
                if let Some(last) = objects.last_mut().filter(|o| o.2.is_none()) {
                    last.2 = Some(material);
                }
            }
            Some("mtllib") => libs.push(words.collect::<Vec<_>>().join(" ")),
            _ => {}
        }
    }
    let mut used = Vec::new();
    let meshes: Vec<NamedMesh> = objects
        .into_iter()
        .filter(|(_, f, _)| !f.is_empty())
        .map(|(name, faces, material)| {
            used.push(material);
            let used: std::collections::BTreeSet<usize> = faces.iter().flatten().copied().collect();
            let renumber: HashMap<usize, u32> = used.iter().enumerate().map(|(k, i)| (*i, k as u32)).collect();
            let verts: Vec<Point3> = used.iter().map(|i| all[*i]).collect();
            let tris = faces.into_iter().map(|f| f.map(|i| renumber[&i])).collect();
            NamedMesh { name: crate::authored(name), mesh: Mesh { verts, tris }, color: None, place: qymcad_core::feature::PLACE_IDENTITY, tri_colors: Vec::new(), within: Vec::new() }
        })
        .collect();
    if meshes.is_empty() {
        return Err("io-obj-no-faces".into());
    }
    Ok(Parsed { meshes, used, libs })
}

/// Cuts a polygon into triangles inside itself. A fan from the first corner is right only for a convex polygon: a
/// remesher's file of 3 672 quads had 1 077 concave or twisted ones, and the fan folded every one of them over its
/// own neighbour. So a quad takes the diagonal whose two halves face most alike (881 of those 1 077 come out flat;
/// the other 196 are twisted so that either diagonal folds), and a larger concave polygon is clipped ear by ear.
/// What cannot be clipped - a polygon crossing itself - is fanned as before.
fn triangulate(c: &[usize], at: &[Point3]) -> Vec<[usize; 3]> {
    let fan = || (1..c.len() - 1).map(|k| [c[0], c[k], c[k + 1]]).collect();
    let p = |i: usize| at[c[i]];
    match c.len() {
        3 => vec![[c[0], c[1], c[2]]],
        4 => {
            if agreement(p(0), p(1), p(2), p(3)) >= agreement(p(1), p(2), p(3), p(0)) {
                vec![[c[0], c[1], c[2]], [c[0], c[2], c[3]]]
            } else {
                vec![[c[1], c[2], c[3]], [c[1], c[3], c[0]]]
            }
        }
        _ => clip_ears(c, at).unwrap_or_else(fan),
    }
}

/// How alike the two halves of the quad `a b c d` cut along `a-c` face: the cosine between their normals, and -2
/// when a half has no area.
fn agreement(a: Point3, b: Point3, c: Point3, d: Point3) -> f64 {
    let (n1, n2) = (normal(a, b, c), normal(a, c, d));
    let (l1, l2) = (dot(n1, n1).sqrt(), dot(n2, n2).sqrt());
    if l1 == 0.0 || l2 == 0.0 {
        -2.0
    } else {
        dot(n1, n2) / (l1 * l2)
    }
}

/// Ear clipping in the plane the polygon best lies in: its Newell normal picks the two axes to drop it onto. A
/// convex polygon is fanned at once, so a cap of hundreds of corners costs nothing. `None` when no ear is left - the
/// polygon crosses itself.
fn clip_ears(c: &[usize], at: &[Point3]) -> Option<Vec<[usize; 3]>> {
    let mut n = [0.0; 3];
    for k in 0..c.len() {
        let (a, b) = (at[c[k]], at[c[(k + 1) % c.len()]]);
        n[0] += (a.y - b.y) * (a.z + b.z);
        n[1] += (a.z - b.z) * (a.x + b.x);
        n[2] += (a.x - b.x) * (a.y + b.y);
    }
    let big = (0..3).max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs()))?;
    // (u, v) is right-handed with the dropped axis, so the polygon runs counter-clockwise in it when that component
    // of the normal is positive
    let (u, v, sign) = ((big + 1) % 3, (big + 2) % 3, n[big].signum());
    let flat = |i: usize| {
        let q = [at[i].x, at[i].y, at[i].z];
        (q[u], q[v])
    };
    let turn = |a: (f64, f64), b: (f64, f64), q: (f64, f64)| ((b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0)) * sign;
    let m = c.len();
    if (0..m).all(|k| turn(flat(c[(k + m - 1) % m]), flat(c[k]), flat(c[(k + 1) % m])) > 0.0) {
        return Some((1..m - 1).map(|k| [c[0], c[k], c[k + 1]]).collect());
    }
    let mut left: Vec<usize> = c.to_vec();
    let mut out = Vec::with_capacity(m - 2);
    while left.len() > 3 {
        let m = left.len();
        let ear = (0..m).find(|&k| {
            let (i0, i1, i2) = (left[(k + m - 1) % m], left[k], left[(k + 1) % m]);
            let (a, b, q) = (flat(i0), flat(i1), flat(i2));
            turn(a, b, q) > 0.0
                && left.iter().all(|&o| {
                    let r = flat(o);
                    o == i0 || o == i1 || o == i2 || turn(a, b, r) < 0.0 || turn(b, q, r) < 0.0 || turn(q, a, r) < 0.0
                })
        })?;
        out.push([left[(ear + m - 1) % m], left[ear], left[(ear + 1) % m]]);
        left.remove(ear);
    }
    out.push([left[0], left[1], left[2]]);
    Some(out)
}

fn normal(a: Point3, b: Point3, c: Point3) -> [f64; 3] {
    let (u, w) = ([b.x - a.x, b.y - a.y, b.z - a.z], [c.x - a.x, c.y - a.y, c.z - a.z]);
    [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]]
}

fn dot(u: [f64; 3], w: [f64; 3]) -> f64 {
    u[0] * w[0] + u[1] * w[1] + u[2] * w[2]
}

/// Write meshes into one OBJ file, one object per mesh. The numbers go out in their shortest exact form, so the
/// file read back gives the same coordinates to the last bit.
pub fn export_obj(meshes: &[Mesh], path: &str) -> Result<(), String> {
    let total: usize = meshes.iter().map(|m| m.tris.len()).sum();
    if total == 0 {
        return Err("io-obj-no-triangles".into());
    }
    let mut out = String::with_capacity(total * 48);
    let mut base = 1usize; // OBJ counts vertices from one, across the whole file
    for (k, m) in meshes.iter().enumerate() {
        let _ = writeln!(out, "o body_{}", k + 1);
        for p in &m.verts {
            let _ = writeln!(out, "v {} {} {}", p.x, p.y, p.z);
        }
        for t in &m.tris {
            let _ = writeln!(out, "f {} {} {}", t[0] as usize + base, t[1] as usize + base, t[2] as usize + base);
        }
        base += m.verts.len();
    }
    std::fs::write(path, out).map_err(|e| format!("io-obj-write-failed#{e}"))
}
