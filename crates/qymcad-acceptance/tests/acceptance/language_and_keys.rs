//! THE LANGUAGE, THE COLOURS AND THE KEYS: the interface set to another language and kept there, a colour scheme
//! that changes what is drawn, the keys of a workbench and the reassigning of one.
use qymcad::{Key, Machine, Session};
use qymcad_acceptance::{build, golden, probe};

/// Open the settings at the section `key` names.
fn open_at(s: &mut Session, key: &str) {
    let (windows, settings) = (s.word("menu-windows"), s.word("menu-settings"));
    let title = s.word("win-settings");
    if !s.shows(&title) {
        s.menu(&[&windows, &settings]);
    }
    let section = s.word(key);
    s.press_word_near(&section, qymcad::pos2(700.0, 200.0));
}

/// Put the settings away.
fn close_the_settings(s: &mut Session) {
    let title = s.word("win-settings");
    s.close_window(&title);
}

/// CLOSE THE PROGRAM AND ANSWER WHAT IT ASKS, keeping what it keeps between runs.
fn close_the_program(s: Session) -> qymcad::Kept {
    match s.quit() {
        Ok(kept) => kept,
        Err(mut s) => {
            let dont_save = s.word("nav-dont-save");
            s.press_word(&dont_save);
            s.quit().unwrap_or_else(|_| panic!("the window was closed and would not give what it keeps"))
        }
    }
}

/// The table of the keys, opened from the Help menu.
fn open_the_keys(s: &mut Session) {
    let (help, keys) = (s.word("menu-help"), s.word("help-hotkeys"));
    s.menu(&[&help, &keys]);
}

/// WHERE THE KEY OF THE ACTION THAT READS `what` IS PRESSED: the button stands at the head of its row.
fn key_button(s: &mut Session, what: &str) -> qymcad::Rect {
    let row = s.find(what, qymcad::pos2(640.0, 400.0)).unwrap_or_else(|| panic!("the table of the keys has no row {what:?}; on screen: {:?}", s.words()));
    s.widgets()
        .into_iter()
        .filter(|w| w.kind == qymcad::Kind::Button && w.rect.center().y > row.min.y && w.rect.center().y < row.max.y && w.rect.max.x <= row.min.x + 1.0)
        .max_by(|a, b| a.rect.max.x.total_cmp(&b.rect.max.x))
        .map(|w| w.rect)
        .unwrap_or_else(|| panic!("the row {what:?} has no key to press; on screen: {:?}", s.words()))
}

/// Is the extrusion in hand? - the bar of options names it at its head.
fn the_extrusion_is_in_hand(s: &mut Session) -> bool {
    let name = s.word("cmd-extrude");
    s.in_hand().first().is_some_and(|w| w.contains(&name))
}

probe! {
    /// THE INTERFACE IS SET TO ANOTHER LANGUAGE and stays in it at the next start.
    fn the_interface_is_set_to_another_language_and_stays() {
        let mut s = Session::start();
        s.key(Key::Escape);
        assert!(s.word("menu-file") == "File", "this check starts from an English interface, and it says {:?}", s.word("menu-file"));
        open_at(&mut s, "settings-sec-general");
        // the languages are named in their own language, in the row under the caption; the help has a row of its
        // own below, where the same names stand again
        let caption = s.find(&s.word("settings-language").clone(), qymcad::pos2(700.0, 200.0)).unwrap_or_else(|| panic!("the settings offer no language; on screen: {:?}", s.words()));
        s.press_word_near("\u{420}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439}", caption.center());
        close_the_settings(&mut s);
        let file = s.word("menu-file");
        assert!(file != "File", "the language was set to Russian and the File menu is still {file:?}");
        assert!(s.shows(&file), "the language was set to Russian and {file:?} is not on screen; on screen: {:?}", s.words());
        let kept = close_the_program(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        s.key(Key::Escape);
        assert!(s.word("menu-file") == file, "the language was not kept for the next run: the File menu is {:?}", s.word("menu-file"));
    }
}

probe! {
    /// A COLOUR SCHEME CHANGES WHAT IS DRAWN and is kept for the next start.
    fn a_colour_scheme_changes_what_is_drawn() {
        let mut s = Session::start();
        s.key(Key::Escape);
        let before = s.snapshot();
        open_at(&mut s, "settings-sec-appearance");
        let caption = s.word("settings-scheme");
        let row = s.find(&caption, qymcad::pos2(700.0, 200.0)).unwrap_or_else(|| panic!("the settings offer no colour scheme; on screen: {:?}", s.words()));
        // the schemes are named under the caption: the first one that is not the one in use is taken
        let other = s
            .widgets()
            .into_iter()
            .filter(|w| {
                matches!(w.kind, qymcad::Kind::Button | qymcad::Kind::RadioButton | qymcad::Kind::Other)
                    && w.checked != Some(true)
                    && !w.label.is_empty()
                    && w.rect.min.x >= row.min.x - 5.0
                    && w.rect.min.y >= row.max.y - 1.0
                    && w.rect.min.y < row.max.y + 60.0
            })
            .min_by(|a, b| (a.rect.min.y, a.rect.min.x).partial_cmp(&(b.rect.min.y, b.rect.min.x)).unwrap())
            .unwrap_or_else(|| panic!("the settings name no other colour scheme; on screen: {:?}", s.words()));
        let name = other.label.clone();
        s.click(other.rect.center());
        close_the_settings(&mut s);
        let after = s.snapshot();
        assert!(!golden::same(&before, &after), "the colour scheme was set to {name:?} and the window looks the same");
        let kept = close_the_program(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        s.key(Key::Escape);
        assert!(golden::same(&after, &s.snapshot()), "the colour scheme {name:?} was not kept for the next run");
    }
}

probe! {
    /// A KEY OF A WORKBENCH DOES ITS WORK: E takes the extrusion in a part.
    fn a_key_of_a_workbench_does_its_work() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish); // the sketch is done and stands chosen, as it does when a person presses E
        s.key(Key::E);
        assert!(the_extrusion_is_in_hand(&mut s), "E was pressed in a part and the bar says {:?}", s.in_hand());
    }
}

