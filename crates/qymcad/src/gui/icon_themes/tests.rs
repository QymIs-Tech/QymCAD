use super::appearance_section::*;
use super::discovery::*;
use super::gallery::*;
use super::manager_window::*;
use super::packager_dialog::*;
use super::sidebar::*;
use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{load_default_pack, BundleFormat, IconId, IconManifest, IconPack, PackSource, PackageType, ALL_ICONS, DEFAULT_THEME_ID};
use qymcad_ui_state::Settings;

static THEME_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_theme_test() -> std::sync::MutexGuard<'static, ()> {
    THEME_TEST_MUTEX.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn test_theme_directories_resolution() {
    let bundled = bundled_themes_dir();
    assert!(bundled.exists());

    let all = all_theme_dirs();
    assert!(!all.is_empty());
    assert!(all.contains(&bundled));
}

#[test]
fn test_apply_icon_themes_cascade_logic() {
    let ctx = egui::Context::default();
    let set = Settings { active_icon_packs: vec!["nonexistent-pack".into()], ..Default::default() };
    apply_icon_themes(&set, &ctx, &qymcad_scheme::dark());

    // Fallback works even when active packs are missing: falls back to built-in default SVG pack
    let stack = qymcad_ui_state::icons::get_active_icon_stack(&ctx);
    let icon = qymcad_ui_state::icons::resolve_icon(qymcad_ui_state::icons::IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(icon.pack_id, DEFAULT_THEME_ID);
}

#[test]
fn test_packager_dialog_state_defaults() {
    let state = PackagerDialogState::default();
    assert!(!state.is_open);
    assert_eq!(state.license, "LGPL-2.1-or-later");
}

#[test]
fn default_theme_card_uses_the_bundle_translation() {
    use crate::gui::App;

    let previous_language = crate::i18n::language();
    crate::i18n::set_language("ru");
    let mut app = App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let dirs = [bundled_themes_dir()];
    let mut draw = || {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs))
    };
    let _ = draw();
    let output = draw();
    fn has_sidebar_text(shape: &egui::epaint::Shape, expected: &str) -> bool {
        match shape {
            egui::epaint::Shape::Text(text) => text.pos.x < 360.0 && text.galley.text() == expected,
            egui::epaint::Shape::Vec(shapes) => shapes.iter().any(|shape| has_sidebar_text(shape, expected)),
            _ => false,
        }
    }
    let default = load_default_pack().expect("embedded default theme");
    let translated = default.manifest.name_for_locale("ru");
    assert!(output.shapes.iter().any(|shape| has_sidebar_text(&shape.shape, translated)), "base card must show its bundle's Russian name");
    crate::i18n::set_language(&previous_language);
}

#[test]
fn theme_card_titles_are_left_aligned_in_sidebar() {
    let _lock = lock_theme_test();
    use crate::gui::App;

    let mut app = App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let dirs = [bundled_themes_dir()];
    let mut draw = || {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs))
    };
    let _ = draw();
    let output = draw();
    fn find_sidebar_text_pos(shape: &egui::epaint::Shape, expected: &str) -> Option<egui::Pos2> {
        match shape {
            egui::epaint::Shape::Text(text) if text.pos.x < 360.0 && text.galley.text() == expected => Some(text.pos),
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find_sidebar_text_pos(s, expected)),
            _ => None,
        }
    }
    let default = load_default_pack().expect("embedded default theme");
    let name = default.manifest.name_for_locale(&crate::i18n::language());
    let badge_text = crate::i18n::tr("bundle-format-embedded");
    let title_pos = output.shapes.iter().find_map(|shape| find_sidebar_text_pos(&shape.shape, name)).expect("base card title");
    let badge_pos = output.shapes.iter().find_map(|shape| find_sidebar_text_pos(&shape.shape, &badge_text)).expect("embedded badge text");
    // Title starts flush with the content column, while badge has a 5px inner margin + gear icon, so title_pos.x is <= badge_pos.x
    assert!(title_pos.x <= badge_pos.x, "title pos x ({}) must be left-aligned before or flush with badge ({})", title_pos.x, badge_pos.x);
    assert!((badge_pos.x - title_pos.x) < 28.0, "title pos x ({}) must be directly above badge ({})", title_pos.x, badge_pos.x);
}

#[test]
fn test_bundled_shapr_alike_pack_discovered() {
    let bundled = bundled_themes_dir();
    let packs = qymcad_ui_state::icons::discover_packs_in(&bundled);
    assert!(packs.iter().any(|p| p.manifest.id == "shapr-alike"), "bundled shapr-alike pack must be found");
    let shapr = packs.iter().find(|p| p.manifest.id == "shapr-alike").unwrap();
    assert!(shapr.coverage().present >= 5);
    assert_eq!(shapr.format(), BundleFormat::Directory);
    assert!(shapr.is_directory());
    let readme = shapr.get_readme();
    assert!(readme.contains("Shapr-Alike"), "Shapr-Alike pack should have markdown description");
}

#[test]
fn test_all_bundled_themes_are_embedded_in_manager() {
    let packs = discover_all_theme_packs(&all_theme_dirs()).packs;
    let shapr = packs.iter().find(|p| p.manifest.id == "shapr-alike" && !p.has_id_conflict()).expect("shapr-alike must be present");
    assert_eq!(shapr.format(), BundleFormat::Embedded, "Shapr-Alike must be marked Embedded in manager");
    assert!(!shapr.is_directory(), "Shapr-Alike must not be marked directory");
}

