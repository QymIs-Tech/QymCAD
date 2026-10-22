// THE VERSION OF OCCT THE BRIDGE IS WRITTEN AGAINST, and what `build.rs` says when it finds another.
//
// The bridge (`occt_*.cpp`) stands on behaviour of OCCT measured on this version: the simple offset of a sheet gives
// no history, the history of a split seam is partial, the loose bounding box is built from poles. Another version may
// behave otherwise under the same calls, so a build against it is warned about - not refused: a distribution or another
// system may build against its own OCCT at its own risk. The build instructions are in `packaging/README.md`.
//
// Kept apart from `build.rs` (which `include!`s this file) so the reading of the header is checked by the tests of the
// crate, on any machine.
pub const OCCT_VERSION: &str = "7.9.3";

/// THE VERSION A `Standard_Version.hxx` NAMES: the string of its `OCC_VERSION_COMPLETE`, or nothing when the text holds
/// no such line.
pub fn occt_version_in(header: &str) -> Option<&str> {
    header.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix("#define")?.trim_start().strip_prefix("OCC_VERSION_COMPLETE")?;
        rest.trim().strip_prefix('"')?.split('"').next()
    })
}

/// THE WARNING A BUILD AGAINST THIS HEADER GETS, or nothing when it names the version the bridge is written against.
pub fn occt_version_warning(header: &str) -> Option<String> {
    match occt_version_in(header) {
        Some(v) if v == OCCT_VERSION => None,
        Some(v) => Some(format!("OCCT {v} found; the bridge is written and tested against OCCT {OCCT_VERSION} - see packaging/README.md")),
        None => Some(format!("the version of OCCT was not read from Standard_Version.hxx; the bridge is written and tested against OCCT {OCCT_VERSION}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines of the header of 7.9.3 that matter, as it has them.
    const HEADER: &str = "#define OCC_VERSION_MAJOR 7\n#define OCC_VERSION_COMPLETE \"7.9.3\"\n#ifdef OCC_VERSION_DEVELOPMENT\n";

    #[test]
    fn the_version_the_bridge_is_written_against_says_nothing() {
        assert_eq!(occt_version_in(HEADER), Some("7.9.3"));
        assert_eq!(occt_version_warning(HEADER), None);
    }

    #[test]
    fn another_version_is_warned_about_by_its_number() {
        let older = HEADER.replace("7.9.3", "7.8.1");
        let said = occt_version_warning(&older).expect("7.8.1 is warned about");
        assert!(said.contains("7.8.1") && said.contains(OCCT_VERSION), "the warning does not name both versions: {said}");
    }

    #[test]
    fn a_header_with_no_version_is_warned_about() {
        assert_eq!(occt_version_in("#define OCC_VERSION_MAJOR 7\n"), None);
        assert!(occt_version_warning("").is_some(), "a header that names no version passed in silence");
    }
}
