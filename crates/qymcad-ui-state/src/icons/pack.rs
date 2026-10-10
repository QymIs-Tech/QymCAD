//! Icon pack loading from disk directories, archives, or memory.

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use super::id::{IconId, ALL_ICONS, DEFAULT_THEME_ID};
use super::manifest::{locale_fallbacks, IconManifest};

const DEFAULT_PACK_ICON_SVG: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon-themes/shapr-alike/icon.svg"));

/// Source storage for an icon pack.
#[derive(Debug, Clone)]
pub enum PackSource {
    /// Folder on disk containing `manifest.ron` and `icons/`.
    Directory(PathBuf),
    /// Packaged .qicons bundle containing `manifest.ron` and `icons/` strictly at root level.
    Archive(PathBuf),
    /// In-memory map (used for tests and virtual bundles).
    Memory(HashMap<String, Vec<u8>>),
    /// Embedded in the application executable binary.
    Embedded(HashMap<String, Vec<u8>>),
}

/// The underlying format and provenance of an icon bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleFormat {
    /// Folder containing unpacked SVG files. Live editing and live watch supported.
    Directory,
    /// Packaged bundle file (.qicons).
    Package,
    /// Embedded in the application executable binary.
    Embedded,
}

impl BundleFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Directory => "Folder",
            Self::Package => "Package",
            Self::Embedded => "Built-in",
        }
    }
}

/// Modification timestamp and length recording a file's state on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSignature {
    pub modified: std::time::SystemTime,
    pub len: u64,
}

/// Image asset data and format for theme previews.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePreviewAsset {
    pub data: Vec<u8>,
    pub extension: &'static str,
}

/// Maximum file size permitted for an icon archive file on disk (16 MB).
pub const MAX_ARCHIVE_FILE_SIZE: u64 = 16 * 1024 * 1024;
/// Maximum number of file entries permitted in an icon archive (prevents zip bomb exhaustion).
pub const MAX_ARCHIVE_ENTRIES: usize = 1_000;
/// Maximum cumulative uncompressed size of all files across an entire icon archive (32 MB).
pub const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 32 * 1024 * 1024;
/// Maximum uncompressed size permitted for any single file inside an icon archive (2 MB).
pub const MAX_SINGLE_FILE_UNCOMPRESSED_SIZE: u64 = 2 * 1024 * 1024;
/// Maximum uncompressed size permitted for an individual SVG icon (512 KB).
pub const MAX_ICON_SVG_SIZE: u64 = 512 * 1024;
/// Maximum allowed size for manifest.ron (64 KB).
pub const MAX_MANIFEST_SIZE: u64 = 64 * 1024;
/// Maximum allowed size for documentation text files (512 KB).
pub const MAX_TEXT_FILE_SIZE: u64 = 512 * 1024;
/// Maximum compression ratio before triggering decompression bomb rejection for files > 64 KB.
pub const MAX_COMPRESSION_RATIO: u64 = 250;

/// Validate a ZIP archive against decompression bombs (Zip Bombs), zip slip, and resource exhaustion.
pub fn validate_archive_safety<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>) -> Result<(), String> {
    if zip.len() > MAX_ARCHIVE_ENTRIES {
        return Err(format!("archive contains too many files: {} (limit is {})", zip.len(), MAX_ARCHIVE_ENTRIES));
    }

    let mut total_uncompressed: u64 = 0;

    for i in 0..zip.len() {
        let file = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name();

        // 1. Path traversal / Zip Slip protection
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') || name.contains(':') {
            return Err(format!("insecure file path inside archive: {name}"));
        }

        let uncompressed = file.size();
        let compressed = file.compressed_size();

        // 2. Single file size ceiling (e.g. rejects gigantic multi-gigabyte files)
        if uncompressed > MAX_SINGLE_FILE_UNCOMPRESSED_SIZE {
            return Err(format!("file {name} exceeds maximum uncompressed size: {uncompressed} bytes (limit is {MAX_SINGLE_FILE_UNCOMPRESSED_SIZE})"));
        }

        // 3. Compression ratio check (detects Deflate bomb payloads)
        if compressed == 0 && uncompressed > 0 {
            return Err(format!("anomalous entry in {name} with zero compressed size but non-zero uncompressed size"));
        }
        if compressed > 0 && uncompressed > 64 * 1024 {
            let ratio = uncompressed / compressed;
            if ratio > MAX_COMPRESSION_RATIO {
                return Err(format!("suspicious compression ratio in {name} ({ratio}:1, possible decompression bomb)"));
            }
        }

        total_uncompressed = total_uncompressed.saturating_add(uncompressed);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE {
            return Err(format!("archive total uncompressed size exceeds limit: {total_uncompressed} bytes (limit is {MAX_TOTAL_UNCOMPRESSED_SIZE})"));
        }
    }

    Ok(())
}

