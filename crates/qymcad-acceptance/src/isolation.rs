//! EVERY CHECK IN A PROCESS OF ITS OWN, and within its time and its memory.
//!
//! Checks run side by side, and a session is a whole program: its frames and its rebuild call the geometry kernel.
//! OCCT keeps shared state without protection, and the program takes its kernel lock only around a rebuild - one
//! program has one rebuild at a time, and its frames rarely touch the kernel while one runs. Two programs in one
//! process break that: the frame of one check meets the rebuild of another inside OCCT, and a check fails, or
//! passes, for a reason that is not in the program. So each check runs in a child process of the same test binary,
//! with the kernel all to itself, while the checks still run side by side.
//!
//! The child is the same binary asked for that one check. It is told apart by `QYMCAD_ACCEPTANCE_CHILD`;
//! `QYMCAD_ACCEPTANCE_IN_PROCESS` runs a check in place, for a debugger.
use std::time::{Duration, Instant};

/// What marks the child process that runs the check itself.
const CHILD: &str = "QYMCAD_ACCEPTANCE_CHILD";

/// How long one check may take unless said otherwise, and the variable that says otherwise, in seconds.
const PROBE_BUDGET: Duration = Duration::from_secs(300);
const PROBE_BUDGET_VAR: &str = "QYMCAD_PROBE_BUDGET_SECS";

/// How long the whole run of checks may take, in seconds; unbounded when not set.
const TIER_BUDGET_VAR: &str = "QYMCAD_TIER_BUDGET_SECS";

/// How much memory one check may hold unless said otherwise, and the variable that says otherwise, in megabytes. A
/// check is a whole program with its kernel: measured at 100 - 670 MB for the heaviest. One past 2 GB has run away,
/// and six of them side by side would take a machine with them - one check running away took a run capped at 16 GB
/// down in under two minutes, and the run said nothing of which check it was.
const PROBE_MEMORY_MB: u64 = 2048;
const PROBE_MEMORY_VAR: &str = "QYMCAD_PROBE_MEMORY_MB";

/// The file the time of every check run in a process of its own is added to, as `seconds<TAB>name` a line; nothing is
/// written when it is not set. `tools/gate.py` sets it and names the slowest checks of the run.
const PROBE_TIMES_VAR: &str = "QYMCAD_PROBE_TIMES";

/// ADD THE TIME `took` OF THE CHECK `path` to the file `file`, one line in one write: the checks of a run finish on
/// several threads at once, and a file opened for appending takes each whole write after the last.
fn note_time(file: &std::path::Path, path: &str, took: Duration) {
    use std::io::Write;
    let line = format!("{:.1}\t{path}\n", took.as_secs_f64());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// How much memory the process `pid` holds now, in megabytes; `None` where the system does not say (no `/proc`).
fn resident_mb(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb: u64 = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb / 1024)
}

/// How a check's own process came to an end.
enum Ended {
    /// It finished by itself.
    Finished(std::process::ExitStatus),
    /// It ran past its time and was stopped.
    PastTime,
    /// It held more memory than it may, this many megabytes, and was stopped.
    PastMemory(u64),
}

/// A budget in seconds read from the environment.
fn seconds(var: &str) -> Option<Duration> {
    std::env::var(var).ok().and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs)
}

/// RUN THE CHECK `name` of the module `module`: in place when this is its child process or a debugger asks for it,
/// otherwise in a child process of its own, within `budget` (or the budget of every check) and within what is left
/// of the run's budget.
///
/// # Panics
/// With the child's own words when the check fails, when it runs past its time, or when the child ran no check at
/// all - a name that no longer finds its check would otherwise pass without anything having been run.
pub fn run(module: &str, name: &str, budget: Option<Duration>, body: impl FnOnce()) {
    if std::env::var_os(CHILD).is_some() || std::env::var_os("QYMCAD_ACCEPTANCE_IN_PROCESS").is_some() {
        // every step of every check is looked at by the oracles
        qymcad::Session::watch_every_step(crate::oracles::after_every_step);
        body();
        return;
    }
    let budget = budget.or_else(|| seconds(PROBE_BUDGET_VAR)).unwrap_or(PROBE_BUDGET);
    let memory = std::env::var(PROBE_MEMORY_VAR).ok().and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(PROBE_MEMORY_MB);
    in_a_process_of_its_own(module, name, budget, memory, seconds(TIER_BUDGET_VAR));
}

