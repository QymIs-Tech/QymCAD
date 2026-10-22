//! THE MATRIX OF NAMES ON REAL DOCUMENTS - the release check of the names: every example and private sample opened as
//! the window opens it, every number a person edits in it (`real_documents::edits`) changed and undone, the direction
//! alternating from one edit to the next (+10 %, -10 %, ...). After each: no reference lost or found by place, no face
//! that did not move renamed, every name back as it opened; a refusal is a fault unless the list of honest refusals
//! holds it with its reason. The time of each document is printed. Live OCCT, headless; the faults are told together.
//!
//! Measured, alternating, of work on the counters of the system (debug build): the case 490 s, the filter 232 s, the vacuum
//! cleaner 24 s, the rest 18 s together - 790 s on the wall. It runs at the release level only (`tools/gate.py`,
//! `RELEASE_ONLY`); the fast probe of the same is `names_hold_on_real_documents`.
use qymcad_testkit::real_documents;

/// The change of an edit by its place in the list: up and down by turns.
fn factor(place: usize) -> f64 {
    if place.is_multiple_of(2) {
        1.1
    } else {
        0.9
    }
}

#[test]
fn every_edit_of_every_document_keeps_every_name() {
    let mut fails: Vec<String> = Vec::new();
    for doc in real_documents::documents() {
        let walked = qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Process, || {
            let mut o = real_documents::open(&doc);
            let edits = real_documents::edits(&o.project);
            for (place, e) in edits.iter().enumerate() {
                fails.extend(real_documents::edit_and_back(&mut o, e, factor(place)).into_iter().map(|f| format!("{}: {f}", doc.name)));
            }
            edits.len()
        });
        eprintln!("MATRIX {}: {} edits in {:.0} s of work", doc.name, walked.out, walked.cost.work.as_secs_f64());
    }
    assert!(fails.is_empty(), "{} faults:\n  {}", fails.len(), fails.join("\n  "));
}
