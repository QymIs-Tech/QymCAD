//! THE SETTINGS WINDOW WITH SECTIONS: the search finds, and a reset returns THAT section alone.
//!
//! The former window was a flat scroll and there was nothing to check in it. With sections two things
//! appear that break silently: a setting that got into the window past the search (nobody will find it
//! and will decide it does not exist), and a reset that touches more than it should (press "reset" in
//! the Sketch section and lose the language).
#[cfg(test)]
mod tests {
    use super::super::settings_sections::SettingsSection as Sec;
    use super::super::{App, Settings};
    use crate::i18n;

    /// THE SECTIONS AND THEIR ROWS ARE IN THE CATALOGUE IN BOTH LANGUAGES.
    #[test]
    fn every_section_and_row_has_words_in_every_language() {
        let prev = i18n::language();
        let mut holes = Vec::new();
        for (code, _) in i18n::available() {
            i18n::set_language(&code);
            for sec in Sec::all() {
                for k in std::iter::once(&sec.key()).chain(sec.row_keys().iter()) {
                    let t = i18n::tr(k);
                    if &t == k || t.trim().is_empty() {
                        holes.push(format!("{code}: {k}"));
                    }
                }
            }
        }
        i18n::set_language(&prev);
        assert!(holes.is_empty(), "a settings section or row would show up as a key ({}):\n{}", holes.len(), holes.join("\n"));
    }

    /// THE SEARCH FINDS A SETTING BY ITS LABEL — IN BOTH LANGUAGES.
    ///
    /// Exactly what the search was started for: a person looks for the word they see, not for the key
    /// and not for the section it was put into.
    #[test]
    fn the_search_finds_a_setting_by_its_label_in_both_languages() {
        let prev = i18n::language();
        for code in ["ru", "en"] {
            i18n::set_language(code);
            for sec in Sec::all() {
                for k in sec.row_keys() {
                    let label = i18n::tr(k);
                    // the search goes by the FIRST WORD of the label — that is how people type
                    let word = label.split_whitespace().next().unwrap_or(&label).to_string();
                    assert!(Sec::row_matches(k, &word), "{code}: the row \"{label}\" is not found by the word \"{word}\"");
                    assert!(sec.has_match(&word), "{code}: the section {:?} must show up when searching for \"{word}\"", sec);
                }
            }
            // rubbish finds nothing
            assert!(!Sec::all().iter().any(|s| s.has_match("zzqqxx")), "{code}: a search for nonsense found something");
        }
        i18n::set_language(&prev);
    }

    /// A RECORD UNLIKE THE FACTORY ONE: every field differs.
    ///
    /// A literal without `..Default::default()` DELIBERATELY: a new field in `Settings` will not compile
    /// until it is written in here — and having written it, the author immediately meets the guard that
    /// says the reset covers everything and remembers the section. The ellipsis would turn both guards
    /// into decoration.
    fn changed() -> Settings {
        Settings {
            language: "en".into(),
            scheme: "light".into(),
            viewcube_size: 2,
            hotkeys: [("part.extrude".to_string(), "W".to_string())].into_iter().collect(),
            help_lang: "en".into(),
            help_external: true,
            msaa: 8,
            autosave_secs: 600,
            undo_cap: 7,
            ghost_alpha: 200,
            persp_fov_deg: 60.0,
            show_rapids: true,
            cam_tab_enabled: true,
            gpu_viewport: false,
            cam_perspective: true,
            smooth_shading: false,
            show_contours: false,
            show_joints: false,
            show_interference: true,
            snap: super::super::Snapping { on: false, grid: 7.5, rot_deg: 30.0 },
            auto_constrain: false,
            defaults: super::super::Defaults { extrude_h: 42.0, offset_2d: 9.5 },
            ui_scale: 1.4,
            recent: vec!["/tmp/a.qcad".into()],
            recent_limit: 3,
            pick_precision: 2,
        }
    }

