//! Checks of one SVG: the cleaner, the validator, the view box, the colours and the tokens of the palette.

use super::*;

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
        },
        source: PackSource::Memory(icons),
        trailer_check: None,
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
        id: "custom-classic".to_string(),
        name: "Custom Classic".to_string(),
        version: "1.0.0".to_string(),
        author: "Community Contributors".to_string(),
        license: "LGPL-2.1-or-later".to_string(),
        description: "Classic colored tool icons".to_string(),
        translations: Default::default(),
    };

    let ron_str = manifest.to_ron().expect("serialization succeeds");
    let parsed = IconManifest::parse_ron(&ron_str).expect("parsing succeeds");

    assert_eq!(parsed.id, "custom-classic");
    assert_eq!(parsed.name, "Custom Classic");
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
    let no_readme = IconPack { manifest: folder.manifest.clone(), source: PackSource::Memory(HashMap::new()), trailer_check: None, duplicate_conflict: None };
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
        },
        source: PackSource::Memory(map_a),
        trailer_check: None,
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
        },
        source: PackSource::Memory(map_b),
        trailer_check: None,
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
fn shared_svg_validator_rejects_external_references_with_precise_locations() {
    let cases = [
        ("href=\"http://example.test/a.svg#icon\"", "http", "href"),
        ("href=\"https://example.test/a.svg#icon\"", "https", "href"),
        ("href=\"file:///tmp/a.svg#icon\"", "file", "href"),
        ("href=\"//example.test/a.svg#icon\"", "protocol-relative", "href"),
        ("href=\"../a.svg#icon\"", "relative file", "href"),
        ("xlink:href=\"https://example.test/a.svg#icon\"", "xlink", "xlink:href"),
        ("fill=\"url(https://example.test/fill.svg#paint)\"", "CSS URL", "url"),
        ("style=\"fill: url(https://example.test/fill.svg#paint)\"", "inline style", "url"),
    ];

    for (attribute, case, location) in cases {
        let source = format!("<svg viewBox=\"0 0 24 24\">\n    <use {attribute}/></svg>");
        let error = validate_svg_detailed(source.as_bytes(), super::super::SvgPurpose::Icon).expect_err(case);
        assert_eq!(error.line, 2, "{case}: {error}");
        assert_eq!(error.column, source.lines().nth(1).unwrap().find(location).unwrap() + 1, "{case}: {error}");
        assert!(error.message.contains("external"), "{case}: {error}");
    }

    let stylesheet = b"<svg viewBox=\"0 0 24 24\">\n  <style>@import url(https://example.test/theme.css);</style></svg>";
    let error = validate_svg_detailed(stylesheet, super::super::SvgPurpose::Icon).unwrap_err();
    assert_eq!(error.line, 2);
    assert!(error.message.contains("external CSS"));
}

#[test]
fn shared_svg_validator_preserves_legacy_metadata_and_token_depth_rules() {
    for source in [br#"<svg viewBox="0 0 24 24"><path data-note="javascript:alert(1)"/></svg>"#.as_slice(), br#"<svg viewBox="0 0 24 24"><ns0:metadata/></svg>"#.as_slice()] {
        assert!(validate_svg(source).is_err(), "legacy prohibited content must remain rejected");
    }

    let mut nested = String::from("<svg viewBox=\"0 0 24 24\"><path fill=\"");
    for _ in 0..12 {
        nested.push_str("var(--icon-stroke, ");
    }
    nested.push_str("#000");
    for _ in 0..12 {
        nested.push(')');
    }
    nested.push_str("\"/></svg>");

    let error = validate_svg_detailed(nested.as_bytes(), super::super::SvgPurpose::Icon).unwrap_err();
    assert!(error.message.contains("excessive var(...) nesting depth"), "{error}");
    assert!(error.line == 1 && error.column > 1, "nested token diagnostic needs a source location: {error}");
}

#[test]
fn svg_cleaner_and_loader_share_external_reference_policy() {
    let source = br#"<svg viewBox="0 0 24 24"><use href="https://example.test/shape.svg#part"/><path d="M0 0h24"/></svg>"#;
    let error = validate_svg_detailed(source, super::super::SvgPurpose::Icon).unwrap_err();
    assert!(error.message.contains("external SVG references"));

    let cleaned = clean_svg(source).expect("cleaner removes the reference rejected by the loader");
    validate_svg(&cleaned).expect("cleaned SVG passes the same validator used by loading");
    assert!(!String::from_utf8_lossy(&cleaned).contains("https://"));
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
        validate_svg(valid.as_bytes()).unwrap();
    }

    // Invalid variables must be rejected (including in nested vars)
    for (invalid, expected) in [
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke)"/></svg>"##, "must specify a fallback color"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--nonexistent-token, #fff)"/></svg>"##, "unknown icon token"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(icon-stroke, #fff)"/></svg>"##, "must start with '--'"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, )"/></svg>"##, "empty fallback color"),
        (r##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, var(--bad-token, #333))"/></svg>"##, "unknown icon token"),
    ] {
        let err = validate_svg(invalid.as_bytes()).unwrap_err();
        assert!(err.contains(expected), "expected '{expected}' in '{err}'");
    }
}

