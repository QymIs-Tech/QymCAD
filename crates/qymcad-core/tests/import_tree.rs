//! A FILE'S TREE COMES IN AS ITS AUTHOR BUILT IT: a subassembly per assembly of the file, a part per body, the
//! file's names kept, every component standing where the file places it in its parent.
use qymcad_core::feature::{ComponentKind, FeatureKind, PLACE_IDENTITY};
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::{ImportNode, Project};

fn tri() -> Mesh {
    Mesh { verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)], tris: vec![[0, 1, 2]], ..Default::default() }
}

fn moved(x: f64, y: f64, z: f64) -> [f64; 12] {
    let mut m = PLACE_IDENTITY;
    m[3] = x;
    m[7] = y;
    m[11] = z;
    m
}

fn leaf(name: &str, body: u64, solid: u32, place: [f64; 12]) -> ImportNode {
    ImportNode { name: name.into(), place, body: Some(body), solid, ..Default::default() }
}

#[test]
fn a_tree_comes_in_as_parts_and_subassemblies_where_the_file_places_them() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("head.step", vec![1, 2, 3]);
    let (a, b, pin) = (p.add_mesh(tri()), p.add_mesh(tri()), p.add_mesh(tri()));
    let tree = vec![ImportNode {
        name: "head".into(),
        children: vec![
            ImportNode { color: Some([200, 30, 30]), ..leaf("plate", a, 0, PLACE_IDENTITY) },
            leaf("plate", b, 1, moved(30.0, 0.0, 0.0)),
            ImportNode { name: "unit".into(), place: moved(5.0, 10.0, 0.0), children: vec![leaf("pin", pin, 2, moved(0.0, 0.0, 5.0))], ..Default::default() },
            ImportNode { name: "empty group".into(), ..Default::default() },
        ],
        ..Default::default()
    }];
    let root = p.import_tree_as_parts(tree, source, "file").expect("something came in");
    let comp = |id| p.components.iter().find(|c| c.id == id).expect("a component");
    let children = |id| p.components.iter().filter(move |c| c.parent == Some(id)).map(|c| (c.name.clone(), c.kind)).collect::<Vec<_>>();
    assert_eq!((comp(root).name.as_str(), comp(root).kind), ("head", ComponentKind::Assembly), "one node at the top stands alone, under its own name");
    assert_eq!(comp(root).parent, Some(p.root), "the import lands in the active context");
    assert_eq!(
        children(root),
        vec![("plate".to_string(), ComponentKind::Part), ("plate".to_string(), ComponentKind::Part), ("unit".to_string(), ComponentKind::Assembly)],
        "an empty group must not come in"
    );
    let unit = p.components.iter().find(|c| c.name == "unit").expect("the subassembly").id;
    assert_eq!(children(unit), vec![("pin".to_string(), ComponentKind::Part)]);
    // every body holds an Import node in its own part, pointing back at its place in the source
    let owner = |body| {
        let node = p.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Import { body: x, .. } if x == body)).expect("an Import node");
        (node.parent.expect("in a part"), node.kind.clone())
    };
    for (body, solid) in [(a, 0), (b, 1), (pin, 2)] {
        let (part, kind) = owner(body);
        assert!(matches!(kind, FeatureKind::Import { source: s, solid: k, .. } if s == source && k == solid), "body {body}: wrong source or place in it");
        assert_eq!(p.component_bodies(part), vec![body], "the body does not live in a part of its own");
    }
    // every component stands where the file places it, so the bodies are not baked into the world
    let at = |body| {
        let w = p.body_world_transform(body);
        [w[3], w[7], w[11]]
    };
    assert_eq!(at(a), [0.0, 0.0, 0.0]);
    assert_eq!(at(b), [30.0, 0.0, 0.0], "the second plate is not where the file puts it");
    assert_eq!(at(pin), [5.0, 10.0, 5.0], "the pin is not placed through its subassembly");
    assert_eq!(comp(owner(pin).0).transform, moved(0.0, 0.0, 5.0), "the part is placed in its parent, not in the world");
    // the file's colour is the part's colour; a part it leaves uncoloured keeps the palette
    let colour = |body| p.mesh_color(p.mesh_index(body).expect("a body"));
    assert_eq!(colour(a), [200, 30, 30], "the file's colour was dropped");
    assert_ne!(colour(b), [200, 30, 30], "a colour went to a part the file leaves uncoloured");
}

