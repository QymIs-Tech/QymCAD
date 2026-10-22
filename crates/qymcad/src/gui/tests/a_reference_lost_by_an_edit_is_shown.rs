//! A REFERENCE AN EDIT TOOK FROM ITS NAME IS SHOWN IN YELLOW, on a part a person made: the private sample of a vacuum
//! cleaner, a dimension of sketch 191 typed 5.5 in place of 5. The edit makes another pair of faces of the two an edge
//! of the rounding 252 stood between; its name finds nothing, the edge standing exactly where it stood is taken, and the
//! person is to see that a fallback took it, and accept or pick again. The check of the same with the name spoiled by
//! hand is `found_by_place`; this one is a loss an edit made, by the person's way.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::errors::CoreError;
use qymcad_core::model::Project;

/// The sketch the dimension is in, and the points it is laid on.
const SKETCH: u64 = 191;
const DIMENSION: [u64; 2] = [243, 244];
/// The rounding whose edge the edit took from its name.
const ROUNDING: u64 = 252;

#[test]
fn the_rounding_whose_edge_an_edit_took_from_its_name_is_yellow() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/vacuumCleaner.qcad");
    if !std::path::Path::new(path).exists() {
        eprintln!("PASSED OVER: the private sample vacuumCleaner.qcad is not in this tree - the check on it runs only where the samples are");
        return;
    }
    let mut app = App::default();
    crate::gui::io_jobs::spawn_project_load(&mut app.regen, path.to_string());
    app.drain_busy_for_test();
    app.wait_bg();
    let si = app.project.sketch_index(SKETCH).expect("setup: the sketch of the dimension");
    let ci = app.project.sketches[si].constraints.iter().position(|c| Project::dim_refs(c).is_some_and(|r| r.len() == 2 && r.iter().all(|p| DIMENSION.contains(p)))).expect("setup: the dimension");
    let value = app.project.sketches[si].constraints[ci].dim_value().expect("setup: the value of the dimension");
    assert!((value - 5.0).abs() < 1e-9, "setup: the dimension stands at {value}, not 5");
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
    assert!(!h.app.project.regen_warnings.contains_key(&ROUNDING), "setup: the rounding is yellow as the sample opens: {:?}", h.app.project.regen_warnings.get(&ROUNDING));
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
    h.type_text("5.5").key(egui::Key::Enter);
    assert!(h.app.project.sketches[si].constraints[ci].dim_value().is_some_and(|d| (d - 5.5).abs() < 1e-9), "the dimension typed 5.5 at its label did not take it");
    let finish = crate::i18n::tr("wb-finish");
    let button = h.written_at(&finish).unwrap_or_else(|| panic!("no {finish:?} on screen")).center();
    h.click_screen(button);
    h.settle();
    assert!(!h.app.project.regen_errors.contains_key(&ROUNDING), "the rounding refused after the edit: {:?}", h.app.project.regen_errors.get(&ROUNDING));
    assert!(
        matches!(h.app.project.regen_warnings.get(&ROUNDING), Some(CoreError::RefFoundByPlace)),
        "the rounding whose edge the edit took from its name says {:?}",
        h.app.project.regen_warnings.get(&ROUNDING)
    );
    // the yellow badge counts it beside the warning the sample opens with, and a click on it lists both - the rounding
    // in yellow words
    let pal = h.app.scheme.pal.clone();
    let badge = h.status_badge(qymcad_ui_state::Tone::Warning).expect("no yellow badge after the edit");
    assert_eq!(
        badge.text,
        format!("{} 2", qymcad_ui_state::tone_icon(qymcad_ui_state::Tone::Warning).unwrap_or_default()),
        "the yellow badge does not count the rounding beside the warning the sample opens with"
    );
    h.click_screen(badge.rect.center());
    assert!(h.window_titled(&crate::i18n::tr("win-faults")).is_some(), "a click on the badge of two warnings opened no window of faults");
    let words = crate::i18n::tr("warn-ref-found-by-place");
    let said = h.painted_words().into_iter().find(|w| w.text.contains(&words));
    assert!(
        said.as_ref().is_some_and(|w| w.colour == pal.warning()),
        "the window of faults does not say in yellow that a reference of the rounding was not found by its name: {:?}",
        said.map(|w| (w.text, w.colour))
    );
}