#[test]
fn selected_bundled_shapr_alike_redraw_stays_responsive() {
    let _lock = lock_theme_test();
    use crate::gui::App;
    let mut app = App::default();
    app.set.active_icon_packs.clear();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let dirs = [bundled_themes_dir()];
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs))
    };
    fn find_sidebar_label(shape: &egui::epaint::Shape, title: &str) -> Option<egui::Pos2> {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == title && text.pos.x < 360.0 => Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center()),
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_sidebar_label(shape, title)),
            _ => None,
        }
    }
    let _ = draw(vec![]);
    let output = draw(vec![]);
    let base_title = load_default_pack().expect("embedded default theme").manifest.name_for_locale(&crate::i18n::language()).to_string();
    let base = output.shapes.iter().find_map(|shape| find_sidebar_label(&shape.shape, &base_title)).expect("base theme card");
    let base_click = |pressed| egui::Event::PointerButton { pos: base, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![egui::Event::PointerMoved(base)]);
    let _ = draw(vec![base_click(true)]);
    let output = draw(vec![base_click(false)]);
    let shapr = IconPack::from_directory(bundled_themes_dir().join("shapr-alike")).expect("bundled Shapr-Alike theme");
    let name = shapr.manifest.name_for_locale(&crate::i18n::language());
    let at = output.shapes.iter().find_map(|shape| find_sidebar_label(&shape.shape, name)).expect("bundled Shapr-Alike card");
    let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![egui::Event::PointerMoved(at)]);
    let _ = draw(vec![click(true)]);
    let _ = draw(vec![click(false)]);
    let selected = ctx.data(|data| data.get_temp::<IconManagerState>(egui::Id::new("icon_manager_window")).expect("manager state"));
    assert_eq!(selected.selected_pack_id, "shapr-alike");
    let start = std::time::Instant::now();
    for _ in 0..8 {
        let _ = draw(vec![]);
    }
    let elapsed = start.elapsed();
    eprintln!("selected bundled Shapr-Alike: eight redraws took {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_millis(180), "eight redraws of the selected bundled theme took {elapsed:?}");
    fn find_gallery_tab(shape: &egui::epaint::Shape, title: &str) -> Option<egui::Pos2> {
        match shape {
            egui::epaint::Shape::Text(label) if label.galley.text() == title => Some(egui::Rect::from_min_size(label.pos, label.galley.size()).center()),
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_gallery_tab(shape, title)),
            _ => None,
        }
    }
    let output = draw(vec![]);
    let gallery_title = crate::i18n::tr("icontheme-mgr-tab-gallery");
    let gallery = output.shapes.iter().find_map(|shape| find_gallery_tab(&shape.shape, &gallery_title)).expect("gallery tab");
    let gallery_click = |pressed| egui::Event::PointerButton { pos: gallery, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![egui::Event::PointerMoved(gallery)]);
    let _ = draw(vec![gallery_click(true)]);
    let _ = draw(vec![gallery_click(false)]);
    let start = std::time::Instant::now();
    for _ in 0..8 {
        let _ = draw(vec![]);
    }
    let gallery_elapsed = start.elapsed();
    eprintln!("selected bundled Shapr-Alike gallery: eight redraws took {gallery_elapsed:?}");
    assert!(gallery_elapsed < std::time::Duration::from_millis(180), "eight gallery redraws of the selected bundled theme took {gallery_elapsed:?}");
    assert!(app.set.active_icon_packs.is_empty(), "selection must not activate the theme");
}

#[test]
fn directory_preview_tracks_svg_edits_without_reloading_the_theme() {
    let root = std::env::temp_dir().join(format!("qymcad_directory_preview_{}", std::process::id()));
    let icon_dir = root.join("icons/sketch");
    std::fs::create_dir_all(&icon_dir).expect("create icon directory");
    let pack = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "preview-edits".into(),
            name: "Preview Edits".into(),
            version: "1.0".into(),
            author: String::new(),
            license: "MIT".into(),
            description: String::new(),
            translations: Default::default(),
            verified: false,
        },
        source: PackSource::Directory(root.clone()),
        is_tampered: false,
        duplicate_conflict: None,
    };
    let ctx = egui::Context::default();
    let empty = manager_directory_preview(&ctx, &pack).expect("initial preview");
    assert_eq!(empty.coverage, 0);
    assert!(empty.readmes.contains_key("en"), "preview must cache text for the interface");
    std::fs::write(root.join("README.en.md"), "# Custom English").expect("add localized README");
    let localized = manager_directory_preview(&ctx, &pack).expect("preview after adding localized README");
    assert_eq!(localized.readmes.get("en").map(String::as_str), Some("# Custom English"));
    assert!(!std::sync::Arc::ptr_eq(&empty, &localized), "new localized text must refresh the preview cache");
    let file = icon_dir.join("line.svg");
    std::fs::write(&file, br#"<svg viewBox="0 0 24 24"><path d="M0 0 L24 24"/></svg>"#).expect("add SVG");
    let added = manager_directory_preview(&ctx, &pack).expect("preview after adding SVG");
    assert_eq!(added.coverage, 1);
    assert!(added.image_generation > empty.image_generation, "added SVG must refresh the image key");
    std::fs::write(&file, br#"<svg viewBox="0 0 24 24"><script>bad()</script><path d="M0 0 L24 24"/></svg>"#).expect("edit SVG");
    let invalid = manager_directory_preview(&ctx, &pack).expect("preview after editing SVG");
    assert_eq!(invalid.invalid_icons, 1);
    std::fs::remove_file(&file).expect("remove SVG");
    let removed = manager_directory_preview(&ctx, &pack).expect("preview after removing SVG");
    assert_eq!(removed.coverage, 0);
    assert_eq!(removed.invalid_icons, 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn icon_manager_discovers_each_pack_once() {
    let _lock = lock_theme_test();
    let bundled = bundled_themes_dir();
    let temp_root = std::env::temp_dir().join(format!("qymcad_duplicate_theme_{}", std::process::id()));
    let custom_theme = temp_root.join("custom_shapr");
    std::fs::create_dir_all(&custom_theme).expect("create another theme directory");
    std::fs::copy(bundled.join("shapr-alike/manifest.ron"), custom_theme.join("manifest.ron")).expect("copy the duplicate manifest");
    let dirs = [temp_root.clone(), bundled];
    let packs = discover_all_theme_packs(&dirs).packs;

    let mut app = crate::gui::App::default();
    app.set.active_icon_packs.clear();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut painted = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let output = ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
        painted.clear();
        for shape in &output.shapes {
            crate::gui::screen_keys::tests::collect_text(&shape.shape, &mut painted);
        }
    }
    assert!(painted.iter().all(|text| !text.contains("widget ID")), "egui paints duplicate-widget warnings: {painted:?}");
    let shapr_copies = packs.iter().filter(|pack| pack.manifest.id == "shapr-alike").count();
    assert_eq!(shapr_copies, 2, "both custom copy and bundled default theme must be present in discovery");
    let conflict = packs.iter().find(|pack| pack.manifest.id == "shapr-alike" && pack.has_id_conflict()).expect("custom copy must be marked with conflict");
    assert!(matches!(&conflict.source, PackSource::Directory(path) if path == &custom_theme), "the custom directory theme must be marked as conflict");
    let default_pack = packs.iter().find(|pack| pack.manifest.id == "shapr-alike" && !pack.has_id_conflict()).expect("bundled theme must be valid");
    assert_eq!(default_pack.format(), BundleFormat::Embedded, "bundled default theme must be embedded");
    let _ = std::fs::remove_dir_all(temp_root);
}

#[test]
fn test_icon_manager_state_defaults() {
    let state = IconManagerState::default();
    assert!(!state.is_open);
    assert_eq!(state.active_tab, IconManagerTab::Readme);
    assert_eq!(state.category_filter, "all");
}

#[test]
fn icon_manager_sidebar_scroll_stops_above_actions() {
    use std::cell::Cell;
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 320.0));
    let actions = Cell::new(egui::Rect::NOTHING);
    let scroll = Cell::new(egui::Rect::NOTHING);
    let content_height = Cell::new(0.0);
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| {
        egui::Panel::left("icon_test_sidebar").exact_size(300.0).show(ui, |ui| {
            let area = manager_sidebar_shell(ui, |ui| {
                actions.set(ui.max_rect());
                let _ = ui.button("Open folder");
                let _ = ui.button("Reload themes");
            })
            .show(ui, |ui| {
                for n in 0..30 {
                    ui.add_sized([ui.available_width(), 35.0], egui::Label::new(format!("Theme {n}")));
                }
            });
            scroll.set(area.inner_rect);
            content_height.set(area.content_size.y);
        });
    });
    assert!(scroll.get().bottom() <= actions.get().top(), "the actions cover part of the list");
    assert!(content_height.get() > scroll.get().height(), "the list must remain scrollable when it is long");
}

