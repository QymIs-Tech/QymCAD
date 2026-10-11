//! SVG validation, security sanitization, and junk element hygiene repair.

use std::io::Write;
use std::path::Path;

use super::id::IconId;
use super::pack::IconPack;

pub use super::super::svg_validator::{parse_viewbox_values, ViewboxDimensions};

/// Return shared-validator diagnostics for SVG security and structure issues.
pub fn find_svg_junk_issues(text: &str) -> Vec<String> {
    super::super::svg_validator::validate_svg(text.as_bytes(), super::super::svg_validator::SvgPurpose::Artwork).err().map(|error| vec![error.to_string()]).unwrap_or_default()
}

/// Validate SVG bytes and retain the exact source location of a failure.
pub fn validate_svg_detailed(data: &[u8], purpose: super::super::svg_validator::SvgPurpose) -> Result<(), super::super::svg_validator::SvgDiagnostic> {
    super::super::svg_validator::validate_svg(data, purpose)
}

/// Validate an icon using the shared SVG policy.
pub fn validate_svg(data: &[u8]) -> Result<(), String> {
    validate_svg_detailed(data, super::super::svg_validator::SvgPurpose::Icon).map_err(|error| error.to_string())
}

/// Validate SVG structure, allowing non-square artwork when `require_square` is false.
pub fn validate_svg_structural(data: &[u8], require_square: bool) -> Result<(), String> {
    let purpose = if require_square { super::super::svg_validator::SvgPurpose::Icon } else { super::super::svg_validator::SvgPurpose::Artwork };
    validate_svg_detailed(data, purpose).map_err(|error| error.to_string())
}

pub fn validate_icon_svg(data: &[u8]) -> Result<(), String> {
    validate_svg(data)
}

pub fn validate_icon_tokens(data: &[u8]) -> Result<(), String> {
    super::super::svg_validator::validate_icon_tokens(data).map_err(|error| error.to_string())
}

fn removable_svg_element(name: &[u8]) -> bool {
    let lower = String::from_utf8_lossy(name).to_ascii_lowercase();
    let local = lower.rsplit(':').next().unwrap_or(&lower);
    matches!(local, "script" | "foreignobject" | "applet" | "object" | "embed" | "iframe" | "audio" | "video" | "metadata" | "image")
        || lower.starts_with("sodipodi:")
        || lower.starts_with("inkscape:")
        || lower.starts_with("adobe:")
        || lower.starts_with("sketch:")
        || lower.starts_with("figma:")
        || (lower.starts_with("ns") && lower.find(':').is_some_and(|idx| lower[2..idx].chars().all(|c| c.is_ascii_digit())))
        || matches!(lower.as_str(), "rdf:rdf" | "i:pgf" | "x:xmpmeta")
}

#[derive(Clone, Copy, Debug)]
struct SvgCleanFlags {
    pub is_root: bool,
    pub has_xlink: bool,
}

