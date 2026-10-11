//! glTF: WRITTEN AND READ BACK the same bodies in their places - and the two conventions of glTF that are not
//! ours, metres and +Y up, checked by the raw numbers in the file rather than by a round trip that would hide
//! a mistake made twice.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_io::{export_glb, export_glb_tree, import_gltf};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, bytes: Option<&[u8]>) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/gltf-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    if let Some(b) = bytes {
        std::fs::write(&p, b).expect("written");
    }
    p.to_string_lossy().into_owned()
}

fn tetra(x: f64, z: f64) -> Mesh {
    let v = |a: f64, b: f64, c: f64| Point3::new(x + a, b, z + c);
    Mesh { verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], ..Default::default() }
}

/// glTF stores positions as 32-bit floats, so a millimetre model comes back to about 1e-7 of its size.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-4 * (1.0 + b.abs())
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

/// The JSON chunk of a `.glb`, for looking at what was really written.
fn json_of(path: &str) -> serde_json::Value {
    let b = std::fs::read(path).expect("the file is there");
    assert_eq!(&b[0..4], b"glTF", "not a binary glTF");
    let len = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
    serde_json::from_slice(&b[20..20 + len]).expect("the JSON chunk parses")
}

/// THREE BODIES GO OUT AND THREE COME BACK, each where it stood, each triangle on its own numbers.
#[test]
fn three_bodies_come_back_in_their_places() {
    let p = file("three.glb", None);
    let out = [tetra(0.0, 0.0), tetra(30.0, 50.0), tetra(-20.0, 5.0)];
    export_glb(&out, &p).expect("the GLB is written");
    let back = import_gltf(&p).expect("the GLB reads back");
    assert_eq!(back.len(), 3, "three bodies went out, {} came back", back.len());
    for (a, b) in out.iter().zip(&back) {
        assert_eq!(a.tris, b.mesh.tris, "the triangles came back renumbered");
        for (p, q) in a.verts.iter().zip(&b.mesh.verts) {
            assert!(close(p.x, q.x) && close(p.y, q.y) && close(p.z, q.z), "a vertex came back moved: {p:?} -> {q:?}");
        }
    }
    assert_eq!(back[1].name, "body_2");
}

/// METRES AND +Y UP, read off the file itself: a body 10 mm tall along our Z is 0.010 tall along glTF's Y.
///
/// A round trip alone would not catch this: a turn made the wrong way on the way out and the same wrong way back
/// comes home looking right, and the part lies on its back in every other program.
#[test]
fn the_file_is_in_metres_with_y_up() {
    let p = file("upright.glb", None);
    export_glb(&[tetra(0.0, 0.0)], &p).expect("written");
    let doc = json_of(&p);
    let max: Vec<f64> = doc["accessors"][0]["max"].as_array().expect("POSITION carries its max").iter().map(|v| v.as_f64().unwrap()).collect();
    let min: Vec<f64> = doc["accessors"][0]["min"].as_array().expect("and its min").iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((max[1] - 0.010).abs() < 1e-7, "the height is not along +Y in metres: max {max:?}");
    assert!((max[0] - 0.010).abs() < 1e-7 && (min[2] + 0.010).abs() < 1e-7, "our +Y is not glTF's -Z: min {min:?}, max {max:?}");
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { A[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// A SCENE AS OTHER PROGRAMS WRITE IT: a parent that moves, a child that turns, a buffer inside the file, a
/// triangle with no indices and a strip.
#[test]
fn a_scene_tree_places_its_meshes() {
    // one triangle in glTF's own frame (metres, +Y up): along X, then up
    let tri = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.001, 0.0]);
    let strip = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.0, -0.001, 0.001, 0.0, -0.001]);
    let mut buf = tri.clone();
    buf.extend(&strip);
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let gltf = serde_json::json!({
        "asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [
            {"name": "carrier", "translation": [0.1, 0.0, 0.0], "children": [1, 2]},
            {"name": "turned", "mesh": 0, "rotation": [0.0, s, 0.0, s]},
            {"name": "strip", "mesh": 1}
        ],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}, {"primitives": [{"attributes": {"POSITION": 1}, "mode": 5}]}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"}
        ],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 48}],
        "buffers": [{"byteLength": buf.len(), "uri": format!("data:application/octet-stream;base64,{}", b64(&buf))}]
    });
    let back = import_gltf(&file("tree.gltf", Some(gltf.to_string().as_bytes()))).expect("the scene reads");
    assert_eq!(back.len(), 2, "two nodes carry meshes");
    assert_eq!(back[0].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["carrier"], "the carrier does not come as the group its children stand in");
    // the turned triangle: a quarter turn about glTF's Y takes +X to -Z, which is our +Y; then 0.1 m along X
    let t = &placed(&back[0]).verts;
    assert!(close(t[1].x, 100.0) && close(t[1].y, 1.0) && close(t[1].z, 0.0), "the child's turn or the parent's move was lost: {:?}", t[1]);
    assert!(close(t[2].z, 1.0), "glTF's +Y did not come in as our +Z: {:?}", t[2]);
    assert_eq!(back[1].mesh.tris.len(), 2, "a strip of four points is two triangles");
}