fn archive_entries<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut map = HashMap::new();
    for index in 0..zip.len() {
        let file = zip.by_index(index).map_err(|err| err.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.take(MAX_SINGLE_FILE_UNCOMPRESSED_SIZE + 1).read_to_end(&mut data).map_err(|err| err.to_string())?;
        if data.len() as u64 > MAX_SINGLE_FILE_UNCOMPRESSED_SIZE {
            return Err(format!("file {name} exceeds safe memory limit"));
        }
        map.insert(name, data);
    }
    Ok(map)
}

/// Describes an ID conflict when two or more packs share the same identifier,
/// or when a custom pack uses the reserved default theme identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateConflict {
    /// The conflicting theme identifier.
    pub conflicting_id: String,
    /// Whether this conflict is specifically with the reserved default theme ID.
    pub is_default_theme: bool,
}

/// A loaded icon pack ready for icon retrieval.
#[derive(Debug, Clone)]
pub struct IconPack {
    pub manifest: IconManifest,
    pub source: PackSource,
    /// Whether this package had its verification trailer tampered with or corrupted.
    pub is_tampered: bool,
    /// Whether this pack has an identifier collision with an earlier loaded theme or the default theme.
    pub duplicate_conflict: Option<DuplicateConflict>,
}

impl IconPack {
    /// Whether this pack has an identifier collision.
    pub fn has_id_conflict(&self) -> bool {
        self.duplicate_conflict.is_some()
    }

    /// Unique key used for selection in UI and caching.
    pub fn selection_key(&self) -> String {
        if self.has_id_conflict() {
            match &self.source {
                PackSource::Directory(p) | PackSource::Archive(p) => format!("conflict:{}:{}", self.manifest.id, p.display()),
                _ => format!("conflict:{}:memory", self.manifest.id),
            }
        } else {
            self.manifest.id.clone()
        }
    }

    /// Display string for the pack's location on disk or in memory.
    pub fn source_display(&self) -> String {
        match &self.source {
            PackSource::Directory(p) | PackSource::Archive(p) => p.display().to_string(),
            PackSource::Memory(_) => "memory".to_string(),
            PackSource::Embedded(_) => "embedded".to_string(),
        }
    }

    /// Path to the pack on disk, if loaded from a directory or archive.
    pub fn source_path(&self) -> Option<&Path> {
        match &self.source {
            PackSource::Directory(p) | PackSource::Archive(p) => Some(p.as_path()),
            _ => None,
        }
    }

    /// The provenance and format of this icon pack.
    pub fn format(&self) -> BundleFormat {
        match &self.source {
            PackSource::Directory(_) => BundleFormat::Directory,
            PackSource::Archive(_) | PackSource::Memory(_) => BundleFormat::Package,
            PackSource::Embedded(_) => BundleFormat::Embedded,
        }
    }

    /// Whether this pack is a folder on disk that supports live file editing.
    pub fn is_directory(&self) -> bool {
        matches!(self.source, PackSource::Directory(_))
    }

    /// Whether an icon file exists on disk in a directory pack (even if temporarily locked).
    pub fn has_icon_on_disk(&self, id: IconId) -> bool {
        match &self.source {
            PackSource::Directory(base) => {
                let rel = id.relative_path();
                let sub = if rel.ends_with(".svg") { rel.to_string() } else { format!("{rel}.svg") };
                base.join("icons").join(sub).exists()
            }
            _ => false,
        }
    }

    /// Whether this pack is an archive (.qicons).
    pub fn is_archive(&self) -> bool {
        matches!(self.source, PackSource::Archive(_))
    }