#[test]
fn clicking_a_theme_card_selects_it_without_activating_it() {
    use std::cell::Cell;
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 160.0));
    let card = Cell::new(egui::Rect::NOTHING);
    let icon = Cell::new(egui::Rect::NOTHING);
    let name = Cell::new(egui::Rect::NOTHING);
    let badge = Cell::new(egui::Rect::NOTHING);
    let button = Cell::new(egui::Rect::NOTHING);
    let selected = Cell::new(false);
    let activated = Cell::new(false);
    let draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let _ = ctx.run_ui(input, |ui| {
            let card_res = manager_theme_card(ui, "test-pack", false, |ui| {
                ui.horizontal(|ui| {
                    icon.set(
                        ui.add(
                            egui::Image::from_bytes("bytes://test-pack-icon.svg", b"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'></svg>").fit_to_exact_size(egui::vec2(20.0, 20.0)),
                        )
                        .rect,
                    );
                    name.set(ui.label("Theme").rect);
                    badge.set(ui.scope(|ui| draw_bundle_format_badge(ui, BundleFormat::Directory, false)).response.rect);
                    let response = ui.button("Activate");
                    button.set(response.rect);
                    if response.clicked() {
                        activated.set(true);
                    }
                    Some(response.rect)
                })
                .inner
            });
            card.set(card_res.rect);
            if card_res.clicked {
                selected.set(true);
            }
        });
    };
    draw(vec![]);
    let at = egui::pos2(card.get().left() + 3.0, card.get().center().y);
    let click = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    draw(vec![egui::Event::PointerMoved(at)]);
    draw(vec![click(at, true)]);
    draw(vec![click(at, false)]);
    assert!(selected.get(), "clicking the card background must select the theme");
    assert!(!activated.get(), "selection must not activate the theme");

    for target in [icon.get(), name.get(), badge.get()] {
        selected.set(false);
        let at = target.center();
        draw(vec![egui::Event::PointerMoved(at)]);
        draw(vec![click(at, true)]);
        draw(vec![click(at, false)]);
        assert!(selected.get(), "clicking visible card content must select the theme: {target:?}");
        assert!(!activated.get(), "clicking card content must not activate the theme");
    }

    selected.set(false);
    let at = button.get().center();
    draw(vec![egui::Event::PointerMoved(at)]);
    draw(vec![click(at, true)]);
    draw(vec![click(at, false)]);
    assert!(activated.get(), "the activation button must keep its own action");
    assert!(!selected.get(), "the card must not steal the activation click");
}

#[test]
fn deactivation_does_not_paint_the_theme_in_both_lists() {
    let _lock = lock_theme_test();
    use crate::gui::App;

    struct PaintedLabel {
        text: String,
        rect: egui::Rect,
    }
    fn labels_in(shape: &egui::epaint::Shape, labels: &mut Vec<PaintedLabel>) {
        match shape {
            egui::epaint::Shape::Text(text) => labels.push(PaintedLabel { text: text.galley.text().to_string(), rect: egui::Rect::from_min_size(text.pos, text.galley.size()) }),
            egui::epaint::Shape::Vec(shapes) => shapes.iter().for_each(|shape| labels_in(shape, labels)),
            _ => {}
        }
    }

    let Some(user_dir) = user_themes_dir() else {
        eprintln!("PASSED OVER: user theme directory is unavailable");
        return;
    };
    let custom_dir = user_dir.join("sample_custom_theme_deactivate");
    let _ = std::fs::create_dir_all(&custom_dir);
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "sample_custom_theme_deactivate".into(),
        name: "Sample Custom".into(),
        version: "1.0".into(),
        author: "Tester".into(),
        license: "MIT".into(),
        description: "Sample".into(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(custom_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    let mut app = App::default();
    app.set.active_icon_packs = vec!["sample_custom_theme_deactivate".into()];
    app.set.inactive_icon_packs.retain(|id| id != "sample_custom_theme_deactivate");
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window(ui.ctx(), &mut app.win_ctx(&mut Vec::new())))
    };
    let _ = draw(vec![]);
    let output = draw(vec![]);
    let mut labels = Vec::new();
    for shape in &output.shapes {
        labels_in(&shape.shape, &mut labels);
    }
    let minus = labels.iter().find(|label| label.text == ph::MINUS).expect("the active theme has a deactivate button").rect.center();
    let click = |pressed| egui::Event::PointerButton { pos: minus, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    draw(vec![egui::Event::PointerMoved(minus)]);
    draw(vec![click(true)]);
    let _ = draw(vec![click(false)]);
    assert!(app.set.active_icon_packs.is_empty(), "the button did not deactivate the theme");
    let _ = std::fs::remove_dir_all(&custom_dir);
}

#[test]
fn default_theme_is_never_duplicated_in_active_cascade() {
    let _lock = lock_theme_test();
    use crate::gui::App;
    let mut app = App::default();
    app.set.active_icon_packs = vec![DEFAULT_THEME_ID.into()];
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| draw_icon_manager_window(ui.ctx(), &mut app.win_ctx(&mut Vec::new())));
    assert!(!app.set.active_icon_packs.contains(&DEFAULT_THEME_ID.to_string()), "default base theme must be cleaned from active list");
}

