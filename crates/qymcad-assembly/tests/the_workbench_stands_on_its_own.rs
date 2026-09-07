//! THE ASSEMBLY WORKBENCH, CHECKED WITHOUT THE APPLICATION.
//!
//! `Bench` owns the records and ties the same `JointCtx`. Nothing here is a mock: the document, the joint
//! command and the status line are the real ones.
//!
//! WHAT IS DELIBERATELY NOT CHECKED HERE: mutual exclusion between the picks. Each arming function says so
//! in its own doc comment - "the clearing of every OTHER tool stayed with the application, because mutual
//! exclusion between workbenches is its decision and not the assembly's". A check written here for a rule
//! that lives one floor up would pass while proving nothing.

use qymcad_core::feature::AnchorRef;
use qymcad_ui_state::Bench;

/// SWITCHING A PICK OFF REALLY TURNS IT OFF, AND SAYS SO.
///
/// The arming functions take `on` rather than toggling, so "off" is a separate path through the same
/// function - which is exactly where an off that only rewrites the status line hides. A tool left armed
/// after its button is released eats the next click on the model, and the person reads that as the model
/// having stopped responding.
#[test]
fn switching_a_pick_off_really_turns_it_off() {
    let mut b = Bench::default();

    qymcad_assembly::start_conn_pick_armed(&mut b.joint_ctx(), true);
    assert!(b.joint.conn_pick, "arming must arm");
    let said_on = b.status.clone();
    assert!(!said_on.is_empty(), "arming must say what is being pointed at");

    qymcad_assembly::start_conn_pick_armed(&mut b.joint_ctx(), false);
    assert!(!b.joint.conn_pick, "off must leave the tool disarmed, not merely relabelled");
    assert_ne!(b.status, said_on, "off must not go on saying that a pick is expected");
}

/// A PICK THAT COLLECTS INTO A LIST STARTS THE LIST EMPTY.
///
/// `group_pick` holds `None` when the tool is down and a list when it is up, so arming it twice must not
/// carry the previous run's parts into the new one: they belong to a group the person has already
/// finished with.
#[test]
fn a_collecting_pick_starts_empty_every_time() {
    let mut b = Bench::default();

    qymcad_assembly::start_group_pick_armed(&mut b.joint_ctx(), true);
    b.joint.group_pick.as_mut().expect("the tool is up, so the list exists").push(1);
    qymcad_assembly::start_group_pick_armed(&mut b.joint_ctx(), false);
    assert!(b.joint.group_pick.is_none(), "putting the tool down must take the list with it");

    qymcad_assembly::start_group_pick_armed(&mut b.joint_ctx(), true);
    assert_eq!(b.joint.group_pick.as_deref(), Some(&[][..]), "the next group starts from nobody");
}

/// ARMING THE GROUND PICK DROPS A HALF-MADE MATE.
///
/// Ground is reached WITH a mate half-assembled - one anchor already pointed at - and that first anchor
/// belongs to the mate, not to the part about to be fixed. Left in place it would be taken as the first
/// half of the NEXT mate, and the person would be assembling a joint they never started.
#[test]
fn arming_the_ground_pick_drops_a_half_made_mate() {
    let mut b = Bench::default();
    b.joint.pick_first = Some((1, AnchorRef::Origin));

    qymcad_assembly::start_ground_pick_armed(&mut b.joint_ctx(), true);

    assert!(b.joint.ground_pick, "the ground pick must be armed");
    assert!(b.joint.pick_first.is_none(), "the anchor of the abandoned mate must not become the first half of the next one");
}
