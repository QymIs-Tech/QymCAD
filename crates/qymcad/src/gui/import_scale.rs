//! THE UNITS AND THE SCALE OF AN IMPORTED FILE, answered before it stays.
//!
//! STL, OBJ and PLY carry no unit, and a file that names one can name it wrong. Measured on sample files: a cube
//! came in at 1 mm and a cow at 10 mm, and an IGES bearing and a hammer that both say "millimetres" at 0.1 mm and
//! 38.9 m - exactly as their numbers are. So what is read is laid into the document inside an open edit, and a
//! window asks what the file is drawn in (for a file without units) and by what factor to take it. The bodies
//! change on screen with the answer; Enter keeps them as one step of undo, Esc rolls the edit back and nothing is
//! imported.

use super::ph;
use qymcad_core::geom::{Mesh, MeshFace, Point3};
use qymcad_core::model::Id;
use qymcad_io::FileUnit;
use qymcad_ui_state::{ImportScale, MeshFormat, WinCtx};

/// A model whose largest side is under this, or over `LARGEST_MM`, is asked about even when its file names a unit:
/// both ends were met in sample files, and no part a person imports is either size.
const SMALLEST_MM: f64 = 1.0;
const LARGEST_MM: f64 = 10_000.0;

/// The factors offered as buttons, beside the field for any other.
const FACTORS: [f64; 7] = [0.001, 0.01, 0.1, 1.0, 10.0, 100.0, 1000.0];

/// A uniform scale about the origin as a 3x4 row-major matrix, the form the kernel takes a placement in.
pub(crate) fn scale_matrix(f: f64) -> [f64; 12] {
    [f, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 0.0, f, 0.0]
}

/// Lay in the meshes read from `path`, then ask about their scale or keep them as they are.
pub(crate) fn land_mesh(wc: &mut WinCtx, path: String, format: MeshFormat, pieces: Vec<qymcad_ui_state::MeshPiece>) {
    keep_pending(wc);
    *wc.dxf_path = Some(path.clone()); // the next file chooser opens where this file was
    let said = super::io_jobs::mesh_added(format, &pieces);
    let span = span_of(pieces.iter().map(|(_, m, ..)| m));
    let read: Vec<(Mesh, Vec<MeshFace>)> = pieces.iter().map(|(_, m, f, ..)| (m.clone(), f.clone())).collect();
    let first = wc.project.bodies.len();
    qymcad_ui_state::begin_edit(wc.edits, wc.project, crate::i18n::tr1("io-import-mesh", "format", super::mesh_entry(format).name())); // an EDIT of the document: bodies are added to the current one
    let source = super::embed_source(wc.project, &path).unwrap_or(0);
    let stem = super::io_jobs::stem_of(&path);
    // EVERY PIECE A PART, as a solid comes in: under its own name or the file's, numbered, in the file's colour.
    // Reported behaviour: a mesh came in as bodies with no part, seen only at the top of the assembly.
    let many = pieces.len() > 1;
    let mut tops: Vec<Entry> = Vec::new();
    for (k, (own, mesh, faces, color, place, tri_colors, within)) in pieces.into_iter().enumerate() {
        let body = wc.project.add_mesh(mesh);
        wc.live.faces.insert(body, faces.clone()); // a face cache keyed by body Id, for quick access
        wc.project.set_body_faces(body, faces);
        if !own.is_empty() {
            wc.project.set_mesh_name(wc.project.bodies.len() - 1, own.clone());
        }
        let name = if !own.is_empty() {
            own
        } else if many {
            format!("{stem} {}", k + 1)
        } else {
            stem.clone()
        };
        put(&mut tops, &within, qymcad_core::model::ImportNode { name, place, body: Some(body), solid: k as u32, color, tri_colors, mesh: true, ..Default::default() });
    }
    // THE FILE'S GROUPS AS SUBASSEMBLIES: a glTF node that holds others, at its place in the group above it
    let nodes = tops.into_iter().map(|e| e.node(&stem)).collect();
    let created = wc.project.import_tree_as_parts(nodes, source, &stem); // one tested topology operation of the core
    // a part of the file reads as one already here ("Part 1" of the file beside "Part 1" of the document): numbered
    if let Some(root) = created {
        let came: Vec<qymcad_core::model::Id> = std::iter::once(root).chain(wc.project.descendants(root)).collect();
        wc.project.name_apart(&came, &crate::i18n::name);
    }
    // where every part stands, so a factor scales the assembly about the file's zero, not each part about its own
    let places = created.map(|root| std::iter::once(root).chain(wc.project.descendants(root)).map(|c| (c, wc.project.component_transform(c))).collect()).unwrap_or_default();
    if let Some(ci) = created.and_then(|cid| wc.project.components.iter().position(|c| c.id == cid)) {
        *wc.sel = super::Sel::Component(ci);
    }
    // THE VIEW GOES WHERE THE FILE CAME IN when it cannot be seen from where it stands: a new part beside the one
    // being worked in is not drawn from inside that one (a neighbour shows only in context), and a mesh brought in
    // could be neither seen nor clicked. From the assembly above, where it can be seen, the view stays.
    if let Some(cid) = created.filter(|&cid| !wc.project.component_is_within(cid, qymcad_ui_state::current_ctx_id(wc.active_path, wc.project))) {
        *wc.active_path = qymcad_ui_state::context_path_to(wc.project, cid);
    }
    qymcad_ui_state::invalidate(wc.regen);
    wc.view.initialized = false;
    wc.cam.init = false;
    *wc.status = said;
    let meshes = (first..wc.project.bodies.len()).filter_map(|i| wc.project.mesh_id(i)).zip(read).map(|(id, (m, f))| (id, m, f)).collect();
    let unitless = matches!(format, MeshFormat::Stl | MeshFormat::Obj | MeshFormat::Ply);
    let ask = ImportScale { file: super::file_name(&path), format: super::mesh_entry(format).name().to_string(), unitless, factor: 1.0, applied: 1.0, span, meshes, solids: Vec::new(), places, again: None };
    ask_or_keep(wc, ask);
}

