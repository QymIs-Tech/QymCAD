//! AMF: the XML file for printing that came before 3MF - objects with vertices and volumes of triangles, a unit,
//! and constellations that place objects. A file may also come zipped, holding one AMF inside.
//!
//! Read and written here, by the same hand as 3MF: the XML goes through the same small reader (`crate::xml`), and
//! the unit is the file's word the same way.
use std::io::Read;

use qymcad_core::geom::{Mesh, Point3};

use crate::xml::Node;

use crate::NamedMesh;

/// The length of the unit an AMF names, in millimetres.
fn unit_mm(unit: &str) -> Option<f64> {
    crate::FileUnit::of_word(unit).map(crate::FileUnit::mm)
}

/// A NUMBER IS FINITE: of the elements `names` under `n`, the first written as `NaN` or `inf` is refused with its line
/// and its text.
fn finite_children(n: &Node, names: &[&str]) -> Result<(), String> {
    match names.iter().filter_map(|k| n.child(k)).find(|c| c.text.trim().parse::<f64>().is_ok_and(|v| !v.is_finite())) {
        Some(c) => Err(crate::not_finite("io-amf-not-finite-line", &[&c.line, &c.text.trim()])),
        None => Ok(()),
    }
}

/// Read an AMF file - plain, or zipped - into bodies in millimetres.
pub fn import_amf(path: &str) -> Result<Vec<NamedMesh>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-amf-read-failed#{e}"))?;
    let xml = if bytes.starts_with(b"PK") {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "io-amf-not-amf".to_string())?;
        let mut text = String::new();
        zip.by_index(0).map_err(|_| "io-amf-not-amf".to_string())?.read_to_string(&mut text).map_err(|_| "io-amf-not-amf".to_string())?;
        text
    } else {
        String::from_utf8(bytes).map_err(|_| "io-amf-not-amf".to_string())?
    };
    parse(&xml)
}

/// A rigid placement: a turn (3x3, row-major) then a shift.
#[derive(Clone, Copy)]
struct Place {
    r: [f64; 9],
    t: [f64; 3],
}

const HOME: Place = Place { r: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], t: [0.0; 3] };

impl Place {
    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|i| self.r[i * 3] * p[0] + self.r[i * 3 + 1] * p[1] + self.r[i * 3 + 2] * p[2] + self.t[i])
    }
    /// `inner` placed inside `self`.
    fn with(&self, inner: &Place) -> Place {
        let mut r = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                r[i * 3 + j] = (0..3).map(|k| self.r[i * 3 + k] * inner.r[k * 3 + j]).sum();
            }
        }
        Place { r, t: self.apply(inner.t) }
    }
    /// An instance: turned about X, then Y, then Z (degrees), then shifted.
    fn instance(d: [f64; 3], rot: [f64; 3]) -> Place {
        let (a, b, c) = (rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
        let rx = [1.0, 0.0, 0.0, 0.0, a.cos(), -a.sin(), 0.0, a.sin(), a.cos()];
        let ry = [b.cos(), 0.0, b.sin(), 0.0, 1.0, 0.0, -b.sin(), 0.0, b.cos()];
        let rz = [c.cos(), -c.sin(), 0.0, c.sin(), c.cos(), 0.0, 0.0, 0.0, 1.0];
        let m = |p: [f64; 9]| Place { r: p, t: [0.0; 3] };
        let turn = m(rz).with(&m(ry)).with(&m(rx));
        Place { r: turn.r, t: d }
    }
}

