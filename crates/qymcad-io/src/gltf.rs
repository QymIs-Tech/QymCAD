//! glTF: the format of the web and of rendering - a scene of nodes with their places, each carrying meshes.
//!
//! Read and written here rather than through OCCT: OCCT reads and writes glTF through RapidJSON, and the OCCT
//! the packages are built with has it switched off (`USE_RAPIDJSON=OFF` in the release build), so a kernel path
//! would work on the developer's machine and nowhere else.
//!
//! TWO CONVENTIONS OF glTF ARE NOT OURS, and both are turned at the border. The unit is the metre (ours is the
//! millimetre), and up is +Y (ours is +Z): a part written without turning lies on its back in every viewer.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::{ExportMesh, ExportNode, Id};
use serde_json::Value;

use crate::NamedMesh;

const MM_PER_M: f64 = 1000.0;

/// From glTF (metres, +Y up) to ours (millimetres, +Z up).
fn inward(p: [f64; 3]) -> Point3 {
    Point3::new(p[0] * MM_PER_M, -p[2] * MM_PER_M, p[1] * MM_PER_M)
}

/// From ours to glTF: the exact inverse of `inward`.
fn outward(p: &Point3) -> [f32; 3] {
    [(p.x / MM_PER_M) as f32, (p.z / MM_PER_M) as f32, (-p.y / MM_PER_M) as f32]
}

/// Read a glTF scene - `.gltf` with its buffers beside it or inside it, or a single `.glb` - into bodies.
pub fn import_gltf(path: &str) -> Result<Vec<NamedMesh>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-gltf-read-failed#{e}"))?;
    let (json, bin) = if bytes.starts_with(b"glTF") { split_glb(&bytes)? } else { (bytes.clone(), None) };
    let doc: Value = serde_json::from_slice(&json).map_err(|_| "io-gltf-not-gltf".to_string())?;
    let dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let buffers = load_buffers(&doc, bin, &dir)?;
    let mut out = Vec::new();
    let nodes = doc["nodes"].as_array().cloned().unwrap_or_default();
    let roots: Vec<usize> = match doc["scenes"].get(doc["scene"].as_u64().unwrap_or(0) as usize).and_then(|s| s["nodes"].as_array()) {
        Some(r) => r.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect(),
        None => (0..nodes.len()).filter(|i| !nodes.iter().any(|n| n["children"].as_array().is_some_and(|c| c.iter().any(|v| v.as_u64() == Some(*i as u64))))).collect(),
    };
    // A TREE WHERE EVERY NODE STANDS RIGIDLY: the nodes that hold others come as groups and every piece keeps its
    // node's frame. What a node scales, shears or mirrors no place can hold: it is baked into the meshes under the node
    // and the tree stays (see `walk`).
    let scene = Scene { doc: &doc, buffers: &buffers };
    for r in roots {
        walk(&scene, r, &crate::rigid::ONE, &[], &mut out, 0)?;
    }
    if out.is_empty() {
        return Err("io-gltf-no-meshes".into());
    }
    Ok(out)
}

/// A column-major 4x4, as glTF writes it.
type M4 = [f64; 16];
const IDENTITY: M4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

/// The linear part of a column-major 4x4, as `rigid` takes it.
fn lin_of(m: &M4) -> crate::rigid::Lin {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c * 4 + r]))
}

/// `m` with its linear part `l`, its shift kept.
fn with_lin(m: &M4, l: &crate::rigid::Lin) -> M4 {
    let mut out = *m;
    for r in 0..3 {
        for c in 0..3 {
            out[c * 4 + r] = l[r][c];
        }
    }
    out
}

