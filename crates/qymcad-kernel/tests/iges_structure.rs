//! AN IGES ASSEMBLY COMES IN AS ITS AUTHOR BUILT IT, the way a STEP one does: its subfigure definitions (308) are the
//! products, their instances (408) the occurrences, their names - written in Windows-1251 with the occurrence number -
//! the names, and the colours of their faces the colours.
//!
//! Reported behaviour: the print head of the report, opened from its IGES, came in as a heap of bodies; the file holds
//! its tree the same as the STEP does - 79 subfigures, 110 instances, 70 solids.
//!
//! The reference `tests/data/assembly.igs` is written by `tests/data/gen_iges.cpp`, laid out the way that file is: the
//! assembly of `assembly.step` - a plate twice, a subassembly holding a pin - as subfigures, solids and face colours.
use qymcad_kernel::{ExactTree, read_exact_tree, ExactFormat, ImportNode};

/// The names the reference was written with: the assembly, the plate, the subassembly, the pin.
const NAMES: &str = include_str!("data/assembly.names");

fn name(k: usize) -> &'static str {
    NAMES.lines().nth(k).expect("a name of the reference")
}

fn reference() -> ExactTree {
    read_exact_tree(ExactFormat::Iges, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/assembly.igs"), 0.5).expect("the reference reads")
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
fn an_iges_assembly_comes_with_its_tree_and_names() {
    let ExactTree { nodes, .. } = reference();
    let tree: Vec<(Option<usize>, &str)> = nodes.iter().map(|n| (n.parent, n.name.as_str())).collect();
    let roots: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].parent.is_none()).collect();
    assert_eq!(roots.len(), 1, "one root: {tree:?}");
    assert_eq!(nodes[roots[0]].name, name(0), "the name is not decoded from Windows-1251, or keeps its occurrence number: {tree:?}");
    let under = |p: usize| -> Vec<&str> { nodes.iter().filter(|n| n.parent == Some(p)).map(|n| n.name.as_str()).collect() };
    assert_eq!(under(roots[0]), [name(1), name(1), name(2)], "{tree:?}");
    assert_eq!(under(named(&nodes, name(2))[0]), [name(3)], "{tree:?}");
}

#[test]
fn an_iges_product_is_read_once_and_placed_by_the_tree() {
    let ExactTree { bodies, shapes, nodes } = reference();
    assert_eq!((bodies.len(), shapes.len()), (2, 2), "two products, two bodies");
    let plates = named(&nodes, name(1));
    let first = nodes[plates[0]].solid.expect("the first plate brings its body");
    assert_eq!((nodes[plates[1]].solid, nodes[plates[1]].repeat_of), (None, Some(first)), "the second plate is read again instead of repeating the first");
    let xs: Vec<f64> = plates.iter().map(|&i| world(&nodes, i)[3]).collect();
    assert!(xs.iter().any(|x| x.abs() < 1e-9) && xs.iter().any(|x| (x - 30.0).abs() < 1e-9), "the plates stand at x {xs:?}, not 0 and 30");
    let b = shapes[first].bbox().expect("a box");
    assert!(b.iter().zip([0.0, 0.0, 0.0, 10.0, 20.0, 5.0]).all(|(g, w)| (g - w).abs() < 1e-6), "the plate's body is {b:?}, not the plate at its own zero");
    let pin = named(&nodes, name(3))[0];
    let w = world(&nodes, pin);
    assert!((w[3] - 5.0).abs() < 1e-9 && (w[7] - 10.0).abs() < 1e-9 && (w[11] - 5.0).abs() < 1e-9, "the pin stands at {:?}, not (5, 10, 5)", [w[3], w[7], w[11]]);
}

/// THE COLOUR OF A BODY IS THE ONE ITS FACES SHARE: the file colours faces, not solids.
#[test]
fn an_iges_assembly_brings_the_colours_of_its_faces() {
    let ExactTree { nodes, .. } = reference();
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    let plate = named(&nodes, name(1))[0];
    assert!(near(nodes[plate].color, [0.8, 0.1, 0.1]), "the plate is coloured {:?}, not red", nodes[plate].color);
    let pin = named(&nodes, name(3))[0];
    assert!(near(nodes[pin].color, [0.1, 0.2, 0.9]), "the pin is coloured {:?}, not blue", nodes[pin].color);
}

/// THE PRINT HEAD OF THE REPORT IN IGES, where its file is at hand: `QYM_CONDOR_IGS` names it. The file holds no one
/// root the way its STEP does: its eight instances at the top are the first level.
#[test]
#[ignore = "the owner's file"]
fn the_print_head_in_iges_comes_as_its_author_built_it() {
    let Ok(path) = std::env::var("QYM_CONDOR_IGS") else { return };
    let ExactTree { shapes, nodes, .. } = read_exact_tree(ExactFormat::Iges, &path, 0.5).expect("reads");
    let first: Vec<&str> = nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.name.as_str()).collect();
    println!("{} nodes, {} bodies, {} repeats, coloured {}", nodes.len(), shapes.len(), nodes.iter().filter(|n| n.repeat_of.is_some()).count(), nodes.iter().filter(|n| n.color.is_some()).count());
    println!("faces of a colour of their own: {}", nodes.iter().map(|n| n.faces.len()).sum::<usize>());
    assert_eq!(first, include_str!("data/condor-first-level.names").lines().collect::<Vec<_>>());
    assert_eq!(shapes.len(), 70, "the file holds 70 solids");
}

