//! Running `packaging/macos/bundle.sh` here, with the two mac-only tools replaced by stubs.
//!
//! WHY THIS EXISTS. That script runs on one runner, the most expensive of the three, at the very END of a
//! build - so a mistake in it costs a whole build to find, and is found one line at a time. The first run
//! died before it, on the link. The second died INSIDE it after six minutes and printed nothing at all:
//! `grep` matched no library naming the build machine, `grep` returns 1 when it matches nothing, and
//! `set -o pipefail` turned that into the end of the script.
//!
//! `install_name_tool` and `otool` exist only on macOS. They are replaced by two stubs on PATH - one
//! records what it was asked to do and remembers which files it rewrote, the other prints a dependency
//! list in the real format and answers according to those marks. Everything the mistakes were actually in
//! - the copying, the loops, the sentinel, the archive - is ordinary shell and runs anywhere.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// What the stub `otool` says a library depends on before anything rewrites it.
#[derive(Clone, Copy, PartialEq)]
enum Deps {
    /// Every path is already `@rpath/...` - the case OCCT built by CMake actually produces, and the one
    /// that killed the second run: nothing for `grep` to match.
    Rpath,
    /// Paths name the build machine and are rewritten - the case the script was written for.
    BuildMachine,
    /// Paths name the build machine and REFUSE to be rewritten: the sentinel must catch it.
    Stubborn,
}

/// A tree that looks enough like the repository for the script: the binary, the icon, the licence, the
/// notices, a manifest with a version, and an OCCT installation of two modules under three names each -
/// `libTKernel.dylib` -> `libTKernel.7.8.dylib` -> `libTKernel.7.8.1.dylib`, exactly as OCCT installs.
fn sandbox(case: &str, deps: Deps) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qym_macos_bundle_{case}"));
    let _ = fs::remove_dir_all(&dir);
    let write = |rel: &str, text: &str| {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().expect("a parent")).expect("the sandbox is writable");
        fs::write(&p, text).expect("the file is written");
        p
    };
    let executable = |p: &Path| fs::set_permissions(p, fs::Permissions::from_mode(0o755)).expect("the bit is set");

    write("Cargo.toml", "[workspace.package]\nversion = \"0.1.0\"\n");
    write("LICENSE", "licence text\n");
    write("THIRD-PARTY-NOTICES.md", "notices\n");
    write("assets/icons/macos/qymcad.icns", "icns\n");
    executable(&write("target/release/qymcad", "the program\n"));

    // As OCCT installs them: one real file per module and two links to it. The chain matters - a `cp` that
    // follows links writes three full copies of every module, which is how an 80 MB archive was measured.
    for module in ["libTKernel", "libTKMath"] {
        write(&format!("occt/lib/{module}.7.8.1.dylib"), "a library\n");
        std::os::unix::fs::symlink(format!("{module}.7.8.1.dylib"), dir.join(format!("occt/lib/{module}.7.8.dylib"))).expect("the link is made");
        std::os::unix::fs::symlink(format!("{module}.7.8.dylib"), dir.join(format!("occt/lib/{module}.dylib"))).expect("the link is made");
    }

    // The stubs answer through this directory: `install_name_tool -change` leaves a mark here, and `otool`
    // reads it. Without that the two would contradict each other - a rewrite that changes nothing, and a
    // sentinel that then fires on every run.
    fs::create_dir_all(dir.join("changed")).expect("the sandbox is writable");
    let occt = dir.join("occt");
    let (marks, calls) = (dir.join("changed"), dir.join("calls.txt"));

    // `-change` is the only call that rewrites a path; the stubborn case records the call and rewrites
    // nothing, which is what a library the tool cannot touch looks like from outside.
    let records = if deps == Deps::Stubborn { "" } else { "[ \"$1\" = -change ] && : > \"$MARKS/$(basename \"$4\")\"\n" };
    executable(&write(
        "bin/install_name_tool",
        &format!(
            "#!/usr/bin/env bash\nMARKS={marks}\nprintf '%s\\n' \"$*\" >> {calls}\n{records}exit 0\n",
            marks = marks.display(),
            calls = calls.display()
        ),
    ));

    // `otool -L` prints the file, then its dependencies, one per tab-indented line. The first of them is
    // the file's own name, and the script skips only the header line - so the shape matters, not the text.
    let before = match deps {
        Deps::Rpath => "@rpath/libTKernel.7.8.dylib".to_string(),
        Deps::BuildMachine | Deps::Stubborn => format!("{}/lib/libTKernel.7.8.dylib", occt.display()),
    };
    // A LINK AND ITS TARGET ARE ONE FILE, and the stub has to answer as one: `otool` opens whatever the
    // name resolves to. Modelling them as separate files made the sentinel fire on names the script had
    // rightly left alone - the fixture lying, not the script.
    executable(&write(
        "bin/otool",
        &format!(
            "#!/usr/bin/env bash\n\
             resolve() {{\n  local f=$1 t\n  while [ -L \"$f\" ]; do\n    t=$(readlink \"$f\")\n    \
             case \"$t\" in /*) f=$t ;; *) f=$(dirname \"$f\")/$t ;; esac\n  done\n  basename \"$f\"\n}}\n\
             shift\nfor f in \"$@\"; do\n  printf '%s:\\n' \"$f\"\n  \
             if [ -e {marks}/\"$(resolve \"$f\")\" ]; then\n    printf '\\t@rpath/libTKernel.7.8.dylib (compatibility version 7.8.0)\\n'\n  \
             else\n    printf '\\t{before} (compatibility version 7.8.0)\\n'\n  fi\n  \
             printf '\\t/usr/lib/libc++.1.dylib (compatibility version 1.0.0)\\n'\ndone\n",
            marks = marks.display(),
            before = before
        ),
    ));
    dir
}

