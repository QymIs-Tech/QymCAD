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
    drop(s);
    journal(&format!("operation: {name}"));
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
        let report = write_report(info);
        journal(&format!(
            "panicked: {} (report {})",
            panic_text(info.payload()),
            report.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "not written".into())
        ));
        previous(info);
    }));
}

/// THE JOURNAL OF THIS RUN, on disk, a line at a time.
///
/// Reported behaviour: every person whose graphics went wrong had no crash file to send. The hook above sees a panic
/// of Rust and nothing else: a fault inside a graphics driver - an access violation in DX12 or Vulkan - or the
/// system ending the process takes the program away before any hook runs. So the run writes down where it is as it
/// goes - the adapters offered, the one chosen, the first frame drawn, every operation opened - each line handed to
/// the system the moment it is written, which a dying process cannot take back. A clean close writes `ended`. The
/// next start finds a journal without it and turns it into a report.
static JOURNAL: Mutex<Option<PathBuf>> = Mutex::new(None);
const JOURNAL_NAME: &str = "start_journal.txt";
const ENDED: &str = "ended";

/// BEGIN THIS RUN'S JOURNAL, and answer the report written for the run before when that one stopped without closing.
/// Only the start of the program calls it: until then nothing is written, so a check never writes into a person's
/// profile.
pub fn begin_journal() -> Option<PathBuf> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(JOURNAL_NAME);
    let report = std::fs::read_to_string(&path).ok().filter(|t| !t.trim().is_empty() && t.lines().last().map(str::trim) != Some(ENDED)).and_then(|t| journal_report(&t));
    std::fs::write(&path, format!("started {}\n", crate::gui::now_iso8601())).ok()?;
    if let Ok(mut j) = JOURNAL.lock() {
        *j = Some(path);
    }
    report
}

/// A REPORT OF A RUN THAT STOPPED WITHOUT CLOSING: this machine, and the journal that run left, its last line last.
/// No backtrace - the process that could have given one is gone.
fn journal_report(journal: &str) -> Option<PathBuf> {
    let path = next_report_path()?;
    let mut out = crate::diagnostics::block();
    out.push_str(&format!("\nTime: {}\n", crate::gui::now_iso8601()));
    out.push_str("Stopped without a word: the run before this one ended without closing - the program, the graphics driver or the system stopped it.\n");
    out.push_str("\nIts journal, the last line last:\n");
    for line in journal.lines() {
        out.push_str(&format!("  {}\n", without_home(line)));
    }
    std::fs::write(&path, out).ok()?;
    Some(path)
}

/// ADD A LINE TO THIS RUN'S JOURNAL - nothing before `begin_journal`.
pub fn journal(line: &str) {
    use std::io::Write;
    let Ok(j) = JOURNAL.lock() else { return };
    let Some(path) = j.as_ref() else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
        let _ = f.write_all(format!("{line}\n").as_bytes());
    }
}

/// THE GRAPHICS DEVICE IS WATCHED: when the driver takes it away (a reset of the card, a driver that hung, the device
/// removed), the reason wgpu gives goes into the journal and into a report at once - nothing can be drawn after it,
/// and what follows it is a fault that names nothing.
pub fn watch_the_device(device: &eframe::wgpu::Device) {
    device.set_device_lost_callback(|reason, message| {
        journal(&format!("the graphics device was lost ({reason:?}): {message}"));
        if let Some(path) = next_report_path() {
            let _ = write_note(&path, "Graphics device lost", &format!("{reason:?}: {message}"), "(the graphics driver)");
        }
    });
}

/// HOW MANY TIMES EACH KIND OF FAILED FRAME HAS BEEN SEEN, so the journal holds the first of each and then every
/// hundredth - a surface timing out on every frame would otherwise fill the disk.
static SURFACE_FAILURES: Mutex<Vec<SurfaceFailure>> = Mutex::new(Vec::new());

/// One kind of frame the window could not get, and how often.
struct SurfaceFailure {
    kind: String,
    seen: u64,
}

/// What the framework does with a frame it could not get.
pub type SurfaceAnswer = dyn Fn(&eframe::wgpu::CurrentSurfaceTexture) -> eframe::egui_wgpu::SurfaceErrorAction + Send + Sync;

