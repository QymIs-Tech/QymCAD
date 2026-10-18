//! THE PANELS AS ONE TEXT, for the checks that read them.
//!
//! Fifty-seven checks assert that a button, a tree row or a string exists by looking for it in the panels'
//! source. That is a strong guard - a tool nobody can reach does not exist for a person, and a text search is
//! the only thing that notices when the button is gone - and splitting the file must not weaken it.
//!
//! THE WORKBENCHES ARE CRATES NOW, and their bars went with them. The list below follows the code: a guard
//! that reads a file BY NAME guards that file and no other, and leaving the list behind would have left
//! twenty-five checks reading the remnants - green, and blind. It happened once already, with the drawing.

/// THE MARK BETWEEN TWO FILES OF A GLUE: a line no source holds, so the working part of each file is taken apart from
/// the next (`qymcad_i18n::ratchet::working_part`). A literal, as `concat!` takes only literals; the same text as
/// `qymcad_i18n::ratchet::FILE_BREAK`.
#[cfg(test)]
macro_rules! file_break {
    () => {
        "\n// ---- the next file of the glue ----\n"
    };
}
#[cfg(test)]
pub(crate) use file_break;

/// The whole of the panels, in the order the file used to have them.
#[cfg(test)]
pub(crate) const PANELS: &str = concat!(
    concat!(
        concat!(
            include_str!("../../../qymcad-part/src/apply/copies.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/faces.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/solids.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/update.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply.rs")
        ),
        file_break!(),
        concat!(
            include_str!("../../../qymcad-part/src/bars/feat_bar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/options.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/toolbar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/commands.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars.rs")
        ),
        file_break!(),
        include_str!("../../../qymcad-part/src/params.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/picks.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/popups.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/reopen.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/start.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/topology.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("../../../qymcad-sketch/src/clicks.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/constraints.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/corners.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/dims.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/drag.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/extend.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/menu.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/offset.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/popups.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/props.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/snap.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/ties.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("../../../qymcad-assembly/src/bars.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/commands.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/gizmo.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/picks.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/popup.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/slots.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("panels_tree/build.rs"),
        file_break!(),
        include_str!("panels_tree/copy.rs"),
        file_break!(),
        include_str!("panels_tree/rename.rs"),
        file_break!(),
        include_str!("panels_tree/rows.rs"),
        file_break!(),
        include_str!("panels_tree.rs")
    ),
    file_break!(),
    include_str!("panels_props.rs"),
    file_break!(),
    // the properties of an edge and of a corner, which the properties panel opens
    include_str!("props_pick.rs"),
    file_break!(),
    // the frame drawn in space and its right-button menu
    include_str!("frame_select.rs"),
    file_break!(),
    // the "make a part" item of a piece, on the canvas and on its row of the tree, and the name it asks
    include_str!("piece_part.rs"),
    file_break!(),
    include_str!("panels_bars.rs"),
    file_break!(),
    // the export submenu the File menu and the tree's component menu both open
    include_str!("export_menu.rs"),
    file_break!(),
    concat!(
        include_str!("panels_windows/dialogs.rs"),
        file_break!(),
        include_str!("panels_windows/library.rs"),
        file_break!(),
        include_str!("panels_windows/params.rs"),
        file_break!(),
        include_str!("panels_windows/schemes.rs"),
        file_break!(),
        include_str!("panels_windows/settings.rs"),
        file_break!(),
        include_str!("panels_windows/layout_settings.rs"),
        file_break!(),
        include_str!("panels_windows.rs")
    ),
    file_break!(),
    include_str!("icon_themes.rs"),
    file_break!(),
    include_str!("icon_themes/discovery.rs"),
    file_break!(),
    include_str!("icon_themes/appearance_section.rs"),
    file_break!(),
    include_str!("icon_themes/conflicts.rs"),
    file_break!(),
    include_str!("icon_themes/sidebar.rs"),
    file_break!(),
    include_str!("icon_themes/manager_window.rs"),
    file_break!(),
    include_str!("icon_themes/gallery.rs"),
    file_break!(),
    include_str!("icon_themes/packager_dialog.rs"),
);

/// THE WINDOWS AND DIALOGUES OF THE APPLICATION, as one text.
///
/// Separate from `PANELS` on purpose: several checks over `PANELS` assert that something is ABSENT from
/// it, and widening that constant would change what those checks mean. A window is not a panel.
#[cfg(test)]
pub(crate) const WINDOWS: &str = concat!(
    include_str!("help_window.rs"),
    file_break!(),
    include_str!("hotkeys.rs"),
    file_break!(),
    include_str!("report_problem.rs"),
    file_break!(),
    include_str!("file_ask.rs"),
    file_break!(),
    include_str!("import_scale.rs"),
    file_break!(),
    include_str!("io_jobs.rs"),
    file_break!(),
    include_str!("start_screen.rs"),
    file_break!(),
    include_str!("command_search.rs"),
    file_break!(),
    include_str!("graphics_settings.rs"),
);

/// THE WINDOWS AS ONE TEXT: everywhere a window is opened. The panels plus the three files that open a window
/// without being a panel - the start screen, the command search and the menu that raises them.
#[cfg(test)]
const WINDOW_OPENERS: &str = concat!(
    concat!(
        concat!(
            include_str!("../../../qymcad-part/src/apply/copies.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/faces.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/solids.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/update.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply.rs")
        ),
        file_break!(),
        concat!(
            include_str!("../../../qymcad-part/src/bars/feat_bar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/options.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/toolbar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/commands.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars.rs")
        ),
        file_break!(),
        include_str!("../../../qymcad-part/src/params.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/picks.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/popups.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/reopen.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/start.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/topology.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/lib.rs")
    ),
    file_break!(),
    include_str!("panels_bars.rs"),
    file_break!(),
    concat!(
        include_str!("panels_windows/dialogs.rs"),
        file_break!(),
        include_str!("panels_windows/library.rs"),
        file_break!(),
        include_str!("panels_windows/params.rs"),
        file_break!(),
        include_str!("panels_windows/faults.rs"),
        file_break!(),
        // the faults lead to their windows from the status line and from the window of faults
        include_str!("../../../qymcad-ui-state/src/faults.rs"),
        file_break!(),
        include_str!("panels_windows/schemes.rs"),
        file_break!(),
        include_str!("panels_windows/settings.rs"),
        file_break!(),
        include_str!("panels_windows/layout_settings.rs"),
        file_break!(),
        include_str!("panels_windows.rs")
    ),
    file_break!(),
    include_str!("start_screen.rs"),
    file_break!(),
    include_str!("command_search.rs"),
    file_break!(),
    include_str!("icon_themes/appearance_section.rs"),
    file_break!(),
    include_str!("icon_themes/packager_dialog.rs"),
    file_break!(),
    concat!(
        include_str!("file_doors.rs"),
        file_break!(),
        include_str!("gizmo_math.rs"),
        file_break!(),
        include_str!("look.rs"),
        file_break!(),
        include_str!("panel_bits.rs"),
        file_break!(),
        include_str!("startup.rs"),
        file_break!(),
        include_str!("view_anim.rs"),
        file_break!(),
        include_str!("../gui.rs"),
        file_break!(),
        include_str!("panel_places.rs"),
        file_break!(),
        include_str!("panels_bars/tool_bar.rs")
    ),
);

/// EVERYWHERE A CONTEXT IS USED: the three workbench crates and the panels of the application. `gui.rs` is
/// deliberately absent - there `self.viewing.mode_3d = true` is correct, and it is the contexts that are being checked.
#[cfg(test)]
const CTX_USERS: &str = concat!(
    concat!(
        concat!(
            include_str!("../../../qymcad-part/src/apply/copies.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/faces.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/solids.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply/update.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/apply.rs")
        ),
        file_break!(),
        concat!(
            include_str!("../../../qymcad-part/src/bars/feat_bar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/options.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/toolbar.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars/commands.rs"),
            file_break!(),
            include_str!("../../../qymcad-part/src/bars.rs")
        ),
        file_break!(),
        include_str!("../../../qymcad-part/src/params.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/picks.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/popups.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/reopen.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/start.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/topology.rs"),
        file_break!(),
        include_str!("../../../qymcad-part/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("../../../qymcad-sketch/src/clicks.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/constraints.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/corners.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/dims.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/drag.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/extend.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/menu.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/offset.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/popups.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/props.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/snap.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/ties.rs"),
        file_break!(),
        include_str!("../../../qymcad-sketch/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("../../../qymcad-assembly/src/bars.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/commands.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/gizmo.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/picks.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/popup.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/slots.rs"),
        file_break!(),
        include_str!("../../../qymcad-assembly/src/lib.rs")
    ),
    file_break!(),
    concat!(
        include_str!("panels_tree/build.rs"),
        file_break!(),
        include_str!("panels_tree/copy.rs"),
        file_break!(),
        include_str!("panels_tree/rename.rs"),
        file_break!(),
        include_str!("panels_tree/rows.rs"),
        file_break!(),
        include_str!("panels_tree.rs")
    ),
    file_break!(),
    include_str!("panels_props.rs"),
    file_break!(),
    include_str!("panels_bars.rs"),
    file_break!(),
    concat!(
        include_str!("panels_windows/dialogs.rs"),
        file_break!(),
        include_str!("panels_windows/library.rs"),
        file_break!(),
        include_str!("panels_windows/params.rs"),
        file_break!(),
        include_str!("panels_windows/schemes.rs"),
        file_break!(),
        include_str!("panels_windows/settings.rs"),
        file_break!(),
        include_str!("panels_windows/layout_settings.rs"),
        file_break!(),
        include_str!("panels_windows.rs")
    ),
);

/// The declarations of the contexts, for the two checks below.
#[cfg(test)]
const CTX_DECL: &str = concat!(
    include_str!("../../../qymcad-ui-state/src/contexts.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/corners.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/dims.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/edits.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/fields.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/fonts.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/geom.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/gizmos.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/handles.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/hotkeys.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/jobs.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/joints.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/navigation.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/painting.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/rebuild.rs"),
    file_break!(),
    concat!(
        include_str!("../../../qymcad-ui-state/src/records/commands.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records/frame.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records/gizmos.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records/pieces.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records/scene.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records/tools.rs"),
        file_break!(),
        include_str!("../../../qymcad-ui-state/src/records.rs")
    ),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/scene.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/section.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/selection.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/settings.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/sketch_diag.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/sketch_edit.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/text.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/tool_steps.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/tree.rs"),
    file_break!(),
    include_str!("../../../qymcad-ui-state/src/lib.rs")
);

#[cfg(test)]
mod tests {
    use qymcad_ui_state::WinKind;

    /// THE WORKING PART OF A GLUE KEEPS EVERY FILE: the code of the second file stands whole though the first one ends
    /// with its checks.
    #[test]
    fn the_working_part_of_a_glue_keeps_the_file_after_one_with_checks() {
        let glue = concat!("fn first() {}\n#[cfg(test)]\nmod tests {\n    fn quoted() {}\n}", super::file_break!(), "fn second() {}\n");
        let code = qymcad_i18n::ratchet::working_part(glue);
        assert!(code.contains("fn first()") && code.contains("fn second()"), "the working part lost a file of the glue: {code:?}");
        assert!(!code.contains("fn quoted()"), "the working part kept the checks of the first file: {code:?}");
    }

    /// THE MARK THE GLUES ARE WRITTEN WITH IS THE ONE THE WORKING PART IS TAKEN BY.
    #[test]
    fn the_glue_is_cut_by_the_mark_it_is_written_with() {
        assert_eq!(super::file_break!(), qymcad_i18n::ratchet::FILE_BREAK);
    }

    /// THE CHECKS OVER THE PANELS SEE THE LAST FILE OF THE PANELS, its working part whole.
    #[test]
    fn the_working_part_of_the_panels_holds_their_last_file() {
        let code = qymcad_i18n::ratchet::working_part(super::PANELS);
        let last = qymcad_i18n::ratchet::working_part(include_str!("icon_themes/packager_dialog.rs"));
        let line = last.lines().find(|l| l.trim_start().starts_with("pub") && l.contains("fn ")).expect("a function of the last file");
        assert!(code.contains(line), "the working part of the panels stops before their last file: {line:?} is not in it");
    }

    /// EVERY FILE THAT DRAWS IS READ BY SOMETHING, or is named here with a reason.
    ///
    /// Fifty-seven checks look for a button, a row or a string in the source of the panels. A guard that
    /// reads a file BY NAME guards that file and no other - that is [D17], and it cost four false greens in
    /// one day. The lists were then gathered into three places, which fixed the SPLITTING of a file but not
    /// the ADDING of one: a new window drawn in a new file is read by nothing, and nothing says so.
    ///
    /// So the coverage is checked rather than assumed. A file that draws and is in no list has to be named
    /// below with the reason, and the reason has to be a sentence rather than "not yet".
    #[test]
    fn every_file_that_draws_is_read_by_a_check() {
        // NOT PANELS, and each says why. A reason here is a decision; silence would be an oversight.
        //
        // KEYED BY PATH, not by file name. Every workbench crate is a single `lib.rs`, so an exception
        // written as "lib.rs" would have quietly excused the Part, the sketcher and the assembly along with
        // the one file it was meant for.
        const NOT_A_PANEL: [(&str, &str); 12] = [
            ("qymcad/src/gui/input.rs", "keyboard handling: it draws nothing, it only takes a frame's context to read keys from"),
            ("qymcad/src/gui/orbit_about.rs", "the centre of a turn of the view: it draws nothing, it only keeps the point of a turn in a frame's context memory"),
            ("qymcad-part/src/trial.rs", "the trial build of a command: it draws nothing, it only keeps its verdict in a frame's context memory"),
            ("qymcad/src/gui/expr_field.rs", "a widget drawn INTO a panel, not a panel: it is reached through the panels that place it"),
            ("qymcad-ui-state/src/voice.rs", "the one way a message is drawn, INTO a panel, a bar or a window: it is reached through what places it"),
            ("qymcad/src/gui/bar_menu.rs", "a widget drawn INTO the menu bar, not a panel: its captions and items are handed to it by `panels_bars.rs`, which the lists read"),
            (
                "qymcad-shell/src/lib.rs",
                "the shell: it opens the CONTAINERS and hands each to whoever fills it, so it draws places rather than content. \
                 Being absent from every panel list is the rule it exists to hold, not an oversight",
            ),
            ("qymcad-shell/src/draw.rs", "the shell drawn: the strips of each edge and the borders between their panels, containers again and no content"),
            ("qymcad-shell/src/bar.rs", "the plan of a bar of buttons: where each button lands, worked out from the room and the groups; it draws nothing"),
            ("qymcad-ui-state/src/icons/runtime.rs", "widgets for rendering icons into panels, reached through the panels that place them"),
            ("qymcad-ui-state/src/icons/manager.rs", "icon resolution and loader registration into egui::Context, not a panel"),
            ("qymcad-ui-state/src/icons/watcher.rs", "background directory watcher and polling thread, not a panel"),
        ];
        // EVERY LIST IN THE FILE, not the four panel ones. What is asserted below is "no source check reads
        // this file at all", so the four panel lists are the wrong measure of it: `gui.rs` is read by
        // `WINDOW_OPENERS` and `qymcad-ui-state` by `CTX_DECL`, and reporting them as unread would have been
        // the guard lying in the other direction. Membership rather than a count, so overlapping lists are
        // safe here - unlike the counting guards, where one list is passed for the reason in D17.
        let lists = [super::PANELS, super::WINDOWS, super::WINDOW_OPENERS, super::CTX_USERS, super::CTX_DECL, crate::gui::render_source::RENDER, crate::gui::sketch_source::SKETCH];
        // EVERY CRATE, not just the application's `gui`. The panels moved out into crates of their own, and
        // a walk rooted at one directory would from then on have reported full coverage of a shrinking part
        // of the interface - the same blindness recorded as D20 for the language guards. Found by widening:
        // `qymcad-shell` draws and no list read it.
        let crates = qymcad_i18n::ratchet::crates_root();
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut stack = qymcad_i18n::ratchet::every_crate_src();
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
        let mut unread = Vec::new();
        let mut looked = 0;
        for p in files {
            let name = p.strip_prefix(&crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            // WHAT THE PRODUCTION HALF OF THE FILE DOES; a test module that draws is not a panel.
            //
            // Cut at the test MODULE, not at the first `#[cfg(test)]`. That attribute also sits on single
            // functions in the middle of a file, and cutting there hid everything below it: `viewcube.rs`
            // drew from its first day and this check never saw it, because one test-only function stood
            // above the drawing. It became visible only when that function was moved out.
            //
            // Whatever its visibility: a test module opened to the checks next door (`pub(crate) mod tests`, so
            // that another check can reuse its helpers) is still a test module, and reading it as production
            // reported the door that opens files as an unread panel.
            let prod = ["#[cfg(test)]\nmod ", "#[cfg(test)]\npub(crate) mod ", "#[cfg(test)]\npub(super) mod "].iter().fold(text.as_str(), |t, cut| t.split(cut).next().unwrap_or(""));
            let draws = prod.contains("ui: &mut egui::Ui") || prod.contains("egui::Window::new") || prod.contains("ctx: &egui::Context");
            if !draws {
                continue;
            }
            looked += 1;
            if NOT_A_PANEL.iter().any(|(n, _)| *n == name) {
                continue;
            }
            // A file is read when a list carries a line only it has: its own module comment's first line.
            let first = prod.lines().find(|l| l.starts_with("//!")).unwrap_or("");
            if first.len() > 20 && !lists.iter().any(|l| l.contains(first)) {
                unread.push(name);
            }
        }
        assert!(looked > 5, "suspiciously few drawing files were found ({looked}) - the sweep looked at nothing");
        assert!(unread.is_empty(), "a file draws and no list reads it, so every source check is blind to it; put it in a list or name it in NOT_A_PANEL with a reason: {unread:?}");
    }

    /// EVERY WINDOW HAS A WAY IN. A window that nothing opens does not exist for a person, and nothing else
    /// notices: it compiles, it draws when the flag is set, and the flag is never set.
    ///
    /// This became possible only when the flags became a set: ten booleans could not be walked over, so the
    /// question "is any of them unreachable" had to be asked ten times by hand, and never was.
    #[test]
    fn every_window_can_be_opened_from_the_interface() {
        let mut lost = Vec::new();
        for k in WinKind::ALL {
            let name = format!("{k:?}");
            let opened = ["open", "toggle", "set"].iter().any(|verb| super::WINDOW_OPENERS.contains(&format!(".{verb}(WinKind::{name}")));
            if !opened {
                lost.push(name);
            }
        }
        assert!(lost.is_empty(), "windows that nothing opens: {lost:?}");
    }

    /// THE LIST OF ALL KINDS MATCHES THE DECLARATION. `ALL` is written by hand, so a variant added later is
    /// left out of every walk unless something counts them.
    #[test]
    fn the_list_of_window_kinds_is_complete() {
        let decl = concat!(
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
        let body = decl.split("pub enum WinKind {").nth(1).expect("the declaration of WinKind");
        let body = body.split("\n}").next().expect("the end of the declaration");
        let declared = body.lines().filter(|l| l.trim().ends_with(',') && !l.trim().starts_with("//")).count();
        assert_eq!(declared, WinKind::ALL.len(), "WinKind::ALL must list every variant");
    }

    /// EVERY FIELD OF A CONTEXT THAT IS HELD BY VALUE, as (context, field).
    ///
    /// Parsed rather than listed, so it follows the code.
    fn fields_held_by_value() -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut ctx: Option<String> = None;
        for line in super::CTX_DECL.lines() {
            if let Some(rest) = line.strip_prefix("pub struct ") {
                let name = rest.split('<').next().unwrap_or("").trim().to_string();
                ctx = name.ends_with("Ctx").then_some(name);
                continue;
            }
            if line == "}" {
                ctx = None;
                continue;
            }
            let Some(c) = ctx.as_ref() else { continue };
            let Some(rest) = line.strip_prefix("    pub ") else { continue };
            let Some((field, ty)) = rest.split_once(": ") else { continue };
            if !ty.starts_with('&') {
                out.push((c.clone(), field.to_string()));
            }
        }
        out.sort();
        out
    }

    /// A FIELD OF A CONTEXT HELD BY VALUE IS NEVER ASSIGNED TO.
    ///
    /// THE ONE CLASS OF TRANSFER ERROR THE COMPILER SAYS NOTHING ABOUT. A method becomes a free function, the
    /// field it wrote to arrives in the context BY VALUE, and `self.viewing.mode_3d = true` becomes `pc.mode_3d = true` -
    /// an assignment to a copy that dies with the call. The build is green, there is no warning, the type is
    /// right, `dead_code` is silent. Three tools of the Part went quiet at once that way, and it was found by
    /// two behaviour tests rather than by anything structural.
    ///
    /// Found live when this check was written: the properties panel wrote `pr.mode_3d = true` for "pick the face
    /// again", and the switch to the viewport never happened. `PropsCtx::mode_3d` is `&mut bool` now.
    #[test]
    fn nothing_assigns_to_a_context_field_held_by_value() {
        let mut caught = Vec::new();
        for (ctx, field) in fields_held_by_value() {
            let needle = format!(".{field} = ");
            for (n, line) in super::CTX_USERS.lines().enumerate() {
                let Some(at) = line.find(&needle) else { continue };
                let head = &line[..at];
                let name_start = head.len() - head.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').count();
                // `*pc.mode_3d = ...` writes THROUGH a reference and is correct; the bare form is the defect.
                if name_start > 0 && head.as_bytes()[name_start - 1] == b'*' {
                    continue;
                }
                caught.push(format!("{ctx}::{field} at line {n}: {}", line.trim()));
            }
        }
        assert!(caught.is_empty(), "assignment to a context field held by value (it writes to a copy):\n{}", caught.join("\n"));
    }

    /// THE LIST OF FIELDS HELD BY VALUE IS THE ONE THAT WAS LOOKED AT.
    ///
    /// On equality, so that a field added by value forces a look and a field that quietly stops being one is
    /// noticed too. By value is right for a fact the panel only READS - which workbench it is, whether the view
    /// is 3D while drawing. It is wrong for anything the panel changes, and the difference is invisible to the
    /// compiler, so it is written down here instead.
    #[test]
    fn the_context_fields_held_by_value_are_the_reviewed_ones() {
        let seen: Vec<String> = fields_held_by_value().into_iter().map(|(c, f)| format!("{c}::{f}")).collect();
        let reviewed = [
            "JointCtx::mode_3d",         // read: the glyphs are drawn only in 3D
            "JointCtx::workbench",       // read: which workbench is current
            "PartCtx::workbench",        // read
            "PropsCtx::workbench",       // read
            "StatusCtx::cursor",         // read: where the pointer is, printed as two numbers and changed by nobody
            "StatusCtx::line",           // a pair of references: what the line says is written through them, not into a copy
            "TextCtx::tool_text_height", // read: the height the preview of the text tool is baked at
            "TextCtx::writes_note",      // read: a note asks its letters of the interface's font
            "TreeCtx::workbench",        // read
            "WinCtx::file_ask_open",     // read: a window does not open a second file dialogue over the first
            "WinCtx::workbench",         // read: the command search puts its own commands first
        ];
        assert_eq!(seen, reviewed, "the fields held by value have changed - look at each before changing this list");
    }
}