/// A NODE'S TRANSFORM AS OUR PLACEMENT, 3x4 row-major: its turn seen in our axes and its shift in millimetres - the
/// frame `inward` turns points into. `None` for a transform no component's place can hold: one that scales, shears or
/// mirrors (to 1e-6).
fn placement(m: &M4) -> Option<[f64; 12]> {
    let ours = |v: [f64; 3]| [v[0], -v[2], v[1]];
    let turn = |v: [f64; 3]| [0, 1, 2].map(|r| m[r] * v[0] + m[4 + r] * v[1] + m[8 + r] * v[2]);
    // our x, y and z as glTF sees them, turned by the node and seen again in ours: the columns of the turn
    let c = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]].map(|e| ours(turn(e)));
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let square = (0..3).all(|i| (dot(c[i], c[i]) - 1.0).abs() < 1e-6) && dot(c[0], c[1]).abs() < 1e-6 && dot(c[1], c[2]).abs() < 1e-6 && dot(c[0], c[2]).abs() < 1e-6;
    if !square || dot(cross(c[0], c[1]), c[2]) <= 0.0 || m[3] != 0.0 || m[7] != 0.0 || m[11] != 0.0 {
        return None;
    }
    let t = ours([m[12], m[13], m[14]]).map(|v| v * MM_PER_M);
    Some([c[0][0], c[1][0], c[2][0], t[0], c[0][1], c[1][1], c[2][1], t[1], c[0][2], c[1][2], c[2][2], t[2]])
}

/// A node's own transform: its matrix, or translation * rotation * scale.
fn local(node: &Value) -> M4 {
    if let Some(m) = node["matrix"].as_array().filter(|m| m.len() == 16) {
        let mut out = [0.0; 16];
        for (k, v) in m.iter().enumerate() {
            out[k] = v.as_f64().unwrap_or(0.0);
        }
        return out;
    }
    let f = |key: &str, k: usize, d: f64| node[key].get(k).and_then(Value::as_f64).unwrap_or(d);
    let (tx, ty, tz) = (f("translation", 0, 0.0), f("translation", 1, 0.0), f("translation", 2, 0.0));
    let (x, y, z, w) = (f("rotation", 0, 0.0), f("rotation", 1, 0.0), f("rotation", 2, 0.0), f("rotation", 3, 1.0));
    let (sx, sy, sz) = (f("scale", 0, 1.0), f("scale", 1, 1.0), f("scale", 2, 1.0));
    let r = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    [r[0] * sx, r[1] * sx, r[2] * sx, 0.0, r[3] * sy, r[4] * sy, r[5] * sy, 0.0, r[6] * sz, r[7] * sz, r[8] * sz, 0.0, tx, ty, tz, 1.0]
}

/// The scene being read: its document and buffers.
struct Scene<'a> {
    doc: &'a Value,
    buffers: &'a [Vec<u8>],
}

/// A node and all under it into `out`. Every node keeps its own frame: a node that holds others is a group the pieces
/// under it stand in (`within`, with its place), a node's mesh stays in the node's coordinates and the turn and shift
/// of the node's transform are the piece's place. `rest` is what the nodes above scale, shear or mirror: it takes this
/// node whole, its shift too, and what it and the node's own transform leave beyond a turn is baked into the node's
/// mesh and handed down to its children.
fn walk(scene: &Scene, at: usize, rest: &crate::rigid::Lin, within: &[qymcad_core::model::FileGroup], out: &mut Vec<NamedMesh>, depth: usize) -> Result<(), String> {
    let (doc, buffers) = (scene.doc, scene.buffers);
    let node = &doc["nodes"][at];
    if node.is_null() || depth > 64 {
        return Err("io-gltf-bad-node".into());
    }
    let own = mul(&with_lin(&IDENTITY, rest), &local(node));
    // a node that flattens space (a scale of 0, which hides a node) keeps its shift and bakes the rest as it is
    let (turn, left) = crate::rigid::split(&lin_of(&own)).unwrap_or((crate::rigid::ONE, lin_of(&own)));
    let (frame, place) = (with_lin(&IDENTITY, &left), placement(&with_lin(&own, &turn)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY));
    let kids = node["children"].as_array().cloned().unwrap_or_default();
    let mesh_name = node["mesh"].as_u64().and_then(|mi| doc["meshes"][mi as usize]["name"].as_str());
    let name = crate::authored(node["name"].as_str().or(mesh_name).unwrap_or("").to_string());
    // a node that holds others is a group; its own mesh, where it has one, stands in it at the group's zero
    let (inner, piece_place) = if !kids.is_empty() {
        let mut w = within.to_vec();
        w.push(qymcad_core::model::FileGroup { index: at, name: name.clone(), place });
        (w, qymcad_core::feature::PLACE_IDENTITY)
    } else {
        (within.to_vec(), place)
    };
    if let Some(mi) = node["mesh"].as_u64() {
        let mut mesh = Mesh { verts: Vec::new(), tris: Vec::new() };
        let mut per: Vec<Option<[u8; 3]>> = Vec::new(); // the colour of every triangle, by its primitive's material
        for prim in doc["meshes"][mi as usize]["primitives"].as_array().cloned().unwrap_or_default() {
            let before = mesh.tris.len();
            primitive(doc, buffers, &prim, &frame, &mut mesh)?;
            let c = prim["material"].as_u64().and_then(|k| base_colour(doc, k as usize));
            per.extend(std::iter::repeat_n(c, mesh.tris.len() - before));
        }
        if !mesh.tris.is_empty() {
            let color = doc["meshes"][mi as usize]["primitives"].as_array().and_then(|ps| ps.iter().find_map(|p| p["material"].as_u64())).and_then(|k| base_colour(doc, k as usize));
            let tri_colors = crate::several_colours(&per, color);
            out.push(NamedMesh { name, mesh, color, place: piece_place, tri_colors, within: inner.clone() });
        }
    }
    for c in kids {
        walk(scene, c.as_u64().ok_or("io-gltf-bad-node")? as usize, &left, &inner, out, depth + 1)?;
    }
    Ok(())
}