/// THE FRAMEWORK'S ANSWER TO A FRAME IT COULD NOT GET, kept as it is, with the failure written into the journal
/// first. A window hidden from view is no failure and is not written.
pub fn journaled_surface_status(answer: std::sync::Arc<SurfaceAnswer>) -> std::sync::Arc<SurfaceAnswer> {
    std::sync::Arc::new(move |status| {
        if !matches!(status, eframe::wgpu::CurrentSurfaceTexture::Occluded) {
            let kind = format!("{status:?}");
            let seen = SURFACE_FAILURES.lock().ok().map(|mut all| {
                let i = all.iter().position(|f| f.kind == kind).unwrap_or_else(|| {
                    all.push(SurfaceFailure { kind: kind.clone(), seen: 0 });
                    all.len() - 1
                });
                all[i].seen += 1;
                all[i].seen
            });
            if let Some(n) = seen.filter(|n| *n == 1 || n % 100 == 0) {
                journal(&format!("the window could not get a frame ({kind}), {n} time(s)"));
            }
        }
        answer(status)
    })
}

/// THE RUN CLOSED AS IT SHOULD: the journal says so, and the next start writes no report for it.
pub fn end_journal() {
    journal(ENDED);
    if let Ok(mut j) = JOURNAL.lock() {
        *j = None;
    }
}

/// Where the reports live: the data directory of the program, beside the parts library.
fn dir() -> Option<PathBuf> {
    if let Ok(s) = STATE.lock() {
        if let Some(d) = s.dir.clone() {
            return Some(d);
        }
    }
    qymcad_paths::data("crashes")
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

/// What a panic said. The payload is a `&str` for `panic!("literal")` and a `String` for a formatted one; anything else
/// has no text to show.
pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "(the panic carried no message)".to_string())
}

fn write_report(info: &std::panic::PanicHookInfo<'_>) -> Option<PathBuf> {
    let path = next_report_path()?;
    let message = panic_text(info.payload());
    let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "(unknown)".into());
    write_note(&path, "Panic", &message, &without_home(&place))
}

/// A RUN THAT NEVER GOT AS FAR AS A WINDOW still leaves a report.
///
/// Reported behaviour: on a machine with an old card and no graphics driver the program starts, blinks and
/// closes, AND THERE IS NOTHING IN THE CRASH FOLDER. There would not be: the framework hands a start-up
/// failure back as an ordinary error, not a panic, so the hook above never sees it - and the message went
/// to standard error, which a windowed build on Windows has nowhere to print to.
pub fn note_failed_start(reason: &str) -> Option<PathBuf> {
    let path = next_report_path()?;
    write_note(&path, "Could not start", reason, "(before the window opened)")
}

/// A free name for the next report: `crash_<seconds>`, and `_2`, `_3` while one already exists.
///
/// A SECOND IS NOT UNIQUE ENOUGH. The name carried only the time in seconds, so two crashes inside one
/// second wrote to the same path and the later one silently ate the earlier. That is not a theoretical
/// worry - a panic in a loop crashes many times over, and those are exactly the reports worth having.
/// Found by a guard: a second test panicked in the same second and the report under test disappeared.
fn next_report_path() -> Option<PathBuf> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let secs = crate::gui::unix_secs();
    // A NAME THAT CANNOT COLLIDE, rather than one checked for existence first.
    //
    // "Does this file exist, no, write it" is a race: two threads panicking in the same second both see
    // nothing and both write the same name, and one report is silently replaced by the other. Measured on
    // a live run: two checks failed at the same moment and the report left behind carried the wrong panic.
    // The process id separates runs, the counter separates panics within one, and neither asks the disk.
    //
    // UNDERSCORE, NOT A DASH, and the reason is a guard rather than taste: `crash-` reads as a catalogue
    // key (the window's strings live under that very prefix), and the check that no service name reaches
    // the screen would flag this file name for ever.
    static NTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Some(dir.join(format!("crash_{secs}_{}_{nth:04}.txt", std::process::id())))
}

