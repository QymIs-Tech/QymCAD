//! AN OPERATION LEAVES THE BODIES IT WAS HANDED AS THEY WERE. A body of the cache shares its edges and vertices with
//! the bodies before it and beside it, and the rebuild builds the nodes of one wave on threads of their own: an
//! operation of OCCT that writes into the body it is given - its tolerances, its curves on the faces - writes into
//! bodies another thread may be reading. Measured on the case of the examples: a chamfer of 3 took the sum of the
//! tolerances of its body from 0.0720624 to 0.0744600, and the chamfer 11963 came out invalid in 3 of 40 rebuilds with
//! another branch blending beside it.
//!
//! Each document of the examples is rebuilt whole; then, kind by kind, the nodes of one kind are marked and rebuilt on
//! the bodies of the cache, and every body they were handed and nothing rebuilt is to be, to the byte of its BRep, the
//! body it was before. Live OCCT, headless.
use qymcad_core::model::{Id, Project};
use std::collections::{BTreeMap, HashMap, HashSet};

/// THE DOCUMENTS OF THE EXAMPLES, opened as the window opens them.
const DOCUMENTS: [&str; 2] = [concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/QymBoxPc-Case.qcad"), concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Filter-v2.qcad")];

/// The kind of a node, by the name of its variant.
fn kind_of(p: &Project, node: Id) -> String {
    let kind = p.timeline.iter().find(|n| n.id == node).map(|n| format!("{:?}", n.kind)).unwrap_or_default();
    kind.split([' ', '{', '(']).next().unwrap_or_default().to_string()
}

/// The bytes of the BRep of every body asked for that the cache holds.
fn bytes_of(shapes: &HashMap<Id, qymcad_kernel::Shape>, bodies: &[Id]) -> BTreeMap<Id, Option<Vec<u8>>> {
    bodies.iter().filter(|b| shapes.contains_key(b)).map(|b| (*b, shapes.get(b).and_then(|s| s.to_brep_bytes()))).collect()
}

#[test]
fn every_operation_of_the_examples_leaves_the_bodies_it_was_handed_as_they_were() {
    let mut moved: Vec<String> = Vec::new();
    for doc in DOCUMENTS {
        let mut p = qymcad_io::load_project(doc).expect("the document opens");
        let shapes = qymcad_testkit::restore_import_shapes(&p);
        let (report, mut shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
        assert!(report.errors.is_empty(), "setup: {doc} is built whole: {:?}", report.errors);
        let mut kinds: BTreeMap<String, Vec<Id>> = BTreeMap::new();
        for n in &p.timeline {
            if n.kind.inputs().iter().any(|i| shapes.contains_key(i)) {
                kinds.entry(kind_of(&p, n.id)).or_default().push(n.id);
            }
        }
        for (kind, nodes) in kinds {
            let handed: Vec<Id> = p.timeline.iter().filter(|n| nodes.contains(&n.id)).flat_map(|n| n.kind.inputs()).collect();
            let before = bytes_of(&shapes, &handed);
            for n in p.timeline.iter_mut().filter(|n| nodes.contains(&n.id)) {
                n.dirty = true;
            }
            let (again, after) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
            shapes = after;
            // a body some node of this rebuild built again is the new one, not the one handed
            let rebuilt: HashSet<Id> = again.spent.iter().filter_map(|(node, _, _)| p.timeline.iter().find(|n| n.id == *node)).flat_map(|n| n.kind.bodies()).collect();
            let now = bytes_of(&shapes, &handed);
            let changed: Vec<String> = before.iter().filter(|(b, was)| !rebuilt.contains(b) && now.get(b) != Some(was)).map(|(b, _)| b.to_string()).collect();
            if !changed.is_empty() {
                let name = doc.rsplit('/').next().unwrap_or(doc);
                moved.push(format!("{name}: {kind} ({} nodes) changed the bodies {}", nodes.len(), changed.join(", ")));
            }
        }
    }
    assert!(moved.is_empty(), "an operation wrote into the bodies it was handed ({}):\n  {}", moved.len(), moved.join("\n  "));
}