/// THE COLOUR OF A MATERIAL as the bytes a colour picker shows. glTF writes `baseColorFactor` in linear light, and it
/// is turned into sRGB, as the format says. A material with no factor gives none, and the piece keeps the palette.
fn base_colour(doc: &Value, k: usize) -> Option<[u8; 3]> {
    let f = doc["materials"][k]["pbrMetallicRoughness"]["baseColorFactor"].as_array()?;
    let srgb = |i: usize| {
        let c = f.get(i).and_then(Value::as_f64).unwrap_or(1.0).clamp(0.0, 1.0);
        crate::srgb_byte(if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 })
    };
    Some([srgb(0), srgb(1), srgb(2)])
}

/// One primitive into `mesh`: triangles as they are, a strip or a fan unrolled; points and lines have no
/// surface and are passed over.
fn primitive(doc: &Value, buffers: &[Vec<u8>], prim: &Value, m: &M4, mesh: &mut Mesh) -> Result<(), String> {
    let mode = prim["mode"].as_u64().unwrap_or(4);
    if !(4..=6).contains(&mode) {
        return Ok(());
    }
    let pos = accessor(doc, buffers, prim["attributes"]["POSITION"].as_u64().ok_or("io-gltf-no-positions")? as usize)?;
    let base = mesh.verts.len() as u32;
    for p in pos.chunks(3) {
        let q = [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
        mesh.verts.push(inward(q));
    }
    let n = (pos.len() / 3) as u32;
    let idx: Vec<u32> = match prim["indices"].as_u64() {
        Some(a) => accessor(doc, buffers, a as usize)?.into_iter().map(|v| v as u32).collect(),
        None => (0..n).collect(),
    };
    if idx.iter().any(|i| *i >= n) {
        return Err("io-gltf-bad-index".into());
    }
    let tri = |a: u32, b: u32, c: u32| [base + a, base + b, base + c];
    let first = mesh.tris.len();
    match mode {
        4 => mesh.tris.extend(idx.as_chunks::<3>().0.iter().map(|t| tri(t[0], t[1], t[2]))),
        5 => mesh.tris.extend((2..idx.len()).map(|k| if k % 2 == 0 { tri(idx[k - 2], idx[k - 1], idx[k]) } else { tri(idx[k - 1], idx[k - 2], idx[k]) })),
        _ => mesh.tris.extend((2..idx.len()).map(|k| tri(idx[0], idx[k - 1], idx[k]))),
    }
    // A MIRROR TURNS THE TRIANGLES OVER: glTF takes the winding as counterclockwise where the determinant of a node's
    // global transform is positive and clockwise where it is negative. The places above are turns, so that sign is
    // the sign of `m`.
    if crate::rigid::det(&lin_of(m)) < 0.0 {
        for t in &mut mesh.tris[first..] {
            t.swap(1, 2);
        }
    }
    Ok(())
}

/// The numbers of an accessor, whatever their component type, as `f64`. A normalised integer stays a raw integer:
/// positions and indices are never normalised.
fn accessor(doc: &Value, buffers: &[Vec<u8>], at: usize) -> Result<Vec<f64>, String> {
    let bad = || "io-gltf-bad-accessor".to_string();
    let a = &doc["accessors"][at];
    let count = a["count"].as_u64().ok_or_else(bad)? as usize;
    let width = match a["type"].as_str().ok_or_else(bad)? {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return Err(bad()),
    };
    let (size, read): (usize, fn(&[u8]) -> f64) = match a["componentType"].as_u64().ok_or_else(bad)? {
        5120 => (1, |b| b[0] as i8 as f64),
        5121 => (1, |b| b[0] as f64),
        5122 => (2, |b| i16::from_le_bytes([b[0], b[1]]) as f64),
        5123 => (2, |b| u16::from_le_bytes([b[0], b[1]]) as f64),
        5125 => (4, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64),
        5126 => (4, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64),
        _ => return Err(bad()),
    };
    let view = &doc["bufferViews"][a["bufferView"].as_u64().ok_or_else(bad)? as usize];
    let buf = buffers.get(view["buffer"].as_u64().ok_or_else(bad)? as usize).ok_or_else(bad)?;
    let element = size * width;
    // glTF 2.0 writes no stride, or one of 4 to 252; a stride of 0 is glTF 1.0's way of saying the items lie side by side,
    // and older writers and converters still emit it, so it is read so. Taken literally, 0 made every item the same few
    // bytes, so any count passed the reads below. A stride narrower than one item would make the items overlap.
    let stride = match view["byteStride"].as_u64() {
        None | Some(0) => element,
        Some(s) if s as usize >= element => s as usize,
        Some(_) => return Err(bad()),
    };
    // NOTHING IS RESERVED BEFORE THE DATA IS KNOWN TO BE THERE. The count comes from the file; reserving it first let a
    // 640-byte file with a count of 4,000,000,000 ask for 96,000,000,000 bytes, and the failed request aborted the
    // program (measured). So the last item must end inside the buffer view - and inside the buffer - computed without
    // overflow, before a single item is read.
    let view_start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    let view_end = match view["byteLength"].as_u64() {
        Some(len) => view_start.checked_add(len as usize).ok_or_else(bad)?,
        None => buf.len(),
    };
    let start = view_start.checked_add(a["byteOffset"].as_u64().unwrap_or(0) as usize).ok_or_else(bad)?;
    if count > 0 {
        let end = (count - 1).checked_mul(stride).and_then(|o| o.checked_add(start)).and_then(|o| o.checked_add(element)).ok_or_else(bad)?;
        if end > view_end || end > buf.len() {
            return Err(bad());
        }
    }
    let mut out = Vec::with_capacity(count * width);
    for i in 0..count {
        for k in 0..width {
            let at = start + i * stride + k * size;
            out.push(read(buf.get(at..at + size).ok_or_else(bad)?));
        }
    }
    Ok(out)
}

/// A `.glb`: the JSON chunk and the binary chunk.
fn split_glb(b: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let u = |at: usize| b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).ok_or_else(|| "io-gltf-truncated".to_string());
    let (mut at, mut json, mut bin) = (12usize, None, None);
    while at + 8 <= b.len() {
        let (len, kind) = (u(at)?, u(at + 4)?);
        let body = b.get(at + 8..at + 8 + len).ok_or_else(|| "io-gltf-truncated".to_string())?.to_vec();
        match kind {
            0x4E4F_534A => json = Some(body),
            0x004E_4942 => bin = Some(body),
            _ => {}
        }
        at += 8 + len;
    }
    Ok((json.ok_or_else(|| "io-gltf-not-gltf".to_string())?, bin))
}

