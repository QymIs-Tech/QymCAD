//! PLY: the format of 3D scanners - a header that says what follows, then vertices and faces, in text or in binary.
//!
//! Of a vertex its x, y, z and colour are taken, of a face its list of corners and colour. Everything else a scanner
//! writes (normals, confidence) is read past, since its size is in the header. PLY names no unit, so its numbers are
//! taken as millimetres, as every mesh here is.
use qymcad_core::geom::{Mesh, Point3};

/// Read a PLY file - text, or binary in either byte order - into one mesh.
pub fn import_ply(path: &str) -> Result<Mesh, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-ply-read-failed#{e}"))?;
    parse(&bytes).map(|(mesh, _)| mesh)
}

/// Read a PLY file into one mesh with the colours its faces carry, or its vertices where the faces carry none: the mesh's
/// colour the one most of its triangles have, and a colour per triangle where they have more than one.
pub fn import_ply_coloured(path: &str) -> Result<crate::NamedMesh, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-ply-read-failed#{e}"))?;
    let (mesh, mut tri_colors) = parse(&bytes)?;
    // the colours in the order met, counted through a map: a scan has tens of thousands of them
    let mut place: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    let mut count: Vec<([u8; 3], usize)> = Vec::new();
    for c in &tri_colors {
        let k = *place.entry(*c).or_insert_with(|| {
            count.push((*c, 0));
            count.len() - 1
        });
        count[k].1 += 1;
    }
    let color = count.iter().max_by_key(|(_, n)| *n).map(|(c, _)| *c);
    if count.len() < 2 {
        tri_colors.clear(); // one colour is the mesh's own
    }
    Ok(crate::NamedMesh { name: String::new(), mesh, color, place: qymcad_core::feature::PLACE_IDENTITY, tri_colors, within: Vec::new() })
}

/// A colour channel as a byte: an integer is taken as it stands, a real number is a share of full.
fn channel(v: f64, s: Scalar) -> u8 {
    match s {
        Scalar::F32 | Scalar::F64 => (v.clamp(0.0, 1.0) * 255.0).round() as u8,
        _ => v.clamp(0.0, 255.0) as u8,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Text,
    Little,
    Big,
}

/// A scalar type of PLY, by the width it takes.
#[derive(Clone, Copy)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Scalar {
    fn of(name: &str) -> Option<Scalar> {
        Some(match name {
            "char" | "int8" => Scalar::I8,
            "uchar" | "uint8" => Scalar::U8,
            "short" | "int16" => Scalar::I16,
            "ushort" | "uint16" => Scalar::U16,
            "int" | "int32" => Scalar::I32,
            "uint" | "uint32" => Scalar::U32,
            "float" | "float32" => Scalar::F32,
            "double" | "float64" => Scalar::F64,
            _ => return None,
        })
    }
    fn width(self) -> usize {
        match self {
            Scalar::I8 | Scalar::U8 => 1,
            Scalar::I16 | Scalar::U16 => 2,
            Scalar::I32 | Scalar::U32 | Scalar::F32 => 4,
            Scalar::F64 => 8,
        }
    }
}

/// A property: a scalar, or a list of scalars preceded by its count.
#[derive(Clone, Copy)]
enum Prop {
    One(Scalar),
    List { count: Scalar, item: Scalar },
}

struct Element {
    name: String,
    count: usize,
    props: Vec<(String, Prop)>,
}

/// Reads values off the body one by one, whatever the encoding.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    enc: Encoding,
}

