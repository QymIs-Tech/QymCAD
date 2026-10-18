//! THE NAMES HOLD ON REAL DOCUMENTS, the measure of every change: on each example and private sample two rebuilds in a
//! row give the same names, and one edit - the first number a person would type - loses no reference, finds none by
//! place and renames nothing that did not move. Every edit of every document is the release matrix
//! (`matrix_names_on_real_documents`). Live OCCT, headless; the faults of all the documents are told together.
use qymcad_testkit::real_documents;

#[test]
fn two_rebuilds_in_a_row_give_every_document_the_same_names() {
    let fails: Vec<String> = real_documents::documents().iter().flat_map(real_documents::determinism).collect();
    assert!(fails.is_empty(), "{} faults:\n  {}", fails.len(), fails.join("\n  "));
}

#[test]
fn the_first_number_of_every_document_edited_keeps_every_name() {
    let mut fails = Vec::new();
    for doc in real_documents::documents() {
        let mut o = real_documents::open(&doc);
        let Some(e) = real_documents::edits(&o.project).into_iter().next() else {
            eprintln!("{}: no number to edit", doc.name);
            continue;
        };
        fails.extend(real_documents::edit_and_back(&mut o, &e, 1.1).into_iter().map(|f| format!("{}: {f}", doc.name)));
    }
    assert!(fails.is_empty(), "{} faults:\n  {}", fails.len(), fails.join("\n  "));
}
