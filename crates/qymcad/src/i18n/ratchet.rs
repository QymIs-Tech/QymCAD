//! THE LOCALISATION RATCHET: a counter of Russian strings in the code that can only GO DOWN.
//!
//! IT COUNTS THE WHOLE WORKSPACE, not the application alone. The first version looked at the
//! interface crate — and showed zero while Russian captions were still visible in an English build:
//! the words sat in the KERNEL (kinds of joint, thread standards, materials) and in the bridge to OCCT
//! (error texts). The kernel is a library and has no language: it must give out CODES, and the
//! application picks the words. While the counter did not see the kernel, that rule was held by
//! nothing.
//!
//! The language framework stands, and the interface hardly goes through it: there are more than two
//! thousand strings typed straight into the code. They cannot be translated in one sitting — that is
//! weeks, and every edit would have to be checked for somebody falling back into the old habit. That
//! was already caught once: a colour-scheme screen was written in Russian text with the catalogue
//! alive.
//!
//! So what is here is not "we will finish it some day" but a MECHANISM. The test counts the Russian
//! literals in the working code and compares them against a ceiling. There is one rule:
//!
//! - **more than before means a red test.** A new screen must go through the catalogue from the start,
//!   like everything already translated;
//! - **fewer than before means a red test too**, and asks for the ceiling to be lowered. Otherwise
//!   slack piles up silently, and one day a whole screen hides under it again.
//!
//! The ceiling in [`CEILING`] is not "this many mistakes are allowed", it is a MARK of how far the
//! work has come. It moves only downwards and only along with the translation.
#[cfg(test)]
pub(crate) mod tests {
    /// How many Russian literals are left in the working code of the WHOLE WORKSPACE.
    ///
    /// THE PATH OF THE MARK: 2085 -> 385 (a fix to the counting: the file was cut at the first
    /// `#[cfg(test)]`, and that also stands on test facades IN THE MIDDLE of the code — half of
    /// `gui.rs` and `sketching.rs` was not checked at all) -> 0.
    ///
    /// ZERO MEANS EXACTLY ONE THING: not one string a person sees is typed in the code. The words live
    /// in the catalogues, the kernel and the bridges give out CODES. Comments in an .nc program written
    /// in Latin are not an exception to the rule but a different requirement: they are read by the
    /// machine controller, not by a window.
    const CEILING: usize = 0;

    /// The files that are counted: the working code of the application. Tests are not counted — they
    /// describe behaviour and are read by whoever works on the code.
    pub(crate) fn is_working_code(path: &std::path::Path) -> bool {
        let n = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        // `src/` ONLY: the integration tests in `tests/` describe behaviour and are read by whoever
        // works on the code.
        let in_src = path.components().any(|c| c.as_os_str() == "src");
        in_src
            && path.extension().is_some_and(|x| x == "rs")
            && n != "tests.rs"
            && n != "ratchet.rs"
            && !n.ends_with("_tests.rs")
            && !n.ends_with("_flow.rs")
            && !n.ends_with("_memory.rs")
            && !["audit.rs", "fuzz.rs", "sketch_paint.rs", "sketch_reopen.rs", "frame_cost.rs", "delete_feature_view.rs", "view_state.rs", "props_readonly.rs", "one_extrude.rs", "sketch_ref.rs"].contains(&n.as_str())
    }

