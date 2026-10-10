//! Unit tests for the icon theme system.

use std::collections::HashMap;

use super::bundle::*;
use super::id::*;
use super::manager::*;
use super::manifest::*;
use super::pack::*;
use super::runtime::forget_all_cad_icons;

#[test]
fn every_icon_id_has_relative_path() {
    for id in ALL_ICONS {
        let path = id.relative_path();
        assert!(!path.is_empty(), "icon {:?} has empty relative path", id);
        assert!(!path.ends_with(".svg"), "relative path should not contain extension: {}", path);

        let roundtrip = IconId::from_id_str(path);
        assert_eq!(roundtrip, Some(*id), "from_id_str failed for {}", path);
    }
}

#[test]
fn generated_correct_break_path() {
    assert_eq!(IconId::SketchBreak.relative_path(), "sketch/break");
}

#[test]
fn gallery_inspection_distinguishes_missing_and_invalid_icons() {
    let valid = br#"<svg viewBox="0 0 32 32"><path d="M0 0h32v32z"/></svg>"#;
    let invalid = br#"<svg viewBox="0 0 32 16"><path d="M0 0h32v16z"/></svg>"#;
    let mut icons = HashMap::new();
    icons.insert("icons/sketch/line.svg".to_string(), valid.to_vec());
    icons.insert("icons/sketch/rect.svg".to_string(), invalid.to_vec());
    let pack = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "gallery-test".to_string(),
            name: "Gallery Test".to_string(),
            version: "1.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            description: String::new(),
            translations: Default::default(),
            verified: false,
        },
        source: PackSource::Memory(icons),
        is_tampered: false,
        duplicate_conflict: None,
    };

    assert_eq!(pack.inspect_svg_for_id(IconId::SketchLine).unwrap(), Some(valid.to_vec()));
    assert_eq!(pack.inspect_svg_for_id(IconId::SketchCircle).unwrap(), None);
    assert!(pack.inspect_svg_for_id(IconId::SketchRect).unwrap_err().contains("non-square viewBox"));
}

#[test]
fn svg_cleaner_removes_forbidden_content_and_preserves_vector_paths() {
    let original = br#"<svg viewBox="0 0 24 24" width="24px" height="24px" id="svg1" version="1.1" onload="alert(1)" xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:dc="http://purl.org/dc/elements/1.1/" sodipodi:docname="test.svg" inkscape:version="1.0">
<metadata><rdf:RDF><script>alert(1)</script></rdf:RDF></metadata>
<sodipodi:namedview id="editor"/>
<foreignObject><p>HTML</p></foreignObject>
<image href="data:image/png;base64,AA=="/>
<defs><linearGradient id="paint" inkscape:collect="always"><stop offset="0" stop-color="red"/></linearGradient></defs>
<path id="drawing" d="M1 1 L20 20" fill="url(#paint)" onclick="alert(1)" inkscape:connector-curvature="0"/>
</svg>"#;
    let cleaned = clean_svg(original).expect("forbidden content can be removed");
    validate_svg(&cleaned).expect("cleaned vector is valid");
    let text = std::str::from_utf8(&cleaned).unwrap();
    assert!(text.contains("id=\"drawing\""));
    assert!(text.contains("M1 1 L20 20"));
    assert!(text.contains("linearGradient") && text.contains("url(#paint)"), "vector paint definitions changed");
    for forbidden in [
        "onload",
        "onclick",
        "<metadata",
        "<script",
        "<sodipodi:",
        "<foreignObject",
        "<image",
        "data:image/",
        "xmlns:sodipodi",
        "xmlns:inkscape",
        "xmlns:dc",
        "sodipodi:docname",
        "inkscape:version",
        "inkscape:collect",
        "inkscape:connector-curvature",
        "id=\"svg1\"",
        "width=\"24px\"",
        "height=\"24px\"",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} remained after cleaning");
    }
}

#[test]
fn svg_cleaner_refuses_geometry_it_cannot_repair() {
    let original = br#"<svg viewBox="0 0 32 16"><path d="M0 0 L20 10"/></svg>"#;
    assert!(clean_svg(original).is_err(), "a non-square viewBox needs a decision from the artist");
    let malformed = br#"<svg viewBox="0 0 24 24"><metadata>editor</metadata><path d="M0 0 L20 20"/>"#;
    assert!(clean_svg(malformed).is_err(), "cleaning must not write an unclosed SVG");
}

#[test]
fn svg_cleaner_removes_custom_xml_entities_without_leaving_references() {
    let original = br#"<!DOCTYPE svg [<!ENTITY payload "untrusted">]><svg viewBox="0 0 24 24"><text>&payload;</text><path d="M0 0 L24 24"/></svg>"#;
    let cleaned = clean_svg(original).expect("entity declaration can be removed");
    let text = std::str::from_utf8(&cleaned).unwrap();
    assert!(!text.contains("DOCTYPE") && !text.contains("ENTITY") && !text.contains("&payload;"));
    assert!(text.contains("M0 0 L24 24"));
    validate_svg(&cleaned).unwrap();
}

#[test]
fn directory_cleaner_updates_single_and_all_repairable_icons() {
    let root = std::env::temp_dir().join(format!("qymcad_clean_icons_{}", std::process::id()));
    let icons = root.join("icons/sketch");
    std::fs::create_dir_all(&icons).expect("create icons directory");
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "clean-icons".into(),
        name: "Clean Icons".into(),
        version: "1.0".into(),
        author: "Test".into(),
        license: "MIT".into(),
        description: String::new(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(root.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let dirty = br#"<svg viewBox="0 0 24 24"><metadata>editor</metadata><path d="M0 0L24 24"/></svg>"#;
    let bad_geometry = br#"<svg viewBox="0 0 32 16"><path d="M0 0L20 10"/></svg>"#;
    std::fs::write(icons.join("line.svg"), dirty).unwrap();
    std::fs::write(icons.join("circle.svg"), dirty).unwrap();
    std::fs::write(icons.join("rect.svg"), bad_geometry).unwrap();
    std::fs::write(icons.join("custom.svg"), dirty).unwrap();
    std::fs::write(root.join("icon.svg"), dirty).unwrap();
    let pack = IconPack::from_directory(&root).unwrap();
    assert!(directory_has_cleanable_icons(&pack).unwrap(), "directory offers bulk cleaning while repairable SVGs exist");

    assert_eq!(clean_directory_icon(&pack, IconId::SketchLine).unwrap(), CleanIconResult::Cleaned);
    validate_svg(&std::fs::read(icons.join("line.svg")).unwrap()).unwrap();
    assert_eq!(clean_directory_icon(&pack, IconId::SketchLine).unwrap(), CleanIconResult::Unchanged);
    let report = clean_directory_icons(&pack).unwrap();
    assert_eq!(report.cleaned.len(), 3);
    for path in ["icons/sketch/circle.svg", "icons/sketch/custom.svg", "icon.svg"] {
        assert!(report.cleaned.contains(&std::path::PathBuf::from(path)), "bulk clean omitted {path}");
    }
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].path, std::path::PathBuf::from("icons/sketch/rect.svg"));
    assert_eq!(std::fs::read(icons.join("rect.svg")).unwrap(), bad_geometry);
    validate_svg(&std::fs::read(icons.join("circle.svg")).unwrap()).unwrap();
    validate_svg(&std::fs::read(icons.join("custom.svg")).unwrap()).expect("bulk clean includes custom SVG files");
    validate_svg(&std::fs::read(root.join("icon.svg")).unwrap()).unwrap();
    assert!(!directory_has_cleanable_icons(&pack).unwrap(), "manual-only errors do not offer bulk cleaning");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_ron_roundtrip() {
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "freecad-classic".to_string(),
        name: "FreeCAD Classic".to_string(),
        version: "1.0.0".to_string(),
        author: "FreeCAD Contributors".to_string(),
        license: "LGPL-2.1-or-later".to_string(),
        description: "Classic colored tool icons".to_string(),
        translations: Default::default(),
        verified: false,
    };

    let ron_str = manifest.to_ron().expect("serialization succeeds");
    let parsed = IconManifest::parse_ron(&ron_str).expect("parsing succeeds");

    assert_eq!(parsed.id, "freecad-classic");
    assert_eq!(parsed.name, "FreeCAD Classic");
    assert_eq!(parsed.description, "Classic colored tool icons");
}