#[test]
fn selecting_unverified_archive_keeps_redraw_responsive() {
    let _lock = lock_theme_test();
    use crate::gui::App;
    let Some(sample_dir) = qymcad_paths::data("icon_themes") else {
        eprintln!("PASSED OVER: icon theme data directory is unavailable");
        return;
    };
    let sample = sample_dir.join("shapr-alike.qicons");
    if !sample.is_file() {
        eprintln!("PASSED OVER: shapr-alike.qicons is unavailable");
        return;
    }
    let sample_pack = IconPack::from_archive(&sample).expect("the reported bundle loads");
    if sample_pack.manifest.id == DEFAULT_THEME_ID || sample_pack.has_id_conflict() {
        eprintln!("PASSED OVER: shapr-alike.qicons uses reserved default theme id");
        return;
    }
    let mut app = App::default();
    app.set.active_icon_packs.retain(|id| id != &sample_pack.manifest.id);
    let active_before = app.set.active_icon_packs.clone();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window(ui.ctx(), &mut app.win_ctx(&mut Vec::new())))
    };
    let _ = draw(vec![]);
    let output = draw(vec![]);
    fn find_label(shape: &egui::epaint::Shape, name: &str) -> Option<egui::Rect> {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == name => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_label(shape, name)),
            _ => None,
        }
    }
    let label = output.shapes.iter().find_map(|shape| find_label(&shape.shape, &sample_pack.manifest.name)).expect("the reported bundle appears in the sidebar");
    let at = label.center();
    let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![egui::Event::PointerMoved(at)]);
    let _ = draw(vec![click(true)]);
    let _ = draw(vec![click(false)]);
    let selected = ctx.data(|data| data.get_temp::<IconManagerState>(egui::Id::new("icon_manager_window")).expect("manager state"));
    assert_eq!(selected.selected_pack_id, sample_pack.manifest.id, "the bundle was not selected");
    let output = draw(vec![]);
    let package_label = format!("{} {}", ph::PACKAGE, crate::i18n::tr("settings-icon-package-btn"));
    let clean_label = format!("{} {}", ph::BROOM, crate::i18n::tr("icontheme-mgr-clean-all"));
    assert!(output.shapes.iter().all(|shape| find_label(&shape.shape, &package_label).is_none()), "archive must not offer packaging");
    assert!(output.shapes.iter().all(|shape| find_label(&shape.shape, &clean_label).is_none()), "archive must not offer SVG cleaning");

    let start = std::time::Instant::now();
    for _ in 0..3 {
        let _ = draw(vec![]);
    }
    let elapsed = start.elapsed();
    eprintln!("selected unverified archive: three redraws took {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_millis(450), "three redraws after selecting the bundle took {elapsed:?}");

    let output = draw(vec![]);
    let gallery_label = crate::i18n::tr("icontheme-mgr-tab-gallery");
    let gallery_tab = output.shapes.iter().find_map(|shape| find_label(&shape.shape, &gallery_label)).expect("the gallery tab is visible").center();
    let tab_click = |pressed| egui::Event::PointerButton { pos: gallery_tab, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![egui::Event::PointerMoved(gallery_tab)]);
    let _ = draw(vec![tab_click(true)]);
    let _ = draw(vec![tab_click(false)]);
    let selected = ctx.data(|data| data.get_temp::<IconManagerState>(egui::Id::new("icon_manager_window")).expect("manager state"));
    assert_eq!(selected.active_tab, IconManagerTab::Gallery, "the gallery tab did not open");
    let start = std::time::Instant::now();
    for _ in 0..3 {
        let _ = draw(vec![]);
    }
    let elapsed = start.elapsed();
    eprintln!("unverified archive gallery: three redraws took {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_millis(450), "three gallery redraws took {elapsed:?}");
    assert_eq!(app.set.active_icon_packs, active_before, "selection must not activate the bundle");
}

#[test]
fn cleaning_a_gallery_icon_updates_its_file_and_preview() {
    let _lock = lock_theme_test();
    use crate::gui::App;
    let root = std::env::temp_dir().join(format!("qymcad_gallery_clean_{}", std::process::id()));
    let theme = root.join("repairable");
    let icons = theme.join("icons/sketch");
    std::fs::create_dir_all(&icons).expect("create theme icons");
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "repairable".into(),
        name: "Repairable Theme".into(),
        version: "1.0".into(),
        author: "Test".into(),
        license: "MIT".into(),
        description: String::new(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(theme.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let path = icons.join("line.svg");
    std::fs::write(&path, br#"<svg viewBox="0 0 24 24"><script>bad()</script><path d="M0 0 L24 24"/></svg>"#).unwrap();
    let second = icons.join("circle.svg");
    std::fs::write(&second, br#"<svg viewBox="0 0 24 24"><metadata>editor</metadata><circle cx="12" cy="12" r="8"/></svg>"#).unwrap();
    let unrepairable = icons.join("rect.svg");
    let bad_viewbox = br#"<svg viewBox="0 0 32 16"><rect width="32" height="16"/></svg>"#;
    std::fs::write(&unrepairable, bad_viewbox).unwrap();
    let dirs = [root.clone()];
    let mut app = App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs))
    };
    fn find_text(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Rect> {
        fn in_shape(shape: &egui::epaint::Shape, needle: &str) -> Option<egui::Rect> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text().contains(needle) => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| in_shape(shape, needle)),
                _ => None,
            }
        }
        shapes.iter().find_map(|shape| in_shape(&shape.shape, needle))
    }
    let click = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let _ = draw(vec![]);
    let output = draw(vec![]);
    let tab = find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-tab-gallery")).expect("gallery tab").center();
    let _ = draw(vec![egui::Event::PointerMoved(tab)]);
    let _ = draw(vec![click(tab, true)]);
    let output = draw(vec![click(tab, false)]);
    assert!(find_text(&output.shapes, &crate::i18n::tr("settings-icon-package-btn")).is_none(), "inactive folder must not offer packaging");
    assert!(find_text(&output.shapes, &crate::i18n::tr("settings-icon-refresh")).is_none(), "the sidebar must not show a refresh button");
    let archive_path = "icons/sketch/line.svg";
    let path_at = find_text(&output.shapes, archive_path).expect("gallery icon path").center();
    let _ = draw(vec![egui::Event::PointerMoved(path_at)]);
    let _ = draw(vec![click(path_at, true)]);
    let copy_output = draw(vec![click(path_at, false)]);
    assert!(copy_output.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == archive_path)), "clicking the icon path must copy it");
    let output = draw(vec![]);
    assert!(find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-path-copied")).is_some(), "copying a path needs visible confirmation");
    let button = find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-clean-icon")).expect("clean button beside invalid SVG").center();
    let before = qymcad_ui_state::icons::get_icon_revision(&ctx);
    let _ = draw(vec![egui::Event::PointerMoved(button)]);
    let _ = draw(vec![click(button, true)]);
    let _ = draw(vec![click(button, false)]);
    qymcad_ui_state::icons::validate_svg(&std::fs::read(&path).unwrap()).expect("button cleaned the SVG on disk");
    assert!(qymcad_ui_state::icons::get_icon_revision(&ctx) > before, "cleaning did not invalidate rendered icon textures");
    let output = draw(vec![]);
    assert!(find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-gallery-present")).is_some(), "the refreshed gallery does not show the cleaned icon");

    let clean_all = find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-clean-all")).expect("bulk clean button").center();
    let before_bulk = qymcad_ui_state::icons::get_icon_revision(&ctx);
    let _ = draw(vec![egui::Event::PointerMoved(clean_all)]);
    let _ = draw(vec![click(clean_all, true)]);
    let _ = draw(vec![click(clean_all, false)]);
    qymcad_ui_state::icons::validate_svg(&std::fs::read(&second).unwrap()).expect("bulk button cleaned another icon");
    assert_eq!(std::fs::read(&unrepairable).unwrap(), bad_viewbox, "bulk clean must leave geometry needing manual repair alone");
    assert!(qymcad_ui_state::icons::get_icon_revision(&ctx) > before_bulk, "bulk cleaning did not refresh icon textures");
    let output = draw(vec![]);
    let activate = find_text(&output.shapes, &crate::i18n::tr("icontheme-mgr-activate-btn")).expect("folder activation button").center();
    let _ = draw(vec![egui::Event::PointerMoved(activate)]);
    let _ = draw(vec![click(activate, true)]);
    let _ = draw(vec![click(activate, false)]);
    let output = draw(vec![]);
    let package_rect = find_text(&output.shapes, &crate::i18n::tr("settings-icon-package-btn")).expect("active folder packaging button");
    let version_rect = find_text(&output.shapes, "v1.0").expect("selected folder version");
    assert!((package_rect.center().y - version_rect.center().y).abs() < 16.0, "packaging button must share the folder badge row");
    let meta_label = crate::i18n::tr1("icontheme-mgr-meta-id", "value", &manifest.id);
    let metadata_rect = find_text(&output.shapes, &meta_label).expect("folder metadata must be visible below the header");
    assert!(metadata_rect.top() - package_rect.bottom() < 100.0, "packaging button must not stretch the folder header");
    assert!(find_text(&output.shapes, archive_path).is_some(), "folder gallery must remain visible after activation");
    let package = package_rect.center();
    let _ = draw(vec![egui::Event::PointerMoved(package)]);
    let _ = draw(vec![click(package, true)]);
    let _ = draw(vec![click(package, false)]);
    let packager = ctx.data(|data| data.get_temp::<PackagerDialogState>(egui::Id::new("icon_packager_dialog")).expect("packager state"));
    assert!(packager.is_open, "packager did not open");
    assert_eq!(packager.source_dir, theme.display().to_string(), "packager must use the selected folder");
    assert_eq!(packager.id, manifest.id);
    assert_eq!(packager.output_file, theme.with_extension("qicons").display().to_string());
    assert!(app.set.active_icon_packs.contains(&manifest.id), "the folder was not activated");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gallery_rows_stack_within_the_panel_width() {
    use std::cell::RefCell;
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 300.0));
    let mut icons = std::collections::HashMap::new();
    icons.insert("icons/sketch/line.svg".to_string(), br#"<svg viewBox="0 0 32 16"/>"#.to_vec());
    let pack = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "gallery-test".into(),
            name: "Gallery Test".into(),
            version: "1.0".into(),
            author: "Test".into(),
            license: "MIT".into(),
            description: String::new(),
            translations: Default::default(),
            verified: false,
        },
        source: qymcad_ui_state::icons::PackSource::Memory(icons),
        is_tampered: false,
        duplicate_conflict: None,
    };
    let rows = RefCell::new(Vec::new());
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let output = ctx.run_ui(input, |ui| {
        ui.set_width(380.0);
        let pal = qymcad_scheme::dark();
        let line = qymcad_ui_state::icons::IconId::SketchLine;
        let line_icon = pack.inspect_svg_for_id(line).map(|data| data.map(egui::load::Bytes::from));
        rows.borrow_mut().push(draw_gallery_icon_row(ui, &GalleryRowParams { pack: &pack, id: line, icon: &line_icon, palette: &pal, cleanable: false, generation: 0, copied_path: None }).rect);
        let longest_path = ALL_ICONS.iter().copied().max_by_key(|id| id.relative_path().len()).unwrap();
        let longest_icon = pack.inspect_svg_for_id(longest_path).map(|data| data.map(egui::load::Bytes::from));
        rows.borrow_mut()
            .push(draw_gallery_icon_row(ui, &GalleryRowParams { pack: &pack, id: longest_path, icon: &longest_icon, palette: &pal, cleanable: false, generation: 0, copied_path: None }).rect);
    });
    let rows = rows.borrow();
    assert!(rows[0].width() <= 380.0, "a gallery row expands the panel");
    assert!(rows[1].top() >= rows[0].bottom(), "gallery icons must form a vertical list");
    assert!(rows[1].width() <= 380.0, "long paths expand the panel");

    fn painted_in(shape: &egui::epaint::Shape, labels: &mut Vec<String>, previews: &mut usize) {
        match shape {
            egui::epaint::Shape::Text(text) => labels.push(text.galley.text().to_string()),
            egui::epaint::Shape::Rect(rect) if (rect.rect.width() - 56.0).abs() < 0.1 && (rect.rect.height() - 56.0).abs() < 0.1 && rect.stroke.width > 0.0 => {
                *previews += 1;
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().for_each(|shape| painted_in(shape, labels, previews)),
            _ => {}
        }
    }
    let mut labels = Vec::new();
    let mut preview_frames = 0;
    for shape in &output.shapes {
        painted_in(&shape.shape, &mut labels, &mut preview_frames);
    }
    let longest_path = ALL_ICONS.iter().max_by_key(|id| id.relative_path().len()).unwrap().relative_path();
    assert!(labels.iter().any(|text| text.contains("icons/sketch/line.svg")), "the archive path is not visible");
    assert!(labels.iter().any(|text| text.contains(&format!("icons/{longest_path}.svg"))), "a missing icon needs its expected archive path");
    assert!(labels.iter().any(|text| text.contains("non-square viewBox")), "the specific SVG error is not visible");
    assert!(!labels.iter().any(|text| text.contains(&crate::i18n::tr("icontheme-mgr-clean-icon"))), "archive icons must not show cleaning controls");
    assert!(labels.iter().any(|text| text.contains(&crate::i18n::tr("icontheme-mgr-gallery-missing"))), "missing icons need a neutral status");
    assert_eq!(preview_frames, 2, "every icon needs a visible 56 px preview frame");
}

