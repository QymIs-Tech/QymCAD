//! HARD-CODED NUMBERS BECAME SETTINGS — AND ARE APPLIED.
//!
//! Four constants answered questions whose answer differs for everybody: how often to write an
//! autosave, how many undo steps to keep, how transparent the ghosts are, and how wide the perspective
//! looks. The price of "one value for all" differs too — from an extra pause on a heavy assembly to an
//! invisible preview — and it is not ours to charge on somebody's behalf.
//!
//! The tests here check not "the field was stored" but WHETHER IT IS APPLIED. A setting that was
//! recorded and never applied is the worst kind of working code: it looks like it works and does
//! nothing.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// NOT ONE OF THESE NUMBERS IS STILL A CONSTANT IN THE CODE.
    #[test]
    fn the_tuned_numbers_are_no_longer_constants() {
        let gui = include_str!("../gui.rs");
        let input = include_str!("input.rs");
        for (name, src, where_) in [("UNDO_CAP", gui, "gui.rs"), ("GHOST_ALPHA", gui, "gui.rs"), ("PERSP_FOV_HALF_TAN", gui, "gui.rs"), ("PERIOD", input, "input.rs")] {
            let code: String = super::super::super::i18n::ratchet::tests::working_part(src).lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
            assert!(!code.contains(&format!("const {name}")), "{where_}: `{name}` is a constant again — the setting stopped affecting anything");
        }
    }

    /// THE ANTIALIASING IS TAKEN FROM THE SETTING AT STARTUP — and the window says so.
    ///
    /// The only setting here that is NOT applied on the fly: the number of samples is baked into the
    /// wgpu pipelines when the renderer is created. The decision was to keep it with an honest note
    /// rather than hide it. The guard watches two things: the value reaches the renderer, and the note
    /// stands next to it.
    #[test]
    fn the_antialiasing_is_taken_at_start_and_says_so() {
        let ok = crate::viewport_gpu::supported_msaa();
        assert!(ok.contains(&1) && ok.contains(&4), "the specification guarantees 1 and 4 — those must always be offered: {ok:?}");

        // WHAT IS SUPPORTED IS TAKEN AS IT IS
        for n in &ok {
            crate::viewport_gpu::set_msaa(*n);
            assert_eq!(crate::viewport_gpu::msaa_samples_for_test(), *n, "the renderer did not take the supported antialiasing {n}");
        }

        // AND WHAT IS UNSUPPORTED DOES NOT CRASH THE PROGRAM but drops to the nearest one it can do.
        //
        // That is the price of getting it wrong: 8x was set, the device cannot do it, and wgpu crashed
        // the program RIGHT AT STARTUP — the only way back was editing the config by hand. A setting
        // has no right to make the program unstartable.
        for n in [3u32, 8, 16, 64, 0] {
            crate::viewport_gpu::set_msaa(n);
            let got = crate::viewport_gpu::msaa_samples_for_test();
            assert!(ok.contains(&got), "a request for {n}x gave an unsupported {got} — the wgpu pipelines crash on that");
            assert!(got <= n.max(1), "a request for {n}x gave a LARGER {got} — it must drop down, not up");
        }
        crate::viewport_gpu::set_msaa(4);

        let gui = include_str!("../gui.rs");
        assert!(gui.contains("crate::viewport_gpu::set_msaa(app.set.msaa);"), "the antialiasing setting does not reach the renderer at startup");
        let panels = crate::gui::panels_source::PANELS;
        assert!(panels.contains("settings-msaa-restart"), "the \"applies on the next start\" note is gone — the setting started lying");
        assert!(panels.contains("crate::viewport_gpu::supported_msaa()"), "the window offers a list of its own instead of the list of the device again — that is exactly how the program was crashed at startup");
        let gpu = include_str!("../viewport_gpu.rs");
        assert!(gpu.contains("probe_supported(&render_state.device"), "the device is no longer asked before the pipelines are built");
    }

    /// THE AUTOSAVE OBEYS THE PERIOD THAT WAS SET, and zero switches it off.
    ///
    /// The first edition put the clock an hour back and therefore passed with ANY threshold — it proved
    /// only that zero switches it off. Here TWO periods are taken on either side of the elapsed time:
    /// the short one must fire and the long one must stay quiet. Otherwise the value can be ignored.
    #[test]
    fn the_autosave_period_is_obeyed_and_zero_turns_it_off() {
        let dir = std::env::temp_dir().join("qym_tuned_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("auto.qcad").to_string_lossy().into_owned();
        let auto = std::path::Path::new(&path).with_extension("").to_string_lossy().into_owned() + ".autosave.qcad";
        let _ = std::fs::remove_file(&auto);

        // a document with unsaved work and a clock that has not ticked for a while
        let mut app = super::super::screen_keys::tests::plate();
        app.set_project_path(path.clone());
        app.set_last_autosave_ago_for_test(90); // a minute and a half has passed since the last write

        app.set.autosave_secs = 0; // switched off: nothing is written however long has passed
        app.maybe_autosave_for_test();
        app.wait_bg_for_test();
        assert!(!std::path::Path::new(&auto).exists(), "the autosave is switched off and a copy appeared anyway");

        app.set.autosave_secs = 600; // the period is NOT UP yet (90 s of 600) — too early to write
        app.maybe_autosave_for_test();
        app.wait_bg_for_test();
        assert!(!std::path::Path::new(&auto).exists(), "the period is 600 s and 90 have passed — and a copy is already written: the value of the period is not read");

        app.set.autosave_secs = 60; // and now the period is up (90 s of 60)
        app.set_last_autosave_ago_for_test(90);
        app.maybe_autosave_for_test();
        app.wait_bg_for_test();
        assert!(std::path::Path::new(&auto).exists(), "the autosave period is up and there is no copy");

        let _ = std::fs::remove_file(&auto);
        let _ = std::fs::remove_file(&path);
    }

    /// THE UNDO DEPTH IS A NUMBER FROM THE SETTINGS rather than forty forever.
    #[test]
    fn the_undo_depth_follows_the_setting() {
        let mut app = App::default();
        app.set.undo_cap = 3;
        for i in 0..10 {
            app.begin_edit(&format!("step {i}"));
            app.project.parameters.push(qymcad_core::model::Param { name: format!("p{i}"), expr: "1".into(), value: 1.0 });
            app.commit_edit();
        }
        assert!(app.undo_len_for_test() <= 3, "{} undo steps with a limit of 3 — the setting has no effect", app.undo_len_for_test());
        assert!(app.undo_len_for_test() > 0, "the limit ate the WHOLE history — there will be nothing left to undo");
    }

    /// THE FIELD OF VIEW CHANGES THE PROJECTION rather than only a number in a window.
    #[test]
    fn the_field_of_view_changes_the_projection() {
        let mut app = App::default();
        app.set.cam_perspective = true; // an orthographic projection has no field of view by definition
        app.set.persp_fov_deg = 20.0;
        let narrow = app.persp_inv_d_for_test(400.0);
        app.set.persp_fov_deg = 80.0;
        let wide = app.persp_inv_d_for_test(400.0);
        assert!(wide > narrow, "a wide angle must give a stronger perspective: {wide} against {narrow}");
    }

    /// THE TRANSPARENCY OF A GHOST REACHES THE COLOUR.
    #[test]
    fn the_ghost_transparency_reaches_the_colour() {
        let pal = crate::palette::dark();
        let base = [160, 160, 160];
        let n = [0.0, 0.0, 1.0];
        let light = [0.0, 0.0, 1.0];
        let faint = App::shade_tri_for_test(&pal, 40, false, true, base, n, light).a();
        let solid = App::shade_tri_for_test(&pal, 240, false, true, base, n, light).a();
        assert_eq!(faint, 40, "the ghost did not take the transparency from the setting: {faint}");
        assert_eq!(solid, 240, "the ghost did not take the transparency from the setting: {solid}");
    }
}