#[test]
fn localized_bundle_text_survives_packaging() {
    let root = std::env::temp_dir().join(format!("qymcad_localized_bundle_{}", std::process::id()));
    let icons = root.join("icons/sketch");
    std::fs::create_dir_all(&icons).expect("create icon folder");
    std::fs::write(icons.join("line.svg"), br#"<svg viewBox="0 0 24 24"><path d="M0 0 L24 24"/></svg>"#).expect("write SVG");
    let manifest = r#"(
        id: "localized-test",
        name: "Color Icons",
        description: "Base description",
        translations: {
            "de": (name: "Farbsymbole", description: "Deutsche Beschreibung"),
        },
    )"#;
    std::fs::write(root.join("manifest.ron"), manifest).expect("write manifest");
    std::fs::write(root.join("README.md"), "# Base documentation").expect("write base README");
    std::fs::write(root.join("README.de.md"), "# Deutsche Dokumentation").expect("write German README");
    let folder = IconPack::from_directory(&root).expect("localized folder loads");
    assert_eq!(folder.manifest.name_for_locale("de"), "Farbsymbole");
    assert_eq!(folder.manifest.description_for_locale("de-DE"), "Deutsche Beschreibung");
    assert_eq!(folder.manifest.name_for_locale("fr"), "Color Icons");
    assert_eq!(folder.get_readme_for_locale("de"), "# Deutsche Dokumentation");
    assert_eq!(folder.get_readme_for_locale("de-DE"), "# Deutsche Dokumentation");
    assert_eq!(folder.get_readme_for_locale("fr"), "# Base documentation");
    let no_readme = IconPack { manifest: folder.manifest.clone(), source: PackSource::Memory(HashMap::new()), is_tampered: false, duplicate_conflict: None };
    let generated = no_readme.get_readme_for_locale("de-DE");
    assert!(generated.starts_with("# Farbsymbole\n\nDeutsche Beschreibung"), "missing README uses localized manifest text");

    let archive = root.join("localized-test.qicons");
    package_bundle(&root, &folder.manifest, &archive).expect("package localized folder");
    let packed = IconPack::from_archive(&archive).expect("localized archive loads");
    assert_eq!(packed.manifest.name_for_locale("de"), "Farbsymbole");
    assert_eq!(packed.get_readme_for_locale("de-DE"), "# Deutsche Dokumentation");
    assert_eq!(packed.get_readme_for_locale("fr"), "# Base documentation");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cascade_fallback_chain() {
    let sample_svg = br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#;

    // Pack A has sketch/line
    let mut map_a = HashMap::new();
    map_a.insert("icons/sketch/line.svg".to_string(), sample_svg.to_vec());
    let pack_a = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "pack-a".to_string(),
            name: "Pack A".to_string(),
            version: "1.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            description: "".to_string(),
            translations: Default::default(),
            verified: false,
        },
        source: PackSource::Memory(map_a),
        is_tampered: false,
        duplicate_conflict: None,
    };

    // Pack B has sketch/rect
    let mut map_b = HashMap::new();
    map_b.insert("icons/sketch/rect.svg".to_string(), sample_svg.to_vec());
    let pack_b = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "pack-b".to_string(),
            name: "Pack B".to_string(),
            version: "1.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            description: "".to_string(),
            translations: Default::default(),
            verified: false,
        },
        source: PackSource::Memory(map_b),
        is_tampered: false,
        duplicate_conflict: None,
    };

    let stack = vec![pack_a, pack_b];
    let pal = qymcad_scheme::dark();

    // 1. Line is resolved from Pack A
    let res_line = resolve_icon(IconId::SketchLine, &stack, &pal);
    assert_eq!(res_line.pack_id, "pack-a");

    // 2. Rect is resolved from Pack B (falling through Pack A)
    let res_rect = resolve_icon(IconId::SketchRect, &stack, &pal);
    assert_eq!(res_rect.pack_id, "pack-b");

    // 3. Extrude is in neither Pack A nor Pack B, falls back to the embedded default SVG pack
    let res_extrude = resolve_icon(IconId::PartExtrude, &stack, &pal);
    assert_eq!(res_extrude.pack_id, DEFAULT_THEME_ID);
}

#[test]
fn svg_validation_rules() {
    let valid = br#"<svg viewBox="0 0 64 64"><path d="M0,0 L64,64"/></svg>"#;
    assert!(validate_svg(valid).is_ok());

    let missing_viewbox = br#"<svg width="64" height="64"><path d="M0,0"/></svg>"#;
    assert!(validate_svg(missing_viewbox).is_err());

    let with_embedded_raster = br#"<svg viewBox="0 0 64 64"><image href="data:image/png;base64,123"/></svg>"#;
    assert!(validate_svg(with_embedded_raster).is_err());

    // Prohibited script and foreignObject
    let with_script = br#"<svg viewBox="0 0 64 64"><script>alert(1)</script></svg>"#;
    assert!(validate_svg(with_script).is_err());
    let with_foreign = br#"<svg viewBox="0 0 64 64"><foreignObject><div>test</div></foreignObject></svg>"#;
    assert!(validate_svg(with_foreign).is_err());

    // Inline event handlers on root and nested elements
    let with_handler = br#"<svg viewBox="0 0 64 64" onload="run()"><circle cx="32" cy="32" r="10"/></svg>"#;
    assert!(validate_svg(with_handler).is_err());
    let with_nested_handler = br#"<svg viewBox="0 0 64 64"><rect onclick="run()"/></svg>"#;
    assert!(validate_svg(with_nested_handler).is_err());

    // Prohibited javascript: in href or xlink:href
    let with_js_href = br#"<svg viewBox="0 0 64 64"><a href="javascript:alert(1)"><circle cx="32" cy="32" r="10"/></a></svg>"#;
    assert!(validate_svg(with_js_href).is_err());

    // Non-square preview banner allows structural SVG but rejects square-enforced icon SVG
    let banner_preview = br#"<svg viewBox="0 0 120 60"><rect width="120" height="60"/></svg>"#;
    assert!(validate_svg_structural(banner_preview, false).is_ok());
    assert!(validate_svg(banner_preview).is_err());

    // Editor metadata and namespaces
    let with_sodipodi = br#"<svg viewBox="0 0 64 64"><sodipodi:namedview id="base"/></svg>"#;
    assert!(validate_svg(with_sodipodi).is_err());
    let with_inkscape = br#"<svg viewBox="0 0 64 64"><inkscape:grid id="grid1"/></svg>"#;
    assert!(validate_svg(with_inkscape).is_err());
    let with_metadata = br#"<svg viewBox="0 0 64 64"><metadata id="meta"><rdf:RDF/></metadata></svg>"#;
    assert!(validate_svg(with_metadata).is_err());
    let with_illustrator = br#"<svg viewBox="0 0 64 64"><i:pgf id="adobe_pgf"/></svg>"#;
    assert!(validate_svg(with_illustrator).is_err());
}

