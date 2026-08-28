//! ASKING FOR A FILE WITHOUT THE PROGRAM FALLING SILENT.
//!
//! A file chooser is the system's own window with a loop of its own, and it does not return until an answer
//! is given. Called straight out of a frame, `rfd::FileDialog::pick_file()` holds the frame thread inside
//! that loop: our window draws nothing for as long as the chooser is open, and a desktop that has had no
//! answer from a window for a few seconds paints "the application is not responding" over it.
//!
//! Reported behaviour: opening a project brings up the chooser and then a system notice that the program
//! has stopped responding.
//!
//! So the chooser is STARTED on the frame thread - macOS begins the panel inside the constructor and takes
//! the main thread for it - and AWAITED on a worker, which sends the answer down a channel. Frames keep
//! being drawn all the while; what was to be done with the file is held here as a continuation and runs on
//! whichever later frame the answer lands in.
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use super::App;

type PathFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Option<rfd::FileHandle>> + Send>>;

/// A file chooser in flight, and what its answer is for.
pub(crate) struct FileAsk {
    rx: Receiver<Option<PathBuf>>,
    /// Runs on the frame thread once the answer arrives, so it is free to touch the document.
    then: Box<dyn FnOnce(&mut App, PathBuf)>,
}

impl App {
    /// Ask for an existing file. `then` runs on a later frame with the chosen path; cancelling drops it
    /// unused. If a chooser is already open this does nothing - see [`App::asking_for_a_file`].
    pub(crate) fn ask_open_file(&mut self, dialog: rfd::AsyncFileDialog, then: impl FnOnce(&mut App, PathBuf) + 'static) {
        if self.asking_for_a_file() {
            return;
        }
        self.spawn_file_ask(Box::pin(dialog.pick_file()), then);
    }

    /// Ask where to write. The same contract as [`App::ask_open_file`].
    pub(crate) fn ask_save_file(&mut self, dialog: rfd::AsyncFileDialog, then: impl FnOnce(&mut App, PathBuf) + 'static) {
        if self.asking_for_a_file() {
            return;
        }
        self.spawn_file_ask(Box::pin(dialog.save_file()), then);
    }

