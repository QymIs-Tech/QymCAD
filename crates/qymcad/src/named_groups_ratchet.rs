//! Ratchet on UNNAMED GROUPS: counters per file that may only go down.
//!
//! The rules of the code say a group of values that travels together is a struct with named fields, two
//! neighbours of one type are not left side by side, and a switch is an enum. Most of the tree was written
//! before the rules; how much of it is left is the table of marks beside this file. Rewriting them is a change of working code in thousands of places, done crate by
//! crate; this guard makes that progress one-way.
//!
//! Three counters per file:
//!
//! * `runs` - functions with two neighbouring parameters of one type (`a: f64, b: f64`): swapped, they
//!   still compile. Named fields of one type are not counted: they are named where the value is built, which
//!   is the cure;
//! * `tuples` - tuple types (`-> (Id, usize)`, `Vec<(String, u8)>`, `struct S(u8, u8)`): the places have
//!   no names and the reader counts commas. A point `(f64, f64)` is the one tuple that says what it holds;
//! * `bools` - parameters of type `bool`: a switch says which way it is set by a named variant.
//!
//! The marks live in `named_groups_marks.txt`, one line per file. A file that grows fails, and the mark is
//! not raised to let it through. A file that shrinks fails too and asks for its mark to be lowered in the
//! same commit, so the slack does not hide new groups. A new file starts at zero.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    /// The counts of one file.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Counts {
        runs: usize,
        tuples: usize,
        bools: usize,
    }

    impl Counts {
        fn is_zero(&self) -> bool {
            *self == Counts::default()
        }

        fn line(&self, path: &str) -> String {
            format!("{path} {} {} {}", self.runs, self.tuples, self.bools)
        }
    }

    /// The text with comments, string and char literals blanked byte for byte, so offsets stay where they were
    /// and a comma or a bracket inside a literal is not read as code.
    fn code_of(text: &str) -> Vec<u8> {
        let mut b = text.as_bytes().to_vec();
        let n = b.len();
        let mut i = 0;
        let blank = |b: &mut Vec<u8>, r: std::ops::Range<usize>| {
            for x in b.iter_mut().take(r.end.min(n)).skip(r.start) {
                if *x != b'\n' {
                    *x = b' ';
                }
            }
        };
        while i < n {
            if b[i..].starts_with(b"//") {
                let end = b[i..].iter().position(|&c| c == b'\n').map_or(n, |k| i + k);
                blank(&mut b, i..end);
                i = end;
            } else if b[i..].starts_with(b"/*") {
                let (mut d, mut j) = (1, i + 2);
                while j < n && d > 0 {
                    if b[j..].starts_with(b"/*") {
                        d += 1;
                        j += 2;
                    } else if b[j..].starts_with(b"*/") {
                        d -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut b, i..j);
                i = j;
            } else if (b[i] == b'r' || (b[i] == b'b' && i + 1 < n && b[i + 1] == b'r')) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && {
                let s = if b[i] == b'b' { i + 2 } else { i + 1 };
                let h = b[s..].iter().take_while(|&&c| c == b'#').count();
                s + h < n && b[s + h] == b'"'
            } {
                let s = if b[i] == b'b' { i + 2 } else { i + 1 };
                let h = b[s..].iter().take_while(|&&c| c == b'#').count();
                let mut close = vec![b'"'];
                close.extend(std::iter::repeat_n(b'#', h));
                let from = s + h + 1;
                let end = (from..n).find(|&k| b[k..].starts_with(&close)).map_or(n, |k| k + close.len());
                blank(&mut b, from..end.saturating_sub(close.len()));
                i = end;
            } else if b[i] == b'"' {
                let mut j = i + 1;
                while j < n && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                blank(&mut b, i + 1..j);
                i = j + 1;
            } else if b[i] == b'\'' {
                // a char literal is 'x', '\n', '\u{..}'; anything else after a quote is a lifetime
                let end = if i + 2 < n && b[i + 1] == b'\\' {
                    (i + 2..n.min(i + 12)).find(|&k| b[k] == b'\'')
                } else {
                    let ch = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                    (i + 1 + ch < n && b[i + 1 + ch] == b'\'').then_some(i + 1 + ch)
                };
                match end {
                    Some(e) => {
                        blank(&mut b, i + 1..e);
                        i = e + 1;
                    }
                    None => i += 1,
                }
            } else {
                i += 1;
            }
        }
        b
    }

    /// The offset just past the bracket that closes the one opened at `open`.
    fn close_of(b: &[u8], open: usize) -> usize {
        let mut d = 0i32;
        let mut prev = b' ';
        for (k, &c) in b.iter().enumerate().skip(open) {
            let arrow = c == b'>' && prev == b'-';
            prev = c;
            match c {
                b'(' | b'[' | b'{' | b'<' => d += 1,
                b'>' if arrow => {}
                b')' | b']' | b'}' | b'>' => {
                    d -= 1;
                    if d == 0 {
                        return k + 1;
                    }
                }
                _ => {}
            }
        }
        b.len()
    }

    /// Splits on the commas at bracket depth zero; the `>` of an arrow closes nothing.
    fn split_top(s: &str) -> Vec<String> {
        let (mut out, mut depth, mut cur, mut prev) = (Vec::new(), 0i32, String::new(), ' ');
        for ch in s.chars() {
            let arrow = ch == '>' && prev == '-';
            prev = ch;
            match ch {
                '<' | '(' | '[' | '{' => depth += 1,
                '>' if arrow => {}
                '>' | ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
            cur.push(ch);
        }
        out.push(cur);
        out.into_iter().map(|x| x.split_whitespace().collect::<String>()).filter(|x| !x.is_empty()).collect()
    }

    /// The type of a parameter or a field: what stands after its first `:` at depth zero, attributes dropped.
    fn type_of(item: &str) -> Option<String> {
        let mut s = item;
        while let Some(rest) = s.strip_prefix("#[") {
            s = &rest[rest.find(']').map_or(rest.len(), |k| k + 1)..];
        }
        let (mut depth, mut prev) = (0i32, ' ');
        for (k, ch) in s.char_indices() {
            match ch {
                '<' | '(' | '[' | '{' => depth += 1,
                '>' if prev == '-' => {}
                '>' | ')' | ']' | '}' => depth -= 1,
                ':' if depth == 0 && !s[k + 1..].starts_with(':') && prev != ':' => return Some(s[k + 1..].to_string()),
                _ => {}
            }
            prev = ch;
        }
        None
    }

    fn word_at(b: &[u8], at: usize, word: &[u8]) -> bool {
        b[at..].starts_with(word) && (at == 0 || !(b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_')) && b.get(at + word.len()).is_some_and(|c| !(c.is_ascii_alphanumeric() || *c == b'_'))
    }

    /// A point `(x, y)`: the type `(f64, f64)` or `(f32, f32)`, or a value of two numbers such as `(40.0, -3.5)`.
    fn is_point(parts: &[String]) -> bool {
        let number = |s: &str| s.trim_start_matches('-').replace('_', "").parse::<f64>().is_ok();
        parts.len() == 2 && ((parts[0] == parts[1] && (parts[0] == "f64" || parts[0] == "f32")) || (number(&parts[0]) && number(&parts[1])))
    }

    /// The ranges of `extern "C"` blocks: their declarations are the C side's, not ours to name.
    fn bridges(raw: &str, b: &[u8]) -> Vec<std::ops::Range<usize>> {
        raw.match_indices("extern \"C\"").filter_map(|(at, _)| b[at..].iter().position(|&c| c == b'{' || c == b';').filter(|&k| b[at + k] == b'{').map(|k| at + k..close_of(b, at + k))).collect()
    }

    fn counts_of(raw: &str) -> Counts {
        let b = code_of(raw);
        let text = String::from_utf8_lossy(&b).into_owned();
        let bridged = bridges(raw, &b);
        let mut c = Counts::default();
        let n = b.len();
        let mut i = 0;
        while i < n {
            if word_at(&b, i, b"fn") && !bridged.iter().any(|r| r.contains(&i)) {
                let mut j = i + 2;
                while j < n && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                if j < n && b[j] == b'<' {
                    j = close_of(&b, j);
                }
                while j < n && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n && b[j] == b'(' {
                    let end = close_of(&b, j);
                    let types: Vec<String> = split_top(&text[j + 1..end - 1]).iter().filter(|p| !p.ends_with("self") && !p.starts_with("self:")).filter_map(|p| type_of(p)).collect();
                    if types.windows(2).any(|w| w[0] == w[1]) {
                        c.runs += 1;
                    }
                    c.bools += types.iter().filter(|t| *t == "bool").count();
                }
            }
            if word_at(&b, i, b"struct") {
                let mut j = i + 6;
                while j < n && !matches!(b[j], b'{' | b'(' | b';') {
                    if b[j] == b'<' {
                        j = close_of(&b, j);
                    } else {
                        j += 1;
                    }
                }
                if j < n && b[j] == b'(' {
                    let end = close_of(&b, j);
                    let parts = split_top(&text[j + 1..end - 1]);
                    if parts.len() >= 2 && !is_point(&parts) {
                        c.tuples += 1;
                    }
                }
            }
            if b[i] == b'(' {
                let before = text[..i].trim_end();
                let typed = before.ends_with("->")
                    || (before.ends_with(':') && !before.ends_with("::"))
                    || before.ends_with('<')
                    || before.ends_with("&[")
                    || before.ends_with("&mut[")
                    || (before.ends_with('[') && !before.ends_with("!["));
                if typed {
                    let end = close_of(&b, i);
                    let parts = split_top(&text[i + 1..end.saturating_sub(1).max(i + 1)]);
                    if parts.len() >= 2 && !is_point(&parts) {
                        c.tuples += 1;
                    }
                }
            }
            i += 1;
        }
        c
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root").to_path_buf()
    }

    /// Every `.rs` file under `crates/` with something to count, by its path from the repository root.
    fn measured() -> BTreeMap<String, Counts> {
        let root = repo_root();
        let mut out = BTreeMap::new();
        let mut stack = vec![root.join("crates")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().and_then(|n| n.to_str()) != Some("target") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let c = counts_of(&std::fs::read_to_string(&p).expect("a source is readable"));
                    if !c.is_zero() {
                        out.insert(p.strip_prefix(&root).expect("under the root").to_string_lossy().replace('\\', "/"), c);
                    }
                }
            }
        }
        out
    }

    fn marks() -> BTreeMap<String, Counts> {
        include_str!("named_groups_marks.txt")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                let num = |k: usize| w.get(k).and_then(|x| x.parse().ok()).unwrap_or_else(|| panic!("a mark is a path and three numbers: {l}"));
                (w[0].to_string(), Counts { runs: num(1), tuples: num(2), bools: num(3) })
            })
            .collect()
    }

    #[test]
    fn the_signal_sees_each_kind_and_spares_a_point() {
        let src = r#"
            fn wide(a: f64, b: f64, on: bool) -> (Id, usize) { let s = "x: (u8, u8)"; 0 }
            fn point(at: (f64, f64)) -> Vec<(String, u8)> { let p: [(f64, f64); 1] = [(40.0, -3.5)]; vec![] }
            fn spared(&self, a: f64, n: usize) {}
            struct Span { from: usize, to: usize }
            struct Pair(u8, u8);
            struct Wrap(u8);
            // fn in_a_comment(a: u8, b: u8, c: (u8, u8)) {}
            unsafe extern "C" { fn qym_c(a: u8, b: u8, on: bool); }
        "#;
        let c = counts_of(src);
        assert_eq!(c.runs, 1, "only `wide` has two neighbours of one type: {c:?}");
        assert_eq!(c.tuples, 3, "`(Id, usize)`, `Vec<(String, u8)>` and `Pair`, not the point or the string: {c:?}");
        assert_eq!(c.bools, 1, "the `bool` of `wide`, not of the C declaration: {c:?}");
    }

    #[test]
    fn unnamed_groups_only_ever_go_down() {
        let now = measured();
        let was = marks();
        let mut faults = Vec::new();
        for (path, c) in &now {
            let m = was.get(path).copied().unwrap_or_default();
            let grew = c.runs > m.runs || c.tuples > m.tuples || c.bools > m.bools;
            if grew {
                faults.push(format!("{path}: {c:?}, the mark is {m:?}. A group that travels together is a struct with named fields, a switch is an enum; the mark is not raised."));
            } else if *c != m {
                faults.push(format!("{path} went down - lower its line in named_groups_marks.txt to:\n    {}", c.line(path)));
            }
        }
        for (path, m) in &was {
            if !now.contains_key(path) {
                faults.push(format!("{path} has nothing left to count (or is gone) - remove its line {}", m.line(path)));
            }
        }
        assert!(faults.is_empty(), "the unnamed groups:\n{}", faults.join("\n"));
    }

    /// The table of marks as it stands, printed for laying the guard down: `cargo test -p qymcad --lib
    /// named_groups_ratchet::tests::print_the_marks -- --ignored --nocapture`.
    #[test]
    #[ignore = "prints the table of marks; run by hand"]
    fn print_the_marks() {
        for (path, c) in measured() {
            println!("{}", c.line(&path));
        }
    }
}
