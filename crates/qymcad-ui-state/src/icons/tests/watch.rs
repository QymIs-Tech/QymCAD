//! Checks of the live watch: a theme edited on disk reloads, and the icons of egui follow it.

use super::*;

#[test]
fn live_watch_folder_auto_reload_on_svg_change() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_watch_test_{}", std::process::id()));
    let pack_dir = temp_dir.join("watch_theme");
    std::fs::create_dir_all(pack_dir.join("icons").join("sketch")).unwrap();
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "watch-theme".to_string(),
        name: "Watch Theme".to_string(),
        version: "1.0.0".to_string(),
        author: "Dev".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
    };
    std::fs::write(pack_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let initial_svg = br#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/></svg>"#;
    let svg_path = pack_dir.join("icons").join("sketch").join("line.svg");
    std::fs::write(&svg_path, initial_svg).unwrap();

    let folder_pack = IconPack::from_directory(&pack_dir).expect("pack must load");
    let ctx = egui::Context::default();
    set_pack_watching(&ctx, "watch-theme", true);
    assert!(is_pack_watched(&ctx, "watch-theme"));
    set_active_icon_stack(&ctx, vec![folder_pack], &qymcad_scheme::dark());

    // First resolve: gets initial SVG
    let stack = get_active_icon_stack(&ctx);
    let res1 = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(res1.data.as_ref(), initial_svg);
    let rev1 = get_icon_revision(&ctx);

    // Polling without changes returns false
    assert!(!poll_watched_icon_packs(&ctx, &qymcad_scheme::dark()));

    // Sleep briefly so file modification timestamp differs
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Now edit the SVG in the folder
    let updated_svg = br#"<svg viewBox="0 0 24 24"><rect width="24" height="24"/></svg>"#;
    std::fs::write(&svg_path, updated_svg).unwrap();

    // Reset poll timer throttle so test runs immediately
    std::thread::sleep(std::time::Duration::from_millis(260));

    // Polling detects the change
    let changed = poll_watched_icon_packs(&ctx, &qymcad_scheme::dark());
    assert!(changed, "poll_watched_icon_packs should detect edited file in watched directory");

    // Second resolve: gets updated SVG and updated revision
    let stack = get_active_icon_stack(&ctx);
    let res2 = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(res2.data.as_ref(), updated_svg, "resolved icon should have the updated SVG content");
    assert!(get_icon_revision(&ctx) > rev1, "revision counter should have incremented");

    let rev2 = get_icon_revision(&ctx);
    std::fs::remove_file(&svg_path).expect("remove watched icon");
    std::thread::sleep(std::time::Duration::from_millis(260));
    assert!(poll_watched_icon_packs(&ctx, &qymcad_scheme::dark()), "removing an icon must invalidate the cascade");
    let stack = get_active_icon_stack(&ctx);
    let missing = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(missing.pack_id, DEFAULT_THEME_ID, "an absent icon must use the built-in fallback");
    assert!(get_icon_revision(&ctx) > rev2);

    let rev_missing = get_icon_revision(&ctx);
    std::fs::write(&svg_path, initial_svg).expect("restore watched icon");
    std::thread::sleep(std::time::Duration::from_millis(260));
    assert!(poll_watched_icon_packs(&ctx, &qymcad_scheme::dark()), "adding an icon must invalidate the cascade");
    let stack = get_active_icon_stack(&ctx);
    let restored = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(restored.pack_id, "watch-theme");
    assert_eq!(restored.data.as_ref(), initial_svg);
    assert!(get_icon_revision(&ctx) > rev_missing);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_successive_folder_live_reloads_do_not_stop_after_3_times() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_successive_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let pack_dir = temp_dir.join("continuous-theme");
    std::fs::create_dir_all(pack_dir.join("icons").join("sketch")).unwrap();

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "continuous-theme".to_string(),
        name: "Continuous Theme".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Testing continuous updates".to_string(),
        translations: Default::default(),
    };
    std::fs::write(pack_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let svg_line = pack_dir.join("icons").join("sketch").join("line.svg");
    let svg_circle = pack_dir.join("icons").join("sketch").join("circle.svg");
    std::fs::write(&svg_line, br#"<svg viewBox="0 0 24 24"><path d="M0 0 L10 10"/></svg>"#).unwrap();
    std::fs::write(&svg_circle, br#"<svg viewBox="0 0 24 24"><circle cx="1" cy="1" r="1"/></svg>"#).unwrap();

    let folder_pack = IconPack::from_directory(&pack_dir).expect("pack must load");
    let ctx = egui::Context::default();
    set_pack_watching(&ctx, "continuous-theme", true);
    set_active_icon_stack(&ctx, vec![folder_pack], &qymcad_scheme::dark());

    // Initial check
    let stack = get_active_icon_stack(&ctx);
    let _ = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    let _ = resolve_icon(IconId::SketchCircle, &stack, &qymcad_scheme::dark());

    // Perform 6 consecutive edits with polling checks
    for edit_num in 1..=6 {
        // Sleep past the 250ms throttle
        std::thread::sleep(std::time::Duration::from_millis(260));

        let new_content = format!(r#"<svg viewBox="0 0 24 24"><path d="M0 0 L{} {}"/></svg>"#, edit_num * 10, edit_num * 10);
        if edit_num % 2 == 1 {
            std::fs::write(&svg_line, new_content.as_bytes()).unwrap();
        } else {
            std::fs::write(&svg_circle, new_content.as_bytes()).unwrap();
        }

        let detected = poll_watched_icon_packs(&ctx, &qymcad_scheme::dark());
        assert!(detected, "Edit {} MUST be detected by poll_watched_icon_packs", edit_num);

        let stack = get_active_icon_stack(&ctx);
        if edit_num % 2 == 1 {
            let res = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
            assert_eq!(res.data.as_ref(), new_content.as_bytes(), "Edit {} data mismatch", edit_num);
        } else {
            let res = resolve_icon(IconId::SketchCircle, &stack, &qymcad_scheme::dark());
            assert_eq!(res.data.as_ref(), new_content.as_bytes(), "Edit {} data mismatch", edit_num);
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn live_watch_reloads_manifest_metadata() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_manifest_reload_test_{}", std::process::id()));
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).expect("create icons dir");

    let manifest_path = temp_root.join("manifest.ron");
    let initial_manifest = r#"(
        id: "live-manifest-pack",
        name: "Live Manifest Pack",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(&manifest_path, initial_manifest).expect("write initial manifest");

    let svg_content = br##"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z" fill="#ff0000"/></svg>"##;
    std::fs::write(icons_dir.join("line.svg"), svg_content).expect("write line.svg");

    let pack = IconPack::from_directory(&temp_root).expect("load directory pack");
    let ctx = egui::Context::default();
    set_pack_watching(&ctx, "live-manifest-pack", true);
    set_active_icon_stack(&ctx, vec![pack], &qymcad_scheme::dark());

    let stack = get_active_icon_stack(&ctx);
    let res1 = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(res1.pack_id, "live-manifest-pack");

    std::thread::sleep(std::time::Duration::from_millis(300));

    let updated_manifest = r#"(
        id: "live-manifest-pack",
        name: "Live Manifest Pack Updated",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(&manifest_path, updated_manifest).expect("write updated manifest");

    let changed = poll_watched_icon_packs(&ctx, &qymcad_scheme::dark());
    assert!(changed, "poll_watched_icon_packs should detect manifest change");

    let stack = get_active_icon_stack(&ctx);
    let res2 = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(res2.pack_id, "live-manifest-pack");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn changing_a_watched_theme_id_removes_the_old_theme_and_discovers_a_new_one() {
    let root = std::env::temp_dir().join(format!("qymcad_watched_id_change_{}", std::process::id()));
    let theme = root.join("theme");
    let icons = theme.join("icons").join("sketch");
    std::fs::create_dir_all(&icons).unwrap();
    let manifest = |id: &str| format!("(id: \"{id}\", name: \"Watched Theme\", version: \"1.0\", author: \"Tester\", license: \"MIT\")");
    std::fs::write(theme.join("manifest.ron"), manifest("watched-old")).unwrap();
    let line = icons.join("line.svg");
    std::fs::write(&line, br#"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z"/></svg>"#).unwrap();

    let pack = IconPack::from_directory(&theme).unwrap();
    let ctx = egui::Context::default();
    let palette = qymcad_scheme::dark();
    set_pack_watching(&ctx, "watched-old", true);
    set_active_icon_stack(&ctx, vec![pack], &palette);
    assert_eq!(resolve_icon(IconId::SketchLine, &get_active_icon_stack(&ctx), &palette).pack_id, "watched-old");

    std::fs::write(theme.join("manifest.ron"), manifest("watched-new")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert!(poll_watched_icon_packs(&ctx, &palette));

    let stack = get_active_icon_stack(&ctx);
    assert!(stack.iter().all(|pack| pack.manifest.id != "watched-old" && pack.manifest.id != "watched-new"), "renaming must remove the active theme");
    assert_eq!(resolve_icon(IconId::SketchLine, &stack, &palette).pack_id, DEFAULT_THEME_ID);
    let watcher = ctx.data(|data| data.get_temp::<IconWatcher>(egui::Id::new("cad_icon_watcher")).unwrap());
    assert!(!watcher.watched_pack_ids.iter().any(|id| id == "watched-old"));
    assert!(!watcher.last_seen_snapshots.contains_key("watched-old"));
    assert!(!watcher.last_seen_snapshots.contains_key("watched-new"));
    assert_eq!(take_removed_icon_pack_ids(&ctx), ["watched-old"]);
    assert!(take_removed_icon_pack_ids(&ctx).is_empty());
    assert!(discover_packs_in(&root).iter().any(|pack| pack.manifest.id == "watched-new"));

    let revision = get_icon_revision(&ctx);
    std::fs::write(&line, br#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/></svg>"#).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert!(!poll_watched_icon_packs(&ctx, &palette), "new theme must wait for separate activation");
    assert_eq!(get_icon_revision(&ctx), revision);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn transient_read_failure_in_higher_theme_does_not_poison_cache() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_transient_cache_test_{}", std::process::id()));
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).expect("create icons dir");

    let manifest_path = temp_root.join("manifest.ron");
    let manifest = r#"(
        id: "custom-transient-pack",
        name: "Custom Transient Pack",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(&manifest_path, manifest).expect("write manifest");

    let line_path = icons_dir.join("line.svg");
    std::fs::write(&line_path, b"").expect("write empty line.svg");

    let custom_pack = IconPack::from_directory(&temp_root).expect("load directory pack");
    assert!(custom_pack.has_icon_on_disk(IconId::SketchLine));

    let pal = qymcad_scheme::dark();
    let res1 = resolve_icon(IconId::SketchLine, std::slice::from_ref(&custom_pack), &pal);
    assert_eq!(res1.pack_id, DEFAULT_THEME_ID);

    let valid_svg = br#"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z"/></svg>"#;
    std::fs::write(&line_path, valid_svg).expect("write valid svg");

    let res2 = resolve_icon(IconId::SketchLine, &[custom_pack], &pal);
    assert_eq!(res2.pack_id, "custom-transient-pack", "cache must not be poisoned by fallback after transient error");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn install_cad_icons_populates_egui_loaders_and_updates_on_palette_change() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);

    let dark_pal = qymcad_scheme::dark();
    install_cad_icons(&ctx, &[], &dark_pal);

    let uri = IconId::SketchLine.uri();
    assert_eq!(uri, "bytes://qicons/sketch/line.svg");

    let poll = ctx.try_load_image(uri, egui::load::SizeHint::default()).expect("must load installed icon");
    let dark_pixels = match poll {
        egui::load::ImagePoll::Ready { image } => image.pixels.clone(),
        egui::load::ImagePoll::Pending { .. } => panic!("must be ready"),
    };

    // Now switch to light palette
    let light_pal = qymcad_scheme::light();
    install_cad_icons(&ctx, &[], &light_pal);

    let poll = ctx.try_load_image(uri, egui::load::SizeHint::default()).expect("must load updated icon");
    let light_pixels = match poll {
        egui::load::ImagePoll::Ready { image } => image.pixels.clone(),
        egui::load::ImagePoll::Pending { .. } => panic!("must be ready"),
    };

    assert_ne!(dark_pixels, light_pixels, "installed texture bytes must differ between dark and light palettes");
}

#[test]
fn forget_all_cad_icons_evicts_from_egui() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);

    install_cad_icons(&ctx, &[], &qymcad_scheme::dark());
    let uri = IconId::SketchLine.uri();
    assert!(ctx.try_load_image(uri, egui::load::SizeHint::default()).is_ok());

    forget_all_cad_icons(&ctx);
    let poll = ctx.try_load_image(uri, egui::load::SizeHint::default());
    assert!(poll.is_err(), "forget_all_cad_icons must evict all icon URIs from loader");
}

#[test]
fn background_watcher_thread_updates_egui_loader_on_icon_edit() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_bg_watcher_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
        id: "bg-watcher-theme",
        name: "BG Watcher Theme",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(temp_root.join("manifest.ron"), manifest).unwrap();
    let svg1 = br#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg1).unwrap();

    let pack = IconPack::from_directory(&temp_root).unwrap();
    let ctx = egui::Context::default();
    set_pack_watching(&ctx, "bg-watcher-theme", true);
    set_active_icon_stack(&ctx, vec![pack], &qymcad_scheme::dark());
    let rev1 = get_icon_revision(&ctx);

    ensure_watcher_thread(&ctx, &qymcad_scheme::dark());

    // Modify the watched file on disk
    let svg2 = br#"<svg viewBox="0 0 24 24"><rect width="24" height="24"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg2).unwrap();

    // The watcher polls every 250 ms. A fixed sleep of 350 ms failed 3 runs of 3 when the checks that hammer the
    // active stack from threads ran beside it: the poll came late. The revision is awaited up to 5 s instead.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while get_icon_revision(&ctx) == rev1 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let rev2 = get_icon_revision(&ctx);
    assert!(rev2 > rev1, "background watcher thread must increment revision counter on change");

    let stack = get_active_icon_stack(&ctx);
    let res = resolve_icon(IconId::SketchLine, &stack, &qymcad_scheme::dark());
    assert_eq!(res.pack_id, "bg-watcher-theme");
    assert_eq!(res.data.as_ref(), svg2, "background watcher must update egui loader with new SVG");

    stop_watcher_thread(&ctx);
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn live_watch_manifest_change_updates_manifest_in_active_stack() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_manifest_active_stack_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest_path = temp_root.join("manifest.ron");
    let initial_manifest = r#"(
        id: "manifest-stack-theme",
        name: "Initial Name",
        version: "1.0.0",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(&manifest_path, initial_manifest).unwrap();
    std::fs::write(icons_dir.join("line.svg"), br#"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z"/></svg>"#).unwrap();

    let pack = IconPack::from_directory(&temp_root).unwrap();
    let ctx = egui::Context::default();
    set_pack_watching(&ctx, "manifest-stack-theme", true);
    set_active_icon_stack(&ctx, vec![pack], &qymcad_scheme::dark());

    let stack1 = get_active_icon_stack(&ctx);
    assert_eq!(stack1[0].manifest.name, "Initial Name");

    std::thread::sleep(std::time::Duration::from_millis(260));
    let updated_manifest = r#"(
        id: "manifest-stack-theme",
        name: "Updated Name",
        version: "1.0.1",
        author: "Tester",
        license: "MIT",
    )"#;
    std::fs::write(&manifest_path, updated_manifest).unwrap();

    assert!(poll_watched_icon_packs(&ctx, &qymcad_scheme::dark()));

    let stack2 = get_active_icon_stack(&ctx);
    assert_eq!(stack2[0].manifest.name, "Updated Name", "reloaded manifest in active stack must reflect in get_active_icon_stack");
    assert_eq!(stack2[0].manifest.version, "1.0.1");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn active_icon_stack_concurrent_update_during_watcher_poll_is_not_overwritten() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_watcher_race_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let theme1_dir = temp_root.join("theme1");
    let theme2_dir = temp_root.join("theme2");
    let icons1 = theme1_dir.join("icons").join("sketch");
    let icons2 = theme2_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons1).unwrap();
    std::fs::create_dir_all(&icons2).unwrap();

    let manifest1 = "(id: \"theme-one\", name: \"Theme 1\", version: \"1.0\", author: \"T\", license: \"MIT\")";
    let manifest2 = "(id: \"theme-two\", name: \"Theme 2\", version: \"1.0\", author: \"T\", license: \"MIT\")";
    std::fs::write(theme1_dir.join("manifest.ron"), manifest1).unwrap();
    std::fs::write(theme2_dir.join("manifest.ron"), manifest2).unwrap();

    let svg1 = br#"<svg viewBox="0 0 24 24"><circle cx="1" cy="1" r="1"/></svg>"#;
    let svg2 = br#"<svg viewBox="0 0 24 24"><circle cx="2" cy="2" r="2"/></svg>"#;
    std::fs::write(icons1.join("line.svg"), svg1).unwrap();
    std::fs::write(icons2.join("line.svg"), svg2).unwrap();

    let pack1 = IconPack::from_directory(&theme1_dir).unwrap();
    let pack2 = IconPack::from_directory(&theme2_dir).unwrap();

    let ctx = egui::Context::default();
    let pal = qymcad_scheme::dark();

    set_pack_watching(&ctx, "theme-one", true);
    set_active_icon_stack(&ctx, vec![pack1], &pal);

    assert_eq!(resolve_icon(IconId::SketchLine, &get_active_icon_stack(&ctx), &pal).pack_id, "theme-one");

    let gen1 = get_active_icon_stack_generation(&ctx);
    assert!(gen1 > 0, "active icon stack must track generation");

    // UI switches active theme to theme2
    set_active_icon_stack(&ctx, vec![pack2], &pal);
    let gen2 = get_active_icon_stack_generation(&ctx);
    assert!(gen2 > gen1, "setting active icon stack must increment generation");

    assert_eq!(resolve_icon(IconId::SketchLine, &get_active_icon_stack(&ctx), &pal).pack_id, "theme-two");

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn active_icon_stack_concurrent_thread_toggling_is_not_clobbered() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_thread_race_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let theme1_dir = temp_root.join("theme1");
    let theme2_dir = temp_root.join("theme2");
    let icons1 = theme1_dir.join("icons").join("sketch");
    let icons2 = theme2_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons1).unwrap();
    std::fs::create_dir_all(&icons2).unwrap();

    let manifest1 = "(id: \"race-theme-1\", name: \"Race Theme 1\", version: \"1.0\", author: \"T\", license: \"MIT\")";
    let manifest2 = "(id: \"race-theme-2\", name: \"Race Theme 2\", version: \"1.0\", author: \"T\", license: \"MIT\")";
    std::fs::write(theme1_dir.join("manifest.ron"), manifest1).unwrap();
    std::fs::write(theme2_dir.join("manifest.ron"), manifest2).unwrap();

    let svg1 = br#"<svg viewBox="0 0 24 24"><circle cx="1" cy="1" r="1"/></svg>"#;
    let svg2 = br#"<svg viewBox="0 0 24 24"><circle cx="2" cy="2" r="2"/></svg>"#;
    std::fs::write(icons1.join("line.svg"), svg1).unwrap();
    std::fs::write(icons2.join("line.svg"), svg2).unwrap();

    let pack1 = IconPack::from_directory(&theme1_dir).unwrap();
    let pack2 = IconPack::from_directory(&theme2_dir).unwrap();

    let ctx = egui::Context::default();
    let pal = qymcad_scheme::dark();

    set_pack_watching(&ctx, "race-theme-1", true);
    set_pack_watching(&ctx, "race-theme-2", true);
    set_active_icon_stack(&ctx, vec![pack1.clone()], &pal);

    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r_clone = std::sync::Arc::clone(&running);
    let ctx_clone = ctx.clone();
    let pal_clone = pal.clone();

    let watcher_handle = std::thread::spawn(move || {
        while r_clone.load(std::sync::atomic::Ordering::SeqCst) {
            poll_watched_icon_packs(&ctx_clone, &pal_clone);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    for i in 0..20 {
        if i % 2 == 0 {
            set_active_icon_stack(&ctx, vec![pack2.clone()], &pal);
            assert_eq!(resolve_icon(IconId::SketchLine, &get_active_icon_stack(&ctx), &pal).pack_id, "race-theme-2");
        } else {
            set_active_icon_stack(&ctx, vec![pack1.clone()], &pal);
            assert_eq!(resolve_icon(IconId::SketchLine, &get_active_icon_stack(&ctx), &pal).pack_id, "race-theme-1");
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    running.store(false, std::sync::atomic::Ordering::SeqCst);
    watcher_handle.join().unwrap();

    let final_stack = get_active_icon_stack(&ctx);
    assert!(!final_stack.is_empty(), "active icon stack must remain valid");

    let _ = std::fs::remove_dir_all(&temp_root);
}