    /// The future is BUILT here, on the frame thread, and only HELD by the worker. That split is not a
    /// preference: on macOS the panel is put up inside the constructor and the main thread is what it is
    /// put up from, while the waiting itself may happen anywhere.
    fn spawn_file_ask(&mut self, fut: PathFuture, then: impl FnOnce(&mut App, PathBuf) + 'static) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(pollster::block_on(fut).map(|h| h.path().to_path_buf()));
        });
        self.arm_file_ask(rx, then);
    }

    /// Hold `rx` and the continuation until [`App::poll_file_ask`] picks the answer up.
    pub(crate) fn arm_file_ask(&mut self, rx: Receiver<Option<PathBuf>>, then: impl FnOnce(&mut App, PathBuf) + 'static) {
        self.file_ask = Some(FileAsk { rx, then: Box::new(then) });
    }

    /// Is a chooser open? Asked before putting one up, because the frames now keep running while one is
    /// answered: a menu is still clickable behind the system window, and a second chooser would take the
    /// slot from the first - whose own answer would then arrive with nothing left to do it with.
    pub(crate) fn asking_for_a_file(&self) -> bool {
        self.file_ask.is_some()
    }

    /// THE PROGRAM GOES INERT WHILE THE SYSTEM IS ASKING FOR A FILE.
    ///
    /// The chooser no longer holds the frame thread, which is the point - but that also means the window
    /// behind it goes on drawing and would otherwise go on ACCEPTING. A menu is clickable through it, a
    /// hotkey fires, and "new project" can be started underneath an open "open project", which lands the
    /// answer to the chooser in a document that is no longer the one it was opened from.
    ///
    /// A system chooser is a modal window and owns the interaction until it is answered, so for as long as
    /// it is up everything here is greyed and deaf: the widgets of the frame are disabled, the panes drawn
    /// straight onto the context are covered by a barrier that eats clicks, and whatever was taking typing
    /// has its focus taken away.
    pub(crate) fn inert_while_choosing(&self, ui: &mut egui::Ui) {
        ui.disable();
        ui.ctx().memory_mut(|m| m.stop_text_input());
        let screen = ui.ctx().viewport_rect();
        // ABOVE `Foreground`, where the menus and the barriers of a rebuild live: a barrier level with what
        // it is meant to cover decides nothing.
        egui::Area::new(egui::Id::new("file_chooser_barrier"))
            .order(egui::Order::Tooltip)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
            });
    }

    /// Once a frame: run the continuation if the answer has landed. Returns whether a chooser is still open,
    /// which is what keeps the frames coming - egui sleeps between events, and the answer arrives from a
    /// thread, which is not an event it knows about.
    pub(crate) fn poll_file_ask(&mut self) -> bool {
        let Some(ask) = &self.file_ask else { return false };
        match ask.rx.try_recv() {
            Err(TryRecvError::Empty) => return true,
            // the answer, or a worker that died without one (which is a cancellation as far as anyone here
            // can tell): either way the slot is freed, so a chooser is never left blocking the next one
            Ok(answer) => {
                let ask = self.file_ask.take().expect("the chooser was there a line ago");
                if let Some(path) = answer {
                    (ask.then)(self, path);
                }
            }
            Err(TryRecvError::Disconnected) => self.file_ask = None,
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// THE ANSWER REACHES THE DOCUMENT. The continuation is what carries the chosen file into the program,
    /// and it must run on a frame rather than at the moment of the answer - it touches the document, which
    /// belongs to the frame thread.
    #[test]
    fn the_chosen_file_reaches_the_continuation() {
        let mut app = crate::gui::App::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(None::<PathBuf>));
        let box_ = seen.clone();
        app.arm_file_ask(rx, move |_app, p| *box_.borrow_mut() = Some(p));

        assert!(app.asking_for_a_file(), "the chooser is open until an answer comes");
        assert!(app.poll_file_ask(), "no answer yet - the frames must keep coming");
        assert!(seen.borrow().is_none(), "nothing was chosen, so nothing was done");

        tx.send(Some(PathBuf::from("/tmp/part.qcad"))).expect("the slot is listening");
        assert!(!app.poll_file_ask(), "the answer landed - the chooser is done");
        assert_eq!(seen.borrow().as_deref(), Some(std::path::Path::new("/tmp/part.qcad")));
        assert!(!app.asking_for_a_file(), "the slot is free for the next question");
    }

    /// CANCELLING LEAVES NO TRACE. Answering "cancel" must free the slot exactly as choosing does, or the
    /// second attempt to open a file is refused by the first one that was walked away from.
    #[test]
    fn cancelling_frees_the_slot_and_does_nothing() {
        let mut app = crate::gui::App::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let ran = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = ran.clone();
        app.arm_file_ask(rx, move |_app, _p| flag.set(true));

        tx.send(None).expect("the slot is listening");
        assert!(!app.poll_file_ask());
        assert!(!ran.get(), "cancelling does not act");
        assert!(!app.asking_for_a_file(), "and does not leave the slot occupied");
    }

    /// A WORKER THAT DIES IS A CANCELLATION. A chooser the system refused to put up drops its end of the
    /// channel; the slot has to clear on that too, or every later attempt is silently ignored.
    #[test]
    fn a_dead_worker_frees_the_slot() {
        let mut app = crate::gui::App::default();
        let (tx, rx) = std::sync::mpsc::channel::<Option<PathBuf>>();
        app.arm_file_ask(rx, |_app, _p| unreachable!("there is no answer to act on"));
        drop(tx);
        assert!(!app.poll_file_ask());
        assert!(!app.asking_for_a_file());
    }

    /// A SECOND CHOOSER DOES NOT TAKE THE FIRST ONE'S PLACE. The frames keep running while a chooser is
    /// answered, so the menu behind it is still clickable. Were the slot simply overwritten, the first
    /// chooser would stay on screen with nothing waiting for its answer.
    #[test]
    fn one_chooser_at_a_time() {
        let mut app = crate::gui::App::default();
        let (first_tx, first_rx) = std::sync::mpsc::channel();
        let ran = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let (a, b) = (ran.clone(), ran.clone());
        app.arm_file_ask(first_rx, move |_app, _p| a.set(1));

        // the second request finds the slot taken and is dropped, first answer intact
        app.ask_open_file(rfd::AsyncFileDialog::new(), move |_app, _p| b.set(2));

        first_tx.send(Some(PathBuf::from("/tmp/first.qcad"))).expect("the first chooser is still the one listening");
        assert!(!app.poll_file_ask());
        assert_eq!(ran.get(), 1, "the answer went to the chooser that was open");
    }

    /// NOT ONE SYNCHRONOUS CHOOSER LEFT. `rfd::FileDialog` (as against `AsyncFileDialog`) does not return
    /// until the person answers, and it is the frame thread it keeps: that is the whole of the "not
    /// responding" defect. Fifteen call sites carried it; this counts them and holds the count at zero.
    #[test]
    fn no_blocking_file_dialog_in_the_interface() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut guilty: Vec<String> = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources are readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().and_then(|s| s.to_str()) != Some("rs") {
                    continue;
                }
                // TWO FILES ARE ALLOWED TO NAME IT. This one names the blocking call in order to forbid
                // it, and must not report itself. `start_notice.rs` is the one place where blocking is
                // right: the window never opened, so there is no frame loop to hold up, and the system's
                // own message box is all that is left to speak with.
                if matches!(p.file_name().and_then(|s| s.to_str()), Some("file_ask.rs" | "start_notice.rs")) {
                    continue;
                }
                let text = std::fs::read_to_string(&p).expect("the file is readable");
                for (n, line) in text.lines().enumerate() {
                    // the async chooser shares the prefix, so the name is matched whole
                    if line.contains("rfd::FileDialog::") || line.contains("rfd::MessageDialog::") {
                        let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
                        guilty.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                    }
                }
            }
        }
        assert!(guilty.is_empty(), "a chooser that stops the frame thread is back:\n{}", guilty.join("\n"));
    }
}