#[test]
fn discover_theme_packs_caches_results_until_directory_changes() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_cache_test_{}", std::process::id()));
    let theme_dir = temp_root.join("test_theme");
    std::fs::create_dir_all(&theme_dir).expect("create test theme dir");
    let manifest_content = r#"(
            id: "test-cache",
            name: "Test Cache",
            version: "1.0.0",
            author: "Tester",
            license: "MIT",
        )"#;
    std::fs::write(theme_dir.join("manifest.ron"), manifest_content).expect("write manifest");

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let initial_stats = discovery_cache_stats();
    let first = discover_theme_packs_in_dirs(&dirs);
    assert_eq!(first.packs.len(), 1);
    let after_first = discovery_cache_stats();
    assert_eq!(after_first.misses, initial_stats.misses + 1);

    let second = discover_theme_packs_in_dirs(&dirs);
    assert_eq!(second.packs.len(), 1);
    let after_second = discovery_cache_stats();
    assert_eq!(after_second.hits, initial_stats.hits + 1);
    assert_eq!(after_second.misses, after_first.misses);

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn icon_manager_window_displays_rejected_archives_with_reason() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_reject_ui_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_root).expect("create temp dir");
    let broken_archive = temp_root.join("broken_pack.qicons");
    std::fs::write(&broken_archive, b"corrupted data that cannot be parsed as zip").expect("write broken archive");

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut painted = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let output = ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
        painted.clear();
        for shape in &output.shapes {
            crate::gui::screen_keys::tests::collect_text(&shape.shape, &mut painted);
        }
    }

    assert!(painted.iter().any(|text| text.contains("broken_pack.qicons")), "icon manager window must display rejected archive filename: {painted:?}");
    assert!(painted.iter().any(|text| text.contains(&crate::i18n::tr("icontheme-mgr-rejected-title"))), "icon manager window must display rejected section title: {painted:?}");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn png_image_loader_is_available_for_theme_previews() {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);

    let png_bytes = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];
    let uri = "bytes://test/preview.png";
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| {
        let img = egui::Image::from_bytes(uri, png_bytes.to_vec());
        ui.add(img);
    });
    let load_result = ctx.try_load_image(uri, egui::load::SizeHint::default());
    assert!(load_result.is_ok(), "PNG preview must be supported by installed image loaders");
}

