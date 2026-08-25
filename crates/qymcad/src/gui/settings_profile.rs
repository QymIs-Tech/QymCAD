//! A SETTINGS PROFILE: carry it to another machine, share it, attach it to a report.
//!
//! What matters here is not the writing to a file but that the settings come into force by ONE path.
//! The theme, the language and the interface scale live not only in the record but also in the state
//! of `egui`, which is not remembered between runs. While that was done by a list of calls at
//! startup, the second path (import) would have had to repeat the same list — and would have drifted
//! from it at the very first new setting, and silently at that: the record is right and the screen
//! shows the old thing.
#[cfg(test)]
mod tests {
    use super::super::{App, Settings};

    fn tmp(name: &str) -> String {
        let dir = std::env::temp_dir().join("qym_settings_profile_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let p = dir.join(name).to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&p);
        p
    }

    /// THE POINT: a profile carries the settings whole and they TAKE EFFECT rather than merely
    /// lying in the record.
    #[test]
    fn a_profile_carries_the_settings_and_they_take_effect() {
        let ctx = egui::Context::default();
        let path = tmp("profile.ron");

        let mut author = App::default();
        author.set.language = "en".into();
        author.set.ui_scale = 1.5;
        author.set.undo_cap = 7;
        author.set.persp_fov_deg = 55.0;
        author.set.hotkeys.insert("part.extrude".into(), "W".into());
        author.export_settings_to(&path).expect("the profile writes");

        let mut other = App::default();
        other.import_settings_from(&path, &ctx).expect("the profile reads");
        assert_eq!(other.set.language, "en", "the language did not carry over");
        assert_eq!(other.set.undo_cap, 7, "the undo depth did not carry over");
        assert_eq!(other.hotkey_key("part.extrude"), "W", "the rebound key did not carry over");
        // AND IT IS APPLIED, not merely recorded: the scale lives in the egui state rather than in
        // the record. A frame is run through — `set_zoom_factor` reaches the state on the next pass.
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        assert!((ctx.zoom_factor() - 1.5).abs() < 1e-6, "the interface scale was not applied: {}", ctx.zoom_factor());
        let _ = std::fs::remove_file(&path);
    }

    /// A BROKEN FILE LEAVES THE CURRENT SETTINGS ALONE. The change arrives whole or does not arrive
    /// at all: "half a profile" is a state there is nothing to explain to a person with.
    #[test]
    fn a_broken_file_leaves_the_current_settings_alone() {
        let ctx = egui::Context::default();
        let path = tmp("broken.ron");
        std::fs::write(&path, "this is not settings").expect("the file writes");

        let mut app = App::default();
        app.set.undo_cap = 11;
        let err = app.import_settings_from(&path, &ctx).expect_err("a broken file must be rejected");
        assert!(!err.is_empty(), "a refusal must explain the reason");
        assert_eq!(app.set.undo_cap, 11, "a broken file overwrote the current settings");
        let _ = std::fs::remove_file(&path);
    }

    /// A PROFILE FROM ANOTHER VERSION STILL LOADS: what is missing takes the factory value.
    ///
    /// Otherwise a profile cannot be shared: a colleague's version is one setting older — and the
    /// whole file is rejected.
    #[test]
    fn a_profile_from_another_version_still_loads() {
        let ctx = egui::Context::default();
        let path = tmp("older.ron");
        let full = ron::ser::to_string_pretty(&Settings::default(), ron::ser::PrettyConfig::default()).expect("it writes");
        // drop a line — as if that setting had not existed in that version yet
        let older: String = full.lines().filter(|l| !l.contains("undo_cap")).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, older).expect("the file writes");

        let mut app = App::default();
        app.set.undo_cap = 99;
        app.import_settings_from(&path, &ctx).expect("a profile missing one field must read");
        assert_eq!(app.set.undo_cap, Settings::default().undo_cap, "a missing field must take the factory value");
        let _ = std::fs::remove_file(&path);
    }

    /// BOTH STARTUP AND IMPORT GO THROUGH ONE HANDLE. Should they diverge, a new setting would be
    /// applied at startup and not on import (or the other way round), and the only way to see it would
    /// be by eye.
    #[test]
    fn startup_and_import_use_the_same_door() {
        let gui = include_str!("../gui.rs");
        assert!(gui.contains("app.adopt_settings(v, &cc.egui_ctx)"), "startup stopped adopting the settings through the common handle");
        assert!(gui.contains("self.adopt_settings(s, ctx)"), "import stopped adopting the settings through the common handle");
        // and nothing assigns the record past it
        let code: String = super::super::super::i18n::ratchet::tests::working_part(gui).lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        let direct = code.matches("self.set = ").count() + code.matches("app.set = ").count();
        assert_eq!(direct, 1, "the settings record is assigned past `adopt_settings` ({direct} places instead of one — the handle itself)");
    }
}
