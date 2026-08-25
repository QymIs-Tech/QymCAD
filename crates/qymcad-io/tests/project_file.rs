//! Round-trip tests of the project file, a `.qcad` zip bundle.

use qymcad_core::geom::{circle_contour, Mesh, Point3};
use qymcad_core::model::{OpKind, OperationDef, Project};
use qymcad_core::tool::{Tool, ToolType};
use qymcad_io::{load_project, save_project};

fn sample() -> Project {
    let mut p = Project::default();
    p.set_contours(vec![circle_contour(0.0, 0.0, 5.0, 0.1)]);
    p.add_mesh(Mesh {
        verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 0.0, 0.0), Point3::new(0.0, 10.0, 5.0)],
        tris: vec![[0, 1, 2]],
    });
    p.tools = vec![Tool { number: 1, name: "EM6".into(), kind: ToolType::FlatEnd, diameter: 6.0, corner_radius: 0.0, flutes: 2, v_angle: None }];
    p.operations.push(OperationDef::new("Engrave", 1, OpKind::Engrave));
    p
}

#[test]
fn qcad_bundle_roundtrip() {
    let path = std::env::temp_dir().join("qym_rt.qcad");
    let path = path.to_str().unwrap();
    let orig = sample();
    save_project(&orig, path).expect("save ok");

    let back = load_project(path).expect("load ok");
    assert_eq!(back.contours.len(), 1);
    assert_eq!(back.bodies.len(), 1, "the mesh was restored from the bundle");
    assert_eq!(back.bodies[0].mesh.tris.len(), 1);
    assert_eq!(back.tools.len(), 1);
    assert_eq!(back.operations.len(), 1);
    // the contents of the mesh match
    assert!((back.bodies[0].mesh.verts[2].z - 5.0).abs() < 1e-9);
}

#[test]
fn embedded_source_survives_reload() {
    let mut p = Project::default();
    let raw = b"0\nSECTION\n  2\nENTITIES\n0\nLINE\n".to_vec();
    let sid = p.add_source("rama.dxf", raw.clone());

    let path = std::env::temp_dir().join("qym_src.qcad");
    let path = path.to_str().unwrap();
    save_project(&p, path).unwrap();
    let back = load_project(path).unwrap();

    assert_eq!(back.sources.len(), 1);
    assert_eq!(back.sources[0].id, sid);
    assert_eq!(back.sources[0].name, "rama.dxf");
    assert_eq!(back.sources[0].ext, "dxf");
    assert_eq!(back.sources[0].data, raw, "the bytes of the original were restored");
}

#[test]
fn op_mesh_ref_survives_reload() {
    // an operation references a mesh by its stable id, and the reference survives the round trip
    let mut p = Project::default();
    let mid = p.add_mesh(Mesh {
        verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 0.0, 0.0), Point3::new(0.0, 10.0, 5.0)],
        tris: vec![[0, 1, 2]],
    });
    p.tools = vec![Tool { number: 1, name: "B2".into(), kind: ToolType::BallNose, diameter: 2.0, corner_radius: 1.0, flutes: 2, v_angle: None }];
    p.operations.push(OperationDef::new("Finish", 1, OpKind::Surface3D { mesh: mid }));

    let path = std::env::temp_dir().join("qym_ref.qcad");
    let path = path.to_str().unwrap();
    save_project(&p, path).unwrap();
    let back = load_project(path).unwrap();

    // the same mesh id after reloading
    assert_eq!(back.bodies.iter().map(|b| b.id).collect::<Vec<_>>(), vec![mid]);
    match back.operations[0].kind {
        OpKind::Surface3D { mesh } => assert_eq!(mesh, mid, "the reference of the operation to the mesh survived"),
        _ => panic!("the wrong operation"),
    }
}

