//! A REFERENCE FOUND BY PLACE WAITS FOR THE PERSON. Its name lost, a reference finds the element standing exactly where it
//! saw it, is rewritten to that name and holds by name from then on; the step stays marked - yellow, in words - until the
//! person accepts what was taken or picks it again, in this session and in the file.
use qymcad_core::errors::CoreError;
use qymcad_core::feature::FeatureKind;
use qymcad_core::model::{ElemSnapshot, Project};
use qymcad_core::names::{EdgeName, GeoName, Role};

#[test]
fn a_rounding_whose_edge_was_found_by_place_stays_marked_until_accepted() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(40.0, 30.0, 10.0);
    let (_, shapes) = qymcad_testkit::regenerate(&mut p);
    let edge = p.regen_edges[&block].iter().find(|e| (e.a[2] - e.b[2]).abs() > 5.0).cloned().expect("an upright edge of the block");
    let fillet = p.add_fillet(block, 2.0, vec![edge.id]);
    let (_, shapes) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(p.placed_refs.is_empty() && p.node_warning(fillet).is_none(), "a rounding of an edge picked by name is marked");
    // the name of the edge lost: a pair of faces no body has, the edge seen where the upright edge stands
    let nowhere = [1, 2].map(|k| p.names.intern_face(GeoName::new(999, Role::Wall, k)));
    let lost = p.names.intern_edge(EdgeName::new(nowhere[0], nowhere[1], 0));
    for n in p.timeline.iter_mut().filter(|n| n.id == fillet) {
        if let FeatureKind::Fillet { edges, .. } = &mut n.kind {
            *edges = qymcad_core::refs::Ref::picks(&[lost]);
        }
        n.dirty = true;
    }
    p.edge_refs.insert(fillet, vec![ElemSnapshot { id: lost, at: edge.mid, dir: edge.dir }]);
    let (report, shapes) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "the rounding did not build on the edge found by place: {:?}", report.errors);
    assert!(p.placed_refs.contains(&fillet), "the rounding whose edge was found by place is not marked");
    assert_eq!(p.node_warning(fillet), Some(CoreError::RefFoundByPlace), "the rounding does not say its edge was found by place");
    // rebuilt again, the reference holds by name - and the step still waits for the person
    p.mark_node_dirty(fillet);
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(p.placed_refs.contains(&fillet), "the mark went with the next rebuild, before anyone looked");
    // and in the file
    let dir = std::env::temp_dir().join(format!("qym_found_by_place_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder for the file");
    let path = dir.join("found.qcad").to_string_lossy().into_owned();
    qymcad_io::save_project(&p, &path).expect("the document is written");
    let mut q = qymcad_io::load_project(&path).expect("the document reads");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(q.node_warning(fillet), Some(CoreError::RefFoundByPlace), "opened again, the rounding no longer says its edge was found by place");
    assert!(q.accept_found_by_place(fillet), "accepting found no mark");
    assert!(q.node_warning(fillet).is_none() && q.placed_refs.is_empty(), "accepted, the rounding is still marked");
}