fn cleaned_svg_start(start: &quick_xml::events::BytesStart<'_>, flags: SvgCleanFlags, changed: &mut bool) -> Result<quick_xml::events::BytesStart<'static>, String> {
    let mut cleaned = start.to_owned();
    cleaned.clear_attributes();
    let mut has_xmlns = false;
    let mut has_xmlns_xlink = false;

    for attr in start.attributes() {
        let attr = attr.map_err(|err| format!("invalid SVG attribute: {err}"))?;
        let key = std::str::from_utf8(attr.key.as_ref()).map_err(|err| format!("invalid SVG attribute name: {err}"))?;
        let value = attr.unescape_value().map_err(|err| format!("invalid SVG attribute value: {err}"))?;
        let lower_key = key.to_ascii_lowercase();
        let lower_value = value.to_ascii_lowercase();
        let event_handler = lower_key.strip_prefix("on").is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphabetic()));
        let editor_attribute = ["sodipodi:", "inkscape:", "adobe:", "sketch:", "figma:", "i:", "x:"].iter().any(|prefix| lower_key.starts_with(prefix))
            || (lower_key.starts_with("ns") && lower_key.find(':').is_some_and(|idx| lower_key[2..idx].chars().all(|c| c.is_ascii_digit()) && !lower_key.ends_with("href")));
        let export_attribute = lower_key.starts_with("export-") || lower_key.contains(":export-");
        let non_standard_xmlns = lower_key.starts_with("xmlns:") && lower_key != "xmlns:xlink";
        let root_junk = flags.is_root && (lower_key == "id" || lower_key == "width" || lower_key == "height" || lower_key == "version");
        let external_link = lower_key.ends_with("href") && super::super::svg_validator::unsafe_reference(&value);
        let external_style = super::super::svg_validator::external_css_reference(&value).is_some();

        if event_handler || editor_attribute || export_attribute || non_standard_xmlns || root_junk || lower_value.contains("data:image/") || external_link || external_style {
            *changed = true;
        } else if lower_key.ends_with(":href") && lower_key != "xlink:href" {
            *changed = true;
            cleaned.push_attribute(("xlink:href", value.as_ref()));
        } else if lower_key == "style" && (lower_value.contains("-inkscape-") || lower_value.contains("-sodipodi-") || lower_value.contains("inkscape-")) {
            *changed = true;
            let mut cleaned_style = Vec::new();
            for part in value.split(';') {
                let part_trimmed = part.trim();
                if part_trimmed.is_empty() {
                    continue;
                }
                if let Some((prop, _val)) = part_trimmed.split_once(':') {
                    let prop_lower = prop.trim().to_ascii_lowercase();
                    if prop_lower.starts_with("-inkscape-") || prop_lower.starts_with("-sodipodi-") || prop_lower.starts_with("inkscape-") {
                        continue;
                    }
                }
                cleaned_style.push(part_trimmed);
            }
            if !cleaned_style.is_empty() {
                cleaned.push_attribute(("style", cleaned_style.join(";").as_str()));
            }
        } else {
            if lower_key == "xmlns" {
                has_xmlns = true;
            } else if lower_key == "xmlns:xlink" {
                has_xmlns_xlink = true;
            }
            cleaned.push_attribute((key, value.as_ref()));
        }
    }

    if flags.is_root {
        if !has_xmlns {
            *changed = true;
            cleaned.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
        }
        if flags.has_xlink && !has_xmlns_xlink {
            *changed = true;
            cleaned.push_attribute(("xmlns:xlink", "http://www.w3.org/1999/xlink"));
        }
    }

    Ok(cleaned)
}