fn load_buffers(doc: &Value, mut bin: Option<Vec<u8>>, dir: &std::path::Path) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for b in doc["buffers"].as_array().cloned().unwrap_or_default() {
        let data = match b["uri"].as_str() {
            None => bin.take().ok_or_else(|| "io-gltf-no-buffer".to_string())?,
            Some(uri) if uri.starts_with("data:") => base64(uri.split_once(',').map(|(_, d)| d).unwrap_or("")).ok_or_else(|| "io-gltf-bad-buffer".to_string())?,
            Some(uri) => std::fs::read(dir.join(uri.replace("%20", " "))).map_err(|_| format!("io-gltf-missing-buffer#{uri}"))?,
        };
        out.push(data);
    }
    Ok(out)
}

/// Base64 as a data URI carries it. Written here: a dependency for twenty lines is not worth its weight.
fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut acc = 0u32;
        for (k, c) in chunk.iter().enumerate() {
            acc |= val(*c)? << (18 - 6 * k);
        }
        let bytes = acc.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(out)
}

/// Write bodies into one `.glb`: a node and a mesh per body, positions in metres with +Y up, indices as u32.
pub fn export_glb(meshes: &[Mesh], path: &str) -> Result<(), String> {
    if meshes.iter().all(|m| m.tris.is_empty()) {
        return Err("io-gltf-no-triangles".into());
    }
    let mut out = Out::default();
    let mut nodes = Vec::new();
    for (k, m) in meshes.iter().enumerate().filter(|(_, m)| !m.tris.is_empty()) {
        let mesh = out.mesh(m, None, &[]);
        nodes.push(serde_json::json!({"name": format!("body_{}", k + 1), "mesh": mesh}));
    }
    let roots = (0..nodes.len()).collect();
    out.write(nodes, roots, path)
}

