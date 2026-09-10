//! THE PROGRAM ANSWERS TO ONE NAME, IN EVERY PLACE THAT ASKS.
//!
//! The same reverse-DNS name identifies the program to the desktop, to macOS, to Flathub, and it decides
//! the directory a person's settings, colour schemes, templates and library of parts live in. It is said in
//! four places that cannot share a constant - Rust, a shell script, a YAML manifest, an XML description -
//! so it is compared instead.
//!
//! MEASURED BEFORE IT WAS MADE ONE. Three different names were in the tree at once: `tech.qymis.qym-cad`
//! written out in six places in the code, `tech.qymis.qymcad` in the macOS bundle, `tech.qymis.cad` in the
//! Flatpak manifest. Nothing was broken by it - each half worked alone - which is exactly why it had gone
//! unnoticed through two releases.
//!
//! WHAT IS NOT COMPARED HERE, and deliberately: `QymIsTech.QymCAD` in the winget manifest and `qymcad-bin`
//! in the AUR package. Those catalogues have naming rules of their own, `Publisher.Package` and the Arch
//! convention, and forcing a reverse-DNS name on them would be wrong rather than consistent.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(rel: &str) -> String {
        let p = root().join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel} must be readable: {e}"))
    }

    /// EVERY PLACE THAT NAMES THE PROGRAM NAMES THE SAME ONE.
    #[test]
    fn the_program_is_called_the_same_thing_everywhere() {
        let id = qymcad_paths::APP_ID;
        let mut wrong = Vec::new();

        // macOS: what the system remembers permissions and file associations by
        let mac = read("packaging/macos/bundle.sh");
        if !mac.contains(&format!("<key>CFBundleIdentifier</key><string>{id}</string>")) {
            wrong.push("the macOS bundle identifies the program as something else".to_string());
        }

        // Flatpak: the manifest, the desktop entry and the metainfo are named by it and carry it
        for f in [format!("{id}.yml"), format!("{id}.desktop"), format!("{id}.metainfo.xml")] {
            let p = root().join("packaging/flatpak").join(&f);
            if !p.exists() {
                wrong.push(format!("packaging/flatpak/{f} does not exist - the Flatpak files are named by the id"));
                continue;
            }
            if !std::fs::read_to_string(&p).unwrap_or_default().contains(id) {
                wrong.push(format!("packaging/flatpak/{f} does not carry {id}"));
            }
        }

        assert!(
            wrong.is_empty(),
            "the program answers to more than one name, and each half works alone - which is why this goes unnoticed:\n{}",
            wrong.join("\n")
        );
    }

    /// AND THE OLD NAMES ARE GONE FROM THE CODE.
    ///
    /// The directory is built from the id in one place now. A second `ProjectDirs::from` anywhere is the
    /// same decision written twice, which is how the three names came about.
    #[test]
    fn the_directory_is_decided_in_one_place() {
        let mut twice = Vec::new();
        for entry in walk(&root().join("crates")) {
            // the one place that may decide it, and this file, which only talks about it - it found itself
            // on the first run, which is the oldest joke in this kind of check
            if entry.contains("qymcad-paths") || entry.ends_with("app_identity.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&entry).unwrap_or_default();
            if src.contains("ProjectDirs::from(") {
                twice.push(entry);
            }
        }
        assert!(twice.is_empty(), "the program's own directory is decided outside qymcad-paths:\n{}", twice.join("\n"));
    }

    /// Every `.rs` file under a directory.
    fn walk(dir: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else { return out };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p.to_string_lossy().into_owned());
            }
        }
        out
    }
}