/// Run the real script over the sandbox. HOME is moved inside it as well: the script asks git to trust the
/// directory it is in, and that must not reach the settings of whoever runs the tests.
fn bundle(dir: &Path) -> Output {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/bundle.sh");
    let path = format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    Command::new("bash")
        .arg(&script)
        .current_dir(dir)
        .env("PATH", path)
        .env("HOME", dir)
        .env("OCCT_ROOT", dir.join("occt"))
        .env_remove("QYMCAD_VERSION")
        .output()
        .expect("bash runs the packaging script")
}

fn said(out: &Output) -> String {
    format!("--- stdout ---\n{}--- stderr ---\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// What the archive holds, one entry per line.
fn archive(dir: &Path) -> String {
    listing(dir, "-Z1")
}

/// The same, in the long form where the first column tells a link from a file.
fn listing(dir: &Path, form: &str) -> String {
    let zip = dir.join("dist/qymcad-0.1.0-macos-arm64.zip");
    assert!(zip.exists(), "the archive was not made: {}", zip.display());
    let out = Command::new("unzip").arg(form).arg(&zip).output().expect("unzip reads the archive");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// THE CASE THAT KILLED THE SECOND RUN. OCCT built by CMake announces itself through `@rpath` already, so
/// nothing in the bundle names the build machine and there is nothing to rewrite. `grep` says so by
/// returning 1, and under `set -o pipefail` that ended the script in silence, six minutes in.
#[test]
fn a_bundle_where_nothing_names_the_build_machine_is_still_assembled() {
    let dir = sandbox("rpath", Deps::Rpath);
    let out = bundle(&dir);
    assert!(out.status.success(), "the script refused although every path was already @rpath:\n{}", said(&out));

    let held = archive(&dir);
    for entry in ["QymCAD.app/Contents/MacOS/qymcad", "QymCAD.app/Contents/Info.plist", "QymCAD.app/Contents/Resources/qymcad.icns", "QymCAD.app/Contents/Resources/LICENSE.txt", "QymCAD.app/Contents/Resources/THIRD-PARTY-NOTICES.md", "README.txt"] {
        assert!(held.contains(entry), "the archive does not hold {entry}:\n{held}");
    }
    // Both notes travel, in both languages; the second is named in an alphabet this file does not spell out.
    assert_eq!(held.lines().filter(|l| l.ends_with(".txt") && !l.contains("Contents/")).count(), 2, "both notes must be in the archive:\n{held}");

    let calls = fs::read_to_string(dir.join("calls.txt")).expect("the tool was called");
    assert!(calls.contains("-add_rpath @executable_path/../Frameworks"), "the bundle was not pointed at its own Frameworks:\n{calls}");
    assert!(calls.contains("-id @rpath/libTKernel.7.8.1.dylib"), "a library was not made to announce itself by @rpath:\n{calls}");
}

/// THE ARCHIVE CARRIES EACH LIBRARY ONCE. OCCT installs every module under three names, one file and two
/// links; a `cp` that follows links wrote all three in full, and the mac download weighed 80 MB against
/// 33 and 35 for the other two systems. Only the name written into the dependencies is ever loaded.
#[test]
fn every_library_travels_once_and_its_other_names_are_links() {
    let dir = sandbox("weight", Deps::Rpath);
    let out = bundle(&dir);
    assert!(out.status.success(), "the script refused:\n{}", said(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains(">>> libraries: 2, links to them: 4"), "the two modules were not counted apart from their links:\n{}", said(&out));

    let long = listing(&dir, "-Z");
    let links = long.lines().filter(|l| l.starts_with('l') && l.contains(".dylib")).count();
    let files = long.lines().filter(|l| l.starts_with('-') && l.contains(".dylib")).count();
    assert_eq!((files, links), (2, 4), "each module must travel once, its other two names as links:\n{long}");

    // The id is set on the file, never through a link: writing through one would leave the real file
    // announcing whichever name came last.
    let calls = fs::read_to_string(dir.join("calls.txt")).expect("the tool was called");
    assert_eq!(calls.matches("-id @rpath/").count(), 2, "the tool was run on links as well as on files:\n{calls}");
}

/// The case the script was written for: paths name the build machine and are rewritten to `@rpath`.
#[test]
fn paths_naming_the_build_machine_are_rewritten() {
    let dir = sandbox("rewritten", Deps::BuildMachine);
    let out = bundle(&dir);
    assert!(out.status.success(), "the script refused although every path was rewritten:\n{}", said(&out));

    let calls = fs::read_to_string(dir.join("calls.txt")).expect("the tool was called");
    assert!(calls.contains("-change ") && calls.contains(" @rpath/libTKernel.7.8.dylib "), "no path was rewritten to @rpath:\n{calls}");
    assert!(calls.contains("MacOS/qymcad"), "the program itself was left naming the build machine:\n{calls}");
    assert!(archive(&dir).contains("QymCAD.app/Contents/MacOS/qymcad"));
}

/// THE SENTINEL MUST STILL FIRE. One path left naming the build machine means a program that starts here
/// and nowhere else, and says so only on somebody else's computer - so that must fail the build, loudly.
#[test]
fn a_path_left_naming_the_build_machine_fails_the_bundle() {
    let dir = sandbox("stubborn", Deps::Stubborn);
    let out = bundle(&dir);
    assert!(!out.status.success(), "a library still named the build machine and the script was happy:\n{}", said(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("still points at the build machine"), "the refusal did not say what was wrong:\n{}", said(&out));
    assert!(!dir.join("dist/qymcad-0.1.0-macos-arm64.zip").exists(), "an archive was made out of a bundle that cannot start");
}