impl Cursor<'_> {
    fn word(&mut self) -> Option<&str> {
        while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
        let start = self.at;
        while self.at < self.bytes.len() && !self.bytes[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
        (self.at > start).then(|| std::str::from_utf8(&self.bytes[start..self.at]).ok()).flatten()
    }

    /// A COORDINATE IS A FINITE NUMBER: text parses `nan` and `inf`, binary floats carry them. One that is not is refused
    /// with where it stands - its line in a text file, its vertex and byte in a binary one - and what it is.
    fn coordinate(&mut self, s: Scalar, vertex: usize) -> Result<f64, String> {
        let start = self.at;
        let v = self.value(s).ok_or_else(|| "io-ply-truncated".to_string())?;
        if v.is_finite() {
            return Ok(v);
        }
        if matches!(self.enc, Encoding::Text) {
            let word_start = start + self.bytes[start..self.at].iter().take_while(|b| b.is_ascii_whitespace()).count();
            let line = 1 + self.bytes[..word_start].iter().filter(|&&b| b == b'\n').count();
            let word = String::from_utf8_lossy(&self.bytes[word_start..self.at]).into_owned();
            return Err(crate::not_finite("io-ply-not-finite-line", &[&line, &word]));
        }
        Err(crate::not_finite("io-ply-not-finite-vertex", &[&vertex, &start, &v]))
    }

    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let b: [u8; N] = self.bytes.get(self.at..self.at + N)?.try_into().ok()?;
        self.at += N;
        Some(b)
    }

    fn value(&mut self, s: Scalar) -> Option<f64> {
        if self.enc == Encoding::Text {
            return self.word()?.parse().ok();
        }
        let big = self.enc == Encoding::Big;
        macro_rules! read {
            ($t:ty, $n:literal) => {{
                let b = self.take::<$n>()?;
                (if big { <$t>::from_be_bytes(b) } else { <$t>::from_le_bytes(b) }) as f64
            }};
        }
        Some(match s {
            Scalar::I8 => read!(i8, 1),
            Scalar::U8 => read!(u8, 1),
            Scalar::I16 => read!(i16, 2),
            Scalar::U16 => read!(u16, 2),
            Scalar::I32 => read!(i32, 4),
            Scalar::U32 => read!(u32, 4),
            Scalar::F32 => read!(f32, 4),
            Scalar::F64 => read!(f64, 8),
        })
    }

    fn skip(&mut self, p: Prop) -> Option<()> {
        match p {
            Prop::One(s) => self.value(s).map(|_| ()),
            Prop::List { count, item } => {
                let n = self.value(count)? as usize;
                if self.enc == Encoding::Text {
                    for _ in 0..n {
                        self.word()?;
                    }
                } else {
                    self.at = self.at.checked_add(n * item.width()).filter(|e| *e <= self.bytes.len())?;
                }
                Some(())
            }
        }
    }
}

