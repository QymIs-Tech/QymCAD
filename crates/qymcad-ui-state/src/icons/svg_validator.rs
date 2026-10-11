//! Shared SVG policy used by runtime loading, packaging, and build-time checks.

use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvgPurpose {
    Icon,
    Artwork,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgDiagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub byte_offset: usize,
}

struct ViewboxCapture {
    text: String,
    byte_offset: usize,
}

#[derive(Default)]
struct SvgParseState {
    root_found: bool,
    viewbox: Option<ViewboxCapture>,
    style_depth: Option<usize>,
}

impl Display for SvgDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at line {}, column {}", self.message, self.line, self.column)
    }
}

impl std::error::Error for SvgDiagnostic {}

fn diagnostic(data: &[u8], byte_offset: usize, message: impl Into<String>) -> SvgDiagnostic {
    let byte_offset = byte_offset.min(data.len());
    let before = &data[..byte_offset];
    let line = before.iter().filter(|byte| **byte == b'\n').count() + 1;
    let column_offset = before.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1);
    let column = std::str::from_utf8(&data[column_offset..byte_offset]).map_or(byte_offset - column_offset + 1, |text| text.chars().count() + 1);
    SvgDiagnostic { message: message.into(), line, column, byte_offset }
}

// Complexity note: `location_of` is evaluated lazily only when an error diagnostic or
// token/viewBox inspection requires exact byte locations. Normal attribute validation
// bypasses it completely.
fn location_of(data: &[u8], base: usize, fragment: &[u8]) -> usize {
    if fragment.is_empty() {
        return base;
    }
    data.get(base..)
        .and_then(|tail| {
            let first = fragment[0];
            let mut offset = 0;
            while let Some(pos) = tail[offset..].iter().position(|&b| b == first) {
                let match_pos = offset + pos;
                if tail.get(match_pos..match_pos + fragment.len()) == Some(fragment) {
                    return Some(match_pos);
                }
                offset = match_pos + 1;
            }
            None
        })
        .map_or(base, |offset| base + offset)
}

/// Width and height parsed from an SVG viewBox attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewboxDimensions {
    pub width: f32,
    pub height: f32,
}

pub fn parse_viewbox_values(value: &str) -> Option<ViewboxDimensions> {
    let parts: Vec<&str> = value.split(|character: char| character == ',' || character.is_whitespace()).filter(|part| !part.is_empty()).collect();
    if parts.len() != 4 {
        return None;
    }
    let x = parts[0].parse::<f32>().ok()?;
    let y = parts[1].parse::<f32>().ok()?;
    let width = parts[2].parse::<f32>().ok()?;
    let height = parts[3].parse::<f32>().ok()?;
    (x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()).then_some(ViewboxDimensions { width, height })
}

fn is_forbidden_element(name: &str) -> bool {
    let local = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
    matches!(local.as_str(), "image" | "script" | "foreignobject" | "applet" | "object" | "embed" | "iframe" | "audio" | "video" | "metadata")
        || ["sodipodi:", "inkscape:", "adobe:", "sketch:", "figma:"].iter().any(|prefix| name.to_ascii_lowercase().starts_with(prefix))
        || is_numeric_namespace_name(name)
        || matches!(name.to_ascii_lowercase().as_str(), "rdf:rdf" | "i:pgf" | "x:xmpmeta")
}

fn is_numeric_namespace_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("ns") && lower.find(':').is_some_and(|colon| colon > 2 && lower[2..colon].bytes().all(|byte| byte.is_ascii_digit()))
}

pub(crate) fn unsafe_reference(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.starts_with('#')
}

pub(crate) fn external_css_reference(value: &str) -> Option<usize> {
    let lower = value.to_ascii_lowercase();
    if let Some(index) = lower.find("@import") {
        return Some(index);
    }
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find("url") {
        let start = cursor + relative;
        let after_word = start + 3;
        let whitespace = lower[after_word..].len() - lower[after_word..].trim_start().len();
        let open_start = after_word + whitespace;
        let Some(open) = lower[open_start..].find('(').map(|index| open_start + index) else {
            cursor = after_word;
            continue;
        };
        if !lower[after_word..open].chars().all(char::is_whitespace) {
            cursor = after_word;
            continue;
        }
        let raw = value[open + 1..].trim_start();
        let raw = raw.strip_prefix(['\'', '"']).unwrap_or(raw);
        if unsafe_reference(raw) {
            return Some(start);
        }
        cursor = open + 1;
    }
    None
}

