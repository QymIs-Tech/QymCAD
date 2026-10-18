//! THE BUDGETS OF TIME OF THE PROBES. A probe of time measures the machine as much as the program: run among the other
//! checks of a crate it reads their load - a budget of 3 s read over it on a sketch of 1 200 segments that takes 1.66 s
//! alone, three runs of nine. So a budget holds at the level of the probes of time alone (`QYMCAD_TIER=time`,
//! `tools/gate.py time`), where they run one at a time, and is scaled there by `QYMCAD_TIME_SCALE` (CI sets 2). What a
//! probe of time checks besides the time - that the work was done, and done right - holds at every level.
//!
//! THE COST OF THE WORK, NOT THE CLOCK ON THE WALL. A machine shared with other work gives a probe a share of its
//! processors, and the wall clock of the probe stretches with the share: under a load of 40-50 on 20 threads a click
//! meant to land 300 ms into a rebuild landed 266-435 ms late, a rebuild of 13 s took 30.4 s. What the program spent is
//! read from the kernel of the system instead (`Cost`): the time a thread ran or was blocked on what the program waits
//! for, less the time it stood ready with no processor free; or, when the work runs on threads of its own, the wall
//! times the share of their ready time the threads of the process ran. On a system without these counters (they are
//! read from `/proc`: macOS and Windows have none) the cost is the wall clock, as it was, and its words say so.
//!
//! `Doer::Process` in a process shared with other checks counts their threads too: it is an estimate of the share of
//! the machine the process got, not an exact measure of one check.

use std::time::{Duration, Instant};

/// Whether this run is the level of the probes of time.
pub fn at_the_level_of_time() -> bool {
    std::env::var("QYMCAD_TIER").as_deref() == Ok("time")
}

/// The budget on this machine: the budget times `QYMCAD_TIME_SCALE`, 1 when unset. A budget past what a `Duration`
/// holds - `Duration::MAX`, a step with no budget - stays no budget rather than overflowing the multiplication.
pub fn scaled(budget: Duration) -> Duration {
    let k = std::env::var("QYMCAD_TIME_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|k| *k >= 1.0).unwrap_or(1.0);
    Duration::try_from_secs_f64(budget.as_secs_f64() * k).unwrap_or(Duration::MAX)
}

/// Whether `took` keeps within `budget`: always, but at the level of the probes of time, against the scaled budget.
pub fn within(took: Duration, budget: Duration) -> bool {
    !at_the_level_of_time() || took <= scaled(budget)
}

/// WHAT A RED OF TIME SAYS FIRST, read by `tools/gate.py`: a work over its budget, and a wait for an event that did not
/// come. The gate names such a red with the load the machine was under.
pub const SAID_OVER_BUDGET: &str = "over its budget of time";
pub const SAID_NOT_IN_TIME: &str = "did not happen in time";

/// WHAT A LINE OF A MEASURE STARTS WITH: a probe of time prints its measure green or red, so how much of the budget is
/// left is read without changing the budget, and the runner of the acceptance set passes such lines on from a probe's
/// own process.
pub const MEASURED: &str = "measured: ";

/// PRINT the cost of `what` as a line of a measure.
pub fn tell(what: &str, cost: Cost) {
    eprintln!("{MEASURED}{what}: {cost}");
}

/// WHAT DOES THE WORK THAT IS MEASURED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doer {
    /// The thread that measures, by itself: the cost is its time less the time it stood ready with no processor.
    Thread,
    /// The whole process, the work on threads of its own: the cost is the wall clock less the share of it the threads
    /// of the process stood ready with no processor - each thread of a work slowed by the load as much as the rest, the
    /// wall shrinks by the share they got. Not the processor time of the process: a rebuild of the case sample spent
    /// 42 s on processors in 13.7 s of the wall, its booleans on 19 threads. The share is read from every thread of the
    /// process: alone in it - a probe of the acceptance set, a check of the gate run one at a time - it is the work's
    /// own; beside other checks it is theirs too, slowed by the same machine. A work of a few milliseconds is read to a
    /// tick of the scheduler.
    Process,
}

