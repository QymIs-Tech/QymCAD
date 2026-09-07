//! Guard on the WIDTH OF SIGNATURES: no function outside the kernel bridge may take more than seven
//! arguments.
//!
//! Seven is where clippy's own `too_many_arguments` sits, and the reason is not taste. A signature of
//! ten reads as a list of commas: to tell `dy2` from `dz2` one has to count places, and swapping two
//! neighbours of the same type compiles and is silently wrong. Every one of them was cured the same
//! way - by NAMING the group that travelled together, never by a type alias, which hides the fault
//! instead of removing it.
//!
//! THE ONE EXEMPTION IS WHAT THE C SIDE DECLARES, and it is recognised by its SHAPE: a function inside
//! an `extern "C"` block IS the declaration of a C function. Its argument list is not ours to choose -
//! the C++ that answers it lives in another repository and changes only together with it.
//!
//! IT USED TO BE WIDER, and no longer needs to be. The trait `Kernel`, its implementations and the mock
//! were exempt too, on the grounds that one signature is written in three places and narrows only all at
//! once. That was true and it was also an excuse: all three were narrowed together, operation by
//! operation, and the trait went from 14 wide signatures out of 59 to none. The wrappers over the C
//! calls went with them - `thread` took sixteen arguments and now takes two.
//!
//! Measured on closing, over the whole tree and with nothing exempt at all: twelve signatures wider than
//! seven remain, and all twelve are `extern "C"` declarations. Not one is a function written in Rust.

#[cfg(test)]
pub(crate) mod tests {
    /// Splits an argument list on the commas that are at bracket depth zero.
    ///
    /// Counting commas is not enough: `&([f64; 3], [f64; 3], [f64; 3])` is ONE argument, and a counter
    /// that misses that reported 128 where there were 63. The earlier measure did exactly this.
    fn split_args(t: &str) -> Vec<&str> {
        let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0usize);
        for (i, ch) in t.char_indices() {
            match ch {
                '<' | '(' | '[' | '{' => depth += 1,
                '>' | ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(t[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        out.push(t[start..].trim());
        out.into_iter().filter(|a| !a.is_empty()).collect()
    }

    /// Whether byte offset `at` falls inside an `extern "C"` block.
    ///
    /// The block is found by its opening line and closed by the first `}` at that line's indentation,
    /// which is how every block in this repository is laid out.
    fn inside_kernel_bridge(text: &str, at: usize) -> bool {
        let mut off = 0usize;
        let mut open: Option<usize> = None; // indentation of the block we are in
        for line in text.lines() {
            let end = off + line.len() + 1;
            let indent = line.len() - line.trim_start().len();
            if let Some(want) = open {
                if line.trim_start().starts_with('}') && indent == want {
                    open = None;
                }
            } else {
                let t = line.trim_start();
                let is_bridge = t.starts_with("extern \"C\"") || t.starts_with("unsafe extern \"C\"");
                if is_bridge {
                    open = Some(indent);
                }
            }
            if open.is_some() && at >= off && at < end {
                return true;
            }
            if at < end {
                return open.is_some();
            }
            off = end;
        }
        false
    }

    /// Every `fn` in the file whose argument list is longer than seven: its name, its width, the offset
    /// of the name.
    fn wide_fns(text: &str) -> Vec<(String, usize, usize)> {
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0usize;
        while let Some(k) = text[i..].find("fn ") {
            let s = i + k;
            // a real `fn` keyword starts a word
            let before_ok = s == 0 || !bytes[s - 1].is_ascii_alphanumeric() && bytes[s - 1] != b'_';
            i = s + 3;
            if !before_ok {
                continue;
            }
            let Some(paren) = text[s..].find('(') else { break };
            let name = text[s + 3..s + paren].trim();
            if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let open = s + paren;
            let mut depth = 0i32;
            let mut close = None;
            for (j, ch) in text[open..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + j);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { break };
            let n = split_args(&text[open + 1..close]).len();
            if n > 7 {
                out.push((name.to_string(), n, s));
            }
        }
        out
    }

    /// A function belongs to the C side when it sits inside an `extern "C"` block.
    ///
    /// A wrapper AROUND such a call is not exempt: it is Rust, and its arguments are ours to group. That
    /// used to be excused too, through "reaches a `qym_` symbol", and the excuse hid `thread` with its
    /// sixteen arguments.
    fn is_bridge(text: &str, at: usize) -> bool {
        inside_kernel_bridge(text, at)
    }

    /// THE SIGNAL ITSELF, CHECKED ON A SAMPLE OF THE SHAPE IT IS FOR.
    ///
    /// Over an already clean tree the sweep below is green whether it works or not, so the detector is
    /// shown a source holding both cases: a wide declaration inside `extern "C"`, which is exempt, and a
    /// Rust wrapper AROUND it that is just as wide - and is not.
    #[test]
    fn the_signal_catches_a_wide_signature_and_spares_the_c_declarations() {
        let sample = concat!(
            "extern \"C\" {\n",
            "    fn qym_shape_hole(s: u8, a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8) -> u8;\n",
            "}\n\n",
            "fn wraps_it(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8, h: u8) -> u8 {\n    unsafe { qym_shape_hole(a, b, c, d, e, f, g, h) }\n}\n",
        );
        let found = wide_fns(sample);
        assert_eq!(found.len(), 2, "both wide signatures must be seen: {found:?}");
        let exempt: Vec<&str> = found.iter().filter(|(_, _, at)| is_bridge(sample, *at)).map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(
            exempt,
            ["qym_shape_hole"],
            "only the C declaration is exempt; a Rust wrapper around it is ours to narrow, and pretending otherwise \
             is what hid a signature of sixteen"
        );
        let narrow = "fn small(a: u8, b: u8) -> u8 {\n    0\n}\n";
        assert!(wide_fns(narrow).is_empty(), "a signature of two must not be reported");
    }

    /// The sweep: nothing outside the bridge takes more than seven.
    #[test]
    fn nothing_written_in_rust_takes_more_than_seven_arguments() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root").join("crates");
        let mut offenders: Vec<String> = Vec::new();
        let (mut files, mut exempt) = (0usize, 0usize);
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().and_then(|n| n.to_str()) != Some("target") {
                        stack.push(p);
                    }
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                    continue;
                }
                // THIS FILE ITSELF holds the sample the detector is tried on: wide signatures written on
                // purpose, inside a string. They are not code and must not be reported as such.
                if p.file_name().and_then(|n| n.to_str()) == Some("wide_signature_ratchet.rs") {
                    continue;
                }
                files += 1;
                let text = std::fs::read_to_string(&p).expect("file is readable");
                for (name, n, at) in wide_fns(&text) {
                    if is_bridge(&text, at) {
                        exempt += 1;
                        continue;
                    }
                    let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
                    offenders.push(format!("{rel}: {name} takes {n}"));
                }
            }
        }
        offenders.sort();
        assert!(
            offenders.is_empty(),
            "a signature wider than seven arguments came back. Name the group that travels together and pass it as one record - \
             there are records ready for most of them (BodyView, Editing, Screen, Profile, BodyOp, HoleTool and the rest). \
             Scanned {files} files, {exempt} exempt as `extern \"C\"` declarations.\n{}",
            offenders.join("\n")
        );
    }
}
