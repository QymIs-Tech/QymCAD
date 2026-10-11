//! AMF: WRITTEN AND READ BACK the same bodies, and the files other programs write - units, constellations,
//! a zipped file - read as they mean.
use std::io::Write;

use qymcad_core::geom::{Mesh, Point3};
use qymcad_io::{export_amf, import_amf};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, text: Option<&str>) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/amf-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    if let Some(t) = text {
        std::fs::write(&p, t).expect("written");
    }
    p.to_string_lossy().into_owned()
}

fn tetra(x: f64) -> Mesh {
    let v = |a: f64, b: f64, c: f64| Point3::new(x + a, b, c);
    Mesh { verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 1.0 / 3.0), v(0.1, 0.2, 10.000000000000002)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], ..Default::default() }
}

/// A piece where it stands: its mesh put at its place, then at the places of the groups it stands in.
fn placed(piece: &qymcad_io::NamedMesh) -> Mesh {
    let mut m = piece.mesh.clone();
    m.transform(&piece.place);
    for place in piece.within.iter().rev().map(|g| &g.place) {
        m.transform(place);
    }
    m
}

/// One triangle, as an object.
fn tri(id: u32) -> String {
    let v = |x: f64, y: f64| format!("<vertex><coordinates><x>{x}</x><y>{y}</y><z>0</z></coordinates></vertex>");
    format!("<object id=\"{id}\"><mesh><vertices>{}{}{}</vertices><volume><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume></mesh></object>", v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0))
}

/// THREE BODIES GO OUT AND THREE COME BACK, every coordinate to the last bit, every triangle on its numbers.
#[test]
fn three_bodies_come_back_to_the_last_bit() {
    let out = [tetra(0.0), tetra(30.0), tetra(-20.0)];
    let p = file("three.amf", None);
    export_amf(&out, &p).expect("the AMF is written");
    let back = import_amf(&p).expect("the AMF reads back");
    assert_eq!(back.len(), 3, "three bodies went out, {} came back", back.len());
    for (a, b) in out.iter().zip(&back) {
        assert_eq!(a.tris, b.mesh.tris, "the triangles came back renumbered");
        for (p, q) in a.verts.iter().zip(&b.mesh.verts) {
            assert!(p.x == q.x && p.y == q.y && p.z == q.z, "a vertex came back moved: {p:?} -> {q:?}");
        }
    }
    assert_eq!(back[1].name, "body_2");
}

/// A FILE IN INCHES COMES IN IN MILLIMETRES.
#[test]
fn a_file_in_inches_comes_in_in_millimetres() {
    let p = file("inches.amf", Some(&format!("<amf unit=\"inch\">{}</amf>", tri(1))));
    assert_eq!(import_amf(&p).expect("reads")[0].mesh.verts[1].x, 25.4);
}

/// A CONSTELLATION PLACES ITS INSTANCES: the same triangle twice, once shifted and once turned a quarter about Z.
#[test]
fn a_constellation_places_its_instances() {
    let cons = "<constellation id=\"9\"><instance objectid=\"1\"><deltax>10</deltax><deltay>0</deltay><deltaz>0</deltaz><rx>0</rx><ry>0</ry><rz>0</rz></instance>\
                <instance objectid=\"1\"><deltax>0</deltax><deltay>0</deltay><deltaz>0</deltaz><rx>0</rx><ry>0</ry><rz>90</rz></instance></constellation>";
    let back = import_amf(&file("constellation.amf", Some(&format!("<amf unit=\"millimeter\">{}{cons}</amf>", tri(1))))).expect("reads");
    assert_eq!(back.len(), 2, "two instances are two bodies");
    let shifted = placed(&back[0]).verts[1];
    assert!((shifted.x - 11.0).abs() < 1e-12, "the shift was lost: {shifted:?}");
    let turned = placed(&back[1]).verts[1];
    assert!(turned.x.abs() < 1e-12 && (turned.y - 1.0).abs() < 1e-12, "a quarter turn about Z did not take +X to +Y: {turned:?}");
}

/// A ZIPPED FILE is read through its one entry.
#[test]
fn a_zipped_file_is_read_through_its_entry() {
    let p = file("zipped.amf", None);
    let mut z = zip::ZipWriter::new(std::fs::File::create(&p).expect("created"));
    z.start_file("model.amf", zip::write::SimpleFileOptions::default()).expect("an entry");
    z.write_all(format!("<amf>{}</amf>", tri(1)).as_bytes()).expect("written");
    z.finish().expect("closed");
    assert_eq!(import_amf(&p).expect("reads")[0].mesh.tris.len(), 1);
}