/// A CHOOSER ANSWERED WHILE THE MODEL IS REBUILDING.
///
/// The frames kept running is the whole point of this change, and it has a consequence: the program is
/// usable behind the system window. Somebody can put up an export chooser, click back into the model,
/// change a dimension, and answer the chooser while the rebuild that edit started is still running. The
/// export and the rebuild both want the one modal slot, and the loser used to be dropped without a word.
#[cfg(test)]
mod exporting_over_a_rebuild {
    use crate::gui::App;

    /// A background job in the modal slot, standing in for a rebuild in flight. The sender is handed back
    /// so the test holds it: a dropped one would read as a job that has already finished.
    fn rebuilding(app: &mut App) -> std::sync::mpsc::Sender<crate::gui::JobResult> {
        let (tx, rx) = std::sync::mpsc::channel();
        app.regen.busy = Some(crate::gui::Busy {
            label: "rebuild".into(),
            rx,
            kind: crate::gui::BgKind::Regen,
            pulse: None,
            quiet: false,
        });
        tx
    }

    #[test]
    fn an_export_does_not_shoulder_a_rebuild_out_of_the_way() {
        let dir = std::env::temp_dir().join("qym_export_over_rebuild");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("late.step");
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let _job = rebuilding(&mut app);
        app.write_step_to(&path, &[], "");

        assert!(
            matches!(&app.regen.busy, Some(b) if b.kind == crate::gui::BgKind::Regen),
            "the rebuild lost the modal slot - its result would then never land"
        );
        assert_eq!(app.status, crate::i18n::tr("io-export-busy"), "the refusal has to be said out loud, not swallowed");
        assert!(!path.exists(), "nothing was written either");
    }

    /// The same for STL, which claims the same slot by the same door.
    #[test]
    fn the_stl_export_waits_its_turn_too() {
        let mut app = App::default();
        let _job = rebuilding(&mut app);
        app.write_stl_to(std::path::Path::new("/tmp/qym-never-written.stl"), &[], "", 0.1);
        assert!(matches!(&app.regen.busy, Some(b) if b.kind == crate::gui::BgKind::Regen));
        assert_eq!(app.status, crate::i18n::tr("io-export-busy"));
    }
}
