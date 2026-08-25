//! Pulling a part carries the whole chain, asked of the core alone without the interface.
//!
//! The interface half of this work was rolled back as unfinished, and the question of whether the target does
//! not work or the hand does not set it has to be separated. Here the target is written straight into the
//! document and only the solver is asked.
use qymcad_core::feature::{apply12, AnchorRef, BasePlane, JointKind};
use qymcad_core::model::{Id, Project};

fn at(p: &Project, c: Id) -> [f64; 3] {
    apply12(&p.world_transform(c), [0.0, 0.0, 0.0])
}

/// A is grounded, A to B is a slider along X and B to C a slider along Z. Pulling C diagonally has to move
/// both degrees of freedom.
#[test]
fn pulling_the_last_part_moves_both_links() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let (a, b, c) = (p.add_part("A"), p.add_part("B"), p.add_part("C"));
    p.set_grounded(a, true);
    let ja = p.add_connector(a, AnchorRef::BasePlane(BasePlane::YZ)); // normal along X
    let jb = p.add_connector(b, AnchorRef::BasePlane(BasePlane::YZ));
    p.add_joint(ja, jb, JointKind::Slider);
    let kb = p.add_connector(b, AnchorRef::BasePlane(BasePlane::XY)); // normal along Z
    let kc = p.add_connector(c, AnchorRef::BasePlane(BasePlane::XY));
    p.add_joint(kb, kc, JointKind::Slider);
    p.solve_joints();

    let (b0, c0) = (at(&p, b), at(&p, c));
    // pull C by 30 along X and 20 along Z: only the first link can give along X and only the second along Z
    p.drag_pull = Some((c, [0.0, 0.0, 0.0], [c0[0] + 30.0, c0[1], c0[2] + 20.0]));
    p.solve_joints();
    p.drag_pull = None;

    let (b1, c1) = (at(&p, b), at(&p, c));
    assert!((b1[0] - b0[0]).abs() > 1.0, "the first link did not move along X: {} -> {}", b0[0], b1[0]);
    assert!((c1[2] - c0[2]).abs() > 1.0, "the second link did not move along Z: {} -> {}", c0[2], c1[2]);
}
