//! WHAT THE PROGRAM ASKS OF THE SYSTEM AROUND IT: a file chosen in the system's chooser, a folder shown in the
//! file manager, a page opened in the browser.
//!
//! In a live window the system answers. A session driven from outside stands in for the system on its own
//! thread: no chooser is put up - the question waits for the session's answer - and nothing is started, only
//! written down. A check that put up a real chooser hangs on a machine without a desktop and pops a window up
//! on a developer's; one that started a file manager did exactly that.
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

/// Which of the system's choosers the program put up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chooser {
    /// Choose an existing file.
    Open,
    /// Choose where to write.
    Save,
}

/// The system as a session stands in for it.
#[derive(Default)]
pub(crate) struct StandIn {
    /// A chooser put up and not answered yet.
    pub(crate) asked: Option<Asked>,
    /// What the program asked the system to start - command and arguments - in order.
    pub(crate) started: Vec<(String, Vec<String>)>,
}

/// A chooser as the stand-in holds it until the session answers.
pub(crate) struct Asked {
    pub(crate) kind: Chooser,
    /// The folder the chooser opens in; `None` leaves it to the system, which on Linux is the home folder.
    pub(crate) folder: Option<PathBuf>,
    /// Where the answer goes.
    pub(crate) tx: Sender<Option<PathBuf>>,
}

/// A session of the checks has run in this process: a real chooser here is a fault of the checks, never a question for
/// whoever sits at the desktop.
static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static STAND_IN: RefCell<Option<StandIn>> = const { RefCell::new(None) };
    /// How many sessions of this thread stand in for the system now.
    static HOLDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A SESSION OF THIS THREAD STANDS IN FOR THE SYSTEM (`true`), or has ended (`false`). The stand-in lives while any
/// session of the thread does: a check that opens a second session inside the first - a copy saved and opened again
/// - must not, by closing it, hand the first one the real system. Measured: it did, and the next chooser of the first
///   session came up as a real dialog on the desktop of whoever was running the checks.
pub(crate) fn stand_in(on: bool) {
    if on {
        CHECKED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let n = HOLDERS.get();
    let n = if on { n + 1 } else { n.saturating_sub(1) };
    HOLDERS.set(n);
    STAND_IN.with_borrow_mut(|s| {
        if n == 0 {
            *s = None;
        } else if s.is_none() {
            *s = Some(StandIn::default());
        }
    });
}

/// Work with the stand-in of this thread, if it has one.
pub(crate) fn with_stand_in<R>(f: impl FnOnce(&mut StandIn) -> R) -> Option<R> {
    STAND_IN.with_borrow_mut(|s| s.as_mut().map(f))
}

/// What a chooser of the system answers with.
pub(crate) type PathFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Option<rfd::FileHandle>> + Send>>;

/// PUT UP A CHOOSER and hand back where its answer will arrive.
///
/// The future is BUILT here, on the frame thread, and only HELD by the worker. That split is not a preference:
/// on macOS the panel is put up inside the constructor and the main thread is what it is put up from, while
/// the waiting itself may happen anywhere. A stand-in system never builds it.
pub(crate) fn choose_file(kind: Chooser, folder: Option<PathBuf>, chooser: impl FnOnce() -> PathFuture) -> Receiver<Option<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = match with_stand_in(|s| s.asked = Some(Asked { kind, folder, tx: tx.clone() })) {
        Some(()) => return rx,
        None => tx,
    };
    assert!(!CHECKED.load(std::sync::atomic::Ordering::Relaxed), "a chooser of the real system came up in a process of the checks: a session left its thread without the stand-in");
    let fut = chooser();
    std::thread::spawn(move || {
        let _ = tx.send(pollster::block_on(fut).map(|h| h.path().to_path_buf()));
    });
    rx
}

/// START A PROGRAM OF THE SYSTEM - the file manager, the browser - and leave it running.
pub(crate) fn start(cmd: &str, args: &[String]) -> std::io::Result<()> {
    if with_stand_in(|s| s.started.push((cmd.to_string(), args.to_vec()))).is_some() {
        return Ok(());
    }
    std::process::Command::new(cmd).args(args).spawn().map(|_| ())
}

#[cfg(test)]
mod stand_in_tests {
    /// A SECOND SESSION CLOSED INSIDE THE FIRST LEAVES THE FIRST ITS STAND-IN: a check that opens a copy of the document
    /// in another start must not hand the first session the real system. Reported behaviour: system dialogs to save a
    /// project came up on the desktop while the checks ran.
    #[test]
    fn a_nested_session_closed_leaves_the_stand_in() {
        super::stand_in(true);
        super::stand_in(true);
        super::stand_in(false);
        assert!(super::with_stand_in(|_| ()).is_some(), "the second session closed and took the stand-in of the first away");
        super::stand_in(false);
        assert!(super::with_stand_in(|_| ()).is_none(), "the last session closed and the stand-in stayed");
    }
}
