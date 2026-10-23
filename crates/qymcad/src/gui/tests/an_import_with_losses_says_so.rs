//! AN IMPORT THAT CAME IN WITH LOSSES IS SAID WITH THE TONE OF A WARNING: the bodies land as any file's do,
//! and the status keeps the fact of the import and adds what was dropped, lifted to a warning - a quiet
//! "imported" would read as whole. What came in without losses keeps the plain tone.
use super::{App, JobResult};
use qymcad_core::feature::PLACE_IDENTITY;
use qymcad_ui_state::{ExactImport, Tone};

/// A one-body STEP on disk, as an import reads it.
fn step_on_disk(tag: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-losses-app", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
    let path = dir.join(format!("{tag}.step"));
    qymcad_kernel::write_step(&[(&cube, PLACE_IDENTITY)], &path.to_string_lossy()).expect("written");
    path
}

/// The import of that file, landed the way a worker lands it, with `note` as the reading said it.
fn landed(path: &std::path::Path, note: &str) -> JobResult {
    let qymcad_kernel::ExactTree { bodies, shapes, nodes } = qymcad_kernel::read_exact_tree(qymcad_kernel::ExactFormat::Step, &path.to_string_lossy(), 0.5).expect("reads");
    JobResult::ExactImported {
        landed: ExactImport { path: path.to_string_lossy().into_owned(), format: qymcad_kernel::ExactFormat::Step, bodies, shapes, nodes, embedded: None, note: note.to_string() },
    }
}

#[test]
fn an_import_with_losses_lands_as_a_warning() {
    let path = step_on_disk("with-losses");
    let mut app = App::default();
    app.apply_job_result(landed(&path, "the converter carried the file in with losses: 8 surfaces were normalized"));
    assert_eq!(app.status.tone, Tone::Warning, "a lossy import kept the plain tone: {:?}", app.status.words);
    assert!(app.status.words.contains("8 surfaces were normalized"), "the losses were not said: {:?}", app.status.words);
}

#[test]
fn an_import_without_losses_keeps_the_plain_tone() {
    let path = step_on_disk("without-losses");
    let mut app = App::default();
    app.apply_job_result(landed(&path, ""));
    assert_eq!(app.status.tone, Tone::Info, "a whole import was said as a warning: {:?}", app.status.words);
}