fn attribute_value_offset(data: &[u8], attribute_offset: usize, key: &str) -> usize {
    let after_key = attribute_offset.saturating_add(key.len());
    let Some(relative_equal) = data.get(after_key..).and_then(|tail| tail.iter().position(|byte| *byte == b'=')) else {
        return attribute_offset;
    };
    let after_equal = after_key + relative_equal + 1;
    let Some(value_start) = data.get(after_equal..).and_then(|tail| tail.iter().position(|byte| !byte.is_ascii_whitespace())).map(|offset| after_equal + offset) else {
        return after_equal;
    };
    if matches!(data.get(value_start), Some(b'\'' | b'"')) {
        value_start + 1
    } else {
        value_start
    }
}

/// Scan context tracking base offset and recursion depth for token validation.
#[derive(Clone, Copy, Debug)]
struct TokenScanContext {
    pub base_offset: usize,
    pub depth: usize,
}

fn validate_tokens(text: &str, data: &[u8], ctx: TokenScanContext) -> Result<(), SvgDiagnostic> {
    if ctx.depth > 10 {
        return Err(diagnostic(data, ctx.base_offset, "excessive var(...) nesting depth (> 10)"));
    }
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("var(") {
        let start = cursor + relative;
        let inside_start = start + 4;
        let mut nesting = 0usize;
        let mut end = None;
        for (index, character) in text[inside_start..].char_indices() {
            if character == '(' {
                nesting += 1;
            } else if character == ')' {
                if nesting == 0 {
                    end = Some(inside_start + index);
                    break;
                }
                nesting -= 1;
            }
        }
        let Some(end) = end else {
            return Err(diagnostic(data, ctx.base_offset + start, "unclosed CSS var() expression"));
        };
        let inside = &text[inside_start..end];
        let Some((token, fallback)) = inside.split_once(',') else {
            return Err(diagnostic(data, ctx.base_offset + start, "icon color variable must specify a fallback color"));
        };
        let token = token.trim();
        let fallback = fallback.trim();
        if fallback.is_empty() {
            return Err(diagnostic(data, ctx.base_offset + start, "icon color variable has an empty fallback color"));
        }
        let Some(name) = token.strip_prefix("--") else {
            return Err(diagnostic(data, ctx.base_offset + start, "icon color variable must start with '--'"));
        };
        if !qymcad_scheme::ICON_TOKENS.contains(&name) {
            return Err(diagnostic(data, ctx.base_offset + start, format!("unknown icon token `{token}`")));
        }
        if let Some(nested) = fallback.find("var(") {
            validate_tokens(fallback, data, TokenScanContext { base_offset: ctx.base_offset + inside_start + inside.find(fallback).unwrap_or(0) + nested, depth: ctx.depth + 1 })?;
        }
        cursor = end + 1;
    }
    Ok(())
}