/// The triangles of a mesh that share one colour: one primitive of the file. `None` is the part's colour.
struct ColourGroup {
    colour: Option<[u8; 3]>,
    tris: Vec<[u32; 3]>,
}

/// WRITE A TREE AS THE SCENE: a node per subassembly and per part under its name, each where it stands in its parent,
/// a part carrying the mesh of its body - in the part's own coordinates - in the material of its colour. A repeat
/// (`same_as`) carries the mesh of the part it repeats, which the file holds once. `meshes` gives the mesh of every
/// body that goes out.
pub fn export_glb_tree(nodes: &[ExportNode], meshes: &[ExportMesh], path: &str) -> Result<(), String> {
    let by_body: std::collections::HashMap<Id, &ExportMesh> = meshes.iter().map(|m| (m.body, m)).collect();
    let mut out = Out::default();
    let mut carried: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut written = Vec::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        carried[i] = match n.same_as {
            Some(k) if k < i => carried[k],
            _ => n.body.and_then(|b| by_body.get(&b)).filter(|m| !m.mesh.tris.is_empty()).map(|m| out.mesh(&m.mesh, n.color, &m.tri_colors)),
        };
        let mut node = serde_json::json!({"name": n.name});
        if n.place != qymcad_core::feature::PLACE_IDENTITY {
            node["matrix"] = serde_json::json!(outward_place(&n.place));
        }
        if let Some(k) = carried[i] {
            node["mesh"] = serde_json::json!(k);
        }
        let children: Vec<usize> = (0..nodes.len()).filter(|&c| nodes[c].parent == Some(i)).collect();
        if !children.is_empty() {
            node["children"] = serde_json::json!(children);
        }
        written.push(node);
    }
    if out.meshes.is_empty() {
        return Err("io-gltf-no-triangles".into());
    }
    let roots = (0..nodes.len()).filter(|&i| nodes[i].parent.is_none()).collect();
    out.write(written, roots, path)
}

/// One of our placements (3x4 row-major, millimetres, +Z up) as a node matrix of glTF (4x4 column-major, metres, +Y
/// up): the same motion seen through the turn and the scale `outward` puts the points through.
fn outward_place(m: &[f64; 12]) -> [f64; 16] {
    const P: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]]; // outward's turn: (x, y, z) -> (x, z, -y)
    let mut out = [0.0; 16];
    for i in 0..3 {
        for j in 0..3 {
            out[j * 4 + i] = (0..3).map(|k| (0..3).map(|l| P[i][k] * m[k * 4 + l] * P[j][l]).sum::<f64>()).sum();
        }
        out[12 + i] = (0..3).map(|k| P[i][k] * m[k * 4 + 3]).sum::<f64>() / MM_PER_M;
    }
    out[15] = 1.0;
    out
}

/// A `.glb` being written: the binary buffer, and the lists of JSON that point into it.
#[derive(Default)]
struct Out {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    meshes: Vec<Value>,
    materials: Vec<Value>,
}

