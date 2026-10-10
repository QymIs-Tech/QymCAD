//! .qicons package bundle serialization and compression.

use std::path::Path;

use super::discovery::{inspect_pack_directory, localized_description_tag, localized_readme_tag, ValidationReport};
use super::hygiene::{validate_svg, validate_svg_structural};
use super::manifest::IconManifest;

/// Package an icon folder into any writer (e.g. file or in-memory cursor).
/// Only valid, verified icons that match a known `IconId` are packaged into the archive.
/// Extraneous files and invalid SVGs are automatically excluded.
pub fn package_bundle_to_writer<W: std::io::Write + std::io::Seek>(source_dir: impl AsRef<Path>, manifest: &IconManifest, mut writer: W) -> Result<ValidationReport, String> {
    manifest.validate()?;
    let source_dir = source_dir.as_ref();
    let report = inspect_pack_directory(source_dir)?;

    if report.included.is_empty() {
        return Err("cannot package bundle: 0 valid CAD icons found in icons/ directory".to_string());
    }

    let mut mem_buf = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut mem_buf));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Write manifest.ron (marked verified)
    let mut manifest = manifest.clone();
    manifest.verified = true;
    let ron_text = manifest.to_ron().map_err(|e| format!("manifest serialization error: {e}"))?;
    zip.start_file("manifest.ron", options).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, ron_text.as_bytes()).map_err(|e| e.to_string())?;

    // 2. Write optional metadata and preview files from root if present
    for doc in &["LICENSE", "LICENSE.txt", "LICENSE.md", "README.md", "README.txt", "description.md", "description.txt", "preview.svg", "preview.png", "preview.webp", "icon.svg"] {
        let doc_path = source_dir.join(doc);
        if doc_path.is_file() {
            let limit = if *doc == "icon.svg" {
                super::pack::MAX_ICON_SVG_SIZE
            } else if doc.starts_with("preview.") {
                super::pack::MAX_SINGLE_FILE_UNCOMPRESSED_SIZE
            } else {
                super::pack::MAX_TEXT_FILE_SIZE
            };
            if std::fs::metadata(&doc_path).ok().is_some_and(|m| m.len() > limit) {
                continue;
            }
            if let Ok(content) = std::fs::read(&doc_path) {
                if content.len() as u64 > limit {
                    continue;
                }
                if *doc == "icon.svg" && validate_svg(&content).is_err() {
                    continue;
                }
                if *doc == "preview.svg" && validate_svg_structural(&content, false).is_err() {
                    continue;
                }
                let _ = zip.start_file(*doc, options);
                let _ = std::io::Write::write_all(&mut zip, &content);
            }
        }
    }

    let mut localized_docs = std::fs::read_dir(source_dir)
        .map_err(|err| format!("cannot list localized documentation: {err}"))?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (entry.path().is_file() && (localized_readme_tag(&name).is_some() || localized_description_tag(&name).is_some())).then_some(name)
        })
        .collect::<Vec<_>>();
    localized_docs.sort();
    for name in localized_docs {
        let path = source_dir.join(&name);
        let metadata = std::fs::metadata(&path).map_err(|err| format!("cannot inspect {name}: {err}"))?;
        if metadata.len() > super::pack::MAX_TEXT_FILE_SIZE {
            return Err(format!("{name} exceeds maximum text size"));
        }
        let content = std::fs::read_to_string(&path).map_err(|err| format!("cannot read {name} as UTF-8: {err}"))?;
        zip.start_file(&name, options).map_err(|err| err.to_string())?;
        std::io::Write::write_all(&mut zip, content.as_bytes()).map_err(|err| err.to_string())?;
    }

    // 3. Write ONLY the validated icons
    let icons_dir = source_dir.join("icons");
    for id in &report.included {
        let rel_path = format!("{}.svg", id.relative_path());
        let src_file = icons_dir.join(&rel_path);
        let data = std::fs::read(&src_file).map_err(|e| format!("failed to read {}: {e}", src_file.display()))?;
        zip.start_file(format!("icons/{rel_path}"), options).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut zip, &data).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;

    // Verify generated zip passes all archive safety requirements
    let mut check_zip = zip::ZipArchive::new(std::io::Cursor::new(&mem_buf)).map_err(|e| e.to_string())?;
    super::pack::validate_archive_safety(&mut check_zip).map_err(|e| format!("generated bundle rejected by safety rules: {e}"))?;

    std::io::Write::write_all(&mut writer, &mem_buf).map_err(|e| e.to_string())?;

    Ok(report)
}

/// Sealed in-memory bundle bytes and its validation report.
#[derive(Clone, Debug)]
pub struct PackagedBundle {
    pub bytes: Vec<u8>,
    pub report: ValidationReport,
}

/// Package an icon folder into an in-memory byte buffer, signed with QymCAD verification trailer.
pub fn package_bundle_to_bytes(source_dir: impl AsRef<Path>, manifest: &IconManifest) -> Result<PackagedBundle, String> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = cursor;
    let report = package_bundle_to_writer(source_dir, manifest, &mut writer)?;
    let mut bytes = writer.into_inner();
    super::sha256::append_qicons_trailer(&mut bytes);
    Ok(PackagedBundle { bytes, report })
}

/// Package an icon folder into a `.qicons` bundle file on disk.
/// Automatically validates all icons, guarantees that only verified CAD icons enter the bundle,
/// and seals the bundle with a trailing cryptographic SHA-256 integrity record.
pub fn package_bundle(source_dir: impl AsRef<Path>, manifest: &IconManifest, output_archive: impl AsRef<Path>) -> Result<ValidationReport, String> {
    let packaged = package_bundle_to_bytes(source_dir, manifest)?;
    std::fs::write(output_archive.as_ref(), packaged.bytes).map_err(|e| e.to_string())?;
    Ok(packaged.report)
}