#[test]
fn theme_preview_image_cache_is_invalidated_when_file_changes() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_preview_cache_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let pack_dir = temp_root.join("test-preview-theme");
    std::fs::create_dir_all(&pack_dir).unwrap();

    let manifest = r#"(
            id: "test-preview-theme",
            name: "Test Preview Theme",
            version: "1.0.0",
            author: "Tester",
            license: "MIT",
        )"#;
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();
    let preview1 = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="5"/></svg>"#;
    std::fs::write(pack_dir.join("preview.svg"), preview1).unwrap();

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);

    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.selected_pack_id = "test-preview-theme".to_string();
        state.active_tab = IconManagerTab::Readme;
    });

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    for _ in 0..2 {
        let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
    }

    let id_key = egui::Id::new("preview_image_prev_uri").with("test-preview-theme");
    let first_uri = ctx.data(|d| d.get_temp::<String>(id_key)).expect("first preview URI should be tracked");
    assert!(first_uri.contains("-g1"), "URI should contain generation number, got: {first_uri}");

    // Now modify preview.svg
    std::thread::sleep(std::time::Duration::from_millis(50));
    let preview2 = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect width="20" height="20"/></svg>"#;
    std::fs::write(pack_dir.join("preview.svg"), preview2).unwrap();

    for _ in 0..2 {
        let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
    }

    let second_uri = ctx.data(|d| d.get_temp::<String>(id_key)).expect("second preview URI should be tracked");
    assert_ne!(first_uri, second_uri, "URI must change when preview file is modified");
    assert!(second_uri.contains("-g2"), "URI should contain updated generation number, got: {second_uri}");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn icon_with_current_color_is_prepared_with_theme_stroke_in_gallery() {
    let _lock = lock_theme_test();
    use egui::load::{ImagePoll, SizeHint};

    let temp_root = std::env::temp_dir().join(format!("qymcad_mono_gallery_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let pack_dir = temp_root.join("test-mono-theme");
    let icons_dir = pack_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
            id: "test-mono-theme",
            name: "Test Mono Theme",
            version: "1.0.0",
            author: "Tester",
            license: "MIT",
        )"#;
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M0 0h24v24z"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg).unwrap();

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);

    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.selected_pack_id = "test-mono-theme".to_string();
        state.active_tab = IconManagerTab::Gallery;
        state.category_filter = "sketch".to_string();
    });

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    for _ in 0..2 {
        let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
    }

    let rev = qymcad_ui_state::icons::get_icon_revision(&ctx);
    let uri = format!("bytes://mgr/test-mono-theme/{}/p{:016x}_r{rev}-g1/sketch/line.svg", app.scheme.pal.identifier(), app.scheme.pal.fingerprint());
    let poll = ctx.try_load_image(&uri, SizeHint::Width(24)).expect("gallery image must load");
    if let ImagePoll::Ready { image } = poll {
        let center_pixel = image.pixels[(image.size[1] / 2) * image.size[0] + image.size[0] / 2];
        assert!(center_pixel.a() > 0, "pixel must be non-transparent");
        let expected_ratio = qymcad_scheme::dark().icon_stroke[0] as f32 / 255.0;
        let actual_ratio = center_pixel.r() as f32 / center_pixel.a() as f32;
        assert!((actual_ratio - expected_ratio).abs() < 0.05, "icon with currentColor must be prepared with active theme stroke color, got: {center_pixel:?}");
    } else {
        panic!("gallery image should be ready");
    }

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn packager_modal_cancel_button_closes_dialog() {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);

    let mut state = PackagerDialogState { is_open: true, id: "test".into(), name: "Test".into(), ..Default::default() };

    fn find_text(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Rect> {
        fn in_shape(shape: &egui::epaint::Shape, needle: &str) -> Option<egui::Rect> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text().contains(needle) => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| in_shape(shape, needle)),
                _ => None,
            }
        }
        shapes.iter().find_map(|shape| in_shape(&shape.shape, needle))
    }

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let cancel_text = crate::i18n::tr("nav-cancel");

    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input.clone(), |ui| draw_packager_modal(ui.ctx(), &mut state));
    let output = ctx.run_ui(input, |ui| draw_packager_modal(ui.ctx(), &mut state));

    let cancel_rect = find_text(&output.shapes, &cancel_text).expect("Cancel button must be rendered in packager modal");
    let cancel_pos = cancel_rect.center();

    let click_down = egui::RawInput {
        screen_rect: Some(screen),
        events: vec![
            egui::Event::PointerMoved(cancel_pos),
            egui::Event::PointerButton { pos: cancel_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        ],
        ..Default::default()
    };
    let _ = ctx.run_ui(click_down, |ui| draw_packager_modal(ui.ctx(), &mut state));

    let click_up = egui::RawInput {
        screen_rect: Some(screen),
        events: vec![egui::Event::PointerButton { pos: cancel_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }],
        ..Default::default()
    };
    let _ = ctx.run_ui(click_up, |ui| draw_packager_modal(ui.ctx(), &mut state));

    assert!(!state.is_open, "packager modal must be closed after clicking cancel");
}