/// THE COST OF A PIECE OF WORK.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    /// What the budget is held against: for a thread its time on the wall less `waited` - running, or blocked on what
    /// the program waits for, so a frame stalled on a lock is a slow frame - for a process the wall times the share of
    /// their ready time its threads ran (`Doer::Process`).
    pub work: Duration,
    /// The clock on the wall, from start to end.
    pub wall: Duration,
    /// The time on a processor: of the thread - read to a tick of the scheduler, a few milliseconds, as the system
    /// counts a running thread's time at its ticks - or of every thread of the process, to 10 ms.
    pub cpu: Duration,
    /// How long the threads stood ready to run with no processor free for them: the share the machine gave to others.
    /// For a process, of the threads still there at the end - one that ended took its counters with it.
    pub waited: Duration,
    /// Where the cost was read from.
    pub source: Source,
}

/// The least processor time of a process its share is read from: three ticks of `/proc/self/stat`.
const PROCESS_TICKS: Duration = Duration::from_millis(30);

/// WHERE A COST WAS READ FROM.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// The counters of the system: the time run and waited by every thread.
    Counters,
    /// The wall clock, the system having no counters: the work is the wall, the machine's share in it.
    #[default]
    Wall,
}

impl std::fmt::Display for Cost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.source {
            Source::Counters => {
                write!(f, "{:.3?} of work ({:.3?} on the wall: {:.3?} on a processor, {:.3?} waiting for one; the counters of the system)", self.work, self.wall, self.cpu, self.waited)
            }
            Source::Wall => write!(f, "{:.3?} of work (the wall clock: this system has no counters of the time of threads)", self.work),
        }
    }
}

/// A WORK AND WHAT IT COST.
pub struct Timed<T> {
    pub out: T,
    pub cost: Cost,
}

/// THE COUNTERS OF THE SYSTEM AT ONE INSTANT: the clock on the wall, the time run and waited by the threads, and the
/// processor time of the whole process; `counted` is false where the system gives no counters.
struct Reading {
    at: Instant,
    threads: Vec<ThreadTime>,
    process: Duration,
    counted: bool,
}

/// The time one thread ran on a processor and stood ready waiting for one, by its id (empty for the measuring thread).
struct ThreadTime {
    id: String,
    ran: Duration,
    waited: Duration,
}

/// The time a thread ran and waited, from its `schedstat`: nanoseconds on a processor, nanoseconds ready on a queue.
fn thread_time(id: String, schedstat: &str) -> Option<ThreadTime> {
    let text = std::fs::read_to_string(schedstat).ok()?;
    let mut f = text.split_whitespace().map(|v| v.parse::<u64>().ok());
    let ran = f.next()??;
    let waited = f.next()??;
    Some(ThreadTime { id, ran: Duration::from_nanos(ran), waited: Duration::from_nanos(waited) })
}

/// The processor time of the whole process, from `/proc/self/stat`: user and system, in ticks of 1/100 s (the tick of
/// that file is fixed by the interface of the system, not by the machine).
fn process_time() -> Option<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields: Vec<&str> = stat.rsplit(')').next()?.split_whitespace().collect();
    let ticks = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    Some(Duration::from_millis(ticks * 10))
}

/// THE COUNTERS AND THE CLOCK, read in the order `when` says. A thread taken off its processor between the two leaves
/// that wait in the counters and not in the clock, or the other way round; the order is chosen so the wait is never
/// charged as work - at the start the counters first, at the end the clock first. Read the other way, 355 frames of 0.80 s
/// among spinning threads were charged 0.20 s while the whole of them did 0.08 s of work.
fn read(doer: Doer, when: Moment) -> Reading {
    let clock_first = Instant::now();
    let threads: Vec<ThreadTime> = match doer {
        Doer::Thread => thread_time(String::new(), "/proc/thread-self/schedstat").into_iter().collect(),
        Doer::Process => std::fs::read_dir("/proc/self/task")
            .map(|d| d.flatten().filter_map(|e| thread_time(e.file_name().to_string_lossy().into_owned(), &format!("{}/schedstat", e.path().display()))).collect())
            .unwrap_or_default(),
    };
    let process = process_time();
    let at = match when {
        Moment::Start => Instant::now(),
        Moment::End => clock_first,
    };
    Reading { at, counted: !threads.is_empty() && process.is_some(), threads, process: process.unwrap_or_default() }
}

/// Which end of a measure a reading is.
#[derive(Clone, Copy)]
enum Moment {
    Start,
    End,
}

/// A MEASURE STARTED: the cost of whatever runs from `start` to `cost`.
pub struct Meter {
    doer: Doer,
    from: Reading,
}

impl Meter {
    pub fn start(doer: Doer) -> Self {
        Self { doer, from: read(doer, Moment::Start) }
    }

