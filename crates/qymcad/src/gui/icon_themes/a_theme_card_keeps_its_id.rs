//! A CARD OF A THEME IN THE MANAGER KEEPS ITS ID while the lists change between frames: a theme activated, the cascade
//! reordered, a theme come to the disk under a live reload. egui marks with a red frame a place whose widgets all took
//! other ids since the frame before.
//!
//! Reported behaviour: under load, the journey of a theme reloaded live ended with "egui marked widgets under one id: a
//! widget took another id" in the window of the icon themes.
#[cfg(test)]
mod tests {
    use super::super::discovery::*;
    use super::super::manager_window::*;
    use qymcad_ui_state::icons::{IconManifest, PackageType};
    use qymcad_ui_state::WinKind;

    /// A THEME of the tests: its id and its name.
    struct Theme {
        id: &'static str,
        name: &'static str,
    }

    /// Theme `t` laid in folder `dir`.
    fn a_theme(dir: &std::path::Path, t: &Theme) {
        let at = dir.join(t.id);
        std::fs::create_dir_all(at.join("icons").join("sketch")).expect("the folder of a theme");
        let manifest = IconManifest {
            package_type: PackageType::IconTheme,
            id: t.id.into(),
            name: t.name.into(),
            version: "1.0".into(),
            author: "Tester".into(),
            license: "MIT".into(),
            description: "Test".into(),
            translations: Default::default(),
        };
        std::fs::write(at.join("manifest.ron"), manifest.to_ron().expect("a manifest")).expect("the manifest of a theme");
    }

    /// egui's red frames in `out`: the places whose widgets all took other ids since the frame before.
    fn red_frames(out: &egui::FullOutput) -> Vec<egui::Rect> {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Rect(r) if s.clip_rect == egui::Rect::EVERYTHING && r.stroke.color == egui::Color32::RED && r.fill == egui::Color32::TRANSPARENT => Some(r.rect),
                _ => None,
            })
            .collect()
    }

    /// egui's complaints of one id taken twice in `out`, by their words.
    fn ids_taken_twice(out: &egui::FullOutput) -> Vec<String> {
        fn words(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Text(t) if ["First use of", "Second use of", "Double use of"].iter().any(|w| t.galley.text().starts_with(w)) => out.push(t.galley.text().to_string()),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| words(s, out)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        out.shapes.iter().for_each(|s| words(&s.shape, &mut found));
        found
    }

    /// THE CARDS OF THE THEMES KEEP THEIR IDS WHILE THE LISTS CHANGE: a theme activated, the cascade swapped, a theme come
    /// to the disk with its reload served in the middle of a frame, a theme twice in the cascade, a theme deactivated -
    /// no place marked red, no id taken twice. Red before: four marks on the swap, one on the theme come to the disk,
    /// three on the deactivation.
    #[test]
    fn the_cards_of_the_themes_keep_their_ids_while_the_lists_change() {
        let dir = std::env::temp_dir().join(format!("qymcad_theme_ids_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for t in [Theme { id: "theme-b", name: "Bravo" }, Theme { id: "theme-c", name: "Charlie" }, Theme { id: "theme-d", name: "Delta" }] {
            a_theme(&dir, &t);
        }
        let dirs = vec![dir.clone()];
        poll_theme_discovery(&dirs);
        let mut app = crate::gui::App::default();
        app.set.active_icon_packs = vec!["theme-c".into()];
        app.win.open(WinKind::IconManager);
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        // a frame of the window; `amid`, done in it after the window is drawn, is what a worker does in the middle of it
        let draw = |app: &mut crate::gui::App, amid: &dyn Fn()| {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| {
                draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs);
                amid();
            })
        };
        let nothing = || {};
        for _ in 0..3 {
            let _ = draw(&mut app, &nothing);
        }
        let mut wrong = Vec::new();
        let mut look = |what: &str, out: egui::FullOutput| {
            let red = red_frames(&out);
            let twice = ids_taken_twice(&out);
            if !red.is_empty() || !twice.is_empty() {
                wrong.push(format!("{what}: marked red at {red:?}, ids taken twice {twice:?}"));
            }
        };
        app.set.active_icon_packs = vec!["theme-c".into(), "theme-b".into()];
        look("a theme activated", draw(&mut app, &nothing));
        app.set.active_icon_packs = vec!["theme-b".into(), "theme-c".into()];
        look("the cascade swapped", draw(&mut app, &nothing));
        let comes = || {
            a_theme(&dir, &Theme { id: "theme-a", name: "Alpha" });
            poll_theme_discovery(&dirs);
        };
        look("a theme come to the disk, reloaded in the middle of a frame", draw(&mut app, &comes));
        look("the frame after the reload", draw(&mut app, &nothing));
        app.set.active_icon_packs = vec!["theme-c".into(), "theme-c".into()];
        look("a theme twice in the cascade", draw(&mut app, &nothing));
        app.set.active_icon_packs = vec!["theme-c".into()];
        look("a theme deactivated", draw(&mut app, &nothing));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// THE THEMES CHANGE BETWEEN FRAMES, NOT WITHIN ONE: a reload served in the middle of a frame is read by the places
    /// drawn after it in that frame as they read it before - the settings and the manager see one list - and by the
    /// next frame as it is.
    #[test]
    fn the_themes_change_between_frames_not_within_one() {
        let dir = std::env::temp_dir().join(format!("qymcad_theme_frame_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        a_theme(&dir, &Theme { id: "theme-b", name: "Bravo" });
        let dirs = vec![dir.clone()];
        poll_theme_discovery(&dirs);
        let ctx = egui::Context::default();
        let names = |t: &DiscoveredThemes| t.packs.iter().map(|p| p.manifest.id.clone()).collect::<Vec<_>>();
        let mut seen = Vec::new();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            seen.push(names(&themes_of_the_frame(ui.ctx(), &dirs)));
            a_theme(&dir, &Theme { id: "theme-a", name: "Alpha" });
            poll_theme_discovery(&dirs);
            seen.push(names(&themes_of_the_frame(ui.ctx(), &dirs)));
        });
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| seen.push(names(&themes_of_the_frame(ui.ctx(), &dirs))));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(seen[0], seen[1], "the list changed within a frame");
        assert!(seen[2].contains(&"theme-a".to_string()), "the next frame did not read the theme come to the disk: {:?}", seen[2]);
    }
}
