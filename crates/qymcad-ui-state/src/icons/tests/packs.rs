//! Checks of the packs found on disk and built in: the cascade of fallbacks, discovery, coverage and the help.

use super::*;

#[test]
fn missing_or_invalid_pack_icon_uses_default() {
    let dir = std::env::temp_dir().join(format!("qymcad_pack_icon_fallback_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("creates pack directory");
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "icon-fallback".into(),
        name: "Icon fallback".into(),
        version: "1.0.0".into(),
        author: String::new(),
        license: "MIT".into(),
        description: String::new(),
        translations: Default::default(),
    };
    std::fs::write(dir.join("manifest.ron"), manifest.to_ron().expect("manifest serializes")).expect("writes manifest");
    let pack = IconPack::from_directory(&dir).expect("folder loads");
    let default_icon = load_default_pack().expect("default loads").get_pack_icon_svg();
    assert_eq!(pack.get_pack_icon_svg(), default_icon);

    std::fs::write(dir.join("icon.svg"), b"<svg><script/></svg>").expect("writes invalid icon");
    assert_eq!(pack.get_pack_icon_svg(), default_icon, "unsafe icons must not reach the UI");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn directory_preview_is_size_limited_and_svg_validated() {
    let dir = std::env::temp_dir().join(format!("qymcad_preview_validation_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("creates pack directory");
    std::fs::create_dir_all(dir.join("icons")).expect("creates icons directory");
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "preview-validation".into(),
        name: "Preview validation".into(),
        version: "1.0.0".into(),
        author: String::new(),
        license: "MIT".into(),
        description: String::new(),
        translations: Default::default(),
    };
    std::fs::write(dir.join("manifest.ron"), manifest.to_ron().expect("manifest serializes")).expect("writes manifest");
    let pack = IconPack::from_directory(&dir).expect("folder loads");

    std::fs::write(dir.join("preview.svg"), br#"<svg viewBox="0 0 120 60"><rect width="120" height="60"/></svg>"#).expect("writes valid artwork preview");
    assert_eq!(pack.get_preview_image().expect("non-square artwork is allowed").extension, "svg");
    let report = inspect_pack_directory(&dir).expect("valid preview pack is inspected");
    assert!(!report.rejected.iter().any(|entry| entry.path == "preview.svg"));

    std::fs::write(dir.join("preview.svg"), br#"<svg viewBox="0 0 120 60"><use href="https://example.test/a.svg#shape"/></svg>"#).expect("writes unsafe preview");
    assert!(pack.get_preview_image().is_none(), "external references must be rejected in previews");
    let report = inspect_pack_directory(&dir).expect("unsafe preview pack is inspected");
    assert!(report.rejected.iter().any(|entry| entry.path == "preview.svg"), "unsafe SVG previews must be reported");

    let oversized_svg = std::fs::File::create(dir.join("preview.svg")).expect("creates oversized SVG");
    oversized_svg.set_len(MAX_SINGLE_FILE_UNCOMPRESSED_SIZE + 1).expect("sizes SVG over the preview limit");
    assert!(pack.get_preview_image().is_none(), "oversized SVG previews must not be read");
    let report = inspect_pack_directory(&dir).expect("oversized preview pack is inspected");
    assert!(report.rejected.iter().any(|entry| entry.path == "preview.svg"), "oversized SVG previews must be reported");

    let oversized_raster = std::fs::File::create(dir.join("preview.png")).expect("creates oversized raster");
    oversized_raster.set_len(MAX_SINGLE_FILE_UNCOMPRESSED_SIZE + 1).expect("sizes raster over the preview limit");
    assert!(pack.get_preview_image().is_none(), "oversized raster previews must not be read");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn default_embedded_pack_is_valid_and_complete() {
    let pack = load_default_pack().expect("embedded default theme must load cleanly");
    assert_eq!(pack.manifest.id, DEFAULT_THEME_ID);
    assert!(pack.manifest.translations.contains_key("kk"));
    assert!(pack.manifest.translations.contains_key("ru"));
    assert!(pack.manifest.translations.contains_key("uk"));
    assert_ne!(pack.manifest.description_for_locale("kk"), pack.manifest.description);
    assert_ne!(pack.manifest.description_for_locale("ru"), pack.manifest.description);
    assert_ne!(pack.manifest.description_for_locale("uk"), pack.manifest.description);
    assert_ne!(pack.get_readme_for_locale("kk"), pack.get_readme());
    assert_ne!(pack.get_readme_for_locale("ru"), pack.get_readme());
    assert_ne!(pack.get_readme_for_locale("uk"), pack.get_readme());
    let default_theme_entry = BUILTIN_ICON_THEMES.iter().find(|t| t.id == DEFAULT_THEME_ID).expect("default builtin theme must exist in BUILTIN_ICON_THEMES");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(default_theme_entry.archive)).expect("embedded bundle reads");
    assert!(archive.by_name("icon.svg").is_ok(), "default icon must be stored beside manifest.ron");
    validate_svg(&pack.get_pack_icon_svg()).expect("default pack icon is a valid SVG");

    // Check coverage of all known IconIds
    let cov = pack.coverage();
    assert_eq!(cov.present, cov.total, "embedded default theme must cover 100% of icons (got {}/{})", cov.present, cov.total);

    // Verify SVG data is valid for every single icon
    for id in ALL_ICONS {
        let svg = pack.get_svg_for_id(*id).expect("must have SVG");
        validate_svg(&svg).expect("default SVG must pass monochrome validation");
    }

    // Verify pack README is loaded
    let readme = pack.get_readme();
    assert!(readme.contains("Default"), "embedded README should be available");
}

#[test]
fn all_embedded_packs_are_valid_and_complete() {
    let packs = load_builtin_packs();
    assert!(!packs.is_empty(), "must load at least default pack; got {}", packs.len());

    let default_pack = packs.iter().find(|p| p.manifest.id == DEFAULT_THEME_ID).expect("default pack exists");
    assert_eq!(default_pack.format(), BundleFormat::Embedded);
    assert!(!default_pack.is_directory());
    assert!(default_pack.checksum_matches());
    assert!(default_pack.manifest.translations.contains_key("kk"));
    assert!(default_pack.manifest.translations.contains_key("ru"));
    assert!(default_pack.manifest.translations.contains_key("uk"));
    assert_ne!(default_pack.manifest.description_for_locale("kk"), default_pack.manifest.description);
    assert_ne!(default_pack.manifest.description_for_locale("ru"), default_pack.manifest.description);
    assert_ne!(default_pack.manifest.description_for_locale("uk"), default_pack.manifest.description);
    assert_ne!(default_pack.get_readme_for_locale("kk"), default_pack.get_readme());
    assert_ne!(default_pack.get_readme_for_locale("ru"), default_pack.get_readme());
    assert_ne!(default_pack.get_readme_for_locale("uk"), default_pack.get_readme());
    let (def_cov, total) = (default_pack.coverage().present, default_pack.coverage().total);
    assert_eq!(def_cov, total, "embedded default pack must cover all icons");

    for pack in packs {
        assert_eq!(pack.format(), BundleFormat::Embedded, "every built-in pack must have Embedded format");
        assert!(!pack.is_directory(), "embedded pack must not be reported as a directory");
        assert!(pack.checksum_matches(), "embedded archive checksum must match");
        validate_svg(&pack.get_pack_icon_svg()).expect("embedded pack icon is valid SVG");
    }
}

#[test]
fn icon_cascade_resolution() {
    let pal = qymcad_scheme::dark();
    // Initial resolution with default pack resolves to default embedded SVG
    let initial = resolve_icon(IconId::SketchLine, &[], &pal);
    assert_eq!(initial.pack_id, DEFAULT_THEME_ID);

    // Now push custom pack on top
    let mut map = HashMap::new();
    let custom_svg = br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#.to_vec();
    map.insert("icons/sketch/line.svg".to_string(), custom_svg.clone());
    let custom_pack = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "pack-custom".to_string(),
            name: "Pack Custom".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            description: "Test".to_string(),
            translations: Default::default(),
        },
        source: PackSource::Memory(map),
        trailer_check: None,
        duplicate_conflict: None,
    };

    let resolved = resolve_icon(IconId::SketchLine, &[custom_pack], &pal);
    assert_eq!(resolved.data.as_ref(), &custom_svg[..]);
    assert_eq!(resolved.pack_id, "pack-custom");
}

#[test]
fn icon_tool_renders_with_icon_id() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _ = crate::icon_tool(ui, IconId::SketchLine, "Line tool", true);
    });

    let uri = IconId::SketchLine.uri();
    let poll = ctx.try_load_image(uri, egui::load::SizeHint::default()).expect("icon image must be loaded by installed loader");
    match poll {
        egui::load::ImagePoll::Ready { image } => {
            assert!(image.size[0] > 0 && image.size[1] > 0, "rasterized icon must have positive dimensions");
            assert!(image.pixels.iter().any(|p| p.a() > 0), "rasterized icon must contain visible pixels");
        }
        egui::load::ImagePoll::Pending { .. } => panic!("in-memory SVG bytes must load synchronously into Ready state"),
    }
}