    /// A RESET RETURNS THE FACTORY VALUES OF THAT SECTION ALONE AND DOES NOT TOUCH THE NEIGHBOURS.
    ///
    /// Both sides are checked: what is ours came back, what is not stayed. The check that ours came back
    /// is on its own green for `*s = Settings::default()` as well — that is, for a button that wipes the
    /// whole window.
    #[test]
    fn resetting_a_section_touches_only_that_section() {
        let d = Settings::default();

        for sec in Sec::all() {
            let mut s = changed();
            sec.reset(&mut s);
            let c = changed();
            // WHAT IS NOT OURS IS UNTOUCHED — one field for each OTHER section
            for other in Sec::all().iter().filter(|o| *o != sec) {
                let mut only_other = changed();
                other.reset(&mut only_other);
                // a field that ONLY `other` changes must stay changed in ours
                match other {
                    Sec::General => assert_eq!(s.language, c.language, "{sec:?}: the reset touched the language"),
                    Sec::Appearance => assert_eq!(s.scheme, c.scheme, "{sec:?}: the reset touched the scheme"),
                    Sec::Viewport => assert_eq!(s.viewcube_size, c.viewcube_size, "{sec:?}: the reset touched the viewport"),
                    Sec::Sketch => assert_eq!(s.snap.grid, c.snap.grid, "{sec:?}: the reset touched the sketch"),
                    Sec::Part => assert_eq!(s.defaults.extrude_h, c.defaults.extrude_h, "{sec:?}: the reset touched the part"),
                    Sec::Assembly => assert_eq!(s.show_joints, c.show_joints, "{sec:?}: the reset touched the assembly"),
                    Sec::Cam => assert_eq!(s.cam_tab_enabled, c.cam_tab_enabled, "{sec:?}: the reset touched machining"),
                }
            }
        }

        // OURS CAME BACK — by name for every section
        let after = |sec: Sec| {
            let mut s = changed();
            sec.reset(&mut s);
            s
        };
        assert_eq!(after(Sec::General).language, d.language);
        assert_eq!(after(Sec::Appearance).scheme, d.scheme);
        let v = after(Sec::Viewport);
        assert_eq!((v.viewcube_size, v.gpu_viewport, v.cam_perspective, v.smooth_shading), (d.viewcube_size, d.gpu_viewport, d.cam_perspective, d.smooth_shading));
        let sk = after(Sec::Sketch);
        assert_eq!((sk.snap.on, sk.snap.grid, sk.snap.rot_deg, sk.auto_constrain), (d.snap.on, d.snap.grid, d.snap.rot_deg, d.auto_constrain));
        let pt = after(Sec::Part);
        assert_eq!((pt.defaults.extrude_h, pt.defaults.offset_2d), (d.defaults.extrude_h, d.defaults.offset_2d));
        let asm = after(Sec::Assembly);
        assert_eq!((asm.show_contours, asm.show_joints, asm.show_interference), (d.show_contours, d.show_joints, d.show_interference));
        let cam = after(Sec::Cam);
        assert_eq!((cam.cam_tab_enabled, cam.show_rapids), (d.cam_tab_enabled, d.show_rapids));
    }

    /// THE SECTIONS COVER EVERY SETTING WHOLE: resetting them all in turn gives exactly the factory
    /// values.
    ///
    /// The former guards checked the reset BY NAME field by field, and so did not see what was skipped:
    /// the row is in the window, it is in the table, and `reset` forgot about it — "reset the section"
    /// does not touch it, and that is learnt only by comparing values by hand. And so it turned out:
    /// autosave, the depth of undo, the transparency of ghosts, the field of view and the shading all
    /// travelled past the reset.
    ///
    /// Here the question is put differently and so catches a whole class: the sections must PARTITION
    /// the settings without a remainder. The comparison goes through serialisation — it sees every field
    /// at once, including the ones added tomorrow.
    #[test]
    fn the_sections_together_cover_every_setting() {
        let mut s = changed();
        for sec in Sec::all() {
            sec.reset(&mut s);
        }
        // NOT SETTINGS BUT HISTORY AND A LAYOUT: the reset deliberately does not touch the list of
        // recent files (it is not a value but a memory of work done), and the keys are reassigned in a
        // window of their own with a button of their own.
        let d = Settings::default();
        s.recent = d.recent.clone();
        s.hotkeys = d.hotkeys.clone();

        let got = ron::ser::to_string_pretty(&s, Default::default()).expect("the settings serialise");
        let want = ron::ser::to_string_pretty(&d, Default::default()).expect("the factory ones serialise");
        let diff: Vec<String> = got.lines().zip(want.lines()).filter(|(a, b)| a != b).map(|(a, b)| format!("  got {} — want {}", a.trim(), b.trim())).collect();
        assert!(
            diff.is_empty(),
            "EVERY section was reset and the settings did not become the factory ones ({}) — so no section resets these fields:\n{}",
            diff.len(),
            diff.join("\n")
        );
    }