    /// The cost from the start to now.
    pub fn cost(&self) -> Cost {
        let to = read(self.doer, Moment::End);
        let wall = to.at.duration_since(self.from.at);
        if !self.from.counted || !to.counted {
            return Cost { work: wall, wall, cpu: wall, waited: Duration::ZERO, source: Source::Wall };
        }
        // a thread started during the work counts from nothing; one that ended took its waiting with it
        let before = |id: &str| self.from.threads.iter().find(|t| t.id == id);
        let since = |t: &ThreadTime, of: fn(&ThreadTime) -> Duration| of(t).saturating_sub(before(&t.id).map_or(Duration::ZERO, of));
        let waited = to.threads.iter().map(|t| since(t, |t| t.waited)).sum();
        match self.doer {
            Doer::Thread => {
                let cpu = to.threads.iter().map(|t| since(t, |t| t.ran)).sum();
                Cost { work: wall.saturating_sub(waited), wall, cpu, waited, source: Source::Counters }
            }
            Doer::Process => {
                // the time run is the process's own count, which keeps the threads that ended - a worker of a rebuild
                // ends before the measure is read, and the threads still there would leave only the idle one that
                // waited for it. A work shorter than a few ticks of that count (10 ms each) is read as its wall.
                let cpu = to.process.saturating_sub(self.from.process);
                let ran = cpu.max(to.threads.iter().map(|t| since(t, |t| t.ran)).sum());
                let work = if ran < PROCESS_TICKS { wall } else { wall.mul_f64(ran.as_secs_f64() / (ran + waited).as_secs_f64()) };
                Cost { work, wall, cpu, waited, source: Source::Counters }
            }
        }
    }
}

/// THE COST OF `work`, done by `doer`.
pub fn cost_of<T>(doer: Doer, work: impl FnOnce() -> T) -> Timed<T> {
    let meter = Meter::start(doer);
    let out = work();
    Timed { out, cost: meter.cost() }
}

/// HOW BUSY THE MACHINE IS: the load of the last minute against the threads it has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Machine {
    pub load: f64,
    pub threads: usize,
}

impl Machine {
    pub fn now() -> Self {
        let load = std::fs::read_to_string("/proc/loadavg").ok().and_then(|l| l.split_whitespace().next().and_then(|v| v.parse().ok())).unwrap_or(0.0);
        Self { load, threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }
    }
}

impl std::fmt::Display for Machine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the machine under a load of {:.1} on {} threads", self.load, self.threads)
    }
}

/// THE WORDS OF A WORK OVER ITS BUDGET: what, its cost, the budget, and how busy the machine was.
pub fn over(what: &str, cost: Cost, budget: Duration) -> String {
    format!("{what} {SAID_OVER_BUDGET}: {cost}, budget {budget:.3?}, {}", Machine::now())
}

/// Whether `cost` keeps within `budget` scaled for this machine, at every level.
pub fn holds(cost: Cost, budget: Duration) -> bool {
    cost.work <= scaled(budget)
}

#[cfg(test)]
mod tests {
    use super::{cost_of, Doer};
    use std::time::{Duration, Instant};

    /// A STEP WITH NO BUDGET (`Duration::MAX`) stays with none when scaled, rather than overflowing: a probe of time that
    /// gave a step no budget panicked inside the multiplication.
    #[test]
    fn no_budget_stays_no_budget() {
        assert_eq!(super::scaled(Duration::MAX), Duration::MAX);
        assert!(super::within(Duration::from_secs(3600), Duration::MAX));
    }

    /// The checks of the measure one at a time: a crowd of one would be counted in the process of another.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn alone() -> std::sync::MutexGuard<'static, ()> {
        ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A CROWD: four spinning threads a processor of the machine, stopped and joined when dropped. Threads of this
    /// process, so it gathers the same on every system; the process measure counts them as it counts the threads of
    /// other checks beside it.
    #[cfg(target_os = "linux")]
    struct Crowd {
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        threads: Vec<std::thread::JoinHandle<()>>,
    }

    #[cfg(target_os = "linux")]
    impl Crowd {
        fn gather() -> Self {
            let n = 4 * std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let threads = (0..n)
                .map(|_| {
                    let stop = stop.clone();
                    std::thread::spawn(move || {
                        let mut k = 0u64;
                        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                            k = k.wrapping_add(1);
                        }
                        std::hint::black_box(k);
                    })
                })
                .collect();
            Self { stop, threads }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for Crowd {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            for t in self.threads.drain(..) {
                let _ = t.join();
            }
        }
    }

