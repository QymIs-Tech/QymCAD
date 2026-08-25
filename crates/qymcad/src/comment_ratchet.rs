//! Ratchet for source comments: counters that may only go down.
//!
//! The project is being opened to the public. Comments were written as a conversation with the
//! customer — first person, quoted complaints, emotions — and they name third-party CAD products.
//! Both are unacceptable in a public repository: the first is unreadable for an outsider, the second
//! is a needless legal exposure.
//!
//! Rewriting 27 000 comment lines takes many sessions, so progress needs to be measurable and
//! irreversible. Same mechanism as [`crate::i18n::ratchet`]: a ceiling that only moves down.
//!
//! Four counters, because the problems differ in urgency:
//!
//! * `PRODUCT_CEILING` — third-party product names. Small set, cleared first, must stay at zero.
//! * `VOICE_CEILING` — first/second person and quoted complaints. Zero once a file is rewritten.
//! * `CYRILLIC_CEILING` — comment lines still in Russian. Drops as translation proceeds.
//! * `LITERAL_CEILING` — string literals still in Russian: assertion and panic messages. They are
//!   read by whoever repairs the code, so they are as public as the comments around them.
//!
//! A counter that grows fails the build. A counter that shrinks also fails the build and asks for the
//! ceiling to be lowered — otherwise slack accumulates silently and a whole file hides under it.
#[cfg(test)]
pub(crate) mod tests {
    /// Third-party CAD products named in comments.
    ///
    /// The target is zero: naming a competitor buys nothing and risks a claim. It is not zero yet
    /// because each mention is rewritten together with the block that holds it — a separate pass over
    /// the same lines would mean editing them twice. Publication waits for this counter to reach zero.
    const PRODUCT_CEILING: usize = 0;

    /// Comment lines written as a conversation: first person, "the user said", quoted complaints.
    const VOICE_CEILING: usize = 0;

    /// Comment lines still in Russian. Target is zero — the public language of the code is English.
    const CYRILLIC_CEILING: usize = 0;

    /// String literals still in Russian: assertion messages, `expect` texts, panic texts.
    ///
    /// Interface strings are not counted here — those live in the localisation catalogue and are held
    /// at zero by [`crate::i18n::ratchet`]. What is left is the text a developer reads when a check
    /// fails, and it has to be readable by the same audience as the code.
    ///
    /// The remainder is DATA rather than messages, and that is why the target is this number and not
    /// zero: the Russian words a lint looks for in the help text, driver names in another alphabet that
    /// prove a formula accepts them, the character ranges `('а'..='я')` the guards themselves are built
    /// from, search keys in other people's documents, language names and file names on disk.
    const LITERAL_CEILING: usize = 93;

    /// Files that carry comments: every source of the repository, tests included.
    ///
    /// NOT only Rust. The kernel bridge is C++ and the post-processors are scripts; their comments are
    /// read by the same people and are just as public. While this counted `.rs` alone, a whole 4900-line
    /// C++ file sat outside the count with a thousand Russian comment lines in it, and the counters
    /// reported zero.
    ///
    /// Tests are counted too, unlike the localisation ratchet: their comments explain behaviour to a
    /// reader and are just as public as the rest.
    fn is_source(path: &std::path::Path) -> bool {
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        matches!(ext, "rs" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "rhai") && path.file_name().is_none_or(|n| n != "comment_ratchet.rs")
    }

