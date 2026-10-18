//! Guard on THE WALL CLOCK IN THE CHECKS: a check does not read the clock on the wall itself. What it times is the work
//! of the program (`qymcad_core::time_budget::cost_of`, `Meter`), and what it waits for is an event
//! (`gui::session::waiting::until`, `Session::wait_for`).
//!
//! Measured on 2026-10-18, before the measure: 54 files of checks read the wall clock, some 35 of them held what they
//! read to a constant, and the gate went red three times on one run on such checks while the machine built other work
//! beside it - a rebuild of 13 s read 30.4 s, a frame of a step 14-21 s against 3, a click meant 300 ms into a rebuild
//! landed 435 ms in. Each red was rerun one at a time on a branch and on the main line to be told from a fault.
//!
//! A check that reads the clock for another reason - a measure printed and never held, a clock given to the program as
//! data, the clock of an event the check stands in for - is named below with the number of its reads and why.

#[cfg(test)]
mod tests {
    /// A file of checks allowed to read the wall clock, how many times, and why.
    struct Allowed {
        path: &'static str,
        reads: usize,
        why: &'static str,
    }

    const ALLOWED: [Allowed; 29] = [
        Allowed { path: "qymcad/src/viewport_gpu.rs", reads: 1, why: "the time of the self-test of the graphics device, printed beside its answer and never held" },
        Allowed {
            path: "qymcad/src/gui/a_step_assembly_lands_as_its_tree.rs",
            reads: 1,
            why: "the time of writing a big assembly out and reading it back, printed by a check run by hand on a file of this machine",
        },
        Allowed { path: "qymcad/src/gui/drag_cost_look.rs", reads: 6, why: "a measure printed and never held to a budget, run by hand: the cost of the frames of a drag" },
        Allowed { path: "qymcad/src/gui/scene_cost_look.rs", reads: 5, why: "a measure printed and never held to a budget, run by hand: the cost of the parts of a scene" },
        Allowed { path: "qymcad/src/gui/format_samples_look.rs", reads: 1, why: "a measure printed and never held to a budget, run by hand: the time of reading the samples of every format" },
        Allowed { path: "qymcad/src/gui/file_ask.rs", reads: 1, why: "the clock given to the program as data: a rebuild in flight is made by hand, started now" },
        Allowed { path: "qymcad/src/gui/modal_barrier.rs", reads: 1, why: "the clock given to the program as data: the splash of the start told to be over now" },
        Allowed { path: "qymcad/src/gui/regen_live_view.rs", reads: 2, why: "the clock given to the program as data: a rebuild made by hand, started now or past the time its sign is due" },
        Allowed { path: "qymcad/src/gui/import_door/tests/meshes.rs", reads: 1, why: "the clock given to the program as data: a job in the background made by hand, started now" },
        Allowed { path: "qymcad/src/gui/tuned_constants.rs", reads: 2, why: "the clock given to the program as data: the last autosave put a minute and a half back" },
        Allowed {
            path: "qymcad/src/gui/tests/command_flow_tests/saving.rs",
            reads: 2,
            why: "the clock as data for a job made by hand, and a quit that must give up within seconds - the load only lengthens its sleep",
        },
        Allowed {
            path: "qymcad/src/gui/tests/a_quick_rebuild_does_not_blink.rs",
            reads: 4,
            why: "the clock of the event: every frame is read against the moment its rebuild started, and a rebuild is dated an hour ahead",
        },
        Allowed { path: "qymcad/src/gui/the_program_ends_during_a_rebuild.rs", reads: 1, why: "eight programs started as processes are given the bound of a hang to end, 120 s" },
        Allowed { path: "qymcad/src/gui/the_program_ends_during_a_trial.rs", reads: 1, why: "eight programs started as processes are given the bound of a hang to end, 120 s" },
        Allowed { path: "qymcad/src/gui/window.rs", reads: 4, why: "the check of the measure of a frame reads the wall to show what the measure leaves out" },
        Allowed { path: "qymcad/src/gui/session/waiting.rs", reads: 1, why: "the one wait for an event itself, counting the queue of the kernel apart from the wait" },
        Allowed { path: "qymcad-acceptance/src/isolation.rs", reads: 2, why: "the checks of the runner hold that a check past its budget or its memory is stopped within seconds" },
        Allowed { path: "qymcad-core/src/time_budget.rs", reads: 2, why: "the checks of the measure itself spin for a span of the wall among a crowd of spinning threads" },
        Allowed { path: "qymcad-core/tests/sketch_speed.rs", reads: 1, why: "a measure printed and never held to a budget, run by hand: the table of the speed of every kind of sketch" },
        Allowed { path: "qymcad-core/tests/state_key.rs", reads: 3, why: "two works timed in turn and told against each other, the best of several, never against a constant" },
        Allowed { path: "qymcad-kernel/tests/a_big_iges_is_measured.rs", reads: 1, why: "a measure printed and never held to a budget, run by hand: reading a big IGES file of this machine" },
        Allowed { path: "qymcad-meshfit/tests/a_mesh_becomes_a_recognised_body.rs", reads: 1, why: "a measure printed and never held to a budget, run by hand: the time of recognising meshes" },
        Allowed { path: "qymcad-meshfit/tests/real_meshes_are_measured.rs", reads: 5, why: "a measure printed and never held to a budget, run by hand: the steps of recognising real meshes" },
        Allowed {
            path: "qymcad-testkit/tests/a_parallel_boolean_gives_the_same_body.rs",
            reads: 1,
            why: "the time of a rebuild on one thread and on many, printed beside a comparison of the bodies, never held",
        },
        Allowed { path: "qymcad-testkit/tests/what_a_grid_of_holes_costs.rs", reads: 1, why: "a measure printed and never held to a budget, run by hand: the cost of a grid of holes" },
        Allowed { path: "qymcad-testkit/tests/what_a_rebuild_costs.rs", reads: 2, why: "a measure printed and never held to a budget, run by hand: the cost of opening and rebuilding samples" },
        Allowed { path: "qymcad-ui-state/src/icons/tests/watch.rs", reads: 2, why: "a wait for the thread of the watcher of a theme, bounded by 30 s against its poll of 250 ms" },
        Allowed { path: "qymcad-ui-state/tests/a_quick_rebuild_paints_no_sign.rs", reads: 2, why: "the clock given to the sign as data: a moment from which every step of a rebuild is told" },
        Allowed { path: "qymcad-ui-state/tests/installed_fonts.rs", reads: 1, why: "a wait for the walk of the fonts in its thread, bounded by 30 s against its half a second" },
    ];