#[test]
fn discover_packs_in_directory() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_discover_test_{}", std::process::id()));
    let theme_a = temp_dir.join("theme_a");
    std::fs::create_dir_all(theme_a.join("icons").join("sketch")).unwrap();
    let manifest_a = IconManifest {
        package_type: PackageType::IconTheme,
        id: "theme-a".to_string(),
        name: "Theme A".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
    };
    std::fs::write(theme_a.join("manifest.ron"), manifest_a.to_ron().unwrap()).unwrap();
    std::fs::write(theme_a.join("icons").join("sketch").join("line.svg"), br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#).unwrap();

    let packs = discover_packs_in(&temp_dir);
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].manifest.id, "theme-a");

    let resolved = resolve_icon(IconId::SketchLine, &packs, &qymcad_scheme::dark());
    assert_eq!(resolved.pack_id, "theme-a");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn repo_default_theme_if_present_loads_and_has_icons() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let default_dir = manifest_dir.join("../../assets/icon-themes/default");
    if default_dir.is_dir() {
        let pack = IconPack::from_directory(&default_dir).expect("default directory must load cleanly");
        assert_eq!(pack.manifest.id, DEFAULT_THEME_ID);
        assert_eq!(pack.manifest.name, "Default");
        let cov = pack.coverage();
        assert_eq!(cov.present, cov.total, "default theme in repo should cover all icons, got {}/{}", cov.present, cov.total);
    }
}