/// AN IGES FACE BRINGS A COLOUR OF ITS OWN: the plate's faces are red but the top one, green. The plate takes the colour
/// most of its faces have, and the green one comes as a face of a colour of its own - 6, the number the plate's top face
/// gets when the body is wrapped. A single face of another colour used to leave the whole part without one.
#[test]
fn an_iges_face_brings_a_colour_of_its_own() {
    let ExactTree { shapes, nodes, .. } = reference();
    let plate = named(&nodes, name(1))[0];
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    assert!(near(nodes[plate].color, [0.8, 0.1, 0.1]), "the plate is coloured {:?}, not red", nodes[plate].color);
    let faces = &nodes[plate].faces;
    assert_eq!(faces.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [6], "the plate's faces of a colour of their own: {faces:?}");
    assert!(near(Some(faces[0].1), [0.1, 0.8, 0.1]), "the top face is not green: {:?}", faces[0].1);
    let live = qymcad_kernel::ask::export_mesh(&shapes[nodes[plate].solid.expect("the plate's body")], 0.5).expect("the plate tessellates");
    assert_eq!(live.faces.iter().find(|f| f.id == 6).map(|f| f.centroid.z), Some(5.0), "face 6 of the plate is not its top face");
    assert!(nodes[named(&nodes, name(3))[0]].faces.is_empty(), "the pin has no face of a colour of its own");
}

/// A FLAT IGES COMES WITH ITS NAMES, COLOURS AND GROUPS: a file with no subfigures - solids standing on their own, each
/// named by its name property, and a group the author made (`tests/data/flat.igs`, `gen_iges --flat`) - comes as
/// parts under their names, the group a subassembly holding its pin; the plate red with its top face green, the pin
/// blue as a whole, each where the file puts it.
#[test]
fn a_flat_iges_comes_with_its_names_colours_and_groups() {
    let ExactTree { shapes, nodes, .. } = read_exact_tree(ExactFormat::Iges, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/flat.igs"), 0.5).expect("the flat reference reads");
    let tree: Vec<(Option<usize>, &str)> = nodes.iter().map(|n| (n.parent, n.name.as_str())).collect();
    assert_eq!(shapes.len(), 2, "two solids: {tree:?}");
    let roots: Vec<&str> = nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.name.as_str()).collect();
    assert_eq!(roots, [name(1), name(2)], "the plate and the group at the top, under their names: {tree:?}");
    let unit = named(&nodes, name(2))[0];
    assert_eq!(nodes.iter().filter(|n| n.parent == Some(unit)).map(|n| n.name.as_str()).collect::<Vec<_>>(), [name(3)], "the group does not hold the pin: {tree:?}");
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    let plate = named(&nodes, name(1))[0];
    assert!(near(nodes[plate].color, [0.8, 0.1, 0.1]), "the plate is coloured {:?}, not red", nodes[plate].color);
    let faces = &nodes[plate].faces;
    assert_eq!(faces.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [6], "the plate's faces of a colour of their own: {faces:?}");
    assert!(near(Some(faces[0].1), [0.1, 0.8, 0.1]), "the top face is not green: {:?}", faces[0].1);
    let pin = named(&nodes, name(3))[0];
    assert!(near(nodes[pin].color, [0.1, 0.2, 0.9]), "the pin is coloured {:?}, not blue", nodes[pin].color);
    let b = shapes[nodes[pin].solid.expect("the pin's body")].bbox().expect("a box");
    let w = world(&nodes, pin);
    let at: Vec<f64> = (0..3).map(|k| b[k] + w[k * 4 + 3]).collect();
    // to 0.05 mm: the box of a round body read from IGES is taken with a margin - 0.011 mm here along x and z, none
    // along y - where a pin in the wrong place would be whole millimetres off
    assert!(at.iter().zip([1.0, 6.0, 5.0]).all(|(g, w)| (g - w).abs() < 0.05), "the pin's box starts at {at:?}, not where the file puts it (1, 6, 5)");
}

