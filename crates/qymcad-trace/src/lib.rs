//! WHAT A CLICK WENT THROUGH - a trace of the mouse, written to a file beside the program.
//!
//! A program that closes itself without saying where leaves one question and no way to answer it: which of
//! the hundred steps a click travels through was the last one. The crash report answers it in words
//! ("- Extrude"), and that is a list of operations, not of calls: it cannot say that the click on a face
//! reached the edge cache, asked the kernel for the edges of that face, and stopped there.
//!
//! WHAT IS RECORDED IS THE PATH, and only its entries. Every traced function writes one line as it is
//! entered, indented by how deep it is, so the file reads as the tree the click walked:
//!
//! ```text
//! 1789102447.881 click at 640.0 405.0
//! 1789102447.881   viewport_3d_click_at
//! 1789102447.882     refresh_edges body=3 edges=12
//! 1789102447.882     pick_face_edges_fillet
//! 1789102447.883     apply_feat_cmd chamfer
//! ```
//!
//! THE LAST LINE IN THE FILE IS WHERE THE PROGRAM DIED. Nothing is written on the way out, and that is the
//! whole design: a line for leaving would double the file to say nothing, while a line for entering says
//! exactly the one thing a fault report cannot - the deepest call that never returned.
//!
//! ## Why not a logging crate
//!
//! A tracing framework would say more - spans, timings, filtering per subsystem - and would cost a
//! dependency, an initialiser and a subscriber to wire up. This is one file, one folder and one macro, and
//! it writes only on the mouse path: a frame draws and rebuilds sixty times a second, a click happens a few
//! times a second at most, and that difference is what lets the trace be left switched on for ever.
//!
//! ## It never takes the program down
//!
//! Every write is fallible and every failure is swallowed, for the reason the crash hook does it too: a
//! trace that panicked about its own file would be the last thing standing between a fault and the fault
//! report. A place that cannot be written to is simply not traced, and the program carries on.

use std::io::Write;
use std::path::PathBuf;

/// How big the file may grow before it is started afresh, in bytes. A trace of the mouse runs to a few
/// lines per click, so this is months of ordinary use - but a machine left tracing over a holiday would
/// otherwise fill a disk.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// The name of the file inside the settings folder.
const FILE_NAME: &str = "clicks.log";

