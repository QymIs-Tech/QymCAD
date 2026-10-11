//! Importing STL, both ASCII and binary, into triangle meshes.
//!
//! Read here, byte by byte, rather than through a crate. The crate read an ASCII file's first solid and stopped at its
//! `endsolid`, so a file of several bodies came in as its first one; it told ASCII from binary by the first line, so a
//! binary file whose header begins with "solid" - several CADs write the part's name there - was taken for text
//! wherever that line read as text (ten triangles: the count's first byte is a newline) and did not read; and it kept
//! neither a solid's name nor the colours a binary file carries. A binary file is told by its length: 84 bytes, then
//! 50 a triangle.

use qymcad_core::geom::{Mesh, Point3};

use crate::NamedMesh;

/// Load an STL file into one mesh: every solid it holds, together.
pub fn import_stl(path: &str) -> Result<Mesh, String> {
    let mut out = Mesh::default();
    for p in import_stl_named(path)? {
        let base = out.verts.len() as u32;
        out.verts.extend(p.mesh.verts);
        out.tris.extend(p.mesh.tris.iter().map(|t| t.map(|i| base + i)));
    }
    Ok(out)
}

/// Read an STL file into its pieces: an ASCII file a piece per solid, under the solid's name; a binary file one piece,
/// in the colours it carries. A solid with no triangle is passed over.
pub fn import_stl_named(path: &str) -> Result<Vec<NamedMesh>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-stl-read-failed#{e}"))?;
    let counted = |b: &[u8]| b.len() >= 84 && (u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize).checked_mul(50).and_then(|n| n.checked_add(84)) == Some(b.len());
    let text = !counted(&bytes) && bytes.trim_ascii_start().starts_with(b"solid");
    let mut pieces: Vec<NamedMesh> = if text { ascii(&bytes)? } else { binary(&bytes)? }.into_iter().filter(|p| !p.mesh.tris.is_empty()).collect();
    if pieces.is_empty() {
        return Err("io-stl-no-faces".into());
    }
    // A LONE SOLID IS NAMED AFTER ITS FILE: the name on its line is most often the writer's stamp ("OpenSCAD_Model",
    // "Exported from Blender-..."), and the file's own name says more. A name is kept where it tells solids apart.
    if let [lone] = pieces.as_mut_slice() {
        lone.name.clear();
    }
    Ok(pieces)
}

/// Triangles welded where their corners meet: the same point, to the bit, is one vertex (-0 is 0).
#[derive(Default)]
struct Weld {
    at: std::collections::HashMap<[u32; 3], u32>,
    mesh: Mesh,
}

impl Weld {
    fn triangle(&mut self, corners: [[f32; 3]; 3]) {
        let t = corners.map(|p| {
            let key = p.map(|c| (c + 0.0).to_bits());
            *self.at.entry(key).or_insert_with(|| {
                self.mesh.verts.push(Point3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])));
                self.mesh.verts.len() as u32 - 1
            })
        });
        self.mesh.tris.push(t);
    }
}

fn piece(name: String, mesh: Mesh, color: Option<[u8; 3]>, tri_colors: Vec<[u8; 3]>) -> NamedMesh {
    NamedMesh { name: crate::authored(name), mesh, color, place: qymcad_core::feature::PLACE_IDENTITY, tri_colors, within: Vec::new() }
}