#[test]
fn packager_modal_labels_and_messages_are_localized() {
    let prev = crate::i18n::language();
    crate::i18n::set_language("uk");

    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);

    let mut state = PackagerDialogState { is_open: true, id: "test".into(), name: "Test".into(), ..Default::default() };

    fn collect_shapes_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
        fn in_shape(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Text(text) => out.push(text.galley.text().to_string()),
                egui::epaint::Shape::Vec(shapes) => {
                    for s in shapes {
                        in_shape(s, out);
                    }
                }
                _ => {}
            }
        }
        for s in shapes {
            in_shape(&s.shape, out);
        }
    }

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input.clone(), |ui| draw_packager_modal(ui.ctx(), &mut state));
    let output = ctx.run_ui(input, |ui| draw_packager_modal(ui.ctx(), &mut state));

    let mut texts = Vec::new();
    collect_shapes_text(&output.shapes, &mut texts);

    let expected_id = crate::i18n::tr("icontheme-packager-field-id");
    let expected_source = crate::i18n::tr("icontheme-packager-field-source");
    crate::i18n::set_language(&prev);

    let joined = texts.join(" ");
    assert!(joined.contains(&expected_id), "Theme ID label must be localized in Ukrainian, got texts: {joined}");
    assert!(joined.contains(&expected_source), "Source Folder label must be localized in Ukrainian, got texts: {joined}");
}