/// A BUFFER BESIDE THE FILE is read from beside the file; a missing one is named.
#[test]
fn a_buffer_beside_the_file_is_read_and_a_missing_one_named() {
    let bin = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.001, 0.0]);
    file("beside.bin", Some(&bin));
    let doc = |uri: &str| {
        serde_json::json!({
            "asset": {"version": "2.0"}, "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
            "bufferViews": [{"buffer": 0, "byteLength": 36}], "buffers": [{"byteLength": 36, "uri": uri}]
        })
        .to_string()
    };
    let back = import_gltf(&file("beside.gltf", Some(doc("beside.bin").as_bytes()))).expect("reads");
    assert_eq!(back[0].mesh.tris.len(), 1);
    let err = import_gltf(&file("lost.gltf", Some(doc("nowhere.bin").as_bytes()))).err();
    assert_eq!(err.as_deref(), Some("io-gltf-missing-buffer#nowhere.bin"));
}

/// A BROKEN FILE IS REFUSED BY NAME.
#[test]
fn a_broken_file_is_refused_by_name() {
    assert_eq!(import_gltf(&file("text.gltf", Some(b"this is not json"))).err().as_deref(), Some("io-gltf-not-gltf"));
    let no_mesh = serde_json::json!({"asset": {"version": "2.0"}, "nodes": [{"name": "empty"}]}).to_string();
    assert_eq!(import_gltf(&file("empty.gltf", Some(no_mesh.as_bytes()))).err().as_deref(), Some("io-gltf-no-meshes"));
    let bin = floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let bad = serde_json::json!({
        "asset": {"version": "2.0"}, "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}, {"bufferView": 1, "componentType": 5121, "count": 3, "type": "SCALAR"}],
        "bufferViews": [{"buffer": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 3}],
        "buffers": [{"byteLength": 39, "uri": format!("data:application/octet-stream;base64,{}", b64(&[bin, vec![0, 1, 9]].concat()))}]
    })
    .to_string();
    assert_eq!(import_gltf(&file("bad-index.gltf", Some(bad.as_bytes()))).err().as_deref(), Some("io-gltf-bad-index"));
    assert!(export_glb(&[], &file("nothing.glb", None)).is_err(), "an empty set was written as a file");
}

