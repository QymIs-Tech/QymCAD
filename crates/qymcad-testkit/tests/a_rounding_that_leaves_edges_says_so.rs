//! A ROUNDING THAT LEAVES EDGES OUT SAYS SO, and builds the rest: a block of 40 x 30 x 10 rounded 11 on an upright edge
//! and a top edge - the upright takes 11 between faces of 30 and 40, the top one cannot, its side being 10 high - and
//! the node carries a warning in words rather than standing green or going red with everything after it.
use qymcad_core::errors::CoreError;
use qymcad_core::model::Project;

#[test]
fn a_rounding_of_two_edges_that_takes_one_says_so() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(40.0, 30.0, 10.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let es = p.regen_edges.get(&block).cloned().expect("the edges of the block");
    let upright = es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 9.0).map(|e| e.id).expect("an upright edge");
    let top = es.iter().find(|e| e.a[2] > 9.9 && e.b[2] > 9.9 && (e.a[0] - e.b[0]).abs() > 39.0).map(|e| e.id).expect("a long top edge");
    let f = p.add_fillet(block, 11.0, vec![upright, top]);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(!report.errors.iter().any(|(id, _)| *id == f), "the rounding went red: {:?}", report.errors);
    assert!(shapes.get(&f).is_some_and(|s| s.volume() < 12000.0 - 1.0), "the upright edge was not rounded");
    let said = p.regen_warnings.get(&f).cloned();
    assert!(matches!(said, Some(CoreError::EdgesDropped { asked: 2, dropped: 1 })), "the rounding that took one of two edges says {said:?}");
}

/// THE ROUNDINGS OF THE PRIVATE SAMPLE vacuumCleaner take all their edges: of R8.6 the one the seam of the round corner
/// beside it ran up from was left out before the seam was moved off it; of the last R2 the two corners beside a crack
/// the rounding before it left (4e-8 of a wall it ate) were left out before cracks were taken out.
#[test]
fn the_rounding_of_the_vacuum_cleaner_takes_all_eight_edges() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/vacuumCleaner.qcad");
    if !std::path::Path::new(path).exists() {
        eprintln!("PASSED OVER: the private sample vacuumCleaner.qcad is not in this tree");
        return;
    }
    let mut p = qymcad_io::load_project(path).expect("the sample opens");
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    // 177: R8.6 on 8 edges, one with the seam of a round corner on its end; 252: R2 on 4 corners, two with a crack
    // of the rounding before it beside them
    for node in [177u64, 252] {
        assert!(!report.errors.iter().any(|(id, _)| *id == node), "the rounding {node} went red: {:?}", report.errors);
        assert!(!p.regen_warnings.contains_key(&node), "the rounding {node} still leaves edges out: {:?}", p.regen_warnings.get(&node));
    }
}

/// AND AFTER AN EDIT: a width of the sample 49 -> 53.9 (the dimension [80, 81] of sketch 3) moved the seam of a round post
/// onto the end of an edge the rounding 177 picked - the seam stood where two arcs of the circle met, and the edit took one
/// of those points away - and R8.6 left that edge out at any radius. The seam stands inside an arc.
#[test]
fn the_rounding_of_the_vacuum_cleaner_takes_all_eight_edges_after_an_edit() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/vacuumCleaner.qcad");
    if !std::path::Path::new(path).exists() {
        eprintln!("PASSED OVER: the private sample vacuumCleaner.qcad is not in this tree");
        return;
    }
    let mut p = qymcad_io::load_project(path).expect("the sample opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    assert!(!p.regen_warnings.contains_key(&177), "setup: the rounding 177 leaves edges out as drawn: {:?}", p.regen_warnings.get(&177));
    let si = p.sketch_index(3).expect("setup: the sketch of the width");
    p.set_dim_target_value(&qymcad_core::model::DimTarget::Sketch { sketch: 3, refs: vec![80, 81] }, 53.9);
    assert!(p.solve_sketch(si) < qymcad_core::model::SKETCH_FITS, "setup: the sketch does not fit the width 53.9");
    p.mark_sketch_dirty(3);
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(!report.errors.iter().any(|(id, _)| *id == 177), "the rounding 177 went red after the edit: {:?}", report.errors);
    assert!(!p.regen_warnings.contains_key(&177), "the rounding 177 leaves edges out after the edit: {:?}", p.regen_warnings.get(&177));
}
