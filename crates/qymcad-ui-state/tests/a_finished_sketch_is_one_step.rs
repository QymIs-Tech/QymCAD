//! THE HISTORY OF A SKETCH: while a sketch is open undo stops at its first step, and closed, its steps are one step of
//! the document.
//!
//! Reported behaviour: the history is one for the whole document, and Ctrl+Z inside a sketch went on past the moment
//! it was opened and took back the steps of another part.
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_ui_state::{begin_edit, close_edit, close_sketch_history, open_sketch_history, undo_floor, Edits, Snapshot, Step};

/// One step of the document: a sketch of one line added inside a named edit.
fn a_step(edits: &mut Edits, p: &mut Project, name: &str) {
    begin_edit(edits, p, name);
    p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false);
    close_edit(edits, p);
}

fn names(edits: &Edits) -> Vec<String> {
    edits.undo.iter().map(|s| s.name.clone()).collect()
}

#[test]
fn an_open_sketch_stops_undo_at_its_first_step_and_closed_is_one_step() {
    let mut p = Project::default();
    p.new_document();
    let mut edits = Edits::default();
    a_step(&mut edits, &mut p, "Extrusion");
    let sid = p.add_line_sketch("Sketch 2", vec![Point2::new(0.0, 0.0), Point2::new(5.0, 0.0)], false);
    let opened = p.clone();
    open_sketch_history(&mut edits, sid);
    assert_eq!(undo_floor(&edits), 1, "the sketch opened over one step of the document");
    for name in ["Line 1", "Line 2", "Line 3"] {
        a_step(&mut edits, &mut p, name);
    }
    assert_eq!(edits.undo.len(), 4, "three steps of the sketch over the one of the document: {:?}", names(&edits));
    close_sketch_history(&mut edits, &p);
    assert_eq!(edits.undo.len(), 2, "closed, the sketch folds into one step: {:?}", names(&edits));
    assert_eq!(edits.undo[1].name, qymcad_i18n::tr1("g-sketch-edited", "name", "Sketch 2"), "the step is named after the sketch");
    assert_eq!(qymcad_ui_state::doc_key(&edits.undo[1].snap.project), qymcad_ui_state::doc_key(&opened), "the folded step takes the document back to where the sketch was opened");
    assert_eq!(undo_floor(&edits), 0, "out of the sketch undo goes all the way");
}

#[test]
fn a_sketch_made_inside_an_operation_keeps_its_making_out_of_reach_and_folds_it_in() {
    let mut p = Project::default();
    p.new_document();
    let mut edits = Edits::default();
    a_step(&mut edits, &mut p, "Extrusion");
    begin_edit(&mut edits, &p, "New sketch");
    let sid = p.add_line_sketch("Sketch 2", vec![Point2::new(0.0, 0.0), Point2::new(5.0, 0.0)], false);
    open_sketch_history(&mut edits, sid);
    assert_eq!(undo_floor(&edits), usize::MAX, "while the operation making the sketch is open nothing is undone from inside");
    close_edit(&mut edits, &p);
    assert_eq!(undo_floor(&edits), 2, "the step making the sketch lies under the floor: {:?}", names(&edits));
    a_step(&mut edits, &mut p, "Line 1");
    close_sketch_history(&mut edits, &p);
    assert_eq!(names(&edits), vec!["Extrusion".to_string(), qymcad_i18n::tr1("g-sketch-edited", "name", "Sketch 2")], "the making and the line are one step");
}

#[test]
fn a_sketch_left_as_it_was_gives_the_document_its_redo_back() {
    let mut p = Project::default();
    p.new_document();
    let mut edits = Edits::default();
    edits.redo.push(Step { name: "Extrusion".into(), snap: Snapshot { project: p.clone() } });
    let sid = p.add_line_sketch("Sketch 1", vec![Point2::new(0.0, 0.0), Point2::new(5.0, 0.0)], false);
    open_sketch_history(&mut edits, sid);
    assert!(edits.redo.is_empty(), "inside the sketch redo walks the sketch's steps only");
    close_sketch_history(&mut edits, &p);
    assert_eq!(edits.redo.len(), 1, "a sketch that laid no step leaves the document's redo as it was");
}