/// The body of a report: the same for a panic and for a start that never happened, so the one window that
/// shows them and the one form that receives them read one shape.
fn write_note(path: &Path, kind: &str, message: &str, place: &str) -> Option<PathBuf> {
    let (steps, doc, autosave) = match STATE.lock() {
        Ok(s) => (s.steps.clone(), s.doc.clone(), s.autosave.clone()),
        Err(_) => (Vec::new(), None, None),
    };

    let mut out = String::new();
    out.push_str(&crate::diagnostics::block());
    out.push_str(&format!("\nTime: {}\n", crate::gui::now_iso8601()));
    out.push_str(&format!("{kind}: {message}\nAt: {place}\n"));
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

    std::fs::write(path, out).ok()?;
    // THE PANIC HOOK RUNS ON THE THREAD THAT PANICKED, so remembering the file here is remembering it for
    // that thread and no other. That is what lets a check find ITS OWN report: the directory is shared by
    // the whole binary, and a check that reads the directory reads whatever the neighbours left in it.
    #[cfg(test)]
    remember_report_here(path);
    Some(path.to_path_buf())
}

#[cfg(test)]
thread_local! {
    /// THE REPORT THIS THREAD'S LAST PANIC LEFT.
    ///
    /// Only the checks ask. In the program a crash ends the run, and what matters at the next start is the
    /// directory - which `unseen_reports` reads.
    static LAST_REPORT_HERE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn remember_report_here(path: &Path) {
    LAST_REPORT_HERE.with(|c| *c.borrow_mut() = Some(path.to_path_buf()));
}

/// WHICH FILE THIS THREAD'S OWN PANIC WROTE. `None` if it has not panicked through the hook.
#[cfg(test)]
pub(crate) fn last_report_here() -> Option<PathBuf> {
    LAST_REPORT_HERE.with(|c| c.borrow().clone())
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

/// THE REPORT DIRECTORY IS ONE PER PROCESS. Every test that redirects it takes this turn first, or they
/// interleave: one clears the directory the other is about to read, and the failure looks like a defect in
/// the reports rather than in the tests. The turn lives HERE rather than in the tests below because the
/// checks that need it live in other files too.
#[cfg(test)]
pub(crate) static TAKE_TURNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn use_dir_for_test(d: Option<&Path>) {
    if let Ok(mut s) = STATE.lock() {
        s.dir = d.map(Path::to_path_buf);
        s.steps.clear();
        s.doc = None;
        s.autosave = None;
    }
    if let Ok(mut j) = JOURNAL.lock() {
        *j = None;
    }
}

#[cfg(test)]
mod tests {
    /// A CRASH LEAVES A FILE, AND THE FILE ANSWERS THE QUESTIONS ASKED OF A CRASH.
    ///
    /// Before this there was no panic hook at all, so the answer to "what was it doing" was whatever the
    /// person remembered. The test panics for real, through the installed hook, because a hook that is
    /// never exercised is a hook that quietly stops working.
    /// TWO PANICS IN THE SAME SECOND GET TWO FILES.
    ///
    /// The name used to be picked by asking the disk "does this exist" and then writing it - a race with a
    /// window between the two calls. Nothing had to go wrong for it to bite: the check below asks for two
    /// names in a row without writing either, which is exactly what two threads do, and the old code handed
    /// back the same name twice.
    #[test]
    fn two_reports_in_one_second_do_not_share_a_name() {
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-crash-names-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::use_dir_for_test(Some(&dir));

        let a = super::next_report_path().expect("a name for the first report");
        let b = super::next_report_path().expect("a name for the second report");
        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
        assert_ne!(a, b, "two panics in the same second would write over each other");
    }

    #[test]
    fn a_crash_leaves_a_report() {
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
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

        // OURS BECAUSE THE HOOK SAID SO, not because we went looking. The hook is installed for the whole
        // process, so any other test that panics while this one runs writes a report into the same
        // directory: this check went red only when two ratchets failed elsewhere, which was a fault here
        // rather than in the hook. Searching the directory by the message was the first cure and it was not
        // enough - the directory can be swapped out from under the search. The hook runs on the panicking
        // thread, so the thread remembers its own file and nothing else can put one there.
        let report = super::last_report_here().expect("the hook wrote no report for this thread");
        let text = std::fs::read_to_string(&report).expect("the report is unreadable");

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
        super::mark_seen(&report);
        assert!(!super::unseen_reports().contains(&report), "the report is offered a second time");
        assert!(report.with_extension("seen.txt").exists(), "marking it seen deleted the file");

        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A RUN THAT STOPPED WITHOUT CLOSING IS TOLD ABOUT AT THE NEXT START, with where it was. Reported behaviour: no
    /// one whose graphics went wrong had a crash file - a fault inside the driver passes no hook.
    #[test]
    fn a_run_that_stopped_without_closing_leaves_a_report_at_the_next_start() {
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-crash-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::use_dir_for_test(Some(&dir));

        // the run that died: its journal as the program writes it, and no `ended`
        assert!(super::begin_journal().is_none(), "a first start found a run before it");
        super::journal("drawing with: wgpu Dx12, Radeon (TM) RX 480 Series (DiscreteGpu)");
        super::journal("the first frame is drawn");
        super::note_step("Extrude");
        let report = super::begin_journal().expect("the run that stopped without closing left no report");
        let text = std::fs::read_to_string(&report).expect("the report is unreadable");
        assert!(text.contains("Stopped without a word"), "the report does not say how the run ended:\n{text}");
        assert!(text.contains("Radeon (TM) RX 480") && text.contains("the first frame is drawn") && text.contains("operation: Extrude"), "the report lost the journal:\n{text}");
        assert!(super::unseen_reports().contains(&report), "the report is not offered at the start");

        // this run closes as it should: the next start says nothing
        super::journal("the first frame is drawn");
        super::end_journal();
        let after_a_clean_close = super::begin_journal();
        super::end_journal();
        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(after_a_clean_close.is_none(), "a run that closed as it should was reported as stopped: {after_a_clean_close:?}");
    }

    /// A LOST GRAPHICS DEVICE IS WRITTEN DOWN with the reason the driver gave, and a report is left at once.
    #[test]
    fn a_lost_graphics_device_is_written_down_and_reported() {
        use eframe::wgpu;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
            eprintln!("PASSED OVER: no graphics device to lose");
            return;
        };
        let Ok((device, _queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
            eprintln!("PASSED OVER: the graphics device would not open");
            return;
        };
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-crash-device-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::use_dir_for_test(Some(&dir));
        let _ = super::begin_journal();

        super::watch_the_device(&device);
        device.destroy();
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        let journal = std::fs::read_to_string(dir.join(super::JOURNAL_NAME)).unwrap_or_default();
        let reported = super::unseen_reports().iter().any(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains("Graphics device lost")));
        super::end_journal();
        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(journal.contains("the graphics device was lost"), "the journal does not say the device was lost:\n{journal}");
        assert!(reported, "no report was left for the lost device");
    }

    /// A FRAME THE WINDOW COULD NOT GET IS WRITTEN DOWN, the first of each kind, and the framework's answer is kept.
    #[test]
    fn a_frame_the_window_could_not_get_is_written_down() {
        use eframe::egui_wgpu::SurfaceErrorAction;
        use eframe::wgpu::CurrentSurfaceTexture;
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("qymcad-crash-surface-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::use_dir_for_test(Some(&dir));
        let _ = super::begin_journal();

        let answer = super::journaled_surface_status(std::sync::Arc::new(|_| SurfaceErrorAction::Reconfigure));
        let kept = matches!(answer(&CurrentSurfaceTexture::Timeout), SurfaceErrorAction::Reconfigure);
        let _ = answer(&CurrentSurfaceTexture::Occluded);
        let journal = std::fs::read_to_string(dir.join(super::JOURNAL_NAME)).unwrap_or_default();
        super::end_journal();
        super::use_dir_for_test(None);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(kept, "the framework's answer to a failed frame was changed");
        assert!(journal.contains("could not get a frame (Timeout)"), "a timed-out frame is not in the journal:\n{journal}");
        assert!(!journal.contains("Occluded"), "a hidden window was written down as a failure:\n{journal}");
    }

    /// A repeated command must not fill the whole trail with one word.
    #[test]
    fn the_trail_does_not_repeat_itself() {
        let _turn = super::TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
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
