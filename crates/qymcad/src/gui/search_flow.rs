//! THE COMMAND SEARCH WORKS.
//!
//! Forty icons in two scrolling columns amount to "I know it exists but I do not remember where".
//! Grown-up CAD is saved by exactly this: press, type three letters, apply. Checked here is that the
//! search really searches, really launches, and does not introduce a launch path of its own that goes
//! round the button.
//!
//! The Russian queries below stay in Cyrillic: they are SEARCH KEYS into the Russian catalogue, and the
//! whole point of those checks is that a Russian substring finds the command.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The scene: inside a part, so that the workbench is the Part.
    fn in_part() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component_for_test(part);
        app
    }

    /// IT FINDS A COMMAND BY ITS NAME IN THE LANGUAGE OF THE INTERFACE.
    #[test]
    fn it_finds_a_command_by_its_name() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        let app = in_part();
        let hits = app.command_search_hits("скругл");
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
        assert!(hits.iter().any(|c| c.code == "part.fillet"), "the Russian prefix did not find the fillet: {:?}", hits.iter().map(|c| c.code).collect::<Vec<_>>());
    }

    /// AND BY THE ENGLISH NAME TOO — in a Russian interface.
    ///
    /// Not politeness to foreigners: a person remembers `fillet` from other people's manuals and
    /// videos, because that is what they learned from. A search that does not understand this sends
    /// them back to scrolling the panel.
    #[test]
    fn it_finds_by_the_english_name_in_a_russian_interface() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        let app = in_part();
        let hits = app.command_search_hits("fillet");
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
        assert!(hits.iter().any(|c| c.code == "part.fillet"), "an English name is not searchable in a Russian interface: {:?}", hits.iter().map(|c| c.code).collect::<Vec<_>>());
    }

    /// AND A COMMAND WITHOUT A KEY OF ITS OWN IS SEARCHABLE IN THE OTHER LANGUAGE.
    ///
    /// The first edition looked at the second language only for commands with a name key OF THEIR OWN,
    /// while for the rest the name comes from the article title — and `fillet` was not found in
    /// English. It showed on a snapshot exactly: a two-letter prefix found one row instead of three.
    #[test]
    fn a_command_named_by_its_help_article_is_searchable_in_both_languages() {
        let fillet = crate::command_catalog::by_code("part.fillet").expect("the fillet in the catalogue");
        assert!(fillet.name_key.is_empty(), "the scene relies on a command WITHOUT a key of its own");
        assert!(fillet.name_in("ru").to_lowercase().contains("скругл"), "the Russian name is not taken from the article: \"{}\"", fillet.name_in("ru"));
        assert!(fillet.name_in("en").to_lowercase().contains("fillet"), "the English name is not taken from the article: \"{}\"", fillet.name_in("en"));
    }

    /// THE COMMANDS OF THE CURRENT WORKBENCH COME FIRST.
    ///
    /// A person searches for what they are busy with. A command of another workbench is legitimate (it
    /// will switch the workbench), but it must not push aside one of your own.
    #[test]
    fn commands_of_the_current_workbench_come_first() {
        let app = in_part();
        let hits = app.command_search_hits("mirror");
        let codes: Vec<&str> = hits.iter().map(|c| c.code).collect();
        assert!(codes.len() >= 2, "\"mirror\" should find both the part one and the sketch one: {codes:?}");
        assert_eq!(codes.first(), Some(&"part.mirror"), "in the Part workbench its own command should come first: {codes:?}");
    }

    /// AN EMPTY QUERY SHOWS NOTHING.
    ///
    /// Otherwise the window opens straight into a sheet of fifty rows, and the first impression is
    /// "everything at once" rather than "ask for what you need".
    #[test]
    fn an_empty_query_shows_nothing() {
        let app = in_part();
        assert!(app.command_search_hits("").is_empty(), "an empty query produced a list");
        assert!(app.command_search_hits("   ").is_empty(), "spaces are an empty query too");
    }

    /// THE LAUNCH GOES THROUGH THE SHARED DOOR rather than by a path of its own.
    ///
    /// A guard over the source: the search must call `run_command` rather than pull `start_feat_cmd`
    /// itself. A second launch path will sooner or later start doing something other than the
    /// button.
    #[test]
    fn the_search_launches_through_the_shared_door() {
        let src = include_str!("command_search.rs");
        assert!(src.contains("self.run_command(code)"), "the search launches commands past the shared door");
        for own in ["start_feat_cmd(", "set_sk_tool(", "start_prim_cmd("] {
            assert!(!src.contains(own), "the search introduced a launch path of its own: {own}");
        }
    }

    /// THE WINDOW OPENS AND CLOSES.
    #[test]
    fn the_window_opens_and_closes() {
        let mut app = in_part();
        assert!(!app.command_search_open_for_test(), "the search must not be open right away");
        app.toggle_command_search();
        assert!(app.command_search_open_for_test(), "the search did not open");
        app.toggle_command_search();
        assert!(!app.command_search_open_for_test(), "the search did not close");
    }

    /// AND IT REACHES THE SCREEN — with the rows of results.
    #[test]
    fn the_window_shows_the_hits() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        let mut app = in_part();
        app.toggle_command_search();
        app.set_command_search_query_for_test("отверст");
        let texts = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.command_search_window(c));
        let want = crate::command_catalog::by_code("part.hole").expect("the hole in the catalogue").name();
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
        assert!(texts.iter().any(|t| t.contains(&want)), "the search window holds no \"{want}\" row: {texts:?}");
    }

    /// TWO WAYS IN, AND THE SECOND WORKS WHILE TYPING.
    ///
    /// A space in a field must be typed (`60 + 2`), so it opens the search only when the keyboard is
    /// free. Ctrl+K always works: otherwise the search is unavailable exactly when the hand is already
    /// on the keyboard.
    #[test]
    fn there_are_two_ways_in_and_one_works_while_typing() {
        let src = include_str!("input.rs");
        assert!(src.contains("i.modifiers.command && i.key_pressed(egui::Key::K)"), "Ctrl+K does not open the search");
        assert!(src.contains("!typing_now && !i.modifiers.any() && i.key_pressed(egui::Key::Space)"), "a space opens the search even from a field — there it must be typed");
        // AND THE KEYBOARD STATE IS ASKED OUTSIDE `ctx.input`: inside it is a deadlock, caught by a
        // full run (one at a time the tests passed, together they hung dead).
        assert!(!src.contains("|| (!ctx.egui_wants_keyboard_input()"), "`wants_keyboard_input` is called inside `ctx.input` again — that is a deadlock");
    }
}