/// A BROKEN FILE IS REFUSED BY NAME.
#[test]
fn a_broken_file_is_refused_by_name() {
    assert_eq!(import_amf(&file("not-xml.amf", Some("not xml at all"))).err().as_deref(), Some("io-amf-not-amf"));
    assert_eq!(import_amf(&file("other-xml.amf", Some("<svg/>"))).err().as_deref(), Some("io-amf-not-amf"));
    let bad = tri(1).replace("<v3>2</v3>", "<v3>7</v3>");
    assert_eq!(import_amf(&file("bad-index.amf", Some(&format!("<amf>{bad}</amf>")))).err().as_deref(), Some("io-amf-bad-index"));
    assert_eq!(import_amf(&file("furlong.amf", Some(&format!("<amf unit=\"furlong\">{}</amf>", tri(1))))).err().as_deref(), Some("io-amf-unknown-unit#furlong"));
    assert_eq!(import_amf(&file("empty.amf", Some("<amf></amf>"))).err().as_deref(), Some("io-amf-no-meshes"));
    assert!(export_amf(&[], &file("nothing.amf", None)).is_err(), "an empty set was written as a file");
}

/// THE COLOUR OF A VOLUME IS THE COLOUR OF ITS OBJECT, where the object has none of its own: the owner's AMF writes
/// its colours on the volumes, one per object.
#[test]
fn a_volume_colour_is_the_colour_of_its_object() {
    let v = |x: f64, y: f64| format!("<vertex><coordinates><x>{x}</x><y>{y}</y><z>0</z></coordinates></vertex>");
    let obj = format!(
        "<object id=\"1\"><mesh><vertices>{}{}{}</vertices><volume><color><r>0.8</r><g>0.1</g><b>0.1</b></color><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume></mesh></object>",
        v(0.0, 0.0),
        v(1.0, 0.0),
        v(0.0, 1.0)
    );
    let back = import_amf(&file("coloured.amf", Some(&format!("<?xml version=\"1.0\"?><amf unit=\"millimeter\">{obj}</amf>")))).expect("reads");
    assert_eq!(back[0].color, Some([204, 26, 26]), "the volume's colour was dropped");
}

/// A CONSTELLATION COMES AS A GROUP OF ITS OWN: the head places a plate and, twice, a unit that holds a pin - every
/// instance of the unit a group of its own, under the unit's name and at its place, and every piece in its object's
/// own frame at its instance's place, not baked where it stands.
#[test]
fn a_constellation_comes_as_a_group_of_its_own() {
    let at = |id: u32, x: f64, z: f64, rz: f64| format!("<instance objectid=\"{id}\"><deltax>{x}</deltax><deltay>0</deltay><deltaz>{z}</deltaz><rx>0</rx><ry>0</ry><rz>{rz}</rz></instance>");
    let unit = format!("<constellation id=\"20\"><metadata type=\"name\">unit</metadata>{}</constellation>", at(2, 5.0, 0.0, 0.0));
    let head = format!("<constellation id=\"10\"><metadata type=\"name\">head</metadata>{}{}{}</constellation>", at(1, 30.0, 0.0, 0.0), at(20, 0.0, 50.0, 90.0), at(20, 0.0, 80.0, 0.0));
    let back = import_amf(&file("groups.amf", Some(&format!("<amf unit=\"millimeter\">{}{}{unit}{head}</amf>", tri(1), tri(2))))).expect("reads");
    let chain = |k: usize| back[k].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>();
    assert_eq!((back.len(), chain(0), chain(1), chain(2)), (3, vec!["head"], vec!["head", "unit"], vec!["head", "unit"]), "the pieces do not come in their groups");
    assert_ne!(back[1].within[1].index, back[2].within[1].index, "the unit placed twice is one group, not two");
    assert!((back[0].place[3] - 30.0).abs() < 1e-12 && (back[1].place[3] - 5.0).abs() < 1e-12, "the pieces stand at {:?} and {:?}", back[0].place, back[1].place);
    assert!(
        (back[1].within[1].place[11] - 50.0).abs() < 1e-12 && (back[2].within[1].place[11] - 80.0).abs() < 1e-12,
        "the units stand at {:?} and {:?}",
        back[1].within[1].place,
        back[2].within[1].place
    );
    assert_eq!(back[1].mesh.verts[1].x, 1.0, "the pin's mesh is baked where it stands, not in its object's frame");
    // where the first pin stands: 5 along the unit's X, which the unit's quarter turn takes to +Y, 50 up
    let p = placed(&back[1]).verts[1];
    assert!(p.x.abs() < 1e-9 && (p.y - 6.0).abs() < 1e-9 && (p.z - 50.0).abs() < 1e-9, "the first pin stands at {p:?}, not (0, 6, 50)");
}
