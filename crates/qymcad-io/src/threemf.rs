//! 3MF: the modern file for printing - a zip holding an XML model: meshes, units, and a build of placed objects.
//!
//! Read and written here; OCCT has no 3MF. The model part is found through the package's relationships, as the
//! standard says, and falls back to its usual name. The unit is the file's word (`unit`, millimetre by
//! default), which is what STL could never carry: a model drawn in inches arrives in inches no longer.
use std::io::{Read, Write};

use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::{ExportMesh, ExportNode, Id};

use crate::NamedMesh;

const MODEL_REL: &str = "http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel";
const CORE_NS: &str = "http://schemas.microsoft.com/3dmanufacturing/core/2015/02";

use crate::xml::Node;

/// The length of the unit a model names, in millimetres.
fn unit_mm(unit: &str) -> Option<f64> {
    crate::FileUnit::of_word(unit).map(crate::FileUnit::mm)
}

/// A 3MF transform: twelve numbers, a 3x3 then the translation, applied to a row vector.
type T = [f64; 12];
const ONE: T = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

fn parse_t(n: &Node) -> Result<T, String> {
    let Some(s) = n.attr("transform") else { return Ok(ONE) };
    let v: Vec<f64> = s.split_whitespace().map(|w| w.parse::<f64>()).collect::<Result<_, _>>().map_err(|_| "io-3mf-bad-transform".to_string())?;
    // a placement is a finite number, as a corner is
    if let Some(k) = v.iter().position(|x| !x.is_finite()) {
        return Err(crate::not_finite("io-3mf-not-finite-line", &[&n.line, &s.split_whitespace().nth(k).unwrap_or_default()]));
    }
    <[f64; 12]>::try_from(v).map_err(|_| "io-3mf-bad-transform".to_string())
}

fn apply(t: &T, p: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|c| p[0] * t[c] + p[1] * t[3 + c] + p[2] * t[6 + c] + t[9 + c])
}

/// `a` then `b`, both applied to row vectors.
/// A 3MF transform (row vectors) as a placement of ours (3x4 row-major, column vectors), its shift in millimetres.
fn placement(t: &T, to_mm: f64) -> [f64; 12] {
    [t[0], t[3], t[6], t[9] * to_mm, t[1], t[4], t[7], t[10] * to_mm, t[2], t[5], t[8], t[11] * to_mm]
}

/// The linear part of a 3MF transform as `rigid` takes it: column vectors, row by row.
fn col(t: &T) -> crate::rigid::Lin {
    std::array::from_fn(|r| std::array::from_fn(|c| t[c * 3 + r]))
}

/// A linear map of `rigid`'s and a shift, as a 3MF transform.
fn row(l: &crate::rigid::Lin, shift: [f64; 3]) -> T {
    let mut t = ONE;
    for r in 0..3 {
        for c in 0..3 {
            t[c * 3 + r] = l[r][c];
        }
    }
    t[9..].copy_from_slice(&shift);
    t
}

fn then(a: &T, b: &T) -> T {
    let mut r = [0.0; 12];
    for row in 0..4 {
        let src = if row < 3 { [a[row * 3], a[row * 3 + 1], a[row * 3 + 2]] } else { [a[9], a[10], a[11]] };
        for c in 0..3 {
            let mut s = src[0] * b[c] + src[1] * b[3 + c] + src[2] * b[6 + c];
            if row == 3 {
                s += b[9 + c];
            }
            r[row * 3 + c] = s;
        }
    }
    r
}

