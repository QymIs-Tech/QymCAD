//! 3MF: WRITTEN AND READ BACK the same bodies, and the packages other programs write - units, components, a
//! model part where the relationships point - read as they mean.
use std::io::Write;

use qymcad_core::geom::{Mesh, Point3};
use qymcad_io::{export_3mf, export_3mf_tree, import_3mf};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/3mf-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

/// A package of the given parts, the way a slicer would write one.
fn package(name: &str, parts: &[(&str, &str)]) -> String {
    let p = file(name);
    let mut z = zip::ZipWriter::new(std::fs::File::create(&p).expect("created"));
    for (n, body) in parts {
        z.start_file(*n, zip::write::SimpleFileOptions::default()).expect("a part");
        z.write_all(body.as_bytes()).expect("written");
    }
    z.finish().expect("closed");
    p
}

const RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="r" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#;

/// One triangle, as an object with a mesh.
fn tri_object(id: u32) -> String {
    format!(
        r#"<object id="{id}" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>"#
    )
}

fn model(unit: &str, resources: &str, build: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><model unit="{unit}" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources>{resources}</resources><build>{build}</build></model>"#
    )
}

fn tetra(x: f64) -> Mesh {
    let v = |a: f64, b: f64, c: f64| Point3::new(x + a, b, c);
    Mesh { verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 1.0 / 3.0), v(0.1, 0.2, 10.000000000000002)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], ..Default::default() }
}

/// THREE BODIES GO OUT AND THREE COME BACK, every coordinate to the last bit, every triangle on its numbers.
#[test]
fn three_bodies_come_back_to_the_last_bit() {
    let out = [tetra(0.0), tetra(30.0), tetra(-20.0)];
    let p = file("three.3mf");
    export_3mf(&out, &p).expect("the 3MF is written");
    let back = import_3mf(&p).expect("the 3MF reads back");
    assert_eq!(back.len(), 3, "three bodies went out, {} came back", back.len());
    for (a, b) in out.iter().zip(&back) {
        assert_eq!(a.tris, b.mesh.tris, "the triangles came back renumbered");
        for (p, q) in a.verts.iter().zip(&b.mesh.verts) {
            assert!(p.x == q.x && p.y == q.y && p.z == q.z, "a vertex came back moved: {p:?} -> {q:?}");
        }
    }
    assert_eq!(back[2].name, "body_3");
}

/// A MODEL IN INCHES COMES IN IN MILLIMETRES - the unit is the file's word, which STL could never say.
#[test]
fn a_model_in_inches_comes_in_in_millimetres() {
    let p = package("inches.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("inch", &tri_object(1), r#"<item objectid="1"/>"#))]);
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back[0].mesh.verts[1].x, 25.4, "one inch did not come in as 25.4 mm");
}