/// A piece of a mesh file, or a group of the file with what stands in it, on the way to the document's tree.
enum Entry {
    Piece(qymcad_core::model::ImportNode),
    Group { id: usize, name: String, place: [f64; 12], kids: Vec<Entry> },
}

/// `leaf` into `level`, under the groups `chain` names from the top down, each made where it is first met.
fn put(level: &mut Vec<Entry>, chain: &[(usize, String, [f64; 12])], leaf: qymcad_core::model::ImportNode) {
    let Some(((id, name, place), rest)) = chain.split_first() else {
        level.push(Entry::Piece(leaf));
        return;
    };
    let at = match level.iter().position(|e| matches!(e, Entry::Group { id: g, .. } if g == id)) {
        Some(at) => at,
        None => {
            level.push(Entry::Group { id: *id, name: name.clone(), place: *place, kids: Vec::new() });
            level.len() - 1
        }
    };
    if let Entry::Group { kids, .. } = &mut level[at] {
        put(kids, rest, leaf);
    }
}

impl Entry {
    /// The document's node: a group the file leaves unnamed takes the file's name, as an unnamed piece does.
    fn node(self, stem: &str) -> qymcad_core::model::ImportNode {
        match self {
            Entry::Piece(n) => n,
            Entry::Group { name, place, kids, .. } => qymcad_core::model::ImportNode { name: if name.is_empty() { stem.to_string() } else { name }, place, children: kids.into_iter().map(|k| k.node(stem)).collect(), ..Default::default() },
        }
    }
}

