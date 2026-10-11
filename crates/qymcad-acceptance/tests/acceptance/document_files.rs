//! THE DOCUMENT AND ITS FILE: made anew, saved, saved under another name, opened again, and the list of the recent
//! ones; the properties a document carries and the question about unsaved work.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// A file of this check's own, in the folder for temporary things.
fn a_path(name: &str) -> String {
    format!("{}/{name}.qcad", std::env::temp_dir().display())
}

/// Walk the menu of files down to `keys`.
fn file_menu(s: &mut Session, keys: &[&str]) {
    let file = s.word("menu-file");
    let mut path = vec![file];
    for key in keys {
        path.push(s.word(key));
    }
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    s.menu(&path);
}

probe! {
    /// A DOCUMENT IS SAVED AND OPENED AGAIN with everything it held.
    fn a_document_is_saved_and_opened_again() {
        let path = a_path("saved-and-opened");
        let mut s = Session::start();
        build::block(&mut s);
        let made = s.document();
        build::save_as(&mut s, &path);
        assert!(std::path::Path::new(&path).exists(), "the file was not written to {path}: the program says {:?}", s.status());
        assert!(!s.document().unsaved, "the document is still marked unsaved after it was saved");
        assert!(s.title().contains("saved-and-opened"), "the window does not name the file it was saved to: the title says {:?}", s.title());
        // a fresh start, and the file opened in it
        let mut s = Session::start();
        build::open_project(&mut s, &path);
        let opened = s.document();
        assert!(opened.bodies.len() == made.bodies.len(), "the file was opened with {} bodies instead of {}", opened.bodies.len(), made.bodies.len());
        assert!((opened.bodies[0].volume - made.bodies[0].volume).abs() < 1e-6, "the block came back holding {} instead of {}", opened.bodies[0].volume, made.bodies[0].volume);
    }
}