/// The check `name` of `module` in a child process, within `budget`, within `memory` megabytes, and within what is
/// left of `tier` since the first check of this run started.
fn in_a_process_of_its_own(module: &str, name: &str, budget: Duration, memory: u64, tier: Option<Duration>) {
    static RUN_STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let run_started = *RUN_STARTED.get_or_init(Instant::now);
    let mut budget = budget;
    if let Some(tier) = tier {
        let left = tier.saturating_sub(run_started.elapsed());
        assert!(!left.is_zero(), "the run of checks is past its budget of {tier:?}: {name} was not started");
        budget = budget.min(left);
    }
    // the name the test binary knows the check by: the module path inside the binary, without the binary itself
    let path = module.split_once("::").map(|(_, rest)| format!("{rest}::{name}")).unwrap_or_else(|| name.to_string());
    let began = Instant::now();
    let ended = apart(&path, budget, memory, &[]);
    if let Some(file) = std::env::var_os(PROBE_TIMES_VAR) {
        note_time(std::path::Path::new(&file), &path, began.elapsed());
    }
    if let Err(said) = ended {
        panic!("{said}");
    }
}

/// RUN THE CHECK AT `path` of this test binary in a process of its own, within `budget` and `memory` megabytes, with
/// `env` set for it; answer what went wrong, in words, when anything did. For a check that runs other checks of its
/// own - a chain shrunk by playing its shorter forms, each apart, so that one which runs the memory away or never
/// comes to rest is a failure like any other rather than the end of the check that plays it.
///
/// # Errors
/// The words of the failure: past the time, past the memory, failed with its own words, or ran no check at all.
pub fn apart(path: &str, budget: Duration, memory: u64, env: &[(&str, String)]) -> Result<(), String> {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let exe = std::env::current_exe().expect("the test binary knows where it is");
    let out_dir = std::env::temp_dir();
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let log = out_dir.join(format!("qymcad-acceptance-{}-{run}-{}.log", std::process::id(), path.replace("::", "-")));
    let file = std::fs::File::create(&log).expect("the child's log opens");
    let mut command = std::process::Command::new(exe);
    command.args(["--exact", path, "--test-threads=1", "--nocapture"]).env(CHILD, "1");
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command.stdout(file.try_clone().expect("the log is shared by both streams")).stderr(file).spawn().expect("the check's own process starts");
    let started = Instant::now();
    let ended = loop {
        if let Some(status) = child.try_wait().expect("the child is waited on") {
            break Ended::Finished(status);
        }
        if started.elapsed() > budget {
            let _ = child.kill();
            let _ = child.wait();
            break Ended::PastTime;
        }
        if let Some(held) = resident_mb(child.id()).filter(|mb| *mb > memory) {
            let _ = child.kill();
            let _ = child.wait();
            break Ended::PastMemory(held);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    match ended {
        Ended::PastTime => Err(format!("{path} ran past its budget of {budget:?} and was stopped; it said:\n{said}")),
        Ended::PastMemory(held) => Err(format!("{path} ran past its memory of {memory} MB - it held {held} MB - and was stopped; it said:\n{said}")),
        Ended::Finished(s) if !s.success() => Err(format!("{path} failed:\n{said}")),
        Ended::Finished(_) if !said.contains("1 passed") => Err(format!("{path} ran no check in its own process - the name finds nothing:\n{said}")),
        Ended::Finished(_) => Ok(()),
    }
}

/// A CHECK, run in a process of its own. `probe!(budget = 600; fn name() { ... })` gives it a budget of its own, in
/// seconds.
#[macro_export]
macro_rules! probe {
    ($(#[$meta:meta])* fn $name:ident() $body:block) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::isolation::run(module_path!(), stringify!($name), None, || $body);
        }
    };
    (budget = $secs:expr; $(#[$meta:meta])* fn $name:ident() $body:block) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::isolation::run(module_path!(), stringify!($name), Some(std::time::Duration::from_secs($secs)), || $body);
        }
    };
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    /// A check for the runner to run: it sleeps five seconds when it is the child, and does nothing otherwise.
    #[test]
    fn a_slow_check_for_the_runner() {
        if std::env::var_os(super::CHILD).is_some() {
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    /// A check for the runner to run: it holds 300 MB for five seconds when it is the child, and does nothing
    /// otherwise.
    #[test]
    fn a_greedy_check_for_the_runner() {
        if std::env::var_os(super::CHILD).is_some() {
            let held = vec![1u8; 300 * 1024 * 1024]; // written, so the pages are really held
            std::thread::sleep(Duration::from_secs(5));
            assert!(held[held.len() - 1] == 1);
        }
    }

    /// A CHECK PAST ITS MEMORY IS STOPPED, says so and says how much it held - long before it would have finished.
    #[test]
    fn a_check_past_its_memory_is_stopped() {
        let began = std::time::Instant::now();
        let said = crate::refusal(|| super::in_a_process_of_its_own(module_path!(), "a_greedy_check_for_the_runner", Duration::from_secs(60), 100, None));
        assert!(said.contains("ran past its memory of 100 MB"), "a check of 300 MB under a memory of 100: {said:?}");
        assert!(began.elapsed() < Duration::from_secs(4), "the check was not stopped at its memory: it took {:?}", began.elapsed());
    }

    /// A check for the runner to run: it fails with words of its own when it is the child.
    #[test]
    fn a_failing_check_for_the_runner() {
        assert!(std::env::var_os(super::CHILD).is_none(), "the words this check fails with");
    }

    /// A CHECK PAST ITS BUDGET IS STOPPED, and says so.
    #[test]
    fn a_check_past_its_budget_is_stopped() {
        let began = std::time::Instant::now();
        let said = crate::refusal(|| super::in_a_process_of_its_own(module_path!(), "a_slow_check_for_the_runner", Duration::from_secs(1), super::PROBE_MEMORY_MB, None));
        assert!(said.contains("ran past its budget"), "a check of five seconds under a budget of one: {said:?}");
        assert!(began.elapsed() < Duration::from_secs(4), "the check was not stopped at its budget: it took {:?}", began.elapsed());
    }

    /// A FAILING CHECK FAILS IN THE RUNNER WITH ITS OWN WORDS.
    #[test]
    fn a_failing_check_fails_with_its_own_words() {
        let said = crate::refusal(|| super::in_a_process_of_its_own(module_path!(), "a_failing_check_for_the_runner", Duration::from_secs(60), super::PROBE_MEMORY_MB, None));
        assert!(said.contains("the words this check fails with"), "the failure came back as {said:?}");
    }

    /// A NAME THAT FINDS NO CHECK FAILS, rather than passing with nothing run.
    #[test]
    fn a_name_that_finds_no_check_fails() {
        let said = crate::refusal(|| super::in_a_process_of_its_own(module_path!(), "no_such_check", Duration::from_secs(60), super::PROBE_MEMORY_MB, None));
        assert!(said.contains("ran no check"), "a name that finds nothing: {said:?}");
    }

    /// THE TIMES OF CHECKS FINISHING TOGETHER ARE KEPT WHOLE: eight threads adding fifty lines each leave four hundred
    /// lines, every one of them `seconds<TAB>name`.
    #[test]
    fn the_times_of_checks_finishing_together_are_kept_whole() {
        let file = std::env::temp_dir().join(format!("qymcad-probe-times-{}.tsv", std::process::id()));
        let _ = std::fs::remove_file(&file);
        std::thread::scope(|s| {
            for t in 0..8 {
                let file = &file;
                s.spawn(move || {
                    for i in 0..50 {
                        super::note_time(file, &format!("module::check_{t}_{i}"), Duration::from_millis(1234));
                    }
                });
            }
        });
        let said = std::fs::read_to_string(&file).unwrap_or_default();
        let _ = std::fs::remove_file(&file);
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), 400, "lines kept: {}", lines.len());
        let whole = lines.iter().filter(|l| l.starts_with("1.2\tmodule::check_")).count();
        assert_eq!(whole, 400, "lines whole of 400: {whole}; the file:\n{said}");
    }

    /// A RUN PAST ITS BUDGET STARTS NO MORE CHECKS.
    #[test]
    fn a_run_past_its_budget_starts_no_more_checks() {
        let said = crate::refusal(|| super::in_a_process_of_its_own(module_path!(), "a_slow_check_for_the_runner", Duration::from_secs(60), super::PROBE_MEMORY_MB, Some(Duration::ZERO)));
        assert!(said.contains("past its budget") && said.contains("was not started"), "a run with no time left: {said:?}");
    }
}
