//! THE ICON THEMES: selecting themes, activating in cascade, fallback chain, live reloading on file changes,
//! and keeping theme preferences across sessions.
use qymcad::{Key, Machine, Session};
use qymcad_acceptance::probe;
use std::time::Duration;

/// Open the settings at the section `key` names.
fn open_at(s: &mut Session, key: &str) {
    let (windows, settings) = (s.word("menu-windows"), s.word("menu-settings"));
    if !s.shows(&s.word("win-settings")) {
        s.menu(&[&windows, &settings]);
    }
    let section = s.word(key);
    s.press_word_near(&section, qymcad::pos2(700.0, 200.0));
}

/// Close the settings window.
fn close_the_settings(s: &mut Session) {
    let title = s.word("win-settings");
    s.close_window(&title);
}

/// Close the icon theme manager window.
fn close_the_manager(s: &mut Session) {
    let title = s.word("icontheme-mgr-title");
    s.close_window(&title);
}

/// Close the program and answer what it asks, keeping settings between runs.
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

probe! {
    /// THE ICON THEME SELECTION, CASCADE FALLBACK, LIVE RELOAD AND SETTINGS PERSISTENCE.
    fn icon_theme_user_journey_with_live_reload_and_persistence() {
        let mut s = Session::start();
        s.key(Key::Escape);

        let user_themes = s.same_machine().home.join("icon_themes");
        let pack_dir = user_themes.join("acceptance-dev-pack");
        let icons_dir = pack_dir.join("icons").join("sketch");
        std::fs::create_dir_all(&icons_dir).expect("create icons directory");

        let manifest = r#"(
            id: "acceptance-dev-pack",
            name: "Acceptance Dev Theme",
            version: "1.0.0",
            author: "Tester",
            license: "MIT",
        )"#;
        std::fs::write(pack_dir.join("manifest.ron"), manifest).expect("write manifest.ron");

        let line_svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/></svg>"#;
        std::fs::write(icons_dir.join("line.svg"), line_svg).expect("write line.svg");

        open_at(&mut s, "settings-sec-appearance");
        let themes_title = s.word("settings-icon-themes-title");
        assert!(s.shows(&themes_title), "appearance settings must show icon themes title; on screen: {:?}", s.words());

        let open_btn = s.word("settings-open-icon-manager");
        s.press_word(&open_btn);
        let mgr_title = s.word("icontheme-mgr-title");
        assert!(s.shows(&mgr_title), "icon theme manager window must open; on screen: {:?}", s.words());

        assert!(s.shows("Acceptance Dev Theme"), "manager must list Acceptance Dev Theme in available themes; on screen: {:?}", s.words());

        let act_btn = s.word("icontheme-mgr-activate-btn");
        let card_loc = s.find("Acceptance Dev Theme", qymcad::pos2(200.0, 400.0)).expect("find Acceptance Dev Theme card");
        s.press_word_near(&act_btn, card_loc.center());

        assert!(s.shows("Acceptance Dev Theme"), "Acceptance Dev Theme must now be in the active cascade");

        close_the_manager(&mut s);
        close_the_settings(&mut s);

        std::thread::sleep(Duration::from_millis(300));
        let line_svg_v2 = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect width="24" height="24"/></svg>"#;
        std::fs::write(icons_dir.join("line.svg"), line_svg_v2).expect("update line.svg");
        s.pause(Duration::from_millis(350));

        let kept = close_the_program(s);

        let mut s2 = Session::start_on(Machine { kept, ..Machine::default() });
        s2.key(Key::Escape);
        open_at(&mut s2, "settings-sec-appearance");
        assert!(s2.shows("Acceptance Dev Theme"), "active icon theme cascade must be preserved across sessions; on screen: {:?}", s2.words());
        close_the_settings(&mut s2);

        let _ = close_the_program(s2);
        let _ = std::fs::remove_dir_all(&pack_dir);
    }
}
