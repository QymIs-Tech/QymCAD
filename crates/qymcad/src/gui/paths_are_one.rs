//! EVERYTHING THE PROGRAM KEEPS FOR A PERSON LIVES IN ONE FOLDER.
//!
//! Reported behaviour: "settings and crash reports turned up in `~/.local/share/cad`". That was repaired by
//! naming the folder in `qymcad-paths` - and the repair covered only what goes through that door. THE
//! SETTINGS DO NOT: they are written by the framework, which picks a folder of its own from the application
//! id. So the program went on keeping a person's things in two places at once, measured on Windows as
//! `AppData\Roaming\tech.qymis.cad\data\app.ron` for the settings against `AppData\Roaming\qymis\qymcad\` for
//! the schemes, the templates, the parts library and the crash reports.
#[cfg(test)]
mod tests {
    /// THE SETTINGS FILE IS ONE OF THE PROGRAM'S OWN FILES, and lies with them.
    #[test]
    fn the_settings_file_lies_among_the_program_s_own_files() {
        let Some(file) = qymcad_paths::settings_file() else {
            return; // a system with no notion of per-user folders keeps nothing to compare
        };
        let Some(dirs) = qymcad_paths::dirs() else { return };
        assert!(file.starts_with(dirs.data_dir()), "the settings are kept apart from everything else: {} against {}", file.display(), dirs.data_dir().display());
    }

    /// EVERYTHING A PERSON'S IN ONE FOLDER: the settings, the schemes, the templates, the parts library and the crash
    /// reports under one root - one folder on every system, not a "config" beside a "data". Reported behaviour: a
    /// person looking for their files found half of them in `~/.config/qymcad` and half in `~/.local/share/qymcad`.
    #[test]
    fn schemes_templates_library_and_reports_lie_with_the_settings() {
        let Some(settings) = qymcad_paths::settings_file() else { return };
        let home = settings.parent().expect("the settings lie in a folder").to_path_buf();
        for (what, dir) in [
            ("schemes", qymcad_paths::config("schemes")),
            ("templates", qymcad_paths::config("templates")),
            ("the parts library", qymcad_paths::data("library/parts")),
            ("crash reports", qymcad_paths::data("crashes")),
        ] {
            let dir = dir.unwrap_or_else(|| panic!("{what} have no folder"));
            assert!(dir.starts_with(&home), "{what} lie in {}, apart from the settings in {}", dir.display(), home.display());
        }
    }

    /// AND THE APPLICATION ASKS US FOR IT rather than letting the framework choose.
    ///
    /// This is read out of the source on purpose. The paths that differ are not this machine's: on Linux the
    /// framework's own choice happens to land in the same folder, so a check that compared two paths here
    /// would be green while Windows kept `AppData\Roaming\qymcad\data` and
    /// `AppData\Roaming\qymis\qymcad\data` side by side, and macOS `qymcad` beside `tech.qymis.qymcad`.
    #[test]
    fn the_framework_is_not_left_to_pick_the_folder() {
        let src = concat!(
            include_str!("file_doors.rs"),
            "\n",
            include_str!("gizmo_math.rs"),
            "\n",
            include_str!("look.rs"),
            "\n",
            include_str!("panel_bits.rs"),
            "\n",
            include_str!("startup.rs"),
            "\n",
            include_str!("view_anim.rs"),
            "\n",
            include_str!("../gui.rs")
        );
        let code: String = src.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        assert!(
            !code.contains("eframe::storage_dir("),
            "the folder of the settings is chosen by the framework again - it derives it from the application id, and on Windows and macOS that is not the folder the rest of a person's things live in"
        );
        assert!(code.contains("persistence_path: qymcad_paths::settings_file()"), "the settings are not pointed at the program's own folder: `persistence_path` is left unset");
    }
}

/// HOW MANY CORES THE KERNEL TAKES IS SAID WHERE A REBUILD STARTS, and nowhere else.
///
/// The setting is a number (`kernel_threads`: zero means all but one, one means single-threaded), and it has
/// to reach the kernel before the work. A rebuild that forgot to say so would use whatever the previous one
/// left set - and the single-threaded switch, the thing a person reaches for when a parallel pass is suspected
/// of lying, would quietly do nothing.
#[cfg(test)]
mod cores {
    #[test]
    fn every_rebuild_tells_the_kernel_how_many_cores_it_may_take() {
        let src = concat!(
            include_str!("../../../qymcad-ui-state/src/contexts.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/corners.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/dims.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/edits.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/fields.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/fonts.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/geom.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/gizmos.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/handles.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/hotkeys.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/jobs.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/joints.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/navigation.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/painting.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/rebuild.rs"),
            "\n",
            concat!(
                include_str!("../../../qymcad-ui-state/src/records/commands.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records/frame.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records/gizmos.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records/pieces.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records/scene.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records/tools.rs"),
                "\n",
                include_str!("../../../qymcad-ui-state/src/records.rs")
            ),
            "\n",
            include_str!("../../../qymcad-ui-state/src/scene.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/section.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/selection.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/settings.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/sketch_diag.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/sketch_edit.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/text.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/tool_steps.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/tree.rs"),
            "\n",
            include_str!("../../../qymcad-ui-state/src/lib.rs")
        );
        let mut built = 0usize;
        let mut told = 0usize;
        for (n, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("//") || !line.contains("qymcad_kernel::OcctKernel {") {
                continue;
            }
            built += 1;
            // within the dozen lines above, the setting must have been handed over
            let from = n.saturating_sub(12);
            let window = src.lines().skip(from).take(n - from).collect::<Vec<_>>().join("\n");
            if window.contains("set_parallel(") {
                told += 1;
            }
        }
        assert!(built >= 2, "the places that build a kernel for work are no longer found by this check ({built})");
        assert_eq!(built, told, "a rebuild builds a kernel without saying how many cores it may take");
    }

    /// AND OUT OF THE BOX THE REBUILD GETS MORE THAN ONE CORE.
    ///
    /// The switch is there for the rare machine where a parallel pass is suspected; a person who never opens
    /// the settings must get the fast path. One core is the OFF position, so the factory value must not be it.
    #[test]
    fn the_factory_setting_is_the_parallel_one() {
        let threads = qymcad_ui_state::Settings::default().kernel_threads;
        assert_ne!(threads, 1, "the factory setting hands a rebuild a single core - everyone gets the slow path until they find the setting");
    }
}