    /// The comment part of a source line: everything from the first `//` that is not inside a string
    /// literal. `None` when the line carries no comment.
    ///
    /// A comment does not have to start the line. Counting only lines that begin with `//` missed every
    /// trailing note — `let x = 1; // note` — and C++ is written that way throughout.
    fn comment_of(line: &str) -> Option<&str> {
        let b = line.as_bytes();
        let (mut i, mut in_str) = (0usize, false);
        while i < b.len() {
            match b[i] {
                b'\\' if in_str => i += 1,
                b'"' => in_str = !in_str,
                b'/' if !in_str && b.get(i + 1) == Some(&b'/') => return Some(&line[i..]),
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Comment lines of a file: every line that carries a comment, the comment part alone.
    fn comment_lines(text: &str) -> Vec<&str> {
        text.lines().filter_map(comment_of).collect()
    }

    /// Russian string literals on a source line. The comment part is cut off first — what a comment
    /// quotes is a comment, and it is counted by the other three.
    fn russian_literals(line: &str) -> usize {
        let line = match comment_of(line) {
            Some(c) => &line[..line.len() - c.len()],
            None => line,
        };
        let (mut n, mut rest) = (0, line);
        while let Some(a) = rest.find('"') {
            let after = &rest[a + 1..];
            let Some(b) = after.find('"') else { break };
            if has_cyrillic(&after[..b]) {
                n += 1;
            }
            rest = &after[b + 1..];
        }
        // A LITERAL DOES NOT HAVE TO FIT ON ONE LINE. Counting only text between a pair of quotes on the
        // same line missed every multi-line string: the setup sheet sat in one for the whole translation,
        // eleven Russian captions of a document a person reads, and the counter said zero. Any Russian
        // outside a comment counts, whether or not the quotes happen to close on this line.
        if n == 0 && has_cyrillic(line) {
            n = 1;
        }
        n
    }

    fn has_cyrillic(s: &str) -> bool {
        s.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c) || c == 'ё' || c == 'Ё')
    }

    /// Names of third-party CAD systems in any spelling seen in this tree.
    fn names_product(s: &str) -> bool {
        const NAMES: &[&str] = &["onshape", "fusion", "solidworks", "solid works", "компас", "kompas", "freecad", "inventor", "catia", "creo", "solidedge", "solid edge"];
        let low = s.to_lowercase();
        NAMES.iter().any(|n| low.contains(n))
    }

    /// Marks of a comment addressed to a person instead of describing the code.
    ///
    /// Whole words only: `я` matches the pronoun, not the middle of `имя`.
    fn speaks_to_a_person(s: &str) -> bool {
        const WORDS: &[&str] = &["юзер", "юзера", "юзеру", "я", "мне", "меня", "мной", "мой", "моя", "моё", "мои", "моего", "мы", "нам", "нас", "наш", "наша", "наше", "наши", "тебе", "тебя", "твой", "вчера", "сегодня"];
        let low = s.to_lowercase();
        let words: Vec<&str> = low.split(|c: char| !c.is_alphabetic()).collect();
        if WORDS.iter().any(|w| words.contains(w)) {
            return true;
        }
        // "пользователь сказал/прислал/поймал/показал/жаловался" — a report of a conversation
        const REPORTED: &[&str] = &["сказал", "прислал", "поймал", "показал", "жалов", "пожалов", "просил", "требовал"];
        low.contains("пользовател") && REPORTED.iter().any(|w| low.contains(w))
    }

    /// Walk the workspace and count all four.
    fn count() -> (usize, usize, usize, usize, Vec<(String, usize, usize, usize)>) {
        // THE WHOLE REPOSITORY, not just `crates`: the post-processor scripts live beside it and carry
        // comments too. Build output and the git store are skipped — they are neither sources nor ours.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root").to_path_buf();
        let (mut prod, mut voice, mut cyr, mut lit) = (0, 0, 0, 0);
        let mut per: Vec<(String, usize, usize, usize)> = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    let skip = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| matches!(n, "target" | ".git"));
                    if !skip {
                        stack.push(p);
                    }
                    continue;
                }
                if !is_source(&p) {
                    continue;
                }
                let text = std::fs::read_to_string(&p).expect("file is readable");
                lit += text.lines().map(russian_literals).sum::<usize>();
                let (mut fp, mut fv, mut fc) = (0, 0, 0);
                for line in comment_lines(&text) {
                    if names_product(line) {
                        fp += 1;
                    }
                    if speaks_to_a_person(line) {
                        fv += 1;
                    }
                    if has_cyrillic(line) {
                        fc += 1;
                    }
                }
                prod += fp;
                voice += fv;
                cyr += fc;
                if fp + fv + fc > 0 {
                    per.push((p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().into_owned(), fp, fv, fc));
                }
            }
        }
        per.sort_by_key(|(_, _, _, c)| std::cmp::Reverse(*c));
        (prod, voice, cyr, lit, per)
    }

    /// All four counters only ever go down.
    #[test]
    fn comments_only_ever_get_cleaner() {
        let (prod, voice, cyr, lit, per) = count();
        let worst: Vec<String> = per.iter().take(10).map(|(f, p, v, c)| format!("  {c:5} cyrillic, {v:4} voice, {p:3} product  {f}")).collect();
        let report = format!("worst files:\n{}", worst.join("\n"));

        assert!(
            prod <= PRODUCT_CEILING,
            "comments name third-party products: {prod} lines, ceiling {PRODUCT_CEILING}.\n\
             Naming a competitor buys nothing and risks a claim — state the engineering rule instead.\n{report}"
        );
        assert!(
            voice <= VOICE_CEILING,
            "comments talk to a person instead of describing the code: {voice} lines, ceiling {VOICE_CEILING}.\n{report}"
        );
        assert!(
            cyr <= CYRILLIC_CEILING,
            "comments still in Russian: {cyr} lines, ceiling {CYRILLIC_CEILING}.\n{report}"
        );
        assert!(lit <= LITERAL_CEILING, "assertion and panic texts still in Russian: {lit} literals, ceiling {LITERAL_CEILING}");

        // Slack is as bad as growth: under it a whole rewritten file hides, and the next regression
        // lands unnoticed. This holds for the product counter most of all — publication waits on it
        // reaching zero, so it is the one counter whose remaining distance has to stay exact.
        assert_eq!(prod, PRODUCT_CEILING, "product ceiling is stale: {prod} lines left, set PRODUCT_CEILING to that");
        assert_eq!(voice, VOICE_CEILING, "voice ceiling is stale: {voice} lines left, set VOICE_CEILING to that");
        assert_eq!(cyr, CYRILLIC_CEILING, "cyrillic ceiling is stale: {cyr} lines left, set CYRILLIC_CEILING to that");
        assert_eq!(lit, LITERAL_CEILING, "literal ceiling is stale: {lit} literals left, set LITERAL_CEILING to that");
    }
}
