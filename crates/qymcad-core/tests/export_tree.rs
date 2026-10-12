//! A DOCUMENT'S TREE GOES OUT AS IT STANDS: every subassembly and part under the export root, where it stands in its
//! parent, a part with its body and colour, a clone as a repeat of its original - and nothing a file cannot carry.
use qymcad_core::feature::PLACE_IDENTITY;
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

/// A head holding a plate twice (the second a clone) and a unit holding a pin; returns the project, the head and the
/// bodies of the plate and the pin.
fn head() -> (Project, u64, u64, u64) {
    let mut p = Project::default();
    p.new_empty_document();
    let source = p.add_source("head.step", vec![1]);
    let (plate, pin) = (p.add_mesh(tri()), p.add_mesh(tri()));
    let leaf = |name: &str, body, k, place, color| ImportNode { name: name.into(), place, body: Some(body), solid: k, color, ..Default::default() };
    let tree = vec![ImportNode {
        name: "head".into(),
        children: vec![
            leaf("plate", plate, 0, PLACE_IDENTITY, Some([204, 26, 26])),
            ImportNode { name: "plate".into(), place: moved(30.0, 0.0, 0.0), repeat_of: Some(0), ..Default::default() },
            ImportNode { name: "unit".into(), place: moved(5.0, 10.0, 0.0), children: vec![leaf("pin", pin, 1, moved(0.0, 0.0, 5.0), Some([26, 51, 230]))], ..Default::default() },
        ],
        ..Default::default()
    }];
    let root = p.import_tree_as_parts(tree, source, "file").expect("came in");
    (p, root, plate, pin)
}

#[test]
fn a_tree_goes_out_as_it_stands() {
    let (p, root, plate, pin) = head();
    let tree = p.export_tree(root, |_| true);
    let shape: Vec<(&str, Option<usize>)> = tree.iter().map(|n| (n.name.as_str(), n.parent)).collect();
    assert_eq!(shape, [("head", None), ("plate", Some(0)), ("plate", Some(0)), ("unit", Some(0)), ("pin", Some(3))], "the tree does not go out as it stands");
    assert_eq!((tree[1].body, tree[1].same_as, tree[1].color), (Some(plate), None, Some([204, 26, 26])), "the first plate goes out without its body or colour");
    assert_eq!(tree[2].same_as, Some(1), "the clone goes out as a copy, not as a second occurrence of its original");
    assert_eq!(tree[2].place, moved(30.0, 0.0, 0.0), "the clone does not stand where it stands");
    assert_eq!((tree[3].body, tree[3].place), (None, moved(5.0, 10.0, 0.0)));
    assert_eq!((tree[4].body, tree[4].place, tree[4].color), (Some(pin), moved(0.0, 0.0, 5.0), Some([26, 51, 230])));
}

/// A PART SPLIT INTO TWO BODIES GOES OUT WHOLE (#195). The export took `active_body` only, so a part of
/// 2 bodies went out as 1 solid while the status counted 2. Every visible body of the part goes out as its
/// own node: two extrusions with no boolean between them (the shape of split pieces) give two body nodes.
#[test]
fn a_part_of_two_bodies_goes_out_whole() {
    use qymcad_core::geom::Point2;
    let mut p = Project::default();
    let _part = p.new_document();
    for (name, x) in [("s1", 0.0), ("s2", 30.0)] {
        let sid = p.add_line_sketch(name, vec![Point2::new(x, 0.0), Point2::new(x + 10.0, 0.0), Point2::new(x + 10.0, 10.0), Point2::new(x, 10.0)], true);
        p.add_sketch_node(sid, name);
        p.add_extrude(sid, 5.0);
    }
    let tree = p.export_tree(p.root, |_| true);
    let bodies: Vec<_> = tree.iter().filter_map(|n| n.body).collect();
    assert_eq!(bodies.len(), 2, "both bodies go out, not just the active one: {tree:?}");
    assert!(tree.iter().all(|n| n.same_as.is_none()), "no silent repeat stands in for a missing body: {tree:?}");
}

/// A PART WITH NO SHAPE TO WRITE DOES NOT GO OUT, and a subassembly left empty by it goes with it.
#[test]
fn what_has_no_shape_does_not_go_out() {
    let (p, root, _, pin) = head();
    let tree = p.export_tree(root, |b| b != pin);
    assert_eq!(tree.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["head", "plate", "plate"], "a part without a shape, or its empty subassembly, went out");
}
