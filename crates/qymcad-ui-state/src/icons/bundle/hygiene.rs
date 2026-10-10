//! SVG validation, security sanitization, and junk element hygiene repair.

use std::path::Path;

use super::id::IconId;
use super::pack::IconPack;

/// Parse width and height from an SVG viewBox attribute value.
pub fn parse_viewbox_values(val_str: &str) -> Option<(f32, f32)> {
    let parts: Vec<&str> = val_str.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    if parts.len() == 4 {
        let x: f32 = parts[0].parse().ok()?;
        let y: f32 = parts[1].parse().ok()?;
        let w: f32 = parts[2].parse().ok()?;
        let h: f32 = parts[3].parse().ok()?;
        (x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()).then_some((w, h))
    } else {
        None
    }
}

/// Inspect SVG text for extraneous editor metadata, dangerous executable elements, or junk tags.
/// Returns a list of human-readable issues detected.
pub fn find_svg_junk_issues(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut issues = Vec::new();

    // Dangerous / executable tags
    if lower.contains("<script") {
        issues.push("prohibited <script> tag".to_string());
    }
    if lower.contains("<foreignobject") {
        issues.push("prohibited <foreignObject> tag".to_string());
    }
    for tag in ["<applet", "<object", "<embed", "<iframe", "<audio", "<video"] {
        if lower.contains(tag) {
            issues.push(format!("prohibited {tag}> tag"));
        }
    }

    // Inline event handlers (onload=, onclick=, onerror=, etc.)
    let mut search_from = 0;
    while let Some(idx) = lower[search_from..].find(" on") {
        let abs_idx = search_from + idx + 3;
        search_from = abs_idx;
        let rest = &lower[abs_idx..];
        if let Some(word) = rest.split_whitespace().next() {
            if let Some((attr, _)) = word.split_once('=') {
                if !attr.is_empty() && attr.chars().all(|c| c.is_ascii_alphabetic()) {
                    issues.push(format!("prohibited event handler attribute (on{attr})"));
                    break;
                }
            }
        }
    }
    if lower.contains("javascript:") {
        issues.push("prohibited javascript: URL".to_string());
    }

    // Editor metadata & proprietary elements or namespaces
    if lower.contains("<sodipodi:") || lower.contains("sodipodi:") || lower.contains("xmlns:sodipodi") {
        issues.push("editor metadata (Sodipodi)".to_string());
    }
    if lower.contains("<inkscape:") || lower.contains("inkscape:") || lower.contains("xmlns:inkscape") {
        issues.push("editor metadata (Inkscape)".to_string());
    }
    if lower.contains("<metadata") {
        issues.push("extraneous <metadata> block".to_string());
    }
    if lower.contains("<rdf:rdf") || lower.contains("xmlns:rdf") {
        issues.push("extraneous <rdf:RDF> block or namespace".to_string());
    }
    if lower.contains("xmlns:dc") || lower.contains("xmlns:cc") {
        issues.push("extraneous metadata namespace (dc/cc)".to_string());
    }
    if lower.contains("<i:pgf") || lower.contains("i:pgf") {
        issues.push("proprietary Illustrator metadata (<i:pgf>)".to_string());
    }
    if lower.contains("<adobe:") || lower.contains("adobe:") || lower.contains("<x:xmpmeta") || lower.contains("x:xmpmeta") {
        issues.push("proprietary Adobe/XMP metadata".to_string());
    }
    if lower.contains("<sketch:") || lower.contains("sketch:") {
        issues.push("proprietary Sketch metadata".to_string());
    }
    if lower.contains("<figma:") || lower.contains("figma:") {
        issues.push("proprietary Figma metadata".to_string());
    }
    if lower.contains("<!entity") {
        issues.push("prohibited <!ENTITY> declaration".to_string());
    }

    issues
}

fn validate_svg_element_security(element: &quick_xml::events::BytesStart<'_>) -> Result<Option<String>, String> {
    let name = std::str::from_utf8(element.local_name().as_ref()).map_err(|err| format!("invalid XML element name: {err}"))?.to_ascii_lowercase();

    if name == "image" {
        return Err("embedded raster images (<image>) are prohibited".to_string());
    }
    if matches!(name.as_str(), "script" | "foreignobject" | "applet" | "object" | "embed" | "iframe" | "audio" | "video") {
        return Err(format!("prohibited <{name}> tag"));
    }

    let mut viewbox = None;
    for attr in element.attributes() {
        let attr = attr.map_err(|err| format!("invalid attribute: {err}"))?;
        let key = std::str::from_utf8(attr.key.as_ref()).map_err(|err| format!("invalid attribute name: {err}"))?.to_ascii_lowercase();

        if key == "viewbox" {
            let val = attr.unescape_value().map_err(|err| format!("invalid viewBox attribute value: {err}"))?;
            viewbox = Some(val.into_owned());
        }
        if key.starts_with("on") && key.len() > 2 && key.chars().skip(2).all(|c| c.is_ascii_alphabetic()) {
            return Err(format!("prohibited event handler attribute '{key}'"));
        }
        if key == "href" || key.ends_with(":href") {
            let val = attr.unescape_value().map_err(|err| format!("invalid attribute value: {err}"))?.to_ascii_lowercase();
            if val.trim().starts_with("javascript:") {
                return Err("prohibited javascript: URL in href".to_string());
            }
        }
    }

    Ok(viewbox)
}

