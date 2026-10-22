//! A REFERENCE FOUND AS THE OTHER PIECE OF ITS FACE IS SHOWN IN YELLOW, as one found by place is. Measured on the filter
//! of the examples: a dimension of sketch 53 typed 24.2 in place of 22 leaves the upper halves of two walls beside no face
//! of their node, the edges a rounding picked on them are found as the other pieces of the walls, and the window said
//! nothing - the rounding stood as if its references had been found by their names. The person is to see that a
//! fallback took them, and accept or pick again.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::model::{Constraint, Project};

/// The sketch the dimension is in, and the points it is laid on.
const SKETCH: u64 = 53;
const DIMENSION: [u64; 2] = [163, 164];
/// The rounding standing on the halves of the walls.
const ROUNDING: u64 = 269;

#[test]
fn the_rounding_whose_edges_went_to_the_other_pieces_is_yellow() {
    let mut app = App::default();
    crate::gui::io_jobs::spawn_project_load(&mut app.regen, format!("{}/../../examples/Filter-v2.qcad", env!("CARGO_MANIFEST_DIR")));
    app.drain_busy_for_test();
    app.wait_bg();
    let si = app.project.sketch_index(SKETCH).expect("setup: the sketch of the dimension");
    let ci = app.project.sketches[si].constraints.iter().position(|c| Project::dim_refs(c).is_some_and(|r| r.len() == 2 && r.iter().all(|p| DIMENSION.contains(p)))).expect("setup: the dimension");
    let value = app.project.sketches[si].constraints[ci].dim_value().expect("setup: the value of the dimension");
    assert!((value - 22.0).abs() < 1e-9, "setup: the dimension stands at {value}, not 22");
    assert!(matches!(app.project.sketches[si].constraints[ci], Constraint::Distance { .. }), "setup: the dimension is a length");
    // the components from the top of the tree down to the part of the sketch: each stepped into by a double click
    let owner = app.project.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Sketch { sketch } if sketch == SKETCH)).and_then(|n| n.parent);
    let mut chain: Vec<String> = Vec::new();
    let mut at = owner;
    while let Some(c) = at.and_then(|id| app.project.components.iter().find(|c| c.id == id)) {
        if c.parent.is_some() {
            chain.push(crate::i18n::name(&c.name));
        }
        at = c.parent;
    }
    chain.reverse();
    let row = crate::i18n::name(&app.project.sketches[si].name);
    let mut h = Hand::new(&mut app);
    h.settle();
    assert!(!h.app.project.regen_warnings.contains_key(&ROUNDING), "setup: the rounding is yellow as the filter opens: {:?}", h.app.project.regen_warnings.get(&ROUNDING));
    for name in &chain {
        assert!(h.double_click_word(name), "the tree shows no component {name:?} on the way to the sketch");
    }
    assert!(h.double_click_word(&row), "the tree shows no sketch {row:?}");
    assert!(h.app.sketch_ses.editing.is_some(), "setup: a double click on the sketch {row:?} does not open it");
    h.frame(Vec::new());
    let rect = h.app.viewing.view_rect;
    let label = crate::gui::sketching::dim_label_pos(&mut h.app.sketch_ctx(), rect, si, ci).expect("the label of the dimension is laid out");
    h.double_click_screen(label);
    h.frame(Vec::new());
    h.type_text("24.2").key(egui::Key::Enter);
    assert!(h.app.project.sketches[si].constraints[ci].dim_value().is_some_and(|d| (d - 24.2).abs() < 1e-9), "the dimension typed 24.2 at its label did not take it");
    let finish = crate::i18n::tr("wb-finish");
    let button = h.written_at(&finish).unwrap_or_else(|| panic!("no {finish:?} on screen")).center();
    h.click_screen(button);
    h.settle();
    assert!(!h.app.project.regen_errors.contains_key(&ROUNDING), "the rounding refused after the edit: {:?}", h.app.project.regen_errors.get(&ROUNDING));
    let pal = h.app.scheme.pal.clone();
    let words = crate::i18n::tr("warn-ref-found-by-place");
    let said = h.status_painted().into_iter().find(|w| w.text.contains(&words));
    assert!(
        said.as_ref().is_some_and(|w| w.colour == pal.warning()),
        "the status line does not say in yellow that a reference of the rounding was not found by its name: {:?}",
        said.map(|w| (w.text, w.colour))
    );
}
