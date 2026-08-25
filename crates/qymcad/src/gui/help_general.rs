//! THE "GENERAL" SECTION OF THE HELP AND ITS TIE TO THE PROGRAM.
//!
//! Articles about tools are held by the `help_map_flow` guard: a command has a row in the table, a row
//! has a file. The cross-cutting articles — settings, hotkeys, the tree, the timeline — have no such
//! table and cannot have one: they describe not a command but the way things are built. So they will
//! rot even more quietly: a setting is added, a section renamed, a key introduced — and the article
//! goes on describing last year's program with nobody seeing it.
//!
//! Hence guards of a different kind here: they check the articles against THE VERY SOURCES the
//! interface itself is assembled from — the list of settings sections and the hotkey reference. Not
//! against a text saying how it ought to be, but against the code.
#[cfg(test)]
mod tests {
    use super::super::hotkeys::{AREAS, HOTKEYS};
    use super::super::settings_sections::SettingsSection;
    use crate::help;

    /// The languages the help is written in.
    fn langs() -> Vec<String> {
        help::languages()
    }

    /// The text of an article IN A PARTICULAR LANGUAGE.
    ///
    /// Past `help::article`, which falls back to English: every translation is checked separately here,
    /// and with a fallback a missing translation would pass the guard on the strength of the English
    /// one.
    fn article_in(lang: &str, path: &str) -> String {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help").join(lang).join(format!("{path}.md"));
        std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("there is no article {lang}/{path}: {e}"))
    }

    /// EVERY SETTINGS SECTION IS NAMED IN THE SETTINGS ARTICLE — and in the language of that article.
    ///
    /// There are seven sections, and a person looks for theirs by eye, by the caption they see in the
    /// window. Adding an eighth and not mentioning it means leaving an article that confidently lists
    /// an incomplete set.
    #[test]
    fn every_settings_section_is_named_in_the_settings_article() {
        for l in langs() {
            let md = article_in(&l, "general/07-settings");
            for sec in SettingsSection::all() {
                let label = crate::i18n::tr_in(&l, sec.key()).unwrap_or_else(|| panic!("there is no caption {} in language {l}", sec.key()));
                assert!(
                    md.contains(&label),
                    "the settings section \"{label}\" is in the window, and the help article ({l}) says not a word about it — the list will be read as complete"
                );
            }
        }
    }

    /// EVERY KEY FROM THE REFERENCE IS NAMED IN THE ARTICLE — again in every language.
    ///
    /// The search is for `` `E` `` — exactly how keys are written in the articles. Searching by the
    /// bare letter is not allowed: "E" occurs in every other English word, and the guard would go green
    /// all by itself.
    ///
    /// Keys of the form `Ctrl+Z / Ctrl+Y` are TWO keys in one row of the reference; the article has the
    /// right to name them separately, so the split is on the slash.
    #[test]
    fn every_hotkey_is_named_in_the_hotkeys_article() {
        for l in langs() {
            let md = article_in(&l, "general/10-hotkeys");
            for r in HOTKEYS {
                for piece in r.key.split('/') {
                    let k = format!("`{}`", piece.trim());
                    assert!(md.contains(&k), "the key {k} ({}) works in the program, and the help article ({l}) does not have it", r.action);
                }
            }
        }
    }

    /// THE FOCUS RULE IS STATED WHEREVER IT WILL BE NEEDED: in the hotkey reference and in the article.
    ///
    /// Half the rule ("with no focus, the bare letter") is obvious and works by itself; the other half
    /// ("inside a field, Alt") is invisible: U is pressed in the length field, nothing happens, and the
    /// conclusion drawn is about the program. A remark in the code of `input.rs` will tell nobody that.
    ///
    /// The guard holds THREE ends: the code really does look at `modifiers.alt`, the reference window
    /// prints the remark, the article repeats it. Throw Alt out of the handler and the first end turns
    /// red; forget the text and the second or the third does.
    #[test]
    fn the_alt_rule_is_stated_wherever_it_is_needed() {
        let input = include_str!("input.rs");
        let from = input.find("fn handle_tool_hotkeys").expect("the key handler is in place");
        let to = input[from..].find("\n    /// ").map(|i| from + i).unwrap_or(input.len());
        assert!(input[from..to].contains("modifiers.alt"), "the Alt rule is declared in the help, and the key handler knows nothing about Alt");

        for l in langs() {
            let note = crate::i18n::tr_in(&l, "hotkeys-alt-note").unwrap_or_else(|| panic!("there is no remark about Alt in language {l}"));
            assert!(note.contains("Alt"), "the remark ({l}) must name the key itself: {note}");
            assert!(
                include_str!("hotkeys.rs").contains("hotkeys-alt-note"),
                "the remark is in the language catalogue, and the reference window does not print it — the rule stayed a secret"
            );
            let md = article_in(&l, "general/10-hotkeys");
            assert!(md.contains("Alt+U"), "the article ({l}) must show the rule BY EXAMPLE: Alt+U instead of U");
        }
    }

    /// AND THE OTHER WAY ROUND FOR THE AREAS: all four areas of the reference are covered by the
    /// article.
    ///
    /// Without this the article could list every key of the Part and stay silent about the Assembly —
    /// each key on its own would be found (the letters repeat across workbenches), while a whole
    /// workbench would be missing from the text.
    #[test]
    fn every_hotkey_area_is_covered_by_the_article() {
        for l in langs() {
            let md = article_in(&l, "general/10-hotkeys");
            for area in AREAS {
                // the caption of the area is taken from where the reference window takes it
                let label = crate::i18n::tr_in(&l, &format!("hotkeys-area-{area}")).unwrap_or_else(|| panic!("there is no caption for area {area} in language {l}"));
                assert!(md.to_lowercase().contains(&label.to_lowercase()), "the key area \"{label}\" is in the reference and the article ({l}) does not have it");
            }
        }
    }

    /// THE MACHINING SECTION HIDES TOGETHER WITH THE MODULE — in the table of contents and in the
    /// search.
    ///
    /// While the box is unticked, none of the innards of CAM are to be visible, and the help is no
    /// exception: a section in the contents would tell of a module the program does not have. The search
    /// is checked apart from the contents: it walks the files by a path of its own, and mending one
    /// while forgetting the other is exactly the case that later gets caught by hand.
    #[test]
    fn the_machining_section_hides_with_the_module() {
        let listed = |cam: bool| -> Vec<String> { help::sections(cam).into_iter().flat_map(|(_, v)| v).collect() };
        let off = listed(false);
        assert!(!off.iter().any(|a| a.starts_with("cam/")), "the machining module is off and its section is in the contents of the help: {off:?}");
        let on = listed(true);
        assert!(on.iter().any(|a| a.starts_with("cam/")), "the module is on and the machining section is not in the contents — the guard is checking emptiness");
        // AND THE SEARCH. The word is one that occurs in the article in BOTH languages: `search`
        // looks in the current interface language, which is shared across the tests and is moved about
        // by the neighbours.
        let word = "CAM";
        assert!(help::search(word, true).iter().any(|a| a.starts_with("cam/")), "with the module on, the search does not find the machining article by the word \"{word}\"");
        assert!(!help::search(word, false).iter().any(|a| a.starts_with("cam/")), "the module is off and the search still returns the machining article");
    }

    /// AND AN OPEN MACHINING ARTICLE DOES NOT OUTLIVE THE SWITCHING OFF OF THE MODULE.
    ///
    /// Hiding the section in the contents is not enough: the window remembers where it stood. Read
    /// about machining, untick the box — the section is gone from the contents while the text stays on
    /// the screen.
    #[test]
    fn an_open_machining_article_goes_away_with_the_module() {
        let _lang = crate::help::lang_guard(); // the help language is shared across the process — see `lang_guard`
        // THE LANGUAGE IS PINNED FOR THE DURATION OF THE CHECK. It is shared across the process and the
        // neighbouring tests move it about: the window drew the article in one language while `title`
        // was taken in another, and the test turned red from the ORDER of the run rather than from a
        // breakage. That is all the nastier to catch because on its own it is green.
        let (prev_ui, prev_help) = (crate::i18n::language(), help::picked_lang());
        crate::i18n::set_language("ru");
        help::set_lang("ru");
        let mut app = super::super::App::default();
        app.set_cam_tab_for_test(true);
        app.open_help("cam/index");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        let cam_title = help::title("cam/index");
        assert!(texts.iter().any(|t| t.contains(&cam_title)), "with the module on, the machining article does not open: {texts:?}");

        app.set_cam_tab_for_test(false);
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
        let body = help::article("cam/index").expect("the machining article");
        let line = body.lines().find(|l| l.len() > 40 && !l.starts_with('#')).expect("a paragraph of the article");
        let word = line.split_whitespace().find(|w| w.chars().count() > 8).expect("a long word from the article").trim_matches(|c: char| !c.is_alphanumeric());
        crate::i18n::set_language(&prev_ui);
        help::set_lang(&prev_help);
        assert!(!texts.iter().any(|t| t.contains(word)), "the module was switched off and the machining article stayed on the screen (the word \"{word}\")");
    }

    /// THE "GENERAL" SECTION COVERS EVERYTHING CROSS-CUTTING that was promised. The list is short and
    /// deliberately nailed down: this is not "some number of articles" but by name the topics without
    /// which the help describes the tools but not the program.
    #[test]
    fn the_general_section_covers_everything_promised() {
        for a in [
            "general/01-window",
            "general/02-tree",
            "general/03-timeline",
            "general/04-datums",
            "general/05-parameters",
            "general/06-import-export",
            "general/07-settings",
            "general/08-documents",
            "general/09-viewport",
            "general/10-hotkeys",
        ] {
            for l in langs() {
                let md = article_in(&l, a);
                assert!(md.starts_with("# "), "the article {l}/{a} has no heading — its path will stand in the contents");
                assert!(md.len() > 400, "the article {l}/{a} is too short ({} bytes) to explain anything", md.len());
            }
        }
    }
}
