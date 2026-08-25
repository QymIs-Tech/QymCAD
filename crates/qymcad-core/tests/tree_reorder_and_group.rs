//! Ordering in the tree and grouping into a subassembly.
//!
//! Two operations:
//!
//! * dragging parts and subassemblies within an assembly so that their order in the tree can be changed for
//!   convenience — pure cosmetics, with no effect on the model;
//! * dropping a selection onto another part or assembly, which creates a new subassembly holding everything
//!   selected together with whatever it was dropped onto.
//!
//! Each lives in the core as a single method, and the tree in the panel only calls them. That leaves something
//! to test, and keeps the logic of the model out of the mouse handling.
use qymcad_core::feature::ComponentKind;
use qymcad_core::model::{Id, Project};

/// An assembly with three parts inside. Returns the assembly and [A, B, C].
fn asm_with_three(p: &mut Project) -> (Id, [Id; 3]) {
    let asm = p.add_assembly("Unit");
    p.set_active_component(Some(asm));
    let a = p.add_part("A");
    let b = p.add_part("B");
    let c = p.add_part("C");
    (asm, [a, b, c])
}

fn order_in(p: &Project, parent: Id) -> Vec<String> {
    p.components.iter().filter(|c| c.parent == Some(parent)).map(|c| c.name.clone()).collect()
}

// ── ordering ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_component_moves_before_its_sibling() {
    let mut p = Project::default();
    p.new_document();
    let (asm, [a, b, c]) = asm_with_three(&mut p);
    assert_eq!(order_in(&p, asm), ["A", "B", "C"], "the initial order");

    assert!(p.reorder_component_before(c, Some(a)), "the reordering has to go through");
    assert_eq!(order_in(&p, asm), ["C", "A", "B"], "C did not move in front of A");
    let _ = b;
}

#[test]
fn a_component_moves_to_the_end() {
    let mut p = Project::default();
    p.new_document();
    let (asm, [a, _b, _c]) = asm_with_three(&mut p);

    assert!(p.reorder_component_before(a, None), "reordering to the end has to go through");
    assert_eq!(order_in(&p, asm), ["B", "C", "A"], "A did not move to the end");
}

/// The children of other assemblies are not mixed in. "To the end" means the end of one's own siblings rather
/// than of the whole vector; otherwise a part of one assembly would travel past the children of another and the
/// tree would rearrange somewhere other than where the drag happened.
#[test]
fn moving_to_the_end_stays_within_the_parent() {
    let mut p = Project::default();
    p.new_document();
    let (asm1, [a, _b, _c]) = asm_with_three(&mut p);
    p.set_active_component(Some(p.root));
    let asm2 = p.add_assembly("Second unit");
    p.set_active_component(Some(asm2));
    let x = p.add_part("X");

    assert!(p.reorder_component_before(a, None));
    assert_eq!(order_in(&p, asm1), ["B", "C", "A"], "A did not move to the end of its own assembly");
    assert_eq!(order_in(&p, asm2), ["X"], "the other assembly should not have been affected");
    let _ = x;
}

/// A non-sibling is not reordered. Dragging into a different parent is a move rather than a change of order:
/// the placement has to be recomputed there, and the two operations must not be confused.
#[test]
fn reordering_across_parents_is_refused() {
    let mut p = Project::default();
    p.new_document();
    let (_asm1, [a, _b, _c]) = asm_with_three(&mut p);
    p.set_active_component(Some(p.root));
    let asm2 = p.add_assembly("Second unit");
    p.set_active_component(Some(asm2));
    let x = p.add_part("X");

    assert!(!p.reorder_component_before(a, Some(x)), "reordering across a parent boundary has to be rejected");
}

#[test]
fn reordering_a_component_before_itself_changes_nothing() {
    let mut p = Project::default();
    p.new_document();
    let (asm, [a, _b, _c]) = asm_with_three(&mut p);

    assert!(!p.reorder_component_before(a, Some(a)));
    assert_eq!(order_in(&p, asm), ["A", "B", "C"], "the order should not have changed");
}

// ── grouping ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn dropping_a_selection_onto_a_part_makes_a_subassembly_with_both() {
    let mut p = Project::default();
    p.new_document();
    let (asm, [a, b, c]) = asm_with_three(&mut p);

    let new_asm = p.group_components_into_assembly(&[a, b], c, "Group").expect("the subassembly is created");
    assert_eq!(p.component_kind(new_asm), Some(ComponentKind::Assembly), "the new component is an assembly");

    let inside = order_in(&p, new_asm);
    assert!(inside.contains(&"A".to_string()) && inside.contains(&"B".to_string()), "the selected components did not end up inside: {inside:?}");
    assert!(inside.contains(&"C".to_string()), "the drop target did not end up inside: {inside:?}");
    assert_eq!(inside.len(), 3, "exactly three have to be inside: {inside:?}");

    // And the subassembly sits where the target sat, rather than surfacing in the root.
    assert_eq!(order_in(&p, asm), ["Group"], "the subassembly has to appear where the target was: {:?}", order_in(&p, asm));
}