/// Read a 3MF package into bodies, one per build item, in millimetres.
pub fn import_3mf(path: &str) -> Result<Vec<NamedMesh>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("io-3mf-read-failed#{e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "io-3mf-not-3mf".to_string())?;
    let mut read = |name: &str| -> Option<String> {
        let mut s = String::new();
        zip.by_name(name.trim_start_matches('/')).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    // the model part, as the package's relationships name it
    let target = read("_rels/.rels")
        .and_then(|rels| {
            // a relationships part that cannot be read - malformed or nested too deep - is passed over, and the model is
            // looked for where it usually lies; the model part itself is refused with its reason
            let doc = crate::xml::parse(&rels).ok()?;
            doc.descendants().into_iter().find(|n| n.name == "Relationship" && n.attr("Type") == Some(MODEL_REL)).and_then(|n| n.attr("Target")).map(str::to_string)
        })
        .unwrap_or_else(|| "3D/3dmodel.model".to_string());
    let xml = read(&target).ok_or_else(|| "io-3mf-no-model".to_string())?;
    let names = read("Metadata/model_settings.config").map(|c| part_names(&c)).unwrap_or_default();
    parse_model(&xml, &names)
}

/// THE NAMES A SLICER'S PROJECT GIVES THE PARTS of its objects (`Metadata/model_settings.config`, as Bambu Studio and
/// its kin write it): a part by the id of the object holding its mesh. The owner's print head came with its 107 part
/// names there, and none in the model.
fn part_names(config: &str) -> std::collections::HashMap<String, String> {
    let Ok(doc) = crate::xml::parse(config) else { return Default::default() };
    doc.descendants()
        .into_iter()
        .filter(|n| n.name == "part")
        .filter_map(|p| Some((p.attr("id")?.to_string(), p.all("metadata").find(|m| m.attr("key") == Some("name")).and_then(|m| m.attr("value"))?.to_string())))
        .collect()
}

fn parse_model(xml: &str, names: &std::collections::HashMap<String, String>) -> Result<Vec<NamedMesh>, String> {
    let model = crate::xml::parse(xml).map_err(|e| e.key("io-3mf-bad-model"))?;
    let to_mm = unit_mm(model.attr("unit").unwrap_or("millimeter")).ok_or_else(|| format!("io-3mf-unknown-unit#{}", model.attr("unit").unwrap_or("")))?;
    // every object: a mesh, or a list of placed components
    let everything = model.descendants();
    let objects: std::collections::HashMap<&str, &Node> = everything.iter().filter(|n| n.name == "object").filter_map(|o| o.attr("id").map(|id| (id, *o))).collect();
    let colours = colours(&everything);
    let colour = |id: &str| objects.get(id).and_then(|o| colour_of(o, &colours));
    let parts = Parts { objects: &objects, colours: &colours, names, to_mm, groups: std::cell::Cell::new(0) };
    let mut out = Vec::new();
    let scale: T = [to_mm, 0.0, 0.0, 0.0, to_mm, 0.0, 0.0, 0.0, to_mm, 0.0, 0.0, 0.0];
    for item in everything.iter().filter(|n| n.name == "item") {
        let id = item.attr("objectid").ok_or("io-3mf-bad-model")?;
        let t = then(&parse_t(item)?, &scale);
        // AN OBJECT OF NAMED PARTS comes in a piece per part: the project names them, so they are the author's pieces
        // and not one body. Components nobody names stay one piece of their item, as before.
        let components: Vec<&Node> = objects.get(id).and_then(|o| o.child("components")).map(|cs| cs.all("component").collect()).unwrap_or_default();
        if components.iter().any(|c| c.attr("objectid").is_some_and(|p| parts.named(p))) {
            // each part in its own coordinates; where it stands is its component's transform, then the item's - so the
            // part can be moved and mated as a part, not a mesh baked into place
            let item_t = parse_t(item)?;
            for c in components {
                parts.part(c.attr("objectid").ok_or("io-3mf-bad-model")?, &then(&parse_t(c)?, &item_t), &[], &mut out)?;
            }
            continue;
        }
        let mut mesh = Mesh { verts: Vec::new(), tris: Vec::new() };
        let mut per = Vec::new();
        place(id, &t, 0, &objects, &colours, &mut mesh, &mut per)?;
        if !mesh.tris.is_empty() {
            let name = objects.get(id).and_then(|o| o.attr("name")).unwrap_or("").to_string();
            let color = colour(id);
            let tri_colors = crate::several_colours(&per, color);
            out.push(NamedMesh { name, mesh, color, place: qymcad_core::feature::PLACE_IDENTITY, tri_colors, within: Vec::new() });
        }
    }
    if out.is_empty() {
        return Err("io-3mf-no-meshes".into());
    }
    Ok(out)
}

/// What the parts of a model are read with: its objects and colours, the names a slicer's project gives them, its
/// unit, and a count of the groups made - each placing of an object of parts a group of its own.
struct Parts<'a> {
    objects: &'a std::collections::HashMap<&'a str, &'a Node>,
    colours: &'a Colours,
    names: &'a std::collections::HashMap<String, String>,
    to_mm: f64,
    groups: std::cell::Cell<usize>,
}

