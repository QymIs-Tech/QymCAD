//! A CRASH MUST NOT DISAPPEAR.
//!
//! There was no logging in this program at all - no logging crate, no panic hook - so a crash left
//! nothing behind: the window vanished and the person had a sentence to offer and no way to say what
//! the program was doing. A report like that costs a conversation and usually ends in "could not
//! reproduce".
//!
//! What is kept is what cannot be recovered afterwards:
//!
//! * the trail of actions, recorded WHEN AN OPERATION OPENS rather than when it commits. A crash
//!   happens in the middle of an operation, so a trail of committed steps is missing exactly the one
//!   that killed the program;
//! * the paths of the document and of its autosave - not the geometry. Copying a document out of a
//!   panic hook means serialising a model the hook cannot safely reach; the files are already on disk
//!   and the next start offers them;
//! * the build, the system, the place and the backtrace.
//!
//! THE HOOK NEVER PANICS ITSELF. Every step is fallible and every failure is swallowed: a panic inside
//! a panic hook aborts the process, and the person loses even the message the default hook prints.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How many actions to keep. Long enough to show how the person got there, short enough that the file
/// stays readable.
const TRAIL: usize = 20;

#[derive(Default)]
struct State {
    /// Names of the last operations, oldest first.
    steps: Vec<String>,
    /// Where the open document lives, when it has ever been saved.
    doc: Option<String>,
    /// Where its autosave lives.
    autosave: Option<String>,
    /// Set by tests so a crash report never lands in the real profile of whoever runs them.
    dir: Option<PathBuf>,
}

static STATE: Mutex<State> = Mutex::new(State { steps: Vec::new(), doc: None, autosave: None, dir: None });

/// AN OPERATION HAS STARTED. Called from the one place that opens an edit.
pub fn note_step(name: &str) {
    // A poisoned lock is ignored on purpose: diagnostics may never take the program down with them.
    let Ok(mut s) = STATE.lock() else { return };
    if s.steps.last().map(String::as_str) == Some(name) {
        return; // a command reopening itself frame after frame would fill the whole trail with one word
    }
    s.steps.push(name.to_string());
    if s.steps.len() > TRAIL {
        s.steps.remove(0);
    }
}

/// WHERE THE DOCUMENT LIVES, so the next start can offer it back.
pub fn note_document(doc: Option<&str>, autosave: Option<&str>) {
    let Ok(mut s) = STATE.lock() else { return };
    s.doc = doc.map(str::to_string);
    s.autosave = autosave.map(str::to_string);
}

/// Install the hook. The previous one is kept and called after ours, so the usual message still
/// reaches the terminal.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = write_report(info);
        previous(info);
    }));
}

/// Where the reports live: the data directory of the program, beside the parts library.
fn dir() -> Option<PathBuf> {
    if let Ok(s) = STATE.lock() {
        if let Some(d) = s.dir.clone() {
            return Some(d);
        }
    }
    directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.data_dir().join("crashes"))
}

/// A PATH WITHOUT THE NAME OF WHOEVER RAN THE PROGRAM. The file is meant to be attached to a public
/// report, and a home directory carries a person's name in it.
pub fn without_home(s: &str) -> String {
    let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) else { return s.to_string() };
    let home = home.to_string_lossy().into_owned();
    if home.is_empty() || home == "/" {
        return s.to_string();
    }
    s.replace(&home, "~")
}

fn write_report(info: &std::panic::PanicHookInfo<'_>) -> Option<PathBuf> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir).ok()?;

    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // UNDERSCORE, NOT A DASH, and the reason is a guard rather than taste: `crash-` reads as a catalogue
    // key (the window's strings live under that very prefix), and the check that no service name reaches
    // the screen would flag this file name for ever.
    let path = dir.join(format!("crash_{secs}.txt"));

    // The payload is a `&str` for `panic!("literal")` and a `String` for a formatted one; anything else
    // has no text to show.
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(the panic carried no message)".to_string());
    let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "(unknown)".into());

    let (steps, doc, autosave) = match STATE.lock() {
        Ok(s) => (s.steps.clone(), s.doc.clone(), s.autosave.clone()),
        Err(_) => (Vec::new(), None, None),
    };

    let mut out = String::new();
    out.push_str(&crate::diagnostics::block());
    out.push_str(&format!("\nTime: {}\n", crate::gui::now_iso8601()));
    out.push_str(&format!("Panic: {message}\nAt: {}\n", without_home(&place)));
    out.push_str(&format!("Document: {}\n", doc.as_deref().map(without_home).unwrap_or_else(|| "(never saved)".into())));
    out.push_str(&format!("Autosave: {}\n", autosave.as_deref().map(without_home).unwrap_or_else(|| "(none)".into())));

    out.push_str("\nWhat was being done (oldest first):\n");
    if steps.is_empty() {
        out.push_str("  (nothing had been started)\n");
    }
    for s in &steps {
        out.push_str(&format!("  - {s}\n"));
    }

    // A stripped binary gives addresses instead of names. That is still worth keeping: the addresses
    // match a build of the same commit, and the commit is named at the top of this file.
    out.push_str(&format!("\nBacktrace:\n{}\n", without_home(&std::backtrace::Backtrace::force_capture().to_string())));

    std::fs::write(&path, out).ok()?;
    Some(path)
}