/// A NODE TAKES THE COLOUR OF ITS MATERIAL. glTF writes `baseColorFactor` in linear light: 0.6038 is sRGB 0.800 and
/// 0.0100 is sRGB 0.0999, a red part, and they come in as the bytes a colour picker shows - 204 and 25.
#[test]
fn a_node_takes_the_colour_of_its_material() {
    let tri = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.001, 0.0]);
    let gltf = serde_json::json!({
        "asset": {"version": "2.0"}, "nodes": [{"name": "plate", "mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 0}]}],
        "materials": [{"pbrMetallicRoughness": {"baseColorFactor": [0.6038, 0.0100, 0.0100, 1.0]}}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
        "bufferViews": [{"buffer": 0, "byteLength": 36}],
        "buffers": [{"byteLength": 36, "uri": format!("data:application/octet-stream;base64,{}", b64(&tri))}]
    });
    let back = import_gltf(&file("coloured.gltf", Some(gltf.to_string().as_bytes()))).expect("reads");
    assert_eq!(back[0].color, Some([204, 25, 25]), "the material's colour was dropped, or not turned into sRGB");
}

/// A WRITER'S PLACEHOLDER IS NO NAME: a node called "empty_7" comes in unnamed, to be named after its file.
#[test]
fn a_placeholder_name_is_no_name() {
    let tri = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.001, 0.0]);
    let gltf = serde_json::json!({
        "asset": {"version": "2.0"}, "nodes": [{"name": "empty_7", "mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
        "bufferViews": [{"buffer": 0, "byteLength": 36}],
        "buffers": [{"byteLength": 36, "uri": format!("data:application/octet-stream;base64,{}", b64(&tri))}]
    });
    let back = import_gltf(&file("placeholder.gltf", Some(gltf.to_string().as_bytes()))).expect("reads");
    assert_eq!(back[0].name, "", "a placeholder came in as a name");
}

/// A TREE GOES OUT AS THE SCENE'S NODES: a subassembly and its parts under their names, each placed in its parent in
/// glTF's frame, a repeated part carrying the mesh the file holds once, every part in the material of its colour -
/// linear in the file - and it reads back with the names, the colours and the places.
#[test]
fn a_tree_goes_out_as_nodes_with_names_and_colours() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    let p = file("tree.glb", None);
    let at = |x: f64, z: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z];
    let node = |name: &str, parent: Option<usize>, place: [f64; 12], body: Option<u64>, same_as: Option<usize>, color: Option<[u8; 3]>| ExportNode {
        name: name.into(),
        parent,
        place,
        body,
        same_as,
        color,
        face_colors: Vec::new(),
    };
    let (red, blue) = (Some([204, 26, 26]), Some([26, 51, 230]));
    let nodes = [
        node("head", None, at(0.0, 0.0), None, None, None),
        node("plate", Some(0), at(0.0, 0.0), Some(1), None, red),
        node("plate", Some(0), at(30.0, 0.0), Some(2), Some(1), red),
        node("unit", Some(0), at(0.0, 50.0), None, None, None),
        node("pin", Some(3), at(5.0, 0.0), Some(3), None, blue),
    ];
    export_glb_tree(
        &nodes,
        &[
            ExportMesh { body: 1, mesh: tetra(0.0, 0.0), tri_colors: Vec::new() },
            ExportMesh { body: 2, mesh: tetra(0.0, 0.0), tri_colors: Vec::new() },
            ExportMesh { body: 3, mesh: tetra(0.0, 0.0), tri_colors: Vec::new() },
        ],
        &p,
    )
    .expect("the tree is written");
    let j = json_of(&p);
    let names: Vec<&str> = j["nodes"].as_array().expect("nodes").iter().map(|n| n["name"].as_str().expect("a name")).collect();
    assert_eq!(names, ["head", "plate", "plate", "unit", "pin"]);
    assert_eq!(j["scenes"][0]["nodes"], serde_json::json!([0]), "the scene does not stand on the head");
    assert_eq!(j["nodes"][0]["children"], serde_json::json!([1, 2, 3]));
    assert_eq!(j["nodes"][3]["children"], serde_json::json!([4]));
    assert_eq!(j["meshes"].as_array().map(Vec::len), Some(2), "the repeated plate carries a mesh of its own");
    assert_eq!(j["nodes"][1]["mesh"], j["nodes"][2]["mesh"]);
    // THE PLACE IN glTF'S FRAME: 50 mm up our Z is 0.05 up glTF's Y
    let m: Vec<f64> = j["nodes"][3]["matrix"].as_array().expect("the unit's matrix").iter().filter_map(|v| v.as_f64()).collect();
    assert!(close(m[13], 0.05) && m[12].abs() < 1e-12 && m[14].abs() < 1e-12, "the unit stands at {m:?}");
    let material = j["meshes"][0]["primitives"][0]["material"].as_u64().expect("the plate's material") as usize;
    let red_linear = j["materials"][material]["pbrMetallicRoughness"]["baseColorFactor"][0].as_f64().expect("a factor");
    assert!((red_linear - 0.6038).abs() < 1e-3, "the plate's red is written as {red_linear}, not in linear light");
    let back = import_gltf(&p).expect("the tree reads back");
    assert_eq!(back.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["plate", "plate", "pin"]);
    assert_eq!(back.iter().map(|b| b.color).collect::<Vec<_>>(), [red, red, blue]);
    let lo = |k: usize| placed(&back[k]).bounds().expect("a mesh").min;
    assert!(close(lo(1).x, 30.0) && close(lo(2).x, 5.0) && close(lo(2).z, 50.0), "the parts come back at {:?}, {:?}", lo(1), lo(2));
}

