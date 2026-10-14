//! AN OLD DOCUMENT'S FORMULA WHOSE VALUE THE NEW READING CHANGES IS NAMED ON OPENING, AND APPLIED. Reported behaviour
//! (issue #220): `-w^2` was read as `(-w)^2`; it is now `-(w^2)`. A document written before holds the old value; on
//! opening it the new value is applied - the document shows it, rather than taking it on some later edit - the
//! window names the formula with its value before and now once the rebuild that applies it has ended, the document
//! counts as changed from the file, and Ctrl+Z does not bring the old number back.
use super::super::hand::Hand;
use super::super::App;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Param, Project};

/// A document as one written before the change lies on disk: a part, and two parameters, `w = 3` and `h = -w^2`
/// holding 9 - the value the old reading gave.
fn saved_old_document(name: &str) -> String {
    let dir = std::env::temp_dir().join("qym_old_formula_named_test");
    std::fs::create_dir_all(&dir).expect("the directory for the check");
    let path = dir.join(name).to_string_lossy().into_owned();
    let mut project = Project::default();
    let sid = project.add_line_sketch("s", vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    project.add_sketch_node(sid, "Sketch");
    let b = project.add_extrude(sid, 5.0);
    project.finish_base_body(b, 1);
    project.parameters.push(Param { name: "w".into(), expr: "3".into(), value: 3.0 });
    project.parameters.push(Param { name: "h".into(), expr: "-w^2".into(), value: 9.0 });
    qymcad_io::save_project(&project, &path).expect("the old document is saved");
    path
}

#[test]
fn an_old_formula_is_applied_and_named_when_opened() {
    let path = saved_old_document("old_formula.qcad");
    let mut app = App::default();
    // File, Open: the chooser answers with the path, and the load runs in the background as in the window
    let (tx, rx) = std::sync::mpsc::channel();
    app.arm_file_ask(rx, |app, p| crate::gui::io_jobs::spawn_project_load(&mut app.regen, p.to_string_lossy().into_owned()));
    tx.send(Some(std::path::PathBuf::from(&path))).expect("the chooser's channel is open");
    let mut hand = Hand::new(&mut app);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("place", "h");
    args.set("formula", "-w^2");
    args.set("was", "9");
    args.set("now", "-9");
    let item = crate::i18n::tr_args("io-sign-rule-item", Some(&args));
    assert_ne!(item, "io-sign-rule-item", "setup: the catalogue has no words for a changed formula");
    // the hand waits as a person waits for the spinner: until the window says it, the rebuild that applies the new
    // reading having ended
    crate::gui::session::waiting::until(&format!("the window to name the changed formula {item:?}"), || {
        hand.frame(Vec::new());
        hand.words_drawn().iter().any(|(t, _)| t.contains(&item))
    });
    let h = |hand: &Hand| hand.app.project.parameters.iter().find(|p| p.name == "h").map(|p| p.value);
    assert_eq!(h(&hand), Some(-9.0), "the new reading is not applied to the opened document");
    assert!(qymcad_ui_state::is_dirty(&mut hand.app.rebuild_ctx()), "the document does not count as changed from the file");
    hand.undo();
    assert_eq!(h(&hand), Some(-9.0), "Ctrl+Z brought the old number back - applying the new reading made a step of undo");
    let _ = std::fs::remove_file(&path);
}

/// THE HISTORY STARTS FROM THE NEW VALUES, whenever the first edit comes. Opening applies the values the new reading
/// gives, and the state an undo comes back to is taken from them, not from the file's, so an edit made before the
/// rebuild that follows has landed does not bring `h = 9` back when it is undone. Through the window the landed
/// rebuild takes that state again, which hides the difference there; the function itself shows it.
#[test]
fn the_history_starts_from_the_new_values() {
    let mut project = Project::default();
    project.new_document();
    project.parameters.push(Param { name: "w".into(), expr: "3".into(), value: 3.0 });
    project.parameters.push(Param { name: "h".into(), expr: "-w^2".into(), value: 9.0 });
    let mut regen = qymcad_ui_state::Rebuilding::default();
    let mut edits = qymcad_ui_state::Edits::default();
    let _ = crate::gui::io_jobs::loaded_status(&None, &mut project, &mut regen, &mut edits);
    let h = |p: &Project| p.parameters.iter().find(|q| q.name == "h").map(|q| q.value);
    assert_eq!(h(&project), Some(-9.0), "setup: the new reading is not applied");
    assert_eq!(h(&edits.baseline.project), Some(-9.0), "the state an undo comes back to holds the file's old value");
}