/// Remove executable tags, editor metadata, raster content, and event attributes from SVG.
/// Geometry and viewBox values are retained; validation refuses changes that need manual repair.
pub fn clean_svg(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() as u64 > super::pack::MAX_ICON_SVG_SIZE {
        return Err("SVG file exceeds the icon size limit".to_string());
    }
    let source = std::str::from_utf8(data).map_err(|err| format!("SVG data is not valid UTF-8: {err}"))?;
    let has_xlink = source.contains("xlink:href") || source.contains(":href");
    let mut reader = quick_xml::Reader::from_str(source);
    let mut writer = quick_xml::Writer::new(Vec::with_capacity(data.len()));
    let mut skipped_depth = 0usize;
    let mut open_depth = 0usize;
    let mut saw_svg_root = false;
    let mut changed = false;
    loop {
        use quick_xml::events::Event;
        let event = reader.read_event().map_err(|err| format!("cannot parse SVG: {err}"))?;
        if open_depth == 0 {
            match &event {
                Event::Text(text) => {
                    let bytes: &[u8] = text.as_ref();
                    if !bytes.iter().all(u8::is_ascii_whitespace) {
                        return Err("SVG contains text outside its root element".to_string());
                    }
                }
                Event::CData(_) | Event::GeneralRef(_) => return Err("SVG contains content outside its root element".to_string()),
                _ => {}
            }
        }
        match &event {
            Event::Start(start) | Event::Empty(start) if open_depth == 0 => {
                if saw_svg_root || start.name().as_ref() != b"svg" {
                    return Err("SVG must have one <svg> root element".to_string());
                }
                saw_svg_root = true;
            }
            _ => {}
        }
        match &event {
            Event::Start(_) => open_depth += 1,
            Event::End(_) => open_depth = open_depth.checked_sub(1).ok_or_else(|| "SVG has an unmatched closing tag".to_string())?,
            _ => {}
        }
        match event {
            Event::Start(_) if skipped_depth > 0 => skipped_depth += 1,
            Event::Start(start) if removable_svg_element(start.name().as_ref()) => {
                skipped_depth = 1;
                changed = true;
            }
            Event::Start(start) => {
                let is_root = open_depth == 1 && start.name().as_ref() == b"svg";
                writer.write_event(Event::Start(cleaned_svg_start(&start, SvgCleanFlags { is_root, has_xlink }, &mut changed)?)).map_err(|err| err.to_string())?;
            }
            Event::Empty(_) if skipped_depth > 0 => {}
            Event::Empty(empty) if removable_svg_element(empty.name().as_ref()) => changed = true,
            Event::Empty(empty) => {
                let is_root = open_depth == 0 && empty.name().as_ref() == b"svg";
                writer.write_event(Event::Empty(cleaned_svg_start(&empty, SvgCleanFlags { is_root, has_xlink }, &mut changed)?)).map_err(|err| err.to_string())?;
            }
            Event::End(_) if skipped_depth > 0 => skipped_depth -= 1,
            Event::Text(text) if open_depth == 0 => {
                let bytes: &[u8] = text.as_ref();
                if !bytes.iter().all(u8::is_ascii_whitespace) {
                    return Err("SVG contains text outside its root element".to_string());
                }
                changed = true;
            }
            Event::Decl(_) | Event::DocType(_) | Event::PI(_) | Event::Comment(_) => changed = true,
            Event::Eof => break,
            _ if skipped_depth > 0 => {}
            Event::GeneralRef(reference) => {
                let name: &[u8] = reference.as_ref();
                if name.starts_with(b"#") || [b"amp".as_slice(), b"lt", b"gt", b"quot", b"apos"].contains(&name) {
                    writer.write_event(Event::GeneralRef(reference)).map_err(|err| err.to_string())?;
                } else {
                    changed = true;
                }
            }
            other => writer.write_event(other).map_err(|err| err.to_string())?,
        }
    }
    if !saw_svg_root || open_depth != 0 || skipped_depth != 0 {
        return Err("SVG has an unclosed or missing root element".to_string());
    }
    let mut cleaned = writer.into_inner();
    if !cleaned.ends_with(b"\n") {
        cleaned.push(b'\n');
    }
    if cleaned.len() as u64 > super::pack::MAX_ICON_SVG_SIZE {
        return Err("cleaned SVG exceeds the icon size limit".to_string());
    }
    validate_svg(&cleaned)?;
    if !changed && validate_svg(data).is_err() {
        return Err("SVG needs manual repair".to_string());
    }
    Ok(cleaned)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanIconResult {
    Missing,
    Unchanged,
    Cleaned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanPackReport {
    pub cleaned: Vec<std::path::PathBuf>,
    pub failed: Vec<CleanFileFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanFileFailure {
    pub path: std::path::PathBuf,
    pub reason: String,
}

fn clean_svg_file(path: &Path) -> Result<CleanIconResult, String> {
    let original = match std::fs::read(path) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(CleanIconResult::Missing),
        Err(err) => return Err(format!("cannot read SVG: {err}")),
    };
    if original.len() as u64 > super::pack::MAX_ICON_SVG_SIZE {
        return Err("SVG file exceeds the icon size limit".to_string());
    }
    if validate_svg(&original).is_ok() {
        return Ok(CleanIconResult::Unchanged);
    }
    let cleaned = clean_svg(&original)?;
    if cleaned == original {
        return Err("SVG needs manual repair".to_string());
    }
    let TempCleanerFile { path: temp, file: mut temp_file } = create_cleaner_temp(path)?;

    struct TempFileGuard {
        path: std::path::PathBuf,
        active: bool,
    }
    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            if self.active {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    let mut guard = TempFileGuard { path: temp.clone(), active: true };
    if let Err(err) = temp_file.write_all(&cleaned) {
        return Err(format!("cannot write cleaned SVG: {err}"));
    }
    drop(temp_file);
    let mut rename_err = None;
    for attempt in 0..6 {
        match std::fs::rename(&temp, path) {
            Ok(()) => {
                guard.active = false;
                return Ok(CleanIconResult::Cleaned);
            }
            Err(err) => {
                rename_err = Some(err);
                if attempt < 5 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }
    Err(format!("cannot replace SVG: {}", rename_err.unwrap()))
}

struct TempCleanerFile {
    pub path: std::path::PathBuf,
    pub file: std::fs::File,
}

fn create_cleaner_temp(path: &Path) -> Result<TempCleanerFile, String> {
    static CLEAN_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let process_id = std::process::id();
    for _attempt in 0..100 {
        let counter = CLEAN_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!("{}.svg.qymcad-{process_id}-{counter}.tmp", path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("icon"));
        let temp = path.with_file_name(&name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok(TempCleanerFile { path: temp, file }),
            Err(error) => {
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(format!("cannot create temporary SVG: {error}"));
                }
            }
        }
    }
    Err("cannot reserve a unique temporary SVG file".to_string())
}

/// Check whether a file name matches the SVG cleaner's temporary file pattern
/// (`*.svg.qymcad-<pid>-<counter>.tmp`).
pub fn is_cleaner_temp_file(name: &str) -> bool {
    let Some(rest) = name.strip_suffix(".tmp") else {
        return false;
    };
    let Some((_stem, marker)) = rest.rsplit_once(".svg.qymcad-") else {
        return false;
    };
    let mut parts = marker.split('-');
    let Some(pid) = parts.next() else { return false };
    let Some(counter) = parts.next() else { return false };
    parts.next().is_none() && !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit()) && !counter.is_empty() && counter.chars().all(|c| c.is_ascii_digit())
}

pub fn clean_directory_icon(pack: &IconPack, id: IconId) -> Result<CleanIconResult, String> {
    if !pack.is_directory() {
        return Err("only editable directory packs can be cleaned".to_string());
    }
    let super::pack::PackSource::Directory(root) = &pack.source else {
        return Err("only directory packs can be cleaned".to_string());
    };
    let file_path = root.join("icons").join(format!("{}.svg", id.relative_path()));
    clean_svg_file(&file_path)
}

fn collect_svg_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(format!("cannot inspect icon directory: {err}")),
    };
    for entry in entries {
        let entry = entry.map_err(|err| format!("cannot inspect icon entry: {err}"))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|err| format!("cannot inspect icon file: {err}"))?;
        if kind.is_dir() {
            collect_svg_files(&path, files)?;
        } else if kind.is_file() {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
            if !is_cleaner_temp_file(name) && path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
                files.push(path);
            }
        }
    }
    Ok(())
}