/// A text file: a piece per `solid`, named by the rest of its line. A facet is its corners - the normal the file
/// writes is taken again from them - and a loop of more than three corners is fanned from its first.
fn ascii(bytes: &[u8]) -> Result<Vec<NamedMesh>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    let mut solid: Option<(String, Weld)> = None;
    let mut corners: Vec<[f32; 3]> = Vec::new();
    for (no, line) in text.lines().map(str::trim).enumerate() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("solid") => {
                if let Some((name, weld)) = solid.take() {
                    out.push(piece(name, weld.mesh, None, Vec::new()));
                }
                solid = Some((line["solid".len()..].trim().to_string(), Weld::default()));
            }
            Some("endsolid") => {
                if let Some((name, weld)) = solid.take() {
                    out.push(piece(name, weld.mesh, None, Vec::new()));
                }
            }
            Some("vertex") => {
                let w = [words.next(), words.next(), words.next()];
                let [Some(x), Some(y), Some(z)] = w.map(|w| w.and_then(|w| w.parse::<f32>().ok())) else { return Err("io-stl-bad-facet".into()) };
                // A CORNER IS A FINITE NUMBER: `NaN` and `inf` parse as f32, and "1e39" overflows into an infinity
                if let Some(k) = [x, y, z].iter().position(|c| !c.is_finite()) {
                    return Err(crate::not_finite("io-stl-not-finite-line", &[&(no + 1), &w[k].unwrap_or_default()]));
                }
                corners.push([x, y, z]);
            }
            Some("endloop") => {
                if corners.len() < 3 {
                    return Err("io-stl-bad-facet".into());
                }
                let weld = &mut solid.get_or_insert_with(|| (String::new(), Weld::default())).1;
                for k in 1..corners.len() - 1 {
                    weld.triangle([corners[0], corners[k], corners[k + 1]]);
                }
                corners.clear();
            }
            _ => {}
        }
    }
    // a file that ends without its last `endsolid` keeps what it holds
    if let Some((name, weld)) = solid.take() {
        out.push(piece(name, weld.mesh, None, Vec::new()));
    }
    Ok(out)
}

/// A binary file: one piece, with the colours it carries where it carries them. Materialise writes the object's
/// colour after "COLOR=" in the header and gives a triangle one of its own with bit 15 of its attribute word clear,
/// red in the low bits; VisCAM and SolidView give a triangle a colour with bit 15 set, blue in the low bits. The
/// header says which. Five bits a channel are spread over the byte (31 is 255).
fn binary(b: &[u8]) -> Result<Vec<NamedMesh>, String> {
    if b.len() < 84 {
        return Err("io-stl-truncated".into());
    }
    let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
    if n.checked_mul(50).and_then(|x| x.checked_add(84)).is_none_or(|end| end > b.len()) {
        return Err("io-stl-truncated".into());
    }
    let head = &b[..80];
    let magics = head.windows(6).position(|w| w == b"COLOR=");
    let object = magics.filter(|&k| k + 10 <= 80).map(|k| [head[k + 6], head[k + 7], head[k + 8]]);
    let five = |v: u16| ((v << 3) | (v >> 2)) as u8;
    let mut weld = Weld::default();
    let mut per: Vec<Option<[u8; 3]>> = Vec::with_capacity(n);
    for k in 0..n {
        let f = &b[84 + 50 * k..84 + 50 * (k + 1)];
        let corner = |i: usize| [0, 1, 2].map(|c| f32::from_le_bytes([f[12 + 12 * i + 4 * c], f[13 + 12 * i + 4 * c], f[14 + 12 * i + 4 * c], f[15 + 12 * i + 4 * c]]));
        let corners = [corner(0), corner(1), corner(2)];
        // the same rule as the text: four bytes may hold a NaN or an infinity. Nothing in the file numbers its triangles,
        // so they are counted from 1, as a person counts them
        if let Some(j) = (0..9).find(|&j| !corners[j / 3][j % 3].is_finite()) {
            return Err(crate::not_finite("io-stl-not-finite-triangle", &[&(k + 1), &(84 + 50 * k + 12 + 4 * j), &corners[j / 3][j % 3]]));
        }
        weld.triangle(corners);
        let a = u16::from_le_bytes([f[48], f[49]]);
        let (low, mid, high) = (five(a & 31), five((a >> 5) & 31), five((a >> 10) & 31));
        per.push(if magics.is_some() { (a & 0x8000 == 0).then_some([low, mid, high]) } else { (a & 0x8000 != 0).then_some([high, mid, low]) });
    }
    // the piece's colour: the object's, or the one most of its coloured triangles have
    let mut count: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    for c in per.iter().flatten() {
        *count.entry(*c).or_default() += 1;
    }
    let most = per.iter().flatten().copied().max_by_key(|c| count[c]);
    let color = object.or(most);
    let tri_colors = crate::several_colours(&per, color);
    Ok(vec![piece(String::new(), weld.mesh, color, tri_colors)])
}
