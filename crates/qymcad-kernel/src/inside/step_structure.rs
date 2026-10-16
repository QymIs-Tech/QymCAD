//! A STEP ASSEMBLY COMES IN AS ITS AUTHOR BUILT IT: the tree of subassemblies and parts, their names - Cyrillic too -
//! where every occurrence stands, and the colours.
//!
//! Reported behaviour: a print head from STEP came in as a flat list "Condor 1 ... 47", with no subassemblies, no
//! names and no colours, where another CAD laid it out as its author did.
//!
//! The reference `tests/data/assembly.step` is written by OCCT's own writer: an assembly holding a plate twice (at x 0
//! and at x 30) and a subassembly (at 5, 10, 0) holding a pin (at z 5), every one of them named in Cyrillic. The
//! plate is red, the pin blue. A plate is a 10 x 20 x 5 box, a pin a cylinder of radius 4 and height 12 standing on its base.
use crate::{ExactTree, document_tree, read_exact_tree, ExactFormat, ImportNode};

/// The names the reference was written with, one per line - the assembly, the plate, the subassembly, the pin. They
/// are data of the file, kept beside it, not messages.
const NAMES: &str = include_str!("../../tests/data/assembly.names");

fn name(k: usize) -> &'static str {
    NAMES.lines().nth(k).expect("a name of the reference")
}

fn reference() -> Vec<ImportNode> {
    read_exact_tree(ExactFormat::Step, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assembly.step"), 0.5).expect("the reference reads").nodes
}

/// Where node `i` stands in the world: its placement composed with every parent's.
fn world(nodes: &[ImportNode], i: usize) -> [f64; 12] {
    let mut m = nodes[i].place;
    let mut at = nodes[i].parent;
    while let Some(p) = at {
        let a = nodes[p].place;
        let mut out = [0.0; 12];
        for r in 0..3 {
            for c in 0..4 {
                out[r * 4 + c] = (0..3).map(|k| a[r * 4 + k] * m[k * 4 + c]).sum::<f64>() + if c == 3 { a[r * 4 + 3] } else { 0.0 };
            }
        }
        m = out;
        at = nodes[p].parent;
    }
    m
}

fn named(nodes: &[ImportNode], name: &str) -> Vec<usize> {
    (0..nodes.len()).filter(|&i| nodes[i].name == name).collect()
}

#[test]
fn a_step_assembly_comes_with_its_tree_and_names() {
    let nodes = reference();
    let tree: Vec<(Option<usize>, &str)> = nodes.iter().map(|n| (n.parent, n.name.as_str())).collect();
    let roots: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].parent.is_none()).collect();
    assert_eq!(roots.len(), 1, "one root: {tree:?}");
    assert_eq!(nodes[roots[0]].name, name(0), "{tree:?}");
    let under = |p: usize| -> Vec<&str> { nodes.iter().filter(|n| n.parent == Some(p)).map(|n| n.name.as_str()).collect() };
    assert_eq!(under(roots[0]), [name(1), name(1), name(2)], "{tree:?}");
    let unit = named(&nodes, name(2))[0];
    assert_eq!(under(unit), [name(3)], "{tree:?}");
}

