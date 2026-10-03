//! THE DEVELOPER'S TWO SWITCHES, AND EACH ONE HIDES WHAT IT NAMES.
//!
//! Two things about the program are for whoever works on it rather than for whoever uses it: the list of
//! every face, edge and corner of every body, and the file every left-button click is written down into.
//! Both are off until a person puts the tick on in the developer's part of the settings - and off in a
//! check that does not say otherwise, so a check that reads the tree or the trail without asking first is
//! reading an empty panel and an absent file, and would pass on either.
//!
//! Checked here, because both fail quietly rather than loudly. A list that stops drawing costs nothing and
//! is noticed only by the person who came to read it; a trail that stops being written looks exactly like a
//! fault that was never reproduced.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A FOLDER FOR THE TRAIL OF THIS CHECK, so that a run of the checks leaves nothing in the person's
    /// own settings and does not read whatever a run of the program left there.
    fn trace_folder(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("qymcad-dev-switch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder of its own for the trail");
        dir
    }

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// Draw the tree once and report how many rows of the model's elements it left behind.
    fn list_rows(app: &mut App) -> usize {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
            egui::CentralPanel::default().show(c, |ui| app.build_tree_for_test(ui));
        });
        app.tree.debug_row_rects.len()
    }

    /// A document with a body in it, so that the tree has something to name.
    fn app_with_a_body() -> App {
        let mut app = App::default();
        app.project.new_document();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        app
    }

    /// THE ELEMENTS ARE NOT IN THE TREE AT ALL WHILE THE TICK IS OFF, AND ARE IN IT WHILE IT IS ON.
    ///
    /// Both halves are needed and neither carries the other: a list drawn empty passes the first, and a
    /// tick that hides nothing passes the second. The document is the same in both frames, so what differs
    /// between them is the setting and nothing else.
    #[test]
    fn the_elements_are_absent_from_the_tree_until_the_tick_is_put_on() {
        let mut app = app_with_a_body();
        app.set.show_model_elements = false;

        let off = list_rows(&mut app);
        assert_eq!(off, 0, "with the tick off the tree still drew {off} rows of the model's elements");

        app.set.show_model_elements = true;
        let on = list_rows(&mut app);
        assert!(on > 0, "with the tick on the tree named nothing at all: the switch hides the list, it does not empty it");
    }

    /// THE FACTORY LEAVES BOTH TICKS OFF, and a person who never asked is not helped by either.
    #[test]
    fn the_factory_has_both_of_them_off() {
        let d = qymcad_ui_state::Settings::default();
        assert!(!d.save_clicks, "the trail is written before anyone has asked for it");
        assert!(!d.show_model_elements, "the model's elements are listed before anyone has asked to read them");
    }

    /// A CLICK LEAVES A TRAIL ONLY WHILE THE TICK IS ON, AND TAKING IT OFF STOPS THE WRITING AT ONCE.
    ///
    /// Measured on the SIZE of the file rather than on whether there is one: a trail left by an earlier run
    /// is a file this run did not write, and asking whether one exists would say nothing about the tick. The
    /// writing is read by its length because that is what grows when a line is added and what stops when the
    /// tick is taken off.
    #[test]
    fn the_clicks_are_written_only_while_the_tick_is_on() {
        let dir = trace_folder("clicks");
        qymcad_trace::write_into(Some(&dir));
        let path = qymcad_trace::path().expect("the trail has a place");
        let size = || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        qymcad_trace::set_recording(false);
        qymcad_trace::trace!("a click while the tick is off");
        qymcad_trace::trace!("a second click, still with the tick off");
        assert_eq!(size(), 0, "the trail was written with the tick off, at {}", path.display());

        qymcad_trace::set_recording(true);
        qymcad_trace::trace!("a click while the tick is on");
        let written = size();
        assert!(written > 0, "the tick is on and nothing was written at {}", path.display());
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("clicks.log"), "the trail is not the file that is looked for: {}", path.display());

        qymcad_trace::set_recording(false);
        qymcad_trace::trace!("a click after the tick was taken off");
        assert_eq!(size(), written, "the trail kept growing after the tick was taken off: {written} → {} bytes", size());

        qymcad_trace::set_recording(true); // back to the factory value, so the checks beside are not left silent
        qymcad_trace::write_into(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE TWO ARE A SECTION OF THE SETTINGS' OWN, and that section STANDS BELOW THE LAYOUT in the list on
    /// the left - not as a group of words inside the layout, which is about where panels stand.
    ///
    /// Reported: the switches were found among the rows of another section, so there was nothing to open,
    /// nothing to search for as a section and nothing of their own to restore. The section is checked here
    /// by where it stands in the order and by what the window draws when it is the one on show, because a
    /// section of its own in the table and a section of its own on the screen are two things.
    #[test]
    fn the_two_switches_have_a_section_of_their_own_below_the_layout() {
        use qymcad_ui_state::settings_sections::SettingsSection as Sec;
        let all = Sec::all();
        let at = |s: Sec| all.iter().position(|q| *q == s).unwrap_or_else(|| panic!("the window's sections do not hold {s:?} at all"));
        assert_eq!(at(Sec::Developer), at(Sec::Layout) + 1, "the developer's section does not stand directly below the layout: {all:?}");
        assert!(
            Sec::Developer.row_keys().contains(&"settings-save-clicks") && Sec::Developer.row_keys().contains(&"settings-show-model-elements"),
            "the section does not hold both switches: {:?}",
            Sec::Developer.row_keys()
        );

        // AND THE WINDOW DRAWS IT: both switches and the name of the section, in the section's own body.
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        let mut app = App::default();
        app.win.open(crate::gui::WinKind::Settings);
        app.scheme.section = Sec::Developer;
        let words = super::super::screen_keys::tests::frame_text(&mut app, |a, c| {
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), c);
            a.do_win_asks(asks, c);
        });
        crate::i18n::set_language(&prev);
        for k in ["settings-sec-developer", "settings-save-clicks", "settings-show-model-elements"] {
            assert!(words.iter().any(|w| w == &crate::i18n::tr(k)), "the section on the screen says nothing for '{k}': {words:?}");
        }
    }

    /// THE ROW UNDER THE TICK NAMES THE FILE THE CLICKS ARE WRITTEN INTO, as it stands on this machine.
    ///
    /// A tick that writes a file nobody can find is a tick that cannot be read, and the person who turns it
    /// on is about to go and read it. The words are checked against the path the trace itself hands out, so
    /// a label that names some other folder - or the program rather than the settings - does not pass.
    #[test]
    fn the_row_under_the_tick_names_where_the_clicks_are_written() {
        let Some(path) = qymcad_trace::path() else {
            eprintln!("PASSED OVER: this machine has no settings folder to name in the settings");
            return;
        };
        let mut app = App::default();
        app.win.open(crate::gui::WinKind::Settings);
        app.scheme.section = qymcad_ui_state::settings_sections::SettingsSection::Developer;
        let words = super::super::screen_keys::tests::frame_text(&mut app, |a, c| {
            let mut asks = Vec::new();
            crate::gui::panels_windows::settings_window(&mut a.win_ctx(&mut asks), c);
            a.do_win_asks(asks, c);
        });
        let want = path.display().to_string();
        assert!(
            words.iter().any(|w| w.contains(&want)),
            "nothing on the screen names the file the clicks go into ({want}); on screen: {words:?}"
        );
    }

    /// THE TRAIL IS WRITTEN INTO THE FOLDER OF THE SETTINGS, and not beside the program.
    ///
    /// Beside the program is where it used to go, and for a packaged program that place does not survive
    /// the run: the image unpacks its executable into a temporary mount that is unmounted when the window
    /// closes, so a trail written there is gone by the time anybody goes to read it. The settings folder is
    /// the one place on the machine that is certainly still there, and it is where the record of what the
    /// program was told already lives.
    #[test]
    fn the_trail_belongs_to_the_settings_folder() {
        let Some(settings) = qymcad_paths::data_root() else {
            eprintln!("PASSED OVER: this machine has no settings folder to put the trail in");
            return;
        };
        let path = qymcad_trace::path().expect("the trail has a place");
        assert_eq!(path.parent(), Some(settings.as_path()), "the trail is at {} rather than in the settings folder {}", path.display(), settings.display());
        assert!(path.starts_with(&settings), "the trail at {} is outside the settings folder {}", path.display(), settings.display());
    }
}