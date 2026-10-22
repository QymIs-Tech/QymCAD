//! A SKETCH SELECTED BEFORE A SHIFT CLICK IS HELD ONLY WITH AN EDGE: a sweep takes the profile and the edge of its path
//! picked together before it, and nothing else changes - a sketch selected and a face picked with Shift holds nothing,
//! as before, so the rows a group delete takes and the bodies a boolean of the tree takes are what they were.

use qymcad_core::model::{Id, Project};
use qymcad_ui_state::{hold_bodies, Adding, Chosen, Sel};

/// A Shift click with nothing in hand: what was selected before it, the body under the pointer, and the selection the
/// click made.
struct Click {
    was: Sel,
    under: Id,
    made: Sel,
}

fn held_after(p: &Project, click: Click) -> Vec<Id> {
    let mut chosen = Chosen { sel: click.made, ..Default::default() };
    hold_bodies(p, click.was, Some(click.under), &mut chosen, Adding::Yes);
    chosen.tree_sel.multi
}

#[test]
fn a_sketch_is_held_with_an_edge_only() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("profile");
    let sketch = p.sketches[si].id;
    let mut fails = Vec::new();
    let held = held_after(&p, Click { was: Sel::Sketch(si), under: 7, made: Sel::Face(0, 0) });
    if !held.is_empty() {
        fails.push(format!("a sketch selected and a face picked with Shift hold {held:?}, and held nothing before"));
    }
    let held = held_after(&p, Click { was: Sel::Sketch(si), under: 7, made: Sel::Edge(7, 3) });
    if held != vec![sketch] {
        fails.push(format!("a sketch selected and an edge picked with Shift - the profile and the path of a sweep - hold {held:?}, not the sketch {sketch}"));
    }
    let held = held_after(&p, Click { was: Sel::Edge(10, 1), under: 20, made: Sel::Edge(20, 2) });
    if held != vec![10, 20] {
        fails.push(format!("an edge of one body and then of another picked with Shift hold {held:?}, not both bodies, as a boolean of the tree takes them"));
    }
    assert!(fails.is_empty(), "\n{}", fails.join("\n"));
}