/// EVERY OCCURRENCE IS A BODY OF ITS OWN, IN ITS OWN COORDINATES, AND THE TREE PLACES IT. A body baked into the world
/// where it stands could not be moved or mated as a part: both plates would be different solids of the same part.
#[test]
fn every_occurrence_is_a_body_in_its_own_coordinates_placed_by_the_tree() {
    let ExactTree { bodies, shapes, nodes } = read_exact_tree(ExactFormat::Step, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assembly.step"), 0.5).expect("reads");
    assert_eq!((bodies.len(), shapes.len()), (2, 2), "two products, two bodies - the plate is held once");
    assert_eq!(nodes.iter().filter(|n| n.solid.is_some() || n.repeat_of.is_some()).count(), 3, "every occurrence carries its body or repeats one");
    let xs: Vec<f64> = named(&nodes, name(1)).into_iter().map(|i| world(&nodes, i)[3]).collect();
    assert!(xs.iter().any(|x| x.abs() < 1e-9) && xs.iter().any(|x| (x - 30.0).abs() < 1e-9), "the plates stand at x {xs:?}, not 0 and 30");
    for i in named(&nodes, name(1)) {
        let b = shapes[nodes[i].solid.or(nodes[i].repeat_of).expect("a plate carries a body or repeats one")].bbox().expect("a box");
        assert!(b.iter().zip([0.0, 0.0, 0.0, 10.0, 20.0, 5.0]).all(|(g, w)| (g - w).abs() < 1e-6), "a plate's body is {b:?}, not the plate at its own zero");
    }
    let pin = named(&nodes, name(3))[0];
    let w = world(&nodes, pin);
    assert!((w[3] - 5.0).abs() < 1e-9 && (w[7] - 10.0).abs() < 1e-9 && (w[11] - 5.0).abs() < 1e-9, "the pin stands at {:?}, not (5, 10, 5)", [w[3], w[7], w[11]]);
    // The kernel's box of a curved body is widened by some 0.011 mm on each side where the surface curves, and is
    // exact where it is flat; so the body's place is checked by the middle of its box, its size loosely.
    let k = nodes[pin].solid.expect("the pin carries a body");
    let b = shapes[k].bbox().expect("a box");
    let middle = [(b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0, (b[2] + b[5]) / 2.0];
    assert!(middle.iter().zip([0.0, 0.0, 6.0]).all(|(g, w)| (g - w).abs() < 1e-6), "the pin's body is at {middle:?}, not at its own zero");
    let size = [b[3] - b[0], b[4] - b[1], b[5] - b[2]];
    assert!(size.iter().zip([8.0, 8.0, 12.0]).all(|(g, w)| (g - w).abs() < 0.03), "the pin's body is {size:?}, not 8 x 8 x 12");
    let seen: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| w[r * 4 + c] * middle[c]).sum::<f64>() + w[r * 4 + 3]);
    assert!(seen.iter().zip([5.0, 10.0, 11.0]).all(|(g, w)| (g - w).abs() < 1e-6), "in the world the pin's middle is {seen:?}, not (5, 10, 11)");
    // the mesh shown is the same body, in the same coordinates
    let m = bodies[k].mesh.bounds().expect("a mesh has bounds");
    assert!(m.min.x >= b[0] - 1e-3 && m.max.x <= b[3] + 1e-3 && m.min.z >= b[2] - 1e-3 && m.max.z <= b[5] + 1e-3, "the pin's mesh is not where its solid is");
}

/// A PRODUCT THE KERNEL'S OWN WRITER GAVE ITS STAND-IN NAME COMES UNNAMED, so the part is named after its file, and
/// not "... STEP translator 7.9 1" as the writer calls whatever it was given no name for.
#[test]
fn the_stand_in_name_of_the_writer_is_no_name() {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/step-structure"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let path = dir.join("unnamed.step").to_string_lossy().into_owned();
    let cube = crate::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
    crate::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], &path).expect("written");
    let ExactTree { nodes, .. } = read_exact_tree(ExactFormat::Step, &path, 0.5).expect("reads");
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(!nodes.is_empty() && names.iter().all(|n| n.is_empty()), "the writer's stand-in came as a name: {names:?}");
}

#[test]
fn a_step_assembly_brings_its_colours() {
    let nodes = reference();
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    for i in named(&nodes, name(1)) {
        assert!(near(nodes[i].color, [0.8, 0.1, 0.1]), "a plate is coloured {:?}, not red", nodes[i].color);
    }
    let pin = named(&nodes, name(3))[0];
    assert!(near(nodes[pin].color, [0.1, 0.2, 0.9]), "the pin is coloured {:?}, not blue", nodes[pin].color);
}

/// THE PRINT HEAD OF THE REPORT, where its file is at hand: `QYM_CONDOR` names it. Its names are written as \X2\
/// escapes, which the reference's are not.
#[test]
#[ignore = "the owner's file"]
fn the_print_head_comes_as_its_author_built_it() {
    let Ok(path) = std::env::var("QYM_CONDOR") else { return };
    let ExactTree { shapes, nodes, .. } = read_exact_tree(ExactFormat::Step, &path, 0.5).expect("reads");
    let root = (0..nodes.len()).find(|&i| nodes[i].parent.is_none()).expect("a root");
    let first: Vec<&str> = nodes.iter().filter(|n| n.parent == Some(root)).map(|n| n.name.as_str()).collect();
    println!("root {:?}, {} nodes, {} bodies, coloured {}", nodes[root].name, nodes.len(), shapes.len(), nodes.iter().filter(|n| n.color.is_some()).count());
    println!("first level: {first:?}");
    assert_eq!(first, include_str!("../../tests/data/condor-first-level.names").lines().collect::<Vec<_>>());
}

fn read(name: &str, parent: Option<usize>, solid: Option<usize>) -> ImportNode {
    ImportNode { name: name.into(), parent, place: qymcad_core::feature::PLACE_IDENTITY, solid, color: None, repeat_of: None, faces: Vec::new() }
}

/// AN UNNAMED FILE'S PARTS ARE NAMED AFTER THE FILE, as before the tree came in: a single body by the file's name,
/// several numbered by their bodies.
#[test]
fn unnamed_parts_are_named_after_the_file() {
    let many = document_tree(&[read("", None, Some(0)), read("", None, Some(1))], &[7, 8], "cube");
    assert_eq!(many.iter().map(|n| (n.name.as_str(), n.body)).collect::<Vec<_>>(), [("cube 1", Some(7)), ("cube 2", Some(8))]);
    assert_eq!(document_tree(&[read("", None, Some(0))], &[7], "cube")[0].name, "cube");
}

