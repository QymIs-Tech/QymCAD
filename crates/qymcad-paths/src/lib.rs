//! WHERE THE PROGRAM KEEPS ITS OWN FILES.
//!
//! THE APPLICATION ID IS ONE STRING, and this is it. The same name identifies the program to the desktop,
//! to macOS and to Flathub. It was written out in six places in the code, plus the macOS bundle, plus the
//! Flatpak manifest - eight copies of one decision, and three of them had already drifted apart:
//! `tech.qymis.qym-cad` in the code, `tech.qymis.qymcad` on macOS, `tech.qymis.cad` on Flathub.
//!
//! THE FOLDER IS A SECOND DECISION, next to it and not derived from it - see `FOLDER` below for what
//! deriving it cost.
//!
//! CHANGING EITHER LOSES WHAT PEOPLE HAVE. The old directory does not move by itself: settings, schemes,
//! templates and the parts library stay behind under the old name and simply vanish from view. That is the
//! price, it was paid deliberately while the audience is two dev releases small, and it is the reason both
//! names live here rather than in eight files.

/// The reverse-DNS name of the program: `cad.qymis.tech` backwards, the project's own site.
pub const APP_ID: &str = "tech.qymis.cad";

/// THE NAME OF THE FOLDER, and it is NOT the last part of the id.
///
/// Reported behaviour: settings and crash reports turned up in `~/.local/share/cad`.
///
/// `ProjectDirs` is given three parts and on Linux uses ONLY the third. `tech.qymis.cad` split at the
/// dots ends with `cad`, so the program claimed a folder named after a whole field of software - a name
/// any other CAD may take tomorrow - and stopped finding what it had already written under its own.
///
/// These are two decisions, not one. The id says who the program is to the desktop, to macOS and to
/// Flathub; the folder says where a person's settings, schemes, templates and library of parts live.
/// Deriving the second from the first is exactly what put a stranger's name on the folder.
const FOLDER: &str = "qymcad";

/// The two parts above the name, taken from `APP_ID` so the id and the folder stay in one file.
fn parts() -> Option<(&'static str, &'static str)> {
    let mut it = APP_ID.split('.');
    Some((it.next()?, it.next()?))
}

/// The program's own directories, or `None` where the system has no notion of them.
pub fn dirs() -> Option<directories::ProjectDirs> {
    let (tld, org) = parts()?;
    directories::ProjectDirs::from(tld, org, FOLDER)
}

/// A ROOT THAT STANDS IN FOR THE PERSON'S DIRECTORIES, for the whole process once set.
static ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// KEEP EVERYTHING THE PROGRAM WRITES UNDER `root`, for the rest of this process: settings, schemes,
/// templates, the parts library, crash and problem reports.
///
/// A program driven from outside - a run of acceptance checks - is still the whole program, and a whole
/// program writes where a person keeps their files. One such run on a working machine replaced a person's
/// settings and filled their crash folder. Set once: a second root would split one process between two
/// homes, so a different root after the first is refused and the answer is `false`.
pub fn keep_under(root: std::path::PathBuf) -> bool {
    ROOT.get_or_init(|| root.clone()) == &root
}

/// THE ONE FOLDER A PERSON'S FILES ARE IN - settings, schemes, templates, parts, reports: under the root when one was
/// given, the system's data folder of the program otherwise (`~/.local/share/qymcad`, `%APPDATA%\qymis\qymcad\data`,
/// `~/Library/Application Support/tech.qymis.qymcad`). One folder on every system, not a "config" beside a "data".
/// Reported behaviour: half a person's files were in `~/.config/qymcad` and half in `~/.local/share/qymcad`.
pub fn data_root() -> Option<std::path::PathBuf> {
    if let Some(root) = ROOT.get() {
        return Some(root.clone());
    }
    dirs().map(|d| d.data_dir().to_path_buf())
}

/// THE FILE THE SETTINGS ARE KEPT IN.
///
/// Named here, with the rest of the program's places, because the framework picks one of its own otherwise -
/// derived from the application id, which is a different string. On Linux both happen to land in the same
/// folder and nothing looks wrong; on Windows they came out as `AppData\Roaming\qymcad\data` against
/// `AppData\Roaming\qymis\qymcad\data`, and on macOS as `qymcad` beside `tech.qymis.qymcad` - a person's
/// settings in one place and their schemes, templates, parts and crash reports in another.
pub fn settings_file() -> Option<std::path::PathBuf> {
    data_root().map(|d| d.join("app.ron"))
}

/// A directory of the person's own things that set how the program looks and starts, e.g. `schemes` or `templates` -
/// in the same one folder as the rest.
pub fn config(sub: &str) -> Option<std::path::PathBuf> {
    data_root().map(|d| d.join(sub))
}

/// A directory under the person's data, e.g. `crashes` or `library/parts`.
pub fn data(sub: &str) -> Option<std::path::PathBuf> {
    data_root().map(|d| d.join(sub))
}

