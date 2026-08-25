//! THE HELP SETTINGS AND THE LINK TO THE SITE.
//!
//! Two settings: WHAT LANGUAGE to read in and WHERE to open it. Both break silently — a help language
//! that did not follow the interface language, and an "in the browser" one of the callers forgot
//! about.
#[cfg(test)]
mod tests {
    use super::super::help_window::HelpTarget;
    use super::super::App;
    use crate::help;

    /// BY DEFAULT THE HELP LANGUAGE FOLLOWS THE INTERFACE, and a chosen one overrides it.
    ///
    /// "As in the interface" is not decoration in a list but a state: getting back to it must not
    /// require remembering which code was the original one.
    #[test]
    fn the_help_language_follows_the_interface_until_it_is_chosen() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let prev = crate::i18n::language();
        let mut app = App::default();

        // empty means following the interface, and ON ITS CHANGE too
        app.set.help_lang = String::new();
        app.set.language = "ru".into();
        app.apply_language();
        assert_eq!(help::lang(), "ru", "the help did not follow the Russian interface");
        app.set.language = "en".into();
        app.apply_language();
        assert_eq!(help::lang(), "en", "the interface was switched and the help stayed in the previous language");

        // once one is chosen, the interface no longer rules it
        app.set.help_lang = "ru".into();
        app.apply_language();
        assert_eq!(help::lang(), "ru", "a chosen help language must override the interface language");
        app.set.language = "ru".into();
        app.set.help_lang = "en".into();
        app.apply_language();
        assert_eq!(help::lang(), "en", "English help was chosen in a Russian interface — it must be English");

        // and back to "as in the interface"
        app.set.help_lang = String::new();
        app.apply_language();
        assert_eq!(help::lang(), "ru", "\"as in the interface\" was restored and the help language did not come back");

        crate::i18n::set_language(&prev);
        help::set_lang("");
    }

    /// A CHOSEN LANGUAGE CHANGES NOT ONLY THE TEXT BUT THE TITLES AND THE TABLE OF CONTENTS.
    ///
    /// Otherwise it would be half a job: the article in one language and the list on the left and the
    /// caption above it in another.
    #[test]
    fn choosing_a_language_changes_the_whole_help() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        help::set_lang("ru");
        let ru_title = help::title("part/01-extrude");
        help::set_lang("en");
        let en_title = help::title("part/01-extrude");
        assert_ne!(ru_title, en_title, "the help language was switched and the article title is the same — the title is taken past the language");
        assert!(help::article("part/01-extrude").is_some_and(|t| t.contains("Extrude") || t.contains("extrude")), "the text of the article is not English");
        help::set_lang("");
        crate::i18n::set_language(&prev);
    }

    /// THE WEB ADDRESS POINTS AT THE SAME ARTICLE IN THE SAME LANGUAGE.
    ///
    /// An "open on the site" button that leads to the front page is useless exactly when it is needed:
    /// a person wants to show a colleague THAT particular paragraph.
    #[test]
    fn the_web_address_points_at_the_same_article() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let prev = crate::i18n::language();
        help::set_lang("en");
        let u = help::web_url("part/08-hole");
        assert!(u.ends_with("/en/part/08-hole"), "the address does not lead to the same article in the same language: {u}");
        assert!(u.starts_with("https://"), "the help address must be https: {u}");
        help::set_lang("ru");
        assert!(help::web_url("part/08-hole").ends_with("/ru/part/08-hole"), "the help language did not reach the address");
        help::set_lang("");
        crate::i18n::set_language(&prev);
    }

    /// "OPEN IN THE BROWSER" APPLIES TO EVERY PATH OF OPENING, not only to the button.
    ///
    /// What is checked is THE DECISION, not the launch: a real browser must not be started during a
    /// run. The same means `reveal_command` uses for the file manager.
    #[test]
    fn the_open_in_browser_setting_is_honoured_everywhere() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let mut app = App::default();
        app.set.help_external = false;
        assert_eq!(app.help_target("index"), HelpTarget::Window, "by default the help must open in its own window — it works without the internet");

        app.set.help_external = true;
        assert_eq!(app.help_target("part/08-hole"), HelpTarget::Site(help::web_url("part/08-hole")), "the browser was chosen and the help still aims at the window");

        // and `open_help` OBEYS THAT DECISION: the window did not open, and the status line holds the address
        app.open_help("part/08-hole");
        assert!(!app.win.help, "it is set to open in the browser and the own window opened anyway");
        assert!(app.status.contains("part/08-hole"), "it opened in the browser and the person was not told what went where: \"{}\"", app.status);
    }

    /// THE "OPEN ON THE SITE" BUTTON IS IN THE WINDOW AND KNOWS THE ADDRESS OF THE CURRENT ARTICLE.
    #[test]
    fn the_open_on_site_button_is_in_the_window() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let mut app = App::default();
        app.open_help("assembly/02-joints");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        let label = crate::i18n::tr("help-open-on-site");
        assert!(texts.iter().any(|t| t.contains(&label)), "there is no \"{label}\" button in the help window: {texts:?}");
        let src = include_str!("help_window.rs");
        assert!(src.contains("web_url(&article)"), "the button leads somewhere other than the current article");
    }

    /// AND BOTH SETTINGS REACH THE SETTINGS WINDOW rather than staying a field in the record.
    #[test]
    fn both_help_settings_reach_the_settings_window() {
        let mut app = App::default();
        app.win.settings = true;
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.settings_window(c));
        for k in ["settings-help-lang", "settings-help-open"] {
            let label = crate::i18n::tr(k);
            assert!(texts.iter().any(|t| t.contains(&label)), "the \"{label}\" setting is not in the window: {texts:?}");
        }
        // and the "as in the interface" choice too: without it there is no way back to the default
        let follow = crate::i18n::tr("settings-help-lang-follow");
        assert!(texts.iter().any(|t| t.contains(&follow)), "there is no \"{follow}\" choice");
    }
}
