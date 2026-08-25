//! A SAVED FILE MUST NAME THE BUILD THAT WROTE IT.
//!
//! The format changes with no backward compatibility, so "it does not open" is a question about which
//! build made the file. Without the stamp the answer costs a conversation with whoever reports it, and
//! they rarely remember which of the week's builds they were running.
//!
//! The producer line is set by the application; the library only carries it. Both halves are checked
//! here: that it reaches the file, and that a document saved by a program which never set one stays
//! readable rather than refusing.

use qymcad_core::model::{from_ron, producer, set_producer, to_ron, Project};

#[test]
fn the_build_that_saved_the_file_is_written_into_it() {
    set_producer("QymCAD 9.9.9 (deadbeef1, 2026-08-25)");

    let mut p = Project::default();
    p.meta.title = "a part".into();
    p.meta.version = "rev. B".into(); // the PERSON's version - it must not be overwritten by ours

    let text = to_ron(&p).expect("the document serialises");
    assert!(
        text.contains("QymCAD 9.9.9 (deadbeef1, 2026-08-25)"),
        "the file does not name the build that wrote it:\n{}",
        &text[..text.len().min(400)]
    );

    let back = from_ron(&text).expect("the document reads back");
    assert_eq!(back.meta.saved_by, producer(), "the stamp did not survive the file");
    assert_eq!(back.meta.version, "rev. B", "our stamp overwrote the person's own version");
    assert_eq!(back.meta.title, "a part");
}

#[test]
fn saving_does_not_dirty_the_open_document() {
    set_producer("QymCAD 9.9.9 (deadbeef1, 2026-08-25)");

    // THE STAMP IS PUT ON THE FILE, NOT ON THE DOCUMENT. Were it written into the model, the act of
    // saving would change the document and mark it unsaved the moment it was saved - the dirty flag is
    // `edit_key() != saved_key`, and a changed field moves the key.
    let p = Project::default();
    let before = p.meta.clone();
    let _ = to_ron(&p).expect("the document serialises");
    assert_eq!(p.meta, before, "saving modified the open document");
}

#[test]
fn a_file_with_no_stamp_still_opens() {
    // Documents written before the stamp existed, and documents written by anything that never set a
    // producer, carry no such field. That must read as an empty string, not as a refusal: the value is
    // a diagnostic, and a diagnostic may never cost a person their file.
    let p = Project::default();
    let text = to_ron(&p).expect("the document serialises");
    let stripped: String = text.lines().filter(|l| !l.contains("saved_by")).collect::<Vec<_>>().join("\n");
    let back = from_ron(&stripped).expect("a document without the stamp opens");
    assert_eq!(back.meta.saved_by, "", "a missing stamp did not read as empty");
}
