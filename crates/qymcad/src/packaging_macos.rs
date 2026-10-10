//! Running `packaging/macos/bundle.sh` here, with the two mac-only tools replaced by stubs.
//!
//! WHY THIS EXISTS. That script runs on one runner, the most expensive of the three, at the very END of a
//! build - so a mistake in it costs a whole build to find, and is found one line at a time. The first run
//! died before it, on the link. The second died INSIDE it after six minutes and printed nothing at all:
//! `grep` matched no library naming the build machine, `grep` returns 1 when it matches nothing, and
//! `set -o pipefail` turned that into the end of the script.
//!
//! `install_name_tool`, `otool` and `codesign` exist only on macOS, `rcodesign` and `dmgbuild` only where they are
//! installed.
//! They are replaced by stubs on PATH - one records what it was asked to do and remembers which files it
//! rewrote, another prints a dependency list in the real format and answers according to those marks, the
//! signing two record their calls and seal the bundle, `codesign --verify` answers by that seal, and `dmgbuild`
//! records what the folder it was given held and writes the image file.
//! Everything the mistakes were actually in - the copying, the loops, the sentinel, the order of signing,
//! the archive, the disk image's folder - is ordinary shell and runs anywhere.

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

/// What signing the bundle does to it, as `codesign --verify` sees it afterwards.
#[derive(Clone, Copy, PartialEq)]
enum Seal {
    /// Signing the `.app` seals it, and the check passes - what the real tools do.
    Holds,
    /// The `.app` is signed and still does not verify - the state the CI package was shipped in.
    Breaks,
}

/// Who signs the bundle.
#[derive(Clone, Copy, PartialEq)]
enum Signer {
    /// No certificate given: `codesign`, ad hoc.
    AdHoc,
    /// A .p12 and its password file given: `rcodesign`.
    Certificate,
}

