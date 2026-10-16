//! A FILE WHOSE BODY HOLDS A NUMBER THAT IS NOT ONE IS REFUSED, with its place.
//!
//! A `.qcad` project and a `.qpart` library part keep each body's mesh as `meshes/<id>.ron` and its faces as
//! `faces/<id>.ron`, and RON reads `NaN` and `inf` as numbers, so a damaged or crafted file brought a non-finite corner
//! or face in past every importer: the mesh's bounds came out wrong, its volume NaN, and the STL it exported was refused
//! by the program's own reader; a face's centroid and normal are what picking and snapping read. Such a file is now
//! refused as it is read, naming the body (by its number), the corner or face and the value - the way the importers
//! refuse a mesh file.
use qymcad_core::geom::{Mesh, MeshFace, Point3};
use qymcad_core::model::Project;
use qymcad_core::part::PartManifest;
use qymcad_io::{load_part, load_project, save_part, save_project};
use std::io::{Read, Write};

fn path(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/project-not-finite"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

/// The bundle at `from` written again to `to`, with `find` replaced by `with` in its entries under `folder`.
fn tampered(from: &str, to: &str, folder: &str, find: &str, with: &str) {
    let mut zin = zip::ZipArchive::new(std::fs::File::open(from).expect("the bundle")).expect("a zip");
    let mut zout = zip::ZipWriter::new(std::fs::File::create(to).expect("written"));
    let mut found = 0;
    for i in 0..zin.len() {
        let mut e = zin.by_index(i).expect("an entry");
        let name = e.name().to_string();
        let mut bytes = Vec::new();
        e.read_to_end(&mut bytes).expect("read");
        if name.starts_with(folder) {
            let text = String::from_utf8(bytes).expect("RON is text");
            found += text.matches(find).count();
            bytes = text.replace(find, with).into_bytes();
        }
        zout.start_file(name, zip::write::SimpleFileOptions::default()).expect("an entry");
        zout.write_all(&bytes).expect("written");
    }
    zout.finish().expect("closed");
    assert_eq!(found, 1, "{find} was not found once under {folder}");
}

/// Two bodies; the corner (1.25, 0, 0) is the second corner of the second body, whose one face has its centroid at
/// x = 0.4375.
fn two_bodies() -> Project {
    let mut p = Project::default();
    let v = |x: f64, y: f64| Point3 { x, y, z: 0.0 };
    p.add_mesh(Mesh { verts: vec![v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0)], tris: vec![[0, 1, 2]], ..Default::default() });
    p.add_mesh(Mesh { verts: vec![v(0.0, 0.0), v(1.25, 0.0), v(0.0, 2.0)], tris: vec![[0, 1, 2]], ..Default::default() });
    p.bodies[1].faces = vec![MeshFace { triangles: vec![0], normal: [0.0, 0.0, 1.0], centroid: v(0.4375, 0.5), area: 1.25, id: 0 }];
    p
}

/// The two bodies saved as a project named `name` (each check its own: they run side by side).
fn saved(name: &str) -> String {
    let clean = path(name);
    save_project(&two_bodies(), &clean).expect("saved");
    clean
}

#[test]
fn a_corner_that_is_not_a_number_refuses_the_project() {
    let clean = saved("clean-to-tamper.qcad");
    for word in ["NaN", "inf"] {
        let bad = path(&format!("corner-{word}.qcad"));
        tampered(&clean, &bad, "meshes/", "1.25", word);
        assert_eq!(load_project(&bad).err(), Some(format!("io-body-corner-not-finite#2\u{1f}2\u{1f}{word}")), "a project with a corner {word} was not refused with its place");
    }
}

#[test]
fn a_face_that_is_not_a_number_refuses_the_project() {
    let clean = saved("clean-faces-to-tamper.qcad");
    let bad = path("face-NaN.qcad");
    tampered(&clean, &bad, "faces/", "0.4375", "NaN");
    assert_eq!(load_project(&bad).err(), Some("io-body-face-not-finite#2\u{1f}1\u{1f}NaN".to_string()), "a project with a face centroid NaN was not refused with its place");
}

#[test]
fn a_library_part_is_checked_as_a_project_is() {
    let clean = path("clean.qpart");
    save_part(&two_bodies(), &PartManifest::new("two bodies"), &[], None, &clean).expect("saved");
    let bad = path("corner-NaN.qpart");
    tampered(&clean, &bad, "meshes/", "1.25", "NaN");
    assert_eq!(load_part(&bad).err(), Some("io-body-corner-not-finite#2\u{1f}2\u{1f}NaN".to_string()), "a part with a corner NaN was not refused with its place");
}

#[test]
fn an_untouched_project_still_opens() {
    let back = load_project(&saved("clean.qcad")).expect("the untouched project opens");
    assert_eq!(back.bodies.len(), 2);
    assert_eq!(back.bodies[1].faces.len(), 1, "the face of the second body did not come back");
}
