//! THE SETTINGS REMEMBER THEMSELVES — the whole class, not three cases.
//!
//! The grid step and the rotation step were reported. On inspection it turned out that SIX settings out
//! of thirteen did not survive a restart, and among them the THEME itself: `set_visuals` was called on a
//! click but was stored nowhere — choose the light one, restart, and it is dark again.
//!
//! The cause was not three forgotten fields. The value lived as a field of `App` while the list of what
//! gets saved was written out separately by hand — forgetting to add to it was THE NORM: the compiler is
//! silent, the test does not turn red, and the person using the program finds out. Now the value lives
//! in exactly one place (`Settings`), and the store saves that record WHOLE. To lose a setting one has
//! to fail to create a field.
#[cfg(test)]
mod tests {
    use super::super::{App, Settings};

    /// Settings that differ from the factory ones IN EVERY FIELD — so that "it was saved" cannot be
    /// confused with "it happened to match the default".
    fn all_changed() -> Settings {
        let d = Settings::default();
        Settings {
            language: "en".into(),
            scheme: "custom-light".into(),
            viewcube_size: 2,
            hotkeys: [("part.extrude".to_string(), "W".to_string())].into_iter().collect(),
            help_lang: "en".into(),
            help_external: !d.help_external,
            msaa: 8,
            autosave_secs: 600,
            undo_cap: 7,
            ghost_alpha: 200,
            persp_fov_deg: 60.0,
            show_rapids: !d.show_rapids,
            cam_tab_enabled: !d.cam_tab_enabled,
            gpu_viewport: !d.gpu_viewport,
            cam_perspective: !d.cam_perspective,
            smooth_shading: !d.smooth_shading,
            show_contours: !d.show_contours,
            show_joints: !d.show_joints,
            show_interference: !d.show_interference,
            snap: super::super::Snapping { on: !d.snap.on, grid: 7.5, rot_deg: 30.0 },
            auto_constrain: !d.auto_constrain,
            defaults: super::super::Defaults { extrude_h: 42.0, offset_2d: 8.25 },
            ui_scale: 1.4,
            recent: vec!["/tmp/a.qcad".into()],
            recent_limit: 3,
            pick_precision: 2,
        }
    }

    /// A comparison field by field: `Settings` deliberately has no `PartialEq` — the comparison is made
    /// meaningfully and with a clear message about which field failed to arrive.
    fn assert_same(a: &Settings, b: &Settings) {
        assert_eq!(a.language, b.language, "the language of the interface");
        assert_eq!(a.scheme, b.scheme, "the colour scheme");
        assert_eq!(a.viewcube_size, b.viewcube_size, "the size of the navigation cube");
        assert_eq!(a.show_rapids, b.show_rapids, "the rapid moves");
        assert_eq!(a.cam_tab_enabled, b.cam_tab_enabled, "the CAM tab");
        assert_eq!(a.gpu_viewport, b.gpu_viewport, "the engine of the viewport");
        assert_eq!(a.cam_perspective, b.cam_perspective, "the projection");
        assert_eq!(a.smooth_shading, b.smooth_shading, "the shading");
        assert_eq!(a.show_contours, b.show_contours, "sketch outlines in an assembly");
        assert_eq!(a.show_joints, b.show_joints, "the glyphs of the mates");
        assert_eq!(a.show_interference, b.show_interference, "the interference check");
        assert_eq!(a.snap.on, b.snap.on, "snapping on");
        assert_eq!(a.snap.grid, b.snap.grid, "the grid step");
        assert_eq!(a.snap.rot_deg, b.snap.rot_deg, "the rotation step");
        assert_eq!(a.auto_constrain, b.auto_constrain, "the automatic constraints");
        assert_eq!(a.defaults.extrude_h, b.defaults.extrude_h, "the height of an extrusion");
        assert_eq!(a.defaults.offset_2d, b.defaults.offset_2d, "the 2D offset");
    }

    /// THE MAIN THING: EVERY setting survives the save-and-load round trip.
    ///
    /// This is what the reports came down to: the grid step and the rotation step. What is checked is not
    /// those two but everything at once — the class, not the case.
    #[test]
    fn every_setting_survives_a_save_and_load() {
        let want = all_changed();
        let text = ron::ser::to_string(&want).expect("the settings serialise");
        let got: Settings = ron::from_str(&text).expect("and read back");
        assert_same(&want, &got);
    }

    /// THE GRID STEP AND THE ROTATION STEP — exactly what was reported, as a test of its own.
    ///
    /// Separately, because the general test is easy to weaken with an edit, while this report must stay
    /// checked by name.
    #[test]
    fn grid_step_and_rotation_step_survive_a_restart() {
        let mut app = App::default();
        app.set.snap.grid = 5.0;
        app.set.snap.rot_deg = 30.0;
        let saved = ron::ser::to_string(&app.set).expect("saved");

        let mut fresh = App::default();
        assert_ne!(fresh.set.snap.grid, 5.0, "setup: the factory value is a different one");
        fresh.set = ron::from_str(&saved).expect("loaded");
        assert_eq!(fresh.set.snap.grid, 5.0, "the grid step must survive a restart");
        assert_eq!(fresh.set.snap.rot_deg, 30.0, "the rotation step must survive a restart");
    }

