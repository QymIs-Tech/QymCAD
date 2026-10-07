//! Two pieces of a split body, rebuilt on two worker threads at once, crashed the program.
//!
//! The pieces of "Split body" hold the SAME cut face (one kernel sub-shape, not a copy). Meshing or healing that
//! face on two threads at once rewrites its triangulation twice over and the process dies. The batch rebuild asks
//! `Kernel::shares` and keeps bodies that share a sub-shape on one thread (see `model/regen.rs`).
//!
//! This drives the real path: a box split in two, a fillet on each piece - so each piece is the input of its own
//! timeline node, and the two fillet nodes are the only pair the walk finds ready together. The rebuild report's
//! `waves` lists how many nodes each batch sent to separate threads side by side. Because the two pieces share the
//! cut face, the dispatch must keep them together: no batch sends two. Without the fix the two fillets travel side
//! by side (a wave of 2, red); with it they stay on one thread (no wave of 2, green). Checked deterministically on
//! the report rather than by waiting for the race to fire.
use qymcad_core::model::Project;

fn volume_of(p: &Project, b: u64) -> f64 {
    p.bodies.iter().find(|x| x.id == b).map(|x| x.mesh.volume()).unwrap_or(0.0)
}

fn build() -> (Project, Vec<u64>) {
    let mut p = Project::default();
    let root = p.ensure_root();
    p.set_active_component(Some(root));
    let part = p.add_part("part");
    p.set_active_component(Some(part));
    let body = p.add_box(20.0, 20.0, 20.0);
    let _ = qymcad_testkit::regenerate(&mut p);
    let pieces = p.add_split_body(body, 0, 0, 10.0, 2);
    let _ = qymcad_testkit::regenerate(&mut p);
    let mut filleted = Vec::new();
    for &pc in &pieces {
        let edge = p.regen_edges.get(&pc).and_then(|es| es.first().map(|e| e.id)).expect("the piece has an edge to fillet");
        filleted.push(p.add_fillet(pc, 1.0, vec![edge]));
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup must build: {:?}", rep.errors);
    (p, filleted)
}

#[test]
fn split_pieces_that_share_the_cut_face_are_not_rebuilt_side_by_side() {
    qymcad_kernel::set_parallel(true, 4);
    let (mut p, filleted) = build();

    // one full rebuild: the two fillet nodes are the only pair ready together, and their pieces share the cut
    // face, so no batch may send two nodes side by side.
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (rep, _) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "the rebuild must pass: {:?}", rep.errors);
    assert!(rep.waves.iter().all(|&w| w < 2), "the two pieces share the cut face and must not be meshed side by side, but a batch sent {:?} nodes to separate threads", rep.waves);

    // and the geometry is intact (the fillets built, nothing was lost)
    for (i, &b) in filleted.iter().enumerate() {
        assert!(volume_of(&p, b) > 1.0, "piece {i} lost its volume: {}", volume_of(&p, b));
    }

    qymcad_kernel::set_parallel(true, 0); // back to the default worker count
}