/// Lay in the solids read from `path` as the file's tree of subassemblies and parts, then ask about their scale or
/// keep them as they are.
pub(crate) fn land_exact(wc: &mut WinCtx, path: String, format: qymcad_kernel::ExactFormat, bodies: Vec<qymcad_kernel::Body>, shapes: Vec<qymcad_kernel::Shape>, nodes: Vec<qymcad_kernel::ImportNode>) {
    keep_pending(wc);
    *wc.dxf_path = Some(path.clone()); // the next file chooser opens where this file was
    let nbodies = bodies.len();
    let tris: usize = bodies.iter().map(|(m, _)| m.tris.len()).sum();
    let named = qymcad_io::Format::of_path(&path).map(|f| f.name()).unwrap_or_default();
    qymcad_ui_state::begin_edit(wc.edits, wc.project, crate::i18n::tr1("io-import-mesh", "format", named));
    let source = super::embed_source(wc.project, &path).unwrap_or(0);
    let stem = super::io_jobs::stem_of(&path);

    // add the mesh, the faces and the shape of every solid, in the coordinates of its own part
    let mut shapes = shapes.into_iter();
    let mut ids = Vec::with_capacity(nbodies);
    let mut read = Vec::with_capacity(nbodies);
    for (mesh, fs) in bodies {
        let bid = wc.project.add_mesh(mesh);
        wc.live.faces.insert(bid, fs.clone()); // a face cache keyed by body Id, for quick access
        wc.project.set_body_faces(bid, fs);
        if let Some(s) = shapes.next() {
            if let Some(copy) = s.transformed(&scale_matrix(1.0)) {
                read.push((bid, copy)); // the shape as read, which every factor is taken from
            }
            wc.live.shapes.insert(bid, s);
            wc.live.shapes_rev = wc.live.shapes_rev.wrapping_add(1);
        }
        ids.push(bid);
    }
    // the file's subassemblies and parts in the active context, each where the file places it (one tested
    // topology operation of the core)
    let created = wc.project.import_tree_as_parts(qymcad_kernel::document_tree(&nodes, &ids, &stem), source, &stem);
    if let Some(root) = created {
        let came: Vec<qymcad_core::model::Id> = std::iter::once(root).chain(wc.project.descendants(root)).collect();
        wc.project.name_apart(&came, &crate::i18n::name);
    }
    let places = created.map(|root| std::iter::once(root).chain(wc.project.descendants(root)).map(|c| (c, wc.project.component_transform(c))).collect()).unwrap_or_default();
    qymcad_ui_state::regenerate_all(&mut wc.rebuild()); // re-tessellate the import nodes, and take faces and edges from the B-rep
    if let Some(ci) = created.and_then(|cid| wc.project.components.iter().position(|c| c.id == cid)) {
        *wc.sel = super::Sel::Component(ci);
    }
    // THE VIEW GOES WHERE THE FILE CAME IN when it cannot be seen from where it stands: a new part beside the one
    // being worked in is not drawn from inside that one (a neighbour shows only in context), and a mesh brought in
    // could be neither seen nor clicked. From the assembly above, where it can be seen, the view stays.
    if let Some(cid) = created.filter(|&cid| !wc.project.component_is_within(cid, qymcad_ui_state::current_ctx_id(wc.active_path, wc.project))) {
        *wc.active_path = qymcad_ui_state::context_path_to(wc.project, cid);
    }
    qymcad_ui_state::invalidate(wc.regen);
    wc.view.initialized = false;
    wc.cam.init = false;
    *wc.status = super::io_jobs::exact_imported(format, nbodies, tris);
    // the size as it stands in the world: a body alone is in its part's coordinates
    let span = world_span(wc.project, &ids);
    let ask = ImportScale { file: super::file_name(&path), format: named.to_string(), unitless: false, factor: 1.0, applied: 1.0, span, meshes: Vec::new(), solids: read, places, again: None };
    ask_or_keep(wc, ask);
}

/// The sides of the box of `bodies` as they stand in the world, each placed by its part.
fn world_span(project: &qymcad_core::model::Project, bodies: &[Id]) -> [f64; 3] {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for &id in bodies {
        let Some(b) = project.mesh_index(id).and_then(|i| project.bodies[i].mesh.bounds()) else { continue };
        let w = project.body_world_transform(id);
        for corner in 0..8 {
            let p = [if corner & 1 == 0 { b.min.x } else { b.max.x }, if corner & 2 == 0 { b.min.y } else { b.max.y }, if corner & 4 == 0 { b.min.z } else { b.max.z }];
            for r in 0..3 {
                let v = w[r * 4] * p[0] + w[r * 4 + 1] * p[1] + w[r * 4 + 2] * p[2] + w[r * 4 + 3];
                lo[r] = lo[r].min(v);
                hi[r] = hi[r].max(v);
            }
        }
    }
    std::array::from_fn(|k| (hi[k] - lo[k]).max(0.0))
}

