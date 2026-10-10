//! Pack discovery, diagnostic inspection, and validation reporting.

use std::path::{Path, PathBuf};

use super::hygiene::{validate_icon_svg, validate_svg, validate_svg_structural};
use super::id::{IconId, ALL_ICONS};
use super::manifest::IconManifest;
use super::pack::IconPack;

pub(crate) fn localized_readme_tag(name: &str) -> Option<&str> {
    let tag = name.strip_prefix("README.")?.strip_suffix(".md")?;
    tag.parse::<unic_langid::LanguageIdentifier>().ok()?;
    Some(tag)
}

pub(crate) fn localized_description_tag(name: &str) -> Option<&str> {
    let tag = name.strip_prefix("description.")?.strip_suffix(".md")?;
    tag.parse::<unic_langid::LanguageIdentifier>().ok()?;
    Some(tag)
}

/// A rejected icon file or manifest within an icon pack.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RejectedArchive {
    pub path: String,
    pub reason: String,
}

/// The number of icons present in a pack relative to the total set of icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoverageCount {
    pub present: usize,
    pub total: usize,
}

/// Category coverage breakdown for an icon pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryCoverage {
    pub category: &'static str,
    pub present: usize,
    pub total: usize,
}

/// Detailed diagnostic report of an icon theme directory or archive.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    /// Valid icons matching a known `IconId` that passed all SVG checks and will be included in the bundle.
    pub included: Vec<IconId>,
    /// Known CAD icons from `ALL_ICONS` that are not provided by this pack (will fall back to default theme).
    pub missing: Vec<IconId>,
    /// Files that failed SVG validation, with relative path and error description.
    pub rejected: Vec<RejectedArchive>,
    /// Extraneous files found (non-SVG files or unrecognized icon names).
    pub extraneous: Vec<String>,
}

impl ValidationReport {
    /// Total included icons and total known CAD icons (e.g. 45 of 107).
    pub fn coverage(&self) -> CoverageCount {
        CoverageCount { present: self.included.len(), total: ALL_ICONS.len() }
    }

    /// Total percentage coverage (0 to 100).
    pub fn coverage_percent(&self) -> usize {
        let cov = self.coverage();
        (cov.present * 100).checked_div(cov.total).unwrap_or(0)
    }

    /// Whether there are any issues (rejected or extraneous files).
    pub fn has_issues(&self) -> bool {
        !self.rejected.is_empty() || !self.extraneous.is_empty()
    }

    /// Counts of included icons per category.
    pub fn category_breakdown(&self) -> Vec<CategoryCoverage> {
        const CATEGORIES: &[&str] = &["sketch", "constraint", "part", "assembly", "datum"];
        let mut breakdown = Vec::new();
        for &cat in CATEGORIES {
            let total = ALL_ICONS.iter().filter(|id| id.relative_path().starts_with(cat)).count();
            let inc = self.included.iter().filter(|id| id.relative_path().starts_with(cat)).count();
            breakdown.push(CategoryCoverage { category: cat, present: inc, total });
        }
        breakdown
    }
}

/// A rejected theme directory or archive with the reason for rejection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryError {
    pub path: PathBuf,
    pub reason: String,
}

/// The result of discovering icon packs in a directory, including rejected items.
#[derive(Clone, Debug, Default)]
pub struct DiscoveryReport {
    pub packs: Vec<IconPack>,
    pub errors: Vec<DiscoveryError>,
}

/// Discover icon packs from a directory, returning both valid packs and any rejection errors.
pub fn discover_packs_detailed(dir: &Path) -> DiscoveryReport {
    let mut packs = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return DiscoveryReport { packs, errors };
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            let manifest_path = path.join("manifest.ron");
            if manifest_path.exists() {
                super::hygiene::cleanup_residual_tmp_files(&path);
                match IconPack::from_directory(&path) {
                    Ok(pack) => packs.push(pack),
                    Err(err) => errors.push(DiscoveryError { path, reason: err }),
                }
            }
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "qicons") {
            match IconPack::from_archive(&path) {
                Ok(pack) => packs.push(pack),
                Err(err) => errors.push(DiscoveryError { path, reason: err }),
            }
        }
    }
    packs.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    errors.sort_by(|a, b| a.path.cmp(&b.path));
    DiscoveryReport { packs, errors }
}

/// Discover icon packs from a directory (subdirectories with `manifest.ron` and `.qicons` archives).
pub fn discover_packs_in(dir: &Path) -> Vec<IconPack> {
    discover_packs_detailed(dir).packs
}