    /// Spin for `span` of the wall: work for a processor and nothing else.
    fn spin(span: Duration) -> u64 {
        let until = Instant::now() + span;
        let mut n = 0u64;
        while Instant::now() < until {
            n = n.wrapping_add(1);
        }
        n
    }

    /// A COST SAYS WHERE IT WAS READ FROM: the counters of the system on Linux, the wall clock where there are none.
    #[test]
    fn a_cost_says_where_it_was_read_from() {
        let _alone = alone();
        let cost = cost_of(Doer::Thread, || spin(Duration::from_millis(20))).cost;
        let words = cost.to_string();
        if cfg!(target_os = "linux") {
            assert!(cost.source == super::Source::Counters && words.contains("the counters of the system"), "read on Linux: {words}");
        } else {
            assert!(cost.source == super::Source::Wall && words.contains("no counters") && cost.work == cost.wall, "read with no counters: {words}");
        }
    }

    /// A THREAD KEPT WAITING FOR A PROCESSOR IS NOT CHARGED FOR THE WAIT: with four spinning threads a processor, the
    /// work of a thread is a part of its wall, and the rest is what it waited. Measured on 20 threads under a load of 27-40
    /// beside, four runs: 0.40-0.42 s of the wall, 0.01-0.03 s of work, 0.37-0.40 s waited. Linux only: the wait is read from its counters, and macOS
    /// and Windows have none - there the cost is the wall clock.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_wait_for_a_processor_is_not_work() {
        let _alone = alone();
        let crowd = Crowd::gather();
        let cost = cost_of(Doer::Thread, || spin(Duration::from_millis(400))).cost;
        drop(crowd);
        eprintln!("a thread among spinning ones: {cost}");
        assert!(cost.waited > Duration::ZERO && cost.work < cost.wall.mul_f64(0.75), "a thread among spinning ones: {cost}");
        assert!(cost.work <= cost.cpu + Duration::from_millis(20), "spinning is all on a processor: {cost}");
    }

    /// A THREAD BLOCKED ON WHAT THE PROGRAM WAITS FOR IS CHARGED FOR IT: a frame stalled on a lock is a slow frame. The
    /// wake after the block waits for a processor like any other, and a busy machine takes that wait off: the block is
    /// held against most of its 80 ms, not all, where a block taken for a wait would leave next to nothing.
    #[test]
    fn a_block_is_work() {
        let _alone = alone();
        let cost = cost_of(Doer::Thread, || std::thread::sleep(Duration::from_millis(80))).cost;
        assert!(cost.work >= Duration::from_millis(60), "a sleep of 80 ms: {cost}");
    }

    /// THE WORK OF A PROCESS IS ITS WALL LESS ITS THREADS' WAIT: two workers spin 0.4 s among four spinning threads a
    /// processor and are read while still there; the work is a part of the wall, as their share of a processor was.
    /// Measured under a load of 27-40 beside, four runs: 0.41-0.60 s of the wall, 0.07-0.12 s of work. Linux only, as the
    /// wait is read from its counters.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_is_not_charged_for_the_wait_of_its_threads() {
        let _alone = alone();
        let crowd = Crowd::gather();
        let (done, read) = (std::sync::Arc::new(std::sync::Barrier::new(3)), std::sync::Arc::new(std::sync::Barrier::new(3)));
        let meter = super::Meter::start(Doer::Process);
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let (done, read) = (done.clone(), read.clone());
                std::thread::spawn(move || {
                    spin(Duration::from_millis(400));
                    done.wait();
                    read.wait();
                })
            })
            .collect();
        done.wait();
        let cost = meter.cost();
        read.wait();
        workers.into_iter().for_each(|t| drop(t.join()));
        drop(crowd);
        eprintln!("two workers among spinning threads: {cost}");
        assert!(cost.waited > Duration::ZERO && cost.work < cost.wall.mul_f64(0.75), "two workers among spinning threads: {cost}");
    }

    /// A WORKER THAT ENDED BEFORE THE MEASURE IS STILL COUNTED: a worker spins 300 ms and is joined, and the process is
    /// charged about its wall - read from the threads still there, the share would be the idle one's that waited for it.
    #[test]
    fn a_worker_that_ended_is_counted() {
        let _alone = alone();
        let cost = cost_of(Doer::Process, || std::thread::spawn(|| spin(Duration::from_millis(300))).join()).cost;
        assert!(cost.work >= Duration::from_millis(250), "a worker spinning 300 ms: {cost}");
    }
}
