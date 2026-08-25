//! A save does not destroy work. This test was written after a loss.
//!
//! An empty document landed on top of a finished project of 1217 nodes, with nowhere to recover it from: an
//! atomic swap saves from a truncated write but not from writing the wrong thing. A copy of the previous version
//! and a refusal to write empty over non-empty are the two things that would have saved that day.
use qymcad_core::model::Project;

fn tmp(tag: &str) -> String {
    let d = std::env::temp_dir().join(format!("qym_save_guard_{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("p.qcad").to_string_lossy().into_owned()
}

/// A project with content: minimal, but not empty.
fn with_content() -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch(String::from("s"));
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p
}

/// Empty does not land on top of non-empty: exactly the case that cost the work.
#[test]
fn an_empty_document_never_overwrites_a_project() {
    let path = tmp("empty_over_full");
    qymcad_io::save_project(&with_content(), &path).expect("a project with content was saved");
    let before = std::fs::metadata(&path).unwrap().len();

    let empty = Project::default();
    let err = qymcad_io::save_project_guarded(&empty, &path).expect_err("empty over non-empty has to be rejected");
    // the file layer has no language: it returns a code and the number of nodes that would have been lost,
    // and the window supplies the words
    assert!(err.starts_with("io-refuse-empty-over-full#"), "a refusal has to arrive as a code: {err}");
    assert!(err.ends_with("#3"), "and name how many nodes were at stake: {err}");

    let after = std::fs::metadata(&path).unwrap().len();
    assert_eq!(before, after, "the file has to stay untouched");
    let back = qymcad_io::load_project(&path).expect("and still read back");
    assert!(qymcad_io::content_weight(&back) > 0, "the content is in place");
}

/// Writing empty into an empty file is allowed: the guard does not get in the way of a new project.
#[test]
fn an_empty_document_saves_fine_when_there_is_nothing_to_lose() {
    let path = tmp("empty_over_nothing");
    qymcad_io::save_project_guarded(&Project::default(), &path).expect("a new empty project saves");
    assert!(std::fs::metadata(&path).is_ok(), "the file was created");
}

/// The previous version stays alongside, for the case where the guard does not fire because the document is not
/// empty but merely the wrong one.
#[test]
fn the_previous_version_is_kept_next_to_the_file() {
    let path = tmp("keeps_bak");
    let first = with_content();
    qymcad_io::save_project(&first, &path).expect("the first save");
    let n1 = qymcad_io::content_weight(&qymcad_io::load_project(&path).unwrap());

    let mut second = with_content();
    let si = second.new_sketch(String::from("s2"));
    second.add_rect_entity(si, 0.0, 0.0, 5.0, 5.0, qymcad_core::feature::Purpose::Real);
    second.regen_sketch(si);
    qymcad_io::save_project(&second, &path).expect("the second save");

    let bak = format!("{path}.bak");
    assert!(std::fs::metadata(&bak).is_ok(), "the previous version has to stay alongside: {bak}");
    let old = qymcad_io::load_project(&bak).expect("the copy reads back");
    assert_eq!(qymcad_io::content_weight(&old), n1, "the copy holds exactly the previous content");
}
