//! THE COMMAND CATALOGUE AND THE PANEL DO NOT DRIFT APART.
//!
//! The catalogue is what the search needs, but a list inside the search would fall behind the panel
//! at the very first new feature. That has already happened twice and both times was fixed the same
//! way: one rule and a guard that holds the link in BOTH directions. The same here — a button in the
//! panel must have a row in the catalogue, a row in the catalogue must have a button.
//!
//! The same means that holds the help together (`help_map_flow`): the table does not draw the
//! buttons, but the build goes red the moment they diverge.
#[cfg(test)]
mod tests {
    use crate::command_catalog::{Launch, COMMANDS};

    /// The tool numbers that really exist in the interface.
    ///
    /// TWO FILES: the workbench panel lives in `panels.rs`, the common creation panel (datums, sketch)
    /// in `gui.rs`. The help guard was already caught out by this once, missing the datums entirely.
    fn tools_in_ui() -> (Vec<(&'static str, u8)>, Vec<(String, u8)>) {
        let joined = format!("{}\n{}", crate::gui::panels_source::PANELS, include_str!("../gui.rs"));
        let src: &str = &joined;
        let mut feats: Vec<(&'static str, u8)> = Vec::new();
        let mut sk: Vec<(String, u8)> = Vec::new();
        for (pat, tag) in [("start_feat_cmd(", "feat"), ("start_prim_cmd(", "prim")] {
            let mut rest = src;
            while let Some(i) = rest.find(pat) {
                let tail = &rest[i + pat.len()..];
                rest = tail;
                if let Some(end) = tail.find(')') {
                    if let Ok(n) = tail[..end].parse::<u8>() {
                        let key = (tag, n);
                        if !feats.contains(&key) {
                            feats.push(key);
                        }
                    }
                }
            }
        }
        for (pat, handle) in [("set_sk_tool(", "sk"), ("set_dim_tool(", "dim"), ("set_click_op(", "click"), ("modify_button(", "mod")] {
            let mut rest = src;
            while let Some(i) = rest.find(pat) {
                let tail = &rest[i + pat.len()..];
                rest = tail;
                if let Some(end) = tail.find(')') {
                    if let Ok(n) = tail[..end].parse::<u8>() {
                        let key = (handle.to_string(), n);
                        if !sk.contains(&key) {
                            sk.push(key);
                        }
                    }
                }
            }
        }
        (feats, sk)
    }

    /// EVERY BUTTON HAS A CATALOGUE ROW. Otherwise the search will not find it, and a person will be
    /// certain the command does not exist.
    #[test]
    fn every_tool_in_the_ui_is_in_the_catalog() {
        let (feats, sk) = tools_in_ui();
        assert!(feats.len() > 20 && sk.len() > 15, "suspiciously few tools were found: {} and {}", feats.len(), sk.len());
        for (tag, n) in &feats {
            let found = COMMANDS.iter().any(|c| match (tag, c.launch) {
                (&"feat", Launch::Feat(m)) => m == *n,
                (&"prim", Launch::Prim(m)) => m == *n,
                _ => false,
            });
            assert!(found, "{tag}({n}) is in the panel and not in the command catalogue — the search will not find it");
        }
        for (h, n) in &sk {
            let found = COMMANDS.iter().any(|c| match (h.as_str(), c.launch) {
                ("sk", Launch::SkTool(m)) => m == *n,
                ("dim", Launch::Dim(m)) => m == *n,
                ("click", Launch::ClickOp(m)) => m == *n,
                ("mod", Launch::Modify(m)) => m == *n,
                _ => false,
            });
            assert!(found, "{h}({n}) is in the panel and not in the catalogue");
        }
    }

    /// AND THE OTHER WAY ROUND: the catalogue promises nothing the program does not have.
    #[test]
    fn the_catalog_promises_nothing_that_does_not_exist() {
        let (feats, sk) = tools_in_ui();
        for c in COMMANDS {
            let ok = match c.launch {
                Launch::Feat(n) => feats.contains(&("feat", n)),
                Launch::Prim(n) => feats.contains(&("prim", n)),
                Launch::SkTool(n) => sk.contains(&("sk".to_string(), n)),
                Launch::Dim(n) => sk.contains(&("dim".to_string(), n)),
                Launch::ClickOp(n) => sk.contains(&("click".to_string(), n)),
                Launch::Modify(n) => sk.contains(&("mod".to_string(), n)),
                Launch::Action(_) => true, // assembly actions have launches of their own and need no numbers
            };
            assert!(ok, "the catalogue promises \"{}\" and there is no button for it in the interface", c.code);
        }
    }

    /// EVERY COMMAND HAS A NAME IN BOTH LANGUAGES, and it is not a code.
    ///
    /// The name is taken from the title of the help article or from a key of its own. A search row
    /// without a name is an empty line in the list, that is, a command that cannot be found.
    #[test]
    fn every_command_has_a_human_name_in_both_languages() {
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let prev = crate::i18n::language();
        for code in ["ru", "en"] {
            crate::i18n::set_language(code);
            crate::help::set_lang(code);
            for c in COMMANDS {
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
        let _lang = crate::help::lang_guard(); // the help language is shared per process — see `lang_guard`
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        crate::help::set_lang("ru");
        for wb in ["sketch", "part", "assembly"] {
            let names: Vec<String> = COMMANDS.iter().filter(|c| c.workbench == wb).map(|c| c.name()).collect();
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
        app.sel = super::super::Sel::Sketch(0);
        app.run_command("part.extrude");
        assert_eq!(app.cmd.kind, 1, "\"part.extrude\" did not open the extrude");
        app.cancel_all_tools();
        app.run_command("part.hole");
        assert_eq!(app.cmd.kind, 7, "\"part.hole\" did not open the hole");
    }
}