probe! {
    /// SAVE PUTS THE WORK IN THE FILE IT CAME FROM, without asking again.
    fn save_puts_the_work_in_the_file_it_came_from() {
        let path = a_path("saved-again");
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let first = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        // more work, and Save with no question asked
        let extrude = s.word("tb-extrude-hint");
        let sketch = s.document().sketches[0].name.clone();
        let row = s.find(&sketch, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the sketch is not in the tree"));
        s.click(row.center());
        s.press_hint(&extrude);
        s.key(Key::Enter);
        assert!(s.document().unsaved, "the document is not marked unsaved after the work that followed the save");
        file_menu(&mut s, &["file-save"]);
        assert!(!s.document().unsaved, "Save left the document marked unsaved: the program says {:?}", s.status());
        let second = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        assert!(second != first, "the file did not change when the work was saved into it: {first} bytes both times");
    }
}

probe! {
    /// A NEW PROJECT STARTS EMPTY - an assembly with no part in it, a part being made by "New part" alone - and the work
    /// that was there is asked about first.
    fn a_new_project_starts_empty() {
        let mut s = Session::start();
        build::block(&mut s);
        assert!(!s.document().bodies.is_empty(), "the block was not built");
        file_menu(&mut s, &["file-new"]);
        let dont_save = s.word("nav-dont-save");
        assert!(s.shows(&dont_save), "a new project was started over unsaved work without a question; on screen: {:?}", s.words());
        let button = s.find(&dont_save, qymcad::pos2(640.0, 400.0)).expect("the answer to the question");
        s.click(button.center());
        assert!(s.document().bodies.is_empty(), "the new project holds what the old one did: {:?}", s.document().bodies);
        assert!(!s.document().parts.iter().any(|p| !p.assembly), "a new project starts with no part, and it holds {:?}", s.document().parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
    }
}

probe! {
    /// THE DOCUMENT CARRIES ITS OWN PROPERTIES: a title and an author typed there are kept in the file.
    fn the_document_carries_its_own_properties() {
        let path = a_path("with-properties");
        let mut s = Session::start();
        build::block(&mut s);
        file_menu(&mut s, &["file-doc-props"]);
        let props = s.word("doc-props-title");
        assert!(s.shows(&props), "the window of the document properties did not open; on screen: {:?}", s.words());
        let (title, author) = (s.word("doc-props-name"), s.word("doc-props-author"));
        s.fill(&title, "A frame of my own");
        s.fill(&author, "A designer").key(Key::Enter);
        s.close_window(&props);
        build::save_as(&mut s, &path);
        let mut s = Session::start();
        build::open_project(&mut s, &path);
        file_menu(&mut s, &["file-doc-props"]);
        assert!(s.words().iter().any(|w| w == "A frame of my own"), "the title typed into the properties did not come back with the file; on screen: {:?}", s.words());
        assert!(s.words().iter().any(|w| w == "A designer"), "the author typed into the properties did not come back with the file; on screen: {:?}", s.words());
    }
}

probe! {
    /// THE RECENT LIST HOLDS WHAT WAS OPENED, and it can be cleared.
    fn the_recent_list_holds_what_was_opened_and_is_cleared() {
        let path = a_path("recent-one");
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let mut s = Session::start();
        build::open_project(&mut s, &path);
        file_menu(&mut s, &["file-recent"]);
        assert!(s.words().iter().any(|w| w.contains("recent-one")), "the file just opened is not in the list of the recent; on screen: {:?}", s.words());
        let clear = s.word("file-recent-clear");
        s.press_word(&clear);
        file_menu(&mut s, &["file-recent"]);
        assert!(!s.words().iter().any(|w| w.contains("recent-one")), "the list of the recent was cleared and the file is still in it; on screen: {:?}", s.words());
    }
}

probe! {
    /// A TEMPLATE IS SAVED AND A NEW PROJECT IS MADE FROM IT, holding what the template held.
    fn a_template_is_saved_and_a_project_is_made_from_it() {
        let mut s = Session::start();
        build::block(&mut s);
        file_menu(&mut s, &["file-save-as-template"]);
        let name = s.word("tpl-name");
        s.fill(&name, "a-block-to-start-from");
        let yes = s.word("confirm-yes");
        s.press_word(&yes);
        let said = s.status();
        assert!(said.starts_with(&s.word("tpl-saved").split(':').next().unwrap_or_default().to_string()), "nothing says the template was written: the program says {said:?}");
        // a new project from that template: the unsaved work asked about, then the template chosen in the chooser, which
        // opens in the folder of templates
        let written = said.split_once(": ").map(|(_, p)| p.trim().to_string()).unwrap_or_default();
        file_menu(&mut s, &["file-new-from-template"]);
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, qymcad::pos2(640.0, 400.0)) {
            s.click(button.center());
        }
        assert!(s.chooser().is_some(), "no chooser of a template came up; on screen: {:?}", s.words());
        s.answer_file(&written);
        let now = s.document();
        assert!(now.bodies.iter().any(|b| (b.volume - 12000.0).abs() < 1.0), "the project made from the template does not hold the block of it: {:?}", now.bodies.iter().map(|b| b.volume).collect::<Vec<_>>());
        assert!(now.path.is_none(), "the project made from the template took the template's own file: {:?}", now.path);
    }
}

probe! {
    /// AUTOSAVE WRITES A COPY BESIDE THE PROJECT, and a save of one's own takes it away again.
    fn autosave_writes_a_copy_beside_the_project() {
        let path = a_path("autosaved");
        let beside = path.replace(".qcad", ".autosave.qcad");
        let _ = std::fs::remove_file(&beside);
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let (windows, settings) = (s.word("menu-windows"), s.word("win-settings"));
        s.menu(&[&windows, &settings]);
        let every = s.word("settings-autosave");
        s.fill(&every, "2").key(Key::Enter);
        s.close_window(&settings);
        // more work, and then the wait
        let sketch = s.document().sketches[0].name.clone();
        let row = s.find(&sketch, qymcad::pos2(0.0, 300.0)).unwrap_or_else(|| panic!("the sketch is not in the tree"));
        s.click(row.center());
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        let what = format!("the autosave of 2 s to write a copy beside the project, {beside}");
        s.wait_for(&what, std::time::Duration::from_secs(30), |_| std::path::Path::new(&beside).exists());
        file_menu(&mut s, &["file-save"]);
        assert!(!std::path::Path::new(&beside).exists(), "the copy beside the project is still there after the work was saved: {beside}");
    }
}