#[test]
fn bundled_themes_dir_finds_themes_in_macos_and_portable_layouts() {
    let temp = std::env::temp_dir().join(format!("qymcad_bundled_themes_layout_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);

    // Layout A: Windows / Portable Linux
    let portable_root = temp.join("portable");
    let portable_exe = portable_root.join("bin").join("qymcad");
    let portable_themes = portable_root.join("bin").join("assets").join("icon-themes");
    std::fs::create_dir_all(&portable_themes).unwrap();
    std::fs::write(portable_themes.join("marker.txt"), "portable").unwrap();

    let resolved_portable = bundled_themes_dir_with(None, Some(&portable_exe));
    assert_eq!(resolved_portable, portable_themes, "portable layout should locate assets/icon-themes beside binary");

    // Layout B: macOS .app bundle
    let app_root = temp.join("QymCAD.app");
    let mac_exe = app_root.join("Contents").join("MacOS").join("qymcad");
    let mac_themes = app_root.join("Contents").join("Resources").join("assets").join("icon-themes");
    std::fs::create_dir_all(&mac_themes).unwrap();
    std::fs::write(mac_themes.join("marker.txt"), "macos").unwrap();

    let resolved_mac = bundled_themes_dir_with(None, Some(&mac_exe));
    assert_eq!(resolved_mac, mac_themes, "macOS bundle layout should locate Resources/assets/icon-themes");

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn draw_frame_polls_watched_icon_themes() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_draw_frame_poll_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
            id: "draw-frame-watched-theme",
            name: "Draw Frame Watched Theme",
            version: "1.0.0",
            author: "Tester",
            license: "MIT",
        )"#;
    std::fs::write(temp_root.join("manifest.ron"), manifest).unwrap();
    let svg1 = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg1).unwrap();

    let pack = qymcad_ui_state::icons::IconPack::from_directory(&temp_root).unwrap();
    let mut app = crate::gui::App::default();
    app.waiting.splash_until = None;
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);
    qymcad_ui_state::icons::set_pack_watching(&ctx, "draw-frame-watched-theme", true);
    qymcad_ui_state::icons::set_active_icon_stack(&ctx, vec![pack], &app.scheme.pal);

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input.clone(), |ui| app.draw_frame(ui));

    let stack1 = qymcad_ui_state::icons::get_active_icon_stack(&ctx);
    let res1 = qymcad_ui_state::icons::resolve_icon(qymcad_ui_state::IconId::SketchLine, &stack1, &app.scheme.pal);
    assert_eq!(res1.pack_id, "draw-frame-watched-theme");
    let rev1 = qymcad_ui_state::icons::get_icon_revision(&ctx);

    let svg2 = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect width="24" height="24"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg2).unwrap();

    // Allow background watcher thread to poll and update egui icon loader cache
    std::thread::sleep(std::time::Duration::from_millis(350));

    // Run draw_frame again (the frame path of both live window and Session/Hand)
    let _ = ctx.run_ui(input, |ui| app.draw_frame(ui));

    let stack2 = qymcad_ui_state::icons::get_active_icon_stack(&ctx);
    let res2 = qymcad_ui_state::icons::resolve_icon(qymcad_ui_state::IconId::SketchLine, &stack2, &app.scheme.pal);
    assert_eq!(res2.pack_id, "draw-frame-watched-theme");
    let rev2 = qymcad_ui_state::icons::get_icon_revision(&ctx);
    assert_ne!(rev1, rev2, "watcher thread must poll watched icon themes and update revision on change");

    qymcad_ui_state::icons::stop_watcher_thread(&ctx);
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn closing_window_or_switching_theme_forgets_gallery_textures() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_forget_test_{}", std::process::id()));
    let pack_dir = temp_root.join("test-forget-theme");
    let icons_dir = pack_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
            package_type: IconTheme,
            id: "test-forget-theme",
            name: "Test Forget Theme",
            version: "1.0.0",
            author: "Author",
            license: "MIT",
            description: "Test description",
        )"#;
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24z"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg).unwrap();

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);

    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.selected_pack_id = "test-forget-theme".to_string();
        state.active_tab = IconManagerTab::Gallery;
        state.category_filter = "sketch".to_string();
    });

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));

    let line_key = egui::Id::new("gallery_icon_prev_uri").with(("test-forget-theme", IconId::SketchLine));
    let cached_uri = ctx.data(|d| d.get_temp::<String>(line_key));
    assert!(cached_uri.is_some(), "gallery icon URI must be tracked while window is open");

    // Now close the window
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.is_open = false;
    });
    // Call forget_pack_gallery_textures directly or trigger close
    forget_pack_gallery_textures(&ctx, "test-forget-theme");

    let after_close = ctx.data(|d| d.get_temp::<String>(line_key));
    assert!(after_close.is_none(), "gallery icon URI must be evicted from temp data when forgotten");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn gallery_icon_uri_reflects_palette_and_evicts_on_scheme_switch() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_palette_switch_test_{}", std::process::id()));
    let pack_dir = temp_root.join("test-pal-theme");
    let icons_dir = pack_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
            package_type: IconTheme,
            id: "test-pal-theme",
            name: "Test Palette Theme",
            version: "1.0.0",
            author: "Author",
            license: "MIT",
            description: "Test description",
        )"#;
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24z" stroke="currentColor"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg).unwrap();

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);

    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.selected_pack_id = "test-pal-theme".to_string();
        state.active_tab = IconManagerTab::Gallery;
        state.category_filter = "sketch".to_string();
    });

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };

    let dark_pal = qymcad_scheme::dark();
    let dark_fp = dark_pal.fingerprint();
    app.scheme.pal = dark_pal;

    let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));

    let line_key = egui::Id::new("gallery_icon_prev_uri").with(("test-pal-theme", IconId::SketchLine));
    let uri_dark = ctx.data(|d| d.get_temp::<String>(line_key)).expect("dark URI must be tracked");
    let dark_tag = format!("p{:016x}", dark_fp);
    assert!(uri_dark.contains("/dark/"), "gallery icon URI must contain palette id /dark/, got {uri_dark}");
    assert!(uri_dark.contains(&dark_tag), "gallery icon URI must contain palette fingerprint {dark_tag}, got {uri_dark}");

    // Switch to light scheme
    let light_pal = qymcad_scheme::light();
    let light_fp = light_pal.fingerprint();
    app.scheme.pal = light_pal;

    let _ = ctx.run_ui(input.clone(), |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));

    let uri_light = ctx.data(|d| d.get_temp::<String>(line_key)).expect("light URI must be tracked");
    let light_tag = format!("p{:016x}", light_fp);
    assert!(uri_light.contains("/light/"), "gallery icon URI must contain palette id /light/, got {uri_light}");
    assert!(uri_light.contains(&light_tag), "gallery icon URI must contain new palette fingerprint {light_tag}, got {uri_light}");
    assert_ne!(uri_dark, uri_light, "switching color palette must generate distinct URI to bust egui texture cache");

    // Switch to custom named scheme (e.g. nord-theme)
    let mut custom_pal = qymcad_scheme::dark();
    custom_pal.id = "nord-theme".to_string();
    let custom_fp = custom_pal.fingerprint();
    app.scheme.pal = custom_pal;

    let _ = ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));

    let uri_custom = ctx.data(|d| d.get_temp::<String>(line_key)).expect("custom URI must be tracked");
    let custom_tag = format!("p{:016x}", custom_fp);
    assert!(uri_custom.contains("/nord-theme/"), "gallery icon URI must contain custom palette id /nord-theme/, got {uri_custom}");
    assert!(uri_custom.contains(&custom_tag), "gallery icon URI must contain custom palette fingerprint {custom_tag}, got {uri_custom}");
    assert_ne!(uri_light, uri_custom);

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn duplicate_theme_id_marks_subsequent_pack_with_conflict_and_prevents_activation() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_dup_test_{}", std::process::id()));
    let dir1 = temp_root.join("dir1");
    let dir2 = temp_root.join("dir2");
    let pack1 = dir1.join("pack1");
    let pack2 = dir2.join("pack2");
    std::fs::create_dir_all(pack1.join("icons/sketch")).unwrap();
    std::fs::create_dir_all(pack2.join("icons/sketch")).unwrap();

    let manifest1 = r#"(
            package_type: IconTheme,
            id: "dup-theme-test",
            name: "First Theme",
            version: "1.0.0",
            author: "Author 1",
            license: "MIT",
        )"#;
    let manifest2 = r#"(
            package_type: IconTheme,
            id: "dup-theme-test",
            name: "Second Theme",
            version: "2.0.0",
            author: "Author 2",
            license: "MIT",
        )"#;
    std::fs::write(pack1.join("manifest.ron"), manifest1).unwrap();
    std::fs::write(pack2.join("manifest.ron"), manifest2).unwrap();

    let dirs = vec![dir1, dir2];
    let report = discover_all_theme_packs(&dirs);

    let dup_packs: Vec<_> = report.packs.into_iter().filter(|p| p.manifest.id == "dup-theme-test").collect();
    assert_eq!(dup_packs.len(), 2, "both packs must be returned so user sees the conflict in UI");
    assert!(!dup_packs[0].has_id_conflict(), "first pack top-to-bottom must be valid");
    assert!(dup_packs[1].has_id_conflict(), "second pack with identical id must be marked with conflict");
    let conflict = dup_packs[1].duplicate_conflict.as_ref().unwrap();
    assert_eq!(conflict.conflicting_id, "dup-theme-test");
    assert!(!conflict.is_default_theme);

    // Verify UI rendering for the conflicting pack
    let mut app = crate::gui::App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    open_icon_manager(&ctx);

    let conflict_key = dup_packs[1].selection_key();
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<IconManagerState>(egui::Id::new("icon_manager_window"));
        state.selected_pack_id = conflict_key.clone();
    });

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut texts = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let out = ctx.run_ui(input, |ui| draw_icon_manager_window_in_dirs(ui.ctx(), &mut app.win_ctx(&mut Vec::new()), &dirs));
        texts.clear();
        for shape in &out.shapes {
            crate::gui::screen_keys::tests::collect_text(&shape.shape, &mut texts);
        }
    }
    let all_text = texts.join(" ");
    assert!(all_text.contains(&crate::i18n::tr("icontheme-mgr-conflict-badge")), "detail panel must show conflict badge for duplicate theme");
    assert!(all_text.contains(&crate::i18n::tr("icontheme-mgr-conflict-title")), "detail panel must display conflict title explaining the duplicate ID");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn custom_theme_with_default_theme_id_is_marked_as_conflict() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_default_override_test_{}", std::process::id()));
    let pack_dir = temp_root.join("rogue-default");
    std::fs::create_dir_all(pack_dir.join("icons/sketch")).unwrap();

    let manifest = format!(
        r#"(
            package_type: IconTheme,
            id: "{DEFAULT_THEME_ID}",
            name: "Fake Default Theme",
            version: "9.9.9",
            author: "Intruder",
            license: "MIT",
        )"#
    );
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();

    let dirs = vec![temp_root.clone()];
    let report = discover_all_theme_packs(&dirs);

    let custom_default = report.packs.iter().find(|p| p.manifest.name == "Fake Default Theme").expect("custom pack discovered");
    assert!(custom_default.has_id_conflict(), "custom pack using DEFAULT_THEME_ID must be marked with conflict");
    let conflict = custom_default.duplicate_conflict.as_ref().unwrap();
    assert_eq!(conflict.conflicting_id, DEFAULT_THEME_ID);
    assert!(conflict.is_default_theme, "must be marked specifically as default theme conflict");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn discover_all_theme_packs_serves_cached_builtin_and_disk_packs_on_subsequent_calls() {
    let _lock = lock_theme_test();
    let temp_root = std::env::temp_dir().join(format!("qymcad_discover_cache_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let pack_dir = temp_root.join("test-cache-pack");
    std::fs::create_dir_all(pack_dir.join("icons/sketch")).unwrap();

    let manifest = r#"(
        id: "test-cache-pack",
        name: "Test Cache Pack",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(pack_dir.join("manifest.ron"), manifest).unwrap();

    let dirs = vec![temp_root.clone()];
    clear_discovery_cache_for_test();

    let first = discover_all_theme_packs(&dirs);
    assert!(first.packs.iter().any(|p| p.manifest.id == "test-cache-pack"));
    assert!(first.packs.iter().any(|p| p.manifest.id == DEFAULT_THEME_ID));

    // Second call without directory modification must return matching pack list
    let second = discover_all_theme_packs(&dirs);
    assert_eq!(first.packs.len(), second.packs.len());
    for (p1, p2) in first.packs.iter().zip(second.packs.iter()) {
        assert_eq!(p1.manifest.id, p2.manifest.id);
        assert_eq!(p1.format(), p2.format());
    }

    let _ = std::fs::remove_dir_all(&temp_root);
}
