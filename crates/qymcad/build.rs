// Embedding resources into the .exe. This matters ONLY when building for Windows: winresource calls the
// resource compiler of whichever toolchain is in use and stitches the icon into the binary. On Linux and
// macOS the whole block is cut out by cfg(windows), and the winresource dependency is not pulled in there
// (it is target-scoped in Cargo.toml).
fn main() {
    stamp_the_build();

    // The embedded catalogue of the parts library: drop a `.qpart` or a category into `library/` and
    // a rebuild stitches them into the binary (`include_dir!` in parts_library.rs). The path is
    // relative to the crate directory.
    println!("cargo:rerun-if-changed=../../library");
    // LOCALISATIONS: put a language directory into `i18n/` and a rebuild stitches it in
    // (`include_dir!` in i18n.rs). Without this line cargo does not treat the directory as an input:
    // a contributor adds a folder, builds, and NOTHING happens, because the binary was not rebuilt. A
    // silent refusal exactly where a person first tries to contribute is the worst possible place.
    println!("cargo:rerun-if-changed=../../i18n");
    // HELP: add or edit a `.md` in `docs/help/` and a rebuild stitches it in (`include_dir!` in
    // help.rs). The reason is the same as for the languages: without this line an edit to an article
    // never reaches the binary, and its author sees the old text without understanding why.
    println!("cargo:rerun-if-changed=../../docs/help");

    #[cfg(windows)]
    {
        // the path is relative to the crate directory (the CWD of build.rs), pointing at the repository assets
        const ICON: &str = "../../assets/icons/windows/qymcad.ico";
        println!("cargo:rerun-if-changed={ICON}");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ICON);
        if let Err(e) = res.compile() {
            // do not fail the build over an icon, only warn (the exe builds without it)
            println!("cargo:warning=could not stitch the icon into the .exe: {e}");
        }
    }
}

/// WHICH BUILD IS THIS? A report from a person carries a screenshot and a complaint, and the first
/// question is always the same - what was it built from. With a build a day, "version 0.1.0" answers
/// nothing: what identifies it is the commit. Three values are stamped in here and read back through
/// `option_env!`, so a build with no git at hand (an unpacked source archive) still compiles and simply
/// says less.
///
/// The dirty flag matters as much as the hash: a binary built over uncommitted edits corresponds to no
/// commit at all, and a report from it must not be chased against the tree of that hash.
fn stamp_the_build() {
    // Without this cargo does not know the stamp depends on the commit: switch branches, rebuild, and the
    // binary would keep claiming the old hash. HEAD alone is enough - it changes on a checkout as well as on
    // a commit. The path is only declared when it exists: naming a missing file makes cargo consider it
    // changed every single time, and the crate would rebuild on every run.
    let head = std::path::Path::new("../../.git/HEAD");
    if head.exists() {
        println!("cargo:rerun-if-changed=../../.git/HEAD");
    }

    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };

    if let Some(hash) = git(&["rev-parse", "--short=9", "HEAD"]) {
        println!("cargo:rustc-env=QYMCAD_GIT_HASH={hash}");
    }
    if let Some(date) = git(&["log", "-1", "--date=format:%Y-%m-%d", "--format=%cd"]) {
        println!("cargo:rustc-env=QYMCAD_COMMIT_DATE={date}");
    }
    // `--porcelain` prints one line per changed file and nothing at all on a clean tree.
    if let Some(_dirty) = git(&["status", "--porcelain", "--untracked-files=no"]) {
        println!("cargo:rustc-env=QYMCAD_GIT_DIRTY=1");
    }
}