/// Validate a standalone SVG using the shared icon or artwork rules.
pub fn validate_svg(data: &[u8], purpose: SvgPurpose) -> Result<(), SvgDiagnostic> {
    let size_limit = if purpose == SvgPurpose::Icon { 512 * 1024 } else { 2 * 1024 * 1024 };
    if data.len() > size_limit {
        return Err(diagnostic(data, 0, format!("SVG file exceeds the {size_limit} byte limit")));
    }
    let text = std::str::from_utf8(data).map_err(|error| diagnostic(data, error.valid_up_to(), "SVG data is not valid UTF-8"))?;
    if let Some(offset) = data.iter().position(|byte| *byte == 0) {
        return Err(diagnostic(data, offset, "SVG contains a NUL byte"));
    }
    if let Some(offset) = text.to_ascii_lowercase().find("javascript:") {
        return Err(diagnostic(data, offset, "prohibited javascript: URL"));
    }

    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    // Invariant: after a well-formed SVG, state.root_found == true, root_closed == true, depth == 0.
    // root_closed becomes true when depth returns to 0 (via Event::End) or when an Empty element
    // is at depth 0 (self-closing <svg ... />).
    let mut depth = 0usize;
    let mut state = SvgParseState::default();
    let mut root_closed = false;

    loop {
        use quick_xml::events::Event;
        let event_start = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|error| {
            let offset = reader.error_position() as usize;
            diagnostic(data, offset, format!("XML syntax error: {error}"))
        })?;
        match event {
            Event::Start(element) => {
                validate_element(data, event_start, &element, depth, purpose, &mut state)?;
                depth += 1;
            }
            Event::Empty(element) => {
                validate_element(data, event_start, &element, depth, purpose, &mut state)?;
                if depth == 0 {
                    root_closed = true;
                }
            }
            Event::End(element) => {
                if depth == 0 {
                    return Err(diagnostic(data, event_start, "unexpected closing tag"));
                }
                if state.style_depth == Some(depth) {
                    state.style_depth = None;
                }
                depth -= 1;
                if depth == 0 {
                    root_closed = true;
                }
                let _ = element;
            }
            Event::Text(text_event) => {
                let text_bytes: &[u8] = text_event.as_ref();
                if depth == 0 && !text_bytes.iter().all(u8::is_ascii_whitespace) {
                    return Err(diagnostic(data, event_start, "text outside the SVG root element"));
                }
                if state.style_depth.is_some() {
                    let decoded = text_event.xml_content().map_err(|error| diagnostic(data, event_start, format!("invalid SVG text: {error}")))?;
                    if let Some(offset) = external_css_reference(&decoded) {
                        return Err(diagnostic(data, event_start + offset, "external CSS references are prohibited"));
                    }
                    if purpose == SvgPurpose::Icon {
                        validate_tokens(&decoded, data, TokenScanContext { base_offset: event_start, depth: 0 })?;
                    }
                }
            }
            Event::CData(cdata) => {
                if depth == 0 {
                    return Err(diagnostic(data, event_start, "CDATA outside the SVG root element"));
                }
                if state.style_depth.is_some() {
                    let content = std::str::from_utf8(cdata.as_ref()).map_err(|error| diagnostic(data, event_start, format!("invalid SVG style text: {error}")))?;
                    if let Some(offset) = external_css_reference(content) {
                        return Err(diagnostic(data, event_start + offset, "external CSS references are prohibited"));
                    }
                }
            }
            Event::GeneralRef(reference) => {
                let name: &[u8] = reference.as_ref();
                if !name.starts_with(b"#") && ![b"amp".as_slice(), b"lt", b"gt", b"quot", b"apos"].contains(&name) {
                    return Err(diagnostic(data, event_start, "undeclared XML entity reference"));
                }
            }
            Event::DocType(_) => return Err(diagnostic(data, event_start, "DOCTYPE and entity declarations are prohibited")),
            Event::PI(_) => return Err(diagnostic(data, event_start, "XML processing instructions are prohibited")),
            Event::Eof => break,
            _ => {}
        }
    }

    if !state.root_found {
        return Err(diagnostic(data, 0, "missing <svg> root element"));
    }
    if depth != 0 || !root_closed {
        return Err(diagnostic(data, data.len(), "unclosed SVG root element"));
    }
    let Some(viewbox) = state.viewbox else {
        return Err(diagnostic(data, 0, "missing viewBox attribute"));
    };
    let Some(dims) = parse_viewbox_values(&viewbox.text) else {
        return Err(diagnostic(data, viewbox.byte_offset, "invalid viewBox attribute (expected four finite numbers)"));
    };
    if dims.width <= 0.0 || dims.height <= 0.0 {
        return Err(diagnostic(data, viewbox.byte_offset, "viewBox width and height must be positive"));
    }
    if purpose == SvgPurpose::Icon && !(0.95..=1.05).contains(&(dims.width / dims.height)) {
        return Err(diagnostic(data, viewbox.byte_offset, format!("non-square viewBox: {}x{} (aspect ratio must be 1:1)", dims.width, dims.height)));
    }
    if purpose == SvgPurpose::Icon {
        validate_tokens(text, data, TokenScanContext { base_offset: 0, depth: 0 })?;
    }
    Ok(())
}

