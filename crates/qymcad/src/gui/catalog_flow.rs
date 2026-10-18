//! THE COMMAND CATALOGUE AND THE HELP DO NOT DRIFT APART.
//!
//! The bars used to write their buttons one by one in the code, and this guard held them to the rows of the
//! catalogue by reading the source in both directions. The bars are drawn from the rows now, so a button without a
//! row cannot be written. What is left to hold: every command the help knows has a row, every row has an article and
//! a name a person reads, and launching by code leads into the command the button starts.
#[cfg(test)]
mod tests {
    use crate::command_catalog::{all, Launch, Named};

    /// EVERY COMMAND THE HELP KNOWS HAS A ROW, so a button on its bar and a line in the search. The tables of the
    /// help name the commands by their kind and number; a number without a row is a command a person cannot reach.
    #[test]
    fn every_command_the_help_knows_is_in_the_catalogue() {
        let mut missing = Vec::new();
        for (kind, _) in qymcad_help::map::PART {
            if !all().any(|c| matches!(c.launch, Launch::Feat(k) | Launch::Prim(k) if k == *kind)) {
                missing.push(format!("the part kind {kind}"));
            }
        }
        for (handle, n, _) in qymcad_help::map::SKETCH {
            let found = all().any(|c| match c.launch {
                Launch::SkTool(m) => *handle == "sk" && m == *n,
                Launch::Dim(m) => *handle == "dim" && m == *n,
                Launch::ClickOp(m) => *handle == "click" && m == *n,
                Launch::Modify(t) => *handle == "mod" && t.code() == *n,
                _ => false,
            });
            if !found {
                missing.push(format!("the sketch tool {handle} {n}"));
            }
        }
        assert!(missing.is_empty(), "the help knows these commands and the catalogue has no row for them: {missing:?}");
    }

    /// EVERY ROW HAS AN ARTICLE: F1 on its button and the help of the search lead somewhere.
    #[test]
    fn every_command_has_an_article() {
        let orphans: Vec<&str> = all().filter(|c| c.help_article().is_none()).map(|c| c.code).collect();
        assert!(orphans.is_empty(), "these commands have no help article: {orphans:?}");
    }

    /// CODES DO NOT REPEAT: a key, a setting of the bar and the search name a command by its code.
    #[test]
    fn codes_do_not_repeat() {
        let mut codes: Vec<&str> = all().map(|c| c.code).collect();
        let n = codes.len();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), n, "two rows of the catalogue share a code");
    }

    /// EVERY COMMAND HAS A NAME IN BOTH LANGUAGES, and it is not a code.
    ///
    /// The name is taken from the title of the help article or from a key of its own. A search row
    /// without a name is an empty line in the list, that is, a command that cannot be found.
    #[test]
    fn every_command_has_a_human_name_in_both_languages() {
        let prev = crate::i18n::language();
        for code in ["ru", "en"] {
            crate::i18n::set_language(code);
            crate::help::set_lang(code);
            for c in all() {
                let n = c.name();
                assert!(!n.trim().is_empty(), "{code}: the command \"{}\" has no name", c.code);
                assert_ne!(n, c.code, "{code}: the name of command \"{}\" is its own code, so there is neither an article nor a key of its own", c.code);
                assert!(!n.starts_with("cmdname-"), "{code}: the name of \"{}\" turned out to be a catalogue key: \"{n}\"", c.code);
            }
        }
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
    }

    /// NAMES DO NOT REPEAT WITHIN A WORKBENCH.
    ///
    /// Six primitives share one article, and without names of their own the search would show six
    /// rows all reading "Primitives" — there is no choosing between them.
    #[test]
    fn names_are_unique_within_a_workbench() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        for wb in ["sketch", "part", "assembly"] {
            let names: Vec<String> = all().filter(|c| c.home() == wb).map(|c| c.name()).collect();
            let mut uniq = names.clone();
            uniq.sort();
            uniq.dedup();
            assert_eq!(uniq.len(), names.len(), "the \"{wb}\" workbench has identical command names: {names:?}");
        }
        crate::i18n::set_language(&prev);
        crate::help::set_lang("");
    }

    /// LAUNCHING FROM THE CATALOGUE LEADS INTO THE SAME COMMAND the button does.
    #[test]
    fn running_a_command_by_code_starts_it() {
        let mut app = super::super::screen_keys::tests::plate();
        app.chosen.sel = super::super::Sel::Sketch(0);
        app.run_command("part.extrude", &egui::Context::default());
        assert_eq!(app.tools.armed.cmd_kind(), 1, "\"part.extrude\" did not open the extrude");
        app.cancel_all_tools();
        app.run_command("part.hole", &egui::Context::default());
        assert_eq!(app.tools.armed.cmd_kind(), 7, "\"part.hole\" did not open the hole");
    }
}