#[test]
fn missing_directory_icons_do_not_block_coverage() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_missing_icons_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create theme directory");
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "missing-icons".to_string(),
        name: "Missing Icons".to_string(),
        version: "1.0.0".to_string(),
        author: "Test".to_string(),
        license: "MIT".to_string(),
        description: String::new(),
        translations: Default::default(),
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().expect("serialize manifest")).expect("write manifest");
    let pack = IconPack::from_directory(&temp_dir).expect("load directory pack");
    let start = std::time::Instant::now();
    let coverage = pack.coverage();
    let elapsed = start.elapsed();
    assert_eq!(coverage, CoverageCount { present: 0, total: ALL_ICONS.len() });
    assert!(elapsed < std::time::Duration::from_millis(300), "checking 106 absent SVG files blocked the frame for {elapsed:?}");
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn directory_icon_exceeding_max_svg_size_is_not_loaded() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_svg_size_test_{}", std::process::id()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "oversized-svg-theme".into(),
        name: "Oversized SVG Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Test".into(),
        translations: Default::default(),
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    // Create a file larger than MAX_ICON_SVG_SIZE (512 KB)
    let file_path = icons_dir.join("line.svg");
    let file = std::fs::File::create(&file_path).unwrap();
    file.set_len(MAX_ICON_SVG_SIZE + 1024).unwrap();

    let pack = IconPack::from_directory(&temp_dir).unwrap();
    let loaded = pack.get_svg("sketch/line");
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(loaded.is_none(), "SVG exceeding MAX_ICON_SVG_SIZE should not be loaded");
}