    /// THE WORKING PART OF A FILE — up to the test MODULE rather than up to the first `#[cfg(test)]`.
    ///
    /// Cutting at the first occurrence was a mistake: `#[cfg(test)]` also stands on test facades in the
    /// middle of the code. Because of that the counter silently did not see half of `gui.rs` and
    /// `sketching.rs` — and showed zero where Russian captions were visible on screen. A test module is
    /// recognised by `mod` immediately after the attribute.
    ///
    /// A VISIBILITY BEFORE `mod` IS A TEST MODULE TOO. A fixture used by neighbouring modules has to be
    /// opened up (`pub(in crate::gui) mod tests`), and the counter stumbled on that: the file stopped
    /// being cut at all, its tests went into the count of Russian strings, and the build reddened where
    /// not one line of working code had changed.
    pub(crate) fn working_part(text: &str) -> &str {
        for nl in ["\n", "\r\n"] {
            let attr = format!("#[cfg(test)]{nl}");
            let mut from = 0usize;
            while let Some(rel) = text[from..].find(&attr) {
                let at = from + rel;
                let rest = &text[at + attr.len()..];
                // `pub`, `pub(crate)`, `pub(in crate::gui)` — the visibility is stripped whole, together
                // with the bracket
                let after_vis = match rest.strip_prefix("pub") {
                    Some(r) => {
                        let r = if r.starts_with('(') { r.find(')').map(|i| &r[i + 1..]).unwrap_or(r) } else { r };
                        r.trim_start()
                    }
                    None => rest,
                };
                if after_vis.starts_with("mod ") {
                    return &text[..at];
                }
                from = at + attr.len();
            }
        }
        text
    }

    /// The Russian literals in a file — WITHOUT the comments and without the test part.
    ///
    /// Comments are not interface text: counting them would mean punishing explanations. The test part
    /// inside a working file does not count either.
    fn russian_literals(text: &str) -> usize {
        let code = working_part(text);
        let mut n = 0;
        for line in code.lines() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            // A DEVELOPER LOG IS NOT THE INTERFACE. `eprintln!` goes to the terminal rather than to a
            // window; it is read by whoever is fixing the program.
            if t.starts_with("eprintln!") || t.starts_with("println!") {
                continue;
            }
            // THE TEXT OF A PANIC IS NOT THE INTERFACE EITHER. `expect` and `panic!` are printed to the
            // terminal when the program is already crashing; they are read by whoever is fixing the
            // code. A person never sees them.
            if line.contains(".expect(") || line.contains("panic!(") || line.contains("unreachable!(") || line.contains("debug_assert") {
                continue;
            }
            let mut rest = line;
            while let Some(a) = rest.find('"') {
                let after = &rest[a + 1..];
                let Some(b) = after.find('"') else { break };
                let s = &after[..b];
                if s.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c) || c == 'ё' || c == 'Ё') {
                    n += 1;
                }
                rest = &after[b + 1..];
            }
        }
        n
    }

    fn count_all() -> (usize, Vec<(String, usize)>) {
        // the root of the workspace: .../crates/qymcad -> .../crates
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the directory of the crates").to_path_buf();
        let mut total = 0;
        let mut per: Vec<(String, usize)> = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if !is_working_code(&p) {
                    continue;
                }
                let n = russian_literals(&std::fs::read_to_string(&p).expect("the file reads"));
                if n > 0 {
                    per.push((p.strip_prefix(&root).unwrap_or(&p).display().to_string(), n));
                    total += n;
                }
            }
        }
        per.sort_by(|a, b| b.1.cmp(&a.1));
        (total, per)
    }

    /// THE RATCHET: the strings in the code never grow in number, and when they shrink the mark comes
    /// down.
    #[test]
    fn untranslated_strings_only_ever_go_down() {
        let (total, per) = count_all();
        let top: Vec<String> = per.iter().take(8).map(|(f, n)| format!("  {n:5}  {f}")).collect();
        assert!(
            total <= CEILING,
            "there are MORE Russian strings in the code: {total} against the mark of {CEILING}.\n\
             New interface must go through the language catalogue from the start — otherwise the translation never ends.\n\
             Where most of them are:\n{}",
            top.join("\n")
        );
        assert!(
            total >= CEILING,
            "{} strings were translated — lower the CEILING mark to {total} in the same commit, otherwise slack piles up silently\n\
             and one day a whole screen hides under it again.\nWhere most of them are left:\n{}",
            CEILING - total,
            top.join("\n")
        );
    }

    /// THE HOTKEY REFERENCE IS FULLY TRANSLATED — the first area closed completely.
    ///
    /// As a check of its own rather than "inside the general counter": a closed area must stay closed
    /// even while the general ceiling is still high.
    #[test]
    fn the_hotkey_reference_is_fully_translated() {
        let src = include_str!("../gui/hotkeys.rs");
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        assert_eq!(russian_literals(code), 0, "the hotkey reference must go through the catalogue in its entirety");
    }
}