/// The mesh, and the colour of every triangle where the file colours its faces or its vertices (empty where it does
/// neither).
fn parse(bytes: &[u8]) -> Result<(Mesh, Vec<[u8; 3]>), String> {
    // THE HEADER is text up to `end_header`, whatever the body is
    let end = bytes.windows(10).position(|w| w == b"end_header").ok_or_else(|| "io-ply-not-ply".to_string())?;
    let head = std::str::from_utf8(&bytes[..end]).map_err(|_| "io-ply-not-ply".to_string())?;
    let mut lines = head.lines().map(str::trim);
    if lines.next() != Some("ply") {
        return Err("io-ply-not-ply".into());
    }
    let mut enc = None;
    let mut elements: Vec<Element> = Vec::new();
    for line in lines {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["format", "ascii", ..] => enc = Some(Encoding::Text),
            ["format", "binary_little_endian", ..] => enc = Some(Encoding::Little),
            ["format", "binary_big_endian", ..] => enc = Some(Encoding::Big),
            ["element", name, n] => elements.push(Element { name: name.to_string(), count: n.parse().map_err(|_| "io-ply-bad-header".to_string())?, props: Vec::new() }),
            ["property", "list", c, i, name] => {
                let (count, item) = (Scalar::of(c).ok_or("io-ply-bad-header")?, Scalar::of(i).ok_or("io-ply-bad-header")?);
                elements.last_mut().ok_or("io-ply-bad-header")?.props.push((name.to_string(), Prop::List { count, item }));
            }
            ["property", t, name] => {
                let s = Scalar::of(t).ok_or("io-ply-bad-header")?;
                elements.last_mut().ok_or("io-ply-bad-header")?.props.push((name.to_string(), Prop::One(s)));
            }
            _ => {} // comments, obj_info
        }
    }
    let enc = enc.ok_or_else(|| "io-ply-bad-header".to_string())?;
    // the body starts after the line that ends the header
    let mut at = end + 10;
    while at < bytes.len() && bytes[at] != b'\n' {
        at += 1;
    }
    let mut cur = Cursor { bytes, at: at + 1, enc };
    let truncated = || "io-ply-truncated".to_string();
    let mut verts: Vec<Point3> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut tri_rgb: Vec<[u8; 3]> = Vec::new();
    let mut vert_rgb: Vec<Option<[u8; 3]>> = Vec::new();
    for el in &elements {
        for i in 0..el.count {
            match el.name.as_str() {
                "vertex" => {
                    let (mut xyz, mut rgb) = ([0.0f64; 3], [None; 3]);
                    for (name, p) in &el.props {
                        match (name.as_str(), p) {
                            ("x", Prop::One(s)) => xyz[0] = cur.coordinate(*s, i)?,
                            ("y", Prop::One(s)) => xyz[1] = cur.coordinate(*s, i)?,
                            ("z", Prop::One(s)) => xyz[2] = cur.coordinate(*s, i)?,
                            ("red" | "diffuse_red", Prop::One(s)) => rgb[0] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            ("green" | "diffuse_green", Prop::One(s)) => rgb[1] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            ("blue" | "diffuse_blue", Prop::One(s)) => rgb[2] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            _ => cur.skip(*p).ok_or_else(truncated)?,
                        }
                    }
                    verts.push(Point3::new(xyz[0], xyz[1], xyz[2]));
                    vert_rgb.push(if let [Some(r), Some(g), Some(b)] = rgb { Some([r, g, b]) } else { None });
                }
                "face" => {
                    // the face's colour, where it carries one: on every triangle it fans into
                    let (from, mut rgb) = (tris.len(), [None; 3]);
                    for (name, p) in &el.props {
                        match (name.as_str(), p) {
                            ("vertex_indices" | "vertex_index", Prop::List { count, item }) => {
                                let n = cur.value(*count).ok_or_else(truncated)? as usize;
                                let mut corners = Vec::with_capacity(n);
                                for _ in 0..n {
                                    corners.push(cur.value(*item).ok_or_else(truncated)?);
                                }
                                for k in 1..n.saturating_sub(1) {
                                    tris.push([corners[0] as u32, corners[k] as u32, corners[k + 1] as u32]);
                                }
                            }
                            ("red" | "diffuse_red", Prop::One(s)) => rgb[0] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            ("green" | "diffuse_green", Prop::One(s)) => rgb[1] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            ("blue" | "diffuse_blue", Prop::One(s)) => rgb[2] = Some(channel(cur.value(*s).ok_or_else(truncated)?, *s)),
                            _ => cur.skip(*p).ok_or_else(truncated)?,
                        }
                    }
                    if let [Some(r), Some(g), Some(b)] = rgb {
                        tri_rgb.extend(std::iter::repeat_n([r, g, b], tris.len() - from));
                    }
                }
                _ => {
                    for (_, p) in &el.props {
                        cur.skip(*p).ok_or_else(truncated)?;
                    }
                }
            }
        }
    }
    if tris.iter().flatten().any(|i| *i as usize >= verts.len()) {
        return Err("io-ply-bad-index".into());
    }
    if tris.is_empty() {
        return Err("io-ply-no-faces".into());
    }
    if tri_rgb.len() != tris.len() {
        tri_rgb.clear(); // colours on some faces only are no colours of the mesh
                         // the vertices' colours, where every vertex has one: a triangle here is of one colour, so it takes the colour most
                         // of its corners have, its first corner's where all three differ - never a blend no vertex has
        if vert_rgb.iter().all(Option::is_some) {
            let c = |i: u32| vert_rgb[i as usize].unwrap_or_default();
            tri_rgb = tris.iter().map(|t| if c(t[1]) == c(t[2]) { c(t[1]) } else { c(t[0]) }).collect();
        }
    }
    Ok((Mesh { verts, tris }, tri_rgb))
}

/// Write meshes into one binary PLY: the coordinates as doubles, so the file read back gives them to the last bit;
/// faces as triangles. PLY holds one mesh, so several bodies go in together.
pub fn export_ply(meshes: &[Mesh], path: &str) -> Result<(), String> {
    let nv: usize = meshes.iter().map(|m| m.verts.len()).sum();
    let nf: usize = meshes.iter().map(|m| m.tris.len()).sum();
    if nf == 0 {
        return Err("io-ply-no-triangles".into());
    }
    let mut out = format!(
        "ply\nformat binary_little_endian 1.0\ncomment QymCAD\nelement vertex {nv}\nproperty double x\nproperty double y\nproperty double z\nelement face {nf}\nproperty list uchar uint vertex_indices\nend_header\n"
    )
    .into_bytes();
    out.reserve(nv * 24 + nf * 13);
    for m in meshes {
        for p in &m.verts {
            for c in [p.x, p.y, p.z] {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    let mut base = 0u32;
    for m in meshes {
        for t in &m.tris {
            out.push(3);
            for i in t {
                out.extend_from_slice(&(i + base).to_le_bytes());
            }
        }
        base += m.verts.len() as u32;
    }
    std::fs::write(path, out).map_err(|e| format!("io-ply-write-failed#{e}"))
}