#[test]
fn invalid_icon_in_custom_pack_continues_fallback_to_default() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_invalid_svg_fallback_{}", std::process::id()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "invalid-svg-theme".into(),
        name: "Invalid SVG Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Test".into(),
        translations: Default::default(),
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    // Write invalid SVG content
    std::fs::write(icons_dir.join("line.svg"), b"not SVG at all").unwrap();

    let pack = IconPack::from_directory(&temp_dir).unwrap();

    // 1. Pack itself must not return invalid SVG
    assert_eq!(pack.get_svg_for_id(IconId::SketchLine), None);

    // 2. Cascade in manager must fall back to default embedded theme
    let resolved = resolve_icon(IconId::SketchLine, &[pack], &qymcad_scheme::dark());
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(resolved.pack_id, DEFAULT_THEME_ID);
    assert!(resolved.data.starts_with(b"<svg"));
}

#[test]
fn custom_folder_with_special_id_is_still_directory_format() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_special_id_test_{}", std::process::id()));
    std::fs::create_dir_all(temp_dir.join("icons").join("sketch")).unwrap();

    // Create a directory pack that uses a custom id
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "custom-theme".into(),
        name: "My Custom Theme".into(),
        version: "1.0".into(),
        author: "Me".into(),
        license: "MIT".into(),
        description: "Test".into(),
        translations: Default::default(),
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    let pack = IconPack::from_directory(&temp_dir).unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Provenance must reflect physical source (Directory), NOT the theme ID
    assert_eq!(pack.format(), BundleFormat::Directory);
    assert!(pack.is_directory(), "a directory on disk must be recognized as directory regardless of ID");
}

#[test]
fn ensure_cad_icons_installed_lazily_populates_context() {
    let ctx = egui::Context::default();
    let id_key = egui::Id::new("cad_icons_installed");
    assert!(!ctx.data(|d| d.get_temp::<bool>(id_key).unwrap_or(false)));

    crate::icons::runtime::ensure_cad_icons_installed(&ctx);
    assert!(ctx.data(|d| d.get_temp::<bool>(id_key).unwrap_or(false)));
}

#[test]
fn discover_packs_detailed_reports_errors_for_corrupt_or_invalid_archives() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_discover_detailed_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let valid_theme = temp_dir.join("valid_theme");
    std::fs::create_dir_all(valid_theme.join("icons").join("sketch")).unwrap();
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-pack".to_string(),
        name: "Valid Pack".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
    };
    std::fs::write(valid_theme.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    let corrupt_qicons = temp_dir.join("corrupted.qicons");
    std::fs::write(&corrupt_qicons, b"not a zip archive at all").unwrap();

    let report = discover_packs_detailed(&temp_dir);
    assert_eq!(report.packs.len(), 1);
    assert_eq!(report.packs[0].manifest.id, "valid-pack");
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].path, corrupt_qicons);
    assert!(!report.errors[0].reason.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn icon_theme_documentation_matches_all_icons() {
    let readme_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon-themes/README.md");
    let content = std::fs::read_to_string(&readme_path).expect("assets/icon-themes/README.md must be readable");

    let mut documented_paths = std::collections::BTreeSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("| `") {
            if let Some(rest) = trimmed.strip_prefix("| `") {
                if let Some((path_with_ext, _)) = rest.split_once('`') {
                    if let Some(rel_path) = path_with_ext.strip_suffix(".svg") {
                        documented_paths.insert(rel_path.to_string());
                    }
                }
            }
        }
    }

    let supported_paths: std::collections::BTreeSet<String> = ALL_ICONS.iter().map(|id| id.relative_path().to_string()).collect();

    let extra: Vec<_> = documented_paths.difference(&supported_paths).collect();
    let missing: Vec<_> = supported_paths.difference(&documented_paths).collect();

    assert!(extra.is_empty(), "README documents non-existent/unsupported icons: {extra:?}");
    assert!(missing.is_empty(), "README is missing documented icons: {missing:?}");
    assert_eq!(documented_paths.len(), ALL_ICONS.len(), "Documented count must match ALL_ICONS count");
}

