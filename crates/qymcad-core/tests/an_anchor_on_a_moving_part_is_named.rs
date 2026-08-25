//! An anchor attached to a moving part is named aloud.
//!
//! The trouble this exists for: an assembly drifted apart on its own, every recomputation carrying it 60 mm
//! further, without end. The cause lay neither in the solver, both halves of the computation converging at a
//! residual of 1e-10, nor in the grounding, but in an anchor: a joint declared its anchor on one component
//! while taking the geometry from a part that travels inside that very component. The plate moves, the anchor
//! moves with it, the outer component catches up, and the plate moves again.
//!
//! Such an arrangement must not be carried round in silence: the cause gets looked for anywhere except the
//! anchor that was placed on the wrong part. The program has to name it.
use qymcad_core::feature::{AnchorRef, ComponentKind, JointKind};
use qymcad_core::model::{Id, Project};

/// A subassembly with two parts inside and a slider between them: one part stays still and the other
/// travels.
fn a_subassembly_with_a_moving_part(p: &mut Project) -> (Id, Id, Id) {
    let root = p.ensure_root();
    let sub = p.add_component_kind("subassembly", ComponentKind::Assembly);
    let still = p.add_component_kind("still", ComponentKind::Part);
    let moving = p.add_component_kind("moving", ComponentKind::Part);
    for (c, parent) in [(sub, root), (still, sub), (moving, sub)] {
        if let Some(i) = p.component_index(c) {
            p.components[i].parent = Some(parent);
        }
    }
    // A slider inside the subassembly: the moving part travels relative to the still one. The still part is
    // grounded, because without a root of reference there is no telling which drives which at all, and the
    // trap guard would honestly report that the document holds no moving part.
    p.set_grounded(still, true);
    let (ca, cb) = (p.add_connector(still, AnchorRef::Origin), p.add_connector(moving, AnchorRef::Origin));
    p.add_joint(ca, cb, JointKind::Slider);
    (sub, still, moving)
}


/// A body belonging to a part. A body lives in the document through a timeline node whose owner is a
/// component, and `body_owner` asks the timeline.
fn body_inside(p: &mut Project, comp: Id) -> Id {
    let body = p.alloc_id();
    let node = p.alloc_id();
    p.timeline.push(qymcad_core::feature::FeatureNode {
        id: node,
        name: "body".into(),
        kind: qymcad_core::feature::FeatureKind::Import { body, source: 0, solid: 0 },
        parent: Some(comp),
        dirty: false,
        suppressed: false,
    });
    body
}

#[test]
fn an_anchor_whose_geometry_lives_in_a_moving_part_is_called_faulty() {
    let mut p = Project::default();
    let (sub, _still, moving) = a_subassembly_with_a_moving_part(&mut p);
    let outer = p.add_component_kind("outside", ComponentKind::Part);
    let root = p.root;
    if let Some(i) = p.component_index(outer) {
        p.components[i].parent = Some(root);
    }

    // A joint from outside: the anchor is declared on the subassembly while the geometry comes from the moving
    // part inside it. The body goes into the moving part and its face is what gets referenced.
    let body = body_inside(&mut p, moving);
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 1 };
    let bad = p.add_connector(sub, AnchorRef::FaceCenter(body, key));
    let out = p.add_connector(outer, AnchorRef::Origin);
    let jid = p.add_joint(out, bad, JointKind::Slider);

    // A trap guard: the moving part really does move, being driven by a joint.
    assert!(p.drive_joint_for(moving).is_some(), "guard: the inner part has to be movable, or there is no fault to find");

    let faults = p.joint_faults();
    assert!(
        faults.iter().any(|(id, why)| *id == jid && *why == "j-fault-anchor-on-moving-part"),
        "the anchor holds on to a part that travels inside the same assembly, and the program says nothing: {faults:?}"
    );
}

#[test]
fn an_anchor_on_stationary_geometry_of_its_own_assembly_is_fine() {
    // The opposite guard: an ordinary arrangement, where the body sits inside its own assembly and goes
    // nowhere, must not raise anything. Otherwise the warning becomes noise and stops being noticed.
    let mut p = Project::default();
    let (sub, still, _moving) = a_subassembly_with_a_moving_part(&mut p);
    let outer = p.add_component_kind("outside", ComponentKind::Part);
    let root = p.root;
    if let Some(i) = p.component_index(outer) {
        p.components[i].parent = Some(root);
    }
    let body = body_inside(&mut p, still);
    let key = qymcad_core::feature::FaceKey { index: 0, centroid: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 1 };
    let ok = p.add_connector(sub, AnchorRef::FaceCenter(body, key));
    let out = p.add_connector(outer, AnchorRef::Origin);
    let jid = p.add_joint(out, ok, JointKind::Slider);

    let faults = p.joint_faults();
    assert!(
        !faults.iter().any(|(id, why)| *id == jid && *why == "j-fault-anchor-on-moving-part"),
        "the anchor rests on stationary geometry of its own assembly, yet was declared faulty: {faults:?}"
    );
}
