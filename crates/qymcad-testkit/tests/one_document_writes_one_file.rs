//! SAVING A DOCUMENT TWICE MUST GIVE THE SAME FILE.
//!
//! It did not. Two saves of one document, with nothing edited in between, produced files differing in
//! 6608 lines - measured on a real project of 5.6 MB. The cause was `HashMap`: it iterates in whatever
//! order its hashing feels like, and that order changes between runs of the SAME binary.
//!
//! For a format people keep in version control this is poison. Every save reads as a rewritten file, a
//! real change drowns in the noise, and two people saving the same untouched document get a conflict out
//! of nothing.
//!
//! The maps that reach the file are ordered now. The derived indices beside them are not written and
//! keep their hashing - their order is nobody's business.

use qymcad_core::model::{from_ron, to_ron, Project};

/// A document with enough in it to have maps worth ordering: named geometry, per-body colour, references.
fn a_document_with_maps() -> Project {
    from_ron(include_str!("doc2.ron")).expect("the sample parses")
}

#[test]
fn two_saves_of_one_document_are_the_same_bytes() {
    let p = a_document_with_maps();
    let first = to_ron(&p).expect("the document serialises");
    let second = to_ron(&p).expect("and again");
    assert_eq!(first.len(), second.len(), "two saves came out different sizes");
    assert!(first == second, "two saves of one document differ - a map is being written in hash order");
}

/// AND IT SURVIVES A ROUND TRIP. Ordering the maps must not change what the document MEANS: read what
/// was written, write it again, and the bytes have to match. Otherwise every open-and-save would rewrite
/// the file, which is the same trouble wearing different clothes.
#[test]
fn a_document_read_back_writes_the_same_bytes() {
    let once = to_ron(&a_document_with_maps()).expect("the document serialises");
    let again = to_ron(&from_ron(&once).expect("it reads back")).expect("and serialises again");
    assert!(once == again, "reading and writing a document changed it");
}