#[test]
fn resolve_icon_tokens_substitutes_active_palette_and_preserves_static() {
    let dark_pal = qymcad_scheme::dark();
    let light_pal = qymcad_scheme::light();

    let svg_with_tokens = br##"<svg viewBox="0 0 24 24"><path stroke="var(--icon-stroke, #111)" fill="var(--icon-sketch-primary, #222)"/><rect fill="currentColor"/></svg>"##;

    let dark_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(svg_with_tokens, &dark_pal)).unwrap();
    assert!(dark_resolved.contains("stroke=\"#000000\""));
    assert!(dark_resolved.contains("fill=\"#18F2F2\""));
    assert!(dark_resolved.contains("fill=\"#000000\""));

    let light_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(svg_with_tokens, &light_pal)).unwrap();
    assert!(light_resolved.contains("stroke=\"#808080\""));
    assert!(light_resolved.contains("fill=\"#18F2F2\""));
    assert!(light_resolved.contains("fill=\"#808080\""));

    // Dimmed underlay token
    let dimmed_svg = br##"<svg viewBox="0 0 24 24"><path fill="var(--icon-dimmed, #FFFFFF)"/></svg>"##;
    let dark_dimmed = String::from_utf8(super::super::manager::resolve_icon_tokens(dimmed_svg, &dark_pal)).unwrap();
    assert!(dark_dimmed.contains("fill=\"#7C7C7C\""));
    let light_dimmed = String::from_utf8(super::super::manager::resolve_icon_tokens(dimmed_svg, &light_pal)).unwrap();
    assert!(light_dimmed.contains("fill=\"#ABABAB\""));

    // Unknown token uses fallback
    let unknown_token_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--custom-fallback, #AABBCC)"/></svg>"##;
    let fallback_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(unknown_token_svg, &dark_pal)).unwrap();
    assert!(fallback_resolved.contains("stroke=\"#AABBCC\""));

    // Nested paren in fallback
    let nested_fallback_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--custom-rgb, rgba(10, 20, 30, 0.5))"/></svg>"##;
    let nested_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(nested_fallback_svg, &dark_pal)).unwrap();
    assert!(nested_resolved.contains("stroke=\"rgba(10, 20, 30, 0.5)\""));

    // Nested var expressions
    let nested_vars_svg = br##"<svg viewBox="0 0 24 24"><path stroke="var(--unknown-outer, var(--icon-accent, #000))" fill="var(--icon-stroke, var(--bad-token, #111))" d="M0 0" color="var(--u1, var(--u2, var(--u3, #112233)))"/></svg>"##;
    let nested_vars_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(nested_vars_svg, &dark_pal)).unwrap();
    assert!(nested_vars_resolved.contains("stroke=\"#18F2F2\""), "outer unknown should resolve inner icon-accent token, got: {nested_vars_resolved}");
    assert!(nested_vars_resolved.contains("fill=\"#000000\""), "outer defined icon-stroke should resolve directly without fallback, got: {nested_vars_resolved}");
    assert!(nested_vars_resolved.contains("color=\"#112233\""), "deeply nested unknown vars should resolve to final fallback, got: {nested_vars_resolved}");

    // Static SVG without tokens is byte-for-byte identical
    let static_svg = br##"<svg viewBox="0 0 24 24"><path stroke="#FF0000" fill="#00FF00"/></svg>"##;
    let static_resolved = super::super::manager::resolve_icon_tokens(static_svg, &dark_pal);
    assert_eq!(static_resolved, static_svg.to_vec());

    // Element id, class or text node containing currentColor must NOT be corrupted
    let id_svg = br##"<svg viewBox="0 0 24 24"><path id="currentColor" class="currentColor" stroke="currentColor" fill="currentColor"/><stop stop-color="currentColor"/><text> currentColor </text><g style="fill: currentColor; stroke: currentColor"/></svg>"##;
    let id_resolved = String::from_utf8(super::super::manager::resolve_icon_tokens(id_svg, &dark_pal)).unwrap();
    assert!(id_resolved.contains("id=\"currentColor\""), "element id must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("class=\"currentColor\""), "element class must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("<text> currentColor </text>"), "text content must not be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("stroke=\"#000000\""), "stroke currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("fill=\"#000000\""), "fill currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("stop-color=\"#000000\""), "stop-color currentColor must be replaced, got: {id_resolved}");
    assert!(id_resolved.contains("style=\"fill: #000000; stroke: #000000\""), "style properties must be replaced, got: {id_resolved}");
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
