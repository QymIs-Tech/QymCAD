//! A DELETION IS SAVED THE SAME WAY EVERY TIME.
//!
//! The bodies a deletion takes away are remembered in the document (`dead_bodies`, saved with it), so that a mate on
//! one of them can say why it went red. They were collected in a `HashSet` and remembered in its order, which is
//! chosen at random for every set: deleting a sketch of the Filter example remembered its four bodies as
//! [13, 26, 25, 14] in one run and [25, 14, 13, 26] in the next, and the same edit saved twice gave two different
//! files. Each deletion's bodies are now appended by id, so the same edits save the same file, and a sketch's
//! deletion returns them by id too.
use qymcad_core::model::{Id, Project};
use qymcad_io::load_project;

fn example(name: &str) -> Project {
    load_project(&format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))).expect("the example opens")
}

fn by_id(ids: &[Id]) -> bool {
    ids.windows(2).all(|w| w[0] < w[1])
}

/// Every sketch of both examples deleted on a copy: the bodies it takes away are appended by id. The check bites:
/// sketch 3 of the Filter example takes four bodies with it (13, 14, 25, 26).
#[test]
fn a_sketch_deleted_leaves_its_bodies_by_id() {
    let mut seen = 0;
    for file in ["Filter-v2.qcad", "QymBoxPc-Case.qcad"] {
        let project = example(file);
        for sid in project.sketches.iter().map(|s| s.id) {
            let mut p = project.clone();
            let before = p.dead_bodies.len();
            let returned = p.delete_sketch_with_dependents(sid);
            let remembered = p.dead_bodies[before..].to_vec();
            if remembered.len() >= 2 {
                seen += 1;
            }
            assert!(by_id(&returned), "{file}: deleting sketch {sid} returned its bodies as {returned:?}");
            assert!(by_id(&remembered), "{file}: deleting sketch {sid} remembered its bodies as {remembered:?}");
        }
    }
    assert!(seen > 0, "no sketch of the examples takes two bodies with it: the check checks nothing");
}

/// Every body of both examples deleted with what stands on it: the bodies are appended by id. Deleting body 13 of the
/// Filter example removes four bodies (13, 14, 25, 26).
#[test]
fn a_body_deleted_leaves_the_bodies_by_id() {
    let mut seen = 0;
    for file in ["Filter-v2.qcad", "QymBoxPc-Case.qcad"] {
        let project = example(file);
        let bodies: Vec<Id> = project.timeline.iter().flat_map(|n| n.kind.bodies()).collect();
        for b in bodies {
            let mut p = project.clone();
            let before = p.dead_bodies.len();
            p.delete_body_cascade(b);
            let remembered = p.dead_bodies[before..].to_vec();
            if remembered.len() >= 2 {
                seen += 1;
            }
            assert!(by_id(&remembered), "{file}: deleting body {b} remembered the bodies as {remembered:?}");
        }
    }
    assert!(seen > 0, "no body of the examples takes another with it: the check checks nothing");
}
