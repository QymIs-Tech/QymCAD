//! A chain of mates moves as one rather than lagging behind.
//!
//! The reported behaviour: with two joints in a chain, dragging the first one leaves the third part trailing
//! and catching up later, so the mechanism behaves like jelly. The first joint is horizontal and the second
//! vertical; dragging the first one horizontally leaves the third part behind.
//!
//! The mechanism itself is rigid: the third part is joined to the second by a vertical slider, so horizontally
//! it has to follow the second one exactly, having no freedom in that direction at all. Trailing and catching
//! up means one thing: a single call does not carry the chain to completion and the remainder is worked off
//! over the following frames. That is not slow, it is wrong — what is seen is a mechanism coming apart under
//! the hand.
//!
//! What is measured here is exactly that: after every step of the drag the third part stands where its joints
//! hold it.
use qymcad_core::feature::{apply12, AnchorRef, BasePlane, JointKind};
use qymcad_core::model::{Id, Project};

fn tr(x: f64, y: f64, z: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

fn set_transform(p: &mut Project, comp: Id, m: [f64; 12]) {
    let i = p.component_index(comp).unwrap();
    p.components[i].transform = m;
}

fn at(p: &Project, comp: Id) -> [f64; 3] {
    apply12(&p.world_transform(comp), [0.0, 0.0, 0.0])
}

/// Three parts in a chain: A is grounded, A to B is a slider along X, B to C a slider along Z.
///
/// The axes come from the base planes of a component: the normal of YZ is X and the normal of XY is Z. This
/// way a joint gets its own axis without depending on whether the live geometry has come up.
fn a_chain(p: &mut Project) -> (Id, Id, Id, Id) {
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let (a, b, c) = (p.add_part("A"), p.add_part("B"), p.add_part("C"));
    p.set_grounded(a, true);
    set_transform(p, b, tr(0.0, 0.0, 0.0));
    set_transform(p, c, tr(0.0, 0.0, 0.0));

    let ja = p.add_connector(a, AnchorRef::BasePlane(BasePlane::YZ)); // a normal along X gives horizontal travel
    let jb = p.add_connector(b, AnchorRef::BasePlane(BasePlane::YZ));
    let horizontal = p.add_joint(ja, jb, JointKind::Slider);

    let kb = p.add_connector(b, AnchorRef::BasePlane(BasePlane::XY)); // a normal along Z gives vertical travel
    let kc = p.add_connector(c, AnchorRef::BasePlane(BasePlane::XY));
    p.add_joint(kb, kc, JointKind::Slider);

    p.solve_joints();
    (a, b, c, horizontal)
}

/// Driving the first joint carries the third part along with the second, at every step.
#[test]
fn dragging_the_first_mate_carries_the_whole_chain_every_step() {
    let mut p = Project::default();
    let (_, b, c, horizontal) = a_chain(&mut p);

    // a drag is a series of small steps rather than a single jump
    let mut worst = 0.0f64;
    for k in 1..=8 {
        let want = 7.5 * k as f64;
        p.joints.iter_mut().find(|x| x.id == horizontal).unwrap().drive[1] = Some(want);
        p.solve_joints();
        let (pb, pc) = (at(&p, b), at(&p, c));
        assert!((pb[0] - want).abs() < 1e-3, "step {k}: the second part fell short, expected x={want} but it is at {pb:?}");
        // C is joined to B by a vertical slider, so it has no horizontal freedom and its x and y match B
        let lag = ((pc[0] - pb[0]).powi(2) + (pc[1] - pb[1]).powi(2)).sqrt();
        worst = worst.max(lag);
        assert!(
            lag < 1e-3,
            "step {k}: the third part trails the second by {lag:.3} mm (B at {pb:?}, C at {pc:?}), which is the lag"
        );
    }
    assert!(worst < 1e-3, "worst lag over the drag: {worst:.3} mm");
}

/// And in reverse: dragging back leaves no tail either.
#[test]
fn dragging_the_chain_back_leaves_nothing_behind() {
    let mut p = Project::default();
    let (_, b, c, horizontal) = a_chain(&mut p);
    for want in [60.0, 45.0, 30.0, 15.0, 0.0] {
        p.joints.iter_mut().find(|x| x.id == horizontal).unwrap().drive[1] = Some(want);
        p.solve_joints();
        let (pb, pc) = (at(&p, b), at(&p, c));
        let lag = ((pc[0] - pb[0]).powi(2) + (pc[1] - pb[1]).powi(2)).sqrt();
        assert!(lag < 1e-3, "dragging back to {want}: the third part trails by {lag:.3} mm (B {pb:?}, C {pc:?})");
    }
}

/// A single solve finishes the job.
///
/// The same property checked from the other side: if a second solve changes anything without a single edit to
/// the document, the first one did not finish, and what is seen on screen is a mechanism catching up.
#[test]
fn solving_twice_changes_nothing_the_first_solve_should_have_done() {
    let mut p = Project::default();
    let (_, b, c, horizontal) = a_chain(&mut p);
    p.joints.iter_mut().find(|x| x.id == horizontal).unwrap().drive[1] = Some(60.0);
    p.solve_joints();
    let (b1, c1) = (at(&p, b), at(&p, c));
    p.solve_joints();
    let (b2, c2) = (at(&p, b), at(&p, c));
    let moved = |x: [f64; 3], y: [f64; 3]| ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt();
    assert!(moved(b1, b2) < 1e-6, "a repeated solve moved the second part by {:.4} mm: {b1:?} -> {b2:?}", moved(b1, b2));
    assert!(moved(c1, c2) < 1e-6, "a repeated solve moved the third part by {:.4} mm: {c1:?} -> {c2:?}", moved(c1, c2));
}
