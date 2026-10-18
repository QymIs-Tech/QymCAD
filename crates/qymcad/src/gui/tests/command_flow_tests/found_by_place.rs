//! A REFERENCE FOUND BY PLACE IS SAID AND ACCEPTED IN THE WINDOW: a document whose rounding found its edge by place, not
//! by name, opens with the row of the rounding yellow and its properties saying so; Accept takes the mark away as one
//! step Ctrl+Z gives back.

use super::*;
use crate::gui::hand::Hand;

/// A block with a rounding of R2 on an upright edge whose name was lost and found by place, rebuilt and written to
/// `path`. Returns the node of the rounding.
fn written_with_an_edge_found_by_place(path: &str) -> Id {
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::names::{EdgeName, GeoName, Role};
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let block = p.add_box(40.0, 30.0, 10.0);
    let kernel = qymcad_kernel::OcctKernel::default();
    let gate = qymcad_kernel::kernel_gate();
    let _ = p.regenerate(&kernel);
    let edge = p.regen_edges[&block].iter().find(|e| (e.a[2] - e.b[2]).abs() > 5.0).cloned().expect("an upright edge of the block");
    let fillet = p.add_fillet(block, 2.0, vec![edge.id]);
    let _ = p.regenerate(&kernel);
    let nowhere = [1, 2].map(|k| p.names.intern_face(GeoName::new(999, Role::Wall, k)));
    let lost = p.names.intern_edge(EdgeName::new(nowhere[0], nowhere[1], 0));
    for n in p.timeline.iter_mut().filter(|n| n.id == fillet) {
        if let FeatureKind::Fillet { edges, .. } = &mut n.kind {
            *edges = qymcad_core::refs::Ref::picks(&[lost]);
        }
        n.dirty = true;
    }
    p.edge_refs.insert(fillet, vec![qymcad_core::model::ElemSnapshot { id: lost, at: edge.mid, dir: edge.dir }]);
    let _ = p.regenerate(&kernel);
    drop(gate);
    assert!(p.placed_refs.contains(&fillet), "GUARD: the rounding found its edge by place");
    qymcad_io::save_project(&p, path).expect("the document is written");
    fillet
}

#[test]
fn a_rounding_whose_edge_was_found_by_place_says_so_and_is_accepted() {
    let dir = std::env::temp_dir().join(format!("qym_found_by_place_window_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder for the file");
    let path = dir.join("found.qcad").to_string_lossy().into_owned();
    let fillet = written_with_an_edge_found_by_place(&path);
    let loaded = qymcad_io::load_project(&path).expect("the document reads");
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = App::default();
    app.finish_project_load(path.clone(), loaded, Vec::new());
    let said = crate::gui::error_words::error_text(&qymcad_core::errors::CoreError::RefFoundByPlace);
    let accept = crate::i18n::tr("props-accept-place");
    let mut hand = Hand::new(&mut app);
    // into the part, as a person goes: a double click on its row in the tree
    let part = hand.app.project.components.iter().find(|c| c.kind == qymcad_core::feature::ComponentKind::Part).map(|c| crate::i18n::name(&c.name)).expect("the part");
    assert!(hand.double_click_word(&part), "the part {part:?} is not in the tree");
    // the row of the rounding, as the tree writes it (the words of the window are read without their signs)
    let name = crate::i18n::tr("feat-name-fillet");
    hand.frame(Vec::new());
    let row = hand.words_drawn().iter().find(|(w, _)| w.starts_with(&name)).map(|(_, r)| r.center());
    let row = row.unwrap_or_else(|| panic!("no row of the rounding; the window writes {:?}", hand.words_drawn().iter().map(|(w, _)| w.clone()).collect::<Vec<_>>()));
    hand.click_screen(row);
    assert!(hand.shows(&said), "the properties of the rounding do not say its edge was found by place");
    assert!(hand.press_word(&accept, row), "no Accept in the properties of the rounding");
    assert!(!hand.shows(&said) && !hand.app.project.placed_refs.contains(&fillet), "accepted, the rounding is still marked");
    hand.undo();
    assert!(hand.app.project.placed_refs.contains(&fillet), "Ctrl+Z did not bring the mark back");
}

/// A ROUNDING BUILT A HAIR SHORT SAYS SO IN ITS PROPERTIES: four roundings of R5 on the top of a cube of 10 meet at its
/// corners and are built a hair short of their size; rebuilt in the window, the properties of the rounding say so in the
/// warning tone, as its row does. Before, the row stood yellow and its properties said nothing of it.
#[test]
fn a_rounding_built_a_hair_short_says_so_in_its_properties() {
    let dir = std::env::temp_dir().join(format!("qym_hair_short_window_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder for the file");
    let path = dir.join("short.qcad").to_string_lossy().into_owned();
    {
        let mut p = qymcad_core::model::Project::default();
        p.new_document();
        let cube = p.add_box(10.0, 10.0, 10.0);
        let kernel = qymcad_kernel::OcctKernel::default();
        let gate = qymcad_kernel::kernel_gate();
        let _ = p.regenerate(&kernel);
        let top: Vec<u32> = p.regen_edges[&cube].iter().filter(|e| (e.a[2] - 10.0).abs() < 1e-9 && (e.b[2] - 10.0).abs() < 1e-9).map(|e| e.id).collect();
        let _ = p.add_fillet(cube, 5.0, top);
        let _ = p.regenerate(&kernel);
        drop(gate);
        qymcad_io::save_project(&p, &path).expect("the document is written");
    }
    let loaded = qymcad_io::load_project(&path).expect("the document reads");
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = App::default();
    app.finish_project_load(path.clone(), loaded, Vec::new());
    let said = crate::gui::error_words::error_text(&qymcad_core::errors::CoreError::BuiltSteppedBack);
    let mut hand = Hand::new(&mut app);
    let part = hand.app.project.components.iter().find(|c| c.kind == qymcad_core::feature::ComponentKind::Part).map(|c| crate::i18n::name(&c.name)).expect("the part");
    assert!(hand.double_click_word(&part), "the part {part:?} is not in the tree");
    // Edit -> Rebuild everything, as a person asks for it
    hand.frame(Vec::new());
    let edit = hand.written_at(&crate::i18n::tr("menu-edit")).map(|r| r.center()).expect("the Edit menu");
    hand.click_screen(edit);
    assert!(hand.press_word(&crate::i18n::tr("menu-rebuild"), edit), "no Rebuild everything in the Edit menu");
    hand.settle();
    let name = crate::i18n::tr("feat-name-fillet");
    hand.frame(Vec::new());
    let row = hand.words_drawn().iter().find(|(w, _)| w.starts_with(&name)).map(|(_, r)| r.center());
    let row = row.unwrap_or_else(|| panic!("no row of the rounding; the window writes {:?}", hand.words_drawn().iter().map(|(w, _)| w.clone()).collect::<Vec<_>>()));
    hand.click_screen(row);
    assert!(hand.shows(&said), "the properties of the rounding do not say it was built a hair short");
}
