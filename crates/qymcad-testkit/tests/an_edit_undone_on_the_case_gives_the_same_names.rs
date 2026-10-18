//! AN EDIT UNDONE GIVES THE SAME NAMES, EVERY TIME. The case of the examples: the height of node 11901 edited 6 -> 5.4
//! and undone, thirty times over, each time the names of every face, edge and end of an edge as the document opened -
//! the second principle of the names: the same document gives the same names. Measured: once in seven runs of the
//! matrix of the real documents this edit left the chamfer 11963 built a hair short, two edges more and the ends of
//! eight renamed, with every node after it; 0 of 240 repeats of the edit alone. A run of the release level (about
//! 190 s debug); the trap of the matrix writes what repeats a rename (`real_documents::trap`).
use qymcad_testkit::real_documents;

/// How many times the edit is made and undone.
const ROUNDS: usize = 30;

#[test]
fn the_height_of_11901_edited_and_undone_thirty_times_renames_nothing() {
    let Some(doc) = real_documents::documents().into_iter().find(|d| d.name.starts_with("QymBoxPc-Case")) else {
        panic!("the case of the examples is not among the documents");
    };
    let mut opened = real_documents::open(&doc);
    let edit = real_documents::edits(&opened.project).into_iter().find(|e| matches!(e, real_documents::Edit::Feature { node: 11901, .. })).expect("the height of node 11901");
    for round in 1..=ROUNDS {
        let faults = real_documents::edit_and_back(&mut opened, &edit, 0.9);
        assert!(faults.is_empty(), "round {round} of {ROUNDS}: {} faults, the first: {}", faults.len(), faults[0]);
    }
}