/// COMPONENTS AND BUILD ITEMS PLACE WHAT THEY CARRY: an object made of one triangle placed twice, then the
/// whole placed by the build - and a model part found where the relationships point, not by its usual name.
#[test]
fn components_and_items_place_what_they_carry() {
    let composite = r#"<object id="2" type="model"><components><component objectid="1"/><component objectid="1" transform="1 0 0 0 1 0 0 0 1 10 0 0"/></components></object>"#;
    let rels = RELS.replace("/3D/3dmodel.model", "/3D/elsewhere.model");
    let p = package(
        "components.3mf",
        &[
            ("_rels/.rels", &rels),
            ("3D/elsewhere.model", &model("millimeter", &format!("{}{composite}", tri_object(1)), r#"<item objectid="2" transform="1 0 0 0 1 0 0 0 1 0 0 5"/>"#)),
        ],
    );
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back.len(), 1, "one build item is one body");
    let m = &back[0].mesh;
    assert_eq!(m.tris.len(), 2, "the triangle placed twice");
    assert!(m.verts.iter().all(|v| v.z == 5.0), "the build item's lift was lost");
    assert!(m.verts.iter().any(|v| v.x == 11.0), "the second component's shift was lost");
}

/// A BROKEN PACKAGE IS REFUSED BY NAME.
#[test]
fn a_broken_package_is_refused_by_name() {
    std::fs::write(file("not-zip.3mf"), "not a zip").expect("written");
    assert_eq!(import_3mf(&file("not-zip.3mf")).err().as_deref(), Some("io-3mf-not-3mf"));
    let no_model = package("no-model.3mf", &[("_rels/.rels", RELS)]);
    assert_eq!(import_3mf(&no_model).err().as_deref(), Some("io-3mf-no-model"));
    let bad = tri_object(1).replace(r#"v3="2""#, r#"v3="9""#);
    let bad_index = package("bad-index.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &bad, r#"<item objectid="1"/>"#))]);
    assert_eq!(import_3mf(&bad_index).err().as_deref(), Some("io-3mf-bad-index"));
    let furlong = package("furlong.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("furlong", &tri_object(1), r#"<item objectid="1"/>"#))]);
    assert_eq!(import_3mf(&furlong).err().as_deref(), Some("io-3mf-unknown-unit#furlong"));
    assert!(export_3mf(&[], &file("nothing.3mf")).is_err(), "an empty set was written as a file");
}

/// A SLICER'S PROJECT NAMES ITS PARTS where the model does not: Bambu Studio writes the part names into
/// `Metadata/model_settings.config`, a part by the id of its object. The owner's print head came with 107 names there
/// and none in the model.
#[test]
fn a_slicer_project_names_its_parts() {
    let resources = format!(
        r#"{}{}<object id="3" type="model"><components><component objectid="1"/><component objectid="2" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></components></object>"#,
        tri_object(1),
        tri_object(2)
    );
    let config = r#"<?xml version="1.0" encoding="UTF-8"?><config><object id="3"><metadata key="name" value="head.step"/><part id="1" subtype="normal_part"><metadata key="name" value="bolt"/></part><part id="2" subtype="normal_part"><metadata key="name" value="plate"/></part></object></config>"#;
    let p = package("slicer.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="3"/>"#)), ("Metadata/model_settings.config", config)]);
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["bolt", "plate"], "the parts came without the names the project gives them");
}

/// A NAMED PART STAYS IN ITS OWN COORDINATES, and its component's transform is where it stands - a part to move and
/// mate, not a mesh baked into place. The plate's component is shifted by 5 along X.
#[test]
fn a_named_part_keeps_its_own_coordinates_and_its_place() {
    let resources = format!(
        r#"{}{}<object id="3" type="model"><components><component objectid="1"/><component objectid="2" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></components></object>"#,
        tri_object(1),
        tri_object(2)
    );
    let config = r#"<?xml version="1.0" encoding="UTF-8"?><config><object id="3"><part id="1"><metadata key="name" value="bolt"/></part><part id="2"><metadata key="name" value="plate"/></part></object></config>"#;
    let p = package("placed.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="3"/>"#)), ("Metadata/model_settings.config", config)]);
    let back = import_3mf(&p).expect("reads");
    let plate = &back[1];
    assert_eq!((plate.mesh.verts[0].x, plate.mesh.verts[1].x), (0.0, 1.0), "the plate is baked where it stands, not kept at its own zero");
    assert_eq!([plate.place[3], plate.place[7], plate.place[11]], [5.0, 0.0, 0.0], "the component's shift is not the part's place");
}

/// AN OBJECT COMES IN THE COLOUR IT NAMES, as sRGB bytes: by its `pid` and `pindex`, from the core's `basematerials`
/// (`displaycolor`) or from a colour group of the materials extension - the two ways a 3MF colours a part.
#[test]
fn an_object_comes_in_its_colour() {
    let coloured = |id: u32, pid: u32, pindex: u32| tri_object(id).replace(r#"type="model""#, &format!(r#"type="model" pid="{pid}" pindex="{pindex}""#));
    let resources = format!(
        r##"<basematerials id="9"><base name="red" displaycolor="#CC1A1A"/><base name="blue" displaycolor="#1A33E6FF"/></basematerials><m:colorgroup id="8"><m:color color="#447766"/></m:colorgroup>{}{}{}{}"##,
        coloured(1, 9, 1),
        coloured(2, 9, 0),
        coloured(3, 8, 0),
        tri_object(4)
    );
    let build = r#"<item objectid="1"/><item objectid="2"/><item objectid="3"/><item objectid="4"/>"#;
    let p = package("coloured.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, build))]);
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back.iter().map(|m| m.color).collect::<Vec<_>>(), [Some([26, 51, 230]), Some([204, 26, 26]), Some([68, 119, 102]), None]);
}

/// AN OBJECT OF PARTS THE MODEL NAMES comes in a piece per part, as one whose parts a slicer's project names: each part
/// under its name, in its own coordinates, placed by its component. Parts nobody names stay one piece (see
/// `components_and_items_place_what_they_carry`).
#[test]
fn an_object_of_parts_the_model_names_comes_in_a_piece_a_part() {
    let named = |id: u32, name: &str| tri_object(id).replace(r#"type="model""#, &format!(r#"type="model" name="{name}""#));
    let resources = format!(
        r#"{}{}<object id="3" type="model" name="head"><components><component objectid="1"/><component objectid="2" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></components></object>"#,
        named(1, "bolt &amp; nut"),
        named(2, "plate")
    );
    let p = package("model-named.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="3"/>"#))]);
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["bolt & nut", "plate"], "an object of named parts came in as one piece");
    assert_eq!((back[1].mesh.verts[1].x, back[1].place[3]), (1.0, 5.0), "the plate is not at its own zero, placed by its component");
}

/// A TREE GOES OUT AS OBJECTS OF PARTS: an object per part under its name - escaped as XML wants - and in its
/// colour, at its own zero; an object of components for the head and one for its unit, each placing what it holds; a
/// repeated part placing the object the file holds once. It reads back a piece a part, named, coloured and placed.
#[test]
fn a_tree_goes_out_as_an_object_of_parts() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    use std::io::Read;
    let p = file("tree.3mf");
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
        node("pin & nut", Some(3), at(5.0, 0.0), Some(3), None, blue),
    ];
    export_3mf_tree(
        &nodes,
        &[
            ExportMesh { body: 1, mesh: tetra(0.0), tri_colors: Vec::new() },
            ExportMesh { body: 2, mesh: tetra(0.0), tri_colors: Vec::new() },
            ExportMesh { body: 3, mesh: tetra(0.0), tri_colors: Vec::new() },
        ],
        &p,
    )
    .expect("the tree is written");
    let mut xml = String::new();
    zip::ZipArchive::new(std::fs::File::open(&p).expect("the package")).expect("a zip").by_name("3D/3dmodel.model").expect("the model").read_to_string(&mut xml).expect("read");
    assert_eq!(xml.matches("<object ").count(), 4, "not an object per part and one of components for the head and the unit: {xml}");
    assert!(xml.contains(r##"displaycolor="#CC1A1A""##) && xml.contains(r##"displaycolor="#1A33E6""##), "the colours are not in the model");
    assert!(xml.contains(r#"name="pin &amp; nut""#), "a name is not escaped");
    assert!(xml.contains(r#"transform="1 0 0 0 1 0 0 0 1 5 0 0""#) && xml.contains(r#"transform="1 0 0 0 1 0 0 0 1 0 0 50""#), "the pin is not placed in its unit, and the unit in the head: {xml}");
    let back = import_3mf(&p).expect("the tree reads back");
    assert_eq!(back.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["plate", "plate", "pin & nut"]);
    assert_eq!(back.iter().map(|b| b.color).collect::<Vec<_>>(), [red, red, blue]);
    assert_eq!([back[1].place[3], back[2].place[3], back[2].within[0].place[11]], [30.0, 5.0, 50.0], "the parts are not placed where they stand");
    assert_eq!(back[2].mesh.verts, tetra(0.0).verts, "the pin is not at its own zero, to the last bit");
}

/// A FACE OF A COLOUR OF ITS OWN GOES OUT ON ITS TRIANGLES: the plate's object is red, its last triangle names green in
/// the materials of the model (`pid` and `p1`) - and it reads back a colour per triangle.
#[test]
fn a_face_of_its_own_colour_goes_out_on_its_triangles() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    use std::io::Read;
    let p = file("faces.3mf");
    let (red, green) = ([204, 26, 26], [26, 204, 26]);
    let place = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let nodes = [
        ExportNode { name: "head".into(), parent: None, place, body: None, same_as: None, color: None, face_colors: Vec::new() },
        ExportNode { name: "plate".into(), parent: Some(0), place, body: Some(1), same_as: None, color: Some(red), face_colors: Vec::new() },
    ];
    export_3mf_tree(&nodes, &[ExportMesh { body: 1, mesh: tetra(0.0), tri_colors: vec![Some(red), Some(red), Some(red), Some(green)] }], &p).expect("the tree is written");
    let mut xml = String::new();
    zip::ZipArchive::new(std::fs::File::open(&p).expect("the package")).expect("a zip").by_name("3D/3dmodel.model").expect("the model").read_to_string(&mut xml).expect("read");
    assert!(xml.contains(r##"displaycolor="#1ACC1A""##), "the green is not among the materials: {xml}");
    assert_eq!(xml.matches(" p1=").count(), 1, "not exactly the green triangle names a colour of its own: {xml}");
    let back = import_3mf(&p).expect("reads back");
    assert_eq!(back[0].color, Some(red), "the plate does not come back in its own colour");
    assert_eq!(back[0].tri_colors, [red, red, red, green], "the triangles do not come back in their colours");
}

/// A PIECE COLOURED OVER ALL ROUND comes back so: the plate's object is red and every one of its triangles names green -
/// it reads back red with every triangle green, not red all over.
#[test]
fn a_piece_coloured_over_all_round_comes_back_so() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    let p = file("over.3mf");
    let (red, green) = ([204, 26, 26], [26, 204, 26]);
    let place = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let nodes = [
        ExportNode { name: "head".into(), parent: None, place, body: None, same_as: None, color: None, face_colors: Vec::new() },
        ExportNode { name: "plate".into(), parent: Some(0), place, body: Some(1), same_as: None, color: Some(red), face_colors: Vec::new() },
    ];
    export_3mf_tree(&nodes, &[ExportMesh { body: 1, mesh: tetra(0.0), tri_colors: vec![Some(green); 4] }], &p).expect("the tree is written");
    let back = import_3mf(&p).expect("reads back");
    assert_eq!(back[0].color, Some(red), "the plate does not come back in its own colour");
    assert_eq!(back[0].tri_colors, [green; 4], "the triangles do not come back in the colour they are given");
}

/// A closed tetrahedron with its triangles facing out, as an object with a mesh.
fn tet_object(id: u32) -> String {
    format!(
        r#"<object id="{id}" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/><vertex x="0" y="0" z="1"/></vertices><triangles><triangle v1="0" v2="2" v3="1"/><triangle v1="0" v2="1" v3="3"/><triangle v1="0" v2="3" v3="2"/><triangle v1="1" v2="2" v3="3"/></triangles></mesh></object>"#
    )
}

/// The volume a closed mesh holds, positive when its triangles face out.
fn volume(m: &Mesh) -> f64 {
    m.tris
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|k| m.verts[k as usize]);
            a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)
        })
        .sum::<f64>()
        / 6.0
}

/// A MIRRORED COMPONENT KEEPS ITS INSIDE IN: the standard says a transform with a negative determinant "MUST NOT change
/// the sign of its volume" (3MF Core, 3.3), so the triangles of a mirrored mesh are turned over. The closed tetrahedron,
/// mirrored along x, holds a positive volume.
#[test]
fn a_mirrored_component_keeps_its_inside_in() {
    let resources = format!(r#"{}<object id="2" type="model"><components><component objectid="1" transform="-1 0 0 0 1 0 0 0 1 0 0 0"/></components></object>"#, tet_object(1));
    let p = package("mirrored.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="2"/>"#))]);
    let back = import_3mf(&p).expect("reads");
    assert!(back[0].mesh.verts.iter().any(|v| v.x < 0.0), "the mirror was lost");
    assert!(volume(&back[0].mesh) > 0.0, "the mirrored tetrahedron came in inside out: volume {}", volume(&back[0].mesh));
}

/// A PART SCALED BY ITS COMPONENT STANDS RIGIDLY AND CARRIES THE SCALE IN ITS MESH: a place holds a turn and a shift
/// only, so the bolt, drawn at 1 and placed at twice its size 5 along x, stands at a place with no scale in it, and its
/// own mesh is twice as large.
#[test]
fn a_part_scaled_by_its_component_stands_rigidly() {
    let named = |id: u32, name: &str| tri_object(id).replace(r#"type="model""#, &format!(r#"type="model" name="{name}""#));
    let resources = format!(
        r#"{}{}<object id="3" type="model" name="head"><components><component objectid="1" transform="2 0 0 0 2 0 0 0 2 5 0 0"/><component objectid="2"/></components></object>"#,
        named(1, "bolt"),
        named(2, "plate")
    );
    let p = package("scaled-part.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="3"/>"#))]);
    let back = import_3mf(&p).expect("reads");
    let bolt = back.iter().find(|m| m.name == "bolt").expect("the bolt came in");
    let p = bolt.place;
    assert!([p[0], p[5], p[10]].iter().all(|d| (d - 1.0).abs() < 1e-12) && p[3] == 5.0, "the bolt's place carries its component's scale: {p:?}");
    assert!((bolt.mesh.verts[1].x - 2.0).abs() < 1e-12, "the bolt's mesh does not carry the scale: {:?}", bolt.mesh.verts);
}

/// A PART MADE OF PARTS COMES IN AS THEIR GROUP, to any depth: the head holds the unit 50 up, and the unit holds the pin 5
/// along and the plate - the pin and the plate come in within the unit, each at its place in it, and the unit stands
/// where the head puts it.
#[test]
fn a_part_made_of_parts_comes_in_as_their_group() {
    let named = |id: u32, name: &str| tri_object(id).replace(r#"type="model""#, &format!(r#"type="model" name="{name}""#));
    let resources = format!(
        r#"{}{}<object id="3" type="model" name="unit"><components><component objectid="1" transform="1 0 0 0 1 0 0 0 1 5 0 0"/><component objectid="2"/></components></object><object id="4" type="model" name="head"><components><component objectid="3" transform="1 0 0 0 1 0 0 0 1 0 0 50"/></components></object>"#,
        named(1, "pin"),
        named(2, "plate")
    );
    let p = package("nested.3mf", &[("_rels/.rels", RELS), ("3D/3dmodel.model", &model("millimeter", &resources, r#"<item objectid="4"/>"#))]);
    let back = import_3mf(&p).expect("reads");
    assert_eq!(back.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["pin", "plate"], "the unit came in as one piece");
    let chain = |k: usize| back[k].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>();
    assert_eq!((chain(0), chain(1)), (vec!["unit"], vec!["unit"]), "the pin and the plate do not come in within the unit");
    assert_eq!(back[0].within[0].index, back[1].within[0].index, "the unit is not one group for both");
    assert_eq!(back[0].within[0].place[11], 50.0, "the unit does not stand 50 up");
    assert_eq!(back[0].place[3], 5.0, "the pin does not stand 5 along in the unit");
}

/// A TREE GOES OUT WITH ITS SUBASSEMBLIES, as 3MF holds them: a subassembly is an object of components under its name,
/// placed by a component of its parent (3MF Core, 4.1 - a component may refer to an object of components). The head
/// holds a plate and the unit 50 up; the unit holds the pin 5 along; a second unit, 80 up, repeats the first. It reads
/// back with the pin within the unit, the unit 50 up and the pin 5 along in it; the repeated unit is the same object
/// placed twice, not a copy of its parts.
#[test]
fn a_tree_goes_out_with_its_subassemblies() {
    use qymcad_core::model::{ExportMesh, ExportNode};
    use std::io::Read;
    let p = file("levels.3mf");
    let at = |x: f64, z: f64| [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z];
    let node = |name: &str, parent: Option<usize>, place: [f64; 12], body: Option<u64>, same_as: Option<usize>| ExportNode {
        name: name.into(),
        parent,
        place,
        body,
        same_as,
        color: None,
        face_colors: Vec::new(),
    };
    let nodes = [
        node("head", None, at(0.0, 0.0), None, None),
        node("plate", Some(0), at(0.0, 0.0), Some(1), None),
        node("unit", Some(0), at(0.0, 50.0), None, None),
        node("pin", Some(2), at(5.0, 0.0), Some(2), None),
        node("unit", Some(0), at(0.0, 80.0), None, Some(2)),
        node("pin", Some(4), at(5.0, 0.0), Some(3), Some(3)),
    ];
    export_3mf_tree(
        &nodes,
        &[
            ExportMesh { body: 1, mesh: tetra(0.0), tri_colors: Vec::new() },
            ExportMesh { body: 2, mesh: tetra(0.0), tri_colors: Vec::new() },
            ExportMesh { body: 3, mesh: tetra(0.0), tri_colors: Vec::new() },
        ],
        &p,
    )
    .expect("the tree is written");
    let mut xml = String::new();
    zip::ZipArchive::new(std::fs::File::open(&p).expect("the package")).expect("a zip").by_name("3D/3dmodel.model").expect("the model").read_to_string(&mut xml).expect("read");
    assert_eq!(xml.matches("<object ").count(), 4, "not an object per part, one for the unit and one for the head: {xml}");
    assert!(xml.contains(r#"name="unit" type="model">"#) && xml.matches("<components>").count() == 2, "the unit is not an object of components: {xml}");
    let back = import_3mf(&p).expect("the tree reads back");
    assert_eq!(back.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["plate", "pin", "pin"]);
    let chain = |k: usize| back[k].within.iter().map(|g| g.name.as_str()).collect::<Vec<_>>();
    assert_eq!((chain(0), chain(1), chain(2)), (vec![], vec!["unit"], vec!["unit"]), "the pins do not come in within their units");
    assert_ne!(back[1].within[0].index, back[2].within[0].index, "the two units came in as one group");
    assert_eq!([back[1].within[0].place[11], back[2].within[0].place[11], back[1].place[3]], [50.0, 80.0, 5.0], "the units or the pin do not stand where they were");
}
