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

    /// THE PACKAGING IMAGE MUST NOT BE OLDER THAN THE FLOOR IT CLAIMS TO BUILD.
    ///
    /// Both numbers live in files nobody compiles - a manifest field and a line in a Dockerfile - so they
    /// drifted apart in silence and were found out only in the place that costs most: the packaging run
    /// spent FORTY-FOUR MINUTES building OCCT from source and then refused at a two-second version check,
    /// because the image was pinned to the floor and the floor was stale. Nobody had noticed for months,
    /// since every machine here builds with something far newer.
    ///
    /// This is free and runs with every ordinary test.
    #[test]
    fn the_packaging_image_can_build_what_the_manifest_asks_for() {
        fn version(text: &str, needle: &str) -> (u32, u32) {
            let at = text.find(needle).unwrap_or_else(|| panic!("`{needle}` is no longer written where it was"));
            let rest = &text[at + needle.len()..];
            let num: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let mut parts = num.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
            (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest reads");
        let dockerfile = std::fs::read_to_string(root.join("packaging/linux/Dockerfile")).expect("the packaging image reads");

        let floor = version(&manifest, "rust-version = \"");
        let pinned = version(&dockerfile, "--default-toolchain ");
        assert!(
            pinned >= floor,
            "the packaging image is pinned to rust {}.{} while the manifest asks for {}.{} at least - \
             the image will refuse AFTER building the kernel, which is the most expensive place to find out",
            pinned.0, pinned.1, floor.0, floor.1
        );
    }

    /// A FILE THE RELEASE WORKFLOW NAMES MUST EXIST, and the job that reads it must have checked the
    /// sources out.
    ///
    /// The text that goes onto the release page lives in a file, and the release job used to check
    /// nothing out at all - it only moved artifacts about. A path pointing at nothing would have been
    /// found where such things always are: on the day of the release, after half an hour of building.
    #[test]
    fn the_release_page_gets_the_text_the_workflow_promises() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let flow = std::fs::read_to_string(root.join(".github/workflows/release.yml")).expect("the release workflow reads");

        let mut named = 0;
        for line in flow.lines() {
            let Some(path) = line.trim().strip_prefix("body_path:") else { continue };
            let path = path.trim();
            let text = std::fs::read_to_string(root.join(path))
                .unwrap_or_else(|_| panic!("the workflow puts `{path}` on the release page and there is no such file"));
            assert!(
                text.contains("## What changed"),
                "`{path}` is what people read on the release page and it no longer says what changed"
            );
            named += 1;
        }
        assert_eq!(named, 1, "the release page is made from exactly one text; found {named} of them");

        // The job reads that file from the working directory, so it has to lay the sources down first.
        let publish = flow.split("\n  publish:").nth(1).expect("the publishing job is still called `publish`");
        let checkout = publish.find("actions/checkout").expect("the publishing job checks nothing out, so the text cannot be there");
        let download = publish.find("download-artifact").expect("the publishing job no longer collects the packages");
        assert!(checkout < download, "checkout runs after the packages are downloaded and wipes them from the working directory");
    }

    /// NO PERSONAL PATHS IN THE LINE. It goes into a public issue tracker, and a build path carries the
    /// name of whoever built it.
    #[test]
    fn the_line_carries_no_paths() {
        let line = super::line();
        assert!(!line.contains('/') && !line.contains('\\'), "the line carries a path: {line:?}");
    }
}
