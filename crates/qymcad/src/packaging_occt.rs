//! EVERY BUILD OF THE KERNEL KEEPS ITS EXCEPTIONS.
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
#[cfg(test)]
mod tests {
    const FLAG: &str = "BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF";

    /// Every tracked file that configures a build of the kernel - it switches the Draw module off, as each of them
    /// does - must keep the exceptions on as many times as it configures one.
    #[test]
    fn every_build_of_the_kernel_keeps_its_exceptions() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let listed = std::process::Command::new("git").arg("ls-files").current_dir(&root).output().expect("git lists the tracked files");
        let mut builds = 0;
        let mut bad = Vec::new();
        for path in String::from_utf8_lossy(&listed.stdout).lines() {
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