fn parse(xml: &str) -> Result<Vec<NamedMesh>, String> {
    let root = crate::xml::parse(xml).ok_or_else(|| "io-amf-not-amf".to_string())?;
    if root.name != "amf" {
        return Err("io-amf-not-amf".into());
    }
    let unit = root.attr("unit").unwrap_or("millimeter");
    let to_mm = unit_mm(unit).ok_or_else(|| format!("io-amf-unknown-unit#{unit}"))?;
    let number = |n: Option<&Node>| n.and_then(|n| n.text.trim().parse::<f64>().ok());
    // every object's own mesh, in its own frame and in the file's unit
    let mut objects: Vec<(String, String, Mesh)> = Vec::new();
    // the colour of each object, beside it: its own, or its first coloured volume's - the owner's AMF writes it there
    let mut colours: Vec<Option<[u8; 3]>> = Vec::new();
    for o in root.all("object") {
        colours.push(o.child("color").or_else(|| o.all("mesh").find_map(|m| m.all("volume").find_map(|v| v.child("color")))).and_then(|c| {
            let [Some(r), Some(g), Some(b)] = [number(c.child("r")), number(c.child("g")), number(c.child("b"))] else { return None };
            Some([r, g, b].map(crate::srgb_byte))
        }));
        let id = o.attr("id").unwrap_or("").to_string();
        let name = o.all("metadata").find(|m| m.attr("type") == Some("name")).map(|m| m.text.trim().to_string()).unwrap_or_default();
        let mut mesh = Mesh { verts: Vec::new(), tris: Vec::new() };
        for m in o.all("mesh") {
            let base = mesh.verts.len() as u32;
            let mut n = 0u32;
            for v in m.child("vertices").map(|vs| vs.all("vertex").collect::<Vec<_>>()).unwrap_or_default() {
                let c = v.child("coordinates").ok_or("io-amf-bad-vertex")?;
                let [Some(x), Some(y), Some(z)] = [number(c.child("x")), number(c.child("y")), number(c.child("z"))] else { return Err("io-amf-bad-vertex".into()) };
                finite_children(c, &["x", "y", "z"])?;
                mesh.verts.push(Point3::new(x, y, z));
                n += 1;
            }
            for vol in m.all("volume") {
                for t in vol.all("triangle") {
                    let [Some(a), Some(b), Some(c)] = [number(t.child("v1")), number(t.child("v2")), number(t.child("v3"))] else { return Err("io-amf-bad-triangle".into()) };
                    let idx = [a, b, c].map(|k| k as u32);
                    if idx.iter().any(|k| *k >= n) {
                        return Err("io-amf-bad-index".into());
                    }
                    mesh.tris.push(idx.map(|k| base + k));
                }
            }
        }
        objects.push((id, name, mesh));
    }
    let constellations: Vec<&Node> = root.all("constellation").collect();
    let place_mesh = |m: &Mesh, p: &Place| Mesh {
        verts: m
            .verts
            .iter()
            .map(|v| {
                let q = p.apply([v.x, v.y, v.z]);
                Point3::new(q[0] * to_mm, q[1] * to_mm, q[2] * to_mm)
            })
            .collect(),
        tris: m.tris.clone(),
    };
    // constellations nobody else instances are the top; each instance under them is a body
    let used: std::collections::HashSet<&str> = constellations.iter().flat_map(|c| c.all("instance").filter_map(|i| i.attr("objectid"))).collect();
    let tops: Vec<&&Node> = constellations.iter().filter(|c| c.attr("id").is_none_or(|id| !used.contains(id))).collect();
    let mut out: Vec<NamedMesh> = Vec::new();
    if tops.is_empty() {
        for ((_, name, m), colour) in objects.iter().zip(&colours) {
            if !m.tris.is_empty() {
                out.push(NamedMesh { name: name.clone(), mesh: place_mesh(m, &HOME), color: *colour, place: qymcad_core::feature::PLACE_IDENTITY, tri_colors: Vec::new(), within: Vec::new() });
            }
        }
    } else {
        // every constellation placed is a group at its instance's place, every object a piece in its own frame at its
        let mut tree = Tree { cons: &constellations, objects: &objects, next: 0 };
        let mut placed = Vec::new();
        for c in tops {
            tree.expand(c.attr("id").unwrap_or(""), &[], &HOME, 0, &mut placed)?;
        }
        for Instance { within, at: p, object: k } in placed {
            let (_, name, m) = &objects[k];
            let within = within.into_iter().map(|g| qymcad_core::model::FileGroup { index: g.index, name: g.name, place: placement(&g.at, to_mm) }).collect();
            out.push(NamedMesh { name: name.clone(), mesh: place_mesh(m, &HOME), color: colours[k], place: placement(&p, to_mm), tri_colors: Vec::new(), within });
        }
    }
    if out.iter().all(|b| b.mesh.tris.is_empty()) {
        return Err("io-amf-no-meshes".into());
    }
    Ok(out)
}

