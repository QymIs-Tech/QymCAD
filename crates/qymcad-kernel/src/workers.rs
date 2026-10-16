//! THE THREADS THAT WORK IN OCCT END BEFORE THE THREAD THAT STARTED THEM, and so before the process.
//!
//! A process ended with a thread still inside OCCT ran the static destructors of OCCT under it: the pool of threads
//! stopped under a mesh (the process stayed with no CPU, forever) or the allocator went from under a list (SIGABRT).
//! Waiting at `atexit` is too late - some statics of OCCT are made lazily during the work, and their destructors run
//! before any handler registered earlier. So the wait is in the thread that started the work: when it ends - a check,
//! with its thread - it waits for its workers; the window's thread, whose end is the end of the program, waits by
//! `wait_for_workers` at the end of the program. Reported behaviour: run one to a process, checks that ended with a
//! rebuild or a trial of a command still running hung 20 min or ended with SIGABRT.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// HOW LONG A THREAD THAT ENDS WAITS FOR ITS WORKERS, at most: as long as the end of the window waits for a rebuild.
const WAIT: Duration = Duration::from_secs(120);

/// The workers a thread started that are still running; waited for when the thread ends.
struct Started {
    live: Arc<AtomicUsize>,
}

impl Drop for Started {
    fn drop(&mut self) {
        wait_until_none(&self.live);
    }
}

thread_local! {
    static STARTED: Started = Started { live: Arc::new(AtomicUsize::new(0)) };
}

/// One worker less when its thread is done with its work, by a return or a panic.
struct Done(Arc<AtomicUsize>);

impl Drop for Done {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// RUN `work` ON A THREAD OF ITS OWN that the calling thread waits for when it ends - every thread that works in OCCT
/// is started by this.
pub fn worker<F: FnOnce() + Send + 'static>(work: F) -> std::thread::JoinHandle<()> {
    let live = STARTED.with(|s| s.live.clone());
    live.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        let _done = Done(live);
        work();
    })
}

/// WAIT FOR THE WORKERS THIS THREAD STARTED, at most `WAIT`: the end of the program calls it, its thread's own end being
/// the end of the process, where no destructor of a thread runs.
pub fn wait_for_workers() {
    STARTED.with(|s| wait_until_none(&s.live));
}

fn wait_until_none(live: &AtomicUsize) {
    let until = Instant::now() + WAIT;
    while live.load(Ordering::SeqCst) > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
}
