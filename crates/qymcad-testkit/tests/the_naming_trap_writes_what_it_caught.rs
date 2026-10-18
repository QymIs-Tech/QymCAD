//! THE TRAP OF DETERMINISM WRITES WHAT IT CAUGHT. A rename of the names the same document was to give again is rare
//! and comes with the state of the process (the chamfer 11963 of the case of the examples: 3 of 20 rebuilds with the
//! filter rebuilt before it in one process, none of 20 alone); the matrix of the real documents keeps what repeats it.
//! Here a document of one block and one parameter is opened and the parameter edited and undone, a fault is handed to the trap, and the trap is to have written
//! the document as opened, the document as it stood and every step up to the fault. Live OCCT, headless.
use qymcad_testkit::real_documents::{self, Doc};

#[test]
fn the_first_fault_of_determinism_leaves_the_document_and_its_steps() {
    let folder = std::env::temp_dir().join(format!("qymcad-naming-trap-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder for the document");
    let path = folder.join("block.qcad");
    let mut p = qymcad_core::model::Project::default();
    p.new_document();
    let _ = p.add_box(10.0, 20.0, 30.0);
    p.parameters.push(qymcad_core::model::Param { name: "h".into(), expr: "8".into(), value: 8.0 });
    qymcad_io::save_project(&p, &path.to_string_lossy()).expect("the block is saved");
    let doc = Doc { name: "block.qcad".to_string(), path: path.to_string_lossy().into_owned() };
    let mut o = real_documents::open(&doc);
    let edit = real_documents::edits(&o.project).into_iter().next().expect("the parameter of the document");
    let faults = real_documents::edit_and_back(&mut o, &edit, 1.1);
    assert!(faults.is_empty(), "the block edited and undone: {faults:?}");

    let dir = real_documents::trap(&o, &["a face renamed".to_string()]);
    assert!(dir.join("block.qcad").exists(), "the document as it was opened is not at {}", dir.display());
    assert!(dir.join("at-the-fault.block.qcad").exists(), "the document as it stood at the fault is not at {}", dir.display());
    let steps = std::fs::read_to_string(dir.join("steps.txt")).expect("the steps are written");
    for said in [format!("open {}", doc.path), "undone".to_string(), "a face renamed".to_string()] {
        assert!(steps.contains(&said), "the steps do not say \"{said}\":\n{steps}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&folder);
}
