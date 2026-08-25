//! THE INTERFACE HOLDS NO RAW UNICODE — ONLY WORDS AND `ph::*` ICONS. Written from a report.
//!
//! Reported behaviour: in one category caption there is a square, an unreadable character. The rule
//! has been repeated for half a year and is written down in the project, but it rested on
//! attentiveness — and was broken.
//!
//! THE FIRST VERSION OF THIS TEST WAS WORSE THAN USELESS. It asked the font whether a glyph existed
//! and was told "it does" — because the character is intercepted by the Phosphor icon font inserted at
//! the head of the family. "A glyph exists" and "what was meant will be drawn" are different claims,
//! and such a test gave false comfort exactly where a person saw a square. It was deleted.
//!
//! What is checked is THE RULE rather than its consequence: there are no arrows, mathematical signs,
//! geometric shapes or dingbats in the interface strings. Neither in the catalogue nor in the code.
#[cfg(test)]
mod tests {
    /// Every string of every catalogue: (language, key, value).
    fn catalogue_strings() -> Vec<(String, String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("i18n");
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "ftl") {
                    continue;
                }
                let lang = p.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                for line in std::fs::read_to_string(&p).expect("the catalogue reads").lines() {
                    let Some((k, v)) = line.split_once(" = ") else { continue };
                    if !k.starts_with(|c: char| c.is_ascii_lowercase()) || k.contains(' ') {
                        continue;
                    }
                    out.push((lang.clone(), k.to_string(), v.to_string()));
                }
            }
        }
        out
    }

    /// THE CATALOGUE HOLDS WORDS, NOT SYMBOLS.
    ///
    /// The earlier check DID NOT CATCH the arrow in a caption, even though a person saw a square in its
    /// place. The reason is instructive: a glyph for it DOES exist in the set — it is intercepted by
    /// the Phosphor icon font inserted at the head of the family. "A glyph exists" and "what was meant
    /// will be drawn" are different claims, and the first does not prove the second.
    ///
    /// So the rule is checked directly: the catalogue is no place for characters from the arrow,
    /// mathematics, geometric shape and dingbat blocks. An icon is placed IN THE CODE and only from
    /// `ph::*` — then the font that actually contains it is the one answering for it.
    ///
    /// The blocks that "symbols" get into captions from: arrows, mathematics, shapes, dingbats.
    fn banned(c: char) -> bool {
        let u = c as u32;
        (0x2190..=0x21FF).contains(&u) // arrows
            || (0x2200..=0x22FF).contains(&u) // mathematical operators
            || (0x2300..=0x23FF).contains(&u) // miscellaneous technical
            || (0x25A0..=0x25FF).contains(&u) // geometric shapes
            || (0x2600..=0x27BF).contains(&u) // symbols and dingbats
    }

    #[test]
    fn the_catalogue_holds_words_not_symbols() {
        let mut bad: Vec<String> = Vec::new();
        for (lang, key, value) in catalogue_strings() {
            for ch in value.chars().filter(|c| banned(*c)) {
                bad.push(format!("{lang}/{key}: \"{ch}\" (U+{:04X}) in \"{value}\"", ch as u32));
            }
        }
        assert!(
            bad.is_empty(),
            "the catalogue holds a symbol instead of a word ({}):\n{}\n\
             Icons are placed IN THE CODE and only from `ph::*`: a raw character is drawn by whichever font\n\
             claimed it first, and that is almost never the one that was meant.",
            bad.len(),
            bad.join("\n")
        );
    }

    /// AND IN THE CODE TOO. The catalogue is not the only place a string reaches the screen from.
    ///
    /// Captions assembled in the code (`format!`, literals in buttons) go past the catalogue, and a ban
    /// checked only against the catalogue does not touch them.
    #[test]
    fn the_ui_code_holds_words_not_symbols() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut bad: Vec<String> = Vec::new();
        let mut stack = vec![src_dir.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let n = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                // "working code" is decided BY THE SAME rule the localisation ratchet uses: test files
                // describe behaviour, and there is nobody to draw an arrow inside a description.
                if !super::super::super::i18n::ratchet::tests::is_working_code(&p) || n == "font_coverage.rs" {
                    continue;
                }
                let text = std::fs::read_to_string(&p).expect("the file reads");
                let code = super::super::super::i18n::ratchet::tests::working_part(&text);
                for (i, line) in code.lines().enumerate() {
                    let t = line.trim_start();
                    if t.starts_with("//") {
                        continue; // in a comment an arrow explains a thought rather than being drawn
                    }
                    let mut rest = line;
                    while let Some(a) = rest.find('"') {
                        let after = &rest[a + 1..];
                        let Some(b) = after.find('"') else { break };
                        for ch in after[..b].chars().filter(|c| banned(*c)) {
                            bad.push(format!("{n}:{}: \"{ch}\" (U+{:04X})", i + 1, ch as u32));
                        }
                        rest = &after[b + 1..];
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "an interface string holds a symbol instead of a word or a `ph::*` icon ({}):\n{}",
            bad.len(),
            bad.join("\n")
        );
    }

    /// AND IN THE HELP TOO — it is drawn by the same window and the same font.
    ///
    /// The ban was checked against the catalogue and the code, while the help articles
    /// (`docs/help/**.md`) are baked into the binary by the same `include_dir` and shown by
    /// `help_window` through ordinary `ui.label` — that is, they obey exactly the same rule. The
    /// measurement this guard exists for: 34 arrows across 14 articles, and on screen a person saw a
    /// square in their place.
    ///
    /// CODE BLOCKS ARE NOT JUDGED: there a character is shown AS THE TEXT OF AN EXAMPLE rather than
    /// drawn as a caption.
    #[test]
    fn the_help_holds_words_not_symbols() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("docs/help");
        let mut bad: Vec<String> = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the help reads").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).expect("the article reads");
                let name = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().into_owned();
                let mut in_code = false;
                for (i, line) in text.lines().enumerate() {
                    if line.trim_start().starts_with("```") {
                        in_code = !in_code;
                        continue;
                    }
                    if in_code {
                        continue;
                    }
                    for ch in line.chars().filter(|c| banned(*c)) {
                        bad.push(format!("{name}:{}: \"{ch}\" (U+{:04X}) in \"{}\"", i + 1, ch as u32, line.trim()));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "the help holds a symbol instead of a word ({}):\n{}", bad.len(), bad.join("\n"));
    }
}