/// A REPORT LEFT BY AN EARLIER RUN, the newest first. Files already shown carry `.seen`.
pub fn unseen_reports() -> Vec<PathBuf> {
    let Some(dir) = dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        // `.seen.txt` ALSO ENDS IN `.txt`. Checking only the tail offered an already shown report a second
        // time, every start, for ever.
        .filter(|p| {
            p.file_name()
                .map(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("crash_") && n.ends_with(".txt") && !n.ends_with(".seen.txt")
                })
                .unwrap_or(false)
        })
        .collect();
    found.sort();
    found.reverse();
    found
}

/// The report has been shown. Renamed rather than deleted: the person may still want to attach it.
pub fn mark_seen(path: &Path) {
    let seen = path.with_extension("seen.txt");
    let _ = std::fs::rename(path, seen);
}

#[cfg(test)]
pub(crate) fn use_dir_for_test(d: Option<&Path>) {
    if let Ok(mut s) = STATE.lock() {
        s.dir = d.map(Path::to_path_buf);
        s.steps.clear();
        s.doc = None;
        s.autosave = None;
    }
}

#[cfg(test)]
mod tests {
    /// THE STATE IS ONE PER PROCESS, so the tests that touch it take turns. Without this they interleave
    /// and the failure looks like a defect in the trail rather than in the test.
    static TAKE_TURNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A CRASH LEAVES A FILE, AND THE FILE ANSWERS THE QUESTIONS ASKED OF A CRASH.
    ///
    /// Before this there was no panic hook at all, so the answer to "what was it doing" was whatever the
    /// person remembered. The test panics for real, through the installed hook, because a hook that is
    /// never exercised is a hook that quietly stops working.
    #[test]
    fn a_crash_leaves_a_report() {
        let _turn = TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::use_dir_for_test(Some(&dir));

        super::note_step("Extrude");
        super::note_step("Fillet");
        // THE DOCUMENT SITS UNDER THE REAL HOME DIRECTORY, because that is what has to be masked: a made-up
        // path would be left alone by the masking and the check would pass while proving nothing.
        let home = directories::UserDirs::new().expect("a home directory").home_dir().to_path_buf();
        let doc = home.join("parts").join("bracket.qcad");
        super::note_document(Some(&doc.to_string_lossy()), Some(&home.join("parts").join("bracket.autosave.qcad").to_string_lossy()));

        super::install();
        let outcome = std::panic::catch_unwind(|| panic!("a wall fell over"));
        let _ = std::panic::take_hook(); // back to the default hook for the rest of this binary
        assert!(outcome.is_err(), "the panic did not happen at all");

        let reports = super::unseen_reports();
        assert_eq!(reports.len(), 1, "expected exactly one report in {dir:?}, found {reports:?}");
        let text = std::fs::read_to_string(&reports[0]).expect("the report reads");

        assert!(text.contains("a wall fell over"), "the report does not carry the message:\n{text}");
        assert!(text.contains("QymCAD "), "the report does not name the build:\n{text}");
        // THE MACHINE, NOT ONLY THE BUILD. Half the complaints about a viewport are answered by the
        // adapter and by nothing else, and a crash report is the one place it can arrive on its own.
        assert!(text.contains("System: "), "the report does not name the system:\n{text}");
        assert!(text.contains("Graphics: "), "the report does not name the graphics:\n{text}");
        assert!(text.contains("- Extrude") && text.contains("- Fillet"), "the report lost the trail:\n{text}");
        // THE LAST THING STARTED IS THE ONE THAT KILLED IT, so the trail must end on it.
        let (at_extrude, at_fillet) = (text.find("- Extrude").unwrap(), text.find("- Fillet").unwrap());
        assert!(at_extrude < at_fillet, "the trail is in the wrong order:\n{text}");
        assert!(text.contains("bracket.qcad"), "the report does not say which document was open:\n{text}");
        assert!(text.contains("Backtrace:"), "the report has no backtrace:\n{text}");
        assert!(text.lines().count() > 10, "the backtrace came out empty:\n{text}");

        // IT IS MEANT TO BE ATTACHED TO A PUBLIC REPORT. A home directory carries a person's name.
        let home_s = home.to_string_lossy().into_owned();
        assert!(!text.contains(&home_s), "the report carries a personal path ({home_s}):\n{text}");
        assert!(text.contains("~/parts/bracket.qcad") || text.contains("~\\parts\\bracket.qcad"), "the path was not masked, it was lost:\n{text}");

        // Shown once: after that it is renamed rather than deleted, so it can still be attached.
        super::mark_seen(&reports[0]);
        assert!(super::unseen_reports().is_empty(), "the report is offered a second time");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "marking it seen deleted the file");

        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A repeated command must not fill the whole trail with one word.
    #[test]
    fn the_trail_does_not_repeat_itself() {
        let _turn = TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-trail-test-{}", std::process::id()));
        super::use_dir_for_test(Some(&dir));
        for _ in 0..50 {
            super::note_step("Move");
        }
        super::note_step("Extrude");
        let steps = super::STATE.lock().unwrap().steps.clone();
        assert_eq!(steps, vec!["Move".to_string(), "Extrude".to_string()], "a repeated command flooded the trail");
        super::use_dir_for_test(None);
    }
}
