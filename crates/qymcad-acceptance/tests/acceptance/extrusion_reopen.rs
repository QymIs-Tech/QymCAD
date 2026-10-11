//! AN EXTRUSION REOPENED COMES UP WITH ITS COMMAND IN HAND - whichever way it was reached.
//!
//! Reported behaviour: editing an extrusion now and then left a bare 2D view of the sketch: no bar of options, no
//! region picked, no Apply or Cancel - nothing to go on with. Not reproduced at will, and no dependence found. The
//! straight double click on the node's row is the tool contract's (`b13_reopen`, green); every other way of coming
//! to the reopening is tried here, each ending in what a person must see: the command's bar with Apply and Cancel.
use qymcad::{pos2, Key, Modifiers, PointerButton, Pos2, Session};
use qymcad_acceptance::{build, probe};

/// A fresh file path for what a check writes, under the build's own scratch folder.
fn scratch(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("acceptance");
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("autosave.qcad"));
    path.to_string_lossy().into_owned()
}

/// The row of the tree that begins with `lead`.
fn row(s: &mut Session, lead: &str) -> Pos2 {
    // the tree: left of the canvas and below its top - the bar of an open command writes the tool's name up there too
    let canvas = s.canvas();
    let rows: Vec<Pos2> = s.words_at().into_iter().filter(|(w, r)| r.max.x < canvas.min.x && r.min.y > canvas.min.y && w.starts_with(lead)).map(|(_, r)| r.center()).collect();
    match rows.as_slice() {
        [r] => *r,
        other => panic!("the row beginning {lead:?} is not one row of the tree: {other:?}; on screen: {:?}", s.words()),
    }
}

/// The row of the extrusion, as the tree names it.
fn extrusion(s: &mut Session) -> Pos2 {
    let lead = s.word("f-extrude");
    row(s, &lead)
}

/// THE EXTRUSION IS IN HAND: its bar of options with Apply and Cancel, as a person goes on from there.
fn in_hand(s: &mut Session, way: &str) {
    let (apply, cancel) = (s.word("cmd-apply-enter"), s.word("cmd-cancel-btn"));
    let bar = s.in_hand();
    assert!(bar.contains(&apply) && bar.contains(&cancel), "{way}: no command in hand - a bare view; the bars say {bar:?}");
}

/// Reopen the extrusion by a double click on its row, and look.
fn reopen(s: &mut Session, way: &str) {
    let at = extrusion(s);
    s.double_click(at);
    in_hand(s, way);
}

probe! {
    /// REOPENED AFTER ESC: the extrusion left with Esc, and reopened.
    fn an_extrusion_reopens_after_esc() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "the first time");
        s.key(Key::Escape);
        reopen(&mut s, "after Esc");
    }
}

probe! {
    /// REOPENED AFTER APPLYING IT: changed, applied with Enter, and reopened.
    fn an_extrusion_reopens_after_it_was_applied() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "the first time");
        let length = s.word("f-length");
        s.fill(&length, "14").key(Key::Enter);
        reopen(&mut s, "after it was applied");
    }
}

probe! {
    /// REOPENED TWICE OVER: a second double click on its row while it is already open.
    fn an_extrusion_reopened_twice_over_stays_in_hand() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "the first time");
        reopen(&mut s, "again, while open");
    }
}

probe! {
    /// REOPENED AFTER UNDO AND REDO of a change made in it.
    fn an_extrusion_reopens_after_undo_and_redo() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "the first time");
        let length = s.word("f-length");
        s.fill(&length, "14").key(Key::Enter);
        s.chord(Modifiers::COMMAND, Key::Z);
        reopen(&mut s, "after undo");
        s.key(Key::Escape);
        s.chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        reopen(&mut s, "after redo");
    }
}

probe! {
    /// REOPENED WHILE ANOTHER COMMAND IS IN HAND: the fillet taken, then the extrusion's row double clicked.
    fn an_extrusion_reopens_while_another_command_is_in_hand() {
        let mut s = Session::start();
        build::block(&mut s);
        let fillet = s.word("tb-fillet-body-hint");
        s.press_hint(&fillet);
        reopen(&mut s, "with the fillet in hand");
    }
}

probe! {
    /// REOPENED AFTER ITS SKETCH WAS OPENED AND FINISHED: the sketch opened from the tree, left by its button, and the
    /// extrusion's row double clicked.
    fn an_extrusion_reopens_after_its_sketch_was_opened() {
        let mut s = Session::start();
        build::block(&mut s);
        let name = s.document().sketches[0].name.clone();
        let sketch = row(&mut s, &name);
        s.double_click(sketch);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        reopen(&mut s, "after its sketch was opened and finished");
    }
}