/// Validate SVG data according to the theme specification:
/// - Must be well-formed XML with properly nested tags.
/// - Must contain a single valid root `<svg>` element.
/// - Must contain a valid square `viewBox` attribute (1:1 aspect ratio, e.g. `viewBox="0 0 64 64"`).
/// - Must NOT contain embedded raster images (`<image>` or `data:image/`).
/// - Must NOT contain junk tags, editor metadata, or executable elements.
pub fn validate_svg(data: &[u8]) -> Result<(), String> {
    validate_svg_structural(data, true)
}

/// Validate SVG structure, optionally enforcing 1:1 square aspect ratio.
/// Non-square SVGs are allowed for theme banners (`preview.svg`), but must still pass
/// all security, XML well-formedness, and raster image checks.
pub fn validate_svg_structural(data: &[u8], require_square: bool) -> Result<(), String> {
    let text = std::str::from_utf8(data).map_err(|_| "SVG data is not valid UTF-8".to_string())?;

    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().check_end_names = true;

    let mut depth: usize = 0;
    let mut root_svg_found = false;
    let mut viewbox_attr: Option<String> = None;

    loop {
        use quick_xml::events::Event;
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = std::str::from_utf8(element.local_name().as_ref()).map_err(|err| format!("invalid XML element name: {err}"))?.to_ascii_lowercase();

                if depth == 0 {
                    if root_svg_found {
                        return Err("multiple root elements in SVG".to_string());
                    }
                    if name != "svg" {
                        return Err(format!("expected root element <svg>, found <{name}>"));
                    }
                    root_svg_found = true;
                    viewbox_attr = validate_svg_element_security(&element)?;
                } else {
                    let _ = validate_svg_element_security(&element)?;
                }
                depth += 1;
            }
            Ok(Event::Empty(element)) => {
                let name = std::str::from_utf8(element.local_name().as_ref()).map_err(|err| format!("invalid XML element name: {err}"))?.to_ascii_lowercase();

                if depth == 0 {
                    if root_svg_found {
                        return Err("multiple root elements in SVG".to_string());
                    }
                    if name != "svg" {
                        return Err(format!("expected root element <svg>, found <{name}>"));
                    }
                    root_svg_found = true;
                    viewbox_attr = validate_svg_element_security(&element)?;
                } else {
                    let _ = validate_svg_element_security(&element)?;
                }
            }
            Ok(Event::End(_)) => {
                if depth == 0 {
                    return Err("unexpected closing tag".to_string());
                }
                depth -= 1;
            }
            Ok(Event::DocType(_) | Event::Decl(_) | Event::Comment(_) | Event::Text(_)) => {}
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML syntax error: {e}")),
            _ => {}
        }
    }

    if !root_svg_found {
        return Err("missing <svg> root element".to_string());
    }
    if depth != 0 {
        return Err("unclosed XML tags in SVG".to_string());
    }

    let Some(viewbox_str) = viewbox_attr else {
        return Err("missing viewBox attribute (expected 1:1, e.g. viewBox=\"0 0 64 64\")".to_string());
    };

    let (w, h) = parse_viewbox_values(&viewbox_str).ok_or_else(|| "invalid viewBox attribute (expected four finite numbers)".to_string())?;
    if w <= 0.0 || h <= 0.0 {
        return Err(format!("non-positive viewBox dimensions: {w}x{h}"));
    }
    if require_square {
        let ratio = w / h;
        if !(0.95..=1.05).contains(&ratio) {
            return Err(format!("non-square viewBox: {w}x{h} (aspect ratio must be 1:1)"));
        }
    }

    if text.contains("data:image/") {
        return Err("embedded raster images (<image>) are prohibited".to_string());
    }

    let junk = find_svg_junk_issues(text);
    if !junk.is_empty() {
        return Err(format!("extraneous/junk tags detected: {}", junk.join(", ")));
    }

    Ok(())
}

pub fn validate_icon_svg(data: &[u8]) -> Result<(), String> {
    validate_svg(data)?;
    validate_icon_tokens(data)?;
    Ok(())
}

/// Check that any CSS `var(...)` references in the SVG use valid approved tokens and specify fallbacks.
/// Recursively validates nested `var(...)` expressions.
pub fn validate_icon_tokens(data: &[u8]) -> Result<(), String> {
    if !data.windows(4).any(|w| w == b"var(") {
        return Ok(());
    }
    let text = std::str::from_utf8(data).map_err(|err| format!("invalid UTF-8 in SVG: {err}"))?;
    validate_tokens_recursive(text, 0)
}

