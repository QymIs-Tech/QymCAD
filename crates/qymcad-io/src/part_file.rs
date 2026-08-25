//! The library part file `.qpart`: a zip bundle.
//!
//! The layout follows that of a project bundle: `part.ron` with the manifest, `document.ron` with a mini
//! project — one part or subassembly extracted by `Project::subproject_of`, with the mesh geometry moved out —
//! plus `meshes/<id>.ron`, `faces/<id>.ron` and an optional `thumb.png` preview. There is no backwards
//! compatibility while the project is in development.

use std::io::{Read, Write};

use qymcad_core::geom::{Mesh, MeshFace};
use qymcad_core::model::{self, Project};
use qymcad_core::part::PartManifest;
use zip::write::SimpleFileOptions;

/// Save a library part as a `.qpart` zip bundle.
///
/// `project` is the mini project from `subproject_of`: a root assembly plus a clone of the component. `faces`
/// is the cache of B-rep faces, parallel to `project.meshes`. `thumb_png` holds the optional preview.
pub fn save_part(project: &Project, manifest: &PartManifest, faces: &[Vec<MeshFace>], thumb_png: Option<&[u8]>, path: &str) -> Result<(), String> {
    let mut doc = project.clone();
    doc.ensure_ids();

    // move the mesh geometry out, leaving placeholders with their ids, as when saving a project
    let ids = doc.bodies.iter().map(|b| b.id).collect::<Vec<_>>();
    let geoms: Vec<Mesh> = doc.bodies.iter().map(|b| b.mesh.clone()).collect::<Vec<_>>();
    for m in doc.bodies.iter_mut().map(|b| &mut b.mesh) {
        *m = Mesh { verts: Vec::new(), tris: Vec::new() };
    }

    // atomic: a temporary file and a rename, so an error halfway through does not damage the existing file
    let tmp = format!("{path}.tmp~");
    let file = std::fs::File::create(&tmp).map_err(|e| format!("io-file-create#{tmp}: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let man_ron = ron::ser::to_string(manifest).map_err(|e| e.to_string())?;
    zip.start_file("part.ron", opts).map_err(|e| e.to_string())?;
    zip.write_all(man_ron.as_bytes()).map_err(|e| e.to_string())?;

    let doc_ron = model::to_ron(&doc)?;
    zip.start_file("document.ron", opts).map_err(|e| e.to_string())?;
    zip.write_all(doc_ron.as_bytes()).map_err(|e| e.to_string())?;

    for (g, id) in geoms.iter().zip(ids.iter()) {
        let mron = ron::ser::to_string(g).map_err(|e| e.to_string())?;
        zip.start_file(format!("meshes/{id}.ron"), opts).map_err(|e| e.to_string())?;
        zip.write_all(mron.as_bytes()).map_err(|e| e.to_string())?;
    }

    for (i, id) in ids.iter().enumerate() {
        if let Some(f) = faces.get(i) {
            if !f.is_empty() {
                let fron = ron::ser::to_string(f).map_err(|e| e.to_string())?;
                zip.start_file(format!("faces/{id}.ron"), opts).map_err(|e| e.to_string())?;
                zip.write_all(fron.as_bytes()).map_err(|e| e.to_string())?;
            }
        }
    }

    if let Some(png) = thumb_png {
        zip.start_file("thumb.png", opts).map_err(|e| e.to_string())?;
        zip.write_all(png).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| format!("io-file-replace#{path}: {e}"))?;
    Ok(())
}

/// A loaded library part: the mini project, the manifest, the face cache parallel to `project.meshes`, and the
/// preview bytes where present.
pub struct LoadedPart {
    pub project: Project,
    pub manifest: PartManifest,
    pub faces: Vec<Vec<MeshFace>>,
    pub thumb_png: Option<Vec<u8>>,
}

/// Load a library part from a `.qpart` bundle on disk.
pub fn load_part(path: &str) -> Result<LoadedPart, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-file-read#{path}: {e}"))?;
    load_part_bytes(&bytes)
}

/// Load a library part from `.qpart` bytes in memory, for the built-in catalogue.
pub fn load_part_bytes(bytes: &[u8]) -> Result<LoadedPart, String> {
    if !bytes.starts_with(b"PK") {
        return Err("io-not-a-qpart".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).map_err(|e| format!("zip: {e}"))?;

    let man_s = read_entry(&mut zip, "part.ron")?;
    let manifest: PartManifest = ron::from_str(&man_s).map_err(|e| format!("part.ron: {e}"))?;

    let doc_s = read_entry(&mut zip, "document.ron")?;
    let mut project = model::from_ron(&doc_s)?;

    let ids = project.bodies.iter().map(|b| b.id).collect::<Vec<_>>();
    let mut faces: Vec<Vec<MeshFace>> = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        if let Ok(s) = read_entry(&mut zip, &format!("meshes/{id}.ron")) {
            if let Ok(m) = ron::from_str::<Mesh>(&s) {
                project.bodies[i].mesh = m;
            }
        }
        let f = match read_entry(&mut zip, &format!("faces/{id}.ron")) {
            Ok(fs) => ron::from_str::<Vec<MeshFace>>(&fs).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        faces.push(f);
    }

    let thumb_png = read_bytes(&mut zip, "thumb.png").ok();

    Ok(LoadedPart { project, manifest, faces, thumb_png })
}

/// Read the manifest alone from a `.qpart` on disk, without unpacking the document or the meshes.
///
/// Used by the catalogue tree, which shows names and descriptions lazily without loading a part in full.
pub fn load_part_manifest(path: &str) -> Result<PartManifest, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-file-read#{path}: {e}"))?;
    load_part_manifest_bytes(&bytes)
}

/// The same from bytes in memory, for the built-in catalogue.
pub fn load_part_manifest_bytes(bytes: &[u8]) -> Result<PartManifest, String> {
    if !bytes.starts_with(b"PK") {
        return Err("io-not-a-qpart".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).map_err(|e| format!("zip: {e}"))?;
    let man_s = read_entry(&mut zip, "part.ron")?;
    ron::from_str(&man_s).map_err(|e| format!("part.ron: {e}"))
}

/// Read the preview alone from a `.qpart` on disk, without unpacking the document or the meshes, for the
/// thumbnail grid of the library. `None` means there is no preview, or an error occurred.
pub fn load_part_thumb(path: &str) -> Option<Vec<u8>> {
    load_part_thumb_bytes(&std::fs::read(path).ok()?)
}

/// The same from bytes in memory, for the built-in catalogue.
pub fn load_part_thumb_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.starts_with(b"PK") {
        return None;
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).ok()?;
    read_bytes(&mut zip, "thumb.png").ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use qymcad_core::feature::{ComponentKind, FeatureKind};
    use qymcad_core::geom::{Contour, Point2};

    // The source: a root assembly plus an active part with a square sketch, an extrusion and a feature
    // dimension.
    fn source_with_part() -> (Project, u64) {
        let mut p = Project::default();
        let part = p.new_document();
        let sq = vec![Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), Point2::new(20.0, 20.0), Point2::new(0.0, 20.0)];
        let sk = p.add_sketch("Section", vec![Contour::closed(sq)], None);
        p.add_sketch_node(sk, "Sketch");
        let body = p.add_extrude(sk, 100.0);
        p.feat_dims.entry(body).or_default().insert("height".into(), "Length".into());
        (p, part)
    }

    #[test]
    fn qpart_disk_round_trip_then_graft() {
        let (src, part) = source_with_part();
        let sub = src.subproject_of(part).expect("the extract");
        let manifest = PartManifest { schema_version: 1, name: "Extrusion 20x20".into(), description: "test".into(), tags: vec!["extrusion".into()], author: "basson".into() };

        let path = std::env::temp_dir().join("qym_test_profile_2020.qpart");
        let path_s = path.to_string_lossy().to_string();
        save_part(&sub, &manifest, &[], None, &path_s).expect("save_part");

        let loaded = load_part(&path_s).expect("load_part");
        assert_eq!(loaded.manifest.name, "Extrusion 20x20");
        assert_eq!(loaded.manifest.tags, vec!["extrusion".to_string()]);
        // the mini project holds one part under the root
        let parts = loaded.project.components.iter().filter(|c| c.kind == ComponentKind::Part).count();
        assert_eq!(parts, 1, "one part in the loaded library part");

        // insertion into a clean host
        let mut host = Project::default();
        host.new_document();
        let ins = host.graft(&loaded.project, host.root).expect("graft");
        let c = host.components.iter().find(|c| c.id == ins).unwrap();
        assert_eq!(c.kind, ComponentKind::Part, "a part was inserted");
        // the sketch and the extrusion arrived
        let kinds: Vec<bool> = host.timeline.iter().filter(|n| n.parent == Some(ins))
            .map(|n| matches!(n.kind, FeatureKind::Sketch { .. } | FeatureKind::Extrude { .. })).collect();
        assert_eq!(kinds.len(), 2, "the inserted part has a sketch and an extrusion");
        // the feature dimension survived the disk and the insertion
        let nb = host.timeline.iter().filter(|n| n.parent == Some(ins)).find_map(|n| n.kind.body()).unwrap();
        assert_eq!(host.feat_dims.get(&nb).and_then(|m| m.get("height")).map(String::as_str), Some("Length"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn qpart_thumb_round_trips() {
        // the preview goes into the bundle and reads back without unpacking the document, for the grid
        let (src, part) = source_with_part();
        let sub = src.subproject_of(part).unwrap();
        let png: &[u8] = b"\x89PNG\r\n\x1a\n-fake-thumbnail-bytes"; // arbitrary bytes standing in for a preview
        let path = std::env::temp_dir().join("qym_test_thumb.qpart");
        let ps = path.to_string_lossy().to_string();
        save_part(&sub, &PartManifest::new("With a preview"), &[], Some(png), &ps).unwrap();

        assert_eq!(load_part_thumb(&ps).as_deref(), Some(png), "the preview reads back exactly as written");
        assert_eq!(load_part(&ps).unwrap().thumb_png.as_deref(), Some(png), "and through a full load as well");
        let _ = std::fs::remove_file(&path);
    }
}