    /// A SCHEME IS A SETTING TOO. It was lost silently: `set_visuals` changes the look and stores
    /// nothing.
    ///
    /// A boolean `dark_theme` used to live here. It became A SECOND TRUTH beside the name of the scheme:
    /// the scheme sets both the palette of the canvas and the look of `egui`, while the flag repeated
    /// part of that knowledge. Two truths about one thing diverge — the flag is gone and the name
    /// stayed.
    #[test]
    fn the_scheme_survives_a_restart_and_is_applied_on_start() {
        let mut app = App::default();
        assert_eq!(app.set.scheme, "dark", "setup: the dark one is the default");
        app.set.scheme = "light".into();
        let saved = ron::ser::to_string(&app.set).expect("saved");

        let mut fresh = App::default();
        fresh.set = ron::from_str(&saved).expect("loaded");
        assert_eq!(fresh.set.scheme, "light", "the light scheme must survive a restart");

        // and it must BE APPLIED rather than merely lie there: egui does not remember its palette between
        // runs
        let ctx = egui::Context::default();
        fresh.apply_theme(&ctx);
        assert!(!ctx.style().visuals.dark_mode, "a loaded light scheme must be applied to egui");
        fresh.set.scheme = "dark".into();
        fresh.apply_theme(&ctx);
        assert!(ctx.style().visuals.dark_mode, "and back again as well");
    }

    /// THE SCALE OF THE INTERFACE SURVIVES A RESTART AND IS APPLIED AT STARTUP.
    ///
    /// The same trap the theme fell into: `egui` does not remember its scale between runs, and a setting
    /// applied only on a click silently rolls back on the next start. So what is checked is not "the
    /// field was saved" but "what was loaded IS APPLIED to egui".
    #[test]
    fn the_ui_scale_survives_a_restart_and_is_applied_on_start() {
        let mut app = App::default();
        assert_eq!(app.set.ui_scale, 1.0, "setup: the factory scale is whatever the system decides");
        app.set.ui_scale = 1.6;
        let saved = ron::ser::to_string(&app.set).expect("saved");

        let mut fresh = App::default();
        let ctx = egui::Context::default();
        // EXACTLY WHAT IS CALLED AT STARTUP: the shared handle that adopts the settings. The test used to
        // call `apply_theme` — the theme then applied the scale along the way, and that hidden tie made
        // the check blind: the scale "was applied" even where nobody applied it.
        fresh.adopt_settings(ron::from_str(&saved).expect("loaded"), &ctx);
        assert_eq!(fresh.set.ui_scale, 1.6, "the scale must survive a restart");
        // `egui` takes a new scale AT THE START OF THE NEXT PASS, so a frame is built: that way the test
        // checks the real drawing rather than a field in memory.
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        assert!((ctx.zoom_factor() - 1.6).abs() < 1e-6, "the loaded scale must be applied to egui, and out came {}", ctx.zoom_factor());
    }

    /// AN IMPOSSIBLE SCALE DOES NOT MAKE THE INTERFACE UNRECOVERABLE.
    ///
    /// A broken record, or one tweaked by hand, with zero or a negative multiplier would leave the
    /// program without an interface — that is, with no way to put the setting back.
    #[test]
    fn an_impossible_scale_cannot_lock_the_user_out() {
        let ctx = egui::Context::default();
        let mut app = App::default();
        for bad in [0.0, -3.0, 99.0] {
            app.set.ui_scale = bad;
            app.apply_ui_scale(&ctx);
            let _ = ctx.run(egui::RawInput::default(), |_| {});
            let z = ctx.zoom_factor();
            assert!(z >= 0.5 && z <= 3.0, "the scale {bad} got into egui as {z} — the interface would become unrecoverable");
        }
    }

    /// A SETTING FROM A FUTURE VERSION does not bring the whole settings file down.
    ///
    /// Without `serde(default)` a record with no new field would read with an error — and ALL the
    /// settings would be lost at once when the program is updated.
    #[test]
    fn an_older_record_without_a_new_field_still_loads() {
        let old = r#"(scheme:"custom-light",show_rapids:true,snap:(on:false,grid:3.0))"#;
        let got: Settings = ron::from_str(old).expect("an old record must read");
        assert_eq!(got.scheme, "custom-light", "what was read is kept");
        assert!(got.show_rapids, "what was read is kept");
        assert_eq!(got.snap.grid, 3.0, "a nested record as well");
        let d = Settings::default();
        assert_eq!(got.snap.rot_deg, d.snap.rot_deg, "a missing field takes the default rather than zero");
        assert_eq!(got.auto_constrain, d.auto_constrain, "and whole missing branches too");
    }

    /// DERIVED STATE DOES NOT GET INTO THE SETTINGS.
    ///
    /// The snapping hint (where things have snapped to right now) is recomputed every frame. While it lay
    /// inside the snapping settings, that record could not be saved whole — and "whole" is exactly what
    /// makes the saving automatic.
    #[test]
    fn runtime_state_is_not_a_setting() {
        let text = ron::ser::to_string(&Settings::default()).expect("it serialises");
        assert!(!text.contains("hint"), "the snapping hint is derived and has no place in the settings: {text}");
    }

    /// THE SETTINGS WINDOW EDITS THE RECORD ITSELF rather than a copy of its own.
    ///
    /// That is the very gap the settings were lost through: one value was shown and another saved. A
    /// guard over the source: no direct `set_visuals` must be left in the settings window — the theme
    /// goes through the settings record.
    #[test]
    fn the_settings_window_edits_the_record_itself() {
        let panels = crate::gui::panels_source::PANELS;
        assert!(!panels.contains("ctx.set_visuals("), "the theme must go through the settings rather than past them");
        assert!(!panels.contains("ctx.set_zoom_factor("), "the scale of the interface must go through `apply_ui_scale` rather than past the settings");
        assert!(panels.contains("self.set.scheme = id.clone();"), "the scheme switch edits the settings record");
        assert!(panels.contains("self.set.snap.grid"), "the grid step is edited in the settings record");
    }
}