#[test]
fn brep_faces_survive_reload() {
    use qymcad_core::geom::MeshFace;
    let mut p = Project::default();
    p.add_mesh(Mesh {
        verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 0.0, 0.0), Point3::new(0.0, 10.0, 0.0)],
        tris: vec![[0, 1, 2]],
    });
    // a B-rep face from STEP lives inside the body itself; there is no parallel list any more
    p.bodies[0].faces = vec![MeshFace { triangles: vec![0], normal: [0.0, 0.0, 1.0], centroid: Point3::new(3.3, 3.3, 0.0), area: 50.0, id: 0 }];

    let path = std::env::temp_dir().join("qym_faces.qcad");
    let path = path.to_str().unwrap();
    save_project(&p, path).unwrap();
    let back = load_project(path).unwrap();

    assert_eq!(back.bodies.len(), 1, "the body was restored");
    assert_eq!(back.bodies[0].faces.len(), 1, "the faces arrived inside the body");
    assert!((back.bodies[0].faces[0].area - 50.0).abs() < 1e-9, "the face was not recomputed by detection");
}

/// A repeated save does not recompress the embedded sources: on an assembly with 89 MB of STEP that cost
/// seconds of freeze on every save and every autosave. The bytes after a raw copy have to match to the last
/// one, or the time saved becomes a damaged file.
#[test]
fn resaving_reuses_stored_sources_bytewise() {
    let path = std::env::temp_dir().join("qym_resave_sources.qcad");
    let path = path.to_str().unwrap();
    let _ = std::fs::remove_file(path);

    let mut p = sample();
    // an imported original: a sizeable body with repetitions, which compresses, plus a random tail, which does
    // not
    let mut raw: Vec<u8> = (0..200_000u32).flat_map(|i| format!("LINE {i};").into_bytes()).collect();
    raw.extend((0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8));
    p.sources.push(qymcad_core::model::SourceFile { id: 42, name: "big.stp".into(), ext: "stp".into(), data: raw.clone() });

    save_project(&p, path).expect("the first save");
    let first_len = std::fs::metadata(path).unwrap().len();
    save_project(&p, path).expect("the repeated save, copying the sources raw");
    let second_len = std::fs::metadata(path).unwrap().len();
    assert!(
        (first_len as i64 - second_len as i64).abs() < (first_len / 10) as i64,
        "the size of the bundle must not jump: {first_len} -> {second_len}"
    );

    let back = load_project(path).expect("the bundle reads back after the raw copy");
    assert_eq!(back.sources.len(), 1, "the source is in place");
    assert_eq!(back.sources[0].data, raw, "the source bytes match to the last one, so the raw copy damages nothing");
    assert_eq!(back.bodies.len(), 1, "the rest of the bundle is intact");
    let _ = std::fs::remove_file(path);
}

/// A source that changed, keeping its id but changing size, has to be rewritten rather than carried across by a
/// raw copy from the previous file.
#[test]
fn changed_source_is_rewritten_not_copied() {
    let path = std::env::temp_dir().join("qym_changed_source.qcad");
    let path = path.to_str().unwrap();
    let _ = std::fs::remove_file(path);

    let mut p = sample();
    p.sources.push(qymcad_core::model::SourceFile { id: 7, name: "a.stp".into(), ext: "stp".into(), data: b"AAAA".repeat(1000).to_vec() });
    save_project(&p, path).expect("save 1");
    p.sources[0].data = b"BBBBBB".repeat(2000).to_vec();
    save_project(&p, path).expect("save 2");

    let back = load_project(path).expect("load");
    assert_eq!(back.sources[0].data, p.sources[0].data, "the file holds the new source bytes");
    let _ = std::fs::remove_file(path);
}

// ── negative loading scenarios ───────────────────────────────────────────────────────────────────
//
// Saving and loading is where data loss lives, and only the happy path had been checked. One requirement: an
// honest error, or an honest partial result, but never a panic and never damage to an existing file.

#[test]
fn corrupt_and_truncated_bundles_are_honest_errors() {
    let dir = std::env::temp_dir();
    let mut bad: Vec<String> = Vec::new();
    let mut check = |label: &str, name: &str, bytes: &[u8]| {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        match load_project(p.to_str().unwrap()) {
            Ok(_) => bad.push(format!("{label}: it read back as a valid project")),
            Err(e) if e.is_empty() => bad.push(format!("{label}: an error without any text")),
            Err(_) => {}
        }
        let _ = std::fs::remove_file(&p);
    };
    check("not a zip at all", "qym_neg_notzip.qcad", b"just text, not a bundle");
    check("an empty file", "qym_neg_empty.qcad", b"");
    check("a zip header followed by rubbish", "qym_neg_pk.qcad", b"PK\x03\x04 and then garbage \xff\xfe\x00");

    // a truncated real bundle: a valid project is written and then cut in half
    let path = dir.join("qym_neg_full.qcad");
    save_project(&sample(), path.to_str().unwrap()).expect("save");
    let full = std::fs::read(&path).unwrap();
    check("a truncated bundle", "qym_neg_cut.qcad", &full[..full.len() / 2]);
    let _ = std::fs::remove_file(&path);

    assert!(bad.is_empty(), "damaged bundles have to give an honest error:\n{}", bad.join("\n"));
}

