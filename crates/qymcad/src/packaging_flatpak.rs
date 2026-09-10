//! THE FLATPAK MANIFEST IS BUILT TO FLATHUB'S CONDITIONS, AND THE ID IS THE SAME EVERYWHERE.
//!
//! Two of Flathub's conditions shape the whole manifest: the application must be built ENTIRELY FROM
//! SOURCE, and THERE IS NO NETWORK DURING THE BUILD. So the kernel is a module of its own with a checksum,
//! and every crate arrives as a declared source - a build that tries to fetch anything simply stops.
//!
//! THE APPLICATION ID IS PERMANENT. It names the manifest, the desktop entry, the metainfo, the icon, the
//! directory of a person's settings and the page in the store. `cad.qymis.tech` is the project's site, so
//! the id is that backwards. Written in six places, and this is what keeps them one.
#[cfg(test)]
mod tests {
    /// IS THIS THE TREE THE WORK HAPPENS IN, or a published copy of it.
    ///
    /// `tools/` never leaves: it holds the publishing script, the snapshot of released dependency
    /// versions and the log of what was published. Several checks read from there, and in a published
    /// tree they would panic on a missing file - which is the FIRST thing somebody who downloaded the
    /// sources and ran `cargo test` would see. Measured on a fresh clone of the public repository: five
    /// checks failed that way, four of them on `tools/`.
    ///
    /// These are ratchets over our own work, not statements about the program. In a copy of the tree
    /// they have nothing to measure, and saying nothing is the honest answer.
    fn in_the_working_tree() -> bool {
        root().join("tools").is_dir()
    }

    use std::path::PathBuf;

