//! THE WORK IN THE BACKGROUND: a job in hand, its result, its pulse, the trial of a command.

use super::*;

/// WHAT THE TRIAL BUILD SAYS of the command in hand (written by the part workbench, read by the drawing).
#[derive(Clone, Debug, PartialEq)]
pub enum Trial {
    /// It builds, or it is not one the geometry bounds, or it is not ready to be tried.
    Clear,
    /// Still being built on the worker: the command waits for the answer before it applies.
    Checking,
    /// It would not build: the key of the field the refusal is said beside, and the words.
    Refused(String, String),
}

impl Trial {
    /// The refusal, if there is one.
    pub fn refusal(&self) -> Option<(String, String)> {
        match self {
            Trial::Refused(k, w) => Some((k.clone(), w.clone())),
            _ => None,
        }
    }
}

/// The triangles of the faces a trial build adds, in the frame of the context: what the preview shows.
pub type TrialFaces = std::sync::Arc<Vec<[[f64; 3]; 3]>>;

/// What a trial came to: the verdict, and the faces the build adds.
#[derive(Clone)]
pub struct TrialAnswer {
    pub verdict: Trial,
    pub faces: TrialFaces,
}

/// A trial in flight or done, for the command's state key: the worker writes its answer when it has one.
#[derive(Clone)]
pub struct TrialSlot {
    pub key: u64,
    pub answer: std::sync::Arc<std::sync::Mutex<Option<TrialAnswer>>>,
}

/// Where the trial of the command in hand is kept in the frame's memory.
pub fn trial_slot_id() -> egui::Id {
    egui::Id::new("feat_cmd_trial")
}

/// Where the frame the trial last answered for the command in hand is kept.
pub(crate) fn trial_seen_id() -> egui::Id {
    egui::Id::new("feat_cmd_trial_seen")
}

/// THE TRIAL IN THE SLOT IS THE ONE OF THE COMMAND IN HAND, as of this frame: said by whoever asked for it with the
/// state of the command.
pub fn trial_is_current(ctx: &egui::Context) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(trial_seen_id(), frame));
}

/// THE FACES THE COMMAND IN HAND WILL ADD, as its trial built them - once the trial has answered that it builds.
///
/// Only a trial asked for in this frame or the one before is drawn: the slot lives in the memory of the window, not of
/// the document, and one left by a command no longer asked about is of another command, or of another document.
/// Reported behaviour: a project closed, a new one made, Extrude taken on a sketch - and the walls a trial of a hole
/// had built in the closed project stood green over the new sketch.
pub fn trial_faces(ctx: &egui::Context) -> Option<TrialFaces> {
    let seen = ctx.data(|d| d.get_temp::<u64>(trial_seen_id()))?;
    if seen + 1 < ctx.cumulative_frame_nr() {
        return None;
    }
    let slot = ctx.data(|d| d.get_temp::<TrialSlot>(trial_slot_id()))?;
    let got = slot.answer.lock().ok()?.clone()?;
    (got.verdict == Trial::Clear && !got.faces.is_empty()).then_some(got.faces)
}

pub struct Busy {
    /// When the job began: a long import shows how long it has gone, so that a wait does not look like a hang.
    pub started: std::time::Instant,
    pub label: String,
    /// The rebuild is quiet: the work is small, no window is shown - only the status line.
    pub quiet: bool,
    pub rx: std::sync::mpsc::Receiver<JobResult>,
    /// Only a rebuild has this: there is nothing to interrupt the other tasks with (a file write stopped halfway
    /// is a broken file, and parsing a STEP is all or nothing).
    pub pulse: Option<std::sync::Arc<RegenPulse>>,
    /// The kind of background task - it shows whether another one of the same kind may be queued (two writes of
    /// one file at once are a race between the temporary file and the rename) and what exactly to wait for on exit.
    pub kind: BgKind,
}

/// HOW LONG A REBUILD RUNS UNSEEN: one that lands sooner paints no sign at all. Reported behaviour: the sign went up on
/// the first frame in flight, and a rebuild of tens of milliseconds was a veil on screen for one or two frames - the
/// window blinked on every edit.
///
/// Measured in a dev build, rebuild in the worker from start to result: an extrusion of a small part 7-9 ms; a pinpoint
/// edit of a 12-node document 15-17 ms, its whole timeline 190-200 ms; a pinpoint edit of an 8-node document with
/// booleans 110-125 ms, its whole timeline 0.86-1.05 s; a pinpoint edit of a 6-node document with fillets 0.65-0.87 s.
/// 300 ms leaves every edit under 0.2 s unmarked and marks every one past 0.6 s.
pub const REBUILD_SIGN_AFTER: std::time::Duration = std::time::Duration::from_millis(300);

/// HOW LONG A SIGN, ONCE PAINTED, STAYS: a rebuild landing just past `REBUILD_SIGN_AFTER` would otherwise blink as well.
pub const REBUILD_SIGN_HOLDS: std::time::Duration = std::time::Duration::from_millis(300);

