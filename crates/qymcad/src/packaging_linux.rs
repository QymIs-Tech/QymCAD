//! THE LINUX PACKAGE CARRIES THE LIBRARIES IT OPENS BY NAME.
//!
//! Reported behaviour: the AppImage check of appimage.github.io ran the package under firejail and it
//! panicked with "Library libxkbcommon-x11.so could not be loaded."
//!
//! WHY IT HAPPENED. `linuxdeploy` gathers what the ELF header asks for, and the window stack does not ask.
//! `xkbcommon-dl`, `x11-dl` and `wayland-sys` open their libraries with `dlopen` by plain name, so none of
//! them appears in `ldd` and none of them was packed. Measured on the built binary: `ldd` names libX11,
//! libxcb, libXau and libXdmcp, and not one of libxkbcommon, libxkbcommon-x11, libXcursor, libXrandr, libXi
//! or libwayland-*. On a machine that happens to have them installed the package starts; on a bare one it
//! dies on the first one it reaches.
//!
//! WHY A CHECK AND NOT JUST THE FIX. The list in the build script and the crates that do the opening are
//! two different places, and they drift silently: a dependency added a year from now brings a library
//! nobody names, and the package goes on building. The failure then appears on somebody else's machine,
//! which is where this one appeared.
//!
//! AND THEN THE CURE BROKE IT THE OTHER WAY. Reported behaviour: the package built with that list inside
//! died on a Wayland desktop with "WGPU error: Failed to create surface for any enabled backend: {}".
//!
//! `libwayland-client.so.0` is on the AppImage excludelist, the format's own list of libraries that have
//! to be the host's own - it is the connection to the compositor, and a copy built elsewhere does not
//! talk to this one. `linuxdeploy` honours that list by itself; `--library` forces a file in and walks
//! straight past it. Measured on the shipped package: with the three wayland libraries inside it fails,
//! with `libwayland-client.so.0` alone deleted the window opens.
//!
//! So the rule has two halves, and both are checked below: what is opened by name travels inside, EXCEPT
//! what has to be the host's own. Its two companions went with it - a machine running a compositor
//! already has all three, and a machine without one never reaches the Wayland path at all.
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

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn script() -> String {
        let p = root().join("packaging/linux/build-appimage.sh");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("the AppImage build script must be readable: {e}"))
    }

    /// THE CRATES THAT OPEN LIBRARIES BY NAME, and what each of them opens.
    ///
    /// Taken from their sources, where the names are written out as string literals - `xkbcommon-dl`
    /// `src/lib.rs` and `src/x11.rs`, `x11-dl` `src/link.rs`, `wayland-sys`. Only the halves this program
    /// actually reaches are listed: `x11-dl` alone names two dozen, most of them belonging to toolkits we
    /// do not use.
    const OPENED_BY: [(&str, &[&str]); 3] = [
        ("xkbcommon-dl", &["libxkbcommon.so.0", "libxkbcommon-x11.so.0"]),
        ("x11-dl", &["libXcursor.so.1", "libXrandr.so.2", "libXi.so.6"]),
        ("wayland-sys", &["libwayland-client.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1"]),
    ];

    /// THE LIBRARIES THAT HAVE TO BE THE HOST'S OWN, opened by name or not.
    ///
    /// The graphics and compositor part of the AppImage excludelist. Each of these is one end of a
    /// conversation whose other end lives on the machine the package is run on: the compositor, the
    /// driver, the display server. A copy carried inside talks to the wrong end.
    const HOST_S_OWN: &[&str] = &[
        "libwayland-client.so.0",
        "libwayland-cursor.so.0",
        "libwayland-egl.so.1",
        "libGL.so.1",
        "libEGL.so.1",
        "libGLX.so.0",
        "libvulkan.so.1",
        "libgbm.so.1",
        "libdrm.so.2",
        "libX11.so.6",
        "libxcb.so.1",
    ];

    /// The names inside one `NAME=(...)` array of the script, and nothing from the prose around it.
    ///
    /// Reading the whole file would answer the wrong question: after this fix `libwayland-client.so.0`
    /// appears in the script twice over - once in the list of what must never be carried, once in the
    /// comment explaining why - and a check that only searched the text would call that "carried".
    fn array(name: &str) -> Vec<String> {
        let sh = script();
        let start = sh.find(&format!("\n{name}=(")).unwrap_or_else(|| panic!("the script has no {name} array"));
        let body = &sh[start + name.len() + 3..];
        let end = body.find("\n)").unwrap_or_else(|| panic!("the {name} array is never closed"));
        body[..end].lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect()
    }

    /// EVERY LIBRARY THE PROGRAM OPENS BY NAME IS NAMED IN THE PACKAGE SCRIPT.
    ///
    /// Walked from the LOCK FILE rather than from a list written out here: a crate that stops being a
    /// dependency stops being asked about, and one that stays is asked about for ever.
    #[test]
    fn the_package_carries_every_library_opened_at_run_time() {
        let lock = std::fs::read_to_string(root().join("Cargo.lock")).expect("the lock file reads");
        let carried = array("DLOPENED");
        let mut missing = Vec::new();
        for (krate, libs) in OPENED_BY {
            if !lock.contains(&format!("name = \"{krate}\"")) {
                continue; // no longer a dependency: nothing of its opens anything
            }
            for lib in libs {
                if HOST_S_OWN.contains(lib) {
                    continue; // opened by name, and still the host's to provide - the other check owns it
                }
                if !carried.iter().any(|c| c == lib) {
                    missing.push(format!("{krate} opens {lib}, and the package script does not carry it"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "the package would build without libraries the program opens by name, and start only on a machine that happens to have them:\n{}",
            missing.join("\n")
        );
    }

    /// AND THE SYSTEM'S HALF OF THE GRAPHICS STACK IS NOT CARRIED.
    ///
    /// The tempting cure for "a library is missing" is to pack every library, and this is the check that
    /// says where that stops. It is not hypothetical: the package that carried `libwayland-client.so.0`
    /// shipped, and died on the first Wayland desktop it met with "Failed to create surface".
    ///
    /// Both halves are asked. The list must not request them - and the script must also refuse them by
    /// itself, because a library can arrive without being asked for, dragged in as somebody else's
    /// dependency. That is why the check exists in the build too, on the unpacked package.
    #[test]
    fn the_system_s_own_libraries_are_left_to_the_system() {
        let carried = array("DLOPENED");
        let packed: Vec<&&str> = HOST_S_OWN.iter().filter(|l| carried.iter().any(|c| &c == *l)).collect();
        assert!(packed.is_empty(), "the package asks for libraries that have to be the host's own, and breaks the machines that have them: {packed:?}");

        let refused = array("NEVER_CARRY");
        let unguarded: Vec<&&str> = HOST_S_OWN.iter().filter(|l| !refused.iter().any(|r| &r == *l)).collect();
        assert!(unguarded.is_empty(), "the build script would let these through without a word: {unguarded:?}");

        let sh = script();
        let after = sh.split("--appimage-extract").nth(1).expect("something follows the extraction");
        assert!(
            after.contains("NEVER_CARRY"),
            "the finished package is never searched for the libraries that must not be in it, so one dragged in as a dependency would ship"
        );
    }

    /// THE FINISHED PACKAGE IS OPENED AND LOOKED INSIDE.
    ///
    /// A list handed to `linuxdeploy` is a request. Whether it was honoured is a different question, and
    /// the difference shows up on somebody else's machine - so the script unpacks what it built and looks.
    /// Without that step the list here would be a wish rather than a guarantee.
    #[test]
    fn the_script_looks_inside_what_it_built() {
        let sh = script();
        assert!(sh.contains("--appimage-extract"), "the script does not open the package it built, so a library that failed to be packed would go unnoticed");
        let after = sh.split("--appimage-extract").nth(1).expect("something follows the extraction");
        assert!(after.contains("exit 1"), "the script opens the package and does not stop on what it finds there");
    }

    /// THE FILES THESE CHECKS READ TRAVEL WITH THE SOURCES.
    ///
    /// Every check about packaging reads a file from `packaging/` - a shell script, a PKGBUILD, a manifest.
    /// That is fine while the tree is whole, and a trap the moment it is not: somebody who downloads the
    /// sources and runs `cargo test` gets a wall of "must be readable" for files that never left this
    /// machine, and concludes the project does not build.
    ///
    /// The public tree is assembled by `tools/publish.py` from a whitelist. So the question is not "are the
    /// files there" but "does the whitelist still carry them", and that is what is asked here. `Cargo.lock`
    /// is on the list too - one check walks it to learn which crates open libraries by name.
    #[test]
    fn the_public_tree_carries_what_these_checks_read() {
        if !in_the_working_tree() {
            return; // a published copy of the tree: nothing here to measure
        }
        let publish = std::fs::read_to_string(root().join("tools/publish.py")).expect("the publishing script reads");
        // CUT BY THE ASSIGNMENT, NOT BY THE NAME. The first edition split on "ALLOW_ONLY_SUFFIXES" and cut
        // the list short: that name appears one line earlier IN A COMMENT, so the slice ended before
        // "packaging" was reached and the check failed over a whitelist that was perfectly correct.
        let allow = publish.split("ALLOW_DIRS = [").nth(1).unwrap_or_default();
        let allow = &allow[..allow.find("\nALLOW_ONLY_SUFFIXES =").unwrap_or(allow.len())];
        let mut absent = Vec::new();
        for what in ["\"packaging\"", "\"crates\"", "\"Cargo.lock\""] {
            if !allow.contains(what) {
                absent.push(what);
            }
        }
        assert!(
            absent.is_empty(),
            "the published sources would not carry {absent:?}, and every packaging check would fail for whoever downloaded them - having proved nothing about their machine"
        );
    }
}