#[test]
fn package_bundle_and_load_from_archive() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_icon_test_{}", std::process::id()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).expect("creates test dirs");

    let svg_content = br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg_content).expect("writes svg");
    let pack_icon = br#"<svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="20"/></svg>"#;
    std::fs::write(temp_dir.join("icon.svg"), pack_icon).expect("writes pack icon");

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "test-pack".to_string(),
        name: "Test Pack".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().expect("manifest serializes")).expect("writes manifest");

    let archive_path = temp_dir.join("test-pack.qicons");
    let report = package_bundle(&temp_dir, &manifest, &archive_path).expect("packaging succeeds");
    assert_eq!(report.included.len(), 1);
    assert_eq!(report.rejected.len(), 0);
    assert_eq!(report.extraneous.len(), 0);
    assert_eq!(report.missing.len(), ALL_ICONS.len() - 1);
    assert_eq!(IconPack::from_directory(&temp_dir).expect("folder loads").get_pack_icon_svg(), pack_icon);

    let pack = IconPack::from_archive(&archive_path).expect("loading archive succeeds");
    assert_eq!(pack.manifest.id, "test-pack");
    assert_eq!(pack.coverage().present, 1);
    assert_eq!(pack.get_pack_icon_svg(), pack_icon, "packaging must retain the icon beside manifest.ron");

    let embedded = IconPack::from_zip_bytes(&std::fs::read(&archive_path).expect("archive reads")).expect("embedded archive loads");
    assert_eq!(embedded.get_pack_icon_svg(), pack_icon);

    let data = pack.get_svg_for_id(IconId::SketchLine).expect("line icon exists in archive");
    assert_eq!(data, svg_content);

    let missing = pack.get_svg_for_id(IconId::SketchCircle);
    assert!(missing.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

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
        verified: false,
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
fn validation_viewbox_square_and_invalid() {
    // 1:1 square viewbox passes
    let valid_svg = br#"<svg viewBox="0 0 64 64"><rect width="64" height="64"/></svg>"#;
    assert!(validate_svg(valid_svg).is_ok());

    // Non-square viewbox (e.g. 100 x 50) fails
    let non_square_svg = br#"<svg viewBox="0 0 100 50"><rect width="100" height="50"/></svg>"#;
    let err = validate_svg(non_square_svg).unwrap_err();
    assert!(err.contains("aspect ratio must be 1:1"), "Expected non-square error, got: {}", err);

    // Missing viewBox fails
    let no_viewbox_svg = br#"<svg width="64" height="64"><circle r="10"/></svg>"#;
    let err = validate_svg(no_viewbox_svg).unwrap_err();
    assert!(err.contains("missing viewBox"), "Expected missing viewBox error, got: {}", err);

    let malformed_viewbox_svg = br#"<svg viewBox="0 0 wide 64"><circle r="10"/></svg>"#;
    let err = validate_svg(malformed_viewbox_svg).unwrap_err();
    assert!(err.contains("invalid viewBox"), "Expected malformed viewBox error, got: {}", err);

    let non_finite_viewbox_svg = br#"<svg viewBox="0 0 NaN NaN"><circle r="10"/></svg>"#;
    let err = validate_svg(non_finite_viewbox_svg).unwrap_err();
    assert!(err.contains("invalid viewBox"), "Expected non-finite viewBox error, got: {}", err);

    // Raster image tag fails
    let raster_svg = br#"<svg viewBox="0 0 64 64"><image href="photo.png"/></svg>"#;
    let err = validate_svg(raster_svg).unwrap_err();
    assert!(err.contains("raster"), "Expected raster image error, got: {}", err);
}

#[test]
fn inspect_and_package_excludes_problematic_files() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_val_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).expect("creates test dirs");

    // Valid icon: sketch/line.svg
    let valid_svg = br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), valid_svg).expect("writes valid svg");

    // Invalid icon (non-square): sketch/circle.svg
    let non_square_svg = br#"<svg viewBox="0 0 100 50"><circle cx="50" cy="25" r="20"/></svg>"#;
    std::fs::write(icons_dir.join("circle.svg"), non_square_svg).expect("writes non-square svg");

    // Extraneous file inside icons: sketch/notes.txt
    std::fs::write(icons_dir.join("notes.txt"), b"some notes").expect("writes notes");

    // Extraneous unknown icon: icons/sketch/unknown_extra_icon.svg
    std::fs::write(icons_dir.join("unknown_extra_icon.svg"), valid_svg).expect("writes unknown svg");

    // Extraneous file in pack root: temp_dir/notes.md
    std::fs::write(temp_dir.join("notes.md"), b"pack notes").expect("writes root notes");

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "validation-test-pack".to_string(),
        name: "Validation Test Pack".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
        verified: false,
    };

    // Test inspect_pack_directory first
    let report = inspect_pack_directory(&temp_dir).expect("inspect succeeds");
    assert_eq!(report.included.len(), 1, "Only 1 valid icon should be included");
    assert_eq!(report.included[0], IconId::SketchLine);

    assert_eq!(report.rejected.len(), 1, "1 icon should be rejected");
    assert_eq!(report.rejected[0].path, "sketch/circle.svg");
    assert!(report.rejected[0].reason.contains("aspect ratio must be 1:1"));

    assert_eq!(report.extraneous.len(), 3, "3 extraneous files should be flagged");

    // Package bundle: archive should only contain valid icon and manifest
    let archive_path = temp_dir.join("val-pack.qicons");
    let pack_report = package_bundle(&temp_dir, &manifest, &archive_path).expect("package succeeds");
    assert_eq!(pack_report.included.len(), 1);
    assert_eq!(pack_report.rejected.len(), 1);
    assert_eq!(pack_report.extraneous.len(), 3);

    // Verify loaded pack from archive
    let pack = IconPack::from_archive(&archive_path).expect("loads archive");
    assert_eq!(pack.coverage().present, 1);
    assert!(pack.get_svg_for_id(IconId::SketchLine).is_some());
    assert!(pack.get_svg_for_id(IconId::SketchCircle).is_none(), "Rejected icon must NOT be in archive");

    let _ = std::fs::remove_dir_all(&temp_dir);
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
        validate_icon_svg(&svg).expect("default SVG must pass monochrome validation");
    }

    // Verify pack README is loaded
    let readme = pack.get_readme();
    assert!(readme.contains("Shapr-Alike"), "embedded README should be available");
}