/// A FACE OF A COLOUR OF ITS OWN GOES OUT AS A PRIMITIVE OF ITS OWN: the plate red, its last triangle green - one mesh,
/// two primitives, each with its material, the body's colour first - and it reads back a colour per triangle.
#[test]
fn a_face_of_its_own_colour_goes_out_as_a_primitive_of_its_own() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    let p = file("faces.glb", None);
    let (red, green) = ([204, 26, 26], [26, 204, 26]);
    let place = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let nodes = [
        ExportNode { name: "head".into(), parent: None, place, body: None, same_as: None, color: None, face_colors: Vec::new() },
        ExportNode { name: "plate".into(), parent: Some(0), place, body: Some(1), same_as: None, color: Some(red), face_colors: Vec::new() },
    ];
    export_glb_tree(&nodes, &[ExportMesh { body: 1, mesh: tetra(0.0, 0.0), tri_colors: vec![Some(red), Some(red), Some(red), Some(green)] }], &p).expect("the tree is written");
    let j = json_of(&p);
    let prims = j["meshes"][0]["primitives"].as_array().expect("primitives");
    assert_eq!(prims.len(), 2, "the green triangle does not go out as a primitive of its own");
    assert_ne!(prims[0]["material"], prims[1]["material"], "the two primitives share a material");
    let back = import_gltf(&p).expect("reads back");
    assert_eq!(back[0].color, Some(red), "the plate does not come back in its own colour");
    assert_eq!(back[0].tri_colors, [red, red, red, green], "the triangles do not come back in their colours");
}

/// A TREE COMES BACK AS ITS GROUPS: a node that holds others comes back as a group the pieces under it stand in, with
/// its name and its place, and every piece in its own node's frame - the head, the unit 50 mm up, the pin 5 mm along
/// it - not baked where it stands.
#[test]
fn a_tree_comes_back_as_its_groups() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    let p = file("groups.glb", None);
    let at = |x: f64, z: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z];
    let node = |name: &str, parent: Option<usize>, place: [f64; 12], body: Option<u64>| ExportNode { name: name.into(), parent, place, body, same_as: None, color: None, face_colors: Vec::new() };
    let nodes = [
        node("head", None, at(0.0, 0.0), None),
        node("plate", Some(0), at(30.0, 0.0), Some(1)),
        node("unit", Some(0), at(0.0, 50.0), None),
        node("pin", Some(2), at(5.0, 0.0), Some(2)),
    ];
    export_glb_tree(&nodes, &[ExportMesh { body: 1, mesh: tetra(0.0, 0.0), tri_colors: Vec::new() }, ExportMesh { body: 2, mesh: tetra(0.0, 0.0), tri_colors: Vec::new() }], &p)
        .expect("the tree is written");
    let back = import_gltf(&p).expect("the tree reads back");
    let chain = |k: usize| back[k].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>();
    assert_eq!((chain(0), chain(1)), (vec!["head"], vec!["head", "unit"]), "the pieces do not come back in their groups");
    assert_eq!(back[0].within[0].index, back[1].within[0].index, "the head is not one group for both pieces");
    let unit = back[1].within[1].place;
    assert!(close(unit[11], 50.0) && unit[3].abs() < 1e-9 && unit[7].abs() < 1e-9, "the unit stands at {unit:?}");
    assert!(close(back[0].place[3], 30.0) && close(back[1].place[3], 5.0), "the pieces stand at {:?} and {:?}", back[0].place, back[1].place);
    let lo = back[1].mesh.bounds().expect("a mesh").min;
    assert!(lo.x.abs() < 1e-4 && lo.z.abs() < 1e-4, "the pin's mesh is baked where it stands, not in its own frame: {lo:?}");
}