probe! {
    /// A KEY IS REASSIGNED AND THE NEW ONE DOES THE WORK, the old one no longer; and every key can be put back.
    fn a_key_is_reassigned_and_put_back() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish); // the sketch is done and chosen, so the extrusion has what to work on
        open_the_keys(&mut s);
        let what = s.word("hotkey-part-e");
        let button = key_button(&mut s, &what);
        s.click(button.center());
        let waiting = s.word("hotkeys-press");
        assert!(s.shows(&waiting), "the key of {what:?} was clicked and the table does not wait for a new one; on screen: {:?}", s.words());
        s.key(Key::J); // a key no tool of this workbench holds
        s.key(Key::Enter); // the key is one press: Enter saves it rather than waiting for a second
        let title = s.word("hotkeys-title");
        s.close_window(&title);
        s.key(Key::J);
        assert!(the_extrusion_is_in_hand(&mut s), "the extrusion was put on J and J does not take it: the bar says {:?}", s.in_hand());
        // the Esc ladder: the first takes the focus out of the length field, the next puts the command down
        for _ in 0..3 {
            if !the_extrusion_is_in_hand(&mut s) {
                break;
            }
            s.key(Key::Escape);
        }
        assert!(!the_extrusion_is_in_hand(&mut s), "Esc did not put the extrusion down: the bar says {:?}", s.in_hand());
        s.key(Key::E);
        assert!(!the_extrusion_is_in_hand(&mut s), "the extrusion was moved off E and E still takes it");
        // and back to the factory keys
        open_the_keys(&mut s);
        let reset = s.word("hotkeys-reset-all");
        s.press_word(&reset);
        s.close_window(&title);
        s.key(Key::E);
        assert!(the_extrusion_is_in_hand(&mut s), "every key was put back and E does not take the extrusion: the bar says {:?}", s.in_hand());
    }
}

/// THE TABLE OF THE KEYS, WAITING FOR A NEW KEY FOR THE EXTRUSION.
fn waiting_for_a_key(s: &mut Session) -> String {
    build::into_the_first_part(s);
    build::rectangle_on_xy(s);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    open_the_keys(s);
    let what = s.word("hotkey-part-e");
    let button = key_button(s, &what);
    s.click(button.center());
    let waiting = s.word("hotkeys-press");
    assert!(s.shows(&waiting), "the key of {what:?} was clicked and the table does not wait for a new one; on screen: {:?}", s.words());
    what
}

/// WHAT THE TABLE SAYS about the key just pressed. The answer stands under the row that waits, so it is read
/// where the row is: scrolling the table down takes it off the screen.
fn what_the_table_says(s: &mut Session) -> Vec<String> {
    let title = s.word("hotkeys-title");
    assert!(s.shows(&title), "the table of the keys is not open; on screen: {:?}", s.words());
    s.words()
}

probe! {
    /// A KEY ANOTHER TOOL OF THE SAME WORKBENCH ALREADY HOLDS IS NAMED, with a way to swap the two; pressing it
    /// while the table waits does not run that tool, and the swap gives each the other's key.
    fn a_key_already_taken_can_be_swapped() {
        let mut s = Session::start();
        let _ = waiting_for_a_key(&mut s);
        s.key(Key::H); // the shell's key in this workbench
        s.key(Key::Enter); // the key is one press: Enter ends it and asks about the clash
        let said = what_the_table_says(&mut s);
        let line = s.word("hotkeys-taken").replace("{ $key }", "H").replace("{$key}", "H");
        let head = line.split(['{', ':']).next().unwrap_or(&line).trim().to_string();
        assert!(said.iter().any(|w| w.contains(&head)), "H is taken by another tool and the table says nothing of {head:?}; on screen: {said:?}");
        let swap = s.word("hotkeys-swap");
        s.press_word(&swap);
        let title = s.word("hotkeys-title");
        s.close_window(&title);
        s.key(Key::H);
        assert!(the_extrusion_is_in_hand(&mut s), "the extrusion was swapped onto H and H does not take it: the bar says {:?}", s.in_hand());
    }
}

probe! {
    /// A KEY THAT BELONGS TO THE SYSTEM IS REFUSED IN WORDS.
    fn a_key_of_the_system_is_refused_in_words() {
        let mut s = Session::start();
        let _ = waiting_for_a_key(&mut s);
        s.key(Key::Enter);
        let said = what_the_table_says(&mut s);
        let reserved = s.word("hotkeys-reserved");
        let head = reserved.split('.').next().unwrap_or(&reserved).trim().to_string();
        assert!(said.iter().any(|w| w.contains(&head)), "Enter belongs to the system and the table says nothing of {head:?}; on screen: {said:?}");
    }
}