#[test]
fn all_embedded_packs_are_valid_and_complete() {
    let packs = load_builtin_packs();
    assert!(!packs.is_empty(), "must load at least shapr-alike; got {}", packs.len());

    let shapr_pack = packs.iter().find(|p| p.manifest.id == "shapr-alike").expect("shapr-alike pack exists");
    assert_eq!(shapr_pack.format(), BundleFormat::Embedded);
    assert!(!shapr_pack.is_directory());
    assert!(shapr_pack.is_verified());
    assert!(shapr_pack.manifest.translations.contains_key("kk"));
    assert!(shapr_pack.manifest.translations.contains_key("ru"));
    assert!(shapr_pack.manifest.translations.contains_key("uk"));
    assert_ne!(shapr_pack.manifest.description_for_locale("kk"), shapr_pack.manifest.description);
    assert_ne!(shapr_pack.manifest.description_for_locale("ru"), shapr_pack.manifest.description);
    assert_ne!(shapr_pack.manifest.description_for_locale("uk"), shapr_pack.manifest.description);
    assert_ne!(shapr_pack.get_readme_for_locale("kk"), shapr_pack.get_readme());
    assert_ne!(shapr_pack.get_readme_for_locale("ru"), shapr_pack.get_readme());
    assert_ne!(shapr_pack.get_readme_for_locale("uk"), shapr_pack.get_readme());
    let (shapr_cov, total) = (shapr_pack.coverage().present, shapr_pack.coverage().total);
    assert_eq!(shapr_cov, total, "embedded Shapr-Alike pack must cover all icons");

    for pack in packs {
        assert_eq!(pack.format(), BundleFormat::Embedded, "every built-in pack must have Embedded format");
        assert!(!pack.is_directory(), "embedded pack must not be reported as a directory");
        assert!(pack.is_verified(), "embedded pack must be marked verified");
        assert!(!pack.is_tampered, "embedded pack must not be marked tampered");
        validate_svg(&pack.get_pack_icon_svg()).expect("embedded pack icon is valid SVG");
    }
}

