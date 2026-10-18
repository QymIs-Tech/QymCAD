//! Guard on the SIZE OF A SOURCE FILE: no `.rs` file under `crates/` grows past 1500 lines.
//!
//! A file of 15886 lines (the library of qymcad-ui-state) was read by searching, never from the top: a
//! reader who needs one record walks past the other two hundred. 31 files stood over 1500 lines when
//! they were split by what they hold, each part a module of its own with the code moved as it was.
//! Three stay over the limit, and each for a reason the split cannot remove:
//!
//! * `gui.rs` holds `impl App`, whose blocks the god-object guard counts; a block may move whole but is
//!   not cut in two, and the size of the object is paid down by moving code out of it, not by spreading
//!   it over files;
//! * `gui/user_case.rs` is ONE chain of actions checked after every step: cut into files, the chain
//!   would no longer read as the path a person takes;
//! * `gui/help_images.rs` is one generator of the pictures of the help, run as one piece.
//!
//! These may only shrink. A mark is lowered in the same commit that shrinks its file, and a file that
//! comes down to the limit leaves the list.

#[cfg(test)]
mod tests {
    /// Above this many lines a new file is split by what it holds.
    const LIMIT: usize = 1500;

    /// A file allowed over the limit, and the most lines it may have.
    struct Known {
        path: &'static str,
        lines: usize,
    }

    const KNOWN: &[Known] = &[
        Known { path: "crates/qymcad/src/gui/user_case.rs", lines: 3141 },
        Known { path: "crates/qymcad/src/gui.rs", lines: 2413 },
        Known { path: "crates/qymcad/src/gui/help_images.rs", lines: 1713 },
    ];

    /// A file over the limit: where it is and how many lines it has.
    #[derive(Debug)]
    struct Long {
        path: String,
        lines: usize,
    }

    fn lines_of(text: &str) -> usize {
        text.lines().count()
    }

    /// Every `.rs` file under `crates/` with more than `LIMIT` lines, by its path from the repository root.
    fn long_files(root: &std::path::Path) -> Vec<Long> {
        let mut out = Vec::new();
        let mut stack = vec![root.join("crates")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().and_then(|n| n.to_str()) != Some("target") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let lines = lines_of(&std::fs::read_to_string(&p).expect("a source is readable"));
                    if lines > LIMIT {
                        let path = p.strip_prefix(root).expect("under the root").to_string_lossy().replace('\\', "/");
                        out.push(Long { path, lines });
                    }
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    #[test]
    fn the_signal_counts_lines_as_the_editor_shows_them() {
        assert_eq!(lines_of("a\nb\nc\n"), 3, "a final newline opens no line");
        assert_eq!(lines_of("a\nb\nc"), 3, "a last line without a newline is still a line");
        assert_eq!(lines_of(&"x\n".repeat(LIMIT + 1)), LIMIT + 1);
    }

    #[test]
    fn no_source_file_grows_past_the_limit() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("repository root");
        let long = long_files(root);
        let mut faults = Vec::new();
        for f in &long {
            match KNOWN.iter().find(|k| k.path == f.path) {
                None => faults.push(format!("{} has {} lines, past {LIMIT}. Split it into modules by what it holds, moving the code as it is.", f.path, f.lines)),
                Some(k) if f.lines > k.lines => faults.push(format!("{} grew from {} to {} lines. Move out what does not belong to it; the mark is not raised.", f.path, k.lines, f.lines)),
                Some(k) if f.lines < k.lines => faults.push(format!("{} shrank from {} to {} lines. Lower its mark in KNOWN to {} in the same commit.", f.path, k.lines, f.lines, f.lines)),
                Some(_) => {}
            }
        }
        for k in KNOWN {
            if !long.iter().any(|f| f.path == k.path) {
                faults.push(format!("{} is no longer past {LIMIT} lines (or is gone). Take it off KNOWN.", k.path));
            }
        }
        assert!(faults.is_empty(), "the size of the source files:\n{}", faults.join("\n"));
    }
}
