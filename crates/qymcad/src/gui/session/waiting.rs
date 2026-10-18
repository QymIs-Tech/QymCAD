//! A CHECK WAITS FOR AN EVENT, NOT FOR A COUNT OF FRAMES. A rebuild, a load, a trial or a write of the window lands
//! some time after it was asked for, and that time is the machine's as much as the program's: a check that gave it 600
//! frames of 50 ms went red when the part of a single block was not built in that half minute, in a run of the whole
//! crate - more than a thousand checks in one process, sharing one lock of the kernel - and passed alone.
//!
//! So a check polls for the event itself, and the limit it gives it is the bound of a fault - an event that never
//! comes - counted over the time its own work had the kernel or the kernel was free: the time the lock was held by the
//! work of another check is the queue, not the wait. A red says what was waited for, how long, how much of it was the
//! queue, the load of the machine and who held the kernel.

use std::time::{Duration, Instant};

/// HOW LONG A CHECK WAITS FOR AN EVENT of the window that is its own: a rebuild of a few bodies takes well under a
/// second alone, and 30 s under the load of the whole crate.
#[cfg(test)]
pub(crate) const EVENT_LIMIT: Duration = Duration::from_secs(120);

/// THE LONGEST A WAIT GOES ON, the queue of the kernel counted too: past it the queue itself is the fault.
const CEILING: Duration = Duration::from_secs(600);

/// How often the event is looked for.
#[cfg(test)]
const POLL: Duration = Duration::from_millis(20);

/// Whether the kernel is held by work that is not this check's: neither its own thread nor a worker it started.
fn held_by_another() -> bool {
    let me = std::thread::current().name().unwrap_or_default().to_string();
    qymcad_kernel::gate_now().holder.is_some_and(|h| h.by != me && !h.by.ends_with(&format!("of {me}")))
}

/// POLL `happened` until it answers yes: each call is a frame of the window or a look at the program, and a pause of
/// `POLL` comes between two. The limit is `EVENT_LIMIT` of the time this check's work had.
///
/// # Panics
/// When the limit passes first, saying what was waited for and what was around.
#[cfg(test)]
pub(crate) fn until(what: &str, happened: impl FnMut() -> bool) {
    until_within(what, EVENT_LIMIT, happened);
}

/// The same, with a limit of its own.
#[cfg(test)]
pub(crate) fn until_within(what: &str, limit: Duration, mut happened: impl FnMut() -> bool) {
    let looked = waited(limit, || {
        happened() || {
            std::thread::sleep(POLL);
            false
        }
    });
    if let Err(late) = looked {
        panic!("{what} {}: {late}", qymcad_core::time_budget::SAID_NOT_IN_TIME);
    }
}

/// A WAIT PAST ITS LIMIT, in words: how long, how much of it was the queue of the kernel, the load of the machine, and
/// who held the kernel at the end.
pub(crate) struct Late {
    words: String,
}

impl std::fmt::Display for Late {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

/// CALL `look` until it answers yes, within `limit` of the time this check's work had; `look` makes its own pauses.
pub(crate) fn waited(limit: Duration, mut look: impl FnMut() -> bool) -> Result<(), Late> {
    let began = Instant::now();
    let mut queued = Duration::ZERO;
    let mut looked = Instant::now();
    while !look() {
        let now = Instant::now();
        if held_by_another() {
            queued += now.duration_since(looked);
        }
        looked = now;
        let waited = began.elapsed();
        if waited.saturating_sub(queued) >= limit || waited >= CEILING {
            let words =
                format!("waited {waited:.1?}, {queued:.1?} of it with the kernel held by the work of other checks; {}; {}", qymcad_core::time_budget::Machine::now(), qymcad_kernel::gate_now());
            return Err(Late { words });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    /// The words a wait failed with, or nothing when it did not fail.
    fn failed_with(work: impl FnOnce() + std::panic::UnwindSafe) -> String {
        std::panic::catch_unwind(work).err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default()
    }

    /// AN EVENT THAT NEVER COMES is named, with the words a red of time is told by.
    #[test]
    fn an_event_that_never_comes_is_named() {
        let said = failed_with(|| super::until_within("the event of the check", Duration::from_millis(100), || false));
        assert!(said.contains("the event of the check") && said.contains(qymcad_core::time_budget::SAID_NOT_IN_TIME), "the wait did not say what it waited for: {said:?}");
    }

    /// THE QUEUE OF THE KERNEL IS NOT THE WAIT: a neighbour holds the kernel 600 ms, the event comes 100 ms after it lets
    /// go, and a limit of 300 ms of the check's own time holds.
    #[test]
    fn the_queue_of_the_kernel_is_not_counted() {
        let (held, holding) = std::sync::mpsc::channel();
        let neighbour = std::thread::Builder::new()
            .name("a check beside".into())
            .spawn(move || {
                let _kernel = qymcad_kernel::kernel_gate();
                held.send(()).expect("the check waits for the kernel to be held");
                std::thread::sleep(Duration::from_millis(600));
            })
            .expect("a thread");
        holding.recv().expect("the neighbour holds the kernel");
        let began = Instant::now();
        let said = failed_with(|| super::until_within("the event after the queue", Duration::from_millis(300), || began.elapsed() > Duration::from_millis(700)));
        neighbour.join().expect("the neighbour lets the kernel go");
        assert!(said.is_empty(), "the time the kernel was held by another was counted as the wait: {said}");
    }
}