pub fn validate_icon_tokens(data: &[u8]) -> Result<(), SvgDiagnostic> {
    let text = std::str::from_utf8(data).map_err(|error| diagnostic(data, error.valid_up_to(), "SVG data is not valid UTF-8"))?;
    validate_tokens(text, data, TokenScanContext { base_offset: 0, depth: 0 })
}

fn validate_element(data: &[u8], event_start: usize, element: &quick_xml::events::BytesStart<'_>, depth: usize, purpose: SvgPurpose, state: &mut SvgParseState) -> Result<(), SvgDiagnostic> {
    let qualified_name = element.name();
    let name = std::str::from_utf8(qualified_name.as_ref()).unwrap_or("");
    let local = element.local_name();
    let local_name = std::str::from_utf8(local.as_ref()).unwrap_or("").to_ascii_lowercase();
    if depth == 0 {
        if state.root_found {
            return Err(diagnostic(data, event_start, "multiple root elements in SVG"));
        }
        if local_name != "svg" {
            return Err(diagnostic(data, event_start, format!("expected root element <svg>, found <{name}>")));
        }
        state.root_found = true;
    }
    if is_forbidden_element(name) {
        if local_name == "image" {
            return Err(diagnostic(data, event_start, "embedded raster images (<image>) are prohibited"));
        }
        return Err(diagnostic(data, event_start, format!("prohibited SVG element <{name}>")));
    }
    if local_name == "style" {
        state.style_depth = Some(depth + 1);
    }
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| diagnostic(data, event_start, format!("invalid XML attribute: {error}")))?;
        let key = std::str::from_utf8(attribute.key.as_ref()).unwrap_or("");
        let value = attribute.unescape_value().map_err(|error| diagnostic(data, event_start, format!("invalid XML attribute value: {error}")))?;
        let lower_key = key.to_ascii_lowercase();
        let get_attribute_offset = || location_of(data, event_start, key.as_bytes());
        if lower_key.starts_with("on") && lower_key.len() > 2 && lower_key[2..].bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(diagnostic(data, get_attribute_offset(), format!("prohibited event handler attribute `{key}`")));
        }
        if lower_key == "xml:base" {
            return Err(diagnostic(data, get_attribute_offset(), "xml:base is prohibited"));
        }
        if (lower_key == "href" || lower_key.ends_with(":href")) && unsafe_reference(&value) {
            return Err(diagnostic(data, get_attribute_offset(), "external SVG references are prohibited; use a local fragment such as `#shape`"));
        }
        let lower_name = lower_key.as_str();
        if ["sodipodi:", "inkscape:", "adobe:", "sketch:", "figma:", "i:", "x:"].iter().any(|prefix| lower_name.starts_with(prefix))
            || (is_numeric_namespace_name(key) && !lower_key.ends_with(":href"))
        {
            return Err(diagnostic(data, get_attribute_offset(), format!("editor metadata attribute `{key}` is prohibited")));
        }
        if lower_key.starts_with("xmlns:") && lower_key != "xmlns:xlink" {
            return Err(diagnostic(data, get_attribute_offset(), format!("editor metadata namespace `{key}` is prohibited")));
        }
        if value.to_ascii_lowercase().contains("data:image/") {
            let attribute_offset = get_attribute_offset();
            let value_offset = attribute_value_offset(data, attribute_offset, key);
            return Err(diagnostic(data, value_offset, "embedded raster images are prohibited"));
        }
        if let Some(offset) = external_css_reference(&value) {
            let attribute_offset = get_attribute_offset();
            let value_offset = attribute_value_offset(data, attribute_offset, key);
            return Err(diagnostic(data, value_offset + offset, "external CSS references are prohibited"));
        }
        if lower_key == "style" && purpose == SvgPurpose::Icon {
            let attribute_offset = get_attribute_offset();
            let value_offset = attribute_value_offset(data, attribute_offset, key);
            validate_tokens(&value, data, TokenScanContext { base_offset: value_offset, depth: 0 })?;
        }
        if depth == 0 && lower_key == "viewbox" {
            let attribute_offset = get_attribute_offset();
            state.viewbox = Some(ViewboxCapture { text: value.into_owned(), byte_offset: attribute_offset });
        }
    }
    Ok(())
}