impl Out {
    /// A mesh into the buffer, its points turned into glTF's frame and written once, its triangles in a primitive per
    /// colour: the body's own `colour` first, then one per colour its faces have of their own (`tri_colors`, a colour or
    /// none per triangle; empty where the body is one colour). A primitive of no colour has no material. Its number among
    /// the meshes.
    fn mesh(&mut self, m: &Mesh, colour: Option<[u8; 3]>, tri_colors: &[Option<[u8; 3]>]) -> usize {
        let pts: Vec<[f32; 3]> = m.verts.iter().map(outward).collect();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let at = self.bin.len();
        for p in &pts {
            for c in 0..3 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
                self.bin.extend_from_slice(&p[c].to_le_bytes());
            }
        }
        self.views.push(serde_json::json!({"buffer": 0, "byteOffset": at, "byteLength": self.bin.len() - at, "target": 34962}));
        self.accessors.push(serde_json::json!({"bufferView": self.views.len() - 1, "componentType": 5126, "count": pts.len(), "type": "VEC3", "min": lo, "max": hi}));
        let position = self.accessors.len() - 1;
        // the triangles by colour, the body's own first
        let mut groups: Vec<ColourGroup> = vec![ColourGroup { colour, tris: Vec::new() }];
        let coloured = tri_colors.len() == m.tris.len();
        for (k, t) in m.tris.iter().enumerate() {
            let c = if coloured { tri_colors[k] } else { colour };
            match groups.iter_mut().find(|g| g.colour == c) {
                Some(g) => g.tris.push(*t),
                None => groups.push(ColourGroup { colour: c, tris: vec![*t] }),
            }
        }
        let mut prims = Vec::new();
        for ColourGroup { colour: c, tris: ts } in groups.into_iter().filter(|g| !g.tris.is_empty()) {
            let at = self.bin.len();
            for t in &ts {
                for i in t {
                    self.bin.extend_from_slice(&i.to_le_bytes());
                }
            }
            self.views.push(serde_json::json!({"buffer": 0, "byteOffset": at, "byteLength": self.bin.len() - at, "target": 34963}));
            self.accessors.push(serde_json::json!({"bufferView": self.views.len() - 1, "componentType": 5125, "count": ts.len() * 3, "type": "SCALAR"}));
            let mut prim = serde_json::json!({"attributes": {"POSITION": position}, "indices": self.accessors.len() - 1, "mode": 4});
            if let Some(c) = c {
                prim["material"] = serde_json::json!(self.material(c));
            }
            prims.push(prim);
        }
        self.meshes.push(serde_json::json!({"primitives": prims}));
        self.meshes.len() - 1
    }

    /// The material of a colour, one per colour. The colour is sRGB, as a colour picker shows it, and glTF holds
    /// `baseColorFactor` in linear light; not metallic, where glTF's default would be.
    fn material(&mut self, rgb: [u8; 3]) -> usize {
        let linear = rgb.map(|b| {
            let c = b as f64 / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        let m = serde_json::json!({"pbrMetallicRoughness": {"baseColorFactor": [linear[0], linear[1], linear[2], 1.0], "metallicFactor": 0.0}});
        match self.materials.iter().position(|x| *x == m) {
            Some(k) => k,
            None => {
                self.materials.push(m);
                self.materials.len() - 1
            }
        }
    }

    /// The file: the scene over `roots`, the nodes, and the one buffer they all point into.
    fn write(mut self, nodes: Vec<Value>, roots: Vec<usize>, path: &str) -> Result<(), String> {
        let mut doc = serde_json::json!({
            "asset": {"version": "2.0", "generator": "QymCAD"},
            "scene": 0,
            "scenes": [{"nodes": roots}],
            "nodes": nodes, "meshes": self.meshes, "accessors": self.accessors, "bufferViews": self.views,
            "buffers": [{"byteLength": self.bin.len()}],
        });
        if !self.materials.is_empty() {
            doc["materials"] = Value::Array(self.materials);
        }
        let mut json = serde_json::to_vec(&doc).map_err(|e| format!("io-gltf-write-failed#{e}"))?;
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let total = 12 + 8 + json.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        out.extend_from_slice(&self.bin);
        std::fs::write(path, out).map_err(|e| format!("io-gltf-write-failed#{e}"))
    }
}
