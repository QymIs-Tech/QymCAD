//! A COORDINATE THAT IS NOT A FINITE NUMBER IS REFUSED, in every mesh format, with where it stands and what it is.
//!
//! Text parses `NaN` and `inf` as numbers, and binary floats carry them too. Without the check such a corner came into
//! the document without a word: an STL with one corner written `NaN` stopped the recognition of its body (the tool's
//! preview and the rebuild both panicked). The refusal names the line in a text file, and the item and the byte in a
//! binary one, with the value - so a person sees what to mend in the file.
use std::io::Write;

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, bytes: &[u8]) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/not-finite-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, bytes).expect("written");
    p.to_string_lossy().into_owned()
}

/// The refusal `key` with its values, as the importers write it.
fn refusal(key: &str, values: &[&str]) -> Option<String> {
    Some(format!("{key}#{}", values.join("\u{1f}")))
}

#[test]
fn stl_text_names_the_line() {
    for word in ["NaN", "nan", "inf", "-inf", "1e39"] {
        let text = format!("solid s\nfacet normal 0 0 1\n outer loop\n  vertex 0 0 2\n  vertex 10 {word} 2\n  vertex 10 10 2\n endloop\nendfacet\nendsolid s\n");
        let p = file(&format!("not-finite-{}.stl", word.trim_start_matches('-')), text.as_bytes());
        assert_eq!(qymcad_io::import_stl_named(&p).err(), refusal("io-stl-not-finite-line", &["5", word]), "an ASCII corner written {word}");
        assert_eq!(qymcad_io::import_stl(&p).err(), refusal("io-stl-not-finite-line", &["5", word]), "the same, as one mesh");
    }
}

#[test]
fn stl_binary_names_the_triangle_and_the_byte() {
    for (bad, said) in [(f32::NAN, "NaN"), (f32::INFINITY, "inf")] {
        let mut b = vec![b' '; 80];
        b.extend(2u32.to_le_bytes());
        for t in 0..2 {
            b.extend([0u8; 12]);
            let y = if t == 1 { bad } else { 0.0 };
            for c in [0.0, 0.0, 0.0, 1.0, y, 0.0, 0.0, 1.0, 0.0f32] {
                b.extend(c.to_le_bytes());
            }
            b.extend(0u16.to_le_bytes());
        }
        // the second triangle, counted from 1; its second corner's y at 84 + 50 + 12 (the normal) + 12 (the first corner) + 4
        let got = qymcad_io::import_stl_named(&file("not-finite-binary.stl", &b)).err();
        assert_eq!(got, refusal("io-stl-not-finite-triangle", &["2", "162", said]), "a binary corner {said}");
    }
}

#[test]
fn obj_names_the_line() {
    let text = "v 0 0 0\nv 1 0 0\nv 0 inf 0\nf 1 2 3\n";
    assert_eq!(qymcad_io::import_obj(&file("not-finite.obj", text.as_bytes())).err(), refusal("io-obj-not-finite-line", &["3", "inf"]));
}

const PLY_HEAD: &str = "element vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n";

#[test]
fn ply_text_names_the_line() {
    // the header is nine lines, the vertices are lines 10 to 12
    let text = format!("ply\nformat ascii 1.0\n{PLY_HEAD}0 0 0\n1 nan 0\n0 1 0\n3 0 1 2\n");
    let p = file("not-finite.ply", text.as_bytes());
    assert_eq!(qymcad_io::import_ply(&p).err(), refusal("io-ply-not-finite-line", &["11", "nan"]));
    assert_eq!(qymcad_io::import_ply_coloured(&p).err(), refusal("io-ply-not-finite-line", &["11", "nan"]), "the same, with its colours");
}

#[test]
fn ply_binary_names_the_vertex_and_the_byte() {
    let head = format!("ply\nformat binary_little_endian 1.0\n{PLY_HEAD}");
    let mut b = head.clone().into_bytes();
    for f in [0.0, 0.0, 0.0, f32::NAN, 0.0, 0.0, 0.0, 1.0, 0.0f32] {
        b.extend(f.to_le_bytes());
    }
    b.push(3);
    for i in [0i32, 1, 2] {
        b.extend(i.to_le_bytes());
    }
    // vertex 1 begins 12 bytes after the header
    let at = (head.len() + 12).to_string();
    assert_eq!(qymcad_io::import_ply(&file("not-finite-binary.ply", &b)).err(), refusal("io-ply-not-finite-vertex", &["1", &at, "NaN"]));
}