/// A file still waiting for its answer when the next one comes in is kept as it stands, so its edit is not left
/// open under the new one.
fn keep_pending(wc: &mut WinCtx) {
    if wc.win.import_scale.take().is_some() {
        qymcad_ui_state::commit_edit(&mut wc.rebuild());
    }
}

/// Ask when the file names no unit, when the settings say always, or when what came in is no sensible size;
/// otherwise keep it as it came, as one step of undo. A format without units starts at the unit chosen for it last.
fn ask_or_keep(wc: &mut WinCtx, mut ask: ImportScale) {
    let largest = ask.span.iter().copied().fold(0.0, f64::max);
    if !(ask.unitless || wc.set.import_ask_always || largest < SMALLEST_MM || largest > LARGEST_MM) {
        qymcad_ui_state::commit_edit(&mut wc.rebuild());
        return;
    }
    if ask.unitless {
        if let Some(u) = wc.set.import_units.get(&ask.format).and_then(|c| FileUnit::of_code(c)) {
            ask.factor = u.mm();
            apply(wc, &mut ask);
        }
    }
    wc.win.import_scale = Some(ask);
}

/// Put every body that came in at `ask.factor` from the file's own numbers: a mesh from its copy as read, a solid
/// from its shape as read, with the factor kept by its import node.
fn apply(wc: &mut WinCtx, ask: &mut ImportScale) {
    let f = ask.factor;
    for (id, mesh, faces) in &ask.meshes {
        wc.project.set_import_scale(*id, f); // the mesh piece keeps the factor its mesh stands at
        let Some(i) = wc.project.bodies.iter().position(|b| b.id == *id) else { continue };
        let mut m = mesh.clone();
        m.scale(0.0, 0.0, 0.0, f);
        let fs = scaled_faces(faces, f);
        wc.project.bodies[i].mesh = m;
        wc.live.faces.insert(*id, fs.clone());
        wc.project.set_body_faces(*id, fs);
    }
    for (id, shape) in &ask.solids {
        if let Some(s) = shape.transformed(&scale_matrix(f)) {
            wc.live.shapes.insert(*id, s);
            wc.live.shapes_rev = wc.live.shapes_rev.wrapping_add(1);
        }
        wc.project.set_import_scale(*id, f);
    }
    // every component that came in stands at the factor too: the whole import is scaled about the file's zero, not
    // each part about its own
    for (id, place) in &ask.places {
        let mut m = *place;
        for k in [3, 7, 11] {
            m[k] *= f;
        }
        wc.project.set_component_transform(*id, m);
    }
    if !ask.solids.is_empty() {
        // now, not when the edit closes: the answer has to show while the window is still asking
        qymcad_ui_state::regenerate_now(&mut wc.rebuild());
    }
    qymcad_ui_state::invalidate(wc.regen);
    wc.view.initialized = false;
    wc.cam.init = false;
    ask.applied = f;
}

/// Faces at factor `f` about the origin: a centroid moves with it, an area grows as its square.
fn scaled_faces(faces: &[MeshFace], f: f64) -> Vec<MeshFace> {
    faces.iter().map(|x| MeshFace { centroid: Point3::new(x.centroid.x * f, x.centroid.y * f, x.centroid.z * f), area: x.area * f * f, ..x.clone() }).collect()
}

