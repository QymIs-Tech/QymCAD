//! THE HISTORY OF A SKETCH: while a sketch is open, undo and redo walk its own steps; closed, the sketch is one step of
//! the document. The history of the document is one stack of snapshots of the whole project, and without a floor Ctrl+Z
//! in a sketch went on past the moment it was opened and took back the steps of another part. Reported behaviour: a
//! new sketch drawn, Ctrl+Z pressed once more than the lines - the extrusion of the part before it was gone.

use super::*;

/// A SKETCH OPENED: its history starts at the top of the document's. Opened inside an operation (a new sketch is made
/// and opened in one step), the floor waits for that operation to close - the step making the sketch is not undone
/// from inside it. Inside the sketch redo walks its own steps: those of the document wait aside.
pub fn open_sketch_history(edits: &mut Edits, sketch: Id) {
    let at = edits.undo.len();
    let floor = edits.open.is_none().then_some(at);
    let outer_redo = std::mem::take(&mut edits.redo);
    edits.sketch = Some(SketchHistory { sketch, fold_from: at, floor, outer_redo });
}

/// THE OPERATION THAT OPENED THE SKETCH HAS CLOSED: the floor stands over the step it left. A step left means the
/// document changed, and the steps to redo it held are gone with that.
pub(crate) fn settle_sketch_floor(edits: &mut Edits) {
    let depth = edits.undo.len();
    if let Some(h) = edits.sketch.as_mut().filter(|h| h.floor.is_none()) {
        h.floor = Some(depth);
        if depth > h.fold_from {
            h.outer_redo.clear();
        }
    }
}

/// THE OLDEST STEP OF THE HISTORY DROPPED for the cap on its length: the marks of the sketch move down with it.
pub(crate) fn shift_sketch_history(edits: &mut Edits) {
    if let Some(h) = edits.sketch.as_mut() {
        h.fold_from = h.fold_from.saturating_sub(1);
        h.floor = h.floor.map(|f| f.saturating_sub(1));
    }
}

/// HOW DEEP UNDO MAY GO: the floor of the sketch open for editing, zero out of one.
pub fn undo_floor(edits: &Edits) -> usize {
    edits.sketch.as_ref().map_or(0, |h| h.floor.unwrap_or(usize::MAX))
}

/// A HISTORY LEFT FROM A SKETCH NO LONGER OPEN (deleted from under its session, or gone with a document reloaded) is
/// closed as a finished sketch is, so it stops no undo.
pub fn drop_stale_sketch_history(edits: &mut Edits, project: &Project, ses: &SketchSession) {
    if edits.sketch.as_ref().is_some_and(|h| ses.editing != Some(h.sketch)) {
        close_sketch_history(edits, project);
    }
}

/// THE SKETCH CLOSED: the steps of its session fold into one step of the document named after the sketch, so one undo
/// takes the session back and one redo brings it whole. The inner snapshots are dropped, the project is not copied:
/// the first step of the session already holds the state before it, and the baseline the state after it. A session
/// that left the document as it found it gives the document its steps to redo back.
pub fn close_sketch_history(edits: &mut Edits, project: &Project) {
    let Some(h) = edits.sketch.take() else { return };
    let depth = edits.undo.len();
    if depth <= h.fold_from {
        edits.redo = h.outer_redo;
        return;
    }
    edits.redo.clear(); // the steps undone in the sketch belong to the session
    if depth > h.floor.unwrap_or(depth) {
        edits.undo.truncate(h.fold_from + 1);
        if let Some(s) = project.sketches.iter().find(|s| s.id == h.sketch) {
            edits.undo[h.fold_from].name = qymcad_i18n::tr1("g-sketch-edited", "name", &qymcad_i18n::name(&s.name));
        }
    }
}