/// THE FOLDER THE PROGRAM SITS IN, or `None` where it cannot be found.
///
/// This is not the data folder: it is the folder the executable itself lies in, which is where a person
/// looks when something has to be found beside the program rather than in a hidden place under their home.
///
/// A PACKAGED PROGRAM DOES NOT RUN FROM WHERE IT LIES. A self-contained image unpacks its executable into
/// a read-only mount that vanishes when the program closes, so the executable's own folder is a temporary
/// one; the variable that names the image file is asked first, and beside THAT file is where a file of ours
/// belongs. A plain build has no such variable - and then the executable is the program.
pub fn program_folder() -> Option<std::path::PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        if let Some(folder) = std::path::Path::new(&image).parent() {
            return Some(folder.to_path_buf());
        }
    }
    Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
}

/// A FOLDER BESIDE THE PROGRAM - `crashes`, `logs` - and `None` where a file could not be put there.
///
/// WHETHER IT CAN BE WRITTEN IS ASKED BY WRITING. A folder that exists is not a folder that can be written
/// in: a system directory, a mounted image and a read-only home all answer yes to a question about the
/// folder, and only a file tells the truth. The probe is taken straight back out, so it leaves nothing for a
/// person to find and account for.
///
/// Decided ONCE per process: the answer is asked at every start and the folder does not change underneath
/// the program while it runs.
pub fn beside_program(sub: &str) -> Option<std::path::PathBuf> {
    static CHOSEN: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    CHOSEN
        .get_or_init(|| {
            let folder = program_folder()?;
            can_be_written_to(&folder).then(|| folder.join(sub))
        })
        .clone()
}

/// Whether a file could be written into `folder` and taken out again.
pub fn can_be_written_to(folder: &std::path::Path) -> bool {
    let probe = folder.join(".qymcad-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    /// THE FOLDER IS NAMED AFTER THE PROGRAM, and this is asked of the finished path, not of the strings.
    ///
    /// The check that stood here compared the three parts against the id and was green while the program
    /// was writing into `~/.local/share/cad`: it asked whether the folder was DERIVED from the id, which
    /// was true, instead of what the folder came out as. So this one builds the path and reads it.
    #[test]
    fn the_folder_is_named_after_the_program() {
        let Some(d) = super::dirs() else {
            return; // a system without a notion of per-user directories has nothing to check
        };
        for dir in [d.config_dir(), d.data_dir()] {
            assert!(
                dir.to_string_lossy().to_lowercase().contains(super::FOLDER),
                "the program's own directory does not carry the program's name: {}",
                dir.display()
            );
            assert!(
                !dir.components().any(|c| c.as_os_str() == "cad"),
                "the folder is named after the last part of the id again, and any other CAD may claim it: {}",
                dir.display()
            );
        }
    }

    /// A FOLDER THAT CANNOT BE WRITTEN IN IS REFUSED, so a file of ours goes to the data folder instead of
    /// being lost.
    ///
    /// THE PROBE IS WRITTEN AND TAKEN OUT AGAIN, not merely asked about: a system directory, a mounted
    /// image and a read-only home all answer "yes, it is a folder" to a question about the folder, and only
    /// a write tells the truth. The refused folder here is a FILE with a child asked for - the shape of
    /// every case where the answer is no, including the one nobody can prepare for: a folder that exists,
    /// looks free, and is not.
    #[test]
    fn a_folder_that_cannot_be_written_in_is_refused() {
        let root = std::env::temp_dir().join(format!("qymcad-write-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a folder of its own for the check");
        assert!(super::can_be_written_to(&root), "a folder in the temporary directory refused a file, and there is nothing wrong with it");

        let file = root.join("not-a-folder");
        std::fs::write(&file, b"x").expect("a plain file is written");
        assert!(!super::can_be_written_to(&file), "a file was taken for a folder our files could be written into - the file would be lost");
        assert!(!super::can_be_written_to(&file.join("deeper")), "a folder under a file was taken for one that can be written into");

        // THE PROBE LEAVES NOTHING BEHIND: a file of its own in the program's folder would be a thing the
        // person finds and cannot account for.
        let left: Vec<String> = std::fs::read_dir(&root)
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        assert_eq!(left, vec!["not-a-folder".to_string()], "the check left files in a folder it had borrowed: {left:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// THE FOLDER BESIDE THE PROGRAM IS A SUBFOLDER OF IT, not the folder itself - a crash report or a log
    /// lands in a file of its own and cannot be mistaken for something the program was made of.
    #[test]
    fn a_file_of_ours_goes_into_a_folder_of_its_own_beside_the_program() {
        let Some(dir) = super::beside_program("crashes") else {
            return; // a place that cannot be written to: the fallback is the data folder, and there is nothing to compare
        };
        let Some(here) = super::program_folder() else { return };
        assert_eq!(dir.parent(), Some(here.as_path()), "a file of ours is not beside the program: {} against {}", dir.display(), here.display());
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("crashes"), "our files share a folder with the program itself: {}", dir.display());
    }
}