thread_local! {
    /// WHETHER ANYTHING IS WRITTEN ON THIS THREAD, set by the program from the person's choice.
    ///
    /// It is per thread, like the open file and the depth beside it: a click is handled and traced on one
    /// thread, and a flag shared by all of them would be one check in a suite of many switching the writing
    /// off under another's feet. It starts ON, because the checks below write and must not have to switch
    /// anything on to be measured; the program sets it from the setting on the thread that draws.
    static RECORDS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Record every click on this thread, or keep silent. This is what the setting in the developer's part of
/// the window reaches the trace through.
///
/// Turning it off closes the open file as well as refusing to open another: a tick put back on then
/// appends to the same file rather than to a second one beside it.
pub fn set_recording(on: bool) {
    RECORDS.with(|r| r.set(on));
    if !on {
        OUT.with(|o| *o.borrow_mut() = None);
    }
}

thread_local! {
    /// The open file, PER THREAD. The depth is per thread as well, and the two belong together: one
    /// handle for the whole program would have two threads writing into it at once, and their lines would
    /// arrive in an order nobody chose. A click is handled on one thread, so each thread's trace is whole.
    static OUT: std::cell::RefCell<Option<std::fs::File>> = const { std::cell::RefCell::new(None) };

    /// The folder a check named with [`write_into`], ahead of the program's own.
    static FORCED: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Where the file goes when a check says so, instead of beside the program.
///
/// A check runs in the folder the build put it in, and a trace that followed it there would leave a log
/// beside the binaries of every run for ever. The handle is dropped as well as forgotten, so the file that
/// is really the one written is the one the check reads back.
pub fn write_into(dir: Option<&std::path::Path>) {
    OUT.with(|o| *o.borrow_mut() = None);
    FORCED.with(|f| *f.borrow_mut() = dir.map(std::path::Path::to_path_buf));
}

/// The folder the trace goes to: the one a check named, or the folder of the person's own settings.
///
/// The settings folder and not the program's own: a packaged program unpacks itself into a temporary
/// mount that vanishes when the window closes, so a file written beside it would be gone by the time
/// anybody went looking. The settings folder is where the program already keeps what it is told, and it
/// is the one folder on the machine that is certainly there afterwards.
fn dir() -> Option<PathBuf> {
    if let Some(d) = FORCED.with(|f| f.borrow().clone()) {
        return Some(d);
    }
    qymcad_paths::data_root()
}

thread_local! {
    /// HOW DEEP THE CALLS ARE NESTED on this thread. One per thread, because two threads drawing at once
    /// would otherwise interleave their depths into nonsense - and a click is handled on one thread.
    static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A traced function, kept alive for as long as it runs. It closes over nothing, so dropping it costs
/// nothing; dropping it is also the only way the depth can go back down, which is what keeps the
/// indentation honest even when a function returns early or is left by a `?`.
pub struct Entered(());

impl Drop for Entered {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// WRITE ONE LINE: a function as it is entered, indented by how deep it is, and one step deeper after it.
pub fn enter(name: &str) -> Entered {
    write(name);
    DEPTH.with(|d| d.set(d.get() + 1));
    Entered(())
}

/// WRITE ONE LINE at the depth as it stands, changing nothing: a value, a branch taken, a refusal in words.
pub fn note(line: &str) {
    write(line);
}

fn write(text: &str) {
    if !RECORDS.with(|r| r.get()) {
        return;
    }
    let Some(mut f) = file() else { return };
    let depth = DEPTH.with(|d| d.get());
    // A BOUND ON THE INDENT, so that a runaway recursion writes lines rather than megabytes of spaces.
    let pad = "  ".repeat(depth.min(24));
    // ONE WRITE, NOT ONE PIECE AT A TIME. Formatting a line writes it out in as many calls as it has
    // fields, and two threads doing that into the same file cut each other's lines in half - measured on a
    // live run, where two clocks came out welded into one: `17909909780781790990978078`. A whole line built
    // first and handed over in one call cannot be cut, and on a system where an append is atomic that is the
    // whole of the guarantee.
    let line = format!("{:>13} {pad}{text}\n", millis());
    let _ = f.write_all(line.as_bytes());
}

/// The open file, opened on first use. `None` where it cannot be opened, and the trace is then silent
/// rather than loud about having failed.
fn file() -> Option<std::fs::File> {
    let open = OUT.with(|o| o.borrow().as_ref().and_then(|f| f.try_clone().ok()));
    if open.is_some() {
        return open;
    }
    let dir = dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(FILE_NAME);
    // THE FILE IS STARTED OVER RATHER THAN NAMED BY THE HOUR. A name that carries the time makes a
    // directory of logs nobody ever opens; one file that is emptied is the one a person looks at.
    if std::fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let f = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()?;
    let keep = f.try_clone().ok();
    OUT.with(|o| *o.borrow_mut() = keep);
    Some(f)
}

/// Milliseconds since the epoch, for the time in front of every line. Written out rather than taken from a
/// date library: the program does not depend on one, and the trace must not add a dependency to the very
/// thing it is there to watch.
fn millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Where the trace goes, for a check that has to read it back.
pub fn path() -> Option<PathBuf> {
    Some(dir()?.join(FILE_NAME))
}

/// WRITE THE NAME OF A FUNCTION AS IT IS ENTERED, and step one level deeper for as long as it runs.
///
/// ```ignore
/// let _here = crate::trace!("pick_face_edges_fillet");
/// ```
///
/// The guard is bound to a name and never used, and the underscore says so: what matters is that it lives
/// until the end of the function.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        let _entered = $crate::enter(&format!($($arg)*));
    };
}

/// WRITE ONE LINE AT THE DEPTH AS IT STANDS - a value, a branch taken, a refusal in words.
///
/// ```ignore
/// $crate::trace_line!("body={} edges={}", body, edges.len());
/// ```
#[macro_export]
macro_rules! trace_line {
    ($($arg:tt)*) => {
        $crate::note(&format!($($arg)*))
    };
}
