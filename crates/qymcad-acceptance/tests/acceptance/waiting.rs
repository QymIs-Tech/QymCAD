//! THE CHECKS WAIT AS A PERSON WAITS: for a rebuild in its thread, a project loading, a write in the background,
//! a hint coming up; clicks meant apart stay apart; real time passes for what the program times by the wall clock;
//! a wait that does not end says what is still running.
use std::time::Duration;

use qymcad::{pos2, Key, Session};
use qymcad_acceptance::{build, refusal};
use qymcad_acceptance::probe;

/// A sample project of the repository.
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/Filter-v2.qcad");

/// A fresh file path for what a check writes, under the build's own scratch folder.
fn scratch(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("acceptance");
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("autosave.qcad"));
    path.to_string_lossy().into_owned()
}

/// The height of the block's extrusion changed through its reopened command: a double click on its row, the
/// length typed, Enter.
fn extrude_to(s: &mut Session, length: &str) {
    let kind = s.word("f-extrude");
    let row = s.words_at().into_iter().find(|(w, _)| w.starts_with(&kind)).unwrap_or_else(|| panic!("the extrusion has no row in the tree"));
    s.double_click(row.1.center());
    let caption = s.word("f-length");
    s.fill(&caption, length).key(Key::Enter);
}

probe! {
    /// A REBUILD IN ITS THREAD IS WAITED FOR: what is read after it is the body it built.
    fn a_rebuild_in_its_thread_is_waited_for() {
        let mut s = Session::start();
        build::block(&mut s);
        extrude_to(&mut s, "25");
        let volumes: Vec<f64> = s.document().bodies.iter().map(|b| b.volume).collect();
        assert_eq!(volumes, [30000.0], "the block made 25 high holds {volumes:?} mm^3");
    }
}

probe! {
    /// A PROJECT LOADING IS WAITED FOR: once the chooser is answered, the document read is the whole project.
    fn a_project_loading_is_waited_for() {
        let mut s = Session::start();
        build::open_project(&mut s, SAMPLE);
        let doc = s.document();
        assert!(doc.bodies.len() >= 3 && doc.bodies.iter().all(|b| b.volume > 0.0), "the loaded project holds bodies {:?}", doc.bodies.iter().map(|b| (&b.name, b.volume)).collect::<Vec<_>>());
        assert!(doc.features.iter().all(|f| f.error.is_none()), "a node of the loaded project did not build: {:?}", doc.features.iter().filter(|f| f.error.is_some()).collect::<Vec<_>>());
    }
}

probe! {
    /// A WRITE IN THE BACKGROUND IS WAITED FOR: once Save as is answered, the file is on the disk, whole.
    fn a_write_in_the_background_is_waited_for() {
        let path = scratch("a_write_in_the_background.qcad");
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        assert!(size > 0, "Save as {path} left {size} bytes on the disk");
        assert_eq!(s.document().path.as_deref(), Some(path.as_str()), "the document is not kept in the file it was saved as");
    }
}

probe! {
    /// A HINT COMES UP AFTER THE POINTER RESTS, and not the moment it arrives.
    fn a_hint_comes_up_after_a_rest() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let hint = s.word("tb-extrude-hint");
        let at = s.find_hint(&hint).expect("the part has an extrusion button");
        s.move_to(pos2(640.0, 400.0)).move_to(at);
        assert!(!s.shows(&hint), "the hint was up the moment the pointer arrived");
        assert!(s.hint_at(at).contains(&hint), "the hint did not come up after the pointer rested over its button");
    }
}

probe! {
    /// TWO CLICKS MEANT APART ARE TWO CLICKS: a row clicked twice is not entered, a row double-clicked is.
    fn two_clicks_apart_are_not_a_double_click() {
        let finish = |s: &Session| s.word("wb-finish");
        let mut s = Session::start();
        build::a_part_in_the_assembly(&mut s);
        let row = s.find("Part 1", pos2(0.0, 300.0)).expect("the first part has a row");
        s.click(row.center()).click(row.center());
        let word = finish(&s);
        assert!(!s.shows(&word), "two clicks on the row entered the part as a double click does");
        s.double_click(row.center());
        assert!(s.shows(&word), "a double click on the row did not enter the part");
    }
}

probe! {
    /// REAL TIME PASSES WHILE A PERSON WAITS: the autosave, timed by the wall clock, writes its copy.
    fn the_wall_clock_moves_while_a_person_waits() {
        let path = scratch("the_wall_clock.qcad");
        let copy = std::path::Path::new(&path).with_extension("autosave.qcad");
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let (windows, settings, every) = (s.word("menu-windows"), s.word("menu-settings"), s.word("settings-autosave"));
        s.menu(&[&windows, &settings]);
        s.fill(&every, "1").key(Key::Enter);
        let title = s.word("win-settings");
        s.close_window(&title);
        extrude_to(&mut s, "20");
        assert!(!copy.exists(), "the autosave copy was written before any time passed");
        let what = format!("the autosave of 1 s to write {} beside unsaved work", copy.display());
        s.wait_for(&what, Duration::from_secs(30), |_| copy.exists());
    }
}

probe! {
    /// A WAIT PAST ITS BUDGET FAILS AND SAYS WHAT IS STILL RUNNING.
    fn a_wait_past_its_budget_says_what_is_still_running() {
        let mut s = Session::start();
        let (file, open) = (s.word("menu-file"), s.word("file-open"));
        s.menu(&[&file, &open]);
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, pos2(0.0, 0.0)) {
            s.click(button.center());
        }
        let loading = s.word("io-loading");
        s.budget(Duration::ZERO);
        let said = refusal(|| {
            s.answer_file(SAMPLE);
        });
        assert!(said.contains("did not come to rest") && said.contains(&loading), "a load waited for with no time to wait: {said:?}");
    }
}
