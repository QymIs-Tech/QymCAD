//! EVERY FACE OF A BODY IS DRAWN: a face the mesher gives up on at the drawing's step leaves a hole in the part, the
//! inside seen through it. Reported behaviour, with the private sample of a frame imported from STEP: the R2 rounding
//! of the top of a corner boss left the band of the boss's side below it undrawn - 57 faces in the body, 56 in its
//! mesh, 92 edges of the mesh open around the boss. The band meshes at 0.01 mm and not at 0.05 or 0.2.

/// THE FACES OF THE LAST BODY OF THE SAMPLE, each with triangles, at the steps the window and an export use.
#[test]
fn the_rounded_boss_of_the_frame_sample_is_drawn_whole() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/bugs_fillet_again.qcad");
    if !std::path::Path::new(path).exists() {
        eprintln!("PASSED OVER: the private sample bugs_fillet_again.qcad is not in this tree");
        return;
    }
    let loaded = qymcad_io::load_project_with_brep(path).expect("the sample opens");
    let mut p = loaded.project;
    let seed = loaded.breps.iter().filter_map(|(id, bytes)| qymcad_kernel::specimen::read(bytes).map(|s| (*id, s))).collect();
    let (report, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, seed);
    assert!(report.errors.is_empty(), "the sample did not rebuild: {:?}", report.errors);
    let last = p.timeline.iter().rev().find_map(|n| n.kind.body()).expect("a node with a body");
    let shape = shapes.get(&last).expect("the live body of the last node");
    let faces: u32 = shape.face_kinds().expect("the kinds of its faces").iter().sum();
    let mut problems = Vec::new();
    for step in [0.2, 0.05, 0.01] {
        let drawn: usize = qymcad_kernel::specimen::mesh(shape, step).iter().map(|b| b.faces.len()).sum();
        if drawn != faces as usize {
            problems.push(format!("at {step} mm {drawn} of {faces} faces are drawn"));
        }
    }
    assert!(problems.is_empty(), "the rounded boss has a hole:\n{}", problems.join("\n"));
}