impl Parts<'_> {
    /// Whether object `id` is a part someone named: the slicer's project or the model itself.
    fn named(&self, id: &str) -> bool {
        self.names.contains_key(id) || self.objects.get(id).and_then(|o| o.attr("name")).is_some_and(|n| !n.trim().is_empty())
    }

    fn name(&self, id: &str) -> String {
        self.names.get(id).cloned().or_else(|| self.objects.get(id).and_then(|o| o.attr("name")).map(str::to_string)).unwrap_or_default()
    }

    /// A PART STANDING AT `t` IN ITS GROUP: a group of its own parts where its object is made of parts someone named,
    /// to any depth, and otherwise one piece with its components baked in. Its place holds the turn and the shift of
    /// `t`; what `t` scales, shears or mirrors is baked into its mesh, or handed down to its own parts.
    fn part(&self, id: &str, t: &T, within: &[qymcad_core::model::FileGroup], out: &mut Vec<NamedMesh>) -> Result<(), String> {
        if within.len() > 32 {
            return Err("io-3mf-bad-model".into());
        }
        let lin = col(t);
        // a transform that flattens space keeps its shift and bakes the rest as it is
        let (turn, rest) = crate::rigid::split(&lin).unwrap_or((crate::rigid::ONE, lin));
        let at = placement(&row(&turn, [t[9], t[10], t[11]]), self.to_mm);
        let components: Vec<&Node> = self.objects.get(id).and_then(|o| o.child("components")).map(|cs| cs.all("component").collect()).unwrap_or_default();
        if components.iter().any(|c| c.attr("objectid").is_some_and(|p| self.named(p))) {
            let mut w = within.to_vec();
            w.push(qymcad_core::model::FileGroup { index: self.groups.replace(self.groups.get() + 1), name: self.name(id), place: at });
            let left = row(&rest, [0.0; 3]);
            for c in components {
                self.part(c.attr("objectid").ok_or("io-3mf-bad-model")?, &then(&parse_t(c)?, &left), &w, out)?;
            }
            return Ok(());
        }
        let mut mesh = Mesh { verts: Vec::new(), tris: Vec::new() };
        let mut per = Vec::new();
        let in_mm = rest.map(|r| r.map(|v| v * self.to_mm));
        place(id, &row(&in_mm, [0.0; 3]), 1, self.objects, self.colours, &mut mesh, &mut per)?;
        if !mesh.tris.is_empty() {
            let color = self.objects.get(id).and_then(|o| colour_of(o, self.colours));
            let tri_colors = crate::several_colours(&per, color);
            out.push(NamedMesh { name: self.name(id), mesh, color, place: at, tri_colors, within: within.to_vec() });
        }
        Ok(())
    }
}

/// The colours a model names, by the id of their resource: the core's `basematerials` (`displaycolor`) and the colour
/// groups of the materials extension (`color`), each sRGB, `#RRGGBB` with an optional alpha.
/// The colours of a model's resources, by id: see `colours`.
type Colours = std::collections::HashMap<String, Vec<Option<[u8; 3]>>>;

