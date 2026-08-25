//! WHICH BUILD IS THIS.
//!
//! A complaint arrives as a screenshot and a sentence, and the first question is always the same: what
//! was it built from. With a build a day "version 0.1.0" answers nothing — every build of the week
//! carries that number — so what identifies a binary here is the commit it was made at.
//!
//! The values are stamped by `build.rs` at compile time and read back through `option_env!`: a build
//! with no git at hand (an unpacked source archive) still compiles and simply says less. Nothing here
//! reads the disk or runs a process at runtime.

/// The version from the manifest — the same number for every build made between two releases.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The short hash of the commit the binary was built at. `None` when there was no git.
pub fn commit() -> Option<&'static str> {
    option_env!("QYMCAD_GIT_HASH")
}

/// The date of that commit, `YYYY-MM-DD`. The COMMIT date, not the date of the build: two people
/// building the same commit on different days must get the same line.
pub fn commit_date() -> Option<&'static str> {
    option_env!("QYMCAD_COMMIT_DATE")
}

/// Was the tree carrying uncommitted edits when this was built.
///
/// THIS MATTERS AS MUCH AS THE HASH. A binary built over uncommitted edits corresponds to no commit at
/// all; chasing a report from it against the tree of that hash sends whoever reads the report looking
/// for a defect in code that was never built.
pub fn is_modified() -> bool {
    option_env!("QYMCAD_GIT_DIRTY").is_some()
}

/// The one line a person copies into a report: `0.1.0 (a8f629971, 2026-08-25)`.
///
/// The mark about uncommitted edits comes from the catalogue: it is read by a person, so it speaks
/// their language like everything else in the window.
pub fn line() -> String {
    let mut s = version().to_string();
    match (commit(), commit_date()) {
        (Some(h), Some(d)) => s.push_str(&format!(" ({h}, {d})")),
        (Some(h), None) => s.push_str(&format!(" ({h})")),
        (None, _) => {}
    }
    if is_modified() {
        s.push_str(&format!(", {}", crate::i18n::tr("about-build-modified")));
    }
    s
}

/// THE BLOCK A PERSON PASTES INTO A REPORT.
///
/// DELIBERATELY IN ENGLISH, whatever language the window speaks. It is not read by the person who
/// copies it — it is read by whoever picks the report up, and a tracker where half the reports arrive
/// in a language the maintainer cannot read is worse than one where the technical block is uniform.
/// What the person themselves reads — the line in the window — does speak their language.
///
/// It grows: the environment (the drawing path, the graphics adapter, the language, the last refusal
/// from the kernel) joins it as those become available.
pub fn report_block() -> String {
    let mut s = format!("QymCAD {}", version());
    match (commit(), commit_date()) {
        (Some(h), Some(d)) => s.push_str(&format!(" ({h}, {d})")),
        (Some(h), None) => s.push_str(&format!(" ({h})")),
        (None, _) => s.push_str(" (built without git)"),
    }
    if is_modified() {
        s.push_str(" MODIFIED");
    }
    s.push_str(&format!("\nOS: {} {}", std::env::consts::OS, std::env::consts::ARCH));
    s
}

#[cfg(test)]
mod tests {
    /// THE LINE MUST NAME THE BUILD, and it must never be empty. An empty or half-built string reaches
    /// a report as an empty string too, and then the report answers nothing — which is the whole reason
    /// this module exists.
    #[test]
    fn the_build_names_itself() {
        let v = super::version();
        assert!(!v.is_empty(), "the version from the manifest is empty");

        let line = super::line();
        assert!(line.starts_with(v), "the line does not start with the version: {line:?}");

        // The workspace is a git repository, so a build made in it MUST carry a commit. Were this
        // assertion absent, a build.rs that quietly stopped stamping would go unnoticed: the line would
        // still be non-empty, just useless.
        let in_a_repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git").exists();
        if in_a_repo {
            let h = super::commit().unwrap_or_default();
            assert!(h.len() >= 7, "built inside a repository and the commit is not in the line: {line:?}");
            assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "the commit is not a hash: {h:?}");
        }
    }

    /// NO PERSONAL PATHS IN THE LINE. It goes into a public issue tracker, and a build path carries the
    /// name of whoever built it.
    #[test]
    fn the_line_carries_no_paths() {
        let line = super::line();
        assert!(!line.contains('/') && !line.contains('\\'), "the line carries a path: {line:?}");
    }
}