    const ID: &str = "tech.qymis.cad";

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(rel: &str) -> String {
        let p = root().join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel} must be readable: {e}"))
    }

    /// THE THREE FILES ARE NAMED BY THE ID, AND SAY THE ID INSIDE.
    #[test]
    fn every_file_of_the_package_carries_the_application_id() {
        for f in [format!("{ID}.yml"), format!("{ID}.desktop"), format!("{ID}.metainfo.xml")] {
            let src = read(&format!("packaging/flatpak/{f}"));
            assert!(src.contains(ID), "{f} does not name {ID} - a file named for the id and not carrying it is the half that gets forgotten");
        }
        let desktop = read(&format!("packaging/flatpak/{ID}.desktop"));
        assert!(desktop.contains(&format!("Icon={ID}")), "the desktop entry points at an icon not named by the id, and a software centre finds nothing");
    }

    /// THE MANIFEST INSTALLS WHAT FLATHUB ASKS FOR, under the id.
    #[test]
    fn the_manifest_installs_the_metainfo_the_icons_and_the_licence() {
        let yml = read(&format!("packaging/flatpak/{ID}.yml"));
        for what in [
            format!("/app/share/metainfo/{ID}.metainfo.xml"),
            format!("/app/share/applications/{ID}.desktop"),
            format!("/app/share/icons/hicolor/${{s}}x${{s}}/apps/{ID}.png"),
            format!("/app/share/licenses/{ID}/LICENSE"),
        ] {
            assert!(yml.contains(&what), "the manifest does not install {what}");
        }
    }

    /// NOTHING IS FETCHED WHILE BUILDING.
    ///
    /// The sandbox has no network, so `--offline` is not caution here but the only way the build can work,
    /// and every crate has to be declared. Both halves are checked: the flag, and the file that carries the
    /// crates.
    #[test]
    fn the_build_asks_the_network_for_nothing() {
        let yml = read(&format!("packaging/flatpak/{ID}.yml"));
        assert!(yml.contains("--offline"), "cargo would try to reach crates.io, and the sandbox has no network");
        assert!(yml.contains("cargo-sources.json"), "the crates are not declared as sources, so an offline build has nothing to build from");
        assert!(yml.contains("--locked"), "the build does not hold the lock file, so it could resolve different versions than the ones declared");
    }

    /// THE KERNEL IS BUILT FROM SOURCE, WITH A REAL CHECKSUM.
    ///
    /// A source without a checksum, or with an invented one, is the difference between building what was
    /// meant and building whatever the download turned out to be. Measured: the archive of V7_8_1 is 48 MB
    /// and hashes to the value below.
    #[test]
    fn the_kernel_arrives_with_a_checksum_that_was_measured() {
        let yml = read(&format!("packaging/flatpak/{ID}.yml"));
        assert!(yml.contains("7321af48c34dc253bf8aae3f0430e8cb10976961d534d8509e72516978aa82f5"), "the kernel's checksum is not the measured one");
        assert!(yml.contains("V7_8_1"), "the kernel version changed and the checksum beside it did not");
    }

    /// AND THE SOURCE OF THE APPLICATION NAMES A COMMIT, not only a tag.
    ///
    /// A tag can be moved. A build that "reproduced a release" would then reproduce something else, and
    /// nobody would know which.
    #[test]
    fn the_application_source_names_a_commit() {
        let yml = read(&format!("packaging/flatpak/{ID}.yml"));
        let commit = yml.lines().find_map(|l| l.trim().strip_prefix("commit: ")).expect("the source names no commit at all");
        assert_eq!(commit.len(), 40, "\"{commit}\" is not a full commit hash");
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "\"{commit}\" is not hexadecimal");
    }

    /// THE DESKTOP ENTRY AND THE METAINFO POINT AT EACH OTHER.
    #[test]
    fn the_metainfo_launches_the_desktop_entry() {
        let xml = read(&format!("packaging/flatpak/{ID}.metainfo.xml"));
        assert!(xml.contains(&format!("<id>{ID}</id>")), "the metainfo declares a different id");
        assert!(
            xml.contains(&format!("<launchable type=\"desktop-id\">{ID}.desktop</launchable>")),
            "the metainfo does not say which desktop entry starts the program, and a software centre shows a page with no button"
        );
    }

    /// EVERY CRATE THE BUILD NEEDS IS DECLARED AS A SOURCE.
    ///
    /// The Flathub sandbox has NO NETWORK: `cargo` cannot fetch anything, so each crate has to arrive as
    /// a declared source, and `cargo-sources.json` is that list. It is generated from `Cargo.lock` by
    /// `packaging/flatpak/update-manifest.sh` - a step somebody has to remember.
    ///
    /// Nothing reminds them. Adding a dependency leaves the list quietly stale, everything here goes on
    /// building, and the failure appears where it costs most: inside the Flathub build, after the kernel
    /// has been compiled from source. This is that reminder, and it runs with every ordinary test.
    ///
    /// THE LIST IS GENERATED, NOT KEPT: it is in `.gitignore`, weighs a quarter of a megabyte and is
    /// derived entirely from `Cargo.lock`. So its ABSENCE is the normal state of a fresh checkout and
    /// says nothing is wrong - only a list that exists can be out of date. Asking for it unconditionally
    /// would greet everyone who clones the sources with a failure about a file that was never theirs.
    #[test]
    fn every_crate_in_the_lock_file_is_declared_as_a_source() {
        let path = root().join("packaging/flatpak/cargo-sources.json");
        let Ok(sources) = std::fs::read_to_string(&path) else {
            return; // not generated here, and therefore not stale
        };
        let lock = read("Cargo.lock");

        // `dest: cargo/vendor/<name>-<version>` is where each crate is unpacked, so the list of names is
        // read from there rather than from the urls, which are escaped and split across fields.
        let declared: Vec<String> = sources
            .lines()
            .filter_map(|l| l.trim().strip_prefix("\"dest\": \"cargo/vendor/"))
            .filter_map(|l| l.split('"').next())
            .map(str::to_string)
            .collect();
        assert!(declared.len() > 500, "the list of crate sources reads as almost empty: {} entries", declared.len());

        // the lock file's own records: name + version, minus our own crates, which are not fetched
        let ours: Vec<&str> = ["qymcad"].to_vec();
        let mut missing = Vec::new();
        let mut name = String::new();
        for line in lock.lines() {
            let t = line.trim();
            if let Some(n) = t.strip_prefix("name = \"").and_then(|s| s.strip_suffix('"')) {
                name = n.to_string();
            } else if let Some(v) = t.strip_prefix("version = \"").and_then(|s| s.strip_suffix('"')) {
                if name.is_empty() || ours.iter().any(|o| name == *o || name.starts_with(&format!("{o}-"))) {
                    continue;
                }
                let want = format!("{name}-{v}");
                if !declared.iter().any(|d| *d == want) {
                    missing.push(want);
                }
            }
        }
        assert!(
            missing.is_empty(),
            "these crates are locked but not declared as Flatpak sources, and the Flathub build reaches no network:\n  {}\nrun packaging/flatpak/update-manifest.sh",
            missing.join("\n  ")
        );
    }

    /// THE PICTURES OF THE STORE LISTING ARE PINNED, AND THEY EXIST.
    ///
    /// Flathub asks for "a link from a tag or a commit and not a branch": what a branch points at can
    /// change after a reviewer has looked, and the pictures people then get are not the pictures that
    /// were approved. Ours pointed at `main` for months.
    ///
    /// And the other half: a link is only a promise. A caption naming a file that is not in the tree
    /// gives the store a blank space where a screenshot should be, and nothing here would have said so.
    #[test]
    fn the_store_pictures_are_pinned_to_a_commit_and_really_exist() {
        if !in_the_working_tree() {
            return; // a published copy of the tree: nothing here to measure
        }
        let xml = read("packaging/flatpak/tech.qymis.cad.metainfo.xml");
        let images: Vec<&str> = xml
            .lines()
            .filter_map(|l| l.trim().strip_prefix("<image>").and_then(|l| l.strip_suffix("</image>")))
            .collect();
        assert!(images.len() >= 3, "a store listing with fewer than three pictures: {}", images.len());

        for url in &images {
            assert!(
                !url.contains("/main/") && !url.contains("/master/"),
                "the picture is linked from a BRANCH, and a branch moves: {url}"
            );
            // A COMMIT, not a tag. Naming the tag closed a circle: the address would name the tag, and
            // the tag has to stand on a commit whose file already carries that address. The pictures do
            // not change from release to release, so the commit that added them is the ref.
            assert!(
                url.split('/').any(|part| part.len() == 40 && part.chars().all(|c| c.is_ascii_hexdigit())),
                "the picture is linked from something that is not a commit: {url}"
            );
            // the file itself, taken from the tail of the address
            let rel = url.split("/docs/").nth(1).unwrap_or_else(|| panic!("the address does not point into docs/: {url}"));
            let path = root().join("docs").join(rel);
            assert!(path.exists(), "the listing promises a picture that is not in the tree: docs/{rel}");
        }

        // every picture is captioned: a store shows the caption under it, and an empty one reads as unfinished
        let captions = xml.matches("<caption>").count();
        assert_eq!(captions, images.len(), "{} pictures and {} captions", images.len(), captions);

        // AND THEY TRAVEL WITH THE SOURCES. The addresses name raw.githubusercontent of the PUBLIC tree,
        // which `tools/publish.py` assembles from a whitelist. A picture that stays behind leaves a blank
        // space in the store, and nothing else here would say so - the file is in this tree, and the
        // check above is happy.
        let publish = read("tools/publish.py");
        let dirs = publish.split("ALLOW_DIRS = [").nth(1).unwrap_or_default().split(']').next().unwrap_or_default();
        for url in &images {
            let rel = url.split("/docs/").nth(1).unwrap_or_default();
            let dir = format!("docs/{}", rel.rsplit_once('/').map(|(d, _)| d).unwrap_or(""));
            assert!(
                dirs.contains(&format!("\"{dir}\"")),
                "the store is promised {url}, and {dir} is not in the whitelist of tools/publish.py - the file never reaches the public tree"
            );
        }
    }
}
