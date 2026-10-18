//! DOCUMENTS: new, opened, saved, closed - as a person handles them.
use qymcad::{pos2, Session};
use qymcad_acceptance::probe;

/// A sample project of the repository.
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Filter-v2.qcad");

probe! {
    /// A PROJECT OPENED AND NOT TOUCHED CLOSES WITHOUT A QUESTION: there is nothing in it to save.
    fn a_project_opened_and_untouched_closes_without_a_question() {
        let mut s = Session::start();
        let file = s.word("menu-file");
        let file = s.find(&file, pos2(0.0, 0.0)).expect("the File menu is on screen");
        s.click(file.center());
        let open = s.word("file-open");
        let open = s.find(&open, file.center()).expect("Open project is in the File menu");
        s.click(open.center());
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, pos2(0.0, 0.0)) {
            s.click(button.center());
        }
        s.answer_file(SAMPLE);
        let title = s.title();
        assert!(title.contains("Filter-v2") && !title.ends_with('*'), "a project just opened calls itself unsaved: {title:?}");
        if let Err(mut s) = s.quit() {
            panic!("a project opened and not touched asks about unsaved changes on closing; on screen: {:?}", s.words());
        }
    }
}

probe! {
    budget = 600;
    /// A PROJECT REBUILT WITH NEW MESHES ASKS TO BE SAVED. Reported behaviour: Edit -> Rebuild everything mended the
    /// roundings and the light of an old document - its meshes came back with the surface's normals - yet the title
    /// had no mark of a change, closing asked nothing, and opened again the roundings were broken as before.
    fn a_project_rebuilt_with_new_meshes_asks_to_be_saved() {
        let mut s = Session::start();
        let (file, open) = (s.word("menu-file"), s.word("file-open"));
        s.menu(&[&file, &open]);
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, pos2(0.0, 0.0)) {
            s.click(button.center());
        }
        // a document of an old version made for the probe: the example stays free to be saved by any build
        let old_document = Session::old_document_of(SAMPLE).expect("the old document is written");
        s.answer_file(&old_document);
        let old = s.document().bodies.iter().all(|b| b.lit_by_the_surface == 0);
        assert!(old, "setup: the old document the probe wrote holds the surface's normals");
        let title = s.title();
        assert!(!title.ends_with('*'), "a project just opened calls itself unsaved: {title:?}");

        let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
        s.menu(&[&edit, &rebuild]);
        let lit = s.document().bodies.iter().map(|b| b.lit_by_the_surface).sum::<usize>();
        assert!(lit > 0, "setup: rebuilt, the bodies carry no normals of the surface");
        let title = s.title();
        assert!(title.ends_with('*'), "the bodies were rebuilt with new meshes and the title does not say the project changed: {title:?}");
        assert!(s.quit().is_err(), "the bodies were rebuilt with new meshes and closing asks nothing - they would be lost");
        let _ = std::fs::remove_file(&old_document);
    }
}

probe! {
    /// THE QUESTION ABOUT UNSAVED WORK HOLDS THE WINDOW: while it waits for an answer, nothing behind it takes a click.
    fn the_question_about_unsaved_work_holds_the_window() {
        let mut s = Session::start();
        qymcad_acceptance::build::into_the_first_part(&mut s);
        qymcad_acceptance::build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let (file, new) = (s.word("menu-file"), s.word("file-new"));
        s.menu(&[&file, &new]);
        let question = s.word("nav-unsaved-title");
        assert!(s.shows(&question), "a new project over an unsaved sketch asks nothing; on screen: {:?}", s.words());
        let edit = s.word("sk-edit-btn");
        let behind = s.find(&edit, pos2(1280.0, 0.0)).unwrap_or_else(|| panic!("{edit:?} stands in the panel behind the question; on screen: {:?}", s.words()));
        s.click(behind.center());
        assert!(s.shows(&question), "a click behind the question put the question away");
        // the sketch opened is read from the document: "Finish" stands on screen in a part as well, for the part itself
        assert!(s.document().editing.is_none(), "a click on {edit:?} behind the question opened the sketch {:?} while the question waited", s.document().editing);
    }
}

/// A new project with a rectangle on XY, and Extrude taken on it: the sketch just finished is the contour it takes.
fn extrude_in_a_new_project(s: &mut Session) -> qymcad::Picture {
    let (file, new) = (s.word("menu-file"), s.word("file-new"));
    s.menu(&[&file, &new]);
    let dont = s.word("nav-dont-save");
    if let Some(r) = s.find(&dont, pos2(640.0, 400.0)) {
        s.click(r.center());
    }
    qymcad_acceptance::build::into_the_first_part(s);
    qymcad_acceptance::build::rectangle_on_xy(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    let extrude = s.word("tb-extrude-hint");
    s.press_hint(&extrude);
    s.snapshot()
}

probe! {
    /// NOTHING OF A CLOSED PROJECT IS DRAWN IN THE NEXT ONE. Reported behaviour: a project closed, a new one made, a
    /// sketch drawn and Extrude taken on it - and the green faces a check of a command had built in the closed project,
    /// a piece of its imported body, stood over the new sketch.
    fn nothing_of_a_closed_project_is_drawn_in_the_next() {
        let mut s = Session::start();
        qymcad_acceptance::build::block(&mut s);
        s.key(qymcad::Key::Escape);
        let hole = s.word("tb-hole-hint");
        s.press_hint(&hole);
        let at = s.face_at([20.0, 15.0, 10.0]);
        s.click(at);
        let _ = s.document();
        s.key(qymcad::Key::Escape).key(qymcad::Key::Escape);
        let after = extrude_in_a_new_project(&mut s);
        let mut clean = Session::start();
        let fresh = extrude_in_a_new_project(&mut clean);
        let canvas = s.canvas();
        if qymcad_acceptance::golden::changed_in(&fresh, &after, canvas, &[]) {
            let dir = std::env::temp_dir();
            let _ = std::fs::write(dir.join("closed-project-after.png"), after.png());
            let _ = std::fs::write(dir.join("closed-project-fresh.png"), fresh.png());
            panic!("Extrude in a new project is drawn otherwise after another project was worked in: see {}", dir.join("closed-project-after.png").display());
        }
    }
}