/// A COLOUR GIVEN TO A SUBASSEMBLY COVERS WHAT IS UNDER IT, unless a part has its own - the way a STEP assembly
/// colours an occurrence. sRGB comes to bytes rounded: 0.8 is 204 and 0.1 is 26.
#[test]
fn a_colour_covers_what_is_under_it() {
    let tinted = |name: &str, parent, solid, c: [f32; 3]| ImportNode { color: Some(c), ..read(name, parent, solid) };
    let t = document_tree(&[tinted("unit", None, None, [0.8, 0.1, 0.1]), read("bare", Some(0), Some(0)), tinted("own", Some(0), Some(1), [0.1, 0.2, 0.9])], &[5, 6], "file");
    let colours: Vec<Option<[u8; 3]>> = t[0].children.iter().map(|c| c.color).collect();
    assert_eq!(colours, [Some([204, 26, 26]), Some([26, 51, 230])]);
    assert_eq!(document_tree(&[read("plain", None, Some(0))], &[5], "file")[0].color, None, "a colour came from nowhere");
}

/// A PRODUCT IS READ ONCE, HOWEVER OFTEN IT OCCURS: the second plate of the reference repeats the first one's body
/// rather than bringing a copy of it - which is what lets it come in as a clone of the same part.
#[test]
fn a_repeated_product_is_read_once() {
    let ExactTree { bodies, nodes, .. } = read_exact_tree(ExactFormat::Step, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assembly.step"), 0.5).expect("reads");
    let plates = named(&nodes, name(1));
    let first = nodes[plates[0]].solid.expect("the first plate brings its body");
    assert_eq!((nodes[plates[1]].solid, nodes[plates[1]].repeat_of), (None, Some(first)), "the second plate is read again instead of repeating the first");
    assert_eq!(bodies.len(), 2, "the plate is held once in the file, and read {} bodies", bodies.len());
}

/// THE FLAT LIST NESTS BY ITS PARENTS, in the file's order, and a named node keeps its name.
#[test]
fn the_flat_list_nests_by_its_parents() {
    let t =
        document_tree(&[read("head", None, None), read("plate", Some(0), Some(0)), read("unit", Some(0), None), read("pin", Some(2), Some(1)), read("plate", Some(0), Some(2))], &[10, 11, 12], "file");
    assert_eq!(t.len(), 1, "one root");
    let names = |n: &qymcad_core::model::ImportNode| n.children.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&t[0]), ["plate", "unit", "plate"]);
    let pin = &t[0].children[1].children[0];
    assert_eq!((pin.name.as_str(), pin.body, pin.solid), ("pin", Some(11), 1));
}

/// A NAME PAST ASCII IS SPELLED AS THE FORMAT HOLDS IT: a run of the basic plane as \X2\ with four digits a
/// character, one past it as \X4\ with eight, each run closed by \X0\ and ASCII between runs left as it is.
#[test]
fn a_name_past_ascii_is_spelled_as_the_format_holds_it() {
    let written = "#7 = PRODUCT('\u{41a}\u{430} x\u{1f600}\u{430}','');";
    assert_eq!(crate::step_spelled(written), "#7 = PRODUCT('\\X2\\041A0430\\X0\\ x\\X4\\0001F600\\X0\\\\X2\\0430\\X0\\','');");
    assert_eq!(crate::step_spelled("#1 = PRODUCT('plate','');"), "#1 = PRODUCT('plate','');", "an ASCII file is not touched");
}

/// A FACE BRINGS A COLOUR OF ITS OWN: the reference's plate is red with its top face green, and the node of the plate
/// carries that colour by the face's persistent id - 6, the number the plate's top face gets when the body is wrapped.
#[test]
fn a_face_brings_a_colour_of_its_own() {
    let ExactTree { shapes, nodes, .. } = read_exact_tree(ExactFormat::Step, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assembly.step"), 0.5).expect("the reference reads");
    let plate = named(&nodes, name(1))[0];
    let faces = &nodes[plate].faces;
    assert_eq!(faces.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [6], "the plate's faces of a colour of their own: {faces:?}");
    assert!(faces[0].1.iter().zip([0.1, 0.8, 0.1]).all(|(g, w)| (g - w).abs() < 0.01), "the top face is not green: {:?}", faces[0].1);
    // the number is the live body's: the one a rebuild from the source gives it, and what the document shows
    let live = shapes[nodes[plate].solid.expect("the plate's body")].tessellate_merged(0.5).expect("the plate tessellates");
    let top = live.faces.iter().find(|f| f.id == 6).map(|f| f.centroid.z);
    assert_eq!(top, Some(5.0), "face 6 of the plate is not its top face");
    assert!(nodes[named(&nodes, name(3))[0]].faces.is_empty(), "the pin has no face of a colour of its own");
}