    /// What reads the wall clock.
    const READ: &str = "Instant::now()";

    /// The part of `text` that is checks: the whole of a file of checks, or what follows a module of them.
    fn checks_of(path: &std::path::Path, text: &str) -> String {
        let under_tests = path.components().any(|c| c.as_os_str() == "tests");
        if under_tests || qymcad_i18n::ratchet::is_test_module(path) {
            return text.to_string();
        }
        ["#[cfg(test)]\nmod ", "#[cfg(test)]\npub(crate) mod ", "#[cfg(test)]\npub(in crate::gui) mod ", "#[cfg(test)]\npub(super) mod "]
            .iter()
            .filter_map(|cut| text.find(cut))
            .min()
            .map(|at| text[at..].to_string())
            .unwrap_or_default()
    }

    /// Every file of the sources and of the checks of every crate.
    fn every_file() -> Vec<std::path::PathBuf> {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut stack: Vec<std::path::PathBuf> =
            std::fs::read_dir(&root).expect("the crates read").flatten().flat_map(|e| [e.path().join("src"), e.path().join("tests")]).filter(|p| p.is_dir()).collect();
        assert!(stack.len() > 2, "the guard walks every crate, not the one it lives in");
        let mut files = Vec::new();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        files
    }

    #[test]
    fn every_exception_says_why() {
        for a in ALLOWED {
            assert!(a.why.split_whitespace().count() >= 8, "the exception of {} gives no reason", a.path);
        }
    }

    /// A CHECK DOES NOT READ THE WALL CLOCK ITSELF: it times the work, or it waits for the event.
    #[test]
    fn no_check_reads_the_wall_clock() {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut found = Vec::new();
        for p in every_file() {
            let name = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if name.ends_with("wall_clock_guard.rs") {
                continue;
            }
            let reads = checks_of(&p, &std::fs::read_to_string(&p).unwrap_or_default()).matches(READ).count();
            let allowed = ALLOWED.iter().find(|a| a.path == name).map_or(0, |a| a.reads);
            if reads > allowed {
                found.push(format!("{name}: {reads} reads of the wall clock, {allowed} allowed"));
            } else if reads < allowed {
                found.push(format!("{name}: {reads} reads of the wall clock, {allowed} allowed - lower its mark"));
            }
        }
        assert!(
            found.is_empty(),
            "a check reads the wall clock: time the work of the program (`time_budget::cost_of`, `Meter`) or wait for the event (`session::waiting::until`, `Session::wait_for`), or name the file here with why; {} of them:\n{}",
            found.len(),
            found.join("\n")
        );
    }
}
