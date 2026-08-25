//! Grounding inside a moving subassembly is named aloud.
//!
//! "Grounded" reads as "it stands still and goes nowhere". But a part lives inside a subassembly, and the
//! subassembly is driven by a joint of its own: when it travels, the grounded part travels with it. The word is
//! there and means nothing.
//!
//! Measured on a real machine: a grounded part sat inside a beam; driving the gantry carried it 100.000 mm along
//! with the beam. There were three such parts in that document, and the program said nothing.
use qymcad_core::feature::{AnchorRef, ComponentKind, JointKind};
use qymcad_core::model::{Id, Project};

/// A subassembly driven by a joint, and a part inside it.
///
/// Returns the moving subassembly, the part inside it, and a stationary part outside.
fn a_moving_subassembly(p: &mut Project) -> (Id, Id, Id) {
    let root = p.ensure_root();
    let still = p.add_component_kind("support", ComponentKind::Part);
    let moving = p.add_component_kind("moving unit", ComponentKind::Assembly);
    let inside = p.add_component_kind("inner part", ComponentKind::Part);
    for (c, parent) in [(still, root), (moving, root), (inside, moving)] {
        if let Some(i) = p.component_index(c) {
            p.components[i].parent = Some(parent);
        }
    }
    p.set_grounded(still, true);
    // the joint drives the whole subassembly: it is the moving unit
    let (ca, cb) = (p.add_connector(still, AnchorRef::Origin), p.add_connector(moving, AnchorRef::Origin));
    p.add_joint(ca, cb, JointKind::Slider);
    (moving, inside, still)
}

#[test]
fn a_part_grounded_inside_a_moving_subassembly_is_listed() {
    let mut p = Project::default();
    let (moving, inside, _still) = a_moving_subassembly(&mut p);
    p.set_grounded(inside, true);

    // a trap guard: the subassembly really does move, or there is no fault at all
    assert!(p.drive_joint_for(moving).is_some(), "guard: the subassembly has to be driven by a joint, or there is nothing to check");

    let named = p.grounded_inside_moving();
    assert!(
        named.contains(&inside),
        "a part is grounded inside a moving unit, so its grounding means nothing, and the program stays silent: named {named:?}"
    );
}

#[test]
fn a_part_grounded_in_a_still_assembly_is_not_bothered() {
    // The opposite guard: ordinary grounding is no reason to raise anything. Otherwise the warning becomes
    // noise and stops being noticed.
    let mut p = Project::default();
    let (_moving, _inside, still) = a_moving_subassembly(&mut p);
    let named = p.grounded_inside_moving();
    assert!(
        !named.contains(&still),
        "a grounded part in the root was declared doubtful although nothing moves it: {named:?}"
    );
}

#[test]
fn a_part_inside_a_still_subassembly_is_not_bothered_either() {
    // And one more of the same kind: a part inside a stationary subassembly is grounded honestly — nothing
    // drives the subassembly, so the part stands still.
    let mut p = Project::default();
    let root = p.ensure_root();
    let sub = p.add_component_kind("still unit", ComponentKind::Assembly);
    let inside = p.add_component_kind("inner part", ComponentKind::Part);
    for (c, parent) in [(sub, root), (inside, sub)] {
        if let Some(i) = p.component_index(c) {
            p.components[i].parent = Some(parent);
        }
    }
    p.set_grounded(inside, true);
    let named = p.grounded_inside_moving();
    assert!(named.is_empty(), "nothing drives the subassembly, yet the part inside was declared doubtful: {named:?}");
}