/// glTF's JSON cannot hold a NaN; its binary data can.
#[test]
fn gltf_names_the_accessor_item_and_byte() {
    let floats: Vec<u8> = [0.0, 0.0, 0.0, 1.0, f32::NAN, 0.0, 0.0, 1.0, 0.0f32].iter().flat_map(|f| f.to_le_bytes()).collect();
    let b64 = |bytes: &[u8]| {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            for k in 0..4 {
                s.push(if k <= c.len() { A[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
            }
        }
        s
    };
    let gltf = serde_json::json!({
        "asset": {"version": "2.0"}, "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
        "bufferViews": [{"buffer": 0, "byteLength": 36}],
        "buffers": [{"byteLength": 36, "uri": format!("data:application/octet-stream;base64,{}", b64(&floats))}]
    });
    // accessor 0, item 1 (the second corner), its y at byte 16 of the buffer
    let got = qymcad_io::import_gltf(&file("not-finite.gltf", gltf.to_string().as_bytes())).err();
    assert_eq!(got, refusal("io-gltf-not-finite", &["0", "1", "16", "NaN"]));
}

/// A 3MF package holding `model` as its model part.
fn package(name: &str, model: &str) -> String {
    let p = file(name, b"");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&p).expect("created"));
    z.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default()).expect("a part");
    z.write_all(model.as_bytes()).expect("written");
    z.finish().expect("closed");
    p
}

#[test]
fn threemf_names_the_line() {
    // the corner on line 3, the item's placement on line 6, a component's placement on line 5
    let model = |x: &str, item: &str, component: &str| {
        format!(
            "<?xml version=\"1.0\"?><model unit=\"millimeter\"><resources>\n<object id=\"1\" type=\"model\"><mesh><vertices>\n<vertex x=\"{x}\" y=\"0\" z=\"0\"/><vertex x=\"1\" y=\"0\" z=\"0\"/><vertex x=\"0\" y=\"1\" z=\"0\"/></vertices><triangles><triangle v1=\"0\" v2=\"1\" v3=\"2\"/></triangles></mesh></object>\n<object id=\"2\" type=\"model\"><components>\n<component objectid=\"1\"{component}/></components></object>\n</resources><build><item objectid=\"2\"{item}/></build></model>\n"
        )
    };
    assert_eq!(qymcad_io::import_3mf(&package("not-finite-corner.3mf", &model("NaN", "", ""))).err(), refusal("io-3mf-not-finite-line", &["3", "NaN"]), "a corner");
    let item = r#" transform="1 0 0 0 1 0 0 0 1 inf 0 0""#;
    assert_eq!(qymcad_io::import_3mf(&package("not-finite-item.3mf", &model("0", item, ""))).err(), refusal("io-3mf-not-finite-line", &["6", "inf"]), "a build item's placement");
    assert_eq!(qymcad_io::import_3mf(&package("not-finite-component.3mf", &model("0", "", item))).err(), refusal("io-3mf-not-finite-line", &["5", "inf"]), "a component's placement");
}

#[test]
fn amf_names_the_line() {
    let v = |x: &str, y: &str| format!("<vertex><coordinates>\n<x>{x}</x><y>{y}</y><z>0</z></coordinates></vertex>");
    // each vertex's x opens a line of its own: lines 3, 4 and 5, the object starting on line 2
    let object = |x: &str| {
        format!("<object id=\"1\"><mesh><vertices>{}{}{}</vertices><volume><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume></mesh></object>", v("0", "0"), v(x, "0"), v("0", "1"))
    };
    let text = format!("<amf>\n{}</amf>", object("NaN"));
    assert_eq!(qymcad_io::import_amf(&file("not-finite.amf", text.as_bytes())).err(), refusal("io-amf-not-finite-line", &["4", "NaN"]), "a corner");
    // a constellation keeps its instance's placement apart from the corners, which are all finite here; its offsets
    // open line 7, after the object's lines 2 to 5 and the constellation's line 6
    let cons = "<constellation id=\"9\"><instance objectid=\"1\">\n<deltax>NaN</deltax><deltay>0</deltay><deltaz>0</deltaz><rx>0</rx><ry>0</ry><rz>0</rz></instance></constellation>";
    let text = format!("<amf>\n{}\n{cons}</amf>", object("1"));
    assert_eq!(qymcad_io::import_amf(&file("not-finite-instance.amf", text.as_bytes())).err(), refusal("io-amf-not-finite-line", &["7", "NaN"]), "an instance's placement");
}