fn validate_tokens_recursive(text: &str, depth: usize) -> Result<(), String> {
    if depth > 10 {
        return Err("excessive var(...) nesting depth (> 10)".to_string());
    }
    let mut rest = text;

    while let Some(start_idx) = rest.find("var(") {
        let after_var = &rest[start_idx + 4..];
        let mut paren_depth = 0usize;
        let mut close_idx = None;
        for (idx, ch) in after_var.char_indices() {
            if ch == '(' {
                paren_depth += 1;
            } else if ch == ')' {
                if paren_depth == 0 {
                    close_idx = Some(idx);
                    break;
                }
                paren_depth -= 1;
            }
        }
        let end_idx = close_idx.ok_or_else(|| "SVG has unclosed var(...) expression".to_string())?;
        let inner = &after_var[..end_idx];
        let (tok, fallback) = inner.split_once(',').ok_or_else(|| format!("icon variable `{}` must specify a fallback color, e.g. var({}, #HEX)", inner.trim(), inner.trim()))?;

        let tok_trimmed = tok.trim();
        let fallback_trimmed = fallback.trim();
        if fallback_trimmed.is_empty() {
            return Err(format!("icon variable `{tok_trimmed}` has an empty fallback color"));
        }

        let token_name = tok_trimmed.strip_prefix("--").ok_or_else(|| format!("icon variable `{tok_trimmed}` must start with '--', e.g. --icon-stroke"))?;

        if !qymcad_scheme::ICON_TOKENS.contains(&token_name) {
            return Err(format!("unknown icon token `{tok_trimmed}`; supported tokens are {:?}", qymcad_scheme::ICON_TOKENS));
        }

        if fallback_trimmed.contains("var(") {
            validate_tokens_recursive(fallback_trimmed, depth + 1)?;
        }

        rest = &after_var[end_idx + 1..];
    }

    Ok(())
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

fn cleaned_svg_start(start: &quick_xml::events::BytesStart<'_>, is_root: bool, has_xlink: bool, changed: &mut bool) -> Result<quick_xml::events::BytesStart<'static>, String> {
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
        let root_junk = is_root && (lower_key == "id" || lower_key == "width" || lower_key == "height" || lower_key == "version");
        let external_link = lower_key.ends_with("href") && ["http:", "https:", "file:", "javascript:", "//"].iter().any(|prefix| lower_value.trim_start().starts_with(prefix));

        if event_handler || editor_attribute || export_attribute || non_standard_xmlns || root_junk || lower_value.contains("data:image/") || external_link {
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

    if is_root {
        if !has_xmlns {
            *changed = true;
            cleaned.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
        }
        if has_xlink && !has_xmlns_xlink {
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
                writer.write_event(Event::Start(cleaned_svg_start(&start, is_root, has_xlink, &mut changed)?)).map_err(|err| err.to_string())?;
            }
            Event::Empty(_) if skipped_depth > 0 => {}
            Event::Empty(empty) if removable_svg_element(empty.name().as_ref()) => changed = true,
            Event::Empty(empty) => {
                let is_root = open_depth == 0 && empty.name().as_ref() == b"svg";
                writer.write_event(Event::Empty(cleaned_svg_start(&empty, is_root, has_xlink, &mut changed)?)).map_err(|err| err.to_string())?;
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
    static CLEAN_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = CLEAN_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("svg.qymcad-{}-{counter}.tmp", std::process::id()));

    struct TempFileGuard<'a> {
        path: &'a Path,
        active: bool,
    }
    impl<'a> Drop for TempFileGuard<'a> {
        fn drop(&mut self) {
            if self.active {
                let _ = std::fs::remove_file(self.path);
            }
        }
    }

    let mut guard = TempFileGuard { path: &temp, active: true };
    if let Err(err) = std::fs::write(&temp, &cleaned) {
        return Err(format!("cannot write cleaned SVG: {err}"));
    }
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

/// Remove residual temporary files (`*.svg.qymcad-<pid>-<counter>.tmp`) created during failed SVG cleaning attempts.
/// Traverses directory entries using `file_type()` without following symbolic links to prevent loops.
pub fn cleanup_residual_tmp_files(dir: &Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            cleanup_residual_tmp_files(&entry.path());
        } else if file_type.is_file() {
            let name = entry.file_name();
            if let Some(name_str) = name.to_str() {
                if is_cleaner_temp_file(name_str) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

pub fn clean_directory_icon(pack: &IconPack, id: IconId) -> Result<CleanIconResult, String> {
    if !pack.is_directory() {
        return Err("only editable directory packs can be cleaned".to_string());
    }
    let super::pack::PackSource::Directory(root) = &pack.source else {
        return Err("only directory packs can be cleaned".to_string());
    };
    let file_path = root.join("icons").join(format!("{}.svg", id.relative_path()));
    if let Some(parent) = file_path.parent() {
        cleanup_residual_tmp_files(parent);
    }
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
            if is_cleaner_temp_file(name) {
                let _ = std::fs::remove_file(&path);
            } else if path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
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
    cleanup_residual_tmp_files(root);
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
    cleanup_residual_tmp_files(root);
    Ok(report)
}