/// ASKED AGAIN FROM ITS NODE: the units and scale of an import already in the document, opened by a double click on
/// its node as any feature is edited. Everything that came in from the same file is asked about together, at the
/// factor it stands at now, and what was read is what stands now divided by that factor. Enter keeps the new factor
/// as one step of undo; Esc leaves the import as it was.
pub(crate) fn rescale(wc: &mut WinCtx, nid: Id) {
    use qymcad_core::feature::FeatureKind;
    let source_of = |k: &FeatureKind| match *k {
        FeatureKind::Import { source, .. } | FeatureKind::MeshPiece { source, .. } => Some(source),
        _ => None,
    };
    let Some(source) = wc.project.timeline.iter().find(|n| n.id == nid).and_then(|n| source_of(&n.kind)) else { return };
    let Some((file, ext)) = wc.project.sources.iter().find(|s| s.id == source).map(|s| (s.name.clone(), s.ext.to_lowercase())) else { return };
    keep_pending(wc);
    // every body from the file, and whether it is a piece of a mesh (its own geometry) or a solid (a live shape)
    let nodes: Vec<(Id, bool)> = wc.project.timeline.iter().filter(|n| source_of(&n.kind) == Some(source)).filter_map(|n| n.kind.body().map(|b| (b, !n.kind.waits_for_brep()))).collect();
    let was = nodes.first().and_then(|&(b, _)| wc.project.import_scale(b)).unwrap_or(1.0);
    let back = 1.0 / was;
    let (mut meshes, mut solids) = (Vec::new(), Vec::new());
    for &(b, piece) in &nodes {
        if piece {
            let Some(i) = wc.project.mesh_index(b) else { continue };
            let mut m = wc.project.bodies[i].mesh.clone();
            m.scale(0.0, 0.0, 0.0, back);
            meshes.push((b, m, scaled_faces(&wc.project.bodies[i].faces, back)));
        } else {
            // a solid's factor is taken from its live shape; one still being read back from its file after opening
            // has none yet
            let Some(s) = wc.live.shapes.get(&b).and_then(|s| s.transformed(&scale_matrix(back))) else {
                *wc.status = crate::i18n::tr("import-scale-wait");
                return;
            };
            solids.push((b, s));
        }
    }
    let bodies: Vec<Id> = nodes.iter().map(|&(b, _)| b).collect();
    // the components this import alone stands in - every body under them came from its file - as the file places them
    let project = &*wc.project;
    let under = |c: Id| std::iter::once(c).chain(project.descendants(c)).flat_map(|d| project.component_bodies(d)).collect::<Vec<_>>();
    let places: Vec<(Id, [f64; 12])> = project
        .components
        .iter()
        .map(|c| c.id)
        .filter(|&c| c != project.root && !under(c).is_empty() && under(c).iter().all(|b| bodies.contains(b)))
        .map(|c| {
            let mut m = project.component_transform(c);
            for k in [3, 7, 11] {
                m[k] *= back;
            }
            (c, m)
        })
        .collect();
    let span = world_span(project, &bodies).map(|s| s * back);
    let format = qymcad_io::Format::of_path(&file).map(|f| f.name()).unwrap_or_default().to_string();
    qymcad_ui_state::begin_edit(wc.edits, wc.project, crate::i18n::tr("import-scale-title"));
    wc.win.import_scale = Some(ImportScale { file, format, unitless: matches!(ext.as_str(), "stl" | "obj" | "ply"), factor: was, applied: was, span, meshes, solids, places, again: Some(was) });
}