/// WHAT THE SIGN OF A REBUILD LOOKS LIKE.
#[derive(Clone)]
pub enum RebuildLook {
    /// A pinpoint rebuild: a faint veil over the canvas and a spinner.
    Quiet,
    /// A long one: the panels dimmed and a card with its words and its count of nodes.
    Loud { label: String, pulse: Option<std::sync::Arc<RegenPulse>> },
}

/// A REBUILD IN FLIGHT, as its sign sees it: when it started and how it looks.
pub struct RebuildRunning {
    pub started: std::time::Instant,
    pub look: RebuildLook,
}

impl RebuildRunning {
    /// The rebuild in hand, when the job in hand is one.
    pub fn of(busy: Option<&Busy>) -> Option<Self> {
        let b = busy.filter(|b| b.kind == BgKind::Regen)?;
        let look = if b.quiet { RebuildLook::Quiet } else { RebuildLook::Loud { label: b.label.clone(), pulse: b.pulse.clone() } };
        Some(Self { started: b.started, look })
    }
}

/// THE SIGN ON SCREEN: since when it is painted and what it shows.
#[derive(Clone)]
pub struct RebuildSign {
    pub since: std::time::Instant,
    pub look: RebuildLook,
}

/// THE SIGN OF A REBUILD AT `now`, given the rebuild in flight and the sign the frame before painted.
///
/// A rebuild in flight is painted once it has run `REBUILD_SIGN_AFTER`, and from then on for as long as it, or the next
/// one, runs. With nothing in flight a sign painted stays until it has been up `REBUILD_SIGN_HOLDS`. Only the paint
/// waits: the barrier of a loud rebuild is not decided here.
pub fn rebuild_sign(running: Option<&RebuildRunning>, shown: Option<RebuildSign>, now: std::time::Instant) -> Option<RebuildSign> {
    let Some(r) = running else {
        return shown.filter(|s| now.saturating_duration_since(s.since) < REBUILD_SIGN_HOLDS);
    };
    let since = match shown {
        Some(s) => s.since,
        None if now.saturating_duration_since(r.started) >= REBUILD_SIGN_AFTER => now,
        None => return None,
    };
    Some(RebuildSign { since, look: r.look.clone() })
}

/// The sign of this frame, the one painted before kept in the memory of the window: the sign outlives its rebuild by
/// up to `REBUILD_SIGN_HOLDS`, and the document has nothing to keep it in.
pub fn rebuild_sign_now(ctx: &egui::Context, running: Option<&RebuildRunning>) -> Option<RebuildSign> {
    let id = egui::Id::new("rebuild_sign");
    let next = rebuild_sign(running, ctx.data(|d| d.get_temp::<RebuildSign>(id)), std::time::Instant::now());
    ctx.data_mut(|d| {
        if let Some(s) = &next {
            d.insert_temp(id, s.clone());
        } else {
            d.remove::<RebuildSign>(id);
        }
    });
    next
}

/// HOW LONG A REBUILD LEFT RUNNING IS WAITED FOR when its job is dropped: as long as the end of the program waits for a
/// rebuild (`wait_bg`).
pub(crate) const DROPPED_REBUILD_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// A REBUILD LEFT RUNNING IS STOPPED AND WAITED FOR when its job is dropped - at the end of the program, or of a check -
/// so the process never ends with a thread inside OCCT: the static destructors of OCCT, some made lazily during the work
/// and so destroyed before any handler of the end runs, then pulled its pool and its allocator from under the rebuild.
/// Its result comes back and is dropped here, the shapes in it freed on this thread. A job whose result was taken has a
/// worker gone or going: the channel is closed and nothing is waited for. A rebuild is started only when no job is in
/// hand, so no job is dropped while the window works. Reported behaviour: of 40 programs ended during a rebuild, 5 hung
/// and 1 ended with SIGABRT.
impl Drop for Busy {
    fn drop(&mut self) {
        let Some(pulse) = &self.pulse else { return };
        pulse.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.rx.recv_timeout(DROPPED_REBUILD_WAIT);
    }
}