    /// Whether this pack is verified.
    pub fn is_verified(&self) -> bool {
        self.manifest.verified && !self.is_tampered
    }

    /// Reload the pack manifest from its source on disk if available.
    pub fn reload_manifest(&mut self) -> Result<(), String> {
        match &self.source {
            PackSource::Directory(dir) => {
                let manifest_path = dir.join("manifest.ron");
                if !manifest_path.is_file() {
                    return Err(format!("missing manifest.ron in {}", dir.display()));
                }
                let content = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
                let manifest = IconManifest::parse_ron(&content).map_err(|e| format!("parse error: {e}"))?;
                manifest.validate().map_err(|e| format!("invalid manifest in {}: {e}", dir.display()))?;
                self.manifest = manifest;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Load an icon pack from a directory on disk.
    pub fn from_directory(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref();
        let manifest_path = dir.join("manifest.ron");
        if !manifest_path.is_file() {
            return Err(format!("missing manifest.ron in {}", dir.display()));
        }
        let content = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let manifest = IconManifest::parse_ron(&content).map_err(|e| format!("parse error: {e}"))?;
        manifest.validate().map_err(|e| format!("invalid manifest in {}: {e}", dir.display()))?;
        Ok(Self { manifest, source: PackSource::Directory(dir.to_path_buf()), is_tampered: false, duplicate_conflict: None })
    }

    /// Load an icon pack from a `.qicons` package bundle file.
    pub fn from_archive(file_path: impl AsRef<Path>) -> Result<Self, String> {
        let file_path = file_path.as_ref();
        if file_path.extension().is_none_or(|e| e != "qicons") {
            return Err("icon package file must have .qicons extension".to_string());
        }
        let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_ARCHIVE_FILE_SIZE {
            return Err(format!("archive file size exceeds limit: {} bytes (limit is {MAX_ARCHIVE_FILE_SIZE})", metadata.len()));
        }

        let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
        let trailer_check = super::sha256::verify_qicons_trailer_stream(&mut file, metadata.len()).map_err(|e| e.to_string())?;

        let (is_verified, is_tampered) = match trailer_check {
            super::sha256::TrailerCheck::IntegrityOk => (true, false),
            super::sha256::TrailerCheck::IntegrityBad => (false, true),
            super::sha256::TrailerCheck::NoTrailer => (false, true),
        };

        file.seek(std::io::SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        // Guard against decompression bombs (Zip Bombs), excessive file counts, and zip-slip
        validate_archive_safety(&mut zip)?;

        let manifest_file = zip.by_name("manifest.ron").map_err(|_| "missing manifest.ron in icon package".to_string())?;
        let mut content = String::new();
        manifest_file.take(MAX_MANIFEST_SIZE + 1).read_to_string(&mut content).map_err(|e| e.to_string())?;
        if content.len() as u64 > MAX_MANIFEST_SIZE {
            return Err("manifest.ron exceeds maximum allowed size".to_string());
        }
        let mut manifest = IconManifest::parse_ron(&content).map_err(|e| format!("parse error: {e}"))?;
        manifest.validate().map_err(|e| format!("invalid manifest in {}: {e}", file_path.display()))?;
        manifest.verified = is_verified;

        Ok(Self { manifest, source: PackSource::Archive(file_path.to_path_buf()), is_tampered, duplicate_conflict: None })
    }

    /// Read a validated archive once for manager previews; 106 separate icon reads took about 100 ms on a 248 KB archive.
    pub fn archive_snapshot(&self) -> Result<Self, String> {
        let PackSource::Archive(path) = &self.source else {
            return Ok(self.clone());
        };
        let file = std::fs::File::open(path).map_err(|err| err.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|err| err.to_string())?;
        validate_archive_safety(&mut zip)?;
        let map = archive_entries(&mut zip)?;
        Ok(Self { manifest: self.manifest.clone(), source: PackSource::Memory(map), is_tampered: self.is_tampered, duplicate_conflict: self.duplicate_conflict.clone() })
    }

    /// Load an icon pack from in-memory ZIP archive bytes (e.g. from `include_bytes!`).
    pub fn from_zip_bytes(bytes: &[u8]) -> Result<Self, String> {
        let trailer_check = super::sha256::verify_qicons_trailer(bytes);
        let cursor = std::io::Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;

        // Guard against decompression bombs and unsafe archives
        validate_archive_safety(&mut zip)?;

        let is_verified = trailer_check == super::sha256::TrailerCheck::IntegrityOk;
        let mut manifest = {
            let manifest_file = zip.by_name("manifest.ron").map_err(|_| "missing manifest.ron in archive".to_string())?;
            let mut content = String::new();
            manifest_file.take(MAX_MANIFEST_SIZE + 1).read_to_string(&mut content).map_err(|e| e.to_string())?;
            if content.len() as u64 > MAX_MANIFEST_SIZE {
                return Err("manifest.ron exceeds maximum allowed size".to_string());
            }
            let parsed = IconManifest::parse_ron(&content).map_err(|e| format!("parse error: {e}"))?;
            parsed.validate().map_err(|e| format!("invalid manifest: {e}"))?;
            parsed
        };
        if is_verified || manifest.id == DEFAULT_THEME_ID {
            manifest.verified = true;
        }

        let map = archive_entries(&mut zip)?;

        Ok(Self { manifest, source: PackSource::Memory(map), is_tampered: trailer_check == super::sha256::TrailerCheck::IntegrityBad, duplicate_conflict: None })
    }

    /// Load an embedded icon pack from in-memory ZIP archive bytes (e.g. from `include_bytes!`).
    pub fn from_embedded_zip_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut pack = Self::from_zip_bytes(bytes)?;
        pack.manifest.verified = true;
        pack.is_tampered = false;
        if let PackSource::Memory(map) = pack.source {
            pack.source = PackSource::Embedded(map);
        }
        Ok(pack)
    }

    /// Retrieve raw SVG bytes for a relative path inside `icons/` (e.g. `"sketch/line.svg"`).
    pub fn get_svg(&self, rel_path: &str) -> Option<Vec<u8>> {
        let clean = rel_path.trim_start_matches('/');
        let file_subpath = if clean.ends_with(".svg") { clean.to_string() } else { format!("{clean}.svg") };

        let data = match &self.source {
            PackSource::Directory(base) => {
                let p = base.join("icons").join(&file_subpath);
                read_svg_with_retry(&p)?
            }
            PackSource::Archive(archive_path) => {
                let file = std::fs::File::open(archive_path).ok()?;
                let mut zip = zip::ZipArchive::new(file).ok()?;
                let full_name = format!("icons/{file_subpath}");
                let index = zip.index_for_name(&full_name).or_else(|| zip.index_for_name(&file_subpath))?;
                let entry = zip.by_index(index).ok()?;
                if entry.size() > MAX_ICON_SVG_SIZE {
                    return None;
                }
                let mut buf = Vec::new();
                entry.take(MAX_ICON_SVG_SIZE + 1).read_to_end(&mut buf).ok()?;
                if buf.len() as u64 > MAX_ICON_SVG_SIZE {
                    return None;
                }
                buf
            }
            PackSource::Memory(map) | PackSource::Embedded(map) => {
                let full_name = format!("icons/{file_subpath}");
                map.get(&full_name).or_else(|| map.get(&file_subpath))?.clone()
            }
        };

        if self.format() != BundleFormat::Embedded {
            if !is_svg_safe(&data) {
                return None;
            }
            if super::bundle::validate_icon_svg(&data).is_err() {
                return None;
            }
        }
        Some(data)
    }

    /// Retrieve raw SVG bytes for an `IconId`.
    pub fn get_svg_for_id(&self, id: IconId) -> Option<Vec<u8>> {
        self.get_svg(id.relative_path())
    }

    /// Read an icon for preview, distinguishing an absent file from a file that cannot be used.
    pub fn inspect_svg_for_id(&self, id: IconId) -> Result<Option<Vec<u8>>, String> {
        let file_subpath = format!("{}.svg", id.relative_path());
        let full_name = format!("icons/{file_subpath}");
        let data = match &self.source {
            PackSource::Directory(base) => {
                let path = base.join(&full_name);
                let metadata = match std::fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(err) => return Err(format!("cannot inspect SVG file: {err}")),
                };
                if !metadata.is_file() {
                    return Err("SVG path is not a file".to_string());
                }
                if metadata.len() > MAX_ICON_SVG_SIZE {
                    return Err(format!("SVG file exceeds {MAX_ICON_SVG_SIZE} byte limit"));
                }
                if metadata.len() == 0 {
                    return Err("SVG file is empty".to_string());
                }
                read_svg_with_retry(&path).ok_or_else(|| "SVG file is empty or cannot be read".to_string())?
            }
            PackSource::Archive(archive_path) => {
                let file = std::fs::File::open(archive_path).map_err(|err| format!("cannot open archive: {err}"))?;
                let mut zip = zip::ZipArchive::new(file).map_err(|err| format!("cannot read archive: {err}"))?;
                let Some(index) = zip.index_for_name(&full_name).or_else(|| zip.index_for_name(&file_subpath)) else {
                    return Ok(None);
                };
                let entry = zip.by_index(index).map_err(|err| format!("cannot read SVG entry: {err}"))?;
                if entry.size() > MAX_ICON_SVG_SIZE {
                    return Err(format!("SVG file exceeds {MAX_ICON_SVG_SIZE} byte limit"));
                }
                let mut data = Vec::new();
                entry.take(MAX_ICON_SVG_SIZE + 1).read_to_end(&mut data).map_err(|err| format!("cannot read SVG entry: {err}"))?;
                data
            }
            PackSource::Memory(map) | PackSource::Embedded(map) => {
                let Some(data) = map.get(&full_name).or_else(|| map.get(&file_subpath)) else {
                    return Ok(None);
                };
                data.clone()
            }
        };

        if data.len() as u64 > MAX_ICON_SVG_SIZE {
            return Err(format!("SVG file exceeds {MAX_ICON_SVG_SIZE} byte limit"));
        }
        if data.is_empty() {
            return Err("SVG file is empty".to_string());
        }
        super::bundle::validate_icon_svg(&data)?;
        if !is_svg_safe(&data) {
            return Err("SVG failed safety checks (external reference, entity expansion, script, or NUL byte)".to_string());
        }
        Ok(Some(data))
    }

    /// Return the pack's root icon.svg, falling back to the built-in pack image.
    pub fn get_pack_icon_svg(&self) -> Vec<u8> {
        let data = match &self.source {
            PackSource::Directory(base) => {
                let path = base.join("icon.svg");
                if std::fs::metadata(&path).ok().is_some_and(|m| m.len() <= MAX_ICON_SVG_SIZE) {
                    std::fs::read(path).ok()
                } else {
                    None
                }
            }
            PackSource::Archive(archive_path) => {
                let file = std::fs::File::open(archive_path).ok();
                file.and_then(|file| zip::ZipArchive::new(file).ok()).and_then(|mut zip| {
                    let entry = zip.by_name("icon.svg").ok()?;
                    if entry.size() > MAX_ICON_SVG_SIZE {
                        return None;
                    }
                    let mut data = Vec::new();
                    entry.take(MAX_ICON_SVG_SIZE + 1).read_to_end(&mut data).ok()?;
                    (data.len() as u64 <= MAX_ICON_SVG_SIZE).then_some(data)
                })
            }
            PackSource::Memory(map) | PackSource::Embedded(map) => map.get("icon.svg").cloned(),
        };
        data.filter(|svg| is_svg_safe(svg) && super::bundle::validate_svg(svg).is_ok()).unwrap_or_else(|| DEFAULT_PACK_ICON_SVG.to_vec())
    }

    /// Return the pack's root `icon.svg` resolved against an active color palette.
    pub fn get_pack_icon_svg_resolved(&self, palette: &qymcad_scheme::Palette) -> Vec<u8> {
        let raw = self.get_pack_icon_svg();
        super::manager::resolve_icon_tokens(&raw, palette)
    }

    /// Alias for [`Self::get_pack_icon_svg_resolved`].
    pub fn get_pack_icon_svg_for_palette(&self, palette: &qymcad_scheme::Palette) -> Vec<u8> {
        self.get_pack_icon_svg_resolved(palette)
    }

    /// List all known `IconId`s present in this pack.
    pub fn available_icons(&self) -> Vec<IconId> {
        ALL_ICONS.iter().copied().filter(|id| self.get_svg_for_id(*id).is_some()).collect()
    }

    /// Calculate coverage as CoverageCount { present, total }.
    pub fn coverage(&self) -> super::bundle::CoverageCount {
        let count = self.available_icons().len();
        super::bundle::CoverageCount { present: count, total: ALL_ICONS.len() }
    }

    /// Retrieve README markdown text for this icon pack, or fallback to a formatted manifest description.
    pub fn get_readme(&self) -> String {
        self.get_readme_for_locale("")
    }

    /// Retrieve the requested language's README, then the base README or manifest text.
    pub fn get_readme_for_locale(&self, locale: &str) -> String {
        let try_file = |name: &str| -> Option<String> {
            match &self.source {
                PackSource::Directory(base) => {
                    let p = base.join(name);
                    if std::fs::metadata(&p).ok()?.len() > MAX_TEXT_FILE_SIZE {
                        return None;
                    }
                    std::fs::read_to_string(p).ok()
                }
                PackSource::Archive(archive_path) => {
                    let file = std::fs::File::open(archive_path).ok()?;
                    let mut zip = zip::ZipArchive::new(file).ok()?;
                    let entry = zip.by_name(name).ok()?;
                    if entry.size() > MAX_TEXT_FILE_SIZE {
                        return None;
                    }
                    let mut s = String::new();
                    entry.take(MAX_TEXT_FILE_SIZE + 1).read_to_string(&mut s).ok()?;
                    if s.len() as u64 > MAX_TEXT_FILE_SIZE {
                        return None;
                    }
                    Some(s)
                }
                PackSource::Memory(map) | PackSource::Embedded(map) => map.get(name).filter(|bytes| bytes.len() as u64 <= MAX_TEXT_FILE_SIZE).and_then(|bytes| String::from_utf8(bytes.clone()).ok()),
            }
        };

        for tag in locale_fallbacks(locale) {
            for prefix in &["README", "description"] {
                let candidate = format!("{prefix}.{tag}.md");
                if let Some(text) = try_file(&candidate).filter(|text| !text.trim().is_empty()) {
                    return text;
                }
            }
        }

        for candidate in &["README.md", "readme.md", "README.txt", "description.md", "description.txt"] {
            if let Some(text) = try_file(candidate) {
                if !text.trim().is_empty() {
                    return text;
                }
            }
        }

        // Fallback: build markdown text from manifest
        let mut out = format!("# {}\n\n", self.manifest.name_for_locale(locale));
        let description = self.manifest.description_for_locale(locale);
        if !description.is_empty() {
            out.push_str(description);
            out.push_str("\n\n");
        }
        out.push_str(&format!("- **ID**: `{}`\n- **Version**: `{}`\n- **Author**: {}\n- **License**: `{}`\n", self.manifest.id, self.manifest.version, self.manifest.author, self.manifest.license));
        out
    }

    /// Retrieve preview image data and extension if provided in the pack (e.g. preview.svg, preview.png).
    pub fn get_preview_image(&self) -> Option<ThemePreviewAsset> {
        let try_file = |candidate: &str| -> Option<Vec<u8>> {
            match &self.source {
                PackSource::Directory(base) => {
                    let p = base.join(candidate);
                    std::fs::read(p).ok()
                }
                PackSource::Archive(archive_path) => {
                    let file = std::fs::File::open(archive_path).ok()?;
                    let mut zip = zip::ZipArchive::new(file).ok()?;
                    let entry = zip.by_name(candidate).ok()?;
                    if entry.size() > MAX_SINGLE_FILE_UNCOMPRESSED_SIZE {
                        return None;
                    }
                    let mut buf = Vec::new();
                    entry.take(MAX_SINGLE_FILE_UNCOMPRESSED_SIZE + 1).read_to_end(&mut buf).ok()?;
                    if buf.len() as u64 > MAX_SINGLE_FILE_UNCOMPRESSED_SIZE {
                        return None;
                    }
                    Some(buf)
                }
                PackSource::Memory(map) | PackSource::Embedded(map) => map.get(candidate).cloned(),
            }
        };

        for (candidate, ext) in &[("preview.svg", "svg"), ("preview.png", "png"), ("preview.webp", "webp")] {
            if let Some(bytes) = try_file(candidate) {
                return Some(ThemePreviewAsset { data: bytes, extension: ext });
            }
        }
        None
    }

    /// Take a full snapshot of all SVG files and manifest.ron in this directory pack,
    /// mapping relative path -> FileSignature.
    pub fn directory_snapshot(&self) -> Option<HashMap<PathBuf, FileSignature>> {
        let PackSource::Directory(base) = &self.source else {
            return None;
        };

        let mut snapshot = HashMap::new();
        let manifest_path = base.join("manifest.ron");
        if let Ok(m) = std::fs::metadata(&manifest_path) {
            let mtime = m.modified().unwrap_or(std::time::UNIX_EPOCH);
            snapshot.insert(PathBuf::from("manifest.ron"), FileSignature { modified: mtime, len: m.len() });
        }

        let icon_path = base.join("icon.svg");
        if let Ok(m) = std::fs::metadata(&icon_path) {
            let mtime = m.modified().unwrap_or(std::time::UNIX_EPOCH);
            snapshot.insert(PathBuf::from("icon.svg"), FileSignature { modified: mtime, len: m.len() });
        }

        let icons_dir = base.join("icons");
        collect_svgs_recursively(&icons_dir, base, &mut snapshot);

        Some(snapshot)
    }

    /// Returns the maximum modification timestamp among files in this directory pack,
    /// or None if this is not a directory.
    pub fn latest_mtime(&self) -> Option<std::time::SystemTime> {
        let snap = self.directory_snapshot()?;
        snap.values().map(|sig| sig.modified).max()
    }
}

/// Helper to read SVG file non-blockingly without stalling the UI rendering thread.
fn read_svg_with_retry(path: &Path) -> Option<Vec<u8>> {
    match std::fs::metadata(path) {
        Ok(m) if m.len() > 0 && m.len() <= MAX_ICON_SVG_SIZE => {
            if let Ok(mut file) = std::fs::File::open(path) {
                let mut buf = Vec::with_capacity(m.len() as usize);
                if file.by_ref().take(MAX_ICON_SVG_SIZE + 1).read_to_end(&mut buf).is_ok() && buf.len() as u64 <= MAX_ICON_SVG_SIZE && !buf.is_empty() {
                    return Some(buf);
                }
            }
            None
        }
        _ => None,
    }
}

/// Recursively collect all .svg files in a directory, ignoring temporary and editor swap files.
fn collect_svgs_recursively(dir: &Path, base: &Path, map: &mut HashMap<PathBuf, FileSignature>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            collect_svgs_recursively(&path, base, map);
        } else if file_type.is_file() {
            if let Ok(name) = entry.file_name().into_string() {
                let name_lower = name.to_lowercase();
                // Only track valid SVG files, ignoring temporary/editor swap files and OS artifacts
                if name_lower.ends_with(".svg") && !name.starts_with('.') && !name.starts_with('~') && !name.ends_with(".tmp") && !name.ends_with(".bak") && !name.ends_with('~') {
                    if let Ok(m) = entry.metadata() {
                        let mtime = m.modified().unwrap_or(std::time::UNIX_EPOCH);
                        if let Ok(rel) = path.strip_prefix(base) {
                            map.insert(rel.to_path_buf(), FileSignature { modified: mtime, len: m.len() });
                        }
                    }
                }
            }
        }
    }
}

/// Security check for SVG icon bytes:
/// Protects against memory exhaustion, XML entity bombs, scripts, null bytes, and non-SVG data.
pub fn is_svg_safe(buf: &[u8]) -> bool {
    // 1. Guard against memory exhaustion (icons should not exceed 512 KB)
    if buf.len() as u64 > MAX_ICON_SVG_SIZE {
        return false;
    }
    // 2. Must be valid UTF-8
    let Ok(text) = std::str::from_utf8(buf) else {
        return false;
    };
    let lower = text.to_lowercase();
    // 3. Must contain root SVG element
    if !lower.contains("<svg") {
        return false;
    }
    // 4. Guard against XML entity expansion / billion laughs attack
    if lower.contains("<!entity") || lower.contains("system \"") || lower.contains("system '") {
        return false;
    }
    // 5. Guard against executable script tags
    if lower.contains("<script") {
        return false;
    }
    // 6. Guard against null bytes
    if buf.contains(&0) {
        return false;
    }
    true
}