/// The window: what the file is drawn in, by what factor to take it, and how big it lands.
pub(crate) fn import_scale_window(wc: &mut WinCtx, ctx: &egui::Context) {
    let Some(mut ask) = wc.win.import_scale.take() else { return };
    let mut keep = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Window::new(format!("{} {}", ph::RULER, crate::i18n::tr("import-scale-title")))
        .collapsible(false)
        .resizable(false)
        // at the bottom, not in the middle: the middle of the view is where the model it asks about stands
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -48.0])
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr1("import-scale-file", "file", &ask.file));
            if ask.unitless {
                ui.label(crate::i18n::tr("import-scale-unit"));
                ui.horizontal(|ui| {
                    for u in FileUnit::ALL {
                        if ui.selectable_label(same(ask.factor, u.mm()), unit_name(u)).clicked() {
                            ask.factor = u.mm();
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("import-scale-factor"));
                for k in FACTORS {
                    if ui.selectable_label(same(ask.factor, k), format!("{k}")).clicked() {
                        ask.factor = k;
                    }
                }
                ui.add(egui::DragValue::new(&mut ask.factor).speed(0.01).range(1e-6..=1e6));
            });
            ui.label(crate::i18n::tr1("import-scale-in-file", "size", &sides(ask.span, 1.0)));
            ui.label(crate::i18n::tr1("import-scale-lands", "size", &sides(ask.span, ask.factor)));
            let largest = ask.span.iter().copied().fold(0.0, f64::max) * ask.factor;
            if largest < SMALLEST_MM {
                ui.label(egui::RichText::new(crate::i18n::tr("import-scale-small")).weak());
            } else if largest > LARGEST_MM {
                ui.label(egui::RichText::new(crate::i18n::tr("import-scale-big")).weak());
            }
            ui.separator();
            ui.horizontal(|ui| {
                // asked again from its node, the file is in already: the factor is applied, and Esc keeps it as it was
                let (take, drop) = if ask.again.is_some() { ("import-scale-apply", "io-cancel") } else { ("import-scale-import", "io-cancel-import") };
                if ui.button(crate::i18n::tr(take)).clicked() {
                    keep = true;
                }
                if ui.button(crate::i18n::tr(drop)).clicked() {
                    cancel = true;
                }
            });
        });
    if cancel {
        qymcad_ui_state::abort_edit(&mut wc.rebuild());
        // asked again: the document is back as it was, but a live solid is not part of it - the one the preview scaled
        // is put back at the factor it stood at
        if let Some(was) = ask.again {
            for (id, shape) in &ask.solids {
                if let Some(s) = shape.transformed(&scale_matrix(was)) {
                    wc.live.shapes.insert(*id, s);
                    wc.live.shapes_rev = wc.live.shapes_rev.wrapping_add(1);
                }
            }
        }
        qymcad_ui_state::invalidate(wc.regen);
        wc.cam.init = false;
        *wc.status = crate::i18n::tr(if ask.again.is_some() { "import-scale-kept" } else { "import-scale-cancelled" });
        return;
    }
    if (ask.factor - ask.applied).abs() > 1e-12 && ask.factor > 0.0 {
        apply(wc, &mut ask);
    }
    if keep {
        if let Some(u) = FileUnit::ALL.into_iter().find(|u| ask.unitless && same(ask.factor, u.mm())) {
            wc.set.import_units.insert(ask.format.clone(), u.code().to_string());
        }
        qymcad_ui_state::commit_edit(&mut wc.rebuild());
        return;
    }
    wc.win.import_scale = Some(ask);
}

fn unit_name(u: FileUnit) -> String {
    crate::i18n::tr(match u {
        FileUnit::Micron => "import-unit-micron",
        FileUnit::Millimetre => "import-unit-mm",
        FileUnit::Centimetre => "import-unit-cm",
        FileUnit::Metre => "import-unit-m",
        FileUnit::Inch => "import-unit-inch",
        FileUnit::Foot => "import-unit-foot",
    })
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// The sides of the box of every mesh.
fn span_of<'a>(meshes: impl Iterator<Item = &'a Mesh>) -> [f64; 3] {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for b in meshes.filter_map(Mesh::bounds) {
        for (k, (a, z)) in [(b.min.x, b.max.x), (b.min.y, b.max.y), (b.min.z, b.max.z)].into_iter().enumerate() {
            lo[k] = lo[k].min(a);
            hi[k] = hi[k].max(z);
        }
    }
    std::array::from_fn(|k| (hi[k] - lo[k]).max(0.0))
}

/// The sides at factor `f`, as `a x b x c` with up to three decimals.
fn sides(span: [f64; 3], f: f64) -> String {
    let n = |v: f64| {
        let s = format!("{:.3}", v * f);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    format!("{} x {} x {}", n(span[0]), n(span[1]), n(span[2]))
}