/// Inspect and validate an icon pack directory, returning a detailed `ValidationReport`.
/// Checks all SVG viewports, identifies extraneous/unknown files, and lists included vs missing icons.
pub fn inspect_pack_directory(source_dir: impl AsRef<Path>) -> Result<ValidationReport, String> {
    let source_dir = source_dir.as_ref();
    super::hygiene::cleanup_residual_tmp_files(source_dir);
    let icons_dir = source_dir.join("icons");
    if !icons_dir.is_dir() {
        return Err(format!("missing icons/ directory in {}", source_dir.display()));
    }

    let mut included = Vec::new();
    let mut rejected = Vec::new();
    let mut extraneous = Vec::new();

    // 1. Inspect root files in source_dir (excluding icons/)
    if let Ok(entries) = std::fs::read_dir(source_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let allowed = fname == "manifest.ron"
                    || fname == "icon.svg"
                    || fname.starts_with("LICENSE")
                    || fname == "README.md"
                    || fname == "README.txt"
                    || fname == "description.md"
                    || fname == "description.txt"
                    || localized_readme_tag(fname).is_some()
                    || localized_description_tag(fname).is_some()
                    || fname.starts_with("preview.")
                    || fname.ends_with(".qicons");
                if !allowed {
                    extraneous.push(format!("extra root file: {fname}"));
                } else if localized_readme_tag(fname).is_some() || localized_description_tag(fname).is_some() {
                    if std::fs::metadata(&p).ok().is_some_and(|metadata| metadata.len() > super::pack::MAX_TEXT_FILE_SIZE) {
                        rejected.push(RejectedArchive { path: fname.to_string(), reason: "localized documentation exceeds maximum text size".to_string() });
                    } else if let Err(err) = std::fs::read_to_string(&p) {
                        rejected.push(RejectedArchive { path: fname.to_string(), reason: format!("localized documentation is not readable UTF-8: {err}") });
                    }
                } else if fname == "preview.svg" {
                    if let Ok(content) = std::fs::read(&p) {
                        if let Err(err) = validate_svg_structural(&content, false) {
                            rejected.push(RejectedArchive { path: fname.to_string(), reason: format!("preview.svg validation error: {err}") });
                        }
                    }
                } else if fname == "manifest.ron" {
                    if std::fs::metadata(&p).ok().is_some_and(|m| m.len() > super::pack::MAX_MANIFEST_SIZE) {
                        rejected.push(RejectedArchive { path: fname.to_string(), reason: "manifest.ron exceeds maximum manifest size".to_string() });
                    } else {
                        match std::fs::read_to_string(&p) {
                            Ok(content) => match IconManifest::parse_ron(&content) {
                                Ok(parsed) => {
                                    if let Err(err) = parsed.validate() {
                                        rejected.push(RejectedArchive { path: fname.to_string(), reason: err });
                                    }
                                }
                                Err(err) => rejected.push(RejectedArchive { path: fname.to_string(), reason: format!("manifest parse error: {err}") }),
                            },
                            Err(err) => rejected.push(RejectedArchive { path: fname.to_string(), reason: format!("read error: {err}") }),
                        }
                    }
                } else if fname == "icon.svg" {
                    if std::fs::metadata(&p).ok().is_some_and(|m| m.len() > super::pack::MAX_ICON_SVG_SIZE) {
                        rejected.push(RejectedArchive { path: fname.to_string(), reason: "pack icon exceeds maximum SVG size".to_string() });
                    } else {
                        match std::fs::read(&p) {
                            Ok(data) => {
                                if let Err(err) = validate_svg(&data) {
                                    rejected.push(RejectedArchive { path: fname.to_string(), reason: err });
                                }
                            }
                            Err(err) => rejected.push(RejectedArchive { path: fname.to_string(), reason: format!("read error: {err}") }),
                        }
                    }
                }
            }
        }
    }

    // 2. Recursively walk icons/ directory
    fn walk_icons(base: &Path, current: &Path, included: &mut Vec<IconId>, rejected: &mut Vec<RejectedArchive>, extraneous: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(current) else { return };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else { continue };
            let p = entry.path();
            if file_type.is_dir() {
                walk_icons(base, &p, included, rejected, extraneous);
            } else if file_type.is_file() {
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let rel_str = rel.to_string_lossy().replace('\\', "/");

                // Non-SVG files in icons/ are extraneous
                if !rel_str.ends_with(".svg") {
                    extraneous.push(format!("non-SVG file in icons/: {rel_str}"));
                    continue;
                }

                // Check if recognized CAD IconId
                let Some(id) = IconId::from_id_str(&rel_str) else {
                    extraneous.push(format!("unrecognized icon path: {rel_str} (not a known CAD icon)"));
                    continue;
                };

                // Validate SVG content, size, and compression ratio
                match std::fs::read(&p) {
                    Ok(data) => {
                        if data.len() as u64 > super::pack::MAX_ICON_SVG_SIZE {
                            rejected.push(RejectedArchive { path: rel_str, reason: format!("SVG exceeds {} byte limit", super::pack::MAX_ICON_SVG_SIZE) });
                            continue;
                        }
                        match validate_icon_svg(&data) {
                            Ok(()) => {
                                if !included.contains(&id) {
                                    included.push(id);
                                }
                            }
                            Err(err) => {
                                rejected.push(RejectedArchive { path: rel_str, reason: err });
                            }
                        }
                    }
                    Err(e) => {
                        rejected.push(RejectedArchive { path: rel_str, reason: format!("read error: {e}") });
                    }
                }
            }
        }
    }

    walk_icons(&icons_dir, &icons_dir, &mut included, &mut rejected, &mut extraneous);
    included.sort_by_key(|id| id.relative_path());

    // 3. Compute missing icons from standard catalog
    let missing: Vec<IconId> = ALL_ICONS.iter().copied().filter(|id| !included.contains(id)).collect();

    Ok(ValidationReport { included, missing, rejected, extraneous })
}
