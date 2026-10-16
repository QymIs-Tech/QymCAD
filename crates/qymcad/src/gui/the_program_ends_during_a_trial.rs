//! THE PROGRAM ENDS WHILE A TRIAL OF A COMMAND RUNS, and the process is gone rather than hung or aborted.
//!
//! The fields of a command try what they hold on a thread of their own (`qymcad_part::trial`), and the end of a check
//! or of the window left that thread inside OCCT as the process ended (`qymcad_kernel::worker` now has the thread that
//! started it wait for it). Reported behaviour: the popups of every tool of the part drawn, the check done in a second,
//! its process hung 400 s once and ended with SIGABRT once of three runs.
#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// EVERY PROCESS IS GONE within 30 s, and it ended well: the check that draws the popups of every tool of the part
    /// - each starting its trial - run 40 times as a program of its own, 8 side by side.
    #[test]
    fn the_program_ends_while_a_trial_of_a_command_runs() {
        let me = std::env::current_exe().expect("the test program");
        let name = "gui::tool_popup_sweep::tests::every_part_tool_popup_offers_the_drivers";
        let mut hung = 0;
        let mut failed = Vec::new();
        for round in 0..5 {
            let children: Vec<_> =
                (0..8).map(|_| Command::new(&me).args(["--exact", name, "--test-threads=1"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("the child starts")).collect();
            let began = Instant::now();
            for mut child in children {
                loop {
                    if let Some(status) = child.try_wait().expect("the child is asked") {
                        if !status.success() {
                            failed.push(format!("round {round}: {status}"));
                        }
                        break;
                    }
                    if began.elapsed() > Duration::from_secs(30) {
                        let _ = child.kill();
                        let _ = child.wait();
                        hung += 1;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
        assert!(hung == 0 && failed.is_empty(), "of 40 programs ended during the trials of the popups, {hung} were still there 30 s later and {} ended badly: {failed:?}", failed.len());
    }
}