/// The result of a background (worker thread) import or export, arriving at the interface over a channel.
pub enum JobResult {
    /// An exact file (STEP, IGES) was imported: the bodies (mesh plus B-rep faces) + the solids' live shapes +
    /// the file's path and format.
    ExactImported { path: String, format: qymcad_kernel::ExactFormat, bodies: Vec<qymcad_core::geom::Built>, shapes: Vec<qymcad_kernel::Shape>, nodes: Vec<qymcad_kernel::ImportNode> },
    /// An IGES with no surfaces, read as a drawing: the curves go into a sketch the way DXF does, and `note` is
    /// what the status adds (a library's cells are shown, entities were not drawn).
    DrawingRead { path: String, curves: Vec<qymcad_core::geom::ProfEdge>, note: String },
    /// Meshes were imported (STL, OBJ): each piece is a body with its detected faces (the detection runs in the
    /// worker too - it is heavy on large meshes).
    MeshImported { path: String, format: MeshFormat, pieces: Vec<MeshPiece> },
    /// An export finished: the status plus the shapes moved into the worker (to be returned to the self.live.shapes cache).
    Exported { status: crate::Notice, shapes_back: Vec<(Id, qymcad_kernel::Shape)> },
    /// A project was loaded (in a thread): the parsed timeline plus the faces. The bodies' geometry comes from the
    /// bundle rather than being rebuilt, so the window shows the model at once.
    ProjectLoaded { path: String, project: Box<Project>, shapes: Vec<(Id, qymcad_kernel::Shape)> },
    /// A REBUILD THAT PANICKED in the worker thread. Its copy of the document is dropped half-done, as a cancelled
    /// one is; the live B-rep it took with it comes home. Reported behaviour (issue #119): the thread died with the
    /// whole cache in it, and every body of the document was left without its live B-rep.
    RegenFailed {
        /// The document's fingerprint at the start, as for `Regenerated`.
        stamp: u64,
        /// The live B-rep the rebuild took with it, back for the cache.
        shapes: Vec<(Id, qymcad_kernel::Shape)>,
        /// What the panic said, for the status line.
        why: String,
    },
    /// A REBUILD of the timeline carried out in a worker thread. Heavy operations (a thread takes seconds on a
    /// boolean) no longer hold the interface thread: the window draws a spinner, and the system does not consider
    /// the program hung nor offer to kill it.
    Regenerated {
        /// THE DOCUMENT'S FINGERPRINT AT THE MOMENT OF THE START. The result was computed from A COPY and replaces
        /// the document ENTIRELY: if the live document moved on in the meantime, applying such a result means
        /// silently erasing an edit. The fingerprint makes that noticeable rather than relying on there being
        /// nowhere for an edit to come from.
        stamp: u64,
        project: Box<Project>,
        shapes: Vec<(Id, qymcad_kernel::Shape)>,
        built: Vec<(Id, Vec<MeshFace>)>,
        errors: Vec<(Id, qymcad_core::errors::CoreError)>,
        /// THE REBUILD WAS STOPPED BY A PERSON. The result is incomplete by construction and must not be applied -
        /// it is thrown away whole and the document stays what it was.
        cancelled: bool,
    },
    /// The imported solids' B-rep, restored from the embedded STEP. `regen` says whether a full rebuild is needed
    /// afterwards (an older file with no saved geometry); otherwise this is a BACKGROUND top-up (the model is
    /// already on the screen and all that is awaited is that operations on the imports become available).
    ImportShapes { shapes: Vec<(Id, qymcad_kernel::Shape)>, regen: bool },
    /// The background write of the project finished (`error` is None on success).
    Saved { path: String, autosave: bool, error: Option<String> },
    /// An error - its text goes to the status line.
    Failed(String),
}

/// THE CONTROL PANEL OF A LONG REBUILD: how much is done, and the request to stop.
///
/// One record for both sides: the thread writes the progress and reads the request, the window reads the progress
/// and writes the request. Two different mechanisms ("progress over a channel, cancellation by a flag") would give
/// two different moments of truth about one piece of work.
#[derive(Default)]
pub struct RegenPulse {
    /// The node being computed right now, and the total number of nodes.
    pub done: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
    /// Shared with the kernel doing the work, which looks at it inside a long node too.
    pub stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The fingerprint (`Project::rebuild_key`) of the document the rebuild computes: once the live one differs, the
    /// result is stale before it arrives.
    pub stamp: u64,
}

/// HOW FAR A REBUILD HAS GOT, in nodes of the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeCount {
    pub done: usize,
    pub total: usize,
}

impl RegenPulse {
    pub fn progress(&self) -> NodeCount {
        use std::sync::atomic::Ordering::Relaxed;
        NodeCount { done: self.done.load(Relaxed), total: self.total.load(Relaxed) }
    }
    pub fn ask_stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    pub(crate) fn stop_asked(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl qymcad_core::feature::RegenWatch for RegenPulse {
    fn step(&self, done: usize, total: usize) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.done.store(done, Relaxed);
        self.total.store(total, Relaxed);
        !self.stop_asked()
    }
    fn stopped(&self) -> bool {
        self.stop_asked()
    }
}

/// The kinds of background work. There used to be a SINGLE slot, and Save during the restoration of the imports'
/// B-rep overwrote its receiver: the thread finished computing and sent the shapes into a closed channel, so the
/// B-rep did not appear until a restart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BgKind {
    /// Writing the project to disk (by hand or by autosave).
    Save,
    /// Restoring the imported solids' B-rep from the embedded STEP.
    ImportShapes,
    /// A rebuild of the timeline by the kernel - modal (edits are forbidden while it runs).
    Regen,
}