fn colours(everything: &[&Node]) -> Colours {
    everything
        .iter()
        .filter_map(|n| match n.name.as_str() {
            "basematerials" => Some((n.attr("id")?.to_string(), n.all("base").map(|b| b.attr("displaycolor").and_then(hex)).collect())),
            "colorgroup" => Some((n.attr("id")?.to_string(), n.all("color").map(|c| c.attr("color").and_then(hex)).collect())),
            _ => None,
        })
        .collect()
}

fn hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6 || h.len() == 8)?;
    let byte = |k: usize| h.get(k..k + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// The colour of object `o`: the one its `pid` and `pindex` name or, where it names none, its first triangle's.
fn colour_of(o: &Node, colours: &std::collections::HashMap<String, Vec<Option<[u8; 3]>>>) -> Option<[u8; 3]> {
    let (pid, k) = match o.attr("pid") {
        Some(pid) => (pid, o.attr("pindex").and_then(|k| k.parse().ok()).unwrap_or(0usize)),
        None => {
            let t = o.child("mesh")?.child("triangles")?.all("triangle").next()?;
            (t.attr("pid")?, t.attr("p1").and_then(|k| k.parse().ok()).unwrap_or(0usize))
        }
    };
    *colours.get(pid)?.get(k)?
}

fn num(n: &Node, a: &str) -> Result<f64, String> {
    let word = n.attr(a).ok_or_else(|| "io-3mf-bad-model".to_string())?;
    let v: f64 = word.parse().map_err(|_| "io-3mf-bad-model".to_string())?;
    // A NUMBER IS FINITE: `NaN` and `inf` parse as f64
    if !v.is_finite() {
        return Err(crate::not_finite("io-3mf-not-finite-line", &[&n.line, &word]));
    }
    Ok(v)
}

/// Place object `id` under `t` into `mesh`: its own triangles, then its components, each under its transform.
fn place(id: &str, t: &T, depth: usize, objects: &std::collections::HashMap<&str, &Node>, colours: &Colours, mesh: &mut Mesh, per: &mut Vec<Option<[u8; 3]>>) -> Result<(), String> {
    let o = objects.get(id).ok_or_else(|| format!("io-3mf-no-object#{id}"))?;
    // the colour the object names for its triangles, where a triangle names none of its own (`pid` with `p1`)
    let own = o.attr("pid").and_then(|p| colours.get(p)?.get(o.attr("pindex").and_then(|k| k.parse().ok()).unwrap_or(0usize)).copied().flatten());
    if depth > 32 {
        return Err("io-3mf-bad-model".into());
    }
    // a mirror turns the triangles over: a transform with a negative determinant "MUST NOT change the sign of its
    // volume" (3MF Core, 3.3)
    let mirrored = crate::rigid::det(&col(t)) < 0.0;
    if let Some(m) = o.child("mesh") {
        let base = mesh.verts.len() as u32;
        let verts: Vec<[f64; 3]> = m.child("vertices").map(|v| v.all("vertex").map(|v| Ok([num(v, "x")?, num(v, "y")?, num(v, "z")?])).collect::<Result<_, String>>()).transpose()?.unwrap_or_default();
        let n = verts.len() as u32;
        for v in verts {
            let p = apply(t, v);
            mesh.verts.push(Point3::new(p[0], p[1], p[2]));
        }
        for tr in m.child("triangles").map(|t| t.all("triangle").collect::<Vec<_>>()).unwrap_or_default() {
            let i = [num(tr, "v1")?, num(tr, "v2")?, num(tr, "v3")?].map(|x| x as u32);
            if i.iter().any(|k| *k >= n) {
                return Err("io-3mf-bad-index".into());
            }
            mesh.tris.push(if mirrored { [i[0], i[2], i[1]] } else { i }.map(|k| base + k));
            per.push(tr.attr("p1").and_then(|k| k.parse::<usize>().ok()).and_then(|k| colours.get(tr.attr("pid").or(o.attr("pid"))?)?.get(k).copied().flatten()).or(own));
        }
    }
    if let Some(cs) = o.child("components") {
        for c in cs.all("component") {
            let own = parse_t(c)?;
            place(c.attr("objectid").ok_or("io-3mf-bad-model")?, &then(&own, t), depth + 1, objects, colours, mesh, per)?;
        }
    }
    Ok(())
}

/// Write bodies into a 3MF package, in millimetres, one object and one build item per body. The numbers go out
/// in their shortest exact form, so the package read back gives the same coordinates to the last bit.
pub fn export_3mf(meshes: &[Mesh], path: &str) -> Result<(), String> {
    if meshes.iter().all(|m| m.tris.is_empty()) {
        return Err("io-3mf-no-triangles".into());
    }
    let mut model = model_head();
    let mut build = String::new();
    for (k, m) in meshes.iter().enumerate().filter(|(_, m)| !m.tris.is_empty()) {
        let id = k + 1;
        mesh_object(&mut model, id, &format!("body_{id}"), None, m, &[]);
        build.push_str(&format!("  <item objectid=\"{id}\"/>\n"));
    }
    model.push_str(&format!(" </resources>\n <build>\n{build} </build>\n</model>\n"));
    write_package(path, &model)
}

/// WRITE A TREE WITH ITS SUBASSEMBLIES, as 3MF holds them: an object per part under its name and in its colour, its mesh
/// in the part's own coordinates; a subassembly an object of components under its name, each component placing a part
/// or a subassembly where it stands in it (3MF Core, 4.1 - a component may refer to an object of components). A repeat
/// (`same_as`) places the object of what it repeats, which the file holds once. Every object is written before the one
/// that refers to it.
pub fn export_3mf_tree(nodes: &[ExportNode], meshes: &[ExportMesh], path: &str) -> Result<(), String> {
    let mut palette: Vec<[u8; 3]> = Vec::new();
    for c in nodes.iter().filter_map(|n| n.color).chain(meshes.iter().flat_map(|m| m.tri_colors.iter().flatten().copied())) {
        if !palette.contains(&c) {
            palette.push(c);
        }
    }
    let mut model = model_head();
    if !palette.is_empty() {
        model.push_str("  <basematerials id=\"1\">\n");
        for c in &palette {
            let hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
            model.push_str(&format!("   <base name=\"{hex}\" displaycolor=\"{hex}\"/>\n"));
        }
        model.push_str("  </basematerials>\n");
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if let Some(p) = n.parent.filter(|&p| p < i) {
            children[p].push(i);
        }
    }
    let mut w = TreeWriter { nodes, children, by_body: meshes.iter().map(|m| (m.body, m)).collect(), palette, model, next: 2, object: vec![None; nodes.len()] };
    let mut build = String::new();
    for root in (0..nodes.len()).filter(|&i| nodes[i].parent.is_none_or(|p| p >= i)) {
        if let Some(id) = w.emit(root) {
            build.push_str(&format!("  <item objectid=\"{id}\"/>\n"));
        }
    }
    if build.is_empty() {
        return Err("io-3mf-no-triangles".into());
    }
    let mut model = w.model;
    model.push_str(&format!(" </resources>\n <build>\n{build} </build>\n</model>\n"));
    write_package(path, &model)
}

/// The objects of a tree being written, each made once and remembered by its node.
struct TreeWriter<'a> {
    nodes: &'a [ExportNode],
    children: Vec<Vec<usize>>,
    by_body: std::collections::HashMap<Id, &'a ExportMesh>,
    palette: Vec<[u8; 3]>,
    model: String,
    next: usize,
    /// for each node, once made: the object that stands for it, or none where nothing under it has a triangle
    object: Vec<Option<Option<usize>>>,
}