/// A bundle without a mesh entry, which a truncated save by an older version could have left behind, is a read
/// error. The requirement: the error is honest and names what is missing, rather than staying silent or killing
/// the process.
#[test]
fn bundle_missing_mesh_entry_names_what_is_lost() {
    let dir = std::env::temp_dir();
    let src = dir.join("qym_neg_mesh_src.qcad");
    save_project(&sample(), src.to_str().unwrap()).expect("save");
    let bytes = std::fs::read(&src).unwrap();

    // rebuild the zip without the mesh entries
    let cut = dir.join("qym_neg_mesh_cut.qcad");
    {
        let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let out = std::fs::File::create(&cut).unwrap();
        let mut zout = zip::ZipWriter::new(out);
        for i in 0..zin.len() {
            let f = zin.by_index_raw(i).unwrap();
            if f.name().starts_with("meshes/") {
                continue;
            }
            zout.raw_copy_file(f).unwrap();
        }
        zout.finish().unwrap();
    }
    match load_project(cut.to_str().unwrap()) {
        Ok(p) => assert!(p.bodies.iter().map(|b| &b.mesh).all(|m| m.tris.is_empty()), "if it read at all, there is no geometry and that is visible"),
        Err(e) => assert!(e.contains("meshes/"), "the error names the missing entry: {e}"),
    }
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&cut);
}

/// A failed write must not touch the existing file, thanks to the temporary file and rename. A regression guard
/// in case anyone optimises the write into going straight to the target path.
#[test]
fn failed_save_leaves_previous_file_intact() {
    let dir = std::env::temp_dir().join("qym_neg_atomic");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("proj.qcad");
    save_project(&sample(), path.to_str().unwrap()).expect("the first save");
    let before = std::fs::read(&path).unwrap();

    // a directory in place of a file: the write has to fail
    let as_dir = dir.join("dir.qcad");
    std::fs::create_dir_all(&as_dir).unwrap();
    assert!(save_project(&sample(), as_dir.to_str().unwrap()).is_err(), "writing into a directory is an error");

    let after = std::fs::read(&path).unwrap();
    assert_eq!(before, after, "the existing project was unharmed");
    let back = load_project(path.to_str().unwrap()).expect("and still reads back");
    assert_eq!(back.bodies.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A REVERSED CUT COMES BACK REVERSED.
///
/// The three switches of a tool - through all, reversed, symmetric - are one named `Extent`, in the record
/// as well as in the code. What this guards is the round trip: a nested value is where a format quietly
/// stops being self-describing, and a document that reads back with a default instead of what was written
/// does not refuse to open - it builds the part differently, cutting the other way.
///
/// `#[serde(flatten)]` was tried first, to keep the three keys side by side in the file, and the format
/// refused it: RON writes a flattened field as a map and then declines to read its own output ("Expected
/// identifier").
#[test]
fn a_reversed_cut_survives_a_save_and_a_load() {
    use qymcad_core::feature::{Extent, FeatureKind};
    let path = std::env::temp_dir().join("qym_extent_rt.qcad");
    let path = path.to_str().unwrap();
    let mut p = sample();
    // The node alone is what is being written and read; no geometry is needed to check the shape of the record.
    let ext = Extent { through: true, reach: qymcad_core::feature::Reach::Backward };
    let node = p.add_combine_on(0, 0, 0, 5.0, 0, ext, 0.0);
    save_project(&p, path).expect("save ok");

    let back = load_project(path).expect("load ok");
    let kind = &back.timeline.iter().find(|n| n.id == node).expect("the combine node survived").kind;
    let FeatureKind::Combine { extent, .. } = kind else { panic!("the node is no longer a combine") };
    assert_eq!(*extent, ext, "the extent of the tool came back exactly as it was written");
    let _ = std::fs::remove_file(path);
}