/// World positions do not change. Otherwise grouping for the sake of tidiness in the tree would scatter the
/// assembly: cosmetics that damage the model.
#[test]
fn grouping_does_not_move_anything() {
    let mut p = Project::default();
    p.new_document();
    let (_asm, [a, b, c]) = asm_with_three(&mut p);
    p.move_component(a, [10.0, 0.0, 0.0]);
    p.move_component(b, [0.0, 20.0, 0.0]);
    let before: Vec<[f64; 12]> = [a, b, c].iter().map(|&i| p.world_transform(i)).collect();

    p.group_components_into_assembly(&[a, b], c, "Group").expect("the subassembly is created");

    let after: Vec<[f64; 12]> = [a, b, c].iter().map(|&i| p.world_transform(i)).collect();
    for (k, (x, y)) in before.iter().zip(after.iter()).enumerate() {
        for j in 0..12 {
            assert!((x[j] - y[j]).abs() < 1e-9, "part {k} moved during grouping: {x:?} -> {y:?}");
        }
    }
}

/// An ancestor is not grouped into its own descendant, which would be a cycle. Such a member is simply left
/// out.
#[test]
fn an_ancestor_is_not_dragged_into_its_own_descendants_group() {
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_assembly("Outer");
    p.set_active_component(Some(outer));
    let inner = p.add_assembly("Inner");
    p.set_active_component(Some(inner));
    let leaf = p.add_part("Leaf");
    p.set_active_component(Some(outer));
    let other = p.add_part("Neighbour");

    let new_asm = p.group_components_into_assembly(&[outer, other], leaf, "Group").expect("the subassembly is created");
    let inside = order_in(&p, new_asm);
    assert!(inside.contains(&"Leaf".to_string()), "the target did not end up inside: {inside:?}");
    assert!(!inside.contains(&"Outer".to_string()), "an ancestor was pulled inside its own descendant: {inside:?}");
    let _ = inner;
}

/// A group of one is not a group. And no empty subassembly is left in the tree after the refusal.
#[test]
fn grouping_a_single_component_is_refused_and_leaves_no_empty_assembly() {
    let mut p = Project::default();
    p.new_document();
    let (asm, [a, _b, _c]) = asm_with_three(&mut p);
    let before = p.components.len();

    assert!(p.group_components_into_assembly(&[], a, "Group").is_none(), "a group of one has to be rejected");
    assert_eq!(p.components.len(), before, "an empty assembly was left in the tree after the refusal");
    assert_eq!(order_in(&p, asm), ["A", "B", "C"], "the tree should not have changed");
}

/// The root is never nested into anything.
#[test]
fn dropping_onto_the_root_is_refused() {
    let mut p = Project::default();
    p.new_document();
    let (_asm, [a, b, _c]) = asm_with_three(&mut p);
    let root = p.root;

    assert!(p.group_components_into_assembly(&[a, b], root, "Group").is_none(), "the root cannot be moved into a subassembly");
}

// ── what may be dropped at all ───────────────────────────────────────────────────────────────────
//
// The tree needs this rule before the drop: while a target is not allowed it draws neither an insertion line
// nor a highlight, so the restriction is visible rather than discovered after releasing the button. The rule
// lives next to the operations themselves; otherwise the highlight and the action drift apart, and that goes
// unnoticed.

#[test]
fn dropping_into_your_own_descendant_is_not_allowed() {
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_assembly("Outer");
    p.set_active_component(Some(outer));
    let inner = p.add_assembly("Inner");

    assert!(!p.tree_drop_allowed(&[outer], inner, true), "an ancestor into its own descendant would be a cycle");
    assert!(p.tree_drop_allowed(&[inner], outer, true), "while a descendant back into its ancestor is allowed");
}

#[test]
fn dropping_onto_yourself_or_the_root_is_not_allowed() {
    let mut p = Project::default();
    p.new_document();
    let (_asm, [a, b, _c]) = asm_with_three(&mut p);
    let root = p.root;

    assert!(!p.tree_drop_allowed(&[a], a, true), "dropping onto oneself does nothing");
    assert!(!p.tree_drop_allowed(&[a], root, true), "the root is not a target");
    assert!(!p.tree_drop_allowed(&[], b, true), "an empty selection has nothing to drop");
    assert!(p.tree_drop_allowed(&[a], b, true), "while a sibling is allowed");
}

#[test]
fn reordering_is_allowed_only_among_siblings() {
    let mut p = Project::default();
    p.new_document();
    let (_asm1, [a, b, _c]) = asm_with_three(&mut p);
    p.set_active_component(Some(p.root));
    let asm2 = p.add_assembly("Second unit");
    p.set_active_component(Some(asm2));
    let x = p.add_part("X");

    assert!(p.tree_drop_allowed(&[a], b, false), "siblings may be reordered");
    assert!(!p.tree_drop_allowed(&[a], x, false), "across a parent boundary it is a move rather than a reorder");
}
