//! The project file `.qcad`: a zip bundle.
//!
//! The layout: `meta.ron` with the schema version, `document.ron` with the project and the mesh geometry moved
//! out, `meshes/<id>.ron` with the geometry of a mesh under its stable id, deflated, and `sources/*` with the
//! originals of imports. The old flat format is not read: the project is in development and compatibility is
//! not needed.

use std::io::{Read, Write};

use qymcad_core::geom::{Mesh, MeshFace};
use qymcad_core::model::{self, Project};
use zip::write::SimpleFileOptions;

const SCHEMA: u32 = 2;

/// Save a project as a `.qcad` zip bundle. The faces live inside the body itself (`Body.faces`), so a separate
/// parallel list is no longer needed and cannot drift apart from the meshes.
pub fn save_project(project: &Project, path: &str) -> Result<(), String> {
    save_project_with_brep(project, path, &[])
}

/// The same, but with the live bodies: `breps` maps a body id to a B-rep blob.
///
/// The complaint was that making a cut meant waiting for a rebuild. A measurement found the culprit: an edit in
/// an already built project costs a second, while the first operation after opening a file rebuilds the whole
/// timeline — the bundle held meshes and faces, that is the geometry of the display, and nobody held a live
/// body, so the kernel built everything anew. A professional CAD puts the body itself into the file, and so
/// does this one now.
///
/// The bytes arrive ready-made: the format need know nothing of the kernel, and the kernel nothing of zip.
pub fn save_project_with_brep(project: &Project, path: &str, breps: &[(model::Id, Vec<u8>)]) -> Result<(), String> {
    let mut doc = project.clone();
    doc.ensure_ids(); // in case of geometry without ids, built directly

    // move the mesh geometry out, leaving placeholders in the document with their ids intact
    let ids = doc.bodies.iter().map(|b| b.id).collect::<Vec<_>>();
    let geoms: Vec<Mesh> = doc.bodies.iter().map(|b| b.mesh.clone()).collect::<Vec<_>>();
    let facelists: Vec<Vec<MeshFace>> = doc.bodies.iter().map(|b| b.faces.clone()).collect();
    // the document keeps placeholders: the heavy geometry lives in separate files of the bundle
    for b in doc.bodies.iter_mut() {
        b.mesh = Mesh { verts: Vec::new(), tris: Vec::new() };
        b.faces = Vec::new();
    }

    // Atomic saving: write to a temporary file alongside and swap it in by rename only on success. Writing
    // straight to the target file meant that any serialisation or disk error halfway through destroyed the
    // existing bundle, leaving a truncated zip. The old file now stays intact until the last moment.
    let tmp = format!("{path}.tmp~");
    let file = std::fs::File::create(&tmp).map_err(|e| format!("io-file-create#{tmp}: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("meta.ron", opts).map_err(|e| e.to_string())?;
    zip.write_all(format!("(schema_version: {SCHEMA}, app: \"qymcad\")\n").as_bytes()).map_err(|e| e.to_string())?;

    let doc_ron = model::to_ron(&doc)?;
    zip.start_file("document.ron", opts).map_err(|e| e.to_string())?;
    zip.write_all(doc_ron.as_bytes()).map_err(|e| e.to_string())?;

    for (g, id) in geoms.iter().zip(ids.iter()) {
        let mron = ron::ser::to_string(g).map_err(|e| e.to_string())?;
        zip.start_file(format!("meshes/{id}.ron"), opts).map_err(|e| e.to_string())?;
        zip.write_all(mron.as_bytes()).map_err(|e| e.to_string())?;
    }

    // the faces of a body sit next to its mesh, so B-rep faces are not recomputed by mesh detection
    for (f, id) in facelists.iter().zip(ids.iter()) {
        if !f.is_empty() {
            let fron = ron::ser::to_string(f).map_err(|e| e.to_string())?;
            zip.start_file(format!("faces/{id}.ron"), opts).map_err(|e| e.to_string())?;
            zip.write_all(fron.as_bytes()).map_err(|e| e.to_string())?;
        }
    }

    // The live bodies sit next to the meshes. Without them opening shows the model instantly, but the very
    // first operation pays for a full rebuild of the timeline.
    for (id, blob) in breps {
        if blob.is_empty() {
            continue;
        }
        zip.start_file(format!("brep/{id}.brep"), opts).map_err(|e| e.to_string())?;
        zip.write_all(blob).map_err(|e| e.to_string())?;
    }

    // The embedded originals of imports; their bytes live here rather than in `document.ron`.
    //
    // Sources are immutable, being the original of a file imported once, yet every save used to compress them
    // again: on an assembly with 89 MB of embedded STEP that was about four seconds of freeze on every save and
    // every autosave. Now, if the previous file holds the same entry at the same size, it is copied across as
    // raw, already compressed bytes without recompression.
    let mut prev = std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok());
    for s in &doc.sources {
        let name = format!("sources/{}.{}", s.id, s.ext);
        let reused = match prev.as_mut() {
            Some(ar) => match ar.index_for_name(&name) {
                Some(i) => match ar.by_index_raw(i) {
                    Ok(f) if f.size() == s.data.len() as u64 => zip.raw_copy_file(f).is_ok(),
                    _ => false,
                },
                None => false,
            },
            None => false,
        };
        if !reused {
            zip.start_file(&name, opts).map_err(|e| e.to_string())?;
            zip.write_all(&s.data).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;

    // The previous version stays alongside. An atomic swap saves from a truncated write but not from writing
    // the wrong thing: an empty document once landed on top of a finished project with nowhere to recover it
    // from. The copy costs milliseconds and one file on disk, and it is worth a working day.
    //
    // Regular files only: if the path holds a directory or anything else it is left untouched, or writing to
    // the wrong place would turn from an error into a quiet success.
    let bak = format!("{path}.bak");
    let backed = std::fs::metadata(path).map(|m| m.is_file()).unwrap_or(false) && std::fs::rename(path, &bak).is_ok();
    if let Err(e) = std::fs::rename(&tmp, path) {
        // the swap failed, so the previous version goes back, or it would merely have been hidden
        if backed {
            let _ = std::fs::rename(&bak, path);
        }
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("io-file-replace#{path}: {e}"));
    }
    Ok(())
}

/// How much content a document holds, which is what tells an empty one from a full one.
///
/// Needed where it is decided whether writing over an existing file is allowed: replacing a finished project
/// with an empty document silently is not acceptable, and there is no other way to tell them apart.
pub fn content_weight(p: &Project) -> usize {
    p.timeline.len() + p.sketches.len() + p.bodies.len() + p.components.len().saturating_sub(1) + p.contours.len()
}

/// A guarded save: refuse when an empty document would land on top of a non-empty file.
///
/// Exactly this case once cost a project: the application held an empty document while the path was left over
/// from the previous file, and saving erased the work. Whether the mistake was human or in the program does not
/// matter here: a write that destroys content and creates nothing makes sense in no scenario.
pub fn save_project_guarded(project: &Project, path: &str) -> Result<(), String> {
    save_project_guarded_with_brep(project, path, &[])
}

/// The same, but with the live bodies; see [`save_project_with_brep`].
pub fn save_project_guarded_with_brep(project: &Project, path: &str, breps: &[(model::Id, Vec<u8>)]) -> Result<(), String> {
    if content_weight(project) == 0 {
        if let Ok(existing) = load_project(path) {
            if content_weight(&existing) > 0 {
                return Err(format!("io-refuse-empty-over-full#{}", content_weight(&existing)));
            }
        }
    }
    save_project_with_brep(project, path, breps)
}

/// Load a `.qcad` bundle. Returns the project and the cache of part faces, parallel to `project.meshes`; an
/// empty vector for a mesh means the bundle held no faces for it.
pub fn load_project(path: &str) -> Result<Project, String> {
    load_project_with_brep(path).map(|(p, _)| p)
}

/// The same, but also returning the live bodies where the bundle holds them, as a map from body id to a B-rep
/// blob.
///
/// An empty list is a legitimate answer: the file was written without live bodies, or none were built. Then
/// everything works as before and the first operation simply pays for a rebuild.
pub fn load_project_with_brep(path: &str) -> Result<(Project, Vec<(model::Id, Vec<u8>)>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-file-read#{path}: {e}"))?;
    if !bytes.starts_with(b"PK") {
        return Err("io-not-a-qcad".into());
    }

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("zip: {e}"))?;
    let doc_s = read_entry(&mut zip, "document.ron")?;
    let mut project = model::from_ron(&doc_s)?; // the meshes are placeholders and the ids are correct

    // load the mesh geometry and the face cache by their ids
    let ids = project.bodies.iter().map(|b| b.id).collect::<Vec<_>>();
    for (i, id) in ids.iter().enumerate() {
        let s = read_entry(&mut zip, &format!("meshes/{id}.ron"))?;
        project.bodies[i].mesh = ron::from_str(&s).map_err(|e| e.to_string())?;
        project.bodies[i].faces = match read_entry(&mut zip, &format!("faces/{id}.ron")) {
            Ok(fs) => ron::from_str::<Vec<MeshFace>>(&fs).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
    }

    // load the embedded originals of imports
    let src: Vec<(usize, String)> = project.sources.iter().enumerate().map(|(i, s)| (i, format!("sources/{}.{}", s.id, s.ext))).collect();
    for (i, name) in src {
        if let Ok(data) = read_bytes(&mut zip, &name) {
            project.sources[i].data = data;
        }
    }
    // the live bodies, under the same ids as the meshes
    let mut breps: Vec<(model::Id, Vec<u8>)> = Vec::new();
    for id in &ids {
        if let Ok(blob) = read_bytes(&mut zip, &format!("brep/{id}.brep")) {
            breps.push((*id, blob));
        }
    }
    Ok((project, breps))
}

fn read_bytes<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>, String> {
    let mut f = zip.by_name(name).map_err(|e| format!("{name}: {e}"))?;
    let mut v = Vec::new();
    f.read_to_end(&mut v).map_err(|e| e.to_string())?;
    Ok(v)
}

fn read_entry<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<String, String> {
    let mut f = zip.by_name(name).map_err(|e| format!("{name}: {e}"))?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}