/// A tree that looks enough like the repository for the script: the binary, the icon, the licence, the
/// notices, a manifest with a version, and an OCCT installation of two modules under three names each -
/// `libTKernel.dylib` -> `libTKernel.7.8.dylib` -> `libTKernel.7.8.1.dylib`, exactly as OCCT installs.
fn sandbox(case: &str, deps: Deps, seal: Seal) -> PathBuf {
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
    executable(&write("bin/install_name_tool", &format!("#!/usr/bin/env bash\nMARKS={marks}\nprintf '%s\\n' \"$*\" >> {calls}\n{records}exit 0\n", marks = marks.display(), calls = calls.display())));

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

    // THE SIGNING TOOLS share one log and one seal. Signing the `.app` itself is what seals it, as with the
    // real tools; `codesign --verify` passes only on a sealed bundle and otherwise says what the CI package
    // said. Signing a library leaves the seal alone - only the order is checked, from the log.
    let (signs, sealed) = (dir.join("signs.txt"), dir.join("sealed"));
    let seals = if seal == Seal::Holds { format!(": > {}", sealed.display()) } else { ":".to_string() };
    executable(&write(
        "bin/codesign",
        &format!(
            "#!/usr/bin/env bash
printf 'codesign %s\\n' \"$*\" >> {signs}
\
             case \" $* \" in *' --verify '*)
  [ -e {sealed} ] && exit 0
  \
             echo 'QymCAD.app: code has no resources but signature indicates they must be present' >&2
  exit 1 ;;
esac
\
             case \"${{@: -1}}\" in *.app) {seals} ;; esac
exit 0
",
            signs = signs.display(),
            sealed = sealed.display(),
            seals = seals
        ),
    ));
    executable(&write(
        "bin/rcodesign",
        &format!(
            "#!/usr/bin/env bash
printf 'rcodesign %s\\n' \"$*\" >> {signs}
case \"${{@: -1}}\" in *.app) {seals} ;; esac
exit 0
",
            signs = signs.display(),
            seals = seals
        ),
    ));

    // `dmgbuild -s SETTINGS -D stage=DIR -D art=DIR VOLUME FILE` lays the staged folder out by the settings
    // file and writes the image. The stub writes down what the staged folder held at the moment of the call -
    // the script removes it afterwards - refuses a settings file that is not there, and logs the call beside
    // the signing ones, so its place after the check is visible.
    executable(&write(
        "bin/dmgbuild",
        &format!(
            "#!/usr/bin/env bash
printf 'dmgbuild %s\\n' \"$*\" >> {signs}
\
             settings=; stage=; prev=
for a in \"$@\"; do
  [ \"$prev\" = -s ] && settings=$a
  \
             [ \"$prev\" = -D ] && case \"$a\" in stage=*) stage=${{a#stage=}} ;; esac
  prev=$a
done
\
             [ -f \"$settings\" ] || {{ echo \"no settings file: $settings\" >&2; exit 1; }}
\
             (cd \"$stage\" && ls -1) > {held}
: > \"${{@: -1}}\"
exit 0
",
            signs = signs.display(),
            held = dir.join("dmg-held.txt").display()
        ),
    ));
    dir
}

/// Run the real script over the sandbox. HOME is moved inside it as well: the script asks git to trust the
/// directory it is in, and that must not reach the settings of whoever runs the tests.
fn bundle(dir: &Path, signer: Signer) -> Output {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/bundle.sh");
    let path = format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    let mut run = Command::new("bash");
    run.arg(&script)
        .current_dir(dir)
        .env("PATH", path)
        .env("HOME", dir)
        .env("OCCT_ROOT", dir.join("occt"))
        .env_remove("QYMCAD_VERSION")
        .env_remove("MACOS_SIGN_P12")
        .env_remove("MACOS_SIGN_P12_PASSWORD_FILE");
    if signer == Signer::Certificate {
        run.env("MACOS_SIGN_P12", dir.join("signing.p12")).env("MACOS_SIGN_P12_PASSWORD_FILE", dir.join("signing.pw"));
    }
    run.output().expect("bash runs the packaging script")
}

/// What the signing tools were asked to do, one call per line, in order.
fn signs(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("signs.txt")).unwrap_or_default().lines().map(str::to_string).collect()
}

fn said(out: &Output) -> String {
    format!("--- stdout ---\n{}--- stderr ---\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// The disk image the script makes beside the archive.
fn disk_image(dir: &Path) -> PathBuf {
    dir.join("dist/qymcad-0.1.0-macos-arm64.dmg")
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
    let dir = sandbox("rpath", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(out.status.success(), "the script refused although every path was already @rpath:\n{}", said(&out));

    let held = archive(&dir);
    for entry in [
        "QymCAD.app/Contents/MacOS/qymcad",
        "QymCAD.app/Contents/Info.plist",
        "QymCAD.app/Contents/Resources/qymcad.icns",
        "QymCAD.app/Contents/Resources/LICENSE.txt",
        "QymCAD.app/Contents/Resources/THIRD-PARTY-NOTICES.md",
        "README.txt",
    ] {
        assert!(held.contains(entry), "the archive does not hold {entry}:\n{held}");
    }
    // A note per language of the program travels; three are named in an alphabet this file does not spell out.
    assert_eq!(held.lines().filter(|l| l.ends_with(".txt") && !l.contains("Contents/")).count(), 4, "all four notes must be in the archive:\n{held}");

    let calls = fs::read_to_string(dir.join("calls.txt")).expect("the tool was called");
    assert!(calls.contains("-add_rpath @executable_path/../Frameworks"), "the bundle was not pointed at its own Frameworks:\n{calls}");
    assert!(calls.contains("-id @rpath/libTKernel.7.8.1.dylib"), "a library was not made to announce itself by @rpath:\n{calls}");
}

/// THE ARCHIVE CARRIES EACH LIBRARY ONCE. OCCT installs every module under three names, one file and two
/// links; a `cp` that follows links wrote all three in full, and the mac download weighed 80 MB against
/// 33 and 35 for the other two systems. Only the name written into the dependencies is ever loaded.
#[test]
fn every_library_travels_once_and_its_other_names_are_links() {
    let dir = sandbox("weight", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
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
    let dir = sandbox("rewritten", Deps::BuildMachine, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
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
    let dir = sandbox("stubborn", Deps::Stubborn, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(!out.status.success(), "a library still named the build machine and the script was happy:\n{}", said(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("still points at the build machine"), "the refusal did not say what was wrong:\n{}", said(&out));
    assert!(!dir.join("dist/qymcad-0.1.0-macos-arm64.zip").exists(), "an archive was made out of a bundle that cannot start");
    assert!(!disk_image(&dir).exists(), "a disk image was made out of a bundle that cannot start");
}

/// THE BUNDLE IS SEALED, INSIDE OUT. The CI package carried only the linker's signature on the program and
/// none on the bundle, and macOS called the download damaged. Each library is signed before the bundle,
/// because sealing the bundle records the signatures of the code inside it; links are not signed twice
/// through their other names; the bundle is verified before it is archived.
#[test]
fn the_bundle_is_signed_after_its_libraries_and_verified() {
    let dir = sandbox("signed", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(out.status.success(), "the script refused:\n{}", said(&out));

    let calls = signs(&dir);
    let signing = |c: &&String| c.starts_with("codesign ") && c.contains("--sign -");
    let libraries: Vec<&str> = calls.iter().filter(signing).flat_map(|c| c.split_whitespace()).filter(|w| w.ends_with(".dylib")).collect();
    assert_eq!(libraries.len(), 2, "each module must be signed once, through its file and not its links:\n{calls:#?}");
    assert!(libraries.iter().all(|l| l.ends_with(".7.8.1.dylib")), "a link was signed instead of the file:\n{calls:#?}");

    let at = |test: &dyn Fn(&String) -> bool| calls.iter().position(test);
    let lib = at(&|c| signing(&c) && c.contains(".dylib"));
    let app = at(&|c| signing(&c) && c.ends_with("QymCAD.app"));
    let verify = at(&|c| c.starts_with("codesign --verify") && c.ends_with("QymCAD.app"));
    assert!(matches!((lib, app, verify), (Some(l), Some(a), Some(v)) if l < a && a < v), "the libraries, then the bundle, then the check - in that order:\n{calls:#?}");
    assert!(archive(&dir).contains("QymCAD.app/Contents/MacOS/qymcad"));
}

/// A CERTIFICATE SIGNS THROUGH `rcodesign`. `codesign` refuses a self-signed certificate the system does not
/// trust, so with a .p12 given the whole bundle goes to `rcodesign`, with the file and its password file.
#[test]
fn a_certificate_signs_the_bundle_through_rcodesign() {
    let dir = sandbox("certificate", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::Certificate);
    assert!(out.status.success(), "the script refused:\n{}", said(&out));

    let calls = signs(&dir);
    let p12 = dir.join("signing.p12");
    let pw = dir.join("signing.pw");
    let wanted = format!("--p12-file {} --p12-password-file {}", p12.display(), pw.display());
    assert!(calls.iter().any(|c| c.starts_with("rcodesign sign ") && c.contains(&wanted) && c.ends_with("QymCAD.app")), "the bundle was not signed with the certificate:\n{calls:#?}");
    assert!(!calls.iter().any(|c| c.contains("--sign")), "codesign signed besides the certificate:\n{calls:#?}");
    assert!(calls.iter().any(|c| c.starts_with("codesign --verify")), "the signed bundle was not verified:\n{calls:#?}");
}

/// A BUNDLE THAT DOES NOT VERIFY IS NOT SHIPPED. That is the "damaged" message, found by whoever downloads
/// the package; the build must stop on it instead.
#[test]
fn a_bundle_that_does_not_verify_is_not_archived() {
    let dir = sandbox("broken", Deps::Rpath, Seal::Breaks);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(!out.status.success(), "a bundle that does not verify was let through:\n{}", said(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("code has no resources"), "the refusal did not say what was wrong:\n{}", said(&out));
    assert!(!dir.join("dist/qymcad-0.1.0-macos-arm64.zip").exists(), "an archive was made out of a bundle that does not verify");
    assert!(!disk_image(&dir).exists(), "a disk image was made out of a bundle that does not verify");
}

/// THE DISK IMAGE, BESIDE THE ARCHIVE. It holds the program and the four notes, laid out by the settings
/// file beside the script, and it is made from the bundle only after that bundle verified. The staging
/// folder is gone afterwards, so `dist/` holds the packages and the bundle and nothing half-made.
#[test]
fn a_disk_image_holds_the_verified_bundle_and_the_notes() {
    let dir = sandbox("dmg", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(out.status.success(), "the script refused:\n{}", said(&out));
    assert!(disk_image(&dir).exists(), "the disk image was not made:\n{}", said(&out));
    assert!(archive(&dir).contains("QymCAD.app/Contents/MacOS/qymcad"), "the archive stopped being made beside the image");

    let held = fs::read_to_string(dir.join("dmg-held.txt")).expect("dmgbuild was called");
    let lines: Vec<&str> = held.lines().collect();
    for entry in ["QymCAD.app", "README.txt"] {
        assert!(lines.contains(&entry), "the image's folder does not hold {entry}:\n{held}");
    }
    assert_eq!(lines.iter().filter(|l| l.ends_with(".txt")).count(), 4, "all four notes must be in the image:\n{held}");

    let calls = signs(&dir);
    let verify = calls.iter().position(|c| c.starts_with("codesign --verify"));
    let image = calls.iter().position(|c| c.starts_with("dmgbuild -s ") && c.contains("/dmg/settings.py") && c.ends_with(" QymCAD dist/qymcad-0.1.0-macos-arm64.dmg"));
    assert!(matches!((verify, image), (Some(v), Some(i)) if v < i), "the image must be made from the bundle after it verified:\n{calls:#?}");
    assert!(!dir.join("dist/dmg").exists(), "the staging folder was left in dist/");
}

/// THE FIRST LAUNCH GOES THROUGH SYSTEM SETTINGS. With a signature that verifies, macOS refuses the first
/// launch and then offers "Open Anyway" under Privacy & Security; the notes lead there. Reported behaviour:
/// the notes sent the reader to Terminal and `xattr -cr`, the way round a broken signature, now outdated.
#[test]
fn the_notes_lead_to_open_anyway_not_to_a_terminal() {
    let dir = sandbox("notes", Deps::Rpath, Seal::Holds);
    let out = bundle(&dir, Signer::AdHoc);
    assert!(out.status.success(), "the script refused:\n{}", said(&out));

    let en = fs::read_to_string(dir.join("dist/README.txt")).expect("the English note is written");
    for step in ["Privacy & Security", "Open Anyway", "Applications"] {
        assert!(en.contains(step), "the English note does not name {step:?}:\n{en}");
    }
    // The other three are named and written in alphabets this file does not spell out, so each is held to the
    // English one's shape: the same four steps, Touch ID, the menu path written with `->`, and the system's
    // button names in English beside its own words - the note's language says nothing of the system's.
    let notes: Vec<String> = fs::read_dir(dir.join("dist"))
        .expect("dist/ is readable")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| fs::read_to_string(p).expect("a note is readable"))
        .collect();
    assert_eq!(notes.len(), 4, "a note per language of the program: English, Russian, Ukrainian, Kazakh");
    for text in &notes {
        for step in ["  1. ", "  2. ", "  3. ", "  4. ", "Touch ID", " -> ", "Privacy & Security", "Open Anyway"] {
            assert!(text.contains(step), "a note lacks {step:?}:\n{text}");
        }
        assert!(!text.contains("xattr") && !text.contains("Terminal"), "a note still sends the reader to a terminal:\n{text}");
    }
}

/// The width and height a PNG file declares in its header.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{} is not readable: {e}", path.display()));
    assert_eq!(&bytes[1..4], b"PNG", "{} is not a PNG", path.display());
    let at = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().expect("four bytes"));
    (at(16), at(20))
}

/// One icon of the window, as the settings file places it.
struct Placed {
    name: String,
    x: f64,
    y: f64,
}

/// THE PICTURE AND THE PLACES AGREE. The background is painted around the icons - the arrow between the
/// program and Applications, the card under the notes - from the numbers in settings.py. A picture of the
/// wrong size is stretched by nobody and cropped by Finder; an icon moved off its place sits on the arrow.
/// The settings file is executed by python3 the way dmgbuild executes it.
///
/// A file is matched to its place by its name as HFS+ stores it, decomposed (NFD): the probe prints the files
/// decomposed and the places as written, so a place keyed by a composed letter with a breve finds no file -
/// as the Ukrainian note found none on the screen. The picture runs on under the status bar's strip, so it is that much taller.
#[test]
fn the_window_layout_fits_its_background() {
    let art = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/dmg");
    let probe = "import sys\n\
                 scope = {'defines': {'stage': 'S', 'art': sys.argv[2]}}\n\
                 exec(compile(open(sys.argv[1], encoding='utf-8').read(), 'settings.py', 'exec'), scope, scope)\n\
                 import unicodedata\n\
                 print(scope['WIDTH'], scope['HEIGHT'], scope['STATUS_BAR'], scope['icon_size'])\n\
                 for n, (x, y) in scope['icon_locations'].items(): print(n, x, y, sep='\\t')\n\
                 print('files', *(unicodedata.normalize('NFD', f) for f in scope['files']), sep='\\t')\n\
                 print('symlinks', *scope['symlinks'], sep='\\t')\n";
    let out = Command::new("python3").arg("-c").arg(probe).arg(art.join("settings.py")).arg(&art).output().expect("python3 runs");
    assert!(out.status.success(), "settings.py does not execute:\n{}", said(&out));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut lines = text.lines();
    let size: Vec<f64> = lines.next().expect("the window size").split(' ').map(|v| v.parse().expect("a number")).collect();
    let (width, height, strip, icon) = (size[0], size[1], size[2], size[3]);
    let mut placed = Vec::new();
    let mut shown = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        match cells[0] {
            "files" => shown.extend(cells[1..].iter().map(|f| f.trim_start_matches("S/").to_string())),
            "symlinks" => shown.extend(cells[1..].iter().map(|s| s.to_string())),
            _ => placed.push(Placed { name: cells[0].to_string(), x: cells[1].parse().expect("x"), y: cells[2].parse().expect("y") }),
        }
    }

    let picture = (width as u32, (height + strip) as u32);
    assert_eq!(png_size(&art.join("background.png")), picture, "background.png is not the window's size");
    assert_eq!(png_size(&art.join("background@2x.png")), (2 * picture.0, 2 * picture.1), "background@2x.png is not twice the window's size");

    for name in &shown {
        assert!(placed.iter().any(|p| &p.name == name), "{name} goes into the image with no place in the window:\n{text}");
    }
    let half = icon / 2.0;
    for p in &placed {
        // The label hangs under the icon, about 20 points of 13-point text.
        assert!(p.x - half >= 0.0 && p.x + half <= width && p.y - half >= 0.0 && p.y + half + 20.0 <= height, "{} runs off the window:\n{text}", p.name);
    }
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            assert!((a.x - b.x).abs() >= icon || (a.y - b.y).abs() >= icon + 20.0, "{} and {} overlap:\n{text}", a.name, b.name);
        }
    }
}

/// THE HEADER STANDS ON THE WINDOW'S CENTRE. The logo, the name and the line under it are one group above the
/// icons; centred on the name alone, the group stood 22 points right of the centre, the line under the name
/// being wider. Measured here on the picture itself: the columns holding anything darker than the pale ground,
/// above the icons' row, must be centred within a point.
#[test]
fn the_header_is_centred_in_the_window() {
    let art = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/dmg");
    let probe = "scope = {'defines': {'stage': '', 'art': ''}}\n\
                 exec(compile(open(__import__('sys').argv[1], encoding='utf-8').read(), 'settings.py', 'exec'), scope, scope)\n\
                 print(scope['APP'][1] - scope['icon_size'] / 2)\n";
    let out = Command::new("python3").arg("-c").arg(probe).arg(art.join("settings.py")).output().expect("python3 runs");
    assert!(out.status.success(), "settings.py does not execute:\n{}", said(&out));
    let icons_top: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().expect("the top of the icons' row");

    let picture = image::open(art.join("background@2x.png")).expect("background@2x.png decodes").to_rgb8();
    let band = (icons_top * 2.0) as u32 - 20;
    let inked: Vec<u32> = (0..picture.width()).filter(|&x| (0..band).any(|y| picture.get_pixel(x, y).0.iter().any(|&c| c < 200))).collect();
    let (first, last) = (inked.first().copied().expect("the header has ink"), inked.last().copied().expect("the header has ink"));
    let off = (f64::from(first + last) / 2.0 - f64::from(picture.width()) / 2.0) / 2.0;
    assert!(off.abs() <= 1.0, "the header is {off:.1} points off the window's centre: ink from {first} to {last} of {} pixels", picture.width());
}