    /// THE LIST OF ROWS AND THE WINDOW ITSELF DO NOT DIVERGE — checked IN BOTH DIRECTIONS.
    ///
    /// The labels of the rows are declared in a table of their own (the search reads it), while the
    /// window draws them. Those are two places, and they can diverge silently: a setting appears in the
    /// window but is not found by the search — and is taken not to exist at all.
    #[test]
    fn the_row_table_and_the_window_agree() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        let body = {
            let a = code.find("fn settings_section_body").expect("the body of the section is in place");
            &code[a..]
        };
        // 1) every declared row IS REALLY drawn
        for sec in Sec::all() {
            for k in sec.row_keys() {
                assert!(body.contains(&format!("show(\"{k}\")")), "the row \"{k}\" is declared in section {sec:?} and the window does not draw it");
            }
        }
        // 2) and the other way round: everything the window draws through `show` is declared in the table
        let declared: Vec<&str> = Sec::all().iter().flat_map(|s| s.row_keys().iter().copied()).collect();
        let mut rest = body;
        while let Some(i) = rest.find("show(\"") {
            let after = &rest[i + 6..];
            let end = after.find('"').expect("the closing quote");
            let k = &after[..end];
            assert!(declared.contains(&k), "the window draws the row \"{k}\", which is not in the table of sections — the search will not find it");
            rest = &after[end..];
        }
    }

    /// THE DECOY LABEL ABOUT UNITS IS GONE.
    ///
    /// It stood among the settings and looked like a setting, yet it switched nothing: inches were left
    /// for a separate piece of work later. An interface pretending it can do what it cannot is worse
    /// than a missing item.
    #[test]
    fn the_fake_units_label_is_gone() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]").next().expect("the working part");
        assert!(!code.contains("settings-units-mm"), "the label about millimetre units is back in the settings window while there is still no choice of units");
    }

    /// THE MACHINING SECTION IS NOT SHOWN WHILE THE MODULE IS OFF.
    #[test]
    fn the_machining_section_hides_with_the_module() {
        let mut app = App::default();
        app.win.settings = true;
        app.set_cam_tab_for_test(false);
        assert!(!app.settings_sections_visible().iter().any(|s| s.is_cam()), "CAM is off and the machining section is visible in the settings window");
        app.set_cam_tab_for_test(true);
        assert!(app.settings_sections_visible().iter().any(|s| s.is_cam()), "CAM is on, so the machining section must come back");
    }
}

/// A SETTING THAT DOES NOT APPLY DOES NOT PRETEND TO WORK.
///
/// The field-of-view slider moved under an orthographic projection and changed nothing; the shading on
/// the software rasteriser did the same. A setting that pretends to work is worse than a missing one: it
/// gets turned, nothing happens, and the conclusion drawn is about the program rather than about the
/// setting.
#[cfg(test)]
mod applicability_tests {
    use super::super::App;

    /// UNDER ORTHO THE REASON THE FIELD OF VIEW DOES NOT WORK IS SAID; UNDER PERSPECTIVE NOTHING IS.
    ///
    /// Both sides are checked: the guard that the reason is shown is on its own green for a window that
    /// writes it always — that is, for a setting declared broken for ever.
    #[test]
    fn the_field_of_view_says_why_it_is_off_under_ortho() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let why = crate::i18n::tr("settings-fov-needs-persp");

        // THE SECTION HAS TO BE OPENED: the window always starts on General, and the first edition of
        // the test looked for the field-of-view row where it could not be.
        let mut app = App::default();
        app.win.settings = true;
        app.scheme.section = super::super::settings_sections::SettingsSection::Viewport;
        app.set.cam_perspective = false;
        let ortho = super::super::screen_keys::tests::frame_text(&mut app, |a, c| a.settings_window(c));

        let mut app2 = App::default();
        app2.win.settings = true;
        app2.scheme.section = super::super::settings_sections::SettingsSection::Viewport;
        app2.set.cam_perspective = true;
        let persp = super::super::screen_keys::tests::frame_text(&mut app2, |a, c| a.settings_window(c));
        crate::i18n::set_language(&prev);

        assert!(ortho.iter().any(|t| t.contains(&why)), "under an orthographic projection it is not said why the field of view does not work: {ortho:?}");
        assert!(!persp.iter().any(|t| t.contains(&why)), "under perspective the setting works and the window still makes excuses");
    }

    /// AND THE ORDER OF THE ROWS RUNS FROM THE IMPORTANT TO THE RARE.
    ///
    /// A guard over the source: the projection is switched daily and the shading once in a lifetime, and
    /// they must stand in that order. The shading used to come first.
    #[test]
    fn the_rows_go_from_the_important_to_the_rare() {
        let src = crate::gui::panels_source::PANELS;
        let sec = &src[src.find("Sec::Viewport => {").expect("the viewport section")..];
        let sec = &sec[..sec.find("Sec::Sketch => {").unwrap_or(sec.len())];
        let at = |k: &str| sec.find(&format!("show(\"{k}\")")).unwrap_or_else(|| panic!("the row {k} has gone from the section"));
        let order = ["settings-engine", "settings-projection", "settings-shading", "settings-viewcube", "settings-pick-precision", "settings-ghost-alpha", "settings-fov", "settings-msaa"];
        let mut prev = 0;
        for k in order {
            let p = at(k);
            assert!(p >= prev, "the row \"{k}\" stands in the wrong place — the order of the section is broken");
            prev = p;
        }
    }
}
