//! A picked axis is the axis of travel.
//!
//! The report: picking a direction axis on a slider set it, while the gizmo of the slider and the axis it should
//! travel along did not change and always pointed vertically.
//!
//! That is what happened: the pick was written into the secondary axis, the roll, while the travel of a slider
//! follows the normal of the frame. On an anchor placed on a face it worked by accident, the travel being
//! rebuilt from X there, while on an anchor by an edge or by origins the travel stayed as it was. For an anchor
//! by origins the frame is the identity and its normal is the world Z, hence "always vertical".
use qymcad_core::feature::{AnchorRef, JointKind};
use qymcad_core::model::{Id, Project};

/// Two parts in the root: enough to place a joint and ask it for its travel axis.
fn two_parts(p: &mut Project) -> (Id, Id) {
    let root = p.ensure_root();
    let a = p.add_component_kind("A", qymcad_core::feature::ComponentKind::Part);
    let b = p.add_component_kind("B", qymcad_core::feature::ComponentKind::Part);
    for c in [a, b] {
        if let Some(i) = p.component_index(c) {
            p.components[i].parent = Some(root);
        }
    }
    (a, b)
}

#[test]
fn a_slider_on_origins_travels_along_the_axis_you_point_at() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let (ca, cb) = (p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin));
    let jid = p.add_joint(ca, cb, JointKind::Slider);

    // a trap guard: without a pick the travel really does follow the world Z, the vertical complained of
    let before = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis before the pick");
    assert!(before[2].abs() > 0.9, "guard: without a pick the travel has to be vertical, and it is {before:?}, so there is no trap");

    // another axis is picked: a base plane whose normal points along X
    if let Some(c) = p.connectors.iter_mut().find(|c| c.id == ca) {
        c.axis_ref = Some(AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
    }
    let after = p.joint_slot_axis(jid, 1, p.root).expect("the travel axis after the pick");
    assert!(
        after[0].abs() > 0.9,
        "an axis along X was picked and the slider still travels {after:?}, so the pick did not reach the travel"
    );
}

#[test]
fn pointing_at_an_axis_changes_where_the_part_actually_goes() {
    // Turning the arrow is not enough: the part has to travel the same way. The arrow and the travel are
    // computed from one source, but what has to be checked is the fact rather than two calls to one function
    // agreeing.
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    p.set_grounded(a, true);
    let (ca, cb) = (p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin));
    let jid = p.add_joint(ca, cb, JointKind::Slider);
    if let Some(c) = p.connectors.iter_mut().find(|c| c.id == ca) {
        c.axis_ref = Some(AnchorRef::BasePlane(qymcad_core::feature::BasePlane::YZ));
    }

    let m = p.world_transform(b);
    let was = [m[3], m[7], m[11]];
    let base = p.joints.iter().find(|x| x.id == jid).map(|x| x.offset).unwrap_or(0.0);
    if let Some(j) = p.joints.iter_mut().find(|x| x.id == jid) {
        j.drive[1] = Some(base + 12.0);
    }
    p.solve_joints();
    let m = p.world_transform(b);
    let went = [m[3] - was[0], m[7] - was[1], m[11] - was[2]];
    let len = (went[0] * went[0] + went[1] * went[1] + went[2] * went[2]).sqrt();
    assert!((len - 12.0).abs() < 1e-3, "the part has to travel 12 mm and travelled {len:.4}");
    assert!(
        went[0].abs() > 11.9,
        "an axis along X was picked, so the part has to travel along X, and it went {went:?}"
    );
}
