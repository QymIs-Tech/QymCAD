//! A SOURCE GUARD MUST NOT BE ROOTED AT ITS OWN CRATE.
//!
//! The disease this forbids has now been paid for twice. The first time, the localisation counter looked
//! at the interface crate and reported zero while Russian captions were still on the screen - the words
//! sat in the kernel, which it could not see. The second time is recorded as D20: five rules walked
//! `CARGO_MANIFEST_DIR/src`, which honestly meant "all the working code there is" while the application
//! was one crate and quietly stopped meaning it when the workbenches moved out. Behind that blind spot lay
//! a raw arrow drawn on the screen as a square, six catalogue keys with no words, and a drawing file no
//! list read.
//!
//! Neither time did anything go red. That is the whole difficulty: a walk rooted too narrowly does not
//! fail, it just finds fewer files, and a guard finding fewer files reports success. So the FORM is
//! forbidden rather than its consequences chased - the third recurrence would be found the same way as the
//! first two, which is to say by accident.
//!
//! The way to walk every crate is `qymcad_i18n::ratchet::every_crate_src()`, which asserts its own
//! coverage.

/// The one place a walk may name a single crate's `src`, and why.
///
/// Keyed by the path from `crates/`: an exception written as a bare file name would excuse every namesake,
/// and each workbench crate is a single `lib.rs`.
const ALLOWED: [(&str, &str); 1] = [(
    "qymcad/src/god_object_ratchet.rs",
    "it addresses ONE NAMED FILE, `src/gui.rs`, because the god object being measured is that file - not a \
     kind of code that could live anywhere else",
)];

#[test]
fn no_source_guard_is_rooted_at_its_own_crate() {
    let crates = qymcad_i18n::ratchet::crates_root();
    let mut rooted: Vec<String> = Vec::new();
    let mut looked = 0usize;
    // `src` AND `tests`: a rule moved into `tests/` is the same rule, and the mistake travels with it.
    let mut stack: Vec<std::path::PathBuf> = std::fs::read_dir(&crates)
        .expect("the crates read")
        .flatten()
        .flat_map(|e| [e.path().join("src"), e.path().join("tests")])
        .filter(|p| p.is_dir())
        .collect();
    assert!(stack.len() > 2, "this guard would be the first to catch itself: it found {} directories", stack.len());
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            looked += 1;
            let name = p.strip_prefix(&crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if name == "qymcad-i18n/tests/a_guard_walks_the_whole_tree.rs" || ALLOWED.iter().any(|(n, _)| *n == name) {
                continue; // this file names the form in order to forbid it
            }
            for (i, line) in std::fs::read_to_string(&p).unwrap_or_default().lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue; // a comment explaining the mistake is not the mistake
                }
                if line.contains("CARGO_MANIFEST_DIR") && line.contains(".join(\"src") {
                    rooted.push(format!("{name}:{}: {}", i + 1, line.trim()));
                }
            }
        }
    }
    assert!(looked > 100, "the sweep read {looked} files, which is too few to have looked at the tree");
    assert!(
        rooted.is_empty(),
        "a guard walks the `src` of the crate it lives in, so it reports full coverage of a shrinking part \
         of the tree and never goes red about it. Use `qymcad_i18n::ratchet::every_crate_src()`, or name the \
         file in ALLOWED with a reason ({}):\n{}",
        rooted.len(),
        rooted.join("\n")
    );
}
