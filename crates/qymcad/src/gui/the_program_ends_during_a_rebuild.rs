//! THE PROGRAM ENDS WHILE A REBUILD RUNS, and the process is gone rather than hung.
//!
//! At the end of a process the static destructors run, the one of OCCT's shared pool of threads among them, and it
//! stopped the threads of the pool under the rebuild still meshing on them: the rebuild waited on a pool thread gone,
//! the destructor waited on the rebuild, and the process stayed with no CPU, forever (the bridge now keeps the pool,
//! `g_pool_outlives_the_process`). Reported behaviour: run one to a process, the checks that end with a rebuild left
//! running hung 20 min each; 2 runs of 20 of one of them hung; a window closed during a rebuild may stay a process.
#[cfg(test)]
mod tests {
    use super::super::App;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// Set for the children the probe below starts: only then does `a_rebuild_is_left_running_as_the_program_ends`
    /// do its part.
    const CHILD: &str = "QYMCAD_END_DURING_A_REBUILD";

    /// THE CHILD'S PART: two parts built, the whole timeline marked and rebuilt in the background - and the check
    /// over at once, the rebuild still running, as a person closes the window. Run by itself (no `CHILD`) it does
    /// nothing.
    #[test]
    fn a_rebuild_is_left_running_as_the_program_ends() {
        if std::env::var_os(CHILD).is_none() {
            return;
        }
        let mut app = App::default();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        crate::gui::joint_flow::tests::add_part_at(&mut app, 100.0);
        crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());
        for n in app.project.timeline.iter_mut() {
            n.dirty = true;
        }
        app.spawn_regen();
    }

    /// EVERY PROCESS IS GONE within 30 s, and it ended well: the child above, 40 times, 8 side by side - where the end
    /// falls in the rebuild is chance (2 of 20 hung one at a time), and the load of the others makes it fall inside
    /// the work of the pool more often.
    #[test]
    fn the_program_ends_while_a_rebuild_runs() {
        let me = std::env::current_exe().expect("the test program");
        let name = "gui::the_program_ends_during_a_rebuild::tests::a_rebuild_is_left_running_as_the_program_ends";
        let mut hung = 0;
        let mut failed = Vec::new();
        for round in 0..5 {
            let children: Vec<_> =
                (0..8).map(|_| Command::new(&me).args(["--exact", name, "--test-threads=1"]).env(CHILD, "1").stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("the child starts")).collect();
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
        assert!(hung == 0 && failed.is_empty(), "of 40 programs ended during a rebuild, {hung} were still there 30 s later and {} ended badly: {failed:?}", failed.len());
    }
}
