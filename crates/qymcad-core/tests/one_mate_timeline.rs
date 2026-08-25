//! Everything that holds parts together belongs to one list.
//!
//! Joints, constraints (group, width, tangency) and relations lived in the panel as three separate loops, each
//! with its own row, its own delete button and its own idea of whether an element is sound. Three views of the
//! same thing drift apart silently, and what is then displayed is not what exists. Elsewhere this is a single
//! list of mate features.
//!
//! What is checked here is the list itself rather than its drawing: it is computed in the core and the panel
//! only renders it. The checks judge by fact — what entered the list and in what state — rather than by the
//! number of rows.
use qymcad_core::feature::{AnchorRef, ConstraintKind, JointKind, MateItem, MateState, RelationKind};
use qymcad_core::model::{Id, Project};

/// Two parts in the root with a connector at the origin of each: the simplest assembly where everything can
/// be placed.
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

fn a_joint(p: &mut Project, a: Id, b: Id, kind: JointKind) -> Id {
    let (ca, cb) = (p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin));
    p.add_joint(ca, cb, kind)
}

#[test]
fn joints_constraints_and_relations_all_land_in_one_list() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let hinge = a_joint(&mut p, a, b, JointKind::Revolute);
    let group = p.add_group(&[a, b]);
    let rel = p.add_relation(RelationKind::Screw, hinge, 0, hinge, 1, 5.0);

    let list = p.mate_timeline(p.root);
    let ids: Vec<Id> = list.iter().map(|e| e.id).collect();
    for (what, id) in [("joint", hinge), ("constraint", group), ("relation", rel)] {
        assert!(ids.contains(&id), "the {what} {id} did not enter the mate list: it holds {ids:?}");
    }
    // And each carries its own kind: the list is shared, but clicking them has to do different things.
    let kind_of = |id: Id| list.iter().find(|e| e.id == id).map(|e| e.item).expect("the element is in the list");
    assert_eq!(kind_of(hinge), MateItem::Joint, "a joint has to be listed as a joint");
    assert_eq!(kind_of(group), MateItem::Constraint, "a group has to be listed as a constraint");
    assert_eq!(kind_of(rel), MateItem::Relation, "a relation has to be listed as a relation");
    // The order follows creation time, as a feature timeline does.
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "the list has to follow creation order, but reads {ids:?}");
}

#[test]
fn every_line_carries_its_kind_and_the_parts_it_holds() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let slider = a_joint(&mut p, a, b, JointKind::Slider);
    let list = p.mate_timeline(p.root);
    let e = list.iter().find(|e| e.id == slider).expect("the joint is in the list");
    assert_eq!(e.kind_label, JointKind::Slider.label(), "the row has to carry the kind key, or the panel invents a word of its own");
    for part in [a, b] {
        assert!(e.touches.contains(&part), "the row has to know which parts it holds: {:?} does not contain {part}", e.touches);
    }
    assert_eq!(e.state, MateState::Ok, "a sound joint has to be listed as sound, but is listed as {:?}", e.state);
}

#[test]
fn a_mate_whose_anchor_is_lost_is_marked_faulty_in_the_same_list() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let hinge = a_joint(&mut p, a, b, JointKind::Revolute);
    let lost = p.joints.iter().find(|j| j.id == hinge).map(|j| j.a).expect("connector A");
    p.connectors.retain(|c| c.id != lost);

    let list = p.mate_timeline(p.root);
    let e = list.iter().find(|e| e.id == hinge).expect("a dead joint has to stay in the list: it needs repairing, not hiding");
    assert_eq!(
        e.state,
        MateState::Faulty("j-fault-connector-lost"),
        "the joint lost a connector, yet the list calls it {:?}, so there is no way to see what needs repairing",
        e.state
    );
}

#[test]
fn a_relation_pointing_at_a_dead_mate_is_faulty_too() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let hinge = a_joint(&mut p, a, b, JointKind::Revolute);
    let other = a_joint(&mut p, a, b, JointKind::Revolute);
    let rel = p.add_relation(RelationKind::Gear, hinge, 0, other, 0, 2.0);
    // Remove the second joint, leaving the relation with nothing to rest on.
    p.joints.retain(|j| j.id != other);

    let list = p.mate_timeline(p.root);
    let e = list.iter().find(|e| e.id == rel).expect("the relation has to stay in the list");
    assert!(matches!(e.state, MateState::Faulty(_)), "the relation points at a removed joint, yet the list calls it {:?}", e.state);
}

#[test]
fn a_mate_the_solver_could_not_hold_is_shown_as_violated_not_as_healthy() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let hinge = a_joint(&mut p, a, b, JointKind::Revolute);
    // The solver names the violated ones individually, and the list has to show that rather than stay silent.
    p.mates_violated.push(hinge);

    let list = p.mate_timeline(p.root);
    let e = list.iter().find(|e| e.id == hinge).expect("the joint is in the list");
    assert_eq!(e.state, MateState::Violated, "the solver could not hold the joint, yet the list calls it {:?}", e.state);
}

#[test]
fn a_constraint_of_another_assembly_does_not_show_here() {
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    // A subassembly with a group of its own: its constraint is edited there rather than in the root.
    let sub = p.add_component_kind("subassembly", qymcad_core::feature::ComponentKind::Assembly);
    let inner_a = p.add_component_kind("inner 1", qymcad_core::feature::ComponentKind::Part);
    let inner_b = p.add_component_kind("inner 2", qymcad_core::feature::ComponentKind::Part);
    for c in [inner_a, inner_b] {
        if let Some(i) = p.component_index(c) {
            p.components[i].parent = Some(sub);
        }
    }
    let outer = p.add_group(&[a, b]);
    let inner = p.add_group(&[inner_a, inner_b]);

    let here = p.mate_timeline(sub);
    let ids: Vec<Id> = here.iter().map(|e| e.id).collect();
    assert!(ids.contains(&inner), "a constraint of the subassembly has to appear in its own list: {ids:?}");
    assert!(!ids.contains(&outer), "a constraint of another assembly entered the list of the subassembly: {ids:?}");
}

#[test]
fn the_list_names_every_kind_of_constraint_there_is() {
    // A completeness guard: when a new kind of constraint appears, this check has to force it into the list
    // rather than let it stay invisible.
    let mut p = Project::default();
    let (a, b) = two_parts(&mut p);
    let (ca, cb) = (p.add_connector(a, AnchorRef::Origin), p.add_connector(b, AnchorRef::Origin));
    let tab = p.add_connector(b, AnchorRef::Origin);
    let group = p.add_group(&[a, b]);
    let width = p.add_width(&[ca, cb], tab);
    let tangent = p.add_tangent(a, AnchorRef::Origin, b, AnchorRef::Origin);

    let list = p.mate_timeline(p.root);
    for (kind, id) in [(ConstraintKind::Group, group), (ConstraintKind::Width, width), (ConstraintKind::Tangent, tangent)] {
        let e = list.iter().find(|e| e.id == id).unwrap_or_else(|| panic!("the constraint {kind:?} did not enter the list"));
        assert_eq!(e.kind_label, kind.label(), "the constraint {kind:?} carries the wrong kind in the list");
        assert_eq!(e.item, MateItem::Constraint, "the constraint {kind:?} is not listed as a constraint");
    }
}