/// A REPEATED PRODUCT COMES IN AS A CLONE of the part that holds its body, standing where the file places it.
#[test]
fn a_repeat_comes_in_as_a_clone_of_the_first() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("bolts.step", vec![1]);
    let bolt = p.add_mesh(tri());
    let tree = vec![ImportNode {
        name: "pair".into(),
        children: vec![
            ImportNode { color: Some([10, 20, 30]), ..leaf("bolt", bolt, 0, PLACE_IDENTITY) },
            ImportNode { name: "bolt".into(), place: moved(15.0, 0.0, 0.0), repeat_of: Some(0), color: Some([10, 20, 30]), ..Default::default() },
        ],
        ..Default::default()
    }];
    let root = p.import_tree_as_parts(tree, source, "file").expect("came in");
    let parts: Vec<u64> = p.components.iter().filter(|c| c.parent == Some(root)).map(|c| c.id).collect();
    assert_eq!(parts.len(), 2, "two occurrences, two parts");
    assert_eq!(p.instance_origin(parts[1]), parts[0], "the repeat is not a clone of the first bolt");
    assert_eq!(p.component_transform(parts[1]), moved(15.0, 0.0, 0.0), "the clone does not stand where the file puts it");
    assert_eq!(p.timeline.iter().filter(|n| matches!(n.kind, FeatureKind::Import { .. })).count(), 1, "the product is imported once");
}

#[test]
fn several_nodes_at_the_top_go_into_one_subassembly_under_the_files_name() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("two.step", vec![1]);
    let (a, b) = (p.add_mesh(tri()), p.add_mesh(tri()));
    let root = p.import_tree_as_parts(vec![leaf("a", a, 0, PLACE_IDENTITY), leaf("b", b, 1, moved(0.0, 7.0, 0.0))], source, "two").expect("came in");
    let c = p.components.iter().find(|c| c.id == root).expect("a component");
    assert_eq!((c.name.as_str(), c.kind), ("two", ComponentKind::Assembly));
    assert_eq!(p.body_world_transform(b)[7], 7.0);
}

#[test]
fn a_single_body_stands_alone_as_a_part() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("cube.step", vec![1]);
    let cube = p.add_mesh(tri());
    let before = p.current_ctx();
    let root = p.import_tree_as_parts(vec![leaf("cube", cube, 0, moved(1.0, 2.0, 3.0))], source, "cube").expect("came in");
    let c = p.components.iter().find(|c| c.id == root).expect("a component");
    assert_eq!((c.name.as_str(), c.kind), ("cube", ComponentKind::Part), "one body is one part, not a subassembly of one");
    assert_eq!(c.transform, moved(1.0, 2.0, 3.0), "a lone part keeps where the file places it");
    assert_eq!(p.current_ctx(), before, "the active context did not come back");
}

/// A MESH COMES IN AS PARTS TOO, a piece per part, in the colours of its file - but as a mesh piece, not an imported
/// solid: it has no B-rep to raise again or to export as one, and exports as the mesh it is.
#[test]
fn mesh_pieces_come_in_as_parts_of_their_own() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("pair.obj", vec![1]);
    let (bolt, plate) = (p.add_mesh(tri()), p.add_mesh(tri()));
    let piece = |name: &str, body: u64, k: u32, color| ImportNode { name: name.into(), body: Some(body), solid: k, color, mesh: true, ..Default::default() };
    let root = p.import_tree_as_parts(vec![piece("bolt", bolt, 0, Some([38, 38, 42])), piece("plate", plate, 1, None)], source, "pair").expect("came in");
    let parts: Vec<(String, u64)> = p.components.iter().filter(|c| c.parent == Some(root)).map(|c| (c.name.clone(), c.id)).collect();
    assert_eq!(parts.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["bolt", "plate"], "a piece per part, under its name");
    for (body, k) in [(bolt, 0), (plate, 1)] {
        let node = p.timeline.iter().find(|n| n.kind.body() == Some(body)).expect("a node holds the piece");
        assert!(matches!(node.kind, FeatureKind::MeshPiece { source: s, piece, .. } if s == source && piece == k), "piece {k} came in as {:?}, not as a mesh piece", node.kind);
        assert_eq!(p.body_owner(body), Some(parts[k as usize].1), "the piece does not live in its own part");
        assert_eq!(p.export_kind(body, false), qymcad_core::model::ExportKind::MeshOnly, "a mesh piece is exported as a mesh, not reported as a failed rebuild");
    }
    assert_eq!(p.mesh_color(p.mesh_index(bolt).expect("a body")), [38, 38, 42], "the file's colour was dropped");
}