/// A FILE OF SURFACES COMES WITH ITS NAMES, COLOURS AND GROUPS TOO: an IGES with no solid at all - every face a surface
/// of its own in the colour of its face, the parts as the author grouped them (`tests/data/surfaces.igs`, `gen_iges
/// --surfaces`) - comes as the plate and the group holding the pin, each group's surfaces sewn into the body they close:
/// the plate red with its top face green, the pin blue, where the file puts them.
#[test]
fn a_file_of_surfaces_comes_with_its_names_colours_and_groups() {
    let ExactTree { bodies, shapes, nodes } = read_exact_tree(ExactFormat::Iges, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/surfaces.igs"), 0.5).expect("the surfaces reference reads");
    let tree: Vec<(Option<usize>, &str)> = nodes.iter().map(|n| (n.parent, n.name.as_str())).collect();
    let roots: Vec<&str> = nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.name.as_str()).collect();
    assert_eq!(roots, [name(1), name(2)], "the plate and the group at the top, under their names: {tree:?}");
    let unit = named(&nodes, name(2))[0];
    assert_eq!(nodes.iter().filter(|n| n.parent == Some(unit)).map(|n| n.name.as_str()).collect::<Vec<_>>(), [name(3)], "the group does not hold the pin: {tree:?}");
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    let plate = named(&nodes, name(1))[0];
    let solid = nodes[plate].solid.expect("the plate is a body");
    let v = shapes[solid].volume();
    assert!((v - 1000.0).abs() < 10.0, "the plate's surfaces are not sewn into its solid: volume {v}");
    assert!(near(nodes[plate].color, [0.8, 0.1, 0.1]), "the plate is coloured {:?}, not red", nodes[plate].color);
    let faces = &nodes[plate].faces;
    assert_eq!(faces.len(), 1, "the plate's faces of a colour of their own: {faces:?}");
    assert!(near(Some(faces[0].1), [0.1, 0.8, 0.1]), "the face of its own colour is not green: {:?}", faces[0].1);
    // the face's number is its place in the order the body's faces are met, from 1 - the persistent ids come later, at
    // the door, and the faces here carry none yet
    let top = bodies[solid].faces.get(faces[0].0 as usize - 1).expect("the green face is in the plate");
    assert!((top.centroid.z - 5.0).abs() < 1e-3, "the green face is not the plate's top: its centre is at {:?}", top.centroid);
    let pin = named(&nodes, name(3))[0];
    let psolid = nodes[pin].solid.expect("the pin is a body");
    let v = shapes[psolid].volume();
    assert!((v - std::f64::consts::PI * 16.0 * 12.0).abs() < 6.0, "the pin's surfaces are not sewn into its solid: volume {v}");
    assert!(near(nodes[pin].color, [0.1, 0.2, 0.9]), "the pin is coloured {:?}, not blue", nodes[pin].color);
    let b = shapes[psolid].bbox().expect("a box");
    let w = world(&nodes, pin);
    let at: Vec<f64> = (0..3).map(|k| b[k] + w[k * 4 + 3]).collect();
    // to 0.05 mm: the box of a round body read from IGES is taken with a margin
    assert!(at.iter().zip([1.0, 6.0, 5.0]).all(|(g, w)| (g - w).abs() < 0.05), "the pin's box starts at {at:?}, not where the file puts it (1, 6, 5)");
}

/// A FILE OF A SOLID WITH SURFACES BESIDE IT LOSES NEITHER: the plate a solid of its own, named and coloured, and the pin
/// surfaces in its group under the unit (`tests/data/mixed.igs`, `gen_iges --mixed`) - the plate comes as a flat file's
/// solid does, and the pin's surfaces sewn into its body under their group, in its colour.
#[test]
fn a_file_of_a_solid_and_surfaces_loses_neither() {
    let ExactTree { shapes, nodes, .. } = read_exact_tree(ExactFormat::Iges, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/mixed.igs"), 0.5).expect("the mixed reference reads");
    let tree: Vec<(Option<usize>, &str)> = nodes.iter().map(|n| (n.parent, n.name.as_str())).collect();
    let roots: Vec<&str> = nodes.iter().filter(|n| n.parent.is_none()).map(|n| n.name.as_str()).collect();
    assert_eq!(roots, [name(1), name(2)], "the plate and the group at the top, under their names: {tree:?}");
    let unit = named(&nodes, name(2))[0];
    assert_eq!(nodes.iter().filter(|n| n.parent == Some(unit)).map(|n| n.name.as_str()).collect::<Vec<_>>(), [name(3)], "the group does not hold the pin: {tree:?}");
    let near = |c: Option<[f32; 3]>, want: [f32; 3]| c.is_some_and(|c| c.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01));
    let plate = named(&nodes, name(1))[0];
    assert!(near(nodes[plate].color, [0.8, 0.1, 0.1]), "the plate is coloured {:?}, not red", nodes[plate].color);
    assert!((shapes[nodes[plate].solid.expect("the plate is a body")].volume() - 1000.0).abs() < 10.0, "the plate is not its solid");
    let pin = named(&nodes, name(3))[0];
    assert!(near(nodes[pin].color, [0.1, 0.2, 0.9]), "the pin is coloured {:?}, not blue", nodes[pin].color);
    let v = shapes[nodes[pin].solid.expect("the pin is a body")].volume();
    assert!((v - std::f64::consts::PI * 16.0 * 12.0).abs() < 6.0, "the pin's surfaces are not sewn into its solid: volume {v}");
}