#[test]
fn pack_icon_svg_resolved_adapts_to_active_palette() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_pack_icon_palette_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_root).unwrap();

    let manifest = r#"(
        package_type: IconTheme,
        id: "test-pack-icon-tokens",
        name: "Test Pack Icon Tokens",
        version: "1.0.0",
        author: "Author",
        license: "MIT",
        description: "Test description",
    )"#;
    std::fs::write(temp_root.join("manifest.ron"), manifest).unwrap();
    let raw_svg = br#"<svg viewBox="0 0 24 24"><path d="M0 0h24v24z" stroke="var(--icon-stroke, #333)" fill="currentColor"/></svg>"#;
    std::fs::write(temp_root.join("icon.svg"), raw_svg).unwrap();

    let pack = IconPack::from_directory(&temp_root).unwrap();
    assert_eq!(pack.get_pack_icon_svg(), raw_svg, "get_pack_icon_svg returns raw SVG bytes");

    let dark_pal = qymcad_scheme::dark();
    let light_pal = qymcad_scheme::light();

    let dark_resolved = String::from_utf8(pack.get_pack_icon_svg_resolved(&dark_pal)).unwrap();
    let light_resolved = String::from_utf8(pack.get_pack_icon_svg_resolved(&light_pal)).unwrap();

    let dark_stroke = dark_pal.format_icon_color("icon-stroke").unwrap();
    let light_stroke = light_pal.format_icon_color("icon-stroke").unwrap();

    assert!(dark_resolved.contains(&format!(r#"stroke="{dark_stroke}""#)));
    assert!(dark_resolved.contains(&format!(r#"fill="{dark_stroke}""#)));

    assert!(light_resolved.contains(&format!(r#"stroke="{light_stroke}""#)));
    assert!(light_resolved.contains(&format!(r#"fill="{light_stroke}""#)));

    assert_ne!(dark_resolved, light_resolved);
    assert_eq!(pack.get_pack_icon_svg_for_palette(&dark_pal), pack.get_pack_icon_svg_resolved(&dark_pal));

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn discovery_and_cleaning_preserve_temporary_files() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_tmp_cleanup_test_{}", std::process::id()));
    let icons_dir = temp_root.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = r#"(
        package_type: IconTheme,
        id: "test-tmp-cleanup",
        name: "Test Tmp Cleanup",
        version: "1.0.0",
        author: "Author",
        license: "MIT",
        description: "Test description",
    )"#;
    std::fs::write(temp_root.join("manifest.ron"), manifest).unwrap();

    // SVG with cleanable metadata/junk
    let cleanable_svg = br#"<svg viewBox="0 0 24 24"><metadata>inkscape junk</metadata><circle cx="12" cy="12" r="10"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), cleanable_svg).unwrap();

    // Create residual temporary files
    let orphan_tmp1 = icons_dir.join(format!("line.svg.qymcad-{}-0.tmp", std::process::id()));
    let other_process_id = if std::process::id() == 888 { 889 } else { 888 };
    let orphan_tmp2 = temp_root.join(format!("icon.svg.qymcad-{other_process_id}-0.tmp"));
    let orphan_tmp3 = icons_dir.join("stray_marker.tmp");
    std::fs::write(&orphan_tmp1, b"residual data 1").unwrap();
    std::fs::write(&orphan_tmp2, b"residual data 2").unwrap();
    std::fs::write(&orphan_tmp3, b"residual data 3").unwrap();

    assert!(orphan_tmp1.exists());
    assert!(orphan_tmp2.exists());
    assert!(orphan_tmp3.exists());

    // Inspection must not modify temporary files in a user theme directory.
    let report = inspect_pack_directory(&temp_root).expect("pack directory is inspectable");
    assert_eq!(report.rejected.len(), 1, "cleanable icon with metadata should be rejected before cleaning");
    assert!(orphan_tmp1.exists(), "inspection must preserve temporary files owned by this process");
    assert!(orphan_tmp2.exists(), "inspection must preserve temporary files owned by another process");
    assert!(orphan_tmp3.exists(), "stray_marker.tmp is not a cleaner tmp file and must not be touched");

    // Explicit SVG cleaning must preserve pre-existing files, even when their names match its temp pattern.
    std::fs::write(&orphan_tmp1, b"residual data before clean").unwrap();
    let pack = IconPack::from_directory(&temp_root).expect("pack loads");
    let clean_report = clean_directory_icons(&pack).expect("cleaning succeeds");
    assert_eq!(clean_report.cleaned.len(), 1);
    assert!(orphan_tmp1.exists(), "temporary-looking files must not be removed by bulk cleaning");
    assert!(orphan_tmp2.exists(), "temporary-looking files with another PID must be preserved");
    assert!(orphan_tmp3.exists(), "stray_marker.tmp must remain untouched after clean_directory_icons");

    // Verify that both temporary-looking files remain untouched.
    let mut remaining_cleaner_tmps = Vec::new();
    for entry in walkdir_cleaner_tmps(&temp_root) {
        remaining_cleaner_tmps.push(entry);
    }
    assert!(remaining_cleaner_tmps.contains(&orphan_tmp1), "the pre-existing current-PID file must remain");
    assert!(remaining_cleaner_tmps.contains(&orphan_tmp2), "the other-PID file must remain");
    assert_eq!(remaining_cleaner_tmps.len(), 2, "the cleaner must remove only its own successfully renamed temp file");

    let _ = std::fs::remove_dir_all(&temp_root);
}

fn walkdir_cleaner_tmps(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut tmps = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                tmps.extend(walkdir_cleaner_tmps(&path));
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(super::super::is_cleaner_temp_file) {
                tmps.push(path);
            }
        }
    }
    tmps
}

#[test]
fn test_is_cleaner_temp_file_pattern_matching() {
    // Valid cleaner temp file patterns
    assert!(super::super::is_cleaner_temp_file("line.svg.qymcad-999-0.tmp"));
    assert!(super::super::is_cleaner_temp_file("icon.svg.qymcad-12345-42.tmp"));
    assert!(super::super::is_cleaner_temp_file("circle_3pt.svg.qymcad-1-0.tmp"));
    assert!(super::super::is_cleaner_temp_file("nested.part.svg.qymcad-500-12.tmp"));

    // User or non-cleaner files ending in .tmp must be preserved
    assert!(!super::super::is_cleaner_temp_file("stray_marker.tmp"));
    assert!(!super::super::is_cleaner_temp_file("backup.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.svg.tmp"));
    assert!(!super::super::is_cleaner_temp_file(".tmp"));

    // Malformed markers
    assert!(!super::super::is_cleaner_temp_file("line.svg.qymcad-abc-0.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.svg.qymcad-999-xyz.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.svg.qymcad-999.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.svg.qymcad-999-0-1.tmp"));
    assert!(!super::super::is_cleaner_temp_file("line.svg.qymcad--.tmp"));
}