/// A FACE OF A COLOUR OF ITS OWN COMES IN WITH ITS PART, by the face's persistent id, and shows on a clone as on the
/// original. A face the file does not colour has no colour of its own.
#[test]
fn a_face_of_its_own_colour_comes_in_with_its_part() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("head.step", vec![1, 2, 3]);
    let plate = p.add_mesh(tri());
    let green = [26, 204, 26];
    let tree = vec![ImportNode {
        name: "head".into(),
        children: vec![
            ImportNode { color: Some([204, 26, 26]), face_colors: vec![(6, green)], ..leaf("plate", plate, 0, PLACE_IDENTITY) },
            ImportNode { name: "plate".into(), place: moved(30.0, 0.0, 0.0), repeat_of: Some(0), ..Default::default() },
        ],
        ..Default::default()
    }];
    p.import_tree_as_parts(tree, source, "head").expect("it came in");
    assert_eq!(p.face_color(plate, 6), Some(green), "the plate's top face did not come in green");
    assert_eq!(p.face_color(plate, 5), None, "a face the file does not colour came in with a colour of its own");
    let clone = p.components.iter().filter(|c| c.name == "plate").map(|c| c.id).nth(1).expect("the second plate");
    let body = p.timeline.iter().find(|n| n.parent == Some(clone)).and_then(|n| n.kind.body()).expect("the clone's body");
    assert_eq!(p.face_color(body, 6), Some(green), "the clone does not show the face as its original does");
}

/// A PIECE OF MORE COLOURS THAN ITS PALETTE HOLDS keeps every triangle near its colour: 255 reds from black up come first,
/// then 45 greens - every one comes in looking like itself, not as the first colour met, nor as a red, the colours met
/// first.
#[test]
fn a_piece_of_more_colours_than_its_palette_holds_keeps_them_near() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("scan.ply", vec![1]);
    let n = 300u32;
    let mut mesh = Mesh { verts: Vec::new(), tris: Vec::new(), ..Default::default() };
    for k in 0..n {
        let x = f64::from(k);
        mesh.verts.extend([Point3::new(x, 0.0, 0.0), Point3::new(x + 1.0, 0.0, 0.0), Point3::new(x, 1.0, 0.0)]);
        mesh.tris.push([3 * k, 3 * k + 1, 3 * k + 2]);
    }
    let colours: Vec<[u8; 3]> = (0..n).map(|k| if k < 255 { [k as u8, 0, 0] } else { [0, (100 + 3 * (k - 255)) as u8, 0] }).collect();
    let body = p.add_mesh(mesh);
    let piece = ImportNode { name: "scan".into(), body: Some(body), mesh: true, tri_colors: colours.clone(), ..Default::default() };
    p.import_tree_as_parts(vec![piece], source, "scan").expect("came in");
    let (palette, places) = p.tri_colors.get(&p.lineage_root(body)).expect("the piece keeps its triangles' colours");
    assert!(palette.len() <= 255, "a palette of {} colours", palette.len());
    let far: Vec<(usize, [u8; 3], [u8; 3])> =
        places.iter().map(|&k| palette[k as usize]).zip(&colours).enumerate().filter(|(_, (got, c))| (0..3).any(|i| got[i].abs_diff(c[i]) > 8)).map(|(k, (got, c))| (k, *c, got)).collect();
    assert!(far.is_empty(), "{} triangles come in far from their colours (triangle, its colour, shown), the first: {:?}", far.len(), &far[..far.len().min(5)]);
}

/// A MESH PIECE KEEPS ITS SCALE and is not rebuilt for it: its mesh is its geometry, scaled when the factor is set, so
/// the factor is only remembered - the next one is taken from the mesh divided by it.
#[test]
fn a_mesh_piece_keeps_its_scale() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("cube.stl", vec![1]);
    let body = p.add_mesh(tri());
    p.import_tree_as_parts(vec![ImportNode { name: "cube".into(), body: Some(body), mesh: true, ..Default::default() }], source, "cube").expect("came in");
    assert_eq!(p.import_scale(body), Some(1.0), "a mesh piece does not come in at the file's own numbers");
    assert!(p.set_import_scale(body, 10.0), "the scale of a mesh piece is not set");
    assert_eq!(p.import_scale(body), Some(10.0), "the mesh piece does not keep its factor");
    assert!(p.timeline.iter().all(|n| !n.dirty), "a mesh piece is marked for a rebuild it has nothing to rebuild from");
}

/// THE SOURCE GOES WITH WHAT CAME OF IT: the part a file came in as is deleted, and the file's source leaves the
/// document with it; a source something still comes of stays. Reported behaviour: the source hung in the tree.
#[test]
fn an_import_source_goes_with_the_part_it_came_in_as() {
    let mut p = Project::default();
    p.new_document();
    let (gone, kept) = (p.add_source("gone.step", vec![1]), p.add_source("kept.step", vec![2]));
    let (a, b) = (p.add_mesh(tri()), p.add_mesh(tri()));
    let first = p.import_tree_as_parts(vec![leaf("block", a, 0, PLACE_IDENTITY)], gone, "gone").expect("came in");
    let _second = p.import_tree_as_parts(vec![leaf("pin", b, 0, PLACE_IDENTITY)], kept, "kept").expect("came in");
    assert!(p.sources.len() == 2, "setup: two sources");
    p.delete_component(first);
    let left: Vec<&str> = p.sources.iter().map(|s| s.name.as_str()).collect();
    assert!(left == ["kept.step"], "the part the file came in as was deleted and the sources are {left:?}");
}
