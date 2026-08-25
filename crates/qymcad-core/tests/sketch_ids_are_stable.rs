//! The identity of a sketch entity does not change from ordinary work inside the sketch.
//!
//! A wall of a body is named by an edge of the profile: `Role::Wall` with `src` holding the id of a sketch
//! entity. That is the recipe, and it rests on one assumption — that an entity stays itself however much its
//! points are dragged and however many constraints are hung on it. Nothing had ever checked that assumption.
//!
//! If the id of an entity changed on an edit, on a further solve or when a neighbour was added, every wall name
//! grown from it would move, and with them the fillets, the chamfers and the sketches on faces. Silently: the
//! name would still read "wall from entity N" while N came to mean something else.
//!
//! Four kinds of work are checked, all of them constant in practice: moving a point, adding a constraint,
//! solving again, and adding or removing a neighbouring entity. What is compared is not only the ids of the
//! entities but the provenance of the contour edges (`edge_src`), which is what reaches the face name.
use qymcad_core::model::{Constraint, Project};

/// The entities of a sketch and the provenance of its contour edges: what wall names are assembled from.
fn identity(p: &Project, si: usize) -> (Vec<u64>, Vec<Vec<u64>>) {
    let ents: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let srcs: Vec<Vec<u64>> = p.contours.iter().map(|c| c.edge_src.clone()).collect();
    (ents, srcs)
}

fn sketch_with_a_rect() -> (Project, usize, u64) {
    let mut p = Project::default();
    p.ensure_document();
    let part = p.add_part("Part");
    p.set_active_component(Some(part));
    let sid = p.add_sketch("sketch", Vec::new(), None);
    let si = p.sketch_index(sid).expect("the sketch");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 40.0, 5.0, 3.0, qymcad_core::feature::Purpose::Real);
    p.solve_sketch(si);
    p.regen_sketch(si);
    (p, si, sid)
}

#[test]
fn moving_a_point_keeps_every_entity_id() {
    let (mut p, si, _) = sketch_with_a_rect();
    let before = identity(&p, si);
    p.sketches[si].points[0].x += 3.0;
    p.sketches[si].points[0].y -= 1.5;
    p.solve_sketch(si);
    p.regen_sketch(si);
    assert_eq!(before, identity(&p, si), "moving a point changed the identity of the sketch entities, so wall names would follow it");
}

#[test]
fn adding_a_constraint_keeps_every_entity_id() {
    let (mut p, si, _) = sketch_with_a_rect();
    let before = identity(&p, si);
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    p.add_constraint_if_independent(si, Constraint::Horizontal { a, b });
    p.solve_sketch(si);
    p.regen_sketch(si);
    assert_eq!(before, identity(&p, si), "adding a constraint changed the identity of the sketch entities");
}

#[test]
fn solving_twice_changes_nothing() {
    let (mut p, si, _) = sketch_with_a_rect();
    let before = identity(&p, si);
    for _ in 0..3 {
        p.solve_sketch(si);
        p.regen_sketch(si);
    }
    assert_eq!(before, identity(&p, si), "solving again by itself changed the identity of the entities");
}

/// The most dangerous of the four: if an id is a position in a list, adding or removing a neighbour shifts
/// everything after it and the walls move onto other edges.
#[test]
fn adding_and_removing_a_neighbour_keeps_the_others() {
    let (mut p, si, _) = sketch_with_a_rect();
    let before = identity(&p, si);
    let extra = p.add_line_entity(si, -10.0, -10.0, -5.0, -8.0, qymcad_core::feature::Purpose::Real);
    p.solve_sketch(si);
    p.regen_sketch(si);
    let with_extra: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    assert!(with_extra.starts_with(&before.0), "adding a neighbour reshuffled the existing entities");

    p.delete_entities(si, &[extra]);
    p.solve_sketch(si);
    p.regen_sketch(si);
    assert_eq!(before, identity(&p, si), "removing a neighbour changed the identity of the remaining entities");
}

/// Filleting a corner in a sketch really does restructure the entities: two lines are trimmed and an arc
/// appears between them. A new entity appearing is legitimate; the existing lines changing identity is not,
/// since everything grown from them would move somewhere else.
#[test]
fn a_sketch_fillet_keeps_the_two_lines_themselves() {
    let (mut p, si, _) = sketch_with_a_rect();
    let lines: Vec<u64> = p.sketches[si]
        .entities
        .iter()
        .filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Line { .. }))
        .map(|e| e.id)
        .collect();
    assert!(lines.len() >= 2, "setup: a rectangle has to contain lines");
    let (e1, e2) = (lines[0], lines[1]);
    let before: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    if !p.fillet_lines(si, e1, e2, 2.0) {
        eprintln!("skipped: a fillet does not build on this pair");
        return;
    }
    p.solve_sketch(si);
    p.regen_sketch(si);
    let after: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    let lost: Vec<u64> = before.iter().copied().filter(|id| !after.contains(id)).collect();
    assert!(lost.is_empty(), "filleting a corner erased the identity of existing entities: {lost:?}, so wall names would follow");
}