#[test]
fn monochrome_inspection_and_packaging_share_color_validation() {
    let root = std::env::temp_dir().join(format!("qymcad_mono_validation_{}", std::process::id()));
    let icons = root.join("icons/sketch");
    std::fs::create_dir_all(&icons).unwrap();
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "mono-validation".into(),
        name: "Mono validation".into(),
        version: "1.0.0".into(),
        author: "Test".into(),
        license: "MIT".into(),
        description: String::new(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(root.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    std::fs::write(icons.join("line.svg"), br#"<svg viewBox="0 0 24 24" fill="var(--icon-stroke, #fff)"><path d="M0 0h24v24"/></svg>"#).unwrap();
    std::fs::write(icons.join("circle.svg"), br#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="var(--invalid-token, #f00)"/></svg>"#).unwrap();
    let report = inspect_pack_directory(&root).unwrap();
    assert_eq!(report.included, vec![IconId::SketchLine]);
    assert!(report.rejected.iter().any(|rej| rej.path == "sketch/circle.svg" && rej.reason.contains("unknown icon token")));
    let pack = IconPack::from_directory(&root).unwrap();
    assert!(pack.inspect_svg_for_id(IconId::SketchCircle).unwrap_err().contains("unknown icon token"));
    let archive = root.join("output.qicons");
    package_bundle(&root, &manifest, &archive).unwrap();
    let bundled = IconPack::from_archive(&archive).unwrap();
    assert!(bundled.get_svg_for_id(IconId::SketchCircle).is_none());

    // With valid token it is included
    std::fs::write(icons.join("circle.svg"), br#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="var(--icon-sketch-primary, #f00)"/></svg>"#).unwrap();
    let valid_archive = root.join("valid.qicons");
    let valid_report = package_bundle(&root, &manifest, &valid_archive).unwrap();
    assert!(valid_report.included.contains(&IconId::SketchCircle));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn token_svg_validates_css_variables_and_fallbacks() {
    // Valid tokens with fallbacks must pass (including nested vars)
    for valid in [
        r##"<svg viewBox="0 0 24 24"><path d="M0 0h24" stroke="var(--icon-stroke, #fff)"/></svg>"##,
        r##"<svg viewBox="0 0 24 24"><circle fill="var(--icon-sketch-primary, #0288D1)"/></svg>"##,
        r##"<svg viewBox="0 0 24 24"><rect fill="var(--icon-neutral, #CCCCCC)" stroke="currentColor"/></svg>"##,
        r##"<svg viewBox="0 0 24 24"><path fill="#f00" stroke="#00f"/></svg>"##,
        r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, var(--icon-accent, #333))"/></svg>"##,
    ] {
        validate_icon_svg(valid.as_bytes()).unwrap();
    }

    // Invalid variables must be rejected (including in nested vars)
    for (invalid, expected) in [
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke)"/></svg>"##, "must specify a fallback color"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--nonexistent-token, #fff)"/></svg>"##, "unknown icon token"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(icon-stroke, #fff)"/></svg>"##, "must start with '--'"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, )"/></svg>"##, "empty fallback color"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, var(--bad-token, #333))"/></svg>"##, "unknown icon token"),
    ] {
        let err = validate_icon_svg(invalid.as_bytes()).unwrap_err();
        assert!(err.contains(expected), "expected '{expected}' in '{err}'");
    }
}

#[test]
fn resolve_icon_tokens_substitutes_active_palette_and_preserves_static() {
    let dark_pal = qymcad_scheme::dark();
    let light_pal = qymcad_scheme::light();

    let svg_with_tokens = br##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, #111)" fill="var(--icon-sketch-primary, #222)"/><rect fill="currentColor"/></svg>"##;

    let dark_resolved = String::from_utf8(super::manager::resolve_icon_tokens(svg_with_tokens, &dark_pal)).unwrap();
    assert!(dark_resolved.contains("stroke=\"#000000\""));
    assert!(dark_resolved.contains("fill=\"#18F2F2\""));
    assert!(dark_resolved.contains("fill=\"#000000\""));

    let light_resolved = String::from_utf8(super::manager::resolve_icon_tokens(svg_with_tokens, &light_pal)).unwrap();
    assert!(light_resolved.contains("stroke=\"#808080\""));
    assert!(light_resolved.contains("fill=\"#18F2F2\""));
    assert!(light_resolved.contains("fill=\"#808080\""));

    // Dimmed underlay token
    let dimmed_svg = br##"<svg viewBox="0 0 24 24"><path fill="var(--icon-dimmed, #FFFFFF)"/></svg>"##;
    let dark_dimmed = String::from_utf8(super::manager::resolve_icon_tokens(dimmed_svg, &dark_pal)).unwrap();
    assert!(dark_dimmed.contains("fill=\"#7C7C7C\""));
    let light_dimmed = String::from_utf8(super::manager::resolve_icon_tokens(dimmed_svg, &light_pal)).unwrap();
    assert!(light_dimmed.contains("fill=\"#ABABAB\""));

    // Unknown token uses fallback
    let unknown_token_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--custom-fallback, #AABBCC)"/></svg>"##;
    let fallback_resolved = String::from_utf8(super::manager::resolve_icon_tokens(unknown_token_svg, &dark_pal)).unwrap();
    assert!(fallback_resolved.contains("stroke=\"#AABBCC\""));

    // Nested paren in fallback
    let nested_fallback_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--custom-rgb, rgba(10, 20, 30, 0.5))"/></svg>"##;
    let nested_resolved = String::from_utf8(super::manager::resolve_icon_tokens(nested_fallback_svg, &dark_pal)).unwrap();
    assert!(nested_resolved.contains("stroke=\"rgba(10, 20, 30, 0.5)\""));

    // Nested var expressions
    let nested_vars_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--unknown-outer, var(--icon-accent, #000))" fill="var(--icon-stroke, var(--bad-token, #111))" d="M0 0" color="var(--u1, var(--u2, var(--u3, #112233)))"/></svg>"##;
    let nested_vars_resolved = String::from_utf8(super::manager::resolve_icon_tokens(nested_vars_svg, &dark_pal)).unwrap();
    assert!(nested_vars_resolved.contains("stroke=\"#18F2F2\""), "outer unknown should resolve inner icon-accent token, got: {nested_vars_resolved}");
    assert!(nested_vars_resolved.contains("fill=\"#000000\""), "outer defined icon-stroke should resolve directly without fallback, got: {nested_vars_resolved}");
    assert!(nested_vars_resolved.contains("color=\"#112233\""), "deeply nested unknown vars should resolve to final fallback, got: {nested_vars_resolved}");

    // Static SVG without tokens is byte-for-byte identical
    let static_svg = br##"<svg viewBox="0 0 24 24"><path stroke="#FF0000" fill="#00FF00"/></svg>"##;
    let static_resolved = super::manager::resolve_icon_tokens(static_svg, &dark_pal);
    assert_eq!(static_resolved, static_svg.to_vec());

    // Element id, class or text node containing currentColor must NOT be corrupted
    let id_svg = br##"<svg viewBox="0 0 24 24"><path id="currentColor" class="currentColor" stroke="currentColor" fill="currentColor"/><stop stop-color="currentColor"/><text> currentColor </text><g style="fill: currentColor; stroke: currentColor"/></svg>"##;
    let id_resolved = String::from_utf8(super::manager::resolve_icon_tokens(id_svg, &dark_pal)).unwrap();
    assert!(id_resolved.contains("id=\"currentColor\""), "element id must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("class=\"currentColor\""), "element class must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("<text> currentColor </text>"), "text content must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("stroke=\"#000000\""), "stroke currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("fill=\"#000000\""), "fill currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("stop-color=\"#000000\""), "stop-color currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("style=\"fill: #000000; stroke: #000000\""), "style properties must be replaced, got: {id_resolved}");
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
            verified: false,
        },
        source: PackSource::Memory(map),
        is_tampered: false,
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
        verified: false,
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
fn repo_shapr_alike_theme_if_present_loads_and_has_icons() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let shapr_dir = manifest_dir.join("../../assets/icon-themes/shapr-alike");
    if shapr_dir.is_dir() {
        let pack = IconPack::from_directory(&shapr_dir).expect("shapr-alike directory must load cleanly");
        assert_eq!(pack.manifest.id, "shapr-alike");
        assert_eq!(pack.manifest.name, "Shapr-Alike");
        let cov = pack.coverage();
        assert_eq!(cov.present, cov.total, "shapr-alike theme in repo should cover all icons, got {}/{}", cov.present, cov.total);
    }
}

#[test]
fn bundle_format_and_provenance_detection() {
    let default_pack = load_default_pack().expect("embedded default pack");
    assert_eq!(default_pack.format(), BundleFormat::Embedded);
    assert!(default_pack.is_verified());
    assert!(!default_pack.is_directory());

    let temp_dir = std::env::temp_dir().join(format!("qymcad_format_test_{}", std::process::id()));
    let folder_pack_dir = temp_dir.join("folder_theme");
    std::fs::create_dir_all(folder_pack_dir.join("icons").join("sketch")).unwrap();
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "folder-theme".to_string(),
        name: "Folder Theme".to_string(),
        version: "1.0.0".to_string(),
        author: "Dev".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(folder_pack_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    std::fs::write(folder_pack_dir.join("icons").join("sketch").join("line.svg"), br#"<svg viewBox="0 0 24 24"><line x1="0" y1="0" x2="24" y2="24"/></svg>"#).unwrap();

    let folder_pack = IconPack::from_directory(&folder_pack_dir).expect("folder pack must load");
    assert_eq!(folder_pack.format(), BundleFormat::Directory);
    assert!(folder_pack.is_directory());
    assert!(!folder_pack.is_archive());

    // Package into .qicons via CAD packager
    let archive_path = temp_dir.join("packaged.qicons");
    package_bundle(&folder_pack_dir, &manifest, &archive_path).expect("packaging must succeed");

    let loaded_archive = IconPack::from_archive(&archive_path).expect("packaged archive must load");
    assert_eq!(loaded_archive.format(), BundleFormat::Package);
    assert!(loaded_archive.is_verified());
    assert!(loaded_archive.is_archive());
    assert!(!loaded_archive.is_directory());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

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
        verified: false,
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
        verified: false,
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
        verified: false,
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
fn qicons_valid_trailer_and_tampered_downgrade() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_tamper_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let pack_dir = temp_dir.join("tamper_src");
    std::fs::create_dir_all(pack_dir.join("icons").join("sketch")).unwrap();

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "tamper-test".to_string(),
        name: "Tamper Test".to_string(),
        version: "1.0.0".to_string(),
        author: "Dev".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(pack_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let valid_svg = br#"<svg viewBox="0 0 24 24"><line x1="0" y1="0" x2="24" y2="24"/></svg>"#;
    std::fs::write(pack_dir.join("icons").join("sketch").join("line.svg"), valid_svg).unwrap();

    // 1. Package legitimate .qicons bundle with trailing verification record
    let bundle_path = temp_dir.join("official.qicons");
    let report = package_bundle(&pack_dir, &manifest, &bundle_path).expect("bundle packaging succeeds");
    assert_eq!(report.included.len(), 1);

    // 2. Load genuine bundle: should be Package with is_tampered == false
    let pack = IconPack::from_archive(&bundle_path).expect("genuine bundle loads cleanly");
    assert_eq!(pack.format(), BundleFormat::Package);
    assert!(!pack.is_tampered);
    assert!(pack.manifest.verified);
    assert_eq!(pack.get_svg_for_id(IconId::SketchLine).unwrap(), valid_svg);

    // 3. Tamper with the bundle bytes (e.g. external archiver or binary patch modification)
    let mut tampered_bytes = std::fs::read(&bundle_path).unwrap();
    assert!(tampered_bytes.len() > 40);
    // Alter a byte inside the zip payload (before the 40-byte trailer)
    tampered_bytes[10] ^= 0xFF;
    let tampered_path = temp_dir.join("tampered.qicons");
    std::fs::write(&tampered_path, &tampered_bytes).unwrap();

    // 4. Load tampered file: zip archive itself is broken or hash fails
    // Even if it parses as zip or if trailer hash doesn't match:
    // When SHA-256 doesn't match, verify_qicons_trailer returns Tampered
    let check = crate::icons::sha256::verify_qicons_trailer(&tampered_bytes);
    assert_eq!(check, crate::icons::sha256::TrailerCheck::IntegrityBad);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn unverified_qicons_with_crash_guard_and_reject_missing_manifest() {
    use std::io::Write;
    let temp_dir = std::env::temp_dir().join(format!("qymcad_qicons_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let valid_svg = br#"<svg viewBox="0 0 32 32"><circle cx="16" cy="16" r="10"/></svg>"#;
    let entity_bomb = br#"<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol"><!ELEMENT lolz (#PCDATA)>]><svg viewBox="0 0 10 10">&lol;</svg>"#;

    // 1. Generic .zip extension must be rejected
    let zip_path = temp_dir.join("my-cool-pack.zip");
    std::fs::write(&zip_path, b"dummy zip").unwrap();
    let zip_err = IconPack::from_archive(&zip_path).unwrap_err();
    assert!(zip_err.contains("must have .qicons extension"), "got: {zip_err}");

    // 2. .qicons without manifest.ron must be rejected
    let no_manifest_path = temp_dir.join("no-manifest.qicons");
    let file = std::fs::File::create(&no_manifest_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("icons/sketch/circle.svg", options).unwrap();
    zip.write_all(valid_svg).unwrap();
    zip.finish().unwrap();
    let no_manifest_err = IconPack::from_archive(&no_manifest_path).unwrap_err();
    assert!(no_manifest_err.contains("missing manifest.ron"), "got: {no_manifest_err}");

    // 3. Unsigned/unverified .qicons with manifest.ron loads with crash-guard active
    let qicons_path = temp_dir.join("unverified.qicons");
    let file = std::fs::File::create(&qicons_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    let manifest = r#"(id: "unverified-pack", name: "Unverified Pack")"#;
    zip.start_file("manifest.ron", options).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.start_file("icons/sketch/circle.svg", options).unwrap();
    zip.write_all(valid_svg).unwrap();
    zip.start_file("icons/sketch/line.svg", options).unwrap();
    zip.write_all(entity_bomb).unwrap();
    zip.finish().unwrap();

    let pack = IconPack::from_archive(&qicons_path).expect("unverified qicons must load");
    assert_eq!(pack.format(), BundleFormat::Package);
    assert!(!pack.manifest.verified);
    assert!(pack.is_tampered);
    assert_eq!(pack.manifest.name, "Unverified Pack");

    // Valid SVG should be retrieved
    let circle_data = pack.get_svg_for_id(IconId::SketchCircle);
    assert_eq!(circle_data.unwrap(), valid_svg);

    // Dangerous XML entity bomb must be rejected by crash-guard (returns None instead of crashing)
    let line_data = pack.get_svg_for_id(IconId::SketchLine);
    assert!(line_data.is_none(), "Entity bomb should be filtered out by crash-guard");
    assert!(pack.inspect_svg_for_id(IconId::SketchLine).unwrap_err().contains("ENTITY"), "the gallery needs the archive error instead of a missing status");
    assert_eq!(pack.inspect_svg_for_id(IconId::SketchRect).unwrap(), None, "an absent archive entry is normal");

    let snapshot = pack.archive_snapshot().expect("the manager can read the archive once");
    assert_eq!(snapshot.format(), pack.format());
    assert_eq!(snapshot.get_svg_for_id(IconId::SketchCircle), pack.get_svg_for_id(IconId::SketchCircle));
    assert_eq!(snapshot.get_svg_for_id(IconId::SketchLine), pack.get_svg_for_id(IconId::SketchLine));
    assert!(snapshot.inspect_svg_for_id(IconId::SketchLine).unwrap_err().contains("ENTITY"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn zip_bomb_excessive_compression_ratio_is_rejected() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_bomb_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create a zip bomb: 500 KB of zeroes compresses to ~500 bytes (ratio ~1000:1)
    let bomb_path = temp_dir.join("bomb.qicons");
    let file = std::fs::File::create(&bomb_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("icons/sketch/line.svg", options).unwrap();
    let zero_payload = vec![0u8; 500 * 1024];
    std::io::Write::write_all(&mut zip, &zero_payload).unwrap();
    zip.finish().unwrap();

    let res = IconPack::from_archive(&bomb_path);
    assert!(res.is_err(), "Zip bomb archive must be rejected");
    let err = res.unwrap_err();
    assert!(err.contains("possible decompression bomb") || err.contains("suspicious compression ratio"), "Expected decompression bomb error, got: {err}");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn zip_slip_path_traversal_is_rejected() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_slip_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let slip_path = temp_dir.join("slip.qicons");
    let file = std::fs::File::create(&slip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("../../etc/passwd", options).unwrap();
    std::io::Write::write_all(&mut zip, b"malicious").unwrap();
    zip.finish().unwrap();

    let res = IconPack::from_archive(&slip_path);
    assert!(res.is_err(), "Zip slip archive must be rejected");
    let err = res.unwrap_err();
    assert!(err.contains("insecure file path"), "Expected insecure file path error, got: {err}");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn manifest_validation_accepts_valid_manifest() {
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-theme-123".into(),
        name: "Valid Theme Name".into(),
        version: "1.0.0".into(),
        author: "Author Person".into(),
        license: "MIT".into(),
        description: "A valid short description of this theme.".into(),
        translations: [("de".into(), LocalizedThemeText { name: "Deutsches Thema".into(), description: "Kurze Beschreibung".into() })].into_iter().collect(),
        verified: false,
    };
    assert!(manifest.validate().is_ok());
}

#[test]
fn manifest_validation_rejects_invalid_or_oversized_id() {
    let mut manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-id".into(),
        name: "Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Desc".into(),
        translations: Default::default(),
        verified: false,
    };

    manifest.id = "".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.id = "a".repeat(super::manifest::MAX_MANIFEST_ID_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("maximum length"));

    manifest.id = "Upper-Case".into();
    assert!(manifest.validate().unwrap_err().contains("lowercase"));

    manifest.id = "has spaces".into();
    assert!(manifest.validate().unwrap_err().contains("alphanumeric"));

    manifest.id = "../traversal".into();
    assert!(manifest.validate().unwrap_err().contains("alphanumeric"));

    manifest.id = "-starts-with-hyphen".into();
    assert!(manifest.validate().unwrap_err().contains("start and end"));

    manifest.id = "ends-with-hyphen-".into();
    assert!(manifest.validate().unwrap_err().contains("start and end"));

    manifest.id = "_underscore_start".into();
    assert!(manifest.validate().unwrap_err().contains("start and end"));
}

#[test]
fn manifest_validation_rejects_invalid_or_oversized_name() {
    let mut manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-id".into(),
        name: "Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Desc".into(),
        translations: Default::default(),
        verified: false,
    };

    manifest.name = "".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.name = "   ".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.name = "a".repeat(super::manifest::MAX_MANIFEST_NAME_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("maximum length"));

    manifest.name = "First Line\nSecond Line".into();
    assert!(manifest.validate().unwrap_err().contains("newlines"));
}

#[test]
fn manifest_validation_rejects_oversized_metadata_fields() {
    let mut manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-id".into(),
        name: "Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Desc".into(),
        translations: Default::default(),
        verified: false,
    };

    manifest.version = "v".repeat(super::manifest::MAX_MANIFEST_VERSION_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("version exceeds maximum length"));
    manifest.version = "1.0".into();

    manifest.author = "a".repeat(super::manifest::MAX_MANIFEST_AUTHOR_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("author exceeds maximum length"));
    manifest.author = "Author".into();

    manifest.license = "l".repeat(super::manifest::MAX_MANIFEST_LICENSE_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("license exceeds maximum length"));
    manifest.license = "MIT".into();

    manifest.description = "d".repeat(super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("description exceeds maximum length"));
}

#[test]
fn manifest_validation_rejects_invalid_or_oversized_translations() {
    let mut manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-id".into(),
        name: "Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Desc".into(),
        translations: Default::default(),
        verified: false,
    };

    // Oversized locale tag
    manifest.translations.insert("too_long_locale_tag".into(), LocalizedThemeText { name: "Name".into(), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("locale tag \"too_long_locale_tag\" exceeds maximum length"));
    manifest.translations.clear();

    // Invalid locale tag
    manifest.translations.insert("bad-lang!#".into(), LocalizedThemeText { name: "Name".into(), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("invalid locale tag"));
    manifest.translations.clear();

    // Oversized translated name
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "n".repeat(super::manifest::MAX_MANIFEST_NAME_LEN + 1), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("translated name for de exceeds maximum length"));
    manifest.translations.clear();

    // Newline in translated name
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "Line1\nLine2".into(), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("cannot contain newlines"));
    manifest.translations.clear();

    // Oversized translated description
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "Name".into(), description: "d".repeat(super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 1) });
    assert!(manifest.validate().unwrap_err().contains("translated description for de exceeds maximum length"));
}

#[test]
fn package_bundle_rejects_oversized_manifest_before_writing() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_pkg_val_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let pack_dir = temp_dir.join("pack");
    std::fs::create_dir_all(pack_dir.join("icons/sketch")).unwrap();
    std::fs::write(pack_dir.join("icons/sketch/line.svg"), br#"<svg viewBox="0 0 24 24"><line x1="0" y1="0" x2="24" y2="24"/></svg>"#).unwrap();

    let invalid_manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-id".into(),
        name: "Oversized Theme Name ".repeat(10), // > 64 chars
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Desc".into(),
        translations: Default::default(),
        verified: false,
    };

    let bundle_path = temp_dir.join("out.qicons");
    let res = package_bundle(&pack_dir, &invalid_manifest, &bundle_path);
    assert!(res.is_err(), "Packaging must fail for invalid manifest");
    assert!(res.unwrap_err().contains("maximum length"));
    assert!(!bundle_path.exists(), "Output file must not be created on validation failure");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn directory_pack_rejects_manifest_with_oversized_description() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_dir_val_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let pack_dir = temp_dir.join("pack");
    std::fs::create_dir_all(pack_dir.join("icons/sketch")).unwrap();
    std::fs::write(pack_dir.join("icons/sketch/line.svg"), br#"<svg viewBox="0 0 24 24"><line x1="0" y1="0" x2="24" y2="24"/></svg>"#).unwrap();

    let invalid_manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-theme".into(),
        name: "Valid Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "x".repeat(super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 10),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(pack_dir.join("manifest.ron"), invalid_manifest.to_ron().unwrap()).unwrap();

    // 1. Loading pack directly should fail with descriptive error
    let load_res = IconPack::from_directory(&pack_dir);
    assert!(load_res.is_err());
    assert!(load_res.unwrap_err().contains("description exceeds maximum length"));

    // 2. Inspecting folder should record the manifest error in rejected
    let rep = inspect_pack_directory(&pack_dir).unwrap();
    assert!(rep.rejected.iter().any(|rej| rej.path == "manifest.ron" && rej.reason.contains("description exceeds maximum length")));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn has_zalgo_correctly_detects_zalgo_and_allows_normal_text() {
    use super::manifest::has_zalgo;

    // Normal natural text (precomposed and normal decomposed)
    assert!(!has_zalgo("QymCAD Default Theme"));
    assert!(!has_zalgo("\u{0421}\u{0442}\u{0430}\u{043d}\u{0434}\u{0430}\u{0440}\u{0442}\u{043d}\u{0456} \u{0456}\u{043a}\u{043e}\u{043d}\u{043a}\u{0438}"));
    assert!(!has_zalgo("Café au lait"));
    assert!(!has_zalgo("Cafe\u{0301}")); // NFD French accent
    assert!(!has_zalgo("Tiếng Việt"));

    // Zalgo text: stacked diacritics
    assert!(has_zalgo("Ȟ̸̠e̷͚͝l̸͌ͅl̷͚̆o̴͙̓"));
    assert!(has_zalgo("Z̵̰͒A̸̜͊L̴͇͝G̸͎̀O̷͙͠"));
    assert!(has_zalgo("T\u{0300}\u{0301}\u{0302}est")); // 3 consecutive combining marks
    assert!(has_zalgo("\u{0300}LeadingMark")); // Isolated combining mark at start
    assert!(has_zalgo("Z̷a̷l̷g̷o̷")); // Heavy combining mark density
}

#[test]
fn manifest_validation_rejects_zalgo_in_all_fields() {
    let base_manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "valid-theme".into(),
        name: "Valid Theme".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Valid short description".into(),
        translations: Default::default(),
        verified: false,
    };

    let zalgo_sample = "T\u{0300}\u{0301}\u{0302}est";

    let mut m = base_manifest.clone();
    m.name = zalgo_sample.into();
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.version = zalgo_sample.into();
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.author = zalgo_sample.into();
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.license = zalgo_sample.into();
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.description = zalgo_sample.into();
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.translations.insert("de".into(), LocalizedThemeText { name: zalgo_sample.into(), description: "Beschreibung".into() });
    assert!(m.validate().unwrap_err().contains("Zalgo"));

    let mut m = base_manifest.clone();
    m.translations.insert("de".into(), LocalizedThemeText { name: "Name".into(), description: zalgo_sample.into() });
    assert!(m.validate().unwrap_err().contains("Zalgo"));
}

#[test]
fn archive_exceeding_max_file_size_is_rejected_without_reading() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_size_limit_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("oversized.qicons");
    let file = std::fs::File::create(&file_path).unwrap();
    // Sparse file with 17 MB size (limit is 16 MB)
    file.set_len(17 * 1024 * 1024).unwrap();

    let res = IconPack::from_archive(&file_path);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(res.is_err(), "expected error for oversized archive");
    let err = res.err().unwrap();
    assert!(err.contains("exceeds limit"), "expected size limit error, got: {err}");
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
        verified: false,
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
fn svg_validation_rejects_malformed_xml_tags() {
    let malformed = br#"<svg viewBox="0 0 24 24"><path></svg>"#;
    assert!(validate_svg(malformed).is_err(), "malformed XML with unclosed <path> should be rejected");

    let unclosed = br#"<svg viewBox="0 0 24 24"><g>"#;
    assert!(validate_svg(unclosed).is_err(), "unclosed tags should be rejected");

    let multiple_roots = br#"<svg viewBox="0 0 24 24"></svg><svg viewBox="0 0 24 24"></svg>"#;
    assert!(validate_svg(multiple_roots).is_err(), "multiple roots should be rejected");
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
        verified: false,
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
fn packager_rejects_icon_with_excessive_compression_ratio() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_bomb_pack_{}", std::process::id()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).unwrap();

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "bomb-pack".into(),
        name: "Bomb Pack".into(),
        version: "1.0".into(),
        author: "Author".into(),
        license: "MIT".into(),
        description: "Test".into(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    // 120 KB SVG with repeating comment that compresses > 250:1 ratio
    let mut bomb_svg = Vec::new();
    bomb_svg.extend_from_slice(b"<svg viewBox=\"0 0 24 24\"><!--");
    bomb_svg.extend(std::iter::repeat_n(b'A', 120 * 1024));
    bomb_svg.extend_from_slice(b"--><path d=\"M0 0h24v24z\"/></svg>");

    std::fs::write(icons_dir.join("line.svg"), &bomb_svg).unwrap();

    let out_archive = temp_dir.join("out.qicons");
    let res = package_bundle(&temp_dir, &manifest, &out_archive);
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(res.is_err(), "package_bundle must fail for directory with excessive compression ratio");
    let err = res.unwrap_err();
    assert!(err.contains("suspicious compression ratio") || err.contains("decompression bomb"), "got: {err}");
}

#[test]
fn custom_folder_with_special_id_is_still_directory_format() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_special_id_test_{}", std::process::id()));
    std::fs::create_dir_all(temp_dir.join("icons").join("sketch")).unwrap();

    // Create a directory pack that uses a known bundled id
    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "freecad-classic".into(),
        name: "My Custom FreeCAD".into(),
        version: "1.0".into(),
        author: "Me".into(),
        license: "MIT".into(),
        description: "Test".into(),
        translations: Default::default(),
        verified: false,
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();

    let pack = IconPack::from_directory(&temp_dir).unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Provenance must reflect physical source (Directory), NOT the theme ID
    assert_eq!(pack.format(), BundleFormat::Directory);
    assert!(pack.is_directory(), "a directory on disk must be recognized as directory even if ID is freecad-classic");
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
        verified: false,
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

    // Sleep to allow background watcher thread to poll and update egui loader cache
    std::thread::sleep(std::time::Duration::from_millis(350));

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
fn clean_directory_and_scan_removes_residual_tmp_files() {
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
    let orphan_tmp1 = icons_dir.join("line.svg.qymcad-999-0.tmp");
    let orphan_tmp2 = temp_root.join("icon.svg.qymcad-888-0.tmp");
    let orphan_tmp3 = icons_dir.join("stray_marker.tmp");
    std::fs::write(&orphan_tmp1, b"residual data 1").unwrap();
    std::fs::write(&orphan_tmp2, b"residual data 2").unwrap();
    std::fs::write(&orphan_tmp3, b"residual data 3").unwrap();

    assert!(orphan_tmp1.exists());
    assert!(orphan_tmp2.exists());
    assert!(orphan_tmp3.exists());

    // inspect_pack_directory must clean cleaner residual temporary files, while leaving unrelated user .tmp files intact
    let report = inspect_pack_directory(&temp_root).expect("pack directory is inspectable");
    assert_eq!(report.rejected.len(), 1, "cleanable icon with metadata should be rejected before cleaning");
    assert!(!orphan_tmp1.exists(), "orphan_tmp1 must be removed during inspection");
    assert!(!orphan_tmp2.exists(), "orphan_tmp2 must be removed during inspection");
    assert!(orphan_tmp3.exists(), "stray_marker.tmp is not a cleaner tmp file and must not be touched");

    // Re-create an orphan tmp file and verify clean_directory_icons also cleans it
    std::fs::write(&orphan_tmp1, b"residual data before clean").unwrap();
    let pack = IconPack::from_directory(&temp_root).expect("pack loads");
    let clean_report = clean_directory_icons(&pack).expect("cleaning succeeds");
    assert_eq!(clean_report.cleaned.len(), 1);
    assert!(!orphan_tmp1.exists(), "residual tmp file must be cleaned after clean_directory_icons");
    assert!(orphan_tmp3.exists(), "stray_marker.tmp must remain untouched after clean_directory_icons");

    // Ensure no cleaner tmp files remain in the pack directory
    let mut remaining_cleaner_tmps = Vec::new();
    for entry in walkdir_cleaner_tmps(&temp_root) {
        remaining_cleaner_tmps.push(entry);
    }
    assert!(remaining_cleaner_tmps.is_empty(), "no cleaner .tmp files should remain in pack directory");

    let _ = std::fs::remove_dir_all(&temp_root);
}

fn walkdir_cleaner_tmps(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut tmps = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                tmps.extend(walkdir_cleaner_tmps(&path));
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(super::is_cleaner_temp_file) {
                tmps.push(path);
            }
        }
    }
    tmps
}

#[test]
fn test_is_cleaner_temp_file_pattern_matching() {
    // Valid cleaner temp file patterns
    assert!(super::is_cleaner_temp_file("line.svg.qymcad-999-0.tmp"));
    assert!(super::is_cleaner_temp_file("icon.svg.qymcad-12345-42.tmp"));
    assert!(super::is_cleaner_temp_file("circle_3pt.svg.qymcad-1-0.tmp"));
    assert!(super::is_cleaner_temp_file("nested.part.svg.qymcad-500-12.tmp"));

    // User or non-cleaner files ending in .tmp must be preserved
    assert!(!super::is_cleaner_temp_file("stray_marker.tmp"));
    assert!(!super::is_cleaner_temp_file("backup.tmp"));
    assert!(!super::is_cleaner_temp_file("line.tmp"));
    assert!(!super::is_cleaner_temp_file("line.svg.tmp"));
    assert!(!super::is_cleaner_temp_file(".tmp"));

    // Malformed markers
    assert!(!super::is_cleaner_temp_file("line.svg.qymcad-abc-0.tmp"));
    assert!(!super::is_cleaner_temp_file("line.svg.qymcad-999-xyz.tmp"));
    assert!(!super::is_cleaner_temp_file("line.svg.qymcad-999.tmp"));
    assert!(!super::is_cleaner_temp_file("line.svg.qymcad-999-0-1.tmp"));
    assert!(!super::is_cleaner_temp_file("line.svg.qymcad--.tmp"));
}

#[cfg(unix)]
#[test]
fn test_cleanup_residual_tmp_files_avoids_symlink_loops() {
    let temp_root = std::env::temp_dir().join(format!("qymcad_symlink_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&temp_root).unwrap();

    let cleaner_tmp = temp_root.join("icon.svg.qymcad-42-0.tmp");
    let user_tmp = temp_root.join("important_user_work.tmp");
    std::fs::write(&cleaner_tmp, b"cleaner temp").unwrap();
    std::fs::write(&user_tmp, b"user temp").unwrap();

    // Create a circular directory symlink pointing back to temp_root
    let symlink_loop = temp_root.join("loop_dir");
    std::os::unix::fs::symlink(&temp_root, &symlink_loop).unwrap();

    // Must return without infinite recursion, delete cleaner_tmp, and preserve user_tmp
    super::cleanup_residual_tmp_files(&temp_root);

    assert!(!cleaner_tmp.exists(), "cleaner temporary file must be removed");
    assert!(user_tmp.exists(), "user temporary file must be preserved");

    let _ = std::fs::remove_file(&symlink_loop);
    let _ = std::fs::remove_dir_all(&temp_root);
}