impl TreeWriter<'_> {
    /// The object of node `i`, written first if it is not yet: a mesh for a part, components for a subassembly, the
    /// object of the original for a repeat.
    fn emit(&mut self, i: usize) -> Option<usize> {
        if let Some(made) = self.object[i] {
            return made;
        }
        let n = &self.nodes[i];
        let made = match (n.same_as.filter(|&k| k < i), n.body) {
            (Some(k), _) => self.emit(k),
            (None, Some(b)) => self.by_body.get(&b).copied().filter(|e| !e.mesh.tris.is_empty()).map(|ExportMesh { mesh: m, tri_colors: t, .. }| {
                let id = self.next;
                self.next += 1;
                let own = n.color.and_then(|c| self.palette.iter().position(|p| *p == c));
                // a triangle whose face has a colour of its own names it; the rest take the object's
                let p1: Vec<Option<usize>> = if t.len() == m.tris.len() {
                    t.iter().map(|c| c.and_then(|c| self.palette.iter().position(|p| *p == c)).filter(|k| Some(*k) != own)).collect()
                } else {
                    Vec::new()
                };
                mesh_object(&mut self.model, id, &n.name, own.map(|k| (1, k)), m, &p1);
                id
            }),
            (None, None) => {
                let mut components = String::new();
                for c in self.children[i].clone() {
                    if let Some(id) = self.emit(c) {
                        components.push_str(&format!("    <component objectid=\"{id}\" transform=\"{}\"/>\n", row_form(&self.nodes[c].place)));
                    }
                }
                (!components.is_empty()).then(|| {
                    let id = self.next;
                    self.next += 1;
                    self.model.push_str(&format!("  <object id=\"{id}\" name=\"{}\" type=\"model\">\n   <components>\n{components}   </components>\n  </object>\n", crate::xml::escape(&n.name)));
                    id
                })
            }
        };
        self.object[i] = Some(made);
        made
    }
}