/// A group met on the walk through the constellations: its number, its name and its place in the group above it.
#[derive(Clone)]
struct Group {
    index: usize,
    name: String,
    at: Place,
}

/// An object as the walk places it: the groups it stands in, its place in the last of them, and its index.
struct Instance {
    within: Vec<Group>,
    at: Place,
    object: usize,
}

/// The constellations and objects of a file on a walk through them, and the number the next group takes.
struct Tree<'a> {
    cons: &'a [&'a Node],
    objects: &'a [(String, String, Mesh)],
    next: usize,
}

impl Tree<'_> {
    /// Every object `id` stands for, at any depth, into `out`: the groups it stands in, its place in the last of them
    /// and the index of the object. A constellation - the top one and every one instanced in another - is a group of
    /// its own at the place it is put, one per instance, so a unit placed twice is two groups.
    fn expand(&mut self, id: &str, within: &[Group], at: &Place, depth: usize, out: &mut Vec<Instance>) -> Result<(), String> {
        if depth > 32 {
            return Err("io-amf-bad-constellation".into());
        }
        if let Some(k) = self.objects.iter().position(|(oid, _, _)| oid == id) {
            out.push(Instance { within: within.to_vec(), at: *at, object: k });
            return Ok(());
        }
        let c = self.cons.iter().find(|c| c.attr("id") == Some(id)).ok_or_else(|| format!("io-amf-no-object#{id}"))?;
        let name = c.all("metadata").find(|m| m.attr("type") == Some("name")).map(|m| m.text.trim().to_string()).unwrap_or_default();
        let mut inner = within.to_vec();
        inner.push(Group { index: self.next, name, at: *at });
        self.next += 1;
        for i in c.all("instance") {
            finite_children(i, &["deltax", "deltay", "deltaz", "rx", "ry", "rz"])?;
            let num = |name: &str| i.child(name).and_then(|n| n.text.trim().parse::<f64>().ok()).unwrap_or(0.0);
            let local = Place::instance([num("deltax"), num("deltay"), num("deltaz")], [num("rx"), num("ry"), num("rz")]);
            self.expand(i.attr("objectid").ok_or("io-amf-bad-constellation")?, &inner, &local, depth + 1, out)?;
        }
        Ok(())
    }
}

/// A placement as the rest of the program takes one: 3x4 row-major, its shift in millimetres.
fn placement(p: &Place, to_mm: f64) -> [f64; 12] {
    let r = p.r;
    [r[0], r[1], r[2], p.t[0] * to_mm, r[3], r[4], r[5], p.t[1] * to_mm, r[6], r[7], r[8], p.t[2] * to_mm]
}

/// Write bodies into one plain AMF, in millimetres, one object per body. The numbers go out in their shortest
/// exact form, so the file read back gives the same coordinates to the last bit.
pub fn export_amf(meshes: &[Mesh], path: &str) -> Result<(), String> {
    if meshes.iter().all(|m| m.tris.is_empty()) {
        return Err("io-amf-no-triangles".into());
    }
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<amf unit=\"millimeter\" version=\"1.1\">\n");
    for (k, m) in meshes.iter().enumerate().filter(|(_, m)| !m.tris.is_empty()) {
        x.push_str(&format!(" <object id=\"{}\">\n  <metadata type=\"name\">body_{}</metadata>\n  <mesh>\n   <vertices>\n", k + 1, k + 1));
        for p in &m.verts {
            x.push_str(&format!("    <vertex><coordinates><x>{}</x><y>{}</y><z>{}</z></coordinates></vertex>\n", p.x, p.y, p.z));
        }
        x.push_str("   </vertices>\n   <volume>\n");
        for t in &m.tris {
            x.push_str(&format!("    <triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>\n", t[0], t[1], t[2]));
        }
        x.push_str("   </volume>\n  </mesh>\n </object>\n");
    }
    x.push_str("</amf>\n");
    std::fs::write(path, x).map_err(|e| format!("io-amf-write-failed#{e}"))
}
