//! EVERY BUILD OF THE KERNEL KEEPS ITS EXCEPTIONS, AND IS OF THE ONE VERSION THE BRIDGE IS WRITTEN AGAINST.
//!
//! OCCT built in Release switches its own checks off unless told otherwise: `BUILD_RELEASE_DISABLE_EXCEPTIONS`
//! defaults to ON in 7.9.3's CMakeLists.txt, which defines `No_Exception` and compiles every
//! `Standard_ConstructionError_Raise_if` and its kin to nothing. The bridge turns a thrown `Standard_Failure` into
//! `None`; with nothing thrown, a degenerate input goes on into the algorithm.
//!
//! Reported behaviour: the first CI run, on a kernel built from source with the defaults, went red at
//! `degenerate_geometry_returns_none_not_abort` - "a zero height gives None" - while the developer's machine,
//! whose system package is built with the checks on, was green. The packages people download were built with
//! the same defaults as that runner.
//!
//! The version: the bridge stands on behaviour of OCCT measured on one version (`qymcad_kernel::occt_version`). An
//! instruction that builds another one builds a kernel nobody has tested the program on, and nothing says so: the
//! Windows instruction kept building 7.8.1 after every build we make had moved to 7.9.3.
#[cfg(test)]
mod tests {
    const FLAG: &str = "BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF";

    /// THE TREE: where it lies and the files git tracks in it.
    struct Tree {
        root: std::path::PathBuf,
        paths: Vec<String>,
    }

    fn tracked() -> Tree {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let listed = std::process::Command::new("git").arg("ls-files").current_dir(&root).output().expect("git lists the tracked files");
        let paths = String::from_utf8_lossy(&listed.stdout).lines().map(str::to_string).collect();
        Tree { root, paths }
    }

    /// The instructions a person building the kernel reads, and the notice of what the packages carry.
    const INSTRUCTIONS: [&str; 6] = ["README.md", "README.ru.md", "CONTRIBUTING.md", "CONTRIBUTING.ru.md", "packaging/README.md", "THIRD-PARTY-NOTICES.md"];

    /// EVERY VERSION OF OCCT A TEXT NAMES: a number `7.x.y` or `7_x_y` standing alone (the tag `V7_9_3`, the folder
    /// `OCCT-7_9_3`, the key of a cache `occt-7.9.3-...`), and a lower bound `OCCT 7.x+`.
    fn versions_named(text: &str) -> Vec<String> {
        let b = text.as_bytes();
        let mut out = Vec::new();
        for (at, _) in text.match_indices('7') {
            if at > 0 && (b[at - 1].is_ascii_digit() || b[at - 1] == b'.') {
                continue;
            }
            let rest = &text[at..];
            let len = rest.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '_' && c != '+').unwrap_or(rest.len());
            let word = rest[..len].trim_end_matches(['.', '_']);
            let parts: Vec<&str> = word.trim_end_matches('+').split(['.', '_']).collect();
            let bound = word.ends_with('+') && parts.len() == 2 && text[..at].ends_with("OCCT ");
            if (parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))) || bound {
                out.push(word.replace('_', "."));
            }
        }
        out
    }

    /// Every tracked file that configures a build of the kernel, and every instruction, names the version the bridge is
    /// written against and no other.
    #[test]
    fn every_build_of_the_kernel_is_of_the_one_version() {
        let want = qymcad_kernel::occt_version::OCCT_VERSION;
        let Tree { root, paths } = tracked();
        let mut bad = Vec::new();
        for path in &paths {
            if path.ends_with(".rs") {
                continue; // the checks name versions themselves
            }
            let Ok(text) = std::fs::read_to_string(root.join(path)) else { continue };
            let builds = text.contains("BUILD_MODULE_Draw=OFF");
            if !builds && !INSTRUCTIONS.contains(&path.as_str()) {
                continue;
            }
            let named = versions_named(&text);
            if !named.iter().any(|v| v == want) {
                bad.push(format!("{path}: does not name OCCT {want}"));
            }
            let other: Vec<&String> = named.iter().filter(|v| *v != want).collect();
            if !other.is_empty() {
                bad.push(format!("{path}: names {other:?}"));
            }
        }
        assert!(bad.is_empty(), "a build or an instruction of the kernel is not of OCCT {want}:\n  {}", bad.join("\n  "));
    }

    /// THE NOTE BESIDE A BUILD OF THE KERNEL: the version, the flag and the signals, each with its reason, and where the
    /// instructions say more. A packager reads the file that builds the kernel, not the checks.
    const NOTE: &str = "THE KERNEL IS OCCT 7.9.3 EXACTLY, built so on purpose (packaging/README.md, \"The kernel\")";

    /// Every tracked file that configures a build of the kernel says beside each build why it is built so; the
    /// instructions the note points to are the place that says it at length.
    #[test]
    fn every_build_of_the_kernel_says_why_it_is_built_so() {
        let Tree { root, paths } = tracked();
        let mut bad = Vec::new();
        for path in paths.iter().filter(|p| !p.ends_with(".rs") && p.as_str() != "packaging/README.md") {
            let Ok(text) = std::fs::read_to_string(root.join(path)) else { continue };
            let configured = text.matches("BUILD_MODULE_Draw=OFF").count();
            let said = text.matches(NOTE).count();
            if said < configured {
                bad.push(format!("{path}: {configured} build(s) of the kernel, {said} with the note"));
            }
        }
        assert!(NOTE.contains(qymcad_kernel::occt_version::OCCT_VERSION), "the note names another version than the bridge is written against");
        assert!(bad.is_empty(), "a build of the kernel does not say why it is built so:\n  {}", bad.join("\n  "));
    }

    /// The reading of versions sees what it is for: the tag, the folder, the key of a cache and a lower bound are read,
    /// a version of another program and a number inside a longer one are not.
    #[test]
    fn the_versions_are_read_where_they_are_written() {
        let text = "V7_9_3.tar.gz OCCT-7_8_1 occt-7.9.3-msvc OCCT 7.8+ glibc 2.35 rust 1.88 17.7.1 page 7.";
        assert_eq!(versions_named(text), ["7.9.3", "7.8.1", "7.9.3", "7.8+"]);
    }

    /// Every tracked file that configures a build of the kernel - it switches the Draw module off, as each of them
    /// does - must keep the exceptions on as many times as it configures one.
    #[test]
    fn every_build_of_the_kernel_keeps_its_exceptions() {
        let Tree { root, paths } = tracked();
        let mut builds = 0;
        let mut bad = Vec::new();
        for path in &paths {
            if path.ends_with(".rs") {
                continue; // this check names the flag itself
            }
            let Ok(text) = std::fs::read_to_string(root.join(path)) else { continue };
            let configured = text.matches("BUILD_MODULE_Draw=OFF").count();
            if configured == 0 {
                continue;
            }
            builds += configured;
            let kept = text.matches(FLAG).count();
            if kept != configured {
                bad.push(format!("{path}: {configured} build(s) of the kernel, {kept} with {FLAG}"));
            }
        }
        assert!(builds >= 5, "the builds of the kernel were not found: {builds} (the Linux image, two jobs of the release, the CI setup, Flatpak)");
        assert!(bad.is_empty(), "a kernel built with its checks compiled out:\n  {}", bad.join("\n  "));
    }
}