/// Whether bulk cleaning can change at least one SVG in an editable directory pack.
pub fn directory_has_cleanable_icons(pack: &IconPack) -> Result<bool, String> {
    if !pack.is_directory() {
        return Ok(false);
    }
    let super::pack::PackSource::Directory(root) = &pack.source else {
        return Ok(false);
    };
    let mut files = Vec::new();
    collect_svg_files(&root.join("icons"), &mut files)?;
    files.push(root.join("icon.svg"));
    for path in files {
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        if data.len() as u64 > super::pack::MAX_ICON_SVG_SIZE || validate_svg(&data).is_ok() {
            continue;
        }
        if clean_svg(&data).is_ok_and(|cleaned| cleaned != data) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn clean_directory_icons(pack: &IconPack) -> Result<CleanPackReport, String> {
    if !pack.is_directory() {
        return Err("only editable directory packs can be cleaned".to_string());
    }
    let super::pack::PackSource::Directory(root) = &pack.source else {
        return Err("only directory packs can be cleaned".to_string());
    };
    let mut files = Vec::new();
    collect_svg_files(&root.join("icons"), &mut files)?;
    files.push(root.join("icon.svg"));
    files.sort();
    let mut report = CleanPackReport { cleaned: Vec::new(), failed: Vec::new() };
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        match clean_svg_file(&path) {
            Ok(CleanIconResult::Cleaned) => report.cleaned.push(relative),
            Ok(CleanIconResult::Missing | CleanIconResult::Unchanged) => {}
            Err(reason) => report.failed.push(CleanFileFailure { path: relative, reason }),
        }
    }
    Ok(report)
}