/// One triangle in glTF's own frame (metres, +Y up), a millimetre along X and a millimetre up, as a scene of `nodes`
/// whose node with the mesh holds mesh 0.
fn one_triangle(nodes: serde_json::Value) -> serde_json::Value {
    let tri = floats(&[0.0, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.001, 0.0]);
    serde_json::json!({
        "asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}],
        "buffers": [{"byteLength": tri.len(), "uri": format!("data:application/octet-stream;base64,{}", b64(&tri))}]
    })
}

/// A NODE THAT SCALES KEEPS THE TREE: a place holds a turn and a shift only, so a node's scale is carried into the
/// meshes and places under it, and the carrier still comes as the group its child stands in. The carrier doubles and
/// stands 0.1 m along x; its child stands 1 mm along x in it and holds a triangle 1 mm long - 0.1 m and twice 2 mm, 104
/// mm, is where the triangle's far corner stands.
#[test]
fn a_node_that_scales_keeps_the_tree() {
    let gltf = one_triangle(serde_json::json!([
        {"name": "carrier", "translation": [0.1, 0.0, 0.0], "scale": [2.0, 2.0, 2.0], "children": [1]},
        {"name": "leaf", "mesh": 0, "translation": [0.001, 0.0, 0.0]}
    ]));
    let back = import_gltf(&file("scaled.gltf", Some(gltf.to_string().as_bytes()))).expect("the scene reads");
    assert_eq!(back[0].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["carrier"], "a node that scales flattened the tree");
    for place in [back[0].place, back[0].within[0].place] {
        let columns: Vec<f64> = (0..3).map(|c| (0..3).map(|r| place[r * 4 + c].powi(2)).sum::<f64>()).collect();
        assert!(columns.iter().all(|l| close(*l, 1.0)), "a place carries a scale: {place:?}");
    }
    let t = &placed(&back[0]).verts;
    assert!(close(t[1].x, 104.0) && close(t[2].z, 2.0), "the scaled triangle does not stand where the scene puts it: {t:?}");
}

/// A MIRRORING NODE TURNS ITS TRIANGLES OVER: glTF takes the winding as counterclockwise where the determinant of a
/// node's global transform is positive and clockwise where it is negative, so a mesh taken through a mirror keeps its
/// front facing out only turned over. The triangle faces glTF's +Z, our -Y, and mirrored along x it still does.
#[test]
fn a_mirroring_node_turns_its_triangles_over() {
    let gltf = one_triangle(serde_json::json!([{"name": "mirror", "mesh": 0, "scale": [-1.0, 1.0, 1.0]}]));
    let back = import_gltf(&file("mirrored.gltf", Some(gltf.to_string().as_bytes()))).expect("the scene reads");
    let m = placed(&back[0]);
    let [a, b, c] = m.tris[0].map(|k| m.verts[k as usize]);
    let facing = (b.z - a.z) * (c.x - a.x) - (b.x - a.x) * (c.z - a.z); // y of (b - a) x (c - a)
    assert!(m.verts.iter().any(|v| v.x < 0.0), "the mirror was lost");
    assert!(facing < 0.0, "the mirrored triangle came in facing away: {facing}");
}