probe! {
    /// REOPENED FROM ITS MENU: the right button on its row, then Edit.
    fn an_extrusion_reopens_from_its_menu() {
        let mut s = Session::start();
        build::block(&mut s);
        let at = extrusion(&mut s);
        s.click_with(at, PointerButton::Secondary, Modifiers::NONE);
        let edit = s.word("act-edit");
        s.press_word_near(&edit, at);
        in_hand(&mut s, "from its menu");
    }
}

probe! {
    budget = 300;
    /// REOPENED IN A PROJECT OPENED FROM ITS FILE: saved, opened again, and its extrusion reopened.
    fn an_extrusion_reopens_in_a_project_opened_from_its_file() {
        let path = scratch("extrusion_reopen.qcad");
        let mut s = Session::start();
        build::block(&mut s);
        build::save_as(&mut s, &path);
        let (file, open) = (s.word("menu-file"), s.word("file-open"));
        s.menu(&[&file, &open]);
        let dont_save = s.word("nav-dont-save");
        if let Some(button) = s.find(&dont_save, pos2(0.0, 0.0)) {
            s.click(button.center());
        }
        s.answer_file(&path);
        qymcad_acceptance::build::into_the_first_part(&mut s);
        reopen(&mut s, "in a project opened from its file");
    }
}

/// NOT A BARE FLAT VIEW: either the command is in hand with its bar, or the view stands in 3D as it did before it.
fn not_bare(s: &mut Session, way: &str) {
    let (apply, cancel) = (s.word("cmd-apply-enter"), s.word("cmd-cancel-btn"));
    let bar = s.in_hand();
    let held = bar.contains(&apply) && bar.contains(&cancel);
    assert!(held || !s.flat(), "{way}: a flat view with no command in hand - the bare view of the report; the bars say {bar:?}");
}

/// Press the button of the bar that opens the flat picker of the regions.
fn pick_contours(s: &mut Session) {
    let pick = s.word("cmd-pick-contours");
    s.press_word_near(&pick, pos2(640.0, 24.0));
}

probe! {
    /// THE PICKER OF THE REGIONS OF A REOPENED EXTRUSION: opened by its button, it holds the command and its bar;
    /// left with Esc, it leaves no bare flat view behind.
    fn the_picker_of_a_reopened_extrusion_holds_its_bar_and_leaves_no_bare_view() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "reopened");
        pick_contours(&mut s);
        assert!(s.flat(), "the button of the regions did not open the flat picker");
        in_hand(&mut s, "in the picker of the regions");
        s.key(Key::Escape);
        not_bare(&mut s, "the picker left with Esc");
        s.key(Key::Escape);
        not_bare(&mut s, "the command left with a second Esc");
    }
}

probe! {
    /// THE PICKER OF THE REGIONS BY ITS KEY, and applied: no bare flat view after it.
    fn the_picker_of_a_reopened_extrusion_by_its_key_and_applied_leaves_no_bare_view() {
        let mut s = Session::start();
        build::block(&mut s);
        reopen(&mut s, "reopened");
        s.key(Key::U);
        in_hand(&mut s, "in the picker opened by its key");
        s.key(Key::Enter);
        not_bare(&mut s, "the picker applied with Enter");
        s.key(Key::Enter);
        not_bare(&mut s, "the command applied with a second Enter");
    }
}

probe! {
    /// A REVOLVE REOPENED, ITS AXIS TAKEN FROM THE SKETCH: the flat view it goes to keeps the revolve in hand. Its node,
    /// not its sketch, is selected there - the editor of a sketch's label used to end a text edit on every flat frame
    /// without a sketch selected, and put down whatever was in hand.
    fn a_revolve_reopened_keeps_its_command_while_its_axis_is_taken_from_the_sketch() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let revolve = s.word("tb-revolve-hint");
        s.press_hint(&revolve);
        s.key(Key::Enter);
        let lead = s.word("f-revolve");
        let at = row(&mut s, &lead);
        s.double_click(at);
        in_hand(&mut s, "the revolve reopened");
        let axis = s.word("cmd-pick-axis-sketch");
        s.press_word_near(&axis, pos2(640.0, 24.0));
        assert!(s.flat(), "the button of the axis did not go to the flat view");
        in_hand(&mut s, "the axis taken from the sketch");
    }
}
