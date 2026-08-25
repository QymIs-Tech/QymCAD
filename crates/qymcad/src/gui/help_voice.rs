//! THE HELP IS WRITTEN FOR WHOEVER WORKS IN THE PROGRAM, not for whoever makes it.
//!
//! Reported after reading the article about the keys: why should people using the CAD need to know
//! what is assembled from where, or that something is not laziness? Help texts must be written for
//! people who use the CAD, not as work reports.
//!
//! That is right, and the miss is systemic: the articles were written in the same voice a work report
//! is written in — defending decisions ("that is not laziness", "more honest than"), exposing innards
//! ("stored by a persistent kernel name") and telling the history of the development ("that was
//! exactly the stumble").
//!
//! A person at their part needs something else: what to press, what will happen, and what to do if it
//! did not work.
//!
//! WHAT THIS GUARD CANNOT DO. It will not check the tone as a whole — that is work for eyes. It
//! catches TWO things a machine tells apart reliably: the developer's vocabulary (the list below is
//! collected from real findings) and the structural damage that comes of appending — a repeated
//! section, languages that have drifted apart. Both times the article was already bad, and both times
//! nobody saw it for years.
#[cfg(test)]
mod tests {
    /// Every help article: the path and the text.
    fn articles() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help");
        let mut out = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the help directory") {
                let p = e.expect("the directory entry").path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n == "img") {
                        continue;
                    }
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "md") {
                    let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
                    out.push((rel, std::fs::read_to_string(&p).expect("the article reads")));
                }
            }
        }
        assert!(out.len() > 50, "only {} articles were found — the check checked nothing", out.len());
        out.sort();
        out
    }

    /// THE DEVELOPER'S VOCABULARY IN THE HELP.
    ///
    /// Every row of the list comes from a real article rather than being invented just in case. On the
    /// left is what stood there, on the right why it is bad for a reader.
    ///
    /// The Russian entries stay in Cyrillic: they are SEARCH KEYS into the Russian articles, and a
    /// translation would find nothing. The help itself is bilingual and stays as it is.
    #[test]
    fn the_help_does_not_talk_like_a_developer() {
        // (the forbidden thing, why it is bad)
        //
        // SUBSTRINGS, NOT WORDS — and that has already gone wrong: one short word was found inside a
        // longer one that was perfectly fine. So the short entries keep a separating space or a
        // continuation; the list is short, and keeping it exact is cheaper than building a word
        // parser.
        const BANNED: &[(&str, &str)] = &[
            ("ядра", "the kernel is a word from inside the program; what matters is the result, not who computes it"),
            ("ядро ", "the same"),
            ("персистент", "an internal detail of how references are stored"),
            ("якобиан", "solver mathematics"),
            ("ранг я", "the same"),
            (" сторож", "our tests are no concern of the reader"),
            ("это не лень", "defending a decision instead of explaining the work"),
            ("честнее, чем", "the same — arguing with an opponent who is not there"),
            ("не поломка, а", "the same"),
            ("не побочный эффект", "the same"),
            ("собран из того же", "a story about how the help itself is built"),
            ("это была та самая", "development history"),
            ("kernel", "the kernel is an internal; the reader needs the result"),
            ("persistent name", "internal storage detail"),
            ("jacobian", "solver mathematics"),
            ("that is not laziness", "defending a decision instead of explaining the work"),
            ("more honest than", "same"),
            ("not a fault but", "same"),
            ("not a side effect", "same"),
            ("built from the same place", "a story about how the help itself is made"),
            ("that was exactly the stumble", "development history"),
        ];
        let mut sins: Vec<String> = Vec::new();
        for (path, text) in articles() {
            let low = text.to_lowercase();
            for (word, why) in BANNED {
                if low.contains(word) {
                    let line = text.lines().find(|l| l.to_lowercase().contains(word)).unwrap_or("");
                    sins.push(format!("{path}: \"{word}\" — {why}\n    {}", line.trim()));
                }
            }
        }
        assert!(sins.is_empty(), "the help speaks in a developer's voice ({}):\n{}", sins.len(), sins.join("\n"));
    }

    /// ONE SECTION APPEARS ONCE.
    ///
    /// The help was extended by appending, and in five articles a section turned out to be written
    /// TWICE: one heading ran as two nearly identical paragraphs in a row, another as the same list in
    /// different words. To a reader that says one thing: nobody reread the text.
    #[test]
    fn no_article_repeats_a_section() {
        let mut sins: Vec<String> = Vec::new();
        for (path, text) in articles() {
            let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
            for i in 0..heads.len() {
                if heads[i + 1..].contains(&heads[i]) {
                    sins.push(format!("{path}: the section \"{}\" is written twice", heads[i].trim_start_matches("## ")));
                }
            }
        }
        assert!(sins.is_empty(), "an article repeats itself ({}):\n{}", sins.len(), sins.join("\n"));
    }

    /// AND THE LANGUAGES DO NOT DRIFT APART.
    ///
    /// A section appended to one article and forgotten in the other is the quietest way to break the
    /// help: each version looks whole on its own. The NUMBER of sections is compared rather than the
    /// text: a translation may sound different, but it must tell of the same things.
    #[test]
    fn both_languages_tell_the_same_story() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help");
        let mut sins: Vec<String> = Vec::new();
        for (path, text) in articles() {
            let Some(rest) = path.strip_prefix("ru/") else { continue };
            let en = root.join("en").join(rest);
            let Ok(other) = std::fs::read_to_string(&en) else {
                sins.push(format!("{path}: there is no English version at all"));
                continue;
            };
            let count = |s: &str| s.lines().filter(|l| l.starts_with("## ")).count();
            if count(&text) != count(&other) {
                sins.push(format!("{path}: {} sections in Russian, {} in English", count(&text), count(&other)));
            }
        }
        assert!(sins.is_empty(), "the languages of the help have drifted apart ({}):\n{}", sins.len(), sins.join("\n"));
    }
}
