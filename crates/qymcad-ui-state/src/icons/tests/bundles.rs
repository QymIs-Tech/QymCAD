//! Checks of packing a theme into one file and reading it back: the manifest, the trailer, sizes, zip bombs and paths.

use super::*;

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
fn bundle_format_and_checksum_detection() {
    let default_pack = load_default_pack().expect("embedded default pack");
    assert_eq!(default_pack.format(), BundleFormat::Embedded);
    assert!(default_pack.checksum_matches());
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
    assert!(loaded_archive.checksum_matches());
    assert!(loaded_archive.is_archive());
    assert!(!loaded_archive.is_directory());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn qicons_modified_after_packaging_is_rejected() {
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
    };
    std::fs::write(pack_dir.join("manifest.ron"), manifest.to_ron().unwrap()).unwrap();
    let valid_svg = br#"<svg viewBox="0 0 24 24"><line x1="0" y1="0" x2="24" y2="24"/></svg>"#;
    std::fs::write(pack_dir.join("icons").join("sketch").join("line.svg"), valid_svg).unwrap();

    // Package a .qicons bundle with a checksum trailer.
    let bundle_path = temp_dir.join("official.qicons");
    let report = package_bundle(&pack_dir, &manifest, &bundle_path).expect("bundle packaging succeeds");
    assert_eq!(report.included.len(), 1);

    let pack = IconPack::from_archive(&bundle_path).expect("bundle loads cleanly");
    assert_eq!(pack.format(), BundleFormat::Package);
    assert_eq!(pack.trailer_check, Some(TrailerCheck::IntegrityOk));
    assert!(pack.checksum_matches());
    assert_eq!(pack.get_svg_for_id(IconId::SketchLine).unwrap(), valid_svg);

    // Alter the trailer while keeping the ZIP valid; reject before opening the ZIP.
    let mut changed_bytes = std::fs::read(&bundle_path).unwrap();
    let hash_start = changed_bytes.len() - crate::icons::sha256::QICONS_TRAILER_LEN;
    changed_bytes[hash_start] ^= 0xFF;
    let changed_path = temp_dir.join("changed.qicons");
    std::fs::write(&changed_path, &changed_bytes).unwrap();
    let changed_err = IconPack::from_archive(&changed_path).unwrap_err();
    assert_eq!(changed_err, crate::icons::pack::INVALID_PACKAGE_ERROR);
    assert_eq!(IconPack::from_zip_bytes(&changed_bytes).unwrap_err(), crate::icons::pack::INVALID_PACKAGE_ERROR);

    std::fs::write(&bundle_path, &changed_bytes).unwrap();
    assert_eq!(pack.get_svg_for_id(IconId::SketchLine).unwrap(), valid_svg);
    assert_eq!(IconPack::from_archive(&bundle_path).unwrap_err(), crate::icons::pack::INVALID_PACKAGE_ERROR);
    assert_eq!(pack.archive_snapshot().unwrap_err(), crate::icons::pack::INVALID_PACKAGE_ERROR);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn archive_without_package_trailer_is_rejected_before_zip_parsing() {
    use std::io::Write;
    let temp_dir = std::env::temp_dir().join(format!("qymcad_qicons_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let valid_svg = br#"<svg viewBox="0 0 32 32"><circle cx="16" cy="16" r="10"/></svg>"#;

    // 1. Generic .zip extension must be rejected
    let zip_path = temp_dir.join("my-cool-pack.zip");
    std::fs::write(&zip_path, b"dummy zip").unwrap();
    let zip_err = IconPack::from_archive(&zip_path).unwrap_err();
    assert!(zip_err.contains("must have .qicons extension"), "got: {zip_err}");

    // A plain ZIP is rejected before its missing manifest is inspected.
    let no_manifest_path = temp_dir.join("no-manifest.qicons");
    let file = std::fs::File::create(&no_manifest_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("icons/sketch/circle.svg", options).unwrap();
    zip.write_all(valid_svg).unwrap();
    zip.finish().unwrap();
    let no_manifest_err = IconPack::from_archive(&no_manifest_path).unwrap_err();
    assert_eq!(no_manifest_err, crate::icons::pack::INVALID_PACKAGE_ERROR);

    // A renamed ZIP with a manifest is still not a QymCAD package.
    let qicons_path = temp_dir.join("renamed-zip.qicons");
    let file = std::fs::File::create(&qicons_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    let manifest = r#"(id: "manual-pack", name: "Manual Pack")"#;
    zip.start_file("manifest.ron", options).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.start_file("icons/sketch/circle.svg", options).unwrap();
    zip.write_all(valid_svg).unwrap();
    zip.finish().unwrap();

    assert_eq!(IconPack::from_archive(&qicons_path).unwrap_err(), crate::icons::pack::INVALID_PACKAGE_ERROR);
    assert_eq!(IconPack::from_zip_bytes(&std::fs::read(&qicons_path).unwrap()).unwrap_err(), crate::icons::pack::INVALID_PACKAGE_ERROR);

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
    let mut bytes = std::fs::read(&bomb_path).unwrap();
    crate::icons::sha256::append_qicons_trailer(&mut bytes);
    std::fs::write(&bomb_path, bytes).unwrap();

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
    let mut bytes = std::fs::read(&slip_path).unwrap();
    crate::icons::sha256::append_qicons_trailer(&mut bytes);
    std::fs::write(&slip_path, bytes).unwrap();

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
    };

    manifest.id = "".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.id = "a".repeat(super::super::manifest::MAX_MANIFEST_ID_LEN + 1);
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
    };

    manifest.name = "".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.name = "   ".into();
    assert!(manifest.validate().unwrap_err().contains("empty"));

    manifest.name = "a".repeat(super::super::manifest::MAX_MANIFEST_NAME_LEN + 1);
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
    };

    manifest.version = "v".repeat(super::super::manifest::MAX_MANIFEST_VERSION_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("version exceeds maximum length"));
    manifest.version = "1.0".into();

    manifest.author = "a".repeat(super::super::manifest::MAX_MANIFEST_AUTHOR_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("author exceeds maximum length"));
    manifest.author = "Author".into();

    manifest.license = "l".repeat(super::super::manifest::MAX_MANIFEST_LICENSE_LEN + 1);
    assert!(manifest.validate().unwrap_err().contains("license exceeds maximum length"));
    manifest.license = "MIT".into();

    manifest.description = "d".repeat(super::super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 1);
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
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "n".repeat(super::super::manifest::MAX_MANIFEST_NAME_LEN + 1), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("translated name for de exceeds maximum length"));
    manifest.translations.clear();

    // Newline in translated name
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "Line1\nLine2".into(), description: "Desc".into() });
    assert!(manifest.validate().unwrap_err().contains("cannot contain newlines"));
    manifest.translations.clear();

    // Oversized translated description
    manifest.translations.insert("de".into(), LocalizedThemeText { name: "Name".into(), description: "d".repeat(super::super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 1) });
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
        description: "x".repeat(super::super::manifest::MAX_MANIFEST_DESCRIPTION_LEN + 10),
        translations: Default::default(),
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
    use super::super::manifest::has_zalgo;

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
fn archive_pack_retains_icons_in_memory_without_reopening_disk_archive() {
    let temp_dir = std::env::temp_dir().join(format!("qymcad_archive_mem_test_{}", std::process::id()));
    let icons_dir = temp_dir.join("icons").join("sketch");
    std::fs::create_dir_all(&icons_dir).expect("creates test dirs");

    let svg_content = br#"<svg viewBox="0 0 64 64"><line x1="0" y1="0" x2="64" y2="64"/></svg>"#;
    std::fs::write(icons_dir.join("line.svg"), svg_content).expect("writes svg");
    let pack_icon = br#"<svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="20"/></svg>"#;
    std::fs::write(temp_dir.join("icon.svg"), pack_icon).expect("writes pack icon");

    let manifest = IconManifest {
        package_type: PackageType::IconTheme,
        id: "archive-mem-pack".to_string(),
        name: "Archive Mem Pack".to_string(),
        version: "1.0.0".to_string(),
        author: "Tester".to_string(),
        license: "MIT".to_string(),
        description: "Test".to_string(),
        translations: Default::default(),
    };
    std::fs::write(temp_dir.join("manifest.ron"), manifest.to_ron().expect("manifest serializes")).expect("writes manifest");

    let archive_path = temp_dir.join("archive-mem-pack.qicons");
    package_bundle(&temp_dir, &manifest, &archive_path).expect("packaging succeeds");

    let pack = IconPack::from_archive(&archive_path).expect("loading archive succeeds");
    assert!(pack.is_archive());
    assert_eq!(pack.source_path(), Some(archive_path.as_path()));

    // Delete the archive file on disk. Icon retrieval must continue from memory,
    // exactly like from_embedded_zip_bytes does.
    std::fs::remove_file(&archive_path).expect("removes archive file");

    let svg = pack.get_svg_for_id(IconId::SketchLine);
    assert_eq!(svg.as_deref(), Some(&svg_content[..]));
    assert_eq!(pack.get_pack_icon_svg(), pack_icon);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn available_icons_checks_presence_without_validating_svg_content() {
    let mut map = std::collections::HashMap::new();
    map.insert("icons/sketch/line.svg".to_string(), b"not an svg".to_vec());
    let pack = IconPack {
        manifest: IconManifest {
            package_type: PackageType::IconTheme,
            id: "lightweight-presence".to_string(),
            name: "Presence".to_string(),
            version: "1.0.0".to_string(),
            author: "Tester".to_string(),
            license: "MIT".to_string(),
            description: String::new(),
            translations: Default::default(),
        },
        source: PackSource::Memory(map),
        trailer_check: None,
        duplicate_conflict: None,
    };

    assert!(pack.has_svg_for_id(IconId::SketchLine));
    assert!(pack.available_icons().contains(&IconId::SketchLine));
    assert_eq!(pack.coverage().present, 1);
    assert!(pack.get_svg_for_id(IconId::SketchLine).is_none());
}

#[test]
fn compression_ratio_rejects_boundary_exceeding_ratio_without_truncation() {
    use std::io::{Cursor, Write};
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buffer));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("icon.svg", options).unwrap();
        zip.write_all(b"<svg></svg>").unwrap();
        zip.finish().unwrap();
    }

    let magic = [0x50, 0x4b, 0x01, 0x02];
    let pos = buffer.windows(4).position(|w| w == magic).expect("found central directory header");
    let compressed_bytes = 300_u32.to_le_bytes();
    let uncompressed_bytes = 75001_u32.to_le_bytes();
    buffer[pos + 20..pos + 24].copy_from_slice(&compressed_bytes);
    buffer[pos + 24..pos + 28].copy_from_slice(&uncompressed_bytes);

    let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).expect("valid zip");
    let res = super::super::pack::validate_archive_safety(&mut archive);
    assert!(res.is_err(), "must reject ratio strictly greater than 250:1 even if integer division truncates to 250");
}