/// A placement of ours (3x4 row-major, applied to a column vector) as a 3MF transform (applied to a row vector): the
/// inverse of `placement`, each number in its shortest exact form.
fn row_form(p: &[f64; 12]) -> String {
    [p[0], p[4], p[8], p[1], p[5], p[9], p[2], p[6], p[10], p[3], p[7], p[11]].iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")
}

/// The opening of a model in millimetres, up to its resources.
fn model_head() -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"{CORE_NS}\">\n <resources>\n")
}

/// One object with a mesh, under `name`, in colour `pindex` of resource `pid` where one is given, a triangle with a place
/// in `p1` in that colour of resource 1 instead; the numbers in their shortest exact form.
fn mesh_object(model: &mut String, id: usize, name: &str, colour: Option<(usize, usize)>, m: &Mesh, p1: &[Option<usize>]) {
    let paint = colour.map_or(String::new(), |(pid, k)| format!(" pid=\"{pid}\" pindex=\"{k}\""));
    model.push_str(&format!("  <object id=\"{id}\" name=\"{}\" type=\"model\"{paint}>\n   <mesh>\n    <vertices>\n", crate::xml::escape(name)));
    for p in &m.verts {
        model.push_str(&format!("     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n", p.x, p.y, p.z));
    }
    model.push_str("    </vertices>\n    <triangles>\n");
    for (k, t) in m.tris.iter().enumerate() {
        let paint = p1.get(k).copied().flatten().map_or(String::new(), |q| format!(" pid=\"1\" p1=\"{q}\""));
        model.push_str(&format!("     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"{paint}/>\n", t[0], t[1], t[2]));
    }
    model.push_str("    </triangles>\n   </mesh>\n  </object>\n");
}

/// The package around a model: the content types, the relationship that points to the model, the model.
fn write_package(path: &str, model: &str) -> Result<(), String> {
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>\n";
    let rels = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"{MODEL_REL}\"/></Relationships>\n");
    let fail = |e: &dyn std::fmt::Display| format!("io-3mf-write-failed#{e}");
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut z = zip::ZipWriter::new(file);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in [("[Content_Types].xml", types), ("_rels/.rels", rels.as_str()), ("3D/3dmodel.model", model)] {
        z.start_file(name, opt).map_err(|e| fail(&e))?;
        z.write_all(body.as_bytes()).map_err(|e| fail(&e))?;
    }
    z.finish().map_err(|e| fail(&e))?;
    Ok(())
}
