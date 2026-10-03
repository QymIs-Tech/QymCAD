//! THE SETTINGS: every section opens, every switch of them is remembered between runs, and the ones that promise
//! something particular - the start screen, the height an extrusion offers, the scale of the interface - do it.
use qymcad::{Key, Modifiers, Session};
use qymcad_acceptance::{build, probe};

/// THE SECTIONS OF THE SETTINGS, in the order the window lists them on the left.
const SECTIONS: [&str; 8] = ["settings-sec-general", "settings-sec-appearance", "settings-sec-viewport", "settings-sec-sketch", "settings-sec-part", "settings-sec-assembly", "settings-sec-layout", "settings-sec-developer"];

/// Open the settings at the section `key` names.
fn open_at(s: &mut Session, key: &str) {
    let (windows, settings) = (s.word("menu-windows"), s.word("menu-settings"));
    if !s.shows(&s.word("win-settings").clone()) {
        s.menu(&[&windows, &settings]);
    }
    let section = s.word(key);
    s.press_word_near(&section, qymcad::pos2(700.0, 200.0));
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

/// Put the settings away.
fn close_the_settings(s: &mut Session) {
    let title = s.word("win-settings");
    s.close_window(&title);
}

/// The switches of the section now shown, by the words beside them.
fn switches(s: &mut Session) -> Vec<(String, Option<bool>)> {
    s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::CheckBox).map(|w| (w.label.clone(), w.checked)).collect()
}

probe! {
    /// EVERY SECTION OF THE SETTINGS OPENS and shows something to set.
    fn every_section_of_the_settings_opens() {
        let mut s = Session::start();
        s.key(Key::Escape);
        for key in SECTIONS {
            open_at(&mut s, key);
            let rows = s.widgets().into_iter().filter(|w| matches!(w.kind, qymcad::Kind::CheckBox | qymcad::Kind::ComboBox | qymcad::Kind::Slider | qymcad::Kind::Number | qymcad::Kind::TextField | qymcad::Kind::Button)).count();
            assert!(rows > 1, "the section {:?} of the settings shows nothing to set; on screen: {:?}", s.word(key), s.words());
        }
        close_the_settings(&mut s);
    }
}

probe! {
    /// EVERY SWITCH OF THE SETTINGS IS REMEMBERED: each one is turned the other way, the program is closed and
    /// started again, and each one stands where it was left.
    fn every_switch_of_the_settings_is_remembered() {
        let mut s = Session::start();
        s.key(Key::Escape);
        // the switches of the panels behind the window are not the settings': only what the window itself brings
        let behind: Vec<String> = switches(&mut s).into_iter().map(|(label, _)| label).collect();
        let mut left: Vec<(String, String, Option<bool>)> = Vec::new();
        for key in SECTIONS {
            open_at(&mut s, key);
            for (label, _) in switches(&mut s).into_iter().filter(|(label, _)| !behind.contains(label)) {
                let switch = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::CheckBox && w.label == label);
                let Some(switch) = switch else { continue };
                s.click(switch.rect.center());
                let now = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::CheckBox && w.label == label).and_then(|w| w.checked);
                left.push((key.to_string(), label, now));
            }
        }
        close_the_settings(&mut s);
        assert!(!left.is_empty(), "the settings hold no switch at all to try");
        let kept = close_the_program(s);
        let mut s = Session::start_on(qymcad::Machine { kept, ..qymcad::Machine::default() });
        s.key(Key::Escape);
        let mut forgotten: Vec<String> = Vec::new();
        for key in SECTIONS {
            open_at(&mut s, key);
            for (section, label, was) in left.iter().filter(|(sec, _, _)| sec == key) {
                let now = s.widgets().into_iter().find(|w| w.kind == qymcad::Kind::CheckBox && &w.label == label).and_then(|w| w.checked);
                if now != *was {
                    forgotten.push(format!("{section}/{label}: was left {was:?} and came back {now:?}"));
                }
            }
        }
        close_the_settings(&mut s);
        assert!(forgotten.is_empty(), "the program was closed and started again and these switches did not hold: {}", forgotten.join("; "));
    }
}

probe! {
    /// THE START SCREEN CAN BE TURNED OFF, and the next start opens straight into an empty document.
    fn the_start_screen_can_be_turned_off() {
        let mut s = Session::start();
        s.key(Key::Escape);
        open_at(&mut s, "settings-sec-general");
        let show = s.word("settings-show-start");
        s.toggle(&show);
        close_the_settings(&mut s);
        let kept = close_the_program(s);
        let mut s = Session::start_on(qymcad::Machine { kept, ..qymcad::Machine::default() });
        let start_screen = s.word("win-start");
        assert!(!s.shows(&start_screen), "the start screen was turned off and it is there at the next start; on screen: {:?}", s.words());
    }
}

probe! {
    /// THE HEIGHT AN EXTRUSION OFFERS COMES FROM THE SETTINGS: 25 set there, and the block built without touching
    /// the field is 25 tall.
    fn the_height_an_extrusion_offers_comes_from_the_settings() {
        let mut s = Session::start();
        s.key(Key::Escape);
        open_at(&mut s, "settings-sec-part");
        let caption = s.word("settings-default-extrude");
        let field = s.field(&caption);
        s.click(field.rect.center()).chord(Modifiers::COMMAND, Key::A).type_text("25").key(Key::Enter);
        close_the_settings(&mut s);
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        let body = s.document().bodies.iter().find(|b| !b.consumed && !b.sheet).cloned().unwrap_or_else(|| panic!("the extrusion made no body"));
        let tall = body.max[2] - body.min[2];
        assert!((tall - 25.0).abs() < 0.01, "the settings say an extrusion is 25 tall and the block came out {tall}");
    }
}

probe! {
    /// THE SCALE OF THE INTERFACE MAKES THE WORDS BIGGER, and the window keeps it for the next run.
    fn the_scale_of_the_interface_makes_the_words_bigger() {
        let mut s = Session::start();
        s.key(Key::Escape);
        // everything drawn grows, so the window holds fewer points across than it did
        let small = s.screen().x;
        open_at(&mut s, "settings-sec-appearance");
        // the row of the scale carries a minus and a plus; the plus is pressed a few times, as a person does
        let caption = s.word("settings-ui-scale");
        let row = s.find(&caption, qymcad::pos2(700.0, 200.0)).unwrap_or_else(|| panic!("the settings offer no scale of the interface; on screen: {:?}", s.words()));
        for _ in 0..4 {
            let plus = s
                .widgets()
                .into_iter()
                .filter(|w| w.kind == qymcad::Kind::Button && w.label == "+" && w.rect.center().y > row.min.y && w.rect.center().y < row.max.y)
                .min_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x))
                .unwrap_or_else(|| panic!("the row of the scale has no plus to press; on screen: {:?}", s.words()));
            s.click(plus.rect.center());
        }
        close_the_settings(&mut s);
        let big = s.screen().x;
        assert!(big < small * 0.97, "the interface was made larger and the window still holds {big} points across, it held {small}");
        let kept = close_the_program(s);
        let mut s = Session::start_on(qymcad::Machine { kept, ..qymcad::Machine::default() });
        s.key(Key::Escape);
        assert!((s.screen().x - big).abs() < 1.0, "the scale of the interface was not kept for the next run: the window holds {} points across, it held {big}", s.screen().x);
    }
}
